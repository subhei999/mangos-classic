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
            opcode: WorldOpcode::SmsgUpdateObject as u16,
            body: vec![1, 2, 3],
        },
    );

    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0].0, SessionId(2));
    assert_eq!(packets[0].1.opcode, WorldOpcode::SmsgUpdateObject as u16);
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
    assert!(map
        .open_db_creature_loot(guid, 1, CreatureLootOwner::Player(1), None, Vec::new())
        .is_some());

    let first = map.take_db_creature_loot_money(1);
    let second = map.take_db_creature_loot_money(1);

    assert_eq!(first.map(|(money, _)| money), Some(7));
    assert!(second.is_none());
    assert!(!map.creatures.get(&guid).unwrap().loot_money_available);
}

#[test]
fn map_runtime_open_db_creature_loot_extends_corpse_decay_while_looting() {
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 45;
    spawn.spawn_time_secs_min = 15;
    spawn.spawn_time_secs_max = 15;
    let guid = creature_spawn_guid(&spawn).raw();
    let mut creature = DbCreatureRuntime::new(spawn);
    let death_at = Instant::now();
    creature.begin_corpse(death_at, 1_000);
    let original_expiry = creature.corpse_expires_at.expect("corpse expiry");
    let mut map = MapRuntime::new(0, 0);
    map.share_db_creature_snapshots(vec![creature]);

    let before_open = Instant::now();
    let opened = map
        .open_db_creature_loot(guid, 1, CreatureLootOwner::Player(1), None, Vec::new())
        .expect("loot should open");
    let after_open = Instant::now();

    let extended_expiry = opened.corpse_expires_at.expect("extended corpse expiry");
    assert!(extended_expiry > original_expiry);
    assert!(extended_expiry >= before_open + Duration::from_secs(120));
    assert!(extended_expiry <= after_open + Duration::from_secs(120));
    assert!(
        map.loaded_db_creature_lifecycle_guids(after_open + Duration::from_secs(20))
            .is_empty(),
        "open loot should requeue corpse expiry past the old short respawn-driven deadline"
    );
}

#[test]
fn map_runtime_db_creature_loot_owner_blocks_unrelated_players() {
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 46;
    let guid = creature_spawn_guid(&spawn).raw();
    let mut creature = DbCreatureRuntime::new(spawn);
    creature.begin_corpse(Instant::now(), 1_000);
    creature.loot_owner = Some(CreatureLootOwner::Player(1));
    let mut map = MapRuntime::new(0, 0);
    map.share_db_creature_snapshots(vec![creature]);

    assert!(map
        .open_db_creature_loot(guid, 2, CreatureLootOwner::Player(2), None, Vec::new())
        .is_none());
    assert!(map
        .open_db_creature_loot(guid, 1, CreatureLootOwner::Player(1), None, Vec::new())
        .is_some());
}

#[tokio::test]
async fn unauthorized_creature_loot_request_sends_cmangos_loot_error() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let parties = PartyManager::default();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 46;
    let target = creature_spawn_guid(&spawn);
    let mut creature = DbCreatureRuntime::new(spawn);
    creature.begin_corpse(Instant::now(), 1_000);
    creature.loot_owner = Some(CreatureLootOwner::Player(1));
    creature.loot_items_generated = true;
    creature.loot_money_available = false;
    creature.loot_items = vec![DbCreatureLootRuntime {
        slot: 0,
        item: 117,
        count: 1,
        display_id: 641,
        quality: 1,
        free_for_all: false,
        quest_drop: false,
    }];
    maps.share_db_creature_snapshots(0, vec![creature]).await;
    let mut body = Vec::new();
    body.extend_from_slice(&target.raw().to_le_bytes());
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 2,
                name: "Alt".to_string(),
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
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);

    handle_loot(
        &mut stream,
        &world_db_pool,
        shared_world,
        &parties,
        read_loot_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    let response = packets
        .iter()
        .find(|packet| packet.opcode == WorldOpcode::SmsgLootResponse as u16)
        .expect("unauthorized corpse loot should receive a client-clearing loot error");
    assert_eq!(&response.body[0..8], &target.raw().to_le_bytes());
    assert_eq!(response.body[8], 0);
    assert_eq!(response.body[9], LOOT_ERROR_DIDNT_KILL);
    assert!(maps
        .db_creature_loot_guid_for_character(0, 2)
        .await
        .is_none());
}

#[tokio::test]
async fn creature_loot_open_and_release_toggle_player_looting_for_observers() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let parties = PartyManager::default();
    let looter_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    let observer_session = SessionId(2);
    maps.add_player(test_player_runtime(1, SessionId(1), looter_position))
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
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 46;
    spawn.position_x = looter_position.x + 3.0;
    spawn.position_y = looter_position.y;
    let target = creature_spawn_guid(&spawn);
    let mut creature = DbCreatureRuntime::new(spawn);
    creature.begin_corpse(Instant::now(), 1_000);
    creature.loot_owner = Some(CreatureLootOwner::Player(1));
    creature.loot_items_generated = true;
    creature.loot_money_available = false;
    creature.loot_items = vec![DbCreatureLootRuntime {
        slot: 0,
        item: 117,
        count: 1,
        display_id: 641,
        quality: 1,
        free_for_all: false,
        quest_drop: false,
    }];
    maps.share_db_creature_snapshots(0, vec![creature]).await;
    let mut body = Vec::new();
    body.extend_from_slice(&target.raw().to_le_bytes());
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 1,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position: looter_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);

    handle_loot(
        &mut stream,
        &world_db_pool,
        shared_world,
        &parties,
        read_loot_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let player = ObjectGuid::new(HighGuid::Player, 0, 1);
    let self_open = rx.try_recv().unwrap();
    assert_eq!(self_open.opcode, WorldOpcode::SmsgUpdateObject as u16);
    let (values, trailing) = decode_values_update_block(&self_open.body[5..], player);
    assert!(trailing.is_empty());
    assert_eq!(
        values[UNIT_FIELD_FLAGS],
        Some(UNIT_FLAG_PLAYER_CONTROLLED | UNIT_FLAG_LOOTING)
    );
    assert_eq!(
        rx.try_recv().unwrap().opcode,
        WorldOpcode::SmsgLootResponse as u16
    );
    let observer_open = observer_rx.try_recv().unwrap();
    let (values, trailing) = decode_values_update_block(&observer_open.body[5..], player);
    assert!(trailing.is_empty());
    assert_eq!(
        values[UNIT_FIELD_FLAGS],
        Some(UNIT_FLAG_PLAYER_CONTROLLED | UNIT_FLAG_LOOTING)
    );

    handle_loot_release(
        &mut stream,
        shared_world,
        read_loot_release_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let self_close = rx.try_recv().unwrap();
    assert_eq!(self_close.opcode, WorldOpcode::SmsgUpdateObject as u16);
    let (values, trailing) = decode_values_update_block(&self_close.body[5..], player);
    assert!(trailing.is_empty());
    assert_eq!(values[UNIT_FIELD_FLAGS], Some(UNIT_FLAG_PLAYER_CONTROLLED));
    assert_eq!(
        rx.try_recv().unwrap().opcode,
        WorldOpcode::SmsgLootReleaseResponse as u16
    );
    assert_eq!(
        rx.try_recv().unwrap().opcode,
        WorldOpcode::SmsgUpdateObject as u16
    );
    let observer_close = observer_rx.try_recv().unwrap();
    let (values, trailing) = decode_values_update_block(&observer_close.body[5..], player);
    assert!(trailing.is_empty());
    assert_eq!(values[UNIT_FIELD_FLAGS], Some(UNIT_FLAG_PLAYER_CONTROLLED));
}

#[test]
fn map_runtime_db_creature_party_loot_owner_allows_party_members() {
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 47;
    let guid = creature_spawn_guid(&spawn).raw();
    let mut creature = DbCreatureRuntime::new(spawn);
    creature.begin_corpse(Instant::now(), 1_000);
    creature.loot_owner = Some(CreatureLootOwner::Party(9));
    let mut map = MapRuntime::new(0, 0);
    map.share_db_creature_snapshots(vec![creature]);

    assert!(map
        .open_db_creature_loot(guid, 2, CreatureLootOwner::Party(9), None, Vec::new())
        .is_some());
    assert!(map
        .open_db_creature_loot(guid, 3, CreatureLootOwner::Party(10), None, Vec::new())
        .is_none());
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
        slot: 0,
        item: 117,
        count: 1,
        display_id: 117,
        quality: 0,
        free_for_all: false,
        quest_drop: false,
    };
    let mut map = MapRuntime::new(0, 0);
    map.share_db_creature_snapshots(vec![creature]);
    assert!(map
        .open_db_creature_loot(
            guid,
            1,
            CreatureLootOwner::Player(1),
            None,
            vec![loot.clone()]
        )
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
fn map_runtime_release_by_current_looter_releases_under_threshold_group_loot_to_party() {
    let spawn = CreatureSpawnQuery {
        guid: 46,
        entry: 6,
        ..test_creature_spawn(6)
    };
    let guid = creature_spawn_guid(&spawn).raw();
    let mut creature = DbCreatureRuntime::new(spawn);
    creature.begin_corpse(Instant::now(), 1_000);
    creature.loot_owner = Some(CreatureLootOwner::Party(7));
    creature.loot_allowed_players = HashSet::from([1, 2]);
    creature.loot_method = Some(CreatureLootMethod {
        method: 3,
        threshold: 2,
        master_looter: 1,
    });
    let loot = DbCreatureLootRuntime {
        slot: 0,
        item: 117,
        count: 1,
        display_id: 117,
        quality: 1,
        free_for_all: false,
        quest_drop: false,
    };

    let mut map = MapRuntime::new(0, 0);
    map.share_db_creature_snapshots(vec![creature]);
    assert!(map
        .open_db_creature_loot(guid, 1, CreatureLootOwner::Party(7), Some(1), vec![loot])
        .is_some());

    let released = map
        .release_db_creature_loot(guid, Instant::now(), Some(1))
        .unwrap()
        .expect("current looter release should update corpse");
    assert!(released
        .creature
        .loot_current_looter_pass_slots
        .contains(&0));
    assert!(released.creature.can_loot_for_player(Some(2)));

    let reopened = map
        .open_db_creature_loot(guid, 2, CreatureLootOwner::Party(7), Some(1), Vec::new())
        .expect("party member should reopen released under-threshold corpse loot");
    let body = build_db_creature_loot_response_body_for_player(
        reopened.guid(),
        &reopened,
        db_creature_loot_method_tuple(reopened.loot_method),
        2,
    );
    assert_eq!(body[13], 1);
    assert_eq!(body[35], LOOT_SLOT_NORMAL);
}

#[test]
fn map_runtime_db_creature_loot_slots_stay_stable_after_top_claim() {
    let spawn = CreatureSpawnQuery {
        guid: 45,
        entry: 6,
        ..test_creature_spawn(6)
    };
    let guid = creature_spawn_guid(&spawn).raw();
    let mut creature = DbCreatureRuntime::new(spawn);
    creature.begin_corpse(Instant::now(), 1_000);
    let mut map = MapRuntime::new(0, 0);
    map.share_db_creature_snapshots(vec![creature]);
    map.open_db_creature_loot(
        guid,
        1,
        CreatureLootOwner::Player(1),
        None,
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
    .expect("creature loot should open");

    assert_eq!(
        map.take_db_creature_loot_item(1, 0)
            .map(|(_, slot, loot, _)| (slot, loot.item)),
        Some((0, 117))
    );
    assert_eq!(
        map.take_db_creature_loot_item(1, 1)
            .map(|(_, slot, loot, _)| (slot, loot.item)),
        Some((1, 118))
    );
    assert_eq!(
        map.take_db_creature_loot_item(1, 2)
            .map(|(_, slot, loot, _)| (slot, loot.item)),
        Some((2, 119))
    );
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
        item: 159,
        count: 1,
        display_id: 159,
        quality: 0,
        free_for_all: false,
        quest_drop: false,
    };
    let mut map = MapRuntime::new(0, 0);
    map.share_db_creature_snapshots(vec![creature]);

    assert_eq!(map.db_creature_needs_loot_item(guid), Some(true));
    let opened = map
        .open_db_creature_loot(
            guid,
            1,
            CreatureLootOwner::Player(1),
            None,
            vec![first_loot.clone()],
        )
        .unwrap();
    assert_eq!(opened.loot_items.first().map(|loot| loot.item), Some(117));
    assert_eq!(map.db_creature_needs_loot_item(guid), Some(false));

    let reopened = map
        .open_db_creature_loot(
            guid,
            1,
            CreatureLootOwner::Player(1),
            None,
            vec![second_loot],
        )
        .unwrap();
    assert_eq!(reopened.loot_items.first().map(|loot| loot.item), Some(117));
}

#[test]
fn map_runtime_tracks_all_open_db_creature_looters_for_ui_fanout() {
    let mut map = MapRuntime::new(0, 0);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 512;
    let guid = creature_spawn_guid(&spawn).raw();
    let mut creature = DbCreatureRuntime::new(spawn);
    creature.begin_corpse(Instant::now(), 1_000);
    map.share_db_creature_snapshots(vec![creature]);

    assert!(map
        .open_db_creature_loot(guid, 1, CreatureLootOwner::Party(7), Some(1), Vec::new())
        .is_some());
    assert!(map
        .open_db_creature_loot(guid, 2, CreatureLootOwner::Party(7), Some(1), Vec::new())
        .is_some());

    let mut looters = map.db_creature_looting_characters(guid);
    looters.sort_unstable();
    assert_eq!(looters, vec![1, 2]);
}

#[tokio::test]
async fn loot_removed_fanout_notifies_all_open_non_targets_after_master_assignment() {
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 513;
    let guid = creature_spawn_guid(&spawn).raw();
    let mut creature = DbCreatureRuntime::new(spawn);
    creature.begin_corpse(Instant::now(), 1_000);
    maps.share_db_creature_snapshots(0, vec![creature]).await;

    assert!(maps
        .open_db_creature_loot(0, guid, 1, CreatureLootOwner::Party(7), Some(1), Vec::new())
        .await
        .is_some());
    assert!(maps
        .open_db_creature_loot(0, guid, 2, CreatureLootOwner::Party(7), Some(1), Vec::new())
        .await
        .is_some());
    assert!(maps
        .open_db_creature_loot(0, guid, 3, CreatureLootOwner::Party(7), Some(1), Vec::new())
        .await
        .is_some());

    let (master_tx, mut master_rx) = mpsc::unbounded_channel();
    let (target_tx, mut target_rx) = mpsc::unbounded_channel();
    let (viewer_tx, mut viewer_rx) = mpsc::unbounded_channel();
    sessions
        .register(
            SessionId(1),
            SessionHandle {
                account_id: 1,
                character_guid: Some(1),
                character_name: Some("Master".to_string()),
                outbound: WorldPacketSender::Unbounded(master_tx),
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
                character_name: Some("Target".to_string()),
                outbound: WorldPacketSender::Unbounded(target_tx),
                disconnect: None,
            },
        )
        .await;
    sessions
        .register(
            SessionId(3),
            SessionHandle {
                account_id: 3,
                character_guid: Some(3),
                character_name: Some("Viewer".to_string()),
                outbound: WorldPacketSender::Unbounded(viewer_tx),
                disconnect: None,
            },
        )
        .await;

    dispatch_creature_loot_removed_to_other_open_looters(
        SharedWorldDeps {
            object_mgr: &object_mgr,
            maps: &maps,
            sessions: &sessions,
        },
        0,
        guid,
        2,
        4,
    )
    .await;

    let packet = master_rx.try_recv().expect("master looter gets UI removal");
    assert_eq!(packet.opcode, WorldOpcode::SmsgLootRemoved as u16);
    assert_eq!(packet.body, vec![4]);
    let packet = viewer_rx.try_recv().expect("other viewer gets UI removal");
    assert_eq!(packet.opcode, WorldOpcode::SmsgLootRemoved as u16);
    assert_eq!(packet.body, vec![4]);
    assert!(target_rx.try_recv().is_err());
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
    assert_eq!(
        event.direct_packet.opcode,
        WorldOpcode::SmsgUpdateObject as u16
    );
    assert!(!event.direct_packet.body.is_empty());
    assert_eq!(event.observer_packets.len(), 1);
    assert_eq!(event.observer_packets[0].0, SessionId(2));
    assert_eq!(
        event.observer_packets[0].1.opcode,
        WorldOpcode::SmsgUpdateObject as u16
    );
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
fn map_runtime_db_creature_death_clears_player_combat_for_regen() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    let mut player = test_player_runtime(7, SessionId(7), position);
    player.class = 1;
    player.spirit = 40;
    player.health = 40;
    player.max_health = 80;
    let world_stats = PlayerWorldStats {
        base_health: 80,
        base_mana: 0,
        stats: [20, 20, 20, 20, 40],
        next_level_xp: 400,
    };
    player.base_world_stats = world_stats;
    player.effective_world_stats = world_stats;
    player.power2 = 100;
    map.add_player(player).unwrap();

    let mut spawn = test_creature_spawn(6);
    spawn.guid = 901;
    spawn.position_x = position.x + 1.0;
    spawn.position_y = position.y;
    spawn.template.min_level_health = 20;
    spawn.template.max_level_health = 20;
    let attacker = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);

    let victim = ObjectGuid::new(HighGuid::Player, 0, 7);
    let now = Instant::now();
    map.begin_db_creature_combat(attacker, victim, now)
        .expect("creature should enter combat with the player");
    assert!(map.player_runtime_snapshot(7).unwrap().in_combat);

    map.apply_db_creature_damage(DbCreatureDamageRequest {
        creature_guid: attacker,
        killer: victim,
        damage: 20,
        melee_outcome: None,
        spell_damage_outcome: None,
        spell_id: None,
        spell_school: 0,
        suppress_attacker_state: false,
        now,
        now_epoch_secs: 0,
        exclude_character_guid: Some(7),
        corpse_loot: None,
    })
    .unwrap()
    .expect("lethal damage should produce an event");

    assert!(!map.player_runtime_snapshot(7).unwrap().in_combat);
    assert!(map.advance_player_regen_tick(now).unwrap().is_empty());
    map.advance_player_regen_tick(now + Duration::from_secs(2))
        .unwrap();

    let snapshot = map.player_runtime_snapshot(7).unwrap();
    assert!(snapshot.health > 40);
    assert!(snapshot.power2 < 100);
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
            spell_damage_outcome: None,
            spell_id: None,
            spell_school: 0,
            suppress_attacker_state: false,
            now,
            now_epoch_secs: 0,
            exclude_character_guid: Some(new_victim.counter()),
            corpse_loot: None,
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
    let mut player = test_player_runtime(7, SessionId(7), player_position);
    player.power2 = 200;
    maps.add_player(player).await.unwrap();

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

#[tokio::test]
async fn map_runtime_manager_advances_db_creature_combats_for_victim_without_session_side_loop() {
    let maps = Arc::new(MapRuntimeManager::default());
    let object_mgr = ObjectMgr::default();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    object_mgr
        .prime_creature_ai_scripts_for_test(0, Vec::new())
        .await;
    let player_position = WorldPosition::new(0, -1.0, 0.0, 0.0, 0.0);
    maps.add_player(test_player_runtime(7, SessionId(7), player_position))
        .await
        .unwrap();

    let mut spawn = test_creature_spawn(6);
    spawn.guid = 336;
    spawn.entry = 0;
    spawn.template.entry = 0;
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.orientation = 0.0;
    let attacker = creature_spawn_guid(&spawn);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(spawn)])
        .await;
    let victim = ObjectGuid::new(HighGuid::Player, 0, 7);
    let now = Instant::now();
    maps.begin_db_creature_combat(0, attacker, victim, now)
        .await
        .expect("map-owned live creature should begin combat");

    let tick = maps
        .advance_db_creature_combats_for_victim(
            &world_db_pool,
            &object_mgr,
            0,
            victim,
            SessionId(7),
            PlayerMeleeDefenseInput {
                level: 1,
                defense_skill: 1,
                armor: 0,
                block_value: 0,
                dodge_percent: 0.0,
                parry_percent: 0.0,
                block_percent: 0.0,
            },
            &DbCreatureNavigationGuardrail::default(),
            now,
        )
        .await
        .unwrap();

    assert!(tick.player_in_combat);
    assert_eq!(tick.active_combats.len(), 1);
    assert!(tick.local_effects.is_empty());
    assert!(
        tick.direct_packets
            .iter()
            .any(|packet| packet.opcode == WorldOpcode::SmsgMonsterMove as u16),
        "manager-owned victim advance should emit the facing update directly"
    );
}

