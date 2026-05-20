#[test]
fn combat_packets_match_cmangos_melee_shapes() {
    let attacker = ObjectGuid::new(HighGuid::Player, 0, 7);
    let victim = ObjectGuid::new(HighGuid::Unit, 6, 45);
    let damage = 10;

    let start = build_attack_start_body(attacker, victim);
    assert_eq!(&start[0..8], &attacker.raw().to_le_bytes());
    assert_eq!(&start[8..16], &victim.raw().to_le_bytes());

    let stop = build_attack_stop_body(attacker, victim, false).unwrap();
    assert_eq!(&stop[stop.len() - 4..], &0u32.to_le_bytes());

    let state = build_attacker_state_update_body(attacker, victim, damage).unwrap();
    let mut cursor = 0;
    assert_eq!(read_u32(&state, &mut cursor).unwrap(), HITINFO_NORMALSWING2);
    cursor += PackedGuid::packed_size(attacker) + PackedGuid::packed_size(victim);
    assert_eq!(read_u32(&state, &mut cursor).unwrap(), damage);
    assert_eq!(state[cursor], 1);
    cursor += 1;
    assert_eq!(read_u32(&state, &mut cursor).unwrap(), 0);
    assert_eq!(
        f32::from_le_bytes(state[cursor..cursor + 4].try_into().unwrap()),
        damage as f32
    );
    cursor += 4;
    assert_eq!(read_u32(&state, &mut cursor).unwrap(), damage);
    assert_eq!(read_u32(&state, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&state, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&state, &mut cursor).unwrap(), VICTIMSTATE_NORMAL);
}

#[test]
fn combat_log_spell_packets_match_cmangos_shapes() {
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    let target = creature_spawn_guid(&{
        let mut spawn = test_creature_spawn(6);
        spawn.guid = 45;
        spawn
    });

    let spell_damage = build_spell_non_melee_damage_log_body(SpellNonMeleeDamageLogPacket {
        attacker: caster,
        target,
        spell_id: WARRIOR_HEROIC_STRIKE_RANK_1,
        damage: 11,
        school: 0,
        absorb: 2,
        resist: -1,
        periodic: false,
        blocked: 3,
        hit_info: 0,
    })
    .unwrap();
    let mut cursor = 0;
    assert_eq!(
        read_packed_guid(&spell_damage, &mut cursor).unwrap(),
        target
    );
    assert_eq!(
        read_packed_guid(&spell_damage, &mut cursor).unwrap(),
        caster
    );
    assert_eq!(
        read_u32(&spell_damage, &mut cursor).unwrap(),
        WARRIOR_HEROIC_STRIKE_RANK_1
    );
    assert_eq!(read_u32(&spell_damage, &mut cursor).unwrap(), 11);
    assert_eq!(spell_damage[cursor], 0);
    cursor += 1;
    assert_eq!(read_u32(&spell_damage, &mut cursor).unwrap(), 2);
    assert_eq!(
        i32::from_le_bytes(spell_damage[cursor..cursor + 4].try_into().unwrap()),
        -1
    );
    cursor += 4;
    assert_eq!(spell_damage[cursor], 0);
    cursor += 1;
    assert_eq!(spell_damage[cursor], 0);
    cursor += 1;
    assert_eq!(read_u32(&spell_damage, &mut cursor).unwrap(), 3);
    assert_eq!(read_u32(&spell_damage, &mut cursor).unwrap(), 0);
    assert_eq!(spell_damage[cursor], 0);
    cursor += 1;
    assert_eq!(cursor, spell_damage.len());

    let spell_failure = build_spell_failure_body(
        caster,
        WARRIOR_HEROIC_STRIKE_RANK_1,
        SPELL_FAILED_OUT_OF_RANGE,
    )
    .unwrap();
    let mut cursor = 0;
    assert_eq!(
        read_packed_guid(&spell_failure, &mut cursor).unwrap(),
        caster
    );
    assert_eq!(
        read_u32(&spell_failure, &mut cursor).unwrap(),
        WARRIOR_HEROIC_STRIKE_RANK_1
    );
    assert_eq!(spell_failure[cursor], SPELL_FAILED_OUT_OF_RANGE);
    cursor += 1;
    assert_eq!(cursor, spell_failure.len());

    let spell_failed_other = build_spell_failed_other_body(caster, WARRIOR_HEROIC_STRIKE_RANK_1);
    assert_eq!(&spell_failed_other[0..8], &caster.raw().to_le_bytes());
    assert_eq!(
        &spell_failed_other[8..12],
        &WARRIOR_HEROIC_STRIKE_RANK_1.to_le_bytes()
    );

    let heal_log = build_spell_heal_log_body(caster, target, 2050, 17, true).unwrap();
    let mut cursor = 0;
    assert_eq!(read_packed_guid(&heal_log, &mut cursor).unwrap(), target);
    assert_eq!(read_packed_guid(&heal_log, &mut cursor).unwrap(), caster);
    assert_eq!(read_u32(&heal_log, &mut cursor).unwrap(), 2050);
    assert_eq!(read_u32(&heal_log, &mut cursor).unwrap(), 17);
    assert_eq!(heal_log[cursor], 1);

    let energize_log =
        build_spell_energize_log_body(caster, target, 1127, POWER_TYPE_MANA, 9).unwrap();
    let mut cursor = 0;
    assert_eq!(
        read_packed_guid(&energize_log, &mut cursor).unwrap(),
        target
    );
    assert_eq!(
        read_packed_guid(&energize_log, &mut cursor).unwrap(),
        caster
    );
    assert_eq!(read_u32(&energize_log, &mut cursor).unwrap(), 1127);
    assert_eq!(
        read_u32(&energize_log, &mut cursor).unwrap(),
        POWER_TYPE_MANA
    );
    assert_eq!(read_u32(&energize_log, &mut cursor).unwrap(), 9);

    let miss_log = build_spell_log_miss_body(caster, target, 1752, SPELL_MISS_DODGE).unwrap();
    let mut cursor = 0;
    assert_eq!(read_u32(&miss_log, &mut cursor).unwrap(), 1752);
    assert_eq!(
        u64::from_le_bytes(miss_log[cursor..cursor + 8].try_into().unwrap()),
        caster.raw()
    );
    cursor += 8;
    assert_eq!(miss_log[cursor], 0);
    cursor += 1;
    assert_eq!(read_u32(&miss_log, &mut cursor).unwrap(), 1);
    assert_eq!(
        u64::from_le_bytes(miss_log[cursor..cursor + 8].try_into().unwrap()),
        target.raw()
    );
    cursor += 8;
    assert_eq!(miss_log[cursor], SPELL_MISS_DODGE);
}

