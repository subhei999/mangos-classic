use super::*;
use wow_proto::world::WorldOpcode;

pub(in crate::world) async fn dispatch_quest_packet(
    ctx: &mut WorldPacketDispatchContext<'_>,
    packet: &packets::ParsedWorldClientPacket,
) -> anyhow::Result<()> {
    match packet {
        packets::ParsedWorldClientPacket::QuestQuery(_) => {
            handle_quest_query(
                &mut *ctx.stream,
                &ctx.runtime_state.object_mgr,
                ctx.world_db_pool,
                packet.quest_query()?,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::QuestgiverStatusQuery(_) => {
            handle_questgiver_status_query(
                &mut *ctx.stream,
                &ctx.runtime_state.object_mgr,
                ctx.world_db_pool,
                packet.questgiver_status_query()?,
                &*ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::QuestgiverHello(_) => {
            handle_questgiver_hello(
                &mut *ctx.stream,
                &ctx.runtime_state.object_mgr,
                ctx.world_db_pool,
                packet.questgiver_hello()?,
                &*ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::QuestgiverQueryQuest(_) => {
            handle_questgiver_query_quest(
                &mut *ctx.stream,
                &ctx.runtime_state.object_mgr,
                ctx.world_db_pool,
                packet.questgiver_quest()?,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::QuestgiverAcceptQuest(_) => {
            handle_questgiver_accept_quest(
                &mut *ctx.stream,
                QuestMutationDeps {
                    character_db_pool: ctx.character_db_pool,
                    object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                    world_db_pool: ctx.world_db_pool,
                    shared_world: SharedWorldDeps {
                        object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                        maps: &ctx.runtime_state.maps,
                        sessions: &ctx.runtime_state.sessions,
                    },
                },
                packet.questgiver_quest()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::QuestgiverCompleteQuest(_) => {
            handle_questgiver_complete_quest(
                &mut *ctx.stream,
                &ctx.runtime_state.object_mgr,
                ctx.world_db_pool,
                packet.questgiver_quest()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::QuestgiverRequestReward(_) => {
            handle_questgiver_request_reward(
                &mut *ctx.stream,
                ctx.character_db_pool,
                &ctx.runtime_state.object_mgr,
                ctx.world_db_pool,
                packet.questgiver_quest()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::QuestReward(_) => {
            handle_questgiver_choose_reward(
                &mut *ctx.stream,
                QuestMutationDeps {
                    character_db_pool: ctx.character_db_pool,
                    object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                    world_db_pool: ctx.world_db_pool,
                    shared_world: SharedWorldDeps {
                        object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                        maps: &ctx.runtime_state.maps,
                        sessions: &ctx.runtime_state.sessions,
                    },
                },
                packet.quest_reward()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::QuestgiverCancel(_) => {
            handle_questgiver_cancel(&mut *ctx.stream, &mut *ctx.header_crypto).await
        }
        packets::ParsedWorldClientPacket::QuestLogRemoveQuest(_) => {
            handle_questlog_remove_quest(
                &mut *ctx.stream,
                ctx.character_db_pool,
                ctx.runtime_state.object_mgr.as_ref(),
                ctx.world_db_pool,
                &ctx.runtime_state.maps,
                packet.questlog_remove_quest()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        other => anyhow::bail!("quest router received opcode 0x{:04X}", other.opcode()),
    }
}

pub(in crate::world) async fn handle_quest_query(
    stream: &mut WorldPacketSink,
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    request: wow_proto::QuestQueryRequest,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let quest_id = request.quest_id;
    let Some(quest) = object_mgr.quest_template(world_db_pool, quest_id).await? else {
        warn!(quest_id, "Ignoring query for unknown quest");
        return Ok(());
    };
    let response = build_quest_query_response_body(&quest);
    send_packet(
        stream,
        WorldOpcode::SmsgQuestQueryResponse as u16,
        &response,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn handle_questgiver_status_query(
    stream: &mut WorldPacketSink,
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    request: wow_proto::QuestgiverStatusQueryRequest,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let guid = ObjectGuid::from_raw(request.raw_guid);
    let status = questgiver_dialog_status(object_mgr, world_db_pool, guid, session).await?;
    send_packet(
        stream,
        WorldOpcode::SmsgQuestgiverStatus as u16,
        &build_questgiver_status_body(guid, status),
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn handle_questgiver_hello(
    stream: &mut WorldPacketSink,
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    request: wow_proto::QuestgiverHelloRequest,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let guid = ObjectGuid::from_raw(request.raw_guid);
    if let Some(quest) =
        questgiver_completed_turnin_quest(object_mgr, world_db_pool, guid, session).await?
    {
        return send_questgiver_completion_response(
            stream,
            world_db_pool,
            guid,
            &quest,
            true,
            true,
            header_crypto,
        )
        .await;
    }

    let quests = questgiver_visible_quests(object_mgr, world_db_pool, guid, session).await?;
    if quests.is_empty() {
        warn!(
            guid = format_args!("0x{:016X}", guid.raw()),
            "Ignoring questgiver hello without visible quests"
        );
        return Ok(());
    }
    let response = build_questgiver_quest_list_body(guid, &quests);
    send_packet(
        stream,
        WorldOpcode::SmsgQuestgiverQuestList as u16,
        &response,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn handle_questgiver_query_quest(
    stream: &mut WorldPacketSink,
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    request: wow_proto::QuestgiverQuestRequest,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let request = QuestgiverQuestRequest::from(request);
    if !questgiver_starts_quest(object_mgr, world_db_pool, request.guid, request.quest).await?
        && !questgiver_completes_quest(object_mgr, world_db_pool, request.guid, request.quest)
            .await?
    {
        warn!(
            quest = request.quest,
            "Ignoring quest details request for invalid giver"
        );
        return Ok(());
    }
    let Some(quest) = object_mgr
        .quest_template(world_db_pool, request.quest)
        .await?
    else {
        return Ok(());
    };
    let displays = quest_reward_item_displays(world_db_pool, &quest).await?;
    let response = build_quest_details_body(request.guid, &quest, &displays);
    send_packet(
        stream,
        WorldOpcode::SmsgQuestgiverQuestDetails as u16,
        &response,
        Some(header_crypto),
    )
    .await
}

#[derive(Clone, Copy)]
pub(in crate::world) struct QuestMutationDeps<'a> {
    pub(in crate::world) character_db_pool: &'a MySqlPool,
    pub(in crate::world) object_mgr: &'a ObjectMgr,
    pub(in crate::world) world_db_pool: &'a MySqlPool,
    pub(in crate::world) shared_world: SharedWorldDeps<'a>,
}

pub(in crate::world) struct QuestSourceItemGrant {
    pub(in crate::world) item: CharacterInventoryItem,
    pub(in crate::world) count: u32,
    pub(in crate::world) created: bool,
    pub(in crate::world) container_slots: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct QuestSourceItemTemplate {
    pub(in crate::world) max_durability: u32,
    pub(in crate::world) max_stack: u32,
    pub(in crate::world) container_slots: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::world) enum QuestSourceItemDestination {
    ExistingStack {
        item_guid: u32,
        new_count: u32,
        grant_count: u32,
    },
    NewStack {
        slot: u8,
        count: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::world) struct QuestSourceItemStoragePlan {
    pub(in crate::world) item_id: u32,
    pub(in crate::world) max_durability: u32,
    pub(in crate::world) container_slots: Option<u32>,
    pub(in crate::world) destinations: Vec<QuestSourceItemDestination>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::world) enum QuestSourceItemStorage {
    NoGrantNeeded,
    NoSpace,
    Grant(QuestSourceItemStoragePlan),
}

pub(in crate::world) async fn handle_questgiver_accept_quest(
    stream: &mut WorldPacketSink,
    deps: QuestMutationDeps<'_>,
    request: wow_proto::QuestgiverQuestRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let character_db_pool = deps.character_db_pool;
    let object_mgr = deps.object_mgr;
    let world_db_pool = deps.world_db_pool;
    let Some(character_guid) = session
        .character
        .active_character
        .as_ref()
        .map(|character| character.guid)
    else {
        warn!("Ignoring quest accept before character login");
        return Ok(());
    };
    let request = QuestgiverQuestRequest::from(request);
    if !questgiver_starts_quest(object_mgr, world_db_pool, request.guid, request.quest).await? {
        warn!(
            quest = request.quest,
            "Ignoring quest accept for invalid giver"
        );
        return Ok(());
    }
    let Some(quest) = object_mgr
        .quest_template(world_db_pool, request.quest)
        .await?
    else {
        return Ok(());
    };
    if !can_take_start_quest(object_mgr, world_db_pool, &quest, session).await? {
        warn!(
            quest = request.quest,
            "Ignoring quest accept that does not satisfy CMaNGOS-style eligibility"
        );
        return Ok(());
    }
    if !quest_log_has_free_slot(session) {
        send_packet(
            stream,
            WorldOpcode::SmsgQuestlogFull as u16,
            &[],
            Some(&mut *header_crypto),
        )
        .await?;
        return Ok(());
    }
    let source_item_storage =
        quest_source_item_storage_plan(world_db_pool, &quest, &session.inventory.items).await?;
    if matches!(source_item_storage, QuestSourceItemStorage::NoSpace) {
        send_inventory_change_failure(
            stream,
            EQUIP_ERR_COULDNT_SPLIT_ITEMS,
            None,
            None,
            header_crypto,
        )
        .await?;
        return Ok(());
    }
    let Some(slot) = assign_quest_log_slot(session, request.quest) else {
        warn!(
            quest = request.quest,
            "Quest log slot disappeared before accept"
        );
        return Ok(());
    };
    let mut status =
        wow_db::accept_character_quest(character_db_pool, character_guid, request.quest).await?;
    session
        .quests
        .quest_statuses
        .insert(request.quest, status.clone());
    let source_item = grant_quest_source_item_if_needed(
        character_db_pool,
        world_db_pool,
        character_guid,
        source_item_storage,
        session,
    )
    .await?;
    if quest_can_complete_from_inventory(&quest, &session.inventory.items) {
        status = wow_db::complete_character_quest(character_db_pool, character_guid, request.quest)
            .await?;
        session
            .quests
            .quest_statuses
            .insert(request.quest, status.clone());
    }
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &build_player_quest_log_update_body(character_guid, slot, &status)?,
        Some(&mut *header_crypto),
    )
    .await?;
    if !source_item.is_empty() {
        let owner_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
        let mut update_blocks = Vec::new();
        let mut created_slots = Vec::new();
        for grant in &source_item {
            if grant.created {
                update_blocks.push(build_item_create_update_block(
                    owner_guid,
                    owner_guid,
                    &grant.item,
                    grant.container_slots,
                )?);
                created_slots.push(grant.item.slot);
            } else {
                update_blocks.push(build_item_stack_count_update_block(
                    grant.item.item,
                    grant.item.count,
                )?);
            }
        }
        if !created_slots.is_empty() {
            update_blocks.push(build_inventory_slots_update_block(
                character_guid,
                &session.inventory.items,
                &created_slots,
            )?);
        }
        let create_body = build_update_object_body(&update_blocks);
        send_packet(
            stream,
            WorldOpcode::SmsgUpdateObject as u16,
            &create_body,
            Some(&mut *header_crypto),
        )
        .await?;
        for grant in &source_item {
            let push_body = build_item_push_result_body(
                character_guid,
                &grant.item,
                grant.count,
                true,
                false,
                true,
            );
            send_packet(
                stream,
                WorldOpcode::SmsgItemPushResult as u16,
                &push_body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
    }
    if status.status == QUEST_STATUS_COMPLETE {
        send_packet(
            stream,
            WorldOpcode::SmsgQuestUpdateComplete as u16,
            &request.quest.to_le_bytes(),
            Some(&mut *header_crypto),
        )
        .await?;
    }
    send_visible_questgiver_status_updates(
        stream,
        object_mgr,
        world_db_pool,
        deps.shared_world,
        session,
        &[request.guid],
        header_crypto,
    )
    .await?;
    close_questgiver_gossip(stream, header_crypto).await?;
    Ok(())
}

pub(in crate::world) async fn handle_questgiver_cancel(
    stream: &mut WorldPacketSink,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    close_questgiver_gossip(stream, header_crypto).await
}

pub(in crate::world) async fn close_questgiver_gossip(
    stream: &mut WorldPacketSink,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    send_packet(
        stream,
        WorldOpcode::SmsgGossipComplete as u16,
        &[],
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn handle_questgiver_complete_quest(
    stream: &mut WorldPacketSink,
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    request: wow_proto::QuestgiverQuestRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if session.character.active_character.is_none() {
        warn!("Ignoring quest completion before character login");
        return Ok(());
    };
    let request = QuestgiverQuestRequest::from(request);
    if !questgiver_completes_quest(object_mgr, world_db_pool, request.guid, request.quest).await? {
        return Ok(());
    }
    let Some(status) = session.quests.quest_statuses.get(&request.quest).cloned() else {
        return Ok(());
    };
    let Some(quest) = object_mgr
        .quest_template(world_db_pool, request.quest)
        .await?
    else {
        return Ok(());
    };
    let complete = if status.status == QUEST_STATUS_COMPLETE {
        quest_status_can_reward_from_inventory(&status, &quest, &session.inventory.items)
    } else {
        quest_status_can_complete(&status, &quest, &session.inventory.items)
    };
    send_questgiver_completion_response(
        stream,
        world_db_pool,
        request.guid,
        &quest,
        complete,
        false,
        header_crypto,
    )
    .await
}

pub(in crate::world) async fn handle_questgiver_request_reward(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    request: wow_proto::QuestgiverQuestRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character_guid) = session
        .character
        .active_character
        .as_ref()
        .map(|character| character.guid)
    else {
        warn!("Ignoring quest reward request before character login");
        return Ok(());
    };
    let request = QuestgiverQuestRequest::from(request);
    if !questgiver_completes_quest(object_mgr, world_db_pool, request.guid, request.quest).await? {
        return Ok(());
    }
    let Some(status) = session.quests.quest_statuses.get(&request.quest).cloned() else {
        return Ok(());
    };
    let Some(quest) = object_mgr
        .quest_template(world_db_pool, request.quest)
        .await?
    else {
        return Ok(());
    };

    let mut updated_status = status;
    if updated_status.status != QUEST_STATUS_COMPLETE {
        if !quest_status_can_complete(&updated_status, &quest, &session.inventory.items) {
            return Ok(());
        }
        updated_status =
            wow_db::complete_character_quest(character_db_pool, character_guid, request.quest)
                .await?;
        session
            .quests
            .quest_statuses
            .insert(request.quest, updated_status.clone());
        if let Some(slot) = quest_log_slot_for_quest(session, request.quest) {
            send_packet(
                stream,
                WorldOpcode::SmsgUpdateObject as u16,
                &build_player_quest_log_update_body(character_guid, slot, &updated_status)?,
                Some(&mut *header_crypto),
            )
            .await?;
        }
        send_packet(
            stream,
            WorldOpcode::SmsgQuestUpdateComplete as u16,
            &request.quest.to_le_bytes(),
            Some(&mut *header_crypto),
        )
        .await?;
    }

    if !quest_status_can_reward_from_inventory(&updated_status, &quest, &session.inventory.items) {
        return Ok(());
    }

    let displays = quest_reward_item_displays(world_db_pool, &quest).await?;
    send_packet(
        stream,
        WorldOpcode::SmsgQuestgiverOfferReward as u16,
        &build_quest_offer_reward_body(request.guid, &quest, &displays),
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn send_questgiver_completion_response(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    guid: ObjectGuid,
    quest: &QuestTemplateQuery,
    complete: bool,
    close_on_cancel: bool,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let displays = quest_reward_item_displays(world_db_pool, quest).await?;
    if quest_request_items_skips_to_offer_reward(quest, complete) {
        send_packet(
            stream,
            WorldOpcode::SmsgQuestgiverOfferReward as u16,
            &build_quest_offer_reward_body(guid, quest, &displays),
            Some(header_crypto),
        )
        .await
    } else {
        send_packet(
            stream,
            WorldOpcode::SmsgQuestgiverRequestItems as u16,
            &build_quest_request_items_body(guid, quest, &displays, complete, close_on_cancel),
            Some(header_crypto),
        )
        .await
    }
}

pub(in crate::world) async fn handle_questgiver_choose_reward(
    stream: &mut WorldPacketSink,
    deps: QuestMutationDeps<'_>,
    request: wow_proto::QuestRewardRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let character_db_pool = deps.character_db_pool;
    let object_mgr = deps.object_mgr;
    let world_db_pool = deps.world_db_pool;
    let Some(character) = &session.character.active_character else {
        warn!("Ignoring quest reward before character login");
        return Ok(());
    };
    let character_guid = character.guid;
    let character_level = character.level;
    let request = QuestRewardRequest::from(request);
    if request.reward >= 6 {
        warn!(
            quest = request.quest,
            reward = request.reward,
            "Ignoring invalid quest reward choice"
        );
        return Ok(());
    }
    if !questgiver_completes_quest(object_mgr, world_db_pool, request.guid, request.quest).await? {
        warn!(
            quest = request.quest,
            "Ignoring reward request for invalid giver"
        );
        return Ok(());
    }
    let Some(quest) = object_mgr
        .quest_template(world_db_pool, request.quest)
        .await?
    else {
        return Ok(());
    };
    let reward_money = quest.rew_or_req_money.max(0) as u32;
    let reward_xp = quest_xp_reward(character_level, &quest);
    let reputation_rewards = quest_reputation_rewards_with_bonus(
        character_level,
        &quest,
        reputation_gain_percent_from_active_auras(&session.auras.active_auras),
    );
    let slot = quest_log_slot_for_quest(session, request.quest);
    let Some(reward_items) = selected_quest_reward_items(&quest, request.reward) else {
        warn!(
            quest = request.quest,
            reward = request.reward,
            "Ignoring invalid quest reward item choice"
        );
        return Ok(());
    };
    let reward_grants = load_quest_reward_grants(world_db_pool, &reward_items).await?;
    let Some(current_status) = session.quests.quest_statuses.get(&request.quest).cloned() else {
        return Ok(());
    };
    if !quest_status_can_reward_from_inventory(&current_status, &quest, &session.inventory.items) {
        if current_status.status == QUEST_STATUS_COMPLETE {
            let updated = wow_db::incomplete_character_quest(
                character_db_pool,
                character_guid,
                request.quest,
            )
            .await?;
            session
                .quests
                .quest_statuses
                .insert(request.quest, updated.clone());
            if let Some(slot) = slot {
                send_packet(
                    stream,
                    WorldOpcode::SmsgUpdateObject as u16,
                    &build_player_quest_log_update_body(character_guid, slot, &updated)?,
                    Some(&mut *header_crypto),
                )
                .await?;
            }
        }
        send_inventory_change_failure(stream, EQUIP_ERR_ITEM_NOT_FOUND, None, None, header_crypto)
            .await?;
        return Ok(());
    }
    if current_status.status != QUEST_STATUS_COMPLETE {
        let updated =
            wow_db::complete_character_quest(character_db_pool, character_guid, request.quest)
                .await?;
        session
            .quests
            .quest_statuses
            .insert(request.quest, updated.clone());
        if let Some(slot) = slot {
            send_packet(
                stream,
                WorldOpcode::SmsgUpdateObject as u16,
                &build_player_quest_log_update_body(character_guid, slot, &updated)?,
                Some(&mut *header_crypto),
            )
            .await?;
        }
    }

    let required_item_slots = quest_required_item_inventory_slots(&quest, &session.inventory.items);
    let equipped_bags = load_equipped_bag_infos(world_db_pool, &session.inventory.items).await?;
    let Some(reward_storage_plans) = plan_quest_reward_storage(
        &session.inventory.items,
        &reward_grants,
        &equipped_bags,
        &required_item_slots,
    ) else {
        send_inventory_change_failure(
            stream,
            EQUIP_ERR_COULDNT_SPLIT_ITEMS,
            None,
            None,
            header_crypto,
        )
        .await?;
        return Ok(());
    };
    consume_quest_required_items(
        stream,
        character_db_pool,
        character_guid,
        &quest,
        session,
        header_crypto,
    )
    .await?;
    let reward_update_blocks = grant_quest_reward_items(
        character_db_pool,
        world_db_pool,
        character_guid,
        &reward_grants,
        &reward_storage_plans,
        session,
    )
    .await?;
    let Some(reward_result) = wow_db::reward_character_quest(
        character_db_pool,
        character_guid,
        request.quest,
        reward_money,
        &reputation_rewards,
    )
    .await?
    else {
        return Ok(());
    };
    if let Some(status) = session.quests.quest_statuses.get_mut(&request.quest) {
        status.status = QUEST_STATUS_COMPLETE;
        status.rewarded = 1;
    }
    for change in &reward_result.reputations {
        if let Some(existing) = session
            .character
            .character_reputations
            .iter_mut()
            .find(|reputation| reputation.faction == change.reputation.faction)
        {
            *existing = change.reputation.clone();
        } else {
            session
                .character
                .character_reputations
                .push(change.reputation.clone());
        }
    }
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &build_player_money_update_body(character_guid, reward_result.money)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_quest_reputation_updates(stream, &reward_result.reputations, header_crypto).await?;
    if !reward_update_blocks.is_empty() {
        let body = build_update_object_body(&reward_update_blocks);
        send_packet(
            stream,
            WorldOpcode::SmsgUpdateObject as u16,
            &body,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    award_character_xp(
        stream,
        character_db_pool,
        world_db_pool,
        deps.shared_world.maps,
        session,
        None,
        reward_xp,
        header_crypto,
    )
    .await?;
    if let Some(slot) = slot {
        clear_quest_log_slot(session, request.quest);
        send_packet(
            stream,
            WorldOpcode::SmsgUpdateObject as u16,
            &build_player_quest_log_clear_body(character_guid, slot)?,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    send_visible_questgiver_status_updates(
        stream,
        object_mgr,
        world_db_pool,
        deps.shared_world,
        session,
        &[request.guid],
        &mut *header_crypto,
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgQuestgiverQuestComplete as u16,
        &build_questgiver_quest_complete_body_with_xp(&quest, reward_xp, reward_money),
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn send_quest_reputation_updates(
    stream: &mut WorldPacketSink,
    reputations: &[CharacterReputationChange],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let changed_reputations = reputations
        .iter()
        .map(|change| change.reputation.clone())
        .collect::<Vec<_>>();
    for reputation in &changed_reputations {
        if (reputation.flags & FACTION_FLAG_VISIBLE) == 0 {
            continue;
        }
        if let Some(body) = build_set_faction_visible_body(reputation.faction) {
            send_packet(
                stream,
                WorldOpcode::SmsgSetFactionVisible as u16,
                &body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
    }

    let body = build_set_faction_standing_body(&changed_reputations);
    if body.len() > 4 {
        send_packet(
            stream,
            WorldOpcode::SmsgSetFactionStanding as u16,
            &body,
            Some(header_crypto),
        )
        .await?;
    }
    for change in reputations {
        if let Some(message) = reputation_gain_system_message(change) {
            let body = build_system_message_chat_body(&message);
            send_packet(
                stream,
                WorldOpcode::SmsgMessageChat as u16,
                &body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn handle_questlog_remove_quest(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    maps: &Arc<MapRuntimeManager>,
    request: wow_proto::QuestLogRemoveQuestRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character_guid) = session
        .character
        .active_character
        .as_ref()
        .map(|character| character.guid)
    else {
        warn!("Ignoring quest abandon before character login");
        return Ok(());
    };
    let slot = usize::from(request.slot);
    if slot >= MAX_QUEST_LOG_SIZE {
        return Ok(());
    }
    let quest = session.quests.quest_log_slots[slot];
    if quest == 0 {
        return Ok(());
    }
    let Some(status) = session.quests.quest_statuses.get(&quest).cloned() else {
        return Ok(());
    };
    if wow_db::abandon_character_quest(character_db_pool, character_guid, status.quest)
        .await?
        .is_some()
    {
        if let Some(local_status) = session.quests.quest_statuses.get_mut(&status.quest) {
            local_status.status = 0;
            local_status.rewarded = 0;
            local_status.mobcount1 = 0;
            local_status.mobcount2 = 0;
            local_status.mobcount3 = 0;
            local_status.mobcount4 = 0;
        }
        session.quests.quest_log_slots[slot] = 0;
        send_packet(
            stream,
            WorldOpcode::SmsgUpdateObject as u16,
            &build_player_quest_log_clear_body(character_guid, slot)?,
            Some(&mut *header_crypto),
        )
        .await?;
        send_visible_quest_gameobject_dynamic_updates(
            stream,
            object_mgr,
            world_db_pool,
            maps,
            session,
            header_crypto,
        )
        .await?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct QuestgiverQuestRequest {
    pub(in crate::world) guid: ObjectGuid,
    pub(in crate::world) quest: u32,
}

impl From<wow_proto::QuestgiverQuestRequest> for QuestgiverQuestRequest {
    fn from(request: wow_proto::QuestgiverQuestRequest) -> Self {
        Self {
            guid: ObjectGuid::from_raw(request.raw_guid),
            quest: request.quest,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct QuestRewardRequest {
    pub(in crate::world) guid: ObjectGuid,
    pub(in crate::world) quest: u32,
    pub(in crate::world) reward: u32,
}

impl From<wow_proto::QuestRewardRequest> for QuestRewardRequest {
    fn from(request: wow_proto::QuestRewardRequest) -> Self {
        Self {
            guid: ObjectGuid::from_raw(request.raw_guid),
            quest: request.quest,
            reward: request.reward,
        }
    }
}

pub(in crate::world) async fn questgiver_dialog_status(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    guid: ObjectGuid,
    session: &WorldSessionState,
) -> anyhow::Result<u32> {
    if !guid.is_creature() && !guid.is_game_object() {
        return Ok(DIALOG_STATUS_NONE);
    }

    let mut dialog_status = DIALOG_STATUS_NONE;
    for quest in questgiver_complete_quests(object_mgr, world_db_pool, guid).await? {
        let Some(status) = session.quests.quest_statuses.get(&quest.entry) else {
            continue;
        };
        if status.rewarded != 0 {
            continue;
        }
        if quest_status_can_reward_from_inventory(status, &quest, &session.inventory.items) {
            dialog_status = dialog_status.max(DIALOG_STATUS_REWARD2);
        } else if status.status == QUEST_STATUS_INCOMPLETE {
            dialog_status = dialog_status.max(DIALOG_STATUS_INCOMPLETE);
        }
    }
    for quest in questgiver_start_quests(object_mgr, world_db_pool, guid).await? {
        if let Some(start_status) =
            quest_start_dialog_status(object_mgr, world_db_pool, &quest, session).await?
        {
            dialog_status = dialog_status.max(start_status);
        }
    }

    for status in session.quests.quest_statuses.values() {
        if status.rewarded == 0
            && questgiver_completes_quest(object_mgr, world_db_pool, guid, status.quest).await?
        {
            if let Some(quest) = object_mgr
                .quest_template(world_db_pool, status.quest)
                .await?
            {
                if quest_status_can_reward_from_inventory(status, &quest, &session.inventory.items)
                {
                    dialog_status = dialog_status.max(DIALOG_STATUS_REWARD2);
                }
            }
        }
    }
    Ok(dialog_status)
}

pub(in crate::world) async fn send_visible_questgiver_status_updates(
    stream: &mut WorldPacketSink,
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    shared_world: SharedWorldDeps<'_>,
    session: &WorldSessionState,
    extra_guids: &[ObjectGuid],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };

    let mut seen = HashSet::new();
    let mut guids = Vec::new();
    for guid in extra_guids {
        if !guid.is_creature() && !guid.is_game_object() {
            continue;
        }
        if seen.insert(guid.raw()) {
            guids.push(*guid);
        }
    }

    let visible_creature_guids = shared_world
        .maps
        .player_visible_db_creature_guids(character.position.map_id, character.guid)
        .await;
    let visible_creatures = shared_world
        .maps
        .db_creature_snapshots(character.position.map_id, &visible_creature_guids)
        .await;
    for creature in visible_creatures {
        let guid = creature.guid();
        if seen.insert(guid.raw()) {
            guids.push(guid);
        }
    }

    for guid in guids {
        if !questgiver_has_quest_relation(object_mgr, world_db_pool, guid).await? {
            continue;
        }
        let status = questgiver_dialog_status(object_mgr, world_db_pool, guid, session).await?;
        send_packet(
            stream,
            WorldOpcode::SmsgQuestgiverStatus as u16,
            &build_questgiver_status_body(guid, status),
            Some(&mut *header_crypto),
        )
        .await?;
    }
    send_visible_quest_gameobject_dynamic_updates(
        stream,
        object_mgr,
        world_db_pool,
        shared_world.maps,
        session,
        header_crypto,
    )
    .await?;
    Ok(())
}

pub(in crate::world) async fn send_visible_quest_gameobject_dynamic_updates(
    stream: &mut WorldPacketSink,
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    maps: &Arc<MapRuntimeManager>,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let visible_gameobject_guids = maps
        .player_visible_db_gameobject_guids(character.position.map_id, character.guid)
        .await;
    if visible_gameobject_guids.is_empty() {
        return Ok(());
    }

    let visible_gameobjects = maps
        .db_gameobject_snapshots(character.position.map_id, &visible_gameobject_guids)
        .await;
    let mut update_blocks = Vec::new();
    for gameobject in visible_gameobjects {
        if !quest_gameobject_needs_dynamic_refresh(object_mgr, world_db_pool, &gameobject).await? {
            continue;
        }
        let dynamic_flags =
            gameobject_dynamic_flags_for_player(object_mgr, world_db_pool, session, &gameobject)
                .await?;
        update_blocks.push(build_db_gameobject_dynamic_flags_update_block_with_flags(
            &gameobject,
            dynamic_flags,
        )?);
    }

    if update_blocks.is_empty() {
        return Ok(());
    }
    for chunk in update_blocks.chunks(CREATURE_UPDATE_CHUNK_SIZE) {
        send_packet(
            stream,
            WorldOpcode::SmsgUpdateObject as u16,
            &build_update_object_body(chunk),
            Some(&mut *header_crypto),
        )
        .await?;
    }
    Ok(())
}

pub(in crate::world) async fn gameobject_dynamic_flags_for_player(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    session: &WorldSessionState,
    gameobject: &DbGameObjectRuntime,
) -> anyhow::Result<u32> {
    let dynamic_flags =
        gameobject_dynamic_flags_for_quest_statuses(gameobject, &session.quests.quest_statuses);
    if dynamic_flags == 0 {
        return Ok(0);
    }
    if gameobject_chest_loot_is_exclusively_quest_drops(
        object_mgr,
        world_db_pool,
        &gameobject.spawn.template,
    )
    .await?
    {
        let loot_items = select_db_gameobject_loot_item_for_character(
            object_mgr,
            world_db_pool,
            session,
            &gameobject.spawn.template,
        )
        .await?;
        if loot_items.is_empty() {
            return Ok(0);
        }
    }
    Ok(dynamic_flags)
}

pub(in crate::world) async fn quest_gameobject_needs_dynamic_refresh(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    gameobject: &DbGameObjectRuntime,
) -> anyhow::Result<bool> {
    let template = &gameobject.spawn.template;
    if gameobject_required_active_quest(template).is_some()
        || gameobject_chest_has_loot_id(template)
    {
        return Ok(true);
    }
    if template.object_type == GO_TYPE_QUESTGIVER {
        return questgiver_has_quest_relation(object_mgr, world_db_pool, gameobject.guid()).await;
    }
    Ok(template.flags & GO_FLAG_INTERACT_COND != 0
        && matches!(
            template.object_type,
            GO_TYPE_GENERIC | GO_TYPE_SPELL_FOCUS | GO_TYPE_GOOBER
        ))
}

pub(in crate::world) async fn questgiver_has_quest_relation(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    guid: ObjectGuid,
) -> anyhow::Result<bool> {
    if !guid.is_creature() && !guid.is_game_object() {
        return Ok(false);
    }
    Ok(!questgiver_start_quests(object_mgr, world_db_pool, guid)
        .await?
        .is_empty()
        || !questgiver_complete_quests(object_mgr, world_db_pool, guid)
            .await?
            .is_empty())
}

pub(in crate::world) async fn questgiver_visible_quests(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    guid: ObjectGuid,
    session: &WorldSessionState,
) -> anyhow::Result<Vec<QuestListItem>> {
    if !guid.is_creature() && !guid.is_game_object() {
        return Ok(Vec::new());
    }
    let mut visible = Vec::new();
    let mut seen = HashSet::new();
    for quest in questgiver_complete_quests(object_mgr, world_db_pool, guid).await? {
        let Some(status) = session.quests.quest_statuses.get(&quest.entry) else {
            continue;
        };
        if status.rewarded != 0 {
            continue;
        }
        let dialog_status =
            if quest_status_can_reward_from_inventory(status, &quest, &session.inventory.items) {
                DIALOG_STATUS_REWARD2
            } else if status.status == QUEST_STATUS_INCOMPLETE {
                DIALOG_STATUS_INCOMPLETE
            } else {
                continue;
            };
        seen.insert(quest.entry);
        visible.push(QuestListItem {
            quest,
            dialog_status,
        });
    }
    let quests = questgiver_start_quests(object_mgr, world_db_pool, guid).await?;
    for quest in quests {
        if seen.contains(&quest.entry) {
            continue;
        }
        if can_take_start_quest(object_mgr, world_db_pool, &quest, session).await? {
            visible.push(QuestListItem {
                quest,
                dialog_status: DIALOG_STATUS_AVAILABLE,
            });
        }
    }
    Ok(visible)
}

pub(in crate::world) async fn questgiver_completed_turnin_quest(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    guid: ObjectGuid,
    session: &WorldSessionState,
) -> anyhow::Result<Option<QuestTemplateQuery>> {
    if !guid.is_creature() && !guid.is_game_object() {
        return Ok(None);
    }

    for status in active_quest_statuses_sorted(&session.quests.quest_statuses) {
        if questgiver_completes_quest(object_mgr, world_db_pool, guid, status.quest).await? {
            let Some(quest) = object_mgr
                .quest_template(world_db_pool, status.quest)
                .await?
            else {
                continue;
            };
            if quest_status_can_reward_from_inventory(status, &quest, &session.inventory.items) {
                return Ok(Some(quest));
            }
        }
    }

    Ok(None)
}

pub(in crate::world) async fn questgiver_start_quests(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    guid: ObjectGuid,
) -> anyhow::Result<Vec<QuestTemplateQuery>> {
    if guid.is_creature() {
        object_mgr
            .creature_start_quests(world_db_pool, guid.entry())
            .await
    } else if guid.is_game_object() {
        object_mgr
            .gameobject_start_quests(world_db_pool, guid.entry())
            .await
    } else {
        Ok(Vec::new())
    }
}

pub(in crate::world) async fn questgiver_complete_quests(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    guid: ObjectGuid,
) -> anyhow::Result<Vec<QuestTemplateQuery>> {
    if guid.is_creature() {
        object_mgr
            .creature_complete_quests(world_db_pool, guid.entry())
            .await
    } else if guid.is_game_object() {
        object_mgr
            .gameobject_complete_quests(world_db_pool, guid.entry())
            .await
    } else {
        Ok(Vec::new())
    }
}

pub(in crate::world) async fn questgiver_starts_quest(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    guid: ObjectGuid,
    quest: u32,
) -> anyhow::Result<bool> {
    if guid.is_creature() {
        object_mgr
            .creature_starts_quest(world_db_pool, guid.entry(), quest)
            .await
    } else if guid.is_game_object() {
        object_mgr
            .gameobject_starts_quest(world_db_pool, guid.entry(), quest)
            .await
    } else {
        Ok(false)
    }
}

pub(in crate::world) async fn questgiver_completes_quest(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    guid: ObjectGuid,
    quest: u32,
) -> anyhow::Result<bool> {
    if guid.is_creature() {
        object_mgr
            .creature_completes_quest(world_db_pool, guid.entry(), quest)
            .await
    } else if guid.is_game_object() {
        object_mgr
            .gameobject_completes_quest(world_db_pool, guid.entry(), quest)
            .await
    } else {
        Ok(false)
    }
}

pub(in crate::world) fn active_quest_statuses_sorted(
    quest_statuses: &HashMap<u32, CharacterQuestStatus>,
) -> Vec<&CharacterQuestStatus> {
    let mut statuses: Vec<_> = quest_statuses
        .values()
        .filter(|status| quest_status_is_current(status))
        .collect();
    statuses.sort_by_key(|status| status.quest);
    statuses
}

pub(in crate::world) async fn grant_quest_source_item_if_needed(
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    character_guid: u32,
    storage: QuestSourceItemStorage,
    session: &mut WorldSessionState,
) -> anyhow::Result<Vec<QuestSourceItemGrant>> {
    let QuestSourceItemStorage::Grant(plan) = storage else {
        return Ok(Vec::new());
    };
    let mut granted = Vec::new();
    for destination in plan.destinations {
        match destination {
            QuestSourceItemDestination::ExistingStack {
                item_guid,
                new_count,
                grant_count,
            } => {
                if !wow_db::update_character_inventory_item_count(
                    character_db_pool,
                    character_guid,
                    item_guid,
                    new_count,
                )
                .await?
                {
                    warn!(
                        item = item_guid,
                        "Cannot grant quest source item into missing existing stack"
                    );
                    continue;
                }
                granted.push((item_guid, grant_count, false));
            }
            QuestSourceItemDestination::NewStack { slot, count } => {
                let random_properties = generate_item_instance_random_properties(
                    world_db_pool,
                    &session.movement.db_creature_navigation.world_data_files,
                    plan.item_id,
                )
                .await?;
                let item = wow_db::add_character_inventory_item_with_random_properties(
                    character_db_pool,
                    wow_db::AddCharacterInventoryItemRequest {
                        guid: character_guid,
                        bag: INVENTORY_SLOT_BAG_0 as u32,
                        slot,
                        item_template: plan.item_id,
                        count,
                        durability: plan.max_durability,
                        random_properties: random_properties.as_ref(),
                    },
                )
                .await?;
                granted.push((item.item, count, true));
            }
        }
    }
    session.inventory.items =
        wow_db::get_character_inventory_items(character_db_pool, character_guid).await?;
    Ok(granted
        .into_iter()
        .filter_map(|(item_guid, count, created)| {
            session
                .inventory
                .items
                .iter()
                .find(|item| item.item == item_guid)
                .cloned()
                .map(|item| QuestSourceItemGrant {
                    item,
                    count,
                    created,
                    container_slots: created.then_some(plan.container_slots).flatten(),
                })
        })
        .collect())
}

pub(in crate::world) async fn quest_source_item_storage_plan(
    world_db_pool: &MySqlPool,
    quest: &QuestTemplateQuery,
    inventory: &[CharacterInventoryItem],
) -> anyhow::Result<QuestSourceItemStorage> {
    if quest.src_item_id == 0 {
        return Ok(QuestSourceItemStorage::NoGrantNeeded);
    }
    let required_count = quest.src_item_count.max(1);
    let current_count = inventory
        .iter()
        .filter(|item| item.item_template == quest.src_item_id)
        .map(|item| item.count)
        .sum::<u32>();
    if current_count >= required_count {
        return Ok(QuestSourceItemStorage::NoGrantNeeded);
    };
    let Some(template) = wow_db::get_item_template_query(world_db_pool, quest.src_item_id).await?
    else {
        warn!(
            quest = quest.entry,
            item = quest.src_item_id,
            "Cannot grant missing quest source item template"
        );
        return Ok(QuestSourceItemStorage::NoGrantNeeded);
    };
    let item_template = QuestSourceItemTemplate {
        max_durability: template.max_durability,
        max_stack: template.stackable.max(1),
        container_slots: (template.container_slots > 0).then_some(template.container_slots),
    };
    Ok(plan_quest_source_item_storage(
        quest,
        inventory,
        item_template,
    ))
}

pub(in crate::world) fn plan_quest_source_item_storage(
    quest: &QuestTemplateQuery,
    inventory: &[CharacterInventoryItem],
    item_template: QuestSourceItemTemplate,
) -> QuestSourceItemStorage {
    if quest.src_item_id == 0 {
        return QuestSourceItemStorage::NoGrantNeeded;
    }
    let required_count = quest.src_item_count.max(1);
    let current_count = inventory
        .iter()
        .filter(|item| item.item_template == quest.src_item_id)
        .map(|item| item.count)
        .sum::<u32>();
    if current_count >= required_count {
        return QuestSourceItemStorage::NoGrantNeeded;
    }

    let mut remaining = required_count - current_count;
    let max_stack = item_template.max_stack.max(1);
    let mut destinations = Vec::new();
    let mut stacks: Vec<_> = inventory
        .iter()
        .filter(|item| item.item_template == quest.src_item_id && item.count < max_stack)
        .collect();
    stacks.sort_by_key(|item| {
        let bag_order = if item.bag == INVENTORY_SLOT_BAG_0 as u32 {
            0
        } else {
            1
        };
        (bag_order, item.bag, item.slot)
    });
    for stack in stacks {
        if remaining == 0 {
            break;
        }
        let grant_count = remaining.min(max_stack - stack.count);
        remaining -= grant_count;
        destinations.push(QuestSourceItemDestination::ExistingStack {
            item_guid: stack.item,
            new_count: stack.count + grant_count,
            grant_count,
        });
    }

    for slot in empty_backpack_slots(inventory) {
        if remaining == 0 {
            break;
        }
        let count = remaining.min(max_stack);
        remaining -= count;
        destinations.push(QuestSourceItemDestination::NewStack { slot, count });
    }

    if remaining != 0 {
        return QuestSourceItemStorage::NoSpace;
    }

    QuestSourceItemStorage::Grant(QuestSourceItemStoragePlan {
        item_id: quest.src_item_id,
        max_durability: item_template.max_durability,
        container_slots: item_template.container_slots,
        destinations,
    })
}

pub(in crate::world) fn quest_can_complete_from_inventory(
    quest: &QuestTemplateQuery,
    inventory: &[CharacterInventoryItem],
) -> bool {
    if quest_has_unsupported_completion_requirements(quest) {
        return false;
    }

    if quest
        .req_creature_or_go_id
        .iter()
        .zip(quest.req_creature_or_go_count.iter())
        .any(|(id, count)| *id != 0 && *count != 0)
    {
        return false;
    }

    quest_required_items_satisfied(quest, inventory)
}

pub(in crate::world) async fn quest_reward_item_displays(
    world_db_pool: &MySqlPool,
    quest: &QuestTemplateQuery,
) -> anyhow::Result<QuestRewardItemDisplays> {
    let mut displays = QuestRewardItemDisplays::default();
    for (index, item) in quest.req_item_id.iter().enumerate() {
        if *item != 0 {
            displays.required[index] = wow_db::get_item_display_id(world_db_pool, *item)
                .await?
                .unwrap_or(0);
        }
    }
    for (index, item) in quest.rew_choice_item_id.iter().enumerate() {
        if *item != 0 {
            displays.choice[index] = wow_db::get_item_display_id(world_db_pool, *item)
                .await?
                .unwrap_or(0);
        }
    }
    for (index, item) in quest.rew_item_id.iter().enumerate() {
        if *item != 0 {
            displays.reward[index] = wow_db::get_item_display_id(world_db_pool, *item)
                .await?
                .unwrap_or(0);
        }
    }
    Ok(displays)
}

pub(in crate::world) fn quest_request_items_skips_to_offer_reward(
    quest: &QuestTemplateQuery,
    complete: bool,
) -> bool {
    quest.request_items_text.is_empty()
        || (complete && quest.req_item_id.iter().all(|item| *item == 0))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct QuestRewardItem {
    pub(in crate::world) item: u32,
    pub(in crate::world) count: u32,
}

#[derive(Debug, Clone)]
pub(in crate::world) struct QuestRewardGrant {
    pub(in crate::world) item: u32,
    pub(in crate::world) count: u32,
    pub(in crate::world) max_durability: u32,
    pub(in crate::world) container_slots: Option<u32>,
    pub(in crate::world) template: ItemTemplateQuery,
}

pub(in crate::world) fn selected_quest_reward_items(
    quest: &QuestTemplateQuery,
    reward: u32,
) -> Option<Vec<QuestRewardItem>> {
    let mut items = Vec::new();
    let has_choice = quest
        .rew_choice_item_id
        .iter()
        .zip(quest.rew_choice_item_count.iter())
        .any(|(item, count)| *item != 0 && *count != 0);
    if has_choice {
        let index = usize::try_from(reward).ok()?;
        let item = *quest.rew_choice_item_id.get(index)?;
        let count = *quest.rew_choice_item_count.get(index)?;
        if item == 0 || count == 0 {
            return None;
        }
        items.push(QuestRewardItem { item, count });
    }
    for (item, count) in quest.rew_item_id.iter().zip(quest.rew_item_count.iter()) {
        if *item != 0 && *count != 0 {
            items.push(QuestRewardItem {
                item: *item,
                count: *count,
            });
        }
    }
    Some(items)
}

pub(in crate::world) async fn load_quest_reward_grants(
    world_db_pool: &MySqlPool,
    items: &[QuestRewardItem],
) -> anyhow::Result<Vec<QuestRewardGrant>> {
    let mut grants = Vec::with_capacity(items.len());
    for item in items {
        let Some(template) = wow_db::get_item_template_query(world_db_pool, item.item).await?
        else {
            anyhow::bail!("Quest reward item {} has no item_template row", item.item);
        };
        grants.push(QuestRewardGrant {
            item: item.item,
            count: item.count,
            max_durability: template.max_durability,
            container_slots: (template.container_slots > 0).then_some(template.container_slots),
            template,
        });
    }
    Ok(grants)
}

pub(in crate::world) fn plan_quest_reward_storage(
    inventory: &[CharacterInventoryItem],
    rewards: &[QuestRewardGrant],
    equipped_bags: &[EquippedBagInfo],
    required_consumes: &[QuestRequiredItemConsume],
) -> Option<Vec<Vec<StoreSlot>>> {
    let mut planned_inventory = inventory.to_vec();
    apply_required_item_consumes_to_planned_inventory(&mut planned_inventory, required_consumes);

    let mut reward_plans = Vec::with_capacity(rewards.len());
    for reward in rewards {
        let store_plan = plan_store_item(
            &planned_inventory,
            &reward.template,
            reward.count,
            equipped_bags,
            None,
            None,
        )?;
        apply_store_plan_to_planned_inventory(&mut planned_inventory, reward, &store_plan);
        reward_plans.push(store_plan);
    }
    Some(reward_plans)
}

pub(in crate::world) fn apply_required_item_consumes_to_planned_inventory(
    inventory: &mut Vec<CharacterInventoryItem>,
    required_consumes: &[QuestRequiredItemConsume],
) {
    for consume in required_consumes {
        let Some(index) = inventory
            .iter()
            .position(|item| item.bag == consume.bag && item.slot == consume.slot)
        else {
            continue;
        };
        if consume.removes_stack || inventory[index].count <= consume.count {
            inventory.remove(index);
        } else {
            inventory[index].count -= consume.count;
        }
    }
}

pub(in crate::world) fn apply_store_plan_to_planned_inventory(
    inventory: &mut Vec<CharacterInventoryItem>,
    reward: &QuestRewardGrant,
    store_plan: &[StoreSlot],
) {
    for slot in store_plan {
        if let Some(item_guid) = slot.existing_item {
            if let Some(item) = inventory.iter_mut().find(|item| item.item == item_guid) {
                item.count = item.count.saturating_add(slot.count);
            }
            continue;
        }
        inventory.push(CharacterInventoryItem {
            bag: slot.bag as u32,
            slot: slot.slot,
            item: 0,
            item_template: 0,
            count: slot.count,
            random_property_id: 0,
            charges: String::new(),
            enchantments: String::new(),
            durability: reward.max_durability,
        });
    }
}

pub(in crate::world) async fn grant_quest_reward_items(
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    character_guid: u32,
    rewards: &[QuestRewardGrant],
    reward_storage_plans: &[Vec<StoreSlot>],
    session: &mut WorldSessionState,
) -> anyhow::Result<Vec<Vec<u8>>> {
    if rewards.len() != reward_storage_plans.len() {
        anyhow::bail!("Quest reward storage plan count did not match reward count");
    }
    let mut update_blocks = Vec::new();
    for (reward, store_plan) in rewards.iter().zip(reward_storage_plans) {
        let random_properties = generate_item_instance_random_properties(
            world_db_pool,
            &session.movement.db_creature_navigation.world_data_files,
            reward.item,
        )
        .await?;
        for slot in store_plan {
            if let Some(item_guid) = slot.existing_item {
                let existing_count = session
                    .inventory
                    .items
                    .iter()
                    .find(|item| item.item == item_guid)
                    .map(|item| item.count)
                    .unwrap_or(0);
                wow_db::update_character_inventory_item_count(
                    character_db_pool,
                    character_guid,
                    item_guid,
                    existing_count.saturating_add(slot.count),
                )
                .await?;
            } else {
                wow_db::add_character_inventory_item_with_random_properties(
                    character_db_pool,
                    wow_db::AddCharacterInventoryItemRequest {
                        guid: character_guid,
                        bag: slot.bag as u32,
                        slot: slot.slot,
                        item_template: reward.item,
                        count: slot.count,
                        durability: reward.max_durability,
                        random_properties: random_properties.as_ref(),
                    },
                )
                .await?;
            }
        }
        session.inventory.items =
            wow_db::get_character_inventory_items(character_db_pool, character_guid).await?;
        let owner_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
        for slot in store_plan {
            if let Some(item_guid) = slot.existing_item {
                if let Some(item) = session
                    .inventory
                    .items
                    .iter()
                    .find(|item| item.item == item_guid)
                {
                    update_blocks.push(build_item_stack_count_update_block(item.item, item.count)?);
                }
                continue;
            }
            if let Some(new_item) = session
                .inventory
                .items
                .iter()
                .find(|item| item.bag == slot.bag as u32 && item.slot == slot.slot)
            {
                let contained_guid =
                    item_contained_guid(owner_guid, &session.inventory.items, new_item);
                update_blocks.push(build_item_create_update_block(
                    owner_guid,
                    contained_guid,
                    new_item,
                    reward.container_slots,
                )?);
                update_blocks.extend(build_inventory_position_update_blocks(
                    character_guid,
                    &session.inventory.items,
                    slot.bag,
                    slot.slot,
                )?);
            }
        }
    }
    Ok(update_blocks)
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct QuestRequiredItemConsume {
    pub(in crate::world) bag: u32,
    pub(in crate::world) slot: u8,
    pub(in crate::world) count: u32,
    pub(in crate::world) removes_stack: bool,
}

pub(in crate::world) fn quest_required_item_inventory_slots(
    quest: &QuestTemplateQuery,
    inventory: &[CharacterInventoryItem],
) -> Vec<QuestRequiredItemConsume> {
    let mut consumes = Vec::new();
    for (required_item, required_count) in quest.req_item_id.iter().zip(quest.req_item_count.iter())
    {
        if *required_item == 0 || *required_count == 0 {
            continue;
        }
        let mut remaining = *required_count;
        let mut stacks: Vec<_> = inventory
            .iter()
            .filter(|item| item.item_template == *required_item)
            .collect();
        stacks.sort_by_key(|item| {
            let bag_order = if item.bag == INVENTORY_SLOT_BAG_0 as u32 {
                0
            } else {
                1
            };
            (bag_order, item.bag, item.slot)
        });
        for item in stacks {
            if remaining == 0 {
                break;
            }
            let count = remaining.min(item.count);
            remaining -= count;
            consumes.push(QuestRequiredItemConsume {
                bag: item.bag,
                slot: item.slot,
                count,
                removes_stack: count >= item.count,
            });
        }
    }
    consumes
}

pub(in crate::world) fn empty_backpack_slots(inventory: &[CharacterInventoryItem]) -> Vec<u8> {
    (INVENTORY_SLOT_ITEM_START..INVENTORY_SLOT_ITEM_END)
        .filter(|slot| {
            inventory
                .iter()
                .all(|item| item.bag != INVENTORY_SLOT_BAG_0 as u32 || item.slot != *slot)
        })
        .collect()
}

pub(in crate::world) async fn consume_quest_required_items(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    character_guid: u32,
    quest: &QuestTemplateQuery,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let consumes = quest_required_item_inventory_slots(quest, &session.inventory.items);
    let mut stack_update_blocks = Vec::new();
    let mut removed_positions = Vec::new();
    let mut removed_non_backpack_items = Vec::new();

    for consume in consumes {
        match wow_db::destroy_character_inventory_item_count(
            character_db_pool,
            character_guid,
            consume.bag,
            consume.slot,
            consume.count,
        )
        .await?
        {
            Some(wow_db::InventoryDestroyResult::CountChanged { item, count }) => {
                stack_update_blocks.push(build_item_stack_count_update_block(item, count)?);
            }
            Some(wow_db::InventoryDestroyResult::Removed { item }) => {
                removed_positions.push((consume.bag, consume.slot));
                if consume.bag != INVENTORY_SLOT_BAG_0 as u32 {
                    removed_non_backpack_items.push(item);
                }
            }
            None => {
                warn!(
                    quest = quest.entry,
                    bag = consume.bag,
                    slot = consume.slot,
                    "Quest required item disappeared before reward consumption"
                );
            }
        }
    }

    if stack_update_blocks.is_empty() && removed_positions.is_empty() {
        return Ok(());
    }

    session.inventory.items =
        wow_db::get_character_inventory_items(character_db_pool, character_guid).await?;
    let mut update_blocks = stack_update_blocks;
    for (bag, slot) in removed_positions {
        let Ok(bag) = u8::try_from(bag) else {
            continue;
        };
        update_blocks.extend(build_inventory_position_update_blocks(
            character_guid,
            &session.inventory.items,
            bag,
            slot,
        )?);
    }
    if !update_blocks.is_empty() {
        let body = build_update_object_body(&update_blocks);
        send_packet(
            stream,
            WorldOpcode::SmsgUpdateObject as u16,
            &body,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    for item in removed_non_backpack_items {
        send_packet(
            stream,
            WorldOpcode::SmsgDestroyObject as u16,
            &build_destroy_object_body(item),
            Some(&mut *header_crypto),
        )
        .await?;
    }
    Ok(())
}

pub(in crate::world) fn quest_status_can_reward_from_inventory(
    status: &CharacterQuestStatus,
    quest: &QuestTemplateQuery,
    inventory: &[CharacterInventoryItem],
) -> bool {
    status.rewarded == 0
        && (status.status == QUEST_STATUS_COMPLETE || status.status == QUEST_STATUS_INCOMPLETE)
        && quest_completion_requirements_satisfied(status, quest, inventory)
}

pub(in crate::world) fn quest_status_can_complete(
    status: &CharacterQuestStatus,
    quest: &QuestTemplateQuery,
    inventory: &[CharacterInventoryItem],
) -> bool {
    status.rewarded == 0
        && status.status == QUEST_STATUS_INCOMPLETE
        && quest_completion_requirements_satisfied(status, quest, inventory)
}

pub(in crate::world) fn quest_completion_requirements_satisfied(
    status: &CharacterQuestStatus,
    quest: &QuestTemplateQuery,
    inventory: &[CharacterInventoryItem],
) -> bool {
    if quest_has_unsupported_completion_requirements(quest) {
        return false;
    }

    quest_creature_or_go_objectives_satisfied(status, quest)
        && quest_required_items_satisfied(quest, inventory)
}

pub(in crate::world) fn quest_has_unsupported_completion_requirements(
    quest: &QuestTemplateQuery,
) -> bool {
    const QUEST_SPECIAL_FLAG_EXPLORATION_OR_EVENT: u32 = 0x002;
    (quest.special_flags & QUEST_SPECIAL_FLAG_EXPLORATION_OR_EVENT) != 0
        || quest.rep_objective_faction != 0
}

pub(in crate::world) fn quest_creature_or_go_objectives_satisfied(
    status: &CharacterQuestStatus,
    quest: &QuestTemplateQuery,
) -> bool {
    quest
        .req_creature_or_go_id
        .iter()
        .zip(quest.req_creature_or_go_count.iter())
        .enumerate()
        .all(|(index, (id, required_count))| {
            if *id == 0 || *required_count == 0 {
                return true;
            }
            quest_status_creature_or_go_count(status, index) >= *required_count
        })
}

pub(in crate::world) fn quest_status_creature_or_go_count(
    status: &CharacterQuestStatus,
    index: usize,
) -> u32 {
    match index {
        0 => status.mobcount1,
        1 => status.mobcount2,
        2 => status.mobcount3,
        3 => status.mobcount4,
        _ => 0,
    }
}

pub(in crate::world) fn quest_required_items_satisfied(
    quest: &QuestTemplateQuery,
    inventory: &[CharacterInventoryItem],
) -> bool {
    quest
        .req_item_id
        .iter()
        .zip(quest.req_item_count.iter())
        .all(|(item_id, required_count)| {
            if *item_id == 0 || *required_count == 0 {
                return true;
            }
            quest_inventory_item_count(inventory, *item_id) >= *required_count
        })
}

pub(in crate::world) fn quest_inventory_item_count(
    inventory: &[CharacterInventoryItem],
    item_id: u32,
) -> u32 {
    inventory
        .iter()
        .filter(|item| item.item_template == item_id)
        .map(|item| item.count)
        .sum()
}

pub(in crate::world) fn quest_has_required_items(quest: &QuestTemplateQuery) -> bool {
    quest
        .req_item_id
        .iter()
        .zip(quest.req_item_count.iter())
        .any(|(item_id, required_count)| *item_id != 0 && *required_count != 0)
}

pub(in crate::world) async fn complete_inventory_item_quests(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    session: &mut WorldSessionState,
    character_guid: u32,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let active_quests: Vec<u32> = session
        .quests
        .quest_statuses
        .values()
        .filter(|status| status.status == QUEST_STATUS_INCOMPLETE && status.rewarded == 0)
        .map(|status| status.quest)
        .collect();

    for quest_id in active_quests {
        let Some(quest) = object_mgr.quest_template(world_db_pool, quest_id).await? else {
            continue;
        };
        let Some(current_status) = session.quests.quest_statuses.get(&quest_id).cloned() else {
            continue;
        };
        if !quest_status_can_complete(&current_status, &quest, &session.inventory.items) {
            continue;
        }

        let status =
            wow_db::complete_character_quest(character_db_pool, character_guid, quest_id).await?;
        session
            .quests
            .quest_statuses
            .insert(quest_id, status.clone());
        let Some(slot) = quest_log_slot_for_quest(session, quest_id) else {
            warn!(
                quest = quest_id,
                "Quest item objective completed but no quest-log slot was available"
            );
            continue;
        };
        send_packet(
            stream,
            WorldOpcode::SmsgUpdateObject as u16,
            &build_player_quest_log_update_body(character_guid, slot, &status)?,
            Some(&mut *header_crypto),
        )
        .await?;
        send_packet(
            stream,
            WorldOpcode::SmsgQuestUpdateComplete as u16,
            &quest_id.to_le_bytes(),
            Some(&mut *header_crypto),
        )
        .await?;
    }

    Ok(())
}

pub(in crate::world) async fn revalidate_completed_item_quests_after_inventory_change(
    stream: &mut WorldPacketSink,
    deps: QuestMutationDeps<'_>,
    session: &mut WorldSessionState,
    character_guid: u32,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let character_db_pool = deps.character_db_pool;
    let object_mgr = deps.object_mgr;
    let world_db_pool = deps.world_db_pool;
    let completed_quests: Vec<u32> = session
        .quests
        .quest_statuses
        .values()
        .filter(|status| status.status == QUEST_STATUS_COMPLETE && status.rewarded == 0)
        .map(|status| status.quest)
        .collect();
    let mut changed = false;

    for quest_id in completed_quests {
        let Some(status) = session.quests.quest_statuses.get(&quest_id).cloned() else {
            continue;
        };
        let Some(quest) = object_mgr.quest_template(world_db_pool, quest_id).await? else {
            continue;
        };
        if !quest_has_required_items(&quest) {
            continue;
        }
        if quest_status_can_reward_from_inventory(&status, &quest, &session.inventory.items) {
            continue;
        }

        let updated =
            wow_db::incomplete_character_quest(character_db_pool, character_guid, quest_id).await?;
        session
            .quests
            .quest_statuses
            .insert(quest_id, updated.clone());
        if let Some(slot) = quest_log_slot_for_quest(session, quest_id) {
            send_packet(
                stream,
                WorldOpcode::SmsgUpdateObject as u16,
                &build_player_quest_log_update_body(character_guid, slot, &updated)?,
                Some(&mut *header_crypto),
            )
            .await?;
        }
        changed = true;
    }

    if changed {
        send_visible_questgiver_status_updates(
            stream,
            object_mgr,
            world_db_pool,
            deps.shared_world,
            session,
            &[],
            header_crypto,
        )
        .await?;
    }

    Ok(())
}

pub(in crate::world) fn can_quest_be_started_from_status(
    quest: &QuestTemplateQuery,
    status: Option<&CharacterQuestStatus>,
) -> bool {
    status.is_none_or(|state| {
        state.status == 0
            || (quest.is_repeatable()
                && state.rewarded != 0
                && state.status == QUEST_STATUS_COMPLETE)
    })
}

pub(in crate::world) fn quest_status_is_current(status: &CharacterQuestStatus) -> bool {
    status.rewarded == 0
        && (status.status == QUEST_STATUS_INCOMPLETE || status.status == QUEST_STATUS_COMPLETE)
}

pub(in crate::world) fn quest_is_current(
    quest_statuses: &HashMap<u32, CharacterQuestStatus>,
    quest: u32,
) -> bool {
    quest_statuses
        .get(&quest)
        .is_some_and(quest_status_is_current)
}

pub(in crate::world) fn quest_race_or_class_mask(id: u8) -> u32 {
    if id == 0 {
        return 0;
    }
    1u32.checked_shl(u32::from(id - 1)).unwrap_or(0)
}

pub(in crate::world) const QUEST_HIGH_LEVEL_HIDE_DIFF: u8 = 7;

pub(in crate::world) fn satisfies_race_class_level(
    quest: &QuestTemplateQuery,
    character: &ActiveCharacter,
) -> bool {
    if character.level < quest.min_level
        || (quest.max_level != 0 && character.level > quest.max_level)
    {
        return false;
    }

    let class_mask = quest_race_or_class_mask(character.class);
    if quest.required_classes != 0 && (quest.required_classes & class_mask) == 0 {
        return false;
    }

    let race_mask = quest_race_or_class_mask(character.race);
    if quest.required_races != 0 && (quest.required_races & race_mask) == 0 {
        return false;
    }

    true
}

pub(in crate::world) fn satisfies_quest_visibility_level(
    quest: &QuestTemplateQuery,
    character: &ActiveCharacter,
) -> bool {
    if quest.max_level != 0 && character.level > quest.max_level {
        return false;
    }

    u16::from(character.level) + u16::from(QUEST_HIGH_LEVEL_HIDE_DIFF) >= u16::from(quest.min_level)
}

pub(in crate::world) fn satisfies_race_class(
    quest: &QuestTemplateQuery,
    character: &ActiveCharacter,
) -> bool {
    let class_mask = quest_race_or_class_mask(character.class);
    if quest.required_classes != 0 && (quest.required_classes & class_mask) == 0 {
        return false;
    }

    let race_mask = quest_race_or_class_mask(character.race);
    if quest.required_races != 0 && (quest.required_races & race_mask) == 0 {
        return false;
    }

    true
}

pub(in crate::world) fn satisfies_required_skill(
    quest: &QuestTemplateQuery,
    character_skills: &[CharacterSkill],
) -> bool {
    if quest.required_skill == 0 {
        return true;
    }

    let skill_value = character_skills
        .iter()
        .find(|skill| u32::from(skill.skill) == quest.required_skill)
        .map_or(0, |skill| u32::from(skill.value));
    skill_value >= quest.required_skill_value
}

pub(in crate::world) async fn satisfies_required_condition(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    quest: &QuestTemplateQuery,
    session: &WorldSessionState,
    depth: u8,
) -> anyhow::Result<bool> {
    if quest.required_condition == 0 {
        return Ok(true);
    }
    let context = ConditionEvaluationContext {
        world_db_pool,
        session,
        source: ConditionSource::Quest,
    };
    if depth == 0 {
        object_mgr
            .is_condition_satisfied(quest.required_condition, context)
            .await
    } else {
        object_mgr
            .is_condition_satisfied_with_depth(quest.required_condition, context, depth)
            .await
    }
}

pub(in crate::world) fn satisfies_required_reputation(
    quest: &QuestTemplateQuery,
    character_reputations: &[CharacterReputation],
) -> bool {
    if quest.required_min_rep_faction != 0 {
        let reputation = character_reputations
            .iter()
            .find(|reputation| reputation.faction == quest.required_min_rep_faction)
            .map_or(0, |reputation| reputation.standing);
        if reputation < quest.required_min_rep_value {
            return false;
        }
    }

    if quest.required_max_rep_faction != 0 {
        let reputation = character_reputations
            .iter()
            .find(|reputation| reputation.faction == quest.required_max_rep_faction)
            .map_or(0, |reputation| reputation.standing);
        if reputation >= quest.required_max_rep_value {
            return false;
        }
    }

    true
}

#[cfg(test)]
pub(in crate::world) fn satisfies_prev_quest_requirement(
    quest_statuses: &HashMap<u32, CharacterQuestStatus>,
    prev_quest_id: i32,
) -> bool {
    let prev_quest = prev_quest_id.unsigned_abs();
    if prev_quest_id > 0 {
        return quest_statuses
            .get(&prev_quest)
            .is_some_and(|status| status.rewarded != 0);
    }
    if prev_quest_id < 0 {
        return quest_is_current(quest_statuses, prev_quest);
    }
    true
}

pub(in crate::world) async fn satisfies_prev_quest_requirements(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    quest: &QuestTemplateQuery,
    quest_statuses: &HashMap<u32, CharacterQuestStatus>,
    prev_quest_ids: &[i32],
) -> anyhow::Result<bool> {
    if prev_quest_ids.is_empty() {
        return Ok(true);
    }

    for prev_quest_id in prev_quest_ids {
        let prev_quest = prev_quest_id.unsigned_abs();
        let Some(prev_status) = quest_statuses.get(&prev_quest) else {
            continue;
        };
        let Some(prev_info) = object_mgr.quest_template(world_db_pool, prev_quest).await? else {
            continue;
        };

        if *prev_quest_id > 0 && prev_status.rewarded != 0 {
            if prev_info.exclusive_group >= 0 {
                return Ok(true);
            }
            if quest.prev_quest_id != 0 && prev_info.next_quest_id != quest.prev_quest_id {
                return Ok(true);
            }
            let exclusive_group_quests = object_mgr
                .exclusive_group_quests(world_db_pool, prev_info.exclusive_group)
                .await?;
            for exclusive_quest in exclusive_group_quests {
                if exclusive_quest == prev_quest {
                    continue;
                }
                if quest_statuses
                    .get(&exclusive_quest)
                    .is_none_or(|status| status.rewarded == 0)
                {
                    return Ok(false);
                }
            }
            return Ok(true);
        }

        if *prev_quest_id < 0 && quest_status_is_current(prev_status) {
            if prev_info.exclusive_group >= 0 {
                return Ok(true);
            }
            if quest.prev_quest_id != 0
                && prev_info.next_quest_id
                    != i32::try_from(quest.prev_quest_id.unsigned_abs()).unwrap_or(i32::MAX)
            {
                return Ok(true);
            }
            let exclusive_group_quests = object_mgr
                .exclusive_group_quests(world_db_pool, prev_info.exclusive_group)
                .await?;
            for exclusive_quest in exclusive_group_quests {
                if exclusive_quest == prev_quest {
                    continue;
                }
                if !quest_is_current(quest_statuses, exclusive_quest) {
                    return Ok(false);
                }
            }
            return Ok(true);
        }
    }

    Ok(false)
}

pub(in crate::world) fn satisfies_exclusive_group(
    quest: &QuestTemplateQuery,
    exclusive_group_quests: &[u32],
    quest_statuses: &HashMap<u32, CharacterQuestStatus>,
) -> bool {
    if quest.exclusive_group <= 0 {
        return true;
    }

    for other_quest in exclusive_group_quests {
        if *other_quest == quest.entry {
            continue;
        }
        if quest_statuses
            .get(other_quest)
            .is_some_and(quest_status_is_current)
        {
            return false;
        }
    }

    true
}

pub(in crate::world) async fn can_take_start_quest(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    quest: &QuestTemplateQuery,
    session: &WorldSessionState,
) -> anyhow::Result<bool> {
    can_take_start_quest_with_condition_depth(object_mgr, world_db_pool, quest, session, 0).await
}

pub(in crate::world) async fn can_take_start_quest_with_condition_depth(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    quest: &QuestTemplateQuery,
    session: &WorldSessionState,
    condition_depth: u8,
) -> anyhow::Result<bool> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(false);
    };
    if !satisfies_race_class_level(quest, character) {
        return Ok(false);
    }
    if !satisfies_required_skill(quest, &session.character.character_skills) {
        return Ok(false);
    }
    if !satisfies_required_condition(object_mgr, world_db_pool, quest, session, condition_depth)
        .await?
    {
        return Ok(false);
    }
    if !satisfies_required_reputation(quest, &session.character.character_reputations) {
        return Ok(false);
    }
    if !can_quest_be_started_from_status(quest, session.quests.quest_statuses.get(&quest.entry)) {
        return Ok(false);
    }

    let prev_quests = object_mgr
        .quest_prev_quests(world_db_pool, quest.entry)
        .await?;
    if !satisfies_prev_quest_requirements(
        object_mgr,
        world_db_pool,
        quest,
        &session.quests.quest_statuses,
        &prev_quests,
    )
    .await?
    {
        return Ok(false);
    }

    let prev_chain_quests = object_mgr
        .quest_prev_chain_quests(world_db_pool, quest.entry)
        .await?;
    if prev_chain_quests
        .into_iter()
        .any(|prev_chain| quest_is_current(&session.quests.quest_statuses, prev_chain))
    {
        return Ok(false);
    }

    if quest.next_quest_in_chain != 0
        && quest_is_current(&session.quests.quest_statuses, quest.next_quest_in_chain)
    {
        return Ok(false);
    }

    let exclusive_group_quests = object_mgr
        .exclusive_group_quests(world_db_pool, quest.exclusive_group)
        .await?;
    if !satisfies_exclusive_group(
        quest,
        &exclusive_group_quests,
        &session.quests.quest_statuses,
    ) {
        return Ok(false);
    }

    Ok(true)
}

pub(in crate::world) async fn can_see_start_quest(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    quest: &QuestTemplateQuery,
    session: &WorldSessionState,
) -> anyhow::Result<bool> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(false);
    };
    if !satisfies_race_class(quest, character) {
        return Ok(false);
    }
    if !satisfies_quest_visibility_level(quest, character) {
        return Ok(false);
    }
    if !satisfies_required_skill(quest, &session.character.character_skills) {
        return Ok(false);
    }
    if !satisfies_required_condition(object_mgr, world_db_pool, quest, session, 0).await? {
        return Ok(false);
    }
    if !satisfies_required_reputation(quest, &session.character.character_reputations) {
        return Ok(false);
    }
    if !can_quest_be_started_from_status(quest, session.quests.quest_statuses.get(&quest.entry)) {
        return Ok(false);
    }

    let prev_quests = object_mgr
        .quest_prev_quests(world_db_pool, quest.entry)
        .await?;
    if !satisfies_prev_quest_requirements(
        object_mgr,
        world_db_pool,
        quest,
        &session.quests.quest_statuses,
        &prev_quests,
    )
    .await?
    {
        return Ok(false);
    }

    let prev_chain_quests = object_mgr
        .quest_prev_chain_quests(world_db_pool, quest.entry)
        .await?;
    if prev_chain_quests
        .into_iter()
        .any(|prev_chain| quest_is_current(&session.quests.quest_statuses, prev_chain))
    {
        return Ok(false);
    }

    if quest.next_quest_in_chain != 0
        && quest_is_current(&session.quests.quest_statuses, quest.next_quest_in_chain)
    {
        return Ok(false);
    }

    let exclusive_group_quests = object_mgr
        .exclusive_group_quests(world_db_pool, quest.exclusive_group)
        .await?;
    if !satisfies_exclusive_group(
        quest,
        &exclusive_group_quests,
        &session.quests.quest_statuses,
    ) {
        return Ok(false);
    }

    Ok(true)
}

pub(in crate::world) async fn quest_start_dialog_status(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    quest: &QuestTemplateQuery,
    session: &WorldSessionState,
) -> anyhow::Result<Option<u32>> {
    let can_take = can_take_start_quest(object_mgr, world_db_pool, quest, session).await?;
    let can_see = if can_take {
        true
    } else {
        can_see_start_quest(object_mgr, world_db_pool, quest, session).await?
    };
    Ok(start_quest_dialog_status(can_take, can_see))
}

pub(in crate::world) fn start_quest_dialog_status(can_take: bool, can_see: bool) -> Option<u32> {
    if can_take {
        Some(DIALOG_STATUS_AVAILABLE)
    } else if can_see {
        Some(DIALOG_STATUS_UNAVAILABLE)
    } else {
        None
    }
}

pub(in crate::world) fn quest_log_slot_for_quest(
    session: &WorldSessionState,
    quest: u32,
) -> Option<usize> {
    session
        .quests
        .quest_log_slots
        .iter()
        .position(|slot_quest| *slot_quest == quest)
}

pub(in crate::world) fn quest_log_slot_for_statuses(
    statuses: &HashMap<u32, CharacterQuestStatus>,
    quest: u32,
) -> Option<usize> {
    active_quest_statuses_sorted(statuses)
        .into_iter()
        .take(MAX_QUEST_LOG_SIZE)
        .position(|status| status.quest == quest)
}

pub(in crate::world) fn quest_log_has_free_slot(session: &WorldSessionState) -> bool {
    session.quests.quest_log_slots.contains(&0)
}

pub(in crate::world) fn assign_quest_log_slot(
    session: &mut WorldSessionState,
    quest: u32,
) -> Option<usize> {
    if let Some(slot) = quest_log_slot_for_quest(session, quest) {
        return Some(slot);
    }
    let slot = session
        .quests
        .quest_log_slots
        .iter()
        .position(|quest| *quest == 0)?;
    session.quests.quest_log_slots[slot] = quest;
    Some(slot)
}

pub(in crate::world) fn clear_quest_log_slot(
    session: &mut WorldSessionState,
    quest: u32,
) -> Option<usize> {
    let slot = quest_log_slot_for_quest(session, quest)?;
    session.quests.quest_log_slots[slot] = 0;
    Some(slot)
}

pub(in crate::world) fn quest_log_slots_from_statuses(
    statuses: &HashMap<u32, CharacterQuestStatus>,
) -> [u32; MAX_QUEST_LOG_SIZE] {
    let mut slots = [0; MAX_QUEST_LOG_SIZE];
    for (slot, status) in active_quest_statuses_sorted(statuses)
        .into_iter()
        .take(MAX_QUEST_LOG_SIZE)
        .enumerate()
    {
        slots[slot] = status.quest;
    }
    slots
}

pub(in crate::world) async fn grant_db_creature_kill_credit(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    session: &mut WorldSessionState,
    killed_guid: ObjectGuid,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.character.active_character else {
        return Ok(());
    };
    let killed_entry = killed_guid.entry();
    let active_quests: Vec<u32> = session
        .quests
        .quest_statuses
        .values()
        .filter(|status| status.status == QUEST_STATUS_INCOMPLETE && status.rewarded == 0)
        .map(|status| status.quest)
        .collect();
    for quest_id in active_quests {
        let Some(quest) = object_mgr.quest_template(world_db_pool, quest_id).await? else {
            continue;
        };
        let Some(index) = quest.required_creature_index(killed_entry) else {
            continue;
        };
        let required = quest.required_creature_count(index);
        if required == 0 {
            continue;
        }
        let current = session
            .quests
            .quest_statuses
            .get(&quest_id)
            .map(|status| match index {
                0 => status.mobcount1,
                1 => status.mobcount2,
                2 => status.mobcount3,
                3 => status.mobcount4,
                _ => 0,
            })
            .unwrap_or(0);
        if current >= required {
            continue;
        }
        let new_count = (current + 1).min(required);
        let mut next_status = session
            .quests
            .quest_statuses
            .get(&quest_id)
            .cloned()
            .unwrap_or(CharacterQuestStatus {
                quest: quest_id,
                status: QUEST_STATUS_INCOMPLETE,
                rewarded: 0,
                mobcount1: 0,
                mobcount2: 0,
                mobcount3: 0,
                mobcount4: 0,
            });
        match index {
            0 => next_status.mobcount1 = new_count,
            1 => next_status.mobcount2 = new_count,
            2 => next_status.mobcount3 = new_count,
            3 => next_status.mobcount4 = new_count,
            _ => {}
        }
        let complete = quest_status_can_complete(&next_status, &quest, &session.inventory.items);
        let status = wow_db::update_character_quest_mob_count(
            character_db_pool,
            character.guid,
            quest_id,
            index,
            new_count,
            complete,
        )
        .await?;
        session
            .quests
            .quest_statuses
            .insert(quest_id, status.clone());
        let Some(slot) = quest_log_slot_for_quest(session, quest_id) else {
            warn!(
                quest = quest_id,
                "Quest progress updated but no quest-log slot was available"
            );
            continue;
        };
        send_packet(
            stream,
            WorldOpcode::SmsgQuestUpdateAddKill as u16,
            &build_quest_update_add_kill_body(&quest, killed_guid, index, new_count),
            Some(&mut *header_crypto),
        )
        .await?;
        send_packet(
            stream,
            WorldOpcode::SmsgUpdateObject as u16,
            &build_player_quest_log_update_body(character.guid, slot, &status)?,
            Some(&mut *header_crypto),
        )
        .await?;
        if complete {
            send_packet(
                stream,
                WorldOpcode::SmsgQuestUpdateComplete as u16,
                &quest_id.to_le_bytes(),
                Some(&mut *header_crypto),
            )
            .await?;
        }
    }
    Ok(())
}
