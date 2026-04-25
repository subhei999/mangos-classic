use super::*;

fn decode_update_values(body: &[u8]) -> Vec<Option<u32>> {
    let block_count = body[0] as usize;
    let mask_start = 1;
    let mut value_cursor = mask_start + block_count * 4;
    let mut values = vec![None; block_count * 32];

    for (index, value_slot) in values.iter_mut().enumerate() {
        let mask_offset = mask_start + (index / 32) * 4;
        let mask = u32::from_le_bytes(
            body[mask_offset..mask_offset + 4]
                .try_into()
                .expect("update mask block"),
        );
        if mask & (1 << (index % 32)) == 0 {
            continue;
        }

        let value = u32::from_le_bytes(
            body[value_cursor..value_cursor + 4]
                .try_into()
                .expect("update value"),
        );
        *value_slot = Some(value);
        value_cursor += 4;
    }

    values
}

fn test_character(race: u8, class: u8) -> CharacterEnumEntry {
    CharacterEnumEntry {
        guid: 7,
        name: "Ada".to_string(),
        race,
        class,
        gender: 0,
        player_bytes: 0x0403_0201,
        player_bytes2: 5,
        level: 1,
        zone: 12,
        map: 0,
        position_x: -8949.95,
        position_y: -132.493,
        position_z: 83.5312,
        orientation: 0.0,
        guildid: None,
        player_flags: 0,
        at_login: 0,
        money: 12345,
        cinematic: 0,
        health: 0,
        power1: 0,
        power2: 0,
        power3: 0,
        power4: 0,
        power5: 0,
        watched_faction: u32::MAX,
        pet_entry: None,
        pet_modelid: None,
        pet_level: None,
        equipment_cache: None,
    }
}

#[test]
fn server_packet_header_matches_world_shape() {
    let mut packet = Vec::new();
    packet.extend_from_slice(&(4u16 + 2).to_be_bytes());
    packet.extend_from_slice(&SMSG_AUTH_CHALLENGE.to_le_bytes());
    packet.extend_from_slice(&SERVER_SEED.to_le_bytes());

    assert_eq!(&packet[0..2], &[0x00, 0x06]);
    assert_eq!(&packet[2..4], &[0xEC, 0x01]);
    assert_eq!(packet.len(), 8);
}

#[test]
fn empty_char_enum_packet_shape() {
    let body = build_char_enum_body(&[]).unwrap();
    let mut packet = Vec::new();
    packet.extend_from_slice(&(body.len() as u16 + 2).to_be_bytes());
    packet.extend_from_slice(&SMSG_CHAR_ENUM.to_le_bytes());
    packet.extend_from_slice(&body);

    assert_eq!(packet, [0x00, 0x03, 0x3B, 0x00, 0x00]);
}

#[test]
fn parses_char_create_packet() {
    let mut body = Vec::new();
    body.extend_from_slice(b"Testname\0");
    body.extend_from_slice(&[1, 1, 0, 2, 3, 4, 5, 6, 0]);

    let packet = CharCreatePacket::read(&body).unwrap();

    assert_eq!(packet.name, "Testname");
    assert_eq!(packet.race, 1);
    assert_eq!(packet.class, 1);
    assert_eq!(packet.gender, 0);
    assert_eq!(packet.skin, 2);
    assert_eq!(packet.face, 3);
    assert_eq!(packet.hair_style, 4);
    assert_eq!(packet.hair_color, 5);
    assert_eq!(packet.facial_hair, 6);
    assert_eq!(packet.outfit_id, 0);
}

#[test]
fn name_query_response_matches_cmangos_shape() {
    let character = CharacterNameQuery {
        guid: 7,
        name: "Rusty".to_string(),
        race: 1,
        gender: 0,
        class: 1,
    };
    let body = build_name_query_response(7, Some(&character));

    assert_eq!(&body[0..8], &7u64.to_le_bytes());
    assert_eq!(&body[8..14], b"Rusty\0");
    assert_eq!(body[14], 0);
    assert_eq!(&body[15..19], &1u32.to_le_bytes());
    assert_eq!(&body[19..23], &0u32.to_le_bytes());
    assert_eq!(&body[23..27], &1u32.to_le_bytes());
}

