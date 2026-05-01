// CMaNGOS reference: src/game/Handlers/QuestHandler.cpp quest packet builders.

fn build_questgiver_status_body(guid: ObjectGuid, status: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(12);

    body.extend_from_slice(&guid.raw().to_le_bytes());

    body.extend_from_slice(&status.to_le_bytes());

    body
}

#[derive(Debug, Clone)]
struct QuestListItem {
    quest: QuestTemplateQuery,
    dialog_status: u32,
}

fn build_questgiver_quest_list_body(guid: ObjectGuid, quests: &[QuestListItem]) -> Vec<u8> {
    let mut body = Vec::with_capacity(64 + quests.len() * 24);

    body.extend_from_slice(&guid.raw().to_le_bytes());

    push_cstring(&mut body, "Greetings.");

    body.extend_from_slice(&0u32.to_le_bytes()); // player emote delay

    body.extend_from_slice(&0u32.to_le_bytes()); // NPC emote

    body.push(quests.len().min(u8::MAX as usize) as u8);

    for item in quests.iter().take(u8::MAX as usize) {
        let quest = &item.quest;

        body.extend_from_slice(&quest.entry.to_le_bytes());

        body.extend_from_slice(&item.dialog_status.to_le_bytes());

        body.extend_from_slice(&quest.quest_level.to_le_bytes());

        push_cstring(&mut body, &quest.title);
    }

    body
}

fn build_quest_details_body(guid: ObjectGuid, quest: &QuestTemplateQuery) -> Vec<u8> {
    let mut body = Vec::with_capacity(256);

    body.extend_from_slice(&guid.raw().to_le_bytes());

    body.extend_from_slice(&quest.entry.to_le_bytes());

    push_cstring(&mut body, &quest.title);

    push_cstring(&mut body, &quest.details);

    push_cstring(&mut body, &quest.objectives);

    body.extend_from_slice(&1u32.to_le_bytes()); // activate accept

    write_quest_reward_items(
        &mut body,
        &quest.rew_choice_item_id,
        &quest.rew_choice_item_count,
    );

    write_quest_reward_items(&mut body, &quest.rew_item_id, &quest.rew_item_count);

    body.extend_from_slice(&(quest.rew_or_req_money.max(0) as u32).to_le_bytes());

    body.extend_from_slice(&quest.rew_spell.to_le_bytes());

    let emote_count = quest
        .details_emote
        .iter()
        .take_while(|emote| **emote != 0)
        .count();

    body.extend_from_slice(&(emote_count as u32).to_le_bytes());

    for index in 0..emote_count {
        body.extend_from_slice(&quest.details_emote[index].to_le_bytes());

        body.extend_from_slice(&quest.details_emote_delay[index].to_le_bytes());
    }

    body
}

