// CMaNGOS reference: src/game/WorldSocket.cpp and WorldSession opcode dispatch.

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
                character_name: None,
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
        account_security: account.gmlevel,
        gm_mode: false,
        db_creature_navigation: DbCreatureNavigationGuardrail {
            world_data_files: runtime_state.world_data_files.clone(),
            ..DbCreatureNavigationGuardrail::default()
        },
        ..WorldSessionState::default()
    };
    load_global_account_data_into_session(&character_db_pool, account.id, &mut session).await?;
    let world_tick_interval = runtime_state.world_tick_interval;
    let mut next_world_tick_at = Instant::now() + world_tick_interval;

    let session_result: anyhow::Result<()> = async {
        loop {
            let loop_timeout = session_loop_timeout_duration(
                runtime_state.maps.as_ref(),
                &session,
                next_world_tick_at,
                Instant::now(),
            )
            .await;
            match timeout(
                loop_timeout,
                read_client_packet(&mut read_stream, Some(&mut read_header_crypto)),
            )
            .await
            {
                Ok(Ok((opcode, body))) => {
                    let map_player_died =
                        refresh_active_player_session_cache(&runtime_state.maps, &mut session)
                            .await;
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
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
                                    parties: &runtime_state.parties,
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
                                &runtime_state.playerbots,
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
                        CMSG_ITEM_NAME_QUERY => {
                            handle_item_name_query(
                                &mut stream,
                                &world_db_pool,
                                &body,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_GAMEOBJECT_QUERY => {
                            handle_gameobject_query(
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
                                &runtime_state.object_mgr,
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
                                    character_db_pool: &character_db_pool,
                                    world_db_pool: &world_db_pool,
                                    object_mgr: &runtime_state.object_mgr,
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
                                    parties: &runtime_state.parties,
                                },
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_GROUP_INVITE => {
                            handle_group_invite(
                                &mut stream,
                                &runtime_state.parties,
                                &runtime_state.sessions,
                                &body,
                                &session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_GROUP_CANCEL => {
                            handle_group_decline(
                                &runtime_state.parties,
                                &runtime_state.sessions,
                                &session,
                            )
                            .await?;
                        }
                        CMSG_GROUP_ACCEPT => {
                            handle_group_accept(
                                &mut stream,
                                &runtime_state.parties,
                                &runtime_state.sessions,
                                &session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_GROUP_DECLINE => {
                            handle_group_decline(
                                &runtime_state.parties,
                                &runtime_state.sessions,
                                &session,
                            )
                            .await?;
                        }
                        CMSG_GROUP_UNINVITE => {
                            handle_group_uninvite(
                                &mut stream,
                                &runtime_state.parties,
                                &runtime_state.sessions,
                                &body,
                                &session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_GROUP_UNINVITE_GUID => {
                            handle_group_uninvite_guid(
                                &mut stream,
                                &runtime_state.parties,
                                &runtime_state.sessions,
                                &body,
                                &session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_GROUP_SET_LEADER => {
                            handle_group_set_leader(
                                &runtime_state.parties,
                                &runtime_state.sessions,
                                &body,
                                &session,
                            )
                            .await?;
                        }
                        CMSG_GROUP_RAID_CONVERT => {
                            handle_group_raid_convert(
                                &mut stream,
                                &runtime_state.parties,
                                &runtime_state.sessions,
                                &session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_GROUP_CHANGE_SUB_GROUP => {
                            handle_group_change_subgroup(
                                &runtime_state.parties,
                                &runtime_state.sessions,
                                &body,
                                &session,
                            )
                            .await?;
                        }
                        CMSG_GROUP_ASSISTANT_LEADER => {
                            handle_group_assistant_leader(
                                &runtime_state.parties,
                                &runtime_state.sessions,
                                &body,
                                &session,
                            )
                            .await?;
                        }
                        CMSG_REQUEST_PARTY_MEMBER_STATS => {
                            handle_request_party_member_stats(
                                &mut stream,
                                &runtime_state.maps,
                                &body,
                                &session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_LOOT_METHOD => {
                            handle_loot_method(
                                &runtime_state.parties,
                                &runtime_state.sessions,
                                &body,
                                &session,
                            )
                            .await?;
                        }
                        CMSG_LOOT_ROLL => {
                            handle_loot_roll(
                                &mut stream,
                                LootMutationDeps {
                                    character_db_pool: &character_db_pool,
                                    world_db_pool: &world_db_pool,
                                    shared_world: SharedWorldDeps {
                                        object_mgr: runtime_state.object_mgr.as_ref(),
                                        maps: &runtime_state.maps,
                                        sessions: &runtime_state.sessions,
                                    },
                                    parties: runtime_state.parties.as_ref(),
                                },
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_LOOT_MASTER_GIVE => {
                            handle_loot_master_give(
                                &mut stream,
                                LootMutationDeps {
                                    character_db_pool: &character_db_pool,
                                    world_db_pool: &world_db_pool,
                                    shared_world: SharedWorldDeps {
                                        object_mgr: runtime_state.object_mgr.as_ref(),
                                        maps: &runtime_state.maps,
                                        sessions: &runtime_state.sessions,
                                    },
                                    parties: runtime_state.parties.as_ref(),
                                },
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_GROUP_DISBAND => {
                            handle_group_disband(
                                &mut stream,
                                &runtime_state.parties,
                                &runtime_state.sessions,
                                &session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_JOIN_CHANNEL => {
                            handle_join_channel(&mut stream, &body, &mut header_crypto).await?;
                        }
                        CMSG_QUERY_TIME => {
                            handle_query_time(&mut stream, &mut header_crypto).await?;
                        }
                        CMSG_REQUEST_ACCOUNT_DATA => {
                            handle_request_account_data(
                                &mut stream,
                                &body,
                                &session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_UPDATE_ACCOUNT_DATA => {
                            handle_update_account_data(
                                &character_db_pool,
                                account.id,
                                &body,
                                &mut session,
                            )
                            .await?;
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
                        CMSG_STANDSTATECHANGE => {
                            handle_stand_state_change(
                                &mut stream,
                                SharedWorldDeps {
                                    object_mgr: runtime_state.object_mgr.as_ref(),
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
                                },
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_TEXT_EMOTE => {
                            handle_text_emote(
                                &mut stream,
                                TextEmoteDeps {
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
                                },
                                &body,
                                &session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_CAST_SPELL => {
                            handle_cast_spell(
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
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_USE_ITEM => {
                            handle_use_item(
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
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_AUTOEQUIP_ITEM
                        | CMSG_AUTOSTORE_BAG_ITEM
                        | CMSG_SWAP_ITEM
                        | CMSG_SWAP_INV_ITEM => {
                            handle_inventory_swap(
                                &mut stream,
                                InventoryDeps {
                                    character_db_pool: &character_db_pool,
                                    world_db_pool: &world_db_pool,
                                    shared_world: SharedWorldDeps {
                                        object_mgr: runtime_state.object_mgr.as_ref(),
                                        maps: &runtime_state.maps,
                                        sessions: &runtime_state.sessions,
                                    },
                                },
                                opcode,
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_SET_ACTION_BUTTON => {
                            handle_set_action_button(&character_db_pool, &body, &session).await?;
                        }
                        CMSG_SET_SELECTION => {
                            handle_set_selection(
                                SharedWorldDeps {
                                    object_mgr: runtime_state.object_mgr.as_ref(),
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
                                },
                                &body,
                                &mut session,
                            )
                            .await?;
                        }
                        CMSG_SET_TARGET_OBSOLETE => {
                            handle_set_target_obsolete(
                                SharedWorldDeps {
                                    object_mgr: runtime_state.object_mgr.as_ref(),
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
                                },
                                &body,
                                &session,
                            )
                            .await?;
                        }
                        CMSG_DESTROYITEM => {
                            handle_destroy_item(
                                &mut stream,
                                QuestMutationDeps {
                                    character_db_pool: &character_db_pool,
                                    object_mgr: runtime_state.object_mgr.as_ref(),
                                    world_db_pool: &world_db_pool,
                                    shared_world: SharedWorldDeps {
                                        object_mgr: runtime_state.object_mgr.as_ref(),
                                        maps: &runtime_state.maps,
                                        sessions: &runtime_state.sessions,
                                    },
                                },
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
                            if !cancel_pending_player_spell_cast(
                                &mut stream,
                                runtime_state.maps.as_ref(),
                                runtime_state.sessions.as_ref(),
                                &mut session,
                                SPELL_FAILED_INTERRUPTED,
                                &mut header_crypto,
                            )
                            .await?
                            {
                                debug!(
                                    opcode = expected_noop_opcode_name(opcode),
                                    "Ignoring spell cancel opcode with no pending spell cast"
                                );
                            }
                        }
                        CMSG_GOSSIP_HELLO => {
                            handle_gossip_hello(
                                &mut stream,
                                &runtime_state.object_mgr,
                                &world_db_pool,
                                &runtime_state.maps,
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
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
                                    account_id: account.id,
                                },
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_GAMEOBJ_USE => {
                            handle_gameobject_use(
                                &mut stream,
                                GameObjectUseDeps {
                                    character_db_pool: &character_db_pool,
                                    world_db_pool: &world_db_pool,
                                    object_mgr: runtime_state.object_mgr.as_ref(),
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
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
                                &runtime_state.object_mgr,
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
                                &runtime_state.object_mgr,
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
                                &runtime_state.object_mgr,
                                &world_db_pool,
                                &body,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_QUESTGIVER_ACCEPT_QUEST => {
                            handle_questgiver_accept_quest(
                                &mut stream,
                                QuestMutationDeps {
                                    character_db_pool: &character_db_pool,
                                    object_mgr: runtime_state.object_mgr.as_ref(),
                                    world_db_pool: &world_db_pool,
                                    shared_world: SharedWorldDeps {
                                        object_mgr: runtime_state.object_mgr.as_ref(),
                                        maps: &runtime_state.maps,
                                        sessions: &runtime_state.sessions,
                                    },
                                },
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_QUESTGIVER_COMPLETE_QUEST | CMSG_QUESTGIVER_REQUEST_REWARD => {
                            handle_questgiver_complete_quest(
                                &mut stream,
                                &character_db_pool,
                                &runtime_state.object_mgr,
                                &world_db_pool,
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_QUESTGIVER_CHOOSE_REWARD => {
                            handle_questgiver_choose_reward(
                                &mut stream,
                                QuestMutationDeps {
                                    character_db_pool: &character_db_pool,
                                    object_mgr: runtime_state.object_mgr.as_ref(),
                                    world_db_pool: &world_db_pool,
                                    shared_world: SharedWorldDeps {
                                        object_mgr: runtime_state.object_mgr.as_ref(),
                                        maps: &runtime_state.maps,
                                        sessions: &runtime_state.sessions,
                                    },
                                },
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_QUESTLOG_REMOVE_QUEST => {
                            handle_questlog_remove_quest(
                                &mut stream,
                                &character_db_pool,
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
                                QuestMutationDeps {
                                    character_db_pool: &character_db_pool,
                                    object_mgr: runtime_state.object_mgr.as_ref(),
                                    world_db_pool: &world_db_pool,
                                    shared_world: SharedWorldDeps {
                                        object_mgr: runtime_state.object_mgr.as_ref(),
                                        maps: &runtime_state.maps,
                                        sessions: &runtime_state.sessions,
                                    },
                                },
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_BUYBACK_ITEM => {
                            handle_buyback_item(
                                &mut stream,
                                QuestMutationDeps {
                                    character_db_pool: &character_db_pool,
                                    object_mgr: runtime_state.object_mgr.as_ref(),
                                    world_db_pool: &world_db_pool,
                                    shared_world: SharedWorldDeps {
                                        object_mgr: runtime_state.object_mgr.as_ref(),
                                        maps: &runtime_state.maps,
                                        sessions: &runtime_state.sessions,
                                    },
                                },
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
                                SharedWorldDeps {
                                    object_mgr: runtime_state.object_mgr.as_ref(),
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
                                },
                                &runtime_state.parties,
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_ATTACKSTOP => {
                            handle_attack_stop(
                                &mut stream,
                                SharedWorldDeps {
                                    object_mgr: runtime_state.object_mgr.as_ref(),
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
                                },
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_REPOP_REQUEST => {
                            handle_repop_request(
                                &mut stream,
                                PlayerDeathDeps {
                                    character_db_pool: &character_db_pool,
                                    world_db_pool: &world_db_pool,
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
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
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
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
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
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
                                    object_mgr: runtime_state.object_mgr.as_ref(),
                                    maps: &runtime_state.maps,
                                    sessions: &runtime_state.sessions,
                                },
                                &runtime_state.parties,
                                &body,
                                &mut session,
                                &mut header_crypto,
                            )
                            .await?;
                        }
                        CMSG_AUTOSTORE_LOOT_ITEM => {
                            handle_autostore_loot_item(
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
                                    object_mgr: runtime_state.object_mgr.as_ref(),
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
                                    object_mgr: runtime_state.object_mgr.as_ref(),
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
                                    account_id: account.id,
                                    character_db_pool: &character_db_pool,
                                    world_db_pool: &world_db_pool,
                                    object_mgr: runtime_state.object_mgr.as_ref(),
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
                            crate::observability::record_world_unknown_opcode(opcode);
                            warn!(
                                opcode = format_args!("0x{opcode:04X}"),
                                "Unhandled authenticated world opcode"
                            );
                        }
                    }
                    if pending_player_spell_cast_is_due(
                        runtime_state.maps.as_ref(),
                        &session,
                        Instant::now(),
                    )
                    .await
                    {
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
                    }
                    sync_active_player_gameplay_state(&runtime_state.maps, &session).await;
                    if Instant::now() >= next_world_tick_at {
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
                                account_id: account.id,
                                session_id,
                            },
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
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
                        sync_active_player_gameplay_state(&runtime_state.maps, &session).await;
                        advance_world_tick_deadline(
                            &mut next_world_tick_at,
                            Instant::now(),
                            world_tick_interval,
                        );
                    }
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
                    let map_player_died =
                        refresh_active_player_session_cache(&runtime_state.maps, &mut session)
                            .await;
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
                    if pending_player_spell_cast_is_due(
                        runtime_state.maps.as_ref(),
                        &session,
                        Instant::now(),
                    )
                    .await
                    {
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
                    }
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
                            account_id: account.id,
                            session_id,
                        },
                        &mut session,
                        &mut header_crypto,
                    )
                    .await?;
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
                    sync_active_player_gameplay_state(&runtime_state.maps, &session).await;
                    advance_world_tick_deadline(
                        &mut next_world_tick_at,
                        Instant::now(),
                        world_tick_interval,
                    );
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
        if let Err(cleanup_error) =
            persist_session_character_state(
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

async fn session_loop_timeout_duration(
    maps: &MapRuntimeManager,
    session: &WorldSessionState,
    next_world_tick_at: Instant,
    now: Instant,
) -> Duration {
    let world_tick_timeout = world_tick_timeout_duration(next_world_tick_at, now);
    next_pending_player_spell_cast_due_at(maps, session)
        .await
        .map(|due_at| due_at.saturating_duration_since(now).min(world_tick_timeout))
        .unwrap_or(world_tick_timeout)
}

fn advance_world_tick_deadline(
    next_world_tick_at: &mut Instant,
    now: Instant,
    world_tick_interval: Duration,
) {
    debug_assert!(!world_tick_interval.is_zero());
    while *next_world_tick_at <= now {
        *next_world_tick_at += world_tick_interval;
    }
}

async fn refresh_active_player_session_cache(
    maps: &Arc<MapRuntimeManager>,
    session: &mut WorldSessionState,
) -> bool {
    let Some(character) = session.active_character.as_ref() else {
        return false;
    };
    let map_id = character.position.map_id;
    let character_guid = character.guid;
    let Some(snapshot) = maps.player_runtime_snapshot(map_id, character_guid).await else {
        return false;
    };

    let map_player_died = session.player_death_state == PlayerDeathState::Alive
        && snapshot.death_state == PlayerDeathState::Corpse
        && snapshot.health == 0;
    session.player_health = snapshot.health;
    session.player_death_state = snapshot.death_state;
    session.player_mana = snapshot.power1;
    session.player_rage = snapshot.power2;
    session.player_energy = snapshot.power4;
    session.active_spells = snapshot.active_spells;
    session.inventory = snapshot.inventory;
    session.quest_statuses = snapshot.quest_statuses;
    session.player_flags = snapshot.flags;
    if let Some(character) = session.active_character.as_mut() {
        character.position = snapshot.position;
        character.level = snapshot.level;
        character.xp = snapshot.xp;
    }
    map_player_died
}

async fn finalize_map_owned_player_death_if_needed(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    account_id: u32,
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
    map_player_died: bool,
) -> anyhow::Result<bool> {
    if !map_player_died || session.player_health != 0 {
        return Ok(false);
    }
    let Some(character) = session.active_character.as_ref() else {
        return Ok(false);
    };
    let map_id = character.position.map_id;
    let character_guid = character.guid;
    let player = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let death_time = Instant::now();
    session.player_death_state = PlayerDeathState::Corpse;
    session.player_corpse = None;
    session.player_health = 0;
    session.player_in_combat = false;
    mirror_session_player_auto_attack(session, None, None);
    clear_session_active_creature_combats(session);
    shared_world
        .maps
        .set_player_auto_attack(map_id, character_guid, None, None)
        .await;
    persist_player_death_state(character_db_pool, account_id, session).await?;
    send_db_creature_victim_death_evades(
        stream,
        shared_world,
        map_id,
        session,
        player,
        death_time,
        header_crypto,
    )
    .await?;
    Ok(true)
}

async fn sync_active_player_gameplay_state(
    maps: &Arc<MapRuntimeManager>,
    session: &WorldSessionState,
) {
    let Some(character) = session.active_character.as_ref() else {
        return;
    };
    maps.sync_player_gameplay_state(character.position.map_id, character.guid, session)
        .await;
}
