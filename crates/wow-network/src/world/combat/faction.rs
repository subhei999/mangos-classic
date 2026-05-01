#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FactionReaction {
    Hostile,
    Neutral,
    Friendly,
}

#[derive(Debug, Clone, Copy)]
struct FactionTemplateEntry {
    faction: u32,
    faction_group_mask: u32,
    friend_group_mask: u32,
    enemy_group_mask: u32,
    enemy_faction: [u32; 4],
    friend_faction: [u32; 4],
}

const FACTION_GROUP_MASK_PLAYER: u32 = 1;
const FACTION_GROUP_MASK_ALLIANCE: u32 = 2;
const FACTION_GROUP_MASK_HORDE: u32 = 4;
const FACTION_GROUP_MASK_MONSTER: u32 = 8;

fn can_creature_attack_player_on_sight(creature_faction: u32, player_race: u8) -> bool {
    can_faction_attack_on_sight(creature_faction, faction_for_race(player_race))
}

fn can_faction_attack_on_sight(creature_faction: u32, player_faction: u32) -> bool {
    faction_reaction_to(creature_faction, player_faction) == FactionReaction::Hostile
}

fn faction_reaction_to(this_faction: u32, other_faction: u32) -> FactionReaction {
    let Some(this_template) = faction_template_entry(this_faction) else {
        return FactionReaction::Neutral;
    };
    let Some(other_template) = faction_template_entry(other_faction) else {
        return FactionReaction::Neutral;
    };
    faction_template_reaction(this_template, other_template)
}

fn faction_template_reaction(
    this_template: FactionTemplateEntry,
    other_template: FactionTemplateEntry,
) -> FactionReaction {
    if other_template.faction_group_mask & this_template.enemy_group_mask != 0 {
        return FactionReaction::Hostile;
    }
    if other_template.faction != 0
        && this_template
            .enemy_faction
            .contains(&other_template.faction)
    {
        return FactionReaction::Hostile;
    }
    if other_template.faction_group_mask & this_template.friend_group_mask != 0 {
        return FactionReaction::Friendly;
    }
    if other_template.faction != 0
        && this_template
            .friend_faction
            .contains(&other_template.faction)
    {
        return FactionReaction::Friendly;
    }
    if this_template.faction_group_mask & other_template.friend_group_mask != 0 {
        return FactionReaction::Friendly;
    }
    if this_template.faction != 0
        && other_template
            .friend_faction
            .contains(&this_template.faction)
    {
        return FactionReaction::Friendly;
    }
    FactionReaction::Neutral
}

fn faction_template_entry(id: u32) -> Option<FactionTemplateEntry> {
    match id {
        // Rust currently serializes generic player faction templates during
        // bootstrap. These preserve the CMaNGOS group-mask relation shape until
        // the real FactionTemplate.dbc loader is wired.
        1 => Some(faction_template(
            1,
            FACTION_GROUP_MASK_PLAYER | FACTION_GROUP_MASK_ALLIANCE,
            FACTION_GROUP_MASK_ALLIANCE,
            FACTION_GROUP_MASK_HORDE,
        )),
        2 => Some(faction_template(
            2,
            FACTION_GROUP_MASK_PLAYER | FACTION_GROUP_MASK_HORDE,
            FACTION_GROUP_MASK_HORDE,
            FACTION_GROUP_MASK_ALLIANCE,
        )),
        // Local Vanilla FactionTemplate.dbc / RealClassicDb Northshire rows:
        // 11/12 are friendly Alliance NPC factions, 17 is hostile Defias,
        // 25 is neutral Kobold, and 32 is neutral Young Wolf.
        // The fixture combat faction keeps an explicitly hostile category.
        11 | 12 | RUST_GUIDE_FACTION_TEMPLATE => Some(faction_template(
            id,
            FACTION_GROUP_MASK_ALLIANCE,
            FACTION_GROUP_MASK_PLAYER | FACTION_GROUP_MASK_ALLIANCE,
            FACTION_GROUP_MASK_HORDE,
        )),
        14 | 17 => Some(faction_template(
            id,
            FACTION_GROUP_MASK_MONSTER,
            FACTION_GROUP_MASK_MONSTER,
            FACTION_GROUP_MASK_PLAYER,
        )),
        25 | 32 => Some(faction_template(id, FACTION_GROUP_MASK_MONSTER, 0, 0)),
        _ => None,
    }
}

fn faction_template(
    faction: u32,
    faction_group_mask: u32,
    friend_group_mask: u32,
    enemy_group_mask: u32,
) -> FactionTemplateEntry {
    FactionTemplateEntry {
        faction,
        faction_group_mask,
        friend_group_mask,
        enemy_group_mask,
        enemy_faction: [0; 4],
        friend_faction: [0; 4],
    }
}


