use super::*;
use wow_proto::world::WorldOpcode;

#[derive(Debug, Clone)]
pub(in crate::world) struct StartedCreatureMotion {
    pub(in crate::world) start: WorldPosition,
    pub(in crate::world) path: Vec<WorldPosition>,
    pub(in crate::world) spline_id: u32,
    pub(in crate::world) duration: Duration,
    pub(in crate::world) run: bool,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct StoppedCreatureMotion {
    pub(in crate::world) position: WorldPosition,
    pub(in crate::world) spline_id: u32,
}

// The LOS-only straight chase path reused the creature's start Z for the
// destination, which made wolves hover/jitter on uneven Northshire terrain.
// Keep chase on mmap-backed paths until the fast path can sample terrain/nav
// height like CMaNGOS' straight-line PathFinder branch.
pub(in crate::world) const DB_CREATURE_CHASE_STRAIGHT_FAST_PATH_ENABLED: bool = false;
pub(in crate::world) const CMANGOS_CONFUSED_MOVEMENT_RADIUS_YARDS: f32 = 2.5;
pub(in crate::world) const CMANGOS_CONFUSED_MOVEMENT_DELAY_MIN_MILLIS: u64 = 500;
pub(in crate::world) const CMANGOS_CONFUSED_MOVEMENT_DELAY_MAX_MILLIS: u64 = 1500;

#[cfg(test)]
pub(in crate::world) fn advance_db_creature_motion(
    session: &mut WorldSessionState,
    creature_guid: ObjectGuid,
    now: Instant,
) {
    let Some(creature) = session
        .visibility
        .db_creatures
        .get_mut(&creature_guid.raw())
    else {
        return;
    };
    advance_db_creature_motion_runtime(creature, now);
}

pub(in crate::world) fn advance_db_creature_motion_runtime(
    creature: &mut DbCreatureRuntime,
    now: Instant,
) {
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
                let pause_millis = if active_aura_has_confuse(&creature.active_auras) {
                    db_creature_confused_pause_millis(
                        creature.guid().raw(),
                        creature.next_spline_id,
                    )
                } else {
                    db_creature_random_pause_millis(creature.guid().raw(), creature.next_spline_id)
                };
                creature.next_random_move_at = Some(now + Duration::from_millis(pause_millis));
                creature.next_waypoint_move_at =
                    DbCreatureRuntime::initial_waypoint_move_at(&creature.spawn);
                return;
            };
            creature.current_position = position;
        }
        CreatureMotionState::Confused(confused) => {
            let Some(position) = advance_timed_path_motion(
                confused.start,
                &confused.path,
                confused.started_at,
                confused.duration,
                now,
            ) else {
                creature.current_position = confused.destination;
                creature.motion = CreatureMotionState::Idle;
                creature.next_confused_move_at = Some(
                    now + Duration::from_millis(db_creature_confused_pause_millis(
                        creature.guid().raw(),
                        creature.next_spline_id,
                    )),
                );
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
                queue_db_creature_waypoint_script(creature, arrived_node);
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
        CreatureMotionState::Flee(flee) => {
            let position = advance_timed_path_motion(
                flee.start,
                &flee.path,
                flee.started_at,
                flee.duration,
                now,
            );
            if let Some(position) = position {
                creature.current_position = position;
                return;
            }
            creature.current_position = flee.destination;
            if now >= flee.flee_until {
                creature.motion = CreatureMotionState::Idle;
            }
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
                creature.waypoint_resume_position = None;
                if creature.has_waypoint_movement() {
                    creature.next_random_move_at = None;
                    creature.next_waypoint_move_at = Some(now);
                } else {
                    creature.next_random_move_at = Some(
                        now + Duration::from_millis(db_creature_random_pause_millis(
                            creature.guid().raw(),
                            creature.next_spline_id,
                        )),
                    );
                    creature.next_waypoint_move_at =
                        DbCreatureRuntime::initial_waypoint_move_at(&creature.spawn);
                }
                return;
            };
            creature.current_position = position;
        }
    }
}

pub(in crate::world) fn queue_db_creature_waypoint_script(
    creature: &mut DbCreatureRuntime,
    node_index: usize,
) {
    if let Some(script_id) = creature
        .spawn
        .waypoint_path
        .get(node_index)
        .map(|node| node.script_id)
        .filter(|script_id| *script_id != 0)
    {
        creature.pending_movement_scripts.push(script_id);
    }
}

pub(in crate::world) fn advance_db_creature_waypoint_index(
    creature: &mut DbCreatureRuntime,
    arrived_node: usize,
) {
    let node_count = creature.spawn.waypoint_path.len();
    let (next_index, next_forward) = db_creature_next_waypoint_state(
        node_count,
        creature.default_movement_type(),
        arrived_node,
        creature.waypoint_forward,
    );
    creature.waypoint_next_index = next_index;
    creature.waypoint_forward = next_forward;
}

pub(in crate::world) fn db_creature_next_waypoint_state(
    node_count: usize,
    movement_type: u8,
    arrived_node: usize,
    waypoint_forward: bool,
) -> (usize, bool) {
    if node_count == 0 {
        return (0, waypoint_forward);
    }
    if movement_type == DB_MOTION_TYPE_LINEAR_WAYPOINT && node_count > 1 {
        let next_forward = if waypoint_forward && arrived_node + 1 >= node_count {
            false
        } else if !waypoint_forward && arrived_node == 0 {
            true
        } else {
            waypoint_forward
        };
        let next_index = if next_forward {
            arrived_node.saturating_add(1).min(node_count - 1)
        } else {
            arrived_node.saturating_sub(1)
        };
        return (next_index, next_forward);
    }
    ((arrived_node + 1) % node_count, waypoint_forward)
}

#[cfg(test)]
pub(in crate::world) fn start_db_creature_random_motion(
    session: &mut WorldSessionState,
    creature_guid: ObjectGuid,
    now: Instant,
) -> Option<StartedCreatureMotion> {
    let creature = session
        .visibility
        .db_creatures
        .get_mut(&creature_guid.raw())?;
    start_db_creature_random_motion_runtime(
        &session.movement.db_creature_navigation,
        None,
        creature,
        now,
    )
}

