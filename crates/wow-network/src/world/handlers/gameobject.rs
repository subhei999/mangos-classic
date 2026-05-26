use super::*;
use wow_proto::world::WorldOpcode;

// CMaNGOS reference: src/game/Entities/QueryHandler.cpp and SpellHandler.cpp
// gameobject query/use flow.

pub(in crate::world) async fn handle_gameobject_query(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    query: wow_proto::GameObjectQueryRequest,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let template = wow_db::get_gameobject_template_query(world_db_pool, query.entry).await?;
    let response = build_gameobject_query_response(query.entry, template.as_ref());
    send_packet(
        stream,
        WorldOpcode::SmsgGameObjectQueryResponse as u16,
        &response,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn handle_gameobject_use(
    stream: &mut WorldPacketSink,
    deps: GameObjectUseDeps<'_>,
    request: wow_proto::GameObjectUseRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.clone() else {
        return Ok(());
    };
    let guid = ObjectGuid::from_raw(request.raw_guid);
    if !guid.is_game_object() {
        return Ok(());
    }
    let Some(runtime) = deps
        .maps
        .db_gameobject_snapshot(character.position.map_id, guid)
        .await
    else {
        return Ok(());
    };
    if runtime.spawn.map != character.position.map_id {
        return Ok(());
    }
    let now = Instant::now();
    if runtime.is_unavailable(now) {
        return Ok(());
    }
    if runtime.spawn.template.flags & (GO_FLAG_LOCKED | GO_FLAG_IN_USE | GO_FLAG_NO_INTERACT) != 0 {
        return Ok(());
    }
    if !is_position_inside_radius(runtime.position(), character.position, 8.0) {
        return Ok(());
    }

    if let Some(required_quest) = gameobject_required_active_quest(&runtime.spawn.template) {
        let can_use = session
            .quests
            .quest_statuses
            .get(&required_quest)
            .is_some_and(|status| status.status == QUEST_STATUS_INCOMPLETE && status.rewarded == 0);
        if !can_use {
            return Ok(());
        }
    }
    if gameobject_type_uses_quest_objective_gate(runtime.spawn.template.object_type) {
        let objective_quests = deps
            .object_mgr
            .gameobject_objective_quests(deps.world_db_pool, runtime.spawn.entry)
            .await?;
        if !objective_quests.is_empty()
            && !session_has_incomplete_gameobject_objective(
                session,
                &objective_quests,
                runtime.spawn.entry,
            )
        {
            return Ok(());
        }
    }

    if runtime.spawn.template.object_type == GO_TYPE_SPELLCASTER {
        return handle_spellcaster_gameobject_use(
            stream,
            deps,
            runtime,
            character,
            session,
            now,
            header_crypto,
        )
        .await;
    }

    if gameobject_chest_has_loot_id(&runtime.spawn.template) {
        return open_gameobject_loot_from_use(
            stream,
            deps,
            &runtime,
            character.guid,
            session,
            header_crypto,
        )
        .await;
    }

    let handled_questgiver = handle_gameobject_questgiver_use(
        stream,
        deps.object_mgr,
        deps.world_db_pool,
        session,
        guid,
        header_crypto,
    )
    .await?;
    let objective_updated = grant_gameobject_use_credit(
        stream,
        GameObjectUseCreditDeps {
            character_db_pool: deps.character_db_pool,
            object_mgr: deps.object_mgr,
            world_db_pool: deps.world_db_pool,
        },
        session,
        runtime.spawn.entry,
        guid,
        header_crypto,
    )
    .await?;

    if !(handled_questgiver || objective_updated) {
        return Ok(());
    }

    if should_consume_gameobject_on_use(&runtime.spawn.template) {
        let consumed = deps
            .maps
            .consume_db_gameobject(character.position.map_id, guid, now, Some(character.guid))
            .await;
        if let Some((gameobject, observer_packets)) = consumed {
            let _ = gameobject;
            deps.sessions.dispatch(observer_packets).await;
        }
        send_packet(
            stream,
            WorldOpcode::SmsgDestroyObject as u16,
            &guid.raw().to_le_bytes(),
            Some(header_crypto),
        )
        .await?;
    }

    Ok(())
}

pub(in crate::world) async fn open_gameobject_loot_from_use(
    stream: &mut WorldPacketSink,
    deps: GameObjectUseDeps<'_>,
    runtime: &DbGameObjectRuntime,
    character_guid: u32,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let loot_items = select_db_gameobject_loot_item_for_character(
        deps.object_mgr,
        deps.world_db_pool,
        session,
        &runtime.spawn.template,
    )
    .await?;
    if loot_items.is_empty()
        && gameobject_chest_loot_is_exclusively_quest_drops(
            deps.object_mgr,
            deps.world_db_pool,
            &runtime.spawn.template,
        )
        .await?
    {
        return Ok(());
    }
    let guid = runtime.guid();
    let Some((_gameobject, loot_items)) = deps
        .maps
        .open_db_gameobject_loot(runtime.spawn.map, guid.raw(), character_guid, loot_items)
        .await
    else {
        warn!(
            target = format_args!("0x{:016X}", guid.raw()),
            "Ignoring gameobject use loot open for unavailable gameobject"
        );
        return Ok(());
    };
    send_player_looting_state_update(
        stream,
        SharedWorldDeps {
            object_mgr: deps.object_mgr,
            maps: deps.maps,
            sessions: deps.sessions,
        },
        session,
        true,
        &mut *header_crypto,
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgLootResponse as u16,
        &build_gameobject_loot_response_body(guid, &loot_items),
        Some(header_crypto),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn handle_spellcaster_gameobject_use(
    stream: &mut WorldPacketSink,
    deps: GameObjectUseDeps<'_>,
    runtime: DbGameObjectRuntime,
    character: ActiveCharacter,
    session: &mut WorldSessionState,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if runtime.spellcaster_party_only() {
        let owner_guid = runtime
            .created_by
            .filter(|guid| guid.is_player())
            .map(ObjectGuid::counter);
        let Some(owner_guid) = owner_guid else {
            return Ok(());
        };
        if !deps.parties.same_party(owner_guid, character.guid).await {
            return Ok(());
        }
    }

    let Some(spell_id) = runtime.spellcaster_spell_id() else {
        return Ok(());
    };
    let Some(spell_template) = deps
        .object_mgr
        .spell_template(deps.world_db_pool, spell_id)
        .await?
    else {
        return Ok(());
    };
    let spell_info = SpellInfo::from_template(&spell_template);
    let Some(prepared_spell) = spell_info.prepare_gameobject_cast(runtime.guid()) else {
        return Ok(());
    };
    let spell_profile = prepared_spell.profile;
    complete_gameobject_use_spell_cast(
        stream,
        SpellCastDeps {
            character_db_pool: deps.character_db_pool,
            world_db_pool: deps.world_db_pool,
            account_id: session.account.account_id,
            shared_world: SharedWorldDeps {
                object_mgr: deps.object_mgr,
                maps: deps.maps,
                sessions: deps.sessions,
            },
            parties: deps.parties,
        },
        session,
        ObjectGuid::new(HighGuid::Player, 0, character.guid),
        prepared_spell,
        spell_template,
        spell_profile,
        SpellCastTargets::default(),
        now,
        header_crypto,
    )
    .await
}

#[derive(Clone, Copy)]
pub(in crate::world) struct GameObjectUseDeps<'a> {
    pub(in crate::world) character_db_pool: &'a MySqlPool,
    pub(in crate::world) world_db_pool: &'a MySqlPool,
    pub(in crate::world) object_mgr: &'a ObjectMgr,
    pub(in crate::world) maps: &'a Arc<MapRuntimeManager>,
    pub(in crate::world) parties: &'a PartyManager,
    pub(in crate::world) sessions: &'a Arc<SessionRegistry>,
}

pub(in crate::world) async fn stream_newly_visible_db_gameobjects(
    stream: &mut WorldPacketSink,
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    maps: &Arc<MapRuntimeManager>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.character.active_character else {
        return Ok(());
    };
    let character_guid = character.guid;
    let position = character.position;
    if !maps
        .should_rescan_player_gameobject_visibility(position.map_id, character_guid, position)
        .await
    {
        return Ok(());
    }
    maps.ensure_db_gameobject_grids_loaded(
        world_db_pool,
        position.map_id,
        position,
        CREATURE_SPAWN_RADIUS_YARDS,
    )
    .await?;
    let nearby_gameobjects = maps
        .nearby_db_gameobject_snapshots(
            position.map_id,
            position,
            CREATURE_SPAWN_RADIUS_YARDS,
            CREATURE_SPAWN_LIMIT,
        )
        .await;
    let now = Instant::now();
    let stage = maps
        .stage_player_db_gameobject_visibility(
            position.map_id,
            character_guid,
            position,
            nearby_gameobjects,
            now,
        )
        .await;
    let create_guids = stage
        .create_guids
        .iter()
        .map(|guid| guid.raw())
        .collect::<HashSet<_>>();
    let create_candidates = stage.nearby_gameobjects.clone();
    let updates = mirror_db_gameobject_visibility_stage(session, stage, now)?;
    for guid in updates.destroy_guids {
        send_packet(
            stream,
            WorldOpcode::SmsgDestroyObject as u16,
            &guid.raw().to_le_bytes(),
            Some(&mut *header_crypto),
        )
        .await?;
    }
    let mut create_blocks = Vec::new();
    for runtime in create_candidates {
        if !create_guids.contains(&runtime.guid().raw()) || runtime.is_unavailable(now) {
            continue;
        }
        let dynamic_flags =
            gameobject_dynamic_flags_for_player(object_mgr, world_db_pool, session, &runtime)
                .await?;
        create_blocks.push(build_db_gameobject_runtime_create_block_with_dynamic_flags(
            &runtime,
            dynamic_flags,
        )?);
    }
    for chunk in create_blocks.chunks(CREATURE_UPDATE_CHUNK_SIZE) {
        send_packet(
            stream,
            WorldOpcode::SmsgUpdateObject as u16,
            &build_update_object_body(chunk),
            Some(&mut *header_crypto),
        )
        .await?;
    }
    send_visible_quest_gameobject_dynamic_updates(
        stream,
        object_mgr,
        world_db_pool,
        maps,
        session,
        header_crypto,
    )
    .await?;
    Ok(())
}

#[derive(Debug, Default)]
#[allow(dead_code)]
pub(in crate::world) struct DbGameObjectVisibilityUpdates {
    pub(in crate::world) create_bodies: Vec<Vec<u8>>,
    pub(in crate::world) destroy_guids: Vec<ObjectGuid>,
}

pub(in crate::world) fn build_gameobject_query_response(
    entry: u32,
    template: Option<&wow_db::GameObjectTemplateQuery>,
) -> Vec<u8> {
    let Some(template) = template else {
        return (entry | 0x8000_0000).to_le_bytes().to_vec();
    };

    let mut body = Vec::with_capacity(160);
    body.extend_from_slice(&entry.to_le_bytes());
    body.extend_from_slice(&(template.object_type as u32).to_le_bytes());
    body.extend_from_slice(&template.display_id.to_le_bytes());
    push_cstring(&mut body, &template.name);
    body.push(0);
    body.push(0);
    body.push(0);
    push_cstring(&mut body, &template.icon_name);
    for value in &template.raw_data {
        body.extend_from_slice(&value.to_le_bytes());
    }
    body
}

#[cfg(test)]
#[allow(dead_code)]
pub(in crate::world) fn should_rescan_db_gameobject_visibility(
    session: &WorldSessionState,
    position: WorldPosition,
) -> bool {
    let Some(previous) = session.visibility.last_gameobject_visibility_position else {
        return true;
    };
    if previous.map_id != position.map_id {
        return true;
    }
    distance_squared_2d(previous.x, previous.y, position.x, position.y)
        >= CREATURE_VISIBILITY_RESCAN_DISTANCE_YARDS * CREATURE_VISIBILITY_RESCAN_DISTANCE_YARDS
}

#[cfg(test)]
pub(in crate::world) fn stage_db_gameobject_visibility_updates(
    session: &mut WorldSessionState,
    position: WorldPosition,
    nearby_gameobjects: Vec<DbGameObjectRuntime>,
    now: Instant,
) -> anyhow::Result<DbGameObjectVisibilityUpdates> {
    let nearby_guids = nearby_gameobjects
        .iter()
        .map(|gameobject| gameobject.guid().raw())
        .collect::<HashSet<_>>();

    let mut destroy_guids = session
        .visibility
        .db_gameobjects
        .iter()
        .filter(|(guid, gameobject)| {
            gameobject.client_visible
                && !nearby_guids.contains(guid)
                && !is_position_inside_radius(
                    gameobject.position(),
                    position,
                    CREATURE_VISIBILITY_UNLOAD_RADIUS_YARDS,
                )
        })
        .map(|(guid, _)| *guid)
        .collect::<Vec<_>>();

    for guid in &destroy_guids {
        session.visibility.db_gameobjects.remove(guid);
    }

    let mut create_blocks = Vec::new();
    for mut runtime in nearby_gameobjects {
        let guid = runtime.guid().raw();
        if let Some(existing) = session.visibility.db_gameobjects.get_mut(&guid) {
            existing.spawn = runtime.spawn;
            existing.consumed_until = runtime.consumed_until;
            if existing.is_consumed(now) {
                if existing.client_visible {
                    destroy_guids.push(guid);
                }
                existing.client_visible = false;
                continue;
            }
            if !existing.client_visible {
                existing.client_visible = true;
                create_blocks.push(build_db_gameobject_runtime_create_block_for_quest_statuses(
                    existing,
                    &session.quests.quest_statuses,
                )?);
            }
            continue;
        }

        runtime.client_visible = !runtime.is_consumed(now);
        if runtime.client_visible {
            create_blocks.push(build_db_gameobject_runtime_create_block_for_quest_statuses(
                &runtime,
                &session.quests.quest_statuses,
            )?);
        }
        session.visibility.db_gameobjects.insert(guid, runtime);
    }

    Ok(DbGameObjectVisibilityUpdates {
        create_bodies: create_blocks
            .chunks(CREATURE_UPDATE_CHUNK_SIZE)
            .map(build_update_object_body)
            .collect(),
        destroy_guids: destroy_guids.drain(..).map(ObjectGuid::from_raw).collect(),
    })
}

pub(in crate::world) fn mirror_db_gameobject_visibility_stage(
    session: &mut WorldSessionState,
    stage: MapDbGameObjectVisibilityStage,
    now: Instant,
) -> anyhow::Result<DbGameObjectVisibilityUpdates> {
    let create_guids = stage
        .create_guids
        .iter()
        .map(|guid| guid.raw())
        .collect::<HashSet<_>>();
    let mut create_blocks = Vec::new();
    for runtime in stage.nearby_gameobjects {
        let guid = runtime.guid().raw();
        let should_create = create_guids.contains(&guid);
        if should_create && !runtime.is_consumed(now) {
            create_blocks.push(build_db_gameobject_runtime_create_block_for_quest_statuses(
                &runtime,
                &session.quests.quest_statuses,
            )?);
        }
    }

    Ok(DbGameObjectVisibilityUpdates {
        create_bodies: create_blocks
            .chunks(CREATURE_UPDATE_CHUNK_SIZE)
            .map(build_update_object_body)
            .collect(),
        destroy_guids: stage.destroy_guids,
    })
}

pub(in crate::world) fn gameobject_required_active_quest(
    template: &wow_db::GameObjectTemplateQuery,
) -> Option<u32> {
    let raw = match template.object_type {
        GO_TYPE_CHEST => template.raw_data[8],
        GO_TYPE_GENERIC => template.raw_data[5],
        GO_TYPE_SPELL_FOCUS => template.raw_data[4],
        GO_TYPE_GOOBER => template.raw_data[1],
        _ => 0,
    };
    (raw > 0).then_some(raw)
}

pub(in crate::world) fn should_consume_gameobject_on_use(
    template: &wow_db::GameObjectTemplateQuery,
) -> bool {
    matches!(
        template.object_type,
        GO_TYPE_CHEST | GO_TYPE_GENERIC | GO_TYPE_SPELL_FOCUS | GO_TYPE_GOOBER
    )
}

pub(in crate::world) fn gameobject_type_uses_quest_objective_gate(object_type: u8) -> bool {
    matches!(
        object_type,
        GO_TYPE_CHEST | GO_TYPE_GENERIC | GO_TYPE_SPELL_FOCUS | GO_TYPE_GOOBER
    )
}

pub(in crate::world) fn quest_gameobject_objective_index(
    quest: &wow_db::QuestTemplateQuery,
    gameobject_entry: u32,
) -> Option<usize> {
    quest
        .req_creature_or_go_id
        .iter()
        .position(|entry| *entry < 0 && entry.unsigned_abs() == gameobject_entry)
}

pub(in crate::world) fn session_has_incomplete_gameobject_objective(
    session: &WorldSessionState,
    objective_quests: &[wow_db::QuestTemplateQuery],
    gameobject_entry: u32,
) -> bool {
    objective_quests.iter().any(|quest| {
        let Some(index) = quest_gameobject_objective_index(quest, gameobject_entry) else {
            return false;
        };
        let required = quest.req_creature_or_go_count[index];
        if required == 0 {
            return false;
        }
        let Some(status) = session.quests.quest_statuses.get(&quest.entry) else {
            return false;
        };
        if status.status != QUEST_STATUS_INCOMPLETE || status.rewarded != 0 {
            return false;
        }
        let current = match index {
            0 => status.mobcount1,
            1 => status.mobcount2,
            2 => status.mobcount3,
            3 => status.mobcount4,
            _ => 0,
        };
        current < required
    })
}

pub(in crate::world) async fn handle_gameobject_questgiver_use(
    stream: &mut WorldPacketSink,
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    session: &WorldSessionState,
    guid: ObjectGuid,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    if !guid.is_game_object() {
        return Ok(false);
    }

    let quests = questgiver_visible_quests(object_mgr, world_db_pool, guid, session).await?;
    if quests.is_empty() {
        return Ok(false);
    }
    if quests.len() == 1 {
        let item = &quests[0];
        let displays = quest_reward_item_displays(world_db_pool, &item.quest).await?;
        if item.dialog_status == DIALOG_STATUS_REWARD2
            || item.dialog_status == DIALOG_STATUS_INCOMPLETE
        {
            let status = session.quests.quest_statuses.get(&item.quest.entry);
            let complete = status.is_some_and(|status| {
                quest_status_can_reward_from_inventory(
                    status,
                    &item.quest,
                    &session.inventory.items,
                )
            });
            send_questgiver_completion_response(
                stream,
                world_db_pool,
                guid,
                &item.quest,
                complete,
                true,
                header_crypto,
            )
            .await?;
            return Ok(true);
        }

        let response = build_quest_details_body(guid, &item.quest, &displays);
        send_packet(
            stream,
            WorldOpcode::SmsgQuestgiverQuestDetails as u16,
            &response,
            Some(header_crypto),
        )
        .await?;
        return Ok(true);
    }

    let response = build_questgiver_quest_list_body(guid, &quests);
    send_packet(
        stream,
        WorldOpcode::SmsgQuestgiverQuestList as u16,
        &response,
        Some(header_crypto),
    )
    .await?;
    Ok(true)
}

pub(in crate::world) fn gameobject_chest_loot_id(
    template: &wow_db::GameObjectTemplateQuery,
) -> Option<u32> {
    (template.object_type == GO_TYPE_CHEST && template.raw_data[1] > 0)
        .then_some(template.raw_data[1])
}

pub(in crate::world) fn gameobject_chest_has_loot_id(
    template: &wow_db::GameObjectTemplateQuery,
) -> bool {
    gameobject_chest_loot_id(template).is_some()
}

pub(in crate::world) fn visible_db_gameobject_runtimes(
    gameobjects: &[DbGameObjectRuntime],
    now: Instant,
) -> Vec<DbGameObjectRuntime> {
    gameobjects
        .iter()
        .filter(|gameobject| !gameobject.is_consumed(now))
        .cloned()
        .collect()
}

pub(in crate::world) struct GameObjectUseCreditDeps<'a> {
    pub(in crate::world) character_db_pool: &'a MySqlPool,
    pub(in crate::world) object_mgr: &'a ObjectMgr,
    pub(in crate::world) world_db_pool: &'a MySqlPool,
}

pub(in crate::world) async fn grant_gameobject_use_credit(
    stream: &mut WorldPacketSink,
    deps: GameObjectUseCreditDeps<'_>,
    session: &mut WorldSessionState,
    gameobject_entry: u32,
    gameobject_guid: ObjectGuid,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    let Some(character) = &session.character.active_character else {
        return Ok(false);
    };
    let active_quests: Vec<u32> = session
        .quests
        .quest_statuses
        .values()
        .filter(|status| status.status == QUEST_STATUS_INCOMPLETE && status.rewarded == 0)
        .map(|status| status.quest)
        .collect();
    let mut any_updates = false;
    for quest_id in active_quests {
        let Some(quest) = deps
            .object_mgr
            .quest_template(deps.world_db_pool, quest_id)
            .await?
        else {
            continue;
        };
        let Some(index) = quest_gameobject_objective_index(&quest, gameobject_entry) else {
            continue;
        };
        let required = quest.req_creature_or_go_count[index];
        if required == 0 {
            continue;
        }
        let Some(current_status) = session.quests.quest_statuses.get(&quest_id) else {
            continue;
        };
        let current = match index {
            0 => current_status.mobcount1,
            1 => current_status.mobcount2,
            2 => current_status.mobcount3,
            3 => current_status.mobcount4,
            _ => 0,
        };
        if current >= required {
            continue;
        }
        let new_count = (current + 1).min(required);
        let complete = new_count >= required;
        let updated_status = wow_db::update_character_quest_mob_count(
            deps.character_db_pool,
            character.guid,
            quest_id,
            index,
            new_count,
            complete,
        )
        .await?;
        session
            .quests
            .quest_statuses
            .insert(quest_id, updated_status.clone());
        let Some(slot) = quest_log_slot_for_quest(session, quest_id) else {
            continue;
        };
        send_packet(
            stream,
            WorldOpcode::SmsgQuestUpdateAddKill as u16,
            &build_quest_update_add_kill_body(&quest, gameobject_guid, index, new_count),
            Some(&mut *header_crypto),
        )
        .await?;
        send_packet(
            stream,
            WorldOpcode::SmsgUpdateObject as u16,
            &build_player_quest_log_update_body(character.guid, slot, &updated_status)?,
            Some(&mut *header_crypto),
        )
        .await?;
        if complete {
            send_packet(
                stream,
                WorldOpcode::SmsgQuestUpdateComplete as u16,
                &quest_id.to_le_bytes(),
                Some(&mut *header_crypto),
            )
            .await?;
        }
        any_updates = true;
    }
    Ok(any_updates)
}
