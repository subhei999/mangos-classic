pub async fn update_character_position(
    pool: &MySqlPool,
    account_id: u32,
    guid: u32,
    position: WorldPosition,
) -> Result<u64, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_position_save");
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

pub async fn get_character_homebind(
    pool: &MySqlPool,
    guid: u32,
) -> Result<Option<WorldPosition>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_homebind_load");
    let row = sqlx::query(
        "SELECT CAST(map AS UNSIGNED) AS map, position_x, position_y, position_z \
         FROM character_homebind WHERE guid = ?",
    )
    .bind(guid)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| {
        WorldPosition::new(
            row.get::<u32, _>("map"),
            row.get::<f32, _>("position_x"),
            row.get::<f32, _>("position_y"),
            row.get::<f32, _>("position_z"),
            0.0,
        )
    }))
}

pub async fn update_character_position_and_vitals(
    pool: &MySqlPool,
    account_id: u32,
    guid: u32,
    position: WorldPosition,
    health: u32,
    power1: u32,
    power2: u32,
) -> Result<u64, DbError> {
    let _query_timer =
        crate::observability::DbQueryTimer::start("character_position_vitals_save");
    let result = sqlx::query(
        "UPDATE characters \
         SET map = ?, position_x = ?, position_y = ?, position_z = ?, orientation = ?, \
             health = ?, power1 = ?, power2 = ? \
         WHERE guid = ? AND account = ?",
    )
    .bind(position.map_id)
    .bind(position.x)
    .bind(position.y)
    .bind(position.z)
    .bind(position.orientation)
    .bind(health)
    .bind(power1)
    .bind(power2)
    .bind(guid)
    .bind(account_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

pub async fn update_character_explored_zones(
    pool: &MySqlPool,
    guid: u32,
    explored_zones: &str,
) -> Result<u64, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_explored_zones_save");
    let result = sqlx::query("UPDATE characters SET exploredZones = ? WHERE guid = ?")
        .bind(explored_zones)
        .bind(guid)
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

pub async fn mark_character_first_login_seen(
    pool: &MySqlPool,
    account_id: u32,
    guid: u32,
) -> Result<u64, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_first_login_update");
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

pub async fn update_character_ammo_id(
    pool: &MySqlPool,
    guid: u32,
    ammo_id: u32,
) -> Result<u64, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_ammo_save");
    let result = sqlx::query("UPDATE characters SET ammoId = ? WHERE guid = ?")
        .bind(ammo_id)
        .bind(guid)
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

pub async fn get_tutorial_flags(pool: &MySqlPool, account_id: u32) -> Result<[u32; 8], DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_tutorial_load");
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
    let _query_timer = crate::observability::DbQueryTimer::start("character_tutorial_save");
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
    let _query_timer = crate::observability::DbQueryTimer::start("character_spells_load");
    let rows = sqlx::query_as::<_, CharacterSpell>(
        "SELECT spell, active, disabled FROM character_spell WHERE guid = ? ORDER BY spell",
    )
    .bind(guid)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn replace_character_spell_cooldowns(
    pool: &MySqlPool,
    guid: u32,
    cooldowns: &[CharacterSpellCooldown],
) -> Result<(), DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_spell_cooldowns_save");
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM character_spell_cooldown WHERE guid = ?")
        .bind(guid)
        .execute(&mut *tx)
        .await?;
    for cooldown in cooldowns {
        sqlx::query(
            "INSERT INTO character_spell_cooldown \
             (guid, SpellId, SpellExpireTime, Category, CategoryExpireTime, ItemId) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(guid)
        .bind(cooldown.spell_id)
        .bind(cooldown.spell_expire_time)
        .bind(cooldown.category)
        .bind(cooldown.category_expire_time)
        .bind(cooldown.item_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    Ok(())
}

pub async fn replace_character_auras(
    pool: &MySqlPool,
    guid: u32,
    auras: &[CharacterAura],
) -> Result<(), DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_auras_save");
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM character_aura WHERE guid = ?")
        .bind(guid)
        .execute(&mut *tx)
        .await?;
    for aura in auras {
        sqlx::query(
            "INSERT INTO character_aura \
             (guid, caster_guid, item_guid, spell, stackcount, remaincharges, \
              basepoints0, basepoints1, basepoints2, periodictime0, periodictime1, periodictime2, \
              maxduration, remaintime, effIndexMask) \
             VALUES (?, ?, 0, ?, ?, 0, 0, 0, 0, 0, 0, 0, ?, ?, ?)",
        )
        .bind(guid)
        .bind(aura.caster_guid)
        .bind(aura.spell)
        .bind(aura.stackcount)
        .bind(aura.maxduration)
        .bind(aura.remaintime)
        .bind(aura.eff_index_mask)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    Ok(())
}

pub async fn update_character_death_state(
    pool: &MySqlPool,
    account_id: u32,
    guid: u32,
    position: WorldPosition,
    health: u32,
    player_flags: u32,
) -> Result<u64, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_death_state_save");
    let result = sqlx::query(
        "UPDATE characters \
         SET map = ?, position_x = ?, position_y = ?, position_z = ?, orientation = ?, \
             health = ?, playerFlags = ? \
         WHERE guid = ? AND account = ?",
    )
    .bind(position.map_id)
    .bind(position.x)
    .bind(position.y)
    .bind(position.z)
    .bind(position.orientation)
    .bind(health)
    .bind(player_flags)
    .bind(guid)
    .bind(account_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

pub async fn save_player_corpse(pool: &MySqlPool, corpse: &NewPlayerCorpse) -> Result<(), DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("corpse_save");
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM corpse WHERE player = ? AND corpse_type <> 0")
        .bind(corpse.player)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO corpse \
         (guid, player, position_x, position_y, position_z, orientation, map, time, corpse_type, instance) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(corpse.guid)
    .bind(corpse.player)
    .bind(corpse.position.x)
    .bind(corpse.position.y)
    .bind(corpse.position.z)
    .bind(corpse.position.orientation)
    .bind(corpse.position.map_id)
    .bind(corpse.time)
    .bind(corpse.corpse_type)
    .bind(corpse.instance)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(())
}

pub async fn delete_player_corpse(pool: &MySqlPool, player: u32) -> Result<u64, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("corpse_delete");
    let result = sqlx::query("DELETE FROM corpse WHERE player = ? AND corpse_type <> 0")
        .bind(player)
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

pub async fn get_player_corpse(
    pool: &MySqlPool,
    player: u32,
) -> Result<Option<PlayerCorpseQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("corpse_load");
    let row = sqlx::query_as::<_, PlayerCorpseQuery>(
        "SELECT corpse.guid, corpse.player, corpse.position_x, corpse.position_y, corpse.position_z, \
                corpse.orientation, corpse.map, corpse.time, corpse.corpse_type, corpse.instance, \
                characters.race, characters.class, characters.gender, characters.playerBytes, \
                characters.playerBytes2, characters.equipmentCache, guild_member.guildid, characters.playerFlags \
         FROM corpse \
         INNER JOIN characters ON characters.guid = corpse.player \
         LEFT JOIN guild_member ON characters.guid = guild_member.guid \
         WHERE corpse.player = ? AND corpse.corpse_type <> 0 \
         LIMIT 1",
    )
    .bind(player)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn get_nearby_player_corpses(
    pool: &MySqlPool,
    map: u32,
    x: f32,
    y: f32,
    radius: f32,
    limit: u32,
) -> Result<Vec<PlayerCorpseQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("corpse_nearby_load");
    let rows = sqlx::query_as::<_, PlayerCorpseQuery>(
        "SELECT corpse.guid, corpse.player, corpse.position_x, corpse.position_y, corpse.position_z, \
                corpse.orientation, corpse.map, corpse.time, corpse.corpse_type, corpse.instance, \
                characters.race, characters.class, characters.gender, characters.playerBytes, \
                characters.playerBytes2, characters.equipmentCache, guild_member.guildid, characters.playerFlags \
         FROM corpse \
         INNER JOIN characters ON characters.guid = corpse.player \
         LEFT JOIN guild_member ON characters.guid = guild_member.guid \
         WHERE corpse.map = ? AND corpse.corpse_type <> 0 \
           AND POW(corpse.position_x - ?, 2) + POW(corpse.position_y - ?, 2) <= POW(?, 2) \
         ORDER BY POW(corpse.position_x - ?, 2) + POW(corpse.position_y - ?, 2) \
         LIMIT ?",
    )
    .bind(map)
    .bind(x)
    .bind(y)
    .bind(radius)
    .bind(x)
    .bind(y)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn learn_character_spell(
    pool: &MySqlPool,
    guid: u32,
    spell: u32,
    cost: u32,
) -> Result<Option<u32>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_spell_learn");
    let mut tx = pool.begin().await?;
    let known: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM character_spell WHERE guid = ? AND spell = ?")
            .bind(guid)
            .bind(spell)
            .fetch_one(&mut *tx)
            .await?;
    if known > 0 {
        tx.commit().await?;
        return Ok(None);
    }

    let current_money: u32 = sqlx::query_scalar("SELECT money FROM characters WHERE guid = ?")
        .bind(guid)
        .fetch_one(&mut *tx)
        .await?;
    if current_money < cost {
        tx.commit().await?;
        return Ok(None);
    }

    let new_money = current_money - cost;
    if cost > 0 {
        sqlx::query("UPDATE characters SET money = ? WHERE guid = ?")
            .bind(new_money)
            .bind(guid)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("INSERT INTO character_spell (guid, spell, active, disabled) VALUES (?, ?, 1, 0)")
        .bind(guid)
        .bind(spell)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    Ok(Some(new_money))
}

pub async fn get_character_actions(
    pool: &MySqlPool,
    guid: u32,
) -> Result<Vec<CharacterAction>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_actions_load");
    let rows = sqlx::query_as::<_, CharacterAction>(
        "SELECT button, action, type FROM character_action WHERE guid = ? ORDER BY button",
    )
    .bind(guid)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn upsert_character_action(
    pool: &MySqlPool,
    guid: u32,
    button: u8,
    action: u32,
    action_type: u8,
) -> Result<(), DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_action_save");
    sqlx::query(
        "INSERT INTO character_action (guid, button, action, type) \
         VALUES (?, ?, ?, ?) \
         ON DUPLICATE KEY UPDATE action = VALUES(action), type = VALUES(type)",
    )
    .bind(guid)
    .bind(button)
    .bind(action)
    .bind(action_type)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn delete_character_action(
    pool: &MySqlPool,
    guid: u32,
    button: u8,
) -> Result<(), DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_action_delete");
    sqlx::query("DELETE FROM character_action WHERE guid = ? AND button = ?")
        .bind(guid)
        .bind(button)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn get_character_skills(
    pool: &MySqlPool,
    guid: u32,
) -> Result<Vec<CharacterSkill>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_skills_load");
    let rows = sqlx::query_as::<_, CharacterSkill>(
        "SELECT skill, value, max FROM character_skills WHERE guid = ? ORDER BY skill",
    )
    .bind(guid)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn upsert_character_skill(
    pool: &MySqlPool,
    guid: u32,
    skill: u16,
    value: u16,
    max: u16,
) -> Result<(), DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_skill_save");
    sqlx::query(
        "INSERT INTO character_skills (guid, skill, value, max) \
         VALUES (?, ?, ?, ?) \
         ON DUPLICATE KEY UPDATE value = VALUES(value), max = VALUES(max)",
    )
    .bind(guid)
    .bind(skill)
    .bind(value)
    .bind(max)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_character_quest_statuses(
    pool: &MySqlPool,
    guid: u32,
) -> Result<Vec<CharacterQuestStatus>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_quests_load");
    let rows = sqlx::query_as::<_, CharacterQuestStatus>(
        "SELECT quest, status, rewarded, mobcount1, mobcount2, mobcount3, mobcount4 \
         FROM character_queststatus \
         WHERE guid = ? \
         ORDER BY quest",
    )
    .bind(guid)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn get_character_quest_status(
    pool: &MySqlPool,
    guid: u32,
    quest: u32,
) -> Result<Option<CharacterQuestStatus>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_quest_load");
    let row = sqlx::query_as::<_, CharacterQuestStatus>(
        "SELECT quest, status, rewarded, mobcount1, mobcount2, mobcount3, mobcount4 \
         FROM character_queststatus \
         WHERE guid = ? AND quest = ?",
    )
    .bind(guid)
    .bind(quest)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn accept_character_quest(
    pool: &MySqlPool,
    guid: u32,
    quest: u32,
) -> Result<CharacterQuestStatus, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_quest_accept");
    sqlx::query(
        "INSERT INTO character_queststatus \
         (guid, quest, status, rewarded, explored, timer, mobcount1, mobcount2, mobcount3, mobcount4, \
          itemcount1, itemcount2, itemcount3, itemcount4) \
         VALUES (?, ?, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0) \
         ON DUPLICATE KEY UPDATE \
           explored = IF(rewarded = 0 AND status = 0, 0, explored), \
           timer = IF(rewarded = 0 AND status = 0, 0, timer), \
           mobcount1 = IF(rewarded = 0 AND status = 0, 0, mobcount1), \
           mobcount2 = IF(rewarded = 0 AND status = 0, 0, mobcount2), \
           mobcount3 = IF(rewarded = 0 AND status = 0, 0, mobcount3), \
           mobcount4 = IF(rewarded = 0 AND status = 0, 0, mobcount4), \
           itemcount1 = IF(rewarded = 0 AND status = 0, 0, itemcount1), \
           itemcount2 = IF(rewarded = 0 AND status = 0, 0, itemcount2), \
           itemcount3 = IF(rewarded = 0 AND status = 0, 0, itemcount3), \
           itemcount4 = IF(rewarded = 0 AND status = 0, 0, itemcount4), \
           status = IF(rewarded = 0 AND status = 0, VALUES(status), status)",
    )
    .bind(guid)
    .bind(quest)
    .execute(pool)
    .await?;

    Ok(get_character_quest_status(pool, guid, quest)
        .await?
        .expect("accepted quest row must exist"))
}

pub async fn update_character_quest_mob_count(
    pool: &MySqlPool,
    guid: u32,
    quest: u32,
    objective_index: usize,
    count: u32,
    complete: bool,
) -> Result<CharacterQuestStatus, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_quest_update");
    let status = if complete { 1 } else { 3 };
    let column = match objective_index {
        0 => "mobcount1",
        1 => "mobcount2",
        2 => "mobcount3",
        3 => "mobcount4",
        _ => "mobcount1",
    };
    sqlx::query(&format!(
        "UPDATE character_queststatus SET {column} = ?, status = ? \
         WHERE guid = ? AND quest = ? AND rewarded = 0"
    ))
    .bind(count)
    .bind(status)
    .bind(guid)
    .bind(quest)
    .execute(pool)
    .await?;

    Ok(get_character_quest_status(pool, guid, quest)
        .await?
        .expect("updated quest row must exist"))
}

pub async fn complete_character_quest(
    pool: &MySqlPool,
    guid: u32,
    quest: u32,
) -> Result<CharacterQuestStatus, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_quest_complete");
    sqlx::query(
        "UPDATE character_queststatus \
         SET status = 1 \
         WHERE guid = ? AND quest = ? AND status = 3 AND rewarded = 0",
    )
    .bind(guid)
    .bind(quest)
    .execute(pool)
    .await?;

    Ok(get_character_quest_status(pool, guid, quest)
        .await?
        .expect("completed quest row must exist"))
}

pub async fn incomplete_character_quest(
    pool: &MySqlPool,
    guid: u32,
    quest: u32,
) -> Result<CharacterQuestStatus, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_quest_incomplete");
    sqlx::query(
        "UPDATE character_queststatus \
         SET status = 3 \
         WHERE guid = ? AND quest = ? AND status = 1 AND rewarded = 0",
    )
    .bind(guid)
    .bind(quest)
    .execute(pool)
    .await?;

    Ok(get_character_quest_status(pool, guid, quest)
        .await?
        .expect("incompleted quest row must exist"))
}