pub(in crate::world) fn start_db_creature_random_motion_runtime(
    navigation: &DbCreatureNavigationGuardrail,
    geometry: Option<&WorldGeometry>,
    creature: &mut DbCreatureRuntime,
    now: Instant,
) -> Option<StartedCreatureMotion> {
    if active_aura_blocks_movement(&creature.active_auras)
        || active_auras_suppress_hostile_refs(&creature.active_auras)
    {
        return None;
    }
    if !matches!(creature.motion, CreatureMotionState::Idle) {
        return None;
    }
    let radius = creature.random_wander_radius();
    if radius <= 0.0 || creature.next_random_move_at.is_none_or(|at| now < at) {
        return None;
    }
    let start = creature.current_position;
    let Some(path_result) = db_creature_random_path(navigation, geometry, creature, start, radius)
    else {
        creature.next_random_move_at =
            Some(now + Duration::from_millis(DB_CREATURE_IDLE_MOTION_FAILED_RETRY_MILLIS));
        return None;
    };
    let path = path_result.points;
    let destination = *path.last()?;
    let move_distance = distance_2d(start.x, start.y, destination.x, destination.y);
    if move_distance <= f32::EPSILON {
        creature.next_random_move_at =
            Some(now + Duration::from_millis(DB_CREATURE_RANDOM_DELAY_MIN_MILLIS));
        return None;
    }
    let run = creature.default_movement_run;
    let duration = db_creature_random_motion_duration(creature, start, &path, run);
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
        run,
    })
}

pub(in crate::world) fn start_db_creature_confused_motion_runtime(
    navigation: &DbCreatureNavigationGuardrail,
    geometry: Option<&WorldGeometry>,
    creature: &mut DbCreatureRuntime,
    now: Instant,
) -> Option<StartedCreatureMotion> {
    if !active_aura_has_confuse(&creature.active_auras)
        || active_aura_blocks_movement(&creature.active_auras)
    {
        return None;
    }
    if !matches!(creature.motion, CreatureMotionState::Idle) {
        return None;
    }
    let start = creature.current_position;
    let center = creature.confused_origin.unwrap_or(start);
    let Some(path_result) = db_creature_random_path_from_center(
        navigation,
        geometry,
        creature,
        start,
        center,
        CMANGOS_CONFUSED_MOVEMENT_RADIUS_YARDS,
    ) else {
        creature.next_confused_move_at =
            Some(now + Duration::from_millis(CMANGOS_CONFUSED_MOVEMENT_DELAY_MIN_MILLIS));
        return None;
    };
    let path = path_result.points;
    let destination = *path.last()?;
    if distance_2d(start.x, start.y, destination.x, destination.y) <= f32::EPSILON {
        creature.next_confused_move_at =
            Some(now + Duration::from_millis(CMANGOS_CONFUSED_MOVEMENT_DELAY_MIN_MILLIS));
        return None;
    }
    let run = false;
    let duration = db_creature_random_motion_duration(creature, start, &path, run);
    let spline_id = creature.next_spline_id;
    creature.next_spline_id = creature.next_spline_id.wrapping_add(1);
    creature.motion = CreatureMotionState::Confused(CreatureRandomMotion {
        start,
        destination,
        path: path.clone(),
        started_at: now,
        duration,
    });
    creature.next_confused_move_at = None;
    Some(StartedCreatureMotion {
        start,
        path,
        spline_id,
        duration,
        run,
    })
}

#[cfg(test)]
pub(in crate::world) fn start_db_creature_waypoint_motion(
    session: &mut WorldSessionState,
    creature_guid: ObjectGuid,
    now: Instant,
) -> Option<StartedCreatureMotion> {
    let creature = session
        .visibility
        .db_creatures
        .get_mut(&creature_guid.raw())?;
    start_db_creature_waypoint_motion_runtime(
        &session.movement.db_creature_navigation,
        None,
        creature,
        now,
    )
}

pub(in crate::world) fn start_db_creature_waypoint_motion_runtime(
    navigation: &DbCreatureNavigationGuardrail,
    geometry: Option<&WorldGeometry>,
    creature: &mut DbCreatureRuntime,
    now: Instant,
) -> Option<StartedCreatureMotion> {
    if active_aura_blocks_movement(&creature.active_auras)
        || active_auras_suppress_hostile_refs(&creature.active_auras)
    {
        return None;
    }
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
    let start = creature.current_position;
    let node = creature.spawn.waypoint_path.get(node_index)?;
    let node_wait_time = node.wait_time;
    let raw_destination = WorldPosition::new(
        creature.spawn.map,
        node.position_x,
        node.position_y,
        node.position_z,
        node.orientation.unwrap_or(start.orientation),
    );
    if distance_2d(start.x, start.y, raw_destination.x, raw_destination.y) <= f32::EPSILON {
        creature.current_position = raw_destination;
        queue_db_creature_waypoint_script(creature, node_index);
        advance_db_creature_waypoint_index(creature, node_index);
        creature.next_waypoint_move_at = Some(now + Duration::from_millis(node_wait_time as u64));
        return None;
    }
    let Some((path, arrived_node)) =
        db_creature_waypoint_buffered_path(navigation, geometry, creature, start, node_index)
    else {
        creature.next_waypoint_move_at =
            Some(now + Duration::from_millis(DB_CREATURE_IDLE_MOTION_FAILED_RETRY_MILLIS));
        return None;
    };
    let destination = *path.last()?;
    let move_distance = distance_2d(start.x, start.y, destination.x, destination.y);
    if move_distance <= f32::EPSILON {
        creature.current_position = destination;
        queue_db_creature_waypoint_script(creature, arrived_node);
        advance_db_creature_waypoint_index(creature, arrived_node);
        let wait_time = creature
            .spawn
            .waypoint_path
            .get(arrived_node)
            .map(|node| node.wait_time)
            .unwrap_or(0);
        creature.next_waypoint_move_at = Some(now + Duration::from_millis(wait_time as u64));
        return None;
    }
    let run = creature.default_movement_run;
    let duration = db_creature_motion_duration_for_run(creature, start, &path, run);
    let spline_id = creature.next_spline_id;
    creature.next_spline_id = creature.next_spline_id.wrapping_add(1);
    creature.motion = CreatureMotionState::Waypoint(CreatureWaypointMotion {
        node_index: arrived_node,
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
        run,
    })
}

