#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpellEffectDispatch {
    Empty,
    SchoolDamage,
    WeaponDamage,
    WeaponPercentDamage,
    ApplyAura,
    Heal,
    Energize,
    Teleport,
    Charge,
    OpenLock,
    LearnSpell,
    LearnSkill,
    TriggerSpell,
    AddComboPoints,
    Unsupported(u32),
}

impl SpellEffectDispatch {
    fn from_effect_id(effect_id: u32) -> Self {
        match effect_id {
            0 => Self::Empty,
            2 => Self::SchoolDamage,
            SPELL_EFFECT_WEAPON_DAMAGE_NOSCHOOL
            | SPELL_EFFECT_WEAPON_DAMAGE
            | SPELL_EFFECT_NORMALIZED_WEAPON_DMG => Self::WeaponDamage,
            SPELL_EFFECT_WEAPON_PERCENT_DAMAGE => Self::WeaponPercentDamage,
            SPELL_EFFECT_APPLY_AURA => Self::ApplyAura,
            SPELL_EFFECT_HEAL => Self::Heal,
            SPELL_EFFECT_ENERGIZE => Self::Energize,
            SPELL_EFFECT_TELEPORT_UNITS | SPELL_EFFECT_TELEPORT_UNITS_FACE_CASTER => {
                Self::Teleport
            }
            SPELL_EFFECT_CHARGE => Self::Charge,
            33 | 59 => Self::OpenLock,
            36 => Self::LearnSpell,
            44 => Self::LearnSkill,
            64 => Self::TriggerSpell,
            SPELL_EFFECT_ADD_COMBO_POINTS => Self::AddComboPoints,
            other => Self::Unsupported(other),
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn apply_player_spell_effects(
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
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let spell_info = SpellInfo::from_template(spell_template);
    let mut charge_applied = false;
    let mut direct_heal_applied = false;
    let mut aura_applied = false;
    let mut weapon_damage_applied = false;
    let mut landed_damage = false;
    let combo_points_for_effects = spell_combo_points_for_effects(
        deps.shared_world,
        caster,
        character_guid,
        map_id,
        spell_profile,
        targets,
    )
    .await;

    for effect in spell_info.effects {
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
            SpellEffectDispatch::SchoolDamage
                if spell_profile.kind != SpellCastKind::Charge
                    && spell_profile.kind != SpellCastKind::NextMeleeSwing =>
            {
                if let Some(damage_effect) =
                    player_direct_damage_effect(
                        spell_template,
                        spell_profile,
                        effect,
                        combo_points_for_effects,
                    )
                {
                    landed_damage |= apply_player_direct_damage_effect(
                        stream,
                        deps,
                        session,
                        caster,
                        character_guid,
                        map_id,
                        damage_effect,
                        targets,
                        header_crypto,
                    )
                    .await?;
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
                    header_crypto,
                )
                .await?;
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
                    targets,
                    header_crypto,
                )
                .await?;
                direct_heal_applied = true;
            }
            SpellEffectDispatch::ApplyAura
                if matches!(
                    spell_profile.kind,
                    SpellCastKind::AuraApplication | SpellCastKind::DirectHeal
                ) && !aura_applied =>
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
                    now,
                    header_crypto,
                )
                .await?;
                aura_applied = true;
            }
            _ => {}
        }
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

    let power_update = match spell_profile.power {
        SpellPowerCost::Rage { .. } => build_player_rage_update_body(caster, session.player_rage)?,
        SpellPowerCost::Mana { .. } => build_player_mana_update_body(caster, session.player_mana)?,
        SpellPowerCost::Energy { .. } => {
            build_player_energy_update_body(caster, session.player_energy)?
        }
    };
    send_packet(stream, SMSG_UPDATE_OBJECT, &power_update, Some(header_crypto)).await
}

#[derive(Debug, Clone, Copy)]
struct PlayerDirectDamageEffect {
    spell_id: u32,
    damage: u32,
    weapon_damage_percent: u32,
    school: u8,
    dmg_class: u32,
    attributes_ex2: u32,
    attributes_ex3: u32,
    requires_melee: bool,
    uses_weapon_outcome: bool,
    suppress_attacker_state: bool,
}

