// CMaNGOS reference: src/game/Entities/QueryHandler.cpp and SpellHandler.cpp
// gameobject query/use flow.

async fn handle_gameobject_query(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let query = GameObjectQuery::read(body)?;
    let template = wow_db::get_gameobject_template_query(world_db_pool, query.entry).await?;
    let response = build_gameobject_query_response(query.entry, template.as_ref());
    send_packet(
        stream,
        SMSG_GAMEOBJECT_QUERY_RESPONSE,
        &response,
        Some(header_crypto),
    )
    .await
}

async fn handle_gameobject_use(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.active_character.as_ref() else {
        return Ok(());
    };
    let guid = read_packet_guid(body, "CMSG_GAMEOBJ_USE")?;
    if !guid.is_game_object() {
        return Ok(());
    }
    let Some(runtime) = session.db_gameobjects.get(&guid.raw()).cloned() else {
        return Ok(());
    };
    if runtime.spawn.map != character.position.map_id {
        return Ok(());
    }
    let now = Instant::now();
    if runtime.is_consumed(now) {
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
            .quest_statuses
            .get(&required_quest)
            .is_some_and(|status| status.status == QUEST_STATUS_INCOMPLETE && status.rewarded == 0);
        if !can_use {
            return Ok(());
        }
    }

    let handled_questgiver =
        handle_gameobject_questgiver_use(stream, world_db_pool, session, guid, header_crypto).await?;
    let objective_updated = grant_gameobject_use_credit(
        stream,
        character_db_pool,
        world_db_pool,
        session,
        guid,
        header_crypto,
    )
    .await?;

    if !(handled_questgiver || objective_updated) {
        return Ok(());
    }

    if should_consume_gameobject_on_use(&runtime.spawn.template) {
        if let Some(gameobject) = session.db_gameobjects.get_mut(&guid.raw()) {
            gameobject.mark_consumed(now);
        }
        send_packet(
            stream,
            SMSG_DESTROY_OBJECT,
            &guid.raw().to_le_bytes(),
            Some(header_crypto),
        )
        .await?;
    }

    Ok(())
}

async fn stream_newly_visible_db_gameobjects(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        return Ok(());
    };
    if !should_rescan_db_gameobject_visibility(session, character.position) {
        return Ok(());
    }
    session.last_gameobject_visibility_position = Some(character.position);
    let nearby_gameobjects = wow_db::get_nearby_gameobject_spawns(
        world_db_pool,
        character.position.map_id,
        character.position.x,
        character.position.y,
        CREATURE_SPAWN_RADIUS_YARDS,
        CREATURE_SPAWN_LIMIT,
    )
    .await?;
    let updates =
        stage_db_gameobject_visibility_updates(session, character.position, nearby_gameobjects, Instant::now())?;
    for guid in updates.destroy_guids {
        send_packet(
            stream,
            SMSG_DESTROY_OBJECT,
            &guid.raw().to_le_bytes(),
            Some(&mut *header_crypto),
        )
        .await?;
    }
    for body in updates.create_bodies {
        send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(&mut *header_crypto)).await?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GameObjectQuery {
    entry: u32,
    guid: ObjectGuid,
}

impl GameObjectQuery {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = 0;
        let entry = read_u32(body, &mut cursor)?;
        ensure_available(body, cursor + 8)?;
        let guid = ObjectGuid::from_raw(u64::from_le_bytes(body[cursor..cursor + 8].try_into()?));
        Ok(Self { entry, guid })
    }
}

#[derive(Debug, Default)]
struct DbGameObjectVisibilityUpdates {
    create_bodies: Vec<Vec<u8>>,
    destroy_guids: Vec<ObjectGuid>,
}

fn build_gameobject_query_response(
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

fn should_rescan_db_gameobject_visibility(
    session: &WorldSessionState,
    position: WorldPosition,
) -> bool {
    let Some(previous) = session.last_gameobject_visibility_position else {
        return true;
    };
    if previous.map_id != position.map_id {
        return true;
    }
    distance_squared_2d(previous.x, previous.y, position.x, position.y)
        >= CREATURE_VISIBILITY_RESCAN_DISTANCE_YARDS * CREATURE_VISIBILITY_RESCAN_DISTANCE_YARDS
}

fn stage_db_gameobject_visibility_updates(
    session: &mut WorldSessionState,
    position: WorldPosition,
    nearby_gameobjects: Vec<wow_db::GameObjectSpawnQuery>,
    now: Instant,
) -> anyhow::Result<DbGameObjectVisibilityUpdates> {
    let nearby_guids = nearby_gameobjects
        .iter()
        .map(gameobject_spawn_guid)
        .map(|guid| guid.raw())
        .collect::<HashSet<_>>();

    let mut destroy_guids = session
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
        session.db_gameobjects.remove(guid);
    }

    let mut create_blocks = Vec::new();
    for spawn in nearby_gameobjects {
        let guid = gameobject_spawn_guid(&spawn).raw();
        if let Some(existing) = session.db_gameobjects.get_mut(&guid) {
            existing.spawn = spawn;
            if existing.is_consumed(now) {
                existing.client_visible = false;
                continue;
            }
            if !existing.client_visible {
                existing.client_visible = true;
                create_blocks.push(build_db_gameobject_runtime_create_block(existing)?);
            }
            continue;
        }

        let mut runtime = DbGameObjectRuntime::new(spawn);
        runtime.client_visible = !runtime.is_consumed(now);
        if runtime.client_visible {
            create_blocks.push(build_db_gameobject_runtime_create_block(&runtime)?);
        }
        session.db_gameobjects.insert(guid, runtime);
    }

    Ok(DbGameObjectVisibilityUpdates {
        create_bodies: create_blocks
            .chunks(CREATURE_UPDATE_CHUNK_SIZE)
            .map(build_update_object_body)
            .collect(),
        destroy_guids: destroy_guids
            .drain(..)
            .map(ObjectGuid::from_raw)
            .collect(),
    })
}