#[test]
fn melee_roll_table_orders_cmangos_defensive_outcomes() {
    let chances = MeleeRollChances {
        miss: 5.0,
        dodge: 5.0,
        parry: 5.0,
        block: 5.0,
        glancing: 5.0,
        crit: 5.0,
        crushing: 5.0,
    };

    assert_eq!(roll_melee_outcome(chances, 1), MeleeHitOutcome::Miss);
    assert_eq!(roll_melee_outcome(chances, 501), MeleeHitOutcome::Dodge);
    assert_eq!(roll_melee_outcome(chances, 1_001), MeleeHitOutcome::Parry);
    assert_eq!(roll_melee_outcome(chances, 1_501), MeleeHitOutcome::Block);
    assert_eq!(
        roll_melee_outcome(chances, 2_001),
        MeleeHitOutcome::Glancing
    );
    assert_eq!(roll_melee_outcome(chances, 2_501), MeleeHitOutcome::Crit);
    assert_eq!(
        roll_melee_outcome(chances, 3_001),
        MeleeHitOutcome::Crushing
    );
    assert_eq!(roll_melee_outcome(chances, 3_501), MeleeHitOutcome::Normal);
}

#[test]
fn melee_damage_outcome_serializes_miss_and_block_like_attacker_state_update() {
    let attacker = ObjectGuid::new(HighGuid::Unit, 0, 101);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 7);
    let input = MeleeDamageInput {
        attacker_level: 1,
        attacker_skill: 5,
        victim_defense: 5,
        min_damage: 10.0,
        max_damage: 10.0,
        victim_armor: 0,
        victim_block_value: 4,
        chances: MeleeRollChances {
            miss: 5.0,
            dodge: 0.0,
            parry: 0.0,
            block: 5.0,
            glancing: 0.0,
            crit: 0.0,
            crushing: 0.0,
        },
    };

    let miss = calculate_melee_damage(input, 1, 1);
    assert_eq!(miss.total_damage, 0);
    assert_eq!(miss.hit_info, HITINFO_NORMALSWING2 | HITINFO_MISS);
    assert_eq!(miss.victim_state, VICTIMSTATE_UNAFFECTED);
    let body = build_attacker_state_update_body_for_outcome(attacker, victim, miss, 0).unwrap();
    let mut cursor = 0;
    assert_eq!(
        read_u32(&body, &mut cursor).unwrap(),
        HITINFO_NORMALSWING2 | HITINFO_MISS
    );
    cursor += PackedGuid::packed_size(attacker) + PackedGuid::packed_size(victim);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);

    let block = calculate_melee_damage(input, 1, 501);
    assert_eq!(block.total_damage, 6);
    assert_eq!(block.blocked, 4);
    assert_eq!(block.hit_info, HITINFO_NORMALSWING2 | HITINFO_BLOCK);
    assert_eq!(block.victim_state, VICTIMSTATE_NORMAL);
}

#[test]
fn next_melee_spell_bonus_does_not_turn_avoids_into_damage() {
    let miss = MeleeDamageOutcome {
        hit_info: HITINFO_NORMALSWING2 | HITINFO_MISS,
        victim_state: VICTIMSTATE_UNAFFECTED,
        outcome: MeleeHitOutcome::Miss,
        total_damage: 0,
        school_damage: 0,
        absorbed: 0,
        resisted: 0,
        blocked: 0,
    };

    let avoided = miss.with_next_melee_spell_bonus(HEROIC_STRIKE_FIXTURE_DAMAGE);
    assert_eq!(avoided.total_damage, 0);
    assert_eq!(avoided.school_damage, 0);
    assert_eq!(avoided.outcome, MeleeHitOutcome::Miss);

    let landed =
        MeleeDamageOutcome::normal_hit(8).with_next_melee_spell_bonus(HEROIC_STRIKE_FIXTURE_DAMAGE);
    assert_eq!(landed.total_damage, 8 + HEROIC_STRIKE_FIXTURE_DAMAGE);
    assert_eq!(landed.school_damage, 8 + HEROIC_STRIKE_FIXTURE_DAMAGE);
}

#[test]
fn spell_damage_outcome_carries_crit_resist_and_full_resist() {
    let mut target_resistances = [0i16; MAX_SPELL_SCHOOL];
    let base = SpellDamageOutcomeInput {
        damage: 100,
        school: 2,
        dmg_class: SPELL_DAMAGE_CLASS_MAGIC,
        attributes_ex2: 0,
        attributes_ex3: 0,
        caster_class: 8,
        caster_level: 10,
        caster_intellect: 100,
        target_level: 10,
        target_resistances,
    };

    let crit = calculate_spell_damage_outcome(
        base,
        SpellDamageOutcomeRolls {
            hit_roll: 10_000,
            crit_roll: 1,
            partial_resist_roll: 1,
        },
    );
    assert_eq!(crit.final_damage, 150);
    assert_eq!(crit.hit_info, SPELL_HIT_TYPE_CRIT);
    assert_eq!(crit.miss_info, None);

    target_resistances[2] = 1_000;
    let high_resistance = SpellDamageOutcomeInput {
        target_resistances,
        attributes_ex2: SPELL_ATTR_EX2_CANT_CRIT,
        ..base
    };
    let partial = calculate_spell_damage_outcome(
        high_resistance,
        SpellDamageOutcomeRolls {
            hit_roll: 10_000,
            crit_roll: 1,
            partial_resist_roll: 10_000,
        },
    );
    assert_eq!(partial.final_damage, 20);
    assert_eq!(partial.resist, 80);
    assert_eq!(partial.hit_info, 0);
    assert_eq!(partial.miss_info, None);

    let resisted = calculate_spell_damage_outcome(
        high_resistance,
        SpellDamageOutcomeRolls {
            hit_roll: 1,
            crit_roll: 10_000,
            partial_resist_roll: 1,
        },
    );
    assert_eq!(resisted.final_damage, 0);
    assert_eq!(resisted.miss_info, Some(SPELL_MISS_RESIST));
}

