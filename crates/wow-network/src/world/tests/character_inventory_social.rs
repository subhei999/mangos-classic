#[test]
fn normalizes_character_names_like_cmangos_create_path() {
    assert_eq!(normalize_character_name("rUSTY").unwrap(), "Rusty");
    assert_eq!(normalize_character_name("").unwrap_err(), CHAR_NAME_NO_NAME);
    assert_eq!(
        normalize_character_name("A").unwrap_err(),
        CHAR_NAME_TOO_SHORT
    );
    assert_eq!(
        normalize_character_name("Thirteenchars").unwrap_err(),
        CHAR_NAME_TOO_LONG
    );
    assert_eq!(
        normalize_character_name("Bad1").unwrap_err(),
        CHAR_NAME_INVALID_CHARACTER
    );
}

#[test]
fn validates_classic_race_class_pairs() {
    assert!(is_valid_race_class(1, 1));
    assert!(is_valid_race_class(7, 8));
    assert!(!is_valid_race_class(1, 7));
    assert!(!is_valid_race_class(9, 1));
}

#[test]
fn serializes_character_enum_entry() {
    let body = build_char_enum_body(&[CharacterEnumEntry {
        guid: 7,
        name: "Rustone".to_string(),
        race: 1,
        class: 1,
        gender: 0,
        player_bytes: 0x0403_0201,
        player_bytes2: 0x0000_0005,
        level: 1,
        xp: 0,
        zone: 12,
        map: 0,
        position_x: -8949.95,
        position_y: -132.493,
        position_z: 83.5312,
        orientation: 0.0,
        guildid: Some(0),
        player_flags: PLAYER_FLAGS_HIDE_HELM,
        at_login: AT_LOGIN_FIRST,
        money: 0,
        cinematic: 0,
        ammo_id: 0,
        health: 20,
        power1: 0,
        power2: 0,
        power3: 0,
        power4: 0,
        power5: 0,
        watched_faction: u32::MAX,
        explored_zones: None,
        pet_entry: None,
        pet_modelid: None,
        pet_level: None,
        equipment_cache: None,
    }])
    .unwrap();

    assert_eq!(body[0], 1);
    assert_eq!(&body[1..9], &7u64.to_le_bytes());
    assert_eq!(&body[9..17], b"Rustone\0");
    assert_eq!(body[17], 1);
    assert_eq!(body[18], 1);
    assert_eq!(body[19], 0);
    assert_eq!(body[20], 1);
    assert_eq!(body[21], 2);
    assert_eq!(body[22], 3);
    assert_eq!(body[23], 4);
    assert_eq!(body[24], 5);
    assert_eq!(
        body.len(),
        1 + 8 + 8 + 1 + 1 + 1 + 5 + 1 + 4 + 4 + 12 + 4 + 4 + 1 + 12 + 100
    );
}

#[test]
fn login_verify_world_packet_shape() {
    let character = CharacterEnumEntry {
        guid: 7,
        name: "Rustone".to_string(),
        race: 1,
        class: 1,
        gender: 0,
        player_bytes: 0,
        player_bytes2: 0,
        level: 1,
        xp: 0,
        zone: 12,
        map: 0,
        position_x: -8949.95,
        position_y: -132.493,
        position_z: 83.5312,
        orientation: 1.25,
        guildid: None,
        player_flags: 0,
        at_login: 0,
        money: 0,
        cinematic: 0,
        ammo_id: 0,
        health: 20,
        power1: 0,
        power2: 0,
        power3: 0,
        power4: 0,
        power5: 0,
        watched_faction: u32::MAX,
        explored_zones: None,
        pet_entry: None,
        pet_modelid: None,
        pet_level: None,
        equipment_cache: None,
    };

    let mut body = Vec::new();
    body.extend_from_slice(&character.map.to_le_bytes());
    body.extend_from_slice(&character.position_x.to_le_bytes());
    body.extend_from_slice(&character.position_y.to_le_bytes());
    body.extend_from_slice(&character.position_z.to_le_bytes());
    body.extend_from_slice(&character.orientation.to_le_bytes());

    assert_eq!(body.len(), 20);
    assert_eq!(&body[0..4], &0u32.to_le_bytes());
    assert_eq!(&body[16..20], &1.25f32.to_le_bytes());
}

#[test]
fn set_rest_start_packet_matches_cmangos_zero_payload() {
    assert_eq!(build_set_rest_start_body(), 0u32.to_le_bytes());
}

#[test]
fn empty_initial_spells_shape() {
    let body = build_initial_spells_body(&[]);
    assert_eq!(body, [0, 0, 0, 0, 0]);
}

#[test]
fn initial_spells_include_active_enabled_spells() {
    let body = build_initial_spells_body(&[
        CharacterSpell {
            spell: 78,
            active: 1,
            disabled: 0,
        },
        CharacterSpell {
            spell: 81,
            active: 0,
            disabled: 0,
        },
        CharacterSpell {
            spell: 107,
            active: 1,
            disabled: 1,
        },
    ]);

    assert_eq!(body, [0, 1, 0, 78, 0, 0, 0, 0, 0]);
}

#[test]
fn initial_spells_include_active_spell_cooldowns() {
    let mut cooldowns = HashMap::new();
    cooldowns.insert(8690, Instant::now() + Duration::from_secs(60));
    let mut cooldown_categories = HashMap::new();
    cooldown_categories.insert(8690, 89);
    let mut cooldown_items = HashMap::new();
    cooldown_items.insert(8690, 6948);
    let mut category_cooldowns = HashMap::new();
    category_cooldowns.insert(89, Instant::now() + Duration::from_secs(60));

    let body = build_initial_spells_body_with_cooldowns(
        &[],
        &cooldowns,
        &cooldown_categories,
        &cooldown_items,
        &category_cooldowns,
    );

    assert_eq!(&body[0..3], &[0, 0, 0]);
    assert_eq!(u16::from_le_bytes([body[3], body[4]]), 1);
    assert_eq!(u16::from_le_bytes([body[5], body[6]]), 8690);
    assert_eq!(u16::from_le_bytes([body[7], body[8]]), 6948);
    assert_eq!(u16::from_le_bytes([body[9], body[10]]), 89);
    let cooldown_ms = u32::from_le_bytes([body[11], body[12], body[13], body[14]]);
    assert!((59_000..=60_000).contains(&cooldown_ms));
    let category_cooldown_ms = u32::from_le_bytes([body[15], body[16], body[17], body[18]]);
    assert!((59_000..=60_000).contains(&category_cooldown_ms));
}

#[test]
fn initial_spells_include_category_only_spell_cooldowns() {
    let now = Instant::now();
    let mut cooldowns = HashMap::new();
    cooldowns.insert(439, now);
    let mut cooldown_categories = HashMap::new();
    cooldown_categories.insert(439, 4);
    let mut cooldown_items = HashMap::new();
    cooldown_items.insert(439, 929);
    let mut category_cooldowns = HashMap::new();
    category_cooldowns.insert(4, now + Duration::from_secs(120));

    let body = build_initial_spells_body_with_cooldowns(
        &[],
        &cooldowns,
        &cooldown_categories,
        &cooldown_items,
        &category_cooldowns,
    );

    assert_eq!(&body[0..3], &[0, 0, 0]);
    assert_eq!(u16::from_le_bytes([body[3], body[4]]), 1);
    assert_eq!(u16::from_le_bytes([body[5], body[6]]), 439);
    assert_eq!(u16::from_le_bytes([body[7], body[8]]), 929);
    assert_eq!(u16::from_le_bytes([body[9], body[10]]), 4);
    assert_eq!(u32::from_le_bytes([body[11], body[12], body[13], body[14]]), 0);
    let category_cooldown_ms = u32::from_le_bytes([body[15], body[16], body[17], body[18]]);
    assert!((119_000..=120_000).contains(&category_cooldown_ms));
}

#[test]
fn set_proficiency_packet_matches_cmangos_class_and_mask_shape() {
    let body = build_set_proficiency_body(ITEM_CLASS_WEAPON, (1 << 7) | (1 << 15));

    assert_eq!(body.len(), 5);
    assert_eq!(body[0], ITEM_CLASS_WEAPON as u8);
    assert_eq!(&body[1..5], &((1u32 << 7) | (1u32 << 15)).to_le_bytes());
}

#[test]
fn proficiency_masks_use_spell_template_class_and_subclass_mask() {
    let mut sword_proficiency = test_spell_template(201);
    sword_proficiency.effect1 = 60;
    sword_proficiency.equipped_item_class = ITEM_CLASS_WEAPON as i32;
    sword_proficiency.equipped_item_subclass_mask = 1 << 7;
    let mut shield_proficiency = test_spell_template(9116);
    shield_proficiency.effect1 = 60;
    shield_proficiency.equipped_item_class = ITEM_CLASS_ARMOR as i32;
    shield_proficiency.equipped_item_subclass_mask = 1 << 6;
    let mut ignored_non_proficiency = test_spell_template(78);
    ignored_non_proficiency.equipped_item_class = ITEM_CLASS_WEAPON as i32;
    ignored_non_proficiency.equipped_item_subclass_mask = 1 << 15;

    let mut weapon_mask = 0;
    let mut armor_mask = 0;
    if spell_template_has_proficiency_effect(&sword_proficiency) {
        add_template_proficiency_masks(&sword_proficiency, &mut weapon_mask, &mut armor_mask);
    }
    if spell_template_has_proficiency_effect(&shield_proficiency) {
        add_template_proficiency_masks(&shield_proficiency, &mut weapon_mask, &mut armor_mask);
    }
    if spell_template_has_proficiency_effect(&ignored_non_proficiency) {
        add_template_proficiency_masks(&ignored_non_proficiency, &mut weapon_mask, &mut armor_mask);
    }

    assert_eq!(weapon_mask, 1 << 7);
    assert_eq!(armor_mask, 1 << 6);
}

#[test]
fn action_buttons_pack_cmangos_action_type_layout() {
    let body = build_action_buttons_body(&[
        CharacterAction {
            button: 0,
            action: 6603,
            action_type: 0,
        },
        CharacterAction {
            button: 11,
            action: 117,
            action_type: 128,
        },
    ]);

    assert_eq!(body.len(), MAX_ACTION_BUTTONS * 4);
    assert_eq!(&body[0..4], &6603u32.to_le_bytes());
    assert_eq!(&body[44..48], &(0x8000_0075u32).to_le_bytes());
}

#[test]
fn set_action_button_reads_cmangos_packed_layout() {
    let request =
        wow_proto::SetActionButtonRequest::read(&mut &[11, 0x75, 0x00, 0x00, 0x80][..]).unwrap();

    assert_eq!(request.button, 11);
    assert_eq!(request.action(), 117);
    assert_eq!(request.action_type(), ACTION_BUTTON_TYPE_ITEM);
    assert!(!request.removes_binding());
}

#[test]
fn set_action_button_reads_remove_binding_packet() {
    let request = wow_proto::SetActionButtonRequest::read(&mut &[3, 0, 0, 0, 0][..]).unwrap();

    assert_eq!(request.button, 3);
    assert!(request.removes_binding());
    assert_eq!(request.action(), 0);
    assert_eq!(request.action_type(), 0);
}

#[test]
fn set_action_button_rejects_truncated_payload() {
    let err = wow_proto::SetActionButtonRequest::read(&mut &[3, 0, 0, 0][..]).unwrap_err();
    assert!(err
        .to_string()
        .contains("CMSG_SET_ACTION_BUTTON payload must be 5 bytes"));
}

#[test]
fn supported_action_button_types_match_cmangos_family() {
    for action_type in [
        ACTION_BUTTON_TYPE_SPELL,
        ACTION_BUTTON_TYPE_CLICK,
        ACTION_BUTTON_TYPE_MACRO,
        ACTION_BUTTON_TYPE_CMACRO,
        ACTION_BUTTON_TYPE_ITEM,
    ] {
        assert!(is_supported_action_button_type(action_type));
    }

    assert!(!is_supported_action_button_type(0x20));
}

#[test]
fn warrior_unit_bytes_set_battle_stance_for_stance_action_bar() {
    let character = CharacterEnumEntry {
        guid: 7,
        name: "Ada".to_string(),
        race: 1,
        class: 1,
        gender: 0,
        player_bytes: 0,
        player_bytes2: 0,
        level: 1,
        xp: 0,
        zone: 12,
        map: 0,
        position_x: -8949.95,
        position_y: -132.493,
        position_z: 83.5312,
        orientation: 0.0,
        guildid: None,
        player_flags: 0,
        at_login: 0,
        money: 0,
        cinematic: 0,
        ammo_id: 0,
        health: 20,
        power1: 0,
        power2: 0,
        power3: 0,
        power4: 0,
        power5: 0,
        watched_faction: u32::MAX,
        explored_zones: None,
        pet_entry: None,
        pet_modelid: None,
        pet_level: None,
        equipment_cache: None,
    };

    assert_eq!(unit_bytes_1(&character), 0x0011_EE00);
}

#[test]
fn self_spawn_update_includes_cmangos_player_vitals_and_defaults() {
    let guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let mut character = test_character(1, 1);
    character.health = 37;
    character.explored_zones = Some("1 0 4 4294967295".to_string());

    let mut body = Vec::new();
    let world_stats = PlayerWorldStats {
        base_health: 20,
        base_mana: 0,
        stats: [23, 20, 22, 20, 21],
        next_level_xp: 400,
    };

    let skills = vec![
        test_skill(95, 1, 5),     // Defense
        test_skill(98, 300, 300), // Common
        test_skill(162, 1, 5),    // Unarmed
    ];
    let equipped = vec![
        equipped_template(
            EQUIPMENT_SLOT_MAINHAND,
            test_item_template(25, ITEM_CLASS_WEAPON, 13, 2.0, 4.0, 0),
        ),
        equipped_template(
            EQUIPMENT_SLOT_OFFHAND,
            test_item_template(2362, ITEM_CLASS_ARMOR, INVTYPE_SHIELD, 0.0, 0.0, 11),
        ),
    ];

    write_minimal_player_update_values(
        &mut body,
        guid,
        &character,
        &[],
        &world_stats,
        &world_stats,
        &skills,
        &std::collections::HashMap::new(),
        &equipped,
        None,
        &[],
    )
    .unwrap();
    let values = decode_update_values(&body);
    let expected_melee_ap =
        class_melee_attack_power(character.class, character.level as u32, 23, 20);
    let expected_ranged_ap = class_ranged_attack_power(character.class, character.level as u32, 20);
    let expected_main_bonus = expected_melee_ap as f32 / 14.0 * 2.0;

    assert_eq!(values[UNIT_FIELD_HEALTH], Some(37));
    assert_eq!(values[UNIT_FIELD_MAXHEALTH], Some(60));
    assert_eq!(values[UNIT_FIELD_MAXPOWER2], Some(1000));
    assert_eq!(values[UNIT_FIELD_LEVEL], Some(1));
    assert_eq!(values[UNIT_FIELD_FLAGS], Some(UNIT_FLAG_PLAYER_CONTROLLED));
    assert_eq!(values[UNIT_FIELD_BASEATTACKTIME], Some(2000));
    assert_eq!(values[UNIT_FIELD_BASEATTACKTIME + 1], Some(2000));
    assert_eq!(values[UNIT_FIELD_STAT0], Some(23));
    assert_eq!(values[UNIT_FIELD_STAT0 + 1], Some(20));
    assert_eq!(values[UNIT_FIELD_STAT0 + 2], Some(22));
    assert_eq!(values[UNIT_FIELD_BASE_HEALTH], Some(20));
    assert_eq!(values[UNIT_FIELD_BASE_MANA], Some(0));
    assert_eq!(values[UNIT_FIELD_RESISTANCES], Some(51));
    assert_eq!(values[UNIT_FIELD_RESISTANCES + 1], Some(0));
    assert_eq!(values[UNIT_FIELD_AURASTATE], Some(0));
    assert_eq!(values[UNIT_FIELD_MOUNTDISPLAYID], Some(0));
    assert_eq!(
        values[UNIT_FIELD_MINDAMAGE],
        Some((2.0f32 + expected_main_bonus).to_bits())
    );
    assert_eq!(
        values[UNIT_FIELD_MAXDAMAGE],
        Some((4.0f32 + expected_main_bonus).to_bits())
    );
    assert_eq!(values[UNIT_FIELD_MINOFFHANDDAMAGE], Some(0.0f32.to_bits()));
    assert_eq!(values[UNIT_FIELD_MAXOFFHANDDAMAGE], Some(0.0f32.to_bits()));
    assert_eq!(values[UNIT_FIELD_ATTACK_POWER], Some(expected_melee_ap));
    assert_eq!(
        values[UNIT_FIELD_RANGED_ATTACK_POWER],
        Some(expected_ranged_ap)
    );
    assert_eq!(values[UNIT_FIELD_ATTACK_POWER_MODS], Some(0));
    assert_eq!(values[UNIT_FIELD_RANGED_ATTACK_POWER_MODS], Some(0));
    assert_eq!(values[UNIT_FIELD_MINRANGEDDAMAGE], Some(0.0f32.to_bits()));
    assert_eq!(values[UNIT_FIELD_MAXRANGEDDAMAGE], Some(0.0f32.to_bits()));
    assert_eq!(values[UNIT_FIELD_POWER_COST_MODIFIER], Some(0));
    assert_eq!(values[PLAYER_NEXT_LEVEL_XP], Some(400));
    assert_eq!(values[PLAYER_SKILL_INFO_1_1], Some(make_pair32(95, 0)));
    assert_eq!(values[PLAYER_SKILL_INFO_1_1 + 1], Some(make_pair32(1, 5)));
    assert_eq!(values[PLAYER_SKILL_INFO_1_1 + 2], Some(0));
    assert_eq!(values[PLAYER_SKILL_INFO_1_1 + 3], Some(make_pair32(98, 0)));
    assert_eq!(
        values[PLAYER_SKILL_INFO_1_1 + 4],
        Some(make_pair32(300, 300))
    );
    assert_eq!(values[PLAYER_SKILL_INFO_1_1 + 6], Some(make_pair32(162, 0)));
    assert_eq!(values[PLAYER_CHARACTER_POINTS1], Some(0));
    assert_eq!(values[PLAYER_CHARACTER_POINTS2], Some(2));
    assert_eq!(values[PLAYER_BLOCK_PERCENTAGE], Some(5.0f32.to_bits()));
    assert_eq!(
        values[PLAYER_DODGE_PERCENTAGE],
        Some(dodge_percent(character.class, character.level, 20).to_bits())
    );
    assert_eq!(values[PLAYER_PARRY_PERCENTAGE], Some(0.0f32.to_bits()));
    assert_eq!(
        values[PLAYER_CRIT_PERCENTAGE],
        Some(melee_crit_percent(character.class, character.level, 20).to_bits())
    );
    assert_eq!(values[PLAYER_EXPLORED_ZONES_1], Some(1));
    assert_eq!(values[PLAYER_EXPLORED_ZONES_1 + 1], Some(0));
    assert_eq!(values[PLAYER_EXPLORED_ZONES_1 + 2], Some(4));
    assert_eq!(values[PLAYER_EXPLORED_ZONES_1 + 3], Some(u32::MAX));
    assert_eq!(values[PLAYER_EXPLORED_ZONES_1 + 63], Some(0));
    assert_eq!(
        values[PLAYER_BYTES_2],
        Some((REST_STATE_NORMAL as u32) << 24 | character.player_bytes2)
    );
    assert_eq!(values[UNIT_FIELD_BYTES_2], Some(unit_bytes_2()));
    assert_eq!(values[PLAYER_FIELD_COINAGE], Some(12345));
    assert_eq!(values[PLAYER_FIELD_POSSTAT0], Some(0));
    assert_eq!(values[PLAYER_FIELD_NEGSTAT0], Some(0));
    assert_eq!(
        values[PLAYER_FIELD_RESISTANCEBUFFMODSPOSITIVE],
        Some(0.0f32.to_bits())
    );
    assert_eq!(values[PLAYER_FIELD_WATCHED_FACTION_INDEX], Some(u32::MAX));
    assert_eq!(
        values[PLAYER_FIELD_MOD_DAMAGE_DONE_PCT],
        Some(1.0f32.to_bits())
    );
    assert_eq!(values[PLAYER_AMMO_ID], Some(0));
    assert_eq!(values[PLAYER_SELF_RES_SPELL], Some(0));
    assert_eq!(values[PLAYER_FIELD_BYTES2], Some(0));
}

#[test]
fn class_power_defaults_match_cmangos_create_powers() {
    let guid = ObjectGuid::new(HighGuid::Player, 0, 7);

    let mut body = Vec::new();
    let mage_stats = PlayerWorldStats {
        base_health: 31,
        base_mana: 100,
        stats: [15, 23, 19, 26, 22],
        next_level_xp: 400,
    };
    write_minimal_player_update_values(
        &mut body,
        guid,
        &test_character(7, 8),
        &[],
        &mage_stats,
        &mage_stats,
        &[],
        &std::collections::HashMap::new(),
        &[],
        None,
        &[],
    )
    .unwrap();
    let values = decode_update_values(&body);
    assert_eq!(values[UNIT_FIELD_POWER1], Some(210));
    assert_eq!(values[UNIT_FIELD_MAXPOWER1], Some(210));
    assert_eq!(values[UNIT_FIELD_MAXPOWER2], Some(0));
    assert_eq!(values[UNIT_FIELD_MAXPOWER4], Some(0));

    let mut body = Vec::new();
    let rogue_stats = PlayerWorldStats {
        base_health: 25,
        base_mana: 0,
        stats: [21, 23, 21, 20, 20],
        next_level_xp: 400,
    };
    write_minimal_player_update_values(
        &mut body,
        guid,
        &test_character(1, 4),
        &[],
        &rogue_stats,
        &rogue_stats,
        &[],
        &std::collections::HashMap::new(),
        &[],
        None,
        &[],
    )
    .unwrap();
    let values = decode_update_values(&body);
    assert_eq!(values[UNIT_FIELD_POWER4], Some(POWER_ENERGY_DEFAULT));
    assert_eq!(values[UNIT_FIELD_MAXPOWER4], Some(POWER_ENERGY_DEFAULT));
    assert_eq!(values[UNIT_FIELD_MAXPOWER2], Some(0));
}

