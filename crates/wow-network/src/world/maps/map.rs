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
    last_fall_z: Option<f32>,
    last_fall_time: u32,
    environment: PlayerEnvironmentRuntime,
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
    xp: u32,
    power1: u32,
    max_power1: u32,
    last_mana_use_at: Option<Instant>,
    power2: u32,
    power4: u32,
    max_power4: u32,
    player_bytes: u32,
    player_bytes2: u32,
    stand_state: u8,
    active_spells: HashSet<u32>,
    inventory: Vec<CharacterInventoryItem>,
    quest_statuses: HashMap<u32, CharacterQuestStatus>,
    active_auras: Vec<ActiveAura>,
    spell_global_cooldowns_until: HashMap<u32, Instant>,
    spell_cooldowns_until: HashMap<u32, Instant>,
    queued_next_melee_spell: Option<QueuedNextMeleeSpell>,
    base_combat_stats: PlayerCombatStats,
    combat_stats: PlayerCombatStats,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct PlayerRuntimeSnapshot {
    position: WorldPosition,
    flags: u32,
    level: u8,
    race: u8,
    class: u8,
    xp: u32,
    health: u32,
    max_health: u32,
    power1: u32,
    max_power1: u32,
    last_mana_use_at: Option<Instant>,
    power2: u32,
    power4: u32,
    max_power4: u32,
    active_spells: HashSet<u32>,
    inventory: Vec<CharacterInventoryItem>,
    quest_statuses: HashMap<u32, CharacterQuestStatus>,
    active_auras: Vec<ActiveAura>,
    spell_global_cooldowns_until: HashMap<u32, Instant>,
    spell_cooldowns_until: HashMap<u32, Instant>,
    queued_next_melee_spell: Option<QueuedNextMeleeSpell>,
    base_combat_stats: PlayerCombatStats,
    combat_stats: PlayerCombatStats,
    active_combat_target: Option<ObjectGuid>,
    active_combat_next_swing_at: Option<Instant>,
}

#[derive(Debug)]
struct PlayerRewardRuntimeUpdate {
    level: u8,
    xp: u32,
    health: u32,
    max_health: u32,
    power1: u32,
    max_power1: u32,
    power2: u32,
    quest_statuses: HashMap<u32, CharacterQuestStatus>,
}

#[derive(Debug)]
struct PlayerHealEvent {
    healed_character_guid: u32,
    health: u32,
    direct_session_id: SessionId,
    direct_packets: Vec<OutboundWorldPacket>,
    observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug)]
#[allow(dead_code)]
struct MapRuntime {
    map_id: u32,
    instance_id: u32,
    geometry: Arc<WorldGeometry>,
    db_scripts: Arc<DbScriptRegistry>,
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
    pending_db_scripts: Vec<PendingDbScriptAction>,
    next_player_regen_tick_at: Option<Instant>,
    active_player_spell_casts: HashMap<u32, ActivePlayerSpellCast>,
    pending_spell_events: Vec<PendingSpellEvent>,
    next_spell_event_id: u64,
}

#[derive(Debug, Clone)]
struct ActivePlayerSpellCast {
    spell_id: u32,
    source: ActivePlayerSpellCastSource,
    profile: SpellCastProfile,
    targets: PendingSpellCastTargets,
    due_at: Instant,
}

#[derive(Debug, Clone)]
enum ActivePlayerSpellCastSource {
    Player,
    Item {
        item_guid: ObjectGuid,
        source_item: CharacterInventoryItem,
        spell_charges: i32,
    },
}

#[derive(Debug, Clone)]
struct PendingSpellEvent {
    event_id: u64,
    caster_character_guid: u32,
    spell_id: u32,
    targets: PendingSpellCastTargets,
    unit_target_generation: Option<(ObjectGuid, u64)>,
    due_at: Instant,
}

#[derive(Debug, Clone)]
struct PendingSpellCastTargets {
    target_mask: u16,
    unit_target: Option<ObjectGuid>,
    gameobject_target: Option<ObjectGuid>,
}

#[derive(Debug, Clone, Copy)]
struct PlayerEnvironmentRuntime {
    flags: u32,
    last_tick_at: Option<Instant>,
    last_damage_at: Option<Instant>,
    fatigue: MirrorTimerRuntime,
    breath: MirrorTimerRuntime,
    environmental: MirrorTimerRuntime,
}

impl Default for PlayerEnvironmentRuntime {
    fn default() -> Self {
        Self {
            flags: 0,
            last_tick_at: None,
            last_damage_at: None,
            fatigue: MirrorTimerRuntime::new(MIRROR_TIMER_FATIGUE, MIRROR_TIMER_FATIGUE_MAX_MILLIS),
            breath: MirrorTimerRuntime::new(MIRROR_TIMER_BREATH, MIRROR_TIMER_BREATH_MAX_MILLIS),
            environmental: MirrorTimerRuntime::new(
                MIRROR_TIMER_ENVIRONMENTAL,
                MIRROR_TIMER_ENVIRONMENTAL_MAX_MILLIS,
            ),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct MirrorTimerRuntime {
    timer_type: u32,
    active: bool,
    elapsed_millis: u32,
    pulse_millis: u32,
    duration_millis: u32,
    scale: i32,
}

impl MirrorTimerRuntime {
    const fn new(timer_type: u32, duration_millis: u32) -> Self {
        Self {
            timer_type,
            active: false,
            elapsed_millis: 0,
            pulse_millis: 0,
            duration_millis,
            scale: -1,
        }
    }
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
    attacker_rage_damage: u32,
    creature: DbCreatureRuntime,
    attacker_state_body: Option<Vec<u8>>,
    spell_non_melee_log_body: Option<Vec<u8>>,
    update_body: Vec<u8>,
    death_finalization: Option<DbCreatureDeathFinalizationEvent>,
    target_switch: Option<DbCreatureThreatTargetSwitchEvent>,
    observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug)]
struct DbCreatureAuraUpdateEvent {
    update_body: Vec<u8>,
    observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug, Clone)]
struct DbCreatureDamageRequest {
    creature_guid: ObjectGuid,
    killer: ObjectGuid,
    damage: u32,
    melee_outcome: Option<MeleeDamageOutcome>,
    spell_id: Option<u32>,
    spell_school: u8,
    suppress_attacker_state: bool,
    now: Instant,
    now_epoch_secs: u64,
    exclude_character_guid: Option<u32>,
    corpse_loot: Option<DbCreatureCorpseLootInit>,
}

#[derive(Debug)]
struct DbCreatureDeleteEvent {
    creature: DbCreatureRuntime,
    direct_packet: OutboundWorldPacket,
    observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug, Clone)]
struct DbCreatureCorpseLootInit {
    owner: CreatureLootOwner,
    allowed_players: Vec<u32>,
    current_looter: Option<u32>,
    loot_method: Option<CreatureLootMethod>,
    loot_items: Vec<DbCreatureLootRuntime>,
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
        Self::with_geometry(
            map_id,
            instance_id,
            Arc::new(WorldGeometry::default()),
            Arc::new(DbScriptRegistry::default()),
        )
    }

    fn with_geometry(
        map_id: u32,
        instance_id: u32,
        geometry: Arc<WorldGeometry>,
        db_scripts: Arc<DbScriptRegistry>,
    ) -> Self {
        Self {
            map_id,
            instance_id,
            geometry,
            db_scripts,
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
            pending_db_scripts: Vec::new(),
            next_player_regen_tick_at: None,
            active_player_spell_casts: HashMap::new(),
            pending_spell_events: Vec::new(),
            next_spell_event_id: 1,
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
