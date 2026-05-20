#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailRecipient {
    pub guid: u32,
    pub account: u32,
    pub race: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterMail {
    pub id: u32,
    pub message_type: u8,
    pub stationery: u8,
    pub mail_template_id: u32,
    pub sender: u32,
    pub receiver: u32,
    pub subject: String,
    pub item_text_id: u32,
    pub has_items: u8,
    pub expire_time: u64,
    pub deliver_time: u64,
    pub money: u32,
    pub cod: u32,
    pub checked: u8,
    pub items: Vec<CharacterMailItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterMailItem {
    pub item_guid: u32,
    pub item_template: u32,
    pub count: u32,
    pub charges: String,
    pub enchantments: String,
    pub random_property_id: i16,
    pub durability: u32,
    pub max_durability: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MailAttachmentInstanceState {
    pub flags: u32,
    pub duration: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendCharacterMailRequest {
    pub sender: u32,
    pub receiver: u32,
    pub subject: String,
    pub item_text_id: u32,
    pub money: u32,
    pub cod: u32,
    pub checked: u8,
    pub deliver_delay_secs: u64,
    pub expire_delay_secs: u64,
    pub attached_item_guid: Option<u32>,
    pub stationery: u8,
    pub message_type: u8,
    pub mail_template_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SendCharacterMailResult {
    pub mail_id: u32,
    pub sender_money: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendCharacterMailError {
    NotEnoughMoney,
    MissingAttachment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailStoreTarget {
    EmptySlot { bag: u32, slot: u8 },
    MergeStack { item_guid: u32, new_count: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TakeMailItemRequest {
    pub receiver: u32,
    pub mail_id: u32,
    pub item_guid: u32,
    pub store_target: MailStoreTarget,
    pub cod_sender: Option<u32>,
    pub cod_expire_delay_secs: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TakeMailItemError {
    MailUnavailable,
    ItemUnavailable,
    NotEnoughMoney,
}

pub async fn find_mail_recipient_by_name(
    pool: &MySqlPool,
    name: &str,
) -> Result<Option<MailRecipient>, DbError> {
    let row = sqlx::query(
        "SELECT guid, account, race FROM characters WHERE LOWER(name) = LOWER(?) LIMIT 1",
    )
    .bind(name)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| MailRecipient {
        guid: row.get("guid"),
        account: row.get("account"),
        race: row.get("race"),
    }))
}

pub async fn mail_count_for_receiver(pool: &MySqlPool, receiver: u32) -> Result<u32, DbError> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mail WHERE receiver = ?")
        .bind(receiver)
        .fetch_one(pool)
        .await?;
    Ok(count.min(u32::MAX as i64) as u32)
}

pub async fn load_delivered_mail(
    pool: &MySqlPool,
    receiver: u32,
    limit: u32,
) -> Result<Vec<CharacterMail>, DbError> {
    let rows = sqlx::query(
        "SELECT id, messageType, stationery, mailTemplateId, sender, receiver, subject, \
                itemTextId, has_items, expire_time, deliver_time, money, cod, checked \
         FROM mail WHERE receiver = ? AND deliver_time <= UNIX_TIMESTAMP() \
         ORDER BY id DESC LIMIT ?",
    )
    .bind(receiver)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    let mut mails = Vec::with_capacity(rows.len());
    for row in rows {
        let mut mail = row_to_character_mail(row)?;
        mail.items = load_mail_items(pool, mail.id, receiver).await?;
        mails.push(mail);
    }
    Ok(mails)
}

pub async fn load_mail(
    pool: &MySqlPool,
    receiver: u32,
    mail_id: u32,
) -> Result<Option<CharacterMail>, DbError> {
    let Some(row) = sqlx::query(
        "SELECT id, messageType, stationery, mailTemplateId, sender, receiver, subject, \
                itemTextId, has_items, expire_time, deliver_time, money, cod, checked \
         FROM mail WHERE id = ? AND receiver = ?",
    )
    .bind(mail_id)
    .bind(receiver)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };
    let mut mail = row_to_character_mail(row)?;
    mail.items = load_mail_items(pool, mail.id, receiver).await?;
    Ok(Some(mail))
}

pub async fn create_item_text(pool: &MySqlPool, text: &str) -> Result<u32, DbError> {
    if text.is_empty() {
        return Ok(0);
    }
    let id = next_item_text_id(pool).await?;
    sqlx::query("INSERT INTO item_text (id, text) VALUES (?, ?)")
        .bind(id)
        .bind(text)
        .execute(pool)
        .await?;
    Ok(id)
}

pub async fn get_item_text(pool: &MySqlPool, item_text_id: u32) -> Result<Option<String>, DbError> {
    let text = sqlx::query_scalar("SELECT text FROM item_text WHERE id = ?")
        .bind(item_text_id)
        .fetch_optional(pool)
        .await?;
    Ok(text)
}

pub async fn mail_attachment_instance_state(
    pool: &MySqlPool,
    owner_guid: u32,
    item_guid: u32,
) -> Result<Option<MailAttachmentInstanceState>, DbError> {
    let row = sqlx::query(
        "SELECT CAST(flags AS UNSIGNED) AS flags, duration \
         FROM item_instance WHERE guid = ? AND owner_guid = ?",
    )
    .bind(item_guid)
    .bind(owner_guid)
    .fetch_optional(pool)
    .await?;
    row.map(|row| {
        Ok(MailAttachmentInstanceState {
            flags: row.try_get("flags")?,
            duration: row.try_get("duration")?,
        })
    })
    .transpose()
}

pub async fn inventory_items_in_container(
    pool: &MySqlPool,
    owner_guid: u32,
    bag_guid: u32,
) -> Result<u32, DbError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM character_inventory WHERE guid = ? AND bag = ?",
    )
    .bind(owner_guid)
    .bind(bag_guid)
    .fetch_one(pool)
    .await?;
    Ok(count.min(u32::MAX as i64) as u32)
}

pub async fn send_character_mail(
    pool: &MySqlPool,
    request: SendCharacterMailRequest,
    charge_sender: u32,
) -> Result<SendCharacterMailResult, SendCharacterMailError> {
    let mut tx = pool.begin().await.map_err(|_| SendCharacterMailError::MissingAttachment)?;
    let spent = sqlx::query("UPDATE characters SET money = money - ? WHERE guid = ? AND money >= ?")
        .bind(charge_sender)
        .bind(request.sender)
        .bind(charge_sender)
        .execute(&mut *tx)
        .await
        .map_err(|_| SendCharacterMailError::MissingAttachment)?;
    if spent.rows_affected() == 0 {
        return Err(SendCharacterMailError::NotEnoughMoney);
    }
    let sender_money: u32 = sqlx::query_scalar("SELECT money FROM characters WHERE guid = ?")
        .bind(request.sender)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| SendCharacterMailError::MissingAttachment)?;
    let mail_id = next_mail_id_in_tx(&mut tx).await.map_err(|_| SendCharacterMailError::MissingAttachment)?;
    insert_mail_in_tx(&mut tx, mail_id, &request)
        .await
        .map_err(|_| SendCharacterMailError::MissingAttachment)?;
    if let Some(item_guid) = request.attached_item_guid {
        let deleted = sqlx::query("DELETE FROM character_inventory WHERE guid = ? AND item = ?")
            .bind(request.sender)
            .bind(item_guid)
            .execute(&mut *tx)
            .await
            .map_err(|_| SendCharacterMailError::MissingAttachment)?;
        if deleted.rows_affected() == 0 {
            return Err(SendCharacterMailError::MissingAttachment);
        }
        let item_template: Option<u32> =
            sqlx::query_scalar("SELECT itemEntry FROM item_instance WHERE guid = ?")
                .bind(item_guid)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|_| SendCharacterMailError::MissingAttachment)?;
        let Some(item_template) = item_template else {
            return Err(SendCharacterMailError::MissingAttachment);
        };
        sqlx::query("UPDATE item_instance SET owner_guid = ? WHERE guid = ?")
            .bind(request.receiver)
            .bind(item_guid)
            .execute(&mut *tx)
            .await
            .map_err(|_| SendCharacterMailError::MissingAttachment)?;
        sqlx::query(
            "INSERT INTO mail_items (mail_id, item_guid, item_template, receiver) VALUES (?, ?, ?, ?)",
        )
        .bind(mail_id)
        .bind(item_guid)
        .bind(item_template)
        .bind(request.receiver)
        .execute(&mut *tx)
        .await
        .map_err(|_| SendCharacterMailError::MissingAttachment)?;
    }
    tx.commit()
        .await
        .map_err(|_| SendCharacterMailError::MissingAttachment)?;
    Ok(SendCharacterMailResult {
        mail_id,
        sender_money,
    })
}

pub async fn update_mail_checked_mask(
    pool: &MySqlPool,
    receiver: u32,
    mail_id: u32,
    checked_mask: u8,
) -> Result<(), DbError> {
    sqlx::query("UPDATE mail SET checked = checked | ? WHERE id = ? AND receiver = ?")
        .bind(checked_mask)
        .bind(mail_id)
        .bind(receiver)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_mail_read(pool: &MySqlPool, receiver: u32, mail_id: u32) -> Result<(), DbError> {
    update_mail_checked_mask(pool, receiver, mail_id, 0x01).await
}

pub async fn delete_mail(pool: &MySqlPool, receiver: u32, mail_id: u32) -> Result<bool, DbError> {
    let mut tx = pool.begin().await?;
    let cod: Option<u32> = sqlx::query_scalar("SELECT cod FROM mail WHERE id = ? AND receiver = ?")
        .bind(mail_id)
        .bind(receiver)
        .fetch_optional(&mut *tx)
        .await?;
    let Some(cod) = cod else {
        tx.commit().await?;
        return Ok(true);
    };
    if cod != 0 {
        tx.commit().await?;
        return Ok(false);
    }
    let item_guids: Vec<u32> = sqlx::query_scalar("SELECT item_guid FROM mail_items WHERE mail_id = ?")
        .bind(mail_id)
        .fetch_all(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM mail_items WHERE mail_id = ?")
        .bind(mail_id)
        .execute(&mut *tx)
        .await?;
    for item_guid in item_guids {
        sqlx::query("DELETE FROM item_instance WHERE guid = ?")
            .bind(item_guid)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("DELETE FROM mail WHERE id = ? AND receiver = ?")
        .bind(mail_id)
        .bind(receiver)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(true)
}

pub async fn take_mail_money(
    pool: &MySqlPool,
    receiver: u32,
    mail_id: u32,
) -> Result<Option<u32>, DbError> {
    let mut tx = pool.begin().await?;
    let row: Option<(u32, u64)> =
        sqlx::query_as("SELECT money, deliver_time FROM mail WHERE id = ? AND receiver = ?")
            .bind(mail_id)
            .bind(receiver)
            .fetch_optional(&mut *tx)
            .await?;
    let Some((money, deliver_time)) = row else {
        tx.commit().await?;
        return Ok(None);
    };
    let now = current_unix_secs();
    if deliver_time > now || money == 0 {
        tx.commit().await?;
        return Ok(None);
    }
    sqlx::query("UPDATE mail SET money = 0 WHERE id = ? AND receiver = ?")
        .bind(mail_id)
        .bind(receiver)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE characters SET money = money + ? WHERE guid = ?")
        .bind(money)
        .bind(receiver)
        .execute(&mut *tx)
        .await?;
    let new_money: u32 = sqlx::query_scalar("SELECT money FROM characters WHERE guid = ?")
        .bind(receiver)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Some(new_money))
}

pub async fn take_mail_item(
    pool: &MySqlPool,
    request: TakeMailItemRequest,
) -> Result<(), TakeMailItemError> {
    let mut tx = pool.begin().await.map_err(|_| TakeMailItemError::MailUnavailable)?;
    let mail: Option<(u32, u64, String)> =
        sqlx::query_as("SELECT cod, deliver_time, subject FROM mail WHERE id = ? AND receiver = ?")
            .bind(request.mail_id)
            .bind(request.receiver)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|_| TakeMailItemError::MailUnavailable)?;
    let Some((cod, deliver_time, subject)) = mail else {
        return Err(TakeMailItemError::MailUnavailable);
    };
    if deliver_time > current_unix_secs() {
        return Err(TakeMailItemError::MailUnavailable);
    }
    if cod != 0 {
        let spent = sqlx::query("UPDATE characters SET money = money - ? WHERE guid = ? AND money >= ?")
            .bind(cod)
            .bind(request.receiver)
            .bind(cod)
            .execute(&mut *tx)
            .await
            .map_err(|_| TakeMailItemError::MailUnavailable)?;
        if spent.rows_affected() == 0 {
            return Err(TakeMailItemError::NotEnoughMoney);
        }
    }
    let item_row: Option<(u32, u32)> =
        sqlx::query_as("SELECT item_guid, item_template FROM mail_items WHERE mail_id = ? AND item_guid = ? AND receiver = ?")
            .bind(request.mail_id)
            .bind(request.item_guid)
            .bind(request.receiver)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|_| TakeMailItemError::MailUnavailable)?;
    let Some((item_guid, _item_template)) = item_row else {
        return Err(TakeMailItemError::ItemUnavailable);
    };
    match request.store_target {
        MailStoreTarget::EmptySlot { bag, slot } => {
            sqlx::query(
                "INSERT INTO character_inventory (guid, bag, slot, item, item_template) \
                 SELECT ?, ?, ?, guid, itemEntry FROM item_instance WHERE guid = ?",
            )
            .bind(request.receiver)
            .bind(bag)
            .bind(slot)
            .bind(item_guid)
            .execute(&mut *tx)
            .await
            .map_err(|_| TakeMailItemError::ItemUnavailable)?;
        }
        MailStoreTarget::MergeStack {
            item_guid: destination,
            new_count,
        } => {
            sqlx::query("UPDATE item_instance SET count = ? WHERE guid = ? AND owner_guid = ?")
                .bind(new_count)
                .bind(destination)
                .bind(request.receiver)
                .execute(&mut *tx)
                .await
                .map_err(|_| TakeMailItemError::ItemUnavailable)?;
            sqlx::query("DELETE FROM item_instance WHERE guid = ?")
                .bind(item_guid)
                .execute(&mut *tx)
                .await
                .map_err(|_| TakeMailItemError::ItemUnavailable)?;
        }
    }
    sqlx::query("DELETE FROM mail_items WHERE mail_id = ? AND item_guid = ?")
        .bind(request.mail_id)
        .bind(item_guid)
        .execute(&mut *tx)
        .await
        .map_err(|_| TakeMailItemError::MailUnavailable)?;
    sqlx::query("UPDATE mail SET has_items = 0, cod = 0 WHERE id = ? AND receiver = ?")
        .bind(request.mail_id)
        .bind(request.receiver)
        .execute(&mut *tx)
        .await
        .map_err(|_| TakeMailItemError::MailUnavailable)?;
    if cod != 0 {
        if let Some(cod_sender) = request.cod_sender.filter(|sender| *sender != 0) {
            let cod_mail_id =
                next_mail_id_in_tx(&mut tx).await.map_err(|_| TakeMailItemError::MailUnavailable)?;
            let cod_request = SendCharacterMailRequest {
                sender: request.receiver,
                receiver: cod_sender,
                subject,
                item_text_id: 0,
                money: cod,
                cod: 0,
                checked: 0x08,
                deliver_delay_secs: 0,
                expire_delay_secs: request.cod_expire_delay_secs,
                attached_item_guid: None,
                stationery: 41,
                message_type: 0,
                mail_template_id: 0,
            };
            insert_mail_in_tx(&mut tx, cod_mail_id, &cod_request)
                .await
                .map_err(|_| TakeMailItemError::MailUnavailable)?;
        }
    }
    tx.commit().await.map_err(|_| TakeMailItemError::MailUnavailable)?;
    Ok(())
}

pub async fn return_mail_to_sender(
    pool: &MySqlPool,
    receiver: u32,
    mail_id: u32,
    receiver_account: u32,
    default_delivery_delay_secs: u64,
) -> Result<bool, DbError> {
    let Some(mail) = load_mail(pool, receiver, mail_id).await? else {
        return Ok(false);
    };
    if mail.deliver_time > current_unix_secs() {
        return Ok(false);
    }
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM mail WHERE id = ? AND receiver = ?")
        .bind(mail_id)
        .bind(receiver)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM mail_items WHERE mail_id = ?")
        .bind(mail_id)
        .execute(&mut *tx)
        .await?;
    if mail.message_type == 0 && mail.sender != 0 {
        let sender_account: Option<u32> =
            sqlx::query_scalar("SELECT account FROM characters WHERE guid = ?")
                .bind(mail.sender)
                .fetch_optional(&mut *tx)
                .await?;
        if let Some(sender_account) = sender_account {
            let returned_id = next_mail_id_in_tx(&mut tx).await?;
            for item in &mail.items {
                sqlx::query("UPDATE item_instance SET owner_guid = ? WHERE guid = ?")
                    .bind(mail.sender)
                    .bind(item.item_guid)
                    .execute(&mut *tx)
                    .await?;
            }
            let has_items = !mail.items.is_empty();
            let deliver_delay_secs = if has_items && sender_account != receiver_account {
                default_delivery_delay_secs
            } else {
                0
            };
            let request = SendCharacterMailRequest {
                sender: receiver,
                receiver: mail.sender,
                subject: mail.subject,
                item_text_id: mail.item_text_id,
                money: mail.money,
                cod: 0,
                checked: 0x02,
                deliver_delay_secs,
                expire_delay_secs: 30 * 24 * 60 * 60,
                attached_item_guid: None,
                stationery: 41,
                message_type: 0,
                mail_template_id: mail.mail_template_id,
            };
            insert_mail_in_tx(&mut tx, returned_id, &request).await?;
            for item in &mail.items {
                sqlx::query(
                    "INSERT INTO mail_items (mail_id, item_guid, item_template, receiver) VALUES (?, ?, ?, ?)",
                )
                .bind(returned_id)
                .bind(item.item_guid)
                .bind(item.item_template)
                .bind(mail.sender)
                .execute(&mut *tx)
                .await?;
            }
        } else {
            for item in &mail.items {
                sqlx::query("DELETE FROM item_instance WHERE guid = ?")
                    .bind(item.item_guid)
                    .execute(&mut *tx)
                    .await?;
            }
        }
    }
    tx.commit().await?;
    Ok(true)
}

async fn load_mail_items(
    pool: &MySqlPool,
    mail_id: u32,
    receiver: u32,
) -> Result<Vec<CharacterMailItem>, DbError> {
    let rows = sqlx::query(
        "SELECT mi.item_guid, mi.item_template, ii.count, ii.charges, ii.enchantments, \
                ii.randomPropertyId, ii.durability \
         FROM mail_items mi \
         JOIN item_instance ii ON ii.guid = mi.item_guid \
         WHERE mi.mail_id = ? AND mi.receiver = ? ORDER BY mi.item_guid",
    )
    .bind(mail_id)
    .bind(receiver)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(CharacterMailItem {
                item_guid: row.try_get("item_guid")?,
                item_template: row.try_get("item_template")?,
                count: row.try_get("count")?,
                charges: row.try_get("charges")?,
                enchantments: row.try_get("enchantments")?,
                random_property_id: row.try_get("randomPropertyId")?,
                durability: row.try_get("durability")?,
                max_durability: row.try_get("durability")?,
            })
        })
        .collect()
}

