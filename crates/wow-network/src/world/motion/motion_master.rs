use super::*;

#[derive(Debug, Clone, Default)]
pub(in crate::world) enum CreatureMotionState {
    #[default]
    Idle,
    Random(CreatureRandomMotion),
    Waypoint(CreatureWaypointMotion),
    Chase(CreatureChaseMotion),
    ReturnHome(CreatureReturnHomeMotion),
}

#[derive(Debug, Clone)]
pub(in crate::world) struct CreatureRandomMotion {
    pub(in crate::world) start: WorldPosition,
    pub(in crate::world) destination: WorldPosition,
    pub(in crate::world) path: Vec<WorldPosition>,
    pub(in crate::world) started_at: Instant,
    pub(in crate::world) duration: Duration,
}

#[derive(Debug, Clone)]
pub(in crate::world) struct CreatureWaypointMotion {
    pub(in crate::world) node_index: usize,
    pub(in crate::world) start: WorldPosition,
    pub(in crate::world) destination: WorldPosition,
    pub(in crate::world) path: Vec<WorldPosition>,
    pub(in crate::world) started_at: Instant,
    pub(in crate::world) duration: Duration,
}

#[derive(Debug, Clone)]
pub(in crate::world) struct CreatureChaseMotion {
    pub(in crate::world) target: ObjectGuid,
    pub(in crate::world) start: WorldPosition,
    pub(in crate::world) destination: WorldPosition,
    pub(in crate::world) path: Vec<WorldPosition>,
    pub(in crate::world) started_at: Instant,
    pub(in crate::world) duration: Duration,
    pub(in crate::world) recheck_at: Instant,
}

#[derive(Debug, Clone)]
pub(in crate::world) struct CreatureReturnHomeMotion {
    pub(in crate::world) start: WorldPosition,
    pub(in crate::world) destination: WorldPosition,
    pub(in crate::world) path: Vec<WorldPosition>,
    pub(in crate::world) started_at: Instant,
    pub(in crate::world) duration: Duration,
}
