use super::*;
use wow_proto::world::WorldOpcode;
// CMaNGOS reference: src/game/Handlers/MovementHandler.cpp movement flow.

pub(in crate::world) const PLAYER_POSITION_STATUS_UPDATE_INTERVAL: Duration =
    Duration::from_millis(100);

pub(in crate::world) fn current_movement_server_time_millis() -> u32 {
    static MOVEMENT_SERVER_TIME_START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    MOVEMENT_SERVER_TIME_START
        .get_or_init(Instant::now)
        .elapsed()
        .as_millis() as u32
}

pub(in crate::world) fn synchronize_movement_server_time(
    session: &mut WorldSessionState,
    client_time: u32,
) -> u32 {
    let delay = *session
        .movement
        .movement_client_time_delay
        .get_or_insert_with(|| current_movement_server_time_millis().wrapping_sub(client_time));
    client_time.wrapping_add(delay)
}

pub(in crate::world) async fn persist_active_character_position(
    character_db_pool: &MySqlPool,
    account_id: u32,
    maps: &Arc<MapRuntimeManager>,
    session: &WorldSessionState,
) -> anyhow::Result<()> {
    let Some(character) = &session.character.active_character else {
        return Ok(());
    };
    let snapshot = maps
        .player_runtime_session_snapshot(character.position.map_id, character.guid)
        .await;
    let position = snapshot
        .as_ref()
        .map(|snapshot| snapshot.position)
        .unwrap_or(character.position);
    let health = snapshot
        .as_ref()
        .map(|snapshot| snapshot.health)
        .unwrap_or(session.character.player_health);
    let power1 = snapshot
        .as_ref()
        .map(|snapshot| snapshot.power1)
        .unwrap_or(session.character.player_mana);
    let power2 = snapshot
        .as_ref()
        .map(|snapshot| snapshot.power2)
        .unwrap_or(session.character.player_rage);

    let rows = wow_db::update_character_position_and_vitals(
        character_db_pool,
        account_id,
        character.guid,
        position,
        health,
        power1,
        power2,
    )
    .await?;

    if rows == 0 {
        warn!(
            account_id,
            guid = character.guid,
            "No character row updated while persisting position"
        );
    } else {
        info!(
            account_id,
            guid = character.guid,
            name = %character.name,
            x = position.x,
            y = position.y,
            z = position.z,
            o = position.orientation,
            "Persisted character position"
        );
    }

    Ok(())
}

