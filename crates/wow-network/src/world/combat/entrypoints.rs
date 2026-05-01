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
    sync_session_db_creatures_from_map(shared_world, session).await;

    if target == rust_combat_dummy_guid() {
        if session.combat_dummy_lootable || session.combat_dummy_health == 0 {
            warn!("Ignoring attack swing against dead combat dummy");
            return Ok(());
        }

        session.active_combat_target = Some(target);
        session.active_combat_next_swing_at = Some(combat_dummy_next_swing_at(Instant::now()));
        let attacker = ObjectGuid::new(HighGuid::Player, 0, character_guid);
        send_packet(
            stream,
            SMSG_ATTACKSTART,
            &build_attack_start_body(attacker, target),
            Some(&mut *header_crypto),
        )
        .await?;
        broadcast_player_attack_start(shared_world, session, attacker, target).await;
        return send_combat_dummy_swing(stream, session, header_crypto).await;
    }

    if !session
        .db_creatures
        .get(&target.raw())
        .is_some_and(DbCreatureRuntime::is_alive)
    {
        warn!(
            target = format_args!("0x{:016X}", target.raw()),
            "Ignoring attack swing against unknown target"
        );
        return Ok(());
    }

    session.active_combat_target = Some(target);
    let now = Instant::now();
    session.active_combat_next_swing_at = Some(now);
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

