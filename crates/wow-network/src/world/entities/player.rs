use super::*;

#[derive(Debug, Clone)]
pub(in crate::world) struct Player {
    pub(in crate::world) guid: u32,
    pub(in crate::world) name: String,
    pub(in crate::world) race: u8,
    pub(in crate::world) class: u8,
    pub(in crate::world) level: u8,
    pub(in crate::world) xp: u32,
    pub(in crate::world) position: WorldPosition,
    pub(in crate::world) movement_flags: u32,
    pub(in crate::world) client_time: u32,
    pub(in crate::world) fall_time: u32,
    pub(in crate::world) jump: JumpInfo,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::world) enum PlayerDeathState {
    #[default]
    Alive,
    JustDied,
    Corpse,
    Ghost,
}

#[derive(Debug, Clone, PartialEq)]
pub(in crate::world) struct PlayerVisualState {
    pub(in crate::world) gender: u8,
    pub(in crate::world) player_bytes: u32,
    pub(in crate::world) player_bytes2: u32,
    pub(in crate::world) equipment_cache: Option<String>,
    pub(in crate::world) guildid: Option<u32>,
}

// CMaNGOS reference: src/game/Entities/Player.* player update builders.
pub(in crate::world) fn build_other_player_create_block(
    player: &PlayerRuntime,
) -> anyhow::Result<Vec<u8>> {
    let guid = ObjectGuid::new(HighGuid::Player, 0, player.guid);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_CREATE_OBJECT2);
    PackedGuid::write(&mut block, guid)?;
    block.push(TYPEID_PLAYER);

    block.push(UPDATEFLAG_ALL | UPDATEFLAG_LIVING | UPDATEFLAG_HAS_POSITION);
    block.extend_from_slice(&player.movement_flags.to_le_bytes());
    block.extend_from_slice(&player.server_time.to_le_bytes());
    block.extend_from_slice(&player.position.x.to_le_bytes());
    block.extend_from_slice(&player.position.y.to_le_bytes());
    block.extend_from_slice(&player.position.z.to_le_bytes());
    block.extend_from_slice(&player.position.orientation.to_le_bytes());
    block.extend_from_slice(&player.fall_time.to_le_bytes());
    if player.movement_flags & MOVEFLAG_JUMPING != 0 {
        block.extend_from_slice(&player.jump.z_speed.to_le_bytes());
        block.extend_from_slice(&player.jump.cos_angle.to_le_bytes());
        block.extend_from_slice(&player.jump.sin_angle.to_le_bytes());
        block.extend_from_slice(&player.jump.xy_speed.to_le_bytes());
    }
    block.extend_from_slice(&2.5f32.to_le_bytes());
    block.extend_from_slice(&7.0f32.to_le_bytes());
    block.extend_from_slice(&4.5f32.to_le_bytes());
    block.extend_from_slice(&4.722222f32.to_le_bytes());
    block.extend_from_slice(&2.5f32.to_le_bytes());
    block.extend_from_slice(&std::f32::consts::PI.to_le_bytes());
    block.extend_from_slice(&1u32.to_le_bytes());

    write_other_player_update_values(&mut block, guid, player)?;
    Ok(block)
}

pub(in crate::world) fn write_other_player_update_values(
    body: &mut Vec<u8>,
    guid: ObjectGuid,
    player: &PlayerRuntime,
) -> anyhow::Result<()> {
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, 0x000, guid.raw() as u32)?;
    set_update_value(&mut values, 0x001, (guid.raw() >> 32) as u32)?;
    set_update_value(&mut values, 0x002, TYPEMASK_OBJECT_UNIT_PLAYER)?;
    set_update_value(&mut values, 0x004, 1.0f32.to_bits())?;
    set_update_value(&mut values, UNIT_FIELD_HEALTH, player.health)?;
    set_update_value(&mut values, UNIT_FIELD_POWER1, player.power1)?;
    set_update_value(&mut values, UNIT_FIELD_POWER2, player.power2)?;
    set_update_value(&mut values, UNIT_FIELD_MAXHEALTH, player.max_health.max(1))?;
    set_update_value(&mut values, UNIT_FIELD_MAXPOWER1, player.max_power1)?;
    set_update_value(&mut values, UNIT_FIELD_MAXPOWER2, POWER_RAGE_DEFAULT)?;
    set_update_value(&mut values, UNIT_FIELD_LEVEL, player.level as u32)?;
    set_update_value(
        &mut values,
        UNIT_FIELD_FACTIONTEMPLATE,
        player_faction_template(player.race, player.flags),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_BYTES_0,
        player.race as u32
            | ((player.class as u32) << 8)
            | ((player.gender as u32) << 16)
            | (u32::from(player.class == 1) << 24),
    )?;
    let unit_flags = player_unit_flags_with_looting_and_auras(
        player.in_combat,
        player.looting,
        &player.active_auras,
    );
    set_update_value(&mut values, UNIT_FIELD_FLAGS, unit_flags)?;
    set_object_guid_update_values(&mut values, UNIT_FIELD_TARGET, player.unit_target)?;
    set_update_value(&mut values, UNIT_FIELD_BASEATTACKTIME, BASE_ATTACK_TIME_MS)?;
    set_update_value(
        &mut values,
        UNIT_FIELD_BASEATTACKTIME + 1,
        BASE_ATTACK_TIME_MS,
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_RANGEDATTACKTIME,
        BASE_ATTACK_TIME_MS,
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_BOUNDINGRADIUS,
        DEFAULT_WORLD_OBJECT_SIZE.to_bits(),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_COMBATREACH,
        PLAYER_COMBAT_REACH_YARDS.to_bits(),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_DISPLAYID,
        display_id_for_runtime_player(player),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_NATIVEDISPLAYID,
        display_id_for_runtime_player(player),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_BYTES_1,
        player_unit_bytes_1_with_auras(player.class, player.stand_state, &player.active_auras),
    )?;
    set_update_value(&mut values, UNIT_MOD_CAST_SPEED, 1.0f32.to_bits())?;
    set_update_value(&mut values, PLAYER_FLAGS_FIELD, player.flags)?;
    set_update_value(&mut values, PLAYER_BYTES, player.player_bytes)?;
    set_update_value(
        &mut values,
        PLAYER_BYTES_2,
        player_bytes2_with_rest_state(player.player_bytes2),
    )?;
    set_update_value(&mut values, PLAYER_BYTES_3, 0)?;
    set_visible_item_update_values_from_equipment(&mut values, &player.visible_equipment)?;
    set_player_aura_update_values(
        &mut values,
        player.class,
        player.stand_state,
        player.aura_state,
        &player.active_auras,
    )?;
    write_update_values(body, &values)
}

pub(in crate::world) fn display_id_for_runtime_player(player: &PlayerRuntime) -> u32 {
    match (player.race, player.gender) {
        (1, 0) => 49,
        (1, 1) => 50,
        (2, 0) => 51,
        (2, 1) => 52,
        (3, 0) => 53,
        (3, 1) => 54,
        (4, 0) => 55,
        (4, 1) => 56,
        (5, 0) => 57,
        (5, 1) => 58,
        (6, 0) => 59,
        (6, 1) => 60,
        (7, 0) => 1563,
        (7, 1) => 1564,
        (8, 0) => 1478,
        (8, 1) => 1479,
        _ => 49,
    }
}

