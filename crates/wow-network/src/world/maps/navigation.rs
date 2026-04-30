#[derive(Debug, Clone)]
struct DbCreatureNavigationGuardrail {
    line_of_sight_clear: bool,
    path_available: bool,
    world_data_files: Arc<WorldDataFiles>,
}

impl Default for DbCreatureNavigationGuardrail {
    fn default() -> Self {
        Self {
            line_of_sight_clear: true,
            path_available: true,
            world_data_files: Arc::new(WorldDataFiles::fallback()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DbCreatureNavigationResult {
    Clear,
    MapMismatch,
    InvalidCoordinate,
    LineOfSightBlocked,
    PathUnavailable,
}

impl DbCreatureNavigationResult {
    fn is_clear(self) -> bool {
        self == Self::Clear
    }
}
