async fn handle_loot(
    stream: &mut WorldPacketSink,
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    shared_world: SharedWorldDeps<'_>,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let target = read_packet_guid(body, "CMSG_LOOT")?;
    if target == rust_combat_dummy_guid() {
        if !session.combat_dummy_lootable {
            warn!("Ignoring loot request for combat dummy before it is lootable");
            return Ok(());
        }

        session.combat_dummy_looting = true;
        let response = build_combat_dummy_loot_response_body(session);
        return send_packet(stream, SMSG_LOOT_RESPONSE, &response, Some(header_crypto)).await;
    }

    let Some(character) = session.active_character.as_ref() else {
        warn!("Ignoring loot request before character login");
        return Ok(());
    };
    if target.is_game_object() {
        let Some(gameobject) = shared_world
            .maps
            .db_gameobject_snapshot(character.position.map_id, target)
            .await
        else {
            warn!(
                target = format_args!("0x{:016X}", target.raw()),
                "Ignoring loot request for unknown gameobject"
            );
            return Ok(());
        };
        if gameobject.spawn.map != character.position.map_id
            || !is_position_inside_radius(gameobject.position(), character.position, 8.0)
        {
            warn!("Ignoring gameobject loot request outside interaction range");
            return Ok(());
        }
        let loot_items = select_db_gameobject_loot_item_for_character(
            object_mgr,
            world_db_pool,
            session,
            &gameobject.spawn.template,
        )
        .await?;
        let Some((gameobject, loot_items)) = shared_world
            .maps
            .open_db_gameobject_loot(
                character.position.map_id,
                target.raw(),
                character.guid,
                loot_items,
            )
            .await
        else {
            warn!("Ignoring loot request for unavailable gameobject");
            return Ok(());
        };
        let _ = gameobject;
        let response = build_gameobject_loot_response_body(target, &loot_items);
        return send_packet(stream, SMSG_LOOT_RESPONSE, &response, Some(header_crypto)).await;
    }
    let Some(creature) = shared_world
        .maps
        .db_creature_snapshot(character.position.map_id, target)
        .await
    else {
        warn!(
            target = format_args!("0x{:016X}", target.raw()),
            "Ignoring loot request for unknown target"
        );
        return Ok(());
    };
    if !creature.lootable {
        warn!("Ignoring loot request for DB creature before it is lootable");
        return Ok(());
    }
    let needs_loot_item = shared_world
        .maps
        .db_creature_needs_loot_item(character.position.map_id, target.raw())
        .await
        .unwrap_or(creature.loot_items.is_empty());
    let entry = creature.spawn.entry;
    let loot_items = if needs_loot_item {
        select_db_creature_loot_item_for_character(object_mgr, world_db_pool, session, entry)
            .await?
    } else {
        Vec::new()
    };
    let Some(creature) = shared_world
        .maps
        .open_db_creature_loot(
            character.position.map_id,
            target.raw(),
            character.guid,
            loot_items,
        )
        .await
    else {
        warn!("Ignoring loot request for DB creature before it is lootable");
        return Ok(());
    };
    let response = build_db_creature_loot_response_body(target, &creature);
    send_packet(stream, SMSG_LOOT_RESPONSE, &response, Some(header_crypto)).await
}

async fn select_db_creature_loot_item_for_character(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    session: &WorldSessionState,
    creature_entry: u32,
) -> anyhow::Result<Vec<DbCreatureLootRuntime>> {
    let loot_rows = object_mgr
        .creature_loot_items(world_db_pool, creature_entry)
        .await?;
    if loot_rows.is_empty() {
        return Ok(Vec::new());
    }

    let active_quest_ids: Vec<u32> = session
        .quest_statuses
        .values()
        .filter(|status| status.rewarded == 0 && status.status == QUEST_STATUS_INCOMPLETE)
        .map(|status| status.quest)
        .collect();
    let mut active_quests = HashMap::new();
    for quest_id in active_quest_ids {
        if let Some(quest) = object_mgr.quest_template(world_db_pool, quest_id).await? {
            active_quests.insert(quest_id, quest);
        }
    }

    Ok(select_creature_loot_for_active_quests(
        &loot_rows,
        &active_quests,
        &session.quest_statuses,
        &session.inventory,
    )
    .into_iter()
    .map(DbCreatureLootRuntime::from)
    .collect())
}

