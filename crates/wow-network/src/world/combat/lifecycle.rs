#[cfg(test)]
fn apply_db_creature_damage(
    session: &mut WorldSessionState,
    target: ObjectGuid,
    requested_damage: u32,
) -> Option<u32> {
    let creature = session.db_creatures.get_mut(&target.raw())?;
    if !creature.is_alive() || creature.is_evading_home() {
        return None;
    }

    let damage = creature.health.min(requested_damage.max(1));
    creature.health = creature.health.saturating_sub(damage);
    if creature.health == 0 {
        creature.begin_corpse(Instant::now(), current_unix_epoch_secs());
        session.active_combat_target = None;
        session.active_combat_next_swing_at = None;
        clear_db_creature_combat_if_attacker(session, target);
    }
    Some(damage)
}

async fn handle_combat_tick(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    shared_world: SharedWorldDeps<'_>,
    account_id: u32,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let now = Instant::now();
    sync_session_db_creatures_from_map(shared_world, session).await;
    advance_db_creature_lifecycle(
        stream,
        character_db_pool,
        shared_world,
        session,
        now,
        header_crypto,
    )
    .await?;
    advance_db_creature_return_home_motions(shared_world, session, now).await;
    sync_session_db_creature_idle_motions_from_map(shared_world, session).await;
    if session.player_death_state != PlayerDeathState::Alive {
        return Ok(());
    }
    if let Some(target) = session.active_combat_target {
        if session
            .active_combat_next_swing_at
            .is_none_or(|next_swing_at| now >= next_swing_at)
        {
            if target == rust_combat_dummy_guid() {
                session.active_combat_next_swing_at = Some(combat_dummy_next_swing_at(now));
                send_combat_dummy_swing(stream, session, header_crypto).await?;
            } else {
                send_db_creature_swing(
                    stream,
                    character_db_pool,
                    world_db_pool,
                    shared_world,
                    session,
                    header_crypto,
                    target,
                )
                .await?;
            }
        }
    }

    try_start_db_creature_aggro(stream, shared_world, session, header_crypto).await?;
    send_active_db_creature_attack(
        stream,
        character_db_pool,
        world_db_pool,
        shared_world,
        account_id,
        session,
        header_crypto,
    )
    .await
}

async fn sync_session_db_creatures_from_map(
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
) {
    let Some(character) = session.active_character.as_ref() else {
        return;
    };
    let guids = session.db_creatures.keys().copied().collect::<Vec<_>>();
    if guids.is_empty() {
        return;
    }
    let snapshots = shared_world
        .maps
        .db_creature_snapshots(character.position.map_id, &guids)
        .await;
    for shared in snapshots {
        let guid = shared.guid().raw();
        let client_visible = session
            .db_creatures
            .get(&guid)
            .map(|creature| creature.client_visible)
            .unwrap_or(shared.client_visible);
        let mut shared = shared;
        shared.client_visible = client_visible && shared.life_state != DbCreatureLifeState::Dead;
        session.db_creatures.insert(guid, shared);
    }
}

