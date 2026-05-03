async fn handle_cast_spell(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    shared_world: SharedWorldDeps<'_>,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let packet = CastSpellPacket::read(body)?;
    expire_session_auras(session, Instant::now());
    let Some(character) = &session.active_character else {
        warn!(
            spell_id = packet.spell_id,
            "Ignoring spell cast before character login"
        );
        return Ok(());
    };
    let character_guid = character.guid;
    let character_level = character.level;
    let map_id = character.position.map_id;
    let caster = ObjectGuid::new(HighGuid::Player, 0, character_guid);

    if packet.spell_id == OPENING_SPELL_ID {
        let request = OpeningSpellRequest {
            caster,
            map_id,
            character_guid,
            targets: packet.targets,
        };
        return handle_opening_spell(
            stream,
            world_db_pool,
            shared_world,
            session,
            header_crypto,
            request,
        )
        .await;
    }

    if !session.active_spells.contains(&packet.spell_id) {
        warn!(
            spell_id = packet.spell_id,
            character_guid,
            "Ignoring spell cast for spell not active on character"
        );
        return Ok(());
    }
    let Some(spell_template) = shared_world
        .object_mgr
        .spell_template(world_db_pool, packet.spell_id)
        .await?
    else {
        warn!(
            spell_id = packet.spell_id,
            "Ignoring spell cast with no spell_template row"
        );
        return Ok(());
    };
    let Some(starter_spell) = supported_starter_spell(&spell_template) else {
        warn!(
            spell_id = packet.spell_id,
            spell_name = spell_template.spell_name.as_str(),
            "Ignoring unsupported spell effect shape in starter spell slice"
        );
        return Ok(());
    };

    let targets = normalize_starter_spell_targets(packet.targets, &starter_spell, caster);
    if let Some(failure) = starter_spell_cast_failure(
        shared_world,
        session,
        &starter_spell,
        &targets,
        Instant::now(),
    )
    .await
    {
        send_packet(
            stream,
            SMSG_CAST_RESULT,
            &build_cast_result_failure_body(packet.spell_id, failure),
            Some(header_crypto),
        )
        .await?;
        send_packet(
            stream,
            SMSG_SPELL_FAILURE,
            &build_spell_failure_body(caster, packet.spell_id, failure)?,
            Some(&mut *header_crypto),
        )
        .await?;
        return send_packet(
            stream,
            SMSG_SPELL_FAILED_OTHER,
            &build_spell_failed_other_body(caster, packet.spell_id),
            Some(header_crypto),
        )
        .await;
    }
    let now = Instant::now();
    if starter_spell.kind != StarterSpellKind::NextMeleeSwing {
        match starter_spell.power {
            StarterSpellPower::Rage { cost } => {
                session.player_rage = session.player_rage.saturating_sub(cost);
            }
            StarterSpellPower::Mana { cost } => {
                session.player_mana = session.player_mana.saturating_sub(cost);
            }
        }
    }
    apply_starter_spell_cooldowns(session, &starter_spell, now);
    send_packet(
        stream,
        SMSG_CAST_RESULT,
        &build_cast_result_ok_body(packet.spell_id),
        Some(&mut *header_crypto),
    )
    .await?;
    if starter_spell.kind == StarterSpellKind::NextMeleeSwing {
        let spell_start_body = build_spell_start_body(caster, packet.spell_id, 0, &targets)?;
        send_packet(
            stream,
            SMSG_SPELL_START,
            &spell_start_body,
            Some(&mut *header_crypto),
        )
        .await?;
        let observer_packets = shared_world
            .maps
            .broadcast_nearby_player_packet(
                map_id,
                character_guid,
                PLAYER_VISIBILITY_RADIUS_YARDS,
                OutboundWorldPacket {
                    opcode: SMSG_SPELL_START,
                    body: spell_start_body,
                },
            )
            .await;
        shared_world.sessions.dispatch(observer_packets).await;
        if let Some(target) = targets.unit_target {
            let (rage_cost, mana_cost) = match starter_spell.power {
                StarterSpellPower::Rage { cost } => (cost, 0),
                StarterSpellPower::Mana { cost } => (0, cost),
            };
            session.queued_next_melee_spell = Some(QueuedNextMeleeSpell {
                spell_id: packet.spell_id,
                target,
                bonus_damage: starter_spell.bonus_damage,
                rage_cost,
                mana_cost,
            });
        }
    } else {
        let spell_go_body = build_spell_go_body(caster, packet.spell_id, &targets)?;
        send_packet(
            stream,
            SMSG_SPELL_GO,
            &spell_go_body,
            Some(&mut *header_crypto),
        )
        .await?;
        let observer_packets = shared_world
            .maps
            .broadcast_nearby_player_packet(
                map_id,
                character_guid,
                PLAYER_VISIBILITY_RADIUS_YARDS,
                OutboundWorldPacket {
                    opcode: SMSG_SPELL_GO,
                    body: spell_go_body,
                },
            )
            .await;
        shared_world.sessions.dispatch(observer_packets).await;
        if starter_spell.kind == StarterSpellKind::InstantDamage
            && targets.unit_target == Some(rust_combat_dummy_guid())
            && !session.combat_dummy_lootable
            && session.combat_dummy_health > 0
        {
            let damage = session.combat_dummy_health.min(starter_spell.damage);
            session.combat_dummy_health = session.combat_dummy_health.saturating_sub(damage);
            if session.combat_dummy_health == 0 {
                session.combat_dummy_lootable = true;
                session.combat_dummy_looting = false;
                session.combat_dummy_loot_money_available = true;
                session.combat_dummy_loot_item_available = true;
                mirror_session_player_auto_attack(session, None, None);
                shared_world
                    .maps
                    .set_player_auto_attack(map_id, character_guid, None, None)
                    .await;
            }
            send_packet(
                stream,
                SMSG_SPELLNONMELEEDAMAGELOG,
                &build_spell_non_melee_damage_log_body(SpellNonMeleeDamageLogPacket {
                    attacker: caster,
                    target: rust_combat_dummy_guid(),
                    spell_id: packet.spell_id,
                    damage,
                    school: 0,
                    absorb: 0,
                    resist: 0,
                    periodic: false,
                    blocked: 0,
                    hit_info: 0,
                })?,
                Some(&mut *header_crypto),
            )
            .await?;
            send_packet(
                stream,
                SMSG_ATTACKERSTATEUPDATE,
                &build_attacker_state_update_body_with_spell_id(
                    caster,
                    rust_combat_dummy_guid(),
                    damage,
                    packet.spell_id,
                )?,
                Some(&mut *header_crypto),
            )
            .await?;
            send_packet(
                stream,
                SMSG_UPDATE_OBJECT,
                &build_combat_dummy_state_update_body(
                    session.combat_dummy_health,
                    if session.combat_dummy_health == 0 {
                        UNIT_DYNFLAG_LOOTABLE
                    } else {
                        0
                    },
                )?,
                Some(&mut *header_crypto),
            )
            .await?;
        } else if starter_spell.kind == StarterSpellKind::InstantDamage {
            if let Some(target) = targets.unit_target {
                let can_apply_damage = if starter_spell.requires_melee {
                    db_creature_player_melee_check_from_map(shared_world, session, target).await
                        == PlayerMeleeCheck::Clear
                } else {
                    true
                };
                if can_apply_damage {
                    if let Some(event) = shared_world
                        .maps
                        .apply_db_creature_damage(
                            map_id,
                            DbCreatureDamageRequest {
                                creature_guid: target,
                                killer: caster,
                                damage: starter_spell.damage,
                                melee_outcome: None,
                                spell_id: Some(packet.spell_id),
                                suppress_attacker_state: false,
                                now: Instant::now(),
                                now_epoch_secs: current_unix_epoch_secs(),
                                exclude_character_guid: Some(character_guid),
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
                            shared_world
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
                            shared_world,
                            map_id,
                            player: caster,
                        };
                        shared_world.sessions.dispatch(event.observer_packets).await;
                        if is_dead {
                            send_db_creature_motion_stop(
                                stream,
                                broadcast,
                                session,
                                target,
                                header_crypto,
                            )
                            .await?;
                            finalize_db_creature_death(
                                stream,
                                character_db_pool,
                                world_db_pool,
                                shared_world,
                                session,
                                death_finalization,
                                header_crypto,
                            )
                            .await?;
                        } else {
                            send_db_creature_threat_target_switch(
                                stream,
                                shared_world,
                                session,
                                target_switch,
                                header_crypto,
                            )
                            .await?;
                            begin_shared_db_creature_combat(
                                shared_world,
                                session,
                                target,
                                Instant::now(),
                            )
                            .await;
                        }
                    }
                }
            }
        }
    }
    if starter_spell.kind == StarterSpellKind::NextMeleeSwing {
        Ok(())
    } else {
        if starter_spell.kind == StarterSpellKind::AuraApplication {
            let aura = build_active_aura(
                &spell_template,
                caster,
                character_level,
                now,
                shared_world
                    .maps
                    .spell_duration(spell_template.duration_index),
            );
            apply_player_aura(session, aura.clone());
            if let Some(event) = shared_world
                .maps
                .apply_player_aura(map_id, character_guid, aura)
                .await?
            {
                for packet in event.direct_packets {
                    send_packet(stream, packet.opcode, &packet.body, Some(&mut *header_crypto))
                        .await?;
                }
                shared_world.sessions.dispatch(event.observer_packets).await;
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
        let power_update = match starter_spell.power {
            StarterSpellPower::Rage { .. } => build_player_rage_update_body(caster, session.player_rage)?,
            StarterSpellPower::Mana { .. } => build_player_mana_update_body(caster, session.player_mana)?,
        };
        send_packet(stream, SMSG_UPDATE_OBJECT, &power_update, Some(header_crypto)).await
    }
}

struct OpeningSpellRequest {
    caster: ObjectGuid,
    map_id: u32,
    character_guid: u32,
    targets: SpellCastTargets,
}

async fn handle_opening_spell(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
    request: OpeningSpellRequest,
) -> anyhow::Result<()> {
    let mut targets = request.targets;
    let Some(gameobject_guid) = targets
        .gameobject_target
        .or(targets.unit_target)
        .filter(|guid| guid.is_game_object())
    else {
        warn!("Ignoring Opening spell without gameobject target");
        return Ok(());
    };
    let Some(gameobject) = shared_world
        .maps
        .db_gameobject_snapshot(request.map_id, gameobject_guid)
        .await
    else {
        warn!(
            target = format_args!("0x{:016X}", gameobject_guid.raw()),
            "Ignoring Opening spell for unknown gameobject"
        );
        return Ok(());
    };
    let Some(character) = session.active_character.as_ref() else {
        return Ok(());
    };
    if gameobject.spawn.map != request.map_id
        || !is_position_inside_radius(gameobject.position(), character.position, 8.0)
    {
        warn!("Ignoring Opening spell outside gameobject interaction range");
        return Ok(());
    }
    if !gameobject_chest_has_loot_id(&gameobject.spawn.template) {
        warn!("Ignoring Opening spell for gameobject without chest loot");
        return Ok(());
    }
    targets.target_mask |= SPELL_CAST_TARGET_GAMEOBJECT;
    targets.gameobject_target = Some(gameobject_guid);

    let loot_items = select_db_gameobject_loot_item_for_character(
        shared_world.object_mgr,
        world_db_pool,
        session,
        &gameobject.spawn.template,
    )
    .await?;
    let Some((gameobject, loot_items)) = shared_world
        .maps
        .open_db_gameobject_loot(
            request.map_id,
            gameobject_guid.raw(),
            request.character_guid,
            loot_items,
        )
        .await
    else {
        warn!("Ignoring Opening spell for unavailable gameobject loot");
        return Ok(());
    };
    let _ = gameobject;

    let spell_start_body =
        build_spell_start_body(request.caster, OPENING_SPELL_ID, OPENING_SPELL_CAST_TIME_MS, &targets)?;
    send_packet(
        stream,
        SMSG_SPELL_START,
        &spell_start_body,
        Some(&mut *header_crypto),
    )
    .await?;
    let observer_start = shared_world
        .maps
        .broadcast_nearby_player_packet(
            request.map_id,
            request.character_guid,
            PLAYER_VISIBILITY_RADIUS_YARDS,
            OutboundWorldPacket {
                opcode: SMSG_SPELL_START,
                body: spell_start_body,
            },
        )
        .await;
    shared_world.sessions.dispatch(observer_start).await;

    tokio::time::sleep(Duration::from_millis(OPENING_SPELL_CAST_TIME_MS as u64)).await;

    send_packet(
        stream,
        SMSG_CAST_RESULT,
        &build_cast_result_ok_body(OPENING_SPELL_ID),
        Some(&mut *header_crypto),
    )
    .await?;
    let spell_go_body = build_spell_go_body(request.caster, OPENING_SPELL_ID, &targets)?;
    send_packet(
        stream,
        SMSG_SPELL_GO,
        &spell_go_body,
        Some(&mut *header_crypto),
    )
    .await?;
    let observer_go = shared_world
        .maps
        .broadcast_nearby_player_packet(
            request.map_id,
            request.character_guid,
            PLAYER_VISIBILITY_RADIUS_YARDS,
            OutboundWorldPacket {
                opcode: SMSG_SPELL_GO,
                body: spell_go_body,
            },
        )
        .await;
    shared_world.sessions.dispatch(observer_go).await;

    let response = build_gameobject_loot_response_body(gameobject_guid, &loot_items);
    send_packet(
        stream,
        SMSG_LOOT_RESPONSE,
        &response,
        Some(header_crypto),
    )
    .await
}

async fn starter_spell_melee_cast_failure(
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    starter_spell: &SupportedStarterSpell,
    targets: &SpellCastTargets,
) -> Option<u8> {
    if !starter_spell.requires_melee {
        return None;
    }
    let target = targets.unit_target?;
    if target == rust_combat_dummy_guid() {
        return None;
    }
    match db_creature_player_melee_check_from_map(shared_world, session, target).await {
        PlayerMeleeCheck::Clear => None,
        PlayerMeleeCheck::BadFacing => Some(SPELL_FAILED_UNIT_NOT_INFRONT),
        PlayerMeleeCheck::NavigationBlocked(DbCreatureNavigationResult::LineOfSightBlocked) => {
            Some(SPELL_FAILED_LINE_OF_SIGHT)
        }
        _ => Some(SPELL_FAILED_OUT_OF_RANGE),
    }
}

async fn starter_spell_cast_failure(
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    starter_spell: &SupportedStarterSpell,
    targets: &SpellCastTargets,
    now: Instant,
) -> Option<u8> {
    if let Some(until) = session
        .starter_spell_cooldowns_until
        .get(&starter_spell.spell_id)
        .copied()
    {
        if now < until {
            return Some(SPELL_FAILED_NOT_READY);
        }
    }
    if starter_spell.triggers_global_cooldown
        && session
            .starter_global_cooldown_until
            .is_some_and(|until| now < until)
    {
        return Some(SPELL_FAILED_NOT_READY);
    }
    match starter_spell.power {
        StarterSpellPower::Rage { cost } if session.player_rage < cost => {
            return Some(SPELL_FAILED_NO_POWER);
        }
        StarterSpellPower::Mana { cost } if session.player_mana < cost => {
            return Some(SPELL_FAILED_NO_POWER);
        }
        _ => {}
    }
    if starter_spell.kind == StarterSpellKind::NextMeleeSwing
        && session
            .queued_next_melee_spell
            .is_some_and(|queued| queued.spell_id == starter_spell.spell_id)
    {
        return Some(SPELL_FAILED_NOT_READY);
    }
    starter_spell_melee_cast_failure(shared_world, session, starter_spell, targets).await
}

fn normalize_starter_spell_targets(
    mut targets: SpellCastTargets,
    starter_spell: &SupportedStarterSpell,
    caster: ObjectGuid,
) -> SpellCastTargets {
    targets.target_mask = (targets.target_mask | SPELL_CAST_TARGET_UNIT)
        & !SPELL_CAST_TARGET_UNIT_ENEMY;
    targets.unit_target = Some(targets.unit_target.unwrap_or_else(|| {
        if starter_spell.kind == StarterSpellKind::AuraApplication {
            caster
        } else {
            rust_combat_dummy_guid()
        }
    }));
    targets.gameobject_target = None;
    targets
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SupportedStarterSpell {
    spell_id: u32,
    kind: StarterSpellKind,
    bonus_damage: u32,
    damage: u32,
    power: StarterSpellPower,
    requires_melee: bool,
    triggers_global_cooldown: bool,
    global_cooldown_millis: u64,
    cooldown_millis: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StarterSpellKind {
    InstantDamage,
    AuraApplication,
    NextMeleeSwing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StarterSpellPower {
    Rage { cost: u32 },
    Mana { cost: u32 },
}

const SPELL_ATTR_ON_NEXT_SWING_NO_DAMAGE: u32 = 0x0000_0004;
const SPELL_ATTR_ON_NEXT_SWING: u32 = 0x0000_0400;
const SPELL_EFFECT_WEAPON_DAMAGE_NOSCHOOL: u32 = 17;
const SPELL_EFFECT_WEAPON_PERCENT_DAMAGE: u32 = 58;
const SPELL_EFFECT_APPLY_AURA: u32 = 6;
const SPELL_AURA_MOD_ATTACK_POWER: u32 = 99;
const POWER_TYPE_MANA: u32 = 0;
const POWER_TYPE_RAGE: u32 = 1;
const POSITIVE_AURA_FLAGS: u32 = 0x05;
const MAX_AURA_SLOTS: usize = 48;
const MAX_AURA_FLAG_FIELDS: usize = 6;
const MAX_AURA_LEVEL_FIELDS: usize = 12;

fn supported_starter_spell(template: &wow_db::SpellTemplateQuery) -> Option<SupportedStarterSpell> {
    let kind = if spell_has_on_next_swing_attribute(template) {
        StarterSpellKind::NextMeleeSwing
    } else if spell_has_aura_application(template) {
        StarterSpellKind::AuraApplication
    } else if spell_has_direct_damage_effect(template) {
        StarterSpellKind::InstantDamage
    } else {
        return None;
    };

    Some(SupportedStarterSpell {
        spell_id: template.id,
        kind,
        bonus_damage: spell_bonus_damage(template),
        damage: spell_direct_damage(template),
        power: spell_power(template),
        requires_melee: kind == StarterSpellKind::NextMeleeSwing || template.dmg_class == 2,
        triggers_global_cooldown: template.start_recovery_category != 0
            || template.start_recovery_time != 0,
        global_cooldown_millis: template.start_recovery_time as u64,
        cooldown_millis: template.recovery_time.max(template.category_recovery_time) as u64,
    })
}

fn spell_has_on_next_swing_attribute(template: &wow_db::SpellTemplateQuery) -> bool {
    (template.attributes & (SPELL_ATTR_ON_NEXT_SWING_NO_DAMAGE | SPELL_ATTR_ON_NEXT_SWING)) != 0
}

fn spell_has_aura_application(template: &wow_db::SpellTemplateQuery) -> bool {
    [template.effect1, template.effect2, template.effect3].contains(&SPELL_EFFECT_APPLY_AURA)
}

fn spell_has_direct_damage_effect(template: &wow_db::SpellTemplateQuery) -> bool {
    [
        template.effect_base_points1,
        template.effect_base_points2,
        template.effect_base_points3,
    ]
    .into_iter()
    .any(|base_points| base_points > 0)
}

fn spell_bonus_damage(template: &wow_db::SpellTemplateQuery) -> u32 {
    spell_effects(template)
        .into_iter()
        .filter(|effect| {
            effect.effect == SPELL_EFFECT_WEAPON_DAMAGE_NOSCHOOL
                || effect.effect == SPELL_EFFECT_WEAPON_PERCENT_DAMAGE
        })
        .filter_map(|effect| spell_effect_simple_value(effect.base_points))
        .max()
        .unwrap_or(0)
}

fn spell_direct_damage(template: &wow_db::SpellTemplateQuery) -> u32 {
    if spell_has_on_next_swing_attribute(template) {
        return 0;
    }
    spell_effects(template)
        .into_iter()
        .filter(|effect| effect.effect != 0 && effect.effect != SPELL_EFFECT_APPLY_AURA)
        .filter_map(|effect| spell_effect_simple_value(effect.base_points))
        .sum()
}

fn spell_effect_simple_value(base_points: i32) -> Option<u32> {
    (base_points >= 0).then_some((base_points + 1) as u32)
}

fn spell_power(template: &wow_db::SpellTemplateQuery) -> StarterSpellPower {
    match template.power_type {
        POWER_TYPE_RAGE => StarterSpellPower::Rage {
            cost: template.mana_cost,
        },
        POWER_TYPE_MANA => StarterSpellPower::Mana {
            cost: template.mana_cost,
        },
        _ => StarterSpellPower::Mana {
            cost: template.mana_cost,
        },
    }
}

#[derive(Debug, Clone, Copy)]
struct SpellEffectData {
    effect: u32,
    aura_name: u32,
    base_points: i32,
}

fn spell_effects(template: &wow_db::SpellTemplateQuery) -> [SpellEffectData; 3] {
    [
        SpellEffectData {
            effect: template.effect1,
            aura_name: template.effect_apply_aura_name1,
            base_points: template.effect_base_points1,
        },
        SpellEffectData {
            effect: template.effect2,
            aura_name: template.effect_apply_aura_name2,
            base_points: template.effect_base_points2,
        },
        SpellEffectData {
            effect: template.effect3,
            aura_name: template.effect_apply_aura_name3,
            base_points: template.effect_base_points3,
        },
    ]
}

fn build_active_aura(
    template: &wow_db::SpellTemplateQuery,
    caster: ObjectGuid,
    level: u8,
    now: Instant,
    duration: Option<SpellDurationEntry>,
) -> ActiveAura {
    let duration_millis = duration
        .map(|duration| {
            if duration.duration_millis == -1 {
                -1
            } else {
                duration.duration_millis.abs()
            }
        })
        .unwrap_or(0);
    ActiveAura {
        spell_id: template.id,
        caster,
        level,
        positive: true,
        duration_millis: (duration_millis > 0).then_some(duration_millis as u32),
        expires_at: (duration_millis > 0)
            .then_some(now + Duration::from_millis(duration_millis as u64)),
        stat_modifiers: spell_aura_stat_modifiers(template),
    }
}

fn spell_aura_stat_modifiers(template: &wow_db::SpellTemplateQuery) -> Vec<AuraStatModifier> {
    spell_effects(template)
        .into_iter()
        .filter(|effect| effect.effect == SPELL_EFFECT_APPLY_AURA)
        .filter_map(|effect| match effect.aura_name {
            SPELL_AURA_MOD_ATTACK_POWER => Some(AuraStatModifier::AttackPower {
                amount: spell_effect_simple_i32(effect.base_points),
            }),
            _ => None,
        })
        .collect()
}

fn spell_effect_simple_i32(base_points: i32) -> i32 {
    base_points.saturating_add(1)
}

fn apply_starter_spell_cooldowns(
    session: &mut WorldSessionState,
    starter_spell: &SupportedStarterSpell,
    now: Instant,
) {
    if starter_spell.triggers_global_cooldown {
        session.starter_global_cooldown_until =
            Some(now + Duration::from_millis(starter_spell.global_cooldown_millis));
    }
    if starter_spell.cooldown_millis > 0 {
        session.starter_spell_cooldowns_until.insert(
            starter_spell.spell_id,
            now + Duration::from_millis(starter_spell.cooldown_millis),
        );
    }
}

fn apply_player_aura(session: &mut WorldSessionState, aura: ActiveAura) {
    apply_active_aura(&mut session.active_auras, aura);
}

fn apply_active_aura(active_auras: &mut Vec<ActiveAura>, aura: ActiveAura) {
    if let Some(existing) = active_auras
        .iter_mut()
        .find(|existing| existing.spell_id == aura.spell_id && existing.caster == aura.caster)
    {
        *existing = aura;
    } else {
        active_auras.push(aura);
    }
}

fn expire_session_auras(session: &mut WorldSessionState, now: Instant) {
    session
        .active_auras
        .retain(|aura| aura.expires_at.is_none_or(|expires_at| now < expires_at));
}

fn build_player_aura_update_body(
    player: ObjectGuid,
    active_auras: &[ActiveAura],
) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player)?;

    let mut values = vec![None; PLAYER_END_FIELDS];
    set_player_aura_update_values(&mut values, active_auras)?;

    write_update_values(&mut block, &values)?;
    Ok(build_update_object_body(&[block]))
}

fn set_player_aura_update_values(
    values: &mut [Option<u32>],
    active_auras: &[ActiveAura],
) -> anyhow::Result<()> {
    for slot in 0..MAX_AURA_SLOTS {
        set_update_value(values, UNIT_FIELD_AURA + slot, 0)?;
    }
    for field in 0..MAX_AURA_FLAG_FIELDS {
        set_update_value(values, UNIT_FIELD_AURAFLAGS + field, 0)?;
    }
    for field in 0..MAX_AURA_LEVEL_FIELDS {
        set_update_value(values, UNIT_FIELD_AURALEVELS + field, 0)?;
        set_update_value(values, UNIT_FIELD_AURAAPPLICATIONS + field, 0)?;
    }

    for (slot, aura) in active_auras.iter().take(MAX_AURA_SLOTS).enumerate() {
        set_update_value(values, UNIT_FIELD_AURA + slot, aura.spell_id)?;
        if aura.positive {
            let flags_index = UNIT_FIELD_AURAFLAGS + (slot / 8);
            let flags_shift = ((slot % 8) * 4) as u32;
            let previous = values[flags_index].unwrap_or(0);
            set_update_value(
                values,
                flags_index,
                previous | (POSITIVE_AURA_FLAGS << flags_shift),
            )?;
        }

        let level_index = UNIT_FIELD_AURALEVELS + (slot / 4);
        let level_shift = ((slot % 4) * 8) as u32;
        let previous = values[level_index].unwrap_or(0);
        set_update_value(
            values,
            level_index,
            previous | ((aura.level.max(1) as u32) << level_shift),
        )?;
    }

    Ok(())
}

fn build_aura_duration_update_body(slot: u8, remaining_millis: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(5);
    body.push(slot);
    body.extend_from_slice(&remaining_millis.to_le_bytes());
    body
}

fn build_player_aura_duration_update_packets(
    active_auras: &[ActiveAura],
    now: Instant,
) -> Vec<OutboundWorldPacket> {
    active_auras
        .iter()
        .take(MAX_AURA_SLOTS)
        .enumerate()
        .filter_map(|(slot, aura)| {
            aura.remaining_duration_millis(now)
                .map(|remaining_millis| OutboundWorldPacket {
                    opcode: SMSG_UPDATE_AURA_DURATION,
                    body: build_aura_duration_update_body(slot as u8, remaining_millis),
                })
        })
        .collect()
}

fn build_cast_result_ok_body(spell_id: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(5);
    body.extend_from_slice(&spell_id.to_le_bytes());
    body.push(0);
    body
}

fn build_cast_result_failure_body(spell_id: u32, failure: u8) -> Vec<u8> {
    let mut body = Vec::with_capacity(6);
    body.extend_from_slice(&spell_id.to_le_bytes());
    body.push(2);
    body.push(failure);
    body
}

fn build_spell_go_body(
    caster: ObjectGuid,
    spell_id: u32,
    targets: &SpellCastTargets,
) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(40);
    PackedGuid::write(&mut body, caster)?;
    PackedGuid::write(&mut body, caster)?;
    body.extend_from_slice(&spell_id.to_le_bytes());
    body.extend_from_slice(&CAST_FLAG_SPELL_GO.to_le_bytes());

    if let Some(target) = targets.unit_target.or(targets.gameobject_target) {
        body.push(1);
        body.extend_from_slice(&target.raw().to_le_bytes());
    } else {
        body.push(0);
    }
    body.push(0); // miss count
    targets.write(&mut body)?;
    Ok(body)
}

fn build_spell_start_body(
    caster: ObjectGuid,
    spell_id: u32,
    cast_time_ms: u32,
    targets: &SpellCastTargets,
) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(44);
    PackedGuid::write(&mut body, caster)?;
    PackedGuid::write(&mut body, caster)?;
    body.extend_from_slice(&spell_id.to_le_bytes());
    body.extend_from_slice(&CAST_FLAG_SPELL_START.to_le_bytes());
    body.extend_from_slice(&cast_time_ms.to_le_bytes());
    targets.write(&mut body)?;
    Ok(body)
}

async fn handle_item_query_single(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if body.len() < 4 {
        anyhow::bail!(
            "CMSG_ITEM_QUERY_SINGLE payload too short: {} bytes",
            body.len()
        );
    }

    let item = u32::from_le_bytes(body[0..4].try_into()?);
    let template = wow_db::get_item_template_query(world_db_pool, item).await?;
    info!(
        item,
        found = template.is_some(),
        "Answering item template query"
    );
    let response = build_item_query_single_response(item, template.as_ref());
    send_packet(
        stream,
        SMSG_ITEM_QUERY_SINGLE_RESPONSE,
        &response,
        Some(header_crypto),
    )
    .await
}