#[test]
fn login_player_create_values_include_visible_saved_buffs() {
    let guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let mut character = test_character(1, 1);
    character.health = 37;
    let world_stats = PlayerWorldStats {
        base_health: 20,
        base_mana: 0,
        stats: [23, 20, 22, 20, 21],
        next_level_xp: 400,
    };
    let aura = ActiveAura {
        spell_id: 6673,
        caster: guid,
        level: 3,
        interrupt_flags: 0,
        positive: true,
        visible: true,
        duration_millis: Some(120_000),
        expires_at: Some(Instant::now() + Duration::from_secs(90)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: Vec::new(),
        proc_triggers: Vec::new(),
    };
    let mut body = Vec::new();

    write_minimal_player_update_values(
        &mut body,
        guid,
        &character,
        &[],
        &world_stats,
        &world_stats,
        &[],
        &std::collections::HashMap::new(),
        &[],
        None,
        &[aura],
    )
    .unwrap();
    let values = decode_update_values(&body);

    assert_eq!(values[UNIT_FIELD_AURA], Some(6673));
    assert_eq!(values[UNIT_FIELD_AURAFLAGS], Some(POSITIVE_AURA_FLAGS));
    assert_eq!(values[UNIT_FIELD_AURALEVELS], Some(3));
}

#[test]
fn login_player_create_values_preserve_zero_health_corpse_state() {
    let guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let character = test_character(1, 1);
    let world_stats = PlayerWorldStats {
        base_health: 20,
        base_mana: 0,
        stats: [23, 20, 22, 20, 21],
        next_level_xp: 400,
    };
    let mut body = Vec::new();

    write_minimal_player_update_values(
        &mut body,
        guid,
        &character,
        &[],
        &world_stats,
        &world_stats,
        &[],
        &std::collections::HashMap::new(),
        &[],
        None,
        &[],
    )
    .unwrap();
    let values = decode_update_values(&body);

    assert_eq!(values[UNIT_FIELD_HEALTH], Some(0));
    assert_eq!(
        values[UNIT_FIELD_BYTES_1],
        Some(unit_bytes_1_for_class(character.class) | u32::from(PLAYER_STAND_STATE_DEAD))
    );
    assert_eq!(
        values[PLAYER_FIELD_BYTES],
        Some(PLAYER_FIELD_BYTE_RELEASE_TIMER)
    );
}

#[test]
fn initial_reputations_packet_matches_cmangos_empty_shape() {
    let body = build_initial_reputations_body(&[]);

    assert_eq!(body.len(), 4 + REPUTATION_LIST_SLOTS * 5);
    assert_eq!(&body[0..4], &(REPUTATION_LIST_SLOTS as u32).to_le_bytes());
    assert!(body[4..].iter().all(|byte| *byte == 0));
}

#[test]
fn initial_reputations_packet_maps_dbc_reputation_list_slots() {
    let body = build_initial_reputations_body(&[
        CharacterReputation {
            faction: 72,
            standing: 250,
            flags: 1,
        },
        CharacterReputation {
            faction: 47,
            standing: 500,
            flags: 1,
        },
        CharacterReputation {
            faction: 999_999,
            standing: 42,
            flags: 1,
        },
    ]);

    let stormwind_offset = 4 + 19 * 5;
    assert_eq!(body[stormwind_offset], 1);
    assert_eq!(
        &body[stormwind_offset + 1..stormwind_offset + 5],
        &250i32.to_le_bytes()
    );
    let ironforge_offset = 4 + 20 * 5;
    assert_eq!(body[ironforge_offset], 1);
    assert_eq!(
        &body[ironforge_offset + 1..ironforge_offset + 5],
        &500i32.to_le_bytes()
    );
    assert_eq!(
        body[4..]
            .chunks_exact(5)
            .enumerate()
            .filter(|(slot, chunk)| *slot != 19
                && *slot != 20
                && chunk.iter().any(|byte| *byte != 0))
            .count(),
        0
    );
}

#[test]
fn trigger_cinematic_packet_uses_vanilla_chrraces_sequence() {
    assert_eq!(cinematic_sequence_for_race(1), Some(81));
    assert_eq!(cinematic_sequence_for_race(2), Some(21));
    assert_eq!(cinematic_sequence_for_race(8), Some(121));
    assert_eq!(cinematic_sequence_for_race(9), None);

    let body = build_trigger_cinematic_body(cinematic_sequence_for_race(1).unwrap());
    assert_eq!(body, 81u32.to_le_bytes());
}

#[test]
fn login_set_time_speed_packs_cmangos_local_time_fields() {
    let fields = CmangosTimeFields {
        year_since_1900: 126,
        month_zero_based: 4,
        day_of_month: 13,
        week_day: 3,
        hour: 15,
        minute: 4,
    };

    assert_eq!(
        cmangos_pack_time_fields(fields),
        (26u32 << 24) | (4u32 << 20) | (12u32 << 14) | (3u32 << 11) | (15u32 << 6) | 4
    );
}

#[test]
fn login_set_time_speed_packet_uses_cmangos_speed_and_time_layout() {
    let fields = CmangosTimeFields {
        year_since_1900: 125,
        month_zero_based: 0,
        day_of_month: 2,
        week_day: 4,
        hour: 6,
        minute: 30,
    };
    let body = build_login_set_time_speed_body_for_fields(fields);

    assert_eq!(body.len(), 8);
    assert_eq!(&body[0..4], &cmangos_pack_time_fields(fields).to_le_bytes());
    assert_eq!(&body[4..8], &0.01666667f32.to_le_bytes());
}

#[test]
fn login_set_time_speed_current_packet_no_longer_sends_zero_time() {
    let body = build_login_set_time_speed_body();
    let packed = u32::from_le_bytes(body[0..4].try_into().unwrap());

    assert_ne!(
        packed, 0,
        "SMSG_LOGIN_SETTIMESPEED should use the current server time, not the old placeholder"
    );
    assert_eq!(&body[4..8], &0.01666667f32.to_le_bytes());
}

#[test]
fn logout_response_uses_cmangos_combat_failure_shape() {
    let body = build_logout_response_body(LOGOUT_FAILURE_CANT_LOGOUT_NOW, false);

    assert_eq!(body, [1, 0, 0, 0, 0]);
}

#[test]
fn logout_request_pending_timer_uses_cmangos_twenty_seconds() {
    let start = Instant::now();
    let mut session = WorldSessionState::default();

    start_pending_logout(&mut session, start);

    assert_eq!(pending_logout_due_at(&session), Some(start + LOGOUT_DELAY));
    assert!(!pending_logout_is_due(
        &session,
        start + LOGOUT_DELAY - Duration::from_millis(1)
    ));
    assert!(pending_logout_is_due(&session, start + LOGOUT_DELAY));
}

#[test]
fn logout_cancel_clears_pending_timer() {
    let mut session = WorldSessionState::default();

    start_pending_logout(&mut session, Instant::now());
    cancel_pending_logout(&mut session);

    assert_eq!(pending_logout_due_at(&session), None);
}

#[test]
fn logout_request_blocks_combat_and_airborne_like_cmangos() {
    let mut session = WorldSessionState::default();
    assert!(!logout_is_blocked_by_combat(&session));

    session.combat.player_in_combat = true;
    assert!(logout_is_blocked_by_combat(&session));

    session.combat.player_in_combat = false;
    session.character.active_character = Some(Player {
        guid: 1,
        name: "Test".to_string(),
        race: 1,
        class: 1,
        level: 1,
        xp: 0,
        position: WorldPosition::default(),
        movement_flags: MOVEFLAG_JUMPING,
        client_time: 0,
        fall_time: 0,
        jump: JumpInfo::default(),
    });
    assert!(logout_is_blocked_by_combat(&session));
}

#[test]
fn logout_request_is_instant_only_without_character_or_when_resting() {
    let mut session = WorldSessionState::default();
    assert!(logout_request_is_instant(&session));

    session.character.active_character = Some(Player {
        guid: 1,
        name: "Test".to_string(),
        race: 1,
        class: 1,
        level: 1,
        xp: 0,
        position: WorldPosition::default(),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
        jump: JumpInfo::default(),
    });
    assert!(!logout_request_is_instant(&session));

    session.character.player_flags |= PLAYER_FLAGS_RESTING;
    assert!(logout_request_is_instant(&session));
}

#[test]
fn tutorial_flags_packet_serializes_account_state() {
    let body = build_tutorial_flags_body(&[1, 0x8000_0000, 0x0102_0304, 0, 0, 0, 0, 0xFFFF_FFFF]);

    assert_eq!(body.len(), 8 * 4);
    assert_eq!(&body[0..4], &1u32.to_le_bytes());
    assert_eq!(&body[4..8], &0x8000_0000u32.to_le_bytes());
    assert_eq!(&body[8..12], &0x0102_0304u32.to_le_bytes());
    assert_eq!(&body[28..32], &0xFFFF_FFFFu32.to_le_bytes());
}

#[test]
fn account_data_times_packet_is_zero_for_missing_data() {
    let body = build_account_data_times_body(&HashMap::new());

    assert_eq!(body.len(), ACCOUNT_DATA_TYPES * MD5_DIGEST_LEN);
    assert!(body.iter().all(|byte| *byte == 0));
}

#[test]
fn account_data_times_packet_uses_md5_for_cached_data() {
    let data = b"SET autoStand \"1\"\n".to_vec();
    let mut account_data = HashMap::new();
    account_data.insert(
        0,
        AccountDataCache {
            time: 1,
            data: data.clone(),
        },
    );

    let body = build_account_data_times_body(&account_data);

    let mut digest = Md5::new();
    digest.update(&data);
    let expected = digest.finalize();
    assert_eq!(body.len(), ACCOUNT_DATA_TYPES * MD5_DIGEST_LEN);
    assert_eq!(&body[0..MD5_DIGEST_LEN], expected.as_slice());
    assert!(body[MD5_DIGEST_LEN..].iter().all(|byte| *byte == 0));
}

#[test]
fn account_data_zlib_roundtrip_matches_declared_size() {
    let data = b"SET autoStand \"1\"\nSET autoLootDefault \"1\"\n";
    let compressed = zlib_compress(data).expect("account data should compress");

    let decoded = zlib_decompress(&compressed, data.len()).expect("account data should decompress");

    assert_eq!(decoded, data);
}

#[test]
fn login_verify_world_packet_keeps_map_and_position_order() {
    let character = test_character(1, 1);
    let body = build_login_verify_world_body(&character);

    assert_eq!(body.len(), 20);
    assert_eq!(&body[0..4], &character.map.to_le_bytes());
    assert_eq!(&body[4..8], &character.position_x.to_le_bytes());
    assert_eq!(&body[8..12], &character.position_y.to_le_bytes());
    assert_eq!(&body[12..16], &character.position_z.to_le_bytes());
    assert_eq!(&body[16..20], &character.orientation.to_le_bytes());
}

#[test]
fn bindpointupdate_packet_keeps_position_map_zone_order() {
    let character = test_character(1, 1);
    let body = build_bindpoint_update_body(&character);

    assert_eq!(body.len(), 20);
    assert_eq!(&body[0..4], &character.position_x.to_le_bytes());
    assert_eq!(&body[4..8], &character.position_y.to_le_bytes());
    assert_eq!(&body[8..12], &character.position_z.to_le_bytes());
    assert_eq!(&body[12..16], &character.map.to_le_bytes());
    assert_eq!(&body[16..20], &character.zone.to_le_bytes());
}

#[test]
fn tutorial_flag_updates_match_cmangos_word_bits() {
    let mut tutorials = [0u32; 8];

    assert!(apply_tutorial_flag(&mut tutorials, 0));
    assert!(apply_tutorial_flag(&mut tutorials, 33));
    assert!(apply_tutorial_flag(&mut tutorials, 255));
    assert!(!apply_tutorial_flag(&mut tutorials, 256));

    assert_eq!(tutorials[0], 1);
    assert_eq!(tutorials[1], 2);
    assert_eq!(tutorials[7], 0x8000_0000);
}

#[test]
fn parses_equipment_cache_item_ids() {
    let equipment = parse_equipment_cache(Some("0 0 0 0 0 0 38 0 0 0 0 0 39 0"));

    assert_eq!(equipment[3], 38);
    assert_eq!(equipment[6], 39);
}

#[test]
fn maps_inventory_slots_to_player_update_guid_fields() {
    assert_eq!(
        inventory_slot_update_field(3),
        Some(PLAYER_FIELD_INV_SLOT_HEAD + 6)
    );
    assert_eq!(
        inventory_slot_update_field(23),
        Some(PLAYER_FIELD_PACK_SLOT_1)
    );
    assert_eq!(
        inventory_slot_update_field(19),
        Some(PLAYER_FIELD_INV_SLOT_HEAD + 38)
    );
    assert_eq!(
        inventory_slot_update_field(BANK_SLOT_ITEM_START),
        Some(PLAYER_FIELD_BANK_SLOT_1)
    );
    assert_eq!(
        inventory_slot_update_field(BANK_SLOT_ITEM_END - 1),
        Some(PLAYER_FIELD_BANK_SLOT_1 + 46)
    );
    assert_eq!(
        inventory_slot_update_field(BANK_SLOT_BAG_START),
        Some(PLAYER_FIELD_BANKBAG_SLOT_1)
    );
    assert_eq!(
        inventory_slot_update_field(BANK_SLOT_BAG_END - 1),
        Some(PLAYER_FIELD_BANKBAG_SLOT_1 + 10)
    );
    assert_eq!(inventory_slot_update_field(BANK_SLOT_BAG_END), None);
}

#[test]
fn visible_item_updates_prefer_live_equipped_inventory() {
    let mut character = test_character(1, 1);
    character.equipment_cache = Some("25 0".to_string());
    let inventory = [CharacterInventoryItem {
        bag: 0,
        slot: 0,
        item: 99,
        item_template: 2362,
        count: 1,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 18,
    }];
    let mut values = vec![None; PLAYER_END_FIELDS];

    set_visible_item_update_values(&mut values, &character, &inventory).unwrap();

    assert_eq!(values[0x104], Some(2362));
}

#[test]
fn writes_inventory_item_guid_update_values() {
    let mut values = vec![None; PLAYER_END_FIELDS];
    let item = CharacterInventoryItem {
        bag: 0,
        slot: 15,
        item: 42,
        item_template: 25,
        count: 1,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 10,
    };

    set_inventory_slot_update_values(&mut values, &[item]).unwrap();

    let guid = ObjectGuid::new(HighGuid::Item, 0, 42);
    let field = PLAYER_FIELD_INV_SLOT_HEAD + 15 * 2;
    assert_eq!(values[field], Some(guid.raw() as u32));
    assert_eq!(values[field + 1], Some((guid.raw() >> 32) as u32));
}

#[test]
fn main_hand_inventory_and_visible_update_values_are_written() {
    let mut character = test_character(1, 1);
    character.equipment_cache =
        Some("0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 25 0".to_string());
    let item = CharacterInventoryItem {
        bag: 0,
        slot: EQUIPMENT_SLOT_MAINHAND,
        item: 42,
        item_template: 25,
        count: 1,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 10,
    };
    let mut values = vec![None; PLAYER_END_FIELDS];

    set_visible_item_update_values(&mut values, &character, std::slice::from_ref(&item)).unwrap();
    set_inventory_slot_update_values(&mut values, &[item]).unwrap();

    let guid = ObjectGuid::new(HighGuid::Item, 0, 42);
    let inventory_field = PLAYER_FIELD_INV_SLOT_HEAD + EQUIPMENT_SLOT_MAINHAND as usize * 2;
    let visible_field = 0x104 + EQUIPMENT_SLOT_MAINHAND as usize * 12;
    assert_eq!(values[inventory_field], Some(guid.raw() as u32));
    assert_eq!(values[inventory_field + 1], Some((guid.raw() >> 32) as u32));
    assert_eq!(values[visible_field], Some(25));
}

#[test]
fn parses_backpack_inventory_move_packets() {
    let swap_inv =
        InventoryMoveRequest::read(WorldOpcode::CmsgSwapInvItem as u32, &[23, 24]).unwrap();
    assert_eq!(
        swap_inv,
        InventoryMoveRequest {
            src_bag: INVENTORY_SLOT_BAG_0,
            src_slot: 23,
            dst_bag: INVENTORY_SLOT_BAG_0,
            dst_slot: 24,
        }
    );
    assert!(swap_inv.is_supported_inventory_move());

    let swap_item = InventoryMoveRequest::read(
        WorldOpcode::CmsgSwapItem as u32,
        &[
            CLIENT_INVENTORY_SLOT_BAG_0,
            25,
            CLIENT_INVENTORY_SLOT_BAG_0,
            23,
        ],
    )
    .unwrap();
    assert_eq!(
        swap_item,
        InventoryMoveRequest {
            src_bag: INVENTORY_SLOT_BAG_0,
            src_slot: 23,
            dst_bag: INVENTORY_SLOT_BAG_0,
            dst_slot: 25,
        }
    );
    assert!(swap_item.is_supported_inventory_move());
}

#[test]
fn parses_equipment_inventory_move_packets() {
    let unequip =
        InventoryMoveRequest::read(WorldOpcode::CmsgSwapInvItem as u32, &[3, 26]).unwrap();
    assert_eq!(
        unequip,
        InventoryMoveRequest {
            src_bag: INVENTORY_SLOT_BAG_0,
            src_slot: 3,
            dst_bag: INVENTORY_SLOT_BAG_0,
            dst_slot: 26,
        }
    );
    assert!(unequip.is_supported_inventory_move());
    assert!(item_fits_equipment_slot(4, 3));
    assert!(item_fits_equipment_slot(1, 0));
    assert!(item_fits_equipment_slot(3, 2));
    assert!(item_fits_equipment_slot(5, 4));
    assert!(item_fits_equipment_slot(20, 4));
    assert!(item_fits_equipment_slot(6, 5));
    assert!(item_fits_equipment_slot(9, 8));
    assert!(item_fits_equipment_slot(10, 9));
    assert!(item_fits_equipment_slot(11, 10));
    assert!(item_fits_equipment_slot(11, 11));
    assert!(item_fits_equipment_slot(12, 12));
    assert!(item_fits_equipment_slot(12, 13));
    assert!(item_fits_equipment_slot(16, 14));
    assert!(item_fits_equipment_slot(14, 16));
    assert!(item_fits_equipment_slot(23, 16));
    assert!(item_fits_equipment_slot(26, 17));
    assert!(item_fits_equipment_slot(19, 18));
    assert_eq!(preferred_equipment_slot(1), Some(0));
    assert_eq!(preferred_equipment_slot(5), Some(4));
    assert_eq!(preferred_equipment_slot(6), Some(5));
    assert_eq!(preferred_equipment_slot(9), Some(8));
    assert_eq!(preferred_equipment_slot(10), Some(9));
    assert_eq!(preferred_equipment_slot(14), Some(16));
    assert_eq!(preferred_equipment_slot(26), Some(17));
    assert!(!item_fits_equipment_slot(4, 15));
}

#[test]
fn inventory_swap_validation_checks_displaced_item_against_source_equipment_slot() {
    let bread = test_item_template(117, 0, 0, 0.0, 0.0, 0);
    let mut sword = test_item_template(25, ITEM_CLASS_WEAPON, 13, 2.0, 4.0, 0);
    sword.subclass = 7;
    let skills = vec![test_skill(SKILL_SWORDS, 1, 5)];
    let active_spells = HashSet::new();
    let context = CharacterEquipValidationContext {
        level: 1,
        race: 1,
        class: 1,
        skills: &skills,
        active_spells: &active_spells,
        reputations: &[],
        in_combat: false,
    };

    assert_eq!(
        item_can_enter_bag0_equipment_or_bag_slot(
            EQUIPMENT_SLOT_MAINHAND,
            &bread,
            context,
        ),
        Some((EQUIP_ERR_ITEM_DOESNT_GO_TO_SLOT, None)),
        "a backpack item displaced by an equipped weapon must still fit the vacated main-hand slot"
    );
    assert_eq!(
        item_can_enter_bag0_equipment_or_bag_slot(
            EQUIPMENT_SLOT_MAINHAND,
            &sword,
            context,
        ),
        None
    );
}

#[test]
fn inventory_combat_equip_state_rules_match_cmangos_allowlist() {
    let chest = test_item_template(38, ITEM_CLASS_ARMOR, 5, 0.0, 0.0, 12);
    let sword = test_item_template(25, ITEM_CLASS_WEAPON, 13, 2.0, 4.0, 0);
    let shield = test_item_template(2362, ITEM_CLASS_ARMOR, INVTYPE_SHIELD, 0.0, 0.0, 11);
    let mut held = test_item_template(2500, ITEM_CLASS_ARMOR, INVTYPE_HOLDABLE, 0.0, 0.0, 0);
    held.subclass = 0;
    let mut relic = test_item_template(2501, ITEM_CLASS_ARMOR, INVTYPE_RELIC, 0.0, 0.0, 0);
    relic.subclass = 0;
    let projectile = test_item_template(2512, ITEM_CLASS_PROJECTILE, INVTYPE_AMMO, 1.0, 1.5, 0);

    assert_eq!(
        item_can_leave_bag0_equipment_or_bag_slot(4, &chest, true),
        Some(EQUIP_ERR_NOT_IN_COMBAT)
    );
    assert!(item_can_leave_bag0_equipment_or_bag_slot(
        EQUIPMENT_SLOT_MAINHAND,
        &sword,
        true
    )
    .is_none());
    assert!(item_can_change_equip_state_in_combat(&shield));
    assert!(item_can_change_equip_state_in_combat(&held));
    assert!(item_can_change_equip_state_in_combat(&relic));
    assert!(item_can_change_equip_state_in_combat(&projectile));
    assert!(!item_can_change_equip_state_in_combat(&chest));
}

#[test]
fn inventory_combat_weapon_slot_changes_drive_cast_and_swing_side_effects() {
    assert!(combat_equipment_slot_change_interrupts_active_spell_cast(
        &[EQUIPMENT_SLOT_MAINHAND],
        true
    ));
    assert!(combat_equipment_slot_change_interrupts_active_spell_cast(
        &[EQUIPMENT_SLOT_OFFHAND],
        true
    ));
    assert!(combat_equipment_slot_change_interrupts_active_spell_cast(
        &[EQUIPMENT_SLOT_RANGED],
        true
    ));
    assert!(!combat_equipment_slot_change_interrupts_active_spell_cast(
        &[4],
        true
    ));
    assert!(!combat_equipment_slot_change_interrupts_active_spell_cast(
        &[EQUIPMENT_SLOT_MAINHAND],
        false
    ));

    assert!(combat_equipment_slot_change_resets_main_hand_swing(
        &[EQUIPMENT_SLOT_MAINHAND],
        true
    ));
    assert!(!combat_equipment_slot_change_resets_main_hand_swing(
        &[EQUIPMENT_SLOT_OFFHAND],
        true
    ));
    assert!(!combat_equipment_slot_change_resets_main_hand_swing(
        &[EQUIPMENT_SLOT_MAINHAND],
        false
    ));
}

#[test]
fn inventory_recomputed_combat_stats_keep_passive_resistance_on_self_update() {
    let world_stats = PlayerWorldStats {
        base_health: 20,
        base_mana: 100,
        stats: [17, 25, 19, 24, 22],
        next_level_xp: 400,
    };
    let nature_resistance = ActiveAura {
        spell_id: 20583,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 1,
        interrupt_flags: 0,
        positive: true,
        visible: false,
        duration_millis: None,
        expires_at: None,
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::Resistance {
            school_mask: 1 << 3,
            amount: 10,
        }],
        proc_triggers: Vec::new(),
    };

    let (base_combat_stats, effective_combat_stats) = inventory_recomputed_combat_stats(
        8,
        1,
        world_stats,
        &[],
        None,
        &[nature_resistance],
    );

    assert_eq!(base_combat_stats.resistances[3], 0);
    assert_eq!(base_combat_stats.resistance_buff_mod_positive[3], 0);
    assert_eq!(effective_combat_stats.resistances[3], 10);
    assert_eq!(effective_combat_stats.resistance_buff_mod_positive[3], 10);

    let body = build_player_combat_stats_update_body(7, &effective_combat_stats).unwrap();
    let (values, trailing) =
        decode_values_update_block(&body[5..], ObjectGuid::new(HighGuid::Player, 0, 7));
    assert!(trailing.is_empty());
    assert_eq!(values[UNIT_FIELD_RESISTANCES + 3], Some(10));
    assert_eq!(
        values[PLAYER_FIELD_RESISTANCEBUFFMODSPOSITIVE + 3],
        Some(10)
    );

}

#[test]
fn inventory_warrior_armor_proficiency_controls_cloth_leather_mail_equips() {
    let mut cloth = test_item_template(1000, ITEM_CLASS_ARMOR, 4, 0.0, 0.0, 1);
    cloth.subclass = 1;
    let mut leather = test_item_template(1001, ITEM_CLASS_ARMOR, 4, 0.0, 0.0, 2);
    leather.subclass = 2;
    let mut mail = test_item_template(1002, ITEM_CLASS_ARMOR, 4, 0.0, 0.0, 3);
    mail.subclass = 3;
    let warrior_skills = vec![
        test_skill(415, 1, 1),
        test_skill(414, 1, 1),
        test_skill(413, 1, 1),
    ];
    let cloth_only_skills = vec![test_skill(415, 1, 1)];

    assert_eq!(
        character_can_equip_item_template(1, 1, 1, &cloth, &warrior_skills, &HashSet::new(), &[]),
        0
    );
    assert_eq!(
        character_can_equip_item_template(1, 1, 1, &leather, &warrior_skills, &HashSet::new(), &[]),
        0
    );
    assert_eq!(
        character_can_equip_item_template(1, 1, 1, &mail, &warrior_skills, &HashSet::new(), &[]),
        0
    );
    assert_eq!(
        character_can_equip_item_template(
            1,
            1,
            8,
            &leather,
            &cloth_only_skills,
            &HashSet::new(),
            &[]
        ),
        EQUIP_ERR_NO_REQUIRED_PROFICIENCY
    );
}

#[test]
fn inventory_equip_validation_rejects_item_with_wrong_allowable_class() {
    let mut cloth = test_item_template(1003, ITEM_CLASS_ARMOR, 4, 0.0, 0.0, 1);
    cloth.subclass = 1;
    cloth.allowable_class = 1 << (8 - 1);
    let skills = vec![test_skill(415, 1, 1)];

    assert_eq!(
        character_can_equip_item_template(1, 1, 1, &cloth, &skills, &HashSet::new(), &[]),
        EQUIP_ERR_YOU_CAN_NEVER_USE_THAT_ITEM
    );
}

