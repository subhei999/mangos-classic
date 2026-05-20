use super::*;
use wow_proto::world::WorldOpcode;

mod creatures;
mod grids;
mod players;
mod spells;
mod ticks;

#[derive(Debug, Default)]
pub(in crate::world) struct MapRuntimeManager {
    pub(in crate::world) maps: Mutex<MapRuntimeHandles>,
    pub(in crate::world) movement_actors: Mutex<MovementActorHandles>,
    pub(in crate::world) movement_actor_settings: MovementActorSettings,
    pub(in crate::world) static_world_cache: Arc<StaticWorldSpawnCache>,
    pub(in crate::world) geometry: Arc<WorldGeometry>,
    pub(in crate::world) db_scripts: Arc<DbScriptRegistry>,
    pub(in crate::world) creature_display_scales: HashMap<u32, f32>,
    pub(in crate::world) spell_cast_times: HashMap<u32, SpellCastTimeEntry>,
    pub(in crate::world) spell_durations: HashMap<u32, SpellDurationEntry>,
    pub(in crate::world) spell_radii: HashMap<u32, SpellRadiusEntry>,
    pub(in crate::world) spell_ranges: HashMap<u32, SpellRangeEntry>,
    pub(in crate::world) skill_line_abilities_by_spell: HashMap<u32, Vec<SkillLineAbilityEntry>>,
    pub(in crate::world) skill_lines: HashMap<u32, SkillLineEntry>,
    pub(in crate::world) skill_race_class_infos_by_skill:
        HashMap<u32, Vec<SkillRaceClassInfoEntry>>,
    pub(in crate::world) faction_templates: FactionTemplateStore,
    pub(in crate::world) active_playerbot_count: AtomicUsize,
    pub(in crate::world) planner_driven_playerbot_count: AtomicUsize,
    pub(in crate::world) next_gm_creature_guid: AtomicU64,
    pub(in crate::world) creature_grid_load_ensure_calls: AtomicU64,
    pub(in crate::world) creature_grid_load_cache_hits: AtomicU64,
    pub(in crate::world) creature_grid_load_db_queries: AtomicU64,
    pub(in crate::world) creature_grid_load_rows: AtomicU64,
}

pub(in crate::world) type MapRuntimeHandles = HashMap<(u32, u32), Arc<Mutex<MapRuntime>>>;
pub(in crate::world) type MovementActorHandles = HashMap<(u32, u32), MovementActorHandle>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub(in crate::world) struct CreatureGridLoadStats {
    pub(in crate::world) ensure_calls: u64,
    pub(in crate::world) cache_hits: u64,
    pub(in crate::world) db_queries: u64,
    pub(in crate::world) rows_loaded: u64,
}

pub(in crate::world) fn apply_creature_display_scale_fallbacks(
    spawns: &mut [CreatureSpawnQuery],
    display_scales: &HashMap<u32, f32>,
) {
    for spawn in spawns {
        if spawn.template.scale > 0.0 {
            continue;
        }
        let Some(scale) = [
            spawn.template.display_id1,
            spawn.template.display_id2,
            spawn.template.display_id3,
            spawn.template.display_id4,
        ]
        .into_iter()
        .find_map(|display_id| {
            display_scales
                .get(&display_id)
                .copied()
                .filter(|scale| *scale > 0.0)
        }) else {
            continue;
        };
        spawn.template.scale = scale;
    }
}

impl MapRuntimeManager {
    pub(in crate::world) fn with_movement_actor_settings(
        mut self,
        settings: MovementActorSettings,
    ) -> Self {
        self.movement_actor_settings = settings;
        self
    }

    #[cfg(test)]
    pub(in crate::world) fn with_movement_actor_settings_for_test(
        mut self,
        settings: MovementActorSettings,
    ) -> Self {
        self.movement_actor_settings = settings;
        self
    }

    #[allow(dead_code)]
    pub(in crate::world) fn with_world_data_files(world_data_files: &WorldDataFiles) -> Self {
        Self::with_world_data_files_and_static_cache(
            world_data_files,
            Arc::new(StaticWorldSpawnCache::default()),
        )
    }

    pub(in crate::world) fn with_world_data_files_and_static_cache(
        world_data_files: &WorldDataFiles,
        static_world_cache: Arc<StaticWorldSpawnCache>,
    ) -> Self {
        Self::with_world_data_files_static_cache_and_next_gm_guid(
            world_data_files,
            static_world_cache,
            1,
            Arc::new(DbScriptRegistry::default()),
        )
    }

