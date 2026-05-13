use super::*;

pub(in crate::world) fn can_creature_attack_player_on_sight(
    faction_templates: &FactionTemplateStore,
    creature_faction: u32,
    player_race: u8,
) -> bool {
    can_faction_attack_on_sight(
        faction_templates,
        creature_faction,
        faction_for_race(player_race),
    )
}

pub(in crate::world) fn can_faction_attack_on_sight(
    faction_templates: &FactionTemplateStore,
    creature_faction: u32,
    player_faction: u32,
) -> bool {
    faction_reaction_to(faction_templates, creature_faction, player_faction)
        == FactionReaction::Hostile
}

pub(in crate::world) fn faction_reaction_to(
    faction_templates: &FactionTemplateStore,
    this_faction: u32,
    other_faction: u32,
) -> FactionReaction {
    let Some(this_template) = faction_templates.entry(this_faction) else {
        return FactionReaction::Neutral;
    };
    let Some(other_template) = faction_templates.entry(other_faction) else {
        return FactionReaction::Neutral;
    };
    faction_template_reaction(this_template, other_template)
}

pub(in crate::world) fn faction_template_reaction(
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