// CMaNGOS reference: src/game/Entities/Player.* self/player update fields.
#[allow(clippy::too_many_arguments)]
pub(in crate::world) fn write_minimal_player_update_values(
    body: &mut Vec<u8>,
    guid: ObjectGuid,
    character: &CharacterEnumEntry,
    inventory: &[CharacterInventoryItem],
    base_world_stats: &PlayerWorldStats,
    world_stats: &PlayerWorldStats,
    skills: &[CharacterSkill],
    active_spells: &HashSet<u32>,
    quest_statuses: &HashMap<u32, CharacterQuestStatus>,
    equipped_templates: &[EquippedItemTemplate],
    ammo_template: Option<&ItemTemplateQuery>,
    active_auras: &[ActiveAura],
) -> anyhow::Result<()> {
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, 0x000, guid.raw() as u32)?;
    set_update_value(&mut values, 0x001, (guid.raw() >> 32) as u32)?;
    set_update_value(&mut values, 0x002, TYPEMASK_OBJECT_UNIT_PLAYER)?;
    set_update_value(&mut values, 0x004, 1.0f32.to_bits())?;
    set_player_vital_update_values(&mut values, character, world_stats)?;
    set_update_value(&mut values, UNIT_FIELD_LEVEL, character.level as u32)?;
    set_update_value(
        &mut values,
        UNIT_FIELD_FACTIONTEMPLATE,
        faction_for_race(character.race),
    )?;
    set_update_value(&mut values, UNIT_FIELD_BYTES_0, unit_bytes_0(character))?;
    set_update_value(
        &mut values,
        UNIT_FIELD_FLAGS,
        player_unit_flags_with_looting_and_auras(false, false, active_auras),
    )?;
    set_object_guid_update_values(&mut values, UNIT_FIELD_TARGET, None)?;
    let combat_stats = combat_stats_with_active_auras(
        player_combat_stats_for_values_with_known_spells_and_ammo(
            character.class,
            character.level,
            world_stats,
            skills,
            active_spells,
            equipped_templates,
            ammo_template,
        ),
        active_auras,
    );
    set_update_value(
        &mut values,
        UNIT_FIELD_BASEATTACKTIME,
        combat_stats.main_attack_time_ms,
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_BASEATTACKTIME + 1,
        combat_stats.off_attack_time_ms,
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_RANGEDATTACKTIME,
        combat_stats.ranged_attack_time_ms,
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_BOUNDINGRADIUS,
        DEFAULT_WORLD_OBJECT_SIZE.to_bits(),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_COMBATREACH,
        PLAYER_COMBAT_REACH_YARDS.to_bits(),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_DISPLAYID,
        display_id_for_character(character),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_NATIVEDISPLAYID,
        display_id_for_character(character),
    )?;
    set_update_value(&mut values, UNIT_FIELD_MOUNTDISPLAYID, 0)?;
    set_update_value(
        &mut values,
        UNIT_FIELD_MINDAMAGE,
        combat_stats.main_min_damage.to_bits(),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_MAXDAMAGE,
        combat_stats.main_max_damage.to_bits(),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_MINOFFHANDDAMAGE,
        combat_stats.off_min_damage.to_bits(),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_MAXOFFHANDDAMAGE,
        combat_stats.off_max_damage.to_bits(),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_BYTES_1,
        if character.health == 0 && character.player_flags & PLAYER_FLAGS_GHOST == 0 {
            player_unit_bytes_1_with_auras(character.class, PLAYER_STAND_STATE_DEAD, active_auras)
        } else {
            player_unit_bytes_1_with_auras(character.class, PLAYER_STAND_STATE_STAND, active_auras)
        },
    )?;
    if character.player_flags & PLAYER_FLAGS_GHOST != 0 {
        set_player_ghost_aura_update_values(&mut values, true, character.race, character.level)?;
    } else {
        set_player_aura_update_values(
            &mut values,
            character.class,
            PLAYER_STAND_STATE_STAND,
            0,
            active_auras,
        )?;
    }
    set_update_value(&mut values, UNIT_FIELD_AURASTATE, 0)?;
    set_update_value(&mut values, UNIT_MOD_CAST_SPEED, 1.0f32.to_bits())?;
    set_player_stat_update_values(&mut values, world_stats)?;
    set_update_value(&mut values, UNIT_FIELD_BYTES_2, unit_bytes_2())?;
    set_player_resistance_update_values(&mut values, &combat_stats)?;
    set_update_value(
        &mut values,
        UNIT_FIELD_ATTACK_POWER,
        combat_stats.melee_attack_power,
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_ATTACK_POWER_MODS,
        attack_power_mod_pair(
            combat_stats.melee_attack_power_mod_positive,
            combat_stats.melee_attack_power_mod_negative,
        ),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_ATTACK_POWER_MULTIPLIER,
        0.0f32.to_bits(),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_RANGED_ATTACK_POWER,
        combat_stats.ranged_attack_power,
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_RANGED_ATTACK_POWER_MODS,
        attack_power_mod_pair(
            combat_stats.ranged_attack_power_mod_positive,
            combat_stats.ranged_attack_power_mod_negative,
        ),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_RANGED_ATTACK_POWER_MULTIPLIER,
        0.0f32.to_bits(),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_MINRANGEDDAMAGE,
        combat_stats.ranged_min_damage.to_bits(),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_MAXRANGEDDAMAGE,
        combat_stats.ranged_max_damage.to_bits(),
    )?;
    for index in UNIT_FIELD_POWER_COST_MODIFIER..UNIT_FIELD_POWER_COST_MODIFIER + MAX_SPELL_SCHOOL {
        set_update_value(&mut values, index, 0)?;
    }
    for index in
        UNIT_FIELD_POWER_COST_MULTIPLIER..UNIT_FIELD_POWER_COST_MULTIPLIER + MAX_SPELL_SCHOOL
    {
        set_update_value(&mut values, index, 0.0f32.to_bits())?;
    }
    set_update_value(&mut values, PLAYER_FLAGS_FIELD, character.player_flags)?;
    set_update_value(&mut values, PLAYER_BYTES, character.player_bytes)?;
    set_update_value(
        &mut values,
        PLAYER_BYTES_2,
        player_bytes2_with_rest_bonus(character.player_bytes2, character.rest_bonus),
    )?;
    set_update_value(&mut values, PLAYER_BYTES_3, 0)?;
    set_visible_item_update_values(&mut values, character, inventory)?;
    set_inventory_slot_update_values(&mut values, inventory)?;
    set_update_value(&mut values, PLAYER_XP, character.xp)?;
    set_update_value(&mut values, PLAYER_NEXT_LEVEL_XP, world_stats.next_level_xp)?;
    set_update_value(
        &mut values,
        PLAYER_REST_STATE_EXPERIENCE,
        character.rest_bonus.max(0.0).min(u32::MAX as f32) as u32,
    )?;
    set_player_quest_log_update_values(&mut values, quest_statuses)?;
    set_player_skill_update_values(&mut values, skills, active_auras)?;
    set_player_secondary_stat_update_values(&mut values, &combat_stats)?;
    set_player_explored_zone_update_values(&mut values, character)?;
    set_update_value(&mut values, PLAYER_FIELD_COINAGE, character.money)?;
    set_player_stat_mod_update_values(&mut values, base_world_stats, world_stats)?;
    set_player_resistance_buff_mod_update_values(&mut values, &combat_stats)?;
    set_player_damage_mod_update_values(&mut values)?;
    set_update_value(
        &mut values,
        PLAYER_FIELD_BYTES,
        if character.health == 0 && character.player_flags & PLAYER_FLAGS_GHOST == 0 {
            PLAYER_FIELD_BYTE_RELEASE_TIMER
        } else {
            0
        },
    )?;
    set_update_value(&mut values, PLAYER_AMMO_ID, character.ammo_id)?;
    set_update_value(&mut values, PLAYER_SELF_RES_SPELL, 0)?;
    set_update_value(&mut values, PLAYER_FIELD_PVP_MEDALS, 0)?;
    set_update_value(&mut values, PLAYER_FIELD_BYTES2, 0)?;
    set_update_value(
        &mut values,
        PLAYER_FIELD_WATCHED_FACTION_INDEX,
        character.watched_faction,
    )?;

    write_update_values(body, &values)?;

    Ok(())
}

pub(in crate::world) fn set_player_quest_log_update_values(
    values: &mut [Option<u32>],
    quest_statuses: &HashMap<u32, CharacterQuestStatus>,
) -> anyhow::Result<()> {
    for (slot, status) in active_quest_statuses_sorted(quest_statuses)
        .into_iter()
        .take(MAX_QUEST_LOG_SIZE)
        .enumerate()
    {
        let base = PLAYER_QUEST_LOG_1_1 + slot * MAX_QUEST_OFFSET;
        set_update_value(values, base + QUEST_LOG_QUEST_ID_OFFSET, status.quest)?;
        set_update_value(
            values,
            base + QUEST_LOG_COUNT_STATE_OFFSET,
            quest_log_count_state(status),
        )?;
        set_update_value(values, base + QUEST_LOG_TIME_OFFSET, 0)?;
    }
    Ok(())
}

