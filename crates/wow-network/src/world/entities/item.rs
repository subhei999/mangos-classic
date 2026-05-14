use super::*;

// CMaNGOS reference: src/game/Entities/Item.{h,cpp}
// Owner for item instance state, visible equipment, inventory slot, stack,
// durability, item dynamic flags, and item/container update fields.

pub(in crate::world) const ITEM_FIELD_ENCHANTMENT: usize = 0x016;
pub(in crate::world) const ITEM_FIELD_SPELL_CHARGES: usize = 0x010;
pub(in crate::world) const ITEM_FIELD_RANDOM_PROPERTIES_ID: usize = 0x02C;
pub(in crate::world) const MAX_ENCHANTMENT_SLOT: usize = 7;
pub(in crate::world) const MAX_ENCHANTMENT_OFFSET: usize = 3;
pub(in crate::world) const ITEM_ENCHANTMENT_FIELD_COUNT: usize =
    MAX_ENCHANTMENT_SLOT * MAX_ENCHANTMENT_OFFSET;
pub(in crate::world) const PROP_ENCHANTMENT_SLOT_0: usize = 3;

pub(in crate::world) fn set_visible_item_update_values(
    values: &mut [Option<u32>],
    character: &CharacterEnumEntry,
    inventory: &[CharacterInventoryItem],
) -> anyhow::Result<()> {
    let equipment =
        visible_equipment_for_inventory(character.equipment_cache.as_deref(), inventory);
    set_visible_item_update_values_from_equipment(values, &equipment)
}

pub(in crate::world) fn visible_equipment_for_inventory(
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

pub(in crate::world) fn set_visible_item_update_values_from_equipment(
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

pub(in crate::world) fn build_player_visible_equipment_update_block(
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
        set_update_value(&mut values, visible_base, visible_equipment[*slot as usize])?;
    }
    write_update_values(&mut block, &values)?;
    Ok(block)
}

pub(in crate::world) fn set_inventory_slot_update_values(
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

pub(in crate::world) fn build_inventory_slots_update_body(
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

pub(in crate::world) fn build_inventory_slots_update_block(
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

pub(in crate::world) fn build_item_push_result_body(
    character_guid: u32,
    item: &CharacterInventoryItem,
    count: u32,
    received: bool,
    created: bool,
    show_in_chat: bool,
) -> Vec<u8> {
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let bag_slot = if item.bag == INVENTORY_SLOT_BAG_0 as u32 {
        CLIENT_INVENTORY_SLOT_BAG_0
    } else {
        item.bag as u8
    };
    let slot = if item.count == count {
        item.slot as u32
    } else {
        u32::MAX
    };

    let mut body = Vec::with_capacity(8 + 4 + 4 + 4 + 1 + 4 + 4 + 4 + 4 + 4);
    body.extend_from_slice(&player_guid.raw().to_le_bytes());
    body.extend_from_slice(&(received as u32).to_le_bytes());
    body.extend_from_slice(&(created as u32).to_le_bytes());
    body.extend_from_slice(&(show_in_chat as u32).to_le_bytes());
    body.push(bag_slot);
    body.extend_from_slice(&slot.to_le_bytes());
    body.extend_from_slice(&item.item_template.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes());
    body.extend_from_slice(&0i32.to_le_bytes());
    body.extend_from_slice(&count.to_le_bytes());
    body
}

pub(in crate::world) fn build_item_stack_count_update_body(
    item_guid: u32,
    count: u32,
) -> anyhow::Result<Vec<u8>> {
    build_item_stack_counts_update_body(&[(item_guid, count)])
}

pub(in crate::world) fn build_item_stack_counts_update_body(
    items: &[(u32, u32)],
) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::new();
    body.extend_from_slice(&(items.len() as u32).to_le_bytes());
    body.push(0);

    for (item_guid, count) in items {
        body.extend_from_slice(&build_item_stack_count_update_block(*item_guid, *count)?);
    }

    Ok(body)
}

pub(in crate::world) fn build_item_stack_count_update_block(
    item_guid: u32,
    count: u32,
) -> anyhow::Result<Vec<u8>> {
    let item_guid = ObjectGuid::new(HighGuid::Item, 0, item_guid);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, item_guid)?;

    let mut values = vec![None; ITEM_END_FIELDS];
    set_update_value(&mut values, 0x00E, count)?;
    write_update_values(&mut block, &values)?;

    Ok(block)
}

pub(in crate::world) fn build_player_money_update_body(
    character_guid: u32,
    money: u32,
) -> anyhow::Result<Vec<u8>> {
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player_guid)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, PLAYER_FIELD_COINAGE, money)?;
    write_update_values(&mut block, &values)?;
    Ok(build_update_object_body(&[block]))
}

