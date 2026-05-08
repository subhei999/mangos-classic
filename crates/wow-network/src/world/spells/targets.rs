#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpellTargetResolution {
    ClientProvided,
    CasterSelf,
    SelectedUnit,
    MissingTargetFallback,
}

fn normalize_spell_cast_targets(
    mut targets: SpellCastTargets,
    spell_profile: &SpellCastProfile,
    caster: ObjectGuid,
) -> SpellCastTargets {
    targets.target_mask = (targets.target_mask | SPELL_CAST_TARGET_UNIT)
        & !SPELL_CAST_TARGET_UNIT_ENEMY;
    if targets.unit_target.is_none()
        && matches!(
            spell_profile.kind,
            SpellCastKind::AuraApplication | SpellCastKind::DirectHeal
        )
        && spell_profile.aura_target == SpellAuraTarget::Caster
    {
        targets.unit_target = Some(caster);
    }
    targets.gameobject_target = None;
    targets
}

fn normalize_item_use_targets(
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
        targets.target_mask = (targets.target_mask | SPELL_CAST_TARGET_UNIT)
            & !SPELL_CAST_TARGET_UNIT_ENEMY;
        targets.unit_target = Some(caster);
        targets.gameobject_target = None;
    }
    targets
}
