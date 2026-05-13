use super::*;

// Shared player-corpse snapshot and lazy grid visibility helpers.

impl MapRuntime {
    pub(in crate::world) fn unloaded_player_corpse_grids_for_area(
        &self,
        position: WorldPosition,
        radius: f32,
    ) -> Vec<GridCoord> {
        let mut grids = calculate_cell_area(position, radius)
            .into_iter()
            .map(|(grid, _)| grid)
            .collect::<HashSet<_>>()
            .into_iter()
            .filter(|grid| !self.loaded_player_corpse_grids.contains(grid))
            .collect::<Vec<_>>();
        grids.sort_by_key(|grid| (grid.x, grid.y));
        grids
    }

    pub(in crate::world) fn insert_loaded_player_corpse_grid(
        &mut self,
        grid_coord: GridCoord,
        corpses: Vec<PlayerCorpseRuntime>,
    ) -> Vec<PlayerCorpseRuntime> {
        self.loaded_player_corpse_grids.insert(grid_coord);
        self.grids.entry(grid_coord).or_default().last_touched = Instant::now();
        let loaded = corpses
            .into_iter()
            .map(|corpse| {
                let guid = corpse.guid.raw();
                self.upsert_player_corpse(corpse);
                self.corpses
                    .get(&guid)
                    .cloned()
                    .unwrap_or_else(|| unreachable!("inserted player corpse must be present"))
            })
            .collect();
        self.refresh_grid_state(grid_coord);
        loaded
    }

    pub(in crate::world) fn upsert_player_corpse(&mut self, corpse: PlayerCorpseRuntime) {
        let guid = corpse.guid.raw();
        let new_grid = grid_coord_for_position(corpse.position);
        if let Some(previous_position) = self.corpses.get(&guid).map(|corpse| corpse.position) {
            self.refresh_player_corpse_spatial_index(guid, previous_position, corpse.position);
        } else {
            let new_cell = cell_coord_for_position(corpse.position);
            self.grids
                .entry(new_grid)
                .or_default()
                .cells
                .entry(new_cell)
                .or_default()
                .corpses
                .insert(guid);
        }
        self.grids.entry(new_grid).or_default().last_touched = Instant::now();
        self.corpses.insert(guid, corpse);
        self.refresh_grid_state(new_grid);
    }

    pub(in crate::world) fn nearby_player_corpse_snapshots(
        &self,
        position: WorldPosition,
        radius: f32,
        limit: u32,
    ) -> Vec<PlayerCorpseRuntime> {
        let mut guids = HashSet::new();
        self.visit_nearby_cells(position, radius, |cell| {
            guids.extend(cell.corpses.iter().copied());
        });
        let mut corpses = guids
            .into_iter()
            .filter_map(|guid| {
                self.corpses
                    .get(&guid)
                    .filter(|corpse| is_position_inside_radius(corpse.position, position, radius))
                    .cloned()
            })
            .collect::<Vec<_>>();
        corpses.sort_by(|left, right| {
            distance_squared_2d(left.position.x, left.position.y, position.x, position.y)
                .partial_cmp(&distance_squared_2d(
                    right.position.x,
                    right.position.y,
                    position.x,
                    position.y,
                ))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.guid.raw().cmp(&right.guid.raw()))
        });
        corpses.truncate(limit as usize);
        corpses
    }

    pub(in crate::world) fn stage_player_corpse_visibility(
        &mut self,
        character_guid: u32,
        position: WorldPosition,
        nearby_corpses: Vec<PlayerCorpseRuntime>,
    ) -> MapPlayerCorpseVisibilityStage {
        let Some(player) = self.players.get(&character_guid) else {
            return MapPlayerCorpseVisibilityStage {
                nearby_corpses,
                ..Default::default()
            };
        };
        let previously_visible = player
            .visible_objects
            .iter()
            .filter(|guid| guid.is_corpse())
            .map(|guid| guid.raw())
            .collect::<HashSet<_>>();
        let nearby_guids = nearby_corpses
            .iter()
            .map(|corpse| corpse.guid.raw())
            .collect::<HashSet<_>>();

        let mut destroy_guids = previously_visible
            .iter()
            .copied()
            .filter(|guid| {
                !nearby_guids.contains(guid)
                    && !self.corpses.get(guid).is_some_and(|corpse| {
                        is_position_inside_radius(
                            corpse.position,
                            position,
                            CREATURE_VISIBILITY_UNLOAD_RADIUS_YARDS,
                        )
                    })
            })
            .map(ObjectGuid::from_raw)
            .collect::<Vec<_>>();
        destroy_guids.sort_by_key(|guid| guid.raw());

        let mut create_guids = nearby_corpses
            .iter()
            .filter(|corpse| !previously_visible.contains(&corpse.guid.raw()))
            .map(|corpse| corpse.guid)
            .collect::<Vec<_>>();
        create_guids.sort_by_key(|guid| guid.raw());

        if let Some(player) = self.players.get_mut(&character_guid) {
            for guid in &destroy_guids {
                player.visible_objects.remove(guid);
            }
            for guid in &create_guids {
                player.visible_objects.insert(*guid);
            }
        }

        MapPlayerCorpseVisibilityStage {
            nearby_corpses,
            create_guids,
            destroy_guids,
        }
    }

    pub(in crate::world) fn refresh_player_corpse_spatial_index(
        &mut self,
        guid: u64,
        previous_position: WorldPosition,
        new_position: WorldPosition,
    ) {
        let previous_grid = grid_coord_for_position(previous_position);
        let previous_cell = cell_coord_for_position(previous_position);
        let new_grid = grid_coord_for_position(new_position);
        let new_cell = cell_coord_for_position(new_position);
        if previous_grid == new_grid && previous_cell == new_cell {
            return;
        }
        if let Some(cell) = self
            .grids
            .get_mut(&previous_grid)
            .and_then(|grid| grid.cells.get_mut(&previous_cell))
        {
            cell.corpses.remove(&guid);
        }
        self.grids
            .entry(new_grid)
            .or_default()
            .cells
            .entry(new_cell)
            .or_default()
            .corpses
            .insert(guid);
        self.refresh_grid_state(previous_grid);
    }
}
