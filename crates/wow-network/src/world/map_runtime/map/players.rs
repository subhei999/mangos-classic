use super::*;

pub(in crate::world) const PLAYER_RANGED_AUTO_ATTACK_WINDUP_MILLIS: u64 = 500;

// CMaNGOS reference: src/game/Maps/Map.cpp player enter, movement, visibility, and nearby broadcast.
pub(in crate::world) const PLAYER_MANA_REGEN_INTERRUPT: Duration = Duration::from_secs(5);
pub(in crate::world) const PLAYER_ENERGY_REGEN_PER_TICK: u32 = 20;
pub(in crate::world) const CMANGOS_DISCONNECTED_PLAYER_LINGER: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(in crate::world) struct PlayerAreaDiscoveryEvent {
    pub(in crate::world) area_flag: u16,
    pub(in crate::world) offset: usize,
    pub(in crate::world) field_value: u32,
    pub(in crate::world) explored_zones: [u32; PLAYER_EXPLORED_ZONES_SIZE],
    pub(in crate::world) update_body: Vec<u8>,
}

impl MapRuntime {
    pub(in crate::world) fn advance_player_environment_tick(
        &mut self,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        self.advance_player_environment_tick_with_flags(now, |geometry, position| {
            geometry.environment_flags(position)
        })
    }

    pub(in crate::world) fn advance_player_environment_tick_with_flags(
        &mut self,
        now: Instant,
        mut flags_for: impl FnMut(&WorldGeometry, WorldPosition) -> u32,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let mut direct_packets = Vec::new();
        let mut damage_events = Vec::new();
        let mut aura_interrupt_events = Vec::new();
        let geometry = self.geometry.clone();

        for player in self.players.values_mut() {
            if player.bot_runtime.is_some() {
                continue;
            }
            let character_guid = player.guid;
            let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
            let old_flags = player.environment.flags;
            let new_flags = flags_for(&geometry, player.position);
            direct_packets.extend(update_player_environment_flags(
                player, old_flags, new_flags,
            )?);
            let player_session_id = player.client_session_id();

            let diff_millis = player
                .environment
                .last_tick_at
                .map(|last| {
                    now.saturating_duration_since(last)
                        .as_millis()
                        .min(u128::from(u32::MAX)) as u32
                })
                .unwrap_or(0);
            player.environment.last_tick_at = Some(now);

            let mut damage = Vec::new();
            let fatigue_active = player_environment_timer_active(player, MIRROR_TIMER_FATIGUE);
            let fatigue_deactivated =
                player_environment_timer_deactivated(player, MIRROR_TIMER_FATIGUE);
            if advance_environment_timer(
                &mut player.environment.fatigue,
                diff_millis,
                fatigue_active,
                fatigue_deactivated,
                &mut direct_packets,
                player_session_id,
            )? {
                damage.push((
                    DAMAGE_EXHAUSTED,
                    environmental_breath_or_fatigue_damage(player.max_health, player.level),
                ));
            }
            let breath_active = player_environment_timer_active(player, MIRROR_TIMER_BREATH);
            let breath_deactivated =
                player_environment_timer_deactivated(player, MIRROR_TIMER_BREATH);
            if advance_environment_timer(
                &mut player.environment.breath,
                diff_millis,
                breath_active,
                breath_deactivated,
                &mut direct_packets,
                player_session_id,
            )? {
                damage.push((
                    DAMAGE_DROWNING,
                    environmental_breath_or_fatigue_damage(player.max_health, player.level),
                ));
            }
            let environmental_active =
                player_environment_timer_active(player, MIRROR_TIMER_ENVIRONMENTAL);
            let environmental_deactivated =
                player_environment_timer_deactivated(player, MIRROR_TIMER_ENVIRONMENTAL);
            if advance_environment_timer(
                &mut player.environment.environmental,
                diff_millis,
                environmental_active,
                environmental_deactivated,
                &mut direct_packets,
                player_session_id,
            )? && player.environment.flags & ENVIRONMENT_FLAG_IN_MAGMA != 0
            {
                damage.push((DAMAGE_LAVA, environmental_lava_damage()));
            }

            if player.health == 0 || player.flags & PLAYER_FLAGS_GHOST != 0 {
                continue;
            }

            for (damage_type, amount) in damage {
                if amount == 0 || player.health == 0 {
                    continue;
                }
                let applied_damage = amount.min(player.health);
                if remove_active_auras_with_interrupt_flag(
                    &mut player.active_auras,
                    AURA_INTERRUPT_FLAG_DAMAGE,
                ) {
                    player.stand_state = PLAYER_STAND_STATE_STAND;
                    let aura_packet = OutboundWorldPacket {
                        opcode: SMSG_UPDATE_OBJECT,
                        body: build_player_aura_update_body(player_guid, &player.active_auras)?,
                    };
                    let stand_packet = OutboundWorldPacket {
                        opcode: SMSG_UPDATE_OBJECT,
                        body: build_player_stand_state_update_body_for_class(
                            character_guid,
                            player.class,
                            player.stand_state,
                        )?,
                    };
                    if let Some(packet) = player.packet_to_client(aura_packet.clone()) {
                        direct_packets.push(packet);
                    }
                    if let Some(packet) = player.packet_to_client(stand_packet.clone()) {
                        direct_packets.push(packet);
                    }
                    aura_interrupt_events.push((
                        character_guid,
                        player.position,
                        aura_packet,
                        stand_packet,
                    ));
                }
                let Some(applied) = apply_player_runtime_world_damage(
                    player,
                    player_guid,
                    None,
                    amount,
                    WorldDamageKind::Environmental,
                    now,
                )?
                else {
                    continue;
                };
                damage_events.push((
                    character_guid,
                    applied.position,
                    player_session_id,
                    applied.direct_packets,
                    applied.observer_packets,
                    applied.aura_packet,
                    applied.died,
                    applied.death_presentation_deferred,
                    OutboundWorldPacket {
                        opcode: SMSG_ENVIRONMENTALDAMAGELOG,
                        body: build_environmental_damage_log_body(
                            player_guid,
                            damage_type,
                            applied_damage,
                            0,
                            0,
                        )?,
                    },
                    applied.health_packet,
                ));
            }
        }

        let mut packets = direct_packets;
        for (character_guid, position, aura_packet, stand_packet) in aura_interrupt_events {
            packets.extend(
                self.nearby_player_guids(
                    position,
                    PLAYER_VISIBILITY_RADIUS_YARDS,
                    Some(character_guid),
                )
                .into_iter()
                .flat_map(|observer_guid| {
                    self.players.get(&observer_guid).map(|observer| {
                        [
                            observer.packet_to_client(aura_packet.clone()),
                            observer.packet_to_client(stand_packet.clone()),
                        ]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                    })
                })
                .flatten(),
            );
        }
        for (
            character_guid,
            position,
            direct_session_id,
            mut direct_death_packets,
            mut observer_death_packets,
            aura_packet,
            died,
            death_presentation_deferred,
            damage_log,
            health_packet,
        ) in damage_events
        {
            if died {
                let player = ObjectGuid::new(HighGuid::Player, 0, character_guid);
                if death_presentation_deferred {
                    self.pending_player_death_presentations.insert(
                        character_guid,
                        PlayerDeathPresentationRuntime { waiting_since: now },
                    );
                } else {
                    self.pending_player_death_presentations
                        .remove(&character_guid);
                }
                let cleanup = self.finalize_player_death_cleanup(player, now)?;
                direct_death_packets.extend(cleanup.direct_packets);
                observer_death_packets.extend(cleanup.observer_packets);
            }
            if let Some(direct_session_id) = direct_session_id {
                for packet in direct_death_packets {
                    packets.push((direct_session_id, packet));
                }
                packets.push((direct_session_id, damage_log.clone()));
                if let Some(packet) = aura_packet.clone() {
                    packets.push((direct_session_id, packet));
                }
                packets.push((direct_session_id, health_packet.clone()));
            }
            packets.extend(observer_death_packets);
            packets.extend(
                self.nearby_player_guids(
                    position,
                    PLAYER_VISIBILITY_RADIUS_YARDS,
                    Some(character_guid),
                )
                .into_iter()
                .flat_map(|observer_guid| {
                    self.players.get(&observer_guid).map(|observer| {
                        [
                            observer.packet_to_client(damage_log.clone()),
                            aura_packet
                                .clone()
                                .and_then(|packet| observer.packet_to_client(packet)),
                            observer.packet_to_client(health_packet.clone()),
                        ]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                    })
                })
                .flatten(),
            );
        }

        Ok(packets)
    }

