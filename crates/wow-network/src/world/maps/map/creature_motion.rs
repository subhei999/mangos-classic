// Shared DB-creature motion and AI-transition authority.

#[derive(Debug)]
struct DbCreatureIdleMotionTick {
    creatures: Vec<DbCreatureRuntime>,
    packets: Vec<(SessionId, OutboundWorldPacket)>,
}

impl MapRuntime {
    fn advance_active_db_creature_idle_motions(
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

    fn advance_active_db_creature_idle_motions_with_interval(
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
            if let Some(creature) =
                self.advance_db_creature_motion(ObjectGuid::from_raw(guid), now)
            {
                creatures.push(creature);
            }
        }

        let mut packets = Vec::new();
        if self
            .next_idle_motion_start_check_at
            .is_none_or(|next| now >= next)
        {
            for guid in self.db_creature_idle_motion_start_guids(now) {
                let creature_guid = ObjectGuid::from_raw(guid);
                let Some((creature, motion)) =
                    self.start_db_creature_idle_motion(navigation, creature_guid, now)
                else {
                    continue;
                };
                let move_packet = OutboundWorldPacket {
                    opcode: SMSG_MONSTER_MOVE,
                    body: build_monster_move_walk_path_body(
                        creature_guid,
                        motion.start,
                        &motion.path,
                        motion.spline_id,
                        motion.duration.as_millis().max(1) as u32,
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
                            packets.push((player.session_id, create_packet.clone()));
                        }
                        packets.push((player.session_id, move_packet.clone()));
                    }
                }
                creatures.push(creature);
            }
            self.next_idle_motion_start_check_at = self.next_db_creature_idle_motion_start_at();
        }