#[test]
fn inventory_use_validation_returns_cmangos_level_skill_spell_and_reputation_errors() {
    let skills = vec![test_skill(185, 50, 75)];
    let mut active_spells = HashSet::new();
    active_spells.insert(33388);
    let reputations = vec![CharacterReputation {
        faction: 72,
        standing: 2_999,
        flags: 0,
    }];

    let mut high_level = test_item_template(1004, ITEM_CLASS_ARMOR, 4, 0.0, 0.0, 1);
    high_level.required_level = 10;
    assert_eq!(
        character_can_use_item_template(
            5,
            1,
            1,
            &high_level,
            &skills,
            &active_spells,
            &reputations
        ),
        EQUIP_ERR_CANT_EQUIP_LEVEL_I
    );

    let mut missing_skill_rank = test_item_template(1005, ITEM_CLASS_ARMOR, 4, 0.0, 0.0, 1);
    missing_skill_rank.required_skill = 185;
    missing_skill_rank.required_skill_rank = 75;
    assert_eq!(
        character_can_use_item_template(
            60,
            1,
            1,
            &missing_skill_rank,
            &skills,
            &active_spells,
            &reputations
        ),
        EQUIP_ERR_CANT_EQUIP_SKILL
    );

    let mut missing_spell = test_item_template(1006, ITEM_CLASS_ARMOR, 4, 0.0, 0.0, 1);
    missing_spell.required_spell = 33391;
    assert_eq!(
        character_can_use_item_template(
            60,
            1,
            1,
            &missing_spell,
            &skills,
            &active_spells,
            &reputations
        ),
        EQUIP_ERR_NO_REQUIRED_PROFICIENCY
    );

    let mut honored_required = test_item_template(1007, ITEM_CLASS_ARMOR, 4, 0.0, 0.0, 1);
    honored_required.required_reputation_faction = 72;
    honored_required.required_reputation_rank = 4;
    assert_eq!(
        character_can_use_item_template(
            60,
            1,
            1,
            &honored_required,
            &skills,
            &active_spells,
            &reputations
        ),
        EQUIP_ERR_CANT_EQUIP_REPUTATION
    );
}

#[test]
fn inventory_change_level_failure_includes_required_level_before_item_guids() {
    let item = ObjectGuid::new(HighGuid::Item, 0, 42);
    let body = build_inventory_change_failure_body(
        EQUIP_ERR_CANT_EQUIP_LEVEL_I,
        Some(item),
        None,
        Some(12),
    );
    assert_eq!(body[0], EQUIP_ERR_CANT_EQUIP_LEVEL_I);
    assert_eq!(u32::from_le_bytes(body[1..5].try_into().unwrap()), 12);
    assert_eq!(
        u64::from_le_bytes(body[5..13].try_into().unwrap()),
        item.raw()
    );
}

#[test]
fn use_item_packet_parses_backpack_slot_spell_index_and_targets() {
    let target = ObjectGuid::new(HighGuid::Player, 0, 7);
    let mut body = vec![CLIENT_INVENTORY_SLOT_BAG_0, 23, 1];
    body.extend_from_slice(&SPELL_CAST_TARGET_UNIT.to_le_bytes());
    PackedGuid::write(&mut body, target).unwrap();

    let packet = UseItemPacket::read(&body).unwrap();
    assert_eq!(packet.bag, INVENTORY_SLOT_BAG_0);
    assert_eq!(packet.slot, 23);
    assert_eq!(packet.spell_index, 1);
    assert_eq!(packet.targets.unit_target, Some(target));
}

#[test]
fn item_use_spell_selects_requested_on_use_spell_or_first_available() {
    let mut template = test_item_template(1008, 0, 0, 0.0, 0.0, 0);
    template.spells[0] = wow_db::ItemTemplateSpell {
        spell_id: 111,
        spell_trigger: 1,
        ..Default::default()
    };
    template.spells[1] = wow_db::ItemTemplateSpell {
        spell_id: 222,
        spell_trigger: ITEM_SPELLTRIGGER_ON_USE,
        spell_charges: -1,
        ..Default::default()
    };

    assert_eq!(item_use_spell(&template, 1).unwrap().spell_id, 222);
    assert_eq!(item_use_spell(&template, 0).unwrap().spell_id, 222);
}

#[test]
fn parses_bag_container_inventory_move_packets() {
    let into_bag = InventoryMoveRequest::read(
        WorldOpcode::CmsgSwapItem as u32,
        &[19, 0, CLIENT_INVENTORY_SLOT_BAG_0, 24],
    )
    .unwrap();
    assert_eq!(
        into_bag,
        InventoryMoveRequest {
            src_bag: INVENTORY_SLOT_BAG_0,
            src_slot: 24,
            dst_bag: 19,
            dst_slot: 0,
        }
    );
    assert!(into_bag.is_supported_inventory_move());

    let within_bag =
        InventoryMoveRequest::read(WorldOpcode::CmsgSwapItem as u32, &[19, 1, 19, 0]).unwrap();
    assert_eq!(
        within_bag,
        InventoryMoveRequest {
            src_bag: 19,
            src_slot: 0,
            dst_bag: 19,
            dst_slot: 1,
        }
    );
    assert!(within_bag.is_supported_inventory_move());
}

#[test]
fn inventory_move_rejects_equipped_bag_into_its_own_container() {
    let into_self =
        InventoryMoveRequest::read(WorldOpcode::CmsgSwapItem as u32, &[19, 0, 255, 19]).unwrap();
    assert!(into_self.moves_equipped_bag_into_itself());

    let reverse_self =
        InventoryMoveRequest::read(WorldOpcode::CmsgSwapItem as u32, &[255, 19, 19, 0]).unwrap();
    assert!(reverse_self.moves_equipped_bag_into_itself());

    let normal_contained_move =
        InventoryMoveRequest::read(WorldOpcode::CmsgSwapItem as u32, &[19, 1, 19, 0]).unwrap();
    assert!(!normal_contained_move.moves_equipped_bag_into_itself());
}

#[test]
fn inventory_change_failure_for_bag_self_move_includes_item_guid() {
    let item_guid = ObjectGuid::new(HighGuid::Item, 0, 77);
    let body = build_inventory_change_failure_body(
        EQUIP_ERR_NONEMPTY_BAG_OVER_OTHER_BAG,
        Some(item_guid),
        None,
        None,
    );

    assert_eq!(body[0], EQUIP_ERR_NONEMPTY_BAG_OVER_OTHER_BAG);
    assert_eq!(&body[1..9], &item_guid.raw().to_le_bytes());
    assert_eq!(&body[9..17], &0u64.to_le_bytes());
}

#[test]
fn inventory_change_failure_for_nonempty_bag_unequip_includes_item_guid() {
    let item_guid = ObjectGuid::new(HighGuid::Item, 0, 77);
    let body = build_inventory_change_failure_body(
        EQUIP_ERR_CAN_ONLY_DO_WITH_EMPTY_BAGS,
        Some(item_guid),
        None,
        None,
    );

    assert_eq!(body[0], EQUIP_ERR_CAN_ONLY_DO_WITH_EMPTY_BAGS);
    assert_eq!(&body[1..9], &item_guid.raw().to_le_bytes());
    assert_eq!(&body[9..17], &0u64.to_le_bytes());
}

#[test]
fn parses_autostore_bag_item_packet_shape() {
    let body = [CLIENT_INVENTORY_SLOT_BAG_0, 3, 19];
    assert_eq!(normalize_client_bag(body[0]), INVENTORY_SLOT_BAG_0);
    assert_eq!(body[1], 3);
    assert_eq!(body[2], 19);
}

#[test]
fn buyback_slot_update_writes_guid_price_and_timestamp_fields() {
    let entry = BuybackItem {
        slot: BUYBACK_SLOT_START,
        item: 42,
        price: 75,
        timestamp: 30 * 3600,
    };
    let body = build_buyback_slot_update_body(11, Some(entry), BUYBACK_SLOT_START).unwrap();
    assert_eq!(&body[0..4], &1u32.to_le_bytes());
    assert_eq!(body[4], 0);

    let player_guid = ObjectGuid::new(HighGuid::Player, 0, 11);
    let (values, rest) = decode_values_update_block(&body[5..], player_guid);
    assert!(rest.is_empty());
    let item_guid = ObjectGuid::new(HighGuid::Item, 0, 42);
    assert_eq!(
        values[PLAYER_FIELD_VENDORBUYBACK_SLOT_1],
        Some(item_guid.raw() as u32)
    );
    assert_eq!(
        values[PLAYER_FIELD_VENDORBUYBACK_SLOT_1 + 1],
        Some((item_guid.raw() >> 32) as u32)
    );
    assert_eq!(values[PLAYER_FIELD_BUYBACK_PRICE_1], Some(75));
    assert_eq!(values[PLAYER_FIELD_BUYBACK_TIMESTAMP_1], Some(30 * 3600));
}

#[test]
fn buyback_fills_all_twelve_slots_including_last() {
    let mut session = WorldSessionState::default();

    for index in 0..12 {
        let slot = next_buyback_slot(&session);
        push_buyback_entry(&mut session, slot, 1_000 + index, 5);
    }

    for slot in BUYBACK_SLOT_START..BUYBACK_SLOT_END {
        assert!(
            session
                .inventory
                .buyback_items
                .iter()
                .any(|entry| entry.slot == slot),
            "buyback slot {slot} should be occupied"
        );
    }
}

#[test]
fn buyback_reuses_last_slot_after_it_is_cleared() {
    let mut session = WorldSessionState::default();

    for index in 0..12 {
        let slot = next_buyback_slot(&session);
        push_buyback_entry(&mut session, slot, 1_000 + index, 5);
    }

    remove_buyback_entry_from_session(&mut session, BUYBACK_SLOT_END - 1);

    assert_eq!(next_buyback_slot(&session), BUYBACK_SLOT_END - 1);
}

#[test]
fn buyback_full_list_replaces_oldest_slot_in_order() {
    let mut session = WorldSessionState::default();

    for index in 0..12 {
        let slot = next_buyback_slot(&session);
        push_buyback_entry(&mut session, slot, 1_000 + index, 5);
    }

    let replacement_slot = next_buyback_slot(&session);
    assert_eq!(replacement_slot, BUYBACK_SLOT_START);
    let replacement = push_buyback_entry(&mut session, replacement_slot, 2_000, 5);

    assert_eq!(replacement.timestamp, 30 * 3600 + 12);
    assert_eq!(next_buyback_slot(&session), BUYBACK_SLOT_START + 1);
}

#[test]
fn autoequip_bag_prefers_first_empty_bag_slot() {
    let mut bag = test_item_template(
        RUST_VENDOR_BAG_ITEM,
        ITEM_CLASS_CONTAINER,
        INVTYPE_BAG,
        0.0,
        0.0,
        0,
    );
    bag.container_slots = 6;
    let inventory = [CharacterInventoryItem {
        bag: INVENTORY_SLOT_BAG_0 as u32,
        slot: INVENTORY_SLOT_BAG_START,
        item: 77,
        item_template: RUST_VENDOR_BAG_ITEM,
        count: 1,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    }];

    assert_eq!(
        preferred_equipment_slot_for_inventory(&bag, &inventory),
        Some(INVENTORY_SLOT_BAG_START + 1)
    );
}

