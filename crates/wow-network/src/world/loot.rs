async fn handle_loot(
    stream: &mut TcpStream,
    world_db_pool: &MySqlPool,
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
    if needs_loot_item {
        let loot_item = wow_db::get_creature_loot_items(world_db_pool, entry)
            .await?
            .into_iter()
            .next()
            .map(DbCreatureLootRuntime::from);
        if let Some(creature) = session.db_creatures.get_mut(&target.raw()) {
            creature.loot_item = loot_item;
        }
    }
    let creature = session
        .db_creatures
        .get_mut(&target.raw())
        .expect("DB creature existed before loot query");
    creature.looting = true;
    let response = build_db_creature_loot_response_body(target, creature);
    send_packet(stream, SMSG_LOOT_RESPONSE, &response, Some(header_crypto)).await
}

async fn handle_autostore_loot_item(
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        warn!("Ignoring loot item request before character login");
        return Ok(());
    };
    let character_guid = character.guid;
    if body.is_empty() {
        anyhow::bail!("CMSG_AUTOSTORE_LOOT_ITEM payload too short: {} bytes", body.len());
    }
    let loot_slot = body[0];
    let db_loot = session
        .db_creatures
        .iter()
        .find_map(|(guid, creature)| {
            creature
                .looting
                .then(|| creature.loot_item.as_ref().map(|loot| (*guid, loot.clone())))
                .flatten()
        });
    if let Some((creature_guid, loot)) = db_loot {
        if loot_slot != 0 {
            warn!(loot_slot, "Ignoring unsupported DB creature loot slot");
            return Ok(());
        }
        return autostore_loot_item(
            LootAutostoreContext {
                stream,
                character_db_pool,
                world_db_pool,
                session,
                header_crypto,
                character_guid,
            },
            creature_guid,
            loot,
            loot_slot,
        )
        .await;
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
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        warn!("Ignoring loot money request before character login");
        return Ok(());
    };
    if let Some((creature_guid, money)) = session
        .db_creatures
        .iter()
        .find(|(_, creature)| creature.looting && creature.loot_money_available)
        .map(|(guid, creature)| (*guid, creature.loot_money()))
    {
        let gained_money = money;
        let money =
            wow_db::add_character_money(character_db_pool, character.guid, gained_money).await?;
        if let Some(creature) = session.db_creatures.get_mut(&creature_guid) {
            creature.loot_money_available = false;
        }
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
    stream: &mut TcpStream,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let target = read_packet_guid(body, "CMSG_LOOT_RELEASE")?;
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

    let Some(creature) = session.db_creatures.get_mut(&target.raw()) else {
        warn!(
            target = format_args!("0x{:016X}", target.raw()),
            "Ignoring loot release for unknown target"
        );
        return Ok(());
    };
    creature.respawn();
    send_packet(
        stream,
        SMSG_LOOT_RELEASE_RESPONSE,
        &build_loot_release_response_body(target, true),
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_db_creature_state_update_body(target, creature.health, 0)?,
        Some(header_crypto),
    )
    .await
}

