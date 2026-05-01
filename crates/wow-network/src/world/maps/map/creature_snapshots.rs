// Shared DB-creature snapshot and lazy grid visibility helpers.

impl MapRuntime {
    #[allow(dead_code)]
    fn share_db_creature_snapshots(
        &mut self,
        creatures: Vec<DbCreatureRuntime>,
    ) -> Vec<DbCreatureRuntime> {
        creatures
            .into_iter()
            .map(|creature| {
                let guid = creature.guid().raw();
                let shared = self.creatures.entry(guid).or_insert_with(|| {
                    let cell = cell_coord_for_position(creature.current_position);
                    let grid = grid_coord_for_position(creature.current_position);
                    self.grids
                        .entry(grid)
                        .or_default()
                        .cells
                        .entry(cell)
                        .or_default()
                        .creatures
                        .insert(creature.guid().raw());
                    creature
                });
                shared.clone()
            })
            .collect()
    }

    fn unloaded_creature_grids_for_area(
        &self,
        position: WorldPosition,
        radius: f32,
    ) -> Vec<GridCoord> {
        let mut grids = calculate_cell_area(position, radius)
            .into_iter()
            .map(|(grid, _)| grid)
            .collect::<HashSet<_>>()
            .into_iter()
            .filter(|grid| !self.loaded_creature_grids.contains(grid))
            .collect::<Vec<_>>();
        grids.sort_by_key(|grid| (grid.x, grid.y));
        grids
    }

    fn insert_loaded_creature_grid(
        &mut self,
        grid_coord: GridCoord,
        creatures: Vec<DbCreatureRuntime>,
    ) -> Vec<DbCreatureRuntime> {
        self.loaded_creature_grids.insert(grid_coord);
        self.grids.entry(grid_coord).or_default().last_touched = Instant::now();
        let loaded = creatures
            .into_iter()
            .map(|creature| {
                let guid = creature.guid().raw();
                let shared = self.creatures.entry(guid).or_insert_with(|| creature);
                let cell = cell_coord_for_position(shared.current_position);
                let grid = grid_coord_for_position(shared.current_position);
                self.grids
                    .entry(grid)
                    .or_default()
                    .cells
                    .entry(cell)
                    .or_default()
                    .creatures
                    .insert(shared.guid().raw());
                shared.clone()
            })
            .collect();
        self.refresh_grid_state(grid_coord);
        loaded
    }

    fn nearby_db_creature_snapshots(
        &self,
        position: WorldPosition,
        radius: f32,
        limit: u32,
    ) -> Vec<DbCreatureRuntime> {
        let mut guids = HashSet::new();
        self.visit_nearby_cells(position, radius, |cell| {
            guids.extend(cell.creatures.iter().copied());
        });
        let mut creatures = guids
            .into_iter()
            .filter_map(|guid| {
                self.creatures
                    .get(&guid)
                    .filter(|creature| {
                        is_position_inside_radius(creature.current_position, position, radius)
                    })
                    .cloned()
            })
            .collect::<Vec<_>>();
        creatures.sort_by(|left, right| {
            distance_squared_2d(
                left.current_position.x,
                left.current_position.y,
                position.x,
                position.y,
            )
            .partial_cmp(&distance_squared_2d(
                right.current_position.x,
                right.current_position.y,
                position.x,
                position.y,
            ))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.guid().raw().cmp(&right.guid().raw()))
        });
        creatures.truncate(limit as usize);
        creatures
    }

    fn db_creature_snapshots(&self, creature_guids: &[u64]) -> Vec<DbCreatureRuntime> {
        creature_guids
            .iter()
            .filter_map(|guid| self.creatures.get(guid).cloned())
            .collect()
    }

    #[allow(dead_code)]
    fn update_db_creature_snapshot(&mut self, creature: DbCreatureRuntime) {
        let guid = creature.guid().raw();
        let new_grid = grid_coord_for_position(creature.current_position);
        if let Some(previous_position) = self.creatures.get(&guid).map(|creature| creature.current_position) {
            self.refresh_db_creature_spatial_index(
                guid,
                previous_position,
                creature.current_position,
            );
        } else {
            let new_cell = cell_coord_for_position(creature.current_position);
            self.grids
                .entry(new_grid)
                .or_default()
                .cells
                .entry(new_cell)
                .or_default()
                .creatures
                .insert(guid);
        }
        self.grids
            .entry(new_grid)
            .or_default()
            .last_touched = Instant::now();
        self.creatures.insert(guid, creature);
        self.refresh_grid_state(new_grid);
    }

    fn update_db_creature_snapshot_and_broadcast(
        &mut self,
        creature: DbCreatureRuntime,
        exclude_character_guid: Option<u32>,
        packet: OutboundWorldPacket,
    ) -> Vec<(SessionId, OutboundWorldPacket)> {
        let position = creature.current_position;
        self.update_db_creature_snapshot(creature);
        self.nearby_player_guids(
            position,
            CREATURE_SPAWN_RADIUS_YARDS,
            exclude_character_guid,
        )
        .into_iter()
        .filter_map(|player_guid| {
            self.players
                .get(&player_guid)
                .map(|player| (player.session_id, packet.clone()))
        })
        .collect()
    }

    fn refresh_grid_state(&mut self, grid_coord: GridCoord) {
        let Some(grid) = self.grids.get(&grid_coord) else {
            return;
        };
        let state = if grid.active_player_count > 0
            || grid.cells.values().any(|cell| !cell.players.is_empty())
        {
            GridState::Active
        } else if let Some(blocker) = self.creature_grid_unload_blocker(grid_coord) {
            GridState::UnloadBlocked(blocker)
        } else if self.loaded_creature_grids.contains(&grid_coord) {
            GridState::Idle
        } else {
            GridState::Loaded
        };
        if let Some(grid) = self.grids.get_mut(&grid_coord) {
            grid.state = state;
        }
    }

    fn creature_grid_unload_blocker(&self, grid_coord: GridCoord) -> Option<GridUnloadBlocker> {
        let grid = self.grids.get(&grid_coord)?;
        let creature_guids = grid
            .cells
            .values()
            .flat_map(|cell| cell.creatures.iter().copied())
            .collect::<HashSet<_>>();
        if creature_guids
            .iter()
            .any(|guid| self.active_creature_combats.contains_key(guid))
        {
            return Some(GridUnloadBlocker::Combat);
        }

        for guid in creature_guids {
            let Some(creature) = self.creatures.get(&guid) else {
                continue;
            };
            if creature.looting || creature.lootable || creature.loot_money_available || creature.loot_item.is_some() {
                return Some(GridUnloadBlocker::Loot);
            }
            if creature.life_state == DbCreatureLifeState::Corpse {
                return Some(GridUnloadBlocker::Corpse);
            }
            if creature.corpse_expires_at.is_some()
                || creature.respawn_at.is_some()
                || creature.next_random_move_at.is_some()
                || creature.next_waypoint_move_at.is_some()
                || !matches!(creature.motion, CreatureMotionState::Idle)
            {
                return Some(GridUnloadBlocker::Timer);
            }
        }

        None
    }
}
