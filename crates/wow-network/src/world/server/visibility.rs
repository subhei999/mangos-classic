// CMaNGOS reference: src/game/Maps/Map.cpp object visibility streaming.

async fn stream_newly_visible_db_creatures(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    maps: &Arc<MapRuntimeManager>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        return Ok(());
    };
    let guid = character.guid;
    let name = character.name.clone();
    let position = character.position;
    if !maps
        .should_rescan_player_creature_visibility(position.map_id, guid, position)
        .await
    {
        return Ok(());
    }

    maps.ensure_db_creature_grids_loaded(
        character_db_pool,
        world_db_pool,
        position.map_id,
        position,
        CREATURE_SPAWN_RADIUS_YARDS,
    )
    .await?;
    let nearby_creature_runtimes = maps
        .nearby_db_creature_snapshots(
            position.map_id,
            position,
            CREATURE_SPAWN_RADIUS_YARDS,
            CREATURE_SPAWN_LIMIT,
        )
        .await;
    let visibility_stage = maps
        .stage_player_db_creature_visibility(
            position.map_id,
            guid,
            position,
            nearby_creature_runtimes,
        )
        .await;
    let visibility_updates =
        mirror_db_creature_visibility_stage(session, visibility_stage)?;
    if visibility_updates.create_bodies.is_empty() && visibility_updates.destroy_guids.is_empty() {
        return Ok(());
    }

    debug!(
        guid,
        name = %name,
        tracked_creatures = visibility_updates.tracked_creature_count,
        alive_creatures = visibility_updates.alive_count,
        corpse_creatures = visibility_updates.corpse_count,
        dead_creatures = visibility_updates.dead_count,
        create_objects = visibility_updates.create_count,
        create_packets = visibility_updates.create_bodies.len(),
        destroy_count = visibility_updates.destroy_guids.len(),
        create_bytes = visibility_updates.create_bodies.iter().map(Vec::len).sum::<usize>(),
        "Updating DB creature visibility after movement"
    );
    for destroy_guid in visibility_updates.destroy_guids {
        let body = build_destroy_guid_body(destroy_guid);
        send_packet(
            stream,
            SMSG_DESTROY_OBJECT,
            &body,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    for body in visibility_updates.create_bodies {
        send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(&mut *header_crypto)).await?;
    }
    Ok(())
}

async fn stream_nearby_player_corpses(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    maps: &Arc<MapRuntimeManager>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.active_character.as_ref() else {
        return Ok(());
    };
    let character_guid = character.guid;
    let position = character.position;
    if !maps
        .should_rescan_player_corpse_visibility(position.map_id, character_guid, position)
        .await
    {
        return Ok(());
    }
    maps.ensure_player_corpse_grids_loaded(
        character_db_pool,
        position.map_id,
        position,
        CREATURE_SPAWN_RADIUS_YARDS,
    )
    .await?;
    let nearby_corpses = maps
        .nearby_player_corpse_snapshots(
            position.map_id,
            position,
            CREATURE_SPAWN_RADIUS_YARDS,
            PLAYER_CORPSE_VISIBILITY_LIMIT,
        )
        .await;
    let stage = maps
        .stage_player_corpse_visibility(position.map_id, character_guid, position, nearby_corpses)
        .await;
    let create_guids = stage
        .create_guids
        .iter()
        .map(|guid| guid.raw())
        .collect::<HashSet<_>>();
    let create_blocks = stage
        .nearby_corpses
        .iter()
        .filter(|corpse| create_guids.contains(&corpse.guid.raw()))
        .map(build_player_corpse_create_block)
        .collect::<anyhow::Result<Vec<_>>>()?;

    for guid in stage.destroy_guids {
        send_packet(
            stream,
            SMSG_DESTROY_OBJECT,
            &build_destroy_guid_body(guid),
            Some(&mut *header_crypto),
        )
        .await?;
    }
    if !create_blocks.is_empty() {
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &build_update_object_body(&create_blocks),
            Some(header_crypto),
        )
        .await?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(dead_code)]
fn should_rescan_player_corpse_visibility(
    session: &WorldSessionState,
    position: WorldPosition,
) -> bool {
    let Some(previous) = session.last_player_corpse_visibility_position else {
        return true;
    };
    if previous.map_id != position.map_id {
        return true;
    }
    distance_squared_2d(previous.x, previous.y, position.x, position.y)
        >= CREATURE_VISIBILITY_RESCAN_DISTANCE_YARDS * CREATURE_VISIBILITY_RESCAN_DISTANCE_YARDS
}

