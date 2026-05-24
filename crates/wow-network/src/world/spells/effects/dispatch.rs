use super::*;

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_spell_effects(
    stream: &mut WorldPacketSink,

    deps: SpellCastDeps<'_>,

    session: &mut WorldSessionState,

    caster: ObjectGuid,

    character_guid: u32,

    character_level: u8,

    map_id: u32,

    spell_template: &wow_db::SpellTemplateQuery,

    spell_profile: &SpellCastProfile,

    targets: &SpellCastTargets,

    target_outcome: Option<PlayerSpellTargetOutcome>,

    now: Instant,

    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let spell_info = SpellInfo::from_template(spell_template);
    let spell_plan = spell_info.player_spell_plan();

    let mut charge_applied = false;

    let mut direct_heal_applied = false;

    let mut direct_energize_applied = false;

    let mut aura_applied = false;

    let mut create_item_applied = false;

    let mut weapon_damage_applied = false;

    let mut landed_damage = false;

    let mut direct_damage_processed = false;

    let mut deferred_hostile_aura = false;

    let mut learned_spells = HashSet::new();

    let spell_has_hostile_direct_damage = spell_plan
        .as_ref()
        .is_some_and(SpellPlan::has_hostile_direct_effect);

    let combo_points_for_effects = spell_combo_points_for_effects(
        deps.shared_world,
        caster,
        character_guid,
        map_id,
        spell_profile,
        targets,
    )
    .await;

    let effect_value_context = player_spell_effect_value_context(
        deps.shared_world.maps,
        spell_template,
        &session.character.character_skills,
        combo_points_for_effects,
    );

    for (effect_index, effect) in spell_info.effects.into_iter().enumerate() {
        match effect.dispatch {
            SpellEffectDispatch::Empty => {}

            SpellEffectDispatch::Charge
                if spell_profile.kind == SpellCastKind::Charge && !charge_applied =>
            {
                apply_player_charge_effect(
                    stream,
                    deps.shared_world,
                    session,
                    caster,
                    map_id,
                    spell_template,
                    spell_profile,
                    targets,
                    header_crypto,
                )
                .await?;

                charge_applied = true;
            }

            SpellEffectDispatch::Taunt => {
                apply_player_taunt_effect(deps, caster, map_id, targets).await?;
            }

            SpellEffectDispatch::SchoolDamage
                if spell_profile.kind != SpellCastKind::Charge
                    && spell_profile.kind != SpellCastKind::NextMeleeSwing =>
            {
                if let Some(damage_effect) = player_direct_damage_effect(
                    spell_template,
                    spell_profile,
                    effect,
                    effect_value_context,
                ) {
                    landed_damage |= apply_player_direct_damage_effect(
                        stream,
                        deps,
                        session,
                        caster,
                        character_guid,
                        map_id,
                        damage_effect,
                        targets,
                        target_outcome,
                        header_crypto,
                    )
                    .await?;

                    direct_damage_processed = true;
                }
            }

            SpellEffectDispatch::WeaponDamage | SpellEffectDispatch::WeaponPercentDamage
                if spell_profile.kind != SpellCastKind::Charge
                    && spell_profile.kind != SpellCastKind::NextMeleeSwing
                    && !weapon_damage_applied =>
            {
                landed_damage |= apply_player_direct_damage_effect(
                    stream,
                    deps,
                    session,
                    caster,
                    character_guid,
                    map_id,
                    player_weapon_damage_effect(spell_profile),
                    targets,
                    None,
                    header_crypto,
                )
                .await?;

                direct_damage_processed = true;

                weapon_damage_applied = true;
            }

            SpellEffectDispatch::AddComboPoints if landed_damage => {
                apply_player_combo_points_effect(
                    stream,
                    deps.shared_world,
                    caster,
                    character_guid,
                    map_id,
                    effect,
                    targets,
                    header_crypto,
                )
                .await?;
            }

            SpellEffectDispatch::Heal
                if spell_profile.kind == SpellCastKind::DirectHeal && !direct_heal_applied =>
            {
                apply_player_direct_heal_effect(
                    stream,
                    deps,
                    session,
                    caster,
                    map_id,
                    &spell_info,
                    effect_value_context,
                    targets,
                    header_crypto,
                )
                .await?;

                direct_heal_applied = true;
            }

            SpellEffectDispatch::Energize if !direct_energize_applied => {
                apply_player_direct_energize_effect(
                    stream,
                    deps,
                    session,
                    caster,
                    character_guid,
                    map_id,
                    &spell_info,
                    effect_value_context,
                    header_crypto,
                )
                .await?;

                direct_energize_applied = true;
            }

            SpellEffectDispatch::CreateItem
                if spell_profile.kind == SpellCastKind::CreateItem && !create_item_applied =>
            {
                apply_player_create_item_effects(
                    stream,
                    deps,
                    session,
                    character_guid,
                    &spell_info,
                    effect_value_context,
                    header_crypto,
                )
                .await?;

                create_item_applied = true;
            }

            SpellEffectDispatch::Leap | SpellEffectDispatch::Teleport
                if spell_profile.kind == SpellCastKind::Teleport =>
            {
                apply_player_near_teleport_effect(
                    stream,
                    deps,
                    session,
                    character_guid,
                    map_id,
                    spell_template,
                    effect,
                    targets,
                    header_crypto,
                )
                .await?;
            }

            SpellEffectDispatch::ApplyAura
                if matches!(
                    spell_plan.as_ref().and_then(|plan| plan.channel),
                    Some(SpellPlanChannel::UnitPeriodicTrigger { .. })
                ) && effect.aura_name == SPELL_AURA_PERIODIC_TRIGGER_SPELL =>
            {
                apply_player_periodic_trigger_channel_effect(
                    stream,
                    deps,
                    caster,
                    character_guid,
                    character_level,
                    map_id,
                    spell_template,
                    effect,
                    effect_value_context,
                    targets,
                    now,
                    header_crypto,
                )
                .await?;

                aura_applied = true;
            }

            SpellEffectDispatch::ApplyAura
                if matches!(
                    spell_profile.kind,
                    SpellCastKind::AuraApplication
                        | SpellCastKind::DirectHeal
                        | SpellCastKind::Interrupt
                ) && !aura_applied
                    && {
                        let hostile_effect = spell_plan
                            .as_ref()
                            .is_some_and(|plan| plan.effect_target(effect_index).is_hostile());
                        if spell_has_hostile_direct_damage && hostile_effect && !landed_damage {
                            if !direct_damage_processed {
                                deferred_hostile_aura = true;
                            }

                            false
                        } else {
                            true
                        }
                    } =>
            {
                apply_player_spell_aura(
                    stream,
                    deps,
                    session,
                    caster,
                    character_guid,
                    character_level,
                    map_id,
                    spell_template,
                    spell_profile,
                    targets,
                    effect_value_context,
                    now,
                    header_crypto,
                )
                .await?;

                aura_applied = true;
            }

            SpellEffectDispatch::PersistentAreaAura => {
                apply_player_persistent_area_aura_effect(
                    stream,
                    deps,
                    session,
                    caster,
                    character_guid,
                    character_level,
                    map_id,
                    spell_template,
                    effect_index,
                    effect,
                    effect_value_context,
                    targets,
                    now,
                    header_crypto,
                )
                .await?;
            }

            SpellEffectDispatch::TriggerSpell if effect.trigger_spell != 0 => {
                apply_player_trigger_spell_effect(
                    stream,
                    deps,
                    session,
                    caster,
                    character_guid,
                    character_level,
                    map_id,
                    effect.trigger_spell,
                    targets,
                    now,
                    header_crypto,
                )
                .await?;
            }

            SpellEffectDispatch::Dispel => {
                apply_player_dispel_effect(
                    stream,
                    deps,
                    session,
                    caster,
                    character_guid,
                    map_id,
                    spell_template.id,
                    effect,
                    effect_value_context,
                    targets,
                    now,
                    header_crypto,
                )
                .await?;
            }

            SpellEffectDispatch::DispelMechanic => {
                apply_player_dispel_mechanic_effect(
                    stream,
                    deps,
                    session,
                    caster,
                    character_guid,
                    map_id,
                    spell_template.id,
                    effect,
                    effect_value_context,
                    targets,
                    now,
                    header_crypto,
                )
                .await?;
            }

            SpellEffectDispatch::InterruptCast
                if spell_profile.kind == SpellCastKind::Interrupt =>
            {
                apply_player_interrupt_cast_effect(deps, map_id, spell_template, targets, now)
                    .await?;
            }

            SpellEffectDispatch::LearnSpell
                if effect.trigger_spell != 0 && learned_spells.insert(effect.trigger_spell) =>
            {
                apply_player_learn_spell_effect(
                    stream,
                    deps,
                    session,
                    character_guid,
                    effect.trigger_spell,
                    header_crypto,
                )
                .await?;
            }

            SpellEffectDispatch::Unsupported(effect_id) => {
                let support = spell_effect_support(effect_id);

                warn!(
                    spell_id = spell_template.id,
                    effect_id,
                    effect_name = spell_effect_coverage_name(effect_id),
                    ?support,
                    "Skipping unsupported player spell effect"
                );
            }

            _ => {}
        }
    }

    if deferred_hostile_aura && landed_damage && !aura_applied {
        apply_player_spell_aura(
            stream,
            deps,
            session,
            caster,
            character_guid,
            character_level,
            map_id,
            spell_template,
            spell_profile,
            targets,
            effect_value_context,
            now,
            header_crypto,
        )
        .await?;
    }

    if spell_profile.needs_combo_points && landed_damage {
        clear_player_combo_points_after_finisher(
            stream,
            deps.shared_world,
            caster,
            character_guid,
            map_id,
            header_crypto,
        )
        .await?;
    }

    Ok(())
}
