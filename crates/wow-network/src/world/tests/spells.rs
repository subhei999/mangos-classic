fn test_spell_template(spell_id: u32) -> wow_db::SpellTemplateQuery {
    wow_db::SpellTemplateQuery {
        id: spell_id,
        spell_name: format!("Spell {spell_id}"),
        rank: None,
        school: 0,
        dispel: 0,
        mechanic: 0,
        attributes: 0,
        attributes_ex: 0,
        attributes_ex2: 0,
        attributes_ex3: 0,
        attributes_serverside: 0,
        interrupt_flags: 0,
        aura_interrupt_flags: 0,
        channel_interrupt_flags: 0,
        casting_time_index: 0,
        range_index: 0,
        speed: 0.0,
        recovery_time: 0,
        category: 0,
        category_recovery_time: 0,
        start_recovery_category: 0,
        start_recovery_time: 0,
        max_level: 0,
        base_level: 0,
        spell_level: 0,
        power_type: 0,
        mana_cost: 0,
        mana_cost_per_level: 0,
        duration_index: 0,
        stack_amount: 0,
        effect1: 0,
        effect2: 0,
        effect3: 0,
        effect_base_points1: 0,
        effect_base_points2: 0,
        effect_base_points3: 0,
        effect_die_sides1: 0,
        effect_die_sides2: 0,
        effect_die_sides3: 0,
        effect_base_dice1: 1,
        effect_base_dice2: 1,
        effect_base_dice3: 1,
        effect_dice_per_level1: 0.0,
        effect_dice_per_level2: 0.0,
        effect_dice_per_level3: 0.0,
        effect_real_points_per_level1: 0.0,
        effect_real_points_per_level2: 0.0,
        effect_real_points_per_level3: 0.0,
        effect_points_per_combo_point1: 0.0,
        effect_points_per_combo_point2: 0.0,
        effect_points_per_combo_point3: 0.0,
        effect_multiple_value1: 0.0,
        effect_multiple_value2: 0.0,
        effect_multiple_value3: 0.0,
        effect_misc_value1: 0,
        effect_misc_value2: 0,
        effect_misc_value3: 0,
        effect_trigger_spell1: 0,
        effect_trigger_spell2: 0,
        effect_trigger_spell3: 0,
        effect_apply_aura_name1: 0,
        effect_apply_aura_name2: 0,
        effect_apply_aura_name3: 0,
        effect_amplitude1: 0,
        effect_amplitude2: 0,
        effect_amplitude3: 0,
        effect_mechanic1: 0,
        effect_mechanic2: 0,
        effect_mechanic3: 0,
        effect_implicit_target_a1: 0,
        effect_implicit_target_a2: 0,
        effect_implicit_target_a3: 0,
        effect_implicit_target_b1: 0,
        effect_implicit_target_b2: 0,
        effect_implicit_target_b3: 0,
        effect_radius_index1: 0,
        effect_radius_index2: 0,
        effect_radius_index3: 0,
        effect_item_type1: 0,
        effect_item_type2: 0,
        effect_item_type3: 0,
        equipped_item_class: -1,
        equipped_item_subclass_mask: 0,
        spell_family_name: 0,
        spell_family_flags: 0,
        dmg_class: 0,
        proc_flags: 0,
        proc_chance: 0,
        proc_charges: 0,
    }
}

fn test_spell_effect_value_context(
    template: &wow_db::SpellTemplateQuery,
) -> SpellEffectValueContext {
    SpellEffectValueContext::unranked(template, 0)
}

const TEST_SPELL_ATTR_EX3_ALWAYS_HIT: u32 = 0x0004_0000;
const TEST_SPELL_ATTR_EX2_CANT_CRIT: u32 = 0x2000_0000;

fn heroic_strike_spell_template() -> wow_db::SpellTemplateQuery {
    let mut template = test_spell_template(WARRIOR_HEROIC_STRIKE_RANK_1);
    template.spell_name = "Heroic Strike".to_string();
    template.rank = Some("Rank 1".to_string());
    template.attributes = 327700;
    template.attributes_ex = 134217728;
    template.attributes_ex3 = 1024;
    template.power_type = 1;
    template.mana_cost = 150;
    template.effect1 = 17;
    template.effect_base_points1 = 10;
    template.spell_family_name = 4;
    template.spell_family_flags = 64;
    template.dmg_class = 2;
    template
}

#[test]
fn heroic_strike_queue_requires_main_hand_weapon_at_swing_resolution() {
    let heroic = heroic_strike_spell_template();
    let mut no_requirement = heroic.clone();
    no_requirement.attributes_ex3 &= !SPELL_ATTR_EX3_REQUIRES_MAIN_HAND_WEAPON;

    assert!(queued_spell_requires_main_hand_weapon(&heroic));
    assert!(!queued_spell_requires_main_hand_weapon(&no_requirement));
    assert_eq!(SPELL_FAILED_EQUIPPED_ITEM_CLASS_MAINHAND, 0x1A);
}

fn raptor_strike_spell_template() -> wow_db::SpellTemplateQuery {
    let mut template = test_spell_template(HUNTER_RAPTOR_STRIKE_RANK_1);
    template.spell_name = "Raptor Strike".to_string();
    template.rank = Some("Rank 1".to_string());
    template.attributes = 328708;
    template.attributes_ex3 = 1024;
    template.category_recovery_time = 6000;
    template.power_type = 0;
    template.mana_cost = 15;
    template.effect1 = 58;
    template.effect_base_points1 = 4;
    template.spell_family_name = 9;
    template.spell_family_flags = 2;
    template.dmg_class = 2;
    template
}

fn auto_shot_spell_template() -> wow_db::SpellTemplateQuery {
    let mut template = test_spell_template(75);
    template.spell_name = "Auto Shot".to_string();
    template.attributes = SPELL_ATTR_USES_RANGED_SLOT;
    template.attributes_ex2 = SPELL_ATTR_EX2_AUTO_REPEAT;
    template.range_index = 114;
    template.speed = 40.0;
    template
}

fn instant_melee_damage_spell_template() -> wow_db::SpellTemplateQuery {
    let mut template = test_spell_template(999_010);
    template.spell_name = "Instant Melee".to_string();
    template.effect1 = SPELL_EFFECT_WEAPON_DAMAGE_NOSCHOOL;
    template.effect_base_points1 = 4;
    template.dmg_class = 2;
    template
}

fn lesser_heal_spell_template() -> wow_db::SpellTemplateQuery {
    let mut template = test_spell_template(2050);
    template.spell_name = "Lesser Heal".to_string();
    template.casting_time_index = 7;
    template.power_type = POWER_TYPE_MANA;
    template.mana_cost = 25;
    template.interrupt_flags = SPELL_INTERRUPT_FLAG_DAMAGE_PUSHBACK;
    template.start_recovery_category = 133;
    template.start_recovery_time = 1_500;
    template.effect1 = SPELL_EFFECT_HEAL;
    template.effect_base_points1 = 19;
    template.effect_implicit_target_a1 = TARGET_UNIT_CASTER;
    template
}

fn sinister_strike_spell_template() -> wow_db::SpellTemplateQuery {
    let mut template = test_spell_template(1752);
    template.spell_name = "Sinister Strike".to_string();
    template.power_type = POWER_TYPE_ENERGY;
    template.mana_cost = 45;
    template.start_recovery_category = 133;
    template.start_recovery_time = 1_500;
    template.effect1 = SPELL_EFFECT_NORMALIZED_WEAPON_DMG;
    template.effect_base_points1 = 2;
    template.effect_implicit_target_a1 = TARGET_UNIT_ENEMY;
    template.effect2 = SPELL_EFFECT_ADD_COMBO_POINTS;
    template.effect_base_points2 = 0;
    template.effect_implicit_target_a2 = TARGET_UNIT_ENEMY;
    template.spell_family_name = 8;
    template.spell_family_flags = 1;
    template.dmg_class = 2;
    template
}

fn backstab_spell_template() -> wow_db::SpellTemplateQuery {
    let mut template = test_spell_template(53);
    template.spell_name = "Backstab".to_string();
    template.power_type = POWER_TYPE_ENERGY;
    template.mana_cost = 60;
    template.start_recovery_category = 133;
    template.start_recovery_time = 1_500;
    template.attributes_serverside = SPELL_ATTR_SS_FACING_BACK;
    template.effect1 = SPELL_EFFECT_NORMALIZED_WEAPON_DMG;
    template.effect_base_points1 = 9;
    template.effect_implicit_target_a1 = TARGET_UNIT_ENEMY;
    template.effect2 = SPELL_EFFECT_WEAPON_PERCENT_DAMAGE;
    template.effect_base_points2 = 149;
    template.effect_implicit_target_a2 = TARGET_UNIT_ENEMY;
    template.effect3 = SPELL_EFFECT_ADD_COMBO_POINTS;
    template.effect_base_points3 = 0;
    template.effect_implicit_target_a3 = TARGET_UNIT_ENEMY;
    template.spell_family_name = 8;
    template.spell_family_flags = 4;
    template.dmg_class = 2;
    template
}

fn eviscerate_spell_template() -> wow_db::SpellTemplateQuery {
    let mut template = test_spell_template(2098);
    template.spell_name = "Eviscerate".to_string();
    template.attributes = 327696;
    template.attributes_ex = SPELL_ATTR_EX_FINISHING_MOVE_DAMAGE;
    template.power_type = POWER_TYPE_ENERGY;
    template.mana_cost = 35;
    template.start_recovery_category = 133;
    template.start_recovery_time = 1_000;
    template.effect1 = 2;
    template.effect_base_points1 = 0;
    template.effect_die_sides1 = 5;
    template.effect_base_dice1 = 1;
    template.effect_points_per_combo_point1 = 5.0;
    template.effect_implicit_target_a1 = TARGET_UNIT_ENEMY;
    template.effect2 = 3;
    template.effect_base_points2 = 0;
    template.effect_implicit_target_a2 = TARGET_UNIT_CASTER;
    template.spell_family_name = 8;
    template.spell_family_flags = 8_519_680;
    template.dmg_class = 2;
    template
}

fn fireball_spell_template() -> wow_db::SpellTemplateQuery {
    let mut template = test_spell_template(133);
    template.spell_name = "Fireball".to_string();
    template.school = 4;
    template.casting_time_index = 7;
    template.power_type = POWER_TYPE_MANA;
    template.mana_cost = 25;
    template.start_recovery_category = 133;
    template.start_recovery_time = 1_500;
    template.effect1 = 2;
    template.effect_base_points1 = 13;
    template.effect_implicit_target_a1 = TARGET_UNIT_ENEMY;
    template
}

fn frost_armor_spell_template() -> wow_db::SpellTemplateQuery {
    let mut template = test_spell_template(12544);
    template.spell_name = "Frost Armor".to_string();
    template.school = 16;
    template.duration_index = 21;
    template.effect1 = SPELL_EFFECT_APPLY_AURA;
    template.effect_base_points1 = 11;
    template.effect_apply_aura_name1 = SPELL_AURA_MOD_RESISTANCE;
    template.effect_implicit_target_a1 = TARGET_UNIT_CASTER;
    template
}

fn fireball_with_dot_spell_template() -> wow_db::SpellTemplateQuery {
    let mut template = fireball_spell_template();
    template.casting_time_index = 0;
    template.duration_index = 9;
    template.effect2 = SPELL_EFFECT_APPLY_AURA;
    template.effect_apply_aura_name2 = SPELL_AURA_PERIODIC_DAMAGE;
    template.effect_base_points2 = 2;
    template.effect_amplitude2 = 2_000;
    template.effect_implicit_target_a2 = TARGET_UNIT_ENEMY;
    template
}

fn instant_firebolt_spell_template() -> wow_db::SpellTemplateQuery {
    let mut template = test_spell_template(999_120);
    template.spell_name = "Instant Firebolt".to_string();
    template.school = 4;
    template.power_type = POWER_TYPE_MANA;
    template.mana_cost = 10;
    template.start_recovery_category = 133;
    template.start_recovery_time = 1_500;
    template.effect1 = SPELL_EFFECT_SCHOOL_DAMAGE;
    template.effect_base_points1 = 13;
    template.effect_implicit_target_a1 = TARGET_UNIT_ENEMY;
    template.dmg_class = SPELL_DAMAGE_CLASS_MAGIC;
    template
}

fn item_firebolt_spell_template() -> wow_db::SpellTemplateQuery {
    let mut template = instant_firebolt_spell_template();
    template.id = 999_122;
    template.spell_name = "Item Firebolt".to_string();
    template.power_type = 0;
    template.mana_cost = 0;
    template.start_recovery_category = 0;
    template.start_recovery_time = 0;
    template
}

fn frostbolt_like_spell_template() -> wow_db::SpellTemplateQuery {
    let mut template = test_spell_template(999_121);
    template.spell_name = "Frostbolt-ish".to_string();
    template.school = 16;
    template.power_type = POWER_TYPE_MANA;
    template.mana_cost = 10;
    template.duration_index = 21;
    template.start_recovery_category = 133;
    template.start_recovery_time = 1_500;
    template.effect1 = SPELL_EFFECT_SCHOOL_DAMAGE;
    template.effect_base_points1 = 13;
    template.effect_implicit_target_a1 = TARGET_UNIT_ENEMY;
    template.effect2 = SPELL_EFFECT_APPLY_AURA;
    template.effect_apply_aura_name2 = SPELL_AURA_MOD_DECREASE_SPEED;
    template.effect_base_points2 = 40;
    template.effect_implicit_target_a2 = TARGET_UNIT_ENEMY;
    template.dmg_class = SPELL_DAMAGE_CLASS_MAGIC;
    template
}

fn immolate_spell_template() -> wow_db::SpellTemplateQuery {
    let mut template = test_spell_template(348);
    template.spell_name = "Immolate".to_string();
    template.school = 2;
    template.casting_time_index = 5;
    template.power_type = POWER_TYPE_MANA;
    template.mana_cost = 25;
    template.duration_index = 8;
    template.attributes_ex2 = TEST_SPELL_ATTR_EX2_CANT_CRIT;
    template.attributes_ex3 = TEST_SPELL_ATTR_EX3_ALWAYS_HIT;
    template.effect1 = SPELL_EFFECT_APPLY_AURA;
    template.effect_base_points1 = 3;
    template.effect_apply_aura_name1 = SPELL_AURA_PERIODIC_DAMAGE;
    template.effect_amplitude1 = 3_000;
    template.effect_implicit_target_a1 = TARGET_UNIT_ENEMY;
    template.effect2 = 2;
    template.effect_base_points2 = 7;
    template.effect_implicit_target_a2 = TARGET_UNIT_ENEMY;
    template.dmg_class = SPELL_DAMAGE_CLASS_MAGIC;
    template
}

fn two_school_damage_effects_spell_template() -> wow_db::SpellTemplateQuery {
    let mut template = test_spell_template(999_011);
    template.spell_name = "Two Bolts".to_string();
    template.school = 4;
    template.attributes_ex3 = TEST_SPELL_ATTR_EX3_ALWAYS_HIT;
    template.power_type = POWER_TYPE_MANA;
    template.mana_cost = 0;
    template.effect1 = 2;
    template.effect_base_points1 = 4;
    template.effect_implicit_target_a1 = TARGET_UNIT_ENEMY;
    template.effect2 = 2;
    template.effect_base_points2 = 6;
    template.effect_implicit_target_a2 = TARGET_UNIT_ENEMY;
    template
}

fn battle_shout_spell_template() -> wow_db::SpellTemplateQuery {
    let mut template = test_spell_template(6673);
    template.spell_name = "Battle Shout".to_string();
    template.rank = Some("Rank 1".to_string());
    template.attributes = 327696;
    template.attributes_ex2 = 4;
    template.start_recovery_category = 133;
    template.start_recovery_time = 1500;
    template.power_type = 1;
    template.mana_cost = 100;
    template.duration_index = 4;
    template.effect1 = 6;
    template.effect_base_points1 = 14;
    template.effect_apply_aura_name1 = 99;
    template.effect_implicit_target_a1 = 20;
    template.spell_family_name = 4;
    template.spell_family_flags = 65536;
    template.dmg_class = 1;
    template
}

fn rend_spell_template() -> wow_db::SpellTemplateQuery {
    let mut template = test_spell_template(772);
    template.spell_name = "Rend".to_string();
    template.school = 1;
    template.power_type = POWER_TYPE_RAGE;
    template.mana_cost = 100;
    template.duration_index = 21;
    template.start_recovery_category = 133;
    template.start_recovery_time = 1500;
    template.effect1 = SPELL_EFFECT_APPLY_AURA;
    template.effect_base_points1 = 4;
    template.effect_apply_aura_name1 = SPELL_AURA_PERIODIC_DAMAGE;
    template.effect_amplitude1 = 3_000;
    template.effect_implicit_target_a1 = TARGET_UNIT_ENEMY;
    template.dmg_class = 2;
    template
}

fn charge_spell_template() -> wow_db::SpellTemplateQuery {
    let mut template = test_spell_template(100);
    template.spell_name = "Charge".to_string();
    template.school = 1;
    template.power_type = POWER_TYPE_RAGE;
    template.mana_cost = 0;
    template.start_recovery_category = 133;
    template.start_recovery_time = 1_500;
    template.speed = 27.0;
    template.effect1 = SPELL_EFFECT_CHARGE;
    template.effect_implicit_target_a1 = TARGET_UNIT_ENEMY;
    template.effect2 = SPELL_EFFECT_ENERGIZE;
    template.effect_base_points2 = 89;
    template.effect_die_sides2 = 1;
    template.effect_implicit_target_a2 = TARGET_UNIT_CASTER;
    template.effect3 = 64;
    template.effect_trigger_spell3 = 7922;
    template.effect_implicit_target_a3 = TARGET_UNIT_ENEMY;
    template.dmg_class = 1;
    template
}

fn charge_stun_spell_template() -> wow_db::SpellTemplateQuery {
    let mut template = test_spell_template(7922);
    template.spell_name = "Charge Stun".to_string();
    template.school = 1;
    template.attributes = 327696;
    template.attributes_ex = 512;
    template.duration_index = 36;
    template.effect1 = SPELL_EFFECT_APPLY_AURA;
    template.effect_apply_aura_name1 = SPELL_AURA_MOD_STUN;
    template.effect_base_points1 = -1;
    template.effect_die_sides1 = 1;
    template.effect_implicit_target_a1 = TARGET_UNIT_ENEMY;
    template.dmg_class = 1;
    template
}

fn thunder_clap_spell_template() -> wow_db::SpellTemplateQuery {
    let mut template = test_spell_template(6343);
    template.spell_name = "Thunder Clap".to_string();
    template.rank = Some("Rank 1".to_string());
    template.attributes_ex2 = TEST_SPELL_ATTR_EX2_CANT_CRIT;
    template.attributes_ex3 = TEST_SPELL_ATTR_EX3_ALWAYS_HIT;
    template.power_type = POWER_TYPE_RAGE;
    template.mana_cost = 200;
    template.start_recovery_category = 133;
    template.start_recovery_time = 1_500;
    template.effect1 = SPELL_EFFECT_SCHOOL_DAMAGE;
    template.effect_base_points1 = 9;
    template.effect_die_sides1 = 1;
    template.effect_implicit_target_a1 = TARGET_LOCATION_CASTER_SRC;
    template.effect_radius_index1 = 14;
    template.effect2 = SPELL_EFFECT_APPLY_AURA;
    template.effect_apply_aura_name2 = SPELL_AURA_MOD_MELEE_HASTE;
    template.effect_base_points2 = -11;
    template.effect_die_sides2 = 1;
    template.effect_implicit_target_a2 = TARGET_LOCATION_CASTER_SRC;
    template.effect_radius_index2 = 14;
    template.duration_index = 1;
    template.spell_family_name = 4;
    template.spell_family_flags = 128;
    template.dmg_class = 1;
    template
}

fn flamestrike_spell_template() -> wow_db::SpellTemplateQuery {
    let mut template = test_spell_template(2120);
    template.spell_name = "Flamestrike".to_string();
    template.rank = Some("Rank 1".to_string());
    template.school = 4;
    template.attributes = 65_536;
    template.attributes_ex = 268_435_592;
    template.attributes_ex2 = 64;
    template.casting_time_index = 14;
    template.range_index = 5;
    template.power_type = POWER_TYPE_MANA;
    template.mana_cost = 195;
    template.start_recovery_category = 133;
    template.start_recovery_time = 1_500;
    template.duration_index = 3;
    template.effect1 = SPELL_EFFECT_SCHOOL_DAMAGE;
    template.effect_base_points1 = 51;
    template.effect_die_sides1 = 17;
    template.effect_base_dice1 = 1;
    template.effect_implicit_target_a1 = TARGET_ENUM_UNITS_ENEMY_AOE_AT_DEST_LOC;
    template.effect_radius_index1 = 8;
    template.effect2 = SPELL_EFFECT_PERSISTENT_AREA_AURA;
    template.effect_base_points2 = 11;
    template.effect_die_sides2 = 1;
    template.effect_apply_aura_name2 = SPELL_AURA_PERIODIC_DAMAGE;
    template.effect_implicit_target_a2 = TARGET_ENUM_UNITS_ENEMY_AOE_AT_DYNOBJ_LOC;
    template.effect_radius_index2 = 8;
    template.effect_amplitude2 = 2_000;
    template.dmg_class = SPELL_DAMAGE_CLASS_MAGIC;
    template
}

fn blizzard_spell_template() -> wow_db::SpellTemplateQuery {
    let mut template = test_spell_template(10);
    template.spell_name = "Blizzard".to_string();
    template.rank = Some("Rank 1".to_string());
    template.school = 16;
    template.attributes_ex = SPELL_ATTR_EX_IS_CHANNELED;
    template.attributes_ex2 = TEST_SPELL_ATTR_EX2_CANT_CRIT;
    template.attributes_ex3 = TEST_SPELL_ATTR_EX3_ALWAYS_HIT;
    template.channel_interrupt_flags = AURA_INTERRUPT_FLAG_DAMAGE_CHANNEL_DURATION;
    template.range_index = 5;
    template.power_type = POWER_TYPE_MANA;
    template.mana_cost = 320;
    template.start_recovery_category = 133;
    template.start_recovery_time = 1_500;
    template.duration_index = 30;
    template.effect1 = SPELL_EFFECT_PERSISTENT_AREA_AURA;
    template.effect_base_points1 = 52;
    template.effect_die_sides1 = 1;
    template.effect_apply_aura_name1 = SPELL_AURA_PERIODIC_DAMAGE;
    template.effect_implicit_target_a1 = TARGET_ENUM_UNITS_ENEMY_AOE_AT_DYNOBJ_LOC;
    template.effect_radius_index1 = 11;
    template.effect_amplitude1 = 1_000;
    template.dmg_class = SPELL_DAMAGE_CLASS_MAGIC;
    template
}

fn arcane_missiles_spell_template() -> wow_db::SpellTemplateQuery {
    let mut template = test_spell_template(5143);
    template.spell_name = "Arcane Missiles".to_string();
    template.rank = Some("Rank 1".to_string());
    template.school = 6;
    template.attributes = 536_936_704;
    template.attributes_ex = SPELL_ATTR_EX_IS_CHANNELED;
    template.attributes_ex2 = TEST_SPELL_ATTR_EX2_CANT_CRIT;
    template.attributes_ex3 = TEST_SPELL_ATTR_EX3_ALWAYS_HIT;
    template.channel_interrupt_flags = AURA_INTERRUPT_FLAG_DAMAGE_CHANNEL_DURATION;
    template.casting_time_index = 0;
    template.range_index = 4;
    template.power_type = POWER_TYPE_MANA;
    template.mana_cost = 85;
    template.start_recovery_category = 133;
    template.start_recovery_time = 1_500;
    template.duration_index = 6;
    template.effect1 = SPELL_EFFECT_APPLY_AURA;
    template.effect_apply_aura_name1 = SPELL_AURA_PERIODIC_TRIGGER_SPELL;
    template.effect_implicit_target_a1 = TARGET_UNIT_CASTER;
    template.effect_amplitude1 = 1_000;
    template.effect_trigger_spell1 = 7268;
    template.dmg_class = SPELL_DAMAGE_CLASS_MAGIC;
    template
}

fn arcane_missile_trigger_spell_template() -> wow_db::SpellTemplateQuery {
    let mut template = test_spell_template(7268);
    template.spell_name = "Arcane Missile".to_string();
    template.rank = Some("Rank 1".to_string());
    template.school = 6;
    template.attributes = 65_536;
    template.attributes_ex2 = TEST_SPELL_ATTR_EX2_CANT_CRIT;
    template.attributes_ex3 = TEST_SPELL_ATTR_EX3_ALWAYS_HIT;
    template.speed = 20.0;
    template.effect1 = SPELL_EFFECT_SCHOOL_DAMAGE;
    template.effect_base_points1 = 23;
    template.effect_die_sides1 = 1;
    template.effect_base_dice1 = 1;
    template.effect_implicit_target_a1 = TARGET_UNIT_ENEMY;
    template.dmg_class = SPELL_DAMAGE_CLASS_MAGIC;
    template
}

fn frost_nova_spell_template() -> wow_db::SpellTemplateQuery {
    let mut template = test_spell_template(122);
    template.spell_name = "Frost Nova".to_string();
    template.effect1 = SPELL_EFFECT_APPLY_AURA;
    template.effect_apply_aura_name1 = SPELL_AURA_MOD_ROOT;
    template.effect_implicit_target_a1 = TARGET_ENUM_UNITS_ENEMY_AOE_AT_SRC_LOC;
    template.effect_radius_index1 = 11;
    template.start_recovery_category = 133;
    template.start_recovery_time = 1_500;
    template
}

fn hearthstone_spell_template() -> wow_db::SpellTemplateQuery {
    let mut template = test_spell_template(8690);
    template.spell_name = "Hearthstone".to_string();
    template.effect1 = SPELL_EFFECT_TELEPORT_UNITS;
    template.effect_implicit_target_a1 = TARGET_UNIT_CASTER;
    template.start_recovery_category = 133;
    template.start_recovery_time = 1_500;
    template
}

#[test]
fn periodic_damage_tick_uses_aura_amount_without_armor_reduction() {
    let periodic = PeriodicDamageAura {
        aura_name: SPELL_AURA_PERIODIC_DAMAGE,
        school: 1,
        damage_class: 2,
        attributes_ex2: 0,
        attributes_ex3: 0,
        caster_snapshot: SpellCombatUnitSnapshot {
            level: 1,
            class: 0,
            intellect: 0,
            resistances: [0; MAX_SPELL_SCHOOL],
        },
        amount: 5,
        tick_millis: 3_000,
        next_tick_at: Instant::now(),
    };

    let snapshot = SpellCombatUnitSnapshot {
        level: 1,
        class: 1,
        intellect: 0,
        resistances: [0; MAX_SPELL_SCHOOL],
    };
    let tick = calculate_periodic_damage_tick(&periodic, snapshot, snapshot, 120);

    assert_eq!(tick.requested_damage, 5);
    assert_eq!(tick.dealt_damage, 5);
    assert_eq!(tick.school, 1);
    assert_eq!(tick.absorb, 0);
    assert_eq!(tick.resist, 0);
    assert_eq!(tick.threat, 5.0);
}