#[tokio::test]
async fn map_runtime_manager_skips_async_planner_for_local_roam_only_perf_bots() {
    let maps = MapRuntimeManager::default();
    let bot_position = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);

    let mut perf_bot = test_bot_player_runtime(2, BotId(1), bot_position);
    {
        let bot = perf_bot.bot_runtime.as_mut().unwrap();
        bot.force_active = true;
        bot.local_roam_only = true;
        bot.combat_enabled = false;
    }
    maps.add_player(perf_bot).await.unwrap();

    assert!(!maps.has_async_playerbot_planner_work());
    assert_eq!(
        maps.planner_driven_playerbot_count
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );

    let tick = maps
        .plan_all_playerbot_intents(&DbCreatureNavigationGuardrail::default(), Instant::now())
        .await
        .unwrap();
    assert_eq!(tick.planned_bots, 0);

    let planner_bot = test_bot_player_runtime(3, BotId(2), bot_position);
    maps.add_player(planner_bot).await.unwrap();

    assert!(maps.has_async_playerbot_planner_work());
    assert_eq!(
        maps.planner_driven_playerbot_count
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );

    maps.remove_player(0, 3).await;
    assert!(!maps.has_async_playerbot_planner_work());
    assert_eq!(
        maps.planner_driven_playerbot_count
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
}

#[tokio::test]
async fn map_runtime_manager_skips_playerbot_ticks_when_world_has_no_playerbots() {
    let maps = MapRuntimeManager::default();
    let position = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);
    maps.add_player(test_player_runtime(2, SessionId(2), position))
        .await
        .unwrap();

    let movement = maps
        .advance_all_playerbot_movement_ticks(
            &DbCreatureNavigationGuardrail::default(),
            Instant::now(),
        )
        .await
        .unwrap();
    assert_eq!(movement.advanced_bots, 0);
    assert!(movement.packets.is_empty());

    let combat = maps
        .advance_all_playerbot_combat_ticks(
            &DbCreatureNavigationGuardrail::default(),
            Instant::now(),
        )
        .await
        .unwrap();
    assert_eq!(combat.advanced_bots, 0);
    assert_eq!(combat.creature_swings, 0);
    assert!(combat.packets.is_empty());
}

#[tokio::test]
async fn map_runtime_manager_playerbot_ticks_do_not_deadlock_when_map_has_playerbots() {
    let maps = MapRuntimeManager::default();
    let bot_position = WorldPosition::new(0, -8950.0, -132.0, 83.5, 0.0);
    let mut bot = test_bot_player_runtime(2, BotId(1), bot_position);
    {
        let bot_runtime = bot.bot_runtime.as_mut().expect("bot runtime");
        bot_runtime.force_active = true;
        bot_runtime.combat_enabled = false;
    }
    maps.add_player(bot).await.unwrap();

    let movement = tokio::time::timeout(
        Duration::from_secs(1),
        maps.advance_all_playerbot_movement_ticks(
            &DbCreatureNavigationGuardrail::default(),
            Instant::now(),
        ),
    )
    .await
    .expect("playerbot movement tick should not deadlock")
    .unwrap();
    assert!(!movement.budget_exhausted);

    let combat = tokio::time::timeout(
        Duration::from_secs(1),
        maps.advance_all_playerbot_combat_ticks(
            &DbCreatureNavigationGuardrail::default(),
            Instant::now(),
        ),
    )
    .await
    .expect("playerbot combat tick should not deadlock")
    .unwrap();
    assert!(!combat.budget_exhausted);
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
            && packet.opcode == WorldOpcode::SmsgDestroyObject as u16));
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
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgAttackerStateUpdate as u16));
    assert!(event
        .observer_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgUpdateObject as u16));
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
            spell_damage_outcome: None,
            spell_id: None,
            spell_school: 0,
            suppress_attacker_state: false,
            now: Instant::now(),
            now_epoch_secs: 0,
            exclude_character_guid: Some(1),
            corpse_loot: None,
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
fn map_runtime_db_creature_melee_rage_damage_uses_preclamp_hit_for_critter() {
    let mut map = MapRuntime::new(0, 0);
    let player = ObjectGuid::new(HighGuid::Player, 0, 1);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 902;
    spawn.template.creature_type = CREATURE_TYPE_CRITTER;
    spawn.template.min_level_health = 1;
    spawn.template.max_level_health = 1;
    let creature_guid = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);

    let event = map
        .apply_db_creature_damage(DbCreatureDamageRequest {
            creature_guid,
            killer: player,
            damage: 30,
            melee_outcome: Some(MeleeDamageOutcome::normal_hit(30)),
            spell_damage_outcome: None,
            spell_id: None,
            spell_school: 0,
            suppress_attacker_state: false,
            now: Instant::now(),
            now_epoch_secs: 0,
            exclude_character_guid: Some(1),
            corpse_loot: None,
        })
        .unwrap()
        .expect("damage event");

    assert_eq!(event.damage, 1);
    assert_eq!(
        event.attacker_rage_damage, 30,
        "CMaNGOS awards attacker rage from the raw melee hit, not remaining hp"
    );
    assert_eq!(event.creature.health, 0);
    assert!(
        rage_gain_from_main_hand_white_damage(
            event.attacker_rage_damage,
            1,
            BASE_ATTACK_TIME_MS,
            MeleeHitOutcome::Normal,
        ) > 0
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
        spell_damage_outcome: None,
        spell_id: None,
        spell_school: 0,
        suppress_attacker_state: false,
        now: refreshed_at,
        now_epoch_secs: current_unix_epoch_secs(),
        exclude_character_guid: None,
        corpse_loot: None,
    })
    .expect("damage apply should succeed")
    .expect("damage event");

    assert!(!map.db_creature_should_evade(attacker, refreshed_at + Duration::from_secs(3),));
    assert!(map.db_creature_should_evade(attacker, refreshed_at + Duration::from_secs(5),));
}

#[test]
fn map_runtime_db_creature_direct_melee_refreshes_leash_timer_while_chasing() {
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
        run: true,
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

    assert!(!map.db_creature_should_evade(attacker, now + Duration::from_secs(16),));
    assert!(map.db_creature_should_evade(attacker, hit_while_chasing_at + Duration::from_secs(16),));
}

#[test]
fn map_runtime_db_creature_periodic_aura_damage_does_not_refresh_leash_timer() {
    let mut map = MapRuntime::new(0, 0);
    let mut attacker_spawn = test_creature_spawn(6);
    attacker_spawn.template.pursuit = 4_000;
    attacker_spawn.template.min_level_health = 100;
    attacker_spawn.template.max_level_health = 100;
    let attacker = creature_spawn_guid(&attacker_spawn);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    let now = Instant::now();
    let victim_position =
        WorldPosition::new(0, DB_CREATURE_LEASH_RADIUS_YARDS + 5.0, 0.0, 0.0, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, victim_position);
    let mut creature = DbCreatureRuntime::new(attacker_spawn);
    creature.active_auras.push(ActiveAura {
        spell_id: 772,
        caster: victim,
        level: 1,
        interrupt_flags: 0,
        positive: false,
        visible: true,
        duration_millis: Some(9_000),
        expires_at: Some(now + Duration::from_secs(9)),
        periodic_damage: Some(PeriodicDamageAura {
            aura_name: SPELL_AURA_PERIODIC_DAMAGE,
            school: 1,
            damage_class: 2,
            attributes_ex2: 0,
            attributes_ex3: 0,
            caster_snapshot: SpellCombatUnitSnapshot {
                level: 1,
                class: 0,
                intellect: 0,
                resistances: [0; MAX_SPELL_SCHOOL],
            },
            amount: 1,
            tick_millis: 3_000,
            next_tick_at: now + Duration::from_secs(3),
        }),
        periodic_regen: None,
        stat_modifiers: Vec::new(),
        proc_triggers: Vec::new(),
    });
    map.creatures.insert(attacker.raw(), creature);
    map.begin_db_creature_combat(attacker, victim, now)
        .expect("combat should start");

    map.advance_db_creature_auras(now + Duration::from_secs(3), 1_000)
        .expect("periodic tick should advance");

    assert_eq!(map.creatures.get(&attacker.raw()).unwrap().health, 99);
    assert!(
        map.db_creature_should_evade(attacker, now + Duration::from_secs(5)),
        "CMaNGOS direct-damage leash refresh does not include periodic aura ticks"
    );
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
            damage: 30,
            melee_outcome: None,
            spell_damage_outcome: Some(SpellDamageOutcome {
                original_damage: 30,
                final_damage: 11,
                absorb: 0,
                resist: 19,
                blocked: 0,
                hit_info: SPELL_HIT_TYPE_CRIT,
                miss_info: None,
            }),
            spell_id: Some(WARRIOR_HEROIC_STRIKE_RANK_1),
            spell_school: 4,
            suppress_attacker_state: false,
            now,
            now_epoch_secs: 2_000,
            exclude_character_guid: Some(1),
            corpse_loot: None,
        })
        .unwrap()
        .expect("spell damage event");

    let spell_log = event
        .spell_non_melee_log_body
        .as_ref()
        .expect("spell damage should include a non-melee log");
    let mut cursor = 0;
    assert_eq!(
        read_packed_guid(spell_log, &mut cursor).unwrap(),
        creature_guid
    );
    assert_eq!(
        read_packed_guid(spell_log, &mut cursor).unwrap(),
        ObjectGuid::new(HighGuid::Player, 0, 1)
    );
    assert_eq!(
        read_u32(spell_log, &mut cursor).unwrap(),
        WARRIOR_HEROIC_STRIKE_RANK_1
    );
    assert_eq!(read_u32(spell_log, &mut cursor).unwrap(), 11);
    assert_eq!(
        spell_log[cursor], 4,
        "map-owned spell damage logs should preserve the spell/effect school"
    );
    cursor += 1;
    assert_eq!(read_u32(spell_log, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(spell_log, &mut cursor).unwrap() as i32, 19);
    cursor += 2;
    assert_eq!(read_u32(spell_log, &mut cursor).unwrap(), 0);
    assert_eq!(
        read_u32(spell_log, &mut cursor).unwrap(),
        SPELL_HIT_TYPE_CRIT
    );
    assert_eq!(event.observer_packets.len(), 3);
    assert!(event
        .observer_packets
        .iter()
        .all(|(session_id, _)| *session_id == SessionId(2)));
    assert!(event
        .observer_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgSpellNonMeleeDamageLog as u16));
    assert!(event
        .observer_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgAttackerStateUpdate as u16));
    assert!(event
        .observer_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgUpdateObject as u16));
}

#[test]
fn map_runtime_db_creature_spell_damage_to_player_uses_shared_outcome_and_logs() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    insert_map_runtime_player_for_test(&mut map, 2, observer_position);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 179;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    let creature_guid = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    let now = Instant::now();
    map.begin_db_creature_combat(creature_guid, victim, now)
        .expect("combat should start");

    let event = map
        .apply_db_creature_player_spell_damage(
            creature_guid,
            victim,
            999_012,
            6,
            1,
            SPELL_DAMAGE_CLASS_MAGIC,
            SPELL_ATTR_EX2_CANT_CRIT,
            TEST_SPELL_ATTR_EX3_ALWAYS_HIT,
            now,
        )
        .unwrap()
        .expect("spell damage should apply");

    assert_eq!(event.damage, 6);
    assert_eq!(event.victim_health, 14);
    assert_eq!(event.outcome.final_damage, 6);
    let spell_log = event
        .spell_non_melee_log_body
        .as_ref()
        .expect("creature spell should produce non-melee log");
    let mut cursor = 0;
    assert_eq!(read_packed_guid(spell_log, &mut cursor).unwrap(), victim);
    assert_eq!(
        read_packed_guid(spell_log, &mut cursor).unwrap(),
        creature_guid
    );
    assert_eq!(read_u32(spell_log, &mut cursor).unwrap(), 999_012);
    assert_eq!(read_u32(spell_log, &mut cursor).unwrap(), 6);
    assert!(event
        .observer_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgSpellNonMeleeDamageLog as u16));
    assert!(event
        .observer_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgUpdateObject as u16));
}

#[test]
fn map_runtime_db_creature_spell_damage_log_reports_runtime_absorb() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    insert_map_runtime_player_for_test(&mut map, 2, observer_position);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 181;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    let creature_guid = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    let now = Instant::now();
    map.begin_db_creature_combat(creature_guid, victim, now)
        .expect("combat should start");
    let player = map.players.get_mut(&1).unwrap();
    player.health = 20;
    player.active_auras.push(ActiveAura {
        spell_id: 11426,
        caster: victim,
        level: 40,
        interrupt_flags: 0,
        positive: true,
        visible: true,
        duration_millis: Some(60_000),
        expires_at: Some(now + Duration::from_secs(60)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::SchoolAbsorb {
            school_mask: 16,
            amount: 20,
        }],
        proc_triggers: Vec::new(),
    });

    let event = map
        .apply_db_creature_player_spell_damage(
            creature_guid,
            victim,
            999_014,
            6,
            16,
            SPELL_DAMAGE_CLASS_MAGIC,
            SPELL_ATTR_EX2_CANT_CRIT,
            TEST_SPELL_ATTR_EX3_ALWAYS_HIT,
            now,
        )
        .unwrap()
        .expect("spell damage should be absorbed");

    assert_eq!(event.damage, 0);
    assert_eq!(event.victim_health, 20);
    let spell_log = event
        .spell_non_melee_log_body
        .as_ref()
        .expect("absorbed creature spell should produce non-melee log");
    let mut cursor = 0;
    assert_eq!(read_packed_guid(spell_log, &mut cursor).unwrap(), victim);
    assert_eq!(
        read_packed_guid(spell_log, &mut cursor).unwrap(),
        creature_guid
    );
    assert_eq!(read_u32(spell_log, &mut cursor).unwrap(), 999_014);
    assert_eq!(
        read_u32(spell_log, &mut cursor).unwrap(),
        0,
        "CMaNGOS sends post-absorb damage in SMSG_SPELLNONMELEEDAMAGELOG"
    );
    cursor += 1;
    assert_eq!(
        read_u32(spell_log, &mut cursor).unwrap(),
        6,
        "runtime school absorbs must be surfaced in the absorb field"
    );
}

#[test]
fn map_runtime_db_creature_melee_damage_packet_reports_runtime_absorb() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    insert_map_runtime_player_for_test(&mut map, 2, observer_position);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 182;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    let creature_guid = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    let now = Instant::now();
    map.begin_db_creature_combat(creature_guid, victim, now)
        .expect("combat should start");
    let player = map.players.get_mut(&1).unwrap();
    player.health = 20;
    player.active_auras.push(ActiveAura {
        spell_id: 1463,
        caster: victim,
        level: 20,
        interrupt_flags: 0,
        positive: true,
        visible: true,
        duration_millis: Some(60_000),
        expires_at: Some(now + Duration::from_secs(60)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::ManaShield {
            school_mask: SPELL_SCHOOL_MASK_NORMAL,
            amount: 20,
            mana_multiplier_millis: 0,
        }],
        proc_triggers: Vec::new(),
    });

    let event = map
        .apply_db_creature_player_melee_outcome(
            creature_guid,
            victim,
            MeleeDamageOutcome::normal_hit(7),
            now,
            now + Duration::from_secs(2),
        )
        .unwrap()
        .expect("melee damage should be absorbed");

    assert_eq!(event.damage, 7);
    assert_eq!(event.victim_health, 20);
    let attacker_state = event
        .observer_packets
        .iter()
        .find_map(|(_, packet)| {
            (packet.opcode == WorldOpcode::SmsgAttackerStateUpdate as u16).then_some(&packet.body)
        })
        .expect("absorbed melee should be broadcast as attacker state");
    let mut cursor = 0;
    assert_eq!(
        read_u32(attacker_state, &mut cursor).unwrap() & HITINFO_ABSORB,
        HITINFO_ABSORB
    );
    assert_eq!(read_packed_guid(attacker_state, &mut cursor).unwrap(), creature_guid);
    assert_eq!(read_packed_guid(attacker_state, &mut cursor).unwrap(), victim);
    assert_eq!(read_u32(attacker_state, &mut cursor).unwrap(), 0);
    cursor += 1 + 4 + 4 + 4;
    assert_eq!(read_u32(attacker_state, &mut cursor).unwrap(), 7);
}

#[tokio::test]
async fn map_runtime_manager_direct_melee_packet_reports_runtime_absorb() {
    let maps = Arc::new(MapRuntimeManager::default());
    let now = Instant::now();
    let player_position = WorldPosition::new(0, 1.0, 0.0, 0.0, 0.0);
    let mut player = test_player_runtime(7, SessionId(7), player_position);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 7);
    player.active_auras.push(ActiveAura {
        spell_id: 1463,
        caster: victim,
        level: 20,
        interrupt_flags: 0,
        positive: true,
        visible: true,
        duration_millis: Some(60_000),
        expires_at: Some(now + Duration::from_secs(60)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::ManaShield {
            school_mask: SPELL_SCHOOL_MASK_NORMAL,
            amount: 10_000,
            mana_multiplier_millis: 0,
        }],
        proc_triggers: Vec::new(),
    });
    maps.add_player(player).await.unwrap();

    let mut spawn = test_creature_spawn(6);
    spawn.guid = 183;
    spawn.entry = 0;
    spawn.template.entry = 0;
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    spawn.orientation = 0.0;
    let attacker = creature_spawn_guid(&spawn);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(spawn)])
        .await;
    maps.begin_db_creature_combat(0, attacker, victim, now)
        .await
        .expect("combat should start");

    let tick = maps
        .apply_db_creature_player_melee_outcome_as_victim_tick(
            0,
            attacker,
            victim,
            MeleeDamageOutcome::normal_hit(7),
            now,
            now + Duration::from_secs(2),
        )
        .await
        .unwrap();

    let attacker_state = tick
        .direct_packets
        .iter()
        .find_map(|packet| {
            (packet.opcode == WorldOpcode::SmsgAttackerStateUpdate as u16).then_some(&packet.body)
        })
        .expect("victim should receive adjusted attacker state");
    let mut cursor = 0;
    assert_eq!(
        read_u32(attacker_state, &mut cursor).unwrap() & HITINFO_ABSORB,
        HITINFO_ABSORB
    );
    assert_eq!(read_packed_guid(attacker_state, &mut cursor).unwrap(), attacker);
    assert_eq!(read_packed_guid(attacker_state, &mut cursor).unwrap(), victim);
    assert_eq!(
        read_u32(attacker_state, &mut cursor).unwrap(),
        0,
        "direct regular-hit packet should report post-absorb damage"
    );
    cursor += 1 + 4 + 4 + 4;
    assert!(
        read_u32(attacker_state, &mut cursor).unwrap() > 0,
        "direct regular-hit packet should carry the absorbed amount"
    );
}