#[test]
fn autoequip_bag_has_no_destination_when_all_bag_slots_are_full() {
    let mut bag = test_item_template(
        RUST_VENDOR_BAG_ITEM,
        ITEM_CLASS_CONTAINER,
        INVTYPE_BAG,
        0.0,
        0.0,
        0,
    );
    bag.container_slots = 6;
    let inventory = (INVENTORY_SLOT_BAG_START..INVENTORY_SLOT_BAG_END)
        .enumerate()
        .map(|(index, slot)| CharacterInventoryItem {
            bag: INVENTORY_SLOT_BAG_0 as u32,
            slot,
            item: 100 + index as u32,
            item_template: RUST_VENDOR_BAG_ITEM,
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        preferred_equipment_slot_for_inventory(&bag, &inventory),
        None
    );
}

#[test]
fn inventory_store_plan_merges_stack_before_empty_slots() {
    let mut bread = test_item_template(4540, 0, 0, 0.0, 0.0, 0);
    bread.stackable = 20;
    let inventory = [CharacterInventoryItem {
        bag: INVENTORY_SLOT_BAG_0 as u32,
        slot: INVENTORY_SLOT_ITEM_START,
        item: 90,
        item_template: 4540,
        count: 5,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    }];

    let plan = plan_store_item(&inventory, &bread, 5, &[], None, None).unwrap();

    assert_eq!(
        plan,
        vec![StoreSlot {
            bag: INVENTORY_SLOT_BAG_0,
            slot: INVENTORY_SLOT_ITEM_START,
            count: 5,
            existing_item: Some(90),
        }]
    );
}

#[test]
fn inventory_store_plan_returns_none_when_backpack_is_full_and_no_stack_can_merge() {
    let mut bread = test_item_template(4540, 0, 0, 0.0, 0.0, 0);
    bread.stackable = 20;
    let inventory = (INVENTORY_SLOT_ITEM_START..INVENTORY_SLOT_ITEM_END)
        .map(|slot| CharacterInventoryItem {
            bag: INVENTORY_SLOT_BAG_0 as u32,
            slot,
            item: 1_000 + slot as u32,
            item_template: 6948,
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        })
        .collect::<Vec<_>>();

    assert!(plan_store_item(&inventory, &bread, 1, &[], None, None).is_none());
}

#[test]
fn inventory_store_plan_uses_equipped_bag_capacity_after_backpack() {
    let mut bread = test_item_template(4540, 0, 0, 0.0, 0.0, 0);
    bread.stackable = 20;
    let mut inventory: Vec<_> = (INVENTORY_SLOT_ITEM_START..INVENTORY_SLOT_ITEM_END)
        .map(|slot| CharacterInventoryItem {
            bag: INVENTORY_SLOT_BAG_0 as u32,
            slot,
            item: 1_000 + slot as u32,
            item_template: 6948,
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        })
        .collect();
    inventory.push(CharacterInventoryItem {
        bag: INVENTORY_SLOT_BAG_0 as u32,
        slot: INVENTORY_SLOT_BAG_START,
        item: 77,
        item_template: RUST_VENDOR_BAG_ITEM,
        count: 1,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    });
    let bags = [EquippedBagInfo {
        slot: INVENTORY_SLOT_BAG_START,
        container_slots: 6,
        class: ITEM_CLASS_CONTAINER,
        subclass: ITEM_SUBCLASS_CONTAINER,
    }];

    let plan = plan_store_item(&inventory, &bread, 5, &bags, None, None).unwrap();

    assert_eq!(
        plan,
        vec![StoreSlot {
            bag: INVENTORY_SLOT_BAG_START,
            slot: 0,
            count: 5,
            existing_item: None,
        }]
    );
}

#[test]
fn bank_bag_slot_count_uses_player_bytes2_byte_2() {
    let mut session = WorldSessionState::default();
    session.character.player_visual = Some(PlayerVisualState {
        gender: 0,
        player_bytes: 0,
        player_bytes2: 0xAA00_0000,
        equipment_cache: None,
        guildid: None,
    });

    let updated = with_bank_bag_slot_count(0xAA00_0000, 3);
    session
        .character
        .player_visual
        .as_mut()
        .unwrap()
        .player_bytes2 = updated;

    assert_eq!(updated, 0xAA03_0000);
    assert_eq!(bank_bag_slot_count(&session), 3);
}

#[test]
fn bank_store_plan_merges_bank_main_before_empty_slots() {
    let mut bread = test_item_template(4540, 0, 0, 0.0, 0.0, 0);
    bread.stackable = 20;
    let inventory = [CharacterInventoryItem {
        bag: INVENTORY_SLOT_BAG_0 as u32,
        slot: BANK_SLOT_ITEM_START,
        item: 90,
        item_template: 4540,
        count: 5,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    }];

    let plan = plan_bank_item(&inventory, &bread, 5, &[], 0, None, None).unwrap();

    assert_eq!(
        plan,
        vec![StoreSlot {
            bag: INVENTORY_SLOT_BAG_0,
            slot: BANK_SLOT_ITEM_START,
            count: 5,
            existing_item: Some(90),
        }]
    );
}

#[test]
fn bank_store_plan_uses_purchased_bank_bag_capacity() {
    let mut bread = test_item_template(4540, 0, 0, 0.0, 0.0, 0);
    bread.stackable = 20;
    let mut inventory: Vec<_> = (BANK_SLOT_ITEM_START..BANK_SLOT_ITEM_END)
        .map(|slot| CharacterInventoryItem {
            bag: INVENTORY_SLOT_BAG_0 as u32,
            slot,
            item: 1_000 + slot as u32,
            item_template: 6948,
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        })
        .collect();
    inventory.push(CharacterInventoryItem {
        bag: INVENTORY_SLOT_BAG_0 as u32,
        slot: BANK_SLOT_BAG_START,
        item: 77,
        item_template: RUST_VENDOR_BAG_ITEM,
        count: 1,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    });
    let bank_bags = [EquippedBagInfo {
        slot: BANK_SLOT_BAG_START,
        container_slots: 6,
        class: ITEM_CLASS_CONTAINER,
        subclass: ITEM_SUBCLASS_CONTAINER,
    }];

    assert!(plan_bank_item(&inventory, &bread, 5, &bank_bags, 0, None, None).is_none());

    let plan = plan_bank_item(&inventory, &bread, 5, &bank_bags, 1, None, None).unwrap();

    assert_eq!(
        plan,
        vec![StoreSlot {
            bag: BANK_SLOT_BAG_START,
            slot: 0,
            count: 5,
            existing_item: None,
        }]
    );
}

#[test]
fn quest_reward_storage_plans_equipped_bag_after_required_item_consumed_from_bag() {
    let reward_template = test_item_template(117, 0, 0, 0.0, 0.0, 0);
    let reward = QuestRewardGrant {
        item: reward_template.entry,
        count: 1,
        max_durability: 0,
        container_slots: None,
        template: reward_template,
    };
    let mut inventory: Vec<_> = (INVENTORY_SLOT_ITEM_START..INVENTORY_SLOT_ITEM_END)
        .map(|slot| CharacterInventoryItem {
            bag: INVENTORY_SLOT_BAG_0 as u32,
            slot,
            item: 1_000 + slot as u32,
            item_template: 6948,
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        })
        .collect();
    inventory.push(CharacterInventoryItem {
        bag: INVENTORY_SLOT_BAG_0 as u32,
        slot: INVENTORY_SLOT_BAG_START,
        item: 77,
        item_template: RUST_VENDOR_BAG_ITEM,
        count: 1,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    });
    inventory.push(CharacterInventoryItem {
        bag: INVENTORY_SLOT_BAG_START as u32,
        slot: 0,
        item: 88,
        item_template: 182,
        count: 1,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    });
    let bags = [EquippedBagInfo {
        slot: INVENTORY_SLOT_BAG_START,
        container_slots: 6,
        class: ITEM_CLASS_CONTAINER,
        subclass: ITEM_SUBCLASS_CONTAINER,
    }];
    let required = [QuestRequiredItemConsume {
        bag: INVENTORY_SLOT_BAG_START as u32,
        slot: 0,
        count: 1,
        removes_stack: true,
    }];

    let plans = plan_quest_reward_storage(&inventory, &[reward], &bags, &required).unwrap();

    assert_eq!(
        plans,
        vec![vec![StoreSlot {
            bag: INVENTORY_SLOT_BAG_START,
            slot: 0,
            count: 1,
            existing_item: None,
        }]]
    );
}

#[test]
fn quest_reward_storage_uses_freed_backpack_and_equipped_bag_for_multiple_rewards() {
    let first_template = test_item_template(117, 0, 0, 0.0, 0.0, 0);
    let second_template = test_item_template(118, 0, 0, 0.0, 0.0, 0);
    let rewards = [
        QuestRewardGrant {
            item: first_template.entry,
            count: 1,
            max_durability: 0,
            container_slots: None,
            template: first_template,
        },
        QuestRewardGrant {
            item: second_template.entry,
            count: 1,
            max_durability: 0,
            container_slots: None,
            template: second_template,
        },
    ];
    let mut inventory: Vec<_> = (INVENTORY_SLOT_ITEM_START..INVENTORY_SLOT_ITEM_END)
        .map(|slot| CharacterInventoryItem {
            bag: INVENTORY_SLOT_BAG_0 as u32,
            slot,
            item: 1_000 + slot as u32,
            item_template: if slot == INVENTORY_SLOT_ITEM_START {
                182
            } else {
                6948
            },
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        })
        .collect();
    inventory.push(CharacterInventoryItem {
        bag: INVENTORY_SLOT_BAG_0 as u32,
        slot: INVENTORY_SLOT_BAG_START,
        item: 77,
        item_template: RUST_VENDOR_BAG_ITEM,
        count: 1,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    });
    let bags = [EquippedBagInfo {
        slot: INVENTORY_SLOT_BAG_START,
        container_slots: 6,
        class: ITEM_CLASS_CONTAINER,
        subclass: ITEM_SUBCLASS_CONTAINER,
    }];
    let required = [QuestRequiredItemConsume {
        bag: INVENTORY_SLOT_BAG_0 as u32,
        slot: INVENTORY_SLOT_ITEM_START,
        count: 1,
        removes_stack: true,
    }];

    let plans = plan_quest_reward_storage(&inventory, &rewards, &bags, &required).unwrap();

    assert_eq!(
        plans,
        vec![
            vec![StoreSlot {
                bag: INVENTORY_SLOT_BAG_0,
                slot: INVENTORY_SLOT_ITEM_START,
                count: 1,
                existing_item: None,
            }],
            vec![StoreSlot {
                bag: INVENTORY_SLOT_BAG_START,
                slot: 0,
                count: 1,
                existing_item: None,
            }],
        ]
    );
}

#[test]
fn quest_reward_storage_fails_without_consuming_partial_required_stack_space() {
    let reward_template = test_item_template(117, 0, 0, 0.0, 0.0, 0);
    let reward = QuestRewardGrant {
        item: reward_template.entry,
        count: 1,
        max_durability: 0,
        container_slots: None,
        template: reward_template,
    };
    let inventory = (INVENTORY_SLOT_ITEM_START..INVENTORY_SLOT_ITEM_END)
        .map(|slot| CharacterInventoryItem {
            bag: INVENTORY_SLOT_BAG_0 as u32,
            slot,
            item: 1_000 + slot as u32,
            item_template: if slot == INVENTORY_SLOT_ITEM_START {
                182
            } else {
                6948
            },
            count: if slot == INVENTORY_SLOT_ITEM_START {
                2
            } else {
                1
            },
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        })
        .collect::<Vec<_>>();
    let required = [QuestRequiredItemConsume {
        bag: INVENTORY_SLOT_BAG_0 as u32,
        slot: INVENTORY_SLOT_ITEM_START,
        count: 1,
        removes_stack: false,
    }];

    assert!(plan_quest_reward_storage(&inventory, &[reward], &[], &required).is_none());
}

#[test]
fn autostore_to_bag_icon_selects_first_valid_slot_in_that_bag() {
    let chest = test_item_template(38, ITEM_CLASS_ARMOR, 5, 0.0, 0.0, 12);
    let inventory = [
        CharacterInventoryItem {
            bag: INVENTORY_SLOT_BAG_0 as u32,
            slot: INVENTORY_SLOT_BAG_START,
            item: 77,
            item_template: RUST_VENDOR_BAG_ITEM,
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        },
        CharacterInventoryItem {
            bag: INVENTORY_SLOT_BAG_START as u32,
            slot: 0,
            item: 88,
            item_template: 6948,
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        },
        CharacterInventoryItem {
            bag: INVENTORY_SLOT_BAG_0 as u32,
            slot: 4,
            item: 99,
            item_template: 38,
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        },
    ];
    let bags = [EquippedBagInfo {
        slot: INVENTORY_SLOT_BAG_START,
        container_slots: 6,
        class: ITEM_CLASS_CONTAINER,
        subclass: ITEM_SUBCLASS_CONTAINER,
    }];

    assert_eq!(
        first_autostore_destination(
            &inventory,
            &inventory[2],
            &chest,
            &bags,
            INVENTORY_SLOT_BAG_START
        ),
        Some((INVENTORY_SLOT_BAG_START, 1))
    );
}

#[test]
fn autostore_to_bank_bag_icon_selects_first_valid_slot_in_that_bank_bag() {
    let chest = test_item_template(38, ITEM_CLASS_ARMOR, 5, 0.0, 0.0, 12);
    let inventory = [
        CharacterInventoryItem {
            bag: INVENTORY_SLOT_BAG_0 as u32,
            slot: BANK_SLOT_BAG_START,
            item: 77,
            item_template: RUST_VENDOR_BAG_ITEM,
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        },
        CharacterInventoryItem {
            bag: BANK_SLOT_BAG_START as u32,
            slot: 0,
            item: 88,
            item_template: 6948,
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        },
        CharacterInventoryItem {
            bag: INVENTORY_SLOT_BAG_0 as u32,
            slot: INVENTORY_SLOT_ITEM_START,
            item: 99,
            item_template: 38,
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        },
    ];
    let bank_bags = [EquippedBagInfo {
        slot: BANK_SLOT_BAG_START,
        container_slots: 6,
        class: ITEM_CLASS_CONTAINER,
        subclass: ITEM_SUBCLASS_CONTAINER,
    }];

    assert_eq!(
        first_bank_store_destination(
            &inventory,
            &inventory[2],
            &chest,
            &bank_bags,
            1,
            Some(BANK_SLOT_BAG_START)
        ),
        Some((BANK_SLOT_BAG_START, 1))
    );
}

#[test]
fn swap_to_bank_bag_icon_resolves_non_bag_item_to_bag_storage() {
    let chest = test_item_template(38, ITEM_CLASS_ARMOR, 5, 0.0, 0.0, 12);
    let inventory = [
        CharacterInventoryItem {
            bag: INVENTORY_SLOT_BAG_0 as u32,
            slot: BANK_SLOT_BAG_START,
            item: 77,
            item_template: RUST_VENDOR_BAG_ITEM,
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        },
        CharacterInventoryItem {
            bag: BANK_SLOT_BAG_START as u32,
            slot: 0,
            item: 88,
            item_template: 6948,
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        },
        CharacterInventoryItem {
            bag: INVENTORY_SLOT_BAG_0 as u32,
            slot: INVENTORY_SLOT_ITEM_START,
            item: 99,
            item_template: 38,
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        },
    ];
    let bank_bags = [EquippedBagInfo {
        slot: BANK_SLOT_BAG_START,
        container_slots: 6,
        class: ITEM_CLASS_CONTAINER,
        subclass: ITEM_SUBCLASS_CONTAINER,
    }];
    let request = InventoryMoveRequest {
        src_bag: INVENTORY_SLOT_BAG_0,
        src_slot: INVENTORY_SLOT_ITEM_START,
        dst_bag: INVENTORY_SLOT_BAG_0,
        dst_slot: BANK_SLOT_BAG_START,
    };

    assert_eq!(
        resolve_bag_icon_move_destination(
            &inventory,
            &inventory[2],
            &chest,
            &[],
            &bank_bags,
            1,
            &request
        ),
        Some(InventoryMoveRequest {
            src_bag: INVENTORY_SLOT_BAG_0,
            src_slot: INVENTORY_SLOT_ITEM_START,
            dst_bag: BANK_SLOT_BAG_START,
            dst_slot: 1,
        })
    );
}

#[test]
fn parses_backpack_inventory_destroy_packets() {
    let destroy = DestroyItemRequest::read(&[CLIENT_INVENTORY_SLOT_BAG_0, 24, 0, 0, 0, 0]).unwrap();
    assert_eq!(
        destroy,
        DestroyItemRequest {
            bag: INVENTORY_SLOT_BAG_0,
            slot: 24,
            count: 0,
        }
    );
    assert!(destroy.is_supported_destroy());

    let partial_stack =
        DestroyItemRequest::read(&[CLIENT_INVENTORY_SLOT_BAG_0, 24, 1, 0, 0, 0]).unwrap();
    assert!(partial_stack.is_supported_destroy());

    let equipped = DestroyItemRequest::read(&[CLIENT_INVENTORY_SLOT_BAG_0, 3, 0, 0, 0, 0]).unwrap();
    assert!(equipped.is_supported_destroy());

    let unsupported_bag = DestroyItemRequest::read(&[1, 24, 0, 0, 0, 0]).unwrap();
    assert!(!unsupported_bag.is_supported_destroy());

    let bag_slot = DestroyItemRequest::read(&[19, 0, 0, 0, 0, 0]).unwrap();
    assert!(bag_slot.is_supported_destroy());
}

#[test]
fn parses_inventory_split_packets() {
    let split = SplitItemRequest::read(&[CLIENT_INVENTORY_SLOT_BAG_0, 24, 19, 0, 2]).unwrap();
    assert_eq!(
        split,
        SplitItemRequest {
            src_bag: INVENTORY_SLOT_BAG_0,
            src_slot: 24,
            dst_bag: 19,
            dst_slot: 0,
            count: 2,
        }
    );
    assert!(split.is_supported_split());

    let zero_count = SplitItemRequest::read(&[CLIENT_INVENTORY_SLOT_BAG_0, 24, 19, 0, 0]).unwrap();
    assert!(!zero_count.is_supported_split());
}

#[test]
fn inventory_slot_update_body_clears_source_and_sets_destination() {
    let item = CharacterInventoryItem {
        bag: 0,
        slot: 24,
        item: 42,
        item_template: 6948,
        count: 1,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    };

    let body = build_inventory_slots_update_body(11, &[item], &[23, 24]).unwrap();
    let packed_guid_mask = body[6];
    let values_start = 4 + 1 + 1 + 1 + packed_guid_mask.count_ones() as usize;
    let values = decode_update_values(&body[values_start..]);
    let source_field = inventory_slot_update_field(23).unwrap();
    let destination_field = inventory_slot_update_field(24).unwrap();
    let guid = ObjectGuid::new(HighGuid::Item, 0, 42);

    assert_eq!(body[5], UPDATE_TYPE_VALUES);
    assert_eq!(values[source_field], Some(0));
    assert_eq!(values[source_field + 1], Some(0));
    assert_eq!(values[destination_field], Some(guid.raw() as u32));
    assert_eq!(
        values[destination_field + 1],
        Some((guid.raw() >> 32) as u32)
    );
}

#[test]
fn inventory_slot_update_body_updates_visible_equipment_slot() {
    let item = CharacterInventoryItem {
        bag: 0,
        slot: 3,
        item: 42,
        item_template: 38,
        count: 1,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    };

    let body = build_inventory_slots_update_body(11, &[item], &[3, 26]).unwrap();
    let packed_guid_mask = body[6];
    let values_start = 4 + 1 + 1 + 1 + packed_guid_mask.count_ones() as usize;
    let values = decode_update_values(&body[values_start..]);
    let equipment_field = inventory_slot_update_field(3).unwrap();
    let backpack_field = inventory_slot_update_field(26).unwrap();
    let guid = ObjectGuid::new(HighGuid::Item, 0, 42);

    assert_eq!(values[equipment_field], Some(guid.raw() as u32));
    assert_eq!(values[equipment_field + 1], Some((guid.raw() >> 32) as u32));
    assert_eq!(values[0x104 + 3 * 12], Some(38));
    assert_eq!(values[backpack_field], Some(0));
    assert_eq!(values[backpack_field + 1], Some(0));
}

#[test]
fn backpack_to_equipped_bag_move_updates_player_and_container_slots() {
    let character_guid = 11;
    let bag_guid = ObjectGuid::new(HighGuid::Item, 0, 77);
    let moved_guid = ObjectGuid::new(HighGuid::Item, 0, 99);
    let inventory = [
        CharacterInventoryItem {
            bag: 0,
            slot: 19,
            item: 77,
            item_template: RUST_VENDOR_BAG_ITEM,
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        },
        CharacterInventoryItem {
            bag: 19,
            slot: 3,
            item: 99,
            item_template: RUST_VENDOR_BAG_ITEM,
            count: 2,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        },
    ];
    let request = InventoryMoveRequest {
        src_bag: INVENTORY_SLOT_BAG_0,
        src_slot: 27,
        dst_bag: 19,
        dst_slot: 3,
    };

    let body = build_update_object_body(
        &build_inventory_move_update_blocks(character_guid, &inventory, &request).unwrap(),
    );
    assert_eq!(&body[0..4], &3u32.to_le_bytes());
    assert_eq!(body[4], 0);

    let mut block = &body[5..];
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let (player_values, rest) = decode_values_update_block(block, player_guid);
    block = rest;
    let source_field = inventory_slot_update_field(27).unwrap();
    assert_eq!(player_values[source_field], Some(0));
    assert_eq!(player_values[source_field + 1], Some(0));

    let (container_values, rest) = decode_values_update_block(block, bag_guid);
    block = rest;
    let container_slot_field = CONTAINER_FIELD_SLOT_1 + 3 * 2;
    assert_eq!(
        container_values[container_slot_field],
        Some(moved_guid.raw() as u32)
    );
    assert_eq!(
        container_values[container_slot_field + 1],
        Some((moved_guid.raw() >> 32) as u32)
    );

    let (moved_values, rest) = decode_values_update_block(block, moved_guid);
    assert!(rest.is_empty());
    assert_eq!(moved_values[0x008], Some(bag_guid.raw() as u32));
    assert_eq!(moved_values[0x009], Some((bag_guid.raw() >> 32) as u32));
}

#[test]
fn equipped_bag_to_backpack_move_updates_player_slot_and_clears_container_slot() {
    let character_guid = 11;
    let bag_guid = ObjectGuid::new(HighGuid::Item, 0, 77);
    let moved_guid = ObjectGuid::new(HighGuid::Item, 0, 99);
    let inventory = [
        CharacterInventoryItem {
            bag: 0,
            slot: 19,
            item: 77,
            item_template: RUST_VENDOR_BAG_ITEM,
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        },
        CharacterInventoryItem {
            bag: 0,
            slot: 27,
            item: 99,
            item_template: RUST_VENDOR_BAG_ITEM,
            count: 2,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        },
    ];
    let request = InventoryMoveRequest {
        src_bag: 19,
        src_slot: 3,
        dst_bag: INVENTORY_SLOT_BAG_0,
        dst_slot: 27,
    };

    let body = build_update_object_body(
        &build_inventory_move_update_blocks(character_guid, &inventory, &request).unwrap(),
    );
    assert_eq!(&body[0..4], &2u32.to_le_bytes());
    assert_eq!(body[4], 0);

    let mut block = &body[5..];
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let (player_values, rest) = decode_values_update_block(block, player_guid);
    block = rest;
    let destination_field = inventory_slot_update_field(27).unwrap();
    assert_eq!(
        player_values[destination_field],
        Some(moved_guid.raw() as u32)
    );
    assert_eq!(
        player_values[destination_field + 1],
        Some((moved_guid.raw() >> 32) as u32)
    );

    let (container_values, rest) = decode_values_update_block(block, bag_guid);
    assert!(rest.is_empty());
    let container_slot_field = CONTAINER_FIELD_SLOT_1 + 3 * 2;
    assert_eq!(container_values[container_slot_field], Some(0));
    assert_eq!(container_values[container_slot_field + 1], Some(0));
}

#[test]
fn equipped_bag_destroy_update_clears_container_slot() {
    let character_guid = 11;
    let bag_guid = ObjectGuid::new(HighGuid::Item, 0, 77);
    let inventory = [CharacterInventoryItem {
        bag: 0,
        slot: 19,
        item: 77,
        item_template: RUST_VENDOR_BAG_ITEM,
        count: 1,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    }];

    let body = build_update_object_body(
        &build_inventory_position_update_blocks(character_guid, &inventory, 19, 3).unwrap(),
    );

    assert_eq!(&body[0..4], &1u32.to_le_bytes());
    assert_eq!(body[4], 0);
    let (container_values, rest) = decode_values_update_block(&body[5..], bag_guid);
    assert!(rest.is_empty());
    let container_slot_field = CONTAINER_FIELD_SLOT_1 + 3 * 2;
    assert_eq!(container_values[container_slot_field], Some(0));
    assert_eq!(container_values[container_slot_field + 1], Some(0));
}

#[test]
fn backpack_stack_merge_update_clears_source_slot_and_updates_destination_count() {
    let character_guid = 11;
    let source_guid = ObjectGuid::new(HighGuid::Item, 0, 88);
    let destination_guid = ObjectGuid::new(HighGuid::Item, 0, 99);
    let inventory = [CharacterInventoryItem {
        bag: 0,
        slot: 26,
        item: 99,
        item_template: RUST_VENDOR_BAG_ITEM,
        count: 7,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    }];
    let mut blocks = build_inventory_position_update_blocks(
        character_guid,
        &inventory,
        INVENTORY_SLOT_BAG_0,
        27,
    )
    .unwrap();
    blocks.push(build_item_stack_count_update_block(99, 7).unwrap());

    let body = build_update_object_body(&blocks);
    assert_eq!(&body[0..4], &2u32.to_le_bytes());
    assert_eq!(body[4], 0);

    let mut block = &body[5..];
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let (player_values, rest) = decode_values_update_block(block, player_guid);
    block = rest;
    let source_field = inventory_slot_update_field(27).unwrap();
    assert_eq!(player_values[source_field], Some(0));
    assert_eq!(player_values[source_field + 1], Some(0));

    let (destination_values, rest) = decode_values_update_block(block, destination_guid);
    assert!(rest.is_empty());
    assert_eq!(destination_values[0x00E], Some(7));
    assert_ne!(
        player_values[source_field],
        Some(source_guid.raw() as u32),
        "source stack should not remain referenced after full merge"
    );
}

#[test]
fn equipped_bag_stack_merge_update_clears_container_slot_and_updates_destination_count() {
    let character_guid = 11;
    let bag_guid = ObjectGuid::new(HighGuid::Item, 0, 77);
    let destination_guid = ObjectGuid::new(HighGuid::Item, 0, 99);
    let inventory = [
        CharacterInventoryItem {
            bag: 0,
            slot: 19,
            item: 77,
            item_template: RUST_VENDOR_BAG_ITEM,
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        },
        CharacterInventoryItem {
            bag: 19,
            slot: 2,
            item: 99,
            item_template: RUST_VENDOR_BAG_ITEM,
            count: 7,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        },
    ];
    let mut blocks =
        build_inventory_position_update_blocks(character_guid, &inventory, 19, 4).unwrap();
    blocks.push(build_item_stack_count_update_block(99, 7).unwrap());

    let body = build_update_object_body(&blocks);
    assert_eq!(&body[0..4], &2u32.to_le_bytes());
    assert_eq!(body[4], 0);

    let mut block = &body[5..];
    let (container_values, rest) = decode_values_update_block(block, bag_guid);
    block = rest;
    let source_container_field = CONTAINER_FIELD_SLOT_1 + 4 * 2;
    assert_eq!(container_values[source_container_field], Some(0));
    assert_eq!(container_values[source_container_field + 1], Some(0));

    let (destination_values, rest) = decode_values_update_block(block, destination_guid);
    assert!(rest.is_empty());
    assert_eq!(destination_values[0x00E], Some(7));
}

#[test]
fn split_into_equipped_bag_update_body_contains_renderable_destination_stack() {
    let character_guid = 11;
    let owner_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let source_guid = ObjectGuid::new(HighGuid::Item, 0, 42);
    let bag_guid = ObjectGuid::new(HighGuid::Item, 0, 77);
    let destination_guid = ObjectGuid::new(HighGuid::Item, 0, 99);
    let inventory = [
        CharacterInventoryItem {
            bag: 0,
            slot: 19,
            item: 77,
            item_template: RUST_VENDOR_BAG_ITEM,
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        },
        CharacterInventoryItem {
            bag: 0,
            slot: 24,
            item: 42,
            item_template: RUST_VENDOR_BAG_ITEM,
            count: 4,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        },
        CharacterInventoryItem {
            bag: 19,
            slot: 1,
            item: 99,
            item_template: RUST_VENDOR_BAG_ITEM,
            count: 2,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        },
    ];
    let destination = &inventory[2];
    let mut blocks = vec![build_item_stack_count_update_block(42, 4).unwrap()];
    blocks.push(build_item_create_update_block(owner_guid, bag_guid, destination, None).unwrap());
    blocks
        .extend(build_inventory_position_update_blocks(character_guid, &inventory, 19, 1).unwrap());

    let body = build_update_object_body(&blocks);
    assert_eq!(&body[0..4], &4u32.to_le_bytes());
    assert_eq!(body[4], 0);

    let mut block = &body[5..];
    let (source_values, rest) = decode_values_update_block(block, source_guid);
    block = rest;
    assert_eq!(source_values[0x00E], Some(4));

    let (destination_values, rest) =
        decode_create_update_block(block, destination_guid, TYPEID_ITEM);
    block = rest;
    assert_eq!(
        destination_values[0x000],
        Some(destination_guid.raw() as u32)
    );
    assert_eq!(
        destination_values[0x001],
        Some((destination_guid.raw() >> 32) as u32)
    );
    assert_eq!(destination_values[0x002], Some(TYPEMASK_OBJECT_ITEM));
    assert_eq!(destination_values[0x003], Some(RUST_VENDOR_BAG_ITEM));
    assert_eq!(destination_values[0x006], Some(owner_guid.raw() as u32));
    assert_eq!(
        destination_values[0x007],
        Some((owner_guid.raw() >> 32) as u32)
    );
    assert_eq!(destination_values[0x008], Some(bag_guid.raw() as u32));
    assert_eq!(
        destination_values[0x009],
        Some((bag_guid.raw() >> 32) as u32)
    );
    assert_eq!(destination_values[0x00E], Some(2));

    let (container_values, rest) = decode_values_update_block(block, bag_guid);
    block = rest;
    let container_slot_field = CONTAINER_FIELD_SLOT_1 + 2;
    assert_eq!(
        container_values[container_slot_field],
        Some(destination_guid.raw() as u32)
    );
    assert_eq!(
        container_values[container_slot_field + 1],
        Some((destination_guid.raw() >> 32) as u32)
    );

    let (contained_values, rest) = decode_values_update_block(block, destination_guid);
    assert!(rest.is_empty());
    assert_eq!(contained_values[0x008], Some(bag_guid.raw() as u32));
    assert_eq!(contained_values[0x009], Some((bag_guid.raw() >> 32) as u32));
}

#[test]
fn builds_create_blocks_for_equipped_and_backpack_items() {
    let character = CharacterEnumEntry {
        guid: 11,
        name: "Tester".to_string(),
        race: 1,
        class: 1,
        gender: 0,
        player_bytes: 0,
        player_bytes2: 0,
        level: 1,
        xp: 0,
        zone: 12,
        map: 0,
        position_x: -8949.95,
        position_y: -132.493,
        position_z: 83.5312,
        orientation: 0.0,
        guildid: None,
        player_flags: 0,
        at_login: 0,
        money: 0,
        cinematic: 0,
        ammo_id: 0,
        health: 20,
        power1: 0,
        power2: 0,
        power3: 0,
        power4: 0,
        power5: 0,
        watched_faction: u32::MAX,
        explored_zones: None,
        pet_entry: None,
        pet_modelid: None,
        pet_level: None,
        equipment_cache: None,
    };
    let items = [
        CharacterInventoryItem {
            bag: 0,
            slot: 16,
            item: 40,
            item_template: 2362,
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 18,
        },
        CharacterInventoryItem {
            bag: 0,
            slot: 24,
            item: 41,
            item_template: 6948,
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        },
    ];

    let blocks = build_inventory_item_create_blocks(&character, &items, &HashMap::new()).unwrap();

    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0][0], UPDATE_TYPE_CREATE_OBJECT);
    assert_eq!(blocks[0][4], TYPEID_ITEM);
    assert_eq!(blocks[0][5], UPDATEFLAG_ALL);
    assert_eq!(blocks[1][0], UPDATE_TYPE_CREATE_OBJECT);
    assert_eq!(blocks[1][4], TYPEID_ITEM);
    assert_eq!(blocks[1][5], UPDATEFLAG_ALL);
}

#[test]
fn login_create_blocks_make_equipped_bags_openable_containers() {
    let character = test_character(1, 1);
    let bag_item = CharacterInventoryItem {
        bag: INVENTORY_SLOT_BAG_0 as u32,
        slot: INVENTORY_SLOT_BAG_START,
        item: 41,
        item_template: 5571,
        count: 1,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    };
    let contained_item = CharacterInventoryItem {
        bag: INVENTORY_SLOT_BAG_START as u32,
        slot: 2,
        item: 42,
        item_template: 117,
        count: 4,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    };
    let inventory = [bag_item, contained_item];
    let container_slots = HashMap::from([(41, 6)]);

    let blocks =
        build_inventory_item_create_blocks(&character, &inventory, &container_slots).unwrap();

    let bag_guid = ObjectGuid::new(HighGuid::Item, 0, 41);
    let contained_guid = ObjectGuid::new(HighGuid::Item, 0, 42);
    let (bag_values, _) = decode_create_update_block(&blocks[0], bag_guid, TYPEID_CONTAINER);
    assert_eq!(bag_values[CONTAINER_FIELD_NUM_SLOTS], Some(6));
    assert!(blocks.iter().any(|block| {
        if block[0] != UPDATE_TYPE_VALUES {
            return false;
        }
        let (values, _) = decode_values_update_block(block, bag_guid);
        let field = CONTAINER_FIELD_SLOT_1 + 2 * 2;
        values[field] == Some(contained_guid.raw() as u32)
            && values[field + 1] == Some((contained_guid.raw() >> 32) as u32)
    }));
    let (contained_values, _) = decode_create_update_block(&blocks[2], contained_guid, TYPEID_ITEM);
    assert_eq!(contained_values[0x008], Some(bag_guid.raw() as u32));
    assert_eq!(contained_values[0x009], Some((bag_guid.raw() >> 32) as u32));
}

#[test]
fn item_create_block_includes_random_property_enchantments() {
    let owner_guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let item_guid = ObjectGuid::new(HighGuid::Item, 0, 42);
    let mut enchantments = [0u32; ITEM_ENCHANTMENT_FIELD_COUNT];
    enchantments[PROP_ENCHANTMENT_SLOT_0 * MAX_ENCHANTMENT_OFFSET] = 141;
    enchantments[(PROP_ENCHANTMENT_SLOT_0 + 1) * MAX_ENCHANTMENT_OFFSET] = 142;
    let item = CharacterInventoryItem {
        bag: INVENTORY_SLOT_BAG_0 as u32,
        slot: INVENTORY_SLOT_ITEM_START,
        item: 42,
        item_template: 11980,
        count: 1,
        random_property_id: 1373,
        charges: String::new(),
        enchantments: enchantments
            .into_iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join(" "),
        durability: 0,
    };

    let block = build_item_create_update_block(owner_guid, owner_guid, &item, None).unwrap();
    let (values, rest) = decode_create_update_block(&block, item_guid, TYPEID_ITEM);

    assert!(rest.is_empty());
    assert_eq!(values[ITEM_FIELD_RANDOM_PROPERTIES_ID], Some(1373));
    assert_eq!(
        values[ITEM_FIELD_ENCHANTMENT + PROP_ENCHANTMENT_SLOT_0 * MAX_ENCHANTMENT_OFFSET],
        Some(141)
    );
    assert_eq!(
        values[ITEM_FIELD_ENCHANTMENT + (PROP_ENCHANTMENT_SLOT_0 + 1) * MAX_ENCHANTMENT_OFFSET],
        Some(142)
    );
}

#[test]
fn item_create_block_includes_instance_spell_charges() {
    let owner_guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let item_guid = ObjectGuid::new(HighGuid::Item, 0, 42);
    let item = CharacterInventoryItem {
        bag: INVENTORY_SLOT_BAG_0 as u32,
        slot: INVENTORY_SLOT_ITEM_START,
        item: 42,
        item_template: 6948,
        count: 1,
        random_property_id: 0,
        charges: "-1 0 3 0 0".to_string(),
        enchantments: String::new(),
        durability: 0,
    };

    let block = build_item_create_update_block(owner_guid, owner_guid, &item, None).unwrap();
    let (values, rest) = decode_create_update_block(&block, item_guid, TYPEID_ITEM);

    assert!(rest.is_empty());
    assert_eq!(values[ITEM_FIELD_SPELL_CHARGES], Some(u32::MAX));
    assert_eq!(values[ITEM_FIELD_SPELL_CHARGES + 1], None);
    assert_eq!(values[ITEM_FIELD_SPELL_CHARGES + 2], Some(3));
}

#[test]
fn item_create_block_for_looted_bag_is_container_immediately() {
    let owner_guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let item_guid = ObjectGuid::new(HighGuid::Item, 0, 42);
    let item = CharacterInventoryItem {
        bag: INVENTORY_SLOT_BAG_0 as u32,
        slot: INVENTORY_SLOT_ITEM_START,
        item: 42,
        item_template: RUST_VENDOR_BAG_ITEM,
        count: 1,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    };

    let block = build_item_create_update_block(owner_guid, owner_guid, &item, Some(6)).unwrap();
    let (values, rest) = decode_create_update_block(&block, item_guid, TYPEID_CONTAINER);

    assert!(rest.is_empty());
    assert_eq!(values[0x002], Some(TYPEMASK_OBJECT_CONTAINER));
    assert_eq!(values[CONTAINER_FIELD_NUM_SLOTS], Some(6));
}

#[test]
fn item_random_properties_dbc_parser_reads_enchantment_slots() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"WDBC");
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&(16u32 * 4).to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    for field in [1373u32, 0, 141, 142, 143, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] {
        bytes.extend_from_slice(&field.to_le_bytes());
    }
    bytes.push(0);

    let properties = parse_item_random_properties(&bytes);

    assert_eq!(
        properties.get(&1373).copied(),
        Some(ItemRandomPropertyEntry {
            id: 1373,
            enchant_ids: [141, 142, 143],
        })
    );
}