pub(in crate::world) fn set_player_vital_update_values(
    values: &mut [Option<u32>],
    character: &CharacterEnumEntry,
    world_stats: &PlayerWorldStats,
) -> anyhow::Result<()> {
    let max_health = world_stats.max_health().max(1);
    let health = if character.health == 0 && character.player_flags & PLAYER_FLAGS_GHOST == 0 {
        0
    } else {
        character
            .health
            .max(if character.player_flags & PLAYER_FLAGS_GHOST != 0 {
                PLAYER_SURVIVOR_HEALTH_FLOOR
            } else {
                1
            })
            .min(max_health)
    };
    let max_mana = world_stats.max_mana();
    let power1 = if character.power1 > 0 {
        character.power1
    } else {
        max_mana
    };
    let power2 = character
        .power2
        .min(create_power_for_class_power(character.class, POWER_RAGE));
    let power4 = if character.power4 > 0 {
        character.power4
    } else {
        create_power_for_class_power(character.class, POWER_ENERGY)
    };

    set_update_value(values, UNIT_FIELD_HEALTH, health)?;
    set_update_value(values, UNIT_FIELD_POWER1, power1)?;
    set_update_value(values, UNIT_FIELD_POWER2, power2)?;
    set_update_value(
        values,
        UNIT_FIELD_POWER3,
        create_power_for_class_power(character.class, POWER_FOCUS),
    )?;
    set_update_value(values, UNIT_FIELD_POWER4, power4)?;
    set_update_value(
        values,
        UNIT_FIELD_POWER5,
        create_power_for_class_power(character.class, POWER_HAPPINESS),
    )?;
    set_update_value(values, UNIT_FIELD_MAXHEALTH, max_health)?;
    set_update_value(values, UNIT_FIELD_MAXPOWER1, max_mana)?;
    set_update_value(
        values,
        UNIT_FIELD_MAXPOWER2,
        create_power_for_class_power(character.class, POWER_RAGE),
    )?;
    set_update_value(
        values,
        UNIT_FIELD_MAXPOWER3,
        create_power_for_class_power(character.class, POWER_FOCUS),
    )?;
    set_update_value(
        values,
        UNIT_FIELD_MAXPOWER4,
        create_power_for_class_power(character.class, POWER_ENERGY),
    )?;
    set_update_value(
        values,
        UNIT_FIELD_MAXPOWER5,
        create_power_for_class_power(character.class, POWER_HAPPINESS),
    )?;
    set_update_value(values, UNIT_FIELD_BASE_MANA, world_stats.base_mana)?;
    set_update_value(values, UNIT_FIELD_BASE_HEALTH, world_stats.base_health)?;

    Ok(())
}

pub(in crate::world) fn player_bytes2_with_rest_state(player_bytes2: u32) -> u32 {
    if (player_bytes2 >> 24) & 0xFF == 0 {
        player_bytes2 | ((REST_STATE_NORMAL as u32) << 24)
    } else {
        player_bytes2
    }
}

pub(in crate::world) fn set_object_guid_update_values(
    values: &mut [Option<u32>],
    field: usize,
    guid: Option<ObjectGuid>,
) -> anyhow::Result<()> {
    let raw = guid.unwrap_or(ObjectGuid::EMPTY).raw();
    set_update_value(values, field, raw as u32)?;
    set_update_value(values, field + 1, (raw >> 32) as u32)?;
    Ok(())
}

pub(in crate::world) fn build_player_selection_update_body(
    player_guid: u32,
    selected_target: Option<ObjectGuid>,
) -> anyhow::Result<Vec<u8>> {
    build_player_target_update_body(player_guid, selected_target)
}

pub(in crate::world) fn build_player_target_update_body(
    player_guid: u32,
    unit_target: Option<ObjectGuid>,
) -> anyhow::Result<Vec<u8>> {
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, player_guid);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player_guid)?;

    let mut values = vec![None; PLAYER_END_FIELDS];
    set_object_guid_update_values(&mut values, UNIT_FIELD_TARGET, unit_target)?;
    write_update_values(&mut block, &values)?;

    Ok(build_update_object_body(&[block]))
}

pub(in crate::world) fn build_player_gm_mode_update_body(
    player: ObjectGuid,
    race: u8,
    player_flags: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player)?;

    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(
        &mut values,
        UNIT_FIELD_FACTIONTEMPLATE,
        player_faction_template(race, player_flags),
    )?;
    set_update_value(&mut values, PLAYER_FLAGS_FIELD, player_flags)?;
    write_update_values(&mut block, &values)?;

    Ok(build_update_object_body(&[block]))
}

pub(in crate::world) fn build_player_stand_state_update_body(
    character: &Player,
    stand_state: u8,
) -> anyhow::Result<Vec<u8>> {
    build_player_stand_state_update_body_for_class(
        character.guid,
        character.class,
        stand_state,
        &[],
    )
}

pub(in crate::world) fn build_player_stand_state_update_body_for_class(
    character_guid: u32,
    class: u8,
    stand_state: u8,
    active_auras: &[ActiveAura],
) -> anyhow::Result<Vec<u8>> {
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player_guid)?;

    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(
        &mut values,
        UNIT_FIELD_BYTES_1,
        player_unit_bytes_1_with_auras(class, stand_state, active_auras),
    )?;
    write_update_values(&mut block, &values)?;

    Ok(build_update_object_body(&[block]))
}

pub(in crate::world) fn set_player_stat_update_values(
    values: &mut [Option<u32>],
    world_stats: &PlayerWorldStats,
) -> anyhow::Result<()> {
    for (offset, stat) in world_stats.stats.into_iter().enumerate() {
        set_update_value(values, UNIT_FIELD_STAT0 + offset, stat)?;
    }

    Ok(())
}

pub(in crate::world) fn set_player_skill_update_values(
    values: &mut [Option<u32>],
    skills: &[CharacterSkill],
    active_auras: &[ActiveAura],
) -> anyhow::Result<()> {
    for (slot, skill) in skills.iter().take(PLAYER_MAX_SKILLS).enumerate() {
        let field = PLAYER_SKILL_INFO_1_1 + slot * 3;
        set_update_value(values, field, make_pair32(skill.skill, 0))?;
        set_update_value(values, field + 1, make_pair32(skill.value, skill.max))?;
        set_update_value(
            values,
            field + 2,
            active_aura_skill_bonus_pair(active_auras, skill.skill),
        )?;
    }

    Ok(())
}

pub(in crate::world) fn set_player_resistance_update_values(
    values: &mut [Option<u32>],
    combat_stats: &PlayerCombatStats,
) -> anyhow::Result<()> {
    for (offset, resistance) in combat_stats.resistances.iter().enumerate() {
        set_update_value(values, UNIT_FIELD_RESISTANCES + offset, *resistance)?;
    }

    Ok(())
}

pub(in crate::world) fn set_player_secondary_stat_update_values(
    values: &mut [Option<u32>],
    combat_stats: &PlayerCombatStats,
) -> anyhow::Result<()> {
    set_update_value(values, PLAYER_CHARACTER_POINTS1, 0)?;
    set_update_value(values, PLAYER_CHARACTER_POINTS2, 2)?;
    set_update_value(values, PLAYER_TRACK_CREATURES, 0)?;
    set_update_value(values, PLAYER_TRACK_RESOURCES, 0)?;
    set_update_value(
        values,
        PLAYER_BLOCK_PERCENTAGE,
        combat_stats.block_percent.to_bits(),
    )?;
    set_update_value(
        values,
        PLAYER_DODGE_PERCENTAGE,
        combat_stats.dodge_percent.to_bits(),
    )?;
    set_update_value(
        values,
        PLAYER_PARRY_PERCENTAGE,
        combat_stats.parry_percent.to_bits(),
    )?;
    set_update_value(
        values,
        PLAYER_CRIT_PERCENTAGE,
        combat_stats.crit_percent.to_bits(),
    )?;
    set_update_value(
        values,
        PLAYER_RANGED_CRIT_PERCENTAGE,
        combat_stats.ranged_crit_percent.to_bits(),
    )?;
    set_update_value(values, PLAYER_REST_STATE_EXPERIENCE, 0)?;

    Ok(())
}

