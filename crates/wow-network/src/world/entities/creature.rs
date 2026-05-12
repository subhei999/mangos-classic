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
    move_speeds: UnitMoveSpeeds,
    health: u32,
    power1: u32,
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
    next_spell_list_update_at: Option<Instant>,
    spell_cooldowns_until: HashMap<u32, Instant>,
    spell_list_availability_id: Option<u32>,
    unavailable_spell_list_positions: HashSet<u32>,
    native_display: CreatureDisplaySelection,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CreatureDisplaySelection {
    display_id: u32,
    gender: u8,
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
        creature_native_display(creature),
        None,
        creature_mana(&creature.template),
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
        creature.native_display,
        creature.display_id_override,
        creature.power1,
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
    native_display: CreatureDisplaySelection,
    display_id_override: Option<u32>,
    power1: u32,
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
        native_display,
        display_id_override,
        power1,
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
    native_display: CreatureDisplaySelection,
    display_id_override: Option<u32>,
    power1: u32,
) -> anyhow::Result<()> {
    let template = &creature.template;
    let max_health = creature_health(template);
    let display_id = display_id_override.unwrap_or(native_display.display_id);
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, 0x000, guid.raw() as u32)?;
    set_update_value(&mut values, 0x001, (guid.raw() >> 32) as u32)?;
    set_update_value(&mut values, 0x002, TYPEMASK_OBJECT_UNIT)?;
    set_update_value(&mut values, 0x003, creature.entry)?;
    set_update_value(&mut values, 0x004, creature_scale(template).to_bits())?;
    set_update_value(&mut values, UNIT_FIELD_HEALTH, health)?;
    set_update_value(&mut values, UNIT_FIELD_MAXHEALTH, max_health)?;
    let max_mana = creature_mana(template);
    set_update_value(&mut values, UNIT_FIELD_POWER1, power1.min(max_mana))?;
    set_update_value(&mut values, UNIT_FIELD_MAXPOWER1, max_mana)?;
    set_update_value(&mut values, UNIT_FIELD_LEVEL, template.min_level as u32)?;
    set_update_value(&mut values, UNIT_FIELD_FACTIONTEMPLATE, template.faction)?;
    set_update_value(
        &mut values,
        UNIT_FIELD_BYTES_0,
        creature_unit_bytes_0(template, native_display.gender),
    )?;
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

fn build_db_creature_power_update_body(creature: ObjectGuid, power1: u32) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, creature)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, UNIT_FIELD_POWER1, power1)?;
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

fn creature_mana(template: &CreatureTemplateQuery) -> u32 {
    if creature_unit_power_type(template) == POWER_MANA as u32 {
        template.max_level_mana.max(template.min_level_mana)
    } else {
        0
    }
}

fn creature_display_id(template: &CreatureTemplateQuery) -> u32 {
    creature_default_display_id(template)
}

fn creature_default_display_id(template: &CreatureTemplateQuery) -> u32 {
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

#[cfg(test)]
fn creature_native_display(creature: &CreatureSpawnQuery) -> CreatureDisplaySelection {
    choose_creature_display_for_roll(&creature.template, creature.guid, false)
}

fn choose_creature_display(template: &CreatureTemplateQuery) -> CreatureDisplaySelection {
    let chance_total = creature_display_chance_total(template);
    let display_roll = if chance_total > 0 {
        rand::thread_rng().gen_range(0..chance_total)
    } else {
        0
    };
    let use_other_gender = rand::thread_rng().gen_range(0..=1) == 0;
    choose_creature_display_for_roll(template, display_roll, use_other_gender)
}

fn choose_creature_display_for_roll(
    template: &CreatureTemplateQuery,
    display_roll: u32,
    use_other_gender: bool,
) -> CreatureDisplaySelection {
    let models = creature_template_model_candidates(template);
    let chance_total = creature_display_chance_total(template);
    let mut roll = if chance_total > 0 {
        display_roll % chance_total
    } else {
        0
    };

    let mut selected = None;
    if chance_total > 0 {
        for model in models {
            if model.display_id == 0 {
                continue;
            }
            if roll < model.probability {
                selected = Some(model);
                break;
            }
            roll = roll.saturating_sub(model.probability);
        }
    }

    let selected = selected.or_else(|| {
        models
            .into_iter()
            .find(|model| model.display_id != 0)
    });

    let Some(selected) = selected else {
        return CreatureDisplaySelection {
            display_id: 0,
            gender: 0,
        };
    };

    if use_other_gender && selected.other_gender_display_id != 0 {
        return CreatureDisplaySelection {
            display_id: selected.other_gender_display_id,
            gender: sanitize_creature_gender(selected.other_gender),
        };
    }

    CreatureDisplaySelection {
        display_id: selected.display_id,
        gender: sanitize_creature_gender(selected.gender),
    }
}

#[derive(Debug, Clone, Copy)]
struct CreatureTemplateModelCandidate {
    display_id: u32,
    probability: u32,
    gender: u8,
    other_gender_display_id: u32,
    other_gender: u8,
}

fn creature_template_model_candidates(
    template: &CreatureTemplateQuery,
) -> [CreatureTemplateModelCandidate; 4] {
    [
        CreatureTemplateModelCandidate {
            display_id: template.display_id1,
            probability: template.display_id_probability1,
            gender: template.model_gender1,
            other_gender_display_id: template.model_other_gender1,
            other_gender: template.model_other_gender_gender1,
        },
        CreatureTemplateModelCandidate {
            display_id: template.display_id2,
            probability: template.display_id_probability2,
            gender: template.model_gender2,
            other_gender_display_id: template.model_other_gender2,
            other_gender: template.model_other_gender_gender2,
        },
        CreatureTemplateModelCandidate {
            display_id: template.display_id3,
            probability: template.display_id_probability3,
            gender: template.model_gender3,
            other_gender_display_id: template.model_other_gender3,
            other_gender: template.model_other_gender_gender3,
        },
        CreatureTemplateModelCandidate {
            display_id: template.display_id4,
            probability: template.display_id_probability4,
            gender: template.model_gender4,
            other_gender_display_id: template.model_other_gender4,
            other_gender: template.model_other_gender_gender4,
        },
    ]
}

fn creature_display_chance_total(template: &CreatureTemplateQuery) -> u32 {
    creature_template_model_candidates(template)
        .into_iter()
        .filter(|model| model.display_id != 0)
        .map(|model| model.probability)
        .sum()
}

fn sanitize_creature_gender(gender: u8) -> u8 {
    if gender <= 2 {
        gender
    } else {
        0
    }
}

fn creature_unit_bytes_0(template: &CreatureTemplateQuery, gender: u8) -> u32 {
    let power_type = creature_unit_power_type(template);
    ((template.unit_class as u32) << 8)
        | ((sanitize_creature_gender(gender) as u32) << 16)
        | (power_type << 24)
}

fn creature_unit_power_type(template: &CreatureTemplateQuery) -> u32 {
    match template.unit_class {
        1 => 1, // warrior rage
        2 | 8 => 0, // paladin/mage mana
        4 => 3, // rogue energy
        _ => 0,
    }
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
