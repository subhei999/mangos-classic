use super::*;
use wow_proto::world::WorldOpcode;

#[derive(Debug)]
pub(in crate::world) struct PlayerChannelEvent {
    pub(in crate::world) direct_session_id: SessionId,
    pub(in crate::world) direct_packets: Vec<OutboundWorldPacket>,
    pub(in crate::world) observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

impl MapRuntime {
    pub(in crate::world) fn start_player_self_aura_channel(
        &mut self,
        caster: ObjectGuid,
        caster_character_guid: u32,
        spell_id: u32,
        duration_millis: u32,
        channel_interrupt_flags: u32,
        now: Instant,
    ) -> anyhow::Result<Option<PlayerChannelEvent>> {
        if duration_millis == 0 {
            return Ok(None);
        }
        let Some(caster_player) = self.players.get(&caster_character_guid) else {
            return Ok(None);
        };
        let Some(direct_session_id) = caster_player.client_session_id() else {
            return Ok(None);
        };

        self.active_player_channels.insert(
            caster_character_guid,
            ActivePlayerChannel {
                caster,
                caster_character_guid,
                spell_id,
                target: None,
                expires_at: now + Duration::from_millis(duration_millis as u64),
                next_tick_at: None,
                tick_millis: 0,
                ticks_remaining: 0,
                channel_interrupt_flags,
                damage_delay_count: 0,
                triggered_spell_speed: 0.0,
                damage_effect: None,
            },
        );

        let direct_packets = vec![
            OutboundWorldPacket {
                opcode: WorldOpcode::MsgChannelStart as u16,
                body: build_channel_start_body(caster, spell_id, duration_millis)?,
            },
            OutboundWorldPacket {
                opcode: WorldOpcode::SmsgUpdateObject as u16,
                body: build_player_channel_update_body(caster, None, spell_id)?,
            },
        ];
        let observer_update = direct_packets[1].clone();
        let mut observer_packets = Vec::new();
        for player_guid in self.nearby_player_guids(
            caster_player.position,
            CREATURE_SPAWN_RADIUS_YARDS,
            Some(caster_character_guid),
        ) {
            let Some(session_id) = self
                .players
                .get(&player_guid)
                .and_then(PlayerRuntime::client_session_id)
            else {
                continue;
            };
            observer_packets.push((session_id, observer_update.clone()));
        }

        Ok(Some(PlayerChannelEvent {
            direct_session_id,
            direct_packets,
            observer_packets,
        }))
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::world) fn start_player_periodic_trigger_channel(
        &mut self,
        caster: ObjectGuid,
        caster_character_guid: u32,
        spell_id: u32,
        target: ObjectGuid,
        duration_millis: u32,
        tick_millis: u32,
        channel_interrupt_flags: u32,
        triggered_spell_speed: f32,
        damage_effect: PlayerDirectDamageEffect,
        now: Instant,
    ) -> anyhow::Result<Option<PlayerChannelEvent>> {
        if tick_millis == 0 || duration_millis == 0 {
            return Ok(None);
        }
        let Some(caster_player) = self.players.get(&caster_character_guid) else {
            return Ok(None);
        };
        let Some(direct_session_id) = caster_player.client_session_id() else {
            return Ok(None);
        };
        if !self
            .creatures
            .get(&target.raw())
            .is_some_and(DbCreatureRuntime::is_alive)
        {
            return Ok(None);
        }

        self.active_player_channels.insert(
            caster_character_guid,
            ActivePlayerChannel {
                caster,
                caster_character_guid,
                spell_id,
                target: Some(target),
                expires_at: now + Duration::from_millis(duration_millis as u64),
                next_tick_at: Some(now),
                tick_millis,
                ticks_remaining: channel_tick_count(duration_millis, tick_millis),
                channel_interrupt_flags,
                damage_delay_count: 0,
                triggered_spell_speed,
                damage_effect: Some(damage_effect),
            },
        );

        let mut direct_packets = vec![
            OutboundWorldPacket {
                opcode: WorldOpcode::MsgChannelStart as u16,
                body: build_channel_start_body(caster, spell_id, duration_millis)?,
            },
            OutboundWorldPacket {
                opcode: WorldOpcode::SmsgUpdateObject as u16,
                body: build_player_channel_update_body(caster, Some(target), spell_id)?,
            },
        ];
        let observer_update = direct_packets[1].clone();
        let mut observer_packets = Vec::new();
        for player_guid in self.nearby_player_guids(
            caster_player.position,
            CREATURE_SPAWN_RADIUS_YARDS,
            Some(caster_character_guid),
        ) {
            let Some(session_id) = self
                .players
                .get(&player_guid)
                .and_then(PlayerRuntime::client_session_id)
            else {
                continue;
            };
            observer_packets.push((session_id, observer_update.clone()));
        }

        Ok(Some(PlayerChannelEvent {
            direct_session_id,
            direct_packets: std::mem::take(&mut direct_packets),
            observer_packets,
        }))
    }

