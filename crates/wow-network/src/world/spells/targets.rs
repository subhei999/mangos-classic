use super::*;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum SpellTargetResolution {
    ClientProvided,
    CasterSelf,
    SelectedUnit,
    MissingTargetFallback,
}

pub(in crate::world) fn normalize_spell_cast_targets(
    mut targets: SpellCastTargets,
    spell_profile: &SpellCastProfile,
    spell_info: &SpellInfo<'_>,
    caster: ObjectGuid,
) -> SpellCastTargets {
    let plan_target = spell_info
        .player_spell_plan()
        .filter(|plan| plan.profile.kind == spell_profile.kind)
        .map(|plan| plan.target)
        .unwrap_or(SpellPlanTarget::Caster);
    let target_kind = plan_target.target_kind();
    if matches!(
        target_kind,
        SpellTargetKind::Caster | SpellTargetKind::Destination
    ) && matches!(
        plan_target,
        SpellPlanTarget::CasterAreaEnemy { .. } | SpellPlanTarget::DestinationAreaEnemy
    ) {
        targets.target_mask &= !(SPELL_CAST_TARGET_UNIT | SPELL_CAST_TARGET_UNIT_ENEMY);
        targets.unit_target = None;
        targets.gameobject_target = None;
        return targets;
    }

    if target_kind == SpellTargetKind::Destination {
        targets.target_mask &= !(SPELL_CAST_TARGET_UNIT | SPELL_CAST_TARGET_UNIT_ENEMY);
        targets.unit_target = None;
        targets.gameobject_target = None;
        return targets;
    }

    targets.target_mask =
        (targets.target_mask | SPELL_CAST_TARGET_UNIT) & !SPELL_CAST_TARGET_UNIT_ENEMY;
    if targets.unit_target.is_none()
        && matches!(
            spell_profile.kind,
            SpellCastKind::AuraApplication | SpellCastKind::CreateItem | SpellCastKind::DirectHeal
        )
        && target_kind == SpellTargetKind::Caster
    {
        targets.unit_target = Some(caster);
    }
    targets.gameobject_target = None;
    targets
}

pub(in crate::world) fn normalize_item_use_targets(
    mut targets: SpellCastTargets,
    item_spell: &SpellCastProfile,
    spell_info: &SpellInfo<'_>,
    caster: ObjectGuid,
) -> SpellCastTargets {
    let plan_target = spell_info
        .item_spell_plan(ObjectGuid::EMPTY)
        .filter(|plan| plan.profile.kind == item_spell.kind)
        .map(|plan| plan.target)
        .unwrap_or(SpellPlanTarget::Caster);
    if targets.target_mask == 0 {
        targets.target_mask = SPELL_CAST_TARGET_UNIT;
        targets.unit_target = Some(caster);
        return targets;
    }
    if item_spell.kind == SpellCastKind::AuraApplication
        && plan_target.target_kind() == SpellTargetKind::Caster
    {
        targets.target_mask =
            (targets.target_mask | SPELL_CAST_TARGET_UNIT) & !SPELL_CAST_TARGET_UNIT_ENEMY;
        targets.unit_target = Some(caster);
        targets.gameobject_target = None;
    }
    targets
}
