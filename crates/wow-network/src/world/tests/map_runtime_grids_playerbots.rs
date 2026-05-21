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
async fn map_runtime_static_world_cache_loads_creatures_without_db_grid_query() {
    let center = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut in_grid = test_creature_spawn(6);
    in_grid.guid = 44;
    in_grid.position_x = center.x + 10.0;
    in_grid.position_y = center.y;
    let in_grid_guid = creature_spawn_guid(&in_grid).raw();

    let other_grid_position = grid_center_position(GridCoord {
        x: grid_coord_for_position(center).x + 1,
        y: grid_coord_for_position(center).y,
    });
    let mut other_grid = test_creature_spawn(6);
    other_grid.guid = 45;
    other_grid.position_x = other_grid_position.x;
    other_grid.position_y = other_grid_position.y;

    let cache = StaticWorldSpawnCache::from_spawns(vec![in_grid, other_grid], Vec::new());
    assert_eq!(
        cache.counts(),
        StaticWorldCacheCounts {
            creature_spawns: 2,
            creature_grids: 2,
            gameobject_spawns: 0,
            gameobject_grids: 0,
        }
    );
    let maps = MapRuntimeManager::with_static_world_cache(cache);

    maps.ensure_static_creature_grids_loaded_for_test(0, center, CREATURE_SPAWN_RADIUS_YARDS)
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
    let nearby = maps
        .nearby_db_creature_snapshots(0, center, CREATURE_SPAWN_RADIUS_YARDS, 16)
        .await;
    assert_eq!(
        nearby
            .into_iter()
            .map(|creature| creature.guid().raw())
            .collect::<Vec<_>>(),
        vec![in_grid_guid]
    );

    maps.ensure_static_creature_grids_loaded_for_test(0, center, CREATURE_SPAWN_RADIUS_YARDS)
        .await;
    assert_eq!(
        maps.creature_grid_load_stats(),
        CreatureGridLoadStats {
            ensure_calls: 2,
            cache_hits: 1,
            db_queries: 1,
            rows_loaded: 1,
        }
    );
}

#[test]
fn static_world_cache_filters_game_event_bound_spawns() {
    let now = ymdhms_to_unix(2026, 5, 7, 12, 30, 0).unwrap();
    let events = GameEventState::from_schedules_at(
        &[
            wow_db::GameEventScheduleQuery {
                entry: 10,
                schedule_type: 1,
                occurrence: 1_440,
                length: 120,
                holiday: 0,
                linked_to: 0,
                description: Some("active test event".to_string()),
                start_time_unix: ymdhms_to_unix(2026, 5, 7, 12, 0, 0),
                end_time_unix: ymdhms_to_unix(2026, 5, 8, 12, 0, 0),
            },
            wow_db::GameEventScheduleQuery {
                entry: 11,
                schedule_type: 1,
                occurrence: 1_440,
                length: 120,
                holiday: 0,
                linked_to: 0,
                description: Some("inactive test event".to_string()),
                start_time_unix: ymdhms_to_unix(2026, 5, 8, 12, 0, 0),
                end_time_unix: ymdhms_to_unix(2026, 5, 9, 12, 0, 0),
            },
        ],
        now,
    );

    let mut normal_creature = test_creature_spawn(6);
    normal_creature.guid = 44;
    let mut active_creature = test_creature_spawn(6);
    active_creature.guid = 45;
    active_creature.game_event = Some(10);
    let mut inactive_creature = test_creature_spawn(6);
    inactive_creature.guid = 46;
    inactive_creature.game_event = Some(11);
    let mut negative_active_creature = test_creature_spawn(6);
    negative_active_creature.guid = 47;
    negative_active_creature.game_event = Some(-10);
    let mut negative_inactive_creature = test_creature_spawn(6);
    negative_inactive_creature.guid = 48;
    negative_inactive_creature.game_event = Some(-11);
    let mut active_gameobject = test_gameobject_spawn(161557, GO_TYPE_GOOBER);
    active_gameobject.guid = 49;
    active_gameobject.game_event = Some(10);
    let mut inactive_gameobject = test_gameobject_spawn(161558, GO_TYPE_GOOBER);
    inactive_gameobject.guid = 50;
    inactive_gameobject.game_event = Some(11);

    let cache = StaticWorldSpawnCache::from_spawns_for_game_events(
        vec![
            normal_creature,
            active_creature,
            inactive_creature,
            negative_active_creature,
            negative_inactive_creature,
        ],
        vec![active_gameobject, inactive_gameobject],
        &events,
    );

    let grid = grid_coord_for_position(WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0));
    let guids = cache
        .creature_spawns_for_grid(0, grid)
        .into_iter()
        .map(|spawn| spawn.guid)
        .collect::<Vec<_>>();
    assert_eq!(guids, vec![44, 45, 48]);
    assert_eq!(
        cache.counts(),
        StaticWorldCacheCounts {
            creature_spawns: 3,
            creature_grids: 1,
            gameobject_spawns: 1,
            gameobject_grids: 1,
        }
    );
    assert_eq!(
        cache
            .gameobject_spawns_for_grid(0, grid)
            .into_iter()
            .map(|spawn| spawn.guid)
            .collect::<Vec<_>>(),
        vec![49]
    );
}

#[test]
fn static_world_cache_reevaluates_game_event_bound_spawns_after_event_change() {
    let grid = grid_coord_for_position(WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0));
    let active_events = GameEventState {
        active_events: HashSet::from([10]),
    };
    let mut first_creature = test_creature_spawn(6);
    first_creature.guid = 44;
    first_creature.game_event = Some(10);
    let mut second_creature = test_creature_spawn(7);
    second_creature.guid = 45;
    second_creature.game_event = Some(11);
    let mut first_gameobject = test_gameobject_spawn(161557, GO_TYPE_GOOBER);
    first_gameobject.guid = 46;
    first_gameobject.game_event = Some(10);
    let mut second_gameobject = test_gameobject_spawn(161558, GO_TYPE_GOOBER);
    second_gameobject.guid = 47;
    second_gameobject.game_event = Some(11);
    let cache = StaticWorldSpawnCache::from_spawns_for_game_events(
        vec![first_creature, second_creature],
        vec![first_gameobject, second_gameobject],
        &active_events,
    );

    assert_eq!(
        cache
            .creature_spawns_for_grid(0, grid)
            .into_iter()
            .map(|spawn| spawn.guid)
            .collect::<Vec<_>>(),
        vec![44]
    );
    assert!(cache.replace_active_game_events(GameEventState {
        active_events: HashSet::from([11]),
    }));
    assert_eq!(
        cache
            .creature_spawns_for_grid(0, grid)
            .into_iter()
            .map(|spawn| spawn.guid)
            .collect::<Vec<_>>(),
        vec![45]
    );
    assert_eq!(
        cache
            .gameobject_spawns_for_grid(0, grid)
            .into_iter()
            .map(|spawn| spawn.guid)
            .collect::<Vec<_>>(),
        vec![47]
    );
    assert_eq!(
        cache.counts(),
        StaticWorldCacheCounts {
            creature_spawns: 1,
            creature_grids: 1,
            gameobject_spawns: 1,
            gameobject_grids: 1,
        }
    );
    assert!(!cache.replace_active_game_events(GameEventState {
        active_events: HashSet::from([11]),
    }));
}

#[test]
fn map_runtime_game_event_refresh_reconciles_loaded_creatures_and_gameobjects() {
    let center = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let grid = grid_coord_for_position(center);
    let mut map = MapRuntime::new(0, 0);
    map.add_player(test_player_runtime(7, SessionId(7), center))
        .unwrap();

    let mut inactive_creature = test_creature_spawn(6);
    inactive_creature.guid = 44;
    inactive_creature.position_x = center.x + 1.0;
    inactive_creature.position_y = center.y;
    inactive_creature.game_event = Some(10);
    let inactive_creature_guid = creature_spawn_guid(&inactive_creature);
    let mut inactive_gameobject = test_gameobject_spawn(161557, GO_TYPE_GOOBER);
    inactive_gameobject.guid = 45;
    inactive_gameobject.position_x = center.x + 2.0;
    inactive_gameobject.position_y = center.y;
    inactive_gameobject.game_event = Some(10);
    let inactive_gameobject_guid = gameobject_spawn_guid(&inactive_gameobject);
    map.insert_loaded_creature_grid(grid, vec![DbCreatureRuntime::new(inactive_creature)]);
    map.insert_loaded_gameobject_grid(grid, vec![DbGameObjectRuntime::new(inactive_gameobject)]);

    let mut active_creature = test_creature_spawn(7);
    active_creature.guid = 46;
    active_creature.position_x = center.x + 3.0;
    active_creature.position_y = center.y;
    active_creature.game_event = Some(11);
    let active_creature_guid = creature_spawn_guid(&active_creature);
    let mut active_gameobject = test_gameobject_spawn(161558, GO_TYPE_GOOBER);
    active_gameobject.guid = 47;
    active_gameobject.position_x = center.x + 4.0;
    active_gameobject.position_y = center.y;
    active_gameobject.game_event = Some(11);
    let active_gameobject_guid = gameobject_spawn_guid(&active_gameobject);

    let creature_packets = map
        .refresh_static_event_creature_grid(grid, vec![DbCreatureRuntime::new(active_creature)])
        .unwrap();
    let gameobject_packets = map
        .refresh_static_event_gameobject_grid(
            grid,
            vec![DbGameObjectRuntime::new(active_gameobject)],
            Instant::now(),
        )
        .unwrap();

    assert!(!map.creatures.contains_key(&inactive_creature_guid.raw()));
    assert!(!map
        .gameobjects
        .contains_key(&inactive_gameobject_guid.raw()));
    assert!(map.creatures.contains_key(&active_creature_guid.raw()));
    assert!(map.gameobjects.contains_key(&active_gameobject_guid.raw()));
    let visible = &map.players.get(&7).unwrap().visible_objects;
    assert!(!visible.contains(&inactive_creature_guid));
    assert!(!visible.contains(&inactive_gameobject_guid));
    assert!(visible.contains(&active_creature_guid));
    assert!(visible.contains(&active_gameobject_guid));
    assert_eq!(creature_packets.len(), 2);
    assert_eq!(gameobject_packets.len(), 2);
    assert_eq!(
        creature_packets[0].1.opcode,
        WorldOpcode::SmsgDestroyObject as u16
    );
    assert_eq!(
        creature_packets[1].1.opcode,
        WorldOpcode::SmsgUpdateObject as u16
    );
    assert_eq!(
        gameobject_packets[0].1.opcode,
        WorldOpcode::SmsgDestroyObject as u16
    );
    assert_eq!(
        gameobject_packets[1].1.opcode,
        WorldOpcode::SmsgUpdateObject as u16
    );
}

