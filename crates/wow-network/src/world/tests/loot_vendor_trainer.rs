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

fn quest_template_with_required_source_item(
    quest_id: u32,
    item_id: u32,
    item_count: u32,
) -> QuestTemplateQuery {
    let mut quest = test_quest_template(quest_id);
    quest.req_source_id[0] = item_id;
    quest.req_source_count[0] = item_count;
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
        &HashMap::new(),
        &active_quests,
        &quest_statuses,
        &[],
        &HashMap::new(),
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
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    }];

    let selected = select_creature_loot_for_active_quests_with_rolls(
        &loot_rows,
        &HashMap::new(),
        &active_quests,
        &quest_statuses,
        &inventory,
        &HashMap::new(),
        || 0.0,
        |min_count, _max_count| min_count,
    );
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].item, 159);
}

#[test]
fn quest_loot_selection_includes_active_required_source_item() {
    let loot_rows = vec![CreatureLootQuery {
        item: 888,
        group_id: 0,
        min_count: 1,
        max_count: 1,
        display_id: 2,
        chance_or_quest_chance: -100.0,
    }];
    let mut active_quests = HashMap::new();
    active_quests.insert(47, quest_template_with_required_source_item(47, 888, 3));
    let mut quest_statuses = HashMap::new();
    quest_statuses.insert(
        47,
        CharacterQuestStatus {
            quest: 47,
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
        item_template: 888,
        count: 2,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    }];

    let selected = select_creature_loot_for_active_quests_with_rolls(
        &loot_rows,
        &HashMap::new(),
        &active_quests,
        &quest_statuses,
        &inventory,
        &HashMap::new(),
        || 0.0,
        |min_count, _max_count| min_count,
    );
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].item, 888);
}

#[test]
fn quest_loot_selection_uses_source_item_template_limit_when_count_is_zero() {
    let loot_rows = vec![CreatureLootQuery {
        item: 889,
        group_id: 0,
        min_count: 1,
        max_count: 1,
        display_id: 2,
        chance_or_quest_chance: -100.0,
    }];
    let mut active_quests = HashMap::new();
    active_quests.insert(48, quest_template_with_required_source_item(48, 889, 0));
    let mut quest_statuses = HashMap::new();
    quest_statuses.insert(
        48,
        CharacterQuestStatus {
            quest: 48,
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
        item_template: 889,
        count: 4,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    }];
    let mut source_item_default_counts = HashMap::new();
    source_item_default_counts.insert(889, 5);

    let selected = select_creature_loot_for_active_quests_with_rolls(
        &loot_rows,
        &HashMap::new(),
        &active_quests,
        &quest_statuses,
        &inventory,
        &source_item_default_counts,
        || 0.0,
        |min_count, _max_count| min_count,
    );
    assert_eq!(selected.len(), 1);

    let full_inventory = vec![CharacterInventoryItem {
        count: 5,
        ..inventory[0].clone()
    }];
    let selected = select_creature_loot_for_active_quests_with_rolls(
        &loot_rows,
        &HashMap::new(),
        &active_quests,
        &quest_statuses,
        &full_inventory,
        &source_item_default_counts,
        || 0.0,
        |min_count, _max_count| min_count,
    );
    assert!(selected.is_empty());
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
        &HashMap::new(),
        &[],
        &HashMap::new(),
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
        &HashMap::new(),
        &[],
        &HashMap::new(),
        || 0.0,
        |min_count, _max_count| min_count,
    );

    let items = selected.iter().map(|loot| loot.item).collect::<Vec<_>>();
    assert_eq!(items, vec![159, 160]);
}

