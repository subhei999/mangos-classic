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

#[derive(Debug, Clone, Copy)]
struct SpellNonMeleeDamageLogPacket {
    attacker: ObjectGuid,
    target: ObjectGuid,
    spell_id: u32,
    damage: u32,
    school: u8,
    absorb: u32,
    resist: i32,
    periodic: bool,
    blocked: u32,
    hit_info: u32,
}

fn build_spell_non_melee_damage_log_body(
    log: SpellNonMeleeDamageLogPacket,
) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(44);
    PackedGuid::write(&mut body, log.target)?;
    PackedGuid::write(&mut body, log.attacker)?;
    body.extend_from_slice(&log.spell_id.to_le_bytes());
    body.extend_from_slice(&log.damage.to_le_bytes());
    body.push(log.school);
    body.extend_from_slice(&log.absorb.to_le_bytes());
    body.extend_from_slice(&log.resist.to_le_bytes());
    body.push(log.periodic as u8);
    body.push(0); // unused
    body.extend_from_slice(&log.blocked.to_le_bytes());
    body.extend_from_slice(&log.hit_info.to_le_bytes());
    body.push(0); // debug switch disabled
    Ok(body)
}

fn build_environmental_damage_log_body(
    player: ObjectGuid,
    damage_type: u8,
    damage: u32,
    absorbed: u32,
    resisted: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(18);
    PackedGuid::write(&mut body, player)?;
    body.push(damage_type);
    body.extend_from_slice(&damage.to_le_bytes());
    body.extend_from_slice(&absorbed.to_le_bytes());
    body.extend_from_slice(&resisted.to_le_bytes());
    Ok(body)
}

fn build_spell_log_miss_body(
    caster: ObjectGuid,
    target: ObjectGuid,
    spell_id: u32,
    miss_info: u8,
) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(26);
    body.extend_from_slice(&spell_id.to_le_bytes());
    body.extend_from_slice(&caster.raw().to_le_bytes());
    body.push(0); // can be 0 or 1 in CMaNGOS
    body.extend_from_slice(&1u32.to_le_bytes());
    body.extend_from_slice(&target.raw().to_le_bytes());
    body.push(miss_info);
    Ok(body)
}

fn build_spell_heal_log_body(
    caster: ObjectGuid,
    target: ObjectGuid,
    spell_id: u32,
    heal: u32,
    critical: bool,
) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(26);
    PackedGuid::write(&mut body, target)?;
    PackedGuid::write(&mut body, caster)?;
    body.extend_from_slice(&spell_id.to_le_bytes());
    body.extend_from_slice(&heal.to_le_bytes());
    body.push(critical as u8);
    Ok(body)
}

fn build_spell_energize_log_body(
    caster: ObjectGuid,
    target: ObjectGuid,
    spell_id: u32,
    power_type: u32,
    amount: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(28);
    PackedGuid::write(&mut body, target)?;
    PackedGuid::write(&mut body, caster)?;
    body.extend_from_slice(&spell_id.to_le_bytes());
    body.extend_from_slice(&power_type.to_le_bytes());
    body.extend_from_slice(&amount.to_le_bytes());
    Ok(body)
}

fn build_spell_failure_body(caster: ObjectGuid, spell_id: u32, result: u8) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(13);
    PackedGuid::write(&mut body, caster)?;
    body.extend_from_slice(&spell_id.to_le_bytes());
    body.push(result);
    Ok(body)
}

fn build_spell_failed_other_body(caster: ObjectGuid, spell_id: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(12);
    body.extend_from_slice(&caster.raw().to_le_bytes());
    body.extend_from_slice(&spell_id.to_le_bytes());
    body
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

fn build_player_energy_update_body(player: ObjectGuid, energy: u32) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, UNIT_FIELD_POWER4, energy.min(POWER_ENERGY_DEFAULT))?;
    write_update_values(&mut block, &values)?;

    let mut body = Vec::with_capacity(5 + block.len());
    body.extend_from_slice(&1u32.to_le_bytes());
    body.push(0);
    body.extend_from_slice(&block);
    Ok(body)
}

fn build_player_combo_points_update_body(
    player: ObjectGuid,
    combo_target: ObjectGuid,
    combo_points: u8,
    player_bytes: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(
        &mut values,
        PLAYER_FIELD_COMBO_TARGET,
        combo_target.raw() as u32,
    )?;
    set_update_value(
        &mut values,
        PLAYER_FIELD_COMBO_TARGET + 1,
        (combo_target.raw() >> 32) as u32,
    )?;
    let player_bytes_with_combo =
        (player_bytes & !0x0000_FF00) | ((combo_points.min(5) as u32) << 8);
    set_update_value(&mut values, PLAYER_FIELD_BYTES, player_bytes_with_combo)?;
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
    set_unit_aura_update_values(&mut values, &[])?;
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
