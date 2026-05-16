use super::*;

// CMaNGOS-shaped nearby player and cell visitor helpers.

impl MapRuntime {
    pub(in crate::world) fn nearby_player_guids(
        &self,
        position: WorldPosition,
        radius: f32,
        exclude_guid: Option<u32>,
    ) -> Vec<u32> {
        let mut players = HashSet::new();
        self.visit_nearby_cells(position, radius, |cell| {
            players.extend(cell.players.iter().copied());
        });
        let mut players = players
            .into_iter()
            .filter(|guid| Some(*guid) != exclude_guid)
            .filter(|guid| {
                self.players.get(guid).is_some_and(|player| {
                    is_position_inside_radius(player.position, position, radius)
                })
            })
            .collect::<Vec<_>>();
        players.sort_unstable();
        players
    }

    pub(in crate::world) fn nearby_client_player_guids(
        &self,
        position: WorldPosition,
        radius: f32,
        exclude_guid: Option<u32>,
    ) -> Vec<u32> {
        let mut players = HashSet::new();
        self.visit_nearby_cells(position, radius, |cell| {
            players.extend(cell.client_players.iter().copied());
        });
        let mut players = players
            .into_iter()
            .filter(|guid| Some(*guid) != exclude_guid)
            .filter(|guid| {
                self.players.get(guid).is_some_and(|player| {
                    is_position_inside_radius(player.position, position, radius)
                })
            })
            .collect::<Vec<_>>();
        players.sort_unstable();
        players
    }

    pub(in crate::world) fn nearby_attackable_db_creature_guids_for_player_spell(
        &self,
        faction_templates: &FactionTemplateStore,
        character_guid: u32,
        radius: f32,
    ) -> Vec<ObjectGuid> {
        let Some(player) = self.players.get(&character_guid) else {
            return Vec::new();
        };
        let position = player.position;
        let mut raw_creatures = HashSet::new();
        self.visit_nearby_cells(position, radius, |cell| {
            raw_creatures.extend(cell.creatures.iter().copied());
        });
        let mut creatures = raw_creatures
            .into_iter()
            .filter_map(|raw_guid| {
                self.creatures
                    .get(&raw_guid)
                    .map(|creature| (raw_guid, creature))
            })
            .filter(|(_, creature)| {
                creature.is_alive()
                    && !creature.is_evading_home()
                    && is_position_inside_radius(creature.current_position, position, radius)
                    && can_player_attack_creature_with_spell(
                        faction_templates,
                        creature.spawn.template.faction,
                        player.race,
                    )
            })
            .map(|(raw_guid, _)| ObjectGuid::from_raw(raw_guid))
            .collect::<Vec<_>>();
        creatures.sort_unstable_by_key(|guid| guid.raw());
        creatures
    }

    pub(in crate::world) fn visit_nearby_cells(
        &self,
        position: WorldPosition,
        radius: f32,
        mut visitor: impl FnMut(&CellRuntime),
    ) {
        for (grid_coord, cell_coord) in calculate_cell_area(position, radius) {
            let Some(grid) = self.grids.get(&grid_coord) else {
                continue;
            };
            if let Some(cell) = grid.cells.get(&cell_coord) {
                visitor(cell);
            }
        }
    }
}
