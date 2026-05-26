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
    let mut preserve_orientation = true;
    let Some(destination) = ({
        if let Some(destination) = spell_target_destination_position(map_id, targets) {
            Some(destination)
        } else if let Some(destination) =
            player_near_teleport_database_destination(deps.world_db_pool, spell_template, effect)
                .await?
        {
            preserve_orientation = false;
            Some(destination)
        } else {
            player_near_teleport_forward_destination(
                deps.shared_world.maps,
                session,
                spell_template,
                effect,
                map_id,
            )
        }
    }) else {
        warn!(
            character_guid,
            "Skipping near teleport spell with missing destination"
        );

        return Ok(());
    };
    if destination.map_id != map_id {
        return apply_player_far_teleport_effect(
            stream,
            deps,
            session,
            character_guid,
            map_id,
            destination,
            header_crypto,
        )
        .await;
    }

    let position = {
        let Some(character) = session.character.active_character.as_mut() else {
            return Ok(());
        };

        WorldPosition::new(
            destination.map_id,
            destination.x,
            destination.y,
            destination.z,
            if preserve_orientation {
                character.position.orientation
            } else {
                destination.orientation
            },
        )
    };

    apply_player_same_map_teleport_effect(
        stream,
        deps,
        session,
        character_guid,
        map_id,
        position,
        header_crypto,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_transport_door_effect(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    character_guid: u32,
    map_id: u32,
    spell_template: &wow_db::SpellTemplateQuery,
    effect: SpellInfoEffect,
    targets: &SpellCastTargets,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let gameobject_entry = effect.misc_value.max(0) as u32;
    if gameobject_entry == 0 {
        warn!(
            spell_id = spell_template.id,
            "Skipping transport door effect with no gameobject entry"
        );
        return Ok(());
    }

    let Some(template) =
        wow_db::get_gameobject_template_query(deps.world_db_pool, gameobject_entry).await?
    else {
        warn!(
            spell_id = spell_template.id,
            gameobject_entry, "Skipping transport door effect with missing gameobject template"
        );
        return Ok(());
    };
    let Some(duration) = deps
        .shared_world
        .maps
        .spell_duration(spell_template.duration_index)
    else {
        warn!(
            spell_id = spell_template.id,
            duration_index = spell_template.duration_index,
            "Skipping transport door effect with missing spell duration"
        );
        return Ok(());
    };
    if duration.duration_millis <= 0 {
        return Ok(());
    }
    let Some(position) = player_transport_door_spawn_position(
        deps.shared_world.maps,
        session,
        map_id,
        spell_template,
        effect,
        targets,
    ) else {
        warn!(
            spell_id = spell_template.id,
            "Skipping transport door effect with missing destination"
        );
        return Ok(());
    };

    let spawn = wow_db::GameObjectSpawnQuery {
        guid: 0,
        entry: template.entry,
        map: position.map_id,
        game_event: None,
        guid_pool_id: None,
        entry_pool_id: None,
        pool_max_limit: None,
        pool_chance: 0.0,
        position_x: position.x,
        position_y: position.y,
        position_z: position.z,
        orientation: position.orientation,
        rotation0: 0.0,
        rotation1: 0.0,
        rotation2: 0.0,
        rotation3: 1.0,
        spawn_time_secs_min: 0,
        spawn_time_secs_max: 0,
        state: -1,
        anim_progress: 100,
        template,
    };

    let Some((direct_packets, observer_packets)) = deps
        .shared_world
        .maps
        .create_temporary_gameobject(
            map_id,
            character_guid,
            spawn,
            caster,
            now + Duration::from_millis(duration.duration_millis as u64),
        )
        .await?
    else {
        return Ok(());
    };

    for packet in direct_packets {
        send_packet(
            stream,
            packet.opcode,
            &packet.body,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    deps.shared_world.sessions.dispatch(observer_packets).await;
    Ok(())
}

async fn player_near_teleport_database_destination(
    world_db_pool: &MySqlPool,
    spell_template: &wow_db::SpellTemplateQuery,
    effect: SpellInfoEffect,
) -> anyhow::Result<Option<WorldPosition>> {
    if !spell_effect_uses_database_target_position(effect) {
        return Ok(None);
    }

    let Some(destination) =
        wow_db::get_spell_target_position_query(world_db_pool, spell_template.id).await?
    else {
        return Ok(None);
    };

    Ok(Some(WorldPosition::new(
        destination.target_map,
        destination.target_position_x,
        destination.target_position_y,
        destination.target_position_z,
        destination.target_orientation,
    )))
}

fn spell_effect_uses_database_target_position(effect: SpellInfoEffect) -> bool {
    [effect.implicit_target_a, effect.implicit_target_b]
        .into_iter()
        .any(|target| target == TARGET_LOCATION_DATABASE)
}

fn player_transport_door_spawn_position(
    maps: &MapRuntimeManager,
    session: &WorldSessionState,
    map_id: u32,
    spell_template: &wow_db::SpellTemplateQuery,
    effect: SpellInfoEffect,
    targets: &SpellCastTargets,
) -> Option<WorldPosition> {
    if let Some(destination) = spell_target_destination_position(map_id, targets) {
        return Some(destination);
    }
    let character = session.character.active_character.as_ref()?;
    let distance = if effect.radius_index != 0 && spell_template.speed == 0.0 {
        maps.spell_radius(effect.radius_index)
            .map(|radius| radius.radius)
            .filter(|radius| radius.is_finite() && *radius > 0.0)
            .or_else(|| {
                maps.spell_range(spell_template.range_index)
                    .map(|range| range.max_range)
                    .filter(|range| range.is_finite() && *range > 0.0)
            })?
    } else {
        maps.spell_range(spell_template.range_index)
            .map(|range| range.max_range)
            .filter(|range| range.is_finite() && *range > 0.0)?
    };
    if distance <= 0.0 || !distance.is_finite() {
        return None;
    }
    Some(WorldPosition::new(
        map_id,
        character.position.x + character.position.orientation.cos() * distance,
        character.position.y + character.position.orientation.sin() * distance,
        character.position.z,
        character.position.orientation,
    ))
}

#[allow(clippy::too_many_arguments)]
async fn apply_player_same_map_teleport_effect(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    character_guid: u32,
    map_id: u32,
    position: WorldPosition,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_mut() else {
        return Ok(());
    };

    character.position = position;
    character.movement_flags = 0;
    character.fall_time = 0;

    let environment_packets = deps
        .shared_world
        .maps
        .set_player_position(map_id, character_guid, position)
        .await?;

    deps.shared_world
        .maps
        .reset_player_visibility_scan_positions(map_id, character_guid)
        .await;

    deps.shared_world
        .maps
        .sync_player_gameplay_state(map_id, character_guid, session)
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

#[allow(clippy::too_many_arguments)]
async fn apply_player_far_teleport_effect(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    character_guid: u32,
    old_map_id: u32,
    destination: WorldPosition,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(runtime_before_transfer) = deps
        .shared_world
        .maps
        .player_runtime(old_map_id, character_guid)
        .await
    else {
        warn!(
            character_guid,
            source_map = old_map_id,
            destination_map = destination.map_id,
            "Skipping far teleport because player runtime was missing"
        );
        return Ok(());
    };

    let Some(character) = session.character.active_character.as_mut() else {
        return Ok(());
    };
    character.position = destination;
    character.movement_flags = 0;
    character.client_time = 0;
    character.fall_time = 0;
    character.jump = JumpInfo::default();

    session.character.selected_target = None;
    session.combat.player_in_combat = false;
    mirror_session_player_auto_attack(session, None, None);
    clear_session_active_creature_combats(session);

    let environment_packets = deps
        .shared_world
        .maps
        .transfer_player(old_map_id, character_guid, destination)
        .await?;

    deps.shared_world
        .maps
        .reset_player_visibility_scan_positions(destination.map_id, character_guid)
        .await;

    deps.shared_world
        .maps
        .sync_player_gameplay_state(destination.map_id, character_guid, session)
        .await;

    wow_db::update_character_position(
        deps.character_db_pool,
        deps.account_id,
        character_guid,
        destination,
    )
    .await?;

    send_packet(
        stream,
        WorldOpcode::SmsgTransferPending as u16,
        &build_transfer_pending_body(destination.map_id),
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgNewWorld as u16,
        &build_new_world_body(destination),
        Some(&mut *header_crypto),
    )
    .await?;

    let Some(mut bootstrap_character) =
        load_worldport_character_entry(deps.character_db_pool, deps.account_id, character_guid)
            .await?
    else {
        deps.shared_world
            .sessions
            .dispatch(environment_packets)
            .await;
        return Ok(());
    };

    bootstrap_character.map = destination.map_id;
    bootstrap_character.position_x = destination.x;
    bootstrap_character.position_y = destination.y;
    bootstrap_character.position_z = destination.z;
    bootstrap_character.orientation = destination.orientation;
    bootstrap_character.health = session.character.player_health;
    bootstrap_character.power1 = session.character.player_mana;
    bootstrap_character.power2 = session.character.player_rage;
    bootstrap_character.power4 = session.character.player_energy;
    bootstrap_character.rest_bonus = session.rest.rest_bonus;
    bootstrap_character.player_bytes2 =
        player_bytes2_with_rest_bonus(bootstrap_character.player_bytes2, session.rest.rest_bonus);

    let inventory_container_slots =
        load_inventory_container_slots(deps.world_db_pool, &session.inventory.items).await?;
    let equipped_templates =
        load_equipped_item_templates(deps.world_db_pool, &session.inventory.items).await?;
    let ammo_template = load_selected_ammo_template(
        deps.world_db_pool,
        &session.inventory.items,
        session.character.player_ammo_id,
    )
    .await?;

    send_self_spawn_update(
        stream,
        SelfSpawnUpdate {
            character: &bootstrap_character,
            inventory: &session.inventory.items,
            inventory_container_slots: &inventory_container_slots,
            base_world_stats: &runtime_before_transfer.base_world_stats,
            world_stats: &runtime_before_transfer.effective_world_stats,
            skills: &session.character.character_skills,
            active_spells: &session.character.active_spells,
            quest_statuses: &session.quests.quest_statuses,
            equipped_templates: &equipped_templates,
            ammo_template: ammo_template.as_ref(),
            active_auras: &session.auras.active_auras,
            nearby_creatures: &[],
            nearby_gameobjects: &[],
            nearby_player_corpses: &[],
        },
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
    stream_newly_visible_db_gameobjects(
        stream,
        deps.shared_world.object_mgr,
        deps.world_db_pool,
        deps.shared_world.maps,
        session,
        header_crypto,
    )
    .await?;
    stream_nearby_player_corpses(
        stream,
        deps.character_db_pool,
        deps.shared_world.maps,
        session,
        header_crypto,
    )
    .await?;

    Ok(())
}

async fn load_worldport_character_entry(
    character_db_pool: &MySqlPool,
    account_id: u32,
    character_guid: u32,
) -> anyhow::Result<Option<CharacterEnumEntry>> {
    Ok(
        wow_db::get_character_enum_entries(character_db_pool, account_id)
            .await?
            .into_iter()
            .find(|character| character.guid == character_guid),
    )
}

fn build_transfer_pending_body(map_id: u32) -> Vec<u8> {
    map_id.to_le_bytes().to_vec()
}

fn build_new_world_body(destination: WorldPosition) -> Vec<u8> {
    let mut body = Vec::with_capacity(20);
    body.extend_from_slice(&destination.map_id.to_le_bytes());
    body.extend_from_slice(&destination.x.to_le_bytes());
    body.extend_from_slice(&destination.y.to_le_bytes());
    body.extend_from_slice(&destination.z.to_le_bytes());
    body.extend_from_slice(&destination.orientation.to_le_bytes());
    body
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

    if homebind.map_id != old_map_id {
        return apply_player_far_teleport_effect(
            stream,
            deps,
            session,
            character_guid,
            old_map_id,
            homebind,
            header_crypto,
        )
        .await;
    }

    apply_player_same_map_teleport_effect(
        stream,
        deps,
        session,
        character_guid,
        old_map_id,
        homebind,
        header_crypto,
    )
    .await
}
