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
    apply_spell_proc_event_to_active_aura(
        &mut aura,
        spell_template,
        wow_db::get_spell_proc_event_query(deps.world_db_pool, spell_template.id).await?,
    );

    resolve_active_aura_transform_displays(
        deps.shared_world.object_mgr,
        deps.world_db_pool,
        &mut aura,
    )
    .await?;

    let spell_plan = SpellInfo::from_template(spell_template)
        .player_spell_plan()
        .filter(|plan| plan.profile.kind == spell_profile.kind);
    let aura_target = spell_plan
        .as_ref()
        .map(|plan| plan.target)
        .unwrap_or(SpellPlanTarget::Caster);

    match aura_target {
        SpellPlanTarget::Caster => {
            let mut resolution = aura_rank_conflict_resolution(
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

            extend_resolution_with_mechanic_immunity_purges(
                deps.shared_world.object_mgr,
                deps.world_db_pool,
                spell_template,
                &session.auras.active_auras,
                &aura,
                &mut resolution,
            )
            .await?;

            apply_player_aura_replacing_conflicts(session, aura.clone(), &resolution);

            if let Some(event) = deps
                .shared_world
                .maps
                .apply_player_aura_replacing_conflicts(
                    map_id,
                    character_guid,
                    aura.clone(),
                    &resolution,
                )
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
                    &build_player_aura_update_body(
                        caster,
                        session
                            .character
                            .active_character
                            .as_ref()
                            .map(|character| character.class)
                            .unwrap_or(0),
                        session.character.player_stand_state,
                        deps.shared_world
                            .maps
                            .player_runtime_snapshot(map_id, character_guid)
                            .await
                            .map(|snapshot| snapshot.aura_state)
                            .unwrap_or(0),
                        &session.auras.active_auras,
                    )?,
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

            start_player_self_aura_channel_if_needed(
                stream,
                deps.shared_world,
                caster,
                character_guid,
                map_id,
                spell_template,
                now,
                header_crypto,
            )
            .await?;

            apply_warrior_stance_rage_retention_if_needed(
                stream,
                deps,
                session,
                caster,
                character_guid,
                map_id,
                &aura,
                header_crypto,
            )
            .await?;

            apply_linked_warrior_stance_passive_if_needed(
                stream,
                deps,
                session,
                caster,
                character_guid,
                character_level,
                map_id,
                &aura,
                now,
                header_crypto,
            )
            .await?;
        }

        SpellPlanTarget::Unit | SpellPlanTarget::HostileUnit | SpellPlanTarget::FriendlyUnit => {
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

                    let mut resolution = aura_rank_conflict_resolution(
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

                    extend_resolution_with_mechanic_immunity_purges(
                        deps.shared_world.object_mgr,
                        deps.world_db_pool,
                        spell_template,
                        &active_auras,
                        &aura,
                        &mut resolution,
                    )
                    .await?;

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

                    let mut resolution = aura_rank_conflict_resolution(
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

                    extend_resolution_with_mechanic_immunity_purges(
                        deps.shared_world.object_mgr,
                        deps.world_db_pool,
                        spell_template,
                        &target_creature.active_auras,
                        &aura,
                        &mut resolution,
                    )
                    .await?;

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
                        let target_switch = deps
                            .shared_world
                            .maps
                            .switch_db_creature_threat_victim_if_needed(
                                map_id,
                                target,
                                Some(character_guid),
                            )
                            .await?;
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

                        send_db_creature_threat_target_switch(
                            stream,
                            deps.shared_world,
                            session,
                            target_switch,
                            header_crypto,
                        )
                        .await?;
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

        SpellPlanTarget::CasterAreaEnemy { cone } => {
            let spell_info = SpellInfo::from_template(spell_template);

            let Some(effect) = spell_info.effects.into_iter().find(|effect| {
                effect.dispatch == SpellEffectDispatch::ApplyAura
                    && plan_effect_target(*effect).is_hostile()
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

            let targets = if cone {
                let cone_radians = spell_cone_radians_for_spell(deps, spell_template.id).await?;
                deps.shared_world
                    .maps
                    .nearby_attackable_db_creature_guids_in_player_spell_cone(
                        map_id,
                        character_guid,
                        radius,
                        cone_radians,
                    )
                    .await
            } else {
                deps.shared_world
                    .maps
                    .nearby_attackable_db_creature_guids_for_player_spell(
                        map_id,
                        character_guid,
                        radius,
                    )
                    .await
            };

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

        SpellPlanTarget::DestinationAreaEnemy => {
            let spell_info = SpellInfo::from_template(spell_template);

            let Some(effect) = spell_info.effects.into_iter().find(|effect| {
                effect.dispatch == SpellEffectDispatch::ApplyAura
                    && plan_effect_target(*effect) == SpellPlanEffectTarget::DestinationAreaEnemy
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
        SpellPlanTarget::Destination => {}
    }

    Ok(())
}

const LINKED_WARRIOR_STANCE_PASSIVE_SPELL_IDS: [u32; 3] = [21156, 7376, 7381];

fn linked_warrior_stance_passive_spell_id(form: u8) -> Option<u32> {
    match form {
        FORM_BATTLESTANCE => Some(21156),
        FORM_DEFENSIVESTANCE => Some(7376),
        FORM_BERSERKERSTANCE => Some(7381),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
async fn apply_warrior_stance_rage_retention_if_needed(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    character_guid: u32,
    map_id: u32,
    aura: &ActiveAura,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if session
        .character
        .active_character
        .as_ref()
        .is_none_or(|character| character.class != 1)
    {
        return Ok(());
    }
    let Some(form) = active_aura_shapeshift_form(std::slice::from_ref(aura)) else {
        return Ok(());
    };
    if linked_warrior_stance_passive_spell_id(form).is_none() {
        return Ok(());
    }

    // CMaNGOS trims rage on every warrior stance swap inside the shapeshift apply path.
    // Tactical Mastery retention comes from override-class-script talent auras and stays
    // out of scope here until the talent lane is implemented.
    let retained_rage = 0;
    if session.character.player_rage <= retained_rage {
        return Ok(());
    }

    session.character.player_rage = retained_rage;
    deps.shared_world
        .maps
        .set_player_power2(map_id, character_guid, retained_rage)
        .await;

    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &build_player_rage_update_body(caster, retained_rage)?,
        Some(header_crypto),
    )
    .await?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn apply_linked_warrior_stance_passive_if_needed(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    character_guid: u32,
    character_level: u8,
    map_id: u32,
    aura: &ActiveAura,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(form) = active_aura_shapeshift_form(std::slice::from_ref(aura)) else {
        return Ok(());
    };
    let Some(passive_spell_id) = linked_warrior_stance_passive_spell_id(form) else {
        return Ok(());
    };
    let Some(passive_template) = deps
        .shared_world
        .object_mgr
        .spell_template(deps.world_db_pool, passive_spell_id)
        .await?
    else {
        warn!(
            stance_spell_id = aura.spell_id,
            passive_spell_id, "Skipping linked warrior stance passive with no spell_template row"
        );
        return Ok(());
    };
    let value_context = player_spell_effect_value_context(
        deps.shared_world.maps,
        &passive_template,
        &session.character.character_skills,
        0,
    );
    let Some(passive_aura) = passive_spell_active_aura(
        &passive_template,
        caster,
        character_level,
        value_context,
        now,
        deps.shared_world
            .maps
            .spell_duration(passive_template.duration_index),
    ) else {
        return Ok(());
    };

    let resolution = AuraRankConflictResolution {
        failure: None,
        replace_spell_ids: LINKED_WARRIOR_STANCE_PASSIVE_SPELL_IDS.to_vec(),
        replace_any_caster_spell_ids: Vec::new(),
        stack_limit: 1,
    };

    apply_player_aura_replacing_conflicts(session, passive_aura.clone(), &resolution);

    if let Some(event) = deps
        .shared_world
        .maps
        .apply_player_aura_replacing_conflicts(map_id, character_guid, passive_aura, &resolution)
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
    }

    Ok(())
}

async fn extend_resolution_with_mechanic_immunity_purges(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    spell_template: &wow_db::SpellTemplateQuery,
    active_auras: &[ActiveAura],
    aura: &ActiveAura,
    resolution: &mut AuraRankConflictResolution,
) -> anyhow::Result<()> {
    for spell_id in mechanic_immunity_purge_spell_ids(
        object_mgr,
        world_db_pool,
        spell_template,
        active_auras,
        aura,
    )
    .await?
    {
        push_unique_spell_id(&mut resolution.replace_any_caster_spell_ids, spell_id);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn start_player_self_aura_channel_if_needed(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    caster: ObjectGuid,
    character_guid: u32,
    map_id: u32,
    spell_template: &wow_db::SpellTemplateQuery,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(SpellPlanChannel::SelfAura {
        duration_index,
        interrupt_flags,
    }) = SpellInfo::from_template(spell_template)
        .player_spell_plan()
        .and_then(|plan| plan.channel)
    else {
        return Ok(());
    };

    let Some(duration) = shared_world.maps.spell_duration(duration_index) else {
        warn!(
            spell_id = spell_template.id,
            duration_index, "Skipping self-channeled aura start with missing spell duration row"
        );
        return Ok(());
    };
    if duration.duration_millis <= 0 {
        return Ok(());
    }

    let Some(event) = shared_world
        .maps
        .start_player_self_aura_channel(
            map_id,
            caster,
            character_guid,
            spell_template.id,
            duration.duration_millis as u32,
            interrupt_flags,
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
    shared_world.sessions.dispatch(event.observer_packets).await;
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
