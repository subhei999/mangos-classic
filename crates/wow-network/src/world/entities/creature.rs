#[derive(Debug, Clone, Copy)]
struct CreatureCombatState {
    attacker: ObjectGuid,
    victim: ObjectGuid,
    next_swing_at: Instant,
}

#[derive(Debug, Clone)]
struct Creature {
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
    life_state: CreatureLifeState,
    corpse_expires_at: Option<Instant>,
    respawn_at: Option<Instant>,
    respawn_epoch_secs: Option<u64>,
    client_visible: bool,
    lootable: bool,
    looting: bool,
    loot_money_available: bool,
    loot_item: Option<CreatureLoot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CreatureLifeState {
    Alive,
    Corpse,
    Dead,
}

#[derive(Debug, Clone)]
struct CreatureLoot {
    item: u32,
    count: u32,
    display_id: u32,
}
