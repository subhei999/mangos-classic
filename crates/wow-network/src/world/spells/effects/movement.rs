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

    let orientation = character.position.orientation;

    let destination = WorldPosition::new(
        map_id,
        character.position.x + orientation.cos() * distance,
        character.position.y + orientation.sin() * distance,
        character.position.z,
        orientation,
    );

    Some(
        maps.geometry
            .ground_position(destination)
            .unwrap_or(destination),
    )
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
