// Shared DB-gameobject loot open/take/restore/release authority.

impl MapRuntime {
    fn open_db_gameobject_loot(
        &mut self,
        gameobject_guid: u64,
        character_guid: u32,
        loot_items: Vec<DbCreatureLootRuntime>,
    ) -> Option<(DbGameObjectRuntime, Vec<DbCreatureLootRuntime>)> {
        let gameobject = self.gameobjects.get(&gameobject_guid)?.clone();
        if !gameobject.client_visible {
            return None;
        }

        if let Some(previous_guid) = self
            .gameobject_looting_by_character
            .insert(character_guid, gameobject_guid)
        {
            if previous_guid != gameobject_guid {
                self.remove_db_gameobject_looter(previous_guid, character_guid);
            }
        }

        let state = self.gameobject_loots.entry(gameobject_guid).or_default();
        if state.loot_items.is_empty() {
            state.loot_items = loot_items;
        }
        state.open_characters.insert(character_guid);

        Some((gameobject, state.loot_items.clone()))
    }

    fn db_gameobject_loot_guid_for_character(&self, character_guid: u32) -> Option<u64> {
        self.gameobject_looting_by_character
            .get(&character_guid)
            .copied()
    }

    fn take_db_gameobject_loot_item(
        &mut self,
        character_guid: u32,
        loot_slot: u8,
    ) -> Option<(u64, u8, DbCreatureLootRuntime)> {
        let gameobject_guid = self.db_gameobject_loot_guid_for_character(character_guid)?;
        let state = self.gameobject_loots.get_mut(&gameobject_guid)?;
        let slot = usize::from(loot_slot);
        let loot = state.loot_items.get(slot).cloned()?;
        state.loot_items.remove(slot);
        Some((gameobject_guid, loot_slot, loot))
    }

    fn restore_db_gameobject_loot_item(
        &mut self,
        gameobject_guid: u64,
        loot_slot: u8,
        loot: DbCreatureLootRuntime,
    ) -> Option<Vec<DbCreatureLootRuntime>> {
        self.gameobjects.get(&gameobject_guid)?;
        let state = self.gameobject_loots.entry(gameobject_guid).or_default();
        let slot = usize::from(loot_slot).min(state.loot_items.len());
        state.loot_items.insert(slot, loot);
        Some(state.loot_items.clone())
    }

    fn release_db_gameobject_loot(
        &mut self,
        gameobject_guid: u64,
        character_guid: u32,
    ) -> Option<()> {
        self.gameobjects.get(&gameobject_guid)?;
        self.remove_db_gameobject_looter(gameobject_guid, character_guid);
        Some(())
    }

    fn clear_db_gameobject_loot(&mut self, gameobject_guid: u64) {
        if let Some(state) = self.gameobject_loots.remove(&gameobject_guid) {
            for character_guid in state.open_characters {
                self.gameobject_looting_by_character
                    .remove(&character_guid);
            }
        }
    }

    fn remove_db_gameobject_looter(&mut self, gameobject_guid: u64, character_guid: u32) {
        self.gameobject_looting_by_character
            .remove(&character_guid);
        if let Some(state) = self.gameobject_loots.get_mut(&gameobject_guid) {
            state.open_characters.remove(&character_guid);
            if state.open_characters.is_empty() && state.loot_items.is_empty() {
                self.gameobject_loots.remove(&gameobject_guid);
            }
        }
    }
}
