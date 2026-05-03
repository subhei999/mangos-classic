async fn handle_npc_text_query(
    stream: &mut WorldPacketSink,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let mut cursor = 0;
    let text_id = read_u32(body, &mut cursor)?;
    ensure_available(body, cursor + 8)?;
    let guid = ObjectGuid::from_raw(u64::from_le_bytes(body[cursor..cursor + 8].try_into()?));
    info!(
        text_id,
        guid = format_args!("0x{:016X}", guid.raw()),
        "Answering NPC text query"
    );
    let text = match text_id {
        DB_VENDOR_GOSSIP_TEXT_ID => DB_VENDOR_GOSSIP_TEXT,
        DB_TRAINER_GOSSIP_TEXT_ID => DB_TRAINER_GOSSIP_TEXT,
        _ => RUST_GUIDE_GOSSIP_TEXT,
    };
    let response = build_npc_text_update(text_id, text);
    send_packet(stream, SMSG_NPC_TEXT_UPDATE, &response, Some(header_crypto)).await
}

async fn handle_list_inventory(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let guid = read_packet_guid(body, "CMSG_LIST_INVENTORY")?;
    let response = if guid == rust_guide_guid() {
        build_rust_guide_vendor_inventory()
    } else if guid.is_creature() {
        let vendor_items = wow_db::get_vendor_items(world_db_pool, guid.entry()).await?;
        let list_items: Vec<VendorListItem> = vendor_items.iter().map(Into::into).collect();
        info!(
            entry = guid.entry(),
            guid = format_args!("0x{:016X}", guid.raw()),
            count = list_items.len(),
            "Answering DB-backed vendor inventory request"
        );
        build_vendor_inventory_body(guid, &list_items)
    } else {
        warn!(
            guid = format_args!("0x{:016X}", guid.raw()),
            "Ignoring vendor inventory request for unknown creature"
        );
        return Ok(());
    };
    send_packet(stream, SMSG_LIST_INVENTORY, &response, Some(header_crypto)).await
}

async fn handle_buy_item(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        warn!("Ignoring vendor buy before character login");
        return Ok(());
    };
    let character_guid = character.guid;
    let buy = BuyItemRequest::read(body)?;
    let vendor_item = vendor_buy_item(world_db_pool, buy).await?;
    let Some(vendor_item) = vendor_item else {
        warn!(
            item = buy.item,
            vendor = format_args!("0x{:016X}", buy.vendor_guid.raw()),
            "Ignoring unsupported vendor buy request"
        );
        return Ok(());
    };
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

    let count = buy.count.max(1);
    let total_count = vendor_item.buy_count.max(1).saturating_mul(count as u32);
    let price = vendor_item.price.saturating_mul(count as u32);
    let money = if price == 0 {
        None
    } else {
        match wow_db::spend_character_money(character_db_pool, character_guid, price).await? {
            Some(money) => Some(money),
            None => {
                return send_packet(
                    stream,
                    SMSG_BUY_FAILED,
                    &build_buy_failed_body(buy.vendor_guid, buy.item, BUY_ERR_NOT_ENOUGHT_MONEY),
                    Some(header_crypto),
                )
                .await;
            }
        }
    };
    wow_db::add_character_inventory_item(
        character_db_pool,
        character_guid,
        INVENTORY_SLOT_BAG_0 as u32,
        dst_slot,
        buy.item,
        total_count,
        0,
    )
    .await?;
    session.inventory = wow_db::get_character_inventory_items(character_db_pool, character_guid).await?;
    let Some(new_item) = session
        .inventory
        .iter()
        .find(|item| item.bag == INVENTORY_SLOT_BAG_0 as u32 && item.slot == dst_slot)
    else {
        return Ok(());
    };

    send_packet(
        stream,
        SMSG_BUY_ITEM,
        &build_buy_item_body(buy.vendor_guid, vendor_item.slot, count),
        Some(&mut *header_crypto),
    )
    .await?;
    let owner_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let container_slots = if vendor_item.container_slots > 0 {
        Some(vendor_item.container_slots)
    } else {
        None
    };
    let create_block =
        build_item_create_update_block(owner_guid, owner_guid, new_item, container_slots)?;
    let slot_block = build_inventory_slots_update_block(character_guid, &session.inventory, &[dst_slot])?;
    let body = build_update_object_body(&[create_block, slot_block]);
    send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(&mut *header_crypto)).await?;
    if let Some(money) = money {
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &build_player_money_update_body(character_guid, money)?,
            Some(header_crypto),
        )
        .await?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct VendorBuyItem {
    slot: u32,
    container_slots: u32,
    buy_count: u32,
    price: u32,
}