#[test]
fn creature_query_response_matches_cmangos_shape() {
    let body = build_creature_query_response(RUST_GUIDE_ENTRY);
    let mut cursor = 0;

    assert_eq!(read_u32(&body, &mut cursor).unwrap(), RUST_GUIDE_ENTRY);
    assert_eq!(read_c_string(&body, &mut cursor).unwrap(), RUST_GUIDE_NAME);
    assert_eq!(body[cursor], 0);
    assert_eq!(body[cursor + 1], 0);
    assert_eq!(body[cursor + 2], 0);
    cursor += 3;
    assert_eq!(
        read_c_string(&body, &mut cursor).unwrap(),
        RUST_GUIDE_SUBNAME
    );
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 7);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), RUST_GUIDE_DISPLAY_ID);
    assert_eq!(&body[cursor..cursor + 2], &0u16.to_le_bytes());
}

#[test]
fn missing_creature_query_marks_entry_unknown() {
    assert_eq!(
        build_creature_query_response(1234),
        (1234u32 | 0x8000_0000).to_le_bytes()
    );
}

#[test]
fn rust_guide_gossip_message_has_empty_menu_shape() {
    let body = build_rust_guide_gossip_message();
    let option_start = 16;
    let option_text_start = option_start + 4 + 1 + 1;

    assert_eq!(&body[0..8], &rust_guide_guid().raw().to_le_bytes());
    assert_eq!(&body[8..12], &RUST_GUIDE_GOSSIP_TEXT_ID.to_le_bytes());
    assert_eq!(&body[12..16], &1u32.to_le_bytes());
    assert_eq!(&body[option_start..option_start + 4], &0u32.to_le_bytes());
    assert_eq!(body[option_start + 4], 0);
    assert_eq!(body[option_start + 5], 0);
    assert_eq!(
        &body[option_text_start..option_text_start + 12],
        b"Keep going.\0"
    );
    assert_eq!(
        &body[option_text_start + 12..option_text_start + 16],
        &0u32.to_le_bytes()
    );
}

#[test]
fn rust_guide_npc_text_update_matches_cmangos_eight_option_shape() {
    let body = build_rust_guide_npc_text_update(RUST_GUIDE_GOSSIP_TEXT_ID);
    let mut cursor = 0;

    assert_eq!(
        read_u32(&body, &mut cursor).unwrap(),
        RUST_GUIDE_GOSSIP_TEXT_ID
    );
    assert_eq!(
        f32::from_le_bytes(body[cursor..cursor + 4].try_into().unwrap()),
        1.0
    );
    cursor += 4;
    assert_eq!(
        read_c_string(&body, &mut cursor).unwrap(),
        RUST_GUIDE_GOSSIP_TEXT
    );
    assert_eq!(
        read_c_string(&body, &mut cursor).unwrap(),
        RUST_GUIDE_GOSSIP_TEXT
    );
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
    for _ in 0..3 {
        assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
        assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
    }

    for _ in 1..8 {
        assert_eq!(
            f32::from_le_bytes(body[cursor..cursor + 4].try_into().unwrap()),
            0.0
        );
        cursor += 4;
        assert_eq!(read_c_string(&body, &mut cursor).unwrap(), "");
        assert_eq!(read_c_string(&body, &mut cursor).unwrap(), "");
        assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
        for _ in 0..3 {
            assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
            assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
        }
    }
    assert_eq!(cursor, body.len());
}

#[test]
fn rust_guide_create_block_has_gossip_unit_fields() {
    let character = test_character(1, 1);
    let block = build_rust_guide_create_block(&character).unwrap();
    let packed_guid_mask = block[1];
    let update_flags_offset = 1 + 1 + packed_guid_mask.count_ones() as usize + 1;
    let values_start = update_flags_offset + 1 + 56;
    let values = decode_update_values(&block[values_start..]);

    assert_eq!(block[0], UPDATE_TYPE_CREATE_OBJECT2);
    assert_eq!(
        block[update_flags_offset],
        UPDATEFLAG_ALL | UPDATEFLAG_LIVING | UPDATEFLAG_HAS_POSITION
    );
    assert_eq!(values[0], Some(rust_guide_guid().raw() as u32));
    assert_eq!(values[1], Some((rust_guide_guid().raw() >> 32) as u32));
    assert_eq!(values[2], Some(TYPEMASK_OBJECT_UNIT));
    assert_eq!(values[3], Some(RUST_GUIDE_ENTRY));
    assert_eq!(values[UNIT_FIELD_DISPLAYID], Some(RUST_GUIDE_DISPLAY_ID));
    assert_eq!(values[UNIT_NPC_FLAGS], Some(UNIT_NPC_FLAG_GOSSIP));
}

