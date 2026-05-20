use super::*;

#[derive(Clone, Copy)]
pub(in crate::world) struct GossipSelectDeps<'a> {
    pub(in crate::world) character_db_pool: &'a MySqlPool,
    pub(in crate::world) world_db_pool: &'a MySqlPool,
    pub(in crate::world) object_mgr: &'a ObjectMgr,
    pub(in crate::world) maps: &'a Arc<MapRuntimeManager>,
    pub(in crate::world) sessions: &'a Arc<SessionRegistry>,
    pub(in crate::world) account_id: u32,
}

#[derive(Debug, Clone)]
pub(in crate::world) struct PreparedGossipMenu {
    pub(in crate::world) menu_id: u32,
    pub(in crate::world) text_id: u32,
    pub(in crate::world) options: Vec<GossipMessageOption>,
    pub(in crate::world) option_actions: Vec<GossipSessionOption>,
    pub(in crate::world) quests: Vec<QuestListItem>,
    pub(in crate::world) npc_flags: u32,
}

pub(in crate::world) async fn handle_gossip_hello(
    stream: &mut WorldPacketSink,
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    maps: &Arc<MapRuntimeManager>,
    request: wow_proto::GossipHelloRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let guid = ObjectGuid::from_raw(request.raw_guid);
    session.gossip = GossipSessionState::default();

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
        session.gossip.active_guid = Some(guid);
        session.gossip.active_options = vec![GossipSessionOption {
            option_id: GOSSIP_OPTION_GOSSIP,
            action_menu_id: -1,
            action_poi_id: 0,
            action_script_id: 0,
        }];
        return send_packet(stream, SMSG_GOSSIP_MESSAGE, &response, Some(header_crypto)).await;
    }

    if guid.is_creature() {
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

        if let Some(prepared) =
            prepare_db_creature_gossip_menu(object_mgr, world_db_pool, maps, guid, None, session)
                .await?
        {
            return send_prepared_gossip_menu(
                stream,
                world_db_pool,
                guid,
                prepared,
                session,
                header_crypto,
            )
            .await;
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
            session.gossip = GossipSessionState::default();
            return send_packet(stream, SMSG_GOSSIP_COMPLETE, &[], Some(header_crypto)).await;
        }
        warn!(
            option = selection.option,
            "Ignoring unsupported Rust Guide gossip option"
        );
        return Ok(());
    }

    if selection.guid.is_creature() {
        let Some(action) = session
            .gossip
            .active_guid
            .filter(|active_guid| *active_guid == selection.guid)
            .and_then(|_| {
                session
                    .gossip
                    .active_options
                    .get(selection.option as usize)
                    .copied()
            })
        else {
            warn!(
                guid = format_args!("0x{:016X}", selection.guid.raw()),
                option = selection.option,
                "Ignoring gossip select without matching prepared DB gossip menu"
            );
            return Ok(());
        };

        info!(
            guid = format_args!("0x{:016X}", selection.guid.raw()),
            entry = selection.guid.entry(),
            option = selection.option,
            option_id = action.option_id,
            action_menu_id = action.action_menu_id,
            "Dispatching DB gossip selection"
        );
        return dispatch_db_gossip_selection(
            stream,
            deps,
            selection.guid,
            action,
            session,
            header_crypto,
        )
        .await;
    }

    warn!(
        guid = format_args!("0x{:016X}", selection.guid.raw()),
        option = selection.option,
        "Ignoring gossip select for unknown creature"
    );
    Ok(())
}

