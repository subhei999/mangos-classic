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

fn decode_positioned_create_update_block(
    block: &[u8],
    guid: ObjectGuid,
    type_id: u8,
) -> (Vec<Option<u32>>, &[u8]) {
    assert_eq!(block[0], UPDATE_TYPE_CREATE_OBJECT2);
    let type_id_offset = 1 + PackedGuid::packed_size(guid);
    assert_eq!(block[type_id_offset], type_id);
    assert_eq!(
        block[type_id_offset + 1],
        UPDATEFLAG_ALL | UPDATEFLAG_HAS_POSITION
    );
    assert_eq!(
        &block[type_id_offset + 18..type_id_offset + 22],
        &1u32.to_le_bytes()
    );

    let values_start = type_id_offset + 22;
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
        model_bounding_radius: DEFAULT_WORLD_OBJECT_SIZE,
        model_combat_reach: PLAYER_COMBAT_REACH_YARDS,
        faction: 35,
        scale: 1.0,
        speed_walk: 1.0,
        speed_run: 1.0,
        detection_range: 20,
        call_for_help: 0,
        pursuit: 15_000,
        leash: 0,
        family: 0,
        creature_type: 7,
        npc_flags: UNIT_NPC_FLAG_GOSSIP,
        unit_flags: 0x20,
        dynamic_flags: 0,
        unit_class: 1,
        rank: 1,
        health_multiplier: 1.0,
        power_multiplier: 1.0,
        damage_multiplier: 1.0,
        damage_variance: 1.0,
        armor_multiplier: 1.0,
        min_level_health: 80,
        max_level_health: 120,
        min_level_mana: 0,
        max_level_mana: 0,
        min_melee_dmg: 3.0,
        max_melee_dmg: 5.0,
        min_ranged_dmg: 0.0,
        max_ranged_dmg: 0.0,
        armor: 0,
        melee_attack_power: 0,
        ranged_attack_power: 0,
        min_loot_gold: 2,
        max_loot_gold: 4,
        melee_base_attack_time: 1800,
        ranged_base_attack_time: 2200,
        damage_school: 0,
        trainer_type: 0,
        trainer_class: 0,
        pet_spell_data_id: 0,
        civilian: 0,
        corpse_decay: 0,
        movement_type: DB_MOTION_TYPE_IDLE,
        equipment_template_id: 0,
        equip_display_id1: 0,
        equip_display_id2: 0,
        equip_display_id3: 0,
        equip_class1: 0,
        equip_class2: 0,
        equip_class3: 0,
        equip_subclass1: 0,
        equip_subclass2: 0,
        equip_subclass3: 0,
        equip_material1: 0,
        equip_material2: 0,
        equip_material3: 0,
        equip_inventory_type1: 0,
        equip_inventory_type2: 0,
        equip_inventory_type3: 0,
        equip_sheath1: 0,
        equip_sheath2: 0,
        equip_sheath3: 0,
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
        spawn_time_secs_min: 120,
        spawn_time_secs_max: 120,
        spawn_dist: 0.0,
        movement_type: DB_MOTION_TYPE_IDLE,
        formation_waypoint_path_id: None,
        template: test_creature_template(entry),
        waypoint_path: Vec::new(),
    }
}

fn test_gameobject_template(entry: u32, object_type: u8) -> wow_db::GameObjectTemplateQuery {
    wow_db::GameObjectTemplateQuery {
        entry,
        object_type,
        display_id: 12_345,
        name: format!("GO {entry}"),
        icon_name: "Attack".to_string(),
        faction: 0,
        flags: 0,
        size: 1.0,
        raw_data: [0; 24],
    }
}

fn test_gameobject_spawn(entry: u32, object_type: u8) -> wow_db::GameObjectSpawnQuery {
    wow_db::GameObjectSpawnQuery {
        guid: 77,
        entry,
        map: 0,
        position_x: -8948.0,
        position_y: -131.0,
        position_z: 83.4,
        orientation: 0.75,
        rotation0: 0.0,
        rotation1: 0.0,
        rotation2: 0.0,
        rotation3: 1.0,
        spawn_time_secs_min: 45,
        spawn_time_secs_max: 45,
        state: -1,
        anim_progress: 100,
        template: test_gameobject_template(entry, object_type),
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

fn test_quest_template(entry: u32) -> QuestTemplateQuery {
    QuestTemplateQuery {
        entry,
        method: 2,
        zone_or_sort: 12,
        min_level: 1,
        max_level: 255,
        quest_level: 1,
        quest_type: 0,
        required_classes: 0,
        required_races: 0,
        rep_objective_faction: 0,
        rep_objective_value: 0,
        special_flags: 0,
        prev_quest_id: 0,
        next_quest_id: 0,
        exclusive_group: 0,
        next_quest_in_chain: 0,
        rew_or_req_money: 0,
        rew_money_max_level: 0,
        rew_spell: 0,
        rew_spell_cast: 0,
        src_item_id: 0,
        src_item_count: 0,
        quest_flags: 0,
        title: "Test Quest".to_string(),
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
        objective_text: [String::new(), String::new(), String::new(), String::new()],
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
        &[(0, GOSSIP_ICON_VENDOR, DB_VENDOR_GOSSIP_OPTION)],
    );
    assert_eq!(&body[0..8], &guid.raw().to_le_bytes());
    assert_eq!(&body[8..12], &DB_VENDOR_GOSSIP_TEXT_ID.to_le_bytes());
    assert_eq!(&body[12..16], &1u32.to_le_bytes());
    assert_eq!(&body[16..20], &0u32.to_le_bytes());
    assert_eq!(body[20], GOSSIP_ICON_VENDOR);
    assert_eq!(body[21], 0);
    assert_eq!(&body[22..36], b"Browse goods.\0");
    assert_eq!(&body[36..40], &0u32.to_le_bytes());
}

#[test]
fn db_trainer_gossip_message_uses_trainer_book_icon() {
    let guid = ObjectGuid::new(HighGuid::Unit, 42, 96_002);
    let body = build_gossip_message(
        guid,
        DB_TRAINER_GOSSIP_TEXT_ID,
        &[(0, GOSSIP_ICON_TRAINER, DB_TRAINER_GOSSIP_OPTION)],
    );
    assert_eq!(&body[0..8], &guid.raw().to_le_bytes());
    assert_eq!(&body[8..12], &DB_TRAINER_GOSSIP_TEXT_ID.to_le_bytes());
    assert_eq!(&body[12..16], &1u32.to_le_bytes());
    assert_eq!(&body[16..20], &0u32.to_le_bytes());
    assert_eq!(body[20], GOSSIP_ICON_TRAINER);
    assert_eq!(body[21], 0);
    assert_eq!(&body[22..39], b"I seek training.\0");
    assert_eq!(&body[39..43], &0u32.to_le_bytes());
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
fn creature_display_info_dbc_parser_reads_display_scale_field() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"WDBC");
    bytes.extend_from_slice(&2u32.to_le_bytes());
    bytes.extend_from_slice(&12u32.to_le_bytes());
    bytes.extend_from_slice(&48u32.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    let mut first = [0u8; 48];
    first[0..4].copy_from_slice(&604u32.to_le_bytes());
    first[16..20].copy_from_slice(&0.7f32.to_le_bytes());
    bytes.extend_from_slice(&first);
    let mut second = [0u8; 48];
    second[0..4].copy_from_slice(&447u32.to_le_bytes());
    second[16..20].copy_from_slice(&0.45f32.to_le_bytes());
    bytes.extend_from_slice(&second);
    bytes.push(0);

    let scales = parse_creature_display_info_scales(&bytes);

    assert_eq!(scales.get(&604).copied(), Some(0.7));
    assert_eq!(scales.get(&447).copied(), Some(0.45));
}

#[test]
fn creature_template_zero_scale_falls_back_to_first_display_dbc_scale() {
    let mut spawns = vec![test_creature_spawn(299)];
    spawns[0].template.scale = 0.0;
    spawns[0].template.display_id1 = 0;
    spawns[0].template.display_id2 = 447;
    let display_scales = HashMap::from([(447, 0.45), (604, 0.7)]);

    apply_creature_display_scale_fallbacks(&mut spawns, &display_scales);

    assert_eq!(spawns[0].template.scale, 0.45);

    spawns[0].template.scale = 1.25;
    apply_creature_display_scale_fallbacks(&mut spawns, &display_scales);
    assert_eq!(spawns[0].template.scale, 1.25);
}

#[test]
fn db_creature_create_block_uses_template_speed_rates_and_equipment_displays() {
    let mut creature = test_creature_spawn(198);
    creature.template.speed_walk = 0.9;
    creature.template.speed_run = 1.14286;
    creature.template.equip_display_id1 = 1001;
    creature.template.equip_display_id2 = 1002;
    creature.template.equip_display_id3 = 1003;
    creature.template.equip_class1 = 2;
    creature.template.equip_subclass1 = 7;
    creature.template.equip_material1 = 1;
    creature.template.equip_inventory_type1 = 21;
    creature.template.equip_sheath1 = 3;
    creature.template.equip_class2 = 4;
    creature.template.equip_subclass2 = 6;
    creature.template.equip_material2 = -1;
    creature.template.equip_inventory_type2 = 14;
    creature.template.equip_sheath2 = 1;

    let block = build_db_creature_create_block(&creature).unwrap();
    let packed_guid_mask = block[1];
    let update_flags_offset = 1 + 1 + packed_guid_mask.count_ones() as usize + 1;
    let movement_start = update_flags_offset + 1;
    let walk_offset = movement_start + 28;
    let run_offset = movement_start + 32;
    let walk_speed = f32::from_le_bytes(block[walk_offset..walk_offset + 4].try_into().unwrap());
    let run_speed = f32::from_le_bytes(block[run_offset..run_offset + 4].try_into().unwrap());

    let values_start = update_flags_offset + 1 + 56;
    let values = decode_update_values(&block[values_start..]);
    assert_eq!(walk_speed, DB_CREATURE_WALK_SPEED_YARDS_PER_SEC * 0.9);
    assert!((run_speed - (DB_CREATURE_RUN_SPEED_YARDS_PER_SEC * 1.14286)).abs() < 0.0001);
    assert_eq!(values[UNIT_VIRTUAL_ITEM_SLOT_DISPLAY], Some(1001));
    assert_eq!(values[UNIT_VIRTUAL_ITEM_SLOT_DISPLAY + 1], Some(1002));
    assert_eq!(values[UNIT_VIRTUAL_ITEM_SLOT_DISPLAY + 2], Some(1003));
    assert_eq!(values[UNIT_VIRTUAL_ITEM_INFO], Some(0x1501_0702));
    assert_eq!(values[UNIT_VIRTUAL_ITEM_INFO + 1], Some(3));
    assert_eq!(values[UNIT_VIRTUAL_ITEM_INFO + 2], Some(0x0EFF_0604));
    assert_eq!(values[UNIT_VIRTUAL_ITEM_INFO + 3], Some(1));
    assert_eq!(values[UNIT_VIRTUAL_ITEM_INFO + 4], Some(0));
    assert_eq!(values[UNIT_VIRTUAL_ITEM_INFO + 5], Some(0));
    assert_eq!(values[UNIT_FIELD_BYTES_2], Some(creature_unit_bytes_2()));
}

#[test]
fn db_creature_runtime_create_block_preserves_corpse_state() {
    let mut runtime = DbCreatureRuntime::new(test_creature_spawn(6));
    runtime.begin_corpse(Instant::now(), 1_000);

    let block = build_db_creature_runtime_create_block(&runtime).unwrap();
    let packed_guid_mask = block[1];
    let update_flags_offset = 1 + 1 + packed_guid_mask.count_ones() as usize + 1;
    let values_start = update_flags_offset + 1 + 56;
    let values = decode_update_values(&block[values_start..]);

    assert_eq!(values[UNIT_FIELD_HEALTH], Some(0));
    assert_eq!(values[UNIT_DYNAMIC_FLAGS], Some(UNIT_DYNFLAG_LOOTABLE));
    assert_eq!(values[UNIT_NPC_FLAGS], Some(0));
}

#[test]
fn db_gameobject_create_block_uses_spawn_and_template_fields() {
    let spawn = test_gameobject_spawn(161557, GO_TYPE_GOOBER);
    let runtime = DbGameObjectRuntime::new(spawn.clone());
    let block = build_db_gameobject_runtime_create_block(&runtime).unwrap();
    let type_id_offset = 1 + PackedGuid::packed_size(runtime.guid());
    assert_eq!(
        &block[type_id_offset + 2..type_id_offset + 6],
        &spawn.position_x.to_le_bytes()
    );
    assert_eq!(
        &block[type_id_offset + 6..type_id_offset + 10],
        &spawn.position_y.to_le_bytes()
    );
    assert_eq!(
        &block[type_id_offset + 10..type_id_offset + 14],
        &spawn.position_z.to_le_bytes()
    );
    assert_eq!(
        &block[type_id_offset + 14..type_id_offset + 18],
        &spawn.orientation.to_le_bytes()
    );
    let (values, trailing) =
        decode_positioned_create_update_block(&block, runtime.guid(), TYPEID_GAMEOBJECT);

    assert!(trailing.is_empty());
    assert_eq!(values[0], Some(runtime.guid().raw() as u32));
    assert_eq!(values[1], Some((runtime.guid().raw() >> 32) as u32));
    assert_eq!(values[2], Some(TYPEMASK_OBJECT_GAMEOBJECT));
    assert_eq!(values[3], Some(spawn.entry));
    assert_eq!(
        values[GAMEOBJECT_DISPLAYID],
        Some(spawn.template.display_id)
    );
    assert_eq!(values[GAMEOBJECT_FLAGS], Some(spawn.template.flags));
    assert_eq!(values[GAMEOBJECT_POS_X], Some(spawn.position_x.to_bits()));
    assert_eq!(values[GAMEOBJECT_POS_Y], Some(spawn.position_y.to_bits()));
    assert_eq!(values[GAMEOBJECT_POS_Z], Some(spawn.position_z.to_bits()));
    assert_eq!(values[GAMEOBJECT_FACING], Some(spawn.orientation.to_bits()));
    assert_eq!(
        values[GAMEOBJECT_TYPE_ID],
        Some(spawn.template.object_type as u32)
    );
    assert_eq!(
        values[GAMEOBJECT_ANIMPROGRESS],
        Some(spawn.anim_progress as u32)
    );
}

#[test]
fn db_gameobject_create_block_sets_quest_chest_dynamic_flags_for_condition_chests() {
    let mut spawn = test_gameobject_spawn(161557, GO_TYPE_CHEST);
    spawn.template.flags = GO_FLAG_INTERACT_COND;
    spawn.template.raw_data[1] = 10119;
    let runtime = DbGameObjectRuntime::new(spawn);
    let session = WorldSessionState::default();

    let block = build_db_gameobject_runtime_create_block_for_quest_statuses(
        &runtime,
        &session.quest_statuses,
    )
    .unwrap();
    let (values, trailing) =
        decode_positioned_create_update_block(&block, runtime.guid(), TYPEID_GAMEOBJECT);

    assert!(trailing.is_empty());
    assert_eq!(
        values[GAMEOBJECT_DYN_FLAGS],
        Some(GO_DYNFLAG_LO_ACTIVATE | GO_DYNFLAG_LO_SPARKLE)
    );
}

#[test]
fn gameobject_query_response_matches_cmangos_shape() {
    let template = wow_db::GameObjectTemplateQuery {
        entry: 161557,
        object_type: GO_TYPE_GOOBER,
        display_id: 35,
        name: "Milly's Harvest".to_string(),
        icon_name: "Attack".to_string(),
        faction: 0,
        flags: 0,
        size: 1.0,
        raw_data: [1; 24],
    };

    let body = build_gameobject_query_response(template.entry, Some(&template));
    assert_eq!(&body[0..4], &template.entry.to_le_bytes());
    assert_eq!(&body[4..8], &(GO_TYPE_GOOBER as u32).to_le_bytes());
    assert_eq!(&body[8..12], &35u32.to_le_bytes());
    assert!(body
        .windows("Milly's Harvest\0".len())
        .any(|w| w == b"Milly's Harvest\0"));
    assert!(body.windows("Attack\0".len()).any(|w| w == b"Attack\0"));
    assert_eq!(
        body.len(),
        12 + "Milly's Harvest\0".len() + 3 + "Attack\0".len() + 24 * 4
    );
}

#[test]
fn self_spawn_update_chunks_without_legacy_fixture_blocks() {
    let character = test_character(1, 1);
    let world_stats = PlayerWorldStats {
        base_health: 20,
        base_mana: 0,
        stats: [23, 20, 22, 20, 21],
        next_level_xp: 400,
    };
    let gameobject = DbGameObjectRuntime::new(test_gameobject_spawn(55, GO_TYPE_CHEST));
    let quest_statuses = HashMap::new();
    let update = SelfSpawnUpdate {
        character: &character,
        inventory: &[],
        world_stats: &world_stats,
        skills: &[],
        quest_statuses: &quest_statuses,
        equipped_templates: &[],
        nearby_creatures: &[],
        nearby_gameobjects: &[gameobject],
        nearby_player_corpses: &[],
    };

    let bodies = build_self_spawn_update_bodies(&update).unwrap();

    assert!(!bodies.is_empty());
}

#[test]
fn gameobject_visibility_stages_destroy_for_out_of_range_objects() {
    let nearby = test_gameobject_spawn(161557, GO_TYPE_GOOBER);
    let out_of_range = wow_db::GameObjectSpawnQuery {
        guid: 78,
        position_x: -8700.0,
        position_y: -10.0,
        ..test_gameobject_spawn(161558, GO_TYPE_GOOBER)
    };
    let out_guid = gameobject_spawn_guid(&out_of_range).raw();
    let mut session = WorldSessionState::default();
    session
        .db_gameobjects
        .insert(out_guid, DbGameObjectRuntime::new(out_of_range));
    session.last_gameobject_visibility_position =
        Some(WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0));

    let updates = stage_db_gameobject_visibility_updates(
        &mut session,
        WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
        vec![DbGameObjectRuntime::new(nearby)],
        Instant::now(),
    )
    .unwrap();

    assert!(updates
        .destroy_guids
        .iter()
        .any(|guid| guid.raw() == out_guid));
    assert!(!updates.create_bodies.is_empty());
}

#[test]
fn gameobject_visibility_stages_destroy_for_shared_consumed_object() {
    let now = Instant::now();
    let spawn = test_gameobject_spawn(161557, GO_TYPE_GOOBER);
    let guid = gameobject_spawn_guid(&spawn).raw();
    let mut session = WorldSessionState::default();
    session
        .db_gameobjects
        .insert(guid, DbGameObjectRuntime::new(spawn.clone()));
    let mut shared = DbGameObjectRuntime::new(spawn);
    shared.mark_consumed(now);

    let updates = stage_db_gameobject_visibility_updates(
        &mut session,
        WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
        vec![shared],
        now,
    )
    .unwrap();

    assert!(updates
        .destroy_guids
        .iter()
        .any(|destroy| destroy.raw() == guid));
    assert!(!session.db_gameobjects.get(&guid).unwrap().client_visible);
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

    let updates = stage_db_creature_visibility_updates(
        &mut session,
        WorldPosition::new(0, -8870.0, -112.0, 83.5, 0.0),
        vec![known, new_spawn]
            .into_iter()
            .map(DbCreatureRuntime::new)
            .collect(),
    )
    .unwrap();
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
fn movement_visibility_tracks_persisted_dead_creature_without_create_block() {
    let dead_spawn = CreatureSpawnQuery {
        guid: 45,
        entry: 6,
        position_x: -8790.0,
        position_y: -95.0,
        ..test_creature_spawn(6)
    };
    let now = Instant::now();
    let dead_runtime =
        DbCreatureRuntime::new_with_persisted_respawn(dead_spawn.clone(), now, 1_000, Some(1_120));
    let dead_guid = creature_spawn_guid(&dead_spawn).raw();
    let mut session = WorldSessionState::default();

    let updates = stage_db_creature_visibility_updates(
        &mut session,
        WorldPosition::new(0, -8790.0, -95.0, 83.5, 0.0),
        vec![dead_runtime],
    )
    .unwrap();

    assert!(updates.create_bodies.is_empty());
    assert!(updates.destroy_guids.is_empty());
    let runtime = session.db_creatures.get(&dead_guid).unwrap();
    assert_eq!(runtime.life_state, DbCreatureLifeState::Dead);
    assert_eq!(runtime.respawn_epoch_secs, Some(1_120));
    assert!(runtime.is_ready_to_respawn(now + Duration::from_secs(120)));
}

#[test]
fn movement_visibility_refreshes_existing_creature_from_shared_dead_snapshot() {
    let spawn = CreatureSpawnQuery {
        guid: 45,
        entry: 6,
        position_x: -8790.0,
        position_y: -95.0,
        ..test_creature_spawn(6)
    };
    let guid = creature_spawn_guid(&spawn).raw();
    let mut local = DbCreatureRuntime::new(spawn.clone());
    local.client_visible = true;
    local.motion = CreatureMotionState::Waypoint(CreatureWaypointMotion {
        node_index: 0,
        start: WorldPosition::new(0, -8790.0, -95.0, 83.5, 0.0),
        destination: WorldPosition::new(0, -8788.0, -95.0, 83.5, 0.0),
        path: vec![WorldPosition::new(0, -8788.0, -95.0, 83.5, 0.0)],
        started_at: Instant::now(),
        duration: Duration::from_secs(2),
    });
    let mut shared = DbCreatureRuntime::new(spawn);
    shared.health = 0;
    shared.life_state = DbCreatureLifeState::Dead;
    shared.client_visible = false;
    shared.lootable = false;
    shared.looting = false;
    let mut session = WorldSessionState::default();
    session.db_creatures.insert(guid, local);

    let updates = stage_db_creature_visibility_updates(
        &mut session,
        WorldPosition::new(0, -8790.0, -95.0, 83.5, 0.0),
        vec![shared],
    )
    .unwrap();

    assert_eq!(updates.destroy_guids, vec![ObjectGuid::from_raw(guid)]);
    let runtime = session.db_creatures.get(&guid).unwrap();
    assert_eq!(runtime.life_state, DbCreatureLifeState::Dead);
    assert!(!runtime.client_visible);
    assert!(!runtime.lootable);
    assert!(matches!(runtime.motion, CreatureMotionState::Idle));
}

#[test]
fn db_creature_death_motion_stop_clears_active_motion() {
    let spawn = CreatureSpawnQuery {
        guid: 45,
        entry: 6,
        position_x: -8790.0,
        position_y: -95.0,
        ..test_creature_spawn(6)
    };
    let guid = creature_spawn_guid(&spawn);
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.motion = CreatureMotionState::Waypoint(CreatureWaypointMotion {
        node_index: 0,
        start: WorldPosition::new(0, -8790.0, -95.0, 83.5, 0.0),
        destination: WorldPosition::new(0, -8788.0, -95.0, 83.5, 0.0),
        path: vec![WorldPosition::new(0, -8788.0, -95.0, 83.5, 0.0)],
        started_at: Instant::now(),
        duration: Duration::from_secs(2),
    });
    let mut session = WorldSessionState::default();
    session.db_creatures.insert(guid.raw(), runtime);

    let body = build_db_creature_motion_stop_body(&mut session, guid)
        .unwrap()
        .expect("motion stop body");

    assert!(!body.is_empty());
    assert!(matches!(
        session.db_creatures.get(&guid.raw()).unwrap().motion,
        CreatureMotionState::Idle
    ));
}

#[test]
fn movement_visibility_recreates_unloaded_corpse_before_respawn() {
    let spawn = CreatureSpawnQuery {
        guid: 45,
        entry: 6,
        position_x: -8790.0,
        position_y: -95.0,
        ..test_creature_spawn(6)
    };
    let guid = creature_spawn_guid(&spawn).raw();
    let mut corpse = DbCreatureRuntime::new(spawn.clone());
    corpse.begin_corpse(Instant::now(), 1_000);
    let mut session = WorldSessionState::default();
    session.db_creatures.insert(guid, corpse);

    let unload = stage_db_creature_visibility_updates(
        &mut session,
        WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(unload.destroy_guids, vec![ObjectGuid::from_raw(guid)]);
    assert!(!session.db_creatures.get(&guid).unwrap().client_visible);
    assert_eq!(
        session.db_creatures.get(&guid).unwrap().life_state,
        DbCreatureLifeState::Corpse
    );

    let reload = stage_db_creature_visibility_updates(
        &mut session,
        WorldPosition::new(0, -8790.0, -95.0, 83.5, 0.0),
        vec![DbCreatureRuntime::new(spawn)],
    )
    .unwrap();

    assert_eq!(reload.create_count, 1);
    assert!(reload.destroy_guids.is_empty());
    assert!(session.db_creatures.get(&guid).unwrap().client_visible);
    let body = &reload.create_bodies[0];
    let block_start = 5;
    let packed_guid_mask = body[block_start + 1];
    let update_flags_offset = block_start + 1 + 1 + packed_guid_mask.count_ones() as usize + 1;
    let values_start = update_flags_offset + 1 + 56;
    let values = decode_update_values(&body[values_start..]);
    assert_eq!(values[UNIT_FIELD_HEALTH], Some(0));
    assert_eq!(values[UNIT_DYNAMIC_FLAGS], Some(UNIT_DYNFLAG_LOOTABLE));
    assert_eq!(values[UNIT_NPC_FLAGS], Some(0));
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

    let updates = stage_db_creature_visibility_updates(
        &mut session,
        WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
        vec![nearby]
            .into_iter()
            .map(DbCreatureRuntime::new)
            .collect(),
    )
    .unwrap();

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
fn movement_visibility_retains_recently_visible_creature_until_unload_radius() {
    let nearby = test_creature_spawn(197);
    let edge_visible = CreatureSpawnQuery {
        guid: 45,
        entry: 6,
        position_x: -8835.0,
        position_y: -130.0,
        ..test_creature_spawn(6)
    };
    let nearby_guid = creature_spawn_guid(&nearby).raw();
    let edge_visible_guid = creature_spawn_guid(&edge_visible);
    let mut session = WorldSessionState::default();
    session
        .db_creatures
        .insert(nearby_guid, DbCreatureRuntime::new(nearby.clone()));
    session.db_creatures.insert(
        edge_visible_guid.raw(),
        DbCreatureRuntime::new(edge_visible),
    );

    let updates = stage_db_creature_visibility_updates(
        &mut session,
        WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
        vec![nearby]
            .into_iter()
            .map(DbCreatureRuntime::new)
            .collect(),
    )
    .unwrap();

    assert!(updates.create_bodies.is_empty());
    assert!(updates.destroy_guids.is_empty());
    assert!(session.db_creatures.contains_key(&edge_visible_guid.raw()));
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

    let updates = stage_db_creature_visibility_updates(
        &mut session,
        WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
        vec![nearby]
            .into_iter()
            .map(DbCreatureRuntime::new)
            .collect(),
    )
    .unwrap();

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
fn combat_log_spell_packets_match_cmangos_shapes() {
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    let target = ObjectGuid::new(HighGuid::Unit, 0, 45);

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

    let updates = advance_level_capped_combat_skill_maxes(2, &mut skills);

    assert_eq!(updates.len(), 3);
    assert_eq!(skills[0].max, 10);
    assert_eq!(skills[1].max, 300);
    assert_eq!(skills[2].max, 10);
    assert_eq!(skills[3].max, 10);
    assert_eq!(skills[0].value, 4);
    assert_eq!(skills[2].value, 3);
    assert_eq!(skills[3].value, 5);

    let body = build_player_skill_updates_body(7, &updates).unwrap();
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
fn db_creature_swing_timer_clamps_zero_template_time() {
    let mut spawn = test_creature_spawn(6);
    spawn.template.melee_base_attack_time = 0;
    let creature = DbCreatureRuntime::new(spawn);

    assert_eq!(creature.base_attack_duration(), Duration::from_millis(1));
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
    let victim = rust_combat_dummy_guid();
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
    let mut quest = test_quest_template(7);
    quest.rew_money_max_level = 210;

    assert_eq!(quest_xp_reward(1, &quest), 350);

    quest.quest_level = 1;
    assert_eq!(quest_xp_reward(10, &quest), 70);
}

#[test]
fn quest_visibility_enforces_level_class_and_race_masks() {
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
    let mut quest = test_quest_template(7);
    quest.min_level = 2;
    assert!(!satisfies_race_class_level(&quest, &character));

    quest.min_level = 1;
    quest.max_level = 1;
    quest.required_classes = 1 << (1 - 1);
    quest.required_races = 1 << (1 - 1);
    assert!(satisfies_race_class_level(&quest, &character));

    quest.required_classes = 1 << (2 - 1);
    assert!(!satisfies_race_class_level(&quest, &character));

    quest.required_classes = 1 << (1 - 1);
    quest.required_races = 1 << (2 - 1);
    assert!(!satisfies_race_class_level(&quest, &character));
}

#[test]
fn repeatable_quest_status_can_be_started_again_after_reward() {
    let mut repeatable = test_quest_template(7);
    repeatable.special_flags = 1;
    let complete_rewarded = CharacterQuestStatus {
        quest: 7,
        status: QUEST_STATUS_COMPLETE,
        rewarded: 1,
        mobcount1: 0,
        mobcount2: 0,
        mobcount3: 0,
        mobcount4: 0,
    };
    assert!(can_quest_be_started_from_status(
        &repeatable,
        Some(&complete_rewarded)
    ));

    let non_repeatable = test_quest_template(8);
    assert!(!can_quest_be_started_from_status(
        &non_repeatable,
        Some(&complete_rewarded)
    ));
}

#[test]
fn prev_quest_requirements_follow_positive_and_negative_rules() {
    let mut statuses = HashMap::new();
    statuses.insert(
        99,
        CharacterQuestStatus {
            quest: 99,
            status: QUEST_STATUS_COMPLETE,
            rewarded: 1,
            mobcount1: 0,
            mobcount2: 0,
            mobcount3: 0,
            mobcount4: 0,
        },
    );
    assert!(satisfies_prev_quest_requirement(&statuses, 99));
    assert!(!satisfies_prev_quest_requirement(&statuses, -99));

    statuses.insert(
        99,
        CharacterQuestStatus {
            quest: 99,
            status: QUEST_STATUS_INCOMPLETE,
            rewarded: 0,
            mobcount1: 0,
            mobcount2: 0,
            mobcount3: 0,
            mobcount4: 0,
        },
    );
    assert!(!satisfies_prev_quest_requirement(&statuses, 99));
    assert!(satisfies_prev_quest_requirement(&statuses, -99));
}

#[tokio::test]
async fn object_mgr_reuses_cached_questgiver_relations_and_templates() {
    let object_mgr = ObjectMgr::default();
    let pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world")
        .expect("lazy mysql pool should not connect");
    let quest = test_quest_template(7);

    object_mgr
        .prime_creature_start_quest_ids_for_test(197, vec![7])
        .await;
    object_mgr
        .prime_quest_template_for_test(7, Some(quest.clone()))
        .await;

    let before = object_mgr.cache_stats_snapshot();
    let first = object_mgr
        .creature_start_quests(&pool, 197)
        .await
        .expect("cached relation should load");
    let second = object_mgr
        .creature_start_quests(&pool, 197)
        .await
        .expect("cached relation should load again");

    assert_eq!(first.len(), 1);
    assert_eq!(first[0].entry, 7);
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].entry, 7);
    assert!(object_mgr
        .creature_starts_quest(&pool, 197, 7)
        .await
        .expect("cached membership should load"));
    assert_eq!(object_mgr.cache_stats_snapshot(), before);
}

#[tokio::test]
async fn object_mgr_cached_loot_templates_feed_quest_drop_selection_without_db_loads() {
    let object_mgr = ObjectMgr::default();
    let pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world")
        .expect("lazy mysql pool should not connect");
    let mut quest = test_quest_template(33);
    quest.req_item_id[0] = 777;
    quest.req_item_count[0] = 1;
    object_mgr
        .prime_quest_template_for_test(33, Some(quest))
        .await;
    object_mgr
        .prime_creature_loot_template_for_test(
            38,
            vec![
                CreatureLootQuery {
                    item: 25,
                    group_id: 0,
                    min_count: 1,
                    max_count: 1,
                    display_id: 25,
                    chance_or_quest_chance: 100.0,
                },
                CreatureLootQuery {
                    item: 777,
                    group_id: 0,
                    min_count: 1,
                    max_count: 1,
                    display_id: 777,
                    chance_or_quest_chance: -100.0,
                },
            ],
        )
        .await;
    let mut session = WorldSessionState::default();
    session.quest_statuses.insert(
        33,
        CharacterQuestStatus {
            quest: 33,
            status: QUEST_STATUS_INCOMPLETE,
            rewarded: 0,
            mobcount1: 0,
            mobcount2: 0,
            mobcount3: 0,
            mobcount4: 0,
        },
    );

    let before = object_mgr.cache_stats_snapshot();
    let selected = select_db_creature_loot_item_for_character(&object_mgr, &pool, &session, 38)
        .await
        .expect("cached loot selection should load");
    let selected = selected
        .iter()
        .find(|loot| loot.item == 777)
        .expect("quest item should be selected");

    assert_eq!(selected.item, 777);
    assert_eq!(selected.count, 1);
    assert_eq!(selected.display_id, 777);
    assert_eq!(object_mgr.cache_stats_snapshot(), before);
}

#[test]
fn exclusive_group_rejects_other_active_quests_in_group() {
    let mut quest = test_quest_template(10);
    quest.exclusive_group = 42;
    let group = vec![10, 11];
    let mut statuses = HashMap::new();
    statuses.insert(
        11,
        CharacterQuestStatus {
            quest: 11,
            status: QUEST_STATUS_INCOMPLETE,
            rewarded: 0,
            mobcount1: 0,
            mobcount2: 0,
            mobcount3: 0,
            mobcount4: 0,
        },
    );
    assert!(!satisfies_exclusive_group(&quest, &group, &statuses));

    statuses.insert(
        11,
        CharacterQuestStatus {
            quest: 11,
            status: QUEST_STATUS_COMPLETE,
            rewarded: 1,
            mobcount1: 0,
            mobcount2: 0,
            mobcount3: 0,
            mobcount4: 0,
        },
    );
    assert!(satisfies_exclusive_group(&quest, &group, &statuses));
}

#[test]
fn questgiver_list_uses_current_quest_dialog_status() {
    let guid = ObjectGuid::new(HighGuid::Unit, 197, 1);
    let mut available = test_quest_template(7);
    available.title = "Available".to_string();
    let mut incomplete = test_quest_template(8);
    incomplete.title = "Incomplete".to_string();
    let mut reward = test_quest_template(9);
    reward.title = "Reward".to_string();
    let mut unavailable = test_quest_template(10);
    unavailable.title = "Unavailable".to_string();

    let body = build_questgiver_quest_list_body(
        guid,
        &[
            QuestListItem {
                quest: available,
                dialog_status: DIALOG_STATUS_AVAILABLE,
            },
            QuestListItem {
                quest: incomplete,
                dialog_status: DIALOG_STATUS_INCOMPLETE,
            },
            QuestListItem {
                quest: reward,
                dialog_status: DIALOG_STATUS_REWARD2,
            },
            QuestListItem {
                quest: unavailable,
                dialog_status: DIALOG_STATUS_UNAVAILABLE,
            },
        ],
    );

    let mut cursor = 8;
    while body[cursor] != 0 {
        cursor += 1;
    }
    cursor += 1;
    cursor += 8;
    assert_eq!(body[cursor], 4);
    cursor += 1;

    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 7);
    assert_eq!(
        read_u32(&body, &mut cursor).unwrap(),
        DIALOG_STATUS_AVAILABLE
    );
    cursor += 4;
    while body[cursor] != 0 {
        cursor += 1;
    }
    cursor += 1;

    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 8);
    assert_eq!(
        read_u32(&body, &mut cursor).unwrap(),
        DIALOG_STATUS_INCOMPLETE
    );
    cursor += 4;
    while body[cursor] != 0 {
        cursor += 1;
    }
    cursor += 1;

    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 9);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), DIALOG_STATUS_REWARD2);
    cursor += 4;
    while body[cursor] != 0 {
        cursor += 1;
    }
    cursor += 1;

    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 10);
    assert_eq!(
        read_u32(&body, &mut cursor).unwrap(),
        DIALOG_STATUS_UNAVAILABLE
    );
}

