pub async fn get_character_inventory_items(
    pool: &MySqlPool,
    guid: u32,
) -> Result<Vec<CharacterInventoryItem>, DbError> {
    let rows = sqlx::query_as::<_, CharacterInventoryItem>(
        "SELECT ci.bag, ci.slot, ci.item, ci.item_template, ii.count, \
                ii.randomPropertyId, ii.enchantments, ii.durability \
         FROM character_inventory ci \
         JOIN item_instance ii ON ci.item = ii.guid \
         WHERE ci.guid = ? ORDER BY ci.bag, ci.slot",
    )
    .bind(guid)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn swap_character_inventory_slots(
    pool: &MySqlPool,
    guid: u32,
    src_bag: u32,
    src_slot: u8,
    dst_bag: u32,
    dst_slot: u8,
) -> Result<Option<InventoryMoveResult>, DbError> {
    swap_character_inventory_slots_with_stack(
        pool, guid, src_bag, src_slot, dst_bag, dst_slot, None,
    )
    .await
}

pub async fn swap_character_inventory_slots_with_stack(
    pool: &MySqlPool,
    guid: u32,
    src_bag: u32,
    src_slot: u8,
    dst_bag: u32,
    dst_slot: u8,
    max_stack: Option<u32>,
) -> Result<Option<InventoryMoveResult>, DbError> {
    let src_item: Option<(u32, u32, u32)> = sqlx::query_as(
        "SELECT ci.item, ci.item_template, ii.count FROM character_inventory ci \
         JOIN item_instance ii ON ci.item = ii.guid \
         WHERE ci.guid = ? AND ci.bag = ? AND ci.slot = ? AND ii.owner_guid = ?",
    )
    .bind(guid)
    .bind(src_bag)
    .bind(src_slot)
    .bind(guid)
    .fetch_optional(pool)
    .await?;

    let Some((src_item, src_template, src_count)) = src_item else {
        return Ok(None);
    };

    let dst_item: Option<(u32, u32, u32)> = sqlx::query_as(
        "SELECT ci.item, ci.item_template, ii.count FROM character_inventory ci \
         JOIN item_instance ii ON ci.item = ii.guid \
         WHERE ci.guid = ? AND ci.bag = ? AND ci.slot = ? AND ii.owner_guid = ?",
    )
    .bind(guid)
    .bind(dst_bag)
    .bind(dst_slot)
    .bind(guid)
    .fetch_optional(pool)
    .await?;

    if let Some((dst_item, dst_template, dst_count)) = dst_item {
        if src_template == dst_template {
            if let Some(max_stack) = max_stack.filter(|max_stack| *max_stack > 1) {
                if dst_count < max_stack {
                    let move_count = src_count.min(max_stack - dst_count);
                    let new_dst_count = dst_count + move_count;
                    let new_src_count = src_count - move_count;
                    sqlx::query(
                        "UPDATE item_instance SET count = ? WHERE guid = ? AND owner_guid = ?",
                    )
                    .bind(new_dst_count)
                    .bind(dst_item)
                    .bind(guid)
                    .execute(pool)
                    .await?;
                    let source_count = if new_src_count == 0 {
                        sqlx::query("DELETE FROM character_inventory WHERE item = ? AND guid = ?")
                            .bind(src_item)
                            .bind(guid)
                            .execute(pool)
                            .await?;
                        sqlx::query("DELETE FROM item_instance WHERE guid = ? AND owner_guid = ?")
                            .bind(src_item)
                            .bind(guid)
                            .execute(pool)
                            .await?;
                        None
                    } else {
                        sqlx::query(
                            "UPDATE item_instance SET count = ? WHERE guid = ? AND owner_guid = ?",
                        )
                        .bind(new_src_count)
                        .bind(src_item)
                        .bind(guid)
                        .execute(pool)
                        .await?;
                        Some(new_src_count)
                    };
                    return Ok(Some(InventoryMoveResult::Merged {
                        source_item: src_item,
                        source_count,
                        destination_item: dst_item,
                        destination_count: new_dst_count,
                    }));
                }
            }
        }
        sqlx::query("UPDATE character_inventory SET bag = ?, slot = ? WHERE guid = ? AND item = ?")
            .bind(src_bag)
            .bind(src_slot)
            .bind(guid)
            .bind(dst_item)
            .execute(pool)
            .await?;
    }

    sqlx::query("UPDATE character_inventory SET bag = ?, slot = ? WHERE guid = ? AND item = ?")
        .bind(dst_bag)
        .bind(dst_slot)
        .bind(guid)
        .bind(src_item)
        .execute(pool)
        .await?;

    refresh_character_equipment_cache(pool, guid).await?;

    Ok(Some(InventoryMoveResult::Swapped))
}

pub async fn destroy_character_inventory_item(
    pool: &MySqlPool,
    guid: u32,
    bag: u32,
    slot: u8,
) -> Result<Option<InventoryDestroyResult>, DbError> {
    destroy_character_inventory_item_count(pool, guid, bag, slot, 0).await
}

pub async fn destroy_character_inventory_item_count(
    pool: &MySqlPool,
    guid: u32,
    bag: u32,
    slot: u8,
    count: u32,
) -> Result<Option<InventoryDestroyResult>, DbError> {
    let row: Option<(u32, u32)> = sqlx::query_as(
        "SELECT ci.item, ii.count FROM character_inventory ci \
         JOIN item_instance ii ON ci.item = ii.guid \
         WHERE ci.guid = ? AND ci.bag = ? AND ci.slot = ? AND ii.owner_guid = ?",
    )
    .bind(guid)
    .bind(bag)
    .bind(slot)
    .bind(guid)
    .fetch_optional(pool)
    .await?;

    let Some((item, current_count)) = row else {
        return Ok(None);
    };

    if count != 0 && count < current_count {
        let new_count = current_count - count;
        sqlx::query("UPDATE item_instance SET count = ? WHERE guid = ? AND owner_guid = ?")
            .bind(new_count)
            .bind(item)
            .bind(guid)
            .execute(pool)
            .await?;
        return Ok(Some(InventoryDestroyResult::CountChanged {
            item,
            count: new_count,
        }));
    }

    sqlx::query("DELETE FROM character_inventory WHERE item = ? AND guid = ?")
        .bind(item)
        .bind(guid)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM item_instance WHERE guid = ? AND owner_guid = ?")
        .bind(item)
        .bind(guid)
        .execute(pool)
        .await?;

    if bag == 0 && slot < ENUM_EQUIPMENT_CACHE_SLOTS as u8 {
        refresh_character_equipment_cache(pool, guid).await?;
    }

    Ok(Some(InventoryDestroyResult::Removed { item }))
}

pub async fn split_character_inventory_item(
    pool: &MySqlPool,
    guid: u32,
    src_bag: u32,
    src_slot: u8,
    dst_bag: u32,
    dst_slot: u8,
    count: u32,
) -> Result<Option<InventorySplitResult>, DbError> {
    if count == 0 {
        return Ok(None);
    }

    let source: Option<(u32, u32)> = sqlx::query_as(
        "SELECT ci.item, ii.count FROM character_inventory ci \
         JOIN item_instance ii ON ci.item = ii.guid \
         WHERE ci.guid = ? AND ci.bag = ? AND ci.slot = ? AND ii.owner_guid = ?",
    )
    .bind(guid)
    .bind(src_bag)
    .bind(src_slot)
    .bind(guid)
    .fetch_optional(pool)
    .await?;
    let Some((source_item, source_count)) = source else {
        return Ok(None);
    };
    if count >= source_count {
        return Ok(None);
    }

    let destination_item: Option<u32> = sqlx::query_scalar(
        "SELECT item FROM character_inventory \
         WHERE guid = ? AND bag = ? AND slot = ?",
    )
    .bind(guid)
    .bind(dst_bag)
    .bind(dst_slot)
    .fetch_optional(pool)
    .await?;
    if destination_item.is_some() {
        return Ok(None);
    }

    let new_item = next_item_guid(pool).await?;
    let new_source_count = source_count - count;
    sqlx::query("UPDATE item_instance SET count = ? WHERE guid = ? AND owner_guid = ?")
        .bind(new_source_count)
        .bind(source_item)
        .bind(guid)
        .execute(pool)
        .await?;
    sqlx::query(
        "INSERT INTO item_instance \
         (guid, owner_guid, itemEntry, creatorGuid, giftCreatorGuid, count, duration, \
          charges, flags, enchantments, randomPropertyId, durability, itemTextId) \
         SELECT ?, owner_guid, itemEntry, creatorGuid, giftCreatorGuid, ?, duration, \
                charges, flags, enchantments, randomPropertyId, durability, itemTextId \
         FROM item_instance WHERE guid = ? AND owner_guid = ?",
    )
    .bind(new_item)
    .bind(count)
    .bind(source_item)
    .bind(guid)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO character_inventory (guid, bag, slot, item, item_template) \
         SELECT ?, ?, ?, ?, item_template FROM character_inventory WHERE item = ? AND guid = ?",
    )
    .bind(guid)
    .bind(dst_bag)
    .bind(dst_slot)
    .bind(new_item)
    .bind(source_item)
    .bind(guid)
    .execute(pool)
    .await?;

    Ok(Some(InventorySplitResult {
        source_item,
        source_count: new_source_count,
        new_item,
    }))
}