#[test]
fn spell_damage_outcome_uses_unit_snapshots_for_player_and_creature_targets() {
    let mut creature_spawn = test_creature_spawn(6);
    creature_spawn.template.max_level = 10;
    creature_spawn.template.resistance_fire = 1_000;
    let creature = DbCreatureRuntime::new(creature_spawn);
    let creature_snapshot = db_creature_spell_snapshot(&creature);
    let mut player_stats = test_player_combat_stats();
    player_stats.resistances[2] = 1_000;
    let player_snapshot = player_spell_snapshot(10, 8, &player_stats);

    let creature_target = calculate_spell_damage_outcome(
        spell_damage_outcome_input(
            100,
            2,
            SPELL_DAMAGE_CLASS_MAGIC,
            SPELL_ATTR_EX2_CANT_CRIT,
            0,
            player_snapshot,
            creature_snapshot,
        ),
        SpellDamageOutcomeRolls {
            hit_roll: 10_000,
            crit_roll: 1,
            partial_resist_roll: 10_000,
        },
    );
    let player_target = calculate_spell_damage_outcome(
        spell_damage_outcome_input(
            100,
            2,
            SPELL_DAMAGE_CLASS_MAGIC,
            SPELL_ATTR_EX2_CANT_CRIT,
            0,
            creature_snapshot,
            player_snapshot,
        ),
        SpellDamageOutcomeRolls {
            hit_roll: 10_000,
            crit_roll: 1,
            partial_resist_roll: 10_000,
        },
    );

    assert_eq!(creature_target.final_damage, 20);
    assert_eq!(creature_target.resist, 80);
    assert_eq!(player_target.final_damage, 20);
    assert_eq!(player_target.resist, 80);
}

#[test]
fn armor_reduced_damage_matches_cmangos_cap_shape() {
    assert_eq!(armor_reduced_damage(1, 0, 10.0), 10);
    let reduced = armor_reduced_damage(1, 100, 10.0);
    assert!(reduced < 10);
    assert_eq!(armor_reduced_damage(60, 999_999, 100.0), 25);
}

#[test]
fn player_miss_chance_uses_weapon_skill_against_creature_defense() {
    let even = player_main_hand_chances_against_db_creature(&test_player_combat_stats(), 5, 5, 1);
    let under_skilled =
        player_main_hand_chances_against_db_creature(&test_player_combat_stats(), 1, 5, 1);
    let heavily_under_skilled =
        player_main_hand_chances_against_db_creature(&test_player_combat_stats(), 5, 20, 4);

    assert_eq!(even.miss, 5.0);
    assert!(under_skilled.miss > even.miss);
    assert_eq!(heavily_under_skilled.miss, 9.0);
}

#[test]
fn combat_skill_progression_uses_intellect_for_weapon_skills_only() {
    let mut without_intellect = vec![CharacterSkill {
        skill: SKILL_SWORDS,
        value: 250,
        max: 300,
    }];
    let missed = try_advance_combat_skill_value_with_rolls(
        60,
        SKILL_SWORDS,
        0,
        true,
        &mut without_intellect,
        || 20.0,
        || 512,
    );
    assert!(missed.is_none());
    assert_eq!(without_intellect[0].value, 250);

    let mut with_intellect = vec![CharacterSkill {
        skill: SKILL_SWORDS,
        value: 250,
        max: 300,
    }];
    let advanced = try_advance_combat_skill_value_with_rolls(
        60,
        SKILL_SWORDS,
        20,
        true,
        &mut with_intellect,
        || 20.0,
        || 512,
    )
    .expect("intellect should raise the weapon skill-up chance enough to pass");
    assert_eq!(advanced.value, 251);
    assert_eq!(with_intellect[0].value, 251);

    let mut defense = vec![CharacterSkill {
        skill: SKILL_DEFENSE,
        value: 250,
        max: 300,
    }];
    let defense_missed = try_advance_combat_skill_value_with_rolls(
        60,
        SKILL_DEFENSE,
        999,
        false,
        &mut defense,
        || 20.0,
        || 512,
    );
    assert!(defense_missed.is_none());
    assert_eq!(defense[0].value, 250);
}

#[test]
fn combat_skill_progression_updates_level_cap_and_value() {
    let mut skills = vec![test_skill(SKILL_UNARMED, 1, 5)];
    let updated = try_advance_combat_skill_value_with_rolls(
        2,
        SKILL_UNARMED,
        0,
        true,
        &mut skills,
        || 0.0,
        || 512,
    )
    .expect("expected unarmed skill progression update");

    assert_eq!(updated.slot, 0);
    assert_eq!(updated.skill, SKILL_UNARMED);
    assert_eq!(updated.value, 2);
    assert_eq!(updated.max, 10);
    assert_eq!(skills[0].value, 2);
    assert_eq!(skills[0].max, 10);
}

