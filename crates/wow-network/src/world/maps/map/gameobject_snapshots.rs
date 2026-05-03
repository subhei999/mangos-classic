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
        let loaded = gameobjects
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
            .collect::<Vec<_>>();
        self.refresh_grid_state(grid_coord);
        loaded
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

    fn stage_player_db_gameobject_visibility(
        &mut self,
        character_guid: u32,
        position: WorldPosition,
        nearby_gameobjects: Vec<DbGameObjectRuntime>,
        now: Instant,
    ) -> MapDbGameObjectVisibilityStage {
        let Some(player) = self.players.get(&character_guid) else {
            return MapDbGameObjectVisibilityStage {
                nearby_gameobjects,
                ..Default::default()
            };
        };
        let previously_visible = player
            .visible_objects
            .iter()
            .filter(|guid| guid.is_game_object())
            .map(|guid| guid.raw())
            .collect::<HashSet<_>>();
        let nearby_by_guid = nearby_gameobjects
            .iter()
            .map(|gameobject| (gameobject.guid().raw(), gameobject))
            .collect::<HashMap<_, _>>();

        let mut destroy_guids = previously_visible
            .iter()
            .copied()
            .filter(|guid| {
                if let Some(gameobject) = nearby_by_guid.get(guid) {
                    return gameobject.is_consumed(now);
                }
                !self.gameobjects.get(guid).is_some_and(|gameobject| {
                    is_position_inside_radius(
                        gameobject.position(),
                        position,
                        CREATURE_VISIBILITY_UNLOAD_RADIUS_YARDS,
                    )
                })
            })
            .map(ObjectGuid::from_raw)
            .collect::<Vec<_>>();
        destroy_guids.sort_by_key(|guid| guid.raw());

        let mut create_guids = nearby_gameobjects
            .iter()
            .filter(|gameobject| {
                !gameobject.is_consumed(now)
                    && !previously_visible.contains(&gameobject.guid().raw())
            })
            .map(|gameobject| gameobject.guid())
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

        MapDbGameObjectVisibilityStage {
            nearby_gameobjects,
            create_guids,
            destroy_guids,
        }
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
        self.clear_db_gameobject_loot(gameobject_guid.raw());
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
