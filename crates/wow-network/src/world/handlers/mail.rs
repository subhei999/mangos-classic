use super::*;
use wow_proto::{
    MailListItemResponse, ServerWorldPacket, SmsgItemTextQueryResponse, SmsgMailListResultResponse,
    SmsgSendMailResultResponse,
};

const GO_TYPE_MAILBOX: u8 = 19;
const MAIL_NORMAL: u8 = 0;
const MAIL_STATIONERY_DEFAULT: u8 = 41;
const MAIL_CHECK_MASK_COPIED: u8 = 0x04;
const MAIL_CHECK_MASK_HAS_BODY: u8 = 0x10;
const MAIL_DELIVERY_DELAY_SECS: u64 = 60 * 60;
const MAIL_MONEY_DELIVERY_DELAY_SECS: u64 = 0;
const MAIL_EXPIRY_SECS: u64 = 30 * 24 * 60 * 60;
const MAIL_COD_EXPIRY_SECS: u64 = 3 * 24 * 60 * 60;
const MAIL_POSTAGE_COPPER: u32 = 30;
const MAX_MAILBOX_MAILS: u32 = 100;
const MAX_INBOX_CLIENT_UI_CAPACITY: u32 = 50;
const MAIL_BODY_ITEM_TEMPLATE: u32 = 8383;
const ITEM_FLAG_CONJURED: u32 = 0x0000_0002;
const ITEM_DYNFLAG_WRAPPED: u32 = 0x0000_0200;