pub(in crate::world) fn build_destroy_object_body(item_guid: u32) -> Vec<u8> {
    ObjectGuid::new(HighGuid::Item, 0, item_guid)
        .raw()
        .to_le_bytes()
        .to_vec()
}
pub(in crate::world) fn inventory_slot_update_field(slot: u8) -> Option<usize> {
    match slot {
        0..INVENTORY_SLOT_ITEM_START => Some(PLAYER_FIELD_INV_SLOT_HEAD + slot as usize * 2),
        INVENTORY_SLOT_ITEM_START..INVENTORY_SLOT_ITEM_END => {
            Some(PLAYER_FIELD_PACK_SLOT_1 + (slot - INVENTORY_SLOT_ITEM_START) as usize * 2)
        }
        _ => None,
    }
}

pub(in crate::world) fn build_inventory_item_create_blocks(
    character: &CharacterEnumEntry,
    inventory: &[CharacterInventoryItem],
    container_slots_by_item: &HashMap<u32, u32>,
) -> anyhow::Result<Vec<Vec<u8>>> {
    let owner_guid = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let mut blocks = Vec::new();

    for item in inventory {
        if !login_inventory_position_is_visible(item, inventory) {
            continue;
        }

        let contained_guid = item_contained_guid(owner_guid, inventory, item);
        let container_slots = container_slots_by_item.get(&item.item).copied();
        blocks.push(build_item_create_update_block(
            owner_guid,
            contained_guid,
            item,
            container_slots,
        )?);
        if container_slots.is_some_and(|slots| slots > 0) && item.bag == INVENTORY_SLOT_BAG_0 as u32
        {
            for contained in inventory
                .iter()
                .filter(|contained| contained.bag == item.slot as u32)
            {
                if let Some(block) =
                    build_container_slot_update_block(inventory, item.slot, contained.slot)?
                {
                    blocks.push(block);
                }
            }
        }
    }

    Ok(blocks)
}

pub(in crate::world) fn login_inventory_position_is_visible(
    item: &CharacterInventoryItem,
    inventory: &[CharacterInventoryItem],
) -> bool {
    if item.bag == INVENTORY_SLOT_BAG_0 as u32 {
        return item.slot < INVENTORY_SLOT_ITEM_END;
    }
    let Ok(bag_slot) = u8::try_from(item.bag) else {
        return false;
    };
    is_bag_slot(bag_slot)
        && item.slot < MAX_BAG_SIZE
        && inventory.iter().any(|container| {
            container.bag == INVENTORY_SLOT_BAG_0 as u32 && container.slot == bag_slot
        })
}

pub(in crate::world) async fn load_inventory_container_slots(
    world_db_pool: &MySqlPool,
    inventory: &[CharacterInventoryItem],
) -> anyhow::Result<HashMap<u32, u32>> {
    let mut slots = HashMap::new();
    for item in inventory {
        if slots.contains_key(&item.item) {
            continue;
        }
        let Some(template) =
            wow_db::get_item_template_query(world_db_pool, item.item_template).await?
        else {
            continue;
        };
        if template.container_slots > 0 {
            slots.insert(item.item, template.container_slots.min(MAX_BAG_SIZE as u32));
        }
    }
    Ok(slots)
}

