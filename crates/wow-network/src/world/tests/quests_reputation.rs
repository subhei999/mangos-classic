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
        jump: JumpInfo::default(),
    };
    let mut quest = test_quest_template(7);
    quest.min_level = 2;
    assert!(!satisfies_race_class_level(&quest, &character));

    quest.min_level = 1;
    quest.max_level = 1;
    quest.required_classes = 1 << (1 - 1);
    quest.required_races = 1 << (1 - 1);
    assert!(satisfies_race_class_level(&quest, &character));

    quest.max_level = 0;
    assert!(satisfies_race_class_level(&quest, &character));

    quest.max_level = 1;
    quest.required_classes = 1 << (2 - 1);
    assert!(!satisfies_race_class_level(&quest, &character));

    quest.required_classes = 1 << (1 - 1);
    quest.required_races = 1 << (2 - 1);
    assert!(!satisfies_race_class_level(&quest, &character));
}

#[test]
fn quest_marker_visibility_uses_cmangos_high_level_hide_diff() {
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
    let mut quest = test_quest_template(7);

    quest.min_level = character.level + QUEST_HIGH_LEVEL_HIDE_DIFF;
    assert!(satisfies_quest_visibility_level(&quest, &character));
    assert!(!satisfies_race_class_level(&quest, &character));

    quest.min_level = character.level + QUEST_HIGH_LEVEL_HIDE_DIFF + 1;
    assert!(!satisfies_quest_visibility_level(&quest, &character));

    quest.min_level = 1;
    quest.max_level = 1;
    assert!(satisfies_quest_visibility_level(&quest, &character));

    quest.max_level = 0;
    assert!(satisfies_quest_visibility_level(&quest, &character));
}

#[test]
fn quest_required_skill_uses_cmangos_value_check() {
    let mut quest = test_quest_template(7);
    assert!(satisfies_required_skill(&quest, &[]));

    quest.required_skill = 164;
    quest.required_skill_value = 75;

    assert!(!satisfies_required_skill(&quest, &[]));
    assert!(!satisfies_required_skill(
        &quest,
        &[test_skill(164, 74, 75)]
    ));
    assert!(satisfies_required_skill(&quest, &[test_skill(164, 75, 75)]));
    assert!(satisfies_required_skill(
        &quest,
        &[test_skill(164, 100, 150)]
    ));
    assert!(!satisfies_required_skill(
        &quest,
        &[test_skill(165, 100, 150)]
    ));

    quest.required_skill_value = 0;
    assert!(satisfies_required_skill(&quest, &[]));
}

