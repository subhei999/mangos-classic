use super::*;

// Shared DB-creature damage authority and observer packet production.

impl MapRuntime {
    #[allow(dead_code)]
    pub(in crate::world) fn apply_db_creature_aura(
        &mut self,
        creature_guid: ObjectGuid,
        caster_character_guid: u32,
        aura: ActiveAura,
        now: Instant,
    ) -> anyhow::Result<Option<DbCreatureAuraUpdateEvent>> {
        self.apply_db_creature_aura_replacing_spell_ids(
            creature_guid,
            caster_character_guid,
            aura,
            &[],
            now,
        )
    }

    pub(in crate::world) fn apply_db_creature_aura_replacing_spell_ids(
        &mut self,
        creature_guid: ObjectGuid,
        caster_character_guid: u32,
        aura: ActiveAura,
        replace_spell_ids: &[u32],
        now: Instant,
    ) -> anyhow::Result<Option<DbCreatureAuraUpdateEvent>> {
        let resolution = AuraRankConflictResolution {
            failure: None,
            replace_spell_ids: replace_spell_ids.to_vec(),
            replace_any_caster_spell_ids: Vec::new(),
        };
        self.apply_db_creature_aura_replacing_conflicts(
            creature_guid,
            caster_character_guid,
            aura,
            &resolution,
            now,
        )
    }

    pub(in crate::world) fn apply_db_creature_aura_replacing_conflicts(
        &mut self,
        creature_guid: ObjectGuid,
        caster_character_guid: u32,
        aura: ActiveAura,
        resolution: &AuraRankConflictResolution,
        now: Instant,
    ) -> anyhow::Result<Option<DbCreatureAuraUpdateEvent>> {
        let Some(creature) = self.creatures.get_mut(&creature_guid.raw()) else {
            return Ok(None);
        };
        if !creature.is_alive() {
            return Ok(None);
        }
        let old_attack_duration = creature.base_attack_duration();
        let old_speeds = creature.move_speeds;
        let was_rooted = active_aura_has_root(&creature.active_auras);
        apply_active_aura_replacing_conflicts(&mut creature.active_auras, aura, resolution);
        let is_rooted = active_aura_has_root(&creature.active_auras);
        let previous_speeds = creature.refresh_move_speeds();
        debug_assert_eq!(old_speeds, previous_speeds);
        let stop_packet = if !was_rooted
            && is_rooted
            && !matches!(creature.motion, CreatureMotionState::Idle)
        {
            let stop = stop_db_creature_motion_runtime(creature);
            Some(OutboundWorldPacket {
                opcode: SMSG_MONSTER_MOVE,
                body: build_monster_move_stop_body(creature_guid, stop.position, stop.spline_id)?,
            })
        } else {
            None
        };
        let new_attack_duration = creature.base_attack_duration();
        let active_auras = creature.active_auras.clone();
        let mut direct_packets =
            db_creature_aura_runtime_packets(creature_guid, creature, old_speeds, now)?;
        if let Some(packet) = stop_packet {
            direct_packets.push(packet);
        }
        let position = creature.current_position;
        self.adjust_db_creature_attack_timer_for_base_time_change(
            creature_guid,
            old_attack_duration,
            new_attack_duration,
            now,
        );
        let update_body = build_db_creature_aura_update_body(creature_guid, &active_auras)?;
        let runtime_packets = direct_packets.clone();
        let mut observer_packets: Vec<(SessionId, OutboundWorldPacket)> = self
            .nearby_player_guids(
                position,
                CREATURE_SPAWN_RADIUS_YARDS,
                Some(caster_character_guid),
            )
            .into_iter()
            .filter_map(|player_guid| {
                self.players.get(&player_guid).and_then(|player| {
                    player.packet_to_client(OutboundWorldPacket {
                        opcode: SMSG_UPDATE_OBJECT,
                        body: update_body.clone(),
                    })
                })
            })
            .collect();
        for packet in runtime_packets {
            observer_packets.extend(
                self.nearby_player_guids(
                    position,
                    CREATURE_SPAWN_RADIUS_YARDS,
                    Some(caster_character_guid),
                )
                .into_iter()
                .filter_map(|player_guid| {
                    self.players
                        .get(&player_guid)
                        .and_then(|player| player.packet_to_client(packet.clone()))
                }),
            );
        }
        Ok(Some(DbCreatureAuraUpdateEvent {
            update_body,
            direct_packets,
            observer_packets,
        }))
    }

