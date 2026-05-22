#[test]
fn db_creature_retaliation_can_kill_player() {
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            player_health: 5,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    let creature = test_creature_spawn(299);
    let target = creature_spawn_guid(&creature);
    let expected_hit = DbCreatureRuntime::new(creature).hit_damage().max(1);
    session.visibility.db_creatures.insert(
        target.raw(),
        DbCreatureRuntime::new(test_creature_spawn(299)),
    );

    let retaliation = retaliation_damage_for_db_creature(&mut session, target);
    assert_eq!(retaliation, expected_hit);
    assert_eq!(
        session.character.player_health,
        (5u32).saturating_sub(expected_hit)
    );

    session.character.player_health = 1;
    let retaliation = retaliation_damage_for_db_creature(&mut session, target);
    assert_eq!(retaliation, expected_hit);
    assert_eq!(session.character.player_health, 0);
}

#[test]
fn player_death_update_sets_health_flags_and_release_timer() {
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let body = build_player_death_update_body(PlayerDeathUpdate {
        player,
        health: 0,
        player_flags: PLAYER_FLAGS_GHOST,
        field_bytes: PLAYER_FIELD_BYTE_RELEASE_TIMER,
        unit_flags: player_unit_flags(false),
        race: 1,
        class: 1,
        stand_state: PLAYER_STAND_STATE_DEAD,
    })
    .unwrap();
    let mut packed = Vec::new();
    PackedGuid::write(&mut packed, player).unwrap();
    let values_start = 4 + 1 + 1 + packed.len();
    let values = decode_update_values(&body[values_start..]);

    assert_eq!(values[UNIT_FIELD_HEALTH], Some(0));
    assert_eq!(values[UNIT_FIELD_POWER2], Some(0));
    assert_eq!(values[UNIT_FIELD_FLAGS], Some(UNIT_FLAG_PLAYER_CONTROLLED));
    assert_eq!(
        values[UNIT_FIELD_BYTES_1],
        Some(unit_bytes_1_for_class(1) | u32::from(PLAYER_STAND_STATE_DEAD))
    );
    assert_eq!(values[PLAYER_FLAGS_FIELD], Some(PLAYER_FLAGS_GHOST));
    assert_eq!(
        values[PLAYER_FIELD_BYTES],
        Some(PLAYER_FIELD_BYTE_RELEASE_TIMER)
    );
    let debuff_slot = MAX_POSITIVE_AURA_SLOTS;
    assert_eq!(values[UNIT_FIELD_AURA], Some(0));
    assert_eq!(values[UNIT_FIELD_AURAFLAGS], Some(0));
    assert_eq!(values[UNIT_FIELD_AURALEVELS], Some(0));
    assert_eq!(values[UNIT_FIELD_AURAAPPLICATIONS], Some(0));
    assert_eq!(values[UNIT_FIELD_AURA + debuff_slot], Some(GHOST_SPELL_ID));
    assert_eq!(
        values[UNIT_FIELD_AURAFLAGS + (debuff_slot / 8)],
        Some(GHOST_AURA_FLAGS)
    );
    assert_eq!(values[UNIT_FIELD_AURALEVELS + (debuff_slot / 4)], Some(1));
    assert_eq!(
        values[UNIT_FIELD_AURAAPPLICATIONS + (debuff_slot / 4)],
        Some(0)
    );
}

#[test]
fn corpse_state_arms_auto_repop_deadline() {
    let now = Instant::now();
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            player_health: 0,
            ..CharacterSessionState::default()
        },
        death: DeathSessionState {
            player_death_state: PlayerDeathState::Corpse,
            ..DeathSessionState::default()
        },
        ..WorldSessionState::default()
    };

    mark_player_auto_repop_if_corpse(&mut session, now);

    let due_at = pending_player_auto_repop_due_at(&session).unwrap();
    assert_eq!(due_at, now + PLAYER_DEATH_AUTO_REPOP_DELAY);
    assert!(!player_auto_repop_is_due(
        &session,
        due_at - Duration::from_millis(1)
    ));
    assert!(player_auto_repop_is_due(&session, due_at));
}