#[test]
fn level_up_updates_combat_skill_maxes_without_waiting_for_skill_gain() {
    let mut skills = vec![
        test_skill(SKILL_DEFENSE, 4, 5),
        test_skill(98, 300, 300),
        test_skill(SKILL_SWORDS, 3, 5),
        test_skill(SKILL_UNARMED, 5, 5),
    ];

    let maps = MapRuntimeManager::default();
    let updates = sync_player_level_backed_skills(&maps, 1, 1, 2, &mut skills);

    assert_eq!(updates.len(), 3);
    assert_eq!(skills[0].max, 10);
    assert_eq!(skills[1].max, 300);
    assert_eq!(skills[2].max, 10);
    assert_eq!(skills[3].max, 10);
    assert_eq!(skills[0].value, 4);
    assert_eq!(skills[2].value, 3);
    assert_eq!(skills[3].value, 5);

    let body = build_player_skill_updates_body(7, &updates, &[]).unwrap();
    let packed_guid_mask = body[6];
    let values_start = 4 + 1 + 1 + 1 + packed_guid_mask.count_ones() as usize;
    let values = decode_update_values(&body[values_start..]);

    assert_eq!(
        values[PLAYER_SKILL_INFO_1_1],
        Some(make_pair32(SKILL_DEFENSE, 0))
    );
    assert_eq!(values[PLAYER_SKILL_INFO_1_1 + 1], Some(make_pair32(4, 10)));
    assert_eq!(
        values[PLAYER_SKILL_INFO_1_1 + 6],
        Some(make_pair32(SKILL_SWORDS, 0))
    );
    assert_eq!(values[PLAYER_SKILL_INFO_1_1 + 7], Some(make_pair32(3, 10)));
    assert_eq!(
        values[PLAYER_SKILL_INFO_1_1 + 9],
        Some(make_pair32(SKILL_UNARMED, 0))
    );
    assert_eq!(values[PLAYER_SKILL_INFO_1_1 + 10], Some(make_pair32(5, 10)));
}

#[test]
fn item_weapon_skill_mapping_matches_cmangos_known_ids() {
    let mut sword = test_item_template(25, ITEM_CLASS_WEAPON, 13, 2.0, 4.0, 0);
    sword.subclass = 7;
    let mut fist = test_item_template(26, ITEM_CLASS_WEAPON, 13, 2.0, 4.0, 0);
    fist.subclass = 13;
    let mut unknown = test_item_template(27, ITEM_CLASS_WEAPON, 13, 2.0, 4.0, 0);
    unknown.subclass = 9;

    assert_eq!(item_weapon_skill_from_template(&sword), Some(SKILL_SWORDS));
    assert_eq!(
        item_weapon_skill_from_template(&fist),
        Some(SKILL_FIST_WEAPONS)
    );
    assert_eq!(item_weapon_skill_from_template(&unknown), None);
}

#[test]
fn player_main_hand_damage_uses_equipped_weapon_and_attack_power() {
    let weapon = test_item_template(25, ITEM_CLASS_WEAPON, 13, 2.0, 4.0, 0);
    let equipped = [equipped_template(EQUIPMENT_SLOT_MAINHAND, weapon)];
    let world_stats = PlayerWorldStats {
        base_health: 20,
        base_mana: 0,
        stats: [23, 20, 22, 20, 20],
        next_level_xp: 400,
    };
    let stats = player_combat_stats_for_values(1, 1, &world_stats, &equipped);

    assert_eq!(stats.main_attack_time_ms, 2000);
    assert!(stats.main_min_damage > 2.0);
    assert!(stats.main_max_damage > 4.0);
    assert_eq!(
        calculate_player_main_hand_melee_damage(&stats, 1, 0, 1),
        stats.main_min_damage.round() as u32
    );
    assert_eq!(
        calculate_player_main_hand_melee_damage(&stats, 1, 0, 10_000),
        stats.main_max_damage.round() as u32
    );
}

#[test]
fn player_swing_timer_uses_equipped_main_hand_delay() {
    let mut weapon = test_item_template(25, ITEM_CLASS_WEAPON, 13, 2.0, 4.0, 0);
    weapon.delay = 2800;
    let equipped = [equipped_template(EQUIPMENT_SLOT_MAINHAND, weapon)];
    let world_stats = PlayerWorldStats {
        base_health: 20,
        base_mana: 0,
        stats: [23, 20, 22, 20, 20],
        next_level_xp: 400,
    };
    let stats = player_combat_stats_for_values(1, 1, &world_stats, &equipped);
    let now = Instant::now();

    assert_eq!(stats.main_attack_time_ms, 2800);
    assert_eq!(
        player_main_hand_next_swing_at(now, &stats),
        now + Duration::from_millis(2800)
    );
}

#[test]
fn ranged_weapon_damage_adds_compatible_ammo_dps_for_weapon_speed() {
    let mut bow = test_item_template(2504, ITEM_CLASS_WEAPON, 15, 5.0, 7.0, 0);
    bow.subclass = ITEM_SUBCLASS_WEAPON_BOW;
    bow.delay = 2000;
    let mut arrow = test_item_template(2512, ITEM_CLASS_PROJECTILE, INVTYPE_AMMO, 1.0, 1.5, 0);
    arrow.subclass = ITEM_SUBCLASS_ARROW;
    let equipped = [equipped_template(EQUIPMENT_SLOT_RANGED, bow)];
    let world_stats = PlayerWorldStats {
        base_health: 46,
        base_mana: 80,
        stats: [21, 24, 22, 20, 20],
        next_level_xp: 400,
    };

    let without_ammo =
        player_combat_stats_for_values_with_ammo(3, 1, &world_stats, &equipped, None);
    let with_ammo =
        player_combat_stats_for_values_with_ammo(3, 1, &world_stats, &equipped, Some(&arrow));

    assert!((with_ammo.ranged_min_damage - without_ammo.ranged_min_damage - 2.0).abs() < 0.001);
    assert!((with_ammo.ranged_max_damage - without_ammo.ranged_max_damage - 3.0).abs() < 0.001);
    assert!(ranged_weapon_accepts_ammo(&equipped[0].template, &arrow));
}

#[test]
fn bow_rejects_bullet_ammo_for_damage_bonus() {
    let mut bow = test_item_template(2504, ITEM_CLASS_WEAPON, 15, 5.0, 7.0, 0);
    bow.subclass = ITEM_SUBCLASS_WEAPON_BOW;
    let mut bullet = test_item_template(2516, ITEM_CLASS_PROJECTILE, INVTYPE_AMMO, 1.0, 1.0, 0);
    bullet.subclass = ITEM_SUBCLASS_BULLET;

    assert!(!ranged_weapon_accepts_ammo(&bow, &bullet));
}