#[test]
fn start_quest_dialog_status_distinguishes_available_gray_and_hidden() {
    assert_eq!(
        start_quest_dialog_status(true, true),
        Some(DIALOG_STATUS_AVAILABLE)
    );
    assert_eq!(
        start_quest_dialog_status(false, true),
        Some(DIALOG_STATUS_UNAVAILABLE)
    );
    assert_eq!(start_quest_dialog_status(false, false), None);
}

#[tokio::test]
async fn quest_state_refresh_sends_gray_status_for_level_locked_visible_questgiver() {
    let object_mgr = ObjectMgr::default();
    let pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world")
        .expect("lazy mysql pool should not connect");
    let mut quest = test_quest_template(18);
    quest.min_level = 2;
    quest.exclusive_group = 0;
    let giver = ObjectGuid::new(HighGuid::Unit, 823, 55);
    object_mgr
        .prime_creature_start_quest_ids_for_test(823, vec![18])
        .await;
    object_mgr
        .prime_creature_complete_quest_ids_for_test(823, Vec::new())
        .await;
    object_mgr
        .prime_quest_template_for_test(18, Some(quest))
        .await;
    object_mgr
        .prime_quest_prev_quests_for_test(18, Vec::new())
        .await;
    object_mgr
        .prime_quest_prev_chain_quests_for_test(18, Vec::new())
        .await;
    object_mgr
        .prime_exclusive_group_quests_for_test(0, Vec::new())
        .await;
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let session = WorldSessionState {
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
    let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel();
    let mut sink = WorldPacketSink::new(outbound_tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);

    send_visible_questgiver_status_updates(
        &mut sink,
        &object_mgr,
        &pool,
        shared_world,
        &session,
        &[giver],
        &mut header_crypto,
    )
    .await
    .unwrap();

    let packet = outbound_rx.try_recv().unwrap();
    assert_eq!(packet.opcode, SMSG_QUESTGIVER_STATUS);
    assert_eq!(
        packet.body,
        build_questgiver_status_body(giver, DIALOG_STATUS_UNAVAILABLE)
    );
    assert!(outbound_rx.try_recv().is_err());
}

#[test]
fn quest_reward_packets_use_item_display_ids() {
    let guid = ObjectGuid::new(HighGuid::Unit, 197, 1);
    let mut quest = test_quest_template(783);
    quest.rew_choice_item_id[0] = 25;
    quest.rew_choice_item_count[0] = 1;
    quest.rew_item_id[0] = 35;
    quest.rew_item_count[0] = 2;
    let mut displays = QuestRewardItemDisplays::default();
    displays.choice[0] = 1001;
    displays.reward[0] = 2002;

    let body = build_quest_offer_reward_body(guid, &quest, &displays);
    let mut cursor = 8;
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 783);
    while body[cursor] != 0 {
        cursor += 1;
    }
    cursor += 1;
    while body[cursor] != 0 {
        cursor += 1;
    }
    cursor += 1;
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 1);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);

    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 1);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 25);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 1);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 1001);

    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 1);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 35);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 2);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 2002);
}

#[test]
fn selected_quest_rewards_include_choice_and_fixed_items() {
    let mut quest = test_quest_template(783);
    quest.rew_choice_item_id[1] = 25;
    quest.rew_choice_item_count[1] = 1;
    quest.rew_item_id[0] = 35;
    quest.rew_item_count[0] = 2;

    let selected = selected_quest_reward_items(&quest, 1).unwrap();
    assert_eq!(
        selected,
        vec![
            QuestRewardItem { item: 25, count: 1 },
            QuestRewardItem { item: 35, count: 2 },
        ]
    );
    assert!(selected_quest_reward_items(&quest, 0).is_none());
}

#[test]
fn active_quest_log_slots_skip_abandoned_status_rows() {
    let mut statuses = HashMap::new();
    statuses.insert(
        7,
        CharacterQuestStatus {
            quest: 7,
            status: 0,
            rewarded: 0,
            mobcount1: 0,
            mobcount2: 0,
            mobcount3: 0,
            mobcount4: 0,
        },
    );
    statuses.insert(
        8,
        CharacterQuestStatus {
            quest: 8,
            status: QUEST_STATUS_INCOMPLETE,
            rewarded: 0,
            mobcount1: 0,
            mobcount2: 0,
            mobcount3: 0,
            mobcount4: 0,
        },
    );

    let active = active_quest_statuses_sorted(&statuses);
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].quest, 8);
}

#[test]
fn source_item_delivery_quest_can_complete_from_inventory() {
    let mut quest = test_quest_template(3100);
    quest.src_item_id = 9542;
    quest.src_item_count = 1;
    quest.req_item_id[0] = 9542;
    quest.req_item_count[0] = 1;
    let inventory = [CharacterInventoryItem {
        bag: INVENTORY_SLOT_BAG_0 as u32,
        slot: INVENTORY_SLOT_ITEM_START,
        item: 77,
        item_template: 9542,
        count: 1,
        durability: 0,
    }];

    assert!(quest_can_complete_from_inventory(&quest, &inventory));

    let empty_inventory = [];
    assert!(!quest_can_complete_from_inventory(&quest, &empty_inventory));
}

#[test]
fn incomplete_item_quest_can_reward_when_inventory_satisfies_objective() {
    let mut quest = test_quest_template(33);
    quest.req_item_id[0] = 777;
    quest.req_item_count[0] = 2;
    let status = CharacterQuestStatus {
        quest: 33,
        status: QUEST_STATUS_INCOMPLETE,
        rewarded: 0,
        mobcount1: 0,
        mobcount2: 0,
        mobcount3: 0,
        mobcount4: 0,
    };
    let inventory = [
        CharacterInventoryItem {
            bag: INVENTORY_SLOT_BAG_0 as u32,
            slot: INVENTORY_SLOT_ITEM_START,
            item: 77,
            item_template: 777,
            count: 1,
            durability: 0,
        },
        CharacterInventoryItem {
            bag: INVENTORY_SLOT_BAG_0 as u32,
            slot: INVENTORY_SLOT_ITEM_START + 1,
            item: 78,
            item_template: 777,
            count: 1,
            durability: 0,
        },
    ];

    assert!(quest_status_can_reward_from_inventory(
        &status, &quest, &inventory
    ));
}

#[test]
fn objective_free_quest_can_complete_on_accept() {
    let quest = test_quest_template(783);
    assert!(quest_can_complete_from_inventory(&quest, &[]));
}

#[test]
fn quest_complete_packet_matches_vanilla_reward_shape() {
    let mut quest = test_quest_template(783);
    quest.rew_item_id[0] = 25;
    quest.rew_item_count[0] = 1;
    quest.rew_item_id[1] = 35;
    quest.rew_item_count[1] = 2;

    let body = build_questgiver_quest_complete_body_with_xp(&quest, 40, 12);
    let mut cursor = 0;

    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 783);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 3);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 40);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 12);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 2);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 25);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 1);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 35);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 2);
    assert_eq!(cursor, body.len());
}