pub(in crate::world) fn build_item_create_update_block(
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

    let mut values = vec![
        None;
        if is_container {
            CONTAINER_END_FIELDS
        } else {
            ITEM_END_FIELDS
        }
    ];
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
    for (offset, charge_value) in parse_item_spell_charges(&item.charges)
        .into_iter()
        .enumerate()
    {
        if charge_value != 0 {
            set_update_value(
                &mut values,
                ITEM_FIELD_SPELL_CHARGES + offset,
                charge_value as u32,
            )?;
        }
    }
    for (offset, enchantment_value) in parse_item_enchantment_fields(&item.enchantments)
        .into_iter()
        .enumerate()
    {
        if enchantment_value != 0 {
            set_update_value(
                &mut values,
                ITEM_FIELD_ENCHANTMENT + offset,
                enchantment_value,
            )?;
        }
    }
    if item.random_property_id != 0 {
        set_update_value(
            &mut values,
            ITEM_FIELD_RANDOM_PROPERTIES_ID,
            item.random_property_id as u32,
        )?;
    }
    set_update_value(&mut values, 0x02E, item.durability)?;
    set_update_value(&mut values, 0x02F, item.durability)?;
    if let Some(container_slots) = container_slots.filter(|slots| *slots > 0) {
        set_update_value(&mut values, CONTAINER_FIELD_NUM_SLOTS, container_slots)?;
    }
    write_update_values(&mut block, &values)?;

    Ok(block)
}

pub(in crate::world) fn parse_item_spell_charges(charges: &str) -> [i32; 5] {
    let mut fields = [0; 5];
    for (index, value) in charges.split_whitespace().take(5).enumerate() {
        if let Ok(value) = value.parse::<i32>() {
            fields[index] = value;
        }
    }
    fields
}