    pub(in crate::world) fn cancel_player_channel(
        &mut self,
        caster_character_guid: u32,
    ) -> anyhow::Result<Option<PlayerChannelEvent>> {
        let Some(channel) = self.active_player_channels.remove(&caster_character_guid) else {
            return Ok(None);
        };
        self.pending_player_channel_impacts
            .retain(|impact| impact.caster_character_guid != caster_character_guid);
        self.player_channel_clear_event(channel)
    }

    pub(in crate::world) fn clear_player_active_spell_runtime(
        &mut self,
        character_guid: u32,
    ) -> anyhow::Result<PlayerSpellRuntimeCleanupPackets> {
        self.active_player_spell_casts.remove(&character_guid);
        self.pending_spell_events
            .retain(|event| event.caster_character_guid != character_guid);

        let mut cleanup = PlayerSpellRuntimeCleanupPackets::default();
        if let Some(event) = self.cancel_player_channel(character_guid)? {
            cleanup.direct_packets.extend(event.direct_packets);
            cleanup.observer_packets.extend(event.observer_packets);
        }

        let dynamic_cleanup = self.remove_player_dynamic_objects(character_guid)?;
        cleanup
            .direct_packets
            .extend(dynamic_cleanup.direct_packets);
        cleanup
            .observer_packets
            .extend(dynamic_cleanup.observer_packets);
        Ok(cleanup)
    }

    pub(in crate::world) fn cancel_player_channel_for_movement(
        &mut self,
        caster_character_guid: u32,
    ) -> anyhow::Result<Option<PlayerChannelEvent>> {
        if !self
            .active_player_channels
            .get(&caster_character_guid)
            .is_some_and(|channel| {
                channel.channel_interrupt_flags & AURA_INTERRUPT_FLAG_MOVING != 0
            })
        {
            return Ok(None);
        }
        self.cancel_player_channel(caster_character_guid)
    }

    pub(in crate::world) fn cancel_player_channels_for_removed_target_auras(
        &mut self,
        target: ObjectGuid,
        removed_spell_ids: &[u32],
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        if removed_spell_ids.is_empty() {
            return Ok(Vec::new());
        }
        let removed_spell_ids = removed_spell_ids.iter().copied().collect::<HashSet<_>>();
        let caster_guids = self
            .active_player_channels
            .iter()
            .filter_map(|(caster_guid, channel)| {
                (channel.target == Some(target) && removed_spell_ids.contains(&channel.spell_id))
                    .then_some(*caster_guid)
            })
            .collect::<Vec<_>>();
        let mut packets = Vec::new();
        for caster_guid in caster_guids {
            if let Some(event) = self.cancel_player_channel(caster_guid)? {
                packets.extend(self.channel_event_packets(event));
            }
        }
        Ok(packets)
    }

