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

    let mut tx = pool.begin().await?;

    let item_guids: Vec<u32> =
        sqlx::query_scalar("SELECT item FROM character_inventory WHERE guid = ?")
            .bind(guid)
            .fetch_all(&mut *tx)
            .await?;
    let pet_ids: Vec<u32> = sqlx::query_scalar("SELECT id FROM character_pet WHERE owner = ?")
        .bind(guid)
        .fetch_all(&mut *tx)
        .await?;

    cleanup_character_group(&mut tx, guid).await?;
    cleanup_character_pets(&mut tx, &pet_ids).await?;
    return_cod_mail_to_senders(&mut tx, account_id, guid).await?;

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
            .execute(&mut *tx)
            .await?;
    }

    sqlx::query("DELETE FROM character_social WHERE guid = ? OR friend = ?")
        .bind(guid)
        .bind(guid)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM guild_member WHERE guid = ?")
        .bind(guid)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM guild_eventlog WHERE PlayerGuid1 = ? OR PlayerGuid2 = ?")
        .bind(guid)
        .bind(guid)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM character_pet WHERE owner = ?")
        .bind(guid)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM mail WHERE receiver = ?")
        .bind(guid)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM mail_items WHERE receiver = ?")
        .bind(guid)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM item_instance WHERE owner_guid = ?")
        .bind(guid)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM auction WHERE itemowner = ? OR buyguid = ?")
        .bind(guid)
        .bind(guid)
        .execute(&mut *tx)
        .await?;

    for item_guid in item_guids {
        sqlx::query("DELETE FROM item_instance WHERE guid = ?")
            .bind(item_guid)
            .execute(&mut *tx)
            .await?;
    }

    let result = sqlx::query("DELETE FROM characters WHERE guid = ? AND account = ?")
        .bind(guid)
        .bind(account_id)
        .execute(&mut *tx)
        .await?;
    let deleted = result.rows_affected() > 0;
    tx.commit().await?;
    Ok(deleted)
}

async fn cleanup_character_pets(
    tx: &mut Transaction<'_, MySql>,
    pet_ids: &[u32],
) -> Result<(), DbError> {
    for pet_id in pet_ids {
        sqlx::query("DELETE FROM pet_aura WHERE guid = ?")
            .bind(pet_id)
            .execute(&mut **tx)
            .await?;
        sqlx::query("DELETE FROM pet_spell WHERE guid = ?")
            .bind(pet_id)
            .execute(&mut **tx)
            .await?;
        sqlx::query("DELETE FROM pet_spell_cooldown WHERE guid = ?")
            .bind(pet_id)
            .execute(&mut **tx)
            .await?;
    }

    Ok(())
}

async fn return_cod_mail_to_senders(
    tx: &mut Transaction<'_, MySql>,
    deleted_account_id: u32,
    deleted_guid: u32,
) -> Result<(), DbError> {
    let rows = sqlx::query(
        "SELECT id, messageType, mailTemplateId, sender, subject, itemTextId, money, has_items \
         FROM mail WHERE receiver = ? AND has_items <> 0 AND cod <> 0",
    )
    .bind(deleted_guid)
    .fetch_all(&mut **tx)
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
                .fetch_all(&mut **tx)
                .await?;

        sqlx::query("DELETE FROM mail WHERE id = ?")
            .bind(mail_id)
            .execute(&mut **tx)
            .await?;

        if message_type != MAIL_NORMAL {
            if has_items != 0 {
                sqlx::query("DELETE FROM mail_items WHERE mail_id = ?")
                    .bind(mail_id)
                    .execute(&mut **tx)
                    .await?;
            }
            continue;
        }

        let sender_account_id: Option<u32> =
            sqlx::query_scalar("SELECT account FROM characters WHERE guid = ?")
                .bind(sender_guid)
                .fetch_optional(&mut **tx)
                .await?;

        let Some(sender_account_id) = sender_account_id else {
            for item_guid in item_guids {
                sqlx::query("DELETE FROM item_instance WHERE guid = ?")
                    .bind(item_guid as u32)
                    .execute(&mut **tx)
                    .await?;
            }
            sqlx::query("DELETE FROM mail_items WHERE mail_id = ?")
                .bind(mail_id)
                .execute(&mut **tx)
                .await?;
            continue;
        };

        let new_mail_id = next_mail_id_above(tx, mail_id).await?;
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
        .execute(&mut **tx)
        .await?;

        for item_guid in item_guids {
            let item_template: u32 =
                sqlx::query_scalar("SELECT itemEntry FROM item_instance WHERE guid = ?")
                    .bind(item_guid as u32)
                    .fetch_one(&mut **tx)
                    .await?;

            sqlx::query("DELETE FROM mail_items WHERE mail_id = ? AND item_guid = ?")
                .bind(mail_id)
                .bind(item_guid)
                .execute(&mut **tx)
                .await?;
            sqlx::query(
                "INSERT INTO mail_items (mail_id, item_guid, item_template, receiver) \
                 VALUES (?, ?, ?, ?)",
            )
            .bind(new_mail_id)
            .bind(item_guid)
            .bind(item_template)
            .bind(sender_guid)
            .execute(&mut **tx)
            .await?;
            sqlx::query("UPDATE item_instance SET owner_guid = ? WHERE guid = ?")
                .bind(sender_guid)
                .bind(item_guid as u32)
                .execute(&mut **tx)
                .await?;
        }
    }

    Ok(())
}

