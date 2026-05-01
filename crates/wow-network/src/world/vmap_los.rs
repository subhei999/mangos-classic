extern "C" {
    fn wow_vmap_line_of_sight(
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
        ignore_m2_model: i32,
    ) -> i32;
}

fn native_vmap_line_of_sight(
    data_dir: &CStr,
    start: WorldPosition,
    target: WorldPosition,
    start_tile: (u32, u32),
    target_tile: (u32, u32),
    ignore_m2_model: bool,
) -> Option<bool> {
    if start.map_id != target.map_id
        || !native_mmap_world_position_is_finite(start)
        || !native_mmap_world_position_is_finite(target)
        || !native_mmap_tile_is_valid(start_tile)
        || !native_mmap_tile_is_valid(target_tile)
    {
        return None;
    }

    // SAFETY: this is the only Rust call into the CMaNGOS vmap LOS bridge.
    // The path is a `CString`, positions and tile ids are prevalidated, and
    // the C++ bridge catches exceptions and returns a status code.
    let result = unsafe {
        wow_vmap_line_of_sight(
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
            i32::from(ignore_m2_model),
        )
    };

    match result {
        1 => Some(true),
        0 => Some(false),
        _ => None,
    }
}
