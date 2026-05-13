use super::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::world) struct StaticWorldCacheCounts {
    pub(in crate::world) creature_spawns: u64,
    pub(in crate::world) creature_grids: u64,
    pub(in crate::world) gameobject_spawns: u64,
    pub(in crate::world) gameobject_grids: u64,
}

#[derive(Debug, Default)]
pub(in crate::world) struct StaticWorldSpawnCache {
    pub(in crate::world) creature_spawns_by_grid:
        HashMap<(u32, GridCoord), Vec<CreatureSpawnQuery>>,
    pub(in crate::world) gameobject_spawns_by_grid:
        HashMap<(u32, GridCoord), Vec<wow_db::GameObjectSpawnQuery>>,
    pub(in crate::world) active_game_events: RwLock<GameEventState>,
}

impl StaticWorldSpawnCache {
    #[cfg(test)]
    pub(in crate::world) fn from_spawns(
        creature_spawns: Vec<CreatureSpawnQuery>,
        gameobject_spawns: Vec<wow_db::GameObjectSpawnQuery>,
    ) -> Self {
        Self::from_spawns_for_game_events(
            creature_spawns,
            gameobject_spawns,
            &GameEventState::default(),
        )
    }

    pub(in crate::world) fn from_spawns_for_game_events(
        creature_spawns: Vec<CreatureSpawnQuery>,
        gameobject_spawns: Vec<wow_db::GameObjectSpawnQuery>,
        game_events: &GameEventState,
    ) -> Self {
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

        Self {
            creature_spawns_by_grid,
            gameobject_spawns_by_grid,
            active_game_events: RwLock::new(game_events.clone()),
        }
    }

    pub(in crate::world) fn counts(&self) -> StaticWorldCacheCounts {
        let game_events = self.active_game_events();
        let creature_spawns = self
            .creature_spawns_by_grid
            .values()
            .flat_map(|spawns| spawns.iter())
            .filter(|spawn| game_events.spawn_is_active(spawn.game_event))
            .count() as u64;
        let creature_grids = self
            .creature_spawns_by_grid
            .iter()
            .filter(|(_, spawns)| {
                spawns
                    .iter()
                    .any(|spawn| game_events.spawn_is_active(spawn.game_event))
            })
            .count() as u64;
        let gameobject_spawns = self
            .gameobject_spawns_by_grid
            .values()
            .flat_map(|spawns| spawns.iter())
            .filter(|spawn| game_events.spawn_is_active(spawn.game_event))
            .count() as u64;
        let gameobject_grids = self
            .gameobject_spawns_by_grid
            .iter()
            .filter(|(_, spawns)| {
                spawns
                    .iter()
                    .any(|spawn| game_events.spawn_is_active(spawn.game_event))
            })
            .count() as u64;
        StaticWorldCacheCounts {
            creature_spawns,
            creature_grids,
            gameobject_spawns,
            gameobject_grids,
        }
    }

    pub(in crate::world) fn active_game_events(&self) -> GameEventState {
        self.active_game_events
            .read()
            .map(|events| events.clone())
            .unwrap_or_default()
    }

    pub(in crate::world) fn replace_active_game_events(&self, game_events: GameEventState) -> bool {
        let Ok(mut active_game_events) = self.active_game_events.write() else {
            return false;
        };
        if *active_game_events == game_events {
            return false;
        }
        *active_game_events = game_events;
        true
    }

    pub(in crate::world) fn creature_spawns_for_grid(
        &self,
        map_id: u32,
        grid: GridCoord,
    ) -> Vec<CreatureSpawnQuery> {
        let game_events = self.active_game_events();
        self.creature_spawns_by_grid
            .get(&(map_id, grid))
            .map(|spawns| {
                spawns
                    .iter()
                    .filter(|spawn| game_events.spawn_is_active(spawn.game_event))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(in crate::world) fn gameobject_spawns_for_grid(
        &self,
        map_id: u32,
        grid: GridCoord,
    ) -> Vec<wow_db::GameObjectSpawnQuery> {
        let game_events = self.active_game_events();
        self.gameobject_spawns_by_grid
            .get(&(map_id, grid))
            .map(|spawns| {
                spawns
                    .iter()
                    .filter(|spawn| game_events.spawn_is_active(spawn.game_event))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }
}
