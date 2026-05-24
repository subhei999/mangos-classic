use super::*;

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_near_teleport_effect(
    stream: &mut WorldPacketSink,

    deps: SpellCastDeps<'_>,

    session: &mut WorldSessionState,

    character_guid: u32,

    map_id: u32,

    spell_template: &wow_db::SpellTemplateQuery,

    effect: SpellInfoEffect,

    targets: &SpellCastTargets,

    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(destination) = spell_target_destination_position(map_id, targets).or_else(|| {
        player_near_teleport_forward_destination(
            deps.shared_world.maps,
            session,
            spell_template,
            effect,
            map_id,
        )
    }) else {
        warn!(
            character_guid,
            "Skipping near teleport spell with missing destination"
        );

        return Ok(());
    };

    let position = {
        let Some(character) = session.character.active_character.as_mut() else {
            return Ok(());
        };

        character.position = WorldPosition::new(
            destination.map_id,
            destination.x,
            destination.y,
            destination.z,
            character.position.orientation,
        );

        character.movement_flags = 0;

        character.fall_time = 0;

        character.position
    };

    let old_map_id = map_id;

    let environment_packets = deps
        .shared_world
        .maps
        .set_player_position(old_map_id, character_guid, position)
        .await?;

    deps.shared_world
        .maps
        .reset_player_visibility_scan_positions(old_map_id, character_guid)
        .await;

    deps.shared_world
        .maps
        .sync_player_gameplay_state(old_map_id, character_guid, session)
        .await;

    wow_db::update_character_position(
        deps.character_db_pool,
        deps.account_id,
        character_guid,
        position,
    )
    .await?;

    send_packet(
        stream,
        WorldOpcode::MsgMoveTeleportAck as u16,
        &build_near_teleport_ack_body(session.character.active_character.as_ref().unwrap(), 0)?,
        Some(&mut *header_crypto),
    )
    .await?;

    deps.shared_world
        .sessions
        .dispatch(environment_packets)
        .await;

    stream_newly_visible_db_creatures(
        stream,
        deps.character_db_pool,
        deps.world_db_pool,
        deps.shared_world.maps,
        session,
        header_crypto,
    )
    .await?;

    Ok(())
}

pub(in crate::world) fn player_near_teleport_forward_destination(
    maps: &MapRuntimeManager,

    session: &WorldSessionState,

    spell_template: &wow_db::SpellTemplateQuery,

    effect: SpellInfoEffect,

    map_id: u32,
) -> Option<WorldPosition> {
    if effect.dispatch != SpellEffectDispatch::Leap {
        return None;
    }

    if plan_effect_target(effect) != SpellPlanEffectTarget::CasterFrontLeap {
        return None;
    }

    let character = session.character.active_character.as_ref()?;

    let distance = maps
        .spell_radius(effect.radius_index)
        .map(|radius| radius.radius)
        .or_else(|| {
            maps.spell_range(spell_template.range_index)
                .map(|range| range.max_range)
        })?;

    if distance <= 0.0 || !distance.is_finite() {
        return None;
    }

    near_teleport_front_leap_destination(
        WorldPosition::new(
            map_id,
            character.position.x,
            character.position.y,
            character.position.z,
            character.position.orientation,
        ),
        distance,
        |position| near_teleport_ground_position(&maps.geometry, position),
        |start, target| near_teleport_has_line_of_sight(&maps.geometry, start, target),
    )
}

