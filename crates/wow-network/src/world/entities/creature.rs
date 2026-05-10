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
    waypoint_resume_position: Option<WorldPosition>,
    already_called_assistance: bool,
    next_spline_id: u32,
    health: u32,
    life_state: CreatureLifeState,
    corpse_expires_at: Option<Instant>,
    respawn_at: Option<Instant>,
    respawn_epoch_secs: Option<u64>,
    life_generation: u64,
    client_visible: bool,
    lootable: bool,
    looting: bool,
    loot_money: u32,
    loot_money_available: bool,
    loot_items: Vec<CreatureLoot>,
    loot_items_generated: bool,
    loot_roll_released_slots: HashSet<u8>,
    loot_current_looter_pass_slots: HashSet<u8>,
    loot_owner: Option<CreatureLootOwner>,
    loot_current_looter: Option<u32>,
    loot_allowed_players: HashSet<u32>,
    loot_method: Option<CreatureLootMethod>,
    active_auras: Vec<ActiveAura>,
    display_id_override: Option<u32>,
    pending_movement_scripts: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CreatureLootOwner {
    Player(u32),
    Party(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CreatureLootMethod {
    method: u8,
    threshold: u8,
    master_looter: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CreatureLifeState {
    Alive,
    Corpse,
    Dead,
}

#[derive(Debug, Clone)]
struct CreatureLoot {
    slot: u8,
    item: u32,
    count: u32,
    display_id: u32,
    quality: u8,
    free_for_all: bool,
    quest_drop: bool,
}

fn build_db_creature_create_blocks_for_player(
    creatures: &[DbCreatureRuntime],
    character_guid: Option<u32>,
) -> anyhow::Result<Vec<Vec<u8>>> {
    creatures
        .iter()
        .map(|creature| build_db_creature_runtime_create_block_for_player(creature, character_guid))
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
        &[],
        None,
    )
}

fn build_db_creature_runtime_create_block(creature: &DbCreatureRuntime) -> anyhow::Result<Vec<u8>> {
    build_db_creature_runtime_create_block_for_player(creature, None)
}

fn build_db_creature_runtime_create_block_for_player(
    creature: &DbCreatureRuntime,
    character_guid: Option<u32>,
) -> anyhow::Result<Vec<u8>> {
    build_db_creature_create_block_inner(
        &creature.spawn,
        creature.current_position,
        creature.health,
        creature.dynamic_flags_for_player(character_guid),
        db_creature_unit_flags(creature, false),
        if creature.life_state == DbCreatureLifeState::Corpse {
            0
        } else {
            db_creature_npc_flags(creature)
        },
        &creature.active_auras,
        creature.display_id_override,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_db_creature_create_block_inner(
    creature: &CreatureSpawnQuery,
    position: WorldPosition,
    health: u32,
    dynamic_flags: u32,
    unit_flags: u32,
    npc_flags: u32,
    active_auras: &[ActiveAura],
    display_id_override: Option<u32>,
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
    let speeds = creature_movement_speeds(&creature.template);
    block.extend_from_slice(&speeds.walk.to_le_bytes());
    block.extend_from_slice(&speeds.run.to_le_bytes());
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
        active_auras,
        display_id_override,
    )?;
    Ok(block)
}

#[allow(clippy::too_many_arguments)]
fn write_db_creature_update_values(
    body: &mut Vec<u8>,
    guid: ObjectGuid,
    creature: &CreatureSpawnQuery,
    health: u32,
    dynamic_flags: u32,
    unit_flags: u32,
    npc_flags: u32,
    active_auras: &[ActiveAura],
    display_id_override: Option<u32>,
) -> anyhow::Result<()> {
    let template = &creature.template;
    let max_health = creature_health(template);
    let display_id = display_id_override.unwrap_or_else(|| creature_display_id(template));
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
    set_update_value(
        &mut values,
        UNIT_VIRTUAL_ITEM_SLOT_DISPLAY,
        template.equip_display_id1,
    )?;
    set_update_value(
        &mut values,
        UNIT_VIRTUAL_ITEM_SLOT_DISPLAY + 1,
        template.equip_display_id2,
    )?;
    set_update_value(
        &mut values,
        UNIT_VIRTUAL_ITEM_SLOT_DISPLAY + 2,
        template.equip_display_id3,
    )?;
    set_update_value(
        &mut values,
        UNIT_VIRTUAL_ITEM_INFO,
        packed_virtual_item_info0(
            template.equip_class1,
            template.equip_subclass1,
            template.equip_material1,
            template.equip_inventory_type1,
        ),
    )?;
    set_update_value(
        &mut values,
        UNIT_VIRTUAL_ITEM_INFO + 1,
        packed_virtual_item_info1(template.equip_sheath1),
    )?;
    set_update_value(
        &mut values,
        UNIT_VIRTUAL_ITEM_INFO + 2,
        packed_virtual_item_info0(
            template.equip_class2,
            template.equip_subclass2,
            template.equip_material2,
            template.equip_inventory_type2,
        ),
    )?;
    set_update_value(
        &mut values,
        UNIT_VIRTUAL_ITEM_INFO + 3,
        packed_virtual_item_info1(template.equip_sheath2),
    )?;
    set_update_value(
        &mut values,
        UNIT_VIRTUAL_ITEM_INFO + 4,
        packed_virtual_item_info0(
            template.equip_class3,
            template.equip_subclass3,
            template.equip_material3,
            template.equip_inventory_type3,
        ),
    )?;
    set_update_value(
        &mut values,
        UNIT_VIRTUAL_ITEM_INFO + 5,
        packed_virtual_item_info1(template.equip_sheath3),
    )?;
    set_update_value(&mut values, UNIT_FIELD_MINDAMAGE, template.min_melee_dmg.to_bits())?;
    set_update_value(&mut values, UNIT_FIELD_MAXDAMAGE, template.max_melee_dmg.to_bits())?;
    set_update_value(&mut values, UNIT_FIELD_BYTES_1, 0)?;
    set_update_value(&mut values, UNIT_FIELD_BYTES_2, creature_unit_bytes_2())?;
    set_update_value(&mut values, UNIT_DYNAMIC_FLAGS, dynamic_flags)?;
    set_update_value(&mut values, UNIT_MOD_CAST_SPEED, 1.0f32.to_bits())?;
    set_update_value(&mut values, UNIT_NPC_EMOTESTATE, creature.addon_emote)?;
    set_update_value(&mut values, UNIT_NPC_FLAGS, npc_flags)?;
    set_unit_aura_update_values(&mut values, active_auras)?;
    write_update_values(body, &values)
}

fn build_db_creature_aura_update_body(
    creature: ObjectGuid,
    active_auras: &[ActiveAura],
) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, creature)?;

    let mut values = vec![None; PLAYER_END_FIELDS];
    set_unit_aura_update_values(&mut values, active_auras)?;

    write_update_values(&mut block, &values)?;
    Ok(build_update_object_body(&[block]))
}

fn build_db_creature_emote_state_update_body(
    creature: ObjectGuid,
    emote: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, creature)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, UNIT_NPC_EMOTESTATE, emote)?;
    write_update_values(&mut block, &values)?;
    Ok(build_update_object_body(&[block]))
}

