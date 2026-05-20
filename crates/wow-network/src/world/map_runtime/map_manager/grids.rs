use super::*;

impl MapRuntimeManager {
    #[allow(dead_code)]
    pub(in crate::world) async fn share_db_creature_snapshots(
        &self,
        map_id: u32,
        creatures: Vec<DbCreatureRuntime>,
    ) -> Vec<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let creatures = map.lock().await.share_db_creature_snapshots(creatures);
        creatures
    }

    pub(in crate::world) async fn ensure_db_creature_grids_loaded(
        &self,
        character_db_pool: &MySqlPool,
        _world_db_pool: &MySqlPool,
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
            let lookup_started_at = Instant::now();
            let spawns = self
                .static_world_cache
                .creature_spawns_for_grid(map_id, grid);
            crate::observability::record_static_world_cache_lookup(
                crate::observability::StaticWorldCacheKind::Creature,
                lookup_started_at.elapsed(),
            );
            let mut spawns = spawns;
            apply_creature_display_scale_fallbacks(&mut spawns, &self.creature_display_scales);
            let spawn_count = spawns.len() as u64;
            let cache_lookups = self
                .creature_grid_load_db_queries
                .fetch_add(1, Ordering::Relaxed)
                + 1;
            let rows_loaded = self
                .creature_grid_load_rows
                .fetch_add(spawn_count, Ordering::Relaxed)
                + spawn_count;
            let instantiation_started_at = Instant::now();
            let runtimes =
                build_db_creature_runtimes_with_respawns(character_db_pool, spawns).await?;
            crate::observability::record_static_world_cache_instantiation(
                crate::observability::StaticWorldCacheKind::Creature,
                spawn_count,
                instantiation_started_at.elapsed(),
            );
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
                cache_lookups,
                rows_loaded,
                "Loaded static creature grid into MapRuntime"
            );
        }
        Ok(())
    }

    pub(in crate::world) async fn refresh_static_game_event_spawns(
        &self,
        character_db_pool: &MySqlPool,
        game_events: GameEventState,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        if !self
            .static_world_cache
            .replace_active_game_events(game_events)
        {
            return Ok(Vec::new());
        }

        let map_handles = {
            self.maps
                .lock()
                .await
                .iter()
                .map(|(key, map)| (*key, map.clone()))
                .collect::<Vec<_>>()
        };
        let mut packets = Vec::new();
        for ((map_id, _instance_id), map) in map_handles {
            let (creature_grids, gameobject_grids) = {
                let map = map.lock().await;
                (map.loaded_creature_grids(), map.loaded_gameobject_grids())
            };

            for grid in creature_grids {
                let mut spawns = self
                    .static_world_cache
                    .creature_spawns_for_grid(map_id, grid);
                apply_creature_display_scale_fallbacks(&mut spawns, &self.creature_display_scales);
                let runtimes =
                    build_db_creature_runtimes_with_respawns(character_db_pool, spawns).await?;
                packets.extend(
                    map.lock()
                        .await
                        .refresh_static_event_creature_grid(grid, runtimes)?,
                );
            }

            for grid in gameobject_grids {
                let spawns = self
                    .static_world_cache
                    .gameobject_spawns_for_grid(map_id, grid);
                let runtimes = spawns.into_iter().map(DbGameObjectRuntime::new).collect();
                packets.extend(
                    map.lock()
                        .await
                        .refresh_static_event_gameobject_grid(grid, runtimes, now)?,
                );
            }
        }
        Ok(packets)
    }
}

impl MapRuntimeManager {
    #[cfg(test)]
    pub(in crate::world) async fn ensure_db_creature_grids_loaded_for_test(
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

    #[cfg(test)]
    pub(in crate::world) async fn ensure_static_creature_grids_loaded_for_test(
        &self,
        map_id: u32,
        position: WorldPosition,
        radius: f32,
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
            let lookup_started_at = Instant::now();
            let spawns = self
                .static_world_cache
                .creature_spawns_for_grid(map_id, grid);
            crate::observability::record_static_world_cache_lookup(
                crate::observability::StaticWorldCacheKind::Creature,
                lookup_started_at.elapsed(),
            );
            let spawn_count = spawns.len() as u64;
            self.creature_grid_load_db_queries
                .fetch_add(1, Ordering::Relaxed);
            self.creature_grid_load_rows
                .fetch_add(spawn_count, Ordering::Relaxed);
            let instantiation_started_at = Instant::now();
            let runtimes = spawns.into_iter().map(DbCreatureRuntime::new).collect();
            crate::observability::record_static_world_cache_instantiation(
                crate::observability::StaticWorldCacheKind::Creature,
                spawn_count,
                instantiation_started_at.elapsed(),
            );
            map.lock().await.insert_loaded_creature_grid(grid, runtimes);
        }
    }

