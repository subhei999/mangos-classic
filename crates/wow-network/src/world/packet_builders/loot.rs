use super::*;
use wow_proto::world::WorldOpcode;
use wow_proto::{
    LootItemResponse, ServerWorldPacket, SmsgLootErrorResponse, SmsgLootMasterListResponse,
    SmsgLootReleaseResponse, SmsgLootResponse,
};

// CMaNGOS reference: src/game/Handlers/LootHandler.cpp loot packet builders.

impl From<CreatureLootQuery> for DbCreatureLootRuntime {
    fn from(loot: CreatureLootQuery) -> Self {
        Self {
            slot: 0,

            item: loot.item,

            count: loot.max_count.max(loot.min_count).max(1),

            display_id: loot.display_id,

            quality: 0,

            free_for_all: false,

            quest_drop: loot.is_quest_drop(),
        }
    }
}

pub(in crate::world) struct LootAutostoreContext<'a> {
    pub(in crate::world) stream: &'a mut WorldPacketSink,

    pub(in crate::world) character_db_pool: &'a MySqlPool,

    pub(in crate::world) object_mgr: &'a ObjectMgr,

    pub(in crate::world) world_db_pool: &'a MySqlPool,

    pub(in crate::world) session: &'a mut WorldSessionState,

    pub(in crate::world) header_crypto: &'a mut HeaderCrypto,

    pub(in crate::world) character_guid: u32,
}