pub(in crate::world) fn db_creature_waypoint_buffered_path(
    navigation: &DbCreatureNavigationGuardrail,
    geometry: Option<&WorldGeometry>,
    creature: &DbCreatureRuntime,
    start: WorldPosition,
    first_node_index: usize,
) -> Option<(Vec<WorldPosition>, usize)> {
    let node_count = creature.spawn.waypoint_path.len();
    if node_count == 0 {
        return None;
    }
    let movement_type = creature.default_movement_type();
    let mut path = Vec::new();
    let mut current_start = start;
    let mut node_index = first_node_index.min(node_count.saturating_sub(1));
    let mut waypoint_forward = creature.waypoint_forward;
    let mut arrived_node = node_index;

    for _ in 0..node_count {
        let node = creature.spawn.waypoint_path.get(node_index)?;
        let raw_destination = WorldPosition::new(
            creature.spawn.map,
            node.position_x,
            node.position_y,
            node.position_z,
            node.orientation.unwrap_or(current_start.orientation),
        );
        let path_result = db_creature_path_to_destination(
            navigation,
            geometry,
            creature,
            current_start,
            raw_destination,
            CreaturePathMode::Full,
        )?;
        let mut leg = path_result.points;
        let leg_destination = *leg.last()?;
        path.append(&mut leg);
        arrived_node = node_index;

        let duration = db_creature_walk_path_motion_duration(start, &path, creature.walk_speed());
        if node.wait_time != 0
            || duration.as_millis() as u64 >= DB_CREATURE_WAYPOINT_MIN_PATH_MILLIS
            || node_count <= 1
        {
            break;
        }

        let next = db_creature_next_waypoint_state(
            node_count,
            movement_type,
            node_index,
            waypoint_forward,
        );
        node_index = next.0;
        waypoint_forward = next.1;
        current_start = leg_destination;
    }

    (!path.is_empty()).then_some((path, arrived_node))
}

#[cfg(test)]
pub(in crate::world) fn start_db_creature_chase_motion(
    session: &mut WorldSessionState,
    creature_guid: ObjectGuid,
    target: ObjectGuid,
    now: Instant,
) -> Option<StartedCreatureMotion> {
    let target_position = session.character.active_character.as_ref()?.position;
    let creature = session
        .visibility
        .db_creatures
        .get_mut(&creature_guid.raw())?;
    start_db_creature_chase_motion_runtime(
        &session.movement.db_creature_navigation,
        None,
        creature,
        target,
        target_position,
        None,
        now,
    )
}

pub(in crate::world) fn start_db_creature_chase_motion_runtime(
    navigation: &DbCreatureNavigationGuardrail,
    geometry: Option<&WorldGeometry>,
    creature: &mut DbCreatureRuntime,
    target: ObjectGuid,
    target_position: WorldPosition,
    chase_destination: Option<WorldPosition>,
    now: Instant,
) -> Option<StartedCreatureMotion> {
    if active_aura_blocks_movement(&creature.active_auras)
        || active_auras_suppress_hostile_refs(&creature.active_auras)
    {
        return None;
    }
    let start = creature.current_position;
    let stop_distance = db_creature_chase_stop_distance(creature);
    if creature.has_waypoint_movement() && creature.waypoint_resume_position.is_none() {
        creature.waypoint_resume_position = Some(start);
    }
    let path_result = if let Some(chase_destination) = chase_destination {
        db_creature_path_to_destination(
            navigation,
            geometry,
            creature,
            start,
            chase_destination,
            CreaturePathMode::Full,
        )?
    } else {
        db_creature_chase_path(
            navigation,
            geometry,
            creature,
            start,
            target_position,
            stop_distance,
        )?
    };
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
    let run = creature.chase_run;
    let duration = db_creature_targeted_motion_duration(creature, start, &path, run);
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
        run,
    });
    Some(StartedCreatureMotion {
        start,
        path,
        spline_id,
        duration,
        run,
    })
}

#[cfg(test)]
pub(in crate::world) fn start_db_creature_return_home_motion(
    session: &mut WorldSessionState,
    creature_guid: ObjectGuid,
    now: Instant,
) -> Option<StartedCreatureMotion> {
    let creature = session
        .visibility
        .db_creatures
        .get_mut(&creature_guid.raw())?;
    start_db_creature_return_home_motion_runtime(
        &session.movement.db_creature_navigation,
        None,
        creature,
        now,
    )
}

pub(in crate::world) fn start_db_creature_return_home_motion_runtime(
    navigation: &DbCreatureNavigationGuardrail,
    geometry: Option<&WorldGeometry>,
    creature: &mut DbCreatureRuntime,
    now: Instant,
) -> Option<StartedCreatureMotion> {
    if active_aura_blocks_movement(&creature.active_auras) {
        return None;
    }
    if matches!(creature.motion, CreatureMotionState::ReturnHome(_)) {
        return None;
    }
    let start = creature.current_position;
    let raw_destination = creature
        .waypoint_resume_position
        .filter(|_| creature.has_waypoint_movement())
        .unwrap_or(creature.home_position);
    if start.map_id != raw_destination.map_id
        || !world_position_is_finite(start)
        || !world_position_is_finite(raw_destination)
    {
        creature.current_position = raw_destination;
        creature.motion = CreatureMotionState::Idle;
        creature.waypoint_resume_position = None;
        if creature.has_waypoint_movement() {
            creature.next_random_move_at = None;
            creature.next_waypoint_move_at = Some(now);
        }
        return None;
    }
    let path_result = db_creature_path_to_destination(
        navigation,
        geometry,
        creature,
        start,
        raw_destination,
        CreaturePathMode::Full,
    )?;
    let path = path_result.points;
    let destination = *path.last()?;
    let move_distance = distance_2d(start.x, start.y, destination.x, destination.y);
    if move_distance <= f32::EPSILON {
        creature.current_position = destination;
        creature.motion = CreatureMotionState::Idle;
        creature.waypoint_resume_position = None;
        if creature.has_waypoint_movement() {
            creature.next_random_move_at = None;
            creature.next_waypoint_move_at = Some(now);
        }
        return None;
    }
    let duration = db_creature_path_motion_duration(start, &path, creature.run_speed());
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
        run: true,
    })
}