#[tokio::test]
async fn map_runtime_static_world_cache_loads_gameobjects_without_db_grid_query() {
    let center = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut in_grid = test_gameobject_spawn(161557, GO_TYPE_GOOBER);
    in_grid.guid = 44;
    in_grid.position_x = center.x + 10.0;
    in_grid.position_y = center.y;
    let in_grid_guid = gameobject_spawn_guid(&in_grid).raw();

    let other_grid_position = grid_center_position(GridCoord {
        x: grid_coord_for_position(center).x + 1,
        y: grid_coord_for_position(center).y,
    });
    let mut other_grid = test_gameobject_spawn(161557, GO_TYPE_GOOBER);
    other_grid.guid = 45;
    other_grid.position_x = other_grid_position.x;
    other_grid.position_y = other_grid_position.y;

    let cache = StaticWorldSpawnCache::from_spawns(Vec::new(), vec![in_grid, other_grid]);
    assert_eq!(
        cache.counts(),
        StaticWorldCacheCounts {
            creature_spawns: 0,
            creature_grids: 0,
            gameobject_spawns: 2,
            gameobject_grids: 2,
        }
    );
    let maps = MapRuntimeManager::with_static_world_cache(cache);

    maps.ensure_static_gameobject_grids_loaded_for_test(0, center, CREATURE_SPAWN_RADIUS_YARDS)
        .await;

    let nearby = maps
        .nearby_db_gameobject_snapshots(0, center, CREATURE_SPAWN_RADIUS_YARDS, 16)
        .await;
    assert_eq!(
        nearby
            .into_iter()
            .map(|gameobject| gameobject.guid().raw())
            .collect::<Vec<_>>(),
        vec![in_grid_guid]
    );
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
        account_id: Some(1),
        controller: PlayerController::Client {
            session_id: observer_session,
        },
        bot_runtime: None,
        selected_target: None,
        unit_target: None,
        active_combat_target: None,
        active_combat_attack_kind: PlayerAutoAttackKind::Melee,
        active_combat_next_swing_at: None,
        ranged_auto_attack_next_shot_at: None,
        in_combat: false,
        looting: false,
        position: center,
        movement_flags: 0,
        client_time: 0,
        server_time: 0,
        fall_time: 0,
        last_fall_z: None,
        last_fall_time: 0,
        environment: PlayerEnvironmentRuntime::default(),
        jump: JumpInfo::default(),
        cell: cell_coord_for_position(center),
        visible_objects: HashSet::new(),
        next_sight_aggro_check_at: None,
        last_sight_aggro_check_position: None,
        last_player_visibility_refresh_position: None,
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
        death_state: PlayerDeathState::Alive,
        level: 1,
        race: 1,
        class: 1,
        spirit: 20,
        gender: 0,
        base_world_stats: PlayerWorldStats {
            base_health: 20,
            base_mana: 0,
            stats: [23, 20, 22, 20, 20],
            next_level_xp: 400,
        },
        effective_world_stats: PlayerWorldStats {
            base_health: 20,
            base_mana: 0,
            stats: [23, 20, 22, 20, 20],
            next_level_xp: 400,
        },
        health: 20,
        max_health: 20,
        xp: 0,
        power1: 0,
        max_power1: 0,
        last_mana_use_at: None,
        power2: 0,
        power4: 0,
        max_power4: POWER_ENERGY_DEFAULT,
        player_bytes: 0,
        player_bytes2: 0,
        combo_target: None,
        combo_points: 0,
        stand_state: PLAYER_STAND_STATE_STAND,
        active_spells: HashSet::new(),
        inventory: Vec::new(),
        quest_statuses: HashMap::new(),
        explored_zones: [0; PLAYER_EXPLORED_ZONES_SIZE],
        active_auras: Vec::new(),
        spell_global_cooldowns_until: HashMap::new(),
        spell_cooldowns_until: HashMap::new(),
        spell_cooldown_categories: HashMap::new(),
        spell_cooldown_item_ids: HashMap::new(),
        queued_next_melee_spell: None,
        base_combat_stats: test_player_combat_stats(),
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
            && packet.opcode == WorldOpcode::SmsgDestroyObject as u16
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
        slot: 0,
        item: 117,
        count: 1,
        display_id: 117,
        quality: 0,
        free_for_all: false,
        quest_drop: false,
    };
    let second_loot = DbCreatureLootRuntime {
        slot: 0,
        item: 118,
        count: 1,
        display_id: 118,
        quality: 0,
        free_for_all: false,
        quest_drop: false,
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
        slot: 0,
        item: 117,
        count: 1,
        display_id: 117,
        quality: 0,
        free_for_all: false,
        quest_drop: false,
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
fn map_runtime_db_gameobject_loot_slots_stay_stable_after_top_claim() {
    let mut map = MapRuntime::new(0, 0);
    let spawn = test_gameobject_spawn(161557, GO_TYPE_CHEST);
    let guid = gameobject_spawn_guid(&spawn).raw();
    map.insert_loaded_gameobject_grid(
        grid_coord_for_position(gameobject_spawn_position(&spawn)),
        vec![DbGameObjectRuntime::new(spawn)],
    );
    map.open_db_gameobject_loot(
        guid,
        1,
        vec![
            DbCreatureLootRuntime {
                slot: 0,
                item: 117,
                count: 1,
                display_id: 117,
                quality: 0,
                free_for_all: false,
                quest_drop: false,
            },
            DbCreatureLootRuntime {
                slot: 0,
                item: 118,
                count: 1,
                display_id: 118,
                quality: 0,
                free_for_all: false,
                quest_drop: false,
            },
            DbCreatureLootRuntime {
                slot: 0,
                item: 119,
                count: 1,
                display_id: 119,
                quality: 0,
                free_for_all: false,
                quest_drop: false,
            },
        ],
    )
    .expect("gameobject loot should open");

    assert_eq!(
        map.take_db_gameobject_loot_item(1, 0)
            .map(|(_, slot, loot)| (slot, loot.item)),
        Some((0, 117))
    );
    assert_eq!(
        map.take_db_gameobject_loot_item(1, 1)
            .map(|(_, slot, loot)| (slot, loot.item)),
        Some((1, 118))
    );
    assert_eq!(
        map.take_db_gameobject_loot_item(1, 2)
            .map(|(_, slot, loot)| (slot, loot.item)),
        Some((2, 119))
    );
}

#[test]
fn map_runtime_db_gameobject_loot_reports_empty_only_after_last_claim() {
    let mut map = MapRuntime::new(0, 0);
    let spawn = test_gameobject_spawn(161557, GO_TYPE_CHEST);
    let guid = gameobject_spawn_guid(&spawn).raw();
    map.insert_loaded_gameobject_grid(
        grid_coord_for_position(gameobject_spawn_position(&spawn)),
        vec![DbGameObjectRuntime::new(spawn)],
    );
    map.open_db_gameobject_loot(
        guid,
        1,
        vec![
            DbCreatureLootRuntime {
                slot: 0,
                item: 117,
                count: 1,
                display_id: 117,
                quality: 0,
                free_for_all: false,
                quest_drop: false,
            },
            DbCreatureLootRuntime {
                slot: 0,
                item: 118,
                count: 1,
                display_id: 118,
                quality: 0,
                free_for_all: false,
                quest_drop: false,
            },
        ],
    )
    .expect("gameobject loot should open");

    assert!(!map.db_gameobject_loot_is_empty(guid));
    assert_eq!(
        map.take_db_gameobject_loot_item(1, 0)
            .map(|(_, slot, loot)| (slot, loot.item)),
        Some((0, 117))
    );
    assert!(!map.db_gameobject_loot_is_empty(guid));
    assert_eq!(
        map.take_db_gameobject_loot_item(1, 1)
            .map(|(_, slot, loot)| (slot, loot.item)),
        Some((1, 118))
    );
    assert!(map.db_gameobject_loot_is_empty(guid));
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
        jump: JumpInfo::default(),
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

    let faction_templates = FactionTemplateStore::fallback_bridge();
    let targets =
        map.select_db_creature_sight_aggro_targets(&faction_templates, &character, Instant::now());

    assert_eq!(
        targets
            .into_iter()
            .map(|creature| creature.guid())
            .collect::<Vec<_>>(),
        vec![near_guid]
    );
}

#[test]
fn map_runtime_sight_aggro_is_throttled_until_player_moves_enough() {
    let mut map = MapRuntime::new(0, 0);
    let mut character = ActiveCharacter {
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
    map.add_player(test_player_runtime(
        character.guid,
        SessionId(7),
        character.position,
    ))
    .expect("player should enter map");

    let mut near_hostile = test_creature_spawn(38);
    near_hostile.guid = 45;
    near_hostile.position_x = character.position.x + 8.0;
    near_hostile.template.faction = 17;
    near_hostile.template.npc_flags = 0;
    near_hostile.template.creature_type = 7;
    near_hostile.template.min_level = 1;
    near_hostile.template.detection_range = 20;
    let near_guid = creature_spawn_guid(&near_hostile);
    map.insert_loaded_creature_grid(
        grid_coord_for_position(character.position),
        vec![DbCreatureRuntime::new(near_hostile)],
    );

    let faction_templates = FactionTemplateStore::fallback_bridge();
    let now = Instant::now();
    let targets = map.select_db_creature_sight_aggro_targets(&faction_templates, &character, now);
    assert_eq!(
        targets
            .into_iter()
            .map(|creature| creature.guid())
            .collect::<Vec<_>>(),
        vec![near_guid]
    );

    assert!(map
        .select_db_creature_sight_aggro_targets(
            &faction_templates,
            &character,
            now + Duration::from_millis(50),
        )
        .is_empty());

    character.position.x += 3.0;
    let targets = map.select_db_creature_sight_aggro_targets(
        &faction_templates,
        &character,
        now + Duration::from_millis(60),
    );
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
fn map_runtime_disconnect_in_combat_lingers_before_removal() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8951.0, -130.0, 83.5, 0.0);
    let grid = grid_coord_for_position(player_position);
    map.add_player(test_player_runtime(7, SessionId(7), player_position))
        .unwrap();
    map.add_player(test_player_runtime(8, SessionId(8), observer_position))
        .unwrap();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 306;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    let creature_guid = creature_spawn_guid(&spawn);
    map.insert_loaded_creature_grid(grid, vec![DbCreatureRuntime::new(spawn)]);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 7);
    assert!(map
        .begin_db_creature_combat(creature_guid, victim, now)
        .is_some());

    assert!(map.disconnect_player_for_linger(7, now).is_some());

    let player = map.players.get(&7).expect("body should remain in map");
    assert_eq!(
        player.disconnected_remove_at(),
        Some(now + CMANGOS_DISCONNECTED_PLAYER_LINGER)
    );
    assert!(player.client_session_id().is_none());
    assert!(player.in_combat);
    assert!(map
        .active_creature_combats
        .contains_key(&creature_guid.raw()));
    assert!(!map
        .grids
        .get(&grid)
        .unwrap()
        .cells
        .get(&cell_coord_for_position(player_position))
        .unwrap()
        .client_players
        .contains(&7));

    assert!(map
        .expire_disconnected_players(
            now + CMANGOS_DISCONNECTED_PLAYER_LINGER - Duration::from_secs(1)
        )
        .is_empty());
    let expired = map.expire_disconnected_players(now + CMANGOS_DISCONNECTED_PLAYER_LINGER);
    assert_eq!(expired.len(), 1);
    assert_eq!(expired[0].player.guid, 7);
    assert!(!map.players.contains_key(&7));
    assert!(!map
        .active_creature_combats
        .contains_key(&creature_guid.raw()));
    assert!(expired[0]
        .observer_packets
        .iter()
        .any(|(session_id, packet)| *session_id == SessionId(8)
            && packet.opcode == WorldOpcode::SmsgDestroyObject as u16));
}