pub(in crate::world) async fn autostore_loot_item(
    context: LootAutostoreContext<'_>,

    _creature_guid: u64,

    loot: DbCreatureLootRuntime,

    loot_slot: u8,
) -> anyhow::Result<bool> {
    let LootAutostoreContext {
        stream,

        character_db_pool,

        object_mgr,

        world_db_pool,

        session,

        header_crypto,

        character_guid,
    } = context;

    let Some(template) = wow_db::get_item_template_query(world_db_pool, loot.item).await? else {
        return Ok(false);
    };
    let bag_model =
        InventoryBagModel::load_inventory(world_db_pool, &session.inventory.items).await?;
    let Some(store_plan) = bag_model.plan_store_item(
        InventoryStorageScope::Inventory,
        &session.inventory.items,
        &template,
        loot.count,
        None,
        None,
    ) else {
        send_inventory_change_failure(stream, EQUIP_ERR_INVENTORY_FULL, None, None, header_crypto)
            .await?;

        return Ok(false);
    };
    let mut update_blocks = Vec::new();
    let mut pushed_item = None;

    let random_properties = generate_item_instance_random_properties(
        world_db_pool,
        &session.movement.db_creature_navigation.world_data_files,
        loot.item,
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
                    item_template: loot.item,
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
    for slot in &store_plan {
        if let Some(item_guid) = slot.existing_item {
            if let Some(item) = session
                .inventory
                .items
                .iter()
                .find(|item| item.item == item_guid)
            {
                update_blocks.push(build_item_stack_count_update_block(item.item, item.count)?);
                pushed_item = Some(item.clone());
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
                (template.container_slots > 0).then_some(template.container_slots),
            )?);
            pushed_item = Some(new_item.clone());
        }
    }

    send_packet(
        stream,
        WorldOpcode::SmsgLootRemoved as u16,
        &[loot_slot],
        Some(&mut *header_crypto),
    )
    .await?;

    if let Some(item) = pushed_item.as_ref() {
        let body =
            build_item_push_result_body(character_guid, item, loot.count, false, false, true);
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
    complete_inventory_item_quests(
        stream,
        character_db_pool,
        object_mgr,
        world_db_pool,
        session,
        character_guid,
        header_crypto,
    )
    .await?;

    Ok(true)
}

pub(in crate::world) fn build_db_creature_loot_response_body_for_player(
    target: ObjectGuid,
    creature: &DbCreatureRuntime,
    loot_method: Option<(u8, u8, u32)>,
    character_guid: u32,
) -> Vec<u8> {
    let item_count = creature.loot_items.len().min(u8::MAX as usize) as u8;

    let money = if creature.loot_money_available {
        creature.loot_money()
    } else {
        0
    };

    SmsgLootResponse {
        target,
        loot_type: CLIENT_LOOT_CORPSE,
        money,
        items: creature
            .loot_items
            .iter()
            .take(item_count as usize)
            .filter_map(|loot| {
                db_creature_loot_slot_type_for_player(creature, loot_method, character_guid, loot)
                    .map(|slot_type| LootItemResponse {
                        slot: loot.slot,
                        item: loot.item,
                        count: loot.count,
                        display_id: loot.display_id,
                        random_suffix: 0,
                        random_property: 0,
                        slot_type,
                    })
            })
            .collect(),
    }
    .body()
}

pub(in crate::world) fn db_creature_loot_slot_type_for_player(
    creature: &DbCreatureRuntime,
    loot_method: Option<(u8, u8, u32)>,
    character_guid: u32,
    loot: &DbCreatureLootRuntime,
) -> Option<u8> {
    const LOOT_SLOT_VIEW: u8 = 1;
    const LOOT_SLOT_MASTER: u8 = 2;
    let Some((method, threshold, master_looter)) = loot_method else {
        return Some(LOOT_SLOT_NORMAL);
    };
    if loot.free_for_all {
        return Some(LOOT_SLOT_NORMAL);
    }
    let under_threshold = loot.quest_drop || loot.quality < threshold;
    match method {
        0 => Some(LOOT_SLOT_NORMAL), // free for all
        1 | 3 | 4 => {
            if !under_threshold {
                if creature.loot_roll_released_slots.contains(&loot.slot) {
                    Some(LOOT_SLOT_NORMAL)
                } else {
                    Some(LOOT_SLOT_VIEW)
                }
            } else if creature.loot_current_looter == Some(character_guid)
                || creature.loot_current_looter_pass_slots.contains(&loot.slot)
                || creature.loot_roll_released_slots.contains(&loot.slot)
            {
                Some(LOOT_SLOT_NORMAL)
            } else {
                None
            }
        }
        2 => {
            if under_threshold {
                (creature.loot_current_looter == Some(character_guid)
                    || creature.loot_current_looter_pass_slots.contains(&loot.slot)
                    || creature.loot_roll_released_slots.contains(&loot.slot))
                .then_some(LOOT_SLOT_NORMAL)
            } else if character_guid == master_looter {
                Some(LOOT_SLOT_MASTER)
            } else {
                Some(LOOT_SLOT_VIEW)
            }
        }
        _ => Some(LOOT_SLOT_NORMAL),
    }
}

pub(in crate::world) fn build_gameobject_loot_response_body(
    target: ObjectGuid,
    loot_items: &[DbCreatureLootRuntime],
) -> Vec<u8> {
    let item_count = loot_items.len().min(u8::MAX as usize) as u8;
    SmsgLootResponse {
        target,
        loot_type: CLIENT_LOOT_CORPSE,
        money: 0,
        items: loot_items
            .iter()
            .take(item_count as usize)
            .map(|loot| LootItemResponse {
                slot: loot.slot,
                item: loot.item,
                count: loot.count,
                display_id: loot.display_id,
                random_suffix: 0,
                random_property: 0,
                slot_type: LOOT_SLOT_NORMAL,
            })
            .collect(),
    }
    .body()
}

pub(in crate::world) fn build_loot_master_list_body(members: &[u32]) -> Vec<u8> {
    SmsgLootMasterListResponse {
        members: members
            .iter()
            .take(u8::MAX as usize)
            .map(|member| ObjectGuid::new(HighGuid::Player, 0, *member))
            .collect(),
    }
    .body()
}

pub(in crate::world) fn build_loot_error_response_body(target: ObjectGuid, error: u8) -> Vec<u8> {
    SmsgLootErrorResponse {
        target,
        loot_type: 0,
        error,
    }
    .body()
}

pub(in crate::world) fn build_loot_release_response_body(
    target: ObjectGuid,
    released: bool,
) -> Vec<u8> {
    SmsgLootReleaseResponse { target, released }.body()
}
