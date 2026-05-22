use super::*;

use std::ffi::CStr;
use std::time::Duration;

pub(in crate::world) const MAX_NATIVE_MMAP_PATH_POINTS: usize = 74;
pub(in crate::world) const NATIVE_PATHFIND_NORMAL: i32 = 0x0001;
pub(in crate::world) const NATIVE_PATHFIND_INCOMPLETE: i32 = 0x0004;
pub(in crate::world) const NATIVE_PATHFIND_NOPATH: i32 = 0x0008;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum NativeMmapPathStatus {
    Normal,
    Incomplete,
    NoPath,
    Unavailable,
    InvalidInput,
    NativeError,
}

#[derive(Debug, Clone)]
pub(in crate::world) struct NativeMmapPath {
    pub(in crate::world) status: NativeMmapPathStatus,
    pub(in crate::world) points: Vec<WorldPosition>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct NativeMmapPathPoint {
    pub(in crate::world) x: f32,
    pub(in crate::world) y: f32,
    pub(in crate::world) z: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub(in crate::world) struct NativeMmapCallTimings {
    pub(in crate::world) lock_and_tile_load_nanos: u64,
    pub(in crate::world) query_alloc_init_nanos: u64,
    pub(in crate::world) find_nearest_poly_nanos: u64,
    pub(in crate::world) find_path_nanos: u64,
    pub(in crate::world) find_smooth_path_nanos: u64,
}

extern "C" {
    pub(in crate::world) fn wow_mmap_find_path(
        data_dir: *const std::os::raw::c_char,
        map_id: u32,
        start_tile_x: u32,
        start_tile_y: u32,
        target_tile_x: u32,
        target_tile_y: u32,
        start_x: f32,
        start_y: f32,
        start_z: f32,
        target_x: f32,
        target_y: f32,
        target_z: f32,
        include_flags: u16,
        exclude_flags: u16,
        out_points: *mut NativeMmapPathPoint,
        max_points: i32,
        out_path_status: *mut i32,
        out_timings: *mut NativeMmapCallTimings,
    ) -> i32;

    pub(in crate::world) fn wow_mmap_find_random_path(
        data_dir: *const std::os::raw::c_char,
        map_id: u32,
        start_tile_x: u32,
        start_tile_y: u32,
        center_x: f32,
        center_y: f32,
        center_z: f32,
        start_x: f32,
        start_y: f32,
        start_z: f32,
        radius: f32,
        angle_seed: f32,
        range_seed: f32,
        include_flags: u16,
        exclude_flags: u16,
        out_points: *mut NativeMmapPathPoint,
        max_points: i32,
        out_path_status: *mut i32,
        out_timings: *mut NativeMmapCallTimings,
    ) -> i32;
}

#[cfg(test)]
pub(in crate::world) fn native_mmap_find_path_points(
    data_dir: &CStr,
    start: WorldPosition,
    target: WorldPosition,
    start_tile: (u32, u32),
    target_tile: (u32, u32),
) -> Option<Vec<WorldPosition>> {
    let path = native_mmap_find_path(
        data_dir,
        start,
        target,
        start_tile,
        target_tile,
        NativeMmapPathFilter::ground(),
    );
    matches!(
        path.status,
        NativeMmapPathStatus::Normal | NativeMmapPathStatus::Incomplete
    )
    .then_some(path.points)
}

pub(in crate::world) fn native_mmap_find_path(
    data_dir: &CStr,
    start: WorldPosition,
    target: WorldPosition,
    start_tile: (u32, u32),
    target_tile: (u32, u32),
    filter: NativeMmapPathFilter,
) -> NativeMmapPath {
    if start.map_id != target.map_id
        || !native_mmap_world_position_is_finite(start)
        || !native_mmap_world_position_is_finite(target)
        || !native_mmap_tile_is_valid(start_tile)
        || !native_mmap_tile_is_valid(target_tile)
    {
        return NativeMmapPath {
            status: NativeMmapPathStatus::InvalidInput,
            points: Vec::new(),
        };
    }

    let mut points = [NativeMmapPathPoint {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    }; MAX_NATIVE_MMAP_PATH_POINTS];
    let mut timings = NativeMmapCallTimings::default();
    let mut path_status = NATIVE_PATHFIND_NOPATH;

    // SAFETY: this is the only Rust call into the Detour mmap bridge. The C
    // string comes from `CString`, positions and tile ids are range-checked
    // above, and the output buffer length matches the `max_points` argument.
    // The C++ side is expected to treat invalid mmap data as a path miss and
    // return an error/count instead of throwing across FFI.
    let count = unsafe {
        wow_mmap_find_path(
            data_dir.as_ptr(),
            start.map_id,
            start_tile.0,
            start_tile.1,
            target_tile.0,
            target_tile.1,
            start.x,
            start.y,
            start.z,
            target.x,
            target.y,
            target.z,
            filter.include_flags,
            filter.exclude_flags,
            points.as_mut_ptr(),
            MAX_NATIVE_MMAP_PATH_POINTS as i32,
            &mut path_status,
            &mut timings,
        )
    };

    crate::observability::record_native_mmap_query(
        crate::observability::NativeMmapQueryKind::Path,
        native_mmap_query_timings(timings),
    );

    native_mmap_path_from_count(start.map_id, count, path_status, &points)
}

pub(in crate::world) struct NativeMmapRandomPathRequest {
    pub(in crate::world) center: WorldPosition,
    pub(in crate::world) start: WorldPosition,
    pub(in crate::world) start_tile: (u32, u32),
    pub(in crate::world) radius: f32,
    pub(in crate::world) angle_seed: f32,
    pub(in crate::world) range_seed: f32,
    pub(in crate::world) filter: NativeMmapPathFilter,
}

pub(in crate::world) fn native_mmap_find_random_path(
    data_dir: &CStr,
    request: NativeMmapRandomPathRequest,
) -> NativeMmapPath {
    if request.start.map_id != request.center.map_id
        || !native_mmap_world_position_is_finite(request.start)
        || !native_mmap_world_position_is_finite(request.center)
        || !native_mmap_tile_is_valid(request.start_tile)
        || !request.radius.is_finite()
        || request.radius <= 0.0
        || !request.angle_seed.is_finite()
        || !request.range_seed.is_finite()
    {
        return NativeMmapPath {
            status: NativeMmapPathStatus::InvalidInput,
            points: Vec::new(),
        };
    }

    let mut points = [NativeMmapPathPoint {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    }; MAX_NATIVE_MMAP_PATH_POINTS];
    let mut timings = NativeMmapCallTimings::default();
    let mut path_status = NATIVE_PATHFIND_NOPATH;

    // SAFETY: the C string comes from `CString`, the start position and tile are
    // validated above, and the output buffer length matches `max_points`.
    let count = unsafe {
        wow_mmap_find_random_path(
            data_dir.as_ptr(),
            request.start.map_id,
            request.start_tile.0,
            request.start_tile.1,
            request.center.x,
            request.center.y,
            request.center.z,
            request.start.x,
            request.start.y,
            request.start.z,
            request.radius,
            request.angle_seed,
            request.range_seed,
            request.filter.include_flags,
            request.filter.exclude_flags,
            points.as_mut_ptr(),
            MAX_NATIVE_MMAP_PATH_POINTS as i32,
            &mut path_status,
            &mut timings,
        )
    };

    crate::observability::record_native_mmap_query(
        crate::observability::NativeMmapQueryKind::RandomPath,
        native_mmap_query_timings(timings),
    );

    native_mmap_path_from_count(request.start.map_id, count, path_status, &points)
}

fn native_mmap_query_timings(
    timings: NativeMmapCallTimings,
) -> crate::observability::NativeMmapQueryTimings {
    crate::observability::NativeMmapQueryTimings {
        lock_and_tile_load: Duration::from_nanos(timings.lock_and_tile_load_nanos),
        query_alloc_init: Duration::from_nanos(timings.query_alloc_init_nanos),
        find_nearest_poly: Duration::from_nanos(timings.find_nearest_poly_nanos),
        find_path: Duration::from_nanos(timings.find_path_nanos),
        find_smooth_path: Duration::from_nanos(timings.find_smooth_path_nanos),
    }
}

pub(in crate::world) fn native_mmap_path_from_count(
    map_id: u32,
    count: i32,
    path_status: i32,
    points: &[NativeMmapPathPoint],
) -> NativeMmapPath {
    if count < 0 {
        return NativeMmapPath {
            status: native_mmap_status_from_error(count),
            points: Vec::new(),
        };
    }
    if count as usize > points.len() {
        return NativeMmapPath {
            status: NativeMmapPathStatus::NativeError,
            points: Vec::new(),
        };
    }
    if count == 0 {
        return NativeMmapPath {
            status: NativeMmapPathStatus::NoPath,
            points: Vec::new(),
        };
    }

    let mut path = Vec::with_capacity(count as usize);
    for point in points.iter().take(count as usize) {
        if !point.x.is_finite() || !point.y.is_finite() || !point.z.is_finite() {
            return NativeMmapPath {
                status: NativeMmapPathStatus::NativeError,
                points: Vec::new(),
            };
        }
        path.push(WorldPosition::new(map_id, point.x, point.y, point.z, 0.0));
    }
    NativeMmapPath {
        status: if path_status & NATIVE_PATHFIND_NOPATH != 0 {
            NativeMmapPathStatus::NoPath
        } else if path_status & NATIVE_PATHFIND_INCOMPLETE != 0 {
            NativeMmapPathStatus::Incomplete
        } else if path_status & NATIVE_PATHFIND_NORMAL != 0 {
            NativeMmapPathStatus::Normal
        } else if count as usize == MAX_NATIVE_MMAP_PATH_POINTS {
            NativeMmapPathStatus::Incomplete
        } else {
            NativeMmapPathStatus::Normal
        },
        points: path,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct NativeMmapPathFilter {
    pub(in crate::world) include_flags: u16,
    pub(in crate::world) exclude_flags: u16,
}

impl NativeMmapPathFilter {
    pub(in crate::world) const NAV_GROUND: u16 = 0x01;
    pub(in crate::world) const NAV_WATER: u16 = 0x04;
    pub(in crate::world) const NAV_MAGMA_SLIME: u16 = 0x08;

    pub(in crate::world) fn ground() -> Self {
        Self {
            include_flags: Self::NAV_GROUND,
            exclude_flags: 0,
        }
    }
}

pub(in crate::world) fn native_mmap_status_from_error(error: i32) -> NativeMmapPathStatus {
    match error {
        -20 | -21 | -22 | -23 | -3 | -4 | -5 | -6 => NativeMmapPathStatus::Unavailable,
        -1 | -7 | -8 | -9 => NativeMmapPathStatus::InvalidInput,
        _ => NativeMmapPathStatus::NativeError,
    }
}

pub(in crate::world) fn native_mmap_world_position_is_finite(position: WorldPosition) -> bool {
    position.x.is_finite()
        && position.y.is_finite()
        && position.z.is_finite()
        && position.orientation.is_finite()
}

pub(in crate::world) fn native_mmap_tile_is_valid(tile: (u32, u32)) -> bool {
    const MAX_NUMBER_OF_GRIDS: u32 = 64;
    tile.0 < MAX_NUMBER_OF_GRIDS && tile.1 < MAX_NUMBER_OF_GRIDS
}