#[test]
fn map_runtime_idle_motion_timers_do_not_lock_grid_unload() {
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
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.next_random_move_at = Some(Instant::now() + Duration::from_secs(5));

    map.insert_loaded_creature_grid(grid, vec![runtime]);

    assert_eq!(
        map.grids.get(&grid).unwrap().state,
        GridState::Idle,
        "future random-walk timers should not permanently unload-lock inactive grids"
    );
}

#[test]
fn map_runtime_expired_idle_grid_unloads_creatures_and_grid_index() {
    let mut map = MapRuntime::new(0, 0);
    let center = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let grid = grid_coord_for_position(center);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 306;
    spawn.position_x = center.x;
    spawn.position_y = center.y;
    spawn.position_z = center.z;
    spawn.movement_type = DB_MOTION_TYPE_RANDOM;
    spawn.spawn_dist = 5.0;
    let guid = creature_spawn_guid(&spawn);
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.next_random_move_at = Some(Instant::now() + Duration::from_secs(5));

    map.insert_loaded_creature_grid(grid, vec![runtime]);
    let touched = map.grids.get(&grid).unwrap().last_touched;
    assert!(map
        .unload_expired_idle_grids(touched + Duration::from_millis(GRID_UNLOAD_DELAY_MILLIS - 1))
        .is_empty());

    let unloaded =
        map.unload_expired_idle_grids(touched + Duration::from_millis(GRID_UNLOAD_DELAY_MILLIS));

    assert_eq!(unloaded, vec![grid]);
    assert!(!map.loaded_creature_grids.contains(&grid));
    assert!(!map.grids.contains_key(&grid));
    assert!(!map.creatures.contains_key(&guid.raw()));
}

#[test]
fn map_runtime_player_interest_prevents_idle_grid_unload() {
    let mut map = MapRuntime::new(0, 0);
    let center = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let grid = grid_coord_for_position(center);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 307;
    spawn.position_x = center.x;
    spawn.position_y = center.y;
    spawn.position_z = center.z;
    let guid = creature_spawn_guid(&spawn);
    map.insert_loaded_creature_grid(grid, vec![DbCreatureRuntime::new(spawn)]);
    map.add_player(test_player_runtime(8, SessionId(8), center))
        .unwrap();
    map.remove_player(8);
    map.add_player(test_player_runtime(
        9,
        SessionId(9),
        WorldPosition::new(
            0,
            center.x + CREATURE_SPAWN_RADIUS_YARDS - 5.0,
            center.y,
            center.z,
            0.0,
        ),
    ))
    .unwrap();

    let touched = map.grids.get(&grid).unwrap().last_touched;
    let unloaded =
        map.unload_expired_idle_grids(touched + Duration::from_millis(GRID_UNLOAD_DELAY_MILLIS));

    assert!(unloaded.is_empty());
    assert!(map.loaded_creature_grids.contains(&grid));
    assert!(map.creatures.contains_key(&guid.raw()));
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
    let nearby_bot = WorldPosition::new(0, -8952.0, -160.0, 83.5, 0.0);
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
    map.add_player(test_bot_player_runtime(5, BotId(5), nearby_bot))
        .unwrap();

    assert_eq!(
        map.nearby_player_guids(center, PLAYER_VISIBILITY_RADIUS_YARDS, Some(1)),
        vec![2, 5]
    );
    assert_eq!(
        map.nearby_client_player_guids(center, PLAYER_VISIBILITY_RADIUS_YARDS, Some(1)),
        vec![2]
    );
}

#[test]
fn map_runtime_bot_controlled_player_is_visible_without_direct_session() {
    let mut map = MapRuntime::new(0, 0);
    let client_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let bot_position = WorldPosition::new(0, -8950.0, -150.0, 83.5, 0.0);
    let client_session = SessionId(77);

    map.add_player(test_player_runtime(1, client_session, client_position))
        .unwrap();
    let packets = map
        .add_player(test_bot_player_runtime(2, BotId(1), bot_position))
        .unwrap();

    assert!(!packets.is_empty());
    assert!(packets
        .iter()
        .all(|(session_id, _)| *session_id == client_session));
    assert!(packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgUpdateObject as u16));
    assert!(map
        .players
        .get(&1)
        .unwrap()
        .visible_objects
        .contains(&ObjectGuid::new(HighGuid::Player, 0, 2)));
    assert!(!map
        .players
        .get(&2)
        .unwrap()
        .visible_objects
        .contains(&ObjectGuid::new(HighGuid::Player, 0, 1)));
    assert_eq!(map.players.get(&2).unwrap().client_session_id(), None);
}

#[test]
fn map_runtime_client_sees_existing_bot_player_on_enter() {
    let mut map = MapRuntime::new(0, 0);
    let bot_position = WorldPosition::new(0, -8950.0, -150.0, 83.5, 0.0);
    let client_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let client_session = SessionId(78);

    assert!(map
        .add_player(test_bot_player_runtime(2, BotId(1), bot_position))
        .unwrap()
        .is_empty());
    {
        let bot = map.players.get_mut(&2).unwrap();
        bot.movement_flags = MOVEFLAG_FORWARD;
        bot.bot_runtime.as_mut().unwrap().active_leg = Some(playerbot_movement_leg(
            bot_position,
            playerbot_roam_destination(bot_position, 2, 0),
            Instant::now(),
        ));
    }
    let packets = map
        .add_player(test_player_runtime(1, client_session, client_position))
        .unwrap();

    assert!(!packets.is_empty());
    assert!(packets
        .iter()
        .all(|(session_id, _)| *session_id == client_session));
    assert!(map
        .players
        .get(&1)
        .unwrap()
        .visible_objects
        .contains(&ObjectGuid::new(HighGuid::Player, 0, 2)));
    assert!(packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::MsgMoveStartForward as u16));
}

#[test]
fn map_runtime_bot_player_does_not_keep_grid_active_without_client_interest() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -150.0, 83.5, 0.0);
    let grid = grid_coord_for_position(position);

    map.add_player(test_bot_player_runtime(2, BotId(1), position))
        .unwrap();

    assert_eq!(map.grids.get(&grid).unwrap().active_player_count, 0);
    assert!(!matches!(
        map.grids.get(&grid).unwrap().state,
        GridState::Active
    ));
}

#[test]
fn map_runtime_playerbot_spawn_is_grounded_before_visibility_and_melee() {
    let data = Arc::new(WorldDataFiles::inspect("C:/World of Warcraft Classic"));
    if !data.maps_available && !data.vmaps_available {
        return;
    }
    let geometry = Arc::new(WorldGeometry::new(data));
    let ground_probe = WorldPosition::new(0, -8939.74, -72.41, 120.0, 0.0);
    let Some(grounded) = geometry.ground_position(ground_probe) else {
        return;
    };
    let mut map = MapRuntime::with_geometry(0, 0, geometry, Arc::new(DbScriptRegistry::default()));

    map.add_player(test_bot_player_runtime(2, BotId(1), ground_probe))
        .unwrap();

    let player = map.players.get(&2).unwrap();
    let bot = player.bot_runtime.as_ref().unwrap();
    assert!((player.position.z - grounded.z).abs() <= 0.01);
    assert!((bot.home_position.z - grounded.z).abs() <= 0.01);
    assert_eq!(player.cell, cell_coord_for_position(grounded));
}

