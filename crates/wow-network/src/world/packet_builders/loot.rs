// CMaNGOS reference: src/game/Handlers/LootHandler.cpp loot packet builders.

fn build_combat_dummy_loot_response_body(session: &WorldSessionState) -> Vec<u8> {
    let item_count = u8::from(session.combat_dummy_loot_item_available);

    let mut body = Vec::with_capacity(14 + item_count as usize * 22);

    body.extend_from_slice(&rust_combat_dummy_guid().raw().to_le_bytes());

    body.push(CLIENT_LOOT_CORPSE);

    body.extend_from_slice(
        &(if session.combat_dummy_loot_money_available {
            RUST_COMBAT_DUMMY_LOOT_MONEY
        } else {
            0
        })
        .to_le_bytes(),
    );

    body.push(item_count);

    if session.combat_dummy_loot_item_available {
        body.push(0); // loot slot

        body.extend_from_slice(&RUST_COMBAT_DUMMY_LOOT_ITEM.to_le_bytes());

        body.extend_from_slice(&RUST_COMBAT_DUMMY_LOOT_ITEM_COUNT.to_le_bytes());

        body.extend_from_slice(&RUST_COMBAT_DUMMY_LOOT_ITEM_DISPLAY.to_le_bytes());

        body.extend_from_slice(&0u32.to_le_bytes()); // random suffix factor

        body.extend_from_slice(&0u32.to_le_bytes()); // random property id

        body.push(LOOT_SLOT_NORMAL);
    }

    body
}

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

struct LootAutostoreContext<'a> {
    stream: &'a mut WorldPacketSink,

    character_db_pool: &'a MySqlPool,

    object_mgr: &'a ObjectMgr,

    world_db_pool: &'a MySqlPool,

    session: &'a mut WorldSessionState,

    header_crypto: &'a mut HeaderCrypto,

    character_guid: u32,
}

async fn autostore_loot_item(
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

    let max_stack = wow_db::get_item_template_query(world_db_pool, loot.item)
        .await?
        .map(|template| template.stackable.max(1))
        .unwrap_or(1);

    let mut remaining_count = loot.count;

    let mut update_blocks = Vec::new();
    let mut pushed_item = None;

    if max_stack > 1 {
        if let Some(existing_stack) = session
            .inventory
            .iter()
            .filter(|item| {
                item.item_template == loot.item
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
                pushed_item = Some(CharacterInventoryItem {
                    count: merged_count,
                    ..existing_stack
                });
            }
        }
    }

    if remaining_count > 0 {
        let Some(dst_slot) = first_empty_backpack_slot(&session.inventory) else {
            send_inventory_change_failure(
                stream,
                EQUIP_ERR_INVENTORY_FULL,
                None,
                None,
                header_crypto,
            )
            .await?;

            return Ok(false);
        };

        let random_properties = generate_item_instance_random_properties(
            world_db_pool,
            &session.db_creature_navigation.world_data_files,
            loot.item,
        )
        .await?;
        wow_db::add_character_inventory_item_with_random_properties(
            character_db_pool,
            wow_db::AddCharacterInventoryItemRequest {
                guid: character_guid,
                bag: INVENTORY_SLOT_BAG_0 as u32,
                slot: dst_slot,
                item_template: loot.item,
                count: remaining_count,
                durability: 0,
                random_properties: random_properties.as_ref(),
            },
        )
        .await?;

        session.inventory =
            wow_db::get_character_inventory_items(character_db_pool, character_guid).await?;

        if let Some(new_item) = session.inventory.iter().find(|item| {
            item.bag == INVENTORY_SLOT_BAG_0 as u32
                && item.slot == dst_slot
                && item.item_template == loot.item
        }) {
            let owner_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);

            update_blocks.push(build_item_create_update_block(
                owner_guid, owner_guid, new_item, None,
            )?);

            update_blocks.push(build_inventory_slots_update_block(
                character_guid,
                &session.inventory,
                &[dst_slot],
            )?);
            pushed_item = Some(new_item.clone());
        }
    } else {
        session.inventory =
            wow_db::get_character_inventory_items(character_db_pool, character_guid).await?;
    }

    send_packet(
        stream,
        SMSG_LOOT_REMOVED,
        &[loot_slot],
        Some(&mut *header_crypto),
    )
    .await?;

    if let Some(item) = pushed_item.as_ref() {
        let body = build_item_push_result_body(character_guid, item, loot.count, true, false, true);
        send_packet(
            stream,
            SMSG_ITEM_PUSH_RESULT,
            &body,
            Some(&mut *header_crypto),
        )
        .await?;
    }

    let body = build_update_object_body(&update_blocks);

    send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(header_crypto)).await?;

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

