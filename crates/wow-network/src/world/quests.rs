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
    let Some(character_guid) = session.active_character.as_ref().map(|character| character.guid) else {
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
    let Some(quest) = wow_db::get_quest_template_query(world_db_pool, request.quest).await? else {
        return Ok(());
    };
    if !can_take_start_quest(world_db_pool, &quest, session).await? {
        warn!(
            quest = request.quest,
            "Ignoring quest accept that does not satisfy CMaNGOS-style eligibility"
        );
        return Ok(());
    }
    let mut status = wow_db::accept_character_quest(character_db_pool, character_guid, request.quest)
        .await?;
    session.quest_statuses.insert(request.quest, status.clone());
    let source_item = grant_quest_source_item_if_needed(
        character_db_pool,
        world_db_pool,
        character_guid,
        &quest,
        session,
    )
    .await?;
    if quest_can_complete_from_inventory(&quest, &session.inventory) {
        status = wow_db::complete_character_quest(character_db_pool, character_guid, request.quest)
            .await?;
        session.quest_statuses.insert(request.quest, status.clone());
    }
    let Some(slot) = quest_log_slot_for_quest(session, request.quest) else {
        warn!(quest = request.quest, "Accepted quest but no quest-log slot was available");
        return Ok(());
    };
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_quest_log_update_body(character_guid, slot, &status)?,
        Some(&mut *header_crypto),
    )
    .await?;
    if let Some(item) = source_item {
        let owner_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
        let container_slots =
            if let Some(template) = wow_db::get_item_template_query(world_db_pool, item.item_template).await? {
                (template.container_slots > 0).then_some(template.container_slots)
            } else {
                None
            };
        let create_body = build_update_object_body(&[build_item_create_update_block(
            owner_guid,
            owner_guid,
            &item,
            container_slots,
        )?]);
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &create_body,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    if status.status == QUEST_STATUS_COMPLETE {
        send_packet(
            stream,
            SMSG_QUESTUPDATE_COMPLETE,
            &request.quest.to_le_bytes(),
            Some(header_crypto),
        )
        .await?;
    }
    Ok(())
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

async fn handle_questlog_remove_quest(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character_guid) = session.active_character.as_ref().map(|character| character.guid) else {
        warn!("Ignoring quest abandon before character login");
        return Ok(());
    };
    let Some(slot) = body.first().copied().map(usize::from) else {
        anyhow::bail!("CMSG_QUESTLOG_REMOVE_QUEST payload too short: {} bytes", body.len());
    };
    if slot >= MAX_QUEST_LOG_SIZE {
        return Ok(());
    }
    let Some(status) = active_quest_statuses_sorted(&session.quest_statuses)
        .into_iter()
        .nth(slot)
        .cloned()
    else {
        return Ok(());
    };
    if wow_db::abandon_character_quest(character_db_pool, character_guid, status.quest)
        .await?
        .is_some()
    {
        if let Some(local_status) = session.quest_statuses.get_mut(&status.quest) {
            local_status.status = 0;
            local_status.rewarded = 0;
            local_status.mobcount1 = 0;
            local_status.mobcount2 = 0;
            local_status.mobcount3 = 0;
            local_status.mobcount4 = 0;
        }
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &build_player_quest_log_clear_body(character_guid, slot)?,
            Some(header_crypto),
        )
        .await?;
    }
    Ok(())
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

    let mut dialog_status = DIALOG_STATUS_NONE;
    for quest in wow_db::get_creature_complete_quests(world_db_pool, guid.entry()).await? {
        let Some(status) = session.quest_statuses.get(&quest.entry) else {
            continue;
        };
        if status.rewarded != 0 {
            continue;
        }
        if status.status == QUEST_STATUS_COMPLETE {
            dialog_status = dialog_status.max(DIALOG_STATUS_REWARD2);
        } else if status.status == QUEST_STATUS_INCOMPLETE {
            dialog_status = dialog_status.max(DIALOG_STATUS_INCOMPLETE);
        }
    }
    for quest in wow_db::get_creature_start_quests(world_db_pool, guid.entry()).await? {
        if can_take_start_quest(world_db_pool, &quest, session).await? {
            dialog_status = dialog_status.max(DIALOG_STATUS_AVAILABLE);
        }
    }

    for status in session.quest_statuses.values() {
        if status.rewarded == 0
            && status.status == QUEST_STATUS_COMPLETE
            && wow_db::creature_completes_quest(world_db_pool, guid.entry(), status.quest).await?
        {
            dialog_status = dialog_status.max(DIALOG_STATUS_REWARD2);
        }
    }
    Ok(dialog_status)
}

async fn questgiver_visible_quests(
    world_db_pool: &MySqlPool,
    guid: ObjectGuid,
    session: &WorldSessionState,
) -> anyhow::Result<Vec<QuestListItem>> {
    if !guid.is_creature() {
        return Ok(Vec::new());
    }
    let mut visible = Vec::new();
    let mut seen = HashSet::new();
    for quest in wow_db::get_creature_complete_quests(world_db_pool, guid.entry()).await? {
        let Some(status) = session.quest_statuses.get(&quest.entry) else {
            continue;
        };
        if status.rewarded != 0 {
            continue;
        }
        let dialog_status = if status.status == QUEST_STATUS_COMPLETE {
            DIALOG_STATUS_REWARD2
        } else if status.status == QUEST_STATUS_INCOMPLETE {
            DIALOG_STATUS_INCOMPLETE
        } else {
            continue;
        };
        seen.insert(quest.entry);
        visible.push(QuestListItem { quest, dialog_status });
    }
    let quests = wow_db::get_creature_start_quests(world_db_pool, guid.entry()).await?;
    for quest in quests {
        if seen.contains(&quest.entry) {
            continue;
        }
        if can_take_start_quest(world_db_pool, &quest, session).await? {
            visible.push(QuestListItem {
                quest,
                dialog_status: DIALOG_STATUS_AVAILABLE,
            });
        }
    }
    Ok(visible)
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
        .filter(|status| quest_status_is_current(status))
        .collect();
    statuses.sort_by_key(|status| status.quest);
    statuses
}

