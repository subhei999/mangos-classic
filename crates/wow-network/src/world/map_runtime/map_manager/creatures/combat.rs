use super::*;

impl MapRuntimeManager {
    pub(in crate::world) async fn apply_db_creature_damage(
        &self,
        map_id: u32,
        request: DbCreatureDamageRequest,
    ) -> anyhow::Result<Option<DbCreatureDamageEvent>> {
        let map = self.get_or_create_map(map_id, 0).await;
        let event = map.lock().await.apply_db_creature_damage(request);
        event
    }

    pub(in crate::world) async fn begin_db_creature_combat(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        now: Instant,
    ) -> Option<(CreatureCombatState, DbCreatureRuntime)> {
        let map = self.get_or_create_map(map_id, 0).await;
        let mut map = map.lock().await;
        let creature = map.db_creature_combat_snapshot(attacker)?;
        let combat = map.begin_db_creature_combat(attacker, victim, now)?;
        Some((combat, creature))
    }

    pub(in crate::world) async fn apply_db_creature_taunt_threat(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        taunter: ObjectGuid,
    ) {
        let map = self.get_or_create_map(map_id, 0).await;
        map.lock()
            .await
            .apply_db_creature_taunt_threat(attacker, taunter);
    }

    pub(in crate::world) async fn switch_db_creature_threat_victim_if_needed(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        exclude_character_guid: Option<u32>,
    ) -> anyhow::Result<Option<DbCreatureThreatTargetSwitchEvent>> {
        let map = self.get_or_create_map(map_id, 0).await;
        let event = map
            .lock()
            .await
            .switch_db_creature_threat_victim_if_needed(attacker, exclude_character_guid);
        event
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn clear_db_creature_combat(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
    ) {
        let map = self.get_or_create_map(map_id, 0).await;
        map.lock().await.clear_db_creature_combat(attacker);
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn clear_db_creature_combats_for_victim(
        &self,
        map_id: u32,
        victim: ObjectGuid,
    ) {
        let map = self.get_or_create_map(map_id, 0).await;
        map.lock()
            .await
            .clear_db_creature_combats_for_victim(victim);
    }

    pub(in crate::world) async fn active_db_creature_combats_for_victim(
        &self,
        map_id: u32,
        victim: ObjectGuid,
    ) -> Vec<CreatureCombatState> {
        let map = self.get_or_create_map(map_id, 0).await;
        let combats = map
            .lock()
            .await
            .active_db_creature_combats_for_victim(victim);
        combats
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::world) async fn advance_db_creature_combats_for_victim(
        &self,
        world_db_pool: &MySqlPool,
        object_mgr: &ObjectMgr,
        map_id: u32,
        victim: ObjectGuid,
        current_session_id: SessionId,
        defense: PlayerMeleeDefenseInput,
        navigation: &DbCreatureNavigationGuardrail,
        now: Instant,
    ) -> anyhow::Result<DbCreatureVictimCombatAdvanceTick> {
        let map = self.get_or_create_map(map_id, 0).await;
        {
            let mut map_guard = map.lock().await;
            let player_alive = map_guard
                .players
                .get(&victim.counter())
                .is_some_and(|player| {
                    player.health > 0 && player.death_state == PlayerDeathState::Alive
                });
            if !player_alive {
                map_guard.clear_db_creature_combats_for_victim(victim);
                return Ok(DbCreatureVictimCombatAdvanceTick::default());
            }
        }

        let attackers = map
            .lock()
            .await
            .active_db_creature_combat_attackers_for_victim(victim);
        let mut tick = DbCreatureVictimCombatAdvanceTick::default();
        for attacker in attackers {
            let victim_died = self
                .advance_db_creature_attack_for_victim(
                    map.clone(),
                    world_db_pool,
                    object_mgr,
                    victim,
                    current_session_id,
                    attacker,
                    defense,
                    navigation,
                    now,
                    &mut tick,
                )
                .await?;
            if victim_died {
                break;
            }
        }
        let map_guard = map.lock().await;
        tick.active_combats = map_guard.active_db_creature_combats_for_victim(victim);
        tick.player_in_combat = map_guard
            .players
            .get(&victim.counter())
            .is_some_and(|player| player.in_combat);
        Ok(tick)
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn active_db_creature_combat_snapshot(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        victim: ObjectGuid,
    ) -> Option<ActiveDbCreatureCombatSnapshot> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let snapshot = map
            .lock()
            .await
            .active_db_creature_combat_snapshot(attacker, victim);
        snapshot
    }

    fn split_packets_for_session(
        current_session_id: SessionId,
        packets: Vec<(SessionId, OutboundWorldPacket)>,
        tick: &mut DbCreatureVictimCombatAdvanceTick,
    ) {
        for (session_id, packet) in packets {
            if session_id == current_session_id {
                tick.direct_packets.push(packet);
            } else {
                tick.observer_packets.push((session_id, packet));
            }
        }
    }

    fn push_creature_broadcast_packet(
        map: &MapRuntime,
        victim: ObjectGuid,
        current_session_id: SessionId,
        position: WorldPosition,
        packet: OutboundWorldPacket,
        tick: &mut DbCreatureVictimCombatAdvanceTick,
    ) {
        tick.direct_packets.push(packet.clone());
        tick.observer_packets
            .extend(map.nearby_player_packet_broadcast(
                position,
                Some(victim.counter()),
                packet.opcode,
                packet.body,
            ));
        let _ = current_session_id;
    }

    fn push_db_creature_player_melee_event(
        tick: &mut DbCreatureVictimCombatAdvanceTick,
        attacker: ObjectGuid,
        player_level: u8,
        event: DbCreaturePlayerDamageEvent,
    ) -> bool {
        tick.local_effects
            .push(DbCreatureVictimCombatLocalEffect::Melee {
                attacker,
                damage_taken: event.damage,
                victim_health: event.victim_health,
                aura_changed: event.aura_changed,
                rage_gain: rage_gain_from_damage_taken(event.damage, player_level),
                player_died: event.victim_health == 0,
            });
        for packet in event.direct_packets {
            tick.direct_packets.push(packet);
        }
        if let Some(packet) = event.aura_packet {
            tick.direct_packets.push(packet);
        }
        tick.direct_packets.push(OutboundWorldPacket {
            opcode: WorldOpcode::SmsgAttackerStateUpdate as u16,
            body: event.attacker_state_body,
        });
        tick.direct_packets.push(OutboundWorldPacket {
            opcode: WorldOpcode::SmsgUpdateObject as u16,
            body: event.health_update_body,
        });
        tick.observer_packets.extend(event.observer_packets);
        event.victim_health == 0
    }

    #[allow(clippy::too_many_arguments)]
    async fn advance_db_creature_attack_for_victim(
        &self,
        map: Arc<Mutex<MapRuntime>>,
        world_db_pool: &MySqlPool,
        object_mgr: &ObjectMgr,
        victim: ObjectGuid,
        current_session_id: SessionId,
        attacker: ObjectGuid,
        defense: PlayerMeleeDefenseInput,
        navigation: &DbCreatureNavigationGuardrail,
        now: Instant,
        tick: &mut DbCreatureVictimCombatAdvanceTick,
    ) -> anyhow::Result<bool> {
        let (active, was_fleeing, player_position, player_level) = {
            let mut map_guard = map.lock().await;
            let was_fleeing = map_guard
                .creatures
                .get(&attacker.raw())
                .is_some_and(|creature| creature.is_fleeing());
            let _ = map_guard.advance_db_creature_motion(attacker, now);
            let help_packets = map_guard
                .db_creature_check_for_help_packets_on_relocation(attacker, navigation, now)?;
            Self::split_packets_for_session(current_session_id, help_packets, tick);
            let Some(active) = map_guard.active_db_creature_combat_snapshot(attacker, victim)
            else {
                return Ok(false);
            };
            let Some(player) = map_guard.players.get(&victim.counter()) else {
                return Ok(false);
            };
            (active, was_fleeing, player.position, player.level)
        };

        if was_fleeing && !active.creature.is_fleeing() {
            let body = build_unit_flags_update_body(
                attacker,
                db_creature_unit_flags(&active.creature, true),
            )?;
            let packet = OutboundWorldPacket {
                opcode: WorldOpcode::SmsgUpdateObject as u16,
                body,
            };
            let map_guard = map.lock().await;
            Self::push_creature_broadcast_packet(
                &map_guard,
                victim,
                current_session_id,
                active.creature.current_position,
                packet,
                tick,
            );
        }

        if active_aura_has_hard_control(&active.creature.active_auras)
            || active.creature.is_fleeing()
        {
            map.lock()
                .await
                .defer_ready_db_creature_swing_retry(attacker, victim, now);
            return Ok(false);
        }

        let should_evade = { map.lock().await.db_creature_should_evade(attacker, now) };
        if should_evade {
            let mut map_guard = map.lock().await;
            let Some(creature) = map_guard.prepare_db_creature_evade(attacker) else {
                return Ok(false);
            };
            let attack_stop_packet = OutboundWorldPacket {
                opcode: WorldOpcode::SmsgAttackStop as u16,
                body: build_attack_stop_body(attacker, victim, false)?,
            };
            Self::push_creature_broadcast_packet(
                &map_guard,
                victim,
                current_session_id,
                creature.current_position,
                attack_stop_packet,
                tick,
            );
            let creature_flags_packet = OutboundWorldPacket {
                opcode: WorldOpcode::SmsgUpdateObject as u16,
                body: build_unit_flags_update_body(
                    attacker,
                    db_creature_unit_flags(&creature, false),
                )?,
            };
            Self::push_creature_broadcast_packet(
                &map_guard,
                victim,
                current_session_id,
                creature.current_position,
                creature_flags_packet,
                tick,
            );
            let state_packet = OutboundWorldPacket {
                opcode: WorldOpcode::SmsgUpdateObject as u16,
                body: build_db_creature_state_update_body(attacker, creature.health, 0)?,
            };
            Self::push_creature_broadcast_packet(
                &map_guard,
                victim,
                current_session_id,
                creature.current_position,
                state_packet,
                tick,
            );
            if let Some((returned, motion)) =
                map_guard.start_db_creature_return_home_motion(navigation, attacker, now)
            {
                let packet = OutboundWorldPacket {
                    opcode: WorldOpcode::SmsgMonsterMove as u16,
                    body: build_monster_move_path_body_inner(
                        attacker,
                        motion.start,
                        &motion.path,
                        motion.spline_id,
                        motion.duration.as_millis().max(1) as u32,
                        None,
                        true,
                    )?,
                };
                Self::push_creature_broadcast_packet(
                    &map_guard,
                    victim,
                    current_session_id,
                    returned.current_position,
                    packet,
                    tick,
                );
            }
            return Ok(false);
        }

        let spell_cast_due_at = {
            map.lock()
                .await
                .active_db_creature_spell_cast_due_at(attacker)
        };
        if let Some(due_at) = spell_cast_due_at {
            if now < due_at {
                return Ok(false);
            }
            let mut map_guard = map.lock().await;
            if let Some(event) = map_guard.complete_ready_db_creature_spell_cast_with_navigation(
                attacker, victim, now, navigation,
            )? {
                let player_died = matches!(
                    &event.effect,
                    DbCreatureCompletedSpellEffect::PlayerDamage(damage)
                        if damage.victim_health == 0
                );
                let local_effect = match &event.effect {
                    DbCreatureCompletedSpellEffect::PlayerDamage(damage) => {
                        Some(DbCreatureVictimCombatLocalEffect::SpellDamage {
                            victim_health: damage.victim_health,
                            player_died: damage.victim_health == 0,
                        })
                    }
                    _ => None,
                };
                let packets = map_guard
                    .materialize_db_creature_completed_spell_cast_packets(attacker, victim, event);
                Self::split_packets_for_session(current_session_id, packets, tick);
                if let Some(local_effect) = local_effect {
                    tick.local_effects.push(local_effect);
                }
                if player_died {
                    map_guard.clear_db_creature_combats_for_victim(victim);
                    return Ok(true);
                }
                return Ok(false);
            }
        }

        let event_ai_scripts = object_mgr
            .creature_ai_scripts(world_db_pool, active.creature.spawn.entry)
            .await?;
        if !event_ai_scripts.is_empty() {
            let ready = map.lock().await.ready_db_creature_event_ai_spell_cast(
                attacker,
                victim,
                &event_ai_scripts,
                now,
            );
            if let Some(ready) = ready {
                if let Some(template) = object_mgr
                    .spell_template(world_db_pool, ready.spell_id)
                    .await?
                {
                    let spell_range = self.spell_range(template.range_index);
                    let spell_info = SpellInfo::from_template(&template);
                    let mut map_guard = map.lock().await;
                    if map_guard
                        .validate_db_creature_spell_against_target(
                            attacker,
                            ready.target,
                            navigation,
                            spell_range,
                            spell_info.requires_behind_target(),
                        )
                        .check
                        == DbCreatureSpellTargetCheck::Clear
                    {
                        if let Some(cast) = map_guard.prepare_db_creature_spell_cast_from_template(
                            attacker,
                            ready.target,
                            &template,
                            self.spell_duration(template.duration_index),
                            spell_range,
                            self.spell_cast_time(template.casting_time_index),
                            now,
                        ) {
                            let cast_time_millis = cast.cast_time_millis;
                            let target = cast.target;
                            if let Some(start_packets) =
                                map_guard.start_db_creature_spell_cast(cast)?
                            {
                                map_guard.apply_db_creature_event_ai_spell_cooldown(
                                    attacker, &ready, now,
                                );
                                Self::split_packets_for_session(
                                    current_session_id,
                                    start_packets,
                                    tick,
                                );
                                if cast_time_millis == 0 {
                                    if let Some(event) = map_guard
                                        .complete_ready_db_creature_spell_cast_with_navigation(
                                            attacker, target, now, navigation,
                                        )?
                                    {
                                        let player_died = matches!(
                                            &event.effect,
                                            DbCreatureCompletedSpellEffect::PlayerDamage(damage)
                                                if damage.victim_health == 0
                                        );
                                        let local_effect = match &event.effect {
                                            DbCreatureCompletedSpellEffect::PlayerDamage(
                                                damage,
                                            ) => Some(
                                                DbCreatureVictimCombatLocalEffect::SpellDamage {
                                                    victim_health: damage.victim_health,
                                                    player_died: damage.victim_health == 0,
                                                },
                                            ),
                                            _ => None,
                                        };
                                        let packets = map_guard
                                            .materialize_db_creature_completed_spell_cast_packets(
                                                attacker, target, event,
                                            );
                                        Self::split_packets_for_session(
                                            current_session_id,
                                            packets,
                                            tick,
                                        );
                                        if let Some(local_effect) = local_effect {
                                            tick.local_effects.push(local_effect);
                                        }
                                        if player_died {
                                            map_guard.clear_db_creature_combats_for_victim(victim);
                                            return Ok(true);
                                        }
                                    }
                                }
                                return Ok(false);
                            }
                        }
                    }
                }
            }
        }

        let spell_list_id = if active.creature.spawn.template.spell_list != 0 {
            active.creature.spawn.template.spell_list
        } else {
            active.creature.spawn.template.entry.saturating_mul(100)
        };
        if spell_list_id != 0 {
            let spell_list = object_mgr
                .creature_spell_list(world_db_pool, spell_list_id)
                .await?;
            if !spell_list.is_empty() {
                let condition_cache =
                    load_db_creature_spell_condition_cache(object_mgr, world_db_pool, &spell_list)
                        .await?;
                let ready = map.lock().await.ready_db_creature_spell_cast(
                    attacker,
                    victim,
                    &spell_list,
                    &condition_cache,
                    now,
                );
                if let Some(ready) = ready {
                    if let Some(template) = object_mgr
                        .spell_template(world_db_pool, ready.spell.spell_id)
                        .await?
                    {
                        if SpellInfo::from_template(&template).can_db_creature_autocast() {
                            let spell_range = self.spell_range(template.range_index);
                            let spell_info = SpellInfo::from_template(&template);
                            let mut map_guard = map.lock().await;
                            if map_guard
                                .validate_db_creature_spell_against_target(
                                    attacker,
                                    ready.target,
                                    navigation,
                                    spell_range,
                                    spell_info.requires_behind_target(),
                                )
                                .check
                                == DbCreatureSpellTargetCheck::Clear
                                && db_creature_spell_school_lockout_ready(
                                    &active.creature.spell_school_lockouts_until,
                                    spell_school_mask_from_school(template.school),
                                    now,
                                )
                            {
                                let target = ready.target;
                                let caster_level = active
                                    .creature
                                    .spawn
                                    .template
                                    .max_level
                                    .max(active.creature.spawn.template.min_level);
                                let value_context = SpellEffectValueContext::with_spell_rank_level(
                                    &template,
                                    (caster_level / 5) as i32,
                                    0,
                                );
                                if let Some(plan) =
                                    spell_info.db_creature_spell_plan(target, value_context)
                                {
                                    let aura = (plan.aura && target.is_player()).then(|| {
                                        build_active_aura(
                                            &template,
                                            attacker,
                                            caster_level,
                                            value_context,
                                            now,
                                            self.spell_duration(template.duration_index),
                                        )
                                    });
                                    let effect = match plan.effect {
                                        DbCreatureSpellPlanEffect::Damage {
                                            amount,
                                            school,
                                            dmg_class,
                                            attributes_ex2,
                                            attributes_ex3,
                                        } => ActiveDbCreatureSpellEffect::Damage {
                                            amount,
                                            school,
                                            dmg_class,
                                            attributes_ex2,
                                            attributes_ex3,
                                        },
                                        DbCreatureSpellPlanEffect::Heal { amount } => {
                                            ActiveDbCreatureSpellEffect::Heal { amount }
                                        }
                                        DbCreatureSpellPlanEffect::AuraOnly => {
                                            ActiveDbCreatureSpellEffect::None
                                        }
                                    };
                                    let cast_time_millis = spell_cast_time_millis(
                                        self.spell_cast_time(template.casting_time_index),
                                    );
                                    let cast = ActiveDbCreatureSpellCast {
                                        caster: attacker,
                                        target,
                                        spell_id: plan.spell_id,
                                        school_mask: spell_school_mask_from_school(template.school),
                                        mechanic: template.mechanic,
                                        requires_behind: plan.requires_behind,
                                        effect,
                                        aura,
                                        range: spell_range,
                                        mana_cost: plan.mana_cost,
                                        cast_time_millis,
                                        due_at: now
                                            + Duration::from_millis(cast_time_millis as u64),
                                    };
                                    if let Some(start_packets) =
                                        map_guard.start_db_creature_spell_cast(cast)?
                                    {
                                        map_guard.apply_db_creature_spell_cooldowns(
                                            attacker,
                                            &ready.spell,
                                            &template,
                                            now,
                                        );
                                        Self::split_packets_for_session(
                                            current_session_id,
                                            start_packets,
                                            tick,
                                        );
                                        if cast_time_millis == 0 {
                                            if let Some(event) = map_guard
                                                .complete_ready_db_creature_spell_cast_with_navigation(
                                                    attacker,
                                                    target,
                                                    now,
                                                    navigation,
                                                )?
                                            {
                                                let player_died = matches!(
                                                    &event.effect,
                                                    DbCreatureCompletedSpellEffect::PlayerDamage(
                                                        damage
                                                    ) if damage.victim_health == 0
                                                );
                                                let local_effect = match &event.effect {
                                                    DbCreatureCompletedSpellEffect::PlayerDamage(
                                                        damage,
                                                    ) => Some(
                                                        DbCreatureVictimCombatLocalEffect::SpellDamage {
                                                            victim_health: damage.victim_health,
                                                            player_died: damage.victim_health == 0,
                                                        },
                                                    ),
                                                    _ => None,
                                                };
                                                let packets = map_guard
                                                    .materialize_db_creature_completed_spell_cast_packets(
                                                        attacker,
                                                        target,
                                                        event,
                                                    );
                                                Self::split_packets_for_session(
                                                    current_session_id,
                                                    packets,
                                                    tick,
                                                );
                                                if let Some(local_effect) = local_effect {
                                                    tick.local_effects.push(local_effect);
                                                }
                                                if player_died {
                                                    map_guard
                                                        .clear_db_creature_combats_for_victim(victim);
                                                    return Ok(true);
                                                }
                                            }
                                        }
                                        return Ok(false);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let can_reach = map
            .lock()
            .await
            .db_creature_can_reach_player_with_navigation(attacker, victim, navigation);
        if !can_reach {
            let mut map_guard = map.lock().await;
            let _ = map_guard.defer_ready_db_creature_swing_retry(attacker, victim, now);
            if let Some((creature, motion)) = map_guard.start_db_creature_chase_motion(
                navigation,
                attacker,
                victim,
                player_position,
                now,
            ) {
                let packet = OutboundWorldPacket {
                    opcode: WorldOpcode::SmsgMonsterMove as u16,
                    body: build_monster_move_facing_target_path_body_with_run(
                        attacker,
                        motion.start,
                        &motion.path,
                        motion.spline_id,
                        motion.duration.as_millis().max(1) as u32,
                        victim,
                        motion.run,
                    )?,
                };
                Self::push_creature_broadcast_packet(
                    &map_guard,
                    victim,
                    current_session_id,
                    creature.current_position,
                    packet,
                    tick,
                );
            }
            return Ok(false);
        }

        if !map
            .lock()
            .await
            .db_creature_has_player_in_arc(attacker, victim)
        {
            let mut map_guard = map.lock().await;
            if let Some((creature, position, spline_id)) =
                map_guard.face_db_creature_toward_position(attacker, player_position)
            {
                let packet = OutboundWorldPacket {
                    opcode: WorldOpcode::SmsgMonsterMove as u16,
                    body: build_monster_move_facing_target_body(
                        attacker, position, position, spline_id, 1, victim,
                    )?,
                };
                Self::push_creature_broadcast_packet(
                    &map_guard,
                    victim,
                    current_session_id,
                    creature.current_position,
                    packet,
                    tick,
                );
            }
            let _ = map_guard.defer_ready_db_creature_swing_retry(attacker, victim, now);
            return Ok(false);
        }

        if now < active.combat.next_swing_at {
            return Ok(false);
        }

        let next_swing_delay = active.creature.base_attack_duration();
        let outcome = active.creature.melee_outcome_against_player(defense);
        let mut map_guard = map.lock().await;
        let Some(event) = map_guard.apply_db_creature_player_melee_outcome(
            attacker,
            victim,
            outcome,
            now,
            now + next_swing_delay,
        )?
        else {
            map_guard.clear_db_creature_combat(attacker);
            return Ok(false);
        };
        if Self::push_db_creature_player_melee_event(tick, attacker, player_level, event) {
            map_guard.clear_db_creature_combats_for_victim(victim);
            return Ok(true);
        }
        Ok(false)
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn apply_db_creature_player_damage(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        damage: u32,
        now: Instant,
        next_swing_at: Instant,
    ) -> anyhow::Result<Option<DbCreaturePlayerDamageEvent>> {
        let map = self.get_or_create_map(map_id, 0).await;
        let event = map.lock().await.apply_db_creature_player_damage(
            attacker,
            victim,
            damage,
            now,
            next_swing_at,
        );
        event
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn apply_db_creature_player_melee_outcome(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        outcome: MeleeDamageOutcome,
        now: Instant,
        next_swing_at: Instant,
    ) -> anyhow::Result<Option<DbCreaturePlayerDamageEvent>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(None);
        };
        let event = map.lock().await.apply_db_creature_player_melee_outcome(
            attacker,
            victim,
            outcome,
            now,
            next_swing_at,
        );
        event
    }

    #[cfg(test)]
    pub(in crate::world) async fn apply_db_creature_player_melee_outcome_as_victim_tick(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        outcome: MeleeDamageOutcome,
        now: Instant,
        next_swing_at: Instant,
    ) -> anyhow::Result<DbCreatureVictimCombatAdvanceTick> {
        let map = self.get_or_create_map(map_id, 0).await;
        let mut tick = DbCreatureVictimCombatAdvanceTick::default();
        let (event, player_level) = {
            let mut map_guard = map.lock().await;
            let player_level = map_guard
                .players
                .get(&victim.counter())
                .map(|player| player.level)
                .unwrap_or(1);
            let Some(event) = map_guard.apply_db_creature_player_melee_outcome(
                attacker,
                victim,
                outcome,
                now,
                next_swing_at,
            )?
            else {
                return Ok(tick);
            };
            (event, player_level)
        };
        let player_died =
            Self::push_db_creature_player_melee_event(&mut tick, attacker, player_level, event);
        let mut map_guard = map.lock().await;
        if player_died {
            map_guard.clear_db_creature_combats_for_victim(victim);
        }
        tick.active_combats = map_guard.active_db_creature_combats_for_victim(victim);
        tick.player_in_combat = map_guard
            .players
            .get(&victim.counter())
            .is_some_and(|player| player.in_combat);
        Ok(tick)
    }
}