async fn select_db_gameobject_loot_item_for_character(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    session: &WorldSessionState,
    template: &wow_db::GameObjectTemplateQuery,
) -> anyhow::Result<Vec<DbCreatureLootRuntime>> {
    let Some(loot_id) = gameobject_chest_loot_id(template) else {
        return Ok(Vec::new());
    };
    let loot_rows = object_mgr
        .gameobject_loot_items(world_db_pool, loot_id)
        .await?;
    if loot_rows.is_empty() {
        return Ok(Vec::new());
    }

    let active_quest_ids: Vec<u32> = session
        .quest_statuses
        .values()
        .filter(|status| status.rewarded == 0 && status.status == QUEST_STATUS_INCOMPLETE)
        .map(|status| status.quest)
        .collect();
    let mut active_quests = HashMap::new();
    for quest_id in active_quest_ids {
        if let Some(quest) = object_mgr.quest_template(world_db_pool, quest_id).await? {
            active_quests.insert(quest_id, quest);
        }
    }

    Ok(select_creature_loot_for_active_quests(
        &loot_rows,
        &active_quests,
        &session.quest_statuses,
        &session.inventory,
    )
    .into_iter()
    .filter(|loot| loot.is_quest_drop())
    .map(DbCreatureLootRuntime::from)
    .collect())
}

fn select_creature_loot_for_active_quests(
    loot_rows: &[CreatureLootQuery],
    active_quests: &HashMap<u32, QuestTemplateQuery>,
    quest_statuses: &HashMap<u32, CharacterQuestStatus>,
    inventory: &[CharacterInventoryItem],
) -> Vec<CreatureLootQuery> {
    let mut chance_rng = rand::thread_rng();
    let mut count_rng = rand::thread_rng();
    select_creature_loot_for_active_quests_with_rolls(
        loot_rows,
        active_quests,
        quest_statuses,
        inventory,
        || rand::Rng::gen_range(&mut chance_rng, 0.0f32..100.0f32),
        |min_count, max_count| rand::Rng::gen_range(&mut count_rng, min_count..=max_count),
    )
}

fn select_creature_loot_for_active_quests_with_rolls(
    loot_rows: &[CreatureLootQuery],
    active_quests: &HashMap<u32, QuestTemplateQuery>,
    quest_statuses: &HashMap<u32, CharacterQuestStatus>,
    inventory: &[CharacterInventoryItem],
    mut chance_roll: impl FnMut() -> f32,
    mut count_roll: impl FnMut(u32, u32) -> u32,
) -> Vec<CreatureLootQuery> {
    let mut rolled = Vec::new();
    let mut grouped: HashMap<u8, Vec<CreatureLootQuery>> = HashMap::new();
    for loot in loot_rows {
        if loot.group_id > 0 {
            grouped.entry(loot.group_id).or_default().push(loot.clone());
            continue;
        }

        if let Some(loot) = roll_loot_row(
            loot,
            active_quests,
            quest_statuses,
            inventory,
            &mut chance_roll,
            &mut count_roll,
        ) {
            rolled.push(loot);
        }
    }

    let mut group_ids = grouped.keys().copied().collect::<Vec<_>>();
    group_ids.sort_unstable();
    for group_id in group_ids {
        let Some(rows) = grouped.remove(&group_id) else {
            continue;
        };
        if let Some(loot) = roll_loot_group(
            rows,
            active_quests,
            quest_statuses,
            inventory,
            &mut chance_roll,
            &mut count_roll,
        ) {
            rolled.push(loot);
        }
    }

    rolled
}

fn roll_loot_group(
    rows: Vec<CreatureLootQuery>,
    active_quests: &HashMap<u32, QuestTemplateQuery>,
    quest_statuses: &HashMap<u32, CharacterQuestStatus>,
    inventory: &[CharacterInventoryItem],
    mut chance_roll: impl FnMut() -> f32,
    mut count_roll: impl FnMut(u32, u32) -> u32,
) -> Option<CreatureLootQuery> {
    let mut explicit_chance = Vec::new();
    let mut equal_chance = Vec::new();
    for loot in rows {
        if loot.is_quest_drop()
            && !player_needs_quest_loot_item(loot.item, active_quests, quest_statuses, inventory)
        {
            continue;
        }
        let chance = loot.chance_or_quest_chance.abs().clamp(0.0, 100.0);
        if chance > 0.0 {
            explicit_chance.push((loot, chance));
        } else {
            equal_chance.push(loot);
        }
    }

    let selected = if !explicit_chance.is_empty() {
        let mut remaining = chance_roll();
        let mut selected = None;
        for (loot, chance) in explicit_chance {
            remaining -= chance;
            if remaining < 0.0 || chance >= 100.0 {
                selected = Some(loot);
                break;
            }
        }
        selected
    } else if equal_chance.is_empty() {
        None
    } else {
        let index =
            count_roll(0, (equal_chance.len() - 1) as u32).min((equal_chance.len() - 1) as u32);
        equal_chance.get(index as usize).cloned()
    }?;

    roll_loot_row(
        &selected,
        active_quests,
        quest_statuses,
        inventory,
        || 0.0,
        count_roll,
    )
}

