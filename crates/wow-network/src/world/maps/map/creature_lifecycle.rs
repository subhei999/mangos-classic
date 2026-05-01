// Shared DB-creature corpse expiry, respawn, and lifecycle packet production.

impl MapRuntime {
    fn advance_db_creature_lifecycle(
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
                creature.remove_corpse();
                let creature = creature.clone();
                let packet = OutboundWorldPacket {
                    opcode: SMSG_DESTROY_OBJECT,
                    body: build_destroy_guid_body(creature_guid),
                };
                let observer_packets = self
                    .nearby_player_guids(
                        creature.current_position,
                        CREATURE_SPAWN_RADIUS_YARDS,
                        exclude_character_guid,
                    )
                    .into_iter()
                    .filter_map(|player_guid| {
                        self.players
                            .get(&player_guid)
                            .map(|player| (player.session_id, packet.clone()))
                    })
                    .collect();
                events.push(DbCreatureLifecycleEvent {
                    creature,
                    direct_packets: vec![packet],
                    observer_packets,
                    clear_respawn_guid: None,
                });
                continue;
            }

            if creature.is_ready_to_respawn(now) {
                creature.respawn();
                let creature = creature.clone();
                let should_send_create =
                    is_db_creature_inside_visibility_radius(&creature, viewer_position);
                let create_packet = if should_send_create {
                    Some(OutboundWorldPacket {
                        opcode: SMSG_UPDATE_OBJECT,
                        body: build_update_object_body(&[build_db_creature_runtime_create_block(
                            &creature,
                        )?]),
                    })
                } else {
                    None
                };
                let observer_packet = OutboundWorldPacket {
                    opcode: SMSG_UPDATE_OBJECT,
                    body: build_update_object_body(&[build_db_creature_runtime_create_block(
                        &creature,
                    )?]),
                };
                let observer_packets = self
                    .nearby_player_guids(
                        creature.current_position,
                        CREATURE_SPAWN_RADIUS_YARDS,
                        exclude_character_guid,
                    )
                    .into_iter()
                    .filter_map(|player_guid| {
                        self.players
                            .get(&player_guid)
                            .map(|player| (player.session_id, observer_packet.clone()))
                    })
                    .collect();
                events.push(DbCreatureLifecycleEvent {
                    creature,
                    direct_packets: create_packet.into_iter().collect(),
                    observer_packets,
                    clear_respawn_guid: Some(creature_guid.counter()),
                });
            }
        }
        Ok(events)
    }
}