pub(in crate::world) fn set_player_explored_zone_update_values(
    values: &mut [Option<u32>],
    character: &CharacterEnumEntry,
) -> anyhow::Result<()> {
    let explored_zones = parse_explored_zones(character.explored_zones.as_deref());
    for (offset, explored_zone) in explored_zones.iter().enumerate() {
        set_update_value(values, PLAYER_EXPLORED_ZONES_1 + offset, *explored_zone)?;
    }

    Ok(())
}

pub(in crate::world) fn parse_explored_zones(
    explored_zones: Option<&str>,
) -> [u32; PLAYER_EXPLORED_ZONES_SIZE] {
    let mut fields = [0u32; PLAYER_EXPLORED_ZONES_SIZE];
    let Some(explored_zones) = explored_zones else {
        return fields;
    };

    for (index, value) in explored_zones
        .split_whitespace()
        .take(PLAYER_EXPLORED_ZONES_SIZE)
        .enumerate()
    {
        if let Ok(value) = value.parse::<u32>() {
            fields[index] = value;
        }
    }

    fields
}

pub(in crate::world) fn format_explored_zones(
    explored_zones: &[u32; PLAYER_EXPLORED_ZONES_SIZE],
) -> String {
    let mut output = String::new();
    for (index, value) in explored_zones.iter().enumerate() {
        if index > 0 {
            output.push(' ');
        }
        output.push_str(&value.to_string());
    }
    output
}

pub(in crate::world) fn build_player_explored_zone_update_body(
    character_guid: u32,
    offset: usize,
    field_value: u32,
) -> anyhow::Result<Vec<u8>> {
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player_guid)?;

    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, PLAYER_EXPLORED_ZONES_1 + offset, field_value)?;
    write_update_values(&mut block, &values)?;

    Ok(build_update_object_body(&[block]))
}

pub(in crate::world) fn set_player_stat_mod_update_values(
    values: &mut [Option<u32>],
    base_world_stats: &PlayerWorldStats,
    effective_world_stats: &PlayerWorldStats,
) -> anyhow::Result<()> {
    let stat_deltas = player_stat_mod_deltas(base_world_stats, effective_world_stats);
    for (offset, delta) in stat_deltas.iter().copied().enumerate() {
        set_update_value(values, PLAYER_FIELD_POSSTAT0 + offset, delta.max(0) as u32)?;
        set_update_value(
            values,
            PLAYER_FIELD_NEGSTAT0 + offset,
            delta.min(0).unsigned_abs(),
        )?;
    }
    Ok(())
}

pub(in crate::world) fn set_player_resistance_buff_mod_update_values(
    values: &mut [Option<u32>],
    combat_stats: &PlayerCombatStats,
) -> anyhow::Result<()> {
    for offset in 0..MAX_SPELL_SCHOOL {
        set_update_value(
            values,
            PLAYER_FIELD_RESISTANCEBUFFMODSPOSITIVE + offset,
            combat_stats.resistance_buff_mod_positive[offset] as u32,
        )?;
        set_update_value(
            values,
            PLAYER_FIELD_RESISTANCEBUFFMODSNEGATIVE + offset,
            combat_stats.resistance_buff_mod_negative[offset].unsigned_abs(),
        )?;
    }

    Ok(())
}

