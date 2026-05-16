use super::*;

#[derive(Debug, Clone, PartialEq)]
pub(in crate::world) enum GmDotCommand {
    Gm(Option<bool>),
    LevelUp(i32),
    LevelSet(u8),
    NpcAdd(u32),
    NpcDelete(Option<u32>),
    Die,
    Go(GmGoDestination),
    ModifySpeed(f32),
}

#[derive(Debug, Clone, PartialEq)]
pub(in crate::world) enum GmGoDestination {
    Coordinates {
        x: f32,
        y: f32,
        z: Option<f32>,
        map_id: Option<u32>,
    },
    Waypoint(String),
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct GmWaypoint {
    pub(in crate::world) aliases: &'static [&'static str],
    pub(in crate::world) position: WorldPosition,
}

pub(in crate::world) const GM_WAYPOINTS: &[GmWaypoint] = &[
    GmWaypoint {
        aliases: &["northshire", "northshireabbey", "abbey"],
        position: WorldPosition {
            map_id: 0,
            x: -8949.95,
            y: -132.493,
            z: 83.5312,
            orientation: 0.0,
        },
    },
    GmWaypoint {
        aliases: &["goldshire", "elwynn", "elwynnforest"],
        position: WorldPosition {
            map_id: 0,
            x: -9464.0,
            y: 62.0,
            z: 56.0,
            orientation: 0.0,
        },
    },
    GmWaypoint {
        aliases: &["stormwind", "sw"],
        position: WorldPosition {
            map_id: 0,
            x: -8913.23,
            y: 554.633,
            z: 93.7944,
            orientation: 0.0,
        },
    },
    GmWaypoint {
        aliases: &["ironforge", "if"],
        position: WorldPosition {
            map_id: 0,
            x: -4981.25,
            y: -881.542,
            z: 501.66,
            orientation: 0.0,
        },
    },
    GmWaypoint {
        aliases: &["westfall", "sentinelhill"],
        position: WorldPosition {
            map_id: 0,
            x: -10645.9,
            y: 1179.06,
            z: 34.46,
            orientation: 0.0,
        },
    },
    GmWaypoint {
        aliases: &["darkshire", "duskwood"],
        position: WorldPosition {
            map_id: 0,
            x: -10559.7,
            y: -1189.02,
            z: 28.07,
            orientation: 0.0,
        },
    },
    GmWaypoint {
        aliases: &["bootybay", "bb", "stranglethorn"],
        position: WorldPosition {
            map_id: 0,
            x: -14406.6,
            y: 419.353,
            z: 22.39,
            orientation: 0.0,
        },
    },
];