#[test]
fn player_alive_recovery_update_clears_all_visible_aura_slots() {
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let body = build_player_death_update_body(PlayerDeathUpdate {
        player,
        health: 50,
        player_flags: 0,
        field_bytes: 0,
        unit_flags: player_unit_flags(false),
        race: 1,
        class: 1,
        stand_state: PLAYER_STAND_STATE_STAND,
    })
    .unwrap();
    let mut packed = Vec::new();
    PackedGuid::write(&mut packed, player).unwrap();
    let values_start = 4 + 1 + 1 + packed.len();
    let values = decode_update_values(&body[values_start..]);

    assert_eq!(values[UNIT_FIELD_AURA], Some(0));
    assert_eq!(values[UNIT_FIELD_POWER2], Some(0));
    assert_eq!(values[UNIT_FIELD_AURAFLAGS], Some(0));
    let debuff_slot = MAX_POSITIVE_AURA_SLOTS;
    assert_eq!(values[UNIT_FIELD_AURA + debuff_slot], Some(0));
    assert_eq!(values[UNIT_FIELD_AURAFLAGS + (debuff_slot / 8)], Some(0));
}

#[test]
fn night_elf_ghost_update_includes_wisp_form_as_negative_aura() {
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let body = build_player_death_update_body(PlayerDeathUpdate {
        player,
        health: 1,
        player_flags: PLAYER_FLAGS_GHOST,
        field_bytes: 0,
        unit_flags: player_unit_flags(false),
        race: PLAYER_RACE_NIGHT_ELF,
        class: 4,
        stand_state: PLAYER_STAND_STATE_STAND,
    })
    .unwrap();
    let mut packed = Vec::new();
    PackedGuid::write(&mut packed, player).unwrap();
    let values_start = 4 + 1 + 1 + packed.len();
    let values = decode_update_values(&body[values_start..]);

    let debuff_slot = MAX_POSITIVE_AURA_SLOTS;
    assert_eq!(values[UNIT_FIELD_AURA], Some(0));
    assert_eq!(values[UNIT_FIELD_AURA + 1], Some(0));
    assert_eq!(
        values[UNIT_FIELD_AURA + debuff_slot],
        Some(NIGHT_ELF_WISP_FORM_SPELL_ID)
    );
    assert_eq!(
        values[UNIT_FIELD_AURA + debuff_slot + 1],
        Some(GHOST_SPELL_ID)
    );
    assert_eq!(
        values[UNIT_FIELD_AURAFLAGS + (debuff_slot / 8)],
        Some(GHOST_AURA_FLAGS | (GHOST_AURA_FLAGS << 4))
    );
}