#[test]
fn periodic_damage_tick_uses_shared_spell_outcome_for_partial_resist() {
    let periodic = PeriodicDamageAura {
        aura_name: SPELL_AURA_PERIODIC_DAMAGE,
        school: 2,
        damage_class: SPELL_DAMAGE_CLASS_MAGIC,
        attributes_ex2: SPELL_ATTR_EX2_CANT_CRIT,
        attributes_ex3: 0,
        caster_snapshot: SpellCombatUnitSnapshot {
            level: 10,
            class: 8,
            intellect: 100,
            resistances: [0; MAX_SPELL_SCHOOL],
        },
        amount: 100,
        tick_millis: 3_000,
        next_tick_at: Instant::now(),
    };
    let caster = SpellCombatUnitSnapshot {
        level: 10,
        class: 8,
        intellect: 100,
        resistances: [0; MAX_SPELL_SCHOOL],
    };
    let mut target = caster;
    target.resistances[2] = 1_000;

    let tick = calculate_periodic_damage_tick_with_rolls(
        &periodic,
        caster,
        target,
        120,
        SpellDamageOutcomeRolls {
            hit_roll: 10_000,
            crit_roll: 1,
            partial_resist_roll: 10_000,
        },
    );

    assert_eq!(tick.requested_damage, 100);
    assert_eq!(tick.dealt_damage, 20);
    assert_eq!(tick.resist, 80);
    assert_eq!(tick.threat, 20.0);
}

#[test]
fn human_spirit_passive_applies_total_spirit_percent_without_visible_buff() {
    let mut template = test_spell_template(20598);
    template.spell_name = "The Human Spirit".to_string();
    template.rank = Some("Racial Passive".to_string());
    template.attributes = SPELL_ATTR_PASSIVE;
    template.effect1 = SPELL_EFFECT_APPLY_AURA;
    template.effect_apply_aura_name1 = SPELL_AURA_MOD_TOTAL_STAT_PERCENTAGE;
    template.effect_base_points1 = 4;
    template.effect_misc_value1 = 4;

    let aura = passive_spell_active_aura(
        &template,
        ObjectGuid::new(HighGuid::Player, 0, 7),
        1,
        test_spell_effect_value_context(&template),
        Instant::now(),
        None,
    )
    .expect("passive racial should build an active modifier aura");
    let world_stats = PlayerWorldStats {
        base_health: 20,
        base_mana: 0,
        stats: [23, 20, 22, 20, 21],
        next_level_xp: 400,
    };
    let effective = player_world_stats_with_active_auras(world_stats, std::slice::from_ref(&aura));
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_player_stat_mod_update_values(&mut values, &world_stats, &effective).unwrap();

    assert!(!aura.visible);
    assert_eq!(
        aura.stat_modifiers,
        vec![AuraStatModifier::TotalStatPercent {
            stat: 4,
            percent: 5
        }]
    );
    assert_eq!(effective.stats[4], 22);
    assert_eq!(values[UNIT_FIELD_STAT0 + 4], None);
    assert_eq!(values[PLAYER_FIELD_POSSTAT0 + 4], Some(1));
    assert_eq!(values[PLAYER_FIELD_NEGSTAT0 + 4], Some(0));
}

#[test]
fn weapon_specialization_passives_apply_skill_bonus_without_persisting_base_skill() {
    let mut template = test_spell_template(20597);
    template.spell_name = "Sword Specialization".to_string();
    template.rank = Some("Racial Passive".to_string());
    template.attributes = SPELL_ATTR_PASSIVE;
    template.effect1 = SPELL_EFFECT_APPLY_AURA;
    template.effect2 = SPELL_EFFECT_APPLY_AURA;
    template.effect_apply_aura_name1 = SPELL_AURA_MOD_SKILL_TALENT;
    template.effect_apply_aura_name2 = SPELL_AURA_MOD_SKILL_TALENT;
    template.effect_base_points1 = 4;
    template.effect_base_points2 = 4;
    template.effect_misc_value1 = 43;
    template.effect_misc_value2 = 55;

    let aura = passive_spell_active_aura(
        &template,
        ObjectGuid::new(HighGuid::Player, 0, 7),
        1,
        test_spell_effect_value_context(&template),
        Instant::now(),
        None,
    )
    .unwrap();
    let skills = vec![test_skill(43, 1, 5)];
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_player_skill_update_values(&mut values, &skills, std::slice::from_ref(&aura)).unwrap();

    assert_eq!(active_aura_skill_bonus(std::slice::from_ref(&aura), 43), 5);
    assert_eq!(active_aura_skill_bonus(std::slice::from_ref(&aura), 55), 5);
    assert_eq!(
        current_skill_value_with_active_auras(&skills, std::slice::from_ref(&aura), 43),
        6
    );
    assert_eq!(values[PLAYER_SKILL_INFO_1_1 + 1], Some(make_pair32(1, 5)));
    assert_eq!(values[PLAYER_SKILL_INFO_1_1 + 2], Some(make_pair32(0, 5)));
}

#[test]
fn diplomacy_passive_modifies_quest_reputation_gain() {
    let aura = ActiveAura {
        spell_id: 20599,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 1,
        interrupt_flags: 0,
        positive: true,
        visible: false,
        duration_millis: None,
        expires_at: None,
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::ReputationGainPercent { percent: 10 }],
        proc_triggers: Vec::new(),
    };
    let mut quest = test_quest_template(3901);
    quest.rew_rep_faction[0] = 72;
    quest.rew_rep_value[0] = 25;

    let rewards = quest_reputation_rewards_with_bonus(
        1,
        &quest,
        reputation_gain_percent_from_active_auras(&[aura]),
    );

    assert_eq!(rewards, vec![(72, 27)]);
}

#[tokio::test]
async fn item_use_spell_failure_allows_refreshing_existing_aura_during_cooldown() {
    let now = Instant::now();
    let maps = MapRuntimeManager::default();
    let map_id = 0;
    let character_guid = 7;
    let spell_id = 1127;
    let mut player = test_player_runtime(
        character_guid,
        SessionId::next(),
        WorldPosition::new(map_id, 0.0, 0.0, 0.0, 0.0),
    );
    player
        .spell_cooldowns_until
        .insert(spell_id, now + Duration::from_secs(30));
    player
        .spell_global_cooldowns_until
        .insert(1, now + Duration::from_secs(30));
    player.active_auras.push(ActiveAura {
        spell_id,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 1,
        interrupt_flags: 0,
        positive: true,
        visible: true,
        duration_millis: Some(30_000),
        expires_at: Some(now + Duration::from_secs(30)),
        periodic_damage: None,
        periodic_regen: Some(PeriodicRegenAura {
            health_amount: 5,
            mana_amount: 0,
            tick_millis: 2_000,
            next_tick_at: now + Duration::from_secs(2),
            interrupts_on_move_and_stand: true,
            suppresses_recent_damage: true,
            makes_player_sit: true,
        }),
        stat_modifiers: Vec::new(),
        proc_triggers: Vec::new(),
    });
    maps.add_player(player).await.unwrap();
    let item_spell = SpellCastProfile {
        spell_id,
        kind: SpellCastKind::AuraApplication,
        aura_target: SpellAuraTarget::Caster,
        bonus_damage: 0,
        weapon_damage_percent: 100,
        damage: 0,
        power: SpellPowerCost::Mana { cost: 0 },
        requires_melee: false,
        requires_behind: false,
        needs_combo_points: false,
        global_cooldown_category: 1,
        global_cooldown_millis: 1_500,
        cooldown_category: 0,
        category_cooldown_millis: 0,
        cooldown_millis: 30_000,
    };

    assert_eq!(
        item_use_spell_failure(&maps, map_id, character_guid, &item_spell, now, false).await,
        None
    );
}

#[tokio::test]
async fn consumable_regen_item_use_does_not_install_duration_cooldown() {
    let now = Instant::now();
    let maps = MapRuntimeManager::default();
    let map_id = 0;
    let character_guid = 7;
    maps.add_player(test_player_runtime(
        character_guid,
        SessionId::next(),
        WorldPosition::new(map_id, 0.0, 0.0, 0.0, 0.0),
    ))
    .await
    .unwrap();
    let item_spell = SpellCastProfile {
        spell_id: 1127,
        kind: SpellCastKind::AuraApplication,
        aura_target: SpellAuraTarget::Caster,
        bonus_damage: 0,
        weapon_damage_percent: 100,
        damage: 0,
        power: SpellPowerCost::Mana { cost: 0 },
        requires_melee: false,
        requires_behind: false,
        needs_combo_points: false,
        global_cooldown_category: 1,
        global_cooldown_millis: 1_500,
        cooldown_category: 0,
        category_cooldown_millis: 0,
        cooldown_millis: 30_000,
    };

    apply_item_use_spell_cooldowns(
        &maps,
        map_id,
        character_guid,
        6948,
        &item_spell,
        now,
        true,
        0,
        0,
    )
    .await;

    let snapshot = maps
        .player_runtime_snapshot(map_id, character_guid)
        .await
        .unwrap();
    assert!(!snapshot
        .spell_cooldowns_until
        .contains_key(&item_spell.spell_id));
    assert!(snapshot
        .spell_global_cooldowns_until
        .contains_key(&item_spell.global_cooldown_category));
    assert_eq!(
        item_use_spell_failure(
            &maps,
            map_id,
            character_guid,
            &item_spell,
            now + Duration::from_secs(2),
            true,
        )
        .await,
        None
    );
}

#[test]
fn item_use_cooldown_keeps_spell_and_item_category_recovery_separate() {
    let mut template = test_spell_template(439);
    template.recovery_time = 10_000;
    template.category = 1;
    template.category_recovery_time = 10_000;
    let item_spell = wow_db::ItemTemplateSpell {
        spell_id: template.id,
        spell_trigger: ITEM_SPELLTRIGGER_ON_USE,
        spell_charges: -1,
        spell_cooldown: -1,
        spell_category: 4,
        spell_category_cooldown: 120_000,
    };
    let profile = SpellCastProfile {
        spell_id: template.id,
        kind: SpellCastKind::DirectHeal,
        aura_target: SpellAuraTarget::Caster,
        bonus_damage: 0,
        weapon_damage_percent: 100,
        damage: 0,
        power: SpellPowerCost::Mana { cost: 0 },
        requires_melee: false,
        requires_behind: false,
        needs_combo_points: false,
        global_cooldown_category: 1,
        global_cooldown_millis: 1_500,
        cooldown_category: 0,
        category_cooldown_millis: 0,
        cooldown_millis: 10_000,
    };

    let (profile, cooldown) =
        item_spell_cast_profile_with_cooldown(profile, item_spell, &template);

    assert_eq!(profile.cooldown_millis, 10_000);
    assert_eq!(profile.cooldown_category, 4);
    assert_eq!(profile.category_cooldown_millis, 120_000);
    assert_eq!(
        cooldown,
        ItemSpellCooldown {
            recovery_millis: 10_000,
            category: 4,
            category_recovery_millis: 120_000,
        }
    );
}

#[tokio::test]
async fn item_use_cooldown_records_item_id_and_item_category_in_map_state() {
    let now = Instant::now();
    let maps = MapRuntimeManager::default();
    let map_id = 0;
    let character_guid = 7;
    maps.add_player(test_player_runtime(
        character_guid,
        SessionId::next(),
        WorldPosition::new(map_id, 0.0, 0.0, 0.0, 0.0),
    ))
    .await
    .unwrap();
    let item_spell = SpellCastProfile {
        spell_id: 439,
        kind: SpellCastKind::DirectHeal,
        aura_target: SpellAuraTarget::Caster,
        bonus_damage: 0,
        weapon_damage_percent: 100,
        damage: 0,
        power: SpellPowerCost::Mana { cost: 0 },
        requires_melee: false,
        requires_behind: false,
        needs_combo_points: false,
        global_cooldown_category: 1,
        global_cooldown_millis: 1_500,
        cooldown_category: 4,
        category_cooldown_millis: 120_000,
        cooldown_millis: 10_000,
    };

    apply_item_use_spell_cooldowns(&maps, map_id, character_guid, 929, &item_spell, now, false, 4, 120_000)
        .await;

    let snapshot = maps
        .player_runtime_snapshot(map_id, character_guid)
        .await
        .unwrap();
    assert_eq!(snapshot.spell_cooldown_item_ids.get(&439), Some(&929));
    assert_eq!(snapshot.spell_cooldown_categories.get(&439), Some(&4));
    let spell_until = snapshot.spell_cooldowns_until.get(&439).unwrap();
    let category_until = snapshot.spell_global_cooldowns_until.get(&4).unwrap();
    assert!(*spell_until >= now + Duration::from_millis(9_900));
    assert!(*spell_until <= now + Duration::from_millis(10_100));
    assert!(*category_until >= now + Duration::from_millis(119_900));
    assert!(*category_until <= now + Duration::from_millis(120_100));
    assert_eq!(
        item_use_spell_failure(
            &maps,
            map_id,
            character_guid,
            &item_spell,
            now + Duration::from_secs(11),
            false,
        )
        .await,
        Some(SPELL_FAILED_NOT_READY)
    );
}

#[tokio::test]
async fn item_use_cooldown_records_category_only_cooldown_like_cmangos() {
    let now = Instant::now();
    let maps = MapRuntimeManager::default();
    let map_id = 0;
    let character_guid = 7;
    maps.add_player(test_player_runtime(
        character_guid,
        SessionId::next(),
        WorldPosition::new(map_id, 0.0, 0.0, 0.0, 0.0),
    ))
    .await
    .unwrap();
    let item_spell = SpellCastProfile {
        spell_id: 439,
        kind: SpellCastKind::DirectHeal,
        aura_target: SpellAuraTarget::Caster,
        bonus_damage: 0,
        weapon_damage_percent: 100,
        damage: 0,
        power: SpellPowerCost::Mana { cost: 0 },
        requires_melee: false,
        requires_behind: false,
        needs_combo_points: false,
        global_cooldown_category: 1,
        global_cooldown_millis: 1_500,
        cooldown_category: 4,
        category_cooldown_millis: 120_000,
        cooldown_millis: 0,
    };

    apply_item_use_spell_cooldowns(
        &maps,
        map_id,
        character_guid,
        929,
        &item_spell,
        now,
        false,
        4,
        120_000,
    )
    .await;

    let snapshot = maps
        .player_runtime_snapshot(map_id, character_guid)
        .await
        .unwrap();
    assert_eq!(snapshot.spell_cooldown_item_ids.get(&439), Some(&929));
    assert_eq!(snapshot.spell_cooldown_categories.get(&439), Some(&4));
    assert_eq!(snapshot.spell_cooldowns_until.get(&439), Some(&now));
    assert!(snapshot.spell_global_cooldowns_until.get(&4).unwrap() > &now);
    assert_eq!(
        item_use_spell_failure(
            &maps,
            map_id,
            character_guid,
            &item_spell,
            now + Duration::from_secs(11),
            false,
        )
        .await,
        Some(SPELL_FAILED_NOT_READY)
    );
}

#[test]
fn create_item_spell_profile_uses_effect_item_type_and_stack_cap() {
    let mut template = test_spell_template(587);
    template.effect1 = SPELL_EFFECT_CREATE_ITEM;
    template.effect_base_points1 = 9;
    template.effect_item_type1 = 1113;

    let profile = player_spell_cast_profile(&template).expect("create item spell profile");
    assert_eq!(profile.kind, SpellCastKind::CreateItem);
    assert_eq!(profile.aura_target, SpellAuraTarget::Caster);
    assert_eq!(profile.damage, 0);
    assert!(!profile.requires_melee);

    let spell_info = SpellInfo::from_template(&template);
    let effects =
        create_item_spell_effects(&spell_info, test_spell_effect_value_context(&template));
    assert_eq!(
        effects,
        vec![CreateItemSpellEffect {
            item_template: 1113,
            requested_count: 10,
        }]
    );

    let mut conjured_food = test_item_template(1113, 0, 0, 0.0, 0.0, 0);
    conjured_food.stackable = 5;
    assert_eq!(
        create_item_count_for_template(effects[0], &conjured_food),
        5
    );
}

#[test]
fn player_spell_rank_context_uses_skill_line_ability_and_skill_value_caps() {
    let mut world_data = WorldDataFiles::fallback();
    world_data.skill_line_abilities_by_spell.insert(
        587,
        vec![SkillLineAbilityEntry {
            id: 1,
            skill_id: 237,
            spell_id: 587,
            race_mask: 0,
            class_mask: 128,
            min_value: 1,
            max_value: 300,
        }],
    );
    world_data.skill_line_abilities_by_spell.insert(
        9999,
        vec![SkillLineAbilityEntry {
            id: 2,
            skill_id: 8,
            spell_id: 9999,
            race_mask: 0,
            class_mask: 0,
            min_value: 1,
            max_value: 300,
        }],
    );
    let maps = MapRuntimeManager::with_world_data_files(&world_data);
    let mut template = test_spell_template(587);
    template.max_level = 10;
    template.base_level = 1;
    template.spell_level = 1;

    let context =
        player_spell_effect_value_context(&maps, &template, &[test_skill(237, 45, 300)], 0);
    assert_eq!(context.spell_rank_level, Some(9));

    let capped =
        player_spell_effect_value_context(&maps, &template, &[test_skill(237, 75, 300)], 0);
    assert_eq!(capped.spell_rank_level, Some(10));

    let missing_skill =
        player_spell_effect_value_context(&maps, &template, &[test_skill(8, 300, 300)], 0);
    assert_eq!(missing_skill.spell_rank_level, Some(0));

    let no_mapping = player_spell_effect_value_context(
        &maps,
        &test_spell_template(597),
        &[test_skill(237, 45, 300)],
        0,
    );
    assert_eq!(no_mapping.spell_rank_level, Some(0));

    let empty_maps = MapRuntimeManager::with_world_data_files(&WorldDataFiles::fallback());
    let degraded =
        player_spell_effect_value_context(&empty_maps, &template, &[test_skill(237, 45, 300)], 0);
    assert_eq!(degraded.spell_rank_level, None);
}

#[test]
fn level_backed_skill_sync_maximizes_class_skills_and_preserves_mono_skills() {
    let mut world_data = WorldDataFiles::fallback();
    world_data.skill_lines.insert(
        237,
        SkillLineEntry {
            id: 237,
            category_id: 7,
        },
    );
    world_data.skill_lines.insert(
        43,
        SkillLineEntry {
            id: 43,
            category_id: 6,
        },
    );
    world_data.skill_lines.insert(
        98,
        SkillLineEntry {
            id: 98,
            category_id: 10,
        },
    );
    world_data.skill_lines.insert(
        415,
        SkillLineEntry {
            id: 415,
            category_id: 8,
        },
    );
    world_data.skill_race_class_infos_by_skill.insert(
        237,
        vec![SkillRaceClassInfoEntry {
            skill_id: 237,
            race_mask: 0,
            class_mask: 1,
            flags: 0x010,
            req_level: 1,
            skill_tier_id: 0,
        }],
    );
    world_data.skill_race_class_infos_by_skill.insert(
        43,
        vec![SkillRaceClassInfoEntry {
            skill_id: 43,
            race_mask: 1,
            class_mask: 1,
            flags: 0,
            req_level: 1,
            skill_tier_id: 0,
        }],
    );
    let maps = MapRuntimeManager::with_world_data_files(&world_data);
    let mut skills = vec![
        test_skill(237, 5, 5),
        test_skill(43, 3, 5),
        test_skill(98, 300, 300),
        test_skill(415, 1, 1),
    ];

    let updates = sync_player_level_backed_skills(&maps, 1, 1, 6, &mut skills);

    assert_eq!(skills[0], test_skill(237, 30, 30));
    assert_eq!(skills[1], test_skill(43, 3, 30));
    assert_eq!(skills[2], test_skill(98, 300, 300));
    assert_eq!(skills[3], test_skill(415, 1, 1));
    assert_eq!(
        updates
            .iter()
            .map(|update| (update.slot, update.skill, update.value, update.max))
            .collect::<Vec<_>>(),
        vec![(0, 237, 30, 30), (1, 43, 3, 30)]
    );
}

#[test]
fn spell_effect_value_matches_cmangos_level_scaling_floor_and_caps() {
    let mut template = test_spell_template(587);
    template.max_level = 15;
    template.base_level = 6;
    template.spell_level = 6;
    template.effect1 = SPELL_EFFECT_CREATE_ITEM;
    template.effect_base_points1 = 1;
    template.effect_base_dice1 = 1;
    template.effect_die_sides1 = 1;
    template.effect_real_points_per_level1 = 2.0;
    let effect = SpellInfo::from_template(&template).effects[0];

    assert_eq!(
        spell_effect_calculated_i32(
            effect,
            SpellEffectValueContext::with_spell_rank_level(&template, 1, 0),
        ),
        2
    );
    assert_eq!(
        spell_effect_calculated_i32(
            effect,
            SpellEffectValueContext::with_spell_rank_level(&template, 10, 0),
        ),
        10
    );
    assert_eq!(
        spell_effect_calculated_i32(
            effect,
            SpellEffectValueContext::with_spell_rank_level(&template, 60, 0),
        ),
        20
    );
}

#[test]
fn spell_effect_value_keeps_rank_seven_base_amount_without_scaling() {
    let mut template = test_spell_template(10144);
    template.effect1 = SPELL_EFFECT_CREATE_ITEM;
    template.effect_base_points1 = 19;
    template.effect_base_dice1 = 1;
    template.effect_die_sides1 = 1;
    let effect = SpellInfo::from_template(&template).effects[0];

    assert_eq!(
        spell_effect_calculated_i32(effect, test_spell_effect_value_context(&template)),
        20
    );
}

#[test]
fn spell_effect_value_handles_signed_aura_die_range_and_combo_points() {
    let mut negative_template = test_spell_template(6136);
    negative_template.effect1 = SPELL_EFFECT_APPLY_AURA;
    negative_template.effect_base_points1 = -26;
    negative_template.effect_base_dice1 = 1;
    negative_template.effect_die_sides1 = 1;
    let negative_effect = SpellInfo::from_template(&negative_template).effects[0];
    assert_eq!(
        spell_effect_calculated_i32(
            negative_effect,
            test_spell_effect_value_context(&negative_template),
        ),
        -25
    );

    let mut random_template = test_spell_template(1000);
    random_template.effect1 = SPELL_EFFECT_SCHOOL_DAMAGE;
    random_template.effect_base_points1 = 0;
    random_template.effect_base_dice1 = 2;
    random_template.effect_die_sides1 = 4;
    let random_effect = SpellInfo::from_template(&random_template).effects[0];
    for _ in 0..16 {
        let value = spell_effect_calculated_i32(
            random_effect,
            test_spell_effect_value_context(&random_template),
        );
        assert!(
            (2..=4).contains(&value),
            "effect roll {value} outside 2..=4"
        );
    }

    let mut combo_template = test_spell_template(1001);
    combo_template.effect1 = SPELL_EFFECT_SCHOOL_DAMAGE;
    combo_template.effect_base_points1 = 0;
    combo_template.effect_base_dice1 = 1;
    combo_template.effect_die_sides1 = 1;
    combo_template.effect_points_per_combo_point1 = 5.0;
    let combo_effect = SpellInfo::from_template(&combo_template).effects[0];
    assert_eq!(
        spell_effect_calculated_i32(
            combo_effect,
            SpellEffectValueContext::unranked(&combo_template, 3),
        ),
        16
    );
}

#[test]
fn create_item_spell_effects_use_scaled_conjure_count_and_stack_cap() {
    let mut template = test_spell_template(587);
    template.max_level = 15;
    template.base_level = 6;
    template.spell_level = 6;
    template.effect1 = SPELL_EFFECT_CREATE_ITEM;
    template.effect_base_points1 = 1;
    template.effect_base_dice1 = 1;
    template.effect_die_sides1 = 1;
    template.effect_real_points_per_level1 = 2.0;
    template.effect_item_type1 = 1113;
    let spell_info = SpellInfo::from_template(&template);

    let effects = create_item_spell_effects(
        &spell_info,
        SpellEffectValueContext::with_spell_rank_level(&template, 10, 0),
    );

    assert_eq!(
        effects,
        vec![CreateItemSpellEffect {
            item_template: 1113,
            requested_count: 10,
        }]
    );
    let mut conjured_food = test_item_template(1113, 0, 0, 0.0, 0.0, 0);
    conjured_food.stackable = 5;
    assert_eq!(
        create_item_count_for_template(effects[0], &conjured_food),
        5
    );
}

#[test]
fn caster_centered_hostile_root_spell_uses_aoe_target_and_radius_metadata() {
    let frost_nova = frost_nova_spell_template();
    let profile = player_spell_cast_profile(&frost_nova).expect("frost nova profile");
    assert_eq!(profile.kind, SpellCastKind::AuraApplication);
    assert_eq!(profile.aura_target, SpellAuraTarget::CasterAreaEnemy);
    let spell_info = SpellInfo::from_template(&frost_nova);
    assert_eq!(
        spell_info.unit_target_kind(profile.kind),
        SpellTargetKind::Caster
    );

    let aura = build_active_aura(
        &frost_nova,
        ObjectGuid::new(HighGuid::Player, 0, 7),
        1,
        test_spell_effect_value_context(&frost_nova),
        Instant::now(),
        None,
    );
    assert!(!aura.positive, "hostile AoE roots must be debuffs");
    assert_eq!(aura.stat_modifiers, vec![AuraStatModifier::Root]);

    let mut world_data = WorldDataFiles::fallback();
    world_data.spell_radii.insert(
        11,
        SpellRadiusEntry {
            radius: 8.0,
            radius_per_level: 0.0,
            max_radius: 8.0,
        },
    );
    let maps = MapRuntimeManager::with_world_data_files(&world_data);
    assert_eq!(
        spell_effect_radius_yards(&maps, spell_info.effects[0]),
        Some(8.0)
    );
}

#[test]
fn thunder_clap_uses_caster_source_aoe_damage_and_attack_speed_debuff() {
    let thunder_clap = thunder_clap_spell_template();
    let profile = player_spell_cast_profile(&thunder_clap).expect("thunder clap profile");
    assert_eq!(profile.kind, SpellCastKind::AuraApplication);
    assert_eq!(profile.aura_target, SpellAuraTarget::CasterAreaEnemy);
    assert_eq!(profile.damage, 10);

    let spell_info = SpellInfo::from_template(&thunder_clap);
    assert_eq!(
        spell_info.unit_target_kind(profile.kind),
        SpellTargetKind::Caster
    );
    assert!(effect_targets_caster_centered_hostile_area(
        spell_info.effects[0]
    ));
    assert!(effect_targets_caster_centered_hostile_area(
        spell_info.effects[1]
    ));

    let aura = build_active_aura(
        &thunder_clap,
        ObjectGuid::new(HighGuid::Player, 0, 7),
        6,
        test_spell_effect_value_context(&thunder_clap),
        Instant::now(),
        None,
    );
    assert!(!aura.positive, "Thunder Clap is a hostile AoE debuff");
    assert_eq!(
        aura.stat_modifiers,
        vec![AuraStatModifier::MeleeAttackTimePercent { percent: -10 }]
    );

    let mut world_data = WorldDataFiles::fallback();
    world_data.spell_radii.insert(
        14,
        SpellRadiusEntry {
            radius: 5.0,
            radius_per_level: 0.0,
            max_radius: 5.0,
        },
    );
    let maps = MapRuntimeManager::with_world_data_files(&world_data);
    assert_eq!(
        spell_effect_radius_yards(&maps, spell_info.effects[0]),
        Some(5.0)
    );
}

