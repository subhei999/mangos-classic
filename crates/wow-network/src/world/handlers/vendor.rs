use super::*;
use wow_proto::world::WorldOpcode;

pub(in crate::world) async fn handle_npc_text_query(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    request: wow_proto::NpcTextQueryRequest,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let text_id = request.text_id;
    let guid = ObjectGuid::from_raw(request.raw_guid);
    info!(
        text_id,
        guid = format_args!("0x{:016X}", guid.raw()),
        "Answering NPC text query"
    );
    let text = match text_id {
        DB_VENDOR_GOSSIP_TEXT_ID => DB_VENDOR_GOSSIP_TEXT,
        DB_TRAINER_GOSSIP_TEXT_ID => DB_TRAINER_GOSSIP_TEXT,
        _ => "",
    };
    let text = if matches!(
        text_id,
        DB_VENDOR_GOSSIP_TEXT_ID | DB_TRAINER_GOSSIP_TEXT_ID
    ) {
        text.to_string()
    } else {
        wow_db::get_npc_text_primary_query(world_db_pool, text_id)
            .await?
            .unwrap_or_else(|| "Greetings $N".to_string())
    };
    let response = build_npc_text_update(text_id, text.as_str());
    send_packet(
        stream,
        WorldOpcode::SmsgNpcTextUpdate as u16,
        &response,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn handle_list_inventory(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    request: wow_proto::ListInventoryRequest,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let guid = ObjectGuid::from_raw(request.raw_guid);
    let response = if guid.is_creature() {
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
    send_packet(
        stream,
        WorldOpcode::SmsgListInventory as u16,
        &response,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn handle_buy_item(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    request: wow_proto::BuyItemRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.character.active_character else {
        warn!("Ignoring vendor buy before character login");
        return Ok(());
    };
    let character_guid = character.guid;
    let buy = BuyItemRequest::from(request);
    let vendor_item = vendor_buy_item(world_db_pool, buy).await?;
    let Some(vendor_item) = vendor_item else {
        warn!(
            item = buy.item,
            vendor = format_args!("0x{:016X}", buy.vendor_guid.raw()),
            "Ignoring unsupported vendor buy request"
        );
        return Ok(());
    };
    let Some(template) = wow_db::get_item_template_query(world_db_pool, buy.item).await? else {
        warn!(
            item = buy.item,
            vendor = format_args!("0x{:016X}", buy.vendor_guid.raw()),
            "Ignoring vendor buy for missing item template"
        );
        return Ok(());
    };
    let count = buy.count.max(1);
    let total_count = vendor_item.buy_count.max(1).saturating_mul(count as u32);
    let equipped_bags = load_equipped_bag_infos(world_db_pool, &session.inventory.items).await?;
    let Some(store_plan) = plan_store_item(
        &session.inventory.items,
        &template,
        total_count,
        &equipped_bags,
        None,
        None,
    ) else {
        send_inventory_change_failure(stream, EQUIP_ERR_INVENTORY_FULL, None, None, header_crypto)
            .await?;
        return Ok(());
    };

    let price = vendor_item.price.saturating_mul(count as u32);
    let money = if price == 0 {
        None
    } else {
        match wow_db::spend_character_money(character_db_pool, character_guid, price).await? {
            Some(money) => Some(money),
            None => {
                return send_packet(
                    stream,
                    WorldOpcode::SmsgBuyFailed as u16,
                    &build_buy_failed_body(buy.vendor_guid, buy.item, BUY_ERR_NOT_ENOUGHT_MONEY),
                    Some(header_crypto),
                )
                .await;
            }
        }
    };
    let random_properties = generate_item_instance_random_properties(
        world_db_pool,
        &session.movement.db_creature_navigation.world_data_files,
        buy.item,
    )
    .await?;
    for slot in &store_plan {
        if let Some(item_guid) = slot.existing_item {
            let existing_count = session
                .inventory
                .items
                .iter()
                .find(|item| item.item == item_guid)
                .map(|item| item.count)
                .unwrap_or(0);
            wow_db::update_character_inventory_item_count(
                character_db_pool,
                character_guid,
                item_guid,
                existing_count.saturating_add(slot.count),
            )
            .await?;
        } else {
            wow_db::add_character_inventory_item_with_random_properties(
                character_db_pool,
                wow_db::AddCharacterInventoryItemRequest {
                    guid: character_guid,
                    bag: slot.bag as u32,
                    slot: slot.slot,
                    item_template: buy.item,
                    count: slot.count,
                    durability: 0,
                    random_properties: random_properties.as_ref(),
                },
            )
            .await?;
        }
    }
    session.inventory.items =
        wow_db::get_character_inventory_items(character_db_pool, character_guid).await?;

    send_packet(
        stream,
        WorldOpcode::SmsgBuyItem as u16,
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
    let mut update_blocks = Vec::new();
    for slot in &store_plan {
        if let Some(item_guid) = slot.existing_item {
            if let Some(item) = session
                .inventory
                .items
                .iter()
                .find(|item| item.item == item_guid)
            {
                update_blocks.push(build_item_stack_count_update_block(item.item, item.count)?);
            }
            continue;
        }
        if let Some(new_item) = session
            .inventory
            .items
            .iter()
            .find(|item| item.bag == slot.bag as u32 && item.slot == slot.slot)
        {
            let contained_guid =
                item_contained_guid(owner_guid, &session.inventory.items, new_item);
            update_blocks.push(build_item_create_update_block(
                owner_guid,
                contained_guid,
                new_item,
                (new_item.item_template == buy.item)
                    .then_some(container_slots)
                    .flatten(),
            )?);
            update_blocks.extend(build_inventory_position_update_blocks(
                character_guid,
                &session.inventory.items,
                slot.bag,
                slot.slot,
            )?);
        }
    }
    let body = build_update_object_body(&update_blocks);
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &body,
        Some(&mut *header_crypto),
    )
    .await?;
    if let Some(money) = money {
        send_packet(
            stream,
            WorldOpcode::SmsgUpdateObject as u16,
            &build_player_money_update_body(character_guid, money)?,
            Some(header_crypto),
        )
        .await?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct BuyItemRequest {
    pub(in crate::world) vendor_guid: ObjectGuid,
    pub(in crate::world) item: u32,
    pub(in crate::world) count: u8,
}

impl From<wow_proto::BuyItemRequest> for BuyItemRequest {
    fn from(request: wow_proto::BuyItemRequest) -> Self {
        Self {
            vendor_guid: ObjectGuid::from_raw(request.vendor_raw_guid),
            item: request.item,
            count: request.count,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct SellItemRequest {
    pub(in crate::world) vendor_guid: ObjectGuid,
    pub(in crate::world) item_guid: ObjectGuid,
    pub(in crate::world) count: u8,
}

impl From<wow_proto::SellItemRequest> for SellItemRequest {
    fn from(request: wow_proto::SellItemRequest) -> Self {
        Self {
            vendor_guid: ObjectGuid::from_raw(request.vendor_raw_guid),
            item_guid: ObjectGuid::from_raw(request.item_raw_guid),
            count: request.count,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct BuybackItemRequest {
    pub(in crate::world) vendor_guid: ObjectGuid,
    pub(in crate::world) slot: u8,
}

impl From<wow_proto::BuybackItemRequest> for BuybackItemRequest {
    fn from(request: wow_proto::BuybackItemRequest) -> Self {
        Self {
            vendor_guid: ObjectGuid::from_raw(request.vendor_raw_guid),
            slot: request.slot,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct VendorBuyItem {
    pub(in crate::world) slot: u32,
    pub(in crate::world) container_slots: u32,
    pub(in crate::world) buy_count: u32,
    pub(in crate::world) price: u32,
}

pub(in crate::world) async fn vendor_buy_item(
    world_db_pool: &MySqlPool,
    buy: BuyItemRequest,
) -> anyhow::Result<Option<VendorBuyItem>> {
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

pub(in crate::world) fn vendor_buyback_slot_index(slot: u8) -> Option<usize> {
    (BUYBACK_SLOT_START..BUYBACK_SLOT_END)
        .contains(&slot)
        .then_some((slot - BUYBACK_SLOT_START) as usize)
}

pub(in crate::world) fn next_buyback_slot(session: &WorldSessionState) -> u8 {
    if (BUYBACK_SLOT_START..BUYBACK_SLOT_END).contains(&session.inventory.next_buyback_slot) {
        session.inventory.next_buyback_slot
    } else {
        BUYBACK_SLOT_START
    }
}

pub(in crate::world) fn advance_buyback_slot(session: &mut WorldSessionState, used_slot: u8) {
    session.inventory.next_buyback_slot = if used_slot < BUYBACK_SLOT_END - 1 {
        used_slot + 1
    } else {
        BUYBACK_SLOT_START
    };
}

pub(in crate::world) fn build_buyback_slot_update_body(
    character_guid: u32,
    entry: Option<BuybackItem>,
    slot: u8,
) -> anyhow::Result<Vec<u8>> {
    let Some(index) = vendor_buyback_slot_index(slot) else {
        anyhow::bail!("invalid buyback slot {slot}");
    };
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player_guid)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    let item_guid = entry
        .map(|entry| ObjectGuid::new(HighGuid::Item, 0, entry.item).raw())
        .unwrap_or(0);
    set_update_value(
        &mut values,
        PLAYER_FIELD_VENDORBUYBACK_SLOT_1 + index * 2,
        item_guid as u32,
    )?;
    set_update_value(
        &mut values,
        PLAYER_FIELD_VENDORBUYBACK_SLOT_1 + index * 2 + 1,
        (item_guid >> 32) as u32,
    )?;
    set_update_value(
        &mut values,
        PLAYER_FIELD_BUYBACK_PRICE_1 + index,
        entry.map(|entry| entry.price).unwrap_or(0),
    )?;
    set_update_value(
        &mut values,
        PLAYER_FIELD_BUYBACK_TIMESTAMP_1 + index,
        entry.map(|entry| entry.timestamp).unwrap_or(0),
    )?;
    write_update_values(&mut block, &values)?;
    Ok(build_update_object_body(&[block]))
}

pub(in crate::world) async fn remove_existing_buyback_slot(
    character_db_pool: &MySqlPool,
    character_guid: u32,
    session: &mut WorldSessionState,
    slot: u8,
) -> anyhow::Result<()> {
    let _ = wow_db::destroy_character_inventory_item_count(
        character_db_pool,
        character_guid,
        INVENTORY_SLOT_BAG_0 as u32,
        slot,
        0,
    )
    .await?;
    session
        .inventory
        .buyback_items
        .retain(|entry| entry.slot != slot);
    Ok(())
}

pub(in crate::world) fn push_buyback_entry(
    session: &mut WorldSessionState,
    slot: u8,
    item: u32,
    price: u32,
) -> BuybackItem {
    let timestamp = 30 * 3600;
    session
        .inventory
        .buyback_items
        .retain(|entry| entry.slot != slot);
    let entry = BuybackItem {
        slot,
        item,
        price,
        timestamp,
    };
    session.inventory.buyback_items.push(entry);
    advance_buyback_slot(session, slot);
    entry
}

pub(in crate::world) async fn handle_sell_item(
    stream: &mut WorldPacketSink,
    deps: QuestMutationDeps<'_>,
    request: wow_proto::SellItemRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let character_db_pool = deps.character_db_pool;
    let world_db_pool = deps.world_db_pool;
    let Some(character) = &session.character.active_character else {
        warn!("Ignoring vendor sell before character login");
        return Ok(());
    };
    let character_guid = character.guid;
    let request = SellItemRequest::from(request);
    let vendor_valid = if request.vendor_guid.is_creature() {
        !wow_db::get_vendor_items(world_db_pool, request.vendor_guid.entry())
            .await?
            .is_empty()
    } else {
        false
    };
    if !vendor_valid {
        return send_packet(
            stream,
            WorldOpcode::SmsgSellItem as u16,
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
        .items
        .iter()
        .find(|item| item.item == request.item_guid.counter())
        .cloned()
    else {
        return Ok(());
    };
    let Some(template) =
        wow_db::get_item_template_query(world_db_pool, source_item.item_template).await?
    else {
        return send_packet(
            stream,
            WorldOpcode::SmsgSellItem as u16,
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
                .items
                .iter()
                .any(|item| item.bag == source_item.slot as u32))
    {
        return send_packet(
            stream,
            WorldOpcode::SmsgSellItem as u16,
            &build_sell_item_error_body(
                request.vendor_guid,
                request.item_guid,
                SELL_ERR_CANT_SELL_ITEM,
            ),
            Some(header_crypto),
        )
        .await;
    }

    let buyback_slot = next_buyback_slot(session);
    remove_existing_buyback_slot(character_db_pool, character_guid, session, buyback_slot).await?;
    let buyback_item = if count < source_item.count {
        let Some(split) = wow_db::split_character_inventory_item(
            character_db_pool,
            character_guid,
            source_item.bag,
            source_item.slot,
            INVENTORY_SLOT_BAG_0 as u32,
            buyback_slot,
            count,
        )
        .await?
        else {
            return Ok(());
        };
        split.new_item
    } else if wow_db::move_character_inventory_item_to_slot(
        character_db_pool,
        character_guid,
        source_item.item,
        INVENTORY_SLOT_BAG_0 as u32,
        buyback_slot,
    )
    .await?
    {
        source_item.item
    } else {
        return Ok(());
    };
    let money = wow_db::add_character_money(
        character_db_pool,
        character_guid,
        template.sell_price.saturating_mul(count),
    )
    .await?;
    session.inventory.items =
        wow_db::get_character_inventory_items(character_db_pool, character_guid).await?;
    let buyback_entry = push_buyback_entry(
        session,
        buyback_slot,
        buyback_item,
        template.sell_price.saturating_mul(count),
    );
    let owner_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);

    let mut update_blocks = Vec::new();
    if count < source_item.count {
        if let Some(item) = session
            .inventory
            .items
            .iter()
            .find(|item| item.item == source_item.item)
        {
            update_blocks.push(build_item_stack_count_update_block(item.item, item.count)?);
        }
        if let Some(item) = session
            .inventory
            .items
            .iter()
            .find(|item| item.item == buyback_item)
        {
            update_blocks.push(build_item_create_update_block(
                owner_guid, owner_guid, item, None,
            )?);
        }
    } else {
        update_blocks.extend(build_inventory_position_update_blocks(
            character_guid,
            &session.inventory.items,
            source_item.bag as u8,
            source_item.slot,
        )?);
        if let Some(item) = session
            .inventory
            .items
            .iter()
            .find(|item| item.item == buyback_item)
        {
            update_blocks.push(build_item_contained_update_block(
                owner_guid,
                &session.inventory.items,
                item,
            )?);
        }
    }
    if !update_blocks.is_empty() {
        send_packet(
            stream,
            WorldOpcode::SmsgUpdateObject as u16,
            &build_update_object_body(&update_blocks),
            Some(&mut *header_crypto),
        )
        .await?;
    }
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &build_buyback_slot_update_body(character_guid, Some(buyback_entry), buyback_slot)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
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

pub(in crate::world) async fn handle_buyback_item(
    stream: &mut WorldPacketSink,
    deps: QuestMutationDeps<'_>,
    request: wow_proto::BuybackItemRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let character_db_pool = deps.character_db_pool;
    let world_db_pool = deps.world_db_pool;
    let Some(character) = &session.character.active_character else {
        warn!("Ignoring vendor buyback before character login");
        return Ok(());
    };
    let character_guid = character.guid;
    let request = BuybackItemRequest::from(request);
    let vendor_valid = if request.vendor_guid.is_creature() {
        !wow_db::get_vendor_items(world_db_pool, request.vendor_guid.entry())
            .await?
            .is_empty()
    } else {
        false
    };
    if !vendor_valid {
        return send_packet(
            stream,
            WorldOpcode::SmsgSellItem as u16,
            &build_sell_item_error_body(
                request.vendor_guid,
                ObjectGuid::from_raw(0),
                SELL_ERR_CANT_FIND_VENDOR,
            ),
            Some(header_crypto),
        )
        .await;
    }

    let Some(entry_index) = session
        .inventory
        .buyback_items
        .iter()
        .position(|entry| entry.slot == request.slot)
    else {
        return send_packet(
            stream,
            WorldOpcode::SmsgBuyFailed as u16,
            &build_buy_failed_body(request.vendor_guid, 0, BUY_ERR_CANT_FIND_ITEM),
            Some(header_crypto),
        )
        .await;
    };
    let entry = session.inventory.buyback_items[entry_index];
    let Some(source_item) = session
        .inventory
        .items
        .iter()
        .find(|item| {
            item.item == entry.item
                && item.bag == INVENTORY_SLOT_BAG_0 as u32
                && item.slot == request.slot
        })
        .cloned()
    else {
        session.inventory.buyback_items.remove(entry_index);
        return send_packet(
            stream,
            WorldOpcode::SmsgBuyFailed as u16,
            &build_buy_failed_body(request.vendor_guid, 0, BUY_ERR_CANT_FIND_ITEM),
            Some(header_crypto),
        )
        .await;
    };
    let Some(template) =
        wow_db::get_item_template_query(world_db_pool, source_item.item_template).await?
    else {
        return send_packet(
            stream,
            WorldOpcode::SmsgBuyFailed as u16,
            &build_buy_failed_body(
                request.vendor_guid,
                source_item.item_template,
                BUY_ERR_CANT_FIND_ITEM,
            ),
            Some(header_crypto),
        )
        .await;
    };
    let equipped_bags = load_equipped_bag_infos(world_db_pool, &session.inventory.items).await?;
    let Some(store_plan) = plan_store_item(
        &session.inventory.items,
        &template,
        source_item.count,
        &equipped_bags,
        None,
        Some(source_item.item),
    ) else {
        return send_inventory_change_failure(
            stream,
            EQUIP_ERR_INVENTORY_FULL,
            Some(ObjectGuid::new(HighGuid::Item, 0, source_item.item)),
            None,
            header_crypto,
        )
        .await;
    };
    let Some(money) =
        wow_db::spend_character_money(character_db_pool, character_guid, entry.price).await?
    else {
        return send_packet(
            stream,
            WorldOpcode::SmsgBuyFailed as u16,
            &build_buy_failed_body(
                request.vendor_guid,
                source_item.item_template,
                BUY_ERR_NOT_ENOUGHT_MONEY,
            ),
            Some(header_crypto),
        )
        .await;
    };

    for slot in &store_plan {
        if let Some(item_guid) = slot.existing_item {
            let existing_count = session
                .inventory
                .items
                .iter()
                .find(|item| item.item == item_guid)
                .map(|item| item.count)
                .unwrap_or(0);
            wow_db::update_character_inventory_item_count(
                character_db_pool,
                character_guid,
                item_guid,
                existing_count.saturating_add(slot.count),
            )
            .await?;
            let _ = wow_db::destroy_character_inventory_item_count(
                character_db_pool,
                character_guid,
                INVENTORY_SLOT_BAG_0 as u32,
                request.slot,
                0,
            )
            .await?;
        } else {
            wow_db::move_character_inventory_item_to_slot(
                character_db_pool,
                character_guid,
                source_item.item,
                slot.bag as u32,
                slot.slot,
            )
            .await?;
        }
    }
    session.inventory.buyback_items.remove(entry_index);
    session.inventory.items =
        wow_db::get_character_inventory_items(character_db_pool, character_guid).await?;

    let owner_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let mut update_blocks = Vec::new();
    for slot in &store_plan {
        if let Some(item_guid) = slot.existing_item {
            if let Some(item) = session
                .inventory
                .items
                .iter()
                .find(|item| item.item == item_guid)
            {
                update_blocks.push(build_item_stack_count_update_block(item.item, item.count)?);
            }
            continue;
        }
        if let Some(item) = session
            .inventory
            .items
            .iter()
            .find(|item| item.bag == slot.bag as u32 && item.slot == slot.slot)
        {
            let contained_guid = item_contained_guid(owner_guid, &session.inventory.items, item);
            let container_slots = if template.container_slots > 0 {
                Some(template.container_slots)
            } else {
                None
            };
            update_blocks.push(build_item_create_update_block(
                owner_guid,
                contained_guid,
                item,
                container_slots,
            )?);
            update_blocks.extend(build_inventory_position_update_blocks(
                character_guid,
                &session.inventory.items,
                slot.bag,
                slot.slot,
            )?);
        }
    }
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &build_update_object_body(&update_blocks),
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &build_buyback_slot_update_body(character_guid, None, request.slot)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &build_player_money_update_body(character_guid, money)?,
        Some(&mut *header_crypto),
    )
    .await?;

    complete_inventory_item_quests(
        stream,
        character_db_pool,
        deps.object_mgr,
        world_db_pool,
        session,
        character_guid,
        header_crypto,
    )
    .await
}
