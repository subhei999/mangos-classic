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
        let retained_non_player_visible = current_player
            .visible_objects
            .iter()
            .filter(|guid| !guid.is_player())
            .copied()
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
            if old_grid != new_grid {
                self.refresh_grid_state(old_grid);
            }
            self.refresh_grid_state(new_grid);
        }

        if let Some(player) = self.players.get_mut(&character_guid) {
            player.position = movement.position;
            player.movement_flags = movement.flags;
            player.client_time = movement.client_time;
            player.fall_time = movement.fall_time;
            player.cell = new_cell;
            player.visible_objects = retained_non_player_visible;
            player.visible_objects.extend(new_visible
                .iter()
                .map(|guid| ObjectGuid::new(HighGuid::Player, 0, *guid))
            );
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

    fn sync_player_gameplay_state(&mut self, character_guid: u32, session: &WorldSessionState) {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return;
        };
        player.health = session.player_health.min(player.max_health);
        player.power1 = session.player_mana.min(player.max_power1);
        player.power2 = session.player_rage.min(POWER_RAGE_DEFAULT);
        player.active_spells = session.active_spells.clone();
        player.inventory = session.inventory.clone();
        player.quest_statuses = session.quest_statuses.clone();
        if let Some(character) = session.active_character.as_ref() {
            player.position = character.position;
            player.movement_flags = character.movement_flags;
            player.client_time = character.client_time;
            player.fall_time = character.fall_time;
            let equipment_cache = session
                .player_visual
                .as_ref()
                .and_then(|visual| visual.equipment_cache.as_deref());
            player.visible_equipment = visible_equipment_for_inventory(
                equipment_cache,
                &session.inventory,
            );
        }
    }

    fn player_runtime_snapshot(&self, character_guid: u32) -> Option<PlayerRuntimeSnapshot> {
        let player = self.players.get(&character_guid)?;
        Some(PlayerRuntimeSnapshot {
            position: player.position,
            health: player.health,
            power1: player.power1,
            power2: player.power2,
            active_spells: player.active_spells.clone(),
            inventory: player.inventory.clone(),
            quest_statuses: player.quest_statuses.clone(),
            combat_stats: player.combat_stats,
            active_combat_target: player.active_combat_target,
            active_combat_next_swing_at: player.active_combat_next_swing_at,
        })
    }

    fn player_visible_db_creature_guids(&self, character_guid: u32) -> Vec<u64> {
        let Some(player) = self.players.get(&character_guid) else {
            return Vec::new();
        };
        let mut guids = player
            .visible_objects
            .iter()
            .filter(|guid| guid.is_creature())
            .map(|guid| guid.raw())
            .collect::<Vec<_>>();
        guids.sort_unstable();
        guids
    }

    fn should_rescan_player_creature_visibility(
        &mut self,
        character_guid: u32,
        position: WorldPosition,
    ) -> bool {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return true;
        };
        if should_rescan_visibility_from(player.last_creature_visibility_position, position) {
            player.last_creature_visibility_position = Some(position);
            return true;
        }
        false
    }

    fn should_rescan_player_gameobject_visibility(
        &mut self,
        character_guid: u32,
        position: WorldPosition,
    ) -> bool {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return true;
        };
        if should_rescan_visibility_from(player.last_gameobject_visibility_position, position) {
            player.last_gameobject_visibility_position = Some(position);
            return true;
        }
        false
    }

    fn should_rescan_player_corpse_visibility(
        &mut self,
        character_guid: u32,
        position: WorldPosition,
    ) -> bool {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return true;
        };
        if should_rescan_visibility_from(player.last_player_corpse_visibility_position, position) {
            player.last_player_corpse_visibility_position = Some(position);
            return true;
        }
        false
    }

    fn reset_player_visibility_scan_positions(&mut self, character_guid: u32) {
        if let Some(player) = self.players.get_mut(&character_guid) {
            player.last_creature_visibility_position = None;
            player.last_gameobject_visibility_position = None;
            player.last_player_corpse_visibility_position = None;
        }
    }

    fn update_player_combat_stats(
        &mut self,
        character_guid: u32,
        combat_stats: PlayerCombatStats,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let player = self
            .players
            .get_mut(&character_guid)
            .ok_or_else(|| anyhow::anyhow!("player {character_guid} is not in map runtime"))?;
        player.combat_stats = combat_stats;
        let position = player.position;
        let packet = OutboundWorldPacket {
            opcode: SMSG_UPDATE_OBJECT,
            body: build_player_combat_stats_update_body(character_guid, &combat_stats)?,
        };
        Ok(self
            .nearby_player_guids(
                position,
                PLAYER_VISIBILITY_RADIUS_YARDS,
                Some(character_guid),
            )
            .into_iter()
            .filter_map(|observer_guid| {
                self.players
                    .get(&observer_guid)
                    .map(|observer| (observer.session_id, packet.clone()))
            })
            .collect())
    }

    fn player_combat_stats(&self, character_guid: u32) -> Option<PlayerCombatStats> {
        self.players
            .get(&character_guid)
            .map(|player| player.combat_stats)
    }

    fn set_player_auto_attack(
        &mut self,
        character_guid: u32,
        target: Option<ObjectGuid>,
        next_swing_at: Option<Instant>,
    ) {
        if let Some(player) = self.players.get_mut(&character_guid) {
            player.active_combat_target = target;
            player.active_combat_next_swing_at = next_swing_at;
        }
    }

    fn player_auto_attack_due(&self, character_guid: u32, now: Instant) -> Option<ObjectGuid> {
        let player = self.players.get(&character_guid)?;
        let target = player.active_combat_target?;
        player
            .active_combat_next_swing_at
            .is_none_or(|next_swing_at| now >= next_swing_at)
            .then_some(target)
    }

    fn player_auto_attack_target(&self, character_guid: u32) -> Option<ObjectGuid> {
        self.players
            .get(&character_guid)
            .and_then(|player| player.active_combat_target)
    }

    fn set_player_next_swing_at(&mut self, character_guid: u32, next_swing_at: Option<Instant>) {
        if let Some(player) = self.players.get_mut(&character_guid) {
            player.active_combat_next_swing_at = next_swing_at;
        }
    }

    fn set_player_power2(&mut self, character_guid: u32, power2: u32) {
        if let Some(player) = self.players.get_mut(&character_guid) {
            player.power2 = power2.min(POWER_RAGE_DEFAULT);
        }
    }

    fn update_player_selection(
        &mut self,
        character_guid: u32,
        selected_target: Option<ObjectGuid>,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return Ok(Vec::new());
        };
        player.selected_target = selected_target;
        let position = player.position;
        let packet = OutboundWorldPacket {
            opcode: SMSG_UPDATE_OBJECT,
            body: build_player_selection_update_body(character_guid, selected_target)?,
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
        let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);

        let player_grid = grid_coord_for_position(player.position);
        if let Some(grid) = self.grids.get_mut(&player_grid) {
            grid.active_player_count = grid.active_player_count.saturating_sub(1);
            grid.last_touched = Instant::now();
            if let Some(cell) = grid.cells.get_mut(&player.cell) {
                cell.players.remove(&character_guid);
            }
        }

        self.clear_db_creature_combats_for_victim(player_guid);
        self.refresh_grid_state(player_grid);
        let destroy = OutboundWorldPacket {
            opcode: SMSG_DESTROY_OBJECT,
            body: build_destroy_guid_body(player_guid),
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

    #[cfg(test)]
    fn update_player_db_creature_visibility(
        &mut self,
        character_guid: u32,
        create_guids: &[ObjectGuid],
        destroy_guids: &[ObjectGuid],
    ) {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return;
        };
        for guid in create_guids {
            player.visible_objects.insert(*guid);
        }
        for guid in destroy_guids {
            player.visible_objects.remove(guid);
        }
    }
}

fn should_rescan_visibility_from(previous: Option<WorldPosition>, position: WorldPosition) -> bool {
    let Some(previous) = previous else {
        return true;
    };
    if previous.map_id != position.map_id {
        return true;
    }
    distance_squared_2d(previous.x, previous.y, position.x, position.y)
        >= CREATURE_VISIBILITY_RESCAN_DISTANCE_YARDS * CREATURE_VISIBILITY_RESCAN_DISTANCE_YARDS
}
