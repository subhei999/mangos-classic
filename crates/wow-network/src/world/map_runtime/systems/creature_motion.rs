use std::cmp::Reverse;
use wow_proto::world::WorldOpcode;

use super::*;

// Shared DB-creature motion and AI-transition authority.
const DB_CREATURE_SIGHT_AGGRO_CHECK_INTERVAL: Duration = Duration::from_millis(250);
const DB_CREATURE_SIGHT_AGGRO_RELOCATION_YARDS: f32 = 2.0;

#[derive(Debug)]
pub(in crate::world) struct DbCreatureIdleMotionTick {
    pub(in crate::world) creatures: Vec<DbCreatureRuntime>,
    pub(in crate::world) packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug, Clone, Copy, Default)]
pub(in crate::world) struct DbCreatureMotionAdvanceMetrics {
    pub(in crate::world) runtime_time: Duration,
    pub(in crate::world) spatial_update_time: Duration,
}

#[derive(Debug, Clone, Copy, Default)]
pub(in crate::world) struct DbCreatureMotionStartMetrics {
    pub(in crate::world) path_build_time: Duration,
    pub(in crate::world) snapshot_clone_time: Duration,
    pub(in crate::world) post_start_tracking_time: Duration,
}

#[derive(Debug)]
pub(in crate::world) struct DbCreatureMotionStartAttempt {
    pub(in crate::world) outcome:
        Option<(DbCreatureRuntime, Option<StartedCreatureMotion>, Vec<u32>)>,
    pub(in crate::world) metrics: DbCreatureMotionStartMetrics,
}

#[derive(Debug, Default)]
pub(in crate::world) struct ReadyDbCreatureIdleMotionAdvancements {
    pub(in crate::world) guids: Vec<u64>,
    pub(in crate::world) validation_time: Duration,
}

#[derive(Debug, Clone)]
pub(in crate::world) struct DbCreatureDistractUpdate {
    pub(in crate::world) creature: DbCreatureRuntime,
    pub(in crate::world) stop: Option<StoppedCreatureMotion>,
    pub(in crate::world) facing_position: WorldPosition,
    pub(in crate::world) facing_spline_id: u32,
}

impl MapRuntime {
    pub(in crate::world) fn advance_active_db_creature_idle_motions(
        &mut self,
        navigation: &DbCreatureNavigationGuardrail,
        now: Instant,
    ) -> anyhow::Result<DbCreatureIdleMotionTick> {
        self.advance_active_db_creature_idle_motions_with_interval(
            navigation,
            now,
            Duration::from_millis(WORLD_TICK_MILLIS),
        )
    }

    pub(in crate::world) fn advance_active_db_creature_idle_motions_with_interval(
        &mut self,
        navigation: &DbCreatureNavigationGuardrail,
        now: Instant,
        world_tick_interval: Duration,
    ) -> anyhow::Result<DbCreatureIdleMotionTick> {
        debug_assert!(!world_tick_interval.is_zero());
        if self.next_idle_motion_tick_at.is_some_and(|next| now < next) {
            return Ok(DbCreatureIdleMotionTick {
                creatures: Vec::new(),
                packets: Vec::new(),
            });
        }
        self.next_idle_motion_tick_at = Some(now + world_tick_interval);
        self.unload_expired_idle_grids(now);

        let mut creatures = Vec::new();
        let advancement_queue_started_at = Instant::now();
        let due_advancements = self.db_creature_idle_motion_advancement_guids(now);
        let advancement_queue_pop_time = advancement_queue_started_at.elapsed();
        let due_creature_count = due_advancements.guids.len();
        let mut packets = Vec::new();
        let mut motion_advance_time = Duration::ZERO;
        let mut spatial_update_time = Duration::ZERO;
        let mut motion_script_schedule_time = Duration::ZERO;
        for guid in due_advancements.guids {
            if let Some((creature, script_ids, metrics)) =
                self.advance_db_creature_motion(ObjectGuid::from_raw(guid), now)
            {
                motion_advance_time += metrics.runtime_time;
                spatial_update_time += metrics.spatial_update_time;
                self.sync_db_creature_idle_motion_tracking(guid);
                for script_id in script_ids {
                    let script_schedule_started_at = Instant::now();
                    self.schedule_db_creature_movement_script(creature.guid(), script_id, now);
                    motion_script_schedule_time += script_schedule_started_at.elapsed();
                }
                packets.extend(self.db_creature_check_for_help_packets_on_relocation(
                    ObjectGuid::from_raw(guid),
                    navigation,
                    now,
                )?);
                creatures.push(creature);
            }
        }

        let pending_script_started_at = Instant::now();
        packets.extend(self.advance_pending_db_scripts(now)?);
        let pending_script_execution_time = pending_script_started_at.elapsed();
        let mut start_schedule_rebuild_time = Duration::ZERO;
        let mut motion_start_broadcast_time = Duration::ZERO;
        if self.idle_motion_start_schedule_dirty {
            let start_schedule_rebuild_started_at = Instant::now();
            self.rebuild_db_creature_motion_start_schedules();
            start_schedule_rebuild_time += start_schedule_rebuild_started_at.elapsed();
        }
        let mut started_creatures = 0usize;
        let mut start_queue_pop_time = Duration::ZERO;
        let mut motion_start_time = Duration::ZERO;
        let mut motion_start_path_build_time = Duration::ZERO;
        let mut motion_start_snapshot_clone_time = Duration::ZERO;
        let mut motion_start_post_start_tracking_time = Duration::ZERO;
        if self
            .next_confused_motion_start_check_at
            .is_none_or(|next| now >= next)
        {
            let start_queue_started_at = Instant::now();
            let confused_guids = self.db_creature_confused_motion_start_guids(now);
            start_queue_pop_time += start_queue_started_at.elapsed();
            for guid in confused_guids {
                let motion_started_at = Instant::now();
                let creature_guid = ObjectGuid::from_raw(guid);
                let attempt =
                    self.start_db_creature_confused_motion(navigation, creature_guid, now);
                motion_start_path_build_time += attempt.metrics.path_build_time;
                motion_start_snapshot_clone_time += attempt.metrics.snapshot_clone_time;
                motion_start_post_start_tracking_time += attempt.metrics.post_start_tracking_time;
                let Some((creature, motion, script_ids)) = attempt.outcome else {
                    motion_start_time += motion_started_at.elapsed();
                    continue;
                };
                motion_start_time += motion_started_at.elapsed();
                for script_id in script_ids {
                    let script_schedule_started_at = Instant::now();
                    self.schedule_db_creature_movement_script(creature.guid(), script_id, now);
                    motion_script_schedule_time += script_schedule_started_at.elapsed();
                }
                if let Some(motion) = motion {
                    let broadcast_started_at = Instant::now();
                    packets.extend(self.broadcast_started_db_creature_motion(
                        creature_guid,
                        &creature,
                        &motion,
                    )?);
                    motion_start_broadcast_time += broadcast_started_at.elapsed();
                }
                packets.extend(self.db_creature_check_for_help_packets_on_relocation(
                    creature_guid,
                    navigation,
                    now,
                )?);
                started_creatures += 1;
                creatures.push(creature);
            }
        }
        if self
            .next_idle_motion_start_check_at
            .is_none_or(|next| now >= next)
        {
            let start_queue_started_at = Instant::now();
            let idle_guids = self.db_creature_idle_motion_start_guids(now);
            start_queue_pop_time += start_queue_started_at.elapsed();
            for guid in idle_guids {
                let motion_started_at = Instant::now();
                let creature_guid = ObjectGuid::from_raw(guid);
                let attempt = self.start_db_creature_idle_motion(navigation, creature_guid, now);
                motion_start_path_build_time += attempt.metrics.path_build_time;
                motion_start_snapshot_clone_time += attempt.metrics.snapshot_clone_time;
                motion_start_post_start_tracking_time += attempt.metrics.post_start_tracking_time;
                let Some((creature, motion, script_ids)) = attempt.outcome else {
                    motion_start_time += motion_started_at.elapsed();
                    continue;
                };
                motion_start_time += motion_started_at.elapsed();
                for script_id in script_ids {
                    let script_schedule_started_at = Instant::now();
                    self.schedule_db_creature_movement_script(creature.guid(), script_id, now);
                    motion_script_schedule_time += script_schedule_started_at.elapsed();
                }
                if let Some(motion) = motion {
                    let broadcast_started_at = Instant::now();
                    packets.extend(self.broadcast_started_db_creature_motion(
                        creature_guid,
                        &creature,
                        &motion,
                    )?);
                    motion_start_broadcast_time += broadcast_started_at.elapsed();
                }
                packets.extend(self.db_creature_check_for_help_packets_on_relocation(
                    creature_guid,
                    navigation,
                    now,
                )?);
                started_creatures += 1;
                creatures.push(creature);
            }
        }
        self.refresh_db_creature_motion_start_check_hints();
        crate::observability::record_idle_motion_due_creatures(due_creature_count);
        crate::observability::record_idle_motion_started_creatures(started_creatures);
        crate::observability::record_idle_motion_packets_emitted(packets.len());
        crate::observability::record_idle_motion_advancement_queue_pop_time(
            advancement_queue_pop_time,
        );
        crate::observability::record_idle_motion_advancement_validation_time(
            due_advancements.validation_time,
        );
        crate::observability::record_idle_motion_motion_advance_time(motion_advance_time);
        crate::observability::record_idle_motion_spatial_update_time(spatial_update_time);
        crate::observability::record_idle_motion_start_queue_pop_time(start_queue_pop_time);
        crate::observability::record_idle_motion_motion_start_time(motion_start_time);
        crate::observability::record_idle_motion_motion_start_path_build_time(
            motion_start_path_build_time,
        );
        crate::observability::record_idle_motion_motion_start_snapshot_clone_time(
            motion_start_snapshot_clone_time,
        );
        crate::observability::record_idle_motion_motion_start_post_start_tracking_time(
            motion_start_post_start_tracking_time,
        );
        crate::observability::record_idle_motion_motion_start_broadcast_time(
            motion_start_broadcast_time,
        );
        crate::observability::record_idle_motion_motion_script_schedule_time(
            motion_script_schedule_time,
        );
        crate::observability::record_idle_motion_pending_script_execution_time(
            pending_script_execution_time,
        );
        crate::observability::record_idle_motion_start_schedule_rebuild_time(
            start_schedule_rebuild_time,
        );

        Ok(DbCreatureIdleMotionTick { creatures, packets })
    }

