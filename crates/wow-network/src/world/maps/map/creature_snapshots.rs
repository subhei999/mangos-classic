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
        creatures
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
            .collect()
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
        let new_cell = cell_coord_for_position(creature.current_position);
        if let Some(previous) = self.creatures.get(&guid) {
            let previous_grid = grid_coord_for_position(previous.current_position);
            let previous_cell = cell_coord_for_position(previous.current_position);
            if previous_grid != new_grid || previous_cell != new_cell {
                if let Some(grid) = self.grids.get_mut(&previous_grid) {
                    if let Some(cell) = grid.cells.get_mut(&previous_cell) {
                        cell.creatures.remove(&creature.guid().raw());
                    }
                }
            }
        }
        self.grids
            .entry(new_grid)
            .or_default()
            .cells
            .entry(new_cell)
            .or_default()
            .creatures
            .insert(creature.guid().raw());
        self.creatures.insert(guid, creature);
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
}