pub async fn move_character_inventory_item_to_slot(
    pool: &MySqlPool,
    owner_guid: u32,
    item_guid: u32,
    bag: u32,
    slot: u8,
) -> Result<bool, DbError> {
    let result = sqlx::query(
        "UPDATE character_inventory ci \
         JOIN item_instance ii ON ci.item = ii.guid \
         SET ci.bag = ?, ci.slot = ? \
         WHERE ci.guid = ? AND ci.item = ? AND ii.owner_guid = ?",
    )
    .bind(bag)
    .bind(slot)
    .bind(owner_guid)
    .bind(item_guid)
    .bind(owner_guid)
    .execute(pool)
    .await?;

    if result.rows_affected() > 0 && bag == 0 && slot < ENUM_EQUIPMENT_CACHE_SLOTS as u8 {
        refresh_character_equipment_cache(pool, owner_guid).await?;
    }

    Ok(result.rows_affected() > 0)
}

pub async fn add_character_inventory_item(
    pool: &MySqlPool,
    guid: u32,
    bag: u32,
    slot: u8,
    item_template: u32,
    count: u32,
    durability: u32,
) -> Result<CharacterInventoryItem, DbError> {
    add_character_inventory_item_with_random_properties(pool, AddCharacterInventoryItemRequest {
        guid,
        bag,
        slot,
        item_template,
        count,
        durability,
        random_properties: None,
    })
    .await
}

