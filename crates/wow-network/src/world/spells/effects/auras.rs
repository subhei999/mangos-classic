use super::*;

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_combo_points_effect(
    stream: &mut WorldPacketSink,

    shared_world: SharedWorldDeps<'_>,

    caster: ObjectGuid,

    character_guid: u32,

    map_id: u32,

    effect: SpellInfoEffect,

    targets: &SpellCastTargets,

    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(target) = targets.unit_target else {
        return Ok(());
    };

    let Some(points) = spell_effect_simple_value(effect.base_points) else {
        return Ok(());
    };

    let Some(event) = shared_world
        .maps
        .add_player_combo_points(map_id, character_guid, target, points as u8)
        .await
    else {
        return Ok(());
    };

    let body = build_player_combo_points_update_body(
        caster,
        event.combo_target,
        event.combo_points,
        event.player_bytes,
    )?;

    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &body,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn clear_player_combo_points_after_finisher(
    stream: &mut WorldPacketSink,

    shared_world: SharedWorldDeps<'_>,

    caster: ObjectGuid,

    character_guid: u32,

    map_id: u32,

    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(event) = shared_world
        .maps
        .clear_player_combo_points(map_id, character_guid)
        .await
    else {
        return Ok(());
    };

    let body = build_player_combo_points_update_body(
        caster,
        event.combo_target,
        event.combo_points,
        event.player_bytes,
    )?;

    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &body,
        Some(header_crypto),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_spell_aura(
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

    value_context: SpellEffectValueContext,

    now: Instant,

    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let mut aura = build_active_aura(
        spell_template,
        caster,
        character_level,
        value_context,
        now,
        deps.shared_world
            .maps
            .spell_duration(spell_template.duration_index),
    );

    resolve_active_aura_transform_displays(
        deps.shared_world.object_mgr,
        deps.world_db_pool,
        &mut aura,
    )
    .await?;

    match spell_profile.aura_target {
        SpellAuraTarget::Caster => {
            let resolution = aura_rank_conflict_resolution(
                deps.shared_world.object_mgr,
                deps.world_db_pool,
                spell_template.id,
                caster,
                &session.auras.active_auras,
            )
            .await?;

            if resolution.failure.is_some() {
                return Ok(());
            }

            apply_player_aura_replacing_conflicts(session, aura.clone(), &resolution);

            if let Some(event) = deps
                .shared_world
                .maps
                .apply_player_aura_replacing_conflicts(map_id, character_guid, aura, &resolution)
                .await?
            {
                send_or_dispatch_player_aura_event(
                    stream,
                    deps.shared_world,
                    character_guid,
                    character_guid,
                    event,
                    header_crypto,
                )
                .await?;
            } else {
                send_packet(
                    stream,
                    WorldOpcode::SmsgUpdateObject as u16,
                    &build_player_aura_update_body(caster, &session.auras.active_auras)?,
                    Some(&mut *header_crypto),
                )
                .await?;

                for packet in
                    build_player_aura_duration_update_packets(&session.auras.active_auras, now)
                {
                    send_packet(
                        stream,
                        packet.opcode,
                        &packet.body,
                        Some(&mut *header_crypto),
                    )
                    .await?;
                }
            }
        }

        SpellAuraTarget::UnitTarget => {
            if let Some(target) = targets.unit_target {
                if target.is_player() {
                    let target_character_guid = target.counter();

                    let active_auras = if target_character_guid == character_guid {
                        session.auras.active_auras.clone()
                    } else {
                        let Some(snapshot) = deps
                            .shared_world
                            .maps
                            .player_runtime_snapshot(map_id, target_character_guid)
                            .await
                        else {
                            return Ok(());
                        };

                        snapshot.active_auras
                    };

                    let resolution = aura_rank_conflict_resolution(
                        deps.shared_world.object_mgr,
                        deps.world_db_pool,
                        spell_template.id,
                        caster,
                        &active_auras,
                    )
                    .await?;

                    if resolution.failure.is_some() {
                        return Ok(());
                    }

                    if target_character_guid == character_guid {
                        apply_player_aura_replacing_conflicts(session, aura.clone(), &resolution);
                    }

                    if let Some(event) = deps
                        .shared_world
                        .maps
                        .apply_player_aura_replacing_conflicts(
                            map_id,
                            target_character_guid,
                            aura,
                            &resolution,
                        )
                        .await?
                    {
                        send_or_dispatch_player_aura_event(
                            stream,
                            deps.shared_world,
                            character_guid,
                            target_character_guid,
                            event,
                            header_crypto,
                        )
                        .await?;
                    }
                } else if target.is_creature() {
                    let Some(target_creature) = deps
                        .shared_world
                        .maps
                        .db_creature_snapshot(map_id, target)
                        .await
                    else {
                        return Ok(());
                    };

                    augment_mage_polymorph_regen_from_helper_spell(
                        deps.shared_world.object_mgr,
                        deps.world_db_pool,
                        spell_template,
                        &mut aura,
                        now,
                        target_creature.max_health(),
                    )
                    .await?;

                    let single_target_descriptor = single_target_aura_descriptor(
                        deps.shared_world.object_mgr,
                        deps.world_db_pool,
                        spell_template,
                    )
                    .await?;

                    let diminishing_group = db_creature_spell_diminishing_group(spell_template);

                    if let Some(group) = diminishing_group {
                        let level = deps
                            .shared_world
                            .maps
                            .current_diminishing_level(map_id, target, group, now)
                            .await
                            .unwrap_or(DiminishingLevelRuntime::Level1);

                        let adjusted_duration =
                            diminishing_duration_millis(aura.duration_millis, level).unwrap_or(0);

                        if adjusted_duration == 0 {
                            begin_db_creature_retaliation_if_needed(
                                stream,
                                deps.shared_world,
                                map_id,
                                session,
                                target,
                                caster,
                                header_crypto,
                            )
                            .await?;

                            return Ok(());
                        }

                        aura.duration_millis = Some(adjusted_duration);

                        aura.expires_at =
                            Some(now + Duration::from_millis(adjusted_duration as u64));
                    }

                    let resolution = aura_rank_conflict_resolution(
                        deps.shared_world.object_mgr,
                        deps.world_db_pool,
                        spell_template.id,
                        caster,
                        &target_creature.active_auras,
                    )
                    .await?;

                    if resolution.failure.is_some() {
                        begin_db_creature_retaliation_if_needed(
                            stream,
                            deps.shared_world,
                            map_id,
                            session,
                            target,
                            caster,
                            header_crypto,
                        )
                        .await?;

                        return Ok(());
                    }

                    if let Some(event) = deps
                        .shared_world
                        .maps
                        .apply_db_creature_aura_replacing_conflicts(
                            map_id,
                            target,
                            character_guid,
                            aura,
                            &resolution,
                            single_target_descriptor,
                            diminishing_group,
                            now,
                        )
                        .await?
                    {
                        send_packet(
                            stream,
                            WorldOpcode::SmsgUpdateObject as u16,
                            &event.update_body,
                            Some(&mut *header_crypto),
                        )
                        .await?;

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
                    }

                    begin_db_creature_retaliation_if_needed(
                        stream,
                        deps.shared_world,
                        map_id,
                        session,
                        target,
                        caster,
                        header_crypto,
                    )
                    .await?;
                }
            }
        }

        SpellAuraTarget::CasterAreaEnemy => {
            let spell_info = SpellInfo::from_template(spell_template);

            let Some(effect) = spell_info.effects.into_iter().find(|effect| {
                effect.dispatch == SpellEffectDispatch::ApplyAura
                    && effect_targets_caster_centered_hostile_area(*effect)
            }) else {
                return Ok(());
            };

            let Some(radius) = spell_effect_radius_yards(deps.shared_world.maps, effect) else {
                warn!(
                    spell_id = spell_template.id,
                    radius_index = effect.radius_index,
                    "Skipping caster-centered AoE aura with missing SpellRadius.dbc row"
                );

                return Ok(());
            };

            let targets = deps
                .shared_world
                .maps
                .nearby_attackable_db_creature_guids_for_player_spell(
                    map_id,
                    character_guid,
                    radius,
                )
                .await;

            for target in targets {
                let Some(target_creature) = deps
                    .shared_world
                    .maps
                    .db_creature_snapshot(map_id, target)
                    .await
                else {
                    continue;
                };

                let resolution = aura_rank_conflict_resolution(
                    deps.shared_world.object_mgr,
                    deps.world_db_pool,
                    spell_template.id,
                    caster,
                    &target_creature.active_auras,
                )
                .await?;

                if resolution.failure.is_some() {
                    continue;
                }

                if let Some(event) = deps
                    .shared_world
                    .maps
                    .apply_db_creature_aura_replacing_conflicts(
                        map_id,
                        target,
                        character_guid,
                        aura.clone(),
                        &resolution,
                        None,
                        None,
                        now,
                    )
                    .await?
                {
                    send_packet(
                        stream,
                        WorldOpcode::SmsgUpdateObject as u16,
                        &event.update_body,
                        Some(&mut *header_crypto),
                    )
                    .await?;

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
                }

                begin_db_creature_retaliation_if_needed(
                    stream,
                    deps.shared_world,
                    map_id,
                    session,
                    target,
                    caster,
                    header_crypto,
                )
                .await?;
            }
        }

        SpellAuraTarget::DestinationAreaEnemy => {
            let spell_info = SpellInfo::from_template(spell_template);

            let Some(effect) = spell_info.effects.into_iter().find(|effect| {
                effect.dispatch == SpellEffectDispatch::ApplyAura
                    && effect_targets_destination_hostile_area(*effect)
            }) else {
                return Ok(());
            };

            let Some(radius) = spell_effect_radius_yards(deps.shared_world.maps, effect) else {
                warn!(
                    spell_id = spell_template.id,
                    radius_index = effect.radius_index,
                    "Skipping destination AoE aura with missing SpellRadius.dbc row"
                );

                return Ok(());
            };

            let Some(destination) = spell_target_destination_position(map_id, targets) else {
                warn!(
                    spell_id = spell_template.id,
                    "Skipping destination AoE aura with missing target destination"
                );

                return Ok(());
            };

            let targets = deps
                .shared_world
                .maps
                .nearby_attackable_db_creature_guids_for_player_spell_at_position(
                    map_id,
                    character_guid,
                    destination,
                    radius,
                )
                .await;

            for target in targets {
                let Some(target_creature) = deps
                    .shared_world
                    .maps
                    .db_creature_snapshot(map_id, target)
                    .await
                else {
                    continue;
                };

                let resolution = aura_rank_conflict_resolution(
                    deps.shared_world.object_mgr,
                    deps.world_db_pool,
                    spell_template.id,
                    caster,
                    &target_creature.active_auras,
                )
                .await?;

                if resolution.failure.is_some() {
                    continue;
                }

                if let Some(event) = deps
                    .shared_world
                    .maps
                    .apply_db_creature_aura_replacing_conflicts(
                        map_id,
                        target,
                        character_guid,
                        aura.clone(),
                        &resolution,
                        None,
                        None,
                        now,
                    )
                    .await?
                {
                    send_packet(
                        stream,
                        WorldOpcode::SmsgUpdateObject as u16,
                        &event.update_body,
                        Some(&mut *header_crypto),
                    )
                    .await?;

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
                }

                begin_db_creature_retaliation_if_needed(
                    stream,
                    deps.shared_world,
                    map_id,
                    session,
                    target,
                    caster,
                    header_crypto,
                )
                .await?;
            }
        }
    }

    Ok(())
}

async fn augment_mage_polymorph_regen_from_helper_spell(
    object_mgr: &ObjectMgr,

    world_db_pool: &MySqlPool,

    spell_template: &wow_db::SpellTemplateQuery,

    aura: &mut ActiveAura,

    now: Instant,

    target_max_health: u32,
) -> anyhow::Result<()> {
    if !spell_is_mage_polymorph(spell_template) {
        return Ok(());
    }

    let Some(helper_template) = object_mgr
        .spell_template(world_db_pool, POLYMORPH_HELPER_REGEN_SPELL_ID)
        .await?
    else {
        warn!(
            spell_id = spell_template.id,
            helper_spell_id = POLYMORPH_HELPER_REGEN_SPELL_ID,
            "Mage polymorph helper regen spell_template row is missing"
        );

        return Ok(());
    };

    let helper_context = SpellEffectValueContext::unranked(&helper_template, 0);

    let Some(mut regen) = spell_periodic_regen_aura(
        &SpellInfo::from_template(&helper_template),
        helper_context,
        now,
    ) else {
        warn!(
            spell_id = spell_template.id,
            helper_spell_id = POLYMORPH_HELPER_REGEN_SPELL_ID,
            "Mage polymorph helper regen spell has no periodic regen aura payload"
        );

        return Ok(());
    };

    regen.health_amount = (target_max_health / 10).max(1);

    aura.periodic_regen = Some(regen);

    Ok(())
}
