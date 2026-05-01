async fn try_start_db_creature_aggro(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.active_character.clone() else {
        return Ok(());
    };
    let map_id = character.position.map_id;
    let player = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    for attacker in select_db_creature_aggro_targets(session) {
        if !begin_shared_db_creature_combat(shared_world, session, attacker, Instant::now()).await {
            continue;
        }
        send_db_creature_combat_start(
            stream,
            shared_world,
            map_id,
            session,
            attacker,
            player,
            header_crypto,
        )
        .await?;

        let assistants = shared_world
            .maps
            .select_db_creature_assist_targets(map_id, attacker, &character)
            .await;
        let assistant_targets = if let Some((caller, assistants)) = assistants {
            session.db_creatures.insert(attacker.raw(), caller);
            assistants
        } else {
            Vec::new()
        };
        for assistant in assistant_targets {
            if begin_shared_db_creature_combat(shared_world, session, assistant, Instant::now()).await {
                send_db_creature_combat_start(
                    stream,
                    shared_world,
                    map_id,
                    session,
                    assistant,
                    player,
                    header_crypto,
                )
                .await?;
            }
        }
    }
    Ok(())
}

async fn send_db_creature_combat_start(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    map_id: u32,
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
    player: ObjectGuid,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let broadcast = CreatureCombatBroadcast {
        shared_world,
        map_id,
        player,
    };
    let attack_start_body = build_attack_start_body(attacker, player);
    send_packet(
        stream,
        SMSG_ATTACKSTART,
        &attack_start_body,
        Some(&mut *header_crypto),
    )
    .await?;
    broadcast_db_creature_packet(
        broadcast,
        session,
        attacker,
        SMSG_ATTACKSTART,
        attack_start_body,
    )
    .await;
    send_player_combat_flag_if_changed(stream, session, true, header_crypto).await?;
    let creature_flags_body = session
        .db_creatures
        .get(&attacker.raw())
        .map(|creature| {
            build_unit_flags_update_body(attacker, db_creature_unit_flags(creature, true))
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
    send_db_creature_chase_if_needed(
        stream,
        broadcast,
        session,
        attacker,
        Instant::now(),
        header_crypto,
    )
    .await
}

async fn send_active_db_creature_attack(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    shared_world: SharedWorldDeps<'_>,
    account_id: u32,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        return Ok(());
    };
    let player = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let active_combats = shared_world
        .maps
        .active_db_creature_combats_for_victim(character.position.map_id, player)
        .await;
    if !active_combats.is_empty() {
        send_player_combat_flag_if_changed(stream, session, true, header_crypto).await?;
    }
    session.active_creature_combats = active_combats
        .iter()
        .map(|combat| (combat.attacker.raw(), *combat))
        .collect();
    let context = ActiveDbCreatureAttackContext {
        character_db_pool,
        world_db_pool,
        shared_world,
        account_id,
    };
    for combat in active_combats {
        send_single_active_db_creature_attack(
            stream,
            context,
            session,
            header_crypto,
            combat,
        )
        .await?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct ActiveDbCreatureAttackContext<'a> {
    character_db_pool: &'a MySqlPool,
    world_db_pool: &'a MySqlPool,
    shared_world: SharedWorldDeps<'a>,
    account_id: u32,
}

async fn send_single_active_db_creature_attack(
    stream: &mut WorldPacketSink,
    context: ActiveDbCreatureAttackContext<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
    combat: CreatureCombatState,
) -> anyhow::Result<()> {
    let ActiveDbCreatureAttackContext {
        character_db_pool,
        world_db_pool,
        shared_world,
        account_id,
    } = context;
    let attacker = combat.attacker;
    let Some(character) = session.active_character.as_ref() else {
        return Ok(());
    };
    let character_snapshot = character.clone();
    let map_id = character.position.map_id;
    let player = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let broadcast = CreatureCombatBroadcast {
        shared_world,
        map_id,
        player,
    };
    if session.player_death_state != PlayerDeathState::Alive {
        session.active_creature_combats.clear();
        shared_world
            .maps
            .clear_db_creature_combats_for_victim(map_id, player)
            .await;
        return Ok(());
    }
    if combat.victim != player {
        session.active_creature_combats.remove(&attacker.raw());
        shared_world.maps.clear_db_creature_combat(map_id, attacker).await;
        return Ok(());
    }
    if !session
        .db_creatures
        .get(&attacker.raw())
        .is_some_and(DbCreatureRuntime::is_alive)
    {
        session.active_creature_combats.remove(&attacker.raw());
        shared_world.maps.clear_db_creature_combat(map_id, attacker).await;
        return Ok(());
    }
    let now = Instant::now();
    advance_db_creature_motion_and_share(shared_world, map_id, session, attacker, now).await;
    if db_creature_should_evade(session, attacker) {
        send_db_creature_evade_and_return_home(
            stream,
            broadcast,
            session,
            attacker,
            now,
            header_crypto,
        )
        .await?;
        return Ok(());
    }
    if !db_creature_can_reach_player(session, attacker) {
        defer_ready_db_creature_swing_retry(shared_world, map_id, session, attacker, player, now)
            .await;
        send_db_creature_chase_if_needed(
            stream,
            broadcast,
            session,
            attacker,
            now,
            header_crypto,
        )
        .await?;
        return Ok(());
    }
    if !db_creature_has_player_in_arc(session, attacker) {
        send_db_creature_face_target(
            stream,
            broadcast,
            session,
            attacker,
            header_crypto,
        )
        .await?;
        defer_ready_db_creature_swing_retry(shared_world, map_id, session, attacker, player, now)
            .await;
        return Ok(());
    }
    if now < combat.next_swing_at {
        return Ok(());
    }

    let Some(attacking_creature) = session.db_creatures.get(&attacker.raw()) else {
        session.active_creature_combats.remove(&attacker.raw());
        shared_world.maps.clear_db_creature_combat(map_id, attacker).await;
        return Ok(());
    };
    let next_swing_delay = attacking_creature.base_attack_duration();
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
    let defense_input = player_melee_defense_input(&character_snapshot, &combat_stats);
    let Some(outcome) = session
        .db_creatures
        .get(&attacker.raw())
        .map(|creature| creature.melee_outcome_against_player(defense_input))
    else {
        session.active_creature_combats.remove(&attacker.raw());
        shared_world.maps.clear_db_creature_combat(map_id, attacker).await;
        return Ok(());
    };
    let Some(event) = shared_world
        .maps
        .apply_db_creature_player_melee_outcome(
            map_id,
            attacker,
            player,
            outcome,
            now + next_swing_delay,
        )
        .await?
    else {
        session.active_creature_combats.remove(&attacker.raw());
        shared_world.maps.clear_db_creature_combat(map_id, attacker).await;
        return Ok(());
    };
    session.player_health = event.victim_health;
    session
        .active_creature_combats
        .insert(attacker.raw(), event.combat);
    shared_world.sessions.dispatch(event.observer_packets).await;
    send_packet(
        stream,
        SMSG_ATTACKERSTATEUPDATE,
        &build_attacker_state_update_body_for_outcome(attacker, player, outcome, 0)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_health_update_body(player, session.player_health)?,
        Some(&mut *header_crypto),
    )
    .await?;
    if session.player_health == 0 {
        kill_player_from_creature(
            stream,
            character_db_pool,
            account_id,
            session,
            player,
            header_crypto,
        )
        .await?;
        shared_world
            .maps
            .clear_db_creature_combats_for_victim(map_id, player)
            .await;
    }
    Ok(())
}

async fn defer_ready_db_creature_swing_retry(
    shared_world: SharedWorldDeps<'_>,
    map_id: u32,
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
    victim: ObjectGuid,
    now: Instant,
) {
    if let Some(combat) = shared_world
        .maps
        .defer_ready_db_creature_swing_retry(map_id, attacker, victim, now)
        .await
    {
        session.active_creature_combats.insert(attacker.raw(), combat);
    }
}

async fn send_db_creature_threat_target_switch(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    event: Option<DbCreatureThreatTargetSwitchEvent>,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(event) = event else {
        return Ok(());
    };
    let current_player = session
        .active_character
        .as_ref()
        .map(|character| ObjectGuid::new(HighGuid::Player, 0, character.guid));
    if current_player == Some(event.old_victim) {
        session.active_creature_combats.remove(&event.attacker.raw());
    }
    if current_player == Some(event.new_victim) {
        session
            .active_creature_combats
            .insert(event.attacker.raw(), event.combat);
        send_player_combat_flag_if_changed(stream, session, true, header_crypto).await?;
    }
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
    Ok(())
}

#[cfg(test)]
fn select_db_creature_aggro_target(session: &WorldSessionState) -> Option<ObjectGuid> {
    select_db_creature_aggro_targets(session).into_iter().next()
}

fn select_db_creature_aggro_targets(session: &WorldSessionState) -> Vec<ObjectGuid> {
    if session.player_death_state != PlayerDeathState::Alive {
        return Vec::new();
    }
    let Some(character) = session.active_character.as_ref() else {
        return Vec::new();
    };
    let mut targets = session
        .db_creatures
        .values()
        .filter(|creature| {
            !session
                .active_creature_combats
                .contains_key(&creature.guid().raw())
        })
        .filter(|creature| creature.can_aggro_player(character))
        .filter_map(|creature| {
            if !db_creature_navigation_check(
                &session.db_creature_navigation,
                creature.current_position,
                character.position,
            )
            .is_clear()
            {
                return None;
            }
            let distance_sq = creature.distance_to_player_squared(character)?;
            let attack_distance = db_creature_attack_distance(
                character.level,
                creature.spawn.template.min_level,
                creature.spawn.template.detection_range,
            );
            (distance_sq <= attack_distance * attack_distance).then_some((distance_sq, creature.guid()))
        })
        .collect::<Vec<_>>();
    targets.sort_by(|(left_distance, left_guid), (right_distance, right_guid)| {
            left_distance
                .total_cmp(right_distance)
                .then_with(|| left_guid.raw().cmp(&right_guid.raw()))
        });
    targets.into_iter().map(|(_, guid)| guid).collect()
}

#[cfg(test)]
fn select_db_creature_assist_targets(
    session: &mut WorldSessionState,
    caller_guid: ObjectGuid,
) -> Vec<ObjectGuid> {
    if session.player_death_state != PlayerDeathState::Alive {
        return Vec::new();
    }
    let Some(character) = session.active_character.as_ref() else {
        return Vec::new();
    };
    let Some(caller) = session.db_creatures.get_mut(&caller_guid.raw()) else {
        return Vec::new();
    };
    if caller.already_called_assistance {
        return Vec::new();
    }
    caller.already_called_assistance = true;
    let caller_position = caller.current_position;
    let caller_faction = caller.spawn.template.faction;
    let radius = if caller.spawn.template.call_for_help > 0 {
        caller.spawn.template.call_for_help as f32
    } else {
        DB_CREATURE_ASSISTANCE_RADIUS_YARDS
    };
    let mut targets = session
        .db_creatures
        .values()
        .filter(|creature| creature.guid() != caller_guid)
        .filter(|creature| {
            !session
                .active_creature_combats
                .contains_key(&creature.guid().raw())
        })
        .filter(|creature| creature.spawn.template.faction == caller_faction)
        .filter(|creature| creature.can_aggro_player(character))
        .filter_map(|creature| {
            let distance = distance_2d(
                caller_position.x,
                caller_position.y,
                creature.current_position.x,
                creature.current_position.y,
            );
            (distance <= radius).then_some((distance, creature.guid()))
        })
        .collect::<Vec<_>>();
    targets.sort_by(|(left_distance, left_guid), (right_distance, right_guid)| {
        left_distance
            .total_cmp(right_distance)
            .then_with(|| left_guid.raw().cmp(&right_guid.raw()))
    });
    targets.into_iter().map(|(_, guid)| guid).collect()
}

#[cfg(test)]
fn begin_db_creature_combat(
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
    now: Instant,
) -> bool {
    if session.player_death_state != PlayerDeathState::Alive {
        return false;
    }
    let Some(character) = &session.active_character else {
        return false;
    };
    let victim = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    if session
        .active_creature_combats
        .get(&attacker.raw())
        .is_some_and(|combat| combat.victim == victim)
    {
        return false;
    }
    session.active_creature_combats.insert(
        attacker.raw(),
        CreatureCombatState {
            attacker,
            victim,
            next_swing_at: now,
        },
    );
    true
}

async fn begin_shared_db_creature_combat(
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
    now: Instant,
) -> bool {
    if session.player_death_state != PlayerDeathState::Alive {
        return false;
    }
    let Some(character) = &session.active_character else {
        return false;
    };
    let Some(creature) = session.db_creatures.get(&attacker.raw()) else {
        return false;
    };
    if !creature.is_alive() {
        return false;
    }
    let victim = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let Some(combat) = shared_world
        .maps
        .begin_db_creature_combat(character.position.map_id, attacker, victim, now)
        .await
    else {
        return false;
    };
    session.active_creature_combats.insert(attacker.raw(), combat);
    true
}

fn clear_db_creature_combat_if_attacker(session: &mut WorldSessionState, attacker: ObjectGuid) {
    session.active_creature_combats.remove(&attacker.raw());
}

async fn send_player_combat_flag_if_changed(
    stream: &mut WorldPacketSink,
    session: &mut WorldSessionState,
    in_combat: bool,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if session.player_in_combat == in_combat {
        return Ok(());
    }
    let Some(character) = &session.active_character else {
        return Ok(());
    };
    session.player_in_combat = in_combat;
    let player = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_unit_flags_update_body(player, player_unit_flags(in_combat))?,
        Some(header_crypto),
    )
    .await
}

fn player_unit_flags(in_combat: bool) -> u32 {
    UNIT_FLAG_PLAYER_CONTROLLED | (if in_combat { UNIT_FLAG_IN_COMBAT } else { 0 })
}

fn db_creature_unit_flags(creature: &DbCreatureRuntime, in_combat: bool) -> u32 {
    creature.spawn.template.unit_flags | (if in_combat { UNIT_FLAG_IN_COMBAT } else { 0 })
}

