use std::ffi::CStr;

const MAX_NATIVE_MMAP_PATH_POINTS: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeMmapPathStatus {
    Normal,
    Incomplete,
    NoPath,
    Unavailable,
    InvalidInput,
    NativeError,
}

#[derive(Debug, Clone)]
struct NativeMmapPath {
    status: NativeMmapPathStatus,
    points: Vec<WorldPosition>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct NativeMmapPathPoint {
    x: f32,
    y: f32,
    z: f32,
}

extern "C" {
    fn wow_mmap_find_path(
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
        out_points: *mut NativeMmapPathPoint,
        max_points: i32,
    ) -> i32;
}

#[cfg(test)]
fn native_mmap_find_path_points(
    data_dir: &CStr,
    start: WorldPosition,
    target: WorldPosition,
    start_tile: (u32, u32),
    target_tile: (u32, u32),
) -> Option<Vec<WorldPosition>> {
    let path = native_mmap_find_path(data_dir, start, target, start_tile, target_tile);
    matches!(
        path.status,
        NativeMmapPathStatus::Normal | NativeMmapPathStatus::Incomplete
    )
    .then_some(path.points)
}

fn native_mmap_find_path(
    data_dir: &CStr,
    start: WorldPosition,
    target: WorldPosition,
    start_tile: (u32, u32),
    target_tile: (u32, u32),
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
            points.as_mut_ptr(),
            MAX_NATIVE_MMAP_PATH_POINTS as i32,
        )
    };

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
        path.push(WorldPosition::new(
            start.map_id,
            point.x,
            point.y,
            point.z,
            0.0,
        ));
    }
    NativeMmapPath {
        status: if count as usize == MAX_NATIVE_MMAP_PATH_POINTS {
            NativeMmapPathStatus::Incomplete
        } else {
            NativeMmapPathStatus::Normal
        },
        points: path,
    }
}

fn native_mmap_status_from_error(error: i32) -> NativeMmapPathStatus {
    match error {
        -20 | -21 | -22 | -23 | -3 | -4 | -5 | -6 => NativeMmapPathStatus::Unavailable,
        -1 | -7 | -8 => NativeMmapPathStatus::InvalidInput,
        _ => NativeMmapPathStatus::NativeError,
    }
}

fn native_mmap_world_position_is_finite(position: WorldPosition) -> bool {
    position.x.is_finite()
        && position.y.is_finite()
        && position.z.is_finite()
        && position.orientation.is_finite()
}

fn native_mmap_tile_is_valid(tile: (u32, u32)) -> bool {
    const MAX_NUMBER_OF_GRIDS: u32 = 64;
    tile.0 < MAX_NUMBER_OF_GRIDS && tile.1 < MAX_NUMBER_OF_GRIDS
}
