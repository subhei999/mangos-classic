// CMaNGOS reference: src/game/Maps/Map.cpp player enter, movement, visibility, and nearby broadcast.

impl MapRuntime {
    fn advance_player_regen_tick(
        &mut self,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        const PLAYER_REGEN_TICK: Duration = Duration::from_secs(2);
        let next_tick = self
            .next_player_regen_tick_at
            .get_or_insert(now + PLAYER_REGEN_TICK);
        if now < *next_tick {
            return Ok(Vec::new());
        }
        while *next_tick <= now {
            *next_tick += PLAYER_REGEN_TICK;
        }

        let in_combat_victims = self
            .active_creature_combats
            .values()
            .map(|combat| combat.victim)
            .collect::<HashSet<_>>();
        let mut update_packets = Vec::new();
        for player in self.players.values_mut() {
            let character_guid = player.guid;
            let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
            let in_combat = player.active_combat_target.is_some() || in_combat_victims.contains(&player_guid);
            let is_dead_or_ghost = player.health == 0 || (player.flags & PLAYER_FLAGS_GHOST) != 0;
            if is_dead_or_ghost {
                continue;
            }

            let mut health_changed = false;
            let mut mana_changed = false;
            let mut rage_changed = false;

            if !in_combat && player.health < player.max_health {
                let regen = health_regen_per_second_for_spirit(player.class, player.spirit).max(0.0);
                let gained = (regen * 2.0).floor() as u32;
                if gained > 0 {
                    let new_health = player.health.saturating_add(gained).min(player.max_health);
                    health_changed = new_health != player.health;
                    player.health = new_health;
                }
            }

            if player.max_power1 > 0 && player.power1 < player.max_power1 {
                let regen = mana_regen_per_second_for_spirit(player.class, player.spirit).max(0.0);
                let gained = (regen * 2.0).floor() as u32;
                if gained > 0 {
                    let new_mana = player.power1.saturating_add(gained).min(player.max_power1);
                    mana_changed = new_mana != player.power1;
                    player.power1 = new_mana;
                }
            }

            if !in_combat && player.class == 1 && player.power2 > 0 {
                // CMaNGOS: decay 2.5 rage every 2 seconds out of combat.
                let loss = 25u32;
                let new_rage = player.power2.saturating_sub(loss);
                rage_changed = new_rage != player.power2;
                player.power2 = new_rage;
            }

            if health_changed {
                update_packets.push((
                    character_guid,
                    player.position,
                    player.session_id,
                    OutboundWorldPacket {
                        opcode: SMSG_UPDATE_OBJECT,
                        body: build_player_health_update_body(player_guid, player.health)?,
                    },
                ));
            }
            if mana_changed {
                update_packets.push((
                    character_guid,
                    player.position,
                    player.session_id,
                    OutboundWorldPacket {
                        opcode: SMSG_UPDATE_OBJECT,
                        body: build_player_mana_update_body(player_guid, player.power1)?,
                    },
                ));
            }
            if rage_changed {
                update_packets.push((
                    character_guid,
                    player.position,
                    player.session_id,
                    OutboundWorldPacket {
                        opcode: SMSG_UPDATE_OBJECT,
                        body: build_player_rage_update_body(player_guid, player.power2)?,
                    },
                ));
            }
        }

        let mut packets = Vec::new();
        for (character_guid, position, direct_session_id, packet) in update_packets {
            packets.push((direct_session_id, packet.clone()));
            packets.extend(self.nearby_player_guids(
                position,
                PLAYER_VISIBILITY_RADIUS_YARDS,
                Some(character_guid),
            )
            .into_iter()
            .filter_map(|observer_guid| {
                self.players
                    .get(&observer_guid)
                    .map(|observer| (observer.session_id, packet.clone()))
            }));
        }

        Ok(packets)
    }

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
        self.invalidate_idle_motion_start_schedule();