pub(in crate::world) fn set_player_damage_mod_update_values(
    values: &mut [Option<u32>],
) -> anyhow::Result<()> {
    for index in
        PLAYER_FIELD_MOD_DAMAGE_DONE_POS..PLAYER_FIELD_MOD_DAMAGE_DONE_POS + MAX_SPELL_SCHOOL
    {
        set_update_value(values, index, 0)?;
    }
    for index in
        PLAYER_FIELD_MOD_DAMAGE_DONE_NEG..PLAYER_FIELD_MOD_DAMAGE_DONE_NEG + MAX_SPELL_SCHOOL
    {
        set_update_value(values, index, 0)?;
    }
    for index in
        PLAYER_FIELD_MOD_DAMAGE_DONE_PCT..PLAYER_FIELD_MOD_DAMAGE_DONE_PCT + MAX_SPELL_SCHOOL
    {
        set_update_value(values, index, 1.0f32.to_bits())?;
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub(in crate::world) struct EquippedItemTemplate {
    pub(in crate::world) slot: u8,
    pub(in crate::world) template: ItemTemplateQuery,
    pub(in crate::world) enchantment_stat_bonuses: [i32; ITEM_MOD_STAT_FIELD_COUNT],
    pub(in crate::world) enchantment_resistance_bonuses: [i32; MAX_SPELL_SCHOOL],
}

pub(in crate::world) const ITEM_MOD_STAT_FIELD_COUNT: usize = 8;
pub(in crate::world) const ITEM_MOD_MANA: u32 = 0;
pub(in crate::world) const ITEM_MOD_HEALTH: u32 = 1;
pub(in crate::world) const ITEM_MOD_AGILITY: u32 = 3;
pub(in crate::world) const ITEM_MOD_STRENGTH: u32 = 4;
pub(in crate::world) const ITEM_MOD_INTELLECT: u32 = 5;
pub(in crate::world) const ITEM_MOD_SPIRIT: u32 = 6;
pub(in crate::world) const ITEM_MOD_STAMINA: u32 = 7;
const ITEM_ENCHANTMENT_TYPE_RESISTANCE: u32 = 4;
const ITEM_ENCHANTMENT_TYPE_STAT: u32 = 5;

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct PlayerCombatStats {
    pub(in crate::world) intellect: u32,
    pub(in crate::world) armor: u32,
    pub(in crate::world) shield_block_value: u32,
    pub(in crate::world) resistances: [u32; MAX_SPELL_SCHOOL],
    pub(in crate::world) resistance_buff_mod_positive: [i32; MAX_SPELL_SCHOOL],
    pub(in crate::world) resistance_buff_mod_negative: [i32; MAX_SPELL_SCHOOL],
    pub(in crate::world) main_attack_time_ms: u32,
    pub(in crate::world) off_attack_time_ms: u32,
    pub(in crate::world) ranged_attack_time_ms: u32,
    pub(in crate::world) melee_attack_power: u32,
    pub(in crate::world) ranged_attack_power: u32,
    pub(in crate::world) melee_attack_power_mod_positive: i16,
    pub(in crate::world) melee_attack_power_mod_negative: i16,
    pub(in crate::world) ranged_attack_power_mod_positive: i16,
    pub(in crate::world) ranged_attack_power_mod_negative: i16,
    pub(in crate::world) main_min_damage: f32,
    pub(in crate::world) main_max_damage: f32,
    pub(in crate::world) off_min_damage: f32,
    pub(in crate::world) off_max_damage: f32,
    pub(in crate::world) ranged_min_damage: f32,
    pub(in crate::world) ranged_max_damage: f32,
    pub(in crate::world) block_percent: f32,
    pub(in crate::world) dodge_percent: f32,
    pub(in crate::world) parry_percent: f32,
    pub(in crate::world) crit_percent: f32,
    pub(in crate::world) ranged_crit_percent: f32,
}

pub(in crate::world) fn player_combat_stats_for_values(
    class: u8,
    level: u8,
    world_stats: &PlayerWorldStats,
    equipped_templates: &[EquippedItemTemplate],
) -> PlayerCombatStats {
    player_combat_stats_for_values_with_ammo(class, level, world_stats, equipped_templates, None)
}

pub(in crate::world) fn player_combat_stats_for_values_with_ammo(
    class: u8,
    level: u8,
    world_stats: &PlayerWorldStats,
    equipped_templates: &[EquippedItemTemplate],
    ammo_template: Option<&ItemTemplateQuery>,
) -> PlayerCombatStats {
    player_combat_stats_for_values_with_known_spells_and_ammo(
        class,
        level,
        world_stats,
        &[],
        &HashSet::new(),
        equipped_templates,
        ammo_template,
    )
}

pub(in crate::world) const SPELL_PASSIVE_PARRY: u32 = 3127;

pub(in crate::world) fn character_can_parry(active_spells: &HashSet<u32>) -> bool {
    active_spells.contains(&SPELL_PASSIVE_PARRY)
}

pub(in crate::world) fn player_parry_percent(
    level: u8,
    skills: &[CharacterSkill],
    active_spells: &HashSet<u32>,
) -> f32 {
    if !character_can_parry(active_spells) {
        return 0.0;
    }
    let defense = skills
        .iter()
        .find(|skill| skill.skill == SKILL_DEFENSE)
        .map(|skill| skill.value)
        .unwrap_or(u16::from(level).saturating_mul(5));
    let max_for_level = u16::from(level).saturating_mul(5);
    (5.0 + (i32::from(defense) - i32::from(max_for_level)) as f32 * 0.04).clamp(0.0, 100.0)
}

pub(in crate::world) fn player_combat_stats_for_values_with_known_spells_and_ammo(
    class: u8,
    level: u8,
    world_stats: &PlayerWorldStats,
    skills: &[CharacterSkill],
    active_spells: &HashSet<u32>,
    equipped_templates: &[EquippedItemTemplate],
    ammo_template: Option<&ItemTemplateQuery>,
) -> PlayerCombatStats {
    let strength = world_stats.stats[0];
    let agility = world_stats.stats[1];
    let intellect = world_stats.stats[3];
    let level = level as u32;
    let melee_attack_power = class_melee_attack_power(class, level, strength, agility);
    let ranged_attack_power = class_ranged_attack_power(class, level, agility);

    let main_weapon = equipped_weapon_template(equipped_templates, EQUIPMENT_SLOT_MAINHAND);
    let off_weapon = equipped_weapon_template(equipped_templates, EQUIPMENT_SLOT_OFFHAND);
    let ranged_weapon = equipped_weapon_template(equipped_templates, EQUIPMENT_SLOT_RANGED);
    let main_attack_time_ms = main_weapon
        .map(|template| template.delay.max(1))
        .unwrap_or(BASE_ATTACK_TIME_MS);
    let off_attack_time_ms = off_weapon
        .map(|template| template.delay.max(1))
        .unwrap_or(BASE_ATTACK_TIME_MS);
    let ranged_attack_time_ms = ranged_weapon
        .map(|template| template.delay.max(1))
        .unwrap_or(BASE_ATTACK_TIME_MS);

    let (main_min_damage, main_max_damage) =
        main_hand_damage_with_attack_power(main_weapon, melee_attack_power, main_attack_time_ms);
    let (off_min_damage, off_max_damage) =
        weapon_damage_with_attack_power(off_weapon, melee_attack_power, off_attack_time_ms);
    let (ranged_min_damage, ranged_max_damage) = ranged_weapon_damage_with_attack_power_and_ammo(
        ranged_weapon,
        ammo_template,
        ranged_attack_power,
        ranged_attack_time_ms,
    );

    PlayerCombatStats {
        intellect,
        armor: player_armor(world_stats, equipped_templates),
        shield_block_value: player_shield_block_value(world_stats, equipped_templates),
        resistances: equipment_resistances(world_stats, equipped_templates),
        resistance_buff_mod_positive: [0; MAX_SPELL_SCHOOL],
        resistance_buff_mod_negative: [0; MAX_SPELL_SCHOOL],
        main_attack_time_ms,
        off_attack_time_ms,
        ranged_attack_time_ms,
        melee_attack_power,
        ranged_attack_power,
        melee_attack_power_mod_positive: 0,
        melee_attack_power_mod_negative: 0,
        ranged_attack_power_mod_positive: 0,
        ranged_attack_power_mod_negative: 0,
        main_min_damage,
        main_max_damage,
        off_min_damage,
        off_max_damage,
        ranged_min_damage,
        ranged_max_damage,
        block_percent: if has_equipped_shield(equipped_templates) {
            5.0
        } else {
            0.0
        },
        dodge_percent: dodge_percent(class, level as u8, agility),
        parry_percent: player_parry_percent(level as u8, skills, active_spells),
        crit_percent: melee_crit_percent(class, level as u8, agility),
        ranged_crit_percent: melee_crit_percent(class, level as u8, agility),
    }
}

pub(in crate::world) fn player_world_stats_with_equipment(
    mut world_stats: PlayerWorldStats,
    equipped_templates: &[EquippedItemTemplate],
) -> PlayerWorldStats {
    for equipped in equipped_templates {
        for stat in equipped.template.stats {
            if stat.stat_value == 0 {
                continue;
            }
            match stat.stat_type {
                ITEM_MOD_MANA => {
                    world_stats.base_mana =
                        apply_signed_item_stat(world_stats.base_mana, stat.stat_value);
                }
                ITEM_MOD_HEALTH => {
                    world_stats.base_health =
                        apply_signed_item_stat(world_stats.base_health, stat.stat_value);
                }
                ITEM_MOD_AGILITY => {
                    world_stats.stats[1] =
                        apply_signed_item_stat(world_stats.stats[1], stat.stat_value);
                }
                ITEM_MOD_STRENGTH => {
                    world_stats.stats[0] =
                        apply_signed_item_stat(world_stats.stats[0], stat.stat_value);
                }
                ITEM_MOD_INTELLECT => {
                    world_stats.stats[3] =
                        apply_signed_item_stat(world_stats.stats[3], stat.stat_value);
                }
                ITEM_MOD_SPIRIT => {
                    world_stats.stats[4] =
                        apply_signed_item_stat(world_stats.stats[4], stat.stat_value);
                }
                ITEM_MOD_STAMINA => {
                    world_stats.stats[2] =
                        apply_signed_item_stat(world_stats.stats[2], stat.stat_value);
                }
                _ => {}
            }
        }
        for (stat_type, stat_value) in equipped.enchantment_stat_bonuses.into_iter().enumerate() {
            apply_item_stat_bonus(&mut world_stats, stat_type as u32, stat_value);
        }
    }
    world_stats
}

pub(in crate::world) fn item_enchantment_bonuses(
    enchantments: &str,
    spell_item_enchantments: &HashMap<u32, SpellItemEnchantmentEntry>,
) -> ([i32; ITEM_MOD_STAT_FIELD_COUNT], [i32; MAX_SPELL_SCHOOL]) {
    let mut stat_bonuses = [0i32; ITEM_MOD_STAT_FIELD_COUNT];
    let mut resistance_bonuses = [0i32; MAX_SPELL_SCHOOL];
    let fields = parse_item_enchantment_fields(enchantments);
    for slot in 0..MAX_ENCHANTMENT_SLOT {
        let enchant_id = fields[slot * MAX_ENCHANTMENT_OFFSET];
        let Some(enchantment) = spell_item_enchantments.get(&enchant_id) else {
            continue;
        };
        for index in 0..3 {
            let amount = enchantment.effect_amounts[index];
            if amount == 0 {
                continue;
            }
            match enchantment.effect_types[index] {
                ITEM_ENCHANTMENT_TYPE_STAT => {
                    let stat_type = enchantment.effect_args[index] as usize;
                    if stat_type < ITEM_MOD_STAT_FIELD_COUNT {
                        stat_bonuses[stat_type] = stat_bonuses[stat_type].saturating_add(amount);
                    }
                }
                ITEM_ENCHANTMENT_TYPE_RESISTANCE => {
                    let school = enchantment.effect_args[index] as usize;
                    if school < MAX_SPELL_SCHOOL {
                        resistance_bonuses[school] =
                            resistance_bonuses[school].saturating_add(amount);
                    }
                }
                _ => {}
            }
        }
    }
    (stat_bonuses, resistance_bonuses)
}

fn apply_item_stat_bonus(world_stats: &mut PlayerWorldStats, stat_type: u32, stat_value: i32) {
    if stat_value == 0 {
        return;
    }
    match stat_type {
        ITEM_MOD_MANA => {
            world_stats.base_mana = apply_signed_item_stat(world_stats.base_mana, stat_value);
        }
        ITEM_MOD_HEALTH => {
            world_stats.base_health = apply_signed_item_stat(world_stats.base_health, stat_value);
        }
        ITEM_MOD_AGILITY => {
            world_stats.stats[1] = apply_signed_item_stat(world_stats.stats[1], stat_value);
        }
        ITEM_MOD_STRENGTH => {
            world_stats.stats[0] = apply_signed_item_stat(world_stats.stats[0], stat_value);
        }
        ITEM_MOD_INTELLECT => {
            world_stats.stats[3] = apply_signed_item_stat(world_stats.stats[3], stat_value);
        }
        ITEM_MOD_SPIRIT => {
            world_stats.stats[4] = apply_signed_item_stat(world_stats.stats[4], stat_value);
        }
        ITEM_MOD_STAMINA => {
            world_stats.stats[2] = apply_signed_item_stat(world_stats.stats[2], stat_value);
        }
        _ => {}
    }
}

fn apply_signed_item_stat(value: u32, amount: i32) -> u32 {
    (i64::from(value) + i64::from(amount)).clamp(0, i64::from(u32::MAX)) as u32
}

pub(in crate::world) fn build_player_ammo_update_body(
    character_guid: u32,
    ammo_id: u32,
) -> anyhow::Result<Vec<u8>> {
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player_guid)?;

    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, PLAYER_AMMO_ID, ammo_id)?;
    write_update_values(&mut block, &values)?;

    Ok(build_update_object_body(&[block]))
}

