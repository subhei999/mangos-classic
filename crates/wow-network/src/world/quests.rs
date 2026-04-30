async fn handle_quest_query(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let mut cursor = 0;
    let quest_id = read_u32(body, &mut cursor)?;
    let Some(quest) = wow_db::get_quest_template_query(world_db_pool, quest_id).await? else {
        warn!(quest_id, "Ignoring query for unknown quest");
        return Ok(());
    };
    let response = build_quest_query_response_body(&quest);
    send_packet(
        stream,
        SMSG_QUEST_QUERY_RESPONSE,
        &response,
        Some(header_crypto),
    )
    .await
}

async fn handle_questgiver_status_query(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    body: &[u8],
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let guid = read_packet_guid(body, "CMSG_QUESTGIVER_STATUS_QUERY")?;
    let status = questgiver_dialog_status(world_db_pool, guid, session).await?;
    send_packet(
        stream,
        SMSG_QUESTGIVER_STATUS,
        &build_questgiver_status_body(guid, status),
        Some(header_crypto),
    )
    .await
}

async fn handle_questgiver_hello(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    body: &[u8],
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let guid = read_packet_guid(body, "CMSG_QUESTGIVER_HELLO")?;
    if let Some(quest) = questgiver_completed_turnin_quest(world_db_pool, guid, session).await? {
        let response = build_quest_offer_reward_body(guid, &quest);
        return send_packet(
            stream,
            SMSG_QUESTGIVER_OFFER_REWARD,
            &response,
            Some(header_crypto),
        )
        .await;
    }

    let quests = questgiver_visible_quests(world_db_pool, guid, session).await?;
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
        SMSG_QUESTGIVER_QUEST_LIST,
        &response,
        Some(header_crypto),
    )
    .await
}

async fn handle_questgiver_query_quest(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let request = QuestgiverQuestRequest::read(body, "CMSG_QUESTGIVER_QUERY_QUEST")?;
    if !request.guid.is_creature()
        || !wow_db::creature_starts_quest(world_db_pool, request.guid.entry(), request.quest).await?
            && !wow_db::creature_completes_quest(world_db_pool, request.guid.entry(), request.quest)
                .await?
    {
        warn!(quest = request.quest, "Ignoring quest details request for invalid giver");
        return Ok(());
    }
    let Some(quest) = wow_db::get_quest_template_query(world_db_pool, request.quest).await? else {
        return Ok(());
    };
    let response = build_quest_details_body(request.guid, &quest);
    send_packet(
        stream,
        SMSG_QUESTGIVER_QUEST_DETAILS,
        &response,
        Some(header_crypto),
    )
    .await
}

async fn handle_questgiver_accept_quest(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        warn!("Ignoring quest accept before character login");
        return Ok(());
    };
    let request = QuestgiverQuestRequest::read(body, "CMSG_QUESTGIVER_ACCEPT_QUEST")?;
    if !request.guid.is_creature()
        || !wow_db::creature_starts_quest(world_db_pool, request.guid.entry(), request.quest).await?
    {
        warn!(quest = request.quest, "Ignoring quest accept for invalid giver");
        return Ok(());
    }
    let status = wow_db::accept_character_quest(character_db_pool, character.guid, request.quest)
        .await?;
    session.quest_statuses.insert(request.quest, status.clone());
    let Some(slot) = quest_log_slot_for_quest(session, request.quest) else {
        warn!(quest = request.quest, "Accepted quest but no quest-log slot was available");
        return Ok(());
    };
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_quest_log_update_body(character.guid, slot, &status)?,
        Some(header_crypto),
    )
    .await
}

async fn handle_questgiver_complete_quest(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    body: &[u8],
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let request = QuestgiverQuestRequest::read(body, "CMSG_QUESTGIVER_COMPLETE_QUEST")?;
    if !request.guid.is_creature()
        || !wow_db::creature_completes_quest(world_db_pool, request.guid.entry(), request.quest)
            .await?
    {
        return Ok(());
    }
    let Some(status) = session.quest_statuses.get(&request.quest) else {
        return Ok(());
    };
    let Some(quest) = wow_db::get_quest_template_query(world_db_pool, request.quest).await? else {
        return Ok(());
    };
    if status.status == QUEST_STATUS_COMPLETE {
        let response = build_quest_offer_reward_body(request.guid, &quest);
        send_packet(
            stream,
            SMSG_QUESTGIVER_OFFER_REWARD,
            &response,
            Some(header_crypto),
        )
        .await
    } else {
        let response = build_quest_request_items_body(request.guid, &quest, false);
        send_packet(
            stream,
            SMSG_QUESTGIVER_REQUEST_ITEMS,
            &response,
            Some(header_crypto),
        )
        .await
    }
}

