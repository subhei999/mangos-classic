use super::*;
use wow_proto::world::WorldOpcode;

pub(in crate::world) async fn dispatch_death_packet(
    ctx: &mut WorldPacketDispatchContext<'_>,
    packet: &packets::ParsedWorldClientPacket,
) -> anyhow::Result<()> {
    match packet {
        packets::ParsedWorldClientPacket::Repop(_) => {
            let _ = packet.repop()?;
            handle_repop_request(
                &mut *ctx.stream,
                PlayerDeathDeps {
                    character_db_pool: ctx.character_db_pool,
                    world_db_pool: ctx.world_db_pool,
                    maps: &ctx.runtime_state.maps,
                    sessions: &ctx.runtime_state.sessions,
                    account_id: ctx.account_id,
                },
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::ReclaimCorpse(_) => {
            handle_reclaim_corpse(
                &mut *ctx.stream,
                PlayerDeathDeps {
                    character_db_pool: ctx.character_db_pool,
                    world_db_pool: ctx.world_db_pool,
                    maps: &ctx.runtime_state.maps,
                    sessions: &ctx.runtime_state.sessions,
                    account_id: ctx.account_id,
                },
                packet.reclaim_corpse()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::SpiritHealerActivate(_) => {
            handle_spirit_healer_activate(
                &mut *ctx.stream,
                PlayerDeathDeps {
                    character_db_pool: ctx.character_db_pool,
                    world_db_pool: ctx.world_db_pool,
                    maps: &ctx.runtime_state.maps,
                    sessions: &ctx.runtime_state.sessions,
                    account_id: ctx.account_id,
                },
                packet.spirit_healer_activate()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::CorpseQuery(_) => {
            let _ = packet.corpse_query()?;
            handle_corpse_query(&mut *ctx.stream, &*ctx.session, &mut *ctx.header_crypto).await
        }
        other => anyhow::bail!("death router received opcode 0x{:04X}", other.opcode()),
    }
}

#[derive(Clone, Copy)]
pub(in crate::world) struct PlayerDeathDeps<'a> {
    pub(in crate::world) character_db_pool: &'a MySqlPool,
    pub(in crate::world) world_db_pool: &'a MySqlPool,
    pub(in crate::world) maps: &'a Arc<MapRuntimeManager>,
    pub(in crate::world) sessions: &'a Arc<SessionRegistry>,
    pub(in crate::world) account_id: u32,
}

pub(in crate::world) async fn handle_repop_request(
    stream: &mut WorldPacketSink,
    deps: PlayerDeathDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let presentation_packets = refresh_session_death_state_before_repop(deps.maps, session).await?;
    deps.sessions.dispatch(presentation_packets).await;
    if session.death.player_death_state != PlayerDeathState::Corpse {
        warn!(
            player_death_state = ?session.death.player_death_state,
            pending = session.death.player_death_presentation_pending,
            health = session.character.player_health,
            "Ignoring Release Spirit request before player is in corpse state"
        );
        return Ok(());
    }
    if session.character.active_character.is_none() {
        return Ok(());
    }

    let corpse = create_or_get_player_corpse(deps.character_db_pool, deps.maps, session).await?;
    let corpse_position = corpse.position;
    let graveyard_position =
        select_repop_graveyard_position(deps.world_db_pool, corpse_position).await?;

    let (character_guid, character_race, character_class) = session
        .character
        .active_character
        .as_ref()
        .map(|character| (character.guid, character.race, character.class))
        .unwrap_or_default();
    let old_map_id = session
        .character
        .active_character
        .as_ref()
        .map(|character| character.position.map_id)
        .unwrap_or(corpse_position.map_id);
    session.death.player_death_state = PlayerDeathState::Ghost;
    session.death.player_death_presentation_pending = false;
    session.character.player_health = PLAYER_SURVIVOR_HEALTH_FLOOR;
    session.character.player_rage = 0;
    session.character.player_flags |= PLAYER_FLAGS_GHOST;
    session.character.player_stand_state = PLAYER_STAND_STATE_STAND;
    session.combat.player_in_combat = false;
    mirror_session_player_auto_attack(session, None, None);
    clear_session_active_creature_combats(session);
    deps.maps
        .set_player_auto_attack(old_map_id, character_guid, None, None)
        .await;
    deps.maps
        .set_player_power2(old_map_id, character_guid, 0)
        .await;
    if let Some(character) = &mut session.character.active_character {
        character.position = graveyard_position;
        character.movement_flags = 0;
        character.fall_time = 0;
    }
    deps.maps
        .sync_player_gameplay_state(old_map_id, character_guid, session)
        .await;
    deps.maps
        .reset_player_visibility_scan_positions(old_map_id, character_guid)
        .await;

    let player = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &build_player_death_update_body(PlayerDeathUpdate {
            player,
            health: session.character.player_health,
            player_flags: session.character.player_flags,
            field_bytes: 0,
            unit_flags: player_unit_flags(false),
            race: character_race,
            class: character_class,
            stand_state: PLAYER_STAND_STATE_STAND,
        })?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgForceMoveUnroot as u16,
        &build_force_move_unroot_body(player, 0)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &build_update_object_body(&[build_player_corpse_create_block(&corpse)?]),
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgCorpseReclaimDelay as u16,
        &build_corpse_reclaim_delay_body(CORPSE_RECLAIM_DELAY_MILLIS),
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::MsgMoveTeleportAck as u16,
        &build_near_teleport_ack_body(session.character.active_character.as_ref().unwrap(), 0)?,
        Some(&mut *header_crypto),
    )
    .await?;
    stream_newly_visible_db_creatures(
        stream,
        deps.character_db_pool,
        deps.world_db_pool,
        deps.maps,
        session,
        header_crypto,
    )
    .await?;
    persist_player_death_state(deps.character_db_pool, deps.account_id, session).await
}

pub(in crate::world) async fn refresh_session_death_state_before_repop(
    maps: &Arc<MapRuntimeManager>,
    session: &mut WorldSessionState,
) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(Vec::new());
    };
    let map_id = character.position.map_id;
    let character_guid = character.guid;
    let Some(mut snapshot) = maps.player_runtime_snapshot(map_id, character_guid).await else {
        return Ok(Vec::new());
    };
    let mut presentation_packets = Vec::new();
    if snapshot.health == 0 && snapshot.death_state == PlayerDeathState::JustDied {
        presentation_packets = maps
            .force_player_death_presentation(map_id, character_guid, Instant::now())
            .await?;
        if let Some(updated) = maps.player_runtime_snapshot(map_id, character_guid).await {
            snapshot = updated;
        }
    }
    if snapshot.health == 0 && snapshot.death_state != PlayerDeathState::Alive {
        refresh_session_from_map_owned_player_death(maps, map_id, session).await;
    }
    Ok(presentation_packets)
}

pub(in crate::world) async fn select_repop_graveyard_position(
    world_db_pool: &MySqlPool,
    corpse_position: WorldPosition,
) -> anyhow::Result<WorldPosition> {
    let linked_graveyard = wow_db::get_closest_graveyard(
        world_db_pool,
        corpse_position.map_id,
        corpse_position.x,
        corpse_position.y,
        corpse_position.z,
        ALLIANCE_FACTION,
    )
    .await?;
    let spirit_healer = wow_db::get_closest_spirit_healer(
        world_db_pool,
        corpse_position.map_id,
        corpse_position.x,
        corpse_position.y,
        corpse_position.z,
    )
    .await?;

    let linked_position = linked_graveyard.as_ref().map(graveyard_query_position);
    let spirit_position = spirit_healer.as_ref().map(graveyard_query_position);
    if let Some(spirit_position) = spirit_position {
        let spirit_distance = distance_2d(
            corpse_position.x,
            corpse_position.y,
            spirit_position.x,
            spirit_position.y,
        );
        let linked_distance = linked_position
            .map(|position| {
                distance_2d(corpse_position.x, corpse_position.y, position.x, position.y)
            })
            .unwrap_or(f32::MAX);
        if spirit_distance <= GRAVEYARD_SPIRIT_HEALER_FALLBACK_RADIUS_YARDS
            && spirit_distance < linked_distance
        {
            return Ok(spirit_position);
        }
    }

    Ok(linked_position.unwrap_or(corpse_position))
}

pub(in crate::world) fn graveyard_query_position(
    graveyard: &wow_db::GraveyardQuery,
) -> WorldPosition {
    WorldPosition::new(
        graveyard.map,
        graveyard.x,
        graveyard.y,
        graveyard.z,
        graveyard.o,
    )
}

pub(in crate::world) async fn handle_corpse_query(
    stream: &mut WorldPacketSink,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let corpse_position = (session.death.player_death_state == PlayerDeathState::Ghost)
        .then_some(
            session
                .death
                .player_corpse
                .as_ref()
                .map(|corpse| corpse.position),
        )
        .flatten();
    send_packet(
        stream,
        WorldOpcode::MsgCorpseQuery as u16,
        &build_corpse_query_body(corpse_position),
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn handle_reclaim_corpse(
    stream: &mut WorldPacketSink,
    deps: PlayerDeathDeps<'_>,
    request: wow_proto::ReclaimCorpseRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if session.death.player_death_state != PlayerDeathState::Ghost {
        return Ok(());
    }
    let _requested_corpse = request.requested_corpse_raw_guid.map(ObjectGuid::from_raw);
    let Some(character) = &mut session.character.active_character else {
        return Ok(());
    };
    let Some(corpse) = session.death.player_corpse.as_ref() else {
        return Ok(());
    };
    let corpse_position = corpse.position;
    let ghost_position = character.position;
    if !can_reclaim_corpse_at_ghost_position(ghost_position, corpse_position) {
        return Ok(());
    }

    resurrect_player_at_position(stream, deps, session, header_crypto, ghost_position).await
}

pub(in crate::world) fn can_reclaim_corpse_at_ghost_position(
    ghost_position: WorldPosition,
    corpse_position: WorldPosition,
) -> bool {
    if ghost_position.map_id != corpse_position.map_id {
        return false;
    }
    distance_2d(
        ghost_position.x,
        ghost_position.y,
        corpse_position.x,
        corpse_position.y,
    ) <= CORPSE_RECLAIM_RADIUS_YARDS
}

pub(in crate::world) async fn handle_spirit_healer_activate(
    stream: &mut WorldPacketSink,
    deps: PlayerDeathDeps<'_>,
    request: wow_proto::SpiritHealerActivateRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if session.death.player_death_state != PlayerDeathState::Ghost {
        return Ok(());
    }
    let healer_guid = ObjectGuid::from_raw(request.raw_guid);
    if !healer_guid.is_creature() {
        return Ok(());
    }
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let character_position = character.position;
    let Some(healer) = deps
        .maps
        .db_creature_snapshot(character_position.map_id, healer_guid)
        .await
    else {
        warn!(
            guid = format_args!("0x{:016X}", healer_guid.raw()),
            "Ignoring spirit healer activation for unloaded creature"
        );
        return Ok(());
    };
    if !is_spirit_healer_creature(&healer) {
        warn!(
            guid = format_args!("0x{:016X}", healer_guid.raw()),
            entry = healer.spawn.entry,
            "Ignoring spirit healer activation for non-healer creature"
        );
        return Ok(());
    }
    if character_position.map_id != healer.current_position.map_id {
        return Ok(());
    }
    if distance_2d(
        character_position.x,
        character_position.y,
        healer.current_position.x,
        healer.current_position.y,
    ) > SPIRIT_HEALER_INTERACTION_RADIUS_YARDS
    {
        return Ok(());
    }

    resurrect_player_at_position(stream, deps, session, header_crypto, character_position).await
}

pub(in crate::world) fn is_spirit_healer_creature(creature: &DbCreatureRuntime) -> bool {
    creature.spawn.template.npc_flags & UNIT_NPC_FLAG_SPIRITHEALER != 0
}

pub(in crate::world) async fn resurrect_player_at_position(
    stream: &mut WorldPacketSink,
    deps: PlayerDeathDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
    position: WorldPosition,
) -> anyhow::Result<()> {
    let Some(character_snapshot) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let (race, class, level) = (
        character_snapshot.race,
        character_snapshot.class,
        character_snapshot.level,
    );
    let world_stats =
        wow_db::get_player_world_stats(deps.world_db_pool, race, class, level).await?;
    let resurrected_health = (world_stats.max_health().max(1) / 2).max(1);
    session.death.player_death_state = PlayerDeathState::Alive;
    session.death.player_death_presentation_pending = false;
    let corpse_to_bones = session.death.player_corpse.take();
    session.character.player_health = resurrected_health;
    session.character.player_rage = 0;
    session.character.player_flags &= !PLAYER_FLAGS_GHOST;
    session.character.player_stand_state = PLAYER_STAND_STATE_STAND;
    let (character_guid, map_id) = {
        let Some(character) = &mut session.character.active_character else {
            return Ok(());
        };
        character.position = position;
        character.movement_flags = 0;
        character.fall_time = 0;
        (character.guid, character.position.map_id)
    };
    let player = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let packets = deps
        .maps
        .update_player_health(map_id, character_guid, session.character.player_health)
        .await?;
    deps.sessions.dispatch(packets).await;
    deps.maps.set_player_power2(map_id, character_guid, 0).await;
    deps.maps
        .sync_player_gameplay_state(map_id, character_guid, session)
        .await;
    deps.maps
        .reset_player_visibility_scan_positions(map_id, character_guid)
        .await;
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &build_player_death_update_body(PlayerDeathUpdate {
            player,
            health: session.character.player_health,
            player_flags: session.character.player_flags,
            field_bytes: 0,
            unit_flags: player_unit_flags(false),
            race,
            class,
            stand_state: PLAYER_STAND_STATE_STAND,
        })?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgForceMoveUnroot as u16,
        &build_force_move_unroot_body(player, 0)?,
        Some(&mut *header_crypto),
    )
    .await?;
    if let Some(corpse) = corpse_to_bones {
        wow_db::delete_player_corpse(deps.character_db_pool, character_guid).await?;
        let bones = player_bones_runtime_from_corpse(corpse);
        deps.maps
            .upsert_player_corpse(bones.position.map_id, bones.clone())
            .await;
        send_packet(
            stream,
            WorldOpcode::SmsgUpdateObject as u16,
            &build_player_corpse_bones_update_body(&bones)?,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    send_packet(
        stream,
        WorldOpcode::MsgMoveTeleportAck as u16,
        &build_near_teleport_ack_body(session.character.active_character.as_ref().unwrap(), 0)?,
        Some(&mut *header_crypto),
    )
    .await?;
    stream_newly_visible_db_creatures(
        stream,
        deps.character_db_pool,
        deps.world_db_pool,
        deps.maps,
        session,
        header_crypto,
    )
    .await?;
    persist_player_death_state(deps.character_db_pool, deps.account_id, session).await
}

pub(in crate::world) async fn create_or_get_player_corpse(
    character_db_pool: &MySqlPool,
    maps: &Arc<MapRuntimeManager>,
    session: &mut WorldSessionState,
) -> anyhow::Result<PlayerCorpseRuntime> {
    if let Some(corpse) = &session.death.player_corpse {
        maps.upsert_player_corpse(corpse.position.map_id, corpse.clone())
            .await;
        return Ok(corpse.clone());
    }
    let Some(character) = session.character.active_character.as_ref() else {
        anyhow::bail!("cannot create player corpse without an active character");
    };
    let corpse = player_corpse_runtime_from_active_character(character, session);
    wow_db::save_player_corpse(
        character_db_pool,
        &NewPlayerCorpse {
            guid: corpse.guid.counter(),
            player: character.guid,
            position: corpse.position,
            time: current_unix_epoch_secs_u64(),
            corpse_type: PLAYER_CORPSE_TYPE_RESURRECTABLE_PVE,
            instance: 0,
        },
    )
    .await?;
    maps.upsert_player_corpse(corpse.position.map_id, corpse.clone())
        .await;
    session.death.player_corpse = Some(corpse.clone());
    Ok(corpse)
}

pub(in crate::world) fn player_corpse_runtime_from_active_character(
    character: &ActiveCharacter,
    session: &WorldSessionState,
) -> PlayerCorpseRuntime {
    let visual = session.character.player_visual.as_ref();
    PlayerCorpseRuntime {
        guid: ObjectGuid::new(HighGuid::Corpse, 0, character.guid),
        owner: ObjectGuid::new(HighGuid::Player, 0, character.guid),
        position: character.position,
        corpse_type: PLAYER_CORPSE_TYPE_RESURRECTABLE_PVE,
        race: character.race,
        class: character.class,
        gender: visual.map(|visual| visual.gender).unwrap_or(0),
        player_bytes: visual.map(|visual| visual.player_bytes).unwrap_or_default(),
        player_bytes2: visual
            .map(|visual| visual.player_bytes2)
            .unwrap_or_default(),
        equipment_cache: Some(equipment_cache_for_corpse(
            visual.and_then(|visual| visual.equipment_cache.as_deref()),
            &session.inventory.items,
        )),
        guildid: visual.and_then(|visual| visual.guildid),
        player_flags: session.character.player_flags,
    }
}

pub(in crate::world) fn player_bones_runtime_from_corpse(
    mut corpse: PlayerCorpseRuntime,
) -> PlayerCorpseRuntime {
    corpse.corpse_type = PLAYER_CORPSE_TYPE_BONES;
    corpse.player_flags &= !(PLAYER_FLAGS_HIDE_HELM | PLAYER_FLAGS_HIDE_CLOAK);
    corpse
}

pub(in crate::world) fn player_corpse_runtime_from_query(
    corpse: PlayerCorpseQuery,
) -> PlayerCorpseRuntime {
    PlayerCorpseRuntime {
        guid: ObjectGuid::new(HighGuid::Corpse, 0, corpse.guid),
        owner: ObjectGuid::new(HighGuid::Player, 0, corpse.player),
        position: WorldPosition::new(
            corpse.map,
            corpse.position_x,
            corpse.position_y,
            corpse.position_z,
            corpse.orientation,
        ),
        corpse_type: corpse.corpse_type,
        race: corpse.race,
        class: corpse.class,
        gender: corpse.gender,
        player_bytes: corpse.player_bytes,
        player_bytes2: corpse.player_bytes2,
        equipment_cache: corpse.equipment_cache,
        guildid: corpse.guildid,
        player_flags: corpse.player_flags,
    }
}

pub(in crate::world) fn equipment_cache_for_corpse(
    equipment_cache: Option<&str>,
    inventory: &[CharacterInventoryItem],
) -> String {
    let mut equipment = parse_equipment_cache(equipment_cache);
    for item in inventory {
        if item.bag == INVENTORY_SLOT_BAG_0 as u32 && item.slot < EQUIPMENT_SLOT_END {
            equipment[item.slot as usize] = item.item_template;
        }
    }

    equipment
        .iter()
        .map(|item| format!("{item} 0"))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(in crate::world) async fn persist_player_death_state(
    character_db_pool: &MySqlPool,
    account_id: u32,
    session: &WorldSessionState,
) -> anyhow::Result<()> {
    let Some(character) = &session.character.active_character else {
        return Ok(());
    };
    let rows = wow_db::update_character_death_state(
        character_db_pool,
        account_id,
        character.guid,
        character.position,
        session.character.player_health,
        session.character.player_flags,
    )
    .await?;
    if rows == 0 {
        warn!(
            guid = character.guid,
            "No character row updated while persisting player death state"
        );
    }
    Ok(())
}