#[test]
fn quest_required_reputation_uses_cmangos_min_max_thresholds() {
    let mut quest = test_quest_template(7);
    assert!(satisfies_required_reputation(&quest, &[]));

    quest.required_min_rep_faction = 72;
    quest.required_min_rep_value = 500;

    assert!(!satisfies_required_reputation(&quest, &[]));
    assert!(!satisfies_required_reputation(
        &quest,
        &[CharacterReputation {
            faction: 72,
            standing: 499,
            flags: 0,
        }]
    ));
    assert!(satisfies_required_reputation(
        &quest,
        &[CharacterReputation {
            faction: 72,
            standing: 500,
            flags: 0,
        }]
    ));

    quest.required_min_rep_faction = 0;
    quest.required_min_rep_value = 0;
    quest.required_max_rep_faction = 72;
    quest.required_max_rep_value = 500;

    assert!(satisfies_required_reputation(&quest, &[]));
    assert!(satisfies_required_reputation(
        &quest,
        &[CharacterReputation {
            faction: 72,
            standing: 499,
            flags: 0,
        }]
    ));
    assert!(!satisfies_required_reputation(
        &quest,
        &[CharacterReputation {
            faction: 72,
            standing: 500,
            flags: 0,
        }]
    ));
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
fn quest_accept_requires_free_quest_log_slot() {
    let mut session = WorldSessionState::default();
    for quest in 1..=MAX_QUEST_LOG_SIZE as u32 {
        session.quests.quest_statuses.insert(
            quest,
            CharacterQuestStatus {
                quest,
                status: QUEST_STATUS_INCOMPLETE,
                rewarded: 0,
                mobcount1: 0,
                mobcount2: 0,
                mobcount3: 0,
                mobcount4: 0,
            },
        );
    }
    session.quests.quest_log_slots = quest_log_slots_from_statuses(&session.quests.quest_statuses);

    assert!(!quest_log_has_free_slot(&session));
    assert_eq!(quest_log_slot_for_quest(&session, 1), Some(0));
    assert_eq!(
        quest_log_slot_for_quest(&session, MAX_QUEST_LOG_SIZE as u32),
        Some(MAX_QUEST_LOG_SIZE - 1)
    );
    assert_eq!(
        quest_log_slot_for_quest(&session, MAX_QUEST_LOG_SIZE as u32 + 1),
        None
    );
}

#[test]
fn rewarded_historical_quests_do_not_consume_quest_log_slots() {
    let mut session = WorldSessionState::default();
    for quest in 1..=MAX_QUEST_LOG_SIZE as u32 {
        session.quests.quest_statuses.insert(
            quest,
            CharacterQuestStatus {
                quest,
                status: QUEST_STATUS_COMPLETE,
                rewarded: 1,
                mobcount1: 0,
                mobcount2: 0,
                mobcount3: 0,
                mobcount4: 0,
            },
        );
    }

    assert!(quest_log_has_free_slot(&session));
    assert_eq!(quest_log_slot_for_quest(&session, 1), None);
}

#[test]
fn quest_log_slots_append_new_quests_without_sorted_shift() {
    let mut session = WorldSessionState::default();

    assert_eq!(assign_quest_log_slot(&mut session, 5261), Some(0));
    assert_eq!(assign_quest_log_slot(&mut session, 7), Some(1));
    assert_eq!(quest_log_slot_for_quest(&session, 5261), Some(0));
    assert_eq!(quest_log_slot_for_quest(&session, 7), Some(1));

    assert_eq!(clear_quest_log_slot(&mut session, 5261), Some(0));
    assert_eq!(assign_quest_log_slot(&mut session, 18), Some(0));
    assert_eq!(quest_log_slot_for_quest(&session, 18), Some(0));
    assert_eq!(quest_log_slot_for_quest(&session, 7), Some(1));
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
async fn prev_quest_requirements_follow_cmangos_any_satisfied_rule() {
    let object_mgr = ObjectMgr::default();
    let pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world")
        .expect("lazy mysql pool should not connect");
    for quest_id in [99, 100, 101] {
        object_mgr
            .prime_quest_template_for_test(quest_id, Some(test_quest_template(quest_id)))
            .await;
    }
    let quest = test_quest_template(18);
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

    assert!(
        satisfies_prev_quest_requirements(&object_mgr, &pool, &quest, &statuses, &[99, 100])
            .await
            .unwrap()
    );
    assert!(
        !satisfies_prev_quest_requirements(&object_mgr, &pool, &quest, &statuses, &[100, -99])
            .await
            .unwrap()
    );

    statuses.insert(
        100,
        CharacterQuestStatus {
            quest: 100,
            status: QUEST_STATUS_INCOMPLETE,
            rewarded: 0,
            mobcount1: 0,
            mobcount2: 0,
            mobcount3: 0,
            mobcount4: 0,
        },
    );

    assert!(
        satisfies_prev_quest_requirements(&object_mgr, &pool, &quest, &statuses, &[101, -100])
            .await
            .unwrap()
    );
    assert!(
        satisfies_prev_quest_requirements(&object_mgr, &pool, &quest, &statuses, &[])
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn prev_quest_requirements_require_all_rewarded_in_negative_exclusive_group() {
    let object_mgr = ObjectMgr::default();
    let pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world")
        .expect("lazy mysql pool should not connect");
    let mut quest = test_quest_template(18);
    quest.prev_quest_id = 10;
    let mut first_prev = test_quest_template(10);
    first_prev.exclusive_group = -7;
    first_prev.next_quest_id = 10;
    let mut second_prev = test_quest_template(11);
    second_prev.exclusive_group = -7;
    object_mgr
        .prime_quest_template_for_test(10, Some(first_prev))
        .await;
    object_mgr
        .prime_quest_template_for_test(11, Some(second_prev))
        .await;
    object_mgr
        .prime_exclusive_group_quests_for_test(-7, vec![10, 11])
        .await;

    let mut statuses = HashMap::new();
    statuses.insert(
        10,
        CharacterQuestStatus {
            quest: 10,
            status: QUEST_STATUS_COMPLETE,
            rewarded: 1,
            mobcount1: 0,
            mobcount2: 0,
            mobcount3: 0,
            mobcount4: 0,
        },
    );

    assert!(
        !satisfies_prev_quest_requirements(&object_mgr, &pool, &quest, &statuses, &[10])
            .await
            .unwrap()
    );

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

    assert!(
        satisfies_prev_quest_requirements(&object_mgr, &pool, &quest, &statuses, &[10])
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn prev_quest_requirements_require_all_active_in_negative_active_exclusive_group() {
    let object_mgr = ObjectMgr::default();
    let pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world")
        .expect("lazy mysql pool should not connect");
    let mut quest = test_quest_template(18);
    quest.prev_quest_id = -10;
    let mut first_prev = test_quest_template(10);
    first_prev.exclusive_group = -7;
    first_prev.next_quest_id = 10;
    let mut second_prev = test_quest_template(11);
    second_prev.exclusive_group = -7;
    object_mgr
        .prime_quest_template_for_test(10, Some(first_prev))
        .await;
    object_mgr
        .prime_quest_template_for_test(11, Some(second_prev))
        .await;
    object_mgr
        .prime_exclusive_group_quests_for_test(-7, vec![10, 11])
        .await;

    let mut statuses = HashMap::new();
    statuses.insert(
        10,
        CharacterQuestStatus {
            quest: 10,
            status: QUEST_STATUS_INCOMPLETE,
            rewarded: 0,
            mobcount1: 0,
            mobcount2: 0,
            mobcount3: 0,
            mobcount4: 0,
        },
    );

    assert!(
        !satisfies_prev_quest_requirements(&object_mgr, &pool, &quest, &statuses, &[-10])
            .await
            .unwrap()
    );

    statuses.insert(
        11,
        CharacterQuestStatus {
            quest: 11,
            status: QUEST_STATUS_COMPLETE,
            rewarded: 0,
            mobcount1: 0,
            mobcount2: 0,
            mobcount3: 0,
            mobcount4: 0,
        },
    );

    assert!(
        satisfies_prev_quest_requirements(&object_mgr, &pool, &quest, &statuses, &[-10])
            .await
            .unwrap()
    );
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
    session.quests.quest_statuses.insert(
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
async fn questgiver_list_hides_visible_but_untakeable_start_quests() {
    let object_mgr = ObjectMgr::default();
    let pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world")
        .expect("lazy mysql pool should not connect");
    let mut available = test_quest_template(7);
    available.min_level = 1;
    available.exclusive_group = 0;
    let mut unavailable = test_quest_template(18);
    unavailable.min_level = 2;
    unavailable.exclusive_group = 0;
    object_mgr
        .prime_creature_start_quest_ids_for_test(197, vec![7, 18])
        .await;
    object_mgr
        .prime_creature_complete_quest_ids_for_test(197, Vec::new())
        .await;
    object_mgr
        .prime_quest_template_for_test(7, Some(available))
        .await;
    object_mgr
        .prime_quest_template_for_test(18, Some(unavailable))
        .await;
    for quest in [7, 18] {
        object_mgr
            .prime_quest_prev_quests_for_test(quest, Vec::new())
            .await;
        object_mgr
            .prime_quest_prev_chain_quests_for_test(quest, Vec::new())
            .await;
    }
    object_mgr
        .prime_exclusive_group_quests_for_test(0, Vec::new())
        .await;
    let session = WorldSessionState {
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

    let quests = questgiver_visible_quests(
        &object_mgr,
        &pool,
        ObjectGuid::new(HighGuid::Unit, 197, 1),
        &session,
    )
    .await
    .unwrap();

    assert_eq!(quests.len(), 1);
    assert_eq!(quests[0].quest.entry, 7);
    assert_eq!(quests[0].dialog_status, DIALOG_STATUS_AVAILABLE);
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
    assert_eq!(packet.opcode, WorldOpcode::SmsgQuestgiverStatus as u16);
    assert_eq!(
        packet.body,
        build_questgiver_status_body(giver, DIALOG_STATUS_UNAVAILABLE)
    );
    assert!(outbound_rx.try_recv().is_err());
}

#[test]
fn world_packet_sink_rejects_full_bounded_queue() {
    let (outbound_tx, _outbound_rx) = mpsc::channel(1);
    let mut sink = WorldPacketSink::new(outbound_tx);

    sink.send(test_smsg_pong_opcode(), &[1, 2, 3]).unwrap();
    let error = sink.send(test_smsg_pong_opcode(), &[4, 5, 6]).unwrap_err();

    assert!(error.to_string().contains("outbound queue full"));
}

#[tokio::test]
async fn questgiver_cancel_closes_gossip_like_cmangos() {
    let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel();
    let mut sink = WorldPacketSink::new(outbound_tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);

    handle_questgiver_cancel(&mut sink, &mut header_crypto)
        .await
        .unwrap();

    let packet = outbound_rx.try_recv().unwrap();
    assert_eq!(packet.opcode, WorldOpcode::SmsgGossipComplete as u16);
    assert!(packet.body.is_empty());
    assert!(outbound_rx.try_recv().is_err());
}

#[tokio::test]
async fn session_registry_requests_disconnect_when_bounded_queue_is_full() {
    let sessions = SessionRegistry::default();
    let session_id = SessionId(42);
    let (outbound_tx, _outbound_rx) = mpsc::channel(1);
    let (disconnect_tx, mut disconnect_rx) = mpsc::channel(1);
    sessions
        .register(
            session_id,
            SessionHandle {
                account_id: 1,
                character_guid: None,
                character_name: None,
                outbound: WorldPacketSender::Bounded(outbound_tx),
                disconnect: Some(disconnect_tx),
            },
        )
        .await;

    sessions
        .send_packet(
            session_id,
            OutboundWorldPacket {
                opcode: test_smsg_pong_opcode(),
                body: vec![1],
            },
        )
        .await;
    sessions
        .send_packet(
            session_id,
            OutboundWorldPacket {
                opcode: test_smsg_pong_opcode(),
                body: vec![2],
            },
        )
        .await;

    assert_eq!(
        disconnect_rx.try_recv().unwrap().reason,
        WorldSessionDisconnectReason::OutboundQueueFull
    );
}

#[tokio::test]
async fn quest_state_refresh_updates_visible_gameobject_dynamic_flags() {
    let object_mgr = ObjectMgr::default();
    let pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world")
        .expect("lazy mysql pool should not connect");
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut spawn = test_gameobject_spawn(161557, GO_TYPE_GOOBER);
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    spawn.template.flags = GO_FLAG_INTERACT_COND;
    spawn.template.raw_data[1] = 18;
    let guid = gameobject_spawn_guid(&spawn);
    let grid = grid_coord_for_position(gameobject_spawn_position(&spawn));
    let maps = Arc::new(MapRuntimeManager::default());
    maps.add_player(test_player_runtime(7, SessionId(7), player_position))
        .await
        .unwrap();
    maps.ensure_db_gameobject_grids_loaded_for_test(
        0,
        player_position,
        CREATURE_SPAWN_RADIUS_YARDS,
        |candidate| {
            if candidate == grid {
                vec![DbGameObjectRuntime::new(spawn.clone())]
            } else {
                Vec::new()
            }
        },
    )
    .await;
    let nearby = maps
        .nearby_db_gameobject_snapshots(
            0,
            player_position,
            CREATURE_SPAWN_RADIUS_YARDS,
            CREATURE_SPAWN_LIMIT,
        )
        .await;
    maps.stage_player_db_gameobject_visibility(0, 7, player_position, nearby, Instant::now())
        .await;

    let sessions = Arc::new(SessionRegistry::default());
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
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    session.quests.quest_statuses.insert(
        18,
        CharacterQuestStatus {
            quest: 18,
            status: QUEST_STATUS_INCOMPLETE,
            rewarded: 0,
            mobcount1: 0,
            mobcount2: 0,
            mobcount3: 0,
            mobcount4: 0,
        },
    );
    let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel();
    let mut sink = WorldPacketSink::new(outbound_tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);

    send_visible_questgiver_status_updates(
        &mut sink,
        &object_mgr,
        &pool,
        shared_world,
        &session,
        &[],
        &mut header_crypto,
    )
    .await
    .unwrap();

    let packet = outbound_rx.try_recv().unwrap();
    assert_eq!(packet.opcode, WorldOpcode::SmsgUpdateObject as u16);
    assert_eq!(&packet.body[0..4], &1u32.to_le_bytes());
    assert_eq!(packet.body[4], 0);
    let (values, trailing) = decode_values_update_block(&packet.body[5..], guid);
    assert!(trailing.is_empty());
    assert_eq!(values[GAMEOBJECT_DYN_FLAGS], Some(GO_DYNFLAG_LO_ACTIVATE));
    assert!(outbound_rx.try_recv().is_err());
}

#[tokio::test]
async fn quest_dialog_status_allows_any_satisfied_previous_quest() {
    let object_mgr = ObjectMgr::default();
    let pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world")
        .expect("lazy mysql pool should not connect");
    let quest = test_quest_template(18);
    object_mgr
        .prime_quest_prev_quests_for_test(18, vec![10, 11])
        .await;
    object_mgr
        .prime_quest_template_for_test(10, Some(test_quest_template(10)))
        .await;
    object_mgr
        .prime_quest_template_for_test(11, Some(test_quest_template(11)))
        .await;
    object_mgr
        .prime_quest_prev_chain_quests_for_test(18, Vec::new())
        .await;
    object_mgr
        .prime_exclusive_group_quests_for_test(0, Vec::new())
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
    session.quests.quest_statuses.insert(
        10,
        CharacterQuestStatus {
            quest: 10,
            status: QUEST_STATUS_COMPLETE,
            rewarded: 1,
            mobcount1: 0,
            mobcount2: 0,
            mobcount3: 0,
            mobcount4: 0,
        },
    );

    assert_eq!(
        quest_start_dialog_status(&object_mgr, &pool, &quest, &session)
            .await
            .unwrap(),
        Some(DIALOG_STATUS_AVAILABLE)
    );
}

#[tokio::test]
async fn quest_dialog_status_requires_db_required_skill() {
    let object_mgr = ObjectMgr::default();
    let pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world")
        .expect("lazy mysql pool should not connect");
    let mut quest = test_quest_template(18);
    quest.required_skill = 164;
    quest.required_skill_value = 75;
    object_mgr
        .prime_quest_prev_quests_for_test(18, Vec::new())
        .await;
    object_mgr
        .prime_quest_prev_chain_quests_for_test(18, Vec::new())
        .await;
    object_mgr
        .prime_exclusive_group_quests_for_test(0, Vec::new())
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

    assert_eq!(
        quest_start_dialog_status(&object_mgr, &pool, &quest, &session)
            .await
            .unwrap(),
        None
    );

    session
        .character
        .character_skills
        .push(test_skill(164, 75, 75));

    assert_eq!(
        quest_start_dialog_status(&object_mgr, &pool, &quest, &session)
            .await
            .unwrap(),
        Some(DIALOG_STATUS_AVAILABLE)
    );
}

#[tokio::test]
async fn quest_dialog_status_hides_unwired_required_condition() {
    let object_mgr = ObjectMgr::default();
    let pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world")
        .expect("lazy mysql pool should not connect");
    let mut quest = test_quest_template(18);
    quest.required_condition = 42;
    object_mgr.prime_condition_for_test(42, None).await;
    object_mgr
        .prime_quest_prev_quests_for_test(18, Vec::new())
        .await;
    object_mgr
        .prime_quest_prev_chain_quests_for_test(18, Vec::new())
        .await;
    object_mgr
        .prime_exclusive_group_quests_for_test(0, Vec::new())
        .await;
    let session = WorldSessionState {
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

    assert_eq!(
        quest_start_dialog_status(&object_mgr, &pool, &quest, &session)
            .await
            .unwrap(),
        None
    );

    quest.required_condition = 0;

    assert_eq!(
        quest_start_dialog_status(&object_mgr, &pool, &quest, &session)
            .await
            .unwrap(),
        Some(DIALOG_STATUS_AVAILABLE)
    );
}

#[tokio::test]
async fn quest_required_condition_uses_cmangos_team_condition() {
    let object_mgr = ObjectMgr::default();
    let pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world")
        .expect("lazy mysql pool should not connect");
    let mut quest = test_quest_template(18);
    quest.required_condition = 42;
    object_mgr
        .prime_condition_for_test(
            42,
            Some(test_condition(42, CONDITION_TEAM, ALLIANCE_FACTION, 0)),
        )
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

    assert_eq!(
        quest_start_dialog_status(&object_mgr, &pool, &quest, &session)
            .await
            .unwrap(),
        Some(DIALOG_STATUS_AVAILABLE)
    );

    session.character.active_character.as_mut().unwrap().race = 2;
    assert_eq!(
        quest_start_dialog_status(&object_mgr, &pool, &quest, &session)
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn quest_required_condition_uses_active_game_event() {
    let object_mgr = ObjectMgr::default();
    let pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world")
        .expect("lazy mysql pool should not connect");
    let now = current_unix_epoch_secs() as i64;
    object_mgr
        .prime_game_event_schedules_for_test(vec![wow_db::GameEventScheduleQuery {
            entry: 10,
            schedule_type: 1,
            occurrence: 1_440,
            length: 120,
            holiday: 0,
            linked_to: 0,
            description: Some("active test event".to_string()),
            start_time_unix: Some(now - 60),
            end_time_unix: Some(now + 86_400),
        }])
        .await;
    let mut quest = test_quest_template(18);
    quest.required_condition = 42;
    object_mgr
        .prime_condition_for_test(
            42,
            Some(test_condition(42, CONDITION_ACTIVE_GAME_EVENT, 10, 0)),
        )
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
    let session = WorldSessionState {
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

    assert_eq!(
        quest_start_dialog_status(&object_mgr, &pool, &quest, &session)
            .await
            .unwrap(),
        Some(DIALOG_STATUS_AVAILABLE)
    );

    object_mgr
        .prime_condition_for_test(
            42,
            Some(test_condition(42, CONDITION_ACTIVE_GAME_EVENT, 11, 0)),
        )
        .await;
    assert_eq!(
        quest_start_dialog_status(&object_mgr, &pool, &quest, &session)
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn quest_required_condition_uses_quest_taken_rewarded_and_boolean_conditions() {
    let object_mgr = ObjectMgr::default();
    let pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world")
        .expect("lazy mysql pool should not connect");
    let mut quest = test_quest_template(18);
    quest.required_condition = 42;
    let mut condition = test_condition(42, CONDITION_AND, 43, 44);
    condition.value3 = 45;
    object_mgr
        .prime_condition_for_test(42, Some(condition))
        .await;
    object_mgr
        .prime_condition_for_test(43, Some(test_condition(43, CONDITION_QUEST_TAKEN, 7, 1)))
        .await;
    object_mgr
        .prime_condition_for_test(44, Some(test_condition(44, CONDITION_QUEST_REWARDED, 8, 0)))
        .await;
    object_mgr
        .prime_condition_for_test(45, Some(test_condition(45, CONDITION_NOT, 46, 0)))
        .await;
    object_mgr
        .prime_condition_for_test(46, Some(test_condition(46, CONDITION_QUEST_NONE, 7, 0)))
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
    session.quests.quest_statuses.insert(
        7,
        CharacterQuestStatus {
            quest: 7,
            status: QUEST_STATUS_INCOMPLETE,
            rewarded: 0,
            mobcount1: 0,
            mobcount2: 0,
            mobcount3: 0,
            mobcount4: 0,
        },
    );
    session.quests.quest_statuses.insert(
        8,
        CharacterQuestStatus {
            quest: 8,
            status: QUEST_STATUS_COMPLETE,
            rewarded: 1,
            mobcount1: 0,
            mobcount2: 0,
            mobcount3: 0,
            mobcount4: 0,
        },
    );

    assert_eq!(
        quest_start_dialog_status(&object_mgr, &pool, &quest, &session)
            .await
            .unwrap(),
        Some(DIALOG_STATUS_AVAILABLE)
    );

    session.quests.quest_statuses.remove(&8);
    assert_eq!(
        quest_start_dialog_status(&object_mgr, &pool, &quest, &session)
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn quest_dialog_status_requires_db_reputation_bounds() {
    let object_mgr = ObjectMgr::default();
    let pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world")
        .expect("lazy mysql pool should not connect");
    let mut quest = test_quest_template(18);
    quest.required_min_rep_faction = 72;
    quest.required_min_rep_value = 500;
    object_mgr
        .prime_quest_prev_quests_for_test(18, Vec::new())
        .await;
    object_mgr
        .prime_quest_prev_chain_quests_for_test(18, Vec::new())
        .await;
    object_mgr
        .prime_exclusive_group_quests_for_test(0, Vec::new())
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

    assert_eq!(
        quest_start_dialog_status(&object_mgr, &pool, &quest, &session)
            .await
            .unwrap(),
        None
    );

    session
        .character
        .character_reputations
        .push(CharacterReputation {
            faction: 72,
            standing: 500,
            flags: 0,
        });

    assert_eq!(
        quest_start_dialog_status(&object_mgr, &pool, &quest, &session)
            .await
            .unwrap(),
        Some(DIALOG_STATUS_AVAILABLE)
    );

    quest.required_min_rep_faction = 0;
    quest.required_min_rep_value = 0;
    quest.required_max_rep_faction = 72;
    quest.required_max_rep_value = 500;

    assert_eq!(
        quest_start_dialog_status(&object_mgr, &pool, &quest, &session)
            .await
            .unwrap(),
        None
    );
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
fn quest_request_items_packet_includes_required_items_and_complete_flags() {
    let guid = ObjectGuid::new(HighGuid::Unit, 197, 1);
    let mut quest = test_quest_template(33);
    quest.title = "Wolves Across the Border".to_string();
    quest.request_items_text = "Have you collected the meat?".to_string();
    quest.req_item_id[0] = 769;
    quest.req_item_count[0] = 8;
    quest.rew_or_req_money = -25;
    let mut displays = QuestRewardItemDisplays::default();
    displays.required[0] = 6689;

    let body = build_quest_request_items_body(guid, &quest, &displays, true, false);
    let mut cursor = 8;
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 33);
    while body[cursor] != 0 {
        cursor += 1;
    }
    cursor += 1;
    while body[cursor] != 0 {
        cursor += 1;
    }
    cursor += 1;
    assert_eq!(
        read_u32(&body, &mut cursor).unwrap(),
        quest.complete_emote_delay
    );
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), quest.complete_emote);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 25);

    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 1);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 769);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 8);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 6689);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 2);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 3);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 4);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 8);
    assert_eq!(cursor, body.len());
}

#[test]
fn quest_request_items_can_close_on_cancel_for_auto_opened_turnins() {
    let guid = ObjectGuid::new(HighGuid::Unit, 197, 1);
    let mut quest = test_quest_template(33);
    quest.request_items_text = "Have you collected the meat?".to_string();
    quest.req_item_id[0] = 769;
    quest.req_item_count[0] = 8;
    let body = build_quest_request_items_body(
        guid,
        &quest,
        &QuestRewardItemDisplays::default(),
        true,
        true,
    );
    let mut cursor = 8 + 4;
    while body[cursor] != 0 {
        cursor += 1;
    }
    cursor += 1;
    while body[cursor] != 0 {
        cursor += 1;
    }
    cursor += 1;
    cursor += 8;

    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 1);
}

#[test]
fn completable_item_turnins_use_request_items_before_offer_reward() {
    let mut quest = test_quest_template(33);
    quest.request_items_text = "Have you collected the meat?".to_string();
    quest.req_item_id[0] = 769;
    quest.req_item_count[0] = 8;

    assert!(!quest_request_items_skips_to_offer_reward(&quest, true));
    assert!(!quest_request_items_skips_to_offer_reward(&quest, false));

    quest.req_item_id = [0; 4];
    quest.req_item_count = [0; 4];
    assert!(quest_request_items_skips_to_offer_reward(&quest, true));

    quest.request_items_text.clear();
    assert!(quest_request_items_skips_to_offer_reward(&quest, false));
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
fn quest_reputation_reward_uses_cmangos_low_level_quest_formula() {
    let mut quest = test_quest_template(783);
    quest.quest_level = 1;
    quest.rew_rep_faction[0] = 72;
    quest.rew_rep_value[0] = 250;

    assert_eq!(quest_reputation_rewards(1, &quest), vec![(72, 250)]);
    assert_eq!(quest_reputation_rewards(7, &quest), vec![(72, 200)]);
    assert_eq!(quest_reputation_rewards(10, &quest), vec![(72, 50)]);
}

#[test]
fn faction_standing_packet_uses_dbc_reputation_list_slots() {
    let body = build_set_faction_standing_body(&[
        CharacterReputation {
            faction: 72,
            standing: 250,
            flags: 1,
        },
        CharacterReputation {
            faction: 999_999,
            standing: 42,
            flags: 1,
        },
    ]);
    let mut cursor = 0;

    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 1);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 19);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 250);
    assert_eq!(cursor, body.len());
}

#[test]
fn reputation_gain_message_uses_dbc_faction_name_bridge() {
    let change = CharacterReputationChange {
        reputation: CharacterReputation {
            faction: 72,
            standing: 250,
            flags: 1,
        },
        delta: 75,
    };

    assert_eq!(
        reputation_gain_system_message(&change).as_deref(),
        Some("Reputation with Stormwind increased by 75.")
    );
}

#[test]
fn system_chat_packet_uses_vanilla_message_shape() {
    let message = "Reputation with Stormwind increased by 75.";
    let body = build_system_message_chat_body(message);
    let mut cursor = 0;

    assert_eq!(body[cursor], CHAT_MSG_SYSTEM as u8);
    cursor += 1;
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), LANG_UNIVERSAL);
    assert_eq!(&body[cursor..cursor + 8], &0u64.to_le_bytes());
    cursor += 8;
    assert_eq!(
        read_u32(&body, &mut cursor).unwrap(),
        (message.len() + 1) as u32
    );
    assert_eq!(&body[cursor..cursor + message.len()], message.as_bytes());
    cursor += message.len();
    assert_eq!(body[cursor], 0);
    cursor += 1;
    assert_eq!(body[cursor], CHAT_TAG_NONE);
    cursor += 1;
    assert_eq!(cursor, body.len());
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
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    }];

    assert!(quest_can_complete_from_inventory(&quest, &inventory));

    let empty_inventory = [];
    assert!(!quest_can_complete_from_inventory(&quest, &empty_inventory));
}

