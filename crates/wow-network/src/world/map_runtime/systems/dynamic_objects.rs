use super::*;
use wow_proto::world::WorldOpcode;

#[derive(Debug)]
pub(in crate::world) struct DynamicObjectCreateEvent {
    pub(in crate::world) direct_packets: Vec<OutboundWorldPacket>,
    pub(in crate::world) observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

impl MapRuntime {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::world) fn create_persistent_area_dynamic_object(
        &mut self,
        caster: ObjectGuid,
        caster_character_guid: u32,
        spell_id: u32,
        _effect_index: usize,
        position: WorldPosition,
        radius: f32,
        duration_millis: u32,
        periodic_damage: Option<PeriodicDamageAura>,
        channeled: bool,
        channel_interrupt_flags: u32,
        now: Instant,
    ) -> anyhow::Result<Option<DynamicObjectCreateEvent>> {
        let Some(caster_player) = self.players.get(&caster_character_guid) else {
            return Ok(None);
        };
        if caster_player.client_session_id().is_none() {
            return Ok(None);
        }
        let counter = self.next_dynamic_object_counter;
        self.next_dynamic_object_counter = self.next_dynamic_object_counter.wrapping_add(1).max(1);
        let guid = ObjectGuid::new(HighGuid::DynamicObject, spell_id, counter);
        let dynamic_object = DynamicObjectRuntime {
            guid,
            caster,
            caster_character_guid,
            spell_id,
            position,
            radius,
            expires_at: now + Duration::from_millis(duration_millis as u64),
            periodic_damage,
            channeled,
            channel_interrupt_flags,
            damage_delay_count: 0,
        };
        let create_body =
            build_update_object_body(&[build_dynamic_object_create_block(&dynamic_object)?]);
        self.dynamic_objects
            .insert(guid.raw(), dynamic_object.clone());

        let mut direct_packets = vec![OutboundWorldPacket {
            opcode: WorldOpcode::SmsgUpdateObject as u16,
            body: create_body.clone(),
        }];
        if channeled {
            direct_packets.push(OutboundWorldPacket {
                opcode: WorldOpcode::MsgChannelStart as u16,
                body: build_channel_start_body(caster, spell_id, duration_millis)?,
            });
            direct_packets.push(OutboundWorldPacket {
                opcode: WorldOpcode::SmsgUpdateObject as u16,
                body: build_player_channel_update_body(caster, Some(guid), spell_id)?,
            });
        }

        let mut observer_packets = Vec::new();
        for player_guid in self.nearby_player_guids(position, CREATURE_SPAWN_RADIUS_YARDS, None) {
            let Some(player) = self.players.get_mut(&player_guid) else {
                continue;
            };
            if !player.visible_objects.insert(guid) {
                continue;
            }
            if player_guid == caster_character_guid {
                continue;
            }
            if let Some(session_id) = player.client_session_id() {
                observer_packets.push((
                    session_id,
                    OutboundWorldPacket {
                        opcode: WorldOpcode::SmsgUpdateObject as u16,
                        body: create_body.clone(),
                    },
                ));
                if channeled {
                    observer_packets.push((
                        session_id,
                        OutboundWorldPacket {
                            opcode: WorldOpcode::SmsgUpdateObject as u16,
                            body: build_player_channel_update_body(caster, Some(guid), spell_id)?,
                        },
                    ));
                }
            }
        }

        if let Some(player) = self.players.get_mut(&caster_character_guid) {
            player.visible_objects.insert(guid);
        }

        Ok(Some(DynamicObjectCreateEvent {
            direct_packets,
            observer_packets,
        }))
    }

