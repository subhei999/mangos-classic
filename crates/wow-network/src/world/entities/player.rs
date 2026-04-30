#[derive(Debug)]
struct Player {
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum PlayerDeathState {
    #[default]
    Alive,
    Corpse,
    Ghost,
}

#[derive(Debug, Clone, PartialEq)]
struct PlayerVisualState {
    gender: u8,
    player_bytes: u32,
    player_bytes2: u32,
    equipment_cache: Option<String>,
    guildid: Option<u32>,
}