#[test]
fn quest_log_count_state_packs_all_objective_counters() {
    let incomplete = CharacterQuestStatus {
        quest: 456,
        status: QUEST_STATUS_INCOMPLETE,
        rewarded: 0,
        mobcount1: 2,
        mobcount2: 3,
        mobcount3: 4,
        mobcount4: 5,
    };

    assert_eq!(
        quest_log_count_state(&incomplete),
        2 | (3 << 6) | (4 << 12) | (5 << 18)
    );

    let complete = CharacterQuestStatus {
        status: QUEST_STATUS_COMPLETE,
        ..incomplete
    };

    assert_eq!(
        quest_log_count_state(&complete),
        (2 | (3 << 6) | (4 << 12) | (5 << 18)) | (QUEST_STATE_COMPLETE << 24)
    );
}

#[test]
fn quest_log_refresh_rewrites_shifted_slots_after_sorted_insert() {
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let mut statuses = HashMap::new();
    statuses.insert(
        5261,
        CharacterQuestStatus {
            quest: 5261,
            status: QUEST_STATUS_INCOMPLETE,
            rewarded: 0,
            mobcount1: 0,
            mobcount2: 0,
            mobcount3: 0,
            mobcount4: 0,
        },
    );
    statuses.insert(
        7,
        CharacterQuestStatus {
            quest: 7,
            status: QUEST_STATUS_INCOMPLETE,
            rewarded: 0,
            mobcount1: 0,
            mobcount2: 0,
            mobcount3: 0,
            mobcount4: 0,
        },
    );

    let slots = quest_log_slots_from_statuses(&statuses);
    let body = build_player_quest_log_refresh_body(7, &statuses, &slots).unwrap();
    assert_eq!(&body[0..4], &1u32.to_le_bytes());
    let (values, trailing) = decode_values_update_block(&body[5..], player);

    assert!(trailing.is_empty());
    assert_eq!(
        values[PLAYER_QUEST_LOG_1_1 + QUEST_LOG_QUEST_ID_OFFSET],
        Some(7)
    );
    assert_eq!(
        values[PLAYER_QUEST_LOG_1_1 + MAX_QUEST_OFFSET + QUEST_LOG_QUEST_ID_OFFSET],
        Some(5261)
    );
}

