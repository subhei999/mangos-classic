use super::*;
use wow_proto::{ServerWorldPacket, SmsgLogoutCompleteResponse, SmsgLogoutResponse};

// CMaNGOS reference: src/game/Handlers/CharacterHandler.cpp logout flow.
pub(in crate::world) const LOGOUT_FAILURE_NONE: u32 = 0;
pub(in crate::world) const LOGOUT_FAILURE_CANT_LOGOUT_NOW: u32 = 1;
pub(in crate::world) const LOGOUT_DELAY: Duration = Duration::from_secs(20);

pub(in crate::world) async fn handle_logout_request(
    stream: &mut WorldPacketSink,
    deps: LogoutDeps<'_>,
    header_crypto: &mut HeaderCrypto,
    session: &mut WorldSessionState,
) -> anyhow::Result<()> {
    if logout_is_blocked_by_combat(session) {
        let body = build_logout_response_body(LOGOUT_FAILURE_CANT_LOGOUT_NOW, false);
        send_packet(stream, SMSG_LOGOUT_RESPONSE, &body, Some(header_crypto)).await?;
        cancel_pending_logout(session);
        return Ok(());
    }

    if !logout_request_is_instant(session) {
        let body = build_logout_response_body(LOGOUT_FAILURE_NONE, false);
        send_packet(
            stream,
            SMSG_LOGOUT_RESPONSE,
            &body,
            Some(&mut *header_crypto),
        )
        .await?;
        start_pending_logout(session, Instant::now());
        apply_logout_timer_stun(stream, deps.maps, deps.sessions, session, header_crypto).await?;
        return Ok(());
    }

    complete_logout_to_character_selection(
        stream,
        deps,
        header_crypto,
        session,
        "Completing instant logout to character selection",
        true,
    )
    .await
}

pub(in crate::world) async fn complete_pending_logout_if_due(
    stream: &mut WorldPacketSink,
    deps: LogoutDeps<'_>,
    header_crypto: &mut HeaderCrypto,
    session: &mut WorldSessionState,
    now: Instant,
) -> anyhow::Result<bool> {
    if !pending_logout_is_due(session, now) {
        return Ok(false);
    }

    complete_logout_to_character_selection(
        stream,
        deps,
        header_crypto,
        session,
        "Completing delayed logout to character selection",
        false,
    )
    .await?;
    Ok(true)
}

pub(in crate::world) async fn handle_logout_cancel(
    stream: &mut WorldPacketSink,
    maps: &Arc<MapRuntimeManager>,
    sessions: &Arc<SessionRegistry>,
    header_crypto: &mut HeaderCrypto,
    session: &mut WorldSessionState,
) -> anyhow::Result<()> {
    let had_pending_logout = session.logout.requested_at.is_some();
    cancel_pending_logout(session);
    if had_pending_logout {
        clear_logout_timer_stun(stream, maps, sessions, session, header_crypto).await?;
    }
    send_packet(
        stream,
        SMSG_LOGOUT_CANCEL_ACK,
        &wow_proto::SmsgLogoutCancelAckResponse.body(),
        Some(header_crypto),
    )
    .await
}

async fn complete_logout_to_character_selection(
    stream: &mut WorldPacketSink,
    deps: LogoutDeps<'_>,
    header_crypto: &mut HeaderCrypto,
    session: &mut WorldSessionState,
    log_message: &'static str,
    send_success_response: bool,
) -> anyhow::Result<()> {
    if let Some(character) = &session.character.active_character {
        info!(
            guid = character.guid,
            name = %character.name,
            x = character.position.x,
            y = character.position.y,
            z = character.position.z,
            o = character.position.orientation,
            "{log_message}"
        );
    } else {
        info!("Completing logout request before character login");
    }

    if send_success_response {
        let body = build_logout_response_body(LOGOUT_FAILURE_NONE, true);
        send_packet(
            stream,
            SMSG_LOGOUT_RESPONSE,
            &body,
            Some(&mut *header_crypto),
        )
        .await?;
    }
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
    cancel_pending_logout(session);
    Ok(())
}

pub(in crate::world) fn logout_is_blocked_by_combat(session: &WorldSessionState) -> bool {
    let airborne = session
        .character
        .active_character
        .as_ref()
        .map(|character| character.movement_flags & (MOVEFLAG_JUMPING | MOVEFLAG_FALLINGFAR) != 0)
        .unwrap_or(false);
    session.combat.player_in_combat || airborne
}

pub(in crate::world) fn logout_request_is_instant(session: &WorldSessionState) -> bool {
    session.character.active_character.is_none()
        || session.character.player_flags & PLAYER_FLAGS_RESTING != 0
}

pub(in crate::world) fn build_logout_response_body(
    failure_reason: u32,
    instant_logout: bool,
) -> Vec<u8> {
    SmsgLogoutResponse {
        failure_reason,
        instant_logout,
    }
    .body()
}