        Ok(DbCreatureIdleMotionTick { creatures, packets })
    }

    fn invalidate_idle_motion_start_schedule(&mut self) {
        self.next_idle_motion_start_check_at = None;
    }

    fn db_creatures_in_active_or_blocked_grids(&self) -> Vec<(u64, &DbCreatureRuntime)> {
        self.creatures
            .iter()
            .filter(|(_, creature)| {
                let grid = grid_coord_for_position(creature.current_position);
                self.grids.get(&grid).is_some_and(|grid| {
                    matches!(
                        grid.state,
                        GridState::Active | GridState::UnloadBlocked(_)
                    )
                })
            })
            .map(|(guid, creature)| (*guid, creature))
            .collect()
    }

    fn db_creatures_in_player_interest_radius(&self) -> Vec<(u64, &DbCreatureRuntime)> {
        if self.players.is_empty() {
            return Vec::new();
        }
        let mut guids = HashSet::new();
        for player in self.players.values() {
            self.visit_nearby_cells(player.position, CREATURE_SPAWN_RADIUS_YARDS, |cell| {
                guids.extend(cell.creatures.iter().copied());
            });
        }
        guids.into_iter()
            .filter_map(|guid| {
                let creature = self.creatures.get(&guid)?;
                self.players
                    .values()
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

    fn db_creature_idle_motion_advancement_guids(&self) -> Vec<u64> {
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

    fn db_creature_idle_motion_start_guids(&self, now: Instant) -> Vec<u64> {
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

    fn next_db_creature_idle_motion_start_at(&self) -> Option<Instant> {
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

    fn advance_db_creature_motion(
        &mut self,
        creature_guid: ObjectGuid,
        now: Instant,
    ) -> Option<DbCreatureRuntime> {
        let old_position = self.creatures.get(&creature_guid.raw())?.current_position;
        let creature = self.creatures.get_mut(&creature_guid.raw())?;
        advance_db_creature_motion_runtime(creature, now);
        let snapshot = creature.clone();
        self.refresh_db_creature_spatial_index(
            creature_guid.raw(),
            old_position,
            snapshot.current_position,
        );
        Some(snapshot)
    }

    fn start_db_creature_idle_motion(
        &mut self,
        navigation: &DbCreatureNavigationGuardrail,
        creature_guid: ObjectGuid,
        now: Instant,
    ) -> Option<(DbCreatureRuntime, StartedCreatureMotion)> {
        if self
            .active_creature_combats
            .contains_key(&creature_guid.raw())
        {
            return None;
        }
        let old_position = self.creatures.get(&creature_guid.raw())?.current_position;
        let creature = self.creatures.get_mut(&creature_guid.raw())?;
        if !creature.is_alive() {
            return None;
        }
        let motion = start_db_creature_random_motion_runtime(navigation, creature, now)
            .or_else(|| start_db_creature_waypoint_motion_runtime(navigation, creature, now))?;
        let snapshot = creature.clone();
        self.refresh_db_creature_spatial_index(
            creature_guid.raw(),
            old_position,
            snapshot.current_position,
        );
        Some((snapshot, motion))
    }

    fn start_db_creature_chase_motion(
        &mut self,
        navigation: &DbCreatureNavigationGuardrail,
        creature_guid: ObjectGuid,
        target: ObjectGuid,
        target_position: WorldPosition,
        now: Instant,
    ) -> Option<(DbCreatureRuntime, StartedCreatureMotion)> {
        let old_position = self.creatures.get(&creature_guid.raw())?.current_position;
        let creature = self.creatures.get_mut(&creature_guid.raw())?;
        if !creature.is_alive() {
            return None;
        }
        let motion =
            start_db_creature_chase_motion_runtime(navigation, creature, target, target_position, now)?;
        let snapshot = creature.clone();
        self.refresh_db_creature_spatial_index(
            creature_guid.raw(),
            old_position,
            snapshot.current_position,
        );
        Some((snapshot, motion))
    }

    fn start_db_creature_return_home_motion(
        &mut self,
        navigation: &DbCreatureNavigationGuardrail,
        creature_guid: ObjectGuid,
        now: Instant,
    ) -> Option<(DbCreatureRuntime, StartedCreatureMotion)> {
        let old_position = self.creatures.get(&creature_guid.raw())?.current_position;
        let creature = self.creatures.get_mut(&creature_guid.raw())?;
        let motion = start_db_creature_return_home_motion_runtime(navigation, creature, now)?;
        let snapshot = creature.clone();
        self.refresh_db_creature_spatial_index(
            creature_guid.raw(),
            old_position,
            snapshot.current_position,
        );
        Some((snapshot, motion))
    }

    fn stop_db_creature_motion(
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

    fn face_db_creature_toward_position(
        &mut self,
        creature_guid: ObjectGuid,
        target_position: WorldPosition,
    ) -> Option<(DbCreatureRuntime, WorldPosition, u32)> {
        let creature = self.creatures.get_mut(&creature_guid.raw())?;
        let dx = target_position.x - creature.current_position.x;
        let dy = target_position.y - creature.current_position.y;
        creature.current_position.orientation = normalize_orientation(dy.atan2(dx));
        let spline_id = creature.next_spline_id;
        creature.next_spline_id = creature.next_spline_id.wrapping_add(1);
        Some((creature.clone(), creature.current_position, spline_id))
    }

    fn prepare_db_creature_evade(
        &mut self,
        creature_guid: ObjectGuid,
    ) -> Option<DbCreatureRuntime> {
        let creature = self.creatures.get_mut(&creature_guid.raw())?;
        creature.health = creature.max_health();
        creature.life_state = DbCreatureLifeState::Alive;
        creature.corpse_expires_at = None;
        creature.respawn_at = None;
        creature.respawn_epoch_secs = None;
        creature.lootable = false;
        creature.looting = false;
        creature.loot_money = 0;
        creature.loot_money_available = false;
        creature.loot_items.clear();
        self.clear_db_creature_combat(creature_guid);
        self.creatures.get(&creature_guid.raw()).cloned()
    }

    fn select_db_creature_sight_aggro_targets(
        &self,
        character: &ActiveCharacter,
    ) -> Vec<DbCreatureRuntime> {
        if character.position.map_id != self.map_id {
            return Vec::new();
        }
        let mut guids = HashSet::new();
        self.visit_nearby_cells(character.position, CREATURE_SPAWN_RADIUS_YARDS, |cell| {
            guids.extend(cell.creatures.iter().copied());
        });
        let mut targets = guids
            .into_iter()
            .filter_map(|guid| self.creatures.get(&guid))
            .filter(|creature| !self.active_creature_combats.contains_key(&creature.guid().raw()))
            .filter(|creature| creature.can_aggro_player(character))
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
        targets.sort_by(|(left_distance, left_guid, _), (right_distance, right_guid, _)| {
            left_distance
                .total_cmp(right_distance)
                .then_with(|| left_guid.raw().cmp(&right_guid.raw()))
        });
        targets
            .into_iter()
            .map(|(_, _, creature)| creature)
            .collect()
    }

    fn select_db_creature_assist_targets(
        &mut self,
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
            .filter(|creature| creature.can_aggro_player(character))
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

    fn refresh_db_creature_spatial_index(
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
}