#[test]
fn player_swing_timer_defaults_to_base_attack_time_without_weapon() {
    let world_stats = PlayerWorldStats {
        base_health: 20,
        base_mana: 0,
        stats: [23, 20, 22, 20, 20],
        next_level_xp: 400,
    };
    let stats = player_combat_stats_for_values(1, 1, &world_stats, &[]);
    let now = Instant::now();

    assert_eq!(stats.main_attack_time_ms, BASE_ATTACK_TIME_MS);
    assert_eq!(
        player_main_hand_next_swing_at(now, &stats),
        now + Duration::from_millis(BASE_ATTACK_TIME_MS as u64)
    );
}

#[test]
fn player_main_hand_outcome_uses_db_creature_template_armor() {
    let weapon = test_item_template(25, ITEM_CLASS_WEAPON, 13, 20.0, 20.0, 0);
    let equipped = [equipped_template(EQUIPMENT_SLOT_MAINHAND, weapon)];
    let world_stats = PlayerWorldStats {
        base_health: 20,
        base_mana: 0,
        stats: [23, 20, 22, 20, 20],
        next_level_xp: 400,
    };
    let stats = player_combat_stats_for_values(1, 1, &world_stats, &equipped);
    let mut spawn = test_creature_spawn(6);
    spawn.template.min_level = 1;
    spawn.template.max_level = 1;
    spawn.template.armor = 100;
    let creature = DbCreatureRuntime::new(spawn);

    let outcome = calculate_player_main_hand_melee_outcome_against_db_creature(
        &stats, 1, 5, &creature, 1, 10_000,
    );

    assert_eq!(outcome.outcome, MeleeHitOutcome::Normal);
    assert_eq!(
        outcome.total_damage,
        armor_reduced_damage(1, 100, stats.main_min_damage)
    );
}

#[test]
fn db_creature_swing_timer_uses_template_melee_base_attack_time() {
    let mut spawn = test_creature_spawn(6);
    spawn.template.melee_base_attack_time = 1450;
    let creature = DbCreatureRuntime::new(spawn);

    assert_eq!(creature.base_attack_duration(), Duration::from_millis(1450));
}

#[test]
fn db_creature_swing_timer_applies_temporary_melee_haste_slow() {
    let mut spawn = test_creature_spawn(6);
    spawn.template.melee_base_attack_time = 2000;
    let mut creature = DbCreatureRuntime::new(spawn);
    creature.active_auras.push(ActiveAura {
        spell_id: 6136,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 1,
        interrupt_flags: 0,
        visible: true,
        positive: false,
        duration_millis: Some(8_000),
        expires_at: Some(Instant::now() + Duration::from_secs(8)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::MeleeAttackTimePercent { percent: -25 }],
        proc_triggers: Vec::new(),
    });

    assert_eq!(creature.base_attack_duration(), Duration::from_millis(2500));
}

#[test]
fn db_creature_swing_timer_clamps_zero_template_time() {
    let mut spawn = test_creature_spawn(6);
    spawn.template.melee_base_attack_time = 0;
    let creature = DbCreatureRuntime::new(spawn);

    assert_eq!(creature.base_attack_duration(), Duration::from_millis(1));
}

#[test]
fn db_creature_mmap_path_filter_uses_cmangos_inhabit_type() {
    let mut creature = DbCreatureRuntime::new(test_creature_spawn(6));

    creature.spawn.template.inhabit_type = 1;
    assert_eq!(
        db_creature_mmap_path_filter(&creature).include_flags,
        NativeMmapPathFilter::NAV_GROUND
    );

    creature.spawn.template.inhabit_type = 2;
    assert_eq!(
        db_creature_mmap_path_filter(&creature).include_flags,
        NativeMmapPathFilter::NAV_WATER | NativeMmapPathFilter::NAV_MAGMA_SLIME
    );

    creature.spawn.template.inhabit_type = 3;
    assert_eq!(
        db_creature_mmap_path_filter(&creature).include_flags,
        NativeMmapPathFilter::NAV_GROUND
            | NativeMmapPathFilter::NAV_WATER
            | NativeMmapPathFilter::NAV_MAGMA_SLIME
    );

    creature.spawn.template.inhabit_type = 0;
    assert_eq!(
        db_creature_mmap_path_filter(&creature).include_flags,
        NativeMmapPathFilter::NAV_GROUND
            | NativeMmapPathFilter::NAV_WATER
            | NativeMmapPathFilter::NAV_MAGMA_SLIME
    );
}

#[test]
fn creature_melee_outcome_uses_player_armor_from_defense_input() {
    let mut spawn = test_creature_spawn(6);
    spawn.template.min_level = 1;
    spawn.template.max_level = 1;
    spawn.template.min_melee_dmg = 20.0;
    spawn.template.max_melee_dmg = 20.0;
    let creature = DbCreatureRuntime::new(spawn);
    let defense = PlayerMeleeDefenseInput {
        level: 1,
        defense_skill: 5,
        armor: 100,
        block_value: 0,
        dodge_percent: 0.0,
        parry_percent: 0.0,
        block_percent: 0.0,
    };

    let outcome = calculate_melee_damage(
        creature_melee_input_against_player(&creature, defense),
        1,
        10_000,
    );

    assert_eq!(outcome.outcome, MeleeHitOutcome::Normal);
    assert_eq!(outcome.total_damage, armor_reduced_damage(1, 100, 20.0));
}

#[test]
fn creature_block_outcome_uses_supplied_player_shield_block_value() {
    let mut spawn = test_creature_spawn(6);
    spawn.template.min_level = 1;
    spawn.template.max_level = 1;
    spawn.template.min_melee_dmg = 20.0;
    spawn.template.max_melee_dmg = 20.0;
    let creature = DbCreatureRuntime::new(spawn);
    let defense = PlayerMeleeDefenseInput {
        level: 1,
        defense_skill: 5,
        armor: 0,
        block_value: 7,
        dodge_percent: 0.0,
        parry_percent: 0.0,
        block_percent: 100.0,
    };

    let outcome = calculate_melee_damage(
        creature_melee_input_against_player(&creature, defense),
        1,
        501,
    );

    assert_eq!(outcome.outcome, MeleeHitOutcome::Block);
    assert_eq!(outcome.blocked, 7);
    assert_eq!(outcome.total_damage, 13);
}

