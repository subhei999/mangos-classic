use super::*;

const CMANGOS_SKILL_CATEGORY_WEAPON: i32 = 6;
const CMANGOS_SKILL_CATEGORY_CLASS: i32 = 7;
const CMANGOS_SKILL_CATEGORY_ARMOR: i32 = 8;
const CMANGOS_SKILL_CATEGORY_LANGUAGES: i32 = 10;
const CMANGOS_SKILL_POISONS: u32 = 40;
const CMANGOS_SKILL_LOCKPICKING: u32 = 633;
const CMANGOS_SKILL_FLAG_MAXIMIZED: u32 = 0x010;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CMaNGOSSkillRangeType {
    None,
    Language,
    Level,
    Mono,
}

fn cmangos_skill_range_type(skill: SkillLineEntry) -> CMaNGOSSkillRangeType {
    match skill.category_id {
        CMANGOS_SKILL_CATEGORY_LANGUAGES => CMaNGOSSkillRangeType::Language,
        CMANGOS_SKILL_CATEGORY_WEAPON => {
            if skill.id != u32::from(SKILL_FIST_WEAPONS) {
                CMaNGOSSkillRangeType::Level
            } else {
                CMaNGOSSkillRangeType::Mono
            }
        }
        CMANGOS_SKILL_CATEGORY_ARMOR | CMANGOS_SKILL_CATEGORY_CLASS => {
            if skill.id != CMANGOS_SKILL_POISONS && skill.id != CMANGOS_SKILL_LOCKPICKING {
                CMaNGOSSkillRangeType::Mono
            } else {
                CMaNGOSSkillRangeType::Level
            }
        }
        _ => CMaNGOSSkillRangeType::None,
    }
}

pub(in crate::world) fn sync_player_level_backed_skills(
    maps: &MapRuntimeManager,
    race: u8,
    class: u8,
    level: u8,
    character_skills: &mut [CharacterSkill],
) -> Vec<SkillProgressionUpdate> {
    if maps.skill_lines.is_empty() || maps.skill_race_class_infos_by_skill.is_empty() {
        return set_level_capped_combat_skill_maxes(level, character_skills);
    }

    let level_cap = u16::from(level.max(1)).saturating_mul(5);
    character_skills
        .iter_mut()
        .enumerate()
        .filter_map(|(slot, skill)| {
            let skill_id = u32::from(skill.skill);
            let skill_line = maps.skill_line(skill_id)?;
            let range_type = cmangos_skill_range_type(skill_line);
            if !matches!(
                range_type,
                CMaNGOSSkillRangeType::Level | CMaNGOSSkillRangeType::Mono
            ) || skill.max == 1
            {
                return None;
            }
            let skill_info = maps.skill_race_class_info(skill_id, race, class);
            let maxed = skill_info
                .map(|entry| (entry.flags & CMANGOS_SKILL_FLAG_MAXIMIZED) != 0)
                .unwrap_or(false);
            let old_value = skill.value;
            let old_max = skill.max;
            skill.max = level_cap;
            if maxed || skill.value > level_cap {
                skill.value = level_cap;
            }
            (skill.value != old_value || skill.max != old_max).then_some(SkillProgressionUpdate {
                slot,
                skill: skill.skill,
                value: skill.value,
                max: skill.max,
            })
        })
        .collect()
}
