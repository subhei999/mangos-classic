use rand::Rng;
use sha1::{Digest, Sha1};
use sqlx::mysql::MySqlPool;
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio::time::{timeout, Duration, Instant};
use tracing::{debug, error, info, warn};
use wow_common::guid::{write_guid, HighGuid, ObjectGuid, PackedGuid};
use wow_common::position::WorldPosition;
use wow_crypto::HeaderCrypto;
use wow_db::{
    CharacterAction, CharacterDeleteOptions, CharacterEnumEntry, CharacterInventoryItem,
    CharacterNameQuery, CharacterQuestStatus, CharacterReputation, CharacterSkill, CharacterSpell,
    CreatureLootQuery, CreatureSpawnQuery, CreatureTemplateQuery, ItemTemplateQuery, NewCharacter,
    NewPlayerCorpse, PlayerCorpseQuery, PlayerWorldStats, QuestTemplateQuery,
};

include!("opcodes.rs");
include!("session.rs");

pub struct WorldServer {
    bind_addr: SocketAddr,
    login_db_pool: MySqlPool,
    character_db_pool: MySqlPool,
    world_db_pool: MySqlPool,
    runtime_state: WorldRuntimeState,
}

impl WorldServer {
    pub fn new(
        bind_addr: SocketAddr,
        login_db_pool: MySqlPool,
        character_db_pool: MySqlPool,
        world_db_pool: MySqlPool,
        delete_options: CharacterDeleteOptions,
        data_dir: impl Into<std::path::PathBuf>,
    ) -> Self {
        let world_data_files = Arc::new(WorldDataFiles::inspect(data_dir));
        info!(
            data_dir = %world_data_files.data_dir.display(),
            maps = world_data_files.maps_available,
            vmaps = world_data_files.vmaps_available,
            mmap_maps = world_data_files.mmap_headers.len(),
            mmap_tiles = world_data_files.mmap_tiles.len(),
            "World data files inspected",
        );
        if world_data_files.mmap_tiles.is_empty() {
            warn!(
                data_dir = %world_data_files.data_dir.display(),
                "No mmap tiles found; DB creature pathing will use the permissive fallback",
            );
        }
        Self {
            bind_addr,
            login_db_pool,
            character_db_pool,
            world_db_pool,
            runtime_state: WorldRuntimeState {
                online_characters: Arc::new(Mutex::new(HashSet::new())),
                player_corpses: Arc::new(Mutex::new(HashMap::new())),
                delete_options,
                world_data_files,
                sessions: Arc::new(SessionRegistry::default()),
                maps: Arc::new(MapRuntimeManager::default()),
            },
        }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let listener = TcpListener::bind(self.bind_addr).await?;
        info!("World server listening on {}", self.bind_addr);

        loop {
            match listener.accept().await {
                Ok((socket, peer)) => {
                    info!(%peer, "Accepted world connection");
                    let login_pool = self.login_db_pool.clone();
                    let character_pool = self.character_db_pool.clone();
                    let world_pool = self.world_db_pool.clone();
                    let runtime_state = self.runtime_state.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_client(
                            socket,
                            login_pool,
                            character_pool,
                            world_pool,
                            runtime_state,
                        )
                        .await
                        {
                            warn!(%peer, "World session ended with error: {}", e);
                        }
                    });
                }
                Err(e) => error!("Failed to accept world connection: {}", e),
            }
        }
    }
}

