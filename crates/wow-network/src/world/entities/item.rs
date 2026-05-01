// CMaNGOS reference: src/game/Entities/Item.{h,cpp}
// Owner for item instance state, visible equipment, inventory slot, stack,
// durability, item dynamic flags, and item/container update fields.

fn set_visible_item_update_values(
    values: &mut [Option<u32>],
    character: &CharacterEnumEntry,
    inventory: &[CharacterInventoryItem],
) -> anyhow::Result<()> {
    let equipment = visible_equipment_for_inventory(character.equipment_cache.as_deref(), inventory);
    set_visible_item_update_values_from_equipment(values, &equipment)
}

fn visible_equipment_for_inventory(
    equipment_cache: Option<&str>,
    inventory: &[CharacterInventoryItem],
) -> [u32; ENUM_EQUIPMENT_SLOTS] {
    let mut equipment = parse_equipment_cache(equipment_cache);
    for item in inventory {
        if item.bag == INVENTORY_SLOT_BAG_0 as u32 && item.slot < EQUIPMENT_SLOT_END {
            equipment[item.slot as usize] = item.item_template;
        }
    }
    equipment
}

fn set_visible_item_update_values_from_equipment(
    values: &mut [Option<u32>],
    equipment: &[u32; ENUM_EQUIPMENT_SLOTS],
) -> anyhow::Result<()> {
    for (slot, item_id) in equipment
        .iter()
        .take(EQUIPMENT_SLOT_END as usize)
        .enumerate()
    {
        if *item_id == 0 {
            continue;
        }

        let visible_base = 0x104 + slot * 12;
        set_update_value(values, visible_base, *item_id)?;
    }

    Ok(())
}

fn build_player_visible_equipment_update_block(
    character_guid: u32,
    visible_equipment: &[u32; ENUM_EQUIPMENT_SLOTS],
    slots: &[u8],
) -> anyhow::Result<Vec<u8>> {
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player_guid)?;

    let mut values = vec![None; PLAYER_END_FIELDS];
    for slot in slots {
        if *slot >= EQUIPMENT_SLOT_END {
            continue;
        }
        let visible_base = 0x104 + *slot as usize * 12;
        set_update_value(
            &mut values,
            visible_base,
            visible_equipment[*slot as usize],
        )?;
    }
    write_update_values(&mut block, &values)?;
    Ok(block)
}

fn set_inventory_slot_update_values(
    values: &mut [Option<u32>],
    inventory: &[CharacterInventoryItem],
) -> anyhow::Result<()> {
    for item in inventory {
        if item.bag != INVENTORY_SLOT_BAG_0 as u32 {
            continue;
        }

        let Some(field) = inventory_slot_update_field(item.slot) else {
            continue;
        };
        let guid = ObjectGuid::new(HighGuid::Item, 0, item.item);
        set_update_value(values, field, guid.raw() as u32)?;
        set_update_value(values, field + 1, (guid.raw() >> 32) as u32)?;
    }

    Ok(())
}

fn build_inventory_slots_update_body(
    character_guid: u32,
    inventory: &[CharacterInventoryItem],
    slots: &[u8],
) -> anyhow::Result<Vec<u8>> {
    let block = build_inventory_slots_update_block(character_guid, inventory, slots)?;

    let mut body = Vec::with_capacity(5 + block.len());
    body.extend_from_slice(&1u32.to_le_bytes());
    body.push(0);
    body.extend_from_slice(&block);
    Ok(body)
}

fn build_inventory_slots_update_block(
    character_guid: u32,
    inventory: &[CharacterInventoryItem],
    slots: &[u8],
) -> anyhow::Result<Vec<u8>> {
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player_guid)?;

    let mut values = vec![None; PLAYER_END_FIELDS];
    for slot in slots {
        let Some(field) = inventory_slot_update_field(*slot) else {
            continue;
        };
        let item = inventory
            .iter()
            .find(|item| item.bag == INVENTORY_SLOT_BAG_0 as u32 && item.slot == *slot);
        let item_guid = item
            .map(|item| ObjectGuid::new(HighGuid::Item, 0, item.item).raw())
            .unwrap_or(0);
        set_update_value(&mut values, field, item_guid as u32)?;
        set_update_value(&mut values, field + 1, (item_guid >> 32) as u32)?;

        if *slot < EQUIPMENT_SLOT_END {
            let visible_base = 0x104 + *slot as usize * 12;
            set_update_value(
                &mut values,
                visible_base,
                item.map(|item| item.item_template).unwrap_or(0),
            )?;
        }
    }
    write_update_values(&mut block, &values)?;

    Ok(block)
}

