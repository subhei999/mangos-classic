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
pub(in crate::world) async fn apply_player_power_burn_effect(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    character_guid: u32,
    map_id: u32,
    spell_template: &wow_db::SpellTemplateQuery,
    effect: SpellInfoEffect,
    value_context: SpellEffectValueContext,
    targets: &SpellCastTargets,
    target_outcome: Option<PlayerSpellTargetOutcome>,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    let Some(target) = targets.unit_target.filter(|target| target.is_creature()) else {
        return Ok(false);
    };
    if target_outcome
        .filter(|outcome| outcome.target == target)
        .is_some_and(|outcome| outcome.miss_info.is_some())
    {
        return Ok(false);
    }

    let Some(mut target_creature) = deps
        .shared_world
        .maps
        .db_creature_snapshot(map_id, target)
        .await
    else {
        return Ok(false);
    };
    let Ok(required_power_type) = u32::try_from(effect.misc_value) else {
        return Ok(false);
    };
    if creature_unit_power_type(&target_creature.spawn.template) != required_power_type {
        return Ok(false);
    }

    let Some(requested_burn) = spell_effect_calculated_u32(effect, value_context) else {
        return Ok(false);
    };
    let burned_power = target_creature.power1.min(requested_burn);
    if burned_power == 0 {
        return Ok(false);
    }

    target_creature.power1 = target_creature.power1.saturating_sub(burned_power);
    mirror_session_db_creature(session, target.raw(), target_creature.clone());
    let power_update_body = build_db_creature_power_update_body(target, target_creature.power1)?;
    let observer_packets = deps
        .shared_world
        .maps
        .update_db_creature_snapshot_and_broadcast(
            map_id,
            target_creature,
            Some(character_guid),
            OutboundWorldPacket {
                opcode: WorldOpcode::SmsgUpdateObject as u16,
                body: power_update_body.clone(),
            },
        )
        .await;

    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &power_update_body,
        Some(&mut *header_crypto),
    )
    .await?;
    deps.shared_world.sessions.dispatch(observer_packets).await;

    let damage = ((burned_power as f32) * effect.multiple_value.max(0.0)).trunc() as u32;
    if damage == 0 {
        return Ok(false);
    }

    apply_db_creature_spell_damage(
        stream,
        deps,
        session,
        caster,
        character_guid,
        map_id,
        PlayerDirectDamageEffect {
            spell_id: spell_template.id,
            damage,
            weapon_damage_percent: 100,
            school: spell_template.school as u8,
            dmg_class: spell_template.dmg_class,
            attributes_ex2: spell_template.attributes_ex2,
            attributes_ex3: spell_template.attributes_ex3,
            requires_melee: false,
            uses_weapon_outcome: false,
            suppress_attacker_state: true,
            caster_centered_hostile_area: false,
            destination_hostile_area: false,
            caster_centered_hostile_cone: false,
            radius_index: 0,
        },
        targets,
        target_outcome,
        header_crypto,
    )
    .await
}

