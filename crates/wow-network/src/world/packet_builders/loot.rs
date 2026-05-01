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
            item: loot.item,

            count: loot.max_count.max(loot.min_count).max(1),

            display_id: loot.display_id,
        }
    }
}

struct LootAutostoreContext<'a> {
    stream: &'a mut WorldPacketSink,

    character_db_pool: &'a MySqlPool,

    world_db_pool: &'a MySqlPool,

    session: &'a mut WorldSessionState,

    header_crypto: &'a mut HeaderCrypto,

    character_guid: u32,
}

async fn autostore_loot_item(
    context: LootAutostoreContext<'_>,

    creature_guid: u64,

    loot: DbCreatureLootRuntime,

    loot_slot: u8,
) -> anyhow::Result<bool> {
    let LootAutostoreContext {
        stream,

        character_db_pool,

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
            }
        }
    }

    if remaining_count > 0 {
        let Some(dst_slot) = first_empty_backpack_slot(&session.inventory) else {
            send_inventory_change_failure(
                stream,
                EQUIP_ERR_COULDNT_SPLIT_ITEMS,
                None,
                None,
                header_crypto,
            )
            .await?;

            return Ok(false);
        };

        wow_db::add_character_inventory_item(
            character_db_pool,
            character_guid,
            INVENTORY_SLOT_BAG_0 as u32,
            dst_slot,
            loot.item,
            remaining_count,
            0,
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
        }
    } else {
        session.inventory =
            wow_db::get_character_inventory_items(character_db_pool, character_guid).await?;
    }

    if let Some(creature) = session.db_creatures.get_mut(&creature_guid) {
        creature.loot_item = None;
    }

    send_packet(
        stream,
        SMSG_LOOT_REMOVED,
        &[loot_slot],
        Some(&mut *header_crypto),
    )
    .await?;

    let body = build_update_object_body(&update_blocks);

    send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(header_crypto)).await?;

    Ok(true)
}

fn build_db_creature_loot_response_body(
    target: ObjectGuid,

    creature: &DbCreatureRuntime,
) -> Vec<u8> {
    let item_count = u8::from(creature.loot_item.is_some());

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

    body.push(item_count);

    if let Some(loot) = &creature.loot_item {
        body.push(0);

        body.extend_from_slice(&loot.item.to_le_bytes());

        body.extend_from_slice(&loot.count.to_le_bytes());

        body.extend_from_slice(&loot.display_id.to_le_bytes());

        body.extend_from_slice(&0u32.to_le_bytes());

        body.extend_from_slice(&0u32.to_le_bytes());

        body.push(LOOT_SLOT_NORMAL);
    }

    body
}

fn build_loot_release_response_body(target: ObjectGuid, released: bool) -> Vec<u8> {
    let mut body = Vec::with_capacity(9);

    body.extend_from_slice(&target.raw().to_le_bytes());

    body.push(released as u8);

    body
}