    pub(in crate::world) fn invalidate_idle_motion_start_schedule(&mut self) {
        self.idle_motion_start_schedule_dirty = true;
        self.next_confused_motion_start_check_at = None;
        self.next_idle_motion_start_check_at = None;
        self.confused_db_creature_motion_start_due_at.clear();
        self.idle_db_creature_motion_start_due_at.clear();
        self.confused_db_creature_motion_starts.clear();
        self.idle_db_creature_motion_starts.clear();
    }

    fn broadcast_started_db_creature_motion(
        &mut self,
        creature_guid: ObjectGuid,
        creature: &DbCreatureRuntime,
        motion: &StartedCreatureMotion,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let move_packet = OutboundWorldPacket {
            opcode: WorldOpcode::SmsgMonsterMove as u16,
            body: build_monster_move_path_body_inner(
                creature_guid,
                motion.start,
                &motion.path,
                motion.spline_id,
                motion.duration.as_millis().max(1) as u32,
                None,
                motion.run,
            )?,
        };
        let create_packet = OutboundWorldPacket {
            opcode: WorldOpcode::SmsgUpdateObject as u16,
            body: build_update_object_body(&[build_db_creature_runtime_create_block(creature)?]),
        };
        let mut packets = Vec::new();
        for player_guid in
            self.nearby_player_guids(creature.current_position, CREATURE_SPAWN_RADIUS_YARDS, None)
        {
            if let Some(player) = self.players.get_mut(&player_guid) {
                if !player.visible_objects.contains(&creature_guid) {
                    player.visible_objects.insert(creature_guid);
                    if let Some(packet) = player.packet_to_client(create_packet.clone()) {
                        packets.push(packet);
                    }
                }
                if let Some(packet) = player.packet_to_client(move_packet.clone()) {
                    packets.push(packet);
                }
            }
        }
        Ok(packets)
    }

    pub(in crate::world) fn schedule_db_creature_movement_script(
        &mut self,
        creature_guid: ObjectGuid,
        script_id: u32,
        now: Instant,
    ) {
        let Some(commands) = self.db_scripts.movement_script(script_id) else {
            return;
        };
        for command in commands.iter().cloned() {
            let action = PendingDbScriptAction {
                due_at: now + Duration::from_millis(command.delay as u64),
                source: creature_guid,
                command,
            };
            self.pending_db_scripts
                .push(Reverse(ScheduledPendingDbScriptAction::new(
                    action,
                    self.next_pending_db_script_sequence,
                )));
            self.next_pending_db_script_sequence =
                self.next_pending_db_script_sequence.wrapping_add(1);
        }
    }

    pub(in crate::world) fn advance_pending_db_scripts(
        &mut self,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        if self.pending_db_scripts.is_empty() {
            return Ok(Vec::new());
        }

        let mut packets = Vec::new();
        while self
            .pending_db_scripts
            .peek()
            .is_some_and(|entry| entry.0.due_at <= now)
        {
            let Some(Reverse(entry)) = self.pending_db_scripts.pop() else {
                break;
            };
            packets.extend(self.execute_db_creature_script_action(entry.action)?);
        }
        Ok(packets)
    }