#[test]
fn force_move_unroot_body_matches_root_ack_shape() {
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let body = build_force_move_unroot_body(player, 23).unwrap();
    let mut expected = Vec::new();
    PackedGuid::write(&mut expected, player).unwrap();
    expected.extend_from_slice(&23u32.to_le_bytes());

    assert_eq!(body, expected);
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
fn spirit_healer_detection_uses_db_npc_flag() {
    let mut flagged = test_creature_spawn(197);
    flagged.template.npc_flags = UNIT_NPC_FLAG_SPIRITHEALER;
    assert!(is_spirit_healer_creature(&DbCreatureRuntime::new(flagged)));

    let mut classic_entry_without_flag = test_creature_spawn(6491);
    classic_entry_without_flag.template.npc_flags = 0;
    assert!(!is_spirit_healer_creature(&DbCreatureRuntime::new(
        classic_entry_without_flag
    )));

    let mut trainer = test_creature_spawn(197);
    trainer.template.npc_flags = UNIT_NPC_FLAG_TRAINER;
    assert!(!is_spirit_healer_creature(&DbCreatureRuntime::new(trainer)));
}

#[test]
fn db_creature_create_block_preserves_db_npc_flags_without_entry_fallback() {
    let mut creature = test_creature_spawn(6491);
    creature.template.npc_flags = UNIT_NPC_FLAG_GOSSIP;
    let runtime = DbCreatureRuntime::new(creature);
    let body = build_db_creature_runtime_create_block(&runtime).unwrap();
    let packed_guid_mask = body[1];
    let update_flags_offset = 1 + 1 + packed_guid_mask.count_ones() as usize + 1;
    let values_start = update_flags_offset + 1 + 56;
    let values = decode_update_values(&body[values_start..]);

    assert_eq!(values[UNIT_NPC_FLAGS], Some(UNIT_NPC_FLAG_GOSSIP));
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
        jump: JumpInfo::default(),
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

fn test_mmap_navigation_for_positions(
    positions: &[WorldPosition],
) -> DbCreatureNavigationGuardrail {
    let mut mmap_headers = HashSet::new();
    let mut mmap_tiles = HashSet::new();
    for position in positions {
        if let Some((tile_x, tile_y)) = mmap_tile_for_position(*position) {
            mmap_headers.insert(position.map_id);
            mmap_tiles.insert((position.map_id, tile_x, tile_y));
        }
    }
    DbCreatureNavigationGuardrail {
        world_data_files: Arc::new(WorldDataFiles {
            data_dir: std::path::PathBuf::from("fixture"),
            data_dir_for_native: None,
            maps_available: false,
            vmaps_available: false,
            creature_display_scales: HashMap::new(),
            spell_cast_times: HashMap::new(),
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
            mmap_headers,
            mmap_tiles,
            vmap_trees: HashSet::new(),
            vmap_tiles: HashSet::new(),
        }),
        ..DbCreatureNavigationGuardrail::default()
    }
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
        jump: JumpInfo::default(),
    };
    let character_position = character.position;
    let mut far_hostile = test_creature_spawn(6);
    far_hostile.guid = 45;
    far_hostile.position_x = -8931.0;
    far_hostile.template.faction = 17;
    far_hostile.template.npc_flags = 0;
    far_hostile.template.min_level = 1;
    let mut near_hostile = test_creature_spawn(6);
    near_hostile.guid = 46;
    near_hostile.position_x = -8940.0;
    near_hostile.template.faction = 17;
    near_hostile.template.npc_flags = 0;
    near_hostile.template.min_level = 1;
    let mut friendly = test_creature_spawn(197);
    friendly.guid = 47;
    friendly.position_x = -8945.0;
    friendly.template.faction = GM_FRIENDLY_FACTION_TEMPLATE;
    friendly.template.npc_flags = UNIT_NPC_FLAG_GOSSIP;
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(character),
            ..CharacterSessionState::default()
        },
        movement: MovementSessionState {
            db_creature_navigation: test_mmap_navigation_for_positions(&[
                character_position,
                WorldPosition::new(
                    0,
                    far_hostile.position_x,
                    far_hostile.position_y,
                    far_hostile.position_z,
                    0.0,
                ),
                WorldPosition::new(
                    0,
                    near_hostile.position_x,
                    near_hostile.position_y,
                    near_hostile.position_z,
                    0.0,
                ),
            ]),
            ..MovementSessionState::default()
        },
        ..WorldSessionState::default()
    };
    for creature in [far_hostile.clone(), near_hostile.clone(), friendly] {
        let runtime = DbCreatureRuntime::new(creature);
        session
            .visibility
            .db_creatures
            .insert(runtime.guid().raw(), runtime);
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
        jump: JumpInfo::default(),
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
        character: CharacterSessionState {
            active_character: Some(character),
            ..CharacterSessionState::default()
        },
        death: DeathSessionState {
            player_death_state: PlayerDeathState::Ghost,
            ..DeathSessionState::default()
        },
        ..WorldSessionState::default()
    };
    session
        .visibility
        .db_creatures
        .insert(runtime.guid().raw(), runtime);

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
        jump: JumpInfo::default(),
    };
    let first = creature_spawn_guid(&test_creature_spawn(6));
    let mut second_spawn = test_creature_spawn(38);
    second_spawn.guid = 46;
    let second = creature_spawn_guid(&second_spawn);
    let now = Instant::now();
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(character),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    assert!(begin_db_creature_combat(&mut session, first, now));
    assert!(begin_db_creature_combat(
        &mut session,
        second,
        now + Duration::from_millis(10)
    ));
    assert_eq!(session.combat.active_creature_combats.len(), 2);
    assert!(!begin_db_creature_combat(
        &mut session,
        first,
        now + Duration::from_secs(1)
    ));

    clear_db_creature_combat_if_attacker(&mut session, first);
    assert!(!session
        .combat
        .active_creature_combats
        .contains_key(&first.raw()));
    assert!(session
        .combat
        .active_creature_combats
        .contains_key(&second.raw()));
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
        jump: JumpInfo::default(),
    };
    let mut kobold = test_creature_spawn(6);
    kobold.guid = 45;
    kobold.position_x = -8931.0;
    kobold.template.faction = 17;
    kobold.template.npc_flags = 0;
    kobold.template.min_level = 1;
    kobold.template.detection_range = 18;
    let character_position = character.position;
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(character),
            ..CharacterSessionState::default()
        },
        movement: MovementSessionState {
            db_creature_navigation: test_mmap_navigation_for_positions(&[
                character_position,
                WorldPosition::new(
                    0,
                    kobold.position_x,
                    kobold.position_y,
                    kobold.position_z,
                    0.0,
                ),
            ]),
            ..MovementSessionState::default()
        },
        ..WorldSessionState::default()
    };
    let runtime = DbCreatureRuntime::new(kobold.clone());
    session
        .visibility
        .db_creatures
        .insert(runtime.guid().raw(), runtime);

    assert_eq!(select_db_creature_aggro_target(&session), None);

    session.visibility.db_creatures.clear();
    kobold.template.detection_range = 20;
    let runtime = DbCreatureRuntime::new(kobold.clone());
    session
        .visibility
        .db_creatures
        .insert(runtime.guid().raw(), runtime);

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
        jump: JumpInfo::default(),
    };
    let character_position = character.position;
    let kobold_position = WorldPosition::new(
        0,
        kobold.position_x,
        kobold.position_y,
        kobold.position_z,
        0.0,
    );
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(character),
            ..CharacterSessionState::default()
        },
        movement: MovementSessionState {
            db_creature_navigation: test_mmap_navigation_for_positions(&[
                character_position,
                kobold_position,
            ]),
            ..MovementSessionState::default()
        },
        ..WorldSessionState::default()
    };
    let runtime = DbCreatureRuntime::new(kobold);
    session
        .visibility
        .db_creatures
        .insert(runtime.guid().raw(), runtime);

    assert_eq!(
        db_creature_player_melee_check(&session, target),
        PlayerMeleeCheck::Clear
    );

    session
        .character
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
        .character
        .active_character
        .as_mut()
        .unwrap()
        .position
        .orientation = 0.0;
    session
        .visibility
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
        .visibility
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
        .visibility
        .db_creatures
        .get_mut(&target.raw())
        .unwrap()
        .current_position
        .x = 4.0;
    session
        .visibility
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
fn db_creature_player_melee_check_allows_evade_feedback_for_returning_creature() {
    let mut kobold = test_creature_spawn(6);
    kobold.guid = 46;
    kobold.position_x = 4.0;
    kobold.position_y = 0.0;
    kobold.position_z = 0.0;
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
        jump: JumpInfo::default(),
    };
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(character),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    let mut runtime = DbCreatureRuntime::new(kobold);
    runtime.motion = CreatureMotionState::ReturnHome(CreatureReturnHomeMotion {
        start: runtime.current_position,
        destination: runtime.home_position,
        path: vec![runtime.current_position, runtime.home_position],
        started_at: Instant::now(),
        duration: Duration::from_secs(1),
    });
    session
        .visibility
        .db_creatures
        .insert(runtime.guid().raw(), runtime);

    assert_eq!(
        db_creature_player_melee_check(&session, target),
        PlayerMeleeCheck::TargetEvading
    );

    session
        .visibility
        .db_creatures
        .get_mut(&target.raw())
        .unwrap()
        .current_position
        .x = 8.0;
    assert_eq!(
        db_creature_player_melee_check(&session, target),
        PlayerMeleeCheck::OutOfRange,
        "CMaNGOS checks melee reach before rolling an evade result"
    );

    assert_eq!(MeleeDamageOutcome::evade().victim_state, VICTIMSTATE_EVADES);
    assert_eq!(
        MeleeDamageOutcome::evade().spell_miss_info(),
        Some(SPELL_MISS_EVADE)
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
        jump: JumpInfo::default(),
    };
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(character),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    let runtime = DbCreatureRuntime::new(kobold);
    session
        .visibility
        .db_creatures
        .insert(runtime.guid().raw(), runtime);

    assert_eq!(
        db_creature_player_melee_check(&session, target),
        PlayerMeleeCheck::Clear
    );

    session
        .visibility
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
        .visibility
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
            WorldOpcode::SmsgAttackSwingNotInRange as u16,
        ),
        (
            PlayerMeleeSwingError::BadFacing,
            WorldOpcode::SmsgAttackSwingBadFacing as u16,
        ),
        (
            PlayerMeleeSwingError::DeadTarget,
            WorldOpcode::SmsgAttackSwingDeadTarget as u16,
        ),
        (
            PlayerMeleeSwingError::CantAttack,
            WorldOpcode::SmsgAttackSwingCantAttack as u16,
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
        jump: JumpInfo::default(),
    };
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(character),
            ..CharacterSessionState::default()
        },
        movement: MovementSessionState {
            db_creature_navigation: DbCreatureNavigationGuardrail {
                line_of_sight_clear: false,
                path_available: true,
                world_data_files: Arc::new(WorldDataFiles {
                    data_dir: std::path::PathBuf::from("Z:/definitely-missing-cmangos-data"),
                    data_dir_for_native: std::ffi::CString::new(
                        "Z:/definitely-missing-cmangos-data",
                    )
                    .ok(),
                    maps_available: true,
                    vmaps_available: false,
                    creature_display_scales: HashMap::new(),
                    spell_cast_times: HashMap::new(),
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
                }),
            },
            ..MovementSessionState::default()
        },
        ..WorldSessionState::default()
    };
    let runtime = DbCreatureRuntime::new(kobold);
    session
        .visibility
        .db_creatures
        .insert(runtime.guid().raw(), runtime);

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
    let spell_profile = player_spell_cast_profile(&instant_melee_damage_spell_template()).unwrap();
    let targets = SpellCastTargets {
        target_mask: SPELL_CAST_TARGET_UNIT,
        unit_target: Some(target),
        gameobject_target: None,
        source_location: None,
        destination: None,
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
        jump: JumpInfo::default(),
    };
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(character),
            ..CharacterSessionState::default()
        },
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
        spell_melee_cast_failure(shared_world, &mut session, &spell_profile, &targets).await,
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
        .character
        .active_character
        .as_mut()
        .unwrap()
        .position
        .orientation = std::f32::consts::PI;
    maps.sync_player_gameplay_state(0, 7, &session).await;
    assert_eq!(
        spell_melee_cast_failure(shared_world, &mut session, &spell_profile, &targets).await,
        Some(SPELL_FAILED_UNIT_NOT_INFRONT)
    );

    session
        .character
        .active_character
        .as_mut()
        .unwrap()
        .position
        .orientation = 0.0;
    maps.sync_player_gameplay_state(0, 7, &session).await;
    assert_eq!(
        spell_melee_cast_failure(shared_world, &mut session, &spell_profile, &targets).await,
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
        jump: JumpInfo::default(),
    };
    let mut friendly = test_creature_spawn(197);
    friendly.guid = 45;
    friendly.position_x = -8945.0;
    friendly.template.faction = GM_FRIENDLY_FACTION_TEMPLATE;
    friendly.template.npc_flags = UNIT_NPC_FLAG_GOSSIP;
    let mut critter = test_creature_spawn(6);
    critter.guid = 46;
    critter.position_x = -8945.0;
    critter.template.faction = 17;
    critter.template.npc_flags = 0;
    critter.template.creature_type = CREATURE_TYPE_CRITTER;
    let mut out_of_range = test_creature_spawn(6);
    out_of_range.guid = 47;
    out_of_range.position_x = -8930.0;
    out_of_range.template.faction = 17;
    out_of_range.template.npc_flags = 0;
    out_of_range.template.min_level = 1;
    let mut lootable = test_creature_spawn(300);
    lootable.guid = 48;
    lootable.position_x = -8945.0;
    lootable.template.faction = 17;
    lootable.template.npc_flags = 0;
    let mut lootable_runtime = DbCreatureRuntime::new(lootable);
    lootable_runtime.health = 0;
    lootable_runtime.lootable = true;
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(character),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    for creature in [friendly, critter, out_of_range] {
        let runtime = DbCreatureRuntime::new(creature);
        session
            .visibility
            .db_creatures
            .insert(runtime.guid().raw(), runtime);
    }
    session
        .visibility
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
        jump: JumpInfo::default(),
    };
    let mut guard = test_creature_spawn(197);
    guard.guid = 45;
    guard.position_x = -8945.0;
    guard.template.faction = 9_999;
    guard.template.npc_flags = 0;
    guard.template.creature_type = 7;
    let runtime = DbCreatureRuntime::new(guard);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(character),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    session
        .visibility
        .db_creatures
        .insert(runtime.guid().raw(), runtime);

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
        jump: JumpInfo::default(),
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
        character: CharacterSessionState {
            active_character: Some(character),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    session
        .visibility
        .db_creatures
        .insert(runtime.guid().raw(), runtime);

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
        jump: JumpInfo::default(),
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
        character: CharacterSessionState {
            active_character: Some(character),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    session
        .visibility
        .db_creatures
        .insert(runtime.guid().raw(), runtime);

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
        jump: JumpInfo::default(),
    };
    let mut defias = test_creature_spawn(38);
    defias.guid = 45;
    defias.position_x = -8951.0;
    defias.position_y = -130.0;
    defias.template.faction = 17;
    defias.template.npc_flags = 0;
    defias.template.creature_type = 7;
    defias.template.min_level = 2;
    let character_position = character.position;
    let defias_position = WorldPosition::new(
        0,
        defias.position_x,
        defias.position_y,
        defias.position_z,
        0.0,
    );
    let defias_guid = creature_spawn_guid(&defias);
    let runtime = DbCreatureRuntime::new(defias);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(character),
            ..CharacterSessionState::default()
        },
        movement: MovementSessionState {
            db_creature_navigation: test_mmap_navigation_for_positions(&[
                character_position,
                defias_position,
            ]),
            ..MovementSessionState::default()
        },
        ..WorldSessionState::default()
    };
    session
        .visibility
        .db_creatures
        .insert(runtime.guid().raw(), runtime);

    assert_eq!(select_db_creature_aggro_target(&session), Some(defias_guid));
}