fn build_item_stack_count_update_body(item_guid: u32, count: u32) -> anyhow::Result<Vec<u8>> {
    build_item_stack_counts_update_body(&[(item_guid, count)])
}

fn build_item_stack_counts_update_body(items: &[(u32, u32)]) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::new();
    body.extend_from_slice(&(items.len() as u32).to_le_bytes());
    body.push(0);

    for (item_guid, count) in items {
        body.extend_from_slice(&build_item_stack_count_update_block(*item_guid, *count)?);
    }

    Ok(body)
}

fn build_item_stack_count_update_block(item_guid: u32, count: u32) -> anyhow::Result<Vec<u8>> {
    let item_guid = ObjectGuid::new(HighGuid::Item, 0, item_guid);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, item_guid)?;

    let mut values = vec![None; ITEM_END_FIELDS];
    set_update_value(&mut values, 0x00E, count)?;
    write_update_values(&mut block, &values)?;

    Ok(block)
}

fn build_player_money_update_body(character_guid: u32, money: u32) -> anyhow::Result<Vec<u8>> {
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player_guid)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, PLAYER_FIELD_COINAGE, money)?;
    write_update_values(&mut block, &values)?;
    Ok(build_update_object_body(&[block]))
}

fn build_destroy_object_body(item_guid: u32) -> Vec<u8> {
    ObjectGuid::new(HighGuid::Item, 0, item_guid)
        .raw()
        .to_le_bytes()
        .to_vec()
}
fn inventory_slot_update_field(slot: u8) -> Option<usize> {
    match slot {
        0..INVENTORY_SLOT_ITEM_START => Some(PLAYER_FIELD_INV_SLOT_HEAD + slot as usize * 2),
        INVENTORY_SLOT_ITEM_START..INVENTORY_SLOT_ITEM_END => {
            Some(PLAYER_FIELD_PACK_SLOT_1 + (slot - INVENTORY_SLOT_ITEM_START) as usize * 2)
        }
        _ => None,
    }
}

fn build_inventory_item_create_blocks(
    character: &CharacterEnumEntry,
    inventory: &[CharacterInventoryItem],
) -> anyhow::Result<Vec<Vec<u8>>> {
    let owner_guid = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let mut blocks = Vec::new();

    for item in inventory {
        if item.bag != INVENTORY_SLOT_BAG_0 as u32 {
            continue;
        }

        if item.slot >= INVENTORY_SLOT_ITEM_END {
            continue;
        }

        let contained_guid = item_contained_guid(owner_guid, inventory, item);
        blocks.push(build_item_create_update_block(owner_guid, contained_guid, item, None)?);
    }

    Ok(blocks)
}

fn build_item_create_update_block(
    owner_guid: ObjectGuid,
    contained_guid: ObjectGuid,
    item: &CharacterInventoryItem,
    container_slots: Option<u32>,
) -> anyhow::Result<Vec<u8>> {
    let item_guid = ObjectGuid::new(HighGuid::Item, 0, item.item);
    let is_container = container_slots.unwrap_or(0) > 0;
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_CREATE_OBJECT);
    PackedGuid::write(&mut block, item_guid)?;
    block.push(if is_container {
        TYPEID_CONTAINER
    } else {
        TYPEID_ITEM
    });
    block.push(UPDATEFLAG_ALL);
    block.extend_from_slice(&1u32.to_le_bytes());

    let mut values = vec![None; if is_container { CONTAINER_END_FIELDS } else { ITEM_END_FIELDS }];
    set_update_value(&mut values, 0x000, item_guid.raw() as u32)?;
    set_update_value(&mut values, 0x001, (item_guid.raw() >> 32) as u32)?;
    set_update_value(
        &mut values,
        0x002,
        if is_container {
            TYPEMASK_OBJECT_CONTAINER
        } else {
            TYPEMASK_OBJECT_ITEM
        },
    )?;
    set_update_value(&mut values, 0x003, item.item_template)?;
    set_update_value(&mut values, 0x004, 1.0f32.to_bits())?;
    set_update_value(&mut values, 0x006, owner_guid.raw() as u32)?;
    set_update_value(&mut values, 0x007, (owner_guid.raw() >> 32) as u32)?;
    set_update_value(&mut values, 0x008, contained_guid.raw() as u32)?;
    set_update_value(&mut values, 0x009, (contained_guid.raw() >> 32) as u32)?;
    set_update_value(&mut values, 0x00E, item.count)?;
    set_update_value(&mut values, 0x02E, item.durability)?;
    set_update_value(&mut values, 0x02F, item.durability)?;
    if let Some(container_slots) = container_slots.filter(|slots| *slots > 0) {
        set_update_value(&mut values, CONTAINER_FIELD_NUM_SLOTS, container_slots)?;
    }
    write_update_values(&mut block, &values)?;

    Ok(block)
}

