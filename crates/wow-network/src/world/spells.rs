async fn handle_cast_spell(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
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
            deps.world_db_pool,
            deps.shared_world,
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
    let Some(spell_template) = deps.shared_world
        .object_mgr
        .spell_template(deps.world_db_pool, packet.spell_id)
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
            deps.shared_world,
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
    let spell_start_body = build_spell_start_body(caster, packet.spell_id, 0, &targets)?;
    send_packet(
        stream,
        SMSG_SPELL_START,
        &spell_start_body,
        Some(&mut *header_crypto),
    )
    .await?;
    let observer_packets = deps.shared_world
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
    deps.shared_world.sessions.dispatch(observer_packets).await;
    if starter_spell.kind == StarterSpellKind::NextMeleeSwing {
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
        send_packet(
            stream,
            SMSG_CAST_RESULT,
            &build_cast_result_ok_body(packet.spell_id),
            Some(&mut *header_crypto),
        )
        .await?;
        let spell_go_body = build_spell_go_body(caster, packet.spell_id, &targets)?;
        send_packet(
            stream,
            SMSG_SPELL_GO,
            &spell_go_body,
            Some(&mut *header_crypto),
        )
        .await?;
        let observer_packets = deps.shared_world
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
        deps.shared_world.sessions.dispatch(observer_packets).await;
        if starter_spell.kind == StarterSpellKind::Charge {
            if let Some(target) = targets.unit_target {
                apply_charge_movement(
                    stream,
                    deps.shared_world,
                    session,
                    caster,
                    target,
                    spell_template.speed,
                    packet.spell_id,
                    header_crypto,
                )
                .await?;
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
                deps.shared_world
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
                    db_creature_player_melee_check_from_map(deps.shared_world, session, target)
                        .await
                        == PlayerMeleeCheck::Clear
                } else {
                    true
                };
                if can_apply_damage {
                    let corpse_loot = if let Some(target_creature) = deps
                        .shared_world
                        .maps
                        .db_creature_snapshot(map_id, target)
                        .await
                        .filter(|creature| starter_spell.damage >= creature.health)
                    {
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
                    if let Some(event) = deps.shared_world
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
                deps.shared_world
                    .maps
                    .spell_duration(spell_template.duration_index),
            );
            match starter_spell.aura_target {
                StarterSpellAuraTarget::Caster => {
                    apply_player_aura(session, aura.clone());
                    if let Some(event) = deps.shared_world
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
                StarterSpellAuraTarget::UnitTarget => {
                    if let Some(target) = targets.unit_target {
                        if target.is_creature() {
                            if let Some(event) = deps.shared_world
                                .maps
                                .apply_db_creature_aura(
                                    map_id,
                                    target,
                                    character_guid,
                                    aura,
                                )
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
        }
        let power_update = match starter_spell.power {
            StarterSpellPower::Rage { .. } => build_player_rage_update_body(caster, session.player_rage)?,
            StarterSpellPower::Mana { .. } => build_player_mana_update_body(caster, session.player_mana)?,
        };
        send_packet(stream, SMSG_UPDATE_OBJECT, &power_update, Some(header_crypto)).await
    }
}

async fn handle_use_item(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let request = UseItemPacket::read(body)?;
    expire_session_auras(session, Instant::now());
    let Some(character) = session.active_character.as_ref() else {
        warn!("Ignoring item use before character login");
        return Ok(());
    };
    let character_guid = character.guid;
    let map_id = character.position.map_id;
    let caster = ObjectGuid::new(HighGuid::Player, 0, character_guid);

    let Some(source_item) = session
        .inventory
        .iter()
        .find(|item| item.bag == request.bag as u32 && item.slot == request.slot)
        .cloned()
    else {
        return send_inventory_change_failure(
            stream,
            EQUIP_ERR_ITEM_NOT_FOUND,
            None,
            None,
            header_crypto,
        )
        .await;
    };
    let item_guid = ObjectGuid::new(HighGuid::Item, 0, source_item.item);
    let Some(template) =
        wow_db::get_item_template_query(deps.world_db_pool, source_item.item_template).await?
    else {
        return send_inventory_change_failure(
            stream,
            EQUIP_ERR_ITEM_NOT_FOUND,
            Some(item_guid),
            None,
            header_crypto,
        )
        .await;
    };

    if template.inventory_type != 0
        && !(source_item.bag == INVENTORY_SLOT_BAG_0 as u32 && source_item.slot < EQUIPMENT_SLOT_END)
    {
        return send_inventory_change_failure(
            stream,
            EQUIP_ERR_ITEM_NOT_FOUND,
            Some(item_guid),
            None,
            header_crypto,
        )
        .await;
    }

    let use_result = character_can_use_item_template(
        character.level,
        character.race,
        character.class,
        &template,
        &session.character_skills,
        &session.active_spells,
        &session.character_reputations,
    );
    if use_result != 0 {
        return send_inventory_change_failure_with_required_level(
            stream,
            use_result,
            Some(item_guid),
            None,
            (use_result == EQUIP_ERR_CANT_EQUIP_LEVEL_I).then_some(template.required_level),
            header_crypto,
        )
        .await;
    }

    let Some(item_spell) = item_use_spell(&template, request.spell_index) else {
        return send_inventory_change_failure(
            stream,
            EQUIP_ERR_ITEM_CANT_BE_EQUIPPED,
            Some(item_guid),
            None,
            header_crypto,
        )
        .await;
    };
    let Some(spell_template) = deps.shared_world
        .object_mgr
        .spell_template(deps.world_db_pool, item_spell.spell_id)
        .await?
    else {
        warn!(
            item = template.entry,
            spell_id = item_spell.spell_id,
            "Ignoring item use with missing spell_template row"
        );
        return send_inventory_change_failure(
            stream,
            EQUIP_ERR_ITEM_CANT_BE_EQUIPPED,
            Some(item_guid),
            None,
            header_crypto,
        )
        .await;
    };

    let Some(item_spell_shape) = supported_item_use_spell(&spell_template) else {
        warn!(
            item = template.entry,
            spell_id = spell_template.id,
            spell_name = spell_template.spell_name.as_str(),
            "Ignoring unsupported item-use spell effect shape"
        );
        return send_inventory_change_failure(
            stream,
            EQUIP_ERR_ITEM_CANT_BE_EQUIPPED,
            Some(item_guid),
            None,
            header_crypto,
        )
        .await;
    };

    let now = Instant::now();
    let targets = normalize_item_use_targets(request.targets, &item_spell_shape, caster);
    let refreshable_consumable_regen = item_spell_shape.kind == StarterSpellKind::AuraApplication
        && spell_periodic_regen_aura(&spell_template, now).is_some();
    if let Some(failure) = item_use_spell_failure(
        session,
        &item_spell_shape,
        now,
        refreshable_consumable_regen,
    ) {
        send_packet(
            stream,
            SMSG_CAST_RESULT,
            &build_cast_result_failure_body(spell_template.id, failure),
            Some(header_crypto),
        )
        .await?;
        send_packet(
            stream,
            SMSG_SPELL_FAILURE,
            &build_spell_failure_body(caster, spell_template.id, failure)?,
            Some(&mut *header_crypto),
        )
        .await?;
        return send_packet(
            stream,
            SMSG_SPELL_FAILED_OTHER,
            &build_spell_failed_other_body(caster, spell_template.id),
            Some(header_crypto),
        )
        .await;
    }

    apply_item_use_spell_cooldowns(
        session,
        &item_spell_shape,
        now,
        refreshable_consumable_regen,
    );
    let spell_start_body =
        build_spell_start_body_with_source(item_guid, caster, spell_template.id, 0, &targets)?;
    send_packet(
        stream,
        SMSG_SPELL_START,
        &spell_start_body,
        Some(&mut *header_crypto),
    )
    .await?;
    deps.shared_world
        .sessions
        .dispatch(
            deps.shared_world
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
                .await,
        )
        .await;
    send_packet(
        stream,
        SMSG_CAST_RESULT,
        &build_cast_result_ok_body(spell_template.id),
        Some(&mut *header_crypto),
    )
    .await?;
    let spell_go_body = build_spell_go_body_with_source(
        item_guid,
        caster,
        spell_template.id,
        CAST_FLAG_SPELL_GO | CAST_FLAG_ITEM_CASTER,
        &targets,
    )?;
    send_packet(
        stream,
        SMSG_SPELL_GO,
        &spell_go_body,
        Some(&mut *header_crypto),
    )
    .await?;
    deps.shared_world
        .sessions
        .dispatch(
            deps.shared_world
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
                .await,
        )
        .await;

    apply_item_use_spell_effects(
        stream,
        deps,
        session,
        caster,
        &spell_template,
        &item_spell_shape,
        now,
        header_crypto,
    )
    .await?;
    if item_spell.spell_charges < 0 {
        consume_used_item(
            stream,
            deps.character_db_pool,
            session,
            character_guid,
            &source_item,
            header_crypto,
        )
        .await?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct SpellCastDeps<'a> {
    character_db_pool: &'a MySqlPool,
    world_db_pool: &'a MySqlPool,
    shared_world: SharedWorldDeps<'a>,
    parties: &'a PartyManager,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UseItemPacket {
    bag: u8,
    slot: u8,
    spell_index: u8,
    targets: SpellCastTargets,
}

impl UseItemPacket {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        if body.len() < 3 {
            anyhow::bail!("CMSG_USE_ITEM payload too short: {} bytes", body.len());
        }
        let mut cursor = 3;
        let targets = if body.len() > cursor {
            SpellCastTargets::read(body, &mut cursor)?
        } else {
            SpellCastTargets {
                target_mask: 0,
                unit_target: None,
                gameobject_target: None,
            }
        };
        Ok(Self {
            bag: normalize_client_bag(body[0]),
            slot: body[1],
            spell_index: body[2],
            targets,
        })
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
    if starter_spell.kind == StarterSpellKind::NextMeleeSwing {
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

async fn starter_spell_charge_cast_failure(
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    targets: &SpellCastTargets,
) -> Option<u8> {
    let target = targets.unit_target?;
    let Some(character) = session.active_character.as_ref() else {
        return Some(SPELL_FAILED_OUT_OF_RANGE);
    };
    let validation = shared_world
        .maps
        .validate_player_charge_against_db_creature(
            character.position.map_id,
            character.guid,
            target,
            &session.db_creature_navigation,
        )
        .await;
    match validation.check {
        PlayerChargeCheck::Clear => None,
        PlayerChargeCheck::NavigationBlocked(DbCreatureNavigationResult::LineOfSightBlocked) => {
            Some(SPELL_FAILED_LINE_OF_SIGHT)
        }
        PlayerChargeCheck::NavigationBlocked(DbCreatureNavigationResult::PathUnavailable) => {
            Some(SPELL_FAILED_NOPATH)
        }
        PlayerChargeCheck::NoActiveCharacter
        | PlayerChargeCheck::MissingTarget
        | PlayerChargeCheck::TargetNotAlive
        | PlayerChargeCheck::NavigationBlocked(_) => Some(SPELL_FAILED_OUT_OF_RANGE),
    }
}

#[allow(clippy::too_many_arguments)]
async fn apply_charge_movement(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    target: ObjectGuid,
    spell_speed: f32,
    spell_id: u32,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.active_character.as_ref() else {
        return Ok(());
    };
    let map_id = character.position.map_id;
    let character_guid = character.guid;
    let start = character.position;
    let Some(creature) = shared_world.maps.db_creature_snapshot(map_id, target).await else {
        return Ok(());
    };
    if !creature.is_alive() {
        return Ok(());
    }

    let destination = charge_destination(start, &creature);
    let speed = if spell_speed > 0.0 {
        spell_speed
    } else {
        BASE_CHARGE_SPEED
    };
    let duration_ms = charge_duration_millis(start, destination, speed);
    let move_body =
        build_monster_move_facing_target_body(caster, start, destination, spell_id, duration_ms, target)?;

    if let Some(character) = session.active_character.as_mut() {
        character.position = destination;
    }
    shared_world
        .maps
        .set_player_position(map_id, character_guid, destination)
        .await;

    send_packet(
        stream,
        SMSG_MONSTER_MOVE,
        &move_body,
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
                opcode: SMSG_MONSTER_MOVE,
                body: move_body,
            },
        )
        .await;
    shared_world.sessions.dispatch(observer_packets).await;

    let next_swing_at = Some(Instant::now());
    mirror_session_player_auto_attack(session, Some(target), next_swing_at);
    shared_world
        .maps
        .set_player_auto_attack(map_id, character_guid, Some(target), next_swing_at)
        .await;

    send_packet(
        stream,
        SMSG_ATTACKSTART,
        &build_attack_start_body(caster, target),
        Some(&mut *header_crypto),
    )
    .await?;
    broadcast_player_attack_start(shared_world, session, caster, target).await;
    Ok(())
}

fn charge_destination(start: WorldPosition, target: &DbCreatureRuntime) -> WorldPosition {
    let target_position = target.current_position;
    let dx = start.x - target_position.x;
    let dy = start.y - target_position.y;
    let distance = (dx * dx + dy * dy).sqrt();
    let reach = creature_combat_reach(&target.spawn.template).max(DEFAULT_WORLD_OBJECT_SIZE);
    let (offset_x, offset_y) = if distance > f32::EPSILON {
        (dx / distance * reach, dy / distance * reach)
    } else {
        (reach, 0.0)
    };
    WorldPosition::new(
        target_position.map_id,
        target_position.x + offset_x,
        target_position.y + offset_y,
        target_position.z + 1.0,
        angle_towards(target_position, start),
    )
}

fn charge_duration_millis(start: WorldPosition, destination: WorldPosition, speed: f32) -> u32 {
    let dx = destination.x - start.x;
    let dy = destination.y - start.y;
    let dz = destination.z - start.z;
    (((dx * dx + dy * dy + dz * dz).sqrt() / speed.max(f32::EPSILON)) * 1000.0)
        .round()
        .max(1.0) as u32
}

fn angle_towards(from: WorldPosition, to: WorldPosition) -> f32 {
    (to.y - from.y).atan2(to.x - from.x)
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
    if session
        .starter_global_cooldowns_until
        .get(&starter_spell.global_cooldown_category)
        .is_some_and(|until| now < *until)
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
    if starter_spell.kind == StarterSpellKind::Charge {
        return starter_spell_charge_cast_failure(shared_world, session, targets).await;
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
        if starter_spell.kind == StarterSpellKind::AuraApplication
            && starter_spell.aura_target == StarterSpellAuraTarget::Caster
        {
            caster
        } else {
            rust_combat_dummy_guid()
        }
    }));
    targets.gameobject_target = None;
    targets
}

fn normalize_item_use_targets(
    mut targets: SpellCastTargets,
    item_spell: &SupportedStarterSpell,
    caster: ObjectGuid,
) -> SpellCastTargets {
    if targets.target_mask == 0 {
        targets.target_mask = SPELL_CAST_TARGET_UNIT;
        targets.unit_target = Some(caster);
        return targets;
    }
    if item_spell.kind == StarterSpellKind::AuraApplication
        && item_spell.aura_target == StarterSpellAuraTarget::Caster
    {
        targets.target_mask = (targets.target_mask | SPELL_CAST_TARGET_UNIT)
            & !SPELL_CAST_TARGET_UNIT_ENEMY;
        targets.unit_target = Some(caster);
        targets.gameobject_target = None;
    }
    targets
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SupportedStarterSpell {
    spell_id: u32,
    kind: StarterSpellKind,
    aura_target: StarterSpellAuraTarget,
    bonus_damage: u32,
    damage: u32,
    power: StarterSpellPower,
    requires_melee: bool,
    global_cooldown_category: u32,
    global_cooldown_millis: u64,
    cooldown_millis: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StarterSpellKind {
    InstantDamage,
    AuraApplication,
    Charge,
    NextMeleeSwing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StarterSpellAuraTarget {
    Caster,
    UnitTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StarterSpellPower {
    Rage { cost: u32 },
    Mana { cost: u32 },
}

const SPELL_ATTR_ON_NEXT_SWING_NO_DAMAGE: u32 = 0x0000_0004;
const SPELL_ATTR_PASSIVE: u32 = 0x0000_0040;
const SPELL_ATTR_ON_NEXT_SWING: u32 = 0x0000_0400;
const SPELL_EFFECT_WEAPON_DAMAGE_NOSCHOOL: u32 = 17;
const SPELL_EFFECT_WEAPON_PERCENT_DAMAGE: u32 = 58;
const SPELL_EFFECT_APPLY_AURA: u32 = 6;
const SPELL_EFFECT_HEAL: u32 = 10;
const SPELL_EFFECT_ENERGIZE: u32 = 30;
const SPELL_EFFECT_CHARGE: u32 = 96;
const SPELL_AURA_PERIODIC_DAMAGE: u32 = 3;
const SPELL_AURA_PERIODIC_HEAL: u32 = 8;
const SPELL_AURA_OBS_MOD_HEALTH: u32 = 20;
const SPELL_AURA_PERIODIC_ENERGIZE: u32 = 24;
const SPELL_AURA_MOD_SKILL_TALENT: u32 = 98;
const SPELL_AURA_MOD_SKILL: u32 = 30;
const SPELL_AURA_MOD_REGEN: u32 = 84;
const SPELL_AURA_MOD_POWER_REGEN: u32 = 85;
const SPELL_AURA_MOD_ATTACK_POWER: u32 = 99;
const SPELL_AURA_MOD_TOTAL_STAT_PERCENTAGE: u32 = 137;
const SPELL_AURA_MOD_REPUTATION_GAIN: u32 = 156;
const AURA_INTERRUPT_FLAG_DAMAGE: u32 = 0x0000_0002;
const AURA_INTERRUPT_FLAG_MOVING: u32 = 0x0000_0008;
const AURA_INTERRUPT_FLAG_STANDING_CANCELS: u32 = 0x0004_0000;
const PLAYER_STAND_STATE_STAND: u8 = 0;
const PLAYER_STAND_STATE_SIT: u8 = 1;
const POWER_TYPE_MANA: u32 = 0;
const POWER_TYPE_RAGE: u32 = 1;
const POSITIVE_AURA_FLAGS: u32 = 0x05;
const NEGATIVE_AURA_FLAGS: u32 = 0x08;
const TARGET_UNIT_CASTER: u32 = 1;
const TARGET_UNIT_ENEMY: u32 = 6;
const TARGET_UNIT: u32 = 25;
const ITEM_SPELLTRIGGER_ON_USE: u32 = 0;
const ITEM_SPELLTRIGGER_ON_NO_DELAY_USE: u32 = 5;
const BASE_CHARGE_SPEED: f32 = 27.0;
const MAX_AURA_SLOTS: usize = 48;
const MAX_POSITIVE_AURA_SLOTS: usize = 32;
const MAX_AURA_FLAG_FIELDS: usize = 6;
const MAX_AURA_LEVEL_FIELDS: usize = 12;

fn supported_starter_spell(template: &wow_db::SpellTemplateQuery) -> Option<SupportedStarterSpell> {
    let kind = if spell_has_on_next_swing_attribute(template) {
        StarterSpellKind::NextMeleeSwing
    } else if spell_has_charge_effect(template) {
        StarterSpellKind::Charge
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
        aura_target: starter_spell_aura_target(template),
        bonus_damage: spell_bonus_damage(template),
        damage: spell_direct_damage(template),
        power: spell_power(template),
        requires_melee: kind == StarterSpellKind::NextMeleeSwing
            || (template.dmg_class == 2 && kind != StarterSpellKind::Charge),
        global_cooldown_category: template.start_recovery_category,
        global_cooldown_millis: template.start_recovery_time as u64,
        cooldown_millis: template.recovery_time.max(template.category_recovery_time) as u64,
    })
}

fn supported_item_use_spell(template: &wow_db::SpellTemplateQuery) -> Option<SupportedStarterSpell> {
    if spell_has_charge_effect(template) || spell_has_on_next_swing_attribute(template) {
        return None;
    }
    let kind = if spell_has_aura_application(template) {
        StarterSpellKind::AuraApplication
    } else if spell_has_item_direct_effect(template) {
        StarterSpellKind::InstantDamage
    } else {
        return None;
    };
    Some(SupportedStarterSpell {
        spell_id: template.id,
        kind,
        aura_target: starter_spell_aura_target(template),
        bonus_damage: 0,
        damage: spell_direct_damage(template),
        power: spell_power(template),
        requires_melee: false,
        global_cooldown_category: template.start_recovery_category,
        global_cooldown_millis: template.start_recovery_time as u64,
        cooldown_millis: template.recovery_time.max(template.category_recovery_time) as u64,
    })
}

fn spell_has_item_direct_effect(template: &wow_db::SpellTemplateQuery) -> bool {
    spell_effects(template).into_iter().any(|effect| {
        matches!(effect.effect, SPELL_EFFECT_HEAL | SPELL_EFFECT_ENERGIZE)
            && spell_effect_simple_value(effect.base_points).is_some()
    })
}

fn spell_has_on_next_swing_attribute(template: &wow_db::SpellTemplateQuery) -> bool {
    (template.attributes & (SPELL_ATTR_ON_NEXT_SWING_NO_DAMAGE | SPELL_ATTR_ON_NEXT_SWING)) != 0
}

fn spell_has_aura_application(template: &wow_db::SpellTemplateQuery) -> bool {
    [template.effect1, template.effect2, template.effect3].contains(&SPELL_EFFECT_APPLY_AURA)
}

fn spell_has_charge_effect(template: &wow_db::SpellTemplateQuery) -> bool {
    [template.effect1, template.effect2, template.effect3].contains(&SPELL_EFFECT_CHARGE)
}

fn starter_spell_aura_target(template: &wow_db::SpellTemplateQuery) -> StarterSpellAuraTarget {
    spell_effects(template)
        .into_iter()
        .find(|effect| effect.effect == SPELL_EFFECT_APPLY_AURA)
        .map(|effect| match effect.implicit_target_a {
            TARGET_UNIT_CASTER => StarterSpellAuraTarget::Caster,
            TARGET_UNIT_ENEMY | TARGET_UNIT => StarterSpellAuraTarget::UnitTarget,
            _ => StarterSpellAuraTarget::Caster,
        })
        .unwrap_or(StarterSpellAuraTarget::Caster)
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
    if spell_has_on_next_swing_attribute(template) || spell_has_charge_effect(template) {
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
    amplitude: u32,
    implicit_target_a: u32,
    misc_value: i32,
}

fn spell_direct_heal(template: &wow_db::SpellTemplateQuery) -> u32 {
    spell_effects(template)
        .into_iter()
        .filter(|effect| effect.effect == SPELL_EFFECT_HEAL)
        .filter_map(|effect| spell_effect_simple_value(effect.base_points))
        .sum()
}

fn spell_direct_energize(template: &wow_db::SpellTemplateQuery) -> u32 {
    spell_effects(template)
        .into_iter()
        .filter(|effect| effect.effect == SPELL_EFFECT_ENERGIZE)
        .filter_map(|effect| spell_effect_simple_value(effect.base_points))
        .sum()
}

fn spell_effects(template: &wow_db::SpellTemplateQuery) -> [SpellEffectData; 3] {
    [
        SpellEffectData {
            effect: template.effect1,
            aura_name: template.effect_apply_aura_name1,
            base_points: template.effect_base_points1,
            amplitude: template.effect_amplitude1,
            implicit_target_a: template.effect_implicit_target_a1,
            misc_value: template.effect_misc_value1,
        },
        SpellEffectData {
            effect: template.effect2,
            aura_name: template.effect_apply_aura_name2,
            base_points: template.effect_base_points2,
            amplitude: template.effect_amplitude2,
            implicit_target_a: template.effect_implicit_target_a2,
            misc_value: template.effect_misc_value2,
        },
        SpellEffectData {
            effect: template.effect3,
            aura_name: template.effect_apply_aura_name3,
            base_points: template.effect_base_points3,
            amplitude: template.effect_amplitude3,
            implicit_target_a: template.effect_implicit_target_a3,
            misc_value: template.effect_misc_value3,
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
        positive: active_aura_is_positive(template),
        visible: true,
        duration_millis: (duration_millis > 0).then_some(duration_millis as u32),
        expires_at: (duration_millis > 0)
            .then_some(now + Duration::from_millis(duration_millis as u64)),
        periodic_damage: spell_periodic_damage_aura(template, now),
        periodic_regen: spell_periodic_regen_aura(template, now),
        stat_modifiers: spell_aura_stat_modifiers(template),
    }
}

fn active_aura_is_positive(template: &wow_db::SpellTemplateQuery) -> bool {
    !spell_effects(template)
        .into_iter()
        .any(|effect| {
            effect.effect == SPELL_EFFECT_APPLY_AURA
                && (effect.implicit_target_a == TARGET_UNIT_ENEMY
                    || effect.aura_name == SPELL_AURA_PERIODIC_DAMAGE)
        })
}

fn spell_periodic_damage_aura(
    template: &wow_db::SpellTemplateQuery,
    now: Instant,
) -> Option<PeriodicDamageAura> {
    spell_effects(template)
        .into_iter()
        .find(|effect| {
            effect.effect == SPELL_EFFECT_APPLY_AURA
                && effect.aura_name == SPELL_AURA_PERIODIC_DAMAGE
                && effect.amplitude > 0
        })
        .and_then(|effect| {
            let damage = spell_effect_simple_value(effect.base_points)?;
            Some(PeriodicDamageAura {
                aura_name: effect.aura_name,
                school: template.school,
                damage_class: template.dmg_class,
                amount: damage,
                tick_millis: effect.amplitude,
                next_tick_at: now + Duration::from_millis(effect.amplitude as u64),
            })
        })
}

fn spell_periodic_regen_aura(
    template: &wow_db::SpellTemplateQuery,
    now: Instant,
) -> Option<PeriodicRegenAura> {
    let mut health_amount = 0u32;
    let mut mana_amount = 0u32;
    let mut tick_millis = 0u32;
    for effect in spell_effects(template) {
        if effect.effect != SPELL_EFFECT_APPLY_AURA {
            continue;
        }
        let Some(amount) = spell_effect_simple_value(effect.base_points) else {
            continue;
        };
        match effect.aura_name {
            SPELL_AURA_PERIODIC_HEAL | SPELL_AURA_OBS_MOD_HEALTH | SPELL_AURA_MOD_REGEN => {
                health_amount = health_amount.saturating_add(amount);
                tick_millis = tick_millis.max(effect.amplitude);
            }
            SPELL_AURA_PERIODIC_ENERGIZE | SPELL_AURA_MOD_POWER_REGEN
                if effect.misc_value == POWER_TYPE_MANA as i32 =>
            {
                mana_amount = mana_amount.saturating_add(amount);
                tick_millis = tick_millis.max(effect.amplitude);
            }
            _ => {}
        }
    }
    if health_amount == 0 && mana_amount == 0 {
        return None;
    }
    let tick_millis = tick_millis.max(2_000);
    Some(PeriodicRegenAura {
        health_amount,
        mana_amount,
        tick_millis,
        next_tick_at: now + Duration::from_millis(tick_millis as u64),
    })
}

fn passive_spell_active_aura(
    template: &wow_db::SpellTemplateQuery,
    caster: ObjectGuid,
    level: u8,
    now: Instant,
    duration: Option<SpellDurationEntry>,
) -> Option<ActiveAura> {
    if !spell_needs_passive_cast_at_learn(template) {
        return None;
    }
    let mut aura = build_active_aura(template, caster, level, now, duration);
    aura.visible = false;
    (!aura.stat_modifiers.is_empty()).then_some(aura)
}

fn spell_needs_passive_cast_at_learn(template: &wow_db::SpellTemplateQuery) -> bool {
    template.attributes & SPELL_ATTR_PASSIVE != 0 && spell_has_aura_application(template)
}

fn spell_aura_stat_modifiers(template: &wow_db::SpellTemplateQuery) -> Vec<AuraStatModifier> {
    spell_effects(template)
        .into_iter()
        .filter(|effect| effect.effect == SPELL_EFFECT_APPLY_AURA)
        .filter_map(|effect| match effect.aura_name {
            SPELL_AURA_MOD_SKILL | SPELL_AURA_MOD_SKILL_TALENT => {
                let skill_id = u16::try_from(effect.misc_value).ok()?;
                Some(AuraStatModifier::Skill {
                    skill_id,
                    amount: spell_effect_simple_i32(effect.base_points)
                        .clamp(i16::MIN as i32, i16::MAX as i32)
                        as i16,
                    permanent: effect.aura_name == SPELL_AURA_MOD_SKILL_TALENT,
                })
            }
            SPELL_AURA_MOD_ATTACK_POWER => Some(AuraStatModifier::AttackPower {
                amount: spell_effect_simple_i32(effect.base_points),
            }),
            SPELL_AURA_MOD_TOTAL_STAT_PERCENTAGE => {
                let stat = usize::try_from(effect.misc_value).ok()?;
                (stat < MAX_STATS).then_some(AuraStatModifier::TotalStatPercent {
                    stat,
                    percent: spell_effect_simple_i32(effect.base_points),
                })
            }
            SPELL_AURA_MOD_REPUTATION_GAIN => Some(AuraStatModifier::ReputationGainPercent {
                percent: spell_effect_simple_i32(effect.base_points),
            }),
            _ => None,
        })
        .collect()
}

fn active_aura_skill_bonus(active_auras: &[ActiveAura], skill_id: u16) -> i16 {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .filter_map(|modifier| match modifier {
            AuraStatModifier::Skill {
                skill_id: modifier_skill,
                amount,
                ..
            } if *modifier_skill == skill_id => Some(*amount),
            _ => None,
        })
        .sum()
}

fn active_aura_skill_bonus_pair(active_auras: &[ActiveAura], skill_id: u16) -> u32 {
    let mut temporary = 0i32;
    let mut permanent = 0i32;
    for modifier in active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
    {
        if let AuraStatModifier::Skill {
            skill_id: modifier_skill,
            amount,
            permanent: is_permanent,
        } = modifier
        {
            if *modifier_skill == skill_id {
                if *is_permanent {
                    permanent += i32::from(*amount);
                } else {
                    temporary += i32::from(*amount);
                }
            }
        }
    }
    make_pair32(
        temporary.clamp(i16::MIN as i32, i16::MAX as i32) as i16 as u16,
        permanent.clamp(i16::MIN as i32, i16::MAX as i32) as i16 as u16,
    )
}

fn current_skill_value_with_active_auras(
    character_skills: &[CharacterSkill],
    active_auras: &[ActiveAura],
    skill_id: u16,
) -> u16 {
    let value = i32::from(current_skill_value(character_skills, skill_id));
    let bonus = i32::from(active_aura_skill_bonus(active_auras, skill_id));
    value.saturating_add(bonus).clamp(0, u16::MAX as i32) as u16
}

fn reputation_gain_percent_from_active_auras(active_auras: &[ActiveAura]) -> i32 {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .filter_map(|modifier| match modifier {
            AuraStatModifier::ReputationGainPercent { percent } => Some(*percent),
            _ => None,
        })
        .sum()
}

fn player_world_stats_with_active_auras(
    mut world_stats: PlayerWorldStats,
    active_auras: &[ActiveAura],
) -> PlayerWorldStats {
    for stat in 0..MAX_STATS {
        let percent = active_auras
            .iter()
            .flat_map(|aura| aura.stat_modifiers.iter())
            .filter_map(|modifier| match modifier {
                AuraStatModifier::TotalStatPercent {
                    stat: modifier_stat,
                    percent,
                } if *modifier_stat == stat => Some(*percent),
                _ => None,
            })
            .sum::<i32>();
        if percent != 0 {
            world_stats.stats[stat] = apply_percent_modifier(world_stats.stats[stat], percent);
        }
    }
    world_stats
}

fn player_stat_mod_deltas(
    base_world_stats: &PlayerWorldStats,
    effective_world_stats: &PlayerWorldStats,
) -> [i32; MAX_STATS] {
    let mut deltas = [0i32; MAX_STATS];
    for (offset, delta) in deltas.iter_mut().enumerate() {
        *delta =
            effective_world_stats.stats[offset] as i32 - base_world_stats.stats[offset] as i32;
    }
    deltas
}

fn apply_percent_modifier(value: u32, percent: i32) -> u32 {
    let multiplier = 100i64.saturating_add(i64::from(percent));
    if multiplier <= 0 {
        return 0;
    }
    ((i64::from(value) * multiplier) / 100).clamp(0, u32::MAX as i64) as u32
}

fn spell_effect_simple_i32(base_points: i32) -> i32 {
    base_points.saturating_add(1)
}

fn apply_starter_spell_cooldowns(
    session: &mut WorldSessionState,
    starter_spell: &SupportedStarterSpell,
    now: Instant,
) {
    if starter_spell.global_cooldown_millis > 0 {
        session.starter_global_cooldowns_until.insert(
            starter_spell.global_cooldown_category,
            now + Duration::from_millis(starter_spell.global_cooldown_millis),
        );
    }
    if starter_spell.cooldown_millis > 0 {
        session.starter_spell_cooldowns_until.insert(
            starter_spell.spell_id,
            now + Duration::from_millis(starter_spell.cooldown_millis),
        );
    }
}

fn apply_item_use_spell_cooldowns(
    session: &mut WorldSessionState,
    item_spell: &SupportedStarterSpell,
    now: Instant,
    skip_spell_cooldown: bool,
) {
    if item_spell.global_cooldown_millis > 0 {
        session.starter_global_cooldowns_until.insert(
            item_spell.global_cooldown_category,
            now + Duration::from_millis(item_spell.global_cooldown_millis),
        );
    }
    if !skip_spell_cooldown && item_spell.cooldown_millis > 0 {
        session.starter_spell_cooldowns_until.insert(
            item_spell.spell_id,
            now + Duration::from_millis(item_spell.cooldown_millis),
        );
    }
}

fn item_use_spell_failure(
    session: &WorldSessionState,
    item_spell: &SupportedStarterSpell,
    now: Instant,
    ignore_spell_cooldown: bool,
) -> Option<u8> {
    let refreshing_active_aura = item_spell.kind == StarterSpellKind::AuraApplication
        && session
            .active_auras
            .iter()
            .any(|aura| aura.spell_id == item_spell.spell_id);
    if refreshing_active_aura {
        return None;
    }
    if !ignore_spell_cooldown {
        if let Some(until) = session
            .starter_spell_cooldowns_until
            .get(&item_spell.spell_id)
            .copied()
        {
            if now < until {
                return Some(SPELL_FAILED_NOT_READY);
            }
        }
    }
    if session
        .starter_global_cooldowns_until
        .get(&item_spell.global_cooldown_category)
        .is_some_and(|until| now < *until)
    {
        return Some(SPELL_FAILED_NOT_READY);
    }
    None
}

#[allow(clippy::too_many_arguments)]
async fn apply_item_use_spell_effects(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    spell_template: &wow_db::SpellTemplateQuery,
    item_spell: &SupportedStarterSpell,
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

    let heal = spell_direct_heal(spell_template);
    if heal != 0 {
        session.player_health = session.player_health.saturating_add(heal).min(max_health);
        update_bodies.push(build_player_health_update_body(caster, session.player_health)?);
    }
    let energize = spell_direct_energize(spell_template);
    if energize != 0 && max_mana != 0 {
        session.player_mana = session.player_mana.saturating_add(energize).min(max_mana);
        update_bodies.push(build_player_mana_update_body(caster, session.player_mana)?);
    }
    if item_spell.kind == StarterSpellKind::AuraApplication {
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
                &character_snapshot,
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
                            &character_snapshot,
                            session.player_stand_state,
                        )?,
                    },
                )
                .await;
            deps.shared_world.sessions.dispatch(observer_packets).await;
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

async fn consume_used_item(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    session: &mut WorldSessionState,
    character_guid: u32,
    source_item: &CharacterInventoryItem,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let destroyed = wow_db::destroy_character_inventory_item_count(
        character_db_pool,
        character_guid,
        source_item.bag,
        source_item.slot,
        1,
    )
    .await?;
    let Some(destroyed) = destroyed else {
        return Ok(());
    };
    session.inventory = wow_db::get_character_inventory_items(character_db_pool, character_guid).await?;
    match destroyed {
        wow_db::InventoryDestroyResult::CountChanged { item, count } => {
            let body = build_item_stack_count_update_body(item, count)?;
            send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(header_crypto)).await?;
        }
        wow_db::InventoryDestroyResult::Removed { item } => {
            if source_item.bag == INVENTORY_SLOT_BAG_0 as u32 {
                let body = build_inventory_slots_update_body(
                    character_guid,
                    &session.inventory,
                    &[source_item.slot],
                )?;
                send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(header_crypto)).await?;
            } else {
                let body = build_destroy_object_body(item);
                send_packet(stream, SMSG_DESTROY_OBJECT, &body, Some(header_crypto)).await?;
            }
        }
    }
    Ok(())
}

fn item_use_spell(
    template: &ItemTemplateQuery,
    requested_index: u8,
) -> Option<wow_db::ItemTemplateSpell> {
    let requested = template.spells.get(requested_index as usize).copied();
    requested
        .filter(|spell| is_item_use_spell(*spell))
        .or_else(|| template.spells.into_iter().find(|spell| is_item_use_spell(*spell)))
}

fn is_item_use_spell(spell: wow_db::ItemTemplateSpell) -> bool {
    spell.spell_id != 0
        && matches!(
            spell.spell_trigger,
            ITEM_SPELLTRIGGER_ON_USE | ITEM_SPELLTRIGGER_ON_NO_DELAY_USE
        )
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

fn active_aura_interrupt_flags(aura: &ActiveAura) -> u32 {
    if aura.periodic_regen.is_some() {
        AURA_INTERRUPT_FLAG_DAMAGE | AURA_INTERRUPT_FLAG_MOVING | AURA_INTERRUPT_FLAG_STANDING_CANCELS
    } else {
        0
    }
}

fn remove_active_auras_with_interrupt_flag(
    active_auras: &mut Vec<ActiveAura>,
    interrupt_flag: u32,
) -> bool {
    let before = active_auras.len();
    active_auras.retain(|aura| active_aura_interrupt_flags(aura) & interrupt_flag == 0);
    active_auras.len() != before
}

async fn interrupt_player_consumable_auras(
    stream: &mut WorldPacketSink,
    maps: &Arc<MapRuntimeManager>,
    sessions: &Arc<SessionRegistry>,
    session: &mut WorldSessionState,
    interrupt_flag: u32,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    if !remove_active_auras_with_interrupt_flag(&mut session.active_auras, interrupt_flag) {
        return Ok(false);
    }
    session.player_stand_state = PLAYER_STAND_STATE_STAND;
    let Some(character) = session.active_character.as_ref() else {
        return Ok(true);
    };
    let map_id = character.position.map_id;
    let character_guid = character.guid;
    let player = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let aura_packet = OutboundWorldPacket {
        opcode: SMSG_UPDATE_OBJECT,
        body: build_player_aura_update_body(player, &session.active_auras)?,
    };
    let stand_packet = OutboundWorldPacket {
        opcode: SMSG_UPDATE_OBJECT,
        body: build_player_stand_state_update_body(character, session.player_stand_state)?,
    };
    send_packet(
        stream,
        aura_packet.opcode,
        &aura_packet.body,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        stand_packet.opcode,
        &stand_packet.body,
        Some(header_crypto),
    )
    .await?;
    maps.sync_player_gameplay_state(map_id, character_guid, session)
        .await;
    let mut observer_packets = maps
        .broadcast_nearby_player_packet(
            map_id,
            character_guid,
            PLAYER_VISIBILITY_RADIUS_YARDS,
            aura_packet,
        )
        .await;
    observer_packets.extend(
        maps.broadcast_nearby_player_packet(
            map_id,
            character_guid,
            PLAYER_VISIBILITY_RADIUS_YARDS,
            stand_packet,
        )
        .await,
    );
    sessions.dispatch(observer_packets).await;
    Ok(true)
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
    set_unit_aura_update_values(values, active_auras)
}

fn set_unit_aura_update_values(
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

    for (slot, aura) in visible_aura_slots(active_auras) {
        set_update_value(values, UNIT_FIELD_AURA + slot, aura.spell_id)?;
        let flags_index = UNIT_FIELD_AURAFLAGS + (slot / 8);
        let flags_shift = ((slot % 8) * 4) as u32;
        let previous = values[flags_index].unwrap_or(0);
        let flags = if aura.positive {
            POSITIVE_AURA_FLAGS
        } else {
            NEGATIVE_AURA_FLAGS
        };
        set_update_value(values, flags_index, previous | (flags << flags_shift))?;

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

fn visible_aura_slots(active_auras: &[ActiveAura]) -> Vec<(usize, &ActiveAura)> {
    let mut positive_slot = 0;
    let mut negative_slot = MAX_POSITIVE_AURA_SLOTS;
    let mut slots = Vec::new();
    for aura in active_auras.iter().filter(|aura| aura.visible) {
        let slot = if aura.positive {
            if positive_slot >= MAX_POSITIVE_AURA_SLOTS {
                continue;
            }
            let slot = positive_slot;
            positive_slot += 1;
            slot
        } else {
            if negative_slot >= MAX_AURA_SLOTS {
                continue;
            }
            let slot = negative_slot;
            negative_slot += 1;
            slot
        };
        slots.push((slot, aura));
    }
    slots
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
    visible_aura_slots(active_auras)
        .into_iter()
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
    build_spell_go_body_with_source(caster, caster, spell_id, CAST_FLAG_SPELL_GO, targets)
}

fn build_spell_go_body_with_source(
    source: ObjectGuid,
    caster: ObjectGuid,
    spell_id: u32,
    cast_flags: u16,
    targets: &SpellCastTargets,
) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(40);
    PackedGuid::write(&mut body, source)?;
    PackedGuid::write(&mut body, caster)?;
    body.extend_from_slice(&spell_id.to_le_bytes());
    body.extend_from_slice(&cast_flags.to_le_bytes());

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
    build_spell_start_body_with_source(caster, caster, spell_id, cast_time_ms, targets)
}

fn build_spell_start_body_with_source(
    source: ObjectGuid,
    caster: ObjectGuid,
    spell_id: u32,
    cast_time_ms: u32,
    targets: &SpellCastTargets,
) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(44);
    PackedGuid::write(&mut body, source)?;
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
    let spell_cooldowns = if let Some(template) = template.as_ref() {
        Some(item_query_spell_cooldowns(world_db_pool, template).await?)
    } else {
        None
    };
    info!(
        item,
        found = template.is_some(),
        "Answering item template query"
    );
    let response = build_item_query_single_response_with_spell_cooldowns(
        item,
        template.as_ref(),
        spell_cooldowns.as_ref(),
    );
    send_packet(
        stream,
        SMSG_ITEM_QUERY_SINGLE_RESPONSE,
        &response,
        Some(header_crypto),
    )
    .await
}

async fn item_query_spell_cooldowns(
    world_db_pool: &MySqlPool,
    template: &wow_db::ItemTemplateQuery,
) -> anyhow::Result<[Option<ItemQuerySpellCooldown>; 5]> {
    let mut cooldowns = [None; 5];
    for (index, spell) in template.spells.iter().enumerate() {
        if spell.spell_id == 0 {
            continue;
        }
        let Some(spell_template) = wow_db::get_spell_template_query(world_db_pool, spell.spell_id).await?
        else {
            continue;
        };
        cooldowns[index] = Some(ItemQuerySpellCooldown {
            recovery_time: spell_template.recovery_time.min(i32::MAX as u32) as i32,
            category: spell_template.category,
            category_recovery_time: spell_template
                .category_recovery_time
                .min(i32::MAX as u32) as i32,
        });
    }
    Ok(cooldowns)
}

async fn handle_item_name_query(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if body.len() < 4 {
        anyhow::bail!("CMSG_ITEM_NAME_QUERY payload too short: {} bytes", body.len());
    }

    let item = u32::from_le_bytes(body[0..4].try_into()?);
    let Some(template) = wow_db::get_item_template_query(world_db_pool, item).await? else {
        warn!(item, "Ignoring item name query for unknown item");
        return Ok(());
    };
    let response = build_item_name_query_response(&template);
    send_packet(
        stream,
        SMSG_ITEM_NAME_QUERY_RESPONSE,
        &response,
        Some(header_crypto),
    )
    .await
}

