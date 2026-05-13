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
    caster: ObjectGuid,
) -> SpellCastTargets {
    targets.target_mask =
        (targets.target_mask | SPELL_CAST_TARGET_UNIT) & !SPELL_CAST_TARGET_UNIT_ENEMY;
    if targets.unit_target.is_none()
        && matches!(
            spell_profile.kind,
            SpellCastKind::AuraApplication | SpellCastKind::CreateItem | SpellCastKind::DirectHeal
        )
        && matches!(
            spell_profile.aura_target,
            SpellAuraTarget::Caster | SpellAuraTarget::CasterAreaEnemy
        )
    {
        targets.unit_target = Some(caster);
    }
    targets.gameobject_target = None;
    targets
}

pub(in crate::world) fn normalize_item_use_targets(
    mut targets: SpellCastTargets,
    item_spell: &SpellCastProfile,
    caster: ObjectGuid,
) -> SpellCastTargets {
    if targets.target_mask == 0 {
        targets.target_mask = SPELL_CAST_TARGET_UNIT;
        targets.unit_target = Some(caster);
        return targets;
    }
    if item_spell.kind == SpellCastKind::AuraApplication
        && item_spell.aura_target == SpellAuraTarget::Caster
    {
        targets.target_mask =
            (targets.target_mask | SPELL_CAST_TARGET_UNIT) & !SPELL_CAST_TARGET_UNIT_ENEMY;
        targets.unit_target = Some(caster);
        targets.gameobject_target = None;
    }
    targets
}