#[test]
fn equipped_shield_block_value_reads_live_item_template_block_stat() {
    let mut shield = test_item_template(2362, ITEM_CLASS_ARMOR, INVTYPE_SHIELD, 0.0, 0.0, 0);
    shield.block = 11;
    let equipped_templates = [equipped_template(EQUIPMENT_SLOT_OFFHAND, shield)];

    assert_eq!(equipped_shield_block_value(&equipped_templates), 11);
}

#[test]
fn player_combat_stats_armor_increases_with_equipped_armor() {
    const EQUIPMENT_SLOT_CHEST: u8 = 4;
    let world_stats = PlayerWorldStats {
        base_health: 20,
        base_mana: 0,
        stats: [23, 20, 22, 20, 20],
        next_level_xp: 400,
    };
    let no_armor = player_combat_stats_for_values(1, 1, &world_stats, &[]);
    let chest = test_item_template(38, ITEM_CLASS_ARMOR, 5, 0.0, 0.0, 12);
    let equipped = [equipped_template(EQUIPMENT_SLOT_CHEST, chest)];
    let with_armor = player_combat_stats_for_values(1, 1, &world_stats, &equipped);

    assert_eq!(no_armor.armor, 40);
    assert_eq!(with_armor.armor, 52);
    assert_eq!(with_armor.resistances[0], with_armor.armor);
}

#[test]
fn player_combat_stats_uses_equipped_shield_block_value() {
    let world_stats = PlayerWorldStats {
        base_health: 20,
        base_mana: 0,
        stats: [23, 20, 22, 20, 20],
        next_level_xp: 400,
    };
    let mut shield = test_item_template(2362, ITEM_CLASS_ARMOR, INVTYPE_SHIELD, 0.0, 0.0, 3);
    shield.block = 11;
    let equipped = [equipped_template(EQUIPMENT_SLOT_OFFHAND, shield)];
    let with_shield = player_combat_stats_for_values(1, 1, &world_stats, &equipped);
    let no_shield = player_combat_stats_for_values(1, 1, &world_stats, &[]);

    assert_eq!(with_shield.shield_block_value, 11);
    assert_eq!(no_shield.shield_block_value, 0);
}

#[test]
fn player_combat_stats_shield_block_value_includes_strength_component() {
    let world_stats = PlayerWorldStats {
        base_health: 20,
        base_mana: 0,
        stats: [40, 20, 22, 20, 20],
        next_level_xp: 400,
    };
    let mut shield = test_item_template(2362, ITEM_CLASS_ARMOR, INVTYPE_SHIELD, 0.0, 0.0, 3);
    shield.block = 11;
    let equipped = [equipped_template(EQUIPMENT_SLOT_OFFHAND, shield)];
    let stats = player_combat_stats_for_values(1, 1, &world_stats, &equipped);

    assert_eq!(stats.shield_block_value, 12);
}

#[test]
fn player_combat_stats_update_body_refreshes_weapon_damage_fields() {
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let weapon = test_item_template(25, ITEM_CLASS_WEAPON, 13, 2.0, 4.0, 0);
    let equipped = [equipped_template(EQUIPMENT_SLOT_MAINHAND, weapon)];
    let world_stats = PlayerWorldStats {
        base_health: 20,
        base_mana: 0,
        stats: [23, 20, 22, 20, 20],
        next_level_xp: 400,
    };
    let stats = player_combat_stats_for_values(1, 1, &world_stats, &equipped);

    let body = build_player_combat_stats_update_body(7, &stats).unwrap();
    assert_eq!(u32::from_le_bytes(body[0..4].try_into().unwrap()), 1);
    let (values, trailing) = decode_values_update_block(&body[5..], player);

    assert!(trailing.is_empty());
    assert_eq!(
        values[UNIT_FIELD_BASEATTACKTIME],
        Some(stats.main_attack_time_ms)
    );
    assert_eq!(
        values[UNIT_FIELD_MINDAMAGE],
        Some(stats.main_min_damage.to_bits())
    );
    assert_eq!(
        values[UNIT_FIELD_MAXDAMAGE],
        Some(stats.main_max_damage.to_bits())
    );
    assert_eq!(
        values[UNIT_FIELD_ATTACK_POWER],
        Some(stats.melee_attack_power)
    );
    assert_eq!(
        values[PLAYER_CRIT_PERCENTAGE],
        Some(stats.crit_percent.to_bits())
    );
}

#[test]
fn combat_unit_flag_updates_include_cmangos_in_combat_bit() {
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let body = build_unit_flags_update_body(player, player_unit_flags(true)).unwrap();
    let packed_guid_mask = body[6];
    let values_start = 4 + 1 + 1 + 1 + packed_guid_mask.count_ones() as usize;
    let values = decode_update_values(&body[values_start..]);
    assert_eq!(
        values[UNIT_FIELD_FLAGS],
        Some(UNIT_FLAG_PLAYER_CONTROLLED | UNIT_FLAG_IN_COMBAT)
    );

    let body = build_unit_flags_update_body(player, player_unit_flags(false)).unwrap();
    let packed_guid_mask = body[6];
    let values_start = 4 + 1 + 1 + 1 + packed_guid_mask.count_ones() as usize;
    let values = decode_update_values(&body[values_start..]);
    assert_eq!(values[UNIT_FIELD_FLAGS], Some(UNIT_FLAG_PLAYER_CONTROLLED));
}