pub(in crate::world) fn near_teleport_front_leap_destination(
    start: WorldPosition,
    distance: f32,
    mut ground_position: impl FnMut(WorldPosition) -> Option<WorldPosition>,
    mut has_line_of_sight: impl FnMut(WorldPosition, WorldPosition) -> bool,
) -> Option<WorldPosition> {
    if distance <= 0.0 || !distance.is_finite() {
        return None;
    }

    let step_length = 2.0_f32;
    let max_slope_radians = 50.0_f32.to_radians();
    let segments = (distance / step_length).ceil().max(1.0) as u32;
    let end_x = start.x + start.orientation.cos() * distance;
    let end_y = start.y + start.orientation.sin() * distance;
    let delta_x = (end_x - start.x) / segments as f32;
    let delta_y = (end_y - start.y) / segments as f32;

    let mut previous = ground_position(start).unwrap_or(start);

    for step in 1..=segments {
        let probe = WorldPosition::new(
            start.map_id,
            start.x + delta_x * step as f32,
            start.y + delta_y * step as f32,
            previous.z,
            start.orientation,
        );
        let Some(next) = ground_position(probe) else {
            break;
        };
        let segment_length = distance_2d(previous.x, previous.y, next.x, next.y);
        if segment_length > f32::EPSILON {
            let slope = ((previous.z - next.z).abs() / segment_length).atan();
            if slope > max_slope_radians {
                break;
            }
        }
        if !has_line_of_sight(previous, next) {
            break;
        }
        previous = next;
    }

    Some(previous)
}

fn near_teleport_ground_position(
    geometry: &WorldGeometry,
    position: WorldPosition,
) -> Option<WorldPosition> {
    geometry
        .ground_position(position)
        .or_else(|| (!geometry.world_data_files.maps_available).then_some(position))
}

fn near_teleport_has_line_of_sight(
    geometry: &WorldGeometry,
    start: WorldPosition,
    target: WorldPosition,
) -> bool {
    if geometry.world_data_files.vmap_tiles.is_empty() {
        return true;
    }
    let Some(data_dir) = geometry.world_data_files.data_dir_for_native.as_ref() else {
        return true;
    };
    let Some(start_tile) = crate::world::mmap_tile_for_position(start) else {
        return false;
    };
    let Some(target_tile) = crate::world::mmap_tile_for_position(target) else {
        return false;
    };
    if !geometry
        .world_data_files
        .has_vmap_support_for_map(start.map_id)
        || !geometry
            .world_data_files
            .has_vmap_tile(start.map_id, start_tile.0, start_tile.1)
        || !geometry
            .world_data_files
            .has_vmap_tile(target.map_id, target_tile.0, target_tile.1)
    {
        return true;
    }

    crate::world::native_vmap_line_of_sight(
        data_dir,
        crate::world::unit_line_of_sight_position(start),
        crate::world::unit_line_of_sight_position(target),
        start_tile,
        target_tile,
        false,
    )
    .unwrap_or(true)
}

pub(in crate::world) async fn apply_item_teleport_spell_effect(
    stream: &mut WorldPacketSink,

    deps: SpellCastDeps<'_>,

    session: &mut WorldSessionState,

    character_guid: u32,

    old_map_id: u32,

    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(homebind) =
        wow_db::get_character_homebind(deps.character_db_pool, character_guid).await?
    else {
        warn!(
            character_guid,
            "Ignoring teleport item spell without character_homebind row"
        );

        return Ok(());
    };

    let Some(character) = session.character.active_character.as_mut() else {
        return Ok(());
    };

    character.position = homebind;

    character.movement_flags = 0;

    character.fall_time = 0;

    let environment_packets = deps
        .shared_world
        .maps
        .set_player_position(old_map_id, character_guid, homebind)
        .await?;

    deps.shared_world
        .maps
        .reset_player_visibility_scan_positions(old_map_id, character_guid)
        .await;

    deps.shared_world
        .maps
        .sync_player_gameplay_state(old_map_id, character_guid, session)
        .await;

    wow_db::update_character_position(
        deps.character_db_pool,
        deps.account_id,
        character_guid,
        homebind,
    )
    .await?;

    send_packet(
        stream,
        WorldOpcode::MsgMoveTeleportAck as u16,
        &build_near_teleport_ack_body(session.character.active_character.as_ref().unwrap(), 0)?,
        Some(&mut *header_crypto),
    )
    .await?;

    deps.shared_world
        .sessions
        .dispatch(environment_packets)
        .await;

    stream_newly_visible_db_creatures(
        stream,
        deps.character_db_pool,
        deps.world_db_pool,
        deps.shared_world.maps,
        session,
        header_crypto,
    )
    .await?;

    Ok(())
}
