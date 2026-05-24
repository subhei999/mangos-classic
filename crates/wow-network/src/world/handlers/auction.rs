use super::*;
use wow_proto::world::WorldOpcode;
use wow_proto::{
    AuctionInfoResponse, ServerWorldPacket, SmsgAuctionBidderListResultResponse,
    SmsgAuctionBidderNotificationResponse, SmsgAuctionCommandResultResponse,
    SmsgAuctionHelloResponse, SmsgAuctionListResultResponse, SmsgAuctionOwnerListResultResponse,
    SmsgAuctionOwnerNotificationResponse, SmsgAuctionRemovedNotificationResponse,
};

pub(in crate::world) const AUCTIONEER_INTERACTION_DISTANCE_YARDS: f32 = 5.0;
const AUCTION_CLIENT_PAGE_SIZE: usize = 50;
const MIN_AUCTION_TIME_SECS: u32 = 2 * 60 * 60;
const AUCTION_HOUSE_HUMAN: u32 = 1;
const AUCTION_HOUSE_DWARF: u32 = 2;
const AUCTION_HOUSE_NIGHT_ELF: u32 = 3;
const AUCTION_HOUSE_UNDEAD: u32 = 4;
const AUCTION_HOUSE_TROLL: u32 = 5;
const AUCTION_HOUSE_ORC: u32 = 6;
const AUCTION_HOUSE_NEUTRAL: u32 = 7;
const INVTYPE_CHEST: u32 = 5;
const INVTYPE_ROBE: u32 = 20;
const ITEM_FLAG_CONJURED: u32 = 0x0000_0002;
const MAX_MONEY_AMOUNT: u32 = 0x7FFF_FFFE;
const AUCTION_ACTION_STARTED: u32 = 0;
const AUCTION_ACTION_REMOVED: u32 = 1;
const AUCTION_ACTION_BID_PLACED: u32 = 2;
const AUCTION_ERR_OK: u32 = 0;
const AUCTION_ERR_INVENTORY: u32 = 1;
const AUCTION_ERR_DATABASE: u32 = 2;
const AUCTION_ERR_NOT_ENOUGH_MONEY: u32 = 3;
const AUCTION_ERR_HIGHER_BID: u32 = 5;
const AUCTION_ERR_BID_INCREMENT: u32 = 7;
const AUCTION_ERR_BID_OWN: u32 = 10;
pub(in crate::world) const AUCTION_MAIL_MESSAGE_TYPE: u8 = 2;
pub(in crate::world) const AUCTION_MAIL_STATIONERY: u8 = 62;
pub(in crate::world) const MAIL_CHECK_MASK_COPIED: u8 = 0x04;
pub(in crate::world) const MAIL_EXPIRY_SECS: u64 = 30 * 24 * 60 * 60;
const AUCTION_OUTBIDDED_ANSWER: u8 = 0;
pub(in crate::world) const AUCTION_WON_ANSWER: u8 = 1;
pub(in crate::world) const AUCTION_SUCCESSFUL_ANSWER: u8 = 2;
pub(in crate::world) const AUCTION_EXPIRED_ANSWER: u8 = 3;
const AUCTION_CANCELLED_TO_BIDDER_ANSWER: u8 = 4;
const AUCTION_CANCELED_ANSWER: u8 = 5;
const EQUIP_ERR_ITEM_NOT_FOUND_U32: u32 = EQUIP_ERR_ITEM_NOT_FOUND as u32;

pub(in crate::world) struct AuctionBrowseDeps<'a> {
    pub(in crate::world) character_db_pool: &'a MySqlPool,
    pub(in crate::world) world_db_pool: &'a MySqlPool,
    pub(in crate::world) world_data_files: &'a WorldDataFiles,
    pub(in crate::world) auction_config: AuctionRuntimeConfig,
    pub(in crate::world) maps: &'a Arc<MapRuntimeManager>,
}

pub(in crate::world) struct AuctionMutationDeps<'a> {
    pub(in crate::world) quest: QuestMutationDeps<'a>,
    pub(in crate::world) world_data_files: &'a WorldDataFiles,
    pub(in crate::world) auction_config: AuctionRuntimeConfig,
    pub(in crate::world) maps: &'a Arc<MapRuntimeManager>,
}