#[test]
fn map_runtime_playerbot_stop_commit_is_grounded_after_high_route_point() {
    let data = Arc::new(WorldDataFiles::inspect("C:/World of Warcraft Classic"));
    if !data.maps_available && !data.vmaps_available {
        return;
    }
    let geometry = Arc::new(WorldGeometry::new(data));
    let start_probe = WorldPosition::new(0, -8939.74, -72.41, 120.0, 0.0);
    let Some(start) = geometry.ground_position(start_probe) else {
        return;
    };
    let high_destination = WorldPosition::new(0, start.x + 5.0, start.y, start.z + 20.0, 0.0);
    let Some(grounded_destination) = geometry.ground_position(high_destination) else {
        return;
    };
    let now = Instant::now();
    let mut map = MapRuntime::with_geometry(0, 0, geometry, Arc::new(DbScriptRegistry::default()));
    map.add_player(test_player_runtime(1, SessionId(77), start))
        .unwrap();
    map.add_player(test_bot_player_runtime(2, BotId(1), start))
        .unwrap();
    {
        let bot = map
            .players
            .get_mut(&2)
            .unwrap()
            .bot_runtime
            .as_mut()
            .unwrap();
        bot.next_think_at = now;
        bot.route = vec![high_destination];
    }

    let start_tick = map
        .advance_playerbot_movement_tick(&DbCreatureNavigationGuardrail::default(), now)
        .unwrap();
    assert_eq!(start_tick.advanced_bots, 1);
    let stop_tick = map
        .advance_playerbot_movement_tick(
            &DbCreatureNavigationGuardrail::default(),
            now + Duration::from_secs(2),
        )
        .unwrap();

    assert_eq!(stop_tick.advanced_bots, 1);
    let player = map.players.get(&2).unwrap();
    assert!((player.position.z - grounded_destination.z).abs() <= 0.01);
    assert_eq!(player.cell, cell_coord_for_position(grounded_destination));
}

#[test]
fn map_runtime_playerbot_far_melee_rejects_before_navigation() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);
    let creature_position = WorldPosition::new(0, -8900.0, -132.0, 83.5, 0.0);
    map.add_player(test_bot_player_runtime(2, BotId(1), player_position))
        .unwrap();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 7010;
    spawn.position_x = creature_position.x;
    spawn.position_y = creature_position.y;
    spawn.position_z = creature_position.z;
    let target = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let navigation = DbCreatureNavigationGuardrail {
        line_of_sight_clear: false,
        path_available: false,
        ..DbCreatureNavigationGuardrail::default()
    };

    let validation = map.validate_player_melee_against_db_creature(2, target, &navigation);

    assert_eq!(validation.check, PlayerMeleeCheck::OutOfRange);
}

#[test]
fn map_runtime_player_melee_rejects_far_evading_target_before_evade_feedback() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    map.add_player(test_player_runtime(1, SessionId(77), player_position))
        .unwrap();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 7011;
    spawn.position_x = 4.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let target = creature_spawn_guid(&spawn);
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.motion = CreatureMotionState::ReturnHome(CreatureReturnHomeMotion {
        start: runtime.current_position,
        destination: runtime.home_position,
        path: vec![runtime.current_position, runtime.home_position],
        started_at: Instant::now(),
        duration: Duration::from_secs(1),
    });
    map.share_db_creature_snapshots(vec![runtime]);

    let in_range = map.validate_player_melee_against_db_creature(
        1,
        target,
        &DbCreatureNavigationGuardrail::default(),
    );
    assert_eq!(in_range.check, PlayerMeleeCheck::TargetEvading);

    map.creatures
        .get_mut(&target.raw())
        .unwrap()
        .current_position
        .x = 8.0;
    let far = map.validate_player_melee_against_db_creature(
        1,
        target,
        &DbCreatureNavigationGuardrail::default(),
    );
    assert_eq!(far.check, PlayerMeleeCheck::OutOfRange);
}

#[test]
fn map_runtime_playerbot_combat_budgets_idle_thinks_without_starving_active_swings() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let bot_position = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);
    let creature_position = WorldPosition::new(0, -8947.0, -132.0, 83.5, std::f32::consts::PI);
    let active_bot_guid = 2;
    map.add_player(test_player_runtime(1, SessionId(77), bot_position))
        .unwrap();
    map.add_player(test_bot_player_runtime(
        active_bot_guid,
        BotId(1),
        bot_position,
    ))
    .unwrap();
    let mut active_spawn = test_creature_spawn(6);
    active_spawn.guid = 7011;
    active_spawn.position_x = creature_position.x;
    active_spawn.position_y = creature_position.y;
    active_spawn.position_z = creature_position.z;
    active_spawn.orientation = creature_position.orientation;
    active_spawn.template.faction = 14;
    let active_target = creature_spawn_guid(&active_spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(active_spawn)]);
    map.start_playerbot_attack(active_bot_guid, active_target, now)
        .unwrap();
    map.set_player_next_swing_at(active_bot_guid, Some(now));

    let first_idle_guid = 10_000;
    for offset in 0..(PLAYERBOT_MAX_COMBAT_THINKS_PER_MAP_TICK as u32 + 3) {
        let guid = first_idle_guid + offset;
        map.add_player(test_bot_player_runtime(
            guid,
            BotId(guid as u64),
            WorldPosition::new(
                0,
                bot_position.x + 40.0 + offset as f32,
                bot_position.y,
                bot_position.z,
                0.0,
            ),
        ))
        .unwrap();
        map.players
            .get_mut(&guid)
            .unwrap()
            .bot_runtime
            .as_mut()
            .unwrap()
            .next_combat_think_at = now;
    }

    let tick = map
        .advance_playerbot_combat_tick(
            &FactionTemplateStore::fallback_bridge(),
            &DbCreatureNavigationGuardrail::default(),
            now,
        )
        .unwrap();

    assert!(tick.budget_exhausted);
    assert!(
        map.players
            .get(&active_bot_guid)
            .unwrap()
            .active_combat_next_swing_at
            .is_some_and(|next| next > now),
        "active swings should be serviced even when idle combat thinking is over budget"
    );
    for guid in first_idle_guid..(first_idle_guid + PLAYERBOT_MAX_COMBAT_THINKS_PER_MAP_TICK as u32)
    {
        assert!(
            map.players
                .get(&guid)
                .unwrap()
                .bot_runtime
                .as_ref()
                .unwrap()
                .next_combat_think_at
                > now,
            "admitted idle bot {guid} should receive a new combat think delay"
        );
    }
    for guid in (first_idle_guid + PLAYERBOT_MAX_COMBAT_THINKS_PER_MAP_TICK as u32)
        ..(first_idle_guid + PLAYERBOT_MAX_COMBAT_THINKS_PER_MAP_TICK as u32 + 3)
    {
        assert_eq!(
            map.players
                .get(&guid)
                .unwrap()
                .bot_runtime
                .as_ref()
                .unwrap()
                .next_combat_think_at,
            now,
            "over-budget idle bot {guid} should wait for a later tick"
        );
    }
}

#[test]
fn map_runtime_playerbot_movement_tick_broadcasts_normal_player_movement() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let client_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let bot_position = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);
    let client_session = SessionId(77);
    map.add_player(test_player_runtime(1, client_session, client_position))
        .unwrap();
    map.add_player(test_bot_player_runtime(2, BotId(1), bot_position))
        .unwrap();
    map.players
        .get_mut(&2)
        .unwrap()
        .bot_runtime
        .as_mut()
        .unwrap()
        .next_think_at = now;
    map.players
        .get_mut(&2)
        .unwrap()
        .bot_runtime
        .as_mut()
        .unwrap()
        .route = vec![WorldPosition::new(0, -8940.0, -132.0, 83.5, 0.0)];

    let start_tick = map
        .advance_playerbot_movement_tick(&DbCreatureNavigationGuardrail::default(), now)
        .unwrap();

    assert_eq!(start_tick.advanced_bots, 1);
    assert!(!start_tick.budget_exhausted);
    assert_eq!(start_tick.packets.len(), 1);
    assert_eq!(start_tick.packets[0].0, client_session);
    assert_eq!(
        start_tick.packets[0].1.opcode,
        WorldOpcode::MsgMoveStartForward as u16
    );
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, 2);
    let movement_start = PackedGuid::packed_size(player_guid);
    let start_movement =
        MovementInfo::read(&start_tick.packets[0].1.body[movement_start..]).unwrap();
    assert_eq!(start_movement.position.map_id, bot_position.map_id);
    assert_eq!(start_movement.position.x, bot_position.x);
    assert!(start_movement.flags & MOVEFLAG_FORWARD != 0);
    assert_eq!(
        map.players.get(&2).unwrap().position,
        start_movement.position
    );

    let mid_leg_tick = map
        .advance_playerbot_movement_tick(
            &DbCreatureNavigationGuardrail::default(),
            now + Duration::from_millis(500),
        )
        .unwrap();

    assert_eq!(mid_leg_tick.advanced_bots, 0);
    assert!(mid_leg_tick.packets.is_empty());
    assert_eq!(
        map.players.get(&2).unwrap().position,
        start_movement.position
    );
    assert!(map.players.get(&2).unwrap().client_session_id().is_none());

    let mut stop_tick = None;
    for step in 2..=20 {
        let tick = map
            .advance_playerbot_movement_tick(
                &DbCreatureNavigationGuardrail::default(),
                now + Duration::from_millis(step * 100),
            )
            .unwrap();
        if tick
            .packets
            .first()
            .is_some_and(|(_, packet)| packet.opcode == WorldOpcode::MsgMoveStop as u16)
        {
            stop_tick = Some(tick);
            break;
        }
    }
    let stop_tick = stop_tick.expect("playerbot should send stop when it reaches the route point");
    assert_eq!(stop_tick.packets.len(), 1);
    let stop_movement = MovementInfo::read(&stop_tick.packets[0].1.body[movement_start..]).unwrap();
    assert_eq!(stop_movement.flags, 0);
}

#[test]
fn map_runtime_playerbot_missing_movement_intent_defers_due_bot() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let bot_position = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);
    map.add_player(test_player_runtime(
        1,
        SessionId(77),
        WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
    ))
    .unwrap();
    map.add_player(test_bot_player_runtime(2, BotId(1), bot_position))
        .unwrap();
    map.players
        .get_mut(&2)
        .unwrap()
        .bot_runtime
        .as_mut()
        .unwrap()
        .next_think_at = now;

    let update = map.prepare_playerbot_roam_movement(2, now);

    assert!(update.is_none());
    assert!(
        map.players
            .get(&2)
            .unwrap()
            .bot_runtime
            .as_ref()
            .unwrap()
            .next_think_at
            > now,
        "a due bot with no queued planner result should not stay due forever"
    );
}

