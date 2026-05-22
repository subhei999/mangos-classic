use super::*;

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct PlayerDirectDamageEffect {
    pub(in crate::world) spell_id: u32,
    pub(in crate::world) damage: u32,
    pub(in crate::world) weapon_damage_percent: u32,
    pub(in crate::world) school: u8,
    pub(in crate::world) dmg_class: u32,
    pub(in crate::world) attributes_ex2: u32,
    pub(in crate::world) attributes_ex3: u32,
    pub(in crate::world) requires_melee: bool,
    pub(in crate::world) uses_weapon_outcome: bool,
    pub(in crate::world) suppress_attacker_state: bool,
    pub(in crate::world) caster_centered_hostile_area: bool,
    pub(in crate::world) destination_hostile_area: bool,
    pub(in crate::world) caster_centered_hostile_cone: bool,
    pub(in crate::world) radius_index: u32,
}

pub(in crate::world) fn player_direct_damage_effect(
    spell_template: &wow_db::SpellTemplateQuery,
    spell_profile: &SpellCastProfile,
    effect: SpellInfoEffect,
    value_context: SpellEffectValueContext,
) -> Option<PlayerDirectDamageEffect> {
    let damage = spell_effect_calculated_u32(effect, value_context)?;
    let school = match effect.dispatch {
        SpellEffectDispatch::SchoolDamage => spell_template.school as u8,
        _ => return None,
    };
    let target = plan_effect_target(effect);
    Some(PlayerDirectDamageEffect {
        spell_id: spell_profile.spell_id,
        damage,
        weapon_damage_percent: 100,
        school,
        dmg_class: spell_template.dmg_class,
        attributes_ex2: spell_template.attributes_ex2,
        attributes_ex3: spell_template.attributes_ex3,
        requires_melee: spell_profile.requires_melee,
        uses_weapon_outcome: false,
        suppress_attacker_state: effect.dispatch == SpellEffectDispatch::SchoolDamage,
        caster_centered_hostile_area: matches!(
            target,
            SpellPlanEffectTarget::CasterAreaEnemy { .. }
        ),
        destination_hostile_area: target == SpellPlanEffectTarget::DestinationAreaEnemy,
        caster_centered_hostile_cone: matches!(
            target,
            SpellPlanEffectTarget::CasterAreaEnemy { cone: true }
        ),
        radius_index: effect.radius_index,
    })
}

pub(in crate::world) fn player_weapon_damage_effect(
    spell_profile: &SpellCastProfile,
) -> PlayerDirectDamageEffect {
    PlayerDirectDamageEffect {
        spell_id: spell_profile.spell_id,
        damage: spell_profile.bonus_damage,
        weapon_damage_percent: spell_profile.weapon_damage_percent,
        school: 0,
        dmg_class: SPELL_DAMAGE_CLASS_MELEE,
        attributes_ex2: 0,
        attributes_ex3: 0,
        requires_melee: spell_profile.requires_melee,
        uses_weapon_outcome: true,
        suppress_attacker_state: true,
        caster_centered_hostile_area: false,
        destination_hostile_area: false,
        caster_centered_hostile_cone: false,
        radius_index: 0,
    }
}