fn roll_loot_row(
    loot: &CreatureLootQuery,
    active_quests: &HashMap<u32, QuestTemplateQuery>,
    quest_statuses: &HashMap<u32, CharacterQuestStatus>,
    inventory: &[CharacterInventoryItem],
    mut chance_roll: impl FnMut() -> f32,
    mut count_roll: impl FnMut(u32, u32) -> u32,
) -> Option<CreatureLootQuery> {
    let is_quest_drop = loot.is_quest_drop();
    if is_quest_drop
        && !player_needs_quest_loot_item(loot.item, active_quests, quest_statuses, inventory)
    {
        return None;
    }
    let chance = if is_quest_drop {
        -loot.chance_or_quest_chance
    } else {
        loot.chance_or_quest_chance
    }
    .clamp(0.0, 100.0);
    if chance <= 0.0 || chance_roll() >= chance {
        return None;
    }

    let min_count = loot.min_count.max(1);
    let max_count = loot.max_count.max(min_count);
    let rolled_count = count_roll(min_count, max_count).clamp(min_count, max_count);
    let mut rolled_loot = loot.clone();
    rolled_loot.min_count = rolled_count;
    rolled_loot.max_count = rolled_count;
    Some(rolled_loot)
}

fn player_needs_quest_loot_item(
    item_id: u32,
    active_quests: &HashMap<u32, QuestTemplateQuery>,
    quest_statuses: &HashMap<u32, CharacterQuestStatus>,
    inventory: &[CharacterInventoryItem],
) -> bool {
    let owned_count: u32 = inventory
        .iter()
        .filter(|item| item.item_template == item_id)
        .map(|item| item.count)
        .sum();

    quest_statuses
        .values()
        .filter(|status| status.rewarded == 0 && status.status == QUEST_STATUS_INCOMPLETE)
        .any(|status| {
            let Some(quest) = active_quests.get(&status.quest) else {
                return false;
            };
            quest.req_item_id
                .iter()
                .zip(quest.req_item_count.iter())
                .any(|(req_item_id, req_count)| {
                    *req_item_id == item_id && owned_count < *req_count && *req_count > 0
                })
        })
}

