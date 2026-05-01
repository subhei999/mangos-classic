#[derive(Debug, Clone)]
struct StartedCreatureMotion {
    start: WorldPosition,
    path: Vec<WorldPosition>,
    spline_id: u32,
    duration: Duration,
}

fn advance_db_creature_motion(
    session: &mut WorldSessionState,
    creature_guid: ObjectGuid,
    now: Instant,
) {
    let Some(creature) = session.db_creatures.get_mut(&creature_guid.raw()) else {
        return;
    };
    match &creature.motion {
        CreatureMotionState::Idle => {}
        CreatureMotionState::Random(random) => {
            let Some(position) = advance_timed_path_motion(
                random.start,
                &random.path,
                random.started_at,
                random.duration,
                now,
            ) else {
                creature.current_position = random.destination;
                creature.motion = CreatureMotionState::Idle;
                creature.next_random_move_at = Some(
                    now + Duration::from_millis(db_creature_random_pause_millis(
                        creature.guid().raw(),
                        creature.next_spline_id,
                    )),
                );
                creature.next_waypoint_move_at =
                    DbCreatureRuntime::initial_waypoint_move_at(&creature.spawn);
                return;
            };
            creature.current_position = position;
        }
        CreatureMotionState::Waypoint(waypoint) => {
            let Some(position) = advance_timed_path_motion(
                waypoint.start,
                &waypoint.path,
                waypoint.started_at,
                waypoint.duration,
                now,
            ) else {
                creature.current_position = waypoint.destination;
                let arrived_node = waypoint.node_index;
                let wait_time = creature
                    .spawn
                    .waypoint_path
                    .get(arrived_node)
                    .map(|node| node.wait_time)
                    .unwrap_or(0);
                advance_db_creature_waypoint_index(creature, arrived_node);
                creature.motion = CreatureMotionState::Idle;
                creature.next_waypoint_move_at =
                    Some(now + Duration::from_millis(wait_time as u64));
                return;
            };
            creature.current_position = position;
        }
        CreatureMotionState::Chase(chase) => {
            let Some(position) = advance_timed_path_motion(
                chase.start,
                &chase.path,
                chase.started_at,
                chase.duration,
                now,
            ) else {
                creature.current_position = chase.destination;
                creature.motion = CreatureMotionState::Idle;
                return;
            };
            creature.current_position = position;
        }
        CreatureMotionState::ReturnHome(return_home) => {
            let Some(position) = advance_timed_path_motion(
                return_home.start,
                &return_home.path,
                return_home.started_at,
                return_home.duration,
                now,
            ) else {
                creature.current_position = return_home.destination;
                creature.motion = CreatureMotionState::Idle;
                creature.already_called_assistance = false;
                creature.next_random_move_at = Some(
                    now + Duration::from_millis(db_creature_random_pause_millis(
                        creature.guid().raw(),
                        creature.next_spline_id,
                    )),
                );
                return;
            };
            creature.current_position = position;
        }
    }
}

fn advance_db_creature_waypoint_index(creature: &mut DbCreatureRuntime, arrived_node: usize) {
    let node_count = creature.spawn.waypoint_path.len();
    if node_count == 0 {
        creature.waypoint_next_index = 0;
        return;
    }
    if creature.default_movement_type() == DB_MOTION_TYPE_LINEAR_WAYPOINT && node_count > 1 {
        if creature.waypoint_forward && arrived_node + 1 >= node_count {
            creature.waypoint_forward = false;
        } else if !creature.waypoint_forward && arrived_node == 0 {
            creature.waypoint_forward = true;
        }
        creature.waypoint_next_index = if creature.waypoint_forward {
            arrived_node.saturating_add(1).min(node_count - 1)
        } else {
            arrived_node.saturating_sub(1)
        };
    } else {
        creature.waypoint_next_index = (arrived_node + 1) % node_count;
    }
}

