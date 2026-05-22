#[test]
fn server_packet_header_matches_world_shape() {
    let mut packet = Vec::new();
    packet.extend_from_slice(&(4u16 + 2).to_be_bytes());
    packet.extend_from_slice(&(WorldOpcode::SmsgAuthChallenge as u16).to_le_bytes());
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
    packet.extend_from_slice(&(WorldOpcode::SmsgCharEnum as u16).to_le_bytes());
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
    let template = test_creature_template(42);
    let body = build_creature_query_response(42, Some(&template));
    let mut cursor = 0;

    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 42);
    assert_eq!(read_c_string(&body, &mut cursor).unwrap(), "Creature 42");
    assert_eq!(body[cursor], 0);
    assert_eq!(body[cursor + 1], 0);
    assert_eq!(body[cursor + 2], 0);
    cursor += 3;
    assert_eq!(read_c_string(&body, &mut cursor).unwrap(), "DB Spawn");
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 7);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 1);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 123);
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

    let parsed = wow_proto::CreatureQueryRequest::read(&mut &query[..]).unwrap();
    let response = build_creature_query_response(parsed.entry, None);

    assert_eq!(parsed.entry, 98_765);
    assert_eq!(ObjectGuid::from_raw(parsed.raw_guid), guid);
    assert_eq!(response, (98_765u32 | 0x8000_0000).to_le_bytes());
}

