use super::*;
use wow_proto::world::WorldOpcode;
use wow_proto::SpellCastTargets;

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
        WorldOpcode::SmsgSpellStart as u16,
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
                opcode: WorldOpcode::SmsgSpellStart as u16,
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
    let (item_spell_profile, item_cooldown) =
        item_spell_cast_profile_with_cooldown(prepared_spell.profile, item_spell, &spell_template);
    prepared_spell.profile = item_spell_profile;
    let cast_time_ms = spell_cast_time_millis(
        deps.shared_world
            .maps
            .spell_cast_time(spell_template.casting_time_index),
    );

    let now = Instant::now();
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
    if !refreshable_consumable_regen {
        stand_player_for_spell_cast(stream, deps.shared_world, session, header_crypto).await?;
    }
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
            WorldOpcode::SmsgCastResult as u16,
            &build_cast_result_failure_body(spell_template.id, failure),
            Some(header_crypto),
        )
        .await?;
        send_packet(
            stream,
            WorldOpcode::SmsgSpellFailure as u16,
            &build_spell_failure_body(caster, spell_template.id, failure)?,
            Some(&mut *header_crypto),
        )
        .await?;
        return send_packet(
            stream,
            WorldOpcode::SmsgSpellFailedOther as u16,
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
        item_cooldown.category,
        item_cooldown.category_recovery_millis,
    )
    .await;
    if let Some(snapshot) = deps
        .shared_world
        .maps
        .player_runtime_snapshot(map_id, character_guid)
        .await
    {
        session.character.spell_global_cooldowns_until = snapshot.spell_global_cooldowns_until;
        session.character.spell_cooldowns_until = snapshot.spell_cooldowns_until;
        session.character.spell_cooldown_categories = snapshot.spell_cooldown_categories;
        session.character.spell_cooldown_item_ids = snapshot.spell_cooldown_item_ids;
    }
    let spell_start_body = prepared_spell.spell_start_body(caster, cast_time_ms, &targets)?;
    send_packet(
        stream,
        WorldOpcode::SmsgSpellStart as u16,
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
                        opcode: WorldOpcode::SmsgSpellStart as u16,
                        body: spell_start_body,
                    },
                )
                .await,
        )
        .await;
    if cast_time_ms > 0 {
        retime_player_auto_attack_after_spell_cast(
            deps,
            session,
            &spell_template,
            &item_spell_profile,
            map_id,
            character_guid,
            now,
        )
        .await?;
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
        WorldOpcode::SmsgCastResult as u16,
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
        WorldOpcode::SmsgSpellGo as u16,
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
                        opcode: WorldOpcode::SmsgSpellGo as u16,
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
        WorldOpcode::SmsgCastResult as u16,
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
        WorldOpcode::SmsgSpellGo as u16,
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
                opcode: WorldOpcode::SmsgSpellGo as u16,
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
        WorldOpcode::SmsgUpdateObject as u16,
        &build_player_stand_state_update_body(character, PLAYER_STAND_STATE_STAND)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgStandStateUpdate as u16,
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
        opcode: WorldOpcode::SmsgSpellFailure as u16,
        body: build_spell_failure_body(caster, spell_id, failure)?,
    };
    let failed_other_packet = OutboundWorldPacket {
        opcode: WorldOpcode::SmsgSpellFailedOther as u16,
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
        WorldOpcode::SmsgCastResult as u16,
        &build_cast_result_failure_body(spell_id, failure),
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgSpellFailure as u16,
        &build_spell_failure_body(caster, spell_id, failure)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgSpellFailedOther as u16,
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
        WorldOpcode::SmsgSpellStart as u16,
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
                opcode: WorldOpcode::SmsgSpellStart as u16,
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
        WorldOpcode::SmsgCastResult as u16,
        &build_cast_result_ok_body(spell_id),
        Some(&mut *header_crypto),
    )
    .await?;
    let spell_go_body = build_spell_go_body(caster, spell_id, &targets)?;
    send_packet(
        stream,
        WorldOpcode::SmsgSpellGo as u16,
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
                opcode: WorldOpcode::SmsgSpellGo as u16,
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
    send_packet(
        stream,
        WorldOpcode::SmsgLootResponse as u16,
        &response,
        Some(header_crypto),
    )
    .await
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
        cooldown_category: 0,
        category_cooldown_millis: 0,
        cooldown_millis: 0,
    }
}
