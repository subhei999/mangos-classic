// CMaNGOS reference: src/game/WorldSession.cpp PlayerLogin and enter-world flow.

async fn handle_player_login(
    stream: &mut WorldPacketSink,
    deps: PlayerLoginDeps<'_>,
    account_id: u32,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
    session: &mut WorldSessionState,
) -> anyhow::Result<()> {
    if body.len() != 8 {
        anyhow::bail!(
            "CMSG_PLAYER_LOGIN payload must be 8 bytes, got {}",
            body.len()
        );
    }

    let guid_raw = u64::from_le_bytes(body.try_into()?);
    let guid = ObjectGuid::from_raw(guid_raw);
    let character_guid = guid.counter();
    let characters = wow_db::get_character_enum_entries(deps.character_db_pool, account_id).await?;
    let Some(character) = characters
        .iter()
        .find(|character| character.guid == character_guid)
    else {
        warn!(
            account_id,
            guid = format_args!("0x{guid_raw:016X}"),
            "Character login rejected: character not found for account"
        );
        send_packet(
            stream,
            SMSG_CHARACTER_LOGIN_FAILED,
            &[CHAR_LOGIN_NO_CHARACTER],
            Some(header_crypto),
        )
        .await?;
        return Ok(());
    };

    if deps
        .online_characters
        .lock()
        .await
        .contains(&character.guid)
    {
        warn!(
            account_id,
            guid = character.guid,
            "Character login rejected: character already loaded"
        );
        send_packet(
            stream,
            SMSG_CHARACTER_LOGIN_FAILED,
            &[CHAR_LOGIN_NO_CHARACTER],
            Some(header_crypto),
        )
        .await?;
        return Ok(());
    }

    info!(
        account_id,
        guid = character.guid,
        name = %character.name,
        map = character.map,
        "Character login selected"
    );
    unregister_active_character(
        deps.online_characters,
        deps.maps,
        deps.sessions,
        deps.session_id,
        session,
    )
    .await;
    deps.online_characters.lock().await.insert(character.guid);
    session.active_character = Some(ActiveCharacter {
        guid: character.guid,
        name: character.name.clone(),
        race: character.race,
        class: character.class,
        level: character.level,
        xp: character.xp,
        position: WorldPosition::new(
            character.map,
            character.position_x,
            character.position_y,
            character.position_z,
            character.orientation,
        ),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
    });
    session.player_visual = Some(PlayerVisualState {
        gender: character.gender,
        player_bytes: character.player_bytes,
        player_bytes2: character.player_bytes2,
        equipment_cache: character.equipment_cache.clone(),
        guildid: character.guildid,
    });
    session.player_flags = character.player_flags;
    session.player_death_state = if character.player_flags & PLAYER_FLAGS_GHOST != 0 {
        PlayerDeathState::Ghost
    } else {
        PlayerDeathState::Alive
    };
    session.player_corpse = if session.player_death_state == PlayerDeathState::Ghost {
        let corpse = wow_db::get_player_corpse(deps.character_db_pool, character.guid).await?;
        corpse.map(player_corpse_runtime_from_query)
    } else {
        None
    };
    if let Some(corpse) = &session.player_corpse {
        deps.maps.upsert_player_corpse(corpse.position.map_id, corpse.clone()).await;
    }
    session.combat_dummy_health = RUST_COMBAT_DUMMY_HEALTH;
    session.combat_dummy_lootable = false;
    session.combat_dummy_looting = false;
    session.combat_dummy_loot_money_available = false;
    session.combat_dummy_loot_item_available = false;
    let login_position = WorldPosition::new(
        character.map,
        character.position_x,
        character.position_y,
        character.position_z,
        character.orientation,
    );
    deps.maps
        .ensure_db_creature_grids_loaded(
            deps.character_db_pool,
            deps.world_db_pool,
            character.map,
            login_position,
            CREATURE_SPAWN_RADIUS_YARDS,
        )
        .await?;
    let nearby_creature_runtimes = deps
        .maps
        .nearby_db_creature_snapshots(
            character.map,
            login_position,
            CREATURE_SPAWN_RADIUS_YARDS,
            CREATURE_SPAWN_LIMIT,
        )
        .await;
    let visible_nearby_creatures = visible_db_creature_runtimes(&nearby_creature_runtimes);
    deps.maps
        .ensure_db_gameobject_grids_loaded(
            deps.world_db_pool,
            character.map,
            login_position,
            CREATURE_SPAWN_RADIUS_YARDS,
        )
        .await?;
    let nearby_gameobject_runtimes = deps
        .maps
        .nearby_db_gameobject_snapshots(
            character.map,
            login_position,
            CREATURE_SPAWN_RADIUS_YARDS,
            CREATURE_SPAWN_LIMIT,
        )
        .await;
    let visible_nearby_gameobjects =
        visible_db_gameobject_runtimes(&nearby_gameobject_runtimes, Instant::now());
    deps.maps
        .ensure_player_corpse_grids_loaded(
            deps.character_db_pool,
            character.map,
            login_position,
            CREATURE_SPAWN_RADIUS_YARDS,
        )
        .await?;
    let nearby_player_corpses = deps
        .maps
        .nearby_player_corpse_snapshots(
            character.map,
            login_position,
            CREATURE_SPAWN_RADIUS_YARDS,
            PLAYER_CORPSE_VISIBILITY_LIMIT,
        )
        .await;
    #[cfg(test)]
    {
        session.db_creatures = nearby_creature_runtimes
            .iter()
            .map(|creature| (creature.guid().raw(), creature.clone()))
            .collect();
    }
    session.player_health = character.health;
    session.player_rage = character.power2.min(POWER_RAGE_DEFAULT);
    session.player_mana = character.power1;
    session.inventory =
        wow_db::get_character_inventory_items(deps.character_db_pool, character.guid).await?;
    session.character_skills =
        wow_db::get_character_skills(deps.character_db_pool, character.guid).await?;
    session.quest_statuses =
        wow_db::get_character_quest_statuses(deps.character_db_pool, character.guid)
            .await?
            .into_iter()
            .map(|status| (status.quest, status))
            .collect();
    let world_stats = wow_db::get_player_world_stats(
        deps.world_db_pool,
        character.race,
        character.class,
        character.level,
    )
    .await?;
    if session.player_mana == 0 {
        session.player_mana = world_stats.max_mana();
    }
    if session.player_health == 0 && session.player_death_state == PlayerDeathState::Alive {
        session.player_health = world_stats.max_health().max(1);
    }
    let equipped_templates = load_equipped_item_templates(deps.world_db_pool, &session.inventory).await?;
    let combat_stats = player_combat_stats_for_values(
        character.class,
        character.level,
        &world_stats,
        &equipped_templates,
    );
    let mut bootstrap_character = character.clone();
    bootstrap_character.health = session.player_health;
    bootstrap_character.power1 = session.player_mana;
    bootstrap_character.power2 = session.player_rage;
    let spells = wow_db::get_character_spells(deps.character_db_pool, character.guid).await?;
    session.active_spells = spells
        .iter()
        .filter(|spell| spell.active != 0 && spell.disabled == 0)
        .map(|spell| spell.spell)
        .collect();
    let tutorial_flags = wow_db::get_tutorial_flags(deps.character_db_pool, account_id).await?;
    let cinematic_sequence = if character.cinematic == 0 {
        cinematic_sequence_for_race(character.race)
    } else {
        None
    };
    if character.cinematic == 0 || character.at_login & AT_LOGIN_FIRST != 0 {
        let rows = wow_db::mark_character_first_login_seen(
            deps.character_db_pool,
            account_id,
            character.guid,
        )
        .await?;
        if rows == 0 {
            warn!(
                account_id,
                guid = character.guid,
                "No character row updated while marking first-login state seen"
            );
        }
    }

    send_enter_world_bootstrap(
        stream,
        EnterWorldBootstrap {
            character_db_pool: deps.character_db_pool,
            character: &bootstrap_character,
            inventory: &session.inventory,
            world_stats: &world_stats,
            equipped_templates: &equipped_templates,
            spells: &spells,
            skills: &session.character_skills,
            quest_statuses: &session.quest_statuses,
            tutorial_flags: &tutorial_flags,
            cinematic_sequence,
            nearby_creatures: &visible_nearby_creatures,
            nearby_gameobjects: &visible_nearby_gameobjects,
            nearby_player_corpses: &nearby_player_corpses,
        },
        Some(header_crypto),
    )
    .await?;
    deps.sessions
        .set_character_guid(deps.session_id, Some(character.guid))
        .await;
    let mut visible_objects = HashSet::new();
    visible_objects.extend(
        visible_nearby_creatures
            .iter()
            .map(DbCreatureRuntime::guid),
    );
    visible_objects.extend(
        visible_nearby_gameobjects
            .iter()
            .map(DbGameObjectRuntime::guid),
    );
    visible_objects.extend(nearby_player_corpses.iter().map(|corpse| corpse.guid));

    let player_runtime = PlayerRuntime {
        guid: character.guid,
        account_id,
        session_id: deps.session_id,
        selected_target: session.selected_target,
        active_combat_target: None,
        active_combat_next_swing_at: None,
        position: login_position,
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
        cell: cell_coord_for_position(login_position),
        visible_objects,
        last_creature_visibility_position: Some(login_position),
        last_gameobject_visibility_position: Some(login_position),
        last_player_corpse_visibility_position: Some(login_position),
        visual: session
            .player_visual
            .clone()
            .ok_or_else(|| anyhow::anyhow!("active player visual missing after login"))?,
        visible_equipment: visible_equipment_for_inventory(
            character.equipment_cache.as_deref(),
            &session.inventory,
        ),
        flags: character.player_flags,
        level: character.level,
        race: character.race,
        class: character.class,
        spirit: world_stats.stats[4],
        gender: character.gender,
        health: session.player_health,
        max_health: world_stats.max_health().max(1),
        power1: session.player_mana,
        max_power1: world_stats.max_mana(),
        power2: session.player_rage,
        player_bytes: character.player_bytes,
        player_bytes2: character.player_bytes2,
        active_spells: session.active_spells.clone(),
        inventory: session.inventory.clone(),
        quest_statuses: session.quest_statuses.clone(),
        active_auras: session.active_auras.clone(),
        base_combat_stats: combat_stats,
        combat_stats,
    };
    let packets = deps.maps.add_player(player_runtime).await?;
    deps.sessions.dispatch(packets).await;

    Ok(())
}

struct PlayerLoginDeps<'a> {
    character_db_pool: &'a MySqlPool,
    world_db_pool: &'a MySqlPool,
    online_characters: &'a OnlineCharacters,
    maps: &'a Arc<MapRuntimeManager>,
    sessions: &'a Arc<SessionRegistry>,
    session_id: SessionId,
}