#[test]
fn db_creature_query_response_uses_world_template_fields() {
    let mut template = test_creature_template(42);
    template.creature_type_flags = CREATURE_TYPE_FLAG_VISIBLE_TO_GHOSTS;
    let body = build_creature_query_response(42, Some(&template));
    let mut cursor = 0;

    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 42);
    assert_eq!(read_c_string(&body, &mut cursor).unwrap(), "Creature 42");
    cursor += 3;
    assert_eq!(read_c_string(&body, &mut cursor).unwrap(), "DB Spawn");
    assert_eq!(
        read_u32(&body, &mut cursor).unwrap(),
        CREATURE_TYPE_FLAG_VISIBLE_TO_GHOSTS
    );
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 7);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 1);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 123);
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
fn db_gossip_message_can_merge_service_and_quest_options() {
    let guid = ObjectGuid::new(HighGuid::Unit, 42, 96_002);
    let quest = QuestListItem {
        quest: test_quest_template(123),
        dialog_status: DIALOG_STATUS_AVAILABLE,
    };
    let body = build_gossip_message_with_quests(
        guid,
        DB_TRAINER_GOSSIP_TEXT_ID,
        &[(0, GOSSIP_ICON_TRAINER, DB_TRAINER_GOSSIP_OPTION)],
        &[quest],
    );
    let quest_count_offset = 39;
    assert_eq!(
        &body[quest_count_offset..quest_count_offset + 4],
        &1u32.to_le_bytes()
    );
    assert_eq!(
        &body[quest_count_offset + 4..quest_count_offset + 8],
        &123u32.to_le_bytes()
    );
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

fn test_gossip_menu_option(
    option_id: u32,
    npc_option_npcflag: u32,
) -> wow_db::GossipMenuOptionQuery {
    wow_db::GossipMenuOptionQuery {
        menu_id: 7,
        id: 0,
        option_icon: GOSSIP_ICON_CHAT,
        option_text: Some("DB option".to_string()),
        option_id,
        npc_option_npcflag,
        action_menu_id: 0,
        action_poi_id: 0,
        action_script_id: 0,
        box_coded: 0,
        box_text: None,
        condition_id: 0,
    }
}

#[tokio::test]
async fn db_gossip_option_visibility_requires_matching_npc_flag() {
    let pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let object_mgr = ObjectMgr::default();
    let row = test_gossip_menu_option(GOSSIP_OPTION_VENDOR, UNIT_NPC_FLAG_VENDOR);
    let session = WorldSessionState::default();
    let service_state = GossipServiceState {
        npc_flags: UNIT_NPC_FLAG_GOSSIP,
        has_vendor_items: true,
        has_trainer_spells: false,
        is_spirit_healer: false,
        is_dead: false,
    };

    assert_eq!(
        gossip_option_visibility(&object_mgr, &pool, &row, &service_state, &session)
            .await
            .unwrap(),
        GossipOptionVisibility::Hide
    );

    let service_state = GossipServiceState {
        npc_flags: UNIT_NPC_FLAG_GOSSIP | UNIT_NPC_FLAG_VENDOR,
        ..service_state
    };
    assert_eq!(
        gossip_option_visibility(&object_mgr, &pool, &row, &service_state, &session)
            .await
            .unwrap(),
        GossipOptionVisibility::Show
    );
}

#[tokio::test]
async fn db_gossip_option_visibility_requires_service_backing() {
    let pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let object_mgr = ObjectMgr::default();
    let session = WorldSessionState::default();
    let service_state = GossipServiceState {
        npc_flags: UNIT_NPC_FLAG_VENDOR | UNIT_NPC_FLAG_TRAINER,
        has_vendor_items: false,
        has_trainer_spells: false,
        is_spirit_healer: false,
        is_dead: false,
    };

    assert_eq!(
        gossip_option_visibility(
            &object_mgr,
            &pool,
            &test_gossip_menu_option(GOSSIP_OPTION_VENDOR, UNIT_NPC_FLAG_VENDOR),
            &service_state,
            &session,
        )
        .await
        .unwrap(),
        GossipOptionVisibility::Hide
    );
    assert_eq!(
        gossip_option_visibility(
            &object_mgr,
            &pool,
            &test_gossip_menu_option(GOSSIP_OPTION_TRAINER, UNIT_NPC_FLAG_TRAINER),
            &service_state,
            &session,
        )
        .await
        .unwrap(),
        GossipOptionVisibility::Hide
    );
}

#[test]
fn db_gossip_message_preserves_prepared_option_actions_shape() {
    let guid = ObjectGuid::new(HighGuid::Unit, 42, 96_003);
    let body = build_gossip_message_from_options_with_quests(
        guid,
        1234,
        &[GossipMessageOption {
            option_index: 0,
            icon: GOSSIP_ICON_TAXI,
            coded: 1,
            text: "Show me where to fly.".to_string(),
        }],
        &[],
    );

    assert_eq!(&body[0..8], &guid.raw().to_le_bytes());
    assert_eq!(&body[8..12], &1234u32.to_le_bytes());
    assert_eq!(&body[12..16], &1u32.to_le_bytes());
    assert_eq!(&body[16..20], &0u32.to_le_bytes());
    assert_eq!(body[20], GOSSIP_ICON_TAXI);
    assert_eq!(body[21], 1);
    assert_eq!(&body[22..44], b"Show me where to fly.\0");
    assert_eq!(&body[44..48], &0u32.to_le_bytes());
}

#[test]
fn session_gossip_state_maps_selection_to_db_option_action() {
    let guid = ObjectGuid::new(HighGuid::Unit, 42, 96_004);
    let mut session = WorldSessionState::default();
    session.gossip.active_guid = Some(guid);
    session.gossip.active_menu_id = 10;
    session.gossip.active_options = vec![
        GossipSessionOption {
            option_id: GOSSIP_OPTION_GOSSIP,
            action_menu_id: 20,
            action_poi_id: 0,
            action_script_id: 0,
        },
        GossipSessionOption {
            option_id: GOSSIP_OPTION_TRAINER,
            action_menu_id: 0,
            action_poi_id: 0,
            action_script_id: 0,
        },
    ];

    assert_eq!(
        session.gossip.active_options[1].option_id,
        GOSSIP_OPTION_TRAINER
    );
    assert_eq!(session.gossip.active_options[0].action_menu_id, 20);
    assert_eq!(session.gossip.active_guid, Some(guid));
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
    assert_eq!(values[UNIT_FIELD_BYTES_0], Some(0x0100_0100));
    assert_eq!(values[UNIT_FIELD_FLAGS], Some(0x20));
    assert_eq!(values[UNIT_FIELD_DISPLAYID], Some(123));
    assert_eq!(values[UNIT_NPC_FLAGS], Some(UNIT_NPC_FLAG_GOSSIP));
}

#[test]
fn db_creature_create_block_serializes_selected_model_gender() {
    let mut creature = test_creature_spawn(42);
    creature.template.display_id1 = 111;
    creature.template.display_id2 = 222;
    creature.template.display_id_probability1 = 0;
    creature.template.display_id_probability2 = 100;
    creature.template.model_gender1 = 0;
    creature.template.model_gender2 = 1;

    let block = build_db_creature_create_block(&creature).unwrap();
    let packed_guid_mask = block[1];
    let update_flags_offset = 1 + 1 + packed_guid_mask.count_ones() as usize + 1;
    let values_start = update_flags_offset + 1 + 56;
    let values = decode_update_values(&block[values_start..]);

    assert_eq!(values[UNIT_FIELD_DISPLAYID], Some(222));
    assert_eq!(values[UNIT_FIELD_NATIVEDISPLAYID], Some(222));
    assert_eq!(values[UNIT_FIELD_BYTES_0], Some(0x0101_0100));
}

#[test]
fn creature_model_selection_uses_model_info_other_gender_row() {
    let mut template = test_creature_template(38);
    template.display_id1 = 111;
    template.display_id_probability1 = 100;
    template.model_gender1 = 0;
    template.model_other_gender1 = 222;
    template.model_other_gender_gender1 = 1;

    assert_eq!(
        choose_creature_display_for_roll(&template, 0, false),
        CreatureDisplaySelection {
            display_id: 111,
            gender: 0,
        }
    );
    assert_eq!(
        choose_creature_display_for_roll(&template, 0, true),
        CreatureDisplaySelection {
            display_id: 222,
            gender: 1,
        }
    );
}

#[test]
fn db_creature_create_block_uses_addon_emote_state() {
    let mut creature = test_creature_spawn(42);
    creature.addon_emote = EMOTE_STATE_DANCE;

    let block = build_db_creature_create_block(&creature).unwrap();
    let packed_guid_mask = block[1];
    let update_flags_offset = 1 + 1 + packed_guid_mask.count_ones() as usize + 1;
    let values_start = update_flags_offset + 1 + 56;
    let values = decode_update_values(&block[values_start..]);

    assert_eq!(values[UNIT_NPC_EMOTESTATE], Some(EMOTE_STATE_DANCE));
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
fn spell_duration_dbc_parser_reads_cmangos_duration_fields() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"WDBC");
    bytes.extend_from_slice(&2u32.to_le_bytes());
    bytes.extend_from_slice(&4u32.to_le_bytes());
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&4u32.to_le_bytes());
    bytes.extend_from_slice(&120_000i32.to_le_bytes());
    bytes.extend_from_slice(&0i32.to_le_bytes());
    bytes.extend_from_slice(&120_000i32.to_le_bytes());
    bytes.extend_from_slice(&99u32.to_le_bytes());
    bytes.extend_from_slice(&(-1i32).to_le_bytes());
    bytes.extend_from_slice(&0i32.to_le_bytes());
    bytes.extend_from_slice(&(-1i32).to_le_bytes());
    bytes.push(0);

    let durations = parse_spell_durations(&bytes);

    assert_eq!(
        durations.get(&4).copied(),
        Some(SpellDurationEntry {
            duration_millis: 120_000,
            duration_per_level_millis: 0,
            max_duration_millis: 120_000,
        })
    );
    assert_eq!(
        durations.get(&99).copied(),
        Some(SpellDurationEntry {
            duration_millis: -1,
            duration_per_level_millis: 0,
            max_duration_millis: -1,
        })
    );
}

