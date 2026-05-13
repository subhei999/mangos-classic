use super::*;

#[derive(Debug, Clone)]
pub(in crate::world) struct DbCreatureNavigationGuardrail {
    pub(in crate::world) line_of_sight_clear: bool,
    pub(in crate::world) path_available: bool,
    pub(in crate::world) world_data_files: Arc<WorldDataFiles>,
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
pub(in crate::world) enum DbCreatureNavigationResult {
    Clear,
    MapMismatch,
    InvalidCoordinate,
    LineOfSightBlocked,
    PathUnavailable,
}

impl DbCreatureNavigationResult {
    pub(in crate::world) fn is_clear(self) -> bool {
        self == Self::Clear
    }
}
