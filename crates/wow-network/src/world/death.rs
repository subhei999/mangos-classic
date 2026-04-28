#[derive(Clone, Copy)]
struct PlayerDeathDeps<'a> {
    character_db_pool: &'a MySqlPool,
    world_db_pool: &'a MySqlPool,
    player_corpses: &'a PlayerCorpses,
    account_id: u32,
}

async fn handle_repop_request(
    stream: &mut TcpStream,
    deps: PlayerDeathDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if session.player_death_state != PlayerDeathState::Corpse {
        return Ok(());
    }
    if session.active_character.is_none() {
        return Ok(());
    }

    let corpse =
        create_or_get_player_corpse(deps.character_db_pool, deps.player_corpses, session).await?;
    let corpse_position = corpse.position;
    let graveyard_position =
        select_repop_graveyard_position(deps.world_db_pool, corpse_position).await?;

    session.player_death_state = PlayerDeathState::Ghost;
    session.player_health = PLAYER_SURVIVOR_HEALTH_FLOOR;
    session.player_flags |= PLAYER_FLAGS_GHOST;
    session.player_in_combat = false;
    session.active_combat_target = None;
    session.active_combat_next_swing_at = None;
    session.active_creature_combats.clear();
    if let Some(character) = &mut session.active_character {
        character.position = graveyard_position;
        character.movement_flags = 0;
        character.fall_time = 0;
    }
    session.last_creature_visibility_position = None;
    session.last_player_corpse_visibility_position = None;

    let character_guid = session.active_character.as_ref().map(|c| c.guid).unwrap_or_default();
    let player = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_death_update_body(
            player,
            session.player_health,
            session.player_flags,
            0,
            player_unit_flags(false),
        )?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_update_object_body(&[build_player_corpse_create_block(&corpse)?]),
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_CORPSE_RECLAIM_DELAY,
        &build_corpse_reclaim_delay_body(CORPSE_RECLAIM_DELAY_MILLIS),
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        MSG_MOVE_TELEPORT_ACK,
        &build_near_teleport_ack_body(session.active_character.as_ref().unwrap(), 0)?,
        Some(&mut *header_crypto),
    )
    .await?;
    stream_newly_visible_db_creatures(
        stream,
        deps.character_db_pool,
        deps.world_db_pool,
        session,
        header_crypto,
    )
    .await?;
    persist_player_death_state(deps.character_db_pool, deps.account_id, session).await
}

async fn select_repop_graveyard_position(
    world_db_pool: &MySqlPool,
    corpse_position: WorldPosition,
) -> anyhow::Result<WorldPosition> {
    let linked_graveyard = wow_db::get_closest_graveyard(
        world_db_pool,
        corpse_position.map_id,
        corpse_position.x,
        corpse_position.y,
        corpse_position.z,
        ALLIANCE_FACTION,
    )
    .await?;
    let spirit_healer = wow_db::get_closest_spirit_healer(
        world_db_pool,
        corpse_position.map_id,
        corpse_position.x,
        corpse_position.y,
        corpse_position.z,
    )
    .await?;

    let linked_position = linked_graveyard.as_ref().map(graveyard_query_position);
    let spirit_position = spirit_healer.as_ref().map(graveyard_query_position);
    if let Some(spirit_position) = spirit_position {
        let spirit_distance = distance_2d(
            corpse_position.x,
            corpse_position.y,
            spirit_position.x,
            spirit_position.y,
        );
        let linked_distance = linked_position
            .map(|position| {
                distance_2d(
                    corpse_position.x,
                    corpse_position.y,
                    position.x,
                    position.y,
                )
            })
            .unwrap_or(f32::MAX);
        if spirit_distance <= GRAVEYARD_SPIRIT_HEALER_FALLBACK_RADIUS_YARDS
            && spirit_distance < linked_distance
        {
            return Ok(spirit_position);
        }
    }

    Ok(linked_position.unwrap_or(corpse_position))
}

fn graveyard_query_position(graveyard: &wow_db::GraveyardQuery) -> WorldPosition {
    WorldPosition::new(graveyard.map, graveyard.x, graveyard.y, graveyard.z, graveyard.o)
}

async fn handle_corpse_query(
    stream: &mut TcpStream,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let corpse_position = (session.player_death_state == PlayerDeathState::Ghost)
        .then_some(session.player_corpse.as_ref().map(|corpse| corpse.position))
        .flatten();
    send_packet(
        stream,
        MSG_CORPSE_QUERY as u16,
        &build_corpse_query_body(corpse_position),
        Some(header_crypto),
    )
    .await
}

