const REST_STATE_NORMAL: u8 = 2;

pub async fn create_character(
    character_pool: &MySqlPool,
    world_pool: &MySqlPool,
    character: NewCharacter,
) -> Result<CreatedCharacter, DbError> {
    let create_info = get_player_create_info(world_pool, character.race, character.class).await?;
    let world_stats =
        get_player_world_stats(world_pool, character.race, character.class, 1).await?;
    let player_bytes = player_bytes(
        character.skin,
        character.face,
        character.hair_style,
        character.hair_color,
    );
    let player_bytes2 = character.facial_hair as u32 | ((REST_STATE_NORMAL as u32) << 24);
    let mut tx = character_pool.begin().await?;
    let guid = next_character_guid_tx(&mut tx).await?;

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
    .execute(&mut *tx)
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
    .execute(&mut *tx)
    .await?;

    seed_character_spells(&mut tx, world_pool, guid, character.race, character.class)
    .await?;
    seed_character_actions(&mut tx, world_pool, guid, character.race, character.class)
    .await?;
    seed_character_skills(&mut tx, world_pool, guid, character.race, character.class)
    .await?;
    seed_character_starter_items(&mut tx, world_pool, guid, character.race, character.class)
    .await?;
    tx.commit().await?;

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