    pub(in crate::world) fn adjust_db_creature_attack_timer_for_base_time_change(
        &mut self,
        creature_guid: ObjectGuid,
        old_duration: Duration,
        new_duration: Duration,
        now: Instant,
    ) {
        if old_duration == new_duration {
            return;
        }
        let Some(combat) = self.active_creature_combats.get_mut(&creature_guid.raw()) else {
            return;
        };
        let old_millis = old_duration.as_millis() as i128;
        let new_millis = new_duration.as_millis() as i128;
        let diff = new_millis - old_millis;
        if diff >= 0 {
            combat.next_swing_at += Duration::from_millis(diff as u64);
            return;
        }
        let remaining = combat.next_swing_at.saturating_duration_since(now);
        let reduction = Duration::from_millis(diff.unsigned_abs() as u64);
        combat.next_swing_at = if reduction >= remaining {
            now
        } else {
            combat.next_swing_at - reduction
        };
    }

    pub(in crate::world) fn advance_db_creature_auras(
        &mut self,
        now: Instant,
        now_epoch_secs: u64,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let creature_guids = self.creatures.keys().copied().collect::<Vec<_>>();
        let mut packets = Vec::new();
        let mut threat_updates = Vec::new();
        let mut attack_timer_adjustments = Vec::new();
        for raw_guid in creature_guids {
            let creature_guid = ObjectGuid::from_raw(raw_guid);
            let Some(creature) = self.creatures.get_mut(&raw_guid) else {
                continue;
            };
            if !creature.is_alive() {
                continue;
            }

            let old_speeds = creature.move_speeds;
            let old_attack_duration = creature.base_attack_duration();
            let before = creature.active_auras.len();
            let mut aura_changed = false;
            let target_snapshot = db_creature_spell_snapshot(creature);

            let mut tick_packets = Vec::new();
            let mut pending_damage_ticks = Vec::new();
            let mut died_from_pending_tick = false;
            for aura in &mut creature.active_auras {
                let Some(periodic) = aura.periodic_damage.as_mut() else {
                    continue;
                };
                if aura
                    .expires_at
                    .is_some_and(|expires_at| periodic.next_tick_at > expires_at)
                {
                    continue;
                }
                if now < periodic.next_tick_at {
                    continue;
                }
                while periodic.next_tick_at <= now {
                    periodic.next_tick_at += Duration::from_millis(periodic.tick_millis as u64);
                }
                let caster_snapshot = periodic_spell_caster_snapshot(&self.players, aura.caster)
                    .unwrap_or(periodic.caster_snapshot);
                let tick = calculate_periodic_damage_tick(
                    periodic,
                    caster_snapshot,
                    target_snapshot,
                    creature.health,
                );
                if tick.dealt_damage == 0 {
                    continue;
                }
                pending_damage_ticks.push((aura.caster, aura.spell_id, periodic.aura_name, tick));
            }
            for (caster, spell_id, aura_name, tick) in pending_damage_ticks {
                let Some(applied) = apply_creature_runtime_world_damage(
                    creature,
                    creature_guid,
                    caster,
                    tick.dealt_damage,
                    WorldDamageKind::PeriodicAura,
                    now,
                    now_epoch_secs,
                )?
                else {
                    continue;
                };
                if applied.remaining_health > 0 {
                    threat_updates.push((creature_guid, caster, tick.threat));
                }
                tick_packets.push((
                    caster,
                    build_periodic_aura_log_body(PeriodicAuraLog {
                        creature_guid,
                        caster,
                        spell_id,
                        aura_name,
                        tick,
                    })?,
                ));
                if applied.died {
                    died_from_pending_tick = true;
                    self.active_creature_combats.remove(&raw_guid);
                    self.active_creature_spell_casts.remove(&raw_guid);
                    self.creature_combat_leash.remove(&raw_guid);
                    self.creature_threats.remove(&raw_guid);
                    break;
                }
            }
            creature
                .active_auras
                .retain(|aura| aura.expires_at.is_none_or(|expires_at| now < expires_at));
            aura_changed |= creature.active_auras.len() != before;
            let died_from_aura = died_from_pending_tick || creature.health == 0;
            let runtime_packets = if aura_changed && !died_from_aura {
                let previous_speeds = creature.refresh_move_speeds();
                debug_assert_eq!(old_speeds, previous_speeds);
                let new_attack_duration = creature.base_attack_duration();
                attack_timer_adjustments.push((
                    creature_guid,
                    old_attack_duration,
                    new_attack_duration,
                ));
                db_creature_aura_runtime_packets(creature_guid, creature, old_speeds, now)?
            } else {
                Vec::new()
            };

            if aura_changed || !tick_packets.is_empty() {
                let update_body = if creature.health == 0 {
                    build_db_creature_death_update_body(
                        creature_guid,
                        creature.dynamic_flags(),
                        db_creature_unit_flags(creature, false),
                    )?
                } else {
                    build_db_creature_aura_state_update_body(
                        creature_guid,
                        &creature.active_auras,
                        creature.health,
                        creature.dynamic_flags(),
                    )?
                };
                let position = creature.current_position;
                let nearby = self
                    .nearby_player_guids(position, CREATURE_SPAWN_RADIUS_YARDS, None)
                    .into_iter()
                    .filter_map(|player_guid| {
                        self.players
                            .get(&player_guid)
                            .and_then(PlayerRuntime::client_session_id)
                    })
                    .collect::<Vec<_>>();
                for session_id in nearby {
                    for (_, log_body) in &tick_packets {
                        packets.push((
                            session_id,
                            OutboundWorldPacket {
                                opcode: SMSG_PERIODICAURALOG,
                                body: log_body.clone(),
                            },
                        ));
                    }
                    packets.push((
                        session_id,
                        OutboundWorldPacket {
                            opcode: SMSG_UPDATE_OBJECT,
                            body: update_body.clone(),
                        },
                    ));
                    for packet in &runtime_packets {
                        packets.push((session_id, packet.clone()));
                    }
                }
            }
            if died_from_aura {
                packets.extend(self.clear_player_melee_state_for_dead_target(creature_guid, None)?);
            }
        }
        for (creature_guid, victim, threat) in threat_updates {
            self.add_db_creature_threat(creature_guid, victim, threat);
            if self
                .active_creature_combats
                .contains_key(&creature_guid.raw())
            {
                self.refresh_db_creature_combat_leash(creature_guid, now);
            }
        }
        for (creature_guid, old_duration, new_duration) in attack_timer_adjustments {
            self.adjust_db_creature_attack_timer_for_base_time_change(
                creature_guid,
                old_duration,
                new_duration,
                now,
            );
        }
        Ok(packets)
    }