#[test]
fn quest_update_add_kill_encodes_gameobject_objective_with_high_bit() {
    let mut quest = test_quest_template(3903);
    quest.title = "Milly's Harvest".to_string();
    quest.req_creature_or_go_id = [-161557, 0, 0, 0];
    quest.req_creature_or_go_count = [8, 0, 0, 0];
    let guid = ObjectGuid::new(HighGuid::GameObject, 161557, 77);
    let body = build_quest_update_add_kill_body(&quest, guid, 0, 3);

    assert_eq!(&body[0..4], &3903u32.to_le_bytes());
    assert_eq!(&body[4..8], &(161557u32 | 0x8000_0000).to_le_bytes());
    assert_eq!(&body[8..12], &3u32.to_le_bytes());
    assert_eq!(&body[12..16], &8u32.to_le_bytes());
    assert_eq!(&body[16..24], &guid.raw().to_le_bytes());
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
fn db_creature_retaliation_can_kill_player() {
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
    assert_eq!(session.player_health, (5u32).saturating_sub(expected_hit));

    session.player_health = 1;
    let retaliation = retaliation_damage_for_db_creature(&mut session, target);
    assert_eq!(retaliation, expected_hit);
    assert_eq!(session.player_health, 0);
}

#[test]
fn player_death_update_sets_health_flags_and_release_timer() {
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let body = build_player_death_update_body(
        player,
        0,
        PLAYER_FLAGS_GHOST,
        PLAYER_FIELD_BYTE_RELEASE_TIMER,
        player_unit_flags(false),
    )
    .unwrap();
    let mut packed = Vec::new();
    PackedGuid::write(&mut packed, player).unwrap();
    let values_start = 4 + 1 + 1 + packed.len();
    let values = decode_update_values(&body[values_start..]);

    assert_eq!(values[UNIT_FIELD_HEALTH], Some(0));
    assert_eq!(values[UNIT_FIELD_FLAGS], Some(UNIT_FLAG_PLAYER_CONTROLLED));
    assert_eq!(values[PLAYER_FLAGS_FIELD], Some(PLAYER_FLAGS_GHOST));
    assert_eq!(
        values[PLAYER_FIELD_BYTES],
        Some(PLAYER_FIELD_BYTE_RELEASE_TIMER)
    );
    assert_eq!(values[UNIT_FIELD_AURA], Some(GHOST_SPELL_ID));
    assert_eq!(values[UNIT_FIELD_AURAFLAGS], Some(GHOST_AURA_FLAGS));
    assert_eq!(values[UNIT_FIELD_AURALEVELS], Some(1));
    assert_eq!(values[UNIT_FIELD_AURAAPPLICATIONS], Some(0));
}

#[test]
fn corpse_query_points_ghosts_back_to_their_body() {
    let corpse = WorldPosition::new(0, -8935.25, -142.5, 83.0, 1.0);
    let body = build_corpse_query_body(Some(corpse));

    assert_eq!(body[0], 1);
    assert_eq!(&body[1..5], &(corpse.map_id as i32).to_le_bytes());
    assert_eq!(&body[5..9], &corpse.x.to_le_bytes());
    assert_eq!(&body[9..13], &corpse.y.to_le_bytes());
    assert_eq!(&body[13..17], &corpse.z.to_le_bytes());
    assert_eq!(&body[17..21], &corpse.map_id.to_le_bytes());
    assert_eq!(build_corpse_query_body(None), vec![0]);
}

#[test]
fn player_corpse_reclaim_resurrects_at_ghost_position_while_query_points_to_body() {
    let ghost_position = WorldPosition::new(0, -8901.0, -130.0, 82.0, 0.25);
    let corpse_position = WorldPosition::new(0, -8935.25, -142.5, 83.0, 1.0);
    assert!(can_reclaim_corpse_at_ghost_position(
        ghost_position,
        corpse_position
    ));
    assert_eq!(ghost_position.map_id, corpse_position.map_id);
    assert_ne!(ghost_position.x, corpse_position.x);
    assert_ne!(ghost_position.y, corpse_position.y);

    let body = build_corpse_query_body(Some(corpse_position));
    assert_eq!(body[0], 1);
    assert_eq!(&body[5..9], &corpse_position.x.to_le_bytes());
    assert_eq!(&body[9..13], &corpse_position.y.to_le_bytes());
}

#[test]
fn player_corpse_create_block_uses_cmangos_corpse_fields() {
    let corpse = PlayerCorpseRuntime {
        guid: ObjectGuid::new(HighGuid::Corpse, 0, 7),
        owner: ObjectGuid::new(HighGuid::Player, 0, 7),
        position: WorldPosition::new(0, -8935.25, -142.5, 83.0, 1.0),
        corpse_type: PLAYER_CORPSE_TYPE_RESURRECTABLE_PVE,
        race: 1,
        class: 1,
        gender: 0,
        player_bytes: 0x0403_0201,
        player_bytes2: 0x0000_0005,
        equipment_cache: Some("25 0 0 0 38 0".to_string()),
        guildid: Some(123),
        player_flags: PLAYER_FLAGS_HIDE_HELM,
    };

    let block = build_player_corpse_create_block(&corpse).unwrap();
    let type_id_offset = 1 + PackedGuid::packed_size(corpse.guid);
    assert_eq!(block[0], UPDATE_TYPE_CREATE_OBJECT2);
    assert_eq!(block[type_id_offset], TYPEID_CORPSE);
    assert_eq!(
        block[type_id_offset + 1],
        UPDATEFLAG_ALL | UPDATEFLAG_HAS_POSITION
    );
    assert_eq!(
        &block[type_id_offset + 2..type_id_offset + 6],
        &corpse.position.x.to_le_bytes()
    );
    let values_start = type_id_offset + 1 + 1 + 16 + 4;
    let values = decode_update_values(&block[values_start..]);

    assert_eq!(values[0x000], Some(corpse.guid.raw() as u32));
    assert_eq!(values[0x001], Some((corpse.guid.raw() >> 32) as u32));
    assert_eq!(values[0x002], Some(TYPEMASK_OBJECT_CORPSE));
    assert_eq!(values[CORPSE_FIELD_OWNER], Some(corpse.owner.raw() as u32));
    assert_eq!(
        values[CORPSE_FIELD_OWNER + 1],
        Some((corpse.owner.raw() >> 32) as u32)
    );
    assert_eq!(values[CORPSE_FIELD_DISPLAY_ID], Some(49));
    assert_eq!(values[CORPSE_FIELD_ITEM], Some(1542 | (21 << 24)));
    assert_eq!(values[CORPSE_FIELD_ITEM + 2], Some(9891 | (4 << 24)));
    assert_eq!(values[CORPSE_FIELD_BYTES_1], Some(0x0100_0100));
    assert_eq!(values[CORPSE_FIELD_BYTES_2], Some(0x0403_0205));
    assert_eq!(values[CORPSE_FIELD_GUILD], Some(123));
    assert_eq!(
        values[CORPSE_FIELD_FLAGS],
        Some(CORPSE_FLAG_UNK2 | CORPSE_FLAG_HIDE_HELM)
    );
}

#[test]
fn player_bones_update_sets_cmangos_bones_flag() {
    let corpse = PlayerCorpseRuntime {
        guid: ObjectGuid::new(HighGuid::Corpse, 0, 7),
        owner: ObjectGuid::new(HighGuid::Player, 0, 7),
        position: WorldPosition::new(0, -8935.25, -142.5, 83.0, 1.0),
        corpse_type: PLAYER_CORPSE_TYPE_BONES,
        race: 1,
        class: 1,
        gender: 0,
        player_bytes: 0,
        player_bytes2: 0,
        equipment_cache: None,
        guildid: None,
        player_flags: PLAYER_FLAGS_HIDE_HELM | PLAYER_FLAGS_HIDE_CLOAK,
    };

    let body = build_player_corpse_bones_update_body(&corpse).unwrap();
    assert_eq!(&body[0..4], &1u32.to_le_bytes());
    assert_eq!(body[4], 0);
    let block = &body[5..];
    assert_eq!(block[0], UPDATE_TYPE_VALUES);
    let values_start = 1 + PackedGuid::packed_size(corpse.guid);
    let values = decode_update_values(&block[values_start..]);
    assert_eq!(values[CORPSE_FIELD_FLAGS], Some(CORPSE_FLAG_BONES));
}

#[test]
fn map_runtime_player_corpse_grid_load_caches_db_snapshots() {
    let position = WorldPosition::new(0, -8935.25, -142.5, 83.0, 1.0);
    let corpse = test_player_corpse_runtime(7, position);
    let mut map = MapRuntime::new(0, 0);
    let unloaded = map.unloaded_player_corpse_grids_for_area(position, 1.0);
    assert_eq!(unloaded.len(), 1);

    let loaded = map.insert_loaded_player_corpse_grid(unloaded[0], vec![corpse.clone()]);

    assert_eq!(loaded, vec![corpse.clone()]);
    assert!(map
        .unloaded_player_corpse_grids_for_area(position, 1.0)
        .is_empty());
    assert_eq!(
        map.nearby_player_corpse_snapshots(position, 20.0, PLAYER_CORPSE_VISIBILITY_LIMIT),
        vec![corpse]
    );
}

#[test]
fn map_runtime_player_corpse_snapshots_follow_map_owned_updates() {
    let first_position = WorldPosition::new(0, -8935.25, -142.5, 83.0, 1.0);
    let second_position = WorldPosition::new(0, -8835.25, -42.5, 83.0, 1.0);
    let mut corpse = test_player_corpse_runtime(7, first_position);
    let mut map = MapRuntime::new(0, 0);

    map.upsert_player_corpse(corpse.clone());
    assert_eq!(
        map.nearby_player_corpse_snapshots(first_position, 20.0, PLAYER_CORPSE_VISIBILITY_LIMIT),
        vec![corpse.clone()]
    );

    corpse.position = second_position;
    map.upsert_player_corpse(corpse.clone());

    assert!(map
        .nearby_player_corpse_snapshots(first_position, 20.0, PLAYER_CORPSE_VISIBILITY_LIMIT)
        .is_empty());
    assert_eq!(
        map.nearby_player_corpse_snapshots(second_position, 20.0, PLAYER_CORPSE_VISIBILITY_LIMIT),
        vec![corpse]
    );
}

fn test_player_corpse_runtime(counter: u32, position: WorldPosition) -> PlayerCorpseRuntime {
    PlayerCorpseRuntime {
        guid: ObjectGuid::new(HighGuid::Corpse, 0, counter),
        owner: ObjectGuid::new(HighGuid::Player, 0, counter),
        position,
        corpse_type: PLAYER_CORPSE_TYPE_RESURRECTABLE_PVE,
        race: 1,
        class: 1,
        gender: 0,
        player_bytes: 0,
        player_bytes2: 0,
        equipment_cache: None,
        guildid: None,
        player_flags: 0,
    }
}

#[test]
fn spirit_healer_detection_accepts_template_flag_or_classic_entry() {
    let mut flagged = test_creature_spawn(197);
    flagged.template.npc_flags = UNIT_NPC_FLAG_SPIRITHEALER;
    assert!(is_spirit_healer_creature(&DbCreatureRuntime::new(flagged)));

    let mut classic = test_creature_spawn(SPIRIT_HEALER_ENTRY);
    classic.template.npc_flags = 0;
    assert!(is_spirit_healer_creature(&DbCreatureRuntime::new(classic)));

    let mut trainer = test_creature_spawn(197);
    trainer.template.npc_flags = UNIT_NPC_FLAG_TRAINER;
    assert!(!is_spirit_healer_creature(&DbCreatureRuntime::new(trainer)));
}

#[test]
fn db_spirit_healer_create_block_forces_spirit_healer_npc_flag() {
    let mut healer = test_creature_spawn(SPIRIT_HEALER_ENTRY);
    healer.template.npc_flags = UNIT_NPC_FLAG_GOSSIP;
    let runtime = DbCreatureRuntime::new(healer);
    let body = build_db_creature_runtime_create_block(&runtime).unwrap();
    let packed_guid_mask = body[1];
    let update_flags_offset = 1 + 1 + packed_guid_mask.count_ones() as usize + 1;
    let values_start = update_flags_offset + 1 + 56;
    let values = decode_update_values(&body[values_start..]);

    assert_eq!(
        values[UNIT_NPC_FLAGS],
        Some(UNIT_NPC_FLAG_GOSSIP | UNIT_NPC_FLAG_SPIRITHEALER)
    );
}

#[test]
fn near_teleport_ack_body_uses_player_guid_counter_and_movement_info() {
    let character = ActiveCharacter {
        guid: 7,
        name: "Ada".to_string(),
        race: 1,
        class: 1,
        level: 1,
        xp: 0,
        position: WorldPosition::new(0, -8910.0, -140.0, 82.0, 0.5),
        movement_flags: 0,
        client_time: 123,
        fall_time: 0,
    };
    let body = build_near_teleport_ack_body(&character, 9).unwrap();
    let mut packed = Vec::new();
    PackedGuid::write(&mut packed, ObjectGuid::new(HighGuid::Player, 0, 7)).unwrap();
    let packed_len = packed.len();
    assert_eq!(&body[packed_len..packed_len + 4], &9u32.to_le_bytes());
    assert_eq!(&body[packed_len + 4..packed_len + 8], &0u32.to_le_bytes());
    assert_eq!(
        &body[packed_len + 8..packed_len + 12],
        &123u32.to_le_bytes()
    );
    assert_eq!(
        &body[packed_len + 12..packed_len + 16],
        &character.position.x.to_le_bytes()
    );
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
fn db_creature_aggro_ignores_ghost_players() {
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
    defias.template.faction = 17;
    defias.template.npc_flags = 0;
    defias.template.creature_type = 7;
    defias.template.min_level = 2;
    let attacker = creature_spawn_guid(&defias);
    let runtime = DbCreatureRuntime::new(defias);
    let mut session = WorldSessionState {
        active_character: Some(character),
        player_death_state: PlayerDeathState::Ghost,
        ..WorldSessionState::default()
    };
    session.db_creatures.insert(runtime.guid().raw(), runtime);

    assert_eq!(select_db_creature_aggro_target(&session), None);
    assert!(!begin_db_creature_combat(
        &mut session,
        attacker,
        Instant::now()
    ));
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
        PlayerMeleeCheck::Clear
    );

    session
        .db_creatures
        .get_mut(&target.raw())
        .unwrap()
        .current_position
        .x = 5.1;
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
fn melee_reach_uses_cmangos_combat_reach_and_model_scale() {
    assert_eq!(
        combined_melee_reach(PLAYER_COMBAT_REACH_YARDS, PLAYER_COMBAT_REACH_YARDS),
        ATTACK_DISTANCE_YARDS
    );

    let mut kobold = test_creature_spawn(6);
    kobold.guid = 145;
    kobold.position_x = 5.75;
    kobold.position_y = 0.0;
    kobold.position_z = 0.0;
    kobold.template.model_combat_reach = 3.0;
    kobold.template.scale = 1.0;
    let target = creature_spawn_guid(&kobold);
    let character_position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    let character = ActiveCharacter {
        guid: 7,
        name: "Ada".to_string(),
        race: 1,
        class: 1,
        level: 1,
        xp: 0,
        position: character_position,
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
        .db_creatures
        .get_mut(&target.raw())
        .unwrap()
        .current_position
        .x = 5.9;
    assert_eq!(
        db_creature_player_melee_check(&session, target),
        PlayerMeleeCheck::OutOfRange
    );

    session
        .db_creatures
        .get_mut(&target.raw())
        .unwrap()
        .spawn
        .template
        .scale = 2.0;
    assert_eq!(
        db_creature_player_melee_check(&session, target),
        PlayerMeleeCheck::Clear
    );
}

#[test]
fn db_creature_create_block_uses_model_radius_and_combat_reach() {
    let mut runtime = DbCreatureRuntime::new(test_creature_spawn(6));
    runtime.spawn.template.model_bounding_radius = 0.5;
    runtime.spawn.template.model_combat_reach = 2.0;
    runtime.spawn.template.scale = 1.25;
    let block = build_db_creature_runtime_create_block(&runtime).unwrap();
    let packed_guid_mask = block[1];
    let update_flags_offset = 1 + 1 + packed_guid_mask.count_ones() as usize + 1;
    let values_start = update_flags_offset + 1 + 56;
    let values = decode_update_values(&block[values_start..]);

    assert_eq!(
        values[UNIT_FIELD_BOUNDINGRADIUS],
        Some((0.5f32 * 1.25).to_bits())
    );
    assert_eq!(
        values[UNIT_FIELD_COMBATREACH],
        Some((2.0f32 * 1.25).to_bits())
    );
}

#[test]
fn player_melee_swing_error_packets_are_empty_vanilla_opcodes() {
    let cases = [
        (
            PlayerMeleeSwingError::NotInRange,
            SMSG_ATTACKSWING_NOTINRANGE,
        ),
        (PlayerMeleeSwingError::BadFacing, SMSG_ATTACKSWING_BADFACING),
        (
            PlayerMeleeSwingError::DeadTarget,
            SMSG_ATTACKSWING_DEADTARGET,
        ),
        (
            PlayerMeleeSwingError::CantAttack,
            SMSG_ATTACKSWING_CANT_ATTACK,
        ),
    ];

    for (error, opcode) in cases {
        let packet = error.packet();
        assert_eq!(packet.opcode, opcode);
        assert!(packet.body.is_empty());
    }
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

#[tokio::test]
async fn starter_melee_spell_failure_uses_melee_validity_before_damage() {
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
        gameobject_target: None,
    };
    let character_position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    let character = ActiveCharacter {
        guid: 7,
        name: "Ada".to_string(),
        race: 1,
        class: 1,
        level: 1,
        xp: 0,
        position: character_position,
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
    };
    let mut session = WorldSessionState {
        active_character: Some(character),
        ..WorldSessionState::default()
    };
    let runtime = DbCreatureRuntime::new(kobold);
    let maps = Arc::new(MapRuntimeManager::default());
    maps.add_player(test_player_runtime(7, SessionId(7), character_position))
        .await
        .unwrap();
    maps.share_db_creature_snapshots(0, vec![runtime]).await;
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };

    assert_eq!(
        starter_spell_melee_cast_failure(shared_world, &mut session, &starter_spell, &targets)
            .await,
        Some(SPELL_FAILED_OUT_OF_RANGE)
    );

    let mut closer = maps
        .db_creature_snapshots(0, &[target.raw()])
        .await
        .pop()
        .unwrap();
    closer.current_position.x = 4.0;
    maps.update_db_creature_snapshot(0, closer).await;
    session
        .active_character
        .as_mut()
        .unwrap()
        .position
        .orientation = std::f32::consts::PI;
    maps.sync_player_gameplay_state(0, 7, &session).await;
    assert_eq!(
        starter_spell_melee_cast_failure(shared_world, &mut session, &starter_spell, &targets)
            .await,
        Some(SPELL_FAILED_UNIT_NOT_INFRONT)
    );

    session
        .active_character
        .as_mut()
        .unwrap()
        .position
        .orientation = 0.0;
    maps.sync_player_gameplay_state(0, 7, &session).await;
    assert_eq!(
        starter_spell_melee_cast_failure(shared_world, &mut session, &starter_spell, &targets)
            .await,
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
fn db_creature_aggro_ignores_neutral_kobold_vermin() {
    let character = ActiveCharacter {
        guid: 7,
        name: "Ada".to_string(),
        race: 1,
        class: 1,
        level: 1,
        xp: 0,
        position: WorldPosition::new(0, -8783.0, -161.0, 82.0, 0.0),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
    };
    let mut kobold = test_creature_spawn(6);
    kobold.guid = 45;
    kobold.position_x = -8783.5;
    kobold.position_y = -161.0;
    kobold.template.faction = 25;
    kobold.template.npc_flags = 0;
    kobold.template.creature_type = 7;
    let runtime = DbCreatureRuntime::new(kobold);
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
        FactionReaction::Neutral,
        "Kobold Vermin faction should not auto-aggro Alliance players"
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

#[tokio::test]
async fn db_creature_combat_state_tracks_victim_and_next_swing() {
    let attacker = creature_spawn_guid(&test_creature_spawn(299));
    let now = Instant::now();
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
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
    let attacker_runtime = DbCreatureRuntime::new(test_creature_spawn(299));
    maps.share_db_creature_snapshots(0, vec![attacker_runtime.clone()])
        .await;
    session
        .db_creatures
        .insert(attacker.raw(), attacker_runtime);

    assert!(begin_shared_db_creature_combat(shared_world, &mut session, attacker, now).await);

    let combat = session
        .active_creature_combats
        .get(&attacker.raw())
        .copied()
        .expect("creature combat state should start");
    assert_eq!(combat.attacker, attacker);
    assert_eq!(combat.victim, ObjectGuid::new(HighGuid::Player, 0, 7));
    assert_eq!(combat.next_swing_at, now);

    let later = now + Duration::from_secs(1);
    assert!(!begin_shared_db_creature_combat(shared_world, &mut session, attacker, later).await);
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
        shared_world,
        0,
        &mut session,
        attacker,
        ObjectGuid::new(HighGuid::Player, 0, 7),
        overdue,
    )
    .await;
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

#[tokio::test]
async fn begin_shared_db_creature_combat_uses_mapruntime_liveness_without_session_cache() {
    let mut attacker_spawn = test_creature_spawn(299);
    attacker_spawn.guid = 333;
    let attacker = creature_spawn_guid(&attacker_spawn);
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
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
        player_death_state: PlayerDeathState::Alive,
        ..WorldSessionState::default()
    };
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(attacker_spawn)])
        .await;

    assert!(
        begin_shared_db_creature_combat(shared_world, &mut session, attacker, Instant::now()).await,
        "shared combat should start from MapRuntime even when the session viewer cache is empty"
    );
    assert!(session.db_creatures.contains_key(&attacker.raw()));
    assert!(session
        .active_creature_combats
        .contains_key(&attacker.raw()));
}

#[tokio::test]
async fn player_melee_validation_refreshes_stale_session_cache_from_mapruntime() {
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    maps.add_player(test_player_runtime(7, SessionId(7), player_position))
        .await
        .unwrap();

    let mut spawn = test_creature_spawn(6);
    spawn.guid = 334;
    spawn.position_x = 4.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let target = creature_spawn_guid(&spawn);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(spawn.clone())])
        .await;

    let mut stale_spawn = spawn;
    stale_spawn.position_x = 30.0;
    let mut session = WorldSessionState {
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
        }),
        db_creatures: HashMap::from([(target.raw(), DbCreatureRuntime::new(stale_spawn))]),
        ..WorldSessionState::default()
    };

    assert_eq!(
        db_creature_player_melee_check_from_map(shared_world, &mut session, target).await,
        PlayerMeleeCheck::Clear
    );
    assert_eq!(
        session
            .db_creatures
            .get(&target.raw())
            .unwrap()
            .current_position
            .x,
        4.0,
        "the session cache should be refreshed from the authoritative map snapshot"
    );

    session.db_creatures.clear();
    assert_eq!(
        db_creature_player_melee_check_from_map(shared_world, &mut session, target).await,
        PlayerMeleeCheck::Clear,
        "a visible map-owned creature should still validate when absent from the session cache"
    );
}

#[tokio::test]
async fn player_hit_announces_db_creature_retaliation_start() {
    let attacker_spawn = test_creature_spawn(299);
    let attacker = creature_spawn_guid(&attacker_spawn);
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
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
        player_health: 1,
        player_death_state: PlayerDeathState::Alive,
        ..WorldSessionState::default()
    };
    let attacker_runtime = DbCreatureRuntime::new(attacker_spawn);
    maps.share_db_creature_snapshots(0, vec![attacker_runtime.clone()])
        .await;
    session
        .db_creatures
        .insert(attacker.raw(), attacker_runtime);
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel();
    let mut sink = WorldPacketSink::new(outbound_tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);

    begin_db_creature_retaliation_if_needed(
        &mut sink,
        shared_world,
        0,
        &mut session,
        attacker,
        player,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let packets = std::iter::from_fn(|| outbound_rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(session
        .active_creature_combats
        .contains_key(&attacker.raw()));
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == SMSG_ATTACKSTART));
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == SMSG_UPDATE_OBJECT));
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
                -8950.0 + ATTACK_DISTANCE_YARDS - 0.1,
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
    session.active_character.as_mut().unwrap().position.x = -8950.0 - ATTACK_DISTANCE_YARDS + 0.1;
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

    session.active_character.as_mut().unwrap().position.x = -8950.0 + ATTACK_DISTANCE_YARDS;
    assert!(db_creature_can_reach_player(&session, target));

    session.active_character.as_mut().unwrap().position.x = -8950.0 + ATTACK_DISTANCE_YARDS + 0.1;
    assert!(!db_creature_can_reach_player(&session, target));
}

