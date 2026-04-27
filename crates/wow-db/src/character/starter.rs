async fn next_character_guid(pool: &MySqlPool) -> Result<u32, DbError> {
    let max_guid: Option<u32> = sqlx::query_scalar("SELECT MAX(guid) FROM characters")
        .fetch_one(pool)
        .await?;

    Ok(max_guid.unwrap_or(0).saturating_add(1))
}

fn player_bytes(skin: u8, face: u8, hair_style: u8, hair_color: u8) -> u32 {
    skin as u32 | ((face as u32) << 8) | ((hair_style as u32) << 16) | ((hair_color as u32) << 24)
}

#[derive(Debug, Clone, Copy)]
struct PlayerCreateInfo {
    zone: u32,
    position: WorldPosition,
}

const AT_LOGIN_FIRST: u32 = 0x20;
const LEVEL_ONE_SKILL_MAX: u16 = 5;
const MAIL_NORMAL: u8 = 0;
const MAIL_CHECK_MASK_RETURNED: u8 = 0x02;
const DEFAULT_MAIL_DELIVERY_DELAY_SECS: u64 = 60 * 60;
const DEFAULT_MAIL_EXPIRY_SECS: u64 = 30 * 24 * 60 * 60;

async fn get_player_create_info(
    world_pool: &MySqlPool,
    race: u8,
    class: u8,
) -> Result<PlayerCreateInfo, DbError> {
    let info = sqlx::query_as::<_, PlayerCreateInfoRow>(
        "SELECT map, zone, position_x, position_y, position_z, orientation \
         FROM playercreateinfo WHERE race = ? AND class = ?",
    )
    .bind(race)
    .bind(class)
    .fetch_one(world_pool)
    .await?;

    Ok(PlayerCreateInfo {
        zone: info.zone,
        position: WorldPosition::new(
            info.map as u32,
            info.position_x,
            info.position_y,
            info.position_z,
            info.orientation,
        ),
    })
}

#[derive(Debug, FromRow)]
struct PlayerCreateInfoRow {
    map: u16,
    zone: u32,
    position_x: f32,
    position_y: f32,
    position_z: f32,
    orientation: f32,
}

#[derive(Debug, FromRow)]
struct PlayerClassLevelStatsRow {
    #[sqlx(rename = "basehp")]
    base_health: u32,
    #[sqlx(rename = "basemana")]
    base_mana: u32,
}

#[derive(Debug, FromRow)]
struct PlayerLevelStatsRow {
    #[sqlx(rename = "str")]
    strength: u32,
    #[sqlx(rename = "agi")]
    agility: u32,
    #[sqlx(rename = "sta")]
    stamina: u32,
    #[sqlx(rename = "inte")]
    intellect: u32,
    #[sqlx(rename = "spi")]
    spirit: u32,
}

#[derive(Debug, FromRow)]
struct PlayerCreateSpellRow {
    #[sqlx(rename = "Spell")]
    spell: u32,
}

#[derive(Debug, FromRow)]
struct PlayerCreateActionRow {
    button: u16,
    action: u32,
    #[sqlx(rename = "type")]
    action_type: u16,
}

#[derive(Debug, FromRow)]
struct PlayerCreateSkillRow {
    skill: u16,
    note: Option<String>,
}

#[derive(Debug, FromRow)]
struct ItemTemplateRow {
    entry: u32,
    #[sqlx(rename = "MaxDurability")]
    max_durability: u32,
}

#[derive(Debug, Clone, Copy)]
struct StarterItem {
    item_id: u32,
    slot: u8,
    amount: u32,
}