pub(in crate::world) fn start_db_creature_flee_motion_runtime(
    navigation: &DbCreatureNavigationGuardrail,
    geometry: Option<&WorldGeometry>,
    creature: &mut DbCreatureRuntime,
    source: ObjectGuid,
    source_position: WorldPosition,
    now: Instant,
    duration: Duration,
) -> Option<StartedCreatureMotion> {
    if active_aura_blocks_movement(&creature.active_auras) {
        return None;
    }
    if matches!(creature.motion, CreatureMotionState::Flee(_)) {
        return None;
    }
    let start = creature.current_position;
    if start.map_id != source_position.map_id
        || !world_position_is_finite(start)
        || !world_position_is_finite(source_position)
    {
        return None;
    }
    if creature.has_waypoint_movement() && creature.waypoint_resume_position.is_none() {
        creature.waypoint_resume_position = Some(start);
    }
    let dx = start.x - source_position.x;
    let dy = start.y - source_position.y;
    let angle = if dx.abs() <= f32::EPSILON && dy.abs() <= f32::EPSILON {
        let seed = (creature.guid().raw() % 6283) as f32 / 1000.0;
        normalize_orientation(seed)
    } else {
        normalize_orientation(dy.atan2(dx))
    };
    let distance = 30.0;
    let raw_destination = WorldPosition::new(
        start.map_id,
        start.x + angle.cos() * distance,
        start.y + angle.sin() * distance,
        start.z,
        angle,
    );
    let path_result = db_creature_path_to_destination(
        navigation,
        geometry,
        creature,
        start,
        raw_destination,
        CreaturePathMode::Full,
    )?;
    let path = path_result.points;
    let destination = *path.last()?;
    let move_distance = distance_2d(start.x, start.y, destination.x, destination.y);
    if move_distance <= f32::EPSILON {
        return None;
    }
    let motion_duration = db_creature_path_motion_duration(start, &path, creature.run_speed());
    let spline_id = creature.next_spline_id;
    creature.next_spline_id = creature.next_spline_id.wrapping_add(1);
    creature.motion = CreatureMotionState::Flee(CreatureFleeMotion {
        source,
        start,
        destination,
        path: path.clone(),
        started_at: now,
        duration: motion_duration,
        flee_until: now + duration,
    });
    Some(StartedCreatureMotion {
        start,
        path,
        spline_id,
        duration: motion_duration,
        run: true,
    })
}

pub(in crate::world) fn stop_db_creature_motion_runtime(
    creature: &mut DbCreatureRuntime,
) -> StoppedCreatureMotion {
    let stop = StoppedCreatureMotion {
        position: creature.current_position,
        spline_id: creature.next_spline_id,
    };
    creature.next_spline_id = creature.next_spline_id.wrapping_add(1);
    creature.motion = CreatureMotionState::Idle;
    stop
}

pub(in crate::world) fn retime_db_creature_motion_for_speed_change(
    creature: &mut DbCreatureRuntime,
    now: Instant,
) -> anyhow::Result<Option<OutboundWorldPacket>> {
    let guid = creature.guid();
    let Some(retimed) = retimed_db_creature_motion(creature, now) else {
        return Ok(None);
    };
    let body = match retimed.facing_target {
        Some(target) => build_monster_move_facing_target_path_body_with_run(
            guid,
            retimed.start,
            &retimed.path,
            retimed.spline_id,
            retimed.duration.as_millis().max(1) as u32,
            target,
            retimed.run,
        )?,
        None if retimed.run => build_monster_move_run_path_body(
            guid,
            retimed.start,
            &retimed.path,
            retimed.spline_id,
            retimed.duration.as_millis().max(1) as u32,
        )?,
        None => build_monster_move_walk_path_body(
            guid,
            retimed.start,
            &retimed.path,
            retimed.spline_id,
            retimed.duration.as_millis().max(1) as u32,
        )?,
    };
    Ok(Some(OutboundWorldPacket {
        opcode: WorldOpcode::SmsgMonsterMove as u16,
        body,
    }))
}

#[derive(Debug, Clone)]
pub(in crate::world) struct RetimedCreatureMotion {
    pub(in crate::world) start: WorldPosition,
    pub(in crate::world) path: Vec<WorldPosition>,
    pub(in crate::world) spline_id: u32,
    pub(in crate::world) duration: Duration,
    pub(in crate::world) run: bool,
    pub(in crate::world) facing_target: Option<ObjectGuid>,
}

pub(in crate::world) fn retimed_db_creature_motion(
    creature: &mut DbCreatureRuntime,
    now: Instant,
) -> Option<RetimedCreatureMotion> {
    let original = creature.motion.clone();
    let (start, path, started_at, duration, run, facing_target) = match &original {
        CreatureMotionState::Idle => return None,
        CreatureMotionState::Random(motion) => (
            motion.start,
            motion.path.clone(),
            motion.started_at,
            motion.duration,
            creature.default_movement_run,
            None,
        ),
        CreatureMotionState::Confused(motion) => (
            motion.start,
            motion.path.clone(),
            motion.started_at,
            motion.duration,
            false,
            None,
        ),
        CreatureMotionState::Waypoint(motion) => (
            motion.start,
            motion.path.clone(),
            motion.started_at,
            motion.duration,
            creature.default_movement_run,
            None,
        ),
        CreatureMotionState::Chase(motion) => (
            motion.start,
            motion.path.clone(),
            motion.started_at,
            motion.duration,
            motion.run,
            Some(motion.target),
        ),
        CreatureMotionState::Flee(motion) => (
            motion.start,
            motion.path.clone(),
            motion.started_at,
            motion.duration,
            true,
            None,
        ),
        CreatureMotionState::ReturnHome(motion) => (
            motion.start,
            motion.path.clone(),
            motion.started_at,
            motion.duration,
            true,
            None,
        ),
    };
    let (current_position, remaining_path) =
        remaining_timed_path_after_speed_change(start, &path, started_at, duration, now)?;
    if path_distance_2d(current_position, &remaining_path) <= f32::EPSILON {
        advance_db_creature_motion_runtime(creature, now);
        return None;
    }
    let new_duration = match original {
        CreatureMotionState::Random(_) | CreatureMotionState::Confused(_) => {
            db_creature_random_motion_duration(creature, current_position, &remaining_path, run)
        }
        CreatureMotionState::Chase(_) => {
            db_creature_targeted_motion_duration(creature, current_position, &remaining_path, run)
        }
        _ => db_creature_motion_duration_for_run(creature, current_position, &remaining_path, run),
    };
    let destination = *remaining_path.last()?;
    let spline_id = creature.next_spline_id;
    creature.next_spline_id = creature.next_spline_id.wrapping_add(1);
    creature.current_position = current_position;
    match creature.motion.clone() {
        CreatureMotionState::Random(_) => {
            creature.motion = CreatureMotionState::Random(CreatureRandomMotion {
                start: current_position,
                destination,
                path: remaining_path.clone(),
                started_at: now,
                duration: new_duration,
            });
        }
        CreatureMotionState::Confused(_) => {
            creature.motion = CreatureMotionState::Confused(CreatureRandomMotion {
                start: current_position,
                destination,
                path: remaining_path.clone(),
                started_at: now,
                duration: new_duration,
            });
        }
        CreatureMotionState::Waypoint(motion) => {
            creature.motion = CreatureMotionState::Waypoint(CreatureWaypointMotion {
                node_index: motion.node_index,
                start: current_position,
                destination,
                path: remaining_path.clone(),
                started_at: now,
                duration: new_duration,
            });
        }
        CreatureMotionState::Chase(motion) => {
            creature.motion = CreatureMotionState::Chase(CreatureChaseMotion {
                target: motion.target,
                start: current_position,
                destination,
                path: remaining_path.clone(),
                started_at: now,
                duration: new_duration,
                recheck_at: now + Duration::from_millis(DB_CREATURE_CHASE_RECHECK_MILLIS),
                run,
            });
        }
        CreatureMotionState::Flee(motion) => {
            creature.motion = CreatureMotionState::Flee(CreatureFleeMotion {
                source: motion.source,
                start: current_position,
                destination,
                path: remaining_path.clone(),
                started_at: now,
                duration: new_duration,
                flee_until: motion.flee_until,
            });
        }
        CreatureMotionState::ReturnHome(_) => {
            creature.motion = CreatureMotionState::ReturnHome(CreatureReturnHomeMotion {
                start: current_position,
                destination,
                path: remaining_path.clone(),
                started_at: now,
                duration: new_duration,
            });
        }
        CreatureMotionState::Idle => return None,
    }
    Some(RetimedCreatureMotion {
        start: current_position,
        path: remaining_path,
        spline_id,
        duration: new_duration,
        run,
        facing_target,
    })
}