#[test]
fn db_creature_navigation_guardrail_blocks_aggro_and_melee_but_not_chase_pathing() {
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
        start_db_creature_chase_motion(&mut session, attacker, player, Instant::now()).is_some(),
        "blocked LOS should prevent aggro/melee, but should not stop an already-combat creature from trying to path around geometry"
    );

    session.db_creature_navigation.path_available = false;
    assert!(
        start_db_creature_chase_motion(&mut session, attacker, player, Instant::now()).is_none(),
        "path availability still gates chase movement"
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
fn map_runtime_grid_coords_match_cmangos_axis_shape() {
    let northshire = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    assert_eq!(
        grid_coord_for_position(northshire),
        GridCoord { x: 32, y: 48 }
    );

    let cell = cell_coord_for_position(northshire);
    assert!(cell.x < MAX_NUMBER_OF_CELLS);
    assert!(cell.y < MAX_NUMBER_OF_CELLS);
}

#[test]
fn map_runtime_cell_area_includes_visibility_radius_cells() {
    let northshire = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let current = (
        grid_coord_for_position(northshire),
        cell_coord_for_position(northshire),
    );
    let area = calculate_cell_area(northshire, PLAYER_VISIBILITY_RADIUS_YARDS);

    assert!(area.contains(&current));
    assert!(area.len() > 1);
    assert!(area.iter().all(|(grid, cell)| {
        grid.x < MAX_NUMBER_OF_GRIDS
            && grid.y < MAX_NUMBER_OF_GRIDS
            && cell.x < MAX_NUMBER_OF_CELLS
            && cell.y < MAX_NUMBER_OF_CELLS
    }));
}

#[test]
fn map_runtime_grid_world_bounds_contain_position() {
    let northshire = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let grid = grid_coord_for_position(northshire);
    let (min_x, max_x, min_y, max_y) = grid_world_bounds(grid);

    assert!((min_x..=max_x).contains(&northshire.x));
    assert!((min_y..=max_y).contains(&northshire.y));
}

fn grid_center_position(grid: GridCoord) -> WorldPosition {
    let (min_x, max_x, min_y, max_y) = grid_world_bounds(grid);
    WorldPosition::new(0, (min_x + max_x) / 2.0, (min_y + max_y) / 2.0, 83.5, 0.0)
}

fn map_cell_has_creature(map: &MapRuntime, position: WorldPosition, guid: u64) -> bool {
    map.grids
        .get(&grid_coord_for_position(position))
        .and_then(|grid| grid.cells.get(&cell_coord_for_position(position)))
        .is_some_and(|cell| cell.creatures.contains(&guid))
}

#[test]
fn map_runtime_lazy_creature_grid_tracks_loaded_grids_and_nearby_snapshots() {
    let mut map = MapRuntime::new(0, 0);
    let center = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let grid = grid_coord_for_position(center);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 44;
    spawn.position_x = center.x + 10.0;
    spawn.position_y = center.y;
    let creature_guid = creature_spawn_guid(&spawn).raw();

    assert_eq!(
        map.unloaded_creature_grids_for_area(center, CREATURE_SPAWN_RADIUS_YARDS),
        vec![grid]
    );
    let loaded = map.insert_loaded_creature_grid(grid, vec![DbCreatureRuntime::new(spawn)]);
    assert_eq!(loaded.len(), 1);
    assert!(map
        .unloaded_creature_grids_for_area(center, CREATURE_SPAWN_RADIUS_YARDS)
        .is_empty());

    let nearby = map.nearby_db_creature_snapshots(center, CREATURE_SPAWN_RADIUS_YARDS, 16);
    assert_eq!(nearby.len(), 1);
    assert_eq!(nearby[0].guid().raw(), creature_guid);
}

#[test]
fn map_runtime_lazy_gameobject_grid_tracks_loaded_grids_and_nearby_snapshots() {
    let mut map = MapRuntime::new(0, 0);
    let center = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let grid = grid_coord_for_position(center);
    let mut spawn = test_gameobject_spawn(161557, GO_TYPE_GOOBER);
    spawn.guid = 44;
    spawn.position_x = center.x + 10.0;
    spawn.position_y = center.y;
    let gameobject_guid = gameobject_spawn_guid(&spawn).raw();

    assert_eq!(
        map.unloaded_gameobject_grids_for_area(center, CREATURE_SPAWN_RADIUS_YARDS),
        vec![grid]
    );
    let loaded = map.insert_loaded_gameobject_grid(grid, vec![DbGameObjectRuntime::new(spawn)]);
    assert_eq!(loaded.len(), 1);
    assert!(map
        .unloaded_gameobject_grids_for_area(center, CREATURE_SPAWN_RADIUS_YARDS)
        .is_empty());

    let nearby = map.nearby_db_gameobject_snapshots(center, CREATURE_SPAWN_RADIUS_YARDS, 16);
    assert_eq!(nearby.len(), 1);
    assert_eq!(nearby[0].guid().raw(), gameobject_guid);
}

#[tokio::test]
async fn map_runtime_gameobject_consume_is_shared_and_broadcasts_destroy() {
    let maps = Arc::new(MapRuntimeManager::default());
    let center = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut spawn = test_gameobject_spawn(161557, GO_TYPE_GOOBER);
    spawn.guid = 44;
    spawn.position_x = center.x + 4.0;
    spawn.position_y = center.y;
    let guid = gameobject_spawn_guid(&spawn);
    let observer_session = SessionId::next();
    maps.add_player(PlayerRuntime {
        guid: 99,
        account_id: 1,
        session_id: observer_session,
        selected_target: None,
        active_combat_target: None,
        active_combat_next_swing_at: None,
        position: center,
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
        cell: cell_coord_for_position(center),
        visible_objects: HashSet::new(),
        last_creature_visibility_position: None,
        last_gameobject_visibility_position: None,
        last_player_corpse_visibility_position: None,
        visual: PlayerVisualState {
            gender: 0,
            player_bytes: 0,
            player_bytes2: 0,
            equipment_cache: None,
            guildid: None,
        },
        visible_equipment: [0; ENUM_EQUIPMENT_SLOTS],
        flags: 0,
        level: 1,
        race: 1,
        class: 1,
        spirit: 20,
        gender: 0,
        health: 20,
        max_health: 20,
        power1: 0,
        max_power1: 0,
        power2: 0,
        player_bytes: 0,
        player_bytes2: 0,
        active_spells: HashSet::new(),
        inventory: Vec::new(),
        quest_statuses: HashMap::new(),
        combat_stats: test_player_combat_stats(),
    })
    .await
    .unwrap();
    maps.ensure_db_gameobject_grids_loaded_for_test(0, center, CREATURE_SPAWN_RADIUS_YARDS, |_| {
        vec![DbGameObjectRuntime::new(spawn.clone())]
    })
    .await;

    let (consumed, packets) = maps
        .consume_db_gameobject(0, guid, Instant::now(), None)
        .await
        .expect("loaded gameobject should be consumable");

    assert!(consumed.consumed_until.is_some());
    assert!(packets
        .iter()
        .any(|(session_id, packet)| *session_id == observer_session
            && packet.opcode == SMSG_DESTROY_OBJECT
            && packet.body == guid.raw().to_le_bytes()));
    let snapshots = maps
        .nearby_db_gameobject_snapshots(0, center, CREATURE_SPAWN_RADIUS_YARDS, 16)
        .await;
    assert_eq!(snapshots.len(), 1);
    assert!(snapshots[0].consumed_until.is_some());
}

#[test]
fn map_runtime_db_gameobject_loot_item_is_shared_between_characters() {
    let mut map = MapRuntime::new(0, 0);
    let spawn = test_gameobject_spawn(161557, GO_TYPE_CHEST);
    let guid = gameobject_spawn_guid(&spawn).raw();
    map.insert_loaded_gameobject_grid(
        grid_coord_for_position(gameobject_spawn_position(&spawn)),
        vec![DbGameObjectRuntime::new(spawn)],
    );
    let first_loot = DbCreatureLootRuntime {
        item: 117,
        count: 1,
        display_id: 117,
    };
    let second_loot = DbCreatureLootRuntime {
        item: 118,
        count: 1,
        display_id: 118,
    };

    let first_open = map.open_db_gameobject_loot(guid, 1, vec![first_loot]);
    let second_open = map.open_db_gameobject_loot(guid, 2, vec![second_loot]);

    assert_eq!(
        first_open
            .as_ref()
            .and_then(|(_, loot)| loot.first())
            .map(|loot| loot.item),
        Some(117)
    );
    assert_eq!(
        second_open
            .as_ref()
            .and_then(|(_, loot)| loot.first())
            .map(|loot| loot.item),
        Some(117)
    );
    assert_eq!(
        map.take_db_gameobject_loot_item(1, 0)
            .map(|(_, _, loot)| loot.item),
        Some(117)
    );
    assert!(map.take_db_gameobject_loot_item(2, 0).is_none());

    map.release_db_gameobject_loot(guid, 1).unwrap();
    assert_eq!(map.db_gameobject_loot_guid_for_character(1), None);
    assert_eq!(map.db_gameobject_loot_guid_for_character(2), Some(guid));
}

#[test]
fn map_runtime_db_gameobject_loot_item_can_restore_after_failed_autostore() {
    let mut map = MapRuntime::new(0, 0);
    let spawn = test_gameobject_spawn(161557, GO_TYPE_CHEST);
    let guid = gameobject_spawn_guid(&spawn).raw();
    map.insert_loaded_gameobject_grid(
        grid_coord_for_position(gameobject_spawn_position(&spawn)),
        vec![DbGameObjectRuntime::new(spawn)],
    );
    let loot = DbCreatureLootRuntime {
        item: 117,
        count: 1,
        display_id: 117,
    };
    map.open_db_gameobject_loot(guid, 1, vec![loot.clone()])
        .expect("gameobject loot should open");
    let taken = map
        .take_db_gameobject_loot_item(1, 0)
        .expect("first shared claim should take the item");
    assert_eq!(taken.2.item, 117);
    assert!(map.take_db_gameobject_loot_item(1, 0).is_none());

    let restored = map
        .restore_db_gameobject_loot_item(guid, 0, loot)
        .expect("failed autostore should restore shared item");
    assert_eq!(restored.first().map(|loot| loot.item), Some(117));

    map.open_db_gameobject_loot(guid, 2, Vec::new())
        .expect("second character should open restored shared loot");
    let reclaimed = map.take_db_gameobject_loot_item(2, 0);
    assert_eq!(reclaimed.map(|(_, _, loot)| loot.item), Some(117));
}

#[test]
fn map_runtime_sight_aggro_uses_cell_buckets_and_detection_range() {
    let mut map = MapRuntime::new(0, 0);
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
    let grid = grid_coord_for_position(character.position);

    let mut near_hostile = test_creature_spawn(38);
    near_hostile.guid = 45;
    near_hostile.position_x = character.position.x + 8.0;
    near_hostile.template.faction = 17;
    near_hostile.template.npc_flags = 0;
    near_hostile.template.creature_type = 7;
    near_hostile.template.min_level = 1;
    near_hostile.template.detection_range = 20;
    let near_guid = creature_spawn_guid(&near_hostile);

    let mut out_of_detection = near_hostile.clone();
    out_of_detection.guid = 46;
    out_of_detection.position_x = character.position.x + 25.0;

    map.insert_loaded_creature_grid(
        grid,
        vec![
            DbCreatureRuntime::new(near_hostile),
            DbCreatureRuntime::new(out_of_detection),
        ],
    );

    let mut unindexed_hostile = test_creature_spawn(38);
    unindexed_hostile.guid = 47;
    unindexed_hostile.position_x = character.position.x + 4.0;
    unindexed_hostile.template.faction = 17;
    unindexed_hostile.template.npc_flags = 0;
    unindexed_hostile.template.creature_type = 7;
    unindexed_hostile.template.min_level = 1;
    map.creatures.insert(
        creature_spawn_guid(&unindexed_hostile).raw(),
        DbCreatureRuntime::new(unindexed_hostile),
    );

    let targets = map.select_db_creature_sight_aggro_targets(&character);

    assert_eq!(
        targets
            .into_iter()
            .map(|creature| creature.guid())
            .collect::<Vec<_>>(),
        vec![near_guid]
    );
}

#[tokio::test]
async fn map_runtime_grid_load_counters_prove_movement_reuses_loaded_area() {
    let maps = MapRuntimeManager::default();
    let center = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);

    maps.ensure_db_creature_grids_loaded_for_test(0, center, CREATURE_SPAWN_RADIUS_YARDS, |grid| {
        let mut spawn = test_creature_spawn(6);
        spawn.guid = 10_000 + grid.x * 64 + grid.y;
        spawn.position_x = center.x;
        spawn.position_y = center.y;
        vec![DbCreatureRuntime::new(spawn)]
    })
    .await;
    assert_eq!(
        maps.creature_grid_load_stats(),
        CreatureGridLoadStats {
            ensure_calls: 1,
            cache_hits: 0,
            db_queries: 1,
            rows_loaded: 1,
        }
    );

    maps.ensure_db_creature_grids_loaded_for_test(0, center, CREATURE_SPAWN_RADIUS_YARDS, |_| {
        Vec::new()
    })
    .await;
    maps.ensure_db_creature_grids_loaded_for_test(
        0,
        WorldPosition::new(
            0,
            center.x + 5.0,
            center.y + 5.0,
            center.z,
            center.orientation,
        ),
        CREATURE_SPAWN_RADIUS_YARDS,
        |_| Vec::new(),
    )
    .await;

    assert_eq!(
        maps.creature_grid_load_stats(),
        CreatureGridLoadStats {
            ensure_calls: 3,
            cache_hits: 2,
            db_queries: 1,
            rows_loaded: 1,
        },
        "movement inside already loaded cells must not become DB-query-per-heartbeat"
    );
}

#[tokio::test]
async fn map_runtime_grid_load_counters_load_new_grid_once_and_reuse_for_nearby_players() {
    let maps = MapRuntimeManager::default();
    let first = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let first_grid = grid_coord_for_position(first);
    let second_grid = GridCoord {
        x: first_grid.x + 1,
        y: first_grid.y,
    };
    let second = grid_center_position(second_grid);

    maps.ensure_db_creature_grids_loaded_for_test(0, first, 1.0, |_| Vec::new())
        .await;
    maps.ensure_db_creature_grids_loaded_for_test(0, second, 1.0, |_| Vec::new())
        .await;
    maps.ensure_db_creature_grids_loaded_for_test(
        0,
        WorldPosition::new(0, second.x + 2.0, second.y, second.z, second.orientation),
        1.0,
        |_| Vec::new(),
    )
    .await;

    assert_eq!(
        maps.creature_grid_load_stats(),
        CreatureGridLoadStats {
            ensure_calls: 3,
            cache_hits: 1,
            db_queries: 2,
            rows_loaded: 0,
        },
        "crossing into one unloaded grid should add one DB rectangle load; a nearby second player should reuse it"
    );
}

#[test]
fn map_runtime_grid_states_prepare_idle_and_unload_blockers() {
    assert_eq!(GridRuntime::default().state, GridState::Loaded);

    let mut map = MapRuntime::new(0, 0);
    let center = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let grid = grid_coord_for_position(center);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 303;
    spawn.position_x = center.x;
    spawn.position_y = center.y;
    let creature_guid = creature_spawn_guid(&spawn);
    map.insert_loaded_creature_grid(grid, vec![DbCreatureRuntime::new(spawn)]);
    assert_eq!(map.grids.get(&grid).unwrap().state, GridState::Idle);

    map.add_player(test_player_runtime(7, SessionId(7), center))
        .unwrap();
    assert_eq!(map.grids.get(&grid).unwrap().state, GridState::Active);
    let mut corpse = map.creatures.get(&creature_guid.raw()).unwrap().clone();
    corpse.begin_corpse(Instant::now(), 1_000);
    corpse.lootable = false;
    corpse.loot_money_available = false;
    map.update_db_creature_snapshot(corpse);
    let packets = map.remove_player(7);
    assert!(packets.is_empty());

    assert!(map.loaded_creature_grids.contains(&grid));
    assert_eq!(
        map.creatures.get(&creature_guid.raw()).unwrap().life_state,
        DbCreatureLifeState::Corpse,
        "logout should not unload or corrupt active shared creature state"
    );
    assert_eq!(
        map.grids.get(&grid).unwrap().state,
        GridState::UnloadBlocked(GridUnloadBlocker::Corpse)
    );

    let combat = map.begin_db_creature_combat(
        creature_guid,
        ObjectGuid::new(HighGuid::Player, 0, 99),
        Instant::now(),
    );
    assert!(combat.is_some());
    assert_eq!(
        map.grids.get(&grid).unwrap().state,
        GridState::UnloadBlocked(GridUnloadBlocker::Combat)
    );
}

#[test]
fn map_runtime_creature_cell_buckets_follow_move_return_home_and_lifecycle() {
    let mut map = MapRuntime::new(0, 0);
    let home = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let moved = WorldPosition::new(
        0,
        home.x + CELL_SIZE_YARDS * 2.0,
        home.y,
        home.z,
        home.orientation,
    );
    let grid = grid_coord_for_position(home);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 304;
    spawn.position_x = home.x;
    spawn.position_y = home.y;
    spawn.position_z = home.z;
    let creature_guid = creature_spawn_guid(&spawn);

    map.insert_loaded_creature_grid(grid, vec![DbCreatureRuntime::new(spawn)]);
    assert!(map_cell_has_creature(&map, home, creature_guid.raw()));

    let mut creature = map.creatures.get(&creature_guid.raw()).unwrap().clone();
    creature.current_position = moved;
    map.update_db_creature_snapshot(creature);
    assert!(!map_cell_has_creature(&map, home, creature_guid.raw()));
    assert!(map_cell_has_creature(&map, moved, creature_guid.raw()));

    let now = Instant::now();
    let (_, motion) = map
        .start_db_creature_return_home_motion(
            &DbCreatureNavigationGuardrail::default(),
            creature_guid,
            now,
        )
        .expect("moved creature should start returning home");
    map.advance_db_creature_motion(creature_guid, now + motion.duration);
    assert!(map_cell_has_creature(&map, home, creature_guid.raw()));
    assert!(!map_cell_has_creature(&map, moved, creature_guid.raw()));

    let mut corpse = map.creatures.get(&creature_guid.raw()).unwrap().clone();
    corpse.current_position = moved;
    corpse.begin_corpse(now, 1_000);
    corpse.lootable = false;
    corpse.loot_money_available = false;
    corpse.corpse_expires_at = Some(now);
    map.update_db_creature_snapshot(corpse);
    assert!(map_cell_has_creature(&map, moved, creature_guid.raw()));

    let events = map
        .advance_db_creature_lifecycle(&[creature_guid.raw()], home, None, now)
        .unwrap();
    assert_eq!(events.len(), 1);
    assert!(map_cell_has_creature(&map, home, creature_guid.raw()));
    assert!(!map_cell_has_creature(&map, moved, creature_guid.raw()));
}

#[test]
fn map_runtime_nearby_players_use_cell_candidates_then_distance_filter() {
    let mut map = MapRuntime::new(0, 0);
    let center = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let nearby = WorldPosition::new(0, -8950.0, -160.0, 83.5, 0.0);
    let same_area_but_outside_radius = WorldPosition::new(
        0,
        -8950.0 - PLAYER_VISIBILITY_RADIUS_YARDS - 5.0,
        -130.0,
        83.5,
        0.0,
    );
    let far = WorldPosition::new(0, -8500.0, -130.0, 83.5, 0.0);

    insert_map_runtime_player_for_test(&mut map, 1, center);
    insert_map_runtime_player_for_test(&mut map, 2, nearby);
    insert_map_runtime_player_for_test(&mut map, 3, same_area_but_outside_radius);
    insert_map_runtime_player_for_test(&mut map, 4, far);

    assert_eq!(
        map.nearby_player_guids(center, PLAYER_VISIBILITY_RADIUS_YARDS, Some(1)),
        vec![2]
    );
}

#[test]
fn map_runtime_reuses_shared_db_creature_snapshot_for_later_sessions() {
    let spawn = CreatureSpawnQuery {
        guid: 45,
        entry: 6,
        position_x: -8790.0,
        position_y: -95.0,
        ..test_creature_spawn(6)
    };
    let guid = creature_spawn_guid(&spawn).raw();
    let mut map = MapRuntime::new(0, 0);
    let mut corpse = DbCreatureRuntime::new(spawn.clone());
    corpse.begin_corpse(Instant::now(), 1_000);

    let first = map.share_db_creature_snapshots(vec![corpse]).pop().unwrap();
    assert_eq!(first.life_state, DbCreatureLifeState::Corpse);
    assert_eq!(map.creatures.len(), 1);

    let second = map
        .share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)])
        .pop()
        .unwrap();

    assert_eq!(second.guid().raw(), guid);
    assert_eq!(second.life_state, DbCreatureLifeState::Corpse);
    assert_eq!(second.health, 0);
    assert!(second.lootable);
    assert_eq!(map.creatures.len(), 1);
}

#[test]
fn map_runtime_broadcasts_db_creature_snapshot_updates_to_nearby_observers() {
    let mut map = MapRuntime::new(0, 0);
    let center = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let nearby = WorldPosition::new(0, -8950.0, -160.0, 83.5, 0.0);
    let far = WorldPosition::new(0, -8500.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, center);
    insert_map_runtime_player_for_test(&mut map, 2, nearby);
    insert_map_runtime_player_for_test(&mut map, 3, far);

    let creature = DbCreatureRuntime::new(CreatureSpawnQuery {
        guid: 45,
        entry: 6,
        position_x: center.x,
        position_y: center.y,
        position_z: center.z,
        orientation: center.orientation,
        map: center.map_id,
        ..test_creature_spawn(6)
    });
    let packets = map.update_db_creature_snapshot_and_broadcast(
        creature,
        Some(1),
        OutboundWorldPacket {
            opcode: SMSG_UPDATE_OBJECT,
            body: vec![1, 2, 3],
        },
    );

    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0].0, SessionId(2));
    assert_eq!(packets[0].1.opcode, SMSG_UPDATE_OBJECT);
    assert_eq!(packets[0].1.body, vec![1, 2, 3]);
}

#[test]
fn map_runtime_db_creature_loot_money_is_claimed_once() {
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 45;
    spawn.template.min_loot_gold = 7;
    spawn.template.max_loot_gold = 7;
    let guid = creature_spawn_guid(&spawn).raw();
    let mut creature = DbCreatureRuntime::new(spawn);
    creature.begin_corpse(Instant::now(), 1_000);
    let mut map = MapRuntime::new(0, 0);
    map.share_db_creature_snapshots(vec![creature]);
    assert!(map.open_db_creature_loot(guid, 1, Vec::new()).is_some());

    let first = map.take_db_creature_loot_money(1);
    let second = map.take_db_creature_loot_money(1);

    assert_eq!(first.map(|(money, _)| money), Some(7));
    assert!(second.is_none());
    assert!(!map.creatures.get(&guid).unwrap().loot_money_available);
}

#[test]
fn map_runtime_db_creature_loot_item_can_restore_after_failed_claim() {
    let spawn = CreatureSpawnQuery {
        guid: 45,
        entry: 6,
        ..test_creature_spawn(6)
    };
    let guid = creature_spawn_guid(&spawn).raw();
    let mut creature = DbCreatureRuntime::new(spawn);
    creature.begin_corpse(Instant::now(), 1_000);
    let loot = DbCreatureLootRuntime {
        item: 117,
        count: 1,
        display_id: 117,
    };
    let mut map = MapRuntime::new(0, 0);
    map.share_db_creature_snapshots(vec![creature]);
    assert!(map
        .open_db_creature_loot(guid, 1, vec![loot.clone()])
        .is_some());

    let first = map.take_db_creature_loot_item(1, 0);
    let second = map.take_db_creature_loot_item(1, 0);
    assert_eq!(first.as_ref().map(|(_, _, loot, _)| loot.item), Some(117));
    assert!(second.is_none());

    let restored = map.restore_db_creature_loot_item(guid, 0, loot).unwrap();
    assert_eq!(restored.loot_items.first().map(|loot| loot.item), Some(117));
    let reclaimed = map.take_db_creature_loot_item(1, 0);
    assert_eq!(reclaimed.map(|(_, _, loot, _)| loot.item), Some(117));
}

#[test]
fn map_runtime_db_creature_loot_item_is_generated_once() {
    let spawn = CreatureSpawnQuery {
        guid: 45,
        entry: 6,
        ..test_creature_spawn(6)
    };
    let guid = creature_spawn_guid(&spawn).raw();
    let mut creature = DbCreatureRuntime::new(spawn);
    creature.begin_corpse(Instant::now(), 1_000);
    let first_loot = DbCreatureLootRuntime {
        item: 117,
        count: 1,
        display_id: 117,
    };
    let second_loot = DbCreatureLootRuntime {
        item: 159,
        count: 1,
        display_id: 159,
    };
    let mut map = MapRuntime::new(0, 0);
    map.share_db_creature_snapshots(vec![creature]);

    assert_eq!(map.db_creature_needs_loot_item(guid), Some(true));
    let opened = map
        .open_db_creature_loot(guid, 1, vec![first_loot.clone()])
        .unwrap();
    assert_eq!(opened.loot_items.first().map(|loot| loot.item), Some(117));
    assert_eq!(map.db_creature_needs_loot_item(guid), Some(false));

    let reopened = map
        .open_db_creature_loot(guid, 1, vec![second_loot])
        .unwrap();
    assert_eq!(reopened.loot_items.first().map(|loot| loot.item), Some(117));
}

#[test]
fn map_runtime_db_creature_loot_release_broadcasts_cleared_flags() {
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 45;
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    let guid = creature_spawn_guid(&spawn);
    let mut creature = DbCreatureRuntime::new(spawn);
    creature.begin_corpse(Instant::now(), 1_000);
    creature.looting = true;
    creature.loot_money_available = false;
    creature.loot_items.clear();
    let mut map = MapRuntime::new(0, 0);
    map.share_db_creature_snapshots(vec![creature]);
    insert_map_runtime_player_for_test(&mut map, 1, WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0));
    insert_map_runtime_player_for_test(&mut map, 2, WorldPosition::new(0, 1.0, 0.0, 0.0, 0.0));

    let event = map
        .release_db_creature_loot(guid.raw(), Instant::now(), Some(1))
        .unwrap()
        .expect("release should produce a shared event");

    assert!(!event.creature.lootable);
    assert_eq!(event.direct_packet.opcode, SMSG_UPDATE_OBJECT);
    assert!(!event.direct_packet.body.is_empty());
    assert_eq!(event.observer_packets.len(), 1);
    assert_eq!(event.observer_packets[0].0, SessionId(2));
    assert_eq!(event.observer_packets[0].1.opcode, SMSG_UPDATE_OBJECT);
}

#[test]
fn map_runtime_db_creature_combat_claim_is_exclusive_until_cleared() {
    let mut map = MapRuntime::new(0, 0);
    let attacker = creature_spawn_guid(&test_creature_spawn(6));
    let first_victim = ObjectGuid::new(HighGuid::Player, 0, 7);
    let second_victim = ObjectGuid::new(HighGuid::Player, 0, 8);
    let now = Instant::now();

    let first = map.begin_db_creature_combat(attacker, first_victim, now);
    let duplicate_same_victim =
        map.begin_db_creature_combat(attacker, first_victim, now + Duration::from_secs(1));
    let duplicate_other_victim =
        map.begin_db_creature_combat(attacker, second_victim, now + Duration::from_secs(2));

    assert_eq!(first.map(|combat| combat.victim), Some(first_victim));
    assert!(duplicate_same_victim.is_none());
    assert!(duplicate_other_victim.is_none());

    map.clear_db_creature_combat(attacker);
    let second =
        map.begin_db_creature_combat(attacker, second_victim, now + Duration::from_secs(3));
    assert_eq!(second.map(|combat| combat.victim), Some(second_victim));
}

#[test]
fn map_runtime_db_creature_threat_records_multiple_players() {
    let mut map = MapRuntime::new(0, 0);
    let attacker = creature_spawn_guid(&test_creature_spawn(6));
    let first_victim = ObjectGuid::new(HighGuid::Player, 0, 7);
    let second_victim = ObjectGuid::new(HighGuid::Player, 0, 8);
    map.begin_db_creature_combat(attacker, first_victim, Instant::now())
        .unwrap();

    map.add_db_creature_threat(attacker, first_victim, 5.0);
    map.add_db_creature_threat(attacker, second_victim, 9.0);

    let threats = map.db_creature_threat_entries(attacker);
    assert_eq!(threats.len(), 2);
    assert_eq!(threats[0].victim, second_victim);
    assert_eq!(threats[0].threat, 9.0);
    assert_eq!(threats[1].victim, first_victim);
    assert_eq!(threats[1].threat, 5.0);
}

#[test]
fn map_runtime_db_creature_threat_uses_cmangos_switch_thresholds() {
    let mut map = MapRuntime::new(0, 0);
    let mut spawn = test_creature_spawn(6);
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    let attacker = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let current_victim = ObjectGuid::new(HighGuid::Player, 0, 7);
    let ranged_challenger = ObjectGuid::new(HighGuid::Player, 0, 8);
    let melee_challenger = ObjectGuid::new(HighGuid::Player, 0, 9);
    insert_map_runtime_player_for_test(&mut map, 7, WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0));
    insert_map_runtime_player_for_test(&mut map, 8, WorldPosition::new(0, 30.0, 0.0, 0.0, 0.0));
    insert_map_runtime_player_for_test(&mut map, 9, WorldPosition::new(0, 1.0, 0.0, 0.0, 0.0));

    map.add_db_creature_threat(attacker, current_victim, 100.0);
    map.add_db_creature_threat(attacker, ranged_challenger, 120.0);
    assert_eq!(
        map.select_db_creature_threat_victim(attacker, Some(current_victim)),
        Some(current_victim)
    );

    map.add_db_creature_threat(attacker, melee_challenger, 112.0);
    assert_eq!(
        map.select_db_creature_threat_victim(attacker, Some(current_victim)),
        Some(melee_challenger)
    );

    map.add_db_creature_threat(attacker, ranged_challenger, 20.0);
    assert_eq!(
        map.select_db_creature_threat_victim(attacker, Some(current_victim)),
        Some(ranged_challenger)
    );
}

#[test]
fn map_runtime_db_creature_damage_switches_active_target_from_threat() {
    let mut map = MapRuntime::new(0, 0);
    let mut spawn = test_creature_spawn(6);
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.template.min_level_health = 500;
    spawn.template.max_level_health = 500;
    let attacker = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let old_victim = ObjectGuid::new(HighGuid::Player, 0, 7);
    let new_victim = ObjectGuid::new(HighGuid::Player, 0, 8);
    insert_map_runtime_player_for_test(&mut map, 7, WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0));
    insert_map_runtime_player_for_test(&mut map, 8, WorldPosition::new(0, 30.0, 0.0, 0.0, 0.0));
    let now = Instant::now();
    map.begin_db_creature_combat(attacker, old_victim, now)
        .unwrap();
    map.add_db_creature_threat(attacker, old_victim, 100.0);

    let event = map
        .apply_db_creature_damage(DbCreatureDamageRequest {
            creature_guid: attacker,
            killer: new_victim,
            damage: 140,
            melee_outcome: None,
            spell_id: None,
            suppress_attacker_state: false,
            now,
            now_epoch_secs: 0,
            exclude_character_guid: Some(new_victim.counter()),
        })
        .unwrap()
        .expect("damage should apply");
    let switch = event
        .target_switch
        .expect("130 percent ranged threat should switch target");

    assert_eq!(switch.old_victim, old_victim);
    assert_eq!(switch.new_victim, new_victim);
    assert_eq!(switch.combat.victim, new_victim);
    assert_eq!(switch.direct_packets.len(), 2);
    assert_eq!(switch.observer_packets.len(), 2);
    assert_eq!(
        map.active_db_creature_combats_for_victim(old_victim).len(),
        0
    );
    assert_eq!(
        map.active_db_creature_combats_for_victim(new_victim)
            .first()
            .map(|combat| combat.attacker),
        Some(attacker)
    );
}

