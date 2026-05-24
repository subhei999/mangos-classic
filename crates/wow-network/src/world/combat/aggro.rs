use super::*;
use wow_proto::world::WorldOpcode;

pub(in crate::world) async fn try_start_db_creature_aggro(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.clone() else {
        return Ok(());
    };
    let map_id = character.position.map_id;
    let player = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let sight_candidates = shared_world
        .maps
        .select_db_creature_sight_aggro_targets(map_id, &character)
        .await;
    for creature in sight_candidates {
        let attacker = creature.guid();
        mirror_session_db_creature(session, attacker.raw(), creature.clone());
        if !db_creature_navigation_check(
            &session.movement.db_creature_navigation,
            creature.current_position,
            character.position,
        )
        .is_clear()
        {
            continue;
        }
        begin_db_creature_combat_with_assistance(
            stream,
            shared_world,
            map_id,
            session,
            attacker,
            player,
            header_crypto,
        )
        .await?;
    }
    Ok(())
}

pub(in crate::world) async fn begin_db_creature_combat_with_assistance(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    map_id: u32,
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
    player: ObjectGuid,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    if !begin_shared_db_creature_combat(shared_world, session, attacker, Instant::now()).await {
        return Ok(false);
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

    let Some(character) = session.character.active_character.clone() else {
        return Ok(true);
    };
    let assistants = shared_world
        .maps
        .select_db_creature_assist_targets(map_id, attacker, &character)
        .await;
    let assistant_targets = if let Some((caller, assistants)) = assistants {
        mirror_session_db_creature(session, attacker.raw(), caller);
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
    Ok(true)
}

pub(in crate::world) async fn send_db_creature_combat_start(
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
        WorldOpcode::SmsgAttackStart as u16,
        &attack_start_body,
        Some(&mut *header_crypto),
    )
    .await?;
    broadcast_db_creature_packet(
        broadcast,
        session,
        attacker,
        WorldOpcode::SmsgAttackStart as u16,
        attack_start_body,
    )
    .await;
    send_player_combat_flag_if_changed(stream, session, true, header_crypto).await?;
    let creature_flags_body = shared_world
        .maps
        .db_creature_snapshot(map_id, attacker)
        .await
        .map(|creature| {
            build_unit_flags_update_body(attacker, db_creature_unit_flags(&creature, true))
        })
        .transpose()?;
    if let Some(creature_flags_body) = creature_flags_body {
        send_packet(
            stream,
            WorldOpcode::SmsgUpdateObject as u16,
            &creature_flags_body,
            Some(&mut *header_crypto),
        )
        .await?;
        broadcast_db_creature_packet(
            broadcast,
            session,
            attacker,
            WorldOpcode::SmsgUpdateObject as u16,
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

pub(in crate::world) async fn send_active_db_creature_attack(
    stream: &mut WorldPacketSink,
    context: ActiveDbCreatureAttackContext<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.clone() else {
        return Ok(());
    };
    let map_id = character.position.map_id;
    let character_guid = character.guid;
    let character_level = character.level;
    let player = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let combat_stats = context
        .shared_world
        .maps
        .player_combat_stats(map_id, character_guid)
        .await
        .ok_or_else(|| {
            anyhow::anyhow!(
                "map-owned player combat stats missing for character {}",
                character_guid
            )
        })?;
    let defense = player_melee_defense_input(
        &character,
        &combat_stats,
        &session.character.character_skills,
        &session.auras.active_auras,
    );
    let tick = context
        .shared_world
        .maps
        .advance_db_creature_combats_for_victim(
            context.world_db_pool,
            context.shared_world.object_mgr,
            map_id,
            player,
            context.session_id,
            defense,
            &session.movement.db_creature_navigation,
            Instant::now(),
        )
        .await?;
    if tick.player_in_combat {
        send_player_combat_flag_if_changed(stream, session, true, header_crypto).await?;
    } else {
        clear_session_active_creature_combats(session);
        send_player_combat_flag_if_changed(stream, session, false, header_crypto).await?;
    }
    mirror_session_active_creature_combats(session, &tick.active_combats);
    for effect in tick.local_effects {
        match effect {
            DbCreatureVictimCombatLocalEffect::SpellDamage {
                victim_health,
                player_died,
            } => {
                session.character.player_health = victim_health;
                if player_died {
                    refresh_session_from_map_owned_player_death(
                        context.shared_world.maps,
                        map_id,
                        session,
                    )
                    .await;
                }
            }
            DbCreatureVictimCombatLocalEffect::Melee {
                attacker,
                damage_taken,
                victim_health,
                aura_changed,
                rage_gain,
                player_died,
            } => {
                session.character.player_health = victim_health;
                if aura_changed {
                    if let Some(snapshot) = context
                        .shared_world
                        .maps
                        .player_runtime_snapshot(map_id, character_guid)
                        .await
                    {
                        session.auras.active_auras = snapshot.active_auras;
                    }
                }
                let advanced_skill = try_advance_combat_skill_value(
                    character_level,
                    SKILL_DEFENSE,
                    combat_stats.intellect,
                    false,
                    &mut session.character.character_skills,
                );
                if let Some(updated) = advanced_skill {
                    wow_db::upsert_character_skill(
                        context.character_db_pool,
                        character_guid,
                        updated.skill,
                        updated.value,
                        updated.max,
                    )
                    .await?;
                    send_packet(
                        stream,
                        WorldOpcode::SmsgUpdateObject as u16,
                        &build_player_skill_update_body(
                            character_guid,
                            updated,
                            &session.auras.active_auras,
                        )?,
                        Some(&mut *header_crypto),
                    )
                    .await?;
                }
                if rage_gain > 0 {
                    session.character.player_rage = session
                        .character
                        .player_rage
                        .saturating_add(rage_gain)
                        .min(POWER_RAGE_DEFAULT);
                    context
                        .shared_world
                        .maps
                        .set_player_power2(map_id, character_guid, session.character.player_rage)
                        .await;
                    send_packet(
                        stream,
                        WorldOpcode::SmsgUpdateObject as u16,
                        &build_player_rage_update_body(player, session.character.player_rage)?,
                        Some(&mut *header_crypto),
                    )
                    .await?;
                }
                interrupt_player_consumable_auras(
                    stream,
                    context.shared_world.maps,
                    context.shared_world.sessions,
                    session,
                    AURA_INTERRUPT_FLAG_DAMAGE,
                    header_crypto,
                )
                .await?;
                if damage_taken > 0 {
                    let opening_cancelled = cancel_pending_opening_spell_cast(
                        stream,
                        context.shared_world.maps,
                        context.shared_world.sessions,
                        session,
                        SPELL_FAILED_INTERRUPTED,
                        header_crypto,
                    )
                    .await?;
                    apply_player_taken_melee_proc_auras(
                        stream,
                        SpellCastDeps {
                            character_db_pool: context.character_db_pool,
                            world_db_pool: context.world_db_pool,
                            account_id: 0,
                            shared_world: context.shared_world,
                            parties: context.parties,
                        },
                        session,
                        map_id,
                        character_guid,
                        player,
                        attacker,
                        Instant::now(),
                        header_crypto,
                    )
                    .await?;
                    if !opening_cancelled {
                        if let Some(channel_event) = context
                            .shared_world
                            .maps
                            .interrupt_active_player_channel_for_damage(
                                map_id,
                                character_guid,
                                Instant::now(),
                            )
                            .await?
                        {
                            context
                                .shared_world
                                .sessions
                                .dispatch(channel_event.observer_packets)
                                .await;
                            for packet in channel_event.direct_packets {
                                send_packet(
                                    stream,
                                    packet.opcode,
                                    &packet.body,
                                    Some(&mut *header_crypto),
                                )
                                .await?;
                            }
                        }
                        if !interrupt_player_spell_cast_for_damage(
                            stream,
                            context.shared_world.maps,
                            context.shared_world.sessions,
                            session,
                            header_crypto,
                        )
                        .await?
                        {
                            if let Some(delay_millis) = context
                                .shared_world
                                .maps
                                .delay_active_player_spell_cast_for_damage(
                                    map_id,
                                    character_guid,
                                    Instant::now(),
                                )
                                .await
                            {
                                send_packet(
                                    stream,
                                    WorldOpcode::SmsgSpellDelayed as u16,
                                    &build_spell_delayed_body(player, delay_millis)?,
                                    Some(&mut *header_crypto),
                                )
                                .await?;
                            }
                        }
                    }
                }
                if player_died {
                    refresh_session_from_map_owned_player_death(
                        context.shared_world.maps,
                        map_id,
                        session,
                    )
                    .await;
                }
            }
        }
    }
    context
        .shared_world
        .sessions
        .dispatch(tick.observer_packets)
        .await;
    for packet in tick.direct_packets {
        send_packet(
            stream,
            packet.opcode,
            &packet.body,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub(in crate::world) struct ActiveDbCreatureAttackContext<'a> {
    pub(in crate::world) character_db_pool: &'a MySqlPool,
    pub(in crate::world) world_db_pool: &'a MySqlPool,
    pub(in crate::world) shared_world: SharedWorldDeps<'a>,
    pub(in crate::world) parties: &'a PartyManager,
    pub(in crate::world) session_id: SessionId,
}

pub(in crate::world) async fn load_db_creature_spell_condition_cache(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    spell_list: &[wow_db::CreatureSpellListQuery],
) -> anyhow::Result<DbCreatureSpellConditionCache> {
    let mut cache = DbCreatureSpellConditionCache::default();
    for spell in spell_list {
        if spell.target_unit_condition > 0 {
            if let Some(condition) = object_mgr
                .unit_condition(world_db_pool, spell.target_unit_condition)
                .await?
            {
                cache.unit_conditions.insert(condition.id, condition);
            }
        }
        if spell.combat_condition > 0 {
            if let Some(condition) = object_mgr
                .combat_condition(world_db_pool, spell.combat_condition)
                .await?
            {
                collect_db_creature_spell_unit_condition(
                    object_mgr,
                    world_db_pool,
                    condition.self_condition_id,
                    &mut cache,
                )
                .await?;
                collect_db_creature_spell_unit_condition(
                    object_mgr,
                    world_db_pool,
                    condition.target_condition_id,
                    &mut cache,
                )
                .await?;
                collect_db_creature_spell_unit_condition(
                    object_mgr,
                    world_db_pool,
                    condition.friend_condition_id_0,
                    &mut cache,
                )
                .await?;
                collect_db_creature_spell_unit_condition(
                    object_mgr,
                    world_db_pool,
                    condition.friend_condition_id_1,
                    &mut cache,
                )
                .await?;
                collect_db_creature_spell_unit_condition(
                    object_mgr,
                    world_db_pool,
                    condition.enemy_condition_id_0,
                    &mut cache,
                )
                .await?;
                collect_db_creature_spell_unit_condition(
                    object_mgr,
                    world_db_pool,
                    condition.enemy_condition_id_1,
                    &mut cache,
                )
                .await?;
                cache.combat_conditions.insert(condition.id, condition);
            }
        }
    }
    Ok(cache)
}

pub(in crate::world) async fn collect_db_creature_spell_unit_condition(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    id: i32,
    cache: &mut DbCreatureSpellConditionCache,
) -> anyhow::Result<()> {
    if id == 0 || cache.unit_conditions.contains_key(&id) {
        return Ok(());
    }
    if let Some(condition) = object_mgr.unit_condition(world_db_pool, id).await? {
        cache.unit_conditions.insert(condition.id, condition);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_taken_melee_proc_auras(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    map_id: u32,
    character_guid: u32,
    player: ObjectGuid,
    attacker: ObjectGuid,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let trigger_spell_ids = active_aura_proc_trigger_spell_ids(
        &mut session.auras.active_auras,
        PROC_FLAG_TAKE_MELEE_SWING,
        now,
    );
    if trigger_spell_ids.is_empty() {
        return Ok(());
    }

    let character_level = session
        .character
        .active_character
        .as_ref()
        .map(|character| character.level)
        .unwrap_or(1);
    let targets = SpellCastTargets {
        target_mask: SPELL_CAST_TARGET_UNIT,
        unit_target: Some(attacker),
        gameobject_target: None,
        source_location: None,
        destination: None,
    };

    for spell_id in trigger_spell_ids {
        apply_player_trigger_spell_by_id(
            stream,
            deps,
            session,
            player,
            character_guid,
            character_level,
            map_id,
            spell_id,
            &targets,
            now,
            header_crypto,
        )
        .await?;
    }

    Ok(())
}

pub(in crate::world) async fn refresh_session_from_map_owned_player_death(
    maps: &Arc<MapRuntimeManager>,
    map_id: u32,
    session: &mut WorldSessionState,
) {
    let Some(character) = session.character.active_character.as_ref() else {
        return;
    };
    let Some(snapshot) = maps.player_runtime_snapshot(map_id, character.guid).await else {
        return;
    };
    if snapshot.health != 0 || snapshot.death_state == PlayerDeathState::Alive {
        return;
    }
    session.character.player_health = 0;
    session.death.player_death_state = snapshot.death_state;
    session.death.player_death_presentation_pending =
        snapshot.death_state == PlayerDeathState::JustDied;
    session.character.player_stand_state = snapshot.stand_state;
    session.auras.active_auras = snapshot.active_auras;
    session.character.player_flags = snapshot.flags;
    session.combat.player_in_combat = false;
    mark_player_auto_repop_if_corpse(session, Instant::now());
    mirror_session_player_auto_attack(session, None, None);
    clear_session_active_creature_combats(session);
    if let Some(character) = session.character.active_character.as_mut() {
        character.position = snapshot.position;
        character.movement_flags = snapshot.movement_flags;
        character.client_time = snapshot.client_time;
        character.fall_time = snapshot.fall_time;
        character.jump = snapshot.jump;
    }
}

#[allow(dead_code)]
pub(in crate::world) async fn defer_ready_db_creature_swing_retry(
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
        mirror_session_active_creature_combat(session, combat);
    }
}

pub(in crate::world) async fn send_db_creature_threat_target_switch(
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
        .character
        .active_character
        .as_ref()
        .map(|character| ObjectGuid::new(HighGuid::Player, 0, character.guid));
    if current_player == Some(event.old_victim) {
        remove_session_active_creature_combat(session, event.attacker);
    }
    if current_player == Some(event.new_victim) {
        mirror_session_active_creature_combat(session, event.combat);
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
pub(in crate::world) fn select_db_creature_aggro_target(
    session: &WorldSessionState,
) -> Option<ObjectGuid> {
    select_db_creature_aggro_targets(session).into_iter().next()
}

#[cfg(test)]
pub(in crate::world) fn select_db_creature_aggro_targets(
    session: &WorldSessionState,
) -> Vec<ObjectGuid> {
    if session.death.player_death_state != PlayerDeathState::Alive {
        return Vec::new();
    }
    let Some(character) = session.character.active_character.as_ref() else {
        return Vec::new();
    };
    let faction_templates = FactionTemplateStore::fallback_bridge();
    let now = Instant::now();
    let mut targets = session
        .visibility
        .db_creatures
        .values()
        .filter(|creature| {
            !session
                .combat
                .active_creature_combats
                .contains_key(&creature.guid().raw())
        })
        .filter(|creature| creature.can_aggro_player(&faction_templates, character, now))
        .filter_map(|creature| {
            let distance_sq = creature.distance_to_player_squared(character)?;
            let attack_distance = db_creature_attack_distance(
                character.level,
                creature.spawn.template.min_level,
                creature.spawn.template.detection_range,
            );
            if distance_sq > attack_distance * attack_distance {
                return None;
            }
            if !db_creature_navigation_check(
                &session.movement.db_creature_navigation,
                creature.current_position,
                character.position,
            )
            .is_clear()
            {
                return None;
            }
            Some((distance_sq, creature.guid()))
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
pub(in crate::world) fn select_db_creature_assist_targets(
    session: &mut WorldSessionState,
    caller_guid: ObjectGuid,
) -> Vec<ObjectGuid> {
    if session.death.player_death_state != PlayerDeathState::Alive {
        return Vec::new();
    }
    let Some(character) = session.character.active_character.as_ref() else {
        return Vec::new();
    };
    let Some(caller) = session.visibility.db_creatures.get_mut(&caller_guid.raw()) else {
        return Vec::new();
    };
    if caller.already_called_assistance {
        return Vec::new();
    }
    caller.already_called_assistance = true;
    let caller_position = caller.current_position;
    let caller_faction = caller.spawn.template.faction;
    let faction_templates = FactionTemplateStore::fallback_bridge();
    let now = Instant::now();
    let radius = if caller.spawn.template.call_for_help > 0 {
        caller.spawn.template.call_for_help as f32
    } else {
        DB_CREATURE_ASSISTANCE_RADIUS_YARDS
    };
    let mut targets = session
        .visibility
        .db_creatures
        .values()
        .filter(|creature| creature.guid() != caller_guid)
        .filter(|creature| {
            !session
                .combat
                .active_creature_combats
                .contains_key(&creature.guid().raw())
        })
        .filter(|creature| creature.spawn.template.faction == caller_faction)
        .filter(|creature| creature.can_aggro_player(&faction_templates, character, now))
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
pub(in crate::world) fn begin_db_creature_combat(
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
    now: Instant,
) -> bool {
    if session.death.player_death_state != PlayerDeathState::Alive {
        return false;
    }
    let Some(character) = &session.character.active_character else {
        return false;
    };
    let victim = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    if session
        .combat
        .active_creature_combats
        .get(&attacker.raw())
        .is_some_and(|combat| combat.victim == victim)
    {
        return false;
    }
    session.combat.active_creature_combats.insert(
        attacker.raw(),
        CreatureCombatState {
            attacker,
            victim,
            started_at: now,
            next_swing_at: now,
        },
    );
    true
}

pub(in crate::world) async fn begin_shared_db_creature_combat(
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
    now: Instant,
) -> bool {
    if session.death.player_death_state != PlayerDeathState::Alive {
        return false;
    }
    let Some(character) = &session.character.active_character else {
        return false;
    };
    let victim = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let Some((combat, creature)) = shared_world
        .maps
        .begin_db_creature_combat(character.position.map_id, attacker, victim, now)
        .await
    else {
        return false;
    };
    mirror_session_db_creature(session, attacker.raw(), creature);
    mirror_session_active_creature_combat(session, combat);
    true
}

pub(in crate::world) fn clear_db_creature_combat_if_attacker(
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
) {
    remove_session_active_creature_combat(session, attacker);
}

pub(in crate::world) async fn send_player_combat_flag_if_changed(
    stream: &mut WorldPacketSink,
    session: &mut WorldSessionState,
    in_combat: bool,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if session.combat.player_in_combat == in_combat {
        return Ok(());
    }
    let Some(character) = &session.character.active_character else {
        return Ok(());
    };
    session.combat.player_in_combat = in_combat;
    let player = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &build_unit_flags_update_body(player, player_unit_flags(in_combat))?,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) fn player_unit_flags(in_combat: bool) -> u32 {
    player_unit_flags_with_looting(in_combat, false)
}

pub(in crate::world) fn player_unit_flags_with_looting(in_combat: bool, looting: bool) -> u32 {
    player_unit_flags_with_looting_and_auras(in_combat, looting, &[])
}

pub(in crate::world) fn player_unit_flags_with_looting_and_auras(
    in_combat: bool,
    looting: bool,
    active_auras: &[ActiveAura],
) -> u32 {
    UNIT_FLAG_PLAYER_CONTROLLED
        | (if looting { UNIT_FLAG_LOOTING } else { 0 })
        | (if in_combat { UNIT_FLAG_IN_COMBAT } else { 0 })
        | (if active_aura_has_disarm(active_auras) {
            UNIT_FLAG_DISARMED
        } else {
            0
        })
}

pub(in crate::world) fn db_creature_unit_flags(
    creature: &DbCreatureRuntime,
    in_combat: bool,
) -> u32 {
    creature.spawn.template.unit_flags
        | (if in_combat { UNIT_FLAG_IN_COMBAT } else { 0 })
        | (if active_aura_has_stun(&creature.active_auras) {
            UNIT_FLAG_STUNNED
        } else {
            0
        })
        | (if active_aura_has_confuse(&creature.active_auras) {
            UNIT_FLAG_CONFUSED
        } else {
            0
        })
        | (if creature.is_fleeing() {
            UNIT_FLAG_FLEEING
        } else {
            0
        })
}