fn start_db_creature_random_motion(
    session: &mut WorldSessionState,
    creature_guid: ObjectGuid,
    now: Instant,
) -> Option<StartedCreatureMotion> {
    let creature = session.db_creatures.get(&creature_guid.raw())?;
    if !matches!(creature.motion, CreatureMotionState::Idle) {
        return None;
    }
    let radius = creature.random_wander_radius();
    if radius <= 0.0 || creature.next_random_move_at.is_none_or(|at| now < at) {
        return None;
    }
    let start = creature.current_position;
    let raw_destination = db_creature_random_destination(
        creature.home_position,
        radius,
        creature.guid().raw(),
        creature.next_spline_id,
    )?;
    let path_result = db_creature_path_to_destination(
        &session.db_creature_navigation,
        start,
        raw_destination,
        CreaturePathMode::Full,
    )?;
    let path = path_result.points;
    let destination = *path.last()?;
    let move_distance = distance_2d(start.x, start.y, destination.x, destination.y);
    if move_distance <= f32::EPSILON {
        session
            .db_creatures
            .get_mut(&creature_guid.raw())?
            .next_random_move_at =
            Some(now + Duration::from_millis(DB_CREATURE_RANDOM_DELAY_MIN_MILLIS));
        return None;
    }
    let duration = db_creature_walk_path_motion_duration(start, &path);
    let creature = session.db_creatures.get_mut(&creature_guid.raw())?;
    let spline_id = creature.next_spline_id;
    creature.next_spline_id = creature.next_spline_id.wrapping_add(1);
    creature.motion = CreatureMotionState::Random(CreatureRandomMotion {
        start,
        destination,
        path: path.clone(),
        started_at: now,
        duration,
    });
    creature.next_random_move_at = None;
    Some(StartedCreatureMotion {
        start,
        path,
        spline_id,
        duration,
    })
}

fn start_db_creature_waypoint_motion(
    session: &mut WorldSessionState,
    creature_guid: ObjectGuid,
    now: Instant,
) -> Option<StartedCreatureMotion> {
    let creature = session.db_creatures.get(&creature_guid.raw())?;
    if !matches!(creature.motion, CreatureMotionState::Idle) {
        return None;
    }
    if !creature.has_waypoint_movement() || creature.next_waypoint_move_at.is_none_or(|at| now < at)
    {
        return None;
    }
    let node_index = creature
        .waypoint_next_index
        .min(creature.spawn.waypoint_path.len().saturating_sub(1));
    let node = creature.spawn.waypoint_path.get(node_index)?;
    let start = creature.current_position;
    let raw_destination = WorldPosition::new(
        creature.spawn.map,
        node.position_x,
        node.position_y,
        node.position_z,
        node.orientation.unwrap_or(creature.current_position.orientation),
    );
    let path_result = db_creature_path_to_destination(
        &session.db_creature_navigation,
        start,
        raw_destination,
        CreaturePathMode::Full,
    )?;
    let path = path_result.points;
    let destination = *path.last()?;
    let move_distance = distance_2d(start.x, start.y, destination.x, destination.y);
    if move_distance <= f32::EPSILON {
        let creature = session.db_creatures.get_mut(&creature_guid.raw())?;
        creature.current_position = destination;
        advance_db_creature_waypoint_index(creature, node_index);
        let wait_time = creature
            .spawn
            .waypoint_path
            .get(node_index)
            .map(|node| node.wait_time)
            .unwrap_or(0);
        creature.next_waypoint_move_at = Some(now + Duration::from_millis(wait_time as u64));
        return None;
    }
    let duration = db_creature_walk_path_motion_duration(start, &path);
    let creature = session.db_creatures.get_mut(&creature_guid.raw())?;
    let spline_id = creature.next_spline_id;
    creature.next_spline_id = creature.next_spline_id.wrapping_add(1);
    creature.motion = CreatureMotionState::Waypoint(CreatureWaypointMotion {
        node_index,
        start,
        destination,
        path: path.clone(),
        started_at: now,
        duration,
    });
    creature.next_waypoint_move_at = None;
    Some(StartedCreatureMotion {
        start,
        path,
        spline_id,
        duration,
    })
}

