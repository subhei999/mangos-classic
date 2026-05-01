// Shared DB-creature motion and AI-transition authority.

impl MapRuntime {
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
        creature.loot_money_available = false;
        creature.loot_item = None;
        self.clear_db_creature_combat(creature_guid);
        self.creatures.get(&creature_guid.raw()).cloned()
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