async fn vendor_buy_item(
    world_db_pool: &MySqlPool,
    buy: BuyItemRequest,
) -> anyhow::Result<Option<VendorBuyItem>> {
    if buy.vendor_guid == rust_guide_guid() {
        return Ok(rust_guide_vendor_slot(buy.item).map(|slot| VendorBuyItem {
            slot,
            container_slots: if buy.item == RUST_VENDOR_BAG_ITEM { 6 } else { 0 },
            buy_count: 1,
            price: 0,
        }));
    }

    if !buy.vendor_guid.is_creature() {
        return Ok(None);
    }

    let vendor_items = wow_db::get_vendor_items(world_db_pool, buy.vendor_guid.entry()).await?;
    Ok(vendor_items
        .iter()
        .enumerate()
        .find(|(_, item)| item.item == buy.item)
        .map(|(index, item)| VendorBuyItem {
            slot: (index + 1) as u32,
            container_slots: item.container_slots,
            buy_count: item.buy_count,
            price: item.buy_price,
        }))
}

async fn handle_sell_item(
    stream: &mut WorldPacketSink,
    deps: QuestMutationDeps<'_>,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let character_db_pool = deps.character_db_pool;
    let world_db_pool = deps.world_db_pool;
    let Some(character) = &session.active_character else {
        warn!("Ignoring vendor sell before character login");
        return Ok(());
    };
    let character_guid = character.guid;
    let request = SellItemRequest::read(body)?;
    let vendor_valid = if request.vendor_guid == rust_guide_guid() {
        true
    } else if request.vendor_guid.is_creature() {
        !wow_db::get_vendor_items(world_db_pool, request.vendor_guid.entry())
            .await?
            .is_empty()
    } else {
        false
    };
    if !vendor_valid {
        return send_packet(
            stream,
            SMSG_SELL_ITEM,
            &build_sell_item_error_body(
                request.vendor_guid,
                request.item_guid,
                SELL_ERR_CANT_FIND_VENDOR,
            ),
            Some(header_crypto),
        )
        .await;
    }

    let Some(source_item) = session
        .inventory
        .iter()
        .find(|item| item.item == request.item_guid.counter())
        .cloned()
    else {
        return Ok(());
    };
    let Some(template) = wow_db::get_item_template_query(world_db_pool, source_item.item_template).await?
    else {
        return send_packet(
            stream,
            SMSG_SELL_ITEM,
            &build_sell_item_error_body(
                request.vendor_guid,
                request.item_guid,
                SELL_ERR_CANT_SELL_ITEM,
            ),
            Some(header_crypto),
        )
        .await;
    };
    let count = if request.count == 0 {
        source_item.count
    } else {
        request.count as u32
    };
    if count == 0
        || count > source_item.count
        || template.sell_price == 0
        || (template.container_slots > 0
            && session
                .inventory
                .iter()
                .any(|item| item.bag == source_item.slot as u32))
    {
        return send_packet(
            stream,
            SMSG_SELL_ITEM,
            &build_sell_item_error_body(
                request.vendor_guid,
                request.item_guid,
                SELL_ERR_CANT_SELL_ITEM,
            ),
            Some(header_crypto),
        )
        .await;
    }

    let sold = wow_db::destroy_character_inventory_item_count(
        character_db_pool,
        character_guid,
        source_item.bag,
        source_item.slot,
        count,
    )
    .await?;
    let Some(sold) = sold else {
        return Ok(());
    };
    let money = wow_db::add_character_money(
        character_db_pool,
        character_guid,
        template.sell_price.saturating_mul(count),
    )
    .await?;
    session.inventory = wow_db::get_character_inventory_items(character_db_pool, character_guid).await?;

    match sold {
        wow_db::InventoryDestroyResult::CountChanged { item, count } => {
            let body = build_item_stack_count_update_body(item, count)?;
            send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(&mut *header_crypto)).await?;
        }
        wow_db::InventoryDestroyResult::Removed { item } => {
            let body = if source_item.bag == INVENTORY_SLOT_BAG_0 as u32 {
                build_inventory_slots_update_body(
                    character_guid,
                    &session.inventory,
                    &[source_item.slot],
                )?
            } else {
                build_destroy_object_body(item)
            };
            let opcode = if source_item.bag == INVENTORY_SLOT_BAG_0 as u32 {
                SMSG_UPDATE_OBJECT
            } else {
                SMSG_DESTROY_OBJECT
            };
            send_packet(stream, opcode, &body, Some(&mut *header_crypto)).await?;
        }
    }
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_money_update_body(character_guid, money)?,
        Some(&mut *header_crypto),
    )
    .await?;

    revalidate_completed_item_quests_after_inventory_change(
        stream,
        deps,
        session,
        character_guid,
        header_crypto,
    )
    .await
}