#[test]
fn db_creature_aggro_uses_cmangos_faction_template_reactions() {
    let faction_templates = FactionTemplateStore::fallback_bridge();
    assert_eq!(
        faction_reaction_to(&faction_templates, 17, 1),
        FactionReaction::Hostile,
        "Defias Thug faction should be hostile to Alliance players"
    );
    assert_eq!(
        faction_reaction_to(&faction_templates, 25, 1),
        FactionReaction::Neutral,
        "Kobold Vermin faction should not auto-aggro Alliance players"
    );
    assert_eq!(
        faction_reaction_to(&faction_templates, 32, 1),
        FactionReaction::Neutral,
        "Young Wolf faction should not auto-aggro"
    );
    assert_eq!(
        faction_reaction_to(&faction_templates, 12, 1),
        FactionReaction::Friendly,
        "Northshire friendly NPCs should not auto-aggro Alliance players"
    );
    assert!(!can_faction_attack_on_sight(&faction_templates, 9_999, 1));
}

#[tokio::test]
async fn dead_player_attack_swing_does_not_start_map_auto_attack() {
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
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    maps.add_player(test_player_runtime(7, SessionId(7), position))
        .await
        .unwrap();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 45;
    spawn.position_x = position.x + 2.0;
    spawn.position_y = position.y;
    spawn.position_z = position.z;
    let target = creature_spawn_guid(&spawn);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(spawn)])
        .await;

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
            player_health: 0,
            ..CharacterSessionState::default()
        },
        death: DeathSessionState {
            player_death_state: PlayerDeathState::Corpse,
            ..DeathSessionState::default()
        },
        ..WorldSessionState::default()
    };
    let mut body = Vec::new();
    body.extend_from_slice(&target.raw().to_le_bytes());

    handle_attack_swing(
        &mut stream,
        shared_world,
        &PartyManager::default(),
        read_attack_swing_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    assert!(rx.try_recv().is_err());
    let snapshot = maps.player_runtime_snapshot(0, 7).await.unwrap();
    assert_eq!(snapshot.active_combat_target, None);
    assert_eq!(snapshot.active_combat_next_swing_at, None);
}