#[test]
fn map_runtime_db_creature_combats_clear_by_victim() {
    let mut map = MapRuntime::new(0, 0);
    let mut first_spawn = test_creature_spawn(6);
    first_spawn.guid = 44;
    let mut second_spawn = test_creature_spawn(7);
    second_spawn.guid = 45;
    let mut other_spawn = test_creature_spawn(8);
    other_spawn.guid = 46;
    let first_attacker = creature_spawn_guid(&first_spawn);
    let second_attacker = creature_spawn_guid(&second_spawn);
    let other_attacker = creature_spawn_guid(&other_spawn);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 7);
    let other_victim = ObjectGuid::new(HighGuid::Player, 0, 8);
    let now = Instant::now();

    assert!(map
        .begin_db_creature_combat(first_attacker, victim, now)
        .is_some());
    assert!(map
        .begin_db_creature_combat(second_attacker, victim, now)
        .is_some());
    assert!(map
        .begin_db_creature_combat(other_attacker, other_victim, now)
        .is_some());

    map.clear_db_creature_combats_for_victim(victim);

    assert!(map.active_db_creature_combats_for_victim(victim).is_empty());
    assert_eq!(
        map.active_db_creature_combats_for_victim(other_victim)
            .len(),
        1
    );
}

#[tokio::test]
async fn active_db_creature_combat_snapshot_uses_mapruntime_without_session_cache() {
    let maps = Arc::new(MapRuntimeManager::default());
    let player_position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    maps.add_player(test_player_runtime(7, SessionId(7), player_position))
        .await
        .unwrap();

    let mut spawn = test_creature_spawn(6);
    spawn.guid = 335;
    spawn.position_x = 1.0;
    spawn.position_y = 0.0;
    let attacker = creature_spawn_guid(&spawn);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(spawn)])
        .await;
    let victim = ObjectGuid::new(HighGuid::Player, 0, 7);
    let now = Instant::now();
    let (combat, _) = maps
        .begin_db_creature_combat(0, attacker, victim, now)
        .await
        .expect("map-owned live creature should begin combat");

    let active = maps
        .active_db_creature_combat_snapshot(0, attacker, victim)
        .await
        .expect("active creature attack should validate from MapRuntime");

    assert_eq!(active.combat.attacker, combat.attacker);
    assert_eq!(active.combat.victim, victim);
    assert_eq!(active.creature.guid(), attacker);
}

#[test]
fn map_runtime_remove_player_clears_shared_creature_combat_claims() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    let observer_position = WorldPosition::new(0, 1.0, 0.0, 0.0, 0.0);
    map.add_player(test_player_runtime(7, SessionId(7), player_position))
        .unwrap();
    map.add_player(test_player_runtime(8, SessionId(8), observer_position))
        .unwrap();
    let attacker = creature_spawn_guid(&test_creature_spawn(6));
    let victim = ObjectGuid::new(HighGuid::Player, 0, 7);
    map.begin_db_creature_combat(attacker, victim, Instant::now())
        .unwrap();
    map.add_db_creature_threat(attacker, victim, 25.0);

    let packets = map.remove_player(7);

    assert!(map.active_db_creature_combats_for_victim(victim).is_empty());
    assert!(!map.creature_threats.contains_key(&attacker.raw()));
    assert!(packets
        .iter()
        .any(|(session_id, packet)| *session_id == SessionId(8)
            && packet.opcode == SMSG_DESTROY_OBJECT));
}

#[test]
fn map_runtime_db_creature_damage_updates_shared_player_and_observers() {
    let mut map = MapRuntime::new(0, 0);
    let victim_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, victim_position);
    insert_map_runtime_player_for_test(&mut map, 2, observer_position);
    let attacker = creature_spawn_guid(&test_creature_spawn(6));
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    let now = Instant::now();
    assert!(map
        .begin_db_creature_combat(attacker, victim, now)
        .is_some());

    let event = map
        .apply_db_creature_player_damage(attacker, victim, 7, now, now + Duration::from_secs(2))
        .unwrap()
        .expect("damage event");

    assert_eq!(event.damage, 7);
    assert_eq!(event.victim_health, 13);
    assert_eq!(map.players.get(&1).unwrap().health, 13);
    assert_eq!(event.combat.next_swing_at, now + Duration::from_secs(2));
    assert_eq!(event.observer_packets.len(), 2);
    assert!(event
        .observer_packets
        .iter()
        .all(|(session_id, _)| *session_id == SessionId(2)));
    assert!(event
        .observer_packets
        .iter()
        .any(|(_, packet)| packet.opcode == SMSG_ATTACKERSTATEUPDATE));
    assert!(event
        .observer_packets
        .iter()
        .any(|(_, packet)| packet.opcode == SMSG_UPDATE_OBJECT));
}

#[test]
fn map_runtime_db_creature_damage_preserves_attacker_state_overkill_damage() {
    let mut map = MapRuntime::new(0, 0);
    let player = ObjectGuid::new(HighGuid::Player, 0, 1);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 901;
    spawn.template.min_level_health = 12;
    spawn.template.max_level_health = 12;
    let creature_guid = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);

    let event = map
        .apply_db_creature_damage(DbCreatureDamageRequest {
            creature_guid,
            killer: player,
            damage: 30,
            melee_outcome: None,
            spell_id: None,
            suppress_attacker_state: false,
            now: Instant::now(),
            now_epoch_secs: 0,
            exclude_character_guid: Some(1),
        })
        .unwrap()
        .expect("damage event");

    assert_eq!(
        event.damage, 12,
        "applied hp damage stays clamped to current hp"
    );
    let state = event.attacker_state_body.expect("attacker state");
    let mut cursor = 0;
    assert_eq!(read_u32(&state, &mut cursor).unwrap(), HITINFO_NORMALSWING2);
    cursor += PackedGuid::packed_size(player) + PackedGuid::packed_size(creature_guid);
    assert_eq!(
        read_u32(&state, &mut cursor).unwrap(),
        30,
        "attacker packet should preserve pre-clamp overkill damage"
    );
}

#[test]
fn map_runtime_db_creature_evade_waits_for_combat_timer_before_leash_check() {
    let mut map = MapRuntime::new(0, 0);
    let mut attacker_spawn = test_creature_spawn(6);
    attacker_spawn.template.pursuit = 12_000;
    let attacker = creature_spawn_guid(&attacker_spawn);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    let now = Instant::now();
    let victim_position =
        WorldPosition::new(0, DB_CREATURE_LEASH_RADIUS_YARDS + 5.0, 0.0, 0.0, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, victim_position);
    map.creatures
        .insert(attacker.raw(), DbCreatureRuntime::new(attacker_spawn));
    map.begin_db_creature_combat(attacker, victim, now)
        .expect("combat should start");

    assert!(!map.db_creature_should_evade(attacker, now + Duration::from_secs(11),));
    assert!(map.db_creature_should_evade(attacker, now + Duration::from_secs(13),));
}

#[test]
fn map_runtime_db_creature_damage_refreshes_leash_timer() {
    let mut map = MapRuntime::new(0, 0);
    let mut attacker_spawn = test_creature_spawn(6);
    attacker_spawn.template.pursuit = 4_000;
    let attacker = creature_spawn_guid(&attacker_spawn);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    let now = Instant::now();
    let victim_position =
        WorldPosition::new(0, DB_CREATURE_LEASH_RADIUS_YARDS + 5.0, 0.0, 0.0, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, victim_position);
    map.creatures
        .insert(attacker.raw(), DbCreatureRuntime::new(attacker_spawn));
    map.begin_db_creature_combat(attacker, victim, now)
        .expect("combat should start");
    assert!(map.db_creature_should_evade(attacker, now + Duration::from_secs(5),));

    let refreshed_at = now + Duration::from_secs(5);
    map.apply_db_creature_damage(DbCreatureDamageRequest {
        creature_guid: attacker,
        killer: victim,
        damage: 1,
        melee_outcome: None,
        spell_id: None,
        suppress_attacker_state: false,
        now: refreshed_at,
        now_epoch_secs: current_unix_epoch_secs(),
        exclude_character_guid: None,
    })
    .expect("damage apply should succeed")
    .expect("damage event");

    assert!(!map.db_creature_should_evade(attacker, refreshed_at + Duration::from_secs(3),));
    assert!(map.db_creature_should_evade(attacker, refreshed_at + Duration::from_secs(5),));
}

#[test]
fn map_runtime_db_creature_chase_melee_does_not_refresh_leash_timer() {
    let mut map = MapRuntime::new(0, 0);
    let attacker_spawn = test_creature_spawn(6);
    let attacker = creature_spawn_guid(&attacker_spawn);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    let now = Instant::now();
    let victim_position =
        WorldPosition::new(0, DB_CREATURE_LEASH_RADIUS_YARDS + 5.0, 0.0, 0.0, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, victim_position);
    let mut creature = DbCreatureRuntime::new(attacker_spawn);
    creature.motion = CreatureMotionState::Chase(CreatureChaseMotion {
        target: victim,
        start: creature.home_position,
        destination: victim_position,
        path: vec![victim_position],
        started_at: now,
        duration: Duration::from_secs(1),
        recheck_at: now + Duration::from_secs(1),
    });
    map.creatures.insert(attacker.raw(), creature);
    map.begin_db_creature_combat(attacker, victim, now)
        .expect("combat should start");

    let hit_while_chasing_at = now + Duration::from_secs(10);
    map.apply_db_creature_player_melee_outcome(
        attacker,
        victim,
        MeleeDamageOutcome::normal_hit(1),
        hit_while_chasing_at,
        hit_while_chasing_at + Duration::from_secs(2),
    )
    .expect("melee outcome should apply")
    .expect("damage event");

    assert!(map.db_creature_should_evade(attacker, now + Duration::from_secs(16),));
}

#[test]
fn map_runtime_db_creature_uses_template_leash_from_combat_start() {
    let mut map = MapRuntime::new(0, 0);
    let mut attacker_spawn = test_creature_spawn(6);
    attacker_spawn.template.pursuit = 60_000;
    attacker_spawn.template.leash = 12;
    let attacker = creature_spawn_guid(&attacker_spawn);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    let now = Instant::now();
    let victim_position = WorldPosition::new(0, 1.0, 0.0, 0.0, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, victim_position);
    map.creatures
        .insert(attacker.raw(), DbCreatureRuntime::new(attacker_spawn));
    map.begin_db_creature_combat(attacker, victim, now)
        .expect("combat should start");

    assert!(!map.db_creature_should_evade(attacker, now + Duration::from_secs(1),));
    map.creatures
        .get_mut(&attacker.raw())
        .expect("creature")
        .current_position
        .x = 13.0;
    assert!(map.db_creature_should_evade(attacker, now + Duration::from_secs(1),));
}

#[test]
fn map_runtime_db_creature_spell_damage_includes_combat_log_packet() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    insert_map_runtime_player_for_test(&mut map, 2, observer_position);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 178;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    let creature_guid = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let now = Instant::now();

    let event = map
        .apply_db_creature_damage(DbCreatureDamageRequest {
            creature_guid,
            killer: ObjectGuid::new(HighGuid::Player, 0, 1),
            damage: 11,
            melee_outcome: None,
            spell_id: Some(WARRIOR_HEROIC_STRIKE_RANK_1),
            suppress_attacker_state: false,
            now,
            now_epoch_secs: 2_000,
            exclude_character_guid: Some(1),
        })
        .unwrap()
        .expect("spell damage event");

    assert!(event.spell_non_melee_log_body.is_some());
    assert_eq!(event.observer_packets.len(), 3);
    assert!(event
        .observer_packets
        .iter()
        .all(|(session_id, _)| *session_id == SessionId(2)));
    assert!(event
        .observer_packets
        .iter()
        .any(|(_, packet)| packet.opcode == SMSG_SPELLNONMELEEDAMAGELOG));
    assert!(event
        .observer_packets
        .iter()
        .any(|(_, packet)| packet.opcode == SMSG_ATTACKERSTATEUPDATE));
    assert!(event
        .observer_packets
        .iter()
        .any(|(_, packet)| packet.opcode == SMSG_UPDATE_OBJECT));
}

#[test]
fn map_runtime_db_creature_damage_owns_death_and_respawn_state() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    insert_map_runtime_player_for_test(&mut map, 2, observer_position);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 77;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    spawn.spawn_time_secs_min = 7;
    spawn.spawn_time_secs_max = 7;
    let creature_guid = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    assert!(map
        .begin_db_creature_combat(
            creature_guid,
            ObjectGuid::new(HighGuid::Player, 0, 1),
            Instant::now(),
        )
        .is_some());
    let now = Instant::now();

    let event = map
        .apply_db_creature_damage(DbCreatureDamageRequest {
            creature_guid,
            killer: ObjectGuid::new(HighGuid::Player, 0, 1),
            damage: 9_999,
            melee_outcome: None,
            spell_id: None,
            suppress_attacker_state: false,
            now,
            now_epoch_secs: 2_000,
            exclude_character_guid: Some(1),
        })
        .unwrap()
        .expect("death event");

    let finalization = event
        .death_finalization
        .as_ref()
        .expect("death should produce one finalization event");
    assert_eq!(finalization.killed, creature_guid);
    assert_eq!(finalization.respawn_epoch_secs, Some(2_007));
    assert_eq!(finalization.combat_flag_packet.opcode, SMSG_UPDATE_OBJECT);
    assert_eq!(finalization.attack_stop_packet.opcode, SMSG_ATTACKSTOP);
    assert_eq!(finalization.observer_packets.len(), 2);
    assert!(finalization
        .observer_packets
        .iter()
        .all(|(session_id, _)| *session_id == SessionId(2)));
    assert!(finalization
        .observer_packets
        .iter()
        .any(|(_, packet)| packet.opcode == SMSG_UPDATE_OBJECT));
    assert!(finalization
        .observer_packets
        .iter()
        .any(|(_, packet)| packet.opcode == SMSG_ATTACKSTOP));
    assert_eq!(event.creature.life_state, DbCreatureLifeState::Corpse);
    assert_eq!(event.creature.health, 0);
    assert_eq!(event.creature.respawn_epoch_secs, Some(2_007));
    assert_eq!(
        map.creatures
            .get(&creature_guid.raw())
            .unwrap()
            .respawn_epoch_secs,
        Some(2_007)
    );
    assert!(!map
        .active_db_creature_combats_for_victim(ObjectGuid::new(HighGuid::Player, 0, 1))
        .iter()
        .any(|combat| combat.attacker == creature_guid));
    assert_eq!(event.observer_packets.len(), 2);
    assert_eq!(event.observer_packets[0].0, SessionId(2));
    assert_eq!(event.observer_packets[0].1.opcode, SMSG_ATTACKERSTATEUPDATE);
    assert_eq!(event.observer_packets[1].0, SessionId(2));
    assert_eq!(event.observer_packets[1].1.opcode, SMSG_UPDATE_OBJECT);
    assert!(map
        .apply_db_creature_damage(DbCreatureDamageRequest {
            creature_guid,
            killer: ObjectGuid::new(HighGuid::Player, 0, 2),
            damage: 9_999,
            melee_outcome: None,
            spell_id: None,
            suppress_attacker_state: false,
            now,
            now_epoch_secs: 2_001,
            exclude_character_guid: Some(2),
        },)
        .unwrap()
        .is_none());
}

#[test]
fn map_runtime_same_mob_torture_keeps_lifecycle_authoritative() {
    let mut map = MapRuntime::new(0, 0);
    let player_a_position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    let player_b_position = WorldPosition::new(0, 1.0, 0.0, 0.0, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_a_position);
    insert_map_runtime_player_for_test(&mut map, 2, player_b_position);
    let player_a = ObjectGuid::new(HighGuid::Player, 0, 1);
    let player_b = ObjectGuid::new(HighGuid::Player, 0, 2);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 177;
    spawn.position_x = 0.5;
    spawn.position_y = 0.0;
    spawn.spawn_time_secs_min = 3;
    spawn.spawn_time_secs_max = 3;
    spawn.template.min_level_health = 30;
    spawn.template.max_level_health = 30;
    spawn.template.min_loot_gold = 7;
    spawn.template.max_loot_gold = 7;
    spawn.template.corpse_decay = 1;
    let creature_guid = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let now = Instant::now();
    map.begin_db_creature_combat(creature_guid, player_a, now)
        .unwrap();

    let a_damage = map
        .apply_db_creature_damage(DbCreatureDamageRequest {
            creature_guid,
            killer: player_a,
            damage: 10,
            melee_outcome: None,
            spell_id: None,
            suppress_attacker_state: false,
            now,
            now_epoch_secs: 1_000,
            exclude_character_guid: Some(1),
        })
        .unwrap()
        .expect("A damage should apply");
    assert_eq!(a_damage.creature.health, 20);
    assert_eq!(map.creatures.get(&creature_guid.raw()).unwrap().health, 20);
    assert!(a_damage
        .observer_packets
        .iter()
        .any(|(session_id, packet)| *session_id == SessionId(2)
            && packet.opcode == SMSG_UPDATE_OBJECT));

    let b_damage = map
        .apply_db_creature_damage(DbCreatureDamageRequest {
            creature_guid,
            killer: player_b,
            damage: 15,
            melee_outcome: None,
            spell_id: None,
            suppress_attacker_state: false,
            now,
            now_epoch_secs: 1_001,
            exclude_character_guid: Some(2),
        })
        .unwrap()
        .expect("B damage should apply to the same shared creature");
    assert_eq!(b_damage.creature.health, 5);
    assert_eq!(map.creatures.get(&creature_guid.raw()).unwrap().health, 5);
    assert!(b_damage
        .observer_packets
        .iter()
        .any(|(session_id, packet)| *session_id == SessionId(1)
            && packet.opcode == SMSG_UPDATE_OBJECT));

    {
        let creature = map.creatures.get_mut(&creature_guid.raw()).unwrap();
        creature.motion = CreatureMotionState::Chase(CreatureChaseMotion {
            target: player_a,
            start: creature.current_position,
            destination: WorldPosition::new(0, 0.25, 0.0, 0.0, 0.0),
            path: vec![WorldPosition::new(0, 0.25, 0.0, 0.0, 0.0)],
            started_at: now,
            duration: Duration::from_secs(1),
            recheck_at: now + Duration::from_secs(1),
        });
    }
    let death = map
        .apply_db_creature_damage(DbCreatureDamageRequest {
            creature_guid,
            killer: player_a,
            damage: 99,
            melee_outcome: None,
            spell_id: None,
            suppress_attacker_state: false,
            now,
            now_epoch_secs: 1_002,
            exclude_character_guid: Some(1),
        })
        .unwrap()
        .expect("A kill should produce one shared death event");
    assert_eq!(death.creature.life_state, DbCreatureLifeState::Corpse);
    let death_finalization = death
        .death_finalization
        .as_ref()
        .expect("death should finalize once");
    assert_eq!(
        death_finalization
            .motion_stop_packet
            .as_ref()
            .map(|packet| packet.opcode),
        Some(SMSG_MONSTER_MOVE)
    );
    assert!(death_finalization
        .observer_packets
        .iter()
        .any(|(session_id, packet)| *session_id == SessionId(2)
            && packet.opcode == SMSG_MONSTER_MOVE));
    assert_eq!(
        map.creatures.get(&creature_guid.raw()).unwrap().life_state,
        DbCreatureLifeState::Corpse
    );
    assert!(map
        .apply_db_creature_damage(DbCreatureDamageRequest {
            creature_guid,
            killer: player_b,
            damage: 99,
            melee_outcome: None,
            spell_id: None,
            suppress_attacker_state: false,
            now,
            now_epoch_secs: 1_003,
            exclude_character_guid: Some(2),
        })
        .unwrap()
        .is_none());

    assert!(map
        .open_db_creature_loot(creature_guid.raw(), 1, Vec::new())
        .is_some());
    let first_money = map.take_db_creature_loot_money(1);
    let second_money = map.take_db_creature_loot_money(1);
    assert_eq!(first_money.map(|(money, _)| money), Some(7));
    assert!(second_money.is_none());
    let release = map
        .release_db_creature_loot(creature_guid.raw(), now, Some(1))
        .unwrap()
        .expect("loot release should be shared");
    assert!(!release.creature.lootable);
    assert_eq!(release.observer_packets.len(), 1);
    assert_eq!(release.observer_packets[0].0, SessionId(2));

    let corpse_events = map
        .advance_db_creature_lifecycle(
            &[creature_guid.raw()],
            player_a_position,
            Some(1),
            now + Duration::from_secs(1),
        )
        .unwrap();
    assert_eq!(corpse_events.len(), 1);
    assert_eq!(
        corpse_events[0].creature.life_state,
        DbCreatureLifeState::Dead
    );
    assert!(map
        .advance_db_creature_lifecycle(
            &[creature_guid.raw()],
            player_b_position,
            Some(2),
            now + Duration::from_secs(1),
        )
        .unwrap()
        .is_empty());

    let respawn_events = map
        .advance_db_creature_lifecycle(
            &[creature_guid.raw()],
            player_a_position,
            Some(1),
            now + Duration::from_secs(3),
        )
        .unwrap();
    assert_eq!(respawn_events.len(), 1);
    assert_eq!(
        respawn_events[0].creature.life_state,
        DbCreatureLifeState::Alive
    );
    assert!(map
        .advance_db_creature_lifecycle(
            &[creature_guid.raw()],
            player_b_position,
            Some(2),
            now + Duration::from_secs(3),
        )
        .unwrap()
        .is_empty());
}

#[test]
fn map_runtime_db_creature_motion_transitions_are_authoritative() {
    let mut map = MapRuntime::new(0, 0);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 188;
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let creature_guid = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let navigation = DbCreatureNavigationGuardrail::default();
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let now = Instant::now();

    let first = map
        .start_db_creature_chase_motion(
            &navigation,
            creature_guid,
            player,
            WorldPosition::new(0, 10.0, 0.0, 0.0, 0.0),
            now,
        )
        .expect("first session should start the shared chase");
    assert_eq!(first.1.spline_id, 0);
    assert!(matches!(first.0.motion, CreatureMotionState::Chase(_)));
    assert!(map
        .start_db_creature_chase_motion(
            &navigation,
            creature_guid,
            player,
            WorldPosition::new(0, 10.0, 0.0, 0.0, 0.0),
            now,
        )
        .is_none());

    let stopped = map
        .stop_db_creature_motion(creature_guid)
        .expect("stop should consume the shared chase motion");
    assert_eq!(stopped.1.spline_id, 1);
    assert!(matches!(stopped.0.motion, CreatureMotionState::Idle));
    assert!(map.stop_db_creature_motion(creature_guid).is_none());
}

#[test]
fn map_runtime_db_creature_evade_and_return_home_are_authoritative() {
    let mut map = MapRuntime::new(0, 0);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 189;
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let creature_guid = creature_spawn_guid(&spawn);
    let mut creature = DbCreatureRuntime::new(spawn);
    creature.current_position = WorldPosition::new(0, 20.0, 0.0, 0.0, 0.0);
    creature.health = 1;
    creature.lootable = true;
    map.share_db_creature_snapshots(vec![creature]);
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let now = Instant::now();
    map.begin_db_creature_combat(creature_guid, player, now)
        .unwrap();

    let evaded = map
        .prepare_db_creature_evade(creature_guid)
        .expect("evade should reset the shared creature");
    assert_eq!(evaded.health, evaded.max_health());
    assert!(!evaded.lootable);
    assert!(!map
        .active_creature_combats
        .contains_key(&creature_guid.raw()));

    let navigation = DbCreatureNavigationGuardrail::default();
    let returning = map
        .start_db_creature_return_home_motion(&navigation, creature_guid, now)
        .expect("first session should start one shared return-home motion");
    assert!(matches!(
        returning.0.motion,
        CreatureMotionState::ReturnHome(_)
    ));
    assert!(map
        .start_db_creature_return_home_motion(&navigation, creature_guid, now)
        .is_none());
}