fn build_db_creature_loot_response_body_for_player(
    target: ObjectGuid,
    creature: &DbCreatureRuntime,
    loot_method: Option<(u8, u8, u32)>,
    character_guid: u32,
) -> Vec<u8> {
    let item_count = creature.loot_items.len().min(u8::MAX as usize) as u8;

    let mut body = Vec::with_capacity(14 + item_count as usize * 22);

    body.extend_from_slice(&target.raw().to_le_bytes());

    body.push(CLIENT_LOOT_CORPSE);

    body.extend_from_slice(
        &(if creature.loot_money_available {
            creature.loot_money()
        } else {
            0
        })
        .to_le_bytes(),
    );

    let count_pos = body.len();
    body.push(0);

    let mut shown = 0u8;
    for loot in creature.loot_items.iter().take(item_count as usize) {
        let Some(slot_type) =
            db_creature_loot_slot_type_for_player(creature, loot_method, character_guid, loot)
        else {
            continue;
        };
        body.push(loot.slot);

        body.extend_from_slice(&loot.item.to_le_bytes());

        body.extend_from_slice(&loot.count.to_le_bytes());

        body.extend_from_slice(&loot.display_id.to_le_bytes());

        body.extend_from_slice(&0u32.to_le_bytes());

        body.extend_from_slice(&0u32.to_le_bytes());

        body.push(slot_type);
        shown = shown.saturating_add(1);
    }
    body[count_pos] = shown;

    body
}

fn db_creature_loot_slot_type_for_player(
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
                || creature
                    .loot_current_looter_pass_slots
                    .contains(&loot.slot)
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
                    || creature
                        .loot_current_looter_pass_slots
                        .contains(&loot.slot)
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

fn build_gameobject_loot_response_body(
    target: ObjectGuid,
    loot_items: &[DbCreatureLootRuntime],
) -> Vec<u8> {
    let item_count = loot_items.len().min(u8::MAX as usize) as u8;
    let mut body = Vec::with_capacity(14 + item_count as usize * 22);

    body.extend_from_slice(&target.raw().to_le_bytes());
    body.push(CLIENT_LOOT_CORPSE);
    body.extend_from_slice(&0u32.to_le_bytes());
    body.push(item_count);

    for loot in loot_items.iter().take(item_count as usize) {
        body.push(loot.slot);
        body.extend_from_slice(&loot.item.to_le_bytes());
        body.extend_from_slice(&loot.count.to_le_bytes());
        body.extend_from_slice(&loot.display_id.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
        body.push(LOOT_SLOT_NORMAL);
    }

    body
}

fn build_loot_master_list_body(members: &[u32]) -> Vec<u8> {
    let mut body = Vec::with_capacity(1 + members.len().min(u8::MAX as usize) * 8);
    body.push(members.len().min(u8::MAX as usize) as u8);
    for member in members.iter().take(u8::MAX as usize) {
        body.extend_from_slice(
            &ObjectGuid::new(HighGuid::Player, 0, *member)
                .raw()
                .to_le_bytes(),
        );
    }
    body
}

fn build_loot_release_response_body(target: ObjectGuid, released: bool) -> Vec<u8> {
    let mut body = Vec::with_capacity(9);

    body.extend_from_slice(&target.raw().to_le_bytes());

    body.push(released as u8);

    body
}