#[test]
fn map_runtime_db_creature_spell_list_schedules_direct_damage_cast() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 180;
    spawn.template.spell_list = 42;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    let creature_guid = creature_spawn_guid(&spawn);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let now = Instant::now();
    map.begin_db_creature_combat(creature_guid, victim, now)
        .expect("combat should start");
    let mut no_roll_spell = test_creature_spell_list_row(42, 1, 999_012, 0, 5_000);
    no_roll_spell.flags = 0;
    let spell_list = vec![no_roll_spell];

    assert!(
        map.ready_db_creature_spell_cast(
            creature_guid,
            victim,
            &spell_list,
            &DbCreatureSpellConditionCache::default(),
            now,
        )
        .is_none(),
        "first AI update should arm the CMaNGOS-style 1200ms spell-list tick"
    );
    let ready = map
        .ready_db_creature_spell_cast(
            creature_guid,
            victim,
            &spell_list,
            &DbCreatureSpellConditionCache::default(),
            now + Duration::from_millis(DB_CREATURE_SPELL_LIST_UPDATE_MILLIS),
        )
        .expect("spell should become ready on the first AI tick");

    assert_eq!(ready.spell.spell_id, 999_012);
    assert_eq!(ready.target, victim);
    assert!(
        !map.creatures
            .get(&creature_guid.raw())
            .expect("creature")
            .spell_cooldowns_until
            .contains_key(&999_012),
        "selection alone should not commit the repeat cooldown"
    );
    let template = test_spell_template(ready.spell.spell_id);
    map.apply_db_creature_spell_cooldowns(creature_guid, &ready.spell, &template, now);
    assert!(map
        .creatures
        .get(&creature_guid.raw())
        .expect("creature")
        .spell_cooldowns_until
        .contains_key(&999_012));
}

#[test]
fn map_runtime_db_creature_spell_list_respects_hard_control() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 181;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    let creature_guid = creature_spawn_guid(&spawn);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let now = Instant::now();
    map.begin_db_creature_combat(creature_guid, victim, now)
        .expect("combat should start");
    let mut no_roll_spell = test_creature_spell_list_row(42, 1, 999_012, 0, 5_000);
    no_roll_spell.flags = 0;
    let spell_list = vec![no_roll_spell];

    assert!(map
        .ready_db_creature_spell_cast(
            creature_guid,
            victim,
            &spell_list,
            &DbCreatureSpellConditionCache::default(),
            now,
        )
        .is_none());
    map.creatures
        .get_mut(&creature_guid.raw())
        .expect("creature")
        .active_auras = vec![test_control_aura(AuraStatModifier::Silence, now)];
    assert!(
        map.ready_db_creature_spell_cast(
            creature_guid,
            victim,
            &spell_list,
            &DbCreatureSpellConditionCache::default(),
            now + Duration::from_millis(DB_CREATURE_SPELL_LIST_UPDATE_MILLIS),
        )
        .is_none(),
        "silenced creatures should not schedule spell-list casts"
    );

    map.creatures
        .get_mut(&creature_guid.raw())
        .expect("creature")
        .active_auras
        .clear();
    let ready = map
        .ready_db_creature_spell_cast(
            creature_guid,
            victim,
            &spell_list,
            &DbCreatureSpellConditionCache::default(),
            now + Duration::from_millis(DB_CREATURE_SPELL_LIST_UPDATE_MILLIS),
        )
        .expect("spell should become ready once hard control clears");
    assert_eq!(ready.spell.spell_id, 999_012);
}

#[test]
fn map_runtime_event_ai_cast_respects_hard_control() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, 5.0, 0.0, 0.0, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    let player = ObjectGuid::new(HighGuid::Player, 0, 1);
    let mut spawn = test_creature_spawn(40);
    spawn.guid = 1907;
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let creature_guid = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let now = Instant::now();
    map.begin_db_creature_combat(creature_guid, player, now)
        .unwrap();
    let scripts = [test_creature_ai_cast_script(
        4007,
        40,
        EVENT_AI_EVENT_TIMER_IN_COMBAT,
        [4_000, 4_000, 38_000, 42_000],
        6016,
        EVENT_AI_TARGET_HOSTILE,
    )];

    assert!(map
        .ready_db_creature_event_ai_spell_cast(creature_guid, player, &scripts, now)
        .is_none());
    map.creatures
        .get_mut(&creature_guid.raw())
        .expect("creature")
        .active_auras = vec![test_control_aura(AuraStatModifier::Confuse, now)];
    assert!(
        map.ready_db_creature_event_ai_spell_cast(
            creature_guid,
            player,
            &scripts,
            now + Duration::from_millis(4_000),
        )
        .is_none(),
        "confused creatures should not schedule EventAI casts"
    );

    map.creatures
        .get_mut(&creature_guid.raw())
        .expect("creature")
        .active_auras
        .clear();
    let ready = map
        .ready_db_creature_event_ai_spell_cast(
            creature_guid,
            player,
            &scripts,
            now + Duration::from_millis(4_000),
        )
        .expect("EventAI cast should become ready once hard control clears");
    assert_eq!(ready.spell_id, 6016);
}

#[test]
fn map_runtime_db_creature_spell_initial_cooldown_blocks_first_ready_tick() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 193;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    let creature_guid = creature_spawn_guid(&spawn);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let now = Instant::now();
    map.begin_db_creature_combat(creature_guid, victim, now)
        .expect("combat should start");
    let mut spell = test_creature_spell_list_row(42, 1, 999_020, 5_000, 0);
    spell.flags = 0;
    let spell_list = vec![spell];

    assert!(map
        .ready_db_creature_spell_cast(
            creature_guid,
            victim,
            &spell_list,
            &DbCreatureSpellConditionCache::default(),
            now,
        )
        .is_none());
    assert!(map
        .ready_db_creature_spell_cast(
            creature_guid,
            victim,
            &spell_list,
            &DbCreatureSpellConditionCache::default(),
            now + Duration::from_millis(DB_CREATURE_SPELL_LIST_UPDATE_MILLIS),
        )
        .is_none());

    let ready = map
        .ready_db_creature_spell_cast(
            creature_guid,
            victim,
            &spell_list,
            &DbCreatureSpellConditionCache::default(),
            now + Duration::from_millis(5_000),
        )
        .expect("spell should become ready after its InitialMin/InitialMax delay");
    assert_eq!(ready.spell.spell_id, 999_020);
}

#[test]
fn map_runtime_db_creature_spell_category_cooldown_blocks_same_category_spell() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 194;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    let creature_guid = creature_spawn_guid(&spawn);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let now = Instant::now();
    map.begin_db_creature_combat(creature_guid, victim, now)
        .expect("combat should start");
    let mut first = test_creature_spell_list_row(42, 1, 999_021, 0, 5_000);
    first.flags = CREATURE_SPELL_LIST_FLAG_CATEGORY_COOLDOWN;
    first.category = 77;
    first.recovery_time = 1_500;
    first.category_recovery_time = 4_000;
    let mut second = test_creature_spell_list_row(42, 2, 999_022, 0, 0);
    second.flags = 0;
    second.category = 77;
    let mut template = test_spell_template(first.spell_id);
    template.category = 77;
    template.recovery_time = 1_500;
    template.category_recovery_time = 4_000;
    let spell_list = vec![first.clone(), second.clone()];

    assert!(map
        .ready_db_creature_spell_cast(
            creature_guid,
            victim,
            &spell_list,
            &DbCreatureSpellConditionCache::default(),
            now,
        )
        .is_none());
    map.apply_db_creature_spell_cooldowns(creature_guid, &first, &template, now);
    let creature = map.creatures.get(&creature_guid.raw()).expect("creature");
    assert!(creature.spell_cooldowns_until.contains_key(&999_021));
    assert!(creature
        .spell_cooldowns_until
        .contains_key(&db_creature_spell_category_cooldown_key(77)));

    assert!(map
        .ready_db_creature_spell_cast(
            creature_guid,
            victim,
            &spell_list,
            &DbCreatureSpellConditionCache::default(),
            now + Duration::from_millis(DB_CREATURE_SPELL_LIST_UPDATE_MILLIS),
        )
        .is_none());

    let ready = map
        .ready_db_creature_spell_cast(
            creature_guid,
            victim,
            &spell_list,
            &DbCreatureSpellConditionCache::default(),
            now + Duration::from_millis(5_000),
        )
        .expect("same-category spells should become eligible after category cooldown");
    assert_eq!(ready.spell.spell_id, 999_021);
}

#[test]
fn map_runtime_db_creature_spell_list_supports_self_and_condition_gates() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 183;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    let creature_guid = creature_spawn_guid(&spawn);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let now = Instant::now();
    map.begin_db_creature_combat(creature_guid, victim, now)
        .expect("combat should start");
    let mut gated = test_creature_spell_list_row(42, 1, 999_013, 0, 0);
    gated.flags = 0;
    gated.combat_condition = 7;
    let mut self_spell = test_creature_spell_list_row(42, 2, 999_014, 0, 0);
    self_spell.flags = 0;
    self_spell.target_id = CREATURE_SPELL_LIST_TARGET_SELF;

    assert!(map
        .ready_db_creature_spell_cast(
            creature_guid,
            victim,
            &[gated.clone()],
            &DbCreatureSpellConditionCache::default(),
            now,
        )
        .is_none());
    let ready = map
        .ready_db_creature_spell_cast(
            creature_guid,
            victim,
            &[gated, self_spell],
            &DbCreatureSpellConditionCache::default(),
            now + Duration::from_millis(DB_CREATURE_SPELL_LIST_UPDATE_MILLIS),
        )
        .expect("supported self-target spell should remain eligible");

    assert_eq!(ready.spell.spell_id, 999_014);
    assert_eq!(ready.target, creature_guid);
}

#[test]
fn map_runtime_db_creature_spell_target_unit_condition_filters_selected_target() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 186;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    let creature_guid = creature_spawn_guid(&spawn);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let now = Instant::now();
    map.begin_db_creature_combat(creature_guid, victim, now)
        .expect("combat should start");
    let mut spell = test_creature_spell_list_row(42, 1, 999_017, 0, 0);
    spell.flags = 0;
    spell.target_unit_condition = 100;
    let mut conditions = DbCreatureSpellConditionCache::default();
    conditions.unit_conditions.insert(
        100,
        test_unit_condition_row(100, UNIT_CONDITION_HEALTH_PERCENT, 3, 50),
    );

    assert!(map
        .ready_db_creature_spell_cast(creature_guid, victim, &[spell.clone()], &conditions, now)
        .is_none());
    assert!(map
        .ready_db_creature_spell_cast(
            creature_guid,
            victim,
            &[spell.clone()],
            &conditions,
            now + Duration::from_millis(DB_CREATURE_SPELL_LIST_UPDATE_MILLIS),
        )
        .is_none());

    let player = map.players.get_mut(&1).expect("player");
    player.health = 10;
    player.max_health = 100;
    let ready = map
        .ready_db_creature_spell_cast(
            creature_guid,
            victim,
            &[spell],
            &conditions,
            now + Duration::from_millis(DB_CREATURE_SPELL_LIST_UPDATE_MILLIS * 2),
        )
        .expect("low-health target should satisfy target UnitCondition");

    assert_eq!(ready.spell.spell_id, 999_017);
    assert_eq!(ready.target, victim);
}

#[test]
fn map_runtime_db_creature_spell_combat_condition_filters_self_condition() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 187;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    spawn.template.min_level_health = 100;
    spawn.template.max_level_health = 100;
    let creature_guid = creature_spawn_guid(&spawn);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let now = Instant::now();
    map.begin_db_creature_combat(creature_guid, victim, now)
        .expect("combat should start");
    let mut spell = test_creature_spell_list_row(42, 1, 999_018, 0, 0);
    spell.flags = 0;
    spell.combat_condition = 200;
    let mut conditions = DbCreatureSpellConditionCache::default();
    conditions.unit_conditions.insert(
        201,
        test_unit_condition_row(201, UNIT_CONDITION_HEALTH_PERCENT, 3, 50),
    );
    conditions
        .combat_conditions
        .insert(200, test_combat_condition_row(200, 201));

    assert!(map
        .ready_db_creature_spell_cast(creature_guid, victim, &[spell.clone()], &conditions, now)
        .is_none());
    assert!(map
        .ready_db_creature_spell_cast(
            creature_guid,
            victim,
            &[spell.clone()],
            &conditions,
            now + Duration::from_millis(DB_CREATURE_SPELL_LIST_UPDATE_MILLIS),
        )
        .is_none());

    map.creatures
        .get_mut(&creature_guid.raw())
        .expect("creature")
        .health = 25;
    let ready = map
        .ready_db_creature_spell_cast(
            creature_guid,
            victim,
            &[spell],
            &conditions,
            now + Duration::from_millis(DB_CREATURE_SPELL_LIST_UPDATE_MILLIS * 2),
        )
        .expect("low-health caster should satisfy CombatCondition self condition");

    assert_eq!(ready.spell.spell_id, 999_018);
}

#[test]
fn map_runtime_db_creature_spell_attack_target_selects_bottom_aggro() {
    let mut map = MapRuntime::new(0, 0);
    let creature_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, creature_position);
    insert_map_runtime_player_for_test(
        &mut map,
        2,
        WorldPosition::new(0, -8953.0, -130.0, 83.5, 0.0),
    );
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 184;
    spawn.position_x = creature_position.x + 1.0;
    spawn.position_y = creature_position.y;
    spawn.position_z = creature_position.z;
    let creature_guid = creature_spawn_guid(&spawn);
    let current_victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    let low_threat = ObjectGuid::new(HighGuid::Player, 0, 2);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let now = Instant::now();
    map.begin_db_creature_combat(creature_guid, current_victim, now)
        .expect("combat should start");
    map.add_db_creature_threat(creature_guid, current_victim, 10.0);
    map.add_db_creature_threat(creature_guid, low_threat, 1.0);
    let mut spell = test_creature_spell_list_row(42, 1, 999_015, 0, 0);
    spell.flags = 0;
    spell.target_type = CREATURE_SPELL_LIST_TARGETING_ATTACK;
    spell.target_param1 = CREATURE_ATTACKING_TARGET_BOTTOM_AGGRO;

    assert!(map
        .ready_db_creature_spell_cast(
            creature_guid,
            current_victim,
            &[spell.clone()],
            &DbCreatureSpellConditionCache::default(),
            now,
        )
        .is_none());
    let ready = map
        .ready_db_creature_spell_cast(
            creature_guid,
            current_victim,
            &[spell],
            &DbCreatureSpellConditionCache::default(),
            now + Duration::from_millis(DB_CREATURE_SPELL_LIST_UPDATE_MILLIS),
        )
        .expect("attack target selector should choose from threat list");

    assert_eq!(ready.target, low_threat);
}

#[test]
fn creature_spell_list_availability_matches_cmangos_lifetime_roll() {
    assert!(db_creature_spell_available_for_lifetime(100, 100));
    assert!(db_creature_spell_available_for_lifetime(50, 50));
    assert!(!db_creature_spell_available_for_lifetime(50, 51));
    assert!(db_creature_spell_available_for_lifetime(0, 0));
    assert!(!db_creature_spell_available_for_lifetime(0, 1));
}

#[test]
fn map_runtime_db_creature_spell_cast_can_heal_creature_target() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    insert_map_runtime_player_for_test(&mut map, 2, observer_position);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 185;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    spawn.template.min_level_health = 80;
    spawn.template.max_level_health = 80;
    let creature_guid = creature_spawn_guid(&spawn);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    let now = Instant::now();
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    map.creatures
        .get_mut(&creature_guid.raw())
        .expect("creature")
        .health = 20;
    map.begin_db_creature_combat(creature_guid, victim, now)
        .expect("combat should start");
    map.start_db_creature_spell_cast(ActiveDbCreatureSpellCast {
        caster: creature_guid,
        target: creature_guid,
        spell_id: 999_016,
        school_mask: spell_school_mask_from_school(0),
        requires_behind: false,
        effect: ActiveDbCreatureSpellEffect::Heal { amount: 15 },
        aura: None,
        range: None,
        mana_cost: 0,
        cast_time_millis: 0,
        due_at: now,
    })
    .unwrap()
    .expect("heal cast should start");

    let completed = map
        .complete_ready_db_creature_spell_cast(creature_guid, victim, now)
        .unwrap()
        .expect("heal cast should complete");
    let DbCreatureCompletedSpellEffect::CreatureHeal(heal) = completed.effect else {
        panic!("heal cast should complete as creature heal");
    };

    assert_eq!(heal.target, creature_guid);
    assert_eq!(heal.amount, 15);
    assert_eq!(heal.target_health, 35);
    assert!(heal
        .observer_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgSpellHealLog as u16));
    assert!(heal
        .observer_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgUpdateObject as u16));
}

#[test]
fn map_runtime_db_creature_spell_cast_start_then_go_damages_player() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    insert_map_runtime_player_for_test(&mut map, 2, observer_position);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 181;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    let creature_guid = creature_spawn_guid(&spawn);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    let now = Instant::now();
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    map.begin_db_creature_combat(creature_guid, victim, now)
        .expect("combat should start");

    let start_packets = map
        .start_db_creature_spell_cast(ActiveDbCreatureSpellCast {
            caster: creature_guid,
            target: victim,
            spell_id: 999_012,
            school_mask: spell_school_mask_from_school(1),
            requires_behind: false,
            effect: ActiveDbCreatureSpellEffect::Damage {
                amount: 6,
                school: 1,
                dmg_class: SPELL_DAMAGE_CLASS_MAGIC,
                attributes_ex2: SPELL_ATTR_EX2_CANT_CRIT,
                attributes_ex3: TEST_SPELL_ATTR_EX3_ALWAYS_HIT,
            },
            aura: None,
            range: None,
            mana_cost: 0,
            cast_time_millis: 1_500,
            due_at: now + Duration::from_millis(1_500),
        })
        .unwrap()
        .expect("cast should start");
    assert_eq!(start_packets.len(), 2);
    assert!(start_packets
        .iter()
        .all(|(_, packet)| packet.opcode == WorldOpcode::SmsgSpellStart as u16));
    assert!(map
        .complete_ready_db_creature_spell_cast(creature_guid, victim, now)
        .unwrap()
        .is_none());

    let completed = map
        .complete_ready_db_creature_spell_cast(
            creature_guid,
            victim,
            now + Duration::from_millis(1_500),
        )
        .unwrap()
        .expect("cast should complete once due");

    let DbCreatureCompletedSpellEffect::PlayerDamage(damage) = completed.effect else {
        panic!("direct damage cast should complete as player damage");
    };
    assert_eq!(damage.damage, 6);
    assert_eq!(damage.victim_health, 14);
    assert!(damage.spell_non_melee_log_body.as_ref().is_some());
    assert!(map
        .active_db_creature_spell_cast_due_at(creature_guid)
        .is_none());
}