fn start_db_creature_chase_motion(
    session: &mut WorldSessionState,
    creature_guid: ObjectGuid,
    target: ObjectGuid,
    now: Instant,
) -> Option<StartedCreatureMotion> {
    let target_position = session.active_character.as_ref()?.position;
    let creature = session.db_creatures.get(&creature_guid.raw())?;
    let start = creature.current_position;
    let path_result = db_creature_chase_path(
        &session.db_creature_navigation,
        start,
        target_position,
    )?;
    let path = path_result.points;
    let destination = *path.last()?;
    if let CreatureMotionState::Chase(chase) = &creature.motion {
        if chase.target == target {
            if now < chase.recheck_at {
                return None;
            }
            let destination_delta = distance_2d(
                chase.destination.x,
                chase.destination.y,
                destination.x,
                destination.y,
            );
            if destination_delta <= DB_CREATURE_CHASE_REPATH_YARDS {
                return None;
            }
        }
    }
    let move_distance = distance_2d(start.x, start.y, destination.x, destination.y);
    if move_distance <= f32::EPSILON {
        return None;
    }
    let duration = db_creature_path_motion_duration(start, &path);
    let creature = session.db_creatures.get_mut(&creature_guid.raw())?;
    let spline_id = creature.next_spline_id;
    creature.next_spline_id = creature.next_spline_id.wrapping_add(1);
    creature.motion = CreatureMotionState::Chase(CreatureChaseMotion {
        target,
        start,
        destination,
        path: path.clone(),
        started_at: now,
        duration,
        recheck_at: now + Duration::from_millis(DB_CREATURE_CHASE_RECHECK_MILLIS),
    });
    Some(StartedCreatureMotion {
        start,
        path,
        spline_id,
        duration,
    })
}

fn start_db_creature_return_home_motion(
    session: &mut WorldSessionState,
    creature_guid: ObjectGuid,
    now: Instant,
) -> Option<StartedCreatureMotion> {
    let creature = session.db_creatures.get(&creature_guid.raw())?;
    let start = creature.current_position;
    let raw_destination = creature.home_position;
    if start.map_id != raw_destination.map_id
        || !world_position_is_finite(start)
        || !world_position_is_finite(raw_destination)
    {
        let creature = session.db_creatures.get_mut(&creature_guid.raw())?;
        creature.current_position = raw_destination;
        creature.motion = CreatureMotionState::Idle;
        return None;
    }
    let path_result = db_creature_path_to_destination(
        &session.db_creature_navigation,
        start,
        raw_destination,
        CreaturePathMode::Full,
    )?;
    let path = path_result.points;
    let destination = *path.last()?;
    let move_distance = distance_2d(start.x, start.y, destination.x, destination.y);
    if move_distance <= f32::EPSILON {
        let creature = session.db_creatures.get_mut(&creature_guid.raw())?;
        creature.current_position = destination;
        creature.motion = CreatureMotionState::Idle;
        return None;
    }
    let duration = db_creature_path_motion_duration(start, &path);
    let creature = session.db_creatures.get_mut(&creature_guid.raw())?;
    let spline_id = creature.next_spline_id;
    creature.next_spline_id = creature.next_spline_id.wrapping_add(1);
    creature.motion = CreatureMotionState::ReturnHome(CreatureReturnHomeMotion {
        start,
        destination,
        path: path.clone(),
        started_at: now,
        duration,
    });
    Some(StartedCreatureMotion {
        start,
        path,
        spline_id,
        duration,
    })
}

