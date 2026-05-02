// CMaNGOS reference: src/game/Handlers/MovementHandler.cpp movement flow.

async fn persist_active_character_position(
    character_db_pool: &MySqlPool,
    account_id: u32,
    session: &WorldSessionState,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        return Ok(());
    };

    let rows = wow_db::update_character_position_and_vitals(
        character_db_pool,
        account_id,
        character.guid,
        character.position,
        session.player_health,
        session.player_mana,
        session.player_rage,
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
            x = character.position.x,
            y = character.position.y,
            z = character.position.z,
            o = character.position.orientation,
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
    let movement = MovementInfo::read(body)?;
    if let Some(character) = &mut session.active_character {
        character.position.x = movement.position.x;
        character.position.y = movement.position.y;
        character.position.z = movement.position.z;
        character.position.orientation = movement.position.orientation;
        character.movement_flags = movement.flags;
        character.client_time = movement.client_time;
        character.fall_time = movement.fall_time;
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
                )
                .await?;
            deps.sessions.dispatch(packets).await;
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
            deps.player_corpses,
            session,
            header_crypto,
        )
        .await?;
        try_start_db_creature_aggro(
            stream,
            SharedWorldDeps {
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
    Ok(())
}

struct MovementDeps<'a> {
    character_db_pool: &'a MySqlPool,
    world_db_pool: &'a MySqlPool,
    player_corpses: &'a PlayerCorpses,
    maps: &'a Arc<MapRuntimeManager>,
    sessions: &'a Arc<SessionRegistry>,
}

