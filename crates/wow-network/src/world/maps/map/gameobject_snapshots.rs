// Shared DB-gameobject snapshot and lazy grid visibility helpers.

impl MapRuntime {
    fn unloaded_gameobject_grids_for_area(
        &self,
        position: WorldPosition,
        radius: f32,
    ) -> Vec<GridCoord> {
        let mut grids = calculate_cell_area(position, radius)
            .into_iter()
            .map(|(grid, _)| grid)
            .collect::<HashSet<_>>()
            .into_iter()
            .filter(|grid| !self.loaded_gameobject_grids.contains(grid))
            .collect::<Vec<_>>();
        grids.sort_by_key(|grid| (grid.x, grid.y));
        grids
    }

    fn insert_loaded_gameobject_grid(
        &mut self,
        grid_coord: GridCoord,
        gameobjects: Vec<DbGameObjectRuntime>,
    ) -> Vec<DbGameObjectRuntime> {
        self.loaded_gameobject_grids.insert(grid_coord);
        self.grids.entry(grid_coord).or_default().last_touched = Instant::now();
        gameobjects
            .into_iter()
            .map(|gameobject| {
                let guid = gameobject.guid().raw();
                let shared = self.gameobjects.entry(guid).or_insert_with(|| gameobject);
                let cell = cell_coord_for_position(shared.position());
                let grid = grid_coord_for_position(shared.position());
                self.grids
                    .entry(grid)
                    .or_default()
                    .cells
                    .entry(cell)
                    .or_default()
                    .gameobjects
                    .insert(shared.guid().raw());
                shared.clone()
            })
            .collect()
    }

    fn nearby_db_gameobject_snapshots(
        &self,
        position: WorldPosition,
        radius: f32,
        limit: u32,
    ) -> Vec<DbGameObjectRuntime> {
        let mut guids = HashSet::new();
        self.visit_nearby_cells(position, radius, |cell| {
            guids.extend(cell.gameobjects.iter().copied());
        });
        let mut gameobjects = guids
            .into_iter()
            .filter_map(|guid| {
                self.gameobjects
                    .get(&guid)
                    .filter(|gameobject| {
                        is_position_inside_radius(gameobject.position(), position, radius)
                    })
                    .cloned()
            })
            .collect::<Vec<_>>();
        gameobjects.sort_by(|left, right| {
            distance_squared_2d(
                left.position().x,
                left.position().y,
                position.x,
                position.y,
            )
            .partial_cmp(&distance_squared_2d(
                right.position().x,
                right.position().y,
                position.x,
                position.y,
            ))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.guid().raw().cmp(&right.guid().raw()))
        });
        gameobjects.truncate(limit as usize);
        gameobjects
    }

    fn db_gameobject_snapshot(&self, gameobject_guid: ObjectGuid) -> Option<DbGameObjectRuntime> {
        self.gameobjects.get(&gameobject_guid.raw()).cloned()
    }

    fn consume_db_gameobject(
        &mut self,
        gameobject_guid: ObjectGuid,
        now: Instant,
        exclude_character_guid: Option<u32>,
    ) -> Option<(DbGameObjectRuntime, Vec<(SessionId, OutboundWorldPacket)>)> {
        let gameobject = self.gameobjects.get_mut(&gameobject_guid.raw())?;
        gameobject.mark_consumed(now);
        let snapshot = gameobject.clone();
        let destroy_packet = OutboundWorldPacket {
            opcode: SMSG_DESTROY_OBJECT,
            body: gameobject_guid.raw().to_le_bytes().to_vec(),
        };
        let packets = self
            .nearby_player_guids(snapshot.position(), CREATURE_SPAWN_RADIUS_YARDS, exclude_character_guid)
            .into_iter()
            .filter_map(|player_guid| {
                self.players
                    .get(&player_guid)
                    .map(|player| (player.session_id, destroy_packet.clone()))
            })
            .collect();
        Some((snapshot, packets))
    }
}
