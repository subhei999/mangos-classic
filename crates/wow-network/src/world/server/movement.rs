// CMaNGOS reference: src/game/Handlers/MovementHandler.cpp movement flow.

fn current_movement_server_time_millis() -> u32 {
    static MOVEMENT_SERVER_TIME_START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    MOVEMENT_SERVER_TIME_START
        .get_or_init(Instant::now)
        .elapsed()
        .as_millis() as u32
}

fn synchronize_movement_server_time(
    session: &mut WorldSessionState,
    client_time: u32,
) -> u32 {
    let delay = *session
        .movement_client_time_delay
        .get_or_insert_with(|| current_movement_server_time_millis().wrapping_sub(client_time));
    client_time.wrapping_add(delay)
}

async fn persist_active_character_position(
    character_db_pool: &MySqlPool,
    account_id: u32,
    maps: &Arc<MapRuntimeManager>,
    session: &WorldSessionState,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        return Ok(());
    };
    let snapshot = maps
        .player_runtime_snapshot(character.position.map_id, character.guid)
        .await;
    let position = snapshot
        .as_ref()
        .map(|snapshot| snapshot.position)
        .unwrap_or(character.position);
    let health = snapshot
        .as_ref()
        .map(|snapshot| snapshot.health)
        .unwrap_or(session.player_health);
    let power1 = snapshot
        .as_ref()
        .map(|snapshot| snapshot.power1)
        .unwrap_or(session.player_mana);
    let power2 = snapshot
        .as_ref()
        .map(|snapshot| snapshot.power2)
        .unwrap_or(session.player_rage);

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

async fn handle_logout_cancel(
    stream: &mut WorldPacketSink,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    send_packet(stream, SMSG_LOGOUT_CANCEL_ACK, &[], Some(header_crypto)).await
}

async fn handle_movement(
    stream: &mut WorldPacketSink,
    deps: MovementDeps<'_>,
    opcode: u32,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if session.player_death_state == PlayerDeathState::Corpse {
        return Ok(());
    }
    let movement = MovementInfo::read(body)?;
    if movement_opcode_interrupts_spell_cast(opcode) {
        cancel_pending_player_spell_cast(
            stream,
            deps.maps,
            deps.sessions,
            session,
            SPELL_FAILED_INTERRUPTED,
            header_crypto,
        )
        .await?;
    }
    let server_time = synchronize_movement_server_time(session, movement.client_time);
    let mut fatal_environmental_damage_player = None;
    if let Some(character) = &mut session.active_character {
        let previous_player_health = session.player_health;
        character.position.x = movement.position.x;
        character.position.y = movement.position.y;
        character.position.z = movement.position.z;
        character.position.orientation = movement.position.orientation;
        character.movement_flags = movement.flags;
        character.client_time = movement.client_time;
        character.fall_time = movement.fall_time;
        character.jump = movement.jump.clone();
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
            let packets = deps
                .maps
                .update_player_position(
                    character.position.map_id,
                    character.guid,
                    server_opcode,
                    &broadcast_movement,
                    server_time,
                )
                .await?;
            deps.sessions.dispatch(packets).await;
            if let Some(snapshot) = deps
                .maps
                .player_runtime_snapshot(character.position.map_id, character.guid)
                .await
            {
                session.player_health = snapshot.health;
                session.player_mana = snapshot.power1;
                session.player_rage = snapshot.power2;
                session.player_energy = snapshot.power4;
                if previous_player_health > 0
                    && session.player_health == 0
                    && session.player_death_state == PlayerDeathState::Alive
                {
                    fatal_environmental_damage_player =
                        Some(ObjectGuid::new(HighGuid::Player, 0, character.guid));
                }
            }
        }
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
        try_start_db_creature_aggro(
            stream,
            SharedWorldDeps {
                object_mgr: deps.object_mgr,
                maps: deps.maps,
                sessions: deps.sessions,
            },
            session,
            header_crypto,
        )
        .await?;
    } else {
        warn!(
            opcode = movement_opcode_name(opcode),
            "Received movement packet before character login"
        );
    }
    if let Some(player) = fatal_environmental_damage_player {
        kill_player_from_creature(
            stream,
            deps.character_db_pool,
            deps.maps,
            deps.account_id,
            session,
            player,
            header_crypto,
        )
        .await?;
    }
    Ok(())
}

fn movement_opcode_interrupts_spell_cast(opcode: u32) -> bool {
    matches!(
        opcode,
        MSG_MOVE_START_FORWARD
            | MSG_MOVE_START_BACKWARD
            | MSG_MOVE_START_STRAFE_LEFT
            | MSG_MOVE_START_STRAFE_RIGHT
            | MSG_MOVE_JUMP
            | MSG_MOVE_START_SWIM
    )
}

struct MovementDeps<'a> {
    account_id: u32,
    character_db_pool: &'a MySqlPool,
    world_db_pool: &'a MySqlPool,
    object_mgr: &'a ObjectMgr,
    maps: &'a Arc<MapRuntimeManager>,
    sessions: &'a Arc<SessionRegistry>,
}

