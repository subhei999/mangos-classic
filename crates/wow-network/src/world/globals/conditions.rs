// CMaNGOS reference: src/game/Globals/Conditions.{h,cpp}
// Shared DB/script condition evaluation used by quests, gossip, vendors,
// trainers, loot, and scripts. This file intentionally mirrors the CMaNGOS
// `ConditionEntry::Meets/Evaluate` shape and fails closed for condition types
// whose required backing state is not wired yet.

use std::future::Future;
use std::pin::Pin;

const CONDITION_NOT: i16 = -3;
const CONDITION_OR: i16 = -2;
const CONDITION_AND: i16 = -1;
const CONDITION_NONE: i16 = 0;
const CONDITION_ITEM: i16 = 2;
const CONDITION_ITEM_EQUIPPED: i16 = 3;
const CONDITION_TEAM: i16 = 6;
const CONDITION_SKILL: i16 = 7;
const CONDITION_QUEST_REWARDED: i16 = 8;
const CONDITION_QUEST_TAKEN: i16 = 9;
const CONDITION_ACTIVE_GAME_EVENT: i16 = 12;
const CONDITION_RACE_CLASS: i16 = 14;
const CONDITION_LEVEL: i16 = 15;
const CONDITION_SPELL: i16 = 17;
const CONDITION_QUEST_AVAILABLE: i16 = 19;
const CONDITION_QUEST_NONE: i16 = 22;
const CONDITION_ACTIVE_HOLIDAY: i16 = 26;
const CONDITION_SKILL_BELOW: i16 = 29;
const CONDITION_GENDER: i16 = 35;
const CONDITION_DEAD_OR_AWAY: i16 = 36;

const CONDITION_FLAG_REVERSE_RESULT: u8 = 0x1;
const CONDITION_FLAG_SWAP_TARGETS: u8 = 0x2;
const HORDE_FACTION: u32 = 67;
const CONDITION_RECURSION_LIMIT: u8 = 32;

type ConditionFuture<'a> = Pin<Box<dyn Future<Output = anyhow::Result<bool>> + Send + 'a>>;

#[derive(Debug, Clone, Copy)]
enum ConditionSource {
    Quest,
}

#[derive(Clone, Copy)]
struct ConditionEvaluationContext<'a> {
    world_db_pool: &'a MySqlPool,
    session: &'a WorldSessionState,
    source: ConditionSource,
}