async fn handle_questgiver_choose_reward(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        warn!("Ignoring quest reward before character login");
        return Ok(());
    };
    let character_guid = character.guid;
    let character_level = character.level;
    let request = QuestRewardRequest::read(body)?;
    if request.reward >= 6 {
        warn!(quest = request.quest, reward = request.reward, "Ignoring invalid quest reward choice");
        return Ok(());
    }
    if !request.guid.is_creature()
        || !wow_db::creature_completes_quest(world_db_pool, request.guid.entry(), request.quest)
            .await?
    {
        warn!(quest = request.quest, "Ignoring reward request for invalid giver");
        return Ok(());
    }
    let Some(quest) = wow_db::get_quest_template_query(world_db_pool, request.quest).await? else {
        return Ok(());
    };
    let reward_money = quest.rew_or_req_money.max(0) as u32;
    let reward_xp = quest_xp_reward(character_level, &quest);
    let slot = quest_log_slot_for_quest(session, request.quest);
    let Some(new_money) =
        wow_db::reward_character_quest(character_db_pool, character_guid, request.quest, reward_money)
            .await?
    else {
        return Ok(());
    };
    if let Some(status) = session.quest_statuses.get_mut(&request.quest) {
        status.status = QUEST_STATUS_COMPLETE;
        status.rewarded = 1;
    }
    send_packet(
        stream,
        SMSG_QUESTGIVER_QUEST_COMPLETE,
        &build_questgiver_quest_complete_body_with_xp(request.quest, reward_xp, reward_money),
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_money_update_body(character_guid, new_money)?,
        Some(&mut *header_crypto),
    )
    .await?;
    award_character_xp(
        stream,
        character_db_pool,
        world_db_pool,
        session,
        None,
        reward_xp,
        header_crypto,
    )
    .await?;
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_quest_log_clear_body(character_guid, slot.unwrap_or(0))?,
        Some(header_crypto),
    )
    .await
}

#[derive(Debug, Clone, Copy)]
struct QuestgiverQuestRequest {
    guid: ObjectGuid,
    quest: u32,
}