    pub(in crate::world) fn execute_db_creature_script_action(
        &mut self,
        action: PendingDbScriptAction,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        if action.command.condition_id != 0 {
            return Ok(Vec::new());
        }
        match action.command.command {
            SCRIPT_COMMAND_TALK => self.execute_db_script_talk(action.source, &action.command),
            SCRIPT_COMMAND_EMOTE => self.execute_db_script_emote(action.source, &action.command),
            SCRIPT_COMMAND_MORPH_TO_ENTRY_OR_MODEL => {
                self.execute_db_script_morph(action.source, &action.command)
            }
            _ => Ok(Vec::new()),
        }
    }

    pub(in crate::world) fn execute_db_script_talk(
        &mut self,
        source: ObjectGuid,
        command: &wow_db::DbScriptCommandQuery,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let Some(creature) = self.creatures.get(&source.raw()) else {
            return Ok(Vec::new());
        };
        let source_name = creature.spawn.template.name.clone();
        let source_position = creature.current_position;
        let Some(text_id) = db_script_random_nonzero_i32([
            command.dataint,
            command.dataint2,
            command.dataint3,
            command.dataint4,
        ]) else {
            return Ok(Vec::new());
        };
        let Some(text) = self.db_scripts.display_text(text_id) else {
            return Ok(Vec::new());
        };
        let text_content = text.content.to_string();
        let text_chat_type = text.chat_type;
        let text_language = text.language;
        let text_emote = text.emote;
        let mut packets = Vec::new();
        if text_emote != 0 {
            packets.extend(self.execute_db_script_emote_id(source, text_emote)?);
        }
        let Some((chat_msg, radius)) = db_script_chat_opcode_and_radius(text_chat_type) else {
            return Ok(packets);
        };
        let body = build_monster_message_chat_body(
            chat_msg,
            text_language,
            source,
            &source_name,
            &text_content,
        );
        let packet = OutboundWorldPacket {
            opcode: WorldOpcode::SmsgMessageChat as u16,
            body,
        };
        packets.extend(
            self.nearby_db_script_player_sessions(source_position, radius, source)
                .into_iter()
                .map(|session_id| (session_id, packet.clone())),
        );
        Ok(packets)
    }

    pub(in crate::world) fn execute_db_script_emote(
        &mut self,
        source: ObjectGuid,
        command: &wow_db::DbScriptCommandQuery,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let Some(emote) = db_script_random_nonzero_u32([
            command.datalong,
            command.dataint.max(0) as u32,
            command.dataint2.max(0) as u32,
            command.dataint3.max(0) as u32,
            command.dataint4.max(0) as u32,
        ]) else {
            return Ok(Vec::new());
        };
        self.execute_db_script_emote_id(source, emote)
    }

    pub(in crate::world) fn execute_db_script_emote_id(
        &mut self,
        source: ObjectGuid,
        emote: u32,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let Some(creature) = self.creatures.get_mut(&source.raw()) else {
            return Ok(Vec::new());
        };
        creature.spawn.addon_emote = emote;
        let position = creature.current_position;
        let packet = OutboundWorldPacket {
            opcode: WorldOpcode::SmsgUpdateObject as u16,
            body: build_db_creature_emote_state_update_body(source, emote)?,
        };
        Ok(self
            .nearby_db_script_player_sessions(position, CREATURE_SPAWN_RADIUS_YARDS, source)
            .into_iter()
            .map(|session_id| (session_id, packet.clone()))
            .collect())
    }

    pub(in crate::world) fn execute_db_script_morph(
        &mut self,
        source: ObjectGuid,
        command: &wow_db::DbScriptCommandQuery,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let Some(creature) = self.creatures.get_mut(&source.raw()) else {
            return Ok(Vec::new());
        };
        let display_id = if command.datalong == 0 {
            creature.display_id_override = None;
            creature.native_display.display_id
        } else if command.data_flags & SCRIPT_FLAG_COMMAND_ADDITIONAL != 0 {
            creature.display_id_override = Some(command.datalong);
            command.datalong
        } else {
            return Ok(Vec::new());
        };
        let position = creature.current_position;
        let packet = OutboundWorldPacket {
            opcode: WorldOpcode::SmsgUpdateObject as u16,
            body: build_db_creature_display_update_body(source, display_id)?,
        };
        Ok(self
            .nearby_db_script_player_sessions(position, CREATURE_SPAWN_RADIUS_YARDS, source)
            .into_iter()
            .map(|session_id| (session_id, packet.clone()))
            .collect())
    }

    pub(in crate::world) fn nearby_db_script_player_sessions(
        &self,
        position: WorldPosition,
        radius: f32,
        source: ObjectGuid,
    ) -> Vec<SessionId> {
        self.players
            .values()
            .filter(|player| player.visible_objects.contains(&source))
            .filter(|player| {
                radius.is_infinite() || is_position_inside_radius(position, player.position, radius)
            })
            .filter_map(PlayerRuntime::client_session_id)
            .collect()
    }

    pub(in crate::world) fn db_creatures_in_player_interest_radius(
        &self,
    ) -> Vec<(u64, &DbCreatureRuntime)> {
        if !self
            .players
            .values()
            .any(PlayerRuntime::is_client_controlled)
        {
            return Vec::new();
        }
        let mut guids = HashSet::new();
        for player in self
            .players
            .values()
            .filter(|player| player.is_client_controlled())
        {
            self.visit_nearby_cells(player.position, CREATURE_SPAWN_RADIUS_YARDS, |cell| {
                guids.extend(cell.creatures.iter().copied());
            });
        }
        guids
            .into_iter()
            .filter_map(|guid| {
                let creature = self.creatures.get(&guid)?;
                self.db_creature_has_player_interest(creature.current_position)
                    .then_some((guid, creature))
            })
            .collect()
    }

    pub(in crate::world) fn db_creature_idle_motion_advancement_guids(
        &mut self,
        now: Instant,
    ) -> ReadyDbCreatureIdleMotionAdvancements {
        let mut ready = ReadyDbCreatureIdleMotionAdvancements::default();
        let mut seen = HashSet::new();
        while self
            .idle_db_creature_motion_advances
            .peek()
            .is_some_and(|entry| entry.0.due_at <= now)
        {
            let Some(Reverse(entry)) = self.idle_db_creature_motion_advances.pop() else {
                break;
            };
            let validation_started_at = Instant::now();
            let Some(current_due_at) = self
                .db_creature_motion_advance_due_at
                .get(&entry.guid)
                .copied()
            else {
                ready.validation_time += validation_started_at.elapsed();
                continue;
            };
            if current_due_at != entry.due_at {
                ready.validation_time += validation_started_at.elapsed();
                continue;
            }
            let should_advance = self.creatures.get(&entry.guid).is_some_and(|creature| {
                self.db_creature_should_advance_idle_motion(entry.guid, creature)
            });
            ready.validation_time += validation_started_at.elapsed();
            if !should_advance {
                self.active_db_creature_motion_guids.remove(&entry.guid);
                self.db_creature_motion_advance_due_at.remove(&entry.guid);
                continue;
            }
            self.db_creature_motion_advance_due_at.remove(&entry.guid);
            if seen.insert(entry.guid) {
                ready.guids.push(entry.guid);
            }
        }
        ready
    }