#[test]
fn map_runtime_playerbot_failed_engage_route_clears_target_and_backs_off() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let bot_position = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);
    map.add_player(test_player_runtime(
        1,
        SessionId(77),
        WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
    ))
    .unwrap();
    map.add_player(test_bot_player_runtime(2, BotId(1), bot_position))
        .unwrap();
    let target = ObjectGuid::new(HighGuid::Unit, 0, 7007);
    {
        let bot_player = map.players.get_mut(&2).unwrap();
        let bot = bot_player.bot_runtime.as_mut().unwrap();
        bot.engage_target = Some(target);
        bot.next_think_at = now;
        bot.next_combat_think_at = now;
        bot_player.selected_target = Some(target);
    }
    map.queue_playerbot_intents(vec![(
        2,
        PlayerbotQueuedIntents {
            movement: Some(PlayerbotMovementIntent::Route { route: None }),
            combat: None,
        },
    )]);

    let update = map.prepare_playerbot_roam_movement(2, now);

    assert!(update.is_none());
    let bot = map.players.get(&2).unwrap().bot_runtime.as_ref().unwrap();
    assert_eq!(bot.engage_target, None);
    assert!(bot.route.is_empty());
    assert!(bot.next_think_at > now);
    assert!(
        bot.next_combat_think_at
            >= now + Duration::from_millis(PLAYERBOT_ENGAGE_FAILED_BACKOFF_MILLIS),
        "failed engagement routes should not immediately re-enter target search"
    );
}

#[test]
fn map_runtime_playerbot_combat_starts_against_nearby_hostile_creature() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let client_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let bot_position = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);
    let creature_position = WorldPosition::new(0, -8947.0, -132.0, 83.5, std::f32::consts::PI);
    let client_session = SessionId(77);
    map.add_player(test_player_runtime(1, client_session, client_position))
        .unwrap();
    map.add_player(test_bot_player_runtime(2, BotId(1), bot_position))
        .unwrap();
    map.players
        .get_mut(&2)
        .unwrap()
        .bot_runtime
        .as_mut()
        .unwrap()
        .next_combat_think_at = now;
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 7001;
    spawn.position_x = creature_position.x;
    spawn.position_y = creature_position.y;
    spawn.position_z = creature_position.z;
    spawn.orientation = creature_position.orientation;
    spawn.template.faction = 14;
    spawn.template.npc_flags = 0;
    let target = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);

    let tick = map
        .advance_playerbot_combat_tick(
            &FactionTemplateStore::fallback_bridge(),
            &DbCreatureNavigationGuardrail::default(),
            now,
        )
        .unwrap();

    assert_eq!(tick.advanced_bots, 1);
    assert!(!tick.budget_exhausted);
    assert_eq!(
        map.players.get(&2).unwrap().active_combat_target,
        Some(target)
    );
    assert!(map
        .active_creature_combats
        .get(&target.raw())
        .is_some_and(|combat| combat.victim == ObjectGuid::new(HighGuid::Player, 0, 2)));
    assert!(tick
        .packets
        .iter()
        .all(|(session_id, _)| *session_id == client_session));
    assert!(tick
        .packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgAttackStart as u16));
    assert!(tick
        .packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgUpdateObject as u16));
}

#[test]
fn map_runtime_moving_playerbot_plants_and_attacks_when_hostile_enters_melee() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let client_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let bot_position = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);
    let destination = WorldPosition::new(0, -8940.0, -132.0, 83.5, 0.0);
    let client_session = SessionId(77);
    map.add_player(test_player_runtime(1, client_session, client_position))
        .unwrap();
    map.add_player(test_bot_player_runtime(2, BotId(1), bot_position))
        .unwrap();
    {
        let bot_player = map.players.get_mut(&2).unwrap();
        let bot = bot_player.bot_runtime.as_mut().unwrap();
        bot.active_leg = Some(playerbot_movement_leg(bot_position, destination, now));
        bot.next_combat_think_at = now + Duration::from_millis(500);
        bot_player.movement_flags = MOVEFLAG_FORWARD;
    }
    let stop_position = playerbot_position_on_leg(
        map.players
            .get(&2)
            .unwrap()
            .bot_runtime
            .as_ref()
            .unwrap()
            .active_leg
            .unwrap(),
        now + Duration::from_millis(500),
    );
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 7004;
    spawn.position_x = stop_position.x + 2.0;
    spawn.position_y = stop_position.y;
    spawn.position_z = stop_position.z;
    spawn.orientation = std::f32::consts::PI;
    spawn.template.faction = 14;
    spawn.template.npc_flags = 0;
    let target = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);

    let tick = map
        .advance_playerbot_combat_tick(
            &FactionTemplateStore::fallback_bridge(),
            &DbCreatureNavigationGuardrail::default(),
            now + Duration::from_millis(500),
        )
        .unwrap();

    let bot_player = map.players.get(&2).unwrap();
    assert_eq!(tick.advanced_bots, 1);
    assert_eq!(bot_player.active_combat_target, Some(target));
    assert_eq!(bot_player.movement_flags, 0);
    assert!(bot_player
        .bot_runtime
        .as_ref()
        .unwrap()
        .active_leg
        .is_none());
    assert!(bot_player.position.distance_2d(&stop_position) <= 0.01);
    assert!(tick.packets.iter().any(|(session_id, packet)| {
        *session_id == client_session && packet.opcode == WorldOpcode::MsgMoveStop as u16
    }));
    assert!(tick
        .packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgAttackStart as u16));
}

#[test]
fn map_runtime_playerbot_targets_neutral_and_routes_into_melee() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let client_session = SessionId(77);
    let bot_position = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);
    let creature_position = WorldPosition::new(0, -8925.0, -132.0, 83.5, std::f32::consts::PI);

    map.add_player(test_player_runtime(
        1,
        client_session,
        WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
    ))
    .unwrap();
    map.add_player(test_bot_player_runtime(2, BotId(1), bot_position))
        .unwrap();
    map.players
        .get_mut(&2)
        .unwrap()
        .bot_runtime
        .as_mut()
        .unwrap()
        .next_combat_think_at = now;

    let mut spawn = test_creature_spawn(6);
    spawn.guid = 7005;
    spawn.position_x = creature_position.x;
    spawn.position_y = creature_position.y;
    spawn.position_z = creature_position.z;
    spawn.orientation = creature_position.orientation;
    spawn.template.faction = 25;
    spawn.template.npc_flags = 0;
    let target = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);

    let acquire_tick = map
        .advance_playerbot_combat_tick(
            &FactionTemplateStore::fallback_bridge(),
            &DbCreatureNavigationGuardrail::default(),
            now,
        )
        .unwrap();

    let bot_player = map.players.get(&2).unwrap();
    assert_eq!(acquire_tick.advanced_bots, 1);
    assert_eq!(bot_player.selected_target, Some(target));
    assert_eq!(bot_player.active_combat_target, None);
    assert_eq!(
        bot_player.bot_runtime.as_ref().unwrap().engage_target,
        Some(target)
    );
    assert!(!acquire_tick
        .packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgAttackStart as u16));

    let move_tick = map
        .advance_playerbot_movement_tick(&DbCreatureNavigationGuardrail::default(), now)
        .unwrap();

    assert_eq!(move_tick.advanced_bots, 1);
    assert!(move_tick.packets.iter().any(|(session_id, packet)| {
        *session_id == client_session && packet.opcode == WorldOpcode::MsgMoveStartForward as u16
    }));
    let arrival_time = map
        .players
        .get(&2)
        .unwrap()
        .bot_runtime
        .as_ref()
        .unwrap()
        .active_leg
        .unwrap()
        .arrival_time;

    map.advance_playerbot_movement_tick(&DbCreatureNavigationGuardrail::default(), arrival_time)
        .unwrap();
    let attack_tick = map
        .advance_playerbot_combat_tick(
            &FactionTemplateStore::fallback_bridge(),
            &DbCreatureNavigationGuardrail::default(),
            arrival_time + Duration::from_millis(1),
        )
        .unwrap();

    assert_eq!(attack_tick.advanced_bots, 1);
    assert_eq!(
        map.players.get(&2).unwrap().active_combat_target,
        Some(target)
    );
    assert!(attack_tick
        .packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgAttackStart as u16));
}

#[test]
fn map_runtime_playerbot_can_attack_critter_even_with_friendly_faction() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let bot_position = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);
    let creature_position = WorldPosition::new(0, -8948.0, -132.0, 83.5, std::f32::consts::PI);

    map.add_player(test_player_runtime(
        1,
        SessionId(77),
        WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
    ))
    .unwrap();
    map.add_player(test_bot_player_runtime(2, BotId(1), bot_position))
        .unwrap();
    map.players
        .get_mut(&2)
        .unwrap()
        .bot_runtime
        .as_mut()
        .unwrap()
        .next_combat_think_at = now;

    let mut spawn = test_creature_spawn(6);
    spawn.guid = 7006;
    spawn.position_x = creature_position.x;
    spawn.position_y = creature_position.y;
    spawn.position_z = creature_position.z;
    spawn.orientation = creature_position.orientation;
    spawn.template.faction = 35;
    spawn.template.civilian = 1;
    spawn.template.creature_type = CREATURE_TYPE_CRITTER;
    spawn.template.npc_flags = 0;
    let target = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);

    let tick = map
        .advance_playerbot_combat_tick(
            &FactionTemplateStore::fallback_bridge(),
            &DbCreatureNavigationGuardrail::default(),
            now,
        )
        .unwrap();

    assert_eq!(tick.advanced_bots, 1);
    assert_eq!(
        map.players.get(&2).unwrap().active_combat_target,
        Some(target)
    );
}

