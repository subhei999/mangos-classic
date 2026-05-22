use super::*;
use wow_proto::world::WorldOpcode;

#[cfg(test)]
pub(in crate::world) fn apply_db_creature_damage(
    session: &mut WorldSessionState,
    target: ObjectGuid,
    requested_damage: u32,
) -> Option<u32> {
    let creature = session.visibility.db_creatures.get_mut(&target.raw())?;
    if !creature.is_alive() || creature.is_evading_home() {
        return None;
    }

    let damage = creature.health.min(requested_damage.max(1));
    creature.health = creature.health.saturating_sub(damage);
    if creature.health == 0 {
        creature.begin_corpse(Instant::now(), current_unix_epoch_secs());
        session.combat.active_combat_target = None;
        session.combat.active_combat_next_swing_at = None;
        clear_db_creature_combat_if_attacker(session, target);
    }
    Some(damage)
}

pub(in crate::world) async fn handle_combat_tick(
    stream: &mut WorldPacketSink,
    deps: CombatTickDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let now = Instant::now();
    if session.death.player_death_state != PlayerDeathState::Alive {
        return Ok(());
    }
    if let Some(character) = session.character.active_character.as_ref() {
        if let Some(due) = deps
            .shared_world
            .maps
            .player_auto_attack_due(character.position.map_id, character.guid, now)
            .await
        {
            match due.kind {
                PlayerAutoAttackKind::Melee => {
                    send_db_creature_swing(
                        stream,
                        CombatRewardDeps {
                            character_db_pool: deps.character_db_pool,
                            world_db_pool: deps.world_db_pool,
                            shared_world: deps.shared_world,
                            parties: deps.parties,
                        },
                        session,
                        header_crypto,
                        due.target,
                    )
                    .await?;
                }
                PlayerAutoAttackKind::Ranged { spell_id, .. } => {
                    send_db_creature_ranged_swing(
                        stream,
                        CombatRewardDeps {
                            character_db_pool: deps.character_db_pool,
                            world_db_pool: deps.world_db_pool,
                            shared_world: deps.shared_world,
                            parties: deps.parties,
                        },
                        session,
                        header_crypto,
                        due.target,
                        spell_id,
                    )
                    .await?;
                }
            }
        }
    }

    try_start_db_creature_aggro(stream, deps.shared_world, session, header_crypto).await?;
    send_active_db_creature_attack(
        stream,
        ActiveDbCreatureAttackContext {
            character_db_pool: deps.character_db_pool,
            world_db_pool: deps.world_db_pool,
            shared_world: deps.shared_world,
            session_id: deps.session_id,
        },
        session,
        header_crypto,
    )
    .await
}

#[derive(Clone, Copy)]
pub(in crate::world) struct CombatTickDeps<'a> {
    pub(in crate::world) character_db_pool: &'a MySqlPool,
    pub(in crate::world) world_db_pool: &'a MySqlPool,
    pub(in crate::world) shared_world: SharedWorldDeps<'a>,
    pub(in crate::world) parties: &'a PartyManager,
    pub(in crate::world) session_id: SessionId,
}

#[derive(Clone, Copy)]
pub(in crate::world) struct CombatRewardDeps<'a> {
    pub(in crate::world) character_db_pool: &'a MySqlPool,
    pub(in crate::world) world_db_pool: &'a MySqlPool,
    pub(in crate::world) shared_world: SharedWorldDeps<'a>,
    pub(in crate::world) parties: &'a PartyManager,
}

