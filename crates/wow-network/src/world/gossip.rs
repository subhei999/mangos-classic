#[derive(Clone, Copy)]
struct GossipSelectDeps<'a> {
    character_db_pool: &'a MySqlPool,
    world_db_pool: &'a MySqlPool,
    player_corpses: &'a PlayerCorpses,
    maps: &'a Arc<MapRuntimeManager>,
    account_id: u32,
}

async fn handle_gossip_hello(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    body: &[u8],
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let guid = read_packet_guid(body, "CMSG_GOSSIP_HELLO")?;
    if guid == rust_guide_guid() {
        let text_update =
            build_npc_text_update(RUST_GUIDE_GOSSIP_TEXT_ID, RUST_GUIDE_GOSSIP_TEXT);
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
            &[(0, RUST_GUIDE_GOSSIP_OPTION)],
        );
        return send_packet(stream, SMSG_GOSSIP_MESSAGE, &response, Some(header_crypto)).await;
    }

    if guid.is_creature() {
        if session.player_death_state == PlayerDeathState::Ghost
            && session
                .db_creatures
                .get(&guid.raw())
                .is_some_and(is_spirit_healer_creature)
        {
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
                &[(0, SPIRIT_HEALER_GOSSIP_OPTION)],
            );
            return send_packet(stream, SMSG_GOSSIP_MESSAGE, &response, Some(header_crypto)).await;
        }

        if let Some(quest) = questgiver_completed_turnin_quest(world_db_pool, guid, session).await? {
            let response = build_quest_offer_reward_body(guid, &quest);
            return send_packet(
                stream,
                SMSG_QUESTGIVER_OFFER_REWARD,
                &response,
                Some(header_crypto),
            )
            .await;
        }

        let quests = questgiver_visible_quests(world_db_pool, guid, session).await?;
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

        let vendor_items = wow_db::get_vendor_items(world_db_pool, guid.entry()).await?;
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
                &[(0, DB_VENDOR_GOSSIP_OPTION)],
            );
            return send_packet(stream, SMSG_GOSSIP_MESSAGE, &response, Some(header_crypto)).await;
        }

        let trainer_spells = wow_db::get_trainer_spells(world_db_pool, guid.entry()).await?;
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
                &[(0, DB_TRAINER_GOSSIP_OPTION)],
            );
            return send_packet(stream, SMSG_GOSSIP_MESSAGE, &response, Some(header_crypto)).await;
        }
    }

    warn!(
        guid = format_args!("0x{:016X}", guid.raw()),
        "Ignoring gossip hello for unknown creature"
    );
    Ok(())
}

async fn handle_gossip_select_option(
    stream: &mut WorldPacketSink,
    deps: GossipSelectDeps<'_>,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let selection = GossipSelectOption::read(body)?;
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
        if !selection.is_supported_browse_option() {
            warn!(
                guid = format_args!("0x{:016X}", selection.guid.raw()),
                option = selection.option,
                "Ignoring unsupported DB vendor gossip option"
            );
            return Ok(());
        }
        if session.player_death_state == PlayerDeathState::Ghost
            && session
                .db_creatures
                .get(&selection.guid.raw())
                .is_some_and(is_spirit_healer_creature)
        {
            send_packet(
                stream,
                SMSG_GOSSIP_COMPLETE,
                &[],
                Some(&mut *header_crypto),
            )
            .await?;
            return handle_spirit_healer_activate(
                stream,
                PlayerDeathDeps {
                    character_db_pool: deps.character_db_pool,
                    world_db_pool: deps.world_db_pool,
                    player_corpses: deps.player_corpses,
                    maps: deps.maps,
                    account_id: deps.account_id,
                },
                body,
                session,
                header_crypto,
            )
            .await;
        }

        let vendor_items =
            wow_db::get_vendor_items(deps.world_db_pool, selection.guid.entry()).await?;
        if !vendor_items.is_empty() {
            let list_items: Vec<VendorListItem> = vendor_items.iter().map(Into::into).collect();
            let response = build_vendor_inventory_body(selection.guid, &list_items);
            return send_packet(stream, SMSG_LIST_INVENTORY, &response, Some(header_crypto)).await;
        }

        let trainer_spells =
            wow_db::get_trainer_spells(deps.world_db_pool, selection.guid.entry()).await?;
        if !trainer_spells.is_empty() {
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
struct GossipSelectOption {
    guid: ObjectGuid,
    option: u32,
}

impl GossipSelectOption {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        if body.len() < 12 {
            anyhow::bail!(
                "CMSG_GOSSIP_SELECT_OPTION payload too short: {} bytes",
                body.len()
            );
        }
        Ok(Self {
            guid: ObjectGuid::from_raw(u64::from_le_bytes(body[0..8].try_into()?)),
            option: u32::from_le_bytes(body[8..12].try_into()?),
        })
    }

    fn is_supported_browse_option(&self) -> bool {
        self.option == 0
    }
}