async fn advance_db_creature_lifecycle(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.active_character.as_ref() else {
        return Ok(());
    };
    let map_id = character.position.map_id;
    let character_guid = character.guid;
    let creature_guids = session.db_creatures.keys().copied().collect::<Vec<_>>();
    if creature_guids.is_empty() {
        return Ok(());
    }
    let events = shared_world
        .maps
        .advance_db_creature_lifecycle(
            map_id,
            &creature_guids,
            character.position,
            Some(character_guid),
            now,
        )
        .await?;
    for event in events {
        let guid = event.creature.guid().raw();
        let mut creature = event.creature;
        creature.client_visible = creature.life_state != DbCreatureLifeState::Dead
            && is_db_creature_inside_visibility_radius(&creature, character.position);
        session.db_creatures.insert(guid, creature);
        for packet in event.direct_packets {
            send_packet(
                stream,
                packet.opcode,
                &packet.body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
        shared_world.sessions.dispatch(event.observer_packets).await;
        if let Some(creature_guid) = event.clear_respawn_guid {
            wow_db::save_creature_respawn_time(
                character_db_pool,
                creature_guid,
                0,
                0,
                current_unix_epoch_secs(),
            )
            .await?;
        }
    }

    Ok(())
}

async fn advance_db_creature_return_home_motions(
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    now: Instant,
) {
    let Some(map_id) = session
        .active_character
        .as_ref()
        .map(|character| character.position.map_id)
    else {
        return;
    };
    let return_home_guids = session
        .db_creatures
        .iter()
        .filter_map(|(guid, creature)| {
            matches!(creature.motion, CreatureMotionState::ReturnHome(_)).then_some(*guid)
        })
        .collect::<Vec<_>>();
    for guid in return_home_guids {
        if let Some(creature) = shared_world
            .maps
            .advance_db_creature_motion(map_id, ObjectGuid::from_raw(guid), now)
            .await
        {
            session.db_creatures.insert(guid, creature);
        }
    }
}

async fn advance_db_creature_motion_and_share(
    shared_world: SharedWorldDeps<'_>,
    map_id: u32,
    session: &mut WorldSessionState,
    creature_guid: ObjectGuid,
    now: Instant,
) {
    if let Some(creature) = shared_world
        .maps
        .advance_db_creature_motion(map_id, creature_guid, now)
        .await
    {
        session.db_creatures.insert(creature_guid.raw(), creature);
    }
}

async fn sync_session_db_creature_idle_motions_from_map(
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
) {
    let Some(character) = session.active_character.as_ref() else {
        return;
    };
    let character_position = character.position;
    let map_id = character_position.map_id;
    let moving_guids = shared_world
        .maps
        .db_creature_idle_motion_advancement_guids(map_id)
        .await;
    let snapshots = shared_world
        .maps
        .db_creature_snapshots(map_id, &moving_guids)
        .await;
    for creature in snapshots {
        remember_db_creature_snapshot_if_relevant(session, character_position, creature);
    }
}

fn remember_db_creature_snapshot_if_relevant(
    session: &mut WorldSessionState,
    character_position: WorldPosition,
    mut creature: DbCreatureRuntime,
) {
    let guid = creature.guid().raw();
    let is_visible = is_db_creature_inside_visibility_radius(&creature, character_position);
    let Some(previously_visible) = session
        .db_creatures
        .get(&guid)
        .map(|existing| existing.client_visible)
        .or_else(|| (is_visible && creature.life_state != DbCreatureLifeState::Dead).then_some(false))
    else {
        return;
    };
    let direct_visible = is_visible && creature.life_state != DbCreatureLifeState::Dead;
    let _became_visible = direct_visible && !previously_visible;
    creature.client_visible = direct_visible;
    session.db_creatures.insert(guid, creature);
}

#[cfg(test)]
#[allow(dead_code)]
fn should_advance_db_creature_idle_motion(
    session: &WorldSessionState,
    guid: u64,
    creature: &DbCreatureRuntime,
) -> bool {
    creature.is_alive()
        && !session.active_creature_combats.contains_key(&guid)
        && session.active_combat_target.is_none_or(|target| target.raw() != guid)
        && matches!(
            creature.motion,
            CreatureMotionState::Random(_) | CreatureMotionState::Waypoint(_)
        )
}

#[cfg(test)]
fn db_creature_idle_motion_start_guids(
    session: &WorldSessionState,
    now: Instant,
) -> Vec<u64> {
    let mut guids = session
        .db_creatures
        .iter()
        .filter_map(|(guid, creature)| {
            should_start_db_creature_idle_motion(session, *guid, creature, now).then_some(*guid)
        })
        .collect::<Vec<_>>();
    guids.sort_unstable();
    guids.truncate(DB_CREATURE_IDLE_MOTION_STARTS_PER_TICK);
    guids
}

#[cfg(test)]
fn should_start_db_creature_idle_motion(
    session: &WorldSessionState,
    guid: u64,
    creature: &DbCreatureRuntime,
    now: Instant,
) -> bool {
    creature.is_alive()
        && !session.active_creature_combats.contains_key(&guid)
        && session.active_combat_target.is_none_or(|target| target.raw() != guid)
        && matches!(creature.motion, CreatureMotionState::Idle)
        && (creature.next_random_move_at.is_some_and(|at| now >= at)
            || creature.next_waypoint_move_at.is_some_and(|at| now >= at))
}

async fn send_db_creature_swing(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
    target: ObjectGuid,
) -> anyhow::Result<()> {
    let Some(character) = session.active_character.as_ref() else {
        return Ok(());
    };
    let map_id = character.position.map_id;
    advance_db_creature_motion_and_share(shared_world, map_id, session, target, Instant::now())
        .await;

    let Some(character) = &session.active_character else {
        return Ok(());
    };
    let character_snapshot = character.clone();
    let attacker = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    match db_creature_player_melee_check(session, target) {
        PlayerMeleeCheck::Clear => {
            session.last_player_melee_swing_error = None;
        }
        PlayerMeleeCheck::MissingTarget | PlayerMeleeCheck::TargetNotAlive => {
            send_player_melee_swing_error_if_changed(
                stream,
                session,
                PlayerMeleeSwingError::DeadTarget,
                header_crypto,
            )
            .await?;
            session.active_combat_target = None;
            session.active_combat_next_swing_at = None;
            return Ok(());
        }
        PlayerMeleeCheck::OutOfRange | PlayerMeleeCheck::NavigationBlocked(_) => {
            send_player_melee_swing_error_if_changed(
                stream,
                session,
                PlayerMeleeSwingError::NotInRange,
                header_crypto,
            )
            .await?;
            session.active_combat_next_swing_at = Some(player_melee_retry_at(Instant::now()));
            return Ok(());
        }
        PlayerMeleeCheck::BadFacing => {
            send_player_melee_swing_error_if_changed(
                stream,
                session,
                PlayerMeleeSwingError::BadFacing,
                header_crypto,
            )
            .await?;
            session.active_combat_next_swing_at = Some(player_melee_retry_at(Instant::now()));
            return Ok(());
        }
        PlayerMeleeCheck::NoActiveCharacter => {
            send_player_melee_swing_error_if_changed(
                stream,
                session,
                PlayerMeleeSwingError::CantAttack,
                header_crypto,
            )
            .await?;
            session.active_combat_next_swing_at = Some(player_melee_retry_at(Instant::now()));
            return Ok(());
        }
    }
    let world_stats = wow_db::get_player_world_stats(
        world_db_pool,
        character_snapshot.race,
        character_snapshot.class,
        character_snapshot.level,
    )
    .await?;
    let equipped_templates = load_equipped_item_templates(world_db_pool, &session.inventory).await?;
    let combat_stats = player_combat_stats_for_values(
        character_snapshot.class,
        character_snapshot.level,
        &world_stats,
        &equipped_templates,
    );
    let Some(target_creature) = session.db_creatures.get(&target.raw()).cloned() else {
        session.active_combat_target = None;
        session.active_combat_next_swing_at = None;
        return Ok(());
    };
    let melee_outcome = player_main_hand_melee_outcome_against_db_creature(
        &combat_stats,
        character_snapshot.level,
        &target_creature,
    );
    let requested_damage = melee_outcome.total_damage;
    let Some(event) = shared_world
        .maps
        .apply_db_creature_damage(
            character_snapshot.position.map_id,
            DbCreatureDamageRequest {
                creature_guid: target,
                killer: attacker,
                damage: requested_damage,
                melee_outcome: Some(melee_outcome),
                spell_id: None,
                now: Instant::now(),
                now_epoch_secs: current_unix_epoch_secs(),
                exclude_character_guid: Some(character_snapshot.guid),
            },
        )
        .await?
    else {
        session.active_combat_target = None;
        session.active_combat_next_swing_at = None;
        return Ok(());
    };
    let death_finalization = event.death_finalization;
    let target_switch = event.target_switch;
    let is_dead = death_finalization.is_some();
    session.db_creatures.insert(target.raw(), event.creature.clone());
    if is_dead {
        session.active_combat_target = None;
        session.active_combat_next_swing_at = None;
        clear_db_creature_combat_if_attacker(session, target);
    } else {
        session.active_combat_next_swing_at =
            Some(player_main_hand_next_swing_at(Instant::now(), &combat_stats));
    }
    session.player_rage =
        (session.player_rage + RUST_COMBAT_DUMMY_RAGE_GAIN).min(POWER_RAGE_DEFAULT);
    if !is_dead {
        begin_db_creature_retaliation_if_needed(
            stream,
            shared_world,
            map_id,
            session,
            target,
            attacker,
            header_crypto,
        )
        .await?;
    }

    send_packet(
        stream,
        SMSG_ATTACKERSTATEUPDATE,
        &event.attacker_state_body,
        Some(&mut *header_crypto),
    )
    .await?;
    let creature_update_body = event.update_body.clone();
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &creature_update_body,
        Some(&mut *header_crypto),
    )
    .await?;
    shared_world.sessions.dispatch(event.observer_packets).await;
    if !is_dead {
        send_db_creature_threat_target_switch(
            stream,
            shared_world,
            session,
            target_switch,
            header_crypto,
        )
        .await?;
    }
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_rage_update_body(attacker, session.player_rage)?,
        Some(&mut *header_crypto),
    )
    .await?;

    if is_dead {
        finalize_db_creature_death(
            stream,
            character_db_pool,
            world_db_pool,
            shared_world,
            session,
            death_finalization,
            header_crypto,
        )
        .await?;
    }

    Ok(())
}

