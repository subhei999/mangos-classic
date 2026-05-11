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
    let spell_info = SpellInfo::from_template(&spell_template);
    let Some(mut prepared_spell) = spell_info.prepare_player_cast()
    else {
        warn!(
            spell_id = packet.spell_id,
            spell_name = spell_template.spell_name.as_str(),
            "Ignoring unsupported spell effect shape in starter spell slice"
        );
        return Ok(());
    };
    prepared_spell.prepare();
    let spell_profile = prepared_spell.profile;
    let cast_time_ms = spell_cast_time_millis(
        deps.shared_world
            .maps
            .spell_cast_time(spell_template.casting_time_index),
    );

    let targets = resolve_player_spell_cast_targets(
        deps.shared_world.maps,
        map_id,
        character_guid,
        normalize_spell_cast_targets(packet.targets, &spell_profile, caster),
        &spell_info,
        spell_profile.kind,
    )
    .await;
    let now = Instant::now();
    stand_player_for_spell_cast(stream, deps.shared_world, session, header_crypto).await?;
    if let Some(failure) = spell_cast_failure(
        deps.shared_world,
        session,
        &spell_template,
        &spell_profile,
        &targets,
        now,
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
    let spell_start_body = prepared_spell.spell_start_body(caster, cast_time_ms, &targets)?;
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
    if spell_profile.kind == SpellCastKind::NextMeleeSwing {
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
            let queued = QueuedNextMeleeSpell {
                spell_id: packet.spell_id,
                target,
                bonus_damage: spell_profile.bonus_damage,
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

    let Some(mut prepared_spell) = SpellInfo::from_template(&spell_template).prepare_item_cast(item_guid)
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
    let refreshable_consumable_regen = item_spell_profile.kind == SpellCastKind::AuraApplication
        && spell_periodic_regen_aura(&spell_info, now).is_some();
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
        &item_spell_profile,
        now,
        refreshable_consumable_regen,
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
async fn complete_item_use_spell_cast_by_id(
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
    let Some(character) = session.active_character.as_ref() else {
        return Ok(());
    };
    let Some(spell_template) = deps
        .shared_world
        .object_mgr
        .spell_template(deps.world_db_pool, spell_id)
        .await?
    else {
        warn!(spell_id, "Dropping pending item spell cast with no spell_template row");
        return Ok(());
    };
    let Some(mut prepared_spell) = SpellInfo::from_template(&spell_template).prepare_item_cast(item_guid)
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
async fn complete_item_use_spell_cast(
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
    let Some(character) = session.active_character.as_ref() else {
        return Ok(());
    };
    let character_guid = character.guid;
    let map_id = character.position.map_id;
    send_packet(
        stream,
        SMSG_CAST_RESULT,
        &build_cast_result_ok_body(spell_template.id),
        Some(&mut *header_crypto),
    )
    .await?;
    let spell_go_body = prepared_spell.spell_go_body(caster, &targets)?;
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
        &item_spell_profile,
        now,
        header_crypto,
    )
    .await?;
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
        }
    }

    fn into_spell_targets(self) -> SpellCastTargets {
        SpellCastTargets {
            target_mask: self.target_mask,
            unit_target: self.unit_target,
            gameobject_target: self.gameobject_target,
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn complete_pending_player_spell_cast(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.active_character.as_ref() else {
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
        return apply_player_spell_impact_by_id(
            stream,
            deps,
            session,
            event.spell_id,
            event.targets.into_spell_targets(),
            now,
            header_crypto,
        )
        .await;
    }
    Ok(())
}

async fn next_pending_player_spell_cast_due_at(
    maps: &MapRuntimeManager,
    session: &WorldSessionState,
) -> Option<Instant> {
    let character = session.active_character.as_ref()?;
    maps.next_pending_player_spell_cast_due_at(character.position.map_id, character.guid)
        .await
}

async fn pending_player_spell_cast_is_due(
    maps: &MapRuntimeManager,
    session: &WorldSessionState,
    now: Instant,
) -> bool {
    next_pending_player_spell_cast_due_at(maps, session)
        .await
        .is_some_and(|due_at| now >= due_at)
}

#[allow(clippy::too_many_arguments)]
async fn complete_player_spell_cast_by_id(
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
        warn!(spell_id, "Dropping pending spell cast with no spell_template row");
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
async fn complete_player_spell_cast(
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
    let Some(character) = session.active_character.as_ref() else {
        return Ok(());
    };
    let character_guid = character.guid;
    let character_level = character.level;
    let map_id = character.position.map_id;
    let caster = ObjectGuid::new(HighGuid::Player, 0, character_guid);

    if let Some(failure) = spell_target_cast_failure(
        deps.shared_world,
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
    sync_session_player_power_from_map(deps.shared_world.maps, session, map_id, character_guid).await;
    send_packet(
        stream,
        SMSG_CAST_RESULT,
        &build_cast_result_ok_body(prepared_spell.spell_id),
        Some(&mut *header_crypto),
    )
    .await?;
    let spell_go_body = prepared_spell.spell_go_body(caster, &targets)?;
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
        now,
        header_crypto,
    )
    .await;
    prepared_spell.finish();
    result
}

#[allow(clippy::too_many_arguments)]
async fn apply_player_spell_impact_by_id(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    spell_id: u32,
    targets: SpellCastTargets,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.active_character.as_ref() else {
        return Ok(());
    };
    let Some(spell_template) = deps
        .shared_world
        .object_mgr
        .spell_template(deps.world_db_pool, spell_id)
        .await?
    else {
        warn!(spell_id, "Dropping pending spell impact with no spell_template row");
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
        now,
        header_crypto,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn apply_player_spell_impact(
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
        now,
        header_crypto,
    )
    .await
}

async fn stand_player_for_spell_cast(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let has_standing_cancel_aura = session
        .active_auras
        .iter()
        .any(|aura| active_aura_interrupt_flags(aura) & AURA_INTERRUPT_FLAG_STANDING_CANCELS != 0);
    if session.player_stand_state == PLAYER_STAND_STATE_STAND && !has_standing_cancel_aura {
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
    session.player_stand_state = PLAYER_STAND_STATE_STAND;
    let Some(character) = session.active_character.as_ref() else {
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

async fn cancel_pending_player_spell_cast(
    stream: &mut WorldPacketSink,
    maps: &MapRuntimeManager,
    sessions: &SessionRegistry,
    session: &mut WorldSessionState,
    failure: u8,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    let Some(character) = session.active_character.as_ref() else {
        return Ok(false);
    };
    let map_id = character.position.map_id;
    let character_guid = character.guid;
    let Some(active_cast) = maps
        .cancel_active_player_spell_cast(map_id, character_guid)
        .await
    else {
        return Ok(false);
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

async fn broadcast_spell_interrupt_to_observers(
    maps: &MapRuntimeManager,
    sessions: &SessionRegistry,
    session: &WorldSessionState,
    caster: ObjectGuid,
    spell_id: u32,
    failure: u8,
) -> anyhow::Result<()> {
    let Some(character) = session.active_character.as_ref() else {
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

async fn send_spell_cast_failure(
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
struct SpellCastDeps<'a> {
    character_db_pool: &'a MySqlPool,
    world_db_pool: &'a MySqlPool,
    account_id: u32,
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

async fn spell_melee_cast_failure(
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

async fn spell_target_is_behind_victim(
    shared_world: SharedWorldDeps<'_>,
    session: &WorldSessionState,
    target: ObjectGuid,
) -> bool {
    let Some(character) = session.active_character.as_ref() else {
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

async fn spell_charge_cast_failure(
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

async fn spell_cast_failure(
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    spell_template: &wow_db::SpellTemplateQuery,
    spell_profile: &SpellCastProfile,
    targets: &SpellCastTargets,
    now: Instant,
) -> Option<u8> {
    if session.player_death_state != PlayerDeathState::Alive {
        return Some(SPELL_FAILED_CASTER_DEAD);
    }
    if let Some(character) = session.active_character.as_ref() {
        if session.player_health == 0
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
    spell_target_cast_failure(shared_world, session, spell_template, spell_profile, targets).await
}

async fn spell_target_cast_failure(
    shared_world: SharedWorldDeps<'_>,
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

async fn spell_combo_point_cast_failure(
    shared_world: SharedWorldDeps<'_>,
    session: &WorldSessionState,
    spell_profile: &SpellCastProfile,
    targets: &SpellCastTargets,
) -> Option<u8> {
    if !spell_profile.needs_combo_points {
        return None;
    }
    let target = targets.unit_target?;
    let character = session.active_character.as_ref()?;
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

async fn spell_heal_cast_failure(
    shared_world: SharedWorldDeps<'_>,
    session: &WorldSessionState,
    spell_template: &wow_db::SpellTemplateQuery,
    targets: &SpellCastTargets,
) -> Option<u8> {
    let character = session.active_character.as_ref()?;
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

async fn spell_unit_target_cast_failure(
    shared_world: SharedWorldDeps<'_>,
    session: &WorldSessionState,
    spell_template: &wow_db::SpellTemplateQuery,
    spell_profile: &SpellCastProfile,
    targets: &SpellCastTargets,
) -> Option<u8> {
    let target_kind = SpellInfo::from_template(spell_template).unit_target_kind(spell_profile.kind);
    if target_kind.requires_unit_target() && targets.unit_target.is_none() {
        return Some(SPELL_FAILED_OUT_OF_RANGE);
    }
    if target_kind != SpellTargetKind::HostileUnit
        || spell_profile.requires_melee
        || matches!(spell_profile.kind, SpellCastKind::NextMeleeSwing | SpellCastKind::Charge)
    {
        return None;
    }
    let character = session.active_character.as_ref()?;
    let target = targets.unit_target?;
    if !target.is_creature() {
        return Some(SPELL_FAILED_OUT_OF_RANGE);
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
            &session.db_creature_navigation,
            range,
        )
        .await;
    match validation.check {
        PlayerSpellTargetCheck::Clear => None,
        PlayerSpellTargetCheck::BadFacing => Some(SPELL_FAILED_UNIT_NOT_INFRONT),
        PlayerSpellTargetCheck::NavigationBlocked(DbCreatureNavigationResult::LineOfSightBlocked) => {
            Some(SPELL_FAILED_LINE_OF_SIGHT)
        }
        PlayerSpellTargetCheck::NoActiveCharacter
        | PlayerSpellTargetCheck::MissingTarget
        | PlayerSpellTargetCheck::TargetNotAlive
        | PlayerSpellTargetCheck::NavigationBlocked(_)
        | PlayerSpellTargetCheck::OutOfRange => Some(SPELL_FAILED_OUT_OF_RANGE),
    }
}

async fn resolve_player_spell_cast_targets(
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
            targets.target_mask = (targets.target_mask | SPELL_CAST_TARGET_UNIT)
                & !SPELL_CAST_TARGET_UNIT_ENEMY;
            targets.unit_target = Some(selected_target);
        }
    }
    targets
}

fn spell_blocks_mana_regen(template: &wow_db::SpellTemplateQuery) -> bool {
    template.power_type == POWER_TYPE_MANA
        && template.mana_cost > 0
        && (template.attributes_ex2 & SPELL_ATTR_EX2_DONT_BLOCK_MANA_REGEN) == 0
}

async fn sync_session_player_power_from_map(
    maps: &MapRuntimeManager,
    session: &mut WorldSessionState,
    map_id: u32,
    character_guid: u32,
) {
    if let Some(snapshot) = maps.player_runtime_snapshot(map_id, character_guid).await {
        session.player_mana = snapshot.power1;
        session.player_rage = snapshot.power2;
        session.player_energy = snapshot.power4;
    }
}

fn spell_cast_time_millis(cast_time: Option<SpellCastTimeEntry>) -> u32 {
    let Some(cast_time) = cast_time else {
        return 0;
    };
    cast_time
        .cast_time_millis
        .max(cast_time.min_cast_time_millis)
        .max(0) as u32
}

async fn spell_travel_delay_millis(
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
    let Some(character) = session.active_character.as_ref() else {
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
struct SpellCastProfile {
    spell_id: u32,
    kind: SpellCastKind,
    aura_target: SpellAuraTarget,
    bonus_damage: u32,
    weapon_damage_percent: u32,
    damage: u32,
    power: SpellPowerCost,
    requires_melee: bool,
    requires_behind: bool,
    needs_combo_points: bool,
    global_cooldown_category: u32,
    global_cooldown_millis: u64,
    cooldown_millis: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpellCastKind {
    InstantDamage,
    DirectHeal,
    AuraApplication,
    Charge,
    NextMeleeSwing,
    Teleport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpellAuraTarget {
    Caster,
    UnitTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpellTargetKind {
    Caster,
    Unit,
    HostileUnit,
    FriendlyUnit,
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
enum SpellPowerCost {
    Rage { cost: u32 },
    Mana { cost: u32 },
    Energy { cost: u32 },
}

const SPELL_ATTR_ON_NEXT_SWING_NO_DAMAGE: u32 = 0x0000_0004;
const SPELL_ATTR_PASSIVE: u32 = 0x0000_0040;
const SPELL_ATTR_ON_NEXT_SWING: u32 = 0x0000_0400;
const SPELL_INTERRUPT_FLAG_DAMAGE_PUSHBACK: u32 = 0x02;
const SPELL_ATTR_EX_FINISHING_MOVE_DAMAGE: u32 = 0x0010_0000;
const SPELL_ATTR_EX_FINISHING_MOVE_DURATION: u32 = 0x0040_0000;
const SPELL_EFFECT_WEAPON_DAMAGE_NOSCHOOL: u32 = 17;
const SPELL_EFFECT_WEAPON_PERCENT_DAMAGE: u32 = 31;
const SPELL_EFFECT_WEAPON_DAMAGE: u32 = 58;
const SPELL_EFFECT_ADD_COMBO_POINTS: u32 = 80;
const SPELL_EFFECT_NORMALIZED_WEAPON_DMG: u32 = 121;
const SPELL_EFFECT_APPLY_AURA: u32 = 6;
const SPELL_EFFECT_TELEPORT_UNITS: u32 = 5;
const SPELL_EFFECT_TELEPORT_UNITS_FACE_CASTER: u32 = 43;
const SPELL_EFFECT_HEAL: u32 = 10;
const SPELL_EFFECT_ENERGIZE: u32 = 30;
const SPELL_EFFECT_CHARGE: u32 = 96;
const SPELL_AURA_PERIODIC_DAMAGE: u32 = 3;
const SPELL_AURA_PERIODIC_HEAL: u32 = 8;
const SPELL_AURA_OBS_MOD_HEALTH: u32 = 20;
const SPELL_AURA_PERIODIC_ENERGIZE: u32 = 24;
const SPELL_AURA_MOD_STAT: u32 = 29;
const SPELL_AURA_MOD_RESISTANCE: u32 = 22;
const SPELL_AURA_MOD_DECREASE_SPEED: u32 = 33;
const SPELL_AURA_PROC_TRIGGER_SPELL: u32 = 42;
const SPELL_AURA_MOD_SKILL_TALENT: u32 = 98;
const SPELL_AURA_MOD_SKILL: u32 = 30;
const SPELL_AURA_MOD_REGEN: u32 = 84;
const SPELL_AURA_MOD_POWER_REGEN: u32 = 85;
const SPELL_AURA_MOD_ATTACK_POWER: u32 = 99;
const SPELL_AURA_MOD_TOTAL_STAT_PERCENTAGE: u32 = 137;
const SPELL_AURA_MOD_MELEE_HASTE: u32 = 138;
const SPELL_AURA_MOD_REPUTATION_GAIN: u32 = 156;
const AURA_INTERRUPT_FLAG_DAMAGE: u32 = 0x0000_0002;
const AURA_INTERRUPT_FLAG_MOVING: u32 = 0x0000_0008;
const AURA_INTERRUPT_FLAG_STANDING_CANCELS: u32 = 0x0004_0000;
const PLAYER_STAND_STATE_STAND: u8 = 0;
const PLAYER_STAND_STATE_SIT: u8 = 1;
const PLAYER_STAND_STATE_SLEEP: u8 = 3;
const PLAYER_STAND_STATE_KNEEL: u8 = 8;
const POWER_TYPE_MANA: u32 = 0;
const POWER_TYPE_RAGE: u32 = 1;
const POWER_TYPE_ENERGY: u32 = 3;
const POSITIVE_AURA_FLAGS: u32 = 0x05;
const NEGATIVE_AURA_FLAGS: u32 = 0x08;
const TARGET_UNIT_CASTER: u32 = 1;
const TARGET_UNIT_ENEMY: u32 = 6;
const TARGET_UNIT: u32 = 25;
const PROC_FLAG_TAKE_MELEE_SWING: u32 = 0x0000_0008;
const ITEM_SPELLTRIGGER_ON_USE: u32 = 0;
const ITEM_SPELLTRIGGER_ON_NO_DELAY_USE: u32 = 5;
const SPELL_ATTR_EX2_DONT_BLOCK_MANA_REGEN: u32 = 0x0200_0000;
const SPELL_ATTR_SS_FACING_BACK: u32 = 0x0000_0008;
const SPELL_RANGE_FLAG_RANGED: u32 = 0x2;
const SPELL_CAST_ARC_RADIANS: f32 = std::f32::consts::PI;
const BASE_CHARGE_SPEED: f32 = 27.0;
const MAX_AURA_SLOTS: usize = 48;
const MAX_POSITIVE_AURA_SLOTS: usize = 32;
const MAX_AURA_FLAG_FIELDS: usize = 6;

fn spell_damage_pushback_delay_millis(pushback_count: u8) -> u32 {
    match pushback_count {
        0 => 1000,
        1 => 800,
        2 => 600,
        3 => 400,
        _ => 200,
    }
}
const MAX_AURA_LEVEL_FIELDS: usize = 12;

include!("spells/effects.rs");
include!("spells/spell.rs");
include!("spells/spell_mgr.rs");
include!("spells/targets.rs");
include!("spells/auras.rs");
include!("spells/cooldowns.rs");
include!("spells/packets.rs");

#[cfg(test)]
fn player_spell_cast_profile(template: &wow_db::SpellTemplateQuery) -> Option<SpellCastProfile> {
    SpellInfo::from_template(template)
        .prepare_player_cast()
        .map(|prepared| prepared.profile)
}

#[cfg(test)]
fn item_use_spell_cast_profile(template: &wow_db::SpellTemplateQuery) -> Option<SpellCastProfile> {
    SpellInfo::from_template(template)
        .prepare_item_cast(ObjectGuid::EMPTY)
        .map(|prepared| prepared.profile)
}

fn spell_has_aura_application(template: &wow_db::SpellTemplateQuery) -> bool {
    SpellInfo::from_template(template)
        .effects
        .iter()
        .any(|effect| effect.dispatch == SpellEffectDispatch::ApplyAura)
}

fn spell_effect_simple_value(base_points: i32) -> Option<u32> {
    (base_points >= 0).then_some((base_points + 1) as u32)
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
    let spell_info = SpellInfo::from_template(template);
    ActiveAura {
        spell_id: template.id,
        caster,
        level,
        positive: active_aura_is_positive(&spell_info),
        visible: true,
        duration_millis: (duration_millis > 0).then_some(duration_millis as u32),
        expires_at: (duration_millis > 0)
            .then_some(now + Duration::from_millis(duration_millis as u64)),
        periodic_damage: spell_periodic_damage_aura(&spell_info, now),
        periodic_regen: spell_periodic_regen_aura(&spell_info, now),
        stat_modifiers: spell_aura_stat_modifiers(&spell_info),
        proc_triggers: spell_aura_proc_triggers(&spell_info),
    }
}

fn active_aura_is_positive(spell_info: &SpellInfo<'_>) -> bool {
    !spell_info
        .effects
        .iter()
        .any(|effect| {
            effect.dispatch == SpellEffectDispatch::ApplyAura
                && (effect.implicit_target_a == TARGET_UNIT_ENEMY
                    || effect.aura_name == SPELL_AURA_PERIODIC_DAMAGE)
        })
}

fn spell_periodic_damage_aura(spell_info: &SpellInfo<'_>, now: Instant) -> Option<PeriodicDamageAura> {
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
            let damage = spell_effect_simple_value(effect.base_points)?;
            Some(PeriodicDamageAura {
                aura_name: effect.aura_name,
                school: spell_info.template.school,
                damage_class: spell_info.template.dmg_class,
                amount: damage,
                tick_millis: effect.amplitude,
                next_tick_at: now + Duration::from_millis(effect.amplitude as u64),
            })
        })
}

fn spell_periodic_regen_aura(spell_info: &SpellInfo<'_>, now: Instant) -> Option<PeriodicRegenAura> {
    let mut health_amount = 0u32;
    let mut mana_amount = 0u32;
    let mut tick_millis = 0u32;
    for effect in spell_info.effects {
        if effect.dispatch != SpellEffectDispatch::ApplyAura {
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

fn spell_aura_proc_triggers(spell_info: &SpellInfo<'_>) -> Vec<AuraProcTrigger> {
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

fn active_aura_proc_trigger_spell_ids(
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

fn aura_proc_roll_succeeds(proc_chance: u32) -> bool {
    if proc_chance >= 100 {
        return true;
    }
    if proc_chance == 0 {
        return false;
    }
    rand::thread_rng().gen_range(1..=10_000) <= proc_chance.saturating_mul(100).min(10_000)
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

fn spell_aura_stat_modifiers(spell_info: &SpellInfo<'_>) -> Vec<AuraStatModifier> {
    spell_info
        .effects
        .into_iter()
        .filter(|effect| effect.dispatch == SpellEffectDispatch::ApplyAura)
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
            SPELL_AURA_MOD_DECREASE_SPEED => Some(AuraStatModifier::MoveSpeedPercent {
                percent: spell_effect_simple_i32(effect.base_points),
            }),
            SPELL_AURA_MOD_MELEE_HASTE => Some(AuraStatModifier::MeleeAttackTimePercent {
                percent: spell_effect_simple_i32(effect.base_points),
            }),
            SPELL_AURA_MOD_RESISTANCE => Some(AuraStatModifier::Resistance {
                school_mask: u32::try_from(effect.misc_value).ok()?,
                amount: spell_effect_simple_i32(effect.base_points),
            }),
            SPELL_AURA_MOD_STAT => {
                let stat = usize::try_from(effect.misc_value).ok();
                Some(AuraStatModifier::Stat {
                    stat: stat.filter(|stat| *stat < MAX_STATS),
                    amount: spell_effect_simple_i32(effect.base_points),
                })
            }
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

fn active_aura_movement_speed_multiplier(active_auras: &[ActiveAura]) -> f32 {
    let strongest_slow = active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .filter_map(|modifier| match modifier {
            AuraStatModifier::MoveSpeedPercent { percent } if *percent < 0 => Some(*percent),
            _ => None,
        })
        .min()
        .unwrap_or(0);

    (100 + strongest_slow).clamp(0, 100) as f32 / 100.0
}

fn active_aura_melee_attack_time_multiplier(active_auras: &[ActiveAura]) -> f32 {
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

fn player_world_stats_with_active_auras(
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

fn apply_flat_modifier(value: u32, amount: i32) -> u32 {
    (value as i64)
        .saturating_add(i64::from(amount))
        .clamp(0, u32::MAX as i64) as u32
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

