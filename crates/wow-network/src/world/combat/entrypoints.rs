async fn handle_attack_swing(
    stream: &mut WorldPacketSink,
    _character_db_pool: &MySqlPool,
    _world_db_pool: &MySqlPool,
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

        let now = Instant::now();
        let next_swing = scheduled_player_auto_attack_next_swing(
            shared_world,
            session,
            target,
            now,
            Duration::from_millis(BASE_ATTACK_TIME_MS as u64),
        )
        .await;
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
        return Ok(());
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
        let next_swing = scheduled_player_auto_attack_next_swing(
            shared_world,
            session,
            target,
            now,
            player_auto_attack_swing_delay(shared_world, character.position.map_id, character.guid).await,
        )
        .await;
        shared_world
            .maps
            .set_player_auto_attack(
                character.position.map_id,
                character.guid,
                Some(target),
                Some(next_swing),
            )
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
    Ok(())
}

async fn player_auto_attack_swing_delay(
    shared_world: SharedWorldDeps<'_>,
    map_id: u32,
    character_guid: u32,
) -> Duration {
    let main_hand_ms = shared_world
        .maps
        .player_combat_stats(map_id, character_guid)
        .await
        .map(|stats| stats.main_attack_time_ms)
        .unwrap_or(BASE_ATTACK_TIME_MS);
    Duration::from_millis(main_hand_ms.max(1) as u64)
}

async fn scheduled_player_auto_attack_next_swing(
    shared_world: SharedWorldDeps<'_>,
    session: &WorldSessionState,
    target: ObjectGuid,
    now: Instant,
    swing_delay: Duration,
) -> Instant {
    let Some(character) = session.active_character.as_ref() else {
        return now + swing_delay;
    };
    if let Some(snapshot) = shared_world
        .maps
        .player_runtime_snapshot(character.position.map_id, character.guid)
        .await
    {
        if snapshot.active_combat_target == Some(target) {
            if let Some(next_swing) = snapshot.active_combat_next_swing_at {
                if next_swing > now {
                    return next_swing;
                }
            }
        }
    }
    now + swing_delay
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