    pub(in crate::world) fn advance_dynamic_objects(
        &mut self,
        faction_templates: &FactionTemplateStore,
        now: Instant,
        now_epoch_secs: u64,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let dynamic_guids = self.dynamic_objects.keys().copied().collect::<Vec<_>>();
        let mut packets = Vec::new();
        for raw_guid in dynamic_guids {
            let Some(dynamic_object) = self.dynamic_objects.get(&raw_guid).cloned() else {
                continue;
            };
            if let Some(mut periodic) = dynamic_object.periodic_damage {
                if now >= periodic.next_tick_at
                    && periodic.next_tick_at <= dynamic_object.expires_at
                {
                    while periodic.next_tick_at <= now
                        && periodic.next_tick_at <= dynamic_object.expires_at
                    {
                        periodic.next_tick_at += Duration::from_millis(periodic.tick_millis as u64);
                    }
                    packets.extend(self.apply_dynamic_object_periodic_damage(
                        faction_templates,
                        &dynamic_object,
                        &periodic,
                        now,
                        now_epoch_secs,
                    )?);
                    if let Some(stored) = self.dynamic_objects.get_mut(&raw_guid) {
                        stored.periodic_damage = Some(periodic);
                    }
                }
            }
            if now >= dynamic_object.expires_at {
                self.dynamic_objects.remove(&raw_guid);
                packets.extend(self.destroy_dynamic_object_packets(dynamic_object)?);
            }
        }
        Ok(packets)
    }

    pub(in crate::world) fn cancel_player_dynamic_object_channel(
        &mut self,
        caster_character_guid: u32,
    ) -> anyhow::Result<Option<PlayerChannelEvent>> {
        let Some(raw_guid) = self
            .dynamic_objects
            .iter()
            .find(|(_, dynamic_object)| {
                dynamic_object.caster_character_guid == caster_character_guid
                    && dynamic_object.channeled
            })
            .map(|(raw_guid, _)| *raw_guid)
        else {
            return Ok(None);
        };
        let Some(dynamic_object) = self.dynamic_objects.remove(&raw_guid) else {
            return Ok(None);
        };
        self.destroy_dynamic_object_channel_event(dynamic_object)
    }

    pub(in crate::world) fn remove_player_dynamic_objects(
        &mut self,
        caster_character_guid: u32,
    ) -> anyhow::Result<PlayerSpellRuntimeCleanupPackets> {
        let direct_session_id = self
            .players
            .get(&caster_character_guid)
            .and_then(PlayerRuntime::client_session_id);
        let raw_guids = self
            .dynamic_objects
            .iter()
            .filter_map(|(raw_guid, dynamic_object)| {
                (dynamic_object.caster_character_guid == caster_character_guid).then_some(*raw_guid)
            })
            .collect::<Vec<_>>();
        let mut cleanup = PlayerSpellRuntimeCleanupPackets::default();
        for raw_guid in raw_guids {
            let Some(dynamic_object) = self.dynamic_objects.remove(&raw_guid) else {
                continue;
            };
            for (session_id, packet) in self.destroy_dynamic_object_packets(dynamic_object)? {
                if Some(session_id) == direct_session_id {
                    cleanup.direct_packets.push(packet);
                } else {
                    cleanup.observer_packets.push((session_id, packet));
                }
            }
        }
        Ok(cleanup)
    }

    pub(in crate::world) fn cancel_player_dynamic_object_channel_for_movement(
        &mut self,
        caster_character_guid: u32,
    ) -> anyhow::Result<Option<PlayerChannelEvent>> {
        let Some(raw_guid) = self
            .dynamic_objects
            .iter()
            .find(|(_, dynamic_object)| {
                dynamic_object.caster_character_guid == caster_character_guid
                    && dynamic_object.channeled
                    && dynamic_object.channel_interrupt_flags & AURA_INTERRUPT_FLAG_MOVING != 0
            })
            .map(|(raw_guid, _)| *raw_guid)
        else {
            return Ok(None);
        };
        let Some(dynamic_object) = self.dynamic_objects.remove(&raw_guid) else {
            return Ok(None);
        };
        self.destroy_dynamic_object_channel_event(dynamic_object)
    }