#[test]
fn flamestrike_uses_destination_hostile_aoe_targeting() {
    let flamestrike = flamestrike_spell_template();
    let profile = player_spell_cast_profile(&flamestrike).expect("flamestrike profile");
    assert_eq!(profile.kind, SpellCastKind::InstantDamage);

    let spell_info = SpellInfo::from_template(&flamestrike);
    assert_eq!(
        spell_info.unit_target_kind(profile.kind),
        SpellTargetKind::Destination
    );
    assert!(effect_targets_destination_hostile_area(
        spell_info.effects[0]
    ));
    assert!(effect_targets_destination_hostile_area(
        spell_info.effects[1]
    ));
    assert!(!effect_targets_caster_centered_hostile_area(
        spell_info.effects[0]
    ));

    let incoming_self_target = SpellCastTargets {
        target_mask: SPELL_CAST_TARGET_UNIT | SPELL_CAST_TARGET_DEST_LOCATION,
        unit_target: Some(ObjectGuid::new(HighGuid::Player, 0, 7)),
        gameobject_target: None,
        source_location: None,
        destination: Some(wow_proto::SpellTargetLocation {
            x: -8947.0,
            y: -132.0,
            z: 83.5312,
        }),
    };
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    let targets = normalize_spell_cast_targets(incoming_self_target, &profile, &spell_info, caster);
    assert_eq!(targets.target_mask, SPELL_CAST_TARGET_DEST_LOCATION);
    assert_eq!(targets.unit_target, None);
    assert!(targets.destination.is_some());
}

#[test]
fn blizzard_uses_destination_persistent_area_profile() {
    let blizzard = blizzard_spell_template();
    let profile = player_spell_cast_profile(&blizzard).expect("blizzard profile");
    assert_eq!(profile.kind, SpellCastKind::AuraApplication);

    let spell_info = SpellInfo::from_template(&blizzard);
    assert_eq!(
        spell_info.unit_target_kind(profile.kind),
        SpellTargetKind::Destination
    );
    assert!(effect_targets_destination_hostile_area(
        spell_info.effects[0]
    ));
}

#[test]
fn caster_centered_hostile_aoe_spell_packets_do_not_self_target() {
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    let incoming_self_target = SpellCastTargets {
        target_mask: SPELL_CAST_TARGET_UNIT,
        unit_target: Some(caster),
        gameobject_target: None,
        source_location: None,
        destination: None,
    };
    let template = thunder_clap_spell_template();
    let profile = player_spell_cast_profile(&template).unwrap();
    let spell_info = SpellInfo::from_template(&template);
    let targets = normalize_spell_cast_targets(incoming_self_target, &profile, &spell_info, caster);

    assert_eq!(targets.target_mask, 0);
    assert_eq!(targets.unit_target, None);
    let go = build_spell_go_body(caster, 6343, &targets).unwrap();
    let mut cursor = PackedGuid::packed_size(caster) * 2;
    assert_eq!(read_u32(&go, &mut cursor).unwrap(), 6343);
    cursor += 2;
    assert_eq!(
        go[cursor], 0,
        "SMSG_SPELL_GO must not list the caster as a hit target"
    );
    cursor += 2;
    assert_eq!(
        u16::from_le_bytes(go[cursor..cursor + 2].try_into().unwrap()),
        0
    );
    assert_eq!(cursor + 2, go.len());
}

#[test]
fn spell_effect_coverage_classifies_every_cmangos_effect_id() {
    for effect_id in 0..CMANGOS_MAX_SPELL_EFFECTS {
        assert_ne!(
            spell_effect_support(effect_id),
            SpellMechanicSupport::Unknown,
            "effect {effect_id} must be classified as implemented, no-op, or pending"
        );
    }
    assert_eq!(
        spell_effect_support(CMANGOS_MAX_SPELL_EFFECTS),
        SpellMechanicSupport::Unknown
    );
}

#[test]
fn spell_aura_coverage_classifies_every_cmangos_aura_id() {
    for aura_type in 0..CMANGOS_TOTAL_AURAS {
        assert_ne!(
            spell_aura_support(aura_type),
            SpellMechanicSupport::Unknown,
            "aura {aura_type} must be classified as implemented, no-op, or pending"
        );
    }
    assert_eq!(
        spell_aura_support(CMANGOS_TOTAL_AURAS),
        SpellMechanicSupport::Unknown
    );
}

#[test]
fn starter_warrior_spell_templates_have_no_spell_coverage_gaps() {
    for template in [
        heroic_strike_spell_template(),
        battle_shout_spell_template(),
        rend_spell_template(),
        charge_spell_template(),
        charge_stun_spell_template(),
        thunder_clap_spell_template(),
    ] {
        assert_eq!(
            spell_template_coverage_issues(&template),
            Vec::new(),
            "{} should be covered by generic spell machinery",
            template.spell_name
        );
    }
}

#[test]
fn spell_coverage_audit_reports_pending_effects_and_auras() {
    let mut template = test_spell_template(999_900);
    template.effect1 = 28;
    template.effect2 = SPELL_EFFECT_APPLY_AURA;
    template.effect_apply_aura_name2 = 2;

    let issues = spell_template_coverage_issues(&template);
    assert_eq!(issues.len(), 2);
    assert_eq!(issues[0].mechanic, SpellCoverageMechanic::Effect);
    assert_eq!(issues[0].mechanic_id, 28);
    assert_eq!(issues[0].support, SpellMechanicSupport::Pending("summon"));
    assert_eq!(issues[1].mechanic, SpellCoverageMechanic::Aura);
    assert_eq!(issues[1].mechanic_id, 2);
    assert_eq!(
        issues[1].support,
        SpellMechanicSupport::Pending("control state")
    );
}

#[tokio::test]
async fn ranked_aura_conflict_replaces_lower_rank_and_bounces_weaker_recast() {
    let object_mgr = ObjectMgr::default();
    object_mgr
        .prime_spell_chain_for_test(
            1459,
            Some(wow_db::SpellChainQuery {
                spell_id: 1459,
                prev_spell: 0,
                first_spell: 1459,
                rank: 1,
                req_spell: 0,
            }),
        )
        .await;
    object_mgr
        .prime_spell_chain_for_test(
            1460,
            Some(wow_db::SpellChainQuery {
                spell_id: 1460,
                prev_spell: 1459,
                first_spell: 1459,
                rank: 2,
                req_spell: 1459,
            }),
        )
        .await;
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    let now = Instant::now();
    let lower = ActiveAura {
        spell_id: 1459,
        caster,
        level: 1,
        interrupt_flags: 0,
        positive: true,
        visible: true,
        duration_millis: Some(1_800_000),
        expires_at: Some(now + Duration::from_secs(1_800)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::Stat {
            stat: Some(3),
            amount: 5,
        }],
        proc_triggers: Vec::new(),
    };
    let higher = ActiveAura {
        spell_id: 1460,
        stat_modifiers: vec![AuraStatModifier::Stat {
            stat: Some(3),
            amount: 12,
        }],
        ..lower.clone()
    };

    let replace_lower = aura_rank_conflict_resolution(
        &object_mgr,
        &world_db_pool,
        1460,
        caster,
        std::slice::from_ref(&lower),
    )
    .await
    .unwrap();
    assert_eq!(replace_lower.failure, None);
    assert_eq!(replace_lower.replace_spell_ids, vec![1459]);

    let mut active_auras = vec![lower];
    apply_active_aura_replacing_spell_ids(
        &mut active_auras,
        higher.clone(),
        &replace_lower.replace_spell_ids,
    );
    assert_eq!(active_auras.len(), 1);
    assert_eq!(active_auras[0].spell_id, 1460);
    let world_stats = PlayerWorldStats {
        base_health: 20,
        base_mana: 20,
        stats: [10; MAX_STATS],
        next_level_xp: 400,
    };
    assert_eq!(
        player_world_stats_with_active_auras(world_stats, &active_auras).stats[3],
        22
    );

    let refresh_same_rank = aura_rank_conflict_resolution(
        &object_mgr,
        &world_db_pool,
        1460,
        caster,
        std::slice::from_ref(&higher),
    )
    .await
    .unwrap();
    assert_eq!(refresh_same_rank, AuraRankConflictResolution::clear());

    let weaker_recast =
        aura_rank_conflict_resolution(&object_mgr, &world_db_pool, 1459, caster, &[higher])
            .await
            .unwrap();
    assert_eq!(weaker_recast.failure, Some(SPELL_FAILED_AURA_BOUNCED));
    assert!(weaker_recast.replace_spell_ids.is_empty());
    assert!(weaker_recast.replace_any_caster_spell_ids.is_empty());
}

#[tokio::test]
async fn mixed_damage_aura_spell_does_not_precast_bounce_on_stronger_debuff() {
    let object_mgr = ObjectMgr::default();
    for (spell_id, prev_spell, rank) in [(348, 0, 1), (707, 348, 2)] {
        object_mgr
            .prime_spell_chain_for_test(
                spell_id,
                Some(wow_db::SpellChainQuery {
                    spell_id,
                    prev_spell,
                    first_spell: 348,
                    rank,
                    req_spell: prev_spell,
                }),
            )
            .await;
        object_mgr
            .prime_spell_group_memberships_for_test(spell_id, Vec::new())
            .await;
    }
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    let position = WorldPosition::new(0, -8958.0, -132.0, 83.5312, 0.0);
    maps.add_player(test_player_runtime(7, SessionId(7), position))
        .await
        .unwrap();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 901_348;
    let target = creature_spawn_guid(&spawn);
    let mut creature = DbCreatureRuntime::new(spawn);
    creature.active_auras.push(ActiveAura {
        spell_id: 707,
        caster,
        level: 10,
        interrupt_flags: 0,
        positive: false,
        visible: true,
        duration_millis: Some(15_000),
        expires_at: Some(Instant::now() + Duration::from_secs(15)),
        periodic_damage: Some(PeriodicDamageAura {
            aura_name: SPELL_AURA_PERIODIC_DAMAGE,
            school: 2,
            damage_class: SPELL_DAMAGE_CLASS_MAGIC,
            attributes_ex2: TEST_SPELL_ATTR_EX2_CANT_CRIT,
            attributes_ex3: TEST_SPELL_ATTR_EX3_ALWAYS_HIT,
            caster_snapshot: SpellCombatUnitSnapshot {
                level: 10,
                class: 8,
                intellect: 20,
                resistances: [0; MAX_SPELL_SCHOOL],
            },
            amount: 4,
            tick_millis: 3_000,
            next_tick_at: Instant::now() + Duration::from_secs(3),
        }),
        periodic_regen: None,
        stat_modifiers: Vec::new(),
        proc_triggers: Vec::new(),
    });
    maps.share_db_creature_snapshots(0, vec![creature]).await;
    let lower = immolate_spell_template();
    let spell_profile = player_spell_cast_profile(&lower).unwrap();
    assert_eq!(spell_profile.kind, SpellCastKind::AuraApplication);
    let targets = SpellCastTargets {
        target_mask: SPELL_CAST_TARGET_UNIT,
        unit_target: Some(target),
        gameobject_target: None,
        source_location: None,
        destination: None,
    };
    let session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 8,
                level: 10,
                xp: 0,
                position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    let failure = player_aura_rank_cast_failure(
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        &session,
        &lower,
        &spell_profile,
        &targets,
        caster,
    )
    .await
    .unwrap();
    assert_eq!(
        failure, None,
        "CMaNGOS lets mixed direct-damage spells cast even when the aura rider is weaker"
    );
}

#[tokio::test]
async fn failed_hostile_aura_rank_cast_still_pulls_db_creature_aggro() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    for (spell_id, prev_spell, rank) in [(118, 0, 1), (12824, 118, 2)] {
        object_mgr
            .prime_spell_chain_for_test(
                spell_id,
                Some(wow_db::SpellChainQuery {
                    spell_id,
                    prev_spell,
                    first_spell: 118,
                    rank,
                    req_spell: prev_spell,
                }),
            )
            .await;
        object_mgr
            .prime_spell_group_memberships_for_test(spell_id, Vec::new())
            .await;
    }
    let mut lower = test_spell_template(118);
    lower.spell_name = "Polymorph".to_string();
    lower.mechanic = MECHANIC_POLYMORPH;
    lower.aura_interrupt_flags = AURA_INTERRUPT_FLAG_DAMAGE;
    lower.effect1 = SPELL_EFFECT_APPLY_AURA;
    lower.effect_apply_aura_name1 = SPELL_AURA_MOD_CONFUSE;
    lower.effect_implicit_target_a1 = TARGET_UNIT_ENEMY;
    object_mgr
        .prime_spell_template_for_test(118, Some(lower))
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let mut player = test_player_runtime(7, SessionId::next(), player_position);
    player.power1 = 100;
    player.max_power1 = 100;
    maps.add_player(player).await.unwrap();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 908_118;
    spawn.position_x = player_position.x + 2.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    let target = creature_spawn_guid(&spawn);
    let mut creature = DbCreatureRuntime::new(spawn);
    creature.active_auras.push(ActiveAura {
        spell_id: 12824,
        caster: player_guid,
        level: 20,
        interrupt_flags: AURA_INTERRUPT_FLAG_DAMAGE,
        positive: false,
        visible: true,
        duration_millis: Some(30_000),
        expires_at: Some(Instant::now() + Duration::from_secs(30)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::Confuse],
        proc_triggers: Vec::new(),
    });
    maps.share_db_creature_snapshots(0, vec![creature]).await;
    let mut body = Vec::new();
    body.extend_from_slice(&118u32.to_le_bytes());
    body.extend_from_slice(&SPELL_CAST_TARGET_UNIT.to_le_bytes());
    PackedGuid::write(&mut body, target).unwrap();
    let mut active_spells = HashSet::new();
    active_spells.insert(118);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 8,
                level: 12,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_mana: 100,
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 7,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let combats = maps
        .active_db_creature_combats_for_victim(0, player_guid)
        .await;
    assert_eq!(combats.len(), 1);
    assert_eq!(combats[0].attacker, target);
    assert!(maps.player_runtime_snapshot(0, 7).await.unwrap().in_combat);
    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    let opcodes = packets
        .iter()
        .map(|packet| packet.opcode)
        .collect::<Vec<_>>();
    assert!(opcodes.contains(&(WorldOpcode::SmsgCastResult as u16)));
    assert!(!opcodes.contains(&(WorldOpcode::SmsgSpellGo as u16)));
}

#[tokio::test]
async fn resisted_hostile_aura_spell_sends_miss_without_applying_aura_and_pulls_aggro() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let mut polymorph = test_spell_template(118);
    polymorph.spell_name = "Polymorph".to_string();
    polymorph.school = 6;
    polymorph.dmg_class = SPELL_DAMAGE_CLASS_MAGIC;
    polymorph.dispel = 1;
    polymorph.mechanic = MECHANIC_POLYMORPH;
    polymorph.spell_family_name = SPELL_FAMILY_MAGE;
    polymorph.spell_family_flags = 0x0100_0000;
    polymorph.aura_interrupt_flags = AURA_INTERRUPT_FLAG_DAMAGE;
    polymorph.effect1 = SPELL_EFFECT_APPLY_AURA;
    polymorph.effect_apply_aura_name1 = SPELL_AURA_MOD_CONFUSE;
    polymorph.effect_implicit_target_a1 = TARGET_UNIT_ENEMY;
    polymorph.effect2 = SPELL_EFFECT_APPLY_AURA;
    polymorph.effect_apply_aura_name2 = SPELL_AURA_TRANSFORM;
    polymorph.effect_misc_value2 = 16372;
    polymorph.effect_implicit_target_a2 = TARGET_UNIT_ENEMY;
    object_mgr
        .prime_spell_template_for_test(118, Some(polymorph))
        .await;
    object_mgr.prime_spell_chain_for_test(118, None).await;
    object_mgr
        .prime_spell_group_memberships_for_test(118, Vec::new())
        .await;
    object_mgr.prime_spell_facing_flag_for_test(118, None).await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let mut player = test_player_runtime(7, SessionId::next(), player_position);
    player.class = 8;
    player.level = 12;
    player.power1 = 100;
    player.max_power1 = 100;
    maps.add_player(player).await.unwrap();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 908_119;
    spawn.position_x = player_position.x + 2.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    spawn.template.min_level = 100;
    spawn.template.max_level = 100;
    spawn.template.resistance_arcane = i16::MAX;
    let target = creature_spawn_guid(&spawn);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(spawn)])
        .await;
    let mut body = Vec::new();
    body.extend_from_slice(&118u32.to_le_bytes());
    body.extend_from_slice(&SPELL_CAST_TARGET_UNIT.to_le_bytes());
    PackedGuid::write(&mut body, target).unwrap();
    let mut active_spells = HashSet::new();
    active_spells.insert(118);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 8,
                level: 12,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_mana: 100,
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 7,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let creature = maps.db_creature_snapshot(0, target).await.unwrap();
    assert!(
        creature.active_auras.is_empty(),
        "resisted Polymorph must not apply the sheep aura"
    );
    let combats = maps
        .active_db_creature_combats_for_victim(0, player_guid)
        .await;
    assert_eq!(combats.len(), 1);
    assert_eq!(combats[0].attacker, target);
    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgCastResult as u16));
    assert!(
        !packets
            .iter()
            .any(|packet| packet.opcode == WorldOpcode::SmsgSpellLogMiss as u16),
        "normal spell-target resist is already encoded in SMSG_SPELL_GO; sending SMSG_SPELLLOGMISS too duplicates client resist feedback"
    );
    let spell_go = packets
        .iter()
        .find(|packet| packet.opcode == WorldOpcode::SmsgSpellGo as u16)
        .expect("resisted hostile aura should still send SMSG_SPELL_GO with miss data");
    let mut cursor = PackedGuid::packed_size(player_guid) * 2 + 4;
    assert_eq!(
        u16::from_le_bytes(spell_go.body[cursor..cursor + 2].try_into().unwrap()),
        CAST_FLAG_SPELL_GO
    );
    cursor += 2;
    assert_eq!(spell_go.body[cursor], 0, "missed target is not a hit");
    cursor += 1;
    assert_eq!(
        spell_go.body[cursor], 1,
        "one missed target should be listed"
    );
    cursor += 1;
    assert_eq!(
        u64::from_le_bytes(spell_go.body[cursor..cursor + 8].try_into().unwrap()),
        target.raw()
    );
    cursor += 8;
    assert_eq!(spell_go.body[cursor], SPELL_MISS_RESIST);
}

#[tokio::test]
async fn resisted_hostile_direct_damage_spell_sends_go_miss_without_damage_or_miss_log() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    object_mgr
        .prime_spell_template_for_test(999_120, Some(instant_firebolt_spell_template()))
        .await;
    object_mgr
        .prime_creature_ai_scripts_for_test(6, Vec::new())
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let mut player = test_player_runtime(7, SessionId::next(), player_position);
    player.class = 8;
    player.level = 12;
    player.power1 = 100;
    player.max_power1 = 100;
    maps.add_player(player).await.unwrap();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 908_120;
    spawn.position_x = player_position.x + 2.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    spawn.template.min_level = 100;
    spawn.template.max_level = 100;
    spawn.template.min_level_health = 120;
    spawn.template.max_level_health = 120;
    spawn.template.resistance_fire = i16::MAX;
    let target = creature_spawn_guid(&spawn);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(spawn)])
        .await;
    let mut body = Vec::new();
    body.extend_from_slice(&999_120u32.to_le_bytes());
    body.extend_from_slice(&SPELL_CAST_TARGET_UNIT.to_le_bytes());
    PackedGuid::write(&mut body, target).unwrap();
    let mut active_spells = HashSet::new();
    active_spells.insert(999_120);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 8,
                level: 12,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_mana: 100,
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 7,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let creature = maps.db_creature_snapshot(0, target).await.unwrap();
    assert_eq!(creature.health, 120, "resisted direct damage must not land");
    let combats = maps
        .active_db_creature_combats_for_victim(0, player_guid)
        .await;
    assert_eq!(combats.len(), 1);
    assert_eq!(combats[0].attacker, target);
    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(!packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellNonMeleeDamageLog as u16));
    assert!(!packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellLogMiss as u16));
    let spell_go = packets
        .iter()
        .find(|packet| packet.opcode == WorldOpcode::SmsgSpellGo as u16)
        .expect("resisted direct damage should still send SMSG_SPELL_GO with miss data");
    assert_spell_go_single_resist_miss(spell_go, player_guid, target);
}

#[tokio::test]
async fn resisted_damage_plus_aura_spell_skips_damage_and_aura_from_same_target_outcome() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    object_mgr
        .prime_spell_template_for_test(999_121, Some(frostbolt_like_spell_template()))
        .await;
    object_mgr
        .prime_creature_ai_scripts_for_test(6, Vec::new())
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(7, SessionId::next(), player_position);
    player.class = 8;
    player.level = 12;
    player.power1 = 100;
    player.max_power1 = 100;
    maps.add_player(player).await.unwrap();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 908_121;
    spawn.position_x = player_position.x + 2.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    spawn.template.min_level = 100;
    spawn.template.max_level = 100;
    spawn.template.min_level_health = 120;
    spawn.template.max_level_health = 120;
    spawn.template.resistance_frost = i16::MAX;
    let target = creature_spawn_guid(&spawn);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(spawn)])
        .await;
    let mut body = Vec::new();
    body.extend_from_slice(&999_121u32.to_le_bytes());
    body.extend_from_slice(&SPELL_CAST_TARGET_UNIT.to_le_bytes());
    PackedGuid::write(&mut body, target).unwrap();
    let mut active_spells = HashSet::new();
    active_spells.insert(999_121);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 8,
                level: 12,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_mana: 100,
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 7,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let creature = maps.db_creature_snapshot(0, target).await.unwrap();
    assert_eq!(creature.health, 120);
    assert!(
        creature.active_auras.is_empty(),
        "a resisted damage+slow spell must not apply the follow-up aura"
    );
    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(!packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellNonMeleeDamageLog as u16));
    let spell_go = packets
        .iter()
        .find(|packet| packet.opcode == WorldOpcode::SmsgSpellGo as u16)
        .expect("resisted damage+aura spell should send SMSG_SPELL_GO with miss data");
    assert_spell_go_single_resist_miss(spell_go, ObjectGuid::new(HighGuid::Player, 0, 7), target);
}

fn assert_spell_go_single_resist_miss(
    spell_go: &OutboundWorldPacket,
    caster_guid: ObjectGuid,
    target: ObjectGuid,
) {
    assert_spell_go_single_resist_miss_with_source(spell_go, caster_guid, caster_guid, target);
}

fn assert_spell_go_single_resist_miss_with_source(
    spell_go: &OutboundWorldPacket,
    source_guid: ObjectGuid,
    caster_guid: ObjectGuid,
    target: ObjectGuid,
) {
    let mut cursor =
        PackedGuid::packed_size(source_guid) + PackedGuid::packed_size(caster_guid) + 4;
    assert_eq!(
        u16::from_le_bytes(spell_go.body[cursor..cursor + 2].try_into().unwrap())
            & CAST_FLAG_SPELL_GO,
        CAST_FLAG_SPELL_GO
    );
    cursor += 2;
    assert_eq!(spell_go.body[cursor], 0, "missed target is not a hit");
    cursor += 1;
    assert_eq!(
        spell_go.body[cursor], 1,
        "one missed target should be listed"
    );
    cursor += 1;
    assert_eq!(
        u64::from_le_bytes(spell_go.body[cursor..cursor + 8].try_into().unwrap()),
        target.raw()
    );
    cursor += 8;
    assert_eq!(spell_go.body[cursor], SPELL_MISS_RESIST);
}

#[tokio::test]
async fn resisted_item_hostile_damage_spell_sends_go_miss_without_damage_or_miss_log() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    object_mgr
        .prime_creature_ai_scripts_for_test(6, Vec::new())
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(7, SessionId::next(), player_position);
    player.class = 8;
    player.level = 12;
    maps.add_player(player).await.unwrap();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 908_122;
    spawn.position_x = player_position.x + 2.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    spawn.template.min_level = 100;
    spawn.template.max_level = 100;
    spawn.template.min_level_health = 120;
    spawn.template.max_level_health = 120;
    spawn.template.resistance_fire = i16::MAX;
    let target = creature_spawn_guid(&spawn);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(spawn)])
        .await;
    let item_guid = ObjectGuid::new(HighGuid::Item, 0, 44_001);
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    let spell_template = item_firebolt_spell_template();
    let mut prepared_spell = SpellInfo::from_template(&spell_template)
        .prepare_item_cast(item_guid)
        .expect("hostile item damage spell should prepare");
    prepared_spell.start_casting();
    let item_spell_profile = prepared_spell.profile;
    let targets = SpellCastTargets {
        target_mask: SPELL_CAST_TARGET_UNIT,
        unit_target: Some(target),
        gameobject_target: None,
        source_location: None,
        destination: None,
    };
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 8,
                level: 12,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    complete_item_use_spell_cast(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 7,
            shared_world,
            parties: &PartyManager::default(),
        },
        &mut session,
        caster,
        prepared_spell,
        spell_template,
        item_spell_profile,
        CharacterInventoryItem {
            bag: 0,
            slot: 0,
            item: 44_001,
            item_template: 55_001,
            count: 1,
            random_property_id: 0,
            charges: "0 0 0 0 0".to_string(),
            enchantments: String::new(),
            durability: 0,
        },
        0,
        targets,
        Instant::now(),
        &mut header_crypto,
    )
    .await
    .unwrap();

    let creature = maps.db_creature_snapshot(0, target).await.unwrap();
    assert_eq!(creature.health, 120, "resisted item damage must not land");
    let combats = maps.active_db_creature_combats_for_victim(0, caster).await;
    assert_eq!(combats.len(), 1);
    assert_eq!(combats[0].attacker, target);
    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(!packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellNonMeleeDamageLog as u16));
    assert!(!packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellLogMiss as u16));
    let spell_go = packets
        .iter()
        .find(|packet| packet.opcode == WorldOpcode::SmsgSpellGo as u16)
        .expect("resisted item damage should still send SMSG_SPELL_GO with miss data");
    assert_spell_go_single_resist_miss_with_source(spell_go, item_guid, caster, target);
}

