#[derive(Debug, Default)]
struct MapRuntimeManager {
    maps: Mutex<MapRuntimeHandles>,
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

impl MapRuntimeManager {
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

    async fn db_creature_snapshots(
        &self,
        map_id: u32,
        creature_guids: &[u64],
    ) -> Vec<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let snapshots = map.lock().await.db_creature_snapshots(creature_guids);
        snapshots
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
        loot_item: Option<DbCreatureLootRuntime>,
    ) -> Option<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let creature = map
            .lock()
            .await
            .open_db_creature_loot(creature_guid, loot_item);
        creature
    }

    async fn take_db_creature_loot_money(
        &self,
        map_id: u32,
        creature_guid: u64,
    ) -> Option<(u32, DbCreatureRuntime)> {
        let map = self.get_or_create_map(map_id, 0).await;
        let loot = map.lock().await.take_db_creature_loot_money(creature_guid);
        loot
    }

    async fn take_db_creature_loot_item(
        &self,
        map_id: u32,
        creature_guid: u64,
    ) -> Option<(DbCreatureLootRuntime, DbCreatureRuntime)> {
        let map = self.get_or_create_map(map_id, 0).await;
        let loot = map.lock().await.take_db_creature_loot_item(creature_guid);
        loot
    }

    async fn restore_db_creature_loot_item(
        &self,
        map_id: u32,
        creature_guid: u64,
        loot: DbCreatureLootRuntime,
    ) -> Option<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let creature = map
            .lock()
            .await
            .restore_db_creature_loot_item(creature_guid, loot);
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
    ) -> Option<CreatureCombatState> {
        let map = self.get_or_create_map(map_id, 0).await;
        let combat = map
            .lock()
            .await
            .begin_db_creature_combat(attacker, victim, now);
        combat
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

    #[allow(dead_code)]
    async fn apply_db_creature_player_damage(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        damage: u32,
        next_swing_at: Instant,
    ) -> anyhow::Result<Option<DbCreaturePlayerDamageEvent>> {
        let map = self.get_or_create_map(map_id, 0).await;
        let event = map
            .lock()
            .await
            .apply_db_creature_player_damage(attacker, victim, damage, next_swing_at);
        event
    }

    async fn apply_db_creature_player_melee_outcome(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        outcome: MeleeDamageOutcome,
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
            next_swing_at,
        );
        event
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
