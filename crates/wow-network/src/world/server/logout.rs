// CMaNGOS reference: src/game/Handlers/CharacterHandler.cpp logout flow.

async fn handle_logout_request(
    stream: &mut WorldPacketSink,
    deps: LogoutDeps<'_>,
    header_crypto: &mut HeaderCrypto,
    session: &mut WorldSessionState,
) -> anyhow::Result<()> {
    if let Some(character) = &session.active_character {
        info!(
            guid = character.guid,
            name = %character.name,
            x = character.position.x,
            y = character.position.y,
            z = character.position.z,
            o = character.position.orientation,
            "Completing instant logout to character selection"
        );
    } else {
        info!("Completing logout request before character login");
    }

    let mut body = Vec::with_capacity(5);
    body.extend_from_slice(&0u32.to_le_bytes()); // no logout failure reason
    body.push(1); // instant logout, matching rested/GM-style response shape
    send_packet(
        stream,
        SMSG_LOGOUT_RESPONSE,
        &body,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(stream, SMSG_LOGOUT_COMPLETE, &[], Some(header_crypto)).await?;
    persist_session_character_state(deps.character_db_pool, deps.account_id, deps.maps, session)
        .await?;
    unregister_active_character(
        deps.online_characters,
        deps.maps,
        deps.sessions,
        deps.session_id,
        session,
    )
    .await;
    Ok(())
}

struct LogoutDeps<'a> {
    character_db_pool: &'a MySqlPool,
    online_characters: &'a OnlineCharacters,
    maps: &'a Arc<MapRuntimeManager>,
    sessions: &'a Arc<SessionRegistry>,
    account_id: u32,
    session_id: SessionId,
}

async fn persist_session_character_state(
    character_db_pool: &MySqlPool,
    account_id: u32,
    maps: &Arc<MapRuntimeManager>,
    session: &WorldSessionState,
) -> anyhow::Result<()> {
    if session.player_death_state == PlayerDeathState::Alive {
        persist_active_character_position(character_db_pool, account_id, maps, session).await
    } else {
        persist_player_death_state(character_db_pool, account_id, session).await
    }
}

async fn unregister_active_character(
    online_characters: &OnlineCharacters,
    maps: &Arc<MapRuntimeManager>,
    sessions: &Arc<SessionRegistry>,
    session_id: SessionId,
    session: &mut WorldSessionState,
) {
    if let Some(character) = session.active_character.take() {
        online_characters.lock().await.remove(&character.guid);
        sessions.set_active_character(session_id, None, None).await;
        let packets = maps
            .remove_player(character.position.map_id, character.guid)
            .await;
        sessions.dispatch(packets).await;
    }
    session.active_spells.clear();
    session.player_death_state = PlayerDeathState::Alive;
    session.player_corpse = None;
    session.player_visual = None;
    session.player_flags = 0;
}