#[tokio::test]
async fn successful_item_hostile_damage_spell_damages_db_creature() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    object_mgr
        .prime_creature_ai_scripts_for_test(6, Vec::new())
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(7, SessionId::next(), player_position);
    player.class = 8;
    player.level = 12;
    maps.add_player(player).await.unwrap();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 908_123;
    spawn.position_x = player_position.x + 2.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    spawn.template.min_level = 1;
    spawn.template.max_level = 1;
    spawn.template.min_level_health = 120;
    spawn.template.max_level_health = 120;
    let target = creature_spawn_guid(&spawn);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(spawn)])
        .await;
    let item_guid = ObjectGuid::new(HighGuid::Item, 0, 44_002);
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    let mut spell_template = item_firebolt_spell_template();
    spell_template.attributes_ex3 = TEST_SPELL_ATTR_EX3_ALWAYS_HIT;
    let mut prepared_spell = SpellInfo::from_template(&spell_template)
        .prepare_item_cast(item_guid)
        .expect("hostile item damage spell should prepare");
    prepared_spell.start_casting();
    let item_spell_profile = prepared_spell.profile;
    let targets = SpellCastTargets {
        target_mask: SPELL_CAST_TARGET_UNIT,
        unit_target: Some(target),
        gameobject_target: None,
        source_location: None,
        destination: None,
    };
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 8,
                level: 12,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    complete_item_use_spell_cast(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 7,
            shared_world,
            parties: &PartyManager::default(),
        },
        &mut session,
        caster,
        prepared_spell,
        spell_template,
        item_spell_profile,
        CharacterInventoryItem {
            bag: 0,
            slot: 0,
            item: 44_002,
            item_template: 55_002,
            count: 1,
            random_property_id: 0,
            charges: "0 0 0 0 0".to_string(),
            enchantments: String::new(),
            durability: 0,
        },
        0,
        targets,
        Instant::now(),
        &mut header_crypto,
    )
    .await
    .unwrap();

    let creature = maps.db_creature_snapshot(0, target).await.unwrap();
    assert!(
        creature.health < 120,
        "a hit hostile item spell should apply its DB-creature damage"
    );
    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellNonMeleeDamageLog as u16));
    let spell_go = packets
        .iter()
        .find(|packet| packet.opcode == WorldOpcode::SmsgSpellGo as u16)
        .expect("hit item damage should send SMSG_SPELL_GO");
    let mut cursor = PackedGuid::packed_size(item_guid) + PackedGuid::packed_size(caster) + 4;
    let cast_flags = u16::from_le_bytes(spell_go.body[cursor..cursor + 2].try_into().unwrap());
    assert_eq!(cast_flags & CAST_FLAG_ITEM_CASTER, CAST_FLAG_ITEM_CASTER);
    cursor += 2;
    assert_eq!(spell_go.body[cursor], 1, "target should be listed as a hit");
}

#[tokio::test]
async fn ranked_aura_conflict_bounces_stronger_other_caster_and_replaces_weaker() {
    let object_mgr = ObjectMgr::default();
    for (spell_id, prev_spell, rank) in [(1459, 0, 1), (1460, 1459, 2)] {
        object_mgr
            .prime_spell_chain_for_test(
                spell_id,
                Some(wow_db::SpellChainQuery {
                    spell_id,
                    prev_spell,
                    first_spell: 1459,
                    rank,
                    req_spell: prev_spell,
                }),
            )
            .await;
        object_mgr
            .prime_spell_group_memberships_for_test(spell_id, Vec::new())
            .await;
    }
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    let other_caster = ObjectGuid::new(HighGuid::Player, 0, 8);
    let now = Instant::now();
    let lower_from_other = ActiveAura {
        spell_id: 1459,
        caster: other_caster,
        level: 1,
        interrupt_flags: 0,
        positive: true,
        visible: true,
        duration_millis: Some(1_800_000),
        expires_at: Some(now + Duration::from_secs(1_800)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: Vec::new(),
        proc_triggers: Vec::new(),
    };
    let higher_from_other = ActiveAura {
        spell_id: 1460,
        ..lower_from_other.clone()
    };

    let stronger_existing = aura_rank_conflict_resolution(
        &object_mgr,
        &world_db_pool,
        1459,
        caster,
        std::slice::from_ref(&higher_from_other),
    )
    .await
    .unwrap();
    assert_eq!(stronger_existing.failure, Some(SPELL_FAILED_AURA_BOUNCED));

    let replace_weaker = aura_rank_conflict_resolution(
        &object_mgr,
        &world_db_pool,
        1460,
        caster,
        &[lower_from_other],
    )
    .await
    .unwrap();
    assert_eq!(replace_weaker.failure, None);
    assert!(replace_weaker.replace_spell_ids.is_empty());
    assert_eq!(replace_weaker.replace_any_caster_spell_ids, vec![1459]);

    let stronger_debuff_from_other = ActiveAura {
        positive: false,
        ..higher_from_other
    };
    let other_caster_debuff = aura_rank_conflict_resolution(
        &object_mgr,
        &world_db_pool,
        1459,
        caster,
        &[stronger_debuff_from_other],
    )
    .await
    .unwrap();
    assert_eq!(other_caster_debuff, AuraRankConflictResolution::clear());
}

#[tokio::test]
async fn spell_group_conflict_resolution_uses_unique_and_unique_per_caster_rules() {
    let object_mgr = ObjectMgr::default();
    for spell_id in [11_001, 11_002, 11_003, 11_004] {
        object_mgr.prime_spell_chain_for_test(spell_id, None).await;
    }
    object_mgr
        .prime_spell_group_memberships_for_test(
            11_001,
            vec![wow_db::SpellGroupMembershipQuery {
                spell_id: 11_001,
                group_id: 13,
                rule: SPELL_GROUP_RULE_UNIQUE,
            }],
        )
        .await;
    object_mgr
        .prime_spell_group_memberships_for_test(
            11_002,
            vec![wow_db::SpellGroupMembershipQuery {
                spell_id: 11_002,
                group_id: 13,
                rule: SPELL_GROUP_RULE_UNIQUE,
            }],
        )
        .await;
    object_mgr
        .prime_spell_group_memberships_for_test(
            11_003,
            vec![wow_db::SpellGroupMembershipQuery {
                spell_id: 11_003,
                group_id: 19,
                rule: SPELL_GROUP_RULE_UNIQUE_PER_CASTER,
            }],
        )
        .await;
    object_mgr
        .prime_spell_group_memberships_for_test(
            11_004,
            vec![wow_db::SpellGroupMembershipQuery {
                spell_id: 11_004,
                group_id: 19,
                rule: SPELL_GROUP_RULE_UNIQUE_PER_CASTER,
            }],
        )
        .await;
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    let other_caster = ObjectGuid::new(HighGuid::Player, 0, 8);
    let now = Instant::now();
    let unique_existing = ActiveAura {
        spell_id: 11_001,
        caster: other_caster,
        level: 1,
        interrupt_flags: 0,
        positive: true,
        visible: true,
        duration_millis: Some(60_000),
        expires_at: Some(now + Duration::from_secs(60)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: Vec::new(),
        proc_triggers: Vec::new(),
    };

    let unique = aura_rank_conflict_resolution(
        &object_mgr,
        &world_db_pool,
        11_002,
        caster,
        std::slice::from_ref(&unique_existing),
    )
    .await
    .unwrap();
    assert_eq!(unique.replace_any_caster_spell_ids, vec![11_001]);

    let same_caster_personal = ActiveAura {
        spell_id: 11_003,
        caster,
        ..unique_existing.clone()
    };
    let other_caster_personal = ActiveAura {
        spell_id: 11_003,
        caster: other_caster,
        ..unique_existing
    };
    let personal = aura_rank_conflict_resolution(
        &object_mgr,
        &world_db_pool,
        11_004,
        caster,
        &[same_caster_personal, other_caster_personal],
    )
    .await
    .unwrap();
    assert_eq!(personal.replace_spell_ids, vec![11_003]);
    assert!(personal.replace_any_caster_spell_ids.is_empty());
}

#[test]
fn direct_friendly_unit_aura_targets_require_a_friendly_unit() {
    let mut intellect = test_spell_template(1459);
    intellect.effect1 = SPELL_EFFECT_APPLY_AURA;
    intellect.effect_apply_aura_name1 = SPELL_AURA_MOD_STAT;
    intellect.effect_implicit_target_a1 = TARGET_UNIT_FRIEND;

    let spell_info = SpellInfo::from_template(&intellect);
    assert_eq!(spell_info.aura_target(), SpellAuraTarget::UnitTarget);
    assert_eq!(
        spell_info.unit_target_kind(SpellCastKind::AuraApplication),
        SpellTargetKind::FriendlyUnit
    );
}

#[test]
fn spell_cast_profiles_are_derived_from_cmangos_spell_template_fields() {
    assert_eq!(
        player_spell_cast_profile(&heroic_strike_spell_template()),
        Some(SpellCastProfile {
            spell_id: WARRIOR_HEROIC_STRIKE_RANK_1,
            kind: SpellCastKind::NextMeleeSwing,
            aura_target: SpellAuraTarget::Caster,
            bonus_damage: HEROIC_STRIKE_FIXTURE_DAMAGE,
            weapon_damage_percent: 100,
            damage: 0,
            power: SpellPowerCost::Rage {
                cost: HEROIC_STRIKE_RAGE_COST
            },
            requires_melee: true,
            requires_behind: false,
            needs_combo_points: false,
            global_cooldown_category: 0,
            global_cooldown_millis: 0,
            cooldown_category: 0,
            category_cooldown_millis: 0,
            cooldown_millis: 0,
        })
    );
    assert_eq!(
        player_spell_cast_profile(&raptor_strike_spell_template()),
        Some(SpellCastProfile {
            spell_id: HUNTER_RAPTOR_STRIKE_RANK_1,
            kind: SpellCastKind::NextMeleeSwing,
            aura_target: SpellAuraTarget::Caster,
            bonus_damage: 5,
            weapon_damage_percent: 100,
            damage: 0,
            power: SpellPowerCost::Mana {
                cost: RAPTOR_STRIKE_MANA_COST
            },
            requires_melee: true,
            requires_behind: false,
            needs_combo_points: false,
            global_cooldown_category: 0,
            global_cooldown_millis: 0,
            cooldown_category: 0,
            category_cooldown_millis: 6000,
            cooldown_millis: 0,
        })
    );
    assert_eq!(
        player_spell_cast_profile(&auto_shot_spell_template()),
        Some(SpellCastProfile {
            spell_id: 75,
            kind: SpellCastKind::AutoRepeatRanged,
            aura_target: SpellAuraTarget::Caster,
            bonus_damage: 0,
            weapon_damage_percent: 100,
            damage: 0,
            power: SpellPowerCost::Mana { cost: 0 },
            requires_melee: false,
            requires_behind: false,
            needs_combo_points: false,
            global_cooldown_category: 0,
            global_cooldown_millis: 0,
            cooldown_category: 0,
            category_cooldown_millis: 0,
            cooldown_millis: 0,
        })
    );
    assert_eq!(
        player_spell_cast_profile(&battle_shout_spell_template()),
        Some(SpellCastProfile {
            spell_id: 6673,
            kind: SpellCastKind::AuraApplication,
            aura_target: SpellAuraTarget::Caster,
            bonus_damage: 0,
            weapon_damage_percent: 100,
            damage: 0,
            power: SpellPowerCost::Rage { cost: 100 },
            requires_melee: false,
            requires_behind: false,
            needs_combo_points: false,
            global_cooldown_category: 133,
            global_cooldown_millis: 1500,
            cooldown_category: 0,
            category_cooldown_millis: 0,
            cooldown_millis: 0,
        })
    );
    assert_eq!(
        player_spell_cast_profile(&fireball_spell_template()),
        Some(SpellCastProfile {
            spell_id: 133,
            kind: SpellCastKind::InstantDamage,
            aura_target: SpellAuraTarget::Caster,
            bonus_damage: 0,
            weapon_damage_percent: 100,
            damage: 14,
            power: SpellPowerCost::Mana { cost: 25 },
            requires_melee: false,
            requires_behind: false,
            needs_combo_points: false,
            global_cooldown_category: 133,
            global_cooldown_millis: 1500,
            cooldown_category: 0,
            category_cooldown_millis: 0,
            cooldown_millis: 0,
        })
    );
    assert_eq!(
        player_spell_cast_profile(&lesser_heal_spell_template()),
        Some(SpellCastProfile {
            spell_id: 2050,
            kind: SpellCastKind::DirectHeal,
            aura_target: SpellAuraTarget::Caster,
            bonus_damage: 0,
            weapon_damage_percent: 100,
            damage: 0,
            power: SpellPowerCost::Mana { cost: 25 },
            requires_melee: false,
            requires_behind: false,
            needs_combo_points: false,
            global_cooldown_category: 133,
            global_cooldown_millis: 1500,
            cooldown_category: 0,
            category_cooldown_millis: 0,
            cooldown_millis: 0,
        })
    );
    assert_eq!(
        player_spell_cast_profile(&sinister_strike_spell_template()),
        Some(SpellCastProfile {
            spell_id: 1752,
            kind: SpellCastKind::InstantDamage,
            aura_target: SpellAuraTarget::Caster,
            bonus_damage: 3,
            weapon_damage_percent: 100,
            damage: 3,
            power: SpellPowerCost::Energy { cost: 45 },
            requires_melee: true,
            requires_behind: false,
            needs_combo_points: false,
            global_cooldown_category: 133,
            global_cooldown_millis: 1500,
            cooldown_category: 0,
            category_cooldown_millis: 0,
            cooldown_millis: 0,
        })
    );
    assert_eq!(
        player_spell_cast_profile(&backstab_spell_template()),
        Some(SpellCastProfile {
            spell_id: 53,
            kind: SpellCastKind::InstantDamage,
            aura_target: SpellAuraTarget::Caster,
            bonus_damage: 15,
            weapon_damage_percent: 150,
            damage: 15,
            power: SpellPowerCost::Energy { cost: 60 },
            requires_melee: true,
            requires_behind: true,
            needs_combo_points: false,
            global_cooldown_category: 133,
            global_cooldown_millis: 1500,
            cooldown_category: 0,
            category_cooldown_millis: 0,
            cooldown_millis: 0,
        })
    );
    assert_eq!(
        player_spell_cast_profile(&rend_spell_template()),
        Some(SpellCastProfile {
            spell_id: 772,
            kind: SpellCastKind::AuraApplication,
            aura_target: SpellAuraTarget::UnitTarget,
            bonus_damage: 0,
            weapon_damage_percent: 100,
            damage: 0,
            power: SpellPowerCost::Rage { cost: 100 },
            requires_melee: true,
            requires_behind: false,
            needs_combo_points: false,
            global_cooldown_category: 133,
            global_cooldown_millis: 1500,
            cooldown_category: 0,
            category_cooldown_millis: 0,
            cooldown_millis: 0,
        })
    );
    assert_eq!(
        player_spell_cast_profile(&charge_spell_template()),
        Some(SpellCastProfile {
            spell_id: 100,
            kind: SpellCastKind::Charge,
            aura_target: SpellAuraTarget::Caster,
            bonus_damage: 0,
            weapon_damage_percent: 100,
            damage: 0,
            power: SpellPowerCost::Rage { cost: 0 },
            requires_melee: false,
            requires_behind: false,
            needs_combo_points: false,
            global_cooldown_category: 133,
            global_cooldown_millis: 1500,
            cooldown_category: 0,
            category_cooldown_millis: 0,
            cooldown_millis: 0,
        })
    );
    assert_eq!(player_spell_cast_profile(&test_spell_template(1)), None);
}

#[test]
fn hearthstone_is_supported_as_item_teleport_spell_not_equip_failure() {
    assert_eq!(
        item_use_spell_cast_profile(&hearthstone_spell_template()),
        Some(SpellCastProfile {
            spell_id: 8690,
            kind: SpellCastKind::Teleport,
            aura_target: SpellAuraTarget::Caster,
            bonus_damage: 0,
            weapon_damage_percent: 100,
            damage: 0,
            power: SpellPowerCost::Mana { cost: 0 },
            requires_melee: false,
            requires_behind: false,
            needs_combo_points: false,
            global_cooldown_category: 133,
            global_cooldown_millis: 1500,
            cooldown_category: 0,
            category_cooldown_millis: 0,
            cooldown_millis: 0,
        })
    );
}

#[tokio::test]
async fn player_damage_spell_executes_each_damage_effect_slot() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    object_mgr
        .prime_spell_template_for_test(999_011, Some(two_school_damage_effects_spell_template()))
        .await;
    object_mgr
        .prime_creature_ai_scripts_for_test(6, Vec::new())
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let mut kobold = test_creature_spawn(6);
    kobold.guid = 45;
    kobold.position_x = -8947.0;
    kobold.position_y = -132.0;
    kobold.position_z = 83.5312;
    let target = creature_spawn_guid(&kobold);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(kobold)])
        .await;
    let player_position = WorldPosition::new(0, -8949.95, -132.493, 83.5312, 0.0);
    let mut player = test_player_runtime(7, SessionId::next(), player_position);
    player.power1 = 100;
    player.max_power1 = 100;
    maps.add_player(player).await.unwrap();
    let mut body = Vec::new();
    body.extend_from_slice(&999_011u32.to_le_bytes());
    body.extend_from_slice(&SPELL_CAST_TARGET_UNIT.to_le_bytes());
    PackedGuid::write(&mut body, target).unwrap();
    let mut active_spells = HashSet::new();
    active_spells.insert(999_011);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let creature = maps.db_creature_snapshot(0, target).await.unwrap();
    assert_eq!(120 - creature.health, 12);
    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    let damage_logs = packets
        .iter()
        .filter(|packet| packet.opcode == WorldOpcode::SmsgSpellNonMeleeDamageLog as u16)
        .collect::<Vec<_>>();
    assert_eq!(
        damage_logs.len(),
        2,
        "each SpellInfo damage effect slot should execute independently"
    );
    let mut logged_damage = Vec::new();
    for packet in damage_logs {
        let mut cursor = 0;
        assert_eq!(read_packed_guid(&packet.body, &mut cursor).unwrap(), target);
        assert_eq!(
            read_packed_guid(&packet.body, &mut cursor).unwrap(),
            ObjectGuid::new(HighGuid::Player, 0, 7)
        );
        assert_eq!(read_u32(&packet.body, &mut cursor).unwrap(), 999_011);
        logged_damage.push(read_u32(&packet.body, &mut cursor).unwrap());
        assert_eq!(packet.body[cursor], 4);
    }
    assert_eq!(logged_damage, vec![5, 7]);
    assert!(
        !packets
            .iter()
            .any(|packet| packet.opcode == WorldOpcode::SmsgAttackerStateUpdate as u16),
        "school-damage spells use spell damage logs, not melee attacker-state logs"
    );
}

#[tokio::test]
async fn sinister_strike_cast_uses_energy_and_spell_damage_log_result() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    object_mgr
        .prime_spell_template_for_test(1752, Some(sinister_strike_spell_template()))
        .await;
    object_mgr
        .prime_creature_ai_scripts_for_test(6, Vec::new())
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let mut kobold = test_creature_spawn(6);
    kobold.guid = 46;
    kobold.position_x = 1.0;
    kobold.position_y = 0.0;
    kobold.position_z = 0.0;
    kobold.template.min_level_health = 200;
    kobold.template.max_level_health = 200;
    let target = creature_spawn_guid(&kobold);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(kobold)])
        .await;
    let player_position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    let mut player = test_player_runtime(7, SessionId::next(), player_position);
    player.class = 4;
    player.power4 = POWER_ENERGY_DEFAULT;
    player.max_power4 = POWER_ENERGY_DEFAULT;
    maps.add_player(player).await.unwrap();
    maps.update_player_selection(0, 7, Some(target))
        .await
        .unwrap();
    let mut body = Vec::new();
    body.extend_from_slice(&1752u32.to_le_bytes());
    body.extend_from_slice(&0u16.to_le_bytes());
    let mut active_spells = HashSet::new();
    active_spells.insert(1752);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 4,
                level: 1,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_energy: POWER_ENERGY_DEFAULT,
            character_skills: vec![test_skill(SKILL_UNARMED, 500, 500)],
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            shared_world,
            parties: &PartyManager::default(),
            account_id: 7,
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let snapshot = maps.player_runtime_snapshot(0, 7).await.unwrap();
    assert_eq!(snapshot.power4, 55);
    assert_eq!(snapshot.combo_target, Some(target));
    assert_eq!(snapshot.combo_points, 1);
    assert!(snapshot.queued_next_melee_spell.is_none());
    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    let spell_damage = packets
        .iter()
        .find(|packet| packet.opcode == WorldOpcode::SmsgSpellNonMeleeDamageLog as u16)
        .expect("instant weapon spell should emit CMaNGOS spell damage log");
    let mut cursor = 0;
    assert_eq!(
        read_packed_guid(&spell_damage.body, &mut cursor).unwrap(),
        target
    );
    assert_eq!(
        read_packed_guid(&spell_damage.body, &mut cursor).unwrap(),
        ObjectGuid::new(HighGuid::Player, 0, 7)
    );
    assert_eq!(read_u32(&spell_damage.body, &mut cursor).unwrap(), 1752);
    assert!(
        read_u32(&spell_damage.body, &mut cursor).unwrap() > 0,
        "yellow weapon-special log should carry the landed damage"
    );
    assert!(
        !packets
            .iter()
            .any(|packet| packet.opcode == WorldOpcode::SmsgAttackerStateUpdate as u16),
        "weapon specials should not masquerade as white-swing attacker-state packets"
    );
}

#[tokio::test]
async fn backstab_cast_requires_caster_behind_target_from_spell_metadata() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    object_mgr
        .prime_spell_template_for_test(53, Some(backstab_spell_template()))
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let mut kobold = test_creature_spawn(6);
    kobold.guid = 46;
    kobold.position_x = 1.0;
    kobold.position_y = 0.0;
    kobold.position_z = 0.0;
    kobold.orientation = 0.0;
    kobold.template.min_level_health = 200;
    kobold.template.max_level_health = 200;
    let target = creature_spawn_guid(&kobold);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(kobold)])
        .await;
    let player_position = WorldPosition::new(0, 2.0, 0.0, 0.0, std::f32::consts::PI);
    let mut player = test_player_runtime(7, SessionId::next(), player_position);
    player.class = 4;
    player.power4 = POWER_ENERGY_DEFAULT;
    player.max_power4 = POWER_ENERGY_DEFAULT;
    maps.add_player(player).await.unwrap();
    maps.update_player_selection(0, 7, Some(target))
        .await
        .unwrap();
    let mut body = Vec::new();
    body.extend_from_slice(&53u32.to_le_bytes());
    body.extend_from_slice(&0u16.to_le_bytes());
    let mut active_spells = HashSet::new();
    active_spells.insert(53);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 4,
                level: 1,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_energy: POWER_ENERGY_DEFAULT,
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            shared_world,
            parties: &PartyManager::default(),
            account_id: 7,
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let snapshot = maps.player_runtime_snapshot(0, 7).await.unwrap();
    assert_eq!(snapshot.power4, POWER_ENERGY_DEFAULT);
    assert_eq!(snapshot.combo_points, 0);
    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(packets.iter().any(|packet| {
        packet.opcode == WorldOpcode::SmsgCastResult as u16
            && packet.body[5] == SPELL_FAILED_NOT_BEHIND
    }));
    assert!(!packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgAttackerStateUpdate as u16));
}

#[tokio::test]
async fn eviscerate_requires_matching_combo_points_before_spending_energy() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    object_mgr
        .prime_spell_template_for_test(2098, Some(eviscerate_spell_template()))
        .await;
    object_mgr
        .prime_creature_ai_scripts_for_test(6, Vec::new())
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let mut kobold = test_creature_spawn(6);
    kobold.guid = 46;
    kobold.position_x = 1.0;
    kobold.position_y = 0.0;
    kobold.position_z = 0.0;
    kobold.template.min_level_health = 200;
    kobold.template.max_level_health = 200;
    let target = creature_spawn_guid(&kobold);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(kobold)])
        .await;
    let player_position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    let mut player = test_player_runtime(7, SessionId::next(), player_position);
    player.class = 4;
    player.power4 = POWER_ENERGY_DEFAULT;
    player.max_power4 = POWER_ENERGY_DEFAULT;
    maps.add_player(player).await.unwrap();
    maps.update_player_selection(0, 7, Some(target))
        .await
        .unwrap();
    let mut body = Vec::new();
    body.extend_from_slice(&2098u32.to_le_bytes());
    body.extend_from_slice(&0u16.to_le_bytes());
    let mut active_spells = HashSet::new();
    active_spells.insert(2098);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 4,
                level: 1,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_energy: POWER_ENERGY_DEFAULT,
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            shared_world,
            parties: &PartyManager::default(),
            account_id: 7,
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let snapshot = maps.player_runtime_snapshot(0, 7).await.unwrap();
    assert_eq!(snapshot.power4, POWER_ENERGY_DEFAULT);
    assert_eq!(snapshot.combo_points, 0);
    let creature = maps.db_creature_snapshot(0, target).await.unwrap();
    assert_eq!(creature.health, 200);
    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(packets.iter().any(|packet| {
        packet.opcode == WorldOpcode::SmsgCastResult as u16
            && packet.body[5] == SPELL_FAILED_NO_COMBO_POINTS
    }));
}

#[tokio::test]
async fn eviscerate_uses_combo_points_for_damage_and_clears_them_on_hit() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    object_mgr
        .prime_spell_template_for_test(2098, Some(eviscerate_spell_template()))
        .await;
    object_mgr
        .prime_creature_ai_scripts_for_test(6, Vec::new())
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let mut kobold = test_creature_spawn(6);
    kobold.guid = 46;
    kobold.position_x = 1.0;
    kobold.position_y = 0.0;
    kobold.position_z = 0.0;
    kobold.template.min_level_health = 200;
    kobold.template.max_level_health = 200;
    let target = creature_spawn_guid(&kobold);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(kobold)])
        .await;
    let player_position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    let mut player = test_player_runtime(7, SessionId::next(), player_position);
    player.class = 4;
    player.power4 = POWER_ENERGY_DEFAULT;
    player.max_power4 = POWER_ENERGY_DEFAULT;
    maps.add_player(player).await.unwrap();
    maps.update_player_selection(0, 7, Some(target))
        .await
        .unwrap();
    maps.add_player_combo_points(0, 7, target, 2).await.unwrap();
    let mut body = Vec::new();
    body.extend_from_slice(&2098u32.to_le_bytes());
    body.extend_from_slice(&0u16.to_le_bytes());
    let mut active_spells = HashSet::new();
    active_spells.insert(2098);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 4,
                level: 1,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_energy: POWER_ENERGY_DEFAULT,
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            shared_world,
            parties: &PartyManager::default(),
            account_id: 7,
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let snapshot = maps.player_runtime_snapshot(0, 7).await.unwrap();
    assert_eq!(snapshot.power4, 65);
    assert_eq!(snapshot.combo_target, None);
    assert_eq!(snapshot.combo_points, 0);
    let creature = maps.db_creature_snapshot(0, target).await.unwrap();
    let damage = 200 - creature.health;
    assert!(
        (11..=15).contains(&damage),
        "Eviscerate rank 1 should roll 1..5 plus 5 per combo point"
    );
    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellNonMeleeDamageLog as u16));
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgUpdateObject as u16));
}