    pub(in crate::world) fn interrupt_player_spell_work_targeting_unit(
        &mut self,
        target: ObjectGuid,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let mut packets = Vec::new();
        let active_casts = self
            .active_player_spell_casts
            .iter()
            .filter(|(_, active_cast)| active_cast.targets.unit_target == Some(target))
            .map(|(character_guid, active_cast)| (*character_guid, active_cast.clone()))
            .collect::<Vec<_>>();
        let mut interrupted_active = HashSet::new();
        for (character_guid, _active_cast) in active_casts {
            let Some(active_cast) = self.active_player_spell_casts.remove(&character_guid) else {
                continue;
            };
            self.clear_player_spell_recovery(character_guid, &active_cast.profile);
            interrupted_active.insert((character_guid, active_cast.spell_id));
            packets.extend(
                self.player_spell_interrupt_session_packets(character_guid, active_cast.spell_id)?,
            );
        }

        let channel_casters = self
            .active_player_channels
            .iter()
            .filter(|(_, channel)| channel.target == Some(target))
            .map(|(caster_guid, _)| *caster_guid)
            .collect::<Vec<_>>();
        for caster_guid in channel_casters {
            if let Some(event) = self.cancel_player_channel(caster_guid)? {
                packets.extend(self.channel_event_packets(event));
            }
        }

        let mut retained_events = Vec::with_capacity(self.pending_spell_events.len());
        let mut interrupted_pending = Vec::new();
        for event in self.pending_spell_events.drain(..) {
            if pending_spell_event_targets_unit(&event, target) {
                if !interrupted_active.contains(&(event.caster_character_guid, event.spell_id)) {
                    interrupted_pending.push((event.caster_character_guid, event.spell_id));
                }
            } else {
                retained_events.push(event);
            }
        }
        self.pending_spell_events = retained_events;
        interrupted_pending.sort_unstable();
        interrupted_pending.dedup();
        for (character_guid, spell_id) in interrupted_pending {
            packets.extend(self.player_spell_interrupt_session_packets(character_guid, spell_id)?);
        }

        self.pending_player_channel_impacts
            .retain(|impact| impact.target != target);

        Ok(packets)
    }

    fn player_spell_interrupt_session_packets(
        &self,
        character_guid: u32,
        spell_id: u32,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let Some(player) = self.players.get(&character_guid) else {
            return Ok(Vec::new());
        };
        let caster = ObjectGuid::new(HighGuid::Player, 0, character_guid);
        let cast_result = OutboundWorldPacket {
            opcode: WorldOpcode::SmsgCastResult as u16,
            body: build_cast_result_failure_body(spell_id, SPELL_FAILED_INTERRUPTED),
        };
        let failure_packet = OutboundWorldPacket {
            opcode: WorldOpcode::SmsgSpellFailure as u16,
            body: build_spell_failure_body(caster, spell_id, SPELL_FAILED_INTERRUPTED)?,
        };
        let failed_other_packet = OutboundWorldPacket {
            opcode: WorldOpcode::SmsgSpellFailedOther as u16,
            body: build_spell_failed_other_body(caster, spell_id),
        };
        let mut packets = Vec::new();
        if let Some(session_id) = player.client_session_id() {
            packets.push((session_id, cast_result));
            packets.push((session_id, failure_packet.clone()));
            packets.push((session_id, failed_other_packet.clone()));
        }
        for observer_guid in self.nearby_player_guids(
            player.position,
            PLAYER_VISIBILITY_RADIUS_YARDS,
            Some(character_guid),
        ) {
            let Some(session_id) = self
                .players
                .get(&observer_guid)
                .and_then(PlayerRuntime::client_session_id)
            else {
                continue;
            };
            packets.push((session_id, failure_packet.clone()));
            packets.push((session_id, failed_other_packet.clone()));
        }
        Ok(packets)
    }

    pub(in crate::world) fn advance_player_channels(
        &mut self,
        now: Instant,
        now_epoch_secs: u64,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let channel_keys = self
            .active_player_channels
            .keys()
            .copied()
            .collect::<Vec<_>>();
        let mut packets = Vec::new();
        for caster_character_guid in channel_keys {
            let Some(mut channel) = self
                .active_player_channels
                .get(&caster_character_guid)
                .cloned()
            else {
                continue;
            };
            if let Some(target) = channel.target {
                let target_alive = self
                    .creatures
                    .get(&target.raw())
                    .is_some_and(DbCreatureRuntime::is_alive);
                if !target_alive {
                    self.active_player_channels.remove(&caster_character_guid);
                    if let Some(event) = self.player_channel_clear_event(channel)? {
                        packets.extend(self.channel_event_packets(event));
                    }
                    continue;
                }
            }
            if let Some(mut next_tick_at) = channel.next_tick_at {
                if now >= next_tick_at
                    && next_tick_at < channel.expires_at
                    && channel.ticks_remaining > 0
                {
                    while next_tick_at <= now
                        && next_tick_at < channel.expires_at
                        && channel.ticks_remaining > 0
                    {
                        packets.extend(self.schedule_player_channel_tick(&channel, next_tick_at)?);
                        channel.ticks_remaining = channel.ticks_remaining.saturating_sub(1);
                        next_tick_at += Duration::from_millis(channel.tick_millis as u64);
                    }
                    if let Some(stored) =
                        self.active_player_channels.get_mut(&caster_character_guid)
                    {
                        stored.next_tick_at = Some(next_tick_at);
                        stored.ticks_remaining = channel.ticks_remaining;
                    }
                }
            }
            if now >= channel.expires_at {
                self.active_player_channels.remove(&caster_character_guid);
                if let Some(event) = self.player_channel_clear_event(channel)? {
                    packets.extend(self.channel_event_packets(event));
                }
            }
        }
        packets.extend(self.advance_player_channel_impacts(now, now_epoch_secs)?);
        Ok(packets)
    }

