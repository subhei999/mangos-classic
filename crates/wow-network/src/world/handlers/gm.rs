use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum GmDotCommand {
    Gm(Option<bool>),
    LevelUp(i32),
    LevelSet(u8),
    NpcAdd(u32),
    NpcDelete(Option<u32>),
    Die,
}

pub(in crate::world) async fn handle_gm_dot_command(
    stream: &mut WorldPacketSink,
    deps: ChatDeps<'_>,
    message: &str,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let command = match parse_gm_dot_command(message) {
        Some(Ok(command)) => command,
        Some(Err(error)) => {
            send_system_message(stream, &error, header_crypto).await?;
            return Ok(());
        }
        None => {
            send_system_message(stream, "Unknown command.", header_crypto).await?;
            return Ok(());
        }
    };

    match command {
        GmDotCommand::Gm(value) => {
            if !require_gm_security(stream, session, 1, header_crypto).await? {
                return Ok(());
            }
            if let Some(value) = value {
                session.account.gm_mode = value;
            }
            let message = if session.account.gm_mode {
                "GM mode is ON."
            } else {
                "GM mode is OFF."
            };
            send_system_message(stream, message, header_crypto).await?;
        }
        GmDotCommand::NpcAdd(entry) => {
            if !require_gm_security(stream, session, 2, header_crypto).await? {
                return Ok(());
            }
            spawn_gm_creature_from_template(stream, deps, session, entry, header_crypto).await?;
        }
        GmDotCommand::LevelUp(delta) => {
            if !require_gm_security(stream, session, 3, header_crypto).await? {
                return Ok(());
            }
            change_gm_character_level_relative(stream, deps, session, delta, header_crypto).await?;
        }
        GmDotCommand::LevelSet(level) => {
            if !require_gm_security(stream, session, 3, header_crypto).await? {
                return Ok(());
            }
            change_gm_character_level_absolute(stream, deps, session, level, header_crypto).await?;
        }
        GmDotCommand::NpcDelete(db_guid) => {
            if !require_gm_security(stream, session, 2, header_crypto).await? {
                return Ok(());
            }
            delete_gm_creature_runtime(stream, deps, session, db_guid, header_crypto).await?;
        }
        GmDotCommand::Die => {
            if !require_gm_security(stream, session, 3, header_crypto).await? {
                return Ok(());
            }
            kill_selected_db_creature(stream, deps, session, header_crypto).await?;
        }
    }
    Ok(())
}

pub(in crate::world) fn parse_gm_dot_command(
    message: &str,
) -> Option<Result<GmDotCommand, String>> {
    let trimmed = message.trim();
    let without_dot = trimmed.strip_prefix('.')?.trim();
    let normalized = without_dot.to_ascii_lowercase();
    if normalized == "die" || normalized.starts_with("die ") {
        return Some(Ok(GmDotCommand::Die));
    }
    if normalized == "gm" {
        return Some(Ok(GmDotCommand::Gm(None)));
    }
    if let Some(args) = normalized.strip_prefix("gm ") {
        let arg = args.trim();
        return Some(match arg {
            "on" | "1" => Ok(GmDotCommand::Gm(Some(true))),
            "off" | "0" => Ok(GmDotCommand::Gm(Some(false))),
            _ => Err("Syntax: .gm on/off".to_string()),
        });
    }
    if normalized == "levelup" || normalized == "level" {
        return Some(Ok(GmDotCommand::LevelUp(1)));
    }
    if let Some(args) = normalized.strip_prefix("levelup ") {
        return Some(match first_i32(args) {
            Some(delta) => Ok(GmDotCommand::LevelUp(delta)),
            None => Err("Syntax: .levelup [#levels]".to_string()),
        });
    }
    if let Some(args) = normalized.strip_prefix("level ") {
        return Some(match first_i32(args) {
            Some(delta) => Ok(GmDotCommand::LevelUp(delta)),
            None => Err("Syntax: .level [#levels]".to_string()),
        });
    }
    if let Some(args) = normalized.strip_prefix("character level ") {
        return Some(match first_u32(args) {
            Some(level) if level > 0 => Ok(GmDotCommand::LevelSet(
                level.min(u32::from(DEFAULT_MAX_PLAYER_LEVEL)) as u8,
            )),
            _ => Err("Syntax: .character level #level".to_string()),
        });
    }
    if let Some(args) = normalized.strip_prefix("npc add") {
        return Some(match first_u32(args) {
            Some(entry) => Ok(GmDotCommand::NpcAdd(entry)),
            None => Err("Syntax: .npc add #creatureid".to_string()),
        });
    }
    if let Some(args) = normalized.strip_prefix("npc delete") {
        return Some(Ok(GmDotCommand::NpcDelete(first_u32(args))));
    }
    None
}