#[tokio::test]
async fn player_heal_spell_cast_restores_self_health_through_map_runtime() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let mut heal = lesser_heal_spell_template();
    heal.casting_time_index = 0;
    object_mgr
        .prime_spell_template_for_test(2050, Some(heal))
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    let mut player = test_player_runtime(7, SessionId::next(), player_position);
    player.class = 5;
    player.health = 10;
    player.max_health = 30;
    player.power1 = 100;
    player.max_power1 = 100;
    maps.add_player(player).await.unwrap();
    let mut body = Vec::new();
    body.extend_from_slice(&2050u32.to_le_bytes());
    body.extend_from_slice(&0u16.to_le_bytes());
    let mut active_spells = HashSet::new();
    active_spells.insert(2050);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 5,
                level: 1,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_health: 10,
            player_mana: 100,
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 7,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let snapshot = maps.player_runtime_snapshot(0, 7).await.unwrap();
    assert_eq!(snapshot.health, 30);
    assert_eq!(snapshot.power1, 75);
    assert_eq!(session.character.player_health, 30);
    assert_eq!(session.character.player_mana, 75);
    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    let opcodes = packets
        .iter()
        .map(|packet| packet.opcode)
        .collect::<Vec<_>>();
    assert!(opcodes.contains(&(WorldOpcode::SmsgCastResult as u16)));
    assert!(opcodes.contains(&(WorldOpcode::SmsgSpellGo as u16)));
    assert!(opcodes.contains(&(WorldOpcode::SmsgSpellHealLog as u16)));
    assert!(opcodes.contains(&(WorldOpcode::SmsgUpdateObject as u16)));
    assert!(!opcodes.contains(&(WorldOpcode::SmsgSpellNonMeleeDamageLog as u16)));
    let heal_log = packets
        .iter()
        .find(|packet| packet.opcode == WorldOpcode::SmsgSpellHealLog as u16)
        .expect("heal spell should emit CMaNGOS heal log");
    let mut cursor = 0;
    assert_eq!(
        read_packed_guid(&heal_log.body, &mut cursor).unwrap(),
        ObjectGuid::new(HighGuid::Player, 0, 7)
    );
    assert_eq!(
        read_packed_guid(&heal_log.body, &mut cursor).unwrap(),
        ObjectGuid::new(HighGuid::Player, 0, 7)
    );
    assert_eq!(read_u32(&heal_log.body, &mut cursor).unwrap(), 2050);
    assert_eq!(read_u32(&heal_log.body, &mut cursor).unwrap(), 20);
    assert_eq!(heal_log.body[cursor], 0);
}

#[tokio::test]
async fn cast_time_spell_sends_start_before_delayed_go_and_effects() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager::with_world_data_files(&WorldDataFiles {
        data_dir: std::path::PathBuf::new(),
        data_dir_for_native: None,
        maps_available: false,
        vmaps_available: false,
        creature_display_scales: HashMap::new(),
        spell_cast_times: HashMap::from([(
            7,
            SpellCastTimeEntry {
                cast_time_millis: 50,
                cast_time_per_level_millis: 0,
                min_cast_time_millis: 50,
            },
        )]),
        spell_durations: HashMap::new(),
        spell_radii: HashMap::new(),
        spell_ranges: HashMap::new(),
        skill_line_abilities_by_spell: HashMap::new(),
        skill_lines: HashMap::new(),
        skill_race_class_infos_by_skill: HashMap::new(),
        faction_templates: FactionTemplateStore::fallback_bridge(),
        item_random_properties: HashMap::new(),
        bank_bag_slot_prices: HashMap::new(),
        area_tables: AreaTableStore::default(),
        wmo_area_tables: WmoAreaTableStore::default(),
        mmap_headers: HashSet::new(),
        mmap_tiles: HashSet::new(),
        vmap_trees: HashSet::new(),
        vmap_tiles: HashSet::new(),
    }));
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let mut fireball = fireball_spell_template();
    fireball.attributes_ex2 = TEST_SPELL_ATTR_EX2_CANT_CRIT;
    fireball.attributes_ex3 = TEST_SPELL_ATTR_EX3_ALWAYS_HIT;
    fireball.speed = 10.0;
    fireball.start_recovery_time = 25;
    object_mgr
        .prime_spell_template_for_test(133, Some(fireball))
        .await;
    object_mgr
        .prime_spell_facing_flag_for_test(133, Some(1))
        .await;
    object_mgr
        .prime_creature_ai_scripts_for_test(6, Vec::new())
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let mut kobold = test_creature_spawn(6);
    kobold.guid = 45;
    kobold.position_x = -8947.0;
    kobold.position_y = -132.0;
    kobold.position_z = 83.5312;
    let target = creature_spawn_guid(&kobold);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(kobold)])
        .await;
    let player_position = WorldPosition::new(0, -8949.95, -132.493, 83.5312, 0.0);
    let mut player = test_player_runtime(7, SessionId::next(), player_position);
    player.power1 = 100;
    player.max_power1 = 100;
    maps.add_player(player).await.unwrap();
    let mut body = Vec::new();
    body.extend_from_slice(&133u32.to_le_bytes());
    body.extend_from_slice(&SPELL_CAST_TARGET_UNIT.to_le_bytes());
    PackedGuid::write(&mut body, target).unwrap();
    let mut active_spells = HashSet::new();
    active_spells.insert(133);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 8,
                level: 1,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_mana: 100,
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    let opcodes = packets
        .iter()
        .map(|packet| packet.opcode)
        .collect::<Vec<_>>();
    assert_eq!(opcodes[0], WorldOpcode::SmsgSpellStart as u16);
    let mut cursor = PackedGuid::packed_size(ObjectGuid::new(HighGuid::Player, 0, 7)) * 2 + 4 + 2;
    assert_eq!(
        read_u32(&packets[0].body, &mut cursor).unwrap(),
        50,
        "SMSG_SPELL_START should expose the DBC cast time to the real client cast bar"
    );
    assert_eq!(
        opcodes,
        vec![WorldOpcode::SmsgSpellStart as u16],
        "cast-time spells should wait for scheduled completion before GO/effects"
    );
    assert!(maps
        .next_pending_player_spell_cast_due_at(0, 7)
        .await
        .is_some());
    assert!(maps
        .player_runtime_snapshot(0, 7)
        .await
        .unwrap()
        .spell_global_cooldowns_until
        .get(&133)
        .is_some_and(|until| *until > Instant::now()));
    assert_eq!(session.character.player_mana, 100);
    assert_eq!(
        maps.db_creature_snapshot(0, target).await.unwrap().health,
        120
    );

    tokio::time::sleep(Duration::from_millis(60)).await;
    complete_pending_player_spell_cast(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    let opcodes = packets
        .iter()
        .map(|packet| packet.opcode)
        .collect::<Vec<_>>();
    assert!(opcodes.contains(&(WorldOpcode::SmsgCastResult as u16)));
    assert!(opcodes.contains(&(WorldOpcode::SmsgSpellGo as u16)));
    assert!(opcodes.contains(&(WorldOpcode::SmsgUpdateObject as u16)));
    assert_eq!(session.character.player_mana, 75);
    assert_eq!(maps.player_runtime_snapshot(0, 7).await.unwrap().power1, 75);
    assert!(packets.iter().any(|packet| {
        if packet.opcode != WorldOpcode::SmsgUpdateObject as u16
            || packet.body[5] != UPDATE_TYPE_VALUES
        {
            return false;
        }
        let (values, _) =
            decode_values_update_block(&packet.body[5..], ObjectGuid::new(HighGuid::Player, 0, 7));
        values[UNIT_FIELD_POWER1] == Some(75)
    }));
    assert!(
        !opcodes.contains(&(WorldOpcode::SmsgSpellNonMeleeDamageLog as u16)),
        "missile damage should wait for projectile impact after SPELL_GO"
    );
    assert!(maps
        .next_pending_player_spell_cast_due_at(0, 7)
        .await
        .is_some());
    assert_eq!(
        maps.db_creature_snapshot(0, target).await.unwrap().health,
        120
    );

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();
    let spam_packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(
        spam_packets
            .iter()
            .any(|packet| packet.opcode == WorldOpcode::SmsgSpellStart as u16),
        "after cast completion the real client can begin a new Fireball while the previous missile is still traveling"
    );
    assert!(!spam_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellNonMeleeDamageLog as u16));
    assert!(
        maps.next_pending_player_spell_cast_due_at(0, 7)
            .await
            .is_some(),
        "spamming Fireball after launch must not overwrite the already-launched impact event"
    );

    tokio::time::sleep(Duration::from_millis(60)).await;
    complete_pending_player_spell_cast(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();
    let second_cast_packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    let second_cast_opcodes = second_cast_packets
        .iter()
        .map(|packet| packet.opcode)
        .collect::<Vec<_>>();
    assert!(second_cast_opcodes.contains(&(WorldOpcode::SmsgCastResult as u16)));
    assert!(second_cast_opcodes.contains(&(WorldOpcode::SmsgSpellGo as u16)));
    assert!(second_cast_packets.iter().any(|packet| {
        if packet.opcode != WorldOpcode::SmsgUpdateObject as u16
            || packet.body[5] != UPDATE_TYPE_VALUES
        {
            return false;
        }
        let (values, _) =
            decode_values_update_block(&packet.body[5..], ObjectGuid::new(HighGuid::Player, 0, 7));
        values[UNIT_FIELD_POWER1] == Some(50)
    }));
    assert!(!second_cast_opcodes.contains(&(WorldOpcode::SmsgSpellNonMeleeDamageLog as u16)));
    assert_eq!(session.character.player_mana, 50);
    assert_eq!(
        maps.db_creature_snapshot(0, target).await.unwrap().health,
        120
    );

    tokio::time::sleep(Duration::from_millis(250)).await;
    complete_pending_player_spell_cast(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    let opcodes = packets
        .iter()
        .map(|packet| packet.opcode)
        .collect::<Vec<_>>();
    assert!(opcodes.contains(&(WorldOpcode::SmsgSpellNonMeleeDamageLog as u16)));
    assert!(
        !opcodes.contains(&(WorldOpcode::SmsgAttackerStateUpdate as u16)),
        "Fireball impact should not also emit a melee hit log"
    );
    assert!(maps
        .next_pending_player_spell_cast_due_at(0, 7)
        .await
        .is_some());
    assert_eq!(session.character.player_mana, 50);
    assert_eq!(
        maps.db_creature_snapshot(0, target).await.unwrap().health,
        106
    );

    tokio::time::sleep(Duration::from_millis(90)).await;
    complete_pending_player_spell_cast(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();
    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellNonMeleeDamageLog as u16));
    assert!(maps
        .next_pending_player_spell_cast_due_at(0, 7)
        .await
        .is_none());
    assert_eq!(
        maps.db_creature_snapshot(0, target).await.unwrap().health,
        92
    );
}

#[tokio::test]
async fn thunder_clap_damages_and_debuffs_nearby_hostile_creatures() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let mut world_data = WorldDataFiles::fallback();
    world_data.spell_radii.insert(
        14,
        SpellRadiusEntry {
            radius: 5.0,
            radius_per_level: 0.0,
            max_radius: 5.0,
        },
    );
    let maps = Arc::new(MapRuntimeManager::with_world_data_files(&world_data));
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    object_mgr
        .prime_spell_template_for_test(6343, Some(thunder_clap_spell_template()))
        .await;
    object_mgr
        .prime_creature_ai_scripts_for_test(6, Vec::new())
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, -8949.95, -132.493, 83.5312, 0.0);
    let mut player = test_player_runtime(7, SessionId::next(), player_position);
    player.power2 = 300;
    maps.add_player(player).await.unwrap();
    let mut first = test_creature_spawn(6);
    first.guid = 45;
    first.position_x = -8947.0;
    first.position_y = -132.0;
    first.position_z = 83.5312;
    first.template.faction = 17;
    let first_target = creature_spawn_guid(&first);
    let mut second = test_creature_spawn(6);
    second.guid = 46;
    second.position_x = -8948.0;
    second.position_y = -135.0;
    second.position_z = 83.5312;
    second.template.faction = 17;
    let second_target = creature_spawn_guid(&second);
    let mut out_of_range = test_creature_spawn(6);
    out_of_range.guid = 47;
    out_of_range.position_x = -8930.0;
    out_of_range.position_y = -132.0;
    out_of_range.position_z = 83.5312;
    out_of_range.template.faction = 17;
    let out_of_range_target = creature_spawn_guid(&out_of_range);
    maps.share_db_creature_snapshots(
        0,
        vec![
            DbCreatureRuntime::new(first),
            DbCreatureRuntime::new(second),
            DbCreatureRuntime::new(out_of_range),
        ],
    )
    .await;

    let mut body = Vec::new();
    body.extend_from_slice(&6343u32.to_le_bytes());
    body.extend_from_slice(&SPELL_CAST_TARGET_UNIT.to_le_bytes());
    PackedGuid::write(&mut body, ObjectGuid::new(HighGuid::Player, 0, 7)).unwrap();
    let mut active_spells = HashSet::new();
    active_spells.insert(6343);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 6,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_rage: 300,
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let first = maps.db_creature_snapshot(0, first_target).await.unwrap();
    let second = maps.db_creature_snapshot(0, second_target).await.unwrap();
    let out_of_range = maps
        .db_creature_snapshot(0, out_of_range_target)
        .await
        .unwrap();
    assert_eq!(first.health, 110);
    assert_eq!(second.health, 110);
    assert_eq!(out_of_range.health, 120);
    assert!(first.active_auras.iter().any(|aura| aura.spell_id == 6343));
    assert!(second.active_auras.iter().any(|aura| aura.spell_id == 6343));
    assert!(out_of_range.active_auras.is_empty());
    assert_eq!(session.character.player_rage, 100);

    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert_eq!(
        packets
            .iter()
            .filter(|packet| packet.opcode == WorldOpcode::SmsgSpellNonMeleeDamageLog as u16)
            .count(),
        2
    );
}

#[tokio::test]
async fn flamestrike_damages_hostile_creatures_at_destination_only() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let mut world_data = WorldDataFiles::fallback();
    world_data.spell_radii.insert(
        8,
        SpellRadiusEntry {
            radius: 8.0,
            radius_per_level: 0.0,
            max_radius: 8.0,
        },
    );
    world_data.spell_durations.insert(
        3,
        SpellDurationEntry {
            duration_millis: 8_000,
            duration_per_level_millis: 0,
            max_duration_millis: 8_000,
        },
    );
    let maps = Arc::new(MapRuntimeManager::with_world_data_files(&world_data));
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    object_mgr
        .prime_spell_template_for_test(2120, {
            let mut flamestrike = flamestrike_spell_template();
            flamestrike.attributes_ex2 = TEST_SPELL_ATTR_EX2_CANT_CRIT;
            flamestrike.attributes_ex3 = TEST_SPELL_ATTR_EX3_ALWAYS_HIT;
            Some(flamestrike)
        })
        .await;
    object_mgr
        .prime_creature_ai_scripts_for_test(6, Vec::new())
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, -8958.0, -132.0, 83.5312, 0.0);
    let mut player = test_player_runtime(7, SessionId::next(), player_position);
    player.power1 = 500;
    maps.add_player(player).await.unwrap();

    let destination = wow_proto::SpellTargetLocation {
        x: -8947.0,
        y: -132.0,
        z: 83.5312,
    };
    let mut hostile = test_creature_spawn(6);
    hostile.guid = 55;
    hostile.position_x = destination.x + 1.0;
    hostile.position_y = destination.y;
    hostile.position_z = destination.z;
    hostile.template.faction = 17;
    let hostile_target = creature_spawn_guid(&hostile);
    let mut friendly = test_creature_spawn(6);
    friendly.guid = 56;
    friendly.position_x = destination.x + 2.0;
    friendly.position_y = destination.y;
    friendly.position_z = destination.z;
    friendly.template.faction = 12;
    let friendly_target = creature_spawn_guid(&friendly);
    let mut out_of_radius = test_creature_spawn(6);
    out_of_radius.guid = 57;
    out_of_radius.position_x = destination.x + 20.0;
    out_of_radius.position_y = destination.y;
    out_of_radius.position_z = destination.z;
    out_of_radius.template.faction = 17;
    let out_of_radius_target = creature_spawn_guid(&out_of_radius);
    maps.share_db_creature_snapshots(
        0,
        vec![
            DbCreatureRuntime::new(hostile),
            DbCreatureRuntime::new(friendly),
            DbCreatureRuntime::new(out_of_radius),
        ],
    )
    .await;

    let mut body = Vec::new();
    body.extend_from_slice(&2120u32.to_le_bytes());
    body.extend_from_slice(&SPELL_CAST_TARGET_DEST_LOCATION.to_le_bytes());
    body.extend_from_slice(&destination.x.to_le_bytes());
    body.extend_from_slice(&destination.y.to_le_bytes());
    body.extend_from_slice(&destination.z.to_le_bytes());
    let mut active_spells = HashSet::new();
    active_spells.insert(2120);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 8,
                level: 20,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_mana: 500,
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let hostile = maps.db_creature_snapshot(0, hostile_target).await.unwrap();
    let direct_health = hostile.health;
    let friendly = maps.db_creature_snapshot(0, friendly_target).await.unwrap();
    let out_of_radius = maps
        .db_creature_snapshot(0, out_of_radius_target)
        .await
        .unwrap();
    assert!(hostile.health < 120);
    assert_eq!(friendly.health, 120);
    assert_eq!(out_of_radius.health, 120);
    assert_eq!(session.character.player_mana, 305);

    let tick_packets = maps
        .advance_all_dynamic_objects(Instant::now() + Duration::from_secs(3), 1_000)
        .await
        .unwrap();
    let hostile = maps.db_creature_snapshot(0, hostile_target).await.unwrap();
    let friendly = maps.db_creature_snapshot(0, friendly_target).await.unwrap();
    let out_of_radius = maps
        .db_creature_snapshot(0, out_of_radius_target)
        .await
        .unwrap();
    assert_eq!(hostile.health, direct_health.saturating_sub(12));
    assert_eq!(friendly.health, 120);
    assert_eq!(out_of_radius.health, 120);
    assert!(tick_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgPeriodicAuraLog as u16));

    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert_eq!(
        packets
            .iter()
            .filter(|packet| packet.opcode == WorldOpcode::SmsgSpellNonMeleeDamageLog as u16)
            .count(),
        1
    );
}

#[tokio::test]
async fn blizzard_creates_channel_dynamic_object_and_ticks_area_damage() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let mut world_data = WorldDataFiles::fallback();
    world_data.spell_radii.insert(
        11,
        SpellRadiusEntry {
            radius: 8.0,
            radius_per_level: 0.0,
            max_radius: 8.0,
        },
    );
    world_data.spell_durations.insert(
        30,
        SpellDurationEntry {
            duration_millis: 4_000,
            duration_per_level_millis: 0,
            max_duration_millis: 4_000,
        },
    );
    let maps = Arc::new(MapRuntimeManager::with_world_data_files(&world_data));
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    object_mgr
        .prime_spell_template_for_test(10, Some(blizzard_spell_template()))
        .await;
    object_mgr
        .prime_creature_ai_scripts_for_test(6, Vec::new())
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, -8958.0, -132.0, 83.5312, 0.0);
    let mut player = test_player_runtime(7, SessionId::next(), player_position);
    player.power1 = 500;
    maps.add_player(player).await.unwrap();

    let destination = wow_proto::SpellTargetLocation {
        x: -8947.0,
        y: -132.0,
        z: 83.5312,
    };
    let mut hostile = test_creature_spawn(6);
    hostile.guid = 58;
    hostile.position_x = destination.x + 1.0;
    hostile.position_y = destination.y;
    hostile.position_z = destination.z;
    hostile.template.faction = 17;
    let hostile_target = creature_spawn_guid(&hostile);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(hostile)])
        .await;

    let mut body = Vec::new();
    body.extend_from_slice(&10u32.to_le_bytes());
    body.extend_from_slice(&SPELL_CAST_TARGET_DEST_LOCATION.to_le_bytes());
    body.extend_from_slice(&destination.x.to_le_bytes());
    body.extend_from_slice(&destination.y.to_le_bytes());
    body.extend_from_slice(&destination.z.to_le_bytes());
    let mut active_spells = HashSet::new();
    active_spells.insert(10);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 8,
                level: 20,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_mana: 500,
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    let opcodes = packets
        .iter()
        .map(|packet| packet.opcode)
        .collect::<Vec<_>>();
    assert!(
        packets
            .iter()
            .any(|packet| packet.opcode == WorldOpcode::MsgChannelStart as u16),
        "{opcodes:?}"
    );
    let channel_start = packets
        .iter()
        .find(|packet| packet.opcode == WorldOpcode::MsgChannelStart as u16)
        .unwrap();
    assert_eq!(
        channel_start.body.len(),
        8,
        "Classic MSG_CHANNEL_START is spell id + duration, no caster guid"
    );
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgUpdateObject as u16));
    assert_eq!(session.character.player_mana, 180);

    let tick_packets = maps
        .advance_all_dynamic_objects(Instant::now() + Duration::from_millis(1_500), 1_000)
        .await
        .unwrap();
    let hostile = maps.db_creature_snapshot(0, hostile_target).await.unwrap();
    assert_eq!(hostile.health, 67);
    assert!(
        maps.active_db_creature_combat_snapshot(
            0,
            hostile_target,
            ObjectGuid::new(HighGuid::Player, 0, 7)
        )
        .await
        .is_some(),
        "Blizzard periodic damage should aggro targets it damages"
    );
    assert!(tick_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgPeriodicAuraLog as u16));

    let expire_packets = maps
        .advance_all_dynamic_objects(Instant::now() + Duration::from_secs(5), 1_005)
        .await
        .unwrap();
    assert!(expire_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgDestroyObject as u16));
    assert!(expire_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::MsgChannelUpdate as u16));
}

#[tokio::test]
async fn cancel_cast_clears_blizzard_dynamic_object_channel() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let mut world_data = WorldDataFiles::fallback();
    world_data.spell_radii.insert(
        11,
        SpellRadiusEntry {
            radius: 8.0,
            radius_per_level: 0.0,
            max_radius: 8.0,
        },
    );
    world_data.spell_durations.insert(
        30,
        SpellDurationEntry {
            duration_millis: 4_000,
            duration_per_level_millis: 0,
            max_duration_millis: 4_000,
        },
    );
    let maps = Arc::new(MapRuntimeManager::with_world_data_files(&world_data));
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    object_mgr
        .prime_spell_template_for_test(10, Some(blizzard_spell_template()))
        .await;
    object_mgr
        .prime_creature_ai_scripts_for_test(6, Vec::new())
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, -8958.0, -132.0, 83.5312, 0.0);
    let mut player = test_player_runtime(7, SessionId::next(), player_position);
    player.power1 = 500;
    maps.add_player(player).await.unwrap();

    let destination = wow_proto::SpellTargetLocation {
        x: -8947.0,
        y: -132.0,
        z: 83.5312,
    };
    let mut hostile = test_creature_spawn(6);
    hostile.guid = 61;
    hostile.position_x = destination.x + 1.0;
    hostile.position_y = destination.y;
    hostile.position_z = destination.z;
    hostile.template.faction = 17;
    let hostile_target = creature_spawn_guid(&hostile);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(hostile)])
        .await;

    let mut body = Vec::new();
    body.extend_from_slice(&10u32.to_le_bytes());
    body.extend_from_slice(&SPELL_CAST_TARGET_DEST_LOCATION.to_le_bytes());
    body.extend_from_slice(&destination.x.to_le_bytes());
    body.extend_from_slice(&destination.y.to_le_bytes());
    body.extend_from_slice(&destination.z.to_le_bytes());
    let mut active_spells = HashSet::new();
    active_spells.insert(10);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 8,
                level: 20,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_mana: 500,
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();
    let _ = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();

    assert!(cancel_pending_player_spell_cast(
        &mut stream,
        &maps,
        &sessions,
        &mut session,
        SPELL_FAILED_INTERRUPTED,
        &mut header_crypto,
    )
    .await
    .unwrap());
    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgDestroyObject as u16));
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::MsgChannelUpdate as u16));

    let tick_packets = maps
        .advance_all_dynamic_objects(Instant::now() + Duration::from_millis(1_500), 1_000)
        .await
        .unwrap();
    let hostile = maps.db_creature_snapshot(0, hostile_target).await.unwrap();
    assert_eq!(hostile.health, 120);
    assert!(tick_packets.is_empty());
}