async fn handle_reclaim_corpse(
    stream: &mut TcpStream,
    deps: PlayerDeathDeps<'_>,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if session.player_death_state != PlayerDeathState::Ghost {
        return Ok(());
    }
    if body.len() >= 8 {
        let _requested_corpse = read_packet_guid(body, "CMSG_RECLAIM_CORPSE").ok();
    }
    let Some(character) = &mut session.active_character else {
        return Ok(());
    };
    let Some(corpse) = session.player_corpse.as_ref() else {
        return Ok(());
    };
    let corpse_position = corpse.position;
    if character.position.map_id != corpse_position.map_id {
        return Ok(());
    }
    if distance_2d(
        character.position.x,
        character.position.y,
        corpse_position.x,
        corpse_position.y,
    ) > CORPSE_RECLAIM_RADIUS_YARDS
    {
        return Ok(());
    }

    resurrect_player_at_position(
        stream,
        deps,
        session,
        header_crypto,
        corpse_position,
    )
    .await
}

async fn handle_spirit_healer_activate(
    stream: &mut TcpStream,
    deps: PlayerDeathDeps<'_>,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if session.player_death_state != PlayerDeathState::Ghost {
        return Ok(());
    }
    let healer_guid = read_packet_guid(body, "CMSG_SPIRIT_HEALER_ACTIVATE")?;
    if !healer_guid.is_creature() {
        return Ok(());
    }
    let Some(character) = session.active_character.as_ref() else {
        return Ok(());
    };
    let character_position = character.position;
    let Some(healer) = session.db_creatures.get(&healer_guid.raw()) else {
        warn!(
            guid = format_args!("0x{:016X}", healer_guid.raw()),
            "Ignoring spirit healer activation for unloaded creature"
        );
        return Ok(());
    };
    if !is_spirit_healer_creature(healer) {
        warn!(
            guid = format_args!("0x{:016X}", healer_guid.raw()),
            entry = healer.spawn.entry,
            "Ignoring spirit healer activation for non-healer creature"
        );
        return Ok(());
    }
    if character_position.map_id != healer.current_position.map_id {
        return Ok(());
    }
    if distance_2d(
        character_position.x,
        character_position.y,
        healer.current_position.x,
        healer.current_position.y,
    ) > SPIRIT_HEALER_INTERACTION_RADIUS_YARDS
    {
        return Ok(());
    }

    resurrect_player_at_position(
        stream,
        deps,
        session,
        header_crypto,
        character_position,
    )
    .await
}

fn is_spirit_healer_creature(creature: &DbCreatureRuntime) -> bool {
    creature.spawn.entry == SPIRIT_HEALER_ENTRY
        || creature.spawn.template.npc_flags & UNIT_NPC_FLAG_SPIRITHEALER != 0
}

async fn resurrect_player_at_position(
    stream: &mut TcpStream,
    deps: PlayerDeathDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
    position: WorldPosition,
) -> anyhow::Result<()> {
    let Some(character_snapshot) = session.active_character.as_ref() else {
        return Ok(());
    };
    let (race, class, level) = (
        character_snapshot.race,
        character_snapshot.class,
        character_snapshot.level,
    );
    let world_stats = wow_db::get_player_world_stats(deps.world_db_pool, race, class, level).await?;
    let resurrected_health = (world_stats.max_health().max(1) / 2).max(1);
    let Some(character) = &mut session.active_character else {
        return Ok(());
    };
    session.player_death_state = PlayerDeathState::Alive;
    let corpse_to_bones = session.player_corpse.take();
    session.player_health = resurrected_health;
    session.player_flags &= !PLAYER_FLAGS_GHOST;
    character.position = position;
    character.movement_flags = 0;
    character.fall_time = 0;

    let player = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_death_update_body(
            player,
            session.player_health,
            session.player_flags,
            0,
            player_unit_flags(false),
        )?,
        Some(&mut *header_crypto),
    )
    .await?;
    if let Some(corpse) = corpse_to_bones {
        wow_db::delete_player_corpse(deps.character_db_pool, character.guid).await?;
        let bones = player_bones_runtime_from_corpse(corpse);
        deps.player_corpses
            .lock()
            .await
            .insert(character.guid, bones.clone());
        session
            .visible_player_corpses
            .insert(bones.guid.raw(), bones.clone());
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &build_player_corpse_bones_update_body(&bones)?,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    send_packet(
        stream,
        MSG_MOVE_TELEPORT_ACK,
        &build_near_teleport_ack_body(character, 0)?,
        Some(header_crypto),
    )
    .await?;
    persist_player_death_state(deps.character_db_pool, deps.account_id, session).await
}

async fn kill_player_from_creature(
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
    account_id: u32,
    session: &mut WorldSessionState,
    player: ObjectGuid,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if session.player_death_state != PlayerDeathState::Alive {
        return Ok(());
    }
    if session.active_character.is_none() {
        return Ok(());
    }
    session.player_death_state = PlayerDeathState::Corpse;
    session.player_corpse = None;
    session.player_health = 0;
    session.player_in_combat = false;
    session.active_combat_target = None;
    session.active_combat_next_swing_at = None;
    session.active_creature_combats.clear();

    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_death_update_body(
            player,
            0,
            session.player_flags,
            PLAYER_FIELD_BYTE_RELEASE_TIMER,
            player_unit_flags(false),
        )?,
        Some(&mut *header_crypto),
    )
    .await?;
    persist_player_death_state(character_db_pool, account_id, session).await
}