pub(in crate::world) fn first_u32(input: &str) -> Option<u32> {
    let mut current = String::new();
    for ch in input.chars() {
        if ch.is_ascii_digit() {
            current.push(ch);
        } else if !current.is_empty() {
            return current.parse().ok();
        }
    }
    (!current.is_empty())
        .then(|| current.parse().ok())
        .flatten()
}

pub(in crate::world) fn first_i32(input: &str) -> Option<i32> {
    let trimmed = input.trim_start();
    let mut chars = trimmed.chars();
    let mut current = String::new();
    if matches!(chars.clone().next(), Some('+') | Some('-')) {
        current.push(chars.next()?);
    }
    for ch in chars {
        if ch.is_ascii_digit() {
            current.push(ch);
        } else {
            break;
        }
    }
    (current.chars().any(|ch| ch.is_ascii_digit()))
        .then(|| current.parse().ok())
        .flatten()
}

pub(in crate::world) fn gm_relative_level(old_level: u8, delta: i32) -> u8 {
    (i32::from(old_level) + delta).clamp(1, i32::from(DEFAULT_MAX_PLAYER_LEVEL)) as u8
}

pub(in crate::world) async fn require_gm_security(
    stream: &mut WorldPacketSink,
    session: &WorldSessionState,
    required_security: u8,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    if session.account.account_security >= required_security {
        return Ok(true);
    }
    send_system_message(
        stream,
        "You do not have permission to use that command.",
        header_crypto,
    )
    .await?;
    Ok(false)
}

pub(in crate::world) async fn spawn_gm_creature_from_template(
    stream: &mut WorldPacketSink,
    deps: ChatDeps<'_>,
    session: &WorldSessionState,
    entry: u32,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.clone() else {
        return Ok(());
    };
    let Some(template) = wow_db::get_creature_template_query(deps.world_db_pool, entry).await?
    else {
        send_system_message(
            stream,
            &format!("Creature template {entry} was not found."),
            header_crypto,
        )
        .await?;
        return Ok(());
    };
    let spawn = CreatureSpawnQuery {
        guid: 0,
        entry,
        map: character.position.map_id,
        game_event: None,
        addon_emote: 0,
        position_x: character.position.x,
        position_y: character.position.y,
        position_z: character.position.z,
        orientation: character.position.orientation,
        spawn_time_secs_min: 0,
        spawn_time_secs_max: 0,
        spawn_dist: 0.0,
        movement_type: 0,
        formation_waypoint_path_id: None,
        template,
        waypoint_path: Vec::new(),
    };
    let (creature, observer_packets) = deps
        .maps
        .spawn_gm_db_creature(spawn, Some(character.guid))
        .await?;
    deps.sessions.dispatch(observer_packets).await;
    let create_body =
        build_update_object_body(&[build_db_creature_runtime_create_block(&creature)?]);
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &create_body,
        Some(&mut *header_crypto),
    )
    .await?;
    send_system_message(
        stream,
        &format!(
            "Spawned {} ({}) with guid {}.",
            creature.spawn.template.name,
            entry,
            creature.guid().counter()
        ),
        header_crypto,
    )
    .await
}

pub(in crate::world) async fn change_gm_character_level_relative(
    stream: &mut WorldPacketSink,
    deps: ChatDeps<'_>,
    session: &mut WorldSessionState,
    delta: i32,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.clone() else {
        return Ok(());
    };
    let old_level = character.level;
    let new_level = gm_relative_level(old_level, delta);
    change_gm_character_level(stream, deps, session, old_level, new_level, header_crypto).await
}