#[tokio::test]
async fn arcane_missiles_starts_unit_channel_and_ticks_triggered_damage() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let mut world_data = WorldDataFiles::fallback();
    world_data.spell_durations.insert(
        6,
        SpellDurationEntry {
            duration_millis: 3_000,
            duration_per_level_millis: 0,
            max_duration_millis: 3_000,
        },
    );
    world_data.spell_ranges.insert(
        4,
        SpellRangeEntry {
            min_range: 0.0,
            max_range: 30.0,
            flags: 0,
        },
    );
    let maps = Arc::new(MapRuntimeManager::with_world_data_files(&world_data));
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    object_mgr
        .prime_spell_template_for_test(5143, Some(arcane_missiles_spell_template()))
        .await;
    object_mgr
        .prime_spell_template_for_test(7268, Some(arcane_missile_trigger_spell_template()))
        .await;
    object_mgr
        .prime_creature_ai_scripts_for_test(6, Vec::new())
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, -8958.0, -132.0, 83.5312, 0.0);
    let mut player = test_player_runtime(7, SessionId::next(), player_position);
    player.power1 = 500;
    maps.add_player(player).await.unwrap();

    let mut hostile = test_creature_spawn(6);
    hostile.guid = 59;
    hostile.position_x = player_position.x + 10.0;
    hostile.position_y = player_position.y;
    hostile.position_z = player_position.z;
    hostile.template.faction = 17;
    let hostile_target = creature_spawn_guid(&hostile);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(hostile)])
        .await;
    maps.update_player_selection(0, 7, Some(hostile_target))
        .await
        .unwrap();

    let mut body = Vec::new();
    body.extend_from_slice(&5143u32.to_le_bytes());
    body.extend_from_slice(&0u16.to_le_bytes());
    let mut active_spells = HashSet::new();
    active_spells.insert(5143);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 8,
                level: 20,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_mana: 500,
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    let opcodes = packets
        .iter()
        .map(|packet| packet.opcode)
        .collect::<Vec<_>>();
    assert!(
        packets
            .iter()
            .any(|packet| packet.opcode == WorldOpcode::MsgChannelStart as u16),
        "{opcodes:?}"
    );
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgUpdateObject as u16));
    let channel_update_values = packets
        .iter()
        .filter(|packet| packet.opcode == WorldOpcode::SmsgUpdateObject as u16)
        .find_map(|packet| {
            if packet.body.len() <= 5 || packet.body[5] != UPDATE_TYPE_VALUES {
                return None;
            }
            let values = decode_values_update_block(
                &packet.body[5..],
                ObjectGuid::new(HighGuid::Player, 0, 7),
            )
            .0;
            (values[UNIT_CHANNEL_SPELL] == Some(5143)).then_some(values)
        })
        .expect("player channel update packet");
    assert_eq!(channel_update_values[UNIT_CHANNEL_SPELL], Some(5143));
    assert_eq!(
        channel_update_values[UNIT_FIELD_CHANNEL_OBJECT],
        Some(hostile_target.raw() as u32)
    );
    assert_eq!(session.character.player_mana, 415);

    let first_launch_packets = maps
        .advance_all_player_channels(Instant::now() + Duration::from_millis(100), 1_000)
        .await
        .unwrap();
    let hostile = maps.db_creature_snapshot(0, hostile_target).await.unwrap();
    assert_eq!(
        hostile.health, 120,
        "Arcane Missiles should launch first, then apply damage after missile travel"
    );
    assert_eq!(
        first_launch_packets
            .iter()
            .filter(|(_, packet)| packet.opcode == WorldOpcode::SmsgSpellGo as u16)
            .count(),
        1
    );
    assert!(!first_launch_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgAttackStart as u16));
    assert!(!first_launch_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgSpellNonMeleeDamageLog as u16));
    assert!(maps
        .active_db_creature_combat_snapshot(
            0,
            hostile_target,
            ObjectGuid::new(HighGuid::Player, 0, 7)
        )
        .await
        .is_none());

    let first_impact_packets = maps
        .advance_all_player_channels(Instant::now() + Duration::from_millis(600), 1_001)
        .await
        .unwrap();
    let hostile = maps.db_creature_snapshot(0, hostile_target).await.unwrap();
    assert_eq!(hostile.health, 96);
    assert!(first_impact_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgSpellNonMeleeDamageLog as u16));
    assert!(first_impact_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgAttackStart as u16));
    assert!(maps
        .active_db_creature_combat_snapshot(
            0,
            hostile_target,
            ObjectGuid::new(HighGuid::Player, 0, 7)
        )
        .await
        .is_some());

    let second_launch_packets = maps
        .advance_all_player_channels(Instant::now() + Duration::from_millis(1_100), 1_002)
        .await
        .unwrap();
    assert_eq!(
        second_launch_packets
            .iter()
            .filter(|(_, packet)| packet.opcode == WorldOpcode::SmsgSpellGo as u16)
            .count(),
        1
    );
    assert!(!second_launch_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::MsgChannelUpdate as u16));

    let second_impact_packets = maps
        .advance_all_player_channels(Instant::now() + Duration::from_millis(1_600), 1_003)
        .await
        .unwrap();
    assert!(second_impact_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgSpellNonMeleeDamageLog as u16));
    assert!(!second_impact_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::MsgChannelUpdate as u16));
    assert_eq!(
        maps.db_creature_snapshot(0, hostile_target)
            .await
            .unwrap()
            .health,
        72,
        "second missile impact should land before channel clear"
    );

    let third_launch_packets = maps
        .advance_all_player_channels(Instant::now() + Duration::from_millis(2_100), 1_004)
        .await
        .unwrap();
    assert_eq!(
        third_launch_packets
            .iter()
            .filter(|(_, packet)| packet.opcode == WorldOpcode::SmsgSpellGo as u16)
            .count(),
        1
    );
    assert!(!third_launch_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::MsgChannelUpdate as u16));
    assert_eq!(
        maps.db_creature_snapshot(0, hostile_target)
            .await
            .unwrap()
            .health,
        72,
        "third missile launch should happen while channel is still active"
    );

    let final_impact_packets = maps
        .advance_all_player_channels(Instant::now() + Duration::from_millis(2_600), 1_005)
        .await
        .unwrap();
    let hostile = maps.db_creature_snapshot(0, hostile_target).await.unwrap();
    assert_eq!(
        hostile.health, 48,
        "rank 1 Arcane Missiles should still land its third missile after channel expiry"
    );
    assert!(final_impact_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgSpellNonMeleeDamageLog as u16));

    let clear_packets = maps
        .advance_all_player_channels(Instant::now() + Duration::from_millis(3_100), 1_006)
        .await
        .unwrap();
    let channel_update = clear_packets
        .iter()
        .find(|(_, packet)| packet.opcode == WorldOpcode::MsgChannelUpdate as u16)
        .unwrap()
        .1
        .body
        .clone();
    assert_eq!(
        channel_update.len(),
        4,
        "Classic MSG_CHANNEL_UPDATE is remaining duration only"
    );
}

#[test]
fn arcane_missile_impact_death_stops_target_motion() {
    let now = Instant::now();
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8958.0, -132.0, 83.5312, 0.0);
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    map.add_player(test_player_runtime(7, SessionId(7), player_position))
        .unwrap();

    let mut hostile = test_creature_spawn(6);
    hostile.guid = 901_514;
    hostile.position_x = player_position.x + 10.0;
    hostile.position_y = player_position.y;
    hostile.position_z = player_position.z;
    hostile.template.faction = 17;
    hostile.template.min_level_health = 24;
    hostile.template.max_level_health = 24;
    let target = creature_spawn_guid(&hostile);
    let mut creature = DbCreatureRuntime::new(hostile);
    creature.motion = CreatureMotionState::Chase(CreatureChaseMotion {
        target: caster,
        start: creature.current_position,
        destination: player_position,
        path: vec![player_position],
        started_at: now,
        duration: Duration::from_secs(3),
        recheck_at: now + Duration::from_secs(1),
        run: true,
    });
    map.creatures.insert(target.raw(), creature);

    let event = map
        .start_player_periodic_trigger_channel(
            caster,
            7,
            5143,
            target,
            3_000,
            1_000,
            AURA_INTERRUPT_FLAG_DAMAGE_CHANNEL_DURATION,
            20.0,
            PlayerDirectDamageEffect {
                spell_id: 7268,
                damage: 24,
                weapon_damage_percent: 100,
                school: 6,
                dmg_class: SPELL_DAMAGE_CLASS_MAGIC,
                attributes_ex2: TEST_SPELL_ATTR_EX2_CANT_CRIT,
                attributes_ex3: TEST_SPELL_ATTR_EX3_ALWAYS_HIT,
                requires_melee: false,
                uses_weapon_outcome: false,
                suppress_attacker_state: true,
                caster_centered_hostile_area: false,
                destination_hostile_area: false,
                radius_index: 0,
            },
            now,
        )
        .unwrap();
    assert!(event.is_some());

    let launch_packets = map
        .advance_player_channels(now + Duration::from_millis(100), 1_001)
        .unwrap();
    assert!(launch_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgSpellGo as u16));
    assert!(!launch_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgAttackStart as u16));
    assert!(!launch_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgMonsterMove as u16));

    let impact_packets = map
        .advance_player_channels(now + Duration::from_millis(600), 1_002)
        .unwrap();
    let creature = map.creatures.get(&target.raw()).unwrap();
    assert_eq!(creature.health, 0);
    assert!(matches!(creature.motion, CreatureMotionState::Idle));
    assert!(impact_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgAttackStart as u16));
    assert!(impact_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgMonsterMove as u16));
}

#[tokio::test]
async fn arcane_missiles_without_selected_target_fails_before_spending_mana() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let mut world_data = WorldDataFiles::fallback();
    world_data.spell_durations.insert(
        6,
        SpellDurationEntry {
            duration_millis: 3_000,
            duration_per_level_millis: 0,
            max_duration_millis: 3_000,
        },
    );
    world_data.spell_ranges.insert(
        4,
        SpellRangeEntry {
            min_range: 0.0,
            max_range: 30.0,
            flags: 0,
        },
    );
    let maps = Arc::new(MapRuntimeManager::with_world_data_files(&world_data));
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    object_mgr
        .prime_spell_template_for_test(5143, Some(arcane_missiles_spell_template()))
        .await;
    object_mgr
        .prime_spell_template_for_test(7268, Some(arcane_missile_trigger_spell_template()))
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, -8958.0, -132.0, 83.5312, 0.0);
    let mut player = test_player_runtime(7, SessionId::next(), player_position);
    player.power1 = 500;
    maps.add_player(player).await.unwrap();

    let mut body = Vec::new();
    body.extend_from_slice(&5143u32.to_le_bytes());
    body.extend_from_slice(&0u16.to_le_bytes());
    let mut active_spells = HashSet::new();
    active_spells.insert(5143);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 8,
                level: 20,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_mana: 500,
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    let failure = packets
        .iter()
        .find(|packet| packet.opcode == WorldOpcode::SmsgCastResult as u16)
        .expect("cast failure");
    assert_eq!(failure.body[5], SPELL_FAILED_BAD_IMPLICIT_TARGETS);
    assert!(!packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::MsgChannelStart as u16));
    assert_eq!(session.character.player_mana, 500);
    let snapshot = maps.player_runtime_snapshot(0, 7).await.unwrap();
    assert_eq!(snapshot.power1, 500);
}

#[tokio::test]
async fn cancel_cast_clears_active_unit_channel() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let mut world_data = WorldDataFiles::fallback();
    world_data.spell_durations.insert(
        6,
        SpellDurationEntry {
            duration_millis: 3_000,
            duration_per_level_millis: 0,
            max_duration_millis: 3_000,
        },
    );
    world_data.spell_ranges.insert(
        4,
        SpellRangeEntry {
            min_range: 0.0,
            max_range: 30.0,
            flags: 0,
        },
    );
    let maps = Arc::new(MapRuntimeManager::with_world_data_files(&world_data));
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    object_mgr
        .prime_spell_template_for_test(5143, Some(arcane_missiles_spell_template()))
        .await;
    object_mgr
        .prime_spell_template_for_test(7268, Some(arcane_missile_trigger_spell_template()))
        .await;
    object_mgr
        .prime_creature_ai_scripts_for_test(6, Vec::new())
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, -8958.0, -132.0, 83.5312, 0.0);
    let mut player = test_player_runtime(7, SessionId::next(), player_position);
    player.power1 = 500;
    maps.add_player(player).await.unwrap();

    let mut hostile = test_creature_spawn(6);
    hostile.guid = 60;
    hostile.position_x = player_position.x + 10.0;
    hostile.position_y = player_position.y;
    hostile.position_z = player_position.z;
    hostile.template.faction = 17;
    let hostile_target = creature_spawn_guid(&hostile);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(hostile)])
        .await;

    let mut body = Vec::new();
    body.extend_from_slice(&5143u32.to_le_bytes());
    body.extend_from_slice(&SPELL_CAST_TARGET_UNIT.to_le_bytes());
    PackedGuid::write(&mut body, hostile_target).unwrap();
    let mut active_spells = HashSet::new();
    active_spells.insert(5143);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 8,
                level: 20,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_mana: 500,
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();
    let _ = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();

    assert!(cancel_pending_player_spell_cast(
        &mut stream,
        &maps,
        &sessions,
        &mut session,
        SPELL_FAILED_INTERRUPTED,
        &mut header_crypto,
    )
    .await
    .unwrap());
    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::MsgChannelUpdate as u16));

    let tick_packets = maps
        .advance_all_player_channels(Instant::now() + Duration::from_millis(1_100), 1_000)
        .await
        .unwrap();
    let hostile = maps.db_creature_snapshot(0, hostile_target).await.unwrap();
    assert_eq!(hostile.health, 120);
    assert!(!tick_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgSpellNonMeleeDamageLog as u16));
}

#[tokio::test]
async fn frost_nova_roots_neutral_attackable_creatures() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let mut world_data = WorldDataFiles::fallback();
    world_data.spell_radii.insert(
        11,
        SpellRadiusEntry {
            radius: 8.0,
            radius_per_level: 0.0,
            max_radius: 8.0,
        },
    );
    let maps = Arc::new(MapRuntimeManager::with_world_data_files(&world_data));
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    object_mgr
        .prime_spell_template_for_test(122, Some(frost_nova_spell_template()))
        .await;
    object_mgr
        .prime_creature_ai_scripts_for_test(6, Vec::new())
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, -8949.95, -132.493, 83.5312, 0.0);
    maps.add_player(test_player_runtime(7, SessionId::next(), player_position))
        .await
        .unwrap();

    let mut neutral = test_creature_spawn(6);
    neutral.guid = 48;
    neutral.position_x = -8947.0;
    neutral.position_y = -132.0;
    neutral.position_z = 83.5312;
    neutral.template.faction = 25;
    let neutral_target = creature_spawn_guid(&neutral);
    let mut friendly = test_creature_spawn(6);
    friendly.guid = 49;
    friendly.position_x = -8948.0;
    friendly.position_y = -135.0;
    friendly.position_z = 83.5312;
    friendly.template.faction = 12;
    let friendly_target = creature_spawn_guid(&friendly);
    let mut out_of_range = test_creature_spawn(6);
    out_of_range.guid = 50;
    out_of_range.position_x = -8930.0;
    out_of_range.position_y = -132.0;
    out_of_range.position_z = 83.5312;
    out_of_range.template.faction = 25;
    let out_of_range_target = creature_spawn_guid(&out_of_range);
    maps.share_db_creature_snapshots(
        0,
        vec![
            DbCreatureRuntime::new(neutral),
            DbCreatureRuntime::new(friendly),
            DbCreatureRuntime::new(out_of_range),
        ],
    )
    .await;

    let mut body = Vec::new();
    body.extend_from_slice(&122u32.to_le_bytes());
    body.extend_from_slice(&SPELL_CAST_TARGET_UNIT.to_le_bytes());
    PackedGuid::write(&mut body, ObjectGuid::new(HighGuid::Player, 0, 7)).unwrap();
    let mut active_spells = HashSet::new();
    active_spells.insert(122);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 8,
                level: 6,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let neutral = maps.db_creature_snapshot(0, neutral_target).await.unwrap();
    let friendly = maps.db_creature_snapshot(0, friendly_target).await.unwrap();
    let out_of_range = maps
        .db_creature_snapshot(0, out_of_range_target)
        .await
        .unwrap();
    assert!(neutral.active_auras.iter().any(|aura| aura.spell_id == 122));
    assert!(active_aura_has_root(&neutral.active_auras));
    assert!(friendly.active_auras.is_empty());
    assert!(out_of_range.active_auras.is_empty());

    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgUpdateObject as u16));
}

#[tokio::test]
async fn spell_cast_from_sitting_auto_stands_player_and_observers() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let (observer_tx, mut observer_rx) = mpsc::unbounded_channel();
    sessions
        .register(
            SessionId(8),
            SessionHandle {
                account_id: 8,
                character_guid: Some(8),
                character_name: Some("Babbage".to_string()),
                outbound: WorldPacketSender::Unbounded(observer_tx),
                disconnect: None,
            },
        )
        .await;
    let object_mgr = ObjectMgr::default();
    object_mgr
        .prime_spell_template_for_test(6673, Some(battle_shout_spell_template()))
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let position = WorldPosition::new(0, -8949.95, -132.493, 83.5312, 0.0);
    let mut player = test_player_runtime(7, SessionId(7), position);
    player.stand_state = PLAYER_STAND_STATE_SIT;
    player.power2 = 100;
    maps.add_player(player).await.unwrap();
    maps.add_player(test_player_runtime(
        8,
        SessionId(8),
        WorldPosition::new(0, -8950.5, -132.493, 83.5312, 0.0),
    ))
    .await
    .unwrap();

    let mut active_spells = HashSet::new();
    active_spells.insert(6673);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_rage: 100,
            player_stand_state: PLAYER_STAND_STATE_SIT,
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    let mut cast_body = Vec::new();
    cast_body.extend_from_slice(&6673u32.to_le_bytes());
    cast_body.extend_from_slice(&0u32.to_le_bytes());

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&cast_body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    let observer_packets = std::iter::from_fn(|| observer_rx.try_recv().ok()).collect::<Vec<_>>();
    assert_eq!(
        session.character.player_stand_state,
        PLAYER_STAND_STATE_STAND
    );
    assert!(packets.iter().any(
        |packet| packet.opcode == WorldOpcode::SmsgStandStateUpdate as u16 && packet.body == [0]
    ));
    assert!(packets.iter().any(|packet| {
        if packet.opcode != WorldOpcode::SmsgUpdateObject as u16 {
            return false;
        }
        let (values, _) =
            decode_values_update_block(&packet.body[5..], ObjectGuid::new(HighGuid::Player, 0, 7));
        values[UNIT_FIELD_BYTES_1]
            == Some(unit_bytes_1_for_class(1) | u32::from(PLAYER_STAND_STATE_STAND))
    }));
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellStart as u16));
    assert!(observer_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgUpdateObject as u16));
    assert!(observer_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellStart as u16));
}

#[tokio::test]
async fn stand_state_change_to_stand_cancels_consumable_regen_aura() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let position = WorldPosition::new(0, -8949.95, -132.493, 83.5312, 0.0);
    let now = Instant::now();
    let regen_aura = ActiveAura {
        spell_id: 1127,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 1,
        interrupt_flags: AURA_INTERRUPT_FLAG_STANDING_CANCELS,
        positive: true,
        visible: true,
        duration_millis: Some(30_000),
        expires_at: Some(now + Duration::from_secs(30)),
        periodic_damage: None,
        periodic_regen: Some(PeriodicRegenAura {
            health_amount: 5,
            mana_amount: 0,
            tick_millis: 2_000,
            next_tick_at: now + Duration::from_secs(2),
            interrupts_on_move_and_stand: true,
            suppresses_recent_damage: true,
            makes_player_sit: true,
        }),
        stat_modifiers: Vec::new(),
        proc_triggers: Vec::new(),
    };
    let mut player = test_player_runtime(7, SessionId(7), position);
    player.stand_state = PLAYER_STAND_STATE_SIT;
    player.active_auras.push(regen_aura.clone());
    maps.add_player(player).await.unwrap();
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_stand_state: PLAYER_STAND_STATE_SIT,
            ..CharacterSessionState::default()
        },
        auras: AuraSessionState {
            active_auras: vec![regen_aura],
        },
        ..WorldSessionState::default()
    };

    handle_stand_state_change(
        &mut stream,
        shared_world,
        wow_proto::StandStateChangeRequest { stand_state: 0 },
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    let snapshot = maps.player_runtime_snapshot(0, 7).await.unwrap();
    assert_eq!(
        session.character.player_stand_state,
        PLAYER_STAND_STATE_STAND
    );
    assert!(session.auras.active_auras.is_empty());
    assert_eq!(snapshot.active_auras, Vec::new());
    assert!(
        packets
            .iter()
            .filter(|packet| packet.opcode == WorldOpcode::SmsgUpdateObject as u16)
            .count()
            >= 2
    );
}

#[tokio::test]
async fn client_stand_state_change_updates_map_without_self_echo() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let position = WorldPosition::new(0, -8949.95, -132.493, 83.5312, 0.0);
    maps.add_player(test_player_runtime(7, SessionId(7), position))
        .await
        .unwrap();
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_stand_state: PLAYER_STAND_STATE_STAND,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_stand_state_change(
        &mut stream,
        shared_world,
        wow_proto::StandStateChangeRequest {
            stand_state: u32::from(PLAYER_STAND_STATE_SIT),
        },
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let map = maps.maps.lock().await.get(&(0, 0)).cloned().unwrap();
    let stand_state = map.lock().await.players.get(&7).unwrap().stand_state;
    assert_eq!(session.character.player_stand_state, PLAYER_STAND_STATE_SIT);
    assert_eq!(stand_state, PLAYER_STAND_STATE_SIT);
    assert!(
        rx.try_recv().is_err(),
        "client-originated stand changes should not echo an immediate self update"
    );
}

#[tokio::test]
async fn moving_during_cast_time_interrupts_spell_before_damage_or_power_spend() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager::with_world_data_files(&WorldDataFiles {
        data_dir: std::path::PathBuf::new(),
        data_dir_for_native: None,
        maps_available: false,
        vmaps_available: false,
        creature_display_scales: HashMap::new(),
        spell_cast_times: HashMap::from([(
            7,
            SpellCastTimeEntry {
                cast_time_millis: 1_500,
                cast_time_per_level_millis: 0,
                min_cast_time_millis: 1_500,
            },
        )]),
        spell_durations: HashMap::new(),
        spell_radii: HashMap::new(),
        spell_ranges: HashMap::new(),
        skill_line_abilities_by_spell: HashMap::new(),
        skill_lines: HashMap::new(),
        skill_race_class_infos_by_skill: HashMap::new(),
        faction_templates: FactionTemplateStore::fallback_bridge(),
        item_random_properties: HashMap::new(),
        bank_bag_slot_prices: HashMap::new(),
        area_tables: AreaTableStore::default(),
        wmo_area_tables: WmoAreaTableStore::default(),
        mmap_headers: HashSet::new(),
        mmap_tiles: HashSet::new(),
        vmap_trees: HashSet::new(),
        vmap_tiles: HashSet::new(),
    }));
    let sessions = Arc::new(SessionRegistry::default());
    let (observer_tx, mut observer_rx) = mpsc::unbounded_channel();
    sessions
        .register(
            SessionId(8),
            SessionHandle {
                account_id: 8,
                character_guid: Some(8),
                character_name: Some("Babbage".to_string()),
                outbound: WorldPacketSender::Unbounded(observer_tx),
                disconnect: None,
            },
        )
        .await;
    let object_mgr = ObjectMgr::default();
    object_mgr
        .prime_spell_template_for_test(133, Some(fireball_spell_template()))
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let position = WorldPosition::new(0, -8949.95, -132.493, 83.5312, 0.0);
    let mut player = test_player_runtime(7, SessionId(7), position);
    player.power1 = 100;
    player.max_power1 = 100;
    maps.add_player(player).await.unwrap();
    maps.add_player(test_player_runtime(
        8,
        SessionId(8),
        WorldPosition::new(0, -8950.5, -132.493, 83.5312, 0.0),
    ))
    .await
    .unwrap();
    let mut kobold = test_creature_spawn(6);
    kobold.guid = 45;
    kobold.position_x = -8947.0;
    kobold.position_y = -132.0;
    kobold.position_z = 83.5312;
    let target = creature_spawn_guid(&kobold);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(kobold)])
        .await;
    let mut cast_body = Vec::new();
    cast_body.extend_from_slice(&133u32.to_le_bytes());
    cast_body.extend_from_slice(&SPELL_CAST_TARGET_UNIT.to_le_bytes());
    PackedGuid::write(&mut cast_body, target).unwrap();
    let mut active_spells = HashSet::new();
    active_spells.insert(133);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 8,
                level: 1,
                xp: 0,
                position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_mana: 100,
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&cast_body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();
    assert!(maps
        .next_pending_player_spell_cast_due_at(0, 7)
        .await
        .is_some());
    let _ = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    let _ = std::iter::from_fn(|| observer_rx.try_recv().ok()).collect::<Vec<_>>();

    assert!(movement_opcode_interrupts_spell_cast(
        WorldOpcode::MsgMoveStartForward as u32
    ));
    cancel_pending_player_spell_cast(
        &mut stream,
        maps.as_ref(),
        sessions.as_ref(),
        &mut session,
        SPELL_FAILED_INTERRUPTED,
        &mut header_crypto,
    )
    .await
    .unwrap();

    assert!(maps
        .next_pending_player_spell_cast_due_at(0, 7)
        .await
        .is_none());
    assert_eq!(session.character.player_mana, 100);
    assert_eq!(
        maps.db_creature_snapshot(0, target).await.unwrap().health,
        120
    );
    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    let observer_packets = std::iter::from_fn(|| observer_rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgCastResult as u16
            && packet.body[5] == SPELL_FAILED_INTERRUPTED));
    assert!(observer_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellFailure as u16));
    assert!(observer_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellFailedOther as u16));
    assert!(!packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellGo as u16));
    assert!(!packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellNonMeleeDamageLog as u16));
    assert!(
        !maps.player_runtime_snapshot(0, 7)
            .await
            .unwrap()
            .spell_global_cooldowns_until
            .contains_key(&133),
        "interrupted active casts should clear their start recovery so the next cast is not server-locked"
    );

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&cast_body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();
    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellStart as u16));
    assert!(!packets.iter().any(|packet| {
        packet.opcode == WorldOpcode::SmsgCastResult as u16
            && packet.body[5] == SPELL_FAILED_NOT_READY
    }));
}

#[tokio::test]
async fn movement_does_not_interrupt_cast_without_movement_interrupt_flag() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let map_id = 0;
    let character_guid = 7;
    let now = Instant::now();
    let position = WorldPosition::new(map_id, -8949.95, -132.493, 83.5312, 0.0);
    maps.add_player(test_player_runtime(character_guid, SessionId(7), position))
        .await
        .unwrap();
    let fireball = fireball_spell_template();
    maps.set_active_player_spell_cast(
        map_id,
        character_guid,
        ActivePlayerSpellCast {
            spell_id: 133,
            source: ActivePlayerSpellCastSource::Player,
            profile: player_spell_cast_profile(&fireball).unwrap(),
            targets: PendingSpellCastTargets {
                target_mask: 0,
                unit_target: None,
                gameobject_target: None,
                source_location: None,
                destination: None,
            },
            due_at: now + Duration::from_millis(1_500),
            cast_time_millis: 1_500,
            interrupt_flags: 0,
            damage_pushback_count: 0,
        },
    )
    .await;
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: character_guid,
                name: "Ada".to_string(),
                race: 1,
                class: 8,
                level: 1,
                xp: 0,
                position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    assert!(!cancel_movement_interrupted_player_spell_cast(
        &mut stream,
        maps.as_ref(),
        sessions.as_ref(),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap());

    assert_eq!(
        maps.next_pending_player_spell_cast_due_at(map_id, character_guid)
            .await,
        Some(now + Duration::from_millis(1_500))
    );
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn movement_interrupts_cast_with_movement_interrupt_flag() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let (observer_tx, mut observer_rx) = mpsc::unbounded_channel();
    sessions
        .register(
            SessionId(8),
            SessionHandle {
                account_id: 8,
                character_guid: Some(8),
                character_name: Some("Babbage".to_string()),
                outbound: WorldPacketSender::Unbounded(observer_tx),
                disconnect: None,
            },
        )
        .await;
    let map_id = 0;
    let character_guid = 7;
    let now = Instant::now();
    let position = WorldPosition::new(map_id, -8949.95, -132.493, 83.5312, 0.0);
    maps.add_player(test_player_runtime(character_guid, SessionId(7), position))
        .await
        .unwrap();
    maps.add_player(test_player_runtime(
        8,
        SessionId(8),
        WorldPosition::new(map_id, -8950.5, -132.493, 83.5312, 0.0),
    ))
    .await
    .unwrap();
    let mut fireball = fireball_spell_template();
    fireball.interrupt_flags = SPELL_INTERRUPT_FLAG_MOVEMENT;
    maps.set_active_player_spell_cast(
        map_id,
        character_guid,
        ActivePlayerSpellCast {
            spell_id: 133,
            source: ActivePlayerSpellCastSource::Player,
            profile: player_spell_cast_profile(&fireball).unwrap(),
            targets: PendingSpellCastTargets {
                target_mask: 0,
                unit_target: None,
                gameobject_target: None,
                source_location: None,
                destination: None,
            },
            due_at: now + Duration::from_millis(1_500),
            cast_time_millis: 1_500,
            interrupt_flags: fireball.interrupt_flags,
            damage_pushback_count: 0,
        },
    )
    .await;
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: character_guid,
                name: "Ada".to_string(),
                race: 1,
                class: 8,
                level: 1,
                xp: 0,
                position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    assert!(cancel_movement_interrupted_player_spell_cast(
        &mut stream,
        maps.as_ref(),
        sessions.as_ref(),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap());

    assert!(maps
        .next_pending_player_spell_cast_due_at(map_id, character_guid)
        .await
        .is_none());
    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    let observer_packets = std::iter::from_fn(|| observer_rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgCastResult as u16
            && packet.body[5] == SPELL_FAILED_INTERRUPTED));
    assert!(observer_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellFailure as u16));
    assert!(observer_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellFailedOther as u16));
}