#[test]
fn random_property_roll_matches_cmangos_cumulative_chance() {
    let rolls = [
        wow_db::ItemRandomPropertyRoll {
            enchantment_id: 10,
            chance: 25.0,
        },
        wow_db::ItemRandomPropertyRoll {
            enchantment_id: 20,
            chance: 25.0,
        },
        wow_db::ItemRandomPropertyRoll {
            enchantment_id: 30,
            chance: 50.0,
        },
    ];

    assert_eq!(roll_item_random_property_id_for_roll(&rolls, 0.0), Some(10));
    assert_eq!(
        roll_item_random_property_id_for_roll(&rolls, 24.99),
        Some(10)
    );
    assert_eq!(
        roll_item_random_property_id_for_roll(&rolls, 25.0),
        Some(20)
    );
    assert_eq!(
        roll_item_random_property_id_for_roll(&rolls, 50.0),
        Some(30)
    );
}

#[test]
fn starter_item_visuals_cover_human_warrior_equipment() {
    assert_eq!(
        starter_item_visual(25),
        Some(StarterItemVisual {
            display_id: 1542,
            inventory_type: 21
        })
    );
    assert_eq!(
        starter_item_visual(2362),
        Some(StarterItemVisual {
            display_id: 18730,
            inventory_type: 14
        })
    );
}

#[test]
fn maps_classic_race_gender_display_ids() {
    let mut character = CharacterEnumEntry {
        guid: 7,
        name: "Ada".to_string(),
        race: 1,
        class: 1,
        gender: 0,
        player_bytes: 0,
        player_bytes2: 0,
        level: 1,
        xp: 0,
        zone: 12,
        map: 0,
        position_x: -8949.95,
        position_y: -132.493,
        position_z: 83.5312,
        orientation: 0.0,
        guildid: None,
        player_flags: 0,
        at_login: 0,
        money: 0,
        cinematic: 0,
        ammo_id: 0,
        health: 20,
        power1: 0,
        power2: 0,
        power3: 0,
        power4: 0,
        power5: 0,
        watched_faction: u32::MAX,
        explored_zones: None,
        pet_entry: None,
        pet_modelid: None,
        pet_level: None,
        equipment_cache: None,
    };

    for (race, male_display, female_display) in [
        (1, 49, 50),
        (2, 51, 52),
        (3, 53, 54),
        (4, 55, 56),
        (5, 57, 58),
        (6, 59, 60),
        (7, 1563, 1564),
        (8, 1478, 1479),
    ] {
        character.race = race;
        character.gender = 0;
        assert_eq!(display_id_for_character(&character), male_display);
        character.gender = 1;
        assert_eq!(display_id_for_character(&character), female_display);
    }

    for (race, male_display, female_display) in [
        (1u8, 49u32, 50u32),
        (2, 51, 52),
        (3, 53, 54),
        (4, 55, 56),
        (5, 57, 58),
        (6, 59, 60),
        (7, 1563, 1564),
        (8, 1478, 1479),
    ] {
        character.race = race;

        for (gender, display_id) in [(0u8, male_display), (1u8, female_display)] {
            character.gender = gender;
            let guid = ObjectGuid::new(HighGuid::Player, 0, 7);
            let mut body = Vec::new();

            write_minimal_player_update_values(
                &mut body,
                guid,
                &character,
                &[],
                &PlayerWorldStats {
                    base_health: 20,
                    base_mana: 0,
                    stats: [23, 20, 22, 20, 21],
                    next_level_xp: 400,
                },
                &PlayerWorldStats {
                    base_health: 20,
                    base_mana: 0,
                    stats: [23, 20, 22, 20, 21],
                    next_level_xp: 400,
                },
                &[],
                &std::collections::HashMap::new(),
                &[],
                None,
                &[],
            )
            .unwrap();

            let values = decode_update_values(&body);
            assert_eq!(values[UNIT_FIELD_DISPLAYID], Some(display_id));
            assert_eq!(values[UNIT_FIELD_NATIVEDISPLAYID], Some(display_id));
        }
    }
}

#[test]
fn parses_basic_movement_info() {
    let mut body = Vec::new();
    body.extend_from_slice(&0x0000_0001u32.to_le_bytes());
    body.extend_from_slice(&1234u32.to_le_bytes());
    body.extend_from_slice(&1.25f32.to_le_bytes());
    body.extend_from_slice(&2.5f32.to_le_bytes());
    body.extend_from_slice(&3.75f32.to_le_bytes());
    body.extend_from_slice(&1.0f32.to_le_bytes());
    body.extend_from_slice(&456u32.to_le_bytes());

    let movement = MovementInfo::read(&body).unwrap();

    assert_eq!(movement.flags, 1);
    assert_eq!(movement.client_time, 1234);
    assert_eq!(movement.position.x, 1.25);
    assert_eq!(movement.position.y, 2.5);
    assert_eq!(movement.position.z, 3.75);
    assert_eq!(movement.position.orientation, 1.0);
    assert_eq!(movement.fall_time, 456);
}

#[test]
fn parses_jump_movement_info() {
    let mut body = Vec::new();
    body.extend_from_slice(&MOVEFLAG_JUMPING.to_le_bytes());
    body.extend_from_slice(&1234u32.to_le_bytes());
    body.extend_from_slice(&1.25f32.to_le_bytes());
    body.extend_from_slice(&2.5f32.to_le_bytes());
    body.extend_from_slice(&3.75f32.to_le_bytes());
    body.extend_from_slice(&1.0f32.to_le_bytes());
    body.extend_from_slice(&456u32.to_le_bytes());
    body.extend_from_slice(&7.0f32.to_le_bytes());
    body.extend_from_slice(&0.0f32.to_le_bytes());
    body.extend_from_slice(&1.0f32.to_le_bytes());
    body.extend_from_slice(&4.5f32.to_le_bytes());

    let movement = MovementInfo::read(&body).unwrap();

    assert_eq!(movement.flags, MOVEFLAG_JUMPING);
    assert_eq!(movement.fall_time, 456);
    assert_eq!(movement.position.z, 3.75);
    assert_eq!(
        movement.jump,
        JumpInfo {
            z_speed: 7.0,
            cos_angle: 0.0,
            sin_angle: 1.0,
            xy_speed: 4.5,
        }
    );
}

#[test]
fn movement_info_rejects_truncated_payload() {
    let err = MovementInfo::read(&[0; 8]).unwrap_err().to_string();
    assert!(err.contains("movement packet truncated"));
}

#[test]
fn active_mover_rejects_truncated_payload() {
    let err = wow_proto::SetActiveMoverRequest::read(&mut &[0; 4][..])
        .unwrap_err()
        .to_string();
    assert!(err.contains("CMSG_SET_ACTIVE_MOVER payload too short"));
}

#[test]
fn active_mover_accepts_matching_player_guid() {
    let guid = 77u32;
    let session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid,
                name: "Mover".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position: WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0),
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    let mover_guid = ObjectGuid::new(HighGuid::Player, 0, guid).raw();

    let result = handle_set_active_mover(
        wow_proto::SetActiveMoverRequest {
            raw_guid: mover_guid,
        },
        &session,
    );

    assert!(result.is_ok());
}

#[test]
fn active_mover_mismatch_is_non_fatal() {
    let session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 77,
                name: "Mover".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position: WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0),
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    let mismatched_mover_guid = ObjectGuid::new(HighGuid::Player, 0, 99).raw();

    let result = handle_set_active_mover(
        wow_proto::SetActiveMoverRequest {
            raw_guid: mismatched_mover_guid,
        },
        &session,
    );

    assert!(result.is_ok());
}

#[test]
fn query_next_mail_time_matches_cmangos_shape() {
    assert_eq!(build_query_next_mail_time_body(true), 0.0f32.to_le_bytes());
    assert_eq!(
        build_query_next_mail_time_body(false),
        (-86400.0f32).to_le_bytes()
    );
}

#[test]
fn parses_basic_chat_message() {
    let mut body = Vec::new();
    body.extend_from_slice(&CHAT_MSG_SAY.to_le_bytes());
    body.extend_from_slice(&7u32.to_le_bytes());
    write_c_string(&mut body, "hello checkpoint");

    let chat = ChatMessage::read(&body).unwrap();

    assert_eq!(
        chat,
        ChatMessage {
            chat_type: CHAT_MSG_SAY,
            language: 7,
            message: "hello checkpoint".to_string()
        }
    );
}

#[test]
fn message_chat_body_matches_cmangos_say_shape() {
    let character = ActiveCharacter {
        guid: 7,
        name: "Ada".to_string(),
        race: 1,
        class: 1,
        level: 1,
        xp: 0,
        position: WorldPosition::new(0, 1.0, 2.0, 3.0, 4.0),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
        jump: JumpInfo::default(),
    };
    let body = build_message_chat_body(CHAT_MSG_SAY, 7, "hello", &character);
    let guid = ObjectGuid::new(HighGuid::Player, 0, 7).raw();

    assert_eq!(body[0], CHAT_MSG_SAY as u8);
    assert_eq!(&body[1..5], &7u32.to_le_bytes());
    assert_eq!(&body[5..13], &guid.to_le_bytes());
    assert_eq!(&body[13..21], &guid.to_le_bytes());
    assert_eq!(&body[21..25], &6u32.to_le_bytes());
    assert_eq!(&body[25..31], b"hello\0");
    assert_eq!(body[31], CHAT_TAG_NONE);
}

#[test]
fn parses_gm_dot_commands_for_creature_spawn_and_die() {
    assert_eq!(
        parse_gm_dot_command(".gm on"),
        Some(Ok(GmDotCommand::Gm(Some(true))))
    );
    assert_eq!(
        parse_gm_dot_command(".gm off"),
        Some(Ok(GmDotCommand::Gm(Some(false))))
    );
    assert_eq!(
        parse_gm_dot_command(".npc add #6"),
        Some(Ok(GmDotCommand::NpcAdd(6)))
    );
    assert_eq!(
        parse_gm_dot_command(".npc add |Hcreature_entry:94|h[Defias Cutpurse]|h"),
        Some(Ok(GmDotCommand::NpcAdd(94)))
    );
    assert_eq!(
        parse_gm_dot_command(".npc delete"),
        Some(Ok(GmDotCommand::NpcDelete(None)))
    );
    assert_eq!(
        parse_gm_dot_command(".npc delete #123"),
        Some(Ok(GmDotCommand::NpcDelete(Some(123))))
    );
    assert_eq!(parse_gm_dot_command(".die"), Some(Ok(GmDotCommand::Die)));
    assert_eq!(
        parse_gm_dot_command(".levelup"),
        Some(Ok(GmDotCommand::LevelUp(1)))
    );
    assert_eq!(
        parse_gm_dot_command(".levelup 5"),
        Some(Ok(GmDotCommand::LevelUp(5)))
    );
    assert_eq!(
        parse_gm_dot_command(".LEVELUP 5"),
        Some(Ok(GmDotCommand::LevelUp(5)))
    );
    assert_eq!(
        parse_gm_dot_command(".levelup +5"),
        Some(Ok(GmDotCommand::LevelUp(5)))
    );
    assert_eq!(
        parse_gm_dot_command(".levelup -2"),
        Some(Ok(GmDotCommand::LevelUp(-2)))
    );
    assert_eq!(
        parse_gm_dot_command(".level +10"),
        Some(Ok(GmDotCommand::LevelUp(10)))
    );
    assert_eq!(
        parse_gm_dot_command(".level 10"),
        Some(Ok(GmDotCommand::LevelUp(10)))
    );
    assert_eq!(
        parse_gm_dot_command(".character level 40"),
        Some(Ok(GmDotCommand::LevelSet(40)))
    );
    assert_eq!(
        parse_gm_dot_command(".character level 999"),
        Some(Ok(GmDotCommand::LevelSet(DEFAULT_MAX_PLAYER_LEVEL)))
    );
    assert_eq!(
        parse_gm_dot_command(".go 1 2"),
        Some(Ok(GmDotCommand::Go(GmGoDestination::Coordinates {
            x: 1.0,
            y: 2.0,
            z: None,
            map_id: None,
        })))
    );
    assert_eq!(
        parse_gm_dot_command(".go 1 2 3 0"),
        Some(Ok(GmDotCommand::Go(GmGoDestination::Coordinates {
            x: 1.0,
            y: 2.0,
            z: Some(3.0),
            map_id: Some(0),
        })))
    );
    assert_eq!(
        parse_gm_dot_command(".go Northshire Abbey"),
        Some(Ok(GmDotCommand::Go(GmGoDestination::Waypoint(
            "Northshire Abbey".to_string()
        ))))
    );
    assert_eq!(
        parse_gm_dot_command(".additem 929"),
        Some(Ok(GmDotCommand::AddItem {
            item: 929,
            count: 1
        }))
    );
    assert_eq!(
        parse_gm_dot_command(".additem #929 #5"),
        Some(Ok(GmDotCommand::AddItem {
            item: 929,
            count: 5
        }))
    );
    assert_eq!(
        parse_gm_dot_command(".additem |Hitem:929:0:0:0|h[Healing Potion]|h 5"),
        Some(Ok(GmDotCommand::AddItem {
            item: 929,
            count: 5
        }))
    );
    assert_eq!(
        parse_gm_dot_command(".modify speed 5"),
        Some(Ok(GmDotCommand::ModifySpeed(5.0)))
    );
    assert_eq!(
        parse_gm_dot_command(".modify money 12345"),
        Some(Ok(GmDotCommand::ModifyMoney(12345)))
    );
    assert_eq!(
        parse_gm_dot_command(".modify money #54321"),
        Some(Ok(GmDotCommand::ModifyMoney(54321)))
    );
    assert!(find_gm_waypoint("Northshire Abbey").is_some());
    assert_eq!(gm_relative_level(58, 5), DEFAULT_MAX_PLAYER_LEVEL);
    assert_eq!(gm_relative_level(3, -10), 1);
}

#[test]
fn malformed_gm_npc_add_returns_syntax_error() {
    assert_eq!(
        parse_gm_dot_command(".npc add"),
        Some(Err("Syntax: .npc add #creatureid".to_string()))
    );
    assert_eq!(
        parse_gm_dot_command(".additem"),
        Some(Err("Syntax: .additem #itemid [#count]".to_string()))
    );
    assert_eq!(
        parse_gm_dot_command(".additem 929 0"),
        Some(Err("Syntax: .additem #itemid [#count]".to_string()))
    );
}

#[tokio::test]
async fn party_manager_invite_accept_leave_updates_membership() {
    let parties = PartyManager::default();
    let invite = parties
        .invite(
            PartyMember {
                guid: 1,
                name: "Leader".to_string(),
            },
            PartyMember {
                guid: 2,
                name: "Member".to_string(),
            },
            SessionId(2),
        )
        .await;
    assert_eq!(invite.result, PartyResult::Ok);
    assert_eq!(invite.invitee_session, Some(SessionId(2)));
    assert_eq!(
        invite.invite_packet.as_ref().map(|packet| packet.opcode),
        Some(WorldOpcode::SmsgGroupInvite as u16)
    );

    let accepted = parties.accept(2).await;
    assert_eq!(accepted.result, PartyResult::Ok);
    assert_eq!(accepted.packets.len(), 2);
    assert!(parties.same_party(1, 2).await);
    assert_eq!(parties.party_members(1).await.len(), 2);
    assert!(matches!(
        parties.loot_owner_for(1).await,
        CreatureLootOwner::Party(_)
    ));
    assert!(parties.membership(1).await.is_some());

    let left = parties.leave(2).await;
    assert_eq!(left.result, PartyResult::Ok);
    assert!(!parties.same_party(1, 2).await);
    assert!(parties.membership(1).await.is_none());
    assert!(parties.membership(2).await.is_none());
}

#[tokio::test]
async fn party_manager_leader_can_kick_and_transfer_leadership() {
    let parties = PartyManager::default();
    parties
        .invite(
            PartyMember {
                guid: 1,
                name: "Leader".to_string(),
            },
            PartyMember {
                guid: 2,
                name: "Member".to_string(),
            },
            SessionId(2),
        )
        .await;
    parties.accept(2).await;
    parties
        .invite(
            PartyMember {
                guid: 1,
                name: "Leader".to_string(),
            },
            PartyMember {
                guid: 3,
                name: "Third".to_string(),
            },
            SessionId(3),
        )
        .await;
    parties.accept(3).await;

    let transfer = parties.set_leader(1, 2).await;
    assert_eq!(transfer.result, PartyResult::Ok);
    assert!(transfer.packets.iter().any(
        |(guid, packet)| *guid == 1 && packet.opcode == WorldOpcode::SmsgGroupSetLeader as u16
    ));
    assert!(transfer.packets.iter().any(
        |(guid, packet)| *guid == 2 && packet.opcode == WorldOpcode::SmsgGroupSetLeader as u16
    ));
    assert_eq!(parties.membership(1).await.map(|m| m.leader), Some(2));
    let kicked = parties.kick(2, 3).await;
    assert_eq!(kicked.result, PartyResult::Ok);
    assert!(
        kicked
            .packets
            .iter()
            .any(|(guid, packet)| *guid == 3
                && packet.opcode == WorldOpcode::SmsgGroupUninvite as u16)
    );
    assert!(kicked.packets.iter().any(|(guid, packet)| {
        *guid == 3
            && packet.opcode == WorldOpcode::SmsgGroupList as u16
            && packet.body == build_empty_group_list_body()
    }));
    assert!(kicked
        .packets
        .iter()
        .any(|(guid, packet)| *guid == 1 && packet.opcode == WorldOpcode::SmsgGroupList as u16));
    assert!(kicked
        .packets
        .iter()
        .any(|(guid, packet)| *guid == 2 && packet.opcode == WorldOpcode::SmsgGroupList as u16));
    assert!(parties.same_party(1, 2).await);
    assert!(!parties.same_party(1, 3).await);
}

#[test]
fn empty_group_list_body_matches_cmangos_three_zero_guids() {
    let body = build_empty_group_list_body();
    assert_eq!(body.len(), 24);
    assert!(body.iter().all(|byte| *byte == 0));
}

#[tokio::test]
async fn party_manager_leader_updates_loot_method() {
    let parties = PartyManager::default();
    parties
        .invite(
            PartyMember {
                guid: 1,
                name: "Leader".to_string(),
            },
            PartyMember {
                guid: 2,
                name: "Member".to_string(),
            },
            SessionId(2),
        )
        .await;
    parties.accept(2).await;

    let outcome = parties.set_loot_method(1, 2, 2, 3).await;
    assert_eq!(outcome.result, PartyResult::Ok);
    assert_eq!(outcome.packets.len(), 2);
    for (_, packet) in outcome.packets {
        assert_eq!(packet.opcode, WorldOpcode::SmsgGroupList as u16);
        let tail = &packet.body[packet.body.len() - 11..];
        assert_eq!(tail[0], 2);
        assert_eq!(
            u64::from_le_bytes(tail[1..9].try_into().unwrap()),
            ObjectGuid::new(HighGuid::Player, 0, 2).raw()
        );
        assert_eq!(tail[9], 3);
    }
}

#[tokio::test]
async fn party_group_loot_roll_need_beats_greed_and_awards_once() {
    let parties = PartyManager::default();
    parties
        .invite(
            PartyMember {
                guid: 1,
                name: "Leader".to_string(),
            },
            PartyMember {
                guid: 2,
                name: "Member".to_string(),
            },
            SessionId(2),
        )
        .await;
    parties.accept(2).await;
    assert_eq!(
        parties.set_loot_method(1, 3, 0, 2).await.result,
        PartyResult::Ok
    );

    let loot_guid = ObjectGuid::new(HighGuid::Unit, 0, 99);
    let loot = DbCreatureLootRuntime {
        slot: 4,
        item: 25,
        count: 1,
        display_id: 100,
        quality: 2,
        free_for_all: false,
        quest_drop: false,
    };
    let start = parties
        .start_loot_roll(1, 0, loot_guid, loot.slot, loot.clone())
        .await
        .expect("group loot starts a roll for party items");
    assert_eq!(start.packets.len(), 2);
    assert!(start
        .packets
        .iter()
        .all(|(_, packet)| packet.opcode == WorldOpcode::SmsgLootStartRoll as u16));

    let first = parties
        .record_loot_roll_vote(1, loot_guid, loot.slot, LootRollVote::Greed)
        .await
        .expect("first vote is recorded");
    assert!(first.winner.is_none());
    assert_eq!(first.packets.len(), 2);
    assert!(first
        .packets
        .iter()
        .all(|(_, packet)| packet.opcode == WorldOpcode::SmsgLootRoll as u16));

    let finished = parties
        .record_loot_roll_vote(2, loot_guid, loot.slot, LootRollVote::Need)
        .await
        .expect("final vote resolves roll");
    assert_eq!(finished.winner, Some(2));
    assert_eq!(finished.loot.as_ref().map(|item| item.item), Some(25));
    assert!(finished.packets.iter().any(|(_, packet)| {
        packet.opcode == WorldOpcode::SmsgLootRoll as u16
            && packet.body[32] == 0
            && packet.body[33] == 0
    }));
    assert!(finished.packets.iter().any(|(_, packet)| {
        packet.opcode == WorldOpcode::SmsgLootRoll as u16
            && (1..=100).contains(&packet.body[32])
            && packet.body[33] == LootRollVote::Need as u8
    }));
    assert!(finished
        .packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgLootRollWon as u16));
    assert!(
        parties
            .record_loot_roll_vote(1, loot_guid, loot.slot, LootRollVote::Need)
            .await
            .is_none(),
        "finished rolls cannot be voted again"
    );
}

#[tokio::test]
async fn party_group_loot_roll_timeout_passes_missing_voters() {
    let parties = PartyManager::default();
    parties
        .invite(
            PartyMember {
                guid: 1,
                name: "Leader".to_string(),
            },
            PartyMember {
                guid: 2,
                name: "Member".to_string(),
            },
            SessionId(2),
        )
        .await;
    parties.accept(2).await;
    assert_eq!(
        parties.set_loot_method(1, 3, 0, 2).await.result,
        PartyResult::Ok
    );

    let loot_guid = ObjectGuid::new(HighGuid::Unit, 0, 101);
    let loot = DbCreatureLootRuntime {
        slot: 1,
        item: 25,
        count: 1,
        display_id: 100,
        quality: 2,
        free_for_all: false,
        quest_drop: false,
    };
    parties
        .start_loot_roll(1, 7, loot_guid, loot.slot, loot.clone())
        .await
        .expect("roll starts");
    parties
        .record_loot_roll_vote(1, loot_guid, loot.slot, LootRollVote::Greed)
        .await
        .expect("first voter greeds");

    let outcomes = parties
        .expire_loot_rolls(Instant::now() + GROUP_LOOT_ROLL_TIMEOUT + Duration::from_millis(1))
        .await;

    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].map_id, 7);
    assert_eq!(outcomes[0].winner, Some(1));
    assert!(outcomes[0].packets.iter().any(|(_, packet)| {
        packet.opcode == WorldOpcode::SmsgLootRoll as u16
            && u64::from_le_bytes(packet.body[12..20].try_into().unwrap())
                == ObjectGuid::new(HighGuid::Player, 0, 2).raw()
            && packet.body[32] == 128
            && packet.body[33] == 128
    }));
    assert!(outcomes[0]
        .packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgLootRollWon as u16));
}

#[test]
fn group_loot_response_hides_under_threshold_items_from_non_current_looter() {
    let mut creature = DbCreatureRuntime::new(test_creature_spawn(6));
    creature.begin_corpse(Instant::now(), 1_000);
    creature.loot_current_looter = Some(1);
    creature.loot_items = vec![DbCreatureLootRuntime {
        slot: 0,
        item: 117,
        count: 1,
        display_id: 117,
        quality: 1,
        free_for_all: false,
        quest_drop: false,
    }];
    let guid = creature.guid();

    let current =
        build_db_creature_loot_response_body_for_player(guid, &creature, Some((3, 2, 0)), 1);
    let other =
        build_db_creature_loot_response_body_for_player(guid, &creature, Some((3, 2, 0)), 2);

    assert_eq!(
        current[13], 1,
        "current looter sees the under-threshold item"
    );
    assert_eq!(current[35], LOOT_SLOT_NORMAL);
    assert_eq!(
        other[13], 0,
        "other party member does not see normal trash loot"
    );
}

