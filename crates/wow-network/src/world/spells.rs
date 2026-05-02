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
    let Some(character) = &session.active_character else {
        warn!(
            spell_id = packet.spell_id,
            "Ignoring spell cast before character login"
        );
        return Ok(());
    };
    let character_guid = character.guid;
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

    let Some(starter_spell) = supported_starter_spell(packet.spell_id) else {
        warn!(
            spell_id = packet.spell_id,
            "Ignoring unsupported spell cast in starter spell fixture slice"
        );
        return Ok(());
    };
    if !session.active_spells.contains(&packet.spell_id) {
        warn!(
            spell_id = packet.spell_id,
            character_guid,
            "Ignoring starter spell cast for spell not active on character"
        );
        return Ok(());
    }

    let targets = normalize_fixture_spell_targets(packet.targets);
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
        if targets.unit_target == Some(rust_combat_dummy_guid())
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
        } else if let Some(target) = targets.unit_target {
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
                    send_db_creature_motion_stop(stream, broadcast, session, target, header_crypto)
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
                    begin_shared_db_creature_combat(shared_world, session, target, Instant::now())
                        .await;
                }
            }
        }
    }
    }
    if starter_spell.kind == StarterSpellKind::NextMeleeSwing {
        Ok(())
    } else {
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

    let loot_item = select_db_gameobject_loot_item_for_character(
        shared_world.object_mgr,
        world_db_pool,
        session,
        &gameobject.spawn.template,
    )
    .await?;
    let Some((gameobject, loot_item)) = shared_world
        .maps
        .open_db_gameobject_loot(
            request.map_id,
            gameobject_guid.raw(),
            request.character_guid,
            loot_item,
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

    let response = build_gameobject_loot_response_body(gameobject_guid, loot_item.as_ref());
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

fn normalize_fixture_spell_targets(mut targets: SpellCastTargets) -> SpellCastTargets {
    targets.target_mask =
        (targets.target_mask | SPELL_CAST_TARGET_UNIT) & !SPELL_CAST_TARGET_UNIT_ENEMY;
    targets.unit_target = Some(targets.unit_target.unwrap_or_else(rust_combat_dummy_guid));
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
    cooldown_millis: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StarterSpellKind {
    #[allow(dead_code)]
    InstantDamage,
    NextMeleeSwing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StarterSpellPower {
    Rage { cost: u32 },
    Mana { cost: u32 },
}

fn supported_starter_spell(spell_id: u32) -> Option<SupportedStarterSpell> {
    match spell_id {
        WARRIOR_HEROIC_STRIKE_RANK_1 => Some(SupportedStarterSpell {
            spell_id,
            kind: StarterSpellKind::NextMeleeSwing,
            bonus_damage: HEROIC_STRIKE_FIXTURE_DAMAGE,
            damage: 0,
            power: StarterSpellPower::Rage {
                cost: HEROIC_STRIKE_RAGE_COST,
            },
            requires_melee: true,
            triggers_global_cooldown: false,
            cooldown_millis: 0,
        }),
        HUNTER_RAPTOR_STRIKE_RANK_1 => Some(SupportedStarterSpell {
            spell_id,
            kind: StarterSpellKind::NextMeleeSwing,
            bonus_damage: RAPTOR_STRIKE_FIXTURE_DAMAGE,
            damage: RAPTOR_STRIKE_FIXTURE_DAMAGE,
            power: StarterSpellPower::Mana {
                cost: RAPTOR_STRIKE_MANA_COST,
            },
            requires_melee: true,
            triggers_global_cooldown: false,
            cooldown_millis: 0,
        }),
        _ => None,
    }
}

fn apply_starter_spell_cooldowns(
    session: &mut WorldSessionState,
    starter_spell: &SupportedStarterSpell,
    now: Instant,
) {
    if starter_spell.triggers_global_cooldown {
        session.starter_global_cooldown_until =
            Some(now + Duration::from_millis(STARTER_GLOBAL_COOLDOWN_MILLIS));
    }
    if starter_spell.cooldown_millis > 0 {
        session.starter_spell_cooldowns_until.insert(
            starter_spell.spell_id,
            now + Duration::from_millis(starter_spell.cooldown_millis),
        );
    }
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