#[tokio::test]
async fn db_creature_combat_state_tracks_victim_and_next_swing() {
    let attacker = creature_spawn_guid(&test_creature_spawn(299));
    let now = Instant::now();
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
    let mut session = WorldSessionState {
        character: CharacterSessionState {
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
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    let attacker_runtime = DbCreatureRuntime::new(test_creature_spawn(299));
    maps.share_db_creature_snapshots(0, vec![attacker_runtime.clone()])
        .await;
    session
        .visibility
        .db_creatures
        .insert(attacker.raw(), attacker_runtime);

    assert!(begin_shared_db_creature_combat(shared_world, &mut session, attacker, now).await);

    let combat = session
        .combat
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
        .combat
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
        .combat
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
    assert!(session.combat.active_creature_combats.is_empty());
}

#[tokio::test]
async fn begin_shared_db_creature_combat_uses_mapruntime_liveness_without_session_cache() {
    let mut attacker_spawn = test_creature_spawn(299);
    attacker_spawn.guid = 333;
    let attacker = creature_spawn_guid(&attacker_spawn);
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
    let mut session = WorldSessionState {
        character: CharacterSessionState {
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
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        death: DeathSessionState {
            player_death_state: PlayerDeathState::Alive,
            ..DeathSessionState::default()
        },
        ..WorldSessionState::default()
    };
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(attacker_spawn)])
        .await;

    assert!(
        begin_shared_db_creature_combat(shared_world, &mut session, attacker, Instant::now()).await,
        "shared combat should start from MapRuntime even when the session viewer cache is empty"
    );
    assert!(session
        .visibility
        .db_creatures
        .contains_key(&attacker.raw()));
    assert!(session
        .combat
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
    let mut player = test_player_runtime(7, SessionId(7), player_position);
    player.power1 = 100;
    player.max_power1 = 100;
    maps.add_player(player).await.unwrap();

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
            ..CharacterSessionState::default()
        },
        visibility: VisibilitySessionState {
            db_creatures: HashMap::from([(target.raw(), DbCreatureRuntime::new(stale_spawn))]),
            ..VisibilitySessionState::default()
        },
        ..WorldSessionState::default()
    };

    assert_eq!(
        db_creature_player_melee_check_from_map(shared_world, &mut session, target).await,
        PlayerMeleeCheck::Clear
    );
    assert_eq!(
        session
            .visibility
            .db_creatures
            .get(&target.raw())
            .unwrap()
            .current_position
            .x,
        4.0,
        "the session cache should be refreshed from the authoritative map snapshot"
    );

    session.visibility.db_creatures.clear();
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
        character: CharacterSessionState {
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
                jump: JumpInfo::default(),
            }),
            player_health: 1,
            ..CharacterSessionState::default()
        },
        death: DeathSessionState {
            player_death_state: PlayerDeathState::Alive,
            ..DeathSessionState::default()
        },
        ..WorldSessionState::default()
    };
    let attacker_runtime = DbCreatureRuntime::new(attacker_spawn);
    maps.share_db_creature_snapshots(0, vec![attacker_runtime.clone()])
        .await;
    session
        .visibility
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
        .combat
        .active_creature_combats
        .contains_key(&attacker.raw()));
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgAttackStart as u16));
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgUpdateObject as u16));
}

