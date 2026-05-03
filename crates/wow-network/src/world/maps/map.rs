#[derive(Debug, Clone)]
#[allow(dead_code)]
struct PlayerRuntime {
    guid: u32,
    account_id: u32,
    session_id: SessionId,
    selected_target: Option<ObjectGuid>,
    active_combat_target: Option<ObjectGuid>,
    active_combat_next_swing_at: Option<Instant>,
    position: WorldPosition,
    movement_flags: u32,
    client_time: u32,
    server_time: u32,
    fall_time: u32,
    jump: JumpInfo,
    cell: CellCoord,
    visible_objects: HashSet<ObjectGuid>,
    last_creature_visibility_position: Option<WorldPosition>,
    last_gameobject_visibility_position: Option<WorldPosition>,
    last_player_corpse_visibility_position: Option<WorldPosition>,
    visual: PlayerVisualState,
    visible_equipment: [u32; ENUM_EQUIPMENT_SLOTS],
    flags: u32,
    level: u8,
    race: u8,
    class: u8,
    spirit: u32,
    gender: u8,
    health: u32,
    max_health: u32,
    power1: u32,
    max_power1: u32,
    power2: u32,
    player_bytes: u32,
    player_bytes2: u32,
    active_spells: HashSet<u32>,
    inventory: Vec<CharacterInventoryItem>,
    quest_statuses: HashMap<u32, CharacterQuestStatus>,
    active_auras: Vec<ActiveAura>,
    base_combat_stats: PlayerCombatStats,
    combat_stats: PlayerCombatStats,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct PlayerRuntimeSnapshot {
    position: WorldPosition,
    health: u32,
    power1: u32,
    power2: u32,
    active_spells: HashSet<u32>,
    inventory: Vec<CharacterInventoryItem>,
    quest_statuses: HashMap<u32, CharacterQuestStatus>,
    active_auras: Vec<ActiveAura>,
    base_combat_stats: PlayerCombatStats,
    combat_stats: PlayerCombatStats,
    active_combat_target: Option<ObjectGuid>,
    active_combat_next_swing_at: Option<Instant>,
}

#[derive(Debug)]
#[allow(dead_code)]
struct MapRuntime {
    map_id: u32,
    instance_id: u32,
    geometry: Arc<WorldGeometry>,
    grids: HashMap<GridCoord, GridRuntime>,
    loaded_creature_grids: HashSet<GridCoord>,
    loaded_gameobject_grids: HashSet<GridCoord>,
    loaded_player_corpse_grids: HashSet<GridCoord>,
    players: HashMap<u32, PlayerRuntime>,
    creatures: HashMap<u64, DbCreatureRuntime>,
    creature_looting_by_character: HashMap<u32, u64>,
    gameobjects: HashMap<u64, DbGameObjectRuntime>,
    gameobject_loots: HashMap<u64, DbGameObjectLootState>,
    gameobject_looting_by_character: HashMap<u32, u64>,
    active_creature_combats: HashMap<u64, CreatureCombatState>,
    creature_combat_leash: HashMap<u64, CreatureCombatLeashState>,
    creature_threats: HashMap<u64, Vec<CreatureThreatEntry>>,
    corpses: HashMap<u64, PlayerCorpseRuntime>,
    next_idle_motion_tick_at: Option<Instant>,
    next_idle_motion_start_check_at: Option<Instant>,
    next_player_regen_tick_at: Option<Instant>,
}

#[derive(Debug)]
#[allow(dead_code)]
struct DbCreaturePlayerDamageEvent {
    damage: u32,
    victim_health: u32,
    combat: CreatureCombatState,
    observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug, Clone, Copy)]
struct CreatureCombatLeashState {
    refresh_position: WorldPosition,
    combat_start_position: WorldPosition,
    expires_at: Instant,
    template_leash_yards: f32,
}

#[derive(Debug)]
struct DbCreatureDamageEvent {
    #[allow(dead_code)]
    damage: u32,
    creature: DbCreatureRuntime,
    attacker_state_body: Option<Vec<u8>>,
    spell_non_melee_log_body: Option<Vec<u8>>,
    update_body: Vec<u8>,
    death_finalization: Option<DbCreatureDeathFinalizationEvent>,
    target_switch: Option<DbCreatureThreatTargetSwitchEvent>,
    observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug, Clone, Copy)]
struct DbCreatureDamageRequest {
    creature_guid: ObjectGuid,
    killer: ObjectGuid,
    damage: u32,
    melee_outcome: Option<MeleeDamageOutcome>,
    spell_id: Option<u32>,
    suppress_attacker_state: bool,
    now: Instant,
    now_epoch_secs: u64,
    exclude_character_guid: Option<u32>,
}