#[test]
fn group_loot_response_releases_current_looter_passed_trash_to_party_looters() {
    let mut creature = DbCreatureRuntime::new(test_creature_spawn(6));
    creature.begin_corpse(Instant::now(), 1_000);
    creature.loot_current_looter = Some(1);
    creature.loot_items = vec![DbCreatureLootRuntime {
        slot: 0,
        item: 117,
        count: 1,
        display_id: 117,
        quality: 1,
        free_for_all: false,
        quest_drop: false,
    }];
    creature.loot_current_looter_pass_slots.insert(0);

    let other = build_db_creature_loot_response_body_for_player(
        creature.guid(),
        &creature,
        Some((3, 2, 0)),
        2,
    );

    assert_eq!(other[13], 1);
    assert_eq!(other[35], LOOT_SLOT_NORMAL);
    assert!(can_autostore_shared_creature_loot(
        2,
        &creature,
        &creature.loot_items[0]
    ));
}

#[test]
fn group_loot_response_shows_green_items_as_view_only_until_roll_resolves() {
    let mut creature = DbCreatureRuntime::new(test_creature_spawn(6));
    creature.begin_corpse(Instant::now(), 1_000);
    creature.loot_current_looter = Some(1);
    creature.loot_items = vec![DbCreatureLootRuntime {
        slot: 0,
        item: 25,
        count: 1,
        display_id: 100,
        quality: 2,
        free_for_all: false,
        quest_drop: false,
    }];
    let body = build_db_creature_loot_response_body_for_player(
        creature.guid(),
        &creature,
        Some((3, 2, 0)),
        2,
    );

    assert_eq!(body[13], 1);
    assert_eq!(
        body[35], 1,
        "green group loot is visible but not directly lootable"
    );
}

#[test]
fn group_loot_response_releases_all_passed_green_to_party_looters() {
    let mut creature = DbCreatureRuntime::new(test_creature_spawn(6));
    creature.begin_corpse(Instant::now(), 1_000);
    creature.loot_current_looter = Some(1);
    creature.loot_items = vec![DbCreatureLootRuntime {
        slot: 0,
        item: 25,
        count: 1,
        display_id: 100,
        quality: 2,
        free_for_all: false,
        quest_drop: false,
    }];
    creature.loot_roll_released_slots.insert(0);

    let current = build_db_creature_loot_response_body_for_player(
        creature.guid(),
        &creature,
        Some((3, 2, 0)),
        1,
    );
    let other = build_db_creature_loot_response_body_for_player(
        creature.guid(),
        &creature,
        Some((3, 2, 0)),
        2,
    );

    assert_eq!(current[13], 1);
    assert_eq!(current[35], LOOT_SLOT_NORMAL);
    assert_eq!(other[13], 1);
    assert_eq!(other[35], LOOT_SLOT_NORMAL);
}

#[test]
fn corpse_loot_method_snapshot_controls_group_loot_autostore() {
    let mut creature = DbCreatureRuntime::new(test_creature_spawn(6));
    creature.begin_corpse(Instant::now(), 1_000);
    creature.loot_current_looter = Some(1);
    creature.loot_method = Some(CreatureLootMethod {
        method: 3,
        threshold: 2,
        master_looter: 0,
    });
    let loot = DbCreatureLootRuntime {
        slot: 0,
        item: 25,
        count: 1,
        display_id: 100,
        quality: 2,
        free_for_all: false,
        quest_drop: false,
    };

    assert!(should_use_group_loot_roll(&creature, &loot));
    assert!(
        !can_autostore_shared_creature_loot(1, &creature, &loot),
        "green group-loot corpse item remains roll-blocked even if party rules later change"
    );

    creature.loot_roll_released_slots.insert(loot.slot);
    assert!(can_autostore_shared_creature_loot(2, &creature, &loot));
}

#[test]
fn corpse_loot_method_snapshot_controls_master_loot_autostore() {
    let mut creature = DbCreatureRuntime::new(test_creature_spawn(6));
    creature.begin_corpse(Instant::now(), 1_000);
    creature.loot_current_looter = Some(1);
    creature.loot_method = Some(CreatureLootMethod {
        method: 2,
        threshold: 2,
        master_looter: 2,
    });
    let loot = DbCreatureLootRuntime {
        slot: 0,
        item: 25,
        count: 1,
        display_id: 100,
        quality: 2,
        free_for_all: false,
        quest_drop: false,
    };

    assert!(should_block_master_loot(&creature, 1, &loot));
    assert!(!should_block_master_loot(&creature, 2, &loot));
}

#[test]
fn master_loot_viewer_can_reopen_after_gold_is_taken() {
    let mut creature = DbCreatureRuntime::new(test_creature_spawn(6));
    creature.begin_corpse(Instant::now(), 1_000);
    creature.loot_items_generated = true;
    creature.loot_money_available = false;
    creature.loot_owner = Some(CreatureLootOwner::Party(77));
    creature.loot_allowed_players = HashSet::from([1, 2]);
    creature.loot_current_looter = Some(2);
    creature.loot_method = Some(CreatureLootMethod {
        method: 2,
        threshold: 2,
        master_looter: 1,
    });
    creature.loot_items = vec![DbCreatureLootRuntime {
        slot: 0,
        item: 25,
        count: 1,
        display_id: 100,
        quality: 2,
        free_for_all: false,
        quest_drop: false,
    }];

    assert!(
        creature.can_loot_for_player(Some(2)),
        "non-master eligible party members can reopen to view master-loot items after money is gone"
    );
    assert_eq!(
        creature.dynamic_flags_for_player(Some(2)),
        UNIT_DYNFLAG_LOOTABLE
    );
    let body = build_db_creature_loot_response_body_for_player(
        creature.guid(),
        &creature,
        db_creature_loot_method_tuple(creature.loot_method),
        2,
    );
    assert_eq!(body[13], 1);
    assert_eq!(body[35], 1, "non-master sees the item as view-only");
}

#[test]
fn creature_loot_money_share_splits_by_corpse_owner_set() {
    let mut creature = DbCreatureRuntime::new(test_creature_spawn(6));
    creature.begin_corpse(Instant::now(), 1_000);
    creature.loot_method = Some(CreatureLootMethod {
        method: 3,
        threshold: 2,
        master_looter: 0,
    });
    creature.loot_allowed_players = HashSet::from([1, 2, 3]);

    assert_eq!(creature_loot_money_share(&creature, 10), 3);
    assert_eq!(creature_loot_money_recipients(&creature, 1), vec![1, 2, 3]);
}

#[test]
fn creature_loot_money_share_does_not_split_solo_corpse() {
    let mut creature = DbCreatureRuntime::new(test_creature_spawn(6));
    creature.begin_corpse(Instant::now(), 1_000);

    assert_eq!(creature_loot_money_share(&creature, 10), 10);
    assert_eq!(creature_loot_money_recipients(&creature, 1), vec![1]);
}

#[test]
fn loot_inventory_full_error_matches_cmangos() {
    assert_eq!(EQUIP_ERR_INVENTORY_FULL, 50);
}

#[test]
fn creature_loot_money_share_falls_back_to_looter_for_empty_group_owner_set() {
    let mut creature = DbCreatureRuntime::new(test_creature_spawn(6));
    creature.begin_corpse(Instant::now(), 1_000);
    creature.loot_method = Some(CreatureLootMethod {
        method: 3,
        threshold: 2,
        master_looter: 0,
    });

    assert_eq!(creature_loot_money_share(&creature, 10), 10);
    assert_eq!(creature_loot_money_recipients(&creature, 7), vec![7]);
}

#[test]
fn loot_start_roll_body_uses_cmangos_vote_mask_all() {
    let loot = DbCreatureLootRuntime {
        slot: 4,
        item: 25,
        count: 1,
        display_id: 100,
        quality: 2,
        free_for_all: false,
        quest_drop: false,
    };
    let body = build_loot_start_roll_body(ObjectGuid::new(HighGuid::Unit, 0, 99), loot.slot, &loot);

    assert_eq!(body.len(), 29);
    assert_eq!(u32::from_le_bytes(body[8..12].try_into().unwrap()), 4);
    assert_eq!(u32::from_le_bytes(body[12..16].try_into().unwrap()), 25);
    assert_eq!(u32::from_le_bytes(body[24..28].try_into().unwrap()), 60_000);
    assert_eq!(body[28], 0x0F);
}

#[test]
fn item_name_query_response_matches_cmangos_shape() {
    let mut template = test_item_template(25, 2, 13, 1.0, 2.0, 0);
    template.name = "Worn Shortsword".to_string();
    template.displayid = 1542;
    let body = build_item_name_query_response(&template);

    assert_eq!(u32::from_le_bytes(body[0..4].try_into().unwrap()), 25);
    assert_eq!(&body[4..20], b"Worn Shortsword\0");
    assert_eq!(
        u32::from_le_bytes(body[20..24].try_into().unwrap()),
        template.inventory_type
    );
}

#[test]
fn item_query_response_includes_cmangos_stats_damage_and_spells() {
    let mut template = test_item_template(1161, 2, 13, 3.0, 6.0, 0);
    template.name = "Militia Shortsword".to_string();
    template.stats[0] = wow_db::ItemTemplateStat {
        stat_type: 4,
        stat_value: 2,
    };
    template.stats[1] = wow_db::ItemTemplateStat {
        stat_type: 7,
        stat_value: 3,
    };
    template.damage[1] = wow_db::ItemTemplateDamage {
        damage_min: 1.0,
        damage_max: 2.0,
        damage_type: 2,
    };
    template.spells[0] = wow_db::ItemTemplateSpell {
        spell_id: 123,
        spell_trigger: 1,
        spell_charges: -1,
        spell_cooldown: 2500,
        spell_category: 7,
        spell_category_cooldown: -1,
    };

    let body = build_item_query_single_response(template.entry, Some(&template));
    let mut offset = 12;
    offset += body[offset..]
        .iter()
        .position(|byte| *byte == 0)
        .expect("name terminator")
        + 1;
    offset += 3;
    let stats_offset = offset + 20 * 4;
    let damage_offset = stats_offset + 10 * 8;
    let armor_offset = damage_offset + 5 * 12;
    let spell_offset = armor_offset + 7 * 4 + 3 * 4;

    assert_eq!(
        u32::from_le_bytes(body[stats_offset..stats_offset + 4].try_into().unwrap()),
        4
    );
    assert_eq!(
        i32::from_le_bytes(body[stats_offset + 4..stats_offset + 8].try_into().unwrap()),
        2
    );
    assert_eq!(
        u32::from_le_bytes(
            body[stats_offset + 8..stats_offset + 12]
                .try_into()
                .unwrap()
        ),
        7
    );
    assert_eq!(
        i32::from_le_bytes(
            body[stats_offset + 12..stats_offset + 16]
                .try_into()
                .unwrap()
        ),
        3
    );
    assert_eq!(
        f32::from_le_bytes(body[damage_offset..damage_offset + 4].try_into().unwrap()),
        3.0
    );
    assert_eq!(
        f32::from_le_bytes(
            body[damage_offset + 4..damage_offset + 8]
                .try_into()
                .unwrap()
        ),
        6.0
    );
    assert_eq!(
        f32::from_le_bytes(
            body[damage_offset + 12..damage_offset + 16]
                .try_into()
                .unwrap()
        ),
        1.0
    );
    assert_eq!(
        f32::from_le_bytes(
            body[damage_offset + 16..damage_offset + 20]
                .try_into()
                .unwrap()
        ),
        2.0
    );
    assert_eq!(
        u32::from_le_bytes(
            body[damage_offset + 20..damage_offset + 24]
                .try_into()
                .unwrap()
        ),
        2
    );
    assert_eq!(
        u32::from_le_bytes(body[spell_offset..spell_offset + 4].try_into().unwrap()),
        123
    );
    assert_eq!(
        i32::from_le_bytes(
            body[spell_offset + 8..spell_offset + 12]
                .try_into()
                .unwrap()
        ),
        -1
    );
    assert_eq!(
        i32::from_le_bytes(
            body[spell_offset + 20..spell_offset + 24]
                .try_into()
                .unwrap()
        ),
        -1
    );
}

#[test]
fn item_query_response_falls_back_to_spell_cooldowns_when_item_has_no_override() {
    let mut template = test_item_template(117, 0, 0, 0.0, 0.0, 0);
    template.spells[0] = wow_db::ItemTemplateSpell {
        spell_id: 433,
        spell_trigger: ITEM_SPELLTRIGGER_ON_USE,
        spell_charges: -1,
        spell_cooldown: -1,
        spell_category: 11,
        spell_category_cooldown: -1,
    };
    let spell_cooldowns = [
        Some(ItemQuerySpellCooldown {
            recovery_time: 0,
            category: 11,
            category_recovery_time: 60_000,
        }),
        None,
        None,
        None,
        None,
    ];

    let body = build_item_query_single_response_with_spell_cooldowns(
        template.entry,
        Some(&template),
        Some(&spell_cooldowns),
    );
    let mut offset = 12;
    offset += body[offset..]
        .iter()
        .position(|byte| *byte == 0)
        .expect("name terminator")
        + 1;
    offset += 3;
    let spell_offset = offset + 20 * 4 + 10 * 8 + 5 * 12 + 7 * 4 + 3 * 4;

    assert_eq!(
        i32::from_le_bytes(
            body[spell_offset + 12..spell_offset + 16]
                .try_into()
                .unwrap()
        ),
        0
    );
    assert_eq!(
        u32::from_le_bytes(
            body[spell_offset + 16..spell_offset + 20]
                .try_into()
                .unwrap()
        ),
        11
    );
    assert_eq!(
        i32::from_le_bytes(
            body[spell_offset + 20..spell_offset + 24]
                .try_into()
                .unwrap()
        ),
        60_000
    );
}

#[test]
fn item_query_response_writes_cmangos_invalid_spell_slots() {
    let template = test_item_template(118, 0, 0, 0.0, 0.0, 0);
    let spell_cooldowns = [None, None, None, None, None];

    let body = build_item_query_single_response_with_spell_cooldowns(
        template.entry,
        Some(&template),
        Some(&spell_cooldowns),
    );
    let mut offset = 12;
    offset += body[offset..]
        .iter()
        .position(|byte| *byte == 0)
        .expect("name terminator")
        + 1;
    offset += 3;
    let spell_offset = offset + 20 * 4 + 10 * 8 + 5 * 12 + 7 * 4 + 3 * 4;

    assert_eq!(
        u32::from_le_bytes(body[spell_offset..spell_offset + 4].try_into().unwrap()),
        0
    );
    assert_eq!(
        u32::from_le_bytes(
            body[spell_offset + 4..spell_offset + 8]
                .try_into()
                .unwrap()
        ),
        0
    );
    assert_eq!(
        u32::from_le_bytes(
            body[spell_offset + 8..spell_offset + 12]
                .try_into()
                .unwrap()
        ),
        0
    );
    assert_eq!(
        i32::from_le_bytes(
            body[spell_offset + 12..spell_offset + 16]
                .try_into()
                .unwrap()
        ),
        -1
    );
    assert_eq!(
        u32::from_le_bytes(
            body[spell_offset + 16..spell_offset + 20]
                .try_into()
                .unwrap()
        ),
        0
    );
    assert_eq!(
        i32::from_le_bytes(
            body[spell_offset + 20..spell_offset + 24]
                .try_into()
                .unwrap()
        ),
        -1
    );
}

#[tokio::test]
async fn party_master_loot_members_only_for_current_master_looter() {
    let parties = PartyManager::default();
    parties
        .invite(
            PartyMember {
                guid: 1,
                name: "Leader".to_string(),
            },
            PartyMember {
                guid: 2,
                name: "Member".to_string(),
            },
            SessionId(2),
        )
        .await;
    parties.accept(2).await;
    assert_eq!(
        parties.set_loot_method(1, 2, 2, 3).await.result,
        PartyResult::Ok
    );

    assert_eq!(parties.master_loot_members_for(2).await, Some(vec![1, 2]));
    assert_eq!(parties.master_loot_members_for(1).await, None);
    assert_eq!(build_loot_master_list_body(&[1, 2]).len(), 17);
}

#[tokio::test]
async fn party_manager_converts_to_raid_and_updates_subgroup_assistant_flags() {
    let parties = PartyManager::default();
    parties
        .invite(
            PartyMember {
                guid: 1,
                name: "Leader".to_string(),
            },
            PartyMember {
                guid: 2,
                name: "Member".to_string(),
            },
            SessionId(2),
        )
        .await;
    parties.accept(2).await;

    let raid = parties.convert_to_raid(1).await;
    assert_eq!(raid.result, PartyResult::Ok);
    assert_eq!(parties.membership(2).await.map(|m| m.raid), Some(true));

    let assistant = parties.set_assistant(1, 2, true).await;
    assert_eq!(assistant.result, PartyResult::Ok);
    let subgroup = parties.change_subgroup(2, "Leader", 1).await;
    assert_eq!(subgroup.result, PartyResult::Ok);

    let leader_list = parties.group_list_packet_for(1).await.unwrap();
    assert_eq!(leader_list.body[0], 1, "group-list marks raid groups");
    assert_eq!(leader_list.body[1], 1, "leader moved to subgroup 1");
    let member_offset = 6 + "Member".len() + 1 + 8 + 1;
    assert_eq!(
        leader_list.body[member_offset], 0x80,
        "member entry carries assistant bit"
    );
}

#[test]
fn party_member_stats_full_body_matches_cmangos_core_fields() {
    let guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let snapshot = PlayerRuntimeSnapshot {
        position: WorldPosition::new(0, 42.0, 84.0, 1.0, 0.0),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
        jump: JumpInfo::default(),
        flags: 0,
        death_state: PlayerDeathState::Alive,
        stand_state: PLAYER_STAND_STATE_STAND,
        level: 3,
        race: 1,
        class: 1,
        xp: 0,
        health: 33,
        max_health: 44,
        power1: 0,
        max_power1: 0,
        last_mana_use_at: None,
        power2: 50,
        power4: 0,
        max_power4: 0,
        active_spells: HashSet::new(),
        inventory: Vec::new(),
        quest_statuses: HashMap::new(),
        active_auras: Vec::new(),
        spell_global_cooldowns_until: HashMap::new(),
        spell_cooldowns_until: HashMap::new(),
        spell_cooldown_categories: HashMap::new(),
        spell_cooldown_item_ids: HashMap::new(),
        queued_next_melee_spell: None,
        combo_target: None,
        combo_points: 0,
        base_combat_stats: test_player_combat_stats(),
        combat_stats: test_player_combat_stats(),
        in_combat: false,
        active_combat_target: None,
        active_combat_attack_kind: PlayerAutoAttackKind::Melee,
        active_combat_next_swing_at: None,
    };

    let body = build_party_member_stats_full_body(guid, Some(&snapshot)).unwrap();
    let packed_len = body[0] as usize + 1;
    let payload = &body[packed_len..];
    assert_eq!(u32::from_le_bytes(payload[0..4].try_into().unwrap()), 0x7FF);
    assert_eq!(payload[4], MEMBER_STATUS_ONLINE);
    assert_eq!(u16::from_le_bytes(payload[5..7].try_into().unwrap()), 33);
    assert_eq!(u16::from_le_bytes(payload[7..9].try_into().unwrap()), 44);
    assert_eq!(payload[9], 1, "warrior reports rage power type");
    assert_eq!(u16::from_le_bytes(payload[10..12].try_into().unwrap()), 50);
    assert_eq!(
        u16::from_le_bytes(payload[12..14].try_into().unwrap()),
        POWER_RAGE_DEFAULT as u16
    );
    assert_eq!(u16::from_le_bytes(payload[14..16].try_into().unwrap()), 3);
}

#[test]
fn party_member_stats_reports_rogue_energy_power() {
    let guid = ObjectGuid::new(HighGuid::Player, 0, 8);
    let mut snapshot = PlayerRuntimeSnapshot {
        position: WorldPosition::new(0, 42.0, 84.0, 1.0, 0.0),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
        jump: JumpInfo::default(),
        flags: 0,
        death_state: PlayerDeathState::Alive,
        stand_state: PLAYER_STAND_STATE_STAND,
        level: 3,
        race: 1,
        class: 4,
        xp: 0,
        health: 33,
        max_health: 44,
        power1: 0,
        max_power1: 0,
        last_mana_use_at: None,
        power2: 0,
        power4: POWER_ENERGY_DEFAULT,
        max_power4: POWER_ENERGY_DEFAULT,
        active_spells: HashSet::new(),
        inventory: Vec::new(),
        quest_statuses: HashMap::new(),
        active_auras: Vec::new(),
        spell_global_cooldowns_until: HashMap::new(),
        spell_cooldowns_until: HashMap::new(),
        spell_cooldown_categories: HashMap::new(),
        spell_cooldown_item_ids: HashMap::new(),
        queued_next_melee_spell: None,
        combo_target: None,
        combo_points: 0,
        base_combat_stats: test_player_combat_stats(),
        combat_stats: test_player_combat_stats(),
        in_combat: false,
        active_combat_target: None,
        active_combat_attack_kind: PlayerAutoAttackKind::Melee,
        active_combat_next_swing_at: None,
    };
    snapshot.class = 4;

    let body = build_party_member_stats_full_body(guid, Some(&snapshot)).unwrap();
    let packed_len = body[0] as usize + 1;
    let payload = &body[packed_len..];

    assert_eq!(payload[9], POWER_ENERGY);
    assert_eq!(
        u16::from_le_bytes(payload[10..12].try_into().unwrap()),
        POWER_ENERGY_DEFAULT as u16
    );
    assert_eq!(
        u16::from_le_bytes(payload[12..14].try_into().unwrap()),
        POWER_ENERGY_DEFAULT as u16
    );
}

#[test]
fn group_xp_rate_matches_cmangos_party_sizes() {
    assert_eq!(group_xp_rate(1), 1.0);
    assert_eq!(group_xp_rate(2), 1.0);
    assert_eq!(group_xp_rate(3), 1.166);
    assert_eq!(group_xp_rate(4), 1.3);
    assert_eq!(group_xp_rate(5), 1.4);
    assert_eq!(group_xp_rate(6), 0.7);
}

#[test]
fn join_channel_request_reads_name_and_password() {
    let request = JoinChannelRequest::read(b"General - Elwynn Forest\0hunter2\0").unwrap();
    assert_eq!(request.channel_name, "General - Elwynn Forest");
    assert_eq!(request.password, "hunter2");
}

#[test]
fn join_channel_request_allows_missing_password_string() {
    let request = JoinChannelRequest::read(b"Rustaceans\0").unwrap();
    assert_eq!(request.channel_name, "Rustaceans");
    assert!(request.password.is_empty());
}

#[test]
fn build_channel_notify_you_joined_body_uses_cmangos_layout() {
    let body = build_channel_notify_you_joined_body("Rustaceans");
    assert_eq!(body[0], CHAT_YOU_JOINED_NOTICE);
    assert_eq!(&body[1..12], b"Rustaceans\0");
    assert_eq!(
        u32::from_le_bytes(body[12..16].try_into().unwrap()),
        CHANNEL_FLAG_CUSTOM
    );
    assert_eq!(u32::from_le_bytes(body[16..20].try_into().unwrap()), 0);
}

#[test]
fn build_channel_notify_you_joined_body_uses_builtin_general_flags() {
    let body = build_channel_notify_you_joined_body("General - Elwynn Forest");
    let flags_offset = "General - Elwynn Forest".len() + 2;
    assert_eq!(
        u32::from_le_bytes(body[flags_offset..flags_offset + 4].try_into().unwrap()),
        CHANNEL_FLAG_GENERAL | CHANNEL_FLAG_NOT_LFG
    );
}

#[test]
fn build_channel_notify_you_joined_body_uses_builtin_trade_flags() {
    let body = build_channel_notify_you_joined_body("Trade - Stormwind City");
    let flags_offset = "Trade - Stormwind City".len() + 2;
    assert_eq!(
        u32::from_le_bytes(body[flags_offset..flags_offset + 4].try_into().unwrap()),
        CHANNEL_FLAG_CITY | CHANNEL_FLAG_GENERAL | CHANNEL_FLAG_NOT_LFG | CHANNEL_FLAG_TRADE
    );
}

#[test]
fn build_channel_notify_you_joined_body_uses_builtin_lfg_flags() {
    let body = build_channel_notify_you_joined_body("LookingForGroup");
    let flags_offset = "LookingForGroup".len() + 2;
    assert_eq!(
        u32::from_le_bytes(body[flags_offset..flags_offset + 4].try_into().unwrap()),
        CHANNEL_FLAG_LFG | CHANNEL_FLAG_GENERAL
    );
}

