use super::*;
use wow_proto::{
    GossipOption, ServerWorldPacket, SmsgGossipMessageResponse, SmsgNpcTextUpdateResponse,
};

// CMaNGOS reference: src/game/Handlers/GossipDef.cpp gossip packet builders.

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::world) struct GossipMessageOption {
    pub(in crate::world) option_index: u32,
    pub(in crate::world) icon: u8,
    pub(in crate::world) coded: u8,
    pub(in crate::world) text: String,
}

#[cfg(test)]
pub(in crate::world) fn build_rust_guide_gossip_message() -> Vec<u8> {
    build_gossip_message(
        rust_guide_guid(),
        RUST_GUIDE_GOSSIP_TEXT_ID,
        &[(0, GOSSIP_ICON_CHAT, RUST_GUIDE_GOSSIP_OPTION)],
    )
}

#[cfg(test)]
pub(in crate::world) fn build_rust_guide_npc_text_update(text_id: u32) -> Vec<u8> {
    build_npc_text_update(text_id, RUST_GUIDE_GOSSIP_TEXT)
}

pub(in crate::world) fn build_gossip_message(
    guid: ObjectGuid,
    text_id: u32,
    options: &[(u32, u8, &str)],
) -> Vec<u8> {
    build_gossip_message_with_quests(guid, text_id, options, &[])
}

pub(in crate::world) fn build_gossip_message_with_quests(
    guid: ObjectGuid,
    text_id: u32,
    options: &[(u32, u8, &str)],
    quests: &[QuestListItem],
) -> Vec<u8> {
    let options = tuple_gossip_options(options);
    build_gossip_message_from_options_with_quests(guid, text_id, &options, quests)
}

pub(in crate::world) fn build_gossip_message_from_options_with_quests(
    guid: ObjectGuid,
    text_id: u32,
    options: &[GossipMessageOption],
    quests: &[QuestListItem],
) -> Vec<u8> {
    SmsgGossipMessageResponse {
        guid,
        text_id,
        options: options
            .iter()
            .map(|option| GossipOption {
                option_index: option.option_index,
                icon: option.icon,
                coded: option.coded,
                text: option.text.clone(),
            })
            .collect(),
        quest_option_count: quests.len() as u32,
    }
    .body_with_gossip_quests(quests)
}

pub(in crate::world) fn tuple_gossip_options(
    options: &[(u32, u8, &str)],
) -> Vec<GossipMessageOption> {
    options
        .iter()
        .map(
            |(option_index, option_icon, option_text)| GossipMessageOption {
                option_index: *option_index,
                icon: *option_icon,
                coded: 0,
                text: (*option_text).to_string(),
            },
        )
        .collect()
}

pub(in crate::world) fn build_npc_text_update(text_id: u32, primary_text: &str) -> Vec<u8> {
    SmsgNpcTextUpdateResponse {
        text_id,
        primary_text: primary_text.to_string(),
    }
    .body()
}

trait GossipMessageQuestBody {
    fn body_with_gossip_quests(&self, quests: &[QuestListItem]) -> Vec<u8>;
}

impl GossipMessageQuestBody for SmsgGossipMessageResponse {
    fn body_with_gossip_quests(&self, quests: &[QuestListItem]) -> Vec<u8> {
        let mut body = self.body();
        if quests.is_empty() {
            return body;
        }
        body.truncate(body.len().saturating_sub(4));
        body.extend_from_slice(&(quests.len() as u32).to_le_bytes());
        for quest in quests {
            body.extend_from_slice(&quest.quest.entry.to_le_bytes());
            body.extend_from_slice(&quest.dialog_status.to_le_bytes());
            body.extend_from_slice(&quest.quest.quest_level.to_le_bytes());
            body.extend_from_slice(quest.quest.title.as_bytes());
            body.push(0);
        }
        body
    }
}