    fn schedule_player_channel_tick(
        &mut self,
        channel: &ActivePlayerChannel,
        tick_at: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let Some(caster_player) = self.players.get(&channel.caster_character_guid) else {
            return Ok(Vec::new());
        };
        let Some(caster_session_id) = caster_player.client_session_id() else {
            return Ok(Vec::new());
        };
        let (Some(target), Some(damage_effect)) = (channel.target, channel.damage_effect) else {
            return Ok(Vec::new());
        };
        let caster_position = caster_player.position;
        let caster_level = caster_player.level;
        let caster_class = caster_player.class;
        let caster_combat_stats = caster_player.combat_stats;
        let Some(target_creature) = self.creatures.get(&target.raw()).cloned() else {
            return Ok(Vec::new());
        };
        if !target_creature.is_alive() {
            return Ok(Vec::new());
        }
        let outcome = roll_spell_damage_outcome(spell_damage_outcome_input(
            damage_effect.damage,
            damage_effect.school,
            damage_effect.dmg_class,
            damage_effect.attributes_ex2,
            damage_effect.attributes_ex3,
            player_spell_snapshot(caster_level, caster_class, &caster_combat_stats),
            db_creature_spell_snapshot(&target_creature),
        ));
        let targets = SpellCastTargets {
            target_mask: SPELL_CAST_TARGET_UNIT,
            unit_target: Some(target),
            gameobject_target: None,
            source_location: None,
            destination: None,
        };
        let spell_go_body = outcome
            .miss_info
            .map(|miss_info| {
                build_spell_go_body_with_miss(
                    channel.caster,
                    damage_effect.spell_id,
                    &targets,
                    miss_info,
                )
            })
            .unwrap_or_else(|| {
                build_spell_go_body(channel.caster, damage_effect.spell_id, &targets)
            })?;
        let mut packets = vec![(
            caster_session_id,
            OutboundWorldPacket {
                opcode: WorldOpcode::SmsgSpellGo as u16,
                body: spell_go_body.clone(),
            },
        )];
        packets.extend(self.broadcast_packet_near_position(
            target_creature.current_position,
            CREATURE_SPAWN_RADIUS_YARDS,
            Some(channel.caster_character_guid),
            OutboundWorldPacket {
                opcode: WorldOpcode::SmsgSpellGo as u16,
                body: spell_go_body,
            },
        ));
        let travel_millis = spell_travel_millis_between(
            caster_position,
            target_creature.current_position,
            channel.triggered_spell_speed,
        );
        self.pending_player_channel_impacts
            .push(PendingPlayerChannelImpact {
                caster: channel.caster,
                caster_character_guid: channel.caster_character_guid,
                target,
                impact_at: tick_at + Duration::from_millis(travel_millis as u64),
                damage_effect,
                outcome,
            });
        Ok(packets)
    }

    fn advance_player_channel_impacts(
        &mut self,
        now: Instant,
        now_epoch_secs: u64,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        if self.pending_player_channel_impacts.is_empty() {
            return Ok(Vec::new());
        }
        let mut due = Vec::new();
        let mut pending = Vec::new();
        for impact in self.pending_player_channel_impacts.drain(..) {
            if impact.impact_at <= now {
                due.push(impact);
            } else {
                pending.push(impact);
            }
        }
        self.pending_player_channel_impacts = pending;
        due.sort_by_key(|impact| impact.impact_at);
        let mut packets = Vec::new();
        for impact in due {
            packets.extend(self.apply_player_channel_impact(impact, now, now_epoch_secs)?);
        }
        Ok(packets)
    }