#[test]
fn completed_delivery_quest_requires_items_to_reward() {
    let quest = quest_template_with_required_item(3100, 9542, 2);
    let status = CharacterQuestStatus {
        quest: 3100,
        status: QUEST_STATUS_COMPLETE,
        rewarded: 0,
        mobcount1: 0,
        mobcount2: 0,
        mobcount3: 0,
        mobcount4: 0,
    };
    let one_item = [CharacterInventoryItem {
        bag: INVENTORY_SLOT_BAG_0 as u32,
        slot: INVENTORY_SLOT_ITEM_START,
        item: 77,
        item_template: 9542,
        count: 1,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    }];
    let enough_items = [CharacterInventoryItem {
        count: 2,
        ..one_item[0].clone()
    }];

    assert!(!quest_status_can_reward_from_inventory(
        &status, &quest, &one_item
    ));
    assert!(quest_status_can_reward_from_inventory(
        &status,
        &quest,
        &enough_items
    ));
}

#[test]
fn quest_completion_requires_every_objective() {
    let mut quest = test_quest_template(3104);
    quest.req_creature_or_go_id = [6, 38, 0, 0];
    quest.req_creature_or_go_count = [1, 2, 0, 0];
    let partial = CharacterQuestStatus {
        quest: 3104,
        status: QUEST_STATUS_INCOMPLETE,
        rewarded: 0,
        mobcount1: 1,
        mobcount2: 1,
        mobcount3: 0,
        mobcount4: 0,
    };
    let complete = CharacterQuestStatus {
        mobcount2: 2,
        ..partial.clone()
    };

    assert!(!quest_status_can_complete(&partial, &quest, &[]));
    assert!(quest_status_can_complete(&complete, &quest, &[]));
}

