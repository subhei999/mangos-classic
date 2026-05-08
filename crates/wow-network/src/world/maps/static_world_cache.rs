#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct StaticWorldCacheCounts {
    creature_spawns: u64,
    creature_grids: u64,
    gameobject_spawns: u64,
    gameobject_grids: u64,
}

#[derive(Debug, Default)]
struct StaticWorldSpawnCache {
    creature_spawns_by_grid: HashMap<(u32, GridCoord), Vec<CreatureSpawnQuery>>,
    gameobject_spawns_by_grid: HashMap<(u32, GridCoord), Vec<wow_db::GameObjectSpawnQuery>>,
    counts: StaticWorldCacheCounts,
}

impl StaticWorldSpawnCache {
    #[cfg(test)]
    fn from_spawns(
        creature_spawns: Vec<CreatureSpawnQuery>,
        gameobject_spawns: Vec<wow_db::GameObjectSpawnQuery>,
    ) -> Self {
        Self::from_spawns_for_game_events(
            creature_spawns,
            gameobject_spawns,
            &GameEventState::default(),
        )
    }

    fn from_spawns_for_game_events(
        creature_spawns: Vec<CreatureSpawnQuery>,
        gameobject_spawns: Vec<wow_db::GameObjectSpawnQuery>,
        game_events: &GameEventState,
    ) -> Self {
        let creature_spawns = creature_spawns
            .into_iter()
            .filter(|spawn| game_events.spawn_is_active(spawn.game_event))
            .collect::<Vec<_>>();
        let gameobject_spawns = gameobject_spawns
            .into_iter()
            .filter(|spawn| game_events.spawn_is_active(spawn.game_event))
            .collect::<Vec<_>>();
        let creature_spawn_count = creature_spawns.len() as u64;
        let gameobject_spawn_count = gameobject_spawns.len() as u64;
        let mut creature_spawns_by_grid: HashMap<(u32, GridCoord), Vec<CreatureSpawnQuery>> =
            HashMap::new();
        let mut gameobject_spawns_by_grid: HashMap<
            (u32, GridCoord),
            Vec<wow_db::GameObjectSpawnQuery>,
        > = HashMap::new();

        for spawn in creature_spawns {
            let position = WorldPosition::new(
                spawn.map,
                spawn.position_x,
                spawn.position_y,
                spawn.position_z,
                spawn.orientation,
            );
            creature_spawns_by_grid
                .entry((spawn.map, grid_coord_for_position(position)))
                .or_default()
                .push(spawn);
        }

        for spawn in gameobject_spawns {
            let position = WorldPosition::new(
                spawn.map,
                spawn.position_x,
                spawn.position_y,
                spawn.position_z,
                spawn.orientation,
            );
            gameobject_spawns_by_grid
                .entry((spawn.map, grid_coord_for_position(position)))
                .or_default()
                .push(spawn);
        }

        for spawns in creature_spawns_by_grid.values_mut() {
            spawns.sort_by_key(|spawn| spawn.guid);
        }
        for spawns in gameobject_spawns_by_grid.values_mut() {
            spawns.sort_by_key(|spawn| spawn.guid);
        }

        let counts = StaticWorldCacheCounts {
            creature_spawns: creature_spawn_count,
            creature_grids: creature_spawns_by_grid.len() as u64,
            gameobject_spawns: gameobject_spawn_count,
            gameobject_grids: gameobject_spawns_by_grid.len() as u64,
        };

        Self {
            creature_spawns_by_grid,
            gameobject_spawns_by_grid,
            counts,
        }
    }

    fn counts(&self) -> StaticWorldCacheCounts {
        self.counts
    }

    fn creature_spawns_for_grid(&self, map_id: u32, grid: GridCoord) -> Vec<CreatureSpawnQuery> {
        self.creature_spawns_by_grid
            .get(&(map_id, grid))
            .cloned()
            .unwrap_or_default()
    }

    fn gameobject_spawns_for_grid(
        &self,
        map_id: u32,
        grid: GridCoord,
    ) -> Vec<wow_db::GameObjectSpawnQuery> {
        self.gameobject_spawns_by_grid
            .get(&(map_id, grid))
            .cloned()
            .unwrap_or_default()
    }
}
