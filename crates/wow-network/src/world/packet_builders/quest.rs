use super::*;
use wow_proto::{
    QuestCompleteRewardItem, QuestDetailsEmote, QuestListResponseItem, QuestObjectiveRequirement,
    QuestOfferRewardEmote, QuestPoint, QuestRewardItem, ServerWorldPacket, SmsgQuestQueryResponse,
    SmsgQuestUpdateAddKillResponse, SmsgQuestgiverOfferRewardResponse,
    SmsgQuestgiverQuestCompleteResponse, SmsgQuestgiverQuestDetailsResponse,
    SmsgQuestgiverQuestListResponse, SmsgQuestgiverRequestItemsResponse,
    SmsgQuestgiverStatusResponse,
};

// CMaNGOS reference: src/game/Handlers/QuestHandler.cpp quest packet builders.

pub(in crate::world) fn build_questgiver_status_body(guid: ObjectGuid, status: u32) -> Vec<u8> {
    SmsgQuestgiverStatusResponse { guid, status }.body()
}

#[derive(Debug, Clone)]
pub(in crate::world) struct QuestListItem {
    pub(in crate::world) quest: QuestTemplateQuery,
    pub(in crate::world) dialog_status: u32,
}

#[derive(Debug, Clone, Default)]
pub(in crate::world) struct QuestRewardItemDisplays {
    pub(in crate::world) choice: [u32; 6],
    pub(in crate::world) reward: [u32; 4],
    pub(in crate::world) required: [u32; 4],
}

pub(in crate::world) fn build_questgiver_quest_list_body(
    guid: ObjectGuid,
    quests: &[QuestListItem],
) -> Vec<u8> {
    SmsgQuestgiverQuestListResponse {
        guid,
        greeting: "Greetings.".to_string(),
        player_emote_delay: 0,
        npc_emote: 0,
        quests: quests
            .iter()
            .map(|item| QuestListResponseItem {
                quest_id: item.quest.entry,
                dialog_status: item.dialog_status,
                quest_level: item.quest.quest_level,
                title: item.quest.title.clone(),
            })
            .collect(),
    }
    .body()
}

pub(in crate::world) fn build_quest_details_body(
    guid: ObjectGuid,
    quest: &QuestTemplateQuery,
    displays: &QuestRewardItemDisplays,
) -> Vec<u8> {
    SmsgQuestgiverQuestDetailsResponse {
        guid,
        quest_id: quest.entry,
        title: quest.title.clone(),
        details: quest.details.clone(),
        objectives: quest.objectives.clone(),
        activate_accept: 1,
        choice_items: quest_reward_items(
            &quest.rew_choice_item_id,
            &quest.rew_choice_item_count,
            &displays.choice,
        ),
        reward_items: quest_reward_items(
            &quest.rew_item_id,
            &quest.rew_item_count,
            &displays.reward,
        ),
        reward_money: quest.rew_or_req_money.max(0) as u32,
        reward_spell: quest.rew_spell,
        emotes: quest
            .details_emote
            .iter()
            .zip(quest.details_emote_delay.iter())
            .take_while(|(emote, _)| **emote != 0)
            .map(|(emote, delay)| QuestDetailsEmote {
                emote: *emote,
                delay: *delay,
            })
            .collect(),
    }
    .body()
}