pub(in crate::world) fn remaining_timed_path_after_speed_change(
    start: WorldPosition,
    path: &[WorldPosition],
    started_at: Instant,
    duration: Duration,
    now: Instant,
) -> Option<(WorldPosition, Vec<WorldPosition>)> {
    let elapsed = now.saturating_duration_since(started_at);
    if elapsed >= duration {
        return None;
    }
    let total = path_distance_2d(start, path);
    if total <= f32::EPSILON || duration.is_zero() {
        return None;
    }
    let travelled = total * (elapsed.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0);
    let current = position_along_path(start, path, travelled)?;
    let remaining = remaining_path_after_travel_distance(start, path, travelled)?;
    Some((current, remaining))
}

pub(in crate::world) fn remaining_path_after_travel_distance(
    start: WorldPosition,
    path: &[WorldPosition],
    mut travelled: f32,
) -> Option<Vec<WorldPosition>> {
    let mut previous = start;
    for (index, point) in path.iter().enumerate() {
        let segment = distance_2d(previous.x, previous.y, point.x, point.y);
        if segment <= f32::EPSILON {
            previous = *point;
            continue;
        }
        if travelled > segment {
            travelled -= segment;
            previous = *point;
            continue;
        }
        let mut remaining = if travelled >= segment - f32::EPSILON {
            path[index + 1..].to_vec()
        } else {
            path[index..].to_vec()
        };
        if remaining.first().is_some_and(|first| {
            distance_2d(previous.x, previous.y, first.x, first.y) <= f32::EPSILON
        }) {
            remaining.remove(0);
        }
        return (!remaining.is_empty()).then_some(remaining);
    }
    None
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) enum CreaturePathMode {
    Full,
    StopShort(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct DbCreaturePathFlags(pub(in crate::world) u8);

impl DbCreaturePathFlags {
    pub(in crate::world) const NORMAL: Self = Self(0x01);
    #[allow(dead_code)]
    pub(in crate::world) const SHORTCUT: Self = Self(0x02);
    pub(in crate::world) const INCOMPLETE: Self = Self(0x04);
    pub(in crate::world) const NOPATH: Self = Self(0x08);
    pub(in crate::world) const NOT_USING_PATH: Self = Self(0x10);
    #[allow(dead_code)]
    pub(in crate::world) const SHORT: Self = Self(0x20);

    pub(in crate::world) fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 != 0
    }
}

#[derive(Debug, Clone)]
pub(in crate::world) struct DbCreaturePath {
    pub(in crate::world) flags: DbCreaturePathFlags,
    pub(in crate::world) points: Vec<WorldPosition>,
}

#[derive(Debug)]
pub(in crate::world) enum DbCreaturePathBuild {
    Path(DbCreaturePath),
    NoPath(DbCreaturePathFlags),
    Unavailable,
}

pub(in crate::world) fn db_creature_chase_path(
    navigation: &DbCreatureNavigationGuardrail,
    geometry: Option<&WorldGeometry>,
    creature: &DbCreatureRuntime,
    start: WorldPosition,
    target_position: WorldPosition,
    stop_distance: f32,
) -> Option<DbCreaturePath> {
    if !db_creature_pathing_check(navigation, start, target_position).is_clear() {
        return None;
    }
    if DB_CREATURE_CHASE_STRAIGHT_FAST_PATH_ENABLED
        && db_creature_can_use_straight_chase_path(navigation, start, target_position)
    {
        if let Some(points) = db_creature_straight_path(
            start,
            target_position,
            CreaturePathMode::StopShort(stop_distance),
        ) {
            return Some(DbCreaturePath {
                flags: DbCreaturePathFlags::NORMAL,
                points,
            });
        }
    }

    let mut path = db_creature_path_to_destination(
        navigation,
        geometry,
        creature,
        start,
        target_position,
        CreaturePathMode::Full,
    )?;
    path.points = if path.flags.contains(DbCreaturePathFlags::NOT_USING_PATH) {
        db_creature_trim_path_to_travel_distance(start, path.points, stop_distance)?
    } else {
        db_creature_cut_chase_path(
            navigation,
            start,
            path.points,
            target_position,
            stop_distance,
        )?
    };
    Some(path)
}

pub(in crate::world) fn db_creature_chase_stop_distance(creature: &DbCreatureRuntime) -> f32 {
    combined_melee_reach(creature.combat_reach(), PLAYER_COMBAT_REACH_YARDS)
        * DB_CREATURE_CHASE_DEFAULT_RANGE_FACTOR
}

pub(in crate::world) fn db_creature_chase_angle_from_target(
    creature_position: WorldPosition,
    target_position: WorldPosition,
) -> f32 {
    let dx = creature_position.x - target_position.x;
    let dy = creature_position.y - target_position.y;
    if dx.abs() <= f32::EPSILON && dy.abs() <= f32::EPSILON {
        target_position.orientation
    } else {
        dy.atan2(dx)
    }
}

pub(in crate::world) fn db_creature_chase_near_point(
    target_position: WorldPosition,
    distance: f32,
    angle: f32,
) -> WorldPosition {
    WorldPosition::new(
        target_position.map_id,
        target_position.x + angle.cos() * distance,
        target_position.y + angle.sin() * distance,
        target_position.z,
        normalize_orientation(angle + std::f32::consts::PI),
    )
}

pub(in crate::world) fn db_creature_can_use_straight_chase_path(
    navigation: &DbCreatureNavigationGuardrail,
    start: WorldPosition,
    target_position: WorldPosition,
) -> bool {
    if !db_creature_has_native_los_data_for_positions(navigation, start, target_position) {
        return false;
    }
    if !db_creature_navigation_check(navigation, start, target_position).is_clear() {
        return false;
    }
    if (start.z - target_position.z).abs() >= 5.0 {
        return false;
    }
    distance_2d(start.x, start.y, target_position.x, target_position.y) <= 200.0
}

pub(in crate::world) fn db_creature_cut_chase_path(
    navigation: &DbCreatureNavigationGuardrail,
    start: WorldPosition,
    path: Vec<WorldPosition>,
    target_position: WorldPosition,
    stop_distance: f32,
) -> Option<Vec<WorldPosition>> {
    if path.is_empty() {
        return None;
    }
    let stop_distance_sq = stop_distance * stop_distance;
    let mut cut = Vec::new();
    for point in path.iter().copied() {
        cut.push(point);
        let dx = point.x - target_position.x;
        let dy = point.y - target_position.y;
        if dx * dx + dy * dy > stop_distance_sq {
            continue;
        }
        if !db_creature_has_line_of_sight(navigation, point, target_position) {
            continue;
        }
        return Some(cut);
    }
    db_creature_trim_path_to_travel_distance(start, path, stop_distance)
}

pub(in crate::world) fn db_creature_path_to_destination(
    navigation: &DbCreatureNavigationGuardrail,
    geometry: Option<&WorldGeometry>,
    creature: &DbCreatureRuntime,
    start: WorldPosition,
    target_position: WorldPosition,
    mode: CreaturePathMode,
) -> Option<DbCreaturePath> {
    let target_position = db_creature_ground_destination(geometry, target_position)?;
    match db_creature_mmap_path(navigation, creature, start, target_position, mode) {
        DbCreaturePathBuild::Path(path) => {
            debug_assert!(!path.flags.contains(DbCreaturePathFlags::NOPATH));
            Some(path)
        }
        DbCreaturePathBuild::NoPath(flags) => {
            debug_assert!(flags.contains(DbCreaturePathFlags::NOPATH));
            None
        }
        DbCreaturePathBuild::Unavailable => {
            if db_creature_uses_unit_fixture_pathing(navigation) {
                db_creature_straight_path(start, target_position, mode).map(|points| {
                    DbCreaturePath {
                        flags: DbCreaturePathFlags(
                            DbCreaturePathFlags::NORMAL.0 | DbCreaturePathFlags::NOT_USING_PATH.0,
                        ),
                        points,
                    }
                })
            } else {
                None
            }
        }
    }
}

pub(in crate::world) const DB_CREATURE_RANDOM_PATH_ATTEMPTS: u32 = 4;

pub(in crate::world) fn db_creature_random_path(
    navigation: &DbCreatureNavigationGuardrail,
    geometry: Option<&WorldGeometry>,
    creature: &DbCreatureRuntime,
    start: WorldPosition,
    radius: f32,
) -> Option<DbCreaturePath> {
    db_creature_random_path_from_center(
        navigation,
        geometry,
        creature,
        creature.home_position,
        start,
        radius,
    )
}

pub(in crate::world) fn db_creature_random_path_from_center(
    navigation: &DbCreatureNavigationGuardrail,
    geometry: Option<&WorldGeometry>,
    creature: &DbCreatureRuntime,
    center: WorldPosition,
    start: WorldPosition,
    radius: f32,
) -> Option<DbCreaturePath> {
    if db_creature_uses_unit_fixture_pathing(navigation) {
        let raw_destination = db_creature_random_destination(
            center,
            radius,
            creature.guid().raw(),
            creature.next_spline_id,
        )?;
        return db_creature_path_to_destination(
            navigation,
            geometry,
            creature,
            start,
            raw_destination,
            CreaturePathMode::Full,
        );
    }

    db_creature_mmap_random_path_from_center(navigation, creature, center, start, radius)
}

#[allow(dead_code)]
pub(in crate::world) fn db_creature_mmap_random_path(
    navigation: &DbCreatureNavigationGuardrail,
    creature: &DbCreatureRuntime,
    start: WorldPosition,
    radius: f32,
) -> Option<DbCreaturePath> {
    db_creature_mmap_random_path_from_center(
        navigation,
        creature,
        creature.home_position,
        start,
        radius,
    )
}

pub(in crate::world) fn db_creature_mmap_random_path_from_center(
    navigation: &DbCreatureNavigationGuardrail,
    creature: &DbCreatureRuntime,
    center: WorldPosition,
    start: WorldPosition,
    radius: f32,
) -> Option<DbCreaturePath> {
    let data_dir = navigation.world_data_files.data_dir_for_native.as_ref()?;
    if start.map_id != center.map_id || radius <= 0.0 || !radius.is_finite() {
        return None;
    }
    let start_tile = mmap_tile_for_position(start)?;
    if !navigation
        .world_data_files
        .has_mmap_support_for_map(start.map_id)
        || !navigation
            .world_data_files
            .has_mmap_tile(start.map_id, start_tile.0, start_tile.1)
    {
        return None;
    }

    for attempt in 0..DB_CREATURE_RANDOM_PATH_ATTEMPTS {
        let native_path = native_mmap_find_random_path(
            data_dir,
            NativeMmapRandomPathRequest {
                center,
                start,
                start_tile,
                radius,
                angle_seed: db_creature_pseudo_random_unit(
                    creature.guid().raw(),
                    creature.next_spline_id,
                    attempt * 2,
                ),
                range_seed: db_creature_pseudo_random_unit(
                    creature.guid().raw(),
                    creature.next_spline_id,
                    attempt * 2 + 1,
                ),
                filter: db_creature_mmap_path_filter(creature),
            },
        );
        let flags = match native_path.status {
            NativeMmapPathStatus::Normal => DbCreaturePathFlags::NORMAL,
            NativeMmapPathStatus::Incomplete => DbCreaturePathFlags::INCOMPLETE,
            NativeMmapPathStatus::NoPath => continue,
            NativeMmapPathStatus::Unavailable
            | NativeMmapPathStatus::InvalidInput
            | NativeMmapPathStatus::NativeError => {
                break;
            }
        };
        let Some(path) = native_mmap_points_to_world_path(start, &native_path.points) else {
            continue;
        };
        if path.is_empty() {
            continue;
        }
        return Some(DbCreaturePath {
            flags,
            points: path,
        });
    }

    None
}

pub(in crate::world) fn db_creature_uses_unit_fixture_pathing(
    navigation: &DbCreatureNavigationGuardrail,
) -> bool {
    navigation.world_data_files.data_dir_for_native.is_none()
        && !navigation.world_data_files.maps_available
        && !navigation.world_data_files.vmaps_available
        && navigation.world_data_files.mmap_tiles.is_empty()
        && navigation.world_data_files.vmap_tiles.is_empty()
}

pub(in crate::world) fn db_creature_ground_destination(
    geometry: Option<&WorldGeometry>,
    target_position: WorldPosition,
) -> Option<WorldPosition> {
    match geometry {
        Some(geometry) => geometry.ground_position(target_position),
        None => Some(target_position),
    }
}

pub(in crate::world) fn db_creature_straight_path(
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

pub(in crate::world) fn db_creature_navigation_check(
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

pub(in crate::world) fn db_creature_pathing_check(
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

pub(in crate::world) fn player_charge_navigation_check(
    navigation: &DbCreatureNavigationGuardrail,
    start: WorldPosition,
    destination: WorldPosition,
) -> DbCreatureNavigationResult {
    if start.map_id != destination.map_id {
        return DbCreatureNavigationResult::MapMismatch;
    }
    if !world_position_is_finite(start) || !world_position_is_finite(destination) {
        return DbCreatureNavigationResult::InvalidCoordinate;
    }
    if !navigation.line_of_sight_clear
        || !db_creature_has_line_of_sight(navigation, start, destination)
    {
        return DbCreatureNavigationResult::LineOfSightBlocked;
    }
    if !navigation.path_available || !player_charge_has_valid_path(navigation, start, destination) {
        return DbCreatureNavigationResult::PathUnavailable;
    }
    DbCreatureNavigationResult::Clear
}

pub(in crate::world) fn world_position_is_finite(position: WorldPosition) -> bool {
    position.x.is_finite()
        && position.y.is_finite()
        && position.z.is_finite()
        && position.orientation.is_finite()
}

pub(in crate::world) fn player_charge_has_valid_path(
    navigation: &DbCreatureNavigationGuardrail,
    start: WorldPosition,
    destination: WorldPosition,
) -> bool {
    if navigation.world_data_files.mmap_tiles.is_empty() {
        return navigation.world_data_files.data_dir_for_native.is_none()
            && !navigation.world_data_files.maps_available;
    }

    let Some(data_dir) = navigation.world_data_files.data_dir_for_native.as_ref() else {
        return false;
    };
    let map_id = start.map_id;
    let Some(start_tile) = mmap_tile_for_position(start) else {
        return false;
    };
    let Some(destination_tile) = mmap_tile_for_position(destination) else {
        return false;
    };
    if !navigation.world_data_files.has_mmap_support_for_map(map_id)
        || !navigation
            .world_data_files
            .has_mmap_tile(map_id, start_tile.0, start_tile.1)
        || !navigation.world_data_files.has_mmap_tile(
            map_id,
            destination_tile.0,
            destination_tile.1,
        )
    {
        return false;
    }

    let path = native_mmap_find_path(
        data_dir,
        start,
        destination,
        start_tile,
        destination_tile,
        NativeMmapPathFilter::ground(),
    );
    matches!(
        path.status,
        NativeMmapPathStatus::Normal | NativeMmapPathStatus::Incomplete
    ) && path.points.len() >= 2
}

pub(in crate::world) fn db_creature_has_line_of_sight(
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
        unit_line_of_sight_position(start),
        unit_line_of_sight_position(target_position),
        (start_tile_x, start_tile_y),
        (target_tile_x, target_tile_y),
        false,
    )
    .unwrap_or(true)
}

pub(in crate::world) fn unit_line_of_sight_position(position: WorldPosition) -> WorldPosition {
    WorldPosition::new(
        position.map_id,
        position.x,
        position.y,
        position.z + DEFAULT_COLLISION_HEIGHT,
        position.orientation,
    )
}

pub(in crate::world) fn db_creature_has_native_los_data_for_positions(
    navigation: &DbCreatureNavigationGuardrail,
    start: WorldPosition,
    target_position: WorldPosition,
) -> bool {
    if navigation.world_data_files.vmap_tiles.is_empty() {
        return false;
    }
    let map_id = start.map_id;
    let Some((start_tile_x, start_tile_y)) = mmap_tile_for_position(start) else {
        return false;
    };
    let Some((target_tile_x, target_tile_y)) = mmap_tile_for_position(target_position) else {
        return false;
    };
    navigation.world_data_files.has_vmap_support_for_map(map_id)
        && navigation
            .world_data_files
            .has_vmap_tile(map_id, start_tile_x, start_tile_y)
        && navigation
            .world_data_files
            .has_vmap_tile(map_id, target_tile_x, target_tile_y)
}

pub(in crate::world) fn db_creature_has_valid_path(
    navigation: &DbCreatureNavigationGuardrail,
    start: WorldPosition,
    target_position: WorldPosition,
) -> bool {
    if navigation.world_data_files.mmap_tiles.is_empty() {
        return navigation.world_data_files.data_dir_for_native.is_none()
            && !navigation.world_data_files.maps_available;
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

pub(in crate::world) fn mmap_tile_for_position(position: WorldPosition) -> Option<(u32, u32)> {
    const MAX_NUMBER_OF_GRIDS: i32 = 64;
    const CENTER_GRID_ID: f32 = 32.0;
    const SIZE_OF_GRIDS: f32 = 533.333_3;

    if !world_position_is_finite(position) {
        return None;
    }
    let tile_x = (CENTER_GRID_ID - position.x / SIZE_OF_GRIDS) as i32;
    let tile_y = (CENTER_GRID_ID - position.y / SIZE_OF_GRIDS) as i32;
    (0..MAX_NUMBER_OF_GRIDS).contains(&tile_x).then_some(())?;
    (0..MAX_NUMBER_OF_GRIDS).contains(&tile_y).then_some(())?;
    Some((tile_x as u32, tile_y as u32))
}

#[cfg(test)]
pub(in crate::world) fn db_creature_mmap_next_path_corner(
    navigation: &DbCreatureNavigationGuardrail,
    creature: &DbCreatureRuntime,
    start: WorldPosition,
    target_position: WorldPosition,
) -> Option<WorldPosition> {
    match db_creature_mmap_path(
        navigation,
        creature,
        start,
        target_position,
        CreaturePathMode::Full,
    ) {
        DbCreaturePathBuild::Path(path) => path.points.into_iter().next(),
        DbCreaturePathBuild::NoPath(_) | DbCreaturePathBuild::Unavailable => None,
    }
}

pub(in crate::world) fn db_creature_mmap_path(
    navigation: &DbCreatureNavigationGuardrail,
    creature: &DbCreatureRuntime,
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
    if !navigation
        .world_data_files
        .has_mmap_support_for_map(start.map_id)
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
        db_creature_mmap_path_filter(creature),
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

pub(in crate::world) const INHABIT_GROUND: u32 = 0x01;
pub(in crate::world) const INHABIT_WATER: u32 = 0x02;
pub(in crate::world) const INHABIT_AIR: u32 = 0x04;
pub(in crate::world) const INHABIT_ANYWHERE: u32 = INHABIT_GROUND | INHABIT_WATER | INHABIT_AIR;

pub(in crate::world) fn db_creature_mmap_path_filter(
    creature: &DbCreatureRuntime,
) -> NativeMmapPathFilter {
    let inhabit_type = db_creature_normalized_inhabit_type(creature.spawn.template.inhabit_type);
    let mut include_flags = 0;
    if inhabit_type & INHABIT_GROUND != 0 {
        include_flags |= NativeMmapPathFilter::NAV_GROUND;
    }
    if inhabit_type & INHABIT_WATER != 0 {
        include_flags |= NativeMmapPathFilter::NAV_WATER | NativeMmapPathFilter::NAV_MAGMA_SLIME;
    }
    if include_flags == 0 {
        include_flags = NativeMmapPathFilter::NAV_GROUND;
    }
    NativeMmapPathFilter {
        include_flags,
        exclude_flags: 0,
    }
}

pub(in crate::world) fn db_creature_normalized_inhabit_type(inhabit_type: u32) -> u32 {
    if inhabit_type == 0 || inhabit_type > INHABIT_ANYWHERE {
        INHABIT_ANYWHERE
    } else {
        inhabit_type
    }
}

pub(in crate::world) fn native_mmap_points_to_world_path(
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

pub(in crate::world) fn db_creature_trim_path_for_mode(
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

pub(in crate::world) fn db_creature_trim_path_to_travel_distance(
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

pub(in crate::world) fn db_creature_path_motion_duration(
    start: WorldPosition,
    path: &[WorldPosition],
    run_speed: f32,
) -> Duration {
    Duration::from_millis(
        ((path_distance_2d(start, path) / run_speed.max(f32::EPSILON)) * 1000.0)
            .ceil()
            .max(1.0) as u64,
    )
}

pub(in crate::world) fn db_creature_walk_path_motion_duration(
    start: WorldPosition,
    path: &[WorldPosition],
    walk_speed: f32,
) -> Duration {
    Duration::from_millis(
        ((path_distance_2d(start, path) / walk_speed.max(f32::EPSILON)) * 1000.0)
            .ceil()
            .max(1.0) as u64,
    )
}

pub(in crate::world) fn db_creature_motion_duration_for_run(
    creature: &DbCreatureRuntime,
    start: WorldPosition,
    path: &[WorldPosition],
    run: bool,
) -> Duration {
    if run {
        db_creature_path_motion_duration(start, path, creature.run_speed())
    } else {
        db_creature_walk_path_motion_duration(start, path, creature.walk_speed())
    }
}

pub(in crate::world) fn db_creature_random_motion_duration(
    creature: &DbCreatureRuntime,
    start: WorldPosition,
    path: &[WorldPosition],
    run: bool,
) -> Duration {
    db_creature_path_motion_duration(start, path, creature.random_motion_speed(run))
}

pub(in crate::world) fn db_creature_targeted_motion_duration(
    creature: &DbCreatureRuntime,
    start: WorldPosition,
    path: &[WorldPosition],
    run: bool,
) -> Duration {
    db_creature_path_motion_duration(start, path, creature.targeted_motion_speed(run))
}

pub(in crate::world) fn db_creature_random_destination(
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

pub(in crate::world) fn db_creature_random_pause_millis(guid: u64, spline_id: u32) -> u64 {
    let span = DB_CREATURE_RANDOM_DELAY_MAX_MILLIS - DB_CREATURE_RANDOM_DELAY_MIN_MILLIS;
    DB_CREATURE_RANDOM_DELAY_MIN_MILLIS
        + (db_creature_pseudo_random_unit(guid, spline_id, 2) * span as f32) as u64
}

pub(in crate::world) fn db_creature_confused_pause_millis(guid: u64, spline_id: u32) -> u64 {
    let span =
        CMANGOS_CONFUSED_MOVEMENT_DELAY_MAX_MILLIS - CMANGOS_CONFUSED_MOVEMENT_DELAY_MIN_MILLIS;
    CMANGOS_CONFUSED_MOVEMENT_DELAY_MIN_MILLIS
        + (db_creature_pseudo_random_unit(guid, spline_id, 3) * span as f32) as u64
}

pub(in crate::world) fn db_creature_pseudo_random_unit(
    guid: u64,
    spline_id: u32,
    salt: u32,
) -> f32 {
    let mut value = guid
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((spline_id as u64) << 32)
        .wrapping_add(salt as u64);
    value ^= value >> 33;
    value = value.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    value ^= value >> 33;
    ((value & 0xFFFF_FFFF) as f32) / (u32::MAX as f32)
}

pub(in crate::world) fn interpolate_position(
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

pub(in crate::world) fn advance_timed_path_motion(
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

pub(in crate::world) fn position_along_path(
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

pub(in crate::world) fn path_distance_2d(start: WorldPosition, path: &[WorldPosition]) -> f32 {
    let mut distance = 0.0;
    let mut previous = start;
    for point in path {
        distance += distance_2d(previous.x, previous.y, point.x, point.y);
        previous = *point;
    }
    distance
}

pub(in crate::world) fn distance_2d(left_x: f32, left_y: f32, right_x: f32, right_y: f32) -> f32 {
    let dx = left_x - right_x;
    let dy = left_y - right_y;
    (dx * dx + dy * dy).sqrt()
}