pub(in crate::world) async fn send_queued_next_melee_spell_cast_failure(
    stream: &mut WorldPacketSink,
    header_crypto: &mut HeaderCrypto,
    caster: ObjectGuid,
    queued: QueuedNextMeleeSpell,
    failure: u8,
) -> anyhow::Result<()> {
    send_packet(
        stream,
        WorldOpcode::SmsgCastResult as u16,
        &build_cast_result_failure_body(queued.spell_id, failure),
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgSpellFailure as u16,
        &build_spell_failure_body(caster, queued.spell_id, failure)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgSpellFailedOther as u16,
        &build_spell_failed_other_body(caster, queued.spell_id),
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn send_queued_next_melee_spell_cast_success(
    stream: &mut WorldPacketSink,
    header_crypto: &mut HeaderCrypto,
    queued: QueuedNextMeleeSpell,
) -> anyhow::Result<()> {
    send_packet(
        stream,
        WorldOpcode::SmsgCastResult as u16,
        &build_cast_result_ok_body(queued.spell_id),
        Some(header_crypto),
    )
    .await
}

#[cfg(test)]
pub(in crate::world) async fn sync_session_db_creatures_from_map(
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
) {
    let Some(character) = session.character.active_character.as_ref() else {
        return;
    };
    let guids = session
        .visibility
        .db_creatures
        .keys()
        .copied()
        .collect::<Vec<_>>();
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
            .visibility
            .db_creatures
            .get(&guid)
            .map(|creature| creature.client_visible)
            .unwrap_or(shared.client_visible);
        let mut shared = shared;
        shared.client_visible = client_visible && shared.life_state != DbCreatureLifeState::Dead;
        mirror_session_db_creature(session, guid, shared);
    }
}

#[cfg(test)]
pub(in crate::world) async fn advance_db_creature_return_home_motions(
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    now: Instant,
) {
    let Some(character) = session.character.active_character.as_ref().cloned() else {
        return;
    };
    let map_id = character.position.map_id;
    let visible_guids = shared_world
        .maps
        .player_visible_db_creature_guids(map_id, character.guid)
        .await;
    #[cfg(test)]
    let visible_guids = if visible_guids.is_empty() {
        session.visibility.db_creatures.keys().copied().collect()
    } else {
        visible_guids
    };
    let return_home_guids = shared_world
        .maps
        .db_creature_return_home_guids(map_id, &visible_guids)
        .await;
    for guid in return_home_guids {
        let advanced = shared_world
            .maps
            .advance_db_creature_motion(map_id, ObjectGuid::from_raw(guid), now)
            .await;
        #[cfg(test)]
        if let Some(creature) = advanced {
            mirror_session_db_creature(session, guid, creature);
        }
        #[cfg(not(test))]
        let _ = advanced;
    }
}

pub(in crate::world) async fn advance_db_creature_motion_and_share(
    shared_world: SharedWorldDeps<'_>,
    map_id: u32,
    _session: &mut WorldSessionState,
    creature_guid: ObjectGuid,
    now: Instant,
) {
    let advanced = shared_world
        .maps
        .advance_db_creature_motion(map_id, creature_guid, now)
        .await;
    #[cfg(test)]
    if let Some(creature) = advanced {
        mirror_session_db_creature(_session, creature_guid.raw(), creature);
    }
    #[cfg(not(test))]
    let _ = advanced;
}

#[cfg(test)]
#[allow(dead_code)]
pub(in crate::world) fn should_advance_db_creature_idle_motion(
    session: &WorldSessionState,
    guid: u64,
    creature: &DbCreatureRuntime,
) -> bool {
    creature.is_alive()
        && !session.combat.active_creature_combats.contains_key(&guid)
        && session
            .combat
            .active_combat_target
            .is_none_or(|target| target.raw() != guid)
        && matches!(
            creature.motion,
            CreatureMotionState::Random(_)
                | CreatureMotionState::Confused(_)
                | CreatureMotionState::Waypoint(_)
        )
}

#[cfg(test)]
pub(in crate::world) fn db_creature_idle_motion_start_guids(
    session: &WorldSessionState,
    now: Instant,
) -> Vec<u64> {
    let mut guids = session
        .visibility
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
pub(in crate::world) fn should_start_db_creature_idle_motion(
    session: &WorldSessionState,
    guid: u64,
    creature: &DbCreatureRuntime,
    now: Instant,
) -> bool {
    creature.is_alive()
        && !session.combat.active_creature_combats.contains_key(&guid)
        && session
            .combat
            .active_combat_target
            .is_none_or(|target| target.raw() != guid)
        && matches!(creature.motion, CreatureMotionState::Idle)
        && !active_aura_has_confuse(&creature.active_auras)
        && (creature.next_random_move_at.is_some_and(|at| now >= at)
            || creature.next_waypoint_move_at.is_some_and(|at| now >= at))
}

pub(in crate::world) async fn send_db_creature_swing(
    stream: &mut WorldPacketSink,
    deps: CombatRewardDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
    target: ObjectGuid,
) -> anyhow::Result<()> {
    let character_db_pool = deps.character_db_pool;
    let world_db_pool = deps.world_db_pool;
    let shared_world = deps.shared_world;
    let parties = deps.parties;
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let map_id = character.position.map_id;
    advance_db_creature_motion_and_share(shared_world, map_id, session, target, Instant::now())
        .await;

    let Some(character) = &session.character.active_character else {
        return Ok(());
    };
    let character_snapshot = character.clone();
    let attacker = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let combat_stats = shared_world
        .maps
        .player_combat_stats(map_id, character_snapshot.guid)
        .await
        .ok_or_else(|| {
            anyhow::anyhow!(
                "map-owned player combat stats missing for character {}",
                character_snapshot.guid
            )
        })?;
    match db_creature_player_melee_check_from_map(shared_world, session, target).await {
        PlayerMeleeCheck::Clear => {
            session.combat.last_player_melee_swing_error = None;
        }
        PlayerMeleeCheck::TargetEvading => {
            session.combat.last_player_melee_swing_error = None;
            let next_swing_at = Some(player_main_hand_next_swing_at(
                Instant::now(),
                &combat_stats,
            ));
            mirror_session_player_next_swing_at(session, next_swing_at);
            shared_world
                .maps
                .set_player_next_swing_at(map_id, character_snapshot.guid, next_swing_at)
                .await;
            send_packet(
                stream,
                WorldOpcode::SmsgAttackerStateUpdate as u16,
                &build_attacker_state_update_body_for_outcome(
                    attacker,
                    target,
                    MeleeDamageOutcome::evade(),
                    0,
                )?,
                Some(header_crypto),
            )
            .await?;
            return Ok(());
        }
        PlayerMeleeCheck::MissingTarget | PlayerMeleeCheck::TargetNotAlive => {
            send_player_melee_swing_error_if_changed(
                stream,
                session,
                PlayerMeleeSwingError::DeadTarget,
                header_crypto,
            )
            .await?;
            mirror_session_player_auto_attack(session, None, None);
            shared_world
                .maps
                .set_player_auto_attack(map_id, character_snapshot.guid, None, None)
                .await;
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
            let next_swing_at = Some(player_melee_retry_at(Instant::now()));
            mirror_session_player_next_swing_at(session, next_swing_at);
            shared_world
                .maps
                .set_player_next_swing_at(map_id, character_snapshot.guid, next_swing_at)
                .await;
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
            let next_swing_at = Some(player_melee_retry_at(Instant::now()));
            mirror_session_player_next_swing_at(session, next_swing_at);
            shared_world
                .maps
                .set_player_next_swing_at(map_id, character_snapshot.guid, next_swing_at)
                .await;
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
            let next_swing_at = Some(player_melee_retry_at(Instant::now()));
            mirror_session_player_next_swing_at(session, next_swing_at);
            shared_world
                .maps
                .set_player_next_swing_at(map_id, character_snapshot.guid, next_swing_at)
                .await;
            return Ok(());
        }
    }
    let Some(target_creature) = shared_world.maps.db_creature_snapshot(map_id, target).await else {
        mirror_session_player_auto_attack(session, None, None);
        shared_world
            .maps
            .set_player_auto_attack(map_id, character_snapshot.guid, None, None)
            .await;
        return Ok(());
    };
    let weapon_skill_id =
        main_hand_weapon_skill_id(world_db_pool, &session.inventory.items).await?;
    let attacker_skill = weapon_skill_id
        .map(|skill_id| {
            current_skill_value_with_active_auras(
                &session.character.character_skills,
                &session.auras.active_auras,
                skill_id,
            )
        })
        .unwrap_or(0);
    let mut melee_outcome = player_main_hand_melee_outcome_against_db_creature(
        &combat_stats,
        character_snapshot.level,
        attacker_skill,
        &target_creature,
    );
    let mut queued_spell = shared_world
        .maps
        .queued_player_next_melee_spell(map_id, character_snapshot.guid, target)
        .await;
    if let Some(queued) = queued_spell {
        let requires_main_hand_weapon = shared_world
            .object_mgr
            .spell_template(world_db_pool, queued.spell_id)
            .await?
            .is_some_and(|template| queued_spell_requires_main_hand_weapon(&template));
        if requires_main_hand_weapon
            && !has_main_hand_weapon_for_attack(world_db_pool, &session.inventory.items).await?
        {
            shared_world
                .maps
                .clear_player_next_melee_spell(map_id, character_snapshot.guid)
                .await;
            send_queued_next_melee_spell_cast_failure(
                stream,
                header_crypto,
                attacker,
                queued,
                SPELL_FAILED_EQUIPPED_ITEM_CLASS_MAINHAND,
            )
            .await?;
            queued_spell = None;
        }
    }
    if let Some(queued) = queued_spell {
        let has_power = shared_world
            .maps
            .player_runtime_snapshot(map_id, character_snapshot.guid)
            .await
            .is_some_and(|snapshot| {
                snapshot.power2 >= queued.rage_cost && snapshot.power1 >= queued.mana_cost
            });
        if !has_power {
            shared_world
                .maps
                .clear_player_next_melee_spell(map_id, character_snapshot.guid)
                .await;
            send_queued_next_melee_spell_cast_failure(
                stream,
                header_crypto,
                attacker,
                queued,
                SPELL_FAILED_NO_POWER,
            )
            .await?;
            queued_spell = None;
        }
    }
    if let Some(queued) = queued_spell {
        melee_outcome = melee_outcome.with_next_melee_spell_bonus(queued.bonus_damage);
    }
    let requested_damage = melee_outcome.total_damage;
    let swing_time = Instant::now();
    let next_swing = player_main_hand_next_swing_at(swing_time, &combat_stats);
    let corpse_loot = if requested_damage >= target_creature.health {
        Some(
            prepare_db_creature_corpse_loot(
                shared_world.object_mgr,
                world_db_pool,
                parties,
                session,
                character_snapshot.guid,
                target_creature.spawn.entry,
            )
            .await?,
        )
    } else {
        None
    };
    let Some(event) = shared_world
        .maps
        .apply_db_creature_damage(
            character_snapshot.position.map_id,
            DbCreatureDamageRequest {
                creature_guid: target,
                killer: attacker,
                damage: requested_damage,
                melee_outcome: Some(melee_outcome),
                spell_damage_outcome: None,
                spell_id: queued_spell.map(|queued| queued.spell_id),
                spell_school: 0,
                suppress_attacker_state: queued_spell.is_some(),
                now: swing_time,
                now_epoch_secs: current_unix_epoch_secs(),
                exclude_character_guid: Some(character_snapshot.guid),
                corpse_loot,
            },
        )
        .await?
    else {
        mirror_session_player_auto_attack(session, None, None);
        shared_world
            .maps
            .set_player_auto_attack(map_id, character_snapshot.guid, None, None)
            .await;
        return Ok(());
    };
    let death_finalization = event.death_finalization;
    let target_switch = event.target_switch;
    let is_dead = death_finalization.is_some();
    let mut queued_rage_update_sent = false;
    if let Some(queued) = queued_spell {
        if let Err(failure) = shared_world
            .maps
            .spend_queued_player_next_melee_spell_power(map_id, character_snapshot.guid, queued)
            .await
        {
            send_queued_next_melee_spell_cast_failure(
                stream,
                header_crypto,
                attacker,
                queued,
                failure,
            )
            .await?;
            queued_spell = None;
        } else {
            if let Some(snapshot) = shared_world
                .maps
                .player_runtime_snapshot(map_id, character_snapshot.guid)
                .await
            {
                session.character.player_mana = snapshot.power1;
                session.character.player_rage = snapshot.power2;
                session.character.player_energy = snapshot.power4;
            }
            send_packet(
                stream,
                WorldOpcode::SmsgUpdateObject as u16,
                &build_player_rage_update_body(attacker, session.character.player_rage)?,
                Some(&mut *header_crypto),
            )
            .await?;
            queued_rage_update_sent = true;
        }
    }
    let mut advanced_skill = None;
    if let Some(skill_id) = weapon_skill_id {
        advanced_skill = try_advance_combat_skill_value(
            character_snapshot.level,
            skill_id,
            combat_stats.intellect,
            true,
            &mut session.character.character_skills,
        );
        if let Some(updated) = advanced_skill {
            wow_db::upsert_character_skill(
                character_db_pool,
                character_snapshot.guid,
                updated.skill,
                updated.value,
                updated.max,
            )
            .await?;
        }
    }
    mirror_session_db_creature(session, target.raw(), event.creature.clone());
    if is_dead {
        mirror_session_player_auto_attack(session, None, Some(next_swing));
        clear_db_creature_combat_if_attacker(session, target);
        shared_world
            .maps
            .set_player_auto_attack(map_id, character_snapshot.guid, None, Some(next_swing))
            .await;
    } else {
        mirror_session_player_next_swing_at(session, Some(next_swing));
        shared_world
            .maps
            .set_player_next_swing_at(map_id, character_snapshot.guid, Some(next_swing))
            .await;
    }
    let rage_gain = if queued_spell.is_some() {
        0
    } else {
        rage_gain_from_main_hand_white_damage(
            event.attacker_rage_damage,
            character_snapshot.level,
            combat_stats.main_attack_time_ms,
            melee_outcome.outcome,
        )
    };
    session.character.player_rage = session
        .character
        .player_rage
        .saturating_add(rage_gain)
        .min(POWER_RAGE_DEFAULT);
    shared_world
        .maps
        .set_player_power2(
            map_id,
            character_snapshot.guid,
            session.character.player_rage,
        )
        .await;
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
        try_process_db_creature_event_ai_hp_actions(
            stream,
            shared_world,
            world_db_pool,
            session,
            map_id,
            target,
            attacker,
            Instant::now(),
            header_crypto,
        )
        .await?;
    }

    if let Some(queued) = queued_spell {
        let targets = SpellCastTargets {
            target_mask: SPELL_CAST_TARGET_UNIT,
            unit_target: Some(target),
            gameobject_target: None,
            source_location: None,
            destination: None,
        };
        let spell_go_body = build_spell_go_body(attacker, queued.spell_id, &targets)?;
        send_queued_next_melee_spell_cast_success(stream, header_crypto, queued).await?;
        send_packet(
            stream,
            WorldOpcode::SmsgSpellGo as u16,
            &spell_go_body,
            Some(&mut *header_crypto),
        )
        .await?;
        let observer_packets = shared_world
            .maps
            .broadcast_nearby_player_packet(
                map_id,
                character_snapshot.guid,
                PLAYER_VISIBILITY_RADIUS_YARDS,
                OutboundWorldPacket {
                    opcode: WorldOpcode::SmsgSpellGo as u16,
                    body: spell_go_body,
                },
            )
            .await;
        shared_world.sessions.dispatch(observer_packets).await;
        if let Some(spell_non_melee_log_body) = &event.spell_non_melee_log_body {
            send_packet(
                stream,
                WorldOpcode::SmsgSpellNonMeleeDamageLog as u16,
                spell_non_melee_log_body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
        if let Some(spell_miss_log_body) = &event.spell_miss_log_body {
            send_packet(
                stream,
                WorldOpcode::SmsgSpellLogMiss as u16,
                spell_miss_log_body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
    } else if let Some(attacker_state_body) = &event.attacker_state_body {
        send_packet(
            stream,
            WorldOpcode::SmsgAttackerStateUpdate as u16,
            attacker_state_body,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    let creature_update_body = event.update_body.clone();
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &creature_update_body,
        Some(&mut *header_crypto),
    )
    .await?;
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
    if !is_dead {
        send_db_creature_threat_target_switch(
            stream,
            shared_world,
            session,
            target_switch,
            header_crypto,
        )
        .await?;
        try_process_db_creature_event_ai_hp_actions(
            stream,
            shared_world,
            world_db_pool,
            session,
            map_id,
            target,
            attacker,
            Instant::now(),
            header_crypto,
        )
        .await?;
    }
    if !queued_rage_update_sent {
        send_packet(
            stream,
            WorldOpcode::SmsgUpdateObject as u16,
            &build_player_rage_update_body(attacker, session.character.player_rage)?,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    if let Some(updated) = advanced_skill {
        send_packet(
            stream,
            WorldOpcode::SmsgUpdateObject as u16,
            &build_player_skill_update_body(
                character_snapshot.guid,
                updated,
                &session.auras.active_auras,
            )?,
            Some(&mut *header_crypto),
        )
        .await?;
    }

    if is_dead {
        finalize_db_creature_death(
            stream,
            CombatRewardDeps {
                character_db_pool,
                world_db_pool,
                shared_world,
                parties,
            },
            session,
            death_finalization,
            header_crypto,
        )
        .await?;
    }

    Ok(())
}

pub(in crate::world) async fn send_db_creature_ranged_swing(
    stream: &mut WorldPacketSink,
    deps: CombatRewardDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
    target: ObjectGuid,
    spell_id: u32,
) -> anyhow::Result<()> {
    let character_db_pool = deps.character_db_pool;
    let world_db_pool = deps.world_db_pool;
    let shared_world = deps.shared_world;
    let parties = deps.parties;
    let Some(character) = session.character.active_character.as_ref().cloned() else {
        return Ok(());
    };
    let map_id = character.position.map_id;
    let attacker = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let Some(spell_template) = shared_world
        .object_mgr
        .spell_template(world_db_pool, spell_id)
        .await?
    else {
        return Ok(());
    };

    if let Some(failure) = player_ranged_auto_attack_failure(
        world_db_pool,
        shared_world,
        session,
        target,
        &spell_template,
    )
    .await?
    {
        send_auto_repeat_spell_failure(stream, header_crypto, attacker, spell_id, failure).await?;
        if !try_transition_ranged_auto_repeat_to_melee(
            stream,
            shared_world,
            session,
            header_crypto,
            target,
        )
        .await?
        {
            shared_world
                .maps
                .set_player_auto_attack(map_id, character.guid, None, None)
                .await;
            mirror_session_player_auto_attack(session, None, None);
        }
        return Ok(());
    }

    let combat_stats = shared_world
        .maps
        .player_combat_stats(map_id, character.guid)
        .await
        .ok_or_else(|| {
            anyhow::anyhow!(
                "map-owned player combat stats missing for character {}",
                character.guid
            )
        })?;
    let Some(target_creature) = shared_world.maps.db_creature_snapshot(map_id, target).await else {
        shared_world
            .maps
            .set_player_auto_attack(map_id, character.guid, None, None)
            .await;
        mirror_session_player_auto_attack(session, None, None);
        return Ok(());
    };
    let weapon_skill_id = ranged_weapon_skill_id(world_db_pool, &session.inventory.items).await?;
    let attacker_skill = weapon_skill_id
        .map(|skill_id| {
            current_skill_value_with_active_auras(
                &session.character.character_skills,
                &session.auras.active_auras,
                skill_id,
            )
        })
        .unwrap_or(0);
    let ranged_outcome = player_ranged_outcome_against_db_creature(
        &combat_stats,
        character.level,
        attacker_skill,
        &target_creature,
    );
    let spell_miss_info = ranged_outcome.spell_miss_info();
    let ammo_visual = player_ranged_spell_ammo_visual(world_db_pool, session).await?;
    let swing_time = Instant::now();
    let next_swing = player_ranged_next_swing_at(swing_time, &combat_stats);

    let ammo_source = session
        .inventory
        .items
        .iter()
        .find(|item| item.item_template == session.character.player_ammo_id && item.count > 0)
        .cloned();
    if let Some(ammo_source) = ammo_source {
        consume_used_item(
            stream,
            character_db_pool,
            session,
            character.guid,
            &ammo_source,
            header_crypto,
        )
        .await?;
    }

    mirror_session_player_next_swing_at(session, Some(next_swing));
    shared_world
        .maps
        .set_player_ranged_next_shot_at(map_id, character.guid, next_swing)
        .await;

    let targets = SpellCastTargets {
        target_mask: SPELL_CAST_TARGET_UNIT,
        unit_target: Some(target),
        gameobject_target: None,
        source_location: None,
        destination: None,
    };
    let spell_start_body =
        build_spell_start_body_with_ammo(attacker, spell_id, 0, &targets, ammo_visual)?;
    send_packet(
        stream,
        WorldOpcode::SmsgSpellStart as u16,
        &spell_start_body,
        Some(&mut *header_crypto),
    )
    .await?;
    let observer_start = shared_world
        .maps
        .broadcast_nearby_player_packet(
            map_id,
            character.guid,
            PLAYER_VISIBILITY_RADIUS_YARDS,
            OutboundWorldPacket {
                opcode: WorldOpcode::SmsgSpellStart as u16,
                body: spell_start_body,
            },
        )
        .await;
    shared_world.sessions.dispatch(observer_start).await;
    send_packet(
        stream,
        WorldOpcode::SmsgCastResult as u16,
        &build_cast_result_ok_body(spell_id),
        Some(&mut *header_crypto),
    )
    .await?;
    let spell_go_body = if let Some(miss_info) = spell_miss_info {
        build_spell_go_body_with_miss_and_ammo(
            attacker,
            spell_id,
            &targets,
            miss_info,
            ammo_visual,
        )?
    } else {
        build_spell_go_body_with_ammo(attacker, spell_id, &targets, ammo_visual)?
    };
    send_packet(
        stream,
        WorldOpcode::SmsgSpellGo as u16,
        &spell_go_body,
        Some(&mut *header_crypto),
    )
    .await?;
    let observer_go = shared_world
        .maps
        .broadcast_nearby_player_packet(
            map_id,
            character.guid,
            PLAYER_VISIBILITY_RADIUS_YARDS,
            OutboundWorldPacket {
                opcode: WorldOpcode::SmsgSpellGo as u16,
                body: spell_go_body,
            },
        )
        .await;
    shared_world.sessions.dispatch(observer_go).await;

    let travel_delay =
        ranged_auto_attack_travel_delay_millis(shared_world, session, &spell_template, target)
            .await;
    if travel_delay > 0 {
        shared_world
            .maps
            .push_pending_ranged_auto_attack_event(
                map_id,
                character.guid,
                PendingRangedAutoAttackImpact {
                    spell_id,
                    target,
                    outcome: ranged_outcome,
                    weapon_skill_id,
                    due_at: swing_time + Duration::from_millis(travel_delay as u64),
                },
            )
            .await;
    } else {
        apply_player_ranged_auto_attack_impact(
            stream,
            CombatRewardDeps {
                character_db_pool,
                world_db_pool,
                shared_world,
                parties,
            },
            session,
            header_crypto,
            target,
            spell_id,
            ranged_outcome,
            weapon_skill_id,
            swing_time,
        )
        .await?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_ranged_auto_attack_impact(
    stream: &mut WorldPacketSink,
    deps: CombatRewardDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
    target: ObjectGuid,
    spell_id: u32,
    ranged_outcome: MeleeDamageOutcome,
    weapon_skill_id: Option<u16>,
    impact_time: Instant,
) -> anyhow::Result<()> {
    let character_db_pool = deps.character_db_pool;
    let world_db_pool = deps.world_db_pool;
    let shared_world = deps.shared_world;
    let parties = deps.parties;
    let Some(character) = session.character.active_character.as_ref().cloned() else {
        return Ok(());
    };
    let map_id = character.position.map_id;
    let attacker = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let Some(target_creature) = shared_world.maps.db_creature_snapshot(map_id, target).await else {
        shared_world
            .maps
            .set_player_auto_attack(map_id, character.guid, None, None)
            .await;
        mirror_session_player_auto_attack(session, None, None);
        return Ok(());
    };
    let combat_stats = shared_world
        .maps
        .player_combat_stats(map_id, character.guid)
        .await
        .ok_or_else(|| {
            anyhow::anyhow!(
                "map-owned player combat stats missing for character {}",
                character.guid
            )
        })?;
    let requested_damage = ranged_outcome.total_damage;
    let corpse_loot = if requested_damage >= target_creature.health {
        Some(
            prepare_db_creature_corpse_loot(
                shared_world.object_mgr,
                world_db_pool,
                parties,
                session,
                character.guid,
                target_creature.spawn.entry,
            )
            .await?,
        )
    } else {
        None
    };
    let Some(event) = shared_world
        .maps
        .apply_db_creature_damage(
            map_id,
            DbCreatureDamageRequest {
                creature_guid: target,
                killer: attacker,
                damage: requested_damage,
                melee_outcome: Some(ranged_outcome),
                spell_damage_outcome: None,
                spell_id: Some(spell_id),
                spell_school: 0,
                suppress_attacker_state: true,
                now: impact_time,
                now_epoch_secs: current_unix_epoch_secs(),
                exclude_character_guid: Some(character.guid),
                corpse_loot,
            },
        )
        .await?
    else {
        shared_world
            .maps
            .set_player_auto_attack(map_id, character.guid, None, None)
            .await;
        mirror_session_player_auto_attack(session, None, None);
        return Ok(());
    };

    let is_dead = event.death_finalization.is_some();
    mirror_session_db_creature(session, target.raw(), event.creature.clone());
    if let Some(spell_non_melee_log_body) = &event.spell_non_melee_log_body {
        send_packet(
            stream,
            WorldOpcode::SmsgSpellNonMeleeDamageLog as u16,
            spell_non_melee_log_body,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    if let Some(attacker_state_body) = &event.attacker_state_body {
        send_packet(
            stream,
            WorldOpcode::SmsgAttackerStateUpdate as u16,
            attacker_state_body,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    if let Some(spell_miss_log_body) = &event.spell_miss_log_body {
        send_packet(
            stream,
            WorldOpcode::SmsgSpellLogMiss as u16,
            spell_miss_log_body,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &event.update_body,
        Some(&mut *header_crypto),
    )
    .await?;
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
    if let Some(skill_id) = weapon_skill_id {
        if let Some(updated) = try_advance_combat_skill_value(
            character.level,
            skill_id,
            combat_stats.intellect,
            true,
            &mut session.character.character_skills,
        ) {
            wow_db::upsert_character_skill(
                character_db_pool,
                character.guid,
                updated.skill,
                updated.value,
                updated.max,
            )
            .await?;
            send_packet(
                stream,
                WorldOpcode::SmsgUpdateObject as u16,
                &build_player_skill_update_body(
                    character.guid,
                    updated,
                    &session.auras.active_auras,
                )?,
                Some(&mut *header_crypto),
            )
            .await?;
        }
    }
    if is_dead {
        finalize_db_creature_death(
            stream,
            CombatRewardDeps {
                character_db_pool,
                world_db_pool,
                shared_world,
                parties,
            },
            session,
            event.death_finalization,
            header_crypto,
        )
        .await?;
    }
    Ok(())
}

pub(in crate::world) async fn ranged_auto_attack_travel_delay_millis(
    shared_world: SharedWorldDeps<'_>,
    session: &WorldSessionState,
    spell_template: &wow_db::SpellTemplateQuery,
    target: ObjectGuid,
) -> u32 {
    if spell_template.speed <= 0.0 {
        return 0;
    }
    let Some(character) = session.character.active_character.as_ref() else {
        return 0;
    };
    let map_id = character.position.map_id;
    let caster_position = shared_world
        .maps
        .player_runtime_snapshot(map_id, character.guid)
        .await
        .map(|snapshot| snapshot.position)
        .unwrap_or(character.position);
    let Some(creature) = shared_world.maps.db_creature_snapshot(map_id, target).await else {
        return 0;
    };
    let distance = caster_position
        .distance_to(&creature.current_position)
        .max(5.0);
    ((distance / spell_template.speed.max(f32::EPSILON)) * 1000.0)
        .floor()
        .max(1.0) as u32
}

pub(in crate::world) async fn send_auto_repeat_spell_failure(
    stream: &mut WorldPacketSink,
    header_crypto: &mut HeaderCrypto,
    caster: ObjectGuid,
    spell_id: u32,
    failure: u8,
) -> anyhow::Result<()> {
    send_packet(
        stream,
        WorldOpcode::SmsgCastResult as u16,
        &build_cast_result_failure_body(spell_id, failure),
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgSpellFailure as u16,
        &build_spell_failure_body(caster, spell_id, failure)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgSpellFailedOther as u16,
        &build_spell_failed_other_body(caster, spell_id),
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn player_ranged_auto_attack_failure(
    world_db_pool: &MySqlPool,
    shared_world: SharedWorldDeps<'_>,
    session: &WorldSessionState,
    target: ObjectGuid,
    spell_template: &wow_db::SpellTemplateQuery,
) -> anyhow::Result<Option<u8>> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(Some(SPELL_FAILED_OUT_OF_RANGE));
    };
    let Some(ranged_weapon) =
        ranged_weapon_template(world_db_pool, &session.inventory.items).await?
    else {
        return Ok(Some(SPELL_FAILED_NO_AMMO));
    };
    if ranged_weapon_requires_ammo(&ranged_weapon) {
        let Some(ammo_template) = load_selected_ammo_template(
            world_db_pool,
            &session.inventory.items,
            session.character.player_ammo_id,
        )
        .await?
        else {
            return Ok(Some(SPELL_FAILED_NO_AMMO));
        };
        if !ranged_weapon_accepts_ammo(&ranged_weapon, &ammo_template) {
            return Ok(Some(SPELL_FAILED_NO_AMMO));
        }
    }
    let range = if spell_template.range_index == 0 {
        None
    } else {
        let Some(range) = shared_world.maps.spell_range(spell_template.range_index) else {
            return Ok(Some(SPELL_FAILED_OUT_OF_RANGE));
        };
        Some(range)
    };
    let validation = shared_world
        .maps
        .validate_player_spell_against_db_creature(
            character.position.map_id,
            character.guid,
            target,
            &session.movement.db_creature_navigation,
            range,
            spell_requires_infront_target(
                shared_world.object_mgr,
                world_db_pool,
                spell_template.id,
            )
            .await
            .unwrap_or(false),
        )
        .await;
    Ok(match validation.check {
        PlayerSpellTargetCheck::Clear => None,
        PlayerSpellTargetCheck::BadFacing => Some(SPELL_FAILED_UNIT_NOT_INFRONT),
        PlayerSpellTargetCheck::NotAttackable => Some(SPELL_FAILED_BAD_TARGETS),
        PlayerSpellTargetCheck::NavigationBlocked(
            DbCreatureNavigationResult::LineOfSightBlocked,
        ) => Some(SPELL_FAILED_LINE_OF_SIGHT),
        PlayerSpellTargetCheck::TooClose => Some(SPELL_FAILED_TOO_CLOSE),
        PlayerSpellTargetCheck::NoActiveCharacter
        | PlayerSpellTargetCheck::MissingTarget
        | PlayerSpellTargetCheck::TargetNotAlive
        | PlayerSpellTargetCheck::NavigationBlocked(_)
        | PlayerSpellTargetCheck::OutOfRange => Some(SPELL_FAILED_OUT_OF_RANGE),
    })
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct SkillProgressionUpdate {
    pub(in crate::world) slot: usize,
    pub(in crate::world) skill: u16,
    pub(in crate::world) value: u16,
    pub(in crate::world) max: u16,
}

pub(in crate::world) fn try_advance_combat_skill_value(
    character_level: u8,
    skill_id: u16,
    intellect: u32,
    weapon: bool,
    character_skills: &mut [CharacterSkill],
) -> Option<SkillProgressionUpdate> {
    let mut chance_rng = rand::thread_rng();
    let mut skill_rng = rand::thread_rng();
    try_advance_combat_skill_value_with_rolls(
        character_level,
        skill_id,
        intellect,
        weapon,
        character_skills,
        || chance_rng.gen_range(0.0f32..100.0f32),
        || skill_rng.gen_range(0..=512),
    )
}

pub(in crate::world) fn try_advance_combat_skill_value_with_rolls(
    character_level: u8,
    skill_id: u16,
    intellect: u32,
    weapon: bool,
    character_skills: &mut [CharacterSkill],
    mut chance_roll: impl FnMut() -> f32,
    mut update_skill_roll: impl FnMut() -> u32,
) -> Option<SkillProgressionUpdate> {
    let slot = character_skills
        .iter()
        .position(|skill| skill.skill == skill_id)?;
    let skill = &mut character_skills[slot];
    let level_cap = u16::from(character_level.max(1)).saturating_mul(5);
    if level_cap == 0 {
        return None;
    }

    let effective_max = skill.max.max(level_cap);
    let max_changed = skill.max != effective_max;
    skill.max = effective_max;
    if skill.value == 0 || skill.value >= effective_max {
        return max_changed.then_some(SkillProgressionUpdate {
            slot,
            skill: skill.skill,
            value: skill.value,
            max: skill.max,
        });
    }

    let room = effective_max.saturating_sub(skill.value);
    let mut chance = (f32::from(room / 5).max(1.0) / (f32::from(effective_max) / 5.0)) * 100.0;
    if weapon {
        chance += (chance * 0.02) * intellect as f32;
    }
    if chance_roll() >= chance {
        return max_changed.then_some(SkillProgressionUpdate {
            slot,
            skill: skill.skill,
            value: skill.value,
            max: skill.max,
        });
    }

    if u32::from(skill.value) * 512 >= u32::from(effective_max) * update_skill_roll() {
        return max_changed.then_some(SkillProgressionUpdate {
            slot,
            skill: skill.skill,
            value: skill.value,
            max: skill.max,
        });
    }

    skill.value = skill.value.saturating_add(1).min(effective_max);
    Some(SkillProgressionUpdate {
        slot,
        skill: skill.skill,
        value: skill.value,
        max: skill.max,
    })
}

pub(in crate::world) fn set_level_capped_combat_skill_maxes(
    character_level: u8,
    character_skills: &mut [CharacterSkill],
) -> Vec<SkillProgressionUpdate> {
    let level_cap = u16::from(character_level.max(1)).saturating_mul(5);
    character_skills
        .iter_mut()
        .enumerate()
        .filter_map(|(slot, skill)| {
            if !is_level_capped_combat_skill(skill.skill) {
                return None;
            }
            let old_value = skill.value;
            let old_max = skill.max;
            skill.max = level_cap;
            skill.value = skill.value.min(level_cap);
            (skill.value != old_value || skill.max != old_max).then_some(SkillProgressionUpdate {
                slot,
                skill: skill.skill,
                value: skill.value,
                max: skill.max,
            })
        })
        .collect()
}

pub(in crate::world) fn is_level_capped_combat_skill(skill_id: u16) -> bool {
    matches!(
        skill_id,
        SKILL_DEFENSE
            | SKILL_SWORDS
            | SKILL_AXES
            | SKILL_BOWS
            | SKILL_GUNS
            | SKILL_MACES
            | SKILL_TWO_HANDED_SWORDS
            | SKILL_STAVES
            | SKILL_TWO_HANDED_MACES
            | SKILL_UNARMED
            | SKILL_TWO_HANDED_AXES
            | SKILL_DAGGERS
            | SKILL_THROWN
            | SKILL_CROSSBOWS
            | SKILL_WANDS
            | SKILL_POLEARMS
            | SKILL_SPEARS
            | SKILL_FISHING
            | SKILL_FIST_WEAPONS
    )
}

pub(in crate::world) async fn main_hand_weapon_skill_id(
    world_db_pool: &MySqlPool,
    inventory: &[CharacterInventoryItem],
) -> anyhow::Result<Option<u16>> {
    let main_hand = inventory.iter().find(|item| {
        item.bag == INVENTORY_SLOT_BAG_0 as u32 && item.slot == EQUIPMENT_SLOT_MAINHAND
    });
    let Some(main_hand) = main_hand else {
        return Ok(Some(SKILL_UNARMED));
    };
    let Some(template) =
        wow_db::get_item_template_query(world_db_pool, main_hand.item_template).await?
    else {
        return Ok(None);
    };
    Ok(item_weapon_skill_from_template(&template))
}

pub(in crate::world) async fn has_main_hand_weapon_for_attack(
    world_db_pool: &MySqlPool,
    inventory: &[CharacterInventoryItem],
) -> anyhow::Result<bool> {
    let Some(main_hand) = inventory.iter().find(|item| {
        item.bag == INVENTORY_SLOT_BAG_0 as u32 && item.slot == EQUIPMENT_SLOT_MAINHAND
    }) else {
        return Ok(false);
    };
    let Some(template) =
        wow_db::get_item_template_query(world_db_pool, main_hand.item_template).await?
    else {
        return Ok(false);
    };
    Ok(template.class == ITEM_CLASS_WEAPON)
}

pub(in crate::world) fn queued_spell_requires_main_hand_weapon(
    template: &wow_db::SpellTemplateQuery,
) -> bool {
    template.attributes_ex3 & SPELL_ATTR_EX3_REQUIRES_MAIN_HAND_WEAPON != 0
}

pub(in crate::world) async fn ranged_weapon_skill_id(
    world_db_pool: &MySqlPool,
    inventory: &[CharacterInventoryItem],
) -> anyhow::Result<Option<u16>> {
    let Some(template) = ranged_weapon_template(world_db_pool, inventory).await? else {
        return Ok(None);
    };
    Ok(item_weapon_skill_from_template(&template))
}

pub(in crate::world) async fn ranged_weapon_template(
    world_db_pool: &MySqlPool,
    inventory: &[CharacterInventoryItem],
) -> anyhow::Result<Option<ItemTemplateQuery>> {
    let ranged = inventory
        .iter()
        .find(|item| item.bag == INVENTORY_SLOT_BAG_0 as u32 && item.slot == EQUIPMENT_SLOT_RANGED);
    let Some(ranged) = ranged else {
        return Ok(None);
    };
    Ok(wow_db::get_item_template_query(world_db_pool, ranged.item_template).await?)
}

pub(in crate::world) async fn player_ranged_spell_ammo_visual(
    world_db_pool: &MySqlPool,
    session: &WorldSessionState,
) -> anyhow::Result<Option<wow_proto::SpellAmmoVisual>> {
    let Some(ranged_weapon) =
        ranged_weapon_template(world_db_pool, &session.inventory.items).await?
    else {
        return Ok(None);
    };
    if ranged_weapon.inventory_type == INVTYPE_THROWN {
        return Ok(Some(wow_proto::SpellAmmoVisual {
            display_id: ranged_weapon.displayid,
            inventory_type: ranged_weapon.inventory_type,
        }));
    }
    let Some(ammo) = load_selected_ammo_template(
        world_db_pool,
        &session.inventory.items,
        session.character.player_ammo_id,
    )
    .await?
    else {
        return Ok(None);
    };
    if !ranged_weapon_accepts_ammo(&ranged_weapon, &ammo) {
        return Ok(None);
    }
    Ok(Some(wow_proto::SpellAmmoVisual {
        display_id: ammo.displayid,
        inventory_type: ammo.inventory_type,
    }))
}

pub(in crate::world) fn ranged_weapon_requires_ammo(template: &ItemTemplateQuery) -> bool {
    template.class == ITEM_CLASS_WEAPON
        && matches!(
            template.subclass,
            ITEM_SUBCLASS_WEAPON_BOW | ITEM_SUBCLASS_WEAPON_GUN | ITEM_SUBCLASS_WEAPON_CROSSBOW
        )
}

pub(in crate::world) fn item_weapon_skill_from_template(
    template: &ItemTemplateQuery,
) -> Option<u16> {
    if template.class != ITEM_CLASS_WEAPON {
        return None;
    }
    match template.subclass {
        0 => Some(SKILL_AXES),
        1 => Some(SKILL_TWO_HANDED_AXES),
        2 => Some(SKILL_BOWS),
        3 => Some(SKILL_GUNS),
        4 => Some(SKILL_MACES),
        5 => Some(SKILL_TWO_HANDED_MACES),
        6 => Some(SKILL_POLEARMS),
        7 => Some(SKILL_SWORDS),
        8 => Some(SKILL_TWO_HANDED_SWORDS),
        10 => Some(SKILL_STAVES),
        13 => Some(SKILL_FIST_WEAPONS),
        15 => Some(SKILL_DAGGERS),
        16 => Some(SKILL_THROWN),
        17 => Some(SKILL_SPEARS),
        18 => Some(SKILL_CROSSBOWS),
        19 => Some(SKILL_WANDS),
        20 => Some(SKILL_FISHING),
        _ => None,
    }
}

pub(in crate::world) fn build_player_skill_update_body(
    character_guid: u32,
    updated: SkillProgressionUpdate,
    active_auras: &[ActiveAura],
) -> anyhow::Result<Vec<u8>> {
    build_player_skill_updates_body(character_guid, &[updated], active_auras)
}

pub(in crate::world) fn build_player_skill_updates_body(
    character_guid: u32,
    updates: &[SkillProgressionUpdate],
    active_auras: &[ActiveAura],
) -> anyhow::Result<Vec<u8>> {
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player_guid)?;

    let mut values = vec![None; PLAYER_END_FIELDS];
    for updated in updates {
        let field = PLAYER_SKILL_INFO_1_1 + updated.slot * 3;
        set_update_value(&mut values, field, make_pair32(updated.skill, 0))?;
        set_update_value(
            &mut values,
            field + 1,
            make_pair32(updated.value, updated.max),
        )?;
        set_update_value(
            &mut values,
            field + 2,
            active_aura_skill_bonus_pair(active_auras, updated.skill),
        )?;
    }
    write_update_values(&mut block, &values)?;
    Ok(build_update_object_body(&[block]))
}

pub(in crate::world) async fn begin_db_creature_retaliation_if_needed(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    map_id: u32,
    session: &mut WorldSessionState,
    creature: ObjectGuid,
    player: ObjectGuid,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    begin_db_creature_combat_with_assistance(
        stream,
        shared_world,
        map_id,
        session,
        creature,
        player,
        header_crypto,
    )
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn try_process_db_creature_event_ai_hp_actions(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    world_db_pool: &MySqlPool,
    session: &mut WorldSessionState,
    map_id: u32,
    creature: ObjectGuid,
    victim: ObjectGuid,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(snapshot) = shared_world
        .maps
        .db_creature_snapshot(map_id, creature)
        .await
    else {
        return Ok(());
    };
    let scripts = shared_world
        .object_mgr
        .creature_ai_scripts(world_db_pool, snapshot.spawn.entry)
        .await?;
    if scripts.is_empty() {
        return Ok(());
    }
    let exclude_character_guid = session.character.active_character.as_ref().map(|c| c.guid);
    let Some(event) = shared_world
        .maps
        .process_db_creature_event_ai_hp_actions(
            map_id,
            &session.movement.db_creature_navigation,
            creature,
            victim,
            &scripts,
            now,
            exclude_character_guid,
        )
        .await?
    else {
        return Ok(());
    };
    mirror_session_db_creature(session, creature.raw(), event.creature);
    for packet in &event.direct_packets {
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

pub(in crate::world) async fn try_transition_ranged_auto_repeat_to_melee(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
    target: ObjectGuid,
) -> anyhow::Result<bool> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(false);
    };
    let map_id = character.position.map_id;
    let character_guid = character.guid;
    if db_creature_player_melee_check_from_map(shared_world, session, target).await
        != PlayerMeleeCheck::Clear
    {
        return Ok(false);
    }

    let attacker = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let next_swing_at = Some(Instant::now());
    mirror_session_player_auto_attack(session, Some(target), next_swing_at);
    shared_world
        .maps
        .set_player_auto_attack(map_id, character_guid, Some(target), next_swing_at)
        .await;

    send_packet(
        stream,
        WorldOpcode::SmsgAttackStart as u16,
        &build_attack_start_body(attacker, target),
        Some(&mut *header_crypto),
    )
    .await?;
    broadcast_player_attack_start(shared_world, session, attacker, target).await;
    Ok(true)
}

pub(in crate::world) fn player_melee_retry_at(now: Instant) -> Instant {
    now + Duration::from_millis(PLAYER_MELEE_RETRY_MILLIS)
}

pub(in crate::world) fn player_main_hand_next_swing_at(
    now: Instant,
    combat_stats: &PlayerCombatStats,
) -> Instant {
    now + Duration::from_millis(combat_stats.main_attack_time_ms.max(1) as u64)
}

pub(in crate::world) fn player_ranged_next_swing_at(
    now: Instant,
    combat_stats: &PlayerCombatStats,
) -> Instant {
    now + Duration::from_millis(combat_stats.ranged_attack_time_ms.max(1) as u64)
}

pub(in crate::world) fn rage_gain_from_main_hand_white_damage(
    damage: u32,
    level: u8,
    attack_time_ms: u32,
    outcome: MeleeHitOutcome,
) -> u32 {
    // Classic Era white-hit rage includes a weapon-speed hit factor. This is
    // especially visible at starter levels where raw damage is small.
    if damage == 0 {
        return 0;
    }
    let conversion = rage_conversion_for_level(level);
    if conversion <= 0.0 {
        return 0;
    }
    let speed = attack_time_ms.max(1) as f64 / 1000.0;
    let hit_factor = if outcome == MeleeHitOutcome::Crit {
        7.0_f64
    } else {
        3.5_f64
    };
    let damage = damage as f64;
    let rage = ((15.0 * damage) / (4.0 * conversion)) + ((hit_factor * speed) / 2.0);
    let cap = (15.0 * damage) / conversion;
    (rage.min(cap).max(0.0) * 10.0) as u32
}

pub(in crate::world) fn rage_gain_from_damage_taken(damage: u32, level: u8) -> u32 {
    if damage == 0 {
        return 0;
    }
    let rage_conversion = rage_conversion_for_level(level);
    if rage_conversion <= 0.0 {
        return 0;
    }
    let rage = (damage as f64 / rage_conversion) * 2.5_f64;
    (rage.max(0.0) * 10.0) as u32
}

pub(in crate::world) fn rage_conversion_for_level(level: u8) -> f64 {
    let level = level as f64;
    0.0091107836_f64 * level * level + 3.225598133_f64 * level + 4.2652911_f64
}

pub(in crate::world) async fn send_player_melee_swing_error_if_changed(
    stream: &mut WorldPacketSink,
    session: &mut WorldSessionState,
    error: PlayerMeleeSwingError,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if session.combat.last_player_melee_swing_error == Some(error) {
        return Ok(());
    }
    session.combat.last_player_melee_swing_error = Some(error);
    let packet = error.packet();
    send_packet(stream, packet.opcode, &packet.body, Some(header_crypto)).await
}

pub(in crate::world) async fn finalize_db_creature_death(
    stream: &mut WorldPacketSink,
    deps: CombatRewardDeps<'_>,
    session: &mut WorldSessionState,
    death_finalization: Option<DbCreatureDeathFinalizationEvent>,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let character_db_pool = deps.character_db_pool;
    let world_db_pool = deps.world_db_pool;
    let shared_world = deps.shared_world;
    let parties = deps.parties;
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
    if let Some((map_id, character_guid)) = session
        .character
        .active_character
        .as_ref()
        .map(|character| (character.position.map_id, character.guid))
    {
        let next_swing_at = shared_world
            .maps
            .player_runtime_snapshot(map_id, character_guid)
            .await
            .and_then(|snapshot| snapshot.active_combat_next_swing_at);
        mirror_session_player_auto_attack(session, None, next_swing_at);
        shared_world
            .maps
            .set_player_auto_attack(map_id, character_guid, None, next_swing_at)
            .await;
    }
    clear_db_creature_combat_if_attacker(session, killed);
    if let Some(character) = session.character.active_character.as_ref() {
        if let Some(creature) = shared_world
            .maps
            .db_creature_snapshot(character.position.map_id, killed)
            .await
        {
            if !reward_party_for_db_creature_kill(
                stream,
                CombatRewardDeps {
                    character_db_pool,
                    world_db_pool,
                    shared_world,
                    parties,
                },
                session,
                killed,
                &creature,
                header_crypto,
            )
            .await?
            {
                grant_db_creature_kill_credit(
                    stream,
                    character_db_pool,
                    shared_world.object_mgr,
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
                    shared_world.maps,
                    session,
                    killed,
                    &creature.spawn.template,
                    header_crypto,
                )
                .await?;
            }
        }
    }
    let player_still_has_attackers =
        if let Some(character) = session.character.active_character.as_ref() {
            let player = ObjectGuid::new(HighGuid::Player, 0, character.guid);
            !shared_world
                .maps
                .active_db_creature_combats_for_victim(character.position.map_id, player)
                .await
                .is_empty()
        } else {
            false
        };
    if !player_still_has_attackers {
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

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn grant_db_creature_xp(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    maps: &MapRuntimeManager,
    session: &mut WorldSessionState,
    killed: ObjectGuid,
    creature_template: &CreatureTemplateQuery,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.character.active_character else {
        return Ok(());
    };
    let xp = creature_xp_reward(character.level, creature_template);
    award_character_xp(
        stream,
        character_db_pool,
        world_db_pool,
        maps,
        session,
        Some(killed),
        xp,
        header_crypto,
    )
    .await
}

pub(in crate::world) const GROUP_XP_DISTANCE_YARDS: f32 = 74.0;

#[derive(Debug)]
pub(in crate::world) struct PartyRewardMember {
    pub(in crate::world) member: PartyMember,
    pub(in crate::world) snapshot: PlayerRuntimeSnapshot,
    pub(in crate::world) xp: u32,
}

pub(in crate::world) async fn reward_party_for_db_creature_kill(
    stream: &mut WorldPacketSink,
    deps: CombatRewardDeps<'_>,
    session: &mut WorldSessionState,
    killed: ObjectGuid,
    creature: &DbCreatureRuntime,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    let character_db_pool = deps.character_db_pool;
    let world_db_pool = deps.world_db_pool;
    let shared_world = deps.shared_world;
    let parties = deps.parties;
    let Some(killer) = session.character.active_character.as_ref() else {
        return Ok(false);
    };
    let killer_guid = killer.guid;
    let map_id = killer.position.map_id;
    let party_members = parties.party_members(killer_guid).await;
    if party_members.len() <= 1 {
        return Ok(false);
    }

    let mut eligible = Vec::new();
    for member in party_members {
        let Some(snapshot) = shared_world
            .maps
            .player_runtime_snapshot(map_id, member.guid)
            .await
        else {
            continue;
        };
        if is_position_inside_radius(
            snapshot.position,
            creature.current_position,
            GROUP_XP_DISTANCE_YARDS,
        ) {
            eligible.push(PartyRewardMember {
                member,
                snapshot,
                xp: 0,
            });
        }
    }
    if eligible.len() <= 1 {
        return Ok(false);
    }

    assign_group_xp(&mut eligible, &creature.spawn.template);
    for reward in eligible {
        let is_killer = reward.member.guid == killer_guid;
        if is_killer {
            grant_db_creature_kill_credit(
                stream,
                character_db_pool,
                shared_world.object_mgr,
                world_db_pool,
                session,
                killed,
                header_crypto,
            )
            .await?;
            award_character_xp(
                stream,
                character_db_pool,
                world_db_pool,
                shared_world.maps,
                session,
                Some(killed),
                reward.xp,
                header_crypto,
            )
            .await?;
            if let Some(character) = session.character.active_character.as_ref() {
                deps.shared_world
                    .maps
                    .sync_player_gameplay_state(character.position.map_id, character.guid, session)
                    .await;
            }
            continue;
        }

        let mut quest_statuses = reward.snapshot.quest_statuses.clone();
        let quest_packets = grant_db_creature_kill_credit_to_member(
            character_db_pool,
            shared_world.object_mgr,
            world_db_pool,
            reward.member.guid,
            killed,
            &reward.snapshot.inventory,
            &mut quest_statuses,
        )
        .await?;
        let xp_award = award_character_xp_to_member(
            character_db_pool,
            world_db_pool,
            reward.member.guid,
            &reward.snapshot,
            Some(killed),
            reward.xp,
        )
        .await?;
        let mut packets = quest_packets;
        packets.extend(xp_award.packets);
        shared_world
            .maps
            .update_player_reward_state(
                map_id,
                reward.member.guid,
                PlayerRewardRuntimeUpdate {
                    level: xp_award.level,
                    xp: xp_award.xp,
                    rest_bonus: xp_award.rest_bonus,
                    player_bytes2: xp_award.player_bytes2,
                    health: xp_award.health,
                    max_health: xp_award.max_health,
                    power1: xp_award.power1,
                    max_power1: xp_award.max_power1,
                    power2: xp_award.power2,
                    world_stats: xp_award.world_stats,
                    combat_stats: xp_award.combat_stats,
                    quest_statuses,
                },
            )
            .await;
        if let Some(session_id) = shared_world
            .sessions
            .session_for_character(reward.member.guid)
            .await
        {
            for packet in packets {
                shared_world.sessions.send_packet(session_id, packet).await;
            }
        }
    }
    Ok(true)
}

pub(in crate::world) fn assign_group_xp(
    members: &mut [PartyRewardMember],
    creature_template: &CreatureTemplateQuery,
) {
    let alive: Vec<usize> = members
        .iter()
        .enumerate()
        .filter_map(|(index, member)| (member.snapshot.health > 0).then_some(index))
        .collect();
    if alive.is_empty() {
        return;
    }
    let sum_levels: u32 = alive
        .iter()
        .map(|index| members[*index].snapshot.level as u32)
        .sum();
    if sum_levels == 0 {
        return;
    }
    let count = alive.len() as u32;
    for index in alive {
        let level = members[index].snapshot.level;
        let base = creature_xp_reward(level, creature_template);
        let share = base as f32 * group_xp_rate(count) * (level as f32 / sum_levels as f32);
        members[index].xp = nearbyint_to_u32(share);
    }
}

pub(in crate::world) fn group_xp_rate(count: u32) -> f32 {
    match count {
        0..=2 => 1.0,
        3 => 1.166,
        4 => 1.3,
        5 => 1.4,
        _ => (1.0 - count as f32 * 0.05).max(0.01),
    }
}

pub(in crate::world) async fn grant_db_creature_kill_credit_to_member(
    character_db_pool: &MySqlPool,
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    character_guid: u32,
    killed_guid: ObjectGuid,
    inventory: &[CharacterInventoryItem],
    quest_statuses: &mut HashMap<u32, CharacterQuestStatus>,
) -> anyhow::Result<Vec<OutboundWorldPacket>> {
    let killed_entry = killed_guid.entry();
    let active_quests: Vec<u32> = quest_statuses
        .values()
        .filter(|status| status.status == QUEST_STATUS_INCOMPLETE && status.rewarded == 0)
        .map(|status| status.quest)
        .collect();
    let mut packets = Vec::new();
    for quest_id in active_quests {
        let Some(quest) = object_mgr.quest_template(world_db_pool, quest_id).await? else {
            continue;
        };
        let Some(index) = quest.required_creature_index(killed_entry) else {
            continue;
        };
        let required = quest.required_creature_count(index);
        if required == 0 {
            continue;
        }
        let current = quest_statuses
            .get(&quest_id)
            .map(|status| match index {
                0 => status.mobcount1,
                1 => status.mobcount2,
                2 => status.mobcount3,
                3 => status.mobcount4,
                _ => 0,
            })
            .unwrap_or(0);
        if current >= required {
            continue;
        }
        let new_count = (current + 1).min(required);
        let mut next_status =
            quest_statuses
                .get(&quest_id)
                .cloned()
                .unwrap_or(CharacterQuestStatus {
                    quest: quest_id,
                    status: QUEST_STATUS_INCOMPLETE,
                    rewarded: 0,
                    mobcount1: 0,
                    mobcount2: 0,
                    mobcount3: 0,
                    mobcount4: 0,
                });
        match index {
            0 => next_status.mobcount1 = new_count,
            1 => next_status.mobcount2 = new_count,
            2 => next_status.mobcount3 = new_count,
            3 => next_status.mobcount4 = new_count,
            _ => {}
        }
        let complete = quest_status_can_complete(&next_status, &quest, inventory);
        let status = wow_db::update_character_quest_mob_count(
            character_db_pool,
            character_guid,
            quest_id,
            index,
            new_count,
            complete,
        )
        .await?;
        quest_statuses.insert(quest_id, status.clone());
        let Some(slot) = quest_log_slot_for_statuses(quest_statuses, quest_id) else {
            continue;
        };
        packets.push(OutboundWorldPacket {
            opcode: WorldOpcode::SmsgQuestUpdateAddKill as u16,
            body: build_quest_update_add_kill_body(&quest, killed_guid, index, new_count),
        });
        packets.push(OutboundWorldPacket {
            opcode: WorldOpcode::SmsgUpdateObject as u16,
            body: build_player_quest_log_update_body(character_guid, slot, &status)?,
        });
        if complete {
            packets.push(OutboundWorldPacket {
                opcode: WorldOpcode::SmsgQuestUpdateComplete as u16,
                body: quest_id.to_le_bytes().to_vec(),
            });
        }
    }
    Ok(packets)
}

pub(in crate::world) async fn award_character_xp_to_member(
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    character_guid: u32,
    snapshot: &PlayerRuntimeSnapshot,
    source: Option<ObjectGuid>,
    xp: u32,
) -> anyhow::Result<MemberXpAward> {
    if xp == 0 || snapshot.level >= DEFAULT_MAX_PLAYER_LEVEL {
        return Ok(MemberXpAward::unchanged(snapshot));
    }
    let previous_stats = wow_db::get_player_world_stats(
        world_db_pool,
        snapshot.race,
        snapshot.class,
        snapshot.level,
    )
    .await?;
    let mut next_level_xp = previous_stats.next_level_xp;
    let rested_bonus_xp = if source.is_some() {
        (snapshot.rest_bonus as u32).min(xp)
    } else {
        0
    };
    let rest_bonus = clamp_rest_bonus(
        snapshot.rest_bonus - rested_bonus_xp as f32,
        snapshot.level,
        next_level_xp,
    );
    let player_bytes2 = player_bytes2_with_rest_bonus(snapshot.player_bytes2, rest_bonus);
    let total_xp = xp.saturating_add(rested_bonus_xp);
    let mut new_level = snapshot.level;
    let mut new_xp = snapshot.xp.saturating_add(total_xp);
    while next_level_xp > 0 && new_xp >= next_level_xp && new_level < DEFAULT_MAX_PLAYER_LEVEL {
        new_xp -= next_level_xp;
        new_level += 1;
        next_level_xp = wow_db::get_player_next_level_xp(world_db_pool, new_level).await?;
    }
    let new_stats =
        wow_db::get_player_world_stats(world_db_pool, snapshot.race, snapshot.class, new_level)
            .await?;
    let leveled = new_level != snapshot.level;
    let (world_stats_update, combat_stats_update) = if leveled {
        let equipped_templates =
            load_equipped_item_templates(world_db_pool, &snapshot.inventory).await?;
        (
            Some(new_stats),
            Some(player_combat_stats_for_values(
                snapshot.class,
                new_level,
                &new_stats,
                &equipped_templates,
            )),
        )
    } else {
        (None, None)
    };
    let max_health = new_stats.max_health().max(1);
    let max_mana = new_stats.max_mana();
    let health = if leveled {
        max_health
    } else {
        snapshot.health.max(1).min(max_health)
    };
    let power1 = if max_mana == 0 {
        0
    } else if leveled {
        max_mana
    } else {
        snapshot.power1.min(max_mana)
    };
    let power2 = snapshot.power2.min(POWER_RAGE_DEFAULT);
    let power3 = 0;
    let power4 = create_power_for_class_power(snapshot.class, POWER_ENERGY);
    let power5 = 0;
    wow_db::update_character_progression_state(
        character_db_pool,
        character_guid,
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
    wow_db::update_character_rest_state(
        character_db_pool,
        character_guid,
        rest_bonus,
        snapshot.flags & PLAYER_FLAGS_RESTING != 0,
        current_unix_time_secs(),
    )
    .await?;

    let mut packets = vec![OutboundWorldPacket {
        opcode: WorldOpcode::SmsgLogXpGain as u16,
        body: build_log_xp_gain_body(source, xp, rested_bonus_xp),
    }];
    if rested_bonus_xp > 0 {
        packets.push(OutboundWorldPacket {
            opcode: WorldOpcode::SmsgUpdateObject as u16,
            body: build_player_rest_update_body(character_guid, player_bytes2, rest_bonus)?,
        });
    }
    if leveled {
        packets.push(OutboundWorldPacket {
            opcode: WorldOpcode::SmsgLevelupInfo as u16,
            body: build_levelup_info_body(new_level, &previous_stats, &new_stats),
        });
    }
    packets.push(OutboundWorldPacket {
        opcode: WorldOpcode::SmsgUpdateObject as u16,
        body: build_player_progression_update_body(PlayerProgressionUpdate {
            character_guid,
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
    });
    Ok(MemberXpAward {
        level: new_level,
        xp: new_xp,
        rest_bonus,
        player_bytes2,
        health,
        max_health,
        power1,
        max_power1: max_mana,
        power2,
        world_stats: world_stats_update,
        combat_stats: combat_stats_update,
        packets,
    })
}

#[derive(Debug)]
pub(in crate::world) struct MemberXpAward {
    pub(in crate::world) level: u8,
    pub(in crate::world) xp: u32,
    pub(in crate::world) rest_bonus: f32,
    pub(in crate::world) player_bytes2: u32,
    pub(in crate::world) health: u32,
    pub(in crate::world) max_health: u32,
    pub(in crate::world) power1: u32,
    pub(in crate::world) max_power1: u32,
    pub(in crate::world) power2: u32,
    pub(in crate::world) world_stats: Option<PlayerWorldStats>,
    pub(in crate::world) combat_stats: Option<PlayerCombatStats>,
    pub(in crate::world) packets: Vec<OutboundWorldPacket>,
}

impl MemberXpAward {
    pub(in crate::world) fn unchanged(snapshot: &PlayerRuntimeSnapshot) -> Self {
        Self {
            level: snapshot.level,
            xp: snapshot.xp,
            rest_bonus: snapshot.rest_bonus,
            player_bytes2: snapshot.player_bytes2,
            health: snapshot.health,
            max_health: snapshot.max_health,
            power1: snapshot.power1,
            max_power1: snapshot.max_power1,
            power2: snapshot.power2,
            world_stats: None,
            combat_stats: None,
            packets: Vec::new(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn award_character_xp(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    maps: &MapRuntimeManager,
    session: &mut WorldSessionState,
    source: Option<ObjectGuid>,
    xp: u32,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if xp == 0 {
        return Ok(());
    }
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    if character.level >= DEFAULT_MAX_PLAYER_LEVEL {
        return Ok(());
    }

    let guid = character.guid;
    let race = character.race;
    let class = character.class;
    let map_id = character.position.map_id;
    let old_level = character.level;
    let old_xp = character.xp;
    let previous_stats =
        wow_db::get_player_world_stats(world_db_pool, race, class, old_level).await?;
    let mut next_level_xp = previous_stats.next_level_xp;
    let rested_bonus_xp = if source.is_some() {
        consume_rested_xp(session, xp, next_level_xp)
    } else {
        0
    };
    let total_xp = xp.saturating_add(rested_bonus_xp);
    let mut new_level = old_level;
    let mut new_xp = old_xp.saturating_add(total_xp);
    while next_level_xp > 0 && new_xp >= next_level_xp && new_level < DEFAULT_MAX_PLAYER_LEVEL {
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
        session.character.player_health.max(1).min(max_health)
    };
    let power1 = if max_mana == 0 {
        0
    } else if leveled {
        max_mana
    } else {
        session.character.player_mana.min(max_mana)
    };
    let power2 = session.character.player_rage.min(POWER_RAGE_DEFAULT);
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
    if let Some(character) = session.character.active_character.as_mut() {
        character.level = new_level;
        character.xp = new_xp;
    }
    session.rest.next_level_xp = new_stats.next_level_xp;
    session.character.player_health = health;
    session.character.player_mana = power1;
    session.character.player_rage = power2;
    session.character.player_energy = power4;
    let skill_cap_updates = if leveled {
        sync_player_level_backed_skills(
            maps,
            race,
            class,
            new_level,
            &mut session.character.character_skills,
        )
    } else {
        Vec::new()
    };
    for updated in &skill_cap_updates {
        wow_db::upsert_character_skill(
            character_db_pool,
            guid,
            updated.skill,
            updated.value,
            updated.max,
        )
        .await?;
    }
    persist_character_rest_state(character_db_pool, session).await?;

    let equipped_templates =
        load_equipped_item_templates(world_db_pool, &session.inventory.items).await?;
    let combat_stats =
        player_combat_stats_for_values(class, new_level, &new_stats, &equipped_templates);
    maps.update_player_level_progression_state(
        map_id,
        guid,
        PlayerLevelProgressionRuntimeUpdate {
            level: new_level,
            xp: new_xp,
            health,
            power1,
            power2,
            power4,
            world_stats: new_stats,
            combat_stats,
        },
    )
    .await;

    send_packet(
        stream,
        WorldOpcode::SmsgLogXpGain as u16,
        &build_log_xp_gain_body(source, xp, rested_bonus_xp),
        Some(&mut *header_crypto),
    )
    .await?;
    if rested_bonus_xp > 0 {
        send_rest_update(stream, session, &mut *header_crypto).await?;
    }
    if leveled {
        send_packet(
            stream,
            WorldOpcode::SmsgLevelupInfo as u16,
            &build_levelup_info_body(new_level, &previous_stats, &new_stats),
            Some(&mut *header_crypto),
        )
        .await?;
    }
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
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
    .await?;
    if !skill_cap_updates.is_empty() {
        send_packet(
            stream,
            WorldOpcode::SmsgUpdateObject as u16,
            &build_player_skill_updates_body(
                guid,
                &skill_cap_updates,
                &session.auras.active_auras,
            )?,
            Some(header_crypto),
        )
        .await?;
    }

    Ok(())
}

pub(in crate::world) fn creature_xp_reward(
    player_level: u8,
    template: &CreatureTemplateQuery,
) -> u32 {
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

pub(in crate::world) fn base_creature_xp_gain(player_level: u32, mob_level: u32) -> f32 {
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

pub(in crate::world) fn gray_level(player_level: u32) -> u32 {
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

pub(in crate::world) fn zero_difference(unit_level: u32) -> u32 {
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

pub(in crate::world) fn quest_xp_reward(player_level: u8, quest: &QuestTemplateQuery) -> u32 {
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

pub(in crate::world) fn quest_xp_ceil(value: f32) -> u32 {
    value.ceil().max(0.0) as u32
}

pub(in crate::world) fn nearbyint_to_u32(value: f32) -> u32 {
    if value <= 0.0 {
        0
    } else {
        value.round_ties_even() as u32
    }
}
