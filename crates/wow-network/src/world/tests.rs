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

fn update_values_encoded_len(body: &[u8]) -> usize {
    let block_count = body[0] as usize;
    let mask_start = 1;
    let mask_len = block_count * 4;
    let value_count = (0..block_count)
        .map(|block| {
            let mask_offset = mask_start + block * 4;
            u32::from_le_bytes(
                body[mask_offset..mask_offset + 4]
                    .try_into()
                    .expect("update mask block"),
            )
            .count_ones() as usize
        })
        .sum::<usize>();

    mask_start + mask_len + value_count * 4
}

fn decode_values_update_block(block: &[u8], guid: ObjectGuid) -> (Vec<Option<u32>>, &[u8]) {
    assert_eq!(block[0], UPDATE_TYPE_VALUES);
    let values_start = 1 + PackedGuid::packed_size(guid);
    let values_len = update_values_encoded_len(&block[values_start..]);
    (
        decode_update_values(&block[values_start..values_start + values_len]),
        &block[values_start + values_len..],
    )
}

fn decode_create_update_block(
    block: &[u8],
    guid: ObjectGuid,
    type_id: u8,
) -> (Vec<Option<u32>>, &[u8]) {
    assert_eq!(block[0], UPDATE_TYPE_CREATE_OBJECT);
    let type_id_offset = 1 + PackedGuid::packed_size(guid);
    assert_eq!(block[type_id_offset], type_id);
    assert_eq!(block[type_id_offset + 1], UPDATEFLAG_ALL);
    assert_eq!(
        &block[type_id_offset + 2..type_id_offset + 6],
        &1u32.to_le_bytes()
    );

    let values_start = type_id_offset + 6;
    let values_len = update_values_encoded_len(&block[values_start..]);
    (
        decode_update_values(&block[values_start..values_start + values_len]),
        &block[values_start + values_len..],
    )
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
        money: 12345,
        cinematic: 0,
        health: 0,
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
    }
}

fn test_skill(skill: u16, value: u16, max: u16) -> CharacterSkill {
    CharacterSkill { skill, value, max }
}

fn test_item_template(
    entry: u32,
    class: u32,
    inventory_type: u32,
    dmg_min1: f32,
    dmg_max1: f32,
    armor: u32,
) -> ItemTemplateQuery {
    ItemTemplateQuery {
        entry,
        class,
        subclass: 0,
        name: format!("Item {entry}"),
        displayid: 0,
        quality: 0,
        flags: 0,
        buy_price: 0,
        sell_price: 0,
        inventory_type,
        allowable_class: -1,
        allowable_race: -1,
        item_level: 1,
        required_level: 0,
        required_skill: 0,
        required_skill_rank: 0,
        required_spell: 0,
        required_honor_rank: 0,
        required_city_rank: 0,
        required_reputation_faction: 0,
        required_reputation_rank: 0,
        max_count: 0,
        stackable: 1,
        container_slots: 0,
        dmg_min1,
        dmg_max1,
        dmg_type1: 0,
        armor,
        holy_res: 0,
        fire_res: 0,
        nature_res: 0,
        frost_res: 0,
        shadow_res: 0,
        arcane_res: 0,
        delay: 2000,
        ammo_type: 0,
        ranged_mod_range: 0.0,
        bonding: 0,
        description: String::new(),
        page_text: 0,
        language_id: 0,
        page_material: 0,
        start_quest: 0,
        lock_id: 0,
        material: 0,
        sheath: 0,
        random_property: 0,
        block: 0,
        itemset: 0,
        max_durability: 0,
        area: 0,
        map: 0,
        bag_family: 0,
    }
}

fn equipped_template(slot: u8, template: ItemTemplateQuery) -> EquippedItemTemplate {
    EquippedItemTemplate { slot, template }
}