async fn begin_db_creature_retaliation_if_needed(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    map_id: u32,
    session: &mut WorldSessionState,
    creature: ObjectGuid,
    player: ObjectGuid,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if begin_shared_db_creature_combat(shared_world, session, creature, Instant::now()).await {
        send_db_creature_combat_start(
            stream,
            shared_world,
            map_id,
            session,
            creature,
            player,
            header_crypto,
        )
        .await?;
    }
    Ok(())
}

fn combat_dummy_next_swing_at(now: Instant) -> Instant {
    now + Duration::from_millis(BASE_ATTACK_TIME_MS as u64)
}

fn player_melee_retry_at(now: Instant) -> Instant {
    now + Duration::from_millis(PLAYER_MELEE_RETRY_MILLIS)
}

fn player_main_hand_next_swing_at(now: Instant, combat_stats: &PlayerCombatStats) -> Instant {
    now + Duration::from_millis(combat_stats.main_attack_time_ms.max(1) as u64)
}

async fn send_player_melee_swing_error_if_changed(
    stream: &mut WorldPacketSink,
    session: &mut WorldSessionState,
    error: PlayerMeleeSwingError,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if session.last_player_melee_swing_error == Some(error) {
        return Ok(());
    }
    session.last_player_melee_swing_error = Some(error);
    let packet = error.packet();
    send_packet(stream, packet.opcode, &packet.body, Some(header_crypto)).await
}