#[tokio::test]
async fn player_hit_calls_nearby_db_creature_assistance() {
    let mut attacker_spawn = test_creature_spawn(299);
    attacker_spawn.guid = 1_900;
    attacker_spawn.position_x = -8950.0;
    attacker_spawn.position_y = -130.0;
    attacker_spawn.position_z = 83.5;
    attacker_spawn.template.faction = 17;
    attacker_spawn.template.npc_flags = 0;
    attacker_spawn.template.call_for_help = 6;
    let attacker = creature_spawn_guid(&attacker_spawn);

    let mut assistant_spawn = test_creature_spawn(299);
    assistant_spawn.guid = 1_901;
    assistant_spawn.position_x = -8947.0;
    assistant_spawn.position_y = -130.0;
    assistant_spawn.position_z = 83.5;
    assistant_spawn.template.faction = 17;
    assistant_spawn.template.npc_flags = 0;
    let assistant = creature_spawn_guid(&assistant_spawn);

    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let mut session = WorldSessionState {
        character: CharacterSessionState {
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
                jump: JumpInfo::default(),
            }),
            player_health: 1,
            ..CharacterSessionState::default()
        },
        death: DeathSessionState {
            player_death_state: PlayerDeathState::Alive,
            ..DeathSessionState::default()
        },
        ..WorldSessionState::default()
    };
    maps.share_db_creature_snapshots(
        0,
        vec![
            DbCreatureRuntime::new(attacker_spawn),
            DbCreatureRuntime::new(assistant_spawn),
        ],
    )
    .await;

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
        .combat
        .active_creature_combats
        .contains_key(&attacker.raw()));
    assert!(session
        .combat
        .active_creature_combats
        .contains_key(&assistant.raw()));
    assert_eq!(
        packets
            .iter()
            .filter(|packet| packet.opcode == WorldOpcode::SmsgAttackStart as u16)
            .count(),
        2,
        "primary and assisting creatures should both announce combat start"
    );
}

