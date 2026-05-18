use super::*;

// CMaNGOS reference: src/game/WorldSocket.cpp and WorldSession opcode dispatch.

pub(in crate::world) const WORLD_LOGIN_TIMEOUT: Duration = Duration::from_secs(30);
pub(in crate::world) const WORLD_SESSION_IDLE_TIMEOUT: Duration = Duration::from_secs(15 * 60);
pub(in crate::world) const WORLD_SESSION_WRITE_TIMEOUT: Duration = Duration::from_secs(10);

pub(in crate::world) async fn handle_client(
    mut stream: TcpStream,
    login_db_pool: MySqlPool,
    character_db_pool: MySqlPool,
    world_db_pool: MySqlPool,
    runtime_state: WorldRuntimeState,
) -> anyhow::Result<()> {
    timeout(
        WORLD_SESSION_WRITE_TIMEOUT,
        send_packet_direct(
            &mut stream,
            SMSG_AUTH_CHALLENGE,
            &SERVER_SEED.to_le_bytes(),
            None,
        ),
    )
    .await??;

    let (opcode, payload) =
        match timeout(WORLD_LOGIN_TIMEOUT, read_client_packet(&mut stream, None)).await {
            Ok(result) => result?,
            Err(_) => {
                crate::observability::record_world_session_disconnect(
                    WorldSessionDisconnectReason::LoginTimeout.metric_label(),
                );
                anyhow::bail!("world auth session timed out waiting for CMSG_AUTH_SESSION");
            }
        };
    if opcode != CMSG_AUTH_SESSION {
        anyhow::bail!("expected CMSG_AUTH_SESSION, got 0x{opcode:04X}");
    }

    let auth = packets::parse_world_auth_session_packet(&payload)?;
    info!(
        account = %auth.account,
        build = auth.client_build,
        client_seed = format_args!("0x{:08X}", auth.client_seed),
        addon_bytes = auth.addon_data.len(),
        "Received CMSG_AUTH_SESSION"
    );

    if !matches!(auth.client_build, 5875 | 6005 | 6141) {
        send_auth_response(&mut stream, AUTH_VERSION_MISMATCH).await?;
        anyhow::bail!("unsupported world client build {}", auth.client_build);
    }

    let account = wow_db::account::get_account_by_username(&login_db_pool, &auth.account)
        .await?
        .ok_or_else(|| anyhow::anyhow!("unknown account {}", auth.account))?;

    if account.sessionkey.trim().is_empty() {
        send_auth_response(&mut stream, AUTH_UNKNOWN_ACCOUNT).await?;
        anyhow::bail!("account {} has no auth session key", auth.account);
    }

    let session_key = hex_to_array40(&account.sessionkey)?;
    if !verify_world_digest(&auth, &session_key) {
        send_auth_response(&mut stream, AUTH_FAILED).await?;
        anyhow::bail!("world auth digest mismatch for account {}", auth.account);
    }

    info!(
        account = %auth.account,
        account_id = account.id,
        "World auth session verified"
    );

    let session_id = SessionId::next();
    let (read_stream, write_stream) = stream.into_split();
    let mut read_stream = read_stream;
    let (outbound_tx, outbound_rx) = mpsc::channel(WORLD_OUTBOUND_QUEUE_CAPACITY);
    let (disconnect_tx, mut disconnect_rx) = mpsc::channel(1);
    let mut stream = WorldPacketSink::new(outbound_tx.clone());
    runtime_state
        .sessions
        .register(
            session_id,
            SessionHandle {
                account_id: account.id,
                character_guid: None,
                character_name: None,
                outbound: WorldPacketSender::Bounded(outbound_tx.clone()),
                disconnect: Some(disconnect_tx.clone()),
            },
        )
        .await;
    let writer_task = tokio::spawn(world_session_writer(
        session_id,
        write_stream,
        outbound_rx,
        HeaderCrypto::new(&session_key),
        disconnect_tx.clone(),
    ));

    let mut read_header_crypto = HeaderCrypto::new(&session_key);
    let mut header_crypto = HeaderCrypto::new(&session_key);
    send_auth_ok(&mut stream, Some(&mut header_crypto)).await?;
    let mut session = WorldSessionState {
        account: AccountSessionState {
            account_id: account.id,
            account_security: account.gmlevel,
            gm_mode: false,
            ..AccountSessionState::default()
        },
        movement: MovementSessionState {
            db_creature_navigation: DbCreatureNavigationGuardrail {
                world_data_files: runtime_state.world_data_files.clone(),
                ..DbCreatureNavigationGuardrail::default()
            },
            ..MovementSessionState::default()
        },
        ..WorldSessionState::default()
    };
    load_global_account_data_into_session(&character_db_pool, account.id, &mut session).await?;
    let world_tick_interval = runtime_state.world_tick_interval;
    let mut next_world_tick_at = Instant::now() + world_tick_interval;
    let mut last_client_packet_at = Instant::now();

    let session_result: anyhow::Result<()> = async {
        loop {
            let now = Instant::now();
            let idle_elapsed = now.saturating_duration_since(last_client_packet_at);
            if idle_elapsed >= WORLD_SESSION_IDLE_TIMEOUT {
                crate::observability::record_world_session_disconnect(
                    WorldSessionDisconnectReason::IdleTimeout.metric_label(),
                );
                anyhow::bail!("world session idle timeout after {:?}", idle_elapsed);
            }
            if complete_pending_logout_if_due(
                &mut stream,
                LogoutDeps {
                    character_db_pool: &character_db_pool,
                    online_characters: &runtime_state.online_characters,
                    maps: &runtime_state.maps,
                    sessions: &runtime_state.sessions,
                    account_id: account.id,
                    session_id,
                },
                &mut header_crypto,
                &mut session,
                now,
            )
            .await?
            {
                continue;
            }
            let loop_timeout = session_loop_timeout_duration(
                runtime_state.maps.as_ref(),
                &session,
                next_world_tick_at,
                now,
            )
            .await
            .min(WORLD_SESSION_IDLE_TIMEOUT - idle_elapsed);
            tokio::select! {
                disconnect_reason = disconnect_rx.recv() => {
                    let reason = disconnect_reason.unwrap_or(WorldSessionDisconnectReason::WriteError);
                    crate::observability::record_world_session_disconnect(reason.metric_label());
                    anyhow::bail!("world session disconnect requested: {:?}", reason);
                }
                read_result = timeout(
                    loop_timeout,
                    read_client_packet(&mut read_stream, Some(&mut read_header_crypto)),
                ) => match read_result {
                Ok(Ok((opcode, body))) => {
                    let packet_branch_started_at = Instant::now();
                    let parsed_packet = packets::parse_world_client_packet(opcode, &body)?;
                    let opcode = parsed_packet.opcode();
                    let packet_received_at = Instant::now();
                    last_client_packet_at = packet_received_at;
                    let refresh_started_at = Instant::now();
                    let map_player_died =
                        refresh_active_player_session_cache(&runtime_state.maps, &mut session)
                            .await;
                    crate::observability::record_world_session_loop_phase_duration(
                        crate::observability::WorldSessionLoopPhase::RefreshActivePlayerCache,
                        refresh_started_at.elapsed(),
                    );
                    let finalize_death_started_at = Instant::now();
                    finalize_map_owned_player_death_if_needed(
                        &mut stream,
                        &character_db_pool,
                        account.id,
                        SharedWorldDeps {
                            object_mgr: runtime_state.object_mgr.as_ref(),
                            maps: &runtime_state.maps,
                            sessions: &runtime_state.sessions,
                        },
                        &mut session,
                        &mut header_crypto,
                        map_player_died,
                    )
                    .await?;
                    crate::observability::record_world_session_loop_phase_duration(
                        crate::observability::WorldSessionLoopPhase::FinalizePlayerDeath,
                        finalize_death_started_at.elapsed(),
                    );
                    if is_movement_opcode(opcode) {
                        debug!(
                            opcode = format_args!("0x{opcode:04X}"),
                            bytes = body.len(),
                            "Received movement packet after auth"
                        );
                    } else {
                        debug!(
                            opcode = format_args!("0x{opcode:04X}"),
                            bytes = body.len(),
                            "Received world packet after auth"
                        );
                    }

                    if pending_player_spell_cast_is_due(
                        runtime_state.maps.as_ref(),
                        &session,
                        Instant::now(),
                    )
                    .await
                    {
                        let pending_spell_started_at = Instant::now();
                        complete_pending_player_spell_cast(
                            &mut stream,
                            SpellCastDeps {
                                character_db_pool: &character_db_pool,
                                world_db_pool: &world_db_pool,
                                account_id: account.id,
                                shared_world: SharedWorldDeps {
                                    object_mgr: runtime_state.object_mgr.as_ref(),
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
                                },
                                parties: runtime_state.parties.as_ref(),
                            },
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                        crate::observability::record_world_session_loop_phase_duration(
                            crate::observability::WorldSessionLoopPhase::PendingSpellCompletion,
                            pending_spell_started_at.elapsed(),
                        );
                    }

                    let mut dispatch_context = WorldPacketDispatchContext {
                        stream: &mut stream,
                        login_db_pool: &login_db_pool,
                        character_db_pool: &character_db_pool,
                        world_db_pool: &world_db_pool,
                        runtime_state: &runtime_state,
                        session_id,
                        account_id: account.id,
                        account_name: &auth.account,
                        session: &mut session,
                        header_crypto: &mut header_crypto,
                    };
                    crate::observability::record_world_packet_dispatch_delay(
                        opcode,
                        packet_received_at.elapsed(),
                    );
                    let packet_handler_started_at = Instant::now();
                    let dispatch_result =
                        dispatch_world_packet(&mut dispatch_context, &parsed_packet, &body).await;
                    crate::observability::record_world_session_loop_phase_duration(
                        crate::observability::WorldSessionLoopPhase::PacketDispatch,
                        packet_handler_started_at.elapsed(),
                    );
                    crate::observability::record_world_packet_handler_duration(
                        opcode,
                        packet_handler_started_at.elapsed(),
                    );
                    crate::observability::record_world_packet_service_time(
                        opcode,
                        packet_received_at.elapsed(),
                    );
                    dispatch_result?;

                    if pending_player_spell_cast_is_due(
                        runtime_state.maps.as_ref(),
                        &session,
                        Instant::now(),
                    )
                    .await
                    {
                        let pending_spell_started_at = Instant::now();
                        complete_pending_player_spell_cast(
                            &mut stream,
                            SpellCastDeps {
                                character_db_pool: &character_db_pool,
                                world_db_pool: &world_db_pool,
                                account_id: account.id,
                                shared_world: SharedWorldDeps {
                                    object_mgr: runtime_state.object_mgr.as_ref(),
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
                                },
                                parties: runtime_state.parties.as_ref(),
                            },
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                        crate::observability::record_world_session_loop_phase_duration(
                            crate::observability::WorldSessionLoopPhase::PendingSpellCompletion,
                            pending_spell_started_at.elapsed(),
                        );
                    }
                    if complete_pending_logout_if_due(
                        &mut stream,
                        LogoutDeps {
                            character_db_pool: &character_db_pool,
                            online_characters: &runtime_state.online_characters,
                            maps: &runtime_state.maps,
                            sessions: &runtime_state.sessions,
                            account_id: account.id,
                            session_id,
                        },
                        &mut header_crypto,
                        &mut session,
                        Instant::now(),
                    )
                    .await?
                    {
                        crate::observability::record_world_session_loop_phase_duration(
                            crate::observability::WorldSessionLoopPhase::PacketBranchTotal,
                            packet_branch_started_at.elapsed(),
                        );
                        continue;
                    }
                    let sync_started_at = Instant::now();
                    sync_active_player_gameplay_state(&runtime_state.maps, &session).await;
                    crate::observability::record_world_session_loop_phase_duration(
                        crate::observability::WorldSessionLoopPhase::SyncGameplayState,
                        sync_started_at.elapsed(),
                    );
                    if Instant::now() >= next_world_tick_at {
                        let combat_tick_started_at = Instant::now();
                        handle_combat_tick(
                            &mut stream,
                            CombatTickDeps {
                                character_db_pool: &character_db_pool,
                                world_db_pool: &world_db_pool,
                                shared_world: SharedWorldDeps {
                                    object_mgr: runtime_state.object_mgr.as_ref(),
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
                                },
                                parties: &runtime_state.parties,
                                session_id,
                            },
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                        crate::observability::record_world_session_loop_phase_duration(
                            crate::observability::WorldSessionLoopPhase::CombatTick,
                            combat_tick_started_at.elapsed(),
                        );
                        let loot_timeouts_started_at = Instant::now();
                        handle_loot_roll_timeouts(
                            &mut stream,
                            LootMutationDeps {
                                character_db_pool: &character_db_pool,
                                world_db_pool: &world_db_pool,
                                shared_world: SharedWorldDeps {
                                    object_mgr: runtime_state.object_mgr.as_ref(),
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
                                },
                                parties: &runtime_state.parties,
                            },
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                        crate::observability::record_world_session_loop_phase_duration(
                            crate::observability::WorldSessionLoopPhase::LootRollTimeouts,
                            loot_timeouts_started_at.elapsed(),
                        );
                        let sync_started_at = Instant::now();
                        sync_active_player_gameplay_state(&runtime_state.maps, &session).await;
                        crate::observability::record_world_session_loop_phase_duration(
                            crate::observability::WorldSessionLoopPhase::SyncGameplayState,
                            sync_started_at.elapsed(),
                        );
                        advance_world_tick_deadline(
                            &mut next_world_tick_at,
                            Instant::now(),
                            world_tick_interval,
                        );
                    }
                    crate::observability::record_world_session_loop_phase_duration(
                        crate::observability::WorldSessionLoopPhase::PacketBranchTotal,
                        packet_branch_started_at.elapsed(),
                    );
                }
                Ok(Err(e)) => {
                    refresh_active_player_session_cache(&runtime_state.maps, &mut session).await;
                    persist_session_character_state(
                        &character_db_pool,
                        account.id,
                        &runtime_state.maps,
                        &session,
                    )
                    .await?;
                    unregister_active_character(
                        &runtime_state.online_characters,
                        &runtime_state.maps,
                        &runtime_state.sessions,
                        session_id,
                        &mut session,
                    )
                    .await;
                    info!("World client disconnected or read failed: {}", e);
                    break Ok(());
                }
                Err(_) => {
                    let timeout_branch_started_at = Instant::now();
                    let refresh_started_at = Instant::now();
                    let map_player_died =
                        refresh_active_player_session_cache(&runtime_state.maps, &mut session)
                            .await;
                    crate::observability::record_world_session_loop_phase_duration(
                        crate::observability::WorldSessionLoopPhase::RefreshActivePlayerCache,
                        refresh_started_at.elapsed(),
                    );
                    let finalize_death_started_at = Instant::now();
                    finalize_map_owned_player_death_if_needed(
                        &mut stream,
                        &character_db_pool,
                        account.id,
                        SharedWorldDeps {
                            object_mgr: runtime_state.object_mgr.as_ref(),
                            maps: &runtime_state.maps,
                            sessions: &runtime_state.sessions,
                        },
                        &mut session,
                        &mut header_crypto,
                        map_player_died,
                    )
                    .await?;
                    crate::observability::record_world_session_loop_phase_duration(
                        crate::observability::WorldSessionLoopPhase::FinalizePlayerDeath,
                        finalize_death_started_at.elapsed(),
                    );
                    if pending_player_spell_cast_is_due(
                        runtime_state.maps.as_ref(),
                        &session,
                        Instant::now(),
                    )
                    .await
                    {
                        let pending_spell_started_at = Instant::now();
                        complete_pending_player_spell_cast(
                            &mut stream,
                            SpellCastDeps {
                                character_db_pool: &character_db_pool,
                                world_db_pool: &world_db_pool,
                                account_id: account.id,
                                shared_world: SharedWorldDeps {
                                    object_mgr: runtime_state.object_mgr.as_ref(),
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
                                },
                                parties: runtime_state.parties.as_ref(),
                            },
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                        crate::observability::record_world_session_loop_phase_duration(
                            crate::observability::WorldSessionLoopPhase::PendingSpellCompletion,
                            pending_spell_started_at.elapsed(),
                        );
                    }
                    if complete_pending_logout_if_due(
                        &mut stream,
                        LogoutDeps {
                            character_db_pool: &character_db_pool,
                            online_characters: &runtime_state.online_characters,
                            maps: &runtime_state.maps,
                            sessions: &runtime_state.sessions,
                            account_id: account.id,
                            session_id,
                        },
                        &mut header_crypto,
                        &mut session,
                        Instant::now(),
                    )
                    .await?
                    {
                        crate::observability::record_world_session_loop_phase_duration(
                            crate::observability::WorldSessionLoopPhase::TimeoutBranchTotal,
                            timeout_branch_started_at.elapsed(),
                        );
                        continue;
                    }
                    let combat_tick_started_at = Instant::now();
                    handle_combat_tick(
                        &mut stream,
                        CombatTickDeps {
                            character_db_pool: &character_db_pool,
                            world_db_pool: &world_db_pool,
                            shared_world: SharedWorldDeps {
                                object_mgr: runtime_state.object_mgr.as_ref(),
                                maps: &runtime_state.maps,
                                sessions: &runtime_state.sessions,
                            },
                            parties: &runtime_state.parties,
                            session_id,
                        },
                        &mut session,
                        &mut header_crypto,
                    )
                    .await?;
                    crate::observability::record_world_session_loop_phase_duration(
                        crate::observability::WorldSessionLoopPhase::CombatTick,
                        combat_tick_started_at.elapsed(),
                    );
                    let loot_timeouts_started_at = Instant::now();
                    handle_loot_roll_timeouts(
                        &mut stream,
                        LootMutationDeps {
                            character_db_pool: &character_db_pool,
                            world_db_pool: &world_db_pool,
                            shared_world: SharedWorldDeps {
                                object_mgr: runtime_state.object_mgr.as_ref(),
                                maps: &runtime_state.maps,
                                sessions: &runtime_state.sessions,
                            },
                            parties: &runtime_state.parties,
                        },
                        &mut session,
                        &mut header_crypto,
                    )
                    .await?;
                    crate::observability::record_world_session_loop_phase_duration(
                        crate::observability::WorldSessionLoopPhase::LootRollTimeouts,
                        loot_timeouts_started_at.elapsed(),
                    );
                    let sync_started_at = Instant::now();
                    sync_active_player_gameplay_state(&runtime_state.maps, &session).await;
                    crate::observability::record_world_session_loop_phase_duration(
                        crate::observability::WorldSessionLoopPhase::SyncGameplayState,
                        sync_started_at.elapsed(),
                    );
                    advance_world_tick_deadline(
                        &mut next_world_tick_at,
                        Instant::now(),
                        world_tick_interval,
                    );
                    crate::observability::record_world_session_loop_phase_duration(
                        crate::observability::WorldSessionLoopPhase::TimeoutBranchTotal,
                        timeout_branch_started_at.elapsed(),
                    );
                }
                }
            }
        }
    }
    .await;

    if let Some(handle) = runtime_state.sessions.unregister(session_id).await {
        debug!(
            ?session_id,
            account_id = handle.account_id,
            character_guid = ?handle.character_guid,
            outbound_closed = handle.outbound.is_closed(),
            "Unregistered world session"
        );
    }
    drop(stream);
    drop(outbound_tx);
    if let Err(join_error) = writer_task.await {
        warn!(
            ?session_id,
            "World session writer task failed to join: {}", join_error
        );
    }

    if session_result.is_err() {
        refresh_active_player_session_cache(&runtime_state.maps, &mut session).await;
        if !session.combat.player_in_combat {
            if let Err(cleanup_error) = persist_session_character_state(
                &character_db_pool,
                account.id,
                &runtime_state.maps,
                &session,
            )
            .await
            {
                warn!(
                    "Failed to persist active character state after world session error: {}",
                    cleanup_error
                );
            }
        }
        unregister_active_character_after_disconnect(
            &runtime_state.online_characters,
            &runtime_state.maps,
            &runtime_state.sessions,
            session_id,
            &mut session,
            Instant::now(),
        )
        .await;
    }

    session_result
}

pub(in crate::world) async fn world_session_writer(
    session_id: SessionId,
    mut write_stream: OwnedWriteHalf,
    mut outbound_rx: mpsc::Receiver<QueuedOutboundWorldPacket>,
    mut header_crypto: HeaderCrypto,
    disconnect_tx: mpsc::Sender<WorldSessionDisconnectReason>,
) {
    while let Some(queued) = outbound_rx.recv().await {
        crate::observability::record_world_packet_outbound_queue_latency(
            queued.packet.opcode,
            queued.enqueued_at.elapsed(),
        );
        let write_started_at = Instant::now();
        match timeout(
            WORLD_SESSION_WRITE_TIMEOUT,
            send_packet_direct(
                &mut write_stream,
                queued.packet.opcode,
                &queued.packet.body,
                Some(&mut header_crypto),
            ),
        )
        .await
        {
            Ok(Ok(())) => {
                crate::observability::record_world_packet_write_duration(
                    queued.packet.opcode,
                    write_started_at.elapsed(),
                );
            }
            Ok(Err(error)) => {
                let _ = disconnect_tx.try_send(WorldSessionDisconnectReason::WriteError);
                warn!(
                    ?session_id,
                    opcode = format_args!("0x{:04X}", queued.packet.opcode),
                    "World session writer stopped after socket write failed: {}",
                    error
                );
                break;
            }
            Err(_) => {
                let _ = disconnect_tx.try_send(WorldSessionDisconnectReason::WriteTimeout);
                warn!(
                    ?session_id,
                    opcode = format_args!("0x{:04X}", queued.packet.opcode),
                    timeout_ms = WORLD_SESSION_WRITE_TIMEOUT.as_millis(),
                    "World session writer stopped after socket write timed out"
                );
                break;
            }
        }
    }
}

pub(in crate::world) fn world_tick_timeout_duration(
    next_world_tick_at: Instant,
    now: Instant,
) -> Duration {
    next_world_tick_at.saturating_duration_since(now)
}

pub(in crate::world) async fn session_loop_timeout_duration(
    maps: &MapRuntimeManager,
    session: &WorldSessionState,
    next_world_tick_at: Instant,
    now: Instant,
) -> Duration {
    let world_tick_timeout = world_tick_timeout_duration(next_world_tick_at, now);
    let spell_timeout = next_pending_player_spell_cast_due_at(maps, session)
        .await
        .map(|due_at| {
            due_at
                .saturating_duration_since(now)
                .min(world_tick_timeout)
        })
        .unwrap_or(world_tick_timeout);
    pending_logout_due_at(session)
        .map(|due_at| due_at.saturating_duration_since(now).min(spell_timeout))
        .unwrap_or(spell_timeout)
}

pub(in crate::world) fn advance_world_tick_deadline(
    next_world_tick_at: &mut Instant,
    now: Instant,
    world_tick_interval: Duration,
) {
    debug_assert!(!world_tick_interval.is_zero());
    while *next_world_tick_at <= now {
        *next_world_tick_at += world_tick_interval;
    }
}

pub(in crate::world) async fn refresh_active_player_session_cache(
    maps: &Arc<MapRuntimeManager>,
    session: &mut WorldSessionState,
) -> bool {
    let Some(character) = session.character.active_character.as_ref() else {
        return false;
    };
    let map_id = character.position.map_id;
    let character_guid = character.guid;
    let Some(snapshot) = maps.player_runtime_snapshot(map_id, character_guid).await else {
        return false;
    };

    let map_player_died = session.death.player_death_state == PlayerDeathState::Alive
        && snapshot.death_state != PlayerDeathState::Alive
        && snapshot.health == 0;
    session.character.player_health = snapshot.health;
    session.death.player_death_state = snapshot.death_state;
    session.death.player_death_presentation_pending =
        snapshot.death_state == PlayerDeathState::JustDied;
    session.character.player_stand_state = snapshot.stand_state;
    session.character.player_mana = snapshot.power1;
    session.character.player_rage = snapshot.power2;
    session.character.player_energy = snapshot.power4;
    session.combat.player_in_combat = snapshot.in_combat;
    session.character.active_spells = snapshot.active_spells;
    session.character.spell_global_cooldowns_until = snapshot.spell_global_cooldowns_until;
    session.character.spell_cooldowns_until = snapshot.spell_cooldowns_until;
    session.character.spell_cooldown_categories = snapshot.spell_cooldown_categories;
    session.character.spell_cooldown_item_ids = snapshot.spell_cooldown_item_ids;
    session.inventory.items = snapshot.inventory;
    session.quests.quest_statuses = snapshot.quest_statuses;
    session.auras.active_auras = snapshot.active_auras;
    session.character.player_flags = snapshot.flags;
    if let Some(character) = session.character.active_character.as_mut() {
        character.position = snapshot.position;
        character.movement_flags = snapshot.movement_flags;
        character.client_time = snapshot.client_time;
        character.fall_time = snapshot.fall_time;
        character.jump = snapshot.jump;
        character.level = snapshot.level;
        character.xp = snapshot.xp;
    }
    map_player_died
}

pub(in crate::world) async fn finalize_map_owned_player_death_if_needed(
    _stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    account_id: u32,
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    _header_crypto: &mut HeaderCrypto,
    map_player_died: bool,
) -> anyhow::Result<bool> {
    if !map_player_died || session.character.player_health != 0 {
        return Ok(false);
    }
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(false);
    };
    let map_id = character.position.map_id;
    let character_guid = character.guid;
    session.death.player_death_presentation_pending =
        session.death.player_death_state == PlayerDeathState::JustDied;
    session.death.player_corpse = None;
    session.character.player_health = 0;
    session.auras.active_auras.clear();
    session.combat.player_in_combat = false;
    mirror_session_player_auto_attack(session, None, None);
    clear_session_active_creature_combats(session);
    shared_world
        .maps
        .set_player_auto_attack(map_id, character_guid, None, None)
        .await;
    persist_player_death_state(character_db_pool, account_id, session).await?;
    Ok(session.death.player_death_state == PlayerDeathState::Corpse)
}

pub(in crate::world) async fn sync_active_player_gameplay_state(
    maps: &Arc<MapRuntimeManager>,
    session: &WorldSessionState,
) {
    let Some(character) = session.character.active_character.as_ref() else {
        return;
    };
    maps.sync_player_gameplay_state(character.position.map_id, character.guid, session)
        .await;
}