pub(in crate::world) fn build_quest_query_response_body(quest: &QuestTemplateQuery) -> Vec<u8> {
    SmsgQuestQueryResponse {
        quest_id: quest.entry,
        method: quest.method,
        quest_level: quest.quest_level,
        zone_or_sort: quest.zone_or_sort as i32 as u32,
        quest_type: quest.quest_type,
        rep_objective_faction: quest.rep_objective_faction,
        rep_objective_value: quest.rep_objective_value as u32,
        next_quest_in_chain: quest.next_quest_in_chain,
        reward_money: quest.rew_or_req_money.max(0) as u32,
        reward_money_max_level: quest.rew_money_max_level,
        reward_spell: quest.rew_spell,
        source_item_id: quest.src_item_id,
        quest_flags: quest.quest_flags,
        reward_items: quest.rew_item_id,
        reward_item_counts: quest.rew_item_count,
        choice_items: quest.rew_choice_item_id,
        choice_item_counts: quest.rew_choice_item_count,
        point: QuestPoint {
            map_id: quest.point_map_id,
            x: quest.point_x,
            y: quest.point_y,
            opt: quest.point_opt,
        },
        title: quest.title.clone(),
        objectives: quest.objectives.clone(),
        details: quest.details.clone(),
        end_text: quest.end_text.clone(),
        requirements: std::array::from_fn(|index| QuestObjectiveRequirement {
            wire_entry: quest_requirement_wire_entry(quest.req_creature_or_go_id[index]),
            required_count: quest.req_creature_or_go_count[index],
            item_id: quest.req_item_id[index],
            item_count: quest.req_item_count[index],
        }),
        objective_text: quest.objective_text.clone(),
    }
    .body()
}

pub(in crate::world) fn build_quest_request_items_body(
    guid: ObjectGuid,

    quest: &QuestTemplateQuery,

    displays: &QuestRewardItemDisplays,

    complete: bool,
) -> Vec<u8> {
    let (delay, emote) = if complete {
        (quest.complete_emote_delay, quest.complete_emote)
    } else {
        (quest.incomplete_emote_delay, quest.incomplete_emote)
    };

    SmsgQuestgiverRequestItemsResponse {
        guid,
        quest_id: quest.entry,
        title: quest.title.clone(),
        request_items_text: quest.request_items_text.clone(),
        emote_delay: delay,
        emote,
        close_on_cancel: 0,
        required_money: quest.rew_or_req_money.min(0).unsigned_abs(),
        required_items: quest_reward_items(
            &quest.req_item_id,
            &quest.req_item_count,
            &displays.required,
        ),
        required_reward_button: 2,
        complete_reward_button: if complete { 3 } else { 0 },
        incomplete_reward_button: 4,
        completion_style: 8,
    }
    .body()
}

pub(in crate::world) fn build_quest_offer_reward_body(
    guid: ObjectGuid,
    quest: &QuestTemplateQuery,
    displays: &QuestRewardItemDisplays,
) -> Vec<u8> {
    SmsgQuestgiverOfferRewardResponse {
        guid,
        quest_id: quest.entry,
        title: quest.title.clone(),
        offer_reward_text: quest.offer_reward_text.clone(),
        enable_next: 1,
        emotes: quest
            .offer_reward_emote
            .iter()
            .zip(quest.offer_reward_emote_delay.iter())
            .take_while(|(emote, _)| **emote != 0)
            .map(|(emote, delay)| QuestOfferRewardEmote {
                delay: *delay,
                emote: *emote,
            })
            .collect(),
        choice_items: quest_reward_items(
            &quest.rew_choice_item_id,
            &quest.rew_choice_item_count,
            &displays.choice,
        ),
        reward_items: quest_reward_items(
            &quest.rew_item_id,
            &quest.rew_item_count,
            &displays.reward,
        ),
        reward_money: quest.rew_or_req_money.max(0) as u32,
        reward_spell: quest.rew_spell,
        reward_spell_cast: quest.rew_spell_cast,
    }
    .body()
}

pub(in crate::world) fn build_questgiver_quest_complete_body_with_xp(
    quest: &QuestTemplateQuery,

    reward_xp: u32,

    reward_money: u32,
) -> Vec<u8> {
    let reward_items = quest
        .rew_item_id
        .iter()
        .zip(quest.rew_item_count.iter())
        .filter(|(id, _count)| **id != 0)
        .map(|(item_id, count)| QuestCompleteRewardItem {
            item_id: *item_id,
            count: *count,
        })
        .collect();

    SmsgQuestgiverQuestCompleteResponse {
        quest_id: quest.entry,
        completion_type: 3,
        reward_xp,
        reward_money,
        reward_items,
    }
    .body()
}