    pub(in crate::world) fn with_world_data_files_static_cache_and_next_gm_guid(
        world_data_files: &WorldDataFiles,
        static_world_cache: Arc<StaticWorldSpawnCache>,
        next_gm_creature_guid: u64,
        db_scripts: Arc<DbScriptRegistry>,
    ) -> Self {
        let world_data_files = Arc::new(world_data_files.clone());
        Self {
            static_world_cache,
            geometry: Arc::new(WorldGeometry::new(world_data_files.clone())),
            db_scripts,
            creature_display_scales: world_data_files.creature_display_scales.clone(),
            spell_cast_times: world_data_files.spell_cast_times.clone(),
            spell_durations: world_data_files.spell_durations.clone(),
            spell_radii: world_data_files.spell_radii.clone(),
            spell_ranges: world_data_files.spell_ranges.clone(),
            skill_line_abilities_by_spell: world_data_files.skill_line_abilities_by_spell.clone(),
            skill_lines: world_data_files.skill_lines.clone(),
            skill_race_class_infos_by_skill: world_data_files
                .skill_race_class_infos_by_skill
                .clone(),
            faction_templates: world_data_files.faction_templates.clone(),
            next_gm_creature_guid: AtomicU64::new(next_gm_creature_guid.max(1)),
            ..Self::default()
        }
    }

    #[cfg(test)]
    pub(in crate::world) fn with_static_world_cache(
        static_world_cache: StaticWorldSpawnCache,
    ) -> Self {
        Self {
            static_world_cache: Arc::new(static_world_cache),
            ..Self::default()
        }
    }

    pub(in crate::world) fn spell_duration(
        &self,
        duration_index: u32,
    ) -> Option<SpellDurationEntry> {
        self.spell_durations.get(&duration_index).copied()
    }

    pub(in crate::world) fn spell_cast_time(
        &self,
        casting_time_index: u32,
    ) -> Option<SpellCastTimeEntry> {
        self.spell_cast_times.get(&casting_time_index).copied()
    }

    pub(in crate::world) fn spell_range(&self, range_index: u32) -> Option<SpellRangeEntry> {
        self.spell_ranges.get(&range_index).copied()
    }

    pub(in crate::world) fn spell_radius(&self, radius_index: u32) -> Option<SpellRadiusEntry> {
        self.spell_radii.get(&radius_index).copied()
    }

    pub(in crate::world) fn has_async_playerbot_planner_work(&self) -> bool {
        self.planner_driven_playerbot_count.load(Ordering::Relaxed) > 0
    }

    pub(in crate::world) fn skill_line_ability_for_spell(
        &self,
        spell_id: u32,
    ) -> Option<SkillLineAbilityEntry> {
        self.skill_line_abilities_by_spell
            .get(&spell_id)
            .and_then(|abilities| abilities.first())
            .copied()
    }

    pub(in crate::world) fn skill_line(&self, skill_id: u32) -> Option<SkillLineEntry> {
        self.skill_lines.get(&skill_id).copied()
    }

    pub(in crate::world) fn skill_race_class_info(
        &self,
        skill_id: u32,
        race: u8,
        class: u8,
    ) -> Option<SkillRaceClassInfoEntry> {
        let race_mask = 1u32.checked_shl(u32::from(race.saturating_sub(1)))?;
        let class_mask = 1u32.checked_shl(u32::from(class.saturating_sub(1)))?;
        self.skill_race_class_infos_by_skill
            .get(&skill_id)?
            .iter()
            .copied()
            .find(|entry| {
                (entry.race_mask == 0 || (entry.race_mask & race_mask) != 0)
                    && (entry.class_mask == 0 || (entry.class_mask & class_mask) != 0)
            })
    }

    pub(in crate::world) async fn get_or_create_map(
        &self,
        map_id: u32,
        instance_id: u32,
    ) -> Arc<Mutex<MapRuntime>> {
        let map_key = (map_id, instance_id);
        let mut maps = self.maps.lock().await;
        maps.entry(map_key)
            .or_insert_with(|| {
                Arc::new(Mutex::new(MapRuntime::with_geometry(
                    map_key.0,
                    map_key.1,
                    self.geometry.clone(),
                    self.db_scripts.clone(),
                )))
            })
            .clone()
    }

    async fn movement_actor_for_map(
        &self,
        map_key: (u32, u32),
        map: Arc<Mutex<MapRuntime>>,
    ) -> Option<MovementActorHandle> {
        if !self.movement_actor_settings.enabled {
            return None;
        }
        let mut actors = self.movement_actors.lock().await;
        Some(
            actors
                .entry(map_key)
                .or_insert_with(|| {
                    MovementActorHandle::spawn_proxy(map, self.movement_actor_settings)
                })
                .clone(),
        )
    }
}

fn pending_spell_event_unit_target_generation(
    map: &MapRuntime,
    kind: &PendingSpellEventKind,
) -> Option<(ObjectGuid, u64)> {
    let target = match kind {
        PendingSpellEventKind::Spell { targets, .. } => targets.unit_target?,
        PendingSpellEventKind::RangedAutoAttack { target, .. } => *target,
    };
    target.is_creature().then_some(target).and_then(|target| {
        map.creatures
            .get(&target.raw())
            .map(|creature| (target, creature.life_generation))
    })
}

pub(in crate::world) fn grid_world_center(grid: GridCoord) -> (f32, f32) {
    let (min_x, max_x, min_y, max_y) = grid_world_bounds(grid);
    ((min_x + max_x) * 0.5, (min_y + max_y) * 0.5)
}

pub(in crate::world) fn player_corpse_grid_query_radius() -> f32 {
    GRID_SIZE_YARDS * std::f32::consts::SQRT_2 * 0.5
}
