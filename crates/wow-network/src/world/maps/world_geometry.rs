#[derive(Debug, Clone)]
struct WorldGeometry {
    world_data_files: Arc<WorldDataFiles>,
}

impl Default for WorldGeometry {
    fn default() -> Self {
        Self {
            world_data_files: Arc::new(WorldDataFiles::fallback()),
        }
    }
}

impl WorldGeometry {
    fn new(world_data_files: Arc<WorldDataFiles>) -> Self {
        Self { world_data_files }
    }

    fn height_static(&self, position: WorldPosition) -> Option<f32> {
        self.sample_height(position, CMANGOS_DEFAULT_HEIGHT_SEARCH, native_map_height_static)
    }

    fn height_in_range(&self, position: WorldPosition) -> Option<f32> {
        self.sample_height(
            position,
            CMANGOS_HEIGHT_IN_RANGE_SEARCH,
            native_map_height_in_range,
        )
    }

    fn ground_position(&self, position: WorldPosition) -> Option<WorldPosition> {
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

    fn sample_height(
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