pub(in crate::world) fn item_template_spell_charges_string(template: &ItemTemplateQuery) -> String {
    template
        .spells
        .iter()
        .map(|spell| spell.spell_charges.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

pub(in crate::world) async fn generate_item_instance_random_properties(
    world_db_pool: &MySqlPool,
    world_data_files: &WorldDataFiles,
    item_template: u32,
) -> anyhow::Result<Option<wow_db::ItemInstanceRandomProperties>> {
    let Some(template) = wow_db::get_item_template_query(world_db_pool, item_template).await?
    else {
        return Ok(None);
    };
    generate_item_instance_random_properties_for_template(
        world_db_pool,
        world_data_files,
        &template,
    )
    .await
}

pub(in crate::world) async fn repair_missing_inventory_random_properties(
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    world_data_files: &WorldDataFiles,
    owner_guid: u32,
    inventory: &mut [CharacterInventoryItem],
) -> anyhow::Result<()> {
    for item in inventory.iter_mut() {
        if item.random_property_id != 0 {
            continue;
        }
        let Some(random_properties) = generate_item_instance_random_properties(
            world_db_pool,
            world_data_files,
            item.item_template,
        )
        .await?
        else {
            continue;
        };
        if wow_db::update_character_inventory_item_random_properties(
            character_db_pool,
            owner_guid,
            item.item,
            &random_properties,
        )
        .await?
        {
            item.random_property_id = random_properties.random_property_id;
            item.enchantments = random_properties.enchantments;
        }
    }
    Ok(())
}

pub(in crate::world) async fn repair_missing_inventory_charges(
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    owner_guid: u32,
    inventory: &mut [CharacterInventoryItem],
) -> anyhow::Result<()> {
    for item in inventory.iter_mut() {
        if parse_item_spell_charges(&item.charges) != [0; 5] {
            continue;
        }
        let Some(template) =
            wow_db::get_item_template_query(world_db_pool, item.item_template).await?
        else {
            continue;
        };
        if template.spells.iter().all(|spell| spell.spell_charges == 0) {
            continue;
        }
        let charges = item_template_spell_charges_string(&template);
        if wow_db::update_character_inventory_item_charges(
            character_db_pool,
            owner_guid,
            item.item,
            &charges,
        )
        .await?
        {
            item.charges = charges;
        }
    }
    Ok(())
}

pub(in crate::world) async fn generate_item_instance_random_properties_for_template(
    world_db_pool: &MySqlPool,
    world_data_files: &WorldDataFiles,
    template: &ItemTemplateQuery,
) -> anyhow::Result<Option<wow_db::ItemInstanceRandomProperties>> {
    if template.random_property == 0 {
        return Ok(None);
    }
    let rolls =
        wow_db::get_item_random_property_rolls(world_db_pool, template.random_property).await?;
    let Some(random_property_id) = roll_item_random_property_id(&rolls) else {
        warn!(
            item = template.entry,
            random_property = template.random_property,
            "Item template references random property without item_enchantment_template rows"
        );
        return Ok(None);
    };
    let Some(random_property) = world_data_files
        .item_random_properties
        .get(&random_property_id)
        .copied()
    else {
        warn!(
            item = template.entry,
            random_property = template.random_property,
            random_property_id,
            "Rolled item random property missing from ItemRandomProperties.dbc"
        );
        return Ok(None);
    };
    Ok(Some(wow_db::ItemInstanceRandomProperties {
        random_property_id: random_property.id as i32,
        enchantments: item_enchantments_for_random_property(random_property),
    }))
}

pub(in crate::world) fn roll_item_random_property_id(
    rolls: &[wow_db::ItemRandomPropertyRoll],
) -> Option<u32> {
    if rolls.is_empty() {
        return None;
    }
    let roll = rand::thread_rng().gen_range(0.0..100.0);
    roll_item_random_property_id_for_roll(rolls, roll).or_else(|| {
        let chance_sum: f32 = rolls.iter().map(|roll| roll.chance).sum();
        if chance_sum <= 0.0 {
            return None;
        }
        let max_roll = (chance_sum * 100.0).floor() as u32 + 1;
        let fallback_roll = rand::thread_rng().gen_range(0..=max_roll) as f32 / 100.0;
        roll_item_random_property_id_for_roll(rolls, fallback_roll)
    })
}

pub(in crate::world) fn roll_item_random_property_id_for_roll(
    rolls: &[wow_db::ItemRandomPropertyRoll],
    roll: f32,
) -> Option<u32> {
    let mut chance = 0.0;
    for property_roll in rolls {
        chance += property_roll.chance;
        if chance > roll {
            return Some(property_roll.enchantment_id);
        }
    }
    None
}

pub(in crate::world) fn item_enchantments_for_random_property(
    random_property: ItemRandomPropertyEntry,
) -> String {
    let mut enchantments = [0u32; ITEM_ENCHANTMENT_FIELD_COUNT];
    for (index, enchant_id) in random_property.enchant_ids.into_iter().enumerate() {
        enchantments[(PROP_ENCHANTMENT_SLOT_0 + index) * MAX_ENCHANTMENT_OFFSET] = enchant_id;
    }
    enchantments
        .into_iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

pub(in crate::world) fn parse_item_enchantment_fields(
    enchantments: &str,
) -> [u32; ITEM_ENCHANTMENT_FIELD_COUNT] {
    let mut fields = [0u32; ITEM_ENCHANTMENT_FIELD_COUNT];
    for (index, value) in enchantments
        .split_whitespace()
        .take(ITEM_ENCHANTMENT_FIELD_COUNT)
        .enumerate()
    {
        fields[index] = value.parse::<u32>().unwrap_or(0);
    }
    fields
}

pub(in crate::world) fn build_item_contained_update_block(
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

pub(in crate::world) fn build_container_slot_update_block(
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

pub(in crate::world) fn item_contained_guid(
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
pub(in crate::world) struct StarterItemVisual {
    pub(in crate::world) display_id: u32,
    pub(in crate::world) inventory_type: u8,
}

pub(in crate::world) fn parse_equipment_cache(cache: Option<&str>) -> [u32; ENUM_EQUIPMENT_SLOTS] {
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

pub(in crate::world) fn starter_item_visual(item_id: u32) -> Option<StarterItemVisual> {
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