#[test]
fn rust_combat_dummy_create_block_has_hostile_unit_fields() {
    let character = test_character(1, 1);
    let block = build_rust_combat_dummy_create_block(&character).unwrap();
    let packed_guid_mask = block[1];
    let update_flags_offset = 1 + 1 + packed_guid_mask.count_ones() as usize + 1;
    let values_start = update_flags_offset + 1 + 56;
    let values = decode_update_values(&block[values_start..]);

    assert_eq!(block[0], UPDATE_TYPE_CREATE_OBJECT2);
    assert_eq!(
        block[update_flags_offset],
        UPDATEFLAG_ALL | UPDATEFLAG_LIVING | UPDATEFLAG_HAS_POSITION
    );
    assert_eq!(values[0], Some(rust_combat_dummy_guid().raw() as u32));
    assert_eq!(
        values[1],
        Some((rust_combat_dummy_guid().raw() >> 32) as u32)
    );
    assert_eq!(values[2], Some(TYPEMASK_OBJECT_UNIT));
    assert_eq!(values[3], Some(RUST_COMBAT_DUMMY_ENTRY));
    assert_eq!(values[UNIT_FIELD_HEALTH], Some(RUST_COMBAT_DUMMY_HEALTH));
    assert_eq!(
        values[UNIT_FIELD_DISPLAYID],
        Some(RUST_COMBAT_DUMMY_DISPLAY_ID)
    );
    assert_eq!(
        values[UNIT_FIELD_FACTIONTEMPLATE],
        Some(RUST_COMBAT_DUMMY_FACTION_TEMPLATE)
    );
}

#[test]
fn combat_packets_match_cmangos_melee_shapes() {
    let attacker = ObjectGuid::new(HighGuid::Player, 0, 7);
    let victim = rust_combat_dummy_guid();

    let start = build_attack_start_body(attacker, victim);
    assert_eq!(&start[0..8], &attacker.raw().to_le_bytes());
    assert_eq!(&start[8..16], &victim.raw().to_le_bytes());

    let stop = build_attack_stop_body(attacker, victim, false).unwrap();
    assert_eq!(&stop[stop.len() - 4..], &0u32.to_le_bytes());

    let state =
        build_attacker_state_update_body(attacker, victim, RUST_COMBAT_DUMMY_HIT_DAMAGE).unwrap();
    let mut cursor = 0;
    assert_eq!(read_u32(&state, &mut cursor).unwrap(), HITINFO_NORMALSWING2);
    cursor += PackedGuid::packed_size(attacker) + PackedGuid::packed_size(victim);
    assert_eq!(
        read_u32(&state, &mut cursor).unwrap(),
        RUST_COMBAT_DUMMY_HIT_DAMAGE
    );
    assert_eq!(state[cursor], 1);
    cursor += 1;
    assert_eq!(read_u32(&state, &mut cursor).unwrap(), 0);
    assert_eq!(
        f32::from_le_bytes(state[cursor..cursor + 4].try_into().unwrap()),
        RUST_COMBAT_DUMMY_HIT_DAMAGE as f32
    );
    cursor += 4;
    assert_eq!(
        read_u32(&state, &mut cursor).unwrap(),
        RUST_COMBAT_DUMMY_HIT_DAMAGE
    );
    assert_eq!(read_u32(&state, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&state, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&state, &mut cursor).unwrap(), VICTIMSTATE_NORMAL);
}

