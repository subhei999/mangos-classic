use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct CreateItemSpellEffect {
    pub(in crate::world) item_template: u32,
    pub(in crate::world) requested_count: u32,
}

pub(in crate::world) fn create_item_spell_effect(
    effect: SpellInfoEffect,
    value_context: SpellEffectValueContext,
) -> Option<CreateItemSpellEffect> {
    if effect.dispatch != SpellEffectDispatch::CreateItem || effect.item_type == 0 {
        return None;
    }
    Some(CreateItemSpellEffect {
        item_template: effect.item_type,
        requested_count: spell_effect_calculated_u32(effect, value_context)
            .unwrap_or(1)
            .max(1),
    })
}

pub(in crate::world) fn create_item_spell_effects(
    spell_info: &SpellInfo<'_>,
    value_context: SpellEffectValueContext,
) -> Vec<CreateItemSpellEffect> {
    spell_info
        .effects
        .into_iter()
        .filter_map(|effect| create_item_spell_effect(effect, value_context))
        .collect()
}

pub(in crate::world) fn create_item_count_for_template(
    effect: CreateItemSpellEffect,
    template: &ItemTemplateQuery,
) -> u32 {
    effect.requested_count.min(template.stackable.max(1)).max(1)
}

pub(in crate::world) async fn player_create_item_cast_inventory_failure(
    deps: SpellCastDeps<'_>,
    session: &WorldSessionState,
    spell_template: &wow_db::SpellTemplateQuery,
) -> anyhow::Result<Option<u8>> {
    let spell_info = SpellInfo::from_template(spell_template);
    let value_context = player_spell_effect_value_context(
        deps.shared_world.maps,
        spell_template,
        &session.character.character_skills,
        0,
    );
    let effects = create_item_spell_effects(&spell_info, value_context);
    if effects.is_empty() {
        return Ok(None);
    }
    let bag_model =
        InventoryBagModel::load_inventory(deps.world_db_pool, &session.inventory.items).await?;
    for effect in effects {
        let Some(template) =
            wow_db::get_item_template_query(deps.world_db_pool, effect.item_template).await?
        else {
            warn!(
                spell_id = spell_template.id,
                item_template = effect.item_template,
                "Create-item spell references missing item_template row"
            );
            return Ok(Some(EQUIP_ERR_ITEM_NOT_FOUND));
        };
        let count = create_item_count_for_template(effect, &template);
        if bag_model
            .plan_store_item(
                InventoryStorageScope::Inventory,
                &session.inventory.items,
                &template,
                count,
                None,
                None,
            )
            .is_none()
        {
            return Ok(Some(EQUIP_ERR_INVENTORY_FULL));
        }
    }
    Ok(None)
}