    fn apply_player_channel_impact(
        &mut self,
        impact: PendingPlayerChannelImpact,
        now: Instant,
        now_epoch_secs: u64,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let Some(caster_session_id) = self
            .players
            .get(&impact.caster_character_guid)
            .and_then(PlayerRuntime::client_session_id)
        else {
            return Ok(Vec::new());
        };
        if !self
            .creatures
            .get(&impact.target.raw())
            .is_some_and(DbCreatureRuntime::is_alive)
        {
            return Ok(Vec::new());
        }
        let Some(caster_position) = self
            .players
            .get(&impact.caster_character_guid)
            .map(|player| player.position)
        else {
            return Ok(Vec::new());
        };
        let Some(target_creature) = self.creatures.get(&impact.target.raw()).cloned() else {
            return Ok(Vec::new());
        };
        let mut packets = self.begin_player_channel_target_combat_packets(
            impact.caster,
            impact.caster_character_guid,
            impact.target,
            caster_session_id,
            caster_position,
            &target_creature,
            now,
        )?;
        let Some(event) = self.apply_db_creature_damage(DbCreatureDamageRequest {
            creature_guid: impact.target,
            killer: impact.caster,
            damage: impact.outcome.final_damage,
            melee_outcome: None,
            spell_damage_outcome: Some(impact.outcome),
            spell_id: Some(impact.damage_effect.spell_id),
            spell_school: impact.damage_effect.school,
            suppress_attacker_state: true,
            now,
            now_epoch_secs,
            exclude_character_guid: Some(impact.caster_character_guid),
            corpse_loot: None,
        })?
        else {
            return Ok(Vec::new());
        };

        if let Some(body) = event.spell_non_melee_log_body {
            packets.push((
                caster_session_id,
                OutboundWorldPacket {
                    opcode: WorldOpcode::SmsgSpellNonMeleeDamageLog as u16,
                    body,
                },
            ));
        }
        if let Some(body) = event.spell_miss_log_body {
            packets.push((
                caster_session_id,
                OutboundWorldPacket {
                    opcode: WorldOpcode::SmsgSpellLogMiss as u16,
                    body,
                },
            ));
        }
        packets.push((
            caster_session_id,
            OutboundWorldPacket {
                opcode: WorldOpcode::SmsgUpdateObject as u16,
                body: event.update_body,
            },
        ));
        packets.extend(
            event
                .direct_packets
                .into_iter()
                .map(|packet| (caster_session_id, packet)),
        );
        if let Some(death) = event.death_finalization {
            if let Some(packet) = death.motion_stop_packet {
                packets.push((caster_session_id, packet));
            }
            packets.push((caster_session_id, death.combat_flag_packet));
            packets.push((caster_session_id, death.attack_stop_packet));
            packets.extend(death.observer_packets);
        }
        packets.extend(event.observer_packets);
        Ok(packets)
    }

    pub(in crate::world) fn interrupt_player_channel_for_damage(
        &mut self,
        caster_character_guid: u32,
        now: Instant,
    ) -> anyhow::Result<Option<PlayerChannelEvent>> {
        let Some(channel) = self
            .active_player_channels
            .get(&caster_character_guid)
            .cloned()
        else {
            return Ok(None);
        };
        if channel.channel_interrupt_flags & AURA_INTERRUPT_FLAG_DAMAGE_CHANNEL_DURATION != 0 {
            return self.delay_player_channel_for_damage(caster_character_guid, now);
        }
        if channel.channel_interrupt_flags & AURA_INTERRUPT_FLAG_DAMAGE != 0 {
            self.active_player_channels.remove(&caster_character_guid);
            self.pending_player_channel_impacts
                .retain(|impact| impact.caster_character_guid != caster_character_guid);
            return self.player_channel_clear_event(channel);
        }
        Ok(None)
    }

