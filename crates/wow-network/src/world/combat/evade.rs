fn db_creature_should_evade(session: &WorldSessionState, attacker: ObjectGuid) -> bool {
    let Some(creature) = session.db_creatures.get(&attacker.raw()) else {
        return false;
    };
    if matches!(creature.motion, CreatureMotionState::ReturnHome(_)) {
        return false;
    }
    distance_2d(
        creature.current_position.x,
        creature.current_position.y,
        creature.home_position.x,
        creature.home_position.y,
    ) > DB_CREATURE_LEASH_RADIUS_YARDS
}

async fn send_db_creature_evade_and_return_home(
    stream: &mut WorldPacketSink,
    broadcast: CreatureCombatBroadcast<'_>,
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    prepare_db_creature_evade(session, attacker);
    broadcast
        .shared_world
        .maps
        .clear_db_creature_combat(broadcast.map_id, attacker)
        .await;

    let attack_stop_body = build_attack_stop_body(attacker, broadcast.player, false)?;
    send_packet(
        stream,
        SMSG_ATTACKSTOP,
        &attack_stop_body,
        Some(&mut *header_crypto),
    )
    .await?;
    broadcast_db_creature_packet(
        broadcast,
        session,
        attacker,
        SMSG_ATTACKSTOP,
        attack_stop_body,
    )
    .await;
    if session.active_creature_combats.is_empty() {
        send_player_combat_flag_if_changed(stream, session, false, header_crypto).await?;
    }
    let creature_flags_body = session
        .db_creatures
        .get(&attacker.raw())
        .map(|creature| {
            build_unit_flags_update_body(attacker, db_creature_unit_flags(creature, false))
        })
        .transpose()?;
    if let Some(creature_flags_body) = creature_flags_body {
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &creature_flags_body,
            Some(&mut *header_crypto),
        )
        .await?;
        broadcast_db_creature_packet(
            broadcast,
            session,
            attacker,
            SMSG_UPDATE_OBJECT,
            creature_flags_body,
        )
        .await;
    }
    let health = session
        .db_creatures
        .get(&attacker.raw())
        .map(|creature| creature.health)
        .unwrap_or_default();
    let state_body = build_db_creature_state_update_body(attacker, health, 0)?;
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &state_body,
        Some(&mut *header_crypto),
    )
    .await?;
    broadcast_db_creature_packet(
        broadcast,
        session,
        attacker,
        SMSG_UPDATE_OBJECT,
        state_body,
    )
    .await;
    if let Some(motion) = start_db_creature_return_home_motion(session, attacker, now) {
        let body = build_monster_move_path_body_inner(
            attacker,
            motion.start,
            &motion.path,
            motion.spline_id,
            motion.duration.as_millis().max(1) as u32,
            None,
            true,
        )?;
        send_packet(
            stream,
            SMSG_MONSTER_MOVE,
            &body,
            Some(header_crypto),
        )
        .await?;
        broadcast_db_creature_packet(
            broadcast,
            session,
            attacker,
            SMSG_MONSTER_MOVE,
            body,
        )
        .await;
    }
    Ok(())
}

fn prepare_db_creature_evade(session: &mut WorldSessionState, attacker: ObjectGuid) {
    if let Some(creature) = session.db_creatures.get_mut(&attacker.raw()) {
        creature.health = creature.max_health();
        creature.life_state = DbCreatureLifeState::Alive;
        creature.corpse_expires_at = None;
        creature.respawn_at = None;
        creature.respawn_epoch_secs = None;
        creature.lootable = false;
        creature.looting = false;
        creature.loot_money_available = false;
        creature.loot_item = None;
    }
    if session.active_combat_target == Some(attacker) {
        session.active_combat_target = None;
        session.active_combat_next_swing_at = None;
    }
    clear_db_creature_combat_if_attacker(session, attacker);
}

async fn send_db_creature_chase_if_needed(
    stream: &mut WorldPacketSink,
    broadcast: CreatureCombatBroadcast<'_>,
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if db_creature_can_reach_player(session, attacker) {
        return Ok(());
    }
    let Some(motion) = start_db_creature_chase_motion(session, attacker, broadcast.player, now)
    else {
        return Ok(());
    };
    let body = build_monster_move_facing_target_path_body(
        attacker,
        motion.start,
        &motion.path,
        motion.spline_id,
        motion.duration.as_millis().max(1) as u32,
        broadcast.player,
    )?;
    send_packet(
        stream,
        SMSG_MONSTER_MOVE,
        &body,
        Some(header_crypto),
    )
    .await?;
    broadcast_db_creature_packet(
        broadcast,
        session,
        attacker,
        SMSG_MONSTER_MOVE,
        body,
    )
    .await;
    Ok(())
}

