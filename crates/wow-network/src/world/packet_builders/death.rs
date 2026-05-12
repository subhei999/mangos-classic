// CMaNGOS reference: src/game/Handlers/CharacterHandler.cpp death/corpse packet builders.

fn build_player_death_update_body(
    player: ObjectGuid,

    health: u32,

    player_flags: u32,

    field_bytes: u32,

    unit_flags: u32,

    class: u8,

    stand_state: u8,
) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();

    block.push(UPDATE_TYPE_VALUES);

    PackedGuid::write(&mut block, player)?;

    let mut values = vec![None; PLAYER_END_FIELDS];

    set_update_value(&mut values, UNIT_FIELD_HEALTH, health)?;

    set_update_value(&mut values, UNIT_FIELD_FLAGS, unit_flags)?;

    set_update_value(
        &mut values,
        UNIT_FIELD_BYTES_1,
        unit_bytes_1_for_class(class) | u32::from(stand_state),
    )?;

    set_update_value(&mut values, PLAYER_FLAGS_FIELD, player_flags)?;

    set_update_value(&mut values, PLAYER_FIELD_BYTES, field_bytes)?;

    set_player_ghost_aura_update_values(&mut values, player_flags & PLAYER_FLAGS_GHOST != 0, 1)?;

    write_update_values(&mut block, &values)?;

    Ok(build_update_object_body(&[block]))
}

fn build_force_move_root_body(player: ObjectGuid, counter: u32) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(12);
    PackedGuid::write(&mut body, player)?;
    body.extend_from_slice(&counter.to_le_bytes());
    Ok(body)
}

fn set_player_ghost_aura_update_values(
    values: &mut [Option<u32>],

    ghost: bool,

    level: u8,
) -> anyhow::Result<()> {
    set_update_value(
        values,
        UNIT_FIELD_AURA,
        if ghost { GHOST_SPELL_ID } else { 0 },
    )?;

    set_update_value(
        values,
        UNIT_FIELD_AURAFLAGS,
        if ghost { GHOST_AURA_FLAGS } else { 0 },
    )?;

    set_update_value(
        values,
        UNIT_FIELD_AURALEVELS,
        if ghost { level.max(1) as u32 } else { 0 },
    )?;

    set_update_value(values, UNIT_FIELD_AURAAPPLICATIONS, 0)
}

fn build_corpse_reclaim_delay_body(delay_millis: u32) -> Vec<u8> {
    delay_millis.to_le_bytes().to_vec()
}

fn build_corpse_query_body(corpse_position: Option<WorldPosition>) -> Vec<u8> {
    let Some(corpse_position) = corpse_position else {
        return vec![0];
    };

    let mut body = Vec::with_capacity(21);

    body.push(1);

    body.extend_from_slice(&(corpse_position.map_id as i32).to_le_bytes());

    body.extend_from_slice(&corpse_position.x.to_le_bytes());

    body.extend_from_slice(&corpse_position.y.to_le_bytes());

    body.extend_from_slice(&corpse_position.z.to_le_bytes());

    body.extend_from_slice(&corpse_position.map_id.to_le_bytes());

    body
}