async fn seed_character_spells(
    tx: &mut Transaction<'_, MySql>,
    world_pool: &MySqlPool,
    guid: u32,
    race: u8,
    class: u8,
) -> Result<(), DbError> {
    let spells = sqlx::query_as::<_, PlayerCreateSpellRow>(
        "SELECT Spell FROM playercreateinfo_spell WHERE race = ? AND class = ? ORDER BY Spell",
    )
    .bind(race)
    .bind(class)
    .fetch_all(world_pool)
    .await?;

    for spell in spells {
        sqlx::query(
            "INSERT INTO character_spell (guid, spell, active, disabled) VALUES (?, ?, 1, 0)",
        )
        .bind(guid)
        .bind(spell.spell)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

async fn seed_character_actions(
    tx: &mut Transaction<'_, MySql>,
    world_pool: &MySqlPool,
    guid: u32,
    race: u8,
    class: u8,
) -> Result<(), DbError> {
    let actions = sqlx::query_as::<_, PlayerCreateActionRow>(
        "SELECT button, action, type FROM playercreateinfo_action \
         WHERE race = ? AND class = ? ORDER BY button",
    )
    .bind(race)
    .bind(class)
    .fetch_all(world_pool)
    .await?;

    for action in actions {
        if action.button >= 120 || action.action_type > u8::MAX as u16 {
            continue;
        }

        sqlx::query(
            "INSERT INTO character_action (guid, button, action, type) VALUES (?, ?, ?, ?)",
        )
        .bind(guid)
        .bind(action.button as u8)
        .bind(action.action)
        .bind(action.action_type as u8)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

async fn seed_character_skills(
    tx: &mut Transaction<'_, MySql>,
    world_pool: &MySqlPool,
    guid: u32,
    race: u8,
    class: u8,
) -> Result<(), DbError> {
    let race_mask = 1u32 << (race - 1);
    let class_mask = 1u32 << (class - 1);
    let skills = sqlx::query_as::<_, PlayerCreateSkillRow>(
        "SELECT skill, note FROM playercreateinfo_skills \
         WHERE (raceMask = 0 OR (raceMask & ?) <> 0) \
           AND (classMask = 0 OR (classMask & ?) <> 0) \
         ORDER BY skill",
    )
    .bind(race_mask)
    .bind(class_mask)
    .fetch_all(world_pool)
    .await?;

    for skill in skills {
        let (value, max) = starter_skill_value(skill.note.as_deref());
        sqlx::query("INSERT INTO character_skills (guid, skill, value, max) VALUES (?, ?, ?, ?)")
            .bind(guid)
            .bind(skill.skill)
            .bind(value)
            .bind(max)
            .execute(&mut **tx)
            .await?;
    }

    Ok(())
}

fn starter_skill_value(note: Option<&str>) -> (u16, u16) {
    match note {
        Some(note) if note.starts_with("Language:") => (300, 300),
        Some(note) if note.starts_with("Misc: GENERIC") => (1, 1),
        Some(note) if note.starts_with("Armor:") => (1, 1),
        Some(note) if note.starts_with("Racial:") => (1, 1),
        _ => (1, LEVEL_ONE_SKILL_MAX),
    }
}

async fn seed_character_starter_items(
    tx: &mut Transaction<'_, MySql>,
    world_pool: &MySqlPool,
    guid: u32,
    race: u8,
    class: u8,
) -> Result<(), DbError> {
    let Some(items) = starter_outfit_items(race, class) else {
        return Ok(());
    };

    let mut equipment_cache = [0u32; ENUM_EQUIPMENT_CACHE_SLOTS];
    for starter_item in items {
        let item_id = source_backed_starter_item_id(starter_item.item_id);
        let Some(template) = get_item_template(world_pool, item_id).await? else {
            continue;
        };

        let item_guid = next_item_guid_tx(tx).await?;
        sqlx::query(
            "INSERT INTO item_instance \
             (guid, owner_guid, itemEntry, creatorGuid, giftCreatorGuid, count, duration, \
              charges, flags, enchantments, randomPropertyId, durability, itemTextId) \
             VALUES (?, ?, ?, 0, 0, ?, 0, ?, 0, ?, 0, ?, 0)",
        )
        .bind(item_guid)
        .bind(guid)
        .bind(template.entry)
        .bind(starter_item.amount)
        .bind(default_item_charges())
        .bind(default_item_enchantments())
        .bind(template.max_durability)
        .execute(&mut **tx)
        .await?;

        sqlx::query(
            "INSERT INTO character_inventory (guid, bag, slot, item, item_template) \
             VALUES (?, 0, ?, ?, ?)",
        )
        .bind(guid)
        .bind(starter_item.slot)
        .bind(item_guid)
        .bind(template.entry)
        .execute(&mut **tx)
        .await?;

        if (starter_item.slot as usize) < EQUIPMENT_SLOT_END {
            equipment_cache[starter_item.slot as usize] = template.entry;
        }
    }

    sqlx::query("UPDATE characters SET equipmentCache = ? WHERE guid = ?")
        .bind(format_equipment_cache(&equipment_cache))
        .bind(guid)
        .execute(&mut **tx)
        .await?;

    Ok(())
}

async fn get_item_template(
    world_pool: &MySqlPool,
    item_id: u32,
) -> Result<Option<ItemTemplateRow>, DbError> {
    let row = sqlx::query_as::<_, ItemTemplateRow>(
        "SELECT entry, MaxDurability FROM item_template WHERE entry = ?",
    )
    .bind(item_id)
    .fetch_optional(world_pool)
    .await?;

    Ok(row)
}

async fn next_item_guid(pool: &MySqlPool) -> Result<u32, DbError> {
    let max_guid: Option<u32> = sqlx::query_scalar("SELECT MAX(guid) FROM item_instance")
        .fetch_one(pool)
        .await?;

    Ok(max_guid.unwrap_or(0).saturating_add(1))
}

async fn next_item_guid_tx(tx: &mut Transaction<'_, MySql>) -> Result<u32, DbError> {
    let max_guid: Option<u32> = sqlx::query_scalar("SELECT MAX(guid) FROM item_instance")
        .fetch_one(&mut **tx)
        .await?;

    Ok(max_guid.unwrap_or(0).saturating_add(1))
}

const EQUIPMENT_SLOT_END: usize = 19;
const ENUM_EQUIPMENT_CACHE_SLOTS: usize = 20;

fn starter_outfit_items(race: u8, class: u8) -> Option<&'static [StarterItem]> {
    match (race, class) {
        (1, 1) => Some(&HUMAN_WARRIOR_ITEMS),
        (1, 2) => Some(&HUMAN_PALADIN_ITEMS),
        (1, 4) => Some(&HUMAN_ROGUE_ITEMS),
        (1, 5) => Some(&HUMAN_PRIEST_ITEMS),
        (1, 8) => Some(&HUMAN_MAGE_ITEMS),
        (1, 9) => Some(&HUMAN_WARLOCK_ITEMS),
        (2, 1) => Some(&ORC_WARRIOR_ITEMS),
        (2, 3) => Some(&ORC_HUNTER_ITEMS),
        (2, 4) => Some(&ORC_ROGUE_ITEMS),
        (2, 7) => Some(&ORC_SHAMAN_ITEMS),
        (2, 9) => Some(&ORC_WARLOCK_ITEMS),
        (3, 1) => Some(&DWARF_WARRIOR_ITEMS),
        (3, 2) => Some(&DWARF_PALADIN_ITEMS),
        (3, 3) => Some(&DWARF_HUNTER_ITEMS),
        (3, 4) => Some(&DWARF_ROGUE_ITEMS),
        (3, 5) => Some(&DWARF_PRIEST_ITEMS),
        (4, 1) => Some(&NIGHTELF_WARRIOR_ITEMS),
        (4, 3) => Some(&NIGHTELF_HUNTER_ITEMS),
        (4, 4) => Some(&NIGHTELF_ROGUE_ITEMS),
        (4, 5) => Some(&NIGHTELF_PRIEST_ITEMS),
        (4, 11) => Some(&NIGHTELF_DRUID_ITEMS),
        (5, 1) => Some(&UNDEAD_WARRIOR_ITEMS),
        (5, 4) => Some(&UNDEAD_ROGUE_ITEMS),
        (5, 5) => Some(&UNDEAD_PRIEST_ITEMS),
        (5, 8) => Some(&UNDEAD_MAGE_ITEMS),
        (5, 9) => Some(&UNDEAD_WARLOCK_ITEMS),
        (6, 1) => Some(&TAUREN_WARRIOR_ITEMS),
        (6, 3) => Some(&TAUREN_HUNTER_ITEMS),
        (6, 7) => Some(&TAUREN_SHAMAN_ITEMS),
        (6, 11) => Some(&TAUREN_DRUID_ITEMS),
        (7, 1) => Some(&GNOME_WARRIOR_ITEMS),
        (7, 4) => Some(&GNOME_ROGUE_ITEMS),
        (7, 8) => Some(&GNOME_MAGE_ITEMS),
        (7, 9) => Some(&GNOME_WARLOCK_ITEMS),
        (8, 1) => Some(&TROLL_WARRIOR_ITEMS),
        (8, 3) => Some(&TROLL_HUNTER_ITEMS),
        (8, 4) => Some(&TROLL_ROGUE_ITEMS),
        (8, 5) => Some(&TROLL_PRIEST_ITEMS),
        (8, 7) => Some(&TROLL_SHAMAN_ITEMS),
        (8, 8) => Some(&TROLL_MAGE_ITEMS),
        _ => None,
    }
}

pub fn starter_item_template_refs() -> Vec<StarterItemTemplateRef> {
    let mut refs = Vec::new();
    for race in 1..=8 {
        for class in [1, 2, 3, 4, 5, 7, 8, 9, 11] {
            let Some(items) = starter_outfit_items(race, class) else {
                continue;
            };
            refs.extend(items.iter().map(move |item| StarterItemTemplateRef {
                race,
                class,
                item_id: source_backed_starter_item_id(item.item_id),
                slot: item.slot,
                amount: item.amount,
            }));
        }
    }
    refs
}

fn format_equipment_cache(equipment: &[u32; ENUM_EQUIPMENT_CACHE_SLOTS]) -> String {
    let mut cache = String::new();
    for item_id in equipment {
        cache.push_str(&item_id.to_string());
        cache.push_str(" 0 ");
    }
    cache
}

fn default_item_charges() -> &'static str {
    "0 0 0 0 0 "
}

fn default_item_enchantments() -> &'static str {
    "0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 "
}

const fn item(item_id: u32, slot: u8, amount: u32) -> StarterItem {
    StarterItem {
        item_id,
        slot,
        amount,
    }
}

const fn source_backed_starter_item_id(item_id: u32) -> u32 {
    match item_id {
        129 => 6127,    // Trapper's Boots is present in the base fixture.
        65020 => 117,   // Tough Jerky
        65021 => 159,   // Refreshing Spring Water
        65022 => 117,   // Tough Jerky
        65023 => 2947,  // Small Throwing Knife
        65024 => 25861, // Crude Throwing Axe
        65025 => 117,   // Tough Jerky
        65026 => 117,   // Tough Jerky
        65027 => 117,   // Tough Jerky
        _ => item_id,
    }
}

const HUMAN_WARRIOR_ITEMS: [StarterItem; 8] = [
    item(38, 3, 1),
    item(39, 6, 1),
    item(40, 7, 1),
    item(25, 15, 1),
    item(2362, 16, 1),
    item(65020, 23, 4),
    item(6948, 24, 1),
    item(14646, 25, 1),
];
const HUMAN_PALADIN_ITEMS: [StarterItem; 7] = [
    item(45, 3, 1),
    item(44, 6, 1),
    item(43, 7, 1),
    item(2361, 15, 1),
    item(6948, 23, 1),
    item(65021, 24, 2),
    item(65022, 25, 4),
];
const HUMAN_ROGUE_ITEMS: [StarterItem; 8] = [
    item(49, 3, 1),
    item(48, 6, 1),
    item(47, 7, 1),
    item(2092, 15, 1),
    item(65023, 17, 100),
    item(65022, 23, 4),
    item(6948, 24, 1),
    item(14646, 25, 1),
];
const HUMAN_PRIEST_ITEMS: [StarterItem; 9] = [
    item(53, 3, 1),
    item(6098, 4, 1),
    item(52, 6, 1),
    item(51, 7, 1),
    item(36, 15, 1),
    item(65021, 23, 2),
    item(65022, 24, 4),
    item(6948, 25, 1),
    item(14646, 26, 1),
];
const HUMAN_MAGE_ITEMS: [StarterItem; 9] = [
    item(6096, 3, 1),
    item(56, 4, 1),
    item(1395, 6, 1),
    item(55, 7, 1),
    item(35, 15, 1),
    item(65022, 23, 4),
    item(65021, 24, 2),
    item(6948, 25, 1),
    item(14646, 26, 1),
];
const HUMAN_WARLOCK_ITEMS: [StarterItem; 9] = [
    item(6097, 3, 1),
    item(57, 4, 1),
    item(1396, 6, 1),
    item(59, 7, 1),
    item(2092, 15, 1),
    item(65027, 23, 4),
    item(65021, 24, 2),
    item(6948, 25, 1),
    item(14646, 26, 1),
];
const ORC_WARRIOR_ITEMS: [StarterItem; 7] = [
    item(6125, 3, 1),
    item(139, 6, 1),
    item(140, 7, 1),
    item(12282, 15, 1),
    item(6948, 23, 1),
    item(65020, 24, 4),
    item(14649, 25, 1),
];
const ORC_HUNTER_ITEMS: [StarterItem; 11] = [
    item(127, 3, 1),
    item(6126, 6, 1),
    item(6127, 7, 1),
    item(37, 15, 1),
    item(2504, 17, 1),
    item(65021, 23, 2),
    item(65020, 24, 4),
    item(6948, 25, 1),
    item(14649, 26, 1),
    item(2512, 27, 200),
    item(2101, 28, 1),
];
const ORC_ROGUE_ITEMS: [StarterItem; 8] = [
    item(2105, 3, 1),
    item(120, 6, 1),
    item(121, 7, 1),
    item(2092, 15, 1),
    item(65024, 17, 100),
    item(65020, 23, 4),
    item(6948, 24, 1),
    item(14649, 25, 1),
];
const ORC_SHAMAN_ITEMS: [StarterItem; 7] = [
    item(154, 3, 1),
    item(153, 6, 1),
    item(36, 15, 1),
    item(6948, 23, 1),
    item(65020, 24, 4),
    item(65021, 25, 2),
    item(14649, 26, 1),
];
const ORC_WARLOCK_ITEMS: [StarterItem; 8] = [
    item(6129, 4, 1),
    item(1396, 6, 1),
    item(59, 7, 1),
    item(2092, 15, 1),
    item(6948, 23, 1),
    item(65020, 24, 4),
    item(65021, 25, 2),
    item(14649, 26, 1),
];
const DWARF_WARRIOR_ITEMS: [StarterItem; 7] = [
    item(38, 3, 1),
    item(39, 6, 1),
    item(40, 7, 1),
    item(12282, 15, 1),
    item(6948, 23, 1),
    item(65020, 24, 4),
    item(14647, 25, 1),
];
const DWARF_PALADIN_ITEMS: [StarterItem; 8] = [
    item(45, 3, 1),
    item(44, 6, 1),
    item(43, 7, 1),
    item(2361, 15, 1),
    item(65026, 23, 4),
    item(65021, 24, 2),
    item(6948, 25, 1),
    item(14647, 26, 1),
];
const DWARF_HUNTER_ITEMS: [StarterItem; 11] = [
    item(148, 3, 1),
    item(147, 6, 1),
    item(129, 7, 1),
    item(37, 15, 1),
    item(2508, 17, 1),
    item(65021, 23, 2),
    item(65020, 24, 4),
    item(6948, 25, 1),
    item(14647, 26, 1),
    item(2516, 27, 200),
    item(2102, 28, 1),
];
const DWARF_ROGUE_ITEMS: [StarterItem; 8] = [
    item(49, 3, 1),
    item(48, 6, 1),
    item(47, 7, 1),
    item(2092, 15, 1),
    item(65024, 17, 100),
    item(65026, 23, 4),
    item(6948, 24, 1),
    item(14647, 25, 1),
];
const DWARF_PRIEST_ITEMS: [StarterItem; 9] = [
    item(53, 3, 1),
    item(6098, 4, 1),
    item(52, 6, 1),
    item(51, 7, 1),
    item(36, 15, 1),
    item(65021, 23, 2),
    item(65026, 24, 4),
    item(6948, 25, 1),
    item(14647, 26, 1),
];
const NIGHTELF_WARRIOR_ITEMS: [StarterItem; 8] = [
    item(38, 3, 1),
    item(39, 6, 1),
    item(40, 7, 1),
    item(25, 15, 1),
    item(2362, 16, 1),
    item(65020, 23, 4),
    item(6948, 24, 1),
    item(14648, 25, 1),
];
const NIGHTELF_HUNTER_ITEMS: [StarterItem; 10] = [
    item(148, 3, 1),
    item(147, 6, 1),
    item(129, 7, 1),
    item(2092, 15, 1),
    item(2504, 17, 1),
    item(65021, 23, 2),
    item(65020, 24, 4),
    item(6948, 25, 1),
    item(14648, 26, 1),
    item(2512, 27, 200),
];
const NIGHTELF_ROGUE_ITEMS: [StarterItem; 8] = [
    item(49, 3, 1),
    item(48, 6, 1),
    item(47, 7, 1),
    item(2092, 15, 1),
    item(65023, 17, 100),
    item(65026, 23, 4),
    item(6948, 24, 1),
    item(14648, 25, 1),
];
const NIGHTELF_PRIEST_ITEMS: [StarterItem; 9] = [
    item(53, 3, 1),
    item(6119, 4, 1),
    item(52, 6, 1),
    item(51, 7, 1),
    item(36, 15, 1),
    item(65022, 23, 4),
    item(65021, 24, 2),
    item(6948, 25, 1),
    item(14648, 26, 1),
];
const NIGHTELF_DRUID_ITEMS: [StarterItem; 7] = [
    item(6123, 4, 1),
    item(44, 6, 1),
    item(3661, 15, 1),
    item(65021, 23, 2),
    item(65025, 24, 4),
    item(6948, 25, 1),
    item(14648, 26, 1),
];
const UNDEAD_WARRIOR_ITEMS: [StarterItem; 8] = [
    item(6125, 3, 1),
    item(139, 6, 1),
    item(140, 7, 1),
    item(25, 15, 1),
    item(2362, 16, 1),
    item(65027, 23, 4),
    item(6948, 24, 1),
    item(14651, 25, 1),
];
const UNDEAD_ROGUE_ITEMS: [StarterItem; 8] = [
    item(2105, 3, 1),
    item(120, 6, 1),
    item(121, 7, 1),
    item(2092, 15, 1),
    item(65023, 17, 100),
    item(65027, 23, 4),
    item(6948, 24, 1),
    item(14651, 25, 1),
];
const UNDEAD_PRIEST_ITEMS: [StarterItem; 9] = [
    item(53, 3, 1),
    item(6144, 4, 1),
    item(52, 6, 1),
    item(51, 7, 1),
    item(36, 15, 1),
    item(65027, 23, 4),
    item(65021, 24, 2),
    item(6948, 25, 1),
    item(14651, 26, 1),
];
const UNDEAD_MAGE_ITEMS: [StarterItem; 9] = [
    item(6096, 3, 1),
    item(6140, 4, 1),
    item(1395, 6, 1),
    item(55, 7, 1),
    item(35, 15, 1),
    item(65027, 23, 4),
    item(65021, 24, 2),
    item(6948, 25, 1),
    item(14651, 26, 1),
];
const UNDEAD_WARLOCK_ITEMS: [StarterItem; 8] = [
    item(6129, 4, 1),
    item(1396, 6, 1),
    item(59, 7, 1),
    item(2092, 15, 1),
    item(65027, 23, 4),
    item(65021, 24, 2),
    item(6948, 25, 1),
    item(14651, 26, 1),
];
const TAUREN_WARRIOR_ITEMS: [StarterItem; 6] = [
    item(6125, 3, 1),
    item(139, 6, 1),
    item(2361, 15, 1),
    item(6948, 23, 1),
    item(65026, 24, 4),
    item(14650, 25, 1),
];
const TAUREN_HUNTER_ITEMS: [StarterItem; 10] = [
    item(127, 3, 1),
    item(6126, 6, 1),
    item(37, 15, 1),
    item(2508, 17, 1),
    item(65021, 23, 2),
    item(65020, 24, 4),
    item(6948, 25, 1),
    item(14650, 26, 1),
    item(2516, 27, 200),
    item(2102, 28, 1),
];
const TAUREN_SHAMAN_ITEMS: [StarterItem; 7] = [
    item(154, 3, 1),
    item(153, 6, 1),
    item(36, 15, 1),
    item(6948, 23, 1),
    item(65027, 24, 4),
    item(65021, 25, 2),
    item(14650, 26, 1),
];
const TAUREN_DRUID_ITEMS: [StarterItem; 7] = [
    item(6139, 4, 1),
    item(6124, 6, 1),
    item(35, 15, 1),
    item(65021, 23, 2),
    item(65025, 24, 4),
    item(6948, 25, 1),
    item(14650, 26, 1),
];
const GNOME_WARRIOR_ITEMS: [StarterItem; 8] = [
    item(38, 4, 1),
    item(39, 6, 1),
    item(40, 7, 1),
    item(25, 15, 1),
    item(2362, 16, 1),
    item(65020, 23, 4),
    item(6948, 24, 1),
    item(14647, 25, 1),
];
const GNOME_ROGUE_ITEMS: [StarterItem; 8] = [
    item(49, 3, 1),
    item(48, 6, 1),
    item(47, 7, 1),
    item(2092, 15, 1),
    item(65023, 17, 100),
    item(65020, 23, 4),
    item(6948, 24, 1),
    item(14647, 25, 1),
];
const GNOME_MAGE_ITEMS: [StarterItem; 9] = [
    item(6096, 3, 1),
    item(56, 4, 1),
    item(1395, 6, 1),
    item(55, 7, 1),
    item(35, 15, 1),
    item(65025, 23, 4),
    item(65021, 24, 2),
    item(6948, 25, 1),
    item(14647, 26, 1),
];
const GNOME_WARLOCK_ITEMS: [StarterItem; 9] = [
    item(6097, 3, 1),
    item(57, 4, 1),
    item(1396, 6, 1),
    item(59, 7, 1),
    item(2092, 15, 1),
    item(65021, 23, 2),
    item(65027, 24, 4),
    item(6948, 25, 1),
    item(14647, 26, 1),
];
const TROLL_WARRIOR_ITEMS: [StarterItem; 9] = [
    item(6125, 3, 1),
    item(139, 6, 1),
    item(140, 7, 1),
    item(37, 15, 1),
    item(2362, 16, 1),
    item(65024, 17, 100),
    item(65020, 23, 4),
    item(6948, 24, 1),
    item(14649, 25, 1),
];
const TROLL_HUNTER_ITEMS: [StarterItem; 11] = [
    item(127, 3, 1),
    item(6126, 6, 1),
    item(6127, 7, 1),
    item(37, 15, 1),
    item(2504, 17, 1),
    item(65027, 23, 4),
    item(65021, 24, 2),
    item(2512, 27, 200),
    item(2101, 28, 1),
    item(14649, 26, 1),
    item(6948, 25, 1),
];
const TROLL_ROGUE_ITEMS: [StarterItem; 8] = [
    item(2105, 3, 1),
    item(120, 6, 1),
    item(121, 7, 1),
    item(2092, 15, 1),
    item(65024, 17, 100),
    item(65020, 23, 4),
    item(6948, 24, 1),
    item(14649, 25, 1),
];
const TROLL_PRIEST_ITEMS: [StarterItem; 8] = [
    item(53, 3, 1),
    item(6144, 4, 1),
    item(52, 6, 1),
    item(36, 15, 1),
    item(65026, 23, 4),
    item(65021, 24, 2),
    item(6948, 25, 1),
    item(14649, 26, 1),
];
const TROLL_SHAMAN_ITEMS: [StarterItem; 7] = [
    item(6134, 3, 1),
    item(6135, 6, 1),
    item(36, 15, 1),
    item(65020, 23, 4),
    item(65021, 24, 2),
    item(6948, 25, 1),
    item(14649, 26, 1),
];
const TROLL_MAGE_ITEMS: [StarterItem; 9] = [
    item(6096, 3, 1),
    item(6140, 4, 1),
    item(1395, 6, 1),
    item(55, 7, 1),
    item(35, 15, 1),
    item(65020, 23, 4),
    item(65021, 24, 2),
    item(6948, 25, 1),
    item(14649, 26, 1),
];

