use super::*;
use wow_proto::{
    ServerWorldPacket, SmsgExplorationExperienceResponse, SmsgLevelupInfoResponse,
    SmsgLogXpGainResponse,
};

// CMaNGOS reference: src/game/Entities/Player.cpp progression packet builders.

pub(in crate::world) fn build_log_xp_gain_body(
    source: Option<ObjectGuid>,
    base_xp: u32,
    rested_bonus_xp: u32,
) -> Vec<u8> {
    SmsgLogXpGainResponse {
        source,
        given_xp: base_xp.saturating_add(rested_bonus_xp),
        base_xp,
    }
    .body()
}

pub(in crate::world) fn build_exploration_experience_body(area: u32, experience: u32) -> Vec<u8> {
    SmsgExplorationExperienceResponse { area, experience }.body()
}

pub(in crate::world) fn build_levelup_info_body(
    new_level: u8,

    previous_stats: &PlayerWorldStats,

    new_stats: &PlayerWorldStats,
) -> Vec<u8> {
    SmsgLevelupInfoResponse {
        new_level,
        health_delta: new_stats.base_health as i32 - previous_stats.base_health as i32,
        mana_delta: new_stats.base_mana as i32 - previous_stats.base_mana as i32,
        power_deltas: [0; 4],
        stat_deltas: (0..MAX_STATS)
            .map(|index| new_stats.stats[index] as i32 - previous_stats.stats[index] as i32)
            .collect(),
    }
    .body()
}

#[derive(Debug, Clone, Copy)]

pub(in crate::world) struct PlayerProgressionUpdate<'a> {
    pub(in crate::world) character_guid: u32,

    pub(in crate::world) level: u8,

    pub(in crate::world) xp: u32,

    pub(in crate::world) health: u32,

    pub(in crate::world) power1: u32,

    pub(in crate::world) power2: u32,

    pub(in crate::world) power3: u32,

    pub(in crate::world) power4: u32,

    pub(in crate::world) power5: u32,

    pub(in crate::world) world_stats: &'a PlayerWorldStats,
}

pub(in crate::world) fn build_player_progression_update_body(
    update: PlayerProgressionUpdate<'_>,
) -> anyhow::Result<Vec<u8>> {
    let PlayerProgressionUpdate {
        character_guid,

        level,

        xp,

        health,

        power1,

        power2,

        power3,

        power4,

        power5,

        world_stats,
    } = update;

    let player = ObjectGuid::new(HighGuid::Player, 0, character_guid);

    let mut block = Vec::new();

    block.push(UPDATE_TYPE_VALUES);

    PackedGuid::write(&mut block, player)?;

    let mut values = vec![None; PLAYER_END_FIELDS];

    let max_health = world_stats.max_health().max(1);

    let max_mana = world_stats.max_mana();

    set_update_value(
        &mut values,
        UNIT_FIELD_HEALTH,
        health.max(1).min(max_health),
    )?;

    set_update_value(&mut values, UNIT_FIELD_POWER1, power1.min(max_mana))?;

    set_update_value(
        &mut values,
        UNIT_FIELD_POWER2,
        power2.min(POWER_RAGE_DEFAULT),
    )?;

    set_update_value(&mut values, UNIT_FIELD_POWER3, power3)?;

    set_update_value(
        &mut values,
        UNIT_FIELD_POWER4,
        power4.min(POWER_ENERGY_DEFAULT),
    )?;

    set_update_value(&mut values, UNIT_FIELD_POWER5, power5)?;

    set_update_value(&mut values, UNIT_FIELD_MAXHEALTH, max_health)?;

    set_update_value(&mut values, UNIT_FIELD_MAXPOWER1, max_mana)?;

    set_update_value(
        &mut values,
        UNIT_FIELD_MAXPOWER2,
        if power2 > 0 { POWER_RAGE_DEFAULT } else { 0 },
    )?;

    set_update_value(&mut values, UNIT_FIELD_MAXPOWER3, 0)?;

    set_update_value(
        &mut values,
        UNIT_FIELD_MAXPOWER4,
        if power4 > 0 { POWER_ENERGY_DEFAULT } else { 0 },
    )?;

    set_update_value(&mut values, UNIT_FIELD_MAXPOWER5, 0)?;

    set_update_value(&mut values, UNIT_FIELD_LEVEL, level as u32)?;

    set_update_value(&mut values, UNIT_FIELD_BASE_MANA, max_mana)?;

    set_update_value(&mut values, UNIT_FIELD_BASE_HEALTH, max_health)?;

    set_player_stat_update_values(&mut values, world_stats)?;

    set_update_value(&mut values, PLAYER_XP, xp)?;

    set_update_value(&mut values, PLAYER_NEXT_LEVEL_XP, world_stats.next_level_xp)?;

    write_update_values(&mut block, &values)?;

    Ok(build_update_object_body(&[block]))
}

pub(in crate::world) fn build_player_rest_update_body(
    character_guid: u32,
    player_bytes2: u32,
    rest_bonus: f32,
) -> anyhow::Result<Vec<u8>> {
    let player = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player)?;

    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, PLAYER_BYTES_2, player_bytes2)?;
    set_update_value(
        &mut values,
        PLAYER_REST_STATE_EXPERIENCE,
        rest_bonus.max(0.0).min(u32::MAX as f32) as u32,
    )?;
    write_update_values(&mut block, &values)?;

    Ok(build_update_object_body(&[block]))
}
