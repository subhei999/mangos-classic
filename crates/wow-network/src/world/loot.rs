async fn handle_loot(
    stream: &mut WorldPacketSink,
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
        let Some(gameobject) = session.db_gameobjects.get(&target.raw()).cloned() else {
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
        let loot_item =
            select_db_gameobject_loot_item_for_character(world_db_pool, session, &gameobject.spawn.template)
                .await?;
        session.db_gameobject_looting = Some(target.raw());
        session.db_gameobject_loot_item = loot_item;
        let response = build_gameobject_loot_response_body(target, session.db_gameobject_loot_item.as_ref());
        return send_packet(stream, SMSG_LOOT_RESPONSE, &response, Some(header_crypto)).await;
    }
    let Some(creature) = session.db_creatures.get(&target.raw()) else {
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
    let needs_loot_item = creature.loot_item.is_none();
    let entry = creature.spawn.entry;
    let loot_item = if needs_loot_item {
        select_db_creature_loot_item_for_character(world_db_pool, session, entry).await?
    } else {
        None
    };
    let Some(creature) = shared_world
        .maps
        .open_db_creature_loot(character.position.map_id, target.raw(), loot_item)
        .await
    else {
        warn!("Ignoring loot request for DB creature before it is lootable");
        return Ok(());
    };
    session.db_creatures.insert(target.raw(), creature.clone());
    let response = build_db_creature_loot_response_body(target, &creature);
    send_packet(stream, SMSG_LOOT_RESPONSE, &response, Some(header_crypto)).await
}

async fn select_db_creature_loot_item_for_character(
    world_db_pool: &MySqlPool,
    session: &WorldSessionState,
    creature_entry: u32,
) -> anyhow::Result<Option<DbCreatureLootRuntime>> {
    let loot_rows = wow_db::get_creature_loot_items(world_db_pool, creature_entry).await?;
    if loot_rows.is_empty() {
        return Ok(None);
    }

    let active_quest_ids: Vec<u32> = session
        .quest_statuses
        .values()
        .filter(|status| status.rewarded == 0 && status.status == QUEST_STATUS_INCOMPLETE)
        .map(|status| status.quest)
        .collect();
    let mut active_quests = HashMap::new();
    for quest_id in active_quest_ids {
        if let Some(quest) = wow_db::get_quest_template_query(world_db_pool, quest_id).await? {
            active_quests.insert(quest_id, quest);
        }
    }

    Ok(select_creature_loot_for_active_quests(
        &loot_rows,
        &active_quests,
        &session.quest_statuses,
        &session.inventory,
    )
    .map(DbCreatureLootRuntime::from))
}

async fn select_db_gameobject_loot_item_for_character(
    world_db_pool: &MySqlPool,
    session: &WorldSessionState,
    template: &wow_db::GameObjectTemplateQuery,
) -> anyhow::Result<Option<DbCreatureLootRuntime>> {
    let Some(loot_id) = gameobject_chest_loot_id(template) else {
        return Ok(None);
    };
    let loot_rows = wow_db::get_gameobject_loot_items(world_db_pool, loot_id).await?;
    if loot_rows.is_empty() {
        return Ok(None);
    }

    let active_quest_ids: Vec<u32> = session
        .quest_statuses
        .values()
        .filter(|status| status.rewarded == 0 && status.status == QUEST_STATUS_INCOMPLETE)
        .map(|status| status.quest)
        .collect();
    let mut active_quests = HashMap::new();
    for quest_id in active_quest_ids {
        if let Some(quest) = wow_db::get_quest_template_query(world_db_pool, quest_id).await? {
            active_quests.insert(quest_id, quest);
        }
    }

    Ok(select_creature_loot_for_active_quests(
        &loot_rows,
        &active_quests,
        &session.quest_statuses,
        &session.inventory,
    )
    .filter(|loot| loot.is_quest_drop())
    .map(DbCreatureLootRuntime::from))
}

fn select_creature_loot_for_active_quests(
    loot_rows: &[CreatureLootQuery],
    active_quests: &HashMap<u32, QuestTemplateQuery>,
    quest_statuses: &HashMap<u32, CharacterQuestStatus>,
    inventory: &[CharacterInventoryItem],
) -> Option<CreatureLootQuery> {
    let mut first_normal = None;
    let mut first_quest = None;
    for loot in loot_rows {
        if loot.is_quest_drop() {
            if player_needs_quest_loot_item(loot.item, active_quests, quest_statuses, inventory) {
                first_quest = Some(loot.clone());
                break;
            }
            continue;
        }
        if first_normal.is_none() {
            first_normal = Some(loot.clone());
        }
    }

    first_quest.or(first_normal)
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
    let db_loot_creature_guid = session
        .db_creatures
        .iter()
        .find_map(|(guid, creature)| {
            creature
                .looting
                .then(|| creature.loot_item.as_ref().map(|loot| (*guid, loot.clone())))
                .flatten()
                .map(|(guid, _)| guid)
        });
    if let Some(creature_guid) = db_loot_creature_guid {
        if loot_slot != 0 {
            warn!(loot_slot, "Ignoring unsupported DB creature loot slot");
            return Ok(());
        }
        let map_id = character
            .position
            .map_id;
        let Some((loot, creature)) = shared_world
            .maps
            .take_db_creature_loot_item(map_id, creature_guid)
            .await
        else {
            warn!("Ignoring DB creature loot item request after shared loot was claimed");
            return Ok(());
        };
        session.db_creatures.insert(creature_guid, creature);
        let stored = autostore_loot_item(
            LootAutostoreContext {
                stream,
                character_db_pool,
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
            if let Some(creature) = shared_world
                .maps
                .restore_db_creature_loot_item(map_id, creature_guid, loot)
                .await
            {
                session.db_creatures.insert(creature_guid, creature);
            }
        }
        return Ok(());
    }

    if let Some(gameobject_guid) = session.db_gameobject_looting {
        if loot_slot != 0 {
            warn!(loot_slot, "Ignoring unsupported DB gameobject loot slot");
            return Ok(());
        }
        let Some(loot) = session.db_gameobject_loot_item.take() else {
            return Ok(());
        };
        let stored = autostore_loot_item(
            LootAutostoreContext {
                stream,
                character_db_pool,
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
            session.db_gameobject_loot_item = Some(loot);
        } else {
            let guid = ObjectGuid::from_raw(gameobject_guid);
            let consumed = shared_world
                .maps
                .consume_db_gameobject(character_map_id, guid, Instant::now(), Some(character_guid))
                .await;
            if let Some((gameobject, observer_packets)) = consumed {
                session.db_gameobjects.insert(gameobject_guid, gameobject);
                shared_world.sessions.dispatch(observer_packets).await;
            }
            send_packet(
                stream,
                SMSG_DESTROY_OBJECT,
                &gameobject_guid.to_le_bytes(),
                Some(&mut *header_crypto),
            )
            .await?;
            session.db_gameobject_looting = None;
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
    if let Some(creature_guid) = session
        .db_creatures
        .iter()
        .find(|(_, creature)| creature.looting && creature.loot_money_available)
        .map(|(guid, _)| *guid)
    {
        let Some((gained_money, creature)) = shared_world
            .maps
            .take_db_creature_loot_money(character.position.map_id, creature_guid)
            .await
        else {
            return Ok(());
        };
        let money =
            wow_db::add_character_money(character_db_pool, character.guid, gained_money).await?;
        session.db_creatures.insert(creature_guid, creature);
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
        session.db_gameobject_looting = None;
        session.db_gameobject_loot_item = None;
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
    session.db_creatures.insert(target.raw(), event.creature);
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