#[test]
fn creature_loot_roll_processes_reference_templates() {
    let loot_rows = vec![CreatureLootQuery {
        item: 34_000,
        group_id: 0,
        min_count: 0,
        max_count: 2,
        display_id: 0,
        chance_or_quest_chance: 100.0,
    }];
    let mut reference_loot_templates = HashMap::new();
    reference_loot_templates.insert(
        34_000,
        vec![
            CreatureLootQuery {
                item: 16_900,
                group_id: 1,
                min_count: 1,
                max_count: 1,
                display_id: 10,
                chance_or_quest_chance: 0.0,
            },
            CreatureLootQuery {
                item: 16_908,
                group_id: 1,
                min_count: 1,
                max_count: 1,
                display_id: 11,
                chance_or_quest_chance: 0.0,
            },
        ],
    );

    let selected = select_creature_loot_for_active_quests_with_rolls(
        &loot_rows,
        &reference_loot_templates,
        &HashMap::new(),
        &HashMap::new(),
        &[],
        &HashMap::new(),
        || 0.0,
        |_min_count, _max_count| 1,
    );

    let items = selected.iter().map(|loot| loot.item).collect::<Vec<_>>();
    assert_eq!(items, vec![16_908, 16_908]);
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
        &HashMap::new(),
        &[],
        &HashMap::new(),
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
        &HashMap::new(),
        &[],
        &HashMap::new(),
        || 0.0,
        |_min_count, _max_count| 4,
    );
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].min_count, 4);
    assert_eq!(selected[0].max_count, 4);
}