    pub(in crate::world) fn db_creature_confused_motion_start_guids(
        &mut self,
        now: Instant,
    ) -> Vec<u64> {
        if self.idle_motion_start_schedule_dirty {
            self.rebuild_db_creature_motion_start_schedules();
        }
        let mut seen = HashSet::new();
        let mut guids = Vec::new();
        while self
            .confused_db_creature_motion_starts
            .peek()
            .is_some_and(|entry| entry.0.due_at <= now)
        {
            let Some(Reverse(entry)) = self.confused_db_creature_motion_starts.pop() else {
                break;
            };
            let Some(current_due_at) = self
                .confused_db_creature_motion_start_due_at
                .get(&entry.guid)
                .copied()
            else {
                continue;
            };
            if current_due_at != entry.due_at {
                continue;
            }
            let Some(creature) = self.creatures.get(&entry.guid) else {
                self.confused_db_creature_motion_start_due_at
                    .remove(&entry.guid);
                continue;
            };
            if self.db_creature_confused_motion_due_at(entry.guid, creature) != Some(entry.due_at)
                || !self.db_creature_has_player_interest(creature.current_position)
            {
                self.confused_db_creature_motion_start_due_at
                    .remove(&entry.guid);
                continue;
            }
            self.confused_db_creature_motion_start_due_at
                .remove(&entry.guid);
            if seen.insert(entry.guid) {
                guids.push(entry.guid);
            }
        }
        self.refresh_db_creature_motion_start_check_hints();
        guids
    }

    pub(in crate::world) fn db_creature_idle_motion_start_guids(
        &mut self,
        now: Instant,
    ) -> Vec<u64> {
        if self.idle_motion_start_schedule_dirty {
            self.rebuild_db_creature_motion_start_schedules();
        }
        let mut seen = HashSet::new();
        let mut guids = Vec::new();
        while self
            .idle_db_creature_motion_starts
            .peek()
            .is_some_and(|entry| entry.0.due_at <= now)
        {
            let Some(Reverse(entry)) = self.idle_db_creature_motion_starts.pop() else {
                break;
            };
            let Some(current_due_at) = self
                .idle_db_creature_motion_start_due_at
                .get(&entry.guid)
                .copied()
            else {
                continue;
            };
            if current_due_at != entry.due_at {
                continue;
            }
            let Some(creature) = self.creatures.get(&entry.guid) else {
                self.idle_db_creature_motion_start_due_at
                    .remove(&entry.guid);
                continue;
            };
            if self.db_creature_idle_motion_due_at(entry.guid, creature) != Some(entry.due_at)
                || !self.db_creature_has_player_interest(creature.current_position)
            {
                self.idle_db_creature_motion_start_due_at
                    .remove(&entry.guid);
                continue;
            }
            self.idle_db_creature_motion_start_due_at
                .remove(&entry.guid);
            if seen.insert(entry.guid) {
                guids.push(entry.guid);
            }
        }
        self.refresh_db_creature_motion_start_check_hints();
        guids
    }

    pub(in crate::world) fn advance_db_creature_motion(
        &mut self,
        creature_guid: ObjectGuid,
        now: Instant,
    ) -> Option<(DbCreatureRuntime, Vec<u32>, DbCreatureMotionAdvanceMetrics)> {
        let old_position = self.creatures.get(&creature_guid.raw())?.current_position;
        let creature = self.creatures.get_mut(&creature_guid.raw())?;
        let runtime_started_at = Instant::now();
        advance_db_creature_motion_runtime(creature, now);
        let runtime_time = runtime_started_at.elapsed();
        let script_ids = std::mem::take(&mut creature.pending_movement_scripts);
        let snapshot = creature.clone();
        let spatial_started_at = Instant::now();
        self.refresh_db_creature_spatial_index(
            creature_guid.raw(),
            old_position,
            snapshot.current_position,
        );
        let spatial_update_time = spatial_started_at.elapsed();
        Some((
            snapshot,
            script_ids,
            DbCreatureMotionAdvanceMetrics {
                runtime_time,
                spatial_update_time,
            },
        ))
    }

    pub(in crate::world) fn start_db_creature_idle_motion(
        &mut self,
        navigation: &DbCreatureNavigationGuardrail,
        creature_guid: ObjectGuid,
        now: Instant,
    ) -> DbCreatureMotionStartAttempt {
        let mut metrics = DbCreatureMotionStartMetrics::default();
        let path_build_started_at = Instant::now();
        if self
            .active_creature_combats
            .contains_key(&creature_guid.raw())
        {
            metrics.path_build_time = path_build_started_at.elapsed();
            return DbCreatureMotionStartAttempt {
                outcome: None,
                metrics,
            };
        }
        let geometry = self.geometry.clone();
        let Some(creature) = self.creatures.get_mut(&creature_guid.raw()) else {
            metrics.path_build_time = path_build_started_at.elapsed();
            return DbCreatureMotionStartAttempt {
                outcome: None,
                metrics,
            };
        };
        if !creature.is_alive() {
            metrics.path_build_time = path_build_started_at.elapsed();
            return DbCreatureMotionStartAttempt {
                outcome: None,
                metrics,
            };
        }
        if active_aura_has_confuse(&creature.active_auras) {
            metrics.path_build_time = path_build_started_at.elapsed();
            return DbCreatureMotionStartAttempt {
                outcome: None,
                metrics,
            };
        }
        let motion =
            start_db_creature_random_motion_runtime(navigation, Some(&geometry), creature, now)
                .or_else(|| {
                    start_db_creature_waypoint_motion_runtime(
                        navigation,
                        Some(&geometry),
                        creature,
                        now,
                    )
                });
        let script_ids = std::mem::take(&mut creature.pending_movement_scripts);
        if motion.is_none() && script_ids.is_empty() {
            metrics.path_build_time = path_build_started_at.elapsed();
            return DbCreatureMotionStartAttempt {
                outcome: None,
                metrics,
            };
        }
        metrics.path_build_time = path_build_started_at.elapsed();
        let clone_started_at = Instant::now();
        let snapshot = creature.clone();
        metrics.snapshot_clone_time = clone_started_at.elapsed();
        let post_start_tracking_started_at = Instant::now();
        self.sync_db_creature_idle_motion_tracking(creature_guid.raw());
        metrics.post_start_tracking_time = post_start_tracking_started_at.elapsed();
        DbCreatureMotionStartAttempt {
            outcome: Some((snapshot, motion, script_ids)),
            metrics,
        }
    }

