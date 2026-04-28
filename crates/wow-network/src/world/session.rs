type OnlineCharacters = Arc<Mutex<HashSet<u32>>>;

#[derive(Clone)]
struct WorldRuntimeState {
    online_characters: OnlineCharacters,
    delete_options: CharacterDeleteOptions,
    world_data_files: Arc<WorldDataFiles>,
}

#[derive(Debug, Default)]
struct WorldSessionState {
    active_character: Option<ActiveCharacter>,
    combat_dummy_health: u32,
    active_combat_target: Option<ObjectGuid>,
    active_combat_next_swing_at: Option<Instant>,
    active_creature_combats: HashMap<u64, CreatureCombatState>,
    player_in_combat: bool,
    combat_dummy_lootable: bool,
    combat_dummy_looting: bool,
    combat_dummy_loot_money_available: bool,
    combat_dummy_loot_item_available: bool,
    db_creatures: HashMap<u64, DbCreatureRuntime>,
    player_health: u32,
    player_rage: u32,
    player_mana: u32,
    active_spells: HashSet<u32>,
    inventory: Vec<CharacterInventoryItem>,
    quest_statuses: HashMap<u32, CharacterQuestStatus>,
    last_creature_visibility_position: Option<WorldPosition>,
    db_creature_navigation: DbCreatureNavigationGuardrail,
}

#[derive(Debug, Clone, Copy)]
struct CreatureCombatState {
    attacker: ObjectGuid,
    victim: ObjectGuid,
    next_swing_at: Instant,
}

#[derive(Debug, Clone, Default)]
enum CreatureMotionState {
    #[default]
    Idle,
    Random(CreatureRandomMotion),
    Waypoint(CreatureWaypointMotion),
    Chase(CreatureChaseMotion),
    ReturnHome(CreatureReturnHomeMotion),
}

#[derive(Debug, Clone)]
struct CreatureRandomMotion {
    start: WorldPosition,
    destination: WorldPosition,
    path: Vec<WorldPosition>,
    started_at: Instant,
    duration: Duration,
}

#[derive(Debug, Clone)]
struct CreatureWaypointMotion {
    node_index: usize,
    start: WorldPosition,
    destination: WorldPosition,
    path: Vec<WorldPosition>,
    started_at: Instant,
    duration: Duration,
}

#[derive(Debug, Clone)]
struct CreatureChaseMotion {
    target: ObjectGuid,
    start: WorldPosition,
    destination: WorldPosition,
    path: Vec<WorldPosition>,
    started_at: Instant,
    duration: Duration,
    recheck_at: Instant,
}

#[derive(Debug, Clone)]
struct CreatureReturnHomeMotion {
    start: WorldPosition,
    destination: WorldPosition,
    path: Vec<WorldPosition>,
    started_at: Instant,
    duration: Duration,
}

#[derive(Debug, Clone)]
struct DbCreatureNavigationGuardrail {
    line_of_sight_clear: bool,
    path_available: bool,
    world_data_files: Arc<WorldDataFiles>,
}

impl Default for DbCreatureNavigationGuardrail {
    fn default() -> Self {
        Self {
            line_of_sight_clear: true,
            path_available: true,
            world_data_files: Arc::new(WorldDataFiles::fallback()),
        }
    }
}

#[derive(Debug, Clone)]
struct WorldDataFiles {
    data_dir: std::path::PathBuf,
    data_dir_for_native: Option<std::ffi::CString>,
    maps_available: bool,
    vmaps_available: bool,
    mmap_headers: HashSet<u32>,
    mmap_tiles: HashSet<(u32, u32, u32)>,
}

impl WorldDataFiles {
    fn fallback() -> Self {
        Self {
            data_dir: std::path::PathBuf::new(),
            data_dir_for_native: None,
            maps_available: false,
            vmaps_available: false,
            mmap_headers: HashSet::new(),
            mmap_tiles: HashSet::new(),
        }
    }

    fn inspect(data_dir: impl Into<std::path::PathBuf>) -> Self {
        let data_dir = data_dir.into();
        let maps_available = data_dir.join("maps").is_dir();
        let vmaps_available = data_dir.join("vmaps").is_dir();
        let mut mmap_headers = HashSet::new();
        let mut mmap_tiles = HashSet::new();
        let mmaps_dir = data_dir.join("mmaps");

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

        Self {
            data_dir_for_native: data_dir
                .to_str()
                .and_then(|path| std::ffi::CString::new(path).ok()),
            data_dir,
            maps_available,
            vmaps_available,
            mmap_headers,
            mmap_tiles,
        }
    }

    fn has_mmap_support_for_map(&self, map_id: u32) -> bool {
        self.mmap_headers.contains(&map_id)
    }

    fn has_mmap_tile(&self, map_id: u32, tile_x: u32, tile_y: u32) -> bool {
        self.mmap_tiles.contains(&(map_id, tile_x, tile_y))
    }
}

fn parse_mmap_header_file_name(file_name: &str) -> Option<u32> {
    let stem = file_name.strip_suffix(".mmap")?;
    (stem.len() == 3)
        .then(|| stem.parse::<u32>().ok())
        .flatten()
}

fn parse_mmap_tile_file_name(file_name: &str) -> Option<(u32, u32, u32)> {
    let stem = file_name.strip_suffix(".mmtile")?;
    if stem.len() != 7 {
        return None;
    }
    let map_id = stem[0..3].parse::<u32>().ok()?;
    let tile_x = stem[3..5].parse::<u32>().ok()?;
    let tile_y = stem[5..7].parse::<u32>().ok()?;
    Some((map_id, tile_x, tile_y))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DbCreatureNavigationResult {
    Clear,
    MapMismatch,
    InvalidCoordinate,
    LineOfSightBlocked,
    PathUnavailable,
}

impl DbCreatureNavigationResult {
    fn is_clear(self) -> bool {
        self == Self::Clear
    }
}

#[derive(Debug, Clone)]
struct DbCreatureRuntime {
    spawn: CreatureSpawnQuery,
    home_position: WorldPosition,
    current_position: WorldPosition,
    motion: CreatureMotionState,
    next_random_move_at: Option<Instant>,
    next_waypoint_move_at: Option<Instant>,
    waypoint_next_index: usize,
    waypoint_forward: bool,
    already_called_assistance: bool,
    next_spline_id: u32,
    health: u32,
    lootable: bool,
    looting: bool,
    loot_money_available: bool,
    loot_item: Option<DbCreatureLootRuntime>,
}

#[derive(Debug, Clone)]
struct DbCreatureLootRuntime {
    item: u32,
    count: u32,
    display_id: u32,
}

#[derive(Debug)]
struct ActiveCharacter {
    guid: u32,
    name: String,
    race: u8,
    class: u8,
    level: u8,
    xp: u32,
    position: WorldPosition,
    movement_flags: u32,
    client_time: u32,
    fall_time: u32,
}