#[test]
fn map_runtime_db_creature_assistance_call_is_shared_once() {
    let mut map = MapRuntime::new(0, 0);
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
    caller_spawn.guid = 190;
    caller_spawn.position_x = 0.0;
    caller_spawn.position_y = 0.0;
    caller_spawn.template.npc_flags = 0;
    caller_spawn.template.faction = 17;
    caller_spawn.template.call_for_help = 6;
    let caller = creature_spawn_guid(&caller_spawn);
    let mut helper_spawn = test_creature_spawn(6);
    helper_spawn.guid = 191;
    helper_spawn.position_x = 5.0;
    helper_spawn.position_y = 0.0;
    helper_spawn.template.npc_flags = 0;
    helper_spawn.template.faction = 17;
    let helper = creature_spawn_guid(&helper_spawn);
    map.share_db_creature_snapshots(vec![
        DbCreatureRuntime::new(caller_spawn),
        DbCreatureRuntime::new(helper_spawn),
    ]);

    let first = map
        .select_db_creature_assist_targets(caller, &character)
        .expect("caller should exist");
    assert_eq!(first.1, vec![helper]);
    assert!(first.0.already_called_assistance);
    let second = map
        .select_db_creature_assist_targets(caller, &character)
        .expect("caller should still exist");
    assert!(second.1.is_empty());
    assert!(
        map.creatures
            .get(&caller.raw())
            .unwrap()
            .already_called_assistance
    );
}

#[test]
fn map_runtime_db_creature_damage_preserves_melee_miss_outcome() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    insert_map_runtime_player_for_test(&mut map, 2, observer_position);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 79;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    let creature_guid = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
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

    let event = map
        .apply_db_creature_damage(DbCreatureDamageRequest {
            creature_guid,
            killer: ObjectGuid::new(HighGuid::Player, 0, 1),
            damage: 99,
            melee_outcome: Some(miss),
            spell_id: None,
            suppress_attacker_state: false,
            now: Instant::now(),
            now_epoch_secs: 2_000,
            exclude_character_guid: Some(1),
        })
        .unwrap()
        .expect("miss event");

    assert_eq!(event.damage, 0);
    assert_eq!(event.creature.health, 120);
    assert_eq!(
        u32::from_le_bytes(
            event.attacker_state_body.as_ref().unwrap()[0..4]
                .try_into()
                .unwrap(),
        ),
        HITINFO_NORMALSWING2 | HITINFO_MISS
    );
    assert_eq!(event.observer_packets[0].0, SessionId(2));
    assert_eq!(event.observer_packets[0].1.opcode, SMSG_ATTACKERSTATEUPDATE);
}

#[test]
fn map_runtime_db_creature_lifecycle_expires_and_respawns_once() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    insert_map_runtime_player_for_test(&mut map, 2, observer_position);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 88;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    spawn.spawn_time_secs_min = 3;
    spawn.spawn_time_secs_max = 3;
    spawn.template.corpse_decay = 1;
    let creature_guid = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let killed_at = Instant::now();
    map.apply_db_creature_damage(DbCreatureDamageRequest {
        creature_guid,
        killer: ObjectGuid::new(HighGuid::Player, 0, 1),
        damage: 9_999,
        melee_outcome: None,
        spell_id: None,
        suppress_attacker_state: false,
        now: killed_at,
        now_epoch_secs: 3_000,
        exclude_character_guid: Some(1),
    })
    .unwrap()
    .expect("death event");

    let corpse_events = map
        .advance_db_creature_lifecycle(
            &[creature_guid.raw()],
            player_position,
            Some(1),
            killed_at + Duration::from_secs(1),
        )
        .unwrap();

    assert_eq!(corpse_events.len(), 1);
    assert_eq!(
        corpse_events[0].creature.life_state,
        DbCreatureLifeState::Dead
    );
    assert_eq!(corpse_events[0].direct_packets.len(), 1);
    assert_eq!(
        corpse_events[0].direct_packets[0].opcode,
        SMSG_DESTROY_OBJECT
    );
    assert_eq!(corpse_events[0].observer_packets.len(), 1);
    assert_eq!(corpse_events[0].observer_packets[0].0, SessionId(2));
    assert_eq!(
        map.creatures.get(&creature_guid.raw()).unwrap().life_state,
        DbCreatureLifeState::Dead
    );
    assert!(map
        .advance_db_creature_lifecycle(
            &[creature_guid.raw()],
            observer_position,
            Some(2),
            killed_at + Duration::from_secs(1),
        )
        .unwrap()
        .is_empty());

    let respawn_events = map
        .advance_db_creature_lifecycle(
            &[creature_guid.raw()],
            player_position,
            Some(1),
            killed_at + Duration::from_secs(3),
        )
        .unwrap();

    assert_eq!(respawn_events.len(), 1);
    assert_eq!(
        respawn_events[0].creature.life_state,
        DbCreatureLifeState::Alive
    );
    assert_eq!(respawn_events[0].clear_respawn_guid, Some(88));
    assert_eq!(respawn_events[0].direct_packets.len(), 1);
    assert_eq!(
        respawn_events[0].direct_packets[0].opcode,
        SMSG_UPDATE_OBJECT
    );
    assert_eq!(respawn_events[0].observer_packets.len(), 1);
    assert_eq!(respawn_events[0].observer_packets[0].0, SessionId(2));
    assert_eq!(
        map.creatures.get(&creature_guid.raw()).unwrap().life_state,
        DbCreatureLifeState::Alive
    );
    assert!(map
        .advance_db_creature_lifecycle(
            &[creature_guid.raw()],
            observer_position,
            Some(2),
            killed_at + Duration::from_secs(3),
        )
        .unwrap()
        .is_empty());
}

fn insert_map_runtime_player_for_test(map: &mut MapRuntime, guid: u32, position: WorldPosition) {
    let grid = grid_coord_for_position(position);
    let cell = cell_coord_for_position(position);
    map.grids
        .entry(grid)
        .or_default()
        .cells
        .entry(cell)
        .or_default()
        .players
        .insert(guid);
    map.players.insert(
        guid,
        test_player_runtime(guid, SessionId(guid as u64), position),
    );
}

fn test_player_runtime(guid: u32, session_id: SessionId, position: WorldPosition) -> PlayerRuntime {
    PlayerRuntime {
        guid,
        account_id: guid,
        session_id,
        selected_target: None,
        active_combat_target: None,
        active_combat_next_swing_at: None,
        position,
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
        cell: cell_coord_for_position(position),
        visible_objects: HashSet::new(),
        last_creature_visibility_position: None,
        last_gameobject_visibility_position: None,
        last_player_corpse_visibility_position: None,
        visual: PlayerVisualState {
            gender: 0,
            player_bytes: 0,
            player_bytes2: 0,
            equipment_cache: None,
            guildid: None,
        },
        visible_equipment: [0; ENUM_EQUIPMENT_SLOTS],
        flags: 0,
        level: 1,
        race: 1,
        class: 1,
        spirit: 20,
        gender: 0,
        health: 20,
        max_health: 20,
        power1: 0,
        max_power1: 0,
        power2: 0,
        player_bytes: 0,
        player_bytes2: 0,
        active_spells: HashSet::new(),
        inventory: Vec::new(),
        quest_statuses: HashMap::new(),
        combat_stats: test_player_combat_stats(),
    }
}

fn test_player_combat_stats() -> PlayerCombatStats {
    let world_stats = PlayerWorldStats {
        base_health: 20,
        base_mana: 0,
        stats: [23, 20, 22, 20, 20],
        next_level_xp: 400,
    };
    player_combat_stats_for_values(1, 1, &world_stats, &[])
}

fn decode_other_player_create_values(block: &[u8], guid: ObjectGuid) -> Vec<Option<u32>> {
    assert_eq!(block[0], UPDATE_TYPE_CREATE_OBJECT2);
    let type_id_offset = 1 + PackedGuid::packed_size(guid);
    assert_eq!(block[type_id_offset], TYPEID_PLAYER);
    let flags_offset = type_id_offset + 1;
    assert_eq!(
        block[flags_offset],
        UPDATEFLAG_ALL | UPDATEFLAG_LIVING | UPDATEFLAG_HAS_POSITION
    );
    let values_start = flags_offset + 1 + 56;
    decode_update_values(&block[values_start..])
}

#[test]
fn other_player_create_block_includes_equipment_and_movement_state() {
    let guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 1.25);
    let mut player = test_player_runtime(7, SessionId(7), position);
    player.movement_flags = 0x21;
    player.client_time = 1234;
    player.fall_time = 456;
    player.visible_equipment[EQUIPMENT_SLOT_MAINHAND as usize] = 25;
    let block = build_other_player_create_block(&player).unwrap();
    let type_id_offset = 1 + PackedGuid::packed_size(guid);
    let movement_start = type_id_offset + 2;
    assert_eq!(
        &block[movement_start..movement_start + 4],
        &0x21u32.to_le_bytes()
    );
    assert_eq!(
        &block[movement_start + 4..movement_start + 8],
        &1234u32.to_le_bytes()
    );
    assert_eq!(
        &block[movement_start + 24..movement_start + 28],
        &456u32.to_le_bytes()
    );

    let values = decode_other_player_create_values(&block, guid);
    assert_eq!(
        values[0x104 + EQUIPMENT_SLOT_MAINHAND as usize * 12],
        Some(25)
    );
}

#[test]
fn other_player_create_block_includes_selected_target() {
    let guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let selected = ObjectGuid::new(HighGuid::Unit, 0, 99);
    let mut player =
        test_player_runtime(7, SessionId(7), WorldPosition::new(0, 1.0, 2.0, 3.0, 0.0));
    player.selected_target = Some(selected);

    let values =
        decode_other_player_create_values(&build_other_player_create_block(&player).unwrap(), guid);
    assert_eq!(values[UNIT_FIELD_TARGET], Some(selected.raw() as u32));
    assert_eq!(
        values[UNIT_FIELD_TARGET + 1],
        Some((selected.raw() >> 32) as u32)
    );
}

#[test]
fn player_visible_equipment_update_block_updates_observer_item_visual() {
    let guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let mut visible_equipment = [0; ENUM_EQUIPMENT_SLOTS];
    visible_equipment[EQUIPMENT_SLOT_MAINHAND as usize] = 25;

    let block = build_player_visible_equipment_update_block(
        7,
        &visible_equipment,
        &[EQUIPMENT_SLOT_MAINHAND],
    )
    .unwrap();
    let (values, trailing) = decode_values_update_block(&block, guid);
    assert!(trailing.is_empty());
    assert_eq!(
        values[0x104 + EQUIPMENT_SLOT_MAINHAND as usize * 12],
        Some(25)
    );
}

#[test]
fn map_runtime_idle_motion_start_guids_require_player_interest() {
    let mut map = MapRuntime::new(0, 0);
    let center = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let grid = grid_coord_for_position(center);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 305;
    spawn.position_x = center.x;
    spawn.position_y = center.y;
    spawn.position_z = center.z;
    spawn.movement_type = DB_MOTION_TYPE_RANDOM;
    spawn.spawn_dist = 5.0;
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.next_random_move_at = Some(now);
    map.insert_loaded_creature_grid(grid, vec![runtime]);

    map.add_player(test_player_runtime(8, SessionId(8), center))
        .unwrap();
    let packets = map.remove_player(8);
    assert!(packets.is_empty());
    assert_eq!(
        map.grids.get(&grid).unwrap().state,
        GridState::UnloadBlocked(GridUnloadBlocker::Timer)
    );

    assert_eq!(
        map.db_creature_idle_motion_start_guids(now),
        Vec::<u64>::new(),
        "CMaNGOS-shaped idle patrol starts should pause once no player keeps the area active"
    );
}

#[test]
fn map_runtime_idle_motion_start_guids_ignore_far_same_grid_creatures() {
    let mut map = MapRuntime::new(0, 0);
    let center = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let far_same_grid = WorldPosition::new(
        0,
        center.x + CREATURE_SPAWN_RADIUS_YARDS + 40.0,
        center.y,
        center.z,
        center.orientation,
    );
    assert_eq!(
        grid_coord_for_position(center),
        grid_coord_for_position(far_same_grid),
        "test fixture needs far creatures to share the player's grid"
    );
    let grid = grid_coord_for_position(center);
    let now = Instant::now();
    let mut runtimes = Vec::new();
    for guid in 300..(300 + DB_CREATURE_IDLE_MOTION_STARTS_PER_TICK as u32) {
        let mut spawn = test_creature_spawn(6);
        spawn.guid = guid;
        spawn.position_x = far_same_grid.x;
        spawn.position_y = far_same_grid.y;
        spawn.position_z = far_same_grid.z;
        spawn.movement_type = DB_MOTION_TYPE_WAYPOINT;
        spawn.waypoint_path = vec![test_waypoint(1, far_same_grid.x + 5.0, far_same_grid.y, 0)];
        let mut runtime = DbCreatureRuntime::new(spawn);
        runtime.next_waypoint_move_at = Some(now);
        runtimes.push(runtime);
    }

    let mut visible_spawn = test_creature_spawn(6);
    visible_spawn.guid = 999;
    visible_spawn.position_x = center.x + 5.0;
    visible_spawn.position_y = center.y;
    visible_spawn.position_z = center.z;
    visible_spawn.movement_type = DB_MOTION_TYPE_WAYPOINT;
    visible_spawn.waypoint_path = vec![test_waypoint(1, center.x + 10.0, center.y, 0)];
    let visible_guid = creature_spawn_guid(&visible_spawn);
    let mut visible_runtime = DbCreatureRuntime::new(visible_spawn);
    visible_runtime.next_waypoint_move_at = Some(now);
    runtimes.push(visible_runtime);

    map.insert_loaded_creature_grid(grid, runtimes);
    map.add_player(test_player_runtime(8, SessionId(8), center))
        .unwrap();

    assert_eq!(
        map.db_creature_idle_motion_start_guids(now),
        vec![visible_guid.raw()],
        "same-grid creatures outside visibility should not starve the nearby patrol start budget"
    );
}

#[test]
fn map_runtime_idle_motion_tick_is_once_per_map_tick() {
    let mut map = MapRuntime::new(0, 0);
    let center = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let now = Instant::now();
    let mut runtimes = Vec::new();
    for guid in 300..(300 + DB_CREATURE_IDLE_MOTION_STARTS_PER_TICK as u32 + 1) {
        let mut spawn = test_creature_spawn(6);
        spawn.guid = guid;
        spawn.position_x = center.x;
        spawn.position_y = center.y;
        spawn.position_z = center.z;
        spawn.movement_type = DB_MOTION_TYPE_WAYPOINT;
        spawn.waypoint_path = vec![test_waypoint(1, center.x + 5.0, center.y, 0)];
        let mut runtime = DbCreatureRuntime::new(spawn);
        runtime.next_waypoint_move_at = Some(now);
        runtimes.push(runtime);
    }
    map.insert_loaded_creature_grid(grid_coord_for_position(center), runtimes);
    map.add_player(test_player_runtime(8, SessionId(8), center))
        .unwrap();

    let first = map
        .advance_active_db_creature_idle_motions(&DbCreatureNavigationGuardrail::default(), now)
        .unwrap();
    assert_eq!(
        first
            .packets
            .iter()
            .filter(|(_, packet)| packet.opcode == SMSG_MONSTER_MOVE)
            .count(),
        DB_CREATURE_IDLE_MOTION_STARTS_PER_TICK
    );

    let duplicate = map
        .advance_active_db_creature_idle_motions(
            &DbCreatureNavigationGuardrail::default(),
            now + Duration::from_millis(1),
        )
        .unwrap();
    assert!(duplicate.creatures.is_empty());
    assert!(duplicate.packets.is_empty());

    let next = map
        .advance_active_db_creature_idle_motions(
            &DbCreatureNavigationGuardrail::default(),
            now + Duration::from_millis(WORLD_TICK_MILLIS),
        )
        .unwrap();
    assert_eq!(
        next.packets
            .iter()
            .filter(|(_, packet)| packet.opcode == SMSG_MONSTER_MOVE)
            .count(),
        1
    );
}

#[tokio::test]
async fn shared_db_creature_idle_motion_prioritizes_player_interest_over_far_guid_order() {
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let player_position = WorldPosition::new(0, 0.0, 0.0, 83.5, 0.0);
    maps.add_player(test_player_runtime(1, SessionId(1), player_position))
        .await
        .unwrap();

    let (direct_tx, mut direct_rx) = mpsc::unbounded_channel();
    sessions
        .register(
            SessionId(1),
            SessionHandle {
                account_id: 1,
                character_guid: Some(1),
                outbound: direct_tx,
            },
        )
        .await;
    let now = Instant::now();

    let mut runtimes = Vec::new();
    for guid in 300..304 {
        let mut spawn = test_creature_spawn(6);
        spawn.guid = guid;
        spawn.position_x = 4000.0;
        spawn.position_y = 4000.0;
        spawn.position_z = 83.5;
        spawn.movement_type = DB_MOTION_TYPE_RANDOM;
        spawn.spawn_dist = 5.0;
        let mut runtime = DbCreatureRuntime::new(spawn);
        runtime.next_random_move_at = Some(now);
        runtimes.push(runtime);
    }

    let mut valid_spawn = test_creature_spawn(6);
    valid_spawn.guid = 304;
    valid_spawn.position_x = 0.0;
    valid_spawn.position_y = 0.0;
    valid_spawn.position_z = 83.5;
    valid_spawn.movement_type = DB_MOTION_TYPE_WAYPOINT;
    valid_spawn.waypoint_path = vec![test_waypoint(1, 5.0, 0.0, 0)];
    let valid_guid = creature_spawn_guid(&valid_spawn);
    let mut valid_runtime = DbCreatureRuntime::new(valid_spawn);
    valid_runtime.next_waypoint_move_at = Some(now);
    runtimes.push(valid_runtime);

    maps.share_db_creature_snapshots(0, runtimes).await;

    let tick = maps
        .advance_all_active_db_creature_idle_motions(&DbCreatureNavigationGuardrail::default(), now)
        .await
        .unwrap();
    sessions.dispatch(tick.packets).await;

    assert_eq!(
        direct_rx.try_recv().unwrap().opcode,
        SMSG_UPDATE_OBJECT,
        "nearby patrol should recreate local visibility before motion when the session has not streamed the creature yet"
    );
    assert_eq!(
        direct_rx.try_recv().unwrap().opcode,
        SMSG_MONSTER_MOVE,
        "nearby patrol should still start even when lower GUID patrols exist in unrelated far grids"
    );
    let creature = maps
        .db_creature_snapshots(0, &[valid_guid.raw()])
        .await
        .pop()
        .expect("valid creature should stay loaded");
    assert!(
        matches!(creature.motion, CreatureMotionState::Waypoint(_)),
        "nearby creature should enter waypoint motion instead of starving behind far-map GUID order"
    );
}

#[test]
fn map_runtime_player_health_update_refreshes_shared_state_and_observers() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), player_position))
        .unwrap();
    map.add_player(test_player_runtime(2, SessionId(2), observer_position))
        .unwrap();

    let packets = map.update_player_health(1, 10).unwrap();

    assert_eq!(map.players.get(&1).unwrap().health, 10);
    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0].0, SessionId(2));
    assert_eq!(packets[0].1.opcode, SMSG_UPDATE_OBJECT);
    let player = ObjectGuid::new(HighGuid::Player, 0, 1);
    let (values, trailing) = decode_values_update_block(&packets[0].1.body[5..], player);
    assert!(trailing.is_empty());
    assert_eq!(values[UNIT_FIELD_HEALTH], Some(10));
}

#[test]
fn map_runtime_player_movement_preserves_db_creature_visibility_set() {
    let mut map = MapRuntime::new(0, 0);
    let start = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), start))
        .unwrap();
    let creature_guid = ObjectGuid::new(HighGuid::Unit, 6, 77);
    map.update_player_db_creature_visibility(1, &[creature_guid], &[]);

    let movement = MovementInfo {
        flags: 0,
        client_time: 1,
        position: WorldPosition::new(0, -8949.0, -130.0, 83.5, 0.0),
        fall_time: 0,
    };
    map.update_player_position(1, MSG_MOVE_HEARTBEAT as u16, &movement)
        .unwrap();

    assert!(map
        .players
        .get(&1)
        .unwrap()
        .visible_objects
        .contains(&creature_guid));
}

#[test]
fn map_runtime_stages_db_creature_visibility_from_player_visible_set() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), player_position))
        .unwrap();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 77;
    spawn.position_x = player_position.x + 5.0;
    spawn.position_y = player_position.y;
    let runtime = DbCreatureRuntime::new(spawn);
    let creature_guid = runtime.guid();
    map.share_db_creature_snapshots(vec![runtime.clone()]);

    let first = map.stage_player_db_creature_visibility(1, player_position, vec![runtime.clone()]);

    assert_eq!(first.create_guids, vec![creature_guid]);
    assert!(first.destroy_guids.is_empty());
    assert!(map
        .players
        .get(&1)
        .unwrap()
        .visible_objects
        .contains(&creature_guid));

    let far_position = WorldPosition::new(
        0,
        player_position.x + CREATURE_VISIBILITY_UNLOAD_RADIUS_YARDS + 10.0,
        player_position.y,
        player_position.z,
        player_position.orientation,
    );
    let second = map.stage_player_db_creature_visibility(1, far_position, Vec::new());

    assert!(second.create_guids.is_empty());
    assert_eq!(second.destroy_guids, vec![creature_guid]);
    assert!(!map
        .players
        .get(&1)
        .unwrap()
        .visible_objects
        .contains(&creature_guid));
}

#[test]
fn map_runtime_player_gameplay_sync_owns_session_mutable_state() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(1, SessionId(1), position);
    player.max_power1 = 20;
    map.add_player(player).unwrap();
    let mut session = WorldSessionState {
        active_character: Some(ActiveCharacter {
            guid: 1,
            name: "Ada".to_string(),
            race: 1,
            class: 1,
            level: 1,
            xp: 0,
            position,
            movement_flags: 0,
            client_time: 0,
            fall_time: 0,
        }),
        player_health: 15,
        player_mana: 7,
        player_rage: 11,
        ..WorldSessionState::default()
    };
    session.active_spells.insert(WARRIOR_HEROIC_STRIKE_RANK_1);
    session.inventory.push(CharacterInventoryItem {
        bag: 0,
        slot: 23,
        item: 100,
        item_template: RUST_COMBAT_DUMMY_LOOT_ITEM,
        count: 1,
        durability: 0,
    });
    session.quest_statuses.insert(
        33,
        CharacterQuestStatus {
            quest: 33,
            status: QUEST_STATUS_INCOMPLETE,
            rewarded: 0,
            mobcount1: 1,
            mobcount2: 0,
            mobcount3: 0,
            mobcount4: 0,
        },
    );

    map.sync_player_gameplay_state(1, &session);

    let snapshot = map.player_runtime_snapshot(1).unwrap();
    assert_eq!(snapshot.health, 15);
    assert_eq!(snapshot.power1, 7);
    assert_eq!(snapshot.power2, 11);
    assert!(snapshot
        .active_spells
        .contains(&WARRIOR_HEROIC_STRIKE_RANK_1));
    assert_eq!(snapshot.inventory.len(), 1);
    assert_eq!(snapshot.quest_statuses.get(&33).unwrap().mobcount1, 1);
}

#[tokio::test]
async fn session_cache_refresh_preserves_map_owned_regen_before_session_sync() {
    let maps = Arc::new(MapRuntimeManager::default());
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(7, SessionId(7), position);
    player.class = 1;
    player.spirit = 30;
    player.health = 10;
    player.max_health = 80;
    player.power2 = 100;
    maps.add_player(player).await.unwrap();

    let now = Instant::now();
    assert!(maps
        .advance_all_player_regen_ticks(now)
        .await
        .unwrap()
        .is_empty());
    let packets = maps
        .advance_all_player_regen_ticks(now + Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(packets.len(), 2);

    let stale_session_health = 10;
    let stale_session_rage = 100;
    let mut session = WorldSessionState {
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
        }),
        player_health: stale_session_health,
        player_rage: stale_session_rage,
        ..WorldSessionState::default()
    };

    refresh_active_player_session_cache(&maps, &mut session).await;
    assert!(session.player_health > stale_session_health);
    assert!(session.player_rage < stale_session_rage);

    sync_active_player_gameplay_state(&maps, &session).await;
    let snapshot = maps.player_runtime_snapshot(0, 7).await.unwrap();
    assert_eq!(snapshot.health, session.player_health);
    assert_eq!(snapshot.power2, session.player_rage);
}

#[test]
fn sync_player_gameplay_state_raises_map_max_health_for_regen_cap() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(7, SessionId(7), position);
    player.class = 1;
    player.spirit = 70;
    player.health = 60;
    player.max_health = 60;
    map.add_player(player).unwrap();

    let session = WorldSessionState {
        active_character: Some(ActiveCharacter {
            guid: 7,
            name: "Ada".to_string(),
            race: 1,
            class: 1,
            level: 2,
            xp: 0,
            position,
            movement_flags: 0,
            client_time: 0,
            fall_time: 0,
        }),
        player_health: 98,
        ..WorldSessionState::default()
    };
    map.sync_player_gameplay_state(7, &session);
    assert_eq!(map.players.get(&7).unwrap().max_health, 98);
    assert_eq!(map.players.get(&7).unwrap().health, 98);

    map.players.get_mut(&7).unwrap().health = 60;
    let now = Instant::now();
    assert!(map.advance_player_regen_tick(now).unwrap().is_empty());
    map.advance_player_regen_tick(now + Duration::from_secs(2))
        .unwrap();
    assert!(map.players.get(&7).unwrap().health > 60);
    assert!(map.players.get(&7).unwrap().health <= 98);
}

