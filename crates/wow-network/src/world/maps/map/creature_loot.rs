// Shared DB-creature loot claim/release authority.

impl MapRuntime {
    fn open_db_creature_loot(
        &mut self,
        creature_guid: u64,
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
        Some(creature.clone())
    }

    fn take_db_creature_loot_money(
        &mut self,
        creature_guid: u64,
    ) -> Option<(u32, DbCreatureRuntime)> {
        let creature = self.creatures.get_mut(&creature_guid)?;
        if !creature.looting || !creature.loot_money_available {
            return None;
        }
        let money = creature.loot_money();
        creature.loot_money_available = false;
        Some((money, creature.clone()))
    }

    fn take_db_creature_loot_item(
        &mut self,
        creature_guid: u64,
    ) -> Option<(DbCreatureLootRuntime, DbCreatureRuntime)> {
        let creature = self.creatures.get_mut(&creature_guid)?;
        if !creature.looting {
            return None;
        }
        let loot = creature.loot_item.take()?;
        Some((loot, creature.clone()))
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
        Some(creature.clone())
    }

    fn release_db_creature_loot(
        &mut self,
        creature_guid: u64,
        now: Instant,
    ) -> Option<DbCreatureRuntime> {
        let creature = self.creatures.get_mut(&creature_guid)?;
        creature.looting = false;
        creature.reduce_corpse_decay_after_loot(now);
        Some(creature.clone())
    }
}