    pub(in crate::world) fn advance_player_regen_tick(
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

        let mut update_packets = Vec::new();
        for player in self.players.values_mut() {
            let character_guid = player.guid;
            let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
            let in_combat = player.in_combat;
            let is_dead_or_ghost = player.health == 0 || (player.flags & PLAYER_FLAGS_GHOST) != 0;
            if is_dead_or_ghost {
                continue;
            }
            let suppress_health_regen =
                player.environment.last_damage_at.is_some_and(|damage_at| {
                    now.saturating_duration_since(damage_at) <= PLAYER_REGEN_TICK
                });

            let mut health_changed = false;
            let mut mana_changed = false;
            let mut energy_changed = false;
            let mut rage_changed = false;
            let mut consumable_health_gain = 0u32;
            let mut consumable_mana_gain = 0u32;
            let mut periodic_regen_events = Vec::new();

            for aura in &mut player.active_auras {
                let Some(regen) = aura.periodic_regen.as_mut() else {
                    continue;
                };
                while regen.next_tick_at <= now {
                    consumable_health_gain =
                        consumable_health_gain.saturating_add(regen.health_amount);
                    consumable_mana_gain = consumable_mana_gain.saturating_add(regen.mana_amount);
                    periodic_regen_events.push((
                        aura.caster,
                        aura.spell_id,
                        regen.health_amount,
                        regen.mana_amount,
                    ));
                    regen.next_tick_at += Duration::from_millis(regen.tick_millis as u64);
                }
            }

            let mut periodic_health_applied = 0u32;
            if consumable_health_gain > 0
                && !suppress_health_regen
                && player.health < player.max_health
            {
                let old_health = player.health;
                let new_health = player
                    .health
                    .saturating_add(consumable_health_gain)
                    .min(player.max_health);
                health_changed = new_health != player.health;
                player.health = new_health;
                periodic_health_applied = player.health.saturating_sub(old_health);
            }

            let mut periodic_mana_applied = 0u32;
            if consumable_mana_gain > 0
                && player.max_power1 > 0
                && player.power1 < player.max_power1
            {
                let old_mana = player.power1;
                let new_mana = player
                    .power1
                    .saturating_add(consumable_mana_gain)
                    .min(player.max_power1);
                mana_changed = new_mana != player.power1;
                player.power1 = new_mana;
                periodic_mana_applied = player.power1.saturating_sub(old_mana);
            }

            if !in_combat && !suppress_health_regen && player.health < player.max_health {
                let regen =
                    health_regen_per_second_for_spirit(player.class, player.spirit).max(0.0);
                let gained = (regen * 2.0).floor() as u32;
                if gained > 0 {
                    let new_health = player.health.saturating_add(gained).min(player.max_health);
                    health_changed = new_health != player.health;
                    player.health = new_health;
                }
            }

            let mana_regen_blocked_by_recent_cast =
                player.last_mana_use_at.is_some_and(|last_mana_use_at| {
                    now.saturating_duration_since(last_mana_use_at) < PLAYER_MANA_REGEN_INTERRUPT
                });
            if !mana_regen_blocked_by_recent_cast
                && player.max_power1 > 0
                && player.power1 < player.max_power1
            {
                let regen = mana_regen_per_second_for_spirit(player.class, player.spirit).max(0.0);
                let gained = (regen * 2.0).floor() as u32;
                if gained > 0 {
                    let new_mana = player.power1.saturating_add(gained).min(player.max_power1);
                    mana_changed = new_mana != player.power1;
                    player.power1 = new_mana;
                }
            }

            if player.class == 4 && player.max_power4 > 0 && player.power4 < player.max_power4 {
                let new_energy = player
                    .power4
                    .saturating_add(PLAYER_ENERGY_REGEN_PER_TICK)
                    .min(player.max_power4);
                energy_changed = new_energy != player.power4;
                player.power4 = new_energy;
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
                    player.client_session_id(),
                    OutboundWorldPacket {
                        opcode: SMSG_UPDATE_OBJECT,
                        body: build_player_health_update_body(player_guid, player.health)?,
                    },
                ));
                let mut remaining = periodic_health_applied;
                for (caster, spell_id, health_amount, _) in &periodic_regen_events {
                    if remaining == 0 {
                        break;
                    }
                    let logged = (*health_amount).min(remaining);
                    if logged == 0 {
                        continue;
                    }
                    remaining = remaining.saturating_sub(logged);
                    update_packets.push((
                        character_guid,
                        player.position,
                        player.client_session_id(),
                        OutboundWorldPacket {
                            opcode: SMSG_SPELLHEALLOG,
                            body: build_spell_heal_log_body(
                                *caster,
                                player_guid,
                                *spell_id,
                                logged,
                                false,
                            )?,
                        },
                    ));
                }
            }
            if mana_changed {
                update_packets.push((
                    character_guid,
                    player.position,
                    player.client_session_id(),
                    OutboundWorldPacket {
                        opcode: SMSG_UPDATE_OBJECT,
                        body: build_player_mana_update_body(player_guid, player.power1)?,
                    },
                ));
                let mut remaining = periodic_mana_applied;
                for (caster, spell_id, _, mana_amount) in &periodic_regen_events {
                    if remaining == 0 {
                        break;
                    }
                    let logged = (*mana_amount).min(remaining);
                    if logged == 0 {
                        continue;
                    }
                    remaining = remaining.saturating_sub(logged);
                    update_packets.push((
                        character_guid,
                        player.position,
                        player.client_session_id(),
                        OutboundWorldPacket {
                            opcode: SMSG_SPELLENERGIZELOG,
                            body: build_spell_energize_log_body(
                                *caster,
                                player_guid,
                                *spell_id,
                                POWER_TYPE_MANA,
                                logged,
                            )?,
                        },
                    ));
                }
            }
            if rage_changed {
                update_packets.push((
                    character_guid,
                    player.position,
                    player.client_session_id(),
                    OutboundWorldPacket {
                        opcode: SMSG_UPDATE_OBJECT,
                        body: build_player_rage_update_body(player_guid, player.power2)?,
                    },
                ));
            }
            if energy_changed {
                update_packets.push((
                    character_guid,
                    player.position,
                    player.client_session_id(),
                    OutboundWorldPacket {
                        opcode: SMSG_UPDATE_OBJECT,
                        body: build_player_energy_update_body(player_guid, player.power4)?,
                    },
                ));
            }
        }

        let mut packets = Vec::new();
        for (character_guid, position, direct_session_id, packet) in update_packets {
            if let Some(direct_session_id) = direct_session_id {
                packets.push((direct_session_id, packet.clone()));
            }
            packets.extend(
                self.nearby_player_guids(
                    position,
                    PLAYER_VISIBILITY_RADIUS_YARDS,
                    Some(character_guid),
                )
                .into_iter()
                .filter_map(|observer_guid| {
                    self.players
                        .get(&observer_guid)
                        .and_then(|observer| observer.packet_to_client(packet.clone()))
                }),
            );
        }

        Ok(packets)
    }

    pub(in crate::world) fn add_player(
        &mut self,
        player: PlayerRuntime,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let mut player = player;
        let mut packets = if self.players.contains_key(&player.guid) {
            self.remove_player(player.guid)
        } else {
            Vec::new()
        };
        if player.bot_runtime.is_some() {
            if let Some(grounded_position) = self.geometry.ground_position(player.position) {
                player.position = grounded_position;
                if let Some(bot) = player.bot_runtime.as_mut() {
                    bot.home_position = grounded_position;
                }
            }
        }
        let player_guid = player.guid;
        let player_grid = grid_coord_for_position(player.position);
        let player_cell = cell_coord_for_position(player.position);
        player.cell = player_cell;
        let player_object = ObjectGuid::new(HighGuid::Player, 0, player_guid);
        let new_player_packet = OutboundWorldPacket {
            opcode: SMSG_UPDATE_OBJECT,
            body: build_update_object_body(&[build_other_player_create_block(&player)?]),
        };
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
            if player.is_client_controlled() {
                if let Some(packet) = player.packet_to_client(OutboundWorldPacket {
                    opcode: SMSG_UPDATE_OBJECT,
                    body: build_update_object_body(&[build_other_player_create_block(other)?]),
                }) {
                    packets.push(packet);
                }
                if let Some(start_packet) = moving_bot_start_packet(other)? {
                    if let Some(packet) = player.packet_to_client(start_packet) {
                        packets.push(packet);
                    }
                }
            }
            if let Some(packet) = other.packet_to_client(new_player_packet.clone()) {
                packets.push(packet);
            }
        }
        if player.is_client_controlled() {
            for other_guid in &visible_others {
                player
                    .visible_objects
                    .insert(ObjectGuid::new(HighGuid::Player, 0, *other_guid));
            }
        }
        for other_guid in &visible_others {
            if let Some(other) = self.players.get_mut(other_guid) {
                if other.is_client_controlled() {
                    other.visible_objects.insert(player_object);
                }
            }
        }

        let grid = self.grids.entry(player_grid).or_default();
        if player.is_client_controlled() {
            grid.state = GridState::Active;
            grid.active_player_count = grid.active_player_count.saturating_add(1);
        }
        grid.last_touched = Instant::now();
        grid.cells
            .entry(player_cell)
            .or_default()
            .players
            .insert(player_guid);
        if player.is_client_controlled() {
            grid.cells
                .entry(player_cell)
                .or_default()
                .client_players
                .insert(player_guid);
        }
        self.players.insert(player_guid, player);
        self.invalidate_idle_motion_start_schedule();

        Ok(packets)
    }

    pub(in crate::world) fn disconnect_player_for_linger(
        &mut self,
        character_guid: u32,
        now: Instant,
    ) -> bool {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return false;
        };
        if matches!(player.controller, PlayerController::Disconnected { .. }) {
            return true;
        }
        if !player.is_client_controlled() {
            return false;
        }

        let player_grid = grid_coord_for_position(player.position);
        if let Some(grid) = self.grids.get_mut(&player_grid) {
            grid.active_player_count = grid.active_player_count.saturating_sub(1);
            grid.last_touched = now;
            if let Some(cell) = grid.cells.get_mut(&player.cell) {
                cell.client_players.remove(&character_guid);
            }
        }
        player.controller = PlayerController::Disconnected {
            remove_at: now + CMANGOS_DISCONNECTED_PLAYER_LINGER,
        };
        player.visible_objects.clear();
        self.active_player_spell_casts.remove(&character_guid);
        self.pending_spell_events
            .retain(|event| event.caster_character_guid != character_guid);
        self.refresh_grid_state(player_grid);
        true
    }

    pub(in crate::world) fn expire_disconnected_players(
        &mut self,
        now: Instant,
    ) -> Vec<ExpiredDisconnectedPlayer> {
        let mut expired_guids = self
            .players
            .iter()
            .filter_map(|(guid, player)| {
                player
                    .disconnected_remove_at()
                    .is_some_and(|remove_at| now >= remove_at)
                    .then_some(*guid)
            })
            .collect::<Vec<_>>();
        expired_guids.sort_unstable();

        expired_guids
            .into_iter()
            .filter_map(|guid| {
                let player = self.players.get(&guid)?.clone();
                let observer_packets = self.remove_player(guid);
                Some(ExpiredDisconnectedPlayer {
                    player,
                    observer_packets,
                })
            })
            .collect()
    }

    pub(in crate::world) fn update_player_position(
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
        let mover_is_client_controlled = current_player.is_client_controlled();

        let old_visible = if mover_is_client_controlled {
            current_player
                .visible_objects
                .iter()
                .filter_map(|guid| {
                    if guid.is_player() {
                        Some(guid.counter())
                    } else {
                        None
                    }
                })
                .collect::<HashSet<_>>()
        } else {
            self.nearby_client_player_guids(
                current_player.position,
                PLAYER_VISIBILITY_RADIUS_YARDS,
                Some(character_guid),
            )
            .into_iter()
            .filter(|observer_guid| {
                self.players
                    .get(observer_guid)
                    .is_some_and(|observer| observer.visible_objects.contains(&player_object))
            })
            .collect::<HashSet<_>>()
        };
        let retained_non_player_visible = current_player
            .visible_objects
            .iter()
            .filter(|guid| !guid.is_player())
            .copied()
            .collect::<HashSet<_>>();
        let nearby_visible = if mover_is_client_controlled {
            self.nearby_player_guids(
                movement.position,
                PLAYER_VISIBILITY_RADIUS_YARDS,
                Some(character_guid),
            )
        } else {
            self.nearby_client_player_guids(
                movement.position,
                PLAYER_VISIBILITY_RADIUS_YARDS,
                Some(character_guid),
            )
        };
        let new_visible = nearby_visible.into_iter().collect::<HashSet<_>>();

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
            if let Some(packet) = current_player.packet_to_client(OutboundWorldPacket {
                opcode: SMSG_DESTROY_OBJECT,
                body: build_destroy_guid_body(ObjectGuid::new(HighGuid::Player, 0, other_guid)),
            }) {
                packets.push(packet);
            }
            if let Some(packet) = other.packet_to_client(OutboundWorldPacket {
                opcode: SMSG_DESTROY_OBJECT,
                body: build_destroy_guid_body(player_object),
            }) {
                packets.push(packet);
            }
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
            if let Some(packet) = other.packet_to_client(OutboundWorldPacket {
                opcode: SMSG_UPDATE_OBJECT,
                body: build_update_object_body(std::slice::from_ref(&moving_player_create)),
            }) {
                packets.push(packet);
            }
        }
        for other_guid in &entering_for_mover {
            if let Some(other) = self.players.get(other_guid) {
                if let Some(packet) = current_player.packet_to_client(OutboundWorldPacket {
                    opcode: SMSG_UPDATE_OBJECT,
                    body: build_update_object_body(&[build_other_player_create_block(other)?]),
                }) {
                    packets.push(packet);
                }
                if let Some(start_packet) = moving_bot_start_packet(other)? {
                    if let Some(packet) = current_player.packet_to_client(start_packet) {
                        packets.push(packet);
                    }
                }
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
            if let Some(packet) = other.packet_to_client(movement_packet.clone()) {
                packets.push(packet);
            }
        }

        if old_grid != new_grid || old_cell != new_cell {
            if let Some(grid) = self.grids.get_mut(&old_grid) {
                if old_grid != new_grid && mover_is_client_controlled {
                    grid.active_player_count = grid.active_player_count.saturating_sub(1);
                }
                if let Some(cell) = grid.cells.get_mut(&old_cell) {
                    cell.players.remove(&character_guid);
                    if mover_is_client_controlled {
                        cell.client_players.remove(&character_guid);
                    }
                }
                grid.last_touched = Instant::now();
            }
            let grid = self.grids.entry(new_grid).or_default();
            if mover_is_client_controlled {
                if old_grid != new_grid {
                    grid.active_player_count = grid.active_player_count.saturating_add(1);
                }
                grid.state = GridState::Active;
            }
            grid.last_touched = Instant::now();
            grid.cells
                .entry(new_cell)
                .or_default()
                .players
                .insert(character_guid);
            if mover_is_client_controlled {
                grid.cells
                    .entry(new_cell)
                    .or_default()
                    .client_players
                    .insert(character_guid);
            }
            if old_grid != new_grid {
                self.refresh_grid_state(old_grid);
            }
            self.refresh_grid_state(new_grid);
            self.invalidate_idle_motion_start_schedule();
        }

        let mut fall_applied = None;
        if let Some(player) = self.players.get_mut(&character_guid) {
            player.position = movement.position;
            player.movement_flags = movement.flags;
            player.client_time = movement.client_time;
            player.server_time = server_time;
            player.fall_time = tracked_player_fall_time(opcode, movement);
            player.last_fall_z = fall_update.last_fall_z;
            player.last_fall_time = fall_update.last_fall_time;
            player.jump = if player.fall_time == 0 {
                JumpInfo::default()
            } else {
                movement.jump.clone()
            };
            player.cell = new_cell;
            if mover_is_client_controlled {
                player.visible_objects = retained_non_player_visible;
                player.visible_objects.extend(
                    new_visible
                        .iter()
                        .map(|guid| ObjectGuid::new(HighGuid::Player, 0, *guid)),
                );
            } else {
                player.visible_objects = retained_non_player_visible;
            }
            if let Some(damage) = fall_update.damage {
                fall_applied = apply_player_runtime_world_damage(
                    player,
                    player_object,
                    None,
                    damage,
                    WorldDamageKind::Fall,
                    Instant::now(),
                )?;
            }
        }

        if let Some(applied) = fall_applied.as_mut().filter(|applied| applied.died) {
            if applied.death_presentation_deferred {
                self.pending_player_death_presentations.insert(
                    character_guid,
                    PlayerDeathPresentationRuntime {
                        waiting_since: Instant::now(),
                    },
                );
            } else {
                self.pending_player_death_presentations
                    .remove(&character_guid);
            }
            let cleanup = self.finalize_player_death_cleanup(player_object, Instant::now())?;
            applied.direct_packets.extend(cleanup.direct_packets);
            applied.observer_packets.extend(cleanup.observer_packets);
        }

        if let Some(damage) = fall_update.damage {
            let (health, death_state, flags) = self
                .players
                .get(&character_guid)
                .map(|player| (player.health, player.death_state, player.flags))
                .unwrap_or((
                    current_player.health.saturating_sub(damage),
                    current_player.death_state,
                    current_player.flags,
                ));
            let damage_log = OutboundWorldPacket {
                opcode: SMSG_ENVIRONMENTALDAMAGELOG,
                body: build_environmental_damage_log_body(
                    player_object,
                    DAMAGE_FALL,
                    damage,
                    0,
                    0,
                )?,
            };
            let health_update_body = if let Some(applied) = fall_applied.as_ref() {
                applied.health_packet.body.clone()
            } else if death_state == PlayerDeathState::Corpse && health == 0 {
                build_player_death_update_body(
                    player_object,
                    0,
                    flags,
                    PLAYER_FIELD_BYTE_RELEASE_TIMER,
                    player_unit_flags(false),
                    current_player.class,
                    PLAYER_STAND_STATE_DEAD,
                )?
            } else {
                build_player_health_update_body(player_object, health)?
            };
            let health_update = OutboundWorldPacket {
                opcode: SMSG_UPDATE_OBJECT,
                body: health_update_body,
            };
            if let Some(applied) = fall_applied.as_ref() {
                for packet in &applied.direct_packets {
                    if let Some(packet) = current_player.packet_to_client(packet.clone()) {
                        packets.push(packet);
                    }
                }
            }
            if let Some(packet) = current_player.packet_to_client(damage_log.clone()) {
                packets.push(packet);
            }
            if let Some(aura_packet) = fall_applied
                .as_ref()
                .and_then(|applied| applied.aura_packet.clone())
            {
                if let Some(packet) = current_player.packet_to_client(aura_packet) {
                    packets.push(packet);
                }
            }
            if let Some(packet) = current_player.packet_to_client(health_update.clone()) {
                packets.push(packet);
            }
            if let Some(applied) = fall_applied.as_ref() {
                packets.extend(applied.observer_packets.clone());
            }
            for other_guid in &new_visible {
                let Some(other) = self.players.get(other_guid) else {
                    continue;
                };
                if let Some(packet) = other.packet_to_client(damage_log.clone()) {
                    packets.push(packet);
                }
                if let Some(aura_packet) = fall_applied
                    .as_ref()
                    .and_then(|applied| applied.aura_packet.clone())
                {
                    if let Some(packet) = other.packet_to_client(aura_packet) {
                        packets.push(packet);
                    }
                }
                if let Some(packet) = other.packet_to_client(health_update.clone()) {
                    packets.push(packet);
                }
            }
        }
        packets.extend(self.present_player_death_if_ready(
            character_guid,
            Instant::now(),
            false,
        )?);
        self.invalidate_idle_motion_start_schedule();

        Ok(packets)
    }

    pub(in crate::world) fn discover_player_area(
        &mut self,
        character_guid: u32,
        area_flag: u16,
    ) -> anyhow::Result<Option<PlayerAreaDiscoveryEvent>> {
        if area_flag == u16::MAX {
            return Ok(None);
        }
        let offset = usize::from(area_flag) / 32;
        if offset >= PLAYER_EXPLORED_ZONES_SIZE {
            return Ok(None);
        }
        let bit = 1u32 << (u32::from(area_flag) % 32);
        let Some(player) = self.players.get_mut(&character_guid) else {
            return Ok(None);
        };
        if player.health == 0 || player.flags & PLAYER_FLAGS_GHOST != 0 {
            return Ok(None);
        }
        if player.explored_zones[offset] & bit != 0 {
            return Ok(None);
        }
        player.explored_zones[offset] |= bit;
        let field_value = player.explored_zones[offset];
        let explored_zones = player.explored_zones;
        Ok(Some(PlayerAreaDiscoveryEvent {
            area_flag,
            offset,
            field_value,
            explored_zones,
            update_body: build_player_explored_zone_update_body(
                character_guid,
                offset,
                field_value,
            )?,
        }))
    }

    pub(in crate::world) fn update_player_visible_equipment(
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
                    .and_then(|other| other.packet_to_client(packet.clone()))
            })
            .collect())
    }

    pub(in crate::world) fn update_player_health(
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
                    .and_then(|other| other.packet_to_client(packet.clone()))
            })
            .collect())
    }

    pub(in crate::world) fn apply_player_heal(
        &mut self,
        target_character_guid: u32,
        amount: u32,
    ) -> anyhow::Result<Option<PlayerHealEvent>> {
        let Some(player) = self.players.get_mut(&target_character_guid) else {
            return Ok(None);
        };
        if player.health == 0 || amount == 0 {
            return Ok(None);
        }
        let previous_health = player.health;
        player.health = player.health.saturating_add(amount).min(player.max_health);
        let amount_healed = player.health.saturating_sub(previous_health);
        if amount_healed == 0 {
            return Ok(None);
        }
        let position = player.position;
        let Some(direct_session_id) = player.client_session_id() else {
            return Ok(None);
        };
        let health = player.health;
        let packet = OutboundWorldPacket {
            opcode: SMSG_UPDATE_OBJECT,
            body: build_player_health_update_body(
                ObjectGuid::new(HighGuid::Player, 0, target_character_guid),
                health,
            )?,
        };
        let _ = player;
        let observer_packets = self
            .nearby_player_guids(
                position,
                PLAYER_VISIBILITY_RADIUS_YARDS,
                Some(target_character_guid),
            )
            .into_iter()
            .filter_map(|other_guid| {
                self.players
                    .get(&other_guid)
                    .and_then(|other| other.packet_to_client(packet.clone()))
            })
            .collect();
        Ok(Some(PlayerHealEvent {
            healed_character_guid: target_character_guid,
            amount_healed,
            health,
            direct_session_id,
            direct_packets: vec![packet.clone()],
            observer_packets,
        }))
    }

    pub(in crate::world) fn add_player_combo_points(
        &mut self,
        character_guid: u32,
        target: ObjectGuid,
        points: u8,
    ) -> Option<PlayerComboPointsEvent> {
        let player = self.players.get_mut(&character_guid)?;
        if points == 0 {
            return None;
        }
        if player.combo_target == Some(target) {
            player.combo_points = player.combo_points.saturating_add(points).min(5);
        } else {
            player.combo_target = Some(target);
            player.combo_points = points.min(5);
        }
        Some(PlayerComboPointsEvent {
            combo_target: target,
            combo_points: player.combo_points,
            player_bytes: player.player_bytes,
        })
    }

    pub(in crate::world) fn clear_player_combo_points(
        &mut self,
        character_guid: u32,
    ) -> Option<PlayerComboPointsEvent> {
        let player = self.players.get_mut(&character_guid)?;
        let target = player.combo_target?;
        player.combo_points = 0;
        player.combo_target = None;
        Some(PlayerComboPointsEvent {
            combo_target: target,
            combo_points: 0,
            player_bytes: player.player_bytes,
        })
    }

    pub(in crate::world) fn sync_player_gameplay_state(
        &mut self,
        character_guid: u32,
        session: &WorldSessionState,
    ) {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return;
        };
        player.max_health = player
            .max_health
            .max(session.character.player_health.max(1));
        player.max_power1 = player.max_power1.max(session.character.player_mana);
        let map_death_is_newer = player.health == 0
            && player.death_state != PlayerDeathState::Alive
            && session.death.player_death_state == PlayerDeathState::Alive;
        let session_death_state = session.death.player_death_state;
        if !map_death_is_newer {
            player.death_state = session.death.player_death_state;
            if session_death_state != PlayerDeathState::Alive {
                player.health = session.character.player_health;
            };
        }
        if !map_death_is_newer {
            player.stand_state =
                if player.death_state == PlayerDeathState::Corpse && player.health == 0 {
                    PLAYER_STAND_STATE_DEAD
                } else {
                    session.character.player_stand_state
                };
        }
        player.active_spells = session.character.active_spells.clone();
        player.inventory = session.inventory.items.clone();
        player.quest_statuses = session.quests.quest_statuses.clone();
        refresh_player_runtime_stats_from_auras(player);
        player.combat_stats =
            combat_stats_with_active_auras(player.base_combat_stats, &player.active_auras);
        if let Some(character) = session.character.active_character.as_ref() {
            player.level = character.level;
            player.xp = character.xp;
            if !map_death_is_newer {
                player.flags = session.character.player_flags;
                player.position = character.position;
                player.movement_flags = character.movement_flags;
                player.client_time = character.client_time;
                player.fall_time = character.fall_time;
                if character.movement_flags & MOVEFLAG_JUMPING == 0 || character.fall_time == 0 {
                    player.last_fall_z = None;
                    player.last_fall_time = 0;
                    player.jump = JumpInfo::default();
                } else {
                    player.jump = character.jump.clone();
                }
            }
            let equipment_cache = session
                .character
                .player_visual
                .as_ref()
                .and_then(|visual| visual.equipment_cache.as_deref());
            player.visible_equipment =
                visible_equipment_for_inventory(equipment_cache, &session.inventory.items);
        }
    }

    pub(in crate::world) fn player_runtime_snapshot(
        &self,
        character_guid: u32,
    ) -> Option<PlayerRuntimeSnapshot> {
        let player = self.players.get(&character_guid)?;
        Some(PlayerRuntimeSnapshot {
            position: player.position,
            movement_flags: player.movement_flags,
            client_time: player.client_time,
            fall_time: player.fall_time,
            jump: player.jump.clone(),
            flags: player.flags,
            death_state: player.death_state,
            stand_state: player.stand_state,
            level: player.level,
            race: player.race,
            class: player.class,
            xp: player.xp,
            health: player.health,
            max_health: player.max_health,
            power1: player.power1,
            max_power1: player.max_power1,
            last_mana_use_at: player.last_mana_use_at,
            power2: player.power2,
            power4: player.power4,
            max_power4: player.max_power4,
            combo_target: player.combo_target,
            combo_points: player.combo_points,
            active_spells: player.active_spells.clone(),
            inventory: player.inventory.clone(),
            quest_statuses: player.quest_statuses.clone(),
            active_auras: player.active_auras.clone(),
            spell_global_cooldowns_until: player.spell_global_cooldowns_until.clone(),
            spell_cooldowns_until: player.spell_cooldowns_until.clone(),
            spell_cooldown_categories: player.spell_cooldown_categories.clone(),
            spell_cooldown_item_ids: player.spell_cooldown_item_ids.clone(),
            queued_next_melee_spell: player.queued_next_melee_spell,
            base_combat_stats: player.base_combat_stats,
            combat_stats: player.combat_stats,
            in_combat: player.in_combat,
            active_combat_target: player.active_combat_target,
            active_combat_attack_kind: player.active_combat_attack_kind,
            active_combat_next_swing_at: player.active_combat_next_swing_at,
        })
    }

    pub(in crate::world) fn player_visible_db_creature_guids(
        &self,
        character_guid: u32,
    ) -> Vec<u64> {
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

    pub(in crate::world) fn update_player_reward_state(
        &mut self,
        character_guid: u32,
        reward: PlayerRewardRuntimeUpdate,
    ) {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return;
        };
        player.level = reward.level;
        player.xp = reward.xp;
        if let Some(world_stats) = reward.world_stats {
            player.base_world_stats = world_stats;
            player.effective_world_stats =
                player_world_stats_with_active_auras(player.base_world_stats, &player.active_auras);
            player.spirit = player.effective_world_stats.stats[4];
            player.max_health = player.effective_world_stats.max_health().max(1);
        } else {
            player.max_health = reward.max_health.max(1);
        }
        player.max_power1 = reward.max_power1;
        player.health = reward.health.min(player.max_health);
        player.power1 = reward.power1.min(player.max_power1);
        player.power2 = reward.power2.min(POWER_RAGE_DEFAULT);
        player.power4 = player.power4.min(player.max_power4);
        if let Some(combat_stats) = reward.combat_stats {
            player.base_combat_stats = combat_stats;
            player.combat_stats =
                combat_stats_with_active_auras(player.base_combat_stats, &player.active_auras);
        }
        player.quest_statuses = reward.quest_statuses;
    }

    pub(in crate::world) fn update_player_level_progression_state(
        &mut self,
        character_guid: u32,
        progression: PlayerLevelProgressionRuntimeUpdate,
    ) {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return;
        };
        player.level = progression.level;
        player.xp = progression.xp;
        player.base_world_stats = progression.world_stats;
        player.effective_world_stats =
            player_world_stats_with_active_auras(player.base_world_stats, &player.active_auras);
        player.spirit = player.effective_world_stats.stats[4];
        player.max_health = player.effective_world_stats.max_health().max(1);
        player.health = progression.health.min(player.max_health);
        player.max_power1 = player.effective_world_stats.max_mana();
        player.power1 = progression.power1.min(player.max_power1);
        player.power2 = progression.power2.min(POWER_RAGE_DEFAULT);
        player.power4 = progression.power4.min(player.max_power4);
        player.base_combat_stats = progression.combat_stats;
        player.combat_stats =
            combat_stats_with_active_auras(player.base_combat_stats, &player.active_auras);
    }

    pub(in crate::world) fn update_player_inventory(
        &mut self,
        character_guid: u32,
        inventory: Vec<CharacterInventoryItem>,
    ) {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return;
        };
        player.inventory = inventory;
    }

    pub(in crate::world) fn player_visible_db_gameobject_guids(
        &self,
        character_guid: u32,
    ) -> Vec<u64> {
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

    pub(in crate::world) fn should_rescan_player_creature_visibility(
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

    pub(in crate::world) fn should_rescan_player_gameobject_visibility(
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

    pub(in crate::world) fn should_rescan_player_corpse_visibility(
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

    pub(in crate::world) fn reset_player_visibility_scan_positions(&mut self, character_guid: u32) {
        if let Some(player) = self.players.get_mut(&character_guid) {
            player.last_creature_visibility_position = None;
            player.last_gameobject_visibility_position = None;
            player.last_player_corpse_visibility_position = None;
        }
    }

    pub(in crate::world) fn update_player_combat_stats(
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
                    .and_then(|observer| observer.packet_to_client(packet.clone()))
            })
            .collect())
    }

    pub(in crate::world) fn player_combat_stats(
        &self,
        character_guid: u32,
    ) -> Option<PlayerCombatStats> {
        self.players
            .get(&character_guid)
            .map(|player| player.combat_stats)
    }

    pub(in crate::world) fn remove_player_auras_with_interrupt_flag(
        &mut self,
        character_guid: u32,
        interrupt_flag: u32,
    ) -> bool {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return false;
        };
        let changed =
            remove_active_auras_with_interrupt_flag(&mut player.active_auras, interrupt_flag);
        if changed {
            refresh_player_runtime_stats_from_auras(player);
            player.combat_stats =
                combat_stats_with_active_auras(player.base_combat_stats, &player.active_auras);
        }
        changed
    }

    pub(in crate::world) fn remove_player_auras_by_dispel_type(
        &mut self,
        character_guid: u32,
        dispel_type: u32,
        count: u32,
        now: Instant,
    ) -> anyhow::Result<Option<PlayerAuraDispelEvent>> {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return Ok(None);
        };
        let remove_count = count.max(1) as usize;
        let mut removed_spell_ids = Vec::new();
        let mut remaining = remove_count;
        let was_rooted = active_aura_has_root(&player.active_auras);
        player.active_auras.retain(|aura| {
            if remaining == 0 || !active_aura_matches_dispel_type(aura, dispel_type) {
                return true;
            }
            removed_spell_ids.push(aura.spell_id);
            remaining -= 1;
            false
        });
        if removed_spell_ids.is_empty() {
            return Ok(None);
        }
        let is_rooted = active_aura_has_root(&player.active_auras);
        refresh_player_runtime_stats_from_auras(player);
        player.combat_stats =
            combat_stats_with_active_auras(player.base_combat_stats, &player.active_auras);
        let mut aura_update = self.build_player_aura_update_event(character_guid, now)?;
        if let Some(packet) =
            build_player_root_transition_packet(character_guid, was_rooted, is_rooted)?
        {
            aura_update.direct_packets.push(packet);
        }
        Ok(Some(PlayerAuraDispelEvent {
            removed_spell_ids,
            aura_update,
        }))
    }

    pub(in crate::world) fn apply_player_aura(
        &mut self,
        character_guid: u32,
        aura: ActiveAura,
    ) -> anyhow::Result<Option<PlayerAuraUpdateEvent>> {
        self.apply_player_aura_replacing_spell_ids(character_guid, aura, &[])
    }

    pub(in crate::world) fn apply_player_aura_replacing_spell_ids(
        &mut self,
        character_guid: u32,
        aura: ActiveAura,
        replace_spell_ids: &[u32],
    ) -> anyhow::Result<Option<PlayerAuraUpdateEvent>> {
        let resolution = AuraRankConflictResolution {
            failure: None,
            replace_spell_ids: replace_spell_ids.to_vec(),
            replace_any_caster_spell_ids: Vec::new(),
        };
        self.apply_player_aura_replacing_conflicts(character_guid, aura, &resolution)
    }

    pub(in crate::world) fn apply_player_aura_replacing_conflicts(
        &mut self,
        character_guid: u32,
        aura: ActiveAura,
        resolution: &AuraRankConflictResolution,
    ) -> anyhow::Result<Option<PlayerAuraUpdateEvent>> {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return Ok(None);
        };
        let was_rooted = active_aura_has_root(&player.active_auras);
        apply_active_aura_replacing_conflicts(&mut player.active_auras, aura, resolution);
        let is_rooted = active_aura_has_root(&player.active_auras);
        refresh_player_runtime_stats_from_auras(player);
        player.combat_stats =
            combat_stats_with_active_auras(player.base_combat_stats, &player.active_auras);
        let mut event = self.build_player_aura_update_event(character_guid, Instant::now())?;
        if let Some(packet) =
            build_player_root_transition_packet(character_guid, was_rooted, is_rooted)?
        {
            event.direct_packets.push(packet);
        }
        Ok(Some(event))
    }

    pub(in crate::world) fn advance_player_aura_expirations(
        &mut self,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let mut packets = Vec::new();
        let player_snapshots = self
            .players
            .iter()
            .map(|(guid, player)| {
                (
                    ObjectGuid::new(HighGuid::Player, 0, *guid).raw(),
                    player_spell_snapshot(player.level, player.class, &player.combat_stats),
                )
            })
            .collect::<HashMap<_, _>>();
        let creature_snapshots = self
            .creatures
            .iter()
            .map(|(guid, creature)| (*guid, db_creature_spell_snapshot(creature)))
            .collect::<HashMap<_, _>>();
        let character_guids = self.players.keys().copied().collect::<Vec<_>>();
        for character_guid in character_guids {
            let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
            let mut tick_packets = Vec::new();
            let mut health_changed = false;
            let mut player_died = false;
            let mut direct_death_packets = Vec::new();
            let mut observer_death_packets = Vec::new();
            let mut pending_damage_ticks = Vec::new();
            let Some(player) = self.players.get_mut(&character_guid) else {
                continue;
            };
            if player.death_state != PlayerDeathState::Alive || player.health == 0 {
                continue;
            }
            let before = player.active_auras.len();
            let target_snapshot =
                player_spell_snapshot(player.level, player.class, &player.combat_stats);
            for aura in &mut player.active_auras {
                let Some(periodic) = aura.periodic_damage.as_mut() else {
                    continue;
                };
                if aura
                    .expires_at
                    .is_some_and(|expires_at| periodic.next_tick_at > expires_at)
                {
                    continue;
                }
                if now < periodic.next_tick_at {
                    continue;
                }
                while periodic.next_tick_at <= now {
                    periodic.next_tick_at += Duration::from_millis(periodic.tick_millis as u64);
                }
                let caster_snapshot = player_snapshots
                    .get(&aura.caster.raw())
                    .or_else(|| creature_snapshots.get(&aura.caster.raw()))
                    .copied()
                    .unwrap_or(periodic.caster_snapshot);
                let tick = calculate_periodic_damage_tick(
                    periodic,
                    caster_snapshot,
                    target_snapshot,
                    player.health,
                );
                if tick.dealt_damage == 0 {
                    continue;
                }
                pending_damage_ticks.push((
                    aura.caster,
                    aura.spell_id,
                    periodic.aura_name,
                    periodic.school,
                    tick,
                ));
            }
            let _ = player;
            for (caster, spell_id, aura_name, school, tick) in pending_damage_ticks {
                let Some(applied) = self.apply_player_world_damage_with_school_mask(
                    player_guid,
                    Some(caster),
                    tick.dealt_damage,
                    WorldDamageKind::PeriodicAura,
                    school.max(SPELL_SCHOOL_MASK_NORMAL),
                    now,
                )?
                else {
                    continue;
                };
                health_changed = true;
                tick_packets.push(OutboundWorldPacket {
                    opcode: SMSG_PERIODICAURALOG,
                    body: build_periodic_aura_log_body(PeriodicAuraLog {
                        creature_guid: player_guid,
                        caster,
                        spell_id,
                        aura_name,
                        tick,
                    })?,
                });
                if let Some(aura_packet) = applied.aura_packet {
                    tick_packets.push(aura_packet);
                }
                direct_death_packets.extend(applied.direct_packets);
                observer_death_packets.extend(applied.observer_packets);
                if applied.died {
                    player_died = true;
                    break;
                }
            }
            let Some(player) = self.players.get_mut(&character_guid) else {
                continue;
            };
            let was_rooted = active_aura_has_root(&player.active_auras);
            player
                .active_auras
                .retain(|aura| aura.expires_at.is_none_or(|expires_at| now < expires_at));
            let is_rooted = active_aura_has_root(&player.active_auras);
            if player_died {
                self.active_player_spell_casts.remove(&character_guid);
            }
            if player.active_auras.len() == before && !health_changed && tick_packets.is_empty() {
                continue;
            }
            refresh_player_runtime_stats_from_auras(player);
            player.combat_stats =
                combat_stats_with_active_auras(player.base_combat_stats, &player.active_auras);
            let mut event = self.build_player_aura_update_event(character_guid, now)?;
            if let Some(packet) =
                build_player_root_transition_packet(character_guid, was_rooted, is_rooted)?
            {
                event.direct_packets.push(packet);
            }
            let Some(player) = self.players.get(&character_guid) else {
                continue;
            };
            for packet in direct_death_packets {
                if let Some(packet) = player.packet_to_client(packet) {
                    packets.push(packet);
                }
            }
            packets.extend(observer_death_packets);
            for packet in &tick_packets {
                if let Some(packet) = player.packet_to_client(packet.clone()) {
                    packets.push(packet);
                }
            }
            if health_changed {
                if let Some(packet) = player.packet_to_client(OutboundWorldPacket {
                    opcode: SMSG_UPDATE_OBJECT,
                    body: if player.death_state == PlayerDeathState::Corpse && player.health == 0 {
                        build_player_death_update_body(
                            player_guid,
                            0,
                            player.flags,
                            PLAYER_FIELD_BYTE_RELEASE_TIMER,
                            player_unit_flags(false),
                            player.class,
                            PLAYER_STAND_STATE_DEAD,
                        )?
                    } else {
                        build_player_health_update_body(player_guid, player.health)?
                    },
                }) {
                    packets.push(packet);
                }
            }
            packets.extend(
                event
                    .direct_packets
                    .into_iter()
                    .filter_map(|packet| player.packet_to_client(packet)),
            );
            for observer_guid in self.nearby_player_guids(
                player.position,
                PLAYER_VISIBILITY_RADIUS_YARDS,
                Some(character_guid),
            ) {
                let Some(observer) = self.players.get(&observer_guid) else {
                    continue;
                };
                for packet in &tick_packets {
                    if let Some(packet) = observer.packet_to_client(packet.clone()) {
                        packets.push(packet);
                    }
                }
                if health_changed {
                    if let Some(packet) = observer.packet_to_client(OutboundWorldPacket {
                        opcode: SMSG_UPDATE_OBJECT,
                        body: build_player_health_update_body(player_guid, player.health)?,
                    }) {
                        packets.push(packet);
                    }
                }
            }
            packets.extend(event.observer_packets);
        }
        Ok(packets)
    }

    pub(in crate::world) fn build_player_aura_update_event(
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
        let world_stats_packet = OutboundWorldPacket {
            opcode: SMSG_UPDATE_OBJECT,
            body: build_player_world_stats_update_body(
                character_guid,
                &player.base_world_stats,
                &player.effective_world_stats,
                player.health,
                player.power1,
            )?,
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
            if let Some(packet) = observer.packet_to_client(aura_packet.clone()) {
                observer_packets.push(packet);
            }
            if let Some(packet) = observer.packet_to_client(combat_stats_packet.clone()) {
                observer_packets.push(packet);
            }
        }

        let mut direct_packets = vec![aura_packet, combat_stats_packet, world_stats_packet];
        direct_packets.extend(build_player_aura_duration_update_packets(
            &player.active_auras,
            now,
        ));

        Ok(PlayerAuraUpdateEvent {
            direct_packets,
            observer_packets,
        })
    }

    pub(in crate::world) fn set_player_auto_attack(
        &mut self,
        character_guid: u32,
        target: Option<ObjectGuid>,
        next_swing_at: Option<Instant>,
    ) {
        if let Some(player) = self.players.get_mut(&character_guid) {
            if target.is_none()
                || player
                    .queued_next_melee_spell
                    .is_some_and(|queued| Some(queued.target) != target)
            {
                player.queued_next_melee_spell = None;
            }
            player.active_combat_target = target;
            player.active_combat_attack_kind = PlayerAutoAttackKind::Melee;
            player.active_combat_next_swing_at = next_swing_at;
        }
    }

    #[cfg(test)]
    pub(in crate::world) fn set_player_ranged_auto_attack(
        &mut self,
        character_guid: u32,
        target: Option<ObjectGuid>,
        next_swing_at: Option<Instant>,
        spell_id: u32,
    ) {
        if let Some(player) = self.players.get_mut(&character_guid) {
            player.queued_next_melee_spell = None;
            player.active_combat_target = target;
            player.active_combat_attack_kind = PlayerAutoAttackKind::Ranged {
                spell_id,
                phase: PlayerRangedAutoAttackPhase::Windup,
            };
            player.active_combat_next_swing_at = next_swing_at;
            player.ranged_auto_attack_next_shot_at = next_swing_at;
        }
    }

    pub(in crate::world) fn set_player_ranged_auto_attack_started(
        &mut self,
        character_guid: u32,
        target: Option<ObjectGuid>,
        requested_next_shot_at: Instant,
        spell_id: u32,
    ) -> Option<Instant> {
        let player = self.players.get_mut(&character_guid)?;
        // CMaNGOS keeps Auto Shot's spell cooldown when CURRENT_AUTOREPEAT_SPELL is interrupted.
        let next_shot_at = player
            .ranged_auto_attack_next_shot_at
            .map(|existing_next_shot_at| existing_next_shot_at.max(requested_next_shot_at))
            .unwrap_or(requested_next_shot_at);
        player.queued_next_melee_spell = None;
        player.active_combat_target = target;
        player.active_combat_attack_kind = PlayerAutoAttackKind::Ranged {
            spell_id,
            phase: PlayerRangedAutoAttackPhase::Windup,
        };
        player.active_combat_next_swing_at = Some(next_shot_at);
        player.ranged_auto_attack_next_shot_at = Some(next_shot_at);
        Some(next_shot_at)
    }

    pub(in crate::world) fn set_player_ranged_next_shot_at(
        &mut self,
        character_guid: u32,
        next_shot_at: Instant,
    ) {
        if let Some(player) = self.players.get_mut(&character_guid) {
            player.ranged_auto_attack_next_shot_at = Some(next_shot_at);
            if let PlayerAutoAttackKind::Ranged { spell_id, .. } = player.active_combat_attack_kind
            {
                player.active_combat_attack_kind = PlayerAutoAttackKind::Ranged {
                    spell_id,
                    phase: PlayerRangedAutoAttackPhase::Windup,
                };
                player.active_combat_next_swing_at = Some(next_shot_at);
            }
        }
    }

    pub(in crate::world) fn stop_player_melee_auto_attack(
        &mut self,
        character_guid: u32,
    ) -> Option<(ObjectGuid, Option<Instant>)> {
        let player = self.players.get_mut(&character_guid)?;
        if player.active_combat_attack_kind != PlayerAutoAttackKind::Melee {
            return None;
        }
        let target = player.active_combat_target?;
        let next_swing_at = player.active_combat_next_swing_at;
        player.active_combat_target = None;
        Some((target, next_swing_at))
    }

    pub(in crate::world) fn player_auto_attack_due(
        &mut self,
        character_guid: u32,
        now: Instant,
    ) -> Option<PlayerAutoAttackDue> {
        if self.active_player_spell_casts.contains_key(&character_guid) {
            return None;
        }
        let player = self.players.get_mut(&character_guid)?;
        let target = player.active_combat_target?;
        let spell_moving = player_is_spell_moving(player);
        if let PlayerAutoAttackKind::Ranged { spell_id, phase } =
            &mut player.active_combat_attack_kind
        {
            let windup = Duration::from_millis(PLAYER_RANGED_AUTO_ATTACK_WINDUP_MILLIS);
            if spell_moving {
                let delayed_shot_at = now + windup;
                if player
                    .active_combat_next_swing_at
                    .is_none_or(|next_shot_at| next_shot_at < delayed_shot_at)
                {
                    player.active_combat_next_swing_at = Some(delayed_shot_at);
                    player.ranged_auto_attack_next_shot_at = Some(delayed_shot_at);
                }
                *phase = PlayerRangedAutoAttackPhase::Windup;
                return None;
            }
            let Some(next_shot_at) = player.active_combat_next_swing_at else {
                let next_shot_at = now + windup;
                player.active_combat_next_swing_at = Some(next_shot_at);
                player.ranged_auto_attack_next_shot_at = Some(next_shot_at);
                *phase = PlayerRangedAutoAttackPhase::Windup;
                return None;
            };
            if now < next_shot_at {
                *phase = PlayerRangedAutoAttackPhase::Windup;
                return None;
            }
            *phase = PlayerRangedAutoAttackPhase::Shooting;
            return Some(PlayerAutoAttackDue {
                target,
                kind: PlayerAutoAttackKind::Ranged {
                    spell_id: *spell_id,
                    phase: PlayerRangedAutoAttackPhase::Shooting,
                },
            });
        }
        player
            .active_combat_next_swing_at
            .is_none_or(|next_swing_at| now >= next_swing_at)
            .then_some(PlayerAutoAttackDue {
                target,
                kind: player.active_combat_attack_kind,
            })
    }

    pub(in crate::world) fn retime_player_auto_attack_after_spell_cast(
        &mut self,
        character_guid: u32,
        now: Instant,
        melee_delay: Duration,
        ranged_windup: Duration,
        cancel_ranged_auto_repeat: bool,
    ) -> PlayerAutoAttackAfterSpellCast {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return PlayerAutoAttackAfterSpellCast::None;
        };
        let Some(target) = player.active_combat_target else {
            return PlayerAutoAttackAfterSpellCast::None;
        };

        match player.active_combat_attack_kind {
            PlayerAutoAttackKind::Melee => {
                let next_swing_at = now + melee_delay;
                player.active_combat_next_swing_at = Some(next_swing_at);
                PlayerAutoAttackAfterSpellCast::MeleeRetimed {
                    target,
                    next_swing_at,
                }
            }
            PlayerAutoAttackKind::Ranged { spell_id, .. } if cancel_ranged_auto_repeat => {
                player.active_combat_target = None;
                player.active_combat_attack_kind = PlayerAutoAttackKind::Melee;
                player.active_combat_next_swing_at = None;
                player.ranged_auto_attack_next_shot_at = None;
                PlayerAutoAttackAfterSpellCast::RangedCanceled { target, spell_id }
            }
            PlayerAutoAttackKind::Ranged { spell_id, .. } => {
                let next_shot_at = player
                    .active_combat_next_swing_at
                    .into_iter()
                    .chain(player.ranged_auto_attack_next_shot_at)
                    .max()
                    .unwrap_or(now)
                    .max(now + ranged_windup);
                player.active_combat_attack_kind = PlayerAutoAttackKind::Ranged {
                    spell_id,
                    phase: PlayerRangedAutoAttackPhase::Windup,
                };
                player.active_combat_next_swing_at = Some(next_shot_at);
                player.ranged_auto_attack_next_shot_at = Some(next_shot_at);
                PlayerAutoAttackAfterSpellCast::RangedRetimed {
                    target,
                    spell_id,
                    next_shot_at,
                }
            }
        }
    }

    pub(in crate::world) fn player_auto_attack_target(
        &self,
        character_guid: u32,
    ) -> Option<ObjectGuid> {
        self.players
            .get(&character_guid)
            .and_then(|player| player.active_combat_target)
    }

    pub(in crate::world) fn set_player_next_swing_at(
        &mut self,
        character_guid: u32,
        next_swing_at: Option<Instant>,
    ) {
        if let Some(player) = self.players.get_mut(&character_guid) {
            player.active_combat_next_swing_at = next_swing_at;
        }
    }

    pub(in crate::world) fn player_selected_target(
        &self,
        character_guid: u32,
    ) -> Option<ObjectGuid> {
        self.players
            .get(&character_guid)
            .and_then(|player| player.selected_target)
    }

    pub(in crate::world) fn set_player_position(
        &mut self,
        character_guid: u32,
        position: WorldPosition,
    ) {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return;
        };
        player.position = position;
        player.cell = cell_coord_for_position(position);
    }

    pub(in crate::world) fn set_player_power2(&mut self, character_guid: u32, power2: u32) {
        if let Some(player) = self.players.get_mut(&character_guid) {
            player.power2 = power2.min(POWER_RAGE_DEFAULT);
        }
    }

    pub(in crate::world) fn player_spell_cast_failure(
        &self,
        character_guid: u32,
        spell_profile: &SpellCastProfile,
        now: Instant,
    ) -> Option<u8> {
        let player = self.players.get(&character_guid)?;
        if self.active_player_spell_casts.contains_key(&character_guid) {
            return Some(SPELL_FAILED_SPELL_IN_PROGRESS);
        }
        if player
            .spell_cooldowns_until
            .get(&spell_profile.spell_id)
            .is_some_and(|until| now < *until)
        {
            return Some(SPELL_FAILED_NOT_READY);
        }
        if player
            .spell_global_cooldowns_until
            .get(&spell_profile.global_cooldown_category)
            .is_some_and(|until| now < *until)
        {
            return Some(SPELL_FAILED_NOT_READY);
        }
        match spell_profile.power {
            SpellPowerCost::Rage { cost } if player.power2 < cost => {
                return Some(SPELL_FAILED_NO_POWER);
            }
            SpellPowerCost::Mana { cost } if player.power1 < cost => {
                return Some(SPELL_FAILED_NO_POWER);
            }
            SpellPowerCost::Energy { cost } if player.power4 < cost => {
                return Some(SPELL_FAILED_NO_POWER);
            }
            _ => {}
        }
        if spell_profile.kind == SpellCastKind::NextMeleeSwing
            && player
                .queued_next_melee_spell
                .is_some_and(|queued| queued.spell_id == spell_profile.spell_id)
        {
            return Some(SPELL_FAILED_NOT_READY);
        }
        None
    }

    pub(in crate::world) fn apply_player_spell_cooldowns(
        &mut self,
        character_guid: u32,
        spell_profile: &SpellCastProfile,
        now: Instant,
        skip_spell_cooldown: bool,
    ) {
        self.apply_player_spell_cooldowns_with_item(
            character_guid,
            spell_profile,
            now,
            skip_spell_cooldown,
            0,
        );
    }

    pub(in crate::world) fn apply_player_spell_cooldowns_with_item(
        &mut self,
        character_guid: u32,
        spell_profile: &SpellCastProfile,
        now: Instant,
        skip_spell_cooldown: bool,
        item_id: u32,
    ) {
        self.apply_player_spell_cooldowns_with_item_category(
            character_guid,
            spell_profile,
            now,
            skip_spell_cooldown,
            item_id,
            0,
            0,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::world) fn apply_player_spell_cooldowns_with_item_category(
        &mut self,
        character_guid: u32,
        spell_profile: &SpellCastProfile,
        now: Instant,
        skip_spell_cooldown: bool,
        item_id: u32,
        category: u32,
        category_cooldown_millis: u64,
    ) {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return;
        };
        if spell_profile.global_cooldown_millis > 0 {
            player.spell_global_cooldowns_until.insert(
                spell_profile.global_cooldown_category,
                now + Duration::from_millis(spell_profile.global_cooldown_millis),
            );
        }
        if !skip_spell_cooldown && spell_profile.cooldown_millis > 0 {
            player.spell_cooldowns_until.insert(
                spell_profile.spell_id,
                now + Duration::from_millis(spell_profile.cooldown_millis),
            );
            if item_id > 0 {
                player
                    .spell_cooldown_item_ids
                    .insert(spell_profile.spell_id, item_id);
            } else {
                player
                    .spell_cooldown_item_ids
                    .remove(&spell_profile.spell_id);
            }
            if category > 0 && category_cooldown_millis > 0 {
                player
                    .spell_cooldown_categories
                    .insert(spell_profile.spell_id, category);
                player.spell_global_cooldowns_until.insert(
                    category,
                    now + Duration::from_millis(category_cooldown_millis),
                );
            } else {
                player
                    .spell_cooldown_categories
                    .remove(&spell_profile.spell_id);
            }
        }
    }

    pub(in crate::world) fn spend_player_spell_power(
        &mut self,
        character_guid: u32,
        spell_profile: &SpellCastProfile,
        now: Instant,
        blocks_mana_regen: bool,
    ) -> Result<(), u8> {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return Ok(());
        };
        match spell_profile.power {
            SpellPowerCost::Rage { cost } if player.power2 < cost => Err(SPELL_FAILED_NO_POWER),
            SpellPowerCost::Mana { cost } if player.power1 < cost => Err(SPELL_FAILED_NO_POWER),
            SpellPowerCost::Energy { cost } if player.power4 < cost => Err(SPELL_FAILED_NO_POWER),
            SpellPowerCost::Rage { cost } => {
                player.power2 = player.power2.saturating_sub(cost);
                Ok(())
            }
            SpellPowerCost::Mana { cost } => {
                player.power1 = player.power1.saturating_sub(cost);
                if cost > 0 && blocks_mana_regen {
                    player.last_mana_use_at = Some(now);
                }
                Ok(())
            }
            SpellPowerCost::Energy { cost } => {
                player.power4 = player.power4.saturating_sub(cost);
                Ok(())
            }
        }
    }

    pub(in crate::world) fn clear_player_spell_recovery(
        &mut self,
        character_guid: u32,
        spell_profile: &SpellCastProfile,
    ) {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return;
        };
        if spell_profile.global_cooldown_millis > 0 {
            player
                .spell_global_cooldowns_until
                .remove(&spell_profile.global_cooldown_category);
        }
        if spell_profile.cooldown_millis > 0 {
            player.spell_cooldowns_until.remove(&spell_profile.spell_id);
            player
                .spell_cooldown_categories
                .remove(&spell_profile.spell_id);
            player
                .spell_cooldown_item_ids
                .remove(&spell_profile.spell_id);
        }
    }

    pub(in crate::world) fn queue_player_next_melee_spell(
        &mut self,
        character_guid: u32,
        queued: QueuedNextMeleeSpell,
    ) {
        if let Some(player) = self.players.get_mut(&character_guid) {
            player.queued_next_melee_spell = Some(queued);
        }
    }

    pub(in crate::world) fn queued_player_next_melee_spell(
        &self,
        character_guid: u32,
        target: ObjectGuid,
    ) -> Option<QueuedNextMeleeSpell> {
        self.players
            .get(&character_guid)
            .and_then(|player| player.queued_next_melee_spell)
            .filter(|queued| queued.target == target)
    }

    pub(in crate::world) fn clear_player_next_melee_spell(&mut self, character_guid: u32) {
        if let Some(player) = self.players.get_mut(&character_guid) {
            player.queued_next_melee_spell = None;
        }
    }

    pub(in crate::world) fn clear_player_melee_state_for_dead_target(
        &mut self,
        target: ObjectGuid,
        exclude_character_guid: Option<u32>,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let affected = self
            .players
            .iter()
            .filter_map(|(character_guid, player)| {
                (player.active_combat_target == Some(target)
                    || player
                        .queued_next_melee_spell
                        .is_some_and(|queued| queued.target == target))
                .then_some(*character_guid)
            })
            .collect::<Vec<_>>();
        let mut packets = Vec::new();
        for character_guid in affected {
            let Some(player) = self.players.get_mut(&character_guid) else {
                continue;
            };
            let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
            let had_active_auto_attack = player.active_combat_target == Some(target);
            if player
                .queued_next_melee_spell
                .is_some_and(|queued| queued.target == target)
            {
                player.queued_next_melee_spell = None;
            }
            if had_active_auto_attack {
                player.active_combat_target = None;
                player.active_combat_next_swing_at = None;
            }
            if exclude_character_guid == Some(character_guid) {
                continue;
            }
            let still_in_combat = player.in_combat;
            let position = player.position;
            let looting = player.looting;
            let mut player_packets = Vec::with_capacity(2);
            if had_active_auto_attack {
                player_packets.push(OutboundWorldPacket {
                    opcode: SMSG_ATTACKSTOP,
                    body: build_attack_stop_body(player_guid, target, false)?,
                });
            }
            if !still_in_combat {
                player_packets.push(OutboundWorldPacket {
                    opcode: SMSG_UPDATE_OBJECT,
                    body: build_unit_flags_update_body(
                        player_guid,
                        player_unit_flags_with_looting(false, looting),
                    )?,
                });
            }
            for recipient_guid in
                self.nearby_player_guids(position, PLAYER_VISIBILITY_RADIUS_YARDS, None)
            {
                let Some(recipient) = self.players.get(&recipient_guid) else {
                    continue;
                };
                packets.extend(
                    player_packets
                        .iter()
                        .cloned()
                        .filter_map(|packet| recipient.packet_to_client(packet)),
                );
            }
        }
        Ok(packets)
    }

    pub(in crate::world) fn spend_queued_player_next_melee_spell_power(
        &mut self,
        character_guid: u32,
        queued: QueuedNextMeleeSpell,
    ) -> Result<(), u8> {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return Ok(());
        };
        if player.power2 < queued.rage_cost || player.power1 < queued.mana_cost {
            player.queued_next_melee_spell = None;
            return Err(SPELL_FAILED_NO_POWER);
        }
        player.queued_next_melee_spell = None;
        player.power2 = player.power2.saturating_sub(queued.rage_cost);
        player.power1 = player.power1.saturating_sub(queued.mana_cost);
        Ok(())
    }

    pub(in crate::world) fn update_player_selection(
        &mut self,
        character_guid: u32,
        selected_target: Option<ObjectGuid>,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return Ok(Vec::new());
        };
        player.selected_target = selected_target;
        player.unit_target = selected_target;
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
                    .and_then(|other| other.packet_to_client(packet.clone()))
            })
            .collect())
    }

    pub(in crate::world) fn update_player_target(
        &mut self,
        character_guid: u32,
        unit_target: Option<ObjectGuid>,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return Ok(Vec::new());
        };
        player.unit_target = unit_target;
        let position = player.position;
        let packet = OutboundWorldPacket {
            opcode: SMSG_UPDATE_OBJECT,
            body: build_player_target_update_body(character_guid, unit_target)?,
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
                    .and_then(|other| other.packet_to_client(packet.clone()))
            })
            .collect())
    }

    pub(in crate::world) fn remove_player(
        &mut self,
        character_guid: u32,
    ) -> Vec<(SessionId, OutboundWorldPacket)> {
        let Some(player) = self.players.remove(&character_guid) else {
            return Vec::new();
        };
        let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
        self.active_player_spell_casts.remove(&character_guid);
        self.pending_spell_events
            .retain(|event| event.caster_character_guid != character_guid);

        let player_grid = grid_coord_for_position(player.position);
        if let Some(grid) = self.grids.get_mut(&player_grid) {
            if player.is_client_controlled() {
                grid.active_player_count = grid.active_player_count.saturating_sub(1);
            }
            grid.last_touched = Instant::now();
            if let Some(cell) = grid.cells.get_mut(&player.cell) {
                cell.players.remove(&character_guid);
                if player.is_client_controlled() {
                    cell.client_players.remove(&character_guid);
                }
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
                .and_then(|other| other.packet_to_client(destroy.clone()))
        })
        .collect()
    }

    pub(in crate::world) fn broadcast_nearby_player_packet(
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
                    .and_then(|other| other.packet_to_client(packet.clone()))
            })
            .collect()
    }

    pub(in crate::world) fn set_player_looting_state(
        &mut self,
        character_guid: u32,
        looting: bool,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return Ok(Vec::new());
        };
        player.looting = looting;
        let player_position = player.position;
        let in_combat = player.in_combat;
        let packet = OutboundWorldPacket {
            opcode: SMSG_UPDATE_OBJECT,
            body: build_unit_flags_update_body(
                ObjectGuid::new(HighGuid::Player, 0, character_guid),
                player_unit_flags_with_looting(in_combat, looting),
            )?,
        };
        Ok(self
            .nearby_player_guids(
                player_position,
                PLAYER_VISIBILITY_RADIUS_YARDS,
                Some(character_guid),
            )
            .into_iter()
            .filter_map(|other_guid| {
                self.players
                    .get(&other_guid)
                    .and_then(|other| other.packet_to_client(packet.clone()))
            })
            .collect())
    }

    pub(in crate::world) fn set_player_stand_state(
        &mut self,
        character_guid: u32,
        stand_state: u8,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return Ok(Vec::new());
        };
        player.stand_state = stand_state;
        let player_position = player.position;
        let packet = OutboundWorldPacket {
            opcode: SMSG_UPDATE_OBJECT,
            body: build_player_stand_state_update_body_for_class(
                character_guid,
                player.class,
                stand_state,
            )?,
        };
        Ok(self
            .nearby_player_guids(
                player_position,
                PLAYER_VISIBILITY_RADIUS_YARDS,
                Some(character_guid),
            )
            .into_iter()
            .filter_map(|other_guid| {
                self.players
                    .get(&other_guid)
                    .and_then(|other| other.packet_to_client(packet.clone()))
            })
            .collect())
    }

    pub(in crate::world) fn set_player_gm_flags(
        &mut self,
        character_guid: u32,
        player_flags: u32,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let Some(player) = self.players.get_mut(&character_guid) else {
            return Ok(Vec::new());
        };
        player.flags = player_flags;
        let player_position = player.position;
        let packet = OutboundWorldPacket {
            opcode: SMSG_UPDATE_OBJECT,
            body: build_player_gm_mode_update_body(
                ObjectGuid::new(HighGuid::Player, 0, character_guid),
                player.race,
                player_flags,
            )?,
        };
        Ok(self
            .nearby_player_guids(
                player_position,
                PLAYER_VISIBILITY_RADIUS_YARDS,
                Some(character_guid),
            )
            .into_iter()
            .filter_map(|other_guid| {
                self.players
                    .get(&other_guid)
                    .and_then(|other| other.packet_to_client(packet.clone()))
            })
            .collect())
    }

    #[cfg(test)]
    pub(in crate::world) fn update_player_db_creature_visibility(
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

pub(in crate::world) fn build_player_root_transition_packet(
    character_guid: u32,
    was_rooted: bool,
    is_rooted: bool,
) -> anyhow::Result<Option<OutboundWorldPacket>> {
    if was_rooted == is_rooted {
        return Ok(None);
    }
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let (opcode, body) = if is_rooted {
        (
            SMSG_FORCE_MOVE_ROOT,
            build_force_move_root_body(player_guid, 0)?,
        )
    } else {
        (
            SMSG_FORCE_MOVE_UNROOT,
            build_force_move_unroot_body(player_guid, 0)?,
        )
    };
    Ok(Some(OutboundWorldPacket { opcode, body }))
}

pub(in crate::world) const DAMAGE_FALL: u8 = 2;
pub(in crate::world) const DAMAGE_EXHAUSTED: u8 = 0;
pub(in crate::world) const DAMAGE_DROWNING: u8 = 1;
pub(in crate::world) const DAMAGE_LAVA: u8 = 3;
pub(in crate::world) const FALL_DAMAGE_MINIMUM_HEIGHT: f32 = 14.57;
pub(in crate::world) const FALL_DAMAGE_DISTANCE_MULTIPLIER: f32 = 0.018;
pub(in crate::world) const FALL_DAMAGE_BASE_PERCENT: f32 = 0.2426;
pub(in crate::world) const ENVIRONMENT_MASK_LIQUID_HAZARD: u32 =
    ENVIRONMENT_FLAG_IN_MAGMA | ENVIRONMENT_FLAG_IN_SLIME;
pub(in crate::world) const MIRROR_TIMER_FATIGUE: u32 = 0;
pub(in crate::world) const MIRROR_TIMER_BREATH: u32 = 1;
pub(in crate::world) const MIRROR_TIMER_ENVIRONMENTAL: u32 = 3;
pub(in crate::world) const MIRROR_TIMER_FATIGUE_MAX_MILLIS: u32 = 60_000;
pub(in crate::world) const MIRROR_TIMER_BREATH_MAX_MILLIS: u32 = 60_000;
pub(in crate::world) const MIRROR_TIMER_ENVIRONMENTAL_MAX_MILLIS: u32 = 1_000;
pub(in crate::world) const MIRROR_TIMER_EXPIRED_PULSE_MILLIS: u32 = 2_000;
pub(in crate::world) const ENVIRONMENTAL_DAMAGE_MIN: u32 = 605;
pub(in crate::world) const ENVIRONMENTAL_DAMAGE_MAX: u32 = 610;

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct PlayerFallUpdate {
    pub(in crate::world) last_fall_z: Option<f32>,
    pub(in crate::world) last_fall_time: u32,
    pub(in crate::world) damage: Option<u32>,
}

pub(in crate::world) fn player_fall_update(
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

    if movement.flags & MOVEFLAG_JUMPING == 0 || movement.fall_time == 0 {
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

pub(in crate::world) fn tracked_player_fall_time(opcode: u16, movement: &MovementInfo) -> u32 {
    if opcode == MSG_MOVE_FALL_LAND as u16 || movement.flags & MOVEFLAG_JUMPING == 0 {
        0
    } else {
        movement.fall_time
    }
}

pub(in crate::world) fn calculate_fall_damage(
    fall_start_z: f32,
    landing_z: f32,
    max_health: u32,
) -> Option<u32> {
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

pub(in crate::world) fn environmental_breath_or_fatigue_damage(max_health: u32, level: u8) -> u32 {
    let variance = if level <= 1 {
        0
    } else {
        rand::thread_rng().gen_range(0..=u32::from(level - 1))
    };
    (max_health.max(1) / 5).saturating_add(variance).max(1)
}

pub(in crate::world) fn environmental_lava_damage() -> u32 {
    rand::thread_rng().gen_range(ENVIRONMENTAL_DAMAGE_MIN..=ENVIRONMENTAL_DAMAGE_MAX)
}

pub(in crate::world) fn update_player_environment_flags(
    player: &mut PlayerRuntime,
    old_flags: u32,
    new_flags: u32,
) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
    if old_flags == new_flags {
        return Ok(Vec::new());
    }
    player.environment.flags = new_flags;
    if (old_flags ^ new_flags) & ENVIRONMENT_FLAG_HIGH_SEA != 0 {
        player.environment.fatigue.scale = if new_flags & ENVIRONMENT_FLAG_HIGH_SEA != 0 {
            -1
        } else {
            10
        };
    }
    if (old_flags ^ new_flags) & ENVIRONMENT_FLAG_UNDERWATER != 0 {
        player.environment.breath.scale = if new_flags & ENVIRONMENT_FLAG_UNDERWATER != 0 {
            -1
        } else {
            10
        };
    }
    if (old_flags ^ new_flags) & ENVIRONMENT_MASK_LIQUID_HAZARD != 0 {
        player.environment.environmental.scale = if new_flags & ENVIRONMENT_MASK_LIQUID_HAZARD != 0
        {
            -1
        } else {
            10
        };
    }

    let mut packets = Vec::new();
    for timer in [player.environment.fatigue, player.environment.breath] {
        if timer.active {
            if let Some(packet) = player.packet_to_client(OutboundWorldPacket {
                opcode: SMSG_START_MIRROR_TIMER,
                body: build_mirror_timer_start_body(
                    timer.timer_type,
                    timer.duration_millis.saturating_sub(timer.elapsed_millis),
                    timer.duration_millis,
                    timer.scale,
                    false,
                    0,
                ),
            }) {
                packets.push(packet);
            }
        }
    }
    Ok(packets)
}

pub(in crate::world) fn player_environment_timer_active(
    player: &PlayerRuntime,
    timer_type: u32,
) -> bool {
    match timer_type {
        MIRROR_TIMER_FATIGUE => player.environment.flags & ENVIRONMENT_FLAG_HIGH_SEA != 0,
        MIRROR_TIMER_BREATH => player.environment.flags & ENVIRONMENT_FLAG_UNDERWATER != 0,
        MIRROR_TIMER_ENVIRONMENTAL => {
            player.environment.flags & ENVIRONMENT_MASK_LIQUID_HAZARD != 0
        }
        _ => false,
    }
}

pub(in crate::world) fn player_environment_timer_deactivated(
    player: &PlayerRuntime,
    timer_type: u32,
) -> bool {
    if player.flags & PLAYER_FLAGS_GHOST != 0 {
        return true;
    }
    match timer_type {
        MIRROR_TIMER_FATIGUE => {
            player.environment.flags & ENVIRONMENT_FLAG_LIQUID == 0 || player.health == 0
        }
        MIRROR_TIMER_BREATH | MIRROR_TIMER_ENVIRONMENTAL => {
            player.environment.flags & ENVIRONMENT_FLAG_LIQUID == 0 || player.health == 0
        }
        _ => true,
    }
}

pub(in crate::world) fn advance_environment_timer(
    timer: &mut MirrorTimerRuntime,
    diff_millis: u32,
    should_activate: bool,
    should_deactivate: bool,
    direct_packets: &mut Vec<(SessionId, OutboundWorldPacket)>,
    session_id: Option<SessionId>,
) -> anyhow::Result<bool> {
    if timer.active || should_activate {
        if should_deactivate {
            stop_mirror_timer(timer, direct_packets, session_id);
            return Ok(false);
        }
        if !timer.active {
            start_mirror_timer(timer, direct_packets, session_id);
        }
    }
    if !timer.active || diff_millis == 0 {
        return Ok(false);
    }

    if timer.scale < 0 {
        let scaled = diff_millis.saturating_mul(timer.scale.unsigned_abs());
        if timer.elapsed_millis < timer.duration_millis {
            timer.elapsed_millis = timer.elapsed_millis.saturating_add(scaled);
            if timer.elapsed_millis >= timer.duration_millis {
                timer.elapsed_millis = timer.duration_millis;
                timer.pulse_millis = 0;
                return Ok(true);
            }
            return Ok(false);
        }

        timer.pulse_millis = timer.pulse_millis.saturating_add(scaled);
        if timer.pulse_millis >= MIRROR_TIMER_EXPIRED_PULSE_MILLIS {
            timer.pulse_millis %= MIRROR_TIMER_EXPIRED_PULSE_MILLIS;
            return Ok(true);
        }
    } else if timer.scale > 0 {
        let scaled = diff_millis.saturating_mul(timer.scale.unsigned_abs());
        if scaled >= timer.elapsed_millis {
            stop_mirror_timer(timer, direct_packets, session_id);
        } else {
            timer.elapsed_millis -= scaled;
            if timer.timer_type < MIRROR_TIMER_ENVIRONMENTAL {
                if let Some(session_id) = session_id {
                    direct_packets.push((
                        session_id,
                        OutboundWorldPacket {
                            opcode: SMSG_START_MIRROR_TIMER,
                            body: build_mirror_timer_start_body(
                                timer.timer_type,
                                timer.duration_millis.saturating_sub(timer.elapsed_millis),
                                timer.duration_millis,
                                timer.scale,
                                false,
                                0,
                            ),
                        },
                    ));
                }
            }
        }
    }
    Ok(false)
}

pub(in crate::world) fn start_mirror_timer(
    timer: &mut MirrorTimerRuntime,
    direct_packets: &mut Vec<(SessionId, OutboundWorldPacket)>,
    session_id: Option<SessionId>,
) {
    if timer.scale >= 0 {
        return;
    }
    timer.active = true;
    timer.elapsed_millis = 0;
    timer.pulse_millis = 0;
    if timer.timer_type < MIRROR_TIMER_ENVIRONMENTAL {
        if let Some(session_id) = session_id {
            direct_packets.push((
                session_id,
                OutboundWorldPacket {
                    opcode: SMSG_START_MIRROR_TIMER,
                    body: build_mirror_timer_start_body(
                        timer.timer_type,
                        timer.duration_millis,
                        timer.duration_millis,
                        timer.scale,
                        false,
                        0,
                    ),
                },
            ));
        }
    }
}

pub(in crate::world) fn stop_mirror_timer(
    timer: &mut MirrorTimerRuntime,
    direct_packets: &mut Vec<(SessionId, OutboundWorldPacket)>,
    session_id: Option<SessionId>,
) {
    if !timer.active {
        return;
    }
    timer.active = false;
    timer.elapsed_millis = 0;
    timer.pulse_millis = 0;
    if timer.timer_type < MIRROR_TIMER_ENVIRONMENTAL {
        if let Some(session_id) = session_id {
            direct_packets.push((
                session_id,
                OutboundWorldPacket {
                    opcode: SMSG_STOP_MIRROR_TIMER,
                    body: timer.timer_type.to_le_bytes().to_vec(),
                },
            ));
        }
    }
}

pub(in crate::world) fn build_mirror_timer_start_body(
    timer_type: u32,
    remaining_millis: u32,
    duration_millis: u32,
    scale: i32,
    paused: bool,
    spell_id: u32,
) -> Vec<u8> {
    let mut body = Vec::with_capacity(21);
    body.extend_from_slice(&timer_type.to_le_bytes());
    body.extend_from_slice(&remaining_millis.to_le_bytes());
    body.extend_from_slice(&duration_millis.to_le_bytes());
    body.extend_from_slice(&scale.to_le_bytes());
    body.push(paused as u8);
    body.extend_from_slice(&spell_id.to_le_bytes());
    body
}

pub(in crate::world) fn health_regen_per_second_for_spirit(class: u8, spirit: u32) -> f32 {
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

pub(in crate::world) fn mana_regen_per_second_for_spirit(class: u8, spirit: u32) -> f32 {
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

pub(in crate::world) fn refresh_player_runtime_stats_from_auras(player: &mut PlayerRuntime) {
    let was_dead = player.health == 0;
    player.effective_world_stats =
        player_world_stats_with_active_auras(player.base_world_stats, &player.active_auras);
    player.spirit = player.effective_world_stats.stats[4];
    player.max_health = player.effective_world_stats.max_health().max(1);
    player.health = if was_dead {
        0
    } else {
        player.health.max(1).min(player.max_health)
    };
    player.max_power1 = player.effective_world_stats.max_mana();
    player.power1 = player.power1.min(player.max_power1);
}

pub(in crate::world) fn player_is_spell_moving(player: &PlayerRuntime) -> bool {
    player.movement_flags & MOVEFLAG_MASK_SPELL_MOVING != 0
}

pub(in crate::world) fn should_rescan_visibility_from(
    previous: Option<WorldPosition>,
    position: WorldPosition,
) -> bool {
    let Some(previous) = previous else {
        return true;
    };
    if previous.map_id != position.map_id {
        return true;
    }
    distance_squared_2d(previous.x, previous.y, position.x, position.y)
        >= CREATURE_VISIBILITY_RESCAN_DISTANCE_YARDS * CREATURE_VISIBILITY_RESCAN_DISTANCE_YARDS
}

pub(in crate::world) fn moving_bot_start_packet(
    player: &PlayerRuntime,
) -> anyhow::Result<Option<OutboundWorldPacket>> {
    let Some(bot) = player.bot_runtime.as_ref() else {
        return Ok(None);
    };
    if bot.active_leg.is_none() || player.movement_flags & MOVEFLAG_FORWARD == 0 {
        return Ok(None);
    }
    Ok(Some(OutboundWorldPacket {
        opcode: MSG_MOVE_START_FORWARD as u16,
        body: build_player_movement_broadcast_body(
            player.guid,
            &MovementInfo {
                flags: player.movement_flags,
                client_time: player.client_time,
                position: player.position,
                fall_time: player.fall_time,
                jump: player.jump.clone(),
            },
            player.server_time,
        )?,
    }))
}