#[derive(Debug, Clone, Copy)]
enum CreaturePathMode {
    Full,
    StopShort(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DbCreaturePathFlags(u8);

impl DbCreaturePathFlags {
    const NORMAL: Self = Self(0x01);
    #[allow(dead_code)]
    const SHORTCUT: Self = Self(0x02);
    const INCOMPLETE: Self = Self(0x04);
    const NOPATH: Self = Self(0x08);
    const NOT_USING_PATH: Self = Self(0x10);
    #[allow(dead_code)]
    const SHORT: Self = Self(0x20);

    fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 != 0
    }

    fn union(self, flag: Self) -> Self {
        Self(self.0 | flag.0)
    }
}

#[derive(Debug, Clone)]
struct DbCreaturePath {
    flags: DbCreaturePathFlags,
    points: Vec<WorldPosition>,
}

#[derive(Debug)]
enum DbCreaturePathBuild {
    Path(DbCreaturePath),
    NoPath(DbCreaturePathFlags),
    Unavailable,
}

fn db_creature_chase_path(
    navigation: &DbCreatureNavigationGuardrail,
    start: WorldPosition,
    target_position: WorldPosition,
) -> Option<DbCreaturePath> {
    if !db_creature_pathing_check(navigation, start, target_position).is_clear() {
        return None;
    }
    let stop_distance = ATTACK_DISTANCE_YARDS * DB_CREATURE_CHASE_DEFAULT_RANGE_FACTOR;
    db_creature_path_to_destination(
        navigation,
        start,
        target_position,
        CreaturePathMode::StopShort(stop_distance),
    )
}

fn db_creature_path_to_destination(
    navigation: &DbCreatureNavigationGuardrail,
    start: WorldPosition,
    target_position: WorldPosition,
    mode: CreaturePathMode,
) -> Option<DbCreaturePath> {
    match db_creature_mmap_path(navigation, start, target_position, mode) {
        DbCreaturePathBuild::Path(path) => {
            debug_assert!(!path.flags.contains(DbCreaturePathFlags::NOPATH));
            Some(path)
        }
        DbCreaturePathBuild::NoPath(flags) => {
            debug_assert!(flags.contains(DbCreaturePathFlags::NOPATH));
            None
        }
        DbCreaturePathBuild::Unavailable => db_creature_straight_path(start, target_position, mode)
            .map(|points| DbCreaturePath {
                flags: DbCreaturePathFlags::NORMAL.union(DbCreaturePathFlags::NOT_USING_PATH),
                points,
            }),
    }
}

fn db_creature_straight_path(
    start: WorldPosition,
    target_position: WorldPosition,
    mode: CreaturePathMode,
) -> Option<Vec<WorldPosition>> {
    if start.map_id != target_position.map_id
        || !world_position_is_finite(start)
        || !world_position_is_finite(target_position)
    {
        return None;
    }
    let dx = target_position.x - start.x;
    let dy = target_position.y - start.y;
    let distance = distance_2d(start.x, start.y, target_position.x, target_position.y);
    let travel = match mode {
        CreaturePathMode::Full => distance,
        CreaturePathMode::StopShort(stop_distance) => {
            if distance <= stop_distance {
                return None;
            }
            distance - stop_distance
        }
    };
    if travel <= f32::EPSILON || distance <= f32::EPSILON {
        return None;
    }
    let nx = dx / distance;
    let ny = dy / distance;
    Some(vec![WorldPosition::new(
        start.map_id,
        start.x + nx * travel,
        start.y + ny * travel,
        start.z,
        dy.atan2(dx),
    )])
}

fn db_creature_navigation_check(
    navigation: &DbCreatureNavigationGuardrail,
    start: WorldPosition,
    target_position: WorldPosition,
) -> DbCreatureNavigationResult {
    if start.map_id != target_position.map_id {
        return DbCreatureNavigationResult::MapMismatch;
    }
    if !world_position_is_finite(start) || !world_position_is_finite(target_position) {
        return DbCreatureNavigationResult::InvalidCoordinate;
    }
    if !navigation.line_of_sight_clear
        || !db_creature_has_line_of_sight(navigation, start, target_position)
    {
        return DbCreatureNavigationResult::LineOfSightBlocked;
    }
    if !navigation.path_available || !db_creature_has_valid_path(navigation, start, target_position)
    {
        return DbCreatureNavigationResult::PathUnavailable;
    }
    DbCreatureNavigationResult::Clear
}

fn db_creature_pathing_check(
    navigation: &DbCreatureNavigationGuardrail,
    start: WorldPosition,
    target_position: WorldPosition,
) -> DbCreatureNavigationResult {
    if start.map_id != target_position.map_id {
        return DbCreatureNavigationResult::MapMismatch;
    }
    if !world_position_is_finite(start) || !world_position_is_finite(target_position) {
        return DbCreatureNavigationResult::InvalidCoordinate;
    }
    if !navigation.path_available || !db_creature_has_valid_path(navigation, start, target_position)
    {
        return DbCreatureNavigationResult::PathUnavailable;
    }
    DbCreatureNavigationResult::Clear
}

fn world_position_is_finite(position: WorldPosition) -> bool {
    position.x.is_finite()
        && position.y.is_finite()
        && position.z.is_finite()
        && position.orientation.is_finite()
}

fn db_creature_has_line_of_sight(
    navigation: &DbCreatureNavigationGuardrail,
    start: WorldPosition,
    target_position: WorldPosition,
) -> bool {
    if navigation.world_data_files.vmap_tiles.is_empty() {
        return true;
    }

    let Some(data_dir) = navigation.world_data_files.data_dir_for_native.as_ref() else {
        return true;
    };
    let map_id = start.map_id;
    let Some((start_tile_x, start_tile_y)) = mmap_tile_for_position(start) else {
        return false;
    };
    let Some((target_tile_x, target_tile_y)) = mmap_tile_for_position(target_position) else {
        return false;
    };
    if !navigation.world_data_files.has_vmap_support_for_map(map_id)
        || !navigation
            .world_data_files
            .has_vmap_tile(map_id, start_tile_x, start_tile_y)
        || !navigation
            .world_data_files
            .has_vmap_tile(map_id, target_tile_x, target_tile_y)
    {
        return true;
    }

    native_vmap_line_of_sight(
        data_dir,
        start,
        target_position,
        (start_tile_x, start_tile_y),
        (target_tile_x, target_tile_y),
        false,
    )
    .unwrap_or(true)
}

fn db_creature_has_valid_path(
    navigation: &DbCreatureNavigationGuardrail,
    start: WorldPosition,
    target_position: WorldPosition,
) -> bool {
    if navigation.world_data_files.mmap_tiles.is_empty() {
        return true;
    }

    let map_id = start.map_id;
    let Some((start_tile_x, start_tile_y)) = mmap_tile_for_position(start) else {
        return false;
    };
    let Some((target_tile_x, target_tile_y)) = mmap_tile_for_position(target_position) else {
        return false;
    };

    navigation.world_data_files.has_mmap_support_for_map(map_id)
        && navigation
            .world_data_files
            .has_mmap_tile(map_id, start_tile_x, start_tile_y)
        && navigation
            .world_data_files
            .has_mmap_tile(map_id, target_tile_x, target_tile_y)
}

fn mmap_tile_for_position(position: WorldPosition) -> Option<(u32, u32)> {
    const MAX_NUMBER_OF_GRIDS: i32 = 64;
    const CENTER_GRID_ID: f32 = 32.0;
    const SIZE_OF_GRIDS: f32 = 533.333_3;

    if !world_position_is_finite(position) {
        return None;
    }
    let tile_x = (CENTER_GRID_ID - position.x / SIZE_OF_GRIDS) as i32;
    let tile_y = (CENTER_GRID_ID - position.y / SIZE_OF_GRIDS) as i32;
    (0..MAX_NUMBER_OF_GRIDS)
        .contains(&tile_x)
        .then_some(())?;
    (0..MAX_NUMBER_OF_GRIDS)
        .contains(&tile_y)
        .then_some(())?;
    Some((tile_x as u32, tile_y as u32))
}

#[cfg(test)]
fn db_creature_mmap_next_path_corner(
    navigation: &DbCreatureNavigationGuardrail,
    start: WorldPosition,
    target_position: WorldPosition,
) -> Option<WorldPosition> {
    match db_creature_mmap_path(navigation, start, target_position, CreaturePathMode::Full) {
        DbCreaturePathBuild::Path(path) => path.points.into_iter().next(),
        DbCreaturePathBuild::NoPath(_) | DbCreaturePathBuild::Unavailable => None,
    }
}

fn db_creature_mmap_path(
    navigation: &DbCreatureNavigationGuardrail,
    start: WorldPosition,
    target_position: WorldPosition,
    mode: CreaturePathMode,
) -> DbCreaturePathBuild {
    let Some(data_dir) = navigation.world_data_files.data_dir_for_native.as_ref() else {
        return DbCreaturePathBuild::Unavailable;
    };
    let Some((start_tile_x, start_tile_y)) = mmap_tile_for_position(start) else {
        return DbCreaturePathBuild::Unavailable;
    };
    let Some((target_tile_x, target_tile_y)) = mmap_tile_for_position(target_position) else {
        return DbCreaturePathBuild::Unavailable;
    };
    if !navigation.world_data_files.has_mmap_support_for_map(start.map_id)
        || !navigation
            .world_data_files
            .has_mmap_tile(start.map_id, start_tile_x, start_tile_y)
        || !navigation
            .world_data_files
            .has_mmap_tile(start.map_id, target_tile_x, target_tile_y)
    {
        return DbCreaturePathBuild::Unavailable;
    }

    let native_path = native_mmap_find_path(
        data_dir,
        start,
        target_position,
        (start_tile_x, start_tile_y),
        (target_tile_x, target_tile_y),
    );
    let flags = match native_path.status {
        NativeMmapPathStatus::Normal => DbCreaturePathFlags::NORMAL,
        NativeMmapPathStatus::Incomplete => DbCreaturePathFlags::INCOMPLETE,
        NativeMmapPathStatus::NoPath
        | NativeMmapPathStatus::Unavailable
        | NativeMmapPathStatus::InvalidInput
        | NativeMmapPathStatus::NativeError => {
            return DbCreaturePathBuild::NoPath(DbCreaturePathFlags::NOPATH);
        }
    };
    let points = native_path.points;
    if points.len() < 2 {
        return DbCreaturePathBuild::NoPath(DbCreaturePathFlags::NOPATH);
    }

    let Some(path) = native_mmap_points_to_world_path(start, &points) else {
        return DbCreaturePathBuild::NoPath(DbCreaturePathFlags::NOPATH);
    };
    match db_creature_trim_path_for_mode(start, path, mode) {
        Some(path) => DbCreaturePathBuild::Path(DbCreaturePath {
            flags,
            points: path,
        }),
        None => DbCreaturePathBuild::NoPath(DbCreaturePathFlags::NOPATH),
    }
}

fn native_mmap_points_to_world_path(
    start: WorldPosition,
    points: &[WorldPosition],
) -> Option<Vec<WorldPosition>> {
    let mut path = Vec::new();
    let mut previous = start;
    for point in points.iter().skip(1) {
        if !world_position_is_finite(*point) {
            return None;
        }
        if distance_2d(previous.x, previous.y, point.x, point.y) <= f32::EPSILON {
            continue;
        }
        let position = WorldPosition::new(
            start.map_id,
            point.x,
            point.y,
            point.z,
            (point.y - previous.y).atan2(point.x - previous.x),
        );
        path.push(position);
        previous = position;
    }
    (!path.is_empty()).then_some(path)
}

fn db_creature_trim_path_for_mode(
    start: WorldPosition,
    path: Vec<WorldPosition>,
    mode: CreaturePathMode,
) -> Option<Vec<WorldPosition>> {
    match mode {
        CreaturePathMode::Full => (!path.is_empty()).then_some(path),
        CreaturePathMode::StopShort(stop_distance) => {
            db_creature_trim_path_to_travel_distance(start, path, stop_distance)
        }
    }
}

fn db_creature_trim_path_to_travel_distance(
    start: WorldPosition,
    path: Vec<WorldPosition>,
    stop_distance: f32,
) -> Option<Vec<WorldPosition>> {
    let total = path_distance_2d(start, &path);
    if total <= stop_distance {
        return None;
    }
    let target_distance = total - stop_distance;
    let mut remaining = target_distance;
    let mut previous = start;
    let mut trimmed = Vec::new();
    for point in path {
        let segment = distance_2d(previous.x, previous.y, point.x, point.y);
        if segment <= f32::EPSILON {
            previous = point;
            continue;
        }
        if remaining > segment {
            trimmed.push(point);
            remaining -= segment;
            previous = point;
            continue;
        }
        let progress = (remaining / segment).clamp(0.0, 1.0);
        if progress <= f32::EPSILON {
            break;
        }
        trimmed.push(interpolate_position(previous, point, progress));
        break;
    }
    (!trimmed.is_empty()).then_some(trimmed)
}

fn db_creature_path_motion_duration(start: WorldPosition, path: &[WorldPosition]) -> Duration {
    Duration::from_millis(
        ((path_distance_2d(start, path) / DB_CREATURE_RUN_SPEED_YARDS_PER_SEC) * 1000.0)
            .ceil()
            .max(1.0) as u64,
    )
}

fn db_creature_walk_path_motion_duration(
    start: WorldPosition,
    path: &[WorldPosition],
) -> Duration {
    Duration::from_millis(
        ((path_distance_2d(start, path) / DB_CREATURE_WALK_SPEED_YARDS_PER_SEC) * 1000.0)
            .ceil()
            .max(1.0) as u64,
    )
}

fn db_creature_random_destination(
    home: WorldPosition,
    radius: f32,
    guid: u64,
    spline_id: u32,
) -> Option<WorldPosition> {
    if radius <= 0.0 || !world_position_is_finite(home) {
        return None;
    }
    let angle_seed = db_creature_pseudo_random_unit(guid, spline_id, 0);
    let radius_seed = db_creature_pseudo_random_unit(guid, spline_id, 1);
    let angle = angle_seed * 2.0 * std::f32::consts::PI;
    let distance = radius * radius_seed.sqrt().clamp(0.2, 1.0);
    Some(WorldPosition::new(
        home.map_id,
        home.x + angle.cos() * distance,
        home.y + angle.sin() * distance,
        home.z,
        angle,
    ))
}

fn db_creature_random_pause_millis(guid: u64, spline_id: u32) -> u64 {
    let span = DB_CREATURE_RANDOM_DELAY_MAX_MILLIS - DB_CREATURE_RANDOM_DELAY_MIN_MILLIS;
    DB_CREATURE_RANDOM_DELAY_MIN_MILLIS
        + (db_creature_pseudo_random_unit(guid, spline_id, 2) * span as f32) as u64
}

fn db_creature_pseudo_random_unit(guid: u64, spline_id: u32, salt: u32) -> f32 {
    let mut value = guid
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((spline_id as u64) << 32)
        .wrapping_add(salt as u64);
    value ^= value >> 33;
    value = value.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    value ^= value >> 33;
    ((value & 0xFFFF_FFFF) as f32) / (u32::MAX as f32)
}

fn interpolate_position(
    start: WorldPosition,
    destination: WorldPosition,
    progress: f32,
) -> WorldPosition {
    WorldPosition::new(
        start.map_id,
        start.x + (destination.x - start.x) * progress,
        start.y + (destination.y - start.y) * progress,
        start.z + (destination.z - start.z) * progress,
        destination.orientation,
    )
}

fn advance_timed_path_motion(
    start: WorldPosition,
    path: &[WorldPosition],
    started_at: Instant,
    duration: Duration,
    now: Instant,
) -> Option<WorldPosition> {
    let elapsed = now.saturating_duration_since(started_at);
    if elapsed >= duration {
        return None;
    }
    let duration_secs = duration.as_secs_f32();
    if duration_secs <= f32::EPSILON || path.is_empty() {
        return None;
    }
    let travel_distance =
        path_distance_2d(start, path) * (elapsed.as_secs_f32() / duration_secs).clamp(0.0, 1.0);
    position_along_path(start, path, travel_distance)
}

fn position_along_path(
    start: WorldPosition,
    path: &[WorldPosition],
    mut travel_distance: f32,
) -> Option<WorldPosition> {
    let mut previous = start;
    for point in path {
        let segment = distance_2d(previous.x, previous.y, point.x, point.y);
        if segment <= f32::EPSILON {
            previous = *point;
            continue;
        }
        if travel_distance > segment {
            travel_distance -= segment;
            previous = *point;
            continue;
        }
        return Some(interpolate_position(
            previous,
            *point,
            (travel_distance / segment).clamp(0.0, 1.0),
        ));
    }
    path.last().copied()
}

fn path_distance_2d(start: WorldPosition, path: &[WorldPosition]) -> f32 {
    let mut distance = 0.0;
    let mut previous = start;
    for point in path {
        distance += distance_2d(previous.x, previous.y, point.x, point.y);
        previous = *point;
    }
    distance
}

fn distance_2d(left_x: f32, left_y: f32, right_x: f32, right_y: f32) -> f32 {
    let dx = left_x - right_x;
    let dy = left_y - right_y;
    (dx * dx + dy * dy).sqrt()
}