#[test]
fn map_runtime_db_creature_spell_start_and_completion_respect_hard_control() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 188;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    let creature_guid = creature_spawn_guid(&spawn);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    let now = Instant::now();
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    map.begin_db_creature_combat(creature_guid, victim, now)
        .expect("combat should start");
    let cast = ActiveDbCreatureSpellCast {
        caster: creature_guid,
        target: victim,
        spell_id: 999_016,
        school_mask: spell_school_mask_from_school(4),
        requires_behind: false,
        effect: ActiveDbCreatureSpellEffect::Damage {
            amount: 5,
            school: 4,
            dmg_class: 2,
            attributes_ex2: 0,
            attributes_ex3: 0,
        },
        aura: None,
        range: None,
        mana_cost: 0,
        cast_time_millis: 1_000,
        due_at: now + Duration::from_millis(1_000),
    };

    map.creatures
        .get_mut(&creature_guid.raw())
        .expect("creature")
        .active_auras = vec![test_control_aura(AuraStatModifier::Silence, now)];
    assert!(map
        .start_db_creature_spell_cast(cast.clone())
        .unwrap()
        .is_none());
    assert!(!map
        .active_creature_spell_casts
        .contains_key(&creature_guid.raw()));

    map.creatures
        .get_mut(&creature_guid.raw())
        .expect("creature")
        .active_auras
        .clear();
    assert!(map
        .start_db_creature_spell_cast(cast)
        .unwrap()
        .expect("cast should start after silence clears")
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgSpellStart as u16));
    map.creatures
        .get_mut(&creature_guid.raw())
        .expect("creature")
        .active_auras = vec![test_control_aura(AuraStatModifier::Stun, now)];

    let completed = map
        .complete_ready_db_creature_spell_cast(creature_guid, victim, now + Duration::from_secs(1))
        .unwrap()
        .expect("controlled in-flight cast should finish as interrupted");
    let DbCreatureCompletedSpellEffect::Interrupted(interrupted) = completed.effect else {
        panic!("controlled in-flight cast should be interrupted");
    };
    assert_eq!(interrupted.failure, SPELL_FAILED_STUNNED);
    assert!(!map
        .active_creature_spell_casts
        .contains_key(&creature_guid.raw()));
}

#[test]
fn map_runtime_db_creature_spell_target_validation_checks_selected_target_range_and_los() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    let mut spawn = test_creature_spawn(3196);
    spawn.guid = 192;
    spawn.position_x = 40.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let creature_guid = creature_spawn_guid(&spawn);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let range = SpellRangeEntry {
        min_range: 0.0,
        max_range: 30.0,
        flags: 0,
    };

    assert_eq!(
        map.validate_db_creature_spell_against_target(
            creature_guid,
            victim,
            &DbCreatureNavigationGuardrail::default(),
            Some(range),
            false,
        )
        .check,
        DbCreatureSpellTargetCheck::OutOfRange
    );

    map.creatures
        .get_mut(&creature_guid.raw())
        .unwrap()
        .current_position
        .x = 10.0;
    let blocked_los = DbCreatureNavigationGuardrail {
        line_of_sight_clear: false,
        ..DbCreatureNavigationGuardrail::default()
    };
    assert_eq!(
        map.validate_db_creature_spell_against_target(
            creature_guid,
            victim,
            &blocked_los,
            Some(range),
            false,
        )
        .check,
        DbCreatureSpellTargetCheck::NavigationBlocked(
            DbCreatureNavigationResult::LineOfSightBlocked
        )
    );

    assert_eq!(
        map.validate_db_creature_spell_against_target(
            creature_guid,
            victim,
            &DbCreatureNavigationGuardrail::default(),
            Some(range),
            false,
        )
        .check,
        DbCreatureSpellTargetCheck::Clear
    );
}

#[test]
fn map_runtime_db_creature_backstab_validation_requires_facing_targets_back() {
    let mut map = MapRuntime::new(0, 0);
    insert_map_runtime_player_for_test(&mut map, 1, WorldPosition::new(0, 5.0, 0.0, 0.0, 0.0));
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    let mut spawn = test_creature_spawn(94);
    spawn.guid = 194;
    spawn.position_x = 7.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    spawn.orientation = std::f32::consts::PI;
    let creature_guid = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let melee_range = SpellRangeEntry {
        min_range: 0.0,
        max_range: 5.0,
        flags: 0,
    };

    assert_eq!(
        map.validate_db_creature_spell_against_target(
            creature_guid,
            victim,
            &DbCreatureNavigationGuardrail::default(),
            Some(melee_range),
            true,
        )
        .check,
        DbCreatureSpellTargetCheck::NotBehind
    );

    map.creatures
        .get_mut(&creature_guid.raw())
        .unwrap()
        .current_position = WorldPosition::new(0, 3.0, 0.0, 0.0, 0.0);
    assert_eq!(
        map.validate_db_creature_spell_against_target(
            creature_guid,
            victim,
            &DbCreatureNavigationGuardrail::default(),
            Some(melee_range),
            true,
        )
        .check,
        DbCreatureSpellTargetCheck::Clear
    );
}

#[test]
fn map_runtime_player_spell_target_validation_treats_ranged_min_range_as_melee_complement() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 193;
    spawn.position_x = 9.9;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    spawn.template.faction = 17;
    let creature_guid = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let hunter_range = SpellRangeEntry {
        min_range: 5.0,
        max_range: 35.0,
        flags: SPELL_RANGE_FLAG_RANGED,
    };

    assert_eq!(
        map.validate_player_spell_against_db_creature(
            &FactionTemplateStore::fallback_bridge(),
            1,
            creature_guid,
            &DbCreatureNavigationGuardrail::default(),
            Some(hunter_range),
            false,
        )
        .check,
        PlayerSpellTargetCheck::TooClose,
        "CMaNGOS range flag 0x2 makes the minimum range start beyond melee reach"
    );

    map.creatures
        .get_mut(&creature_guid.raw())
        .unwrap()
        .current_position
        .x = 10.0;
    assert_eq!(
        map.validate_player_spell_against_db_creature(
            &FactionTemplateStore::fallback_bridge(),
            1,
            creature_guid,
            &DbCreatureNavigationGuardrail::default(),
            Some(hunter_range),
            false,
        )
        .check,
        PlayerSpellTargetCheck::Clear
    );
}

#[test]
fn map_runtime_db_creature_spell_completion_rechecks_range_and_los() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    let mut spawn = test_creature_spawn(3196);
    spawn.guid = 195;
    spawn.position_x = 10.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let creature_guid = creature_spawn_guid(&spawn);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    let now = Instant::now();
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    map.begin_db_creature_combat(creature_guid, victim, now)
        .expect("combat should start");
    let range = SpellRangeEntry {
        min_range: 0.0,
        max_range: 30.0,
        flags: 0,
    };

    map.start_db_creature_spell_cast(ActiveDbCreatureSpellCast {
        caster: creature_guid,
        target: victim,
        spell_id: 999_012,
        school_mask: spell_school_mask_from_school(1),
        requires_behind: false,
        effect: ActiveDbCreatureSpellEffect::Damage {
            amount: 6,
            school: 1,
            dmg_class: SPELL_DAMAGE_CLASS_MAGIC,
            attributes_ex2: SPELL_ATTR_EX2_CANT_CRIT,
            attributes_ex3: TEST_SPELL_ATTR_EX3_ALWAYS_HIT,
        },
        aura: None,
        range: Some(range),
        mana_cost: 0,
        cast_time_millis: 1_500,
        due_at: now + Duration::from_millis(1_500),
    })
    .unwrap()
    .expect("cast should start");
    map.players.get_mut(&1).expect("player").position.x = 80.0;

    let interrupted = map
        .complete_ready_db_creature_spell_cast(
            creature_guid,
            victim,
            now + Duration::from_millis(1_500),
        )
        .unwrap()
        .expect("out-of-range completion should send an interrupted cast cleanup");
    let DbCreatureCompletedSpellEffect::Interrupted(interrupted) = interrupted.effect else {
        panic!("out-of-range completion should be interrupted");
    };
    assert_eq!(interrupted.failure, SPELL_FAILED_OUT_OF_RANGE);
    let opcodes = interrupted
        .observer_packets
        .iter()
        .map(|(_, packet)| packet.opcode)
        .collect::<Vec<_>>();
    assert!(opcodes.contains(&(WorldOpcode::SmsgSpellFailure as u16)));
    assert!(opcodes.contains(&(WorldOpcode::SmsgSpellFailedOther as u16)));
    assert!(!opcodes.contains(&(WorldOpcode::SmsgSpellGo as u16)));
    assert_eq!(map.players.get(&1).expect("player").health, 20);
    assert!(map
        .active_db_creature_spell_cast_due_at(creature_guid)
        .is_none());

    map.players.get_mut(&1).expect("player").position = player_position;
    map.start_db_creature_spell_cast(ActiveDbCreatureSpellCast {
        caster: creature_guid,
        target: victim,
        spell_id: 999_012,
        school_mask: spell_school_mask_from_school(1),
        requires_behind: false,
        effect: ActiveDbCreatureSpellEffect::Damage {
            amount: 6,
            school: 1,
            dmg_class: SPELL_DAMAGE_CLASS_MAGIC,
            attributes_ex2: SPELL_ATTR_EX2_CANT_CRIT,
            attributes_ex3: TEST_SPELL_ATTR_EX3_ALWAYS_HIT,
        },
        aura: None,
        range: Some(range),
        mana_cost: 0,
        cast_time_millis: 1_500,
        due_at: now + Duration::from_millis(3_000),
    })
    .unwrap()
    .expect("second cast should start");
    let blocked_los = DbCreatureNavigationGuardrail {
        line_of_sight_clear: false,
        ..DbCreatureNavigationGuardrail::default()
    };

    let interrupted = map
        .complete_ready_db_creature_spell_cast_with_navigation(
            creature_guid,
            victim,
            now + Duration::from_millis(3_000),
            &blocked_los,
        )
        .unwrap()
        .expect("LOS completion should send an interrupted cast cleanup");
    let DbCreatureCompletedSpellEffect::Interrupted(interrupted) = interrupted.effect else {
        panic!("LOS completion should be interrupted");
    };
    assert_eq!(interrupted.failure, SPELL_FAILED_LINE_OF_SIGHT);
    let opcodes = interrupted
        .observer_packets
        .iter()
        .map(|(_, packet)| packet.opcode)
        .collect::<Vec<_>>();
    assert!(opcodes.contains(&(WorldOpcode::SmsgSpellFailure as u16)));
    assert!(opcodes.contains(&(WorldOpcode::SmsgSpellFailedOther as u16)));
    assert!(!opcodes.contains(&(WorldOpcode::SmsgSpellGo as u16)));
    assert_eq!(map.players.get(&1).expect("player").health, 20);
    assert!(map
        .active_db_creature_spell_cast_due_at(creature_guid)
        .is_none());
}

#[test]
fn map_runtime_db_creature_spell_start_stops_chase_spends_mana_and_exposes_cast_time() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    insert_map_runtime_player_for_test(&mut map, 2, observer_position);
    let mut spawn = test_creature_spawn(3196);
    spawn.guid = 183;
    spawn.position_x = player_position.x + 8.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    spawn.template.unit_class = 2;
    spawn.template.min_level_mana = 178;
    spawn.template.max_level_mana = 191;
    let creature_guid = creature_spawn_guid(&spawn);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    let now = Instant::now();
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    map.begin_db_creature_combat(creature_guid, victim, now)
        .expect("combat should start");
    map.start_db_creature_chase_motion(
        &DbCreatureNavigationGuardrail::default(),
        creature_guid,
        victim,
        player_position,
        now,
    )
    .expect("creature should be chasing before starting its cast");

    let start_packets = map
        .start_db_creature_spell_cast(ActiveDbCreatureSpellCast {
            caster: creature_guid,
            target: victim,
            spell_id: 348,
            school_mask: spell_school_mask_from_school(2),
            requires_behind: false,
            effect: ActiveDbCreatureSpellEffect::Damage {
                amount: 8,
                school: 2,
                dmg_class: SPELL_DAMAGE_CLASS_MAGIC,
                attributes_ex2: TEST_SPELL_ATTR_EX2_CANT_CRIT,
                attributes_ex3: TEST_SPELL_ATTR_EX3_ALWAYS_HIT,
            },
            aura: None,
            range: None,
            mana_cost: 25,
            cast_time_millis: 2_000,
            due_at: now + Duration::from_millis(2_000),
        })
        .unwrap()
        .expect("mana caster should start cast");

    let opcodes = start_packets
        .iter()
        .map(|(_, packet)| packet.opcode)
        .collect::<Vec<_>>();
    assert_eq!(
        opcodes,
        vec![
            WorldOpcode::SmsgMonsterMove as u16,
            WorldOpcode::SmsgUpdateObject as u16,
            WorldOpcode::SmsgSpellStart as u16,
            WorldOpcode::SmsgMonsterMove as u16,
            WorldOpcode::SmsgUpdateObject as u16,
            WorldOpcode::SmsgSpellStart as u16,
        ]
    );
    let creature = map.creatures.get(&creature_guid.raw()).unwrap();
    assert_eq!(creature.power1, 166);
    assert!(matches!(creature.motion, CreatureMotionState::Idle));
    let start = start_packets
        .iter()
        .find(|(_, packet)| packet.opcode == WorldOpcode::SmsgSpellStart as u16)
        .unwrap();
    let mut cursor = PackedGuid::packed_size(creature_guid) * 2 + 4 + 2;
    assert_eq!(read_u32(&start.1.body, &mut cursor).unwrap(), 2_000);
    let power_update = start_packets
        .iter()
        .find(|(_, packet)| packet.opcode == WorldOpcode::SmsgUpdateObject as u16)
        .unwrap();
    let (values, trailing) = decode_values_update_block(&power_update.1.body[5..], creature_guid);
    assert!(trailing.is_empty());
    assert_eq!(values[UNIT_FIELD_POWER1], Some(166));
}

#[test]
fn map_runtime_db_creature_immolate_applies_player_dot_ticks() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    insert_map_runtime_player_for_test(&mut map, 2, observer_position);
    let mut spawn = test_creature_spawn(3196);
    spawn.guid = 186;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    spawn.template.unit_class = 2;
    spawn.template.min_level_mana = 178;
    spawn.template.max_level_mana = 191;
    let creature_guid = creature_spawn_guid(&spawn);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    let now = Instant::now();
    let immolate = immolate_spell_template();
    let aura = build_active_aura(
        &immolate,
        creature_guid,
        6,
        test_spell_effect_value_context(&immolate),
        now,
        Some(SpellDurationEntry {
            duration_millis: 9_000,
            duration_per_level_millis: 0,
            max_duration_millis: 9_000,
        }),
    );
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    map.begin_db_creature_combat(creature_guid, victim, now)
        .expect("combat should start");
    map.start_db_creature_spell_cast(ActiveDbCreatureSpellCast {
        caster: creature_guid,
        target: victim,
        spell_id: immolate.id,
        school_mask: spell_school_mask_from_school(immolate.school),
        requires_behind: false,
        effect: ActiveDbCreatureSpellEffect::Damage {
            amount: 8,
            school: immolate.school as u8,
            dmg_class: immolate.dmg_class,
            attributes_ex2: immolate.attributes_ex2,
            attributes_ex3: immolate.attributes_ex3,
        },
        aura: Some(aura),
        range: None,
        mana_cost: immolate.mana_cost,
        cast_time_millis: 0,
        due_at: now,
    })
    .unwrap()
    .expect("cast should start");

    let completed = map
        .complete_ready_db_creature_spell_cast(creature_guid, victim, now)
        .unwrap()
        .expect("instant cast should complete");
    assert!(completed.aura_event.is_some());
    let DbCreatureCompletedSpellEffect::PlayerDamage(damage) = completed.effect else {
        panic!("Immolate should include direct player damage");
    };
    assert_eq!(damage.damage, 8);
    assert_eq!(damage.victim_health, 12);
    let player = map.players.get(&1).unwrap();
    assert_eq!(player.active_auras.len(), 1);
    assert_eq!(player.active_auras[0].spell_id, 348);
    assert_eq!(player.active_auras[0].periodic_damage.unwrap().amount, 4);

    let tick_at = player.active_auras[0].periodic_damage.unwrap().next_tick_at;
    let tick_packets = map.advance_player_aura_expirations(tick_at).unwrap();
    let player = map.players.get(&1).unwrap();
    assert_eq!(player.health, 8);
    assert!(tick_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgPeriodicAuraLog as u16));
    assert!(tick_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgUpdateObject as u16));
}

#[test]
fn map_runtime_db_creature_delayed_immolate_applies_player_dot_ticks() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    let mut spawn = test_creature_spawn(3196);
    spawn.guid = 18_600;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    spawn.template.unit_class = 2;
    spawn.template.min_level_mana = 178;
    spawn.template.max_level_mana = 191;
    let creature_guid = creature_spawn_guid(&spawn);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    let now = Instant::now();
    let immolate = immolate_spell_template();
    let aura = build_active_aura(
        &immolate,
        creature_guid,
        6,
        test_spell_effect_value_context(&immolate),
        now,
        Some(SpellDurationEntry {
            duration_millis: 9_000,
            duration_per_level_millis: 0,
            max_duration_millis: 9_000,
        }),
    );
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    map.begin_db_creature_combat(creature_guid, victim, now)
        .expect("combat should start");
    map.start_db_creature_spell_cast(ActiveDbCreatureSpellCast {
        caster: creature_guid,
        target: victim,
        spell_id: immolate.id,
        school_mask: spell_school_mask_from_school(immolate.school),
        requires_behind: false,
        effect: ActiveDbCreatureSpellEffect::Damage {
            amount: 8,
            school: immolate.school as u8,
            dmg_class: immolate.dmg_class,
            attributes_ex2: immolate.attributes_ex2,
            attributes_ex3: immolate.attributes_ex3,
        },
        aura: Some(aura),
        range: None,
        mana_cost: immolate.mana_cost,
        cast_time_millis: 2_000,
        due_at: now + Duration::from_millis(2_000),
    })
    .unwrap()
    .expect("cast should start");

    assert!(map
        .complete_ready_db_creature_spell_cast(
            creature_guid,
            victim,
            now + Duration::from_millis(1_999)
        )
        .unwrap()
        .is_none());

    let completed = map
        .complete_ready_db_creature_spell_cast(
            creature_guid,
            victim,
            now + Duration::from_millis(2_000),
        )
        .unwrap()
        .expect("delayed cast should complete once due");
    assert!(completed.aura_event.is_some());
    let DbCreatureCompletedSpellEffect::PlayerDamage(damage) = completed.effect else {
        panic!("Immolate should include direct player damage");
    };
    assert_eq!(damage.damage, 8);
    assert_eq!(damage.victim_health, 12);
    let player = map.players.get(&1).unwrap();
    assert_eq!(player.active_auras.len(), 1);
    assert_eq!(player.active_auras[0].spell_id, 348);
    let tick_at = player.active_auras[0].periodic_damage.unwrap().next_tick_at;
    let tick_packets = map.advance_player_aura_expirations(tick_at).unwrap();
    let player = map.players.get(&1).unwrap();
    assert_eq!(player.health, 8);
    assert!(tick_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgPeriodicAuraLog as u16));
}

#[test]
fn map_runtime_db_creature_dot_keeps_ticking_after_caster_runtime_is_missing() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    {
        let player = map.players.get_mut(&1).unwrap();
        player.health = 40;
        player.max_health = 40;
    }
    let mut spawn = test_creature_spawn(3196);
    spawn.guid = 191;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    spawn.template.unit_class = 2;
    spawn.template.min_level_mana = 178;
    spawn.template.max_level_mana = 191;
    let creature_guid = creature_spawn_guid(&spawn);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    let now = Instant::now();
    let immolate = immolate_spell_template();
    let aura = build_active_aura(
        &immolate,
        creature_guid,
        6,
        test_spell_effect_value_context(&immolate),
        now,
        Some(SpellDurationEntry {
            duration_millis: 9_000,
            duration_per_level_millis: 0,
            max_duration_millis: 9_000,
        }),
    );
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    map.begin_db_creature_combat(creature_guid, victim, now)
        .expect("combat should start");
    map.start_db_creature_spell_cast(ActiveDbCreatureSpellCast {
        caster: creature_guid,
        target: victim,
        spell_id: immolate.id,
        school_mask: spell_school_mask_from_school(immolate.school),
        requires_behind: false,
        effect: ActiveDbCreatureSpellEffect::Damage {
            amount: 8,
            school: immolate.school as u8,
            dmg_class: immolate.dmg_class,
            attributes_ex2: immolate.attributes_ex2,
            attributes_ex3: immolate.attributes_ex3,
        },
        aura: Some(aura),
        range: None,
        mana_cost: immolate.mana_cost,
        cast_time_millis: 0,
        due_at: now,
    })
    .unwrap()
    .expect("cast should start");
    map.complete_ready_db_creature_spell_cast(creature_guid, victim, now)
        .unwrap()
        .expect("instant cast should complete");
    assert_eq!(map.players.get(&1).unwrap().health, 32);

    map.creatures.remove(&creature_guid.raw());

    for (tick_index, expected_health) in [(1, 28), (2, 24), (3, 20)] {
        let tick_at = now + Duration::from_millis(3_000 * tick_index);
        let tick_packets = map.advance_player_aura_expirations(tick_at).unwrap();
        let player = map.players.get(&1).unwrap();
        assert_eq!(player.health, expected_health);
        assert!(
            tick_packets
                .iter()
                .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgPeriodicAuraLog as u16),
            "missing periodic aura log for tick {tick_index}"
        );
    }
    assert!(
        map.players.get(&1).unwrap().active_auras.is_empty(),
        "third Immolate tick at aura duration should also expire the debuff"
    );
}

