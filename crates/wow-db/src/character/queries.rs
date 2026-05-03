pub async fn get_character_enum_entries(
    pool: &MySqlPool,
    account_id: u32,
) -> Result<Vec<CharacterEnumEntry>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_enum");
    let rows = sqlx::query_as::<_, CharacterEnumEntry>(
        "SELECT characters.guid, characters.name, characters.race, characters.class, \
                characters.gender, characters.playerBytes, characters.playerBytes2, \
                characters.level, characters.xp, characters.zone, characters.map, \
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
    let _query_timer = crate::observability::DbQueryTimer::start("character_name");
    let row = sqlx::query_as::<_, CharacterNameQuery>(
        "SELECT guid, name, race, gender, class FROM characters WHERE guid = ?",
    )
    .bind(guid)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn character_name_exists(pool: &MySqlPool, name: &str) -> Result<bool, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_name_exists");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM characters WHERE name = ?")
        .bind(name)
        .fetch_one(pool)
        .await?;

    Ok(count > 0)
}

pub async fn character_count_for_account(pool: &MySqlPool, account_id: u32) -> Result<u8, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_count");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM characters WHERE account = ?")
        .bind(account_id)
        .fetch_one(pool)
        .await?;

    Ok(count.min(u8::MAX as i64) as u8)
}

pub async fn is_guild_leader(pool: &MySqlPool, guid: u32) -> Result<bool, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("guild_leader_check");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM guild WHERE leaderguid = ?")
        .bind(guid)
        .fetch_one(pool)
        .await?;

    Ok(count > 0)
}

