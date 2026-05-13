use super::*;
use wow_proto::{
    ServerWorldPacket, SmsgAttackStartResponse, SmsgAttackStopResponse,
    SmsgAttackerStateUpdateResponse, SmsgEnvironmentalDamageLogResponse, SmsgSpellDelayedResponse,
    SmsgSpellEnergizeLogResponse, SmsgSpellFailedOtherResponse, SmsgSpellFailureResponse,
    SmsgSpellHealLogResponse, SmsgSpellLogMissResponse, SmsgSpellNonMeleeDamageLogResponse,
};

// CMaNGOS reference: src/game/Entities/Unit.cpp combat packet builders.

pub(in crate::world) fn build_attack_start_body(
    attacker: ObjectGuid,
    victim: ObjectGuid,
) -> Vec<u8> {
    SmsgAttackStartResponse { attacker, victim }.body()
}

pub(in crate::world) fn build_attack_stop_body(
    attacker: ObjectGuid,
    victim: ObjectGuid,
    attacker_dead: bool,
) -> anyhow::Result<Vec<u8>> {
    Ok(SmsgAttackStopResponse {
        attacker,
        victim,
        attacker_dead,
    }
    .body())
}

pub(in crate::world) fn build_attacker_state_update_body(
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

pub(in crate::world) fn build_attacker_state_update_body_with_spell_id(
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

pub(in crate::world) fn build_attacker_state_update_body_for_outcome(
    attacker: ObjectGuid,
    victim: ObjectGuid,
    outcome: MeleeDamageOutcome,
    spell_id: u32,
) -> anyhow::Result<Vec<u8>> {
    Ok(SmsgAttackerStateUpdateResponse {
        hit_info: outcome.hit_info,
        attacker,
        victim,
        total_damage: outcome.total_damage,
        school: 0,
        school_damage: outcome.school_damage,
        absorbed: outcome.absorbed,
        resisted: outcome.resisted,
        victim_state: outcome.victim_state,
        spell_id,
        blocked: outcome.blocked,
    }
    .body())
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct SpellNonMeleeDamageLogPacket {
    pub(in crate::world) attacker: ObjectGuid,
    pub(in crate::world) target: ObjectGuid,
    pub(in crate::world) spell_id: u32,
    pub(in crate::world) damage: u32,
    pub(in crate::world) school: u8,
    pub(in crate::world) absorb: u32,
    pub(in crate::world) resist: i32,
    pub(in crate::world) periodic: bool,
    pub(in crate::world) blocked: u32,
    pub(in crate::world) hit_info: u32,
}

pub(in crate::world) fn build_spell_non_melee_damage_log_body(
    log: SpellNonMeleeDamageLogPacket,
) -> anyhow::Result<Vec<u8>> {
    Ok(SmsgSpellNonMeleeDamageLogResponse {
        attacker: log.attacker,
        target: log.target,
        spell_id: log.spell_id,
        damage: log.damage,
        school: log.school,
        absorb: log.absorb,
        resist: log.resist,
        periodic: log.periodic,
        blocked: log.blocked,
        hit_info: log.hit_info,
    }
    .body())
}

pub(in crate::world) fn build_environmental_damage_log_body(
    player: ObjectGuid,
    damage_type: u8,
    damage: u32,
    absorbed: u32,
    resisted: u32,
) -> anyhow::Result<Vec<u8>> {
    Ok(SmsgEnvironmentalDamageLogResponse {
        player,
        damage_type,
        damage,
        absorbed,
        resisted,
    }
    .body())
}

pub(in crate::world) fn build_spell_log_miss_body(
    caster: ObjectGuid,
    target: ObjectGuid,
    spell_id: u32,
    miss_info: u8,
) -> anyhow::Result<Vec<u8>> {
    Ok(SmsgSpellLogMissResponse {
        caster,
        target,
        spell_id,
        miss_info,
    }
    .body())
}

pub(in crate::world) fn build_spell_heal_log_body(
    caster: ObjectGuid,
    target: ObjectGuid,
    spell_id: u32,
    heal: u32,
    critical: bool,
) -> anyhow::Result<Vec<u8>> {
    Ok(SmsgSpellHealLogResponse {
        caster,
        target,
        spell_id,
        heal,
        critical,
    }
    .body())
}

pub(in crate::world) fn build_spell_energize_log_body(
    caster: ObjectGuid,
    target: ObjectGuid,
    spell_id: u32,
    power_type: u32,
    amount: u32,
) -> anyhow::Result<Vec<u8>> {
    Ok(SmsgSpellEnergizeLogResponse {
        caster,
        target,
        spell_id,
        power_type,
        amount,
    }
    .body())
}

pub(in crate::world) fn build_spell_failure_body(
    caster: ObjectGuid,
    spell_id: u32,
    result: u8,
) -> anyhow::Result<Vec<u8>> {
    Ok(SmsgSpellFailureResponse {
        caster,
        spell_id,
        result,
    }
    .body())
}

pub(in crate::world) fn build_spell_failed_other_body(
    caster: ObjectGuid,
    spell_id: u32,
) -> Vec<u8> {
    SmsgSpellFailedOtherResponse { caster, spell_id }.body()
}

pub(in crate::world) fn build_spell_delayed_body(
    caster: ObjectGuid,
    delay_millis: u32,
) -> anyhow::Result<Vec<u8>> {
    Ok(SmsgSpellDelayedResponse {
        caster,
        delay_millis,
    }
    .body())
}

pub(in crate::world) fn build_player_rage_update_body(
    player: ObjectGuid,
    rage: u32,
) -> anyhow::Result<Vec<u8>> {
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

pub(in crate::world) fn build_player_energy_update_body(
    player: ObjectGuid,
    energy: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(
        &mut values,
        UNIT_FIELD_POWER4,
        energy.min(POWER_ENERGY_DEFAULT),
    )?;
    write_update_values(&mut block, &values)?;

    let mut body = Vec::with_capacity(5 + block.len());
    body.extend_from_slice(&1u32.to_le_bytes());
    body.push(0);
    body.extend_from_slice(&block);
    Ok(body)
}

pub(in crate::world) fn build_player_combo_points_update_body(
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

pub(in crate::world) fn build_db_creature_state_update_body(
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

pub(in crate::world) fn build_db_creature_death_update_body(
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

pub(in crate::world) fn build_unit_flags_update_body(
    guid: ObjectGuid,
    flags: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, guid)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, UNIT_FIELD_FLAGS, flags)?;
    write_update_values(&mut block, &values)?;

    Ok(build_update_object_body(&[block]))
}

pub(in crate::world) fn build_player_mana_update_body(
    player: ObjectGuid,
    mana: u32,
) -> anyhow::Result<Vec<u8>> {
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

pub(in crate::world) fn build_player_health_update_body(
    player: ObjectGuid,
    health: u32,
) -> anyhow::Result<Vec<u8>> {
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
pub(in crate::world) fn retaliation_damage_for_db_creature(
    session: &mut WorldSessionState,
    target: ObjectGuid,
) -> u32 {
    let Some(creature) = session.visibility.db_creatures.get(&target.raw()) else {
        return 0;
    };
    let retaliation_damage = creature.hit_damage().max(1);
    session.character.player_health = session
        .character
        .player_health
        .saturating_sub(retaliation_damage);
    retaliation_damage
}