#[test]
fn map_runtime_db_creature_immolate_full_resist_still_sends_go_without_dot() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    insert_map_runtime_player_for_test(&mut map, 2, observer_position);
    map.players.get_mut(&1).unwrap().level = 30;
    let mut spawn = test_creature_spawn(3196);
    spawn.guid = 190;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    spawn.template.unit_class = 2;
    spawn.template.min_level = 9;
    spawn.template.max_level = 10;
    spawn.template.min_level_mana = 178;
    spawn.template.max_level_mana = 191;
    let creature_guid = creature_spawn_guid(&spawn);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    let now = Instant::now();
    let immolate = immolate_spell_template();
    let aura = build_active_aura(
        &immolate,
        creature_guid,
        10,
        test_spell_effect_value_context(&immolate),
        now,
        Some(SpellDurationEntry {
            duration_millis: 9_000,
            duration_per_level_millis: 0,
            max_duration_millis: 9_000,
        }),
    );
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    map.begin_db_creature_combat(creature_guid, victim, now)
        .expect("combat should start");
    map.start_db_creature_spell_cast(ActiveDbCreatureSpellCast {
        caster: creature_guid,
        target: victim,
        spell_id: immolate.id,
        school_mask: spell_school_mask_from_school(immolate.school),
        requires_behind: false,
        effect: ActiveDbCreatureSpellEffect::Damage {
            amount: 8,
            school: immolate.school as u8,
            dmg_class: immolate.dmg_class,
            attributes_ex2: immolate.attributes_ex2,
            attributes_ex3: 0,
        },
        aura: Some(aura),
        range: None,
        mana_cost: immolate.mana_cost,
        cast_time_millis: 0,
        due_at: now,
    })
    .unwrap()
    .expect("cast should start");

    let completed = map
        .complete_ready_db_creature_spell_cast(creature_guid, victim, now)
        .unwrap()
        .expect("fully resisted casts should still complete and emit SPELL_GO");
    assert!(!completed.spell_go_body.is_empty());
    let mut cursor = PackedGuid::packed_size(creature_guid) * 2 + 4;
    assert_eq!(
        u16::from_le_bytes(
            completed.spell_go_body[cursor..cursor + 2]
                .try_into()
                .unwrap()
        ),
        CAST_FLAG_SPELL_GO
    );
    cursor += 2;
    assert_eq!(
        completed.spell_go_body[cursor], 0,
        "fully resisted creature spells must not list the target as a hit in SMSG_SPELL_GO"
    );
    cursor += 1;
    assert_eq!(completed.spell_go_body[cursor], 1);
    cursor += 1;
    assert_eq!(
        u64::from_le_bytes(
            completed.spell_go_body[cursor..cursor + 8]
                .try_into()
                .unwrap()
        ),
        victim.raw()
    );
    cursor += 8;
    assert_eq!(completed.spell_go_body[cursor], SPELL_MISS_RESIST);
    cursor += 1;
    assert_eq!(
        u16::from_le_bytes(
            completed.spell_go_body[cursor..cursor + 2]
                .try_into()
                .unwrap()
        ),
        SPELL_CAST_TARGET_UNIT
    );
    cursor += 2;
    assert_eq!(
        read_packed_guid(&completed.spell_go_body, &mut cursor).unwrap(),
        victim
    );
    assert_eq!(cursor, completed.spell_go_body.len());
    assert!(completed.aura_event.is_none());
    let DbCreatureCompletedSpellEffect::PlayerDamage(damage) = completed.effect else {
        panic!("Immolate should complete as player spell damage");
    };
    assert_eq!(damage.damage, 0);
    assert_eq!(damage.victim_health, 20);
    assert!(damage.spell_miss_log_body.is_some());
    assert!(damage.spell_non_melee_log_body.is_none());
    assert!(map.players.get(&1).unwrap().active_auras.is_empty());
    assert!(map
        .active_db_creature_spell_cast_due_at(creature_guid)
        .is_none());
}

#[test]
fn map_runtime_creature_dot_death_presents_release_and_clears_combat() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    let mut spawn = test_creature_spawn(3196);
    spawn.guid = 189;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    spawn.template.unit_class = 2;
    spawn.template.min_level_mana = 178;
    spawn.template.max_level_mana = 191;
    let creature_guid = creature_spawn_guid(&spawn);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    let now = Instant::now();
    let immolate = immolate_spell_template();
    let aura = build_active_aura(
        &immolate,
        creature_guid,
        6,
        test_spell_effect_value_context(&immolate),
        now,
        Some(SpellDurationEntry {
            duration_millis: 9_000,
            duration_per_level_millis: 0,
            max_duration_millis: 9_000,
        }),
    );
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    map.begin_db_creature_combat(creature_guid, victim, now)
        .expect("combat should start");
    map.start_db_creature_spell_cast(ActiveDbCreatureSpellCast {
        caster: creature_guid,
        target: victim,
        spell_id: immolate.id,
        school_mask: spell_school_mask_from_school(immolate.school),
        requires_behind: false,
        effect: ActiveDbCreatureSpellEffect::Damage {
            amount: 8,
            school: immolate.school as u8,
            dmg_class: immolate.dmg_class,
            attributes_ex2: immolate.attributes_ex2,
            attributes_ex3: immolate.attributes_ex3,
        },
        aura: Some(aura),
        range: None,
        mana_cost: immolate.mana_cost,
        cast_time_millis: 0,
        due_at: now,
    })
    .unwrap()
    .expect("cast should start");
    map.complete_ready_db_creature_spell_cast(creature_guid, victim, now)
        .unwrap()
        .expect("instant cast should complete");
    let tick_at = map.players.get(&1).unwrap().active_auras[0]
        .periodic_damage
        .unwrap()
        .next_tick_at;
    {
        let player = map.players.get_mut(&1).unwrap();
        player.health = 4;
        player.active_combat_target = Some(creature_guid);
        player.active_combat_next_swing_at = Some(tick_at);
        player.queued_next_melee_spell = Some(QueuedNextMeleeSpell {
            spell_id: 78,
            target: creature_guid,
            bonus_damage: 1,
            rage_cost: 15,
            mana_cost: 0,
        });
    }

    let packets = map.advance_player_aura_expirations(tick_at).unwrap();
    let player = map.players.get(&1).unwrap();
    assert_eq!(player.health, 0);
    assert_eq!(player.death_state, PlayerDeathState::Corpse);
    assert_eq!(player.stand_state, PLAYER_STAND_STATE_DEAD);
    assert!(player.active_auras.is_empty());
    assert!(player.active_combat_target.is_none());
    assert!(player.active_combat_next_swing_at.is_none());
    assert!(player.queued_next_melee_spell.is_none());
    assert!(map.active_db_creature_combats_for_victim(victim).is_empty());
    assert!(packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgForceMoveRoot as u16));
    assert!(packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgAttackStop as u16));
    assert!(packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgUpdateObject as u16));
    let mut packed_player_guid = Vec::new();
    PackedGuid::write(&mut packed_player_guid, victim).unwrap();
    let direct_player_health_updates = packets
        .iter()
        .filter(|(session, packet)| {
            *session == SessionId(1)
                && packet.opcode == WorldOpcode::SmsgUpdateObject as u16
                && packet.body.len() > 6 + packed_player_guid.len()
                && packet.body[5] == UPDATE_TYPE_VALUES
                && packet.body[6..6 + packed_player_guid.len()] == packed_player_guid
        })
        .filter_map(|(_, packet)| {
            let (values, _) = decode_values_update_block(&packet.body[5..], victim);
            values.get(UNIT_FIELD_HEALTH).copied().flatten()
        })
        .collect::<Vec<_>>();
    assert!(direct_player_health_updates.contains(&0));
    assert!(
        !direct_player_health_updates.contains(&1),
        "death aura cleanup must not send a later 1 HP stat refresh: {direct_player_health_updates:?}"
    );
}

#[test]
fn map_runtime_db_creature_lethal_immolate_does_not_apply_dot() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    map.players.get_mut(&1).unwrap().health = 4;
    let mut spawn = test_creature_spawn(3196);
    spawn.guid = 188;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    spawn.template.unit_class = 2;
    spawn.template.min_level_mana = 178;
    spawn.template.max_level_mana = 191;
    let creature_guid = creature_spawn_guid(&spawn);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    let now = Instant::now();
    let immolate = immolate_spell_template();
    let aura = build_active_aura(
        &immolate,
        creature_guid,
        6,
        test_spell_effect_value_context(&immolate),
        now,
        Some(SpellDurationEntry {
            duration_millis: 9_000,
            duration_per_level_millis: 0,
            max_duration_millis: 9_000,
        }),
    );
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    map.begin_db_creature_combat(creature_guid, victim, now)
        .expect("combat should start");
    map.start_db_creature_spell_cast(ActiveDbCreatureSpellCast {
        caster: creature_guid,
        target: victim,
        spell_id: immolate.id,
        school_mask: spell_school_mask_from_school(immolate.school),
        requires_behind: false,
        effect: ActiveDbCreatureSpellEffect::Damage {
            amount: 8,
            school: immolate.school as u8,
            dmg_class: immolate.dmg_class,
            attributes_ex2: immolate.attributes_ex2,
            attributes_ex3: immolate.attributes_ex3,
        },
        aura: Some(aura),
        range: None,
        mana_cost: immolate.mana_cost,
        cast_time_millis: 0,
        due_at: now,
    })
    .unwrap()
    .expect("cast should start");

    let completed = map
        .complete_ready_db_creature_spell_cast(creature_guid, victim, now)
        .unwrap()
        .expect("instant cast should complete");
    let DbCreatureCompletedSpellEffect::PlayerDamage(damage) = completed.effect else {
        panic!("Immolate should include direct player damage");
    };

    assert_eq!(damage.victim_health, 0);
    assert!(completed.aura_event.is_none());
    assert!(damage.aura_packet.is_none());
    let player = map.players.get(&1).unwrap();
    assert_eq!(player.active_auras.len(), 0);
    assert_eq!(player.death_state, PlayerDeathState::Corpse);
    assert_eq!(player.stand_state, PLAYER_STAND_STATE_DEAD);
}

#[test]
fn map_runtime_db_creature_dot_survives_session_sync_and_sends_expire_update() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    let mut spawn = test_creature_spawn(3196);
    spawn.guid = 187;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    spawn.template.unit_class = 2;
    spawn.template.min_level_mana = 178;
    spawn.template.max_level_mana = 191;
    let creature_guid = creature_spawn_guid(&spawn);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    let now = Instant::now();
    let immolate = immolate_spell_template();
    let aura = build_active_aura(
        &immolate,
        creature_guid,
        6,
        test_spell_effect_value_context(&immolate),
        now,
        Some(SpellDurationEntry {
            duration_millis: 9_000,
            duration_per_level_millis: 0,
            max_duration_millis: 9_000,
        }),
    );
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    map.begin_db_creature_combat(creature_guid, victim, now)
        .expect("combat should start");
    map.start_db_creature_spell_cast(ActiveDbCreatureSpellCast {
        caster: creature_guid,
        target: victim,
        spell_id: immolate.id,
        school_mask: spell_school_mask_from_school(immolate.school),
        requires_behind: false,
        effect: ActiveDbCreatureSpellEffect::Damage {
            amount: 8,
            school: immolate.school as u8,
            dmg_class: immolate.dmg_class,
            attributes_ex2: immolate.attributes_ex2,
            attributes_ex3: immolate.attributes_ex3,
        },
        aura: Some(aura),
        range: None,
        mana_cost: immolate.mana_cost,
        cast_time_millis: 0,
        due_at: now,
    })
    .unwrap()
    .expect("cast should start");
    map.complete_ready_db_creature_spell_cast(creature_guid, victim, now)
        .unwrap()
        .expect("instant cast should complete");

    let session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 1,
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
            player_health: 12,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    map.sync_player_gameplay_state(1, &session);
    assert_eq!(map.players.get(&1).unwrap().active_auras.len(), 1);

    let packets = map
        .advance_player_aura_expirations(now + Duration::from_millis(9_000))
        .unwrap();
    let player = map.players.get(&1).unwrap();
    assert!(player.active_auras.is_empty());
    let debuff_slot = MAX_POSITIVE_AURA_SLOTS;
    let values = packets
        .iter()
        .filter(|(_, packet)| packet.opcode == WorldOpcode::SmsgUpdateObject as u16)
        .find_map(|(_, packet)| {
            let (values, trailing) = decode_values_update_block(&packet.body[5..], victim);
            if trailing.is_empty() && values[UNIT_FIELD_AURA + debuff_slot].is_some() {
                Some(values)
            } else {
                None
            }
        })
        .expect("aura expiration should send an aura-field update object packet");
    assert_eq!(values[UNIT_FIELD_AURA + debuff_slot], Some(0));
    assert_eq!(values[UNIT_FIELD_AURAFLAGS + (debuff_slot / 8)], Some(0));
}

#[test]
fn map_runtime_db_creature_spell_cast_drops_if_victim_dies_before_go() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 182;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    let creature_guid = creature_spawn_guid(&spawn);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);
    let now = Instant::now();
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    map.begin_db_creature_combat(creature_guid, victim, now)
        .expect("combat should start");
    map.start_db_creature_spell_cast(ActiveDbCreatureSpellCast {
        caster: creature_guid,
        target: victim,
        spell_id: 999_012,
        school_mask: spell_school_mask_from_school(1),
        requires_behind: false,
        effect: ActiveDbCreatureSpellEffect::Damage {
            amount: 6,
            school: 1,
            dmg_class: SPELL_DAMAGE_CLASS_MAGIC,
            attributes_ex2: SPELL_ATTR_EX2_CANT_CRIT,
            attributes_ex3: TEST_SPELL_ATTR_EX3_ALWAYS_HIT,
        },
        aura: None,
        range: None,
        mana_cost: 0,
        cast_time_millis: 1_500,
        due_at: now + Duration::from_millis(1_500),
    })
    .unwrap()
    .expect("cast should start");
    map.players.get_mut(&1).expect("player").health = 0;

    let interrupted = map
        .complete_ready_db_creature_spell_cast(
            creature_guid,
            victim,
            now + Duration::from_millis(1_500),
        )
        .unwrap()
        .expect("dead target completion should send an interrupted cast cleanup");
    let DbCreatureCompletedSpellEffect::Interrupted(interrupted) = interrupted.effect else {
        panic!("dead target completion should be interrupted");
    };
    assert_eq!(interrupted.failure, SPELL_FAILED_OUT_OF_RANGE);
    assert!(interrupted
        .observer_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgSpellFailure as u16));
    assert!(interrupted
        .observer_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgSpellFailedOther as u16));
    assert!(map
        .active_db_creature_spell_cast_due_at(creature_guid)
        .is_none());
}

#[test]
fn map_runtime_weapon_spell_avoid_sends_spell_miss_log_without_damage_log() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    insert_map_runtime_player_for_test(&mut map, 2, observer_position);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 179;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    spawn.template.min_level_health = 30;
    spawn.template.max_level_health = 30;
    let creature_guid = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);

    let event = map
        .apply_db_creature_damage(DbCreatureDamageRequest {
            creature_guid,
            killer: ObjectGuid::new(HighGuid::Player, 0, 1),
            damage: 0,
            melee_outcome: Some(MeleeDamageOutcome {
                hit_info: HITINFO_NORMALSWING2,
                victim_state: VICTIMSTATE_DODGE,
                outcome: MeleeHitOutcome::Dodge,
                total_damage: 0,
                school_damage: 0,
                absorbed: 0,
                resisted: 0,
                blocked: 0,
            }),
            spell_damage_outcome: None,
            spell_id: Some(1752),
            spell_school: 0,
            suppress_attacker_state: true,
            now: Instant::now(),
            now_epoch_secs: 2_000,
            exclude_character_guid: Some(1),
            corpse_loot: None,
        })
        .unwrap()
        .expect("spell miss event");

    assert!(event.spell_non_melee_log_body.is_none());
    assert!(event.attacker_state_body.is_none());
    let miss_log = event.spell_miss_log_body.expect("spell miss log");
    let mut cursor = 0;
    assert_eq!(read_u32(&miss_log, &mut cursor).unwrap(), 1752);
    cursor += 8; // caster raw GUID
    assert_eq!(miss_log[cursor], 0);
    cursor += 1;
    assert_eq!(read_u32(&miss_log, &mut cursor).unwrap(), 1);
    cursor += 8; // target raw GUID
    assert_eq!(miss_log[cursor], SPELL_MISS_DODGE);
    assert!(event
        .observer_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgSpellLogMiss as u16));
    assert_eq!(map.db_creature_snapshot(creature_guid).unwrap().health, 30);
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
            spell_damage_outcome: None,
            spell_id: None,
            spell_school: 0,
            suppress_attacker_state: false,
            now,
            now_epoch_secs: 2_000,
            exclude_character_guid: Some(1),
            corpse_loot: None,
        })
        .unwrap()
        .expect("death event");

    let finalization = event
        .death_finalization
        .as_ref()
        .expect("death should produce one finalization event");
    assert_eq!(finalization.killed, creature_guid);
    assert_eq!(finalization.respawn_epoch_secs, Some(2_007));
    assert_eq!(
        finalization.combat_flag_packet.opcode,
        WorldOpcode::SmsgUpdateObject as u16
    );
    assert_eq!(
        finalization.attack_stop_packet.opcode,
        WorldOpcode::SmsgAttackStop as u16
    );
    assert_eq!(
        &finalization.attack_stop_packet.body[finalization.attack_stop_packet.body.len() - 4..],
        &0u32.to_le_bytes(),
        "CMaNGOS writes attacker IsDead() in SMSG_ATTACKSTOP; living player killers must send 0"
    );
    assert_eq!(finalization.observer_packets.len(), 2);
    assert!(finalization
        .observer_packets
        .iter()
        .all(|(session_id, _)| *session_id == SessionId(2)));
    assert!(finalization
        .observer_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgUpdateObject as u16));
    assert!(finalization
        .observer_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgAttackStop as u16));
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
    assert!(event.observer_packets.iter().any(|(session_id, packet)| {
        *session_id == SessionId(2) && packet.opcode == WorldOpcode::SmsgAttackerStateUpdate as u16
    }));
    assert!(event.observer_packets.iter().any(|(session_id, packet)| {
        *session_id == SessionId(2) && packet.opcode == WorldOpcode::SmsgUpdateObject as u16
    }));
    let player_combat_clear_body = build_unit_flags_update_body(
        ObjectGuid::new(HighGuid::Player, 0, 1),
        UNIT_FLAG_PLAYER_CONTROLLED,
    )
    .unwrap();
    assert!(event.observer_packets.iter().any(|(session_id, packet)| {
        *session_id == SessionId(1)
            && packet.opcode == WorldOpcode::SmsgUpdateObject as u16
            && packet.body == player_combat_clear_body
    }));
    assert!(map
        .apply_db_creature_damage(DbCreatureDamageRequest {
            creature_guid,
            killer: ObjectGuid::new(HighGuid::Player, 0, 2),
            damage: 9_999,
            melee_outcome: None,
            spell_damage_outcome: None,
            spell_id: None,
            spell_school: 0,
            suppress_attacker_state: false,
            now,
            now_epoch_secs: 2_001,
            exclude_character_guid: Some(2),
            corpse_loot: None,
        },)
        .unwrap()
        .is_none());
}