impl QuestgiverQuestRequest {
    fn read(body: &[u8], packet_name: &str) -> anyhow::Result<Self> {
        if body.len() < 12 {
            anyhow::bail!("{packet_name} payload too short: {} bytes", body.len());
        }
        Ok(Self {
            guid: ObjectGuid::from_raw(u64::from_le_bytes(body[0..8].try_into()?)),
            quest: u32::from_le_bytes(body[8..12].try_into()?),
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct QuestRewardRequest {
    guid: ObjectGuid,
    quest: u32,
    reward: u32,
}

impl QuestRewardRequest {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        if body.len() < 16 {
            anyhow::bail!(
                "CMSG_QUESTGIVER_CHOOSE_REWARD payload too short: {} bytes",
                body.len()
            );
        }
        Ok(Self {
            guid: ObjectGuid::from_raw(u64::from_le_bytes(body[0..8].try_into()?)),
            quest: u32::from_le_bytes(body[8..12].try_into()?),
            reward: u32::from_le_bytes(body[12..16].try_into()?),
        })
    }
}

async fn questgiver_dialog_status(
    world_db_pool: &MySqlPool,
    guid: ObjectGuid,
    session: &WorldSessionState,
) -> anyhow::Result<u32> {
    if !guid.is_creature() {
        return Ok(DIALOG_STATUS_NONE);
    }
    for quest in wow_db::get_creature_start_quests(world_db_pool, guid.entry()).await? {
        match session.quest_statuses.get(&quest.entry) {
            Some(status) if status.rewarded != 0 => {}
            Some(status) if status.status == QUEST_STATUS_COMPLETE => {
                return Ok(DIALOG_STATUS_REWARD2);
            }
            Some(_) => return Ok(DIALOG_STATUS_INCOMPLETE),
            None => return Ok(DIALOG_STATUS_AVAILABLE),
        }
    }
    for status in session.quest_statuses.values() {
        if status.rewarded == 0
            && status.status == QUEST_STATUS_COMPLETE
            && wow_db::creature_completes_quest(world_db_pool, guid.entry(), status.quest).await?
        {
            return Ok(DIALOG_STATUS_REWARD2);
        }
    }
    Ok(DIALOG_STATUS_NONE)
}

async fn questgiver_visible_quests(
    world_db_pool: &MySqlPool,
    guid: ObjectGuid,
    session: &WorldSessionState,
) -> anyhow::Result<Vec<QuestTemplateQuery>> {
    if !guid.is_creature() {
        return Ok(Vec::new());
    }
    let quests = wow_db::get_creature_start_quests(world_db_pool, guid.entry()).await?;
    Ok(quests
        .into_iter()
        .filter(|quest| {
            session
                .quest_statuses
                .get(&quest.entry)
                .is_none_or(|status| status.rewarded == 0 && status.status != QUEST_STATUS_COMPLETE)
        })
        .collect())
}

async fn questgiver_completed_turnin_quest(
    world_db_pool: &MySqlPool,
    guid: ObjectGuid,
    session: &WorldSessionState,
) -> anyhow::Result<Option<QuestTemplateQuery>> {
    if !guid.is_creature() {
        return Ok(None);
    }

    for status in active_quest_statuses_sorted(&session.quest_statuses) {
        if status.status == QUEST_STATUS_COMPLETE
            && wow_db::creature_completes_quest(world_db_pool, guid.entry(), status.quest).await?
        {
            return Ok(wow_db::get_quest_template_query(world_db_pool, status.quest).await?);
        }
    }

    Ok(None)
}

fn active_quest_statuses_sorted(
    quest_statuses: &HashMap<u32, CharacterQuestStatus>,
) -> Vec<&CharacterQuestStatus> {
    let mut statuses: Vec<_> = quest_statuses
        .values()
        .filter(|status| status.rewarded == 0)
        .collect();
    statuses.sort_by_key(|status| status.quest);
    statuses
}

fn quest_log_slot_for_quest(session: &WorldSessionState, quest: u32) -> Option<usize> {
    active_quest_statuses_sorted(&session.quest_statuses)
        .into_iter()
        .take(MAX_QUEST_LOG_SIZE)
        .position(|status| status.quest == quest)
}

async fn grant_db_creature_kill_credit(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    session: &mut WorldSessionState,
    killed_guid: ObjectGuid,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        return Ok(());
    };
    let Some(creature) = session.db_creatures.get(&killed_guid.raw()) else {
        return Ok(());
    };
    let killed_entry = creature.spawn.entry;
    let active_quests: Vec<u32> = session
        .quest_statuses
        .values()
        .filter(|status| status.status == QUEST_STATUS_INCOMPLETE && status.rewarded == 0)
        .map(|status| status.quest)
        .collect();
    for quest_id in active_quests {
        let Some(quest) = wow_db::get_quest_template_query(world_db_pool, quest_id).await? else {
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
        let complete = new_count >= required;
        let status = wow_db::update_character_quest_mob_count(
            character_db_pool,
            character.guid,
            quest_id,
            index,
            new_count,
            complete,
        )
        .await?;
        session.quest_statuses.insert(quest_id, status.clone());
        let Some(slot) = quest_log_slot_for_quest(session, quest_id) else {
            warn!(quest = quest_id, "Quest progress updated but no quest-log slot was available");
            continue;
        };
        send_packet(
            stream,
            SMSG_QUESTUPDATE_ADD_KILL,
            &build_quest_update_add_kill_body(&quest, killed_guid, index, new_count),
            Some(&mut *header_crypto),
        )
        .await?;
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &build_player_quest_log_update_body(character.guid, slot, &status)?,
            Some(&mut *header_crypto),
        )
        .await?;
        if complete {
            send_packet(
                stream,
                SMSG_QUESTUPDATE_COMPLETE,
                &quest_id.to_le_bytes(),
                Some(&mut *header_crypto),
            )
            .await?;
        }
    }
    Ok(())
}

