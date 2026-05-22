#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuctionRecord {
    pub id: u32,
    pub house_id: u32,
    pub item_guid: u32,
    pub item_template: u32,
    pub item_count: u32,
    pub item_random_property_id: i32,
    pub item_owner: u32,
    pub buyout_price: u32,
    pub expire_time: u64,
    pub bidder: u32,
    pub current_bid: u32,
    pub start_bid: u32,
    pub deposit: u32,
    pub charges: String,
    pub enchantments: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreateAuctionFromInventoryRequest {
    pub owner_guid: u32,
    pub item_guid: u32,
    pub house_id: u32,
    pub start_bid: u32,
    pub buyout_price: u32,
    pub expire_time: u64,
    pub deposit: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreateAuctionFromInventoryResult {
    pub auction_id: u32,
    pub money: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RemoveAuctionRequest {
    pub auction_id: u32,
    pub house_bucket: AuctionHouseBucket,
    pub owner_guid: u32,
    pub cut_percent: u32,
    pub cut_rate: f64,
    pub bidder_cancelled_answer: u8,
    pub owner_cancelled_answer: u8,
    pub mail_checked: u8,
    pub mail_message_type: u8,
    pub mail_stationery: u8,
    pub mail_expire_delay_secs: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoveAuctionResult {
    pub auction_id: u32,
    pub item_template: u32,
    pub item_random_property_id: i32,
    pub bidder_guid: u32,
    pub owner_money: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaceAuctionBidRequest {
    pub auction_id: u32,
    pub house_bucket: AuctionHouseBucket,
    pub bidder_guid: u32,
    pub bidder_account_id: u32,
    pub offered_bid: u32,
    pub cut_percent: u32,
    pub cut_rate: f64,
    pub outbid_answer: u8,
    pub won_answer: u8,
    pub successful_answer: u8,
    pub mail_checked: u8,
    pub mail_message_type: u8,
    pub mail_stationery: u8,
    pub mail_expire_delay_secs: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaceAuctionBidResult {
    pub auction_id: u32,
    pub house_id: u32,
    pub item_template: u32,
    pub item_random_property_id: i32,
    pub owner_guid: u32,
    pub previous_bidder_guid: u32,
    pub previous_bid: u32,
    pub previous_min_outbid: u32,
    pub current_bidder_guid: u32,
    pub current_bid: u32,
    pub current_min_outbid: u32,
    pub bidder_money: u32,
    pub completed_buyout: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::FromRow)]
pub struct ExpiredAuctionCandidate {
    pub auction_id: u32,
    pub house_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExpireAuctionRequest {
    pub auction_id: u32,
    pub house_id: u32,
    pub now_unix_secs: u64,
    pub cut_percent: u32,
    pub cut_rate: f64,
    pub expired_answer: u8,
    pub won_answer: u8,
    pub successful_answer: u8,
    pub mail_checked: u8,
    pub mail_message_type: u8,
    pub mail_stationery: u8,
    pub mail_expire_delay_secs: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpireAuctionResult {
    pub auction_id: u32,
    pub house_id: u32,
    pub item_template: u32,
    pub item_random_property_id: i32,
    pub owner_guid: u32,
    pub bidder_guid: u32,
    pub current_bid: u32,
    pub current_min_outbid: u32,
    pub kind: ExpireAuctionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpireAuctionKind {
    Expired,
    Sold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuctionHouseBucket {
    Alliance,
    Horde,
    Neutral,
    Global,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateAuctionError {
    MissingItem,
    NotEnoughMoney,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoveAuctionError {
    NotFoundOrNotOwner,
    MissingItem,
    NotEnoughMoney,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceAuctionBidError {
    NotFound,
    BidOwn,
    MissingItem,
    HigherBid {
        current_bidder_guid: u32,
        current_bid: u32,
        current_min_outbid: u32,
    },
    BidIncrement,
    NotEnoughMoney,
    BelowStartBid,
}

pub async fn list_expired_auction_candidates(
    pool: &MySqlPool,
    now_unix_secs: u64,
) -> Result<Vec<ExpiredAuctionCandidate>, DbError> {
    sqlx::query_as(
        "SELECT id AS auction_id, houseid AS house_id \
         FROM auction WHERE time <= ? ORDER BY time, id",
    )
    .bind(now_unix_secs)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

pub async fn get_bucket_auctions(
    pool: &MySqlPool,
    house_bucket: AuctionHouseBucket,
    now_unix_secs: u64,
) -> Result<Vec<AuctionRecord>, DbError> {
    let rows = sqlx::query(
        "SELECT a.id, a.houseid, a.itemguid, a.item_template, a.item_count, \
                a.item_randompropertyid, a.itemowner, a.buyoutprice, a.time, a.buyguid, \
                a.lastbid, a.startbid, a.deposit, ii.charges, ii.enchantments \
         FROM auction a \
         JOIN item_instance ii ON ii.guid = a.itemguid \
         WHERE a.time > ? \
         ORDER BY a.id",
    )
    .bind(now_unix_secs)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(auction_record_from_row)
        .filter(|result| {
            result
                .as_ref()
                .ok()
                .is_some_and(|auction| house_matches_bucket(auction.house_id, house_bucket))
        })
        .collect()
}

fn auction_record_from_row(row: sqlx::mysql::MySqlRow) -> Result<AuctionRecord, DbError> {
    Ok(AuctionRecord {
        id: row.try_get("id")?,
        house_id: row.try_get("houseid")?,
        item_guid: row.try_get("itemguid")?,
        item_template: row.try_get("item_template")?,
        item_count: row.try_get("item_count")?,
        item_random_property_id: row.try_get("item_randompropertyid")?,
        item_owner: row.try_get("itemowner")?,
        buyout_price: non_negative_u32(&row, "buyoutprice")?,
        expire_time: row.try_get("time")?,
        bidder: row.try_get("buyguid")?,
        current_bid: non_negative_u32(&row, "lastbid")?,
        start_bid: non_negative_u32(&row, "startbid")?,
        deposit: non_negative_u32(&row, "deposit")?,
        charges: row.try_get("charges")?,
        enchantments: row.try_get("enchantments")?,
    })
}

fn non_negative_u32(row: &sqlx::mysql::MySqlRow, column: &str) -> Result<u32, DbError> {
    Ok(row.try_get::<i64, _>(column)?.max(0) as u32)
}

pub fn house_matches_bucket(house_id: u32, bucket: AuctionHouseBucket) -> bool {
    match bucket {
        AuctionHouseBucket::Alliance => matches!(house_id, 1..=3),
        AuctionHouseBucket::Horde => matches!(house_id, 4..=6),
        AuctionHouseBucket::Neutral => house_id == 7,
        AuctionHouseBucket::Global => matches!(house_id, 1..=7),
    }
}

pub async fn auction_has_item_guid(pool: &MySqlPool, item_guid: u32) -> Result<bool, DbError> {
    let exists: Option<u8> = sqlx::query_scalar("SELECT 1 FROM auction WHERE itemguid = ? LIMIT 1")
        .bind(item_guid)
        .fetch_optional(pool)
        .await?;
    Ok(exists.is_some())
}

pub async fn create_auction_from_inventory(
    pool: &MySqlPool,
    request: CreateAuctionFromInventoryRequest,
) -> Result<Result<CreateAuctionFromInventoryResult, CreateAuctionError>, DbError> {
    let mut tx = pool.begin().await?;
    let item: Option<(u32, u32, i32)> = sqlx::query_as(
        "SELECT ci.item_template, ii.count, ii.randomPropertyId \
         FROM character_inventory ci \
         JOIN item_instance ii ON ci.item = ii.guid \
         WHERE ci.guid = ? AND ci.item = ? AND ii.owner_guid = ?",
    )
    .bind(request.owner_guid)
    .bind(request.item_guid)
    .bind(request.owner_guid)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((item_template, item_count, item_random_property_id)) = item else {
        return Ok(Err(CreateAuctionError::MissingItem));
    };

    let spent = sqlx::query("UPDATE characters SET money = money - ? WHERE guid = ? AND money >= ?")
        .bind(request.deposit)
        .bind(request.owner_guid)
        .bind(request.deposit)
        .execute(&mut *tx)
        .await?;
    if spent.rows_affected() == 0 {
        return Ok(Err(CreateAuctionError::NotEnoughMoney));
    }

    sqlx::query("DELETE FROM character_inventory WHERE guid = ? AND item = ?")
        .bind(request.owner_guid)
        .bind(request.item_guid)
        .execute(&mut *tx)
        .await?;

    let auction_id: u32 =
        sqlx::query_scalar("SELECT COALESCE(MAX(id), 0) + 1 FROM auction FOR UPDATE")
            .fetch_one(&mut *tx)
            .await?;
    sqlx::query(
        "INSERT INTO auction \
         (id, houseid, itemguid, item_template, item_count, item_randompropertyid, itemowner, \
          buyoutprice, time, buyguid, lastbid, startbid, deposit) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 0, 0, ?, ?)",
    )
    .bind(auction_id)
    .bind(request.house_id)
    .bind(request.item_guid)
    .bind(item_template)
    .bind(item_count)
    .bind(item_random_property_id)
    .bind(request.owner_guid)
    .bind(request.buyout_price)
    .bind(request.expire_time)
    .bind(request.start_bid)
    .bind(request.deposit)
    .execute(&mut *tx)
    .await?;

    let money = sqlx::query_scalar("SELECT money FROM characters WHERE guid = ?")
        .bind(request.owner_guid)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;

    Ok(Ok(CreateAuctionFromInventoryResult { auction_id, money }))
}

pub async fn remove_auction(
    pool: &MySqlPool,
    request: RemoveAuctionRequest,
) -> Result<Result<RemoveAuctionResult, RemoveAuctionError>, DbError> {
    let mut tx = pool.begin().await?;
    let auction: Option<(u32, u32, u32, i32, u32, u32, i64)> = sqlx::query_as(
        "SELECT houseid, itemguid, item_template, item_randompropertyid, itemowner, buyguid, \
                lastbid FROM auction WHERE id = ? FOR UPDATE",
    )
    .bind(request.auction_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((
        house_id,
        item_guid,
        item_template,
        item_random_property_id,
        item_owner,
        bidder_guid,
        bid,
    )) =
        auction
    else {
        return Ok(Err(RemoveAuctionError::NotFoundOrNotOwner));
    };
    if !house_matches_bucket(house_id, request.house_bucket) || item_owner != request.owner_guid {
        return Ok(Err(RemoveAuctionError::NotFoundOrNotOwner));
    }

    let item_exists: Option<u8> =
        sqlx::query_scalar("SELECT 1 FROM item_instance WHERE guid = ? LIMIT 1")
            .bind(item_guid)
            .fetch_optional(&mut *tx)
            .await?;
    if item_exists.is_none() {
        return Ok(Err(RemoveAuctionError::MissingItem));
    }

    let cut_charge = auction_cut(
        request.cut_percent,
        bid.max(0) as u32,
        request.cut_rate,
    );
    if cut_charge != 0 {
        let spent =
            sqlx::query("UPDATE characters SET money = money - ? WHERE guid = ? AND money >= ?")
                .bind(cut_charge)
                .bind(request.owner_guid)
                .bind(cut_charge)
                .execute(&mut *tx)
                .await?;
        if spent.rows_affected() == 0 {
            return Ok(Err(RemoveAuctionError::NotEnoughMoney));
        }
    }

    if bidder_guid != 0 && character_exists_in_tx(&mut tx, bidder_guid).await? {
        insert_auction_mail_in_tx(
            &mut tx,
            AuctionMailInsertRequest {
                house_id,
                receiver_guid: bidder_guid,
                item_template,
                item_random_property_id,
                answer: request.bidder_cancelled_answer,
                body: None,
                money: bid.max(0) as u32,
                attached_item: None,
                checked: request.mail_checked,
                message_type: request.mail_message_type,
                stationery: request.mail_stationery,
                expire_delay_secs: request.mail_expire_delay_secs,
            },
        )
        .await?;
    }

    insert_auction_mail_in_tx(
        &mut tx,
        AuctionMailInsertRequest {
            house_id,
            receiver_guid: request.owner_guid,
            item_template,
            item_random_property_id,
            answer: request.owner_cancelled_answer,
            body: None,
            money: 0,
            attached_item: Some((item_guid, item_template)),
            checked: request.mail_checked,
            message_type: request.mail_message_type,
            stationery: request.mail_stationery,
            expire_delay_secs: request.mail_expire_delay_secs,
        },
    )
    .await?;

    sqlx::query("UPDATE item_instance SET owner_guid = ? WHERE guid = ?")
        .bind(request.owner_guid)
        .bind(item_guid)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM auction WHERE id = ?")
        .bind(request.auction_id)
        .execute(&mut *tx)
        .await?;

    let owner_money = sqlx::query_scalar("SELECT money FROM characters WHERE guid = ?")
        .bind(request.owner_guid)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;

    Ok(Ok(RemoveAuctionResult {
        auction_id: request.auction_id,
        item_template,
        item_random_property_id,
        bidder_guid,
        owner_money,
    }))
}

pub async fn place_auction_bid(
    pool: &MySqlPool,
    request: PlaceAuctionBidRequest,
) -> Result<Result<PlaceAuctionBidResult, PlaceAuctionBidError>, DbError> {
    let mut tx = pool.begin().await?;
    let auction: Option<AuctionBidRow> = sqlx::query_as(
        "SELECT a.houseid, a.itemguid, a.item_template, a.item_randompropertyid, a.itemowner, \
                a.buyguid, owner.account, a.lastbid, a.startbid, a.buyoutprice, a.deposit \
         FROM auction a \
         JOIN characters owner ON owner.guid = a.itemowner \
         WHERE a.id = ? FOR UPDATE",
    )
    .bind(request.auction_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(auction) = auction else {
        return Ok(Err(PlaceAuctionBidError::NotFound));
    };
    if !house_matches_bucket(auction.houseid, request.house_bucket) {
        return Ok(Err(PlaceAuctionBidError::NotFound));
    }

    if auction.itemowner == request.bidder_guid || auction.account == request.bidder_account_id {
        return Ok(Err(PlaceAuctionBidError::BidOwn));
    }

    let item_exists: Option<u8> =
        sqlx::query_scalar("SELECT 1 FROM item_instance WHERE guid = ? LIMIT 1")
            .bind(auction.itemguid)
            .fetch_optional(&mut *tx)
            .await?;
    if item_exists.is_none() {
        return Ok(Err(PlaceAuctionBidError::MissingItem));
    }

    let previous_bid = auction.lastbid.max(0) as u32;
    let start_bid = auction.startbid.max(0) as u32;
    let buyout_price = auction.buyoutprice.max(0) as u32;
    let previous_min_outbid = auction_min_outbid(previous_bid);
    if request.offered_bid <= previous_bid {
        return Ok(Err(PlaceAuctionBidError::HigherBid {
            current_bidder_guid: auction.buyguid,
            current_bid: previous_bid,
            current_min_outbid: previous_min_outbid,
        }));
    }

    let current_bid = if buyout_price != 0 {
        request.offered_bid.min(buyout_price)
    } else {
        request.offered_bid
    };
    if current_bid < start_bid {
        return Ok(Err(PlaceAuctionBidError::BelowStartBid));
    }
    if (current_bid < buyout_price || buyout_price == 0)
        && current_bid < previous_bid.saturating_add(previous_min_outbid)
    {
        return Ok(Err(PlaceAuctionBidError::BidIncrement));
    }

    let bidder_charge = if auction.buyguid == request.bidder_guid {
        current_bid.saturating_sub(previous_bid)
    } else {
        current_bid
    };
    if bidder_charge != 0 {
        let spent =
            sqlx::query("UPDATE characters SET money = money - ? WHERE guid = ? AND money >= ?")
                .bind(bidder_charge)
                .bind(request.bidder_guid)
                .bind(bidder_charge)
                .execute(&mut *tx)
                .await?;
        if spent.rows_affected() == 0 {
            return Ok(Err(PlaceAuctionBidError::NotEnoughMoney));
        }
    }

    if auction.buyguid != 0
        && auction.buyguid != request.bidder_guid
        && character_exists_in_tx(&mut tx, auction.buyguid).await?
    {
        insert_auction_mail_in_tx(
            &mut tx,
            AuctionMailInsertRequest {
                house_id: auction.houseid,
                receiver_guid: auction.buyguid,
                item_template: auction.item_template,
                item_random_property_id: auction.item_randompropertyid,
                answer: request.outbid_answer,
                body: None,
                money: previous_bid,
                attached_item: None,
                checked: request.mail_checked,
                message_type: request.mail_message_type,
                stationery: request.mail_stationery,
                expire_delay_secs: request.mail_expire_delay_secs,
            },
        )
        .await?;
    }

    let completed_buyout = buyout_price != 0 && current_bid >= buyout_price;
    if completed_buyout {
        let actual_cut = auction_cut(request.cut_percent, current_bid, request.cut_rate);
        let owner_body = auction_successful_body(
            request.bidder_guid,
            current_bid,
            buyout_price,
            auction.deposit,
            actual_cut,
        );
        insert_auction_mail_in_tx(
            &mut tx,
            AuctionMailInsertRequest {
                house_id: auction.houseid,
                receiver_guid: auction.itemowner,
                item_template: auction.item_template,
                item_random_property_id: auction.item_randompropertyid,
                answer: request.successful_answer,
                body: Some(owner_body),
                money: current_bid
                    .saturating_add(auction.deposit)
                    .saturating_sub(actual_cut),
                attached_item: None,
                checked: request.mail_checked,
                message_type: request.mail_message_type,
                stationery: request.mail_stationery,
                expire_delay_secs: request.mail_expire_delay_secs,
            },
        )
        .await?;

        let bidder_body = auction_won_body(auction.itemowner, current_bid, buyout_price);
        insert_auction_mail_in_tx(
            &mut tx,
            AuctionMailInsertRequest {
                house_id: auction.houseid,
                receiver_guid: request.bidder_guid,
                item_template: auction.item_template,
                item_random_property_id: auction.item_randompropertyid,
                answer: request.won_answer,
                body: Some(bidder_body),
                money: 0,
                attached_item: Some((auction.itemguid, auction.item_template)),
                checked: request.mail_checked,
                message_type: request.mail_message_type,
                stationery: request.mail_stationery,
                expire_delay_secs: request.mail_expire_delay_secs,
            },
        )
        .await?;

        sqlx::query("UPDATE item_instance SET owner_guid = ? WHERE guid = ?")
            .bind(request.bidder_guid)
            .bind(auction.itemguid)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM auction WHERE id = ?")
            .bind(request.auction_id)
            .execute(&mut *tx)
            .await?;
    } else {
        sqlx::query("UPDATE auction SET buyguid = ?, lastbid = ? WHERE id = ?")
            .bind(request.bidder_guid)
            .bind(current_bid)
            .bind(request.auction_id)
            .execute(&mut *tx)
            .await?;
    }

    let bidder_money = sqlx::query_scalar("SELECT money FROM characters WHERE guid = ?")
        .bind(request.bidder_guid)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;

    Ok(Ok(PlaceAuctionBidResult {
        auction_id: request.auction_id,
        house_id: auction.houseid,
        item_template: auction.item_template,
        item_random_property_id: auction.item_randompropertyid,
        owner_guid: auction.itemowner,
        previous_bidder_guid: auction.buyguid,
        previous_bid,
        previous_min_outbid,
        current_bidder_guid: request.bidder_guid,
        current_bid,
        current_min_outbid: auction_min_outbid(current_bid),
        bidder_money,
        completed_buyout,
    }))
}

pub async fn expire_auction(
    pool: &MySqlPool,
    request: ExpireAuctionRequest,
) -> Result<Option<ExpireAuctionResult>, DbError> {
    let mut tx = pool.begin().await?;
    let auction: Option<ExpiredAuctionRow> = sqlx::query_as(
        "SELECT id, houseid, itemguid, item_template, item_randompropertyid, itemowner, \
                buyguid, lastbid, buyoutprice, deposit \
         FROM auction WHERE id = ? AND houseid = ? AND time <= ? FOR UPDATE",
    )
    .bind(request.auction_id)
    .bind(request.house_id)
    .bind(request.now_unix_secs)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(auction) = auction else {
        return Ok(None);
    };

    let item_exists: Option<u8> =
        sqlx::query_scalar("SELECT 1 FROM item_instance WHERE guid = ? LIMIT 1")
            .bind(auction.itemguid)
            .fetch_optional(&mut *tx)
            .await?;
    if item_exists.is_none() {
        sqlx::query("DELETE FROM auction WHERE id = ?")
            .bind(auction.id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        return Ok(None);
    }

    let owner_exists = character_exists_in_tx(&mut tx, auction.itemowner).await?;
    let bidder_exists =
        auction.buyguid != 0 && character_exists_in_tx(&mut tx, auction.buyguid).await?;
    let current_bid = auction.lastbid.max(0) as u32;
    let current_min_outbid = auction_min_outbid(current_bid);
    let sold = current_bid != 0 && auction.buyguid != 0;

    if sold {
        let actual_cut = auction_cut(request.cut_percent, current_bid, request.cut_rate);
        if owner_exists {
            let owner_body = auction_successful_body(
                auction.buyguid,
                current_bid,
                auction.buyoutprice.max(0) as u32,
                auction.deposit,
                actual_cut,
            );
            insert_auction_mail_in_tx(
                &mut tx,
                AuctionMailInsertRequest {
                    house_id: auction.houseid,
                    receiver_guid: auction.itemowner,
                    item_template: auction.item_template,
                    item_random_property_id: auction.item_randompropertyid,
                    answer: request.successful_answer,
                    body: Some(owner_body),
                    money: current_bid
                        .saturating_add(auction.deposit)
                        .saturating_sub(actual_cut),
                    attached_item: None,
                    checked: request.mail_checked,
                    message_type: request.mail_message_type,
                    stationery: request.mail_stationery,
                    expire_delay_secs: request.mail_expire_delay_secs,
                },
            )
            .await?;
        }

        if bidder_exists {
            let bidder_body = auction_won_body(
                auction.itemowner,
                current_bid,
                auction.buyoutprice.max(0) as u32,
            );
            insert_auction_mail_in_tx(
                &mut tx,
                AuctionMailInsertRequest {
                    house_id: auction.houseid,
                    receiver_guid: auction.buyguid,
                    item_template: auction.item_template,
                    item_random_property_id: auction.item_randompropertyid,
                    answer: request.won_answer,
                    body: Some(bidder_body),
                    money: 0,
                    attached_item: Some((auction.itemguid, auction.item_template)),
                    checked: request.mail_checked,
                    message_type: request.mail_message_type,
                    stationery: request.mail_stationery,
                    expire_delay_secs: request.mail_expire_delay_secs,
                },
            )
            .await?;
            sqlx::query("UPDATE item_instance SET owner_guid = ? WHERE guid = ?")
                .bind(auction.buyguid)
                .bind(auction.itemguid)
                .execute(&mut *tx)
                .await?;
        } else {
            sqlx::query("DELETE FROM item_instance WHERE guid = ?")
                .bind(auction.itemguid)
                .execute(&mut *tx)
                .await?;
        }
    } else if owner_exists {
        insert_auction_mail_in_tx(
            &mut tx,
            AuctionMailInsertRequest {
                house_id: auction.houseid,
                receiver_guid: auction.itemowner,
                item_template: auction.item_template,
                item_random_property_id: auction.item_randompropertyid,
                answer: request.expired_answer,
                body: None,
                money: 0,
                attached_item: Some((auction.itemguid, auction.item_template)),
                checked: request.mail_checked,
                message_type: request.mail_message_type,
                stationery: request.mail_stationery,
                expire_delay_secs: request.mail_expire_delay_secs,
            },
        )
        .await?;
        sqlx::query("UPDATE item_instance SET owner_guid = ? WHERE guid = ?")
            .bind(auction.itemowner)
            .bind(auction.itemguid)
            .execute(&mut *tx)
            .await?;
    } else {
        sqlx::query("DELETE FROM item_instance WHERE guid = ?")
            .bind(auction.itemguid)
            .execute(&mut *tx)
            .await?;
    }

    sqlx::query("DELETE FROM auction WHERE id = ?")
        .bind(auction.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    Ok(Some(ExpireAuctionResult {
        auction_id: auction.id,
        house_id: auction.houseid,
        item_template: auction.item_template,
        item_random_property_id: auction.item_randompropertyid,
        owner_guid: auction.itemowner,
        bidder_guid: auction.buyguid,
        current_bid,
        current_min_outbid,
        kind: if sold {
            ExpireAuctionKind::Sold
        } else {
            ExpireAuctionKind::Expired
        },
    }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AuctionMailInsertRequest {
    house_id: u32,
    receiver_guid: u32,
    item_template: u32,
    item_random_property_id: i32,
    answer: u8,
    body: Option<String>,
    money: u32,
    attached_item: Option<(u32, u32)>,
    checked: u8,
    message_type: u8,
    stationery: u8,
    expire_delay_secs: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::FromRow)]
struct AuctionBidRow {
    houseid: u32,
    itemguid: u32,
    item_template: u32,
    item_randompropertyid: i32,
    itemowner: u32,
    buyguid: u32,
    account: u32,
    lastbid: i64,
    startbid: i64,
    buyoutprice: i64,
    deposit: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::FromRow)]
struct ExpiredAuctionRow {
    id: u32,
    houseid: u32,
    itemguid: u32,
    item_template: u32,
    item_randompropertyid: i32,
    itemowner: u32,
    buyguid: u32,
    lastbid: i64,
    buyoutprice: i64,
    deposit: u32,
}

fn auction_cut(cut_percent: u32, bid: u32, cut_rate: f64) -> u32 {
    ((cut_percent as f64 * bid as f64 * cut_rate) / 100.0) as u32
}

async fn insert_auction_mail_in_tx(
    tx: &mut Transaction<'_, MySql>,
    request: AuctionMailInsertRequest,
) -> Result<(), DbError> {
    let item_text_id = if let Some(body) = request.body.as_deref() {
        insert_item_text_in_tx(tx, body).await?
    } else {
        0
    };
    let mail_id = next_mail_id_in_tx(tx).await?;
    insert_mail_in_tx(
        tx,
        mail_id,
        &SendCharacterMailRequest {
            sender: request.house_id,
            receiver: request.receiver_guid,
            subject: auction_mail_subject(
                request.item_template,
                request.item_random_property_id,
                request.answer,
            ),
            item_text_id,
            money: request.money,
            cod: 0,
            checked: request.checked,
            deliver_delay_secs: 0,
            expire_delay_secs: request.expire_delay_secs,
            attached_item_guid: request.attached_item.map(|(item_guid, _)| item_guid),
            stationery: request.stationery,
            message_type: request.message_type,
            mail_template_id: 0,
        },
    )
    .await?;
    if let Some((item_guid, attached_template)) = request.attached_item {
        sqlx::query(
            "INSERT INTO mail_items (mail_id, item_guid, item_template, receiver) VALUES (?, ?, ?, ?)",
        )
        .bind(mail_id)
        .bind(item_guid)
        .bind(attached_template)
        .bind(request.receiver_guid)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn character_exists_in_tx(
    tx: &mut Transaction<'_, MySql>,
    guid: u32,
) -> Result<bool, DbError> {
    let exists: Option<u8> = sqlx::query_scalar("SELECT 1 FROM characters WHERE guid = ? LIMIT 1")
        .bind(guid)
        .fetch_optional(&mut **tx)
        .await?;
    Ok(exists.is_some())
}

fn auction_mail_subject(item_template: u32, item_random_property_id: i32, answer: u8) -> String {
    format!("{item_template}:{item_random_property_id}:{answer}")
}

fn auction_min_outbid(current_bid: u32) -> u32 {
    if current_bid == 0 {
        0
    } else {
        ((current_bid / 100) * 5).max(1)
    }
}

fn auction_won_body(owner_guid: u32, bid: u32, buyout_price: u32) -> String {
    format!("{owner_guid:>16x}:{bid}:{buyout_price}")
}

fn auction_successful_body(
    bidder_guid: u32,
    bid: u32,
    buyout_price: u32,
    deposit: u32,
    auction_cut: u32,
) -> String {
    format!("{bidder_guid:>16x}:{bid}:{buyout_price}:{deposit}:{auction_cut}")
}

async fn insert_item_text_in_tx(
    tx: &mut Transaction<'_, MySql>,
    text: &str,
) -> Result<u32, DbError> {
    let max_id: Option<u32> = sqlx::query_scalar("SELECT MAX(id) FROM item_text")
        .fetch_one(&mut **tx)
        .await?;
    let item_text_id = max_id.unwrap_or(0).saturating_add(1);
    sqlx::query("INSERT INTO item_text (id, text) VALUES (?, ?)")
        .bind(item_text_id)
        .bind(text)
        .execute(&mut **tx)
        .await?;
    Ok(item_text_id)
}