async fn finalize_db_creature_death(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    death_finalization: Option<DbCreatureDeathFinalizationEvent>,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(death_finalization) = death_finalization else {
        return Ok(());
    };
    let killed = death_finalization.killed;
    if let Some(respawn_epoch_secs) = death_finalization.respawn_epoch_secs {
        wow_db::save_creature_respawn_time(
            character_db_pool,
            killed.counter(),
            respawn_epoch_secs,
            0,
            current_unix_epoch_secs(),
        )
        .await?;
    }
    session.active_combat_target = None;
    session.active_combat_next_swing_at = None;
    clear_db_creature_combat_if_attacker(session, killed);
    grant_db_creature_kill_credit(
        stream,
        character_db_pool,
        world_db_pool,
        session,
        killed,
        header_crypto,
    )
    .await?;
    grant_db_creature_xp(
        stream,
        character_db_pool,
        world_db_pool,
        session,
        killed,
        header_crypto,
    )
    .await?;
    if session.active_creature_combats.is_empty() {
        send_player_combat_flag_if_changed(stream, session, false, header_crypto).await?;
    }
    shared_world
        .sessions
        .dispatch(death_finalization.observer_packets)
        .await;
    if let Some(motion_stop_packet) = death_finalization.motion_stop_packet {
        send_packet(
            stream,
            motion_stop_packet.opcode,
            &motion_stop_packet.body,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    send_packet(
        stream,
        death_finalization.combat_flag_packet.opcode,
        &death_finalization.combat_flag_packet.body,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        death_finalization.attack_stop_packet.opcode,
        &death_finalization.attack_stop_packet.body,
        Some(header_crypto),
    )
    .await
}

async fn grant_db_creature_xp(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    session: &mut WorldSessionState,
    killed: ObjectGuid,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        return Ok(());
    };
    let Some(creature) = session.db_creatures.get(&killed.raw()) else {
        return Ok(());
    };
    let xp = creature_xp_reward(character.level, &creature.spawn.template);
    award_character_xp(
        stream,
        character_db_pool,
        world_db_pool,
        session,
        Some(killed),
        xp,
        header_crypto,
    )
    .await
}

async fn award_character_xp(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    session: &mut WorldSessionState,
    source: Option<ObjectGuid>,
    xp: u32,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if xp == 0 {
        return Ok(());
    }
    let Some(character) = session.active_character.as_ref() else {
        return Ok(());
    };
    if character.level >= DEFAULT_MAX_PLAYER_LEVEL {
        return Ok(());
    }

    let guid = character.guid;
    let race = character.race;
    let class = character.class;
    let old_level = character.level;
    let old_xp = character.xp;
    let previous_stats = wow_db::get_player_world_stats(world_db_pool, race, class, old_level).await?;
    let mut new_level = old_level;
    let mut new_xp = old_xp.saturating_add(xp);
    let mut next_level_xp = previous_stats.next_level_xp;
    while next_level_xp > 0
        && new_xp >= next_level_xp
        && new_level < DEFAULT_MAX_PLAYER_LEVEL
    {
        new_xp -= next_level_xp;
        new_level += 1;
        next_level_xp = wow_db::get_player_next_level_xp(world_db_pool, new_level).await?;
    }
    let new_stats = wow_db::get_player_world_stats(world_db_pool, race, class, new_level).await?;
    let leveled = new_level != old_level;
    let max_health = new_stats.max_health().max(1);
    let max_mana = new_stats.max_mana();
    let health = if leveled {
        max_health
    } else {
        session.player_health.max(1).min(max_health)
    };
    let power1 = if max_mana == 0 {
        0
    } else if leveled {
        max_mana
    } else {
        session.player_mana.min(max_mana)
    };
    let power2 = session.player_rage.min(POWER_RAGE_DEFAULT);
    let power3 = 0;
    let power4 = create_power_for_class_power(class, POWER_ENERGY);
    let power5 = 0;

    wow_db::update_character_progression_state(
        character_db_pool,
        guid,
        wow_db::CharacterProgressionState {
            level: new_level,
            xp: new_xp,
            health,
            power1,
            power2,
            power3,
            power4,
            power5,
        },
    )
    .await?;
    if let Some(character) = session.active_character.as_mut() {
        character.level = new_level;
        character.xp = new_xp;
    }
    session.player_health = health;
    session.player_mana = power1;
    session.player_rage = power2;

    send_packet(
        stream,
        SMSG_LOG_XPGAIN,
        &build_log_xp_gain_body(source, xp),
        Some(&mut *header_crypto),
    )
    .await?;
    if leveled {
        send_packet(
            stream,
            SMSG_LEVELUP_INFO,
            &build_levelup_info_body(new_level, &previous_stats, &new_stats),
            Some(&mut *header_crypto),
        )
        .await?;
    }
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_progression_update_body(PlayerProgressionUpdate {
            character_guid: guid,
            level: new_level,
            xp: new_xp,
            health,
            power1,
            power2,
            power3,
            power4,
            power5,
            world_stats: &new_stats,
        })?,
        Some(header_crypto),
    )
    .await
}

