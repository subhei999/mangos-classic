use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::world) struct GridCoord {
    pub(in crate::world) x: u32,
    pub(in crate::world) y: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::world) struct CellCoord {
    pub(in crate::world) x: u32,
    pub(in crate::world) y: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum GridState {
    Loaded,
    Active,
    Idle,
    UnloadBlocked(GridUnloadBlocker),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum GridUnloadBlocker {
    Combat,
    Loot,
    Corpse,
    Timer,
}

#[derive(Debug, Default)]
#[allow(dead_code)]
pub(in crate::world) struct CellRuntime {
    pub(in crate::world) players: HashSet<u32>,
    pub(in crate::world) client_players: HashSet<u32>,
    pub(in crate::world) creatures: HashSet<u64>,
    pub(in crate::world) gameobjects: HashSet<u64>,
    pub(in crate::world) corpses: HashSet<u64>,
}

#[derive(Debug)]
pub(in crate::world) struct GridRuntime {
    pub(in crate::world) state: GridState,
    pub(in crate::world) cells: HashMap<CellCoord, CellRuntime>,
    pub(in crate::world) active_player_count: u32,
    pub(in crate::world) last_touched: Instant,
}

impl Default for GridRuntime {
    fn default() -> Self {
        Self {
            state: GridState::Loaded,
            cells: HashMap::new(),
            active_player_count: 0,
            last_touched: Instant::now(),
        }
    }
}

pub(in crate::world) const PLAYER_VISIBILITY_RADIUS_YARDS: f32 = CREATURE_SPAWN_RADIUS_YARDS;
pub(in crate::world) const CHAT_SAY_RADIUS_YARDS: f32 = 25.0;
pub(in crate::world) const CHAT_YELL_RADIUS_YARDS: f32 = 300.0;
pub(in crate::world) const CHAT_EMOTE_RADIUS_YARDS: f32 = CHAT_SAY_RADIUS_YARDS;
pub(in crate::world) const MAX_NUMBER_OF_GRIDS: u32 = 64;
pub(in crate::world) const MAX_NUMBER_OF_CELLS: u32 = 16;
pub(in crate::world) const MAP_SIZE_YARDS: f32 = 34133.333;
pub(in crate::world) const GRID_SIZE_YARDS: f32 = 533.333_3;
pub(in crate::world) const CELL_COUNT_PER_GRID: f32 = MAX_NUMBER_OF_CELLS as f32;
pub(in crate::world) const CELL_SIZE_YARDS: f32 = GRID_SIZE_YARDS / CELL_COUNT_PER_GRID;
pub(in crate::world) const TOTAL_CELL_COUNT_PER_AXIS: u32 =
    MAX_NUMBER_OF_GRIDS * MAX_NUMBER_OF_CELLS;
pub(in crate::world) const GRID_UNLOAD_DELAY_MILLIS: u64 = 60_000;

pub(in crate::world) fn grid_coord_for_position(position: WorldPosition) -> GridCoord {
    GridCoord {
        x: global_cell_axis_for_world_axis(position.y) / MAX_NUMBER_OF_CELLS,
        y: global_cell_axis_for_world_axis(position.x) / MAX_NUMBER_OF_CELLS,
    }
}

pub(in crate::world) fn cell_coord_for_position(position: WorldPosition) -> CellCoord {
    let global_x = global_cell_axis_for_world_axis(position.y);
    let global_y = global_cell_axis_for_world_axis(position.x);
    CellCoord {
        x: global_x % MAX_NUMBER_OF_CELLS,
        y: global_y % MAX_NUMBER_OF_CELLS,
    }
}

pub(in crate::world) fn calculate_cell_area(
    position: WorldPosition,
    radius: f32,
) -> Vec<(GridCoord, CellCoord)> {
    let radius = radius.max(0.0);
    let min_global_x = global_cell_axis_for_world_axis(position.y + radius);
    let max_global_x = global_cell_axis_for_world_axis(position.y - radius);
    let min_global_y = global_cell_axis_for_world_axis(position.x + radius);
    let max_global_y = global_cell_axis_for_world_axis(position.x - radius);

    let mut cells = Vec::new();
    for global_x in min_global_x..=max_global_x {
        for global_y in min_global_y..=max_global_y {
            cells.push((
                GridCoord {
                    x: global_x / MAX_NUMBER_OF_CELLS,
                    y: global_y / MAX_NUMBER_OF_CELLS,
                },
                CellCoord {
                    x: global_x % MAX_NUMBER_OF_CELLS,
                    y: global_y % MAX_NUMBER_OF_CELLS,
                },
            ));
        }
    }
    cells
}

pub(in crate::world) fn global_cell_axis_for_world_axis(axis: f32) -> u32 {
    let half = MAP_SIZE_YARDS / 2.0;
    ((half - axis) / CELL_SIZE_YARDS)
        .floor()
        .clamp(0.0, (TOTAL_CELL_COUNT_PER_AXIS - 1) as f32) as u32
}

pub(in crate::world) fn grid_world_bounds(grid: GridCoord) -> (f32, f32, f32, f32) {
    let min_x = world_axis_min_for_grid_axis(grid.y);
    let max_x = world_axis_max_for_grid_axis(grid.y);
    let min_y = world_axis_min_for_grid_axis(grid.x);
    let max_y = world_axis_max_for_grid_axis(grid.x);
    (min_x, max_x, min_y, max_y)
}

pub(in crate::world) fn world_axis_min_for_grid_axis(grid_axis: u32) -> f32 {
    let half = MAP_SIZE_YARDS / 2.0;
    half - ((grid_axis + 1) as f32 * GRID_SIZE_YARDS)
}

pub(in crate::world) fn world_axis_max_for_grid_axis(grid_axis: u32) -> f32 {
    let half = MAP_SIZE_YARDS / 2.0;
    half - (grid_axis as f32 * GRID_SIZE_YARDS)
}
