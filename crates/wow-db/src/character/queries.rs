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
                characters.money, characters.cinematic, characters.ammoId, \
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

pub async fn get_global_account_data(
    pool: &MySqlPool,
    account_id: u32,
) -> Result<Vec<AccountDataEntry>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("account_data_global");
    let rows = sqlx::query_as::<_, AccountDataEntry>(
        "SELECT type, time, data FROM account_data WHERE account = ?",
    )
    .bind(account_id)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn get_character_account_data(
    pool: &MySqlPool,
    character_guid: u32,
) -> Result<Vec<AccountDataEntry>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("account_data_character");
    let rows = sqlx::query_as::<_, AccountDataEntry>(
        "SELECT type, time, data FROM character_account_data WHERE guid = ?",
    )
    .bind(character_guid)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn get_character_spell_cooldowns(
    pool: &MySqlPool,
    guid: u32,
) -> Result<Vec<CharacterSpellCooldown>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_spell_cooldowns_load");
    let rows = sqlx::query_as::<_, CharacterSpellCooldown>(
        "SELECT SpellId, SpellExpireTime, Category, CategoryExpireTime, ItemId \
         FROM character_spell_cooldown WHERE guid = ?",
    )
    .bind(guid)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn get_character_auras(
    pool: &MySqlPool,
    guid: u32,
) -> Result<Vec<CharacterAura>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_auras_load");
    let rows = sqlx::query_as::<_, CharacterAura>(
        "SELECT spell, caster_guid, stackcount, maxduration, remaintime, effIndexMask \
         FROM character_aura WHERE guid = ?",
    )
    .bind(guid)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn replace_global_account_data(
    pool: &MySqlPool,
    account_id: u32,
    data_type: u32,
    data: &[u8],
) -> Result<(), DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("account_data_global_replace");
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM account_data WHERE account = ? AND type = ?")
        .bind(account_id)
        .bind(data_type)
        .execute(&mut *tx)
        .await?;
    if !data.is_empty() {
        sqlx::query("INSERT INTO account_data (account, type, time, data) VALUES (?, ?, UNIX_TIMESTAMP(), ?)")
            .bind(account_id)
            .bind(data_type)
            .bind(data)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn replace_character_account_data(
    pool: &MySqlPool,
    character_guid: u32,
    data_type: u32,
    data: &[u8],
) -> Result<(), DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("account_data_character_replace");
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM character_account_data WHERE guid = ? AND type = ?")
        .bind(character_guid)
        .bind(data_type)
        .execute(&mut *tx)
        .await?;
    if !data.is_empty() {
        sqlx::query("INSERT INTO character_account_data (guid, type, time, data) VALUES (?, ?, UNIX_TIMESTAMP(), ?)")
            .bind(character_guid)
            .bind(data_type)
            .bind(data)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