async fn grant_quest_source_item_if_needed(
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    character_guid: u32,
    quest: &QuestTemplateQuery,
    session: &mut WorldSessionState,
) -> anyhow::Result<Option<CharacterInventoryItem>> {
    if quest.src_item_id == 0 {
        return Ok(None);
    }
    let required_count = quest.src_item_count.max(1);
    let current_count = session
        .inventory
        .iter()
        .filter(|item| item.item_template == quest.src_item_id)
        .map(|item| item.count)
        .sum::<u32>();
    if current_count >= required_count {
        return Ok(None);
    }
    let Some(slot) = first_empty_backpack_slot(&session.inventory) else {
        warn!(
            quest = quest.entry,
            item = quest.src_item_id,
            "Cannot grant quest source item because backpack is full"
        );
        return Ok(None);
    };
    let Some(template) = wow_db::get_item_template_query(world_db_pool, quest.src_item_id).await?
    else {
        warn!(
            quest = quest.entry,
            item = quest.src_item_id,
            "Cannot grant missing quest source item template"
        );
        return Ok(None);
    };
    let item = wow_db::add_character_inventory_item(
        character_db_pool,
        character_guid,
        INVENTORY_SLOT_BAG_0 as u32,
        slot,
        quest.src_item_id,
        required_count - current_count,
        template.max_durability,
    )
    .await?;
    session.inventory = wow_db::get_character_inventory_items(character_db_pool, character_guid)
        .await?;
    Ok(Some(item))
}

fn quest_can_complete_from_inventory(
    quest: &QuestTemplateQuery,
    inventory: &[CharacterInventoryItem],
) -> bool {
    const QUEST_SPECIAL_FLAG_EXPLORATION_OR_EVENT: u32 = 0x002;
    if (quest.special_flags & QUEST_SPECIAL_FLAG_EXPLORATION_OR_EVENT) != 0
        || quest.rep_objective_faction != 0
    {
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

    for (item_id, required_count) in quest.req_item_id.iter().zip(quest.req_item_count.iter()) {
        if *item_id == 0 || *required_count == 0 {
            continue;
        }
        let current_count = inventory
            .iter()
            .filter(|item| item.item_template == *item_id)
            .map(|item| item.count)
            .sum::<u32>();
        if current_count < *required_count {
            return false;
        }
    }

    true
}

fn can_quest_be_started_from_status(quest: &QuestTemplateQuery, status: Option<&CharacterQuestStatus>) -> bool {
    status.is_none_or(|state| {
        state.status == 0 || (quest.is_repeatable() && state.rewarded != 0 && state.status == QUEST_STATUS_COMPLETE)
    })
}

fn quest_status_is_current(status: &CharacterQuestStatus) -> bool {
    status.rewarded == 0 && (status.status == QUEST_STATUS_INCOMPLETE || status.status == QUEST_STATUS_COMPLETE)
}

fn quest_is_current(quest_statuses: &HashMap<u32, CharacterQuestStatus>, quest: u32) -> bool {
    quest_statuses.get(&quest).is_some_and(quest_status_is_current)
}

fn quest_race_or_class_mask(id: u8) -> u32 {
    if id == 0 {
        return 0;
    }
    1u32.checked_shl(u32::from(id - 1)).unwrap_or(0)
}

fn satisfies_race_class_level(quest: &QuestTemplateQuery, character: &ActiveCharacter) -> bool {
    if character.level < quest.min_level || character.level > quest.max_level {
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

fn satisfies_prev_quest_requirement(
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

fn satisfies_exclusive_group(
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
        if quest_statuses.get(other_quest).is_some_and(quest_status_is_current) {
            return false;
        }
    }

    true
}

async fn can_take_start_quest(
    world_db_pool: &MySqlPool,
    quest: &QuestTemplateQuery,
    session: &WorldSessionState,
) -> anyhow::Result<bool> {
    let Some(character) = session.active_character.as_ref() else {
        return Ok(false);
    };
    if !satisfies_race_class_level(quest, character) {
        return Ok(false);
    }
    if !can_quest_be_started_from_status(quest, session.quest_statuses.get(&quest.entry)) {
        return Ok(false);
    }

    let prev_quests = wow_db::get_quest_prev_quests(world_db_pool, quest.entry).await?;
    if !prev_quests
        .into_iter()
        .all(|prev| satisfies_prev_quest_requirement(&session.quest_statuses, prev))
    {
        return Ok(false);
    }

    let prev_chain_quests = wow_db::get_quest_prev_chain_quests(world_db_pool, quest.entry).await?;
    if prev_chain_quests
        .into_iter()
        .any(|prev_chain| quest_is_current(&session.quest_statuses, prev_chain))
    {
        return Ok(false);
    }

    if quest.next_quest_in_chain != 0 && quest_is_current(&session.quest_statuses, quest.next_quest_in_chain)
    {
        return Ok(false);
    }

    let exclusive_group_quests =
        wow_db::get_exclusive_group_quests(world_db_pool, quest.exclusive_group).await?;
    if !satisfies_exclusive_group(quest, &exclusive_group_quests, &session.quest_statuses) {
        return Ok(false);
    }

    Ok(true)
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

