#[derive(Debug, Default)]
struct MapRuntimeManager {
    maps: Mutex<MapRuntimeHandles>,
    creature_display_scales: HashMap<u32, f32>,
    spell_durations: HashMap<u32, SpellDurationEntry>,
    creature_grid_load_ensure_calls: AtomicU64,
    creature_grid_load_cache_hits: AtomicU64,
    creature_grid_load_db_queries: AtomicU64,
    creature_grid_load_rows: AtomicU64,
}

type MapRuntimeHandles = HashMap<(u32, u32), Arc<Mutex<MapRuntime>>>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(dead_code)]
struct CreatureGridLoadStats {
    ensure_calls: u64,
    cache_hits: u64,
    db_queries: u64,
    rows_loaded: u64,
}

fn apply_creature_display_scale_fallbacks(
    spawns: &mut [CreatureSpawnQuery],
    display_scales: &HashMap<u32, f32>,
) {
    for spawn in spawns {
        if spawn.template.scale > 0.0 {
            continue;
        }
        let Some(scale) = [
            spawn.template.display_id1,
            spawn.template.display_id2,
            spawn.template.display_id3,
            spawn.template.display_id4,
        ]
        .into_iter()
        .find_map(|display_id| display_scales.get(&display_id).copied().filter(|scale| *scale > 0.0))
        else {
            continue;
        };
        spawn.template.scale = scale;
    }
}

impl MapRuntimeManager {
    fn with_world_data_files(world_data_files: &WorldDataFiles) -> Self {
        Self {
            creature_display_scales: world_data_files.creature_display_scales.clone(),
            spell_durations: world_data_files.spell_durations.clone(),
            ..Self::default()
        }
    }

    fn spell_duration(&self, duration_index: u32) -> Option<SpellDurationEntry> {
        self.spell_durations.get(&duration_index).copied()
    }

    async fn add_player(
        &self,
        player: PlayerRuntime,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let map_key = (player.position.map_id, 0);
        let map = {
            let mut maps = self.maps.lock().await;
            maps.entry(map_key)
                .or_insert_with(|| Arc::new(Mutex::new(MapRuntime::new(map_key.0, map_key.1))))
                .clone()
        };
        let packets = map.lock().await.add_player(player);
        packets
    }