        Ok(packets)
    }

    fn update_player_position(
        &mut self,
        character_guid: u32,
        opcode: u16,
        movement: &MovementInfo,
        server_time: u32,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let Some(current_player) = self.players.get(&character_guid).cloned() else {
            return Ok(Vec::new());
        };
        let player_object = ObjectGuid::new(HighGuid::Player, 0, character_guid);
        let old_cell = current_player.cell;
        let old_grid = grid_coord_for_position(current_player.position);
        let new_cell = cell_coord_for_position(movement.position);
        let new_grid = grid_coord_for_position(movement.position);
        let fall_update = player_fall_update(&current_player, opcode, movement);

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
            moved_player.movement_flags = movement.flags;
            moved_player.client_time = movement.client_time;
            moved_player.server_time = server_time;
            moved_player.fall_time = movement.fall_time;
            moved_player.jump = movement.jump.clone();
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
            body: build_player_movement_broadcast_body(character_guid, movement, server_time)?,
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
            self.invalidate_idle_motion_start_schedule();
        }

        if let Some(player) = self.players.get_mut(&character_guid) {
            player.position = movement.position;
            player.movement_flags = movement.flags;
            player.client_time = movement.client_time;
            player.server_time = server_time;
            player.fall_time = movement.fall_time;
            player.last_fall_z = fall_update.last_fall_z;
            player.last_fall_time = fall_update.last_fall_time;
            player.jump = movement.jump.clone();
            player.cell = new_cell;
            player.visible_objects = retained_non_player_visible;
            player.visible_objects.extend(new_visible
                .iter()
                .map(|guid| ObjectGuid::new(HighGuid::Player, 0, *guid))
            );
            if let Some(damage) = fall_update.damage {
                player.health = player.health.saturating_sub(damage);
            }
        }

        if let Some(damage) = fall_update.damage {
            let health = self
                .players
                .get(&character_guid)
                .map(|player| player.health)
                .unwrap_or(current_player.health.saturating_sub(damage));
            let damage_log = OutboundWorldPacket {
                opcode: SMSG_ENVIRONMENTALDAMAGELOG,
                body: build_environmental_damage_log_body(player_object, DAMAGE_FALL, damage, 0, 0)?,
            };
            let health_update = OutboundWorldPacket {
                opcode: SMSG_UPDATE_OBJECT,
                body: build_player_health_update_body(player_object, health)?,
            };
            packets.push((current_player.session_id, damage_log.clone()));
            packets.push((current_player.session_id, health_update.clone()));
            for other_guid in &new_visible {
                let Some(other) = self.players.get(other_guid) else {
                    continue;
                };
                packets.push((other.session_id, damage_log.clone()));
                packets.push((other.session_id, health_update.clone()));
            }
        }
        self.invalidate_idle_motion_start_schedule();

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
        player.max_health = player.max_health.max(session.player_health.max(1));
        player.max_power1 = player.max_power1.max(session.player_mana);
        player.health = session.player_health.min(player.max_health);
        player.power1 = session.player_mana.min(player.max_power1);
        player.power2 = session.player_rage.min(POWER_RAGE_DEFAULT);
        player.active_spells = session.active_spells.clone();
        player.inventory = session.inventory.clone();
        player.quest_statuses = session.quest_statuses.clone();
        player.active_auras = session.active_auras.clone();
        player.combat_stats =
            combat_stats_with_active_auras(player.base_combat_stats, &player.active_auras);
        if let Some(character) = session.active_character.as_ref() {
            player.level = character.level;
            player.xp = character.xp;
            player.flags = session.player_flags;
            player.position = character.position;
            player.movement_flags = character.movement_flags;
            player.client_time = character.client_time;
            player.fall_time = character.fall_time;
            player.jump = character.jump.clone();
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
            flags: player.flags,
            level: player.level,
            race: player.race,
            class: player.class,
            xp: player.xp,
            health: player.health,
            max_health: player.max_health,
            power1: player.power1,
            max_power1: player.max_power1,
            power2: player.power2,
            active_spells: player.active_spells.clone(),
            inventory: player.inventory.clone(),
            quest_statuses: player.quest_statuses.clone(),
            active_auras: player.active_auras.clone(),
            base_combat_stats: player.base_combat_stats,
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

    fn update_player_reward_state(
        &mut self,
        character_guid: u32,
        reward: PlayerRewardRuntimeUpdate,
    ) {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return;
        };
        player.level = reward.level;
        player.xp = reward.xp;
        player.max_health = reward.max_health.max(1);
        player.max_power1 = reward.max_power1;
        player.health = reward.health.min(player.max_health);
        player.power1 = reward.power1.min(player.max_power1);
        player.power2 = reward.power2.min(POWER_RAGE_DEFAULT);
        player.quest_statuses = reward.quest_statuses;
    }

    fn update_player_inventory(
        &mut self,
        character_guid: u32,
        inventory: Vec<CharacterInventoryItem>,
    ) {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return;
        };
        player.inventory = inventory;
    }

    fn player_visible_db_gameobject_guids(&self, character_guid: u32) -> Vec<u64> {
        let Some(player) = self.players.get(&character_guid) else {
            return Vec::new();
        };
        let mut guids = player
            .visible_objects
            .iter()
            .filter(|guid| guid.is_game_object())
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
        player.base_combat_stats = combat_stats;
        player.combat_stats =
            combat_stats_with_active_auras(player.base_combat_stats, &player.active_auras);
        let position = player.position;
        let packet = OutboundWorldPacket {
            opcode: SMSG_UPDATE_OBJECT,
            body: build_player_combat_stats_update_body(character_guid, &player.combat_stats)?,
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

    fn apply_player_aura(
        &mut self,
        character_guid: u32,
        aura: ActiveAura,
    ) -> anyhow::Result<Option<PlayerAuraUpdateEvent>> {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return Ok(None);
        };
        apply_active_aura(&mut player.active_auras, aura);
        player.combat_stats =
            combat_stats_with_active_auras(player.base_combat_stats, &player.active_auras);
        self.build_player_aura_update_event(character_guid, Instant::now())
            .map(Some)
    }

    fn advance_player_aura_expirations(
        &mut self,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let mut packets = Vec::new();
        let character_guids = self.players.keys().copied().collect::<Vec<_>>();
        for character_guid in character_guids {
            let Some(player) = self.players.get_mut(&character_guid) else {
                continue;
            };
            let before = player.active_auras.len();
            player
                .active_auras
                .retain(|aura| aura.expires_at.is_none_or(|expires_at| now < expires_at));
            if player.active_auras.len() == before {
                continue;
            }
            player.combat_stats =
                combat_stats_with_active_auras(player.base_combat_stats, &player.active_auras);
            let event = self.build_player_aura_update_event(character_guid, now)?;
            let Some(player) = self.players.get(&character_guid) else {
                continue;
            };
            packets.extend(
                event
                    .direct_packets
                    .into_iter()
                    .map(|packet| (player.session_id, packet)),
            );
            packets.extend(event.observer_packets);
        }
        Ok(packets)
    }

    fn build_player_aura_update_event(
        &self,
        character_guid: u32,
        now: Instant,
    ) -> anyhow::Result<PlayerAuraUpdateEvent> {
        let Some(player) = self.players.get(&character_guid) else {
            return Ok(PlayerAuraUpdateEvent {
                direct_packets: Vec::new(),
                observer_packets: Vec::new(),
            });
        };
        let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
        let aura_packet = OutboundWorldPacket {
            opcode: SMSG_UPDATE_OBJECT,
            body: build_player_aura_update_body(player_guid, &player.active_auras)?,
        };
        let combat_stats_packet = OutboundWorldPacket {
            opcode: SMSG_UPDATE_OBJECT,
            body: build_player_combat_stats_update_body(character_guid, &player.combat_stats)?,
        };
        let mut observer_packets = Vec::new();
        for observer_guid in self.nearby_player_guids(
            player.position,
            PLAYER_VISIBILITY_RADIUS_YARDS,
            Some(character_guid),
        ) {
            let Some(observer) = self.players.get(&observer_guid) else {
                continue;
            };
            observer_packets.push((observer.session_id, aura_packet.clone()));
            observer_packets.push((observer.session_id, combat_stats_packet.clone()));
        }

        let mut direct_packets = vec![aura_packet, combat_stats_packet];
        direct_packets.extend(build_player_aura_duration_update_packets(
            &player.active_auras,
            now,
        ));

        Ok(PlayerAuraUpdateEvent {
            direct_packets,
            observer_packets,
        })
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

    fn set_player_position(&mut self, character_guid: u32, position: WorldPosition) {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return;
        };
        player.position = position;
        player.cell = cell_coord_for_position(position);
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

const DAMAGE_FALL: u8 = 2;
const FALL_DAMAGE_MINIMUM_HEIGHT: f32 = 14.57;
const FALL_DAMAGE_DISTANCE_MULTIPLIER: f32 = 0.018;
const FALL_DAMAGE_BASE_PERCENT: f32 = 0.2426;

#[derive(Debug, Clone, Copy)]
struct PlayerFallUpdate {
    last_fall_z: Option<f32>,
    last_fall_time: u32,
    damage: Option<u32>,
}

fn player_fall_update(
    player: &PlayerRuntime,
    opcode: u16,
    movement: &MovementInfo,
) -> PlayerFallUpdate {
    if player.health == 0 || player.flags & PLAYER_FLAGS_GHOST != 0 {
        return PlayerFallUpdate {
            last_fall_z: None,
            last_fall_time: 0,
            damage: None,
        };
    }

    if opcode == MSG_MOVE_FALL_LAND as u16 {
        let fall_start_z = player.last_fall_z.unwrap_or(player.position.z);
        return PlayerFallUpdate {
            last_fall_z: None,
            last_fall_time: 0,
            damage: calculate_fall_damage(fall_start_z, movement.position.z, player.max_health),
        };
    }

    if movement.fall_time == 0 {
        return PlayerFallUpdate {
            last_fall_z: None,
            last_fall_time: 0,
            damage: None,
        };
    }

    let mut last_fall_z = player.last_fall_z;
    let mut last_fall_time = player.last_fall_time;
    if movement.fall_time > player.fall_time || movement.fall_time > player.last_fall_time {
        let highest_z = player
            .last_fall_z
            .unwrap_or(player.position.z)
            .max(player.position.z);
        if highest_z > movement.position.z {
            last_fall_z = Some(highest_z);
            last_fall_time = movement.fall_time;
        }
    }

    PlayerFallUpdate {
        last_fall_z,
        last_fall_time,
        damage: None,
    }
}

fn calculate_fall_damage(fall_start_z: f32, landing_z: f32, max_health: u32) -> Option<u32> {
    let fall_height = fall_start_z - landing_z;
    if fall_height <= FALL_DAMAGE_MINIMUM_HEIGHT {
        return None;
    }
    let damage_percent = FALL_DAMAGE_DISTANCE_MULTIPLIER * fall_height - FALL_DAMAGE_BASE_PERCENT;
    if damage_percent <= 0.0 {
        return None;
    }
    let damage = ((max_health.max(1) as f32) * damage_percent).floor() as u32;
    Some(damage.max(1).min(max_health.max(1)))
}

fn health_regen_per_second_for_spirit(class: u8, spirit: u32) -> f32 {
    let spirit = spirit as f32;
    match class {
        1 => spirit * 1.26 - 22.6, // Warrior
        2 => spirit * 0.25,        // Paladin
        3 => spirit * 0.43 - 5.5,  // Hunter
        4 => spirit * 0.84 - 13.0, // Rogue
        5 => spirit * 0.15 + 1.4,  // Priest
        7 => spirit * 0.28 - 3.6,  // Shaman
        8 => spirit * 0.11 + 1.0,  // Mage
        9 => spirit * 0.12 + 1.5,  // Warlock
        11 => spirit * 0.11 + 1.0, // Druid
        _ => 0.0,
    }
}

fn mana_regen_per_second_for_spirit(class: u8, spirit: u32) -> f32 {
    let spirit = spirit as f32;
    let per_two_seconds = match class {
        2 => spirit / 5.0 + 15.0,  // Paladin
        3 => spirit / 5.0 + 15.0,  // Hunter
        5 => spirit / 4.0 + 12.5,  // Priest
        7 => spirit / 5.0 + 17.0,  // Shaman
        8 => spirit / 4.0 + 12.5,  // Mage
        9 => spirit / 5.0 + 15.0,  // Warlock
        11 => spirit / 5.0 + 15.0, // Druid
        _ => 0.0,
    };
    per_two_seconds / 2.0
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