    fn apply_dynamic_object_periodic_damage(
        &mut self,
        faction_templates: &FactionTemplateStore,
        dynamic_object: &DynamicObjectRuntime,
        periodic: &PeriodicDamageAura,
        now: Instant,
        now_epoch_secs: u64,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let targets = self.nearby_attackable_db_creature_guids_for_player_spell_at_position(
            faction_templates,
            dynamic_object.caster_character_guid,
            dynamic_object.position,
            dynamic_object.radius,
        );
        let caster_snapshot = periodic_spell_caster_snapshot(&self.players, dynamic_object.caster)
            .unwrap_or(periodic.caster_snapshot);
        let mut packets = Vec::new();
        for target in targets {
            let Some(target_snapshot) = self
                .creatures
                .get(&target.raw())
                .map(db_creature_spell_snapshot)
            else {
                continue;
            };
            let Some(target_health) = self
                .creatures
                .get(&target.raw())
                .map(|creature| creature.health)
            else {
                continue;
            };
            let Some(target_active_auras) = self
                .creatures
                .get(&target.raw())
                .map(|creature| creature.active_auras.clone())
            else {
                continue;
            };
            let tick = calculate_periodic_damage_tick_with_target_auras(
                periodic,
                caster_snapshot,
                target_snapshot,
                &target_active_auras,
                target_health,
            );
            if tick.dealt_damage == 0 {
                continue;
            }
            let motion_stop_packet = if tick.dealt_damage >= target_health {
                let Some(creature) = self.creatures.get_mut(&target.raw()) else {
                    continue;
                };
                if matches!(creature.motion, CreatureMotionState::Idle) {
                    None
                } else {
                    let stop = stop_db_creature_motion_runtime(creature);
                    Some(OutboundWorldPacket {
                        opcode: WorldOpcode::SmsgMonsterMove as u16,
                        body: build_monster_move_stop_body(target, stop.position, stop.spline_id)?,
                    })
                }
            } else {
                None
            };
            let damage_interrupt_packets =
                self.remove_db_creature_damage_interrupt_auras(target, now)?;
            let Some(applied) = self.apply_creature_world_damage(
                target,
                dynamic_object.caster,
                tick.dealt_damage,
                WorldDamageKind::PeriodicAura,
                now,
                now_epoch_secs,
            )?
            else {
                continue;
            };
            if applied.remaining_health > 0 {
                packets.extend(self.begin_dynamic_object_target_combat_packets(
                    dynamic_object,
                    target,
                    now,
                )?);
                self.add_db_creature_threat_with_school_mask(
                    target,
                    dynamic_object.caster,
                    tick.threat,
                    spell_school_mask_from_school(tick.school),
                );
            } else {
                packets.extend(self.clear_player_melee_state_for_dead_target(target, None)?);
                packets.extend(self.interrupt_player_spell_work_targeting_unit(target)?);
                packets.extend(self.clear_db_creature_combat_with_player_flag_packets(target)?);
            }

            let Some(creature) = self.creatures.get(&target.raw()) else {
                continue;
            };
            let update_body = if creature.health == 0 {
                build_db_creature_death_update_body(
                    target,
                    creature.dynamic_flags(),
                    db_creature_unit_flags(creature, false),
                )?
            } else {
                build_db_creature_state_update_body(
                    target,
                    creature.health,
                    creature.dynamic_flags(),
                )?
            };
            let log_body = build_periodic_aura_log_body(PeriodicAuraLog {
                creature_guid: target,
                caster: dynamic_object.caster,
                spell_id: dynamic_object.spell_id,
                aura_name: periodic.aura_name,
                tick,
            })?;
            for player_guid in self.nearby_player_guids(
                creature.current_position,
                CREATURE_SPAWN_RADIUS_YARDS,
                None,
            ) {
                let Some(session_id) = self
                    .players
                    .get(&player_guid)
                    .and_then(PlayerRuntime::client_session_id)
                else {
                    continue;
                };
                packets.push((
                    session_id,
                    OutboundWorldPacket {
                        opcode: WorldOpcode::SmsgPeriodicAuraLog as u16,
                        body: log_body.clone(),
                    },
                ));
                packets.push((
                    session_id,
                    OutboundWorldPacket {
                        opcode: WorldOpcode::SmsgUpdateObject as u16,
                        body: update_body.clone(),
                    },
                ));
                for packet in &damage_interrupt_packets {
                    packets.push((session_id, packet.clone()));
                }
                if let Some(packet) = &motion_stop_packet {
                    packets.push((session_id, packet.clone()));
                }
            }
        }
        Ok(packets)
    }

