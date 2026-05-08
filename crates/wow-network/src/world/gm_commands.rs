#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GmDotCommand {
    Gm(Option<bool>),
    NpcAdd(u32),
    NpcDelete(Option<u32>),
    Die,
}

async fn handle_gm_dot_command(
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
                session.gm_mode = value;
            }
            let message = if session.gm_mode {
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

fn parse_gm_dot_command(message: &str) -> Option<Result<GmDotCommand, String>> {
    let trimmed = message.trim();
    let without_dot = trimmed.strip_prefix('.')?.trim();
    if without_dot.eq_ignore_ascii_case("die") || without_dot.starts_with("die ") {
        return Some(Ok(GmDotCommand::Die));
    }
    if without_dot.eq_ignore_ascii_case("gm") {
        return Some(Ok(GmDotCommand::Gm(None)));
    }
    if let Some(args) = without_dot.strip_prefix("gm ") {
        let arg = args.trim();
        return Some(match arg.to_ascii_lowercase().as_str() {
            "on" | "1" => Ok(GmDotCommand::Gm(Some(true))),
            "off" | "0" => Ok(GmDotCommand::Gm(Some(false))),
            _ => Err("Syntax: .gm on/off".to_string()),
        });
    }
    if let Some(args) = without_dot.strip_prefix("npc add") {
        return Some(match first_u32(args) {
            Some(entry) => Ok(GmDotCommand::NpcAdd(entry)),
            None => Err("Syntax: .npc add #creatureid".to_string()),
        });
    }
    if let Some(args) = without_dot.strip_prefix("npc delete") {
        return Some(Ok(GmDotCommand::NpcDelete(first_u32(args))));
    }
    None
}

fn first_u32(input: &str) -> Option<u32> {
    let mut current = String::new();
    for ch in input.chars() {
        if ch.is_ascii_digit() {
            current.push(ch);
        } else if !current.is_empty() {
            return current.parse().ok();
        }
    }
    (!current.is_empty()).then(|| current.parse().ok()).flatten()
}

async fn require_gm_security(
    stream: &mut WorldPacketSink,
    session: &WorldSessionState,
    required_security: u8,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    if session.account_security >= required_security {
        return Ok(true);
    }
    send_system_message(stream, "You do not have permission to use that command.", header_crypto)
        .await?;
    Ok(false)
}

async fn spawn_gm_creature_from_template(
    stream: &mut WorldPacketSink,
    deps: ChatDeps<'_>,
    session: &WorldSessionState,
    entry: u32,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.active_character.clone() else {
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
    let create_body = build_update_object_body(&[build_db_creature_runtime_create_block(&creature)?]);
    send_packet(stream, SMSG_UPDATE_OBJECT, &create_body, Some(&mut *header_crypto)).await?;
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

async fn kill_selected_db_creature(
    stream: &mut WorldPacketSink,
    deps: ChatDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.active_character.clone() else {
        return Ok(());
    };
    let Some(target) = session.selected_target else {
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
        send_system_message(stream, "Selected creature is not spawned on this map.", header_crypto)
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
                spell_id: None,
                suppress_attacker_state: true,
                now: Instant::now(),
                now_epoch_secs: current_unix_epoch_secs(),
                exclude_character_guid: Some(character.guid),
                corpse_loot: Some(corpse_loot),
            },
        )
        .await?
    else {
        send_system_message(stream, "Selected creature could not be killed.", header_crypto).await?;
        return Ok(());
    };
    mirror_session_db_creature(session, target.raw(), event.creature.clone());
    if let Some(body) = event.attacker_state_body.as_ref() {
        send_packet(stream, SMSG_ATTACKERSTATEUPDATE, body, Some(&mut *header_crypto)).await?;
    }
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &event.update_body,
        Some(&mut *header_crypto),
    )
    .await?;
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

async fn delete_gm_creature_runtime(
    stream: &mut WorldPacketSink,
    deps: ChatDeps<'_>,
    session: &mut WorldSessionState,
    db_guid: Option<u32>,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.active_character.clone() else {
        return Ok(());
    };
    let target = if db_guid.is_some() {
        None
    } else {
        let Some(target) = session.selected_target else {
            send_system_message(stream, "Select a creature first.", header_crypto).await?;
            return Ok(());
        };
        if !target.is_creature() {
            send_system_message(stream, "Selected target is not a creature.", header_crypto).await?;
            return Ok(());
        }
        Some(target)
    };
    let Some(deleted) = deps
        .maps
        .delete_db_creature_runtime(character.position.map_id, target, db_guid, Some(character.guid))
        .await?
    else {
        send_system_message(stream, "Creature was not found on this map.", header_crypto).await?;
        return Ok(());
    };
    if session.selected_target == Some(deleted.creature.guid()) {
        session.selected_target = None;
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

async fn send_system_message(
    stream: &mut WorldPacketSink,
    message: &str,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let body = build_system_message_chat_body(message);
    send_packet(stream, SMSG_MESSAGECHAT, &body, Some(header_crypto)).await
}
