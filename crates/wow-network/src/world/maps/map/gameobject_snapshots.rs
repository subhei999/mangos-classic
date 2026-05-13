use super::*;

// Shared DB-gameobject snapshot and lazy grid visibility helpers.

impl MapRuntime {
    pub(in crate::world) fn unloaded_gameobject_grids_for_area(
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

    pub(in crate::world) fn loaded_gameobject_grids(&self) -> Vec<GridCoord> {
        let mut grids = self
            .loaded_gameobject_grids
            .iter()
            .copied()
            .collect::<Vec<_>>();
        grids.sort_by_key(|grid| (grid.x, grid.y));
        grids
    }

    pub(in crate::world) fn insert_loaded_gameobject_grid(
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

    pub(in crate::world) fn refresh_static_event_gameobject_grid(
        &mut self,
        grid_coord: GridCoord,
        active_gameobjects: Vec<DbGameObjectRuntime>,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let desired_event_guids = active_gameobjects
            .iter()
            .filter(|gameobject| gameobject.spawn.game_event.is_some())
            .map(|gameobject| gameobject.guid().raw())
            .collect::<HashSet<_>>();
        let mut packets = Vec::new();
        let mut remove_guids = self
            .gameobjects
            .iter()
            .filter_map(|(guid, gameobject)| {
                let is_grid_spawn =
                    grid_coord_for_position(gameobject_spawn_position(&gameobject.spawn))
                        == grid_coord;
                (is_grid_spawn
                    && gameobject.spawn.game_event.is_some()
                    && !desired_event_guids.contains(guid))
                .then_some(*guid)
            })
            .collect::<Vec<_>>();
        remove_guids.sort_unstable();
        for guid in remove_guids {
            if let Some(destroy_packets) =
                self.delete_db_gameobject_runtime(ObjectGuid::from_raw(guid), None)
            {
                packets.extend(destroy_packets);
            }
        }

        for gameobject in active_gameobjects
            .into_iter()
            .filter(|gameobject| gameobject.spawn.game_event.is_some())
        {
            let guid = gameobject.guid();
            if self.gameobjects.contains_key(&guid.raw()) {
                continue;
            }
            let position = gameobject.position();
            self.insert_loaded_gameobject_grid(
                grid_coord_for_position(position),
                vec![gameobject.clone()],
            );
            for player_guid in self.nearby_player_guids(position, CREATURE_SPAWN_RADIUS_YARDS, None)
            {
                let Some(player) = self.players.get_mut(&player_guid) else {
                    continue;
                };
                if gameobject.is_consumed(now) || !player.visible_objects.insert(guid) {
                    continue;
                }
                let create_block = build_db_gameobject_runtime_create_block_for_quest_statuses(
                    &gameobject,
                    &player.quest_statuses,
                )?;
                if let Some(packet) = player.packet_to_client(OutboundWorldPacket {
                    opcode: SMSG_UPDATE_OBJECT,
                    body: build_update_object_body(&[create_block]),
                }) {
                    packets.push(packet);
                }
            }
        }
        Ok(packets)
    }

    pub(in crate::world) fn delete_db_gameobject_runtime(
        &mut self,
        gameobject_guid: ObjectGuid,
        exclude_character_guid: Option<u32>,
    ) -> Option<Vec<(SessionId, OutboundWorldPacket)>> {
        let gameobject = self.gameobjects.remove(&gameobject_guid.raw())?;
        let position = gameobject.position();
        let grid_coord = grid_coord_for_position(position);
        let cell_coord = cell_coord_for_position(position);
        if let Some(grid) = self.grids.get_mut(&grid_coord) {
            if let Some(cell) = grid.cells.get_mut(&cell_coord) {
                cell.gameobjects.remove(&gameobject_guid.raw());
            }
            grid.last_touched = Instant::now();
        }
        self.clear_db_gameobject_loot(gameobject_guid.raw());
        self.gameobject_looting_by_character
            .retain(|_, looting_guid| *looting_guid != gameobject_guid.raw());
        for player in self.players.values_mut() {
            player.visible_objects.remove(&gameobject_guid);
            if player.selected_target == Some(gameobject_guid) {
                player.selected_target = None;
            }
            if player.unit_target == Some(gameobject_guid) {
                player.unit_target = None;
            }
        }
        self.refresh_grid_state(grid_coord);
        let packet = OutboundWorldPacket {
            opcode: SMSG_DESTROY_OBJECT,
            body: build_destroy_guid_body(gameobject_guid),
        };
        Some(
            self.nearby_player_guids(
                position,
                CREATURE_SPAWN_RADIUS_YARDS,
                exclude_character_guid,
            )
            .into_iter()
            .filter_map(|player_guid| {
                self.players
                    .get(&player_guid)
                    .and_then(|player| player.packet_to_client(packet.clone()))
            })
            .collect(),
        )
    }

    pub(in crate::world) fn nearby_db_gameobject_snapshots(
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
            distance_squared_2d(left.position().x, left.position().y, position.x, position.y)
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

    pub(in crate::world) fn db_gameobject_snapshot(
        &self,
        gameobject_guid: ObjectGuid,
    ) -> Option<DbGameObjectRuntime> {
        self.gameobjects.get(&gameobject_guid.raw()).cloned()
    }

    pub(in crate::world) fn db_gameobject_snapshots(
        &self,
        gameobject_guids: &[u64],
    ) -> Vec<DbGameObjectRuntime> {
        gameobject_guids
            .iter()
            .filter_map(|guid| self.gameobjects.get(guid).cloned())
            .collect()
    }

    pub(in crate::world) fn stage_player_db_gameobject_visibility(
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

    pub(in crate::world) fn consume_db_gameobject(
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
            .nearby_player_guids(
                snapshot.position(),
                CREATURE_SPAWN_RADIUS_YARDS,
                exclude_character_guid,
            )
            .into_iter()
            .filter_map(|player_guid| {
                self.players
                    .get(&player_guid)
                    .and_then(|player| player.packet_to_client(destroy_packet.clone()))
            })
            .collect();
        Some((snapshot, packets))
    }
}