fn player_direct_damage_effect(
    spell_template: &wow_db::SpellTemplateQuery,
    spell_profile: &SpellCastProfile,
    effect: SpellInfoEffect,
    combo_points: u8,
) -> Option<PlayerDirectDamageEffect> {
    let damage = spell_effect_roll_value(effect, combo_points)?;
    let school = match effect.dispatch {
        SpellEffectDispatch::SchoolDamage => spell_template.school as u8,
        _ => return None,
    };
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
    })
}

fn player_weapon_damage_effect(spell_profile: &SpellCastProfile) -> PlayerDirectDamageEffect {
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
    }
}

async fn spell_combo_points_for_effects(
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

fn spell_effect_roll_value(effect: SpellInfoEffect, combo_points: u8) -> Option<u32> {
    let base_dice = effect.base_dice as i32;
    let mut value = effect.base_points;
    match effect.die_sides {
        0 | 1 => {
            value = value.saturating_add(base_dice);
        }
        die_sides => {
            let low = die_sides.min(base_dice);
            let high = die_sides.max(base_dice);
            value = value.saturating_add(rand::thread_rng().gen_range(low..=high));
        }
    }
    if effect.points_per_combo_point != 0.0 && combo_points > 0 {
        value = value.saturating_add(
            (effect.points_per_combo_point * f32::from(combo_points)).trunc() as i32,
        );
    }
    (value >= 0).then_some(value as u32)
}

#[allow(clippy::too_many_arguments)]
async fn apply_player_charge_effect(
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
async fn apply_player_direct_damage_effect(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    character_guid: u32,
    map_id: u32,
    damage_effect: PlayerDirectDamageEffect,
    targets: &SpellCastTargets,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    apply_db_creature_spell_damage(
        stream,
        deps,
        session,
        caster,
        character_guid,
        map_id,
        damage_effect,
        targets,
        header_crypto,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn apply_player_direct_heal_effect(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    map_id: u32,
    spell_info: &SpellInfo<'_>,
    targets: &SpellCastTargets,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let heal = spell_direct_heal(spell_info);
    if heal == 0 {
        return Ok(());
    }
    let Some(target) = targets.unit_target.filter(|target| target.is_player()) else {
        return Ok(());
    };
    let Some(event) = deps
        .shared_world
        .maps
        .apply_player_heal(map_id, target.counter(), heal)
        .await?
    else {
        return Ok(());
    };
    send_player_spell_log_to_target_set(
        stream,
        deps.shared_world,
        character_guid_from_caster(caster),
        event.healed_character_guid,
        event.direct_session_id,
        &event.observer_packets,
        OutboundWorldPacket {
            opcode: SMSG_SPELLHEALLOG,
            body: build_spell_heal_log_body(
                caster,
                target,
                spell_info.template.id,
                event.amount_healed,
                false,
            )?,
        },
        header_crypto,
    )
    .await?;
    if event.healed_character_guid == caster.counter() {
        session.player_health = event.health;
        for packet in event.direct_packets {
            send_packet(stream, packet.opcode, &packet.body, Some(&mut *header_crypto)).await?;
        }
    } else {
        deps.shared_world.sessions.dispatch(
            event
                .direct_packets
                .into_iter()
                .map(|packet| (event.direct_session_id, packet))
                .collect(),
        )
        .await;
    }
    deps.shared_world.sessions.dispatch(event.observer_packets).await;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn send_player_spell_log_to_target_set(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    caster_character_guid: u32,
    target_character_guid: u32,
    target_session_id: SessionId,
    observer_packets: &[(SessionId, OutboundWorldPacket)],
    packet: OutboundWorldPacket,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    send_packet(stream, packet.opcode, &packet.body, Some(&mut *header_crypto)).await?;

    let caster_session_id = shared_world
        .sessions
        .session_for_character(caster_character_guid)
        .await;
    let mut dispatch = Vec::new();
    let mut seen = HashSet::new();
    if Some(target_session_id) != caster_session_id || target_character_guid != caster_character_guid {
        seen.insert(target_session_id);
        dispatch.push((target_session_id, packet.clone()));
    }
    for (session_id, _) in observer_packets {
        if Some(*session_id) == caster_session_id || !seen.insert(*session_id) {
            continue;
        }
        dispatch.push((*session_id, packet.clone()));
    }
    shared_world.sessions.dispatch(dispatch).await;
    Ok(())
}

fn character_guid_from_caster(caster: ObjectGuid) -> u32 {
    caster.counter()
}

#[allow(clippy::too_many_arguments)]
async fn apply_db_creature_spell_damage(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    character_guid: u32,
    map_id: u32,
    damage_effect: PlayerDirectDamageEffect,
    targets: &SpellCastTargets,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    let Some(target) = targets.unit_target else {
        return Ok(false);
    };
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
        let weapon_skill_id = main_hand_weapon_skill_id(deps.world_db_pool, &session.inventory).await?;
        let attacker_skill = weapon_skill_id
            .map(|skill_id| {
                current_skill_value_with_active_auras(
                    &session.character_skills,
                    &session.active_auras,
                    skill_id,
                )
            })
            .unwrap_or(0);
        let character_level = session
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
        let character = session.active_character.as_ref();
        Some(roll_spell_damage_outcome(SpellDamageOutcomeInput {
            damage: damage_effect.damage,
            school: damage_effect.school,
            dmg_class: damage_effect.dmg_class,
            attributes_ex2: damage_effect.attributes_ex2,
            attributes_ex3: damage_effect.attributes_ex3,
            caster_class: character.map(|character| character.class).unwrap_or(1),
            caster_level: character.map(|character| character.level).unwrap_or(1),
            caster_intellect: combat_stats.intellect,
            target_level: target_creature.spawn.template.max_level,
            target_resistances: creature_spell_resistances(&target_creature.spawn.template),
        }))
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
                SMSG_SPELLNONMELEEDAMAGELOG,
                spell_non_melee_log_body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
        if let Some(spell_miss_log_body) = &event.spell_miss_log_body {
            send_packet(
                stream,
                SMSG_SPELLLOGMISS,
                spell_miss_log_body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
        if let Some(attacker_state_body) = &event.attacker_state_body {
            send_packet(
                stream,
                SMSG_ATTACKERSTATEUPDATE,
                attacker_state_body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
        let creature_update_body = event.update_body.clone();
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &creature_update_body,
            Some(&mut *header_crypto),
        )
        .await?;
        let broadcast = CreatureCombatBroadcast {
            shared_world: deps.shared_world,
            map_id,
            player: caster,
        };
        deps.shared_world.sessions.dispatch(event.observer_packets).await;
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
            begin_shared_db_creature_combat(
                deps.shared_world,
                session,
                target,
                Instant::now(),
            )
            .await;
        }
        return Ok(requested_damage > 0);
    }
    Ok(false)
}

#[allow(clippy::too_many_arguments)]
async fn apply_player_combo_points_effect(
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
    send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(header_crypto)).await
}

async fn clear_player_combo_points_after_finisher(
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
    send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(header_crypto)).await
}

#[allow(clippy::too_many_arguments)]
async fn apply_player_spell_aura(
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
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let aura = build_active_aura(
        spell_template,
        caster,
        character_level,
        now,
        deps.shared_world.maps.spell_duration(spell_template.duration_index),
    );
    match spell_profile.aura_target {
        SpellAuraTarget::Caster => {
            apply_player_aura(session, aura.clone());
            if let Some(event) = deps
                .shared_world
                .maps
                .apply_player_aura(map_id, character_guid, aura)
                .await?
            {
                for packet in event.direct_packets {
                    send_packet(stream, packet.opcode, &packet.body, Some(&mut *header_crypto))
                        .await?;
                }
                deps.shared_world.sessions.dispatch(event.observer_packets).await;
            } else {
                send_packet(
                    stream,
                    SMSG_UPDATE_OBJECT,
                    &build_player_aura_update_body(caster, &session.active_auras)?,
                    Some(&mut *header_crypto),
                )
                .await?;
                for packet in build_player_aura_duration_update_packets(&session.active_auras, now)
                {
                    send_packet(stream, packet.opcode, &packet.body, Some(&mut *header_crypto))
                        .await?;
                }
            }
        }
        SpellAuraTarget::UnitTarget => {
            if let Some(target) = targets.unit_target {
                if target.is_creature() {
                    if let Some(event) = deps
                        .shared_world
                        .maps
                        .apply_db_creature_aura(map_id, target, character_guid, aura)
                        .await?
                    {
                        send_packet(
                            stream,
                            SMSG_UPDATE_OBJECT,
                            &event.update_body,
                            Some(&mut *header_crypto),
                        )
                        .await?;
                        deps.shared_world.sessions.dispatch(event.observer_packets).await;
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
    }
    Ok(())
}

fn spell_direct_heal(spell_info: &SpellInfo<'_>) -> u32 {
    spell_info
        .effects
        .into_iter()
        .filter(|effect| effect.dispatch == SpellEffectDispatch::Heal)
        .filter_map(|effect| spell_effect_simple_value(effect.base_points))
        .sum()
}

fn spell_direct_energize(spell_info: &SpellInfo<'_>) -> u32 {
    spell_info
        .effects
        .into_iter()
        .filter(|effect| effect.dispatch == SpellEffectDispatch::Energize)
        .filter_map(|effect| spell_effect_simple_value(effect.base_points))
        .sum()
}

#[allow(clippy::too_many_arguments)]
async fn apply_item_use_spell_effects(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    spell_template: &wow_db::SpellTemplateQuery,
    item_spell: &SpellCastProfile,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.active_character.as_ref() else {
        return Ok(());
    };
    let map_id = character.position.map_id;
    let character_guid = character.guid;
    let character_level = character.level;
    let character_snapshot = character.clone();
    let mut update_bodies = Vec::new();
    let spell_info = SpellInfo::from_template(spell_template);

    if spell_info
        .effects
        .iter()
        .any(|effect| effect.dispatch == SpellEffectDispatch::Teleport)
        && item_spell.kind == SpellCastKind::Teleport
    {
        for effect in spell_info.effects {
            if effect.dispatch == SpellEffectDispatch::Teleport {
                return apply_item_teleport_spell_effect(
                    stream,
                    deps,
                    session,
                    character_guid,
                    map_id,
                    header_crypto,
                )
                .await;
            }
        }
    }

    let world_stats = wow_db::get_player_world_stats(
        deps.world_db_pool,
        character.race,
        character.class,
        character.level,
    )
    .await?;
    let effective_world_stats = player_world_stats_with_active_auras(world_stats, &session.active_auras);
    let max_health = effective_world_stats.max_health().max(1);
    let max_mana = effective_world_stats.max_mana();

    let mut direct_heal_applied = false;
    let mut direct_energize_applied = false;
    let mut aura_applied = false;
    for effect in spell_info.effects {
        match effect.dispatch {
            SpellEffectDispatch::Heal if !direct_heal_applied => {
                let heal = spell_direct_heal(&spell_info);
                if heal != 0 {
                    let old_health = session.player_health;
                    session.player_health = session.player_health.saturating_add(heal).min(max_health);
                    let amount_healed = session.player_health.saturating_sub(old_health);
                    if amount_healed > 0 {
                        let log = build_spell_heal_log_body(
                            caster,
                            caster,
                            spell_template.id,
                            amount_healed,
                            false,
                        )?;
                        send_packet(
                            stream,
                            SMSG_SPELLHEALLOG,
                            &log,
                            Some(&mut *header_crypto),
                        )
                        .await?;
                    }
                    update_bodies.push(build_player_health_update_body(caster, session.player_health)?);
                }
                direct_heal_applied = true;
            }
            SpellEffectDispatch::Energize if !direct_energize_applied => {
                let energize = spell_direct_energize(&spell_info);
                if energize != 0 && max_mana != 0 {
                    let old_mana = session.player_mana;
                    session.player_mana = session.player_mana.saturating_add(energize).min(max_mana);
                    let amount_energized = session.player_mana.saturating_sub(old_mana);
                    if amount_energized > 0 {
                        let log = build_spell_energize_log_body(
                            caster,
                            caster,
                            spell_template.id,
                            POWER_TYPE_MANA,
                            amount_energized,
                        )?;
                        send_packet(
                            stream,
                            SMSG_SPELLENERGIZELOG,
                            &log,
                            Some(&mut *header_crypto),
                        )
                        .await?;
                    }
                    update_bodies.push(build_player_mana_update_body(caster, session.player_mana)?);
                }
                direct_energize_applied = true;
            }
            SpellEffectDispatch::ApplyAura
                if item_spell.kind == SpellCastKind::AuraApplication && !aura_applied =>
            {
                apply_item_aura_effect(
                    stream,
                    deps,
                    session,
                    caster,
                    character_guid,
                    character_level,
                    map_id,
                    spell_template,
                    &character_snapshot,
                    now,
                    &mut update_bodies,
                    header_crypto,
                )
                .await?;
                aura_applied = true;
            }
            _ => {}
        }
    }

    for body in update_bodies {
        send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(&mut *header_crypto)).await?;
    }
    deps.shared_world
        .maps
        .sync_player_gameplay_state(map_id, character_guid, session)
        .await;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn apply_item_aura_effect(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    character_guid: u32,
    character_level: u8,
    map_id: u32,
    spell_template: &wow_db::SpellTemplateQuery,
    character_snapshot: &Player,
    now: Instant,
    update_bodies: &mut Vec<Vec<u8>>,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let aura = build_active_aura(
        spell_template,
        caster,
        character_level,
        now,
        deps.shared_world.maps.spell_duration(spell_template.duration_index),
    );
    let makes_player_sit = aura.periodic_regen.is_some();
    apply_player_aura(session, aura.clone());
    if makes_player_sit {
        session.player_stand_state = PLAYER_STAND_STATE_SIT;
        update_bodies.push(build_player_stand_state_update_body(
            character_snapshot,
            session.player_stand_state,
        )?);
    }
    if let Some(event) = deps
        .shared_world
        .maps
        .apply_player_aura(map_id, character_guid, aura)
        .await?
    {
        for packet in event.direct_packets {
            send_packet(stream, packet.opcode, &packet.body, Some(&mut *header_crypto)).await?;
        }
        deps.shared_world.sessions.dispatch(event.observer_packets).await;
    } else {
        update_bodies.push(build_player_aura_update_body(caster, &session.active_auras)?);
        for packet in build_player_aura_duration_update_packets(&session.active_auras, now) {
            send_packet(stream, packet.opcode, &packet.body, Some(&mut *header_crypto)).await?;
        }
    }
    if makes_player_sit {
        let observer_packets = deps
            .shared_world
            .maps
            .broadcast_nearby_player_packet(
                map_id,
                character_guid,
                PLAYER_VISIBILITY_RADIUS_YARDS,
                OutboundWorldPacket {
                    opcode: SMSG_UPDATE_OBJECT,
                    body: build_player_stand_state_update_body(
                        character_snapshot,
                        session.player_stand_state,
                    )?,
                },
            )
            .await;
        deps.shared_world.sessions.dispatch(observer_packets).await;
    }
    Ok(())
}

async fn apply_item_teleport_spell_effect(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    character_guid: u32,
    old_map_id: u32,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(homebind) = wow_db::get_character_homebind(deps.character_db_pool, character_guid).await? else {
        warn!(character_guid, "Ignoring teleport item spell without character_homebind row");
        return Ok(());
    };
    let Some(character) = session.active_character.as_mut() else {
        return Ok(());
    };

    character.position = homebind;
    character.movement_flags = 0;
    character.fall_time = 0;
    deps.shared_world
        .maps
        .set_player_position(old_map_id, character_guid, homebind)
        .await;
    deps.shared_world
        .maps
        .reset_player_visibility_scan_positions(old_map_id, character_guid)
        .await;
    deps.shared_world
        .maps
        .sync_player_gameplay_state(old_map_id, character_guid, session)
        .await;
    wow_db::update_character_position(
        deps.character_db_pool,
        deps.account_id,
        character_guid,
        homebind,
    )
    .await?;

    send_packet(
        stream,
        MSG_MOVE_TELEPORT_ACK,
        &build_near_teleport_ack_body(session.active_character.as_ref().unwrap(), 0)?,
        Some(&mut *header_crypto),
    )
    .await?;
    stream_newly_visible_db_creatures(
        stream,
        deps.character_db_pool,
        deps.world_db_pool,
        deps.shared_world.maps,
        session,
        header_crypto,
    )
    .await?;
    Ok(())
}
