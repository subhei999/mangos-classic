// Shared DB-creature loot claim/release authority.

impl MapRuntime {
    fn db_creature_needs_loot_item(&self, creature_guid: u64) -> Option<bool> {
        let creature = self.creatures.get(&creature_guid)?;
        creature.lootable.then_some(!creature.loot_items_generated)
    }

    fn open_db_creature_loot(
        &mut self,
        creature_guid: u64,
        character_guid: u32,
        access_owner: CreatureLootOwner,
        current_looter: Option<u32>,
        loot_items: Vec<DbCreatureLootRuntime>,
    ) -> Option<DbCreatureRuntime> {
        let creature = self.creatures.get_mut(&creature_guid)?;
        if !creature.lootable {
            return None;
        }
        if !creature_loot_owner_allows(creature.loot_owner, access_owner, character_guid) {
            return None;
        }
        if creature.loot_owner.is_none() {
            creature.loot_owner = Some(access_owner);
        }
        if creature.loot_current_looter.is_none() {
            creature.loot_current_looter = current_looter.or(Some(character_guid));
        }
        if !creature.loot_items_generated {
            creature.loot_items = loot_items_with_stable_slots(loot_items);
            creature.loot_roll_released_slots.clear();
            creature.loot_current_looter_pass_slots.clear();
            creature.loot_items_generated = true;
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

    fn db_creature_looting_characters(&self, creature_guid: u64) -> Vec<u32> {
        self.creature_looting_by_character
            .iter()
            .filter_map(|(character_guid, looting_guid)| {
                (*looting_guid == creature_guid).then_some(*character_guid)
            })
            .collect()
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
        loot_slot: u8,
    ) -> Option<(u64, u8, DbCreatureLootRuntime, DbCreatureRuntime)> {
        let creature_guid = self.db_creature_loot_guid_for_character(character_guid)?;
        let creature = self.creatures.get_mut(&creature_guid)?;
        if !creature.looting {
            return None;
        }
        let slot = creature
            .loot_items
            .iter()
            .position(|loot| loot.slot == loot_slot)?;
        let loot = creature.loot_items.remove(slot);
        let creature_snapshot = creature.clone();
        creature.loot_roll_released_slots.remove(&loot_slot);
        creature.loot_current_looter_pass_slots.remove(&loot_slot);
        self.refresh_grid_state(grid_coord_for_position(creature_snapshot.current_position));
        Some((creature_guid, loot_slot, loot, creature_snapshot))
    }

    fn take_db_creature_loot_item_by_guid(
        &mut self,
        creature_guid: u64,
        loot_slot: u8,
    ) -> Option<(u8, DbCreatureLootRuntime, DbCreatureRuntime)> {
        let creature = self.creatures.get_mut(&creature_guid)?;
        if !creature.lootable {
            return None;
        }
        let slot = creature
            .loot_items
            .iter()
            .position(|loot| loot.slot == loot_slot)?;
        let loot = creature.loot_items.remove(slot);
        creature.loot_roll_released_slots.remove(&loot_slot);
        creature.loot_current_looter_pass_slots.remove(&loot_slot);
        let creature = creature.clone();
        self.refresh_grid_state(grid_coord_for_position(creature.current_position));
        Some((loot_slot, loot, creature))
    }

    fn restore_db_creature_loot_item(
        &mut self,
        creature_guid: u64,
        loot_slot: u8,
        loot: DbCreatureLootRuntime,
    ) -> Option<DbCreatureRuntime> {
        let creature = self.creatures.get_mut(&creature_guid)?;
        let mut loot = loot;
        loot.slot = loot_slot;
        creature.loot_items.push(loot);
        creature.loot_items.sort_by_key(|loot| loot.slot);
        let creature = creature.clone();
        self.refresh_grid_state(grid_coord_for_position(creature.current_position));
        Some(creature)
    }

    fn release_db_creature_loot_roll_item(
        &mut self,
        creature_guid: u64,
        loot_slot: u8,
    ) -> Option<DbCreatureRuntime> {
        let creature = self.creatures.get_mut(&creature_guid)?;
        if !creature.loot_items.iter().any(|loot| loot.slot == loot_slot) {
            return None;
        }
        creature.loot_roll_released_slots.insert(loot_slot);
        let creature = creature.clone();
        self.refresh_grid_state(grid_coord_for_position(creature.current_position));
        Some(creature)
    }

    fn release_db_creature_current_looter_pass_item(
        &mut self,
        creature_guid: u64,
        loot_slot: u8,
    ) -> Option<DbCreatureRuntime> {
        let creature = self.creatures.get_mut(&creature_guid)?;
        if !creature.loot_items.iter().any(|loot| loot.slot == loot_slot) {
            return None;
        }
        creature.loot_current_looter_pass_slots.insert(loot_slot);
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
        if exclude_character_guid.is_some() && creature.loot_current_looter == exclude_character_guid
        {
            let releasable_slots: Vec<u8> = creature
                .loot_items
                .iter()
                .filter(|loot| db_creature_loot_release_marks_current_looter_pass(creature, loot))
                .map(|loot| loot.slot)
                .collect();
            creature
                .loot_current_looter_pass_slots
                .extend(releasable_slots);
        }
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
                creature.dynamic_flags_for_player(exclude_character_guid),
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
                let player = self.players.get(&player_guid)?;
                let body = build_db_creature_state_update_body(
                    creature.guid(),
                    creature.health,
                    creature.dynamic_flags_for_player(Some(player_guid)),
                )
                .ok()?;
                Some((
                    player.session_id,
                    OutboundWorldPacket {
                        opcode: SMSG_UPDATE_OBJECT,
                        body,
                    },
                ))
            })
            .collect();
        Ok(Some(DbCreatureLootReleaseEvent {
            creature,
            direct_packet,
            observer_packets,
        }))
    }