async fn dispatch_db_gossip_selection(
    stream: &mut WorldPacketSink,
    deps: GossipSelectDeps<'_>,
    guid: ObjectGuid,
    action: GossipSessionOption,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    match action.option_id {
        GOSSIP_OPTION_GOSSIP => {
            if action.action_poi_id != 0 {
                debug!(
                    guid = format_args!("0x{:016X}", guid.raw()),
                    action_poi_id = action.action_poi_id,
                    "Gossip POI action is not implemented yet"
                );
            }
            if action.action_menu_id > 0 {
                if let Some(prepared) = prepare_db_creature_gossip_menu(
                    deps.object_mgr,
                    deps.world_db_pool,
                    deps.maps,
                    guid,
                    Some(action.action_menu_id as u32),
                    session,
                )
                .await?
                {
                    return send_prepared_gossip_menu(
                        stream,
                        deps.world_db_pool,
                        guid,
                        prepared,
                        session,
                        header_crypto,
                    )
                    .await;
                }
            } else if action.action_menu_id == 0 {
                return Ok(());
            }
            session.gossip = GossipSessionState::default();
            send_packet(stream, SMSG_GOSSIP_COMPLETE, &[], Some(header_crypto)).await
        }
        GOSSIP_OPTION_VENDOR | GOSSIP_OPTION_ARMORER => {
            let vendor_items = wow_db::get_vendor_items(deps.world_db_pool, guid.entry()).await?;
            let list_items: Vec<VendorListItem> = vendor_items.iter().map(Into::into).collect();
            let response = build_vendor_inventory_body(guid, &list_items);
            send_packet(stream, SMSG_LIST_INVENTORY, &response, Some(header_crypto)).await
        }
        GOSSIP_OPTION_TRAINER => {
            send_trainer_list(
                stream,
                deps.character_db_pool,
                deps.world_db_pool,
                guid,
                session,
                header_crypto,
            )
            .await
        }
        GOSSIP_OPTION_SPIRITHEALER => {
            send_packet(stream, SMSG_GOSSIP_COMPLETE, &[], Some(&mut *header_crypto)).await?;
            handle_spirit_healer_activate(
                stream,
                PlayerDeathDeps {
                    character_db_pool: deps.character_db_pool,
                    world_db_pool: deps.world_db_pool,
                    maps: deps.maps,
                    sessions: deps.sessions,
                    account_id: deps.account_id,
                },
                wow_proto::SpiritHealerActivateRequest {
                    raw_guid: guid.raw(),
                },
                session,
                header_crypto,
            )
            .await
        }
        unsupported => {
            warn!(
                guid = format_args!("0x{:016X}", guid.raw()),
                option_id = unsupported,
                "Closing unsupported DB gossip service option"
            );
            session.gossip = GossipSessionState::default();
            send_packet(stream, SMSG_GOSSIP_COMPLETE, &[], Some(header_crypto)).await
        }
    }?;

    if action.action_script_id != 0 {
        debug!(
            guid = format_args!("0x{:016X}", guid.raw()),
            action_script_id = action.action_script_id,
            "Gossip action script is not implemented yet"
        );
    }

    Ok(())
}

async fn send_prepared_gossip_menu(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    guid: ObjectGuid,
    prepared: PreparedGossipMenu,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if prepared.options.is_empty()
        && !prepared.quests.is_empty()
        && prepared.npc_flags & UNIT_NPC_FLAG_GOSSIP == 0
    {
        session.gossip = GossipSessionState::default();
        let response = build_questgiver_quest_list_body(guid, &prepared.quests);
        return send_packet(
            stream,
            SMSG_QUESTGIVER_QUEST_LIST,
            &response,
            Some(header_crypto),
        )
        .await;
    }

    if prepared.text_id != DEFAULT_GOSSIP_MESSAGE {
        if let Some(text) = db_npc_text_primary(world_db_pool, prepared.text_id).await? {
            let text_update = build_npc_text_update(prepared.text_id, text.as_str());
            send_packet(
                stream,
                SMSG_NPC_TEXT_UPDATE,
                &text_update,
                Some(&mut *header_crypto),
            )
            .await?;
        }
    }

    session.gossip.active_guid = Some(guid);
    session.gossip.active_menu_id = prepared.menu_id;
    session.gossip.active_options = prepared.option_actions.clone();
    let response = build_gossip_message_from_options_with_quests(
        guid,
        prepared.text_id,
        &prepared.options,
        &prepared.quests,
    );
    send_packet(stream, SMSG_GOSSIP_MESSAGE, &response, Some(header_crypto)).await
}