impl ObjectMgr {
    fn is_condition_satisfied<'a>(
        &'a self,
        condition_id: u32,
        context: ConditionEvaluationContext<'a>,
    ) -> ConditionFuture<'a> {
        self.is_condition_satisfied_with_depth(condition_id, context, 0)
    }

    fn is_condition_satisfied_with_depth<'a>(
        &'a self,
        condition_id: u32,
        context: ConditionEvaluationContext<'a>,
        depth: u8,
    ) -> ConditionFuture<'a> {
        Box::pin(async move {
            if condition_id == 0 {
                return Ok(true);
            }
            if depth >= CONDITION_RECURSION_LIMIT {
                warn!(
                    condition_id,
                    depth,
                    "Condition recursion limit reached; failing closed"
                );
                return Ok(false);
            }

            let Some(condition) = self
                .condition_entry(context.world_db_pool, condition_id)
                .await?
            else {
                return Ok(false);
            };

            self.condition_meets(&condition, context, depth).await
        })
    }

    async fn condition_meets(
        &self,
        condition: &wow_db::ConditionQuery,
        context: ConditionEvaluationContext<'_>,
        depth: u8,
    ) -> anyhow::Result<bool> {
        let swaps_targets = condition.flags & CONDITION_FLAG_SWAP_TARGETS != 0;
        let mut result = self
            .condition_evaluate(condition, context, depth, swaps_targets)
            .await?;
        if condition.flags & CONDITION_FLAG_REVERSE_RESULT != 0 {
            result = !result;
        }
        Ok(result)
    }

    async fn condition_evaluate(
        &self,
        condition: &wow_db::ConditionQuery,
        context: ConditionEvaluationContext<'_>,
        depth: u8,
        swaps_targets: bool,
    ) -> anyhow::Result<bool> {
        let player_available = context.session.active_character.is_some() && !swaps_targets;
        match condition.condition_type {
            CONDITION_NOT => {
                self.is_condition_satisfied_with_depth(
                    condition.value1,
                    context,
                    depth.saturating_add(1),
                )
                .await
                .map(|result| !result)
            }
            CONDITION_OR => {
                if condition.value3 != 0
                    && self
                        .is_condition_satisfied_with_depth(
                            condition.value3,
                            ConditionEvaluationContext { ..context },
                            depth.saturating_add(1),
                        )
                        .await?
                {
                    return Ok(true);
                }
                if condition.value4 != 0
                    && self
                        .is_condition_satisfied_with_depth(
                            condition.value4,
                            ConditionEvaluationContext { ..context },
                            depth.saturating_add(1),
                        )
                        .await?
                {
                    return Ok(true);
                }
                Ok(self
                    .is_condition_satisfied_with_depth(
                        condition.value1,
                        ConditionEvaluationContext { ..context },
                        depth.saturating_add(1),
                    )
                    .await?
                    || self
                        .is_condition_satisfied_with_depth(
                            condition.value2,
                            context,
                            depth.saturating_add(1),
                        )
                        .await?)
            }
            CONDITION_AND => {
                if condition.value3 != 0
                    && !self
                        .is_condition_satisfied_with_depth(
                            condition.value3,
                            ConditionEvaluationContext { ..context },
                            depth.saturating_add(1),
                        )
                        .await?
                {
                    return Ok(false);
                }
                if condition.value4 != 0
                    && !self
                        .is_condition_satisfied_with_depth(
                            condition.value4,
                            ConditionEvaluationContext { ..context },
                            depth.saturating_add(1),
                        )
                        .await?
                {
                    return Ok(false);
                }
                Ok(self
                    .is_condition_satisfied_with_depth(
                        condition.value1,
                        ConditionEvaluationContext { ..context },
                        depth.saturating_add(1),
                    )
                    .await?
                    && self
                        .is_condition_satisfied_with_depth(
                            condition.value2,
                            context,
                            depth.saturating_add(1),
                        )
                        .await?)
            }
            CONDITION_NONE => Ok(true),
            CONDITION_ITEM => {
                if !player_available {
                    return Ok(false);
                }
                Ok(session_item_count(context.session, condition.value1) >= condition.value2)
            }
            CONDITION_ITEM_EQUIPPED => {
                if !player_available {
                    return Ok(false);
                }
                Ok(session_has_equipped_item(context.session, condition.value1))
            }
            CONDITION_TEAM => {
                let Some(character) = context.session.active_character.as_ref() else {
                    return Ok(false);
                };
                Ok(player_team_for_race(character.race) == Some(condition.value1))
            }
            CONDITION_SKILL => {
                if !player_available {
                    return Ok(false);
                }
                Ok(session_skill_value(context.session, condition.value1)
                    .is_some_and(|value| value >= condition.value2))
            }
            CONDITION_QUEST_REWARDED => {
                if !player_available {
                    return Ok(false);
                }
                Ok(context
                    .session
                    .quest_statuses
                    .get(&condition.value1)
                    .is_some_and(|status| status.rewarded != 0))
            }
            CONDITION_QUEST_TAKEN => {
                if !player_available {
                    return Ok(false);
                }
                Ok(session_is_current_quest(
                    context.session,
                    condition.value1,
                    condition.value2,
                ))
            }
            CONDITION_ACTIVE_GAME_EVENT => {
                Ok(self.active_game_event_state().await.is_active(condition.value1 as u16))
            }
            CONDITION_RACE_CLASS => {
                let Some(character) = context.session.active_character.as_ref() else {
                    return Ok(false);
                };
                let race_ok =
                    condition.value1 == 0 || (quest_race_or_class_mask(character.race) & condition.value1) != 0;
                let class_ok =
                    condition.value2 == 0 || (quest_race_or_class_mask(character.class) & condition.value2) != 0;
                Ok(race_ok && class_ok)
            }
            CONDITION_LEVEL => {
                let Some(character) = context.session.active_character.as_ref() else {
                    return Ok(false);
                };
                Ok(match condition.value2 {
                    0 => u32::from(character.level) == condition.value1,
                    1 => u32::from(character.level) >= condition.value1,
                    2 => u32::from(character.level) <= condition.value1,
                    _ => false,
                })
            }
            CONDITION_SPELL => {
                if !player_available {
                    return Ok(false);
                }
                Ok(match condition.value2 {
                    0 => context.session.active_spells.contains(&condition.value1),
                    1 => !context.session.active_spells.contains(&condition.value1),
                    _ => false,
                })
            }
            CONDITION_QUEST_AVAILABLE => {
                if !player_available {
                    return Ok(false);
                }
                let Some(quest) = self
                    .quest_template(context.world_db_pool, condition.value1)
                    .await?
                else {
                    return Ok(false);
                };
                can_take_start_quest_with_condition_depth(
                    self,
                    context.world_db_pool,
                    &quest,
                    context.session,
                    depth.saturating_add(1),
                )
                .await
            }
            CONDITION_QUEST_NONE => {
                if !player_available {
                    return Ok(false);
                }
                Ok(context
                    .session
                    .quest_statuses
                    .get(&condition.value1)
                    .is_none_or(|status| !quest_status_is_current(status) && status.rewarded == 0))
            }
            CONDITION_ACTIVE_HOLIDAY => {
                Ok(self.active_holidays().await.contains(&condition.value1))
            }
            CONDITION_SKILL_BELOW => {
                if !player_available {
                    return Ok(false);
                }
                let skill = session_skill_value(context.session, condition.value1);
                if condition.value2 == 1 {
                    return Ok(skill.is_none());
                }
                Ok(skill.is_some_and(|value| value < condition.value2))
            }
            CONDITION_GENDER => {
                let Some(character) = context.session.active_character.as_ref() else {
                    return Ok(false);
                };
                Ok(u32::from(character_gender(context.session, character)) == condition.value1)
            }
            CONDITION_DEAD_OR_AWAY => {
                Ok(condition_dead_or_away(context.session, condition.value1, condition.value2))
            }
            unsupported => {
                debug!(
                    condition_entry = condition.condition_entry,
                    condition_type = unsupported,
                    source = ?context.source,
                    "Unsupported condition type; failing closed"
                );
                Ok(false)
            }
        }
    }
}