#[tokio::test]
async fn movement_does_not_interrupt_channel_without_moving_interrupt_flag() {
    let maps = MapRuntimeManager::default();
    let map_id = 0;
    let character_guid = 7;
    let caster = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let target = creature_spawn_guid(&{
        let mut spawn = test_creature_spawn(6);
        spawn.guid = 45;
        spawn
    });
    let now = Instant::now();
    {
        let map = maps.get_or_create_map(map_id, 0).await;
        let mut map = map.lock().await;
        insert_map_runtime_player_for_test(
            &mut map,
            character_guid,
            WorldPosition::new(map_id, -8949.95, -132.493, 83.5312, 0.0),
        );
        let mut spawn = test_creature_spawn(6);
        spawn.guid = 45;
        map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    }
    maps.start_player_periodic_trigger_channel(
        map_id,
        caster,
        character_guid,
        5143,
        target,
        5_000,
        1_000,
        player_weapon_damage_effect(
            &player_spell_cast_profile(&fireball_spell_template()).unwrap(),
        ),
        0,
        0.0,
        now,
    )
    .await
    .unwrap();

    assert!(maps
        .cancel_movement_interrupted_player_channel(map_id, character_guid)
        .await
        .unwrap()
        .is_none());
    let map = maps.maps.lock().await.get(&(map_id, 0)).cloned().unwrap();
    assert!(map
        .lock()
        .await
        .active_player_channels
        .contains_key(&character_guid));
}

#[tokio::test]
async fn movement_interrupts_channel_with_moving_interrupt_flag() {
    let maps = MapRuntimeManager::default();
    let map_id = 0;
    let character_guid = 7;
    let caster = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let target = creature_spawn_guid(&{
        let mut spawn = test_creature_spawn(6);
        spawn.guid = 45;
        spawn
    });
    let now = Instant::now();
    {
        let map = maps.get_or_create_map(map_id, 0).await;
        let mut map = map.lock().await;
        insert_map_runtime_player_for_test(
            &mut map,
            character_guid,
            WorldPosition::new(map_id, -8949.95, -132.493, 83.5312, 0.0),
        );
        let mut spawn = test_creature_spawn(6);
        spawn.guid = 45;
        map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    }
    maps.start_player_periodic_trigger_channel(
        map_id,
        caster,
        character_guid,
        5143,
        target,
        5_000,
        1_000,
        player_weapon_damage_effect(
            &player_spell_cast_profile(&fireball_spell_template()).unwrap(),
        ),
        AURA_INTERRUPT_FLAG_MOVING,
        0.0,
        now,
    )
    .await
    .unwrap();

    assert!(maps
        .cancel_movement_interrupted_player_channel(map_id, character_guid)
        .await
        .unwrap()
        .is_some());
    let map = maps.maps.lock().await.get(&(map_id, 0)).cloned().unwrap();
    assert!(!map
        .lock()
        .await
        .active_player_channels
        .contains_key(&character_guid));
}

#[test]
fn corpse_falling_movement_allows_landing_but_blocks_walking() {
    let standing = MovementInfo {
        flags: 0,
        client_time: 1,
        position: WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
        fall_time: 0,
        jump: JumpInfo::default(),
    };
    let falling = MovementInfo {
        flags: MOVEFLAG_JUMPING,
        fall_time: 250,
        jump: JumpInfo {
            z_speed: 7.0,
            cos_angle: 0.25,
            sin_angle: 0.75,
            xy_speed: 4.5,
        },
        ..standing.clone()
    };
    let landing = MovementInfo {
        fall_time: 250,
        ..standing.clone()
    };

    assert!(corpse_falling_movement_allowed(
        WorldOpcode::MsgMoveJump as u32,
        &falling
    ));
    assert!(corpse_falling_movement_allowed(
        WorldOpcode::MsgMoveHeartbeat as u32,
        &landing
    ));
    assert!(corpse_falling_movement_allowed(
        WorldOpcode::MsgMoveFallLand as u32,
        &standing
    ));
    assert!(corpse_falling_movement_allowed(
        WorldOpcode::MsgMoveStartSwim as u32,
        &landing
    ));
    assert!(!corpse_falling_movement_allowed(
        WorldOpcode::MsgMoveStartForward as u32,
        &standing
    ));
    assert!(!corpse_falling_movement_allowed(
        WorldOpcode::MsgMoveHeartbeat as u32,
        &standing
    ));
}

#[tokio::test]
async fn cast_time_spell_rechecks_facing_before_completion_go() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager::with_world_data_files(&WorldDataFiles {
        data_dir: std::path::PathBuf::new(),
        data_dir_for_native: None,
        maps_available: false,
        vmaps_available: false,
        creature_display_scales: HashMap::new(),
        spell_cast_times: HashMap::from([(
            7,
            SpellCastTimeEntry {
                cast_time_millis: 50,
                cast_time_per_level_millis: 0,
                min_cast_time_millis: 50,
            },
        )]),
        spell_durations: HashMap::new(),
        spell_radii: HashMap::new(),
        spell_ranges: HashMap::from([(
            900,
            SpellRangeEntry {
                min_range: 0.0,
                max_range: 30.0,
                flags: 0,
            },
        )]),
        skill_line_abilities_by_spell: HashMap::new(),
        skill_lines: HashMap::new(),
        skill_race_class_infos_by_skill: HashMap::new(),
        faction_templates: FactionTemplateStore::fallback_bridge(),
        item_random_properties: HashMap::new(),
        bank_bag_slot_prices: HashMap::new(),
        area_tables: AreaTableStore::default(),
        wmo_area_tables: WmoAreaTableStore::default(),
        mmap_headers: HashSet::new(),
        mmap_tiles: HashSet::new(),
        vmap_trees: HashSet::new(),
        vmap_tiles: HashSet::new(),
    }));
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let mut template = fireball_spell_template();
    template.range_index = 900;
    object_mgr
        .prime_spell_template_for_test(133, Some(template))
        .await;
    object_mgr
        .prime_spell_facing_flag_for_test(133, Some(1))
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    let mut player = test_player_runtime(7, SessionId(7), position);
    player.power1 = 100;
    player.max_power1 = 100;
    maps.add_player(player).await.unwrap();
    let mut kobold = test_creature_spawn(6);
    kobold.guid = 45;
    kobold.position_x = 10.0;
    kobold.position_y = 0.0;
    kobold.position_z = 0.0;
    let target = creature_spawn_guid(&kobold);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(kobold)])
        .await;
    let mut cast_body = Vec::new();
    cast_body.extend_from_slice(&133u32.to_le_bytes());
    cast_body.extend_from_slice(&SPELL_CAST_TARGET_UNIT.to_le_bytes());
    PackedGuid::write(&mut cast_body, target).unwrap();
    let mut active_spells = HashSet::new();
    active_spells.insert(133);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 8,
                level: 1,
                xp: 0,
                position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_mana: 100,
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&cast_body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();
    let _ = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    session
        .character
        .active_character
        .as_mut()
        .unwrap()
        .position
        .orientation = std::f32::consts::PI;
    maps.sync_player_gameplay_state(0, 7, &session).await;

    tokio::time::sleep(Duration::from_millis(60)).await;
    complete_pending_player_spell_cast(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(packets.iter().any(|packet| {
        packet.opcode == WorldOpcode::SmsgCastResult as u16
            && packet.body[5] == SPELL_FAILED_UNIT_NOT_INFRONT
    }));
    assert!(!packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellGo as u16));
    assert!(!packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellNonMeleeDamageLog as u16));
    assert_eq!(session.character.player_mana, 100);
    assert_eq!(
        maps.db_creature_snapshot(0, target).await.unwrap().health,
        120
    );
}

#[tokio::test]
async fn cast_time_spell_rechecks_los_before_completion_go() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager::with_world_data_files(&WorldDataFiles {
        data_dir: std::path::PathBuf::new(),
        data_dir_for_native: None,
        maps_available: false,
        vmaps_available: false,
        creature_display_scales: HashMap::new(),
        spell_cast_times: HashMap::from([(
            7,
            SpellCastTimeEntry {
                cast_time_millis: 50,
                cast_time_per_level_millis: 0,
                min_cast_time_millis: 50,
            },
        )]),
        spell_durations: HashMap::new(),
        spell_radii: HashMap::new(),
        spell_ranges: HashMap::from([(
            900,
            SpellRangeEntry {
                min_range: 0.0,
                max_range: 30.0,
                flags: 0,
            },
        )]),
        skill_line_abilities_by_spell: HashMap::new(),
        skill_lines: HashMap::new(),
        skill_race_class_infos_by_skill: HashMap::new(),
        faction_templates: FactionTemplateStore::fallback_bridge(),
        item_random_properties: HashMap::new(),
        bank_bag_slot_prices: HashMap::new(),
        area_tables: AreaTableStore::default(),
        wmo_area_tables: WmoAreaTableStore::default(),
        mmap_headers: HashSet::new(),
        mmap_tiles: HashSet::new(),
        vmap_trees: HashSet::new(),
        vmap_tiles: HashSet::new(),
    }));
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let mut template = fireball_spell_template();
    template.range_index = 900;
    object_mgr
        .prime_spell_template_for_test(133, Some(template))
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    let mut player = test_player_runtime(7, SessionId(7), position);
    player.power1 = 100;
    player.max_power1 = 100;
    maps.add_player(player).await.unwrap();
    let mut kobold = test_creature_spawn(6);
    kobold.guid = 45;
    kobold.position_x = 10.0;
    kobold.position_y = 0.0;
    kobold.position_z = 0.0;
    let target = creature_spawn_guid(&kobold);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(kobold)])
        .await;
    let mut cast_body = Vec::new();
    cast_body.extend_from_slice(&133u32.to_le_bytes());
    cast_body.extend_from_slice(&SPELL_CAST_TARGET_UNIT.to_le_bytes());
    PackedGuid::write(&mut cast_body, target).unwrap();
    let mut active_spells = HashSet::new();
    active_spells.insert(133);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 8,
                level: 1,
                xp: 0,
                position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_mana: 100,
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&cast_body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();
    let _ = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    session.movement.db_creature_navigation.line_of_sight_clear = false;

    tokio::time::sleep(Duration::from_millis(60)).await;
    complete_pending_player_spell_cast(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(packets.iter().any(|packet| {
        packet.opcode == WorldOpcode::SmsgCastResult as u16
            && packet.body[5] == SPELL_FAILED_LINE_OF_SIGHT
    }));
    assert!(!packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellGo as u16));
    assert!(!packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellNonMeleeDamageLog as u16));
    assert_eq!(session.character.player_mana, 100);
    assert_eq!(
        maps.db_creature_snapshot(0, target).await.unwrap().health,
        120
    );
}

#[tokio::test]
async fn fireball_with_periodic_aura_applies_direct_damage_and_dot() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager {
        spell_cast_times: HashMap::new(),
        spell_durations: HashMap::from([(
            9,
            SpellDurationEntry {
                duration_millis: 4_000,
                duration_per_level_millis: 0,
                max_duration_millis: 4_000,
            },
        )]),
        ..MapRuntimeManager::default()
    });
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let mut fireball = fireball_with_dot_spell_template();
    fireball.attributes_ex2 = TEST_SPELL_ATTR_EX2_CANT_CRIT;
    fireball.attributes_ex3 = TEST_SPELL_ATTR_EX3_ALWAYS_HIT;
    object_mgr
        .prime_spell_template_for_test(133, Some(fireball))
        .await;
    object_mgr
        .prime_creature_ai_scripts_for_test(6, Vec::new())
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    let mut player = test_player_runtime(7, SessionId(7), player_position);
    player.power1 = 100;
    player.max_power1 = 100;
    maps.add_player(player).await.unwrap();
    let mut kobold = test_creature_spawn(6);
    kobold.guid = 45;
    kobold.position_x = 4.0;
    kobold.position_y = 0.0;
    kobold.position_z = 0.0;
    kobold.template.npc_flags = 0;
    let target = creature_spawn_guid(&kobold);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(kobold)])
        .await;
    let mut body = Vec::new();
    body.extend_from_slice(&133u32.to_le_bytes());
    body.extend_from_slice(&SPELL_CAST_TARGET_UNIT.to_le_bytes());
    PackedGuid::write(&mut body, target).unwrap();
    let mut active_spells = HashSet::new();
    active_spells.insert(133);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 8,
                level: 4,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_mana: 100,
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    assert_eq!(session.character.player_mana, 75);
    let creature = maps.db_creature_snapshot(0, target).await.unwrap();
    assert_eq!(
        creature.health, 106,
        "Fireball should apply its direct school-damage effect before installing its DoT aura"
    );
    assert_eq!(creature.active_auras.len(), 1);
    let aura = &creature.active_auras[0];
    assert_eq!(aura.spell_id, 133);
    assert_eq!(aura.duration_millis, Some(4_000));
    assert_eq!(aura.periodic_damage.unwrap().amount, 3);
    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellNonMeleeDamageLog as u16));
    assert!(packets.iter().any(
        |packet| packet.opcode == WorldOpcode::SmsgUpdateObject as u16
            && packet
                .body
                .windows(4)
                .any(|window| window == 133u32.to_le_bytes().as_slice())
    ));
}

#[tokio::test]
async fn heroic_strike_cast_sends_spell_start_until_next_swing() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    object_mgr
        .prime_spell_template_for_test(
            WARRIOR_HEROIC_STRIKE_RANK_1,
            Some(heroic_strike_spell_template()),
        )
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, -8949.95, -132.493, 83.5312, 0.0);
    let mut player = test_player_runtime(7, SessionId::next(), player_position);
    player.power2 = POWER_RAGE_DEFAULT;
    maps.add_player(player).await.unwrap();
    let target = ObjectGuid::new(HighGuid::Unit, 6, 45);
    let mut body = Vec::new();
    body.extend_from_slice(&WARRIOR_HEROIC_STRIKE_RANK_1.to_le_bytes());
    body.extend_from_slice(&SPELL_CAST_TARGET_UNIT.to_le_bytes());
    PackedGuid::write(&mut body, target).unwrap();
    let mut active_spells = HashSet::new();
    active_spells.insert(WARRIOR_HEROIC_STRIKE_RANK_1);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_rage: POWER_RAGE_DEFAULT,
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    assert_eq!(
        maps.player_runtime_snapshot(0, 7)
            .await
            .unwrap()
            .queued_next_melee_spell,
        Some(QueuedNextMeleeSpell {
            spell_id: WARRIOR_HEROIC_STRIKE_RANK_1,
            target,
            bonus_damage: HEROIC_STRIKE_FIXTURE_DAMAGE,
            rage_cost: HEROIC_STRIKE_RAGE_COST,
            mana_cost: 0,
        })
    );
    assert_eq!(
        session.character.player_rage, POWER_RAGE_DEFAULT,
        "next-melee rage is spent when the queued swing fires, not when it is queued"
    );
    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(!packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgCastResult as u16));
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellStart as u16));
    assert!(!packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellGo as u16));
    assert!(!packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgUpdateObject as u16));
}

#[tokio::test]
async fn heroic_strike_can_queue_before_target_is_in_melee_range() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    object_mgr
        .prime_spell_template_for_test(
            WARRIOR_HEROIC_STRIKE_RANK_1,
            Some(heroic_strike_spell_template()),
        )
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };

    let player_position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    let mut player = test_player_runtime(7, SessionId(7), player_position);
    player.power2 = POWER_RAGE_DEFAULT;
    maps.add_player(player).await.unwrap();
    let mut kobold = test_creature_spawn(6);
    kobold.guid = 45;
    kobold.position_x = 8.0;
    kobold.position_y = 0.0;
    kobold.position_z = 0.0;
    kobold.template.npc_flags = 0;
    let target = creature_spawn_guid(&kobold);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(kobold)])
        .await;

    let mut body = Vec::new();
    body.extend_from_slice(&WARRIOR_HEROIC_STRIKE_RANK_1.to_le_bytes());
    body.extend_from_slice(&SPELL_CAST_TARGET_UNIT.to_le_bytes());
    PackedGuid::write(&mut body, target).unwrap();
    let mut active_spells = HashSet::new();
    active_spells.insert(WARRIOR_HEROIC_STRIKE_RANK_1);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_rage: POWER_RAGE_DEFAULT,
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    assert_eq!(
        maps.player_runtime_snapshot(0, 7)
            .await
            .unwrap()
            .queued_next_melee_spell,
        Some(QueuedNextMeleeSpell {
            spell_id: WARRIOR_HEROIC_STRIKE_RANK_1,
            target,
            bonus_damage: HEROIC_STRIKE_FIXTURE_DAMAGE,
            rage_cost: HEROIC_STRIKE_RAGE_COST,
            mana_cost: 0,
        })
    );
    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(!packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgCastResult as u16));
    assert!(!packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellFailure as u16));
    assert!(!packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellFailedOther as u16));

    let start = packets
        .iter()
        .find(|packet| packet.opcode == WorldOpcode::SmsgSpellStart as u16)
        .expect("queued Heroic Strike should send SpellStart for client queued-state UI");
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    let mut cursor = PackedGuid::packed_size(caster) * 2;
    assert_eq!(
        read_u32(&start.body, &mut cursor).unwrap(),
        WARRIOR_HEROIC_STRIKE_RANK_1
    );
    assert_eq!(
        u16::from_le_bytes(start.body[cursor..cursor + 2].try_into().unwrap()),
        CAST_FLAG_SPELL_START
    );
    cursor += 2;
    assert_eq!(read_u32(&start.body, &mut cursor).unwrap(), 0);
    assert_eq!(
        u16::from_le_bytes(start.body[cursor..cursor + 2].try_into().unwrap()),
        SPELL_CAST_TARGET_UNIT
    );
    cursor += 2;
    assert_eq!(read_packed_guid(&start.body, &mut cursor).unwrap(), target);
    assert_eq!(cursor, start.body.len());
}

#[tokio::test]
async fn db_creature_death_clears_queued_next_melee_spell_for_target() {
    let maps = Arc::new(MapRuntimeManager::default());
    let map_id = 0;
    let now = Instant::now();
    let mut kobold = test_creature_spawn(6);
    kobold.guid = 45;
    kobold.template.min_level_health = 20;
    kobold.template.max_level_health = 20;
    let target = creature_spawn_guid(&kobold);
    maps.share_db_creature_snapshots(map_id, vec![DbCreatureRuntime::new(kobold)])
        .await;

    let mut queued_without_auto_attack = test_player_runtime(
        7,
        SessionId(7),
        WorldPosition::new(map_id, 0.0, 0.0, 0.0, 0.0),
    );
    queued_without_auto_attack.power2 = POWER_RAGE_DEFAULT;
    maps.add_player(queued_without_auto_attack).await.unwrap();
    maps.queue_player_next_melee_spell(
        map_id,
        7,
        QueuedNextMeleeSpell {
            spell_id: WARRIOR_HEROIC_STRIKE_RANK_1,
            target,
            bonus_damage: HEROIC_STRIKE_FIXTURE_DAMAGE,
            rage_cost: HEROIC_STRIKE_RAGE_COST,
            mana_cost: 0,
        },
    )
    .await;

    let mut queued_with_auto_attack = test_player_runtime(
        8,
        SessionId(8),
        WorldPosition::new(map_id, 1.0, 0.0, 0.0, 0.0),
    );
    queued_with_auto_attack.power2 = POWER_RAGE_DEFAULT;
    maps.add_player(queued_with_auto_attack).await.unwrap();
    maps.queue_player_next_melee_spell(
        map_id,
        8,
        QueuedNextMeleeSpell {
            spell_id: WARRIOR_HEROIC_STRIKE_RANK_1,
            target,
            bonus_damage: HEROIC_STRIKE_FIXTURE_DAMAGE,
            rage_cost: HEROIC_STRIKE_RAGE_COST,
            mana_cost: 0,
        },
    )
    .await;
    maps.set_player_auto_attack(map_id, 8, Some(target), Some(now))
        .await;

    let event = maps
        .apply_db_creature_damage(
            map_id,
            DbCreatureDamageRequest {
                creature_guid: target,
                killer: ObjectGuid::new(HighGuid::Player, 0, 7),
                damage: 20,
                melee_outcome: None,
                spell_damage_outcome: None,
                spell_id: None,
                spell_school: 0,
                suppress_attacker_state: false,
                now,
                now_epoch_secs: 0,
                exclude_character_guid: Some(7),
                corpse_loot: None,
            },
        )
        .await
        .unwrap()
        .expect("damage should kill the target");

    let without_auto_attack = maps.player_runtime_snapshot(map_id, 7).await.unwrap();
    assert!(without_auto_attack.queued_next_melee_spell.is_none());
    let with_auto_attack = maps.player_runtime_snapshot(map_id, 8).await.unwrap();
    assert!(with_auto_attack.queued_next_melee_spell.is_none());
    assert_eq!(with_auto_attack.active_combat_target, None);
    assert_eq!(with_auto_attack.active_combat_next_swing_at, None);

    let other_attacker = ObjectGuid::new(HighGuid::Player, 0, 8);
    let attack_stop = event
        .observer_packets
        .iter()
        .find(|(session, packet)| {
            *session == SessionId(8) && packet.opcode == WorldOpcode::SmsgAttackStop as u16
        })
        .expect("other attacker should receive attack stop for the dead target");
    let mut cursor = 0;
    assert_eq!(
        read_packed_guid(&attack_stop.1.body, &mut cursor).unwrap(),
        other_attacker
    );
    assert_eq!(
        read_packed_guid(&attack_stop.1.body, &mut cursor).unwrap(),
        target
    );
    assert_eq!(
        u32::from_le_bytes(attack_stop.1.body[cursor..cursor + 4].try_into().unwrap()),
        0
    );
    let combat_flag = event
        .observer_packets
        .iter()
        .find(|(session, packet)| {
            if *session != SessionId(8) || packet.opcode != WorldOpcode::SmsgUpdateObject as u16 {
                return false;
            }
            let (values, trailing) = decode_values_update_block(&packet.body[5..], other_attacker);
            trailing.is_empty() && values[UNIT_FIELD_FLAGS] == Some(UNIT_FLAG_PLAYER_CONTROLLED)
        })
        .expect("other attacker should receive player in-combat flag clear");
    assert_eq!(combat_flag.0, SessionId(8));
}

#[tokio::test]
async fn battle_shout_uses_spell_template_gcd_cost_and_aura_slot() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager {
        spell_cast_times: HashMap::new(),
        spell_durations: HashMap::from([(
            4,
            SpellDurationEntry {
                duration_millis: 120_000,
                duration_per_level_millis: 0,
                max_duration_millis: 120_000,
            },
        )]),
        ..MapRuntimeManager::default()
    });
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    object_mgr
        .prime_spell_template_for_test(6673, Some(battle_shout_spell_template()))
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, -8949.95, -132.493, 83.5312, 0.0);
    let mut player = test_player_runtime(7, SessionId::next(), player_position);
    player.power2 = 250;
    maps.add_player(player).await.unwrap();
    let mut body = Vec::new();
    body.extend_from_slice(&6673u32.to_le_bytes());
    body.extend_from_slice(&0u16.to_le_bytes());
    let mut active_spells = HashSet::new();
    active_spells.insert(6673);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 3,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_rage: 250,
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    assert_eq!(session.character.player_rage, 150);
    let snapshot = maps.player_runtime_snapshot(0, 7).await.unwrap();
    assert!(snapshot
        .spell_global_cooldowns_until
        .get(&133)
        .is_some_and(|until| *until > Instant::now()));
    assert_eq!(session.auras.active_auras.len(), 1);
    let active_aura = &session.auras.active_auras[0];
    assert_eq!(active_aura.spell_id, 6673);
    assert_eq!(active_aura.caster, ObjectGuid::new(HighGuid::Player, 0, 7));
    assert_eq!(active_aura.level, 3);
    assert!(active_aura.positive);
    assert_eq!(active_aura.duration_millis, Some(120_000));
    assert!(active_aura
        .expires_at
        .is_some_and(|expires_at| expires_at > Instant::now()));
    assert_eq!(
        active_aura.stat_modifiers,
        vec![AuraStatModifier::AttackPower { amount: 15 }]
    );

    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellStart as u16));
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellGo as u16));
    assert!(
        packets
            .iter()
            .position(|packet| packet.opcode == WorldOpcode::SmsgSpellStart as u16)
            < packets
                .iter()
                .position(|packet| packet.opcode == WorldOpcode::SmsgSpellGo as u16)
    );
    let aura_update = packets
        .iter()
        .find(|packet| {
            packet.opcode == WorldOpcode::SmsgUpdateObject as u16
                && packet
                    .body
                    .windows(4)
                    .any(|window| window == 6673u32.to_le_bytes().as_slice())
        })
        .expect("Battle Shout should update the player's visible aura slot");
    let (values, trailing) = decode_values_update_block(
        &aura_update.body[5..],
        ObjectGuid::new(HighGuid::Player, 0, 7),
    );
    assert!(trailing.is_empty());
    assert_eq!(values[UNIT_FIELD_AURA], Some(6673));
    assert_eq!(values[UNIT_FIELD_AURAFLAGS], Some(POSITIVE_AURA_FLAGS));
    assert_eq!(values[UNIT_FIELD_AURALEVELS], Some(3));
    let duration_packet = packets
        .iter()
        .find(|packet| packet.opcode == WorldOpcode::SmsgUpdateAuraDuration as u16)
        .expect("Battle Shout should send the owner aura duration timer");
    assert_eq!(duration_packet.body[0], 0);
    let remaining = u32::from_le_bytes(duration_packet.body[1..5].try_into().unwrap());
    assert!(remaining > 0 && remaining <= 120_000);

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    assert_eq!(session.character.player_rage, 150);
    let second_packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(second_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgCastResult as u16));
    assert!(second_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellFailure as u16));
    assert!(!second_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellStart as u16));
    assert!(!second_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellGo as u16));
}