fn build_item_contained_update_block(
    owner_guid: ObjectGuid,
    inventory: &[CharacterInventoryItem],
    item: &CharacterInventoryItem,
) -> anyhow::Result<Vec<u8>> {
    let item_guid = ObjectGuid::new(HighGuid::Item, 0, item.item);
    let contained_guid = item_contained_guid(owner_guid, inventory, item);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, item_guid)?;

    let mut values = vec![None; ITEM_END_FIELDS];
    set_update_value(&mut values, 0x008, contained_guid.raw() as u32)?;
    set_update_value(&mut values, 0x009, (contained_guid.raw() >> 32) as u32)?;
    write_update_values(&mut block, &values)?;
    Ok(block)
}

fn build_container_slot_update_block(
    inventory: &[CharacterInventoryItem],
    bag_slot: u8,
    container_slot: u8,
) -> anyhow::Result<Option<Vec<u8>>> {
    let Some(container_item) = inventory
        .iter()
        .find(|item| item.bag == INVENTORY_SLOT_BAG_0 as u32 && item.slot == bag_slot)
    else {
        return Ok(None);
    };
    let container_guid = ObjectGuid::new(HighGuid::Item, 0, container_item.item);
    let contained_guid = inventory
        .iter()
        .find(|item| item.bag == bag_slot as u32 && item.slot == container_slot)
        .map(|item| ObjectGuid::new(HighGuid::Item, 0, item.item).raw())
        .unwrap_or(0);

    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, container_guid)?;
    let mut values = vec![None; CONTAINER_END_FIELDS];
    let field = CONTAINER_FIELD_SLOT_1 + container_slot as usize * 2;
    set_update_value(&mut values, field, contained_guid as u32)?;
    set_update_value(&mut values, field + 1, (contained_guid >> 32) as u32)?;
    write_update_values(&mut block, &values)?;
    Ok(Some(block))
}

fn item_contained_guid(
    owner_guid: ObjectGuid,
    inventory: &[CharacterInventoryItem],
    item: &CharacterInventoryItem,
) -> ObjectGuid {
    if item.bag == INVENTORY_SLOT_BAG_0 as u32 {
        return owner_guid;
    }
    inventory
        .iter()
        .find(|container| {
            container.bag == INVENTORY_SLOT_BAG_0 as u32 && container.slot == item.bag as u8
        })
        .map(|container| ObjectGuid::new(HighGuid::Item, 0, container.item))
        .unwrap_or(owner_guid)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StarterItemVisual {
    display_id: u32,
    inventory_type: u8,
}

fn parse_equipment_cache(cache: Option<&str>) -> [u32; ENUM_EQUIPMENT_SLOTS] {
    let mut equipment = [0u32; ENUM_EQUIPMENT_SLOTS];
    let Some(cache) = cache else {
        return equipment;
    };

    for (slot, chunk) in cache
        .split_whitespace()
        .filter_map(|value| value.parse::<u32>().ok())
        .collect::<Vec<_>>()
        .chunks(2)
        .take(ENUM_EQUIPMENT_SLOTS)
        .enumerate()
    {
        if let Some(item_id) = chunk.first() {
            equipment[slot] = *item_id;
        }
    }

    equipment
}

fn starter_item_visual(item_id: u32) -> Option<StarterItemVisual> {
    match item_id {
        25 => Some(StarterItemVisual {
            display_id: 1542,
            inventory_type: 21,
        }),
        38 => Some(StarterItemVisual {
            display_id: 9891,
            inventory_type: 4,
        }),
        39 => Some(StarterItemVisual {
            display_id: 9892,
            inventory_type: 7,
        }),
        40 => Some(StarterItemVisual {
            display_id: 10141,
            inventory_type: 8,
        }),
        2362 => Some(StarterItemVisual {
            display_id: 18730,
            inventory_type: 14,
        }),
        _ => None,
    }
}