pub struct AddCharacterInventoryItemRequest<'a> {
    pub guid: u32,
    pub bag: u32,
    pub slot: u8,
    pub item_template: u32,
    pub count: u32,
    pub durability: u32,
    pub random_properties: Option<&'a ItemInstanceRandomProperties>,
}

pub async fn add_character_inventory_item_with_random_properties(
    pool: &MySqlPool,
    request: AddCharacterInventoryItemRequest<'_>,
) -> Result<CharacterInventoryItem, DbError> {
    let item_guid = next_item_guid(pool).await?;
    let random_property_id = request
        .random_properties
        .map(|properties| properties.random_property_id)
        .unwrap_or(0);
    let enchantments = request
        .random_properties
        .map(|properties| properties.enchantments.clone())
        .unwrap_or_else(|| default_item_enchantments().to_string());
    sqlx::query(
        "INSERT INTO item_instance \
         (guid, owner_guid, itemEntry, creatorGuid, giftCreatorGuid, count, duration, \
          charges, flags, enchantments, randomPropertyId, durability, itemTextId) \
         VALUES (?, ?, ?, 0, 0, ?, 0, '0 0 0 0 0 ', 0, ?, ?, ?, 0)",
    )
    .bind(item_guid)
    .bind(request.guid)
    .bind(request.item_template)
    .bind(request.count)
    .bind(&enchantments)
    .bind(random_property_id)
    .bind(request.durability)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO character_inventory (guid, bag, slot, item, item_template) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(request.guid)
    .bind(request.bag)
    .bind(request.slot)
    .bind(item_guid)
    .bind(request.item_template)
    .execute(pool)
    .await?;

    Ok(CharacterInventoryItem {
        bag: request.bag,
        slot: request.slot,
        item: item_guid,
        item_template: request.item_template,
        count: request.count,
        random_property_id,
        enchantments,
        durability: request.durability,
    })
}