pub(in crate::world) fn build_player_combat_stats_update_body(
    character_guid: u32,
    combat_stats: &PlayerCombatStats,
) -> anyhow::Result<Vec<u8>> {
    build_player_combat_stats_update_body_with_flags(character_guid, combat_stats, None)
}

pub(in crate::world) fn build_player_combat_stats_update_body_with_flags(
    character_guid: u32,
    combat_stats: &PlayerCombatStats,
    unit_flags: Option<u32>,
) -> anyhow::Result<Vec<u8>> {
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player_guid)?;

    let mut values = vec![None; PLAYER_END_FIELDS];
    if let Some(unit_flags) = unit_flags {
        set_update_value(&mut values, UNIT_FIELD_FLAGS, unit_flags)?;
    }
    set_update_value(
        &mut values,
        UNIT_FIELD_BASEATTACKTIME,
        combat_stats.main_attack_time_ms,
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_BASEATTACKTIME + 1,
        combat_stats.off_attack_time_ms,
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_RANGEDATTACKTIME,
        combat_stats.ranged_attack_time_ms,
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_MINDAMAGE,
        combat_stats.main_min_damage.to_bits(),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_MAXDAMAGE,
        combat_stats.main_max_damage.to_bits(),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_MINOFFHANDDAMAGE,
        combat_stats.off_min_damage.to_bits(),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_MAXOFFHANDDAMAGE,
        combat_stats.off_max_damage.to_bits(),
    )?;
    set_player_resistance_update_values(&mut values, combat_stats)?;
    set_update_value(
        &mut values,
        UNIT_FIELD_ATTACK_POWER,
        combat_stats.melee_attack_power,
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_ATTACK_POWER_MODS,
        attack_power_mod_pair(
            combat_stats.melee_attack_power_mod_positive,
            combat_stats.melee_attack_power_mod_negative,
        ),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_ATTACK_POWER_MULTIPLIER,
        0.0f32.to_bits(),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_RANGED_ATTACK_POWER,
        combat_stats.ranged_attack_power,
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_RANGED_ATTACK_POWER_MODS,
        attack_power_mod_pair(
            combat_stats.ranged_attack_power_mod_positive,
            combat_stats.ranged_attack_power_mod_negative,
        ),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_RANGED_ATTACK_POWER_MULTIPLIER,
        0.0f32.to_bits(),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_MINRANGEDDAMAGE,
        combat_stats.ranged_min_damage.to_bits(),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_MAXRANGEDDAMAGE,
        combat_stats.ranged_max_damage.to_bits(),
    )?;
    set_player_secondary_stat_update_values(&mut values, combat_stats)?;
    set_player_resistance_buff_mod_update_values(&mut values, combat_stats)?;
    write_update_values(&mut block, &values)?;

    Ok(build_update_object_body(&[block]))
}

pub(in crate::world) fn build_player_world_stats_update_body(
    character_guid: u32,
    base_world_stats: &PlayerWorldStats,
    effective_world_stats: &PlayerWorldStats,
    health: u32,
    power1: u32,
) -> anyhow::Result<Vec<u8>> {
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player_guid)?;

    let max_health = effective_world_stats.max_health().max(1);
    let max_mana = effective_world_stats.max_mana();
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, UNIT_FIELD_HEALTH, health.min(max_health))?;
    set_update_value(&mut values, UNIT_FIELD_POWER1, power1.min(max_mana))?;
    set_update_value(&mut values, UNIT_FIELD_MAXHEALTH, max_health)?;
    set_update_value(&mut values, UNIT_FIELD_MAXPOWER1, max_mana)?;
    set_update_value(
        &mut values,
        UNIT_FIELD_BASE_MANA,
        effective_world_stats.base_mana,
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_BASE_HEALTH,
        effective_world_stats.base_health,
    )?;
    set_player_stat_update_values(&mut values, effective_world_stats)?;
    set_player_stat_mod_update_values(&mut values, base_world_stats, effective_world_stats)?;
    write_update_values(&mut block, &values)?;

    Ok(build_update_object_body(&[block]))
}

pub(in crate::world) fn class_melee_attack_power(
    class: u8,
    level: u32,
    strength: u32,
    agility: u32,
) -> u32 {
    let value = match class {
        1 | 2 => level as i32 * 3 + strength as i32 * 2 - 20,
        3 | 4 => level as i32 * 2 + strength as i32 + agility as i32 - 20,
        7 | 11 => level as i32 * 3 + strength as i32 * 2 - 20,
        8 | 5 | 9 => strength as i32 - 10,
        _ => 0,
    };
    value.max(0) as u32
}

pub(in crate::world) fn class_ranged_attack_power(class: u8, level: u32, agility: u32) -> u32 {
    let value = match class {
        3 => level as i32 * 2 + agility as i32 * 2 - 10,
        1 | 4 => level as i32 + agility as i32 - 10,
        11 => agility as i32 - 10,
        _ => agility as i32 - 10,
    };
    value.max(0) as u32
}

pub(in crate::world) fn equipped_weapon_template(
    equipped_templates: &[EquippedItemTemplate],
    slot: u8,
) -> Option<&ItemTemplateQuery> {
    equipped_templates
        .iter()
        .find(|item| item.slot == slot && item.template.class == ITEM_CLASS_WEAPON)
        .map(|item| &item.template)
}

pub(in crate::world) fn main_hand_damage_with_attack_power(
    weapon: Option<&ItemTemplateQuery>,
    attack_power: u32,
    attack_time_ms: u32,
) -> (f32, f32) {
    melee_damage_with_attack_power(
        weapon,
        attack_power,
        attack_time_ms,
        Some((BASE_UNARMED_MIN_DAMAGE, BASE_UNARMED_MAX_DAMAGE)),
    )
}

pub(in crate::world) fn weapon_damage_with_attack_power(
    weapon: Option<&ItemTemplateQuery>,
    attack_power: u32,
    attack_time_ms: u32,
) -> (f32, f32) {
    melee_damage_with_attack_power(weapon, attack_power, attack_time_ms, None)
}

fn melee_damage_with_attack_power(
    weapon: Option<&ItemTemplateQuery>,
    attack_power: u32,
    attack_time_ms: u32,
    fallback_damage: Option<(f32, f32)>,
) -> (f32, f32) {
    let (base_min_damage, base_max_damage) = match weapon {
        Some(weapon) => (weapon.dmg_min1, weapon.dmg_max1),
        None => {
            let Some(fallback_damage) = fallback_damage else {
                return (0.0, 0.0);
            };
            fallback_damage
        }
    };
    let ap_damage = attack_power as f32 / 14.0 * attack_time_ms as f32 / 1000.0;
    (base_min_damage + ap_damage, base_max_damage + ap_damage)
}

pub(in crate::world) fn ranged_weapon_damage_with_attack_power_and_ammo(
    weapon: Option<&ItemTemplateQuery>,
    ammo: Option<&ItemTemplateQuery>,
    attack_power: u32,
    attack_time_ms: u32,
) -> (f32, f32) {
    let (mut min_damage, mut max_damage) =
        weapon_damage_with_attack_power(weapon, attack_power, attack_time_ms);
    if min_damage == 0.0 && max_damage == 0.0 {
        return (min_damage, max_damage);
    }
    if let (Some(weapon), Some(ammo)) = (weapon, ammo) {
        if ranged_weapon_accepts_ammo(weapon, ammo) {
            let speed = attack_time_ms.max(1) as f32 / 1000.0;
            min_damage += ammo.dmg_min1 * speed;
            max_damage += ammo.dmg_max1 * speed;
        }
    }
    (min_damage, max_damage)
}

