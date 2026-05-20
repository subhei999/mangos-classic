use super::*;

pub(in crate::world) fn spell_effect_simple_value(base_points: i32) -> Option<u32> {
    (base_points >= 0).then_some((base_points + 1) as u32)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct SpellEffectValueContext {
    pub(in crate::world) spell_id: u32,
    pub(in crate::world) max_level: u32,
    pub(in crate::world) base_level: u32,
    pub(in crate::world) spell_level: u32,
    pub(in crate::world) spell_rank_level: Option<i32>,
    pub(in crate::world) combo_points: u8,
}

impl SpellEffectValueContext {
    pub(in crate::world) fn unranked(
        template: &wow_db::SpellTemplateQuery,
        combo_points: u8,
    ) -> Self {
        Self {
            spell_id: template.id,
            max_level: template.max_level,
            base_level: template.base_level,
            spell_level: template.spell_level,
            spell_rank_level: None,
            combo_points,
        }
    }

    pub(in crate::world) fn with_spell_rank_level(
        template: &wow_db::SpellTemplateQuery,
        spell_rank_level: i32,
        combo_points: u8,
    ) -> Self {
        Self {
            spell_rank_level: Some(spell_rank_level),
            ..Self::unranked(template, combo_points)
        }
    }
}

pub(in crate::world) fn player_spell_effect_value_context(
    maps: &MapRuntimeManager,
    template: &wow_db::SpellTemplateQuery,
    character_skills: &[CharacterSkill],
    combo_points: u8,
) -> SpellEffectValueContext {
    let Some(ability) = maps.skill_line_ability_for_spell(template.id) else {
        return if maps.skill_line_abilities_by_spell.is_empty() {
            SpellEffectValueContext::unranked(template, combo_points)
        } else {
            SpellEffectValueContext::with_spell_rank_level(template, 0, combo_points)
        };
    };
    let mut spell_rank = character_skills
        .iter()
        .find(|skill| u32::from(skill.skill) == ability.skill_id)
        .map(|skill| u32::from(skill.value))
        .unwrap_or(0);
    if template.max_level > 0 {
        let max_rank = template.max_level.saturating_mul(5);
        if spell_rank >= max_rank {
            spell_rank = max_rank;
        }
    }
    SpellEffectValueContext::with_spell_rank_level(template, (spell_rank / 5) as i32, combo_points)
}

pub(in crate::world) fn spell_effect_calculated_i32(
    effect: SpellInfoEffect,
    context: SpellEffectValueContext,
) -> i32 {
    let base_dice = effect.base_dice as i32;
    let mut base_points = effect.base_points as f32;
    let mut random_points = effect.die_sides;

    if effect.real_points_per_level != 0.0 {
        if let Some(mut level) = context.spell_rank_level {
            if context.max_level > 0 && level > context.max_level as i32 {
                level = context.max_level as i32;
            } else if level < context.base_level as i32 {
                level = context.base_level as i32;
            }
            level -= context.spell_level as i32;
            base_points += level as f32 * effect.real_points_per_level;
            random_points =
                random_points.saturating_add((level as f32 * effect.dice_per_level).trunc() as i32);
        } else {
            warn!(
                spell_id = context.spell_id,
                effect_id = effect.effect_id,
                "Skipping level scaling for spell effect because SkillLineAbility.dbc rank data is unavailable"
            );
        }
    }

    match random_points {
        0 | 1 => {
            base_points += base_dice as f32;
        }
        random_points => {
            let low = random_points.min(base_dice);
            let high = random_points.max(base_dice);
            base_points += rand::thread_rng().gen_range(low..=high) as f32;
        }
    }

    if effect.points_per_combo_point != 0.0 && context.combo_points > 0 {
        base_points +=
            (effect.points_per_combo_point * f32::from(context.combo_points)).trunc() as i32 as f32;
    }

    base_points.trunc() as i32
}

pub(in crate::world) fn spell_effect_calculated_u32(
    effect: SpellInfoEffect,
    context: SpellEffectValueContext,
) -> Option<u32> {
    let value = spell_effect_calculated_i32(effect, context);
    (value >= 0).then_some(value as u32)
}

pub(in crate::world) fn spell_power_cost_amount(
    template: &wow_db::SpellTemplateQuery,
    context: SpellEffectValueContext,
) -> u32 {
    let Some(rank_level) = context.spell_rank_level else {
        return template.mana_cost;
    };
    let cost = i64::from(template.mana_cost)
        + i64::from(template.mana_cost_per_level)
            * i64::from(rank_level.saturating_sub(template.base_level as i32));
    cost.clamp(0, u32::MAX as i64) as u32
}