    pub(in crate::world) fn interrupt_dynamic_object_channel_for_damage(
        &mut self,
        caster_character_guid: u32,
        now: Instant,
    ) -> anyhow::Result<Option<PlayerChannelEvent>> {
        let Some(raw_guid) = self
            .dynamic_objects
            .iter()
            .find(|(_, dynamic_object)| {
                dynamic_object.caster_character_guid == caster_character_guid
                    && dynamic_object.channeled
                    && dynamic_object.channel_interrupt_flags
                        & (AURA_INTERRUPT_FLAG_DAMAGE | AURA_INTERRUPT_FLAG_DAMAGE_CHANNEL_DURATION)
                        != 0
            })
            .map(|(raw_guid, _)| *raw_guid)
        else {
            return Ok(None);
        };
        let flags = self
            .dynamic_objects
            .get(&raw_guid)
            .map(|dynamic_object| dynamic_object.channel_interrupt_flags)
            .unwrap_or(0);
        if flags & AURA_INTERRUPT_FLAG_DAMAGE_CHANNEL_DURATION != 0 {
            return self.delay_dynamic_object_channel_for_damage(raw_guid, now);
        }
        let Some(dynamic_object) = self.dynamic_objects.remove(&raw_guid) else {
            return Ok(None);
        };
        self.destroy_dynamic_object_channel_event(dynamic_object)
    }

    fn delay_dynamic_object_channel_for_damage(
        &mut self,
        raw_guid: u64,
        now: Instant,
    ) -> anyhow::Result<Option<PlayerChannelEvent>> {
        let Some(dynamic_object) = self.dynamic_objects.get_mut(&raw_guid) else {
            return Ok(None);
        };
        if now >= dynamic_object.expires_at {
            return Ok(None);
        }
        let delay = spell_damage_pushback_delay_millis(dynamic_object.damage_delay_count);
        dynamic_object.damage_delay_count = dynamic_object.damage_delay_count.saturating_add(1);
        let remaining = dynamic_object
            .expires_at
            .saturating_duration_since(now)
            .as_millis()
            .min(u128::from(u32::MAX)) as u32;
        if delay >= remaining {
            let Some(dynamic_object) = self.dynamic_objects.remove(&raw_guid) else {
                return Ok(None);
            };
            return self.destroy_dynamic_object_channel_event(dynamic_object);
        }
        dynamic_object.expires_at -= Duration::from_millis(delay as u64);
        let dynamic_object = dynamic_object.clone();
        let remaining = remaining - delay;
        self.dynamic_object_channel_update_event(dynamic_object, remaining)
    }

    fn dynamic_object_channel_update_event(
        &self,
        dynamic_object: DynamicObjectRuntime,
        remaining_millis: u32,
    ) -> anyhow::Result<Option<PlayerChannelEvent>> {
        let Some(caster_player) = self.players.get(&dynamic_object.caster_character_guid) else {
            return Ok(None);
        };
        let Some(direct_session_id) = caster_player.client_session_id() else {
            return Ok(None);
        };
        let packet = OutboundWorldPacket {
            opcode: WorldOpcode::MsgChannelUpdate as u16,
            body: build_channel_update_body(dynamic_object.caster, remaining_millis)?,
        };
        let mut observer_packets = Vec::new();
        for player_guid in self.nearby_player_guids(
            caster_player.position,
            CREATURE_SPAWN_RADIUS_YARDS,
            Some(dynamic_object.caster_character_guid),
        ) {
            let Some(session_id) = self
                .players
                .get(&player_guid)
                .and_then(PlayerRuntime::client_session_id)
            else {
                continue;
            };
            observer_packets.push((session_id, packet.clone()));
        }
        Ok(Some(PlayerChannelEvent {
            direct_session_id,
            direct_packets: vec![packet],
            observer_packets,
        }))
    }

