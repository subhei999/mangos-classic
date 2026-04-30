use std::ffi::CStr;

const MAX_NATIVE_MMAP_PATH_POINTS: usize = 16;

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

fn native_mmap_find_path_points(
    data_dir: &CStr,
    start: WorldPosition,
    target: WorldPosition,
    start_tile: (u32, u32),
    target_tile: (u32, u32),
) -> Option<Vec<WorldPosition>> {
    if start.map_id != target.map_id
        || !native_mmap_world_position_is_finite(start)
        || !native_mmap_world_position_is_finite(target)
        || !native_mmap_tile_is_valid(start_tile)
        || !native_mmap_tile_is_valid(target_tile)
    {
        return None;
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

    if count < 0 || count as usize > points.len() {
        return None;
    }

    let mut path = Vec::with_capacity(count as usize);
    for point in points.iter().take(count as usize) {
        if !point.x.is_finite() || !point.y.is_finite() || !point.z.is_finite() {
            return None;
        }
        path.push(WorldPosition::new(
            start.map_id,
            point.x,
            point.y,
            point.z,
            0.0,
        ));
    }
    Some(path)
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