fn test_creature_template(entry: u32) -> CreatureTemplateQuery {
    CreatureTemplateQuery {
        entry,
        name: format!("Creature {entry}"),
        subname: Some("DB Spawn".to_string()),
        min_level: 4,
        max_level: 6,
        display_id1: 123,
        display_id2: 0,
        display_id3: 0,
        display_id4: 0,
        faction: 35,
        scale: 1.0,
        family: 0,
        creature_type: 7,
        npc_flags: UNIT_NPC_FLAG_GOSSIP,
        unit_flags: 0x20,
        dynamic_flags: 0,
        rank: 1,
        min_level_health: 80,
        max_level_health: 120,
        min_melee_dmg: 3.0,
        max_melee_dmg: 5.0,
        min_loot_gold: 2,
        max_loot_gold: 4,
        melee_base_attack_time: 1800,
        ranged_base_attack_time: 2200,
        trainer_type: 0,
        trainer_class: 0,
        pet_spell_data_id: 0,
        civilian: 0,
        experience_multiplier: 1.0,
    }
}

fn test_creature_spawn(entry: u32) -> CreatureSpawnQuery {
    CreatureSpawnQuery {
        guid: 44,
        entry,
        map: 0,
        position_x: -8950.0,
        position_y: -130.0,
        position_z: 83.5,
        orientation: 1.25,
        template: test_creature_template(entry),
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
    let body = build_creature_query_response(RUST_GUIDE_ENTRY, None);
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
        build_creature_query_response(1234, None),
        (1234u32 | 0x8000_0000).to_le_bytes()
    );
}

#[test]
fn parses_unknown_db_creature_query_and_marks_entry_unknown() {
    let guid = ObjectGuid::new(HighGuid::Unit, 98_765, 43_210);
    let mut query = Vec::new();
    query.extend_from_slice(&98_765u32.to_le_bytes());
    query.extend_from_slice(&guid.raw().to_le_bytes());

    let parsed = CreatureQuery::read(&query).unwrap();
    let response = build_creature_query_response(parsed.entry, None);

    assert_eq!(parsed.entry, 98_765);
    assert_eq!(parsed.guid, guid);
    assert_eq!(response, (98_765u32 | 0x8000_0000).to_le_bytes());
}

#[test]
fn db_creature_query_response_uses_world_template_fields() {
    let template = test_creature_template(42);
    let body = build_creature_query_response(42, Some(&template));
    let mut cursor = 0;

    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 42);
    assert_eq!(read_c_string(&body, &mut cursor).unwrap(), "Creature 42");
    cursor += 3;
    assert_eq!(read_c_string(&body, &mut cursor).unwrap(), "DB Spawn");
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 7);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 1);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 123);
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
fn db_vendor_gossip_message_points_at_db_creature() {
    let guid = ObjectGuid::new(HighGuid::Unit, 42, 96_001);
    let body = build_gossip_message(
        guid,
        DB_VENDOR_GOSSIP_TEXT_ID,
        &[(0, DB_VENDOR_GOSSIP_OPTION)],
    );
    assert_eq!(&body[0..8], &guid.raw().to_le_bytes());
    assert_eq!(&body[8..12], &DB_VENDOR_GOSSIP_TEXT_ID.to_le_bytes());
    assert_eq!(&body[12..16], &1u32.to_le_bytes());
    assert_eq!(&body[16..20], &0u32.to_le_bytes());
    assert_eq!(body[20], 0);
    assert_eq!(body[21], 0);
    assert_eq!(&body[22..36], b"Browse goods.\0");
    assert_eq!(&body[36..40], &0u32.to_le_bytes());
}

#[test]
fn parses_gossip_select_option_packet() {
    let guid = ObjectGuid::new(HighGuid::Unit, 42, 96_001);
    let mut body = Vec::new();
    body.extend_from_slice(&guid.raw().to_le_bytes());
    body.extend_from_slice(&1u32.to_le_bytes());
    let selection = GossipSelectOption::read(&body).unwrap();
    assert_eq!(selection.guid, guid);
    assert_eq!(selection.option, 1);
}