    pub(in crate::world) fn apply_db_creature_damage(
        &mut self,
        request: DbCreatureDamageRequest,
    ) -> anyhow::Result<Option<DbCreatureDamageEvent>> {
        let creature_guid = request.creature_guid;
        let Some(creature) = self.creatures.get(&creature_guid.raw()) else {
            return Ok(None);
        };
        if !creature.is_alive() || creature.is_evading_home() {
            return Ok(None);
        }
        let requested_damage = request
            .melee_outcome
            .map(|outcome| outcome.total_damage)
            .or_else(|| {
                request
                    .spell_damage_outcome
                    .map(|outcome| outcome.final_damage)
            })
            .unwrap_or_else(|| request.damage.max(1));
        let attacker_rage_damage = if request.melee_outcome.is_some() {
            requested_damage
        } else {
            0
        };
        let motion_stop_packet = {
            let Some(creature) = self.creatures.get_mut(&creature_guid.raw()) else {
                return Ok(None);
            };
            let will_die = requested_damage >= creature.health;
            if will_die && !matches!(creature.motion, CreatureMotionState::Idle) {
                let stop = stop_db_creature_motion_runtime(creature);
                Some(OutboundWorldPacket {
                    opcode: SMSG_MONSTER_MOVE,
                    body: build_monster_move_stop_body(
                        creature_guid,
                        stop.position,
                        stop.spline_id,
                    )?,
                })
            } else {
                None
            }
        };
        let Some(applied) = self.apply_creature_world_damage(
            creature_guid,
            request.killer,
            requested_damage,
            if request.spell_damage_outcome.is_some() {
                WorldDamageKind::SpellDirect
            } else {
                WorldDamageKind::Melee
            },
            request.now,
            request.now_epoch_secs,
        )?
        else {
            return Ok(None);
        };
        let damage = applied.applied_damage;
        let is_dead = applied.died;
        let Some(creature) = self.creatures.get_mut(&creature_guid.raw()) else {
            return Ok(None);
        };
        if is_dead {
            if let Some(corpse_loot) = request.corpse_loot {
                creature.loot_owner.get_or_insert(corpse_loot.owner);
                creature.loot_allowed_players = corpse_loot.allowed_players.into_iter().collect();
                creature.loot_current_looter = corpse_loot
                    .current_looter
                    .or_else(|| request.killer.is_player().then(|| request.killer.counter()));
                creature.loot_method = corpse_loot.loot_method;
                creature.loot_items = loot_items_with_stable_slots(corpse_loot.loot_items);
                creature.loot_items_generated = true;
                if !creature.can_loot_for_player(None) {
                    creature.lootable = false;
                }
            }
        }
        let creature = creature.clone();
        self.add_db_creature_threat(creature_guid, request.killer, damage as f32);
        if is_dead {
            self.active_creature_combats.remove(&creature_guid.raw());
            self.active_creature_spell_casts
                .remove(&creature_guid.raw());
            self.creature_combat_leash.remove(&creature_guid.raw());
            self.creature_threats.remove(&creature_guid.raw());
        } else if self
            .active_creature_combats
            .contains_key(&creature_guid.raw())
        {
            self.refresh_db_creature_combat_leash(creature_guid, request.now);
        }
        let player_melee_cleanup_packets = if is_dead {
            self.clear_player_melee_state_for_dead_target(
                creature_guid,
                request.exclude_character_guid,
            )?
        } else {
            Vec::new()
        };
        self.refresh_grid_state(grid_coord_for_position(creature.current_position));
        let update_body = if is_dead {
            build_db_creature_death_update_body(
                creature_guid,
                creature.dynamic_flags_for_player(
                    request.killer.is_player().then(|| request.killer.counter()),
                ),
                db_creature_unit_flags(&creature, false),
            )?
        } else {
            build_db_creature_state_update_body(
                creature_guid,
                creature.health,
                creature.dynamic_flags(),
            )?
        };
        let nearby_observers = self
            .nearby_player_guids(
                creature.current_position,
                CREATURE_SPAWN_RADIUS_YARDS,
                request.exclude_character_guid,
            )
            .into_iter()
            .filter_map(|player_guid| {
                self.players
                    .get(&player_guid)
                    .and_then(PlayerRuntime::client_session_id)
                    .map(|session_id| (player_guid, session_id))
            })
            .collect::<Vec<_>>();
        let attacker_state_body = if request.suppress_attacker_state {
            None
        } else if let Some(outcome) = request.melee_outcome {
            let mut outcome = outcome;
            // Preserve attacker packet overkill semantics from the original melee outcome.
            outcome.total_damage = requested_damage;
            outcome.school_damage = outcome.school_damage.min(requested_damage);
            Some(build_attacker_state_update_body_for_outcome(
                request.killer,
                creature_guid,
                outcome,
                request.spell_id.unwrap_or(0),
            )?)
        } else if let Some(spell_id) = request.spell_id {
            Some(build_attacker_state_update_body_with_spell_id(
                request.killer,
                creature_guid,
                requested_damage,
                spell_id,
            )?)
        } else {
            Some(build_attacker_state_update_body(
                request.killer,
                creature_guid,
                requested_damage,
            )?)
        };
        let spell_non_melee_log_body = request
            .spell_id
            .filter(|_| {
                if let Some(outcome) = request.spell_damage_outcome {
                    outcome.miss_info.is_none()
                } else {
                    request.melee_outcome.is_none()
                        || (request.suppress_attacker_state && requested_damage > 0)
                }
            })
            .map(|spell_id| {
                let (absorb, resist, blocked, hit_info) = request
                    .spell_damage_outcome
                    .map(|outcome| {
                        (
                            outcome.absorb,
                            outcome.resist,
                            outcome.blocked,
                            outcome.hit_info,
                        )
                    })
                    .or_else(|| {
                        request.melee_outcome.map(|outcome| {
                            let hit_info = if outcome.hit_info & HITINFO_CRITICALHIT != 0 {
                                SPELL_HIT_TYPE_CRIT
                            } else {
                                0
                            };
                            (
                                outcome.absorbed,
                                outcome.resisted,
                                outcome.blocked,
                                hit_info,
                            )
                        })
                    })
                    .unwrap_or((0, 0, 0, 0));
                build_spell_non_melee_damage_log_body(SpellNonMeleeDamageLogPacket {
                    attacker: request.killer,
                    target: creature_guid,
                    spell_id,
                    damage: requested_damage,
                    school: request.spell_school,
                    absorb,
                    resist,
                    periodic: false,
                    blocked,
                    hit_info,
                })
            })
            .transpose()?;
        let spell_miss_log_body = request
            .spell_id
            .zip(
                request
                    .spell_damage_outcome
                    .and_then(|outcome| outcome.miss_info)
                    .or_else(|| {
                        request
                            .melee_outcome
                            .and_then(|outcome| outcome.spell_miss_info())
                    }),
            )
            .map(|(spell_id, miss_info)| {
                build_spell_log_miss_body(request.killer, creature_guid, spell_id, miss_info)
            })
            .transpose()?;
        let mut observer_packets: Vec<(SessionId, OutboundWorldPacket)> = nearby_observers
            .iter()
            .copied()
            .flat_map(|(player_guid, session_id)| {
                let mut packets = Vec::with_capacity(3);
                if let Some(spell_miss_log_body) = &spell_miss_log_body {
                    packets.push((
                        session_id,
                        OutboundWorldPacket {
                            opcode: SMSG_SPELLLOGMISS,
                            body: spell_miss_log_body.clone(),
                        },
                    ));
                }
                if let Some(spell_non_melee_log_body) = &spell_non_melee_log_body {
                    packets.push((
                        session_id,
                        OutboundWorldPacket {
                            opcode: SMSG_SPELLNONMELEEDAMAGELOG,
                            body: spell_non_melee_log_body.clone(),
                        },
                    ));
                }
                if let Some(attacker_state_body) = &attacker_state_body {
                    packets.push((
                        session_id,
                        OutboundWorldPacket {
                            opcode: SMSG_ATTACKERSTATEUPDATE,
                            body: attacker_state_body.clone(),
                        },
                    ));
                }
                let update_body = if is_dead {
                    build_db_creature_death_update_body(
                        creature_guid,
                        creature.dynamic_flags_for_player(Some(player_guid)),
                        db_creature_unit_flags(&creature, false),
                    )
                    .unwrap_or_else(|_| update_body.clone())
                } else {
                    update_body.clone()
                };
                packets.push((
                    session_id,
                    OutboundWorldPacket {
                        opcode: SMSG_UPDATE_OBJECT,
                        body: update_body,
                    },
                ));
                packets
            })
            .collect();
        observer_packets.extend(player_melee_cleanup_packets);
        let death_finalization = if is_dead {
            let combat_flag_packet = OutboundWorldPacket {
                opcode: SMSG_UPDATE_OBJECT,
                body: build_unit_flags_update_body(
                    creature_guid,
                    db_creature_unit_flags(&creature, false),
                )?,
            };
            let attack_stop_packet = OutboundWorldPacket {
                opcode: SMSG_ATTACKSTOP,
                body: build_attack_stop_body(request.killer, creature_guid, false)?,
            };
            let observer_packets = nearby_observers
                .into_iter()
                .flat_map(|(_, session_id)| {
                    motion_stop_packet
                        .iter()
                        .cloned()
                        .chain([combat_flag_packet.clone(), attack_stop_packet.clone()])
                        .map(move |packet| (session_id, packet))
                })
                .collect();
            Some(DbCreatureDeathFinalizationEvent {
                killed: creature_guid,
                respawn_epoch_secs: creature.respawn_epoch_secs,
                motion_stop_packet,
                attack_stop_packet,
                combat_flag_packet,
                observer_packets,
            })
        } else {
            None
        };
        let target_switch = if is_dead {
            None
        } else {
            self.switch_db_creature_threat_victim_if_needed(
                creature_guid,
                request.exclude_character_guid,
            )?
        };
        Ok(Some(DbCreatureDamageEvent {
            damage,
            attacker_rage_damage,
            creature,
            attacker_state_body,
            spell_non_melee_log_body,
            spell_miss_log_body,
            update_body,
            death_finalization,
            target_switch,
            observer_packets,
        }))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::world) struct PeriodicDamageTick {
    pub(in crate::world) school: u32,
    pub(in crate::world) requested_damage: u32,
    pub(in crate::world) dealt_damage: u32,
    pub(in crate::world) absorb: u32,
    pub(in crate::world) resist: i32,
    pub(in crate::world) threat: f32,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct PeriodicAuraLog {
    pub(in crate::world) creature_guid: ObjectGuid,
    pub(in crate::world) caster: ObjectGuid,
    pub(in crate::world) spell_id: u32,
    pub(in crate::world) aura_name: u32,
    pub(in crate::world) tick: PeriodicDamageTick,
}

pub(in crate::world) fn calculate_periodic_damage_tick(
    periodic: &PeriodicDamageAura,
    caster: SpellCombatUnitSnapshot,
    target: SpellCombatUnitSnapshot,
    target_health: u32,
) -> PeriodicDamageTick {
    let mut rng = rand::thread_rng();
    calculate_periodic_damage_tick_with_rolls(
        periodic,
        caster,
        target,
        target_health,
        SpellDamageOutcomeRolls {
            hit_roll: rng.gen_range(1..=10_000),
            crit_roll: rng.gen_range(1..=10_000),
            partial_resist_roll: rng.gen_range(1..=10_000),
        },
    )
}

pub(in crate::world) fn calculate_periodic_damage_tick_with_rolls(
    periodic: &PeriodicDamageAura,
    caster: SpellCombatUnitSnapshot,
    target: SpellCombatUnitSnapshot,
    target_health: u32,
    rolls: SpellDamageOutcomeRolls,
) -> PeriodicDamageTick {
    let requested_damage = periodic.amount.max(1);
    let outcome = calculate_spell_damage_outcome(
        spell_damage_outcome_input(
            requested_damage,
            periodic.school as u8,
            periodic.damage_class,
            periodic.attributes_ex2,
            periodic.attributes_ex3,
            caster,
            target,
        ),
        rolls,
    );
    let absorb = outcome.absorb;
    let resist = outcome.resist;
    let effective_damage = outcome.final_damage;
    let dealt_damage = target_health.min(effective_damage);
    let threat = dealt_damage as f32;
    PeriodicDamageTick {
        school: periodic.school,
        requested_damage,
        dealt_damage,
        absorb,
        resist,
        threat,
    }
}

pub(in crate::world) fn periodic_spell_caster_snapshot(
    players: &HashMap<u32, PlayerRuntime>,
    caster: ObjectGuid,
) -> Option<SpellCombatUnitSnapshot> {
    if !caster.is_player() {
        return None;
    }
    let player = players.get(&caster.counter())?;
    Some(player_spell_snapshot(
        player.level,
        player.class,
        &player.combat_stats,
    ))
}

pub(in crate::world) fn db_creature_aura_runtime_packets(
    creature_guid: ObjectGuid,
    creature: &mut DbCreatureRuntime,
    old_speeds: UnitMoveSpeeds,
    now: Instant,
) -> anyhow::Result<Vec<OutboundWorldPacket>> {
    let mut packets =
        db_creature_speed_change_packets(creature_guid, old_speeds, creature.move_speeds)?;
    if db_creature_motion_speed_changed(&creature.motion, old_speeds, creature.move_speeds) {
        if let Some(packet) = retime_db_creature_motion_for_speed_change(creature, now)? {
            packets.push(packet);
        }
    }
    Ok(packets)
}

pub(in crate::world) fn db_creature_speed_change_packets(
    creature_guid: ObjectGuid,
    old_speeds: UnitMoveSpeeds,
    new_speeds: UnitMoveSpeeds,
) -> anyhow::Result<Vec<OutboundWorldPacket>> {
    let mut packets = Vec::new();
    push_speed_change_packet(
        &mut packets,
        creature_guid,
        SMSG_SPLINE_SET_WALK_SPEED,
        old_speeds.walk,
        new_speeds.walk,
    )?;
    push_speed_change_packet(
        &mut packets,
        creature_guid,
        SMSG_SPLINE_SET_RUN_SPEED,
        old_speeds.run,
        new_speeds.run,
    )?;
    push_speed_change_packet(
        &mut packets,
        creature_guid,
        SMSG_SPLINE_SET_RUN_BACK_SPEED,
        old_speeds.run_back,
        new_speeds.run_back,
    )?;
    push_speed_change_packet(
        &mut packets,
        creature_guid,
        SMSG_SPLINE_SET_SWIM_SPEED,
        old_speeds.swim,
        new_speeds.swim,
    )?;
    push_speed_change_packet(
        &mut packets,
        creature_guid,
        SMSG_SPLINE_SET_SWIM_BACK_SPEED,
        old_speeds.swim_back,
        new_speeds.swim_back,
    )?;
    Ok(packets)
}

pub(in crate::world) fn push_speed_change_packet(
    packets: &mut Vec<OutboundWorldPacket>,
    creature_guid: ObjectGuid,
    opcode: u16,
    old_speed: f32,
    new_speed: f32,
) -> anyhow::Result<()> {
    if (old_speed - new_speed).abs() <= f32::EPSILON {
        return Ok(());
    }
    packets.push(OutboundWorldPacket {
        opcode,
        body: build_spline_set_speed_body(creature_guid, new_speed)?,
    });
    Ok(())
}

pub(in crate::world) fn db_creature_motion_speed_changed(
    motion: &CreatureMotionState,
    old_speeds: UnitMoveSpeeds,
    new_speeds: UnitMoveSpeeds,
) -> bool {
    match motion {
        CreatureMotionState::Random(_) | CreatureMotionState::Waypoint(_) => {
            (old_speeds.walk - new_speeds.walk).abs() > f32::EPSILON
        }
        CreatureMotionState::Chase(_) | CreatureMotionState::ReturnHome(_) => {
            (old_speeds.run - new_speeds.run).abs() > f32::EPSILON
        }
        CreatureMotionState::Idle => false,
    }
}

pub(in crate::world) fn build_periodic_aura_log_body(
    log: PeriodicAuraLog,
) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(40);
    PackedGuid::write(&mut body, log.creature_guid)?;
    PackedGuid::write(&mut body, log.caster)?;
    body.extend_from_slice(&log.spell_id.to_le_bytes());
    body.extend_from_slice(&1u32.to_le_bytes());
    body.extend_from_slice(&log.aura_name.to_le_bytes());
    body.extend_from_slice(&log.tick.dealt_damage.to_le_bytes());
    body.extend_from_slice(&log.tick.school.to_le_bytes());
    body.extend_from_slice(&log.tick.absorb.to_le_bytes());
    body.extend_from_slice(&log.tick.resist.to_le_bytes());
    Ok(body)
}

pub(in crate::world) fn build_db_creature_aura_state_update_body(
    creature: ObjectGuid,
    active_auras: &[ActiveAura],
    health: u32,
    dynamic_flags: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, creature)?;

    let mut values = vec![None; PLAYER_END_FIELDS];
    set_unit_aura_update_values(&mut values, active_auras)?;
    set_update_value(&mut values, UNIT_FIELD_HEALTH, health)?;
    set_update_value(&mut values, UNIT_DYNAMIC_FLAGS, dynamic_flags)?;

    write_update_values(&mut block, &values)?;
    Ok(build_update_object_body(&[block]))
}