pub(in crate::world) async fn change_gm_character_level_absolute(
    stream: &mut WorldPacketSink,
    deps: ChatDeps<'_>,
    session: &mut WorldSessionState,
    new_level: u8,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let old_level = character.level;
    change_gm_character_level(stream, deps, session, old_level, new_level, header_crypto).await
}

pub(in crate::world) async fn change_gm_character_level(
    stream: &mut WorldPacketSink,
    deps: ChatDeps<'_>,
    session: &mut WorldSessionState,
    old_level: u8,
    new_level: u8,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.clone() else {
        return Ok(());
    };
    if new_level == old_level {
        send_system_message(
            stream,
            &format!("Level remains {new_level}."),
            header_crypto,
        )
        .await?;
        return Ok(());
    }

    let previous_stats = wow_db::get_player_world_stats(
        deps.world_db_pool,
        character.race,
        character.class,
        old_level,
    )
    .await?;
    let new_stats = wow_db::get_player_world_stats(
        deps.world_db_pool,
        character.race,
        character.class,
        new_level,
    )
    .await?;
    let health = new_stats.max_health().max(1);
    let power1 = new_stats.max_mana();
    let power2 = session.character.player_rage.min(POWER_RAGE_DEFAULT);
    let power3 = 0;
    let power4 = create_power_for_class_power(character.class, POWER_ENERGY);
    let power5 = 0;
    let xp = 0;

    wow_db::update_character_progression_state(
        deps.character_db_pool,
        character.guid,
        wow_db::CharacterProgressionState {
            level: new_level,
            xp,
            health,
            power1,
            power2,
            power3,
            power4,
            power5,
        },
    )
    .await?;

    if let Some(active) = session.character.active_character.as_mut() {
        active.level = new_level;
        active.xp = xp;
    }
    session.character.player_health = health;
    session.character.player_mana = power1;
    session.character.player_rage = power2;
    session.character.player_energy = power4;

    let skill_cap_updates = sync_player_level_backed_skills(
        deps.maps,
        character.race,
        character.class,
        new_level,
        &mut session.character.character_skills,
    );
    for updated in &skill_cap_updates {
        wow_db::upsert_character_skill(
            deps.character_db_pool,
            character.guid,
            updated.skill,
            updated.value,
            updated.max,
        )
        .await?;
    }

    let equipped_templates =
        load_equipped_item_templates(deps.world_db_pool, &session.inventory.items).await?;
    let combat_stats =
        player_combat_stats_for_values(character.class, new_level, &new_stats, &equipped_templates);
    deps.maps
        .update_player_level_progression_state(
            character.position.map_id,
            character.guid,
            PlayerLevelProgressionRuntimeUpdate {
                level: new_level,
                xp,
                health,
                power1,
                power2,
                power4,
                world_stats: new_stats,
                combat_stats,
            },
        )
        .await;

    send_packet(
        stream,
        SMSG_LEVELUP_INFO,
        &build_levelup_info_body(new_level, &previous_stats, &new_stats),
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_progression_update_body(PlayerProgressionUpdate {
            character_guid: character.guid,
            level: new_level,
            xp,
            health,
            power1,
            power2,
            power3,
            power4,
            power5,
            world_stats: &new_stats,
        })?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_combat_stats_update_body(character.guid, &combat_stats)?,
        Some(&mut *header_crypto),
    )
    .await?;
    if !skill_cap_updates.is_empty() {
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &build_player_skill_updates_body(
                character.guid,
                &skill_cap_updates,
                &session.auras.active_auras,
            )?,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    let direction = if new_level > old_level {
        "increased"
    } else {
        "decreased"
    };
    send_system_message(
        stream,
        &format!("Your level {direction} from {old_level} to {new_level}."),
        header_crypto,
    )
    .await
}

pub(in crate::world) async fn kill_selected_db_creature(
    stream: &mut WorldPacketSink,
    deps: ChatDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.clone() else {
        return Ok(());
    };
    let Some(target) = session.character.selected_target else {
        send_system_message(stream, "Select a creature first.", header_crypto).await?;
        return Ok(());
    };
    if !target.is_creature() {
        send_system_message(stream, "Selected target is not a creature.", header_crypto).await?;
        return Ok(());
    }
    let Some(target_creature) = deps
        .maps
        .db_creature_snapshot(character.position.map_id, target)
        .await
    else {
        send_system_message(
            stream,
            "Selected creature is not spawned on this map.",
            header_crypto,
        )
        .await?;
        return Ok(());
    };
    if !target_creature.is_alive() {
        send_system_message(stream, "Selected creature is already dead.", header_crypto).await?;
        return Ok(());
    }
    let corpse_loot = prepare_db_creature_corpse_loot(
        deps.object_mgr,
        deps.world_db_pool,
        deps.parties,
        session,
        character.guid,
        target_creature.spawn.entry,
    )
    .await?;
    deps.maps
        .force_db_creature_loot_owner(character.position.map_id, target, corpse_loot.owner)
        .await;
    let killer = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let Some(event) = deps
        .maps
        .apply_db_creature_damage(
            character.position.map_id,
            DbCreatureDamageRequest {
                creature_guid: target,
                killer,
                damage: target_creature.health,
                melee_outcome: None,
                spell_damage_outcome: None,
                spell_id: None,
                spell_school: 0,
                suppress_attacker_state: true,
                now: Instant::now(),
                now_epoch_secs: current_unix_epoch_secs(),
                exclude_character_guid: Some(character.guid),
                corpse_loot: Some(corpse_loot),
            },
        )
        .await?
    else {
        send_system_message(
            stream,
            "Selected creature could not be killed.",
            header_crypto,
        )
        .await?;
        return Ok(());
    };
    mirror_session_db_creature(session, target.raw(), event.creature.clone());
    if let Some(body) = event.attacker_state_body.as_ref() {
        send_packet(
            stream,
            SMSG_ATTACKERSTATEUPDATE,
            body,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &event.update_body,
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
    deps.sessions.dispatch(event.observer_packets).await;
    if let Some(death_finalization) = event.death_finalization {
        deps.sessions
            .dispatch(death_finalization.observer_packets)
            .await;
        if let Some(motion_stop_packet) = death_finalization.motion_stop_packet {
            send_packet(
                stream,
                motion_stop_packet.opcode,
                &motion_stop_packet.body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
        send_packet(
            stream,
            death_finalization.combat_flag_packet.opcode,
            &death_finalization.combat_flag_packet.body,
            Some(&mut *header_crypto),
        )
        .await?;
        send_packet(
            stream,
            death_finalization.attack_stop_packet.opcode,
            &death_finalization.attack_stop_packet.body,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    send_system_message(stream, "Selected creature killed.", header_crypto).await
}

pub(in crate::world) async fn delete_gm_creature_runtime(
    stream: &mut WorldPacketSink,
    deps: ChatDeps<'_>,
    session: &mut WorldSessionState,
    db_guid: Option<u32>,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.clone() else {
        return Ok(());
    };
    let target = if db_guid.is_some() {
        None
    } else {
        let Some(target) = session.character.selected_target else {
            send_system_message(stream, "Select a creature first.", header_crypto).await?;
            return Ok(());
        };
        if !target.is_creature() {
            send_system_message(stream, "Selected target is not a creature.", header_crypto)
                .await?;
            return Ok(());
        }
        Some(target)
    };
    let Some(deleted) = deps
        .maps
        .delete_db_creature_runtime(
            character.position.map_id,
            target,
            db_guid,
            Some(character.guid),
        )
        .await?
    else {
        send_system_message(stream, "Creature was not found on this map.", header_crypto).await?;
        return Ok(());
    };
    if session.character.selected_target == Some(deleted.creature.guid()) {
        session.character.selected_target = None;
    }
    send_packet(
        stream,
        deleted.direct_packet.opcode,
        &deleted.direct_packet.body,
        Some(&mut *header_crypto),
    )
    .await?;
    deps.sessions.dispatch(deleted.observer_packets).await;
    send_system_message(
        stream,
        &format!(
            "Deleted {} ({}) from the live map runtime.",
            deleted.creature.spawn.template.name, deleted.creature.spawn.guid
        ),
        header_crypto,
    )
    .await
}

pub(in crate::world) async fn send_system_message(
    stream: &mut WorldPacketSink,
    message: &str,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let body = build_system_message_chat_body(message);
    send_packet(stream, SMSG_MESSAGECHAT, &body, Some(header_crypto)).await
}