    async fn remove_player(
        &self,
        map_id: u32,
        character_guid: u32,
    ) -> Vec<(SessionId, OutboundWorldPacket)> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Vec::new();
        };
        let packets = map.lock().await.remove_player(character_guid);
        packets
    }

    async fn update_player_position(
        &self,
        map_id: u32,
        character_guid: u32,
        opcode: u16,
        movement: &MovementInfo,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(Vec::new());
        };
        let packets = map
            .lock()
            .await
            .update_player_position(character_guid, opcode, movement);
        packets
    }

    async fn update_player_visible_equipment(
        &self,
        map_id: u32,
        character_guid: u32,
        visible_equipment: [u32; ENUM_EQUIPMENT_SLOTS],
        changed_slots: &[u8],
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(Vec::new());
        };
        let packets = map
            .lock()
            .await
            .update_player_visible_equipment(character_guid, visible_equipment, changed_slots);
        packets
    }

    async fn update_player_health(
        &self,
        map_id: u32,
        character_guid: u32,
        health: u32,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(Vec::new());
        };
        let packets = map
            .lock()
            .await
            .update_player_health(character_guid, health);
        packets
    }

    async fn sync_player_gameplay_state(
        &self,
        map_id: u32,
        character_guid: u32,
        session: &WorldSessionState,
    ) {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return;
        };
        map.lock()
            .await
            .sync_player_gameplay_state(character_guid, session);
    }

    async fn player_runtime_snapshot(
        &self,
        map_id: u32,
        character_guid: u32,
    ) -> Option<PlayerRuntimeSnapshot> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let snapshot = map.lock().await.player_runtime_snapshot(character_guid);
        snapshot
    }

    async fn player_visible_db_creature_guids(&self, map_id: u32, character_guid: u32) -> Vec<u64> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Vec::new();
        };
        let guids = map
            .lock()
            .await
            .player_visible_db_creature_guids(character_guid);
        guids
    }

    async fn should_rescan_player_creature_visibility(
        &self,
        map_id: u32,
        character_guid: u32,
        position: WorldPosition,
    ) -> bool {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return true;
        };
        let should_rescan = map
            .lock()
            .await
            .should_rescan_player_creature_visibility(character_guid, position);
        should_rescan
    }

    async fn should_rescan_player_gameobject_visibility(
        &self,
        map_id: u32,
        character_guid: u32,
        position: WorldPosition,
    ) -> bool {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return true;
        };
        let should_rescan = map
            .lock()
            .await
            .should_rescan_player_gameobject_visibility(character_guid, position);
        should_rescan
    }

    async fn should_rescan_player_corpse_visibility(
        &self,
        map_id: u32,
        character_guid: u32,
        position: WorldPosition,
    ) -> bool {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return true;
        };
        let should_rescan = map
            .lock()
            .await
            .should_rescan_player_corpse_visibility(character_guid, position);
        should_rescan
    }

    async fn reset_player_visibility_scan_positions(&self, map_id: u32, character_guid: u32) {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return;
        };
        map.lock()
            .await
            .reset_player_visibility_scan_positions(character_guid);
    }

    async fn update_player_combat_stats(
        &self,
        map_id: u32,
        character_guid: u32,
        combat_stats: PlayerCombatStats,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(Vec::new());
        };
        let packets = map
            .lock()
            .await
            .update_player_combat_stats(character_guid, combat_stats);
        packets
    }

    async fn player_combat_stats(
        &self,
        map_id: u32,
        character_guid: u32,
    ) -> Option<PlayerCombatStats> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let combat_stats = map.lock().await.player_combat_stats(character_guid);
        combat_stats
    }

    async fn set_player_auto_attack(
        &self,
        map_id: u32,
        character_guid: u32,
        target: Option<ObjectGuid>,
        next_swing_at: Option<Instant>,
    ) {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return;
        };
        map.lock()
            .await
            .set_player_auto_attack(character_guid, target, next_swing_at);
    }

    async fn player_auto_attack_due(
        &self,
        map_id: u32,
        character_guid: u32,
        now: Instant,
    ) -> Option<ObjectGuid> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let target = map.lock().await.player_auto_attack_due(character_guid, now);
        target
    }

    async fn player_auto_attack_target(
        &self,
        map_id: u32,
        character_guid: u32,
    ) -> Option<ObjectGuid> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let target = map.lock().await.player_auto_attack_target(character_guid);
        target
    }

    async fn set_player_next_swing_at(
        &self,
        map_id: u32,
        character_guid: u32,
        next_swing_at: Option<Instant>,
    ) {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return;
        };
        map.lock()
            .await
            .set_player_next_swing_at(character_guid, next_swing_at);
    }

    async fn set_player_power2(&self, map_id: u32, character_guid: u32, power2: u32) {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return;
        };
        map.lock().await.set_player_power2(character_guid, power2);
    }

    async fn update_player_selection(
        &self,
        map_id: u32,
        character_guid: u32,
        selected_target: Option<ObjectGuid>,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(Vec::new());
        };
        let packets = map
            .lock()
            .await
            .update_player_selection(character_guid, selected_target);
        packets
    }

    #[cfg(test)]
    async fn update_player_db_creature_visibility(
        &self,
        map_id: u32,
        character_guid: u32,
        create_guids: &[ObjectGuid],
        destroy_guids: &[ObjectGuid],
    ) {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return;
        };
        map.lock().await.update_player_db_creature_visibility(
            character_guid,
            create_guids,
            destroy_guids,
        );
    }

    async fn broadcast_nearby_player_packet(
        &self,
        map_id: u32,
        character_guid: u32,
        radius: f32,
        packet: OutboundWorldPacket,
    ) -> Vec<(SessionId, OutboundWorldPacket)> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Vec::new();
        };
        let packets = map
            .lock()
            .await
            .broadcast_nearby_player_packet(character_guid, radius, packet);
        packets
    }

    #[allow(dead_code)]
    async fn share_db_creature_snapshots(
        &self,
        map_id: u32,
        creatures: Vec<DbCreatureRuntime>,
    ) -> Vec<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let creatures = map.lock().await.share_db_creature_snapshots(creatures);
        creatures
    }

    async fn ensure_db_creature_grids_loaded(
        &self,
        character_db_pool: &MySqlPool,
        world_db_pool: &MySqlPool,
        map_id: u32,
        position: WorldPosition,
        radius: f32,
    ) -> anyhow::Result<()> {
        self.creature_grid_load_ensure_calls
            .fetch_add(1, Ordering::Relaxed);
        let map = self.get_or_create_map(map_id, 0).await;
        let grids = {
            map.lock()
                .await
                .unloaded_creature_grids_for_area(position, radius)
        };
        if grids.is_empty() {
            let cache_hits = self
                .creature_grid_load_cache_hits
                .fetch_add(1, Ordering::Relaxed)
                + 1;
            debug!(
                map_id,
                x = position.x,
                y = position.y,
                radius,
                cache_hits,
                "DB creature visibility served from already loaded grids"
            );
            return Ok(());
        }
        for grid in grids {
            let (min_x, max_x, min_y, max_y) = grid_world_bounds(grid);
            let spawns = wow_db::get_creature_spawns_in_rect(
                world_db_pool,
                map_id,
                min_x,
                max_x,
                min_y,
                max_y,
            )
            .await?;
            let mut spawns = spawns;
            apply_creature_display_scale_fallbacks(&mut spawns, &self.creature_display_scales);
            let spawn_count = spawns.len() as u64;
            let db_queries = self
                .creature_grid_load_db_queries
                .fetch_add(1, Ordering::Relaxed)
                + 1;
            let rows_loaded = self
                .creature_grid_load_rows
                .fetch_add(spawn_count, Ordering::Relaxed)
                + spawn_count;
            let runtimes = build_db_creature_runtimes_with_respawns(character_db_pool, spawns).await?;
            map.lock().await.insert_loaded_creature_grid(grid, runtimes);
            info!(
                map_id,
                grid_x = grid.x,
                grid_y = grid.y,
                min_x,
                max_x,
                min_y,
                max_y,
                spawn_count,
                db_queries,
                rows_loaded,
                "Loaded DB creature grid into MapRuntime"
            );
        }
        Ok(())
    }

    async fn apply_player_aura(
        &self,
        map_id: u32,
        character_guid: u32,
        aura: ActiveAura,
    ) -> anyhow::Result<Option<PlayerAuraUpdateEvent>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(None);
        };
        let event = map.lock().await.apply_player_aura(character_guid, aura);
        event
    }

    #[cfg(test)]
    async fn ensure_db_creature_grids_loaded_for_test(
        &self,
        map_id: u32,
        position: WorldPosition,
        radius: f32,
        mut runtimes_for_grid: impl FnMut(GridCoord) -> Vec<DbCreatureRuntime>,
    ) {
        self.creature_grid_load_ensure_calls
            .fetch_add(1, Ordering::Relaxed);
        let map = self.get_or_create_map(map_id, 0).await;
        let grids = {
            map.lock()
                .await
                .unloaded_creature_grids_for_area(position, radius)
        };
        if grids.is_empty() {
            self.creature_grid_load_cache_hits
                .fetch_add(1, Ordering::Relaxed);
            return;
        }
        for grid in grids {
            let runtimes = runtimes_for_grid(grid);
            self.creature_grid_load_db_queries
                .fetch_add(1, Ordering::Relaxed);
            self.creature_grid_load_rows
                .fetch_add(runtimes.len() as u64, Ordering::Relaxed);
            map.lock().await.insert_loaded_creature_grid(grid, runtimes);
        }
    }

    #[allow(dead_code)]
    fn creature_grid_load_stats(&self) -> CreatureGridLoadStats {
        CreatureGridLoadStats {
            ensure_calls: self
                .creature_grid_load_ensure_calls
                .load(Ordering::Relaxed),
            cache_hits: self
                .creature_grid_load_cache_hits
                .load(Ordering::Relaxed),
            db_queries: self
                .creature_grid_load_db_queries
                .load(Ordering::Relaxed),
            rows_loaded: self.creature_grid_load_rows.load(Ordering::Relaxed),
        }
    }

    async fn nearby_db_creature_snapshots(
        &self,
        map_id: u32,
        position: WorldPosition,
        radius: f32,
        limit: u32,
    ) -> Vec<DbCreatureRuntime> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Vec::new();
        };
        let snapshots = map
            .lock()
            .await
            .nearby_db_creature_snapshots(position, radius, limit);
        snapshots
    }

    async fn stage_player_db_creature_visibility(
        &self,
        map_id: u32,
        character_guid: u32,
        position: WorldPosition,
        nearby_creatures: Vec<DbCreatureRuntime>,
    ) -> MapDbCreatureVisibilityStage {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return MapDbCreatureVisibilityStage {
                nearby_creatures,
                ..Default::default()
            };
        };
        let stage = map.lock().await.stage_player_db_creature_visibility(
            character_guid,
            position,
            nearby_creatures,
        );
        stage
    }

    async fn stage_player_db_gameobject_visibility(
        &self,
        map_id: u32,
        character_guid: u32,
        position: WorldPosition,
        nearby_gameobjects: Vec<DbGameObjectRuntime>,
        now: Instant,
    ) -> MapDbGameObjectVisibilityStage {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return MapDbGameObjectVisibilityStage {
                nearby_gameobjects,
                ..Default::default()
            };
        };
        let stage = map
            .lock()
            .await
            .stage_player_db_gameobject_visibility(
                character_guid,
                position,
                nearby_gameobjects,
                now,
            );
        stage
    }

    async fn ensure_db_gameobject_grids_loaded(
        &self,
        world_db_pool: &MySqlPool,
        map_id: u32,
        position: WorldPosition,
        radius: f32,
    ) -> anyhow::Result<()> {
        let map = self.get_or_create_map(map_id, 0).await;
        let grids = {
            map.lock()
                .await
                .unloaded_gameobject_grids_for_area(position, radius)
        };
        for grid in grids {
            let (min_x, max_x, min_y, max_y) = grid_world_bounds(grid);
            let spawns = wow_db::get_gameobject_spawns_in_rect(
                world_db_pool,
                map_id,
                min_x,
                max_x,
                min_y,
                max_y,
            )
            .await?;
            let runtimes = spawns.into_iter().map(DbGameObjectRuntime::new).collect();
            map.lock()
                .await
                .insert_loaded_gameobject_grid(grid, runtimes);
            info!(
                map_id,
                grid_x = grid.x,
                grid_y = grid.y,
                min_x,
                max_x,
                min_y,
                max_y,
                "Loaded DB gameobject grid into MapRuntime"
            );
        }
        Ok(())
    }

    #[cfg(test)]
    async fn ensure_db_gameobject_grids_loaded_for_test(
        &self,
        map_id: u32,
        position: WorldPosition,
        radius: f32,
        mut runtimes_for_grid: impl FnMut(GridCoord) -> Vec<DbGameObjectRuntime>,
    ) {
        let map = self.get_or_create_map(map_id, 0).await;
        let grids = {
            map.lock()
                .await
                .unloaded_gameobject_grids_for_area(position, radius)
        };
        for grid in grids {
            let runtimes = runtimes_for_grid(grid);
            map.lock()
                .await
                .insert_loaded_gameobject_grid(grid, runtimes);
        }
    }

    async fn nearby_db_gameobject_snapshots(
        &self,
        map_id: u32,
        position: WorldPosition,
        radius: f32,
        limit: u32,
    ) -> Vec<DbGameObjectRuntime> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Vec::new();
        };
        let snapshots = map
            .lock()
            .await
            .nearby_db_gameobject_snapshots(position, radius, limit);
        snapshots
    }

    async fn ensure_player_corpse_grids_loaded(
        &self,
        character_db_pool: &MySqlPool,
        map_id: u32,
        position: WorldPosition,
        radius: f32,
    ) -> anyhow::Result<()> {
        let map = self.get_or_create_map(map_id, 0).await;
        let grids = {
            map.lock()
                .await
                .unloaded_player_corpse_grids_for_area(position, radius)
        };
        for grid in grids {
            let (center_x, center_y) = grid_world_center(grid);
            let corpses = wow_db::get_nearby_player_corpses(
                character_db_pool,
                map_id,
                center_x,
                center_y,
                player_corpse_grid_query_radius(),
                u32::MAX,
            )
            .await?
            .into_iter()
            .map(player_corpse_runtime_from_query)
            .collect::<Vec<_>>();
            map.lock()
                .await
                .insert_loaded_player_corpse_grid(grid, corpses);
            let corpse_count = map.lock().await.corpses.len();
            debug!(
                map_id,
                grid_x = grid.x,
                grid_y = grid.y,
                corpse_count,
                "Loaded player corpse grid into MapRuntime"
            );
        }
        Ok(())
    }

    async fn nearby_player_corpse_snapshots(
        &self,
        map_id: u32,
        position: WorldPosition,
        radius: f32,
        limit: u32,
    ) -> Vec<PlayerCorpseRuntime> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Vec::new();
        };
        let snapshots = map
            .lock()
            .await
            .nearby_player_corpse_snapshots(position, radius, limit);
        snapshots
    }

    async fn stage_player_corpse_visibility(
        &self,
        map_id: u32,
        character_guid: u32,
        position: WorldPosition,
        nearby_corpses: Vec<PlayerCorpseRuntime>,
    ) -> MapPlayerCorpseVisibilityStage {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return MapPlayerCorpseVisibilityStage {
                nearby_corpses,
                ..Default::default()
            };
        };
        let stage = map
            .lock()
            .await
            .stage_player_corpse_visibility(character_guid, position, nearby_corpses);
        stage
    }

    async fn upsert_player_corpse(&self, map_id: u32, corpse: PlayerCorpseRuntime) {
        let map = self.get_or_create_map(map_id, 0).await;
        map.lock().await.upsert_player_corpse(corpse);
    }

    async fn db_gameobject_snapshot(
        &self,
        map_id: u32,
        gameobject_guid: ObjectGuid,
    ) -> Option<DbGameObjectRuntime> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let snapshot = map.lock().await.db_gameobject_snapshot(gameobject_guid);
        snapshot
    }

    async fn consume_db_gameobject(
        &self,
        map_id: u32,
        gameobject_guid: ObjectGuid,
        now: Instant,
        exclude_character_guid: Option<u32>,
    ) -> Option<(DbGameObjectRuntime, Vec<(SessionId, OutboundWorldPacket)>)> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let consumed = map
            .lock()
            .await
            .consume_db_gameobject(gameobject_guid, now, exclude_character_guid);
        consumed
    }

    async fn open_db_gameobject_loot(
        &self,
        map_id: u32,
        gameobject_guid: u64,
        character_guid: u32,
        loot_items: Vec<DbCreatureLootRuntime>,
    ) -> Option<(DbGameObjectRuntime, Vec<DbCreatureLootRuntime>)> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let opened = map.lock().await.open_db_gameobject_loot(
            gameobject_guid,
            character_guid,
            loot_items,
        );
        opened
    }

    async fn db_gameobject_loot_guid_for_character(
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

    async fn take_db_gameobject_loot_item(
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

    async fn restore_db_gameobject_loot_item(
        &self,
        map_id: u32,
        gameobject_guid: u64,
        loot_slot: u8,
        loot: DbCreatureLootRuntime,
    ) -> Option<Vec<DbCreatureLootRuntime>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let restored = map
            .lock()
            .await
            .restore_db_gameobject_loot_item(gameobject_guid, loot_slot, loot);
        restored
    }

    async fn release_db_gameobject_loot(
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

    async fn db_creature_snapshots(
        &self,
        map_id: u32,
        creature_guids: &[u64],
    ) -> Vec<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let snapshots = map.lock().await.db_creature_snapshots(creature_guids);
        snapshots
    }

    async fn db_creature_snapshot(
        &self,
        map_id: u32,
        creature_guid: ObjectGuid,
    ) -> Option<DbCreatureRuntime> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let snapshot = map.lock().await.db_creature_snapshot(creature_guid);
        snapshot
    }

    async fn db_creature_combat_snapshot(
        &self,
        map_id: u32,
        creature_guid: ObjectGuid,
    ) -> Option<DbCreatureRuntime> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let creature = map.lock().await.db_creature_combat_snapshot(creature_guid);
        creature
    }

    async fn validate_player_melee_against_db_creature(
        &self,
        map_id: u32,
        character_guid: u32,
        target: ObjectGuid,
        navigation: &DbCreatureNavigationGuardrail,
    ) -> DbCreaturePlayerMeleeValidation {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return DbCreaturePlayerMeleeValidation {
                check: PlayerMeleeCheck::MissingTarget,
            };
        };
        let validation = map
            .lock()
            .await
            .validate_player_melee_against_db_creature(character_guid, target, navigation);
        validation
    }

    #[allow(dead_code)]
    async fn update_db_creature_snapshot(&self, map_id: u32, creature: DbCreatureRuntime) {
        let map = self.get_or_create_map(map_id, 0).await;
        map.lock().await.update_db_creature_snapshot(creature);
    }

    async fn update_db_creature_snapshot_and_broadcast(
        &self,
        map_id: u32,
        creature: DbCreatureRuntime,
        exclude_character_guid: Option<u32>,
        packet: OutboundWorldPacket,
    ) -> Vec<(SessionId, OutboundWorldPacket)> {
        let map = self.get_or_create_map(map_id, 0).await;
        let packets = map
            .lock()
            .await
            .update_db_creature_snapshot_and_broadcast(
                creature,
                exclude_character_guid,
                packet,
        );
        packets
    }

    async fn apply_db_creature_damage(
        &self,
        map_id: u32,
        request: DbCreatureDamageRequest,
    ) -> anyhow::Result<Option<DbCreatureDamageEvent>> {
        let map = self.get_or_create_map(map_id, 0).await;
        let event = map.lock().await.apply_db_creature_damage(request);
        event
    }

    async fn advance_db_creature_lifecycle(
        &self,
        map_id: u32,
        creature_guids: &[u64],
        viewer_position: WorldPosition,
        exclude_character_guid: Option<u32>,
        now: Instant,
    ) -> anyhow::Result<Vec<DbCreatureLifecycleEvent>> {
        let map = self.get_or_create_map(map_id, 0).await;
        let events = map.lock().await.advance_db_creature_lifecycle(
            creature_guids,
            viewer_position,
            exclude_character_guid,
            now,
        );
        events
    }

    async fn open_db_creature_loot(
        &self,
        map_id: u32,
        creature_guid: u64,
        character_guid: u32,
        loot_items: Vec<DbCreatureLootRuntime>,
    ) -> Option<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let creature = map
            .lock()
            .await
            .open_db_creature_loot(creature_guid, character_guid, loot_items);
        creature
    }

    async fn db_creature_loot_guid_for_character(
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

    async fn db_creature_needs_loot_item(&self, map_id: u32, creature_guid: u64) -> Option<bool> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let needs_loot_item = map.lock().await.db_creature_needs_loot_item(creature_guid);
        needs_loot_item
    }

    async fn take_db_creature_loot_money(
        &self,
        map_id: u32,
        character_guid: u32,
    ) -> Option<(u32, DbCreatureRuntime)> {
        let map = self.get_or_create_map(map_id, 0).await;
        let loot = map.lock().await.take_db_creature_loot_money(character_guid);
        loot
    }

    async fn take_db_creature_loot_item(
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

    async fn restore_db_creature_loot_item(
        &self,
        map_id: u32,
        creature_guid: u64,
        loot_slot: u8,
        loot: DbCreatureLootRuntime,
    ) -> Option<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let creature = map
            .lock()
            .await
            .restore_db_creature_loot_item(creature_guid, loot_slot, loot);
        creature
    }

    async fn release_db_creature_loot(
        &self,
        map_id: u32,
        creature_guid: u64,
        now: Instant,
        exclude_character_guid: Option<u32>,
    ) -> anyhow::Result<Option<DbCreatureLootReleaseEvent>> {
        let map = self.get_or_create_map(map_id, 0).await;
        let event = map
            .lock()
            .await
            .release_db_creature_loot(creature_guid, now, exclude_character_guid);
        event
    }

    async fn begin_db_creature_combat(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        now: Instant,
    ) -> Option<(CreatureCombatState, DbCreatureRuntime)> {
        let map = self.get_or_create_map(map_id, 0).await;
        let mut map = map.lock().await;
        let creature = map.db_creature_combat_snapshot(attacker)?;
        let combat = map.begin_db_creature_combat(attacker, victim, now)?;
        Some((combat, creature))
    }

    async fn clear_db_creature_combat(&self, map_id: u32, attacker: ObjectGuid) {
        let map = self.get_or_create_map(map_id, 0).await;
        map.lock().await.clear_db_creature_combat(attacker);
    }

    async fn clear_db_creature_combats_for_victim(&self, map_id: u32, victim: ObjectGuid) {
        let map = self.get_or_create_map(map_id, 0).await;
        map.lock()
            .await
            .clear_db_creature_combats_for_victim(victim);
    }

    async fn active_db_creature_combats_for_victim(
        &self,
        map_id: u32,
        victim: ObjectGuid,
    ) -> Vec<CreatureCombatState> {
        let map = self.get_or_create_map(map_id, 0).await;
        let combats = map
            .lock()
            .await
            .active_db_creature_combats_for_victim(victim);
        combats
    }

    async fn active_db_creature_combat_snapshot(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        victim: ObjectGuid,
    ) -> Option<ActiveDbCreatureCombatSnapshot> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let snapshot = map
            .lock()
            .await
            .active_db_creature_combat_snapshot(attacker, victim);
        snapshot
    }

    #[allow(dead_code)]
    async fn apply_db_creature_player_damage(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        damage: u32,
        now: Instant,
        next_swing_at: Instant,
    ) -> anyhow::Result<Option<DbCreaturePlayerDamageEvent>> {
        let map = self.get_or_create_map(map_id, 0).await;
        let event = map
            .lock()
            .await
            .apply_db_creature_player_damage(attacker, victim, damage, now, next_swing_at);
        event
    }

    async fn apply_db_creature_player_melee_outcome(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        outcome: MeleeDamageOutcome,
        now: Instant,
        next_swing_at: Instant,
    ) -> anyhow::Result<Option<DbCreaturePlayerDamageEvent>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(None);
        };
        let event = map.lock().await.apply_db_creature_player_melee_outcome(
            attacker,
            victim,
            outcome,
            now,
            next_swing_at,
        );
        event
    }

    async fn db_creature_should_evade(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        now: Instant,
    ) -> bool {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return false;
        };
        let should_evade = map.lock().await.db_creature_should_evade(attacker, now);
        should_evade
    }

    async fn defer_ready_db_creature_swing_retry(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        now: Instant,
    ) -> Option<CreatureCombatState> {
        let map = self.get_or_create_map(map_id, 0).await;
        let combat = map
            .lock()
            .await
            .defer_ready_db_creature_swing_retry(attacker, victim, now);
        combat
    }

    async fn advance_db_creature_motion(
        &self,
        map_id: u32,
        creature_guid: ObjectGuid,
        now: Instant,
    ) -> Option<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let creature = map
            .lock()
            .await
            .advance_db_creature_motion(creature_guid, now);
        creature
    }

    #[allow(dead_code)]
    async fn advance_active_db_creature_idle_motions(
        &self,
        map_id: u32,
        navigation: &DbCreatureNavigationGuardrail,
        now: Instant,
    ) -> anyhow::Result<DbCreatureIdleMotionTick> {
        let map = self.get_or_create_map(map_id, 0).await;
        let tick = map
            .lock()
            .await
            .advance_active_db_creature_idle_motions(navigation, now);
        tick
    }

    async fn advance_all_active_db_creature_idle_motions(
        &self,
        navigation: &DbCreatureNavigationGuardrail,
        now: Instant,
    ) -> anyhow::Result<DbCreatureIdleMotionTick> {
        let maps = {
            self.maps
                .lock()
                .await
                .values()
                .cloned()
                .collect::<Vec<_>>()
        };
        let mut creatures = Vec::new();
        let mut packets = Vec::new();
        for map in maps {
            let tick = map
                .lock()
                .await
                .advance_active_db_creature_idle_motions(navigation, now)?;
            creatures.extend(tick.creatures);
            packets.extend(tick.packets);
        }
        Ok(DbCreatureIdleMotionTick { creatures, packets })
    }

    async fn advance_all_player_regen_ticks(
        &self,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let maps = {
            self.maps
                .lock()
                .await
                .values()
                .cloned()
                .collect::<Vec<_>>()
        };
        let mut packets = Vec::new();
        for map in maps {
            packets.extend(map.lock().await.advance_player_regen_tick(now)?);
        }
        Ok(packets)
    }

    async fn advance_all_player_aura_expirations(
        &self,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let maps = {
            self.maps
                .lock()
                .await
                .values()
                .cloned()
                .collect::<Vec<_>>()
        };
        let mut packets = Vec::new();
        for map in maps {
            packets.extend(map.lock().await.advance_player_aura_expirations(now)?);
        }
        Ok(packets)
    }

    async fn record_observability_snapshots(&self) {
        let maps = {
            self.maps
                .lock()
                .await
                .values()
                .cloned()
                .collect::<Vec<_>>()
        };
        let mut snapshots = Vec::with_capacity(maps.len());
        for map in maps {
            snapshots.push(map.lock().await.observability_snapshot());
        }
        crate::observability::record_map_runtime_snapshots(snapshots);
    }

    #[allow(dead_code)]
    async fn db_creature_idle_motion_advancement_guids(
        &self,
        map_id: u32,
    ) -> Vec<u64> {
        let map = self.get_or_create_map(map_id, 0).await;
        let guids = map.lock().await.db_creature_idle_motion_advancement_guids();
        guids
    }

    #[allow(dead_code)]
    async fn db_creature_idle_motion_start_guids(
        &self,
        map_id: u32,
        now: Instant,
    ) -> Vec<u64> {
        let map = self.get_or_create_map(map_id, 0).await;
        let guids = map.lock().await.db_creature_idle_motion_start_guids(now);
        guids
    }

    #[allow(dead_code)]
    async fn start_db_creature_idle_motion(
        &self,
        map_id: u32,
        navigation: &DbCreatureNavigationGuardrail,
        creature_guid: ObjectGuid,
        now: Instant,
    ) -> Option<(DbCreatureRuntime, StartedCreatureMotion)> {
        let map = self.get_or_create_map(map_id, 0).await;
        let motion = map
            .lock()
            .await
            .start_db_creature_idle_motion(navigation, creature_guid, now);
        motion
    }

    async fn start_db_creature_chase_motion(
        &self,
        map_id: u32,
        navigation: &DbCreatureNavigationGuardrail,
        creature_guid: ObjectGuid,
        target: ObjectGuid,
        target_position: WorldPosition,
        now: Instant,
    ) -> Option<(DbCreatureRuntime, StartedCreatureMotion)> {
        let map = self.get_or_create_map(map_id, 0).await;
        let motion = map.lock().await.start_db_creature_chase_motion(
            navigation,
            creature_guid,
            target,
            target_position,
            now,
        );
        motion
    }

    async fn start_db_creature_return_home_motion(
        &self,
        map_id: u32,
        navigation: &DbCreatureNavigationGuardrail,
        creature_guid: ObjectGuid,
        now: Instant,
    ) -> Option<(DbCreatureRuntime, StartedCreatureMotion)> {
        let map = self.get_or_create_map(map_id, 0).await;
        let motion = map
            .lock()
            .await
            .start_db_creature_return_home_motion(navigation, creature_guid, now);
        motion
    }

    async fn stop_db_creature_motion(
        &self,
        map_id: u32,
        creature_guid: ObjectGuid,
    ) -> Option<(DbCreatureRuntime, StoppedCreatureMotion)> {
        let map = self.get_or_create_map(map_id, 0).await;
        let motion = map.lock().await.stop_db_creature_motion(creature_guid);
        motion
    }

    async fn face_db_creature_toward_position(
        &self,
        map_id: u32,
        creature_guid: ObjectGuid,
        target_position: WorldPosition,
    ) -> Option<(DbCreatureRuntime, WorldPosition, u32)> {
        let map = self.get_or_create_map(map_id, 0).await;
        let result = map
            .lock()
            .await
            .face_db_creature_toward_position(creature_guid, target_position);
        result
    }

    async fn prepare_db_creature_evade(
        &self,
        map_id: u32,
        creature_guid: ObjectGuid,
    ) -> Option<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let creature = map.lock().await.prepare_db_creature_evade(creature_guid);
        creature
    }

    async fn select_db_creature_sight_aggro_targets(
        &self,
        map_id: u32,
        character: &ActiveCharacter,
    ) -> Vec<DbCreatureRuntime> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Vec::new();
        };
        let targets = map
            .lock()
            .await
            .select_db_creature_sight_aggro_targets(character);
        targets
    }

    async fn select_db_creature_assist_targets(
        &self,
        map_id: u32,
        caller_guid: ObjectGuid,
        character: &ActiveCharacter,
    ) -> Option<(DbCreatureRuntime, Vec<ObjectGuid>)> {
        let map = self.get_or_create_map(map_id, 0).await;
        let targets = map
            .lock()
            .await
            .select_db_creature_assist_targets(caller_guid, character);
        targets
    }

    async fn get_or_create_map(&self, map_id: u32, instance_id: u32) -> Arc<Mutex<MapRuntime>> {
        let map_key = (map_id, instance_id);
        let mut maps = self.maps.lock().await;
        maps.entry(map_key)
            .or_insert_with(|| Arc::new(Mutex::new(MapRuntime::new(map_key.0, map_key.1))))
            .clone()
    }
}

fn grid_world_center(grid: GridCoord) -> (f32, f32) {
    let (min_x, max_x, min_y, max_y) = grid_world_bounds(grid);
    ((min_x + max_x) * 0.5, (min_y + max_y) * 0.5)
}

fn player_corpse_grid_query_radius() -> f32 {
    GRID_SIZE_YARDS * std::f32::consts::SQRT_2 * 0.5
}
