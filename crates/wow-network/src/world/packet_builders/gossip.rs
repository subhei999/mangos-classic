use super::*;
use wow_proto::{
    GossipOption, ServerWorldPacket, SmsgGossipMessageResponse, SmsgNpcTextUpdateResponse,
};

// CMaNGOS reference: src/game/Handlers/GossipDef.cpp gossip packet builders.

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
    SmsgGossipMessageResponse {
        guid,
        text_id,
        options: options
            .iter()
            .map(|(option_index, option_icon, option_text)| GossipOption {
                option_index: *option_index,
                icon: *option_icon,
                coded: 0,
                text: (*option_text).to_string(),
            })
            .collect(),
        quest_option_count: 0,
    }
    .body()
}

pub(in crate::world) fn build_npc_text_update(text_id: u32, primary_text: &str) -> Vec<u8> {
    SmsgNpcTextUpdateResponse {
        text_id,
        primary_text: primary_text.to_string(),
    }
    .body()
}