#[test]
fn chase_monster_move_can_face_target_like_cmangos_spline() {
    let creature = ObjectGuid::new(HighGuid::Unit, 0, 45);
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let start = WorldPosition::new(0, 1.0, 2.0, 3.0, 0.0);
    let destination = WorldPosition::new(0, 4.0, 5.0, 6.0, 0.0);
    let body = build_monster_move_facing_target_body(creature, start, destination, 9, 100, player)
        .unwrap();

    let mut cursor = PackedGuid::packed_size(creature) + 12 + 4;
    assert_eq!(body[cursor], MONSTER_MOVE_TYPE_FACING_TARGET);
    cursor += 1;
    assert_eq!(
        u64::from_le_bytes(body[cursor..cursor + 8].try_into().unwrap()),
        player.raw()
    );
    cursor += 8;
    assert_eq!(
        u32::from_le_bytes(body[cursor..cursor + 4].try_into().unwrap()),
        MONSTER_MOVE_SPLINE_FLAG_RUNMODE
    );
}

#[test]
fn monster_move_path_serializes_multiple_points() {
    let creature = ObjectGuid::new(HighGuid::Unit, 0, 45);
    let start = WorldPosition::new(0, 1.0, 2.0, 3.0, 0.0);
    let path = vec![
        WorldPosition::new(0, 4.0, 5.0, 6.0, 0.0),
        WorldPosition::new(0, 7.0, 8.0, 9.0, 0.0),
    ];
    let body = build_monster_move_walk_path_body(creature, start, &path, 9, 100).unwrap();

    let mut cursor = PackedGuid::packed_size(creature) + 12 + 4;
    assert_eq!(body[cursor], MONSTER_MOVE_TYPE_NORMAL);
    cursor += 1;
    assert_eq!(
        u32::from_le_bytes(body[cursor..cursor + 4].try_into().unwrap()),
        0
    );
    cursor += 4;
    assert_eq!(
        u32::from_le_bytes(body[cursor..cursor + 4].try_into().unwrap()),
        100
    );
    cursor += 4;
    assert_eq!(
        u32::from_le_bytes(body[cursor..cursor + 4].try_into().unwrap()),
        2
    );
    cursor += 4;
    assert_eq!(
        f32::from_le_bytes(body[cursor..cursor + 4].try_into().unwrap()),
        7.0
    );
    cursor += 12;
    assert_eq!(
        u32::from_le_bytes(body[cursor..cursor + 4].try_into().unwrap()),
        pack_monster_move_xyz_offset(3.0, 3.0, 3.0)
    );
}

#[test]
fn monster_move_facing_target_path_serializes_run_mode() {
    let creature = ObjectGuid::new(HighGuid::Unit, 0, 45);
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let start = WorldPosition::new(0, 1.0, 2.0, 3.0, 0.0);
    let path = vec![
        WorldPosition::new(0, 4.0, 5.0, 6.0, 0.0),
        WorldPosition::new(0, 7.0, 8.0, 9.0, 0.0),
    ];
    let body =
        build_monster_move_facing_target_path_body(creature, start, &path, 9, 100, player).unwrap();

    let mut cursor = PackedGuid::packed_size(creature) + 12 + 4;
    assert_eq!(body[cursor], MONSTER_MOVE_TYPE_FACING_TARGET);
    cursor += 1 + 8;
    assert_eq!(
        u32::from_le_bytes(body[cursor..cursor + 4].try_into().unwrap()),
        MONSTER_MOVE_SPLINE_FLAG_RUNMODE
    );
}

#[test]
fn monster_move_path_skips_tiny_offsets_like_cmangos() {
    let creature = ObjectGuid::new(HighGuid::Unit, 0, 45);
    let start = WorldPosition::new(0, 1.0, 2.0, 3.0, 0.0);
    let path = vec![
        WorldPosition::new(0, 7.25, 8.0, 9.0, 0.0),
        WorldPosition::new(0, 7.0, 8.0, 9.0, 0.0),
    ];
    let body = build_monster_move_walk_path_body(creature, start, &path, 9, 100).unwrap();

    let mut cursor = PackedGuid::packed_size(creature) + 12 + 4;
    assert_eq!(body[cursor], MONSTER_MOVE_TYPE_NORMAL);
    cursor += 1 + 4 + 4;
    assert_eq!(
        u32::from_le_bytes(body[cursor..cursor + 4].try_into().unwrap()),
        1
    );
    assert_eq!(body.len(), cursor + 4 + 12);
}

#[test]
fn monster_move_stop_uses_cmangos_stop_shape() {
    let creature = ObjectGuid::new(HighGuid::Unit, 0, 45);
    let position = WorldPosition::new(0, 1.0, 2.0, 3.0, 0.0);
    let body = build_monster_move_stop_body(creature, position, 9).unwrap();

    let mut cursor = PackedGuid::packed_size(creature);
    assert_eq!(
        f32::from_le_bytes(body[cursor..cursor + 4].try_into().unwrap()),
        1.0
    );
    cursor += 12;
    assert_eq!(
        u32::from_le_bytes(body[cursor..cursor + 4].try_into().unwrap()),
        9
    );
    cursor += 4;
    assert_eq!(body[cursor], MONSTER_MOVE_TYPE_STOP);
    assert_eq!(body.len(), cursor + 1);
}

#[test]
fn heroic_strike_fixture_damage_builds_spell_damage_log() {
    let attacker = ObjectGuid::new(HighGuid::Player, 0, 7);
    let victim = ObjectGuid::new(HighGuid::Unit, 6, 45);
    let state = build_spell_non_melee_damage_log_body(SpellNonMeleeDamageLogPacket {
        attacker,
        target: victim,
        spell_id: WARRIOR_HEROIC_STRIKE_RANK_1,
        damage: HEROIC_STRIKE_FIXTURE_DAMAGE,
        school: 0,
        absorb: 0,
        resist: 0,
        periodic: false,
        blocked: 0,
        hit_info: 0,
    })
    .unwrap();
    let mut cursor = 0;
    assert_eq!(read_packed_guid(&state, &mut cursor).unwrap(), victim);
    assert_eq!(read_packed_guid(&state, &mut cursor).unwrap(), attacker);
    assert_eq!(
        read_u32(&state, &mut cursor).unwrap(),
        WARRIOR_HEROIC_STRIKE_RANK_1
    );
    assert_eq!(
        read_u32(&state, &mut cursor).unwrap(),
        HEROIC_STRIKE_FIXTURE_DAMAGE
    );
    assert_eq!(state[cursor], 0);
}

