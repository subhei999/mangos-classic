// CMaNGOS reference: src/game/Maps/Map.cpp player enter, movement, visibility, and nearby broadcast.

impl MapRuntime {
    fn add_player(
        &mut self,
        player: PlayerRuntime,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let mut player = player;
        let player_guid = player.guid;
        let player_grid = grid_coord_for_position(player.position);
        let player_cell = cell_coord_for_position(player.position);
        player.cell = player_cell;
        let player_object = ObjectGuid::new(HighGuid::Player, 0, player_guid);
        let new_player_packet = OutboundWorldPacket {
            opcode: SMSG_UPDATE_OBJECT,
            body: build_update_object_body(&[build_other_player_create_block(&player)?]),
        };
        let mut packets = Vec::new();
        let mut visible_others = Vec::new();

        for other_guid in self.nearby_player_guids(
            player.position,
            PLAYER_VISIBILITY_RADIUS_YARDS,
            Some(player_guid),
        ) {
            let Some(other) = self.players.get(&other_guid) else {
                continue;
            };

            visible_others.push(other.guid);
            packets.push((
                player.session_id,
                OutboundWorldPacket {
                    opcode: SMSG_UPDATE_OBJECT,
                    body: build_update_object_body(&[build_other_player_create_block(other)?]),
                },
            ));
            packets.push((other.session_id, new_player_packet.clone()));
        }
        for other_guid in &visible_others {
            player
                .visible_objects
                .insert(ObjectGuid::new(HighGuid::Player, 0, *other_guid));
            if let Some(other) = self.players.get_mut(other_guid) {
                other.visible_objects.insert(player_object);
            }
        }

        let grid = self.grids.entry(player_grid).or_default();
        grid.state = GridState::Active;
        grid.active_player_count = grid.active_player_count.saturating_add(1);
        grid.last_touched = Instant::now();
        grid.cells
            .entry(player_cell)
            .or_default()
            .players
            .insert(player_guid);
        self.players.insert(player_guid, player);

        Ok(packets)
    }

    fn update_player_position(
        &mut self,
        character_guid: u32,
        opcode: u16,
        movement: &MovementInfo,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let Some(current_player) = self.players.get(&character_guid).cloned() else {
            return Ok(Vec::new());
        };
        let player_object = ObjectGuid::new(HighGuid::Player, 0, character_guid);
        let old_cell = current_player.cell;
        let old_grid = grid_coord_for_position(current_player.position);
        let new_cell = cell_coord_for_position(movement.position);
        let new_grid = grid_coord_for_position(movement.position);

        let old_visible = current_player
            .visible_objects
            .iter()
            .filter_map(|guid| {
                if guid.is_player() {
                    Some(guid.counter())
                } else {
                    None
                }
            })
            .collect::<HashSet<_>>();
        let new_visible = self
            .nearby_player_guids(
                movement.position,
                PLAYER_VISIBILITY_RADIUS_YARDS,
                Some(character_guid),
            )
            .into_iter()
            .collect::<HashSet<_>>();

        let mut packets = Vec::new();
        for other_guid in old_visible
            .difference(&new_visible)
            .copied()
            .collect::<Vec<_>>()
        {
            let Some(other) = self.players.get_mut(&other_guid) else {
                continue;
            };
            other.visible_objects.remove(&player_object);
            packets.push((
                current_player.session_id,
                OutboundWorldPacket {
                    opcode: SMSG_DESTROY_OBJECT,
                    body: build_destroy_guid_body(ObjectGuid::new(HighGuid::Player, 0, other_guid)),
                },
            ));
            packets.push((
                other.session_id,
                OutboundWorldPacket {
                    opcode: SMSG_DESTROY_OBJECT,
                    body: build_destroy_guid_body(player_object),
                },
            ));
        }

        let entering = new_visible
            .difference(&old_visible)
            .copied()
            .collect::<Vec<_>>();
        let mut entering_for_mover = Vec::new();
        let moving_player_create = {
            let mut moved_player = current_player.clone();
            moved_player.position = movement.position;
            moved_player.cell = new_cell;
            build_other_player_create_block(&moved_player)?
        };
        for other_guid in entering {
            let Some(other) = self.players.get_mut(&other_guid) else {
                continue;
            };
            other.visible_objects.insert(player_object);
            entering_for_mover.push(other_guid);
            packets.push((
                other.session_id,
                OutboundWorldPacket {
                    opcode: SMSG_UPDATE_OBJECT,
                    body: build_update_object_body(std::slice::from_ref(&moving_player_create)),
                },
            ));
        }
        for other_guid in &entering_for_mover {
            if let Some(other) = self.players.get(other_guid) {
                packets.push((
                    current_player.session_id,
                    OutboundWorldPacket {
                        opcode: SMSG_UPDATE_OBJECT,
                        body: build_update_object_body(&[build_other_player_create_block(other)?]),
                    },
                ));
            }
        }

        let movement_packet = OutboundWorldPacket {
            opcode,
            body: build_player_movement_broadcast_body(character_guid, movement)?,
        };
        for other_guid in &new_visible {
            let Some(other) = self.players.get(other_guid) else {
                continue;
            };
            packets.push((other.session_id, movement_packet.clone()));
        }

        if old_grid != new_grid || old_cell != new_cell {
            if let Some(grid) = self.grids.get_mut(&old_grid) {
                if let Some(cell) = grid.cells.get_mut(&old_cell) {
                    cell.players.remove(&character_guid);
                }
                grid.last_touched = Instant::now();
            }
            let grid = self.grids.entry(new_grid).or_default();
            grid.state = GridState::Active;
            grid.last_touched = Instant::now();
            grid.cells
                .entry(new_cell)
                .or_default()
                .players
                .insert(character_guid);
        }

        if let Some(player) = self.players.get_mut(&character_guid) {
            player.position = movement.position;
            player.movement_flags = movement.flags;
            player.client_time = movement.client_time;
            player.fall_time = movement.fall_time;
            player.cell = new_cell;
            player.visible_objects = new_visible
                .iter()
                .map(|guid| ObjectGuid::new(HighGuid::Player, 0, *guid))
                .collect();
        }

        Ok(packets)
    }