fn build_db_creature_display_update_body(
    creature: ObjectGuid,
    display_id: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, creature)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, UNIT_FIELD_DISPLAYID, display_id)?;
    write_update_values(&mut block, &values)?;
    Ok(build_update_object_body(&[block]))
}

fn packed_virtual_item_info0(class: u32, subclass: u32, material: i32, inventory_type: u32) -> u32 {
    (class & 0xFF)
        | ((subclass & 0xFF) << 8)
        | (((material as u32) & 0xFF) << 16)
        | ((inventory_type & 0xFF) << 24)
}

fn packed_virtual_item_info1(sheath: u32) -> u32 {
    sheath & 0xFF
}

fn creature_unit_bytes_2() -> u32 {
    const SHEATH_STATE_MELEE: u32 = 1;
    const UNIT_BYTE2_FLAG_AURAS: u32 = 0x10;
    SHEATH_STATE_MELEE | (UNIT_BYTE2_FLAG_AURAS << 8)
}

fn creature_spawn_guid(creature: &CreatureSpawnQuery) -> ObjectGuid {
    ObjectGuid::new(HighGuid::Unit, creature.entry, creature.guid)
}

fn db_creature_npc_flags(creature: &DbCreatureRuntime) -> u32 {
    creature.spawn.template.npc_flags
}

const CREATURE_TYPE_FLAG_VISIBLE_TO_GHOSTS: u32 = 0x0000_0002;

fn db_creature_visible_to_ghosts(creature: &DbCreatureRuntime) -> bool {
    creature.spawn.template.creature_type_flags & CREATURE_TYPE_FLAG_VISIBLE_TO_GHOSTS != 0
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

#[derive(Debug, Clone, Copy)]
struct CreatureMovementSpeeds {
    walk: f32,
    run: f32,
}

fn creature_movement_speeds(template: &CreatureTemplateQuery) -> CreatureMovementSpeeds {
    let walk_rate = if template.speed_walk > 0.0 {
        template.speed_walk
    } else {
        1.0
    };
    let run_rate = if template.speed_run > 0.0 {
        template.speed_run
    } else {
        1.0
    };
    CreatureMovementSpeeds {
        walk: DB_CREATURE_WALK_SPEED_YARDS_PER_SEC * walk_rate,
        run: DB_CREATURE_RUN_SPEED_YARDS_PER_SEC * run_rate,
    }
}