#[test]
fn quest_source_item_storage_rejects_full_backpack_without_stack_room() {
    let mut quest = test_quest_template(3101);
    quest.src_item_id = 9542;
    quest.src_item_count = 1;
    let inventory: Vec<_> = (INVENTORY_SLOT_ITEM_START..INVENTORY_SLOT_ITEM_END)
        .enumerate()
        .map(|(index, slot)| CharacterInventoryItem {
            bag: INVENTORY_SLOT_BAG_0 as u32,
            slot,
            item: 1000 + index as u32,
            item_template: 8000 + index as u32,
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        })
        .collect();

    assert_eq!(
        plan_quest_source_item_storage(
            &quest,
            &inventory,
            QuestSourceItemTemplate {
                max_durability: 0,
                max_stack: 1,
                container_slots: None,
            },
        ),
        QuestSourceItemStorage::NoSpace
    );
}

#[test]
fn quest_source_item_storage_uses_existing_stack_when_backpack_is_full() {
    let mut quest = test_quest_template(3102);
    quest.src_item_id = 9542;
    quest.src_item_count = 3;
    let inventory: Vec<_> = (INVENTORY_SLOT_ITEM_START..INVENTORY_SLOT_ITEM_END)
        .enumerate()
        .map(|(index, slot)| CharacterInventoryItem {
            bag: INVENTORY_SLOT_BAG_0 as u32,
            slot,
            item: 1000 + index as u32,
            item_template: if index == 0 {
                9542
            } else {
                8000 + index as u32
            },
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        })
        .collect();

    assert_eq!(
        plan_quest_source_item_storage(
            &quest,
            &inventory,
            QuestSourceItemTemplate {
                max_durability: 0,
                max_stack: 5,
                container_slots: None,
            },
        ),
        QuestSourceItemStorage::Grant(QuestSourceItemStoragePlan {
            item_id: 9542,
            max_durability: 0,
            container_slots: None,
            destinations: vec![QuestSourceItemDestination::ExistingStack {
                item_guid: 1000,
                new_count: 3,
                grant_count: 2,
            }],
        })
    );
}