#[test]
fn rage_gain_from_damage_matches_cmangos_reward_rage_formula() {
    // CMaNGOS: src/game/Entities/Player.cpp Player::RewardRage
    assert_eq!(rage_gain_from_damage(0, 1, true), 0);
    assert_eq!(rage_gain_from_damage(100, 1, true), 999);
    assert_eq!(rage_gain_from_damage(100, 1, false), 333);
}

#[test]
fn map_runtime_player_regen_tick_restores_health_and_mana_from_spirit() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(1, SessionId(1), position);
    player.class = 8; // Mage
    player.spirit = 40;
    player.max_health = 80;
    player.health = 40;
    player.max_power1 = 100;
    player.power1 = 10;
    map.add_player(player).unwrap();
    let now = Instant::now();

    assert!(map.advance_player_regen_tick(now).unwrap().is_empty());
    let packets = map
        .advance_player_regen_tick(now + Duration::from_secs(2))
        .unwrap();

    assert_eq!(packets.len(), 2);
    let runtime = map.players.get(&1).unwrap();
    assert!(runtime.health > 40);
    assert!(runtime.power1 > 10);
}

#[test]
fn map_runtime_player_regen_tick_degenerates_warrior_rage_out_of_combat() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(1, SessionId(1), position);
    player.class = 1; // Warrior
    player.power2 = 100;
    map.add_player(player).unwrap();
    let now = Instant::now();

    assert!(map.advance_player_regen_tick(now).unwrap().is_empty());
    let packets = map
        .advance_player_regen_tick(now + Duration::from_secs(2))
        .unwrap();

    assert_eq!(packets.len(), 1);
    assert_eq!(map.players.get(&1).unwrap().power2, 75);
}

#[test]
fn map_runtime_player_regen_tick_skips_dead_or_ghost_players() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut dead = test_player_runtime(1, SessionId(1), position);
    dead.health = 0;
    dead.max_power1 = 100;
    dead.power1 = 1;
    dead.power2 = 100;
    map.add_player(dead).unwrap();
    let mut ghost = test_player_runtime(2, SessionId(2), position);
    ghost.flags = PLAYER_FLAGS_GHOST;
    ghost.health = 1;
    ghost.max_power1 = 100;
    ghost.power1 = 1;
    ghost.power2 = 100;
    map.add_player(ghost).unwrap();
    let now = Instant::now();

    assert!(map.advance_player_regen_tick(now).unwrap().is_empty());
    let packets = map
        .advance_player_regen_tick(now + Duration::from_secs(2))
        .unwrap();

    assert!(packets.is_empty());
    assert_eq!(map.players.get(&1).unwrap().power1, 1);
    assert_eq!(map.players.get(&2).unwrap().power2, 100);
}

#[test]
fn player_selection_update_body_sets_unit_target_guid() {
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let selected = ObjectGuid::new(HighGuid::Unit, 0, 99);
    let body = build_player_selection_update_body(7, Some(selected)).unwrap();
    let (values, trailing) = decode_values_update_block(&body[5..], player);
    assert!(trailing.is_empty());
    assert_eq!(values[UNIT_FIELD_TARGET], Some(selected.raw() as u32));
    assert_eq!(
        values[UNIT_FIELD_TARGET + 1],
        Some((selected.raw() >> 32) as u32)
    );
}

#[test]
fn player_selection_update_body_clears_unit_target_guid() {
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let body = build_player_selection_update_body(7, None).unwrap();
    let (values, trailing) = decode_values_update_block(&body[5..], player);
    assert!(trailing.is_empty());
    assert_eq!(values[UNIT_FIELD_TARGET], Some(0));
    assert_eq!(values[UNIT_FIELD_TARGET + 1], Some(0));
}

#[test]
fn map_runtime_player_selection_update_refreshes_shared_state_and_observers() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), player_position))
        .unwrap();
    map.add_player(test_player_runtime(2, SessionId(2), observer_position))
        .unwrap();

    let selected = ObjectGuid::new(HighGuid::Unit, 0, 77);
    let packets = map.update_player_selection(1, Some(selected)).unwrap();

    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0].0, SessionId(2));
    assert_eq!(packets[0].1.opcode, SMSG_UPDATE_OBJECT);
    let (values, trailing) = decode_values_update_block(
        &packets[0].1.body[5..],
        ObjectGuid::new(HighGuid::Player, 0, 1),
    );
    assert!(trailing.is_empty());
    assert_eq!(values[UNIT_FIELD_TARGET], Some(selected.raw() as u32));
    assert_eq!(
        values[UNIT_FIELD_TARGET + 1],
        Some((selected.raw() >> 32) as u32)
    );
    assert_eq!(map.players.get(&1).unwrap().selected_target, Some(selected));
}

#[tokio::test]
async fn shared_creature_combat_start_broadcasts_to_nearby_observer() {
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let victim_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    let victim_session_id = SessionId(1);
    let observer_session_id = SessionId(2);
    let (victim_tx, mut victim_rx) = mpsc::unbounded_channel();
    let (observer_tx, mut observer_rx) = mpsc::unbounded_channel();
    let mut sink = WorldPacketSink::new(victim_tx);
    sessions
        .register(
            observer_session_id,
            SessionHandle {
                account_id: 2,
                character_guid: Some(2),
                outbound: observer_tx,
            },
        )
        .await;
    maps.add_player(test_player_runtime(1, victim_session_id, victim_position))
        .await
        .unwrap();
    maps.add_player(test_player_runtime(
        2,
        observer_session_id,
        observer_position,
    ))
    .await
    .unwrap();

    let mut spawn = test_creature_spawn(6);
    spawn.guid = 177;
    spawn.position_x = victim_position.x + 12.0;
    spawn.position_y = victim_position.y;
    spawn.position_z = victim_position.z;
    let attacker = creature_spawn_guid(&spawn);
    let creature = DbCreatureRuntime::new(spawn);
    maps.share_db_creature_snapshots(0, vec![creature.clone()])
        .await;
    let mut session = WorldSessionState {
        active_character: Some(ActiveCharacter {
            guid: 1,
            name: "Ada".to_string(),
            race: 1,
            class: 1,
            level: 1,
            xp: 0,
            position: victim_position,
            movement_flags: 0,
            client_time: 0,
            fall_time: 0,
        }),
        player_health: 20,
        player_death_state: PlayerDeathState::Alive,
        ..WorldSessionState::default()
    };
    session.db_creatures.insert(attacker.raw(), creature);
    let player = ObjectGuid::new(HighGuid::Player, 0, 1);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);

    send_db_creature_combat_start(
        &mut sink,
        shared_world,
        0,
        &mut session,
        attacker,
        player,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let victim_packets = std::iter::from_fn(|| victim_rx.try_recv().ok()).collect::<Vec<_>>();
    let observer_packets = std::iter::from_fn(|| observer_rx.try_recv().ok()).collect::<Vec<_>>();

    assert!(victim_packets
        .iter()
        .any(|packet| packet.opcode == SMSG_ATTACKSTART));
    assert!(observer_packets
        .iter()
        .any(|packet| packet.opcode == SMSG_ATTACKSTART));
    assert!(observer_packets
        .iter()
        .any(|packet| packet.opcode == SMSG_UPDATE_OBJECT));
    assert!(observer_packets
        .iter()
        .any(|packet| packet.opcode == SMSG_MONSTER_MOVE));
}

#[tokio::test]
async fn shared_chase_motion_advances_map_position_for_other_attackers() {
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let victim_position = WorldPosition::new(0, 10.0, 0.0, 0.0, 0.0);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 188;
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let attacker = creature_spawn_guid(&spawn);
    let creature = DbCreatureRuntime::new(spawn);
    maps.share_db_creature_snapshots(0, vec![creature.clone()])
        .await;
    let mut owner_session = WorldSessionState {
        active_character: Some(ActiveCharacter {
            guid: 1,
            name: "Ada".to_string(),
            race: 1,
            class: 1,
            level: 1,
            xp: 0,
            position: victim_position,
            movement_flags: 0,
            client_time: 0,
            fall_time: 0,
        }),
        player_health: 20,
        player_death_state: PlayerDeathState::Alive,
        ..WorldSessionState::default()
    };
    owner_session.db_creatures.insert(attacker.raw(), creature);
    let now = Instant::now();
    let motion = start_db_creature_chase_motion(
        &mut owner_session,
        attacker,
        ObjectGuid::new(HighGuid::Player, 0, 1),
        now,
    )
    .expect("chase should start");
    maps.update_db_creature_snapshot(
        0,
        owner_session
            .db_creatures
            .get(&attacker.raw())
            .cloned()
            .unwrap(),
    )
    .await;

    let half_duration = Duration::from_millis((motion.duration.as_millis() as u64 / 2).max(1));
    advance_db_creature_motion_and_share(
        shared_world,
        0,
        &mut owner_session,
        attacker,
        now + half_duration,
    )
    .await;

    let shared = maps
        .db_creature_snapshots(0, &[attacker.raw()])
        .await
        .pop()
        .expect("shared creature snapshot");
    let owner = owner_session.db_creatures.get(&attacker.raw()).unwrap();
    assert!(shared.current_position.x > motion.start.x);
    assert_eq!(shared.current_position.x, owner.current_position.x);

    let mut observer_session = WorldSessionState {
        active_character: Some(ActiveCharacter {
            guid: 2,
            name: "Ben".to_string(),
            race: 1,
            class: 1,
            level: 1,
            xp: 0,
            position: shared.current_position,
            movement_flags: 0,
            client_time: 0,
            fall_time: 0,
        }),
        player_health: 20,
        player_death_state: PlayerDeathState::Alive,
        ..WorldSessionState::default()
    };
    observer_session.db_creatures.insert(
        attacker.raw(),
        DbCreatureRuntime::new(test_creature_spawn(6)),
    );
    sync_session_db_creatures_from_map(shared_world, &mut observer_session).await;
    assert_eq!(
        db_creature_player_melee_check(&observer_session, attacker),
        PlayerMeleeCheck::Clear
    );
}

#[tokio::test]
async fn repeated_auto_attack_input_preserves_swing_timer_and_uses_normal_due_tick() {
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let map_id = 0;
    let character_guid = 1;
    let position = WorldPosition::new(map_id, -8950.0, -130.0, 83.5, 0.0);
    maps.add_player(test_player_runtime(character_guid, SessionId(1), position))
        .await
        .unwrap();
    let target = ObjectGuid::new(HighGuid::Unit, 6, 77);
    let now = Instant::now();
    let swing_delay = Duration::from_millis(1200);

    let mut session = WorldSessionState {
        active_character: Some(ActiveCharacter {
            guid: character_guid,
            name: "Ada".to_string(),
            race: 1,
            class: 1,
            level: 1,
            xp: 0,
            position,
            movement_flags: 0,
            client_time: 0,
            fall_time: 0,
        }),
        ..WorldSessionState::default()
    };

    let first_next =
        scheduled_player_auto_attack_next_swing(shared_world, &session, target, now, swing_delay)
            .await;
    assert_eq!(first_next, now, "first swing should be immediately due");
    maps.set_player_auto_attack(map_id, character_guid, Some(target), Some(first_next))
        .await;

    let repeated_next = scheduled_player_auto_attack_next_swing(
        shared_world,
        &session,
        target,
        now + Duration::from_millis(150),
        swing_delay,
    )
    .await;
    assert_eq!(repeated_next, first_next);

    assert_eq!(
        maps.player_auto_attack_due(
            map_id,
            character_guid,
            first_next - Duration::from_millis(1)
        )
        .await,
        None
    );
    assert_eq!(
        maps.player_auto_attack_due(
            map_id,
            character_guid,
            first_next + Duration::from_millis(1)
        )
        .await,
        Some(target)
    );

    let future_next = now + swing_delay;
    maps.set_player_auto_attack(map_id, character_guid, None, Some(future_next))
        .await;
    let restarted_next = scheduled_player_auto_attack_next_swing(
        shared_world,
        &session,
        target,
        now + Duration::from_millis(250),
        swing_delay,
    )
    .await;
    assert_eq!(
        restarted_next, future_next,
        "manual attack stop/start must preserve the existing swing cooldown"
    );
    assert_eq!(
        maps.player_auto_attack_due(map_id, character_guid, future_next)
            .await,
        None,
        "a preserved cooldown without an active target must not swing by itself"
    );

    session.active_character = None;
}