pub async fn update_character_inventory_item_count(
    pool: &MySqlPool,
    owner_guid: u32,
    item_guid: u32,
    count: u32,
) -> Result<bool, DbError> {
    let result =
        sqlx::query("UPDATE item_instance SET count = ? WHERE guid = ? AND owner_guid = ?")
            .bind(count)
            .bind(item_guid)
            .bind(owner_guid)
            .execute(pool)
            .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn update_character_inventory_item_random_properties(
    pool: &MySqlPool,
    owner_guid: u32,
    item_guid: u32,
    random_properties: &ItemInstanceRandomProperties,
) -> Result<bool, DbError> {
    let result = sqlx::query(
        "UPDATE item_instance SET randomPropertyId = ?, enchantments = ? \
         WHERE guid = ? AND owner_guid = ? AND randomPropertyId = 0",
    )
    .bind(random_properties.random_property_id)
    .bind(&random_properties.enchantments)
    .bind(item_guid)
    .bind(owner_guid)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn add_character_money(pool: &MySqlPool, guid: u32, amount: u32) -> Result<u32, DbError> {
    sqlx::query("UPDATE characters SET money = money + ? WHERE guid = ?")
        .bind(amount)
        .bind(guid)
        .execute(pool)
        .await?;
    let money = sqlx::query_scalar("SELECT money FROM characters WHERE guid = ?")
        .bind(guid)
        .fetch_one(pool)
        .await?;
    Ok(money)
}

pub async fn spend_character_money(
    pool: &MySqlPool,
    guid: u32,
    amount: u32,
) -> Result<Option<u32>, DbError> {
    let result =
        sqlx::query("UPDATE characters SET money = money - ? WHERE guid = ? AND money >= ?")
            .bind(amount)
            .bind(guid)
            .bind(amount)
            .execute(pool)
            .await?;
    if result.rows_affected() == 0 {
        return Ok(None);
    }
    let money = sqlx::query_scalar("SELECT money FROM characters WHERE guid = ?")
        .bind(guid)
        .fetch_one(pool)
        .await?;
    Ok(Some(money))
}

pub async fn refresh_character_equipment_cache(pool: &MySqlPool, guid: u32) -> Result<(), DbError> {
    let equipment_rows: Vec<(u8, u32)> = sqlx::query_as(
        "SELECT slot, item_template FROM character_inventory \
         WHERE guid = ? AND bag = 0 AND slot < ?",
    )
    .bind(guid)
    .bind(ENUM_EQUIPMENT_CACHE_SLOTS as u8)
    .fetch_all(pool)
    .await?;

    let mut equipment = [0u32; ENUM_EQUIPMENT_CACHE_SLOTS];
    for (slot, item_template) in equipment_rows {
        equipment[slot as usize] = item_template;
    }

    sqlx::query("UPDATE characters SET equipmentCache = ? WHERE guid = ?")
        .bind(format_equipment_cache(&equipment))
        .bind(guid)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn character_has_unread_mail(pool: &MySqlPool, guid: u32) -> Result<bool, DbError> {
    let unread: Option<u8> = sqlx::query_scalar(
        "SELECT 1 FROM mail \
         WHERE receiver = ? AND checked = 0 AND deliver_time <= UNIX_TIMESTAMP() LIMIT 1",
    )
    .bind(guid)
    .fetch_optional(pool)
    .await?;

    Ok(unread.is_some())
}

pub async fn get_item_template_query(
    pool: &MySqlPool,
    entry: u32,
) -> Result<Option<ItemTemplateQuery>, DbError> {
    let Some(row) = sqlx::query(
        "SELECT entry, class, subclass, name, displayid, Quality, Flags, BuyPrice, SellPrice, \
         InventoryType, AllowableClass, AllowableRace, ItemLevel, RequiredLevel, RequiredSkill, \
         RequiredSkillRank, requiredspell, requiredhonorrank, RequiredCityRank, \
         RequiredReputationFaction, RequiredReputationRank, maxcount, stackable, ContainerSlots, \
         stat_type1, stat_value1, stat_type2, stat_value2, stat_type3, stat_value3, \
         stat_type4, stat_value4, stat_type5, stat_value5, stat_type6, stat_value6, \
         stat_type7, stat_value7, stat_type8, stat_value8, stat_type9, stat_value9, \
         stat_type10, stat_value10, dmg_min1, dmg_max1, dmg_type1, dmg_min2, dmg_max2, \
         dmg_type2, dmg_min3, dmg_max3, dmg_type3, dmg_min4, dmg_max4, dmg_type4, \
         dmg_min5, dmg_max5, dmg_type5, armor, holy_res, fire_res, nature_res, frost_res, \
         shadow_res, arcane_res, delay, ammo_type, RangedModRange, bonding, description, \
         spellid_1, spelltrigger_1, spellcharges_1, spellcooldown_1, spellcategory_1, \
         spellcategorycooldown_1, spellid_2, spelltrigger_2, spellcharges_2, spellcooldown_2, \
         spellcategory_2, spellcategorycooldown_2, spellid_3, spelltrigger_3, spellcharges_3, \
         spellcooldown_3, spellcategory_3, spellcategorycooldown_3, spellid_4, \
         spelltrigger_4, spellcharges_4, spellcooldown_4, spellcategory_4, \
         spellcategorycooldown_4, spellid_5, spelltrigger_5, spellcharges_5, spellcooldown_5, \
         spellcategory_5, spellcategorycooldown_5, \
         PageText, LanguageID, PageMaterial, \
         startquest, lockid, Material, sheath, RandomProperty, block, itemset, MaxDurability, \
         area, Map, BagFamily FROM item_template WHERE entry = ?",
    )
    .bind(entry)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };

    Ok(Some(ItemTemplateQuery {
        entry: row.try_get("entry")?,
        class: row.try_get::<u8, _>("class")? as u32,
        subclass: row.try_get::<u8, _>("subclass")? as u32,
        name: row.try_get("name")?,
        displayid: row.try_get("displayid")?,
        quality: row.try_get::<u8, _>("Quality")? as u32,
        flags: row.try_get("Flags")?,
        buy_price: row.try_get("BuyPrice")?,
        sell_price: row.try_get("SellPrice")?,
        inventory_type: row.try_get::<u8, _>("InventoryType")? as u32,
        allowable_class: row.try_get("AllowableClass")?,
        allowable_race: row.try_get("AllowableRace")?,
        item_level: row.try_get::<u8, _>("ItemLevel")? as u32,
        required_level: row.try_get::<u8, _>("RequiredLevel")? as u32,
        required_skill: row.try_get::<u16, _>("RequiredSkill")? as u32,
        required_skill_rank: row.try_get::<u16, _>("RequiredSkillRank")? as u32,
        required_spell: row.try_get("requiredspell")?,
        required_honor_rank: row.try_get("requiredhonorrank")?,
        required_city_rank: row.try_get("RequiredCityRank")?,
        required_reputation_faction: row.try_get::<u16, _>("RequiredReputationFaction")? as u32,
        required_reputation_rank: row.try_get::<u16, _>("RequiredReputationRank")? as u32,
        max_count: row.try_get::<u16, _>("maxcount")? as u32,
        stackable: row.try_get::<u16, _>("stackable")? as u32,
        container_slots: row.try_get::<u8, _>("ContainerSlots")? as u32,
        stats: read_item_template_stats(&row)?,
        damage: read_item_template_damage(&row)?,
        dmg_min1: row.try_get("dmg_min1")?,
        dmg_max1: row.try_get("dmg_max1")?,
        dmg_type1: row.try_get::<u8, _>("dmg_type1")? as u32,
        armor: row.try_get::<u16, _>("armor")? as u32,
        holy_res: row.try_get::<u8, _>("holy_res")? as u32,
        fire_res: row.try_get::<u8, _>("fire_res")? as u32,
        nature_res: row.try_get::<u8, _>("nature_res")? as u32,
        frost_res: row.try_get::<u8, _>("frost_res")? as u32,
        shadow_res: row.try_get::<u8, _>("shadow_res")? as u32,
        arcane_res: row.try_get::<u8, _>("arcane_res")? as u32,
        delay: row.try_get::<u16, _>("delay")? as u32,
        ammo_type: row.try_get::<u8, _>("ammo_type")? as u32,
        ranged_mod_range: row.try_get("RangedModRange")?,
        spells: read_item_template_spells(&row)?,
        bonding: row.try_get::<u8, _>("bonding")? as u32,
        description: row.try_get("description")?,
        page_text: row.try_get("PageText")?,
        language_id: row.try_get::<u8, _>("LanguageID")? as u32,
        page_material: row.try_get::<u8, _>("PageMaterial")? as u32,
        start_quest: row.try_get("startquest")?,
        lock_id: row.try_get("lockid")?,
        material: row.try_get::<i8, _>("Material")? as i32,
        sheath: row.try_get::<u8, _>("sheath")? as u32,
        random_property: row.try_get("RandomProperty")?,
        block: row.try_get("block")?,
        itemset: row.try_get("itemset")?,
        max_durability: row.try_get::<u16, _>("MaxDurability")? as u32,
        area: row.try_get("area")?,
        map: row.try_get::<i16, _>("Map")? as i32,
        bag_family: row.try_get("BagFamily")?,
    }))
}

fn read_item_template_stats(
    row: &sqlx::mysql::MySqlRow,
) -> Result<[ItemTemplateStat; 10], DbError> {
    let mut stats = [ItemTemplateStat::default(); 10];
    for index in 1..=10 {
        stats[index - 1] = ItemTemplateStat {
            stat_type: row.try_get::<u8, _>(format!("stat_type{index}").as_str())? as u32,
            stat_value: row.try_get::<i16, _>(format!("stat_value{index}").as_str())? as i32,
        };
    }
    Ok(stats)
}

fn read_item_template_damage(
    row: &sqlx::mysql::MySqlRow,
) -> Result<[ItemTemplateDamage; 5], DbError> {
    let mut damage = [ItemTemplateDamage::default(); 5];
    for index in 1..=5 {
        damage[index - 1] = ItemTemplateDamage {
            damage_min: row.try_get(format!("dmg_min{index}").as_str())?,
            damage_max: row.try_get(format!("dmg_max{index}").as_str())?,
            damage_type: row.try_get::<u8, _>(format!("dmg_type{index}").as_str())? as u32,
        };
    }
    Ok(damage)
}

fn read_item_template_spells(
    row: &sqlx::mysql::MySqlRow,
) -> Result<[ItemTemplateSpell; 5], DbError> {
    let mut spells = [ItemTemplateSpell::default(); 5];
    for index in 1..=5 {
        spells[index - 1] = ItemTemplateSpell {
            spell_id: row.try_get(format!("spellid_{index}").as_str())?,
            spell_trigger: row.try_get::<u8, _>(format!("spelltrigger_{index}").as_str())? as u32,
            spell_charges: row.try_get::<i8, _>(format!("spellcharges_{index}").as_str())? as i32,
            spell_cooldown: row.try_get(format!("spellcooldown_{index}").as_str())?,
            spell_category: row.try_get::<u16, _>(format!("spellcategory_{index}").as_str())?
                as u32,
            spell_category_cooldown: row
                .try_get(format!("spellcategorycooldown_{index}").as_str())?,
        };
    }
    Ok(spells)
}