struct AuctionBidCommandResult {
    auction_id: u32,
    error_code: u32,
    bid_min_outbid: Option<u32>,
    higher_bidder: Option<ObjectGuid>,
    higher_bid: Option<u32>,
    higher_min_outbid: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AuctionHouseAccess {
    entry_house_id: u32,
    storage_bucket: wow_db::AuctionHouseBucket,
}

pub(in crate::world) fn build_auction_bidder_notification_packet(
    house_id: u32,
    auction_id: u32,
    bidder_guid: u32,
    bid_or_zero_if_won: u32,
    min_outbid: u32,
    item_template: u32,
    item_random_property_id: i32,
) -> OutboundWorldPacket {
    OutboundWorldPacket {
        opcode: WorldOpcode::SmsgAuctionBidderNotification as u16,
        body: SmsgAuctionBidderNotificationResponse {
            house_id,
            auction_id,
            bidder: ObjectGuid::new(HighGuid::Player, 0, bidder_guid),
            bid_or_zero_if_won,
            min_outbid,
            item_template,
            item_random_property_id,
        }
        .body(),
    }
}

pub(in crate::world) fn build_auction_owner_notification_packet(
    auction_id: u32,
    bid: u32,
    min_outbid: u32,
    bidder_guid: u32,
    sold: bool,
    item_template: u32,
    item_random_property_id: i32,
) -> OutboundWorldPacket {
    OutboundWorldPacket {
        opcode: WorldOpcode::SmsgAuctionOwnerNotification as u16,
        body: SmsgAuctionOwnerNotificationResponse {
            auction_id,
            bid,
            min_outbid,
            bidder: if sold {
                ObjectGuid::EMPTY
            } else {
                ObjectGuid::new(HighGuid::Player, 0, bidder_guid)
            },
            item_template,
            item_random_property_id,
        }
        .body(),
    }
}

pub(in crate::world) fn build_auction_removed_notification_packet(
    auction_id: u32,
    item_template: u32,
    item_random_property_id: i32,
) -> OutboundWorldPacket {
    OutboundWorldPacket {
        opcode: WorldOpcode::SmsgAuctionRemovedNotification as u16,
        body: SmsgAuctionRemovedNotificationResponse {
            auction_id,
            item_template,
            item_random_property_id,
        }
        .body(),
    }
}

pub(in crate::world) async fn send_auction_hello(
    stream: &mut WorldPacketSink,
    maps: &Arc<MapRuntimeManager>,
    world_data_files: &WorldDataFiles,
    auction_config: AuctionRuntimeConfig,
    auctioneer: ObjectGuid,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(access) =
        checked_auction_house_access(maps, world_data_files, auction_config, session, auctioneer)
            .await
    else {
        warn!(
            auctioneer = format_args!("0x{:016X}", auctioneer.raw()),
            "Ignoring auction hello for inaccessible non-auctioneer"
        );
        return Ok(());
    };

    send_packet(
        stream,
        WorldOpcode::MsgAuctionHello as u16,
        &SmsgAuctionHelloResponse {
            auctioneer,
            house_id: access.entry_house_id,
        }
        .body(),
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn handle_auction_sell_item(
    stream: &mut WorldPacketSink,
    deps: AuctionMutationDeps<'_>,
    request: wow_proto::AuctionSellItemRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        warn!("Ignoring auction sell before character login");
        return Ok(());
    };
    let auctioneer = ObjectGuid::from_raw(request.auctioneer_raw_guid);
    let Some(access) = checked_auction_house_access(
        deps.maps,
        deps.world_data_files,
        deps.auction_config,
        session,
        auctioneer,
    )
    .await
    else {
        warn!(
            auctioneer = format_args!("0x{:016X}", auctioneer.raw()),
            "Ignoring auction sell for inaccessible non-auctioneer"
        );
        return Ok(());
    };

    let Some(duration_secs) =
        auction_duration_secs_from_client(request.duration_minutes, deps.auction_config)
    else {
        return Ok(());
    };
    if request.bid == 0 || !auctioneer.is_creature() {
        return Ok(());
    }
    if request.bid > MAX_MONEY_AMOUNT || request.buyout > MAX_MONEY_AMOUNT {
        return send_auction_command_result(
            stream,
            0,
            AUCTION_ACTION_STARTED,
            AUCTION_ERR_DATABASE,
            None,
            header_crypto,
        )
        .await;
    }

    let item_guid = ObjectGuid::from_raw(request.item_raw_guid);
    if !item_guid.is_item() {
        return send_auction_command_result(
            stream,
            0,
            AUCTION_ACTION_STARTED,
            AUCTION_ERR_INVENTORY,
            Some(EQUIP_ERR_ITEM_NOT_FOUND_U32),
            header_crypto,
        )
        .await;
    }
    let Some(source_item) = session
        .inventory
        .items
        .iter()
        .find(|item| item.item == item_guid.counter())
        .cloned()
    else {
        return send_auction_command_result(
            stream,
            0,
            AUCTION_ACTION_STARTED,
            AUCTION_ERR_INVENTORY,
            Some(EQUIP_ERR_ITEM_NOT_FOUND_U32),
            header_crypto,
        )
        .await;
    };
    if !auction_source_position_supported(&source_item) {
        return send_auction_command_result(
            stream,
            0,
            AUCTION_ACTION_STARTED,
            AUCTION_ERR_INVENTORY,
            Some(EQUIP_ERR_ITEM_NOT_FOUND_U32),
            header_crypto,
        )
        .await;
    }
    if wow_db::auction_has_item_guid(deps.quest.character_db_pool, source_item.item).await? {
        return send_auction_command_result(
            stream,
            0,
            AUCTION_ACTION_STARTED,
            AUCTION_ERR_INVENTORY,
            Some(EQUIP_ERR_ITEM_NOT_FOUND_U32),
            header_crypto,
        )
        .await;
    }

    let Some(template) =
        wow_db::get_item_template_query(deps.quest.world_db_pool, source_item.item_template)
            .await?
    else {
        return send_auction_command_result(
            stream,
            0,
            AUCTION_ACTION_STARTED,
            AUCTION_ERR_DATABASE,
            None,
            header_crypto,
        )
        .await;
    };
    if !auction_item_can_be_listed(
        deps.quest.character_db_pool,
        &session.inventory.items,
        character.guid,
        &source_item,
        &template,
    )
    .await?
    {
        return send_auction_command_result(
            stream,
            0,
            AUCTION_ACTION_STARTED,
            AUCTION_ERR_INVENTORY,
            Some(EQUIP_ERR_ITEM_NOT_FOUND_U32),
            header_crypto,
        )
        .await;
    }

    let Some(auction_house_entry) = deps
        .world_data_files
        .auction_houses
        .get(&access.entry_house_id)
        .copied()
    else {
        warn!(
            house_id = access.entry_house_id,
            "AuctionHouse.dbc entry missing for auction sell"
        );
        return send_auction_command_result(
            stream,
            0,
            AUCTION_ACTION_STARTED,
            AUCTION_ERR_DATABASE,
            None,
            header_crypto,
        )
        .await;
    };
    let deposit = auction_deposit_from_template(
        &auction_house_entry,
        duration_secs,
        &template,
        source_item.count,
        deps.auction_config,
    );

    let expire_time = current_unix_time_secs().saturating_add(duration_secs as u64);
    let created = wow_db::create_auction_from_inventory(
        deps.quest.character_db_pool,
        wow_db::CreateAuctionFromInventoryRequest {
            owner_guid: character.guid,
            item_guid: source_item.item,
            house_id: access.entry_house_id,
            start_bid: request.bid,
            buyout_price: request.buyout,
            expire_time,
            deposit,
        },
    )
    .await?;
    let created = match created {
        Ok(created) => created,
        Err(wow_db::CreateAuctionError::MissingItem) => {
            return send_auction_command_result(
                stream,
                0,
                AUCTION_ACTION_STARTED,
                AUCTION_ERR_INVENTORY,
                Some(EQUIP_ERR_ITEM_NOT_FOUND_U32),
                header_crypto,
            )
            .await;
        }
        Err(wow_db::CreateAuctionError::NotEnoughMoney) => {
            return send_auction_command_result(
                stream,
                0,
                AUCTION_ACTION_STARTED,
                AUCTION_ERR_NOT_ENOUGH_MONEY,
                None,
                header_crypto,
            )
            .await;
        }
    };

    session.inventory.items =
        wow_db::get_character_inventory_items(deps.quest.character_db_pool, character.guid).await?;
    let update_blocks = build_inventory_position_update_blocks(
        character.guid,
        &session.inventory.items,
        source_item.bag as u8,
        source_item.slot,
    )?;
    if !update_blocks.is_empty() {
        send_packet(
            stream,
            WorldOpcode::SmsgUpdateObject as u16,
            &build_update_object_body(&update_blocks),
            Some(&mut *header_crypto),
        )
        .await?;
    }
    if source_item.bag != INVENTORY_SLOT_BAG_0 as u32 {
        send_packet(
            stream,
            WorldOpcode::SmsgDestroyObject as u16,
            &build_destroy_object_body(source_item.item),
            Some(&mut *header_crypto),
        )
        .await?;
    }
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &build_player_money_update_body(character.guid, created.money)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_auction_command_result(
        stream,
        created.auction_id,
        AUCTION_ACTION_STARTED,
        AUCTION_ERR_OK,
        None,
        header_crypto,
    )
    .await?;

    revalidate_completed_item_quests_after_inventory_change(
        stream,
        deps.quest,
        session,
        character.guid,
        header_crypto,
    )
    .await
}

pub(in crate::world) async fn handle_auction_remove_item(
    stream: &mut WorldPacketSink,
    deps: AuctionMutationDeps<'_>,
    request: wow_proto::AuctionRemoveItemRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        warn!("Ignoring auction remove before character login");
        return Ok(());
    };
    let auctioneer = ObjectGuid::from_raw(request.auctioneer_raw_guid);
    let Some(access) = checked_auction_house_access(
        deps.maps,
        deps.world_data_files,
        deps.auction_config,
        session,
        auctioneer,
    )
    .await
    else {
        warn!(
            auctioneer = format_args!("0x{:016X}", auctioneer.raw()),
            "Ignoring auction remove for inaccessible non-auctioneer"
        );
        return Ok(());
    };

    let Some(auction_house_entry) = deps
        .world_data_files
        .auction_houses
        .get(&access.entry_house_id)
        .copied()
    else {
        warn!(
            house_id = access.entry_house_id,
            "AuctionHouse.dbc entry missing for auction remove"
        );
        return send_auction_command_result(
            stream,
            0,
            AUCTION_ACTION_REMOVED,
            AUCTION_ERR_DATABASE,
            None,
            header_crypto,
        )
        .await;
    };
    let removed = wow_db::remove_auction(
        deps.quest.character_db_pool,
        wow_db::RemoveAuctionRequest {
            auction_id: request.auction_id,
            house_bucket: access.storage_bucket,
            owner_guid: character.guid,
            cut_percent: auction_house_entry.cut_percent,
            cut_rate: deps.auction_config.cut_rate,
            bidder_cancelled_answer: AUCTION_CANCELLED_TO_BIDDER_ANSWER,
            owner_cancelled_answer: AUCTION_CANCELED_ANSWER,
            mail_checked: MAIL_CHECK_MASK_COPIED,
            mail_message_type: AUCTION_MAIL_MESSAGE_TYPE,
            mail_stationery: AUCTION_MAIL_STATIONERY,
            mail_expire_delay_secs: MAIL_EXPIRY_SECS,
        },
    )
    .await?;
    let removed = match removed {
        Ok(removed) => removed,
        Err(wow_db::RemoveAuctionError::NotFoundOrNotOwner) => {
            return send_auction_command_result(
                stream,
                0,
                AUCTION_ACTION_REMOVED,
                AUCTION_ERR_DATABASE,
                None,
                header_crypto,
            )
            .await;
        }
        Err(wow_db::RemoveAuctionError::MissingItem) => {
            return send_auction_command_result(
                stream,
                0,
                AUCTION_ACTION_REMOVED,
                AUCTION_ERR_INVENTORY,
                Some(EQUIP_ERR_ITEM_NOT_FOUND_U32),
                header_crypto,
            )
            .await;
        }
        Err(wow_db::RemoveAuctionError::NotEnoughMoney) => return Ok(()),
    };

    if let Some(session_id) = deps
        .quest
        .shared_world
        .sessions
        .session_for_character(removed.bidder_guid)
        .await
    {
        deps.quest
            .shared_world
            .sessions
            .send_packet(
                session_id,
                build_auction_removed_notification_packet(
                    removed.auction_id,
                    removed.item_template,
                    removed.item_random_property_id,
                ),
            )
            .await;
    }

    send_auction_command_result(
        stream,
        removed.auction_id,
        AUCTION_ACTION_REMOVED,
        AUCTION_ERR_OK,
        None,
        header_crypto,
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &build_player_money_update_body(character.guid, removed.owner_money)?,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn handle_auction_place_bid(
    stream: &mut WorldPacketSink,
    deps: AuctionMutationDeps<'_>,
    request: wow_proto::AuctionPlaceBidRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        warn!("Ignoring auction bid before character login");
        return Ok(());
    };
    if request.auction_id == 0 || request.price == 0 || request.price > MAX_MONEY_AMOUNT {
        return Ok(());
    }

    let auctioneer = ObjectGuid::from_raw(request.auctioneer_raw_guid);
    let Some(access) = checked_auction_house_access(
        deps.maps,
        deps.world_data_files,
        deps.auction_config,
        session,
        auctioneer,
    )
    .await
    else {
        warn!(
            auctioneer = format_args!("0x{:016X}", auctioneer.raw()),
            "Ignoring auction bid for inaccessible non-auctioneer"
        );
        return Ok(());
    };
    let Some(auction_house_entry) = deps
        .world_data_files
        .auction_houses
        .get(&access.entry_house_id)
        .copied()
    else {
        warn!(
            house_id = access.entry_house_id,
            "AuctionHouse.dbc entry missing for auction bid"
        );
        return send_auction_command_result(
            stream,
            0,
            AUCTION_ACTION_BID_PLACED,
            AUCTION_ERR_DATABASE,
            None,
            header_crypto,
        )
        .await;
    };

    let placed = wow_db::place_auction_bid(
        deps.quest.character_db_pool,
        wow_db::PlaceAuctionBidRequest {
            auction_id: request.auction_id,
            house_bucket: access.storage_bucket,
            bidder_guid: character.guid,
            bidder_account_id: session.account.account_id,
            offered_bid: request.price,
            cut_percent: auction_house_entry.cut_percent,
            cut_rate: deps.auction_config.cut_rate,
            outbid_answer: AUCTION_OUTBIDDED_ANSWER,
            won_answer: AUCTION_WON_ANSWER,
            successful_answer: AUCTION_SUCCESSFUL_ANSWER,
            mail_checked: MAIL_CHECK_MASK_COPIED,
            mail_message_type: AUCTION_MAIL_MESSAGE_TYPE,
            mail_stationery: AUCTION_MAIL_STATIONERY,
            mail_expire_delay_secs: MAIL_EXPIRY_SECS,
        },
    )
    .await?;
    let placed = match placed {
        Ok(placed) => placed,
        Err(wow_db::PlaceAuctionBidError::NotFound | wow_db::PlaceAuctionBidError::BidOwn) => {
            return send_auction_bid_command_result(
                stream,
                AuctionBidCommandResult {
                    auction_id: 0,
                    error_code: AUCTION_ERR_BID_OWN,
                    bid_min_outbid: None,
                    higher_bidder: None,
                    higher_bid: None,
                    higher_min_outbid: None,
                },
                header_crypto,
            )
            .await;
        }
        Err(wow_db::PlaceAuctionBidError::MissingItem) => {
            return send_auction_command_result(
                stream,
                0,
                AUCTION_ACTION_BID_PLACED,
                AUCTION_ERR_INVENTORY,
                Some(EQUIP_ERR_ITEM_NOT_FOUND_U32),
                header_crypto,
            )
            .await;
        }
        Err(wow_db::PlaceAuctionBidError::HigherBid {
            current_bidder_guid,
            current_bid,
            current_min_outbid,
        }) => {
            return send_auction_bid_command_result(
                stream,
                AuctionBidCommandResult {
                    auction_id: request.auction_id,
                    error_code: AUCTION_ERR_HIGHER_BID,
                    bid_min_outbid: None,
                    higher_bidder: Some(ObjectGuid::new(HighGuid::Player, 0, current_bidder_guid)),
                    higher_bid: Some(current_bid),
                    higher_min_outbid: Some(current_min_outbid),
                },
                header_crypto,
            )
            .await;
        }
        Err(wow_db::PlaceAuctionBidError::BidIncrement) => {
            return send_auction_bid_command_result(
                stream,
                AuctionBidCommandResult {
                    auction_id: request.auction_id,
                    error_code: AUCTION_ERR_BID_INCREMENT,
                    bid_min_outbid: None,
                    higher_bidder: None,
                    higher_bid: None,
                    higher_min_outbid: None,
                },
                header_crypto,
            )
            .await;
        }
        Err(
            wow_db::PlaceAuctionBidError::NotEnoughMoney
            | wow_db::PlaceAuctionBidError::BelowStartBid,
        ) => {
            return Ok(());
        }
    };

    send_auction_bid_command_result(
        stream,
        AuctionBidCommandResult {
            auction_id: placed.auction_id,
            error_code: AUCTION_ERR_OK,
            bid_min_outbid: Some(placed.previous_min_outbid),
            higher_bidder: None,
            higher_bid: None,
            higher_min_outbid: None,
        },
        header_crypto,
    )
    .await?;

    if placed.previous_bidder_guid != 0 && placed.previous_bidder_guid != placed.current_bidder_guid
    {
        if let Some(previous_session_id) = deps
            .quest
            .shared_world
            .sessions
            .session_for_character(placed.previous_bidder_guid)
            .await
        {
            deps.quest
                .shared_world
                .sessions
                .send_packet(
                    previous_session_id,
                    build_auction_bidder_notification_packet(
                        placed.house_id,
                        placed.auction_id,
                        placed.previous_bidder_guid,
                        placed.previous_bid,
                        placed.previous_min_outbid,
                        placed.item_template,
                        placed.item_random_property_id,
                    ),
                )
                .await;
        }
    }

    if let Some(owner_session_id) = deps
        .quest
        .shared_world
        .sessions
        .session_for_character(placed.owner_guid)
        .await
    {
        deps.quest
            .shared_world
            .sessions
            .send_packet(
                owner_session_id,
                build_auction_owner_notification_packet(
                    placed.auction_id,
                    placed.current_bid,
                    placed.current_min_outbid,
                    placed.current_bidder_guid,
                    placed.completed_buyout,
                    placed.item_template,
                    placed.item_random_property_id,
                ),
            )
            .await;
    }

    if placed.completed_buyout {
        send_packet(
            stream,
            WorldOpcode::SmsgAuctionBidderNotification as u16,
            &build_auction_bidder_notification_packet(
                placed.house_id,
                placed.auction_id,
                placed.current_bidder_guid,
                0,
                placed.current_min_outbid,
                placed.item_template,
                placed.item_random_property_id,
            )
            .body,
            Some(&mut *header_crypto),
        )
        .await?;
    }

    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &build_player_money_update_body(character.guid, placed.bidder_money)?,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn handle_auction_list_items(
    stream: &mut WorldPacketSink,
    deps: AuctionBrowseDeps<'_>,
    request: wow_proto::AuctionListItemsRequest,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        warn!("Ignoring auction list request before character login");
        return Ok(());
    };
    let auctioneer = ObjectGuid::from_raw(request.auctioneer_raw_guid);
    let Some(access) = checked_auction_house_access(
        deps.maps,
        deps.world_data_files,
        deps.auction_config,
        session,
        auctioneer,
    )
    .await
    else {
        warn!(
            auctioneer = format_args!("0x{:016X}", auctioneer.raw()),
            "Ignoring auction search for inaccessible non-auctioneer"
        );
        return Ok(());
    };

    let now_secs = current_unix_time_secs();
    let search_name = request.searched_name.to_lowercase();
    let mut matching = Vec::new();
    for auction in
        wow_db::get_bucket_auctions(deps.character_db_pool, access.storage_bucket, now_secs).await?
    {
        let Some(template) =
            wow_db::get_item_template_query(deps.world_db_pool, auction.item_template).await?
        else {
            continue;
        };
        if !auction_matches_search(
            session,
            character,
            &auction,
            &template,
            &request,
            &search_name,
        ) {
            continue;
        }
        matching.push(build_auction_info_response(&auction, now_secs));
    }

    let total_count = matching.len() as u32;
    let auctions = paged_auction_infos(matching, request.list_from);
    send_packet(
        stream,
        WorldOpcode::SmsgAuctionListResult as u16,
        &SmsgAuctionListResultResponse {
            auctions,
            total_count,
        }
        .body(),
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn handle_auction_list_owner_items(
    stream: &mut WorldPacketSink,
    deps: AuctionBrowseDeps<'_>,
    request: wow_proto::AuctionListOwnerItemsRequest,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        warn!("Ignoring owner auction list request before character login");
        return Ok(());
    };
    let auctioneer = ObjectGuid::from_raw(request.auctioneer_raw_guid);
    let Some(access) = checked_auction_house_access(
        deps.maps,
        deps.world_data_files,
        deps.auction_config,
        session,
        auctioneer,
    )
    .await
    else {
        warn!(
            auctioneer = format_args!("0x{:016X}", auctioneer.raw()),
            "Ignoring owner auction list for inaccessible non-auctioneer"
        );
        return Ok(());
    };

    let now_secs = current_unix_time_secs();
    let matching: Vec<_> =
        wow_db::get_bucket_auctions(deps.character_db_pool, access.storage_bucket, now_secs)
            .await?
            .into_iter()
            .filter(|auction| auction.item_owner == character.guid)
            .map(|auction| build_auction_info_response(&auction, now_secs))
            .collect();
    let total_count = matching.len() as u32;
    let auctions = paged_auction_infos(matching, request.list_from);

    send_packet(
        stream,
        WorldOpcode::SmsgAuctionOwnerListResult as u16,
        &SmsgAuctionOwnerListResultResponse {
            auctions,
            total_count,
        }
        .body(),
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn handle_auction_list_bidder_items(
    stream: &mut WorldPacketSink,
    deps: AuctionBrowseDeps<'_>,
    request: wow_proto::AuctionListBidderItemsRequest,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        warn!("Ignoring bidder auction list request before character login");
        return Ok(());
    };
    let auctioneer = ObjectGuid::from_raw(request.auctioneer_raw_guid);
    let Some(access) = checked_auction_house_access(
        deps.maps,
        deps.world_data_files,
        deps.auction_config,
        session,
        auctioneer,
    )
    .await
    else {
        warn!(
            auctioneer = format_args!("0x{:016X}", auctioneer.raw()),
            "Ignoring bidder auction list for inaccessible non-auctioneer"
        );
        return Ok(());
    };

    let now_secs = current_unix_time_secs();
    let auctions =
        wow_db::get_bucket_auctions(deps.character_db_pool, access.storage_bucket, now_secs)
            .await?;
    let matching = collect_bidder_auction_infos(
        &auctions,
        character.guid,
        &request.outbid_auction_ids,
        now_secs,
    );
    let total_count = matching.len() as u32;
    let page = paged_auction_infos(matching, request.list_from);

    send_packet(
        stream,
        WorldOpcode::SmsgAuctionBidderListResult as u16,
        &SmsgAuctionBidderListResultResponse {
            auctions: page,
            total_count,
        }
        .body(),
        Some(header_crypto),
    )
    .await
}

fn collect_bidder_auction_infos(
    auctions: &[wow_db::AuctionRecord],
    bidder_guid: u32,
    outbid_auction_ids: &[u32],
    now_secs: u64,
) -> Vec<AuctionInfoResponse> {
    let mut seen = HashSet::new();
    let mut matching = Vec::new();

    for auction_id in outbid_auction_ids {
        let Some(auction) = auctions.iter().find(|auction| auction.id == *auction_id) else {
            continue;
        };
        if seen.insert(auction.id) {
            matching.push(build_auction_info_response(auction, now_secs));
        }
    }

    for auction in auctions
        .iter()
        .filter(|auction| auction.bidder == bidder_guid)
    {
        if seen.insert(auction.id) {
            matching.push(build_auction_info_response(auction, now_secs));
        }
    }

    matching
}

fn paged_auction_infos(
    mut auctions: Vec<AuctionInfoResponse>,
    list_from: u32,
) -> Vec<AuctionInfoResponse> {
    let page_start = usize::try_from(list_from).unwrap_or(usize::MAX);
    if page_start >= auctions.len() {
        return Vec::new();
    }
    auctions
        .drain(page_start..)
        .take(AUCTION_CLIENT_PAGE_SIZE)
        .collect()
}

fn build_auction_info_response(
    auction: &wow_db::AuctionRecord,
    now_secs: u64,
) -> AuctionInfoResponse {
    let enchantments = parse_item_enchantment_fields(&auction.enchantments);
    let charges = parse_item_spell_charges(&auction.charges);
    AuctionInfoResponse {
        id: auction.id,
        item: auction.item_template,
        enchantment: enchantments[0],
        random_property_id: auction.item_random_property_id as u32,
        suffix_factor: 0,
        count: auction.item_count,
        charges: charges[0] as u32,
        owner: ObjectGuid::new(HighGuid::Player, 0, auction.item_owner),
        start_bid: auction.start_bid,
        min_outbid: auction_min_outbid(auction.current_bid),
        buyout: auction.buyout_price,
        time_left_millis: auction_time_left_millis(auction.expire_time, now_secs),
        bidder: ObjectGuid::new(HighGuid::Player, 0, auction.bidder),
        current_bid: auction.current_bid,
    }
}

fn auction_matches_search(
    session: &WorldSessionState,
    character: &ActiveCharacter,
    _auction: &wow_db::AuctionRecord,
    template: &wow_db::ItemTemplateQuery,
    request: &wow_proto::AuctionListItemsRequest,
    search_name: &str,
) -> bool {
    if request.item_class != u32::MAX && template.class != request.item_class {
        return false;
    }
    if request.item_subclass != u32::MAX && template.subclass != request.item_subclass {
        return false;
    }
    if request.inventory_type != u32::MAX
        && template.inventory_type != request.inventory_type
        && (request.inventory_type != INVTYPE_CHEST || template.inventory_type != INVTYPE_ROBE)
    {
        return false;
    }
    if request.quality != u32::MAX && template.quality < request.quality {
        return false;
    }
    if request.level_min != 0
        && (template.required_level < u32::from(request.level_min)
            || (request.level_max != 0 && template.required_level > u32::from(request.level_max)))
    {
        return false;
    }
    if request.usable != 0
        && character_can_use_item_template(
            character.level,
            character.race,
            character.class,
            template,
            &session.character.character_skills,
            &session.character.active_spells,
            &session.character.character_reputations,
        ) != 0
    {
        return false;
    }
    search_name.is_empty() || template.name.to_lowercase().contains(search_name)
}

async fn auction_item_can_be_listed(
    character_db_pool: &MySqlPool,
    inventory: &[CharacterInventoryItem],
    owner_guid: u32,
    source_item: &CharacterInventoryItem,
    template: &wow_db::ItemTemplateQuery,
) -> anyhow::Result<bool> {
    let Some(instance_state) =
        wow_db::mail_attachment_instance_state(character_db_pool, owner_guid, source_item.item)
            .await?
    else {
        return Ok(false);
    };
    if !auction_attachment_can_be_listed(
        instance_state,
        template.flags,
        sell_item_is_non_empty_container(inventory, source_item, template),
    ) {
        return Ok(false);
    }
    Ok(true)
}

fn auction_attachment_can_be_listed(
    instance: wow_db::MailAttachmentInstanceState,
    template_flags: u32,
    non_empty_container: bool,
) -> bool {
    !item_instance_is_soulbound(instance.flags)
        && instance.duration == 0
        && template_flags & ITEM_FLAG_CONJURED == 0
        && !non_empty_container
}

fn auction_source_position_supported(source_item: &CharacterInventoryItem) -> bool {
    let bag = source_item.bag as u8;
    source_item.bag == INVENTORY_SLOT_BAG_0 as u32
        && (is_backpack_item_slot(source_item.slot) || is_inventory_bag_slot(source_item.slot))
        || is_inventory_bag_slot(bag)
}

fn auction_duration_secs_from_client(
    duration_minutes: u32,
    auction_config: AuctionRuntimeConfig,
) -> Option<u32> {
    let duration_secs = duration_minutes.checked_mul(60)?;
    if duration_secs == MIN_AUCTION_TIME_SECS
        || duration_secs == MIN_AUCTION_TIME_SECS * 4
        || duration_secs == MIN_AUCTION_TIME_SECS * 12
    {
        Some((duration_secs as f64 * auction_config.time_rate) as u32)
    } else {
        None
    }
}

fn auction_deposit_from_template(
    auction_house: &AuctionHouseEntry,
    duration_secs: u32,
    template: &wow_db::ItemTemplateQuery,
    count: u32,
    auction_config: AuctionRuntimeConfig,
) -> u32 {
    let base_deposit =
        template.sell_price.saturating_mul(count) * (duration_secs / MIN_AUCTION_TIME_SECS);
    let deposit = (base_deposit as f64 * auction_house.deposit_percent as f64 / 100.0)
        .max(auction_config.deposit_min as f64);
    (deposit * auction_config.deposit_rate) as u32
}

async fn send_auction_command_result(
    stream: &mut WorldPacketSink,
    auction_id: u32,
    action: u32,
    error_code: u32,
    inventory_error: Option<u32>,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    send_packet(
        stream,
        WorldOpcode::SmsgAuctionCommandResult as u16,
        &SmsgAuctionCommandResultResponse {
            auction_id,
            action,
            error_code,
            bid_min_outbid: None,
            inventory_error,
            higher_bidder: None,
            higher_bid: None,
            higher_min_outbid: None,
        }
        .body(),
        Some(header_crypto),
    )
    .await
}

async fn send_auction_bid_command_result(
    stream: &mut WorldPacketSink,
    result: AuctionBidCommandResult,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    send_packet(
        stream,
        WorldOpcode::SmsgAuctionCommandResult as u16,
        &SmsgAuctionCommandResultResponse {
            auction_id: result.auction_id,
            action: AUCTION_ACTION_BID_PLACED,
            error_code: result.error_code,
            bid_min_outbid: result.bid_min_outbid,
            inventory_error: None,
            higher_bidder: result.higher_bidder,
            higher_bid: result.higher_bid,
            higher_min_outbid: result.higher_min_outbid,
        }
        .body(),
        Some(header_crypto),
    )
    .await
}

fn auction_time_left_millis(expire_time: u64, now_secs: u64) -> u32 {
    expire_time
        .saturating_sub(now_secs)
        .saturating_mul(1_000)
        .min(u64::from(u32::MAX)) as u32
}

fn auction_min_outbid(current_bid: u32) -> u32 {
    if current_bid == 0 {
        return 0;
    }
    let outbid = (current_bid / 100) * 5;
    outbid.max(1)
}

async fn checked_auction_house_access(
    maps: &Arc<MapRuntimeManager>,
    world_data_files: &WorldDataFiles,
    auction_config: AuctionRuntimeConfig,
    session: &WorldSessionState,
    auctioneer: ObjectGuid,
) -> Option<AuctionHouseAccess> {
    let character = session.character.active_character.as_ref()?;
    if auctioneer == ObjectGuid::new(HighGuid::Player, 0, character.guid) {
        return session.account.gm_mode.then_some(AuctionHouseAccess {
            entry_house_id: AUCTION_HOUSE_NEUTRAL,
            storage_bucket: if auction_config.allow_two_side_interaction_auction {
                wow_db::AuctionHouseBucket::Global
            } else {
                wow_db::AuctionHouseBucket::Neutral
            },
        });
    }
    if !auctioneer.is_creature() {
        return None;
    }
    let creature = maps
        .db_creature_snapshot(character.position.map_id, auctioneer)
        .await?;
    if creature.spawn.template.npc_flags & UNIT_NPC_FLAG_AUCTIONEER == 0
        || !is_position_inside_radius(
            creature.current_position,
            character.position,
            AUCTIONEER_INTERACTION_DISTANCE_YARDS,
        )
    {
        return None;
    }
    Some(auction_house_access_for_faction(
        creature.spawn.template.faction,
        world_data_files,
        auction_config,
    ))
}

fn auction_house_access_for_faction(
    faction: u32,
    world_data_files: &WorldDataFiles,
    auction_config: AuctionRuntimeConfig,
) -> AuctionHouseAccess {
    if auction_config.allow_two_side_interaction_auction {
        return AuctionHouseAccess {
            entry_house_id: AUCTION_HOUSE_HUMAN,
            storage_bucket: wow_db::AuctionHouseBucket::Global,
        };
    }

    let entry_house_id = match faction {
        12 => AUCTION_HOUSE_HUMAN,
        29 => AUCTION_HOUSE_ORC,
        55 => AUCTION_HOUSE_DWARF,
        68 => AUCTION_HOUSE_UNDEAD,
        80 => AUCTION_HOUSE_NIGHT_ELF,
        104 => AUCTION_HOUSE_TROLL,
        120 | 474 | 855 => AUCTION_HOUSE_NEUTRAL,
        534 => AUCTION_HOUSE_DWARF,
        _ => match world_data_files.faction_templates.entry(faction) {
            Some(template) if template.faction_group_mask & FACTION_GROUP_MASK_ALLIANCE != 0 => {
                AUCTION_HOUSE_HUMAN
            }
            Some(template) if template.faction_group_mask & FACTION_GROUP_MASK_HORDE != 0 => {
                AUCTION_HOUSE_ORC
            }
            _ => AUCTION_HOUSE_NEUTRAL,
        },
    };
    AuctionHouseAccess {
        entry_house_id,
        storage_bucket: auction_storage_bucket_for_house(entry_house_id),
    }
}

fn auction_storage_bucket_for_house(house_id: u32) -> wow_db::AuctionHouseBucket {
    match house_id {
        AUCTION_HOUSE_HUMAN | AUCTION_HOUSE_DWARF | AUCTION_HOUSE_NIGHT_ELF => {
            wow_db::AuctionHouseBucket::Alliance
        }
        AUCTION_HOUSE_UNDEAD | AUCTION_HOUSE_TROLL | AUCTION_HOUSE_ORC => {
            wow_db::AuctionHouseBucket::Horde
        }
        _ => wow_db::AuctionHouseBucket::Neutral,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    #[test]
    fn auction_info_response_matches_cmangos_fields() {
        let auction = wow_db::AuctionRecord {
            id: 77,
            house_id: AUCTION_HOUSE_NEUTRAL,
            item_guid: 91,
            item_template: 744,
            item_count: 3,
            item_random_property_id: -15,
            item_owner: 11,
            buyout_price: 500,
            expire_time: 106,
            bidder: 12,
            current_bid: 125,
            start_bid: 100,
            deposit: 25,
            charges: "-1 0 0 0 0".to_string(),
            enchantments: "1900 0 0".to_string(),
        };

        let info = build_auction_info_response(&auction, 100);
        assert_eq!(info.id, 77);
        assert_eq!(info.item, 744);
        assert_eq!(info.enchantment, 1900);
        assert_eq!(info.random_property_id, u32::MAX - 14);
        assert_eq!(info.count, 3);
        assert_eq!(info.charges, u32::MAX);
        assert_eq!(info.owner, ObjectGuid::new(HighGuid::Player, 0, 11));
        assert_eq!(info.min_outbid, 5);
        assert_eq!(info.buyout, 500);
        assert_eq!(info.time_left_millis, 6_000);
        assert_eq!(info.bidder, ObjectGuid::new(HighGuid::Player, 0, 12));
        assert_eq!(info.current_bid, 125);
    }

    #[test]
    fn auction_house_id_uses_cmangos_faction_mapping_with_group_fallback() {
        let mut world_data = WorldDataFiles::fallback();
        world_data.faction_templates.entries.insert(
            9_999,
            FactionTemplateEntry {
                id: 9_999,
                faction: 9_999,
                faction_flags: 0,
                faction_group_mask: FACTION_GROUP_MASK_HORDE,
                friend_group_mask: 0,
                enemy_group_mask: 0,
                enemy_faction: [0; 4],
                friend_faction: [0; 4],
            },
        );

        let config = AuctionRuntimeConfig {
            allow_two_side_interaction_auction: false,
            time_rate: 1.0,
            deposit_rate: 1.0,
            cut_rate: 1.0,
            deposit_min: 0,
        };

        assert_eq!(
            auction_house_access_for_faction(12, &world_data, config).entry_house_id,
            AUCTION_HOUSE_HUMAN
        );
        assert_eq!(
            auction_house_access_for_faction(534, &world_data, config).entry_house_id,
            AUCTION_HOUSE_DWARF
        );
        assert_eq!(
            auction_house_access_for_faction(855, &world_data, config).entry_house_id,
            AUCTION_HOUSE_NEUTRAL
        );
        assert_eq!(
            auction_house_access_for_faction(9_999, &world_data, config).entry_house_id,
            AUCTION_HOUSE_ORC
        );
        assert_eq!(
            auction_house_access_for_faction(42_4242, &world_data, config).entry_house_id,
            AUCTION_HOUSE_NEUTRAL
        );
    }

    #[test]
    fn auction_house_access_groups_city_houses_but_can_flip_to_global_market() {
        let world_data = WorldDataFiles::fallback();
        let normal = AuctionRuntimeConfig {
            allow_two_side_interaction_auction: false,
            time_rate: 1.0,
            deposit_rate: 1.0,
            cut_rate: 1.0,
            deposit_min: 0,
        };
        let global = AuctionRuntimeConfig {
            allow_two_side_interaction_auction: true,
            ..normal
        };

        let stormwind = auction_house_access_for_faction(12, &world_data, normal);
        let ironforge = auction_house_access_for_faction(55, &world_data, normal);
        let orgrimmar = auction_house_access_for_faction(29, &world_data, normal);
        let goblin = auction_house_access_for_faction(120, &world_data, normal);
        let cross_faction = auction_house_access_for_faction(29, &world_data, global);

        assert_eq!(
            stormwind.storage_bucket,
            wow_db::AuctionHouseBucket::Alliance
        );
        assert_eq!(
            ironforge.storage_bucket,
            wow_db::AuctionHouseBucket::Alliance
        );
        assert_eq!(orgrimmar.storage_bucket, wow_db::AuctionHouseBucket::Horde);
        assert_eq!(goblin.storage_bucket, wow_db::AuctionHouseBucket::Neutral);
        assert_eq!(cross_faction.entry_house_id, AUCTION_HOUSE_HUMAN);
        assert_eq!(
            cross_faction.storage_bucket,
            wow_db::AuctionHouseBucket::Global
        );
    }

    #[test]
    fn bidder_listing_prioritizes_outbid_then_pages_combined_results() {
        let auctions = vec![
            wow_db::AuctionRecord {
                id: 11,
                house_id: AUCTION_HOUSE_NEUTRAL,
                item_guid: 101,
                item_template: 744,
                item_count: 1,
                item_random_property_id: 0,
                item_owner: 1,
                buyout_price: 500,
                expire_time: 200,
                bidder: 99,
                current_bid: 100,
                start_bid: 90,
                deposit: 25,
                charges: "0 0 0 0 0".to_string(),
                enchantments: "0 0 0".to_string(),
            },
            wow_db::AuctionRecord {
                id: 22,
                house_id: AUCTION_HOUSE_NEUTRAL,
                item_guid: 102,
                item_template: 745,
                item_count: 1,
                item_random_property_id: 0,
                item_owner: 2,
                buyout_price: 600,
                expire_time: 200,
                bidder: 99,
                current_bid: 120,
                start_bid: 100,
                deposit: 25,
                charges: "0 0 0 0 0".to_string(),
                enchantments: "0 0 0".to_string(),
            },
            wow_db::AuctionRecord {
                id: 33,
                house_id: AUCTION_HOUSE_NEUTRAL,
                item_guid: 103,
                item_template: 746,
                item_count: 1,
                item_random_property_id: 0,
                item_owner: 3,
                buyout_price: 700,
                expire_time: 200,
                bidder: 99,
                current_bid: 140,
                start_bid: 110,
                deposit: 25,
                charges: "0 0 0 0 0".to_string(),
                enchantments: "0 0 0".to_string(),
            },
        ];

        let combined = collect_bidder_auction_infos(&auctions, 99, &[22, 11, 22], 100);
        let ids: Vec<_> = combined.iter().map(|auction| auction.id).collect();
        assert_eq!(ids, vec![22, 11, 33]);

        let paged_ids: Vec<_> = paged_auction_infos(combined, 2)
            .into_iter()
            .map(|auction| auction.id)
            .collect();
        assert_eq!(paged_ids, vec![33]);
    }

    #[test]
    fn sell_duration_and_deposit_follow_cmangos_defaults() {
        let config = AuctionRuntimeConfig {
            allow_two_side_interaction_auction: false,
            time_rate: 1.0,
            deposit_rate: 1.0,
            cut_rate: 1.0,
            deposit_min: 0,
        };
        let entry = AuctionHouseEntry {
            house_id: AUCTION_HOUSE_HUMAN,
            faction: 12,
            deposit_percent: 25,
            cut_percent: 5,
        };
        let template = wow_db::ItemTemplateQuery {
            entry: 744,
            class: 2,
            subclass: 7,
            name: "Test Sword".to_string(),
            displayid: 1,
            quality: 2,
            flags: 0,
            buy_price: 100,
            sell_price: 25,
            inventory_type: 13,
            allowable_class: -1,
            allowable_race: -1,
            item_level: 10,
            required_level: 1,
            required_skill: 0,
            required_skill_rank: 0,
            required_spell: 0,
            required_honor_rank: 0,
            required_city_rank: 0,
            required_reputation_faction: 0,
            required_reputation_rank: 0,
            max_count: 0,
            stackable: 1,
            container_slots: 0,
            stats: [wow_db::ItemTemplateStat::default(); 10],
            damage: [wow_db::ItemTemplateDamage::default(); 5],
            dmg_min1: 0.0,
            dmg_max1: 0.0,
            dmg_type1: 0,
            armor: 0,
            holy_res: 0,
            fire_res: 0,
            nature_res: 0,
            frost_res: 0,
            shadow_res: 0,
            arcane_res: 0,
            delay: 0,
            ammo_type: 0,
            ranged_mod_range: 0.0,
            spells: [wow_db::ItemTemplateSpell::default(); 5],
            bonding: 0,
            description: String::new(),
            page_text: 0,
            language_id: 0,
            page_material: 0,
            start_quest: 0,
            lock_id: 0,
            material: 0,
            sheath: 0,
            random_property: 0,
            block: 0,
            itemset: 0,
            max_durability: 0,
            area: 0,
            map: 0,
            bag_family: 0,
        };

        assert_eq!(auction_duration_secs_from_client(120, config), Some(7_200));
        assert_eq!(auction_duration_secs_from_client(121, config), None);
        assert_eq!(
            auction_deposit_from_template(&entry, 7_200, &template, 3, config),
            18
        );
    }

    #[test]
    fn sell_duration_and_deposit_follow_runtime_knobs() {
        let config = AuctionRuntimeConfig {
            allow_two_side_interaction_auction: true,
            time_rate: 2.0,
            deposit_rate: 1.5,
            cut_rate: 0.5,
            deposit_min: 100,
        };
        let entry = AuctionHouseEntry {
            house_id: AUCTION_HOUSE_HUMAN,
            faction: 12,
            deposit_percent: 25,
            cut_percent: 5,
        };
        let template = wow_db::ItemTemplateQuery {
            entry: 744,
            class: 2,
            subclass: 7,
            name: "Test Sword".to_string(),
            displayid: 1,
            quality: 2,
            flags: 0,
            buy_price: 100,
            sell_price: 25,
            inventory_type: 13,
            allowable_class: -1,
            allowable_race: -1,
            item_level: 10,
            required_level: 1,
            required_skill: 0,
            required_skill_rank: 0,
            required_spell: 0,
            required_honor_rank: 0,
            required_city_rank: 0,
            required_reputation_faction: 0,
            required_reputation_rank: 0,
            max_count: 0,
            stackable: 1,
            container_slots: 0,
            stats: [wow_db::ItemTemplateStat::default(); 10],
            damage: [wow_db::ItemTemplateDamage::default(); 5],
            dmg_min1: 0.0,
            dmg_max1: 0.0,
            dmg_type1: 0,
            armor: 0,
            holy_res: 0,
            fire_res: 0,
            nature_res: 0,
            frost_res: 0,
            shadow_res: 0,
            arcane_res: 0,
            delay: 0,
            ammo_type: 0,
            ranged_mod_range: 0.0,
            spells: [wow_db::ItemTemplateSpell::default(); 5],
            bonding: 0,
            description: String::new(),
            page_text: 0,
            language_id: 0,
            page_material: 0,
            start_quest: 0,
            lock_id: 0,
            material: 0,
            sheath: 0,
            random_property: 0,
            block: 0,
            itemset: 0,
            max_durability: 0,
            area: 0,
            map: 0,
            bag_family: 0,
        };

        assert_eq!(auction_duration_secs_from_client(120, config), Some(14_400));
        assert_eq!(
            auction_deposit_from_template(&entry, 7_200, &template, 1, config),
            150
        );
    }

    #[test]
    fn auction_listing_rejects_soulbound_but_not_unrelated_instance_flags() {
        assert!(!auction_attachment_can_be_listed(
            wow_db::MailAttachmentInstanceState {
                flags: ITEM_DYNFLAG_BINDED,
                duration: 0,
            },
            0,
            false,
        ));
        assert!(auction_attachment_can_be_listed(
            wow_db::MailAttachmentInstanceState {
                flags: 0x0000_0200,
                duration: 0,
            },
            0,
            false,
        ));
    }

    #[test]
    fn sold_owner_notification_zeroes_bidder_guid_for_client_sold_message() {
        let packet = build_auction_owner_notification_packet(77, 500, 25, 42, true, 744, -15);
        let body = Bytes::from(packet.body);

        assert_eq!(
            packet.opcode,
            WorldOpcode::SmsgAuctionOwnerNotification as u16
        );
        assert_eq!(body.len(), 28);
        assert_eq!(u32::from_le_bytes(body[0..4].try_into().unwrap()), 77);
        assert_eq!(u32::from_le_bytes(body[4..8].try_into().unwrap()), 500);
        assert_eq!(u32::from_le_bytes(body[8..12].try_into().unwrap()), 25);
        assert_eq!(u64::from_le_bytes(body[12..20].try_into().unwrap()), 0);
        assert_eq!(u32::from_le_bytes(body[20..24].try_into().unwrap()), 744);
        assert_eq!(i32::from_le_bytes(body[24..28].try_into().unwrap()), -15);
    }

    #[test]
    fn won_bidder_notification_zeroes_bid_amount_for_client_won_message() {
        let packet = build_auction_bidder_notification_packet(7, 88, 42, 0, 25, 744, -15);
        let body = Bytes::from(packet.body);

        assert_eq!(
            packet.opcode,
            WorldOpcode::SmsgAuctionBidderNotification as u16
        );
        assert_eq!(body.len(), 32);
        assert_eq!(u32::from_le_bytes(body[0..4].try_into().unwrap()), 7);
        assert_eq!(u32::from_le_bytes(body[4..8].try_into().unwrap()), 88);
        assert_eq!(
            u64::from_le_bytes(body[8..16].try_into().unwrap()),
            ObjectGuid::new(HighGuid::Player, 0, 42).raw()
        );
        assert_eq!(u32::from_le_bytes(body[16..20].try_into().unwrap()), 0);
        assert_eq!(u32::from_le_bytes(body[20..24].try_into().unwrap()), 25);
        assert_eq!(u32::from_le_bytes(body[24..28].try_into().unwrap()), 744);
        assert_eq!(i32::from_le_bytes(body[28..32].try_into().unwrap()), -15);
    }
}
