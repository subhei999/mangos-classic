use super::*;
use std::ffi::CStr;

#[derive(Debug, Clone)]
pub(in crate::world) struct WorldGeometry {
    pub(in crate::world) world_data_files: Arc<WorldDataFiles>,
}

pub(in crate::world) const CMANGOS_PLAYER_COLLISION_HEIGHT: f32 = 2.03128;
pub(in crate::world) const LIQUID_MAP_IN_WATER: u32 = 0x04;
pub(in crate::world) const LIQUID_MAP_UNDER_WATER: u32 = 0x08;
pub(in crate::world) const LIQUID_MAP_WATER_WALK: u32 = 0x02;
pub(in crate::world) const MAP_LIQUID_TYPE_MAGMA: u32 = 0x01;
pub(in crate::world) const MAP_LIQUID_TYPE_OCEAN: u32 = 0x02;
pub(in crate::world) const MAP_LIQUID_TYPE_SLIME: u32 = 0x04;
pub(in crate::world) const MAP_LIQUID_TYPE_WATER: u32 = 0x08;
pub(in crate::world) const MAP_LIQUID_TYPE_DEEP_WATER: u32 = 0x10;

pub(in crate::world) const ENVIRONMENT_FLAG_IN_WATER: u32 = 0x01;
pub(in crate::world) const ENVIRONMENT_FLAG_IN_MAGMA: u32 = 0x02;
pub(in crate::world) const ENVIRONMENT_FLAG_IN_SLIME: u32 = 0x04;
pub(in crate::world) const ENVIRONMENT_FLAG_HIGH_SEA: u32 = 0x08;
pub(in crate::world) const ENVIRONMENT_FLAG_UNDERWATER: u32 = 0x10;
pub(in crate::world) const ENVIRONMENT_FLAG_HIGH_LIQUID: u32 = 0x20;
pub(in crate::world) const ENVIRONMENT_FLAG_LIQUID: u32 = 0x40;

impl Default for WorldGeometry {
    fn default() -> Self {
        Self {
            world_data_files: Arc::new(WorldDataFiles::fallback()),
        }
    }
}

impl WorldGeometry {
    pub(in crate::world) fn new(world_data_files: Arc<WorldDataFiles>) -> Self {
        Self { world_data_files }
    }

    pub(in crate::world) fn height_static(&self, position: WorldPosition) -> Option<f32> {
        self.sample_height(
            position,
            CMANGOS_DEFAULT_HEIGHT_SEARCH,
            native_map_height_static,
        )
    }

    pub(in crate::world) fn height_in_range(&self, position: WorldPosition) -> Option<f32> {
        self.sample_height(
            position,
            CMANGOS_HEIGHT_IN_RANGE_SEARCH,
            native_map_height_in_range,
        )
    }

    pub(in crate::world) fn ground_position(
        &self,
        position: WorldPosition,
    ) -> Option<WorldPosition> {
        if !self.world_data_files.maps_available && !self.world_data_files.vmaps_available {
            return Some(position);
        }
        let z = self
            .height_in_range(position)
            .or_else(|| self.height_static(position))?;
        Some(WorldPosition::new(
            position.map_id,
            position.x,
            position.y,
            z,
            position.orientation,
        ))
    }

