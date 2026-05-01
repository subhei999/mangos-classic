// CMaNGOS reference: src/game/Entities/Player.cpp progression packet builders.

fn build_log_xp_gain_body(source: Option<ObjectGuid>, given_xp: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(21);

    body.extend_from_slice(&source.map_or(0, |guid| guid.raw()).to_le_bytes());

    body.extend_from_slice(&given_xp.to_le_bytes());

    body.push(u8::from(source.is_none()));

    if source.is_some() {
        body.extend_from_slice(&given_xp.to_le_bytes());

        body.extend_from_slice(&1.0f32.to_le_bytes());
    }

    body
}

fn build_levelup_info_body(
    new_level: u8,

    previous_stats: &PlayerWorldStats,

    new_stats: &PlayerWorldStats,
) -> Vec<u8> {
    let mut body = Vec::with_capacity(48);

    body.extend_from_slice(&(new_level as u32).to_le_bytes());

    body.extend_from_slice(
        &(new_stats.base_health as i32 - previous_stats.base_health as i32).to_le_bytes(),
    );

    body.extend_from_slice(
        &(new_stats.base_mana as i32 - previous_stats.base_mana as i32).to_le_bytes(),
    );

    for _ in 0..4 {
        body.extend_from_slice(&0u32.to_le_bytes());
    }

    for index in 0..MAX_STATS {
        body.extend_from_slice(
            &(new_stats.stats[index] as i32 - previous_stats.stats[index] as i32).to_le_bytes(),
        );
    }

    body
}

#[derive(Debug, Clone, Copy)]

struct PlayerProgressionUpdate<'a> {
    character_guid: u32,

    level: u8,

    xp: u32,

    health: u32,

    power1: u32,

    power2: u32,

    power3: u32,

    power4: u32,

    power5: u32,

    world_stats: &'a PlayerWorldStats,
}

fn build_player_progression_update_body(
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