pub(in crate::world) async fn handle_movement(
    stream: &mut WorldPacketSink,
    deps: MovementDeps<'_>,
    opcode: u32,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let movement = MovementInfo::read(body)?;
    if session.logout.requested_at.is_some() {
        return Ok(());
    }
    let map_death_state = if let Some(character) = session.character.active_character.as_ref() {
        deps.maps
            .player_runtime_session_snapshot(character.position.map_id, character.guid)
            .await
            .map(|snapshot| snapshot.death_state)
    } else {
        None
    };
    let corpse_movement = matches!(
        session.death.player_death_state,
        PlayerDeathState::JustDied | PlayerDeathState::Corpse
    ) || matches!(
        map_death_state,
        Some(PlayerDeathState::JustDied | PlayerDeathState::Corpse)
    );
    if corpse_movement && !corpse_falling_movement_allowed(opcode, &movement) {
        return Ok(());
    }
    if !corpse_movement && movement_opcode_interrupts_spell_cast(opcode) {
        cancel_movement_interrupted_player_spell_cast(
            stream,
            deps.maps,
            deps.sessions,
            session,
            header_crypto,
        )
        .await?;
        clear_player_state_emote_on_movement(
            stream,
            deps.maps,
            deps.sessions,
            session,
            header_crypto,
        )
        .await?;
    }
    let server_time = synchronize_movement_server_time(session, movement.client_time);
    let mut map_owned_death_detected = false;
    if let Some(character) = &mut session.character.active_character {
        trace_named_movement(
            "dispatch_recv",
            character.guid,
            &character.name,
            opcode,
            &movement,
            &format!(
                "server_time={} corpse_movement={}",
                server_time, corpse_movement
            ),
        );
        let previous_player_health = session.character.player_health;
        character.position.x = movement.position.x;
        character.position.y = movement.position.y;
        character.position.z = movement.position.z;
        character.position.orientation = movement.position.orientation;
        character.movement_flags = movement.flags;
        character.client_time = movement.client_time;
        character.fall_time = tracked_session_fall_time(opcode, &movement);
        character.jump = if character.fall_time == 0 {
            JumpInfo::default()
        } else {
            movement.jump.clone()
        };
        debug!(
            opcode = movement_opcode_name(opcode),
            guid = character.guid,
            name = %character.name,
            flags = format_args!("0x{:08X}", movement.flags),
            client_time = movement.client_time,
            x = movement.position.x,
            y = movement.position.y,
            z = movement.position.z,
            o = movement.position.orientation,
            "Updated in-memory character movement"
        );
        if let Ok(server_opcode) = u16::try_from(opcode) {
            let mut broadcast_movement = movement.clone();
            broadcast_movement.position.map_id = character.position.map_id;
            let movement_outcome = deps
                .maps
                .update_player_position(
                    character.position.map_id,
                    character.guid,
                    server_opcode,
                    &broadcast_movement,
                    server_time,
                )
                .await?;
            let MovementUpdateOutcome::Applied { packets } = movement_outcome;
            deps.sessions.dispatch(packets).await;
            if let Some(snapshot) = deps
                .maps
                .player_runtime_session_snapshot(character.position.map_id, character.guid)
                .await
            {
                session.character.player_health = snapshot.health;
                session.character.player_mana = snapshot.power1;
                session.character.player_rage = snapshot.power2;
                session.character.player_energy = snapshot.power4;
                if !corpse_movement
                    && previous_player_health > 0
                    && session.character.player_health == 0
                    && session.death.player_death_state == PlayerDeathState::Alive
                {
                    map_owned_death_detected = true;
                }
            }
            if !corpse_movement {
                stream_newly_visible_db_creatures(
                    stream,
                    deps.character_db_pool,
                    deps.world_db_pool,
                    deps.maps,
                    session,
                    header_crypto,
                )
                .await?;
                stream_newly_visible_db_gameobjects(
                    stream,
                    deps.object_mgr,
                    deps.world_db_pool,
                    deps.maps,
                    session,
                    header_crypto,
                )
                .await?;
                stream_nearby_player_corpses(
                    stream,
                    deps.character_db_pool,
                    deps.maps,
                    session,
                    header_crypto,
                )
                .await?;
                interrupt_player_consumable_auras(
                    stream,
                    deps.maps,
                    deps.sessions,
                    session,
                    AURA_INTERRUPT_FLAG_MOVING,
                    header_crypto,
                )
                .await?;
            }
        }
    } else {
        warn!(
            opcode = movement_opcode_name(opcode),
            "Received movement packet before character login"
        );
    }
    if corpse_movement {
        crate::observability::record_world_position_status("skipped_corpse_movement");
    } else if player_position_status_update_due(session, Instant::now()) {
        crate::observability::record_world_position_status("attempted");
        handle_player_area_discovery(stream, &deps, session, header_crypto).await?;
    } else {
        crate::observability::record_world_position_status("skipped_not_due");
    }
    if map_owned_death_detected {
        if let Some(character) = session.character.active_character.as_ref() {
            refresh_session_from_map_owned_player_death(
                deps.maps,
                character.position.map_id,
                session,
            )
            .await;
        }
    }
    Ok(())
}

pub(in crate::world) fn player_position_status_update_due(
    session: &mut WorldSessionState,
    now: Instant,
) -> bool {
    if session
        .movement
        .next_position_status_update_at
        .is_some_and(|due_at| now < due_at)
    {
        return false;
    }
    session.movement.next_position_status_update_at =
        Some(now + PLAYER_POSITION_STATUS_UPDATE_INTERVAL);
    true
}

async fn handle_player_area_discovery(
    stream: &mut WorldPacketSink,
    deps: &MovementDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        crate::observability::record_world_position_status("skipped_no_active_character");
        return Ok(());
    };
    let character_guid = character.guid;
    let map_id = character.position.map_id;
    let position = character.position;
    let player_level = character.level;
    let Some((area_flag, area_entry)) = deps
        .maps
        .geometry
        .area_entry_with_source(position, "movement_position_status")
    else {
        crate::observability::record_world_position_status("area_not_found");
        return Ok(());
    };
    let Some(discovery) = deps
        .maps
        .discover_player_area(map_id, character_guid, area_flag)
        .await?
    else {
        crate::observability::record_world_position_status("area_already_discovered");
        return Ok(());
    };
    crate::observability::record_world_position_status("area_discovered");

    let explored_zones = format_explored_zones(&discovery.explored_zones);
    let rows = wow_db::update_character_explored_zones(
        deps.character_db_pool,
        character_guid,
        &explored_zones,
    )
    .await?;
    if rows == 0 {
        warn!(
            guid = character_guid,
            area_flag = discovery.area_flag,
            "No character row updated while persisting explored zones"
        );
    }
    crate::observability::record_world_outbound_source_packet(
        "area_discovery",
        WorldOpcode::SmsgUpdateObject as u16,
        discovery.update_body.len() + 4,
    );
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &discovery.update_body,
        Some(&mut *header_crypto),
    )
    .await?;

    if area_entry.area_level == 0 {
        return Ok(());
    }
    let xp = exploration_xp_for_area_level(
        deps.object_mgr,
        deps.world_db_pool,
        player_level,
        area_entry.area_level,
    )
    .await?;
    award_character_xp(
        stream,
        deps.character_db_pool,
        deps.world_db_pool,
        deps.maps,
        session,
        None,
        xp,
        header_crypto,
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgExplorationExperience as u16,
        &build_exploration_experience_body(area_entry.id, xp),
        Some(header_crypto),
    )
    .await?;

    Ok(())
}