pub(in crate::world) fn ranged_weapon_accepts_ammo(
    weapon: &ItemTemplateQuery,
    ammo: &ItemTemplateQuery,
) -> bool {
    if weapon.class != ITEM_CLASS_WEAPON || ammo.class != ITEM_CLASS_PROJECTILE {
        return false;
    }
    matches!(
        (weapon.subclass, ammo.subclass),
        (
            ITEM_SUBCLASS_WEAPON_BOW | ITEM_SUBCLASS_WEAPON_CROSSBOW,
            ITEM_SUBCLASS_ARROW
        ) | (ITEM_SUBCLASS_WEAPON_GUN, ITEM_SUBCLASS_BULLET)
    )
}

pub(in crate::world) fn attack_power_mod_pair(positive: i16, negative: i16) -> u32 {
    make_pair32(positive as u16, negative as u16)
}

pub(in crate::world) fn combat_stats_with_active_auras(
    base_stats: PlayerCombatStats,
    active_auras: &[ActiveAura],
) -> PlayerCombatStats {
    let mut stats = base_stats;
    let intellect_delta = active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .map(|modifier| match modifier {
            AuraStatModifier::Stat {
                stat: Some(3),
                amount,
            }
            | AuraStatModifier::Stat { stat: None, amount } => *amount,
            _ => 0,
        })
        .sum::<i32>();
    stats.intellect = apply_flat_modifier(stats.intellect, intellect_delta);

    let intellect_percent = active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .filter_map(|modifier| match modifier {
            AuraStatModifier::TotalStatPercent { stat: 3, percent } => Some(*percent),
            _ => None,
        })
        .sum::<i32>();
    if intellect_percent != 0 {
        stats.intellect = apply_percent_modifier(stats.intellect, intellect_percent);
    }

    let attack_power_delta = active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .map(|modifier| match modifier {
            AuraStatModifier::AttackPower { amount } => *amount,
            _ => 0,
        })
        .sum::<i32>();
    let block_percent_delta = active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .map(|modifier| match modifier {
            AuraStatModifier::BlockPercent { percent } => *percent,
            _ => 0,
        })
        .sum::<i32>();
    let crit_percent_delta = active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .map(|modifier| match modifier {
            AuraStatModifier::CritPercent { percent } => *percent,
            _ => 0,
        })
        .sum::<i32>();
    let physical_damage_done = active_aura_physical_damage_done(active_auras) as f32;
    let physical_damage_done_multiplier =
        active_aura_damage_done_multiplier(active_auras, SPELL_SCHOOL_MASK_NORMAL);

    for modifier in active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
    {
        let AuraStatModifier::Resistance {
            school_mask,
            amount,
        } = modifier
        else {
            continue;
        };
        apply_resistance_delta(&mut stats, *school_mask, *amount);
    }
    for modifier in active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
    {
        let AuraStatModifier::ResistancePercent {
            school_mask,
            percent,
        } = modifier
        else {
            continue;
        };
        apply_resistance_percent(&mut stats, *school_mask, *percent);
    }

    let melee_attack_time_multiplier = active_aura_melee_attack_time_multiplier(active_auras);
    if (melee_attack_time_multiplier - 1.0).abs() > f32::EPSILON {
        stats.main_attack_time_ms =
            multiply_attack_time(stats.main_attack_time_ms, melee_attack_time_multiplier);
        stats.off_attack_time_ms =
            multiply_attack_time(stats.off_attack_time_ms, melee_attack_time_multiplier);
    }

    if physical_damage_done != 0.0 {
        stats.main_min_damage = (stats.main_min_damage + physical_damage_done).max(0.0);
        stats.main_max_damage = (stats.main_max_damage + physical_damage_done).max(0.0);
        stats.off_min_damage = (stats.off_min_damage + physical_damage_done).max(0.0);
        stats.off_max_damage = (stats.off_max_damage + physical_damage_done).max(0.0);
        stats.ranged_min_damage = (stats.ranged_min_damage + physical_damage_done).max(0.0);
        stats.ranged_max_damage = (stats.ranged_max_damage + physical_damage_done).max(0.0);
    }

    stats = apply_attack_power_delta(stats, attack_power_delta, 0);
    if stats.block_percent > 0.0 && block_percent_delta != 0 {
        stats.block_percent = (stats.block_percent + block_percent_delta as f32).clamp(0.0, 100.0);
    }
    if crit_percent_delta != 0 {
        stats.crit_percent = (stats.crit_percent + crit_percent_delta as f32).clamp(0.0, 100.0);
        stats.ranged_crit_percent =
            (stats.ranged_crit_percent + crit_percent_delta as f32).clamp(0.0, 100.0);
    }
    if active_aura_has_disarm(active_auras) {
        stats.main_attack_time_ms = BASE_ATTACK_TIME_MS;
        let (main_min_damage, main_max_damage) = main_hand_damage_with_attack_power(
            None,
            stats.melee_attack_power,
            stats.main_attack_time_ms,
        );
        stats.main_min_damage = main_min_damage;
        stats.main_max_damage = main_max_damage;
    }
    if (physical_damage_done_multiplier - 1.0).abs() > f32::EPSILON {
        stats.main_min_damage = (stats.main_min_damage * physical_damage_done_multiplier).max(0.0);
        stats.main_max_damage = (stats.main_max_damage * physical_damage_done_multiplier).max(0.0);
        stats.off_min_damage = (stats.off_min_damage * physical_damage_done_multiplier).max(0.0);
        stats.off_max_damage = (stats.off_max_damage * physical_damage_done_multiplier).max(0.0);
        stats.ranged_min_damage =
            (stats.ranged_min_damage * physical_damage_done_multiplier).max(0.0);
        stats.ranged_max_damage =
            (stats.ranged_max_damage * physical_damage_done_multiplier).max(0.0);
    }

    stats
}

pub(in crate::world) fn multiply_attack_time(attack_time_ms: u32, multiplier: f32) -> u32 {
    ((attack_time_ms.max(1) as f32 * multiplier).round() as u32).max(1)
}

pub(in crate::world) fn apply_resistance_delta(
    stats: &mut PlayerCombatStats,
    school_mask: u32,
    amount: i32,
) {
    for school in 0..MAX_SPELL_SCHOOL {
        if school_mask & (1u32 << school) == 0 {
            continue;
        }
        let adjusted = (i64::from(stats.resistances[school]) + i64::from(amount))
            .clamp(0, u32::MAX as i64) as u32;
        stats.resistances[school] = adjusted;
        if amount >= 0 {
            stats.resistance_buff_mod_positive[school] =
                stats.resistance_buff_mod_positive[school].saturating_add(amount);
        } else {
            stats.resistance_buff_mod_negative[school] =
                stats.resistance_buff_mod_negative[school].saturating_add(amount);
        }
    }
    stats.armor = stats.resistances[0];
}

pub(in crate::world) fn apply_resistance_percent(
    stats: &mut PlayerCombatStats,
    school_mask: u32,
    percent: i32,
) {
    for school in 0..MAX_SPELL_SCHOOL {
        if school_mask & (1u32 << school) == 0 {
            continue;
        }
        stats.resistances[school] = apply_percent_modifier(stats.resistances[school], percent);
    }
    stats.armor = stats.resistances[0];
}

pub(in crate::world) fn apply_attack_power_delta(
    mut stats: PlayerCombatStats,
    melee_delta: i32,
    ranged_delta: i32,
) -> PlayerCombatStats {
    stats.melee_attack_power_mod_positive = melee_delta.max(0).min(i16::MAX as i32) as i16;
    stats.melee_attack_power_mod_negative = melee_delta.min(0).max(i16::MIN as i32) as i16;
    stats.ranged_attack_power_mod_positive = ranged_delta.max(0).min(i16::MAX as i32) as i16;
    stats.ranged_attack_power_mod_negative = ranged_delta.min(0).max(i16::MIN as i32) as i16;

    let melee_effective_delta = effective_attack_power_delta(stats.melee_attack_power, melee_delta);
    let ranged_effective_delta =
        effective_attack_power_delta(stats.ranged_attack_power, ranged_delta);
    let main_damage_delta = attack_power_damage_delta(
        melee_effective_delta,
        stats.main_attack_time_ms,
        stats.main_max_damage > 0.0,
    );
    let off_damage_delta = attack_power_damage_delta(
        melee_effective_delta,
        stats.off_attack_time_ms,
        stats.off_max_damage > 0.0,
    );
    let ranged_damage_delta = attack_power_damage_delta(
        ranged_effective_delta,
        stats.ranged_attack_time_ms,
        stats.ranged_max_damage > 0.0,
    );
    stats.main_min_damage += main_damage_delta;
    stats.main_max_damage += main_damage_delta;
    stats.off_min_damage += off_damage_delta;
    stats.off_max_damage += off_damage_delta;
    stats.ranged_min_damage += ranged_damage_delta;
    stats.ranged_max_damage += ranged_damage_delta;
    stats
}

