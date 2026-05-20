use super::*;
use wow_proto::world::WorldOpcode;

// Shared DB-creature corpse expiry, respawn, and lifecycle packet production.

impl MapRuntime {
    pub(in crate::world) fn clear_db_creature_lifecycle_tracking(&mut self, guid: u64) {
        self.db_creature_corpse_expiry_due_at.remove(&guid);
        self.db_creature_respawn_due_at.remove(&guid);
    }

    pub(in crate::world) fn sync_db_creature_lifecycle_tracking(&mut self, guid: u64) {
        self.clear_db_creature_lifecycle_tracking(guid);
        let Some(creature) = self.creatures.get(&guid) else {
            return;
        };
        if creature.life_state == DbCreatureLifeState::Corpse {
            if let Some(due_at) = creature.corpse_expires_at {
                self.db_creature_corpse_expiry_due_at.insert(guid, due_at);
                self.db_creature_corpse_expiries
                    .push(Reverse(ScheduledDbCreatureLifecycle { due_at, guid }));
            }
            return;
        }
        if creature.life_state == DbCreatureLifeState::Dead {
            if let Some(due_at) = creature.respawn_at {
                self.db_creature_respawn_due_at.insert(guid, due_at);
                self.db_creature_respawns
                    .push(Reverse(ScheduledDbCreatureLifecycle { due_at, guid }));
            }
        }
    }

    fn ready_db_creature_corpse_expiry_guids(&mut self, now: Instant) -> Vec<u64> {
        let mut guids = Vec::new();
        while self
            .db_creature_corpse_expiries
            .peek()
            .is_some_and(|entry| entry.0.due_at <= now)
        {
            let Some(Reverse(entry)) = self.db_creature_corpse_expiries.pop() else {
                break;
            };
            if self
                .db_creature_corpse_expiry_due_at
                .get(&entry.guid)
                .copied()
                != Some(entry.due_at)
            {
                continue;
            }
            self.db_creature_corpse_expiry_due_at.remove(&entry.guid);
            if self
                .creatures
                .get(&entry.guid)
                .is_some_and(|creature| creature.is_corpse_expired(now))
            {
                guids.push(entry.guid);
            } else {
                self.sync_db_creature_lifecycle_tracking(entry.guid);
            }
        }
        guids
    }

    fn ready_db_creature_respawn_guids(&mut self, now: Instant) -> Vec<u64> {
        let mut guids = Vec::new();
        while self
            .db_creature_respawns
            .peek()
            .is_some_and(|entry| entry.0.due_at <= now)
        {
            let Some(Reverse(entry)) = self.db_creature_respawns.pop() else {
                break;
            };
            if self.db_creature_respawn_due_at.get(&entry.guid).copied() != Some(entry.due_at) {
                continue;
            }
            self.db_creature_respawn_due_at.remove(&entry.guid);
            if self
                .creatures
                .get(&entry.guid)
                .is_some_and(|creature| creature.is_ready_to_respawn(now))
            {
                guids.push(entry.guid);
            } else {
                self.sync_db_creature_lifecycle_tracking(entry.guid);
            }
        }
        guids
    }

    #[cfg(test)]
    pub(in crate::world) fn loaded_db_creature_lifecycle_guids(
        &mut self,
        now: Instant,
    ) -> Vec<u64> {
        let mut guids = self.ready_db_creature_corpse_expiry_guids(now);
        guids.extend(self.ready_db_creature_respawn_guids(now));
        guids.sort_unstable();
        guids.dedup();
        guids
    }

    fn expire_db_creature_corpse(&mut self, guid: u64, now: Instant) -> Option<DbCreatureRuntime> {
        let creature_guid = ObjectGuid::from_raw(guid);
        let old_position = {
            let creature = self.creatures.get_mut(&guid)?;
            if !creature.is_corpse_expired(now) {
                self.sync_db_creature_lifecycle_tracking(guid);
                return None;
            }
            let old_position = creature.current_position;
            creature.remove_corpse();
            old_position
        };
        self.refresh_db_creature_spatial_index(
            guid,
            old_position,
            self.creatures.get(&guid)?.current_position,
        );
        self.sync_db_creature_lifecycle_tracking(guid);
        self.sync_db_creature_ooc_event_ai_tracking(guid, now);
        self.creatures.get(&creature_guid.raw()).cloned()
    }

    fn respawn_db_creature(&mut self, guid: u64, now: Instant) -> Option<DbCreatureRuntime> {
        let creature_guid = ObjectGuid::from_raw(guid);
        let old_position = {
            let creature = self.creatures.get_mut(&guid)?;
            if !creature.is_ready_to_respawn(now) {
                self.sync_db_creature_lifecycle_tracking(guid);
                return None;
            }
            let old_position = creature.current_position;
            creature.respawn(now);
            old_position
        };
        self.invalidate_idle_motion_start_schedule();
        self.refresh_db_creature_spatial_index(
            guid,
            old_position,
            self.creatures.get(&guid)?.current_position,
        );
        self.sync_db_creature_lifecycle_tracking(guid);
        self.sync_db_creature_ooc_event_ai_tracking(guid, now);
        self.creatures.get(&creature_guid.raw()).cloned()
    }

