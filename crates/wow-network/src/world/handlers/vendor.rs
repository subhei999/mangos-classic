use super::*;
use wow_proto::world::WorldOpcode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::world) struct VendorStockKey {
    pub(in crate::world) vendor_guid_raw: u64,
    pub(in crate::world) item: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct VendorStockEntry {
    pub(in crate::world) count: u32,
    pub(in crate::world) last_increment_time: u64,
}

#[derive(Clone, Default)]
pub(in crate::world) struct VendorStockState {
    entries: Arc<Mutex<HashMap<VendorStockKey, VendorStockEntry>>>,
}

impl VendorStockState {
    pub(in crate::world) async fn current_count(
        &self,
        vendor_guid: ObjectGuid,
        vendor_item: &wow_db::VendorItemQuery,
        now_secs: u64,
    ) -> u32 {
        let mut entries = self.entries.lock().await;
        let key = VendorStockKey::new(vendor_guid, vendor_item.item);
        let current = vendor_item_current_count(vendor_item, entries.get(&key).copied(), now_secs);
        update_vendor_stock_entry(&mut entries, key, current.updated_entry);
        current.count
    }

    pub(in crate::world) async fn list_items(
        &self,
        vendor_guid: ObjectGuid,
        vendor_items: &[wow_db::VendorItemQuery],
        now_secs: u64,
    ) -> Vec<VendorListItem> {
        let mut entries = self.entries.lock().await;
        vendor_items
            .iter()
            .map(|item| {
                let key = VendorStockKey::new(vendor_guid, item.item);
                let current = vendor_item_current_count(item, entries.get(&key).copied(), now_secs);
                update_vendor_stock_entry(&mut entries, key, current.updated_entry);
                VendorListItem::from_vendor_item(item, current.count)
            })
            .collect()
    }

    pub(in crate::world) async fn consume_item(
        &self,
        vendor_guid: ObjectGuid,
        vendor_item: &wow_db::VendorItemQuery,
        used_count: u32,
        now_secs: u64,
    ) -> Option<u32> {
        let mut entries = self.entries.lock().await;
        let key = VendorStockKey::new(vendor_guid, vendor_item.item);
        let outcome = vendor_item_consume_count(
            vendor_item,
            entries.get(&key).copied(),
            used_count,
            now_secs,
        )?;
        update_vendor_stock_entry(&mut entries, key, outcome.updated_entry);
        Some(outcome.count)
    }
}

impl VendorStockKey {
    fn new(vendor_guid: ObjectGuid, item: u32) -> Self {
        Self {
            vendor_guid_raw: vendor_guid.raw(),
            item,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct VendorStockCount {
    pub(in crate::world) count: u32,
    pub(in crate::world) updated_entry: Option<VendorStockEntry>,
}

pub(in crate::world) fn vendor_item_current_count(
    vendor_item: &wow_db::VendorItemQuery,
    entry: Option<VendorStockEntry>,
    now_secs: u64,
) -> VendorStockCount {
    if vendor_item.max_count == 0 {
        return VendorStockCount {
            count: 0,
            updated_entry: None,
        };
    }

    let Some(mut entry) = entry else {
        return VendorStockCount {
            count: vendor_item.max_count,
            updated_entry: None,
        };
    };

    if let Some(replenished_count) = vendor_item_replenished_count(vendor_item, entry, now_secs) {
        if replenished_count >= vendor_item.max_count {
            return VendorStockCount {
                count: vendor_item.max_count,
                updated_entry: None,
            };
        }
        entry.count = replenished_count;
        entry.last_increment_time = now_secs;
    }

    VendorStockCount {
        count: entry.count,
        updated_entry: Some(entry),
    }
}

pub(in crate::world) fn vendor_item_consume_count(
    vendor_item: &wow_db::VendorItemQuery,
    entry: Option<VendorStockEntry>,
    used_count: u32,
    now_secs: u64,
) -> Option<VendorStockCount> {
    if vendor_item.max_count == 0 {
        return Some(VendorStockCount {
            count: 0,
            updated_entry: None,
        });
    }

    let mut current = vendor_item_current_count(vendor_item, entry, now_secs);
    if current.count < used_count {
        return None;
    }

    let new_count = current.count.saturating_sub(used_count);
    current.updated_entry = Some(VendorStockEntry {
        count: new_count,
        last_increment_time: now_secs,
    });
    current.count = new_count;
    Some(current)
}

fn vendor_item_replenished_count(
    vendor_item: &wow_db::VendorItemQuery,
    entry: VendorStockEntry,
    now_secs: u64,
) -> Option<u32> {
    if vendor_item.incr_time == 0
        || entry
            .last_increment_time
            .saturating_add(vendor_item.incr_time as u64)
            > now_secs
    {
        return None;
    }
    let intervals = (now_secs - entry.last_increment_time) / vendor_item.incr_time as u64;
    Some(
        entry
            .count
            .saturating_add((intervals as u32).saturating_mul(vendor_item.buy_count.max(1))),
    )
}

fn update_vendor_stock_entry(
    entries: &mut HashMap<VendorStockKey, VendorStockEntry>,
    key: VendorStockKey,
    entry: Option<VendorStockEntry>,
) {
    if let Some(entry) = entry {
        entries.insert(key, entry);
    } else {
        entries.remove(&key);
    }
}

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
    vendor_stock: &VendorStockState,
    request: wow_proto::ListInventoryRequest,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let guid = ObjectGuid::from_raw(request.raw_guid);
    let response = if guid.is_creature() {
        let vendor_items = wow_db::get_vendor_items(world_db_pool, guid.entry()).await?;
        let list_items = vendor_stock
            .list_items(guid, &vendor_items, current_unix_time_secs())
            .await;
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
    vendor_stock: &VendorStockState,
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
    let now_secs = current_unix_time_secs();
    if vendor_item.query.max_count != 0
        && vendor_stock
            .current_count(buy.vendor_guid, &vendor_item.query, now_secs)
            .await
            < total_count
    {
        return send_packet(
            stream,
            WorldOpcode::SmsgBuyFailed as u16,
            &build_buy_failed_body(buy.vendor_guid, buy.item, BUY_ERR_ITEM_ALREADY_SOLD),
            Some(header_crypto),
        )
        .await;
    }
    let bag_model =
        InventoryBagModel::load_inventory(world_db_pool, &session.inventory.items).await?;
    let Some(store_plan) = bag_model.plan_store_item(
        InventoryStorageScope::Inventory,
        &session.inventory.items,
        &template,
        total_count,
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
                    initial_flags: item_binding_flags_on_pickup(&template),
                    random_properties: random_properties.as_ref(),
                },
            )
            .await?;
        }
    }
    session.inventory.items =
        wow_db::get_character_inventory_items(character_db_pool, character_guid).await?;
    let remaining_count = if vendor_item.query.max_count == 0 {
        u32::MAX
    } else {
        vendor_stock
            .consume_item(buy.vendor_guid, &vendor_item.query, total_count, now_secs)
            .await
            .unwrap_or(0)
    };

    send_packet(
        stream,
        WorldOpcode::SmsgBuyItem as u16,
        &build_buy_item_body(buy.vendor_guid, vendor_item.slot, remaining_count, count),
        Some(&mut *header_crypto),
    )
    .await?;
    let container_slots = if vendor_item.container_slots > 0 {
        Some(vendor_item.container_slots)
    } else {
        None
    };
    let mut update_blocks = Vec::new();
    let mut push_results = Vec::new();
    for slot in &store_plan {
        if let Some(item_guid) = slot.existing_item {
            if let Some(item) = session
                .inventory
                .items
                .iter()
                .find(|item| item.item == item_guid)
            {
                update_blocks.push(build_item_stack_count_update_block(item.item, item.count)?);
                push_results.push(build_item_push_result_body(
                    character_guid,
                    item,
                    slot.count,
                    true,
                    false,
                    true,
                ));
            }
            continue;
        }
        if let Some(new_item) = session
            .inventory
            .items
            .iter()
            .find(|item| item.bag == slot.bag as u32 && item.slot == slot.slot)
        {
            update_blocks.extend(build_stored_item_create_update_blocks(
                character_guid,
                &session.inventory.items,
                new_item,
                (new_item.item_template == buy.item)
                    .then_some(container_slots)
                    .flatten(),
            )?);
            push_results.push(build_item_push_result_body(
                character_guid,
                new_item,
                slot.count,
                true,
                false,
                true,
            ));
        }
    }
    for body in push_results {
        send_packet(
            stream,
            WorldOpcode::SmsgItemPushResult as u16,
            &body,
            Some(&mut *header_crypto),
        )
        .await?;
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

pub(in crate::world) async fn handle_buy_item_in_slot(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    vendor_stock: &VendorStockState,
    request: wow_proto::BuyItemInSlotRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.character.active_character else {
        warn!("Ignoring vendor buy-in-slot before character login");
        return Ok(());
    };
    let character_guid = character.guid;
    let request = BuyItemInSlotRequest::from(request);
    let buy = BuyItemRequest {
        vendor_guid: request.vendor_guid,
        item: request.item,
        count: request.count,
    };
    let Some((dst_bag, dst_slot)) =
        resolve_buy_item_in_slot_destination(character_guid, request, &session.inventory.items)
    else {
        return Ok(());
    };
    let vendor_item = vendor_buy_item(world_db_pool, buy).await?;
    let Some(vendor_item) = vendor_item else {
        return Ok(());
    };
    let Some(template) = wow_db::get_item_template_query(world_db_pool, buy.item).await? else {
        return Ok(());
    };
    let count = buy.count.max(1);
    let total_count = vendor_item.buy_count.max(1).saturating_mul(count as u32);
    let now_secs = current_unix_time_secs();
    if vendor_item.query.max_count != 0
        && vendor_stock
            .current_count(buy.vendor_guid, &vendor_item.query, now_secs)
            .await
            < total_count
    {
        return send_packet(
            stream,
            WorldOpcode::SmsgBuyFailed as u16,
            &build_buy_failed_body(buy.vendor_guid, buy.item, BUY_ERR_ITEM_ALREADY_SOLD),
            Some(header_crypto),
        )
        .await;
    }
    let bag_model =
        InventoryBagModel::load_inventory(world_db_pool, &session.inventory.items).await?;
    let Some(store_plan) = plan_store_vendor_item_in_slot(
        &session.inventory.items,
        &template,
        total_count,
        &bag_model,
        dst_bag,
        dst_slot,
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
                    initial_flags: item_binding_flags_on_pickup(&template),
                    random_properties: random_properties.as_ref(),
                },
            )
            .await?;
        }
    }
    session.inventory.items =
        wow_db::get_character_inventory_items(character_db_pool, character_guid).await?;
    let remaining_count = if vendor_item.query.max_count == 0 {
        u32::MAX
    } else {
        vendor_stock
            .consume_item(buy.vendor_guid, &vendor_item.query, total_count, now_secs)
            .await
            .unwrap_or(0)
    };

    send_packet(
        stream,
        WorldOpcode::SmsgBuyItem as u16,
        &build_buy_item_body(buy.vendor_guid, vendor_item.slot, remaining_count, count),
        Some(&mut *header_crypto),
    )
    .await?;
    let container_slots = if vendor_item.container_slots > 0 {
        Some(vendor_item.container_slots)
    } else {
        None
    };
    let mut update_blocks = Vec::new();
    let mut push_results = Vec::new();
    for slot in &store_plan {
        if let Some(item_guid) = slot.existing_item {
            if let Some(item) = session
                .inventory
                .items
                .iter()
                .find(|item| item.item == item_guid)
            {
                update_blocks.push(build_item_stack_count_update_block(item.item, item.count)?);
                push_results.push(build_item_push_result_body(
                    character_guid,
                    item,
                    slot.count,
                    true,
                    false,
                    true,
                ));
            }
            continue;
        }
        if let Some(new_item) = session
            .inventory
            .items
            .iter()
            .find(|item| item.bag == slot.bag as u32 && item.slot == slot.slot)
        {
            update_blocks.extend(build_stored_item_create_update_blocks(
                character_guid,
                &session.inventory.items,
                new_item,
                (new_item.item_template == buy.item)
                    .then_some(container_slots)
                    .flatten(),
            )?);
            push_results.push(build_item_push_result_body(
                character_guid,
                new_item,
                slot.count,
                true,
                false,
                true,
            ));
        }
    }
    for body in push_results {
        send_packet(
            stream,
            WorldOpcode::SmsgItemPushResult as u16,
            &body,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &build_update_object_body(&update_blocks),
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
pub(in crate::world) struct BuyItemInSlotRequest {
    pub(in crate::world) vendor_guid: ObjectGuid,
    pub(in crate::world) item: u32,
    pub(in crate::world) bag_guid: ObjectGuid,
    pub(in crate::world) bag_slot: u8,
    pub(in crate::world) count: u8,
}

impl From<wow_proto::BuyItemInSlotRequest> for BuyItemInSlotRequest {
    fn from(request: wow_proto::BuyItemInSlotRequest) -> Self {
        Self {
            vendor_guid: ObjectGuid::from_raw(request.vendor_raw_guid),
            item: request.item,
            bag_guid: ObjectGuid::from_raw(request.bag_raw_guid),
            bag_slot: request.bag_slot,
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

pub(in crate::world) fn sell_item_container_contents_bag(
    source_item: &CharacterInventoryItem,
    template: &ItemTemplateQuery,
) -> Option<u8> {
    if template.container_slots == 0 {
        return None;
    }
    (source_item.bag == INVENTORY_SLOT_BAG_0 as u32 && is_bag_slot(source_item.slot))
        .then_some(source_item.slot)
}

pub(in crate::world) fn sell_item_is_non_empty_container(
    inventory: &[CharacterInventoryItem],
    source_item: &CharacterInventoryItem,
    template: &ItemTemplateQuery,
) -> bool {
    sell_item_container_contents_bag(source_item, template)
        .is_some_and(|bag| inventory.iter().any(|item| item.bag == bag as u32))
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

#[derive(Debug, Clone)]
pub(in crate::world) struct VendorBuyItem {
    pub(in crate::world) slot: u32,
    pub(in crate::world) container_slots: u32,
    pub(in crate::world) buy_count: u32,
    pub(in crate::world) price: u32,
    pub(in crate::world) query: wow_db::VendorItemQuery,
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
            query: item.clone(),
        }))
}

fn resolve_buy_item_in_slot_destination(
    character_guid: u32,
    request: BuyItemInSlotRequest,
    inventory: &[CharacterInventoryItem],
) -> Option<(u8, u8)> {
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    if request.bag_guid == player_guid {
        return is_backpack_item_slot(request.bag_slot)
            .then_some((INVENTORY_SLOT_BAG_0, request.bag_slot));
    }
    inventory
        .iter()
        .find(|item| {
            item.item == request.bag_guid.counter()
                && item.bag == INVENTORY_SLOT_BAG_0 as u32
                && is_bag_slot(item.slot)
        })
        .map(|item| (item.slot, request.bag_slot))
}

pub(in crate::world) fn plan_store_vendor_item_in_slot(
    inventory: &[CharacterInventoryItem],
    template: &ItemTemplateQuery,
    count: u32,
    bag_model: &InventoryBagModel,
    dst_bag: u8,
    dst_slot: u8,
) -> Option<Vec<StoreSlot>> {
    if count == 0
        || !bag_model.storage_position_exists(
            InventoryStorageScope::Inventory,
            InventoryPosition::new(dst_bag, dst_slot),
        )
        || !bag_model.bag_accepts_item(InventoryStorageScope::Inventory, dst_bag, template)
    {
        return None;
    }

    let max_stack = template.stackable.max(1);
    let existing = inventory
        .iter()
        .find(|item| item.bag == dst_bag as u32 && item.slot == dst_slot);
    if let Some(existing) = existing {
        if existing.item_template != template.entry || existing.count >= max_stack {
            return None;
        }
        let free = max_stack - existing.count;
        if count > free {
            return None;
        }
        return Some(vec![StoreSlot {
            bag: dst_bag,
            slot: dst_slot,
            count,
            existing_item: Some(existing.item),
        }]);
    }

    if count > max_stack {
        return None;
    }
    Some(vec![StoreSlot {
        bag: dst_bag,
        slot: dst_slot,
        count,
        existing_item: None,
    }])
}

pub(in crate::world) fn vendor_buyback_slot_index(slot: u8) -> Option<usize> {
    (BUYBACK_SLOT_START..BUYBACK_SLOT_END)
        .contains(&slot)
        .then_some((slot - BUYBACK_SLOT_START) as usize)
}

fn normalized_buyback_cursor(session: &WorldSessionState) -> u8 {
    if (BUYBACK_SLOT_START..BUYBACK_SLOT_END).contains(&session.inventory.next_buyback_slot) {
        session.inventory.next_buyback_slot
    } else {
        BUYBACK_SLOT_START
    }
}

fn buyback_slot_occupied(session: &WorldSessionState, slot: u8) -> bool {
    session
        .inventory
        .buyback_items
        .iter()
        .any(|entry| entry.slot == slot)
}

pub(in crate::world) fn next_buyback_slot(session: &WorldSessionState) -> u8 {
    let cursor = normalized_buyback_cursor(session);
    if !buyback_slot_occupied(session, cursor) {
        return cursor;
    }

    for slot in BUYBACK_SLOT_START..BUYBACK_SLOT_END {
        if !buyback_slot_occupied(session, slot) {
            return slot;
        }
    }

    session
        .inventory
        .buyback_items
        .iter()
        .min_by_key(|entry| entry.timestamp)
        .map(|entry| entry.slot)
        .unwrap_or(BUYBACK_SLOT_START)
}

pub(in crate::world) fn advance_buyback_slot(session: &mut WorldSessionState, used_slot: u8) {
    session.inventory.next_buyback_slot = if used_slot < BUYBACK_SLOT_END - 1 {
        used_slot + 1
    } else {
        BUYBACK_SLOT_END - 1
    };
}

fn refresh_buyback_cursor_after_slot_cleared(session: &mut WorldSessionState, slot: u8) {
    let cursor = normalized_buyback_cursor(session);
    if buyback_slot_occupied(session, cursor) {
        session.inventory.next_buyback_slot = slot;
    }
}

pub(in crate::world) fn remove_buyback_entry_from_session(
    session: &mut WorldSessionState,
    slot: u8,
) {
    session
        .inventory
        .buyback_items
        .retain(|entry| entry.slot != slot);
    refresh_buyback_cursor_after_slot_cleared(session, slot);
}

fn next_buyback_timestamp(session: &WorldSessionState) -> u32 {
    const BUYBACK_DURATION_SECS: u32 = 30 * 3600;
    session
        .inventory
        .buyback_items
        .iter()
        .map(|entry| entry.timestamp)
        .max()
        .map(|timestamp| timestamp.saturating_add(1))
        .unwrap_or(BUYBACK_DURATION_SECS)
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
    remove_buyback_entry_from_session(session, slot);
    Ok(())
}

pub(in crate::world) fn push_buyback_entry(
    session: &mut WorldSessionState,
    slot: u8,
    item: u32,
    price: u32,
) -> BuybackItem {
    let timestamp = next_buyback_timestamp(session);
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
        || sell_item_is_non_empty_container(&session.inventory.items, &source_item, &template)
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
        update_blocks.extend(build_inventory_positions_update_blocks(
            character_guid,
            &session.inventory.items,
            u8::try_from(source_item.bag)
                .ok()
                .map(|bag| InventoryPosition::new(bag, source_item.slot)),
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
        remove_buyback_entry_from_session(session, request.slot);
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
    let bag_model =
        InventoryBagModel::load_inventory(world_db_pool, &session.inventory.items).await?;
    let Some(store_plan) = bag_model.plan_store_item(
        InventoryStorageScope::Inventory,
        &session.inventory.items,
        &template,
        source_item.count,
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
    remove_buyback_entry_from_session(session, request.slot);
    session.inventory.items =
        wow_db::get_character_inventory_items(character_db_pool, character_guid).await?;

    let mut update_blocks = Vec::new();
    let mut moved_positions = Vec::new();
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
        moved_positions.push(InventoryPosition::new(slot.bag, slot.slot));
    }
    update_blocks.extend(build_inventory_positions_update_blocks(
        character_guid,
        &session.inventory.items,
        moved_positions,
    )?);
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