    fn delay_player_channel_for_damage(
        &mut self,
        caster_character_guid: u32,
        now: Instant,
    ) -> anyhow::Result<Option<PlayerChannelEvent>> {
        let Some(channel) = self.active_player_channels.get_mut(&caster_character_guid) else {
            return Ok(None);
        };
        if now >= channel.expires_at {
            return Ok(None);
        }
        let delay = spell_damage_pushback_delay_millis(channel.damage_delay_count);
        channel.damage_delay_count = channel.damage_delay_count.saturating_add(1);
        let remaining = channel
            .expires_at
            .saturating_duration_since(now)
            .as_millis()
            .min(u128::from(u32::MAX)) as u32;
        if delay >= remaining {
            let Some(channel) = self.active_player_channels.remove(&caster_character_guid) else {
                return Ok(None);
            };
            self.pending_player_channel_impacts
                .retain(|impact| impact.caster_character_guid != caster_character_guid);
            return self.player_channel_clear_event(channel);
        }
        channel.expires_at -= Duration::from_millis(delay as u64);
        let channel = channel.clone();
        let remaining = remaining - delay;
        self.player_channel_update_event(channel, remaining)
    }

    fn player_channel_update_event(
        &self,
        channel: ActivePlayerChannel,
        remaining_millis: u32,
    ) -> anyhow::Result<Option<PlayerChannelEvent>> {
        let Some(caster_player) = self.players.get(&channel.caster_character_guid) else {
            return Ok(None);
        };
        let Some(direct_session_id) = caster_player.client_session_id() else {
            return Ok(None);
        };
        let packet = OutboundWorldPacket {
            opcode: WorldOpcode::MsgChannelUpdate as u16,
            body: build_channel_update_body(channel.caster, remaining_millis)?,
        };
        let mut observer_packets = Vec::new();
        for player_guid in self.nearby_player_guids(
            caster_player.position,
            CREATURE_SPAWN_RADIUS_YARDS,
            Some(channel.caster_character_guid),
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

    #[allow(clippy::too_many_arguments)]
    fn begin_player_channel_target_combat_packets(
        &mut self,
        caster: ObjectGuid,
        caster_character_guid: u32,
        target: ObjectGuid,
        caster_session_id: SessionId,
        _caster_position: WorldPosition,
        _target_creature: &DbCreatureRuntime,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        self.begin_db_creature_combat_packets_with_assistance(
            target,
            caster,
            caster_character_guid,
            caster_session_id,
            now,
        )
    }

    fn player_channel_clear_event(
        &self,
        channel: ActivePlayerChannel,
    ) -> anyhow::Result<Option<PlayerChannelEvent>> {
        let Some(caster_player) = self.players.get(&channel.caster_character_guid) else {
            return Ok(None);
        };
        let Some(direct_session_id) = caster_player.client_session_id() else {
            return Ok(None);
        };
        let direct_packets = vec![
            OutboundWorldPacket {
                opcode: WorldOpcode::MsgChannelUpdate as u16,
                body: build_channel_update_body(channel.caster, 0)?,
            },
            OutboundWorldPacket {
                opcode: WorldOpcode::SmsgUpdateObject as u16,
                body: build_player_channel_update_body(channel.caster, None, 0)?,
            },
        ];
        let mut observer_packets = Vec::new();
        for player_guid in self.nearby_player_guids(
            caster_player.position,
            CREATURE_SPAWN_RADIUS_YARDS,
            Some(channel.caster_character_guid),
        ) {
            let Some(session_id) = self
                .players
                .get(&player_guid)
                .and_then(PlayerRuntime::client_session_id)
            else {
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

    fn channel_event_packets(
        &self,
        event: PlayerChannelEvent,
    ) -> Vec<(SessionId, OutboundWorldPacket)> {
        let mut packets = event.observer_packets;
        for packet in event.direct_packets {
            packets.push((event.direct_session_id, packet));
        }
        packets
    }
}

fn spell_travel_millis_between(source: WorldPosition, target: WorldPosition, speed: f32) -> u32 {
    if speed <= 0.0 {
        return 0;
    }
    let distance = source.distance_to(&target).max(5.0);
    ((distance / speed) * 1000.0).floor().max(1.0) as u32
}

fn channel_tick_count(duration_millis: u32, tick_millis: u32) -> u32 {
    if duration_millis == 0 || tick_millis == 0 {
        return 0;
    }
    duration_millis.div_ceil(tick_millis).max(1)
}

fn pending_spell_event_targets_unit(event: &PendingSpellEvent, target: ObjectGuid) -> bool {
    match &event.kind {
        PendingSpellEventKind::Spell { targets, .. } => targets.unit_target == Some(target),
        PendingSpellEventKind::RangedAutoAttack {
            target: event_target,
            ..
        } => *event_target == target,
    }
}