pub async fn abandon_character_quest(
    pool: &MySqlPool,
    guid: u32,
    quest: u32,
) -> Result<Option<CharacterQuestStatus>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_quest_abandon");
    let changed = sqlx::query(
        "UPDATE character_queststatus \
         SET status = 0, rewarded = 0, mobcount1 = 0, mobcount2 = 0, mobcount3 = 0, mobcount4 = 0, \
             itemcount1 = 0, itemcount2 = 0, itemcount3 = 0, itemcount4 = 0 \
         WHERE guid = ? AND quest = ? AND rewarded = 0",
    )
    .bind(guid)
    .bind(quest)
    .execute(pool)
    .await?
    .rows_affected();

    if changed == 0 {
        return Ok(None);
    }

    get_character_quest_status(pool, guid, quest).await
}

pub async fn reward_character_quest(
    pool: &MySqlPool,
    guid: u32,
    quest: u32,
    money: u32,
    reputation_rewards: &[(u32, i32)],
) -> Result<Option<CharacterQuestRewardResult>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_quest_reward");
    const FACTION_FLAG_VISIBLE: i32 = 0x01;
    const REPUTATION_CAP: i32 = 42_999;
    const REPUTATION_BOTTOM: i32 = -42_000;

    let mut tx = pool.begin().await?;
    let changed = sqlx::query(
        "UPDATE character_queststatus \
         SET status = 1, rewarded = 1 \
         WHERE guid = ? AND quest = ? AND status = 1 AND rewarded = 0",
    )
    .bind(guid)
    .bind(quest)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    if changed == 0 {
        tx.commit().await?;
        return Ok(None);
    }

    let current_money: u32 = sqlx::query_scalar("SELECT money FROM characters WHERE guid = ?")
        .bind(guid)
        .fetch_one(&mut *tx)
        .await?;
    let new_money = current_money.saturating_add(money);
    sqlx::query("UPDATE characters SET money = ? WHERE guid = ?")
        .bind(new_money)
        .bind(guid)
        .execute(&mut *tx)
        .await?;

    let mut reputations = Vec::new();
    for (faction, delta) in reputation_rewards {
        if *faction == 0 || *delta == 0 {
            continue;
        }
        let existing = sqlx::query_as::<_, CharacterReputation>(
            "SELECT faction, standing, flags FROM character_reputation \
             WHERE guid = ? AND faction = ?",
        )
        .bind(guid)
        .bind(*faction)
        .fetch_optional(&mut *tx)
        .await?;
        let (current_standing, current_flags) = existing
            .as_ref()
            .map(|row| (row.standing, row.flags))
            .unwrap_or((0, 0));
        let standing = current_standing
            .saturating_add(*delta)
            .clamp(REPUTATION_BOTTOM, REPUTATION_CAP);
        let flags = current_flags | FACTION_FLAG_VISIBLE;
        let applied_delta = standing.saturating_sub(current_standing);
        sqlx::query(
            "INSERT INTO character_reputation (guid, faction, standing, flags) \
             VALUES (?, ?, ?, ?) \
             ON DUPLICATE KEY UPDATE standing = VALUES(standing), flags = VALUES(flags)",
        )
        .bind(guid)
        .bind(*faction)
        .bind(standing)
        .bind(flags)
        .execute(&mut *tx)
        .await?;
        reputations.push(CharacterReputationChange {
            reputation: CharacterReputation {
                faction: *faction,
                standing,
                flags,
            },
            delta: applied_delta,
        });
    }
    tx.commit().await?;

    Ok(Some(CharacterQuestRewardResult {
        money: new_money,
        reputations,
    }))
}

pub async fn get_character_reputations(
    pool: &MySqlPool,
    guid: u32,
) -> Result<Vec<CharacterReputation>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("character_reputation_load");
    let rows = sqlx::query_as::<_, CharacterReputation>(
        "SELECT faction, standing, flags FROM character_reputation WHERE guid = ? ORDER BY faction",
    )
    .bind(guid)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}