#[test]
fn quest_source_item_storage_splits_large_grants_across_empty_slots() {
    let mut quest = test_quest_template(3103);
    quest.src_item_id = 9542;
    quest.src_item_count = 5;
    let inventory = [CharacterInventoryItem {
        bag: INVENTORY_SLOT_BAG_0 as u32,
        slot: INVENTORY_SLOT_ITEM_START,
        item: 77,
        item_template: 9542,
        count: 1,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    }];

    assert_eq!(
        plan_quest_source_item_storage(
            &quest,
            &inventory,
            QuestSourceItemTemplate {
                max_durability: 0,
                max_stack: 2,
                container_slots: None,
            },
        ),
        QuestSourceItemStorage::Grant(QuestSourceItemStoragePlan {
            item_id: 9542,
            max_durability: 0,
            container_slots: None,
            destinations: vec![
                QuestSourceItemDestination::ExistingStack {
                    item_guid: 77,
                    new_count: 2,
                    grant_count: 1,
                },
                QuestSourceItemDestination::NewStack {
                    slot: INVENTORY_SLOT_ITEM_START + 1,
                    count: 2,
                },
                QuestSourceItemDestination::NewStack {
                    slot: INVENTORY_SLOT_ITEM_START + 2,
                    count: 1,
                },
            ],
        })
    );
}

