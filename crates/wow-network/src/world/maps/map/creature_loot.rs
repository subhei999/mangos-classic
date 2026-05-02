// Shared DB-creature loot claim/release authority.

impl MapRuntime {
    fn db_creature_needs_loot_item(&self, creature_guid: u64) -> Option<bool> {
        let creature = self.creatures.get(&creature_guid)?;
        creature.lootable.then_some(creature.loot_item.is_none())
    }

    fn open_db_creature_loot(
        &mut self,
        creature_guid: u64,
        character_guid: u32,
        loot_item: Option<DbCreatureLootRuntime>,
    ) -> Option<DbCreatureRuntime> {
        let creature = self.creatures.get_mut(&creature_guid)?;
        if !creature.lootable {
            return None;
        }
        if creature.loot_item.is_none() {
            creature.loot_item = loot_item;
        }
        creature.looting = true;
        self.creature_looting_by_character
            .insert(character_guid, creature_guid);
        let creature = creature.clone();
        self.refresh_grid_state(grid_coord_for_position(creature.current_position));
        Some(creature)
    }

    fn db_creature_loot_guid_for_character(&self, character_guid: u32) -> Option<u64> {
        self.creature_looting_by_character
            .get(&character_guid)
            .copied()
    }

    fn take_db_creature_loot_money(
        &mut self,
        character_guid: u32,
    ) -> Option<(u32, DbCreatureRuntime)> {
        let creature_guid = self.db_creature_loot_guid_for_character(character_guid)?;
        let creature = self.creatures.get_mut(&creature_guid)?;
        if !creature.looting || !creature.loot_money_available {
            return None;
        }
        let money = creature.loot_money();
        creature.loot_money_available = false;
        let creature = creature.clone();
        self.refresh_grid_state(grid_coord_for_position(creature.current_position));
        Some((money, creature))
    }

    fn take_db_creature_loot_item(
        &mut self,
        character_guid: u32,
    ) -> Option<(u64, DbCreatureLootRuntime, DbCreatureRuntime)> {
        let creature_guid = self.db_creature_loot_guid_for_character(character_guid)?;
        let creature = self.creatures.get_mut(&creature_guid)?;
        if !creature.looting {
            return None;
        }
        let loot = creature.loot_item.take()?;
        let creature = creature.clone();
        self.refresh_grid_state(grid_coord_for_position(creature.current_position));
        Some((creature_guid, loot, creature))
    }

    fn restore_db_creature_loot_item(
        &mut self,
        creature_guid: u64,
        loot: DbCreatureLootRuntime,
    ) -> Option<DbCreatureRuntime> {
        let creature = self.creatures.get_mut(&creature_guid)?;
        if creature.loot_item.is_none() {
            creature.loot_item = Some(loot);
        }
        let creature = creature.clone();
        self.refresh_grid_state(grid_coord_for_position(creature.current_position));
        Some(creature)
    }

    fn release_db_creature_loot(
        &mut self,
        creature_guid: u64,
        now: Instant,
        exclude_character_guid: Option<u32>,
    ) -> anyhow::Result<Option<DbCreatureLootReleaseEvent>> {
        let Some(creature) = self.creatures.get_mut(&creature_guid) else {
            return Ok(None);
        };
        creature.looting = false;
        self.creature_looting_by_character
            .retain(|_, looting_guid| *looting_guid != creature_guid);
        creature.reduce_corpse_decay_after_loot(now);
        let creature = creature.clone();
        self.refresh_grid_state(grid_coord_for_position(creature.current_position));
        let direct_packet = OutboundWorldPacket {
            opcode: SMSG_UPDATE_OBJECT,
            body: build_db_creature_state_update_body(
                creature.guid(),
                creature.health,
                creature.dynamic_flags(),
            )?,
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
                    .map(|player| (player.session_id, direct_packet.clone()))
            })
            .collect();
        Ok(Some(DbCreatureLootReleaseEvent {
            creature,
            direct_packet,
            observer_packets,
        }))
    }
}
