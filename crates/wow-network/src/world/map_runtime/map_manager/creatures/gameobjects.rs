use super::*;

impl MapRuntimeManager {
    pub(in crate::world) async fn db_gameobject_snapshot(
        &self,
        map_id: u32,
        gameobject_guid: ObjectGuid,
    ) -> Option<DbGameObjectRuntime> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let snapshot = map.lock().await.db_gameobject_snapshot(gameobject_guid);
        snapshot
    }

    pub(in crate::world) async fn consume_db_gameobject(
        &self,
        map_id: u32,
        gameobject_guid: ObjectGuid,
        now: Instant,
        exclude_character_guid: Option<u32>,
    ) -> Option<(DbGameObjectRuntime, Vec<(SessionId, OutboundWorldPacket)>)> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let consumed =
            map.lock()
                .await
                .consume_db_gameobject(gameobject_guid, now, exclude_character_guid);
        consumed
    }

    pub(in crate::world) async fn open_db_gameobject_loot(
        &self,
        map_id: u32,
        gameobject_guid: u64,
        character_guid: u32,
        loot_items: Vec<DbCreatureLootRuntime>,
    ) -> Option<(DbGameObjectRuntime, Vec<DbCreatureLootRuntime>)> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let opened =
            map.lock()
                .await
                .open_db_gameobject_loot(gameobject_guid, character_guid, loot_items);
        opened
    }

    pub(in crate::world) async fn db_gameobject_loot_guid_for_character(
        &self,
        map_id: u32,
        character_guid: u32,
    ) -> Option<u64> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let gameobject_guid = map
            .lock()
            .await
            .db_gameobject_loot_guid_for_character(character_guid);
        gameobject_guid
    }

    pub(in crate::world) async fn take_db_gameobject_loot_item(
        &self,
        map_id: u32,
        character_guid: u32,
        loot_slot: u8,
    ) -> Option<(u64, u8, DbCreatureLootRuntime)> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let loot = map
            .lock()
            .await
            .take_db_gameobject_loot_item(character_guid, loot_slot);
        loot
    }

    pub(in crate::world) async fn restore_db_gameobject_loot_item(
        &self,
        map_id: u32,
        gameobject_guid: u64,
        loot_slot: u8,
        loot: DbCreatureLootRuntime,
    ) -> Option<Vec<DbCreatureLootRuntime>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let restored =
            map.lock()
                .await
                .restore_db_gameobject_loot_item(gameobject_guid, loot_slot, loot);
        restored
    }

    pub(in crate::world) async fn db_gameobject_loot_is_empty(
        &self,
        map_id: u32,
        gameobject_guid: u64,
    ) -> bool {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return true;
        };
        let is_empty = map
            .lock()
            .await
            .db_gameobject_loot_is_empty(gameobject_guid);
        is_empty
    }

    pub(in crate::world) async fn release_db_gameobject_loot(
        &self,
        map_id: u32,
        gameobject_guid: u64,
        character_guid: u32,
    ) -> Option<()> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let released = map
            .lock()
            .await
            .release_db_gameobject_loot(gameobject_guid, character_guid);
        released
    }

    pub(in crate::world) async fn db_gameobject_snapshots(
        &self,
        map_id: u32,
        gameobject_guids: &[u64],
    ) -> Vec<DbGameObjectRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let snapshots = map.lock().await.db_gameobject_snapshots(gameobject_guids);
        snapshots
    }
}
