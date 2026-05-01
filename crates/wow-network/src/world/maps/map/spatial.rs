// CMaNGOS-shaped nearby player and cell visitor helpers.

impl MapRuntime {
    fn nearby_player_guids(
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

    fn visit_nearby_cells(
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
