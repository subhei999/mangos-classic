use super::*;

#[derive(Clone, Copy)]
pub(in crate::world) struct GossipSelectDeps<'a> {
    pub(in crate::world) character_db_pool: &'a MySqlPool,
    pub(in crate::world) world_db_pool: &'a MySqlPool,
    pub(in crate::world) maps: &'a Arc<MapRuntimeManager>,
    pub(in crate::world) sessions: &'a Arc<SessionRegistry>,
    pub(in crate::world) account_id: u32,
}

pub(in crate::world) async fn handle_gossip_hello(
    stream: &mut WorldPacketSink,
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    maps: &Arc<MapRuntimeManager>,
    request: wow_proto::GossipHelloRequest,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let guid = ObjectGuid::from_raw(request.raw_guid);
    if guid == rust_guide_guid() {
        let text_update = build_npc_text_update(RUST_GUIDE_GOSSIP_TEXT_ID, RUST_GUIDE_GOSSIP_TEXT);
        send_packet(
            stream,
            SMSG_NPC_TEXT_UPDATE,
            &text_update,
            Some(&mut *header_crypto),
        )
        .await?;
        let response = build_gossip_message(
            guid,
            RUST_GUIDE_GOSSIP_TEXT_ID,
            &[(0, GOSSIP_ICON_CHAT, RUST_GUIDE_GOSSIP_OPTION)],
        );
        return send_packet(stream, SMSG_GOSSIP_MESSAGE, &response, Some(header_crypto)).await;
    }

    if guid.is_creature() {
        let is_spirit_healer = if session.death.player_death_state == PlayerDeathState::Ghost {
            if let Some(character) = session.character.active_character.as_ref() {
                maps.db_creature_snapshot(character.position.map_id, guid)
                    .await
                    .is_some_and(|creature| is_spirit_healer_creature(&creature))
            } else {
                false
            }
        } else {
            false
        };
        if is_spirit_healer {
            let text_update =
                build_npc_text_update(SPIRIT_HEALER_GOSSIP_TEXT_ID, SPIRIT_HEALER_GOSSIP_TEXT);
            send_packet(
                stream,
                SMSG_NPC_TEXT_UPDATE,
                &text_update,
                Some(&mut *header_crypto),
            )
            .await?;
            let response = build_gossip_message(
                guid,
                SPIRIT_HEALER_GOSSIP_TEXT_ID,
                &[(0, GOSSIP_ICON_INTERACT_1, SPIRIT_HEALER_GOSSIP_OPTION)],
            );
            return send_packet(stream, SMSG_GOSSIP_MESSAGE, &response, Some(header_crypto)).await;
        }

        if let Some(quest) =
            questgiver_completed_turnin_quest(object_mgr, world_db_pool, guid, session).await?
        {
            let displays = quest_reward_item_displays(world_db_pool, &quest).await?;
            let response = build_quest_offer_reward_body(guid, &quest, &displays);
            return send_packet(
                stream,
                SMSG_QUESTGIVER_OFFER_REWARD,
                &response,
                Some(header_crypto),
            )
            .await;
        }

        let quests = questgiver_visible_quests(object_mgr, world_db_pool, guid, session).await?;
        let vendor_items = wow_db::get_vendor_items(world_db_pool, guid.entry()).await?;
        let trainer_spells = wow_db::get_trainer_spells(world_db_pool, guid.entry()).await?;
        let mut options = Vec::new();
        if !vendor_items.is_empty() {
            options.push((
                options.len() as u32,
                GOSSIP_ICON_VENDOR,
                DB_VENDOR_GOSSIP_OPTION,
            ));
        }
        if !trainer_spells.is_empty() {
            options.push((
                options.len() as u32,
                GOSSIP_ICON_TRAINER,
                DB_TRAINER_GOSSIP_OPTION,
            ));
        }
        if !quests.is_empty() && !options.is_empty() {
            let text_id = db_creature_gossip_text_id(world_db_pool, guid.entry())
                .await?
                .unwrap_or(DB_TRAINER_GOSSIP_TEXT_ID);
            let text = db_npc_text_primary(world_db_pool, text_id)
                .await?
                .unwrap_or_else(|| DB_TRAINER_GOSSIP_TEXT.to_string());
            let text_update = build_npc_text_update(text_id, text.as_str());
            send_packet(
                stream,
                SMSG_NPC_TEXT_UPDATE,
                &text_update,
                Some(&mut *header_crypto),
            )
            .await?;
            let response = build_gossip_message_with_quests(guid, text_id, &options, &quests);
            return send_packet(stream, SMSG_GOSSIP_MESSAGE, &response, Some(header_crypto)).await;
        }
        if !quests.is_empty() {
            let response = build_questgiver_quest_list_body(guid, &quests);
            return send_packet(
                stream,
                SMSG_QUESTGIVER_QUEST_LIST,
                &response,
                Some(header_crypto),
            )
            .await;
        }

        if !vendor_items.is_empty() {
            let text_update =
                build_npc_text_update(DB_VENDOR_GOSSIP_TEXT_ID, DB_VENDOR_GOSSIP_TEXT);
            send_packet(
                stream,
                SMSG_NPC_TEXT_UPDATE,
                &text_update,
                Some(&mut *header_crypto),
            )
            .await?;
            let response = build_gossip_message(
                guid,
                DB_VENDOR_GOSSIP_TEXT_ID,
                &[(0, GOSSIP_ICON_VENDOR, DB_VENDOR_GOSSIP_OPTION)],
            );
            return send_packet(stream, SMSG_GOSSIP_MESSAGE, &response, Some(header_crypto)).await;
        }

        if !trainer_spells.is_empty() {
            let text_update =
                build_npc_text_update(DB_TRAINER_GOSSIP_TEXT_ID, DB_TRAINER_GOSSIP_TEXT);
            send_packet(
                stream,
                SMSG_NPC_TEXT_UPDATE,
                &text_update,
                Some(&mut *header_crypto),
            )
            .await?;
            let response = build_gossip_message(
                guid,
                DB_TRAINER_GOSSIP_TEXT_ID,
                &[(0, GOSSIP_ICON_TRAINER, DB_TRAINER_GOSSIP_OPTION)],
            );
            return send_packet(stream, SMSG_GOSSIP_MESSAGE, &response, Some(header_crypto)).await;
        }

        if let Some(text_id) = db_creature_gossip_text_id(world_db_pool, guid.entry()).await? {
            let text = db_npc_text_primary(world_db_pool, text_id)
                .await?
                .unwrap_or_else(|| RUST_GUIDE_GOSSIP_TEXT.to_string());
            let text_update = build_npc_text_update(text_id, text.as_str());
            send_packet(
                stream,
                SMSG_NPC_TEXT_UPDATE,
                &text_update,
                Some(&mut *header_crypto),
            )
            .await?;
            let response = build_gossip_message(guid, text_id, &[]);
            return send_packet(stream, SMSG_GOSSIP_MESSAGE, &response, Some(header_crypto)).await;
        }
    }

    warn!(
        guid = format_args!("0x{:016X}", guid.raw()),
        "Ignoring gossip hello for unknown creature"
    );
    Ok(())
}