#[test]
fn db_creature_melee_reach_is_position_gated() {
    let mut creature = test_creature_spawn(299);
    creature.position_x = -8950.0;
    creature.position_y = -130.0;
    creature.orientation = 0.0;
    let target = creature_spawn_guid(&creature);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
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
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    session
        .visibility
        .db_creatures
        .insert(target.raw(), DbCreatureRuntime::new(creature));

    assert!(db_creature_can_reach_player(&session, target));
    assert!(db_creature_has_player_in_arc(&session, target));
    session
        .character
        .active_character
        .as_mut()
        .unwrap()
        .position
        .x = -8950.0 - ATTACK_DISTANCE_YARDS + 0.1;
    assert!(db_creature_can_reach_player(&session, target));
    assert!(!db_creature_has_player_in_arc(&session, target));
    let (facing_position, spline_id) =
        face_db_creature_toward_player(&mut session, target).expect("creature should face player");
    assert_eq!(spline_id, 0);
    assert_eq!(facing_position.x, -8950.0);
    assert!(db_creature_has_player_in_arc(&session, target));
    assert_eq!(
        session
            .visibility
            .db_creatures
            .get(&target.raw())
            .expect("creature should stay loaded")
            .next_spline_id,
        1
    );

    session
        .character
        .active_character
        .as_mut()
        .unwrap()
        .position
        .x = -8950.0 + ATTACK_DISTANCE_YARDS;
    assert!(db_creature_can_reach_player(&session, target));

    session
        .character
        .active_character
        .as_mut()
        .unwrap()
        .position
        .x = -8950.0 + ATTACK_DISTANCE_YARDS + 0.1;
    assert!(!db_creature_can_reach_player(&session, target));
}