pub(in crate::world) async fn apply_player_create_item_effects(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    character_guid: u32,
    spell_info: &SpellInfo<'_>,
    value_context: SpellEffectValueContext,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let effects = create_item_spell_effects(spell_info, value_context);
    if effects.is_empty() {
        return Ok(());
    }
    let bag_model =
        InventoryBagModel::load_inventory(deps.world_db_pool, &session.inventory.items).await?;
    let mut update_blocks = Vec::new();
    let mut push_results = Vec::new();

    for effect in effects {
        let Some(template) =
            wow_db::get_item_template_query(deps.world_db_pool, effect.item_template).await?
        else {
            warn!(
                spell_id = spell_info.template.id,
                item_template = effect.item_template,
                "Skipping create-item spell effect with missing item_template row"
            );
            send_inventory_change_failure(
                stream,
                EQUIP_ERR_ITEM_NOT_FOUND,
                None,
                None,
                header_crypto,
            )
            .await?;
            return Ok(());
        };
        let count = create_item_count_for_template(effect, &template);
        let Some(store_plan) = bag_model.plan_store_item(
            InventoryStorageScope::Inventory,
            &session.inventory.items,
            &template,
            count,
            None,
            None,
        ) else {
            send_inventory_change_failure(
                stream,
                EQUIP_ERR_INVENTORY_FULL,
                None,
                None,
                header_crypto,
            )
            .await?;
            return Ok(());
        };

        let random_properties = generate_item_instance_random_properties_for_template(
            deps.world_db_pool,
            &session.movement.db_creature_navigation.world_data_files,
            &template,
        )
        .await?;
        for slot in &store_plan {
            if let Some(item_guid) = slot.existing_item {
                let existing_count = session
                    .inventory
                    .items
                    .iter()
                    .find(|item| item.item == item_guid)
                    .map(|item| item.count)
                    .unwrap_or(0);
                wow_db::update_character_inventory_item_count(
                    deps.character_db_pool,
                    character_guid,
                    item_guid,
                    existing_count.saturating_add(slot.count),
                )
                .await?;
            } else {
                wow_db::add_character_inventory_item_with_random_properties(
                    deps.character_db_pool,
                    wow_db::AddCharacterInventoryItemRequest {
                        guid: character_guid,
                        bag: slot.bag as u32,
                        slot: slot.slot,
                        item_template: template.entry,
                        count: slot.count,
                        durability: template.max_durability,
                        initial_flags: item_binding_flags_on_pickup(&template),
                        random_properties: random_properties.as_ref(),
                    },
                )
                .await?;
            }
        }

        session.inventory.items =
            wow_db::get_character_inventory_items(deps.character_db_pool, character_guid).await?;
        for slot in &store_plan {
            if let Some(item_guid) = slot.existing_item {
                if let Some(item) = session
                    .inventory
                    .items
                    .iter()
                    .find(|item| item.item == item_guid)
                {
                    update_blocks.push(build_item_stack_count_update_block(item.item, item.count)?);
                    push_results.push(build_item_push_result_body(
                        character_guid,
                        item,
                        slot.count,
                        true,
                        true,
                        true,
                    ));
                }
                continue;
            }
            if let Some(new_item) = session
                .inventory
                .items
                .iter()
                .find(|item| item.bag == slot.bag as u32 && item.slot == slot.slot)
            {
                update_blocks.extend(build_stored_item_create_update_blocks(
                    character_guid,
                    &session.inventory.items,
                    new_item,
                    (template.container_slots > 0).then_some(template.container_slots),
                )?);
                push_results.push(build_item_push_result_body(
                    character_guid,
                    new_item,
                    slot.count,
                    true,
                    true,
                    true,
                ));
            }
        }
    }

    for body in push_results {
        send_packet(
            stream,
            WorldOpcode::SmsgItemPushResult as u16,
            &body,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    if !update_blocks.is_empty() {
        let body = build_update_object_body(&update_blocks);
        send_packet(
            stream,
            WorldOpcode::SmsgUpdateObject as u16,
            &body,
            Some(header_crypto),
        )
        .await?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_item_use_spell_effects(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    spell_template: &wow_db::SpellTemplateQuery,
    item_spell: &SpellCastProfile,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
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
    let effective_world_stats =
        player_world_stats_with_active_auras(world_stats, &session.auras.active_auras);
    let max_mana = effective_world_stats.max_mana();

    let mut direct_heal_applied = false;
    let mut direct_energize_applied = false;
    let mut aura_applied = false;
    let value_context = player_spell_effect_value_context(
        deps.shared_world.maps,
        spell_template,
        &session.character.character_skills,
        0,
    );
    for effect in spell_info.effects {
        match effect.dispatch {
            SpellEffectDispatch::Heal if !direct_heal_applied => {
                let heal = spell_direct_heal(&spell_info, value_context);
                if heal != 0 {
                    let event = deps
                        .shared_world
                        .maps
                        .apply_player_heal(map_id, character_guid, heal)
                        .await?;
                    if let Some(event) = event {
                        session.character.player_health = event.health;
                        let log = build_spell_heal_log_body(
                            caster,
                            caster,
                            spell_template.id,
                            event.amount_healed,
                            false,
                        )?;
                        send_packet(
                            stream,
                            WorldOpcode::SmsgSpellHealLog as u16,
                            &log,
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
                }
                direct_heal_applied = true;
            }
            SpellEffectDispatch::Energize if !direct_energize_applied => {
                let energize = spell_direct_energize(&spell_info, value_context);
                if energize != 0 && max_mana != 0 {
                    let old_mana = session.character.player_mana;
                    session.character.player_mana = session
                        .character
                        .player_mana
                        .saturating_add(energize)
                        .min(max_mana);
                    let amount_energized = session.character.player_mana.saturating_sub(old_mana);
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
                            WorldOpcode::SmsgSpellEnergizeLog as u16,
                            &log,
                            Some(&mut *header_crypto),
                        )
                        .await?;
                    }
                    let observer_packets = deps
                        .shared_world
                        .maps
                        .update_player_power1(map_id, character_guid, session.character.player_mana)
                        .await?;
                    deps.shared_world.sessions.dispatch(observer_packets).await;
                    update_bodies.push(build_player_mana_update_body(
                        caster,
                        session.character.player_mana,
                    )?);
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
                    value_context,
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
        send_packet(
            stream,
            WorldOpcode::SmsgUpdateObject as u16,
            &body,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    deps.shared_world
        .maps
        .sync_player_gameplay_state(map_id, character_guid, session)
        .await;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_item_aura_effect(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    character_guid: u32,
    character_level: u8,
    map_id: u32,
    spell_template: &wow_db::SpellTemplateQuery,
    character_snapshot: &Player,
    value_context: SpellEffectValueContext,
    now: Instant,
    update_bodies: &mut Vec<Vec<u8>>,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let aura = build_active_aura(
        spell_template,
        caster,
        character_level,
        value_context,
        now,
        deps.shared_world
            .maps
            .spell_duration(spell_template.duration_index),
    );
    let mut aura = aura;
    mark_active_aura_periodic_regen_as_consumable(&mut aura);
    let makes_player_sit = aura
        .periodic_regen
        .is_some_and(|regen| regen.makes_player_sit);
    apply_player_aura(session, aura.clone());
    if makes_player_sit {
        session.character.player_stand_state = PLAYER_STAND_STATE_SIT;
        update_bodies.push(build_player_stand_state_update_body(
            character_snapshot,
            session.character.player_stand_state,
        )?);
    }
    if let Some(event) = deps
        .shared_world
        .maps
        .apply_player_aura(map_id, character_guid, aura)
        .await?
    {
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
    } else {
        update_bodies.push(build_player_aura_update_body(
            caster,
            character_snapshot.class,
            session.character.player_stand_state,
            deps.shared_world
                .maps
                .player_runtime_snapshot(map_id, character_guid)
                .await
                .map(|snapshot| snapshot.aura_state)
                .unwrap_or(0),
            &session.auras.active_auras,
        )?);
        for packet in build_player_aura_duration_update_packets(&session.auras.active_auras, now) {
            send_packet(
                stream,
                packet.opcode,
                &packet.body,
                Some(&mut *header_crypto),
            )
            .await?;
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
                    opcode: WorldOpcode::SmsgUpdateObject as u16,
                    body: build_player_stand_state_update_body(
                        character_snapshot,
                        session.character.player_stand_state,
                    )?,
                },
            )
            .await;
        deps.shared_world.sessions.dispatch(observer_packets).await;
    }
    Ok(())
}