fn creature_xp_reward(player_level: u8, template: &CreatureTemplateQuery) -> u32 {
    if template.civilian != 0 || template.creature_type == CREATURE_TYPE_CRITTER {
        return 0;
    }

    let mut xp_gain = base_creature_xp_gain(player_level as u32, template.min_level as u32);
    if xp_gain == 0.0 {
        return 0;
    }
    if template.rank == CREATURE_ELITE_NORMAL || template.rank == CREATURE_ELITE_RARE_ELITE {
        xp_gain *= 2.5;
    }
    xp_gain *= template.experience_multiplier;
    nearbyint_to_u32(xp_gain)
}

fn base_creature_xp_gain(player_level: u32, mob_level: u32) -> f32 {
    let base_xp = player_level * 5 + 45;
    if mob_level >= player_level {
        let level_diff = (mob_level - player_level).min(4);
        return base_xp as f32 * (1.0 + 0.05 * level_diff as f32);
    }
    if mob_level > gray_level(player_level) {
        let level_diff = player_level - mob_level;
        return base_xp as f32 * (1.0 - (level_diff as f32 / zero_difference(player_level) as f32));
    }
    0.0
}

fn gray_level(player_level: u32) -> u32 {
    if player_level <= 5 {
        0
    } else if player_level <= 39 {
        player_level - 5 - player_level / 10
    } else if player_level <= 59 {
        player_level - 1 - player_level / 5
    } else {
        player_level - 9
    }
}