#[test]
fn invalid_db_vendor_gossip_option_is_not_the_supported_browse_option() {
    let guid = ObjectGuid::new(HighGuid::Unit, 42, 96_001);
    let mut valid = Vec::new();
    valid.extend_from_slice(&guid.raw().to_le_bytes());
    valid.extend_from_slice(&0u32.to_le_bytes());
    let valid_selection = GossipSelectOption::read(&valid).unwrap();

    let mut invalid = Vec::new();
    invalid.extend_from_slice(&guid.raw().to_le_bytes());
    invalid.extend_from_slice(&1u32.to_le_bytes());
    let invalid_selection = GossipSelectOption::read(&invalid).unwrap();

    assert_eq!(valid_selection.guid, guid);
    assert_eq!(valid_selection.option, 0);
    assert_eq!(invalid_selection.guid, guid);
    assert_eq!(invalid_selection.option, 1);
    assert!(valid_selection.guid.is_creature());
    assert!(invalid_selection.guid.is_creature());
    assert!(valid_selection.is_supported_browse_option());
    assert!(!invalid_selection.is_supported_browse_option());
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
    assert_eq!(
        values[UNIT_NPC_FLAGS],
        Some(UNIT_NPC_FLAG_GOSSIP | UNIT_NPC_FLAG_VENDOR)
    );
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
fn db_creature_create_block_uses_spawn_and_template_fields() {
    let creature = test_creature_spawn(42);
    let block = build_db_creature_create_block(&creature).unwrap();
    let packed_guid_mask = block[1];
    let update_flags_offset = 1 + 1 + packed_guid_mask.count_ones() as usize + 1;
    let values_start = update_flags_offset + 1 + 56;
    let values = decode_update_values(&block[values_start..]);
    let guid = creature_spawn_guid(&creature);

    assert_eq!(block[0], UPDATE_TYPE_CREATE_OBJECT2);
    assert_eq!(
        block[update_flags_offset],
        UPDATEFLAG_ALL | UPDATEFLAG_LIVING | UPDATEFLAG_HAS_POSITION
    );
    assert_eq!(values[0], Some(guid.raw() as u32));
    assert_eq!(values[1], Some((guid.raw() >> 32) as u32));
    assert_eq!(values[2], Some(TYPEMASK_OBJECT_UNIT));
    assert_eq!(values[3], Some(42));
    assert_eq!(values[4], Some(1.0f32.to_bits()));
    assert_eq!(values[UNIT_FIELD_HEALTH], Some(120));
    assert_eq!(values[UNIT_FIELD_MAXHEALTH], Some(120));
    assert_eq!(values[UNIT_FIELD_LEVEL], Some(4));
    assert_eq!(values[UNIT_FIELD_FACTIONTEMPLATE], Some(35));
    assert_eq!(values[UNIT_FIELD_FLAGS], Some(0x20));
    assert_eq!(values[UNIT_FIELD_DISPLAYID], Some(123));
    assert_eq!(values[UNIT_NPC_FLAGS], Some(UNIT_NPC_FLAG_GOSSIP));
}

#[test]
fn db_creature_create_block_defaults_zero_template_scale_to_one() {
    let mut creature = test_creature_spawn(197);
    creature.template.scale = 0.0;

    let block = build_db_creature_create_block(&creature).unwrap();
    let packed_guid_mask = block[1];
    let update_flags_offset = 1 + 1 + packed_guid_mask.count_ones() as usize + 1;
    let values_start = update_flags_offset + 1 + 56;
    let values = decode_update_values(&block[values_start..]);

    assert_eq!(values[4], Some(1.0f32.to_bits()));

    creature.template.scale = 0.7;
    let block = build_db_creature_create_block(&creature).unwrap();
    let packed_guid_mask = block[1];
    let update_flags_offset = 1 + 1 + packed_guid_mask.count_ones() as usize + 1;
    let values_start = update_flags_offset + 1 + 56;
    let values = decode_update_values(&block[values_start..]);

    assert_eq!(values[4], Some(0.7f32.to_bits()));
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
fn raptor_strike_fixture_damage_marks_attacker_state_spell_id() {
    let attacker = ObjectGuid::new(HighGuid::Player, 0, 7);
    let victim = rust_combat_dummy_guid();
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

#[test]
fn quest_xp_reward_uses_cmangos_rew_money_max_level_formula() {
    let mut quest = QuestTemplateQuery {
        entry: 7,
        method: 2,
        zone_or_sort: 12,
        quest_level: 1,
        quest_type: 0,
        rep_objective_faction: 0,
        rep_objective_value: 0,
        next_quest_in_chain: 0,
        rew_or_req_money: 0,
        rew_money_max_level: 210,
        rew_spell: 0,
        rew_spell_cast: 0,
        src_item_id: 0,
        quest_flags: 0,
        title: String::new(),
        details: String::new(),
        objectives: String::new(),
        offer_reward_text: String::new(),
        request_items_text: String::new(),
        end_text: String::new(),
        req_creature_or_go_id: [0; 4],
        req_creature_or_go_count: [0; 4],
        req_item_id: [0; 4],
        req_item_count: [0; 4],
        rew_choice_item_id: [0; 6],
        rew_choice_item_count: [0; 6],
        rew_item_id: [0; 4],
        rew_item_count: [0; 4],
        point_map_id: 0,
        point_x: 0.0,
        point_y: 0.0,
        point_opt: 0,
        details_emote: [0; 4],
        details_emote_delay: [0; 4],
        complete_emote: 0,
        complete_emote_delay: 0,
        incomplete_emote: 0,
        incomplete_emote_delay: 0,
        offer_reward_emote: [0; 4],
        offer_reward_emote_delay: [0; 4],
        objective_text: Default::default(),
    };

    assert_eq!(quest_xp_reward(1, &quest), 350);

    quest.quest_level = 1;
    assert_eq!(quest_xp_reward(10, &quest), 70);
}

#[test]
fn xp_gain_packets_match_vanilla_shapes() {
    let source = ObjectGuid::new(HighGuid::Unit, 6, 44);
    let kill = build_log_xp_gain_body(Some(source), 52);
    assert_eq!(&kill[0..8], &source.raw().to_le_bytes());
    assert_eq!(&kill[8..12], &52u32.to_le_bytes());
    assert_eq!(kill[12], 0);
    assert_eq!(&kill[13..17], &52u32.to_le_bytes());
    assert_eq!(&kill[17..21], &1.0f32.to_le_bytes());

    let quest = build_log_xp_gain_body(None, 350);
    assert_eq!(&quest[0..8], &0u64.to_le_bytes());
    assert_eq!(&quest[8..12], &350u32.to_le_bytes());
    assert_eq!(quest[12], 1);
    assert_eq!(quest.len(), 13);
}

#[test]
fn progression_update_sets_level_xp_vitals_and_stats() {
    let stats = PlayerWorldStats {
        base_health: 29,
        base_mana: 0,
        stats: [24, 21, 23, 20, 21],
        next_level_xp: 900,
    };
    let body = build_player_progression_update_body(PlayerProgressionUpdate {
        character_guid: 7,
        level: 2,
        xp: 2,
        health: stats.max_health(),
        power1: 0,
        power2: POWER_RAGE_DEFAULT,
        power3: 0,
        power4: 0,
        power5: 0,
        world_stats: &stats,
    })
    .unwrap();
    let packed_guid_mask = body[6];
    let values_start = 4 + 1 + 1 + 1 + packed_guid_mask.count_ones() as usize;
    let values = decode_update_values(&body[values_start..]);

    assert_eq!(values[UNIT_FIELD_LEVEL], Some(2));
    assert_eq!(values[UNIT_FIELD_HEALTH], Some(stats.max_health()));
    assert_eq!(values[UNIT_FIELD_MAXHEALTH], Some(stats.max_health()));
    assert_eq!(values[UNIT_FIELD_STAT0], Some(24));
    assert_eq!(values[PLAYER_XP], Some(2));
    assert_eq!(values[PLAYER_NEXT_LEVEL_XP], Some(900));
}

#[test]
fn db_creature_retaliation_reduces_player_health_but_keeps_survivor_floor() {
    let mut session = WorldSessionState {
        player_health: 5,
        ..WorldSessionState::default()
    };
    let creature = test_creature_spawn(299);
    let target = creature_spawn_guid(&creature);
    let expected_hit = DbCreatureRuntime::new(creature).hit_damage().max(1);
    session.db_creatures.insert(
        target.raw(),
        DbCreatureRuntime::new(test_creature_spawn(299)),
    );

    let retaliation = retaliation_damage_for_db_creature(&mut session, target);
    assert_eq!(retaliation, expected_hit);
    assert_eq!(
        session.player_health,
        (5u32)
            .saturating_sub(expected_hit)
            .max(PLAYER_SURVIVOR_HEALTH_FLOOR)
    );

    session.player_health = 1;
    let retaliation = retaliation_damage_for_db_creature(&mut session, target);
    assert_eq!(retaliation, expected_hit);
    assert_eq!(session.player_health, PLAYER_SURVIVOR_HEALTH_FLOOR);
}

#[test]
fn combat_dummy_loot_packets_match_empty_corpse_shape() {
    let loot = build_combat_dummy_loot_response_body(&WorldSessionState::default());
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
fn combat_dummy_loot_packets_include_fixture_money_and_item() {
    let session = WorldSessionState {
        combat_dummy_loot_money_available: true,
        combat_dummy_loot_item_available: true,
        ..WorldSessionState::default()
    };
    let loot = build_combat_dummy_loot_response_body(&session);
    assert_eq!(&loot[0..8], &rust_combat_dummy_guid().raw().to_le_bytes());
    assert_eq!(loot[8], CLIENT_LOOT_CORPSE);
    assert_eq!(&loot[9..13], &RUST_COMBAT_DUMMY_LOOT_MONEY.to_le_bytes());
    assert_eq!(loot[13], 1);
    assert_eq!(loot[14], 0);
    assert_eq!(&loot[15..19], &RUST_COMBAT_DUMMY_LOOT_ITEM.to_le_bytes());
    assert_eq!(
        &loot[19..23],
        &RUST_COMBAT_DUMMY_LOOT_ITEM_COUNT.to_le_bytes()
    );
    assert_eq!(
        &loot[23..27],
        &RUST_COMBAT_DUMMY_LOOT_ITEM_DISPLAY.to_le_bytes()
    );
    assert_eq!(loot[35], LOOT_SLOT_NORMAL);
}

#[test]
fn rust_guide_vendor_inventory_lists_bag_and_stack_item() {
    let body = build_rust_guide_vendor_inventory();
    assert_eq!(&body[0..8], &rust_guide_guid().raw().to_le_bytes());
    assert_eq!(body[8], 2);
    assert_eq!(&body[9..13], &1u32.to_le_bytes());
    assert_eq!(&body[13..17], &RUST_VENDOR_BAG_ITEM.to_le_bytes());
    assert_eq!(&body[17..21], &RUST_VENDOR_BAG_DISPLAY.to_le_bytes());
    assert_eq!(&body[37..41], &2u32.to_le_bytes());
    assert_eq!(&body[41..45], &RUST_COMBAT_DUMMY_LOOT_ITEM.to_le_bytes());
    assert_eq!(
        &body[45..49],
        &RUST_COMBAT_DUMMY_LOOT_ITEM_DISPLAY.to_le_bytes()
    );
}

#[test]
fn db_vendor_inventory_uses_cmangos_list_shape() {
    let guid = ObjectGuid::new(HighGuid::Unit, 42, 96_001);
    let db_items = [
        wow_db::VendorItemQuery {
            item: RUST_COMBAT_DUMMY_LOOT_ITEM,
            max_count: 0,
            slot: 7,
            display_id: RUST_COMBAT_DUMMY_LOOT_ITEM_DISPLAY,
            buy_price: 3,
            max_durability: 0,
            buy_count: 2,
            container_slots: 0,
        },
        wow_db::VendorItemQuery {
            item: RUST_VENDOR_BAG_ITEM,
            max_count: 5,
            slot: 9,
            display_id: RUST_VENDOR_BAG_DISPLAY,
            buy_price: 10,
            max_durability: 20,
            buy_count: 1,
            container_slots: 6,
        },
    ];
    let items: Vec<VendorListItem> = db_items.iter().map(Into::into).collect();
    let body = build_vendor_inventory_body(guid, &items);

    assert_eq!(&body[0..8], &guid.raw().to_le_bytes());
    assert_eq!(body[8], 2);
    assert_eq!(&body[9..13], &1u32.to_le_bytes());
    assert_eq!(&body[13..17], &RUST_COMBAT_DUMMY_LOOT_ITEM.to_le_bytes());
    assert_eq!(&body[21..25], &u32::MAX.to_le_bytes());
    assert_eq!(&body[25..29], &3u32.to_le_bytes());
    assert_eq!(&body[33..37], &2u32.to_le_bytes());
    assert_eq!(&body[37..41], &2u32.to_le_bytes());
    assert_eq!(&body[41..45], &RUST_VENDOR_BAG_ITEM.to_le_bytes());
    assert_eq!(&body[49..53], &5u32.to_le_bytes());
    assert_eq!(&body[53..57], &10u32.to_le_bytes());
    assert_eq!(&body[57..61], &20u32.to_le_bytes());
}

#[test]
fn trainer_list_uses_cmangos_spell_row_shape() {
    let guid = ObjectGuid::new(HighGuid::Unit, 951, 44);
    let spells = [
        TrainerListSpell {
            spell: 772,
            learned_spell: 772,
            state: TRAINER_SPELL_GREEN,
            cost: 10,
            req_level: 4,
            req_skill: 0,
            req_skill_value: 0,
            req_ability: [78, 0, 0],
        },
        TrainerListSpell {
            spell: 6546,
            learned_spell: 6546,
            state: TRAINER_SPELL_RED,
            cost: 100,
            req_level: 10,
            req_skill: 0,
            req_skill_value: 0,
            req_ability: [772, 0, 0],
        },
    ];
    let body = build_trainer_list_body(guid, 0, &spells, "Train well.");

    assert_eq!(&body[0..8], &guid.raw().to_le_bytes());
    assert_eq!(&body[8..12], &0u32.to_le_bytes());
    assert_eq!(&body[12..16], &2u32.to_le_bytes());
    assert_eq!(&body[16..20], &772u32.to_le_bytes());
    assert_eq!(body[20], TRAINER_SPELL_GREEN);
    assert_eq!(&body[21..25], &10u32.to_le_bytes());
    assert_eq!(body[33], 4);
    assert_eq!(&body[34..38], &0u32.to_le_bytes());
    assert_eq!(&body[38..42], &0u32.to_le_bytes());
    assert_eq!(&body[42..46], &78u32.to_le_bytes());
    let second = 16 + 38;
    assert_eq!(&body[second..second + 4], &6546u32.to_le_bytes());
    assert_eq!(body[second + 4], TRAINER_SPELL_RED);
    assert_eq!(&body[body.len() - 12..], b"Train well.\0");
}

#[test]
fn trainer_spell_state_marks_known_level_and_requirement_gates() {
    let character = ActiveCharacter {
        guid: 7,
        name: "Ada".to_string(),
        race: 1,
        class: 1,
        level: 4,
        xp: 0,
        position: WorldPosition::new(0, 1.0, 2.0, 3.0, 4.0),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
    };
    let known = [wow_db::CharacterSpell {
        spell: 78,
        active: 1,
        disabled: 0,
    }];
    let available = wow_db::TrainerSpellQuery {
        spell: 772,
        learned_spell: 772,
        spell_cost: 10,
        req_skill: 0,
        req_skill_value: 0,
        req_level: 4,
        req_ability1: Some(78),
        req_ability2: None,
        req_ability3: None,
    };
    let too_high = wow_db::TrainerSpellQuery {
        req_level: 5,
        ..available.clone()
    };
    let known_spell = wow_db::TrainerSpellQuery {
        spell: 78,
        learned_spell: 78,
        ..available.clone()
    };
    let known_trainer_cast = wow_db::TrainerSpellQuery {
        spell: 6674,
        learned_spell: 6673,
        ..available.clone()
    };

    assert_eq!(
        TrainerListSpell::from_query(&available, &character, &known).state,
        TRAINER_SPELL_GREEN
    );
    assert_eq!(
        TrainerListSpell::from_query(&too_high, &character, &known).state,
        TRAINER_SPELL_RED
    );
    assert_eq!(
        TrainerListSpell::from_query(&known_spell, &character, &known).state,
        TRAINER_SPELL_GRAY
    );
    assert_eq!(
        TrainerListSpell::from_query(
            &known_trainer_cast,
            &character,
            &[wow_db::CharacterSpell {
                spell: 6673,
                active: 1,
                disabled: 0,
            }]
        )
        .state,
        TRAINER_SPELL_GRAY
    );
}

#[test]
fn trainer_buy_packets_match_vanilla_shapes() {
    let guid = ObjectGuid::new(HighGuid::Unit, 951, 44);
    let mut request = Vec::new();
    request.extend_from_slice(&guid.raw().to_le_bytes());
    request.extend_from_slice(&772u32.to_le_bytes());
    let parsed = TrainerBuySpellRequest::read(&request).unwrap();
    assert_eq!(parsed.trainer_guid, guid);
    assert_eq!(parsed.spell, 772);

    let success = build_trainer_buy_succeeded_body(guid, 772);
    assert_eq!(success.len(), 12);
    assert_eq!(&success[0..8], &guid.raw().to_le_bytes());
    assert_eq!(&success[8..12], &772u32.to_le_bytes());
    let failed = build_trainer_buy_failed_body(guid, 772, 2);
    assert_eq!(failed.len(), 16);
    assert_eq!(&failed[12..16], &2u32.to_le_bytes());
    let learned = build_learned_spell_body(6673);
    assert_eq!(learned.len(), 4);
    assert_eq!(&learned, &6673u32.to_le_bytes());
}

#[test]
fn empty_vendor_inventory_marks_no_inventory() {
    let guid = ObjectGuid::new(HighGuid::Unit, 42, 96_001);
    let body = build_vendor_inventory_body(guid, &[]);
    assert_eq!(&body[0..8], &guid.raw().to_le_bytes());
    assert_eq!(body[8], 0);
    assert_eq!(body[9], 0);
}

#[test]
fn parses_rust_guide_buy_item_packet() {
    let mut body = Vec::new();
    body.extend_from_slice(&rust_guide_guid().raw().to_le_bytes());
    body.extend_from_slice(&RUST_VENDOR_BAG_ITEM.to_le_bytes());
    body.push(1);
    body.push(1);
    let buy = BuyItemRequest::read(&body).unwrap();
    assert_eq!(buy.vendor_guid, rust_guide_guid());
    assert_eq!(buy.item, RUST_VENDOR_BAG_ITEM);
    assert_eq!(buy.count, 1);
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
fn starter_spell_support_covers_warrior_and_hunter_active_spells() {
    assert_eq!(
        supported_starter_spell(WARRIOR_HEROIC_STRIKE_RANK_1),
        Some(SupportedStarterSpell {
            damage: HEROIC_STRIKE_FIXTURE_DAMAGE,
            power: StarterSpellPower::Rage {
                cost: HEROIC_STRIKE_RAGE_COST
            },
        })
    );
    assert_eq!(
        supported_starter_spell(HUNTER_RAPTOR_STRIKE_RANK_1),
        Some(SupportedStarterSpell {
            damage: RAPTOR_STRIKE_FIXTURE_DAMAGE,
            power: StarterSpellPower::Mana {
                cost: RAPTOR_STRIKE_MANA_COST
            },
        })
    );
    assert_eq!(supported_starter_spell(1), None);
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
        &skills,
        &std::collections::HashMap::new(),
        &equipped,
    )
    .unwrap();
    let values = decode_update_values(&body);
    let expected_melee_ap =
        class_melee_attack_power(character.class, character.level as u32, 23, 20);
    let expected_ranged_ap = class_ranged_attack_power(character.class, character.level as u32, 20);
    let expected_main_bonus = expected_melee_ap as f32 / 14.0 * 2.0;

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
    assert_eq!(values[UNIT_FIELD_BYTES_2], Some(unit_bytes_2()));
    assert_eq!(values[PLAYER_FIELD_COINAGE], Some(12345));
    assert_eq!(values[PLAYER_FIELD_POSSTAT0], Some(0.0f32.to_bits()));
    assert_eq!(values[PLAYER_FIELD_NEGSTAT0], Some(0.0f32.to_bits()));
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
        &[],
        &std::collections::HashMap::new(),
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
        &[],
        &std::collections::HashMap::new(),
        &[],
    )
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
    assert_eq!(
        inventory_slot_update_field(19),
        Some(PLAYER_FIELD_INV_SLOT_HEAD + 38)
    );
    assert_eq!(inventory_slot_update_field(39), None);
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
    let swap_inv = InventoryMoveRequest::read(CMSG_SWAP_INV_ITEM, &[23, 24]).unwrap();
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
        CMSG_SWAP_ITEM,
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
    let unequip = InventoryMoveRequest::read(CMSG_SWAP_INV_ITEM, &[3, 26]).unwrap();
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
    assert!(!item_fits_equipment_slot(4, 15));
}

#[test]
fn parses_bag_container_inventory_move_packets() {
    let into_bag =
        InventoryMoveRequest::read(CMSG_SWAP_ITEM, &[19, 0, CLIENT_INVENTORY_SLOT_BAG_0, 24])
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

    let within_bag = InventoryMoveRequest::read(CMSG_SWAP_ITEM, &[19, 1, 19, 0]).unwrap();
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
            durability: 0,
        },
        CharacterInventoryItem {
            bag: 19,
            slot: 3,
            item: 99,
            item_template: RUST_COMBAT_DUMMY_LOOT_ITEM,
            count: 2,
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
            durability: 0,
        },
        CharacterInventoryItem {
            bag: 0,
            slot: 27,
            item: 99,
            item_template: RUST_COMBAT_DUMMY_LOOT_ITEM,
            count: 2,
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
fn backpack_stack_merge_update_clears_source_slot_and_updates_destination_count() {
    let character_guid = 11;
    let source_guid = ObjectGuid::new(HighGuid::Item, 0, 88);
    let destination_guid = ObjectGuid::new(HighGuid::Item, 0, 99);
    let inventory = [CharacterInventoryItem {
        bag: 0,
        slot: 26,
        item: 99,
        item_template: RUST_COMBAT_DUMMY_LOOT_ITEM,
        count: 7,
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
            durability: 0,
        },
        CharacterInventoryItem {
            bag: 19,
            slot: 2,
            item: 99,
            item_template: RUST_COMBAT_DUMMY_LOOT_ITEM,
            count: 7,
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
            durability: 0,
        },
        CharacterInventoryItem {
            bag: 0,
            slot: 24,
            item: 42,
            item_template: RUST_COMBAT_DUMMY_LOOT_ITEM,
            count: 4,
            durability: 0,
        },
        CharacterInventoryItem {
            bag: 19,
            slot: 1,
            item: 99,
            item_template: RUST_COMBAT_DUMMY_LOOT_ITEM,
            count: 2,
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
    assert_eq!(destination_values[0x003], Some(RUST_COMBAT_DUMMY_LOOT_ITEM));
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
                &[],
                &std::collections::HashMap::new(),
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
        race: 1,
        class: 1,
        level: 1,
        xp: 0,
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
fn raptor_strike_starter_spell_packets_match_success_shapes() {
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    let target = rust_combat_dummy_guid();
    let targets = normalize_fixture_spell_targets(SpellCastTargets {
        target_mask: SPELL_CAST_TARGET_UNIT_ENEMY,
        unit_target: Some(target),
    });

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
        race: 1,
        class: 1,
        level: 1,
        xp: 0,
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
        race: 1,
        class: 1,
        level: 1,
        xp: 0,
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