pub(in crate::world) fn effective_attack_power_delta(base_attack_power: u32, delta: i32) -> i32 {
    let effective = (base_attack_power as i32 + delta).max(0);
    effective - base_attack_power as i32
}

pub(in crate::world) fn attack_power_damage_delta(
    delta: i32,
    attack_time_ms: u32,
    has_weapon_damage: bool,
) -> f32 {
    if !has_weapon_damage || delta == 0 {
        return 0.0;
    }
    delta as f32 / 14.0 * attack_time_ms as f32 / 1000.0
}

pub(in crate::world) fn equipment_resistances(
    world_stats: &PlayerWorldStats,
    equipped_templates: &[EquippedItemTemplate],
) -> [u32; MAX_SPELL_SCHOOL] {
    let mut resistances = [0u32; MAX_SPELL_SCHOOL];
    resistances[0] = player_armor(world_stats, equipped_templates);

    for equipped in equipped_templates {
        resistances[1] += equipped.template.holy_res;
        resistances[2] += equipped.template.fire_res;
        resistances[3] += equipped.template.nature_res;
        resistances[4] += equipped.template.frost_res;
        resistances[5] += equipped.template.shadow_res;
        resistances[6] += equipped.template.arcane_res;
        for (school, amount) in equipped
            .enchantment_resistance_bonuses
            .into_iter()
            .enumerate()
        {
            resistances[school] = apply_signed_item_stat(resistances[school], amount);
        }
    }

    resistances
}

pub(in crate::world) fn player_armor(
    world_stats: &PlayerWorldStats,
    equipped_templates: &[EquippedItemTemplate],
) -> u32 {
    let mut armor = world_stats.stats[1] * 2;
    for equipped in equipped_templates {
        armor += equipped.template.armor;
    }
    armor
}

pub(in crate::world) fn equipped_shield_block_value(
    equipped_templates: &[EquippedItemTemplate],
) -> u32 {
    equipped_templates
        .iter()
        .find(|equipped| {
            equipped.slot == EQUIPMENT_SLOT_OFFHAND
                && equipped.template.class == ITEM_CLASS_ARMOR
                && equipped.template.inventory_type == INVTYPE_SHIELD
        })
        .map(|equipped| equipped.template.block)
        .unwrap_or(0)
}

pub(in crate::world) fn player_shield_block_value(
    world_stats: &PlayerWorldStats,
    equipped_templates: &[EquippedItemTemplate],
) -> u32 {
    let shield_block = equipped_shield_block_value(equipped_templates);
    if shield_block == 0 {
        return 0;
    }
    shield_block
        .saturating_add(world_stats.stats[0] / 20)
        .saturating_sub(1)
}

pub(in crate::world) fn has_equipped_shield(equipped_templates: &[EquippedItemTemplate]) -> bool {
    equipped_templates.iter().any(|equipped| {
        equipped.slot == EQUIPMENT_SLOT_OFFHAND
            && equipped.template.class == ITEM_CLASS_ARMOR
            && equipped.template.inventory_type == INVTYPE_SHIELD
    })
}

pub(in crate::world) fn melee_crit_percent(class: u8, level: u8, agility: u32) -> f32 {
    agility as f32 / agility_rating_for_class(class, level, false).unwrap_or(f32::INFINITY)
}

pub(in crate::world) fn dodge_percent(class: u8, level: u8, agility: u32) -> f32 {
    let base = match class {
        2 => 0.75,
        3 => 0.64,
        5 => 3.0,
        7 => 1.75,
        8 => 3.25,
        9 => 2.0,
        11 => 0.75,
        _ => 0.0,
    };
    base + agility as f32 / agility_rating_for_class(class, level, true).unwrap_or(f32::INFINITY)
}

pub(in crate::world) fn agility_rating_for_class(class: u8, level: u8, dodge: bool) -> Option<f32> {
    let (level_one, level_sixty) = match (class, dodge) {
        (2 | 7 | 11, _) => (4.6, 20.0),
        (8, _) => (12.9, 20.0),
        (4, false) => (2.2, 29.0),
        (4, true) => (1.1, 14.5),
        (3, false) => (3.5, 53.0),
        (3, true) => (1.8, 26.5),
        (5, _) => (11.0, 20.0),
        (9, _) => (8.4, 20.0),
        (1, _) => (3.9, 20.0),
        _ => return None,
    };
    let level = level as f32;
    Some(level_one * (60.0 - level) / 59.0 + level_sixty * (level - 1.0) / 59.0)
}

// CMaNGOS reference: src/game/Entities/Player.* packed player identity fields.
pub(in crate::world) fn unit_bytes_0(character: &CharacterEnumEntry) -> u32 {
    let power_type = match character.class {
        1 => 1, // warrior rage
        4 => 3, // rogue energy
        _ => 0, // mana
    };
    character.race as u32
        | ((character.class as u32) << 8)
        | ((character.gender as u32) << 16)
        | (power_type << 24)
}

pub(in crate::world) fn unit_bytes_1(character: &CharacterEnumEntry) -> u32 {
    unit_bytes_1_for_class(character.class)
}

pub(in crate::world) fn unit_bytes_1_for_class(class: u8) -> u32 {
    let pet_loyalty = match class {
        1 | 8 => 0xEE, // CMaNGOS initializes this for rage and mana users.
        _ => 0,
    };
    let shapeshift_form = match class {
        1 => FORM_BATTLESTANCE,
        _ => 0,
    };

    ((pet_loyalty as u32) << 8) | ((shapeshift_form as u32) << 16)
}

pub(in crate::world) fn player_unit_bytes_1_with_auras(
    class: u8,
    stand_state: u8,
    active_auras: &[ActiveAura],
) -> u32 {
    let base = unit_bytes_1_for_class(class);
    let shapeshift_form =
        active_aura_shapeshift_form(active_auras).unwrap_or(((base >> 16) & 0xFF) as u8);
    let vis_flags = active_aura_unit_vis_flags(active_auras);
    u32::from(stand_state)
        | (base & 0x0000_FF00)
        | (u32::from(shapeshift_form) << 16)
        | (vis_flags << 24)
}

pub(in crate::world) fn unit_bytes_2() -> u32 {
    (0x08 | 0x20) << 8
}

pub(in crate::world) fn create_power_for_class_power(class: u8, power: u8) -> u32 {
    match (class, power) {
        (_, POWER_MANA) => 0,
        (1, POWER_RAGE) => POWER_RAGE_DEFAULT,
        (4, POWER_ENERGY) => POWER_ENERGY_DEFAULT,
        _ => 0,
    }
}

// CMaNGOS reference: src/game/Entities/Player.* race display and faction helpers.
pub(in crate::world) fn faction_for_race(race: u8) -> u32 {
    match race {
        1 | 3 | 4 | 7 => 1,
        2 | 5 | 6 | 8 => 2,
        _ => 1,
    }
}

pub(in crate::world) fn player_faction_template(race: u8, player_flags: u32) -> u32 {
    if player_flags & PLAYER_FLAGS_GM != 0 {
        GM_FRIENDLY_FACTION_TEMPLATE
    } else {
        faction_for_race(race)
    }
}

pub(in crate::world) fn display_id_for_character(character: &CharacterEnumEntry) -> u32 {
    match (character.race, character.gender) {
        (1, 0) => 49,
        (1, 1) => 50,
        (2, 0) => 51,
        (2, 1) => 52,
        (3, 0) => 53,
        (3, 1) => 54,
        (4, 0) => 55,
        (4, 1) => 56,
        (5, 0) => 57,
        (5, 1) => 58,
        (6, 0) => 59,
        (6, 1) => 60,
        (7, 0) => 1563,
        (7, 1) => 1564,
        (8, 0) => 1478,
        (8, 1) => 1479,
        _ => 49,
    }
}
