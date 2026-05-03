const CMANGOS_DEFAULT_HEIGHT_SEARCH: f32 = 10.0;
const CMANGOS_HEIGHT_IN_RANGE_SEARCH: f32 = 4.0;

extern "C" {
    fn wow_map_height_static(
        data_dir: *const std::os::raw::c_char,
        map_id: u32,
        tile_x: u32,
        tile_y: u32,
        x: f32,
        y: f32,
        z: f32,
        max_search_dist: f32,
        out_height: *mut f32,
    ) -> i32;

    fn wow_map_height_in_range(
        data_dir: *const std::os::raw::c_char,
        map_id: u32,
        tile_x: u32,
        tile_y: u32,
        x: f32,
        y: f32,
        z: f32,
        max_search_dist: f32,
        out_height: *mut f32,
    ) -> i32;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeTerrainHeightStatus {
    Found,
    NotFound,
    InvalidInput,
    NativeError,
}

#[derive(Debug, Clone, Copy)]
struct NativeTerrainHeight {
    status: NativeTerrainHeightStatus,
    height: Option<f32>,
}

fn native_map_height_static(
    data_dir: &CStr,
    position: WorldPosition,
    tile: (u32, u32),
    max_search_dist: f32,
) -> NativeTerrainHeight {
    if !world_position_is_finite(position)
        || !native_mmap_tile_is_valid(tile)
        || !max_search_dist.is_finite()
    {
        return NativeTerrainHeight {
            status: NativeTerrainHeightStatus::InvalidInput,
            height: None,
        };
    }

    let mut height = 0.0f32;
    // SAFETY: path, tile ids, coordinates, and output pointer are validated.
    // The C++ bridge catches exceptions and returns a status code instead of
    // unwinding across the FFI boundary.
    let result = unsafe {
        wow_map_height_static(
            data_dir.as_ptr(),
            position.map_id,
            tile.0,
            tile.1,
            position.x,
            position.y,
            position.z,
            max_search_dist,
            &mut height,
        )
    };
    native_terrain_height_from_status(result, height)
}

fn native_map_height_in_range(
    data_dir: &CStr,
    position: WorldPosition,
    tile: (u32, u32),
    max_search_dist: f32,
) -> NativeTerrainHeight {
    if !world_position_is_finite(position)
        || !native_mmap_tile_is_valid(tile)
        || !max_search_dist.is_finite()
    {
        return NativeTerrainHeight {
            status: NativeTerrainHeightStatus::InvalidInput,
            height: None,
        };
    }

    let mut height = 0.0f32;
    // SAFETY: path, tile ids, coordinates, and output pointer are validated.
    // The C++ bridge catches exceptions and returns a status code instead of
    // unwinding across the FFI boundary.
    let result = unsafe {
        wow_map_height_in_range(
            data_dir.as_ptr(),
            position.map_id,
            tile.0,
            tile.1,
            position.x,
            position.y,
            position.z,
            max_search_dist,
            &mut height,
        )
    };
    native_terrain_height_from_status(result, height)
}

fn native_terrain_height_from_status(result: i32, height: f32) -> NativeTerrainHeight {
    match result {
        1 if height.is_finite() => NativeTerrainHeight {
            status: NativeTerrainHeightStatus::Found,
            height: Some(height),
        },
        0 => NativeTerrainHeight {
            status: NativeTerrainHeightStatus::NotFound,
            height: None,
        },
        -3..=-1 => NativeTerrainHeight {
            status: NativeTerrainHeightStatus::InvalidInput,
            height: None,
        },
        _ => NativeTerrainHeight {
            status: NativeTerrainHeightStatus::NativeError,
            height: None,
        },
    }
}
