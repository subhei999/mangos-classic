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
        detection_range: 20,
        call_for_help: 0,
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
        movement_type: DB_MOTION_TYPE_IDLE,
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
        spawn_dist: 0.0,
        movement_type: DB_MOTION_TYPE_IDLE,
        template: test_creature_template(entry),
        waypoint_path: Vec::new(),
    }
}

fn test_waypoint(point: u32, x: f32, y: f32, wait_time: u32) -> wow_db::CreatureWaypointQuery {
    wow_db::CreatureWaypointQuery {
        point,
        position_x: x,
        position_y: y,
        position_z: 83.5,
        orientation: None,
        wait_time,
        script_id: 0,
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
fn movement_visibility_stages_only_new_db_creature_create_blocks() {
    let known = test_creature_spawn(197);
    let new_spawn = CreatureSpawnQuery {
        guid: 45,
        entry: 6,
        position_x: -8790.0,
        position_y: -95.0,
        ..test_creature_spawn(6)
    };
    let known_guid = creature_spawn_guid(&known).raw();
    let new_guid = creature_spawn_guid(&new_spawn).raw();
    let mut session = WorldSessionState::default();
    session
        .db_creatures
        .insert(known_guid, DbCreatureRuntime::new(known.clone()));

    let updates =
        stage_db_creature_visibility_updates(&mut session, vec![known, new_spawn]).unwrap();
    let bodies = updates.create_bodies;

    assert_eq!(bodies.len(), 1);
    assert!(updates.destroy_guids.is_empty());
    assert!(session.db_creatures.contains_key(&known_guid));
    assert!(session.db_creatures.contains_key(&new_guid));
    assert!(
        bodies[0]
            .windows(new_guid.to_le_bytes().len())
            .any(|window| window == new_guid.to_le_bytes()),
        "new creature create block was missing from movement visibility update"
    );
    assert!(
        !bodies[0]
            .windows(known_guid.to_le_bytes().len())
            .any(|window| window == known_guid.to_le_bytes()),
        "already visible creature should not be recreated"
    );
}

#[test]
fn movement_visibility_stages_destroy_for_out_of_range_db_creatures() {
    let nearby = test_creature_spawn(197);
    let out_of_range = CreatureSpawnQuery {
        guid: 45,
        entry: 6,
        position_x: -8790.0,
        position_y: -95.0,
        ..test_creature_spawn(6)
    };
    let nearby_guid = creature_spawn_guid(&nearby).raw();
    let out_of_range_guid = creature_spawn_guid(&out_of_range);
    let mut session = WorldSessionState::default();
    session
        .db_creatures
        .insert(nearby_guid, DbCreatureRuntime::new(nearby.clone()));
    session.db_creatures.insert(
        out_of_range_guid.raw(),
        DbCreatureRuntime::new(out_of_range),
    );

    let updates = stage_db_creature_visibility_updates(&mut session, vec![nearby]).unwrap();

    assert!(updates.create_bodies.is_empty());
    assert_eq!(updates.destroy_guids, vec![out_of_range_guid]);
    assert!(session.db_creatures.contains_key(&nearby_guid));
    assert!(!session.db_creatures.contains_key(&out_of_range_guid.raw()));
    assert_eq!(session.active_combat_target, None);
    assert!(session.active_creature_combats.is_empty());
    assert_eq!(
        build_destroy_guid_body(out_of_range_guid),
        out_of_range_guid.raw().to_le_bytes()
    );
}

#[test]
fn movement_visibility_retains_out_of_query_active_combat_creature() {
    let nearby = test_creature_spawn(197);
    let out_of_query = CreatureSpawnQuery {
        guid: 45,
        entry: 6,
        position_x: -8790.0,
        position_y: -95.0,
        ..test_creature_spawn(6)
    };
    let nearby_guid = creature_spawn_guid(&nearby).raw();
    let out_of_query_guid = creature_spawn_guid(&out_of_query);
    let mut session = WorldSessionState::default();
    session
        .db_creatures
        .insert(nearby_guid, DbCreatureRuntime::new(nearby.clone()));
    session.db_creatures.insert(
        out_of_query_guid.raw(),
        DbCreatureRuntime::new(out_of_query),
    );
    session.active_combat_target = Some(out_of_query_guid);
    session.active_creature_combats.insert(
        out_of_query_guid.raw(),
        CreatureCombatState {
            attacker: out_of_query_guid,
            victim: ObjectGuid::new(HighGuid::Player, 0, 7),
            next_swing_at: Instant::now(),
        },
    );

    let updates = stage_db_creature_visibility_updates(&mut session, vec![nearby]).unwrap();

    assert!(updates.create_bodies.is_empty());
    assert!(updates.destroy_guids.is_empty());
    assert!(session.db_creatures.contains_key(&nearby_guid));
    assert!(session.db_creatures.contains_key(&out_of_query_guid.raw()));
    assert_eq!(session.active_combat_target, Some(out_of_query_guid));
    assert!(!session.active_creature_combats.is_empty());
}

#[test]
fn movement_visibility_rescan_uses_distance_threshold() {
    let session = WorldSessionState {
        last_creature_visibility_position: Some(WorldPosition::new(0, 10.0, 10.0, 0.0, 0.0)),
        ..WorldSessionState::default()
    };

    assert!(!should_rescan_db_creature_visibility(
        &session,
        WorldPosition::new(0, 20.0, 10.0, 0.0, 0.0),
    ));
    assert!(should_rescan_db_creature_visibility(
        &session,
        WorldPosition::new(
            0,
            10.0 + CREATURE_VISIBILITY_RESCAN_DISTANCE_YARDS,
            10.0,
            0.0,
            0.0,
        ),
    ));
    assert!(should_rescan_db_creature_visibility(
        &session,
        WorldPosition::new(1, 10.0, 10.0, 0.0, 0.0),
    ));
}

#[test]
fn world_tick_timeout_is_due_when_deadline_passed() {
    let now = Instant::now();
    assert_eq!(
        world_tick_timeout_duration(now + Duration::from_millis(10), now),
        Duration::from_millis(10)
    );
    assert_eq!(
        world_tick_timeout_duration(now, now + Duration::from_millis(1)),
        Duration::ZERO
    );
}

#[test]
fn world_tick_deadline_advances_past_now() {
    let now = Instant::now();
    let mut next = now;

    advance_world_tick_deadline(
        &mut next,
        now + Duration::from_millis(WORLD_TICK_MILLIS * 2 + 1),
    );

    assert!(next > now + Duration::from_millis(WORLD_TICK_MILLIS * 2 + 1));
    assert_eq!(next, now + Duration::from_millis(WORLD_TICK_MILLIS * 3));
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
        4.0
    );
    cursor += 12;
    assert_eq!(
        f32::from_le_bytes(body[cursor..cursor + 4].try_into().unwrap()),
        7.0
    );
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
fn db_creature_attack_distance_matches_cmangos_level_delta_shape() {
    assert_eq!(db_creature_attack_distance(1, 1, 20), 20.0);
    assert_eq!(db_creature_attack_distance(1, 3, 20), 22.0);
    assert_eq!(db_creature_attack_distance(10, 1, 20), 11.0);
    assert_eq!(db_creature_attack_distance(60, 1, 20), 5.0);
    assert_eq!(db_creature_attack_distance(1, 60, 20), 45.0);
    assert_eq!(db_creature_attack_distance(1, 1, 0), 0.0);
}

#[test]
fn db_creature_aggro_selects_nearest_hostile_in_range() {
    let character = ActiveCharacter {
        guid: 7,
        name: "Ada".to_string(),
        race: 1,
        class: 1,
        level: 1,
        xp: 0,
        position: WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
    };
    let mut far_hostile = test_creature_spawn(6);
    far_hostile.guid = 45;
    far_hostile.position_x = -8931.0;
    far_hostile.template.faction = RUST_COMBAT_DUMMY_FACTION_TEMPLATE;
    far_hostile.template.npc_flags = 0;
    far_hostile.template.min_level = 1;
    let mut near_hostile = test_creature_spawn(6);
    near_hostile.guid = 46;
    near_hostile.position_x = -8940.0;
    near_hostile.template.faction = RUST_COMBAT_DUMMY_FACTION_TEMPLATE;
    near_hostile.template.npc_flags = 0;
    near_hostile.template.min_level = 1;
    let mut friendly = test_creature_spawn(197);
    friendly.guid = 47;
    friendly.position_x = -8945.0;
    friendly.template.faction = RUST_GUIDE_FACTION_TEMPLATE;
    friendly.template.npc_flags = UNIT_NPC_FLAG_GOSSIP;
    let mut session = WorldSessionState {
        active_character: Some(character),
        ..WorldSessionState::default()
    };
    for creature in [far_hostile.clone(), near_hostile.clone(), friendly] {
        let runtime = DbCreatureRuntime::new(creature);
        session.db_creatures.insert(runtime.guid().raw(), runtime);
    }

    assert_eq!(
        select_db_creature_aggro_target(&session),
        Some(creature_spawn_guid(&near_hostile))
    );
    assert_eq!(
        select_db_creature_aggro_targets(&session),
        vec![
            creature_spawn_guid(&near_hostile),
            creature_spawn_guid(&far_hostile)
        ]
    );
}

#[test]
fn db_creature_combat_can_track_multiple_attackers() {
    let character = ActiveCharacter {
        guid: 7,
        name: "Ada".to_string(),
        race: 1,
        class: 1,
        level: 1,
        xp: 0,
        position: WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
    };
    let first = creature_spawn_guid(&test_creature_spawn(6));
    let mut second_spawn = test_creature_spawn(38);
    second_spawn.guid = 46;
    let second = creature_spawn_guid(&second_spawn);
    let now = Instant::now();
    let mut session = WorldSessionState {
        active_character: Some(character),
        ..WorldSessionState::default()
    };

    assert!(begin_db_creature_combat(&mut session, first, now));
    assert!(begin_db_creature_combat(
        &mut session,
        second,
        now + Duration::from_millis(10)
    ));
    assert_eq!(session.active_creature_combats.len(), 2);
    assert!(!begin_db_creature_combat(
        &mut session,
        first,
        now + Duration::from_secs(1)
    ));

    clear_db_creature_combat_if_attacker(&mut session, first);
    assert!(!session.active_creature_combats.contains_key(&first.raw()));
    assert!(session.active_creature_combats.contains_key(&second.raw()));
}

#[test]
fn db_creature_aggro_uses_template_detection_range() {
    let character = ActiveCharacter {
        guid: 7,
        name: "Ada".to_string(),
        race: 1,
        class: 1,
        level: 1,
        xp: 0,
        position: WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
    };
    let mut kobold = test_creature_spawn(6);
    kobold.guid = 45;
    kobold.position_x = -8931.0;
    kobold.template.faction = RUST_COMBAT_DUMMY_FACTION_TEMPLATE;
    kobold.template.npc_flags = 0;
    kobold.template.min_level = 1;
    kobold.template.detection_range = 18;
    let mut session = WorldSessionState {
        active_character: Some(character),
        ..WorldSessionState::default()
    };
    let runtime = DbCreatureRuntime::new(kobold.clone());
    session.db_creatures.insert(runtime.guid().raw(), runtime);

    assert_eq!(select_db_creature_aggro_target(&session), None);

    session.db_creatures.clear();
    kobold.template.detection_range = 20;
    let runtime = DbCreatureRuntime::new(kobold.clone());
    session.db_creatures.insert(runtime.guid().raw(), runtime);

    assert_eq!(
        select_db_creature_aggro_target(&session),
        Some(creature_spawn_guid(&kobold))
    );
}

#[test]
fn db_creature_player_melee_check_requires_range_and_facing() {
    let mut kobold = test_creature_spawn(6);
    kobold.guid = 45;
    kobold.position_x = 4.0;
    kobold.position_y = 0.0;
    kobold.position_z = 0.0;
    kobold.template.npc_flags = 0;
    let target = creature_spawn_guid(&kobold);
    let character = ActiveCharacter {
        guid: 7,
        name: "Ada".to_string(),
        race: 1,
        class: 1,
        level: 1,
        xp: 0,
        position: WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
    };
    let mut session = WorldSessionState {
        active_character: Some(character),
        ..WorldSessionState::default()
    };
    let runtime = DbCreatureRuntime::new(kobold);
    session.db_creatures.insert(runtime.guid().raw(), runtime);

    assert_eq!(
        db_creature_player_melee_check(&session, target),
        PlayerMeleeCheck::Clear
    );

    session
        .active_character
        .as_mut()
        .unwrap()
        .position
        .orientation = std::f32::consts::PI;
    assert_eq!(
        db_creature_player_melee_check(&session, target),
        PlayerMeleeCheck::BadFacing
    );

    session
        .active_character
        .as_mut()
        .unwrap()
        .position
        .orientation = 0.0;
    session
        .db_creatures
        .get_mut(&target.raw())
        .unwrap()
        .current_position
        .x = 5.0;
    assert_eq!(
        db_creature_player_melee_check(&session, target),
        PlayerMeleeCheck::OutOfRange
    );

    session
        .db_creatures
        .get_mut(&target.raw())
        .unwrap()
        .current_position
        .x = 4.0;
    session
        .db_creatures
        .get_mut(&target.raw())
        .unwrap()
        .current_position
        .z = 4.0;
    assert_eq!(
        db_creature_player_melee_check(&session, target),
        PlayerMeleeCheck::OutOfRange
    );
}

#[test]
fn db_creature_player_melee_check_uses_navigation_guardrail() {
    let mut kobold = test_creature_spawn(6);
    kobold.guid = 45;
    kobold.position_x = 4.0;
    kobold.position_y = 0.0;
    kobold.position_z = 0.0;
    kobold.template.npc_flags = 0;
    let target = creature_spawn_guid(&kobold);
    let character = ActiveCharacter {
        guid: 7,
        name: "Ada".to_string(),
        race: 1,
        class: 1,
        level: 1,
        xp: 0,
        position: WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
    };
    let mut session = WorldSessionState {
        active_character: Some(character),
        db_creature_navigation: DbCreatureNavigationGuardrail {
            line_of_sight_clear: false,
            path_available: true,
            ..DbCreatureNavigationGuardrail::default()
        },
        ..WorldSessionState::default()
    };
    let runtime = DbCreatureRuntime::new(kobold);
    session.db_creatures.insert(runtime.guid().raw(), runtime);

    assert_eq!(
        db_creature_player_melee_check(&session, target),
        PlayerMeleeCheck::NavigationBlocked(DbCreatureNavigationResult::LineOfSightBlocked)
    );
}

#[test]
fn starter_melee_spell_failure_uses_melee_validity_before_damage() {
    let mut kobold = test_creature_spawn(6);
    kobold.guid = 45;
    kobold.position_x = 8.0;
    kobold.position_y = 0.0;
    kobold.position_z = 0.0;
    kobold.template.npc_flags = 0;
    let target = creature_spawn_guid(&kobold);
    let starter_spell = supported_starter_spell(WARRIOR_HEROIC_STRIKE_RANK_1).unwrap();
    let targets = SpellCastTargets {
        target_mask: SPELL_CAST_TARGET_UNIT,
        unit_target: Some(target),
    };
    let character = ActiveCharacter {
        guid: 7,
        name: "Ada".to_string(),
        race: 1,
        class: 1,
        level: 1,
        xp: 0,
        position: WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
    };
    let mut session = WorldSessionState {
        active_character: Some(character),
        ..WorldSessionState::default()
    };
    let runtime = DbCreatureRuntime::new(kobold);
    session.db_creatures.insert(runtime.guid().raw(), runtime);

    assert_eq!(
        starter_spell_melee_cast_failure(&session, &starter_spell, &targets),
        Some(SPELL_FAILED_OUT_OF_RANGE)
    );

    session
        .db_creatures
        .get_mut(&target.raw())
        .unwrap()
        .current_position
        .x = 4.0;
    session
        .active_character
        .as_mut()
        .unwrap()
        .position
        .orientation = std::f32::consts::PI;
    assert_eq!(
        starter_spell_melee_cast_failure(&session, &starter_spell, &targets),
        Some(SPELL_FAILED_UNIT_NOT_INFRONT)
    );

    session
        .active_character
        .as_mut()
        .unwrap()
        .position
        .orientation = 0.0;
    assert_eq!(
        starter_spell_melee_cast_failure(&session, &starter_spell, &targets),
        None
    );
}

#[test]
fn db_creature_aggro_ignores_friendly_critter_lootable_and_out_of_range_units() {
    let character = ActiveCharacter {
        guid: 7,
        name: "Ada".to_string(),
        race: 1,
        class: 1,
        level: 20,
        xp: 0,
        position: WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
    };
    let mut friendly = test_creature_spawn(197);
    friendly.guid = 45;
    friendly.position_x = -8945.0;
    friendly.template.faction = RUST_GUIDE_FACTION_TEMPLATE;
    friendly.template.npc_flags = UNIT_NPC_FLAG_GOSSIP;
    let mut critter = test_creature_spawn(6);
    critter.guid = 46;
    critter.position_x = -8945.0;
    critter.template.faction = RUST_COMBAT_DUMMY_FACTION_TEMPLATE;
    critter.template.npc_flags = 0;
    critter.template.creature_type = CREATURE_TYPE_CRITTER;
    let mut out_of_range = test_creature_spawn(6);
    out_of_range.guid = 47;
    out_of_range.position_x = -8930.0;
    out_of_range.template.faction = RUST_COMBAT_DUMMY_FACTION_TEMPLATE;
    out_of_range.template.npc_flags = 0;
    out_of_range.template.min_level = 1;
    let mut lootable = test_creature_spawn(300);
    lootable.guid = 48;
    lootable.position_x = -8945.0;
    lootable.template.faction = RUST_COMBAT_DUMMY_FACTION_TEMPLATE;
    lootable.template.npc_flags = 0;
    let mut lootable_runtime = DbCreatureRuntime::new(lootable);
    lootable_runtime.health = 0;
    lootable_runtime.lootable = true;
    let mut session = WorldSessionState {
        active_character: Some(character),
        ..WorldSessionState::default()
    };
    for creature in [friendly, critter, out_of_range] {
        let runtime = DbCreatureRuntime::new(creature);
        session.db_creatures.insert(runtime.guid().raw(), runtime);
    }
    session
        .db_creatures
        .insert(lootable_runtime.guid().raw(), lootable_runtime);

    assert_eq!(select_db_creature_aggro_target(&session), None);
}

#[test]
fn db_creature_aggro_ignores_unknown_faction_templates() {
    let character = ActiveCharacter {
        guid: 7,
        name: "Ada".to_string(),
        race: 1,
        class: 1,
        level: 1,
        xp: 0,
        position: WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
    };
    let mut guard = test_creature_spawn(197);
    guard.guid = 45;
    guard.position_x = -8945.0;
    guard.template.faction = 9_999;
    guard.template.npc_flags = 0;
    guard.template.creature_type = 7;
    let runtime = DbCreatureRuntime::new(guard);
    let mut session = WorldSessionState {
        active_character: Some(character),
        ..WorldSessionState::default()
    };
    session.db_creatures.insert(runtime.guid().raw(), runtime);

    assert_eq!(select_db_creature_aggro_target(&session), None);
}

#[test]
fn db_creature_aggro_ignores_neutral_young_wolves() {
    let character = ActiveCharacter {
        guid: 7,
        name: "Ada".to_string(),
        race: 1,
        class: 1,
        level: 1,
        xp: 0,
        position: WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
    };
    let mut wolf = test_creature_spawn(299);
    wolf.guid = 45;
    wolf.position_x = -8950.5;
    wolf.position_y = -130.0;
    wolf.template.faction = 32;
    wolf.template.npc_flags = 0;
    wolf.template.creature_type = 7;
    let runtime = DbCreatureRuntime::new(wolf);
    let mut session = WorldSessionState {
        active_character: Some(character),
        ..WorldSessionState::default()
    };
    session.db_creatures.insert(runtime.guid().raw(), runtime);

    assert_eq!(select_db_creature_aggro_target(&session), None);
}

#[test]
fn db_creature_aggro_includes_real_defias_thugs() {
    let character = ActiveCharacter {
        guid: 7,
        name: "Ada".to_string(),
        race: 1,
        class: 1,
        level: 2,
        xp: 0,
        position: WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
    };
    let mut defias = test_creature_spawn(38);
    defias.guid = 45;
    defias.position_x = -8951.0;
    defias.position_y = -130.0;
    defias.template.faction = 17;
    defias.template.npc_flags = 0;
    defias.template.creature_type = 7;
    defias.template.min_level = 2;
    let defias_guid = creature_spawn_guid(&defias);
    let runtime = DbCreatureRuntime::new(defias);
    let mut session = WorldSessionState {
        active_character: Some(character),
        ..WorldSessionState::default()
    };
    session.db_creatures.insert(runtime.guid().raw(), runtime);

    assert_eq!(select_db_creature_aggro_target(&session), Some(defias_guid));
}

#[test]
fn db_creature_aggro_uses_cmangos_faction_template_reactions() {
    assert_eq!(
        faction_reaction_to(17, 1),
        FactionReaction::Hostile,
        "Defias Thug faction should be hostile to Alliance players"
    );
    assert_eq!(
        faction_reaction_to(25, 1),
        FactionReaction::Hostile,
        "Kobold faction should be hostile to Alliance players"
    );
    assert_eq!(
        faction_reaction_to(32, 1),
        FactionReaction::Neutral,
        "Young Wolf faction should not auto-aggro"
    );
    assert_eq!(
        faction_reaction_to(12, 1),
        FactionReaction::Friendly,
        "Northshire friendly NPCs should not auto-aggro Alliance players"
    );
    assert!(!can_faction_attack_on_sight(9_999, 1));
}

#[test]
fn db_creature_combat_state_tracks_victim_and_next_swing() {
    let attacker = creature_spawn_guid(&test_creature_spawn(299));
    let now = Instant::now();
    let mut session = WorldSessionState {
        active_character: Some(ActiveCharacter {
            guid: 7,
            name: "Ada".to_string(),
            race: 1,
            class: 1,
            level: 1,
            xp: 0,
            position: WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
            movement_flags: 0,
            client_time: 0,
            fall_time: 0,
        }),
        ..WorldSessionState::default()
    };

    assert!(begin_db_creature_combat(&mut session, attacker, now));

    let combat = session
        .active_creature_combats
        .get(&attacker.raw())
        .copied()
        .expect("creature combat state should start");
    assert_eq!(combat.attacker, attacker);
    assert_eq!(combat.victim, ObjectGuid::new(HighGuid::Player, 0, 7));
    assert_eq!(combat.next_swing_at, now);

    let later = now + Duration::from_secs(1);
    assert!(!begin_db_creature_combat(&mut session, attacker, later));
    let combat = session
        .active_creature_combats
        .get(&attacker.raw())
        .copied()
        .expect("creature combat state should stay active");
    assert_eq!(
        combat.next_swing_at, now,
        "incoming player hits must not reset the creature swing timer"
    );

    let overdue = now + Duration::from_secs(3);
    defer_ready_db_creature_swing_retry(
        &mut session,
        attacker,
        ObjectGuid::new(HighGuid::Player, 0, 7),
        overdue,
    );
    let combat = session
        .active_creature_combats
        .get(&attacker.raw())
        .copied()
        .expect("creature combat state should stay active");
    assert_eq!(
        combat.next_swing_at,
        overdue + Duration::from_millis(DB_CREATURE_MELEE_RETRY_MILLIS),
        "out-of-range ready swings should retry shortly instead of staying overdue"
    );

    clear_db_creature_combat_if_attacker(&mut session, attacker);
    assert!(session.active_creature_combats.is_empty());
}

#[test]
fn db_creature_melee_reach_is_position_gated() {
    let mut creature = test_creature_spawn(299);
    creature.position_x = -8950.0;
    creature.position_y = -130.0;
    creature.orientation = 0.0;
    let target = creature_spawn_guid(&creature);
    let mut session = WorldSessionState {
        active_character: Some(ActiveCharacter {
            guid: 7,
            name: "Ada".to_string(),
            race: 1,
            class: 1,
            level: 1,
            xp: 0,
            position: WorldPosition::new(
                0,
                -8950.0 + DB_CREATURE_MELEE_REACH_YARDS - 0.1,
                -130.0,
                83.5,
                0.0,
            ),
            movement_flags: 0,
            client_time: 0,
            fall_time: 0,
        }),
        ..WorldSessionState::default()
    };
    session
        .db_creatures
        .insert(target.raw(), DbCreatureRuntime::new(creature));

    assert!(db_creature_can_reach_player(&session, target));
    assert!(db_creature_has_player_in_arc(&session, target));
    session.active_character.as_mut().unwrap().position.x =
        -8950.0 - DB_CREATURE_MELEE_REACH_YARDS + 0.1;
    assert!(db_creature_can_reach_player(&session, target));
    assert!(!db_creature_has_player_in_arc(&session, target));
    let (facing_position, spline_id) =
        face_db_creature_toward_player(&mut session, target).expect("creature should face player");
    assert_eq!(spline_id, 0);
    assert_eq!(facing_position.x, -8950.0);
    assert!(db_creature_has_player_in_arc(&session, target));
    assert_eq!(
        session
            .db_creatures
            .get(&target.raw())
            .expect("creature should stay loaded")
            .next_spline_id,
        1
    );

    session.active_character.as_mut().unwrap().position.x =
        -8950.0 + DB_CREATURE_MELEE_REACH_YARDS + 0.1;
    assert!(!db_creature_can_reach_player(&session, target));
}

#[test]
fn db_creature_navigation_guardrail_blocks_aggro_chase_and_melee() {
    let mut creature = test_creature_spawn(6);
    creature.position_x = 0.0;
    creature.position_y = 0.0;
    creature.position_z = 0.0;
    creature.template.npc_flags = 0;
    creature.template.min_level = 1;
    let attacker = creature_spawn_guid(&creature);
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let mut session = WorldSessionState {
        active_character: Some(ActiveCharacter {
            guid: 7,
            name: "Ada".to_string(),
            race: 1,
            class: 1,
            level: 1,
            xp: 0,
            position: WorldPosition::new(0, 1.0, 0.0, 0.0, 0.0),
            movement_flags: 0,
            client_time: 0,
            fall_time: 0,
        }),
        db_creature_navigation: DbCreatureNavigationGuardrail {
            line_of_sight_clear: false,
            path_available: true,
            ..DbCreatureNavigationGuardrail::default()
        },
        ..WorldSessionState::default()
    };
    session
        .db_creatures
        .insert(attacker.raw(), DbCreatureRuntime::new(creature));

    assert_eq!(select_db_creature_aggro_target(&session), None);
    assert!(!db_creature_can_reach_player(&session, attacker));
    session.active_character.as_mut().unwrap().position.x = 10.0;
    assert!(
        start_db_creature_chase_motion(&mut session, attacker, player, Instant::now()).is_none()
    );
}

#[test]
fn db_creature_navigation_guardrail_reports_invalid_path_inputs() {
    let navigation = DbCreatureNavigationGuardrail::default();
    let start = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    let target = WorldPosition::new(0, 10.0, 0.0, 0.0, 0.0);

    assert_eq!(
        db_creature_navigation_check(&navigation, start, target),
        DbCreatureNavigationResult::Clear
    );
    assert_eq!(
        db_creature_navigation_check(
            &navigation,
            start,
            WorldPosition::new(1, 10.0, 0.0, 0.0, 0.0)
        ),
        DbCreatureNavigationResult::MapMismatch
    );
    assert_eq!(
        db_creature_navigation_check(
            &navigation,
            start,
            WorldPosition::new(0, f32::NAN, 0.0, 0.0, 0.0)
        ),
        DbCreatureNavigationResult::InvalidCoordinate
    );
    assert_eq!(
        db_creature_navigation_check(
            &DbCreatureNavigationGuardrail {
                line_of_sight_clear: true,
                path_available: false,
                ..DbCreatureNavigationGuardrail::default()
            },
            start,
            target
        ),
        DbCreatureNavigationResult::PathUnavailable
    );
}

#[test]
fn mmap_tile_for_position_matches_cmangos_grid_files() {
    assert_eq!(
        mmap_tile_for_position(WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0)),
        Some((48, 32))
    );
    assert_eq!(
        mmap_tile_for_position(WorldPosition::new(0, f32::NAN, 0.0, 0.0, 0.0)),
        None
    );
}

#[test]
fn db_creature_navigation_uses_mmap_tile_availability_when_loaded() {
    let navigation = DbCreatureNavigationGuardrail {
        world_data_files: Arc::new(WorldDataFiles {
            data_dir: std::path::PathBuf::from("fixture"),
            data_dir_for_native: None,
            maps_available: true,
            vmaps_available: true,
            mmap_headers: HashSet::from([0]),
            mmap_tiles: HashSet::from([(0, 48, 32)]),
        }),
        ..DbCreatureNavigationGuardrail::default()
    };
    let northshire = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let same_tile = WorldPosition::new(0, -8948.0, -132.0, 83.5, 0.0);
    let missing_tile = WorldPosition::new(0, -8400.0, -130.0, 83.5, 0.0);

    assert_eq!(
        db_creature_navigation_check(&navigation, northshire, same_tile),
        DbCreatureNavigationResult::Clear
    );
    assert_eq!(
        db_creature_navigation_check(&navigation, northshire, missing_tile),
        DbCreatureNavigationResult::PathUnavailable
    );
}

#[test]
fn db_creature_mmap_path_corner_uses_local_detour_data_when_available() {
    let data = Arc::new(WorldDataFiles::inspect("C:/World of Warcraft Classic"));
    if !data.has_mmap_tile(0, 48, 32) {
        return;
    }
    let navigation = DbCreatureNavigationGuardrail {
        world_data_files: data,
        ..DbCreatureNavigationGuardrail::default()
    };
    let start = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let target = WorldPosition::new(0, -8940.0, -130.0, 83.5, 0.0);
    let (start_tile_x, start_tile_y) = mmap_tile_for_position(start).unwrap();
    let (target_tile_x, target_tile_y) = mmap_tile_for_position(target).unwrap();
    let data_dir = std::ffi::CString::new("C:/World of Warcraft Classic").unwrap();
    let mut native_points = [NativeMmapPathPoint {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    }; 16];
    let native_count = unsafe {
        wow_mmap_find_path(
            data_dir.as_ptr(),
            0,
            start_tile_x,
            start_tile_y,
            target_tile_x,
            target_tile_y,
            start.x,
            start.y,
            start.z,
            target.x,
            target.y,
            target.z,
            native_points.as_mut_ptr(),
            native_points.len() as i32,
        )
    };

    let corner = db_creature_mmap_next_path_corner(&navigation, start, target).unwrap_or_else(|| {
        panic!(
            "local Northshire mmap should produce a Detour path corner; native_count={native_count}, tiles={:?}->{:?}",
            mmap_tile_for_position(start),
            mmap_tile_for_position(target)
        )
    });

    assert_eq!(corner.map_id, 0);
    assert!(corner.x.is_finite());
    assert!(corner.y.is_finite());
    assert!(corner.z.is_finite());
}

#[test]
fn db_creature_path_trim_keeps_intermediate_corners() {
    let start = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    let path = vec![
        WorldPosition::new(0, 4.0, 0.0, 0.0, 0.0),
        WorldPosition::new(0, 4.0, 4.0, 0.0, 0.0),
        WorldPosition::new(0, 10.0, 4.0, 0.0, 0.0),
    ];

    let trimmed =
        db_creature_trim_path_to_travel_distance(start, path, 3.0).expect("path should trim");

    assert_eq!(trimmed.len(), 3);
    assert_eq!(trimmed[0].x, 4.0);
    assert_eq!(trimmed[1].y, 4.0);
    assert!(trimmed[2].x > 6.0);
    assert!(trimmed[2].x < 8.0);
}

#[test]
fn db_creature_path_motion_interpolates_across_corners() {
    let start = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    let path = vec![
        WorldPosition::new(0, 4.0, 0.0, 0.0, 0.0),
        WorldPosition::new(0, 4.0, 4.0, 0.0, 0.0),
    ];
    let now = Instant::now();

    let position = advance_timed_path_motion(
        start,
        &path,
        now,
        Duration::from_secs(8),
        now + Duration::from_secs(6),
    )
    .expect("motion should still be active");

    assert_eq!(position.x, 4.0);
    assert!(position.y > 1.9);
    assert!(position.y < 2.1);
}

#[test]
fn db_creature_runtime_position_is_separate_from_home_spawn() {
    let mut spawn = test_creature_spawn(6);
    spawn.position_x = -8950.0;
    spawn.position_y = -130.0;
    let home_x = spawn.position_x;
    let home_y = spawn.position_y;
    let mut runtime = DbCreatureRuntime::new(spawn);

    runtime.current_position.x += 8.0;
    runtime.current_position.y += 2.0;

    assert_eq!(runtime.spawn.position_x, home_x);
    assert_eq!(runtime.spawn.position_y, home_y);
    assert_eq!(runtime.home_position.x, home_x);
    assert_eq!(runtime.home_position.y, home_y);

    runtime.respawn();
    assert_eq!(runtime.current_position.x, runtime.home_position.x);
    assert_eq!(runtime.current_position.y, runtime.home_position.y);
    assert!(matches!(runtime.motion, CreatureMotionState::Idle));
}

#[test]
fn db_creature_evades_after_leash_radius_and_prepares_return_home() {
    let mut spawn = test_creature_spawn(6);
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    spawn.template.min_level = 1;
    let attacker = creature_spawn_guid(&spawn);
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.current_position.x = DB_CREATURE_LEASH_RADIUS_YARDS + 1.0;
    runtime.health = runtime.max_health() - 1;
    let mut session = WorldSessionState {
        active_character: Some(ActiveCharacter {
            guid: 7,
            name: "Ada".to_string(),
            race: 1,
            class: 1,
            level: 1,
            xp: 0,
            position: WorldPosition::new(0, DB_CREATURE_LEASH_RADIUS_YARDS + 5.0, 0.0, 0.0, 0.0),
            movement_flags: 0,
            client_time: 0,
            fall_time: 0,
        }),
        active_combat_target: Some(attacker),
        active_combat_next_swing_at: Some(now),
        ..WorldSessionState::default()
    };
    session.active_creature_combats.insert(
        attacker.raw(),
        CreatureCombatState {
            attacker,
            victim: ObjectGuid::new(HighGuid::Player, 0, 7),
            next_swing_at: now,
        },
    );
    session.db_creatures.insert(attacker.raw(), runtime);

    assert!(db_creature_should_evade(&session, attacker));
    prepare_db_creature_evade(&mut session, attacker);
    let motion = start_db_creature_return_home_motion(&mut session, attacker, now)
        .expect("leashed creature should run home");

    let destination = *motion.path.last().unwrap();
    assert_eq!(destination.x, 0.0);
    assert_eq!(destination.y, 0.0);
    assert_eq!(motion.spline_id, 0);
    assert!(motion.duration > Duration::ZERO);
    assert!(session.active_combat_target.is_none());
    assert!(session.active_combat_next_swing_at.is_none());
    assert!(session.active_creature_combats.is_empty());
    let runtime = session.db_creatures.get(&attacker.raw()).unwrap();
    assert_eq!(runtime.health, runtime.max_health());
    assert!(matches!(runtime.motion, CreatureMotionState::ReturnHome(_)));
    assert!(!db_creature_should_evade(&session, attacker));
}

#[test]
fn db_creature_return_home_motion_finishes_at_home() {
    let mut spawn = test_creature_spawn(6);
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let attacker = creature_spawn_guid(&spawn);
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.current_position.x = 14.0;
    let mut session = WorldSessionState::default();
    session.db_creatures.insert(attacker.raw(), runtime);

    let motion = start_db_creature_return_home_motion(&mut session, attacker, now)
        .expect("away creature should start return-home motion");
    let half_duration = Duration::from_millis((motion.duration.as_millis() as u64 / 2).max(1));
    advance_db_creature_motion(&mut session, attacker, now + half_duration);
    let mid_x = session
        .db_creatures
        .get(&attacker.raw())
        .expect("creature should still be loaded")
        .current_position
        .x;
    assert!(mid_x < motion.start.x);
    let destination = *motion.path.last().unwrap();
    assert!(mid_x > destination.x);

    advance_db_creature_motion(&mut session, attacker, now + motion.duration);
    let runtime = session.db_creatures.get(&attacker.raw()).unwrap();
    assert_eq!(runtime.current_position.x, runtime.home_position.x);
    assert_eq!(runtime.current_position.y, runtime.home_position.y);
    assert!(matches!(runtime.motion, CreatureMotionState::Idle));
}

#[test]
fn db_creature_return_home_motion_advances_without_active_combat() {
    let mut spawn = test_creature_spawn(6);
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let attacker = creature_spawn_guid(&spawn);
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.current_position.x = 7.0;
    let mut session = WorldSessionState::default();
    session.db_creatures.insert(attacker.raw(), runtime);

    let motion = start_db_creature_return_home_motion(&mut session, attacker, now)
        .expect("away creature should start return-home motion");
    assert!(session.active_creature_combats.is_empty());

    advance_db_creature_return_home_motions(&mut session, now + motion.duration);
    let runtime = session.db_creatures.get(&attacker.raw()).unwrap();
    assert_eq!(runtime.current_position.x, runtime.home_position.x);
    assert!(matches!(runtime.motion, CreatureMotionState::Idle));
}

#[test]
fn db_creature_return_home_motion_ignores_combat_path_guardrail() {
    let mut spawn = test_creature_spawn(6);
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let attacker = creature_spawn_guid(&spawn);
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.current_position.x = 9.0;
    let mut session = WorldSessionState {
        db_creature_navigation: DbCreatureNavigationGuardrail {
            line_of_sight_clear: false,
            path_available: false,
            ..DbCreatureNavigationGuardrail::default()
        },
        ..WorldSessionState::default()
    };
    session.db_creatures.insert(attacker.raw(), runtime);

    assert!(start_db_creature_return_home_motion(&mut session, attacker, Instant::now()).is_some());
    assert!(matches!(
        session.db_creatures.get(&attacker.raw()).unwrap().motion,
        CreatureMotionState::ReturnHome(_)
    ));
}

#[test]
fn db_creature_returning_home_does_not_reaggro_or_take_damage() {
    let mut spawn = test_creature_spawn(6);
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    spawn.template.npc_flags = 0;
    spawn.template.min_level = 1;
    let attacker = creature_spawn_guid(&spawn);
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.current_position.x = 6.0;
    runtime.motion = CreatureMotionState::ReturnHome(CreatureReturnHomeMotion {
        start: runtime.current_position,
        destination: runtime.home_position,
        path: vec![runtime.home_position],
        started_at: now,
        duration: Duration::from_secs(1),
    });
    let health = runtime.health;
    let mut session = WorldSessionState {
        active_character: Some(ActiveCharacter {
            guid: 7,
            name: "Ada".to_string(),
            race: 1,
            class: 1,
            level: 1,
            xp: 0,
            position: WorldPosition::new(0, 5.5, 0.0, 0.0, 0.0),
            movement_flags: 0,
            client_time: 0,
            fall_time: 0,
        }),
        ..WorldSessionState::default()
    };
    session.db_creatures.insert(attacker.raw(), runtime);

    assert_eq!(select_db_creature_aggro_target(&session), None);
    assert_eq!(apply_db_creature_damage(&mut session, attacker, 1), None);
    assert_eq!(
        session.db_creatures.get(&attacker.raw()).unwrap().health,
        health
    );
}

#[test]
fn db_creature_random_motion_uses_spawn_movement_type_and_spawn_dist() {
    let mut spawn = test_creature_spawn(6);
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    spawn.movement_type = DB_MOTION_TYPE_RANDOM;
    spawn.spawn_dist = 5.0;
    let creature_guid = creature_spawn_guid(&spawn);
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.next_random_move_at = Some(now);
    let mut session = WorldSessionState::default();
    session.db_creatures.insert(creature_guid.raw(), runtime);

    let motion = start_db_creature_random_motion(&mut session, creature_guid, now)
        .expect("random movement creature should start a wander spline");

    assert_eq!(motion.spline_id, 0);
    assert_eq!(motion.start.x, 0.0);
    assert!(motion.duration >= Duration::from_millis(1));
    assert!(!motion.path.is_empty());
    assert!(
        distance_2d(
            motion.path.last().unwrap().x,
            motion.path.last().unwrap().y,
            0.0,
            0.0
        ) <= 5.0,
        "wander destination should stay inside spawndist"
    );
    let runtime = session.db_creatures.get(&creature_guid.raw()).unwrap();
    assert!(matches!(runtime.motion, CreatureMotionState::Random(_)));
    assert_eq!(runtime.next_spline_id, 1);

    advance_db_creature_motion(&mut session, creature_guid, now + motion.duration);
    let runtime = session.db_creatures.get(&creature_guid.raw()).unwrap();
    assert!(matches!(runtime.motion, CreatureMotionState::Idle));
    assert!(runtime.next_random_move_at.is_some());
}

#[test]
fn db_creature_random_motion_ignores_idle_or_zero_spawndist_creatures() {
    let mut idle_spawn = test_creature_spawn(6);
    idle_spawn.movement_type = DB_MOTION_TYPE_IDLE;
    idle_spawn.spawn_dist = 5.0;
    let idle_guid = creature_spawn_guid(&idle_spawn);
    let mut zero_radius_spawn = test_creature_spawn(6);
    zero_radius_spawn.guid = 45;
    zero_radius_spawn.movement_type = DB_MOTION_TYPE_RANDOM;
    zero_radius_spawn.spawn_dist = 0.0;
    let zero_radius_guid = creature_spawn_guid(&zero_radius_spawn);
    let now = Instant::now();
    let mut session = WorldSessionState::default();
    session
        .db_creatures
        .insert(idle_guid.raw(), DbCreatureRuntime::new(idle_spawn));
    session.db_creatures.insert(
        zero_radius_guid.raw(),
        DbCreatureRuntime::new(zero_radius_spawn),
    );

    assert!(start_db_creature_random_motion(&mut session, idle_guid, now).is_none());
    assert!(start_db_creature_random_motion(&mut session, zero_radius_guid, now).is_none());
}

#[test]
fn db_creature_waypoint_motion_uses_db_path_and_wait_time() {
    let mut spawn = test_creature_spawn(6);
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 83.5;
    spawn.movement_type = DB_MOTION_TYPE_WAYPOINT;
    spawn.waypoint_path = vec![test_waypoint(1, 5.0, 0.0, 250)];
    let creature_guid = creature_spawn_guid(&spawn);
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.next_waypoint_move_at = Some(now);
    let mut session = WorldSessionState::default();
    session.db_creatures.insert(creature_guid.raw(), runtime);

    let motion = start_db_creature_waypoint_motion(&mut session, creature_guid, now)
        .expect("waypoint creature should start a DB path spline");

    assert_eq!(motion.spline_id, 0);
    assert_eq!(motion.start.x, 0.0);
    assert_eq!(motion.path.last().unwrap().x, 5.0);
    let runtime = session.db_creatures.get(&creature_guid.raw()).unwrap();
    assert!(matches!(runtime.motion, CreatureMotionState::Waypoint(_)));
    assert_eq!(runtime.next_spline_id, 1);
    assert_eq!(runtime.next_waypoint_move_at, None);

    advance_db_creature_motion(&mut session, creature_guid, now + motion.duration);
    let runtime = session.db_creatures.get(&creature_guid.raw()).unwrap();
    assert!(matches!(runtime.motion, CreatureMotionState::Idle));
    assert_eq!(runtime.current_position.x, 5.0);
    assert_eq!(runtime.waypoint_next_index, 0);
    assert!(runtime
        .next_waypoint_move_at
        .is_some_and(|at| at == now + motion.duration + Duration::from_millis(250)));
}

#[test]
fn db_creature_linear_waypoint_motion_reverses_at_ends() {
    let mut spawn = test_creature_spawn(6);
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 83.5;
    spawn.movement_type = DB_MOTION_TYPE_LINEAR_WAYPOINT;
    spawn.waypoint_path = vec![
        test_waypoint(1, 5.0, 0.0, 0),
        test_waypoint(2, 10.0, 0.0, 0),
        test_waypoint(3, 15.0, 0.0, 0),
    ];
    let creature_guid = creature_spawn_guid(&spawn);
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.next_waypoint_move_at = Some(now);
    let mut session = WorldSessionState::default();
    session.db_creatures.insert(creature_guid.raw(), runtime);

    let first = start_db_creature_waypoint_motion(&mut session, creature_guid, now).unwrap();
    advance_db_creature_motion(&mut session, creature_guid, now + first.duration);
    assert_eq!(
        session
            .db_creatures
            .get(&creature_guid.raw())
            .unwrap()
            .waypoint_next_index,
        1
    );

    let second_at = now + first.duration;
    let second = start_db_creature_waypoint_motion(&mut session, creature_guid, second_at).unwrap();
    advance_db_creature_motion(&mut session, creature_guid, second_at + second.duration);
    assert_eq!(
        session
            .db_creatures
            .get(&creature_guid.raw())
            .unwrap()
            .waypoint_next_index,
        2
    );

    let third_at = second_at + second.duration;
    let third = start_db_creature_waypoint_motion(&mut session, creature_guid, third_at).unwrap();
    advance_db_creature_motion(&mut session, creature_guid, third_at + third.duration);
    let runtime = session.db_creatures.get(&creature_guid.raw()).unwrap();
    assert_eq!(runtime.current_position.x, 15.0);
    assert_eq!(runtime.waypoint_next_index, 1);
    assert!(!runtime.waypoint_forward);
}

#[test]
fn db_creature_assistance_calls_nearby_same_faction_hostiles_once() {
    let character = ActiveCharacter {
        guid: 7,
        name: "Ada".to_string(),
        race: 1,
        class: 1,
        level: 1,
        xp: 0,
        position: WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
    };
    let mut caller_spawn = test_creature_spawn(6);
    caller_spawn.position_x = 0.0;
    caller_spawn.position_y = 0.0;
    caller_spawn.template.npc_flags = 0;
    caller_spawn.template.faction = 25;
    caller_spawn.template.call_for_help = 6;
    let caller = creature_spawn_guid(&caller_spawn);
    let mut helper_spawn = test_creature_spawn(6);
    helper_spawn.guid = 45;
    helper_spawn.position_x = 5.0;
    helper_spawn.position_y = 0.0;
    helper_spawn.template.npc_flags = 0;
    helper_spawn.template.faction = 25;
    let helper = creature_spawn_guid(&helper_spawn);
    let mut far_spawn = test_creature_spawn(6);
    far_spawn.guid = 46;
    far_spawn.position_x = 9.0;
    far_spawn.position_y = 0.0;
    far_spawn.template.npc_flags = 0;
    far_spawn.template.faction = 25;
    let mut session = WorldSessionState {
        active_character: Some(character),
        ..WorldSessionState::default()
    };
    session
        .db_creatures
        .insert(caller.raw(), DbCreatureRuntime::new(caller_spawn));
    session
        .db_creatures
        .insert(helper.raw(), DbCreatureRuntime::new(helper_spawn));
    let far_runtime = DbCreatureRuntime::new(far_spawn);
    session
        .db_creatures
        .insert(far_runtime.guid().raw(), far_runtime);

    assert_eq!(
        select_db_creature_assist_targets(&mut session, caller),
        vec![helper]
    );
    assert!(select_db_creature_assist_targets(&mut session, caller).is_empty());
}

#[test]
fn db_creature_chase_motion_advances_position_over_time_before_reach() {
    let mut creature = test_creature_spawn(6);
    creature.position_x = 0.0;
    creature.position_y = 0.0;
    creature.position_z = 0.0;
    let attacker = creature_spawn_guid(&creature);
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let now = Instant::now();
    let mut session = WorldSessionState {
        active_character: Some(ActiveCharacter {
            guid: 7,
            name: "Ada".to_string(),
            race: 1,
            class: 1,
            level: 1,
            xp: 0,
            position: WorldPosition::new(0, 10.0, 0.0, 0.0, 0.0),
            movement_flags: 0,
            client_time: 0,
            fall_time: 0,
        }),
        ..WorldSessionState::default()
    };
    session
        .db_creatures
        .insert(attacker.raw(), DbCreatureRuntime::new(creature));

    let motion = start_db_creature_chase_motion(&mut session, attacker, player, now)
        .expect("out-of-range creature should start chase motion");
    assert_eq!(motion.spline_id, 0);
    assert_eq!(motion.start.x, 0.0);
    assert!(!motion.path.is_empty());
    assert_eq!(
        motion.path.last().unwrap().x,
        10.0 - DB_CREATURE_MELEE_REACH_YARDS * DB_CREATURE_CHASE_DEFAULT_RANGE_FACTOR
    );
    let runtime = session
        .db_creatures
        .get(&attacker.raw())
        .expect("creature should still be loaded");
    let CreatureMotionState::Chase(chase) = &runtime.motion else {
        panic!("creature should be in chase motion");
    };
    assert_eq!(chase.target, player);
    assert_eq!(runtime.next_spline_id, 1);
    assert_eq!(
        chase.recheck_at,
        now + Duration::from_millis(DB_CREATURE_CHASE_RECHECK_MILLIS)
    );
    assert!(!db_creature_can_reach_player(&session, attacker));

    let half_duration = Duration::from_millis((motion.duration.as_millis() as u64 / 2).max(1));
    advance_db_creature_motion(&mut session, attacker, now + half_duration);
    let mid_x = session
        .db_creatures
        .get(&attacker.raw())
        .expect("creature should still be loaded")
        .current_position
        .x;
    assert!(mid_x > motion.start.x);
    let destination = *motion.path.last().unwrap();
    assert!(mid_x < destination.x);
    assert!(!db_creature_can_reach_player(&session, attacker));

    advance_db_creature_motion(&mut session, attacker, now + motion.duration);
    let runtime = session
        .db_creatures
        .get(&attacker.raw())
        .expect("creature should still be loaded");
    assert_eq!(runtime.current_position.x, destination.x);
    assert_eq!(runtime.spawn.position_x, 0.0);
    assert!(matches!(runtime.motion, CreatureMotionState::Idle));
    assert!(db_creature_can_reach_player(&session, attacker));
}

#[test]
fn db_creature_chase_motion_waits_for_recheck_before_repathing() {
    let mut creature = test_creature_spawn(6);
    creature.position_x = 0.0;
    creature.position_y = 0.0;
    creature.position_z = 0.0;
    let attacker = creature_spawn_guid(&creature);
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let now = Instant::now();
    let mut session = WorldSessionState {
        active_character: Some(ActiveCharacter {
            guid: 7,
            name: "Ada".to_string(),
            race: 1,
            class: 1,
            level: 1,
            xp: 0,
            position: WorldPosition::new(0, 10.0, 0.0, 0.0, 0.0),
            movement_flags: 0,
            client_time: 0,
            fall_time: 0,
        }),
        ..WorldSessionState::default()
    };
    session
        .db_creatures
        .insert(attacker.raw(), DbCreatureRuntime::new(creature));

    let first_motion = start_db_creature_chase_motion(&mut session, attacker, player, now)
        .expect("out-of-range creature should start chase motion");
    session.active_character.as_mut().unwrap().position.x = 20.0;

    let before_recheck = now + Duration::from_millis(DB_CREATURE_CHASE_RECHECK_MILLIS - 1);
    assert!(
        start_db_creature_chase_motion(&mut session, attacker, player, before_recheck).is_none()
    );

    let runtime = session
        .db_creatures
        .get(&attacker.raw())
        .expect("creature should still be loaded");
    assert_eq!(runtime.next_spline_id, 1);
    let CreatureMotionState::Chase(chase) = &runtime.motion else {
        panic!("creature should remain in chase motion");
    };
    assert_eq!(chase.destination.x, first_motion.path.last().unwrap().x);
    assert_eq!(runtime.next_spline_id, 1);
}

#[test]
fn db_creature_chase_motion_repaths_to_moved_player_after_recheck() {
    let mut creature = test_creature_spawn(6);
    creature.position_x = 0.0;
    creature.position_y = 0.0;
    creature.position_z = 0.0;
    let attacker = creature_spawn_guid(&creature);
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let now = Instant::now();
    let mut session = WorldSessionState {
        active_character: Some(ActiveCharacter {
            guid: 7,
            name: "Ada".to_string(),
            race: 1,
            class: 1,
            level: 1,
            xp: 0,
            position: WorldPosition::new(0, 10.0, 0.0, 0.0, 0.0),
            movement_flags: 0,
            client_time: 0,
            fall_time: 0,
        }),
        ..WorldSessionState::default()
    };
    session
        .db_creatures
        .insert(attacker.raw(), DbCreatureRuntime::new(creature));

    let first_motion = start_db_creature_chase_motion(&mut session, attacker, player, now)
        .expect("out-of-range creature should start chase motion");
    let recheck_at = now + Duration::from_millis(DB_CREATURE_CHASE_RECHECK_MILLIS);
    advance_db_creature_motion(&mut session, attacker, recheck_at);
    let moved_start = session
        .db_creatures
        .get(&attacker.raw())
        .expect("creature should still be loaded")
        .current_position;
    assert!(moved_start.x > 0.0);

    session.active_character.as_mut().unwrap().position.x = 20.0;
    let second_motion = start_db_creature_chase_motion(&mut session, attacker, player, recheck_at)
        .expect("moved player should trigger a refreshed chase spline");

    assert_eq!(second_motion.spline_id, 1);
    assert_eq!(second_motion.start.x, moved_start.x);
    assert!(
        second_motion.path.last().unwrap().x
            > first_motion.path.last().unwrap().x + DB_CREATURE_CHASE_REPATH_YARDS
    );
    let runtime = session
        .db_creatures
        .get(&attacker.raw())
        .expect("creature should still be loaded");
    let CreatureMotionState::Chase(chase) = &runtime.motion else {
        panic!("creature should remain in chase motion");
    };
    assert_eq!(runtime.next_spline_id, 2);
    assert_eq!(
        chase.recheck_at,
        recheck_at + Duration::from_millis(DB_CREATURE_CHASE_RECHECK_MILLIS)
    );
}

#[test]
fn db_creature_chase_motion_ignores_tiny_destination_shift_after_recheck() {
    let mut creature = test_creature_spawn(6);
    creature.position_x = 0.0;
    creature.position_y = 0.0;
    creature.position_z = 0.0;
    let attacker = creature_spawn_guid(&creature);
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let now = Instant::now();
    let mut session = WorldSessionState {
        active_character: Some(ActiveCharacter {
            guid: 7,
            name: "Ada".to_string(),
            race: 1,
            class: 1,
            level: 1,
            xp: 0,
            position: WorldPosition::new(0, 10.0, 0.0, 0.0, 0.0),
            movement_flags: 0,
            client_time: 0,
            fall_time: 0,
        }),
        ..WorldSessionState::default()
    };
    session
        .db_creatures
        .insert(attacker.raw(), DbCreatureRuntime::new(creature));

    let first_motion = start_db_creature_chase_motion(&mut session, attacker, player, now)
        .expect("out-of-range creature should start chase motion");
    session.active_character.as_mut().unwrap().position.x =
        10.0 + DB_CREATURE_CHASE_REPATH_YARDS * 0.5;
    let recheck_at = now + Duration::from_millis(DB_CREATURE_CHASE_RECHECK_MILLIS);

    assert!(start_db_creature_chase_motion(&mut session, attacker, player, recheck_at).is_none());
    let runtime = session
        .db_creatures
        .get(&attacker.raw())
        .expect("creature should still be loaded");
    assert_eq!(runtime.next_spline_id, 1);
    let CreatureMotionState::Chase(chase) = &runtime.motion else {
        panic!("creature should remain in chase motion");
    };
    assert_eq!(chase.destination.x, first_motion.path.last().unwrap().x);
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
            requires_melee: true,
        })
    );
    assert_eq!(
        supported_starter_spell(HUNTER_RAPTOR_STRIKE_RANK_1),
        Some(SupportedStarterSpell {
            damage: RAPTOR_STRIKE_FIXTURE_DAMAGE,
            power: StarterSpellPower::Mana {
                cost: RAPTOR_STRIKE_MANA_COST
            },
            requires_melee: true,
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