#[tokio::test]
async fn handle_join_channel_sends_you_joined_notify_packet() {
    let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel();
    let mut sink = WorldPacketSink::new(outbound_tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);

    handle_join_channel(
        &mut sink,
        read_join_channel_request(b"General - Elwynn Forest\0hunter2\0"),
        &mut header_crypto,
    )
    .await
    .unwrap();

    let packet = outbound_rx.try_recv().unwrap();
    assert_eq!(packet.opcode, WorldOpcode::SmsgChannelNotify as u16);
    assert_eq!(
        packet.body,
        build_channel_notify_you_joined_body("General - Elwynn Forest")
    );
}

#[tokio::test]
async fn handle_join_channel_ignores_empty_channel_name() {
    let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel();
    let mut sink = WorldPacketSink::new(outbound_tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);

    handle_join_channel(
        &mut sink,
        read_join_channel_request(b"\0hunter2\0"),
        &mut header_crypto,
    )
    .await
    .unwrap();

    assert!(outbound_rx.try_recv().is_err());
}

#[test]
fn parses_text_emote_packet() {
    let mut body = Vec::new();
    body.extend_from_slice(&12u32.to_le_bytes());
    body.extend_from_slice(&33u32.to_le_bytes());
    body.extend_from_slice(&99u64.to_le_bytes());

    let emote = TextEmote::read(&body).unwrap();

    assert_eq!(
        emote,
        TextEmote {
            text_emote: 12,
            emote_num: 33,
            target_guid: 99
        }
    );
}

#[test]
fn parses_cast_spell_packet_with_unit_target() {
    let target = ObjectGuid::new(HighGuid::Unit, 6, 45);
    let mut body = Vec::new();
    body.extend_from_slice(&WARRIOR_HEROIC_STRIKE_RANK_1.to_le_bytes());
    body.extend_from_slice(&SPELL_CAST_TARGET_UNIT.to_le_bytes());
    PackedGuid::write(&mut body, target).unwrap();

    let cast = CastSpellPacket::read(&body).unwrap();

    assert_eq!(cast.spell_id, WARRIOR_HEROIC_STRIKE_RANK_1);
    assert_eq!(cast.targets.target_mask, SPELL_CAST_TARGET_UNIT);
    assert_eq!(cast.targets.unit_target, Some(target));
    assert_eq!(cast.targets.gameobject_target, None);
}

#[test]
fn spell_packets_match_cmangos_success_shapes() {
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    let target = ObjectGuid::new(HighGuid::Unit, 6, 45);
    let targets = SpellCastTargets {
        target_mask: SPELL_CAST_TARGET_UNIT,
        unit_target: Some(target),
        gameobject_target: None,
        source_location: None,
        destination: None,
    };

    let result = build_cast_result_ok_body(WARRIOR_HEROIC_STRIKE_RANK_1);
    assert_eq!(&result[0..4], &WARRIOR_HEROIC_STRIKE_RANK_1.to_le_bytes());
    assert_eq!(result[4], 0);

    let failure =
        build_cast_result_failure_body(WARRIOR_HEROIC_STRIKE_RANK_1, SPELL_FAILED_OUT_OF_RANGE);
    assert_eq!(&failure[0..4], &WARRIOR_HEROIC_STRIKE_RANK_1.to_le_bytes());
    assert_eq!(failure[4], 2);
    assert_eq!(failure[5], SPELL_FAILED_OUT_OF_RANGE);

    let go = build_spell_go_body(caster, WARRIOR_HEROIC_STRIKE_RANK_1, &targets).unwrap();
    let mut cursor = 0;
    cursor += PackedGuid::packed_size(caster) * 2;
    assert_eq!(
        read_u32(&go, &mut cursor).unwrap(),
        WARRIOR_HEROIC_STRIKE_RANK_1
    );
    assert_eq!(
        u16::from_le_bytes(go[cursor..cursor + 2].try_into().unwrap()),
        CAST_FLAG_SPELL_GO
    );
    cursor += 2;
    assert_eq!(go[cursor], 1);
    cursor += 1;
    assert_eq!(
        u64::from_le_bytes(go[cursor..cursor + 8].try_into().unwrap()),
        target.raw()
    );
    cursor += 8;
    assert_eq!(go[cursor], 0);
    cursor += 1;
    assert_eq!(
        u16::from_le_bytes(go[cursor..cursor + 2].try_into().unwrap()),
        SPELL_CAST_TARGET_UNIT
    );
    cursor += 2;
    assert_eq!(read_packed_guid(&go, &mut cursor).unwrap(), target);
    assert_eq!(cursor, go.len());

    let resisted = build_spell_go_body_with_miss(
        caster,
        WARRIOR_HEROIC_STRIKE_RANK_1,
        &targets,
        SPELL_MISS_RESIST,
    )
    .unwrap();
    let mut cursor = PackedGuid::packed_size(caster) * 2 + 4;
    assert_eq!(
        u16::from_le_bytes(resisted[cursor..cursor + 2].try_into().unwrap()),
        CAST_FLAG_SPELL_GO
    );
    cursor += 2;
    assert_eq!(resisted[cursor], 0);
    cursor += 1;
    assert_eq!(resisted[cursor], 1);
    cursor += 1;
    assert_eq!(
        u64::from_le_bytes(resisted[cursor..cursor + 8].try_into().unwrap()),
        target.raw()
    );
    cursor += 8;
    assert_eq!(resisted[cursor], SPELL_MISS_RESIST);
    cursor += 1;
    assert_eq!(
        u16::from_le_bytes(resisted[cursor..cursor + 2].try_into().unwrap()),
        SPELL_CAST_TARGET_UNIT
    );
    cursor += 2;
    assert_eq!(read_packed_guid(&resisted, &mut cursor).unwrap(), target);
    assert_eq!(cursor, resisted.len());
}

#[test]
fn ranged_spell_packets_include_cmangos_ammo_visual_payload() {
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    let target = ObjectGuid::new(HighGuid::Unit, 6, 45);
    let targets = SpellCastTargets {
        target_mask: SPELL_CAST_TARGET_UNIT,
        unit_target: Some(target),
        gameobject_target: None,
        source_location: None,
        destination: None,
    };
    let ammo = wow_proto::SpellAmmoVisual {
        display_id: 5996,
        inventory_type: INVTYPE_AMMO,
    };

    let start = build_spell_start_body_with_ammo(caster, 75, 0, &targets, Some(ammo)).unwrap();
    let mut cursor = PackedGuid::packed_size(caster) * 2 + 4;
    assert_eq!(
        u16::from_le_bytes(start[cursor..cursor + 2].try_into().unwrap()),
        CAST_FLAG_SPELL_START | CAST_FLAG_AMMO
    );
    cursor += 2;
    assert_eq!(read_u32(&start, &mut cursor).unwrap(), 0);
    assert_eq!(
        u16::from_le_bytes(start[cursor..cursor + 2].try_into().unwrap()),
        SPELL_CAST_TARGET_UNIT
    );
    cursor += 2;
    assert_eq!(read_packed_guid(&start, &mut cursor).unwrap(), target);
    assert_eq!(read_u32(&start, &mut cursor).unwrap(), ammo.display_id);
    assert_eq!(read_u32(&start, &mut cursor).unwrap(), ammo.inventory_type);
    assert_eq!(cursor, start.len());

    let go = build_spell_go_body_with_ammo(caster, 75, &targets, Some(ammo)).unwrap();
    let mut cursor = PackedGuid::packed_size(caster) * 2 + 4;
    assert_eq!(
        u16::from_le_bytes(go[cursor..cursor + 2].try_into().unwrap()),
        CAST_FLAG_SPELL_GO | CAST_FLAG_AMMO
    );
    cursor += 2;
    assert_eq!(go[cursor], 1);
    cursor += 1;
    assert_eq!(
        u64::from_le_bytes(go[cursor..cursor + 8].try_into().unwrap()),
        target.raw()
    );
    cursor += 8;
    assert_eq!(go[cursor], 0);
    cursor += 1;
    assert_eq!(
        u16::from_le_bytes(go[cursor..cursor + 2].try_into().unwrap()),
        SPELL_CAST_TARGET_UNIT
    );
    cursor += 2;
    assert_eq!(read_packed_guid(&go, &mut cursor).unwrap(), target);
    assert_eq!(read_u32(&go, &mut cursor).unwrap(), ammo.display_id);
    assert_eq!(read_u32(&go, &mut cursor).unwrap(), ammo.inventory_type);
    assert_eq!(cursor, go.len());
}

#[test]
fn item_spell_packets_use_item_source_and_item_cast_flag() {
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    let item = ObjectGuid::new(HighGuid::Item, 0, 42);
    let targets = SpellCastTargets {
        target_mask: 0,
        unit_target: None,
        gameobject_target: None,
        source_location: None,
        destination: None,
    };

    let start = build_spell_start_body_with_source(item, caster, 433, 0, &targets).unwrap();
    let mut cursor = 0;
    assert_eq!(read_packed_guid(&start, &mut cursor).unwrap(), item);
    assert_eq!(read_packed_guid(&start, &mut cursor).unwrap(), caster);
    assert_eq!(read_u32(&start, &mut cursor).unwrap(), 433);
    assert_eq!(
        u16::from_le_bytes(start[cursor..cursor + 2].try_into().unwrap()),
        CAST_FLAG_SPELL_START
    );
    cursor += 2;
    assert_eq!(read_u32(&start, &mut cursor).unwrap(), 0);
    assert_eq!(
        u16::from_le_bytes(start[cursor..cursor + 2].try_into().unwrap()),
        0
    );
    cursor += 2;
    assert_eq!(cursor, start.len());

    let go = build_spell_go_body_with_source(
        item,
        caster,
        433,
        CAST_FLAG_SPELL_GO | CAST_FLAG_ITEM_CASTER,
        &targets,
        None,
    )
    .unwrap();
    let mut cursor = 0;
    assert_eq!(read_packed_guid(&go, &mut cursor).unwrap(), item);
    assert_eq!(read_packed_guid(&go, &mut cursor).unwrap(), caster);
    assert_eq!(read_u32(&go, &mut cursor).unwrap(), 433);
    assert_eq!(
        u16::from_le_bytes(go[cursor..cursor + 2].try_into().unwrap()),
        CAST_FLAG_SPELL_GO | CAST_FLAG_ITEM_CASTER
    );
    cursor += 2;
    assert_eq!(go[cursor], 0);
    cursor += 1;
    assert_eq!(go[cursor], 0);
    cursor += 1;
    assert_eq!(
        u16::from_le_bytes(go[cursor..cursor + 2].try_into().unwrap()),
        0
    );
    cursor += 2;
    assert_eq!(cursor, go.len());
}

#[test]
fn prepared_spell_cast_builds_lifecycle_packets_for_player_and_item_sources() {
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    let item = ObjectGuid::new(HighGuid::Item, 0, 42);
    let targets = SpellCastTargets {
        target_mask: 0,
        unit_target: None,
        gameobject_target: None,
        source_location: None,
        destination: None,
    };
    let profile = SpellCastProfile {
        spell_id: 433,
        kind: SpellCastKind::InstantDamage,
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
    };

    let mut player_cast = PreparedSpellCast::new(433, SpellCastSource::Player, profile);
    player_cast.prepare();
    assert_eq!(player_cast.state, SpellLifecycleState::Preparing);
    let player_start = player_cast.spell_start_body(caster, 0, &targets).unwrap();
    let mut cursor = 0;
    assert_eq!(
        read_packed_guid(&player_start, &mut cursor).unwrap(),
        caster
    );
    assert_eq!(
        read_packed_guid(&player_start, &mut cursor).unwrap(),
        caster
    );
    assert_eq!(player_cast.state, SpellLifecycleState::Casting);
    let player_go = player_cast.spell_go_body(caster, &targets).unwrap();
    let cursor = PackedGuid::packed_size(caster) * 2 + 4;
    assert_eq!(
        u16::from_le_bytes(player_go[cursor..cursor + 2].try_into().unwrap()),
        CAST_FLAG_SPELL_GO
    );
    player_cast.finish();
    assert_eq!(player_cast.state, SpellLifecycleState::Finished);

    let mut item_cast =
        PreparedSpellCast::new(433, SpellCastSource::Item { item_guid: item }, profile);
    item_cast.prepare();
    let item_start = item_cast.spell_start_body(caster, 0, &targets).unwrap();
    let mut cursor = 0;
    assert_eq!(read_packed_guid(&item_start, &mut cursor).unwrap(), item);
    assert_eq!(read_packed_guid(&item_start, &mut cursor).unwrap(), caster);
    let item_go = item_cast.spell_go_body(caster, &targets).unwrap();
    let cursor = PackedGuid::packed_size(item) + PackedGuid::packed_size(caster) + 4;
    assert_eq!(
        u16::from_le_bytes(item_go[cursor..cursor + 2].try_into().unwrap()),
        CAST_FLAG_SPELL_GO | CAST_FLAG_ITEM_CASTER
    );
}

#[test]
fn opening_spell_packets_include_gameobject_target_mask() {
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    let gameobject = ObjectGuid::new(HighGuid::GameObject, 0, 12345);
    let targets = SpellCastTargets {
        target_mask: SPELL_CAST_TARGET_LOCKED | SPELL_CAST_TARGET_GAMEOBJECT,
        unit_target: None,
        gameobject_target: Some(gameobject),
        source_location: None,
        destination: None,
    };

    let start = build_spell_start_body(
        caster,
        OPENING_SPELL_ID,
        OPENING_SPELL_CAST_TIME_MS,
        &targets,
    )
    .unwrap();
    let mut cursor = PackedGuid::packed_size(caster) * 2;
    assert_eq!(read_u32(&start, &mut cursor).unwrap(), OPENING_SPELL_ID);
    assert_eq!(
        u16::from_le_bytes(start[cursor..cursor + 2].try_into().unwrap()),
        CAST_FLAG_SPELL_START
    );
    cursor += 2;
    assert_eq!(
        read_u32(&start, &mut cursor).unwrap(),
        OPENING_SPELL_CAST_TIME_MS
    );
    assert_eq!(
        u16::from_le_bytes(start[cursor..cursor + 2].try_into().unwrap()),
        SPELL_CAST_TARGET_LOCKED | SPELL_CAST_TARGET_GAMEOBJECT
    );
    cursor += 2;
    assert_eq!(read_packed_guid(&start, &mut cursor).unwrap(), gameobject);
    assert_eq!(cursor, start.len());

    let go = build_spell_go_body(caster, OPENING_SPELL_ID, &targets).unwrap();
    let mut cursor = PackedGuid::packed_size(caster) * 2;
    assert_eq!(read_u32(&go, &mut cursor).unwrap(), OPENING_SPELL_ID);
    assert_eq!(
        u16::from_le_bytes(go[cursor..cursor + 2].try_into().unwrap()),
        CAST_FLAG_SPELL_GO
    );
    cursor += 2;
    assert_eq!(go[cursor], 1);
    cursor += 1;
    assert_eq!(
        u64::from_le_bytes(go[cursor..cursor + 8].try_into().unwrap()),
        gameobject.raw()
    );
    cursor += 8;
    assert_eq!(go[cursor], 0);
    cursor += 1;
    assert_eq!(
        u16::from_le_bytes(go[cursor..cursor + 2].try_into().unwrap()),
        SPELL_CAST_TARGET_LOCKED | SPELL_CAST_TARGET_GAMEOBJECT
    );
    cursor += 2;
    assert_eq!(read_packed_guid(&go, &mut cursor).unwrap(), gameobject);
    assert_eq!(cursor, go.len());
}

#[test]
fn opening_spell_profile_is_cancelable_pending_cast_without_recovery() {
    let profile = opening_spell_cast_profile(3365);

    assert_eq!(profile.spell_id, 3365);
    assert_eq!(profile.kind, SpellCastKind::OpeningGameObject);
    assert_eq!(profile.power, SpellPowerCost::Mana { cost: 0 });
    assert_eq!(profile.global_cooldown_millis, 0);
    assert_eq!(profile.cooldown_millis, 0);
    assert!(!profile.requires_melee);
}

#[test]
fn removing_player_clears_pending_opening_spell_work() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8948.0, -131.0, 83.4, 0.0);
    map.add_player(test_player_runtime(7, SessionId(7), position))
        .unwrap();
    let targets = PendingSpellCastTargets {
        target_mask: SPELL_CAST_TARGET_GAMEOBJECT,
        unit_target: None,
        gameobject_target: Some(ObjectGuid::new(HighGuid::GameObject, 0, 77)),
        source_location: None,
        destination: None,
    };
    map.active_player_spell_casts.insert(
        7,
        ActivePlayerSpellCast {
            spell_id: OPENING_SPELL_ID,
            source: ActivePlayerSpellCastSource::OpeningGameObject,
            profile: opening_spell_cast_profile(OPENING_SPELL_ID),
            targets: targets.clone(),
            due_at: Instant::now() + Duration::from_secs(5),
            cast_time_millis: OPENING_SPELL_CAST_TIME_MS,
            interrupt_flags: 0,
            damage_pushback_count: 0,
        },
    );
    map.pending_spell_events.push(PendingSpellEvent {
        event_id: 1,
        caster_character_guid: 7,
        spell_id: OPENING_SPELL_ID,
        kind: PendingSpellEventKind::Spell {
            targets,
            target_outcome: None,
        },
        unit_target_generation: None,
        due_at: Instant::now() + Duration::from_secs(5),
    });

    map.remove_player(7);

    assert!(!map.active_player_spell_casts.contains_key(&7));
    assert!(map
        .pending_spell_events
        .iter()
        .all(|event| event.caster_character_guid != 7));
}

#[test]
fn player_death_clears_active_spell_channels_and_dynamic_objects() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let caster_guid = 7;
    let caster = ObjectGuid::new(HighGuid::Player, 0, caster_guid);
    let position = WorldPosition::new(0, -8948.0, -131.0, 83.4, 0.0);
    map.add_player(test_player_runtime(caster_guid, SessionId(7), position))
        .unwrap();
    map.add_player(test_player_runtime(
        8,
        SessionId(8),
        WorldPosition::new(0, -8947.0, -131.0, 83.4, 0.0),
    ))
    .unwrap();

    let mut spawn = test_creature_spawn(6);
    spawn.guid = 404;
    spawn.position_x = position.x + 5.0;
    spawn.position_y = position.y;
    spawn.position_z = position.z;
    let target = creature_spawn_guid(&spawn);
    map.creatures
        .insert(target.raw(), DbCreatureRuntime::new(spawn));

    let targets = PendingSpellCastTargets {
        target_mask: SPELL_CAST_TARGET_UNIT_ENEMY,
        unit_target: Some(target),
        gameobject_target: None,
        source_location: None,
        destination: None,
    };
    let fireball = fireball_spell_template();
    let profile = player_spell_cast_profile(&fireball).unwrap();
    map.active_player_spell_casts.insert(
        caster_guid,
        ActivePlayerSpellCast {
            spell_id: fireball.id,
            source: ActivePlayerSpellCastSource::Player,
            profile,
            targets: targets.clone(),
            due_at: now + Duration::from_secs(2),
            cast_time_millis: 2_000,
            interrupt_flags: fireball.interrupt_flags,
            damage_pushback_count: 0,
        },
    );
    map.pending_spell_events.push(PendingSpellEvent {
        event_id: 1,
        caster_character_guid: caster_guid,
        spell_id: fireball.id,
        kind: PendingSpellEventKind::Spell {
            targets,
            target_outcome: None,
        },
        unit_target_generation: None,
        due_at: now + Duration::from_secs(2),
    });
    let damage_effect = player_weapon_damage_effect(&profile);
    map.start_player_periodic_trigger_channel(
        caster,
        caster_guid,
        5143,
        target,
        5_000,
        1_000,
        0,
        0.0,
        damage_effect,
        now,
    )
    .unwrap()
    .expect("channel should start");
    map.pending_player_channel_impacts
        .push(PendingPlayerChannelImpact {
            caster,
            caster_character_guid: caster_guid,
            target,
            impact_at: now + Duration::from_millis(500),
            damage_effect,
            outcome: SpellDamageOutcome::normal_hit(1),
        });
    map.create_persistent_area_dynamic_object(
        caster,
        caster_guid,
        10,
        0,
        position,
        8.0,
        5_000,
        None,
        true,
        0,
        now,
    )
    .unwrap()
    .expect("dynamic object should spawn");
    assert!(!map.dynamic_objects.is_empty());

    let death = map
        .apply_player_world_damage(
            caster,
            Some(target),
            999,
            WorldDamageKind::SpellDirect,
            now + Duration::from_millis(100),
        )
        .unwrap()
        .expect("damage should apply");

    assert!(!map.active_player_spell_casts.contains_key(&caster_guid));
    assert!(map
        .pending_spell_events
        .iter()
        .all(|event| event.caster_character_guid != caster_guid));
    assert!(!map.active_player_channels.contains_key(&caster_guid));
    assert!(map
        .pending_player_channel_impacts
        .iter()
        .all(|impact| impact.caster_character_guid != caster_guid));
    assert!(map.dynamic_objects.is_empty());
    assert!(death
        .direct_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::MsgChannelUpdate as u16));
    assert!(death
        .direct_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgDestroyObject as u16));
    assert!(death
        .observer_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgDestroyObject as u16));
}

#[test]
fn removing_player_clears_spell_channels_and_notifies_observers() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let caster_guid = 7;
    let caster = ObjectGuid::new(HighGuid::Player, 0, caster_guid);
    let position = WorldPosition::new(0, -8948.0, -131.0, 83.4, 0.0);
    map.add_player(test_player_runtime(caster_guid, SessionId(7), position))
        .unwrap();
    map.add_player(test_player_runtime(
        8,
        SessionId(8),
        WorldPosition::new(0, -8947.0, -131.0, 83.4, 0.0),
    ))
    .unwrap();

    map.create_persistent_area_dynamic_object(
        caster,
        caster_guid,
        10,
        0,
        position,
        8.0,
        5_000,
        None,
        true,
        0,
        now,
    )
    .unwrap()
    .expect("dynamic object should spawn");

    let packets = map.remove_player(caster_guid);

    assert!(map.dynamic_objects.is_empty());
    assert!(packets
        .iter()
        .any(|(session_id, packet)| *session_id == SessionId(8)
            && packet.opcode == WorldOpcode::SmsgDestroyObject as u16));
    assert!(packets.iter().any(|(session_id, packet)| {
        *session_id == SessionId(8) && packet.opcode == WorldOpcode::MsgChannelUpdate as u16
    }));
}

#[test]
fn near_teleport_position_set_clears_active_spell_runtime() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let caster_guid = 7;
    let caster = ObjectGuid::new(HighGuid::Player, 0, caster_guid);
    let position = WorldPosition::new(0, -8948.0, -131.0, 83.4, 0.0);
    map.add_player(test_player_runtime(caster_guid, SessionId(7), position))
        .unwrap();
    map.add_player(test_player_runtime(
        8,
        SessionId(8),
        WorldPosition::new(0, -8947.0, -131.0, 83.4, 0.0),
    ))
    .unwrap();

    let mut spawn = test_creature_spawn(6);
    spawn.guid = 405;
    spawn.position_x = position.x + 5.0;
    spawn.position_y = position.y;
    spawn.position_z = position.z;
    let target = creature_spawn_guid(&spawn);
    map.creatures
        .insert(target.raw(), DbCreatureRuntime::new(spawn));

    let targets = PendingSpellCastTargets {
        target_mask: SPELL_CAST_TARGET_UNIT_ENEMY,
        unit_target: Some(target),
        gameobject_target: None,
        source_location: None,
        destination: None,
    };
    let fireball = fireball_spell_template();
    let profile = player_spell_cast_profile(&fireball).unwrap();
    map.active_player_spell_casts.insert(
        caster_guid,
        ActivePlayerSpellCast {
            spell_id: fireball.id,
            source: ActivePlayerSpellCastSource::Player,
            profile,
            targets: targets.clone(),
            due_at: now + Duration::from_secs(2),
            cast_time_millis: 2_000,
            interrupt_flags: fireball.interrupt_flags,
            damage_pushback_count: 0,
        },
    );
    map.pending_spell_events.push(PendingSpellEvent {
        event_id: 1,
        caster_character_guid: caster_guid,
        spell_id: fireball.id,
        kind: PendingSpellEventKind::Spell {
            targets,
            target_outcome: None,
        },
        unit_target_generation: None,
        due_at: now + Duration::from_secs(2),
    });
    let damage_effect = player_weapon_damage_effect(&profile);
    map.start_player_periodic_trigger_channel(
        caster,
        caster_guid,
        5143,
        target,
        5_000,
        1_000,
        0,
        0.0,
        damage_effect,
        now,
    )
    .unwrap()
    .expect("channel should start");
    map.pending_player_channel_impacts
        .push(PendingPlayerChannelImpact {
            caster,
            caster_character_guid: caster_guid,
            target,
            impact_at: now + Duration::from_millis(500),
            damage_effect,
            outcome: SpellDamageOutcome::normal_hit(1),
        });
    map.create_persistent_area_dynamic_object(
        caster,
        caster_guid,
        10,
        0,
        position,
        8.0,
        5_000,
        None,
        true,
        0,
        now,
    )
    .unwrap()
    .expect("dynamic object should spawn");

    let packets = map
        .set_player_position(
            caster_guid,
            WorldPosition::new(0, -8900.0, -100.0, 83.4, 0.0),
        )
        .unwrap();

    assert!(!map.active_player_spell_casts.contains_key(&caster_guid));
    assert!(map
        .pending_spell_events
        .iter()
        .all(|event| event.caster_character_guid != caster_guid));
    assert!(!map.active_player_channels.contains_key(&caster_guid));
    assert!(map
        .pending_player_channel_impacts
        .iter()
        .all(|impact| impact.caster_character_guid != caster_guid));
    assert!(map.dynamic_objects.is_empty());
    assert!(packets
        .iter()
        .any(|(session_id, packet)| *session_id == SessionId(8)
            && packet.opcode == WorldOpcode::SmsgDestroyObject as u16));
    assert!(packets.iter().any(|(session_id, packet)| {
        *session_id == SessionId(8) && packet.opcode == WorldOpcode::MsgChannelUpdate as u16
    }));
}

