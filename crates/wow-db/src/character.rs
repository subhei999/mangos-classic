use serde::{Deserialize, Serialize};
use sqlx::mysql::MySqlPool;
use sqlx::FromRow;

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
