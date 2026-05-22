use super::*;
use wow_proto::world::WorldOpcode;

// Shared DB-creature damage authority and observer packet production.

type TrackedSingleTargetRemovalPackets = (
    Vec<OutboundWorldPacket>,
    Vec<(SessionId, OutboundWorldPacket)>,
);

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
            None,
            None,
            now,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::world) fn apply_db_creature_aura_replacing_spell_ids(
        &mut self,
        creature_guid: ObjectGuid,
        caster_character_guid: u32,
        aura: ActiveAura,
        replace_spell_ids: &[u32],
        single_target_descriptor: Option<SingleTargetAuraDescriptor>,
        diminishing_group: Option<DiminishingGroupRuntime>,
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
            single_target_descriptor,
            diminishing_group,
            now,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::world) fn apply_db_creature_aura_replacing_conflicts(
        &mut self,
        creature_guid: ObjectGuid,
        caster_character_guid: u32,
        aura: ActiveAura,
        resolution: &AuraRankConflictResolution,
        single_target_descriptor: Option<SingleTargetAuraDescriptor>,
        diminishing_group: Option<DiminishingGroupRuntime>,
        now: Instant,
    ) -> anyhow::Result<Option<DbCreatureAuraUpdateEvent>> {
        let (mut tracked_direct_packets, mut tracked_observer_packets) =
            if let Some(descriptor) = single_target_descriptor {
                self.remove_tracked_single_target_creature_auras(
                    creature_guid,
                    aura.caster,
                    descriptor,
                    caster_character_guid,
                    now,
                )?
            } else {
                (Vec::new(), Vec::new())
            };
        let aura_spell_id = aura.spell_id;
        let aura_caster = aura.caster;
        let (
            old_attack_duration,
            new_attack_duration,
            active_auras,
            position,
            direct_packets,
            update_body,
            confused_changed,
        ) = {
            let in_combat = self
                .active_creature_combats
                .contains_key(&creature_guid.raw());
            let Some(creature) = self.creatures.get_mut(&creature_guid.raw()) else {
                return Ok(None);
            };
            if !creature.is_alive() {
                return Ok(None);
            }
            let old_attack_duration = creature.base_attack_duration();
            let old_speeds = creature.move_speeds;
            let was_movement_blocked = active_aura_blocks_movement(&creature.active_auras);
            let was_stunned = active_aura_has_stun(&creature.active_auras);
            let was_confused = active_aura_has_confuse(&creature.active_auras);
            let old_display_id = db_creature_effective_display_id(creature);
            apply_active_aura_replacing_conflicts(&mut creature.active_auras, aura, resolution);
            let is_movement_blocked = active_aura_blocks_movement(&creature.active_auras);
            let is_stunned = active_aura_has_stun(&creature.active_auras);
            let is_confused = active_aura_has_confuse(&creature.active_auras);
            refresh_db_creature_aura_display_override(creature);
            sync_db_creature_confused_motion(creature, was_confused, is_confused, now);
            let previous_speeds = creature.refresh_move_speeds();
            debug_assert_eq!(old_speeds, previous_speeds);
            let stop_packet = if ((!was_movement_blocked && is_movement_blocked)
                || (!was_confused && is_confused))
                && !matches!(creature.motion, CreatureMotionState::Idle)
            {
                let stop = stop_db_creature_motion_runtime(creature);
                Some(OutboundWorldPacket {
                    opcode: WorldOpcode::SmsgMonsterMove as u16,
                    body: build_monster_move_stop_body(
                        creature_guid,
                        stop.position,
                        stop.spline_id,
                    )?,
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
            if is_confused {
                if let Some(confused_motion) = start_db_creature_confused_motion_runtime(
                    &DbCreatureNavigationGuardrail::default(),
                    Some(&self.geometry),
                    creature,
                    now,
                ) {
                    direct_packets.push(OutboundWorldPacket {
                        opcode: WorldOpcode::SmsgMonsterMove as u16,
                        body: build_monster_move_path_body_inner(
                            creature_guid,
                            confused_motion.start,
                            &confused_motion.path,
                            confused_motion.spline_id,
                            confused_motion.duration.as_millis().max(1) as u32,
                            None,
                            confused_motion.run,
                        )?,
                    });
                }
            }
            let new_display_id = db_creature_effective_display_id(creature);
            if old_display_id != new_display_id {
                direct_packets.push(OutboundWorldPacket {
                    opcode: WorldOpcode::SmsgUpdateObject as u16,
                    body: build_db_creature_display_update_body(creature_guid, new_display_id)?,
                });
            }
            if was_stunned != is_stunned || was_confused != is_confused {
                direct_packets.push(OutboundWorldPacket {
                    opcode: WorldOpcode::SmsgUpdateObject as u16,
                    body: build_unit_flags_update_body(
                        creature_guid,
                        db_creature_unit_flags(creature, in_combat),
                    )?,
                });
            }
            let position = creature.current_position;
            let update_body = build_db_creature_aura_update_body(creature_guid, &active_auras)?;
            (
                old_attack_duration,
                new_attack_duration,
                active_auras,
                position,
                direct_packets,
                update_body,
                was_confused != is_confused,
            )
        };
        if confused_changed {
            self.invalidate_idle_motion_start_schedule();
            self.sync_db_creature_idle_motion_tracking(creature_guid.raw());
        }
        self.reconcile_target_aura_trackers(creature_guid, &active_auras, now);
        if let Some(group) = diminishing_group {
            self.register_diminishing_aura(creature_guid, aura_caster, aura_spell_id, group, now);
        }
        if let Some(descriptor) = single_target_descriptor {
            let entries = self
                .tracked_single_target_auras
                .entry(aura_caster.raw())
                .or_default();
            entries.retain(|entry| {
                entry.target == creature_guid
                    || !single_target_aura_descriptors_match(entry.descriptor, descriptor)
            });
            entries.push(TrackedSingleTargetAuraRuntime {
                target: creature_guid,
                descriptor,
            });
        }
        self.adjust_db_creature_attack_timer_for_base_time_change(
            creature_guid,
            old_attack_duration,
            new_attack_duration,
            now,
        );
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
                        opcode: WorldOpcode::SmsgUpdateObject as u16,
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
        tracked_direct_packets.extend(direct_packets.clone());
        tracked_observer_packets.extend(observer_packets.clone());
        Ok(Some(DbCreatureAuraUpdateEvent {
            update_body,
            direct_packets: tracked_direct_packets,
            observer_packets: tracked_observer_packets,
        }))
    }

    pub(in crate::world) fn remove_db_creature_auras_by_dispel_type(
        &mut self,
        creature_guid: ObjectGuid,
        caster_character_guid: u32,
        dispel_type: u32,
        count: u32,
        now: Instant,
    ) -> anyhow::Result<Option<DbCreatureAuraDispelEvent>> {
        let in_combat = self
            .active_creature_combats
            .contains_key(&creature_guid.raw());
        let Some(creature) = self.creatures.get_mut(&creature_guid.raw()) else {
            return Ok(None);
        };
        if !creature.is_alive() {
            return Ok(None);
        }
        let old_attack_duration = creature.base_attack_duration();
        let old_speeds = creature.move_speeds;
        let was_movement_blocked = active_aura_blocks_movement(&creature.active_auras);
        let was_stunned = active_aura_has_stun(&creature.active_auras);
        let was_confused = active_aura_has_confuse(&creature.active_auras);
        let old_display_id = db_creature_effective_display_id(creature);
        let remove_count = count.max(1) as usize;
        let mut remaining = remove_count;
        let mut removed_spell_ids = Vec::new();
        creature.active_auras.retain(|aura| {
            if remaining == 0 || !active_aura_matches_dispel_type(aura, dispel_type) {
                return true;
            }
            removed_spell_ids.push(aura.spell_id);
            remaining -= 1;
            false
        });
        if removed_spell_ids.is_empty() {
            return Ok(None);
        }
        let is_movement_blocked = active_aura_blocks_movement(&creature.active_auras);
        let is_stunned = active_aura_has_stun(&creature.active_auras);
        let is_confused = active_aura_has_confuse(&creature.active_auras);
        refresh_db_creature_aura_display_override(creature);
        sync_db_creature_confused_motion(creature, was_confused, is_confused, now);
        let previous_speeds = creature.refresh_move_speeds();
        debug_assert_eq!(old_speeds, previous_speeds);
        let stop_packet = if was_movement_blocked
            && !is_movement_blocked
            && !was_confused
            && matches!(creature.motion, CreatureMotionState::Idle)
        {
            None
        } else if ((was_confused && !is_confused) || (!was_movement_blocked && is_movement_blocked))
            && !matches!(creature.motion, CreatureMotionState::Idle)
        {
            let stop = stop_db_creature_motion_runtime(creature);
            Some(OutboundWorldPacket {
                opcode: WorldOpcode::SmsgMonsterMove as u16,
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
        let new_display_id = db_creature_effective_display_id(creature);
        if old_display_id != new_display_id {
            direct_packets.push(OutboundWorldPacket {
                opcode: WorldOpcode::SmsgUpdateObject as u16,
                body: build_db_creature_display_update_body(creature_guid, new_display_id)?,
            });
        }
        if was_stunned != is_stunned || was_confused != is_confused {
            direct_packets.push(OutboundWorldPacket {
                opcode: WorldOpcode::SmsgUpdateObject as u16,
                body: build_unit_flags_update_body(
                    creature_guid,
                    db_creature_unit_flags(creature, in_combat),
                )?,
            });
        }
        let position = creature.current_position;
        self.adjust_db_creature_attack_timer_for_base_time_change(
            creature_guid,
            old_attack_duration,
            new_attack_duration,
            now,
        );
        let update_body = build_db_creature_aura_update_body(creature_guid, &active_auras)?;
        if was_confused != is_confused {
            self.invalidate_idle_motion_start_schedule();
            self.sync_db_creature_idle_motion_tracking(creature_guid.raw());
        }
        let mut aura_update = DbCreatureAuraUpdateEvent {
            update_body: update_body.clone(),
            direct_packets: direct_packets.clone(),
            observer_packets: self
                .nearby_player_guids(
                    position,
                    CREATURE_SPAWN_RADIUS_YARDS,
                    Some(caster_character_guid),
                )
                .into_iter()
                .filter_map(|player_guid| {
                    self.players.get(&player_guid).and_then(|player| {
                        player.packet_to_client(OutboundWorldPacket {
                            opcode: WorldOpcode::SmsgUpdateObject as u16,
                            body: update_body.clone(),
                        })
                    })
                })
                .collect(),
        };
        aura_update.observer_packets.extend(
            self.cancel_player_channels_for_removed_target_auras(
                creature_guid,
                &removed_spell_ids,
            )?,
        );
        Ok(Some(DbCreatureAuraDispelEvent {
            removed_spell_ids,
            aura_update,
        }))
    }

    fn remove_tracked_single_target_creature_auras(
        &mut self,
        current_target: ObjectGuid,
        caster: ObjectGuid,
        descriptor: SingleTargetAuraDescriptor,
        caster_character_guid: u32,
        now: Instant,
    ) -> anyhow::Result<TrackedSingleTargetRemovalPackets> {
        let tracked = self
            .tracked_single_target_auras
            .get(&caster.raw())
            .cloned()
            .unwrap_or_default();
        let mut direct_packets = Vec::new();
        let mut observer_packets = Vec::new();
        for entry in tracked {
            if entry.target == current_target
                || !single_target_aura_descriptors_match(entry.descriptor, descriptor)
            {
                continue;
            }
            let Some(event) = self.remove_db_creature_aura_by_spell_and_caster(
                entry.target,
                caster,
                entry.descriptor.spell_id,
                caster_character_guid,
                now,
            )?
            else {
                continue;
            };
            direct_packets.push(OutboundWorldPacket {
                opcode: WorldOpcode::SmsgUpdateObject as u16,
                body: event.update_body.clone(),
            });
            direct_packets.extend(event.direct_packets);
            observer_packets.extend(event.observer_packets);
        }
        Ok((direct_packets, observer_packets))
    }

    fn remove_db_creature_aura_by_spell_and_caster(
        &mut self,
        creature_guid: ObjectGuid,
        caster: ObjectGuid,
        spell_id: u32,
        caster_character_guid: u32,
        now: Instant,
    ) -> anyhow::Result<Option<DbCreatureAuraUpdateEvent>> {
        let in_combat = self
            .active_creature_combats
            .contains_key(&creature_guid.raw());
        let (
            old_attack_duration,
            new_attack_duration,
            active_auras,
            position,
            direct_packets,
            update_body,
            confused_changed,
        ) = {
            let Some(creature) = self.creatures.get_mut(&creature_guid.raw()) else {
                return Ok(None);
            };
            if !creature.is_alive() {
                return Ok(None);
            }
            let old_attack_duration = creature.base_attack_duration();
            let old_speeds = creature.move_speeds;
            let was_movement_blocked = active_aura_blocks_movement(&creature.active_auras);
            let was_stunned = active_aura_has_stun(&creature.active_auras);
            let was_confused = active_aura_has_confuse(&creature.active_auras);
            let old_display_id = db_creature_effective_display_id(creature);
            let before = creature.active_auras.len();
            creature
                .active_auras
                .retain(|aura| !(aura.spell_id == spell_id && aura.caster == caster));
            if creature.active_auras.len() == before {
                return Ok(None);
            }
            let is_movement_blocked = active_aura_blocks_movement(&creature.active_auras);
            let is_stunned = active_aura_has_stun(&creature.active_auras);
            let is_confused = active_aura_has_confuse(&creature.active_auras);
            refresh_db_creature_aura_display_override(creature);
            sync_db_creature_confused_motion(creature, was_confused, is_confused, now);
            let previous_speeds = creature.refresh_move_speeds();
            debug_assert_eq!(old_speeds, previous_speeds);
            let stop_packet = if ((was_confused && !is_confused)
                || (!was_movement_blocked && is_movement_blocked))
                && !matches!(creature.motion, CreatureMotionState::Idle)
            {
                let stop = stop_db_creature_motion_runtime(creature);
                Some(OutboundWorldPacket {
                    opcode: WorldOpcode::SmsgMonsterMove as u16,
                    body: build_monster_move_stop_body(
                        creature_guid,
                        stop.position,
                        stop.spline_id,
                    )?,
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
            let new_display_id = db_creature_effective_display_id(creature);
            if old_display_id != new_display_id {
                direct_packets.push(OutboundWorldPacket {
                    opcode: WorldOpcode::SmsgUpdateObject as u16,
                    body: build_db_creature_display_update_body(creature_guid, new_display_id)?,
                });
            }
            if was_stunned != is_stunned || was_confused != is_confused {
                direct_packets.push(OutboundWorldPacket {
                    opcode: WorldOpcode::SmsgUpdateObject as u16,
                    body: build_unit_flags_update_body(
                        creature_guid,
                        db_creature_unit_flags(creature, in_combat),
                    )?,
                });
            }
            let position = creature.current_position;
            let update_body = build_db_creature_aura_update_body(creature_guid, &active_auras)?;
            (
                old_attack_duration,
                new_attack_duration,
                active_auras,
                position,
                direct_packets,
                update_body,
                was_confused != is_confused,
            )
        };
        if confused_changed {
            self.invalidate_idle_motion_start_schedule();
            self.sync_db_creature_idle_motion_tracking(creature_guid.raw());
        }
        self.reconcile_target_aura_trackers(creature_guid, &active_auras, now);
        self.adjust_db_creature_attack_timer_for_base_time_change(
            creature_guid,
            old_attack_duration,
            new_attack_duration,
            now,
        );
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
                        opcode: WorldOpcode::SmsgUpdateObject as u16,
                        body: update_body.clone(),
                    })
                })
            })
            .collect();
        for packet in &direct_packets {
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
        let old_millis = old_duration.as_millis() as i128;
        let new_millis = new_duration.as_millis() as i128;
        let diff = new_millis - old_millis;
        let Some(due_at) = ({
            let Some(combat) = self.active_creature_combats.get_mut(&creature_guid.raw()) else {
                return;
            };
            if diff >= 0 {
                combat.next_swing_at += Duration::from_millis(diff as u64);
            } else {
                let remaining = combat.next_swing_at.saturating_duration_since(now);
                let reduction = Duration::from_millis(diff.unsigned_abs() as u64);
                combat.next_swing_at = if reduction >= remaining {
                    now
                } else {
                    combat.next_swing_at - reduction
                };
            }
            Some(combat.next_swing_at)
        }) else {
            return;
        };
        self.schedule_db_creature_combat_due_at(creature_guid, due_at);
    }

    pub(in crate::world) fn remove_db_creature_damage_interrupt_auras(
        &mut self,
        creature_guid: ObjectGuid,
        now: Instant,
    ) -> anyhow::Result<Vec<OutboundWorldPacket>> {
        let in_combat = self
            .active_creature_combats
            .contains_key(&creature_guid.raw());
        let (packets, active_auras) = {
            let Some(creature) = self.creatures.get_mut(&creature_guid.raw()) else {
                return Ok(Vec::new());
            };
            let packets = remove_db_creature_damage_interrupt_auras_from_runtime(
                creature_guid,
                creature,
                in_combat,
                now,
            )?;
            (packets, creature.active_auras.clone())
        };
        if !packets.is_empty() {
            self.invalidate_idle_motion_start_schedule();
            self.sync_db_creature_idle_motion_tracking(creature_guid.raw());
            self.reconcile_target_aura_trackers(creature_guid, &active_auras, now);
        }
        Ok(packets)
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
            let in_combat = self.active_creature_combats.contains_key(&raw_guid);
            let (
                tracking_active_auras,
                tick_packets,
                runtime_packets,
                death_motion_stop_packet,
                observer_update,
                died_from_aura,
                invalidated_motion_schedule,
            ) = {
                let Some(creature) = self.creatures.get_mut(&raw_guid) else {
                    continue;
                };
                if !creature.is_alive() {
                    continue;
                }

                let old_speeds = creature.move_speeds;
                let old_attack_duration = creature.base_attack_duration();
                let old_display_id = db_creature_effective_display_id(creature);
                let mut aura_changed = false;
                let target_snapshot = db_creature_spell_snapshot(creature);

                let mut tick_packets = Vec::new();
                let mut damage_interrupt_packets = Vec::new();
                let mut death_motion_stop_packet = None;
                let mut pending_damage_ticks = Vec::new();
                let mut pending_regen_ticks = Vec::new();
                let mut died_from_pending_tick = false;
                let mut tracking_active_auras = None;
                let mut invalidated_motion_schedule = false;
                for aura in &mut creature.active_auras {
                    if let Some(regen) = aura.periodic_regen.as_mut() {
                        while regen.next_tick_at <= now {
                            pending_regen_ticks.push((regen.health_amount, regen.mana_amount));
                            regen.next_tick_at += Duration::from_millis(regen.tick_millis as u64);
                        }
                    }
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
                    let caster_snapshot =
                        periodic_spell_caster_snapshot(&self.players, aura.caster)
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
                    pending_damage_ticks.push((
                        aura.caster,
                        aura.spell_id,
                        periodic.aura_name,
                        tick,
                    ));
                }
                for (health_amount, mana_amount) in pending_regen_ticks {
                    if health_amount > 0 && creature.health < creature.max_health() {
                        creature.health = creature
                            .health
                            .saturating_add(health_amount)
                            .min(creature.max_health());
                        aura_changed = true;
                    }
                    let max_mana = creature_mana(&creature.spawn.template);
                    if mana_amount > 0 && max_mana > 0 && creature.power1 < max_mana {
                        creature.power1 = creature.power1.saturating_add(mana_amount).min(max_mana);
                        aura_changed = true;
                    }
                }
                if !pending_damage_ticks.is_empty() {
                    damage_interrupt_packets =
                        remove_db_creature_damage_interrupt_auras_from_runtime(
                            creature_guid,
                            creature,
                            in_combat,
                            now,
                        )?;
                    if !damage_interrupt_packets.is_empty() {
                        invalidated_motion_schedule = true;
                        tracking_active_auras = Some(creature.active_auras.clone());
                        aura_changed = true;
                    }
                }
                for (caster, spell_id, aura_name, tick) in pending_damage_ticks {
                    if tick.dealt_damage >= creature.health
                        && !matches!(creature.motion, CreatureMotionState::Idle)
                    {
                        let stop = stop_db_creature_motion_runtime(creature);
                        death_motion_stop_packet = Some(OutboundWorldPacket {
                            opcode: WorldOpcode::SmsgMonsterMove as u16,
                            body: build_monster_move_stop_body(
                                creature_guid,
                                stop.position,
                                stop.spline_id,
                            )?,
                        });
                    }
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
                        break;
                    }
                }
                let was_movement_blocked = active_aura_blocks_movement(&creature.active_auras);
                let was_stunned = active_aura_has_stun(&creature.active_auras);
                let was_confused = active_aura_has_confuse(&creature.active_auras);
                let before_expiration = creature.active_auras.len();
                creature
                    .active_auras
                    .retain(|aura| aura.expires_at.is_none_or(|expires_at| now < expires_at));
                let expired_auras = creature.active_auras.len() != before_expiration;
                aura_changed |= expired_auras;
                let mut expiration_control_packets = Vec::new();
                if expired_auras {
                    let is_movement_blocked = active_aura_blocks_movement(&creature.active_auras);
                    let is_stunned = active_aura_has_stun(&creature.active_auras);
                    let is_confused = active_aura_has_confuse(&creature.active_auras);
                    sync_db_creature_confused_motion(creature, was_confused, is_confused, now);
                    if ((was_confused && !is_confused)
                        || (!was_movement_blocked && is_movement_blocked))
                        && !matches!(creature.motion, CreatureMotionState::Idle)
                    {
                        let stop = stop_db_creature_motion_runtime(creature);
                        expiration_control_packets.push(OutboundWorldPacket {
                            opcode: WorldOpcode::SmsgMonsterMove as u16,
                            body: build_monster_move_stop_body(
                                creature_guid,
                                stop.position,
                                stop.spline_id,
                            )?,
                        });
                    }
                    if was_stunned != is_stunned || was_confused != is_confused {
                        expiration_control_packets.push(OutboundWorldPacket {
                            opcode: WorldOpcode::SmsgUpdateObject as u16,
                            body: build_unit_flags_update_body(
                                creature_guid,
                                db_creature_unit_flags(creature, in_combat),
                            )?,
                        });
                    }
                    if was_confused != is_confused || was_movement_blocked != is_movement_blocked {
                        invalidated_motion_schedule = true;
                    }
                    tracking_active_auras = Some(creature.active_auras.clone());
                }
                if aura_changed {
                    refresh_db_creature_aura_display_override(creature);
                }
                let died_from_aura = died_from_pending_tick || creature.health == 0;
                let mut runtime_packets = if aura_changed && !died_from_aura {
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
                runtime_packets.extend(damage_interrupt_packets.iter().cloned());
                if aura_changed && !died_from_aura {
                    let new_display_id = db_creature_effective_display_id(creature);
                    if old_display_id != new_display_id {
                        runtime_packets.push(OutboundWorldPacket {
                            opcode: WorldOpcode::SmsgUpdateObject as u16,
                            body: build_db_creature_display_update_body(
                                creature_guid,
                                new_display_id,
                            )?,
                        });
                    }
                }
                runtime_packets.extend(expiration_control_packets);

                let observer_update = if aura_changed || !tick_packets.is_empty() {
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
                            db_creature_unit_flags(creature, in_combat),
                        )?
                    };
                    Some((update_body, creature.current_position))
                } else {
                    None
                };
                (
                    tracking_active_auras,
                    tick_packets,
                    runtime_packets,
                    death_motion_stop_packet,
                    observer_update,
                    died_from_aura,
                    invalidated_motion_schedule,
                )
            };
            if invalidated_motion_schedule {
                self.invalidate_idle_motion_start_schedule();
                self.sync_db_creature_idle_motion_tracking(creature_guid.raw());
            }
            if let Some((update_body, position)) = observer_update {
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
                                opcode: WorldOpcode::SmsgPeriodicAuraLog as u16,
                                body: log_body.clone(),
                            },
                        ));
                    }
                    packets.push((
                        session_id,
                        OutboundWorldPacket {
                            opcode: WorldOpcode::SmsgUpdateObject as u16,
                            body: update_body.clone(),
                        },
                    ));
                    for packet in &runtime_packets {
                        packets.push((session_id, packet.clone()));
                    }
                    if let Some(packet) = &death_motion_stop_packet {
                        packets.push((session_id, packet.clone()));
                    }
                }
            }
            if died_from_aura {
                packets.extend(self.clear_player_melee_state_for_dead_target(creature_guid, None)?);
                packets.extend(self.interrupt_player_spell_work_targeting_unit(creature_guid)?);
                packets
                    .extend(self.clear_db_creature_combat_with_player_flag_packets(creature_guid)?);
            }
            if let Some(active_auras) = tracking_active_auras {
                self.reconcile_target_aura_trackers(creature_guid, &active_auras, now);
            }
        }
        for (creature_guid, victim, threat) in threat_updates {
            self.add_db_creature_threat(creature_guid, victim, threat);
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
        let old_wounded_speed_multiplier = creature.wounded_combat_speed_multiplier();
        let damage_interrupt_packets = if requested_damage > 0 {
            self.remove_db_creature_damage_interrupt_auras(creature_guid, request.now)?
        } else {
            Vec::new()
        };
        let motion_stop_packet = {
            let Some(creature) = self.creatures.get_mut(&creature_guid.raw()) else {
                return Ok(None);
            };
            let will_die = requested_damage >= creature.health;
            if will_die && !matches!(creature.motion, CreatureMotionState::Idle) {
                let stop = stop_db_creature_motion_runtime(creature);
                Some(OutboundWorldPacket {
                    opcode: WorldOpcode::SmsgMonsterMove as u16,
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
        let wounded_motion_packet = if is_dead {
            None
        } else {
            let Some(creature) = self.creatures.get_mut(&creature_guid.raw()) else {
                return Ok(None);
            };
            let new_wounded_speed_multiplier = creature.wounded_combat_speed_multiplier();
            if (old_wounded_speed_multiplier - new_wounded_speed_multiplier).abs() > f32::EPSILON {
                retime_db_creature_motion_for_speed_change(creature, request.now)?
            } else {
                None
            }
        };
        let Some(creature) = self.creatures.get(&creature_guid.raw()).cloned() else {
            return Ok(None);
        };
        self.sync_db_creature_lifecycle_tracking(creature_guid.raw());
        self.add_db_creature_threat(creature_guid, request.killer, damage as f32);
        if !is_dead
            && self
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
        let player_spell_target_cleanup_packets = if is_dead {
            self.interrupt_player_spell_work_targeting_unit(creature_guid)?
        } else {
            Vec::new()
        };
        let player_combat_flag_packets = if is_dead {
            self.clear_db_creature_combat_with_player_flag_packets(creature_guid)?
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
                            opcode: WorldOpcode::SmsgSpellLogMiss as u16,
                            body: spell_miss_log_body.clone(),
                        },
                    ));
                }
                if let Some(spell_non_melee_log_body) = &spell_non_melee_log_body {
                    packets.push((
                        session_id,
                        OutboundWorldPacket {
                            opcode: WorldOpcode::SmsgSpellNonMeleeDamageLog as u16,
                            body: spell_non_melee_log_body.clone(),
                        },
                    ));
                }
                if let Some(attacker_state_body) = &attacker_state_body {
                    packets.push((
                        session_id,
                        OutboundWorldPacket {
                            opcode: WorldOpcode::SmsgAttackerStateUpdate as u16,
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
                        opcode: WorldOpcode::SmsgUpdateObject as u16,
                        body: update_body,
                    },
                ));
                packets
            })
            .collect();
        if let Some(packet) = &wounded_motion_packet {
            observer_packets.extend(
                nearby_observers
                    .iter()
                    .copied()
                    .map(|(_, session_id)| (session_id, packet.clone())),
            );
        }
        if !damage_interrupt_packets.is_empty() {
            observer_packets.extend(nearby_observers.iter().copied().flat_map(
                |(_, session_id)| {
                    damage_interrupt_packets
                        .iter()
                        .cloned()
                        .map(move |packet| (session_id, packet))
                },
            ));
        }
        observer_packets.extend(player_melee_cleanup_packets);
        observer_packets.extend(player_spell_target_cleanup_packets);
        observer_packets.extend(player_combat_flag_packets);
        let death_finalization = if is_dead {
            let combat_flag_packet = OutboundWorldPacket {
                opcode: WorldOpcode::SmsgUpdateObject as u16,
                body: build_unit_flags_update_body(
                    creature_guid,
                    db_creature_unit_flags(&creature, false),
                )?,
            };
            let attack_stop_packet = OutboundWorldPacket {
                opcode: WorldOpcode::SmsgAttackStop as u16,
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
        let mut direct_packets = damage_interrupt_packets;
        if let Some(packet) = wounded_motion_packet {
            direct_packets.push(packet);
        }
        Ok(Some(DbCreatureDamageEvent {
            damage,
            attacker_rage_damage,
            creature,
            attacker_state_body,
            spell_non_melee_log_body,
            spell_miss_log_body,
            update_body,
            direct_packets,
            death_finalization,
            target_switch,
            observer_packets,
        }))
    }
}

fn remove_db_creature_damage_interrupt_auras_from_runtime(
    creature_guid: ObjectGuid,
    creature: &mut DbCreatureRuntime,
    in_combat: bool,
    now: Instant,
) -> anyhow::Result<Vec<OutboundWorldPacket>> {
    if !creature.is_alive()
        || !creature
            .active_auras
            .iter()
            .any(active_aura_breaks_on_damage)
    {
        return Ok(Vec::new());
    }

    let old_speeds = creature.move_speeds;
    let old_display_id = db_creature_effective_display_id(creature);
    let was_movement_blocked = active_aura_blocks_movement(&creature.active_auras);
    let was_stunned = active_aura_has_stun(&creature.active_auras);
    let was_confused = active_aura_has_confuse(&creature.active_auras);
    creature
        .active_auras
        .retain(|aura| !active_aura_breaks_on_damage(aura));
    refresh_db_creature_aura_display_override(creature);
    let is_movement_blocked = active_aura_blocks_movement(&creature.active_auras);
    let is_stunned = active_aura_has_stun(&creature.active_auras);
    let is_confused = active_aura_has_confuse(&creature.active_auras);
    sync_db_creature_confused_motion(creature, was_confused, is_confused, now);
    let previous_speeds = creature.refresh_move_speeds();
    debug_assert_eq!(old_speeds, previous_speeds);

    let mut packets = Vec::new();
    if ((was_confused && !is_confused) || (!was_movement_blocked && is_movement_blocked))
        && !matches!(creature.motion, CreatureMotionState::Idle)
    {
        let stop = stop_db_creature_motion_runtime(creature);
        packets.push(OutboundWorldPacket {
            opcode: WorldOpcode::SmsgMonsterMove as u16,
            body: build_monster_move_stop_body(creature_guid, stop.position, stop.spline_id)?,
        });
    }
    packets.extend(db_creature_aura_runtime_packets(
        creature_guid,
        creature,
        old_speeds,
        now,
    )?);
    let new_display_id = db_creature_effective_display_id(creature);
    if old_display_id != new_display_id {
        packets.push(OutboundWorldPacket {
            opcode: WorldOpcode::SmsgUpdateObject as u16,
            body: build_db_creature_display_update_body(creature_guid, new_display_id)?,
        });
    }
    if was_stunned != is_stunned || was_confused != is_confused {
        packets.push(OutboundWorldPacket {
            opcode: WorldOpcode::SmsgUpdateObject as u16,
            body: build_unit_flags_update_body(
                creature_guid,
                db_creature_unit_flags(creature, in_combat),
            )?,
        });
    }
    packets.push(OutboundWorldPacket {
        opcode: WorldOpcode::SmsgUpdateObject as u16,
        body: build_db_creature_aura_update_body(creature_guid, &creature.active_auras)?,
    });
    Ok(packets)
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
        WorldOpcode::SmsgSplineSetWalkSpeed as u16,
        old_speeds.walk,
        new_speeds.walk,
    )?;
    push_speed_change_packet(
        &mut packets,
        creature_guid,
        WorldOpcode::SmsgSplineSetRunSpeed as u16,
        old_speeds.run,
        new_speeds.run,
    )?;
    push_speed_change_packet(
        &mut packets,
        creature_guid,
        WorldOpcode::SmsgSplineSetRunBackSpeed as u16,
        old_speeds.run_back,
        new_speeds.run_back,
    )?;
    push_speed_change_packet(
        &mut packets,
        creature_guid,
        WorldOpcode::SmsgSplineSetSwimSpeed as u16,
        old_speeds.swim,
        new_speeds.swim,
    )?;
    push_speed_change_packet(
        &mut packets,
        creature_guid,
        WorldOpcode::SmsgSplineSetSwimBackSpeed as u16,
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

pub(in crate::world) fn refresh_db_creature_aura_display_override(
    creature: &mut DbCreatureRuntime,
) {
    creature.aura_display_id_override = active_aura_transform_display_id(&creature.active_auras);
}

fn sync_db_creature_confused_motion(
    creature: &mut DbCreatureRuntime,
    was_confused: bool,
    is_confused: bool,
    now: Instant,
) {
    if !was_confused && is_confused {
        creature.begin_confused_motion(now);
    } else if was_confused && !is_confused {
        creature.clear_confused_motion();
        creature.resume_default_motion_now(now);
    }
}

pub(in crate::world) fn db_creature_effective_display_id(creature: &DbCreatureRuntime) -> u32 {
    creature
        .aura_display_id_override
        .or(creature.display_id_override)
        .unwrap_or(creature.native_display.display_id)
}

pub(in crate::world) fn db_creature_motion_speed_changed(
    motion: &CreatureMotionState,
    old_speeds: UnitMoveSpeeds,
    new_speeds: UnitMoveSpeeds,
) -> bool {
    match motion {
        CreatureMotionState::Random(_)
        | CreatureMotionState::Confused(_)
        | CreatureMotionState::Waypoint(_) => {
            (old_speeds.walk - new_speeds.walk).abs() > f32::EPSILON
        }
        CreatureMotionState::Chase(chase) => {
            if chase.run {
                (old_speeds.run - new_speeds.run).abs() > f32::EPSILON
            } else {
                (old_speeds.walk - new_speeds.walk).abs() > f32::EPSILON
            }
        }
        CreatureMotionState::Flee(_) | CreatureMotionState::ReturnHome(_) => {
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
    unit_flags: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, creature)?;

    let mut values = vec![None; PLAYER_END_FIELDS];
    set_unit_aura_update_values(&mut values, active_auras)?;
    set_update_value(
        &mut values,
        UNIT_FIELD_BYTES_1,
        creature_unit_bytes_1(active_auras),
    )?;
    set_update_value(&mut values, UNIT_FIELD_HEALTH, health)?;
    set_update_value(&mut values, UNIT_DYNAMIC_FLAGS, dynamic_flags)?;
    set_update_value(&mut values, UNIT_FIELD_FLAGS, unit_flags)?;

    write_update_values(&mut block, &values)?;
    Ok(build_update_object_body(&[block]))
}
