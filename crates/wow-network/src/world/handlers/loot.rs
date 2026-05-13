use super::*;

pub(in crate::world) async fn dispatch_loot_packet(
    ctx: &mut WorldPacketDispatchContext<'_>,
    packet: &packets::ParsedWorldClientPacket,
) -> anyhow::Result<()> {
    match packet {
        packets::ParsedWorldClientPacket::LootRoll(_) => {
            handle_loot_roll(
                &mut *ctx.stream,
                LootMutationDeps {
                    character_db_pool: ctx.character_db_pool,
                    world_db_pool: ctx.world_db_pool,
                    shared_world: SharedWorldDeps {
                        object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                        maps: &ctx.runtime_state.maps,
                        sessions: &ctx.runtime_state.sessions,
                    },
                    parties: ctx.runtime_state.parties.as_ref(),
                },
                packet.loot_roll()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::LootMasterGive(_) => {
            handle_loot_master_give(
                &mut *ctx.stream,
                LootMutationDeps {
                    character_db_pool: ctx.character_db_pool,
                    world_db_pool: ctx.world_db_pool,
                    shared_world: SharedWorldDeps {
                        object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                        maps: &ctx.runtime_state.maps,
                        sessions: &ctx.runtime_state.sessions,
                    },
                    parties: ctx.runtime_state.parties.as_ref(),
                },
                packet.loot_master_give()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::Loot(_) => {
            handle_loot(
                &mut *ctx.stream,
                ctx.world_db_pool,
                SharedWorldDeps {
                    object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                    maps: &ctx.runtime_state.maps,
                    sessions: &ctx.runtime_state.sessions,
                },
                &ctx.runtime_state.parties,
                packet.loot()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::AutostoreLootItem(_) => {
            handle_autostore_loot_item(
                &mut *ctx.stream,
                LootMutationDeps {
                    character_db_pool: ctx.character_db_pool,
                    world_db_pool: ctx.world_db_pool,
                    shared_world: SharedWorldDeps {
                        object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                        maps: &ctx.runtime_state.maps,
                        sessions: &ctx.runtime_state.sessions,
                    },
                    parties: &ctx.runtime_state.parties,
                },
                packet.autostore_loot_item()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::LootMoney(_) => {
            let _ = packet.loot_money()?;
            handle_loot_money(
                &mut *ctx.stream,
                ctx.character_db_pool,
                SharedWorldDeps {
                    object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                    maps: &ctx.runtime_state.maps,
                    sessions: &ctx.runtime_state.sessions,
                },
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::LootRelease(_) => {
            handle_loot_release(
                &mut *ctx.stream,
                SharedWorldDeps {
                    object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                    maps: &ctx.runtime_state.maps,
                    sessions: &ctx.runtime_state.sessions,
                },
                packet.loot_release()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        other => anyhow::bail!("loot router received opcode 0x{:04X}", other.opcode()),
    }
}

pub(in crate::world) async fn handle_loot(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    shared_world: SharedWorldDeps<'_>,
    parties: &PartyManager,
    request: wow_proto::LootRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let target = ObjectGuid::from_raw(request.raw_guid);
    let Some(character) = session.character.active_character.as_ref() else {
        warn!("Ignoring loot request before character login");
        return Ok(());
    };
    if target.is_game_object() {
        let Some(gameobject) = shared_world
            .maps
            .db_gameobject_snapshot(character.position.map_id, target)
            .await
        else {
            warn!(
                target = format_args!("0x{:016X}", target.raw()),
                "Ignoring loot request for unknown gameobject"
            );
            return Ok(());
        };
        if gameobject.spawn.map != character.position.map_id
            || !is_position_inside_radius(gameobject.position(), character.position, 8.0)
        {
            warn!("Ignoring gameobject loot request outside interaction range");
            return Ok(());
        }
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
                character.position.map_id,
                target.raw(),
                character.guid,
                loot_items,
            )
            .await
        else {
            warn!("Ignoring loot request for unavailable gameobject");
            return Ok(());
        };
        let _ = gameobject;
        send_player_looting_state_update(stream, shared_world, session, true, &mut *header_crypto)
            .await?;
        let response = build_gameobject_loot_response_body(target, &loot_items);
        return send_packet(stream, SMSG_LOOT_RESPONSE, &response, Some(header_crypto)).await;
    }
    let Some(creature) = shared_world
        .maps
        .db_creature_snapshot(character.position.map_id, target)
        .await
    else {
        warn!(
            target = format_args!("0x{:016X}", target.raw()),
            "Ignoring loot request for unknown target"
        );
        return Ok(());
    };
    if !creature.lootable {
        warn!("Ignoring loot request for DB creature before it is lootable");
        return Ok(());
    }
    if !creature.can_loot_for_player(Some(character.guid)) {
        warn!(
            character_guid = character.guid,
            target = format_args!("0x{:016X}", target.raw()),
            "Rejecting loot request for DB creature owned by another player or party"
        );
        return send_packet(
            stream,
            SMSG_LOOT_RESPONSE,
            &build_loot_error_response_body(target, LOOT_ERROR_DIDNT_KILL),
            Some(header_crypto),
        )
        .await;
    }
    let needs_loot_item = shared_world
        .maps
        .db_creature_needs_loot_item(character.position.map_id, target.raw())
        .await
        .unwrap_or(creature.loot_items.is_empty());
    let entry = creature.spawn.entry;
    let loot_items = if needs_loot_item {
        select_db_creature_loot_item_for_character(
            shared_world.object_mgr,
            world_db_pool,
            session,
            entry,
        )
        .await?
    } else {
        Vec::new()
    };
    let current_looter = if needs_loot_item {
        parties.assign_current_looter(character.guid).await
    } else {
        None
    };
    let Some(creature) = shared_world
        .maps
        .open_db_creature_loot(
            character.position.map_id,
            target.raw(),
            character.guid,
            parties.loot_owner_for(character.guid).await,
            current_looter,
            loot_items,
        )
        .await
    else {
        warn!(
            character_guid = character.guid,
            target = format_args!("0x{:016X}", target.raw()),
            "Rejecting loot request for unavailable or unauthorized DB creature"
        );
        return send_packet(
            stream,
            SMSG_LOOT_RESPONSE,
            &build_loot_error_response_body(target, LOOT_ERROR_DIDNT_KILL),
            Some(header_crypto),
        )
        .await;
    };
    send_player_looting_state_update(stream, shared_world, session, true, &mut *header_crypto)
        .await?;
    let response = build_db_creature_loot_response_body_for_player(
        target,
        &creature,
        db_creature_loot_method_tuple(creature.loot_method),
        character.guid,
    );
    send_packet(
        stream,
        SMSG_LOOT_RESPONSE,
        &response,
        Some(&mut *header_crypto),
    )
    .await?;
    start_group_loot_rolls_for_open_creature(
        shared_world,
        parties,
        character.guid,
        target,
        &creature,
    )
    .await;
    if creature
        .loot_method
        .is_some_and(|method| method.method == 2 && method.master_looter == character.guid)
    {
        let mut members = creature
            .loot_allowed_players
            .iter()
            .copied()
            .collect::<Vec<_>>();
        members.sort_unstable();
        let body = build_loot_master_list_body(&members);
        send_packet(stream, SMSG_LOOT_MASTER_LIST, &body, Some(header_crypto)).await?;
    }
    Ok(())
}

pub(in crate::world) async fn send_player_looting_state_update(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    session: &WorldSessionState,
    looting: bool,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let player = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let flags = player_unit_flags_with_looting(session.combat.player_in_combat, looting);
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_unit_flags_update_body(player, flags)?,
        Some(&mut *header_crypto),
    )
    .await?;
    let observer_packets = shared_world
        .maps
        .set_player_looting_state(character.position.map_id, character.guid, looting)
        .await?;
    shared_world.sessions.dispatch(observer_packets).await;
    Ok(())
}

pub(in crate::world) async fn select_db_creature_loot_item_for_character(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    session: &WorldSessionState,
    creature_entry: u32,
) -> anyhow::Result<Vec<DbCreatureLootRuntime>> {
    let loot_rows = object_mgr
        .creature_loot_items(world_db_pool, creature_entry)
        .await?;
    if loot_rows.is_empty() {
        return Ok(Vec::new());
    }
    let reference_loot_templates =
        load_reference_loot_templates(object_mgr, world_db_pool, &loot_rows).await?;

    let active_quest_ids: Vec<u32> = session
        .quests
        .quest_statuses
        .values()
        .filter(|status| status.rewarded == 0 && status.status == QUEST_STATUS_INCOMPLETE)
        .map(|status| status.quest)
        .collect();
    let mut active_quests = HashMap::new();
    for quest_id in active_quest_ids {
        if let Some(quest) = object_mgr.quest_template(world_db_pool, quest_id).await? {
            active_quests.insert(quest_id, quest);
        }
    }
    let source_item_default_counts =
        load_quest_source_item_default_counts(world_db_pool, &active_quests).await?;

    let mut loot_items = select_creature_loot_for_active_quests(
        &loot_rows,
        &reference_loot_templates,
        &active_quests,
        &session.quests.quest_statuses,
        &session.inventory.items,
        &source_item_default_counts,
    )
    .into_iter()
    .map(DbCreatureLootRuntime::from)
    .collect::<Vec<_>>();
    apply_loot_item_template_metadata(world_db_pool, &mut loot_items).await?;
    Ok(loot_items)
}

pub(in crate::world) async fn prepare_db_creature_corpse_loot(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    parties: &PartyManager,
    session: &WorldSessionState,
    character_guid: u32,
    creature_entry: u32,
) -> anyhow::Result<DbCreatureCorpseLootInit> {
    let owner = parties.loot_owner_for(character_guid).await;
    let party_members = parties.party_members(character_guid).await;
    let allowed_players = if party_members.is_empty() {
        vec![character_guid]
    } else {
        party_members
            .into_iter()
            .map(|member| member.guid)
            .collect::<Vec<_>>()
    };
    let current_looter = parties
        .assign_current_looter(character_guid)
        .await
        .or(Some(character_guid));
    let loot_method =
        parties
            .loot_method_for(character_guid)
            .await
            .map(|(method, threshold, master_looter)| CreatureLootMethod {
                method,
                threshold,
                master_looter,
            });
    let loot_items = select_db_creature_loot_item_for_character(
        object_mgr,
        world_db_pool,
        session,
        creature_entry,
    )
    .await?;
    Ok(DbCreatureCorpseLootInit {
        owner,
        allowed_players,
        current_looter,
        loot_method,
        loot_items,
    })
}

pub(in crate::world) async fn select_db_gameobject_loot_item_for_character(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    session: &WorldSessionState,
    template: &wow_db::GameObjectTemplateQuery,
) -> anyhow::Result<Vec<DbCreatureLootRuntime>> {
    let Some(loot_id) = gameobject_chest_loot_id(template) else {
        return Ok(Vec::new());
    };
    let loot_rows = object_mgr
        .gameobject_loot_items(world_db_pool, loot_id)
        .await?;
    if loot_rows.is_empty() {
        return Ok(Vec::new());
    }
    let reference_loot_templates =
        load_reference_loot_templates(object_mgr, world_db_pool, &loot_rows).await?;

    let active_quest_ids: Vec<u32> = session
        .quests
        .quest_statuses
        .values()
        .filter(|status| status.rewarded == 0 && status.status == QUEST_STATUS_INCOMPLETE)
        .map(|status| status.quest)
        .collect();
    let mut active_quests = HashMap::new();
    for quest_id in active_quest_ids {
        if let Some(quest) = object_mgr.quest_template(world_db_pool, quest_id).await? {
            active_quests.insert(quest_id, quest);
        }
    }
    let source_item_default_counts =
        load_quest_source_item_default_counts(world_db_pool, &active_quests).await?;

    let mut loot_items = select_creature_loot_for_active_quests(
        &loot_rows,
        &reference_loot_templates,
        &active_quests,
        &session.quests.quest_statuses,
        &session.inventory.items,
        &source_item_default_counts,
    )
    .into_iter()
    .filter(|loot| loot.is_quest_drop())
    .map(DbCreatureLootRuntime::from)
    .collect::<Vec<_>>();
    apply_loot_item_template_metadata(world_db_pool, &mut loot_items).await?;
    Ok(loot_items)
}

pub(in crate::world) async fn apply_loot_item_template_metadata(
    world_db_pool: &MySqlPool,
    loot_items: &mut [DbCreatureLootRuntime],
) -> anyhow::Result<()> {
    const ITEM_FLAG_MULTI_DROP: u32 = 0x0000_0800;
    for loot in loot_items {
        let template = match wow_db::get_item_template_query(world_db_pool, loot.item).await {
            Ok(Some(template)) => template,
            Ok(None) => continue,
            Err(error) => {
                warn!(
                    item = loot.item,
                    "Could not load item template metadata for loot item: {}", error
                );
                continue;
            }
        };
        loot.quality = template.quality.min(u8::MAX as u32) as u8;
        loot.free_for_all = (template.flags & ITEM_FLAG_MULTI_DROP) != 0;
    }
    Ok(())
}

pub(in crate::world) async fn start_group_loot_rolls_for_open_creature(
    shared_world: SharedWorldDeps<'_>,
    parties: &PartyManager,
    character_guid: u32,
    loot_guid: ObjectGuid,
    creature: &DbCreatureRuntime,
) {
    let Some(loot_method) = creature.loot_method else {
        return;
    };
    if loot_method.method != 3 {
        return;
    }
    for loot in creature.loot_items.iter().filter(|loot| {
        !loot.quest_drop
            && !loot.free_for_all
            && loot.quality >= loot_method.threshold
            && !creature.loot_roll_released_slots.contains(&loot.slot)
    }) {
        if let Some(start) = parties
            .start_loot_roll(
                character_guid,
                creature.current_position.map_id,
                loot_guid,
                loot.slot,
                loot.clone(),
            )
            .await
        {
            dispatch_party_member_packets(shared_world.sessions, start.packets).await;
        }
    }
}

pub(in crate::world) async fn load_reference_loot_templates(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    root_rows: &[CreatureLootQuery],
) -> anyhow::Result<HashMap<u32, Vec<CreatureLootQuery>>> {
    let mut templates = HashMap::new();
    let mut seen = HashSet::new();
    let mut pending = root_rows
        .iter()
        .filter(|row| row.is_reference())
        .map(|row| row.item)
        .collect::<Vec<_>>();

    while let Some(reference_entry) = pending.pop() {
        if !seen.insert(reference_entry) {
            continue;
        }

        let rows = object_mgr
            .reference_loot_items(world_db_pool, reference_entry)
            .await?;
        for nested_reference in rows.iter().filter(|row| row.is_reference()) {
            if !seen.contains(&nested_reference.item) {
                pending.push(nested_reference.item);
            }
        }
        templates.insert(reference_entry, rows);
    }

    Ok(templates)
}

pub(in crate::world) fn select_creature_loot_for_active_quests(
    loot_rows: &[CreatureLootQuery],
    reference_loot_templates: &HashMap<u32, Vec<CreatureLootQuery>>,
    active_quests: &HashMap<u32, QuestTemplateQuery>,
    quest_statuses: &HashMap<u32, CharacterQuestStatus>,
    inventory: &[CharacterInventoryItem],
    source_item_default_counts: &HashMap<u32, u32>,
) -> Vec<CreatureLootQuery> {
    let mut chance_rng = rand::thread_rng();
    let mut count_rng = rand::thread_rng();
    select_creature_loot_for_active_quests_with_rolls(
        loot_rows,
        reference_loot_templates,
        active_quests,
        quest_statuses,
        inventory,
        source_item_default_counts,
        || rand::Rng::gen_range(&mut chance_rng, 0.0f32..100.0f32),
        |min_count, max_count| rand::Rng::gen_range(&mut count_rng, min_count..=max_count),
    )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) fn select_creature_loot_for_active_quests_with_rolls(
    loot_rows: &[CreatureLootQuery],
    reference_loot_templates: &HashMap<u32, Vec<CreatureLootQuery>>,
    active_quests: &HashMap<u32, QuestTemplateQuery>,
    quest_statuses: &HashMap<u32, CharacterQuestStatus>,
    inventory: &[CharacterInventoryItem],
    source_item_default_counts: &HashMap<u32, u32>,
    mut chance_roll: impl FnMut() -> f32,
    mut count_roll: impl FnMut(u32, u32) -> u32,
) -> Vec<CreatureLootQuery> {
    select_creature_loot_for_active_quests_with_rolls_inner(
        loot_rows,
        reference_loot_templates,
        active_quests,
        quest_statuses,
        inventory,
        source_item_default_counts,
        &mut chance_roll,
        &mut count_roll,
    )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) fn select_creature_loot_for_active_quests_with_rolls_inner(
    loot_rows: &[CreatureLootQuery],
    reference_loot_templates: &HashMap<u32, Vec<CreatureLootQuery>>,
    active_quests: &HashMap<u32, QuestTemplateQuery>,
    quest_statuses: &HashMap<u32, CharacterQuestStatus>,
    inventory: &[CharacterInventoryItem],
    source_item_default_counts: &HashMap<u32, u32>,
    chance_roll: &mut dyn FnMut() -> f32,
    count_roll: &mut dyn FnMut(u32, u32) -> u32,
) -> Vec<CreatureLootQuery> {
    let mut active_references = Vec::new();
    let mut rolled = Vec::new();
    let mut grouped: HashMap<u8, Vec<CreatureLootQuery>> = HashMap::new();
    for loot in loot_rows {
        if loot.group_id > 0 {
            grouped.entry(loot.group_id).or_default().push(loot.clone());
            continue;
        }

        if let Some(loot) = roll_loot_row(
            loot,
            active_quests,
            quest_statuses,
            inventory,
            source_item_default_counts,
            &mut *chance_roll,
            &mut *count_roll,
        ) {
            if loot.is_reference() {
                active_references.push(loot);
            } else {
                rolled.push(loot);
            }
        }
    }

    let mut group_ids = grouped.keys().copied().collect::<Vec<_>>();
    group_ids.sort_unstable();
    for group_id in group_ids {
        let Some(rows) = grouped.remove(&group_id) else {
            continue;
        };
        if let Some(loot) = roll_loot_group(
            rows,
            active_quests,
            quest_statuses,
            inventory,
            source_item_default_counts,
            &mut *chance_roll,
            &mut *count_roll,
        ) {
            if loot.is_reference() {
                active_references.push(loot);
            } else {
                rolled.push(loot);
            }
        }
    }

    for reference in active_references {
        let Some(reference_rows) = reference_loot_templates.get(&reference.item) else {
            continue;
        };
        let loops = reference.max_count.max(1);
        for _ in 0..loops {
            rolled.extend(select_creature_loot_for_active_quests_with_rolls_inner(
                reference_rows,
                reference_loot_templates,
                active_quests,
                quest_statuses,
                inventory,
                source_item_default_counts,
                &mut *chance_roll,
                &mut *count_roll,
            ));
        }
    }

    rolled
}

pub(in crate::world) fn roll_loot_group(
    rows: Vec<CreatureLootQuery>,
    active_quests: &HashMap<u32, QuestTemplateQuery>,
    quest_statuses: &HashMap<u32, CharacterQuestStatus>,
    inventory: &[CharacterInventoryItem],
    source_item_default_counts: &HashMap<u32, u32>,
    mut chance_roll: impl FnMut() -> f32,
    mut count_roll: impl FnMut(u32, u32) -> u32,
) -> Option<CreatureLootQuery> {
    let mut explicit_chance = Vec::new();
    let mut equal_chance = Vec::new();
    for loot in rows {
        if loot.is_quest_drop()
            && !player_needs_quest_loot_item(
                loot.item,
                active_quests,
                quest_statuses,
                inventory,
                source_item_default_counts,
            )
        {
            continue;
        }
        let chance = loot.chance_or_quest_chance.abs().clamp(0.0, 100.0);
        if chance > 0.0 {
            explicit_chance.push((loot, chance));
        } else {
            equal_chance.push(loot);
        }
    }

    let selected = if !explicit_chance.is_empty() {
        let mut remaining = chance_roll();
        let mut selected = None;
        for (loot, chance) in explicit_chance {
            remaining -= chance;
            if remaining < 0.0 || chance >= 100.0 {
                selected = Some(loot);
                break;
            }
        }
        selected
    } else if equal_chance.is_empty() {
        None
    } else {
        let index =
            count_roll(0, (equal_chance.len() - 1) as u32).min((equal_chance.len() - 1) as u32);
        equal_chance.get(index as usize).cloned()
    }?;

    let mut selected = selected;
    if selected.chance_or_quest_chance == 0.0 {
        selected.chance_or_quest_chance = 100.0;
    }

    roll_loot_row(
        &selected,
        active_quests,
        quest_statuses,
        inventory,
        source_item_default_counts,
        || 0.0,
        count_roll,
    )
}

pub(in crate::world) fn roll_loot_row(
    loot: &CreatureLootQuery,
    active_quests: &HashMap<u32, QuestTemplateQuery>,
    quest_statuses: &HashMap<u32, CharacterQuestStatus>,
    inventory: &[CharacterInventoryItem],
    source_item_default_counts: &HashMap<u32, u32>,
    mut chance_roll: impl FnMut() -> f32,
    mut count_roll: impl FnMut(u32, u32) -> u32,
) -> Option<CreatureLootQuery> {
    let is_reference = loot.is_reference();
    let is_quest_drop = !is_reference && loot.is_quest_drop();
    if is_quest_drop
        && !player_needs_quest_loot_item(
            loot.item,
            active_quests,
            quest_statuses,
            inventory,
            source_item_default_counts,
        )
    {
        return None;
    }
    let chance = if is_quest_drop {
        -loot.chance_or_quest_chance
    } else {
        loot.chance_or_quest_chance.abs()
    }
    .clamp(0.0, 100.0);
    if chance <= 0.0 || chance_roll() >= chance {
        return None;
    }

    if is_reference {
        let mut rolled_loot = loot.clone();
        rolled_loot.max_count = loot.max_count.max(1);
        return Some(rolled_loot);
    }

    let min_count = loot.min_count.max(1);
    let max_count = loot.max_count.max(min_count);
    let rolled_count = count_roll(min_count, max_count).clamp(min_count, max_count);
    let mut rolled_loot = loot.clone();
    rolled_loot.min_count = rolled_count;
    rolled_loot.max_count = rolled_count;
    Some(rolled_loot)
}

pub(in crate::world) fn player_needs_quest_loot_item(
    item_id: u32,
    active_quests: &HashMap<u32, QuestTemplateQuery>,
    quest_statuses: &HashMap<u32, CharacterQuestStatus>,
    inventory: &[CharacterInventoryItem],
    source_item_default_counts: &HashMap<u32, u32>,
) -> bool {
    let owned_count: u32 = inventory
        .iter()
        .filter(|item| item.item_template == item_id)
        .map(|item| item.count)
        .sum();

    quest_statuses
        .values()
        .filter(|status| status.rewarded == 0 && status.status == QUEST_STATUS_INCOMPLETE)
        .any(|status| {
            let Some(quest) = active_quests.get(&status.quest) else {
                return false;
            };
            quest
                .req_item_id
                .iter()
                .zip(quest.req_item_count.iter())
                .any(|(req_item_id, req_count)| {
                    *req_item_id == item_id && owned_count < *req_count && *req_count > 0
                })
                || quest
                    .req_source_id
                    .iter()
                    .zip(quest.req_source_count.iter())
                    .any(|(req_source_id, req_source_count)| {
                        if *req_source_id != item_id {
                            return false;
                        }
                        let required_count = if *req_source_count > 0 {
                            *req_source_count
                        } else {
                            source_item_default_counts
                                .get(&item_id)
                                .copied()
                                .unwrap_or(0)
                        };
                        required_count > 0 && owned_count < required_count
                    })
        })
}

pub(in crate::world) async fn load_quest_source_item_default_counts(
    world_db_pool: &MySqlPool,
    active_quests: &HashMap<u32, QuestTemplateQuery>,
) -> anyhow::Result<HashMap<u32, u32>> {
    let mut counts = HashMap::new();
    for quest in active_quests.values() {
        for source_item in quest
            .req_source_id
            .iter()
            .copied()
            .filter(|item| *item != 0)
        {
            if counts.contains_key(&source_item) {
                continue;
            }
            let Some(template) =
                wow_db::get_item_template_query(world_db_pool, source_item).await?
            else {
                continue;
            };
            let default_count = if template.max_count > 0 {
                template.max_count
            } else {
                template.stackable.max(1)
            };
            counts.insert(source_item, default_count);
        }
    }

    Ok(counts)
}

pub(in crate::world) async fn handle_autostore_loot_item(
    stream: &mut WorldPacketSink,
    deps: LootMutationDeps<'_>,
    request: wow_proto::AutostoreLootItemRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let LootMutationDeps {
        character_db_pool,
        world_db_pool,
        shared_world,
        parties,
    } = deps;
    let Some(character) = &session.character.active_character else {
        warn!("Ignoring loot item request before character login");
        return Ok(());
    };
    let character_guid = character.guid;
    let character_map_id = character.position.map_id;
    let loot_slot = request.loot_slot;
    if shared_world
        .maps
        .db_creature_loot_guid_for_character(character_map_id, character_guid)
        .await
        .is_some()
    {
        let Some((creature_guid, loot_slot, loot, creature)) = shared_world
            .maps
            .take_db_creature_loot_item(character_map_id, character_guid, loot_slot)
            .await
        else {
            warn!("Ignoring DB creature loot item request after shared loot was claimed");
            return Ok(());
        };
        let release_state = LootItemReleaseState::from_creature(&creature, loot_slot);
        if !can_autostore_shared_creature_loot(character_guid, &creature, &loot) {
            restore_db_creature_loot_item_with_release_state(
                shared_world,
                character_map_id,
                creature_guid,
                loot_slot,
                loot,
                release_state,
            )
            .await;
            return Ok(());
        }
        if !creature.loot_roll_released_slots.contains(&loot_slot)
            && should_use_group_loot_roll(&creature, &loot)
        {
            shared_world
                .maps
                .restore_db_creature_loot_item(
                    character_map_id,
                    creature_guid,
                    loot_slot,
                    loot.clone(),
                )
                .await;
            if let Some(start) = parties
                .start_loot_roll(
                    character_guid,
                    character_map_id,
                    ObjectGuid::from_raw(creature_guid),
                    loot_slot,
                    loot.clone(),
                )
                .await
            {
                dispatch_party_member_packets(shared_world.sessions, start.packets).await;
            }
            return Ok(());
        }
        if should_block_master_loot(&creature, character_guid, &loot) {
            shared_world
                .maps
                .restore_db_creature_loot_item(character_map_id, creature_guid, loot_slot, loot)
                .await;
            return Ok(());
        }
        let stored = autostore_loot_item(
            LootAutostoreContext {
                stream,
                character_db_pool,
                object_mgr: shared_world.object_mgr,
                world_db_pool,
                session,
                header_crypto,
                character_guid,
            },
            creature_guid,
            loot.clone(),
            loot_slot,
        )
        .await?;
        if !stored {
            restore_db_creature_loot_item_with_release_state(
                shared_world,
                character_map_id,
                creature_guid,
                loot_slot,
                loot,
                release_state,
            )
            .await;
        } else {
            dispatch_creature_loot_removed_to_other_open_looters(
                shared_world,
                character_map_id,
                creature_guid,
                character_guid,
                loot_slot,
            )
            .await;
        }
        return Ok(());
    }

    if shared_world
        .maps
        .db_gameobject_loot_guid_for_character(character_map_id, character_guid)
        .await
        .is_some()
    {
        let Some((gameobject_guid, loot_slot, loot)) = shared_world
            .maps
            .take_db_gameobject_loot_item(character_map_id, character_guid, loot_slot)
            .await
        else {
            return Ok(());
        };
        let stored = autostore_loot_item(
            LootAutostoreContext {
                stream,
                character_db_pool,
                object_mgr: shared_world.object_mgr,
                world_db_pool,
                session,
                header_crypto,
                character_guid,
            },
            gameobject_guid,
            loot.clone(),
            loot_slot,
        )
        .await?;
        if !stored {
            shared_world
                .maps
                .restore_db_gameobject_loot_item(character_map_id, gameobject_guid, loot_slot, loot)
                .await;
        } else {
            let guid = ObjectGuid::from_raw(gameobject_guid);
            let consumed = shared_world
                .maps
                .consume_db_gameobject(character_map_id, guid, Instant::now(), Some(character_guid))
                .await;
            if let Some((gameobject, observer_packets)) = consumed {
                let _ = gameobject;
                shared_world.sessions.dispatch(observer_packets).await;
            }
            send_packet(
                stream,
                SMSG_DESTROY_OBJECT,
                &gameobject_guid.to_le_bytes(),
                Some(&mut *header_crypto),
            )
            .await?;
        }
        return Ok(());
    }

    warn!(loot_slot, "Ignoring loot item request without open DB loot");
    Ok(())
}

#[derive(Clone, Copy)]
pub(in crate::world) struct LootMutationDeps<'a> {
    pub(in crate::world) character_db_pool: &'a MySqlPool,
    pub(in crate::world) world_db_pool: &'a MySqlPool,
    pub(in crate::world) shared_world: SharedWorldDeps<'a>,
    pub(in crate::world) parties: &'a PartyManager,
}

pub(in crate::world) struct LootGrantRequest {
    pub(in crate::world) target_guid: u32,
    pub(in crate::world) loot_slot: u8,
    pub(in crate::world) loot: DbCreatureLootRuntime,
}

#[derive(Clone, Copy)]
pub(in crate::world) struct LootItemReleaseState {
    pub(in crate::world) roll_released: bool,
    pub(in crate::world) current_looter_pass: bool,
}

impl LootItemReleaseState {
    pub(in crate::world) fn from_creature(creature: &DbCreatureRuntime, loot_slot: u8) -> Self {
        Self {
            roll_released: creature.loot_roll_released_slots.contains(&loot_slot),
            current_looter_pass: creature.loot_current_looter_pass_slots.contains(&loot_slot),
        }
    }
}

pub(in crate::world) async fn restore_db_creature_loot_item_with_release_state(
    shared_world: SharedWorldDeps<'_>,
    map_id: u32,
    creature_guid: u64,
    loot_slot: u8,
    loot: DbCreatureLootRuntime,
    release_state: LootItemReleaseState,
) {
    shared_world
        .maps
        .restore_db_creature_loot_item(map_id, creature_guid, loot_slot, loot)
        .await;
    if release_state.roll_released {
        shared_world
            .maps
            .release_db_creature_loot_roll_item(map_id, creature_guid, loot_slot)
            .await;
    }
    if release_state.current_looter_pass {
        shared_world
            .maps
            .release_db_creature_current_looter_pass_item(map_id, creature_guid, loot_slot)
            .await;
    }
}

pub(in crate::world) fn db_creature_loot_method_tuple(
    method: Option<CreatureLootMethod>,
) -> Option<(u8, u8, u32)> {
    method.map(|method| (method.method, method.threshold, method.master_looter))
}

pub(in crate::world) async fn resolve_loot_roll_outcome(
    stream: &mut WorldPacketSink,
    deps: LootMutationDeps<'_>,
    outcome: LootRollVoteOutcome,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(loot) = outcome.loot else {
        return Ok(());
    };
    let creature_guid = outcome.loot_guid.raw();
    if let Some(winner) = outcome.winner {
        let Some((loot_slot, removed_loot, _creature)) = deps
            .shared_world
            .maps
            .take_db_creature_loot_item_by_guid(outcome.map_id, creature_guid, outcome.loot_slot)
            .await
        else {
            return Ok(());
        };
        if removed_loot.item != loot.item || loot_slot != outcome.loot_slot {
            deps.shared_world
                .maps
                .restore_db_creature_loot_item(
                    outcome.map_id,
                    creature_guid,
                    loot_slot,
                    removed_loot,
                )
                .await;
            return Ok(());
        }
        let granted = grant_loot_item_to_character(
            stream,
            deps,
            LootGrantRequest {
                target_guid: winner,
                loot_slot,
                loot: removed_loot.clone(),
            },
            session,
            header_crypto,
        )
        .await?;
        if granted {
            dispatch_creature_loot_removed_to_other_open_looters(
                deps.shared_world,
                outcome.map_id,
                creature_guid,
                winner,
                loot_slot,
            )
            .await;
        } else {
            deps.shared_world
                .maps
                .restore_db_creature_loot_item(
                    outcome.map_id,
                    creature_guid,
                    loot_slot,
                    removed_loot,
                )
                .await;
            deps.shared_world
                .maps
                .release_db_creature_loot_roll_item(outcome.map_id, creature_guid, loot_slot)
                .await;
        }
    } else {
        deps.shared_world
            .maps
            .release_db_creature_loot_roll_item(outcome.map_id, creature_guid, outcome.loot_slot)
            .await;
    }
    Ok(())
}

pub(in crate::world) fn should_use_group_loot_roll(
    creature: &DbCreatureRuntime,
    loot: &DbCreatureLootRuntime,
) -> bool {
    let Some(loot_method) = creature.loot_method else {
        return false;
    };
    if loot_method.method != 3 {
        return false;
    }
    if loot.quest_drop || loot.free_for_all {
        return false;
    }
    loot.quality >= loot_method.threshold
}

pub(in crate::world) fn can_autostore_shared_creature_loot(
    character_guid: u32,
    creature: &DbCreatureRuntime,
    loot: &DbCreatureLootRuntime,
) -> bool {
    let Some(loot_method) = creature.loot_method else {
        return true;
    };
    if loot.free_for_all {
        return true;
    }
    let under_threshold = loot.quest_drop || loot.quality < loot_method.threshold;
    match loot_method.method {
        0 => true,
        1 | 4 => {
            !under_threshold
                || creature.loot_current_looter == Some(character_guid)
                || creature.loot_current_looter_pass_slots.contains(&loot.slot)
                || creature.loot_roll_released_slots.contains(&loot.slot)
        }
        3 => {
            under_threshold
                && (creature.loot_current_looter == Some(character_guid)
                    || creature.loot_current_looter_pass_slots.contains(&loot.slot))
                || creature.loot_roll_released_slots.contains(&loot.slot)
        }
        2 if under_threshold => {
            creature.loot_current_looter == Some(character_guid)
                || creature.loot_current_looter_pass_slots.contains(&loot.slot)
                || creature.loot_roll_released_slots.contains(&loot.slot)
        }
        2 => false,
        _ => true,
    }
}

pub(in crate::world) fn should_block_master_loot(
    creature: &DbCreatureRuntime,
    character_guid: u32,
    loot: &DbCreatureLootRuntime,
) -> bool {
    let Some(loot_method) = creature.loot_method else {
        return false;
    };
    if loot_method.method != 2 || character_guid == loot_method.master_looter {
        return false;
    }
    loot.quality >= loot_method.threshold
}

pub(in crate::world) async fn handle_loot_master_give(
    stream: &mut WorldPacketSink,
    deps: LootMutationDeps<'_>,
    request: wow_proto::LootMasterGiveRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let LootMutationDeps {
        character_db_pool,
        world_db_pool,
        shared_world,
        parties,
    } = deps;
    let loot_guid = ObjectGuid::from_raw(request.loot_raw_guid);
    let loot_slot = request.loot_slot;
    let target_guid = ObjectGuid::from_raw(request.target_raw_guid);
    if !target_guid.is_player() {
        return Ok(());
    }
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };

    let character_map_id = character.position.map_id;
    let Some((creature_guid, loot_slot, loot, creature)) = shared_world
        .maps
        .take_db_creature_loot_item(character_map_id, character.guid, loot_slot)
        .await
    else {
        return Ok(());
    };
    let valid_assignment = creature_guid == loot_guid.raw()
        && creature.loot_method.is_some_and(|loot_method| {
            loot_method.method == 2
                && loot_method.master_looter == character.guid
                && loot.quality >= loot_method.threshold
        })
        && creature
            .loot_allowed_players
            .contains(&target_guid.counter());
    if !valid_assignment {
        shared_world
            .maps
            .restore_db_creature_loot_item(character_map_id, creature_guid, loot_slot, loot)
            .await;
        return Ok(());
    }
    let granted = grant_loot_item_to_character(
        stream,
        LootMutationDeps {
            character_db_pool,
            world_db_pool,
            shared_world,
            parties,
        },
        LootGrantRequest {
            target_guid: target_guid.counter(),
            loot_slot,
            loot: loot.clone(),
        },
        session,
        header_crypto,
    )
    .await?;
    if !granted {
        shared_world
            .maps
            .restore_db_creature_loot_item(character_map_id, creature_guid, loot_slot, loot)
            .await;
    } else {
        dispatch_creature_loot_removed_to_other_open_looters(
            shared_world,
            character_map_id,
            creature_guid,
            target_guid.counter(),
            loot_slot,
        )
        .await;
    }
    Ok(())
}

pub(in crate::world) async fn grant_loot_item_to_character(
    stream: &mut WorldPacketSink,
    deps: LootMutationDeps<'_>,
    request: LootGrantRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    let LootMutationDeps {
        character_db_pool,
        world_db_pool,
        shared_world,
        parties: _,
    } = deps;
    let LootGrantRequest {
        target_guid,
        loot_slot,
        loot,
    } = request;
    let current_guid = session
        .character
        .active_character
        .as_ref()
        .map(|character| character.guid);
    let target_is_current = current_guid == Some(target_guid);
    let target_map_id = if target_is_current {
        session
            .character
            .active_character
            .as_ref()
            .map(|character| character.position.map_id)
    } else {
        let Some(character) = session.character.active_character.as_ref() else {
            return Ok(false);
        };
        shared_world
            .maps
            .player_runtime_snapshot(character.position.map_id, target_guid)
            .await
            .map(|snapshot| snapshot.position.map_id)
    };
    let Some(target_map_id) = target_map_id else {
        return Ok(false);
    };
    let target_session_id = shared_world
        .sessions
        .session_for_character(target_guid)
        .await;
    if target_session_id.is_none() && !target_is_current {
        return Ok(false);
    }

    let inventory = if target_is_current {
        session.inventory.items.clone()
    } else {
        wow_db::get_character_inventory_items(character_db_pool, target_guid).await?
    };
    let Some(template) = wow_db::get_item_template_query(world_db_pool, loot.item).await? else {
        return Ok(false);
    };
    let equipped_bags = load_equipped_bag_infos(world_db_pool, &inventory).await?;
    let Some(store_plan) = plan_store_item(
        &inventory,
        &template,
        loot.count,
        &equipped_bags,
        None,
        None,
    ) else {
        if target_is_current {
            send_inventory_change_failure(
                stream,
                EQUIP_ERR_INVENTORY_FULL,
                None,
                None,
                header_crypto,
            )
            .await?;
        }
        return Ok(false);
    };
    let owner_guid = ObjectGuid::new(HighGuid::Player, 0, target_guid);

    let mut update_blocks = Vec::new();
    let mut pushed_item = None;

    let random_properties = generate_item_instance_random_properties(
        world_db_pool,
        &session.movement.db_creature_navigation.world_data_files,
        loot.item,
    )
    .await?;
    for slot in &store_plan {
        if let Some(item_guid) = slot.existing_item {
            let existing_count = inventory
                .iter()
                .find(|item| item.item == item_guid)
                .map(|item| item.count)
                .unwrap_or(0);
            wow_db::update_character_inventory_item_count(
                character_db_pool,
                target_guid,
                item_guid,
                existing_count.saturating_add(slot.count),
            )
            .await?;
        } else {
            wow_db::add_character_inventory_item_with_random_properties(
                character_db_pool,
                wow_db::AddCharacterInventoryItemRequest {
                    guid: target_guid,
                    bag: slot.bag as u32,
                    slot: slot.slot,
                    item_template: loot.item,
                    count: slot.count,
                    durability: 0,
                    random_properties: random_properties.as_ref(),
                },
            )
            .await?;
        }
    }

    let new_inventory =
        wow_db::get_character_inventory_items(character_db_pool, target_guid).await?;
    for slot in &store_plan {
        if let Some(item_guid) = slot.existing_item {
            if let Some(item) = new_inventory.iter().find(|item| item.item == item_guid) {
                update_blocks.push(build_item_stack_count_update_block(item.item, item.count)?);
                pushed_item = Some(item.clone());
            }
            continue;
        }
        if let Some(item) = new_inventory
            .iter()
            .find(|item| item.bag == slot.bag as u32 && item.slot == slot.slot)
        {
            pushed_item = Some(item.clone());
            let contained_guid = item_contained_guid(owner_guid, &new_inventory, item);
            update_blocks.push(build_item_create_update_block(
                owner_guid,
                contained_guid,
                item,
                (template.container_slots > 0).then_some(template.container_slots),
            )?);
            update_blocks.extend(build_inventory_position_update_blocks(
                target_guid,
                &new_inventory,
                slot.bag,
                slot.slot,
            )?);
        }
    }

    if target_is_current {
        session.inventory.items = new_inventory.clone();
    }
    shared_world
        .maps
        .update_player_inventory(target_map_id, target_guid, new_inventory)
        .await;

    let mut packets = vec![OutboundWorldPacket {
        opcode: SMSG_LOOT_REMOVED,
        body: vec![loot_slot],
    }];
    if let Some(item) = pushed_item.as_ref() {
        packets.push(OutboundWorldPacket {
            opcode: SMSG_ITEM_PUSH_RESULT,
            body: build_item_push_result_body(target_guid, item, loot.count, true, false, true),
        });
    }
    packets.push(OutboundWorldPacket {
        opcode: SMSG_UPDATE_OBJECT,
        body: build_update_object_body(&update_blocks),
    });

    if target_is_current {
        for packet in packets {
            send_packet(
                stream,
                packet.opcode,
                &packet.body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
        complete_inventory_item_quests(
            stream,
            character_db_pool,
            shared_world.object_mgr,
            world_db_pool,
            session,
            target_guid,
            header_crypto,
        )
        .await?;
    } else if let Some(session_id) = target_session_id {
        shared_world
            .sessions
            .dispatch(
                packets
                    .into_iter()
                    .map(|packet| (session_id, packet))
                    .collect(),
            )
            .await;
    }

    Ok(true)
}

pub(in crate::world) async fn handle_loot_money(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.character.active_character else {
        warn!("Ignoring loot money request before character login");
        return Ok(());
    };
    let character_guid = character.guid;
    let character_map_id = character.position.map_id;
    if shared_world
        .maps
        .db_creature_loot_guid_for_character(character_map_id, character_guid)
        .await
        .is_some()
    {
        let Some(creature_guid) = shared_world
            .maps
            .db_creature_loot_guid_for_character(character_map_id, character_guid)
            .await
        else {
            return Ok(());
        };
        let Some((gained_money, creature)) = shared_world
            .maps
            .take_db_creature_loot_money(character_map_id, character_guid)
            .await
        else {
            return Ok(());
        };
        send_packet(
            stream,
            SMSG_LOOT_CLEAR_MONEY,
            &[],
            Some(&mut *header_crypto),
        )
        .await?;
        dispatch_creature_loot_clear_money_to_other_open_looters(
            shared_world,
            character_map_id,
            creature_guid,
            character_guid,
        )
        .await;
        return grant_creature_loot_money(
            stream,
            character_db_pool,
            shared_world,
            session,
            header_crypto,
            character_guid,
            gained_money,
            &creature,
        )
        .await;
    }

    warn!("Ignoring loot money request without open DB creature loot");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn grant_creature_loot_money(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
    looter_guid: u32,
    gained_money: u32,
    creature: &DbCreatureRuntime,
) -> anyhow::Result<()> {
    let recipients = creature_loot_money_recipients(creature, looter_guid);
    let share = creature_loot_money_share(creature, gained_money);
    for recipient_guid in recipients {
        let target_session_id = if recipient_guid == looter_guid {
            None
        } else {
            shared_world
                .sessions
                .session_for_character(recipient_guid)
                .await
        };
        if recipient_guid != looter_guid && target_session_id.is_none() {
            continue;
        }
        let money = wow_db::add_character_money(character_db_pool, recipient_guid, share).await?;
        let packets = vec![
            OutboundWorldPacket {
                opcode: SMSG_LOOT_MONEY_NOTIFY,
                body: share.to_le_bytes().to_vec(),
            },
            OutboundWorldPacket {
                opcode: SMSG_UPDATE_OBJECT,
                body: build_player_money_update_body(recipient_guid, money)?,
            },
        ];
        if recipient_guid == looter_guid {
            for packet in packets {
                send_packet(
                    stream,
                    packet.opcode,
                    &packet.body,
                    Some(&mut *header_crypto),
                )
                .await?;
            }
        } else if let Some(session_id) = target_session_id {
            shared_world
                .sessions
                .dispatch(
                    packets
                        .into_iter()
                        .map(|packet| (session_id, packet))
                        .collect(),
                )
                .await;
        }
    }
    if session
        .character
        .active_character
        .as_ref()
        .is_some_and(|character| character.guid == looter_guid)
    {
        sync_active_player_gameplay_state(shared_world.maps, session).await;
    }
    Ok(())
}

pub(in crate::world) fn creature_loot_money_recipients(
    creature: &DbCreatureRuntime,
    looter_guid: u32,
) -> Vec<u32> {
    if creature.loot_method.is_none() {
        return vec![looter_guid];
    }
    let mut recipients = creature
        .loot_allowed_players
        .iter()
        .copied()
        .collect::<Vec<_>>();
    if recipients.is_empty() {
        recipients.push(looter_guid);
    }
    recipients.sort_unstable();
    recipients.dedup();
    recipients
}

pub(in crate::world) fn creature_loot_money_share(
    creature: &DbCreatureRuntime,
    gained_money: u32,
) -> u32 {
    let divisor = if creature.loot_method.is_some() {
        creature.loot_allowed_players.len().max(1)
    } else {
        1
    };
    gained_money / divisor as u32
}

pub(in crate::world) async fn dispatch_creature_loot_removed_to_other_open_looters(
    shared_world: SharedWorldDeps<'_>,
    map_id: u32,
    creature_guid: u64,
    looter_guid: u32,
    loot_slot: u8,
) {
    let packets = shared_world
        .maps
        .db_creature_looting_characters(map_id, creature_guid)
        .await
        .into_iter()
        .filter(|character_guid| *character_guid != looter_guid)
        .map(|character_guid| {
            (
                character_guid,
                OutboundWorldPacket {
                    opcode: SMSG_LOOT_REMOVED,
                    body: vec![loot_slot],
                },
            )
        })
        .collect();
    dispatch_party_member_packets(shared_world.sessions, packets).await;
}

pub(in crate::world) async fn dispatch_creature_loot_clear_money_to_other_open_looters(
    shared_world: SharedWorldDeps<'_>,
    map_id: u32,
    creature_guid: u64,
    looter_guid: u32,
) {
    let packets = shared_world
        .maps
        .db_creature_looting_characters(map_id, creature_guid)
        .await
        .into_iter()
        .filter(|character_guid| *character_guid != looter_guid)
        .map(|character_guid| {
            (
                character_guid,
                OutboundWorldPacket {
                    opcode: SMSG_LOOT_CLEAR_MONEY,
                    body: Vec::new(),
                },
            )
        })
        .collect();
    dispatch_party_member_packets(shared_world.sessions, packets).await;
}

pub(in crate::world) async fn handle_loot_release(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    request: wow_proto::LootReleaseRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let target = ObjectGuid::from_raw(request.raw_guid);
    if target.is_game_object() {
        if let Some(character) = session.character.active_character.as_ref() {
            shared_world
                .maps
                .release_db_gameobject_loot(character.position.map_id, target.raw(), character.guid)
                .await;
            send_player_looting_state_update(
                stream,
                shared_world,
                session,
                false,
                &mut *header_crypto,
            )
            .await?;
        }
        return send_packet(
            stream,
            SMSG_LOOT_RELEASE_RESPONSE,
            &build_loot_release_response_body(target, true),
            Some(header_crypto),
        )
        .await;
    }
    let Some(character) = session.character.active_character.as_ref() else {
        warn!("Ignoring loot release before character login");
        return Ok(());
    };
    let Some(event) = shared_world
        .maps
        .release_db_creature_loot(
            character.position.map_id,
            target.raw(),
            Instant::now(),
            Some(character.guid),
        )
        .await?
    else {
        warn!(
            target = format_args!("0x{:016X}", target.raw()),
            "Ignoring loot release for unknown target"
        );
        return Ok(());
    };
    let _ = event.creature;
    send_player_looting_state_update(stream, shared_world, session, false, &mut *header_crypto)
        .await?;
    send_packet(
        stream,
        SMSG_LOOT_RELEASE_RESPONSE,
        &build_loot_release_response_body(target, true),
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        event.direct_packet.opcode,
        &event.direct_packet.body,
        Some(header_crypto),
    )
    .await?;
    shared_world.sessions.dispatch(event.observer_packets).await;
    Ok(())
}