#[test]
fn map_runtime_death_update_uses_corpse_created_group_loot_rights() {
    let mut map = MapRuntime::new(0, 0);
    insert_map_runtime_player_for_test(&mut map, 1, WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0));
    insert_map_runtime_player_for_test(&mut map, 2, WorldPosition::new(0, 1.0, 0.0, 0.0, 0.0));
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 178;
    spawn.position_x = 0.5;
    spawn.position_y = 0.0;
    spawn.template.min_level_health = 10;
    spawn.template.max_level_health = 10;
    spawn.template.min_loot_gold = 0;
    spawn.template.max_loot_gold = 0;
    let creature_guid = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);

    let event = map
        .apply_db_creature_damage(DbCreatureDamageRequest {
            creature_guid,
            killer: ObjectGuid::new(HighGuid::Player, 0, 1),
            damage: 10,
            melee_outcome: None,
            spell_damage_outcome: None,
            spell_id: None,
            spell_school: 0,
            suppress_attacker_state: false,
            now: Instant::now(),
            now_epoch_secs: 1_000,
            exclude_character_guid: Some(1),
            corpse_loot: Some(DbCreatureCorpseLootInit {
                owner: CreatureLootOwner::Party(9),
                allowed_players: vec![1, 2],
                current_looter: Some(1),
                loot_method: Some(CreatureLootMethod {
                    method: 1,
                    threshold: 2,
                    master_looter: 1,
                }),
                loot_items: vec![DbCreatureLootRuntime {
                    slot: 0,
                    item: 117,
                    count: 1,
                    display_id: 641,
                    quality: 1,
                    free_for_all: false,
                    quest_drop: false,
                }],
            }),
        })
        .unwrap()
        .expect("death event");

    let (direct_values, _) = decode_values_update_block(&event.update_body[5..], creature_guid);
    assert_eq!(
        direct_values[UNIT_DYNAMIC_FLAGS],
        Some(UNIT_DYNFLAG_LOOTABLE)
    );
    let observer_update = event
        .observer_packets
        .iter()
        .find(|(session_id, packet)| {
            *session_id == SessionId(2) && packet.opcode == WorldOpcode::SmsgUpdateObject as u16
        })
        .expect("observer update packet");
    let (observer_values, _) =
        decode_values_update_block(&observer_update.1.body[5..], creature_guid);
    assert_eq!(observer_values[UNIT_DYNAMIC_FLAGS], Some(0));
    assert_eq!(
        map.db_creature_needs_loot_item(creature_guid.raw()),
        Some(false)
    );
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
    map.players
        .get_mut(&1)
        .unwrap()
        .visible_objects
        .insert(creature_guid);
    map.players
        .get_mut(&2)
        .unwrap()
        .visible_objects
        .insert(creature_guid);
    let now = Instant::now();
    map.begin_db_creature_combat(creature_guid, player_a, now)
        .unwrap();

    let a_damage = map
        .apply_db_creature_damage(DbCreatureDamageRequest {
            creature_guid,
            killer: player_a,
            damage: 10,
            melee_outcome: None,
            spell_damage_outcome: None,
            spell_id: None,
            spell_school: 0,
            suppress_attacker_state: false,
            now,
            now_epoch_secs: 1_000,
            exclude_character_guid: Some(1),
            corpse_loot: None,
        })
        .unwrap()
        .expect("A damage should apply");
    assert_eq!(a_damage.creature.health, 20);
    assert_eq!(map.creatures.get(&creature_guid.raw()).unwrap().health, 20);
    assert!(a_damage
        .observer_packets
        .iter()
        .any(|(session_id, packet)| *session_id == SessionId(2)
            && packet.opcode == WorldOpcode::SmsgUpdateObject as u16));

    let b_damage = map
        .apply_db_creature_damage(DbCreatureDamageRequest {
            creature_guid,
            killer: player_b,
            damage: 15,
            melee_outcome: None,
            spell_damage_outcome: None,
            spell_id: None,
            spell_school: 0,
            suppress_attacker_state: false,
            now,
            now_epoch_secs: 1_001,
            exclude_character_guid: Some(2),
            corpse_loot: None,
        })
        .unwrap()
        .expect("B damage should apply to the same shared creature");
    assert_eq!(b_damage.creature.health, 5);
    assert_eq!(map.creatures.get(&creature_guid.raw()).unwrap().health, 5);
    assert!(b_damage
        .observer_packets
        .iter()
        .any(|(session_id, packet)| *session_id == SessionId(1)
            && packet.opcode == WorldOpcode::SmsgUpdateObject as u16));

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
            run: true,
        });
    }
    let death = map
        .apply_db_creature_damage(DbCreatureDamageRequest {
            creature_guid,
            killer: player_a,
            damage: 99,
            melee_outcome: None,
            spell_damage_outcome: None,
            spell_id: None,
            spell_school: 0,
            suppress_attacker_state: false,
            now,
            now_epoch_secs: 1_002,
            exclude_character_guid: Some(1),
            corpse_loot: None,
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
        Some(WorldOpcode::SmsgMonsterMove as u16)
    );
    assert!(death_finalization
        .observer_packets
        .iter()
        .any(|(session_id, packet)| *session_id == SessionId(2)
            && packet.opcode == WorldOpcode::SmsgMonsterMove as u16));
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
            spell_damage_outcome: None,
            spell_id: None,
            spell_school: 0,
            suppress_attacker_state: false,
            now,
            now_epoch_secs: 1_003,
            exclude_character_guid: Some(2),
            corpse_loot: None,
        })
        .unwrap()
        .is_none());

    assert!(map
        .open_db_creature_loot(
            creature_guid.raw(),
            1,
            CreatureLootOwner::Player(1),
            None,
            Vec::new(),
        )
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
    assert!(corpse_events.is_empty());
    let corpse_events = map
        .advance_db_creature_lifecycle(
            &[creature_guid.raw()],
            player_a_position,
            Some(1),
            now + Duration::from_secs(120),
        )
        .unwrap();
    assert_eq!(corpse_events.len(), 1);
    assert_eq!(
        corpse_events[0].creature.life_state,
        DbCreatureLifeState::Dead
    );
    let respawn_events = map
        .advance_db_creature_lifecycle(
            &[creature_guid.raw()],
            player_b_position,
            Some(2),
            now + Duration::from_secs(120),
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
            player_a_position,
            Some(1),
            now + Duration::from_secs(120),
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
fn map_runtime_event_ai_hp_flee_for_assist_starts_data_driven_flee() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    let player = ObjectGuid::new(HighGuid::Player, 0, 1);
    let mut spawn = test_creature_spawn(97);
    spawn.guid = 1901;
    spawn.position_x = 10.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    spawn.template.min_level_health = 100;
    spawn.template.max_level_health = 100;
    let creature_guid = creature_spawn_guid(&spawn);
    let mut creature = DbCreatureRuntime::new(spawn);
    creature.health = 10;
    map.share_db_creature_snapshots(vec![creature]);
    let now = Instant::now();
    map.begin_db_creature_combat(creature_guid, player, now)
        .unwrap();

    let event = map
        .process_db_creature_event_ai_hp_actions(
            &DbCreatureNavigationGuardrail::default(),
            creature_guid,
            player,
            &[test_creature_ai_flee_script(77, 97, 15)],
            now,
            Some(1),
        )
        .unwrap()
        .expect("HP EventAI flee action should start CMaNGOS flee motion");
    assert!(event.observer_packets.is_empty());
    assert_eq!(
        event.direct_packets[0].opcode,
        WorldOpcode::SmsgUpdateObject as u16
    );
    assert_eq!(
        event.direct_packets[1].opcode,
        WorldOpcode::SmsgMonsterMove as u16
    );
    let packed_guid_mask = event.direct_packets[0].body[6];
    let values_start = 4 + 1 + 1 + 1 + packed_guid_mask.count_ones() as usize;
    let values = decode_update_values(&event.direct_packets[0].body[values_start..]);
    let flags = values[UNIT_FIELD_FLAGS].expect("creature flags update");
    assert_eq!(flags & UNIT_FLAG_IN_COMBAT, UNIT_FLAG_IN_COMBAT);
    assert_eq!(flags & UNIT_FLAG_FLEEING, UNIT_FLAG_FLEEING);
    assert_eq!(
        event.direct_packets[1].body[PackedGuid::packed_size(creature_guid) + 12 + 4],
        MONSTER_MOVE_TYPE_NORMAL
    );
    let snapshot = map
        .db_creature_snapshot(creature_guid)
        .expect("creature should remain loaded");
    let CreatureMotionState::Flee(flee) = &snapshot.motion else {
        panic!("flee script should install flee motion");
    };
    assert_eq!(flee.source, player);
    assert!(flee.destination.x > snapshot.home_position.x);
    assert!(snapshot.triggered_event_ai_scripts.contains(&77));
}

#[test]
fn map_runtime_event_ai_hp_flee_for_assist_respects_threshold_and_one_shot_flag() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    let player = ObjectGuid::new(HighGuid::Player, 0, 1);
    let mut spawn = test_creature_spawn(97);
    spawn.guid = 1902;
    spawn.position_x = 10.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    spawn.template.min_level_health = 100;
    spawn.template.max_level_health = 100;
    let creature_guid = creature_spawn_guid(&spawn);
    let mut creature = DbCreatureRuntime::new(spawn);
    creature.health = 50;
    map.share_db_creature_snapshots(vec![creature]);
    let now = Instant::now();
    map.begin_db_creature_combat(creature_guid, player, now)
        .unwrap();
    let navigation = DbCreatureNavigationGuardrail::default();
    let scripts = [test_creature_ai_flee_script(78, 97, 15)];

    assert!(map
        .process_db_creature_event_ai_hp_actions(
            &navigation,
            creature_guid,
            player,
            &scripts,
            now,
            Some(1),
        )
        .unwrap()
        .is_none());

    map.creatures.get_mut(&creature_guid.raw()).unwrap().health = 10;
    assert!(map
        .process_db_creature_event_ai_hp_actions(
            &navigation,
            creature_guid,
            player,
            &scripts,
            now,
            Some(1),
        )
        .unwrap()
        .is_some());
    map.advance_db_creature_motion(
        creature_guid,
        now + CMANGOS_CREATURE_FAMILY_FLEE_DELAY + Duration::from_millis(1),
    );
    assert!(matches!(
        map.db_creature_snapshot(creature_guid).unwrap().motion,
        CreatureMotionState::Idle
    ));
    assert!(map
        .process_db_creature_event_ai_hp_actions(
            &navigation,
            creature_guid,
            player,
            &scripts,
            now + CMANGOS_CREATURE_FAMILY_FLEE_DELAY + Duration::from_millis(2),
            Some(1),
        )
        .unwrap()
        .is_none());
}

#[test]
fn map_runtime_event_ai_hp_set_walk_chase_retimes_active_chase() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, 10.0, 0.0, 0.0, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    let player = ObjectGuid::new(HighGuid::Player, 0, 1);
    let mut spawn = test_creature_spawn(97);
    spawn.guid = 1903;
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    spawn.template.min_level_health = 100;
    spawn.template.max_level_health = 100;
    let creature_guid = creature_spawn_guid(&spawn);
    let mut creature = DbCreatureRuntime::new(spawn);
    creature.health = 10;
    map.share_db_creature_snapshots(vec![creature]);
    let now = Instant::now();
    map.begin_db_creature_combat(creature_guid, player, now)
        .unwrap();
    let navigation = DbCreatureNavigationGuardrail::default();
    map.start_db_creature_chase_motion(&navigation, creature_guid, player, player_position, now)
        .expect("initial chase should run");

    let event = map
        .process_db_creature_event_ai_hp_actions(
            &navigation,
            creature_guid,
            player,
            &[test_creature_ai_set_walk_script(
                79,
                97,
                15,
                EVENT_AI_WALK_SETTING_WALK_CHASE,
            )],
            now + Duration::from_millis(250),
            Some(1),
        )
        .unwrap()
        .expect("HP EventAI set-walk action should retime the chase spline");
    assert_eq!(event.direct_packets.len(), 1);
    assert_eq!(
        event.direct_packets[0].opcode,
        WorldOpcode::SmsgMonsterMove as u16
    );
    let mut cursor = PackedGuid::packed_size(creature_guid) + 12 + 4;
    assert_eq!(
        event.direct_packets[0].body[cursor],
        MONSTER_MOVE_TYPE_FACING_TARGET
    );
    cursor += 1 + 8;
    assert_eq!(
        u32::from_le_bytes(
            event.direct_packets[0].body[cursor..cursor + 4]
                .try_into()
                .unwrap()
        ),
        0
    );
    let snapshot = map
        .db_creature_snapshot(creature_guid)
        .expect("creature should remain loaded");
    assert!(!snapshot.chase_run);
    let CreatureMotionState::Chase(chase) = snapshot.motion else {
        panic!("set walk should keep the creature chasing");
    };
    assert!(!chase.run);
    assert!(snapshot.triggered_event_ai_scripts.contains(&79));
}

#[test]
fn map_runtime_event_ai_hp_set_walk_chase_affects_next_chase_motion() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, 10.0, 0.0, 0.0, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    let player = ObjectGuid::new(HighGuid::Player, 0, 1);
    let mut spawn = test_creature_spawn(97);
    spawn.guid = 1904;
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    spawn.template.min_level_health = 100;
    spawn.template.max_level_health = 100;
    spawn.template.speed_walk = 1.0;
    spawn.template.speed_run = 1.0;
    let creature_guid = creature_spawn_guid(&spawn);
    let mut creature = DbCreatureRuntime::new(spawn);
    creature.health = 10;
    map.share_db_creature_snapshots(vec![creature]);
    let now = Instant::now();
    map.begin_db_creature_combat(creature_guid, player, now)
        .unwrap();
    let navigation = DbCreatureNavigationGuardrail::default();

    let event = map
        .process_db_creature_event_ai_hp_actions(
            &navigation,
            creature_guid,
            player,
            &[test_creature_ai_set_walk_script(
                80,
                97,
                15,
                EVENT_AI_WALK_SETTING_WALK_CHASE,
            )],
            now,
            Some(1),
        )
        .unwrap()
        .expect("HP EventAI set-walk action should execute even before chase starts");
    assert!(event.direct_packets.is_empty());
    let (_, motion) = map
        .start_db_creature_chase_motion(&navigation, creature_guid, player, player_position, now)
        .expect("chase should start after EventAI walk setting");
    assert!(!motion.run);
    let expected = db_creature_walk_path_motion_duration(
        motion.start,
        &motion.path,
        DB_CREATURE_WALK_SPEED_YARDS_PER_SEC,
    );
    assert_eq!(motion.duration, expected);
}

#[test]
fn map_runtime_event_ai_timer_in_combat_schedules_cast() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, 5.0, 0.0, 0.0, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    let player = ObjectGuid::new(HighGuid::Player, 0, 1);
    let mut spawn = test_creature_spawn(40);
    spawn.guid = 1905;
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let creature_guid = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let now = Instant::now();
    map.begin_db_creature_combat(creature_guid, player, now)
        .unwrap();
    let scripts = [test_creature_ai_cast_script(
        4002,
        40,
        EVENT_AI_EVENT_TIMER_IN_COMBAT,
        [4_000, 4_000, 38_000, 42_000],
        6016,
        EVENT_AI_TARGET_HOSTILE,
    )];

    assert!(map
        .ready_db_creature_event_ai_spell_cast(creature_guid, player, &scripts, now)
        .is_none());
    assert!(map
        .ready_db_creature_event_ai_spell_cast(
            creature_guid,
            player,
            &scripts,
            now + Duration::from_millis(DB_CREATURE_EVENT_AI_UPDATE_INTERVAL.as_millis() as u64 - 1),
        )
        .is_none());

    let ready = map
        .ready_db_creature_event_ai_spell_cast(
            creature_guid,
            player,
            &scripts,
            now + Duration::from_millis(4_000),
        )
        .expect("timer-in-combat EventAI cast should become ready after initial delay");
    assert_eq!(ready.spell_id, 6016);
    assert_eq!(ready.target, player);
    map.apply_db_creature_event_ai_spell_cooldown(creature_guid, &ready, now);
    assert!(map
        .creatures
        .get(&creature_guid.raw())
        .unwrap()
        .event_ai_cooldowns_until
        .contains_key(&4002));
}

#[test]
fn map_runtime_event_ai_zero_initial_timer_waits_for_cmangos_update_pulse() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, 5.0, 0.0, 0.0, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    let player = ObjectGuid::new(HighGuid::Player, 0, 1);
    let mut spawn = test_creature_spawn(40);
    spawn.guid = 1915;
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let creature_guid = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let now = Instant::now();
    map.begin_db_creature_combat(creature_guid, player, now)
        .unwrap();
    let scripts = [test_creature_ai_cast_script(
        4015,
        40,
        EVENT_AI_EVENT_TIMER_IN_COMBAT,
        [0, 0, 38_000, 42_000],
        6016,
        EVENT_AI_TARGET_HOSTILE,
    )];

    assert!(
        map.ready_db_creature_event_ai_spell_cast(
            creature_guid,
            player,
            &scripts,
            now + DB_CREATURE_EVENT_AI_UPDATE_INTERVAL - Duration::from_millis(1),
        )
        .is_none(),
        "CMaNGOS EventAI does not process timer events before the first 500ms update pulse"
    );
    let ready = map
        .ready_db_creature_event_ai_spell_cast(
            creature_guid,
            player,
            &scripts,
            now + DB_CREATURE_EVENT_AI_UPDATE_INTERVAL,
        )
        .expect("zero-initial timer should become ready on the first EventAI update pulse");
    assert_eq!(ready.spell_id, 6016);
    assert_eq!(ready.target, player);
}

#[test]
fn map_runtime_event_ai_aggro_cast_targets_self() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, 5.0, 0.0, 0.0, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    let player = ObjectGuid::new(HighGuid::Player, 0, 1);
    let mut spawn = test_creature_spawn(103);
    spawn.guid = 1906;
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let creature_guid = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let now = Instant::now();
    map.begin_db_creature_combat(creature_guid, player, now)
        .unwrap();
    let mut script = test_creature_ai_cast_script(
        10301,
        103,
        EVENT_AI_EVENT_AGGRO,
        [0, 0, 0, 0],
        7164,
        EVENT_AI_TARGET_SELF,
    );
    script.event_flags = 0;

    let ready = map
        .ready_db_creature_event_ai_spell_cast(creature_guid, player, &[script.clone()], now)
        .expect("aggro EventAI cast should be ready immediately");
    assert_eq!(ready.target, creature_guid);
    map.apply_db_creature_event_ai_spell_cooldown(creature_guid, &ready, now);
    assert!(map
        .ready_db_creature_event_ai_spell_cast(
            creature_guid,
            player,
            &[script],
            now + Duration::from_millis(1),
        )
        .is_none());
}

