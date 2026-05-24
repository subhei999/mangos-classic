use super::*;

#[derive(Debug, Clone)]
pub(in crate::world) struct WorldDataFiles {
    pub(in crate::world) data_dir: std::path::PathBuf,
    pub(in crate::world) data_dir_for_native: Option<std::ffi::CString>,
    pub(in crate::world) maps_available: bool,
    pub(in crate::world) vmaps_available: bool,
    pub(in crate::world) auction_houses: HashMap<u32, AuctionHouseEntry>,
    pub(in crate::world) taxi_nodes: HashMap<u32, TaxiNodeEntry>,
    pub(in crate::world) taxi_paths: HashMap<(u32, u32), TaxiPathEntry>,
    pub(in crate::world) taxi_path_nodes: HashMap<u32, Vec<TaxiPathNodeEntry>>,
    pub(in crate::world) taxi_node_mask: [u32; 8],
    pub(in crate::world) creature_display_scales: HashMap<u32, f32>,
    pub(in crate::world) spell_cast_times: HashMap<u32, SpellCastTimeEntry>,
    pub(in crate::world) spell_durations: HashMap<u32, SpellDurationEntry>,
    pub(in crate::world) spell_radii: HashMap<u32, SpellRadiusEntry>,
    pub(in crate::world) spell_cones: HashMap<u32, SpellConeEntry>,
    pub(in crate::world) spell_ranges: HashMap<u32, SpellRangeEntry>,
    pub(in crate::world) skill_line_abilities_by_spell: HashMap<u32, Vec<SkillLineAbilityEntry>>,
    pub(in crate::world) skill_lines: HashMap<u32, SkillLineEntry>,
    pub(in crate::world) skill_race_class_infos_by_skill:
        HashMap<u32, Vec<SkillRaceClassInfoEntry>>,
    pub(in crate::world) faction_templates: FactionTemplateStore,
    pub(in crate::world) item_random_properties: HashMap<u32, ItemRandomPropertyEntry>,
    pub(in crate::world) spell_item_enchantments: HashMap<u32, SpellItemEnchantmentEntry>,
    pub(in crate::world) bank_bag_slot_prices: HashMap<u32, u32>,
    pub(in crate::world) area_triggers: HashMap<u32, AreaTriggerEntry>,
    pub(in crate::world) area_tables: AreaTableStore,
    pub(in crate::world) wmo_area_tables: WmoAreaTableStore,
    pub(in crate::world) mmap_headers: HashSet<u32>,
    pub(in crate::world) mmap_tiles: HashSet<(u32, u32, u32)>,
    pub(in crate::world) vmap_trees: HashSet<u32>,
    pub(in crate::world) vmap_tiles: HashSet<(u32, u32, u32)>,
}

impl WorldDataFiles {
    pub(in crate::world) fn fallback() -> Self {
        Self {
            data_dir: std::path::PathBuf::new(),
            data_dir_for_native: None,
            maps_available: false,
            vmaps_available: false,
            auction_houses: HashMap::new(),
            taxi_nodes: HashMap::new(),
            taxi_paths: HashMap::new(),
            taxi_path_nodes: HashMap::new(),
            taxi_node_mask: [0; 8],
            creature_display_scales: HashMap::new(),
            spell_cast_times: HashMap::new(),
            spell_durations: HashMap::new(),
            spell_radii: HashMap::new(),
            spell_cones: HashMap::new(),
            spell_ranges: HashMap::new(),
            skill_line_abilities_by_spell: HashMap::new(),
            skill_lines: HashMap::new(),
            skill_race_class_infos_by_skill: HashMap::new(),
            faction_templates: FactionTemplateStore::fallback_bridge(),
            item_random_properties: HashMap::new(),
            spell_item_enchantments: HashMap::new(),
            bank_bag_slot_prices: HashMap::new(),
            area_triggers: HashMap::new(),
            area_tables: AreaTableStore::default(),
            wmo_area_tables: WmoAreaTableStore::default(),
            mmap_headers: HashSet::new(),
            mmap_tiles: HashSet::new(),
            vmap_trees: HashSet::new(),
            vmap_tiles: HashSet::new(),
        }
    }

    pub(in crate::world) fn inspect(data_dir: impl Into<std::path::PathBuf>) -> Self {
        let data_dir = data_dir.into();
        let maps_available = data_dir.join("maps").is_dir();
        let vmaps_available = data_dir.join("vmaps").is_dir();
        let auction_houses = load_auction_houses(&data_dir.join("dbc").join("AuctionHouse.dbc"));
        let taxi_nodes = load_taxi_nodes(&data_dir.join("dbc").join("TaxiNodes.dbc"));
        let taxi_paths = load_taxi_paths(&data_dir.join("dbc").join("TaxiPath.dbc"));
        let taxi_path_nodes = load_taxi_path_nodes(&data_dir.join("dbc").join("TaxiPathNode.dbc"));
        let taxi_node_mask = taxi_network_mask(&taxi_nodes, &taxi_paths);
        let creature_display_scales = load_creature_display_info_scales(
            &data_dir.join("dbc").join("CreatureDisplayInfo.dbc"),
        );
        let spell_cast_times =
            load_spell_cast_times(&data_dir.join("dbc").join("SpellCastTimes.dbc"));
        let spell_durations = load_spell_durations(&data_dir.join("dbc").join("SpellDuration.dbc"));
        let spell_radii = load_spell_radii(&data_dir.join("dbc").join("SpellRadius.dbc"));
        let spell_cones = load_spell_cones(&data_dir);
        let spell_ranges = load_spell_ranges(&data_dir.join("dbc").join("SpellRange.dbc"));
        let skill_line_abilities_by_spell =
            load_skill_line_abilities(&data_dir.join("dbc").join("SkillLineAbility.dbc"));
        let skill_lines = load_skill_lines(&data_dir.join("dbc").join("SkillLine.dbc"));
        let skill_race_class_infos_by_skill =
            load_skill_race_class_infos(&data_dir.join("dbc").join("SkillRaceClassInfo.dbc"));
        let faction_templates =
            load_faction_templates(&data_dir.join("dbc").join("FactionTemplate.dbc"));
        let item_random_properties =
            load_item_random_properties(&data_dir.join("dbc").join("ItemRandomProperties.dbc"));
        let spell_item_enchantments =
            load_spell_item_enchantments(&data_dir.join("dbc").join("SpellItemEnchantment.dbc"));
        let bank_bag_slot_prices =
            load_bank_bag_slot_prices(&data_dir.join("dbc").join("BankBagSlotPrices.dbc"));
        let area_triggers = load_area_triggers(&data_dir.join("dbc").join("AreaTrigger.dbc"));
        let area_tables = load_area_tables(&data_dir.join("dbc").join("AreaTable.dbc"));
        let wmo_area_tables = load_wmo_area_tables(&data_dir.join("dbc").join("WMOAreaTable.dbc"));
        let mut mmap_headers = HashSet::new();
        let mut mmap_tiles = HashSet::new();
        let mut vmap_trees = HashSet::new();
        let mut vmap_tiles = HashSet::new();
        let mmaps_dir = data_dir.join("mmaps");
        let vmaps_dir = data_dir.join("vmaps");

        if let Ok(entries) = std::fs::read_dir(&mmaps_dir) {
            for entry in entries.flatten() {
                let Some(file_name) = entry.file_name().to_str().map(str::to_owned) else {
                    continue;
                };
                if let Some(map_id) = parse_mmap_header_file_name(&file_name) {
                    mmap_headers.insert(map_id);
                    continue;
                }
                if let Some(tile) = parse_mmap_tile_file_name(&file_name) {
                    mmap_tiles.insert(tile);
                }
            }
        }

        if let Ok(entries) = std::fs::read_dir(&vmaps_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !file_has_magic(&path, b"VMAP_7.0") {
                    continue;
                }
                let Some(file_name) = entry.file_name().to_str().map(str::to_owned) else {
                    continue;
                };
                if let Some(map_id) = parse_vmap_tree_file_name(&file_name) {
                    vmap_trees.insert(map_id);
                    continue;
                }
                if let Some(tile) = parse_vmap_tile_file_name(&file_name) {
                    vmap_tiles.insert(tile);
                }
            }
        }