    fn begin_dynamic_object_target_combat_packets(
        &mut self,
        dynamic_object: &DynamicObjectRuntime,
        target: ObjectGuid,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let Some(caster_session_id) = self
            .players
            .get(&dynamic_object.caster_character_guid)
            .and_then(PlayerRuntime::client_session_id)
        else {
            return Ok(Vec::new());
        };
        self.begin_db_creature_combat_packets_with_assistance(
            target,
            dynamic_object.caster,
            dynamic_object.caster_character_guid,
            caster_session_id,
            now,
        )
    }

    fn destroy_dynamic_object_packets(
        &mut self,
        dynamic_object: DynamicObjectRuntime,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let mut packets = Vec::new();
        let destroy_packet = OutboundWorldPacket {
            opcode: WorldOpcode::SmsgDestroyObject as u16,
            body: build_destroy_guid_body(dynamic_object.guid),
        };
        let player_guids = self.players.keys().copied().collect::<Vec<_>>();
        for player_guid in player_guids {
            let Some(player) = self.players.get_mut(&player_guid) else {
                continue;
            };
            if !player.visible_objects.remove(&dynamic_object.guid) {
                continue;
            }
            if let Some(session_id) = player.client_session_id() {
                packets.push((session_id, destroy_packet.clone()));
                if dynamic_object.channeled {
                    packets.push((
                        session_id,
                        OutboundWorldPacket {
                            opcode: WorldOpcode::MsgChannelUpdate as u16,
                            body: build_channel_update_body(dynamic_object.caster, 0)?,
                        },
                    ));
                    packets.push((
                        session_id,
                        OutboundWorldPacket {
                            opcode: WorldOpcode::SmsgUpdateObject as u16,
                            body: build_player_channel_update_body(dynamic_object.caster, None, 0)?,
                        },
                    ));
                }
            }
        }
        Ok(packets)
    }

    fn destroy_dynamic_object_channel_event(
        &mut self,
        dynamic_object: DynamicObjectRuntime,
    ) -> anyhow::Result<Option<PlayerChannelEvent>> {
        let Some(direct_session_id) = self
            .players
            .get(&dynamic_object.caster_character_guid)
            .and_then(PlayerRuntime::client_session_id)
        else {
            return Ok(None);
        };
        let direct_packets = vec![
            OutboundWorldPacket {
                opcode: WorldOpcode::SmsgDestroyObject as u16,
                body: build_destroy_guid_body(dynamic_object.guid),
            },
            OutboundWorldPacket {
                opcode: WorldOpcode::MsgChannelUpdate as u16,
                body: build_channel_update_body(dynamic_object.caster, 0)?,
            },
            OutboundWorldPacket {
                opcode: WorldOpcode::SmsgUpdateObject as u16,
                body: build_player_channel_update_body(dynamic_object.caster, None, 0)?,
            },
        ];
        let mut observer_packets = Vec::new();
        let player_guids = self.players.keys().copied().collect::<Vec<_>>();
        for player_guid in player_guids {
            let Some(player) = self.players.get_mut(&player_guid) else {
                continue;
            };
            if !player.visible_objects.remove(&dynamic_object.guid) {
                continue;
            }
            if player_guid == dynamic_object.caster_character_guid {
                continue;
            }
            let Some(session_id) = player.client_session_id() else {
                continue;
            };
            for packet in &direct_packets {
                observer_packets.push((session_id, packet.clone()));
            }
        }
        Ok(Some(PlayerChannelEvent {
            direct_session_id,
            direct_packets,
            observer_packets,
        }))
    }
}