#[test]
fn removing_channeled_creature_aura_interrupts_player_channel() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let caster_guid = 7;
    let caster = ObjectGuid::new(HighGuid::Player, 0, caster_guid);
    let position = WorldPosition::new(0, -8948.0, -131.0, 83.4, 0.0);
    map.add_player(test_player_runtime(caster_guid, SessionId(7), position))
        .unwrap();
    map.add_player(test_player_runtime(
        8,
        SessionId(8),
        WorldPosition::new(0, -8947.0, -131.0, 83.4, 0.0),
    ))
    .unwrap();

    let mut spawn = test_creature_spawn(6);
    spawn.guid = 406;
    spawn.position_x = position.x + 5.0;
    spawn.position_y = position.y;
    spawn.position_z = position.z;
    let target = creature_spawn_guid(&spawn);
    let mut creature = DbCreatureRuntime::new(spawn);
    creature.active_auras.push(ActiveAura {
        spell_id: 5143,
        caster,
        level: 1,
        interrupt_flags: 0,
        positive: false,
        visible: true,
        duration_millis: Some(5_000),
        expires_at: Some(now + Duration::from_secs(5)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::DispelType { dispel_type: 1 }],
        proc_triggers: Vec::new(),
    });
    map.creatures.insert(target.raw(), creature);

    let fireball = fireball_spell_template();
    let profile = player_spell_cast_profile(&fireball).unwrap();
    let damage_effect = player_weapon_damage_effect(&profile);
    map.start_player_periodic_trigger_channel(
        caster,
        caster_guid,
        5143,
        target,
        5_000,
        1_000,
        0,
        0.0,
        damage_effect,
        now,
    )
    .unwrap()
    .expect("channel should start");
    map.pending_player_channel_impacts
        .push(PendingPlayerChannelImpact {
            caster,
            caster_character_guid: caster_guid,
            target,
            impact_at: now + Duration::from_millis(500),
            damage_effect,
            outcome: SpellDamageOutcome::normal_hit(1),
        });

    let event = map
        .remove_db_creature_auras_by_dispel_type(target, caster_guid, 1, 1, now)
        .unwrap()
        .expect("aura should be removed");

    assert!(!map.active_player_channels.contains_key(&caster_guid));
    assert!(map
        .pending_player_channel_impacts
        .iter()
        .all(|impact| impact.caster_character_guid != caster_guid));
    assert!(event
        .aura_update
        .observer_packets
        .iter()
        .any(|(session_id, packet)| *session_id == SessionId(7)
            && packet.opcode == WorldOpcode::MsgChannelUpdate as u16));
    assert!(event
        .aura_update
        .observer_packets
        .iter()
        .any(|(session_id, packet)| *session_id == SessionId(8)
            && packet.opcode == WorldOpcode::MsgChannelUpdate as u16));
}

#[test]
fn creature_target_death_interrupts_active_player_spell_work_targeting_it() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let caster_guid = 7;
    let caster = ObjectGuid::new(HighGuid::Player, 0, caster_guid);
    let position = WorldPosition::new(0, -8948.0, -131.0, 83.4, 0.0);
    map.add_player(test_player_runtime(caster_guid, SessionId(7), position))
        .unwrap();
    map.add_player(test_player_runtime(
        8,
        SessionId(8),
        WorldPosition::new(0, -8947.0, -131.0, 83.4, 0.0),
    ))
    .unwrap();

    let mut spawn = test_creature_spawn(6);
    spawn.guid = 407;
    spawn.position_x = position.x + 5.0;
    spawn.position_y = position.y;
    spawn.position_z = position.z;
    let target = creature_spawn_guid(&spawn);
    map.creatures
        .insert(target.raw(), DbCreatureRuntime::new(spawn));

    let targets = PendingSpellCastTargets {
        target_mask: SPELL_CAST_TARGET_UNIT_ENEMY,
        unit_target: Some(target),
        gameobject_target: None,
        source_location: None,
        destination: None,
    };
    let fireball = fireball_spell_template();
    let profile = player_spell_cast_profile(&fireball).unwrap();
    map.active_player_spell_casts.insert(
        caster_guid,
        ActivePlayerSpellCast {
            spell_id: fireball.id,
            source: ActivePlayerSpellCastSource::Player,
            profile,
            targets: targets.clone(),
            due_at: now + Duration::from_secs(2),
            cast_time_millis: 2_000,
            interrupt_flags: fireball.interrupt_flags,
            damage_pushback_count: 0,
        },
    );
    map.pending_spell_events.push(PendingSpellEvent {
        event_id: 1,
        caster_character_guid: caster_guid,
        spell_id: fireball.id,
        kind: PendingSpellEventKind::Spell {
            targets,
            target_outcome: None,
        },
        unit_target_generation: map
            .creatures
            .get(&target.raw())
            .map(|creature| (target, creature.life_generation)),
        due_at: now + Duration::from_secs(3),
    });

    let event = map
        .apply_db_creature_damage(DbCreatureDamageRequest {
            creature_guid: target,
            killer: caster,
            damage: 999,
            melee_outcome: None,
            spell_damage_outcome: None,
            spell_id: Some(fireball.id),
            spell_school: SPELL_SCHOOL_MASK_NORMAL as u8,
            suppress_attacker_state: true,
            now,
            now_epoch_secs: 1_000,
            exclude_character_guid: None,
            corpse_loot: None,
        })
        .unwrap()
        .expect("damage should kill creature");

    assert!(event.death_finalization.is_some());
    assert!(!map.active_player_spell_casts.contains_key(&caster_guid));
    assert!(map
        .pending_spell_events
        .iter()
        .all(|event| event.caster_character_guid != caster_guid));
    assert!(event
        .observer_packets
        .iter()
        .any(|(session_id, packet)| *session_id == SessionId(7)
            && packet.opcode == WorldOpcode::SmsgSpellFailure as u16));
    assert!(event
        .observer_packets
        .iter()
        .any(|(session_id, packet)| *session_id == SessionId(8)
            && packet.opcode == WorldOpcode::SmsgSpellFailedOther as u16));
}

#[test]
fn deleting_creature_target_interrupts_active_player_spell_work_targeting_it() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let caster_guid = 7;
    let position = WorldPosition::new(0, -8948.0, -131.0, 83.4, 0.0);
    map.add_player(test_player_runtime(caster_guid, SessionId(7), position))
        .unwrap();
    map.add_player(test_player_runtime(
        8,
        SessionId(8),
        WorldPosition::new(0, -8947.0, -131.0, 83.4, 0.0),
    ))
    .unwrap();

    let mut spawn = test_creature_spawn(6);
    spawn.guid = 408;
    spawn.position_x = position.x + 5.0;
    spawn.position_y = position.y;
    spawn.position_z = position.z;
    let target = creature_spawn_guid(&spawn);
    map.creatures
        .insert(target.raw(), DbCreatureRuntime::new(spawn));

    let fireball = fireball_spell_template();
    let profile = player_spell_cast_profile(&fireball).unwrap();
    map.active_player_spell_casts.insert(
        caster_guid,
        ActivePlayerSpellCast {
            spell_id: fireball.id,
            source: ActivePlayerSpellCastSource::Player,
            profile,
            targets: PendingSpellCastTargets {
                target_mask: SPELL_CAST_TARGET_UNIT_ENEMY,
                unit_target: Some(target),
                gameobject_target: None,
                source_location: None,
                destination: None,
            },
            due_at: now + Duration::from_secs(2),
            cast_time_millis: 2_000,
            interrupt_flags: fireball.interrupt_flags,
            damage_pushback_count: 0,
        },
    );

    let event = map
        .delete_db_creature_runtime(Some(target), None, None)
        .unwrap()
        .expect("creature should be deleted");

    assert!(!map.active_player_spell_casts.contains_key(&caster_guid));
    assert!(event
        .observer_packets
        .iter()
        .any(|(session_id, packet)| *session_id == SessionId(7)
            && packet.opcode == WorldOpcode::SmsgSpellFailure as u16));
    assert!(event
        .observer_packets
        .iter()
        .any(|(session_id, packet)| *session_id == SessionId(8)
            && packet.opcode == WorldOpcode::SmsgSpellFailedOther as u16));
}

#[test]
fn regular_movement_position_update_preserves_non_movement_interrupt_cast() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let caster_guid = 7;
    let position = WorldPosition::new(0, -8948.0, -131.0, 83.4, 0.0);
    map.add_player(test_player_runtime(caster_guid, SessionId(7), position))
        .unwrap();
    let fireball = fireball_spell_template();
    map.active_player_spell_casts.insert(
        caster_guid,
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
            due_at: now + Duration::from_secs(2),
            cast_time_millis: 2_000,
            interrupt_flags: 0,
            damage_pushback_count: 0,
        },
    );
    let movement = MovementInfo {
        flags: 0,
        client_time: 1,
        position: WorldPosition::new(0, -8947.5, -131.0, 83.4, 0.0),
        fall_time: 0,
        jump: JumpInfo::default(),
    };

    map.update_player_position(
        caster_guid,
        WorldOpcode::MsgMoveHeartbeat as u16,
        &movement,
        1,
    )
    .unwrap();

    assert!(
        map.active_player_spell_casts.contains_key(&caster_guid),
        "ordinary movement should still defer to movement interrupt flags"
    );
}

#[test]
fn raptor_strike_spell_packets_match_success_shapes() {
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    let target = ObjectGuid::new(HighGuid::Unit, 6, 45);
    let template = raptor_strike_spell_template();
    let spell_profile = player_spell_cast_profile(&template).unwrap();
    let targets = normalize_spell_cast_targets(
        SpellCastTargets {
            target_mask: SPELL_CAST_TARGET_UNIT_ENEMY,
            unit_target: Some(target),
            gameobject_target: None,
            source_location: None,
            destination: None,
        },
        &spell_profile,
        &SpellInfo::from_template(&template),
        caster,
    );

    let result = build_cast_result_ok_body(HUNTER_RAPTOR_STRIKE_RANK_1);
    assert_eq!(&result[0..4], &HUNTER_RAPTOR_STRIKE_RANK_1.to_le_bytes());
    assert_eq!(result[4], 0);

    let go = build_spell_go_body(caster, HUNTER_RAPTOR_STRIKE_RANK_1, &targets).unwrap();
    let mut cursor = 0;
    cursor += PackedGuid::packed_size(caster) * 2;
    assert_eq!(
        read_u32(&go, &mut cursor).unwrap(),
        HUNTER_RAPTOR_STRIKE_RANK_1
    );
    assert_eq!(
        u16::from_le_bytes(go[cursor..cursor + 2].try_into().unwrap()),
        CAST_FLAG_SPELL_GO
    );
    cursor += 2;
    assert_eq!(go[cursor], 1);
    cursor += 1;
    assert_eq!(
        u64::from_le_bytes(go[cursor..cursor + 8].try_into().unwrap()),
        target.raw()
    );
    cursor += 8;
    assert_eq!(go[cursor], 0);
    cursor += 1;
    assert_eq!(
        u16::from_le_bytes(go[cursor..cursor + 2].try_into().unwrap()),
        SPELL_CAST_TARGET_UNIT
    );
}

#[test]
fn text_emote_body_matches_cmangos_empty_target_shape() {
    let character = ActiveCharacter {
        guid: 7,
        name: "Ada".to_string(),
        race: 1,
        class: 1,
        level: 1,
        xp: 0,
        position: WorldPosition::new(0, 1.0, 2.0, 3.0, 4.0),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
        jump: JumpInfo::default(),
    };
    let body = build_text_emote_body(&character, 12, 33, None);
    let guid = ObjectGuid::new(HighGuid::Player, 0, 7).raw();

    assert_eq!(&body[0..8], &guid.to_le_bytes());
    assert_eq!(&body[8..12], &12u32.to_le_bytes());
    assert_eq!(&body[12..16], &33u32.to_le_bytes());
    assert_eq!(&body[16..20], &1u32.to_le_bytes());
    assert_eq!(body[20], 0);
}

#[test]
fn text_emote_body_includes_target_name_when_known() {
    let character = ActiveCharacter {
        guid: 7,
        name: "Ada".to_string(),
        race: 1,
        class: 1,
        level: 1,
        xp: 0,
        position: WorldPosition::new(0, 1.0, 2.0, 3.0, 4.0),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
        jump: JumpInfo::default(),
    };
    let body = build_text_emote_body(&character, 12, 33, Some("Bert"));

    assert_eq!(&body[16..20], &5u32.to_le_bytes());
    assert_eq!(&body[20..], b"Bert\0");
}

#[test]
fn maps_common_text_emotes_to_animation_emotes() {
    assert_eq!(
        animation_emote_for_text_emote(TEXTEMOTE_WAVE),
        Some(EMOTE_ONESHOT_WAVE)
    );
    assert_eq!(
        animation_emote_for_text_emote(TEXTEMOTE_POINT),
        Some(EMOTE_ONESHOT_POINT)
    );
    assert_eq!(
        animation_emote_for_text_emote(TEXTEMOTE_DANCE),
        Some(EMOTE_STATE_DANCE)
    );
    assert_eq!(
        animation_emote_for_text_emote(TEXTEMOTE_SLEEP),
        Some(EMOTE_STATE_SLEEP)
    );
}

#[test]
fn emote_animation_body_matches_cmangos_command_shape() {
    let character = ActiveCharacter {
        guid: 7,
        name: "Ada".to_string(),
        race: 1,
        class: 1,
        level: 1,
        xp: 0,
        position: WorldPosition::new(0, 1.0, 2.0, 3.0, 4.0),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
        jump: JumpInfo::default(),
    };
    let body = build_emote_body(&character, EMOTE_ONESHOT_WAVE);
    let guid = ObjectGuid::new(HighGuid::Player, 0, 7).raw();

    assert_eq!(&body[0..4], &EMOTE_ONESHOT_WAVE.to_le_bytes());
    assert_eq!(&body[4..12], &guid.to_le_bytes());
}

#[test]
fn emote_state_update_sets_unit_emote_state() {
    let character = ActiveCharacter {
        guid: 7,
        name: "Ada".to_string(),
        race: 1,
        class: 1,
        level: 1,
        xp: 0,
        position: WorldPosition::new(0, 1.0, 2.0, 3.0, 4.0),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
        jump: JumpInfo::default(),
    };
    let body = build_emote_state_update_body(&character, EMOTE_STATE_DANCE).unwrap();
    let packed_guid_mask = body[6];
    let values_start = 4 + 1 + 1 + 1 + packed_guid_mask.count_ones() as usize;
    let values = decode_update_values(&body[values_start..]);

    assert_eq!(&body[0..4], &1u32.to_le_bytes());
    assert_eq!(body[4], 0);
    assert_eq!(body[5], UPDATE_TYPE_VALUES);
    assert_eq!(values[UNIT_NPC_EMOTESTATE], Some(EMOTE_STATE_DANCE));
}

#[tokio::test]
async fn text_emote_broadcasts_text_and_animation_to_nearby_observers() {
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let actor_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8950.0, -140.0, 83.5, 0.0);
    let far_position = WorldPosition::new(0, -8950.0, -200.0, 83.5, 0.0);
    let observer_session = SessionId(2);
    let far_session = SessionId(3);

    maps.add_player(test_player_runtime(1, SessionId(1), actor_position))
        .await
        .unwrap();
    maps.add_player(test_player_runtime(2, observer_session, observer_position))
        .await
        .unwrap();
    maps.add_player(test_player_runtime(3, far_session, far_position))
        .await
        .unwrap();

    let (observer_tx, mut observer_rx) = mpsc::unbounded_channel();
    sessions
        .register(
            observer_session,
            SessionHandle {
                account_id: 2,
                character_guid: Some(2),
                character_name: Some("Bert".to_string()),
                outbound: WorldPacketSender::Unbounded(observer_tx),
                disconnect: None,
            },
        )
        .await;
    let (far_tx, mut far_rx) = mpsc::unbounded_channel();
    sessions
        .register(
            far_session,
            SessionHandle {
                account_id: 3,
                character_guid: Some(3),
                character_name: Some("Clio".to_string()),
                outbound: WorldPacketSender::Unbounded(far_tx),
                disconnect: None,
            },
        )
        .await;

    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 1,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position: actor_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    let mut body = Vec::new();
    body.extend_from_slice(&TEXTEMOTE_WAVE.to_le_bytes());
    body.extend_from_slice(&66u32.to_le_bytes());
    body.extend_from_slice(&ObjectGuid::new(HighGuid::Player, 0, 2).raw().to_le_bytes());

    let (self_tx, mut self_rx) = mpsc::unbounded_channel();
    let mut sink = WorldPacketSink::new(self_tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);

    handle_text_emote(
        &mut sink,
        TextEmoteDeps {
            maps: &maps,
            sessions: &sessions,
        },
        read_text_emote_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let self_animation = self_rx.try_recv().unwrap();
    let self_text = self_rx.try_recv().unwrap();
    assert_eq!(self_animation.opcode, WorldOpcode::SmsgEmote as u16);
    assert_eq!(self_text.opcode, WorldOpcode::SmsgTextEmote as u16);
    assert!(self_text.body.ends_with(b"Bert\0"));

    let observer_animation = observer_rx.try_recv().unwrap();
    let observer_text = observer_rx.try_recv().unwrap();
    assert_eq!(observer_animation.opcode, WorldOpcode::SmsgEmote as u16);
    assert_eq!(observer_animation.body, self_animation.body);
    assert_eq!(observer_text.opcode, WorldOpcode::SmsgTextEmote as u16);
    assert_eq!(observer_text.body, self_text.body);
    assert!(far_rx.try_recv().is_err());
}

#[tokio::test]
async fn text_emote_broadcasts_state_animation_to_nearby_observers() {
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let actor_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8950.0, -140.0, 83.5, 0.0);
    let observer_session = SessionId(2);

    maps.add_player(test_player_runtime(1, SessionId(1), actor_position))
        .await
        .unwrap();
    maps.add_player(test_player_runtime(2, observer_session, observer_position))
        .await
        .unwrap();

    let (observer_tx, mut observer_rx) = mpsc::unbounded_channel();
    sessions
        .register(
            observer_session,
            SessionHandle {
                account_id: 2,
                character_guid: Some(2),
                character_name: Some("Bert".to_string()),
                outbound: WorldPacketSender::Unbounded(observer_tx),
                disconnect: None,
            },
        )
        .await;

    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 1,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position: actor_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    let mut body = Vec::new();
    body.extend_from_slice(&TEXTEMOTE_DANCE.to_le_bytes());
    body.extend_from_slice(&66u32.to_le_bytes());
    body.extend_from_slice(&0u64.to_le_bytes());

    let (self_tx, mut self_rx) = mpsc::unbounded_channel();
    let mut sink = WorldPacketSink::new(self_tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);

    handle_text_emote(
        &mut sink,
        TextEmoteDeps {
            maps: &maps,
            sessions: &sessions,
        },
        read_text_emote_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let self_state = self_rx.try_recv().unwrap();
    let self_text = self_rx.try_recv().unwrap();
    assert_eq!(self_state.opcode, WorldOpcode::SmsgUpdateObject as u16);
    assert_eq!(self_text.opcode, WorldOpcode::SmsgTextEmote as u16);

    let observer_state = observer_rx.try_recv().unwrap();
    let observer_text = observer_rx.try_recv().unwrap();
    assert_eq!(observer_state.opcode, WorldOpcode::SmsgUpdateObject as u16);
    assert_eq!(observer_state.body, self_state.body);
    assert_eq!(observer_text.opcode, WorldOpcode::SmsgTextEmote as u16);
    assert_eq!(observer_text.body, self_text.body);
    assert_eq!(session.character.player_emote_state, EMOTE_STATE_DANCE);
}

#[test]
fn recognizes_observed_movement_opcodes() {
    for opcode in [
        0x00B5, 0x00B7, 0x00B8, 0x00B9, 0x00BA, 0x00BB, 0x00BD, 0x00BE, 0x00C9, 0x00DA, 0x00EE,
    ] {
        assert!(is_movement_opcode(opcode), "opcode 0x{opcode:04X}");
    }
}

#[test]
fn recognizes_expected_world_bootstrap_noise() {
    for opcode in [
        WorldOpcode::CmsgCancelTrade as u32,
        WorldOpcode::CmsgZoneUpdate as u32,
        WorldOpcode::CmsgMeetingStoneInfo as u32,
        WorldOpcode::CmsgRequestRaidInfo as u32,
        WorldOpcode::CmsgMoveTimeSkipped as u32,
        WorldOpcode::CmsgBattlefieldStatus as u32,
    ] {
        assert!(is_expected_noop_opcode(opcode), "opcode 0x{opcode:04X}");
    }

    for opcode in [
        WorldOpcode::CmsgTutorialFlag as u32,
        WorldOpcode::CmsgTutorialClear as u32,
        WorldOpcode::CmsgTutorialReset as u32,
        WorldOpcode::CmsgJoinChannel as u32,
        WorldOpcode::CmsgStandStateChange as u32,
        WorldOpcode::CmsgSetSelection as u32,
        WorldOpcode::CmsgSetTargetObsolete as u32,
    ] {
        assert!(
            !is_expected_noop_opcode(opcode),
            "tutorial opcode 0x{opcode:04X} should be handled, not ignored"
        );
    }
}

#[test]
fn parses_bank_packets() {
    let banker = ObjectGuid::new(HighGuid::Unit, 2456, 99).raw();
    let parsed = packets::parse_world_client_packet(
        WorldOpcode::CmsgBankerActivate as u32,
        &banker.to_le_bytes(),
    )
    .unwrap();
    assert_eq!(parsed.banker_activate().unwrap().banker_raw_guid, banker);

    let parsed = packets::parse_world_client_packet(
        WorldOpcode::CmsgBuyBankSlot as u32,
        &banker.to_le_bytes(),
    )
    .unwrap();
    assert_eq!(parsed.buy_bank_slot().unwrap().banker_raw_guid, banker);

    let parsed =
        packets::parse_world_client_packet(WorldOpcode::CmsgAutobankItem as u32, &[0xFF, 23])
            .unwrap();
    let auto_bank = parsed.auto_bank_item().unwrap();
    assert_eq!(auto_bank.src_bag, CLIENT_INVENTORY_SLOT_BAG_0);
    assert_eq!(auto_bank.src_slot, 23);

    let parsed =
        packets::parse_world_client_packet(WorldOpcode::CmsgAutostoreBankItem as u32, &[63, 0])
            .unwrap();
    let auto_store = parsed.auto_store_bank_item().unwrap();
    assert_eq!(auto_store.src_bag, BANK_SLOT_BAG_START);
    assert_eq!(auto_store.src_slot, 0);
}

#[test]
fn parses_auth_session_packet() {
    let mut payload = Vec::new();
    payload.extend_from_slice(&5875u32.to_le_bytes());
    payload.extend_from_slice(&0u32.to_le_bytes());
    payload.extend_from_slice(b"RUSTAUTH\0");
    payload.extend_from_slice(&0xAABBCCDDu32.to_le_bytes());
    payload.extend_from_slice(&[0x11; 20]);
    payload.extend_from_slice(&[0x22, 0x33]);

    let auth = packets::parse_world_auth_session_packet(&payload).unwrap();
    assert_eq!(auth.client_build, 5875);
    assert_eq!(auth.account, "RUSTAUTH");
    assert_eq!(auth.client_seed, 0xAABBCCDD);
    assert_eq!(auth.digest, [0x11; 20]);
    assert_eq!(auth.addon_data, [0x22, 0x33]);
}
