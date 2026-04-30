#[derive(Debug, Clone, PartialEq)]
struct Corpse {
    guid: ObjectGuid,
    owner: ObjectGuid,
    position: WorldPosition,
    corpse_type: u8,
    race: u8,
    class: u8,
    gender: u8,
    player_bytes: u32,
    player_bytes2: u32,
    equipment_cache: Option<String>,
    guildid: Option<u32>,
    player_flags: u32,
}