#[test]
fn db_vendor_inventory_uses_cmangos_list_shape() {
    let guid = ObjectGuid::new(HighGuid::Unit, 42, 96_001);
    let db_items = [
        wow_db::VendorItemQuery {
            item: RUST_VENDOR_BAG_ITEM,
            max_count: 0,
            incr_time: 0,
            slot: 7,
            display_id: RUST_VENDOR_BAG_DISPLAY,
            buy_price: 3,
            max_durability: 0,
            buy_count: 2,
            container_slots: 0,
        },
        wow_db::VendorItemQuery {
            item: RUST_VENDOR_BAG_ITEM,
            max_count: 5,
            incr_time: 60,
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
    assert_eq!(&body[13..17], &RUST_VENDOR_BAG_ITEM.to_le_bytes());
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
fn vendor_limited_stock_helpers_decrement_and_restock_like_cmangos() {
    let vendor_item = wow_db::VendorItemQuery {
        item: RUST_VENDOR_BAG_ITEM,
        max_count: 5,
        incr_time: 60,
        slot: 1,
        display_id: RUST_VENDOR_BAG_DISPLAY,
        buy_price: 10,
        max_durability: 0,
        buy_count: 2,
        container_slots: 0,
    };

    let consumed = vendor_item_consume_count(&vendor_item, None, 2, 1_000).unwrap();
    assert_eq!(consumed.count, 3);
    assert_eq!(
        consumed.updated_entry,
        Some(VendorStockEntry {
            count: 3,
            last_increment_time: 1_000,
        })
    );

    let restocked = vendor_item_current_count(&vendor_item, consumed.updated_entry, 1_060);
    assert_eq!(restocked.count, 5);
    assert_eq!(restocked.updated_entry, None);
}

#[test]
fn vendor_limited_stock_helper_rejects_sold_out_purchase() {
    let vendor_item = wow_db::VendorItemQuery {
        item: RUST_VENDOR_BAG_ITEM,
        max_count: 1,
        incr_time: 300,
        slot: 1,
        display_id: RUST_VENDOR_BAG_DISPLAY,
        buy_price: 10,
        max_durability: 0,
        buy_count: 1,
        container_slots: 0,
    };

    assert!(vendor_item_consume_count(&vendor_item, None, 1, 1_000).is_some());
    let sold_out = vendor_item_consume_count(
        &vendor_item,
        Some(VendorStockEntry {
            count: 0,
            last_increment_time: 1_000,
        }),
        1,
        1_001,
    );
    assert!(sold_out.is_none());
}

#[test]
fn vendor_buy_item_in_slot_plan_accepts_last_secondary_bag_slot() {
    let bread = test_item_template(4542, 0, 0, 0.0, 0.0, 0);
    let mut inventory: Vec<_> = (INVENTORY_SLOT_ITEM_START..INVENTORY_SLOT_ITEM_END)
        .map(|slot| CharacterInventoryItem {
            bag: INVENTORY_SLOT_BAG_0 as u32,
            slot,
            item: 3_000 + slot as u32,
            item_template: 6948,
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        })
        .collect();
    inventory.push(CharacterInventoryItem {
        bag: INVENTORY_SLOT_BAG_0 as u32,
        slot: INVENTORY_SLOT_BAG_START,
        item: 77,
        item_template: RUST_VENDOR_BAG_ITEM,
        count: 1,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    });
    inventory.extend((0..5).map(|slot| CharacterInventoryItem {
        bag: INVENTORY_SLOT_BAG_START as u32,
        slot,
        item: 3_100 + slot as u32,
        item_template: 6948,
        count: 1,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    }));
    let equipped_bags = [EquippedBagInfo {
        slot: INVENTORY_SLOT_BAG_START,
        container_slots: 6,
        class: ITEM_CLASS_CONTAINER,
        subclass: ITEM_SUBCLASS_CONTAINER,
    }];
    let bag_model = InventoryBagModel::inventory_only(&equipped_bags);

    let plan = plan_store_vendor_item_in_slot(
        &inventory,
        &bread,
        1,
        &bag_model,
        INVENTORY_SLOT_BAG_START,
        5,
    )
    .unwrap();

    assert_eq!(
        plan,
        vec![StoreSlot {
            bag: INVENTORY_SLOT_BAG_START,
            slot: 5,
            count: 1,
            existing_item: None,
        }]
    );
}

#[test]
fn vendor_sell_bag_in_secondary_bag_slot_matching_bag_id_is_not_treated_as_non_empty() {
    let mut bag_template = test_item_template(RUST_VENDOR_BAG_ITEM, 0, 0, 0.0, 0.0, 0);
    bag_template.container_slots = 6;

    let inventory = [
        CharacterInventoryItem {
            bag: INVENTORY_SLOT_BAG_0 as u32,
            slot: INVENTORY_SLOT_BAG_START,
            item: 77,
            item_template: RUST_VENDOR_BAG_ITEM,
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        },
        CharacterInventoryItem {
            bag: INVENTORY_SLOT_BAG_START as u32,
            slot: INVENTORY_SLOT_BAG_START,
            item: 88,
            item_template: RUST_VENDOR_BAG_ITEM,
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        },
        CharacterInventoryItem {
            bag: INVENTORY_SLOT_BAG_START as u32,
            slot: 0,
            item: 99,
            item_template: 6948,
            count: 1,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: 0,
        },
    ];

    assert!(sell_item_is_non_empty_container(
        &inventory,
        &inventory[0],
        &bag_template
    ));
    assert!(!sell_item_is_non_empty_container(
        &inventory,
        &inventory[1],
        &bag_template
    ));
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
        jump: JumpInfo::default(),
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
fn parses_buy_item_packet() {
    let mut body = Vec::new();
    let vendor_guid = ObjectGuid::new(HighGuid::Unit, 42, 96_001);
    body.extend_from_slice(&vendor_guid.raw().to_le_bytes());
    body.extend_from_slice(&RUST_VENDOR_BAG_ITEM.to_le_bytes());
    body.push(1);
    body.push(1);
    let buy = BuyItemRequest::read(&body).unwrap();
    assert_eq!(buy.vendor_guid, vendor_guid);
    assert_eq!(buy.item, RUST_VENDOR_BAG_ITEM);
    assert_eq!(buy.count, 1);
}

#[test]
fn parses_buy_item_in_slot_packet() {
    let vendor_guid = ObjectGuid::new(HighGuid::Unit, 42, 96_001);
    let bag_guid = ObjectGuid::new(HighGuid::Item, 0, 77);
    let mut body = Vec::new();
    body.extend_from_slice(&vendor_guid.raw().to_le_bytes());
    body.extend_from_slice(&RUST_VENDOR_BAG_ITEM.to_le_bytes());
    body.extend_from_slice(&bag_guid.raw().to_le_bytes());
    body.push(5);
    body.push(2);

    let request = wow_proto::BuyItemInSlotRequest::read(&mut body.as_slice()).unwrap();
    assert_eq!(request.vendor_raw_guid, vendor_guid.raw());
    assert_eq!(request.item, RUST_VENDOR_BAG_ITEM);
    assert_eq!(request.bag_raw_guid, bag_guid.raw());
    assert_eq!(request.bag_slot, 5);
    assert_eq!(request.count, 2);
}