fn zero_difference(unit_level: u32) -> u32 {
    match unit_level {
        0..=7 => 5,
        8..=9 => 6,
        10..=11 => 7,
        12..=15 => 8,
        16..=19 => 9,
        20..=29 => 11,
        30..=39 => 12,
        40..=44 => 13,
        45..=49 => 14,
        50..=54 => 15,
        55..=59 => 16,
        _ => 17,
    }
}

fn quest_xp_reward(player_level: u8, quest: &QuestTemplateQuery) -> u32 {
    if quest.rew_money_max_level == 0 {
        return 0;
    }
    let quest_level = quest.quest_level;
    let divisor = match quest_level {
        65.. => 6.0,
        64 => 4.8,
        63 => 3.6,
        62 => 2.4,
        61 => 1.2,
        1..=60 => 0.6,
        _ => return 0,
    };
    let full_xp = quest.rew_money_max_level as f32 / divisor;
    let player_level = player_level as u32;
    let factor = if player_level <= quest_level + 5 {
        1.0
    } else if player_level == quest_level + 6 {
        0.8
    } else if player_level == quest_level + 7 {
        0.6
    } else if player_level == quest_level + 8 {
        0.4
    } else if player_level == quest_level + 9 {
        0.2
    } else {
        0.1
    };
    quest_xp_ceil(full_xp * factor)
}

fn quest_xp_ceil(value: f32) -> u32 {
    value.ceil().max(0.0) as u32
}

fn nearbyint_to_u32(value: f32) -> u32 {
    if value <= 0.0 {
        0
    } else {
        value.round_ties_even() as u32
    }
}

async fn send_combat_dummy_swing(
    stream: &mut WorldPacketSink,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        return Ok(());
    };
    let attacker = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let target = rust_combat_dummy_guid();

    let damage = session
        .combat_dummy_health
        .min(RUST_COMBAT_DUMMY_HIT_DAMAGE);
    session.combat_dummy_health = session.combat_dummy_health.saturating_sub(damage);
    session.player_rage =
        (session.player_rage + RUST_COMBAT_DUMMY_RAGE_GAIN).min(POWER_RAGE_DEFAULT);

    send_packet(
        stream,
        SMSG_ATTACKERSTATEUPDATE,
        &build_attacker_state_update_body(attacker, target, damage)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_combat_dummy_state_update_body(session.combat_dummy_health, 0)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_rage_update_body(attacker, session.player_rage)?,
        Some(&mut *header_crypto),
    )
    .await?;

    if session.combat_dummy_health == 0 {
        session.combat_dummy_lootable = true;
        session.combat_dummy_looting = false;
        session.combat_dummy_loot_money_available = true;
        session.combat_dummy_loot_item_available = true;
        send_packet(
            stream,
            SMSG_ATTACKSTOP,
            &build_attack_stop_body(attacker, target, true)?,
            Some(&mut *header_crypto),
        )
        .await?;
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &build_combat_dummy_state_update_body(0, UNIT_DYNFLAG_LOOTABLE)?,
            Some(header_crypto),
        )
        .await?;
        session.active_combat_target = None;
        session.active_combat_next_swing_at = None;
    }

    Ok(())
}