#[test]
fn db_creature_navigation_uses_mmap_tile_availability_when_loaded() {
    let navigation = DbCreatureNavigationGuardrail {
        world_data_files: Arc::new(WorldDataFiles {
            data_dir: std::path::PathBuf::from("fixture"),
            data_dir_for_native: None,
            maps_available: true,
            vmaps_available: true,
            creature_display_scales: HashMap::new(),
            mmap_headers: HashSet::from([0]),
            mmap_tiles: HashSet::from([(0, 48, 32)]),
            vmap_trees: HashSet::new(),
            vmap_tiles: HashSet::new(),
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
fn world_data_parses_cmangos_vmap_file_names() {
    assert_eq!(parse_vmap_tree_file_name("000.vmtree"), Some(0));
    assert_eq!(parse_vmap_tree_file_name("530.vmtree"), Some(530));
    assert_eq!(
        parse_vmap_tile_file_name("000_32_48.vmtile"),
        Some((0, 48, 32))
    );
    assert_eq!(
        parse_vmap_tile_file_name("530_24_31.vmtile"),
        Some((530, 31, 24))
    );
    assert_eq!(parse_vmap_tile_file_name("000_48_32.vmtile.tmp"), None);
}

#[test]
fn db_creature_vmap_los_uses_local_cmangos_data_when_available() {
    let data = Arc::new(WorldDataFiles::inspect("C:/World of Warcraft Classic"));
    if !data.has_vmap_tile(0, 48, 32) {
        return;
    }
    let start = WorldPosition::new(0, -8950.0, -130.0, 85.5, 0.0);
    let target = WorldPosition::new(0, -8940.0, -130.0, 85.5, 0.0);
    let result = native_vmap_line_of_sight(
        data.data_dir_for_native.as_ref().unwrap(),
        start,
        target,
        mmap_tile_for_position(start).unwrap(),
        mmap_tile_for_position(target).unwrap(),
        false,
    );

    assert_eq!(result, Some(true));
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
    let native_points = native_mmap_find_path_points(
        navigation
            .world_data_files
            .data_dir_for_native
            .as_ref()
            .unwrap(),
        start,
        target,
        mmap_tile_for_position(start).unwrap(),
        mmap_tile_for_position(target).unwrap(),
    );

    let corner = db_creature_mmap_next_path_corner(&navigation, start, target).unwrap_or_else(|| {
        panic!(
            "local Northshire mmap should produce a Detour path corner; native_points={}, tiles={:?}->{:?}",
            native_points.as_ref().map_or(0, Vec::len),
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
fn db_creature_mmap_path_uses_cmangos_smooth_steps_when_available() {
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

    let path = db_creature_path_to_destination(&navigation, start, target, CreaturePathMode::Full)
        .expect("local Northshire mmap should produce a smoothed path");

    assert!(path.flags.contains(DbCreaturePathFlags::NORMAL));
    assert!(!path.flags.contains(DbCreaturePathFlags::NOT_USING_PATH));
    assert!(
        path.points.len() > 1,
        "CMaNGOS-style smooth path should expose intermediate 4-yard-ish steps"
    );

    let mut previous = start;
    for point in &path.points {
        let segment = distance_2d(previous.x, previous.y, point.x, point.y);
        assert!(
            segment <= 5.0,
            "smooth path segment should stay near CMaNGOS 4-yard step size, got {segment}"
        );
        previous = *point;
    }
}

#[test]
fn db_creature_path_uses_straight_fallback_only_when_mmap_unavailable() {
    let start = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let target = WorldPosition::new(0, -8940.0, -130.0, 83.5, 0.0);
    let fallback_navigation = DbCreatureNavigationGuardrail::default();

    let fallback_path = db_creature_path_to_destination(
        &fallback_navigation,
        start,
        target,
        CreaturePathMode::Full,
    )
    .expect("missing mmap data should preserve the permissive straight fallback");
    assert_eq!(fallback_path.points.len(), 1);
    assert_eq!(fallback_path.points[0].x, target.x);
    assert!(fallback_path
        .flags
        .contains(DbCreaturePathFlags::NOT_USING_PATH));

    let native_missing_navigation = DbCreatureNavigationGuardrail {
        world_data_files: Arc::new(WorldDataFiles {
            data_dir: std::path::PathBuf::from("Z:/definitely-missing-cmangos-data"),
            data_dir_for_native: std::ffi::CString::new("Z:/definitely-missing-cmangos-data").ok(),
            maps_available: true,
            vmaps_available: false,
            creature_display_scales: HashMap::new(),
            mmap_headers: HashSet::from([0]),
            mmap_tiles: HashSet::from([(0, 48, 32)]),
            vmap_trees: HashSet::new(),
            vmap_tiles: HashSet::new(),
        }),
        ..DbCreatureNavigationGuardrail::default()
    };

    assert!(
        matches!(
            db_creature_mmap_path(
                &native_missing_navigation,
                start,
                target,
                CreaturePathMode::Full,
            ),
            DbCreaturePathBuild::NoPath(flags) if flags.contains(DbCreaturePathFlags::NOPATH)
        ),
        "advertised-but-unloadable mmap data should be reported as a real no-path result"
    );
    assert!(
        db_creature_path_to_destination(
            &native_missing_navigation,
            start,
            target,
            CreaturePathMode::Full,
        )
        .is_none(),
        "when MMAP data is advertised for both tiles, native query failure should not collapse to a through-geometry straight path"
    );
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
fn db_creature_death_uses_db_respawn_and_cmangos_corpse_timers() {
    let mut spawn = test_creature_spawn(6);
    spawn.spawn_time_secs_min = 7;
    spawn.spawn_time_secs_max = 7;
    spawn.template.corpse_decay = 11;
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(spawn);

    runtime.begin_corpse(now, 1_000);

    assert_eq!(runtime.life_state, DbCreatureLifeState::Corpse);
    assert_eq!(runtime.health, 0);
    assert!(runtime.lootable);
    assert_eq!(runtime.respawn_at, Some(now + Duration::from_secs(7)));
    assert_eq!(runtime.respawn_epoch_secs, Some(1_007));
    assert_eq!(
        runtime.corpse_expires_at,
        Some(now + Duration::from_secs(11))
    );
    assert!(!runtime.is_corpse_expired(now + Duration::from_secs(10)));
    assert!(runtime.is_corpse_expired(now + Duration::from_secs(11)));
}

#[test]
fn db_creature_loot_release_does_not_respawn_before_corpse_and_spawn_timers() {
    let mut spawn = test_creature_spawn(6);
    spawn.spawn_time_secs_min = 3;
    spawn.spawn_time_secs_max = 3;
    spawn.template.corpse_decay = 1;
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(spawn);

    runtime.begin_corpse(now, 2_000);
    runtime.loot_money_available = false;
    runtime.loot_items.clear();
    runtime.looting = true;
    runtime.reduce_corpse_decay_after_loot(now);
    assert_eq!(runtime.life_state, DbCreatureLifeState::Corpse);
    assert_eq!(runtime.health, 0);
    assert!(!runtime.lootable);
    assert!(!runtime.is_ready_to_respawn(now + Duration::from_secs(2)));

    runtime.remove_corpse();
    assert_eq!(runtime.life_state, DbCreatureLifeState::Dead);
    assert_eq!(runtime.current_position.x, runtime.home_position.x);
    assert!(!runtime.is_ready_to_respawn(now + Duration::from_secs(2)));
    assert!(runtime.is_ready_to_respawn(now + Duration::from_secs(3)));
    runtime.respawn();

    assert_eq!(runtime.life_state, DbCreatureLifeState::Alive);
    assert_eq!(runtime.health, runtime.max_health());
    assert!(!runtime.lootable);
    assert_eq!(runtime.respawn_at, None);
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

#[tokio::test]
async fn db_creature_return_home_motion_advances_without_active_combat() {
    let mut spawn = test_creature_spawn(6);
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let attacker = creature_spawn_guid(&spawn);
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.current_position.x = 7.0;
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    maps.share_db_creature_snapshots(0, vec![runtime.clone()])
        .await;
    let mut session = WorldSessionState {
        active_character: Some(ActiveCharacter {
            guid: 7,
            name: "Ada".to_string(),
            race: 1,
            class: 1,
            level: 1,
            xp: 0,
            position: WorldPosition::new(0, 0.0, 0.0, 83.5, 0.0),
            movement_flags: 0,
            client_time: 0,
            fall_time: 0,
        }),
        ..WorldSessionState::default()
    };
    session.db_creatures.insert(attacker.raw(), runtime);

    let (creature, motion) = maps
        .start_db_creature_return_home_motion(0, &session.db_creature_navigation, attacker, now)
        .await
        .expect("away creature should start return-home motion");
    session.db_creatures.insert(attacker.raw(), creature);
    assert!(session.active_creature_combats.is_empty());

    advance_db_creature_return_home_motions(shared_world, &mut session, now + motion.duration)
        .await;
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
fn db_creature_random_motion_duration_uses_template_walk_speed() {
    let mut spawn = test_creature_spawn(6);
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    spawn.movement_type = DB_MOTION_TYPE_RANDOM;
    spawn.spawn_dist = 5.0;
    spawn.template.speed_walk = 2.0;
    let creature_guid = creature_spawn_guid(&spawn);
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.next_random_move_at = Some(now);
    let mut session = WorldSessionState::default();
    session.db_creatures.insert(creature_guid.raw(), runtime);

    let motion = start_db_creature_random_motion(&mut session, creature_guid, now)
        .expect("random movement creature should start a wander spline");
    let distance = path_distance_2d(motion.start, &motion.path);
    let expected_millis = ((distance / (DB_CREATURE_WALK_SPEED_YARDS_PER_SEC * 2.0)) * 1000.0)
        .ceil()
        .max(1.0) as u64;
    assert_eq!(motion.duration, Duration::from_millis(expected_millis));
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
fn db_creature_idle_motion_start_guids_are_paced_per_tick() {
    let now = Instant::now();
    let mut session = WorldSessionState::default();
    for index in 0..(DB_CREATURE_IDLE_MOTION_STARTS_PER_TICK + 3) {
        let mut spawn = test_creature_spawn(6);
        spawn.guid = 1_000 + index as u32;
        spawn.movement_type = DB_MOTION_TYPE_RANDOM;
        spawn.spawn_dist = 5.0;
        let guid = creature_spawn_guid(&spawn);
        let mut runtime = DbCreatureRuntime::new(spawn);
        runtime.next_random_move_at = Some(now);
        session.db_creatures.insert(guid.raw(), runtime);
    }

    let start_guids = db_creature_idle_motion_start_guids(&session, now);

    assert_eq!(start_guids.len(), DB_CREATURE_IDLE_MOTION_STARTS_PER_TICK);
    assert!(start_guids.windows(2).all(|window| window[0] < window[1]));
}

#[tokio::test]
async fn shared_db_creature_idle_motion_updates_map_and_observers() {
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let player_position = WorldPosition::new(0, 0.0, 0.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, 1.0, 0.0, 83.5, 0.0);
    maps.add_player(test_player_runtime(1, SessionId(1), player_position))
        .await
        .unwrap();
    maps.add_player(test_player_runtime(2, SessionId(2), observer_position))
        .await
        .unwrap();
    let (direct_tx, mut direct_rx) = mpsc::unbounded_channel();
    let (observer_tx, mut observer_rx) = mpsc::unbounded_channel();
    sessions
        .register(
            SessionId(1),
            SessionHandle {
                account_id: 1,
                character_guid: Some(1),
                outbound: direct_tx,
            },
        )
        .await;
    sessions
        .register(
            SessionId(2),
            SessionHandle {
                account_id: 2,
                character_guid: Some(2),
                outbound: observer_tx,
            },
        )
        .await;

    let mut spawn = test_creature_spawn(6);
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 83.5;
    spawn.movement_type = DB_MOTION_TYPE_WAYPOINT;
    spawn.waypoint_path = vec![test_waypoint(1, 5.0, 0.0, 0)];
    let creature_guid = creature_spawn_guid(&spawn);
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.next_waypoint_move_at = Some(now);
    maps.share_db_creature_snapshots(0, vec![runtime.clone()])
        .await;
    maps.update_player_db_creature_visibility(0, 1, &[creature_guid], &[])
        .await;
    maps.update_player_db_creature_visibility(0, 2, &[creature_guid], &[])
        .await;

    let tick = maps
        .advance_all_active_db_creature_idle_motions(&DbCreatureNavigationGuardrail::default(), now)
        .await
        .unwrap();
    sessions.dispatch(tick.packets).await;

    assert_eq!(direct_rx.try_recv().unwrap().opcode, SMSG_MONSTER_MOVE);
    assert_eq!(observer_rx.try_recv().unwrap().opcode, SMSG_MONSTER_MOVE);
    let snapshot = maps
        .db_creature_snapshots(0, &[creature_guid.raw()])
        .await
        .pop()
        .unwrap();
    assert!(matches!(snapshot.motion, CreatureMotionState::Waypoint(_)));
}

#[tokio::test]
async fn shared_db_creature_idle_motion_recreates_local_visibility_before_move() {
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let player_position = WorldPosition::new(0, 0.0, 0.0, 83.5, 0.0);
    maps.add_player(test_player_runtime(1, SessionId(1), player_position))
        .await
        .unwrap();

    let (direct_tx, mut direct_rx) = mpsc::unbounded_channel();
    sessions
        .register(
            SessionId(1),
            SessionHandle {
                account_id: 1,
                character_guid: Some(1),
                outbound: direct_tx,
            },
        )
        .await;

    let mut spawn = test_creature_spawn(6);
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 83.5;
    spawn.movement_type = DB_MOTION_TYPE_WAYPOINT;
    spawn.waypoint_path = vec![test_waypoint(1, 5.0, 0.0, 0)];
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.next_waypoint_move_at = Some(now);
    maps.share_db_creature_snapshots(0, vec![runtime.clone()])
        .await;

    let tick = maps
        .advance_all_active_db_creature_idle_motions(&DbCreatureNavigationGuardrail::default(), now)
        .await
        .unwrap();
    sessions.dispatch(tick.packets).await;

    assert_eq!(direct_rx.try_recv().unwrap().opcode, SMSG_UPDATE_OBJECT);
    assert_eq!(direct_rx.try_recv().unwrap().opcode, SMSG_MONSTER_MOVE);
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
fn db_creature_waypoint_motion_buffers_short_zero_wait_paths() {
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
    assert_eq!(first.path.len(), 3);
    assert_eq!(first.path.last().unwrap().x, 15.0);
    advance_db_creature_motion(&mut session, creature_guid, now + first.duration);
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
    caller_spawn.template.faction = 17;
    caller_spawn.template.call_for_help = 6;
    let caller = creature_spawn_guid(&caller_spawn);
    let mut helper_spawn = test_creature_spawn(6);
    helper_spawn.guid = 45;
    helper_spawn.position_x = 5.0;
    helper_spawn.position_y = 0.0;
    helper_spawn.template.npc_flags = 0;
    helper_spawn.template.faction = 17;
    let helper = creature_spawn_guid(&helper_spawn);
    let mut far_spawn = test_creature_spawn(6);
    far_spawn.guid = 46;
    far_spawn.position_x = 9.0;
    far_spawn.position_y = 0.0;
    far_spawn.template.npc_flags = 0;
    far_spawn.template.faction = 17;
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
        10.0 - ATTACK_DISTANCE_YARDS * DB_CREATURE_CHASE_DEFAULT_RANGE_FACTOR
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
fn db_creature_chase_motion_stop_distance_uses_combined_reach() {
    let mut creature = test_creature_spawn(6);
    creature.position_x = 0.0;
    creature.position_y = 0.0;
    creature.position_z = 0.0;
    creature.template.model_combat_reach = 4.0;
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
            position: WorldPosition::new(0, 20.0, 0.0, 0.0, 0.0),
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
        .expect("large-reach creature should start chase motion");
    let expected_stop_distance = combined_melee_reach(4.0, PLAYER_COMBAT_REACH_YARDS)
        * DB_CREATURE_CHASE_DEFAULT_RANGE_FACTOR;
    let destination = motion.path.last().unwrap();

    assert!((destination.x - (20.0 - expected_stop_distance)).abs() < 0.001);
}

#[test]
fn db_creature_chase_cut_path_uses_first_los_valid_reach_point() {
    let start = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    let target = WorldPosition::new(0, 10.0, 0.0, 0.0, 0.0);
    let path = vec![
        WorldPosition::new(0, 4.0, 0.0, 0.0, 0.0),
        WorldPosition::new(0, 8.0, 0.0, 0.0, 0.0),
        WorldPosition::new(0, 10.0, 0.0, 0.0, 0.0),
    ];

    let cut = db_creature_cut_chase_path(
        &DbCreatureNavigationGuardrail::default(),
        start,
        path,
        target,
        2.5,
    )
    .expect("path should cut at first reachable point");

    assert_eq!(cut.len(), 2);
    assert_eq!(cut.last().unwrap().x, 8.0);
}

#[test]
fn db_creature_chase_path_skips_los_backed_straight_fast_path() {
    let start = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    let target = WorldPosition::new(0, 10.0, 0.0, 0.0, 0.0);
    let navigation = DbCreatureNavigationGuardrail {
        world_data_files: Arc::new(WorldDataFiles {
            data_dir: std::path::PathBuf::from("fixture"),
            data_dir_for_native: None,
            maps_available: true,
            vmaps_available: true,
            creature_display_scales: HashMap::new(),
            mmap_headers: HashSet::new(),
            mmap_tiles: HashSet::new(),
            vmap_trees: HashSet::from([0]),
            vmap_tiles: HashSet::from([(0, 32, 32), (0, 31, 32)]),
        }),
        ..DbCreatureNavigationGuardrail::default()
    };

    let path = db_creature_chase_path(&navigation, start, target, 2.5)
        .expect("missing mmap data should preserve the permissive straight fallback");

    assert!(path.flags.contains(DbCreaturePathFlags::NORMAL));
    assert!(path.flags.contains(DbCreaturePathFlags::NOT_USING_PATH));
    assert_eq!(path.points.len(), 1);
    assert!((path.points[0].x - 7.5).abs() < 0.001);
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

fn quest_template_with_required_item(
    quest_id: u32,
    item_id: u32,
    item_count: u32,
) -> QuestTemplateQuery {
    let mut quest = test_quest_template(quest_id);
    quest.req_item_id[0] = item_id;
    quest.req_item_count[0] = item_count;
    quest
}

#[test]
fn quest_loot_selection_prefers_active_required_quest_item() {
    let loot_rows = vec![
        CreatureLootQuery {
            item: 159,
            group_id: 0,
            min_count: 1,
            max_count: 1,
            display_id: 1,
            chance_or_quest_chance: 100.0,
        },
        CreatureLootQuery {
            item: 777,
            group_id: 0,
            min_count: 1,
            max_count: 1,
            display_id: 2,
            chance_or_quest_chance: -100.0,
        },
    ];
    let mut active_quests = HashMap::new();
    active_quests.insert(31, quest_template_with_required_item(31, 777, 1));
    let mut quest_statuses = HashMap::new();
    quest_statuses.insert(
        31,
        CharacterQuestStatus {
            quest: 31,
            status: QUEST_STATUS_INCOMPLETE,
            rewarded: 0,
            mobcount1: 0,
            mobcount2: 0,
            mobcount3: 0,
            mobcount4: 0,
        },
    );

    let selected = select_creature_loot_for_active_quests_with_rolls(
        &loot_rows,
        &active_quests,
        &quest_statuses,
        &[],
        || 0.0,
        |min_count, _max_count| min_count,
    );
    assert!(selected.iter().any(|loot| loot.item == 777));
}

#[test]
fn quest_loot_selection_skips_fulfilled_quest_item_requirement() {
    let loot_rows = vec![
        CreatureLootQuery {
            item: 159,
            group_id: 0,
            min_count: 1,
            max_count: 1,
            display_id: 1,
            chance_or_quest_chance: 100.0,
        },
        CreatureLootQuery {
            item: 777,
            group_id: 0,
            min_count: 1,
            max_count: 1,
            display_id: 2,
            chance_or_quest_chance: -100.0,
        },
    ];
    let mut active_quests = HashMap::new();
    active_quests.insert(31, quest_template_with_required_item(31, 777, 1));
    let mut quest_statuses = HashMap::new();
    quest_statuses.insert(
        31,
        CharacterQuestStatus {
            quest: 31,
            status: QUEST_STATUS_INCOMPLETE,
            rewarded: 0,
            mobcount1: 0,
            mobcount2: 0,
            mobcount3: 0,
            mobcount4: 0,
        },
    );
    let inventory = vec![CharacterInventoryItem {
        bag: 0,
        slot: 23,
        item: 901,
        item_template: 777,
        count: 1,
        durability: 0,
    }];

    let selected = select_creature_loot_for_active_quests_with_rolls(
        &loot_rows,
        &active_quests,
        &quest_statuses,
        &inventory,
        || 0.0,
        |min_count, _max_count| min_count,
    );
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].item, 159);
}

#[test]
fn creature_loot_roll_respects_chance_thresholds() {
    let loot_rows = vec![
        CreatureLootQuery {
            item: 159,
            group_id: 0,
            min_count: 1,
            max_count: 1,
            display_id: 1,
            chance_or_quest_chance: 49.9,
        },
        CreatureLootQuery {
            item: 160,
            group_id: 0,
            min_count: 1,
            max_count: 1,
            display_id: 2,
            chance_or_quest_chance: 50.0,
        },
    ];
    let selected = select_creature_loot_for_active_quests_with_rolls(
        &loot_rows,
        &HashMap::new(),
        &HashMap::new(),
        &[],
        || 49.95,
        |min_count, _max_count| min_count,
    );
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].item, 160);
}

#[test]
fn creature_loot_roll_can_return_multiple_independent_rows() {
    let loot_rows = vec![
        CreatureLootQuery {
            item: 159,
            group_id: 0,
            min_count: 1,
            max_count: 1,
            display_id: 1,
            chance_or_quest_chance: 100.0,
        },
        CreatureLootQuery {
            item: 160,
            group_id: 0,
            min_count: 1,
            max_count: 1,
            display_id: 2,
            chance_or_quest_chance: 100.0,
        },
    ];

    let selected = select_creature_loot_for_active_quests_with_rolls(
        &loot_rows,
        &HashMap::new(),
        &HashMap::new(),
        &[],
        || 0.0,
        |min_count, _max_count| min_count,
    );

    let items = selected.iter().map(|loot| loot.item).collect::<Vec<_>>();
    assert_eq!(items, vec![159, 160]);
}

#[test]
fn creature_loot_roll_picks_one_row_per_group() {
    let loot_rows = vec![
        CreatureLootQuery {
            item: 159,
            group_id: 1,
            min_count: 1,
            max_count: 1,
            display_id: 1,
            chance_or_quest_chance: 20.0,
        },
        CreatureLootQuery {
            item: 160,
            group_id: 1,
            min_count: 1,
            max_count: 1,
            display_id: 2,
            chance_or_quest_chance: 80.0,
        },
        CreatureLootQuery {
            item: 161,
            group_id: 0,
            min_count: 1,
            max_count: 1,
            display_id: 3,
            chance_or_quest_chance: 100.0,
        },
    ];

    let selected = select_creature_loot_for_active_quests_with_rolls(
        &loot_rows,
        &HashMap::new(),
        &HashMap::new(),
        &[],
        || 25.0,
        |min_count, _max_count| min_count,
    );

    let items = selected.iter().map(|loot| loot.item).collect::<Vec<_>>();
    assert_eq!(items, vec![161, 160]);
}

#[test]
fn creature_loot_roll_uses_randomized_count_range() {
    let loot_rows = vec![CreatureLootQuery {
        item: 118,
        group_id: 0,
        min_count: 2,
        max_count: 5,
        display_id: 9,
        chance_or_quest_chance: 100.0,
    }];
    let selected = select_creature_loot_for_active_quests_with_rolls(
        &loot_rows,
        &HashMap::new(),
        &HashMap::new(),
        &[],
        || 0.0,
        |_min_count, _max_count| 4,
    );
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].min_count, 4);
    assert_eq!(selected[0].max_count, 4);
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

    let visual = build_play_spell_visual_body(guid, 0xB3);
    assert_eq!(visual.len(), 12);
    assert_eq!(&visual[0..8], &guid.raw().to_le_bytes());
    assert_eq!(&visual[8..12], &0xB3u32.to_le_bytes());

    let impact = build_play_spell_impact_body(123, 0x016A);
    assert_eq!(impact.len(), 12);
    assert_eq!(
        &impact[0..8],
        &ObjectGuid::new(HighGuid::Player, REALM_ID, 123)
            .raw()
            .to_le_bytes()
    );
    assert_eq!(&impact[8..12], &0x016Au32.to_le_bytes());
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
            spell_id: WARRIOR_HEROIC_STRIKE_RANK_1,
            kind: StarterSpellKind::NextMeleeSwing,
            bonus_damage: HEROIC_STRIKE_FIXTURE_DAMAGE,
            damage: 0,
            power: StarterSpellPower::Rage {
                cost: HEROIC_STRIKE_RAGE_COST
            },
            requires_melee: true,
            triggers_global_cooldown: false,
            cooldown_millis: 0,
        })
    );
    assert_eq!(
        supported_starter_spell(HUNTER_RAPTOR_STRIKE_RANK_1),
        Some(SupportedStarterSpell {
            spell_id: HUNTER_RAPTOR_STRIKE_RANK_1,
            kind: StarterSpellKind::NextMeleeSwing,
            bonus_damage: RAPTOR_STRIKE_FIXTURE_DAMAGE,
            damage: RAPTOR_STRIKE_FIXTURE_DAMAGE,
            power: StarterSpellPower::Mana {
                cost: RAPTOR_STRIKE_MANA_COST
            },
            requires_melee: true,
            triggers_global_cooldown: false,
            cooldown_millis: 0,
        })
    );
    assert_eq!(supported_starter_spell(1), None);
}

#[tokio::test]
async fn heroic_strike_queue_consumes_on_next_swing_only_once() {
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
    let character = ActiveCharacter {
        guid: 7,
        name: "Ada".to_string(),
        race: 1,
        class: 1,
        level: 1,
        xp: 0,
        position: WorldPosition::new(0, -8949.95, -132.493, 83.5312, 0.0),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
    };
    let initial_dummy_health = RUST_COMBAT_DUMMY_HEALTH + RUST_COMBAT_DUMMY_HIT_DAMAGE;
    let mut session = WorldSessionState {
        active_character: Some(character),
        combat_dummy_health: initial_dummy_health,
        queued_next_melee_spell: Some(QueuedNextMeleeSpell {
            spell_id: WARRIOR_HEROIC_STRIKE_RANK_1,
            target: rust_combat_dummy_guid(),
            bonus_damage: HEROIC_STRIKE_FIXTURE_DAMAGE,
            rage_cost: HEROIC_STRIKE_RAGE_COST,
            mana_cost: 0,
        }),
        player_rage: HEROIC_STRIKE_RAGE_COST,
        ..WorldSessionState::default()
    };

    send_combat_dummy_swing(&mut stream, shared_world, &mut session, &mut header_crypto)
        .await
        .unwrap();
    let first_damage = initial_dummy_health - session.combat_dummy_health;
    assert_eq!(
        first_damage,
        RUST_COMBAT_DUMMY_HIT_DAMAGE + HEROIC_STRIKE_FIXTURE_DAMAGE
    );
    assert!(session.queued_next_melee_spell.is_none());
    assert_eq!(
        session.player_rage, 0,
        "Heroic Strike replaces the white swing and must not award attack rage"
    );

    send_combat_dummy_swing(&mut stream, shared_world, &mut session, &mut header_crypto)
        .await
        .unwrap();
    let total_damage = initial_dummy_health - session.combat_dummy_health;
    assert_eq!(
        total_damage,
        RUST_COMBAT_DUMMY_HIT_DAMAGE * 2 + HEROIC_STRIKE_FIXTURE_DAMAGE
    );
    assert!(
        session.player_rage > 0,
        "the following normal white swing should still award rage"
    );

    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert_eq!(
        packets
            .iter()
            .filter(|packet| packet.opcode == SMSG_SPELL_GO)
            .count(),
        1
    );
    assert_eq!(
        packets
            .iter()
            .filter(|packet| packet.opcode == SMSG_SPELLNONMELEEDAMAGELOG)
            .count(),
        1
    );
    let attacker_packets = packets
        .iter()
        .filter(|packet| packet.opcode == SMSG_ATTACKERSTATEUPDATE)
        .collect::<Vec<_>>();
    assert_eq!(attacker_packets.len(), 1);
    assert!(!attacker_packets[0]
        .body
        .windows(4)
        .any(|window| { window == WARRIOR_HEROIC_STRIKE_RANK_1.to_le_bytes().as_slice() }));
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
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let target = rust_combat_dummy_guid();
    let mut body = Vec::new();
    body.extend_from_slice(&WARRIOR_HEROIC_STRIKE_RANK_1.to_le_bytes());
    body.extend_from_slice(&SPELL_CAST_TARGET_UNIT.to_le_bytes());
    PackedGuid::write(&mut body, target).unwrap();
    let mut active_spells = HashSet::new();
    active_spells.insert(WARRIOR_HEROIC_STRIKE_RANK_1);
    let mut session = WorldSessionState {
        active_character: Some(ActiveCharacter {
            guid: 7,
            name: "Ada".to_string(),
            race: 1,
            class: 1,
            level: 1,
            xp: 0,
            position: WorldPosition::new(0, -8949.95, -132.493, 83.5312, 0.0),
            movement_flags: 0,
            client_time: 0,
            fall_time: 0,
        }),
        player_rage: POWER_RAGE_DEFAULT,
        active_spells,
        ..WorldSessionState::default()
    };

    handle_cast_spell(
        &mut stream,
        &character_db_pool,
        &world_db_pool,
        shared_world,
        &body,
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    assert_eq!(
        session.queued_next_melee_spell,
        Some(QueuedNextMeleeSpell {
            spell_id: WARRIOR_HEROIC_STRIKE_RANK_1,
            target,
            bonus_damage: HEROIC_STRIKE_FIXTURE_DAMAGE,
            rage_cost: HEROIC_STRIKE_RAGE_COST,
            mana_cost: 0,
        })
    );
    assert_eq!(
        session.player_rage, POWER_RAGE_DEFAULT,
        "next-melee rage is spent when the queued swing fires, not when it is queued"
    );
    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == SMSG_CAST_RESULT));
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == SMSG_SPELL_START));
    assert!(!packets.iter().any(|packet| packet.opcode == SMSG_SPELL_GO));
    assert!(!packets
        .iter()
        .any(|packet| packet.opcode == SMSG_UPDATE_OBJECT));
}

#[tokio::test]
async fn starter_spell_cast_failure_rejects_missing_power_gcd_and_duplicate_queue() {
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let target = rust_combat_dummy_guid();
    let targets = SpellCastTargets {
        target_mask: SPELL_CAST_TARGET_UNIT,
        unit_target: Some(target),
        gameobject_target: None,
    };

    let mut session = WorldSessionState::default();
    let starter = supported_starter_spell(WARRIOR_HEROIC_STRIKE_RANK_1).unwrap();
    assert_eq!(
        starter_spell_cast_failure(
            shared_world,
            &mut session,
            &starter,
            &targets,
            Instant::now()
        )
        .await,
        Some(SPELL_FAILED_NO_POWER)
    );

    session.player_rage = HEROIC_STRIKE_RAGE_COST;
    session.starter_global_cooldown_until = Some(Instant::now() + Duration::from_millis(100));
    let gcd_starter = SupportedStarterSpell {
        spell_id: 999_001,
        kind: StarterSpellKind::InstantDamage,
        bonus_damage: 0,
        damage: 1,
        power: StarterSpellPower::Rage { cost: 0 },
        requires_melee: false,
        triggers_global_cooldown: true,
        cooldown_millis: 0,
    };
    assert_eq!(
        starter_spell_cast_failure(
            shared_world,
            &mut session,
            &gcd_starter,
            &targets,
            Instant::now()
        )
        .await,
        Some(SPELL_FAILED_NOT_READY)
    );

    session.starter_global_cooldown_until = None;
    session.queued_next_melee_spell = Some(QueuedNextMeleeSpell {
        spell_id: WARRIOR_HEROIC_STRIKE_RANK_1,
        target,
        bonus_damage: HEROIC_STRIKE_FIXTURE_DAMAGE,
        rage_cost: HEROIC_STRIKE_RAGE_COST,
        mana_cost: 0,
    });
    assert_eq!(
        starter_spell_cast_failure(
            shared_world,
            &mut session,
            &starter,
            &targets,
            Instant::now()
        )
        .await,
        Some(SPELL_FAILED_NOT_READY)
    );
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
fn set_action_button_reads_cmangos_packed_layout() {
    let request = SetActionButtonRequest::read(&[11, 0x75, 0x00, 0x00, 0x80]).unwrap();

    assert_eq!(request.button, 11);
    assert_eq!(request.action(), 117);
    assert_eq!(request.action_type(), ACTION_BUTTON_TYPE_ITEM);
    assert!(!request.removes_binding());
}

#[test]
fn set_action_button_reads_remove_binding_packet() {
    let request = SetActionButtonRequest::read(&[3, 0, 0, 0, 0]).unwrap();

    assert_eq!(request.button, 3);
    assert!(request.removes_binding());
    assert_eq!(request.action(), 0);
    assert_eq!(request.action_type(), 0);
}

#[test]
fn set_action_button_rejects_truncated_payload() {
    let err = SetActionButtonRequest::read(&[3, 0, 0, 0]).unwrap_err();
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

    assert!(character_can_equip_item_template(
        1,
        1,
        &cloth,
        &warrior_skills
    ));
    assert!(character_can_equip_item_template(
        1,
        1,
        &leather,
        &warrior_skills
    ));
    assert!(character_can_equip_item_template(
        1,
        1,
        &mail,
        &warrior_skills
    ));
    assert!(!character_can_equip_item_template(
        1,
        8,
        &leather,
        &cloth_only_skills
    ));
}

#[test]
fn inventory_equip_validation_rejects_item_with_wrong_allowable_class() {
    let mut cloth = test_item_template(1003, ITEM_CLASS_ARMOR, 4, 0.0, 0.0, 1);
    cloth.subclass = 1;
    cloth.allowable_class = 1 << (8 - 1);
    let skills = vec![test_skill(415, 1, 1)];

    assert!(!character_can_equip_item_template(1, 1, &cloth, &skills));
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
fn active_mover_accepts_matching_player_guid() {
    let guid = 77u32;
    let session = WorldSessionState {
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
        }),
        ..WorldSessionState::default()
    };
    let mover_guid = ObjectGuid::new(HighGuid::Player, 0, guid)
        .raw()
        .to_le_bytes();

    let result = handle_set_active_mover(&mover_guid, &session);

    assert!(result.is_ok());
}

#[test]
fn active_mover_mismatch_is_non_fatal() {
    let session = WorldSessionState {
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
        }),
        ..WorldSessionState::default()
    };
    let mismatched_mover_guid = ObjectGuid::new(HighGuid::Player, 0, 99).raw().to_le_bytes();

    let result = handle_set_active_mover(&mismatched_mover_guid, &session);

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
        b"General - Elwynn Forest\0hunter2\0",
        &mut header_crypto,
    )
    .await
    .unwrap();

    let packet = outbound_rx.try_recv().unwrap();
    assert_eq!(packet.opcode, SMSG_CHANNEL_NOTIFY);
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

    handle_join_channel(&mut sink, b"\0hunter2\0", &mut header_crypto)
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
    let target = rust_combat_dummy_guid();
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
fn starter_spell_packets_match_cmangos_success_shapes() {
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    let target = rust_combat_dummy_guid();
    let targets = SpellCastTargets {
        target_mask: SPELL_CAST_TARGET_UNIT,
        unit_target: Some(target),
        gameobject_target: None,
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
fn opening_spell_packets_include_gameobject_target_mask() {
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    let gameobject = ObjectGuid::new(HighGuid::GameObject, 0, 12345);
    let targets = SpellCastTargets {
        target_mask: SPELL_CAST_TARGET_LOCKED | SPELL_CAST_TARGET_GAMEOBJECT,
        unit_target: None,
        gameobject_target: Some(gameobject),
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
fn raptor_strike_starter_spell_packets_match_success_shapes() {
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    let target = rust_combat_dummy_guid();
    let targets = normalize_fixture_spell_targets(SpellCastTargets {
        target_mask: SPELL_CAST_TARGET_UNIT_ENEMY,
        unit_target: Some(target),
        gameobject_target: None,
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
        CMSG_CANCEL_TRADE,
        CMSG_ZONEUPDATE,
        CMSG_MEETINGSTONE_INFO,
        CMSG_REQUEST_RAID_INFO,
        CMSG_MOVE_TIME_SKIPPED,
        CMSG_BATTLEFIELD_STATUS,
    ] {
        assert!(is_expected_noop_opcode(opcode), "opcode 0x{opcode:04X}");
    }

    for opcode in [
        CMSG_TUTORIAL_FLAG,
        CMSG_TUTORIAL_CLEAR,
        CMSG_TUTORIAL_RESET,
        CMSG_JOIN_CHANNEL,
        CMSG_SET_SELECTION,
    ] {
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
