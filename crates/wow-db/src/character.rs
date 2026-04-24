use serde::{Deserialize, Serialize};
use sqlx::mysql::MySqlPool;
use sqlx::FromRow;
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

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CharacterNameQuery {
    pub guid: u32,
    pub name: String,
    pub race: u8,
    pub gender: u8,
    pub class: u8,
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

pub async fn create_character(
    pool: &MySqlPool,
    character: NewCharacter,
) -> Result<CreatedCharacter, DbError> {
    let guid = next_character_guid(pool).await?;
    let create_info = player_create_info(character.race, character.class);
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
          exploredZones, health, power1) \
         VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, '', '', '', '', 1, 0)",
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
    .execute(pool)
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
    .execute(pool)
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

fn player_create_info(race: u8, class: u8) -> PlayerCreateInfo {
    match race {
        1 => PlayerCreateInfo {
            zone: 12,
            position: WorldPosition::new(0, -8949.95, -132.493, 83.5312, 0.0),
        },
        2 | 8 => PlayerCreateInfo {
            zone: 14,
            position: WorldPosition::new(1, -618.518, -4251.67, 38.718, 0.0),
        },
        3 | 7 => PlayerCreateInfo {
            zone: 1,
            position: WorldPosition::new(0, -6240.32, 331.033, 382.758, 0.0),
        },
        4 => PlayerCreateInfo {
            zone: 141,
            position: WorldPosition::new(1, 10311.3, 832.463, 1326.41, 0.0),
        },
        5 => PlayerCreateInfo {
            zone: 85,
            position: WorldPosition::new(0, 1676.35, 1677.45, 121.67, 0.0),
        },
        6 => PlayerCreateInfo {
            zone: 215,
            position: WorldPosition::new(1, -2917.58, -257.98, 52.9968, 0.0),
        },
        _ => {
            let _ = class;
            PlayerCreateInfo {
                zone: 12,
                position: WorldPosition::new(0, -8949.95, -132.493, 83.5312, 0.0),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_bytes_match_cmangos_layout() {
        assert_eq!(player_bytes(1, 2, 3, 4), 0x0403_0201);
    }

    #[test]
    fn human_create_info_matches_seed_position() {
        let info = player_create_info(1, 1);

        assert_eq!(info.zone, 12);
        assert_eq!(info.position.map_id, 0);
        assert_eq!(info.position.x, -8949.95);
    }
}
