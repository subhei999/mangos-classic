use super::*;
use wow_proto::{ServerWorldPacket, SpellCastTargets};

pub(in crate::world) async fn handle_cast_spell(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    packet: wow_proto::CastSpellRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    expire_session_auras(session, Instant::now());
    let Some(character) = &session.character.active_character else {
        warn!(
            spell_id = packet.spell_id,
            "Ignoring spell cast before character login"
        );
        return Ok(());
    };
    let character_guid = character.guid;
    let map_id = character.position.map_id;
    let caster = ObjectGuid::new(HighGuid::Player, 0, character_guid);

    if let Some(spell_template) = deps
        .shared_world
        .object_mgr
        .spell_template(deps.world_db_pool, packet.spell_id)
        .await?
    {
        let spell_info = SpellInfo::from_template(&spell_template);
        let targets_gameobject = packet
            .targets
            .gameobject_target
            .or(packet.targets.unit_target)
            .is_some_and(|target| target.is_game_object());
        if targets_gameobject && spell_info.has_effect(SpellEffectDispatch::OpenLock) {
            let request = OpeningSpellRequest {
                spell_id: packet.spell_id,
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
    }

    if !session.character.active_spells.contains(&packet.spell_id) {
        warn!(
            spell_id = packet.spell_id,
            character_guid, "Ignoring spell cast for spell not active on character"
        );
        return Ok(());
    }
    let Some(spell_template) = deps
        .shared_world
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
    let spell_info = SpellInfo::from_template(&spell_template);
    let Some(mut prepared_spell) = spell_info.prepare_player_cast() else {
        warn!(
            spell_id = packet.spell_id,
            spell_name = spell_template.spell_name.as_str(),
            "Ignoring unsupported spell effect shape in starter spell slice"
        );
        return Ok(());
    };
    prepared_spell.prepare();
    let mut spell_profile = prepared_spell.profile;
    let power_value_context = player_spell_effect_value_context(
        deps.shared_world.maps,
        &spell_template,
        &session.character.character_skills,
        0,
    );
    spell_profile.power = spell_info.power_with_context(power_value_context);
    prepared_spell.profile = spell_profile;
    let cast_time_ms = spell_cast_time_millis(
        deps.shared_world
            .maps
            .spell_cast_time(spell_template.casting_time_index),
    );

    let targets = resolve_player_spell_cast_targets(
        deps.shared_world.maps,
        map_id,
        character_guid,
        normalize_spell_cast_targets(packet.targets, &spell_profile, &spell_info, caster),
        &spell_info,
        spell_profile.kind,
    )
    .await;
    let now = Instant::now();
    stand_player_for_spell_cast(stream, deps.shared_world, session, header_crypto).await?;
    if let Some(failure) = spell_cast_failure(
        deps.shared_world,
        deps.world_db_pool,
        session,
        &spell_template,
        &spell_profile,
        &targets,
        now,
    )
    .await
    {
        return send_spell_cast_failure(stream, caster, packet.spell_id, failure, header_crypto)
            .await;
    }
    if let Some(failure) = player_aura_rank_cast_failure(
        deps,
        session,
        &spell_template,
        &spell_profile,
        &targets,
        caster,
    )
    .await?
    {
        begin_failed_hostile_db_creature_spell_retaliation(
            stream,
            deps.shared_world,
            session,
            caster,
            map_id,
            &spell_template,
            &spell_profile,
            &targets,
            header_crypto,
        )
        .await?;
        return send_spell_cast_failure(stream, caster, packet.spell_id, failure, header_crypto)
            .await;
    }
    if spell_profile.kind == SpellCastKind::CreateItem {
        if let Some(failure) =
            player_create_item_cast_inventory_failure(deps, session, &spell_template).await?
        {
            return send_inventory_change_failure(stream, failure, None, None, header_crypto).await;
        }
    }
    if spell_profile.kind == SpellCastKind::AutoRepeatRanged {
        if let Some(failure) = player_ranged_auto_attack_failure(
            deps.world_db_pool,
            deps.shared_world,
            session,
            targets.unit_target.unwrap_or(caster),
            &spell_template,
        )
        .await?
        {
            send_auto_repeat_spell_failure(stream, header_crypto, caster, packet.spell_id, failure)
                .await?;
            return Ok(());
        }
    }
    let ranged_ammo_visual = if spell_profile.kind == SpellCastKind::AutoRepeatRanged {
        player_ranged_spell_ammo_visual(deps.world_db_pool, session).await?
    } else {
        None
    };
    let spell_start_body = if spell_profile.kind == SpellCastKind::AutoRepeatRanged {
        prepared_spell.start_casting();
        build_spell_start_body_with_ammo(
            caster,
            prepared_spell.spell_id,
            cast_time_ms,
            &targets,
            ranged_ammo_visual,
        )?
    } else {
        prepared_spell.spell_start_body(caster, cast_time_ms, &targets)?
    };
    send_packet(
        stream,
        SMSG_SPELL_START,
        &spell_start_body,
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
                opcode: SMSG_SPELL_START,
                body: spell_start_body,
            },
        )
        .await;
    deps.shared_world.sessions.dispatch(observer_packets).await;
    if spell_profile.kind == SpellCastKind::AutoRepeatRanged {
        let next_shot_at = deps
            .shared_world
            .maps
            .set_player_ranged_auto_attack_started(
                map_id,
                character_guid,
                targets.unit_target,
                now + Duration::from_millis(PLAYER_RANGED_AUTO_ATTACK_WINDUP_MILLIS),
                packet.spell_id,
            )
            .await;
        mirror_session_player_auto_attack(session, targets.unit_target, Some(next_shot_at));
    } else if spell_profile.kind == SpellCastKind::NextMeleeSwing {
        deps.shared_world
            .maps
            .apply_player_spell_cooldowns(map_id, character_guid, &spell_profile, now, false)
            .await;
        if let Some(target) = targets.unit_target {
            let (rage_cost, mana_cost) = match spell_profile.power {
                SpellPowerCost::Rage { cost } => (cost, 0),
                SpellPowerCost::Mana { cost } => (0, cost),
                SpellPowerCost::Energy { .. } => (0, 0),
            };
            let value_context = player_spell_effect_value_context(
                deps.shared_world.maps,
                &spell_template,
                &session.character.character_skills,
                0,
            );
            let queued = QueuedNextMeleeSpell {
                spell_id: packet.spell_id,
                target,
                bonus_damage: spell_info.bonus_damage_with_context(value_context),
                rage_cost,
                mana_cost,
            };
            deps.shared_world
                .maps
                .queue_player_next_melee_spell(map_id, character_guid, queued)
                .await;
        }
    } else {
        deps.shared_world
            .maps
            .apply_player_spell_cooldowns(map_id, character_guid, &spell_profile, now, false)
            .await;
        if cast_time_ms > 0 {
            deps.shared_world
                .maps
                .set_active_player_spell_cast(
                    map_id,
                    character_guid,
                    ActivePlayerSpellCast {
                        spell_id: packet.spell_id,
                        source: ActivePlayerSpellCastSource::Player,
                        profile: spell_profile,
                        targets: PendingSpellCastTargets::from_spell_targets(&targets),
                        due_at: now + Duration::from_millis(cast_time_ms as u64),
                        cast_time_millis: cast_time_ms,
                        interrupt_flags: spell_template.interrupt_flags,
                        damage_pushback_count: 0,
                    },
                )
                .await;
            return Ok(());
        }
        complete_player_spell_cast(
            stream,
            deps,
            session,
            prepared_spell,
            spell_template,
            spell_profile,
            targets,
            now,
            header_crypto,
        )
        .await?;
    }
    Ok(())
}

pub(in crate::world) async fn handle_use_item(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    request: wow_proto::UseItemRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let request = wow_proto::UseItemRequest {
        bag: normalize_client_bag(request.bag),
        ..request
    };
    expire_session_auras(session, Instant::now());
    let Some(character) = session.character.active_character.as_ref() else {
        warn!("Ignoring item use before character login");
        return Ok(());
    };
    let character_guid = character.guid;
    let map_id = character.position.map_id;
    let caster = ObjectGuid::new(HighGuid::Player, 0, character_guid);

    let Some(source_item) = session
        .inventory
        .items
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
        && !(source_item.bag == INVENTORY_SLOT_BAG_0 as u32
            && source_item.slot < EQUIPMENT_SLOT_END)
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
        &session.character.character_skills,
        &session.character.active_spells,
        &session.character.character_reputations,
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
    let Some(spell_template) = deps
        .shared_world
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

    let Some(mut prepared_spell) =
        SpellInfo::from_template(&spell_template).prepare_item_cast(item_guid)
    else {
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
    prepared_spell.prepare();
    let item_spell_profile = prepared_spell.profile;
    let cast_time_ms = spell_cast_time_millis(
        deps.shared_world
            .maps
            .spell_cast_time(spell_template.casting_time_index),
    );

    let now = Instant::now();
    stand_player_for_spell_cast(stream, deps.shared_world, session, header_crypto).await?;
    let targets = normalize_item_use_targets(request.targets, &item_spell_profile, caster);
    let spell_info = SpellInfo::from_template(&spell_template);
    let item_value_context = player_spell_effect_value_context(
        deps.shared_world.maps,
        &spell_template,
        &session.character.character_skills,
        0,
    );
    let refreshable_consumable_regen = item_spell_profile.kind == SpellCastKind::AuraApplication
        && spell_periodic_regen_aura(&spell_info, item_value_context, now).is_some();
    if let Some(failure) = item_use_spell_failure(
        deps.shared_world.maps,
        map_id,
        character_guid,
        &item_spell_profile,
        now,
        refreshable_consumable_regen,
    )
    .await
    {
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
        deps.shared_world.maps,
        map_id,
        character_guid,
        source_item.item_template,
        &item_spell_profile,
        now,
        refreshable_consumable_regen,
        spell_template.category,
        spell_template.category_recovery_time as u64,
    )
    .await;
    let spell_start_body = prepared_spell.spell_start_body(caster, cast_time_ms, &targets)?;
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
    if cast_time_ms > 0 {
        deps.shared_world
            .maps
            .set_active_player_spell_cast(
                map_id,
                character_guid,
                ActivePlayerSpellCast {
                    spell_id: spell_template.id,
                    source: ActivePlayerSpellCastSource::Item {
                        item_guid,
                        source_item,
                        spell_charges: item_spell.spell_charges,
                    },
                    profile: item_spell_profile,
                    targets: PendingSpellCastTargets::from_spell_targets(&targets),
                    due_at: now + Duration::from_millis(cast_time_ms as u64),
                    cast_time_millis: cast_time_ms,
                    interrupt_flags: spell_template.interrupt_flags,
                    damage_pushback_count: 0,
                },
            )
            .await;
        return Ok(());
    }
    complete_item_use_spell_cast(
        stream,
        deps,
        session,
        caster,
        prepared_spell,
        spell_template,
        item_spell_profile,
        source_item,
        item_spell.spell_charges,
        targets,
        now,
        header_crypto,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn complete_item_use_spell_cast_by_id(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    spell_id: u32,
    item_guid: ObjectGuid,
    source_item: CharacterInventoryItem,
    spell_charges: i32,
    targets: SpellCastTargets,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let Some(spell_template) = deps
        .shared_world
        .object_mgr
        .spell_template(deps.world_db_pool, spell_id)
        .await?
    else {
        warn!(
            spell_id,
            "Dropping pending item spell cast with no spell_template row"
        );
        return Ok(());
    };
    let Some(mut prepared_spell) =
        SpellInfo::from_template(&spell_template).prepare_item_cast(item_guid)
    else {
        warn!(
            spell_id,
            spell_name = spell_template.spell_name.as_str(),
            "Dropping pending unsupported item spell cast"
        );
        return Ok(());
    };
    prepared_spell.start_casting();
    let item_spell_profile = prepared_spell.profile;
    let caster = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    complete_item_use_spell_cast(
        stream,
        deps,
        session,
        caster,
        prepared_spell,
        spell_template,
        item_spell_profile,
        source_item,
        spell_charges,
        targets,
        now,
        header_crypto,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn complete_item_use_spell_cast(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    mut prepared_spell: PreparedSpellCast,
    spell_template: wow_db::SpellTemplateQuery,
    item_spell_profile: SpellCastProfile,
    source_item: CharacterInventoryItem,
    spell_charges: i32,
    targets: SpellCastTargets,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let character_guid = character.guid;
    let character_level = character.level;
    let map_id = character.position.map_id;
    let target_outcome = player_db_creature_spell_target_outcome(
        deps.shared_world,
        session,
        character_guid,
        map_id,
        &spell_template,
        &item_spell_profile,
        &targets,
    )
    .await?;
    send_packet(
        stream,
        SMSG_CAST_RESULT,
        &build_cast_result_ok_body(spell_template.id),
        Some(&mut *header_crypto),
    )
    .await?;
    let spell_go_body =
        if let Some(miss_info) = target_outcome.and_then(|outcome| outcome.miss_info) {
            prepared_spell.spell_go_body_with_miss(caster, &targets, miss_info)?
        } else {
            prepared_spell.spell_go_body(caster, &targets)?
        };
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

    if target_outcome.is_some_and(|outcome| outcome.miss_info.is_some()) {
        begin_failed_hostile_db_creature_spell_retaliation(
            stream,
            deps.shared_world,
            session,
            caster,
            map_id,
            &spell_template,
            &item_spell_profile,
            &targets,
            header_crypto,
        )
        .await?;
    } else if target_outcome.is_some() {
        apply_player_spell_impact(
            stream,
            deps,
            session,
            caster,
            character_guid,
            character_level,
            map_id,
            &spell_template,
            &item_spell_profile,
            &targets,
            target_outcome,
            now,
            header_crypto,
        )
        .await?;
    } else {
        apply_item_use_spell_effects(
            stream,
            deps,
            session,
            caster,
            &spell_template,
            &item_spell_profile,
            now,
            header_crypto,
        )
        .await?;
    }
    if spell_charges < 0 {
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
    prepared_spell.finish();
    Ok(())
}

impl PendingSpellCastTargets {
    fn from_spell_targets(targets: &SpellCastTargets) -> Self {
        Self {
            target_mask: targets.target_mask,
            unit_target: targets.unit_target,
            gameobject_target: targets.gameobject_target,
            source_location: targets.source_location,
            destination: targets.destination,
        }
    }

    fn into_spell_targets(self) -> SpellCastTargets {
        SpellCastTargets {
            target_mask: self.target_mask,
            unit_target: self.unit_target,
            gameobject_target: self.gameobject_target,
            source_location: self.source_location,
            destination: self.destination,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn complete_pending_player_spell_cast(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let map_id = character.position.map_id;
    let character_guid = character.guid;
    let now = Instant::now();
    if let Some(active_cast) = deps
        .shared_world
        .maps
        .take_due_active_player_spell_cast(map_id, character_guid, now)
        .await
    {
        return match active_cast.source {
            ActivePlayerSpellCastSource::Player => {
                complete_player_spell_cast_by_id(
                    stream,
                    deps,
                    session,
                    active_cast.spell_id,
                    active_cast.targets.into_spell_targets(),
                    now,
                    header_crypto,
                )
                .await
            }
            ActivePlayerSpellCastSource::OpeningGameObject => {
                complete_opening_spell_cast(
                    stream,
                    deps.world_db_pool,
                    deps.shared_world,
                    session,
                    active_cast.spell_id,
                    active_cast.targets.into_spell_targets(),
                    header_crypto,
                )
                .await
            }
            ActivePlayerSpellCastSource::Item {
                item_guid,
                source_item,
                spell_charges,
            } => {
                complete_item_use_spell_cast_by_id(
                    stream,
                    deps,
                    session,
                    active_cast.spell_id,
                    item_guid,
                    source_item,
                    spell_charges,
                    active_cast.targets.into_spell_targets(),
                    now,
                    header_crypto,
                )
                .await
            }
        };
    }
    if let Some(event) = deps
        .shared_world
        .maps
        .take_due_pending_spell_event(map_id, character_guid, now)
        .await
    {
        return match event.kind {
            PendingSpellEventKind::Spell {
                targets,
                target_outcome,
            } => {
                apply_player_spell_impact_by_id(
                    stream,
                    deps,
                    session,
                    event.spell_id,
                    targets.into_spell_targets(),
                    target_outcome,
                    now,
                    header_crypto,
                )
                .await
            }
            PendingSpellEventKind::RangedAutoAttack {
                target,
                outcome,
                weapon_skill_id,
            } => {
                apply_player_ranged_auto_attack_impact(
                    stream,
                    CombatRewardDeps {
                        character_db_pool: deps.character_db_pool,
                        world_db_pool: deps.world_db_pool,
                        shared_world: deps.shared_world,
                        parties: deps.parties,
                    },
                    session,
                    header_crypto,
                    target,
                    event.spell_id,
                    outcome,
                    weapon_skill_id,
                    now,
                )
                .await
            }
        };
    }
    Ok(())
}

pub(in crate::world) async fn next_pending_player_spell_cast_due_at(
    maps: &MapRuntimeManager,
    session: &WorldSessionState,
) -> Option<Instant> {
    let character = session.character.active_character.as_ref()?;
    maps.next_pending_player_spell_cast_due_at(character.position.map_id, character.guid)
        .await
}

pub(in crate::world) async fn pending_player_spell_cast_is_due(
    maps: &MapRuntimeManager,
    session: &WorldSessionState,
    now: Instant,
) -> bool {
    next_pending_player_spell_cast_due_at(maps, session)
        .await
        .is_some_and(|due_at| now >= due_at)
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn complete_player_spell_cast_by_id(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    spell_id: u32,
    targets: SpellCastTargets,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(spell_template) = deps
        .shared_world
        .object_mgr
        .spell_template(deps.world_db_pool, spell_id)
        .await?
    else {
        warn!(
            spell_id,
            "Dropping pending spell cast with no spell_template row"
        );
        return Ok(());
    };
    let Some(mut prepared_spell) = SpellInfo::from_template(&spell_template).prepare_player_cast()
    else {
        warn!(
            spell_id,
            spell_name = spell_template.spell_name.as_str(),
            "Dropping pending unsupported spell effect shape"
        );
        return Ok(());
    };
    prepared_spell.start_casting();
    let spell_profile = prepared_spell.profile;
    complete_player_spell_cast(
        stream,
        deps,
        session,
        prepared_spell,
        spell_template,
        spell_profile,
        targets,
        now,
        header_crypto,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn complete_player_spell_cast(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    mut prepared_spell: PreparedSpellCast,
    spell_template: wow_db::SpellTemplateQuery,
    spell_profile: SpellCastProfile,
    targets: SpellCastTargets,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let character_guid = character.guid;
    let character_level = character.level;
    let map_id = character.position.map_id;
    let caster = ObjectGuid::new(HighGuid::Player, 0, character_guid);

    if let Some(failure) = spell_target_cast_failure(
        deps.shared_world,
        deps.world_db_pool,
        session,
        &spell_template,
        &spell_profile,
        &targets,
    )
    .await
    {
        return send_spell_cast_failure(
            stream,
            caster,
            prepared_spell.spell_id,
            failure,
            header_crypto,
        )
        .await;
    }
    if let Some(failure) = player_aura_rank_cast_failure(
        deps,
        session,
        &spell_template,
        &spell_profile,
        &targets,
        caster,
    )
    .await?
    {
        begin_failed_hostile_db_creature_spell_retaliation(
            stream,
            deps.shared_world,
            session,
            caster,
            map_id,
            &spell_template,
            &spell_profile,
            &targets,
            header_crypto,
        )
        .await?;
        return send_spell_cast_failure(
            stream,
            caster,
            prepared_spell.spell_id,
            failure,
            header_crypto,
        )
        .await;
    }
    if spell_profile.kind == SpellCastKind::CreateItem {
        if let Some(failure) =
            player_create_item_cast_inventory_failure(deps, session, &spell_template).await?
        {
            return send_inventory_change_failure(stream, failure, None, None, header_crypto).await;
        }
    }

    if let Err(failure) = deps
        .shared_world
        .maps
        .spend_player_spell_power(
            map_id,
            character_guid,
            &spell_profile,
            now,
            spell_blocks_mana_regen(&spell_template),
        )
        .await
    {
        return send_spell_cast_failure(
            stream,
            caster,
            prepared_spell.spell_id,
            failure,
            header_crypto,
        )
        .await;
    }
    sync_session_player_power_from_map(deps.shared_world.maps, session, map_id, character_guid)
        .await;
    send_player_spell_power_update(stream, caster, &spell_profile, session, header_crypto).await?;
    let target_outcome = player_db_creature_spell_target_outcome(
        deps.shared_world,
        session,
        character_guid,
        map_id,
        &spell_template,
        &spell_profile,
        &targets,
    )
    .await?;
    send_packet(
        stream,
        SMSG_CAST_RESULT,
        &build_cast_result_ok_body(prepared_spell.spell_id),
        Some(&mut *header_crypto),
    )
    .await?;
    let spell_go_body =
        if let Some(miss_info) = target_outcome.and_then(|outcome| outcome.miss_info) {
            prepared_spell.spell_go_body_with_miss(caster, &targets, miss_info)?
        } else {
            prepared_spell.spell_go_body(caster, &targets)?
        };
    send_packet(
        stream,
        SMSG_SPELL_GO,
        &spell_go_body,
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
                opcode: SMSG_SPELL_GO,
                body: spell_go_body,
            },
        )
        .await;
    deps.shared_world.sessions.dispatch(observer_packets).await;
    retime_player_auto_attack_after_spell_cast(
        deps,
        session,
        &spell_template,
        &spell_profile,
        map_id,
        character_guid,
        now,
    )
    .await?;
    if target_outcome.is_some_and(|outcome| outcome.miss_info.is_some()) {
        begin_failed_hostile_db_creature_spell_retaliation(
            stream,
            deps.shared_world,
            session,
            caster,
            map_id,
            &spell_template,
            &spell_profile,
            &targets,
            header_crypto,
        )
        .await?;
        prepared_spell.finish();
        return Ok(());
    }
    let travel_delay =
        spell_travel_delay_millis(deps.shared_world, session, &spell_template, &targets).await;
    if travel_delay > 0 {
        deps.shared_world
            .maps
            .push_pending_spell_event(
                map_id,
                character_guid,
                prepared_spell.spell_id,
                PendingSpellCastTargets::from_spell_targets(&targets),
                target_outcome,
                now + Duration::from_millis(travel_delay as u64),
            )
            .await;
        prepared_spell.finish();
        return Ok(());
    }
    let result = apply_player_spell_impact(
        stream,
        deps,
        session,
        caster,
        character_guid,
        character_level,
        map_id,
        &spell_template,
        &spell_profile,
        &targets,
        target_outcome,
        now,
        header_crypto,
    )
    .await;
    prepared_spell.finish();
    result
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_spell_impact_by_id(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    spell_id: u32,
    targets: SpellCastTargets,
    target_outcome: Option<PlayerSpellTargetOutcome>,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let Some(spell_template) = deps
        .shared_world
        .object_mgr
        .spell_template(deps.world_db_pool, spell_id)
        .await?
    else {
        warn!(
            spell_id,
            "Dropping pending spell impact with no spell_template row"
        );
        return Ok(());
    };
    let Some(prepared_spell) = SpellInfo::from_template(&spell_template).prepare_player_cast()
    else {
        warn!(
            spell_id,
            spell_name = spell_template.spell_name.as_str(),
            "Dropping pending unsupported spell impact"
        );
        return Ok(());
    };
    let spell_profile = prepared_spell.profile;
    let caster = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    apply_player_spell_impact(
        stream,
        deps,
        session,
        caster,
        character.guid,
        character.level,
        character.position.map_id,
        &spell_template,
        &spell_profile,
        &targets,
        target_outcome,
        now,
        header_crypto,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_spell_impact(
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
    target_outcome: Option<PlayerSpellTargetOutcome>,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    apply_player_spell_effects(
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
        target_outcome,
        now,
        header_crypto,
    )
    .await
}

pub(in crate::world) fn spell_resets_auto_attack_timers_on_cast(
    spell_template: &wow_db::SpellTemplateQuery,
    spell_profile: &SpellCastProfile,
) -> bool {
    spell_template.interrupt_flags & SPELL_INTERRUPT_FLAG_COMBAT != 0
        && !matches!(
            spell_profile.kind,
            SpellCastKind::AutoRepeatRanged | SpellCastKind::NextMeleeSwing
        )
}

pub(in crate::world) fn auto_repeat_spell_cancels_when_casting(
    spell_template: &wow_db::SpellTemplateQuery,
) -> bool {
    spell_template.attributes_ex3 & SPELL_ATTR_EX3_CASTING_CANCELS_AUTOREPEAT != 0
}

async fn retime_player_auto_attack_after_spell_cast(
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    spell_template: &wow_db::SpellTemplateQuery,
    spell_profile: &SpellCastProfile,
    map_id: u32,
    character_guid: u32,
    now: Instant,
) -> anyhow::Result<()> {
    if !spell_resets_auto_attack_timers_on_cast(spell_template, spell_profile) {
        return Ok(());
    }

    let Some(snapshot) = deps
        .shared_world
        .maps
        .player_runtime_snapshot(map_id, character_guid)
        .await
    else {
        return Ok(());
    };

    let cancel_ranged_auto_repeat = match snapshot.active_combat_attack_kind {
        PlayerAutoAttackKind::Ranged { spell_id, .. } => deps
            .shared_world
            .object_mgr
            .spell_template(deps.world_db_pool, spell_id)
            .await?
            .as_ref()
            .is_some_and(auto_repeat_spell_cancels_when_casting),
        PlayerAutoAttackKind::Melee => false,
    };

    let melee_delay =
        player_auto_attack_swing_delay(deps.shared_world, map_id, character_guid).await;
    let adjusted = deps
        .shared_world
        .maps
        .retime_player_auto_attack_after_spell_cast(
            map_id,
            character_guid,
            now,
            melee_delay,
            Duration::from_millis(PLAYER_RANGED_AUTO_ATTACK_WINDUP_MILLIS),
            cancel_ranged_auto_repeat,
        )
        .await;
    match adjusted {
        PlayerAutoAttackAfterSpellCast::None => {}
        PlayerAutoAttackAfterSpellCast::MeleeRetimed { next_swing_at, .. }
        | PlayerAutoAttackAfterSpellCast::RangedRetimed {
            next_shot_at: next_swing_at,
            ..
        } => {
            mirror_session_player_next_swing_at(session, Some(next_swing_at));
        }
        PlayerAutoAttackAfterSpellCast::RangedCanceled { .. } => {
            mirror_session_player_auto_attack(session, None, None);
        }
    }
    Ok(())
}

pub(in crate::world) async fn stand_player_for_spell_cast(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let has_standing_cancel_aura =
        session.auras.active_auras.iter().any(|aura| {
            active_aura_interrupt_flags(aura) & AURA_INTERRUPT_FLAG_STANDING_CANCELS != 0
        });
    if session.character.player_stand_state == PLAYER_STAND_STATE_STAND && !has_standing_cancel_aura
    {
        return Ok(());
    }
    if has_standing_cancel_aura {
        interrupt_player_consumable_auras(
            stream,
            shared_world.maps,
            shared_world.sessions,
            session,
            AURA_INTERRUPT_FLAG_STANDING_CANCELS,
            header_crypto,
        )
        .await?;
    }
    session.character.player_stand_state = PLAYER_STAND_STATE_STAND;
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_stand_state_update_body(character, PLAYER_STAND_STATE_STAND)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_STANDSTATE_UPDATE,
        &[PLAYER_STAND_STATE_STAND],
        Some(&mut *header_crypto),
    )
    .await?;
    let packets = shared_world
        .maps
        .set_player_stand_state(
            character.position.map_id,
            character.guid,
            PLAYER_STAND_STATE_STAND,
        )
        .await?;
    shared_world.sessions.dispatch(packets).await;
    Ok(())
}

pub(in crate::world) async fn cancel_pending_player_spell_cast(
    stream: &mut WorldPacketSink,
    maps: &MapRuntimeManager,
    sessions: &SessionRegistry,
    session: &mut WorldSessionState,
    failure: u8,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(false);
    };
    let map_id = character.position.map_id;
    let character_guid = character.guid;
    let Some(active_cast) = maps
        .cancel_active_player_spell_cast(map_id, character_guid)
        .await
    else {
        let Some(channel_event) = maps
            .cancel_active_player_channel(map_id, character_guid)
            .await?
        else {
            return Ok(false);
        };
        for packet in channel_event.direct_packets {
            send_packet(
                stream,
                packet.opcode,
                &packet.body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
        sessions.dispatch(channel_event.observer_packets).await;
        return Ok(true);
    };
    maps.clear_player_spell_recovery(map_id, character_guid, &active_cast.profile)
        .await;
    let caster = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    send_spell_cast_failure(stream, caster, active_cast.spell_id, failure, header_crypto).await?;
    broadcast_spell_interrupt_to_observers(
        maps,
        sessions,
        session,
        caster,
        active_cast.spell_id,
        failure,
    )
    .await?;
    Ok(true)
}

pub(in crate::world) async fn cancel_movement_interrupted_player_spell_cast(
    stream: &mut WorldPacketSink,
    maps: &MapRuntimeManager,
    sessions: &SessionRegistry,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(false);
    };
    let map_id = character.position.map_id;
    let character_guid = character.guid;
    let Some(active_cast) = maps
        .cancel_movement_interrupted_player_spell_cast(map_id, character_guid)
        .await
    else {
        let Some(channel_event) = maps
            .cancel_movement_interrupted_player_channel(map_id, character_guid)
            .await?
        else {
            return Ok(false);
        };
        for packet in channel_event.direct_packets {
            send_packet(
                stream,
                packet.opcode,
                &packet.body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
        sessions.dispatch(channel_event.observer_packets).await;
        return Ok(true);
    };
    maps.clear_player_spell_recovery(map_id, character_guid, &active_cast.profile)
        .await;
    let caster = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    send_spell_cast_failure(
        stream,
        caster,
        active_cast.spell_id,
        SPELL_FAILED_INTERRUPTED,
        header_crypto,
    )
    .await?;
    broadcast_spell_interrupt_to_observers(
        maps,
        sessions,
        session,
        caster,
        active_cast.spell_id,
        SPELL_FAILED_INTERRUPTED,
    )
    .await?;
    Ok(true)
}

pub(in crate::world) async fn interrupt_player_spell_cast_for_damage(
    stream: &mut WorldPacketSink,
    maps: &MapRuntimeManager,
    sessions: &SessionRegistry,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(false);
    };
    let map_id = character.position.map_id;
    let character_guid = character.guid;
    let Some(active_cast) = maps
        .cancel_active_player_spell_cast_for_damage(map_id, character_guid)
        .await
    else {
        return Ok(false);
    };
    maps.clear_player_spell_recovery(map_id, character_guid, &active_cast.profile)
        .await;
    let caster = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    send_spell_cast_failure(
        stream,
        caster,
        active_cast.spell_id,
        SPELL_FAILED_INTERRUPTED,
        header_crypto,
    )
    .await?;
    broadcast_spell_interrupt_to_observers(
        maps,
        sessions,
        session,
        caster,
        active_cast.spell_id,
        SPELL_FAILED_INTERRUPTED,
    )
    .await?;
    Ok(true)
}

pub(in crate::world) async fn cancel_pending_opening_spell_cast(
    stream: &mut WorldPacketSink,
    maps: &MapRuntimeManager,
    sessions: &SessionRegistry,
    session: &mut WorldSessionState,
    failure: u8,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(false);
    };
    let map_id = character.position.map_id;
    let character_guid = character.guid;
    let Some(active_cast) = maps
        .cancel_active_player_opening_spell_cast(map_id, character_guid)
        .await
    else {
        return Ok(false);
    };
    let caster = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    send_spell_cast_failure(stream, caster, active_cast.spell_id, failure, header_crypto).await?;
    broadcast_spell_interrupt_to_observers(
        maps,
        sessions,
        session,
        caster,
        active_cast.spell_id,
        failure,
    )
    .await?;
    Ok(true)
}

pub(in crate::world) async fn broadcast_spell_interrupt_to_observers(
    maps: &MapRuntimeManager,
    sessions: &SessionRegistry,
    session: &WorldSessionState,
    caster: ObjectGuid,
    spell_id: u32,
    failure: u8,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let failure_packet = OutboundWorldPacket {
        opcode: SMSG_SPELL_FAILURE,
        body: build_spell_failure_body(caster, spell_id, failure)?,
    };
    let failed_other_packet = OutboundWorldPacket {
        opcode: SMSG_SPELL_FAILED_OTHER,
        body: build_spell_failed_other_body(caster, spell_id),
    };
    let mut observer_packets = maps
        .broadcast_nearby_player_packet(
            character.position.map_id,
            character.guid,
            PLAYER_VISIBILITY_RADIUS_YARDS,
            failure_packet,
        )
        .await;
    observer_packets.extend(
        maps.broadcast_nearby_player_packet(
            character.position.map_id,
            character.guid,
            PLAYER_VISIBILITY_RADIUS_YARDS,
            failed_other_packet,
        )
        .await,
    );
    sessions.dispatch(observer_packets).await;
    Ok(())
}

pub(in crate::world) async fn send_spell_cast_failure(
    stream: &mut WorldPacketSink,
    caster: ObjectGuid,
    spell_id: u32,
    failure: u8,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    send_packet(
        stream,
        SMSG_CAST_RESULT,
        &build_cast_result_failure_body(spell_id, failure),
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_SPELL_FAILURE,
        &build_spell_failure_body(caster, spell_id, failure)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_SPELL_FAILED_OTHER,
        &build_spell_failed_other_body(caster, spell_id),
        Some(header_crypto),
    )
    .await
}

#[derive(Clone, Copy)]
pub(in crate::world) struct SpellCastDeps<'a> {
    pub(in crate::world) character_db_pool: &'a MySqlPool,
    pub(in crate::world) world_db_pool: &'a MySqlPool,
    pub(in crate::world) account_id: u32,
    pub(in crate::world) shared_world: SharedWorldDeps<'a>,
    pub(in crate::world) parties: &'a PartyManager,
}

pub(in crate::world) struct OpeningSpellRequest {
    pub(in crate::world) spell_id: u32,
    pub(in crate::world) caster: ObjectGuid,
    pub(in crate::world) map_id: u32,
    pub(in crate::world) character_guid: u32,
    pub(in crate::world) targets: SpellCastTargets,
}

pub(in crate::world) async fn handle_opening_spell(
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
    let Some(character) = session.character.active_character.as_ref() else {
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
    if gameobject_chest_loot_is_exclusively_quest_drops(
        shared_world.object_mgr,
        world_db_pool,
        &gameobject.spawn.template,
    )
    .await?
        && select_db_gameobject_loot_item_for_character(
            shared_world.object_mgr,
            world_db_pool,
            session,
            &gameobject.spawn.template,
        )
        .await?
        .is_empty()
    {
        return Ok(());
    }
    targets.target_mask |= SPELL_CAST_TARGET_GAMEOBJECT;
    targets.gameobject_target = Some(gameobject_guid);

    let spell_start_body = build_spell_start_body(
        request.caster,
        request.spell_id,
        OPENING_SPELL_CAST_TIME_MS,
        &targets,
    )?;
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

    shared_world
        .maps
        .set_active_player_spell_cast(
            request.map_id,
            request.character_guid,
            ActivePlayerSpellCast {
                spell_id: request.spell_id,
                source: ActivePlayerSpellCastSource::OpeningGameObject,
                profile: opening_spell_cast_profile(request.spell_id),
                targets: PendingSpellCastTargets::from_spell_targets(&targets),
                due_at: Instant::now() + Duration::from_millis(OPENING_SPELL_CAST_TIME_MS as u64),
                cast_time_millis: OPENING_SPELL_CAST_TIME_MS,
                interrupt_flags: 0,
                damage_pushback_count: 0,
            },
        )
        .await;

    Ok(())
}

pub(in crate::world) async fn complete_opening_spell_cast(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    spell_id: u32,
    mut targets: SpellCastTargets,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let character_guid = character.guid;
    let map_id = character.position.map_id;
    let caster = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let Some(gameobject_guid) = targets
        .gameobject_target
        .or(targets.unit_target)
        .filter(|guid| guid.is_game_object())
    else {
        warn!("Ignoring completed Opening spell without gameobject target");
        return Ok(());
    };
    let Some(gameobject) = shared_world
        .maps
        .db_gameobject_snapshot(map_id, gameobject_guid)
        .await
    else {
        warn!(
            target = format_args!("0x{:016X}", gameobject_guid.raw()),
            "Ignoring completed Opening spell for unknown gameobject"
        );
        return Ok(());
    };
    if gameobject.spawn.map != map_id
        || !is_position_inside_radius(gameobject.position(), character.position, 8.0)
    {
        warn!("Ignoring completed Opening spell outside gameobject interaction range");
        return Ok(());
    }
    if !gameobject_chest_has_loot_id(&gameobject.spawn.template) {
        warn!("Ignoring completed Opening spell for gameobject without chest loot");
        return Ok(());
    }
    targets.target_mask |= SPELL_CAST_TARGET_GAMEOBJECT;
    targets.gameobject_target = Some(gameobject_guid);

    send_packet(
        stream,
        SMSG_CAST_RESULT,
        &build_cast_result_ok_body(spell_id),
        Some(&mut *header_crypto),
    )
    .await?;
    let spell_go_body = build_spell_go_body(caster, spell_id, &targets)?;
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
            map_id,
            character_guid,
            PLAYER_VISIBILITY_RADIUS_YARDS,
            OutboundWorldPacket {
                opcode: SMSG_SPELL_GO,
                body: spell_go_body,
            },
        )
        .await;
    shared_world.sessions.dispatch(observer_go).await;

    let loot_items = select_db_gameobject_loot_item_for_character(
        shared_world.object_mgr,
        world_db_pool,
        session,
        &gameobject.spawn.template,
    )
    .await?;
    if loot_items.is_empty()
        && gameobject_chest_loot_is_exclusively_quest_drops(
            shared_world.object_mgr,
            world_db_pool,
            &gameobject.spawn.template,
        )
        .await?
    {
        return Ok(());
    }
    let Some((gameobject, loot_items)) = shared_world
        .maps
        .open_db_gameobject_loot(map_id, gameobject_guid.raw(), character_guid, loot_items)
        .await
    else {
        warn!("Ignoring completed Opening spell for unavailable gameobject loot");
        return Ok(());
    };
    let _ = gameobject;

    send_player_looting_state_update(stream, shared_world, session, true, &mut *header_crypto)
        .await?;
    let response = build_gameobject_loot_response_body(gameobject_guid, &loot_items);
    send_packet(stream, SMSG_LOOT_RESPONSE, &response, Some(header_crypto)).await
}

pub(in crate::world) fn opening_spell_cast_profile(spell_id: u32) -> SpellCastProfile {
    SpellCastProfile {
        spell_id,
        kind: SpellCastKind::OpeningGameObject,
        aura_target: SpellAuraTarget::Caster,
        bonus_damage: 0,
        weapon_damage_percent: 0,
        damage: 0,
        power: SpellPowerCost::Mana { cost: 0 },
        requires_melee: false,
        requires_behind: false,
        needs_combo_points: false,
        global_cooldown_category: 0,
        global_cooldown_millis: 0,
        cooldown_millis: 0,
    }
}

pub(in crate::world) async fn spell_melee_cast_failure(
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    spell_profile: &SpellCastProfile,
    targets: &SpellCastTargets,
) -> Option<u8> {
    if !spell_profile.requires_melee {
        return None;
    }
    if spell_profile.kind == SpellCastKind::NextMeleeSwing {
        return None;
    }
    let target = targets.unit_target?;
    match db_creature_player_melee_check_from_map(shared_world, session, target).await {
        PlayerMeleeCheck::Clear => {
            if spell_profile.requires_behind
                && !spell_target_is_behind_victim(shared_world, session, target).await
            {
                Some(SPELL_FAILED_NOT_BEHIND)
            } else {
                None
            }
        }
        PlayerMeleeCheck::BadFacing => Some(SPELL_FAILED_UNIT_NOT_INFRONT),
        PlayerMeleeCheck::NavigationBlocked(DbCreatureNavigationResult::LineOfSightBlocked) => {
            Some(SPELL_FAILED_LINE_OF_SIGHT)
        }
        _ => Some(SPELL_FAILED_OUT_OF_RANGE),
    }
}

pub(in crate::world) async fn spell_target_is_behind_victim(
    shared_world: SharedWorldDeps<'_>,
    session: &WorldSessionState,
    target: ObjectGuid,
) -> bool {
    let Some(character) = session.character.active_character.as_ref() else {
        return false;
    };
    let Some(player) = shared_world
        .maps
        .player_runtime_snapshot(character.position.map_id, character.guid)
        .await
    else {
        return false;
    };
    let Some(creature) = shared_world
        .maps
        .db_creature_snapshot(character.position.map_id, target)
        .await
    else {
        return false;
    };
    !has_in_arc(
        creature.current_position,
        player.position,
        std::f32::consts::PI,
    )
}

pub(in crate::world) async fn spell_charge_cast_failure(
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    targets: &SpellCastTargets,
) -> Option<u8> {
    let target = targets.unit_target?;
    let Some(character) = session.character.active_character.as_ref() else {
        return Some(SPELL_FAILED_OUT_OF_RANGE);
    };
    let validation = shared_world
        .maps
        .validate_player_charge_against_db_creature(
            character.position.map_id,
            character.guid,
            target,
            &session.movement.db_creature_navigation,
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
pub(in crate::world) async fn apply_charge_movement(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    target: ObjectGuid,
    spell_speed: f32,
    spell_id: u32,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
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
    let move_body = build_monster_move_facing_target_body(
        caster,
        start,
        destination,
        spell_id,
        duration_ms,
        target,
    )?;

    if let Some(character) = session.character.active_character.as_mut() {
        character.position = destination;
    }
    let environment_packets = shared_world
        .maps
        .set_player_position(map_id, character_guid, destination)
        .await?;

    send_packet(
        stream,
        SMSG_MONSTER_MOVE,
        &move_body,
        Some(&mut *header_crypto),
    )
    .await?;
    shared_world.sessions.dispatch(environment_packets).await;
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

pub(in crate::world) fn charge_destination(
    start: WorldPosition,
    target: &DbCreatureRuntime,
) -> WorldPosition {
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

pub(in crate::world) fn charge_duration_millis(
    start: WorldPosition,
    destination: WorldPosition,
    speed: f32,
) -> u32 {
    let dx = destination.x - start.x;
    let dy = destination.y - start.y;
    let dz = destination.z - start.z;
    (((dx * dx + dy * dy + dz * dz).sqrt() / speed.max(f32::EPSILON)) * 1000.0)
        .round()
        .max(1.0) as u32
}

pub(in crate::world) fn angle_towards(from: WorldPosition, to: WorldPosition) -> f32 {
    (to.y - from.y).atan2(to.x - from.x)
}

pub(in crate::world) async fn spell_cast_failure(
    shared_world: SharedWorldDeps<'_>,
    world_db_pool: &MySqlPool,
    session: &mut WorldSessionState,
    spell_template: &wow_db::SpellTemplateQuery,
    spell_profile: &SpellCastProfile,
    targets: &SpellCastTargets,
    now: Instant,
) -> Option<u8> {
    if session.death.player_death_state != PlayerDeathState::Alive {
        return Some(SPELL_FAILED_CASTER_DEAD);
    }
    if let Some(character) = session.character.active_character.as_ref() {
        if session.character.player_health == 0
            && shared_world
                .maps
                .player_runtime_snapshot(character.position.map_id, character.guid)
                .await
                .is_some_and(|snapshot| snapshot.health == 0)
        {
            return Some(SPELL_FAILED_CASTER_DEAD);
        }
        if let Some(failure) = shared_world
            .maps
            .player_spell_cast_failure(
                character.position.map_id,
                character.guid,
                spell_profile,
                now,
            )
            .await
        {
            return Some(failure);
        }
    }
    spell_target_cast_failure(
        shared_world,
        world_db_pool,
        session,
        spell_template,
        spell_profile,
        targets,
    )
    .await
}

pub(in crate::world) async fn spell_target_cast_failure(
    shared_world: SharedWorldDeps<'_>,
    world_db_pool: &MySqlPool,
    session: &mut WorldSessionState,
    spell_template: &wow_db::SpellTemplateQuery,
    spell_profile: &SpellCastProfile,
    targets: &SpellCastTargets,
) -> Option<u8> {
    if spell_profile.kind == SpellCastKind::Charge {
        return spell_charge_cast_failure(shared_world, session, targets).await;
    }
    if spell_profile.kind == SpellCastKind::DirectHeal {
        return spell_heal_cast_failure(shared_world, session, spell_template, targets).await;
    }
    if let Some(failure) = spell_unit_target_cast_failure(
        shared_world,
        world_db_pool,
        session,
        spell_template,
        spell_profile,
        targets,
    )
    .await
    {
        return Some(failure);
    }
    if let Some(failure) =
        spell_combo_point_cast_failure(shared_world, session, spell_profile, targets).await
    {
        return Some(failure);
    }
    spell_melee_cast_failure(shared_world, session, spell_profile, targets).await
}

pub(in crate::world) async fn player_aura_rank_cast_failure(
    deps: SpellCastDeps<'_>,
    session: &WorldSessionState,
    spell_template: &wow_db::SpellTemplateQuery,
    spell_profile: &SpellCastProfile,
    targets: &SpellCastTargets,
    caster: ObjectGuid,
) -> anyhow::Result<Option<u8>> {
    if !spell_has_aura_application(spell_template)
        || !matches!(
            spell_profile.kind,
            SpellCastKind::AuraApplication | SpellCastKind::DirectHeal
        )
        || spell_has_direct_damage_application(spell_template)
    {
        return Ok(None);
    }
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
            Ok(resolution.failure)
        }
        SpellAuraTarget::UnitTarget => {
            let Some(character) = session.character.active_character.as_ref() else {
                return Ok(None);
            };
            let Some(target) = targets.unit_target else {
                return Ok(None);
            };
            let active_auras = if target.is_player() {
                if target.counter() == character.guid {
                    session.auras.active_auras.clone()
                } else {
                    let Some(snapshot) = deps
                        .shared_world
                        .maps
                        .player_runtime_snapshot(character.position.map_id, target.counter())
                        .await
                    else {
                        return Ok(None);
                    };
                    snapshot.active_auras
                }
            } else if target.is_creature() {
                let Some(creature) = deps
                    .shared_world
                    .maps
                    .db_creature_snapshot(character.position.map_id, target)
                    .await
                else {
                    return Ok(None);
                };
                creature.active_auras
            } else {
                return Ok(None);
            };
            let resolution = aura_rank_conflict_resolution(
                deps.shared_world.object_mgr,
                deps.world_db_pool,
                spell_template.id,
                caster,
                &active_auras,
            )
            .await?;
            Ok(resolution.failure)
        }
        SpellAuraTarget::CasterAreaEnemy | SpellAuraTarget::DestinationAreaEnemy => Ok(None),
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn begin_failed_hostile_db_creature_spell_retaliation(
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
    let has_hostile_unit_aura = matches!(spell_profile.aura_target, SpellAuraTarget::UnitTarget)
        && spell_template_has_hostile_unit_aura(spell_template);
    if !has_hostile_unit_aura && !spell_template_has_hostile_unit_school_damage(spell_template) {
        return Ok(());
    }
    let Some(target) = targets.unit_target.filter(|target| target.is_creature()) else {
        return Ok(());
    };
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
pub(in crate::world) async fn player_db_creature_spell_target_outcome(
    shared_world: SharedWorldDeps<'_>,
    session: &WorldSessionState,
    character_guid: u32,
    map_id: u32,
    spell_template: &wow_db::SpellTemplateQuery,
    spell_profile: &SpellCastProfile,
    targets: &SpellCastTargets,
) -> anyhow::Result<Option<PlayerSpellTargetOutcome>> {
    if !spell_uses_db_creature_unit_target_outcome(spell_template, spell_profile) {
        return Ok(None);
    }
    let Some(target) = targets.unit_target.filter(|target| target.is_creature()) else {
        return Ok(None);
    };
    let Some(target_creature) = shared_world.maps.db_creature_snapshot(map_id, target).await else {
        return Ok(None);
    };
    let combat_stats = shared_world
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
    let (school, dmg_class) = spell_target_outcome_school_and_damage_class(spell_template);
    let outcome = roll_spell_damage_outcome(spell_damage_outcome_input(
        1,
        school,
        dmg_class,
        spell_template.attributes_ex2,
        spell_template.attributes_ex3,
        player_spell_snapshot(
            character.map(|character| character.level).unwrap_or(1),
            character.map(|character| character.class).unwrap_or(1),
            &combat_stats,
        ),
        db_creature_spell_snapshot(&target_creature),
    ));
    Ok(Some(PlayerSpellTargetOutcome {
        target,
        miss_info: outcome.miss_info,
    }))
}

fn spell_uses_db_creature_unit_target_outcome(
    spell_template: &wow_db::SpellTemplateQuery,
    spell_profile: &SpellCastProfile,
) -> bool {
    let has_hostile_unit_school_damage =
        spell_template_has_hostile_unit_school_damage(spell_template);
    let has_hostile_unit_aura = matches!(spell_profile.aura_target, SpellAuraTarget::UnitTarget)
        && spell_template_has_hostile_unit_aura(spell_template);
    matches!(
        spell_profile.kind,
        SpellCastKind::InstantDamage | SpellCastKind::AuraApplication
    ) && (has_hostile_unit_school_damage || has_hostile_unit_aura)
}

fn spell_template_has_hostile_unit_school_damage(
    spell_template: &wow_db::SpellTemplateQuery,
) -> bool {
    SpellInfo::from_template(spell_template)
        .effects
        .iter()
        .any(|effect| {
            effect.dispatch == SpellEffectDispatch::SchoolDamage
                && [effect.implicit_target_a, effect.implicit_target_b]
                    .into_iter()
                    .any(|target| target == TARGET_UNIT_ENEMY)
        })
}

fn spell_target_outcome_school_and_damage_class(
    spell_template: &wow_db::SpellTemplateQuery,
) -> (u8, u32) {
    let school = spell_template.school as u8;
    let dmg_class = if spell_template.dmg_class == SPELL_DAMAGE_CLASS_NONE
        && is_resistable_spell_school(school)
    {
        SPELL_DAMAGE_CLASS_MAGIC
    } else {
        spell_template.dmg_class
    };
    (school, dmg_class)
}

fn spell_template_has_hostile_unit_aura(spell_template: &wow_db::SpellTemplateQuery) -> bool {
    SpellInfo::from_template(spell_template)
        .effects
        .iter()
        .any(|effect| {
            matches!(effect.dispatch, SpellEffectDispatch::ApplyAura)
                && [effect.implicit_target_a, effect.implicit_target_b]
                    .into_iter()
                    .any(|target| target == TARGET_UNIT_ENEMY)
        })
}

pub(in crate::world) async fn spell_combo_point_cast_failure(
    shared_world: SharedWorldDeps<'_>,
    session: &WorldSessionState,
    spell_profile: &SpellCastProfile,
    targets: &SpellCastTargets,
) -> Option<u8> {
    if !spell_profile.needs_combo_points {
        return None;
    }
    let target = targets.unit_target?;
    let character = session.character.active_character.as_ref()?;
    let snapshot = shared_world
        .maps
        .player_runtime_snapshot(character.position.map_id, character.guid)
        .await?;
    if snapshot.combo_points == 0 || snapshot.combo_target != Some(target) {
        Some(SPELL_FAILED_NO_COMBO_POINTS)
    } else {
        None
    }
}

pub(in crate::world) async fn spell_heal_cast_failure(
    shared_world: SharedWorldDeps<'_>,
    session: &WorldSessionState,
    spell_template: &wow_db::SpellTemplateQuery,
    targets: &SpellCastTargets,
) -> Option<u8> {
    let character = session.character.active_character.as_ref()?;
    if SpellInfo::from_template(spell_template).unit_target_kind(SpellCastKind::DirectHeal)
        == SpellTargetKind::Caster
    {
        return None;
    }
    let target = targets.unit_target?;
    if !target.is_player() {
        return Some(SPELL_FAILED_OUT_OF_RANGE);
    }
    let target_guid = target.counter();
    let Some(snapshot) = shared_world
        .maps
        .player_runtime_snapshot(character.position.map_id, target_guid)
        .await
    else {
        return Some(SPELL_FAILED_OUT_OF_RANGE);
    };
    if snapshot.health == 0 {
        return Some(SPELL_FAILED_OUT_OF_RANGE);
    }
    None
}

pub(in crate::world) async fn spell_unit_target_cast_failure(
    shared_world: SharedWorldDeps<'_>,
    world_db_pool: &MySqlPool,
    session: &WorldSessionState,
    spell_template: &wow_db::SpellTemplateQuery,
    spell_profile: &SpellCastProfile,
    targets: &SpellCastTargets,
) -> Option<u8> {
    let target_kind = SpellInfo::from_template(spell_template).unit_target_kind(spell_profile.kind);
    if target_kind.requires_unit_target() && targets.unit_target.is_none() {
        return Some(SPELL_FAILED_BAD_IMPLICIT_TARGETS);
    }
    if target_kind == SpellTargetKind::FriendlyUnit {
        let character = session.character.active_character.as_ref()?;
        let target = targets.unit_target?;
        if !target.is_player() {
            return Some(SPELL_FAILED_OUT_OF_RANGE);
        }
        let Some(snapshot) = shared_world
            .maps
            .player_runtime_snapshot(character.position.map_id, target.counter())
            .await
        else {
            return Some(SPELL_FAILED_OUT_OF_RANGE);
        };
        return (snapshot.health == 0).then_some(SPELL_FAILED_OUT_OF_RANGE);
    }
    if target_kind != SpellTargetKind::HostileUnit
        || spell_profile.requires_melee
        || matches!(
            spell_profile.kind,
            SpellCastKind::NextMeleeSwing | SpellCastKind::Charge
        )
    {
        return None;
    }
    let character = session.character.active_character.as_ref()?;
    let target = targets.unit_target?;
    if !target.is_creature() {
        return Some(SPELL_FAILED_BAD_TARGETS);
    }
    let range = if spell_template.range_index == 0 {
        None
    } else {
        let range = shared_world.maps.spell_range(spell_template.range_index);
        if range.is_none() {
            return Some(SPELL_FAILED_OUT_OF_RANGE);
        }
        range
    };
    let validation = shared_world
        .maps
        .validate_player_spell_against_db_creature(
            character.position.map_id,
            character.guid,
            target,
            &session.movement.db_creature_navigation,
            range,
            spell_requires_infront_target(
                shared_world.object_mgr,
                world_db_pool,
                spell_template.id,
            )
            .await
            .unwrap_or(false),
        )
        .await;
    match validation.check {
        PlayerSpellTargetCheck::Clear => None,
        PlayerSpellTargetCheck::BadFacing => Some(SPELL_FAILED_UNIT_NOT_INFRONT),
        PlayerSpellTargetCheck::NavigationBlocked(
            DbCreatureNavigationResult::LineOfSightBlocked,
        ) => Some(SPELL_FAILED_LINE_OF_SIGHT),
        PlayerSpellTargetCheck::TooClose => Some(SPELL_FAILED_TOO_CLOSE),
        PlayerSpellTargetCheck::NoActiveCharacter
        | PlayerSpellTargetCheck::MissingTarget
        | PlayerSpellTargetCheck::TargetNotAlive
        | PlayerSpellTargetCheck::NavigationBlocked(_)
        | PlayerSpellTargetCheck::OutOfRange => Some(SPELL_FAILED_OUT_OF_RANGE),
    }
}

pub(in crate::world) async fn spell_requires_infront_target(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    spell_id: u32,
) -> anyhow::Result<bool> {
    Ok(object_mgr
        .spell_facing_flag(world_db_pool, spell_id)
        .await?
        & SPELL_FACING_FLAG_INFRONT
        != 0)
}

pub(in crate::world) async fn resolve_player_spell_cast_targets(
    maps: &MapRuntimeManager,
    map_id: u32,
    character_guid: u32,
    mut targets: SpellCastTargets,
    spell_info: &SpellInfo<'_>,
    kind: SpellCastKind,
) -> SpellCastTargets {
    let target_kind = spell_info.unit_target_kind(kind);
    if target_kind.requires_unit_target() && targets.unit_target.is_none() {
        if let Some(selected_target) = maps.player_selected_target(map_id, character_guid).await {
            targets.target_mask =
                (targets.target_mask | SPELL_CAST_TARGET_UNIT) & !SPELL_CAST_TARGET_UNIT_ENEMY;
            targets.unit_target = Some(selected_target);
        }
    }
    targets
}

pub(in crate::world) fn spell_blocks_mana_regen(template: &wow_db::SpellTemplateQuery) -> bool {
    template.power_type == POWER_TYPE_MANA
        && template.mana_cost > 0
        && (template.attributes_ex2 & SPELL_ATTR_EX2_DONT_BLOCK_MANA_REGEN) == 0
}

pub(in crate::world) async fn sync_session_player_power_from_map(
    maps: &MapRuntimeManager,
    session: &mut WorldSessionState,
    map_id: u32,
    character_guid: u32,
) {
    if let Some(snapshot) = maps.player_runtime_snapshot(map_id, character_guid).await {
        session.character.player_mana = snapshot.power1;
        session.character.player_rage = snapshot.power2;
        session.character.player_energy = snapshot.power4;
    }
}

pub(in crate::world) async fn send_player_spell_power_update(
    stream: &mut WorldPacketSink,
    caster: ObjectGuid,
    spell_profile: &SpellCastProfile,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let power_update = match spell_profile.power {
        SpellPowerCost::Rage { .. } => {
            build_player_rage_update_body(caster, session.character.player_rage)?
        }
        SpellPowerCost::Mana { .. } => {
            build_player_mana_update_body(caster, session.character.player_mana)?
        }
        SpellPowerCost::Energy { .. } => {
            build_player_energy_update_body(caster, session.character.player_energy)?
        }
    };
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &power_update,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) fn spell_cast_time_millis(cast_time: Option<SpellCastTimeEntry>) -> u32 {
    let Some(cast_time) = cast_time else {
        return 0;
    };
    cast_time
        .cast_time_millis
        .max(cast_time.min_cast_time_millis)
        .max(0) as u32
}

pub(in crate::world) async fn spell_travel_delay_millis(
    shared_world: SharedWorldDeps<'_>,
    session: &WorldSessionState,
    spell_template: &wow_db::SpellTemplateQuery,
    targets: &SpellCastTargets,
) -> u32 {
    if spell_template.speed <= 0.0 {
        return 0;
    }
    let spell_info = SpellInfo::from_template(spell_template);
    let has_missile_damage = spell_info
        .effects
        .iter()
        .any(|effect| effect.dispatch == SpellEffectDispatch::SchoolDamage);
    if !has_missile_damage {
        return 0;
    }
    let Some(character) = session.character.active_character.as_ref() else {
        return 0;
    };
    let Some(target) = targets.unit_target.filter(|target| target.is_creature()) else {
        return 0;
    };
    let Some(creature) = shared_world
        .maps
        .db_creature_snapshot(character.position.map_id, target)
        .await
    else {
        return 0;
    };
    let distance = character.position.distance_to(&creature.current_position);
    ((distance / spell_template.speed.max(f32::EPSILON)) * 1000.0)
        .round()
        .max(1.0) as u32
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct SpellCastProfile {
    pub(in crate::world) spell_id: u32,
    pub(in crate::world) kind: SpellCastKind,
    pub(in crate::world) aura_target: SpellAuraTarget,
    pub(in crate::world) bonus_damage: u32,
    pub(in crate::world) weapon_damage_percent: u32,
    pub(in crate::world) damage: u32,
    pub(in crate::world) power: SpellPowerCost,
    pub(in crate::world) requires_melee: bool,
    pub(in crate::world) requires_behind: bool,
    pub(in crate::world) needs_combo_points: bool,
    pub(in crate::world) global_cooldown_category: u32,
    pub(in crate::world) global_cooldown_millis: u64,
    pub(in crate::world) cooldown_millis: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct PlayerSpellTargetOutcome {
    pub(in crate::world) target: ObjectGuid,
    pub(in crate::world) miss_info: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum SpellCastKind {
    InstantDamage,
    DirectHeal,
    AuraApplication,
    CreateItem,
    OpeningGameObject,
    AutoRepeatRanged,
    Charge,
    NextMeleeSwing,
    Teleport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum SpellAuraTarget {
    Caster,
    UnitTarget,
    CasterAreaEnemy,
    DestinationAreaEnemy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum SpellTargetKind {
    Caster,
    Unit,
    HostileUnit,
    FriendlyUnit,
    Destination,
}

impl SpellTargetKind {
    fn requires_unit_target(self) -> bool {
        matches!(
            self,
            SpellTargetKind::Unit | SpellTargetKind::HostileUnit | SpellTargetKind::FriendlyUnit
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum SpellPowerCost {
    Rage { cost: u32 },
    Mana { cost: u32 },
    Energy { cost: u32 },
}

pub(in crate::world) const SPELL_ATTR_ON_NEXT_SWING_NO_DAMAGE: u32 = 0x0000_0004;
pub(in crate::world) const SPELL_ATTR_USES_RANGED_SLOT: u32 = 0x0000_0002;
pub(in crate::world) const SPELL_ATTR_PASSIVE: u32 = 0x0000_0040;
pub(in crate::world) const SPELL_ATTR_ON_NEXT_SWING: u32 = 0x0000_0400;
pub(in crate::world) const SPELL_INTERRUPT_FLAG_MOVEMENT: u32 = 0x01;
pub(in crate::world) const SPELL_INTERRUPT_FLAG_DAMAGE_PUSHBACK: u32 = 0x02;
pub(in crate::world) const SPELL_INTERRUPT_FLAG_DAMAGE_CANCELS: u32 = 0x10;
pub(in crate::world) const SPELL_ATTR_EX_IS_CHANNELED: u32 = 0x0000_0004;
pub(in crate::world) const SPELL_ATTR_EX_IS_SELF_CHANNELED: u32 = 0x0000_0040;
pub(in crate::world) const SPELL_ATTR_EX_FINISHING_MOVE_DAMAGE: u32 = 0x0010_0000;
pub(in crate::world) const SPELL_ATTR_EX_FINISHING_MOVE_DURATION: u32 = 0x0040_0000;
pub(in crate::world) const SPELL_EFFECT_SCHOOL_DAMAGE: u32 = 2;
pub(in crate::world) const SPELL_EFFECT_PERSISTENT_AREA_AURA: u32 = 27;
pub(in crate::world) const SPELL_EFFECT_TRIGGER_MISSILE: u32 = 32;
pub(in crate::world) const SPELL_EFFECT_WEAPON_DAMAGE_NOSCHOOL: u32 = 17;
pub(in crate::world) const SPELL_EFFECT_CREATE_ITEM: u32 = 24;
pub(in crate::world) const SPELL_EFFECT_LEAP: u32 = 29;
pub(in crate::world) const SPELL_EFFECT_WEAPON_PERCENT_DAMAGE: u32 = 31;
pub(in crate::world) const SPELL_EFFECT_WEAPON_DAMAGE: u32 = 58;
pub(in crate::world) const SPELL_EFFECT_ADD_COMBO_POINTS: u32 = 80;
pub(in crate::world) const SPELL_EFFECT_NORMALIZED_WEAPON_DMG: u32 = 121;
pub(in crate::world) const SPELL_EFFECT_APPLY_AURA: u32 = 6;
pub(in crate::world) const SPELL_EFFECT_TELEPORT_UNITS: u32 = 5;
pub(in crate::world) const SPELL_EFFECT_TELEPORT_UNITS_FACE_CASTER: u32 = 43;
pub(in crate::world) const SPELL_EFFECT_DISPEL: u32 = 38;
pub(in crate::world) const SPELL_EFFECT_DISPEL_MECHANIC: u32 = 108;
pub(in crate::world) const SPELL_EFFECT_HEAL: u32 = 10;
pub(in crate::world) const SPELL_EFFECT_ENERGIZE: u32 = 30;
pub(in crate::world) const SPELL_EFFECT_CHARGE: u32 = 96;
pub(in crate::world) const SPELL_EFFECT_DUEL: u32 = 83;
pub(in crate::world) const SPELL_EFFECT_STUCK: u32 = 84;
pub(in crate::world) const SPELL_EFFECT_SKIN_PLAYER_CORPSE: u32 = 116;
pub(in crate::world) const SPELL_AURA_PERIODIC_DAMAGE: u32 = 3;
pub(in crate::world) const SPELL_AURA_DUMMY: u32 = 4;
pub(in crate::world) const SPELL_AURA_MOD_CONFUSE: u32 = 5;
pub(in crate::world) const SPELL_AURA_MOD_FEAR: u32 = 7;
pub(in crate::world) const SPELL_AURA_PERIODIC_HEAL: u32 = 8;
pub(in crate::world) const SPELL_AURA_MOD_STUN: u32 = 12;
pub(in crate::world) const SPELL_AURA_MOD_DAMAGE_DONE: u32 = 13;
pub(in crate::world) const SPELL_AURA_MOD_STEALTH_DETECT: u32 = 17;
pub(in crate::world) const SPELL_AURA_MOD_INVISIBILITY_DETECTION: u32 = 19;
pub(in crate::world) const SPELL_AURA_OBS_MOD_HEALTH: u32 = 20;
pub(in crate::world) const SPELL_AURA_PERIODIC_TRIGGER_SPELL: u32 = 23;
pub(in crate::world) const SPELL_AURA_PERIODIC_ENERGIZE: u32 = 24;
pub(in crate::world) const SPELL_AURA_MOD_PACIFY: u32 = 25;
pub(in crate::world) const SPELL_AURA_MOD_ROOT: u32 = 26;
pub(in crate::world) const SPELL_AURA_MOD_SILENCE: u32 = 27;
pub(in crate::world) const SPELL_AURA_MOD_STAT: u32 = 29;
pub(in crate::world) const SPELL_AURA_MOD_RESISTANCE: u32 = 22;
pub(in crate::world) const SPELL_AURA_MOD_INCREASE_SPEED: u32 = 31;
pub(in crate::world) const SPELL_AURA_MOD_DECREASE_SPEED: u32 = 33;
pub(in crate::world) const SPELL_AURA_PROC_TRIGGER_SPELL: u32 = 42;
pub(in crate::world) const SPELL_AURA_MOD_PACIFY_SILENCE: u32 = 60;
pub(in crate::world) const SPELL_AURA_MOD_STALKED: u32 = 68;
pub(in crate::world) const SPELL_AURA_SCHOOL_ABSORB: u32 = 69;
pub(in crate::world) const SPELL_AURA_MANA_SHIELD: u32 = 97;
pub(in crate::world) const SPELL_AURA_MOD_RESISTANCE_PCT: u32 = 101;
pub(in crate::world) const SPELL_AURA_MOD_SKILL_TALENT: u32 = 98;
pub(in crate::world) const SPELL_AURA_MOD_SKILL: u32 = 30;
pub(in crate::world) const SPELL_AURA_MOD_REGEN: u32 = 84;
pub(in crate::world) const SPELL_AURA_MOD_POWER_REGEN: u32 = 85;
pub(in crate::world) const SPELL_AURA_MOD_ATTACK_POWER: u32 = 99;
pub(in crate::world) const SPELL_AURA_MOD_TOTAL_STAT_PERCENTAGE: u32 = 137;
pub(in crate::world) const SPELL_AURA_MOD_MELEE_HASTE: u32 = 138;
pub(in crate::world) const SPELL_AURA_MOD_REPUTATION_GAIN: u32 = 156;
pub(in crate::world) const SPELL_AURA_TRACK_CREATURES: u32 = 44;
pub(in crate::world) const SPELL_AURA_TRACK_RESOURCES: u32 = 45;
pub(in crate::world) const SPELL_AURA_TRANSFORM: u32 = 56;
pub(in crate::world) const SPELL_AURA_GHOST: u32 = 95;
pub(in crate::world) const SPELL_AURA_WATER_WALK: u32 = 104;
pub(in crate::world) const SPELL_AURA_FEATHER_FALL: u32 = 105;
pub(in crate::world) const AURA_INTERRUPT_FLAG_DAMAGE: u32 = 0x0000_0002;
pub(in crate::world) const AURA_INTERRUPT_FLAG_MOVING: u32 = 0x0000_0008;
pub(in crate::world) const AURA_INTERRUPT_FLAG_DAMAGE_CHANNEL_DURATION: u32 = 0x0000_4000;
pub(in crate::world) const AURA_INTERRUPT_FLAG_STANDING_CANCELS: u32 = 0x0004_0000;
pub(in crate::world) const PLAYER_STAND_STATE_STAND: u8 = 0;
pub(in crate::world) const PLAYER_STAND_STATE_SIT: u8 = 1;
pub(in crate::world) const PLAYER_STAND_STATE_SLEEP: u8 = 3;
pub(in crate::world) const PLAYER_STAND_STATE_DEAD: u8 = 7;
pub(in crate::world) const PLAYER_STAND_STATE_KNEEL: u8 = 8;
pub(in crate::world) const POWER_TYPE_MANA: u32 = 0;
pub(in crate::world) const POWER_TYPE_RAGE: u32 = 1;
pub(in crate::world) const POWER_TYPE_ENERGY: u32 = 3;
pub(in crate::world) const DISPEL_ALL: u32 = 7;
pub(in crate::world) const POSITIVE_AURA_FLAGS: u32 = 0x05;
pub(in crate::world) const NEGATIVE_AURA_FLAGS: u32 = 0x08;
pub(in crate::world) const TARGET_UNIT_CASTER: u32 = 1;
pub(in crate::world) const TARGET_UNIT_ENEMY: u32 = 6;
pub(in crate::world) const TARGET_ENUM_UNITS_ENEMY_AOE_AT_SRC_LOC: u32 = 15;
pub(in crate::world) const TARGET_ENUM_UNITS_ENEMY_AOE_AT_DEST_LOC: u32 = 16;
pub(in crate::world) const TARGET_LOCATION_CASTER_SRC: u32 = 22;
pub(in crate::world) const TARGET_ENUM_UNITS_ENEMY_AOE_AT_DYNOBJ_LOC: u32 = 28;
pub(in crate::world) const TARGET_UNIT_FRIEND: u32 = 21;
pub(in crate::world) const TARGET_UNIT: u32 = 25;
pub(in crate::world) const TARGET_UNIT_PARTY: u32 = 35;
pub(in crate::world) const TARGET_ENUM_UNITS_ENEMY_WITHIN_CASTER_RANGE: u32 = 36;
pub(in crate::world) const TARGET_LOCATION_CASTER_TARGET_POSITION: u32 = 53;
pub(in crate::world) const TARGET_LOCATION_CASTER_FRONT_LEAP: u32 = 55;
pub(in crate::world) const TARGET_UNIT_FRIEND_AND_PARTY: u32 = 37;
pub(in crate::world) const TARGET_UNIT_FRIEND_CHAIN_HEAL: u32 = 45;
pub(in crate::world) const TARGET_UNIT_RAID: u32 = 57;
pub(in crate::world) const TARGET_UNIT_RAID_NEAR_CASTER: u32 = 58;
pub(in crate::world) const TARGET_UNIT_RAID_AND_CLASS: u32 = 61;
pub(in crate::world) const SPELL_GROUP_RULE_UNIQUE: u32 = 1;
pub(in crate::world) const SPELL_GROUP_RULE_UNIQUE_PER_CASTER: u32 = 2;
pub(in crate::world) const PROC_FLAG_TAKE_MELEE_SWING: u32 = 0x0000_0008;
pub(in crate::world) const ITEM_SPELLTRIGGER_ON_USE: u32 = 0;
pub(in crate::world) const ITEM_SPELLTRIGGER_ON_NO_DELAY_USE: u32 = 5;
pub(in crate::world) const SPELL_ATTR_EX2_DONT_BLOCK_MANA_REGEN: u32 = 0x0200_0000;
pub(in crate::world) const SPELL_ATTR_EX2_AUTO_REPEAT: u32 = 0x0000_0020;
pub(in crate::world) const SPELL_ATTR_EX3_CASTING_CANCELS_AUTOREPEAT: u32 = 0x0040_0000;
pub(in crate::world) const SPELL_ATTR_SS_FACING_BACK: u32 = 0x0000_0008;
pub(in crate::world) const SPELL_FACING_FLAG_INFRONT: u32 = 0x0000_0001;
pub(in crate::world) const SPELL_INTERRUPT_FLAG_COMBAT: u32 = 0x08;
pub(in crate::world) const SPELL_RANGE_FLAG_MELEE: u32 = 0x1;
pub(in crate::world) const SPELL_RANGE_FLAG_RANGED: u32 = 0x2;
pub(in crate::world) const SPELL_CAST_ARC_RADIANS: f32 = std::f32::consts::PI;
pub(in crate::world) const BASE_CHARGE_SPEED: f32 = 27.0;
pub(in crate::world) const SPELL_SCHOOL_MASK_NORMAL: u32 = 0x01;
pub(in crate::world) const SPELL_FAMILY_GENERIC: u32 = 0;
pub(in crate::world) const SPELL_FAMILY_MAGE: u32 = 3;
pub(in crate::world) const SPELL_FAMILY_HUNTER: u32 = 9;
pub(in crate::world) const MECHANIC_FEAR: u32 = 5;
pub(in crate::world) const MECHANIC_ROOT: u32 = 7;
pub(in crate::world) const MECHANIC_SLEEP: u32 = 10;
pub(in crate::world) const MECHANIC_KNOCKOUT: u32 = 14;
pub(in crate::world) const MECHANIC_POLYMORPH: u32 = 17;
pub(in crate::world) const MECHANIC_BANISH: u32 = 18;
pub(in crate::world) const MECHANIC_SHACKLE: u32 = 20;
pub(in crate::world) const MECHANIC_TURN: u32 = 23;
pub(in crate::world) const POLYMORPH_HELPER_REGEN_SPELL_ID: u32 = 12_939;
pub(in crate::world) const MAX_AURA_SLOTS: usize = 48;
pub(in crate::world) const MAX_POSITIVE_AURA_SLOTS: usize = 32;
pub(in crate::world) const MAX_AURA_FLAG_FIELDS: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::world) enum DiminishingGroupRuntime {
    Polymorph,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum DiminishingLevelRuntime {
    Level1,
    Level2,
    Level3,
    Immune,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct SingleTargetAuraDescriptor {
    pub(in crate::world) spell_id: u32,
    pub(in crate::world) chain_root: u32,
    pub(in crate::world) spell_family_name: u32,
    pub(in crate::world) spell_family_flags: u64,
    pub(in crate::world) mechanic: u32,
}

pub(in crate::world) fn spell_damage_pushback_delay_millis(pushback_count: u8) -> u32 {
    match pushback_count {
        0 => 1000,
        1 => 800,
        2 => 600,
        3 => 400,
        _ => 200,
    }
}
pub(in crate::world) const MAX_AURA_LEVEL_FIELDS: usize = 12;

mod auras;
mod cooldowns;
mod effects;
mod packets;
mod spell;
mod spell_mgr;
mod targets;

pub(in crate::world) use self::auras::*;
pub(in crate::world) use self::cooldowns::*;
pub(in crate::world) use self::effects::*;
pub(in crate::world) use self::packets::*;
pub(in crate::world) use self::spell::*;
pub(in crate::world) use self::spell_mgr::*;
pub(in crate::world) use self::targets::*;

#[cfg(test)]
pub(in crate::world) fn player_spell_cast_profile(
    template: &wow_db::SpellTemplateQuery,
) -> Option<SpellCastProfile> {
    SpellInfo::from_template(template)
        .prepare_player_cast()
        .map(|prepared| prepared.profile)
}

#[cfg(test)]
pub(in crate::world) fn item_use_spell_cast_profile(
    template: &wow_db::SpellTemplateQuery,
) -> Option<SpellCastProfile> {
    SpellInfo::from_template(template)
        .prepare_item_cast(ObjectGuid::EMPTY)
        .map(|prepared| prepared.profile)
}

pub(in crate::world) fn spell_has_aura_application(template: &wow_db::SpellTemplateQuery) -> bool {
    SpellInfo::from_template(template)
        .effects
        .iter()
        .any(|effect| effect.dispatch == SpellEffectDispatch::ApplyAura)
}

pub(in crate::world) fn spell_has_direct_damage_application(
    template: &wow_db::SpellTemplateQuery,
) -> bool {
    SpellInfo::from_template(template)
        .effects
        .iter()
        .any(|effect| {
            matches!(
                effect.dispatch,
                SpellEffectDispatch::SchoolDamage
                    | SpellEffectDispatch::WeaponDamage
                    | SpellEffectDispatch::WeaponPercentDamage
            )
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::world) struct AuraRankConflictResolution {
    pub(in crate::world) failure: Option<u8>,
    pub(in crate::world) replace_spell_ids: Vec<u32>,
    pub(in crate::world) replace_any_caster_spell_ids: Vec<u32>,
}

impl AuraRankConflictResolution {
    pub(in crate::world) fn clear() -> Self {
        Self {
            failure: None,
            replace_spell_ids: Vec::new(),
            replace_any_caster_spell_ids: Vec::new(),
        }
    }
}

pub(in crate::world) async fn aura_rank_conflict_resolution(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    spell_id: u32,
    caster: ObjectGuid,
    active_auras: &[ActiveAura],
) -> anyhow::Result<AuraRankConflictResolution> {
    let conflicting_auras = active_auras
        .iter()
        .filter(|aura| aura.spell_id != spell_id || aura.caster != caster)
        .collect::<Vec<_>>();
    if conflicting_auras.is_empty() {
        return Ok(AuraRankConflictResolution::clear());
    }
    let new_chain = object_mgr.spell_chain(world_db_pool, spell_id).await?;
    let new_root = new_chain.map(spell_chain_root);
    let mut replace_spell_ids = Vec::new();
    let mut replace_any_caster_spell_ids = Vec::new();

    for existing in &conflicting_auras {
        let Some(new_chain) = new_chain else {
            continue;
        };
        let existing_chain = object_mgr
            .spell_chain(world_db_pool, existing.spell_id)
            .await?;
        let Some(existing_chain) = existing_chain.filter(|existing_chain| {
            Some(spell_chain_root(*existing_chain)) == new_root
                && existing_chain.spell_id != new_chain.spell_id
        }) else {
            continue;
        };
        if existing_chain.rank >= new_chain.rank {
            if existing.caster == caster || existing.positive {
                return Ok(AuraRankConflictResolution {
                    failure: Some(SPELL_FAILED_AURA_BOUNCED),
                    replace_spell_ids: Vec::new(),
                    replace_any_caster_spell_ids: Vec::new(),
                });
            }
            continue;
        }
        if existing.caster == caster {
            push_unique_spell_id(&mut replace_spell_ids, existing.spell_id);
        } else if existing.positive {
            push_unique_spell_id(&mut replace_any_caster_spell_ids, existing.spell_id);
        }
    }

    if conflicting_auras.iter().all(|existing| {
        existing.caster == caster && replace_spell_ids.contains(&existing.spell_id)
            || replace_any_caster_spell_ids.contains(&existing.spell_id)
    }) {
        return Ok(AuraRankConflictResolution {
            failure: None,
            replace_spell_ids,
            replace_any_caster_spell_ids,
        });
    }

    let new_groups = object_mgr
        .spell_group_memberships(world_db_pool, spell_id)
        .await?;
    if !new_groups.is_empty() {
        for existing in &conflicting_auras {
            if existing.caster == caster && replace_spell_ids.contains(&existing.spell_id)
                || replace_any_caster_spell_ids.contains(&existing.spell_id)
            {
                continue;
            }
            let existing_groups = object_mgr
                .spell_group_memberships(world_db_pool, existing.spell_id)
                .await?;
            for group in &new_groups {
                if !existing_groups
                    .iter()
                    .any(|existing_group| existing_group.group_id == group.group_id)
                {
                    continue;
                }
                match group.rule {
                    SPELL_GROUP_RULE_UNIQUE => {
                        push_unique_spell_id(&mut replace_any_caster_spell_ids, existing.spell_id);
                    }
                    SPELL_GROUP_RULE_UNIQUE_PER_CASTER if existing.caster == caster => {
                        push_unique_spell_id(&mut replace_spell_ids, existing.spell_id);
                    }
                    _ => {}
                }
                break;
            }
        }
    }
    Ok(AuraRankConflictResolution {
        failure: None,
        replace_spell_ids,
        replace_any_caster_spell_ids,
    })
}

pub(in crate::world) fn spell_chain_root(chain: wow_db::SpellChainQuery) -> u32 {
    if chain.first_spell != 0 {
        chain.first_spell
    } else {
        chain.spell_id
    }
}

pub(in crate::world) fn push_unique_spell_id(spell_ids: &mut Vec<u32>, spell_id: u32) {
    if !spell_ids.contains(&spell_id) {
        spell_ids.push(spell_id);
    }
}

pub(in crate::world) fn spell_effect_simple_value(base_points: i32) -> Option<u32> {
    (base_points >= 0).then_some((base_points + 1) as u32)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct SpellEffectValueContext {
    pub(in crate::world) spell_id: u32,
    pub(in crate::world) max_level: u32,
    pub(in crate::world) base_level: u32,
    pub(in crate::world) spell_level: u32,
    pub(in crate::world) spell_rank_level: Option<i32>,
    pub(in crate::world) combo_points: u8,
}

impl SpellEffectValueContext {
    pub(in crate::world) fn unranked(
        template: &wow_db::SpellTemplateQuery,
        combo_points: u8,
    ) -> Self {
        Self {
            spell_id: template.id,
            max_level: template.max_level,
            base_level: template.base_level,
            spell_level: template.spell_level,
            spell_rank_level: None,
            combo_points,
        }
    }

    pub(in crate::world) fn with_spell_rank_level(
        template: &wow_db::SpellTemplateQuery,
        spell_rank_level: i32,
        combo_points: u8,
    ) -> Self {
        Self {
            spell_rank_level: Some(spell_rank_level),
            ..Self::unranked(template, combo_points)
        }
    }
}

pub(in crate::world) fn player_spell_effect_value_context(
    maps: &MapRuntimeManager,
    template: &wow_db::SpellTemplateQuery,
    character_skills: &[CharacterSkill],
    combo_points: u8,
) -> SpellEffectValueContext {
    let Some(ability) = maps.skill_line_ability_for_spell(template.id) else {
        return if maps.skill_line_abilities_by_spell.is_empty() {
            SpellEffectValueContext::unranked(template, combo_points)
        } else {
            SpellEffectValueContext::with_spell_rank_level(template, 0, combo_points)
        };
    };
    let mut spell_rank = character_skills
        .iter()
        .find(|skill| u32::from(skill.skill) == ability.skill_id)
        .map(|skill| u32::from(skill.value))
        .unwrap_or(0);
    if template.max_level > 0 {
        let max_rank = template.max_level.saturating_mul(5);
        if spell_rank >= max_rank {
            spell_rank = max_rank;
        }
    }
    SpellEffectValueContext::with_spell_rank_level(template, (spell_rank / 5) as i32, combo_points)
}

pub(in crate::world) fn spell_effect_calculated_i32(
    effect: SpellInfoEffect,
    context: SpellEffectValueContext,
) -> i32 {
    let base_dice = effect.base_dice as i32;
    let mut base_points = effect.base_points as f32;
    let mut random_points = effect.die_sides;

    if effect.real_points_per_level != 0.0 {
        if let Some(mut level) = context.spell_rank_level {
            if context.max_level > 0 && level > context.max_level as i32 {
                level = context.max_level as i32;
            } else if level < context.base_level as i32 {
                level = context.base_level as i32;
            }
            level -= context.spell_level as i32;
            base_points += level as f32 * effect.real_points_per_level;
            random_points =
                random_points.saturating_add((level as f32 * effect.dice_per_level).trunc() as i32);
        } else {
            warn!(
                spell_id = context.spell_id,
                effect_id = effect.effect_id,
                "Skipping level scaling for spell effect because SkillLineAbility.dbc rank data is unavailable"
            );
        }
    }

    match random_points {
        0 | 1 => {
            base_points += base_dice as f32;
        }
        random_points => {
            let low = random_points.min(base_dice);
            let high = random_points.max(base_dice);
            base_points += rand::thread_rng().gen_range(low..=high) as f32;
        }
    }

    if effect.points_per_combo_point != 0.0 && context.combo_points > 0 {
        base_points +=
            (effect.points_per_combo_point * f32::from(context.combo_points)).trunc() as i32 as f32;
    }

    base_points.trunc() as i32
}

pub(in crate::world) fn spell_effect_calculated_u32(
    effect: SpellInfoEffect,
    context: SpellEffectValueContext,
) -> Option<u32> {
    let value = spell_effect_calculated_i32(effect, context);
    (value >= 0).then_some(value as u32)
}

pub(in crate::world) fn spell_power_cost_amount(
    template: &wow_db::SpellTemplateQuery,
    context: SpellEffectValueContext,
) -> u32 {
    let Some(rank_level) = context.spell_rank_level else {
        return template.mana_cost;
    };
    let cost = i64::from(template.mana_cost)
        + i64::from(template.mana_cost_per_level)
            * i64::from(rank_level.saturating_sub(template.base_level as i32));
    cost.clamp(0, u32::MAX as i64) as u32
}

const CMANGOS_SKILL_CATEGORY_WEAPON: i32 = 6;
const CMANGOS_SKILL_CATEGORY_CLASS: i32 = 7;
const CMANGOS_SKILL_CATEGORY_ARMOR: i32 = 8;
const CMANGOS_SKILL_CATEGORY_LANGUAGES: i32 = 10;
const CMANGOS_SKILL_POISONS: u32 = 40;
const CMANGOS_SKILL_LOCKPICKING: u32 = 633;
const CMANGOS_SKILL_FLAG_MAXIMIZED: u32 = 0x010;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CMaNGOSSkillRangeType {
    None,
    Language,
    Level,
    Mono,
}

fn cmangos_skill_range_type(skill: SkillLineEntry) -> CMaNGOSSkillRangeType {
    match skill.category_id {
        CMANGOS_SKILL_CATEGORY_LANGUAGES => CMaNGOSSkillRangeType::Language,
        CMANGOS_SKILL_CATEGORY_WEAPON => {
            if skill.id != u32::from(SKILL_FIST_WEAPONS) {
                CMaNGOSSkillRangeType::Level
            } else {
                CMaNGOSSkillRangeType::Mono
            }
        }
        CMANGOS_SKILL_CATEGORY_ARMOR | CMANGOS_SKILL_CATEGORY_CLASS => {
            if skill.id != CMANGOS_SKILL_POISONS && skill.id != CMANGOS_SKILL_LOCKPICKING {
                CMaNGOSSkillRangeType::Mono
            } else {
                CMaNGOSSkillRangeType::Level
            }
        }
        _ => CMaNGOSSkillRangeType::None,
    }
}

pub(in crate::world) fn sync_player_level_backed_skills(
    maps: &MapRuntimeManager,
    race: u8,
    class: u8,
    level: u8,
    character_skills: &mut [CharacterSkill],
) -> Vec<SkillProgressionUpdate> {
    if maps.skill_lines.is_empty() || maps.skill_race_class_infos_by_skill.is_empty() {
        return set_level_capped_combat_skill_maxes(level, character_skills);
    }

    let level_cap = u16::from(level.max(1)).saturating_mul(5);
    character_skills
        .iter_mut()
        .enumerate()
        .filter_map(|(slot, skill)| {
            let skill_id = u32::from(skill.skill);
            let skill_line = maps.skill_line(skill_id)?;
            let range_type = cmangos_skill_range_type(skill_line);
            if !matches!(
                range_type,
                CMaNGOSSkillRangeType::Level | CMaNGOSSkillRangeType::Mono
            ) || skill.max == 1
            {
                return None;
            }
            let skill_info = maps.skill_race_class_info(skill_id, race, class);
            let maxed = skill_info
                .map(|entry| (entry.flags & CMANGOS_SKILL_FLAG_MAXIMIZED) != 0)
                .unwrap_or(false);
            let old_value = skill.value;
            let old_max = skill.max;
            skill.max = level_cap;
            if maxed || skill.value > level_cap {
                skill.value = level_cap;
            }
            (skill.value != old_value || skill.max != old_max).then_some(SkillProgressionUpdate {
                slot,
                skill: skill.skill,
                value: skill.value,
                max: skill.max,
            })
        })
        .collect()
}

pub(in crate::world) fn build_active_aura(
    template: &wow_db::SpellTemplateQuery,
    caster: ObjectGuid,
    level: u8,
    value_context: SpellEffectValueContext,
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
    let spell_info = SpellInfo::from_template(template);
    ActiveAura {
        spell_id: template.id,
        caster,
        level,
        interrupt_flags: template.aura_interrupt_flags,
        positive: active_aura_is_positive(&spell_info),
        visible: true,
        duration_millis: (duration_millis > 0).then_some(duration_millis as u32),
        expires_at: (duration_millis > 0)
            .then_some(now + Duration::from_millis(duration_millis as u64)),
        periodic_damage: spell_periodic_damage_aura(&spell_info, level, value_context, now),
        periodic_regen: spell_periodic_regen_aura(&spell_info, value_context, now),
        stat_modifiers: spell_aura_stat_modifiers(&spell_info, value_context),
        proc_triggers: spell_aura_proc_triggers(&spell_info),
    }
}

pub(in crate::world) async fn resolve_active_aura_transform_displays(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    aura: &mut ActiveAura,
) -> anyhow::Result<()> {
    for modifier in &mut aura.stat_modifiers {
        let AuraStatModifier::Transform {
            display_id,
            creature_entry,
        } = modifier
        else {
            continue;
        };
        if *creature_entry == 0 {
            continue;
        }
        let Some(template) = object_mgr
            .creature_template(world_db_pool, *creature_entry)
            .await?
        else {
            warn!(
                spell_id = aura.spell_id,
                creature_entry, "Transform aura references missing creature_template entry"
            );
            continue;
        };
        *display_id = choose_creature_display(&template).display_id;
    }
    Ok(())
}

pub(in crate::world) fn active_aura_is_positive(spell_info: &SpellInfo<'_>) -> bool {
    !spell_info.effects.iter().any(|effect| {
        effect.dispatch == SpellEffectDispatch::ApplyAura
            && (effect_targets_direct_hostile_unit(*effect)
                || effect_targets_caster_centered_hostile_area(*effect)
                || effect_targets_destination_hostile_area(*effect)
                || effect.aura_name == SPELL_AURA_PERIODIC_DAMAGE)
    })
}

pub(in crate::world) fn spell_periodic_damage_aura(
    spell_info: &SpellInfo<'_>,
    caster_level: u8,
    value_context: SpellEffectValueContext,
    now: Instant,
) -> Option<PeriodicDamageAura> {
    spell_info
        .effects
        .iter()
        .copied()
        .find(|effect| {
            effect.dispatch == SpellEffectDispatch::ApplyAura
                && effect.aura_name == SPELL_AURA_PERIODIC_DAMAGE
                && effect.amplitude > 0
        })
        .and_then(|effect| {
            let damage = spell_effect_calculated_u32(effect, value_context)?;
            Some(PeriodicDamageAura {
                aura_name: effect.aura_name,
                school: spell_info.template.school,
                damage_class: spell_info.template.dmg_class,
                attributes_ex2: spell_info.template.attributes_ex2,
                attributes_ex3: spell_info.template.attributes_ex3,
                caster_snapshot: spell_periodic_damage_fallback_caster_snapshot(caster_level),
                amount: damage,
                tick_millis: effect.amplitude,
                next_tick_at: now + Duration::from_millis(effect.amplitude as u64),
            })
        })
}

pub(in crate::world) fn spell_periodic_damage_fallback_caster_snapshot(
    caster_level: u8,
) -> SpellCombatUnitSnapshot {
    SpellCombatUnitSnapshot {
        level: caster_level.max(1),
        class: 0,
        intellect: 0,
        resistances: [0; MAX_SPELL_SCHOOL],
    }
}

pub(in crate::world) fn spell_periodic_regen_aura(
    spell_info: &SpellInfo<'_>,
    value_context: SpellEffectValueContext,
    now: Instant,
) -> Option<PeriodicRegenAura> {
    let mut health_amount = 0u32;
    let mut mana_amount = 0u32;
    let mut tick_millis = 0u32;
    for effect in spell_info.effects {
        if effect.dispatch != SpellEffectDispatch::ApplyAura {
            continue;
        }
        let Some(amount) = spell_effect_calculated_u32(effect, value_context) else {
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
        interrupts_on_move_and_stand: false,
        suppresses_recent_damage: false,
        makes_player_sit: false,
    })
}

pub(in crate::world) fn mark_active_aura_periodic_regen_as_consumable(aura: &mut ActiveAura) {
    let Some(regen) = aura.periodic_regen.as_mut() else {
        return;
    };
    regen.interrupts_on_move_and_stand = true;
    regen.suppresses_recent_damage = true;
    regen.makes_player_sit = true;
}

pub(in crate::world) fn spell_is_mage_polymorph(template: &wow_db::SpellTemplateQuery) -> bool {
    template.spell_family_name == SPELL_FAMILY_MAGE
        && template.spell_family_flags & 0x0100_0000 != 0
        && SpellInfo::from_template(template)
            .effects
            .iter()
            .any(|effect| effect.aura_name == SPELL_AURA_MOD_CONFUSE)
}

pub(in crate::world) fn spell_is_single_target_aura_template(
    template: &wow_db::SpellTemplateQuery,
) -> bool {
    match template.mechanic {
        MECHANIC_FEAR | MECHANIC_TURN => true,
        MECHANIC_ROOT | MECHANIC_SLEEP | MECHANIC_KNOCKOUT | MECHANIC_POLYMORPH
        | MECHANIC_BANISH | MECHANIC_SHACKLE => {
            template.spell_family_name != SPELL_FAMILY_GENERIC && template.spell_family_flags != 0
        }
        _ => {
            template.spell_family_name == SPELL_FAMILY_HUNTER
                && SpellInfo::from_template(template)
                    .effects
                    .iter()
                    .any(|effect| effect.aura_name == SPELL_AURA_MOD_STALKED)
        }
    }
}

pub(in crate::world) async fn single_target_aura_descriptor(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    template: &wow_db::SpellTemplateQuery,
) -> anyhow::Result<Option<SingleTargetAuraDescriptor>> {
    if !spell_is_single_target_aura_template(template) {
        return Ok(None);
    }
    let chain_root = object_mgr
        .spell_chain(world_db_pool, template.id)
        .await?
        .map(spell_chain_root)
        .unwrap_or(template.id);
    Ok(Some(SingleTargetAuraDescriptor {
        spell_id: template.id,
        chain_root,
        spell_family_name: template.spell_family_name,
        spell_family_flags: template.spell_family_flags,
        mechanic: template.mechanic,
    }))
}

pub(in crate::world) fn single_target_aura_descriptors_match(
    left: SingleTargetAuraDescriptor,
    right: SingleTargetAuraDescriptor,
) -> bool {
    left.spell_id == right.spell_id
        || (left.chain_root != 0 && left.chain_root == right.chain_root)
        || (left.spell_family_name != SPELL_FAMILY_GENERIC
            && right.spell_family_name != SPELL_FAMILY_GENERIC
            && left.spell_family_name == right.spell_family_name
            && left.spell_family_flags != 0
            && left.spell_family_flags == right.spell_family_flags)
}

pub(in crate::world) fn spell_diminishing_group(
    template: &wow_db::SpellTemplateQuery,
) -> Option<DiminishingGroupRuntime> {
    (template.mechanic == MECHANIC_POLYMORPH).then_some(DiminishingGroupRuntime::Polymorph)
}

pub(in crate::world) fn db_creature_spell_diminishing_group(
    template: &wow_db::SpellTemplateQuery,
) -> Option<DiminishingGroupRuntime> {
    // CMaNGOS classifies Polymorph as DRTYPE_PLAYER, so ordinary DB creatures
    // are not diminished through PvP DR levels.
    if template.mechanic == MECHANIC_POLYMORPH {
        None
    } else {
        spell_diminishing_group(template)
    }
}

pub(in crate::world) fn diminishing_duration_millis(
    duration_millis: Option<u32>,
    level: DiminishingLevelRuntime,
) -> Option<u32> {
    let duration = duration_millis?;
    Some(match level {
        DiminishingLevelRuntime::Level1 => duration,
        DiminishingLevelRuntime::Level2 => duration / 2,
        DiminishingLevelRuntime::Level3 => duration / 4,
        DiminishingLevelRuntime::Immune => 0,
    })
}

pub(in crate::world) fn spell_aura_proc_triggers(
    spell_info: &SpellInfo<'_>,
) -> Vec<AuraProcTrigger> {
    spell_info
        .effects
        .into_iter()
        .filter(|effect| {
            effect.dispatch == SpellEffectDispatch::ApplyAura
                && effect.aura_name == SPELL_AURA_PROC_TRIGGER_SPELL
                && effect.trigger_spell != 0
        })
        .map(|effect| AuraProcTrigger {
            triggered_spell_id: effect.trigger_spell,
            proc_flags: spell_info.template.proc_flags,
            proc_chance: spell_info.template.proc_chance,
            remaining_charges: (spell_info.template.proc_charges > 0)
                .then_some(spell_info.template.proc_charges),
        })
        .collect()
}

pub(in crate::world) fn active_aura_proc_trigger_spell_ids(
    active_auras: &mut [ActiveAura],
    proc_flag: u32,
    now: Instant,
) -> Vec<u32> {
    let mut triggered_spell_ids = Vec::new();
    for aura in active_auras
        .iter_mut()
        .filter(|aura| aura.expires_at.is_none_or(|expires_at| now < expires_at))
    {
        for trigger in &mut aura.proc_triggers {
            if trigger.proc_flags & proc_flag == 0 {
                continue;
            }
            if trigger.remaining_charges == Some(0) {
                continue;
            }
            if !aura_proc_roll_succeeds(trigger.proc_chance) {
                continue;
            }
            if let Some(remaining_charges) = trigger.remaining_charges.as_mut() {
                *remaining_charges = remaining_charges.saturating_sub(1);
            }
            triggered_spell_ids.push(trigger.triggered_spell_id);
        }
    }
    triggered_spell_ids
}

pub(in crate::world) fn aura_proc_roll_succeeds(proc_chance: u32) -> bool {
    if proc_chance >= 100 {
        return true;
    }
    if proc_chance == 0 {
        return false;
    }
    rand::thread_rng().gen_range(1..=10_000) <= proc_chance.saturating_mul(100).min(10_000)
}

pub(in crate::world) fn passive_spell_active_aura(
    template: &wow_db::SpellTemplateQuery,
    caster: ObjectGuid,
    level: u8,
    value_context: SpellEffectValueContext,
    now: Instant,
    duration: Option<SpellDurationEntry>,
) -> Option<ActiveAura> {
    if !spell_needs_passive_cast_at_learn(template) {
        return None;
    }
    let mut aura = build_active_aura(template, caster, level, value_context, now, duration);
    aura.visible = false;
    (!aura.stat_modifiers.is_empty()).then_some(aura)
}

pub(in crate::world) fn spell_needs_passive_cast_at_learn(
    template: &wow_db::SpellTemplateQuery,
) -> bool {
    template.attributes & SPELL_ATTR_PASSIVE != 0 && spell_has_aura_application(template)
}

pub(in crate::world) fn spell_aura_stat_modifiers(
    spell_info: &SpellInfo<'_>,
    value_context: SpellEffectValueContext,
) -> Vec<AuraStatModifier> {
    spell_info
        .effects
        .into_iter()
        .filter(|effect| effect.dispatch == SpellEffectDispatch::ApplyAura)
        .filter_map(|effect| match effect.aura_name {
            SPELL_AURA_MOD_SKILL | SPELL_AURA_MOD_SKILL_TALENT => {
                let skill_id = u16::try_from(effect.misc_value).ok()?;
                Some(AuraStatModifier::Skill {
                    skill_id,
                    amount: spell_effect_calculated_i32(effect, value_context)
                        .clamp(i16::MIN as i32, i16::MAX as i32) as i16,
                    permanent: effect.aura_name == SPELL_AURA_MOD_SKILL_TALENT,
                })
            }
            SPELL_AURA_MOD_ATTACK_POWER => Some(AuraStatModifier::AttackPower {
                amount: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_DAMAGE_DONE => Some(AuraStatModifier::DamageDone {
                school_mask: spell_school_mask_from_misc_value(effect.misc_value),
                amount: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_INCREASE_SPEED => Some(AuraStatModifier::MoveSpeedPercent {
                percent: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_DECREASE_SPEED => Some(AuraStatModifier::MoveSpeedPercent {
                percent: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_MELEE_HASTE => Some(AuraStatModifier::MeleeAttackTimePercent {
                percent: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_RESISTANCE => Some(AuraStatModifier::Resistance {
                school_mask: spell_school_mask_from_misc_value(effect.misc_value),
                amount: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_RESISTANCE_PCT => Some(AuraStatModifier::ResistancePercent {
                school_mask: spell_school_mask_from_misc_value(effect.misc_value),
                percent: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_ROOT => Some(AuraStatModifier::Root),
            SPELL_AURA_MOD_STUN => Some(AuraStatModifier::Stun),
            SPELL_AURA_MOD_CONFUSE => Some(AuraStatModifier::Confuse),
            SPELL_AURA_MOD_FEAR => Some(AuraStatModifier::Fear),
            SPELL_AURA_TRANSFORM => {
                let display_id = spell_effect_calculated_u32(effect, value_context).unwrap_or(0);
                Some(AuraStatModifier::Transform {
                    display_id,
                    creature_entry: u32::try_from(effect.misc_value).unwrap_or(0),
                })
            }
            SPELL_AURA_MOD_PACIFY => Some(AuraStatModifier::Pacify),
            SPELL_AURA_MOD_SILENCE => Some(AuraStatModifier::Silence),
            SPELL_AURA_MOD_PACIFY_SILENCE => Some(AuraStatModifier::PacifySilence),
            SPELL_AURA_FEATHER_FALL => Some(AuraStatModifier::FeatherFall),
            SPELL_AURA_SCHOOL_ABSORB => Some(AuraStatModifier::SchoolAbsorb {
                school_mask: spell_school_mask_from_misc_value(effect.misc_value),
                amount: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MANA_SHIELD => Some(AuraStatModifier::ManaShield {
                school_mask: spell_school_mask_from_misc_value(effect.misc_value),
                amount: spell_effect_calculated_i32(effect, value_context),
                mana_multiplier_millis: (effect.multiple_value.max(0.0) * 1000.0).round() as u32,
            }),
            SPELL_AURA_MOD_STAT => {
                let stat = usize::try_from(effect.misc_value).ok();
                Some(AuraStatModifier::Stat {
                    stat: stat.filter(|stat| *stat < MAX_STATS),
                    amount: spell_effect_calculated_i32(effect, value_context),
                })
            }
            SPELL_AURA_MOD_TOTAL_STAT_PERCENTAGE => {
                let stat = usize::try_from(effect.misc_value).ok()?;
                (stat < MAX_STATS).then_some(AuraStatModifier::TotalStatPercent {
                    stat,
                    percent: spell_effect_calculated_i32(effect, value_context),
                })
            }
            SPELL_AURA_MOD_REPUTATION_GAIN => Some(AuraStatModifier::ReputationGainPercent {
                percent: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_STEALTH_DETECT => Some(AuraStatModifier::StealthDetect {
                kind: effect.misc_value,
                amount: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_INVISIBILITY_DETECTION => Some(AuraStatModifier::InvisibilityDetect {
                kind: effect.misc_value,
                amount: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_TRACK_CREATURES => Some(AuraStatModifier::TrackCreatures {
                creature_type: effect.misc_value,
            }),
            SPELL_AURA_TRACK_RESOURCES => Some(AuraStatModifier::TrackResources {
                resource_type: effect.misc_value,
            }),
            SPELL_AURA_GHOST => Some(AuraStatModifier::Ghost),
            SPELL_AURA_WATER_WALK => Some(AuraStatModifier::WaterWalk),
            SPELL_AURA_DUMMY => Some(AuraStatModifier::Dummy {
                aura_name: effect.aura_name,
                misc_value: effect.misc_value,
                amount: spell_effect_calculated_i32(effect, value_context),
            }),
            _ => None,
        })
        .chain(
            (spell_info.template.dispel > 0).then_some(AuraStatModifier::DispelType {
                dispel_type: spell_info.template.dispel,
            }),
        )
        .collect()
}

pub(in crate::world) fn spell_school_mask_from_misc_value(misc_value: i32) -> u32 {
    if misc_value < 0 {
        u32::MAX
    } else {
        misc_value as u32
    }
}

pub(in crate::world) fn active_aura_skill_bonus(active_auras: &[ActiveAura], skill_id: u16) -> i16 {
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

pub(in crate::world) fn active_aura_skill_bonus_pair(
    active_auras: &[ActiveAura],
    skill_id: u16,
) -> u32 {
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

pub(in crate::world) fn current_skill_value_with_active_auras(
    character_skills: &[CharacterSkill],
    active_auras: &[ActiveAura],
    skill_id: u16,
) -> u16 {
    let value = i32::from(current_skill_value(character_skills, skill_id));
    let bonus = i32::from(active_aura_skill_bonus(active_auras, skill_id));
    value.saturating_add(bonus).clamp(0, u16::MAX as i32) as u16
}

pub(in crate::world) fn reputation_gain_percent_from_active_auras(
    active_auras: &[ActiveAura],
) -> i32 {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .filter_map(|modifier| match modifier {
            AuraStatModifier::ReputationGainPercent { percent } => Some(*percent),
            _ => None,
        })
        .sum()
}

pub(in crate::world) fn active_aura_has_root(active_auras: &[ActiveAura]) -> bool {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .any(|modifier| *modifier == AuraStatModifier::Root)
}

pub(in crate::world) fn active_aura_has_stun(active_auras: &[ActiveAura]) -> bool {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .any(|modifier| *modifier == AuraStatModifier::Stun)
}

pub(in crate::world) fn active_aura_has_confuse(active_auras: &[ActiveAura]) -> bool {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .any(|modifier| *modifier == AuraStatModifier::Confuse)
}

pub(in crate::world) fn active_aura_has_hard_control(active_auras: &[ActiveAura]) -> bool {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .any(|modifier| {
            matches!(
                modifier,
                AuraStatModifier::Stun
                    | AuraStatModifier::Confuse
                    | AuraStatModifier::Fear
                    | AuraStatModifier::Pacify
                    | AuraStatModifier::PacifySilence
            )
        })
}

pub(in crate::world) fn active_aura_player_spell_cast_failure(
    active_auras: &[ActiveAura],
    spell_profile: &SpellCastProfile,
) -> Option<u8> {
    if active_aura_has_modifier(active_auras, |modifier| *modifier == AuraStatModifier::Stun) {
        return Some(SPELL_FAILED_STUNNED);
    }
    if spell_cast_is_silence_prevented(spell_profile)
        && active_aura_has_modifier(active_auras, |modifier| {
            matches!(
                modifier,
                AuraStatModifier::Silence | AuraStatModifier::PacifySilence
            )
        })
    {
        return Some(SPELL_FAILED_SILENCED);
    }
    if spell_cast_is_pacify_prevented(spell_profile)
        && active_aura_has_modifier(active_auras, |modifier| {
            matches!(
                modifier,
                AuraStatModifier::Pacify | AuraStatModifier::PacifySilence
            )
        })
    {
        return Some(SPELL_FAILED_PACIFIED);
    }
    if active_aura_has_modifier(active_auras, |modifier| *modifier == AuraStatModifier::Fear) {
        return Some(SPELL_FAILED_FLEEING);
    }
    if active_aura_has_modifier(active_auras, |modifier| {
        *modifier == AuraStatModifier::Confuse
    }) {
        return Some(SPELL_FAILED_CONFUSED);
    }
    None
}

pub(in crate::world) fn active_aura_existing_player_spell_interrupt_failure(
    active_auras: &[ActiveAura],
    spell_profile: &SpellCastProfile,
) -> Option<u8> {
    if active_aura_has_modifier(active_auras, |modifier| *modifier == AuraStatModifier::Stun) {
        return Some(SPELL_FAILED_STUNNED);
    }
    if spell_cast_is_silence_prevented(spell_profile)
        && active_aura_has_modifier(active_auras, |modifier| {
            matches!(
                modifier,
                AuraStatModifier::Silence | AuraStatModifier::PacifySilence
            )
        })
    {
        return Some(SPELL_FAILED_SILENCED);
    }
    if active_aura_has_modifier(active_auras, |modifier| *modifier == AuraStatModifier::Fear) {
        return Some(SPELL_FAILED_FLEEING);
    }
    if active_aura_has_modifier(active_auras, |modifier| {
        *modifier == AuraStatModifier::Confuse
    }) {
        return Some(SPELL_FAILED_CONFUSED);
    }
    None
}

pub(in crate::world) fn active_aura_creature_spell_cast_failure(
    active_auras: &[ActiveAura],
) -> Option<u8> {
    if active_aura_has_modifier(active_auras, |modifier| *modifier == AuraStatModifier::Stun) {
        return Some(SPELL_FAILED_STUNNED);
    }
    if active_aura_has_modifier(active_auras, |modifier| {
        matches!(
            modifier,
            AuraStatModifier::Silence | AuraStatModifier::PacifySilence
        )
    }) {
        return Some(SPELL_FAILED_SILENCED);
    }
    if active_aura_has_modifier(active_auras, |modifier| *modifier == AuraStatModifier::Fear) {
        return Some(SPELL_FAILED_FLEEING);
    }
    if active_aura_has_modifier(active_auras, |modifier| {
        *modifier == AuraStatModifier::Confuse
    }) {
        return Some(SPELL_FAILED_CONFUSED);
    }
    None
}

fn active_aura_has_modifier(
    active_auras: &[ActiveAura],
    predicate: impl Fn(&AuraStatModifier) -> bool,
) -> bool {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .any(predicate)
}

fn spell_cast_is_silence_prevented(spell_profile: &SpellCastProfile) -> bool {
    !matches!(
        spell_profile.kind,
        SpellCastKind::AutoRepeatRanged | SpellCastKind::Charge | SpellCastKind::NextMeleeSwing
    )
}

fn spell_cast_is_pacify_prevented(spell_profile: &SpellCastProfile) -> bool {
    spell_profile.requires_melee
        || matches!(
            spell_profile.kind,
            SpellCastKind::AutoRepeatRanged | SpellCastKind::Charge | SpellCastKind::NextMeleeSwing
        )
}

pub(in crate::world) fn active_aura_dispel_type(active_aura: &ActiveAura) -> Option<u32> {
    active_aura
        .stat_modifiers
        .iter()
        .find_map(|modifier| match modifier {
            AuraStatModifier::DispelType { dispel_type } => Some(*dispel_type),
            _ => None,
        })
}

pub(in crate::world) fn active_aura_matches_dispel_type(
    active_aura: &ActiveAura,
    dispel_type: u32,
) -> bool {
    active_aura_dispel_type(active_aura).is_some_and(|aura_dispel_type| {
        dispel_type == DISPEL_ALL || aura_dispel_type == dispel_type
    })
}

pub(in crate::world) fn active_aura_blocks_movement(active_auras: &[ActiveAura]) -> bool {
    active_aura_has_root(active_auras) || active_aura_has_stun(active_auras)
}

pub(in crate::world) fn active_aura_transform_display_id(
    active_auras: &[ActiveAura],
) -> Option<u32> {
    active_auras.iter().rev().find_map(|aura| {
        aura.stat_modifiers
            .iter()
            .find_map(|modifier| match modifier {
                AuraStatModifier::Transform { display_id, .. } if *display_id != 0 => {
                    Some(*display_id)
                }
                _ => None,
            })
    })
}

pub(in crate::world) fn active_aura_breaks_on_damage(active_aura: &ActiveAura) -> bool {
    active_aura.interrupt_flags & AURA_INTERRUPT_FLAG_DAMAGE != 0
        && active_aura.stat_modifiers.iter().any(|modifier| {
            matches!(
                modifier,
                AuraStatModifier::Confuse
                    | AuraStatModifier::Stun
                    | AuraStatModifier::Transform { .. }
            )
        })
}

pub(in crate::world) fn active_aura_suppresses_hostile_refs(active_aura: &ActiveAura) -> bool {
    active_aura
        .stat_modifiers
        .iter()
        .any(|modifier| matches!(modifier, AuraStatModifier::Confuse | AuraStatModifier::Fear))
        || (active_aura_breaks_on_damage(active_aura)
            && active_aura
                .stat_modifiers
                .iter()
                .any(|modifier| matches!(modifier, AuraStatModifier::Stun)))
}

pub(in crate::world) fn active_auras_suppress_hostile_refs(active_auras: &[ActiveAura]) -> bool {
    active_auras.iter().any(active_aura_suppresses_hostile_refs)
}

pub(in crate::world) fn active_aura_movement_speed_multiplier(active_auras: &[ActiveAura]) -> f32 {
    if active_aura_blocks_movement(active_auras) {
        return 0.0;
    }

    let strongest_slow = active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .filter_map(|modifier| match modifier {
            AuraStatModifier::MoveSpeedPercent { percent } if *percent < 0 => Some(*percent),
            _ => None,
        })
        .min()
        .unwrap_or(0);
    let strongest_increase = active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .filter_map(|modifier| match modifier {
            AuraStatModifier::MoveSpeedPercent { percent } if *percent > 0 => Some(*percent),
            _ => None,
        })
        .max()
        .unwrap_or(0);

    ((100 + strongest_slow).max(0) as f32 / 100.0)
        * ((100 + strongest_increase).max(1) as f32 / 100.0)
}

pub(in crate::world) fn active_aura_melee_attack_time_multiplier(
    active_auras: &[ActiveAura],
) -> f32 {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .filter_map(|modifier| match modifier {
            AuraStatModifier::MeleeAttackTimePercent { percent } => Some(*percent),
            _ => None,
        })
        .fold(1.0, |multiplier, percent| {
            let effect = if percent >= 0 {
                (100 - percent).max(0) as f32 / 100.0
            } else {
                (100 + percent.saturating_abs()).max(0) as f32 / 100.0
            };
            multiplier * effect
        })
}

pub(in crate::world) fn active_aura_physical_damage_done(active_auras: &[ActiveAura]) -> i32 {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .filter_map(|modifier| match modifier {
            AuraStatModifier::DamageDone {
                school_mask,
                amount,
            } if *school_mask == 0 || *school_mask & 1 != 0 => Some(*amount),
            _ => None,
        })
        .sum()
}

pub(in crate::world) fn active_aura_track_creatures_mask(active_auras: &[ActiveAura]) -> u32 {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .filter_map(|modifier| match modifier {
            AuraStatModifier::TrackCreatures { creature_type } if *creature_type > 0 => {
                u32::try_from(*creature_type - 1)
                    .ok()
                    .filter(|bit| *bit < 32)
                    .map(|bit| 1u32 << bit)
            }
            _ => None,
        })
        .fold(0, |mask, flag| mask | flag)
}

pub(in crate::world) fn active_aura_track_resources_mask(active_auras: &[ActiveAura]) -> u32 {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .filter_map(|modifier| match modifier {
            AuraStatModifier::TrackResources { resource_type } if *resource_type > 0 => {
                u32::try_from(*resource_type - 1)
                    .ok()
                    .filter(|bit| *bit < 32)
                    .map(|bit| 1u32 << bit)
            }
            _ => None,
        })
        .fold(0, |mask, flag| mask | flag)
}

pub(in crate::world) fn active_aura_unit_vis_flags(active_auras: &[ActiveAura]) -> u32 {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .fold(0, |flags, modifier| match modifier {
            AuraStatModifier::Ghost => flags | UNIT_VIS_FLAG_GHOST,
            _ => flags,
        })
}

pub(in crate::world) fn player_world_stats_with_active_auras(
    mut world_stats: PlayerWorldStats,
    active_auras: &[ActiveAura],
) -> PlayerWorldStats {
    for modifier in active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
    {
        let AuraStatModifier::Stat { stat, amount } = modifier else {
            continue;
        };
        if let Some(stat) = stat {
            world_stats.stats[*stat] = apply_flat_modifier(world_stats.stats[*stat], *amount);
        } else {
            for stat_value in &mut world_stats.stats {
                *stat_value = apply_flat_modifier(*stat_value, *amount);
            }
        }
    }

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

pub(in crate::world) fn player_stat_mod_deltas(
    base_world_stats: &PlayerWorldStats,
    effective_world_stats: &PlayerWorldStats,
) -> [i32; MAX_STATS] {
    let mut deltas = [0i32; MAX_STATS];
    for (offset, delta) in deltas.iter_mut().enumerate() {
        *delta = effective_world_stats.stats[offset] as i32 - base_world_stats.stats[offset] as i32;
    }
    deltas
}

pub(in crate::world) fn apply_flat_modifier(value: u32, amount: i32) -> u32 {
    (value as i64)
        .saturating_add(i64::from(amount))
        .clamp(0, u32::MAX as i64) as u32
}

pub(in crate::world) fn apply_percent_modifier(value: u32, percent: i32) -> u32 {
    let multiplier = 100i64.saturating_add(i64::from(percent));
    if multiplier <= 0 {
        return 0;
    }
    ((i64::from(value) * multiplier) / 100).clamp(0, u32::MAX as i64) as u32
}

pub(in crate::world) async fn consume_used_item(
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
    session.inventory.items =
        wow_db::get_character_inventory_items(character_db_pool, character_guid).await?;
    match destroyed {
        wow_db::InventoryDestroyResult::CountChanged { item, count } => {
            let body = build_item_stack_count_update_body(item, count)?;
            send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(header_crypto)).await?;
        }
        wow_db::InventoryDestroyResult::Removed { item } => {
            if source_item.bag == INVENTORY_SLOT_BAG_0 as u32 {
                let body = build_inventory_slots_update_body(
                    character_guid,
                    &session.inventory.items,
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

pub(in crate::world) fn item_use_spell(
    template: &ItemTemplateQuery,
    requested_index: u8,
) -> Option<wow_db::ItemTemplateSpell> {
    let requested = template.spells.get(requested_index as usize).copied();
    requested
        .filter(|spell| is_item_use_spell(*spell))
        .or_else(|| {
            template
                .spells
                .into_iter()
                .find(|spell| is_item_use_spell(*spell))
        })
}

pub(in crate::world) fn is_item_use_spell(spell: wow_db::ItemTemplateSpell) -> bool {
    spell.spell_id != 0
        && matches!(
            spell.spell_trigger,
            ITEM_SPELLTRIGGER_ON_USE | ITEM_SPELLTRIGGER_ON_NO_DELAY_USE
        )
}

pub(in crate::world) fn apply_player_aura(session: &mut WorldSessionState, aura: ActiveAura) {
    apply_active_aura(&mut session.auras.active_auras, aura);
}

pub(in crate::world) fn apply_player_aura_replacing_conflicts(
    session: &mut WorldSessionState,
    aura: ActiveAura,
    resolution: &AuraRankConflictResolution,
) {
    apply_active_aura_replacing_conflicts(&mut session.auras.active_auras, aura, resolution);
}

pub(in crate::world) fn apply_active_aura(active_auras: &mut Vec<ActiveAura>, aura: ActiveAura) {
    if let Some(existing) = active_auras
        .iter_mut()
        .find(|existing| existing.spell_id == aura.spell_id && existing.caster == aura.caster)
    {
        *existing = aura;
    } else {
        active_auras.push(aura);
    }
}

#[cfg(test)]
pub(in crate::world) fn apply_active_aura_replacing_spell_ids(
    active_auras: &mut Vec<ActiveAura>,
    aura: ActiveAura,
    replace_spell_ids: &[u32],
) {
    if !replace_spell_ids.is_empty() {
        active_auras.retain(|existing| {
            existing.caster != aura.caster || !replace_spell_ids.contains(&existing.spell_id)
        });
    }
    apply_active_aura(active_auras, aura);
}

pub(in crate::world) fn apply_active_aura_replacing_conflicts(
    active_auras: &mut Vec<ActiveAura>,
    aura: ActiveAura,
    resolution: &AuraRankConflictResolution,
) {
    if !resolution.replace_spell_ids.is_empty()
        || !resolution.replace_any_caster_spell_ids.is_empty()
    {
        active_auras.retain(|existing| {
            !resolution
                .replace_any_caster_spell_ids
                .contains(&existing.spell_id)
                && (existing.caster != aura.caster
                    || !resolution.replace_spell_ids.contains(&existing.spell_id))
        });
    }
    apply_active_aura(active_auras, aura);
}

pub(in crate::world) fn expire_session_auras(session: &mut WorldSessionState, now: Instant) {
    session
        .auras
        .active_auras
        .retain(|aura| aura.expires_at.is_none_or(|expires_at| now < expires_at));
}

pub(in crate::world) fn active_aura_interrupt_flags(aura: &ActiveAura) -> u32 {
    let derived = aura.periodic_regen.map_or(0, |regen| {
        if regen.interrupts_on_move_and_stand {
            AURA_INTERRUPT_FLAG_DAMAGE
                | AURA_INTERRUPT_FLAG_MOVING
                | AURA_INTERRUPT_FLAG_STANDING_CANCELS
        } else {
            0
        }
    });
    aura.interrupt_flags | derived
}

pub(in crate::world) fn remove_active_auras_with_interrupt_flag(
    active_auras: &mut Vec<ActiveAura>,
    interrupt_flag: u32,
) -> bool {
    let before = active_auras.len();
    active_auras.retain(|aura| active_aura_interrupt_flags(aura) & interrupt_flag == 0);
    active_auras.len() != before
}

pub(in crate::world) async fn interrupt_player_consumable_auras(
    stream: &mut WorldPacketSink,
    maps: &Arc<MapRuntimeManager>,
    sessions: &Arc<SessionRegistry>,
    session: &mut WorldSessionState,
    interrupt_flag: u32,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    if !remove_active_auras_with_interrupt_flag(&mut session.auras.active_auras, interrupt_flag) {
        return Ok(false);
    }
    session.character.player_stand_state = PLAYER_STAND_STATE_STAND;
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(true);
    };
    let map_id = character.position.map_id;
    let character_guid = character.guid;
    let player = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    maps.remove_player_auras_with_interrupt_flag(map_id, character_guid, interrupt_flag)
        .await;
    let aura_packet = OutboundWorldPacket {
        opcode: SMSG_UPDATE_OBJECT,
        body: build_player_aura_update_body(player, &session.auras.active_auras)?,
    };
    let stand_packet = OutboundWorldPacket {
        opcode: SMSG_UPDATE_OBJECT,
        body: build_player_stand_state_update_body(
            character,
            session.character.player_stand_state,
        )?,
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

pub(in crate::world) fn build_player_aura_update_body(
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

pub(in crate::world) fn set_player_aura_update_values(
    values: &mut [Option<u32>],
    active_auras: &[ActiveAura],
) -> anyhow::Result<()> {
    set_unit_aura_update_values(values, active_auras)?;
    set_update_value(
        values,
        PLAYER_TRACK_CREATURES,
        active_aura_track_creatures_mask(active_auras),
    )?;
    set_update_value(
        values,
        PLAYER_TRACK_RESOURCES,
        active_aura_track_resources_mask(active_auras),
    )?;
    Ok(())
}

pub(in crate::world) fn set_unit_aura_update_values(
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

pub(in crate::world) async fn handle_item_query_single(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    request: wow_proto::ItemQuerySingleRequest,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let item = request.item_id;
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

pub(in crate::world) async fn item_query_spell_cooldowns(
    world_db_pool: &MySqlPool,
    template: &wow_db::ItemTemplateQuery,
) -> anyhow::Result<[Option<ItemQuerySpellCooldown>; 5]> {
    let mut cooldowns = [None; 5];
    for (index, spell) in template.spells.iter().enumerate() {
        if spell.spell_id == 0 {
            continue;
        }
        let Some(spell_template) =
            wow_db::get_spell_template_query(world_db_pool, spell.spell_id).await?
        else {
            continue;
        };
        cooldowns[index] = Some(ItemQuerySpellCooldown {
            recovery_time: spell_template.recovery_time.min(i32::MAX as u32) as i32,
            category: spell_template.category,
            category_recovery_time: spell_template.category_recovery_time.min(i32::MAX as u32)
                as i32,
        });
    }
    Ok(cooldowns)
}

pub(in crate::world) async fn handle_item_name_query(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    request: wow_proto::ItemNameQueryRequest,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let item = request.item_id;
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

pub(in crate::world) async fn handle_page_text_query(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    request: wow_proto::PageTextQueryRequest,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let page_text = wow_db::get_page_text_query(world_db_pool, request.page_text_id).await?;
    let response = if let Some(page_text) = page_text {
        wow_proto::SmsgPageTextQueryResponse {
            page_text_id: page_text.id,
            text: page_text.text,
            next_page_text_id: page_text.next_page_text_id,
        }
    } else {
        warn!(
            page_text_id = request.page_text_id,
            item = format_args!("0x{:016X}", request.item_raw_guid),
            "Answering missing page text query with empty page"
        );
        wow_proto::SmsgPageTextQueryResponse {
            page_text_id: request.page_text_id,
            text: String::new(),
            next_page_text_id: 0,
        }
    };
    send_packet(
        stream,
        SMSG_PAGE_TEXT_QUERY_RESPONSE,
        &response.body(),
        Some(header_crypto),
    )
    .await
}