    pub(in crate::world) fn start_db_creature_confused_motion(
        &mut self,
        navigation: &DbCreatureNavigationGuardrail,
        creature_guid: ObjectGuid,
        now: Instant,
    ) -> DbCreatureMotionStartAttempt {
        let mut metrics = DbCreatureMotionStartMetrics::default();
        let path_build_started_at = Instant::now();
        let geometry = self.geometry.clone();
        let Some(creature) = self.creatures.get_mut(&creature_guid.raw()) else {
            metrics.path_build_time = path_build_started_at.elapsed();
            return DbCreatureMotionStartAttempt {
                outcome: None,
                metrics,
            };
        };
        if !creature.is_alive() || !active_aura_has_confuse(&creature.active_auras) {
            metrics.path_build_time = path_build_started_at.elapsed();
            return DbCreatureMotionStartAttempt {
                outcome: None,
                metrics,
            };
        }
        let motion =
            start_db_creature_confused_motion_runtime(navigation, Some(&geometry), creature, now);
        let script_ids = std::mem::take(&mut creature.pending_movement_scripts);
        if motion.is_none() && script_ids.is_empty() {
            metrics.path_build_time = path_build_started_at.elapsed();
            return DbCreatureMotionStartAttempt {
                outcome: None,
                metrics,
            };
        }
        metrics.path_build_time = path_build_started_at.elapsed();
        let clone_started_at = Instant::now();
        let snapshot = creature.clone();
        metrics.snapshot_clone_time = clone_started_at.elapsed();
        let post_start_tracking_started_at = Instant::now();
        self.sync_db_creature_idle_motion_tracking(creature_guid.raw());
        metrics.post_start_tracking_time = post_start_tracking_started_at.elapsed();
        DbCreatureMotionStartAttempt {
            outcome: Some((snapshot, motion, script_ids)),
            metrics,
        }
    }

    pub(in crate::world) fn start_db_creature_chase_motion(
        &mut self,
        navigation: &DbCreatureNavigationGuardrail,
        creature_guid: ObjectGuid,
        target: ObjectGuid,
        target_position: WorldPosition,
        now: Instant,
    ) -> Option<(DbCreatureRuntime, StartedCreatureMotion)> {
        let geometry = self.geometry.clone();
        let chase_destination = self.db_creature_chase_slot_destination(
            navigation,
            creature_guid,
            target,
            target_position,
        );
        let creature = self.creatures.get_mut(&creature_guid.raw())?;
        if !creature.is_alive() {
            return None;
        }
        let motion = start_db_creature_chase_motion_runtime(
            navigation,
            Some(&geometry),
            creature,
            DbCreatureChaseTarget {
                guid: target,
                position: target_position,
            },
            chase_destination,
            now,
        )?;
        let snapshot = creature.clone();
        self.sync_db_creature_idle_motion_tracking(creature_guid.raw());
        Some((snapshot, motion))
    }

    pub(in crate::world) fn start_db_creature_flee_motion(
        &mut self,
        navigation: &DbCreatureNavigationGuardrail,
        creature_guid: ObjectGuid,
        source: ObjectGuid,
        source_position: WorldPosition,
        now: Instant,
        duration: Duration,
    ) -> Option<(DbCreatureRuntime, StartedCreatureMotion)> {
        let geometry = self.geometry.clone();
        let creature = self.creatures.get_mut(&creature_guid.raw())?;
        if !creature.is_alive() {
            return None;
        }
        let motion = start_db_creature_flee_motion_runtime(
            navigation,
            Some(&geometry),
            creature,
            source,
            source_position,
            now,
            duration,
        )?;
        let snapshot = creature.clone();
        self.sync_db_creature_idle_motion_tracking(creature_guid.raw());
        Some((snapshot, motion))
    }

    pub(in crate::world) fn start_db_creature_return_home_motion(
        &mut self,
        navigation: &DbCreatureNavigationGuardrail,
        creature_guid: ObjectGuid,
        now: Instant,
    ) -> Option<(DbCreatureRuntime, StartedCreatureMotion)> {
        let geometry = self.geometry.clone();
        let creature = self.creatures.get_mut(&creature_guid.raw())?;
        let motion = start_db_creature_return_home_motion_runtime(
            navigation,
            Some(&geometry),
            creature,
            now,
        )?;
        let snapshot = creature.clone();
        self.sync_db_creature_idle_motion_tracking(creature_guid.raw());
        Some((snapshot, motion))
    }

    pub(in crate::world) fn stop_db_creature_motion(
        &mut self,
        creature_guid: ObjectGuid,
    ) -> Option<(DbCreatureRuntime, StoppedCreatureMotion)> {
        let old_position = self.creatures.get(&creature_guid.raw())?.current_position;
        let creature = self.creatures.get_mut(&creature_guid.raw())?;
        if matches!(creature.motion, CreatureMotionState::Idle) {
            return None;
        }
        let stop = stop_db_creature_motion_runtime(creature);
        let snapshot = creature.clone();
        self.refresh_db_creature_spatial_index(
            creature_guid.raw(),
            old_position,
            snapshot.current_position,
        );
        Some((snapshot, stop))
    }

    pub(in crate::world) fn face_db_creature_toward_position(
        &mut self,
        creature_guid: ObjectGuid,
        target_position: WorldPosition,
    ) -> Option<(DbCreatureRuntime, WorldPosition, u32)> {
        let creature = self.creatures.get_mut(&creature_guid.raw())?;
        // CMaNGOS Unit::SetFacingToObject refuses in-place facing while a
        // movement spline is active. An arrived chase keeps its movement
        // generator for repath timing, but its spline has completed.
        let arrived_chase = match &creature.motion {
            CreatureMotionState::Idle => false,
            CreatureMotionState::Chase(chase)
                if db_creature_chase_motion_arrived(creature, chase) =>
            {
                true
            }
            _ => return None,
        };
        let dx = target_position.x - creature.current_position.x;
        let dy = target_position.y - creature.current_position.y;
        if dx * dx + dy * dy <= f32::EPSILON {
            return None;
        }
        let orientation = normalize_orientation(dy.atan2(dx));
        if (normalize_orientation(orientation - creature.current_position.orientation).min(
            normalize_orientation(creature.current_position.orientation - orientation),
        )) <= f32::EPSILON
        {
            return None;
        }
        creature.current_position.orientation = orientation;
        if arrived_chase {
            if let CreatureMotionState::Chase(chase) = &mut creature.motion {
                chase.destination.orientation = orientation;
            }
        }
        let spline_id = creature.next_spline_id;
        creature.next_spline_id = creature.next_spline_id.wrapping_add(1);
        Some((creature.clone(), creature.current_position, spline_id))
    }