#[test]
fn heroic_strike_fixture_damage_marks_attacker_state_spell_id() {
    let attacker = ObjectGuid::new(HighGuid::Player, 0, 7);
    let victim = rust_combat_dummy_guid();
    let state = build_attacker_state_update_body_with_spell_id(
        attacker,
        victim,
        HEROIC_STRIKE_FIXTURE_DAMAGE,
        WARRIOR_HEROIC_STRIKE_RANK_1,
    )
    .unwrap();
    let mut cursor = 0;
    assert_eq!(read_u32(&state, &mut cursor).unwrap(), HITINFO_NORMALSWING2);
    cursor += PackedGuid::packed_size(attacker) + PackedGuid::packed_size(victim);
    assert_eq!(
        read_u32(&state, &mut cursor).unwrap(),
        HEROIC_STRIKE_FIXTURE_DAMAGE
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
        WARRIOR_HEROIC_STRIKE_RANK_1
    );
}

#[test]
fn combat_dummy_state_update_sets_health_and_dynamic_flags() {
    let body = build_combat_dummy_state_update_body(20, UNIT_DYNFLAG_LOOTABLE).unwrap();
    let packed_guid_mask = body[6];
    let values_start = 4 + 1 + 1 + 1 + packed_guid_mask.count_ones() as usize;
    let values = decode_update_values(&body[values_start..]);

    assert_eq!(&body[0..4], &1u32.to_le_bytes());
    assert_eq!(body[4], 0);
    assert_eq!(body[5], UPDATE_TYPE_VALUES);
    assert_eq!(values[UNIT_FIELD_HEALTH], Some(20));
    assert_eq!(values[UNIT_DYNAMIC_FLAGS], Some(UNIT_DYNFLAG_LOOTABLE));
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
fn combat_dummy_loot_packets_match_empty_corpse_shape() {
    let loot = build_combat_dummy_loot_response_body();
    assert_eq!(&loot[0..8], &rust_combat_dummy_guid().raw().to_le_bytes());
    assert_eq!(loot[8], CLIENT_LOOT_CORPSE);
    assert_eq!(&loot[9..13], &0u32.to_le_bytes());
    assert_eq!(loot[13], 0);

    let release = build_loot_release_response_body(rust_combat_dummy_guid(), true);
    assert_eq!(
        &release[0..8],
        &rust_combat_dummy_guid().raw().to_le_bytes()
    );
    assert_eq!(release[8], 1);
}

#[test]
fn combat_session_tracks_active_dummy_target_and_loot_state() {
    let mut session = WorldSessionState::default();
    assert_eq!(session.active_combat_target, None);
    assert!(!session.combat_dummy_lootable);
    assert!(!session.combat_dummy_looting);

    session.active_combat_target = Some(rust_combat_dummy_guid());
    session.combat_dummy_lootable = true;
    session.combat_dummy_looting = true;
    session.player_rage = HEROIC_STRIKE_RAGE_COST;
    assert_eq!(session.active_combat_target, Some(rust_combat_dummy_guid()));
    assert!(session.combat_dummy_lootable);
    assert!(session.combat_dummy_looting);
    assert_eq!(session.player_rage, HEROIC_STRIKE_RAGE_COST);

    session.active_combat_target = None;
    session.combat_dummy_lootable = false;
    session.combat_dummy_looting = false;
    session.player_rage = 0;
    assert_eq!(session.active_combat_target, None);
    assert!(!session.combat_dummy_lootable);
    assert!(!session.combat_dummy_looting);
    assert_eq!(session.player_rage, 0);
}

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
        health: 20,
        power1: 0,
        power2: 0,
        power3: 0,
        power4: 0,
        power5: 0,
        watched_faction: u32::MAX,
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
        health: 20,
        power1: 0,
        power2: 0,
        power3: 0,
        power4: 0,
        power5: 0,
        watched_faction: u32::MAX,
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
        health: 20,
        power1: 0,
        power2: 0,
        power3: 0,
        power4: 0,
        power5: 0,
        watched_faction: u32::MAX,
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
    let character = test_character(1, 1);

    let mut body = Vec::new();
    let world_stats = PlayerWorldStats {
        base_health: 20,
        base_mana: 0,
        stats: [23, 20, 22, 20, 21],
        next_level_xp: 400,
    };

    write_minimal_player_update_values(&mut body, guid, &character, &[], &world_stats).unwrap();
    let values = decode_update_values(&body);

    assert_eq!(values[UNIT_FIELD_HEALTH], Some(60));
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
    assert_eq!(values[PLAYER_NEXT_LEVEL_XP], Some(400));
    assert_eq!(values[UNIT_FIELD_BYTES_2], Some(unit_bytes_2()));
    assert_eq!(values[PLAYER_FIELD_COINAGE], Some(12345));
    assert_eq!(values[PLAYER_FIELD_WATCHED_FACTION_INDEX], Some(u32::MAX));
    assert_eq!(
        values[PLAYER_FIELD_MOD_DAMAGE_DONE_PCT],
        Some(1.0f32.to_bits())
    );
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
    write_minimal_player_update_values(&mut body, guid, &test_character(7, 8), &[], &mage_stats)
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
    write_minimal_player_update_values(&mut body, guid, &test_character(1, 4), &[], &rogue_stats)
        .unwrap();
    let values = decode_update_values(&body);
    assert_eq!(values[UNIT_FIELD_POWER4], Some(POWER_ENERGY_DEFAULT));
    assert_eq!(values[UNIT_FIELD_MAXPOWER4], Some(POWER_ENERGY_DEFAULT));
    assert_eq!(values[UNIT_FIELD_MAXPOWER2], Some(0));
}

