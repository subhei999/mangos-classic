
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
    let victim = shared_world
        .maps
        .player_auto_attack_target(map_id, character_guid)
        .await
        .unwrap_or_else(rust_combat_dummy_guid);
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
    send_packet(
        stream,
        SMSG_ATTACKSTOP,
        &build_attack_stop_body(attacker, victim, false)?,
        Some(header_crypto),
    )
    .await
}

