use serde::{Deserialize, Serialize};
use sqlx::mysql::MySqlPool;
use sqlx::{FromRow, Row};
use wow_common::position::WorldPosition;

use crate::pool::DbError;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CharacterEnumEntry {
    pub guid: u32,
    pub name: String,
    pub race: u8,
    pub class: u8,
    pub gender: u8,
    #[sqlx(rename = "playerBytes")]
    pub player_bytes: u32,
    #[sqlx(rename = "playerBytes2")]
    pub player_bytes2: u32,
    pub level: u8,
    pub zone: u32,
    pub map: u32,
    pub position_x: f32,
    pub position_y: f32,
    pub position_z: f32,
    pub orientation: f32,
    pub guildid: Option<u32>,
    #[sqlx(rename = "playerFlags")]
    pub player_flags: u32,
    pub at_login: u32,
    pub money: u32,
    pub cinematic: u8,
    pub health: u32,
    pub power1: u32,
    pub power2: u32,
    pub power3: u32,
    pub power4: u32,
    pub power5: u32,
    #[sqlx(rename = "watchedFaction")]
    pub watched_faction: u32,
    #[sqlx(rename = "exploredZones")]
    pub explored_zones: Option<String>,
    pub pet_entry: Option<u32>,
    pub pet_modelid: Option<u32>,
    pub pet_level: Option<u32>,
    #[sqlx(rename = "equipmentCache")]
    pub equipment_cache: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewCharacter {
    pub account_id: u32,
    pub name: String,
    pub race: u8,
    pub class: u8,
    pub gender: u8,
    pub skin: u8,
    pub face: u8,
    pub hair_style: u8,
    pub hair_color: u8,
    pub facial_hair: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreatedCharacter {
    pub guid: u32,
    pub account_id: u32,
    pub name: String,
    pub race: u8,
    pub class: u8,
    pub position: WorldPosition,
    pub zone: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterDeleteMethod {
    HardDelete,
    Unlink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharacterDeleteOptions {
    pub method: CharacterDeleteMethod,
    pub min_level_for_unlink: u8,
    pub force_hard_delete: bool,
}

impl CharacterDeleteOptions {
    pub fn hard_delete() -> Self {
        Self {
            method: CharacterDeleteMethod::HardDelete,
            min_level_for_unlink: 0,
            force_hard_delete: true,
        }
    }
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CharacterNameQuery {
    pub guid: u32,
    pub name: String,
    pub race: u8,
    pub gender: u8,
    pub class: u8,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CharacterSpell {
    pub spell: u32,
    pub active: u8,
    pub disabled: u8,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CharacterAction {
    pub button: u8,
    pub action: u32,
    #[sqlx(rename = "type")]
    pub action_type: u8,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CharacterReputation {
    pub faction: u32,
    pub standing: i32,
    pub flags: i32,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CharacterInventoryItem {
    pub bag: u32,
    pub slot: u8,
    pub item: u32,
    pub item_template: u32,
    pub count: u32,
    pub durability: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryDestroyResult {
    Removed { item: u32 },
    CountChanged { item: u32, count: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryMoveResult {
    Swapped,
    Merged {
        source_item: u32,
        source_count: Option<u32>,
        destination_item: u32,
        destination_count: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InventorySplitResult {
    pub source_item: u32,
    pub source_count: u32,
    pub new_item: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemTemplateQuery {
    pub entry: u32,
    pub class: u32,
    pub subclass: u32,
    pub name: String,
    pub displayid: u32,
    pub quality: u32,
    pub flags: u32,
    pub buy_price: u32,
    pub sell_price: u32,
    pub inventory_type: u32,
    pub allowable_class: i32,
    pub allowable_race: i32,
    pub item_level: u32,
    pub required_level: u32,
    pub required_skill: u32,
    pub required_skill_rank: u32,
    pub required_spell: u32,
    pub required_honor_rank: u32,
    pub required_city_rank: u32,
    pub required_reputation_faction: u32,
    pub required_reputation_rank: u32,
    pub max_count: u32,
    pub stackable: u32,
    pub container_slots: u32,
    pub dmg_min1: f32,
    pub dmg_max1: f32,
    pub dmg_type1: u32,
    pub armor: u32,
    pub holy_res: u32,
    pub fire_res: u32,
    pub nature_res: u32,
    pub frost_res: u32,
    pub shadow_res: u32,
    pub arcane_res: u32,
    pub delay: u32,
    pub ammo_type: u32,
    pub ranged_mod_range: f32,
    pub bonding: u32,
    pub description: String,
    pub page_text: u32,
    pub language_id: u32,
    pub page_material: u32,
    pub start_quest: u32,
    pub lock_id: u32,
    pub material: i32,
    pub sheath: u32,
    pub random_property: u32,
    pub block: u32,
    pub itemset: u32,
    pub max_durability: u32,
    pub area: u32,
    pub map: i32,
    pub bag_family: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerWorldStats {
    pub base_health: u32,
    pub base_mana: u32,
    pub stats: [u32; 5],
    pub next_level_xp: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRow, Serialize, Deserialize)]
pub struct CharacterSkill {
    pub skill: u16,
    pub value: u16,
    pub max: u16,
}

impl PlayerWorldStats {
    pub fn max_health(self) -> u32 {
        self.base_health + health_bonus_from_stamina(self.stats[2])
    }

    pub fn max_mana(self) -> u32 {
        if self.base_mana == 0 {
            return 0;
        }

        self.base_mana + mana_bonus_from_intellect(self.stats[3])
    }
}

pub async fn get_character_enum_entries(
    pool: &MySqlPool,
    account_id: u32,
) -> Result<Vec<CharacterEnumEntry>, DbError> {
    let rows = sqlx::query_as::<_, CharacterEnumEntry>(
        "SELECT characters.guid, characters.name, characters.race, characters.class, \
                characters.gender, characters.playerBytes, characters.playerBytes2, \
                characters.level, characters.zone, characters.map, \
                characters.position_x, characters.position_y, characters.position_z, \
                characters.orientation, \
                guild_member.guildid, characters.playerFlags, characters.at_login, \
                characters.money, characters.cinematic, \
                characters.health, characters.power1, characters.power2, \
                characters.power3, characters.power4, characters.power5, \
                characters.watchedFaction, characters.exploredZones, \
                character_pet.entry AS pet_entry, character_pet.modelid AS pet_modelid, \
                character_pet.level AS pet_level, characters.equipmentCache \
         FROM characters \
         LEFT JOIN character_pet ON characters.guid = character_pet.owner AND character_pet.slot = 0 \
         LEFT JOIN guild_member ON characters.guid = guild_member.guid \
         WHERE characters.account = ? \
         ORDER BY characters.guid",
    )
    .bind(account_id)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn get_character_name_query(
    pool: &MySqlPool,
    guid: u32,
) -> Result<Option<CharacterNameQuery>, DbError> {
    let row = sqlx::query_as::<_, CharacterNameQuery>(
        "SELECT guid, name, race, gender, class FROM characters WHERE guid = ?",
    )
    .bind(guid)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn character_name_exists(pool: &MySqlPool, name: &str) -> Result<bool, DbError> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM characters WHERE name = ?")
        .bind(name)
        .fetch_one(pool)
        .await?;

    Ok(count > 0)
}

pub async fn character_count_for_account(pool: &MySqlPool, account_id: u32) -> Result<u8, DbError> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM characters WHERE account = ?")
        .bind(account_id)
        .fetch_one(pool)
        .await?;

    Ok(count.min(u8::MAX as i64) as u8)
}

pub async fn is_guild_leader(pool: &MySqlPool, guid: u32) -> Result<bool, DbError> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM guild WHERE leaderguid = ?")
        .bind(guid)
        .fetch_one(pool)
        .await?;

    Ok(count > 0)
}

pub async fn delete_character(
    pool: &MySqlPool,
    account_id: u32,
    guid: u32,
) -> Result<bool, DbError> {
    delete_character_with_options(
        pool,
        account_id,
        guid,
        CharacterDeleteOptions::hard_delete(),
    )
    .await
}

pub async fn delete_character_with_options(
    pool: &MySqlPool,
    account_id: u32,
    guid: u32,
    options: CharacterDeleteOptions,
) -> Result<bool, DbError> {
    let row = sqlx::query("SELECT account, name, level FROM characters WHERE guid = ?")
        .bind(guid)
        .fetch_optional(pool)
        .await?;
    let Some(row) = row else {
        return Ok(false);
    };
    let owner: u32 = row.try_get("account")?;
    let name: String = row.try_get("name")?;
    let level: u8 = row.try_get("level")?;
    if owner != account_id {
        return Ok(false);
    }

    let should_unlink = options.method == CharacterDeleteMethod::Unlink
        && !options.force_hard_delete
        && level >= options.min_level_for_unlink;
    if should_unlink {
        let result = sqlx::query(
            "UPDATE characters \
             SET deleteInfos_Name = ?, deleteInfos_Account = ?, deleteDate = UNIX_TIMESTAMP(), \
                 name = '', account = 0 \
             WHERE guid = ? AND account = ?",
        )
        .bind(name)
        .bind(account_id)
        .bind(guid)
        .bind(account_id)
        .execute(pool)
        .await?;
        return Ok(result.rows_affected() > 0);
    }

    let item_guids: Vec<u32> =
        sqlx::query_scalar("SELECT item FROM character_inventory WHERE guid = ?")
            .bind(guid)
            .fetch_all(pool)
            .await?;
    let pet_ids: Vec<u32> = sqlx::query_scalar("SELECT id FROM character_pet WHERE owner = ?")
        .bind(guid)
        .fetch_all(pool)
        .await?;

    cleanup_character_group(pool, guid).await?;
    cleanup_character_pets(pool, &pet_ids).await?;
    return_cod_mail_to_senders(pool, account_id, guid).await?;

    for table in [
        "character_account_data",
        "character_action",
        "character_aura",
        "character_battleground_data",
        "character_homebind",
        "character_honor_cp",
        "character_instance",
        "character_inventory",
        "character_queststatus",
        "character_queststatus_weekly",
        "character_reputation",
        "character_skills",
        "character_forgotten_skills",
        "character_spell",
        "character_spell_cooldown",
        "character_stats",
    ] {
        sqlx::query(&format!("DELETE FROM {table} WHERE guid = ?"))
            .bind(guid)
            .execute(pool)
            .await?;
    }

    sqlx::query("DELETE FROM character_social WHERE guid = ? OR friend = ?")
        .bind(guid)
        .bind(guid)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM guild_member WHERE guid = ?")
        .bind(guid)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM guild_eventlog WHERE PlayerGuid1 = ? OR PlayerGuid2 = ?")
        .bind(guid)
        .bind(guid)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM character_pet WHERE owner = ?")
        .bind(guid)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM mail WHERE receiver = ?")
        .bind(guid)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM mail_items WHERE receiver = ?")
        .bind(guid)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM item_instance WHERE owner_guid = ?")
        .bind(guid)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM auction WHERE itemowner = ? OR buyguid = ?")
        .bind(guid)
        .bind(guid)
        .execute(pool)
        .await?;

    for item_guid in item_guids {
        sqlx::query("DELETE FROM item_instance WHERE guid = ?")
            .bind(item_guid)
            .execute(pool)
            .await?;
    }

    let result = sqlx::query("DELETE FROM characters WHERE guid = ? AND account = ?")
        .bind(guid)
        .bind(account_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

async fn cleanup_character_pets(pool: &MySqlPool, pet_ids: &[u32]) -> Result<(), DbError> {
    for pet_id in pet_ids {
        sqlx::query("DELETE FROM pet_aura WHERE guid = ?")
            .bind(pet_id)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM pet_spell WHERE guid = ?")
            .bind(pet_id)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM pet_spell_cooldown WHERE guid = ?")
            .bind(pet_id)
            .execute(pool)
            .await?;
    }

    Ok(())
}

async fn return_cod_mail_to_senders(
    pool: &MySqlPool,
    deleted_account_id: u32,
    deleted_guid: u32,
) -> Result<(), DbError> {
    let rows = sqlx::query(
        "SELECT id, messageType, mailTemplateId, sender, subject, itemTextId, money, has_items \
         FROM mail WHERE receiver = ? AND has_items <> 0 AND cod <> 0",
    )
    .bind(deleted_guid)
    .fetch_all(pool)
    .await?;

    for row in rows {
        let mail_id: u32 = row.try_get("id")?;
        let message_type: u8 = row.try_get("messageType")?;
        let mail_template_id: u32 = row.try_get("mailTemplateId")?;
        let sender_guid: u32 = row.try_get("sender")?;
        let subject: Option<String> = row.try_get("subject")?;
        let item_text_id: u32 = row.try_get("itemTextId")?;
        let money: u32 = row.try_get("money")?;
        let has_items: u8 = row.try_get("has_items")?;

        let item_guids: Vec<i32> =
            sqlx::query_scalar("SELECT item_guid FROM mail_items WHERE mail_id = ?")
                .bind(mail_id)
                .fetch_all(pool)
                .await?;

        sqlx::query("DELETE FROM mail WHERE id = ?")
            .bind(mail_id)
            .execute(pool)
            .await?;

        if message_type != MAIL_NORMAL {
            if has_items != 0 {
                sqlx::query("DELETE FROM mail_items WHERE mail_id = ?")
                    .bind(mail_id)
                    .execute(pool)
                    .await?;
            }
            continue;
        }

        let sender_account_id: Option<u32> =
            sqlx::query_scalar("SELECT account FROM characters WHERE guid = ?")
                .bind(sender_guid)
                .fetch_optional(pool)
                .await?;

        let Some(sender_account_id) = sender_account_id else {
            for item_guid in item_guids {
                sqlx::query("DELETE FROM item_instance WHERE guid = ?")
                    .bind(item_guid as u32)
                    .execute(pool)
                    .await?;
            }
            sqlx::query("DELETE FROM mail_items WHERE mail_id = ?")
                .bind(mail_id)
                .execute(pool)
                .await?;
            continue;
        };

        let new_mail_id = next_mail_id_above(pool, mail_id).await?;
        let deliver_delay = if sender_account_id != deleted_account_id {
            DEFAULT_MAIL_DELIVERY_DELAY_SECS
        } else {
            0
        };

        sqlx::query(
            "INSERT INTO mail \
             (id, messageType, stationery, mailTemplateId, sender, receiver, subject, itemTextId, \
              has_items, expire_time, deliver_time, money, cod, checked) \
             VALUES (?, ?, 41, ?, ?, ?, ?, ?, ?, UNIX_TIMESTAMP() + ?, UNIX_TIMESTAMP() + ?, ?, 0, ?)",
        )
        .bind(new_mail_id)
        .bind(MAIL_NORMAL)
        .bind(mail_template_id)
        .bind(deleted_guid)
        .bind(sender_guid)
        .bind(subject.unwrap_or_default())
        .bind(item_text_id)
        .bind((!item_guids.is_empty()) as u8)
        .bind(DEFAULT_MAIL_EXPIRY_SECS)
        .bind(deliver_delay)
        .bind(money)
        .bind(MAIL_CHECK_MASK_RETURNED)
        .execute(pool)
        .await?;

        for item_guid in item_guids {
            let item_template: u32 =
                sqlx::query_scalar("SELECT itemEntry FROM item_instance WHERE guid = ?")
                    .bind(item_guid as u32)
                    .fetch_one(pool)
                    .await?;

            sqlx::query("DELETE FROM mail_items WHERE mail_id = ? AND item_guid = ?")
                .bind(mail_id)
                .bind(item_guid)
                .execute(pool)
                .await?;
            sqlx::query(
                "INSERT INTO mail_items (mail_id, item_guid, item_template, receiver) \
                 VALUES (?, ?, ?, ?)",
            )
            .bind(new_mail_id)
            .bind(item_guid)
            .bind(item_template)
            .bind(sender_guid)
            .execute(pool)
            .await?;
            sqlx::query("UPDATE item_instance SET owner_guid = ? WHERE guid = ?")
                .bind(sender_guid)
                .bind(item_guid as u32)
                .execute(pool)
                .await?;
        }
    }

    Ok(())
}

async fn next_mail_id_above(pool: &MySqlPool, minimum: u32) -> Result<u32, DbError> {
    let max_id: Option<u32> = sqlx::query_scalar("SELECT MAX(id) FROM mail")
        .fetch_one(pool)
        .await?;

    Ok(max_id.unwrap_or(0).max(minimum).saturating_add(1))
}

async fn cleanup_character_group(pool: &MySqlPool, guid: u32) -> Result<(), DbError> {
    let Some(group_id) =
        sqlx::query_scalar::<_, u32>("SELECT groupId FROM group_member WHERE memberGuid = ?")
            .bind(guid)
            .fetch_optional(pool)
            .await?
    else {
        sqlx::query("DELETE FROM group_instance WHERE leaderGuid = ?")
            .bind(guid)
            .execute(pool)
            .await?;
        return Ok(());
    };

    let member_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM group_member WHERE groupId = ?")
            .bind(group_id)
            .fetch_one(pool)
            .await?;

    if member_count <= 2 {
        sqlx::query("DELETE FROM `groups` WHERE groupId = ?")
            .bind(group_id)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM group_member WHERE groupId = ?")
            .bind(group_id)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM group_instance WHERE leaderGuid = ?")
            .bind(guid)
            .execute(pool)
            .await?;
        return Ok(());
    }

    let leader_guid: Option<u32> =
        sqlx::query_scalar("SELECT leaderGuid FROM `groups` WHERE groupId = ?")
            .bind(group_id)
            .fetch_optional(pool)
            .await?;

    if leader_guid == Some(guid) {
        if let Some(new_leader) = sqlx::query_scalar::<_, u32>(
            "SELECT memberGuid FROM group_member WHERE groupId = ? AND memberGuid <> ? ORDER BY memberGuid LIMIT 1",
        )
        .bind(group_id)
        .bind(guid)
        .fetch_optional(pool)
        .await?
        {
            sqlx::query("UPDATE `groups` SET leaderGuid = ? WHERE groupId = ?")
                .bind(new_leader)
                .bind(group_id)
                .execute(pool)
                .await?;
            sqlx::query("UPDATE group_instance SET leaderGuid = ? WHERE leaderGuid = ?")
                .bind(new_leader)
                .bind(guid)
                .execute(pool)
                .await?;
        }
    }

    sqlx::query("DELETE FROM group_member WHERE memberGuid = ?")
        .bind(guid)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM group_instance WHERE leaderGuid = ?")
        .bind(guid)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn create_character(
    character_pool: &MySqlPool,
    world_pool: &MySqlPool,
    character: NewCharacter,
) -> Result<CreatedCharacter, DbError> {
    let guid = next_character_guid(character_pool).await?;
    let create_info = get_player_create_info(world_pool, character.race, character.class).await?;
    let world_stats =
        get_player_world_stats(world_pool, character.race, character.class, 1).await?;
    let player_bytes = player_bytes(
        character.skin,
        character.face,
        character.hair_style,
        character.hair_color,
    );
    let player_bytes2 = character.facial_hair as u32;

    sqlx::query(
        "INSERT INTO characters \
         (guid, account, name, race, class, gender, level, zone, map, \
          position_x, position_y, position_z, orientation, playerBytes, \
          playerBytes2, playerFlags, at_login, equipmentCache, taximask, taxi_path, \
          exploredZones, health, power1, watchedFaction) \
         VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, '', '', '', '', ?, ?, ?)",
    )
    .bind(guid)
    .bind(character.account_id)
    .bind(&character.name)
    .bind(character.race)
    .bind(character.class)
    .bind(character.gender)
    .bind(create_info.zone)
    .bind(create_info.position.map_id)
    .bind(create_info.position.x)
    .bind(create_info.position.y)
    .bind(create_info.position.z)
    .bind(create_info.position.orientation)
    .bind(player_bytes)
    .bind(player_bytes2)
    .bind(AT_LOGIN_FIRST)
    .bind(world_stats.max_health())
    .bind(world_stats.max_mana())
    .bind(u32::MAX)
    .execute(character_pool)
    .await?;

    sqlx::query(
        "INSERT INTO character_homebind (guid, map, zone, position_x, position_y, position_z) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(guid)
    .bind(create_info.position.map_id)
    .bind(create_info.zone)
    .bind(create_info.position.x)
    .bind(create_info.position.y)
    .bind(create_info.position.z)
    .execute(character_pool)
    .await?;

    seed_character_spells(
        character_pool,
        world_pool,
        guid,
        character.race,
        character.class,
    )
    .await?;
    seed_character_actions(
        character_pool,
        world_pool,
        guid,
        character.race,
        character.class,
    )
    .await?;
    seed_character_skills(
        character_pool,
        world_pool,
        guid,
        character.race,
        character.class,
    )
    .await?;
    seed_character_starter_items(
        character_pool,
        world_pool,
        guid,
        character.race,
        character.class,
    )
    .await?;

    Ok(CreatedCharacter {
        guid,
        account_id: character.account_id,
        name: character.name,
        race: character.race,
        class: character.class,
        position: create_info.position,
        zone: create_info.zone,
    })
}

pub async fn update_character_position(
    pool: &MySqlPool,
    account_id: u32,
    guid: u32,
    position: WorldPosition,
) -> Result<u64, DbError> {
    let result = sqlx::query(
        "UPDATE characters \
         SET map = ?, position_x = ?, position_y = ?, position_z = ?, orientation = ? \
         WHERE guid = ? AND account = ?",
    )
    .bind(position.map_id)
    .bind(position.x)
    .bind(position.y)
    .bind(position.z)
    .bind(position.orientation)
    .bind(guid)
    .bind(account_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

pub async fn mark_character_first_login_seen(
    pool: &MySqlPool,
    account_id: u32,
    guid: u32,
) -> Result<u64, DbError> {
    let result = sqlx::query(
        "UPDATE characters SET cinematic = 1, at_login = at_login & ? \
         WHERE guid = ? AND account = ?",
    )
    .bind(u32::MAX ^ AT_LOGIN_FIRST)
    .bind(guid)
    .bind(account_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

pub async fn get_tutorial_flags(pool: &MySqlPool, account_id: u32) -> Result<[u32; 8], DbError> {
    let Some(row) = sqlx::query(
        "SELECT tut0, tut1, tut2, tut3, tut4, tut5, tut6, tut7 \
         FROM character_tutorial WHERE account = ?",
    )
    .bind(account_id)
    .fetch_optional(pool)
    .await?
    else {
        return Ok([0; 8]);
    };

    Ok([
        row.try_get("tut0")?,
        row.try_get("tut1")?,
        row.try_get("tut2")?,
        row.try_get("tut3")?,
        row.try_get("tut4")?,
        row.try_get("tut5")?,
        row.try_get("tut6")?,
        row.try_get("tut7")?,
    ])
}

pub async fn save_tutorial_flags(
    pool: &MySqlPool,
    account_id: u32,
    tutorials: [u32; 8],
) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO character_tutorial \
         (account, tut0, tut1, tut2, tut3, tut4, tut5, tut6, tut7) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
         ON DUPLICATE KEY UPDATE \
         tut0 = VALUES(tut0), tut1 = VALUES(tut1), tut2 = VALUES(tut2), \
         tut3 = VALUES(tut3), tut4 = VALUES(tut4), tut5 = VALUES(tut5), \
         tut6 = VALUES(tut6), tut7 = VALUES(tut7)",
    )
    .bind(account_id)
    .bind(tutorials[0])
    .bind(tutorials[1])
    .bind(tutorials[2])
    .bind(tutorials[3])
    .bind(tutorials[4])
    .bind(tutorials[5])
    .bind(tutorials[6])
    .bind(tutorials[7])
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_character_spells(
    pool: &MySqlPool,
    guid: u32,
) -> Result<Vec<CharacterSpell>, DbError> {
    let rows = sqlx::query_as::<_, CharacterSpell>(
        "SELECT spell, active, disabled FROM character_spell WHERE guid = ? ORDER BY spell",
    )
    .bind(guid)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn get_character_actions(
    pool: &MySqlPool,
    guid: u32,
) -> Result<Vec<CharacterAction>, DbError> {
    let rows = sqlx::query_as::<_, CharacterAction>(
        "SELECT button, action, type FROM character_action WHERE guid = ? ORDER BY button",
    )
    .bind(guid)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn get_character_skills(
    pool: &MySqlPool,
    guid: u32,
) -> Result<Vec<CharacterSkill>, DbError> {
    let rows = sqlx::query_as::<_, CharacterSkill>(
        "SELECT skill, value, max FROM character_skills WHERE guid = ? ORDER BY skill",
    )
    .bind(guid)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn get_character_reputations(
    pool: &MySqlPool,
    guid: u32,
) -> Result<Vec<CharacterReputation>, DbError> {
    let rows = sqlx::query_as::<_, CharacterReputation>(
        "SELECT faction, standing, flags FROM character_reputation WHERE guid = ? ORDER BY faction",
    )
    .bind(guid)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn get_character_inventory_items(
    pool: &MySqlPool,
    guid: u32,
) -> Result<Vec<CharacterInventoryItem>, DbError> {
    let rows = sqlx::query_as::<_, CharacterInventoryItem>(
        "SELECT ci.bag, ci.slot, ci.item, ci.item_template, ii.count, ii.durability \
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

pub async fn add_character_inventory_item(
    pool: &MySqlPool,
    guid: u32,
    bag: u32,
    slot: u8,
    item_template: u32,
    count: u32,
    durability: u32,
) -> Result<CharacterInventoryItem, DbError> {
    let item_guid = next_item_guid(pool).await?;
    sqlx::query(
        "INSERT INTO item_instance \
         (guid, owner_guid, itemEntry, creatorGuid, giftCreatorGuid, count, duration, \
          charges, flags, enchantments, randomPropertyId, durability, itemTextId) \
         VALUES (?, ?, ?, 0, 0, ?, 0, '0 0 0 0 0 ', 0, \
                 '0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 ', 0, ?, 0)",
    )
    .bind(item_guid)
    .bind(guid)
    .bind(item_template)
    .bind(count)
    .bind(durability)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO character_inventory (guid, bag, slot, item, item_template) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(guid)
    .bind(bag)
    .bind(slot)
    .bind(item_guid)
    .bind(item_template)
    .execute(pool)
    .await?;

    Ok(CharacterInventoryItem {
        bag,
        slot,
        item: item_guid,
        item_template,
        count,
        durability,
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
         dmg_min1, dmg_max1, dmg_type1, armor, holy_res, fire_res, nature_res, frost_res, \
         shadow_res, arcane_res, delay, ammo_type, RangedModRange, bonding, description, \
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

pub async fn get_player_world_stats(
    world_pool: &MySqlPool,
    race: u8,
    class: u8,
    level: u8,
) -> Result<PlayerWorldStats, DbError> {
    let class_stats = sqlx::query_as::<_, PlayerClassLevelStatsRow>(
        "SELECT basehp, basemana FROM player_classlevelstats WHERE class = ? AND level = ?",
    )
    .bind(class)
    .bind(level)
    .fetch_one(world_pool)
    .await?;

    let level_stats = sqlx::query_as::<_, PlayerLevelStatsRow>(
        "SELECT str, agi, sta, inte, spi FROM player_levelstats \
         WHERE race = ? AND class = ? AND level = ?",
    )
    .bind(race)
    .bind(class)
    .bind(level)
    .fetch_one(world_pool)
    .await?;
    let next_level_xp = get_player_next_level_xp(world_pool, level).await?;

    Ok(PlayerWorldStats {
        base_health: class_stats.base_health,
        base_mana: class_stats.base_mana,
        stats: [
            level_stats.strength,
            level_stats.agility,
            level_stats.stamina,
            level_stats.intellect,
            level_stats.spirit,
        ],
        next_level_xp,
    })
}

async fn get_player_next_level_xp(world_pool: &MySqlPool, level: u8) -> Result<u32, DbError> {
    let xp = sqlx::query_scalar("SELECT xp_for_next_level FROM player_xp_for_level WHERE lvl = ?")
        .bind(level)
        .fetch_optional(world_pool)
        .await?;

    Ok(xp.unwrap_or(0))
}

fn health_bonus_from_stamina(stamina: u32) -> u32 {
    let base = stamina.min(20);
    let more = stamina.saturating_sub(base);
    base + more * 10
}

fn mana_bonus_from_intellect(intellect: u32) -> u32 {
    let base = intellect.min(20);
    let more = intellect.saturating_sub(base);
    base + more * 15
}

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
    character_pool: &MySqlPool,
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
        .execute(character_pool)
        .await?;
    }

    Ok(())
}

async fn seed_character_actions(
    character_pool: &MySqlPool,
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
        .execute(character_pool)
        .await?;
    }

    Ok(())
}

async fn seed_character_skills(
    character_pool: &MySqlPool,
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
            .execute(character_pool)
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
    character_pool: &MySqlPool,
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
        let Some(template) = get_item_template(world_pool, starter_item.item_id).await? else {
            continue;
        };

        let item_guid = next_item_guid(character_pool).await?;
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
        .execute(character_pool)
        .await?;

        sqlx::query(
            "INSERT INTO character_inventory (guid, bag, slot, item, item_template) \
             VALUES (?, 0, ?, ?, ?)",
        )
        .bind(guid)
        .bind(starter_item.slot)
        .bind(item_guid)
        .bind(template.entry)
        .execute(character_pool)
        .await?;

        if (starter_item.slot as usize) < EQUIPMENT_SLOT_END {
            equipment_cache[starter_item.slot as usize] = template.entry;
        }
    }

    sqlx::query("UPDATE characters SET equipmentCache = ? WHERE guid = ?")
        .bind(format_equipment_cache(&equipment_cache))
        .bind(guid)
        .execute(character_pool)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_bytes_match_cmangos_layout() {
        assert_eq!(player_bytes(1, 2, 3, 4), 0x0403_0201);
    }

    #[test]
    fn starter_skill_values_match_basic_cmangos_ranges() {
        assert_eq!(starter_skill_value(Some("Language: Common")), (300, 300));
        assert_eq!(starter_skill_value(Some("Armor: Cloth")), (1, 1));
        assert_eq!(starter_skill_value(Some("Warrior: Arms")), (1, 5));
    }

    #[test]
    fn player_world_stats_apply_cmangos_stamina_and_intellect_bonuses() {
        let warrior = PlayerWorldStats {
            base_health: 20,
            base_mana: 0,
            stats: [23, 20, 22, 20, 21],
            next_level_xp: 400,
        };
        let mage = PlayerWorldStats {
            base_health: 31,
            base_mana: 100,
            stats: [15, 23, 19, 26, 22],
            next_level_xp: 400,
        };

        assert_eq!(warrior.max_health(), 60);
        assert_eq!(warrior.max_mana(), 0);
        assert_eq!(mage.max_health(), 50);
        assert_eq!(mage.max_mana(), 210);
    }

    #[test]
    fn human_warrior_outfit_matches_archived_cmangos_rows() {
        let items = starter_outfit_items(1, 1).unwrap();

        assert_eq!(items[0].item_id, 38);
        assert_eq!(items[0].slot, 3);
        assert!(items
            .iter()
            .any(|item| item.item_id == 25 && item.slot == 15));
        assert!(items
            .iter()
            .any(|item| item.item_id == 2362 && item.slot == 16));
    }

    #[test]
    fn non_human_starter_outfit_rows_cover_existing_race_class_pairs() {
        let cases: &[(u8, u8, u32, u8)] = &[
            (2, 1, 6125, 3),
            (2, 3, 127, 3),
            (3, 2, 45, 3),
            (4, 11, 6123, 4),
            (5, 8, 6096, 3),
            (7, 1, 38, 4),
            (8, 7, 6134, 3),
        ];

        for (race, class, item_id, slot) in cases {
            let items = starter_outfit_items(*race, *class)
                .unwrap_or_else(|| panic!("missing starter outfit for {race}/{class}"));
            assert!(items
                .iter()
                .any(|item| item.item_id == *item_id && item.slot == *slot));
            assert!(!items.is_empty());
        }
    }

    #[test]
    fn equipment_cache_uses_item_id_enchant_pairs() {
        let mut equipment = [0u32; ENUM_EQUIPMENT_CACHE_SLOTS];
        equipment[3] = 38;
        equipment[15] = 25;

        let cache = format_equipment_cache(&equipment);

        assert!(cache.starts_with("0 0 0 0 0 0 38 0"));
        assert_eq!(
            cache.split_whitespace().count(),
            ENUM_EQUIPMENT_CACHE_SLOTS * 2
        );
    }
}