#[test]
fn map_runtime_playerbot_auto_attack_uses_shared_creature_damage_packets() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let client_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let bot_position = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);
    let creature_position = WorldPosition::new(0, -8947.0, -132.0, 83.5, std::f32::consts::PI);
    map.add_player(test_player_runtime(1, SessionId(77), client_position))
        .unwrap();
    map.add_player(test_bot_player_runtime(2, BotId(1), bot_position))
        .unwrap();
    map.players
        .get_mut(&2)
        .unwrap()
        .bot_runtime
        .as_mut()
        .unwrap()
        .next_combat_think_at = now;
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 7002;
    spawn.position_x = creature_position.x;
    spawn.position_y = creature_position.y;
    spawn.position_z = creature_position.z;
    spawn.orientation = creature_position.orientation;
    spawn.template.faction = 14;
    spawn.template.npc_flags = 0;
    let target = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    map.advance_playerbot_combat_tick(
        &FactionTemplateStore::fallback_bridge(),
        &DbCreatureNavigationGuardrail::default(),
        now,
    )
    .unwrap();
    map.active_creature_combats
        .get_mut(&target.raw())
        .unwrap()
        .next_swing_at = now + Duration::from_secs(10);

    let tick = map
        .advance_playerbot_combat_tick(
            &FactionTemplateStore::fallback_bridge(),
            &DbCreatureNavigationGuardrail::default(),
            now + Duration::from_millis(1),
        )
        .unwrap();

    assert_eq!(tick.advanced_bots, 1);
    assert!(tick
        .packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgAttackerStateUpdate as u16));
    assert!(tick
        .packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgUpdateObject as u16));
    assert!(map
        .players
        .get(&2)
        .unwrap()
        .active_combat_next_swing_at
        .is_some_and(|next| next > now));
}

#[test]
fn map_runtime_playerbot_creature_retaliation_damages_bot_runtime() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let client_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let bot_position = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);
    let creature_position = WorldPosition::new(0, -8947.0, -132.0, 83.5, std::f32::consts::PI);
    map.add_player(test_player_runtime(1, SessionId(77), client_position))
        .unwrap();
    map.add_player(test_bot_player_runtime(2, BotId(1), bot_position))
        .unwrap();
    map.players
        .get_mut(&2)
        .unwrap()
        .bot_runtime
        .as_mut()
        .unwrap()
        .next_combat_think_at = now;
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 7003;
    spawn.position_x = creature_position.x;
    spawn.position_y = creature_position.y;
    spawn.position_z = creature_position.z;
    spawn.orientation = creature_position.orientation;
    spawn.template.faction = 14;
    spawn.template.npc_flags = 0;
    let target = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    map.advance_playerbot_combat_tick(
        &FactionTemplateStore::fallback_bridge(),
        &DbCreatureNavigationGuardrail::default(),
        now,
    )
    .unwrap();
    map.set_player_next_swing_at(2, Some(now + Duration::from_secs(10)));
    map.active_creature_combats
        .get_mut(&target.raw())
        .unwrap()
        .next_swing_at = now;

    let tick = map
        .advance_playerbot_combat_tick(
            &FactionTemplateStore::fallback_bridge(),
            &DbCreatureNavigationGuardrail::default(),
            now + Duration::from_millis(1),
        )
        .unwrap();

    assert_eq!(tick.creature_swings, 1);
    assert!(tick
        .packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgAttackerStateUpdate as u16));
    assert!(map.players.get(&2).unwrap().health <= 20);
}

#[test]
fn map_runtime_playerbot_travel_uses_navigation_path_destination() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let client_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let bot_position = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);
    let travel_destination = WorldPosition::new(0, -8940.0, -110.0, 83.5, 0.0);

    map.add_player(test_player_runtime(1, SessionId(77), client_position))
        .unwrap();
    map.add_player(test_bot_player_runtime(2, BotId(1), bot_position))
        .unwrap();
    {
        let bot = map
            .players
            .get_mut(&2)
            .unwrap()
            .bot_runtime
            .as_mut()
            .unwrap();
        bot.next_think_at = now;
        bot.travel_destination = Some(travel_destination);
    }

    let start_tick = map
        .advance_playerbot_movement_tick(&DbCreatureNavigationGuardrail::default(), now)
        .unwrap();

    assert_eq!(start_tick.advanced_bots, 1);
    let bot = map.players.get(&2).unwrap().bot_runtime.as_ref().unwrap();
    let active_leg = bot.active_leg.expect("travel should start a path leg");
    assert!(
        bot.route
            .last()
            .is_some_and(|point| point.distance_2d(&travel_destination) <= 0.01)
            || active_leg.destination.distance_2d(&travel_destination) <= 0.01,
        "remaining route or active leg should be aimed at travel destination"
    );
    assert_ne!(
        active_leg.destination,
        playerbot_roam_destination(bot_position, 2, 0),
        "travel mode should not use the square roam target"
    );
}

#[test]
fn playerbot_travel_uses_local_nav_leg_toward_stormwind_when_available() {
    let data = Arc::new(WorldDataFiles::inspect("C:/World of Warcraft Classic"));
    let start = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);
    let stormwind = WorldPosition::new(0, -9_095.62, 422.026, 92.0445, 0.0);
    let Some((start_tile_x, start_tile_y)) = mmap_tile_for_position(start) else {
        panic!("Northshire bot position should resolve to a mmap tile");
    };
    let Some((target_tile_x, target_tile_y)) = mmap_tile_for_position(stormwind) else {
        panic!("Stormwind travel target should resolve to a mmap tile");
    };
    if !data.has_mmap_tile(0, start_tile_x, start_tile_y)
        || !data.has_mmap_tile(0, target_tile_x, target_tile_y)
    {
        return;
    }
    let geometry = WorldGeometry::new(data.clone());
    let navigation = DbCreatureNavigationGuardrail {
        path_available: true,
        world_data_files: data,
        ..DbCreatureNavigationGuardrail::default()
    };

    let route = playerbot_route_points(
        &navigation,
        Some(&geometry),
        start,
        Some(stormwind),
        start,
        0,
        2,
    )
    .expect("local playerbot should build a bounded nav leg toward Stormwind");

    assert!(
        route
            .last()
            .is_some_and(|point| point.distance_2d(&stormwind) < start.distance_2d(&stormwind)),
        "route should make progress toward the configured Stormwind travel target"
    );
}

#[test]
fn playerbot_roam_destination_uses_seeded_far_wander_points() {
    let home = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);
    let first = playerbot_roam_destination(home, 9010000, 0);
    let second = playerbot_roam_destination(home, 9010000, 1);
    let other_bot = playerbot_roam_destination(home, 9010001, 0);

    for destination in [first, second, other_bot] {
        let distance = home.distance_2d(&destination);
        assert!(
            (PLAYERBOT_WANDER_MIN_RADIUS_YARDS..=PLAYERBOT_WANDER_MAX_RADIUS_YARDS)
                .contains(&distance),
            "wander destination should be far enough to spread bots without leaving the configured band"
        );
    }
    assert!(first.distance_2d(&second) > PLAYERBOT_DESTINATION_EPSILON_YARDS);
    assert!(first.distance_2d(&other_bot) > PLAYERBOT_DESTINATION_EPSILON_YARDS);
}

#[test]
fn playerbot_travel_recovers_from_observed_nav_edge_when_available() {
    let data = Arc::new(WorldDataFiles::inspect("C:/World of Warcraft Classic"));
    let stuck_position = WorldPosition::new(0, -9046.40, -106.67, 90.96, 0.0);
    let stormwind = WorldPosition::new(0, -9095.35, 412.33, 92.04, 0.0);
    let Some((start_tile_x, start_tile_y)) = mmap_tile_for_position(stuck_position) else {
        panic!("observed stuck bot position should resolve to a mmap tile");
    };
    let Some((target_tile_x, target_tile_y)) = mmap_tile_for_position(stormwind) else {
        panic!("Stormwind travel target should resolve to a mmap tile");
    };
    if !data.has_mmap_tile(0, start_tile_x, start_tile_y)
        || !data.has_mmap_tile(0, target_tile_x, target_tile_y)
    {
        return;
    }
    let geometry = WorldGeometry::new(data.clone());
    let navigation = DbCreatureNavigationGuardrail {
        path_available: true,
        world_data_files: data,
        ..DbCreatureNavigationGuardrail::default()
    };

    let route = playerbot_route_points(
        &navigation,
        Some(&geometry),
        stuck_position,
        Some(stormwind),
        stuck_position,
        0,
        2,
    )
    .expect("observed nav-edge position should recover a follow-up route");

    assert!(
        route.last().is_some_and(
            |point| point.distance_2d(&stormwind) < stuck_position.distance_2d(&stormwind)
        ),
        "recovered route should continue making progress toward Stormwind"
    );
}

#[test]
fn playerbot_travel_recovers_from_observed_spawn_edge_when_available() {
    let data = Arc::new(WorldDataFiles::inspect("C:/World of Warcraft Classic"));
    let stuck_position = WorldPosition::new(0, -8955.61, -132.80, 83.50, 0.0);
    let stormwind = WorldPosition::new(0, -9095.35, 412.33, 92.04, 0.0);
    let Some((start_tile_x, start_tile_y)) = mmap_tile_for_position(stuck_position) else {
        panic!("observed stuck bot spawn should resolve to a mmap tile");
    };
    let Some((target_tile_x, target_tile_y)) = mmap_tile_for_position(stormwind) else {
        panic!("Stormwind travel target should resolve to a mmap tile");
    };
    if !data.has_mmap_tile(0, start_tile_x, start_tile_y)
        || !data.has_mmap_tile(0, target_tile_x, target_tile_y)
    {
        return;
    }
    let geometry = WorldGeometry::new(data.clone());
    let navigation = DbCreatureNavigationGuardrail {
        path_available: true,
        world_data_files: data,
        ..DbCreatureNavigationGuardrail::default()
    };

    let route = playerbot_route_points(
        &navigation,
        Some(&geometry),
        stuck_position,
        Some(stormwind),
        stuck_position,
        0,
        2,
    )
    .expect("observed spawn-edge position should recover a follow-up route");

    assert!(
        route.last().is_some_and(
            |point| point.distance_2d(&stormwind) < stuck_position.distance_2d(&stormwind)
        ),
        "recovered spawn route should continue making progress toward Stormwind"
    );
}