#[tokio::test]
async fn rend_applies_harmful_periodic_aura_to_db_creature() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager {
        spell_cast_times: HashMap::new(),
        spell_durations: HashMap::from([(
            21,
            SpellDurationEntry {
                duration_millis: 9_000,
                duration_per_level_millis: 0,
                max_duration_millis: 9_000,
            },
        )]),
        ..MapRuntimeManager::default()
    });
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    object_mgr
        .prime_spell_template_for_test(772, Some(rend_spell_template()))
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    let mut player = test_player_runtime(7, SessionId(7), player_position);
    player.power2 = 200;
    maps.add_player(player).await.unwrap();
    let mut kobold = test_creature_spawn(6);
    kobold.guid = 45;
    kobold.position_x = 4.0;
    kobold.position_y = 0.0;
    kobold.position_z = 0.0;
    kobold.template.npc_flags = 0;
    let target = creature_spawn_guid(&kobold);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(kobold)])
        .await;

    let mut body = Vec::new();
    body.extend_from_slice(&772u32.to_le_bytes());
    body.extend_from_slice(&SPELL_CAST_TARGET_UNIT.to_le_bytes());
    PackedGuid::write(&mut body, target).unwrap();
    let mut active_spells = HashSet::new();
    active_spells.insert(772);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 4,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_rage: 200,
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    assert_eq!(session.character.player_rage, 100);
    assert!(
        session.auras.active_auras.is_empty(),
        "Rend must not become a player buff"
    );
    let creature = maps.db_creature_snapshot(0, target).await.unwrap();
    assert_eq!(creature.health, 120);
    assert_eq!(creature.active_auras.len(), 1);
    let aura = &creature.active_auras[0];
    assert_eq!(aura.spell_id, 772);
    assert!(!aura.positive);
    assert_eq!(aura.duration_millis, Some(9_000));
    assert_eq!(
        aura.periodic_damage,
        Some(PeriodicDamageAura {
            aura_name: SPELL_AURA_PERIODIC_DAMAGE,
            school: 1,
            damage_class: 2,
            attributes_ex2: 0,
            attributes_ex3: 0,
            caster_snapshot: SpellCombatUnitSnapshot {
                level: 4,
                class: 0,
                intellect: 0,
                resistances: [0; MAX_SPELL_SCHOOL],
            },
            amount: 5,
            tick_millis: 3_000,
            next_tick_at: aura.periodic_damage.unwrap().next_tick_at,
        })
    );

    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    let aura_update = packets
        .iter()
        .find(|packet| {
            packet.opcode == WorldOpcode::SmsgUpdateObject as u16
                && packet
                    .body
                    .windows(4)
                    .any(|window| window == 772u32.to_le_bytes().as_slice())
        })
        .expect("Rend should update the creature debuff aura slot");
    let (values, trailing) = decode_values_update_block(&aura_update.body[5..], target);
    assert!(trailing.is_empty());
    let debuff_slot = MAX_POSITIVE_AURA_SLOTS;
    assert_eq!(values[UNIT_FIELD_AURA + debuff_slot], Some(772));
    assert_eq!(
        values[UNIT_FIELD_AURAFLAGS + (debuff_slot / 8)],
        Some(NEGATIVE_AURA_FLAGS)
    );
    assert_eq!(values[UNIT_FIELD_AURALEVELS + (debuff_slot / 4)], Some(4));
    assert!(
        packets
            .iter()
            .all(|packet| packet.opcode != WorldOpcode::SmsgUpdateAuraDuration as u16),
        "CMaNGOS exposes creature debuffs through unit aura fields, not a caster-directed duration packet"
    );

    let tick_at = aura.periodic_damage.unwrap().next_tick_at;
    let tick_packets = maps
        .advance_all_db_creature_auras(tick_at, 1_000)
        .await
        .unwrap();
    let creature = maps.db_creature_snapshot(0, target).await.unwrap();
    assert_eq!(creature.health, 115);
    let periodic_log = tick_packets
        .iter()
        .find(|(_, packet)| packet.opcode == WorldOpcode::SmsgPeriodicAuraLog as u16)
        .expect("Rend tick should broadcast a periodic aura log");
    let mut cursor = PackedGuid::packed_size(target) + PackedGuid::packed_size(aura.caster);
    assert_eq!(read_u32(&periodic_log.1.body, &mut cursor).unwrap(), 772);
    assert_eq!(read_u32(&periodic_log.1.body, &mut cursor).unwrap(), 1);
    assert_eq!(
        read_u32(&periodic_log.1.body, &mut cursor).unwrap(),
        SPELL_AURA_PERIODIC_DAMAGE
    );
    assert_eq!(read_u32(&periodic_log.1.body, &mut cursor).unwrap(), 5);
    assert_eq!(read_u32(&periodic_log.1.body, &mut cursor).unwrap(), 1);
    assert_eq!(read_u32(&periodic_log.1.body, &mut cursor).unwrap(), 0);
    assert_eq!(
        i32::from_le_bytes(periodic_log.1.body[cursor..cursor + 4].try_into().unwrap()),
        0
    );
    assert!(tick_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgUpdateObject as u16));
    maps.advance_all_db_creature_auras(tick_at + Duration::from_millis(3_000), 1_003)
        .await
        .unwrap();
    let creature = maps.db_creature_snapshot(0, target).await.unwrap();
    assert_eq!(creature.health, 110);
    let final_tick_packets = maps
        .advance_all_db_creature_auras(tick_at + Duration::from_millis(6_000), 1_006)
        .await
        .unwrap();
    let creature = maps.db_creature_snapshot(0, target).await.unwrap();
    assert_eq!(creature.health, 105);
    assert!(creature.active_auras.is_empty());
    let final_update = final_tick_packets
        .iter()
        .find(|(_, packet)| packet.opcode == WorldOpcode::SmsgUpdateObject as u16)
        .expect("Rend expiry should broadcast an aura-clear update");
    let (values, trailing) = decode_values_update_block(&final_update.1.body[5..], target);
    assert!(trailing.is_empty());
    assert_eq!(values[UNIT_FIELD_AURA + debuff_slot], Some(0));
    assert_eq!(values[UNIT_FIELD_AURAFLAGS + (debuff_slot / 8)], Some(0));
    assert_eq!(values[UNIT_FIELD_AURALEVELS + (debuff_slot / 4)], Some(0));
    assert_eq!(
        values[UNIT_FIELD_AURAAPPLICATIONS + (debuff_slot / 4)],
        Some(0)
    );
}

#[tokio::test]
async fn charge_moves_player_to_target_instead_of_dealing_remote_damage() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    object_mgr
        .prime_spell_template_for_test(100, Some(charge_spell_template()))
        .await;
    object_mgr
        .prime_spell_template_for_test(7922, Some(charge_stun_spell_template()))
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    maps.add_player(test_player_runtime(7, SessionId(7), player_position))
        .await
        .unwrap();
    let mut kobold = test_creature_spawn(6);
    kobold.guid = 45;
    kobold.position_x = 10.0;
    kobold.position_y = 0.0;
    kobold.position_z = 0.0;
    kobold.template.npc_flags = 0;
    let target = creature_spawn_guid(&kobold);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(kobold)])
        .await;

    let mut body = Vec::new();
    body.extend_from_slice(&100u32.to_le_bytes());
    body.extend_from_slice(&SPELL_CAST_TARGET_UNIT.to_le_bytes());
    PackedGuid::write(&mut body, target).unwrap();
    let mut active_spells = HashSet::new();
    active_spells.insert(100);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 4,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_rage: 0,
            active_spells,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let creature = maps.db_creature_snapshot(0, target).await.unwrap();
    assert_eq!(
        creature.health, 120,
        "Charge must not apply fake remote damage"
    );
    let charged_position = session
        .character
        .active_character
        .as_ref()
        .unwrap()
        .position;
    assert!(charged_position.x > 0.0 && charged_position.x < 10.0);
    assert_eq!(session.combat.active_combat_target, Some(target));
    assert_eq!(session.character.player_rage, 90);
    assert!(
        active_aura_has_stun(&creature.active_auras),
        "Charge should fire its triggered Charge Stun spell"
    );
    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgMonsterMove as u16));
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgAttackStart as u16));
    assert!(!packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellNonMeleeDamageLog as u16));
}

#[tokio::test]
async fn charge_cast_fails_before_movement_when_navigation_is_blocked() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    object_mgr
        .prime_spell_template_for_test(100, Some(charge_spell_template()))
        .await;
    object_mgr
        .prime_spell_template_for_test(7922, Some(charge_stun_spell_template()))
        .await;
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    maps.add_player(test_player_runtime(7, SessionId(7), player_position))
        .await
        .unwrap();
    let mut kobold = test_creature_spawn(6);
    kobold.guid = 45;
    kobold.position_x = 10.0;
    kobold.position_y = 0.0;
    kobold.position_z = 0.0;
    let target = creature_spawn_guid(&kobold);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(kobold)])
        .await;

    let mut body = Vec::new();
    body.extend_from_slice(&100u32.to_le_bytes());
    body.extend_from_slice(&SPELL_CAST_TARGET_UNIT.to_le_bytes());
    PackedGuid::write(&mut body, target).unwrap();
    let mut active_spells = HashSet::new();
    active_spells.insert(100);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 4,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            active_spells,
            ..CharacterSessionState::default()
        },
        movement: MovementSessionState {
            db_creature_navigation: DbCreatureNavigationGuardrail {
                line_of_sight_clear: false,
                ..DbCreatureNavigationGuardrail::default()
            },
            ..MovementSessionState::default()
        },
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        read_cast_spell_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    assert_eq!(
        session
            .character
            .active_character
            .as_ref()
            .unwrap()
            .position,
        player_position
    );
    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(packets.iter().any(|packet| {
        packet.opcode == WorldOpcode::SmsgCastResult as u16
            && packet.body[5] == SPELL_FAILED_LINE_OF_SIGHT
    }));
    assert!(!packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellStart as u16));
    assert!(!packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgMonsterMove as u16));
}

#[tokio::test]
async fn spell_cast_failure_rejects_missing_power_gcd_and_duplicate_queue() {
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let target = ObjectGuid::new(HighGuid::Unit, 6, 45);
    let targets = SpellCastTargets {
        target_mask: SPELL_CAST_TARGET_UNIT,
        unit_target: Some(target),
        gameobject_target: None,
        source_location: None,
        destination: None,
    };

    let map_id = 0;
    let character_guid = 7;
    maps.add_player(test_player_runtime(
        character_guid,
        SessionId::next(),
        WorldPosition::new(map_id, 0.0, 0.0, 0.0, 0.0),
    ))
    .await
    .unwrap();
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: character_guid,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position: WorldPosition::new(map_id, 0.0, 0.0, 0.0, 0.0),
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    let heroic_template = heroic_strike_spell_template();
    let profile = player_spell_cast_profile(&heroic_template).unwrap();
    session.death.player_death_state = PlayerDeathState::Corpse;
    session.character.player_health = 0;
    assert_eq!(
        spell_cast_failure(
            shared_world,
            &world_db_pool,
            &mut session,
            &heroic_template,
            &profile,
            &targets,
            Instant::now()
        )
        .await,
        Some(SPELL_FAILED_CASTER_DEAD)
    );

    session.death.player_death_state = PlayerDeathState::Alive;
    assert_eq!(
        spell_cast_failure(
            shared_world,
            &world_db_pool,
            &mut session,
            &heroic_template,
            &profile,
            &targets,
            Instant::now()
        )
        .await,
        Some(SPELL_FAILED_NO_POWER)
    );

    maps.set_player_power2(map_id, character_guid, HEROIC_STRIKE_RAGE_COST)
        .await;
    maps.set_active_player_spell_cast(
        map_id,
        character_guid,
        ActivePlayerSpellCast {
            spell_id: WARRIOR_HEROIC_STRIKE_RANK_1,
            source: ActivePlayerSpellCastSource::Player,
            profile,
            targets: PendingSpellCastTargets {
                target_mask: targets.target_mask,
                unit_target: targets.unit_target,
                gameobject_target: targets.gameobject_target,
                source_location: None,
                destination: None,
            },
            due_at: Instant::now() + Duration::from_millis(1_500),
            cast_time_millis: 1_500,
            interrupt_flags: 0,
            damage_pushback_count: 0,
        },
    )
    .await;
    assert_eq!(
        spell_cast_failure(
            shared_world,
            &world_db_pool,
            &mut session,
            &heroic_template,
            &profile,
            &targets,
            Instant::now()
        )
        .await,
        Some(SPELL_FAILED_SPELL_IN_PROGRESS)
    );
    maps.cancel_active_player_spell_cast(map_id, character_guid)
        .await;

    let gcd_profile = SpellCastProfile {
        spell_id: 999_001,
        kind: SpellCastKind::InstantDamage,
        aura_target: SpellAuraTarget::Caster,
        bonus_damage: 0,
        weapon_damage_percent: 100,
        damage: 1,
        power: SpellPowerCost::Rage { cost: 0 },
        requires_melee: false,
        requires_behind: false,
        needs_combo_points: false,
        global_cooldown_category: 133,
        global_cooldown_millis: 1500,
        cooldown_category: 0,
        category_cooldown_millis: 0,
        cooldown_millis: 0,
    };
    let gcd_template = test_spell_template(999_001);
    maps.apply_player_spell_cooldowns(map_id, character_guid, &gcd_profile, Instant::now(), false)
        .await;
    assert_eq!(
        spell_cast_failure(
            shared_world,
            &world_db_pool,
            &mut session,
            &gcd_template,
            &gcd_profile,
            &targets,
            Instant::now()
        )
        .await,
        Some(SPELL_FAILED_NOT_READY)
    );

    let queued = QueuedNextMeleeSpell {
        spell_id: WARRIOR_HEROIC_STRIKE_RANK_1,
        target,
        bonus_damage: HEROIC_STRIKE_FIXTURE_DAMAGE,
        rage_cost: HEROIC_STRIKE_RAGE_COST,
        mana_cost: 0,
    };
    maps.queue_player_next_melee_spell(map_id, character_guid, queued)
        .await;
    assert_eq!(
        spell_cast_failure(
            shared_world,
            &world_db_pool,
            &mut session,
            &heroic_template,
            &profile,
            &targets,
            Instant::now()
        )
        .await,
        Some(SPELL_FAILED_NOT_READY)
    );
}

#[test]
fn player_spell_cast_failure_rejects_hard_control_auras() {
    let now = Instant::now();
    let mut map = MapRuntime::new(0, 0);
    insert_map_runtime_player_for_test(&mut map, 7, WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0));
    {
        let player = map.players.get_mut(&7).expect("player");
        player.power1 = 100;
        player.max_power1 = 100;
        player.power2 = HEROIC_STRIKE_RAGE_COST;
    }
    let fireball_template = fireball_spell_template();
    let fireball_profile = player_spell_cast_profile(&fireball_template).unwrap();
    let heroic_template = heroic_strike_spell_template();
    let heroic_profile = player_spell_cast_profile(&heroic_template).unwrap();

    for (modifier, expected) in [
        (AuraStatModifier::Stun, SPELL_FAILED_STUNNED),
        (AuraStatModifier::Confuse, SPELL_FAILED_CONFUSED),
        (AuraStatModifier::Fear, SPELL_FAILED_FLEEING),
        (AuraStatModifier::Silence, SPELL_FAILED_SILENCED),
        (AuraStatModifier::PacifySilence, SPELL_FAILED_SILENCED),
    ] {
        map.players.get_mut(&7).expect("player").active_auras =
            vec![test_control_aura(modifier, now)];
        assert_eq!(
            map.player_spell_cast_failure(7, &fireball_profile, now),
            Some(expected)
        );
    }

    map.players.get_mut(&7).expect("player").active_auras =
        vec![test_control_aura(AuraStatModifier::Pacify, now)];
    assert_eq!(
        map.player_spell_cast_failure(7, &fireball_profile, now),
        None
    );
    assert_eq!(
        map.player_spell_cast_failure(7, &heroic_profile, now),
        Some(SPELL_FAILED_PACIFIED)
    );
}

#[test]
fn applying_hard_control_aura_interrupts_active_player_cast() {
    let now = Instant::now();
    let mut map = MapRuntime::new(0, 0);
    insert_map_runtime_player_for_test(&mut map, 7, WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0));
    let fireball = fireball_spell_template();
    map.active_player_spell_casts.insert(
        7,
        ActivePlayerSpellCast {
            spell_id: fireball.id,
            source: ActivePlayerSpellCastSource::Player,
            profile: player_spell_cast_profile(&fireball).unwrap(),
            targets: PendingSpellCastTargets {
                target_mask: 0,
                unit_target: None,
                gameobject_target: None,
                source_location: None,
                destination: None,
            },
            due_at: now + Duration::from_millis(1_500),
            cast_time_millis: 1_500,
            interrupt_flags: 0,
            damage_pushback_count: 0,
        },
    );

    let event = map
        .apply_player_aura(7, test_control_aura(AuraStatModifier::Stun, now))
        .unwrap()
        .expect("aura event");

    assert!(
        !map.active_player_spell_casts.contains_key(&7),
        "CMaNGOS interrupts active non-melee spells when hard control lands"
    );
    assert!(event
        .direct_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgCastResult as u16));
    assert!(event
        .direct_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellFailure as u16));
    assert!(event
        .direct_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgSpellFailedOther as u16));
}

#[test]
fn applying_silence_and_pacify_only_interrupt_matching_existing_casts() {
    let now = Instant::now();
    let mut map = MapRuntime::new(0, 0);
    insert_map_runtime_player_for_test(&mut map, 7, WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0));
    let fireball = fireball_spell_template();
    map.active_player_spell_casts.insert(
        7,
        ActivePlayerSpellCast {
            spell_id: fireball.id,
            source: ActivePlayerSpellCastSource::Player,
            profile: player_spell_cast_profile(&fireball).unwrap(),
            targets: PendingSpellCastTargets {
                target_mask: 0,
                unit_target: None,
                gameobject_target: None,
                source_location: None,
                destination: None,
            },
            due_at: now + Duration::from_millis(1_500),
            cast_time_millis: 1_500,
            interrupt_flags: 0,
            damage_pushback_count: 0,
        },
    );

    map.apply_player_aura(7, test_control_aura(AuraStatModifier::Silence, now))
        .unwrap();

    assert!(
        !map.active_player_spell_casts.contains_key(&7),
        "CMaNGOS silence interrupts active spells with silence prevention"
    );

    let heroic = heroic_strike_spell_template();
    map.active_player_spell_casts.insert(
        7,
        ActivePlayerSpellCast {
            spell_id: heroic.id,
            source: ActivePlayerSpellCastSource::Player,
            profile: player_spell_cast_profile(&heroic).unwrap(),
            targets: PendingSpellCastTargets {
                target_mask: 0,
                unit_target: None,
                gameobject_target: None,
                source_location: None,
                destination: None,
            },
            due_at: now + Duration::from_millis(1_500),
            cast_time_millis: 1_500,
            interrupt_flags: 0,
            damage_pushback_count: 0,
        },
    );

    map.apply_player_aura(7, test_control_aura(AuraStatModifier::Silence, now))
        .unwrap();

    assert!(
        map.active_player_spell_casts.contains_key(&7),
        "CMaNGOS silence interrupts only spells with silence prevention"
    );

    map.apply_player_aura(7, test_control_aura(AuraStatModifier::Pacify, now))
        .unwrap();

    assert!(
        map.active_player_spell_casts.contains_key(&7),
        "CMaNGOS pacify blocks new melee-prevented casts but does not CastStop an existing spell"
    );
}

#[test]
fn player_auto_attack_due_pauses_while_hard_controlled() {
    let now = Instant::now();
    let mut map = MapRuntime::new(0, 0);
    insert_map_runtime_player_for_test(&mut map, 7, WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0));
    let target = ObjectGuid::new(HighGuid::Unit, 0, 45);
    map.set_player_auto_attack(7, Some(target), Some(now));

    for modifier in [
        AuraStatModifier::Pacify,
        AuraStatModifier::PacifySilence,
        AuraStatModifier::Stun,
        AuraStatModifier::Confuse,
        AuraStatModifier::Fear,
    ] {
        map.players.get_mut(&7).expect("player").active_auras =
            vec![test_control_aura(modifier, now)];
        assert_eq!(map.player_auto_attack_due(7, now), None);
    }

    map.players
        .get_mut(&7)
        .expect("player")
        .active_auras
        .clear();
    assert_eq!(
        map.player_auto_attack_due(7, now),
        Some(PlayerAutoAttackDue {
            target,
            kind: PlayerAutoAttackKind::Melee,
        })
    );
}

#[tokio::test]
async fn hostile_spell_cast_failure_checks_range_los_and_facing_from_map() {
    let mut manager = MapRuntimeManager::default();
    manager.spell_ranges.insert(
        900,
        SpellRangeEntry {
            min_range: 0.0,
            max_range: 30.0,
            flags: 0,
        },
    );
    let maps = Arc::new(manager);
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    object_mgr
        .prime_spell_facing_flag_for_test(133, Some(1))
        .await;
    let map_id = 0;
    let character_guid = 7;
    let player_position = WorldPosition::new(map_id, 0.0, 0.0, 0.0, 0.0);
    let mut player = test_player_runtime(character_guid, SessionId(7), player_position);
    let caster_world_stats = PlayerWorldStats {
        base_health: 20,
        base_mana: 80,
        stats: [23, 20, 22, 20, 20],
        next_level_xp: 400,
    };
    player.base_world_stats = caster_world_stats;
    player.effective_world_stats = caster_world_stats;
    player.max_power1 = caster_world_stats.max_mana();
    player.power1 = 100;
    maps.add_player(player).await.unwrap();
    let mut creature_spawn = test_creature_spawn(6);
    creature_spawn.guid = 66;
    creature_spawn.position_x = 40.0;
    creature_spawn.position_y = 0.0;
    creature_spawn.position_z = 0.0;
    let target = creature_spawn_guid(&creature_spawn);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(creature_spawn)])
        .await;

    let mut template = fireball_spell_template();
    template.range_index = 900;
    let profile = player_spell_cast_profile(&template).unwrap();
    let targets = SpellCastTargets {
        target_mask: SPELL_CAST_TARGET_UNIT,
        unit_target: Some(target),
        gameobject_target: None,
        source_location: None,
        destination: None,
    };
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: character_guid,
                name: "Ada".to_string(),
                race: 1,
                class: 8,
                level: 1,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_health: caster_world_stats.max_health(),
            player_mana: 100,
            ..CharacterSessionState::default()
        },
        movement: MovementSessionState {
            db_creature_navigation: DbCreatureNavigationGuardrail::default(),
            ..MovementSessionState::default()
        },
        ..WorldSessionState::default()
    };

    assert_eq!(
        spell_cast_failure(
            shared_world,
            &world_db_pool,
            &mut session,
            &template,
            &profile,
            &targets,
            Instant::now()
        )
        .await,
        Some(SPELL_FAILED_OUT_OF_RANGE)
    );

    let mut closer = maps.db_creature_snapshot(map_id, target).await.unwrap();
    closer.current_position.x = 10.0;
    maps.update_db_creature_snapshot(map_id, closer).await;
    session.movement.db_creature_navigation.line_of_sight_clear = false;
    assert_eq!(
        spell_cast_failure(
            shared_world,
            &world_db_pool,
            &mut session,
            &template,
            &profile,
            &targets,
            Instant::now()
        )
        .await,
        Some(SPELL_FAILED_LINE_OF_SIGHT)
    );

    session.movement.db_creature_navigation.line_of_sight_clear = true;
    session
        .character
        .active_character
        .as_mut()
        .unwrap()
        .position
        .orientation = std::f32::consts::PI;
    maps.sync_player_gameplay_state(map_id, character_guid, &session)
        .await;
    assert_eq!(
        spell_cast_failure(
            shared_world,
            &world_db_pool,
            &mut session,
            &template,
            &profile,
            &targets,
            Instant::now()
        )
        .await,
        Some(SPELL_FAILED_UNIT_NOT_INFRONT)
    );

    session
        .character
        .active_character
        .as_mut()
        .unwrap()
        .position
        .orientation = 0.0;
    maps.sync_player_gameplay_state(map_id, character_guid, &session)
        .await;
    assert_eq!(
        spell_cast_failure(
            shared_world,
            &world_db_pool,
            &mut session,
            &template,
            &profile,
            &targets,
            Instant::now()
        )
        .await,
        None
    );
}

#[tokio::test]
async fn polymorph_cast_failure_does_not_require_facing_without_spell_facing_flag() {
    let mut manager = MapRuntimeManager::default();
    manager.spell_ranges.insert(
        900,
        SpellRangeEntry {
            min_range: 0.0,
            max_range: 30.0,
            flags: 0,
        },
    );
    let maps = Arc::new(manager);
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    object_mgr.prime_spell_facing_flag_for_test(118, None).await;

    let map_id = 0;
    let character_guid = 7;
    let player_position = WorldPosition::new(map_id, 0.0, 0.0, 0.0, std::f32::consts::PI);
    let mut player = test_player_runtime(character_guid, SessionId(7), player_position);
    let caster_world_stats = PlayerWorldStats {
        base_health: 20,
        base_mana: 80,
        stats: [23, 20, 22, 20, 20],
        next_level_xp: 400,
    };
    player.base_world_stats = caster_world_stats;
    player.effective_world_stats = caster_world_stats;
    player.max_power1 = caster_world_stats.max_mana();
    player.power1 = 100;
    maps.add_player(player).await.unwrap();
    let mut creature_spawn = test_creature_spawn(6);
    creature_spawn.guid = 66;
    creature_spawn.position_x = 10.0;
    creature_spawn.position_y = 0.0;
    creature_spawn.position_z = 0.0;
    let target = creature_spawn_guid(&creature_spawn);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(creature_spawn)])
        .await;

    let mut template = test_spell_template(118);
    template.range_index = 900;
    template.dispel = 1;
    template.mechanic = MECHANIC_POLYMORPH;
    template.spell_family_name = SPELL_FAMILY_MAGE;
    template.spell_family_flags = 0x0100_0000;
    template.aura_interrupt_flags = AURA_INTERRUPT_FLAG_DAMAGE;
    template.effect1 = SPELL_EFFECT_APPLY_AURA;
    template.effect_apply_aura_name1 = SPELL_AURA_MOD_CONFUSE;
    template.effect_implicit_target_a1 = TARGET_UNIT_ENEMY;
    template.effect2 = SPELL_EFFECT_APPLY_AURA;
    template.effect_apply_aura_name2 = SPELL_AURA_TRANSFORM;
    template.effect_misc_value2 = 16372;
    template.effect_implicit_target_a2 = TARGET_UNIT_ENEMY;
    let profile = player_spell_cast_profile(&template).unwrap();
    let targets = SpellCastTargets {
        target_mask: SPELL_CAST_TARGET_UNIT,
        unit_target: Some(target),
        gameobject_target: None,
        source_location: None,
        destination: None,
    };
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: character_guid,
                name: "Ada".to_string(),
                race: 1,
                class: 8,
                level: 1,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_health: caster_world_stats.max_health(),
            player_mana: 100,
            ..CharacterSessionState::default()
        },
        movement: MovementSessionState {
            db_creature_navigation: DbCreatureNavigationGuardrail::default(),
            ..MovementSessionState::default()
        },
        ..WorldSessionState::default()
    };

    assert_eq!(
        spell_cast_failure(
            shared_world,
            &world_db_pool,
            &mut session,
            &template,
            &profile,
            &targets,
            Instant::now()
        )
        .await,
        None
    );
}