    fn update_player_visible_equipment(
        &mut self,
        character_guid: u32,
        visible_equipment: [u32; ENUM_EQUIPMENT_SLOTS],
        changed_slots: &[u8],
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return Ok(Vec::new());
        };
        player.visible_equipment = visible_equipment;
        let position = player.position;
        let packet = OutboundWorldPacket {
            opcode: SMSG_UPDATE_OBJECT,
            body: build_update_object_body(&[build_player_visible_equipment_update_block(
                character_guid,
                &player.visible_equipment,
                changed_slots,
            )?]),
        };
        Ok(self
            .nearby_player_guids(
                position,
                PLAYER_VISIBILITY_RADIUS_YARDS,
                Some(character_guid),
            )
            .into_iter()
            .filter_map(|other_guid| {
                self.players
                    .get(&other_guid)
                    .map(|other| (other.session_id, packet.clone()))
            })
            .collect())
    }

    fn update_player_health(
        &mut self,
        character_guid: u32,
        health: u32,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return Ok(Vec::new());
        };
        player.health = health.min(player.max_health);
        let position = player.position;
        let packet = OutboundWorldPacket {
            opcode: SMSG_UPDATE_OBJECT,
            body: build_player_health_update_body(
                ObjectGuid::new(HighGuid::Player, 0, character_guid),
                player.health,
            )?,
        };

        Ok(self
            .nearby_player_guids(
                position,
                PLAYER_VISIBILITY_RADIUS_YARDS,
                Some(character_guid),
            )
            .into_iter()
            .filter_map(|other_guid| {
                self.players
                    .get(&other_guid)
                    .map(|other| (other.session_id, packet.clone()))
            })
            .collect())
    }

    fn remove_player(&mut self, character_guid: u32) -> Vec<(SessionId, OutboundWorldPacket)> {
        let Some(player) = self.players.remove(&character_guid) else {
            return Vec::new();
        };

        if let Some(grid) = self
            .grids
            .get_mut(&grid_coord_for_position(player.position))
        {
            grid.active_player_count = grid.active_player_count.saturating_sub(1);
            grid.last_touched = Instant::now();
            if let Some(cell) = grid.cells.get_mut(&player.cell) {
                cell.players.remove(&character_guid);
            }
        }

        let destroy = OutboundWorldPacket {
            opcode: SMSG_DESTROY_OBJECT,
            body: build_destroy_guid_body(ObjectGuid::new(HighGuid::Player, 0, character_guid)),
        };
        self.nearby_player_guids(
            player.position,
            PLAYER_VISIBILITY_RADIUS_YARDS,
            Some(character_guid),
        )
        .into_iter()
        .filter_map(|other_guid| {
            self.players
                .get(&other_guid)
                .map(|other| (other.session_id, destroy.clone()))
        })
        .collect()
    }

    fn broadcast_nearby_player_packet(
        &self,
        sender_guid: u32,
        radius: f32,
        packet: OutboundWorldPacket,
    ) -> Vec<(SessionId, OutboundWorldPacket)> {
        let Some(sender) = self.players.get(&sender_guid) else {
            return Vec::new();
        };
        self.nearby_player_guids(sender.position, radius, Some(sender_guid))
            .into_iter()
            .filter_map(|other_guid| {
                self.players
                    .get(&other_guid)
                    .map(|other| (other.session_id, packet.clone()))
            })
            .collect()
    }
}