pub(in crate::world) fn build_quest_update_add_kill_body(
    quest: &QuestTemplateQuery,

    killed_guid: ObjectGuid,

    objective_index: usize,

    count: u32,
) -> Vec<u8> {
    let objective_entry = quest.req_creature_or_go_id[objective_index];
    SmsgQuestUpdateAddKillResponse {
        quest_id: quest.entry,
        objective: quest_requirement_wire_entry(objective_entry),
        count,
        required_count: quest.req_creature_or_go_count[objective_index],
        killed_guid,
    }
    .body()
}

pub(in crate::world) fn build_player_quest_log_update_body(
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

pub(in crate::world) fn build_player_quest_log_clear_body(
    character_guid: u32,
    slot: usize,
) -> anyhow::Result<Vec<u8>> {
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

#[cfg(test)]
pub(in crate::world) fn build_player_quest_log_refresh_body(
    character_guid: u32,
    statuses: &HashMap<u32, CharacterQuestStatus>,
    slots: &[u32; MAX_QUEST_LOG_SIZE],
) -> anyhow::Result<Vec<u8>> {
    let player = ObjectGuid::new(HighGuid::Player, 0, character_guid);

    let mut block = Vec::new();

    block.push(UPDATE_TYPE_VALUES);

    PackedGuid::write(&mut block, player)?;

    let mut values = vec![None; PLAYER_END_FIELDS];

    for (slot, quest) in slots.iter().enumerate().take(MAX_QUEST_LOG_SIZE) {
        let base = PLAYER_QUEST_LOG_1_1 + slot * MAX_QUEST_OFFSET;
        if let Some(status) = (*quest != 0)
            .then_some(*quest)
            .and_then(|quest| statuses.get(&quest))
            .filter(|status| quest_status_is_current(status))
        {
            set_update_value(&mut values, base + QUEST_LOG_QUEST_ID_OFFSET, status.quest)?;
            set_update_value(
                &mut values,
                base + QUEST_LOG_COUNT_STATE_OFFSET,
                quest_log_count_state(status),
            )?;
        } else {
            set_update_value(&mut values, base + QUEST_LOG_QUEST_ID_OFFSET, 0)?;
            set_update_value(&mut values, base + QUEST_LOG_COUNT_STATE_OFFSET, 0)?;
        }
        set_update_value(&mut values, base + QUEST_LOG_TIME_OFFSET, 0)?;
    }

    write_update_values(&mut block, &values)?;

    Ok(build_update_object_body(&[block]))
}

pub(in crate::world) fn quest_log_count_state(status: &CharacterQuestStatus) -> u32 {
    let count = (status.mobcount1 & 0x3F)
        | ((status.mobcount2 & 0x3F) << 6)
        | ((status.mobcount3 & 0x3F) << 12)
        | ((status.mobcount4 & 0x3F) << 18);

    let complete = if status.status == QUEST_STATUS_COMPLETE {
        QUEST_STATE_COMPLETE << 24
    } else {
        0
    };

    count | complete
}

pub(in crate::world) fn quest_reward_items<const N: usize>(
    ids: &[u32; N],
    counts: &[u32; N],
    displays: &[u32; N],
) -> Vec<QuestRewardItem> {
    ids.iter()
        .zip(counts.iter())
        .zip(displays.iter())
        .filter(|((id, count), _display)| **id != 0 && **count != 0)
        .map(|((item_id, count), display_id)| QuestRewardItem {
            item_id: *item_id,
            count: *count,
            display_id: *display_id,
        })
        .collect()
}

pub(in crate::world) fn quest_requirement_wire_entry(entry: i32) -> u32 {
    if entry < 0 {
        entry.unsigned_abs() | 0x8000_0000
    } else {
        entry as u32
    }
}