    pub(in crate::world) fn apply_db_creature_distract(
        &mut self,
        creature_guid: ObjectGuid,
        target_position: WorldPosition,
        distract_until: Instant,
    ) -> Option<DbCreatureDistractUpdate> {
        if self
            .active_creature_combats
            .contains_key(&creature_guid.raw())
        {
            return None;
        }
        let stop = self
            .stop_db_creature_motion(creature_guid)
            .map(|(_, stop)| stop);
        {
            let creature = self.creatures.get_mut(&creature_guid.raw())?;
            if creature.has_waypoint_movement() {
                creature.next_waypoint_move_at = Some(
                    creature
                        .next_waypoint_move_at
                        .map_or(distract_until, |at| at.max(distract_until)),
                );
            } else if creature.random_wander_radius() > 0.0 {
                creature.next_random_move_at = Some(
                    creature
                        .next_random_move_at
                        .map_or(distract_until, |at| at.max(distract_until)),
                );
            }
        }
        let (creature, facing_position, facing_spline_id) =
            self.face_db_creature_toward_position(creature_guid, target_position)?;
        Some(DbCreatureDistractUpdate {
            creature,
            stop,
            facing_position,
            facing_spline_id,
        })
    }

    pub(in crate::world) fn prepare_db_creature_evade(
        &mut self,
        creature_guid: ObjectGuid,
    ) -> Option<DbCreatureRuntime> {
        let active_auras = {
            let creature = self.creatures.get_mut(&creature_guid.raw())?;
            creature.active_auras.clear();
            creature.aura_display_id_override = None;
            creature.refresh_move_speeds();
            creature.health = creature.max_health();
            creature.power1 = creature_mana(&creature.spawn.template);
            creature.life_state = DbCreatureLifeState::Alive;
            creature.corpse_expires_at = None;
            creature.respawn_at = None;
            creature.respawn_epoch_secs = None;
            creature.lootable = false;
            creature.looting = false;
            creature.loot_money = 0;
            creature.loot_money_available = false;
            creature.loot_items.clear();
            creature.loot_items_generated = false;
            creature.loot_kind = DbCreatureLootKind::Corpse;
            creature.pickpocket_restock_at = None;
            creature.loot_roll_released_slots.clear();
            creature.loot_current_looter_pass_slots.clear();
            creature.loot_owner = None;
            creature.triggered_event_ai_scripts.clear();
            creature.event_ai_cooldowns_until.clear();
            creature.event_ai_update_accum = Duration::ZERO;
            creature.next_event_ai_update_at = None;
            creature.clear_confused_motion();
            if matches!(creature.motion, CreatureMotionState::Confused(_)) {
                creature.motion = CreatureMotionState::Idle;
            }
            creature.active_auras.clone()
        };
        self.reconcile_target_aura_trackers(creature_guid, &active_auras, Instant::now());
        self.clear_db_creature_combat(creature_guid);
        self.sync_db_creature_lifecycle_tracking(creature_guid.raw());
        self.creatures.get(&creature_guid.raw()).cloned()
    }

    pub(in crate::world) fn select_db_creature_sight_aggro_targets(
        &mut self,
        faction_templates: &FactionTemplateStore,
        character: &ActiveCharacter,
        now: Instant,
    ) -> Vec<DbCreatureRuntime> {
        if character.position.map_id != self.map_id {
            return Vec::new();
        }
        if !self.player_sight_aggro_check_is_due(character, now) {
            return Vec::new();
        }
        let mut guids = HashSet::new();
        self.visit_nearby_cells(character.position, CREATURE_SPAWN_RADIUS_YARDS, |cell| {
            guids.extend(cell.creatures.iter().copied());
        });
        let mut targets = guids
            .into_iter()
            .filter_map(|guid| self.creatures.get(&guid))
            .filter(|creature| {
                !self
                    .active_creature_combats
                    .contains_key(&creature.guid().raw())
            })
            .filter(|creature| creature.can_aggro_player(faction_templates, character, now))
            .filter_map(|creature| {
                let distance_sq = creature.distance_to_player_squared(character)?;
                let attack_distance = db_creature_attack_distance(
                    character.level,
                    creature.spawn.template.min_level,
                    creature.spawn.template.detection_range,
                );
                (distance_sq <= attack_distance * attack_distance)
                    .then(|| (distance_sq, creature.guid()))
            })
            .collect::<Vec<_>>();
        targets.sort_by(|(left_distance, left_guid), (right_distance, right_guid)| {
            left_distance
                .total_cmp(right_distance)
                .then_with(|| left_guid.raw().cmp(&right_guid.raw()))
        });
        targets
            .into_iter()
            .filter_map(|(_, creature_guid)| self.creatures.get(&creature_guid.raw()).cloned())
            .collect()
    }

    fn player_sight_aggro_check_is_due(
        &mut self,
        character: &ActiveCharacter,
        now: Instant,
    ) -> bool {
        let Some(player) = self.players.get_mut(&character.guid) else {
            return true;
        };
        if player.flags & PLAYER_FLAGS_GM != 0 {
            return false;
        }
        let moved_enough = player
            .last_sight_aggro_check_position
            .is_some_and(|last_position| {
                distance_2d(
                    last_position.x,
                    last_position.y,
                    character.position.x,
                    character.position.y,
                ) >= DB_CREATURE_SIGHT_AGGRO_RELOCATION_YARDS
            });
        if moved_enough
            || player
                .next_sight_aggro_check_at
                .is_none_or(|next| now >= next)
        {
            player.last_sight_aggro_check_position = Some(character.position);
            player.next_sight_aggro_check_at = Some(now + DB_CREATURE_SIGHT_AGGRO_CHECK_INTERVAL);
            return true;
        }
        false
    }

