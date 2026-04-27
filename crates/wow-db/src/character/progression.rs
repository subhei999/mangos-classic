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

pub async fn get_player_next_level_xp(world_pool: &MySqlPool, level: u8) -> Result<u32, DbError> {
    let xp = sqlx::query_scalar("SELECT xp_for_next_level FROM player_xp_for_level WHERE lvl = ?")
        .bind(level)
        .fetch_optional(world_pool)
        .await?;

    Ok(xp.unwrap_or(0))
}

pub async fn update_character_progression_state(
    pool: &MySqlPool,
    guid: u32,
    state: CharacterProgressionState,
) -> Result<(), DbError> {
    sqlx::query(
        "UPDATE characters \
         SET level = ?, xp = ?, health = ?, power1 = ?, power2 = ?, power3 = ?, power4 = ?, power5 = ? \
         WHERE guid = ?",
    )
    .bind(state.level)
    .bind(state.xp)
    .bind(state.health)
    .bind(state.power1)
    .bind(state.power2)
    .bind(state.power3)
    .bind(state.power4)
    .bind(state.power5)
    .bind(guid)
    .execute(pool)
    .await?;

    Ok(())
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