#[test]
fn map_runtime_event_ai_range_and_missing_aura_select_casts() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, 30.0, 0.0, 0.0, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    let player = ObjectGuid::new(HighGuid::Player, 0, 1);
    let mut spawn = test_creature_spawn(476);
    spawn.guid = 1907;
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let creature_guid = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let now = Instant::now();
    map.begin_db_creature_combat(creature_guid, player, now)
        .unwrap();
    let fireball = test_creature_ai_cast_script(
        47604,
        476,
        EVENT_AI_EVENT_RANGE,
        [0, 40, 3_600, 4_800],
        20793,
        EVENT_AI_TARGET_HOSTILE,
    );
    let frost_armor = test_creature_ai_cast_script(
        47603,
        476,
        EVENT_AI_EVENT_MISSING_AURA,
        [12544, 1, 5_000, 5_000],
        12544,
        EVENT_AI_TARGET_SELF,
    );
    let event_ai_tick = DB_CREATURE_EVENT_AI_UPDATE_INTERVAL;

    let ready = map
        .ready_db_creature_event_ai_spell_cast(creature_guid, player, &[fireball], now + event_ai_tick)
        .expect("range EventAI should cast when target is inside configured distance");
    assert_eq!(ready.spell_id, 20793);
    assert_eq!(ready.target, player);

    let ready = map
        .ready_db_creature_event_ai_spell_cast(
            creature_guid,
            player,
            std::slice::from_ref(&frost_armor),
            now + event_ai_tick + event_ai_tick,
        )
        .expect("missing-aura EventAI should cast when the aura is absent");
    assert_eq!(ready.spell_id, 12544);
    assert_eq!(ready.target, creature_guid);

    let template = frost_armor_spell_template();
    let aura = build_active_aura(
        &template,
        creature_guid,
        6,
        test_spell_effect_value_context(&template),
        now,
        None,
    );
    map.creatures
        .get_mut(&creature_guid.raw())
        .unwrap()
        .active_auras
        .push(aura);
    assert!(map
        .ready_db_creature_event_ai_spell_cast(
            creature_guid,
            player,
            &[frost_armor],
            now + event_ai_tick + event_ai_tick + event_ai_tick,
        )
        .is_none());
}

#[test]
fn map_runtime_event_ai_facing_target_matches_cmangos_position_and_repeat_rules() {
    let mut map = MapRuntime::new(0, 0);
    insert_map_runtime_player_for_test(&mut map, 1, WorldPosition::new(0, 5.0, 0.0, 0.0, 0.0));
    let player = ObjectGuid::new(HighGuid::Player, 0, 1);
    let mut spawn = test_creature_spawn(94);
    spawn.guid = 1911;
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let creature_guid = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let now = Instant::now();
    map.begin_db_creature_combat(creature_guid, player, now)
        .unwrap();
    let event_ai_tick = DB_CREATURE_EVENT_AI_UPDATE_INTERVAL;

    let backstab_without_repeat_timer = test_creature_ai_cast_script(
        9401,
        94,
        EVENT_AI_EVENT_FACING_TARGET,
        [0, 0, 0, 0],
        53,
        EVENT_AI_TARGET_HOSTILE,
    );
    let ready = map
        .ready_db_creature_event_ai_spell_cast(
            creature_guid,
            player,
            std::slice::from_ref(&backstab_without_repeat_timer),
            now + event_ai_tick,
        )
        .expect("creature should be behind the player and inside the CMaNGOS 5yd check");
    assert_eq!(ready.spell_id, 53);
    map.apply_db_creature_event_ai_spell_cooldown(creature_guid, &ready, now);
    assert!(map
        .ready_db_creature_event_ai_spell_cast(
            creature_guid,
            player,
            &[backstab_without_repeat_timer],
            now + Duration::from_millis(1),
        )
        .is_none());

    let front_only = test_creature_ai_cast_script(
        9402,
        94,
        EVENT_AI_EVENT_FACING_TARGET,
        [1, 0, 5_000, 5_000],
        53,
        EVENT_AI_TARGET_HOSTILE,
    );
    assert!(map
        .ready_db_creature_event_ai_spell_cast(
            creature_guid,
            player,
            std::slice::from_ref(&front_only),
            now + event_ai_tick + event_ai_tick,
        )
        .is_none());

    map.creatures
        .get_mut(&creature_guid.raw())
        .unwrap()
        .current_position = WorldPosition::new(0, 7.0, 0.0, 0.0, 0.0);
    assert!(map
        .ready_db_creature_event_ai_spell_cast(
            creature_guid,
            player,
            &[front_only],
            now + event_ai_tick + event_ai_tick + event_ai_tick,
        )
        .is_some());

    map.creatures
        .get_mut(&creature_guid.raw())
        .unwrap()
        .current_position = WorldPosition::new(0, -1.0, 0.0, 0.0, 0.0);
    let far_backstab = test_creature_ai_cast_script(
        9403,
        94,
        EVENT_AI_EVENT_FACING_TARGET,
        [0, 0, 5_000, 5_000],
        53,
        EVENT_AI_TARGET_HOSTILE,
    );
    assert!(map
        .ready_db_creature_event_ai_spell_cast(
            creature_guid,
            player,
            &[far_backstab],
            now + event_ai_tick + event_ai_tick + event_ai_tick + event_ai_tick,
        )
        .is_none());
}

#[test]
fn map_runtime_event_ai_ooc_timer_and_spawned_select_self_casts() {
    let mut map = MapRuntime::new(0, 0);
    let mut spawn = test_creature_spawn(68);
    spawn.guid = 1909;
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let creature_guid = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let now = Instant::now();

    let ooc = test_creature_ai_cast_script(
        6801,
        68,
        EVENT_AI_EVENT_TIMER_OOC,
        [2_000, 2_000, 5_000, 5_000],
        18950,
        EVENT_AI_TARGET_SELF,
    );
    assert!(map
        .ready_db_creature_event_ai_ooc_spell_cast(creature_guid, std::slice::from_ref(&ooc), now,)
        .is_none());
    let ready = map
        .ready_db_creature_event_ai_ooc_spell_cast(
            creature_guid,
            &[ooc],
            now + Duration::from_millis(2_000),
        )
        .expect("OOC EventAI timer should become ready after its initial delay");
    assert_eq!(ready.spell_id, 18950);
    assert_eq!(ready.target, creature_guid);

    let mut spawned = test_creature_ai_cast_script(
        6802,
        68,
        EVENT_AI_EVENT_SPAWNED,
        [EVENT_AI_SPAWNED_ALWAYS, 0, 0, 0],
        9036,
        EVENT_AI_TARGET_SELF,
    );
    spawned.event_flags = 0;
    let ready = map
        .ready_db_creature_event_ai_ooc_spell_cast(creature_guid, &[spawned.clone()], now)
        .expect("spawned EventAI should execute once for always condition");
    assert_eq!(ready.spell_id, 9036);
    map.apply_db_creature_event_ai_spell_cooldown(creature_guid, &ready, now);
    assert!(map
        .ready_db_creature_event_ai_ooc_spell_cast(creature_guid, &[spawned], now)
        .is_none());
}

#[tokio::test]
async fn map_runtime_manager_ooc_event_ai_tick_runs_without_viewer_session() {
    let maps = Arc::new(MapRuntimeManager::default());
    let object_mgr = ObjectMgr::default();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let map_id = 0;
    let mut spawn = test_creature_spawn(6810);
    spawn.guid = 6810;
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let creature_guid = creature_spawn_guid(&spawn);
    maps.share_db_creature_snapshots(map_id, vec![DbCreatureRuntime::new(spawn)])
        .await;
    object_mgr
        .prime_creature_ai_scripts_for_test(
            6810,
            vec![test_creature_ai_cast_script(
                681_001,
                6810,
                EVENT_AI_EVENT_TIMER_OOC,
                [0, 0, 5_000, 5_000],
                12544,
                EVENT_AI_TARGET_SELF,
            )],
        )
        .await;
    object_mgr
        .prime_spell_template_for_test(12544, Some(frost_armor_spell_template()))
        .await;
    let now = Instant::now();

    let tick = maps
        .advance_all_db_creature_ooc_event_ai_spell_ticks(
            &world_db_pool,
            &object_mgr,
            &DbCreatureNavigationGuardrail::default(),
            now,
            DB_CREATURE_EVENT_AI_UPDATE_INTERVAL,
        )
        .await
        .expect("map-owned OOC EventAI tick should succeed");
    assert!(tick.packets.is_empty());

    let map = maps.get_or_create_map(map_id, 0).await;
    let map = map.lock().await;
    let creature = map
        .creatures
        .get(&creature_guid.raw())
        .expect("creature should remain loaded");
    assert!(!creature.triggered_event_ai_scripts.contains(&681_001));
    assert!(!creature.event_ai_cooldowns_until.contains_key(&681_001));
    assert!(!matches!(
        map.db_creature_ooc_event_ai_capabilities.get(&6810),
        Some(DbCreatureOocEventAiCapability::OocCast(_))
    ));
}

#[tokio::test]
async fn map_runtime_manager_ooc_event_ai_tick_dispatches_packets_to_nearby_players() {
    let maps = Arc::new(MapRuntimeManager::default());
    let object_mgr = ObjectMgr::default();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let map_id = 0;
    let player_position = WorldPosition::new(map_id, 5.0, 0.0, 0.0, 0.0);
    maps.add_player(test_player_runtime(77, SessionId(77), player_position))
        .await
        .expect("player should be added");
    let mut spawn = test_creature_spawn(6811);
    spawn.guid = 6811;
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    maps.share_db_creature_snapshots(map_id, vec![DbCreatureRuntime::new(spawn)])
        .await;
    object_mgr
        .prime_creature_ai_scripts_for_test(
            6811,
            vec![test_creature_ai_cast_script(
                681_002,
                6811,
                EVENT_AI_EVENT_TIMER_OOC,
                [0, 0, 5_000, 5_000],
                12544,
                EVENT_AI_TARGET_SELF,
            )],
        )
        .await;
    object_mgr
        .prime_spell_template_for_test(12544, Some(frost_armor_spell_template()))
        .await;
    let now = Instant::now();

    let tick = maps
        .advance_all_db_creature_ooc_event_ai_spell_ticks(
            &world_db_pool,
            &object_mgr,
            &DbCreatureNavigationGuardrail::default(),
            now,
            DB_CREATURE_EVENT_AI_UPDATE_INTERVAL,
        )
        .await
        .expect("map-owned OOC EventAI tick should succeed");

    assert!(tick
        .packets
        .iter()
        .any(|(session_id, packet)| *session_id == SessionId(77)
            && packet.opcode == WorldOpcode::SmsgSpellStart as u16));
    assert!(tick
        .packets
        .iter()
        .any(|(session_id, packet)| *session_id == SessionId(77)
            && packet.opcode == WorldOpcode::SmsgSpellGo as u16));
}

#[tokio::test]
async fn map_runtime_manager_ooc_event_ai_classification_does_not_requeue_same_entry_siblings() {
    let mut map = MapRuntime::new(0, 0);

    let mut first = test_creature_spawn(6812);
    first.guid = 68_120;

    let mut second = first.clone();
    second.guid = 68_121;

    let first_guid = ObjectGuid::new(HighGuid::Unit, 0, first.guid);
    let second_guid = ObjectGuid::new(HighGuid::Unit, 0, second.guid);
    map.creatures
        .insert(first_guid.raw(), DbCreatureRuntime::new(first));
    map.creatures
        .insert(second_guid.raw(), DbCreatureRuntime::new(second));
    map.set_db_creature_ooc_event_ai_capability(
        6812,
        DbCreatureOocEventAiCapability::OocCast(Arc::from(vec![test_creature_ai_cast_script(
            681_003,
            6812,
            EVENT_AI_EVENT_TIMER_OOC,
            [5_000, 5_000, 5_000, 5_000],
            12544,
            EVENT_AI_TARGET_SELF,
        )])),
    );

    assert!(matches!(
        map.db_creature_ooc_event_ai_capabilities.get(&6812),
        Some(DbCreatureOocEventAiCapability::OocCast(_))
    ));

    let first_runtime = map
        .creatures
        .get(&first_guid.raw())
        .expect("first creature should remain loaded");
    assert_eq!(first_runtime.event_ai_update_accum, Duration::ZERO);
    assert!(!first_runtime
        .event_ai_cooldowns_until
        .contains_key(&681_003));

    let second_runtime = map
        .creatures
        .get(&second_guid.raw())
        .expect("second creature should remain loaded");
    assert_eq!(second_runtime.event_ai_update_accum, Duration::ZERO);
    assert!(!second_runtime
        .event_ai_cooldowns_until
        .contains_key(&681_003));
    assert!(!second_runtime.triggered_event_ai_scripts.contains(&681_003));
}

#[test]
fn map_runtime_ooc_event_ai_prepare_respects_cmangos_update_interval() {
    let mut map = MapRuntime::new(0, 0);
    let mut spawn = test_creature_spawn(6813);
    spawn.guid = 68_130;
    let creature_guid = creature_spawn_guid(&spawn);
    map.creatures
        .insert(creature_guid.raw(), DbCreatureRuntime::new(spawn));
    map.set_db_creature_ooc_event_ai_capability(
        6813,
        DbCreatureOocEventAiCapability::OocCast(Arc::from(vec![test_creature_ai_cast_script(
            681_004,
            6813,
            EVENT_AI_EVENT_TIMER_OOC,
            [0, 0, 5_000, 5_000],
            12544,
            EVENT_AI_TARGET_SELF,
        )])),
    );
    let now = Instant::now();

    assert!(map
        .prepare_ready_db_creature_ooc_event_ai_action(
            creature_guid.raw(),
            now,
            Duration::from_millis(100),
        )
        .is_none());

    let Some(ReadyDbCreatureOocEventAiAction::Start { ready, .. }) = map
        .prepare_ready_db_creature_ooc_event_ai_action(
            creature_guid.raw(),
            now + Duration::from_millis(500),
            Duration::from_millis(400),
        )
    else {
        panic!("interval-complete OOC EventAI tick should prepare a spell start");
    };
    assert_eq!(ready.spell_id, 12544);
}

#[test]
fn map_runtime_event_ai_target_modes_use_threat_list() {
    let mut map = MapRuntime::new(0, 0);
    insert_map_runtime_player_for_test(&mut map, 1, WorldPosition::new(0, 3.0, 0.0, 0.0, 0.0));
    insert_map_runtime_player_for_test(&mut map, 2, WorldPosition::new(0, 10.0, 0.0, 0.0, 0.0));
    let first = ObjectGuid::new(HighGuid::Player, 0, 1);
    let second = ObjectGuid::new(HighGuid::Player, 0, 2);
    let mut spawn = test_creature_spawn(40);
    spawn.guid = 1910;
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let creature_guid = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let now = Instant::now();
    map.begin_db_creature_combat(creature_guid, first, now)
        .unwrap();
    map.add_db_creature_threat(creature_guid, first, 20.0);
    map.add_db_creature_threat(creature_guid, second, 5.0);

    let second_aggro = test_creature_ai_cast_script(
        4010,
        40,
        EVENT_AI_EVENT_AGGRO,
        [0, 0, 0, 0],
        6016,
        EVENT_AI_TARGET_HOSTILE_SECOND_AGGRO,
    );
    let ready = map
        .ready_db_creature_event_ai_spell_cast(creature_guid, first, &[second_aggro], now)
        .expect("second-aggro EventAI target should select the second threat entry");
    assert_eq!(ready.target, second);

    let farthest = test_creature_ai_cast_script(
        4011,
        40,
        EVENT_AI_EVENT_AGGRO,
        [0, 0, 0, 0],
        6016,
        EVENT_AI_TARGET_HOSTILE_FARTHEST_AWAY,
    );
    let ready = map
        .ready_db_creature_event_ai_spell_cast(creature_guid, first, &[farthest], now)
        .expect("farthest EventAI target should select the farthest threat entry");
    assert_eq!(ready.target, second);
}

#[test]
fn map_runtime_creature_aura_only_spell_cast_applies_to_creature() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, 5.0, 0.0, 0.0, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    let player = ObjectGuid::new(HighGuid::Player, 0, 1);
    let mut spawn = test_creature_spawn(476);
    spawn.guid = 1908;
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let creature_guid = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let now = Instant::now();
    map.begin_db_creature_combat(creature_guid, player, now)
        .unwrap();
    let template = frost_armor_spell_template();
    let cast = map
        .prepare_db_creature_spell_cast_from_template(
            creature_guid,
            creature_guid,
            &template,
            None,
            None,
            None,
            now,
        )
        .expect("aura-only creature spell should prepare a real cast");
    assert!(matches!(cast.effect, ActiveDbCreatureSpellEffect::None));
    map.start_db_creature_spell_cast(cast)
        .unwrap()
        .expect("aura-only cast should start");

    let completed = map
        .complete_ready_db_creature_spell_cast(creature_guid, player, now)
        .unwrap()
        .expect("aura-only cast should complete");
    assert!(matches!(
        completed.effect,
        DbCreatureCompletedSpellEffect::AuraOnly
    ));
    assert!(completed.creature_aura_event.is_some());
    assert!(map
        .creatures
        .get(&creature_guid.raw())
        .unwrap()
        .active_auras
        .iter()
        .any(|aura| aura.spell_id == 12544));
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
        jump: JumpInfo::default(),
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

    let faction_templates = FactionTemplateStore::fallback_bridge();
    let first = map
        .select_db_creature_assist_targets(&faction_templates, caller, &character)
        .expect("caller should exist");
    assert_eq!(first.1, vec![helper]);
    assert!(first.0.already_called_assistance);
    let second = map
        .select_db_creature_assist_targets(&faction_templates, caller, &character)
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
fn map_runtime_db_creature_combat_packets_call_assistance() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let player_position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    insert_map_runtime_player_for_test(&mut map, 7, player_position);
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);

    let mut caller_spawn = test_creature_spawn(6);
    caller_spawn.guid = 192;
    caller_spawn.position_x = 0.0;
    caller_spawn.position_y = 0.0;
    caller_spawn.template.npc_flags = 0;
    caller_spawn.template.faction = 17;
    caller_spawn.template.call_for_help = 6;
    let caller = creature_spawn_guid(&caller_spawn);
    let mut helper_spawn = test_creature_spawn(6);
    helper_spawn.guid = 193;
    helper_spawn.position_x = 5.0;
    helper_spawn.position_y = 0.0;
    helper_spawn.template.npc_flags = 0;
    helper_spawn.template.faction = 17;
    let helper = creature_spawn_guid(&helper_spawn);
    map.share_db_creature_snapshots(vec![
        DbCreatureRuntime::new(caller_spawn),
        DbCreatureRuntime::new(helper_spawn),
    ]);

    let packets = map
        .begin_db_creature_combat_packets_with_assistance(
            caller,
            player,
            7,
            SessionId(7),
            now,
        )
        .unwrap();

    assert!(map.active_creature_combats.contains_key(&caller.raw()));
    assert!(map.active_creature_combats.contains_key(&helper.raw()));
    assert_eq!(
        packets
            .iter()
            .filter(|(_, packet)| packet.opcode == WorldOpcode::SmsgAttackStart as u16)
            .count(),
        2
    );
}