    pub(in crate::world) fn select_db_creature_assist_targets(
        &mut self,
        faction_templates: &FactionTemplateStore,
        caller_guid: ObjectGuid,
        character: &ActiveCharacter,
    ) -> Option<(DbCreatureRuntime, Vec<ObjectGuid>)> {
        let caller = self.creatures.get_mut(&caller_guid.raw())?;
        if caller.already_called_assistance {
            return Some((caller.clone(), Vec::new()));
        }
        caller.already_called_assistance = true;
        let caller = caller.clone();
        let radius = if caller.spawn.template.call_for_help > 0 {
            caller.spawn.template.call_for_help as f32
        } else {
            DB_CREATURE_ASSISTANCE_RADIUS_YARDS
        };
        let mut targets = self
            .creatures
            .values()
            .filter(|creature| creature.guid() != caller_guid)
            .filter(|creature| {
                !self
                    .active_creature_combats
                    .contains_key(&creature.guid().raw())
            })
            .filter(|creature| creature.spawn.template.faction == caller.spawn.template.faction)
            .filter(|creature| {
                creature.can_aggro_player(faction_templates, character, Instant::now())
            })
            .filter_map(|creature| {
                let distance = distance_2d(
                    caller.current_position.x,
                    caller.current_position.y,
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
        Some((caller, targets.into_iter().map(|(_, guid)| guid).collect()))
    }

    pub(in crate::world) fn sync_db_creature_idle_motion_tracking(&mut self, creature_guid: u64) {
        let Some(creature) = self.creatures.get(&creature_guid) else {
            self.active_db_creature_motion_guids.remove(&creature_guid);
            self.db_creature_motion_advance_due_at
                .remove(&creature_guid);
            self.confused_db_creature_motion_start_due_at
                .remove(&creature_guid);
            self.idle_db_creature_motion_start_due_at
                .remove(&creature_guid);
            return;
        };
        let current_position = creature.current_position;
        let should_advance = self.db_creature_should_advance_idle_motion(creature_guid, creature);
        let confused_due_at = self.db_creature_confused_motion_due_at(creature_guid, creature);
        let idle_due_at = self.db_creature_idle_motion_due_at(creature_guid, creature);

        if should_advance {
            self.active_db_creature_motion_guids.insert(creature_guid);
            let due_at = self
                .db_creature_motion_advance_due_at
                .get(&creature_guid)
                .copied()
                .unwrap_or_else(|| self.next_idle_motion_tick_at.unwrap_or_else(Instant::now));
            self.set_db_creature_motion_advance_due_at(creature_guid, due_at);
        } else {
            self.active_db_creature_motion_guids.remove(&creature_guid);
            self.db_creature_motion_advance_due_at
                .remove(&creature_guid);
        }

        if self.idle_motion_start_schedule_dirty {
            return;
        }

        let has_interest = self.db_creature_has_player_interest(current_position);
        if has_interest {
            if let Some(due_at) = confused_due_at {
                self.set_db_creature_confused_motion_start_due_at(creature_guid, due_at);
            } else {
                self.confused_db_creature_motion_start_due_at
                    .remove(&creature_guid);
            }
            if let Some(due_at) = idle_due_at {
                self.set_db_creature_idle_motion_start_due_at(creature_guid, due_at);
            } else {
                self.idle_db_creature_motion_start_due_at
                    .remove(&creature_guid);
            }
        } else {
            self.confused_db_creature_motion_start_due_at
                .remove(&creature_guid);
            self.idle_db_creature_motion_start_due_at
                .remove(&creature_guid);
        }
        self.refresh_db_creature_motion_start_check_hints();
    }

    pub(in crate::world) fn sync_db_creature_idle_motion_tracking_for_player_interest_positions(
        &mut self,
        positions: &[WorldPosition],
    ) {
        let mut creature_guids = HashSet::new();
        for position in positions {
            self.visit_nearby_cells(*position, CREATURE_SPAWN_RADIUS_YARDS, |cell| {
                creature_guids.extend(cell.creatures.iter().copied());
            });
        }
        for creature_guid in creature_guids {
            self.sync_db_creature_idle_motion_tracking(creature_guid);
        }
    }

    fn set_db_creature_motion_advance_due_at(&mut self, creature_guid: u64, due_at: Instant) {
        if self
            .db_creature_motion_advance_due_at
            .get(&creature_guid)
            .is_some_and(|current| *current == due_at)
        {
            return;
        }
        self.db_creature_motion_advance_due_at
            .insert(creature_guid, due_at);
        self.idle_db_creature_motion_advances
            .push(Reverse(ScheduledDbCreatureMotionAdvance {
                due_at,
                guid: creature_guid,
            }));
    }

    fn set_db_creature_confused_motion_start_due_at(
        &mut self,
        creature_guid: u64,
        due_at: Instant,
    ) {
        if self
            .confused_db_creature_motion_start_due_at
            .get(&creature_guid)
            .is_some_and(|current| *current == due_at)
        {
            return;
        }
        self.confused_db_creature_motion_start_due_at
            .insert(creature_guid, due_at);
        self.confused_db_creature_motion_starts
            .push(Reverse(ScheduledDbCreatureMotionStart {
                due_at,
                guid: creature_guid,
            }));
    }

    fn set_db_creature_idle_motion_start_due_at(&mut self, creature_guid: u64, due_at: Instant) {
        if self
            .idle_db_creature_motion_start_due_at
            .get(&creature_guid)
            .is_some_and(|current| *current == due_at)
        {
            return;
        }
        self.idle_db_creature_motion_start_due_at
            .insert(creature_guid, due_at);
        self.idle_db_creature_motion_starts
            .push(Reverse(ScheduledDbCreatureMotionStart {
                due_at,
                guid: creature_guid,
            }));
    }

    pub(in crate::world) fn sync_db_creature_idle_motion_tracking_for_grid(
        &mut self,
        grid_coord: GridCoord,
    ) {
        let guids = self
            .grids
            .get(&grid_coord)
            .map(|grid| {
                grid.cells
                    .values()
                    .flat_map(|cell| cell.creatures.iter().copied())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for guid in guids {
            self.sync_db_creature_idle_motion_tracking(guid);
        }
    }

    fn rebuild_db_creature_motion_start_schedules(&mut self) {
        self.confused_db_creature_motion_start_due_at.clear();
        self.idle_db_creature_motion_start_due_at.clear();
        self.confused_db_creature_motion_starts.clear();
        self.idle_db_creature_motion_starts.clear();
        let scheduled = self
            .db_creatures_in_player_interest_radius()
            .into_iter()
            .map(|(guid, creature)| {
                (
                    guid,
                    self.db_creature_confused_motion_due_at(guid, creature),
                    self.db_creature_idle_motion_due_at(guid, creature),
                )
            })
            .collect::<Vec<_>>();
        for (guid, confused_due_at, idle_due_at) in scheduled {
            if let Some(due_at) = confused_due_at {
                self.set_db_creature_confused_motion_start_due_at(guid, due_at);
            }
            if let Some(due_at) = idle_due_at {
                self.set_db_creature_idle_motion_start_due_at(guid, due_at);
            }
        }
        self.idle_motion_start_schedule_dirty = false;
        self.refresh_db_creature_motion_start_check_hints();
    }

    fn refresh_db_creature_motion_start_check_hints(&mut self) {
        self.next_confused_motion_start_check_at =
            self.next_valid_db_creature_motion_start_at(true);
        self.next_idle_motion_start_check_at = self.next_valid_db_creature_motion_start_at(false);
    }

    fn next_valid_db_creature_motion_start_at(&mut self, confused: bool) -> Option<Instant> {
        loop {
            let entry = if confused {
                self.confused_db_creature_motion_starts.peek().copied()
            } else {
                self.idle_db_creature_motion_starts.peek().copied()
            }?;
            let Reverse(entry) = entry;
            let valid = self.creatures.get(&entry.guid).is_some_and(|creature| {
                let due_at = if confused {
                    self.confused_db_creature_motion_start_due_at
                        .get(&entry.guid)
                        .copied()
                } else {
                    self.idle_db_creature_motion_start_due_at
                        .get(&entry.guid)
                        .copied()
                };
                due_at == Some(entry.due_at)
                    && self.db_creature_has_player_interest(creature.current_position)
            });
            if valid {
                return Some(entry.due_at);
            }
            if confused {
                self.confused_db_creature_motion_start_due_at
                    .remove(&entry.guid);
            } else {
                self.idle_db_creature_motion_start_due_at
                    .remove(&entry.guid);
            }
            if confused {
                self.confused_db_creature_motion_starts.pop();
            } else {
                self.idle_db_creature_motion_starts.pop();
            }
        }
    }

    fn db_creature_should_advance_idle_motion(
        &self,
        creature_guid: u64,
        creature: &DbCreatureRuntime,
    ) -> bool {
        let is_confused_motion = matches!(creature.motion, CreatureMotionState::Confused(_));
        creature.is_alive()
            && !active_aura_blocks_movement(&creature.active_auras)
            && (is_confused_motion || !self.active_creature_combats.contains_key(&creature_guid))
            && self.db_creature_is_in_active_or_blocked_grid(creature.current_position)
            && matches!(
                creature.motion,
                CreatureMotionState::Random(_)
                    | CreatureMotionState::Confused(_)
                    | CreatureMotionState::Waypoint(_)
                    | CreatureMotionState::ReturnHome(_)
            )
    }

    fn db_creature_confused_motion_due_at(
        &self,
        _creature_guid: u64,
        creature: &DbCreatureRuntime,
    ) -> Option<Instant> {
        (creature.is_alive()
            && !active_aura_blocks_movement(&creature.active_auras)
            && matches!(creature.motion, CreatureMotionState::Idle)
            && active_aura_has_confuse(&creature.active_auras))
        .then_some(creature.next_confused_move_at)
        .flatten()
    }

    fn db_creature_idle_motion_due_at(
        &self,
        creature_guid: u64,
        creature: &DbCreatureRuntime,
    ) -> Option<Instant> {
        if !creature.is_alive()
            || self.active_creature_combats.contains_key(&creature_guid)
            || !matches!(creature.motion, CreatureMotionState::Idle)
            || active_aura_has_confuse(&creature.active_auras)
        {
            return None;
        }
        [creature.next_random_move_at, creature.next_waypoint_move_at]
            .into_iter()
            .flatten()
            .min()
    }

    fn db_creature_is_in_active_or_blocked_grid(&self, position: WorldPosition) -> bool {
        let grid = grid_coord_for_position(position);
        self.grids.get(&grid).is_some_and(|grid| {
            matches!(grid.state, GridState::Active | GridState::UnloadBlocked(_))
        })
    }

    fn db_creature_has_player_interest(&self, position: WorldPosition) -> bool {
        self.has_nearby_client_player(position, CREATURE_SPAWN_RADIUS_YARDS, None)
    }

    pub(in crate::world) fn refresh_db_creature_spatial_index(
        &mut self,
        creature_guid: u64,
        old_position: WorldPosition,
        new_position: WorldPosition,
    ) {
        let old_grid = grid_coord_for_position(old_position);
        let old_cell = cell_coord_for_position(old_position);
        let new_grid = grid_coord_for_position(new_position);
        let new_cell = cell_coord_for_position(new_position);
        if old_grid != new_grid || old_cell != new_cell {
            if let Some(grid) = self.grids.get_mut(&old_grid) {
                if let Some(cell) = grid.cells.get_mut(&old_cell) {
                    cell.creatures.remove(&creature_guid);
                }
                grid.last_touched = Instant::now();
            }
        }
        self.grids
            .entry(new_grid)
            .or_default()
            .cells
            .entry(new_cell)
            .or_default()
            .creatures
            .insert(creature_guid);
        if old_grid != new_grid {
            self.refresh_grid_state(old_grid);
        }
        self.refresh_grid_state(new_grid);
        self.sync_db_creature_idle_motion_tracking(creature_guid);
    }

    pub(in crate::world) fn db_creature_chase_slot_destination(
        &self,
        navigation: &DbCreatureNavigationGuardrail,
        creature_guid: ObjectGuid,
        target: ObjectGuid,
        target_position: WorldPosition,
    ) -> Option<WorldPosition> {
        let creature = self.creatures.get(&creature_guid.raw())?;
        let stop_distance = db_creature_chase_stop_distance(creature);
        let base_angle =
            db_creature_chase_angle_from_target(creature.current_position, target_position);
        let fan_angle = self.db_creature_chase_fan_angle(
            creature_guid,
            target,
            target_position,
            stop_distance,
            base_angle,
        );
        let geometry = self.geometry.clone();
        let searcher_bounding_radius = creature_bounding_radius(&creature.spawn.template);
        Some(db_creature_chase_near_point_with_cmangos_los_selector(
            target_position,
            stop_distance,
            fan_angle,
            searcher_bounding_radius,
            |candidate| {
                let candidate =
                    db_creature_ground_destination(Some(&geometry), candidate).unwrap_or(candidate);
                db_creature_has_line_of_sight(navigation, target_position, candidate)
            },
        ))
    }

    pub(in crate::world) fn db_creature_chase_fan_angle(
        &self,
        creature_guid: ObjectGuid,
        target: ObjectGuid,
        target_position: WorldPosition,
        stop_distance: f32,
        base_angle: f32,
    ) -> f32 {
        const FAN_OUT_RADIUS_YARDS: f32 = 1.0;
        const FAN_ANGLE_STEP: f32 = std::f32::consts::PI / 5.0;
        let mut angle = base_angle;
        for step in 1..=6 {
            let candidate = db_creature_chase_near_point(target_position, stop_distance, angle);
            let collides = self.creatures.values().any(|other| {
                if other.guid() == creature_guid {
                    return false;
                }
                let Some(combat) = self.active_creature_combats.get(&other.guid().raw()) else {
                    return false;
                };
                if combat.victim != target {
                    return false;
                }
                let occupied_position = match &other.motion {
                    CreatureMotionState::Idle => other.current_position,
                    CreatureMotionState::Chase(chase) if chase.target == target => {
                        chase.destination
                    }
                    _ => return false,
                };
                distance_2d(
                    occupied_position.x,
                    occupied_position.y,
                    candidate.x,
                    candidate.y,
                ) <= FAN_OUT_RADIUS_YARDS
            });
            if !collides {
                return angle;
            }
            let direction = if step % 2 == 0 { -1.0 } else { 1.0 };
            let magnitude = ((step + 1) / 2) as f32;
            angle = normalize_orientation(base_angle + direction * FAN_ANGLE_STEP * magnitude);
        }
        angle
    }
}
