include!("entities/player.rs");
include!("entities/creature.rs");
include!("entities/corpse.rs");
include!("motion/motion_master.rs");
include!("maps/world_data.rs");
include!("maps/navigation.rs");

type OnlineCharacters = Arc<Mutex<HashSet<u32>>>;
type PlayerCorpses = Arc<Mutex<HashMap<u32, PlayerCorpseRuntime>>>;

type ActiveCharacter = Player;
type DbCreatureRuntime = Creature;
type DbCreatureLifeState = CreatureLifeState;
type DbCreatureLootRuntime = CreatureLoot;
type PlayerCorpseRuntime = Corpse;

#[derive(Clone)]
struct WorldRuntimeState {
    online_characters: OnlineCharacters,
    player_corpses: PlayerCorpses,
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
    player_death_state: PlayerDeathState,
    player_corpse: Option<PlayerCorpseRuntime>,
    visible_player_corpses: HashMap<u64, PlayerCorpseRuntime>,
    player_visual: Option<PlayerVisualState>,
    player_flags: u32,
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
    last_player_corpse_visibility_position: Option<WorldPosition>,
    db_creature_navigation: DbCreatureNavigationGuardrail,
}