#[test]
fn map_runtime_active_combat_creature_pulls_help_after_aggro_delay() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let player_position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    insert_map_runtime_player_for_test(&mut map, 7, player_position);
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);

    let mut runner_spawn = test_creature_spawn(6);
    runner_spawn.guid = 194;
    runner_spawn.position_x = 0.0;
    runner_spawn.position_y = 0.0;
    runner_spawn.template.npc_flags = 0;
    runner_spawn.template.faction = 17;
    let runner = creature_spawn_guid(&runner_spawn);
    let mut helper_spawn = test_creature_spawn(6);
    helper_spawn.guid = 195;
    helper_spawn.position_x = 4.0;
    helper_spawn.position_y = 0.0;
    helper_spawn.template.npc_flags = 0;
    helper_spawn.template.faction = 17;
    let helper = creature_spawn_guid(&helper_spawn);
    map.share_db_creature_snapshots(vec![
        DbCreatureRuntime::new(runner_spawn),
        DbCreatureRuntime::new(helper_spawn),
    ]);
    map.begin_db_creature_combat(runner, player, now).unwrap();

    let navigation = DbCreatureNavigationGuardrail::default();
    let early_packets = map
        .db_creature_check_for_help_packets_on_relocation(
            runner,
            &navigation,
            now + Duration::from_millis(DB_CREATURE_CHECK_FOR_HELP_AGGRO_DELAY_MILLIS - 1),
        )
        .unwrap();
    assert!(early_packets.is_empty());
    assert!(!map.active_creature_combats.contains_key(&helper.raw()));

    let packets = map
        .db_creature_check_for_help_packets_on_relocation(
            runner,
            &navigation,
            now + Duration::from_millis(DB_CREATURE_CHECK_FOR_HELP_AGGRO_DELAY_MILLIS),
        )
        .unwrap();

    assert!(map.active_creature_combats.contains_key(&helper.raw()));
    assert!(packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgAttackStart as u16));
}

#[test]
fn map_runtime_idle_patrol_walking_by_active_fight_joins_combat() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let player_position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    insert_map_runtime_player_for_test(&mut map, 7, player_position);
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);

    let mut fighting_spawn = test_creature_spawn(6);
    fighting_spawn.guid = 196;
    fighting_spawn.position_x = 0.0;
    fighting_spawn.position_y = 0.0;
    fighting_spawn.template.npc_flags = 0;
    fighting_spawn.template.faction = 17;
    let fighting = creature_spawn_guid(&fighting_spawn);
    let mut patrol_spawn = test_creature_spawn(6);
    patrol_spawn.guid = 197;
    patrol_spawn.position_x = 4.0;
    patrol_spawn.position_y = 0.0;
    patrol_spawn.template.npc_flags = 0;
    patrol_spawn.template.faction = 17;
    let patrol = creature_spawn_guid(&patrol_spawn);
    map.share_db_creature_snapshots(vec![
        DbCreatureRuntime::new(fighting_spawn),
        DbCreatureRuntime::new(patrol_spawn),
    ]);
    map.begin_db_creature_combat(fighting, player, now)
        .unwrap();

    let packets = map
        .db_creature_check_for_help_packets_on_relocation(
            patrol,
            &DbCreatureNavigationGuardrail::default(),
            now + Duration::from_millis(DB_CREATURE_CHECK_FOR_HELP_AGGRO_DELAY_MILLIS),
        )
        .unwrap();

    assert!(map.active_creature_combats.contains_key(&patrol.raw()));
    assert!(packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgAttackStart as u16));
}

#[test]
fn gm_instakill_suppresses_damage_packet_but_keeps_killer_loot_rights() {
    let mut map = MapRuntime::new(0, 0);
    insert_map_runtime_player_for_test(&mut map, 1, WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0));
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 179;
    spawn.position_x = 0.5;
    spawn.position_y = 0.0;
    spawn.template.min_level_health = 10;
    spawn.template.max_level_health = 10;
    spawn.template.min_loot_gold = 0;
    spawn.template.max_loot_gold = 0;
    let creature_guid = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);

    let event = map
        .apply_db_creature_damage(DbCreatureDamageRequest {
            creature_guid,
            killer: ObjectGuid::new(HighGuid::Player, 0, 1),
            damage: 10,
            melee_outcome: None,
            spell_damage_outcome: None,
            spell_id: None,
            spell_school: 0,
            suppress_attacker_state: true,
            now: Instant::now(),
            now_epoch_secs: 1_000,
            exclude_character_guid: Some(1),
            corpse_loot: Some(DbCreatureCorpseLootInit {
                owner: CreatureLootOwner::Player(1),
                allowed_players: vec![1],
                current_looter: Some(1),
                loot_method: None,
                loot_items: vec![DbCreatureLootRuntime {
                    slot: 0,
                    item: 117,
                    count: 1,
                    display_id: 641,
                    quality: 1,
                    free_for_all: false,
                    quest_drop: false,
                }],
            }),
        })
        .unwrap()
        .expect("death event");

    assert!(
        event.attacker_state_body.is_none(),
        "GM instakill should not look like a melee damage packet"
    );
    let (direct_values, _) = decode_values_update_block(&event.update_body[5..], creature_guid);
    assert_eq!(
        direct_values[UNIT_DYNAMIC_FLAGS],
        Some(UNIT_DYNFLAG_LOOTABLE)
    );
    let corpse = map.db_creature_snapshot(creature_guid).unwrap();
    assert!(corpse.can_loot_for_player(Some(1)));
}

#[test]
fn gm_instakill_can_reclaim_existing_loot_owner_before_death() {
    let mut map = MapRuntime::new(0, 0);
    insert_map_runtime_player_for_test(&mut map, 1, WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0));
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 180;
    spawn.position_x = 0.5;
    spawn.position_y = 0.0;
    spawn.template.min_level_health = 10;
    spawn.template.max_level_health = 10;
    spawn.template.min_loot_gold = 0;
    spawn.template.max_loot_gold = 0;
    let creature_guid = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    map.force_db_creature_loot_owner(creature_guid, CreatureLootOwner::Player(2));

    let owner = CreatureLootOwner::Player(1);
    map.force_db_creature_loot_owner(creature_guid, owner);
    let event = map
        .apply_db_creature_damage(DbCreatureDamageRequest {
            creature_guid,
            killer: ObjectGuid::new(HighGuid::Player, 0, 1),
            damage: 10,
            melee_outcome: None,
            spell_damage_outcome: None,
            spell_id: None,
            spell_school: 0,
            suppress_attacker_state: true,
            now: Instant::now(),
            now_epoch_secs: 1_000,
            exclude_character_guid: Some(1),
            corpse_loot: Some(DbCreatureCorpseLootInit {
                owner,
                allowed_players: vec![1],
                current_looter: Some(1),
                loot_method: None,
                loot_items: vec![DbCreatureLootRuntime {
                    slot: 0,
                    item: 117,
                    count: 1,
                    display_id: 641,
                    quality: 1,
                    free_for_all: false,
                    quest_drop: false,
                }],
            }),
        })
        .unwrap()
        .expect("death event");

    let (direct_values, _) = decode_values_update_block(&event.update_body[5..], creature_guid);
    assert_eq!(
        direct_values[UNIT_DYNAMIC_FLAGS],
        Some(UNIT_DYNFLAG_LOOTABLE)
    );
    let corpse = map.db_creature_snapshot(creature_guid).unwrap();
    assert!(corpse.can_loot_for_player(Some(1)));
    assert!(!corpse.can_loot_for_player(Some(2)));
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
            spell_damage_outcome: None,
            spell_id: None,
            spell_school: 0,
            suppress_attacker_state: false,
            now: Instant::now(),
            now_epoch_secs: 2_000,
            exclude_character_guid: Some(1),
            corpse_loot: None,
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
    assert_eq!(
        event.observer_packets[0].1.opcode,
        WorldOpcode::SmsgAttackerStateUpdate as u16
    );
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
    map.players
        .get_mut(&1)
        .unwrap()
        .visible_objects
        .insert(creature_guid);
    map.players
        .get_mut(&2)
        .unwrap()
        .visible_objects
        .insert(creature_guid);
    let killed_at = Instant::now();
    map.apply_db_creature_damage(DbCreatureDamageRequest {
        creature_guid,
        killer: ObjectGuid::new(HighGuid::Player, 0, 1),
        damage: 9_999,
        melee_outcome: None,
        spell_damage_outcome: None,
        spell_id: None,
        spell_school: 0,
        suppress_attacker_state: false,
        now: killed_at,
        now_epoch_secs: 3_000,
        exclude_character_guid: Some(1),
        corpse_loot: None,
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
        WorldOpcode::SmsgDestroyObject as u16
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
        WorldOpcode::SmsgUpdateObject as u16
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

#[test]
fn map_runtime_creature_lifecycle_due_scan_does_not_require_player_visibility() {
    let mut map = MapRuntime::new(0, 0);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 89;
    spawn.spawn_time_secs_min = 3;
    spawn.spawn_time_secs_max = 3;
    spawn.template.corpse_decay = 1;
    let creature_guid = creature_spawn_guid(&spawn);
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.begin_corpse(now, 3_000);
    runtime.corpse_expires_at = Some(now);
    map.share_db_creature_snapshots(vec![runtime]);

    assert_eq!(
        map.loaded_db_creature_lifecycle_guids(now),
        vec![creature_guid.raw()]
    );
}

#[test]
fn map_runtime_db_creature_lifecycle_tick_processes_due_events_once() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, player_position);
    insert_map_runtime_player_for_test(&mut map, 2, observer_position);

    let mut spawn = test_creature_spawn(6);
    spawn.guid = 90;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    spawn.spawn_time_secs_min = 3;
    spawn.spawn_time_secs_max = 3;
    spawn.template.corpse_decay = 1;
    let creature_guid = creature_spawn_guid(&spawn);

    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    map.players
        .get_mut(&1)
        .unwrap()
        .visible_objects
        .insert(creature_guid);
    map.players
        .get_mut(&2)
        .unwrap()
        .visible_objects
        .insert(creature_guid);

    let killed_at = Instant::now();
    map.apply_db_creature_damage(DbCreatureDamageRequest {
        creature_guid,
        killer: ObjectGuid::new(HighGuid::Player, 0, 1),
        damage: 9_999,
        melee_outcome: None,
        spell_damage_outcome: None,
        spell_id: None,
        spell_school: 0,
        suppress_attacker_state: false,
        now: killed_at,
        now_epoch_secs: 3_000,
        exclude_character_guid: Some(1),
        corpse_loot: None,
    })
    .unwrap()
    .expect("death event");

    let corpse_tick = map
        .advance_db_creature_lifecycle_tick(killed_at + Duration::from_secs(1))
        .unwrap();
    assert_eq!(corpse_tick.respawn_updates.len(), 0);
    assert_eq!(corpse_tick.packets.len(), 2);
    assert!(corpse_tick
        .packets
        .iter()
        .all(|(_, packet)| packet.opcode == WorldOpcode::SmsgDestroyObject as u16));
    assert!(map
        .advance_db_creature_lifecycle_tick(killed_at + Duration::from_secs(1))
        .unwrap()
        .packets
        .is_empty());

    let respawn_tick = map
        .advance_db_creature_lifecycle_tick(killed_at + Duration::from_secs(3))
        .unwrap();
    assert_eq!(respawn_tick.respawn_updates.len(), 1);
    assert_eq!(respawn_tick.respawn_updates[0].creature_spawn_guid, 90);
    assert_eq!(respawn_tick.packets.len(), 2);
    assert!(respawn_tick
        .packets
        .iter()
        .all(|(_, packet)| packet.opcode == WorldOpcode::SmsgUpdateObject as u16));
    assert!(map
        .advance_db_creature_lifecycle_tick(killed_at + Duration::from_secs(3))
        .unwrap()
        .packets
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
    map.grids
        .entry(grid)
        .or_default()
        .cells
        .entry(cell)
        .or_default()
        .client_players
        .insert(guid);
    map.players.insert(
        guid,
        test_player_runtime(guid, SessionId(guid as u64), position),
    );
}

#[test]
fn db_creature_movement_scripts_execute_emote_and_morph_from_db_data() {
    let now = Instant::now();
    let mut registry = DbScriptRegistry::default();
    let mut emote = test_db_script_command(77, SCRIPT_COMMAND_EMOTE, 0);
    emote.datalong = 234;
    let mut morph = test_db_script_command(77, SCRIPT_COMMAND_MORPH_TO_ENTRY_OR_MODEL, 0);
    morph.datalong = 89;
    morph.data_flags = SCRIPT_FLAG_COMMAND_ADDITIONAL;
    registry.movement_scripts.insert(77, vec![emote, morph]);

    let mut map =
        MapRuntime::with_geometry(0, 0, Arc::new(WorldGeometry::default()), Arc::new(registry));
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, position);
    let creature = DbCreatureRuntime::new(test_creature_spawn(11260));
    let creature_guid = creature.guid();
    map.players
        .get_mut(&1)
        .unwrap()
        .visible_objects
        .insert(creature_guid);
    map.creatures.insert(creature_guid.raw(), creature);

    map.schedule_db_creature_movement_script(creature_guid, 77, now);
    let packets = map.advance_pending_db_scripts(now).unwrap();
    let runtime = map.creatures.get(&creature_guid.raw()).unwrap();

    assert_eq!(packets.len(), 2);
    assert!(packets.iter().all(|(session, packet)| {
        *session == SessionId(1) && packet.opcode == WorldOpcode::SmsgUpdateObject as u16
    }));
    assert_eq!(runtime.spawn.addon_emote, 234);
    assert_eq!(runtime.display_id_override, Some(89));
}

#[test]
fn db_creature_movement_scripts_send_monster_talk_from_broadcast_text() {
    let now = Instant::now();
    let mut registry = DbScriptRegistry::default();
    let mut talk = test_db_script_command(88, SCRIPT_COMMAND_TALK, 0);
    talk.dataint = 12345;
    registry.movement_scripts.insert(88, vec![talk]);
    registry.broadcast_texts.insert(
        12345,
        wow_db::BroadcastTextQuery {
            id: 12345,
            text: Some("Back to work!".to_string()),
            text1: None,
            chat_type: CHAT_TYPE_SAY,
            language: LANG_UNIVERSAL,
            sound: 0,
            emote: 0,
        },
    );

    let mut map =
        MapRuntime::with_geometry(0, 0, Arc::new(WorldGeometry::default()), Arc::new(registry));
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, position);
    let creature = DbCreatureRuntime::new(test_creature_spawn(11260));
    let creature_guid = creature.guid();
    map.players
        .get_mut(&1)
        .unwrap()
        .visible_objects
        .insert(creature_guid);
    map.creatures.insert(creature_guid.raw(), creature);

    map.schedule_db_creature_movement_script(creature_guid, 88, now);
    let packets = map.advance_pending_db_scripts(now).unwrap();

    assert_eq!(packets.len(), 1);
    let (session, packet) = &packets[0];
    assert_eq!(*session, SessionId(1));
    assert_eq!(packet.opcode, WorldOpcode::SmsgMessageChat as u16);
    assert_eq!(packet.body[0], CHAT_MSG_MONSTER_SAY as u8);
    assert_eq!(&packet.body[1..5], &LANG_UNIVERSAL.to_le_bytes());
    assert!(packet
        .body
        .windows("Creature 11260".len())
        .any(|window| window == b"Creature 11260"));
    assert!(packet
        .body
        .windows("Back to work!".len())
        .any(|window| window == b"Back to work!"));
}

#[test]
fn db_creature_movement_script_delays_are_milliseconds() {
    let now = Instant::now();
    let mut registry = DbScriptRegistry::default();
    let mut emote = test_db_script_command(99, SCRIPT_COMMAND_EMOTE, 1000);
    emote.datalong = 234;
    registry.movement_scripts.insert(99, vec![emote]);

    let mut map =
        MapRuntime::with_geometry(0, 0, Arc::new(WorldGeometry::default()), Arc::new(registry));
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, position);
    let creature = DbCreatureRuntime::new(test_creature_spawn(11260));
    let creature_guid = creature.guid();
    map.players
        .get_mut(&1)
        .unwrap()
        .visible_objects
        .insert(creature_guid);
    map.creatures.insert(creature_guid.raw(), creature);

    map.schedule_db_creature_movement_script(creature_guid, 99, now);

    assert!(map
        .advance_pending_db_scripts(now + Duration::from_millis(999))
        .unwrap()
        .is_empty());
    assert_eq!(
        map.advance_pending_db_scripts(now + Duration::from_millis(1000))
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn db_creature_movement_scripts_preserve_due_time_then_priority_order() {
    let now = Instant::now();
    let mut registry = DbScriptRegistry::default();
    let mut late_priority = test_db_script_command(101, SCRIPT_COMMAND_TALK, 0);
    late_priority.priority = 5;
    late_priority.dataint = 5001;
    let mut early_priority = test_db_script_command(101, SCRIPT_COMMAND_TALK, 0);
    early_priority.priority = 1;
    early_priority.dataint = 5002;
    registry
        .movement_scripts
        .insert(101, vec![late_priority, early_priority]);
    registry.broadcast_texts.insert(
        5001,
        wow_db::BroadcastTextQuery {
            id: 5001,
            text: Some("Later".to_string()),
            text1: None,
            chat_type: CHAT_TYPE_SAY,
            language: LANG_UNIVERSAL,
            sound: 0,
            emote: 0,
        },
    );
    registry.broadcast_texts.insert(
        5002,
        wow_db::BroadcastTextQuery {
            id: 5002,
            text: Some("Sooner".to_string()),
            text1: None,
            chat_type: CHAT_TYPE_SAY,
            language: LANG_UNIVERSAL,
            sound: 0,
            emote: 0,
        },
    );

    let mut map =
        MapRuntime::with_geometry(0, 0, Arc::new(WorldGeometry::default()), Arc::new(registry));
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, position);
    let creature = DbCreatureRuntime::new(test_creature_spawn(11260));
    let creature_guid = creature.guid();
    map.players
        .get_mut(&1)
        .unwrap()
        .visible_objects
        .insert(creature_guid);
    map.creatures.insert(creature_guid.raw(), creature);

    map.schedule_db_creature_movement_script(creature_guid, 101, now);
    let packets = map.advance_pending_db_scripts(now).unwrap();

    assert_eq!(packets.len(), 2);
    assert!(packets[0]
        .1
        .body
        .windows("Sooner".len())
        .any(|window| window == b"Sooner"));
    assert!(packets[1]
        .1
        .body
        .windows("Later".len())
        .any(|window| window == b"Later"));
}

#[test]
fn db_creature_movement_scripts_run_for_zero_distance_waypoint_nodes() {
    let now = Instant::now();
    let mut registry = DbScriptRegistry::default();
    let mut emote = test_db_script_command(100, SCRIPT_COMMAND_EMOTE, 0);
    emote.datalong = 234;
    registry.movement_scripts.insert(100, vec![emote]);

    let mut map =
        MapRuntime::with_geometry(0, 0, Arc::new(WorldGeometry::default()), Arc::new(registry));
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 1.25);
    insert_map_runtime_player_for_test(&mut map, 1, position);
    let mut spawn = test_creature_spawn(11260);
    spawn.movement_type = DB_MOTION_TYPE_WAYPOINT;
    spawn.template.movement_type = DB_MOTION_TYPE_WAYPOINT;
    spawn.waypoint_path = vec![wow_db::CreatureWaypointQuery {
        point: 1,
        position_x: spawn.position_x,
        position_y: spawn.position_y,
        position_z: spawn.position_z,
        orientation: Some(spawn.orientation),
        wait_time: 0,
        script_id: 100,
    }];
    let mut creature = DbCreatureRuntime::new(spawn);
    creature.next_waypoint_move_at = Some(now);
    let creature_guid = creature.guid();
    map.players
        .get_mut(&1)
        .unwrap()
        .visible_objects
        .insert(creature_guid);
    map.creatures.insert(creature_guid.raw(), creature);

    let (snapshot, motion, script_ids) = map
        .start_db_creature_idle_motion(
            &DbCreatureNavigationGuardrail::default(),
            creature_guid,
            now,
        )
        .outcome
        .unwrap();
    for script_id in script_ids {
        map.schedule_db_creature_movement_script(snapshot.guid(), script_id, now);
    }
    let packets = map.advance_pending_db_scripts(now).unwrap();

    assert!(motion.is_none());
    assert_eq!(
        map.creatures
            .get(&creature_guid.raw())
            .unwrap()
            .spawn
            .addon_emote,
        234
    );
    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0].1.opcode, WorldOpcode::SmsgUpdateObject as u16);
}