        Self {
            data_dir_for_native: data_dir
                .to_str()
                .and_then(|path| std::ffi::CString::new(path).ok()),
            data_dir,
            maps_available,
            vmaps_available,
            auction_houses,
            taxi_nodes,
            taxi_paths,
            taxi_path_nodes,
            taxi_node_mask,
            creature_display_scales,
            spell_cast_times,
            spell_durations,
            spell_radii,
            spell_cones,
            spell_ranges,
            skill_line_abilities_by_spell,
            skill_lines,
            skill_race_class_infos_by_skill,
            faction_templates,
            item_random_properties,
            spell_item_enchantments,
            bank_bag_slot_prices,
            area_triggers,
            area_tables,
            wmo_area_tables,
            mmap_headers,
            mmap_tiles,
            vmap_trees,
            vmap_tiles,
        }
    }

    pub(in crate::world) fn has_mmap_support_for_map(&self, map_id: u32) -> bool {
        self.mmap_headers.contains(&map_id)
    }

    pub(in crate::world) fn has_mmap_tile(&self, map_id: u32, tile_x: u32, tile_y: u32) -> bool {
        self.mmap_tiles.contains(&(map_id, tile_x, tile_y))
    }

    pub(in crate::world) fn has_vmap_support_for_map(&self, map_id: u32) -> bool {
        self.vmap_trees.contains(&map_id)
    }

    pub(in crate::world) fn has_vmap_tile(&self, map_id: u32, tile_x: u32, tile_y: u32) -> bool {
        self.vmap_tiles.contains(&(map_id, tile_x, tile_y))
    }

    pub(in crate::world) fn area_entry_by_flag_and_map(
        &self,
        area_flag: u16,
        map_id: u32,
    ) -> Option<AreaTableEntry> {
        self.area_tables.entry_by_flag_and_map(area_flag, map_id)
    }

    pub(in crate::world) fn area_trigger_contains_position(
        &self,
        trigger_id: u32,
        position: WorldPosition,
        delta: f32,
    ) -> Option<bool> {
        self.area_triggers
            .get(&trigger_id)
            .map(|trigger| trigger.contains_position(position, delta))
    }

    pub(in crate::world) fn area_entry_by_wmo_triple_and_map(
        &self,
        root_id: i32,
        adt_id: i32,
        group_id: i32,
        map_id: u32,
    ) -> Option<AreaTableEntry> {
        self.wmo_area_tables
            .entries_by_triple(root_id, adt_id, group_id)
            .iter()
            .filter_map(|entry| self.area_tables.entry(entry.area_id))
            .rfind(|area| area.map_id == map_id)
    }

    pub(in crate::world) fn nearest_taxi_node(
        &self,
        position: WorldPosition,
        alliance: bool,
    ) -> Option<u32> {
        self.taxi_nodes
            .values()
            .filter(|node| {
                node.map_id == position.map_id
                    && self.taxi_node_known(self.taxi_node_mask, node.id)
                    && node.mount_creature_id(alliance) != 0
            })
            .min_by(|left, right| {
                left.distance_squared(position)
                    .partial_cmp(&right.distance_squared(position))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|node| node.id)
    }

    pub(in crate::world) fn taxi_path(
        &self,
        source: u32,
        destination: u32,
    ) -> Option<TaxiPathEntry> {
        self.taxi_paths.get(&(source, destination)).copied()
    }

    pub(in crate::world) fn taxi_node(&self, node: u32) -> Option<TaxiNodeEntry> {
        self.taxi_nodes.get(&node).copied()
    }

    pub(in crate::world) fn taxi_path_nodes(&self, path: u32) -> &[TaxiPathNodeEntry] {
        self.taxi_path_nodes
            .get(&path)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub(in crate::world) fn taxi_node_known(&self, taximask: [u32; 8], node: u32) -> bool {
        taxi_mask_has_node(taximask, node)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct SpellCastTimeEntry {
    pub(in crate::world) cast_time_millis: i32,
    pub(in crate::world) cast_time_per_level_millis: i32,
    pub(in crate::world) min_cast_time_millis: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct AuctionHouseEntry {
    pub(in crate::world) house_id: u32,
    pub(in crate::world) faction: u32,
    pub(in crate::world) deposit_percent: u32,
    pub(in crate::world) cut_percent: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::world) struct TaxiNodeEntry {
    pub(in crate::world) id: u32,
    pub(in crate::world) map_id: u32,
    pub(in crate::world) x: f32,
    pub(in crate::world) y: f32,
    pub(in crate::world) z: f32,
    pub(in crate::world) horde_mount_creature: u32,
    pub(in crate::world) alliance_mount_creature: u32,
}

impl TaxiNodeEntry {
    pub(in crate::world) fn position(self) -> WorldPosition {
        WorldPosition::new(self.map_id, self.x, self.y, self.z, 0.0)
    }

    pub(in crate::world) fn mount_creature_id(self, alliance: bool) -> u32 {
        if alliance {
            self.alliance_mount_creature
        } else {
            self.horde_mount_creature
        }
    }

    fn distance_squared(self, position: WorldPosition) -> f32 {
        let dx = self.x - position.x;
        let dy = self.y - position.y;
        let dz = self.z - position.z;
        dx * dx + dy * dy + dz * dz
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct TaxiPathEntry {
    pub(in crate::world) id: u32,
    pub(in crate::world) source: u32,
    pub(in crate::world) destination: u32,
    pub(in crate::world) price: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct SpellDurationEntry {
    pub(in crate::world) duration_millis: i32,
    pub(in crate::world) duration_per_level_millis: i32,
    pub(in crate::world) max_duration_millis: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::world) struct SpellRadiusEntry {
    pub(in crate::world) radius: f32,
    pub(in crate::world) radius_per_level: f32,
    pub(in crate::world) max_radius: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::world) struct SpellRangeEntry {
    pub(in crate::world) min_range: f32,
    pub(in crate::world) max_range: f32,
    pub(in crate::world) flags: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct AreaTableEntry {
    pub(in crate::world) id: u32,
    pub(in crate::world) map_id: u32,
    pub(in crate::world) zone_id: u32,
    pub(in crate::world) explore_flag: u16,
    pub(in crate::world) flags: u32,
    pub(in crate::world) area_level: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::world) struct SpellConeEntry {
    pub(in crate::world) angle_degrees: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::world) struct AreaTriggerEntry {
    pub(in crate::world) id: u32,
    pub(in crate::world) map_id: u32,
    pub(in crate::world) x: f32,
    pub(in crate::world) y: f32,
    pub(in crate::world) z: f32,
    pub(in crate::world) radius: f32,
    pub(in crate::world) box_x: f32,
    pub(in crate::world) box_y: f32,
    pub(in crate::world) box_z: f32,
    pub(in crate::world) box_orientation: f32,
}

impl AreaTriggerEntry {
    pub(in crate::world) fn contains_position(&self, position: WorldPosition, delta: f32) -> bool {
        if position.map_id != self.map_id {
            return false;
        }
        if self.radius > 0.0 {
            let dx = position.x - self.x;
            let dy = position.y - self.y;
            let dz = position.z - self.z;
            return dx * dx + dy * dy + dz * dz <= (self.radius + delta) * (self.radius + delta);
        }

        let rotation = 2.0 * std::f32::consts::PI - self.box_orientation;
        let sin_val = rotation.sin();
        let cos_val = rotation.cos();
        let player_box_dist_x = position.x - self.x;
        let player_box_dist_y = position.y - self.y;
        let rot_player_x = self.x + player_box_dist_x * cos_val - player_box_dist_y * sin_val;
        let rot_player_y = self.y + player_box_dist_y * cos_val + player_box_dist_x * sin_val;
        let dx = rot_player_x - self.x;
        let dy = rot_player_y - self.y;
        let dz = position.z - self.z;
        dx.abs() <= self.box_x / 2.0 + delta
            && dy.abs() <= self.box_y / 2.0 + delta
            && dz.abs() <= self.box_z / 2.0 + delta
    }
}

#[derive(Debug, Clone, Default)]
pub(in crate::world) struct AreaTableStore {
    pub(in crate::world) entries: HashMap<u32, AreaTableEntry>,
    pub(in crate::world) entry_ids_by_explore_flag: HashMap<u16, Vec<u32>>,
}

impl AreaTableStore {
    pub(in crate::world) fn from_entries(entries: HashMap<u32, AreaTableEntry>) -> Self {
        let mut entry_ids_by_explore_flag: HashMap<u16, Vec<u32>> = HashMap::new();
        for entry in entries.values() {
            entry_ids_by_explore_flag
                .entry(entry.explore_flag)
                .or_default()
                .push(entry.id);
        }
        for ids in entry_ids_by_explore_flag.values_mut() {
            ids.sort_unstable();
        }
        Self {
            entries,
            entry_ids_by_explore_flag,
        }
    }

    pub(in crate::world) fn entry(&self, id: u32) -> Option<AreaTableEntry> {
        self.entries.get(&id).copied()
    }

    pub(in crate::world) fn entry_by_flag_and_map(
        &self,
        area_flag: u16,
        map_id: u32,
    ) -> Option<AreaTableEntry> {
        let ids = self.entry_ids_by_explore_flag.get(&area_flag)?;
        ids.iter()
            .filter_map(|id| self.entries.get(id).copied())
            .find(|entry| entry.map_id == map_id)
            .or_else(|| {
                ids.iter()
                    .filter_map(|id| self.entries.get(id).copied())
                    .next_back()
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct WmoAreaTableEntry {
    pub(in crate::world) id: u32,
    pub(in crate::world) root_id: i32,
    pub(in crate::world) adt_id: i32,
    pub(in crate::world) group_id: i32,
    pub(in crate::world) flags: u32,
    pub(in crate::world) area_id: u32,
}

#[derive(Debug, Clone, Default)]
pub(in crate::world) struct WmoAreaTableStore {
    entries_by_triple: HashMap<(i32, i32, i32), Vec<WmoAreaTableEntry>>,
}

impl WmoAreaTableStore {
    pub(in crate::world) fn from_entries(entries: HashMap<u32, WmoAreaTableEntry>) -> Self {
        let mut entries_by_triple: HashMap<(i32, i32, i32), Vec<WmoAreaTableEntry>> =
            HashMap::new();
        for entry in entries.values().copied() {
            entries_by_triple
                .entry((entry.root_id, entry.adt_id, entry.group_id))
                .or_default()
                .push(entry);
        }
        for entries in entries_by_triple.values_mut() {
            entries.sort_unstable_by_key(|entry| entry.id);
        }
        Self { entries_by_triple }
    }

    pub(in crate::world) fn entries_by_triple(
        &self,
        root_id: i32,
        adt_id: i32,
        group_id: i32,
    ) -> &[WmoAreaTableEntry] {
        self.entries_by_triple
            .get(&(root_id, adt_id, group_id))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct SkillLineAbilityEntry {
    pub(in crate::world) id: u32,
    pub(in crate::world) skill_id: u32,
    pub(in crate::world) spell_id: u32,
    pub(in crate::world) race_mask: u32,
    pub(in crate::world) class_mask: u32,
    pub(in crate::world) min_value: u32,
    pub(in crate::world) max_value: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct SkillLineEntry {
    pub(in crate::world) id: u32,
    pub(in crate::world) category_id: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct SkillRaceClassInfoEntry {
    pub(in crate::world) skill_id: u32,
    pub(in crate::world) race_mask: u32,
    pub(in crate::world) class_mask: u32,
    pub(in crate::world) flags: u32,
    pub(in crate::world) req_level: u32,
    pub(in crate::world) skill_tier_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct ItemRandomPropertyEntry {
    pub(in crate::world) id: u32,
    pub(in crate::world) enchant_ids: [u32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct SpellItemEnchantmentEntry {
    pub(in crate::world) id: u32,
    pub(in crate::world) effect_types: [u32; 3],
    pub(in crate::world) effect_amounts: [i32; 3],
    pub(in crate::world) effect_args: [u32; 3],
    pub(in crate::world) flags: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum FactionReaction {
    Hostile,
    Neutral,
    Friendly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct FactionTemplateEntry {
    pub(in crate::world) id: u32,
    pub(in crate::world) faction: u32,
    pub(in crate::world) faction_flags: u32,
    pub(in crate::world) faction_group_mask: u32,
    pub(in crate::world) friend_group_mask: u32,
    pub(in crate::world) enemy_group_mask: u32,
    pub(in crate::world) enemy_faction: [u32; 4],
    pub(in crate::world) friend_faction: [u32; 4],
}

#[derive(Debug, Clone)]
pub(in crate::world) struct FactionTemplateStore {
    pub(in crate::world) entries: HashMap<u32, FactionTemplateEntry>,
    pub(in crate::world) source: FactionTemplateStoreSource,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::world) enum FactionTemplateStoreSource {
    Dbc,
    #[default]
    FallbackBridge,
}

impl FactionTemplateStore {
    pub(in crate::world) fn fallback_bridge() -> Self {
        let entries = [
            faction_template_from_fields([
                1,
                1,
                72,
                FACTION_GROUP_MASK_PLAYER | FACTION_GROUP_MASK_ALLIANCE,
                FACTION_GROUP_MASK_ALLIANCE,
                FACTION_GROUP_MASK_HORDE,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            ]),
            faction_template_from_fields([
                2,
                2,
                72,
                FACTION_GROUP_MASK_PLAYER | FACTION_GROUP_MASK_HORDE,
                FACTION_GROUP_MASK_HORDE,
                FACTION_GROUP_MASK_ALLIANCE,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            ]),
            faction_template_from_fields([
                11,
                72,
                2081,
                FACTION_GROUP_MASK_PLAYER | FACTION_GROUP_MASK_ALLIANCE,
                FACTION_GROUP_MASK_ALLIANCE,
                FACTION_GROUP_MASK_HORDE | FACTION_GROUP_MASK_MONSTER,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            ]),
            faction_template_from_fields([
                12,
                72,
                0,
                FACTION_GROUP_MASK_ALLIANCE,
                FACTION_GROUP_MASK_ALLIANCE,
                FACTION_GROUP_MASK_HORDE,
                0,
                0,
                0,
                0,
                72,
                0,
                0,
                0,
            ]),
            faction_template_from_fields([
                GM_FRIENDLY_FACTION_TEMPLATE,
                GM_FRIENDLY_FACTION_TEMPLATE,
                0,
                FACTION_GROUP_MASK_ALLIANCE,
                FACTION_GROUP_MASK_PLAYER | FACTION_GROUP_MASK_ALLIANCE,
                FACTION_GROUP_MASK_HORDE,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            ]),
            faction_template_from_fields([
                14,
                14,
                0,
                FACTION_GROUP_MASK_MONSTER,
                0,
                FACTION_GROUP_MASK_PLAYER,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            ]),
            faction_template_from_fields([
                17,
                15,
                1,
                FACTION_GROUP_MASK_MONSTER,
                0,
                FACTION_GROUP_MASK_PLAYER,
                0,
                0,
                0,
                0,
                15,
                0,
                0,
                0,
            ]),
            faction_template_from_fields([
                25,
                25,
                0,
                FACTION_GROUP_MASK_MONSTER,
                0,
                0,
                0,
                0,
                0,
                0,
                25,
                0,
                0,
                0,
            ]),
            faction_template_from_fields([32, 29, 16, 0, 0, 0, 28, 0, 0, 0, 0, 0, 0, 0]),
        ]
        .into_iter()
        .map(|entry| (entry.id, entry))
        .collect();
        Self {
            entries,
            source: FactionTemplateStoreSource::FallbackBridge,
        }
    }

    pub(in crate::world) fn from_dbc(entries: HashMap<u32, FactionTemplateEntry>) -> Self {
        if entries.is_empty() {
            Self::fallback_bridge()
        } else {
            Self {
                entries,
                source: FactionTemplateStoreSource::Dbc,
            }
        }
    }

    pub(in crate::world) fn entry(&self, id: u32) -> Option<FactionTemplateEntry> {
        self.entries.get(&id).copied()
    }

    pub(in crate::world) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(in crate::world) fn is_dbc_backed(&self) -> bool {
        self.source == FactionTemplateStoreSource::Dbc
    }
}

impl Default for FactionTemplateStore {
    fn default() -> Self {
        Self::fallback_bridge()
    }
}

pub(in crate::world) const FACTION_GROUP_MASK_PLAYER: u32 = 1;
pub(in crate::world) const FACTION_GROUP_MASK_ALLIANCE: u32 = 2;
pub(in crate::world) const FACTION_GROUP_MASK_HORDE: u32 = 4;
pub(in crate::world) const FACTION_GROUP_MASK_MONSTER: u32 = 8;

pub(in crate::world) fn faction_template_from_fields(fields: [u32; 14]) -> FactionTemplateEntry {
    FactionTemplateEntry {
        id: fields[0],
        faction: fields[1],
        faction_flags: fields[2],
        faction_group_mask: fields[3],
        friend_group_mask: fields[4],
        enemy_group_mask: fields[5],
        enemy_faction: [fields[6], fields[7], fields[8], fields[9]],
        friend_faction: [fields[10], fields[11], fields[12], fields[13]],
    }
}

pub(in crate::world) fn load_faction_templates(path: &std::path::Path) -> FactionTemplateStore {
    let Ok(bytes) = std::fs::read(path) else {
        return FactionTemplateStore::fallback_bridge();
    };
    FactionTemplateStore::from_dbc(parse_faction_templates(&bytes))
}

pub(in crate::world) fn load_auction_houses(
    path: &std::path::Path,
) -> HashMap<u32, AuctionHouseEntry> {
    let Ok(bytes) = std::fs::read(path) else {
        return HashMap::new();
    };
    parse_auction_houses(&bytes)
}

pub(in crate::world) fn parse_auction_houses(bytes: &[u8]) -> HashMap<u32, AuctionHouseEntry> {
    const DBC_HEADER_SIZE: usize = 20;
    const AUCTION_HOUSE_FIELD_COUNT: usize = 4;
    if bytes.len() < DBC_HEADER_SIZE || &bytes[0..4] != b"WDBC" {
        return HashMap::new();
    }
    let record_count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let field_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let record_size = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if field_count < AUCTION_HOUSE_FIELD_COUNT || record_size < AUCTION_HOUSE_FIELD_COUNT * 4 {
        return HashMap::new();
    }
    let records_size = record_count.saturating_mul(record_size);
    if bytes.len() < DBC_HEADER_SIZE + records_size {
        return HashMap::new();
    }

    let mut entries = HashMap::with_capacity(record_count);
    for record_index in 0..record_count {
        let record_offset = DBC_HEADER_SIZE + record_index * record_size;
        let field = |index: usize| {
            let offset = record_offset + index * 4;
            u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
        };
        let house_id = field(0);
        if house_id == 0 {
            continue;
        }
        entries.insert(
            house_id,
            AuctionHouseEntry {
                house_id,
                faction: field(1),
                deposit_percent: field(2),
                cut_percent: field(3),
            },
        );
    }
    entries
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::world) struct TaxiPathNodeEntry {
    pub(in crate::world) id: u32,
    pub(in crate::world) path: u32,
    pub(in crate::world) index: u32,
    pub(in crate::world) map_id: u32,
    pub(in crate::world) x: f32,
    pub(in crate::world) y: f32,
    pub(in crate::world) z: f32,
    pub(in crate::world) action_flag: u32,
    pub(in crate::world) delay: u32,
}

impl TaxiPathNodeEntry {
    pub(in crate::world) fn position(self) -> WorldPosition {
        WorldPosition::new(self.map_id, self.x, self.y, self.z, 0.0)
    }
}

pub(in crate::world) fn load_taxi_nodes(path: &std::path::Path) -> HashMap<u32, TaxiNodeEntry> {
    let Ok(bytes) = std::fs::read(path) else {
        return HashMap::new();
    };
    parse_taxi_nodes(&bytes)
}

pub(in crate::world) fn parse_taxi_nodes(bytes: &[u8]) -> HashMap<u32, TaxiNodeEntry> {
    const DBC_HEADER_SIZE: usize = 20;
    const TAXI_NODE_FIELD_COUNT: usize = 16;
    const HORDE_MOUNT_FIELD: usize = 14;
    const ALLIANCE_MOUNT_FIELD: usize = 15;
    if bytes.len() < DBC_HEADER_SIZE || &bytes[0..4] != b"WDBC" {
        return HashMap::new();
    }
    let record_count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let field_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let record_size = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if field_count < TAXI_NODE_FIELD_COUNT || record_size < TAXI_NODE_FIELD_COUNT * 4 {
        return HashMap::new();
    }
    let records_size = record_count.saturating_mul(record_size);
    if bytes.len() < DBC_HEADER_SIZE + records_size {
        return HashMap::new();
    }

    let mut entries = HashMap::with_capacity(record_count);
    for record_index in 0..record_count {
        let record_offset = DBC_HEADER_SIZE + record_index * record_size;
        let field = |index: usize| {
            let offset = record_offset + index * 4;
            u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
        };
        let f32_field = |index: usize| {
            let offset = record_offset + index * 4;
            f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
        };
        let id = field(0);
        if id == 0 {
            continue;
        }
        entries.insert(
            id,
            TaxiNodeEntry {
                id,
                map_id: field(1),
                x: f32_field(2),
                y: f32_field(3),
                z: f32_field(4),
                horde_mount_creature: field(HORDE_MOUNT_FIELD),
                alliance_mount_creature: field(ALLIANCE_MOUNT_FIELD),
            },
        );
    }
    entries
}

pub(in crate::world) fn load_taxi_paths(
    path: &std::path::Path,
) -> HashMap<(u32, u32), TaxiPathEntry> {
    let Ok(bytes) = std::fs::read(path) else {
        return HashMap::new();
    };
    parse_taxi_paths(&bytes)
}

pub(in crate::world) fn parse_taxi_paths(bytes: &[u8]) -> HashMap<(u32, u32), TaxiPathEntry> {
    const DBC_HEADER_SIZE: usize = 20;
    const TAXI_PATH_FIELD_COUNT: usize = 4;
    if bytes.len() < DBC_HEADER_SIZE || &bytes[0..4] != b"WDBC" {
        return HashMap::new();
    }
    let record_count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let field_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let record_size = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if field_count < TAXI_PATH_FIELD_COUNT || record_size < TAXI_PATH_FIELD_COUNT * 4 {
        return HashMap::new();
    }
    let records_size = record_count.saturating_mul(record_size);
    if bytes.len() < DBC_HEADER_SIZE + records_size {
        return HashMap::new();
    }

    let mut entries = HashMap::with_capacity(record_count);
    for record_index in 0..record_count {
        let record_offset = DBC_HEADER_SIZE + record_index * record_size;
        let field = |index: usize| {
            let offset = record_offset + index * 4;
            u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
        };
        let id = field(0);
        let source = field(1);
        let destination = field(2);
        if id == 0 || source == 0 || destination == 0 {
            continue;
        }
        entries.insert(
            (source, destination),
            TaxiPathEntry {
                id,
                source,
                destination,
                price: field(3),
            },
        );
    }
    entries
}

pub(in crate::world) fn load_taxi_path_nodes(
    path: &std::path::Path,
) -> HashMap<u32, Vec<TaxiPathNodeEntry>> {
    let Ok(bytes) = std::fs::read(path) else {
        return HashMap::new();
    };
    parse_taxi_path_nodes(&bytes)
}

pub(in crate::world) fn parse_taxi_path_nodes(
    bytes: &[u8],
) -> HashMap<u32, Vec<TaxiPathNodeEntry>> {
    const DBC_HEADER_SIZE: usize = 20;
    const TAXI_PATH_NODE_FIELD_COUNT: usize = 9;
    if bytes.len() < DBC_HEADER_SIZE || &bytes[0..4] != b"WDBC" {
        return HashMap::new();
    }
    let record_count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let field_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let record_size = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if field_count < TAXI_PATH_NODE_FIELD_COUNT || record_size < TAXI_PATH_NODE_FIELD_COUNT * 4 {
        return HashMap::new();
    }
    let records_size = record_count.saturating_mul(record_size);
    if bytes.len() < DBC_HEADER_SIZE + records_size {
        return HashMap::new();
    }

    let mut entries: HashMap<u32, Vec<TaxiPathNodeEntry>> = HashMap::new();
    for record_index in 0..record_count {
        let record_offset = DBC_HEADER_SIZE + record_index * record_size;
        let field = |index: usize| {
            let offset = record_offset + index * 4;
            u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
        };
        let f32_field = |index: usize| {
            let offset = record_offset + index * 4;
            f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
        };
        let id = field(0);
        let path = field(1);
        if id == 0 || path == 0 {
            continue;
        }
        entries.entry(path).or_default().push(TaxiPathNodeEntry {
            id,
            path,
            index: field(2),
            map_id: field(3),
            x: f32_field(4),
            y: f32_field(5),
            z: f32_field(6),
            action_flag: field(7),
            delay: field(8),
        });
    }
    for nodes in entries.values_mut() {
        nodes.sort_by_key(|node| node.index);
    }
    entries
}

pub(in crate::world) fn taxi_network_mask(
    nodes: &HashMap<u32, TaxiNodeEntry>,
    paths: &HashMap<(u32, u32), TaxiPathEntry>,
) -> [u32; 8] {
    let mut mask = [0u32; 8];
    for node in nodes.keys().copied() {
        if paths.keys().any(|(source, _)| *source == node) {
            set_taxi_mask_node(&mut mask, node);
        }
    }
    mask
}

pub(in crate::world) fn taxi_mask_has_node(taximask: [u32; 8], node: u32) -> bool {
    if node == 0 {
        return false;
    }
    let index = ((node - 1) / 32) as usize;
    if index >= taximask.len() {
        return false;
    }
    let submask = 1u32 << ((node - 1) % 32);
    taximask[index] & submask == submask
}

pub(in crate::world) fn set_taxi_mask_node(taximask: &mut [u32; 8], node: u32) -> bool {
    if node == 0 {
        return false;
    }
    let index = ((node - 1) / 32) as usize;
    if index >= taximask.len() {
        return false;
    }
    let submask = 1u32 << ((node - 1) % 32);
    let learned = taximask[index] & submask == 0;
    taximask[index] |= submask;
    learned
}

pub(in crate::world) fn parse_faction_templates(
    bytes: &[u8],
) -> HashMap<u32, FactionTemplateEntry> {
    const DBC_HEADER_SIZE: usize = 20;
    const FACTION_TEMPLATE_FIELD_COUNT: usize = 14;
    if bytes.len() < DBC_HEADER_SIZE || &bytes[0..4] != b"WDBC" {
        return HashMap::new();
    }
    let record_count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let field_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let record_size = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if field_count != FACTION_TEMPLATE_FIELD_COUNT || record_size < FACTION_TEMPLATE_FIELD_COUNT * 4
    {
        return HashMap::new();
    }
    let records_size = record_count.saturating_mul(record_size);
    if bytes.len() < DBC_HEADER_SIZE + records_size {
        return HashMap::new();
    }

    let mut templates = HashMap::with_capacity(record_count);
    for record_index in 0..record_count {
        let record_offset = DBC_HEADER_SIZE + record_index * record_size;
        let field = |index: usize| {
            let offset = record_offset + index * 4;
            u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
        };
        let id = field(0);
        if id == 0 {
            continue;
        }
        templates.insert(
            id,
            faction_template_from_fields([
                id,
                field(1),
                field(2),
                field(3),
                field(4),
                field(5),
                field(6),
                field(7),
                field(8),
                field(9),
                field(10),
                field(11),
                field(12),
                field(13),
            ]),
        );
    }
    templates
}

pub(in crate::world) fn load_item_random_properties(
    path: &std::path::Path,
) -> HashMap<u32, ItemRandomPropertyEntry> {
    let Ok(bytes) = std::fs::read(path) else {
        return HashMap::new();
    };
    parse_item_random_properties(&bytes)
}

pub(in crate::world) fn parse_item_random_properties(
    bytes: &[u8],
) -> HashMap<u32, ItemRandomPropertyEntry> {
    const DBC_HEADER_SIZE: usize = 20;
    const MIN_ITEM_RANDOM_PROPERTY_FIELDS: usize = 5;
    if bytes.len() < DBC_HEADER_SIZE || &bytes[0..4] != b"WDBC" {
        return HashMap::new();
    }
    let record_count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let field_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let record_size = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if field_count < MIN_ITEM_RANDOM_PROPERTY_FIELDS
        || record_size < MIN_ITEM_RANDOM_PROPERTY_FIELDS * 4
    {
        return HashMap::new();
    }
    let records_size = record_count.saturating_mul(record_size);
    if bytes.len() < DBC_HEADER_SIZE + records_size {
        return HashMap::new();
    }

    let mut properties = HashMap::with_capacity(record_count);
    for record_index in 0..record_count {
        let record_offset = DBC_HEADER_SIZE + record_index * record_size;
        let field = |index: usize| {
            let offset = record_offset + index * 4;
            u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
        };
        let id = field(0);
        if id == 0 {
            continue;
        }
        properties.insert(
            id,
            ItemRandomPropertyEntry {
                id,
                enchant_ids: [field(2), field(3), field(4)],
            },
        );
    }
    properties
}

pub(in crate::world) fn load_spell_item_enchantments(
    path: &std::path::Path,
) -> HashMap<u32, SpellItemEnchantmentEntry> {
    let Ok(bytes) = std::fs::read(path) else {
        return HashMap::new();
    };
    parse_spell_item_enchantments(&bytes)
}

pub(in crate::world) fn parse_spell_item_enchantments(
    bytes: &[u8],
) -> HashMap<u32, SpellItemEnchantmentEntry> {
    const DBC_HEADER_SIZE: usize = 20;
    const SPELL_ITEM_ENCHANTMENT_FIELD_COUNT: usize = 24;
    if bytes.len() < DBC_HEADER_SIZE || &bytes[0..4] != b"WDBC" {
        return HashMap::new();
    }
    let record_count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let field_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let record_size = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if field_count < SPELL_ITEM_ENCHANTMENT_FIELD_COUNT
        || record_size < SPELL_ITEM_ENCHANTMENT_FIELD_COUNT * 4
    {
        return HashMap::new();
    }
    let records_size = record_count.saturating_mul(record_size);
    if bytes.len() < DBC_HEADER_SIZE + records_size {
        return HashMap::new();
    }

    let mut enchantments = HashMap::with_capacity(record_count);
    for record_index in 0..record_count {
        let record_offset = DBC_HEADER_SIZE + record_index * record_size;
        let field = |index: usize| {
            let offset = record_offset + index * 4;
            u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
        };
        let signed_field = |index: usize| {
            let offset = record_offset + index * 4;
            i32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
        };
        let id = field(0);
        if id == 0 {
            continue;
        }
        enchantments.insert(
            id,
            SpellItemEnchantmentEntry {
                id,
                effect_types: [field(1), field(2), field(3)],
                effect_amounts: [signed_field(4), signed_field(5), signed_field(6)],
                effect_args: [field(10), field(11), field(12)],
                flags: field(23),
            },
        );
    }
    enchantments
}

pub(in crate::world) fn load_bank_bag_slot_prices(path: &std::path::Path) -> HashMap<u32, u32> {
    let Ok(bytes) = std::fs::read(path) else {
        return HashMap::new();
    };
    parse_bank_bag_slot_prices(&bytes)
}

pub(in crate::world) fn parse_bank_bag_slot_prices(bytes: &[u8]) -> HashMap<u32, u32> {
    const DBC_HEADER_SIZE: usize = 20;
    const BANK_BAG_SLOT_PRICE_FIELDS: usize = 2;
    if bytes.len() < DBC_HEADER_SIZE || &bytes[0..4] != b"WDBC" {
        return HashMap::new();
    }
    let record_count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let field_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let record_size = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if field_count != BANK_BAG_SLOT_PRICE_FIELDS || record_size < BANK_BAG_SLOT_PRICE_FIELDS * 4 {
        return HashMap::new();
    }
    let records_size = record_count.saturating_mul(record_size);
    if bytes.len() < DBC_HEADER_SIZE + records_size {
        return HashMap::new();
    }

    let mut prices = HashMap::with_capacity(record_count);
    for record_index in 0..record_count {
        let record_offset = DBC_HEADER_SIZE + record_index * record_size;
        let id = u32::from_le_bytes(bytes[record_offset..record_offset + 4].try_into().unwrap());
        let cost = u32::from_le_bytes(
            bytes[record_offset + 4..record_offset + 8]
                .try_into()
                .unwrap(),
        );
        if id != 0 {
            prices.insert(id, cost);
        }
    }
    prices
}

pub(in crate::world) fn load_area_tables(path: &std::path::Path) -> AreaTableStore {
    let Ok(bytes) = std::fs::read(path) else {
        return AreaTableStore::default();
    };
    AreaTableStore::from_entries(parse_area_tables(&bytes))
}

pub(in crate::world) fn load_area_triggers(
    path: &std::path::Path,
) -> HashMap<u32, AreaTriggerEntry> {
    let Ok(bytes) = std::fs::read(path) else {
        return HashMap::new();
    };
    parse_area_triggers(&bytes)
}

pub(in crate::world) fn parse_area_triggers(bytes: &[u8]) -> HashMap<u32, AreaTriggerEntry> {
    const DBC_HEADER_SIZE: usize = 20;
    const AREA_TRIGGER_MIN_FIELD_COUNT: usize = 10;
    const AREA_TRIGGER_ID_FIELD: usize = 0;
    const AREA_TRIGGER_MAP_FIELD: usize = 1;
    const AREA_TRIGGER_X_FIELD: usize = 2;
    const AREA_TRIGGER_Y_FIELD: usize = 3;
    const AREA_TRIGGER_Z_FIELD: usize = 4;
    const AREA_TRIGGER_RADIUS_FIELD: usize = 5;
    const AREA_TRIGGER_BOX_X_FIELD: usize = 6;
    const AREA_TRIGGER_BOX_Y_FIELD: usize = 7;
    const AREA_TRIGGER_BOX_Z_FIELD: usize = 8;
    const AREA_TRIGGER_BOX_ORIENTATION_FIELD: usize = 9;
    if bytes.len() < DBC_HEADER_SIZE || &bytes[0..4] != b"WDBC" {
        return HashMap::new();
    }
    let record_count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let field_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let record_size = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if field_count < AREA_TRIGGER_MIN_FIELD_COUNT || record_size < field_count * 4 {
        return HashMap::new();
    }
    let records_size = record_count.saturating_mul(record_size);
    if bytes.len() < DBC_HEADER_SIZE + records_size {
        return HashMap::new();
    }

    let mut triggers = HashMap::with_capacity(record_count);
    for record_index in 0..record_count {
        let record_offset = DBC_HEADER_SIZE + record_index * record_size;
        let field_u32 = |index: usize| {
            let offset = record_offset + index * 4;
            u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
        };
        let field_f32 = |index: usize| f32::from_bits(field_u32(index));
        let id = field_u32(AREA_TRIGGER_ID_FIELD);
        if id == 0 {
            continue;
        }
        triggers.insert(
            id,
            AreaTriggerEntry {
                id,
                map_id: field_u32(AREA_TRIGGER_MAP_FIELD),
                x: field_f32(AREA_TRIGGER_X_FIELD),
                y: field_f32(AREA_TRIGGER_Y_FIELD),
                z: field_f32(AREA_TRIGGER_Z_FIELD),
                radius: field_f32(AREA_TRIGGER_RADIUS_FIELD),
                box_x: field_f32(AREA_TRIGGER_BOX_X_FIELD),
                box_y: field_f32(AREA_TRIGGER_BOX_Y_FIELD),
                box_z: field_f32(AREA_TRIGGER_BOX_Z_FIELD),
                box_orientation: field_f32(AREA_TRIGGER_BOX_ORIENTATION_FIELD),
            },
        );
    }
    triggers
}

pub(in crate::world) fn parse_area_tables(bytes: &[u8]) -> HashMap<u32, AreaTableEntry> {
    const DBC_HEADER_SIZE: usize = 20;
    const AREA_TABLE_MIN_FIELD_COUNT: usize = 25;
    const AREA_TABLE_ID_FIELD: usize = 0;
    const AREA_TABLE_MAP_FIELD: usize = 1;
    const AREA_TABLE_ZONE_FIELD: usize = 2;
    const AREA_TABLE_EXPLORE_FLAG_FIELD: usize = 3;
    const AREA_TABLE_FLAGS_FIELD: usize = 4;
    const AREA_TABLE_LEVEL_FIELD: usize = 10;
    if bytes.len() < DBC_HEADER_SIZE || &bytes[0..4] != b"WDBC" {
        return HashMap::new();
    }
    let record_count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let field_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let record_size = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if field_count < AREA_TABLE_MIN_FIELD_COUNT || record_size < field_count * 4 {
        return HashMap::new();
    }
    let records_size = record_count.saturating_mul(record_size);
    if bytes.len() < DBC_HEADER_SIZE + records_size {
        return HashMap::new();
    }

    let mut areas = HashMap::with_capacity(record_count);
    for record_index in 0..record_count {
        let record_offset = DBC_HEADER_SIZE + record_index * record_size;
        let field = |index: usize| {
            let offset = record_offset + index * 4;
            u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
        };
        let id = field(AREA_TABLE_ID_FIELD);
        if id == 0 {
            continue;
        }
        let explore_flag = field(AREA_TABLE_EXPLORE_FLAG_FIELD);
        if explore_flag > u32::from(u16::MAX) {
            continue;
        }
        areas.insert(
            id,
            AreaTableEntry {
                id,
                map_id: field(AREA_TABLE_MAP_FIELD),
                zone_id: field(AREA_TABLE_ZONE_FIELD),
                explore_flag: explore_flag as u16,
                flags: field(AREA_TABLE_FLAGS_FIELD),
                area_level: field(AREA_TABLE_LEVEL_FIELD).min(u32::from(u8::MAX)) as u8,
            },
        );
    }
    areas
}

pub(in crate::world) fn load_wmo_area_tables(path: &std::path::Path) -> WmoAreaTableStore {
    let Ok(bytes) = std::fs::read(path) else {
        return WmoAreaTableStore::default();
    };
    WmoAreaTableStore::from_entries(parse_wmo_area_tables(&bytes))
}

pub(in crate::world) fn parse_wmo_area_tables(bytes: &[u8]) -> HashMap<u32, WmoAreaTableEntry> {
    const DBC_HEADER_SIZE: usize = 20;
    const WMO_AREA_TABLE_MIN_FIELD_COUNT: usize = 21;
    const WMO_AREA_TABLE_ID_FIELD: usize = 0;
    const WMO_AREA_TABLE_ROOT_ID_FIELD: usize = 1;
    const WMO_AREA_TABLE_ADT_ID_FIELD: usize = 2;
    const WMO_AREA_TABLE_GROUP_ID_FIELD: usize = 3;
    const WMO_AREA_TABLE_FLAGS_FIELD: usize = 9;
    const WMO_AREA_TABLE_AREA_ID_FIELD: usize = 10;
    if bytes.len() < DBC_HEADER_SIZE || &bytes[0..4] != b"WDBC" {
        return HashMap::new();
    }
    let record_count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let field_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let record_size = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if field_count < WMO_AREA_TABLE_MIN_FIELD_COUNT || record_size < field_count * 4 {
        return HashMap::new();
    }
    let records_size = record_count.saturating_mul(record_size);
    if bytes.len() < DBC_HEADER_SIZE + records_size {
        return HashMap::new();
    }

    let mut areas = HashMap::with_capacity(record_count);
    for record_index in 0..record_count {
        let record_offset = DBC_HEADER_SIZE + record_index * record_size;
        let field = |index: usize| {
            let offset = record_offset + index * 4;
            u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
        };
        let signed_field = |index: usize| {
            let offset = record_offset + index * 4;
            i32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
        };
        let id = field(WMO_AREA_TABLE_ID_FIELD);
        if id == 0 {
            continue;
        }
        areas.insert(
            id,
            WmoAreaTableEntry {
                id,
                root_id: signed_field(WMO_AREA_TABLE_ROOT_ID_FIELD),
                adt_id: signed_field(WMO_AREA_TABLE_ADT_ID_FIELD),
                group_id: signed_field(WMO_AREA_TABLE_GROUP_ID_FIELD),
                flags: field(WMO_AREA_TABLE_FLAGS_FIELD),
                area_id: field(WMO_AREA_TABLE_AREA_ID_FIELD),
            },
        );
    }
    areas
}

pub(in crate::world) fn load_creature_display_info_scales(
    path: &std::path::Path,
) -> HashMap<u32, f32> {
    let Ok(bytes) = std::fs::read(path) else {
        return HashMap::new();
    };
    parse_creature_display_info_scales(&bytes)
}

pub(in crate::world) fn parse_creature_display_info_scales(bytes: &[u8]) -> HashMap<u32, f32> {
    const DBC_HEADER_SIZE: usize = 20;
    const CREATURE_DISPLAY_INFO_SCALE_FIELD: usize = 4;
    if bytes.len() < DBC_HEADER_SIZE || &bytes[0..4] != b"WDBC" {
        return HashMap::new();
    }
    let record_count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let field_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let record_size = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if field_count <= CREATURE_DISPLAY_INFO_SCALE_FIELD || record_size < field_count * 4 {
        return HashMap::new();
    }
    let records_size = record_count.saturating_mul(record_size);
    if bytes.len() < DBC_HEADER_SIZE + records_size {
        return HashMap::new();
    }

    let mut scales = HashMap::with_capacity(record_count);
    for record_index in 0..record_count {
        let record_offset = DBC_HEADER_SIZE + record_index * record_size;
        let display_id =
            u32::from_le_bytes(bytes[record_offset..record_offset + 4].try_into().unwrap());
        let scale_offset = record_offset + CREATURE_DISPLAY_INFO_SCALE_FIELD * 4;
        let scale = f32::from_le_bytes(bytes[scale_offset..scale_offset + 4].try_into().unwrap());
        if display_id != 0 && scale > 0.0 {
            scales.insert(display_id, scale);
        }
    }
    scales
}

pub(in crate::world) fn load_spell_durations(
    path: &std::path::Path,
) -> HashMap<u32, SpellDurationEntry> {
    let Ok(bytes) = std::fs::read(path) else {
        return HashMap::new();
    };
    parse_spell_durations(&bytes)
}

pub(in crate::world) fn load_spell_cast_times(
    path: &std::path::Path,
) -> HashMap<u32, SpellCastTimeEntry> {
    let Ok(bytes) = std::fs::read(path) else {
        return HashMap::new();
    };
    parse_spell_cast_times(&bytes)
}

pub(in crate::world) fn load_spell_ranges(path: &std::path::Path) -> HashMap<u32, SpellRangeEntry> {
    let Ok(bytes) = std::fs::read(path) else {
        return HashMap::new();
    };
    parse_spell_ranges(&bytes)
}

pub(in crate::world) fn load_spell_radii(path: &std::path::Path) -> HashMap<u32, SpellRadiusEntry> {
    let Ok(bytes) = std::fs::read(path) else {
        return HashMap::new();
    };
    parse_spell_radii(&bytes)
}

pub(in crate::world) fn load_spell_cones(
    data_dir: &std::path::Path,
) -> HashMap<u32, SpellConeEntry> {
    let candidates = [
        data_dir
            .join("dbc")
            .join("original_data")
            .join("SpellCone.sql"),
        data_dir.join("SpellCone.sql"),
        std::env::current_dir()
            .unwrap_or_default()
            .join("sql")
            .join("base")
            .join("dbc")
            .join("original_data")
            .join("SpellCone.sql"),
    ];
    candidates
        .iter()
        .find_map(|path| std::fs::read_to_string(path).ok())
        .map(|sql| parse_spell_cones_sql(&sql))
        .unwrap_or_default()
}

pub(in crate::world) fn parse_spell_cones_sql(sql: &str) -> HashMap<u32, SpellConeEntry> {
    let Some(values_start) = sql.find("VALUES") else {
        return HashMap::new();
    };
    let mut entries = HashMap::new();
    for tuple in sql[values_start..].split('(').skip(1) {
        let Some(tuple) = tuple.split(')').next() else {
            continue;
        };
        let mut fields = tuple.split(',').map(str::trim);
        let Some(id) = fields.next().and_then(|value| value.parse::<u32>().ok()) else {
            continue;
        };
        let Some(angle_degrees) = fields.next().and_then(|value| value.parse::<i32>().ok()) else {
            continue;
        };
        entries.insert(id, SpellConeEntry { angle_degrees });
    }
    entries
}

pub(in crate::world) fn load_skill_line_abilities(
    path: &std::path::Path,
) -> HashMap<u32, Vec<SkillLineAbilityEntry>> {
    let Ok(bytes) = std::fs::read(path) else {
        return HashMap::new();
    };
    parse_skill_line_abilities(&bytes)
}

pub(in crate::world) fn load_skill_lines(path: &std::path::Path) -> HashMap<u32, SkillLineEntry> {
    let Ok(bytes) = std::fs::read(path) else {
        return HashMap::new();
    };
    parse_skill_lines(&bytes)
}

pub(in crate::world) fn load_skill_race_class_infos(
    path: &std::path::Path,
) -> HashMap<u32, Vec<SkillRaceClassInfoEntry>> {
    let Ok(bytes) = std::fs::read(path) else {
        return HashMap::new();
    };
    parse_skill_race_class_infos(&bytes)
}

pub(in crate::world) fn parse_spell_cast_times(bytes: &[u8]) -> HashMap<u32, SpellCastTimeEntry> {
    const DBC_HEADER_SIZE: usize = 20;
    const SPELL_CAST_TIME_FIELD_COUNT: usize = 4;
    if bytes.len() < DBC_HEADER_SIZE || &bytes[0..4] != b"WDBC" {
        return HashMap::new();
    }
    let record_count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let field_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let record_size = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if field_count != SPELL_CAST_TIME_FIELD_COUNT || record_size < SPELL_CAST_TIME_FIELD_COUNT * 4 {
        return HashMap::new();
    }
    let records_size = record_count.saturating_mul(record_size);
    if bytes.len() < DBC_HEADER_SIZE + records_size {
        return HashMap::new();
    }

    let mut cast_times = HashMap::with_capacity(record_count);
    for record_index in 0..record_count {
        let record_offset = DBC_HEADER_SIZE + record_index * record_size;
        let id = u32::from_le_bytes(bytes[record_offset..record_offset + 4].try_into().unwrap());
        let cast_time_millis = i32::from_le_bytes(
            bytes[record_offset + 4..record_offset + 8]
                .try_into()
                .unwrap(),
        );
        let cast_time_per_level_millis = i32::from_le_bytes(
            bytes[record_offset + 8..record_offset + 12]
                .try_into()
                .unwrap(),
        );
        let min_cast_time_millis = i32::from_le_bytes(
            bytes[record_offset + 12..record_offset + 16]
                .try_into()
                .unwrap(),
        );
        if id != 0 {
            cast_times.insert(
                id,
                SpellCastTimeEntry {
                    cast_time_millis,
                    cast_time_per_level_millis,
                    min_cast_time_millis,
                },
            );
        }
    }
    cast_times
}

pub(in crate::world) fn parse_spell_ranges(bytes: &[u8]) -> HashMap<u32, SpellRangeEntry> {
    const DBC_HEADER_SIZE: usize = 20;
    const SPELL_RANGE_FIELD_COUNT: usize = 22;
    if bytes.len() < DBC_HEADER_SIZE || &bytes[0..4] != b"WDBC" {
        return HashMap::new();
    }
    let record_count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let field_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let record_size = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if field_count != SPELL_RANGE_FIELD_COUNT || record_size < SPELL_RANGE_FIELD_COUNT * 4 {
        return HashMap::new();
    }
    let records_size = record_count.saturating_mul(record_size);
    if bytes.len() < DBC_HEADER_SIZE + records_size {
        return HashMap::new();
    }

    let mut ranges = HashMap::with_capacity(record_count);
    for record_index in 0..record_count {
        let record_offset = DBC_HEADER_SIZE + record_index * record_size;
        let id = u32::from_le_bytes(bytes[record_offset..record_offset + 4].try_into().unwrap());
        let min_range = f32::from_le_bytes(
            bytes[record_offset + 4..record_offset + 8]
                .try_into()
                .unwrap(),
        );
        let max_range = f32::from_le_bytes(
            bytes[record_offset + 8..record_offset + 12]
                .try_into()
                .unwrap(),
        );
        let flags = u32::from_le_bytes(
            bytes[record_offset + 12..record_offset + 16]
                .try_into()
                .unwrap(),
        );
        if id != 0 {
            ranges.insert(
                id,
                SpellRangeEntry {
                    min_range,
                    max_range,
                    flags,
                },
            );
        }
    }
    ranges
}

pub(in crate::world) fn parse_spell_radii(bytes: &[u8]) -> HashMap<u32, SpellRadiusEntry> {
    const DBC_HEADER_SIZE: usize = 20;
    const SPELL_RADIUS_FIELD_COUNT: usize = 4;
    if bytes.len() < DBC_HEADER_SIZE || &bytes[0..4] != b"WDBC" {
        return HashMap::new();
    }
    let record_count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let field_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let record_size = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if field_count != SPELL_RADIUS_FIELD_COUNT || record_size < SPELL_RADIUS_FIELD_COUNT * 4 {
        return HashMap::new();
    }
    let records_size = record_count.saturating_mul(record_size);
    if bytes.len() < DBC_HEADER_SIZE + records_size {
        return HashMap::new();
    }

    let mut radii = HashMap::with_capacity(record_count);
    for record_index in 0..record_count {
        let record_offset = DBC_HEADER_SIZE + record_index * record_size;
        let id = u32::from_le_bytes(bytes[record_offset..record_offset + 4].try_into().unwrap());
        let radius = f32::from_le_bytes(
            bytes[record_offset + 4..record_offset + 8]
                .try_into()
                .unwrap(),
        );
        let radius_per_level = f32::from_le_bytes(
            bytes[record_offset + 8..record_offset + 12]
                .try_into()
                .unwrap(),
        );
        let max_radius = f32::from_le_bytes(
            bytes[record_offset + 12..record_offset + 16]
                .try_into()
                .unwrap(),
        );
        if id != 0 {
            radii.insert(
                id,
                SpellRadiusEntry {
                    radius,
                    radius_per_level,
                    max_radius,
                },
            );
        }
    }
    radii
}

pub(in crate::world) fn parse_skill_line_abilities(
    bytes: &[u8],
) -> HashMap<u32, Vec<SkillLineAbilityEntry>> {
    const DBC_HEADER_SIZE: usize = 20;
    const SKILL_LINE_ABILITY_FIELD_COUNT: usize = 15;
    if bytes.len() < DBC_HEADER_SIZE || &bytes[0..4] != b"WDBC" {
        return HashMap::new();
    }
    let record_count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let field_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let record_size = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if field_count < SKILL_LINE_ABILITY_FIELD_COUNT
        || record_size < SKILL_LINE_ABILITY_FIELD_COUNT * 4
    {
        return HashMap::new();
    }
    let records_size = record_count.saturating_mul(record_size);
    if bytes.len() < DBC_HEADER_SIZE + records_size {
        return HashMap::new();
    }

    let mut abilities: HashMap<u32, Vec<SkillLineAbilityEntry>> = HashMap::new();
    for record_index in 0..record_count {
        let record_offset = DBC_HEADER_SIZE + record_index * record_size;
        let field = |index: usize| {
            let offset = record_offset + index * 4;
            u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
        };
        let id = field(0);
        let skill_id = field(1);
        let spell_id = field(2);
        if id == 0 || skill_id == 0 || spell_id == 0 {
            continue;
        }
        abilities
            .entry(spell_id)
            .or_default()
            .push(SkillLineAbilityEntry {
                id,
                skill_id,
                spell_id,
                race_mask: field(3),
                class_mask: field(4),
                min_value: field(11),
                max_value: field(10),
            });
    }
    abilities
}

pub(in crate::world) fn parse_skill_lines(bytes: &[u8]) -> HashMap<u32, SkillLineEntry> {
    const DBC_HEADER_SIZE: usize = 20;
    const SKILL_LINE_FIELD_COUNT: usize = 22;
    if bytes.len() < DBC_HEADER_SIZE || &bytes[0..4] != b"WDBC" {
        return HashMap::new();
    }
    let record_count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let field_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let record_size = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if field_count < SKILL_LINE_FIELD_COUNT || record_size < SKILL_LINE_FIELD_COUNT * 4 {
        return HashMap::new();
    }
    let records_size = record_count.saturating_mul(record_size);
    if bytes.len() < DBC_HEADER_SIZE + records_size {
        return HashMap::new();
    }

    let mut lines = HashMap::with_capacity(record_count);
    for record_index in 0..record_count {
        let record_offset = DBC_HEADER_SIZE + record_index * record_size;
        let id = u32::from_le_bytes(bytes[record_offset..record_offset + 4].try_into().unwrap());
        if id == 0 {
            continue;
        }
        let category_id = i32::from_le_bytes(
            bytes[record_offset + 4..record_offset + 8]
                .try_into()
                .unwrap(),
        );
        lines.insert(id, SkillLineEntry { id, category_id });
    }
    lines
}

pub(in crate::world) fn parse_skill_race_class_infos(
    bytes: &[u8],
) -> HashMap<u32, Vec<SkillRaceClassInfoEntry>> {
    const DBC_HEADER_SIZE: usize = 20;
    const SKILL_RACE_CLASS_INFO_FIELD_COUNT: usize = 8;
    if bytes.len() < DBC_HEADER_SIZE || &bytes[0..4] != b"WDBC" {
        return HashMap::new();
    }
    let record_count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let field_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let record_size = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if field_count < SKILL_RACE_CLASS_INFO_FIELD_COUNT
        || record_size < SKILL_RACE_CLASS_INFO_FIELD_COUNT * 4
    {
        return HashMap::new();
    }
    let records_size = record_count.saturating_mul(record_size);
    if bytes.len() < DBC_HEADER_SIZE + records_size {
        return HashMap::new();
    }

    let mut infos: HashMap<u32, Vec<SkillRaceClassInfoEntry>> = HashMap::new();
    for record_index in 0..record_count {
        let record_offset = DBC_HEADER_SIZE + record_index * record_size;
        let field = |index: usize| {
            let offset = record_offset + index * 4;
            u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
        };
        let skill_id = field(1);
        if skill_id == 0 {
            continue;
        }
        infos
            .entry(skill_id)
            .or_default()
            .push(SkillRaceClassInfoEntry {
                skill_id,
                race_mask: field(2),
                class_mask: field(3),
                flags: field(4),
                req_level: field(5),
                skill_tier_id: field(6),
            });
    }
    infos
}

pub(in crate::world) fn parse_spell_durations(bytes: &[u8]) -> HashMap<u32, SpellDurationEntry> {
    const DBC_HEADER_SIZE: usize = 20;
    const SPELL_DURATION_FIELD_COUNT: usize = 4;
    if bytes.len() < DBC_HEADER_SIZE || &bytes[0..4] != b"WDBC" {
        return HashMap::new();
    }
    let record_count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let field_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let record_size = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if field_count != SPELL_DURATION_FIELD_COUNT || record_size < SPELL_DURATION_FIELD_COUNT * 4 {
        return HashMap::new();
    }
    let records_size = record_count.saturating_mul(record_size);
    if bytes.len() < DBC_HEADER_SIZE + records_size {
        return HashMap::new();
    }

    let mut durations = HashMap::with_capacity(record_count);
    for record_index in 0..record_count {
        let record_offset = DBC_HEADER_SIZE + record_index * record_size;
        let id = u32::from_le_bytes(bytes[record_offset..record_offset + 4].try_into().unwrap());
        let duration_millis = i32::from_le_bytes(
            bytes[record_offset + 4..record_offset + 8]
                .try_into()
                .unwrap(),
        );
        let duration_per_level_millis = i32::from_le_bytes(
            bytes[record_offset + 8..record_offset + 12]
                .try_into()
                .unwrap(),
        );
        let max_duration_millis = i32::from_le_bytes(
            bytes[record_offset + 12..record_offset + 16]
                .try_into()
                .unwrap(),
        );
        if id != 0 {
            durations.insert(
                id,
                SpellDurationEntry {
                    duration_millis,
                    duration_per_level_millis,
                    max_duration_millis,
                },
            );
        }
    }
    durations
}

pub(in crate::world) fn parse_mmap_header_file_name(file_name: &str) -> Option<u32> {
    let stem = file_name.strip_suffix(".mmap")?;
    (stem.len() == 3)
        .then(|| stem.parse::<u32>().ok())
        .flatten()
}

pub(in crate::world) fn parse_mmap_tile_file_name(file_name: &str) -> Option<(u32, u32, u32)> {
    let stem = file_name.strip_suffix(".mmtile")?;
    if stem.len() != 7 {
        return None;
    }
    let map_id = stem[0..3].parse::<u32>().ok()?;
    let tile_x = stem[3..5].parse::<u32>().ok()?;
    let tile_y = stem[5..7].parse::<u32>().ok()?;
    Some((map_id, tile_x, tile_y))
}

pub(in crate::world) fn parse_vmap_tree_file_name(file_name: &str) -> Option<u32> {
    let stem = file_name.strip_suffix(".vmtree")?;
    (stem.len() == 3)
        .then(|| stem.parse::<u32>().ok())
        .flatten()
}

pub(in crate::world) fn parse_vmap_tile_file_name(file_name: &str) -> Option<(u32, u32, u32)> {
    let stem = file_name.strip_suffix(".vmtile")?;
    if stem.len() != 9 || &stem[3..4] != "_" || &stem[6..7] != "_" {
        return None;
    }
    let map_id = stem[0..3].parse::<u32>().ok()?;
    let tile_y = stem[4..6].parse::<u32>().ok()?;
    let tile_x = stem[7..9].parse::<u32>().ok()?;
    Some((map_id, tile_x, tile_y))
}

pub(in crate::world) fn file_has_magic(path: &std::path::Path, magic: &[u8]) -> bool {
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut buffer = vec![0; magic.len()];
    std::io::Read::read_exact(&mut file, &mut buffer).is_ok() && buffer == magic
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dbc_bytes(field_count: usize, records: Vec<Vec<u32>>) -> Vec<u8> {
        let record_size = field_count * 4;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"WDBC");
        bytes.extend_from_slice(&(records.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(field_count as u32).to_le_bytes());
        bytes.extend_from_slice(&(record_size as u32).to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        for record in records {
            assert_eq!(record.len(), field_count);
            for field in record {
                bytes.extend_from_slice(&field.to_le_bytes());
            }
        }
        bytes.push(0);
        bytes
    }

    #[test]
    fn taxi_node_parser_uses_cmangos_dbc_fields() {
        let mut record = vec![0u32; 16];
        record[0] = 5;
        record[1] = 0;
        record[2] = 1.25f32.to_bits();
        record[3] = 2.5f32.to_bits();
        record[4] = 3.75f32.to_bits();
        record[14] = 111;
        record[15] = 222;

        let nodes = parse_taxi_nodes(&dbc_bytes(16, vec![record]));
        let node = nodes.get(&5).unwrap();

        assert_eq!(node.map_id, 0);
        assert_eq!(node.position(), WorldPosition::new(0, 1.25, 2.5, 3.75, 0.0));
        assert_eq!(node.mount_creature_id(false), 111);
        assert_eq!(node.mount_creature_id(true), 222);
    }

    #[test]
    fn taxi_path_parser_and_network_mask_follow_source_nodes() {
        let paths = parse_taxi_paths(&dbc_bytes(4, vec![vec![10, 5, 6, 123]]));
        let path = paths.get(&(5, 6)).unwrap();
        assert_eq!(path.id, 10);
        assert_eq!(path.price, 123);

        let mut nodes = HashMap::new();
        nodes.insert(
            5,
            TaxiNodeEntry {
                id: 5,
                map_id: 0,
                x: 0.0,
                y: 0.0,
                z: 0.0,
                horde_mount_creature: 1,
                alliance_mount_creature: 2,
            },
        );
        nodes.insert(
            6,
            TaxiNodeEntry {
                id: 6,
                map_id: 0,
                x: 1.0,
                y: 0.0,
                z: 0.0,
                horde_mount_creature: 1,
                alliance_mount_creature: 2,
            },
        );

        let mut mask = taxi_network_mask(&nodes, &paths);
        assert!(taxi_mask_has_node(mask, 5));
        assert!(!taxi_mask_has_node(mask, 6));
        assert!(set_taxi_mask_node(&mut mask, 33));
        assert!(taxi_mask_has_node(mask, 33));
        assert!(!set_taxi_mask_node(&mut mask, 33));
    }

    #[test]
    fn taxi_path_node_parser_groups_and_sorts_nodes_by_path() {
        let nodes = parse_taxi_path_nodes(&dbc_bytes(
            9,
            vec![
                vec![
                    2,
                    10,
                    1,
                    0,
                    4.0f32.to_bits(),
                    5.0f32.to_bits(),
                    6.0f32.to_bits(),
                    7,
                    8,
                ],
                vec![
                    1,
                    10,
                    0,
                    0,
                    1.0f32.to_bits(),
                    2.0f32.to_bits(),
                    3.0f32.to_bits(),
                    0,
                    0,
                ],
            ],
        ));

        let path_nodes = nodes.get(&10).unwrap();
        assert_eq!(path_nodes.len(), 2);
        assert_eq!(path_nodes[0].id, 1);
        assert_eq!(path_nodes[1].id, 2);
        assert_eq!(
            path_nodes[1].position(),
            WorldPosition::new(0, 4.0, 5.0, 6.0, 0.0)
        );
        assert_eq!(path_nodes[1].action_flag, 7);
        assert_eq!(path_nodes[1].delay, 8);
    }
}