    #[allow(dead_code)]
    pub(in crate::world) fn creature_grid_load_stats(&self) -> CreatureGridLoadStats {
        CreatureGridLoadStats {
            ensure_calls: self.creature_grid_load_ensure_calls.load(Ordering::Relaxed),
            cache_hits: self.creature_grid_load_cache_hits.load(Ordering::Relaxed),
            db_queries: self.creature_grid_load_db_queries.load(Ordering::Relaxed),
            rows_loaded: self.creature_grid_load_rows.load(Ordering::Relaxed),
        }
    }

    pub(in crate::world) async fn nearby_db_creature_snapshots(
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

    pub(in crate::world) async fn stage_player_db_creature_visibility(
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

    pub(in crate::world) async fn stage_player_db_gameobject_visibility(
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
        let stage = map.lock().await.stage_player_db_gameobject_visibility(
            character_guid,
            position,
            nearby_gameobjects,
            now,
        );
        stage
    }

    pub(in crate::world) async fn ensure_db_gameobject_grids_loaded(
        &self,
        _world_db_pool: &MySqlPool,
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
            let lookup_started_at = Instant::now();
            let spawns = self
                .static_world_cache
                .gameobject_spawns_for_grid(map_id, grid);
            crate::observability::record_static_world_cache_lookup(
                crate::observability::StaticWorldCacheKind::GameObject,
                lookup_started_at.elapsed(),
            );
            let spawn_count = spawns.len() as u64;
            let instantiation_started_at = Instant::now();
            let runtimes = spawns.into_iter().map(DbGameObjectRuntime::new).collect();
            crate::observability::record_static_world_cache_instantiation(
                crate::observability::StaticWorldCacheKind::GameObject,
                spawn_count,
                instantiation_started_at.elapsed(),
            );
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
                spawn_count,
                "Loaded static gameobject grid into MapRuntime"
            );
        }
        Ok(())
    }

    #[cfg(test)]
    pub(in crate::world) async fn ensure_db_gameobject_grids_loaded_for_test(
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

    #[cfg(test)]
    pub(in crate::world) async fn ensure_static_gameobject_grids_loaded_for_test(
        &self,
        map_id: u32,
        position: WorldPosition,
        radius: f32,
    ) {
        let map = self.get_or_create_map(map_id, 0).await;
        let grids = {
            map.lock()
                .await
                .unloaded_gameobject_grids_for_area(position, radius)
        };
        for grid in grids {
            let lookup_started_at = Instant::now();
            let spawns = self
                .static_world_cache
                .gameobject_spawns_for_grid(map_id, grid);
            crate::observability::record_static_world_cache_lookup(
                crate::observability::StaticWorldCacheKind::GameObject,
                lookup_started_at.elapsed(),
            );
            let spawn_count = spawns.len() as u64;
            let instantiation_started_at = Instant::now();
            let runtimes = spawns.into_iter().map(DbGameObjectRuntime::new).collect();
            crate::observability::record_static_world_cache_instantiation(
                crate::observability::StaticWorldCacheKind::GameObject,
                spawn_count,
                instantiation_started_at.elapsed(),
            );
            map.lock()
                .await
                .insert_loaded_gameobject_grid(grid, runtimes);
        }
    }

    pub(in crate::world) async fn nearby_db_gameobject_snapshots(
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

    pub(in crate::world) async fn ensure_player_corpse_grids_loaded(
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

    pub(in crate::world) async fn nearby_player_corpse_snapshots(
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

    pub(in crate::world) async fn stage_player_corpse_visibility(
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
        let stage = map.lock().await.stage_player_corpse_visibility(
            character_guid,
            position,
            nearby_corpses,
        );
        stage
    }

    pub(in crate::world) async fn upsert_player_corpse(
        &self,
        map_id: u32,
        corpse: PlayerCorpseRuntime,
    ) {
        let map = self.get_or_create_map(map_id, 0).await;
        map.lock().await.upsert_player_corpse(corpse);
    }
}
