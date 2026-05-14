use super::*;
use std::ffi::CStr;

pub(in crate::world) const CMANGOS_DEFAULT_HEIGHT_SEARCH: f32 = 10.0;
pub(in crate::world) const CMANGOS_HEIGHT_IN_RANGE_SEARCH: f32 = 4.0;

extern "C" {
    pub(in crate::world) fn wow_map_height_static(
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

    pub(in crate::world) fn wow_map_height_in_range(
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

    pub(in crate::world) fn wow_map_liquid_status(
        data_dir: *const std::os::raw::c_char,
        map_id: u32,
        tile_x: u32,
        tile_y: u32,
        x: f32,
        y: f32,
        z: f32,
        collision_height: f32,
        out_status: *mut i32,
        out_type_flags: *mut u32,
        out_entry: *mut u32,
        out_level: *mut f32,
        out_depth_level: *mut f32,
    ) -> i32;

    pub(in crate::world) fn wow_map_area_flag(
        data_dir: *const std::os::raw::c_char,
        map_id: u32,
        tile_x: u32,
        tile_y: u32,
        x: f32,
        y: f32,
        out_area_flag: *mut u32,
    ) -> i32;

    pub(in crate::world) fn wow_map_area_info(
        data_dir: *const std::os::raw::c_char,
        map_id: u32,
        tile_x: u32,
        tile_y: u32,
        x: f32,
        y: f32,
        z: f32,
        out_flags: *mut u32,
        out_adt_id: *mut i32,
        out_root_id: *mut i32,
        out_group_id: *mut i32,
        out_ground_z: *mut f32,
    ) -> i32;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum NativeTerrainHeightStatus {
    Found,
    NotFound,
    InvalidInput,
    NativeError,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct NativeTerrainHeight {
    pub(in crate::world) status: NativeTerrainHeightStatus,
    pub(in crate::world) height: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum NativeTerrainLiquidResultStatus {
    Found,
    NotFound,
    InvalidInput,
    NativeError,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct NativeTerrainLiquid {
    pub(in crate::world) status: NativeTerrainLiquidResultStatus,
    pub(in crate::world) liquid: Option<NativeTerrainLiquidData>,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct NativeTerrainLiquidData {
    pub(in crate::world) status_flags: u32,
    pub(in crate::world) type_flags: u32,
    pub(in crate::world) entry: u32,
    pub(in crate::world) level: f32,
    pub(in crate::world) depth_level: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum NativeTerrainAreaFlagStatus {
    Found,
    NotFound,
    InvalidInput,
    NativeError,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct NativeTerrainAreaFlag {
    pub(in crate::world) status: NativeTerrainAreaFlagStatus,
    pub(in crate::world) area_flag: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum NativeTerrainAreaInfoStatus {
    Found,
    NotFound,
    InvalidInput,
    NativeError,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct NativeTerrainAreaInfo {
    pub(in crate::world) status: NativeTerrainAreaInfoStatus,
    pub(in crate::world) info: Option<NativeTerrainAreaInfoData>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::world) struct NativeTerrainAreaInfoData {
    pub(in crate::world) flags: u32,
    pub(in crate::world) adt_id: i32,
    pub(in crate::world) root_id: i32,
    pub(in crate::world) group_id: i32,
    pub(in crate::world) ground_z: f32,
}

pub(in crate::world) fn native_map_height_static(
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

pub(in crate::world) fn native_map_height_in_range(
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

pub(in crate::world) fn native_map_liquid_status(
    data_dir: &CStr,
    position: WorldPosition,
    tile: (u32, u32),
    collision_height: f32,
) -> NativeTerrainLiquid {
    if !world_position_is_finite(position)
        || !native_mmap_tile_is_valid(tile)
        || !collision_height.is_finite()
    {
        return NativeTerrainLiquid {
            status: NativeTerrainLiquidResultStatus::InvalidInput,
            liquid: None,
        };
    }

    let mut status_flags = 0i32;
    let mut type_flags = 0u32;
    let mut entry = 0u32;
    let mut level = 0.0f32;
    let mut depth_level = 0.0f32;
    // SAFETY: path, tile ids, coordinates, and output pointers are validated.
    // The C++ bridge catches exceptions and returns a status code instead of
    // unwinding across the FFI boundary.
    let result = unsafe {
        wow_map_liquid_status(
            data_dir.as_ptr(),
            position.map_id,
            tile.0,
            tile.1,
            position.x,
            position.y,
            position.z,
            collision_height,
            &mut status_flags,
            &mut type_flags,
            &mut entry,
            &mut level,
            &mut depth_level,
        )
    };
    native_terrain_liquid_from_status(result, status_flags, type_flags, entry, level, depth_level)
}

pub(in crate::world) fn native_map_area_flag(
    data_dir: &CStr,
    position: WorldPosition,
    tile: (u32, u32),
) -> NativeTerrainAreaFlag {
    if !world_position_is_finite(position) || !native_mmap_tile_is_valid(tile) {
        return NativeTerrainAreaFlag {
            status: NativeTerrainAreaFlagStatus::InvalidInput,
            area_flag: None,
        };
    }

    let mut area_flag = 0u32;
    // SAFETY: path, tile ids, coordinates, and output pointer are validated.
    // The C++ bridge catches exceptions and returns a status code instead of
    // unwinding across the FFI boundary.
    let result = unsafe {
        wow_map_area_flag(
            data_dir.as_ptr(),
            position.map_id,
            tile.0,
            tile.1,
            position.x,
            position.y,
            &mut area_flag,
        )
    };
    native_terrain_area_flag_from_status(result, area_flag)
}

pub(in crate::world) fn native_map_area_info(
    data_dir: &CStr,
    position: WorldPosition,
    tile: (u32, u32),
) -> NativeTerrainAreaInfo {
    if !world_position_is_finite(position) || !native_mmap_tile_is_valid(tile) {
        return NativeTerrainAreaInfo {
            status: NativeTerrainAreaInfoStatus::InvalidInput,
            info: None,
        };
    }

    let mut flags = 0u32;
    let mut adt_id = 0i32;
    let mut root_id = 0i32;
    let mut group_id = 0i32;
    let mut ground_z = 0.0f32;
    // SAFETY: path, tile ids, coordinates, and output pointers are validated.
    // The C++ bridge catches exceptions and returns a status code instead of
    // unwinding across the FFI boundary.
    let result = unsafe {
        wow_map_area_info(
            data_dir.as_ptr(),
            position.map_id,
            tile.0,
            tile.1,
            position.x,
            position.y,
            position.z,
            &mut flags,
            &mut adt_id,
            &mut root_id,
            &mut group_id,
            &mut ground_z,
        )
    };
    native_terrain_area_info_from_status(result, flags, adt_id, root_id, group_id, ground_z)
}

pub(in crate::world) fn native_terrain_height_from_status(
    result: i32,
    height: f32,
) -> NativeTerrainHeight {
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

pub(in crate::world) fn native_terrain_area_info_from_status(
    result: i32,
    flags: u32,
    adt_id: i32,
    root_id: i32,
    group_id: i32,
    ground_z: f32,
) -> NativeTerrainAreaInfo {
    match result {
        1 if ground_z.is_finite() => NativeTerrainAreaInfo {
            status: NativeTerrainAreaInfoStatus::Found,
            info: Some(NativeTerrainAreaInfoData {
                flags,
                adt_id,
                root_id,
                group_id,
                ground_z,
            }),
        },
        0 => NativeTerrainAreaInfo {
            status: NativeTerrainAreaInfoStatus::NotFound,
            info: None,
        },
        -3..=-1 => NativeTerrainAreaInfo {
            status: NativeTerrainAreaInfoStatus::InvalidInput,
            info: None,
        },
        _ => NativeTerrainAreaInfo {
            status: NativeTerrainAreaInfoStatus::NativeError,
            info: None,
        },
    }
}

pub(in crate::world) fn native_terrain_area_flag_from_status(
    result: i32,
    area_flag: u32,
) -> NativeTerrainAreaFlag {
    match result {
        1 if area_flag <= u32::from(u16::MAX) => NativeTerrainAreaFlag {
            status: NativeTerrainAreaFlagStatus::Found,
            area_flag: Some(area_flag as u16),
        },
        0 => NativeTerrainAreaFlag {
            status: NativeTerrainAreaFlagStatus::NotFound,
            area_flag: None,
        },
        -3..=-1 => NativeTerrainAreaFlag {
            status: NativeTerrainAreaFlagStatus::InvalidInput,
            area_flag: None,
        },
        _ => NativeTerrainAreaFlag {
            status: NativeTerrainAreaFlagStatus::NativeError,
            area_flag: None,
        },
    }
}

pub(in crate::world) fn native_terrain_liquid_from_status(
    result: i32,
    status_flags: i32,
    type_flags: u32,
    entry: u32,
    level: f32,
    depth_level: f32,
) -> NativeTerrainLiquid {
    match result {
        1 if status_flags > 0 && level.is_finite() && depth_level.is_finite() => {
            NativeTerrainLiquid {
                status: NativeTerrainLiquidResultStatus::Found,
                liquid: Some(NativeTerrainLiquidData {
                    status_flags: status_flags as u32,
                    type_flags,
                    entry,
                    level,
                    depth_level,
                }),
            }
        }
        1 | 0 => NativeTerrainLiquid {
            status: NativeTerrainLiquidResultStatus::NotFound,
            liquid: None,
        },
        -3..=-1 => NativeTerrainLiquid {
            status: NativeTerrainLiquidResultStatus::InvalidInput,
            liquid: None,
        },
        _ => NativeTerrainLiquid {
            status: NativeTerrainLiquidResultStatus::NativeError,
            liquid: None,
        },
    }
}
