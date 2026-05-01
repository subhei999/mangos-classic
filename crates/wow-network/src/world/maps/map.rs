#[derive(Debug, Clone)]
#[allow(dead_code)]
struct PlayerRuntime {
    guid: u32,
    account_id: u32,
    session_id: SessionId,
    position: WorldPosition,
    movement_flags: u32,
    client_time: u32,
    fall_time: u32,
    cell: CellCoord,
    visible_objects: HashSet<ObjectGuid>,
    visual: PlayerVisualState,
    visible_equipment: [u32; ENUM_EQUIPMENT_SLOTS],
    flags: u32,
    level: u8,
    race: u8,
    class: u8,
    gender: u8,
    health: u32,
    max_health: u32,
    power1: u32,
    max_power1: u32,
    power2: u32,
    player_bytes: u32,
    player_bytes2: u32,
}

#[derive(Debug)]
#[allow(dead_code)]
struct MapRuntime {
    map_id: u32,
    instance_id: u32,
    grids: HashMap<GridCoord, GridRuntime>,
    loaded_creature_grids: HashSet<GridCoord>,
    players: HashMap<u32, PlayerRuntime>,
    creatures: HashMap<u64, DbCreatureRuntime>,
    active_creature_combats: HashMap<u64, CreatureCombatState>,
    creature_threats: HashMap<u64, Vec<CreatureThreatEntry>>,
    corpses: HashMap<u64, PlayerCorpseRuntime>,
}

#[derive(Debug)]
#[allow(dead_code)]
struct DbCreaturePlayerDamageEvent {
    damage: u32,
    victim_health: u32,
    combat: CreatureCombatState,
    observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug)]
struct DbCreatureDamageEvent {
    damage: u32,
    creature: DbCreatureRuntime,
    attacker_state_body: Vec<u8>,
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

impl MapRuntime {
    fn new(map_id: u32, instance_id: u32) -> Self {
        Self {
            map_id,
            instance_id,
            grids: HashMap::new(),
            loaded_creature_grids: HashSet::new(),
            players: HashMap::new(),
            creatures: HashMap::new(),
            active_creature_combats: HashMap::new(),
            creature_threats: HashMap::new(),
            corpses: HashMap::new(),
        }
    }
}

include!("map/players.rs");
include!("map/creature_snapshots.rs");
include!("map/creature_damage.rs");
include!("map/creature_lifecycle.rs");
include!("map/creature_loot.rs");
include!("map/creature_combat.rs");
include!("map/creature_motion.rs");
include!("map/spatial.rs");