pub(in crate::world) async fn exploration_xp_for_area_level(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    player_level: u8,
    area_level: u8,
) -> anyhow::Result<u32> {
    if area_level == 0 || player_level >= DEFAULT_MAX_PLAYER_LEVEL {
        return Ok(0);
    }
    let diff = i32::from(player_level) - i32::from(area_level);
    let (base_level, percent) = if diff < -5 {
        (player_level.saturating_add(5), 100u32)
    } else if diff > 5 {
        let percent = 100i32.saturating_sub(diff.saturating_sub(5).saturating_mul(5));
        (area_level, percent.clamp(0, 100) as u32)
    } else {
        (area_level, 100u32)
    };
    let base_xp = object_mgr
        .exploration_base_xp(world_db_pool, base_level)
        .await?;
    Ok(base_xp.saturating_mul(percent) / 100)
}

pub(in crate::world) async fn clear_player_state_emote_on_movement(
    stream: &mut WorldPacketSink,
    maps: &Arc<MapRuntimeManager>,
    sessions: &Arc<SessionRegistry>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if session.character.player_emote_state == 0 {
        return Ok(());
    }
    let Some(character) = session.character.active_character.clone() else {
        session.character.player_emote_state = 0;
        return Ok(());
    };
    session.character.player_emote_state = 0;
    let body = build_emote_state_update_body(&character, 0)?;
    crate::observability::record_world_outbound_source_packet(
        "movement_clear_emote_direct",
        WorldOpcode::SmsgUpdateObject as u16,
        body.len() + 4,
    );
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &body,
        Some(header_crypto),
    )
    .await?;
    let packets = maps
        .broadcast_nearby_player_packet(
            character.position.map_id,
            character.guid,
            CHAT_EMOTE_RADIUS_YARDS,
            OutboundWorldPacket {
                opcode: WorldOpcode::SmsgUpdateObject as u16,
                body,
            },
        )
        .await;
    sessions.dispatch(packets).await;
    Ok(())
}

pub(in crate::world) fn corpse_falling_movement_allowed(
    opcode: u32,
    movement: &MovementInfo,
) -> bool {
    if opcode == WorldOpcode::MsgMoveFallLand as u32 {
        return true;
    }

    if movement.flags & MOVEFLAG_JUMPING != 0 {
        return true;
    }

    matches!(
        WorldOpcode::try_from(opcode).ok(),
        Some(
            WorldOpcode::MsgMoveFallLand
                | WorldOpcode::MsgMoveStartSwim
                | WorldOpcode::MsgMoveHeartbeat
        )
    ) && movement.fall_time > 0
}

pub(in crate::world) fn tracked_session_fall_time(opcode: u32, movement: &MovementInfo) -> u32 {
    if opcode == WorldOpcode::MsgMoveFallLand as u32 || movement.flags & MOVEFLAG_JUMPING == 0 {
        0
    } else {
        movement.fall_time
    }
}

pub(in crate::world) fn movement_opcode_interrupts_spell_cast(opcode: u32) -> bool {
    matches!(
        WorldOpcode::try_from(opcode).ok(),
        Some(
            WorldOpcode::MsgMoveStartForward
                | WorldOpcode::MsgMoveStartBackward
                | WorldOpcode::MsgMoveStartStrafeLeft
                | WorldOpcode::MsgMoveStartStrafeRight
                | WorldOpcode::MsgMoveJump
                | WorldOpcode::MsgMoveStartSwim
        )
    )
}

pub(in crate::world) struct MovementDeps<'a> {
    pub(in crate::world) character_db_pool: &'a MySqlPool,
    pub(in crate::world) world_db_pool: &'a MySqlPool,
    pub(in crate::world) object_mgr: &'a ObjectMgr,
    pub(in crate::world) maps: &'a Arc<MapRuntimeManager>,
    pub(in crate::world) sessions: &'a Arc<SessionRegistry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_position_status_update_is_throttled() {
        let now = Instant::now();
        let mut session = WorldSessionState::default();

        assert!(player_position_status_update_due(&mut session, now));
        assert!(!player_position_status_update_due(
            &mut session,
            now + Duration::from_millis(99)
        ));
        assert!(player_position_status_update_due(
            &mut session,
            now + Duration::from_millis(100)
        ));
    }
}
