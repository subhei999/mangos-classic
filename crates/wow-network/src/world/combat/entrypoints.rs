async fn handle_attack_swing(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    shared_world: SharedWorldDeps<'_>,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let target = read_packet_guid(body, "CMSG_ATTACKSWING")?;
    let Some(character_guid) = session.active_character.as_ref().map(|character| character.guid)
    else {
        warn!("Ignoring attack swing before character login");
        return Ok(());
    };
    if target == rust_combat_dummy_guid() {
        if session.combat_dummy_lootable || session.combat_dummy_health == 0 {
            warn!("Ignoring attack swing against dead combat dummy");
            return Ok(());
        }

        let next_swing = combat_dummy_next_swing_at(Instant::now());
        if let Some(character) = session.active_character.as_ref() {
            shared_world
                .maps
                .set_player_auto_attack(character.position.map_id, character.guid, Some(target), Some(next_swing))
                .await;
        }
        let attacker = ObjectGuid::new(HighGuid::Player, 0, character_guid);
        send_packet(
            stream,
            SMSG_ATTACKSTART,
            &build_attack_start_body(attacker, target),
            Some(&mut *header_crypto),
        )
        .await?;
        broadcast_player_attack_start(shared_world, session, attacker, target).await;
        return send_combat_dummy_swing(stream, shared_world, session, header_crypto).await;
    }

    let Some(character) = session.active_character.as_ref() else {
        return Ok(());
    };
    if shared_world
        .maps
        .db_creature_combat_snapshot(character.position.map_id, target)
        .await
        .is_none()
    {
        warn!(
            target = format_args!("0x{:016X}", target.raw()),
            "Ignoring attack swing against unknown target"
        );
        return Ok(());
    }

    let now = Instant::now();
    if let Some(character) = session.active_character.as_ref() {
        shared_world
            .maps
            .set_player_auto_attack(character.position.map_id, character.guid, Some(target), Some(now))
            .await;
    }
    let attacker = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    send_packet(
        stream,
        SMSG_ATTACKSTART,
        &build_attack_start_body(attacker, target),
        Some(&mut *header_crypto),
    )
    .await?;
    broadcast_player_attack_start(shared_world, session, attacker, target).await;
    send_db_creature_swing(
        stream,
        character_db_pool,
        world_db_pool,
        shared_world,
        session,
        header_crypto,
        target,
    )
    .await
}

async fn broadcast_player_attack_start(
    shared_world: SharedWorldDeps<'_>,
    session: &WorldSessionState,
    attacker: ObjectGuid,
    target: ObjectGuid,
) {
    let Some(character) = session.active_character.as_ref() else {
        return;
    };
    let packets = shared_world
        .maps
        .broadcast_nearby_player_packet(
            character.position.map_id,
            character.guid,
            PLAYER_VISIBILITY_RADIUS_YARDS,
            OutboundWorldPacket {
                opcode: SMSG_ATTACKSTART,
                body: build_attack_start_body(attacker, target),
            },
        )
        .await;
    shared_world.sessions.dispatch(packets).await;
}

