type OnlineCharacters = Arc<Mutex<HashSet<u32>>>;

#[derive(Clone)]
struct WorldRuntimeState {
    online_characters: OnlineCharacters,
    delete_options: CharacterDeleteOptions,
}

#[derive(Debug, Default)]
struct WorldSessionState {
    active_character: Option<ActiveCharacter>,
    combat_dummy_health: u32,
    active_combat_target: Option<ObjectGuid>,
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
}

#[derive(Debug, Clone)]
struct DbCreatureRuntime {
    spawn: CreatureSpawnQuery,
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

