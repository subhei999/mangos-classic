use super::*;
use wow_proto::world::WorldOpcode;
use wow_proto::ServerWorldPacket;

// CMaNGOS reference: Player::SetRestType, Player::SetRestBonus, Player::ComputeRest,
// Player::GetXPRestBonus, and WorldSession::HandleAreaTriggerOpcode.
pub(in crate::world) fn rest_bonus_max(next_level_xp: u32) -> f32 {
    next_level_xp as f32 * 1.5 / 2.0
}

pub(in crate::world) fn compute_rest_bonus(time_passed_secs: u64, next_level_xp: u32) -> f32 {
    time_passed_secs as f32 * (next_level_xp as f32 / 1_152_000.0)
}

pub(in crate::world) fn rest_state_for_bonus(rest_bonus: f32) -> u8 {
    if rest_bonus > 10.0 {
        REST_STATE_RESTED
    } else {
        REST_STATE_NORMAL
    }
}

pub(in crate::world) fn clamp_rest_bonus(rest_bonus: f32, level: u8, next_level_xp: u32) -> f32 {
    if level >= DEFAULT_MAX_PLAYER_LEVEL {
        return 0.0;
    }
    rest_bonus.max(0.0).min(rest_bonus_max(next_level_xp))
}

pub(in crate::world) fn offline_rest_bonus(
    stored_bonus: f32,
    logout_time: u64,
    is_logout_resting: bool,
    now_secs: u64,
    level: u8,
    next_level_xp: u32,
) -> f32 {
    if logout_time == 0 {
        return clamp_rest_bonus(stored_bonus, level, next_level_xp);
    }
    let elapsed = now_secs.saturating_sub(logout_time);
    let mut gained = compute_rest_bonus(elapsed, next_level_xp);
    if !is_logout_resting {
        gained /= 4.0;
    }
    clamp_rest_bonus(stored_bonus + gained, level, next_level_xp)
}

pub(in crate::world) fn player_bytes2_with_rest_bonus(player_bytes2: u32, rest_bonus: f32) -> u32 {
    (player_bytes2 & 0x00FF_FFFF) | ((rest_state_for_bonus(rest_bonus) as u32) << 24)
}

pub(in crate::world) fn set_session_rest_bonus(
    session: &mut WorldSessionState,
    rest_bonus: f32,
    level: u8,
    next_level_xp: u32,
) -> u32 {
    let rest_bonus = clamp_rest_bonus(rest_bonus, level, next_level_xp);
    session.rest.rest_bonus = rest_bonus;
    let Some(visual) = session.character.player_visual.as_mut() else {
        return 0;
    };
    visual.player_bytes2 = player_bytes2_with_rest_bonus(visual.player_bytes2, rest_bonus);
    visual.player_bytes2
}

pub(in crate::world) async fn send_rest_update(
    stream: &mut WorldPacketSink,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let Some(visual) = session.character.player_visual.as_ref() else {
        return Ok(());
    };
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &build_player_rest_update_body(
            character.guid,
            visual.player_bytes2,
            session.rest.rest_bonus,
        )?,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn set_rest_type(
    stream: &mut WorldPacketSink,
    maps: &Arc<MapRuntimeManager>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
    rest_type: RestType,
    area_trigger_id: Option<u32>,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    if session.rest.rest_type == rest_type
        && (rest_type != RestType::InTavern || session.rest.inn_trigger_id == area_trigger_id)
    {
        return Ok(());
    }

    session.rest.rest_type = rest_type;
    if rest_type == RestType::No {
        session.character.player_flags &= !PLAYER_FLAGS_RESTING;
        session.rest.time_inn_enter = None;
        session.rest.inn_trigger_id = None;
    } else {
        session.character.player_flags |= PLAYER_FLAGS_RESTING;
        session.rest.time_inn_enter = Some(current_unix_time_secs());
        session.rest.inn_trigger_id = area_trigger_id;
        send_packet(
            stream,
            WorldOpcode::SmsgSetRestStart as u16,
            &wow_proto::SmsgSetRestStartResponse {
                rest_start: session
                    .rest
                    .time_inn_enter
                    .unwrap_or_default()
                    .min(u64::from(u32::MAX)) as u32,
            }
            .body(),
            Some(&mut *header_crypto),
        )
        .await?;
    }

    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &build_player_gm_mode_update_body(
            ObjectGuid::new(HighGuid::Player, 0, character.guid),
            character.race,
            session.character.player_flags,
        )?,
        Some(&mut *header_crypto),
    )
    .await?;
    maps.sync_player_gameplay_state(character.position.map_id, character.guid, session)
        .await;
    Ok(())
}

pub(in crate::world) async fn update_online_rest_bonus_if_due(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if session.rest.rest_type == RestType::No {
        return Ok(());
    }
    let now_secs = current_unix_time_secs();
    let Some(time_inn_enter) = session.rest.time_inn_enter else {
        session.rest.time_inn_enter = Some(now_secs);
        return Ok(());
    };
    let time_inn = now_secs.saturating_sub(time_inn_enter);
    if time_inn < 10 {
        return Ok(());
    }
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let next_level_xp = session.rest.next_level_xp;
    let gained = compute_rest_bonus(time_inn, next_level_xp);
    set_session_rest_bonus(
        session,
        session.rest.rest_bonus + gained,
        character.level,
        next_level_xp,
    );
    session.rest.time_inn_enter = Some(now_secs);
    send_rest_update(stream, session, header_crypto).await?;
    persist_character_rest_state(character_db_pool, session).await
}

pub(in crate::world) async fn clear_tavern_rest_if_outside_inn_trigger(
    stream: &mut WorldPacketSink,
    maps: &Arc<MapRuntimeManager>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if session.rest.rest_type != RestType::InTavern {
        return Ok(());
    }
    let Some(trigger_id) = session.rest.inn_trigger_id else {
        return set_rest_type(stream, maps, session, header_crypto, RestType::No, None).await;
    };
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    if maps
        .geometry
        .world_data_files
        .area_trigger_contains_position(trigger_id, character.position, 0.0)
        .unwrap_or(false)
    {
        return Ok(());
    }
    set_rest_type(stream, maps, session, header_crypto, RestType::No, None).await
}

pub(in crate::world) fn consume_rested_xp(
    session: &mut WorldSessionState,
    base_xp: u32,
    next_level_xp: u32,
) -> u32 {
    let Some(character) = session.character.active_character.as_ref() else {
        return 0;
    };
    let rested_bonus = (session.rest.rest_bonus as u32).min(base_xp);
    if rested_bonus == 0 {
        return 0;
    }
    set_session_rest_bonus(
        session,
        session.rest.rest_bonus - rested_bonus as f32,
        character.level,
        next_level_xp,
    );
    rested_bonus
}

pub(in crate::world) async fn persist_character_rest_state(
    character_db_pool: &MySqlPool,
    session: &WorldSessionState,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let rows = wow_db::update_character_rest_state(
        character_db_pool,
        character.guid,
        session.rest.rest_bonus,
        session.character.player_flags & PLAYER_FLAGS_RESTING != 0,
        current_unix_time_secs(),
    )
    .await?;
    if rows == 0 {
        warn!(
            guid = character.guid,
            "No character row updated while persisting rest state"
        );
    }
    Ok(())
}