fn row_to_character_mail(row: sqlx::mysql::MySqlRow) -> Result<CharacterMail, DbError> {
    Ok(CharacterMail {
        id: row.try_get("id")?,
        message_type: row.try_get("messageType")?,
        stationery: row.try_get("stationery")?,
        mail_template_id: row.try_get("mailTemplateId")?,
        sender: row.try_get("sender")?,
        receiver: row.try_get("receiver")?,
        subject: row.try_get::<Option<String>, _>("subject")?.unwrap_or_default(),
        item_text_id: row.try_get("itemTextId")?,
        has_items: row.try_get("has_items")?,
        expire_time: row.try_get("expire_time")?,
        deliver_time: row.try_get("deliver_time")?,
        money: row.try_get("money")?,
        cod: row.try_get("cod")?,
        checked: row.try_get("checked")?,
        items: Vec::new(),
    })
}

async fn insert_mail_in_tx(
    tx: &mut Transaction<'_, MySql>,
    mail_id: u32,
    request: &SendCharacterMailRequest,
) -> Result<(), DbError> {
    let deliver_time = current_unix_secs().saturating_add(request.deliver_delay_secs);
    let expire_time = deliver_time.saturating_add(request.expire_delay_secs);
    sqlx::query(
        "INSERT INTO mail \
         (id, messageType, stationery, mailTemplateId, sender, receiver, subject, itemTextId, \
          has_items, expire_time, deliver_time, money, cod, checked) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(mail_id)
    .bind(request.message_type)
    .bind(request.stationery)
    .bind(request.mail_template_id)
    .bind(request.sender)
    .bind(request.receiver)
    .bind(&request.subject)
    .bind(request.item_text_id)
    .bind(request.attached_item_guid.is_some() as u8)
    .bind(expire_time)
    .bind(deliver_time)
    .bind(request.money)
    .bind(request.cod)
    .bind(request.checked)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn next_mail_id_in_tx(tx: &mut Transaction<'_, MySql>) -> Result<u32, DbError> {
    let max_id: Option<u32> = sqlx::query_scalar("SELECT MAX(id) FROM mail")
        .fetch_one(&mut **tx)
        .await?;
    Ok(max_id.unwrap_or(0).saturating_add(1))
}

async fn next_item_text_id(pool: &MySqlPool) -> Result<u32, DbError> {
    let max_id: Option<u32> = sqlx::query_scalar("SELECT MAX(id) FROM item_text")
        .fetch_one(pool)
        .await?;
    Ok(max_id.unwrap_or(0).saturating_add(1))
}

fn current_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
