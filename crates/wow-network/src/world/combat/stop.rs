
async fn handle_attack_stop(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        return Ok(());
    };
    let map_id = character.position.map_id;
    let character_guid = character.guid;
    let attacker = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    shared_world
        .maps
        .clear_player_next_melee_spell(map_id, character_guid)
        .await;
    let Some(victim) = shared_world
        .maps
        .player_auto_attack_target(map_id, character_guid)
        .await
    else {
        return Ok(());
    };
    let next_swing_at = shared_world
        .maps
        .player_runtime_snapshot(map_id, character_guid)
        .await
        .and_then(|snapshot| snapshot.active_combat_next_swing_at);
    session.last_player_melee_swing_error = None;
    mirror_session_player_auto_attack(session, None, next_swing_at);
    shared_world
        .maps
        .set_player_auto_attack(map_id, character_guid, None, next_swing_at)
        .await;
    let attack_stop_body = build_attack_stop_body(attacker, victim, false)?;
    send_packet(
        stream,
        SMSG_ATTACKSTOP,
        &attack_stop_body,
        Some(header_crypto),
    )
    .await?;
    let packets = shared_world
        .maps
        .broadcast_nearby_player_packet(
            map_id,
            character_guid,
            PLAYER_VISIBILITY_RADIUS_YARDS,
            OutboundWorldPacket {
                opcode: SMSG_ATTACKSTOP,
                body: attack_stop_body,
            },
        )
        .await;
    shared_world.sessions.dispatch(packets).await;
    Ok(())
}

