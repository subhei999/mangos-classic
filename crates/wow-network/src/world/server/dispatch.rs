use super::*;

pub(in crate::world) struct WorldPacketDispatchContext<'a> {
    pub(in crate::world) stream: &'a mut WorldPacketSink,
    pub(in crate::world) login_db_pool: &'a MySqlPool,
    pub(in crate::world) character_db_pool: &'a MySqlPool,
    pub(in crate::world) world_db_pool: &'a MySqlPool,
    pub(in crate::world) runtime_state: &'a WorldRuntimeState,
    pub(in crate::world) session_id: SessionId,
    pub(in crate::world) account_id: u32,
    pub(in crate::world) account_name: &'a str,
    pub(in crate::world) session: &'a mut WorldSessionState,
    pub(in crate::world) header_crypto: &'a mut HeaderCrypto,
}

pub(in crate::world) async fn dispatch_world_packet(
    ctx: &mut WorldPacketDispatchContext<'_>,
    packet: &packets::ParsedWorldClientPacket,
    body: &[u8],
) -> anyhow::Result<()> {
    match packet {
        packets::ParsedWorldClientPacket::CharCreate(_)
        | packets::ParsedWorldClientPacket::CharEnum(_)
        | packets::ParsedWorldClientPacket::CharDelete(_)
        | packets::ParsedWorldClientPacket::PlayerLogin(_) => {
            handlers::character::dispatch_character_packet(ctx, packet).await
        }
        packets::ParsedWorldClientPacket::Ping(_)
        | packets::ParsedWorldClientPacket::NameQuery(_)
        | packets::ParsedWorldClientPacket::MessageChat(_)
        | packets::ParsedWorldClientPacket::JoinChannel(_)
        | packets::ParsedWorldClientPacket::TextEmote(_) => {
            handlers::chat::dispatch_chat_packet(ctx, packet).await
        }
        packets::ParsedWorldClientPacket::GroupInvite(_)
        | packets::ParsedWorldClientPacket::GroupCancel(_)
        | packets::ParsedWorldClientPacket::GroupAccept(_)
        | packets::ParsedWorldClientPacket::GroupDecline(_)
        | packets::ParsedWorldClientPacket::GroupUninvite(_)
        | packets::ParsedWorldClientPacket::GroupUninviteGuid(_)
        | packets::ParsedWorldClientPacket::GroupSetLeader(_)
        | packets::ParsedWorldClientPacket::GroupRaidConvert(_)
        | packets::ParsedWorldClientPacket::GroupChangeSubGroup(_)
        | packets::ParsedWorldClientPacket::GroupAssistantLeader(_)
        | packets::ParsedWorldClientPacket::RequestPartyMemberStats(_)
        | packets::ParsedWorldClientPacket::GroupDisband(_)
        | packets::ParsedWorldClientPacket::LootMethod(_) => {
            handlers::social::dispatch_social_packet(ctx, packet).await
        }
        packets::ParsedWorldClientPacket::CastSpell(_)
        | packets::ParsedWorldClientPacket::UseItem(_)
        | packets::ParsedWorldClientPacket::CancelCast(_)
        | packets::ParsedWorldClientPacket::CancelAutoRepeatSpell(_)
        | packets::ParsedWorldClientPacket::AttackSwing(_)
        | packets::ParsedWorldClientPacket::AttackStop(_) => {
            handlers::combat::dispatch_combat_packet(ctx, packet).await
        }
        packets::ParsedWorldClientPacket::InventoryMove(_)
        | packets::ParsedWorldClientPacket::DestroyItem(_)
        | packets::ParsedWorldClientPacket::ReadItem(_)
        | packets::ParsedWorldClientPacket::SetAmmo(_)
        | packets::ParsedWorldClientPacket::SplitItem(_)
        | packets::ParsedWorldClientPacket::AutoBankItem(_)
        | packets::ParsedWorldClientPacket::AutoStoreBankItem(_) => {
            handlers::inventory::dispatch_inventory_packet(ctx, packet).await
        }
        packets::ParsedWorldClientPacket::BankerActivate(_)
        | packets::ParsedWorldClientPacket::BuyBankSlot(_) => {
            handlers::bank::dispatch_bank_packet(ctx, packet).await
        }
        packets::ParsedWorldClientPacket::QuestQuery(_)
        | packets::ParsedWorldClientPacket::QuestgiverStatusQuery(_)
        | packets::ParsedWorldClientPacket::QuestgiverHello(_)
        | packets::ParsedWorldClientPacket::QuestgiverQueryQuest(_)
        | packets::ParsedWorldClientPacket::QuestgiverAcceptQuest(_)
        | packets::ParsedWorldClientPacket::QuestgiverCompleteQuest(_)
        | packets::ParsedWorldClientPacket::QuestgiverRequestReward(_)
        | packets::ParsedWorldClientPacket::QuestReward(_)
        | packets::ParsedWorldClientPacket::QuestgiverCancel(_)
        | packets::ParsedWorldClientPacket::QuestLogRemoveQuest(_) => {
            handlers::quest::dispatch_quest_packet(ctx, packet).await
        }
        packets::ParsedWorldClientPacket::GameObjectQuery(_)
        | packets::ParsedWorldClientPacket::GameObjectUse(_)
        | packets::ParsedWorldClientPacket::CreatureQuery(_)
        | packets::ParsedWorldClientPacket::GossipHello(_)
        | packets::ParsedWorldClientPacket::GossipSelectOption(_)
        | packets::ParsedWorldClientPacket::NpcTextQuery(_)
        | packets::ParsedWorldClientPacket::ListInventory(_)
        | packets::ParsedWorldClientPacket::SellItem(_)
        | packets::ParsedWorldClientPacket::BuybackItem(_)
        | packets::ParsedWorldClientPacket::BuyItem(_)
        | packets::ParsedWorldClientPacket::BuyItemInSlot(_)
        | packets::ParsedWorldClientPacket::TrainerList(_)
        | packets::ParsedWorldClientPacket::TrainerBuySpell(_)
        | packets::ParsedWorldClientPacket::AuctionHello(_)
        | packets::ParsedWorldClientPacket::AuctionSellItem(_)
        | packets::ParsedWorldClientPacket::AuctionRemoveItem(_)
        | packets::ParsedWorldClientPacket::AuctionPlaceBid(_)
        | packets::ParsedWorldClientPacket::AuctionListItems(_)
        | packets::ParsedWorldClientPacket::AuctionListOwnerItems(_)
        | packets::ParsedWorldClientPacket::AuctionListBidderItems(_) => {
            handlers::npc::dispatch_npc_packet(ctx, packet).await
        }
        packets::ParsedWorldClientPacket::Loot(_)
        | packets::ParsedWorldClientPacket::AutostoreLootItem(_)
        | packets::ParsedWorldClientPacket::LootMoney(_)
        | packets::ParsedWorldClientPacket::LootRelease(_)
        | packets::ParsedWorldClientPacket::LootRoll(_)
        | packets::ParsedWorldClientPacket::LootMasterGive(_) => {
            handlers::loot::dispatch_loot_packet(ctx, packet).await
        }
        packets::ParsedWorldClientPacket::Repop(_)
        | packets::ParsedWorldClientPacket::ReclaimCorpse(_)
        | packets::ParsedWorldClientPacket::SpiritHealerActivate(_)
        | packets::ParsedWorldClientPacket::CorpseQuery(_) => {
            handlers::death::dispatch_death_packet(ctx, packet).await
        }
        packets::ParsedWorldClientPacket::SetActiveMover(_)
        | packets::ParsedWorldClientPacket::MoveTeleportAck(_) => {
            handlers::movement::dispatch_movement_packet(ctx, packet, body).await
        }
        packets::ParsedWorldClientPacket::QueryTime(_)
        | packets::ParsedWorldClientPacket::RequestAccountData(_)
        | packets::ParsedWorldClientPacket::UpdateAccountData(_)
        | packets::ParsedWorldClientPacket::TutorialFlag(_)
        | packets::ParsedWorldClientPacket::TutorialClear(_)
        | packets::ParsedWorldClientPacket::TutorialReset(_)
        | packets::ParsedWorldClientPacket::StandStateChange(_)
        | packets::ParsedWorldClientPacket::SetActionButton(_)
        | packets::ParsedWorldClientPacket::SetSelection(_)
        | packets::ParsedWorldClientPacket::SetTargetObsolete(_)
        | packets::ParsedWorldClientPacket::GmTicketGetTicket(_)
        | packets::ParsedWorldClientPacket::QueryNextMailTime(_)
        | packets::ParsedWorldClientPacket::AreaTrigger(_)
        | packets::ParsedWorldClientPacket::ZoneUpdate(_)
        | packets::ParsedWorldClientPacket::LogoutRequest(_)
        | packets::ParsedWorldClientPacket::LogoutCancel(_)
        | packets::ParsedWorldClientPacket::PlayerLogout(_) => {
            handlers::misc::dispatch_misc_packet(ctx, packet).await
        }
        packets::ParsedWorldClientPacket::SendMail(_)
        | packets::ParsedWorldClientPacket::GetMailList(_)
        | packets::ParsedWorldClientPacket::MailTakeMoney(_)
        | packets::ParsedWorldClientPacket::MailTakeItem(_)
        | packets::ParsedWorldClientPacket::MailMarkAsRead(_)
        | packets::ParsedWorldClientPacket::MailReturnToSender(_)
        | packets::ParsedWorldClientPacket::MailDelete(_)
        | packets::ParsedWorldClientPacket::MailCreateTextItem(_)
        | packets::ParsedWorldClientPacket::ItemTextQuery(_) => {
            handlers::mail::dispatch_mail_packet(ctx, packet).await
        }
        packets::ParsedWorldClientPacket::ItemQuerySingle(_)
        | packets::ParsedWorldClientPacket::ItemNameQuery(_)
        | packets::ParsedWorldClientPacket::PageTextQuery(_) => {
            handlers::misc::dispatch_item_query_packet(ctx, packet).await
        }
        packets::ParsedWorldClientPacket::AuthSession(_) => {
            anyhow::bail!("unexpected CMSG_AUTH_SESSION after world authentication")
        }
        packets::ParsedWorldClientPacket::Untyped { opcode } if is_movement_opcode(*opcode) => {
            handlers::movement::dispatch_movement_packet(ctx, packet, body).await
        }
        packets::ParsedWorldClientPacket::Untyped { opcode }
            if is_expected_noop_opcode(*opcode) =>
        {
            info!(
                opcode = expected_noop_opcode_name(*opcode),
                bytes = body.len(),
                "Ignoring expected world bootstrap opcode"
            );
            Ok(())
        }
        packets::ParsedWorldClientPacket::Untyped { opcode } => {
            crate::observability::record_world_unknown_opcode(*opcode);
            warn!(
                opcode = format_args!("0x{opcode:04X}"),
                "Unhandled authenticated world opcode"
            );
            Ok(())
        }
    }
}