fn gameobject_required_active_quest(template: &wow_db::GameObjectTemplateQuery) -> Option<u32> {
    let raw = match template.object_type {
        GO_TYPE_CHEST => template.raw_data[8],
        GO_TYPE_GENERIC => template.raw_data[5],
        GO_TYPE_SPELL_FOCUS => template.raw_data[4],
        GO_TYPE_GOOBER => template.raw_data[1],
        _ => 0,
    };
    (raw > 0).then_some(raw)
}

fn should_consume_gameobject_on_use(template: &wow_db::GameObjectTemplateQuery) -> bool {
    matches!(
        template.object_type,
        GO_TYPE_CHEST | GO_TYPE_GENERIC | GO_TYPE_SPELL_FOCUS | GO_TYPE_GOOBER
    )
}

async fn handle_gameobject_questgiver_use(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    session: &WorldSessionState,
    guid: ObjectGuid,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    if !guid.is_game_object() {
        return Ok(false);
    }
    if let Some(quest) = gameobject_completed_turnin_quest(world_db_pool, guid, session).await? {
        let response = build_quest_offer_reward_body(guid, &quest);
        send_packet(
            stream,
            SMSG_QUESTGIVER_OFFER_REWARD,
            &response,
            Some(header_crypto),
        )
        .await?;
        return Ok(true);
    }

    let quests = gameobject_visible_quests(world_db_pool, guid, session).await?;
    if quests.is_empty() {
        return Ok(false);
    }
    let response = build_questgiver_quest_list_body(guid, &quests);
    send_packet(
        stream,
        SMSG_QUESTGIVER_QUEST_LIST,
        &response,
        Some(header_crypto),
    )
    .await?;
    Ok(true)
}

async fn gameobject_visible_quests(
    world_db_pool: &MySqlPool,
    guid: ObjectGuid,
    session: &WorldSessionState,
) -> anyhow::Result<Vec<QuestTemplateQuery>> {
    if !guid.is_game_object() {
        return Ok(Vec::new());
    }
    let quests = wow_db::get_gameobject_start_quests(world_db_pool, guid.entry()).await?;
    Ok(quests
        .into_iter()
        .filter(|quest| {
            session
                .quest_statuses
                .get(&quest.entry)
                .is_none_or(|status| status.rewarded == 0 && status.status != QUEST_STATUS_COMPLETE)
        })
        .collect())
}

async fn gameobject_completed_turnin_quest(
    world_db_pool: &MySqlPool,
    guid: ObjectGuid,
    session: &WorldSessionState,
) -> anyhow::Result<Option<QuestTemplateQuery>> {
    if !guid.is_game_object() {
        return Ok(None);
    }
    for status in active_quest_statuses_sorted(&session.quest_statuses) {
        if status.status == QUEST_STATUS_COMPLETE
            && wow_db::gameobject_completes_quest(world_db_pool, guid.entry(), status.quest).await?
        {
            return Ok(wow_db::get_quest_template_query(world_db_pool, status.quest).await?);
        }
    }
    Ok(None)
}

async fn grant_gameobject_use_credit(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    session: &mut WorldSessionState,
    gameobject_guid: ObjectGuid,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    let Some(character) = &session.active_character else {
        return Ok(false);
    };
    let Some(gameobject_entry) = session
        .db_gameobjects
        .get(&gameobject_guid.raw())
        .map(|gameobject| gameobject.spawn.entry)
    else {
        return Ok(false);
    };
    let active_quests: Vec<u32> = session
        .quest_statuses
        .values()
        .filter(|status| status.status == QUEST_STATUS_INCOMPLETE && status.rewarded == 0)
        .map(|status| status.quest)
        .collect();
    let mut any_updates = false;
    for quest_id in active_quests {
        let Some(quest) = wow_db::get_quest_template_query(world_db_pool, quest_id).await? else {
            continue;
        };
        let Some(index) = quest
            .req_creature_or_go_id
            .iter()
            .position(|entry| *entry < 0 && entry.unsigned_abs() == gameobject_entry)
        else {
            continue;
        };
        let required = quest.req_creature_or_go_count[index];
        if required == 0 {
            continue;
        }
        let Some(current_status) = session.quest_statuses.get(&quest_id) else {
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
            character_db_pool,
            character.guid,
            quest_id,
            index,
            new_count,
            complete,
        )
        .await?;
        session.quest_statuses.insert(quest_id, updated_status.clone());
        let Some(slot) = quest_log_slot_for_quest(session, quest_id) else {
            continue;
        };
        send_packet(
            stream,
            SMSG_QUESTUPDATE_ADD_KILL,
            &build_quest_update_add_kill_body(&quest, gameobject_guid, index, new_count),
            Some(&mut *header_crypto),
        )
        .await?;
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &build_player_quest_log_update_body(character.guid, slot, &updated_status)?,
            Some(&mut *header_crypto),
        )
        .await?;
        if complete {
            send_packet(
                stream,
                SMSG_QUESTUPDATE_COMPLETE,
                &quest_id.to_le_bytes(),
                Some(&mut *header_crypto),
            )
            .await?;
        }
        any_updates = true;
    }
    Ok(any_updates)
}