#[test]
fn quest_source_item_push_result_matches_cmangos_shape() {
    let item = CharacterInventoryItem {
        bag: INVENTORY_SLOT_BAG_0 as u32,
        slot: INVENTORY_SLOT_ITEM_START,
        item: 77,
        item_template: 9542,
        count: 1,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    };
    let body = build_item_push_result_body(11, &item, 1, true, false, true);
    let mut cursor = 0;

    assert_eq!(
        &body[cursor..cursor + 8],
        &ObjectGuid::new(HighGuid::Player, 0, 11).raw().to_le_bytes()
    );
    cursor += 8;
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 1);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 1);
    assert_eq!(body[cursor], CLIENT_INVENTORY_SLOT_BAG_0);
    cursor += 1;
    assert_eq!(
        read_u32(&body, &mut cursor).unwrap(),
        INVENTORY_SLOT_ITEM_START as u32
    );
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 9542);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 1);
    assert_eq!(cursor, body.len());
}

#[test]
fn gm_additem_push_result_matches_cmangos_self_grant_flags() {
    let item = CharacterInventoryItem {
        bag: INVENTORY_SLOT_BAG_0 as u32,
        slot: INVENTORY_SLOT_ITEM_START,
        item: 77,
        item_template: 929,
        count: 1,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    };
    let body = build_item_push_result_body(11, &item, 1, false, true, true);
    let mut cursor = 8;

    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 1);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 1);
}