    fn set_db_creature_loot_owner(
        &mut self,
        creature_guid: ObjectGuid,
        owner: CreatureLootOwner,
    ) -> Option<DbCreatureRuntime> {
        let creature = self.creatures.get_mut(&creature_guid.raw())?;
        if creature.loot_owner.is_none() {
            creature.loot_owner = Some(owner);
        }
        Some(creature.clone())
    }

    fn force_db_creature_loot_owner(
        &mut self,
        creature_guid: ObjectGuid,
        owner: CreatureLootOwner,
    ) -> Option<DbCreatureRuntime> {
        let creature = self.creatures.get_mut(&creature_guid.raw())?;
        creature.loot_owner = Some(owner);
        Some(creature.clone())
    }
}

fn db_creature_loot_release_marks_current_looter_pass(
    creature: &DbCreatureRuntime,
    loot: &DbCreatureLootRuntime,
) -> bool {
    if loot.free_for_all || creature.loot_roll_released_slots.contains(&loot.slot) {
        return false;
    }
    let Some(loot_method) = creature.loot_method else {
        return false;
    };
    let under_threshold = loot.quest_drop || loot.quality < loot_method.threshold;
    matches!(loot_method.method, 1..=4) && under_threshold
}

fn creature_loot_owner_allows(
    current_owner: Option<CreatureLootOwner>,
    access_owner: CreatureLootOwner,
    character_guid: u32,
) -> bool {
    match current_owner {
        None => true,
        Some(CreatureLootOwner::Player(owner)) => owner == character_guid,
        Some(CreatureLootOwner::Party(owner)) => access_owner == CreatureLootOwner::Party(owner),
    }
}

fn loot_items_with_stable_slots(
    loot_items: Vec<DbCreatureLootRuntime>,
) -> Vec<DbCreatureLootRuntime> {
    const CMANGOS_MAX_NR_LOOT_ITEMS: usize = 16;
    loot_items
        .into_iter()
        .take(CMANGOS_MAX_NR_LOOT_ITEMS)
        .enumerate()
        .map(|(index, mut loot)| {
            loot.slot = index.min(u8::MAX as usize) as u8;
            loot
        })
        .collect()
}