pub(in crate::world) async fn dispatch_mail_packet(
    ctx: &mut WorldPacketDispatchContext<'_>,
    packet: &packets::ParsedWorldClientPacket,
) -> anyhow::Result<()> {
    let deps = MailDeps {
        character_db_pool: ctx.character_db_pool,
        world_db_pool: ctx.world_db_pool,
        shared_world: SharedWorldDeps {
            object_mgr: ctx.runtime_state.object_mgr.as_ref(),
            maps: &ctx.runtime_state.maps,
            sessions: &ctx.runtime_state.sessions,
        },
        account_id: ctx.account_id,
    };
    match packet {
        packets::ParsedWorldClientPacket::SendMail(_) => {
            handle_send_mail(
                &mut *ctx.stream,
                deps,
                packet.send_mail()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::GetMailList(_) => {
            handle_get_mail_list(
                &mut *ctx.stream,
                deps,
                packet.get_mail_list()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::MailTakeMoney(_) => {
            handle_mail_take_money(
                &mut *ctx.stream,
                deps,
                packet.mail_id_request()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::MailTakeItem(_) => {
            handle_mail_take_item(
                &mut *ctx.stream,
                deps,
                packet.mail_id_request()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::MailMarkAsRead(_) => {
            handle_mail_mark_as_read(deps, packet.mail_id_request()?, &mut *ctx.session).await
        }
        packets::ParsedWorldClientPacket::MailReturnToSender(_) => {
            handle_mail_return_to_sender(
                &mut *ctx.stream,
                deps,
                packet.mail_id_request()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::MailDelete(_) => {
            handle_mail_delete(
                &mut *ctx.stream,
                deps,
                packet.mail_id_request()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::MailCreateTextItem(_) => {
            handle_mail_create_text_item(
                &mut *ctx.stream,
                deps,
                packet.mail_create_text_item()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::ItemTextQuery(_) => {
            handle_item_text_query(
                &mut *ctx.stream,
                deps.character_db_pool,
                packet.item_text_query()?,
                &mut *ctx.header_crypto,
            )
            .await
        }
        other => anyhow::bail!("mail router received opcode 0x{:04X}", other.opcode()),
    }
}

#[derive(Clone, Copy)]
pub(in crate::world) struct MailDeps<'a> {
    pub(in crate::world) character_db_pool: &'a MySqlPool,
    pub(in crate::world) world_db_pool: &'a MySqlPool,
    pub(in crate::world) shared_world: SharedWorldDeps<'a>,
    pub(in crate::world) account_id: u32,
}

pub(in crate::world) async fn handle_send_mail(
    stream: &mut WorldPacketSink,
    deps: MailDeps<'_>,
    request: wow_proto::SendMailRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let character_guid = character.guid;
    if !check_mailbox(deps.shared_world.maps, session, request.mailbox_raw_guid).await {
        return Ok(());
    }
    if request.receiver.trim().is_empty() {
        return Ok(());
    }
    let Some(recipient) =
        wow_db::find_mail_recipient_by_name(deps.character_db_pool, request.receiver.trim())
            .await?
    else {
        return send_mail_result(
            stream,
            0,
            MAIL_SEND,
            MAIL_ERR_RECIPIENT_NOT_FOUND,
            None,
            None,
            header_crypto,
        )
        .await;
    };
    if recipient.guid == character_guid {
        return send_mail_result(
            stream,
            0,
            MAIL_SEND,
            MAIL_ERR_CANNOT_SEND_TO_SELF,
            None,
            None,
            header_crypto,
        )
        .await;
    }
    if player_team_for_race(character.race)
        .zip(player_team_for_race(recipient.race))
        .is_some_and(|(sender_team, recipient_team)| sender_team != recipient_team)
    {
        return send_mail_result(
            stream,
            0,
            MAIL_SEND,
            MAIL_ERR_NOT_YOUR_TEAM,
            None,
            None,
            header_crypto,
        )
        .await;
    }
    if request.money != 0 && request.cod != 0 {
        return send_mail_result(
            stream,
            0,
            MAIL_SEND,
            MAIL_ERR_INTERNAL_ERROR,
            None,
            None,
            header_crypto,
        )
        .await;
    }
    if wow_db::mail_count_for_receiver(deps.character_db_pool, recipient.guid).await?
        > MAX_MAILBOX_MAILS
    {
        return send_mail_result(
            stream,
            0,
            MAIL_SEND,
            MAIL_ERR_RECIPIENT_CAP_REACHED,
            None,
            None,
            header_crypto,
        )
        .await;
    }

    let item_guid = ObjectGuid::from_raw(request.item_raw_guid);
    let attached_item_guid = if item_guid == ObjectGuid::EMPTY {
        None
    } else {
        Some(item_guid.counter())
    };
    if let Some(item_guid) = attached_item_guid {
        let Some(item) = session
            .inventory
            .items
            .iter()
            .find(|item| item.item == item_guid)
        else {
            return send_mail_result(
                stream,
                0,
                MAIL_SEND,
                MAIL_ERR_MAIL_ATTACHMENT_INVALID,
                None,
                None,
                header_crypto,
            )
            .await;
        };
        let Some(template) =
            wow_db::get_item_template_query(deps.world_db_pool, item.item_template).await?
        else {
            return send_mail_result(
                stream,
                0,
                MAIL_SEND,
                MAIL_ERR_MAIL_ATTACHMENT_INVALID,
                None,
                None,
                header_crypto,
            )
            .await;
        };
        if template.flags & ITEM_FLAG_CONJURED != 0 {
            return send_mail_result(
                stream,
                0,
                MAIL_SEND,
                MAIL_ERR_MAIL_ATTACHMENT_INVALID,
                None,
                None,
                header_crypto,
            )
            .await;
        }
        if template.container_slots != 0
            && wow_db::inventory_items_in_container(
                deps.character_db_pool,
                character_guid,
                item.item,
            )
            .await?
                != 0
        {
            return send_mail_result(
                stream,
                0,
                MAIL_SEND,
                MAIL_ERR_MAIL_ATTACHMENT_INVALID,
                None,
                None,
                header_crypto,
            )
            .await;
        }
        let Some(instance) = wow_db::mail_attachment_instance_state(
            deps.character_db_pool,
            character_guid,
            item.item,
        )
        .await?
        else {
            return send_mail_result(
                stream,
                0,
                MAIL_SEND,
                MAIL_ERR_MAIL_ATTACHMENT_INVALID,
                None,
                None,
                header_crypto,
            )
            .await;
        };
        if instance.duration != 0 {
            return send_mail_result(
                stream,
                0,
                MAIL_SEND,
                MAIL_ERR_MAIL_ATTACHMENT_INVALID,
                None,
                None,
                header_crypto,
            )
            .await;
        }
        if request.cod != 0 && instance.flags & ITEM_DYNFLAG_WRAPPED != 0 {
            return send_mail_result(
                stream,
                0,
                MAIL_SEND,
                MAIL_ERR_CANT_SEND_WRAPPED_COD,
                None,
                None,
                header_crypto,
            )
            .await;
        }
    }

    let item_text_id = wow_db::create_item_text(deps.character_db_pool, &request.body).await?;
    let charge = MAIL_POSTAGE_COPPER.saturating_add(request.money);
    let deliver_delay_secs = send_mail_delivery_delay_secs(&request, attached_item_guid);
    let result = wow_db::send_character_mail(
        deps.character_db_pool,
        wow_db::SendCharacterMailRequest {
            sender: character_guid,
            receiver: recipient.guid,
            subject: request.subject.clone(),
            item_text_id,
            money: request.money,
            cod: request.cod,
            checked: if request.body.is_empty() {
                MAIL_CHECK_MASK_COPIED
            } else {
                MAIL_CHECK_MASK_HAS_BODY
            },
            deliver_delay_secs,
            expire_delay_secs: if request.cod != 0 {
                MAIL_COD_EXPIRY_SECS
            } else {
                MAIL_EXPIRY_SECS
            },
            attached_item_guid,
            stationery: MAIL_STATIONERY_DEFAULT,
            message_type: MAIL_NORMAL,
            mail_template_id: 0,
        },
        charge,
    )
    .await;
    let result = match result {
        Ok(result) => result,
        Err(wow_db::SendCharacterMailError::NotEnoughMoney) => {
            return send_mail_result(
                stream,
                0,
                MAIL_SEND,
                MAIL_ERR_NOT_ENOUGH_MONEY,
                None,
                None,
                header_crypto,
            )
            .await;
        }
        Err(wow_db::SendCharacterMailError::MissingAttachment) => {
            return send_mail_result(
                stream,
                0,
                MAIL_SEND,
                MAIL_ERR_MAIL_ATTACHMENT_INVALID,
                None,
                None,
                header_crypto,
            )
            .await;
        }
    };
    send_mail_result(stream, 0, MAIL_SEND, MAIL_OK, None, None, header_crypto).await?;
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_money_update_body(character_guid, result.sender_money)?,
        Some(&mut *header_crypto),
    )
    .await?;
    if let Some(item_guid) = attached_item_guid {
        refresh_inventory_after_mail_change(stream, deps, session, Some(item_guid), header_crypto)
            .await?;
    }
    Ok(())
}

fn send_mail_delivery_delay_secs(
    request: &wow_proto::SendMailRequest,
    attached_item_guid: Option<u32>,
) -> u64 {
    if request.money != 0 && request.cod == 0 && attached_item_guid.is_none() {
        MAIL_MONEY_DELIVERY_DELAY_SECS
    } else {
        MAIL_DELIVERY_DELAY_SECS
    }
}

pub(in crate::world) async fn handle_get_mail_list(
    stream: &mut WorldPacketSink,
    deps: MailDeps<'_>,
    request: wow_proto::GetMailListRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if !check_mailbox(deps.shared_world.maps, session, request.raw_guid).await {
        return Ok(());
    }
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let mails = wow_db::load_delivered_mail(
        deps.character_db_pool,
        character.guid,
        MAX_INBOX_CLIENT_UI_CAPACITY,
    )
    .await?;
    let body = build_mail_list_body(deps.world_db_pool, &mails).await?;
    send_packet(stream, SMSG_MAIL_LIST_RESULT, &body, Some(header_crypto)).await
}

pub(in crate::world) async fn handle_mail_take_money(
    stream: &mut WorldPacketSink,
    deps: MailDeps<'_>,
    request: wow_proto::MailIdRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if !check_mailbox(deps.shared_world.maps, session, request.mailbox_raw_guid).await {
        return Ok(());
    }
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    match wow_db::take_mail_money(deps.character_db_pool, character.guid, request.mail_id).await? {
        Some(new_money) => {
            send_mail_result(
                stream,
                request.mail_id,
                MAIL_MONEY_TAKEN,
                MAIL_OK,
                None,
                None,
                header_crypto,
            )
            .await?;
            send_packet(
                stream,
                SMSG_UPDATE_OBJECT,
                &build_player_money_update_body(character.guid, new_money)?,
                Some(header_crypto),
            )
            .await
        }
        None => {
            send_mail_result(
                stream,
                request.mail_id,
                MAIL_MONEY_TAKEN,
                MAIL_ERR_INTERNAL_ERROR,
                None,
                None,
                header_crypto,
            )
            .await
        }
    }
}

pub(in crate::world) async fn handle_mail_take_item(
    stream: &mut WorldPacketSink,
    deps: MailDeps<'_>,
    request: wow_proto::MailIdRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if !check_mailbox(deps.shared_world.maps, session, request.mailbox_raw_guid).await {
        return Ok(());
    }
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let character_guid = character.guid;
    let Some(mail) =
        wow_db::load_mail(deps.character_db_pool, character_guid, request.mail_id).await?
    else {
        return send_mail_result(
            stream,
            request.mail_id,
            MAIL_ITEM_TAKEN,
            MAIL_ERR_INTERNAL_ERROR,
            None,
            None,
            header_crypto,
        )
        .await;
    };
    let Some(item) = mail.items.first() else {
        return send_mail_result(
            stream,
            request.mail_id,
            MAIL_ITEM_TAKEN,
            MAIL_ERR_INTERNAL_ERROR,
            None,
            None,
            header_crypto,
        )
        .await;
    };
    let Some(template) =
        wow_db::get_item_template_query(deps.world_db_pool, item.item_template).await?
    else {
        return Ok(());
    };
    let equipped_bags =
        load_equipped_bag_infos(deps.world_db_pool, &session.inventory.items).await?;
    let Some(store_plan) = plan_store_item(
        &session.inventory.items,
        &template,
        item.count,
        &equipped_bags,
        None,
        None,
    ) else {
        return send_mail_result(
            stream,
            request.mail_id,
            MAIL_ITEM_TAKEN,
            MAIL_ERR_EQUIP_ERROR,
            Some(EQUIP_ERR_INVENTORY_FULL as u32),
            None,
            header_crypto,
        )
        .await;
    };
    if store_plan.len() != 1 {
        return send_mail_result(
            stream,
            request.mail_id,
            MAIL_ITEM_TAKEN,
            MAIL_ERR_EQUIP_ERROR,
            Some(EQUIP_ERR_INVENTORY_FULL as u32),
            None,
            header_crypto,
        )
        .await;
    }
    let slot = store_plan[0];
    let store_target = if let Some(existing_item) = slot.existing_item {
        wow_db::MailStoreTarget::MergeStack {
            item_guid: existing_item,
            new_count: session
                .inventory
                .items
                .iter()
                .find(|item| item.item == existing_item)
                .map(|item| item.count)
                .unwrap_or(0)
                .saturating_add(slot.count),
        }
    } else {
        wow_db::MailStoreTarget::EmptySlot {
            bag: slot.bag as u32,
            slot: slot.slot,
        }
    };
    let take_result = wow_db::take_mail_item(
        deps.character_db_pool,
        wow_db::TakeMailItemRequest {
            receiver: character_guid,
            mail_id: request.mail_id,
            item_guid: item.item_guid,
            store_target,
            cod_sender: (mail.sender != 0).then_some(mail.sender),
            cod_expire_delay_secs: MAIL_EXPIRY_SECS,
        },
    )
    .await;
    match take_result {
        Ok(()) => {}
        Err(wow_db::TakeMailItemError::NotEnoughMoney) => {
            return send_mail_result(
                stream,
                request.mail_id,
                MAIL_ITEM_TAKEN,
                MAIL_ERR_NOT_ENOUGH_MONEY,
                None,
                None,
                header_crypto,
            )
            .await;
        }
        Err(_) => {
            return send_mail_result(
                stream,
                request.mail_id,
                MAIL_ITEM_TAKEN,
                MAIL_ERR_INTERNAL_ERROR,
                None,
                None,
                header_crypto,
            )
            .await;
        }
    }
    send_mail_result(
        stream,
        request.mail_id,
        MAIL_ITEM_TAKEN,
        MAIL_OK,
        None,
        Some((item.item_template, item.count)),
        header_crypto,
    )
    .await?;
    refresh_inventory_after_mail_change(stream, deps, session, Some(item.item_guid), header_crypto)
        .await?;
    if mail.cod != 0 {
        let money = sqlx::query_scalar("SELECT money FROM characters WHERE guid = ?")
            .bind(character_guid)
            .fetch_one(deps.character_db_pool)
            .await?;
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &build_player_money_update_body(character_guid, money)?,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    complete_inventory_item_quests(
        stream,
        deps.character_db_pool,
        deps.shared_world.object_mgr,
        deps.world_db_pool,
        session,
        character_guid,
        header_crypto,
    )
    .await
}

pub(in crate::world) async fn handle_mail_mark_as_read(
    deps: MailDeps<'_>,
    request: wow_proto::MailIdRequest,
    session: &mut WorldSessionState,
) -> anyhow::Result<()> {
    if !check_mailbox(deps.shared_world.maps, session, request.mailbox_raw_guid).await {
        return Ok(());
    }
    if let Some(character) = session.character.active_character.as_ref() {
        wow_db::mark_mail_read(deps.character_db_pool, character.guid, request.mail_id).await?;
    }
    Ok(())
}

pub(in crate::world) async fn handle_mail_return_to_sender(
    stream: &mut WorldPacketSink,
    deps: MailDeps<'_>,
    request: wow_proto::MailIdRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if !check_mailbox(deps.shared_world.maps, session, request.mailbox_raw_guid).await {
        return Ok(());
    }
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let returned = wow_db::return_mail_to_sender(
        deps.character_db_pool,
        character.guid,
        request.mail_id,
        deps.account_id,
        MAIL_DELIVERY_DELAY_SECS,
    )
    .await?;
    send_mail_result(
        stream,
        request.mail_id,
        MAIL_RETURNED_TO_SENDER,
        if returned {
            MAIL_OK
        } else {
            MAIL_ERR_INTERNAL_ERROR
        },
        None,
        None,
        header_crypto,
    )
    .await
}

pub(in crate::world) async fn handle_mail_delete(
    stream: &mut WorldPacketSink,
    deps: MailDeps<'_>,
    request: wow_proto::MailIdRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if !check_mailbox(deps.shared_world.maps, session, request.mailbox_raw_guid).await {
        return Ok(());
    }
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let deleted =
        wow_db::delete_mail(deps.character_db_pool, character.guid, request.mail_id).await?;
    send_mail_result(
        stream,
        request.mail_id,
        MAIL_DELETED,
        if deleted {
            MAIL_OK
        } else {
            MAIL_ERR_INTERNAL_ERROR
        },
        None,
        None,
        header_crypto,
    )
    .await
}

pub(in crate::world) async fn handle_mail_create_text_item(
    stream: &mut WorldPacketSink,
    deps: MailDeps<'_>,
    request: wow_proto::MailCreateTextItemRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if !check_mailbox(deps.shared_world.maps, session, request.mailbox_raw_guid).await {
        return Ok(());
    }
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let Some(mail) =
        wow_db::load_mail(deps.character_db_pool, character.guid, request.mail_id).await?
    else {
        return send_mail_result(
            stream,
            request.mail_id,
            MAIL_MADE_PERMANENT,
            MAIL_ERR_INTERNAL_ERROR,
            None,
            None,
            header_crypto,
        )
        .await;
    };
    if mail.item_text_id == 0 && mail.mail_template_id == 0 {
        return send_mail_result(
            stream,
            request.mail_id,
            MAIL_MADE_PERMANENT,
            MAIL_ERR_INTERNAL_ERROR,
            None,
            None,
            header_crypto,
        )
        .await;
    }
    let Some(template) =
        wow_db::get_item_template_query(deps.world_db_pool, MAIL_BODY_ITEM_TEMPLATE).await?
    else {
        return Ok(());
    };
    let equipped_bags =
        load_equipped_bag_infos(deps.world_db_pool, &session.inventory.items).await?;
    let Some(store_plan) = plan_store_item(
        &session.inventory.items,
        &template,
        1,
        &equipped_bags,
        None,
        None,
    ) else {
        return send_mail_result(
            stream,
            request.mail_id,
            MAIL_MADE_PERMANENT,
            MAIL_ERR_EQUIP_ERROR,
            Some(EQUIP_ERR_INVENTORY_FULL as u32),
            None,
            header_crypto,
        )
        .await;
    };
    let Some(slot) = store_plan
        .first()
        .filter(|slot| slot.existing_item.is_none())
    else {
        return send_mail_result(
            stream,
            request.mail_id,
            MAIL_MADE_PERMANENT,
            MAIL_ERR_EQUIP_ERROR,
            Some(EQUIP_ERR_INVENTORY_FULL as u32),
            None,
            header_crypto,
        )
        .await;
    };
    let body_item = wow_db::add_character_inventory_item(
        deps.character_db_pool,
        character.guid,
        slot.bag as u32,
        slot.slot,
        MAIL_BODY_ITEM_TEMPLATE,
        1,
        template.max_durability,
    )
    .await?;
    sqlx::query("UPDATE item_instance SET itemTextId = ?, creatorGuid = ? WHERE guid = ?")
        .bind(mail.item_text_id)
        .bind(mail.sender)
        .bind(body_item.item)
        .execute(deps.character_db_pool)
        .await?;
    wow_db::update_mail_checked_mask(
        deps.character_db_pool,
        character.guid,
        request.mail_id,
        MAIL_CHECK_MASK_COPIED,
    )
    .await?;
    send_mail_result(
        stream,
        request.mail_id,
        MAIL_MADE_PERMANENT,
        MAIL_OK,
        None,
        None,
        header_crypto,
    )
    .await?;
    refresh_inventory_after_mail_change(stream, deps, session, None, header_crypto).await
}

pub(in crate::world) async fn handle_item_text_query(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    request: wow_proto::ItemTextQueryRequest,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let text = wow_db::get_item_text(character_db_pool, request.item_text_id)
        .await?
        .unwrap_or_default();
    let body = SmsgItemTextQueryResponse {
        item_text_id: request.item_text_id,
        text,
    }
    .body();
    send_packet(
        stream,
        SMSG_ITEM_TEXT_QUERY_RESPONSE,
        &body,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn check_mailbox(
    maps: &MapRuntimeManager,
    session: &WorldSessionState,
    mailbox_raw_guid: u64,
) -> bool {
    let Some(character) = session.character.active_character.as_ref() else {
        return false;
    };
    let mailbox_guid = ObjectGuid::from_raw(mailbox_raw_guid);
    if !mailbox_guid.is_game_object() {
        return false;
    }
    let Some(mailbox) = maps
        .db_gameobject_snapshot(character.position.map_id, mailbox_guid)
        .await
    else {
        return false;
    };
    mailbox.spawn.template.object_type == GO_TYPE_MAILBOX
        && is_position_inside_radius(mailbox.position(), character.position, 8.0)
}

pub(in crate::world) async fn build_mail_list_body(
    world_db_pool: &MySqlPool,
    mails: &[wow_db::CharacterMail],
) -> anyhow::Result<Vec<u8>> {
    let mut responses = Vec::new();
    for mail in mails.iter().take(MAX_INBOX_CLIENT_UI_CAPACITY as usize) {
        let item = mail.items.first();
        let max_durability = if let Some(item) = item {
            wow_db::get_item_template_query(world_db_pool, item.item_template)
                .await?
                .map(|template| template.max_durability)
                .unwrap_or(item.durability)
        } else {
            0
        };
        let enchantment = item
            .map(|item| parse_item_enchantment_fields(&item.enchantments)[0])
            .unwrap_or(0);
        let charges = item
            .map(|item| parse_item_spell_charges(&item.charges)[0].max(0) as u32)
            .unwrap_or(0);
        let (sender_raw_guid, sender_entry) = match mail.message_type {
            0 => (
                Some(ObjectGuid::new(HighGuid::Player, 0, mail.sender).raw()),
                None,
            ),
            2..=4 => (None, Some(mail.sender)),
            _ => (None, None),
        };
        responses.push(MailListItemResponse {
            mail_id: mail.id,
            message_type: mail.message_type,
            sender_raw_guid,
            sender_entry,
            subject: mail.subject.clone(),
            item_text_id: mail.item_text_id,
            package_id: 0,
            stationery: mail.stationery as u32,
            item_entry: item.map(|item| item.item_template).unwrap_or(0),
            item_enchantment: enchantment,
            item_random_property_id: item.map(|item| item.random_property_id as i32).unwrap_or(0),
            item_suffix_factor: 0,
            item_count: item
                .map(|item| item.count.min(u8::MAX as u32) as u8)
                .unwrap_or(0),
            item_charges: charges,
            item_max_durability: max_durability,
            item_durability: item.map(|item| item.durability).unwrap_or(0),
            money: mail.money,
            cod: mail.cod,
            checked: mail.checked as u32,
            expire_delay_days: mail_expire_delay_days(mail.expire_time),
            mail_template_id: mail.mail_template_id,
        });
    }
    Ok(SmsgMailListResultResponse { mails: responses }.body())
}

pub(in crate::world) fn mail_expire_delay_days(expire_time: u64) -> f32 {
    let now = current_unix_time();
    expire_time.saturating_sub(now) as f32 / 86_400.0
}

pub(in crate::world) async fn refresh_inventory_after_mail_change(
    stream: &mut WorldPacketSink,
    deps: MailDeps<'_>,
    session: &mut WorldSessionState,
    removed_item: Option<u32>,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let old_inventory = session.inventory.items.clone();
    session.inventory.items =
        wow_db::get_character_inventory_items(deps.character_db_pool, character.guid).await?;
    deps.shared_world
        .maps
        .update_player_inventory(
            character.position.map_id,
            character.guid,
            session.inventory.items.clone(),
        )
        .await;
    let mut blocks = Vec::new();
    for item in &session.inventory.items {
        let old = old_inventory.iter().find(|old| old.item == item.item);
        if old.is_none() {
            let owner = ObjectGuid::new(HighGuid::Player, 0, character.guid);
            let contained = item_contained_guid(owner, &session.inventory.items, item);
            let template =
                wow_db::get_item_template_query(deps.world_db_pool, item.item_template).await?;
            blocks.push(build_item_create_update_block(
                owner,
                contained,
                item,
                template.as_ref().and_then(|template| {
                    (template.container_slots > 0).then_some(template.container_slots)
                }),
            )?);
            blocks.extend(build_inventory_position_update_blocks(
                character.guid,
                &session.inventory.items,
                item.bag as u8,
                item.slot,
            )?);
        } else if old.is_some_and(|old| old.count != item.count) {
            blocks.push(build_item_stack_count_update_block(item.item, item.count)?);
        }
    }
    if let Some(removed_item) = removed_item {
        if let Some(old) = old_inventory.iter().find(|old| old.item == removed_item) {
            blocks.extend(build_inventory_position_update_blocks(
                character.guid,
                &session.inventory.items,
                old.bag as u8,
                old.slot,
            )?);
            if old.bag != INVENTORY_SLOT_BAG_0 as u32 {
                send_packet(
                    stream,
                    SMSG_DESTROY_OBJECT,
                    &build_destroy_object_body(removed_item),
                    Some(&mut *header_crypto),
                )
                .await?;
            }
        }
    }
    if !blocks.is_empty() {
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &build_update_object_body(&blocks),
            Some(header_crypto),
        )
        .await?;
    }
    Ok(())
}

pub(in crate::world) async fn send_mail_result(
    stream: &mut WorldPacketSink,
    mail_id: u32,
    action: u32,
    error: u32,
    equip_error: Option<u32>,
    taken_item: Option<(u32, u32)>,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let body = SmsgSendMailResultResponse {
        mail_id,
        action,
        error,
        equip_error,
        taken_item,
    }
    .body();
    send_packet(stream, SMSG_SEND_MAIL_RESULT, &body, Some(header_crypto)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn send_mail_request(money: u32, cod: u32) -> wow_proto::SendMailRequest {
        wow_proto::SendMailRequest {
            mailbox_raw_guid: 0,
            receiver: "Rustone".to_string(),
            subject: "coin".to_string(),
            body: String::new(),
            stationery: MAIL_STATIONERY_DEFAULT as u32,
            unknown1: 0,
            item_raw_guid: 0,
            money,
            cod,
            unknown2: 0,
            unknown3: 0,
        }
    }

    #[test]
    fn money_only_player_mail_delivers_immediately() {
        assert_eq!(
            send_mail_delivery_delay_secs(&send_mail_request(123, 0), None),
            0
        );
    }

    #[test]
    fn non_money_player_mail_keeps_default_delivery_delay() {
        assert_eq!(
            send_mail_delivery_delay_secs(&send_mail_request(0, 0), None),
            MAIL_DELIVERY_DELAY_SECS
        );
        assert_eq!(
            send_mail_delivery_delay_secs(&send_mail_request(123, 0), Some(99)),
            MAIL_DELIVERY_DELAY_SECS
        );
        assert_eq!(
            send_mail_delivery_delay_secs(&send_mail_request(0, 123), Some(99)),
            MAIL_DELIVERY_DELAY_SECS
        );
    }
}