#[test]
fn playerbot_travel_recovers_from_observed_stormwind_route_edges_when_available() {
    let data = Arc::new(WorldDataFiles::inspect("C:/World of Warcraft Classic"));
    let samples = [
        (
            WorldPosition::new(0, -9110.27, 67.45, 83.25, 0.0),
            WorldPosition::new(0, -9101.79, 416.26, 92.04, 0.0),
        ),
        (
            WorldPosition::new(0, -9139.19, 324.60, 92.89, 0.0),
            WorldPosition::new(0, -9098.91, 414.84, 92.04, 0.0),
        ),
        (
            WorldPosition::new(0, -9112.54, 384.51, 93.27, 0.0),
            WorldPosition::new(0, -9094.55, 421.59, 92.04, 0.0),
        ),
    ];
    let navigation = DbCreatureNavigationGuardrail {
        path_available: true,
        world_data_files: data.clone(),
        ..DbCreatureNavigationGuardrail::default()
    };
    let geometry = WorldGeometry::new(data.clone());

    for (start, stormwind) in samples {
        let Some((start_tile_x, start_tile_y)) = mmap_tile_for_position(start) else {
            panic!("observed route-edge position should resolve to a mmap tile");
        };
        let Some((target_tile_x, target_tile_y)) = mmap_tile_for_position(stormwind) else {
            panic!("Stormwind travel target should resolve to a mmap tile");
        };
        if !data.has_mmap_tile(0, start_tile_x, start_tile_y)
            || !data.has_mmap_tile(0, target_tile_x, target_tile_y)
        {
            continue;
        }

        let route = playerbot_route_points(
            &navigation,
            Some(&geometry),
            start,
            Some(stormwind),
            start,
            0,
            2,
        )
        .expect("observed route-edge position should recover a follow-up route");

        assert!(
            route
                .last()
                .is_some_and(|point| point.distance_2d(&stormwind) < start.distance_2d(&stormwind)),
            "recovered route from {start:?} should continue making progress toward {stormwind:?}"
        );
    }
}

#[test]
fn map_runtime_playerbot_stormwind_travel_broadcasts_visible_movement_when_available() {
    let data = Arc::new(WorldDataFiles::inspect("C:/World of Warcraft Classic"));
    let bot_position = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);
    let stormwind = WorldPosition::new(0, -9_095.62, 422.026, 92.0445, 0.0);
    let Some((start_tile_x, start_tile_y)) = mmap_tile_for_position(bot_position) else {
        panic!("Northshire bot position should resolve to a mmap tile");
    };
    let Some((target_tile_x, target_tile_y)) = mmap_tile_for_position(stormwind) else {
        panic!("Stormwind travel target should resolve to a mmap tile");
    };
    if !data.has_mmap_tile(0, start_tile_x, start_tile_y)
        || !data.has_mmap_tile(0, target_tile_x, target_tile_y)
    {
        return;
    }

    let navigation = DbCreatureNavigationGuardrail {
        path_available: true,
        world_data_files: data.clone(),
        ..DbCreatureNavigationGuardrail::default()
    };
    let mut map = MapRuntime::with_geometry(
        0,
        0,
        Arc::new(WorldGeometry::new(data)),
        Arc::new(DbScriptRegistry::default()),
    );
    let client_session = SessionId(77);
    map.add_player(test_player_runtime(
        1,
        client_session,
        WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
    ))
    .unwrap();
    map.add_player(test_bot_player_runtime(2, BotId(1), bot_position))
        .unwrap();
    {
        let bot = map
            .players
            .get_mut(&2)
            .unwrap()
            .bot_runtime
            .as_mut()
            .unwrap();
        bot.next_think_at = Instant::now();
        bot.travel_destination = Some(stormwind);
    }

    let tick = map
        .advance_playerbot_movement_tick(&navigation, Instant::now())
        .unwrap();

    assert_eq!(tick.advanced_bots, 1);
    assert!(
        tick.packets.iter().any(|(session_id, packet)| {
            *session_id == client_session
                && packet.opcode == WorldOpcode::MsgMoveStartForward as u16
        }),
        "visible Stormwind travel should broadcast a client movement start"
    );
}

#[test]
fn map_runtime_playerbot_movement_budget_is_fair_to_later_bot_guids() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let client_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let bot_position = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);

    map.add_player(test_player_runtime(1, SessionId(77), client_position))
        .unwrap();
    for offset in 0..(PLAYERBOT_MAX_MOVES_PER_MAP_TICK as u32 + 4) {
        let guid = 10_000 + offset;
        map.add_player(test_bot_player_runtime(
            guid,
            BotId(guid as u64),
            bot_position,
        ))
        .unwrap();
        map.players
            .get_mut(&guid)
            .unwrap()
            .bot_runtime
            .as_mut()
            .unwrap()
            .next_think_at = now;
        map.players
            .get_mut(&guid)
            .unwrap()
            .bot_runtime
            .as_mut()
            .unwrap()
            .route = vec![playerbot_roam_destination(bot_position, guid, 0)];
    }

    let first_tick = map
        .advance_playerbot_movement_tick(&DbCreatureNavigationGuardrail::default(), now)
        .unwrap();
    assert_eq!(
        first_tick.advanced_bots as usize,
        PLAYERBOT_MAX_MOVES_PER_MAP_TICK
    );
    assert!(first_tick.budget_exhausted);

    let second_tick = map
        .advance_playerbot_movement_tick(
            &DbCreatureNavigationGuardrail::default(),
            now + Duration::from_millis(100),
        )
        .unwrap();
    assert!(second_tick.advanced_bots >= 4);

    for guid in (10_000 + PLAYERBOT_MAX_MOVES_PER_MAP_TICK as u32)
        ..(10_000 + PLAYERBOT_MAX_MOVES_PER_MAP_TICK as u32 + 4)
    {
        assert!(
            map.players
                .get(&guid)
                .unwrap()
                .bot_runtime
                .as_ref()
                .unwrap()
                .active_leg
                .is_some(),
            "later bot guid {guid} should receive movement time on the next tick"
        );
    }
}

#[test]
fn map_runtime_playerbot_route_planning_is_budgeted() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let bot_position = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);
    let travel_destination = WorldPosition::new(0, -8940.0, -110.0, 83.5, 0.0);

    for offset in 0..(PLAYERBOT_MAX_ROUTE_PLANS_PER_MAP_TICK as u32 + 3) {
        let guid = 20_000 + offset;
        map.add_player(test_bot_player_runtime(
            guid,
            BotId(guid as u64),
            bot_position,
        ))
        .unwrap();
        let bot = map
            .players
            .get_mut(&guid)
            .unwrap()
            .bot_runtime
            .as_mut()
            .unwrap();
        bot.next_think_at = now;
        bot.travel_destination = Some(travel_destination);
    }

    let tick = map
        .advance_playerbot_movement_tick(&DbCreatureNavigationGuardrail::default(), now)
        .unwrap();
    assert_eq!(
        tick.advanced_bots as usize,
        PLAYERBOT_MAX_ROUTE_PLANS_PER_MAP_TICK
    );
    assert!(!tick.budget_exhausted);

    for guid in 20_000..(20_000 + PLAYERBOT_MAX_ROUTE_PLANS_PER_MAP_TICK as u32) {
        assert!(
            map.players
                .get(&guid)
                .unwrap()
                .bot_runtime
                .as_ref()
                .unwrap()
                .active_leg
                .is_some(),
            "route-planning budget should admit lower due/guid bot {guid}"
        );
    }
    for guid in (20_000 + PLAYERBOT_MAX_ROUTE_PLANS_PER_MAP_TICK as u32)
        ..(20_000 + PLAYERBOT_MAX_ROUTE_PLANS_PER_MAP_TICK as u32 + 3)
    {
        let bot = map
            .players
            .get(&guid)
            .unwrap()
            .bot_runtime
            .as_ref()
            .unwrap();
        assert!(bot.active_leg.is_none());
        assert!(bot.route.is_empty());
        assert!(bot.next_think_at > now);
    }
}

#[test]
fn map_runtime_playerbot_random_roam_planning_is_not_nav_budget_limited() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let client_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let bot_position = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);

    map.add_player(test_player_runtime(1, SessionId(77), client_position))
        .unwrap();
    for offset in 0..(PLAYERBOT_MAX_ROUTE_PLANS_PER_MAP_TICK as u32 + 16) {
        let guid = 30_000 + offset;
        map.add_player(test_bot_player_runtime(
            guid,
            BotId(guid as u64),
            bot_position,
        ))
        .unwrap();
        map.players
            .get_mut(&guid)
            .unwrap()
            .bot_runtime
            .as_mut()
            .unwrap()
            .next_think_at = now;
    }

    let tick = map
        .advance_playerbot_movement_tick(&DbCreatureNavigationGuardrail::default(), now)
        .unwrap();

    assert_eq!(
        tick.advanced_bots,
        PLAYERBOT_MAX_ROUTE_PLANS_PER_MAP_TICK as u32 + 16
    );
    assert!(!tick.budget_exhausted);
    for guid in 30_000..(30_000 + PLAYERBOT_MAX_ROUTE_PLANS_PER_MAP_TICK as u32 + 16) {
        assert!(
            map.players
                .get(&guid)
                .unwrap()
                .bot_runtime
                .as_ref()
                .unwrap()
                .active_leg
                .is_some(),
            "random roam bot {guid} should start moving without consuming scarce nav budget"
        );
    }
}

#[test]
fn map_runtime_force_active_playerbot_moves_without_client_interest() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let bot_position = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);

    map.add_player(test_bot_player_runtime(2, BotId(1), bot_position))
        .unwrap();
    let bot = map
        .players
        .get_mut(&2)
        .unwrap()
        .bot_runtime
        .as_mut()
        .unwrap();
    bot.force_active = true;
    bot.next_think_at = now;

    let tick = map
        .advance_playerbot_movement_tick(&DbCreatureNavigationGuardrail::default(), now)
        .unwrap();

    assert_eq!(tick.advanced_bots, 1);
    assert!(
        map.players
            .get(&2)
            .unwrap()
            .bot_runtime
            .as_ref()
            .unwrap()
            .active_leg
            .is_some(),
        "force-active bots should keep roaming even without nearby client players"
    );
}