#[test]
fn initial_reputations_packet_matches_cmangos_empty_shape() {
    let body = build_initial_reputations_body(&[]);

    assert_eq!(body.len(), 4 + REPUTATION_LIST_SLOTS * 5);
    assert_eq!(&body[0..4], &(REPUTATION_LIST_SLOTS as u32).to_le_bytes());
    assert!(body[4..].iter().all(|byte| *byte == 0));
}

#[test]
fn initial_reputations_packet_ignores_rows_without_dbc_slot_mapping() {
    let body = build_initial_reputations_body(&[
        CharacterReputation {
            faction: 72,
            standing: 0,
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

    assert!(body[4..].iter().all(|byte| *byte == 0));
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
fn tutorial_flags_packet_serializes_account_state() {
    let body = build_tutorial_flags_body(&[1, 0x8000_0000, 0x0102_0304, 0, 0, 0, 0, 0xFFFF_FFFF]);

    assert_eq!(body.len(), 8 * 4);
    assert_eq!(&body[0..4], &1u32.to_le_bytes());
    assert_eq!(&body[4..8], &0x8000_0000u32.to_le_bytes());
    assert_eq!(&body[8..12], &0x0102_0304u32.to_le_bytes());
    assert_eq!(&body[28..32], &0xFFFF_FFFFu32.to_le_bytes());
}

#[test]
fn account_data_times_packet_matches_placeholder_shape() {
    let body = build_account_data_times_body();

    assert_eq!(body.len(), ACCOUNT_DATA_TYPES * MD5_DIGEST_LEN);
    assert!(body.iter().all(|byte| *byte == 0));
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
    assert_eq!(inventory_slot_update_field(40), None);
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
        durability: 10,
    };

    set_inventory_slot_update_values(&mut values, &[item]).unwrap();

    let guid = ObjectGuid::new(HighGuid::Item, 0, 42);
    let field = PLAYER_FIELD_INV_SLOT_HEAD + 15 * 2;
    assert_eq!(values[field], Some(guid.raw() as u32));
    assert_eq!(values[field + 1], Some((guid.raw() >> 32) as u32));
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
        health: 20,
        power1: 0,
        power2: 0,
        power3: 0,
        power4: 0,
        power5: 0,
        watched_faction: u32::MAX,
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
            durability: 18,
        },
        CharacterInventoryItem {
            bag: 0,
            slot: 24,
            item: 41,
            item_template: 6948,
            count: 1,
            durability: 0,
        },
    ];

    let blocks = build_inventory_item_create_blocks(&character, &items).unwrap();

    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0][0], UPDATE_TYPE_CREATE_OBJECT);
    assert_eq!(blocks[0][4], TYPEID_ITEM);
    assert_eq!(blocks[0][5], UPDATEFLAG_ALL);
    assert_eq!(blocks[1][0], UPDATE_TYPE_CREATE_OBJECT);
    assert_eq!(blocks[1][4], TYPEID_ITEM);
    assert_eq!(blocks[1][5], UPDATEFLAG_ALL);
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
        health: 20,
        power1: 0,
        power2: 0,
        power3: 0,
        power4: 0,
        power5: 0,
        watched_faction: u32::MAX,
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
}