#[test]
fn spell_cast_times_dbc_parser_reads_cmangos_cast_time_fields() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"WDBC");
    bytes.extend_from_slice(&2u32.to_le_bytes());
    bytes.extend_from_slice(&4u32.to_le_bytes());
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&7u32.to_le_bytes());
    bytes.extend_from_slice(&1500i32.to_le_bytes());
    bytes.extend_from_slice(&0i32.to_le_bytes());
    bytes.extend_from_slice(&1500i32.to_le_bytes());
    bytes.extend_from_slice(&9u32.to_le_bytes());
    bytes.extend_from_slice(&3000i32.to_le_bytes());
    bytes.extend_from_slice(&100i32.to_le_bytes());
    bytes.extend_from_slice(&1000i32.to_le_bytes());
    bytes.push(0);

    let cast_times = parse_spell_cast_times(&bytes);

    assert_eq!(
        cast_times.get(&7).copied(),
        Some(SpellCastTimeEntry {
            cast_time_millis: 1500,
            cast_time_per_level_millis: 0,
            min_cast_time_millis: 1500,
        })
    );
    assert_eq!(
        cast_times.get(&9).copied(),
        Some(SpellCastTimeEntry {
            cast_time_millis: 3000,
            cast_time_per_level_millis: 100,
            min_cast_time_millis: 1000,
        })
    );
}