#[test]
fn db_creature_navigation_guardrail_blocks_aggro_and_chase_not_reach() {
    let mut creature = test_creature_spawn(6);
    creature.position_x = 0.0;
    creature.position_y = 0.0;
    creature.position_z = 0.0;
    creature.template.npc_flags = 0;
    creature.template.min_level = 1;
    let attacker = creature_spawn_guid(&creature);
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
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
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        movement: MovementSessionState {
            db_creature_navigation: DbCreatureNavigationGuardrail {
                line_of_sight_clear: false,
                path_available: true,
                world_data_files: Arc::new(WorldDataFiles {
                    data_dir: std::path::PathBuf::from("Z:/definitely-missing-cmangos-data"),
                    data_dir_for_native: std::ffi::CString::new(
                        "Z:/definitely-missing-cmangos-data",
                    )
                    .ok(),
                    maps_available: true,
                    vmaps_available: false,
                    creature_display_scales: HashMap::new(),
                    spell_cast_times: HashMap::new(),
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
                }),
            },
            ..MovementSessionState::default()
        },
        ..WorldSessionState::default()
    };
    session
        .visibility
        .db_creatures
        .insert(attacker.raw(), DbCreatureRuntime::new(creature));

    assert_eq!(select_db_creature_aggro_target(&session), None);
    assert!(
        db_creature_can_reach_player(&session, attacker),
        "CMaNGOS melee reach is distance-only; navigation guardrails gate aggro/chase ownership"
    );
    session
        .character
        .active_character
        .as_mut()
        .unwrap()
        .position
        .x = 10.0;
    assert!(
        start_db_creature_chase_motion(&mut session, attacker, player, Instant::now()).is_none(),
        "missing mmap data must not create generated aggro/chase movement"
    );

    session.movement.db_creature_navigation.path_available = false;
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
