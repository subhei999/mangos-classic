use super::*;

#[derive(Debug, Clone)]
pub(in crate::world) struct WorldDataFiles {
    pub(in crate::world) data_dir: std::path::PathBuf,
    pub(in crate::world) data_dir_for_native: Option<std::ffi::CString>,
    pub(in crate::world) maps_available: bool,
    pub(in crate::world) vmaps_available: bool,
    pub(in crate::world) creature_display_scales: HashMap<u32, f32>,
    pub(in crate::world) spell_cast_times: HashMap<u32, SpellCastTimeEntry>,
    pub(in crate::world) spell_durations: HashMap<u32, SpellDurationEntry>,
    pub(in crate::world) spell_radii: HashMap<u32, SpellRadiusEntry>,
    pub(in crate::world) spell_ranges: HashMap<u32, SpellRangeEntry>,
    pub(in crate::world) faction_templates: FactionTemplateStore,
    pub(in crate::world) item_random_properties: HashMap<u32, ItemRandomPropertyEntry>,
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
            creature_display_scales: HashMap::new(),
            spell_cast_times: HashMap::new(),
            spell_durations: HashMap::new(),
            spell_radii: HashMap::new(),
            spell_ranges: HashMap::new(),
            faction_templates: FactionTemplateStore::fallback_bridge(),
            item_random_properties: HashMap::new(),
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
        let creature_display_scales = load_creature_display_info_scales(
            &data_dir.join("dbc").join("CreatureDisplayInfo.dbc"),
        );
        let spell_cast_times =
            load_spell_cast_times(&data_dir.join("dbc").join("SpellCastTimes.dbc"));
        let spell_durations = load_spell_durations(&data_dir.join("dbc").join("SpellDuration.dbc"));
        let spell_radii = load_spell_radii(&data_dir.join("dbc").join("SpellRadius.dbc"));
        let spell_ranges = load_spell_ranges(&data_dir.join("dbc").join("SpellRange.dbc"));
        let faction_templates =
            load_faction_templates(&data_dir.join("dbc").join("FactionTemplate.dbc"));
        let item_random_properties =
            load_item_random_properties(&data_dir.join("dbc").join("ItemRandomProperties.dbc"));
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
            creature_display_scales,
            spell_cast_times,
            spell_durations,
            spell_radii,
            spell_ranges,
            faction_templates,
            item_random_properties,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct SpellCastTimeEntry {
    pub(in crate::world) cast_time_millis: i32,
    pub(in crate::world) cast_time_per_level_millis: i32,
    pub(in crate::world) min_cast_time_millis: i32,
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
pub(in crate::world) struct ItemRandomPropertyEntry {
    pub(in crate::world) id: u32,
    pub(in crate::world) enchant_ids: [u32; 3],
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
                RUST_GUIDE_FACTION_TEMPLATE,
                RUST_GUIDE_FACTION_TEMPLATE,
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