#[test]
fn map_runtime_combat_disabled_playerbot_skips_combat_planning() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let bot_position = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);

    map.add_player(test_bot_player_runtime(2, BotId(1), bot_position))
        .unwrap();
    let bot = map
        .players
        .get_mut(&2)
        .unwrap()
        .bot_runtime
        .as_mut()
        .unwrap();
    bot.force_active = true;
    bot.combat_enabled = false;
    bot.next_think_at = now;
    bot.next_combat_think_at = now;

    let inputs = map.collect_playerbot_plan_inputs(now);

    assert_eq!(inputs.len(), 1);
    assert!(inputs[0].movement_due_at.is_some());
    assert!(inputs[0].combat_due_at.is_none());
    let tick = map
        .advance_playerbot_combat_tick(
            &FactionTemplateStore::fallback_bridge(),
            &DbCreatureNavigationGuardrail::default(),
            now,
        )
        .unwrap();
    assert_eq!(tick.advanced_bots, 0);
    assert_eq!(tick.creature_swings, 0);
}

#[test]
fn map_runtime_local_roam_only_playerbot_skips_planner_inputs_and_still_moves() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let bot_position = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);

    map.add_player(test_bot_player_runtime(2, BotId(1), bot_position))
        .unwrap();
    let bot = map
        .players
        .get_mut(&2)
        .unwrap()
        .bot_runtime
        .as_mut()
        .unwrap();
    bot.force_active = true;
    bot.local_roam_only = true;
    bot.combat_enabled = false;
    bot.next_think_at = now;
    bot.next_combat_think_at = now;

    let inputs = map.collect_playerbot_plan_inputs(now);

    assert!(
        inputs.is_empty(),
        "local-roam-only perf bots should not enter the planner input queue"
    );

    let tick = map
        .advance_playerbot_movement_tick(&DbCreatureNavigationGuardrail::default(), now)
        .unwrap();
    assert_eq!(tick.advanced_bots, 1);
    assert!(
        map.players
            .get(&2)
            .unwrap()
            .bot_runtime
            .as_ref()
            .unwrap()
            .active_leg
            .is_some(),
        "local-roam-only perf bots should still begin roaming from the movement tick"
    );
}

#[test]
fn map_runtime_playerbot_route_failure_backs_off() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let bot_position = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);
    let unreachable_destination = WorldPosition::new(1, -8950.0, -132.0, 83.5, 0.0);

    map.add_player(test_bot_player_runtime(2, BotId(1), bot_position))
        .unwrap();
    let bot = map
        .players
        .get_mut(&2)
        .unwrap()
        .bot_runtime
        .as_mut()
        .unwrap();
    bot.next_think_at = now;
    bot.travel_destination = Some(unreachable_destination);

    let tick = map
        .advance_playerbot_movement_tick(&DbCreatureNavigationGuardrail::default(), now)
        .unwrap();
    let bot = map.players.get(&2).unwrap().bot_runtime.as_ref().unwrap();

    assert_eq!(tick.advanced_bots, 0);
    assert!(tick.packets.is_empty());
    assert!(bot.active_leg.is_none());
    assert!(bot.route.is_empty());
    assert!(
        bot.next_think_at >= now + Duration::from_millis(PLAYERBOT_ROUTE_PLAN_FAILED_RETRY_MILLIS)
    );
}

#[test]
fn map_runtime_playerbot_travel_arrival_sleeps_without_route_failure() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let bot_position = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);

    map.add_player(test_bot_player_runtime(2, BotId(1), bot_position))
        .unwrap();
    let bot = map
        .players
        .get_mut(&2)
        .unwrap()
        .bot_runtime
        .as_mut()
        .unwrap();
    bot.next_think_at = now;
    bot.travel_destination = Some(bot_position);

    let tick = map
        .advance_playerbot_movement_tick(&DbCreatureNavigationGuardrail::default(), now)
        .unwrap();
    let bot = map.players.get(&2).unwrap().bot_runtime.as_ref().unwrap();

    assert_eq!(tick.advanced_bots, 0);
    assert!(tick.packets.is_empty());
    assert!(bot.active_leg.is_none());
    assert!(bot.route.is_empty());
    assert!(
        bot.next_think_at >= now + PLAYERBOT_TRAVEL_ARRIVED_RECHECK_INTERVAL,
        "arrived travel bots should not compete with active travelers for route-planning budget"
    );
}

#[test]
fn playerbot_route_compaction_preserves_turns_and_final_destination() {
    let start = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    let points = vec![
        WorldPosition::new(0, 2.0, 0.0, 0.0, 0.0),
        WorldPosition::new(0, 8.0, 0.0, 0.0, 0.0),
        WorldPosition::new(0, 12.0, 0.0, 0.0, 0.0),
        WorldPosition::new(0, 12.0, 8.0, 0.0, 0.0),
        WorldPosition::new(0, 12.0, 20.0, 0.0, 0.0),
    ];

    let compact = playerbot_compact_route_points(start, points);

    assert!(
        compact.iter().any(
            |point| point.distance_2d(&WorldPosition::new(0, 12.0, 0.0, 0.0, 0.0))
                <= PLAYERBOT_DESTINATION_EPSILON_YARDS
        ),
        "the right-angle path corner should stay in the compacted route"
    );
    assert_eq!(
        compact.last().copied(),
        Some(WorldPosition::new(0, 12.0, 20.0, 0.0, 0.0))
    );
    assert!(
        compact.len() < 5,
        "dense straight intermediate points should be folded into longer movement legs"
    );
}

#[test]
fn map_runtime_playerbot_travel_advances_without_real_client_observers() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let bot_position = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);
    let destination = WorldPosition::new(0, -8940.0, -132.0, 83.5, 0.0);

    map.add_player(test_bot_player_runtime(2, BotId(1), bot_position))
        .unwrap();
    let bot = map
        .players
        .get_mut(&2)
        .unwrap()
        .bot_runtime
        .as_mut()
        .unwrap();
    bot.next_think_at = now;
    bot.travel_destination = Some(destination);
    bot.route = vec![destination];

    let start_tick = map
        .advance_playerbot_movement_tick(&DbCreatureNavigationGuardrail::default(), now)
        .unwrap();
    assert_eq!(start_tick.advanced_bots, 1);
    assert!(start_tick.packets.is_empty());

    let stop_tick = map
        .advance_playerbot_movement_tick(
            &DbCreatureNavigationGuardrail::default(),
            now + Duration::from_secs(2),
        )
        .unwrap();
    assert_eq!(stop_tick.advanced_bots, 1);
    assert!(stop_tick.packets.is_empty());
    assert_eq!(map.players.get(&2).unwrap().position, destination);
}

#[test]
fn map_runtime_playerbot_movement_updates_cell_bucket_without_grid_interest() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let home = find_position_with_playerbot_roam_cell_change();
    let old_cell = cell_coord_for_position(home);
    let old_grid = grid_coord_for_position(home);
    let client_session = SessionId(99);
    map.add_player(test_player_runtime(1, client_session, home))
        .unwrap();
    map.add_player(test_bot_player_runtime(2, BotId(1), home))
        .unwrap();
    map.players
        .get_mut(&2)
        .unwrap()
        .bot_runtime
        .as_mut()
        .unwrap()
        .next_think_at = now;
    let destination = playerbot_roam_destination(home, 2, 0);
    map.players
        .get_mut(&2)
        .unwrap()
        .bot_runtime
        .as_mut()
        .unwrap()
        .route = vec![destination];
    let active_count_before = map.grids.get(&old_grid).unwrap().active_player_count;

    let mut tick = map
        .advance_playerbot_movement_tick(&DbCreatureNavigationGuardrail::default(), now)
        .unwrap();
    let mut moved = map.players.get(&2).unwrap().position;
    for step in 1..=300 {
        if cell_coord_for_position(moved) != old_cell {
            break;
        }
        tick = map
            .advance_playerbot_movement_tick(
                &DbCreatureNavigationGuardrail::default(),
                now + Duration::from_millis(step * 100),
            )
            .unwrap();
        moved = map.players.get(&2).unwrap().position;
    }
    let new_cell = cell_coord_for_position(moved);
    let new_grid = grid_coord_for_position(moved);

    assert!(tick.advanced_bots >= 1);
    assert_ne!(old_cell, new_cell);
    assert!(!map
        .grids
        .get(&old_grid)
        .and_then(|grid| grid.cells.get(&old_cell))
        .is_some_and(|cell| cell.players.contains(&2)));
    assert!(map
        .grids
        .get(&new_grid)
        .and_then(|grid| grid.cells.get(&new_cell))
        .is_some_and(|cell| cell.players.contains(&2)));
    assert_eq!(
        map.grids.get(&new_grid).unwrap().active_player_count,
        active_count_before
    );
}

#[test]
fn map_runtime_playerbot_movement_sleeps_without_client_grid_interest() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let position = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);
    map.add_player(test_bot_player_runtime(2, BotId(1), position))
        .unwrap();
    map.players
        .get_mut(&2)
        .unwrap()
        .bot_runtime
        .as_mut()
        .unwrap()
        .next_think_at = now;

    let tick = map
        .advance_playerbot_movement_tick(&DbCreatureNavigationGuardrail::default(), now)
        .unwrap();

    assert_eq!(tick.advanced_bots, 0);
    assert!(tick.packets.is_empty());
    assert_eq!(map.players.get(&2).unwrap().position, position);
}

#[test]
fn playerbot_roster_answers_name_query() {
    let mut roster = PlayerbotRoster::default();
    roster.insert(PlayerbotRosterEntry {
        guid: 9000001,
        name: "Northshirebot".to_string(),
        race: 1,
        gender: 0,
        class: 1,
    });

    let query = roster.name_query(9000001).expect("bot name query");
    assert_eq!(query.name, "Northshirebot");
    assert_eq!(query.race, 1);
    assert_eq!(query.class, 1);
    assert!(roster.name_query(42).is_none());
}

fn find_position_with_playerbot_roam_cell_change() -> WorldPosition {
    let base = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);
    (0..200)
        .map(|step| WorldPosition::new(0, base.x + step as f32 * 0.25, base.y, base.z, 0.0))
        .find(|position| {
            let destination = playerbot_roam_destination(*position, 2, 0);
            grid_coord_for_position(*position) == grid_coord_for_position(destination)
                && cell_coord_for_position(*position) != cell_coord_for_position(destination)
        })
        .expect("test should find a nearby position crossing a cell boundary")
}
