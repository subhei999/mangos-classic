// Shared DB-creature damage authority and observer packet production.

impl MapRuntime {
    fn apply_db_creature_aura(
        &mut self,
        creature_guid: ObjectGuid,
        caster_character_guid: u32,
        aura: ActiveAura,
    ) -> anyhow::Result<Option<DbCreatureAuraUpdateEvent>> {
        let Some(creature) = self.creatures.get_mut(&creature_guid.raw()) else {
            return Ok(None);
        };
        if !creature.is_alive() {
            return Ok(None);
        }
        apply_active_aura(&mut creature.active_auras, aura);
        let active_auras = creature.active_auras.clone();
        let position = creature.current_position;
        let update_body = build_db_creature_aura_update_body(creature_guid, &active_auras)?;
        let observer_packets = self
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
        Ok(Some(DbCreatureAuraUpdateEvent {
            update_body,
            observer_packets,
        }))
    }

    fn advance_db_creature_auras(
        &mut self,
        now: Instant,
        now_epoch_secs: u64,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let creature_guids = self.creatures.keys().copied().collect::<Vec<_>>();
        let mut packets = Vec::new();
        let mut threat_updates = Vec::new();
        for raw_guid in creature_guids {
            let creature_guid = ObjectGuid::from_raw(raw_guid);
            let Some(creature) = self.creatures.get_mut(&raw_guid) else {
                continue;
            };
            if !creature.is_alive() {
                continue;
            }

            let before = creature.active_auras.len();
            let mut aura_changed = false;

            let mut tick_packets = Vec::new();
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
                let tick = calculate_periodic_damage_tick(periodic, creature.health);
                if tick.dealt_damage == 0 {
                    continue;
                }
                creature.health = creature.health.saturating_sub(tick.dealt_damage);
                if creature.health > 0 {
                    threat_updates.push((creature_guid, aura.caster, tick.threat));
                }
                tick_packets.push((
                    aura.caster,
                    build_periodic_aura_log_body(PeriodicAuraLog {
                        creature_guid,
                        caster: aura.caster,
                        spell_id: aura.spell_id,
                        aura_name: periodic.aura_name,
                        tick,
                    })?,
                ));
                if creature.health == 0 {
                    creature.begin_corpse(now, now_epoch_secs);
                    self.active_creature_combats.remove(&raw_guid);
                    self.creature_combat_leash.remove(&raw_guid);
                    self.creature_threats.remove(&raw_guid);
                    break;
                }
            }
            creature
                .active_auras
                .retain(|aura| aura.expires_at.is_none_or(|expires_at| now < expires_at));
            aura_changed |= creature.active_auras.len() != before;

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
                }
            }
        }
        for (creature_guid, victim, threat) in threat_updates {
            self.add_db_creature_threat(creature_guid, victim, threat);
            if self.active_creature_combats.contains_key(&creature_guid.raw()) {
                self.refresh_db_creature_combat_leash(creature_guid, now);
            }
        }
        Ok(packets)
    }

    fn apply_db_creature_damage(
        &mut self,
        request: DbCreatureDamageRequest,
    ) -> anyhow::Result<Option<DbCreatureDamageEvent>> {
        let creature_guid = request.creature_guid;
        let Some(creature) = self.creatures.get_mut(&creature_guid.raw()) else {
            return Ok(None);
        };
        if !creature.is_alive() || creature.is_evading_home() {
            return Ok(None);
        }
        let requested_damage = request
            .melee_outcome
            .map(|outcome| outcome.total_damage)
            .unwrap_or_else(|| request.damage.max(1));
        let attacker_rage_damage = if request.melee_outcome.is_some() {
            requested_damage
        } else {
            0
        };
        let damage = creature.health.min(requested_damage);
        creature.health = creature.health.saturating_sub(damage);
        let is_dead = creature.health == 0;
        let motion_stop_packet = if is_dead && !matches!(creature.motion, CreatureMotionState::Idle)
        {
            let stop = stop_db_creature_motion_runtime(creature);
            Some(OutboundWorldPacket {
                opcode: SMSG_MONSTER_MOVE,
                body: build_monster_move_stop_body(creature_guid, stop.position, stop.spline_id)?,
            })
        } else {
            None
        };
        if is_dead {
            creature.begin_corpse(request.now, request.now_epoch_secs);
            if let Some(corpse_loot) = request.corpse_loot {
                creature.loot_owner.get_or_insert(corpse_loot.owner);
                creature.loot_allowed_players = corpse_loot.allowed_players.into_iter().collect();
                creature.loot_current_looter = corpse_loot.current_looter.or_else(|| {
                    request
                        .killer
                        .is_player()
                        .then(|| request.killer.counter())
                });
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
            self.creature_combat_leash.remove(&creature_guid.raw());
            self.creature_threats.remove(&creature_guid.raw());
        } else if self.active_creature_combats.contains_key(&creature_guid.raw()) {
            self.refresh_db_creature_combat_leash(creature_guid, request.now);
        }
        self.refresh_grid_state(grid_coord_for_position(creature.current_position));
        let update_body = if is_dead {
            build_db_creature_death_update_body(
                creature_guid,
                creature.dynamic_flags_for_player(
                    request
                        .killer
                        .is_player()
                        .then(|| request.killer.counter()),
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
            .filter(|_| request.melee_outcome.is_none())
            .map(|spell_id| {
                build_spell_non_melee_damage_log_body(SpellNonMeleeDamageLogPacket {
                    attacker: request.killer,
                    target: creature_guid,
                    spell_id,
                    damage: requested_damage,
                    school: request.spell_school,
                    absorb: 0,
                    resist: 0,
                    periodic: false,
                    blocked: 0,
                    hit_info: 0,
                })
            })
            .transpose()?;
        let observer_packets = nearby_observers
            .iter()
            .copied()
            .flat_map(|(player_guid, session_id)| {
                let mut packets = Vec::with_capacity(3);
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
                body: build_attack_stop_body(request.killer, creature_guid, true)?,
            };
            let observer_packets = nearby_observers
                .into_iter()
                .flat_map(|(_, session_id)| {
                    motion_stop_packet
                        .iter()
                        .cloned()
                        .chain([
                            combat_flag_packet.clone(),
                            attack_stop_packet.clone(),
                        ])
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
            update_body,
            death_finalization,
            target_switch,
            observer_packets,
        }))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PeriodicDamageTick {
    school: u32,
    requested_damage: u32,
    dealt_damage: u32,
    absorb: u32,
    resist: i32,
    threat: f32,
}

#[derive(Debug, Clone, Copy)]
struct PeriodicAuraLog {
    creature_guid: ObjectGuid,
    caster: ObjectGuid,
    spell_id: u32,
    aura_name: u32,
    tick: PeriodicDamageTick,
}

fn calculate_periodic_damage_tick(
    periodic: &PeriodicDamageAura,
    target_health: u32,
) -> PeriodicDamageTick {
    let requested_damage = periodic.amount.max(1);
    let absorb = 0;
    let resist = 0;
    let effective_damage = requested_damage.saturating_sub(absorb);
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

fn build_periodic_aura_log_body(log: PeriodicAuraLog) -> anyhow::Result<Vec<u8>> {
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

fn build_db_creature_aura_state_update_body(
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