fn build_quest_query_response_body(quest: &QuestTemplateQuery) -> Vec<u8> {
    let mut body = Vec::with_capacity(512);

    body.extend_from_slice(&quest.entry.to_le_bytes());

    body.extend_from_slice(&quest.method.to_le_bytes());

    body.extend_from_slice(&quest.quest_level.to_le_bytes());

    body.extend_from_slice(&(quest.zone_or_sort as i32 as u32).to_le_bytes());

    body.extend_from_slice(&quest.quest_type.to_le_bytes());

    body.extend_from_slice(&quest.rep_objective_faction.to_le_bytes());

    body.extend_from_slice(&(quest.rep_objective_value as u32).to_le_bytes());

    body.extend_from_slice(&0u32.to_le_bytes());

    body.extend_from_slice(&0u32.to_le_bytes());

    body.extend_from_slice(&quest.next_quest_in_chain.to_le_bytes());

    body.extend_from_slice(&(quest.rew_or_req_money.max(0) as u32).to_le_bytes());

    body.extend_from_slice(&quest.rew_money_max_level.to_le_bytes());

    body.extend_from_slice(&quest.rew_spell.to_le_bytes());

    body.extend_from_slice(&quest.src_item_id.to_le_bytes());

    body.extend_from_slice(&quest.quest_flags.to_le_bytes());

    for index in 0..4 {
        body.extend_from_slice(&quest.rew_item_id[index].to_le_bytes());

        body.extend_from_slice(&quest.rew_item_count[index].to_le_bytes());
    }

    for index in 0..6 {
        body.extend_from_slice(&quest.rew_choice_item_id[index].to_le_bytes());

        body.extend_from_slice(&quest.rew_choice_item_count[index].to_le_bytes());
    }

    body.extend_from_slice(&quest.point_map_id.to_le_bytes());

    body.extend_from_slice(&quest.point_x.to_le_bytes());

    body.extend_from_slice(&quest.point_y.to_le_bytes());

    body.extend_from_slice(&quest.point_opt.to_le_bytes());

    push_cstring(&mut body, &quest.title);

    push_cstring(&mut body, &quest.objectives);

    push_cstring(&mut body, &quest.details);

    push_cstring(&mut body, &quest.end_text);

    for index in 0..4 {
        let entry = quest.req_creature_or_go_id[index];

        let wire_entry = if entry < 0 {
            ((-entry) as u32) | 0x8000_0000
        } else {
            entry as u32
        };

        body.extend_from_slice(&wire_entry.to_le_bytes());

        body.extend_from_slice(&quest.req_creature_or_go_count[index].to_le_bytes());

        body.extend_from_slice(&quest.req_item_id[index].to_le_bytes());

        body.extend_from_slice(&quest.req_item_count[index].to_le_bytes());
    }

    for text in &quest.objective_text {
        push_cstring(&mut body, text);
    }

    body
}

fn build_quest_request_items_body(
    guid: ObjectGuid,

    quest: &QuestTemplateQuery,

    complete: bool,
) -> Vec<u8> {
    let mut body = Vec::with_capacity(128);

    body.extend_from_slice(&guid.raw().to_le_bytes());

    body.extend_from_slice(&quest.entry.to_le_bytes());

    push_cstring(&mut body, &quest.title);

    push_cstring(&mut body, &quest.request_items_text);

    let (delay, emote) = if complete {
        (quest.complete_emote_delay, quest.complete_emote)
    } else {
        (quest.incomplete_emote_delay, quest.incomplete_emote)
    };

    body.extend_from_slice(&delay.to_le_bytes());

    body.extend_from_slice(&emote.to_le_bytes());

    body.extend_from_slice(&0u32.to_le_bytes()); // close on cancel

    body.extend_from_slice(&0u32.to_le_bytes()); // required money

    body.extend_from_slice(&0u32.to_le_bytes()); // required item count

    body.extend_from_slice(&2u32.to_le_bytes());

    body.extend_from_slice(&(if complete { 3u32 } else { 0u32 }).to_le_bytes());

    body.extend_from_slice(&4u32.to_le_bytes());

    body.extend_from_slice(&8u32.to_le_bytes());

    body
}

fn build_quest_offer_reward_body(guid: ObjectGuid, quest: &QuestTemplateQuery) -> Vec<u8> {
    let mut body = Vec::with_capacity(192);

    body.extend_from_slice(&guid.raw().to_le_bytes());

    body.extend_from_slice(&quest.entry.to_le_bytes());

    push_cstring(&mut body, &quest.title);

    push_cstring(&mut body, &quest.offer_reward_text);

    body.extend_from_slice(&1u32.to_le_bytes()); // enable next

    let emote_count = quest
        .offer_reward_emote
        .iter()
        .take_while(|emote| **emote != 0)
        .count();

    body.extend_from_slice(&(emote_count as u32).to_le_bytes());

    for index in 0..emote_count {
        body.extend_from_slice(&quest.offer_reward_emote_delay[index].to_le_bytes());

        body.extend_from_slice(&quest.offer_reward_emote[index].to_le_bytes());
    }

    write_quest_reward_items(
        &mut body,
        &quest.rew_choice_item_id,
        &quest.rew_choice_item_count,
    );

    write_quest_reward_items(&mut body, &quest.rew_item_id, &quest.rew_item_count);

    body.extend_from_slice(&(quest.rew_or_req_money.max(0) as u32).to_le_bytes());

    body.extend_from_slice(&quest.rew_spell.to_le_bytes());

    body.extend_from_slice(&quest.rew_spell_cast.to_le_bytes());

    body
}