async fn handle_client(
    mut stream: TcpStream,
    login_db_pool: MySqlPool,
    character_db_pool: MySqlPool,
    world_db_pool: MySqlPool,
    runtime_state: WorldRuntimeState,
) -> anyhow::Result<()> {
    send_packet_direct(
        &mut stream,
        SMSG_AUTH_CHALLENGE,
        &SERVER_SEED.to_le_bytes(),
        None,
    )
    .await?;

    let (opcode, payload) = read_client_packet(&mut stream, None).await?;
    if opcode != CMSG_AUTH_SESSION {
        anyhow::bail!("expected CMSG_AUTH_SESSION, got 0x{opcode:04X}");
    }

    let auth = AuthSessionPacket::read(&payload)?;
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
    let (outbound_tx, outbound_rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(outbound_tx.clone());
    runtime_state
        .sessions
        .register(
            session_id,
            SessionHandle {
                account_id: account.id,
                character_guid: None,
                outbound: outbound_tx.clone(),
            },
        )
        .await;
    let writer_task = tokio::spawn(world_session_writer(
        session_id,
        write_stream,
        outbound_rx,
        HeaderCrypto::new(&session_key),
    ));

    let mut read_header_crypto = HeaderCrypto::new(&session_key);
    let mut header_crypto = HeaderCrypto::new(&session_key);
    send_auth_ok(&mut stream, Some(&mut header_crypto)).await?;
    let mut session = WorldSessionState {
        db_creature_navigation: DbCreatureNavigationGuardrail {
            world_data_files: runtime_state.world_data_files.clone(),
            ..DbCreatureNavigationGuardrail::default()
        },
        ..WorldSessionState::default()
    };
    let mut next_world_tick_at = Instant::now() + Duration::from_millis(WORLD_TICK_MILLIS);

    let session_result: anyhow::Result<()> = async {
        loop {
            match timeout(
                world_tick_timeout_duration(next_world_tick_at, Instant::now()),
                read_client_packet(&mut read_stream, Some(&mut read_header_crypto)),
            )
            .await
            {
                Ok(Ok((opcode, body))) => {
                    if is_movement_opcode(opcode) {
                        debug!(
                            opcode = format_args!("0x{opcode:04X}"),
                            bytes = body.len(),
                            "Received movement packet after auth"
                        );
                    } else {
                        info!(
                            opcode = format_args!("0x{opcode:04X}"),
                            bytes = body.len(),
                            "Received world packet after auth"
                        );
                    }

                    match opcode {
                        CMSG_CHAR_CREATE => {
                            handle_char_create(
                                &mut stream,
                                &login_db_pool,
                                &character_db_pool,
                                &world_db_pool,
                                account.id,
                                &body,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_CHAR_ENUM => {
                            let characters =
                                wow_db::get_character_enum_entries(&character_db_pool, account.id)
                                    .await?;
                            info!(
                                account = %auth.account,
                                count = characters.len(),
                                "Sending character enum"
                            );
                            send_char_enum(&mut stream, &characters, Some(&mut header_crypto))
                                .await?;
                        }
                        CMSG_CHAR_DELETE => {
                            handle_char_delete(
                                &mut stream,
                                &login_db_pool,
                                &character_db_pool,
                                account.id,
                                &body,
                                &mut header_crypto,
                                &runtime_state,
                            )
                            .await?;
                        }
                        CMSG_PLAYER_LOGIN => {
                            handle_player_login(
                                &mut stream,
                                PlayerLoginDeps {
                                    character_db_pool: &character_db_pool,
                                    world_db_pool: &world_db_pool,
                                    online_characters: &runtime_state.online_characters,
                                    player_corpses: &runtime_state.player_corpses,
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
                                    session_id,
                                },
                                account.id,
                                &body,
                                &mut header_crypto,
                                &mut session,
                            )
                            .await?;
                        }
                        CMSG_PING => {
                            handle_ping(&mut stream, &body, Some(&mut header_crypto)).await?;
                        }
                        CMSG_NAME_QUERY => {
                            handle_name_query(
                                &mut stream,
                                &character_db_pool,
                                &body,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_ITEM_QUERY_SINGLE => {
                            handle_item_query_single(
                                &mut stream,
                                &world_db_pool,
                                &body,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_CREATURE_QUERY => {
                            handle_creature_query(
                                &mut stream,
                                &world_db_pool,
                                &body,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_QUEST_QUERY => {
                            handle_quest_query(
                                &mut stream,
                                &world_db_pool,
                                &body,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_MESSAGECHAT => {
                            handle_message_chat(
                                &mut stream,
                                ChatDeps {
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
                                },
                                &body,
                                &session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_QUERY_TIME => {
                            handle_query_time(&mut stream, &mut header_crypto).await?;
                        }
                        CMSG_REQUEST_ACCOUNT_DATA => {
                            handle_request_account_data(&mut stream, &body, &mut header_crypto)
                                .await?;
                        }
                        CMSG_UPDATE_ACCOUNT_DATA => {
                            handle_update_account_data(&body);
                        }
                        CMSG_TUTORIAL_FLAG => {
                            handle_tutorial_flag(&character_db_pool, account.id, &body).await?;
                        }
                        CMSG_TUTORIAL_CLEAR => {
                            handle_tutorial_clear(&character_db_pool, account.id).await?;
                        }
                        CMSG_TUTORIAL_RESET => {
                            handle_tutorial_reset(&character_db_pool, account.id).await?;
                        }
                        CMSG_TEXT_EMOTE => {
                            handle_text_emote(&mut stream, &body, &session, &mut header_crypto)
                                .await?;
                        }
                        CMSG_CAST_SPELL => {
                            handle_cast_spell(
                                &mut stream,
                                &character_db_pool,
                                &world_db_pool,
                                SharedWorldDeps {
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
                                },
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_AUTOEQUIP_ITEM | CMSG_SWAP_ITEM | CMSG_SWAP_INV_ITEM => {
                            handle_inventory_swap(
                                &mut stream,
                                &character_db_pool,
                                &world_db_pool,
                                opcode,
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_DESTROYITEM => {
                            handle_destroy_item(
                                &mut stream,
                                &character_db_pool,
                                &world_db_pool,
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_SPLIT_ITEM => {
                            handle_split_item(
                                &mut stream,
                                &character_db_pool,
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_CANCEL_CAST | CMSG_CANCEL_AUTO_REPEAT_SPELL => {
                            info!(
                                opcode = expected_noop_opcode_name(opcode),
                                "Ignoring spell cancel opcode for fixture spell slice"
                            );
                        }
                        CMSG_GOSSIP_HELLO => {
                            handle_gossip_hello(
                                &mut stream,
                                &world_db_pool,
                                &body,
                                &session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_GOSSIP_SELECT_OPTION => {
                            handle_gossip_select_option(
                                &mut stream,
                                GossipSelectDeps {
                                    character_db_pool: &character_db_pool,
                                    world_db_pool: &world_db_pool,
                                    player_corpses: &runtime_state.player_corpses,
                                    maps: &runtime_state.maps,
                                    account_id: account.id,
                                },
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_QUESTGIVER_STATUS_QUERY => {
                            handle_questgiver_status_query(
                                &mut stream,
                                &world_db_pool,
                                &body,
                                &session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_QUESTGIVER_HELLO => {
                            handle_questgiver_hello(
                                &mut stream,
                                &world_db_pool,
                                &body,
                                &session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_QUESTGIVER_QUERY_QUEST => {
                            handle_questgiver_query_quest(
                                &mut stream,
                                &world_db_pool,
                                &body,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_QUESTGIVER_ACCEPT_QUEST => {
                            handle_questgiver_accept_quest(
                                &mut stream,
                                &character_db_pool,
                                &world_db_pool,
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_QUESTGIVER_COMPLETE_QUEST | CMSG_QUESTGIVER_REQUEST_REWARD => {
                            handle_questgiver_complete_quest(
                                &mut stream,
                                &world_db_pool,
                                &body,
                                &session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_QUESTGIVER_CHOOSE_REWARD => {
                            handle_questgiver_choose_reward(
                                &mut stream,
                                &character_db_pool,
                                &world_db_pool,
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_NPC_TEXT_QUERY => {
                            handle_npc_text_query(&mut stream, &body, &mut header_crypto).await?;
                        }
                        CMSG_LIST_INVENTORY => {
                            handle_list_inventory(
                                &mut stream,
                                &world_db_pool,
                                &body,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_SELL_ITEM => {
                            handle_sell_item(
                                &mut stream,
                                &character_db_pool,
                                &world_db_pool,
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_BUY_ITEM => {
                            handle_buy_item(
                                &mut stream,
                                &character_db_pool,
                                &world_db_pool,
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_TRAINER_LIST => {
                            handle_trainer_list(
                                &mut stream,
                                &character_db_pool,
                                &world_db_pool,
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_TRAINER_BUY_SPELL => {
                            handle_trainer_buy_spell(
                                &mut stream,
                                &character_db_pool,
                                &world_db_pool,
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_ATTACKSWING => {
                            handle_attack_swing(
                                &mut stream,
                                &character_db_pool,
                                &world_db_pool,
                                SharedWorldDeps {
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
                                },
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_ATTACKSTOP => {
                            handle_attack_stop(&mut stream, &mut session, &mut header_crypto)
                                .await?;
                        }
                        CMSG_REPOP_REQUEST => {
                            handle_repop_request(
                                &mut stream,
                                PlayerDeathDeps {
                                    character_db_pool: &character_db_pool,
                                    world_db_pool: &world_db_pool,
                                    player_corpses: &runtime_state.player_corpses,
                                    maps: &runtime_state.maps,
                                    account_id: account.id,
                                },
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_RECLAIM_CORPSE => {
                            handle_reclaim_corpse(
                                &mut stream,
                                PlayerDeathDeps {
                                    character_db_pool: &character_db_pool,
                                    world_db_pool: &world_db_pool,
                                    player_corpses: &runtime_state.player_corpses,
                                    maps: &runtime_state.maps,
                                    account_id: account.id,
                                },
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_SPIRIT_HEALER_ACTIVATE => {
                            handle_spirit_healer_activate(
                                &mut stream,
                                PlayerDeathDeps {
                                    character_db_pool: &character_db_pool,
                                    world_db_pool: &world_db_pool,
                                    player_corpses: &runtime_state.player_corpses,
                                    maps: &runtime_state.maps,
                                    account_id: account.id,
                                },
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        MSG_CORPSE_QUERY => {
                            handle_corpse_query(&mut stream, &session, &mut header_crypto).await?;
                        }
                        CMSG_LOOT => {
                            handle_loot(
                                &mut stream,
                                &world_db_pool,
                                SharedWorldDeps {
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
                                },
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_AUTOSTORE_LOOT_ITEM => {
                            handle_autostore_loot_item(
                                &mut stream,
                                &character_db_pool,
                                &world_db_pool,
                                SharedWorldDeps {
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
                                },
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_LOOT_MONEY => {
                            handle_loot_money(
                                &mut stream,
                                &character_db_pool,
                                SharedWorldDeps {
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
                                },
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_LOOT_RELEASE => {
                            handle_loot_release(
                                &mut stream,
                                SharedWorldDeps {
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
                                },
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_GMTICKET_GETTICKET => {
                            handle_gmticket_getticket(&mut stream, &mut header_crypto).await?;
                        }
                        CMSG_SET_ACTIVE_MOVER => {
                            handle_set_active_mover(&body, &session)?;
                        }
                        MSG_QUERY_NEXT_MAIL_TIME => {
                            handle_query_next_mail_time(
                                &mut stream,
                                &character_db_pool,
                                &session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_LOGOUT_REQUEST => {
                            handle_logout_request(
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
                            )
                            .await?;
                        }
                        CMSG_LOGOUT_CANCEL => {
                            handle_logout_cancel(&mut stream, &mut header_crypto).await?;
                        }
                        CMSG_PLAYER_LOGOUT => {
                            info!("Received client-side player logout notification");
                        }
                        _ if is_movement_opcode(opcode) => {
                            handle_movement(
                                &mut stream,
                                MovementDeps {
                                    character_db_pool: &character_db_pool,
                                    world_db_pool: &world_db_pool,
                                    player_corpses: &runtime_state.player_corpses,
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
                                },
                                opcode,
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        _ if is_expected_noop_opcode(opcode) => {
                            info!(
                                opcode = expected_noop_opcode_name(opcode),
                                bytes = body.len(),
                                "Ignoring expected world bootstrap opcode"
                            );
                        }
                        _ => {
                            warn!(
                                opcode = format_args!("0x{opcode:04X}"),
                                "Unhandled authenticated world opcode"
                            );
                        }
                    }
                    if Instant::now() >= next_world_tick_at {
                        handle_combat_tick(
                            &mut stream,
                            &character_db_pool,
                            &world_db_pool,
                            SharedWorldDeps {
                                maps: &runtime_state.maps,
                                sessions: &runtime_state.sessions,
                            },
                            account.id,
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                        advance_world_tick_deadline(&mut next_world_tick_at, Instant::now());
                    }
                }
                Ok(Err(e)) => {
                    persist_session_character_state(&character_db_pool, account.id, &session)
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
                    handle_combat_tick(
                        &mut stream,
                        &character_db_pool,
                        &world_db_pool,
                        SharedWorldDeps {
                            maps: &runtime_state.maps,
                            sessions: &runtime_state.sessions,
                        },
                        account.id,
                        &mut session,
                        &mut header_crypto,
                    )
                    .await?;
                    advance_world_tick_deadline(&mut next_world_tick_at, Instant::now());
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
        if let Err(cleanup_error) =
            persist_session_character_state(&character_db_pool, account.id, &session).await
        {
            warn!(
                "Failed to persist active character state after world session error: {}",
                cleanup_error
            );
        }
        unregister_active_character(
            &runtime_state.online_characters,
            &runtime_state.maps,
            &runtime_state.sessions,
            session_id,
            &mut session,
        )
        .await;
    }

    session_result
}

async fn world_session_writer(
    session_id: SessionId,
    mut write_stream: OwnedWriteHalf,
    mut outbound_rx: mpsc::UnboundedReceiver<OutboundWorldPacket>,
    mut header_crypto: HeaderCrypto,
) {
    while let Some(packet) = outbound_rx.recv().await {
        if let Err(error) = send_packet_direct(
            &mut write_stream,
            packet.opcode,
            &packet.body,
            Some(&mut header_crypto),
        )
        .await
        {
            warn!(
                ?session_id,
                opcode = format_args!("0x{:04X}", packet.opcode),
                "World session writer stopped after socket write failed: {}",
                error
            );
            break;
        }
    }
}

fn world_tick_timeout_duration(next_world_tick_at: Instant, now: Instant) -> Duration {
    next_world_tick_at.saturating_duration_since(now)
}

fn advance_world_tick_deadline(next_world_tick_at: &mut Instant, now: Instant) {
    let tick = Duration::from_millis(WORLD_TICK_MILLIS);
    while *next_world_tick_at <= now {
        *next_world_tick_at += tick;
    }
}

#[derive(Debug, Clone, PartialEq)]
struct CharCreatePacket {
    name: String,
    race: u8,
    class: u8,
    gender: u8,
    skin: u8,
    face: u8,
    hair_style: u8,
    hair_color: u8,
    facial_hair: u8,
    outfit_id: u8,
}

impl CharCreatePacket {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        let name_end = body
            .iter()
            .position(|b| *b == 0)
            .ok_or_else(|| anyhow::anyhow!("CMSG_CHAR_CREATE name is not NUL-terminated"))?;
        let name = String::from_utf8(body[..name_end].to_vec())?;
        let cursor = name_end + 1;
        ensure_available(body, cursor + 9)?;

        Ok(Self {
            name,
            race: body[cursor],
            class: body[cursor + 1],
            gender: body[cursor + 2],
            skin: body[cursor + 3],
            face: body[cursor + 4],
            hair_style: body[cursor + 5],
            hair_color: body[cursor + 6],
            facial_hair: body[cursor + 7],
            outfit_id: body[cursor + 8],
        })
    }
}

fn normalize_character_name(name: &str) -> Result<String, u8> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(CHAR_NAME_NO_NAME);
    }
    if trimmed.len() < 2 {
        return Err(CHAR_NAME_TOO_SHORT);
    }
    if trimmed.len() > 12 {
        return Err(CHAR_NAME_TOO_LONG);
    }
    if !trimmed.bytes().all(|b| b.is_ascii_alphabetic()) {
        return Err(CHAR_NAME_INVALID_CHARACTER);
    }

    let mut chars = trimmed.chars();
    let first = chars.next().expect("empty name checked above");
    let normalized = first.to_ascii_uppercase().to_string() + &chars.as_str().to_ascii_lowercase();
    Ok(normalized)
}

fn is_valid_race_class(race: u8, class: u8) -> bool {
    matches!(
        (race, class),
        (1, 1 | 2 | 4 | 5 | 8 | 9)
            | (2, 1 | 3 | 4 | 7 | 9)
            | (3, 1..=5)
            | (4, 1 | 3 | 4 | 5 | 11)
            | (5, 1 | 4 | 5 | 8 | 9)
            | (6, 1 | 3 | 7 | 11)
            | (7, 1 | 4 | 8 | 9)
            | (8, 1 | 3 | 4 | 5 | 7 | 8)
    )
}

async fn send_auth_response(stream: &mut TcpStream, response: u8) -> anyhow::Result<()> {
    send_packet_direct(stream, SMSG_AUTH_RESPONSE, &[response], None).await
}

async fn send_auth_ok(
    stream: &mut WorldPacketSink,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let mut body = Vec::with_capacity(11);
    body.push(AUTH_OK);
    body.extend_from_slice(&0u32.to_le_bytes()); // BillingTimeRemaining
    body.push(0); // BillingPlanFlags
    body.extend_from_slice(&0u32.to_le_bytes()); // BillingTimeRested
    body.push(0); // expansion
    send_packet(stream, SMSG_AUTH_RESPONSE, &body, header_crypto).await
}

async fn send_char_enum(
    stream: &mut WorldPacketSink,
    characters: &[CharacterEnumEntry],
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_char_enum_body(characters)?;
    send_packet(stream, SMSG_CHAR_ENUM, &body, header_crypto).await
}

async fn handle_char_delete(
    stream: &mut WorldPacketSink,
    login_db_pool: &MySqlPool,
    character_db_pool: &MySqlPool,
    account_id: u32,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
    runtime_state: &WorldRuntimeState,
) -> anyhow::Result<()> {
    if body.len() != 8 {
        warn!("Rejected malformed CMSG_CHAR_DELETE bytes={}", body.len());
        return send_char_delete_result(stream, CHAR_DELETE_FAILED, Some(header_crypto)).await;
    }

    let raw_guid = u64::from_le_bytes(body.try_into()?);
    let guid = ObjectGuid::from_raw(raw_guid).counter();
    if runtime_state.online_characters.lock().await.contains(&guid) {
        warn!(account_id, guid, "Rejected loaded character delete");
        return send_char_delete_result(stream, CHAR_DELETE_FAILED, Some(header_crypto)).await;
    }
    if wow_db::is_guild_leader(character_db_pool, guid).await? {
        warn!(account_id, guid, "Rejected guild leader character delete");
        return send_char_delete_result(stream, CHAR_DELETE_FAILED, Some(header_crypto)).await;
    }

    let deleted = wow_db::delete_character_with_options(
        character_db_pool,
        account_id,
        guid,
        runtime_state.delete_options,
    )
    .await?;
    if deleted {
        let count = wow_db::refresh_realm_character_count(
            login_db_pool,
            character_db_pool,
            account_id,
            REALM_ID,
        )
        .await?;
        info!(account_id, guid, count, "Deleted character");
        send_char_delete_result(stream, CHAR_DELETE_SUCCESS, Some(header_crypto)).await
    } else {
        warn!(account_id, guid, "Rejected character delete");
        send_char_delete_result(stream, CHAR_DELETE_FAILED, Some(header_crypto)).await
    }
}

async fn send_char_delete_result(
    stream: &mut WorldPacketSink,
    result: u8,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    send_packet(stream, SMSG_CHAR_DELETE, &[result], header_crypto).await
}

async fn handle_char_create(
    stream: &mut WorldPacketSink,
    login_db_pool: &MySqlPool,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    account_id: u32,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let create = match CharCreatePacket::read(body) {
        Ok(create) => create,
        Err(e) => {
            warn!("Rejected malformed CMSG_CHAR_CREATE: {}", e);
            send_char_create_result(stream, CHAR_CREATE_FAILED, Some(header_crypto)).await?;
            return Ok(());
        }
    };

    let name = match normalize_character_name(&create.name) {
        Ok(name) => name,
        Err(code) => {
            send_char_create_result(stream, code, Some(header_crypto)).await?;
            return Ok(());
        }
    };

    if !is_valid_race_class(create.race, create.class) || create.gender > 1 {
        warn!(
            account_id,
            race = create.race,
            class = create.class,
            gender = create.gender,
            "Rejected invalid character create attributes"
        );
        send_char_create_result(stream, CHAR_CREATE_FAILED, Some(header_crypto)).await?;
        return Ok(());
    }

    if wow_db::character_name_exists(character_db_pool, &name).await? {
        send_char_create_result(stream, CHAR_CREATE_NAME_IN_USE, Some(header_crypto)).await?;
        return Ok(());
    }

    let char_count = wow_db::character_count_for_account(character_db_pool, account_id).await?;
    if char_count >= MAX_CHARACTERS_PER_REALM {
        send_char_create_result(stream, CHAR_CREATE_SERVER_LIMIT, Some(header_crypto)).await?;
        return Ok(());
    }

    let created = wow_db::create_character(
        character_db_pool,
        world_db_pool,
        NewCharacter {
            account_id,
            name,
            race: create.race,
            class: create.class,
            gender: create.gender,
            skin: create.skin,
            face: create.face,
            hair_style: create.hair_style,
            hair_color: create.hair_color,
            facial_hair: create.facial_hair,
        },
    )
    .await?;

    let new_count = wow_db::refresh_realm_character_count(
        login_db_pool,
        character_db_pool,
        account_id,
        REALM_ID,
    )
    .await?;

    info!(
        account_id,
        guid = created.guid,
        name = %created.name,
        race = created.race,
        class = created.class,
        count = new_count,
        "Created character"
    );

    send_char_create_result(stream, CHAR_CREATE_SUCCESS, Some(header_crypto)).await
}

async fn send_char_create_result(
    stream: &mut WorldPacketSink,
    result: u8,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    send_packet(stream, SMSG_CHAR_CREATE, &[result], header_crypto).await
}

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
        deps.player_corpses
            .lock()
            .await
            .insert(character.guid, corpse.clone());
    }
    session.combat_dummy_health = RUST_COMBAT_DUMMY_HEALTH;
    session.combat_dummy_lootable = false;
    session.combat_dummy_looting = false;
    session.combat_dummy_loot_money_available = false;
    session.combat_dummy_loot_item_available = false;
    let nearby_creatures = wow_db::get_nearby_creature_spawns(
        deps.world_db_pool,
        character.map,
        character.position_x,
        character.position_y,
        CREATURE_SPAWN_RADIUS_YARDS,
        CREATURE_SPAWN_LIMIT,
    )
    .await?;
    let nearby_creature_runtimes =
        build_db_creature_runtimes_with_respawns(deps.character_db_pool, nearby_creatures).await?;
    let nearby_creature_runtimes = deps
        .maps
        .share_db_creature_snapshots(character.map, nearby_creature_runtimes)
        .await;
    let visible_nearby_creatures = visible_db_creature_spawns(&nearby_creature_runtimes);
    let nearby_db_player_corpses = wow_db::get_nearby_player_corpses(
        deps.character_db_pool,
        character.map,
        character.position_x,
        character.position_y,
        CREATURE_SPAWN_RADIUS_YARDS,
        PLAYER_CORPSE_VISIBILITY_LIMIT,
    )
    .await?
    .into_iter()
    .map(player_corpse_runtime_from_query)
    .collect::<Vec<_>>();
    let login_position = WorldPosition::new(
        character.map,
        character.position_x,
        character.position_y,
        character.position_z,
        character.orientation,
    );
    let nearby_player_corpses = merge_player_corpse_visibility(
        nearby_db_player_corpses,
        nearby_runtime_player_corpses(
            deps.player_corpses,
            login_position,
            CREATURE_SPAWN_RADIUS_YARDS,
            PLAYER_CORPSE_VISIBILITY_LIMIT,
        )
        .await,
    );
    session.visible_player_corpses = nearby_player_corpses
        .iter()
        .cloned()
        .map(|corpse| (corpse.guid.raw(), corpse))
        .collect();
    session.db_creatures = nearby_creature_runtimes
        .into_iter()
        .map(|creature| (creature.guid().raw(), creature))
        .collect();
    session.last_creature_visibility_position = Some(login_position);
    session.last_player_corpse_visibility_position = session.last_creature_visibility_position;
    session.player_health = character.health;
    session.player_rage = character.power2.min(POWER_RAGE_DEFAULT);
    session.player_mana = character.power1;
    session.inventory =
        wow_db::get_character_inventory_items(deps.character_db_pool, character.guid).await?;
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
            world_db_pool: deps.world_db_pool,
            character,
            inventory: &session.inventory,
            world_stats: &world_stats,
            spells: &spells,
            quest_statuses: &session.quest_statuses,
            tutorial_flags: &tutorial_flags,
            cinematic_sequence,
            nearby_creatures: &visible_nearby_creatures,
            nearby_player_corpses: &nearby_player_corpses,
        },
        Some(header_crypto),
    )
    .await?;
    deps.sessions
        .set_character_guid(deps.session_id, Some(character.guid))
        .await;
    let player_runtime = PlayerRuntime {
        guid: character.guid,
        account_id,
        session_id: deps.session_id,
        position: login_position,
        cell: cell_coord_for_position(login_position),
        visible_objects: HashSet::new(),
        visual: session
            .player_visual
            .clone()
            .ok_or_else(|| anyhow::anyhow!("active player visual missing after login"))?,
        flags: character.player_flags,
        level: character.level,
        race: character.race,
        class: character.class,
        gender: character.gender,
        health: session.player_health,
        max_health: world_stats.max_health().max(1),
        power1: session.player_mana,
        max_power1: world_stats.max_mana(),
        power2: session.player_rage,
        player_bytes: character.player_bytes,
        player_bytes2: character.player_bytes2,
    };
    let packets = deps.maps.add_player(player_runtime).await?;
    deps.sessions.dispatch(packets).await;

    Ok(())
}

struct PlayerLoginDeps<'a> {
    character_db_pool: &'a MySqlPool,
    world_db_pool: &'a MySqlPool,
    online_characters: &'a OnlineCharacters,
    player_corpses: &'a PlayerCorpses,
    maps: &'a Arc<MapRuntimeManager>,
    sessions: &'a Arc<SessionRegistry>,
    session_id: SessionId,
}

async fn handle_logout_request(
    stream: &mut WorldPacketSink,
    deps: LogoutDeps<'_>,
    header_crypto: &mut HeaderCrypto,
    session: &mut WorldSessionState,
) -> anyhow::Result<()> {
    if let Some(character) = &session.active_character {
        info!(
            guid = character.guid,
            name = %character.name,
            x = character.position.x,
            y = character.position.y,
            z = character.position.z,
            o = character.position.orientation,
            "Completing instant logout to character selection"
        );
    } else {
        info!("Completing logout request before character login");
    }

    let mut body = Vec::with_capacity(5);
    body.extend_from_slice(&0u32.to_le_bytes()); // no logout failure reason
    body.push(1); // instant logout, matching rested/GM-style response shape
    send_packet(
        stream,
        SMSG_LOGOUT_RESPONSE,
        &body,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(stream, SMSG_LOGOUT_COMPLETE, &[], Some(header_crypto)).await?;
    persist_session_character_state(deps.character_db_pool, deps.account_id, session).await?;
    unregister_active_character(
        deps.online_characters,
        deps.maps,
        deps.sessions,
        deps.session_id,
        session,
    )
    .await;
    Ok(())
}

struct LogoutDeps<'a> {
    character_db_pool: &'a MySqlPool,
    online_characters: &'a OnlineCharacters,
    maps: &'a Arc<MapRuntimeManager>,
    sessions: &'a Arc<SessionRegistry>,
    account_id: u32,
    session_id: SessionId,
}

async fn persist_session_character_state(
    character_db_pool: &MySqlPool,
    account_id: u32,
    session: &WorldSessionState,
) -> anyhow::Result<()> {
    if session.player_death_state == PlayerDeathState::Alive {
        persist_active_character_position(character_db_pool, account_id, session).await
    } else {
        persist_player_death_state(character_db_pool, account_id, session).await
    }
}

async fn unregister_active_character(
    online_characters: &OnlineCharacters,
    maps: &Arc<MapRuntimeManager>,
    sessions: &Arc<SessionRegistry>,
    session_id: SessionId,
    session: &mut WorldSessionState,
) {
    if let Some(character) = session.active_character.take() {
        online_characters.lock().await.remove(&character.guid);
        sessions.set_character_guid(session_id, None).await;
        let packets = maps
            .remove_player(character.position.map_id, character.guid)
            .await;
        sessions.dispatch(packets).await;
    }
    session.active_spells.clear();
    session.player_death_state = PlayerDeathState::Alive;
    session.player_corpse = None;
    session.visible_player_corpses.clear();
    session.player_visual = None;
    session.player_flags = 0;
}

fn current_unix_epoch_secs_u64() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

async fn build_db_creature_runtimes_with_respawns(
    character_db_pool: &MySqlPool,
    spawns: Vec<CreatureSpawnQuery>,
) -> anyhow::Result<Vec<DbCreatureRuntime>> {
    let now = Instant::now();
    let now_epoch_secs = current_unix_epoch_secs_u64();
    let guids = spawns.iter().map(|spawn| spawn.guid).collect::<Vec<_>>();
    let respawn_times =
        wow_db::get_creature_respawn_times(character_db_pool, &guids, 0, now_epoch_secs).await?;
    Ok(spawns
        .into_iter()
        .map(|spawn| {
            let respawn_epoch_secs = respawn_times.get(&spawn.guid).copied();
            DbCreatureRuntime::new_with_persisted_respawn(
                spawn,
                now,
                now_epoch_secs,
                respawn_epoch_secs,
            )
        })
        .collect())
}

fn visible_db_creature_spawns(creatures: &[DbCreatureRuntime]) -> Vec<CreatureSpawnQuery> {
    creatures
        .iter()
        .filter(|creature| creature.life_state != DbCreatureLifeState::Dead)
        .map(|creature| creature.spawn.clone())
        .collect()
}

async fn persist_active_character_position(
    character_db_pool: &MySqlPool,
    account_id: u32,
    session: &WorldSessionState,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        return Ok(());
    };

    let rows = wow_db::update_character_position(
        character_db_pool,
        account_id,
        character.guid,
        character.position,
    )
    .await?;

    if rows == 0 {
        warn!(
            account_id,
            guid = character.guid,
            "No character row updated while persisting position"
        );
    } else {
        info!(
            account_id,
            guid = character.guid,
            name = %character.name,
            x = character.position.x,
            y = character.position.y,
            z = character.position.z,
            o = character.position.orientation,
            "Persisted character position"
        );
    }

    Ok(())
}

async fn handle_logout_cancel(
    stream: &mut WorldPacketSink,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    send_packet(stream, SMSG_LOGOUT_CANCEL_ACK, &[], Some(header_crypto)).await
}

async fn handle_movement(
    stream: &mut WorldPacketSink,
    deps: MovementDeps<'_>,
    opcode: u32,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let movement = MovementInfo::read(body)?;
    if let Some(character) = &mut session.active_character {
        character.position.x = movement.position.x;
        character.position.y = movement.position.y;
        character.position.z = movement.position.z;
        character.position.orientation = movement.position.orientation;
        character.movement_flags = movement.flags;
        character.client_time = movement.client_time;
        character.fall_time = movement.fall_time;
        debug!(
            opcode = movement_opcode_name(opcode),
            guid = character.guid,
            name = %character.name,
            flags = format_args!("0x{:08X}", movement.flags),
            client_time = movement.client_time,
            x = movement.position.x,
            y = movement.position.y,
            z = movement.position.z,
            o = movement.position.orientation,
            "Updated in-memory character movement"
        );
        if let Ok(server_opcode) = u16::try_from(opcode) {
            let mut broadcast_movement = movement.clone();
            broadcast_movement.position.map_id = character.position.map_id;
            let packets = deps
                .maps
                .update_player_position(
                    character.position.map_id,
                    character.guid,
                    server_opcode,
                    &broadcast_movement,
                )
                .await?;
            deps.sessions.dispatch(packets).await;
        }
        stream_newly_visible_db_creatures(
            stream,
            deps.character_db_pool,
            deps.world_db_pool,
            deps.maps,
            session,
            header_crypto,
        )
        .await?;
        stream_nearby_player_corpses(
            stream,
            deps.character_db_pool,
            deps.player_corpses,
            session,
            header_crypto,
        )
        .await?;
        try_start_db_creature_aggro(
            stream,
            SharedWorldDeps {
                maps: deps.maps,
                sessions: deps.sessions,
            },
            session,
            header_crypto,
        )
        .await?;
    } else {
        warn!(
            opcode = movement_opcode_name(opcode),
            "Received movement packet before character login"
        );
    }
    Ok(())
}

struct MovementDeps<'a> {
    character_db_pool: &'a MySqlPool,
    world_db_pool: &'a MySqlPool,
    player_corpses: &'a PlayerCorpses,
    maps: &'a Arc<MapRuntimeManager>,
    sessions: &'a Arc<SessionRegistry>,
}

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
    if !should_rescan_db_creature_visibility(session, character.position) {
        return Ok(());
    }
    let guid = character.guid;
    let name = character.name.clone();
    let position = character.position;
    session.last_creature_visibility_position = Some(position);

    let nearby_creatures = wow_db::get_nearby_creature_spawns(
        world_db_pool,
        position.map_id,
        position.x,
        position.y,
        CREATURE_SPAWN_RADIUS_YARDS,
        CREATURE_SPAWN_LIMIT,
    )
    .await?;
    let nearby_creature_runtimes =
        build_db_creature_runtimes_with_respawns(character_db_pool, nearby_creatures).await?;
    let nearby_creature_runtimes = maps
        .share_db_creature_snapshots(position.map_id, nearby_creature_runtimes)
        .await;
    let visibility_updates =
        stage_db_creature_visibility_updates(session, position, nearby_creature_runtimes)?;
    if visibility_updates.create_bodies.is_empty() && visibility_updates.destroy_guids.is_empty() {
        return Ok(());
    }

    info!(
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
    player_corpses: &PlayerCorpses,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.active_character.as_ref() else {
        return Ok(());
    };
    let position = character.position;
    if !should_rescan_player_corpse_visibility(session, position) {
        return Ok(());
    }
    session.last_player_corpse_visibility_position = Some(position);
    let nearby_db_corpses = wow_db::get_nearby_player_corpses(
        character_db_pool,
        position.map_id,
        position.x,
        position.y,
        CREATURE_SPAWN_RADIUS_YARDS,
        PLAYER_CORPSE_VISIBILITY_LIMIT,
    )
    .await?
    .into_iter()
    .map(player_corpse_runtime_from_query)
    .collect::<Vec<_>>();
    let nearby_corpses = merge_player_corpse_visibility(
        nearby_db_corpses,
        nearby_runtime_player_corpses(
            player_corpses,
            position,
            CREATURE_SPAWN_RADIUS_YARDS,
            PLAYER_CORPSE_VISIBILITY_LIMIT,
        )
        .await,
    );
    let nearby_guids = nearby_corpses
        .iter()
        .map(|corpse| corpse.guid.raw())
        .collect::<HashSet<_>>();
    let mut destroy_guids = Vec::new();
    for (guid, corpse) in &session.visible_player_corpses {
        if !nearby_guids.contains(guid)
            && !is_position_inside_radius(
                corpse.position,
                position,
                CREATURE_VISIBILITY_UNLOAD_RADIUS_YARDS,
            )
        {
            destroy_guids.push(*guid);
        }
    }
    for guid in &destroy_guids {
        session.visible_player_corpses.remove(guid);
    }
    let new_corpses = nearby_corpses
        .into_iter()
        .filter(|corpse| {
            !session
                .visible_player_corpses
                .contains_key(&corpse.guid.raw())
        })
        .collect::<Vec<_>>();
    let create_blocks = new_corpses
        .iter()
        .map(build_player_corpse_create_block)
        .collect::<anyhow::Result<Vec<_>>>()?;
    for corpse in new_corpses {
        session
            .visible_player_corpses
            .insert(corpse.guid.raw(), corpse);
    }

    for guid in destroy_guids {
        send_packet(
            stream,
            SMSG_DESTROY_OBJECT,
            &build_destroy_guid_body(ObjectGuid::from_raw(guid)),
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

async fn nearby_runtime_player_corpses(
    player_corpses: &PlayerCorpses,
    position: WorldPosition,
    radius: f32,
    limit: u32,
) -> Vec<PlayerCorpseRuntime> {
    let radius_squared = radius * radius;
    let mut corpses = player_corpses
        .lock()
        .await
        .values()
        .filter(|corpse| {
            corpse.position.map_id == position.map_id
                && distance_squared_2d(corpse.position.x, corpse.position.y, position.x, position.y)
                    <= radius_squared
        })
        .cloned()
        .collect::<Vec<_>>();
    corpses.sort_by(|left, right| {
        distance_squared_2d(left.position.x, left.position.y, position.x, position.y)
            .partial_cmp(&distance_squared_2d(
                right.position.x,
                right.position.y,
                position.x,
                position.y,
            ))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    corpses.truncate(limit as usize);
    corpses
}

fn merge_player_corpse_visibility(
    db_corpses: Vec<PlayerCorpseRuntime>,
    runtime_corpses: Vec<PlayerCorpseRuntime>,
) -> Vec<PlayerCorpseRuntime> {
    let mut merged = db_corpses
        .into_iter()
        .map(|corpse| (corpse.guid.raw(), corpse))
        .collect::<HashMap<_, _>>();
    for corpse in runtime_corpses {
        merged.insert(corpse.guid.raw(), corpse);
    }
    merged.into_values().collect()
}

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

    let mut create_blocks = Vec::new();
    for runtime in nearby_creatures {
        let guid = runtime.guid().raw();
        if let Some(creature) = session.db_creatures.get_mut(&guid) {
            if creature.life_state != DbCreatureLifeState::Alive
                && runtime.life_state == DbCreatureLifeState::Alive
            {
                if !creature.client_visible && creature.life_state != DbCreatureLifeState::Dead {
                    creature.client_visible = true;
                    create_blocks.push(build_db_creature_runtime_create_block(creature)?);
                }
                continue;
            }
            let was_visible = creature.client_visible;
            let became_dead = runtime.life_state == DbCreatureLifeState::Dead && was_visible;
            *creature = runtime;
            creature.client_visible = was_visible && !became_dead;
            if became_dead {
                if !destroy_guids.contains(&guid) {
                    destroy_guids.push(guid);
                }
                continue;
            }
            if !was_visible && creature.life_state != DbCreatureLifeState::Dead {
                creature.client_visible = true;
                create_blocks.push(build_db_creature_runtime_create_block(creature)?);
            }
            continue;
        }
        if runtime.life_state != DbCreatureLifeState::Dead {
            create_blocks.push(build_db_creature_runtime_create_block(&runtime)?);
        }
        session.db_creatures.insert(guid, runtime);
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

include!("bootstrap.rs");
include!("interactions.rs");
include!("wire.rs");
#[cfg(test)]
mod tests;
