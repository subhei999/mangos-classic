use super::*;
use wow_proto::{ServerWorldPacket, SmsgLogoutCompleteResponse, SmsgLogoutResponse};

// CMaNGOS reference: src/game/Handlers/CharacterHandler.cpp logout flow.

pub(in crate::world) async fn handle_logout_request(
    stream: &mut WorldPacketSink,
    deps: LogoutDeps<'_>,
    header_crypto: &mut HeaderCrypto,
    session: &mut WorldSessionState,
) -> anyhow::Result<()> {
    if let Some(character) = &session.character.active_character {
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

    let body = SmsgLogoutResponse {
        failure_reason: 0,
        instant_logout: true,
    }
    .body();
    send_packet(
        stream,
        SMSG_LOGOUT_RESPONSE,
        &body,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_LOGOUT_COMPLETE,
        &SmsgLogoutCompleteResponse.body(),
        Some(header_crypto),
    )
    .await?;
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

pub(in crate::world) struct LogoutDeps<'a> {
    pub(in crate::world) character_db_pool: &'a MySqlPool,
    pub(in crate::world) online_characters: &'a OnlineCharacters,
    pub(in crate::world) maps: &'a Arc<MapRuntimeManager>,
    pub(in crate::world) sessions: &'a Arc<SessionRegistry>,
    pub(in crate::world) account_id: u32,
    pub(in crate::world) session_id: SessionId,
}

pub(in crate::world) async fn persist_session_character_state(
    character_db_pool: &MySqlPool,
    account_id: u32,
    maps: &Arc<MapRuntimeManager>,
    session: &WorldSessionState,
) -> anyhow::Result<()> {
    if session.death.player_death_state == PlayerDeathState::Alive {
        persist_active_character_position(character_db_pool, account_id, maps, session).await
    } else {
        persist_player_death_state(character_db_pool, account_id, session).await
    }
}

pub(in crate::world) async fn unregister_active_character(
    online_characters: &OnlineCharacters,
    maps: &Arc<MapRuntimeManager>,
    sessions: &Arc<SessionRegistry>,
    session_id: SessionId,
    session: &mut WorldSessionState,
) {
    if let Some(character) = session.character.active_character.take() {
        online_characters.lock().await.remove(&character.guid);
        sessions.set_active_character(session_id, None, None).await;
        let packets = maps
            .remove_player(character.position.map_id, character.guid)
            .await;
        sessions.dispatch(packets).await;
    }
    session.character.active_spells.clear();
    session.death.player_death_state = PlayerDeathState::Alive;
    session.death.player_death_presentation_pending = false;
    session.death.player_corpse = None;
    session.character.player_visual = None;
    session.character.player_flags = 0;
}
