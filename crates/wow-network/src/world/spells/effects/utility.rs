use super::*;

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_direct_energize_effect(
    stream: &mut WorldPacketSink,

    deps: SpellCastDeps<'_>,

    session: &mut WorldSessionState,

    caster: ObjectGuid,

    character_guid: u32,

    map_id: u32,

    spell_info: &SpellInfo<'_>,

    value_context: SpellEffectValueContext,

    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let energize = spell_direct_energize(spell_info, value_context);

    if energize == 0 {
        return Ok(());
    }

    if spell_info.template.power_type == POWER_TYPE_RAGE {
        let old_rage = session.character.player_rage;

        session.character.player_rage = session
            .character
            .player_rage
            .saturating_add(energize)
            .min(POWER_RAGE_DEFAULT);

        let amount = session.character.player_rage.saturating_sub(old_rage);

        if amount == 0 {
            return Ok(());
        }

        deps.shared_world
            .maps
            .set_player_power2(map_id, character_guid, session.character.player_rage)
            .await;

        send_packet(
            stream,
            WorldOpcode::SmsgSpellEnergizeLog as u16,
            &build_spell_energize_log_body(
                caster,
                caster,
                spell_info.template.id,
                POWER_TYPE_RAGE,
                amount,
            )?,
            Some(&mut *header_crypto),
        )
        .await?;

        send_packet(
            stream,
            WorldOpcode::SmsgUpdateObject as u16,
            &build_player_rage_update_body(caster, session.character.player_rage)?,
            Some(header_crypto),
        )
        .await?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_dispel_effect(
    stream: &mut WorldPacketSink,

    deps: SpellCastDeps<'_>,

    session: &mut WorldSessionState,

    caster: ObjectGuid,

    character_guid: u32,

    map_id: u32,

    spell_id: u32,

    effect: SpellInfoEffect,

    value_context: SpellEffectValueContext,

    targets: &SpellCastTargets,

    now: Instant,

    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Ok(dispel_type) = u32::try_from(effect.misc_value) else {
        return Ok(());
    };

    if dispel_type == 0 {
        return Ok(());
    }

    let count = spell_effect_calculated_u32(effect, value_context)
        .unwrap_or(1)
        .max(1);

    let target = targets.unit_target.unwrap_or(caster);

    if target.is_player() {
        let target_character_guid = target.counter();

        if target_character_guid == character_guid {
            remove_session_auras_by_dispel_type(
                &mut session.auras.active_auras,
                dispel_type,
                count,
            );
        }

        let Some(event) = deps
            .shared_world
            .maps
            .remove_player_auras_by_dispel_type(
                map_id,
                target_character_guid,
                dispel_type,
                count,
                now,
            )
            .await?
        else {
            return Ok(());
        };

        send_packet(
            stream,
            WorldOpcode::SmsgSpellDispelLog as u16,
            &build_spell_dispel_log_body(target, caster, &event.removed_spell_ids)?,
            Some(&mut *header_crypto),
        )
        .await?;

        send_or_dispatch_player_aura_event(
            stream,
            deps.shared_world,
            character_guid,
            target_character_guid,
            event.aura_update,
            header_crypto,
        )
        .await?;
    } else if target.is_creature() {
        let Some(event) = deps
            .shared_world
            .maps
            .remove_db_creature_auras_by_dispel_type(
                map_id,
                target,
                character_guid,
                dispel_type,
                count,
                now,
            )
            .await?
        else {
            return Ok(());
        };

        send_packet(
            stream,
            WorldOpcode::SmsgSpellDispelLog as u16,
            &build_spell_dispel_log_body(target, caster, &event.removed_spell_ids)?,
            Some(&mut *header_crypto),
        )
        .await?;

        send_packet(
            stream,
            WorldOpcode::SmsgUpdateObject as u16,
            &event.aura_update.update_body,
            Some(&mut *header_crypto),
        )
        .await?;

        for packet in event.aura_update.direct_packets {
            send_packet(
                stream,
                packet.opcode,
                &packet.body,
                Some(&mut *header_crypto),
            )
            .await?;
        }

        deps.shared_world
            .sessions
            .dispatch(event.aura_update.observer_packets)
            .await;
    }

    debug!(spell_id, dispel_type, count, "Applied player dispel effect");

    Ok(())
}

pub(in crate::world) fn remove_session_auras_by_dispel_type(
    active_auras: &mut Vec<ActiveAura>,

    dispel_type: u32,

    count: u32,
) -> Vec<u32> {
    let mut remaining = count.max(1) as usize;

    let mut removed = Vec::new();

    active_auras.retain(|aura| {
        if remaining == 0 || !active_aura_matches_dispel_type(aura, dispel_type) {
            return true;
        }

        removed.push(aura.spell_id);

        remaining -= 1;

        false
    });

    removed
}

pub(in crate::world) fn build_spell_dispel_log_body(
    target: ObjectGuid,

    caster: ObjectGuid,

    removed_spell_ids: &[u32],
) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(8 + 8 + 4 + removed_spell_ids.len() * 4);

    body.extend_from_slice(&target.raw().to_le_bytes());

    body.extend_from_slice(&caster.raw().to_le_bytes());

    body.extend_from_slice(&(removed_spell_ids.len() as u32).to_le_bytes());

    for spell_id in removed_spell_ids {
        body.extend_from_slice(&spell_id.to_le_bytes());
    }

    Ok(body)
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_trigger_spell_effect(
    stream: &mut WorldPacketSink,

    deps: SpellCastDeps<'_>,

    session: &mut WorldSessionState,

    caster: ObjectGuid,

    character_guid: u32,

    character_level: u8,

    map_id: u32,

    triggered_spell_id: u32,

    targets: &SpellCastTargets,

    now: Instant,

    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(triggered_template) = deps
        .shared_world
        .object_mgr
        .spell_template(deps.world_db_pool, triggered_spell_id)
        .await?
    else {
        warn!(
            triggered_spell_id,
            "Skipping trigger-spell effect with missing spell_template row"
        );

        return Ok(());
    };

    let triggered_info = SpellInfo::from_template(&triggered_template);

    let Some(triggered_profile) = triggered_info.player_cast_profile() else {
        warn!(
            triggered_spell_id,
            "Skipping trigger-spell effect with unsupported triggered spell shape"
        );

        return Ok(());
    };

    let triggered_value_context = player_spell_effect_value_context(
        deps.shared_world.maps,
        &triggered_template,
        &session.character.character_skills,
        0,
    );

    match triggered_profile.kind {
        SpellCastKind::AuraApplication | SpellCastKind::DirectHeal => {
            apply_player_spell_aura(
                stream,
                deps,
                session,
                caster,
                character_guid,
                character_level,
                map_id,
                &triggered_template,
                &triggered_profile,
                targets,
                triggered_value_context,
                now,
                header_crypto,
            )
            .await?;
        }

        SpellCastKind::InstantDamage => {
            for effect in triggered_info.effects {
                if let Some(damage_effect) = player_direct_damage_effect(
                    &triggered_template,
                    &triggered_profile,
                    effect,
                    triggered_value_context,
                ) {
                    apply_player_direct_damage_effect(
                        stream,
                        deps,
                        session,
                        caster,
                        character_guid,
                        map_id,
                        damage_effect,
                        targets,
                        None,
                        header_crypto,
                    )
                    .await?;
                }
            }
        }

        _ => {}
    }

    Ok(())
}

pub(in crate::world) async fn apply_player_learn_spell_effect(
    stream: &mut WorldPacketSink,

    deps: SpellCastDeps<'_>,

    session: &mut WorldSessionState,

    character_guid: u32,

    learned_spell_id: u32,

    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if learned_spell_id == 0 || session.character.active_spells.contains(&learned_spell_id) {
        return Ok(());
    }

    let Some(_) =
        wow_db::learn_character_spell(deps.character_db_pool, character_guid, learned_spell_id, 0)
            .await?
    else {
        return Ok(());
    };

    session.character.active_spells.insert(learned_spell_id);

    send_packet(
        stream,
        WorldOpcode::SmsgLearnedSpell as u16,
        &build_learned_spell_body(learned_spell_id),
        Some(&mut *header_crypto),
    )
    .await?;

    let known_spells = wow_db::get_character_spells(deps.character_db_pool, character_guid).await?;

    send_known_proficiencies(
        stream,
        deps.world_db_pool,
        &known_spells,
        Some(&mut *header_crypto),
    )
    .await?;

    send_packet(
        stream,
        WorldOpcode::SmsgInitialSpells as u16,
        &build_initial_spells_body(&known_spells),
        Some(header_crypto),
    )
    .await
}
