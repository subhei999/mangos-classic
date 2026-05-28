#[test]
fn db_creature_navigation_uses_mmap_tile_availability_when_loaded() {
    let navigation = DbCreatureNavigationGuardrail {
        world_data_files: Arc::new(WorldDataFiles {
            data_dir: std::path::PathBuf::from("fixture"),
            data_dir_for_native: None,
            maps_available: true,
            vmaps_available: true,
            auction_houses: HashMap::new(),
            taxi_nodes: HashMap::new(),
            taxi_paths: HashMap::new(),
            taxi_path_nodes: HashMap::new(),
            taxi_node_mask: [0; 8],
            creature_display_scales: HashMap::new(),
            spell_cast_times: HashMap::new(),
            spell_durations: HashMap::new(),
            spell_radii: HashMap::new(),
            spell_cones: HashMap::new(),
            spell_ranges: HashMap::new(),
            skill_line_abilities_by_spell: HashMap::new(),
            skill_lines: HashMap::new(),
            skill_race_class_infos_by_skill: HashMap::new(),
            faction_templates: FactionTemplateStore::fallback_bridge(),
            item_random_properties: HashMap::new(),
            spell_item_enchantments: HashMap::new(),
            bank_bag_slot_prices: HashMap::new(),
            area_triggers: HashMap::new(),
            area_tables: AreaTableStore::default(),
            wmo_area_tables: WmoAreaTableStore::default(),
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
fn faction_template_uses_local_cmangos_dbc_when_available() {
    let data = WorldDataFiles::inspect("C:/World of Warcraft Classic");
    if !data.faction_templates.is_dbc_backed() {
        return;
    }

    assert!(
        data.faction_templates.len() > 100,
        "local FactionTemplate.dbc should load the broad Classic faction table"
    );
    assert_eq!(
        faction_reaction_to(&data.faction_templates, 22, 1),
        FactionReaction::Hostile,
        "Webwood faction 22 should aggro Alliance players from the real DBC"
    );
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
fn unit_los_query_uses_cmangos_collision_height() {
    let ground = WorldPosition::new(0, -8950.0, -130.0, 83.5, 1.25);
    let sight = unit_line_of_sight_position(ground);

    assert_eq!(sight.map_id, ground.map_id);
    assert_eq!(sight.x, ground.x);
    assert_eq!(sight.y, ground.y);
    assert_eq!(sight.orientation, ground.orientation);
    assert_eq!(sight.z, ground.z + DEFAULT_COLLISION_HEIGHT);
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
    let creature = DbCreatureRuntime::new(test_creature_spawn(6));
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

    let corner = db_creature_mmap_next_path_corner(&navigation, &creature, start, target).unwrap_or_else(|| {
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
    let creature = DbCreatureRuntime::new(test_creature_spawn(6));

    let path = db_creature_path_to_destination(
        &navigation,
        None,
        &creature,
        start,
        target,
        CreaturePathMode::Full,
    )
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
fn db_creature_mmap_path_uses_kalimdor_teldrassil_data_when_available() {
    let data = Arc::new(WorldDataFiles::inspect("C:/World of Warcraft Classic"));
    let start = WorldPosition::new(1, 10311.3, 832.463, 1326.41, 5.69632);
    let target = WorldPosition::new(1, 10321.3, 832.463, 1326.41, 5.69632);
    let Some(start_tile) = mmap_tile_for_position(start) else {
        panic!("Night Elf starter position should resolve to a mmap tile");
    };
    let Some(target_tile) = mmap_tile_for_position(target) else {
        panic!("nearby Teldrassil target should resolve to a mmap tile");
    };
    if !data.has_mmap_tile(1, start_tile.0, start_tile.1)
        || !data.has_mmap_tile(1, target_tile.0, target_tile.1)
    {
        return;
    }
    let navigation = DbCreatureNavigationGuardrail {
        world_data_files: data,
        ..DbCreatureNavigationGuardrail::default()
    };
    let creature = DbCreatureRuntime::new(test_creature_spawn(6));

    let path = db_creature_path_to_destination(
        &navigation,
        None,
        &creature,
        start,
        target,
        CreaturePathMode::Full,
    )
    .unwrap_or_else(|| {
            panic!(
                "local Teldrassil mmap should produce a Detour path; tiles={start_tile:?}->{target_tile:?}"
            )
        });

    assert!(path.flags.contains(DbCreaturePathFlags::NORMAL));
    assert!(!path.flags.contains(DbCreaturePathFlags::NOT_USING_PATH));
    assert!(path.points.iter().all(|point| point.map_id == 1));
}

#[test]
fn terrain_height_uses_local_cmangos_map_data_when_available() {
    let data = Arc::new(WorldDataFiles::inspect("C:/World of Warcraft Classic"));
    if !data.maps_available {
        return;
    }
    let geometry = WorldGeometry::new(data);
    let northshire = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let teldrassil = WorldPosition::new(1, 10311.3, 832.463, 1326.41, 5.69632);

    let northshire_ground = geometry
        .ground_position(northshire)
        .expect("local Northshire map/vmap data should produce a ground z");
    assert!(northshire_ground.z.is_finite());
    assert!(
        (northshire_ground.z - northshire.z).abs() < 25.0,
        "Northshire sampled ground should stay near the known DB/player z, got {} from {}",
        northshire_ground.z,
        northshire.z
    );

    if geometry.world_data_files.has_mmap_tile(
        1,
        mmap_tile_for_position(teldrassil).unwrap().0,
        mmap_tile_for_position(teldrassil).unwrap().1,
    ) {
        let teldrassil_ground = geometry
            .ground_position(teldrassil)
            .expect("local Teldrassil map/vmap data should produce a ground z");
        assert!(teldrassil_ground.z.is_finite());
        assert!(
            (teldrassil_ground.z - teldrassil.z).abs() < 50.0,
            "Teldrassil sampled ground should stay near the known starter z, got {} from {}",
            teldrassil_ground.z,
            teldrassil.z
        );
    }
}

#[test]
fn db_creature_path_does_not_generate_movement_when_mmap_unavailable() {
    let start = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let target = WorldPosition::new(0, -8940.0, -130.0, 83.5, 0.0);
    let creature = DbCreatureRuntime::new(test_creature_spawn(6));

    let native_missing_navigation = DbCreatureNavigationGuardrail {
        world_data_files: Arc::new(WorldDataFiles {
            data_dir: std::path::PathBuf::from("Z:/definitely-missing-cmangos-data"),
            data_dir_for_native: std::ffi::CString::new("Z:/definitely-missing-cmangos-data").ok(),
            maps_available: true,
            vmaps_available: false,
            auction_houses: HashMap::new(),
            taxi_nodes: HashMap::new(),
            taxi_paths: HashMap::new(),
            taxi_path_nodes: HashMap::new(),
            taxi_node_mask: [0; 8],
            creature_display_scales: HashMap::new(),
            spell_cast_times: HashMap::new(),
            spell_durations: HashMap::new(),
            spell_radii: HashMap::new(),
            spell_cones: HashMap::new(),
            spell_ranges: HashMap::new(),
            skill_line_abilities_by_spell: HashMap::new(),
            skill_lines: HashMap::new(),
            skill_race_class_infos_by_skill: HashMap::new(),
            faction_templates: FactionTemplateStore::fallback_bridge(),
            item_random_properties: HashMap::new(),
            spell_item_enchantments: HashMap::new(),
            bank_bag_slot_prices: HashMap::new(),
            area_triggers: HashMap::new(),
            area_tables: AreaTableStore::default(),
            wmo_area_tables: WmoAreaTableStore::default(),
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
                &creature,
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
            None,
            &creature,
            start,
            target,
            CreaturePathMode::Full,
        )
        .is_none(),
        "when MMAP data is advertised for both tiles, native query failure should not collapse to a through-geometry straight path"
    );
}

#[test]
fn db_creature_random_path_does_not_generate_movement_when_mmap_unavailable() {
    let mut spawn = test_creature_spawn(6);
    spawn.position_x = -8950.0;
    spawn.position_y = -130.0;
    spawn.position_z = 83.5;
    spawn.movement_type = DB_MOTION_TYPE_RANDOM;
    spawn.spawn_dist = 10.0;
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.next_random_move_at = Some(now);

    let native_missing_navigation = DbCreatureNavigationGuardrail {
        world_data_files: Arc::new(WorldDataFiles {
            data_dir: std::path::PathBuf::from("Z:/definitely-missing-cmangos-data"),
            data_dir_for_native: std::ffi::CString::new("Z:/definitely-missing-cmangos-data").ok(),
            maps_available: true,
            vmaps_available: false,
            auction_houses: HashMap::new(),
            taxi_nodes: HashMap::new(),
            taxi_paths: HashMap::new(),
            taxi_path_nodes: HashMap::new(),
            taxi_node_mask: [0; 8],
            creature_display_scales: HashMap::new(),
            spell_cast_times: HashMap::new(),
            spell_durations: HashMap::new(),
            spell_radii: HashMap::new(),
            spell_cones: HashMap::new(),
            spell_ranges: HashMap::new(),
            skill_line_abilities_by_spell: HashMap::new(),
            skill_lines: HashMap::new(),
            skill_race_class_infos_by_skill: HashMap::new(),
            faction_templates: FactionTemplateStore::fallback_bridge(),
            item_random_properties: HashMap::new(),
            spell_item_enchantments: HashMap::new(),
            bank_bag_slot_prices: HashMap::new(),
            area_triggers: HashMap::new(),
            area_tables: AreaTableStore::default(),
            wmo_area_tables: WmoAreaTableStore::default(),
            mmap_headers: HashSet::from([0]),
            mmap_tiles: HashSet::from([(0, 48, 32)]),
            vmap_trees: HashSet::new(),
            vmap_tiles: HashSet::new(),
        }),
        ..DbCreatureNavigationGuardrail::default()
    };

    assert!(
        start_db_creature_random_motion_runtime(
            &native_missing_navigation,
            None,
            &mut runtime,
            now,
        )
        .is_none(),
        "advertised-but-unloadable mmap data must not collapse random wander to a fake straight path"
    );
    assert_eq!(
        runtime.next_random_move_at,
        Some(now + Duration::from_millis(DB_CREATURE_IDLE_MOTION_FAILED_RETRY_MILLIS))
    );
    assert!(matches!(runtime.motion, CreatureMotionState::Idle));
}

#[test]
fn db_creature_random_mmap_path_uses_local_detour_data_when_available() {
    let data = Arc::new(WorldDataFiles::inspect("C:/World of Warcraft Classic"));
    let home = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let Some(start_tile) = mmap_tile_for_position(home) else {
        panic!("Northshire home should resolve to a mmap tile");
    };
    if !data.has_mmap_tile(0, start_tile.0, start_tile.1) {
        return;
    }
    let Some(data_dir) = data.data_dir_for_native.as_ref() else {
        return;
    };

    let native_path = [
        (0.0, 0.25),
        (0.125, 0.25),
        (0.25, 0.25),
        (0.375, 0.25),
        (0.5, 0.25),
        (0.625, 0.25),
        (0.75, 0.25),
        (0.875, 0.25),
        (0.0, 0.5),
        (0.125, 0.5),
        (0.25, 0.5),
        (0.375, 0.5),
        (0.5, 0.5),
        (0.625, 0.5),
        (0.75, 0.5),
        (0.875, 0.5),
    ]
    .into_iter()
    .map(|(angle_seed, range_seed)| {
        native_mmap_find_random_path(
            data_dir,
            NativeMmapRandomPathRequest {
                center: home,
                start: home,
                start_tile,
                radius: 10.0,
                angle_seed,
                range_seed,
                filter: NativeMmapPathFilter::ground(),
            },
        )
    })
    .find(|path| {
        matches!(
            path.status,
            NativeMmapPathStatus::Normal | NativeMmapPathStatus::Incomplete
        )
    })
    .unwrap_or_else(|| {
        native_mmap_find_random_path(
            data_dir,
            NativeMmapRandomPathRequest {
                center: home,
                start: home,
                start_tile,
                radius: 10.0,
                angle_seed: 0.0,
                range_seed: 0.25,
                filter: NativeMmapPathFilter::ground(),
            },
        )
    });

    assert!(
        matches!(
            native_path.status,
            NativeMmapPathStatus::Normal | NativeMmapPathStatus::Incomplete
        ),
        "local Northshire mmap should produce a random Detour path, got {:?}",
        native_path.status
    );
    assert!(native_path.points.len() >= 2);
    assert!(native_path.points.iter().all(|point| point.map_id == 0));
    let destination = native_path.points.last().unwrap();
    assert!(
        distance_2d(home.x, home.y, destination.x, destination.y) <= 10.5,
        "random destination should stay inside the DB wander radius with a small smoothing tolerance"
    );
    assert!(
        native_path.points.iter().all(|point| point.z.is_finite()),
        "native random path should return grounded finite z values"
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

    let now = Instant::now();
    runtime.respawn(now);
    assert_eq!(runtime.current_position.x, runtime.home_position.x);
    assert_eq!(runtime.current_position.y, runtime.home_position.y);
    assert!(matches!(runtime.motion, CreatureMotionState::Idle));
}

#[test]
fn db_creature_respawn_delays_sight_aggro_like_cmangos() {
    let mut spawn = test_creature_spawn(38);
    spawn.template.faction = 17;
    spawn.template.npc_flags = 0;
    spawn.template.creature_type = 7;
    let mut runtime = DbCreatureRuntime::new(spawn);
    let now = Instant::now();
    let character = ActiveCharacter {
        guid: 7,
        name: "Ada".to_string(),
        race: 1,
        class: 1,
        level: 1,
        xp: 0,
        position: runtime.current_position,
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
        jump: JumpInfo::default(),
    };
    let faction_templates = FactionTemplateStore::fallback_bridge();

    runtime.respawn(now);

    assert!(!runtime.can_aggro_player(&faction_templates, &character, now));
    assert!(runtime.can_aggro_player(
        &faction_templates,
        &character,
        now + CMANGOS_CREATURE_RESPAWN_AGGRO_DELAY
    ));
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
        Some(now + Duration::from_secs(6))
    );
    assert!(!runtime.is_corpse_expired(now + Duration::from_secs(5)));
    assert!(runtime.is_corpse_expired(now + Duration::from_secs(6)));
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
    runtime.respawn(now + Duration::from_secs(3));

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
                    DB_CREATURE_LEASH_RADIUS_YARDS + 5.0,
                    0.0,
                    0.0,
                    0.0,
                ),
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        combat: CombatSessionState {
            active_combat_target: Some(attacker),
            active_combat_next_swing_at: Some(now),
            ..CombatSessionState::default()
        },
        ..WorldSessionState::default()
    };
    session.combat.active_creature_combats.insert(
        attacker.raw(),
        CreatureCombatState {
            attacker,
            victim: ObjectGuid::new(HighGuid::Player, 0, 7),
            started_at: now,
            next_swing_at: now,
        },
    );
    session
        .visibility
        .db_creatures
        .insert(attacker.raw(), runtime);

    assert!(db_creature_should_evade(&session, attacker));
    prepare_db_creature_evade(&mut session, attacker);
    let motion = start_db_creature_return_home_motion(&mut session, attacker, now)
        .expect("leashed creature should run home");

    let destination = *motion.path.last().unwrap();
    assert_eq!(destination.x, 0.0);
    assert_eq!(destination.y, 0.0);
    assert_eq!(motion.spline_id, 0);
    assert!(motion.duration > Duration::ZERO);
    assert!(session.combat.active_combat_target.is_none());
    assert!(session.combat.active_combat_next_swing_at.is_none());
    assert!(session.combat.active_creature_combats.is_empty());
    let runtime = session
        .visibility
        .db_creatures
        .get(&attacker.raw())
        .unwrap();
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
    session
        .visibility
        .db_creatures
        .insert(attacker.raw(), runtime);

    let motion = start_db_creature_return_home_motion(&mut session, attacker, now)
        .expect("away creature should start return-home motion");
    let half_duration = Duration::from_millis((motion.duration.as_millis() as u64 / 2).max(1));
    advance_db_creature_motion(&mut session, attacker, now + half_duration);
    let mid_x = session
        .visibility
        .db_creatures
        .get(&attacker.raw())
        .expect("creature should still be loaded")
        .current_position
        .x;
    assert!(mid_x < motion.start.x);
    let destination = *motion.path.last().unwrap();
    assert!(mid_x > destination.x);

    advance_db_creature_motion(&mut session, attacker, now + motion.duration);
    let runtime = session
        .visibility
        .db_creatures
        .get(&attacker.raw())
        .unwrap();
    assert_eq!(runtime.current_position.x, runtime.home_position.x);
    assert_eq!(runtime.current_position.y, runtime.home_position.y);
    assert!(matches!(runtime.motion, CreatureMotionState::Idle));
}

#[test]
fn waypoint_creature_return_home_resumes_patrol_after_evade() {
    let mut spawn = test_creature_spawn(6);
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    spawn.movement_type = DB_MOTION_TYPE_WAYPOINT;
    spawn.template.movement_type = DB_MOTION_TYPE_WAYPOINT;
    spawn.waypoint_path = vec![
        test_waypoint(1, 10.0, 0.0, 0),
        test_waypoint(2, 20.0, 0.0, 0),
    ];
    let attacker = creature_spawn_guid(&spawn);
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.current_position = WorldPosition::new(0, 4.0, 0.0, 0.0, 0.0);
    runtime.next_waypoint_move_at = Some(now);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position: WorldPosition::new(0, 30.0, 0.0, 0.0, 0.0),
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
        .insert(attacker.raw(), runtime);

    start_db_creature_chase_motion(&mut session, attacker, player, now)
        .expect("waypoint creature should chase");
    let runtime = session
        .visibility
        .db_creatures
        .get_mut(&attacker.raw())
        .unwrap();
    runtime.current_position = WorldPosition::new(0, 16.0, 0.0, 0.0, 0.0);
    let motion = start_db_creature_return_home_motion(&mut session, attacker, now)
        .expect("waypoint creature should return to patrol reset point");
    let destination = *motion.path.last().unwrap();
    assert_eq!(destination.x, 4.0);

    advance_db_creature_motion(&mut session, attacker, now + motion.duration);
    let runtime = session
        .visibility
        .db_creatures
        .get(&attacker.raw())
        .unwrap();
    assert_eq!(runtime.current_position.x, 4.0);
    assert!(matches!(runtime.motion, CreatureMotionState::Idle));
    assert_eq!(runtime.next_waypoint_move_at, Some(now + motion.duration));
    assert_eq!(runtime.next_random_move_at, None);
    assert_eq!(runtime.waypoint_next_index, 0);
    assert!(runtime.waypoint_resume_position.is_none());
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
        character: CharacterSessionState {
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
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    session
        .visibility
        .db_creatures
        .insert(attacker.raw(), runtime);

    let (creature, motion) = maps
        .start_db_creature_return_home_motion(
            0,
            &session.movement.db_creature_navigation,
            attacker,
            now,
        )
        .await
        .expect("away creature should start return-home motion");
    session
        .visibility
        .db_creatures
        .insert(attacker.raw(), creature);
    assert!(session.combat.active_creature_combats.is_empty());

    advance_db_creature_return_home_motions(shared_world, &mut session, now + motion.duration)
        .await;
    let runtime = session
        .visibility
        .db_creatures
        .get(&attacker.raw())
        .unwrap();
    assert_eq!(runtime.current_position.x, runtime.home_position.x);
    assert!(matches!(runtime.motion, CreatureMotionState::Idle));
}

#[tokio::test]
async fn map_runtime_tick_advances_return_home_motion_without_session_polling() {
    let now = Instant::now();
    let maps = Arc::new(MapRuntimeManager::default());
    let player_position = WorldPosition::new(0, 9.0, 0.0, 0.0, 0.0);
    maps.add_player(test_player_runtime(7, SessionId(7), player_position))
        .await
        .expect("player should activate creature grid");

    let mut spawn = test_creature_spawn(6);
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let attacker = creature_spawn_guid(&spawn);
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.current_position.x = 9.0;
    maps.share_db_creature_snapshots(0, vec![runtime]).await;

    let (_, motion) = maps
        .start_db_creature_return_home_motion(
            0,
            &DbCreatureNavigationGuardrail::default(),
            attacker,
            now,
        )
        .await
        .expect("away creature should start return-home motion");

    maps.advance_all_active_db_creature_idle_motions_with_interval(
        &DbCreatureNavigationGuardrail::default(),
        now + motion.duration,
        Duration::from_millis(100),
    )
    .await
    .expect("map motion tick should advance return-home creature");

    let runtime = maps
        .db_creature_snapshot(0, attacker)
        .await
        .expect("creature should remain in map");
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
        movement: MovementSessionState {
            db_creature_navigation: DbCreatureNavigationGuardrail {
                line_of_sight_clear: false,
                path_available: false,
                ..DbCreatureNavigationGuardrail::default()
            },
            ..MovementSessionState::default()
        },
        ..WorldSessionState::default()
    };
    session
        .visibility
        .db_creatures
        .insert(attacker.raw(), runtime);

    assert!(start_db_creature_return_home_motion(&mut session, attacker, Instant::now()).is_some());
    assert!(matches!(
        session
            .visibility
            .db_creatures
            .get(&attacker.raw())
            .unwrap()
            .motion,
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
        character: CharacterSessionState {
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
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    session
        .visibility
        .db_creatures
        .insert(attacker.raw(), runtime);

    assert_eq!(select_db_creature_aggro_target(&session), None);
    assert_eq!(apply_db_creature_damage(&mut session, attacker, 1), None);
    assert_eq!(
        session
            .visibility
            .db_creatures
            .get(&attacker.raw())
            .unwrap()
            .health,
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
    session
        .visibility
        .db_creatures
        .insert(creature_guid.raw(), runtime);

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
    let runtime = session
        .visibility
        .db_creatures
        .get(&creature_guid.raw())
        .unwrap();
    assert!(matches!(runtime.motion, CreatureMotionState::Random(_)));
    assert_eq!(runtime.next_spline_id, 1);

    advance_db_creature_motion(&mut session, creature_guid, now + motion.duration);
    let runtime = session
        .visibility
        .db_creatures
        .get(&creature_guid.raw())
        .unwrap();
    assert!(matches!(runtime.motion, CreatureMotionState::Idle));
    assert!(runtime.next_random_move_at.is_some());
}

#[test]
fn db_creature_confused_motion_uses_dedicated_state_and_pause_timer() {
    let mut spawn = test_creature_spawn(6);
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.active_auras.push(ActiveAura {
        spell_id: 118,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 1,
        interrupt_flags: AURA_INTERRUPT_FLAG_DAMAGE,
        visible: true,
        positive: false,
        duration_millis: Some(8_000),
        expires_at: Some(now + Duration::from_secs(8)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::Confuse],
        proc_triggers: Vec::new(),
    });
    runtime.begin_confused_motion(now);
    let original_waypoint_due = runtime.next_waypoint_move_at;

    let motion = start_db_creature_confused_motion_runtime(
        &DbCreatureNavigationGuardrail::default(),
        None,
        &mut runtime,
        now,
    )
    .expect("confused creature should start a sheep wander spline");

    assert!(matches!(runtime.motion, CreatureMotionState::Confused(_)));
    assert_eq!(runtime.next_confused_move_at, None);

    advance_db_creature_motion_runtime(&mut runtime, now + motion.duration);

    assert!(matches!(runtime.motion, CreatureMotionState::Idle));
    assert!(runtime.next_confused_move_at.is_some());
    assert_eq!(runtime.next_waypoint_move_at, original_waypoint_due);
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
    session
        .visibility
        .db_creatures
        .insert(creature_guid.raw(), runtime);

    let motion = start_db_creature_random_motion(&mut session, creature_guid, now)
        .expect("random movement creature should start a wander spline");
    let distance = path_distance_2d(motion.start, &motion.path);
    let expected_millis = ((distance / (DB_CREATURE_WALK_SPEED_YARDS_PER_SEC * 2.0)) * 1000.0)
        .ceil()
        .max(1.0) as u64;
    assert_eq!(motion.duration, Duration::from_millis(expected_millis));
}

#[test]
fn db_creature_random_motion_is_blocked_by_root() {
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
    runtime.active_auras.push(ActiveAura {
        spell_id: 122,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 1,
        interrupt_flags: 0,
        visible: true,
        positive: false,
        duration_millis: Some(8_000),
        expires_at: Some(now + Duration::from_secs(8)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::Root],
        proc_triggers: Vec::new(),
    });
    let mut session = WorldSessionState::default();
    session
        .visibility
        .db_creatures
        .insert(creature_guid.raw(), runtime);

    assert!(start_db_creature_random_motion(&mut session, creature_guid, now).is_none());
    let runtime = session
        .visibility
        .db_creatures
        .get(&creature_guid.raw())
        .unwrap();
    assert!(matches!(runtime.motion, CreatureMotionState::Idle));
    assert_eq!(runtime.next_random_move_at, Some(now));
}

#[test]
fn db_creature_chase_motion_duration_applies_temporary_run_speed_slow() {
    let mut spawn = test_creature_spawn(6);
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let creature_guid = creature_spawn_guid(&spawn);
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.active_auras.push(ActiveAura {
        spell_id: 6136,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 1,
        interrupt_flags: 0,
        visible: true,
        positive: false,
        duration_millis: Some(8_000),
        expires_at: Some(now + Duration::from_secs(8)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::MoveSpeedPercent { percent: -30 }],
        proc_triggers: Vec::new(),
    });
    runtime.refresh_move_speeds();
    let mut session = WorldSessionState {
        character: CharacterSessionState {
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
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    session
        .visibility
        .db_creatures
        .insert(creature_guid.raw(), runtime);

    let motion = start_db_creature_chase_motion(&mut session, creature_guid, player, now)
        .expect("out-of-range slowed creature should start chase motion");
    let distance = path_distance_2d(motion.start, &motion.path);
    let expected_millis = ((distance / (DB_CREATURE_RUN_SPEED_YARDS_PER_SEC * 0.7)) * 1000.0)
        .ceil()
        .max(1.0) as u64;
    assert_eq!(motion.duration, Duration::from_millis(expected_millis));
}

#[test]
fn db_creature_chase_motion_duration_applies_cmangos_wounded_slowdown() {
    let mut spawn = test_creature_spawn(299);
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let creature_guid = creature_spawn_guid(&spawn);
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.health = ((runtime.max_health() as f32) * 0.20).round() as u32;
    let expected_speed = runtime.targeted_motion_speed(true);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
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
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    session
        .visibility
        .db_creatures
        .insert(creature_guid.raw(), runtime);

    let motion = start_db_creature_chase_motion(&mut session, creature_guid, player, now)
        .expect("wounded creature should start chase motion");
    let distance = path_distance_2d(motion.start, &motion.path);
    let expected_millis = ((distance / expected_speed) * 1000.0).ceil().max(1.0) as u64;
    assert_eq!(motion.duration, Duration::from_millis(expected_millis));
    assert!(expected_speed < DB_CREATURE_RUN_SPEED_YARDS_PER_SEC);
}

#[test]
fn db_creature_wounded_slowdown_honors_static_flag_opt_out() {
    let mut spawn = test_creature_spawn(299);
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    spawn.template.static_flags2 = CREATURE_STATIC_FLAGS2_NO_WOUNDED_SLOWDOWN;
    let creature_guid = creature_spawn_guid(&spawn);
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.health = ((runtime.max_health() as f32) * 0.20).round() as u32;
    assert_eq!(runtime.wounded_combat_speed_multiplier(), 1.0);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
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
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    session
        .visibility
        .db_creatures
        .insert(creature_guid.raw(), runtime);

    let motion = start_db_creature_chase_motion(&mut session, creature_guid, player, now)
        .expect("flagged creature should start chase motion");
    let distance = path_distance_2d(motion.start, &motion.path);
    let expected_millis = ((distance / DB_CREATURE_RUN_SPEED_YARDS_PER_SEC) * 1000.0)
        .ceil()
        .max(1.0) as u64;
    assert_eq!(motion.duration, Duration::from_millis(expected_millis));
}

#[test]
fn db_creature_damage_crossing_wounded_threshold_retimes_active_chase() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let player_position = WorldPosition::new(0, 20.0, 0.0, 0.0, 0.0);
    map.add_player(test_player_runtime(7, SessionId(7), player_position))
        .unwrap();
    let mut spawn = test_creature_spawn(299);
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let creature_guid = creature_spawn_guid(&spawn);
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.health = ((runtime.max_health() as f32) * 0.40).round() as u32;
    map.creatures.insert(creature_guid.raw(), runtime);
    let (_, original_motion) = map
        .start_db_creature_chase_motion(
            &DbCreatureNavigationGuardrail::default(),
            creature_guid,
            player_guid,
            player_position,
            now,
        )
        .expect("healthy creature should start normal chase");
    let max_health = map
        .creatures
        .get(&creature_guid.raw())
        .unwrap()
        .max_health();
    let damage = ((max_health as f32) * 0.20).ceil() as u32 + 1;

    let event = map
        .apply_db_creature_damage(DbCreatureDamageRequest {
            creature_guid,
            killer: player_guid,
            damage,
            melee_outcome: Some(MeleeDamageOutcome::normal_hit(damage)),
            spell_damage_outcome: None,
            spell_id: None,
            spell_school: 0,
            suppress_attacker_state: false,
            now: now + Duration::from_millis(100),
            now_epoch_secs: 1,
            exclude_character_guid: Some(7),
            corpse_loot: None,
        })
        .unwrap()
        .expect("damage should apply");

    assert_eq!(event.direct_packets.len(), 1);
    assert_eq!(
        event.direct_packets[0].opcode,
        WorldOpcode::SmsgMonsterMove as u16
    );
    let CreatureMotionState::Chase(chase) =
        &map.creatures.get(&creature_guid.raw()).unwrap().motion
    else {
        panic!("creature should remain in chase motion");
    };
    assert!(chase.duration > original_motion.duration);
}

#[test]
fn db_creature_slow_aura_retimes_active_chase_and_adjusts_swing_timer() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let player_position = WorldPosition::new(0, 10.0, 0.0, 0.0, 0.0);
    map.add_player(test_player_runtime(7, SessionId(7), player_position))
        .unwrap();
    let mut spawn = test_creature_spawn(6);
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    spawn.template.melee_base_attack_time = 2000;
    let creature_guid = creature_spawn_guid(&spawn);
    map.creatures
        .insert(creature_guid.raw(), DbCreatureRuntime::new(spawn));
    map.active_creature_combats.insert(
        creature_guid.raw(),
        CreatureCombatState {
            attacker: creature_guid,
            victim: player_guid,
            started_at: now,
            next_swing_at: now + Duration::from_millis(2000),
        },
    );
    let (_, motion) = map
        .start_db_creature_chase_motion(
            &DbCreatureNavigationGuardrail::default(),
            creature_guid,
            player_guid,
            player_position,
            now,
        )
        .expect("creature should start a chase before being chilled");
    let half_duration = Duration::from_millis((motion.duration.as_millis() as u64 / 2).max(1));
    let aura = ActiveAura {
        spell_id: 6136,
        caster: player_guid,
        level: 1,
        interrupt_flags: 0,
        positive: false,
        visible: true,
        duration_millis: Some(8_000),
        expires_at: Some(now + Duration::from_secs(8)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![
            AuraStatModifier::MeleeAttackTimePercent { percent: -25 },
            AuraStatModifier::MoveSpeedPercent { percent: -30 },
        ],
        proc_triggers: Vec::new(),
    };

    let event = map
        .apply_db_creature_aura(creature_guid, 7, aura, now + half_duration)
        .unwrap()
        .unwrap();

    let opcodes = event
        .direct_packets
        .iter()
        .map(|packet| packet.opcode)
        .collect::<Vec<_>>();
    assert!(opcodes.contains(&(WorldOpcode::SmsgSplineSetRunSpeed as u16)));
    assert!(opcodes.contains(&(WorldOpcode::SmsgSplineSetRunBackSpeed as u16)));
    assert!(opcodes.contains(&(WorldOpcode::SmsgSplineSetSwimSpeed as u16)));
    assert!(!opcodes.contains(&(WorldOpcode::SmsgSplineSetWalkSpeed as u16)));
    assert!(opcodes.contains(&(WorldOpcode::SmsgMonsterMove as u16)));
    let creature = map.creatures.get(&creature_guid.raw()).unwrap();
    assert_eq!(
        creature.run_speed(),
        DB_CREATURE_RUN_SPEED_YARDS_PER_SEC * 0.7
    );
    let CreatureMotionState::Chase(chase) = &creature.motion else {
        panic!("speed change should keep active chase motion");
    };
    assert!(chase.start.x > motion.start.x);
    assert!(chase.duration > Duration::from_millis(1));
    assert_eq!(
        map.active_creature_combats
            .get(&creature_guid.raw())
            .unwrap()
            .next_swing_at,
        now + Duration::from_millis(2500)
    );
}

#[test]
fn db_creature_slow_aura_expiration_restores_speed_and_attack_timer() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let player_position = WorldPosition::new(0, 10.0, 0.0, 0.0, 0.0);
    map.add_player(test_player_runtime(7, SessionId(7), player_position))
        .unwrap();
    let mut spawn = test_creature_spawn(6);
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    spawn.template.melee_base_attack_time = 2000;
    let creature_guid = creature_spawn_guid(&spawn);
    let mut creature = DbCreatureRuntime::new(spawn);
    creature.active_auras.push(ActiveAura {
        spell_id: 6136,
        caster: player_guid,
        level: 1,
        interrupt_flags: 0,
        positive: false,
        visible: true,
        duration_millis: Some(1_000),
        expires_at: Some(now + Duration::from_secs(1)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![
            AuraStatModifier::MeleeAttackTimePercent { percent: -25 },
            AuraStatModifier::MoveSpeedPercent { percent: -30 },
        ],
        proc_triggers: Vec::new(),
    });
    creature.refresh_move_speeds();
    map.creatures.insert(creature_guid.raw(), creature);
    map.active_creature_combats.insert(
        creature_guid.raw(),
        CreatureCombatState {
            attacker: creature_guid,
            victim: player_guid,
            started_at: now,
            next_swing_at: now + Duration::from_millis(2500),
        },
    );
    map.start_db_creature_chase_motion(
        &DbCreatureNavigationGuardrail::default(),
        creature_guid,
        player_guid,
        player_position,
        now,
    )
    .expect("slowed creature should start a chase before expiration");

    let packets = map
        .advance_db_creature_auras(now + Duration::from_secs(1), 0)
        .unwrap();

    let opcodes = packets
        .iter()
        .map(|(_, packet)| packet.opcode)
        .collect::<Vec<_>>();
    assert!(opcodes.contains(&(WorldOpcode::SmsgSplineSetRunSpeed as u16)));
    assert!(opcodes.contains(&(WorldOpcode::SmsgSplineSetRunBackSpeed as u16)));
    assert!(opcodes.contains(&(WorldOpcode::SmsgSplineSetSwimSpeed as u16)));
    assert!(opcodes.contains(&(WorldOpcode::SmsgMonsterMove as u16)));
    let creature = map.creatures.get(&creature_guid.raw()).unwrap();
    assert_eq!(creature.run_speed(), DB_CREATURE_RUN_SPEED_YARDS_PER_SEC);
    assert!(creature.active_auras.is_empty());
    assert_eq!(
        map.active_creature_combats
            .get(&creature_guid.raw())
            .unwrap()
            .next_swing_at,
        now + Duration::from_millis(2000)
    );
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
        .visibility
        .db_creatures
        .insert(idle_guid.raw(), DbCreatureRuntime::new(idle_spawn));
    session.visibility.db_creatures.insert(
        zero_radius_guid.raw(),
        DbCreatureRuntime::new(zero_radius_spawn),
    );

    assert!(start_db_creature_random_motion(&mut session, idle_guid, now).is_none());
    assert!(start_db_creature_random_motion(&mut session, zero_radius_guid, now).is_none());
}

#[test]
fn db_creature_idle_motion_start_guids_return_all_ready_creatures() {
    let now = Instant::now();
    let mut session = WorldSessionState::default();
    let ready_creatures = 7;
    for index in 0..ready_creatures {
        let mut spawn = test_creature_spawn(6);
        spawn.guid = 1_000 + index as u32;
        spawn.movement_type = DB_MOTION_TYPE_RANDOM;
        spawn.spawn_dist = 5.0;
        let guid = creature_spawn_guid(&spawn);
        let mut runtime = DbCreatureRuntime::new(spawn);
        runtime.next_random_move_at = Some(now);
        session.visibility.db_creatures.insert(guid.raw(), runtime);
    }

    let start_guids = db_creature_idle_motion_start_guids(&session, now);

    assert_eq!(start_guids.len(), ready_creatures);
    assert!(start_guids.windows(2).all(|window| window[0] < window[1]));
}

#[test]
fn db_creature_random_motion_failed_path_defers_retry() {
    let mut spawn = test_creature_spawn(6);
    spawn.movement_type = DB_MOTION_TYPE_RANDOM;
    spawn.spawn_dist = 5.0;
    let creature_guid = creature_spawn_guid(&spawn);
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.current_position.map_id = 1;
    runtime.next_random_move_at = Some(now);
    let mut session = WorldSessionState::default();
    session
        .visibility
        .db_creatures
        .insert(creature_guid.raw(), runtime);

    assert!(start_db_creature_random_motion(&mut session, creature_guid, now).is_none());
    let runtime = session
        .visibility
        .db_creatures
        .get(&creature_guid.raw())
        .unwrap();
    assert_eq!(
        runtime.next_random_move_at,
        Some(now + Duration::from_millis(DB_CREATURE_IDLE_MOTION_FAILED_RETRY_MILLIS))
    );
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
                character_name: Some("Player1".to_string()),
                outbound: WorldPacketSender::Unbounded(direct_tx),
                disconnect: None,
            },
        )
        .await;
    sessions
        .register(
            SessionId(2),
            SessionHandle {
                account_id: 2,
                character_guid: Some(2),
                character_name: Some("Player2".to_string()),
                outbound: WorldPacketSender::Unbounded(observer_tx),
                disconnect: None,
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

    assert_eq!(
        direct_rx.try_recv().unwrap().opcode,
        WorldOpcode::SmsgMonsterMove as u16
    );
    assert_eq!(
        observer_rx.try_recv().unwrap().opcode,
        WorldOpcode::SmsgMonsterMove as u16
    );
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
                character_name: Some("Player1".to_string()),
                outbound: WorldPacketSender::Unbounded(direct_tx),
                disconnect: None,
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

    assert_eq!(
        direct_rx.try_recv().unwrap().opcode,
        WorldOpcode::SmsgUpdateObject as u16
    );
    assert_eq!(
        direct_rx.try_recv().unwrap().opcode,
        WorldOpcode::SmsgMonsterMove as u16
    );
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
    session
        .visibility
        .db_creatures
        .insert(creature_guid.raw(), runtime);

    let motion = start_db_creature_waypoint_motion(&mut session, creature_guid, now)
        .expect("waypoint creature should start a DB path spline");

    assert_eq!(motion.spline_id, 0);
    assert_eq!(motion.start.x, 0.0);
    assert_eq!(motion.path.last().unwrap().x, 5.0);
    let runtime = session
        .visibility
        .db_creatures
        .get(&creature_guid.raw())
        .unwrap();
    assert!(matches!(runtime.motion, CreatureMotionState::Waypoint(_)));
    assert_eq!(runtime.next_spline_id, 1);
    assert_eq!(runtime.next_waypoint_move_at, None);

    advance_db_creature_motion(&mut session, creature_guid, now + motion.duration);
    let runtime = session
        .visibility
        .db_creatures
        .get(&creature_guid.raw())
        .unwrap();
    assert!(matches!(runtime.motion, CreatureMotionState::Idle));
    assert_eq!(runtime.current_position.x, 5.0);
    assert_eq!(runtime.waypoint_next_index, 0);
    assert!(runtime
        .next_waypoint_move_at
        .is_some_and(|at| at == now + motion.duration + Duration::from_millis(250)));
}

#[test]
fn db_creature_waypoint_motion_advances_zero_distance_node_with_wait() {
    let mut spawn = test_creature_spawn(6);
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 83.5;
    spawn.movement_type = DB_MOTION_TYPE_WAYPOINT;
    spawn.waypoint_path = vec![
        test_waypoint(1, 0.0, 0.0, 60_000),
        test_waypoint(2, 5.0, 0.0, 1),
    ];
    let creature_guid = creature_spawn_guid(&spawn);
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.next_waypoint_move_at = Some(now);
    let mut session = WorldSessionState::default();
    session
        .visibility
        .db_creatures
        .insert(creature_guid.raw(), runtime);

    assert!(start_db_creature_waypoint_motion(&mut session, creature_guid, now).is_none());
    let runtime = session
        .visibility
        .db_creatures
        .get(&creature_guid.raw())
        .unwrap();
    assert!(matches!(runtime.motion, CreatureMotionState::Idle));
    assert_eq!(runtime.current_position.x, 0.0);
    assert_eq!(runtime.waypoint_next_index, 1);
    assert_eq!(
        runtime.next_waypoint_move_at,
        Some(now + Duration::from_millis(60_000))
    );

    assert!(start_db_creature_waypoint_motion(
        &mut session,
        creature_guid,
        now + Duration::from_millis(59_999),
    )
    .is_none());
    let motion = start_db_creature_waypoint_motion(
        &mut session,
        creature_guid,
        now + Duration::from_millis(60_000),
    )
    .expect("creature should move to the next waypoint after the node wait");
    assert_eq!(motion.path.last().unwrap().x, 5.0);
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
    session
        .visibility
        .db_creatures
        .insert(creature_guid.raw(), runtime);

    let first = start_db_creature_waypoint_motion(&mut session, creature_guid, now).unwrap();
    assert_eq!(first.path.len(), 3);
    assert_eq!(first.path.last().unwrap().x, 15.0);
    advance_db_creature_motion(&mut session, creature_guid, now + first.duration);
    let runtime = session
        .visibility
        .db_creatures
        .get(&creature_guid.raw())
        .unwrap();
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
        jump: JumpInfo::default(),
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
        character: CharacterSessionState {
            active_character: Some(character),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    session
        .visibility
        .db_creatures
        .insert(caller.raw(), DbCreatureRuntime::new(caller_spawn));
    session
        .visibility
        .db_creatures
        .insert(helper.raw(), DbCreatureRuntime::new(helper_spawn));
    let far_runtime = DbCreatureRuntime::new(far_spawn);
    session
        .visibility
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
        character: CharacterSessionState {
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
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    session
        .visibility
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
        .visibility
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
        .visibility
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
        .visibility
        .db_creatures
        .get(&attacker.raw())
        .expect("creature should still be loaded");
    assert_eq!(runtime.current_position.x, destination.x);
    assert_eq!(runtime.spawn.position_x, 0.0);
    let CreatureMotionState::Chase(chase) = &runtime.motion else {
        panic!("arrived creature should keep its chase generator for repath timing");
    };
    assert!(db_creature_chase_motion_arrived(runtime, chase));
    assert!(db_creature_can_reach_player(&session, attacker));
}

#[test]
fn db_creature_melee_reach_ignores_los_and_path_like_cmangos() {
    let mut creature = test_creature_spawn(6);
    creature.position_x = 0.0;
    creature.position_y = 0.0;
    creature.position_z = 0.0;
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
                position: WorldPosition::new(0, 2.0, 0.0, 0.0, 0.0),
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
                path_available: false,
                ..DbCreatureNavigationGuardrail::default()
            },
            ..MovementSessionState::default()
        },
        ..WorldSessionState::default()
    };
    session
        .visibility
        .db_creatures
        .insert(attacker.raw(), DbCreatureRuntime::new(creature));

    assert!(
        db_creature_can_reach_player(&session, attacker),
        "CMaNGOS Unit::CanReachWithMeleeAttack is a reach check, not a path/LOS check"
    );

    let mut map = MapRuntime::new(0, 0);
    map.share_db_creature_snapshots(
        session
            .visibility
            .db_creatures
            .values()
            .cloned()
            .collect::<Vec<_>>(),
    );
    map.players.insert(
        7,
        test_player_runtime(7, SessionId(7), WorldPosition::new(0, 2.0, 0.0, 0.0, 0.0)),
    );

    assert!(map.db_creature_can_reach_player_with_navigation(
        attacker,
        player,
        &session.movement.db_creature_navigation
    ));
}

#[test]
fn db_creature_moving_melee_leeway_matches_cmangos_backpedal_boundary() {
    let mut creature = test_creature_spawn(6);
    creature.position_x = 0.0;
    creature.position_y = 0.0;
    creature.position_z = 0.0;
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
                position: WorldPosition::new(0, ATTACK_DISTANCE_YARDS + 1.0, 0.0, 0.0, 0.0),
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
        .insert(attacker.raw(), DbCreatureRuntime::new(creature));

    assert!(!db_creature_can_reach_player(&session, attacker));
    start_db_creature_chase_motion(&mut session, attacker, player, Instant::now())
        .expect("out-of-range creature should start chasing");
    assert!(
        !db_creature_can_reach_player(&session, attacker),
        "CMaNGOS leeway requires both units to be moving"
    );

    session
        .character
        .active_character
        .as_mut()
        .unwrap()
        .movement_flags = MOVEFLAG_BACKWARD;
    assert!(
        db_creature_can_reach_player(&session, attacker),
        "S-key backpedal is moving, not walk-mode, so a moving creature gets melee leeway"
    );

    session
        .character
        .active_character
        .as_mut()
        .unwrap()
        .movement_flags = MOVEFLAG_BACKWARD | MOVEFLAG_WALK_MODE;
    assert!(
        !db_creature_can_reach_player(&session, attacker),
        "walk-mode movement suppresses CMaNGOS melee leeway"
    );

    let creature_snapshot = session
        .visibility
        .db_creatures
        .get(&attacker.raw())
        .expect("creature should still be loaded")
        .clone();
    let mut map = MapRuntime::new(0, 0);
    map.share_db_creature_snapshots(vec![creature_snapshot]);
    let mut player_runtime = test_player_runtime(
        7,
        SessionId(7),
        WorldPosition::new(0, ATTACK_DISTANCE_YARDS + 1.0, 0.0, 0.0, 0.0),
    );
    player_runtime.movement_flags = MOVEFLAG_BACKWARD;
    map.players.insert(7, player_runtime);

    assert!(map.db_creature_can_reach_player_with_navigation(
        attacker,
        player,
        &DbCreatureNavigationGuardrail::default()
    ));
    map.players.get_mut(&7).unwrap().movement_flags = MOVEFLAG_BACKWARD | MOVEFLAG_WALK_MODE;
    assert!(!map.db_creature_can_reach_player_with_navigation(
        attacker,
        player,
        &DbCreatureNavigationGuardrail::default()
    ));
}

#[test]
fn walking_patrol_does_not_grant_moving_melee_leeway() {
    let mut creature = test_creature_spawn(6);
    creature.position_x = 0.0;
    creature.position_y = 0.0;
    creature.position_z = 0.0;
    let attacker = creature_spawn_guid(&creature);
    let mut runtime = DbCreatureRuntime::new(creature);
    runtime.default_movement_run = false;
    runtime.motion = CreatureMotionState::Waypoint(CreatureWaypointMotion {
        node_index: 0,
        start: WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0),
        destination: WorldPosition::new(0, 2.0, 0.0, 0.0, 0.0),
        path: vec![
            WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0),
            WorldPosition::new(0, 2.0, 0.0, 0.0, 0.0),
        ],
        started_at: Instant::now(),
        duration: Duration::from_secs(1),
    });
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position: WorldPosition::new(0, ATTACK_DISTANCE_YARDS + 1.0, 0.0, 0.0, 0.0),
                movement_flags: MOVEFLAG_BACKWARD,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    session.visibility.db_creatures.insert(attacker.raw(), runtime);

    assert!(
        !db_creature_can_reach_player(&session, attacker),
        "CMaNGOS walking patrol motion has WALK_MODE, so it should not get moving melee leeway"
    );

    session
        .visibility
        .db_creatures
        .get_mut(&attacker.raw())
        .unwrap()
        .default_movement_run = true;
    assert!(
        db_creature_can_reach_player(&session, attacker),
        "explicit run movement is moving and not walk-mode, so it can get leeway"
    );
}

#[test]
fn native_mmap_path_status_preserves_cmangos_incomplete_flag() {
    let points = [
        NativeMmapPathPoint {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        NativeMmapPathPoint {
            x: 4.0,
            y: 0.0,
            z: 0.0,
        },
    ];

    let path = native_mmap_path_from_count(0, 2, NATIVE_PATHFIND_INCOMPLETE, &points);

    assert_eq!(path.status, NativeMmapPathStatus::Incomplete);
    assert_eq!(path.points.len(), 2);
}

#[test]
fn incomplete_chase_endpoint_must_reach_target_like_cmangos() {
    let creature = DbCreatureRuntime::new(test_creature_spawn(6));
    let target = WorldPosition::new(0, 10.0, 0.0, 0.0, 0.0);

    assert!(
        db_creature_chase_endpoint_reaches_target(
            &creature,
            WorldPosition::new(0, 8.0, 0.0, 0.0, 0.0),
            target
        ),
        "CMaNGOS accepts incomplete paths only when the final point is still in melee reach"
    );
    assert!(
        !db_creature_chase_endpoint_reaches_target(
            &creature,
            WorldPosition::new(0, 4.0, 0.0, 0.0, 0.0),
            target
        ),
        "a partial endpoint outside reach must not become a stable chase destination"
    );
}

#[test]
fn map_runtime_refuses_in_place_facing_while_creature_is_moving() {
    let mut creature = test_creature_spawn(6);
    creature.position_x = 0.0;
    creature.position_y = 0.0;
    creature.position_z = 0.0;
    creature.orientation = 0.0;
    let attacker = creature_spawn_guid(&creature);
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(creature);
    runtime.next_spline_id = 4;
    runtime.motion = CreatureMotionState::Chase(CreatureChaseMotion {
        target: player,
        start: runtime.current_position,
        destination: WorldPosition::new(0, 5.0, 0.0, 0.0, 0.0),
        path: vec![WorldPosition::new(0, 5.0, 0.0, 0.0, 0.0)],
        started_at: now,
        duration: Duration::from_secs(1),
        recheck_at: now + Duration::from_secs(1),
        run: true,
    });
    let mut map = MapRuntime::new(0, 0);
    map.share_db_creature_snapshots(vec![runtime]);

    assert!(map
        .face_db_creature_toward_position(attacker, WorldPosition::new(0, 0.0, 10.0, 0.0, 0.0),)
        .is_none());
    let snapshot = map
        .db_creature_snapshot(attacker)
        .expect("creature should stay loaded");
    assert_eq!(snapshot.current_position.orientation, 0.0);
    assert_eq!(snapshot.next_spline_id, 4);
}

#[test]
fn map_runtime_chase_destination_fans_out_same_victim_attackers() {
    let target = ObjectGuid::new(HighGuid::Player, 0, 7);
    let target_position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    let mut first = test_creature_spawn(6);
    first.guid = 601;
    first.position_x = 10.0;
    first.position_y = 0.0;
    first.position_z = 0.0;
    let first_guid = creature_spawn_guid(&first);
    let mut first_runtime = DbCreatureRuntime::new(first);
    let first_stop = db_creature_chase_stop_distance(&first_runtime);
    first_runtime.current_position = db_creature_chase_near_point(target_position, first_stop, 0.0);

    let mut second = test_creature_spawn(6);
    second.guid = 602;
    second.position_x = 10.0;
    second.position_y = 0.0;
    second.position_z = 0.0;
    let second_guid = creature_spawn_guid(&second);
    let mut map = MapRuntime::new(0, 0);
    map.share_db_creature_snapshots(vec![first_runtime, DbCreatureRuntime::new(second)]);
    map.active_creature_combats.insert(
        first_guid.raw(),
        CreatureCombatState {
            attacker: first_guid,
            victim: target,
            started_at: Instant::now(),
            next_swing_at: Instant::now(),
        },
    );

    let (_snapshot, motion) = map
        .start_db_creature_chase_motion(
            &DbCreatureNavigationGuardrail::default(),
            second_guid,
            target,
            target_position,
            Instant::now(),
        )
        .expect("second attacker should choose a fan-out chase slot");
    let destination = motion.path.last().unwrap();

    assert!(
        destination.y.abs() > 0.1,
        "same-victim attackers should not choose the same target-centered chase slot"
    );
    assert!(distance_2d(first_stop, 0.0, destination.x, destination.y,) > 1.0);
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
        character: CharacterSessionState {
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
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    session
        .visibility
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
fn db_creature_chase_near_point_retries_adjacent_angles_when_primary_los_fails() {
    let target = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    let primary = db_creature_chase_near_point(target, 2.0, 0.0);

    let selected = db_creature_chase_near_point_with_cmangos_los_selector(
        target,
        2.0,
        0.0,
        DEFAULT_WORLD_OBJECT_SIZE,
        |candidate| candidate.y > 0.0,
    );

    assert_ne!(selected.y, primary.y);
    assert!(
        selected.y > 0.0,
        "CMaNGOS GetNearPointAt should try nearby angles instead of staying on the blocked original point"
    );
}

#[test]
fn db_creature_chase_near_point_keeps_primary_when_no_los_candidate_exists() {
    let target = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    let primary = db_creature_chase_near_point(target, 2.0, 0.0);

    let selected = db_creature_chase_near_point_with_cmangos_los_selector(
        target,
        2.0,
        0.0,
        DEFAULT_WORLD_OBJECT_SIZE,
        |_| false,
    );

    assert_eq!(selected.x, primary.x);
    assert_eq!(selected.y, primary.y);
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
            auction_houses: HashMap::new(),
            taxi_nodes: HashMap::new(),
            taxi_paths: HashMap::new(),
            taxi_path_nodes: HashMap::new(),
            taxi_node_mask: [0; 8],
            creature_display_scales: HashMap::new(),
            spell_cast_times: HashMap::new(),
            spell_durations: HashMap::new(),
            spell_radii: HashMap::new(),
            spell_cones: HashMap::new(),
            spell_ranges: HashMap::new(),
            skill_line_abilities_by_spell: HashMap::new(),
            skill_lines: HashMap::new(),
            skill_race_class_infos_by_skill: HashMap::new(),
            faction_templates: FactionTemplateStore::fallback_bridge(),
            item_random_properties: HashMap::new(),
            spell_item_enchantments: HashMap::new(),
            bank_bag_slot_prices: HashMap::new(),
            area_triggers: HashMap::new(),
            area_tables: AreaTableStore::default(),
            wmo_area_tables: WmoAreaTableStore::default(),
            mmap_headers: HashSet::new(),
            mmap_tiles: HashSet::new(),
            vmap_trees: HashSet::from([0]),
            vmap_tiles: HashSet::from([(0, 32, 32), (0, 31, 32)]),
        }),
        ..DbCreatureNavigationGuardrail::default()
    };
    let creature = DbCreatureRuntime::new(test_creature_spawn(6));

    assert!(
        db_creature_chase_path(&navigation, None, &creature, start, target, 2.5).is_none(),
        "LOS-only straight chase must not run without mmap path support"
    );
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
        character: CharacterSessionState {
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
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    session
        .visibility
        .db_creatures
        .insert(attacker.raw(), DbCreatureRuntime::new(creature));

    let first_motion = start_db_creature_chase_motion(&mut session, attacker, player, now)
        .expect("out-of-range creature should start chase motion");
    session
        .character
        .active_character
        .as_mut()
        .unwrap()
        .position
        .x = 20.0;

    let before_recheck = now + Duration::from_millis(DB_CREATURE_CHASE_RECHECK_MILLIS - 1);
    assert!(
        start_db_creature_chase_motion(&mut session, attacker, player, before_recheck).is_none()
    );

    let runtime = session
        .visibility
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
fn active_db_creature_chase_motion_commits_until_arrival() {
    let mut creature = test_creature_spawn(6);
    creature.position_x = 0.0;
    creature.position_y = 0.0;
    creature.position_z = 0.0;
    let attacker = creature_spawn_guid(&creature);
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let now = Instant::now();
    let mut session = WorldSessionState {
        character: CharacterSessionState {
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
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    session
        .visibility
        .db_creatures
        .insert(attacker.raw(), DbCreatureRuntime::new(creature));

    let first_motion = start_db_creature_chase_motion(&mut session, attacker, player, now)
        .expect("out-of-range creature should start chase motion");
    session
        .character
        .active_character
        .as_mut()
        .unwrap()
        .position
        .x = 20.0;
    let recheck_at = now + Duration::from_millis(DB_CREATURE_CHASE_RECHECK_MILLIS);
    advance_db_creature_motion(&mut session, attacker, recheck_at);
    let moved_start = session
        .visibility
        .db_creatures
        .get(&attacker.raw())
        .expect("creature should still be loaded")
        .current_position;
    assert!(moved_start.x > 0.0);

    assert!(
        start_db_creature_chase_motion(&mut session, attacker, player, recheck_at).is_none(),
        "real-WoW chase should commit to the active spline instead of retargeting mid-run"
    );
    {
        let runtime = session
            .visibility
            .db_creatures
            .get(&attacker.raw())
            .expect("creature should still be loaded");
        let CreatureMotionState::Chase(chase) = &runtime.motion else {
            panic!("creature should remain in chase motion");
        };
        assert_eq!(runtime.next_spline_id, 1);
        assert_eq!(chase.destination.x, first_motion.path.last().unwrap().x);
    }

    let arrived_at = now + first_motion.duration;
    advance_db_creature_motion(&mut session, attacker, arrived_at);
    let landed_start = session
        .visibility
        .db_creatures
        .get(&attacker.raw())
        .expect("creature should still be loaded")
        .current_position;
    let second_motion = start_db_creature_chase_motion(&mut session, attacker, player, arrived_at)
        .expect("far moved player should immediately trigger a refreshed chase spline after arrival");

    assert_eq!(second_motion.spline_id, 1);
    assert_eq!(second_motion.start.x, landed_start.x);
    assert!(
        second_motion.path.last().unwrap().x
            > first_motion.path.last().unwrap().x + ATTACK_DISTANCE_YARDS
    );
    let runtime = session
        .visibility
        .db_creatures
        .get(&attacker.raw())
        .expect("creature should still be loaded");
    let CreatureMotionState::Chase(chase) = &runtime.motion else {
        panic!("creature should remain in chase motion");
    };
    assert_eq!(runtime.next_spline_id, 2);
    assert_eq!(
        chase.recheck_at,
        arrived_at + Duration::from_millis(DB_CREATURE_CHASE_RECHECK_MILLIS)
    );
}

#[test]
fn active_chase_refreshes_when_moving_target_cuts_inside_old_destination() {
    let mut creature = test_creature_spawn(6);
    creature.position_x = 0.0;
    creature.position_y = 0.0;
    creature.position_z = 0.0;
    let attacker = creature_spawn_guid(&creature);
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let now = Instant::now();
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position: WorldPosition::new(0, 10.0, 0.0, 0.0, 0.0),
                movement_flags: MOVEFLAG_FORWARD,
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
        .insert(attacker.raw(), DbCreatureRuntime::new(creature));

    let first_motion = start_db_creature_chase_motion(&mut session, attacker, player, now)
        .expect("out-of-range creature should start chase motion");
    let first_destination = *first_motion.path.last().unwrap();
    let moving_recheck_at =
        now + Duration::from_millis(DB_CREATURE_CHASE_MOVING_TARGET_RECHECK_MILLIS);
    advance_db_creature_motion(&mut session, attacker, moving_recheck_at);
    let runtime = session
        .visibility
        .db_creatures
        .get(&attacker.raw())
        .expect("creature should still be loaded");
    assert_eq!(runtime.next_spline_id, 1);
    assert_eq!(
        match &runtime.motion {
            CreatureMotionState::Chase(chase) => chase.recheck_at,
            _ => panic!("creature should remain in chase motion"),
        },
        now + Duration::from_millis(DB_CREATURE_CHASE_MOVING_TARGET_RECHECK_MILLIS)
    );
    let desired_stop_distance = db_creature_chase_stop_distance(runtime);
    session
        .character
        .active_character
        .as_mut()
        .unwrap()
        .position
        .x = first_destination.x - desired_stop_distance - 0.5;

    let refreshed =
        start_db_creature_chase_motion(&mut session, attacker, player, moving_recheck_at)
            .expect("moving target cutting inside the old destination should refresh active chase");
    assert_eq!(refreshed.spline_id, 1);
    assert!(refreshed.path.last().unwrap().x < first_destination.x);
}

#[test]
fn active_chase_does_not_refresh_when_moving_target_runs_farther_away() {
    let mut creature = test_creature_spawn(6);
    creature.position_x = 0.0;
    creature.position_y = 0.0;
    creature.position_z = 0.0;
    let attacker = creature_spawn_guid(&creature);
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let now = Instant::now();
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position: WorldPosition::new(0, 10.0, 0.0, 0.0, 0.0),
                movement_flags: MOVEFLAG_FORWARD,
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
        .insert(attacker.raw(), DbCreatureRuntime::new(creature));

    let first_motion = start_db_creature_chase_motion(&mut session, attacker, player, now)
        .expect("out-of-range creature should start chase motion");
    session
        .character
        .active_character
        .as_mut()
        .unwrap()
        .position
        .x = 20.0;
    let moving_recheck_at =
        now + Duration::from_millis(DB_CREATURE_CHASE_MOVING_TARGET_RECHECK_MILLIS);
    advance_db_creature_motion(&mut session, attacker, moving_recheck_at);

    assert!(
        start_db_creature_chase_motion(&mut session, attacker, player, moving_recheck_at).is_none(),
        "moving farther away should keep the active spline committed until landing"
    );
    let runtime = session
        .visibility
        .db_creatures
        .get(&attacker.raw())
        .expect("creature should still be loaded");
    let CreatureMotionState::Chase(chase) = &runtime.motion else {
        panic!("creature should remain in chase motion");
    };
    assert_eq!(runtime.next_spline_id, 1);
    assert_eq!(chase.destination.x, first_motion.path.last().unwrap().x);
}

#[test]
fn db_creature_chase_motion_keeps_position_while_target_remains_in_desired_stop_range() {
    let mut creature = test_creature_spawn(6);
    creature.position_x = 0.0;
    creature.position_y = 0.0;
    creature.position_z = 0.0;
    let attacker = creature_spawn_guid(&creature);
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let now = Instant::now();
    let mut session = WorldSessionState {
        character: CharacterSessionState {
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
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    session
        .visibility
        .db_creatures
        .insert(attacker.raw(), DbCreatureRuntime::new(creature));

    let first_motion = start_db_creature_chase_motion(&mut session, attacker, player, now)
        .expect("out-of-range creature should start chase motion");
    let original_destination = *first_motion.path.last().unwrap();
    let desired_stop_distance = db_creature_chase_stop_distance(
        session
            .visibility
            .db_creatures
            .get(&attacker.raw())
            .expect("creature should still be loaded"),
    );
    session
        .character
        .active_character
        .as_mut()
        .unwrap()
        .position
        .x = original_destination.x + desired_stop_distance - 0.1;
    let recheck_at = now + Duration::from_millis(DB_CREATURE_CHASE_RECHECK_MILLIS);

    assert!(start_db_creature_chase_motion(&mut session, attacker, player, recheck_at).is_none());
    let runtime = session
        .visibility
        .db_creatures
        .get(&attacker.raw())
        .expect("creature should still be loaded");
    assert_eq!(runtime.next_spline_id, 1);
    let CreatureMotionState::Chase(chase) = &runtime.motion else {
        panic!("creature should remain in chase motion");
    };
    assert_eq!(chase.destination.x, original_destination.x);
}

#[test]
fn arrived_chase_repaths_once_player_leaves_desired_stop_range() {
    let mut creature = test_creature_spawn(6);
    creature.position_x = 0.0;
    creature.position_y = 0.0;
    creature.position_z = 0.0;
    let attacker = creature_spawn_guid(&creature);
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let now = Instant::now();
    let mut session = WorldSessionState {
        character: CharacterSessionState {
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
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    session
        .visibility
        .db_creatures
        .insert(attacker.raw(), DbCreatureRuntime::new(creature));

    let first_motion = start_db_creature_chase_motion(&mut session, attacker, player, now)
        .expect("out-of-range creature should start chase motion");
    let arrived_at = now + first_motion.duration;
    advance_db_creature_motion(&mut session, attacker, arrived_at);
    let destination = *first_motion.path.last().unwrap();
    let desired_stop_distance = db_creature_chase_stop_distance(
        session
            .visibility
            .db_creatures
            .get(&attacker.raw())
            .expect("creature should still be loaded"),
    );
    {
        let runtime = session
            .visibility
            .db_creatures
            .get(&attacker.raw())
            .expect("creature should still be loaded");
        let CreatureMotionState::Chase(chase) = &runtime.motion else {
            panic!("arrived creature should keep its chase generator");
        };
        assert!(db_creature_chase_motion_arrived(runtime, chase));
        assert_eq!(runtime.current_position.x, destination.x);
    }

    let player_runtime = session.character.active_character.as_mut().unwrap();
    player_runtime.position.x = destination.x + desired_stop_distance - 0.1;
    player_runtime.movement_flags = MOVEFLAG_BACKWARD;

    assert!(
        db_creature_can_reach_player(&session, attacker),
        "target is still inside base melee range"
    );
    assert!(
        start_db_creature_chase_motion(&mut session, attacker, player, arrived_at).is_none(),
        "real-WoW chase should stay put while the target remains inside the desired stop range"
    );
    {
        let runtime = session
            .visibility
            .db_creatures
            .get(&attacker.raw())
            .expect("creature should still be loaded");
        let CreatureMotionState::Chase(chase) = &runtime.motion else {
            panic!("small shift should leave the original chase generator in place");
        };
        assert!(db_creature_chase_motion_arrived(runtime, chase));
        assert_eq!(runtime.next_spline_id, 1);
    }

    let player_runtime = session.character.active_character.as_mut().unwrap();
    player_runtime.position.x = destination.x + desired_stop_distance + 0.1;

    assert!(
        db_creature_can_reach_player(&session, attacker),
        "target is still melee-reachable, but real-WoW chase should correct the landing distance"
    );

    let repath = start_db_creature_chase_motion(&mut session, attacker, player, arrived_at)
        .expect("target outside desired stop range should start a refreshed chase");
    assert_eq!(repath.spline_id, 1);
    assert!(repath.path.last().unwrap().x > destination.x);
}

#[test]
fn arrived_chase_repaths_immediately_for_backpedal_correction() {
    let mut creature = test_creature_spawn(6);
    creature.position_x = 0.0;
    creature.position_y = 0.0;
    creature.position_z = 0.0;
    let attacker = creature_spawn_guid(&creature);
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let now = Instant::now();
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position: WorldPosition::new(0, 10.0, 0.0, 0.0, 0.0),
                movement_flags: MOVEFLAG_BACKWARD,
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
        .insert(attacker.raw(), DbCreatureRuntime::new(creature));

    let first_motion = start_db_creature_chase_motion(&mut session, attacker, player, now)
        .expect("out-of-range creature should start chase motion");
    let arrived_at = now + first_motion.duration;
    advance_db_creature_motion(&mut session, attacker, arrived_at);
    let destination = *first_motion.path.last().unwrap();
    let desired_stop_distance = db_creature_chase_stop_distance(
        session
            .visibility
            .db_creatures
            .get(&attacker.raw())
            .expect("creature should still be loaded"),
    );

    let player_runtime = session.character.active_character.as_mut().unwrap();
    player_runtime.position.x = destination.x + desired_stop_distance + 0.1;
    assert!(
        db_creature_can_reach_player(&session, attacker),
        "target is still in melee range; this is a landing correction, not a full route"
    );
    let repath = start_db_creature_chase_motion(&mut session, attacker, player, arrived_at)
        .expect("out-of-stop-range backpedal should refresh the landing correction immediately");
    assert_eq!(repath.spline_id, 1);
    assert!(repath.path.last().unwrap().x > destination.x);
}

#[test]
fn arrived_chase_relaunches_immediately_once_target_opens_real_gap() {
    let mut creature = test_creature_spawn(6);
    creature.position_x = 0.0;
    creature.position_y = 0.0;
    creature.position_z = 0.0;
    let attacker = creature_spawn_guid(&creature);
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let now = Instant::now();
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position: WorldPosition::new(0, 10.0, 0.0, 0.0, 0.0),
                movement_flags: MOVEFLAG_BACKWARD,
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
        .insert(attacker.raw(), DbCreatureRuntime::new(creature));

    let first_motion = start_db_creature_chase_motion(&mut session, attacker, player, now)
        .expect("out-of-range creature should start chase motion");
    let arrived_at = now + first_motion.duration;
    advance_db_creature_motion(&mut session, attacker, arrived_at);
    let destination = *first_motion.path.last().unwrap();

    let player_runtime = session.character.active_character.as_mut().unwrap();
    player_runtime.position.x = destination.x + ATTACK_DISTANCE_YARDS + 0.1;

    assert!(
        !db_creature_can_reach_player(&session, attacker),
        "target has opened a real gap beyond normal melee reach"
    );
    let repath = start_db_creature_chase_motion(&mut session, attacker, player, arrived_at)
        .expect("far target should relaunch chase immediately without stability pausing");
    assert_eq!(repath.spline_id, 1);
    assert!(repath.path.last().unwrap().x > destination.x);
}
