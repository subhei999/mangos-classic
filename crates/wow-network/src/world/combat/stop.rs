
async fn handle_attack_stop(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        return Ok(());
    };
    let attacker = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let victim = shared_world
        .maps
        .player_auto_attack_target(character.position.map_id, character.guid)
        .await
        .unwrap_or_else(rust_combat_dummy_guid);
    session.last_player_melee_swing_error = None;
    shared_world
        .maps
        .set_player_auto_attack(character.position.map_id, character.guid, None, None)
        .await;
    send_packet(
        stream,
        SMSG_ATTACKSTOP,
        &build_attack_stop_body(attacker, victim, false)?,
        Some(header_crypto),
    )
    .await
}

