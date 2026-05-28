use super::*;

pub(in crate::world) fn spell_effect_radius_yards(
    maps: &MapRuntimeManager,

    effect: SpellInfoEffect,
) -> Option<f32> {
    maps.spell_radius(effect.radius_index)
        .map(|entry| entry.radius)
        .filter(|radius| *radius > 0.0)
}

pub(in crate::world) fn spell_direct_heal(
    spell_info: &SpellInfo<'_>,

    value_context: SpellEffectValueContext,
) -> u32 {
    spell_info
        .effects
        .into_iter()
        .filter(|effect| effect.dispatch == SpellEffectDispatch::Heal)
        .filter_map(|effect| spell_effect_calculated_u32(effect, value_context))
        .sum()
}

pub(in crate::world) fn spell_direct_energize(
    spell_info: &SpellInfo<'_>,

    value_context: SpellEffectValueContext,
) -> u32 {
    spell_info
        .effects
        .into_iter()
        .filter(|effect| effect.dispatch == SpellEffectDispatch::Energize)
        .filter_map(|effect| spell_effect_calculated_u32(effect, value_context))
        .sum()
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_persistent_area_aura_effect(
    stream: &mut WorldPacketSink,

    deps: SpellCastDeps<'_>,

    _session: &mut WorldSessionState,

    caster: ObjectGuid,

    character_guid: u32,

    character_level: u8,

    map_id: u32,

    spell_template: &wow_db::SpellTemplateQuery,

    effect_index: usize,

    effect: SpellInfoEffect,

    value_context: SpellEffectValueContext,

    targets: &SpellCastTargets,

    now: Instant,

    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(origin) = (match plan_effect_target(effect) {
        SpellPlanEffectTarget::CasterAreaEnemy { .. } => deps
            .shared_world
            .maps
            .player_runtime_snapshot(map_id, character_guid)
            .await
            .map(|snapshot| snapshot.position),
        SpellPlanEffectTarget::DestinationAreaEnemy => {
            spell_target_destination_position(map_id, targets)
        }
        _ => spell_target_destination_position(map_id, targets),
    }) else {
        warn!(
            spell_id = spell_template.id,
            effect_target = ?plan_effect_target(effect),
            "Skipping persistent area aura with missing origin position"
        );

        return Ok(());
    };

    let Some(radius) = spell_effect_radius_yards(deps.shared_world.maps, effect) else {
        warn!(
            spell_id = spell_template.id,
            radius_index = effect.radius_index,
            "Skipping persistent area aura with missing SpellRadius.dbc row"
        );

        return Ok(());
    };

    let channel = SpellInfo::from_template(spell_template).plan_channel();
    let (channeled, duration_index, channel_interrupt_flags) = match channel {
        Some(SpellPlanChannel::PersistentArea {
            duration_index,
            interrupt_flags,
        }) => (true, duration_index, interrupt_flags),
        _ => (false, spell_template.duration_index, 0),
    };

    let Some(duration) = deps
        .shared_world
        .maps
        .spell_duration(duration_index)
        .map(|duration| duration.duration_millis)
        .filter(|duration| *duration > 0)
    else {
        warn!(
            spell_id = spell_template.id,
            duration_index,
            "Skipping persistent area aura with missing positive SpellDuration.dbc row"
        );

        return Ok(());
    };

    let periodic_damage = persistent_area_periodic_damage(
        spell_template,
        effect,
        character_level,
        value_context,
        now,
    );

    let Some(event) = deps
        .shared_world
        .maps
        .create_persistent_area_dynamic_object(
            map_id,
            caster,
            character_guid,
            spell_template.id,
            effect_index,
            origin,
            radius,
            duration as u32,
            periodic_damage,
            channeled,
            channel_interrupt_flags,
            now,
        )
        .await?
    else {
        return Ok(());
    };

    for packet in event.direct_packets {
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
        .dispatch(event.observer_packets)
        .await;

    Ok(())
}

pub(in crate::world) fn persistent_area_periodic_damage(
    spell_template: &wow_db::SpellTemplateQuery,

    effect: SpellInfoEffect,

    caster_level: u8,

    value_context: SpellEffectValueContext,

    now: Instant,
) -> Option<PeriodicDamageAura> {
    if effect.aura_name != SPELL_AURA_PERIODIC_DAMAGE || effect.amplitude == 0 {
        return None;
    }

    let damage = spell_effect_calculated_u32(effect, value_context)?;

    Some(PeriodicDamageAura {
        aura_name: effect.aura_name,

        school: spell_template.school,

        damage_class: spell_template.dmg_class,

        attributes_ex2: spell_template.attributes_ex2,

        attributes_ex3: spell_template.attributes_ex3,

        caster_snapshot: spell_periodic_damage_fallback_caster_snapshot(caster_level),

        profile: PeriodicDamageProfile::Flat,

        amount: damage,

        tick_millis: effect.amplitude,

        next_tick_at: now + Duration::from_millis(effect.amplitude as u64),
    })
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_periodic_trigger_channel_effect(
    stream: &mut WorldPacketSink,

    deps: SpellCastDeps<'_>,

    caster: ObjectGuid,

    character_guid: u32,

    character_level: u8,

    map_id: u32,

    spell_template: &wow_db::SpellTemplateQuery,

    effect: SpellInfoEffect,

    _value_context: SpellEffectValueContext,

    targets: &SpellCastTargets,

    now: Instant,

    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(SpellPlanChannel::UnitPeriodicTrigger {
        trigger_spell,
        tick_millis,
        duration_index,
        interrupt_flags,
    }) = SpellInfo::from_template(spell_template).plan_channel()
    else {
        return Ok(());
    };

    let Some(target) = targets.unit_target else {
        warn!(
            spell_id = spell_template.id,
            "Skipping periodic trigger channel with missing unit target"
        );

        return Ok(());
    };

    if trigger_spell == 0 || tick_millis == 0 || effect.trigger_spell != trigger_spell {
        warn!(
            spell_id = spell_template.id,
            effect_trigger_spell = trigger_spell,
            effect_amplitude = tick_millis,
            "Skipping periodic trigger channel with incomplete trigger data"
        );

        return Ok(());
    }

    let Some(duration) = deps
        .shared_world
        .maps
        .spell_duration(duration_index)
        .map(|duration| duration.duration_millis)
        .filter(|duration| *duration > 0)
    else {
        warn!(
            spell_id = spell_template.id,
            duration_index,
            "Skipping periodic trigger channel with missing positive SpellDuration.dbc row"
        );

        return Ok(());
    };

    let max_range = deps
        .shared_world
        .maps
        .spell_range(spell_template.range_index)
        .map(|range| range.max_range)
        .unwrap_or(0.0);

    let Some(triggered_template) = deps
        .shared_world
        .object_mgr
        .spell_template(deps.world_db_pool, trigger_spell)
        .await?
    else {
        warn!(
            spell_id = spell_template.id,
            triggered_spell_id = trigger_spell,
            "Skipping periodic trigger channel with missing triggered spell_template row"
        );

        return Ok(());
    };

    let triggered_info = SpellInfo::from_template(&triggered_template);

    let Some(triggered_profile) = triggered_info.player_cast_profile() else {
        warn!(
            spell_id = spell_template.id,
            triggered_spell_id = trigger_spell,
            "Skipping periodic trigger channel with unsupported triggered spell shape"
        );

        return Ok(());
    };

    let triggered_value_context = SpellEffectValueContext::with_spell_rank_level(
        &triggered_template,
        character_level as i32,
        0,
    );

    let Some(damage_effect) = triggered_info
        .effects
        .into_iter()
        .find_map(|triggered_effect| {
            player_direct_damage_effect(
                &triggered_template,
                &triggered_profile,
                triggered_effect,
                triggered_value_context,
            )
        })
    else {
        warn!(
            spell_id = spell_template.id,
            triggered_spell_id = trigger_spell,
            "Skipping periodic trigger channel whose triggered spell has no direct damage effect"
        );

        return Ok(());
    };

    let Some(event) = deps
        .shared_world
        .maps
        .start_player_periodic_trigger_channel(
            map_id,
            caster,
            character_guid,
            spell_template.id,
            target,
            duration as u32,
            tick_millis,
            max_range,
            damage_effect,
            interrupt_flags,
            triggered_template.speed,
            now,
        )
        .await?
    else {
        return Ok(());
    };

    for packet in event.direct_packets {
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
        .dispatch(event.observer_packets)
        .await;

    Ok(())
}
