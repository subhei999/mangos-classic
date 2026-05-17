use super::*;

// Shared DB-creature motion and AI-transition authority.

#[derive(Debug)]
pub(in crate::world) struct DbCreatureIdleMotionTick {
    pub(in crate::world) creatures: Vec<DbCreatureRuntime>,
    pub(in crate::world) packets: Vec<(SessionId, OutboundWorldPacket)>,
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
        for guid in self.db_creature_idle_motion_advancement_guids() {
            if let Some((creature, script_ids)) =
                self.advance_db_creature_motion(ObjectGuid::from_raw(guid), now)
            {
                for script_id in script_ids {
                    self.schedule_db_creature_movement_script(creature.guid(), script_id, now);
                }
                creatures.push(creature);
            }
        }

        let mut packets = Vec::new();
        packets.extend(self.advance_pending_db_scripts(now)?);
        if self
            .next_idle_motion_start_check_at
            .is_none_or(|next| now >= next)
        {
            for guid in self.db_creature_idle_motion_start_guids(now) {
                let creature_guid = ObjectGuid::from_raw(guid);
                let Some((creature, motion, script_ids)) =
                    self.start_db_creature_idle_motion(navigation, creature_guid, now)
                else {
                    continue;
                };
                for script_id in script_ids {
                    self.schedule_db_creature_movement_script(creature.guid(), script_id, now);
                }
                let Some(motion) = motion else {
                    creatures.push(creature);
                    continue;
                };
                let move_packet = OutboundWorldPacket {
                    opcode: SMSG_MONSTER_MOVE,
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
                    opcode: SMSG_UPDATE_OBJECT,
                    body: build_update_object_body(&[build_db_creature_runtime_create_block(
                        &creature,
                    )?]),
                };
                for player_guid in self.nearby_player_guids(
                    creature.current_position,
                    CREATURE_SPAWN_RADIUS_YARDS,
                    None,
                ) {
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
                creatures.push(creature);
            }
            self.next_idle_motion_start_check_at = self.next_db_creature_idle_motion_start_at();
        }

        Ok(DbCreatureIdleMotionTick { creatures, packets })
    }

    pub(in crate::world) fn invalidate_idle_motion_start_schedule(&mut self) {
        self.next_idle_motion_start_check_at = None;
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
        self.pending_db_scripts
            .extend(
                commands
                    .iter()
                    .cloned()
                    .map(|command| PendingDbScriptAction {
                        due_at: now + Duration::from_millis(command.delay as u64),
                        source: creature_guid,
                        command,
                    }),
            );
        self.pending_db_scripts
            .sort_by_key(|action| (action.due_at, action.command.priority));
    }

    pub(in crate::world) fn advance_pending_db_scripts(
        &mut self,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        if self.pending_db_scripts.is_empty() {
            return Ok(Vec::new());
        }

        let mut due = Vec::new();
        let mut pending = Vec::new();
        for action in self.pending_db_scripts.drain(..) {
            if action.due_at <= now {
                due.push(action);
            } else {
                pending.push(action);
            }
        }
        self.pending_db_scripts = pending;

        let mut packets = Vec::new();
        for action in due {
            packets.extend(self.execute_db_creature_script_action(action)?);
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
            opcode: SMSG_MESSAGECHAT,
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
            opcode: SMSG_UPDATE_OBJECT,
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
            opcode: SMSG_UPDATE_OBJECT,
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

    pub(in crate::world) fn db_creatures_in_active_or_blocked_grids(
        &self,
    ) -> Vec<(u64, &DbCreatureRuntime)> {
        self.creatures
            .iter()
            .filter(|(_, creature)| {
                let grid = grid_coord_for_position(creature.current_position);
                self.grids.get(&grid).is_some_and(|grid| {
                    matches!(grid.state, GridState::Active | GridState::UnloadBlocked(_))
                })
            })
            .map(|(guid, creature)| (*guid, creature))
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
                self.players
                    .values()
                    .filter(|player| player.is_client_controlled())
                    .any(|player| {
                        is_position_inside_radius(
                            creature.current_position,
                            player.position,
                            CREATURE_SPAWN_RADIUS_YARDS,
                        )
                    })
                    .then_some((guid, creature))
            })
            .collect()
    }