    pub(in crate::world) fn advance_db_creature_lifecycle_tick(
        &mut self,
        now: Instant,
    ) -> anyhow::Result<DbCreatureLifecycleTick> {
        let mut tick = DbCreatureLifecycleTick::default();
        let corpse_guids = self.ready_db_creature_corpse_expiry_guids(now);
        for guid in corpse_guids {
            let Some(creature) = self.expire_db_creature_corpse(guid, now) else {
                continue;
            };
            let creature_guid = creature.guid();
            let packet = OutboundWorldPacket {
                opcode: WorldOpcode::SmsgDestroyObject as u16,
                body: build_destroy_guid_body(creature_guid),
            };
            for player in self.players.values_mut() {
                if !player.visible_objects.remove(&creature_guid) {
                    continue;
                }
                if let Some(packet) = player.packet_to_client(packet.clone()) {
                    tick.packets.push(packet);
                }
            }
        }

        let respawn_guids = self.ready_db_creature_respawn_guids(now);
        for guid in respawn_guids {
            let Some(creature) = self.respawn_db_creature(guid, now) else {
                continue;
            };
            let creature_guid = creature.guid();
            for player in self.players.values_mut() {
                let player_is_ghost = player.flags & PLAYER_FLAGS_GHOST != 0;
                if !Self::db_creature_visible_for_player_death_state(&creature, player_is_ghost)
                    || !is_db_creature_inside_visibility_radius(&creature, player.position)
                {
                    continue;
                }
                if !player.visible_objects.insert(creature_guid) {
                    continue;
                }
                let packet = OutboundWorldPacket {
                    opcode: WorldOpcode::SmsgUpdateObject as u16,
                    body: build_update_object_body(&[
                        build_db_creature_runtime_create_block_for_player(
                            &creature,
                            Some(player.guid),
                        )?,
                    ]),
                };
                if let Some(packet) = player.packet_to_client(packet) {
                    tick.packets.push(packet);
                }
            }
            tick.respawn_updates
                .push(DbCreatureRespawnPersistenceUpdate {
                    creature_spawn_guid: creature.spawn.guid,
                });
        }
        Ok(tick)
    }

    #[cfg(test)]
    pub(in crate::world) fn advance_db_creature_lifecycle(
        &mut self,
        creature_guids: &[u64],
        viewer_position: WorldPosition,
        exclude_character_guid: Option<u32>,
        now: Instant,
    ) -> anyhow::Result<Vec<DbCreatureLifecycleEvent>> {
        let mut events = Vec::new();
        for guid in creature_guids {
            if let Some(creature) = self.expire_db_creature_corpse(*guid, now) {
                let creature_guid = creature.guid();
                let packet = OutboundWorldPacket {
                    opcode: WorldOpcode::SmsgDestroyObject as u16,
                    body: build_destroy_guid_body(creature_guid),
                };
                let mut direct_packets = Vec::new();
                let mut observer_packets = Vec::new();
                for player in self.players.values_mut() {
                    if !player.visible_objects.remove(&creature_guid) {
                        continue;
                    }
                    if Some(player.guid) == exclude_character_guid {
                        direct_packets.push(packet.clone());
                    } else if let Some(packet) = player.packet_to_client(packet.clone()) {
                        observer_packets.push(packet);
                    }
                }
                events.push(DbCreatureLifecycleEvent {
                    creature,
                    direct_packets,
                    observer_packets,
                    clear_respawn_guid: None,
                });
                continue;
            }

            if let Some(creature) = self.respawn_db_creature(*guid, now) {
                let should_send_create =
                    is_db_creature_inside_visibility_radius(&creature, viewer_position);
                let creature_guid = creature.guid();
                let clear_respawn_guid = creature.spawn.guid;
                let mut direct_packets = Vec::new();
                let mut observer_packets = Vec::new();
                for player in self.players.values_mut() {
                    let player_is_ghost = player.flags & PLAYER_FLAGS_GHOST != 0;
                    if !Self::db_creature_visible_for_player_death_state(&creature, player_is_ghost)
                        || !is_db_creature_inside_visibility_radius(&creature, player.position)
                    {
                        continue;
                    }
                    if !player.visible_objects.insert(creature_guid) {
                        continue;
                    }
                    let packet = OutboundWorldPacket {
                        opcode: WorldOpcode::SmsgUpdateObject as u16,
                        body: build_update_object_body(&[
                            build_db_creature_runtime_create_block_for_player(
                                &creature,
                                Some(player.guid),
                            )?,
                        ]),
                    };
                    if Some(player.guid) == exclude_character_guid && should_send_create {
                        direct_packets.push(packet);
                    } else if let Some(packet) = player.packet_to_client(packet) {
                        observer_packets.push(packet);
                    }
                }
                events.push(DbCreatureLifecycleEvent {
                    creature,
                    direct_packets,
                    observer_packets,
                    clear_respawn_guid: Some(clear_respawn_guid),
                });
            }
        }
        Ok(events)
    }
}