pub(in crate::world) fn start_pending_logout(session: &mut WorldSessionState, now: Instant) {
    session.logout.requested_at = Some(now);
}

pub(in crate::world) fn cancel_pending_logout(session: &mut WorldSessionState) {
    session.logout.requested_at = None;
}

async fn apply_logout_timer_stun(
    stream: &mut WorldPacketSink,
    maps: &Arc<MapRuntimeManager>,
    sessions: &Arc<SessionRegistry>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    session.character.player_stand_state = PLAYER_STAND_STATE_SIT;
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_stand_state_update_body(character, PLAYER_STAND_STATE_SIT)?,
        Some(&mut *header_crypto),
    )
    .await?;
    let player = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    send_packet(
        stream,
        SMSG_FORCE_MOVE_ROOT,
        &build_force_move_root_body(player, 0)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_unit_flags_update_body(
            player,
            player_unit_flags_with_looting(session.combat.player_in_combat, false)
                | UNIT_FLAG_STUNNED,
        )?,
        Some(&mut *header_crypto),
    )
    .await?;
    let packets = maps
        .set_player_stand_state(
            character.position.map_id,
            character.guid,
            PLAYER_STAND_STATE_SIT,
        )
        .await?;
    sessions.dispatch(packets).await;
    Ok(())
}

async fn clear_logout_timer_stun(
    stream: &mut WorldPacketSink,
    maps: &Arc<MapRuntimeManager>,
    sessions: &Arc<SessionRegistry>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    if session.character.player_stand_state == PLAYER_STAND_STATE_SIT {
        session.character.player_stand_state = PLAYER_STAND_STATE_STAND;
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &build_player_stand_state_update_body(character, PLAYER_STAND_STATE_STAND)?,
            Some(&mut *header_crypto),
        )
        .await?;
        let packets = maps
            .set_player_stand_state(
                character.position.map_id,
                character.guid,
                PLAYER_STAND_STATE_STAND,
            )
            .await?;
        sessions.dispatch(packets).await;
    }
    let player = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    send_packet(
        stream,
        SMSG_FORCE_MOVE_UNROOT,
        &build_force_move_unroot_body(player, 0)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_unit_flags_update_body(
            player,
            player_unit_flags_with_looting(session.combat.player_in_combat, false),
        )?,
        Some(header_crypto),
    )
    .await?;
    Ok(())
}

pub(in crate::world) fn pending_logout_due_at(session: &WorldSessionState) -> Option<Instant> {
    session
        .logout
        .requested_at
        .map(|requested_at| requested_at + LOGOUT_DELAY)
}

pub(in crate::world) fn pending_logout_is_due(session: &WorldSessionState, now: Instant) -> bool {
    pending_logout_due_at(session).is_some_and(|due_at| now >= due_at)
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
        persist_active_character_position(character_db_pool, account_id, maps, session).await?;
    } else {
        persist_player_death_state(character_db_pool, account_id, session).await?;
    }
    persist_character_spell_cooldowns(
        character_db_pool,
        session
            .character
            .active_character
            .as_ref()
            .map(|character| character.guid),
        &session.character.spell_cooldowns_until,
        &session.character.spell_cooldown_categories,
        &session.character.spell_cooldown_item_ids,
        &session.character.spell_global_cooldowns_until,
        Instant::now(),
    )
    .await?;
    persist_character_auras(character_db_pool, maps, session).await?;
    Ok(())
}