    pub(in crate::world) fn db_creature_idle_motion_advancement_guids(&self) -> Vec<u64> {
        let mut guids = self
            .db_creatures_in_active_or_blocked_grids()
            .into_iter()
            .filter_map(|(guid, creature)| {
                (creature.is_alive()
                    && !self.active_creature_combats.contains_key(&guid)
                    && matches!(
                        creature.motion,
                        CreatureMotionState::Random(_) | CreatureMotionState::Waypoint(_)
                    ))
                .then_some(guid)
            })
            .collect::<Vec<_>>();
        guids.sort_unstable();
        guids
    }

    pub(in crate::world) fn db_creature_idle_motion_start_guids(&self, now: Instant) -> Vec<u64> {
        let mut guids = self
            .db_creatures_in_player_interest_radius()
            .into_iter()
            .filter_map(|(guid, creature)| {
                (creature.is_alive()
                    && !self.active_creature_combats.contains_key(&guid)
                    && matches!(creature.motion, CreatureMotionState::Idle)
                    && (creature.next_random_move_at.is_some_and(|at| now >= at)
                        || creature.next_waypoint_move_at.is_some_and(|at| now >= at)))
                .then_some(guid)
            })
            .collect::<Vec<_>>();
        guids.sort_unstable();
        guids
    }

    pub(in crate::world) fn next_db_creature_idle_motion_start_at(&self) -> Option<Instant> {
        self.db_creatures_in_player_interest_radius()
            .into_iter()
            .filter(|(guid, creature)| {
                creature.is_alive()
                    && !self.active_creature_combats.contains_key(guid)
                    && matches!(creature.motion, CreatureMotionState::Idle)
            })
            .filter_map(|(_, creature)| {
                [creature.next_random_move_at, creature.next_waypoint_move_at]
                    .into_iter()
                    .flatten()
                    .min()
            })
            .min()
    }

    pub(in crate::world) fn advance_db_creature_motion(
        &mut self,
        creature_guid: ObjectGuid,
        now: Instant,
    ) -> Option<(DbCreatureRuntime, Vec<u32>)> {
        let old_position = self.creatures.get(&creature_guid.raw())?.current_position;
        let creature = self.creatures.get_mut(&creature_guid.raw())?;
        advance_db_creature_motion_runtime(creature, now);
        let script_ids = std::mem::take(&mut creature.pending_movement_scripts);
        let snapshot = creature.clone();
        self.refresh_db_creature_spatial_index(
            creature_guid.raw(),
            old_position,
            snapshot.current_position,
        );
        Some((snapshot, script_ids))
    }

    pub(in crate::world) fn start_db_creature_idle_motion(
        &mut self,
        navigation: &DbCreatureNavigationGuardrail,
        creature_guid: ObjectGuid,
        now: Instant,
    ) -> Option<(DbCreatureRuntime, Option<StartedCreatureMotion>, Vec<u32>)> {
        if self
            .active_creature_combats
            .contains_key(&creature_guid.raw())
        {
            return None;
        }
        let old_position = self.creatures.get(&creature_guid.raw())?.current_position;
        let geometry = self.geometry.clone();
        let creature = self.creatures.get_mut(&creature_guid.raw())?;
        if !creature.is_alive() {
            return None;
        }
        let motion = if active_aura_has_confuse(&creature.active_auras) {
            start_db_creature_confused_motion_runtime(navigation, Some(&geometry), creature, now)
        } else {
            start_db_creature_random_motion_runtime(navigation, Some(&geometry), creature, now)
                .or_else(|| {
                    start_db_creature_waypoint_motion_runtime(
                        navigation,
                        Some(&geometry),
                        creature,
                        now,
                    )
                })
        };
        let script_ids = std::mem::take(&mut creature.pending_movement_scripts);
        if motion.is_none() && script_ids.is_empty() {
            return None;
        }
        let snapshot = creature.clone();
        self.refresh_db_creature_spatial_index(
            creature_guid.raw(),
            old_position,
            snapshot.current_position,
        );
        Some((snapshot, motion, script_ids))
    }

