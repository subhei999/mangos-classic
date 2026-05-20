use super::*;

impl MapRuntimeManager {
    pub(in crate::world) async fn open_db_creature_loot(
        &self,

        map_id: u32,

        creature_guid: u64,

        character_guid: u32,

        access_owner: CreatureLootOwner,

        current_looter: Option<u32>,

        loot_items: Vec<DbCreatureLootRuntime>,
    ) -> Option<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;

        let creature = map.lock().await.open_db_creature_loot(
            creature_guid,
            character_guid,
            access_owner,
            current_looter,
            loot_items,
        );

        creature
    }

    pub(in crate::world) async fn set_db_creature_loot_owner(
        &self,

        map_id: u32,

        creature_guid: ObjectGuid,

        owner: CreatureLootOwner,
    ) -> Option<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;

        let creature = map
            .lock()
            .await
            .set_db_creature_loot_owner(creature_guid, owner);

        creature
    }

    pub(in crate::world) async fn force_db_creature_loot_owner(
        &self,

        map_id: u32,

        creature_guid: ObjectGuid,

        owner: CreatureLootOwner,
    ) -> Option<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;

        let creature = map
            .lock()
            .await
            .force_db_creature_loot_owner(creature_guid, owner);

        creature
    }

    pub(in crate::world) async fn db_creature_loot_guid_for_character(
        &self,

        map_id: u32,

        character_guid: u32,
    ) -> Option<u64> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };

        let map = map?;

        let creature_guid = map
            .lock()
            .await
            .db_creature_loot_guid_for_character(character_guid);

        creature_guid
    }

    pub(in crate::world) async fn db_creature_looting_characters(
        &self,

        map_id: u32,

        creature_guid: u64,
    ) -> Vec<u32> {
        let Some(map) = self.maps.lock().await.get(&(map_id, 0)).cloned() else {
            return Vec::new();
        };

        let characters = map
            .lock()
            .await
            .db_creature_looting_characters(creature_guid);

        characters
    }

    pub(in crate::world) async fn db_creature_needs_loot_item(
        &self,

        map_id: u32,

        creature_guid: u64,
    ) -> Option<bool> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };

        let map = map?;

        let needs_loot_item = map.lock().await.db_creature_needs_loot_item(creature_guid);

        needs_loot_item
    }

    pub(in crate::world) async fn take_db_creature_loot_money(
        &self,

        map_id: u32,

        character_guid: u32,
    ) -> Option<(u32, DbCreatureRuntime)> {
        let map = self.get_or_create_map(map_id, 0).await;

        let loot = map.lock().await.take_db_creature_loot_money(character_guid);

        loot
    }

    pub(in crate::world) async fn take_db_creature_loot_item(
        &self,

        map_id: u32,

        character_guid: u32,

        loot_slot: u8,
    ) -> Option<(u64, u8, DbCreatureLootRuntime, DbCreatureRuntime)> {
        let map = self.get_or_create_map(map_id, 0).await;

        let loot = map
            .lock()
            .await
            .take_db_creature_loot_item(character_guid, loot_slot);

        loot
    }

    pub(in crate::world) async fn take_db_creature_loot_item_by_guid(
        &self,

        map_id: u32,

        creature_guid: u64,

        loot_slot: u8,
    ) -> Option<(u8, DbCreatureLootRuntime, DbCreatureRuntime)> {
        let map = self.get_or_create_map(map_id, 0).await;

        let loot = map
            .lock()
            .await
            .take_db_creature_loot_item_by_guid(creature_guid, loot_slot);

        loot
    }

    pub(in crate::world) async fn restore_db_creature_loot_item(
        &self,

        map_id: u32,

        creature_guid: u64,

        loot_slot: u8,

        loot: DbCreatureLootRuntime,
    ) -> Option<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;

        let creature =
            map.lock()
                .await
                .restore_db_creature_loot_item(creature_guid, loot_slot, loot);

        creature
    }

    pub(in crate::world) async fn release_db_creature_loot_roll_item(
        &self,

        map_id: u32,

        creature_guid: u64,

        loot_slot: u8,
    ) -> Option<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;

        let creature = map
            .lock()
            .await
            .release_db_creature_loot_roll_item(creature_guid, loot_slot);

        creature
    }

    pub(in crate::world) async fn release_db_creature_current_looter_pass_item(
        &self,

        map_id: u32,

        creature_guid: u64,

        loot_slot: u8,
    ) -> Option<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;

        let creature = map
            .lock()
            .await
            .release_db_creature_current_looter_pass_item(creature_guid, loot_slot);

        creature
    }

    pub(in crate::world) async fn release_db_creature_loot(
        &self,

        map_id: u32,

        creature_guid: u64,

        now: Instant,

        exclude_character_guid: Option<u32>,
    ) -> anyhow::Result<Option<DbCreatureLootReleaseEvent>> {
        let map = self.get_or_create_map(map_id, 0).await;

        let event =
            map.lock()
                .await
                .release_db_creature_loot(creature_guid, now, exclude_character_guid);

        event
    }
}
