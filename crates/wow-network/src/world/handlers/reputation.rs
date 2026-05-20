use super::*;

// CMaNGOS reference: src/game/Reputation/ReputationMgr.{h,cpp}
// Faction.dbc reference bridge: faction ID -> reputationListID. Replace this
// table with the DBC loader once broader reputation/faction work lands.

pub(in crate::world) const FACTION_FLAG_VISIBLE: i32 = 0x01;
pub(in crate::world) const REPUTATION_GAIN_RATE: f32 = 1.0;
pub(in crate::world) const REPUTATION_LOWLEVEL_QUEST_RATE: f32 = 0.2;

pub(in crate::world) fn reputation_list_slot_for_faction(faction: u32) -> Option<usize> {
    match faction {
        59 => Some(4),   // Thorium Brotherhood
        76 => Some(14),  // Orgrimmar
        54 => Some(18),  // Gnomeregan Exiles
        72 => Some(19),  // Stormwind
        47 => Some(20),  // Ironforge
        69 => Some(21),  // Darnassus
        609 => Some(36), // Cenarion Circle
        730 => Some(40), // Stormpike Guard
        729 => Some(41), // Frostwolf Clan
        849 => Some(45), // Silverwing Sentinels
        889 => Some(46), // Warsong Outriders
        909 => Some(50), // Darkmoon Faire
        270 => Some(51), // Zandalar Tribe
        510 => Some(52), // The Defilers
        509 => Some(53), // The League of Arathor
        910 => Some(54), // Brood of Nozdormu
        _ => None,
    }
}

pub(in crate::world) fn reputation_faction_name(faction: u32) -> Option<&'static str> {
    match faction {
        59 => Some("Thorium Brotherhood"),
        76 => Some("Orgrimmar"),
        54 => Some("Gnomeregan Exiles"),
        72 => Some("Stormwind"),
        47 => Some("Ironforge"),
        69 => Some("Darnassus"),
        609 => Some("Cenarion Circle"),
        730 => Some("Stormpike Guard"),
        729 => Some("Frostwolf Clan"),
        849 => Some("Silverwing Sentinels"),
        889 => Some("Warsong Outriders"),
        909 => Some("Darkmoon Faire"),
        270 => Some("Zandalar Tribe"),
        510 => Some("The Defilers"),
        509 => Some("The League of Arathor"),
        910 => Some("Brood of Nozdormu"),
        _ => None,
    }
}

#[cfg(test)]
pub(in crate::world) fn quest_reputation_rewards(
    player_level: u8,
    quest: &QuestTemplateQuery,
) -> Vec<(u32, i32)> {
    quest_reputation_rewards_with_bonus(player_level, quest, 0)
}

pub(in crate::world) fn quest_reputation_rewards_with_bonus(
    player_level: u8,
    quest: &QuestTemplateQuery,
    gain_bonus_percent: i32,
) -> Vec<(u32, i32)> {
    quest
        .rew_rep_faction
        .iter()
        .zip(quest.rew_rep_value.iter())
        .filter_map(|(faction, value)| {
            if *faction == 0 || *value == 0 {
                return None;
            }
            let reward = apply_reputation_gain_bonus(
                calculate_quest_reputation_gain(player_level, quest, *value),
                gain_bonus_percent,
            );
            (reward != 0).then_some((*faction, reward))
        })
        .collect()
}

pub(in crate::world) fn apply_reputation_gain_bonus(reward: i32, gain_bonus_percent: i32) -> i32 {
    if reward == 0 || gain_bonus_percent == 0 {
        return reward;
    }
    let multiplier = 100i64.saturating_add(i64::from(gain_bonus_percent));
    if multiplier <= 0 {
        return 0;
    }
    ((i64::from(reward) * multiplier) / 100).clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

pub(in crate::world) fn calculate_quest_reputation_gain(
    player_level: u8,
    quest: &QuestTemplateQuery,
    rep: i32,
) -> i32 {
    let quest_level = if quest.quest_level > 0 {
        quest.quest_level
    } else {
        player_level as u32
    };
    let player_level = player_level as u32;
    let mut percent = 100.0;

    if quest_level > 0 {
        let threshold = quest_level + 5;
        if player_level > threshold {
            percent *=
                REPUTATION_LOWLEVEL_QUEST_RATE.max(1.0 - (0.2 * (player_level - threshold) as f32));
        }
    }

    if percent <= 0.0 {
        return 0;
    }
    (REPUTATION_GAIN_RATE * rep as f32 * percent / 100.0) as i32
}

pub(in crate::world) fn build_set_faction_visible_body(faction: u32) -> Option<Vec<u8>> {
    let slot = reputation_list_slot_for_faction(faction)?;
    Some((slot as u32).to_le_bytes().to_vec())
}

pub(in crate::world) fn build_set_faction_standing_body(
    reputations: &[CharacterReputation],
) -> Vec<u8> {
    let mut body = Vec::with_capacity(4 + reputations.len() * 8);
    let mapped: Vec<_> = reputations
        .iter()
        .filter_map(|reputation| {
            reputation_list_slot_for_faction(reputation.faction)
                .map(|slot| (slot as u32, reputation.standing))
        })
        .collect();
    body.extend_from_slice(&(mapped.len() as u32).to_le_bytes());
    for (slot, standing) in mapped {
        body.extend_from_slice(&slot.to_le_bytes());
        body.extend_from_slice(&standing.to_le_bytes());
    }
    body
}

pub(in crate::world) fn reputation_gain_system_message(
    change: &CharacterReputationChange,
) -> Option<String> {
    let name = reputation_faction_name(change.reputation.faction)?;
    match change.delta.cmp(&0) {
        std::cmp::Ordering::Greater => Some(format!(
            "Reputation with {name} increased by {}.",
            change.delta
        )),
        std::cmp::Ordering::Less => Some(format!(
            "Reputation with {name} decreased by {}.",
            change.delta.abs()
        )),
        std::cmp::Ordering::Equal => None,
    }
}