async fn next_mail_id_above(
    tx: &mut Transaction<'_, MySql>,
    minimum: u32,
) -> Result<u32, DbError> {
    let max_id: Option<u32> = sqlx::query_scalar("SELECT MAX(id) FROM mail")
        .fetch_one(&mut **tx)
        .await?;

    Ok(max_id.unwrap_or(0).max(minimum).saturating_add(1))
}

async fn cleanup_character_group(
    tx: &mut Transaction<'_, MySql>,
    guid: u32,
) -> Result<(), DbError> {
    let Some(group_id) =
        sqlx::query_scalar::<_, u32>("SELECT groupId FROM group_member WHERE memberGuid = ?")
            .bind(guid)
            .fetch_optional(&mut **tx)
            .await?
    else {
        sqlx::query("DELETE FROM group_instance WHERE leaderGuid = ?")
            .bind(guid)
            .execute(&mut **tx)
            .await?;
        return Ok(());
    };

    let member_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM group_member WHERE groupId = ?")
            .bind(group_id)
            .fetch_one(&mut **tx)
            .await?;

    if member_count <= 2 {
        sqlx::query("DELETE FROM `groups` WHERE groupId = ?")
            .bind(group_id)
            .execute(&mut **tx)
            .await?;
        sqlx::query("DELETE FROM group_member WHERE groupId = ?")
            .bind(group_id)
            .execute(&mut **tx)
            .await?;
        sqlx::query("DELETE FROM group_instance WHERE leaderGuid = ?")
            .bind(guid)
            .execute(&mut **tx)
            .await?;
        return Ok(());
    }

    let leader_guid: Option<u32> =
        sqlx::query_scalar("SELECT leaderGuid FROM `groups` WHERE groupId = ?")
            .bind(group_id)
            .fetch_optional(&mut **tx)
            .await?;

    if leader_guid == Some(guid) {
        if let Some(new_leader) = sqlx::query_scalar::<_, u32>(
            "SELECT memberGuid FROM group_member WHERE groupId = ? AND memberGuid <> ? ORDER BY memberGuid LIMIT 1",
        )
        .bind(group_id)
        .bind(guid)
        .fetch_optional(&mut **tx)
        .await?
        {
            sqlx::query("UPDATE `groups` SET leaderGuid = ? WHERE groupId = ?")
                .bind(new_leader)
                .bind(group_id)
                .execute(&mut **tx)
                .await?;
            sqlx::query("UPDATE group_instance SET leaderGuid = ? WHERE leaderGuid = ?")
                .bind(new_leader)
                .bind(guid)
                .execute(&mut **tx)
                .await?;
        }
    }

    sqlx::query("DELETE FROM group_member WHERE memberGuid = ?")
        .bind(guid)
        .execute(&mut **tx)
        .await?;
    sqlx::query("DELETE FROM group_instance WHERE leaderGuid = ?")
        .bind(guid)
        .execute(&mut **tx)
        .await?;

    Ok(())
}

