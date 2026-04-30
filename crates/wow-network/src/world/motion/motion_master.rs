#[derive(Debug, Clone, Default)]
enum CreatureMotionState {
    #[default]
    Idle,
    Random(CreatureRandomMotion),
    Waypoint(CreatureWaypointMotion),
    Chase(CreatureChaseMotion),
    ReturnHome(CreatureReturnHomeMotion),
}

#[derive(Debug, Clone)]
struct CreatureRandomMotion {
    start: WorldPosition,
    destination: WorldPosition,
    path: Vec<WorldPosition>,
    started_at: Instant,
    duration: Duration,
}

#[derive(Debug, Clone)]
struct CreatureWaypointMotion {
    node_index: usize,
    start: WorldPosition,
    destination: WorldPosition,
    path: Vec<WorldPosition>,
    started_at: Instant,
    duration: Duration,
}

#[derive(Debug, Clone)]
struct CreatureChaseMotion {
    target: ObjectGuid,
    start: WorldPosition,
    destination: WorldPosition,
    path: Vec<WorldPosition>,
    started_at: Instant,
    duration: Duration,
    recheck_at: Instant,
}

#[derive(Debug, Clone)]
struct CreatureReturnHomeMotion {
    start: WorldPosition,
    destination: WorldPosition,
    path: Vec<WorldPosition>,
    started_at: Instant,
    duration: Duration,
}