pub(in crate::world) const PLAYER_BASE_RUN_SPEED_YARDS_PER_SEC: f32 = 7.0;

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
            if session.account.gm_mode {
                send_system_message(stream, "Unknown command.", header_crypto).await?;
            } else {
                send_system_message(
                    stream,
                    "You must turn GM mode on first with .gm on.",
                    header_crypto,
                )
                .await?;
            }
            return Ok(());
        }
    };

    match command {
        GmDotCommand::Gm(value) => {
            if !require_gm_security(stream, session, 1, header_crypto).await? {
                return Ok(());
            }
            if let Some(value) = value {
                set_gm_mode(stream, deps, session, value, header_crypto).await?;
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
            if !require_gm_mode(stream, session, header_crypto).await? {
                return Ok(());
            }
            spawn_gm_creature_from_template(stream, deps, session, entry, header_crypto).await?;
        }
        GmDotCommand::LevelUp(delta) => {
            if !require_gm_security(stream, session, 3, header_crypto).await? {
                return Ok(());
            }
            if !require_gm_mode(stream, session, header_crypto).await? {
                return Ok(());
            }
            change_gm_character_level_relative(stream, deps, session, delta, header_crypto).await?;
        }
        GmDotCommand::LevelSet(level) => {
            if !require_gm_security(stream, session, 3, header_crypto).await? {
                return Ok(());
            }
            if !require_gm_mode(stream, session, header_crypto).await? {
                return Ok(());
            }
            change_gm_character_level_absolute(stream, deps, session, level, header_crypto).await?;
        }
        GmDotCommand::NpcDelete(db_guid) => {
            if !require_gm_security(stream, session, 2, header_crypto).await? {
                return Ok(());
            }
            if !require_gm_mode(stream, session, header_crypto).await? {
                return Ok(());
            }
            delete_gm_creature_runtime(stream, deps, session, db_guid, header_crypto).await?;
        }
        GmDotCommand::Die => {
            if !require_gm_security(stream, session, 3, header_crypto).await? {
                return Ok(());
            }
            if !require_gm_mode(stream, session, header_crypto).await? {
                return Ok(());
            }
            kill_selected_db_creature(stream, deps, session, header_crypto).await?;
        }
        GmDotCommand::Go(destination) => {
            if !require_gm_security(stream, session, 1, header_crypto).await? {
                return Ok(());
            }
            if !require_gm_mode(stream, session, header_crypto).await? {
                return Ok(());
            }
            teleport_gm(stream, deps, session, destination, header_crypto).await?;
        }
        GmDotCommand::ModifySpeed(speed_rate) => {
            if !require_gm_security(stream, session, 1, header_crypto).await? {
                return Ok(());
            }
            if !require_gm_mode(stream, session, header_crypto).await? {
                return Ok(());
            }
            modify_gm_run_speed(stream, session, speed_rate, header_crypto).await?;
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
    if normalized.starts_with("go ") {
        let args = without_dot
            .find(char::is_whitespace)
            .map(|index| &without_dot[index..])
            .unwrap_or_default();
        return Some(parse_go_destination(args));
    }
    if normalized == "go" {
        return Some(Err(
            "Syntax: .go #x #y [#z [#mapid]] or .go #waypoint".to_string()
        ));
    }
    if let Some(args) = normalized.strip_prefix("modify speed ") {
        return Some(match first_f32(args) {
            Some(speed) => Ok(GmDotCommand::ModifySpeed(speed)),
            None => Err("Syntax: .modify speed #rate".to_string()),
        });
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

pub(in crate::world) fn parse_go_destination(input: &str) -> Result<GmDotCommand, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Syntax: .go #x #y [#z [#mapid]] or .go #waypoint".to_string());
    }
    let normalized_args = trimmed.replace(',', " ");
    let first = normalized_args
        .split_whitespace()
        .next()
        .unwrap_or_default();
    if first.parse::<f32>().is_ok() {
        let numbers = coordinate_numbers(&normalized_args);
        return match numbers.as_slice() {
            [x, y] => Ok(GmDotCommand::Go(GmGoDestination::Coordinates {
                x: *x,
                y: *y,
                z: None,
                map_id: None,
            })),
            [x, y, z] => Ok(GmDotCommand::Go(GmGoDestination::Coordinates {
                x: *x,
                y: *y,
                z: Some(*z),
                map_id: None,
            })),
            [x, y, z, map_id, ..] if *map_id >= 0.0 => {
                Ok(GmDotCommand::Go(GmGoDestination::Coordinates {
                    x: *x,
                    y: *y,
                    z: Some(*z),
                    map_id: Some(*map_id as u32),
                }))
            }
            _ => Err("Syntax: .go #x #y [#z [#mapid]]".to_string()),
        };
    }
    Ok(GmDotCommand::Go(GmGoDestination::Waypoint(
        trimmed.to_string(),
    )))
}

pub(in crate::world) fn coordinate_numbers(input: &str) -> Vec<f32> {
    input
        .split_whitespace()
        .filter_map(|token| token.parse::<f32>().ok())
        .collect()
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

pub(in crate::world) fn first_f32(input: &str) -> Option<f32> {
    input
        .split_whitespace()
        .next()
        .and_then(|token| token.parse::<f32>().ok())
}

pub(in crate::world) fn find_gm_waypoint(name: &str) -> Option<GmWaypoint> {
    let key = normalize_gm_waypoint_name(name);
    GM_WAYPOINTS
        .iter()
        .copied()
        .find(|waypoint| waypoint.aliases.iter().any(|alias| *alias == key))
}

pub(in crate::world) fn normalize_gm_waypoint_name(name: &str) -> String {
    name.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .collect()
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

pub(in crate::world) async fn require_gm_mode(
    stream: &mut WorldPacketSink,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    if session.account.gm_mode {
        return Ok(true);
    }
    send_system_message(
        stream,
        "You must turn GM mode on first with .gm on.",
        header_crypto,
    )
    .await?;
    Ok(false)
}

pub(in crate::world) async fn set_gm_mode(
    stream: &mut WorldPacketSink,
    deps: ChatDeps<'_>,
    session: &mut WorldSessionState,
    enabled: bool,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    session.account.gm_mode = enabled;
    if enabled {
        session.character.player_flags |= PLAYER_FLAGS_GM;
    } else {
        session.character.player_flags &= !PLAYER_FLAGS_GM;
    }
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_gm_mode_update_body(
            player_guid,
            character.race,
            session.character.player_flags,
        )?,
        Some(&mut *header_crypto),
    )
    .await?;
    let observer_packets = deps
        .maps
        .set_player_gm_flags(
            character.position.map_id,
            character.guid,
            session.character.player_flags,
        )
        .await?;
    deps.sessions.dispatch(observer_packets).await;
    Ok(())
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
    let DbCreatureDamageEvent {
        creature,
        attacker_state_body,
        update_body,
        direct_packets,
        death_finalization,
        observer_packets,
        ..
    } = event;
    mirror_session_db_creature(session, target.raw(), creature);
    if let Some(body) = attacker_state_body.as_ref() {
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
        &update_body,
        Some(&mut *header_crypto),
    )
    .await?;
    for packet in direct_packets {
        send_packet(
            stream,
            packet.opcode,
            &packet.body,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    deps.sessions.dispatch(observer_packets).await;
    finalize_db_creature_death(
        stream,
        CombatRewardDeps {
            character_db_pool: deps.character_db_pool,
            world_db_pool: deps.world_db_pool,
            shared_world: SharedWorldDeps {
                object_mgr: deps.object_mgr,
                maps: deps.maps,
                sessions: deps.sessions,
            },
            parties: deps.parties,
        },
        session,
        death_finalization,
        header_crypto,
    )
    .await?;
    send_system_message(stream, "Selected creature killed.", header_crypto).await
}

pub(in crate::world) async fn teleport_gm(
    stream: &mut WorldPacketSink,
    deps: ChatDeps<'_>,
    session: &mut WorldSessionState,
    destination: GmGoDestination,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(current_character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let old_map_id = current_character.position.map_id;
    let (mut target, use_ground_z) = match destination {
        GmGoDestination::Coordinates { x, y, z, map_id } => (
            WorldPosition {
                map_id: map_id.unwrap_or(old_map_id),
                x,
                y,
                z: z.unwrap_or(current_character.position.z),
                orientation: current_character.position.orientation,
            },
            z.is_none(),
        ),
        GmGoDestination::Waypoint(name) => {
            let Some(waypoint) = find_gm_waypoint(&name) else {
                send_system_message(
                    stream,
                    &format!("Unknown waypoint '{name}'."),
                    header_crypto,
                )
                .await?;
                return Ok(());
            };
            (waypoint.position, false)
        }
    };
    if use_ground_z {
        target = deps.maps.geometry.ground_position(target).unwrap_or(target);
    }
    if target.map_id != old_map_id {
        send_system_message(
            stream,
            "Cross-map .go is not wired yet; use a waypoint on the current map.",
            header_crypto,
        )
        .await?;
        return Ok(());
    }

    let character_guid = current_character.guid;
    let client_time = current_character.client_time;
    if let Some(character) = session.character.active_character.as_mut() {
        character.position = target;
        character.movement_flags = 0;
        character.fall_time = 0;
        character.jump = JumpInfo::default();
    }
    let account_id = session.account.account_id;
    let movement = MovementInfo {
        flags: 0,
        client_time,
        position: target,
        fall_time: 0,
        jump: JumpInfo::default(),
    };
    let observer_packets = deps
        .maps
        .update_player_position(
            old_map_id,
            character_guid,
            MSG_MOVE_HEARTBEAT as u16,
            &movement,
            movement.client_time,
        )
        .await?;
    deps.sessions.dispatch(observer_packets).await;
    deps.maps
        .reset_player_visibility_scan_positions(old_map_id, character_guid)
        .await;
    deps.maps
        .sync_player_gameplay_state(old_map_id, character_guid, session)
        .await;
    wow_db::update_character_position(deps.character_db_pool, account_id, character_guid, target)
        .await?;
    send_packet(
        stream,
        MSG_MOVE_TELEPORT_ACK,
        &build_near_teleport_ack_body(session.character.active_character.as_ref().unwrap(), 0)?,
        Some(&mut *header_crypto),
    )
    .await?;
    stream_newly_visible_db_creatures(
        stream,
        deps.character_db_pool,
        deps.world_db_pool,
        deps.maps,
        session,
        header_crypto,
    )
    .await?;
    send_system_message(
        stream,
        &format!(
            "Teleported to {:.2} {:.2} {:.2}.",
            target.x, target.y, target.z
        ),
        header_crypto,
    )
    .await
}

pub(in crate::world) async fn modify_gm_run_speed(
    stream: &mut WorldPacketSink,
    session: &WorldSessionState,
    speed_rate: f32,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    if !(0.1..=50.0).contains(&speed_rate) {
        send_system_message(
            stream,
            "Speed rate must be between 0.1 and 50.",
            header_crypto,
        )
        .await?;
        return Ok(());
    }
    let player = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let speed = PLAYER_BASE_RUN_SPEED_YARDS_PER_SEC * speed_rate;
    send_packet(
        stream,
        SMSG_FORCE_RUN_SPEED_CHANGE,
        &build_force_run_speed_change_body(player, 0, speed)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_system_message(
        stream,
        &format!("Run speed set to {speed_rate:.2}x."),
        header_crypto,
    )
    .await
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