async fn create_or_get_player_corpse(
    character_db_pool: &MySqlPool,
    player_corpses: &PlayerCorpses,
    session: &mut WorldSessionState,
) -> anyhow::Result<PlayerCorpseRuntime> {
    if let Some(corpse) = &session.player_corpse {
        return Ok(corpse.clone());
    }
    let Some(character) = session.active_character.as_ref() else {
        anyhow::bail!("cannot create player corpse without an active character");
    };
    let corpse = player_corpse_runtime_from_active_character(character, session);
    wow_db::save_player_corpse(
        character_db_pool,
        &NewPlayerCorpse {
            guid: corpse.guid.counter(),
            player: character.guid,
            position: corpse.position,
            time: current_unix_epoch_secs_u64(),
            corpse_type: PLAYER_CORPSE_TYPE_RESURRECTABLE_PVE,
            instance: 0,
        },
    )
    .await?;
    player_corpses
        .lock()
        .await
        .insert(character.guid, corpse.clone());
    session
        .visible_player_corpses
        .insert(corpse.guid.raw(), corpse.clone());
    session.player_corpse = Some(corpse.clone());
    Ok(corpse)
}

fn player_corpse_runtime_from_active_character(
    character: &ActiveCharacter,
    session: &WorldSessionState,
) -> PlayerCorpseRuntime {
    let visual = session.player_visual.as_ref();
    PlayerCorpseRuntime {
        guid: ObjectGuid::new(HighGuid::Corpse, 0, character.guid),
        owner: ObjectGuid::new(HighGuid::Player, 0, character.guid),
        position: character.position,
        corpse_type: PLAYER_CORPSE_TYPE_RESURRECTABLE_PVE,
        race: character.race,
        class: character.class,
        gender: visual.map(|visual| visual.gender).unwrap_or(0),
        player_bytes: visual.map(|visual| visual.player_bytes).unwrap_or_default(),
        player_bytes2: visual.map(|visual| visual.player_bytes2).unwrap_or_default(),
        equipment_cache: Some(equipment_cache_for_corpse(
            visual.and_then(|visual| visual.equipment_cache.as_deref()),
            &session.inventory,
        )),
        guildid: visual.and_then(|visual| visual.guildid),
        player_flags: session.player_flags,
    }
}

fn player_bones_runtime_from_corpse(mut corpse: PlayerCorpseRuntime) -> PlayerCorpseRuntime {
    corpse.corpse_type = PLAYER_CORPSE_TYPE_BONES;
    corpse.player_flags &= !(PLAYER_FLAGS_HIDE_HELM | PLAYER_FLAGS_HIDE_CLOAK);
    corpse
}

fn player_corpse_runtime_from_query(corpse: PlayerCorpseQuery) -> PlayerCorpseRuntime {
    PlayerCorpseRuntime {
        guid: ObjectGuid::new(HighGuid::Corpse, 0, corpse.guid),
        owner: ObjectGuid::new(HighGuid::Player, 0, corpse.player),
        position: WorldPosition::new(
            corpse.map,
            corpse.position_x,
            corpse.position_y,
            corpse.position_z,
            corpse.orientation,
        ),
        corpse_type: corpse.corpse_type,
        race: corpse.race,
        class: corpse.class,
        gender: corpse.gender,
        player_bytes: corpse.player_bytes,
        player_bytes2: corpse.player_bytes2,
        equipment_cache: corpse.equipment_cache,
        guildid: corpse.guildid,
        player_flags: corpse.player_flags,
    }
}

fn equipment_cache_for_corpse(
    equipment_cache: Option<&str>,
    inventory: &[CharacterInventoryItem],
) -> String {
    let mut equipment = parse_equipment_cache(equipment_cache);
    for item in inventory {
        if item.bag == INVENTORY_SLOT_BAG_0 as u32 && item.slot < EQUIPMENT_SLOT_END {
            equipment[item.slot as usize] = item.item_template;
        }
    }

    equipment
        .iter()
        .map(|item| format!("{item} 0"))
        .collect::<Vec<_>>()
        .join(" ")
}

async fn persist_player_death_state(
    character_db_pool: &MySqlPool,
    account_id: u32,
    session: &WorldSessionState,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        return Ok(());
    };
    let rows = wow_db::update_character_death_state(
        character_db_pool,
        account_id,
        character.guid,
        character.position,
        session.player_health,
        session.player_flags,
    )
    .await?;
    if rows == 0 {
        warn!(
            guid = character.guid,
            "No character row updated while persisting player death state"
        );
    }
    Ok(())
}