#[cfg(test)]
fn should_rescan_db_creature_visibility(
    session: &WorldSessionState,
    position: WorldPosition,
) -> bool {
    let Some(previous) = session.last_creature_visibility_position else {
        return true;
    };
    if previous.map_id != position.map_id {
        return true;
    }
    let dx = previous.x - position.x;
    let dy = previous.y - position.y;
    dx * dx + dy * dy
        >= CREATURE_VISIBILITY_RESCAN_DISTANCE_YARDS * CREATURE_VISIBILITY_RESCAN_DISTANCE_YARDS
}

#[derive(Debug, Default)]
struct DbCreatureVisibilityUpdates {
    create_bodies: Vec<Vec<u8>>,
    destroy_guids: Vec<ObjectGuid>,
    create_count: usize,
    tracked_creature_count: usize,
    alive_count: usize,
    corpse_count: usize,
    dead_count: usize,
}

#[cfg(test)]
fn stage_db_creature_visibility_updates(
    session: &mut WorldSessionState,
    position: WorldPosition,
    nearby_creatures: Vec<DbCreatureRuntime>,
) -> anyhow::Result<DbCreatureVisibilityUpdates> {
    let nearby_guids = nearby_creatures
        .iter()
        .map(|creature| creature.guid().raw())
        .collect::<HashSet<_>>();
    let mut retained_combat_guids = HashSet::new();
    if let Some(target) = session.active_combat_target {
        if session.db_creatures.contains_key(&target.raw()) {
            retained_combat_guids.insert(target.raw());
        }
    }
    for combat in session.active_creature_combats.values() {
        if session.db_creatures.contains_key(&combat.attacker.raw()) {
            retained_combat_guids.insert(combat.attacker.raw());
        }
    }
    let mut destroy_guids = session
        .db_creatures
        .iter()
        .filter(|(guid, creature)| {
            creature.client_visible
                && !nearby_guids.contains(guid)
                && !retained_combat_guids.contains(guid)
                && !is_db_creature_inside_unload_radius(creature, position)
        })
        .map(|(guid, _)| *guid)
        .collect::<Vec<_>>();
    for guid in &destroy_guids {
        if session
            .db_creatures
            .get(guid)
            .is_some_and(|creature| creature.life_state == DbCreatureLifeState::Alive)
        {
            session.db_creatures.remove(guid);
        } else if let Some(creature) = session.db_creatures.get_mut(guid) {
            creature.client_visible = false;
        }
    }
    if session
        .active_combat_target
        .is_some_and(|target| destroy_guids.contains(&target.raw()))
    {
        session.active_combat_target = None;
        session.active_combat_next_swing_at = None;
    }
    session
        .active_creature_combats
        .retain(|guid, _| !destroy_guids.contains(guid));

    let character_guid = session.active_character.as_ref().map(|character| character.guid);
    let mut create_blocks = Vec::new();
    let mut create_guids = Vec::new();
    for runtime in nearby_creatures {
        let creature_raw_guid = runtime.guid().raw();
        if let Some(creature) = session.db_creatures.get_mut(&creature_raw_guid) {
            if creature.life_state != DbCreatureLifeState::Alive
                && runtime.life_state == DbCreatureLifeState::Alive
            {
                if !creature.client_visible && creature.life_state != DbCreatureLifeState::Dead {
                    creature.client_visible = true;
                    create_guids.push(creature.guid());
                    create_blocks.push(build_db_creature_runtime_create_block_for_player(
                        creature,
                        character_guid,
                    )?);
                }
                continue;
            }
            let was_visible = creature.client_visible;
            let became_dead = runtime.life_state == DbCreatureLifeState::Dead && was_visible;
            *creature = runtime;
            creature.client_visible = was_visible && !became_dead;
            if became_dead {
                if !destroy_guids.contains(&creature_raw_guid) {
                    destroy_guids.push(creature_raw_guid);
                }
                continue;
            }
            if !was_visible && creature.life_state != DbCreatureLifeState::Dead {
                creature.client_visible = true;
                create_guids.push(creature.guid());
                create_blocks.push(build_db_creature_runtime_create_block_for_player(
                    creature,
                    character_guid,
                )?);
            }
            continue;
        }
        if runtime.life_state != DbCreatureLifeState::Dead {
            create_guids.push(runtime.guid());
            create_blocks.push(build_db_creature_runtime_create_block_for_player(
                &runtime,
                character_guid,
            )?);
        }
        session.db_creatures.insert(creature_raw_guid, runtime);
    }

    let create_count = create_blocks.len();
    let tracked_creature_count = session.db_creatures.len();
    let alive_count = session
        .db_creatures
        .values()
        .filter(|creature| creature.life_state == DbCreatureLifeState::Alive)
        .count();
    let corpse_count = session
        .db_creatures
        .values()
        .filter(|creature| creature.life_state == DbCreatureLifeState::Corpse)
        .count();
    let dead_count = session
        .db_creatures
        .values()
        .filter(|creature| creature.life_state == DbCreatureLifeState::Dead)
        .count();
    Ok(DbCreatureVisibilityUpdates {
        create_bodies: create_blocks
            .chunks(CREATURE_UPDATE_CHUNK_SIZE)
            .map(build_update_object_body)
            .collect(),
        destroy_guids: destroy_guids
            .into_iter()
            .map(ObjectGuid::from_raw)
            .collect(),
        create_count,
        tracked_creature_count,
        alive_count,
        corpse_count,
        dead_count,
    })
}