pub(in crate::world) async fn handle_gossip_select_option(
    stream: &mut WorldPacketSink,
    deps: GossipSelectDeps<'_>,
    request: wow_proto::GossipSelectOptionRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let selection = GossipSelectOption::from(request);
    if selection.guid == rust_guide_guid() {
        if selection.is_supported_browse_option() {
            return send_packet(stream, SMSG_GOSSIP_COMPLETE, &[], Some(header_crypto)).await;
        }
        warn!(
            option = selection.option,
            "Ignoring unsupported Rust Guide gossip option"
        );
        return Ok(());
    }

    if selection.guid.is_creature() {
        let is_spirit_healer = if session.death.player_death_state == PlayerDeathState::Ghost {
            if let Some(character) = session.character.active_character.as_ref() {
                deps.maps
                    .db_creature_snapshot(character.position.map_id, selection.guid)
                    .await
                    .is_some_and(|creature| is_spirit_healer_creature(&creature))
            } else {
                false
            }
        } else {
            false
        };
        if is_spirit_healer {
            send_packet(stream, SMSG_GOSSIP_COMPLETE, &[], Some(&mut *header_crypto)).await?;
            return handle_spirit_healer_activate(
                stream,
                PlayerDeathDeps {
                    character_db_pool: deps.character_db_pool,
                    world_db_pool: deps.world_db_pool,
                    maps: deps.maps,
                    sessions: deps.sessions,
                    account_id: deps.account_id,
                },
                wow_proto::SpiritHealerActivateRequest {
                    raw_guid: selection.guid.raw(),
                },
                session,
                header_crypto,
            )
            .await;
        }

        let vendor_items =
            wow_db::get_vendor_items(deps.world_db_pool, selection.guid.entry()).await?;
        let trainer_spells =
            wow_db::get_trainer_spells(deps.world_db_pool, selection.guid.entry()).await?;
        let mut next_option = 0;
        if !vendor_items.is_empty() {
            if selection.option != next_option {
                next_option += 1;
            } else {
                let list_items: Vec<VendorListItem> = vendor_items.iter().map(Into::into).collect();
                let response = build_vendor_inventory_body(selection.guid, &list_items);
                return send_packet(stream, SMSG_LIST_INVENTORY, &response, Some(header_crypto))
                    .await;
            }
        }

        if !trainer_spells.is_empty() {
            if selection.option != next_option {
                warn!(
                    guid = format_args!("0x{:016X}", selection.guid.raw()),
                    option = selection.option,
                    "Ignoring unsupported DB gossip service option"
                );
                return Ok(());
            }
            return send_trainer_list(
                stream,
                deps.character_db_pool,
                deps.world_db_pool,
                selection.guid,
                session,
                header_crypto,
            )
            .await;
        }
    }

    warn!(
        guid = format_args!("0x{:016X}", selection.guid.raw()),
        option = selection.option,
        "Ignoring gossip select for unknown creature"
    );
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct GossipSelectOption {
    pub(in crate::world) guid: ObjectGuid,
    pub(in crate::world) option: u32,
}

impl GossipSelectOption {
    pub(in crate::world) fn is_supported_browse_option(&self) -> bool {
        self.option == 0
    }
}

async fn db_creature_gossip_text_id(
    world_db_pool: &MySqlPool,
    creature_entry: u32,
) -> anyhow::Result<Option<u32>> {
    Ok(
        wow_db::get_creature_gossip_menu_query(world_db_pool, creature_entry)
            .await?
            .map(|menu| menu.text_id),
    )
}

async fn db_npc_text_primary(
    world_db_pool: &MySqlPool,
    text_id: u32,
) -> anyhow::Result<Option<String>> {
    let text = wow_db::get_npc_text_query(world_db_pool, text_id)
        .await?
        .map(|row| {
            if row.text0_0.is_empty() {
                row.text0_1
            } else {
                row.text0_0
            }
        })
        .filter(|text| !text.is_empty());
    Ok(text)
}

impl From<wow_proto::GossipSelectOptionRequest> for GossipSelectOption {
    fn from(request: wow_proto::GossipSelectOptionRequest) -> Self {
        Self {
            guid: ObjectGuid::from_raw(request.raw_guid),
            option: request.option,
        }
    }
}