#[test]
fn loot_item_push_result_matches_cmangos_looted_flags() {
    let item = CharacterInventoryItem {
        bag: INVENTORY_SLOT_BAG_0 as u32,
        slot: INVENTORY_SLOT_ITEM_START,
        item: 77,
        item_template: 929,
        count: 1,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    };
    let body = build_item_push_result_body(11, &item, 1, false, false, true);
    let mut cursor = 8;

    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 1);
}

#[test]
fn vendor_buy_item_push_result_matches_cmangos_purchased_flags() {
    let item = CharacterInventoryItem {
        bag: INVENTORY_SLOT_BAG_0 as u32,
        slot: INVENTORY_SLOT_ITEM_START,
        item: 77,
        item_template: 929,
        count: 1,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    };
    let body = build_item_push_result_body(11, &item, 1, true, false, true);
    let mut cursor = 8;

    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 1);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 0);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 1);
}

#[test]
fn item_push_result_for_stack_merge_uses_unknown_slot_like_cmangos() {
    let item = CharacterInventoryItem {
        bag: INVENTORY_SLOT_BAG_0 as u32,
        slot: INVENTORY_SLOT_ITEM_START,
        item: 77,
        item_template: 117,
        count: 4,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    };
    let body = build_item_push_result_body(11, &item, 1, true, false, true);
    let mut cursor = 8 + 4 + 4 + 4 + 1;

    assert_eq!(read_u32(&body, &mut cursor).unwrap(), u32::MAX);
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 117);
    cursor += 8;
    assert_eq!(read_u32(&body, &mut cursor).unwrap(), 1);
    assert_eq!(cursor, body.len());
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
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        },
        CharacterInventoryItem {
            bag: INVENTORY_SLOT_BAG_0 as u32,
            slot: INVENTORY_SLOT_ITEM_START + 1,
            item: 78,
            item_template: 777,
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
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