#[test]
fn movement_info_rejects_truncated_payload() {
    let err = MovementInfo::read(&[0; 8]).unwrap_err().to_string();
    assert!(err.contains("movement packet truncated"));
}

#[test]
fn active_mover_rejects_truncated_payload() {
    let err = handle_set_active_mover(&[0; 4], &WorldSessionState::default())
        .unwrap_err()
        .to_string();
    assert!(err.contains("CMSG_SET_ACTIVE_MOVER payload must be 8 bytes"));
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
        position: WorldPosition::new(0, 1.0, 2.0, 3.0, 4.0),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
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
    let target = rust_combat_dummy_guid();
    let mut body = Vec::new();
    body.extend_from_slice(&WARRIOR_HEROIC_STRIKE_RANK_1.to_le_bytes());
    body.extend_from_slice(&SPELL_CAST_TARGET_UNIT.to_le_bytes());
    PackedGuid::write(&mut body, target).unwrap();

    let cast = CastSpellPacket::read(&body).unwrap();

    assert_eq!(cast.spell_id, WARRIOR_HEROIC_STRIKE_RANK_1);
    assert_eq!(cast.targets.target_mask, SPELL_CAST_TARGET_UNIT);
    assert_eq!(cast.targets.unit_target, Some(target));
}

#[test]
fn starter_spell_packets_match_cmangos_success_shapes() {
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    let target = rust_combat_dummy_guid();
    let targets = SpellCastTargets {
        target_mask: SPELL_CAST_TARGET_UNIT,
        unit_target: Some(target),
    };

    let result = build_cast_result_ok_body(WARRIOR_HEROIC_STRIKE_RANK_1);
    assert_eq!(&result[0..4], &WARRIOR_HEROIC_STRIKE_RANK_1.to_le_bytes());
    assert_eq!(result[4], 0);

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
    assert_eq!(
        read_packed_guid(&go, &mut cursor).unwrap(),
        rust_combat_dummy_guid()
    );
    assert_eq!(cursor, go.len());
}

#[test]
fn text_emote_body_matches_cmangos_empty_target_shape() {
    let character = ActiveCharacter {
        guid: 7,
        name: "Ada".to_string(),
        position: WorldPosition::new(0, 1.0, 2.0, 3.0, 4.0),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
    };
    let body = build_text_emote_body(&character, 12, 33);
    let guid = ObjectGuid::new(HighGuid::Player, 0, 7).raw();

    assert_eq!(&body[0..8], &guid.to_le_bytes());
    assert_eq!(&body[8..12], &12u32.to_le_bytes());
    assert_eq!(&body[12..16], &33u32.to_le_bytes());
    assert_eq!(&body[16..20], &1u32.to_le_bytes());
    assert_eq!(body[20], 0);
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
        position: WorldPosition::new(0, 1.0, 2.0, 3.0, 4.0),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
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
        position: WorldPosition::new(0, 1.0, 2.0, 3.0, 4.0),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
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
        CMSG_JOIN_CHANNEL,
        CMSG_CANCEL_TRADE,
        CMSG_ZONEUPDATE,
        CMSG_MEETINGSTONE_INFO,
        CMSG_REQUEST_RAID_INFO,
        CMSG_MOVE_TIME_SKIPPED,
        CMSG_BATTLEFIELD_STATUS,
    ] {
        assert!(is_expected_noop_opcode(opcode), "opcode 0x{opcode:04X}");
    }

    for opcode in [CMSG_TUTORIAL_FLAG, CMSG_TUTORIAL_CLEAR, CMSG_TUTORIAL_RESET] {
        assert!(
            !is_expected_noop_opcode(opcode),
            "tutorial opcode 0x{opcode:04X} should be handled, not ignored"
        );
    }
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

    let auth = AuthSessionPacket::read(&payload).unwrap();
    assert_eq!(auth.client_build, 5875);
    assert_eq!(auth.account, "RUSTAUTH");
    assert_eq!(auth.client_seed, 0xAABBCCDD);
    assert_eq!(auth.digest, [0x11; 20]);
    assert_eq!(auth.addon_data, [0x22, 0x33]);
}