async fn prepare_db_creature_gossip_menu(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    maps: &Arc<MapRuntimeManager>,
    guid: ObjectGuid,
    menu_id_override: Option<u32>,
    session: &WorldSessionState,
) -> anyhow::Result<Option<PreparedGossipMenu>> {
    let Some(template) = wow_db::get_creature_template_query(world_db_pool, guid.entry()).await?
    else {
        return Ok(None);
    };
    let default_menu_id = wow_db::get_creature_gossip_menu_id(world_db_pool, guid.entry())
        .await?
        .unwrap_or(0);
    let menu_id = menu_id_override.unwrap_or(default_menu_id);
    let mut can_see_quests = menu_id == default_menu_id || menu_id_override.is_none();
    let mut option_rows = wow_db::get_gossip_menu_option_queries(world_db_pool, menu_id).await?;
    if option_rows.is_empty() && can_see_quests {
        option_rows = wow_db::get_gossip_menu_option_queries(world_db_pool, 0).await?;
    }

    let service_state =
        GossipServiceState::load(world_db_pool, maps, guid, &template, session).await?;
    let mut options = Vec::new();
    let mut option_actions = Vec::new();
    for row in option_rows {
        match gossip_option_visibility(object_mgr, world_db_pool, &row, &service_state, session)
            .await?
        {
            GossipOptionVisibility::Show => {
                let option_index = options.len() as u32;
                options.push(GossipMessageOption {
                    option_index,
                    icon: gossip_icon_or_default(row.option_icon),
                    coded: u8::from(row.box_coded != 0),
                    text: row.option_text.unwrap_or_default(),
                });
                option_actions.push(GossipSessionOption {
                    option_id: row.option_id,
                    action_menu_id: row.action_menu_id,
                    action_poi_id: row.action_poi_id,
                    action_script_id: row.action_script_id,
                });
            }
            GossipOptionVisibility::HideQuestMenu => {
                can_see_quests = false;
            }
            GossipOptionVisibility::Hide => {}
        }
    }

    let quests = if can_see_quests && service_state.npc_flags & UNIT_NPC_FLAG_QUESTGIVER != 0 {
        questgiver_visible_quests(object_mgr, world_db_pool, guid, session).await?
    } else {
        Vec::new()
    };

    Ok(Some(PreparedGossipMenu {
        menu_id,
        text_id: db_gossip_text_id(object_mgr, world_db_pool, menu_id, session).await?,
        options,
        option_actions,
        quests,
        npc_flags: service_state.npc_flags,
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum GossipOptionVisibility {
    Show,
    Hide,
    HideQuestMenu,
}

#[derive(Debug)]
pub(in crate::world) struct GossipServiceState {
    pub(in crate::world) npc_flags: u32,
    pub(in crate::world) has_vendor_items: bool,
    pub(in crate::world) has_trainer_spells: bool,
    pub(in crate::world) is_spirit_healer: bool,
    pub(in crate::world) is_dead: bool,
}

impl GossipServiceState {
    async fn load(
        world_db_pool: &MySqlPool,
        maps: &Arc<MapRuntimeManager>,
        guid: ObjectGuid,
        template: &wow_db::CreatureTemplateQuery,
        session: &WorldSessionState,
    ) -> anyhow::Result<Self> {
        let has_vendor_items = !wow_db::get_vendor_items(world_db_pool, guid.entry())
            .await?
            .is_empty();
        let has_trainer_spells =
            if let Some(character) = session.character.active_character.as_ref() {
                !wow_db::get_trainer_spells(world_db_pool, guid.entry())
                    .await?
                    .is_empty()
                    && trainer_spell_matches_class(template, character.class)
            } else {
                false
            };
        let is_dead = session.death.player_death_state == PlayerDeathState::Ghost;
        let is_spirit_healer = if is_dead {
            if let Some(character) = session.character.active_character.as_ref() {
                maps.db_creature_snapshot(character.position.map_id, guid)
                    .await
                    .is_some_and(|creature| is_spirit_healer_creature(&creature))
                    || template.npc_flags & UNIT_NPC_FLAG_SPIRITHEALER != 0
            } else {
                template.npc_flags & UNIT_NPC_FLAG_SPIRITHEALER != 0
            }
        } else {
            false
        };
        Ok(Self {
            npc_flags: template.npc_flags,
            has_vendor_items,
            has_trainer_spells,
            is_spirit_healer,
            is_dead,
        })
    }
}

pub(in crate::world) async fn gossip_option_visibility(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    row: &wow_db::GossipMenuOptionQuery,
    service_state: &GossipServiceState,
    session: &WorldSessionState,
) -> anyhow::Result<GossipOptionVisibility> {
    if row.condition_id != 0 {
        let context = ConditionEvaluationContext {
            world_db_pool,
            session,
            source: ConditionSource::GossipOption,
        };
        if !object_mgr
            .is_condition_satisfied(row.condition_id, context)
            .await?
        {
            return Ok(if row.option_id == GOSSIP_OPTION_QUESTGIVER {
                GossipOptionVisibility::HideQuestMenu
            } else {
                GossipOptionVisibility::Hide
            });
        }
    }

    if row.npc_option_npcflag != 0 && row.npc_option_npcflag & service_state.npc_flags == 0 {
        return Ok(GossipOptionVisibility::Hide);
    }

    Ok(match row.option_id {
        GOSSIP_OPTION_GOSSIP => GossipOptionVisibility::Show,
        GOSSIP_OPTION_QUESTGIVER => GossipOptionVisibility::Hide,
        GOSSIP_OPTION_VENDOR | GOSSIP_OPTION_ARMORER => {
            if service_state.has_vendor_items {
                GossipOptionVisibility::Show
            } else {
                GossipOptionVisibility::Hide
            }
        }
        GOSSIP_OPTION_TRAINER => {
            if service_state.has_trainer_spells {
                GossipOptionVisibility::Show
            } else {
                GossipOptionVisibility::Hide
            }
        }
        GOSSIP_OPTION_SPIRITHEALER => {
            if service_state.is_spirit_healer && service_state.is_dead {
                GossipOptionVisibility::Show
            } else {
                GossipOptionVisibility::Hide
            }
        }
        GOSSIP_OPTION_TAXIVENDOR
        | GOSSIP_OPTION_SPIRITGUIDE
        | GOSSIP_OPTION_INNKEEPER
        | GOSSIP_OPTION_BANKER
        | GOSSIP_OPTION_PETITIONER
        | GOSSIP_OPTION_TABARDDESIGNER
        | GOSSIP_OPTION_BATTLEFIELD
        | GOSSIP_OPTION_AUCTIONEER
        | GOSSIP_OPTION_STABLEPET
        | GOSSIP_OPTION_UNLEARNTALENTS
        | GOSSIP_OPTION_UNLEARNPETSKILLS => GossipOptionVisibility::Show,
        _ => GossipOptionVisibility::Hide,
    })
}

async fn db_gossip_text_id(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    menu_id: u32,
    session: &WorldSessionState,
) -> anyhow::Result<u32> {
    if menu_id == 0 {
        return Ok(DEFAULT_GOSSIP_MESSAGE);
    }
    let mut text_id = DEFAULT_GOSSIP_MESSAGE;
    let mut last_condition_id = 0;
    for row in wow_db::get_gossip_menu_queries(world_db_pool, menu_id).await? {
        let condition_matches = if row.condition_id == 0 {
            last_condition_id == 0
        } else if row.condition_id > last_condition_id {
            let context = ConditionEvaluationContext {
                world_db_pool,
                session,
                source: ConditionSource::GossipMenu,
            };
            object_mgr
                .is_condition_satisfied(row.condition_id, context)
                .await?
        } else {
            false
        };
        if condition_matches {
            last_condition_id = row.condition_id;
            text_id = row.text_id;
            if row.script_id != 0 {
                debug!(
                    menu_id,
                    script_id = row.script_id,
                    "Gossip menu hello script is not implemented yet"
                );
            }
        }
    }
    Ok(text_id)
}

fn gossip_icon_or_default(icon: u8) -> u8 {
    if icon < GOSSIP_ICON_MAX {
        icon
    } else {
        GOSSIP_ICON_CHAT
    }
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