#[derive(Debug)]
struct DbCreatureDeathFinalizationEvent {
    killed: ObjectGuid,
    respawn_epoch_secs: Option<u64>,
    motion_stop_packet: Option<OutboundWorldPacket>,
    attack_stop_packet: OutboundWorldPacket,
    combat_flag_packet: OutboundWorldPacket,
    observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug)]
struct DbCreatureThreatTargetSwitchEvent {
    attacker: ObjectGuid,
    old_victim: ObjectGuid,
    new_victim: ObjectGuid,
    combat: CreatureCombatState,
    direct_packets: Vec<OutboundWorldPacket>,
    observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug)]
struct DbCreatureLifecycleEvent {
    #[allow(dead_code)]
    creature: DbCreatureRuntime,
    direct_packets: Vec<OutboundWorldPacket>,
    observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
    clear_respawn_guid: Option<u32>,
}

#[derive(Debug)]
struct DbCreatureLootReleaseEvent {
    creature: DbCreatureRuntime,
    direct_packet: OutboundWorldPacket,
    observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug)]
struct PlayerAuraUpdateEvent {
    direct_packets: Vec<OutboundWorldPacket>,
    observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug, Default)]
struct MapDbCreatureVisibilityStage {
    nearby_creatures: Vec<DbCreatureRuntime>,
    create_guids: Vec<ObjectGuid>,
    destroy_guids: Vec<ObjectGuid>,
}

#[derive(Debug, Default)]
struct MapDbGameObjectVisibilityStage {
    nearby_gameobjects: Vec<DbGameObjectRuntime>,
    create_guids: Vec<ObjectGuid>,
    destroy_guids: Vec<ObjectGuid>,
}

#[derive(Debug, Default)]
struct MapPlayerCorpseVisibilityStage {
    nearby_corpses: Vec<PlayerCorpseRuntime>,
    create_guids: Vec<ObjectGuid>,
    destroy_guids: Vec<ObjectGuid>,
}

#[derive(Debug, Default)]
struct DbGameObjectLootState {
    open_characters: HashSet<u32>,
    loot_items: Vec<DbCreatureLootRuntime>,
}

impl MapRuntime {
    #[cfg(test)]
    fn new(map_id: u32, instance_id: u32) -> Self {
        Self::with_geometry(map_id, instance_id, Arc::new(WorldGeometry::default()))
    }

    fn with_geometry(map_id: u32, instance_id: u32, geometry: Arc<WorldGeometry>) -> Self {
        Self {
            map_id,
            instance_id,
            geometry,
            grids: HashMap::new(),
            loaded_creature_grids: HashSet::new(),
            loaded_gameobject_grids: HashSet::new(),
            loaded_player_corpse_grids: HashSet::new(),
            players: HashMap::new(),
            creatures: HashMap::new(),
            creature_looting_by_character: HashMap::new(),
            gameobjects: HashMap::new(),
            gameobject_loots: HashMap::new(),
            gameobject_looting_by_character: HashMap::new(),
            active_creature_combats: HashMap::new(),
            creature_combat_leash: HashMap::new(),
            creature_threats: HashMap::new(),
            corpses: HashMap::new(),
            next_idle_motion_tick_at: None,
            next_idle_motion_start_check_at: None,
            next_player_regen_tick_at: None,
        }
    }

    fn observability_snapshot(&self) -> crate::observability::MapRuntimeSnapshot {
        crate::observability::MapRuntimeSnapshot {
            map_id: self.map_id,
            instance_id: self.instance_id,
            active_players: self.players.len() as u64,
            active_creatures: self.creatures.len() as u64,
            active_gameobjects: self.gameobjects.len() as u64,
            loaded_grids: self.grids.len() as u64,
            loaded_creature_grids: self.loaded_creature_grids.len() as u64,
            loaded_gameobject_grids: self.loaded_gameobject_grids.len() as u64,
            loaded_player_corpse_grids: self.loaded_player_corpse_grids.len() as u64,
            active_creature_combats: self.active_creature_combats.len() as u64,
            corpses: self.corpses.len() as u64,
        }
    }
}

include!("map/players.rs");
include!("map/player_corpses.rs");
include!("map/creature_snapshots.rs");
include!("map/gameobject_snapshots.rs");
include!("map/creature_damage.rs");
include!("map/creature_lifecycle.rs");
include!("map/creature_loot.rs");
include!("map/gameobject_loot.rs");
include!("map/creature_combat.rs");
include!("map/creature_motion.rs");
include!("map/spatial.rs");