    pub(in crate::world) fn start_db_creature_chase_motion(
        &mut self,
        navigation: &DbCreatureNavigationGuardrail,
        creature_guid: ObjectGuid,
        target: ObjectGuid,
        target_position: WorldPosition,
        now: Instant,
    ) -> Option<(DbCreatureRuntime, StartedCreatureMotion)> {
        let old_position = self.creatures.get(&creature_guid.raw())?.current_position;
        let geometry = self.geometry.clone();
        let chase_destination =
            self.db_creature_chase_slot_destination(creature_guid, target, target_position);
        let creature = self.creatures.get_mut(&creature_guid.raw())?;
        if !creature.is_alive() {
            return None;
        }
        let motion = start_db_creature_chase_motion_runtime(
            navigation,
            Some(&geometry),
            creature,
            target,
            target_position,
            chase_destination,
            now,
        )?;
        let snapshot = creature.clone();
        self.refresh_db_creature_spatial_index(
            creature_guid.raw(),
            old_position,
            snapshot.current_position,
        );
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
        let old_position = self.creatures.get(&creature_guid.raw())?.current_position;
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
        self.refresh_db_creature_spatial_index(
            creature_guid.raw(),
            old_position,
            snapshot.current_position,
        );
        Some((snapshot, motion))
    }

    pub(in crate::world) fn start_db_creature_return_home_motion(
        &mut self,
        navigation: &DbCreatureNavigationGuardrail,
        creature_guid: ObjectGuid,
        now: Instant,
    ) -> Option<(DbCreatureRuntime, StartedCreatureMotion)> {
        let old_position = self.creatures.get(&creature_guid.raw())?.current_position;
        let geometry = self.geometry.clone();
        let creature = self.creatures.get_mut(&creature_guid.raw())?;
        let motion = start_db_creature_return_home_motion_runtime(
            navigation,
            Some(&geometry),
            creature,
            now,
        )?;
        let snapshot = creature.clone();
        self.refresh_db_creature_spatial_index(
            creature_guid.raw(),
            old_position,
            snapshot.current_position,
        );
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
        // CMaNGOS Unit::SetFacingToObject refuses in-place facing while a movement
        // spline is active. Chase splines already carry the target-facing flag.
        if !matches!(creature.motion, CreatureMotionState::Idle) {
            return None;
        }
        let dx = target_position.x - creature.current_position.x;
        let dy = target_position.y - creature.current_position.y;
        creature.current_position.orientation = normalize_orientation(dy.atan2(dx));
        let spline_id = creature.next_spline_id;
        creature.next_spline_id = creature.next_spline_id.wrapping_add(1);
        Some((creature.clone(), creature.current_position, spline_id))
    }

    pub(in crate::world) fn prepare_db_creature_evade(
        &mut self,
        creature_guid: ObjectGuid,
    ) -> Option<DbCreatureRuntime> {
        let creature = self.creatures.get_mut(&creature_guid.raw())?;
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
        creature.loot_roll_released_slots.clear();
        creature.loot_current_looter_pass_slots.clear();
        creature.loot_owner = None;
        creature.triggered_event_ai_scripts.clear();
        creature.event_ai_cooldowns_until.clear();
        self.clear_db_creature_combat(creature_guid);
        self.creatures.get(&creature_guid.raw()).cloned()
    }

    pub(in crate::world) fn select_db_creature_sight_aggro_targets(
        &self,
        faction_templates: &FactionTemplateStore,
        character: &ActiveCharacter,
        now: Instant,
    ) -> Vec<DbCreatureRuntime> {
        if character.position.map_id != self.map_id {
            return Vec::new();
        }
        if self
            .players
            .get(&character.guid)
            .is_some_and(|player| player.flags & PLAYER_FLAGS_GM != 0)
        {
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
                    .then(|| (distance_sq, creature.guid(), creature.clone()))
            })
            .collect::<Vec<_>>();
        targets.sort_by(
            |(left_distance, left_guid, _), (right_distance, right_guid, _)| {
                left_distance
                    .total_cmp(right_distance)
                    .then_with(|| left_guid.raw().cmp(&right_guid.raw()))
            },
        );
        targets
            .into_iter()
            .map(|(_, _, creature)| creature)
            .collect()
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
    }

    pub(in crate::world) fn db_creature_chase_slot_destination(
        &self,
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
        Some(db_creature_chase_near_point(
            target_position,
            stop_distance,
            fan_angle,
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