async fn handle_autostore_loot_item(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    shared_world: SharedWorldDeps<'_>,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        warn!("Ignoring loot item request before character login");
        return Ok(());
    };
    let character_guid = character.guid;
    let character_map_id = character.position.map_id;
    if body.is_empty() {
        anyhow::bail!("CMSG_AUTOSTORE_LOOT_ITEM payload too short: {} bytes", body.len());
    }
    let loot_slot = body[0];
    if shared_world
        .maps
        .db_creature_loot_guid_for_character(character_map_id, character_guid)
        .await
        .is_some()
    {
        let Some((creature_guid, loot_slot, loot, _creature)) = shared_world
            .maps
            .take_db_creature_loot_item(character_map_id, character_guid, loot_slot)
            .await
        else {
            warn!("Ignoring DB creature loot item request after shared loot was claimed");
            return Ok(());
        };
        let stored = autostore_loot_item(
            LootAutostoreContext {
                stream,
                character_db_pool,
                object_mgr: shared_world.object_mgr,
                world_db_pool,
                session,
                header_crypto,
                character_guid,
            },
            creature_guid,
            loot.clone(),
            loot_slot,
        )
        .await?;
        if !stored {
            shared_world
                .maps
                .restore_db_creature_loot_item(character_map_id, creature_guid, loot_slot, loot)
                .await;
        }
        return Ok(());
    }

    if shared_world
        .maps
        .db_gameobject_loot_guid_for_character(character_map_id, character_guid)
        .await
        .is_some()
    {
        let Some((gameobject_guid, loot_slot, loot)) = shared_world
            .maps
            .take_db_gameobject_loot_item(character_map_id, character_guid, loot_slot)
            .await
        else {
            return Ok(());
        };
        let stored = autostore_loot_item(
            LootAutostoreContext {
                stream,
                character_db_pool,
                object_mgr: shared_world.object_mgr,
                world_db_pool,
                session,
                header_crypto,
                character_guid,
            },
            gameobject_guid,
            loot.clone(),
            loot_slot,
        )
        .await?;
        if !stored {
            shared_world
                .maps
                .restore_db_gameobject_loot_item(character_map_id, gameobject_guid, loot_slot, loot)
                .await;
        } else {
            let guid = ObjectGuid::from_raw(gameobject_guid);
            let consumed = shared_world
                .maps
                .consume_db_gameobject(character_map_id, guid, Instant::now(), Some(character_guid))
                .await;
            if let Some((gameobject, observer_packets)) = consumed {
                let _ = gameobject;
                shared_world.sessions.dispatch(observer_packets).await;
            }
            send_packet(
                stream,
                SMSG_DESTROY_OBJECT,
                &gameobject_guid.to_le_bytes(),
                Some(&mut *header_crypto),
            )
            .await?;
        }
        return Ok(());
    }

    if !session.combat_dummy_looting || loot_slot != 0 || !session.combat_dummy_loot_item_available {
        warn!(
            loot_slot,
            "Ignoring loot item request without available combat dummy loot"
        );
        return Ok(());
    }

    let max_stack = wow_db::get_item_template_query(world_db_pool, RUST_COMBAT_DUMMY_LOOT_ITEM)
        .await?
        .map(|template| template.stackable.max(1))
        .unwrap_or(1);
    let mut remaining_count = RUST_COMBAT_DUMMY_LOOT_ITEM_COUNT;
    let mut update_blocks = Vec::new();

    if max_stack > 1 {
        if let Some(existing_stack) = session
            .inventory
            .iter()
            .filter(|item| {
                item.item_template == RUST_COMBAT_DUMMY_LOOT_ITEM
                    && item.count < max_stack
                    && remaining_count <= max_stack - item.count
                    && u8::try_from(item.bag)
                        .ok()
                        .is_some_and(|bag| is_supported_storage_position(bag, item.slot))
            })
            .min_by_key(|item| {
                let bag_order = if item.bag == INVENTORY_SLOT_BAG_0 as u32 {
                    0
                } else {
                    1
                };
                (bag_order, item.bag, item.slot)
            })
            .cloned()
        {
            let merged_count = existing_stack.count + remaining_count;
            if wow_db::update_character_inventory_item_count(
                character_db_pool,
                character_guid,
                existing_stack.item,
                merged_count,
            )
            .await?
            {
                remaining_count = 0;
                update_blocks.push(build_item_stack_count_update_block(
                    existing_stack.item,
                    merged_count,
                )?);
            }
        }
    }

    if remaining_count == 0 {
        session.combat_dummy_loot_item_available = false;
        session.inventory =
            wow_db::get_character_inventory_items(character_db_pool, character_guid).await?;
        send_packet(
            stream,
            SMSG_LOOT_REMOVED,
            &[loot_slot],
            Some(&mut *header_crypto),
        )
        .await?;
        let body = build_update_object_body(&update_blocks);
        return send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(header_crypto)).await;
    }

    let Some(dst_slot) = first_empty_backpack_slot(&session.inventory) else {
        send_inventory_change_failure(
            stream,
            EQUIP_ERR_COULDNT_SPLIT_ITEMS,
            None,
            None,
            header_crypto,
        )
        .await?;
        return Ok(());
    };

    wow_db::add_character_inventory_item(
        character_db_pool,
        character_guid,
        INVENTORY_SLOT_BAG_0 as u32,
        dst_slot,
        RUST_COMBAT_DUMMY_LOOT_ITEM,
        remaining_count,
        0,
    )
    .await?;
    session.combat_dummy_loot_item_available = false;
    session.inventory = wow_db::get_character_inventory_items(character_db_pool, character_guid).await?;
    let Some(new_item) = session.inventory.iter().find(|item| {
        item.bag == INVENTORY_SLOT_BAG_0 as u32
            && item.slot == dst_slot
            && item.item_template == RUST_COMBAT_DUMMY_LOOT_ITEM
    }) else {
        return Ok(());
    };

    send_packet(stream, SMSG_LOOT_REMOVED, &[loot_slot], Some(&mut *header_crypto)).await?;
    let owner_guid = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let create_block = build_item_create_update_block(owner_guid, owner_guid, new_item, None)?;
    let slot_block = build_inventory_slots_update_block(character_guid, &session.inventory, &[dst_slot])?;
    update_blocks.push(create_block);
    update_blocks.push(slot_block);
    let body = build_update_object_body(&update_blocks);
    send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(header_crypto)).await
}