fn is_db_creature_inside_unload_radius(
    creature: &DbCreatureRuntime,
    position: WorldPosition,
) -> bool {
    is_db_creature_inside_radius(creature, position, CREATURE_VISIBILITY_UNLOAD_RADIUS_YARDS)
}

fn is_db_creature_inside_visibility_radius(
    creature: &DbCreatureRuntime,
    position: WorldPosition,
) -> bool {
    is_db_creature_inside_radius(creature, position, CREATURE_SPAWN_RADIUS_YARDS)
}

fn is_db_creature_inside_radius(
    creature: &DbCreatureRuntime,
    position: WorldPosition,
    radius: f32,
) -> bool {
    is_position_inside_radius(creature.current_position, position, radius)
}

fn is_position_inside_radius(
    object_position: WorldPosition,
    position: WorldPosition,
    radius: f32,
) -> bool {
    if object_position.map_id != position.map_id {
        return false;
    }
    let dx = object_position.x - position.x;
    let dy = object_position.y - position.y;
    dx * dx + dy * dy <= radius * radius
}

fn distance_squared_2d(left_x: f32, left_y: f32, right_x: f32, right_y: f32) -> f32 {
    let dx = left_x - right_x;
    let dy = left_y - right_y;
    dx * dx + dy * dy
}

fn build_destroy_guid_body(guid: ObjectGuid) -> Vec<u8> {
    guid.raw().to_le_bytes().to_vec()
}

fn mirror_db_creature_visibility_stage(
    session: &mut WorldSessionState,
    stage: MapDbCreatureVisibilityStage,
) -> anyhow::Result<DbCreatureVisibilityUpdates> {
    let create_guids = stage.create_guids.iter().map(|guid| guid.raw()).collect::<HashSet<_>>();
    let character_guid = session.active_character.as_ref().map(|character| character.guid);

    let mut create_blocks = Vec::new();
    for runtime in &stage.nearby_creatures {
        let guid = runtime.guid().raw();
        if create_guids.contains(&guid) {
            create_blocks.push(build_db_creature_runtime_create_block_for_player(
                runtime,
                character_guid,
            )?);
        }
    }

    let create_count = create_blocks.len();
    let tracked_creature_count = stage.nearby_creatures.len();
    let alive_count = stage
        .nearby_creatures
        .iter()
        .filter(|creature| creature.life_state == DbCreatureLifeState::Alive)
        .count();
    let corpse_count = stage
        .nearby_creatures
        .iter()
        .filter(|creature| creature.life_state == DbCreatureLifeState::Corpse)
        .count();
    let dead_count = stage
        .nearby_creatures
        .iter()
        .filter(|creature| creature.life_state == DbCreatureLifeState::Dead)
        .count();
    Ok(DbCreatureVisibilityUpdates {
        create_bodies: create_blocks
            .chunks(CREATURE_UPDATE_CHUNK_SIZE)
            .map(build_update_object_body)
            .collect(),
        destroy_guids: stage.destroy_guids,
        create_count,
        tracked_creature_count,
        alive_count,
        corpse_count,
        dead_count,
    })
}