fn build_questgiver_quest_complete_body_with_xp(
    quest: u32,

    reward_xp: u32,

    reward_money: u32,
) -> Vec<u8> {
    let mut body = Vec::with_capacity(16);

    body.extend_from_slice(&quest.to_le_bytes());

    body.extend_from_slice(&reward_xp.to_le_bytes());

    body.extend_from_slice(&0u32.to_le_bytes());

    body.extend_from_slice(&reward_money.to_le_bytes());

    body
}

fn build_quest_update_add_kill_body(
    quest: &QuestTemplateQuery,

    killed_guid: ObjectGuid,

    objective_index: usize,

    count: u32,
) -> Vec<u8> {
    let mut body = Vec::with_capacity(24);

    body.extend_from_slice(&quest.entry.to_le_bytes());

    body.extend_from_slice(&(quest.req_creature_or_go_id[objective_index] as u32).to_le_bytes());

    body.extend_from_slice(&count.to_le_bytes());

    body.extend_from_slice(&quest.req_creature_or_go_count[objective_index].to_le_bytes());

    body.extend_from_slice(&killed_guid.raw().to_le_bytes());

    body
}

fn build_player_quest_log_update_body(
    character_guid: u32,

    slot: usize,

    status: &CharacterQuestStatus,
) -> anyhow::Result<Vec<u8>> {
    let player = ObjectGuid::new(HighGuid::Player, 0, character_guid);

    let mut block = Vec::new();

    block.push(UPDATE_TYPE_VALUES);

    PackedGuid::write(&mut block, player)?;

    let mut values = vec![None; PLAYER_END_FIELDS];

    let base = PLAYER_QUEST_LOG_1_1 + slot * MAX_QUEST_OFFSET;

    set_update_value(&mut values, base + QUEST_LOG_QUEST_ID_OFFSET, status.quest)?;

    set_update_value(
        &mut values,
        base + QUEST_LOG_COUNT_STATE_OFFSET,
        quest_log_count_state(status),
    )?;

    set_update_value(&mut values, base + QUEST_LOG_TIME_OFFSET, 0)?;

    write_update_values(&mut block, &values)?;

    Ok(build_update_object_body(&[block]))
}

fn build_player_quest_log_clear_body(character_guid: u32, slot: usize) -> anyhow::Result<Vec<u8>> {
    let player = ObjectGuid::new(HighGuid::Player, 0, character_guid);

    let mut block = Vec::new();

    block.push(UPDATE_TYPE_VALUES);

    PackedGuid::write(&mut block, player)?;

    let mut values = vec![None; PLAYER_END_FIELDS];

    let base = PLAYER_QUEST_LOG_1_1 + slot * MAX_QUEST_OFFSET;

    set_update_value(&mut values, base + QUEST_LOG_QUEST_ID_OFFSET, 0)?;

    set_update_value(&mut values, base + QUEST_LOG_COUNT_STATE_OFFSET, 0)?;

    set_update_value(&mut values, base + QUEST_LOG_TIME_OFFSET, 0)?;

    write_update_values(&mut block, &values)?;

    Ok(build_update_object_body(&[block]))
}

fn quest_log_count_state(status: &CharacterQuestStatus) -> u32 {
    let count = status.mobcount1 & 0x3F;

    let complete = if status.status == QUEST_STATUS_COMPLETE {
        QUEST_STATE_COMPLETE << 24
    } else {
        0
    };

    count | complete
}

fn write_quest_reward_items<const N: usize>(body: &mut Vec<u8>, ids: &[u32; N], counts: &[u32; N]) {
    let non_zero: Vec<_> = ids
        .iter()
        .zip(counts.iter())
        .filter(|(id, count)| **id != 0 && **count != 0)
        .collect();

    body.extend_from_slice(&(non_zero.len() as u32).to_le_bytes());

    for (id, count) in non_zero {
        body.extend_from_slice(&id.to_le_bytes());

        body.extend_from_slice(&(*count).to_le_bytes());

        body.extend_from_slice(&0u32.to_le_bytes());
    }
}