pub(in crate::world) async fn apply_player_taunt_effect(
    deps: SpellCastDeps<'_>,
    caster: ObjectGuid,
    map_id: u32,
    targets: &SpellCastTargets,
) -> anyhow::Result<()> {
    let Some(target) = targets.unit_target.filter(|target| target.is_creature()) else {
        return Ok(());
    };

    deps.shared_world
        .maps
        .apply_db_creature_taunt_threat(map_id, target, caster)
        .await;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_threat_effect(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    character_guid: u32,
    map_id: u32,
    spell_template: &wow_db::SpellTemplateQuery,
    effect: SpellInfoEffect,
    value_context: SpellEffectValueContext,
    targets: &SpellCastTargets,
    target_outcome: Option<PlayerSpellTargetOutcome>,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(target) = targets.unit_target.filter(|target| target.is_creature()) else {
        return Ok(());
    };
    if target_outcome
        .filter(|outcome| outcome.target == target)
        .is_some_and(|outcome| outcome.miss_info.is_some())
    {
        return Ok(());
    }

    let threat = spell_effect_calculated_i32(effect, value_context) as f32;
    deps.shared_world
        .maps
        .add_db_creature_threat_with_school_mask(
            map_id,
            target,
            caster,
            threat,
            spell_school_mask_from_school(spell_template.school),
        )
        .await;

    let target_switch = deps
        .shared_world
        .maps
        .switch_db_creature_threat_victim_if_needed(map_id, target, Some(character_guid))
        .await?;
    send_db_creature_threat_target_switch(
        stream,
        deps.shared_world,
        session,
        target_switch,
        header_crypto,
    )
    .await?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_distract_effect(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    character_guid: u32,
    map_id: u32,
    effect: SpellInfoEffect,
    value_context: SpellEffectValueContext,
    targets: &SpellCastTargets,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(destination) = targets.destination else {
        return Ok(());
    };
    let Some(radius) = spell_effect_radius_yards(deps.shared_world.maps, effect) else {
        warn!(
            radius_index = effect.radius_index,
            "Skipping distract effect with missing SpellRadius.dbc row"
        );
        return Ok(());
    };
    let Some(duration_secs) = spell_effect_calculated_u32(effect, value_context) else {
        return Ok(());
    };
    let distract_position =
        WorldPosition::new(map_id, destination.x, destination.y, destination.z, 0.0);
    let distract_until = now + Duration::from_millis(u64::from(duration_secs) * 1_000);
    let targets = deps
        .shared_world
        .maps
        .nearby_attackable_db_creature_guids_for_player_spell_at_position(
            map_id,
            character_guid,
            distract_position,
            radius,
        )
        .await;

    for target in targets {
        let Some(update) = deps
            .shared_world
            .maps
            .apply_db_creature_distract(map_id, target, distract_position, distract_until)
            .await
        else {
            continue;
        };

        mirror_session_db_creature(session, target.raw(), update.creature.clone());

        if let Some(stop) = update.stop {
            let body = build_monster_move_stop_body(target, stop.position, stop.spline_id)?;
            send_packet(
                stream,
                WorldOpcode::SmsgMonsterMove as u16,
                &body,
                Some(&mut *header_crypto),
            )
            .await?;
            let packets = deps
                .shared_world
                .maps
                .broadcast_nearby_player_packet(
                    map_id,
                    character_guid,
                    PLAYER_VISIBILITY_RADIUS_YARDS,
                    OutboundWorldPacket {
                        opcode: WorldOpcode::SmsgMonsterMove as u16,
                        body,
                    },
                )
                .await;
            deps.shared_world.sessions.dispatch(packets).await;
        }

        let body = build_monster_move_facing_spot_body(
            target,
            update.facing_position,
            update.facing_position,
            update.facing_spline_id,
            1,
            distract_position,
        )?;
        send_packet(
            stream,
            WorldOpcode::SmsgMonsterMove as u16,
            &body,
            Some(&mut *header_crypto),
        )
        .await?;
        let packets = deps
            .shared_world
            .maps
            .broadcast_nearby_player_packet(
                map_id,
                character_guid,
                PLAYER_VISIBILITY_RADIUS_YARDS,
                OutboundWorldPacket {
                    opcode: WorldOpcode::SmsgMonsterMove as u16,
                    body,
                },
            )
            .await;
        deps.shared_world.sessions.dispatch(packets).await;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_pickpocket_effect(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    character_guid: u32,
    character_level: u8,
    map_id: u32,
    targets: &SpellCastTargets,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(target) = targets.unit_target.filter(|target| target.is_creature()) else {
        return Ok(());
    };
    let Some(creature) = deps
        .shared_world
        .maps
        .db_creature_snapshot(map_id, target)
        .await
    else {
        return Ok(());
    };
    let loot_entry = creature.spawn.template.pickpocket_loot_id;
    if loot_entry == 0 {
        return Ok(());
    }
    if creature.pickpocket_is_on_cooldown(now) {
        let body = build_loot_error_response_body(target, LOOT_ERROR_ALREADY_PICKPOCKETED);
        send_packet(
            stream,
            WorldOpcode::SmsgLootResponse as u16,
            &body,
            Some(header_crypto),
        )
        .await?;
        return Ok(());
    }

    let loot_items =
        if creature.loot_kind == DbCreatureLootKind::Pickpocket && creature.loot_items_generated {
            Vec::new()
        } else {
            select_db_pickpocket_loot_item_for_character(
                deps.shared_world.object_mgr,
                deps.world_db_pool,
                session,
                loot_entry,
            )
            .await?
        };
    let loot_money =
        if creature.loot_kind == DbCreatureLootKind::Pickpocket && creature.loot_items_generated {
            creature.loot_money()
        } else {
            cmangos_pickpocket_loot_money(creature.spawn.template.min_level, character_level)
        };
    let Some(creature) = deps
        .shared_world
        .maps
        .open_db_creature_pickpocket_loot(
            map_id,
            target.raw(),
            character_guid,
            now,
            loot_money,
            loot_items,
        )
        .await
    else {
        let body = build_loot_error_response_body(target, LOOT_ERROR_ALREADY_PICKPOCKETED);
        send_packet(
            stream,
            WorldOpcode::SmsgLootResponse as u16,
            &body,
            Some(header_crypto),
        )
        .await?;
        return Ok(());
    };

    send_player_looting_state_update(
        stream,
        deps.shared_world,
        session,
        true,
        &mut *header_crypto,
    )
    .await?;
    let response =
        build_db_creature_loot_response_body_for_player(target, &creature, None, character_guid);
    send_packet(
        stream,
        WorldOpcode::SmsgLootResponse as u16,
        &response,
        Some(header_crypto),
    )
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn apply_player_owned_runtime_creature_summon_effect(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    character_guid: u32,
    character_level: u8,
    map_id: u32,
    spell_template: &wow_db::SpellTemplateQuery,
    summon_entry: u32,
    summon_counter: u32,
    summon_guid: ObjectGuid,
    set_pet_runtime_fields: bool,
    take_client_control: bool,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if summon_entry == 0 {
        return Ok(());
    }
    let Some(active_character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let active_position = active_character.position;
    let active_race = active_character.race;
    let Some(mut template) = deps
        .shared_world
        .object_mgr
        .creature_template(deps.world_db_pool, summon_entry)
        .await?
    else {
        warn!(
            spell_id = spell_template.id,
            summon_entry, "Skipping summon effect with missing creature_template row"
        );
        return Ok(());
    };

    template.faction = player_faction_template(active_race, session.character.player_flags);
    template.min_level = character_level;
    template.max_level = character_level;

    for existing_guid in deps
        .shared_world
        .maps
        .db_creature_guids_for_owner(map_id, caster)
        .await
    {
        let Some(event) = deps
            .shared_world
            .maps
            .delete_db_creature_runtime(map_id, Some(existing_guid), None, Some(character_guid))
            .await?
        else {
            continue;
        };
        remove_session_db_creature(session, existing_guid.raw());
        send_packet(
            stream,
            WorldOpcode::SmsgDestroyObject as u16,
            &event.direct_packet.body,
            Some(&mut *header_crypto),
        )
        .await?;
        deps.shared_world
            .sessions
            .dispatch(event.observer_packets)
            .await;
    }

    let mut creature = DbCreatureRuntime::new(CreatureSpawnQuery {
        guid: summon_counter,
        entry: summon_entry,
        map: map_id,
        game_event: None,
        guid_pool_id: None,
        entry_pool_id: None,
        pool_max_limit: None,
        pool_chance: 0.0,
        addon_emote: 0,
        position_x: active_position.x,
        position_y: active_position.y,
        position_z: active_position.z,
        orientation: active_position.orientation,
        spawn_time_secs_min: 0,
        spawn_time_secs_max: 0,
        spawn_dist: 0.0,
        movement_type: DB_MOTION_TYPE_IDLE,
        formation_waypoint_path_id: None,
        template,
        waypoint_path: Vec::new(),
    });
    creature.guid_override = Some(summon_guid);
    creature.owner_guid = Some(caster);
    creature.charmer_guid = take_client_control.then_some(caster);
    creature.created_by_spell = Some(spell_template.id);
    if set_pet_runtime_fields {
        creature.pet_name_timestamp =
            Some(current_unix_time_secs().min(u64::from(u32::MAX)) as u32);
        creature.pet_number = Some(summon_counter);
    }
    creature.player_controlled = true;

    mirror_session_db_creature(session, summon_guid.raw(), creature.clone());

    let create_body =
        build_update_object_body(&[build_db_creature_runtime_create_block(&creature)?]);
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &create_body,
        Some(&mut *header_crypto),
    )
    .await?;
    let observer_packets = deps
        .shared_world
        .maps
        .spawn_db_creature_runtime(creature, Some(character_guid))
        .await?;
    deps.shared_world.sessions.dispatch(observer_packets).await;

    if take_client_control {
        session.movement.controlled_unit = Some(summon_guid);
        session.movement.active_mover = Some(summon_guid);

        let charm_body = build_player_charm_update_body(character_guid, Some(summon_guid))?;
        send_packet(
            stream,
            WorldOpcode::SmsgUpdateObject as u16,
            &charm_body,
            Some(&mut *header_crypto),
        )
        .await?;
        let observer_packets = deps
            .shared_world
            .maps
            .broadcast_nearby_player_packet(
                map_id,
                character_guid,
                PLAYER_VISIBILITY_RADIUS_YARDS,
                OutboundWorldPacket {
                    opcode: WorldOpcode::SmsgUpdateObject as u16,
                    body: charm_body,
                },
            )
            .await;
        deps.shared_world.sessions.dispatch(observer_packets).await;

        let mut client_control_body = Vec::new();
        PackedGuid::write(&mut client_control_body, summon_guid)?;
        client_control_body.push(1);
        send_packet(
            stream,
            0x0159,
            &client_control_body,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_summon_pet_effect(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    character_guid: u32,
    character_level: u8,
    map_id: u32,
    spell_template: &wow_db::SpellTemplateQuery,
    effect: SpellInfoEffect,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Ok(pet_entry) = u32::try_from(effect.misc_value) else {
        return Ok(());
    };
    let summon_counter = deps.shared_world.maps.allocate_gm_creature_guid();
    let pet_guid = ObjectGuid::new(HighGuid::Pet, pet_entry, summon_counter);
    apply_player_owned_runtime_creature_summon_effect(
        stream,
        deps,
        session,
        caster,
        character_guid,
        character_level,
        map_id,
        spell_template,
        pet_entry,
        summon_counter,
        pet_guid,
        true,
        false,
        header_crypto,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_summon_possessed_effect(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    character_guid: u32,
    character_level: u8,
    map_id: u32,
    spell_template: &wow_db::SpellTemplateQuery,
    effect: SpellInfoEffect,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Ok(creature_entry) = u32::try_from(effect.misc_value) else {
        return Ok(());
    };
    let summon_counter = deps.shared_world.maps.allocate_gm_creature_guid();
    let creature_guid = ObjectGuid::new(HighGuid::Unit, creature_entry, summon_counter);
    apply_player_owned_runtime_creature_summon_effect(
        stream,
        deps,
        session,
        caster,
        character_guid,
        character_level,
        map_id,
        spell_template,
        creature_entry,
        summon_counter,
        creature_guid,
        false,
        true,
        header_crypto,
    )
    .await
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

        let target_switch = deps
            .shared_world
            .maps
            .switch_db_creature_threat_victim_if_needed(map_id, target, Some(character_guid))
            .await?;

        send_db_creature_threat_target_switch(
            stream,
            deps.shared_world,
            session,
            target_switch,
            header_crypto,
        )
        .await?;
    }

    debug!(spell_id, dispel_type, count, "Applied player dispel effect");

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_dispel_mechanic_effect(
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
    let Ok(mechanic) = u32::try_from(effect.misc_value) else {
        return Ok(());
    };

    if mechanic == 0 {
        return Ok(());
    }

    let count = spell_effect_calculated_u32(effect, value_context)
        .unwrap_or(1)
        .max(1);

    let target = targets.unit_target.unwrap_or(caster);

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

        let removed_spell_ids = active_aura_spell_ids_with_mechanic(
            deps.shared_world.object_mgr,
            deps.world_db_pool,
            &active_auras,
            mechanic,
            count,
        )
        .await?;
        if removed_spell_ids.is_empty() {
            return Ok(());
        }

        if target_character_guid == character_guid {
            remove_session_auras_by_spell_ids(&mut session.auras.active_auras, &removed_spell_ids);
        }

        let Some(event) = deps
            .shared_world
            .maps
            .remove_player_auras_by_spell_ids(
                map_id,
                target_character_guid,
                &removed_spell_ids,
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
        let Some(snapshot) = deps
            .shared_world
            .maps
            .db_creature_snapshot(map_id, target)
            .await
        else {
            return Ok(());
        };

        let removed_spell_ids = active_aura_spell_ids_with_mechanic(
            deps.shared_world.object_mgr,
            deps.world_db_pool,
            &snapshot.active_auras,
            mechanic,
            count,
        )
        .await?;
        if removed_spell_ids.is_empty() {
            return Ok(());
        }

        let Some(event) = deps
            .shared_world
            .maps
            .remove_db_creature_auras_by_spell_ids(
                map_id,
                target,
                character_guid,
                &removed_spell_ids,
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

        let target_switch = deps
            .shared_world
            .maps
            .switch_db_creature_threat_victim_if_needed(map_id, target, Some(character_guid))
            .await?;

        send_db_creature_threat_target_switch(
            stream,
            deps.shared_world,
            session,
            target_switch,
            header_crypto,
        )
        .await?;
    }

    debug!(
        spell_id,
        mechanic, count, "Applied player dispel mechanic effect"
    );

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

pub(in crate::world) fn remove_session_auras_by_spell_ids(
    active_auras: &mut Vec<ActiveAura>,
    spell_ids: &[u32],
) -> Vec<u32> {
    remove_active_auras_by_spell_ids(active_auras, spell_ids)
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
    apply_player_trigger_spell_by_id(
        stream,
        deps,
        session,
        caster,
        character_guid,
        character_level,
        map_id,
        triggered_spell_id,
        targets,
        now,
        header_crypto,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_trigger_spell_by_id(
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
                None,
                triggered_value_context,
                now,
                header_crypto,
            )
            .await?;
        }

        SpellCastKind::InstantDamage => {
            let mut weapon_damage_applied = false;
            for effect in triggered_info.effects {
                match effect.dispatch {
                    SpellEffectDispatch::SchoolDamage => {
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
                    SpellEffectDispatch::WeaponDamage
                    | SpellEffectDispatch::WeaponPercentDamage
                        if !weapon_damage_applied =>
                    {
                        apply_player_direct_damage_effect(
                            stream,
                            deps,
                            session,
                            caster,
                            character_guid,
                            map_id,
                            player_weapon_damage_effect(&triggered_profile),
                            targets,
                            None,
                            header_crypto,
                        )
                        .await?;
                        weapon_damage_applied = true;
                    }
                    _ => {}
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
