// Shared DB-creature damage authority and observer packet production.

impl MapRuntime {
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
                creature.dynamic_flags(),
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
                    .map(|player| player.session_id)
            })
            .collect::<Vec<_>>();
        let attacker_state_body = if let Some(outcome) = request.melee_outcome {
            let mut outcome = outcome;
            outcome.total_damage = damage;
            outcome.school_damage = outcome.school_damage.min(damage);
            build_attacker_state_update_body_for_outcome(
                request.killer,
                creature_guid,
                outcome,
                request.spell_id.unwrap_or(0),
            )?
        } else if let Some(spell_id) = request.spell_id {
            build_attacker_state_update_body_with_spell_id(
                request.killer,
                creature_guid,
                damage,
                spell_id,
            )?
        } else {
            build_attacker_state_update_body(request.killer, creature_guid, damage)?
        };
        let spell_non_melee_log_body = request
            .spell_id
            .map(|spell_id| {
                build_spell_non_melee_damage_log_body(SpellNonMeleeDamageLogPacket {
                    attacker: request.killer,
                    target: creature_guid,
                    spell_id,
                    damage,
                    school: 0,
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
            .flat_map(|session_id| {
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
                packets.push((
                    session_id,
                    OutboundWorldPacket {
                        opcode: SMSG_ATTACKERSTATEUPDATE,
                        body: attacker_state_body.clone(),
                    },
                ));
                packets.push((
                    session_id,
                    OutboundWorldPacket {
                        opcode: SMSG_UPDATE_OBJECT,
                        body: update_body.clone(),
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
                .flat_map(|session_id| {
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