    pub(in crate::world) fn environment_flags(&self, position: WorldPosition) -> u32 {
        let Some(data_dir) = self.world_data_files.data_dir_for_native.as_ref() else {
            return 0;
        };
        let Some(tile) = mmap_tile_for_position(position) else {
            return 0;
        };
        let result =
            native_map_liquid_status(data_dir, position, tile, CMANGOS_PLAYER_COLLISION_HEIGHT);
        if result.status != NativeTerrainLiquidResultStatus::Found {
            return 0;
        }
        let Some(liquid) = result.liquid else {
            return 0;
        };
        let _liquid_entry = liquid.entry;

        let mut flags = ENVIRONMENT_FLAG_LIQUID;
        if liquid.status_flags & LIQUID_MAP_UNDER_WATER != 0 {
            flags |= ENVIRONMENT_FLAG_UNDERWATER;
        }
        if liquid.type_flags & (MAP_LIQUID_TYPE_WATER | MAP_LIQUID_TYPE_OCEAN) != 0
            && liquid.status_flags & (LIQUID_MAP_UNDER_WATER | LIQUID_MAP_IN_WATER) != 0
        {
            flags |= ENVIRONMENT_FLAG_IN_WATER;
        }
        if liquid.type_flags & MAP_LIQUID_TYPE_MAGMA != 0
            && liquid.status_flags
                & (LIQUID_MAP_UNDER_WATER | LIQUID_MAP_IN_WATER | LIQUID_MAP_WATER_WALK)
                != 0
        {
            flags |= ENVIRONMENT_FLAG_IN_MAGMA;
        }
        if liquid.type_flags & MAP_LIQUID_TYPE_SLIME != 0
            && liquid.status_flags
                & (LIQUID_MAP_UNDER_WATER | LIQUID_MAP_IN_WATER | LIQUID_MAP_WATER_WALK)
                != 0
        {
            flags |= ENVIRONMENT_FLAG_IN_SLIME;
        }
        if liquid.type_flags & MAP_LIQUID_TYPE_DEEP_WATER != 0 {
            flags |= ENVIRONMENT_FLAG_HIGH_SEA;
        }
        if liquid.status_flags & (LIQUID_MAP_UNDER_WATER | LIQUID_MAP_IN_WATER) != 0
            && liquid.level > liquid.depth_level + CMANGOS_PLAYER_COLLISION_HEIGHT * 0.75
        {
            flags |= ENVIRONMENT_FLAG_HIGH_LIQUID;
        }
        flags
    }

    pub(in crate::world) fn area_flag(&self, position: WorldPosition) -> Option<u16> {
        let data_dir = self.world_data_files.data_dir_for_native.as_ref()?;
        let tile = mmap_tile_for_position(position)?;
        let result = native_map_area_flag(data_dir, position, tile);
        match result.status {
            NativeTerrainAreaFlagStatus::Found => result.area_flag,
            NativeTerrainAreaFlagStatus::NotFound
            | NativeTerrainAreaFlagStatus::InvalidInput
            | NativeTerrainAreaFlagStatus::NativeError => None,
        }
    }

    pub(in crate::world) fn area_entry(
        &self,
        position: WorldPosition,
    ) -> Option<(u16, AreaTableEntry)> {
        if let Some(entry) = self.wmo_area_entry(position) {
            return Some((entry.explore_flag, entry));
        }
        let area_flag = self.area_flag(position)?;
        let entry = self
            .world_data_files
            .area_entry_by_flag_and_map(area_flag, position.map_id)?;
        Some((area_flag, entry))
    }

    pub(in crate::world) fn wmo_area_entry(
        &self,
        position: WorldPosition,
    ) -> Option<AreaTableEntry> {
        let data_dir = self.world_data_files.data_dir_for_native.as_ref()?;
        let tile = mmap_tile_for_position(position)?;
        let result = native_map_area_info(data_dir, position, tile);
        let info = match result.status {
            NativeTerrainAreaInfoStatus::Found => result.info?,
            NativeTerrainAreaInfoStatus::NotFound
            | NativeTerrainAreaInfoStatus::InvalidInput
            | NativeTerrainAreaInfoStatus::NativeError => return None,
        };
        self.world_data_files.area_entry_by_wmo_triple_and_map(
            info.root_id,
            info.adt_id,
            info.group_id,
            position.map_id,
        )
    }

    pub(in crate::world) fn sample_height(
        &self,
        position: WorldPosition,
        max_search_dist: f32,
        sampler: fn(&CStr, WorldPosition, (u32, u32), f32) -> NativeTerrainHeight,
    ) -> Option<f32> {
        let data_dir = self.world_data_files.data_dir_for_native.as_ref()?;
        let tile = mmap_tile_for_position(position)?;
        let result = sampler(data_dir, position, tile, max_search_dist);
        match result.status {
            NativeTerrainHeightStatus::Found => result.height,
            NativeTerrainHeightStatus::NotFound
            | NativeTerrainHeightStatus::InvalidInput
            | NativeTerrainHeightStatus::NativeError => None,
        }
    }
}
