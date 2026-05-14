use super::*;

// Shared DB-creature corpse expiry, respawn, and lifecycle packet production.

impl MapRuntime {
    pub(in crate::world) fn loaded_db_creature_lifecycle_guids(&self, now: Instant) -> Vec<u64> {
        let mut guids = self
            .creatures
            .iter()
            .filter_map(|(guid, creature)| {
                (creature.is_corpse_expired(now) || creature.is_ready_to_respawn(now))
                    .then_some(*guid)
            })
            .collect::<Vec<_>>();
        guids.sort_unstable();
        guids
    }

    pub(in crate::world) fn advance_db_creature_lifecycle(
        &mut self,
        creature_guids: &[u64],
        viewer_position: WorldPosition,
        exclude_character_guid: Option<u32>,
        now: Instant,
    ) -> anyhow::Result<Vec<DbCreatureLifecycleEvent>> {
        let mut events = Vec::new();
        for guid in creature_guids {
            let Some(creature) = self.creatures.get_mut(guid) else {
                continue;
            };
            let creature_guid = creature.guid();
            if creature.is_corpse_expired(now) {
                let old_position = creature.current_position;
                creature.remove_corpse();
                let creature = creature.clone();
                self.refresh_db_creature_spatial_index(
                    creature_guid.raw(),
                    old_position,
                    creature.current_position,
                );
                let packet = OutboundWorldPacket {
                    opcode: SMSG_DESTROY_OBJECT,
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

            if creature.is_ready_to_respawn(now) {
                let old_position = creature.current_position;
                creature.respawn(now);
                let creature = creature.clone();
                self.invalidate_idle_motion_start_schedule();
                self.refresh_db_creature_spatial_index(
                    creature_guid.raw(),
                    old_position,
                    creature.current_position,
                );
                let should_send_create =
                    is_db_creature_inside_visibility_radius(&creature, viewer_position);
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
                        opcode: SMSG_UPDATE_OBJECT,
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
                    clear_respawn_guid: Some(creature_guid.counter()),
                });
            }
        }
        Ok(events)
    }
}