pub(in crate::world) async fn persist_character_auras(
    character_db_pool: &MySqlPool,
    maps: &Arc<MapRuntimeManager>,
    session: &WorldSessionState,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let now = Instant::now();
    let active_auras = maps
        .player_runtime_snapshot(character.position.map_id, character.guid)
        .await
        .map(|snapshot| snapshot.active_auras)
        .unwrap_or_else(|| session.auras.active_auras.clone());
    let saved: Vec<wow_db::CharacterAura> = active_auras
        .iter()
        .filter(|aura| aura.visible && aura.positive)
        .filter_map(|aura| {
            let maxduration = aura.duration_millis.unwrap_or(0).min(i32::MAX as u32) as i32;
            let remaintime = aura
                .remaining_duration_millis(now)
                .unwrap_or(0)
                .min(i32::MAX as u32) as i32;
            if maxduration > 0 && remaintime == 0 {
                return None;
            }
            Some(wow_db::CharacterAura {
                spell: aura.spell_id,
                caster_guid: aura.caster.raw(),
                stackcount: 1,
                maxduration,
                remaintime,
                eff_index_mask: 0x7,
            })
        })
        .collect();
    wow_db::replace_character_auras(character_db_pool, character.guid, &saved).await?;
    Ok(())
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

pub(in crate::world) async fn unregister_active_character_after_disconnect(
    online_characters: &OnlineCharacters,
    maps: &Arc<MapRuntimeManager>,
    sessions: &Arc<SessionRegistry>,
    session_id: SessionId,
    session: &mut WorldSessionState,
    now: Instant,
) {
    if let Some(character) = session.character.active_character.take() {
        online_characters.lock().await.remove(&character.guid);
        sessions.set_active_character(session_id, None, None).await;
        let linger_packets = if session.combat.player_in_combat {
            maps.disconnect_player_for_linger(character.position.map_id, character.guid, now)
                .await
        } else {
            None
        };
        if let Some(packets) = linger_packets {
            sessions.dispatch(packets).await;
        } else {
            let packets = maps
                .remove_player(character.position.map_id, character.guid)
                .await;
            sessions.dispatch(packets).await;
        }
    }
    session.character.active_spells.clear();
    session.death.player_death_state = PlayerDeathState::Alive;
    session.death.player_death_presentation_pending = false;
    session.death.player_corpse = None;
    session.character.player_visual = None;
    session.character.player_flags = 0;
}

pub(in crate::world) async fn persist_expired_disconnected_player(
    character_db_pool: &MySqlPool,
    player: &PlayerRuntime,
) -> anyhow::Result<()> {
    let Some(account_id) = player.account_id else {
        warn!(
            guid = player.guid,
            "Skipping disconnected player linger persistence without account id"
        );
        return Ok(());
    };
    if player.death_state == PlayerDeathState::Alive && player.health > 0 {
        wow_db::update_character_position_and_vitals(
            character_db_pool,
            account_id,
            player.guid,
            player.position,
            player.health,
            player.power1,
            player.power2,
        )
        .await?;
    } else {
        wow_db::update_character_death_state(
            character_db_pool,
            account_id,
            player.guid,
            player.position,
            player.health,
            player.flags,
        )
        .await?;
    }
    persist_character_spell_cooldowns(
        character_db_pool,
        Some(player.guid),
        &player.spell_cooldowns_until,
        &player.spell_cooldown_categories,
        &player.spell_cooldown_item_ids,
        &player.spell_global_cooldowns_until,
        Instant::now(),
    )
    .await?;
    let now = Instant::now();
    let saved: Vec<wow_db::CharacterAura> = player
        .active_auras
        .iter()
        .filter(|aura| aura.visible && aura.positive)
        .filter_map(|aura| {
            let maxduration = aura.duration_millis.unwrap_or(0).min(i32::MAX as u32) as i32;
            let remaintime = aura
                .remaining_duration_millis(now)
                .unwrap_or(0)
                .min(i32::MAX as u32) as i32;
            if maxduration > 0 && remaintime == 0 {
                return None;
            }
            Some(wow_db::CharacterAura {
                spell: aura.spell_id,
                caster_guid: aura.caster.raw(),
                stackcount: 1,
                maxduration,
                remaintime,
                eff_index_mask: 0x7,
            })
        })
        .collect();
    wow_db::replace_character_auras(character_db_pool, player.guid, &saved).await?;
    Ok(())
}

async fn persist_character_spell_cooldowns(
    character_db_pool: &MySqlPool,
    character_guid: Option<u32>,
    spell_cooldowns_until: &HashMap<u32, Instant>,
    spell_cooldown_categories: &HashMap<u32, u32>,
    spell_cooldown_item_ids: &HashMap<u32, u32>,
    spell_global_cooldowns_until: &HashMap<u32, Instant>,
    now: Instant,
) -> anyhow::Result<()> {
    let Some(character_guid) = character_guid else {
        return Ok(());
    };
    let now_epoch_secs = current_unix_time_secs();
    let cooldowns: Vec<wow_db::CharacterSpellCooldown> = spell_cooldowns_until
        .iter()
        .filter_map(|(spell_id, spell_until)| {
            if *spell_until <= now {
                return None;
            }
            let spell_expire_time =
                now_epoch_secs + spell_until.duration_since(now).as_secs().max(1);
            Some(wow_db::CharacterSpellCooldown {
                spell_id: *spell_id,
                spell_expire_time,
                category: spell_cooldown_categories
                    .get(spell_id)
                    .copied()
                    .unwrap_or_default(),
                category_expire_time: spell_cooldown_categories
                    .get(spell_id)
                    .and_then(|category| spell_global_cooldowns_until.get(category))
                    .filter(|category_until| **category_until > now)
                    .map(|category_until| {
                        now_epoch_secs + category_until.duration_since(now).as_secs().max(1)
                    })
                    .unwrap_or_default(),
                item_id: spell_cooldown_item_ids
                    .get(spell_id)
                    .copied()
                    .unwrap_or_default(),
            })
        })
        .collect();
    wow_db::replace_character_spell_cooldowns(character_db_pool, character_guid, &cooldowns)
        .await?;

    if !spell_global_cooldowns_until.is_empty() {
        debug!(
            guid = character_guid,
            count = spell_global_cooldowns_until.len(),
            "Skipping standalone category cooldown persistence until spell/category ownership is tracked together"
        );
    }
    Ok(())
}