pub(in crate::world) async fn spell_combo_points_for_effects(
    shared_world: SharedWorldDeps<'_>,
    caster: ObjectGuid,
    character_guid: u32,
    map_id: u32,
    spell_profile: &SpellCastProfile,
    targets: &SpellCastTargets,
) -> u8 {
    if !spell_profile.needs_combo_points {
        return 0;
    }
    let Some(target) = targets.unit_target else {
        return 0;
    };
    shared_world
        .maps
        .player_runtime_snapshot(map_id, character_guid)
        .await
        .filter(|snapshot| snapshot.combo_target == Some(target) || target == caster)
        .map(|snapshot| snapshot.combo_points)
        .unwrap_or(0)
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_charge_effect(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    map_id: u32,
    spell_template: &wow_db::SpellTemplateQuery,
    spell_profile: &SpellCastProfile,
    targets: &SpellCastTargets,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(target) = targets.unit_target else {
        return Ok(());
    };
    apply_charge_movement(
        stream,
        shared_world,
        session,
        caster,
        target,
        spell_template.speed,
        spell_profile.spell_id,
        header_crypto,
    )
    .await?;
    begin_db_creature_retaliation_if_needed(
        stream,
        shared_world,
        map_id,
        session,
        target,
        caster,
        header_crypto,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_direct_damage_effect(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    character_guid: u32,
    map_id: u32,
    damage_effect: PlayerDirectDamageEffect,
    targets: &SpellCastTargets,
    target_outcome: Option<PlayerSpellTargetOutcome>,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    if damage_effect.caster_centered_hostile_area {
        let Some(radius) = deps
            .shared_world
            .maps
            .spell_radius(damage_effect.radius_index)
            .map(|entry| entry.radius)
            .filter(|radius| *radius > 0.0)
        else {
            warn!(
                spell_id = damage_effect.spell_id,
                radius_index = damage_effect.radius_index,
                "Skipping caster-centered AoE damage with missing SpellRadius.dbc row"
            );
            return Ok(false);
        };
        let targets = if damage_effect.caster_centered_hostile_cone {
            let cone_radians = spell_cone_radians_for_spell(deps, damage_effect.spell_id).await?;
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
        let mut landed = false;
        for target in targets {
            let area_targets = SpellCastTargets {
                target_mask: SPELL_CAST_TARGET_UNIT,
                unit_target: Some(target),
                gameobject_target: None,
                source_location: None,
                destination: None,
            };
            landed |= apply_db_creature_spell_damage(
                stream,
                deps,
                session,
                caster,
                character_guid,
                map_id,
                damage_effect,
                &area_targets,
                None,
                header_crypto,
            )
            .await?;
        }
        return Ok(landed);
    }
    if damage_effect.destination_hostile_area {
        let Some(radius) = deps
            .shared_world
            .maps
            .spell_radius(damage_effect.radius_index)
            .map(|entry| entry.radius)
            .filter(|radius| *radius > 0.0)
        else {
            warn!(
                spell_id = damage_effect.spell_id,
                radius_index = damage_effect.radius_index,
                "Skipping destination AoE damage with missing SpellRadius.dbc row"
            );
            return Ok(false);
        };
        let Some(destination) = spell_target_destination_position(map_id, targets) else {
            warn!(
                spell_id = damage_effect.spell_id,
                "Skipping destination AoE damage with missing target destination"
            );
            return Ok(false);
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
        let mut landed = false;
        for target in targets {
            let area_targets = SpellCastTargets {
                target_mask: SPELL_CAST_TARGET_UNIT,
                unit_target: Some(target),
                gameobject_target: None,
                source_location: None,
                destination: None,
            };
            landed |= apply_db_creature_spell_damage(
                stream,
                deps,
                session,
                caster,
                character_guid,
                map_id,
                damage_effect,
                &area_targets,
                None,
                header_crypto,
            )
            .await?;
        }
        return Ok(landed);
    }
    apply_db_creature_spell_damage(
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
    .await
}

pub(in crate::world) fn spell_target_destination_position(
    map_id: u32,
    targets: &SpellCastTargets,
) -> Option<WorldPosition> {
    let destination = targets.destination?;
    Some(WorldPosition::new(
        map_id,
        destination.x,
        destination.y,
        destination.z,
        0.0,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_db_creature_spell_damage(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    character_guid: u32,
    map_id: u32,
    damage_effect: PlayerDirectDamageEffect,
    targets: &SpellCastTargets,
    target_outcome: Option<PlayerSpellTargetOutcome>,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    let Some(target) = targets.unit_target else {
        return Ok(false);
    };
    if target_outcome
        .filter(|outcome| outcome.target == target)
        .is_some_and(|outcome| outcome.miss_info.is_some())
    {
        return Ok(false);
    }
    let can_apply_damage = if damage_effect.requires_melee {
        db_creature_player_melee_check_from_map(deps.shared_world, session, target).await
            == PlayerMeleeCheck::Clear
    } else {
        true
    };
    if !can_apply_damage {
        return Ok(false);
    }

    let Some(target_creature) = deps
        .shared_world
        .maps
        .db_creature_snapshot(map_id, target)
        .await
    else {
        return Ok(false);
    };
    let melee_outcome = if damage_effect.uses_weapon_outcome {
        let combat_stats = deps
            .shared_world
            .maps
            .player_combat_stats(map_id, character_guid)
            .await
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "map-owned player combat stats missing for character {}",
                    character_guid
                )
            })?;
        let weapon_skill_id =
            main_hand_weapon_skill_id(deps.world_db_pool, &session.inventory.items).await?;
        let attacker_skill = weapon_skill_id
            .map(|skill_id| {
                current_skill_value_with_active_auras(
                    &session.character.character_skills,
                    &session.auras.active_auras,
                    skill_id,
                )
            })
            .unwrap_or(0);
        let character_level = session
            .character
            .active_character
            .as_ref()
            .map(|character| character.level)
            .unwrap_or(1);
        Some(
            player_main_hand_melee_outcome_against_db_creature(
                &combat_stats,
                character_level,
                attacker_skill,
                &target_creature,
            )
            .with_weapon_spell_modifier(damage_effect.damage, damage_effect.weapon_damage_percent),
        )
    } else {
        None
    };
    let spell_damage_outcome = if melee_outcome.is_none() {
        let combat_stats = deps
            .shared_world
            .maps
            .player_combat_stats(map_id, character_guid)
            .await
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "map-owned player combat stats missing for character {}",
                    character_guid
                )
            })?;
        let character = session.character.active_character.as_ref();
        let attributes_ex3 = if target_outcome
            .filter(|outcome| outcome.target == target)
            .is_some_and(|outcome| outcome.miss_info.is_none())
        {
            damage_effect.attributes_ex3 | SPELL_ATTR_EX3_ALWAYS_HIT
        } else {
            damage_effect.attributes_ex3
        };
        Some(roll_spell_damage_outcome(spell_damage_outcome_input(
            damage_effect.damage,
            damage_effect.school,
            damage_effect.dmg_class,
            damage_effect.attributes_ex2,
            attributes_ex3,
            player_spell_snapshot(
                character.map(|character| character.level).unwrap_or(1),
                character.map(|character| character.class).unwrap_or(1),
                &combat_stats,
            ),
            db_creature_spell_snapshot(&target_creature),
        )))
    } else {
        None
    };
    let requested_damage = melee_outcome
        .map(|outcome| outcome.total_damage)
        .or_else(|| spell_damage_outcome.map(|outcome| outcome.final_damage))
        .unwrap_or(damage_effect.damage);

    let corpse_loot = if requested_damage >= target_creature.health {
        Some(
            prepare_db_creature_corpse_loot(
                deps.shared_world.object_mgr,
                deps.world_db_pool,
                deps.parties,
                session,
                character_guid,
                target_creature.spawn.entry,
            )
            .await?,
        )
    } else {
        None
    };
    if let Some(event) = deps
        .shared_world
        .maps
        .apply_db_creature_damage(
            map_id,
            DbCreatureDamageRequest {
                creature_guid: target,
                killer: caster,
                damage: requested_damage,
                melee_outcome,
                spell_damage_outcome,
                spell_id: Some(damage_effect.spell_id),
                spell_school: damage_effect.school,
                suppress_attacker_state: damage_effect.suppress_attacker_state,
                now: Instant::now(),
                now_epoch_secs: current_unix_epoch_secs(),
                exclude_character_guid: Some(character_guid),
                corpse_loot,
            },
        )
        .await?
    {
        let death_finalization = event.death_finalization;
        let target_switch = event.target_switch;
        let is_dead = death_finalization.is_some();
        mirror_session_db_creature(session, target.raw(), event.creature.clone());
        if is_dead {
            mirror_session_player_auto_attack(session, None, None);
            deps.shared_world
                .maps
                .set_player_auto_attack(map_id, character_guid, None, None)
                .await;
            clear_db_creature_combat_if_attacker(session, target);
        }
        if let Some(spell_non_melee_log_body) = &event.spell_non_melee_log_body {
            send_packet(
                stream,
                WorldOpcode::SmsgSpellNonMeleeDamageLog as u16,
                spell_non_melee_log_body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
        if let Some(spell_miss_log_body) = &event.spell_miss_log_body {
            send_packet(
                stream,
                WorldOpcode::SmsgSpellLogMiss as u16,
                spell_miss_log_body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
        if let Some(attacker_state_body) = &event.attacker_state_body {
            send_packet(
                stream,
                WorldOpcode::SmsgAttackerStateUpdate as u16,
                attacker_state_body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
        let creature_update_body = event.update_body.clone();
        send_packet(
            stream,
            WorldOpcode::SmsgUpdateObject as u16,
            &creature_update_body,
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
        let broadcast = CreatureCombatBroadcast {
            shared_world: deps.shared_world,
            map_id,
            player: caster,
        };
        deps.shared_world
            .sessions
            .dispatch(event.observer_packets)
            .await;
        if is_dead {
            send_db_creature_motion_stop(stream, broadcast, session, target, header_crypto).await?;
            finalize_db_creature_death(
                stream,
                CombatRewardDeps {
                    character_db_pool: deps.character_db_pool,
                    world_db_pool: deps.world_db_pool,
                    shared_world: deps.shared_world,
                    parties: deps.parties,
                },
                session,
                death_finalization,
                header_crypto,
            )
            .await?;
        } else {
            send_db_creature_threat_target_switch(
                stream,
                deps.shared_world,
                session,
                target_switch,
                header_crypto,
            )
            .await?;
            begin_db_creature_combat_with_assistance(
                stream,
                deps.shared_world,
                map_id,
                session,
                target,
                caster,
                header_crypto,
            )
            .await?;
            try_process_db_creature_event_ai_hp_actions(
                stream,
                deps.shared_world,
                deps.world_db_pool,
                session,
                map_id,
                target,
                caster,
                Instant::now(),
                header_crypto,
            )
            .await?;
        }
        return Ok(requested_damage > 0);
    }
    Ok(false)
}