#[test]
fn raptor_strike_fixture_damage_marks_attacker_state_spell_id() {
    let attacker = ObjectGuid::new(HighGuid::Player, 0, 7);
    let victim = ObjectGuid::new(HighGuid::Unit, 6, 45);
    let state = build_attacker_state_update_body_with_spell_id(
        attacker,
        victim,
        RAPTOR_STRIKE_FIXTURE_DAMAGE,
        HUNTER_RAPTOR_STRIKE_RANK_1,
    )
    .unwrap();
    let mut cursor = 0;
    assert_eq!(read_u32(&state, &mut cursor).unwrap(), HITINFO_NORMALSWING2);
    cursor += PackedGuid::packed_size(attacker) + PackedGuid::packed_size(victim);
    assert_eq!(
        read_u32(&state, &mut cursor).unwrap(),
        RAPTOR_STRIKE_FIXTURE_DAMAGE
    );
    cursor += 1; // damage school count
    cursor += 4; // normal school
    cursor += 4; // float damage
    cursor += 4; // integer damage
    cursor += 4; // absorb
    cursor += 4; // resist
    cursor += 4; // victim state
    cursor += 4; // unknown
    assert_eq!(
        read_u32(&state, &mut cursor).unwrap(),
        HUNTER_RAPTOR_STRIKE_RANK_1
    );
}

#[test]
fn db_creature_death_update_clears_cmangos_death_fields() {
    let guid = ObjectGuid::new(HighGuid::Unit, 6, 45);
    let body = build_db_creature_death_update_body(guid, UNIT_DYNFLAG_LOOTABLE, 0x20).unwrap();
    let packed_guid_mask = body[6];
    let values_start = 4 + 1 + 1 + 1 + packed_guid_mask.count_ones() as usize;
    let values = decode_update_values(&body[values_start..]);

    assert_eq!(values[UNIT_FIELD_TARGET], Some(0));
    assert_eq!(values[UNIT_FIELD_TARGET + 1], Some(0));
    assert_eq!(values[UNIT_FIELD_HEALTH], Some(0));
    assert_eq!(values[UNIT_FIELD_FLAGS], Some(0x20));
    assert_eq!(values[UNIT_DYNAMIC_FLAGS], Some(UNIT_DYNFLAG_LOOTABLE));
    assert_eq!(values[UNIT_NPC_FLAGS], Some(0));
    assert_eq!(values[UNIT_FIELD_AURA + MAX_POSITIVE_AURA_SLOTS], Some(0));
    assert_eq!(
        values[UNIT_FIELD_AURAFLAGS + (MAX_POSITIVE_AURA_SLOTS / 8)],
        Some(0)
    );
    assert_eq!(
        values[UNIT_FIELD_AURALEVELS + (MAX_POSITIVE_AURA_SLOTS / 4)],
        Some(0)
    );
}

#[test]
fn player_rage_update_sets_warrior_power_field() {
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let body = build_player_rage_update_body(player, HEROIC_STRIKE_RAGE_COST).unwrap();
    let packed_guid_mask = body[6];
    let values_start = 4 + 1 + 1 + 1 + packed_guid_mask.count_ones() as usize;
    let values = decode_update_values(&body[values_start..]);

    assert_eq!(&body[0..4], &1u32.to_le_bytes());
    assert_eq!(body[4], 0);
    assert_eq!(body[5], UPDATE_TYPE_VALUES);
    assert_eq!(values[UNIT_FIELD_POWER2], Some(HEROIC_STRIKE_RAGE_COST));
}

#[test]
fn player_mana_update_sets_mana_power_field() {
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let body = build_player_mana_update_body(player, 42).unwrap();
    let packed_guid_mask = body[6];
    let values_start = 4 + 1 + 1 + 1 + packed_guid_mask.count_ones() as usize;
    let values = decode_update_values(&body[values_start..]);

    assert_eq!(&body[0..4], &1u32.to_le_bytes());
    assert_eq!(body[4], 0);
    assert_eq!(body[5], UPDATE_TYPE_VALUES);
    assert_eq!(values[UNIT_FIELD_POWER1], Some(42));
}

#[test]
fn player_energy_update_sets_energy_power_field() {
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let body = build_player_energy_update_body(player, 250).unwrap();
    let packed_guid_mask = body[6];
    let values_start = 4 + 1 + 1 + 1 + packed_guid_mask.count_ones() as usize;
    let values = decode_update_values(&body[values_start..]);

    assert_eq!(&body[0..4], &1u32.to_le_bytes());
    assert_eq!(body[4], 0);
    assert_eq!(body[5], UPDATE_TYPE_VALUES);
    assert_eq!(values[UNIT_FIELD_POWER4], Some(POWER_ENERGY_DEFAULT));
}

#[test]
fn player_health_update_sets_health_field() {
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let body = build_player_health_update_body(player, 37).unwrap();
    let packed_guid_mask = body[6];
    let values_start = 4 + 1 + 1 + 1 + packed_guid_mask.count_ones() as usize;
    let values = decode_update_values(&body[values_start..]);

    assert_eq!(&body[0..4], &1u32.to_le_bytes());
    assert_eq!(body[4], 0);
    assert_eq!(body[5], UPDATE_TYPE_VALUES);
    assert_eq!(values[UNIT_FIELD_HEALTH], Some(37));
}

#[test]
fn creature_xp_reward_matches_cmangos_base_gain_for_starter_levels() {
    let mut wolf = test_creature_template(6);
    wolf.min_level = 1;
    wolf.rank = 0;
    wolf.creature_type = 1;
    wolf.experience_multiplier = 1.0;

    assert_eq!(creature_xp_reward(1, &wolf), 50);

    wolf.min_level = 2;
    assert_eq!(creature_xp_reward(1, &wolf), 52);

    wolf.experience_multiplier = 2.0;
    assert_eq!(creature_xp_reward(1, &wolf), 105);

    wolf.creature_type = CREATURE_TYPE_CRITTER;
    assert_eq!(creature_xp_reward(1, &wolf), 0);
}