#[test]
fn spell_radius_dbc_parser_reads_cmangos_radius_fields() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"WDBC");
    bytes.extend_from_slice(&2u32.to_le_bytes());
    bytes.extend_from_slice(&4u32.to_le_bytes());
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    for (id, radius, per_level, max_radius) in [
        (7u32, 10.0f32, 0.0f32, 10.0f32),
        (11u32, 8.0f32, 0.25f32, 20.0f32),
    ] {
        bytes.extend_from_slice(&id.to_le_bytes());
        bytes.extend_from_slice(&radius.to_le_bytes());
        bytes.extend_from_slice(&per_level.to_le_bytes());
        bytes.extend_from_slice(&max_radius.to_le_bytes());
    }
    bytes.push(0);

    let radii = parse_spell_radii(&bytes);

    assert_eq!(
        radii.get(&7).copied(),
        Some(SpellRadiusEntry {
            radius: 10.0,
            radius_per_level: 0.0,
            max_radius: 10.0,
        })
    );
    assert_eq!(
        radii.get(&11).copied(),
        Some(SpellRadiusEntry {
            radius: 8.0,
            radius_per_level: 0.25,
            max_radius: 20.0,
        })
    );
}

fn test_u32_dbc<const N: usize>(rows: &[[u32; N]]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"WDBC");
    bytes.extend_from_slice(&(rows.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(N as u32).to_le_bytes());
    bytes.extend_from_slice(&((N * 4) as u32).to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    for row in rows {
        for field in row {
            bytes.extend_from_slice(&field.to_le_bytes());
        }
    }
    bytes.push(0);
    bytes
}

#[test]
fn bank_bag_slot_price_dbc_parser_reads_cmangos_rows() {
    let bytes = test_u32_dbc(&[[1, 1_000], [2, 10_000], [6, 250_000]]);

    let prices = parse_bank_bag_slot_prices(&bytes);

    assert_eq!(prices.get(&1), Some(&1_000));
    assert_eq!(prices.get(&2), Some(&10_000));
    assert_eq!(prices.get(&6), Some(&250_000));
}

#[test]
fn auction_house_dbc_parser_reads_cmangos_rates() {
    let bytes = test_u32_dbc(&[[1, 12, 25, 5], [7, 120, 15, 15]]);

    let auction_houses = parse_auction_houses(&bytes);

    assert_eq!(
        auction_houses.get(&1),
        Some(&AuctionHouseEntry {
            house_id: 1,
            faction: 12,
            deposit_percent: 25,
            cut_percent: 5,
        })
    );
    assert_eq!(
        auction_houses.get(&7),
        Some(&AuctionHouseEntry {
            house_id: 7,
            faction: 120,
            deposit_percent: 15,
            cut_percent: 15,
        })
    );
}

#[test]
fn area_table_dbc_parser_indexes_explore_flags_by_map() {
    let bytes = test_u32_dbc(&[
        [
            12, 0, 0, 64, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
        [
            13, 1, 0, 64, 0, 0, 0, 0, 0, 0, 9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
        [
            14, 0, 12, 65, 2, 0, 0, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
    ]);

    let store = AreaTableStore::from_entries(parse_area_tables(&bytes));

    assert_eq!(
        store.entry_by_flag_and_map(64, 0),
        Some(AreaTableEntry {
            id: 12,
            map_id: 0,
            zone_id: 0,
            explore_flag: 64,
            flags: 0,
            area_level: 5,
        })
    );
    assert_eq!(
        store.entry_by_flag_and_map(64, 1),
        Some(AreaTableEntry {
            id: 13,
            map_id: 1,
            zone_id: 0,
            explore_flag: 64,
            flags: 0,
            area_level: 9,
        })
    );
    assert_eq!(store.entry_by_flag_and_map(999, 0), None);
}

#[test]
fn wmo_area_table_dbc_parser_maps_vmap_triple_to_area_table_entry() {
    let area_bytes = test_u32_dbc(&[
        [
            12, 0, 0, 64, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
        [
            13, 1, 0, 64, 0, 0, 0, 0, 0, 0, 9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
    ]);
    let wmo_bytes = test_u32_dbc(&[
        [
            40, 100, 2, 7, 0, 0, 0, 0, 0, 0x8000, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
        [
            41, 100, 2, 8, 0, 0, 0, 0, 0, 0, 13, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
    ]);
    let mut files = WorldDataFiles::fallback();
    files.area_tables = AreaTableStore::from_entries(parse_area_tables(&area_bytes));
    files.wmo_area_tables = WmoAreaTableStore::from_entries(parse_wmo_area_tables(&wmo_bytes));

    assert_eq!(
        files.area_entry_by_wmo_triple_and_map(100, 2, 7, 0),
        Some(AreaTableEntry {
            id: 12,
            map_id: 0,
            zone_id: 0,
            explore_flag: 64,
            flags: 0,
            area_level: 5,
        })
    );
    assert_eq!(
        files.area_entry_by_wmo_triple_and_map(100, 2, 8, 1),
        Some(AreaTableEntry {
            id: 13,
            map_id: 1,
            zone_id: 0,
            explore_flag: 64,
            flags: 0,
            area_level: 9,
        })
    );
    assert_eq!(files.area_entry_by_wmo_triple_and_map(100, 2, 7, 1), None);
}

#[test]
fn skill_line_ability_dbc_parser_reads_spell_to_skill_rank_fields() {
    let bytes = test_u32_dbc(&[
        [1, 237, 587, 0, 128, 0, 0, 0, 0, 0, 300, 1, 0, 0, 0],
        [2, 237, 597, 0, 128, 0, 0, 0, 0, 0, 300, 50, 0, 0, 0],
    ]);

    let abilities = parse_skill_line_abilities(&bytes);

    assert_eq!(
        abilities.get(&587).and_then(|entries| entries.first()),
        Some(&SkillLineAbilityEntry {
            id: 1,
            skill_id: 237,
            spell_id: 587,
            race_mask: 0,
            class_mask: 128,
            min_value: 1,
            max_value: 300,
        })
    );
    assert_eq!(
        abilities.get(&597).and_then(|entries| entries.first()),
        Some(&SkillLineAbilityEntry {
            id: 2,
            skill_id: 237,
            spell_id: 597,
            race_mask: 0,
            class_mask: 128,
            min_value: 50,
            max_value: 300,
        })
    );
}

#[test]
fn skill_line_dbc_parser_reads_cmangos_skill_categories() {
    let bytes = test_u32_dbc(&[
        [
            43, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
        [
            237, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
    ]);

    let lines = parse_skill_lines(&bytes);

    assert_eq!(
        lines.get(&43).copied(),
        Some(SkillLineEntry {
            id: 43,
            category_id: 6,
        })
    );
    assert_eq!(
        lines.get(&237).copied(),
        Some(SkillLineEntry {
            id: 237,
            category_id: 7,
        })
    );
}

#[test]
fn skill_race_class_info_dbc_parser_reads_masks_flags_and_tiers() {
    let bytes = test_u32_dbc(&[[1, 237, 0, 128, 0x010, 1, 0, 0], [2, 43, 1, 1, 0, 1, 2, 0]]);

    let infos = parse_skill_race_class_infos(&bytes);

    assert_eq!(
        infos.get(&237).and_then(|entries| entries.first()),
        Some(&SkillRaceClassInfoEntry {
            skill_id: 237,
            race_mask: 0,
            class_mask: 128,
            flags: 0x010,
            req_level: 1,
            skill_tier_id: 0,
        })
    );
    assert_eq!(
        infos.get(&43).and_then(|entries| entries.first()),
        Some(&SkillRaceClassInfoEntry {
            skill_id: 43,
            race_mask: 1,
            class_mask: 1,
            flags: 0,
            req_level: 1,
            skill_tier_id: 2,
        })
    );
}

#[test]
fn faction_template_dbc_parser_reads_cmangos_relation_fields() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"WDBC");
    bytes.extend_from_slice(&3u32.to_le_bytes());
    bytes.extend_from_slice(&14u32.to_le_bytes());
    bytes.extend_from_slice(&56u32.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    let fixture_rows: [[u32; 14]; 3] = [
        [1, 1, 72, 3, 2, 12, 0, 0, 0, 0, 0, 0, 0, 0],
        [22, 22, 0, 8, 0, 1, 0, 0, 0, 0, 22, 0, 0, 0],
        [25, 25, 0, 8, 0, 0, 0, 0, 0, 0, 25, 0, 0, 0],
    ];
    for fields in fixture_rows {
        for field in fields {
            bytes.extend_from_slice(&field.to_le_bytes());
        }
    }
    bytes.push(0);

    let templates = FactionTemplateStore::from_dbc(parse_faction_templates(&bytes));

    assert!(templates.is_dbc_backed());
    assert_eq!(templates.len(), 3);
    assert_eq!(
        faction_reaction_to(&templates, 22, 1),
        FactionReaction::Hostile,
        "Webwood faction template is hostile to player faction templates in CMaNGOS DBC"
    );
    assert_eq!(
        faction_reaction_to(&templates, 25, 1),
        FactionReaction::Neutral,
        "neutral monster factions must stay neutral even when the DBC parser is active"
    );
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
fn db_creature_runtime_create_block_uses_runtime_mana_for_mana_creatures() {
    let mut spawn = test_creature_spawn(3196);
    spawn.template.unit_class = 2;
    spawn.template.min_level_mana = 178;
    spawn.template.max_level_mana = 191;
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.power1 = 166;

    let block = build_db_creature_runtime_create_block(&runtime).unwrap();
    let packed_guid_mask = block[1];
    let update_flags_offset = 1 + 1 + packed_guid_mask.count_ones() as usize + 1;
    let values_start = update_flags_offset + 1 + 56;
    let values = decode_update_values(&block[values_start..]);

    assert_eq!(values[UNIT_FIELD_POWER1], Some(166));
    assert_eq!(values[UNIT_FIELD_MAXPOWER1], Some(191));
}

#[test]
fn db_creature_runtime_create_block_uses_player_specific_lootability() {
    let mut runtime = DbCreatureRuntime::new(test_creature_spawn(6));
    runtime.begin_corpse(Instant::now(), 1_000);
    runtime.loot_owner = Some(CreatureLootOwner::Player(1));
    runtime.loot_items_generated = true;
    runtime.loot_money_available = false;
    runtime.loot_items = vec![DbCreatureLootRuntime {
        slot: 0,
        item: 117,
        count: 1,
        display_id: 641,
        quality: 1,
        free_for_all: false,
        quest_drop: false,
    }];

    let owner_block = build_db_creature_runtime_create_block_for_player(&runtime, Some(1)).unwrap();
    let other_block = build_db_creature_runtime_create_block_for_player(&runtime, Some(2)).unwrap();
    let packed_guid_mask = owner_block[1];
    let update_flags_offset = 1 + 1 + packed_guid_mask.count_ones() as usize + 1;
    let values_start = update_flags_offset + 1 + 56;
    let owner_values = decode_update_values(&owner_block[values_start..]);
    let other_values = decode_update_values(&other_block[values_start..]);

    assert_eq!(
        owner_values[UNIT_DYNAMIC_FLAGS],
        Some(UNIT_DYNFLAG_LOOTABLE)
    );
    assert_eq!(other_values[UNIT_DYNAMIC_FLAGS], Some(0));
}

#[test]
fn db_creature_unrolled_corpse_loot_still_respects_owner_for_player() {
    let mut runtime = DbCreatureRuntime::new(test_creature_spawn(6));
    runtime.begin_corpse(Instant::now(), 1_000);
    runtime.loot_owner = Some(CreatureLootOwner::Player(1));
    runtime.loot_items_generated = false;

    assert!(runtime.can_loot_for_player(Some(1)));
    assert!(!runtime.can_loot_for_player(Some(2)));
    assert_eq!(
        runtime.dynamic_flags_for_player(Some(1)),
        UNIT_DYNFLAG_LOOTABLE
    );
    assert_eq!(runtime.dynamic_flags_for_player(Some(2)), 0);
}

#[test]
fn loot_error_response_matches_cmangos_shape() {
    let target = ObjectGuid::new(HighGuid::Unit, 6, 45);
    let body = build_loot_error_response_body(target, LOOT_ERROR_DIDNT_KILL);

    assert_eq!(&body[0..8], &target.raw().to_le_bytes());
    assert_eq!(body[8], 0);
    assert_eq!(body[9], LOOT_ERROR_DIDNT_KILL);
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
        &session.quests.quest_statuses,
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
fn quest_objective_gameobject_requires_active_incomplete_objective() {
    assert!(gameobject_type_uses_quest_objective_gate(GO_TYPE_CHEST));
    assert!(gameobject_type_uses_quest_objective_gate(GO_TYPE_GOOBER));

    let mut quest = test_quest_template(3904);
    quest.req_creature_or_go_id = [-161557, 0, 0, 0];
    quest.req_creature_or_go_count = [8, 0, 0, 0];
    let mut session = WorldSessionState::default();

    assert!(!session_has_incomplete_gameobject_objective(
        &session,
        &[quest.clone()],
        161557
    ));

    session.quests.quest_statuses.insert(
        quest.entry,
        CharacterQuestStatus {
            quest: quest.entry,
            status: QUEST_STATUS_INCOMPLETE,
            rewarded: 0,
            mobcount1: 7,
            mobcount2: 0,
            mobcount3: 0,
            mobcount4: 0,
        },
    );
    assert!(session_has_incomplete_gameobject_objective(
        &session,
        &[quest.clone()],
        161557
    ));

    session.quests.quest_statuses.insert(
        quest.entry,
        CharacterQuestStatus {
            mobcount1: 8,
            ..session.quests.quest_statuses[&quest.entry].clone()
        },
    );
    assert!(!session_has_incomplete_gameobject_objective(
        &session,
        &[quest],
        161557
    ));
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
fn self_spawn_update_chunks_without_synthetic_fixture_blocks() {
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
        inventory_container_slots: &HashMap::new(),
        base_world_stats: &world_stats,
        world_stats: &world_stats,
        skills: &[],
        quest_statuses: &quest_statuses,
        equipped_templates: &[],
        ammo_template: None,
        active_auras: &[],
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
        .visibility
        .db_gameobjects
        .insert(out_guid, DbGameObjectRuntime::new(out_of_range));
    session.visibility.last_gameobject_visibility_position =
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
        .visibility
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
    assert!(
        !session
            .visibility
            .db_gameobjects
            .get(&guid)
            .unwrap()
            .client_visible
    );
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
        .visibility
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
    assert!(session.visibility.db_creatures.contains_key(&known_guid));
    assert!(session.visibility.db_creatures.contains_key(&new_guid));
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
    let runtime = session.visibility.db_creatures.get(&dead_guid).unwrap();
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
    session.visibility.db_creatures.insert(guid, local);

    let updates = stage_db_creature_visibility_updates(
        &mut session,
        WorldPosition::new(0, -8790.0, -95.0, 83.5, 0.0),
        vec![shared],
    )
    .unwrap();

    assert_eq!(updates.destroy_guids, vec![ObjectGuid::from_raw(guid)]);
    let runtime = session.visibility.db_creatures.get(&guid).unwrap();
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
    session.visibility.db_creatures.insert(guid.raw(), runtime);

    let body = build_db_creature_motion_stop_body(&mut session, guid)
        .unwrap()
        .expect("motion stop body");

    assert!(!body.is_empty());
    assert!(matches!(
        session
            .visibility
            .db_creatures
            .get(&guid.raw())
            .unwrap()
            .motion,
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
    session.visibility.db_creatures.insert(guid, corpse);

    let unload = stage_db_creature_visibility_updates(
        &mut session,
        WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(unload.destroy_guids, vec![ObjectGuid::from_raw(guid)]);
    assert!(
        !session
            .visibility
            .db_creatures
            .get(&guid)
            .unwrap()
            .client_visible
    );
    assert_eq!(
        session
            .visibility
            .db_creatures
            .get(&guid)
            .unwrap()
            .life_state,
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
    assert!(
        session
            .visibility
            .db_creatures
            .get(&guid)
            .unwrap()
            .client_visible
    );
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
        .visibility
        .db_creatures
        .insert(nearby_guid, DbCreatureRuntime::new(nearby.clone()));
    session.visibility.db_creatures.insert(
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
    assert!(session.visibility.db_creatures.contains_key(&nearby_guid));
    assert!(!session
        .visibility
        .db_creatures
        .contains_key(&out_of_range_guid.raw()));
    assert_eq!(session.combat.active_combat_target, None);
    assert!(session.combat.active_creature_combats.is_empty());
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
        .visibility
        .db_creatures
        .insert(nearby_guid, DbCreatureRuntime::new(nearby.clone()));
    session.visibility.db_creatures.insert(
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
    assert!(session
        .visibility
        .db_creatures
        .contains_key(&edge_visible_guid.raw()));
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
        .visibility
        .db_creatures
        .insert(nearby_guid, DbCreatureRuntime::new(nearby.clone()));
    session.visibility.db_creatures.insert(
        out_of_query_guid.raw(),
        DbCreatureRuntime::new(out_of_query),
    );
    session.combat.active_combat_target = Some(out_of_query_guid);
    session.combat.active_creature_combats.insert(
        out_of_query_guid.raw(),
        CreatureCombatState {
            attacker: out_of_query_guid,
            victim: ObjectGuid::new(HighGuid::Player, 0, 7),
            started_at: Instant::now(),
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
    assert!(session.visibility.db_creatures.contains_key(&nearby_guid));
    assert!(session
        .visibility
        .db_creatures
        .contains_key(&out_of_query_guid.raw()));
    assert_eq!(session.combat.active_combat_target, Some(out_of_query_guid));
    assert!(!session.combat.active_creature_combats.is_empty());
}

#[test]
fn movement_visibility_rescan_uses_distance_threshold() {
    let session = WorldSessionState {
        visibility: VisibilitySessionState {
            last_creature_visibility_position: Some(WorldPosition::new(0, 10.0, 10.0, 0.0, 0.0)),
            ..VisibilitySessionState::default()
        },
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
    let tick = Duration::from_millis(WORLD_TICK_MILLIS);

    advance_world_tick_deadline(
        &mut next,
        now + Duration::from_millis(WORLD_TICK_MILLIS * 2 + 1),
        tick,
    );

    assert!(next > now + Duration::from_millis(WORLD_TICK_MILLIS * 2 + 1));
    assert_eq!(next, now + Duration::from_millis(WORLD_TICK_MILLIS * 3));
}

#[test]
fn world_tick_deadline_uses_configured_interval() {
    let now = Instant::now();
    let mut next = now;
    let tick = Duration::from_millis(250);

    advance_world_tick_deadline(&mut next, now + Duration::from_millis(501), tick);

    assert_eq!(next, now + Duration::from_millis(750));
}