fn session_item_count(session: &WorldSessionState, item: u32) -> u32 {
    session
        .inventory
        .iter()
        .filter(|inventory_item| inventory_item.item_template == item)
        .map(|inventory_item| inventory_item.count)
        .sum()
}

fn session_has_equipped_item(session: &WorldSessionState, item: u32) -> bool {
    session
        .inventory
        .iter()
        .any(|inventory_item| inventory_item.item_template == item && inventory_item.bag == 0 && inventory_item.slot < 19)
}

fn session_skill_value(session: &WorldSessionState, skill: u32) -> Option<u32> {
    session
        .character_skills
        .iter()
        .find(|character_skill| u32::from(character_skill.skill) == skill)
        .map(|character_skill| u32::from(character_skill.value))
}

fn session_is_current_quest(session: &WorldSessionState, quest: u32, quest_taken_mode: u32) -> bool {
    let Some(status) = session.quest_statuses.get(&quest) else {
        return false;
    };
    if !quest_status_is_current(status) {
        return false;
    }
    match quest_taken_mode {
        0 => true,
        1 => status.status == QUEST_STATUS_INCOMPLETE,
        2 => status.status == QUEST_STATUS_COMPLETE,
        _ => false,
    }
}

fn player_team_for_race(race: u8) -> Option<u32> {
    match race {
        1 | 3 | 4 | 7 => Some(ALLIANCE_FACTION),
        2 | 5 | 6 | 8 => Some(HORDE_FACTION),
        _ => None,
    }
}

fn character_gender(session: &WorldSessionState, character: &ActiveCharacter) -> u8 {
    let _ = character;
    session.player_visual.as_ref().map_or(0, |visual| visual.gender)
}

fn condition_dead_or_away(session: &WorldSessionState, value1: u32, _range: u32) -> bool {
    let player_is_alive = session
        .active_character
        .as_ref()
        .is_some_and(|_| session.player_health != 0 && session.player_death_state == PlayerDeathState::Alive);
    match value1 {
        0 | 1 => !player_is_alive,
        2 => false,
        3 => true,
        _ => false,
    }
}
