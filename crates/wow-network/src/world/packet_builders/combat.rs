// CMaNGOS reference: src/game/Entities/Unit.cpp combat packet builders.

fn build_attack_start_body(attacker: ObjectGuid, victim: ObjectGuid) -> Vec<u8> {
    let mut body = Vec::with_capacity(16);
    body.extend_from_slice(&attacker.raw().to_le_bytes());
    body.extend_from_slice(&victim.raw().to_le_bytes());
    body
}

fn build_attack_stop_body(
    attacker: ObjectGuid,
    victim: ObjectGuid,
    dead: bool,
) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(20);
    PackedGuid::write(&mut body, attacker)?;
    PackedGuid::write(&mut body, victim)?;
    body.extend_from_slice(&(dead as u32).to_le_bytes());
    Ok(body)
}

fn build_attacker_state_update_body(
    attacker: ObjectGuid,
    victim: ObjectGuid,
    damage: u32,
) -> anyhow::Result<Vec<u8>> {
    build_attacker_state_update_body_for_outcome(
        attacker,
        victim,
        MeleeDamageOutcome::normal_hit(damage),
        0,
    )
}

fn build_attacker_state_update_body_with_spell_id(
    attacker: ObjectGuid,
    victim: ObjectGuid,
    damage: u32,
    spell_id: u32,
) -> anyhow::Result<Vec<u8>> {
    build_attacker_state_update_body_for_outcome(
        attacker,
        victim,
        MeleeDamageOutcome::normal_hit(damage),
        spell_id,
    )
}

fn build_attacker_state_update_body_for_outcome(
    attacker: ObjectGuid,
    victim: ObjectGuid,
    outcome: MeleeDamageOutcome,
    spell_id: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(42);
    body.extend_from_slice(&outcome.hit_info.to_le_bytes());
    PackedGuid::write(&mut body, attacker)?;
    PackedGuid::write(&mut body, victim)?;
    body.extend_from_slice(&outcome.total_damage.to_le_bytes());
    body.push(1);
    body.extend_from_slice(&0u32.to_le_bytes()); // normal school
    body.extend_from_slice(&(outcome.school_damage as f32).to_le_bytes());
    body.extend_from_slice(&outcome.school_damage.to_le_bytes());
    body.extend_from_slice(&outcome.absorbed.to_le_bytes());
    body.extend_from_slice(&outcome.resisted.to_le_bytes());
    body.extend_from_slice(&outcome.victim_state.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes()); // unknown
    body.extend_from_slice(&spell_id.to_le_bytes());
    body.extend_from_slice(&outcome.blocked.to_le_bytes());
    Ok(body)
}

fn build_combat_dummy_state_update_body(
    health: u32,
    dynamic_flags: u32,
) -> anyhow::Result<Vec<u8>> {
    let guid = rust_combat_dummy_guid();
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, guid)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, UNIT_FIELD_HEALTH, health)?;
    set_update_value(&mut values, UNIT_DYNAMIC_FLAGS, dynamic_flags)?;
    write_update_values(&mut block, &values)?;

    let mut body = Vec::with_capacity(5 + block.len());
    body.extend_from_slice(&1u32.to_le_bytes());
    body.push(0);
    body.extend_from_slice(&block);
    Ok(body)
}

fn build_player_rage_update_body(player: ObjectGuid, rage: u32) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, UNIT_FIELD_POWER2, rage.min(POWER_RAGE_DEFAULT))?;
    write_update_values(&mut block, &values)?;

    let mut body = Vec::with_capacity(5 + block.len());
    body.extend_from_slice(&1u32.to_le_bytes());
    body.push(0);
    body.extend_from_slice(&block);
    Ok(body)
}

fn build_db_creature_state_update_body(
    guid: ObjectGuid,
    health: u32,
    dynamic_flags: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, guid)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, UNIT_FIELD_HEALTH, health)?;
    set_update_value(&mut values, UNIT_DYNAMIC_FLAGS, dynamic_flags)?;
    write_update_values(&mut block, &values)?;

    Ok(build_update_object_body(&[block]))
}

fn build_db_creature_death_update_body(
    guid: ObjectGuid,
    dynamic_flags: u32,
    unit_flags: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, guid)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, UNIT_FIELD_TARGET, 0)?;
    set_update_value(&mut values, UNIT_FIELD_TARGET + 1, 0)?;
    set_update_value(&mut values, UNIT_FIELD_HEALTH, 0)?;
    set_update_value(&mut values, UNIT_FIELD_FLAGS, unit_flags)?;
    set_update_value(&mut values, UNIT_DYNAMIC_FLAGS, dynamic_flags)?;
    set_update_value(&mut values, UNIT_NPC_FLAGS, 0)?;
    write_update_values(&mut block, &values)?;

    Ok(build_update_object_body(&[block]))
}

fn build_unit_flags_update_body(guid: ObjectGuid, flags: u32) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, guid)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, UNIT_FIELD_FLAGS, flags)?;
    write_update_values(&mut block, &values)?;

    Ok(build_update_object_body(&[block]))
}

fn build_player_mana_update_body(player: ObjectGuid, mana: u32) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, UNIT_FIELD_POWER1, mana)?;
    write_update_values(&mut block, &values)?;

    let mut body = Vec::with_capacity(5 + block.len());
    body.extend_from_slice(&1u32.to_le_bytes());
    body.push(0);
    body.extend_from_slice(&block);
    Ok(body)
}

fn build_player_health_update_body(player: ObjectGuid, health: u32) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, UNIT_FIELD_HEALTH, health)?;
    write_update_values(&mut block, &values)?;

    let mut body = Vec::with_capacity(5 + block.len());
    body.extend_from_slice(&1u32.to_le_bytes());
    body.push(0);
    body.extend_from_slice(&block);
    Ok(body)
}

#[cfg(test)]
fn retaliation_damage_for_db_creature(session: &mut WorldSessionState, target: ObjectGuid) -> u32 {
    let Some(creature) = session.db_creatures.get(&target.raw()) else {
        return 0;
    };
    let retaliation_damage = creature.hit_damage().max(1);
    session.player_health = session.player_health.saturating_sub(retaliation_damage);
    retaliation_damage
}
