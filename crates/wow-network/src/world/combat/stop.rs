
async fn handle_attack_stop(
    stream: &mut WorldPacketSink,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        return Ok(());
    };
    let attacker = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let victim = session.active_combat_target.unwrap_or_else(rust_combat_dummy_guid);
    session.active_combat_target = None;
    session.active_combat_next_swing_at = None;
    session.last_player_melee_swing_error = None;
    send_packet(
        stream,
        SMSG_ATTACKSTOP,
        &build_attack_stop_body(attacker, victim, false)?,
        Some(header_crypto),
    )
    .await
}