async fn handle_loot_money(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        warn!("Ignoring loot money request before character login");
        return Ok(());
    };
    if shared_world
        .maps
        .db_creature_loot_guid_for_character(character.position.map_id, character.guid)
        .await
        .is_some()
    {
        let Some((gained_money, _creature)) = shared_world
            .maps
            .take_db_creature_loot_money(character.position.map_id, character.guid)
            .await
        else {
            return Ok(());
        };
        let money =
            wow_db::add_character_money(character_db_pool, character.guid, gained_money).await?;
        send_packet(
            stream,
            SMSG_LOOT_MONEY_NOTIFY,
            &gained_money.to_le_bytes(),
            Some(&mut *header_crypto),
        )
        .await?;
        send_packet(stream, SMSG_LOOT_CLEAR_MONEY, &[], Some(&mut *header_crypto)).await?;
        return send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &build_player_money_update_body(character.guid, money)?,
            Some(header_crypto),
        )
        .await;
    }

    if !session.combat_dummy_looting {
        warn!("Ignoring loot money request without an open combat dummy loot window");
        return Ok(());
    }
    if !session.combat_dummy_loot_money_available {
        return Ok(());
    }

    let money = wow_db::add_character_money(
        character_db_pool,
        character.guid,
        RUST_COMBAT_DUMMY_LOOT_MONEY,
    )
    .await?;
    session.combat_dummy_loot_money_available = false;
    send_packet(
        stream,
        SMSG_LOOT_MONEY_NOTIFY,
        &RUST_COMBAT_DUMMY_LOOT_MONEY.to_le_bytes(),
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(stream, SMSG_LOOT_CLEAR_MONEY, &[], Some(&mut *header_crypto)).await?;
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_money_update_body(character.guid, money)?,
        Some(header_crypto),
    )
    .await
}

async fn handle_loot_release(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let target = read_packet_guid(body, "CMSG_LOOT_RELEASE")?;
    if target.is_game_object() {
        if let Some(character) = session.active_character.as_ref() {
            shared_world
                .maps
                .release_db_gameobject_loot(
                    character.position.map_id,
                    target.raw(),
                    character.guid,
                )
                .await;
        }
        return send_packet(
            stream,
            SMSG_LOOT_RELEASE_RESPONSE,
            &build_loot_release_response_body(target, true),
            Some(header_crypto),
        )
        .await;
    }
    if target == rust_combat_dummy_guid() {
        session.combat_dummy_looting = false;
        session.combat_dummy_lootable = false;
        session.combat_dummy_loot_money_available = false;
        session.combat_dummy_loot_item_available = false;
        session.combat_dummy_health = RUST_COMBAT_DUMMY_HEALTH;
        send_packet(
            stream,
            SMSG_LOOT_RELEASE_RESPONSE,
            &build_loot_release_response_body(target, true),
            Some(&mut *header_crypto),
        )
        .await?;
        return send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &build_combat_dummy_state_update_body(RUST_COMBAT_DUMMY_HEALTH, 0)?,
            Some(header_crypto),
        )
        .await;
    }

    let Some(character) = session.active_character.as_ref() else {
        warn!("Ignoring loot release before character login");
        return Ok(());
    };
    let Some(event) = shared_world
        .maps
        .release_db_creature_loot(
            character.position.map_id,
            target.raw(),
            Instant::now(),
            Some(character.guid),
        )
        .await?
    else {
        warn!(
            target = format_args!("0x{:016X}", target.raw()),
            "Ignoring loot release for unknown target"
        );
        return Ok(());
    };
    let _ = event.creature;
    send_packet(
        stream,
        SMSG_LOOT_RELEASE_RESPONSE,
        &build_loot_release_response_body(target, true),
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        event.direct_packet.opcode,
        &event.direct_packet.body,
        Some(header_crypto),
    )
    .await?;
    shared_world.sessions.dispatch(event.observer_packets).await;
    Ok(())
}
