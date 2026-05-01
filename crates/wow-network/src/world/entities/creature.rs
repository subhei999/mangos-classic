#[derive(Debug, Clone, Copy)]
struct CreatureCombatState {
    attacker: ObjectGuid,
    victim: ObjectGuid,
    next_swing_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct CreatureThreatEntry {
    victim: ObjectGuid,
    threat: f32,
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

// CMaNGOS reference: src/game/Entities/Creature.* DB creature update builders.
fn build_db_creature_create_blocks(creatures: &[DbCreatureRuntime]) -> anyhow::Result<Vec<Vec<u8>>> {
    creatures
        .iter()
        .map(build_db_creature_runtime_create_block)
        .collect()
}

#[cfg(test)]
fn build_db_creature_create_block(creature: &CreatureSpawnQuery) -> anyhow::Result<Vec<u8>> {
    build_db_creature_create_block_inner(
        creature,
        db_creature_spawn_position(creature),
        creature_health(&creature.template),
        creature.template.dynamic_flags,
        creature.template.unit_flags,
        creature.template.npc_flags,
    )
}

fn build_db_creature_runtime_create_block(creature: &DbCreatureRuntime) -> anyhow::Result<Vec<u8>> {
    build_db_creature_create_block_inner(
        &creature.spawn,
        creature.current_position,
        creature.health,
        creature.dynamic_flags(),
        db_creature_unit_flags(creature, false),
        if creature.life_state == DbCreatureLifeState::Corpse {
            0
        } else {
            db_creature_npc_flags(creature)
        },
    )
}

fn build_db_creature_create_block_inner(
    creature: &CreatureSpawnQuery,
    position: WorldPosition,
    health: u32,
    dynamic_flags: u32,
    unit_flags: u32,
    npc_flags: u32,
) -> anyhow::Result<Vec<u8>> {
    let guid = creature_spawn_guid(creature);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_CREATE_OBJECT2);
    PackedGuid::write(&mut block, guid)?;
    block.push(TYPEID_UNIT);

    block.push(UPDATEFLAG_ALL | UPDATEFLAG_LIVING | UPDATEFLAG_HAS_POSITION);
    block.extend_from_slice(&0u32.to_le_bytes());
    block.extend_from_slice(&0u32.to_le_bytes());
    block.extend_from_slice(&position.x.to_le_bytes());
    block.extend_from_slice(&position.y.to_le_bytes());
    block.extend_from_slice(&position.z.to_le_bytes());
    block.extend_from_slice(&position.orientation.to_le_bytes());
    block.extend_from_slice(&0u32.to_le_bytes());
    block.extend_from_slice(&2.5f32.to_le_bytes());
    block.extend_from_slice(&7.0f32.to_le_bytes());
    block.extend_from_slice(&4.5f32.to_le_bytes());
    block.extend_from_slice(&4.722222f32.to_le_bytes());
    block.extend_from_slice(&2.5f32.to_le_bytes());
    block.extend_from_slice(&std::f32::consts::PI.to_le_bytes());
    block.extend_from_slice(&1u32.to_le_bytes());

    write_db_creature_update_values(
        &mut block,
        guid,
        creature,
        health,
        dynamic_flags,
        unit_flags,
        npc_flags,
    )?;
    Ok(block)
}

fn write_db_creature_update_values(
    body: &mut Vec<u8>,
    guid: ObjectGuid,
    creature: &CreatureSpawnQuery,
    health: u32,
    dynamic_flags: u32,
    unit_flags: u32,
    npc_flags: u32,
) -> anyhow::Result<()> {
    let template = &creature.template;
    let max_health = creature_health(template);
    let display_id = creature_display_id(template);
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, 0x000, guid.raw() as u32)?;
    set_update_value(&mut values, 0x001, (guid.raw() >> 32) as u32)?;
    set_update_value(&mut values, 0x002, TYPEMASK_OBJECT_UNIT)?;
    set_update_value(&mut values, 0x003, creature.entry)?;
    set_update_value(&mut values, 0x004, creature_scale(template).to_bits())?;
    set_update_value(&mut values, UNIT_FIELD_HEALTH, health)?;
    set_update_value(&mut values, UNIT_FIELD_MAXHEALTH, max_health)?;
    set_update_value(&mut values, UNIT_FIELD_LEVEL, template.min_level as u32)?;
    set_update_value(&mut values, UNIT_FIELD_FACTIONTEMPLATE, template.faction)?;
    set_update_value(&mut values, UNIT_FIELD_BYTES_0, 0)?;
    set_update_value(&mut values, UNIT_FIELD_FLAGS, unit_flags)?;
    set_update_value(
        &mut values,
        UNIT_FIELD_BASEATTACKTIME,
        template.melee_base_attack_time.max(1),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_BASEATTACKTIME + 1,
        template.melee_base_attack_time.max(1),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_RANGEDATTACKTIME,
        template.ranged_base_attack_time.max(1),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_BOUNDINGRADIUS,
        creature_bounding_radius(template).to_bits(),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_COMBATREACH,
        creature_combat_reach(template).to_bits(),
    )?;
    set_update_value(&mut values, UNIT_FIELD_DISPLAYID, display_id)?;
    set_update_value(&mut values, UNIT_FIELD_NATIVEDISPLAYID, display_id)?;
    set_update_value(&mut values, UNIT_FIELD_MINDAMAGE, template.min_melee_dmg.to_bits())?;
    set_update_value(&mut values, UNIT_FIELD_MAXDAMAGE, template.max_melee_dmg.to_bits())?;
    set_update_value(&mut values, UNIT_FIELD_BYTES_1, 0)?;
    set_update_value(&mut values, UNIT_DYNAMIC_FLAGS, dynamic_flags)?;
    set_update_value(&mut values, UNIT_MOD_CAST_SPEED, 1.0f32.to_bits())?;
    set_update_value(&mut values, UNIT_NPC_FLAGS, npc_flags)?;
    write_update_values(body, &values)
}

fn creature_spawn_guid(creature: &CreatureSpawnQuery) -> ObjectGuid {
    ObjectGuid::new(HighGuid::Unit, creature.entry, creature.guid)
}

fn db_creature_npc_flags(creature: &DbCreatureRuntime) -> u32 {
    if is_spirit_healer_creature(creature) {
        creature.spawn.template.npc_flags | UNIT_NPC_FLAG_SPIRITHEALER
    } else {
        creature.spawn.template.npc_flags
    }
}

fn creature_health(template: &CreatureTemplateQuery) -> u32 {
    template
        .max_level_health
        .max(template.min_level_health)
        .max(1)
}

fn creature_display_id(template: &CreatureTemplateQuery) -> u32 {
    [
        template.display_id1,
        template.display_id2,
        template.display_id3,
        template.display_id4,
    ]
    .into_iter()
    .find(|display_id| *display_id != 0)
    .unwrap_or(0)
}

fn creature_scale(template: &CreatureTemplateQuery) -> f32 {
    if template.scale > 0.0 {
        template.scale
    } else {
        1.0
    }
}
