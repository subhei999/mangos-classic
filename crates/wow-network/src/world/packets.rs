use wow_proto::{
    AttackStopRequest, AttackSwingRequest, AutostoreLootItemRequest, BuyItemRequest,
    BuybackItemRequest, CancelAutoRepeatSpellRequest, CancelCastRequest, CastSpellRequest,
    CharCreateRequest, CharDeleteRequest, CharEnumRequest, CorpseQueryRequest,
    CreatureQueryRequest, DestroyItemRequest, GameObjectQueryRequest, GameObjectUseRequest,
    GetMailListRequest, GmTicketGetTicketRequest, GossipHelloRequest, GossipSelectOptionRequest,
    GroupAcceptRequest, GroupAssistantLeaderRequest, GroupCancelRequest,
    GroupChangeSubGroupRequest, GroupDeclineRequest, GroupDisbandRequest, GroupInviteRequest,
    GroupRaidConvertRequest, GroupSetLeaderRequest, GroupUninviteGuidRequest, GroupUninviteRequest,
    InventoryMoveClientRequest, ItemNameQueryRequest, ItemQuerySingleRequest, ItemTextQueryRequest,
    JoinChannelRequest, ListInventoryRequest, LogoutCancelRequest, LogoutRequest,
    LootMasterGiveRequest, LootMethodRequest, LootMoneyRequest, LootReleaseRequest, LootRequest,
    LootRollRequest, MailCreateTextItemRequest, MailIdRequest, MessageChatRequest,
    MoveTeleportAckRequest, NameQueryRequest, NpcTextQueryRequest, PageTextQueryRequest,
    PingRequest, PlayerLoginRequest, PlayerLogoutRequest, QueryNextMailTimeRequest,
    QueryTimeRequest, QuestLogRemoveQuestRequest, QuestQueryRequest, QuestRewardRequest,
    QuestgiverHelloRequest, QuestgiverQuestRequest, QuestgiverStatusQueryRequest, ReadItemRequest,
    ReclaimCorpseRequest, RepopRequest, RequestAccountDataRequest, RequestPartyMemberStatsRequest,
    SellItemRequest, SendMailRequest, SetActionButtonRequest, SetActiveMoverRequest,
    SetAmmoRequest, SetSelectionRequest, SetTargetObsoleteRequest, SpiritHealerActivateRequest,
    SplitItemRequest, StandStateChangeRequest, TextEmoteRequest, TrainerBuySpellRequest,
    TrainerListRequest, TutorialClearRequest, TutorialFlagRequest, TutorialResetRequest,
    UpdateAccountDataRequest, UseItemRequest, WorldAuthSessionRequest, WorldOpcode,
};

#[derive(Debug, Clone, PartialEq)]
pub(super) enum ParsedWorldClientPacket {
    AuthSession(WorldAuthSessionRequest),
    Ping(PingRequest),
    CharCreate(CharCreateRequest),
    CharEnum(CharEnumRequest),
    CharDelete(CharDeleteRequest),
    PlayerLogin(PlayerLoginRequest),
    NameQuery(NameQueryRequest),
    ItemQuerySingle(ItemQuerySingleRequest),
    ItemNameQuery(ItemNameQueryRequest),
    PageTextQuery(PageTextQueryRequest),
    QuestQuery(QuestQueryRequest),
    GameObjectQuery(GameObjectQueryRequest),
    CreatureQuery(CreatureQueryRequest),
    GroupInvite(GroupInviteRequest),
    GroupCancel(GroupCancelRequest),
    GroupAccept(GroupAcceptRequest),
    GroupDecline(GroupDeclineRequest),
    GroupUninvite(GroupUninviteRequest),
    GroupUninviteGuid(GroupUninviteGuidRequest),
    GroupSetLeader(GroupSetLeaderRequest),
    LootMethod(LootMethodRequest),
    LootRoll(LootRollRequest),
    LootMasterGive(LootMasterGiveRequest),
    GroupDisband(GroupDisbandRequest),
    MessageChat(MessageChatRequest),
    JoinChannel(JoinChannelRequest),
    TextEmote(TextEmoteRequest),
    CastSpell(CastSpellRequest),
    UseItem(UseItemRequest),
    ReadItem(ReadItemRequest),
    InventoryMove(InventoryMoveClientRequest),
    DestroyItem(DestroyItemRequest),
    SplitItem(SplitItemRequest),
    SetAmmo(SetAmmoRequest),
    CancelCast(CancelCastRequest),
    CancelAutoRepeatSpell(CancelAutoRepeatSpellRequest),
    GameObjectUse(GameObjectUseRequest),
    QueryTime(QueryTimeRequest),
    RequestAccountData(RequestAccountDataRequest),
    UpdateAccountData(UpdateAccountDataRequest),
    TutorialFlag(TutorialFlagRequest),
    TutorialClear(TutorialClearRequest),
    TutorialReset(TutorialResetRequest),
    StandStateChange(StandStateChangeRequest),
    SetActionButton(SetActionButtonRequest),
    SetSelection(SetSelectionRequest),
    SetTargetObsolete(SetTargetObsoleteRequest),
    GossipHello(GossipHelloRequest),
    GossipSelectOption(GossipSelectOptionRequest),
    NpcTextQuery(NpcTextQueryRequest),
    QuestgiverStatusQuery(QuestgiverStatusQueryRequest),
    QuestgiverHello(QuestgiverHelloRequest),
    QuestgiverQueryQuest(QuestgiverQuestRequest),
    QuestgiverAcceptQuest(QuestgiverQuestRequest),
    QuestgiverCompleteQuest(QuestgiverQuestRequest),
    QuestgiverRequestReward(QuestgiverQuestRequest),
    QuestReward(QuestRewardRequest),
    QuestLogRemoveQuest(QuestLogRemoveQuestRequest),
    ListInventory(ListInventoryRequest),
    SellItem(SellItemRequest),
    BuybackItem(BuybackItemRequest),
    BuyItem(BuyItemRequest),
    TrainerList(TrainerListRequest),
    TrainerBuySpell(TrainerBuySpellRequest),
    AttackSwing(AttackSwingRequest),
    AttackStop(AttackStopRequest),
    Repop(RepopRequest),
    ReclaimCorpse(ReclaimCorpseRequest),
    SpiritHealerActivate(SpiritHealerActivateRequest),
    CorpseQuery(CorpseQueryRequest),
    Loot(LootRequest),
    AutostoreLootItem(AutostoreLootItemRequest),
    LootMoney(LootMoneyRequest),
    LootRelease(LootReleaseRequest),
    GmTicketGetTicket(GmTicketGetTicketRequest),
    SetActiveMover(SetActiveMoverRequest),
    MoveTeleportAck(MoveTeleportAckRequest),
    GroupChangeSubGroup(GroupChangeSubGroupRequest),
    RequestPartyMemberStats(RequestPartyMemberStatsRequest),
    QueryNextMailTime(QueryNextMailTimeRequest),
    SendMail(SendMailRequest),
    GetMailList(GetMailListRequest),
    MailTakeMoney(MailIdRequest),
    MailTakeItem(MailIdRequest),
    MailMarkAsRead(MailIdRequest),
    MailReturnToSender(MailIdRequest),
    MailDelete(MailIdRequest),
    MailCreateTextItem(MailCreateTextItemRequest),
    ItemTextQuery(ItemTextQueryRequest),
    GroupRaidConvert(GroupRaidConvertRequest),
    GroupAssistantLeader(GroupAssistantLeaderRequest),
    LogoutRequest(LogoutRequest),
    LogoutCancel(LogoutCancelRequest),
    PlayerLogout(PlayerLogoutRequest),
    Untyped { opcode: u32 },
}

macro_rules! packet_accessor {
    ($name:ident, $variant:ident, $ty:ty, $expected:literal) => {
        pub(super) fn $name(&self) -> anyhow::Result<$ty> {
            match self {
                Self::$variant(request) => Ok(request.clone()),
                other => {
                    anyhow::bail!(
                        "expected {}, got opcode 0x{:04X}",
                        $expected,
                        other.opcode()
                    )
                }
            }
        }
    };
}

impl ParsedWorldClientPacket {
    pub(super) fn opcode(&self) -> u32 {
        match self {
            Self::AuthSession(_) => WorldOpcode::CmsgAuthSession.into(),
            Self::Ping(_) => WorldOpcode::CmsgPing.into(),
            Self::CharCreate(_) => WorldOpcode::CmsgCharCreate.into(),
            Self::CharEnum(_) => WorldOpcode::CmsgCharEnum.into(),
            Self::CharDelete(_) => WorldOpcode::CmsgCharDelete.into(),
            Self::PlayerLogin(_) => WorldOpcode::CmsgPlayerLogin.into(),
            Self::NameQuery(_) => WorldOpcode::CmsgNameQuery.into(),
            Self::ItemQuerySingle(_) => WorldOpcode::CmsgItemQuerySingle.into(),
            Self::ItemNameQuery(_) => WorldOpcode::CmsgItemNameQuery.into(),
            Self::PageTextQuery(_) => WorldOpcode::CmsgPageTextQuery.into(),
            Self::QuestQuery(_) => WorldOpcode::CmsgQuestQuery.into(),
            Self::GameObjectQuery(_) => WorldOpcode::CmsgGameObjectQuery.into(),
            Self::CreatureQuery(_) => WorldOpcode::CmsgCreatureQuery.into(),
            Self::GroupInvite(_) => WorldOpcode::CmsgGroupInvite.into(),
            Self::GroupCancel(_) => WorldOpcode::CmsgGroupCancel.into(),
            Self::GroupAccept(_) => WorldOpcode::CmsgGroupAccept.into(),
            Self::GroupDecline(_) => WorldOpcode::CmsgGroupDecline.into(),
            Self::GroupUninvite(_) => WorldOpcode::CmsgGroupUninvite.into(),
            Self::GroupUninviteGuid(_) => WorldOpcode::CmsgGroupUninviteGuid.into(),
            Self::GroupSetLeader(_) => WorldOpcode::CmsgGroupSetLeader.into(),
            Self::LootMethod(_) => WorldOpcode::CmsgLootMethod.into(),
            Self::LootRoll(_) => WorldOpcode::CmsgLootRoll.into(),
            Self::LootMasterGive(_) => WorldOpcode::CmsgLootMasterGive.into(),
            Self::GroupDisband(_) => WorldOpcode::CmsgGroupDisband.into(),
            Self::MessageChat(_) => WorldOpcode::CmsgMessageChat.into(),
            Self::JoinChannel(_) => WorldOpcode::CmsgJoinChannel.into(),
            Self::TextEmote(_) => WorldOpcode::CmsgTextEmote.into(),
            Self::CastSpell(_) => WorldOpcode::CmsgCastSpell.into(),
            Self::UseItem(_) => WorldOpcode::CmsgUseItem.into(),
            Self::ReadItem(_) => WorldOpcode::CmsgReadItem.into(),
            Self::InventoryMove(request) => match request {
                InventoryMoveClientRequest::AutoEquip { .. } => {
                    WorldOpcode::CmsgAutoequipItem.into()
                }
                InventoryMoveClientRequest::AutoStoreBag { .. } => {
                    WorldOpcode::CmsgAutostoreBagItem.into()
                }
                InventoryMoveClientRequest::SwapItem { .. } => WorldOpcode::CmsgSwapItem.into(),
                InventoryMoveClientRequest::SwapInvItem { .. } => {
                    WorldOpcode::CmsgSwapInvItem.into()
                }
            },
            Self::DestroyItem(_) => WorldOpcode::CmsgDestroyItem.into(),
            Self::SplitItem(_) => WorldOpcode::CmsgSplitItem.into(),
            Self::SetAmmo(_) => WorldOpcode::CmsgSetAmmo.into(),
            Self::CancelCast(_) => WorldOpcode::CmsgCancelCast.into(),
            Self::CancelAutoRepeatSpell(_) => WorldOpcode::CmsgCancelAutoRepeatSpell.into(),
            Self::GameObjectUse(_) => WorldOpcode::CmsgGameObjUse.into(),
            Self::QueryTime(_) => WorldOpcode::CmsgQueryTime.into(),
            Self::RequestAccountData(_) => WorldOpcode::CmsgRequestAccountData.into(),
            Self::UpdateAccountData(_) => WorldOpcode::CmsgUpdateAccountData.into(),
            Self::TutorialFlag(_) => WorldOpcode::CmsgTutorialFlag.into(),
            Self::TutorialClear(_) => WorldOpcode::CmsgTutorialClear.into(),
            Self::TutorialReset(_) => WorldOpcode::CmsgTutorialReset.into(),
            Self::StandStateChange(_) => WorldOpcode::CmsgStandStateChange.into(),
            Self::SetActionButton(_) => WorldOpcode::CmsgSetActionButton.into(),
            Self::SetSelection(_) => WorldOpcode::CmsgSetSelection.into(),
            Self::SetTargetObsolete(_) => WorldOpcode::CmsgSetTargetObsolete.into(),
            Self::GossipHello(_) => WorldOpcode::CmsgGossipHello.into(),
            Self::GossipSelectOption(_) => WorldOpcode::CmsgGossipSelectOption.into(),
            Self::NpcTextQuery(_) => WorldOpcode::CmsgNpcTextQuery.into(),
            Self::QuestgiverStatusQuery(_) => WorldOpcode::CmsgQuestgiverStatusQuery.into(),
            Self::QuestgiverHello(_) => WorldOpcode::CmsgQuestgiverHello.into(),
            Self::QuestgiverQueryQuest(_) => WorldOpcode::CmsgQuestgiverQueryQuest.into(),
            Self::QuestgiverAcceptQuest(_) => WorldOpcode::CmsgQuestgiverAcceptQuest.into(),
            Self::QuestgiverCompleteQuest(_) => WorldOpcode::CmsgQuestgiverCompleteQuest.into(),
            Self::QuestgiverRequestReward(_) => WorldOpcode::CmsgQuestgiverRequestReward.into(),
            Self::QuestReward(_) => WorldOpcode::CmsgQuestgiverChooseReward.into(),
            Self::QuestLogRemoveQuest(_) => WorldOpcode::CmsgQuestlogRemoveQuest.into(),
            Self::ListInventory(_) => WorldOpcode::CmsgListInventory.into(),
            Self::SellItem(_) => WorldOpcode::CmsgSellItem.into(),
            Self::BuybackItem(_) => WorldOpcode::CmsgBuybackItem.into(),
            Self::BuyItem(_) => WorldOpcode::CmsgBuyItem.into(),
            Self::TrainerList(_) => WorldOpcode::CmsgTrainerList.into(),
            Self::TrainerBuySpell(_) => WorldOpcode::CmsgTrainerBuySpell.into(),
            Self::AttackSwing(_) => WorldOpcode::CmsgAttackSwing.into(),
            Self::AttackStop(_) => WorldOpcode::CmsgAttackStop.into(),
            Self::Repop(_) => WorldOpcode::CmsgRepopRequest.into(),
            Self::ReclaimCorpse(_) => WorldOpcode::CmsgReclaimCorpse.into(),
            Self::SpiritHealerActivate(_) => WorldOpcode::CmsgSpiritHealerActivate.into(),
            Self::CorpseQuery(_) => WorldOpcode::MsgCorpseQuery.into(),
            Self::Loot(_) => WorldOpcode::CmsgLoot.into(),
            Self::AutostoreLootItem(_) => WorldOpcode::CmsgAutostoreLootItem.into(),
            Self::LootMoney(_) => WorldOpcode::CmsgLootMoney.into(),
            Self::LootRelease(_) => WorldOpcode::CmsgLootRelease.into(),
            Self::GmTicketGetTicket(_) => WorldOpcode::CmsgGmTicketGetTicket.into(),
            Self::SetActiveMover(_) => WorldOpcode::CmsgSetActiveMover.into(),
            Self::MoveTeleportAck(_) => WorldOpcode::MsgMoveTeleportAck.into(),
            Self::GroupChangeSubGroup(_) => WorldOpcode::CmsgGroupChangeSubGroup.into(),
            Self::RequestPartyMemberStats(_) => WorldOpcode::CmsgRequestPartyMemberStats.into(),
            Self::QueryNextMailTime(_) => WorldOpcode::MsgQueryNextMailTime.into(),
            Self::SendMail(_) => WorldOpcode::CmsgSendMail.into(),
            Self::GetMailList(_) => WorldOpcode::CmsgGetMailList.into(),
            Self::MailTakeMoney(_) => WorldOpcode::CmsgMailTakeMoney.into(),
            Self::MailTakeItem(_) => WorldOpcode::CmsgMailTakeItem.into(),
            Self::MailMarkAsRead(_) => WorldOpcode::CmsgMailMarkAsRead.into(),
            Self::MailReturnToSender(_) => WorldOpcode::CmsgMailReturnToSender.into(),
            Self::MailDelete(_) => WorldOpcode::CmsgMailDelete.into(),
            Self::MailCreateTextItem(_) => WorldOpcode::CmsgMailCreateTextItem.into(),
            Self::ItemTextQuery(_) => WorldOpcode::CmsgItemTextQuery.into(),
            Self::GroupRaidConvert(_) => WorldOpcode::CmsgGroupRaidConvert.into(),
            Self::GroupAssistantLeader(_) => WorldOpcode::CmsgGroupAssistantLeader.into(),
            Self::LogoutRequest(_) => WorldOpcode::CmsgLogoutRequest.into(),
            Self::LogoutCancel(_) => WorldOpcode::CmsgLogoutCancel.into(),
            Self::PlayerLogout(_) => WorldOpcode::CmsgPlayerLogout.into(),
            Self::Untyped { opcode } => *opcode,
        }
    }

    packet_accessor!(ping, Ping, PingRequest, "CMSG_PING");
    packet_accessor!(
        char_create,
        CharCreate,
        CharCreateRequest,
        "CMSG_CHAR_CREATE"
    );
    packet_accessor!(char_enum, CharEnum, CharEnumRequest, "CMSG_CHAR_ENUM");
    packet_accessor!(
        char_delete,
        CharDelete,
        CharDeleteRequest,
        "CMSG_CHAR_DELETE"
    );
    packet_accessor!(
        player_login,
        PlayerLogin,
        PlayerLoginRequest,
        "CMSG_PLAYER_LOGIN"
    );
    packet_accessor!(name_query, NameQuery, NameQueryRequest, "CMSG_NAME_QUERY");
    packet_accessor!(
        item_query_single,
        ItemQuerySingle,
        ItemQuerySingleRequest,
        "CMSG_ITEM_QUERY_SINGLE"
    );
    packet_accessor!(
        item_name_query,
        ItemNameQuery,
        ItemNameQueryRequest,
        "CMSG_ITEM_NAME_QUERY"
    );
    packet_accessor!(
        page_text_query,
        PageTextQuery,
        PageTextQueryRequest,
        "CMSG_PAGE_TEXT_QUERY"
    );
    packet_accessor!(
        quest_query,
        QuestQuery,
        QuestQueryRequest,
        "CMSG_QUEST_QUERY"
    );
    packet_accessor!(
        gameobject_query,
        GameObjectQuery,
        GameObjectQueryRequest,
        "CMSG_GAMEOBJECT_QUERY"
    );
    packet_accessor!(
        creature_query,
        CreatureQuery,
        CreatureQueryRequest,
        "CMSG_CREATURE_QUERY"
    );
    packet_accessor!(
        gameobject_use,
        GameObjectUse,
        GameObjectUseRequest,
        "CMSG_GAMEOBJ_USE"
    );
    packet_accessor!(
        group_invite,
        GroupInvite,
        GroupInviteRequest,
        "CMSG_GROUP_INVITE"
    );
    packet_accessor!(
        group_uninvite,
        GroupUninvite,
        GroupUninviteRequest,
        "CMSG_GROUP_UNINVITE"
    );
    packet_accessor!(
        group_uninvite_guid,
        GroupUninviteGuid,
        GroupUninviteGuidRequest,
        "CMSG_GROUP_UNINVITE_GUID"
    );
    packet_accessor!(
        group_set_leader,
        GroupSetLeader,
        GroupSetLeaderRequest,
        "CMSG_GROUP_SET_LEADER"
    );
    packet_accessor!(
        loot_method,
        LootMethod,
        LootMethodRequest,
        "CMSG_LOOT_METHOD"
    );
    packet_accessor!(loot_roll, LootRoll, LootRollRequest, "CMSG_LOOT_ROLL");
    packet_accessor!(
        loot_master_give,
        LootMasterGive,
        LootMasterGiveRequest,
        "CMSG_LOOT_MASTER_GIVE"
    );
    packet_accessor!(
        message_chat,
        MessageChat,
        MessageChatRequest,
        "CMSG_MESSAGECHAT"
    );
    packet_accessor!(
        join_channel,
        JoinChannel,
        JoinChannelRequest,
        "CMSG_JOIN_CHANNEL"
    );
    packet_accessor!(text_emote, TextEmote, TextEmoteRequest, "CMSG_TEXT_EMOTE");
    packet_accessor!(cast_spell, CastSpell, CastSpellRequest, "CMSG_CAST_SPELL");
    packet_accessor!(use_item, UseItem, UseItemRequest, "CMSG_USE_ITEM");
    packet_accessor!(read_item, ReadItem, ReadItemRequest, "CMSG_READ_ITEM");
    packet_accessor!(
        inventory_move,
        InventoryMove,
        InventoryMoveClientRequest,
        "CMSG_*_ITEM"
    );
    packet_accessor!(
        destroy_item,
        DestroyItem,
        DestroyItemRequest,
        "CMSG_DESTROYITEM"
    );
    packet_accessor!(split_item, SplitItem, SplitItemRequest, "CMSG_SPLIT_ITEM");
    packet_accessor!(set_ammo, SetAmmo, SetAmmoRequest, "CMSG_SET_AMMO");
    packet_accessor!(
        request_account_data,
        RequestAccountData,
        RequestAccountDataRequest,
        "CMSG_REQUEST_ACCOUNT_DATA"
    );
    packet_accessor!(
        update_account_data,
        UpdateAccountData,
        UpdateAccountDataRequest,
        "CMSG_UPDATE_ACCOUNT_DATA"
    );
    packet_accessor!(
        tutorial_flag,
        TutorialFlag,
        TutorialFlagRequest,
        "CMSG_TUTORIAL_FLAG"
    );
    packet_accessor!(
        stand_state_change,
        StandStateChange,
        StandStateChangeRequest,
        "CMSG_STANDSTATECHANGE"
    );
    packet_accessor!(
        set_action_button,
        SetActionButton,
        SetActionButtonRequest,
        "CMSG_SET_ACTION_BUTTON"
    );
    packet_accessor!(
        set_selection,
        SetSelection,
        SetSelectionRequest,
        "CMSG_SET_SELECTION"
    );
    packet_accessor!(
        set_target_obsolete,
        SetTargetObsolete,
        SetTargetObsoleteRequest,
        "CMSG_SET_TARGET_OBSOLETE"
    );
    packet_accessor!(
        gossip_hello,
        GossipHello,
        GossipHelloRequest,
        "CMSG_GOSSIP_HELLO"
    );
    packet_accessor!(
        gossip_select_option,
        GossipSelectOption,
        GossipSelectOptionRequest,
        "CMSG_GOSSIP_SELECT_OPTION"
    );
    packet_accessor!(
        npc_text_query,
        NpcTextQuery,
        NpcTextQueryRequest,
        "CMSG_NPC_TEXT_QUERY"
    );
    packet_accessor!(
        questgiver_status_query,
        QuestgiverStatusQuery,
        QuestgiverStatusQueryRequest,
        "CMSG_QUESTGIVER_STATUS_QUERY"
    );
    packet_accessor!(
        questgiver_hello,
        QuestgiverHello,
        QuestgiverHelloRequest,
        "CMSG_QUESTGIVER_HELLO"
    );
    pub(super) fn questgiver_quest(&self) -> anyhow::Result<QuestgiverQuestRequest> {
        match self {
            Self::QuestgiverQueryQuest(request)
            | Self::QuestgiverAcceptQuest(request)
            | Self::QuestgiverCompleteQuest(request)
            | Self::QuestgiverRequestReward(request) => Ok(*request),
            other => {
                anyhow::bail!(
                    "expected CMSG_QUESTGIVER_*_QUEST, got opcode 0x{:04X}",
                    other.opcode()
                )
            }
        }
    }
    packet_accessor!(
        quest_reward,
        QuestReward,
        QuestRewardRequest,
        "CMSG_QUESTGIVER_CHOOSE_REWARD"
    );
    packet_accessor!(
        questlog_remove_quest,
        QuestLogRemoveQuest,
        QuestLogRemoveQuestRequest,
        "CMSG_QUESTLOG_REMOVE_QUEST"
    );
    packet_accessor!(
        list_inventory,
        ListInventory,
        ListInventoryRequest,
        "CMSG_LIST_INVENTORY"
    );
    packet_accessor!(sell_item, SellItem, SellItemRequest, "CMSG_SELL_ITEM");
    packet_accessor!(
        buyback_item,
        BuybackItem,
        BuybackItemRequest,
        "CMSG_BUYBACK_ITEM"
    );
    packet_accessor!(buy_item, BuyItem, BuyItemRequest, "CMSG_BUY_ITEM");
    packet_accessor!(
        trainer_list,
        TrainerList,
        TrainerListRequest,
        "CMSG_TRAINER_LIST"
    );
    packet_accessor!(
        trainer_buy_spell,
        TrainerBuySpell,
        TrainerBuySpellRequest,
        "CMSG_TRAINER_BUY_SPELL"
    );
    packet_accessor!(
        attack_swing,
        AttackSwing,
        AttackSwingRequest,
        "CMSG_ATTACKSWING"
    );
    packet_accessor!(
        attack_stop,
        AttackStop,
        AttackStopRequest,
        "CMSG_ATTACKSTOP"
    );
    packet_accessor!(repop, Repop, RepopRequest, "CMSG_REPOP_REQUEST");
    packet_accessor!(
        reclaim_corpse,
        ReclaimCorpse,
        ReclaimCorpseRequest,
        "CMSG_RECLAIM_CORPSE"
    );
    packet_accessor!(
        spirit_healer_activate,
        SpiritHealerActivate,
        SpiritHealerActivateRequest,
        "CMSG_SPIRIT_HEALER_ACTIVATE"
    );
    packet_accessor!(
        corpse_query,
        CorpseQuery,
        CorpseQueryRequest,
        "MSG_CORPSE_QUERY"
    );
    packet_accessor!(loot, Loot, LootRequest, "CMSG_LOOT");
    packet_accessor!(
        autostore_loot_item,
        AutostoreLootItem,
        AutostoreLootItemRequest,
        "CMSG_AUTOSTORE_LOOT_ITEM"
    );
    packet_accessor!(loot_money, LootMoney, LootMoneyRequest, "CMSG_LOOT_MONEY");
    packet_accessor!(
        loot_release,
        LootRelease,
        LootReleaseRequest,
        "CMSG_LOOT_RELEASE"
    );
    packet_accessor!(
        set_active_mover,
        SetActiveMover,
        SetActiveMoverRequest,
        "CMSG_SET_ACTIVE_MOVER"
    );
    packet_accessor!(
        move_teleport_ack,
        MoveTeleportAck,
        MoveTeleportAckRequest,
        "MSG_MOVE_TELEPORT_ACK"
    );
    packet_accessor!(
        group_change_subgroup,
        GroupChangeSubGroup,
        GroupChangeSubGroupRequest,
        "CMSG_GROUP_CHANGE_SUB_GROUP"
    );
    packet_accessor!(
        request_party_member_stats,
        RequestPartyMemberStats,
        RequestPartyMemberStatsRequest,
        "CMSG_REQUEST_PARTY_MEMBER_STATS"
    );
    packet_accessor!(send_mail, SendMail, SendMailRequest, "CMSG_SEND_MAIL");
    packet_accessor!(
        get_mail_list,
        GetMailList,
        GetMailListRequest,
        "CMSG_GET_MAIL_LIST"
    );
    packet_accessor!(
        mail_create_text_item,
        MailCreateTextItem,
        MailCreateTextItemRequest,
        "CMSG_MAIL_CREATE_TEXT_ITEM"
    );
    packet_accessor!(
        item_text_query,
        ItemTextQuery,
        ItemTextQueryRequest,
        "CMSG_ITEM_TEXT_QUERY"
    );
    pub(super) fn mail_id_request(&self) -> anyhow::Result<MailIdRequest> {
        match self {
            Self::MailTakeMoney(request)
            | Self::MailTakeItem(request)
            | Self::MailMarkAsRead(request)
            | Self::MailReturnToSender(request)
            | Self::MailDelete(request) => Ok(*request),
            other => anyhow::bail!("expected CMSG_MAIL_*, got opcode 0x{:04X}", other.opcode()),
        }
    }
    packet_accessor!(
        group_assistant_leader,
        GroupAssistantLeader,
        GroupAssistantLeaderRequest,
        "CMSG_GROUP_ASSISTANT_LEADER"
    );
}

pub(super) fn parse_world_auth_session_packet(
    body: &[u8],
) -> anyhow::Result<WorldAuthSessionRequest> {
    let mut body = body;
    Ok(WorldAuthSessionRequest::read(&mut body)?)
}

pub(super) fn parse_world_client_packet(
    opcode: u32,
    body: &[u8],
) -> anyhow::Result<ParsedWorldClientPacket> {
    match WorldOpcode::try_from(opcode) {
        Ok(WorldOpcode::CmsgAuthSession) => Ok(ParsedWorldClientPacket::AuthSession(
            parse_world_auth_session_packet(body)?,
        )),
        Ok(WorldOpcode::CmsgPing) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::Ping(PingRequest::read(&mut body)?))
        }
        Ok(WorldOpcode::CmsgCharCreate) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::CharCreate(
                CharCreateRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgCharEnum) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::CharEnum(CharEnumRequest::read(
                &mut body,
            )?))
        }
        Ok(WorldOpcode::CmsgCharDelete) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::CharDelete(
                CharDeleteRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgPlayerLogin) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::PlayerLogin(
                PlayerLoginRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgNameQuery) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::NameQuery(NameQueryRequest::read(
                &mut body,
            )?))
        }
        Ok(WorldOpcode::CmsgItemQuerySingle) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::ItemQuerySingle(
                ItemQuerySingleRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgItemNameQuery) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::ItemNameQuery(
                ItemNameQueryRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgPageTextQuery) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::PageTextQuery(
                PageTextQueryRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgQuestQuery) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::QuestQuery(
                QuestQueryRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgGameObjectQuery) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::GameObjectQuery(
                GameObjectQueryRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgCreatureQuery) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::CreatureQuery(
                CreatureQueryRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgGroupInvite) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::GroupInvite(
                GroupInviteRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgGroupCancel) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::GroupCancel(
                GroupCancelRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgGroupAccept) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::GroupAccept(
                GroupAcceptRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgGroupDecline) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::GroupDecline(
                GroupDeclineRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgGroupUninvite) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::GroupUninvite(
                GroupUninviteRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgGroupUninviteGuid) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::GroupUninviteGuid(
                GroupUninviteGuidRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgGroupSetLeader) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::GroupSetLeader(
                GroupSetLeaderRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgLootMethod) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::LootMethod(
                LootMethodRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgLootRoll) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::LootRoll(LootRollRequest::read(
                &mut body,
            )?))
        }
        Ok(WorldOpcode::CmsgLootMasterGive) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::LootMasterGive(
                LootMasterGiveRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgGroupDisband) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::GroupDisband(
                GroupDisbandRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgMessageChat) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::MessageChat(
                MessageChatRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgJoinChannel) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::JoinChannel(
                JoinChannelRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgTextEmote) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::TextEmote(TextEmoteRequest::read(
                &mut body,
            )?))
        }
        Ok(WorldOpcode::CmsgCastSpell) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::CastSpell(CastSpellRequest::read(
                &mut body,
            )?))
        }
        Ok(WorldOpcode::CmsgUseItem) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::UseItem(UseItemRequest::read(
                &mut body,
            )?))
        }
        Ok(WorldOpcode::CmsgReadItem) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::ReadItem(ReadItemRequest::read(
                &mut body,
            )?))
        }
        Ok(WorldOpcode::CmsgAutoequipItem) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::InventoryMove(
                InventoryMoveClientRequest::read_auto_equip(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgAutostoreBagItem) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::InventoryMove(
                InventoryMoveClientRequest::read_auto_store_bag(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgSwapItem) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::InventoryMove(
                InventoryMoveClientRequest::read_swap_item(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgSwapInvItem) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::InventoryMove(
                InventoryMoveClientRequest::read_swap_inv_item(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgDestroyItem) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::DestroyItem(
                DestroyItemRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgSplitItem) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::SplitItem(SplitItemRequest::read(
                &mut body,
            )?))
        }
        Ok(WorldOpcode::CmsgSetAmmo) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::SetAmmo(SetAmmoRequest::read(
                &mut body,
            )?))
        }
        Ok(WorldOpcode::CmsgCancelCast) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::CancelCast(
                CancelCastRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgCancelAutoRepeatSpell) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::CancelAutoRepeatSpell(
                CancelAutoRepeatSpellRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgGameObjUse) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::GameObjectUse(
                GameObjectUseRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgQueryTime) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::QueryTime(QueryTimeRequest::read(
                &mut body,
            )?))
        }
        Ok(WorldOpcode::CmsgRequestAccountData) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::RequestAccountData(
                RequestAccountDataRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgUpdateAccountData) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::UpdateAccountData(
                UpdateAccountDataRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgTutorialFlag) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::TutorialFlag(
                TutorialFlagRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgTutorialClear) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::TutorialClear(
                TutorialClearRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgTutorialReset) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::TutorialReset(
                TutorialResetRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgStandStateChange) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::StandStateChange(
                StandStateChangeRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgSetActionButton) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::SetActionButton(
                SetActionButtonRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgSetSelection) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::SetSelection(
                SetSelectionRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgSetTargetObsolete) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::SetTargetObsolete(
                SetTargetObsoleteRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgGossipHello) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::GossipHello(
                GossipHelloRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgGossipSelectOption) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::GossipSelectOption(
                GossipSelectOptionRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgNpcTextQuery) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::NpcTextQuery(
                NpcTextQueryRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgQuestgiverStatusQuery) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::QuestgiverStatusQuery(
                QuestgiverStatusQueryRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgQuestgiverHello) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::QuestgiverHello(
                QuestgiverHelloRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgQuestgiverQueryQuest) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::QuestgiverQueryQuest(
                QuestgiverQuestRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgQuestgiverAcceptQuest) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::QuestgiverAcceptQuest(
                QuestgiverQuestRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgQuestgiverCompleteQuest) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::QuestgiverCompleteQuest(
                QuestgiverQuestRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgQuestgiverRequestReward) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::QuestgiverRequestReward(
                QuestgiverQuestRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgQuestgiverChooseReward) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::QuestReward(
                QuestRewardRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgQuestlogRemoveQuest) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::QuestLogRemoveQuest(
                QuestLogRemoveQuestRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgListInventory) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::ListInventory(
                ListInventoryRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgSellItem) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::SellItem(SellItemRequest::read(
                &mut body,
            )?))
        }
        Ok(WorldOpcode::CmsgBuybackItem) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::BuybackItem(
                BuybackItemRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgBuyItem) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::BuyItem(BuyItemRequest::read(
                &mut body,
            )?))
        }
        Ok(WorldOpcode::CmsgTrainerList) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::TrainerList(
                TrainerListRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgTrainerBuySpell) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::TrainerBuySpell(
                TrainerBuySpellRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgAttackSwing) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::AttackSwing(
                AttackSwingRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgAttackStop) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::AttackStop(
                AttackStopRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgRepopRequest) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::Repop(RepopRequest::read(
                &mut body,
            )?))
        }
        Ok(WorldOpcode::CmsgReclaimCorpse) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::ReclaimCorpse(
                ReclaimCorpseRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgSpiritHealerActivate) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::SpiritHealerActivate(
                SpiritHealerActivateRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::MsgCorpseQuery) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::CorpseQuery(
                CorpseQueryRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgLoot) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::Loot(LootRequest::read(&mut body)?))
        }
        Ok(WorldOpcode::CmsgAutostoreLootItem) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::AutostoreLootItem(
                AutostoreLootItemRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgLootMoney) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::LootMoney(LootMoneyRequest::read(
                &mut body,
            )?))
        }
        Ok(WorldOpcode::CmsgLootRelease) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::LootRelease(
                LootReleaseRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgGmTicketGetTicket) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::GmTicketGetTicket(
                GmTicketGetTicketRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgSetActiveMover) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::SetActiveMover(
                SetActiveMoverRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::MsgMoveTeleportAck) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::MoveTeleportAck(
                MoveTeleportAckRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgGroupChangeSubGroup) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::GroupChangeSubGroup(
                GroupChangeSubGroupRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgRequestPartyMemberStats) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::RequestPartyMemberStats(
                RequestPartyMemberStatsRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::MsgQueryNextMailTime) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::QueryNextMailTime(
                QueryNextMailTimeRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgSendMail) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::SendMail(SendMailRequest::read(
                &mut body,
            )?))
        }
        Ok(WorldOpcode::CmsgGetMailList) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::GetMailList(
                GetMailListRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgMailTakeMoney) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::MailTakeMoney(MailIdRequest::read(
                &mut body,
                "CMSG_MAIL_TAKE_MONEY",
            )?))
        }
        Ok(WorldOpcode::CmsgMailTakeItem) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::MailTakeItem(MailIdRequest::read(
                &mut body,
                "CMSG_MAIL_TAKE_ITEM",
            )?))
        }
        Ok(WorldOpcode::CmsgMailMarkAsRead) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::MailMarkAsRead(
                MailIdRequest::read(&mut body, "CMSG_MAIL_MARK_AS_READ")?,
            ))
        }
        Ok(WorldOpcode::CmsgMailReturnToSender) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::MailReturnToSender(
                MailIdRequest::read(&mut body, "CMSG_MAIL_RETURN_TO_SENDER")?,
            ))
        }
        Ok(WorldOpcode::CmsgMailDelete) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::MailDelete(MailIdRequest::read(
                &mut body,
                "CMSG_MAIL_DELETE",
            )?))
        }
        Ok(WorldOpcode::CmsgMailCreateTextItem) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::MailCreateTextItem(
                MailCreateTextItemRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgItemTextQuery) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::ItemTextQuery(
                ItemTextQueryRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgGroupRaidConvert) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::GroupRaidConvert(
                GroupRaidConvertRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgGroupAssistantLeader) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::GroupAssistantLeader(
                GroupAssistantLeaderRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgLogoutRequest) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::LogoutRequest(LogoutRequest::read(
                &mut body,
            )?))
        }
        Ok(WorldOpcode::CmsgLogoutCancel) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::LogoutCancel(
                LogoutCancelRequest::read(&mut body)?,
            ))
        }
        Ok(WorldOpcode::CmsgPlayerLogout) => {
            let mut body = body;
            Ok(ParsedWorldClientPacket::PlayerLogout(
                PlayerLogoutRequest::read(&mut body)?,
            ))
        }
        Ok(server_opcode) if server_opcode.is_server_only() => {
            anyhow::bail!("server opcode {server_opcode:?} received from client")
        }
        Ok(known_unparsed_opcode) => Ok(ParsedWorldClientPacket::Untyped {
            opcode: known_unparsed_opcode.into(),
        }),
        Err(_) => Ok(ParsedWorldClientPacket::Untyped { opcode }),
    }
}

#[cfg(test)]
mod packet_dispatch_tests {
    use super::*;

    #[test]
    pub(in crate::world) fn parse_world_client_packet_decodes_ping_request() {
        let parsed = parse_world_client_packet(0x01DC, &0x12345678u32.to_le_bytes()).unwrap();
        assert_eq!(parsed.opcode(), 0x01DC);
        assert_eq!(parsed.ping().unwrap().sequence, 0x12345678);
    }

    #[test]
    pub(in crate::world) fn parse_world_client_packet_decodes_control_requests() {
        let parsed =
            parse_world_client_packet(0x0050, &0x0102_0304_0506_0708u64.to_le_bytes()).unwrap();
        assert_eq!(parsed.name_query().unwrap().raw_guid, 0x0102_0304_0506_0708);

        let parsed = parse_world_client_packet(0x0056, &6948u32.to_le_bytes()).unwrap();
        assert_eq!(parsed.item_query_single().unwrap().item_id, 6948);

        let parsed = parse_world_client_packet(0x02C4, &25u32.to_le_bytes()).unwrap();
        assert_eq!(parsed.item_name_query().unwrap().item_id, 25);

        let parsed = parse_world_client_packet(0x020A, &2u32.to_le_bytes()).unwrap();
        assert_eq!(parsed.request_account_data().unwrap().data_type, 2);

        let mut update_body = Vec::new();
        update_body.extend_from_slice(&3u32.to_le_bytes());
        update_body.extend_from_slice(&12u32.to_le_bytes());
        update_body.extend_from_slice(&[1, 2, 3, 4]);
        let parsed = parse_world_client_packet(0x020B, &update_body).unwrap();
        let update = parsed.update_account_data().unwrap();
        assert_eq!(update.data_type, 3);
        assert_eq!(update.decompressed_size, 12);
        assert_eq!(update.compressed_data, [1, 2, 3, 4]);

        let parsed = parse_world_client_packet(0x00FE, &42u32.to_le_bytes()).unwrap();
        assert_eq!(parsed.tutorial_flag().unwrap().flag, 42);

        let parsed = parse_world_client_packet(0x0101, &1u32.to_le_bytes()).unwrap();
        assert_eq!(parsed.stand_state_change().unwrap().stand_state, 1);

        let parsed = parse_world_client_packet(0x0128, &[7, 25, 0, 0, 0x80]).unwrap();
        let action = parsed.set_action_button().unwrap();
        assert_eq!(action.button, 7);
        assert_eq!(action.action(), 25);
        assert_eq!(action.action_type(), 0x80);

        let parsed =
            parse_world_client_packet(0x013D, &0xAABB_CCDD_EEFF_0011u64.to_le_bytes()).unwrap();
        assert_eq!(
            parsed.set_selection().unwrap().raw_guid,
            0xAABB_CCDD_EEFF_0011
        );

        let parsed =
            parse_world_client_packet(0x013E, &0x1122_3344_5566_7788u64.to_le_bytes()).unwrap();
        assert_eq!(
            parsed.set_target_obsolete().unwrap().raw_guid,
            0x1122_3344_5566_7788
        );

        let parsed =
            parse_world_client_packet(0x026A, &0x8877_6655_4433_2211u64.to_le_bytes()).unwrap();
        assert_eq!(
            parsed.set_active_mover().unwrap().raw_guid,
            0x8877_6655_4433_2211
        );

        let mut teleport_ack_body = vec![0x01, 0x07];
        teleport_ack_body.extend_from_slice(&9u32.to_le_bytes());
        teleport_ack_body.extend_from_slice(&1234u32.to_le_bytes());
        let parsed = parse_world_client_packet(0x00C7, &teleport_ack_body).unwrap();
        assert_eq!(parsed.opcode(), 0x00C7);
        let ack = parsed.move_teleport_ack().unwrap();
        assert_eq!(ack.player.raw(), 7);
        assert_eq!(ack.counter, 9);
        assert_eq!(ack.client_time, 1234);

        let parsed = parse_world_client_packet(0x005C, &33u32.to_le_bytes()).unwrap();
        assert_eq!(parsed.quest_query().unwrap().quest_id, 33);

        let mut creature_body = Vec::new();
        creature_body.extend_from_slice(&3101u32.to_le_bytes());
        creature_body.extend_from_slice(&0x1000_0000_0000_0C1Du64.to_le_bytes());
        let parsed = parse_world_client_packet(0x0060, &creature_body).unwrap();
        let creature = parsed.creature_query().unwrap();
        assert_eq!(creature.entry, 3101);
        assert_eq!(creature.raw_guid, 0x1000_0000_0000_0C1D);

        let mut gameobject_body = Vec::new();
        gameobject_body.extend_from_slice(&55u32.to_le_bytes());
        gameobject_body.extend_from_slice(&0xF110_0000_0000_0037u64.to_le_bytes());
        let parsed = parse_world_client_packet(0x005E, &gameobject_body).unwrap();
        let gameobject = parsed.gameobject_query().unwrap();
        assert_eq!(gameobject.entry, 55);
        assert_eq!(gameobject.raw_guid, 0xF110_0000_0000_0037);

        let parsed =
            parse_world_client_packet(0x00B1, &0xF110_0000_0000_0037u64.to_le_bytes()).unwrap();
        assert_eq!(
            parsed.gameobject_use().unwrap().raw_guid,
            0xF110_0000_0000_0037
        );

        let parsed =
            parse_world_client_packet(0x017B, &0xF130_0000_0000_0037u64.to_le_bytes()).unwrap();
        assert_eq!(
            parsed.gossip_hello().unwrap().raw_guid,
            0xF130_0000_0000_0037
        );

        let mut npc_text_body = Vec::new();
        npc_text_body.extend_from_slice(&68u32.to_le_bytes());
        npc_text_body.extend_from_slice(&0xF130_0000_0000_0037u64.to_le_bytes());
        let parsed = parse_world_client_packet(0x017F, &npc_text_body).unwrap();
        let npc_text = parsed.npc_text_query().unwrap();
        assert_eq!(npc_text.text_id, 68);
        assert_eq!(npc_text.raw_guid, 0xF130_0000_0000_0037);

        let parsed =
            parse_world_client_packet(0x0182, &0xF130_0000_0000_0037u64.to_le_bytes()).unwrap();
        assert_eq!(
            parsed.questgiver_status_query().unwrap().raw_guid,
            0xF130_0000_0000_0037
        );

        let parsed =
            parse_world_client_packet(0x0184, &0xF130_0000_0000_0037u64.to_le_bytes()).unwrap();
        assert_eq!(
            parsed.questgiver_hello().unwrap().raw_guid,
            0xF130_0000_0000_0037
        );

        let parsed =
            parse_world_client_packet(0x019E, &0xF130_0000_0000_0037u64.to_le_bytes()).unwrap();
        assert_eq!(
            parsed.list_inventory().unwrap().raw_guid,
            0xF130_0000_0000_0037
        );

        let parsed =
            parse_world_client_packet(0x01B0, &0xF130_0000_0000_0037u64.to_le_bytes()).unwrap();
        assert_eq!(
            parsed.trainer_list().unwrap().raw_guid,
            0xF130_0000_0000_0037
        );
    }

    #[test]
    pub(in crate::world) fn parse_world_client_packet_keeps_questgiver_intents_distinct() {
        let mut body = Vec::new();
        body.extend_from_slice(&0xF130_0000_0000_0037u64.to_le_bytes());
        body.extend_from_slice(&47u32.to_le_bytes());

        let parsed = parse_world_client_packet(0x0186, &body).unwrap();
        assert!(matches!(
            parsed,
            ParsedWorldClientPacket::QuestgiverQueryQuest(_)
        ));
        assert_eq!(parsed.opcode(), 0x0186);

        let parsed = parse_world_client_packet(0x0189, &body).unwrap();
        assert!(matches!(
            parsed,
            ParsedWorldClientPacket::QuestgiverAcceptQuest(_)
        ));
        assert_eq!(parsed.opcode(), 0x0189);

        let parsed = parse_world_client_packet(0x018A, &body).unwrap();
        assert!(matches!(
            parsed,
            ParsedWorldClientPacket::QuestgiverCompleteQuest(_)
        ));
        assert_eq!(parsed.opcode(), 0x018A);

        let parsed = parse_world_client_packet(0x018C, &body).unwrap();
        assert!(matches!(
            parsed,
            ParsedWorldClientPacket::QuestgiverRequestReward(_)
        ));
        assert_eq!(parsed.opcode(), 0x018C);
    }

    #[test]
    pub(in crate::world) fn parse_world_client_packet_decodes_empty_control_requests() {
        assert_eq!(
            parse_world_client_packet(0x01CE, &[]).unwrap().opcode(),
            0x01CE
        );
        assert_eq!(
            parse_world_client_packet(0x00FF, &[]).unwrap().opcode(),
            0x00FF
        );
        assert_eq!(
            parse_world_client_packet(0x0100, &[]).unwrap().opcode(),
            0x0100
        );
        assert_eq!(
            parse_world_client_packet(0x0211, &[]).unwrap().opcode(),
            0x0211
        );
        assert_eq!(
            parse_world_client_packet(0x0284, &[]).unwrap().opcode(),
            0x0284
        );
        assert_eq!(
            parse_world_client_packet(0x004B, &[]).unwrap().opcode(),
            0x004B
        );
        assert_eq!(
            parse_world_client_packet(0x004E, &[]).unwrap().opcode(),
            0x004E
        );
        assert_eq!(
            parse_world_client_packet(0x004A, &[]).unwrap().opcode(),
            0x004A
        );
        assert_eq!(
            parse_world_client_packet(0x0070, &[]).unwrap().opcode(),
            0x0070
        );
        assert_eq!(
            parse_world_client_packet(0x0072, &[]).unwrap().opcode(),
            0x0072
        );
        assert_eq!(
            parse_world_client_packet(0x0073, &[]).unwrap().opcode(),
            0x0073
        );
        assert_eq!(
            parse_world_client_packet(0x007B, &[]).unwrap().opcode(),
            0x007B
        );
        assert_eq!(
            parse_world_client_packet(0x028E, &[]).unwrap().opcode(),
            0x028E
        );
    }

    #[test]
    pub(in crate::world) fn parse_world_client_packet_decodes_group_requests() {
        let parsed = parse_world_client_packet(0x006E, b"Ada\0").unwrap();
        assert_eq!(parsed.group_invite().unwrap().member_name, "Ada");

        let parsed = parse_world_client_packet(0x0075, b"Grace\0").unwrap();
        assert_eq!(parsed.group_uninvite().unwrap().member_name, "Grace");

        let parsed =
            parse_world_client_packet(0x0076, &0x0102_0304_0506_0708u64.to_le_bytes()).unwrap();
        assert_eq!(
            parsed.group_uninvite_guid().unwrap().raw_guid,
            0x0102_0304_0506_0708
        );

        let parsed =
            parse_world_client_packet(0x0078, &0x8877_6655_4433_2211u64.to_le_bytes()).unwrap();
        assert_eq!(
            parsed.group_set_leader().unwrap().raw_guid,
            0x8877_6655_4433_2211
        );

        let mut subgroup = Vec::new();
        subgroup.extend_from_slice(b"Linus\0");
        subgroup.push(3);
        let parsed = parse_world_client_packet(0x027E, &subgroup).unwrap();
        let subgroup = parsed.group_change_subgroup().unwrap();
        assert_eq!(subgroup.member_name, "Linus");
        assert_eq!(subgroup.subgroup, 3);

        let mut assistant = Vec::new();
        assistant.extend_from_slice(&0xAABB_CCDD_EEFF_0011u64.to_le_bytes());
        assistant.push(1);
        let parsed = parse_world_client_packet(0x028F, &assistant).unwrap();
        let assistant = parsed.group_assistant_leader().unwrap();
        assert_eq!(assistant.raw_guid, 0xAABB_CCDD_EEFF_0011);
        assert!(assistant.enabled);

        let parsed =
            parse_world_client_packet(0x027F, &0xF130_0000_0000_0037u64.to_le_bytes()).unwrap();
        assert_eq!(
            parsed.request_party_member_stats().unwrap().raw_guid,
            0xF130_0000_0000_0037
        );

        let mut loot_method = Vec::new();
        loot_method.extend_from_slice(&2u32.to_le_bytes());
        loot_method.extend_from_slice(&0xF130_0000_0000_0037u64.to_le_bytes());
        loot_method.extend_from_slice(&3u32.to_le_bytes());
        let parsed = parse_world_client_packet(0x007A, &loot_method).unwrap();
        let loot_method = parsed.loot_method().unwrap();
        assert_eq!(loot_method.loot_method, 2);
        assert_eq!(loot_method.master_looter_raw_guid, 0xF130_0000_0000_0037);
        assert_eq!(loot_method.loot_threshold, 3);
    }

    #[test]
    pub(in crate::world) fn parse_world_client_packet_decodes_mail_requests() {
        let mailbox = 0x0102_0304_0506_0708u64;
        let mut send_body = Vec::new();
        send_body.extend_from_slice(&mailbox.to_le_bytes());
        send_body.extend_from_slice(b"Receiver\0Subject\0Body\0");
        send_body.extend_from_slice(&41u32.to_le_bytes());
        send_body.extend_from_slice(&0u32.to_le_bytes());
        send_body.extend_from_slice(&0u64.to_le_bytes());
        send_body.extend_from_slice(&123u32.to_le_bytes());
        send_body.extend_from_slice(&0u32.to_le_bytes());
        send_body.extend_from_slice(&0u64.to_le_bytes());
        send_body.push(0);
        let parsed = parse_world_client_packet(0x0238, &send_body).unwrap();
        let send = parsed.send_mail().unwrap();
        assert_eq!(send.mailbox_raw_guid, mailbox);
        assert_eq!(send.receiver, "Receiver");
        assert_eq!(send.subject, "Subject");
        assert_eq!(send.body, "Body");
        assert_eq!(send.money, 123);

        let mut mail_id_body = Vec::new();
        mail_id_body.extend_from_slice(&mailbox.to_le_bytes());
        mail_id_body.extend_from_slice(&42u32.to_le_bytes());
        let parsed = parse_world_client_packet(0x0246, &mail_id_body).unwrap();
        assert_eq!(parsed.mail_id_request().unwrap().mail_id, 42);

        let mut text_body = Vec::new();
        text_body.extend_from_slice(&5u32.to_le_bytes());
        text_body.extend_from_slice(&42u32.to_le_bytes());
        text_body.extend_from_slice(&0x7000_0000u32.to_le_bytes());
        let parsed = parse_world_client_packet(0x0243, &text_body).unwrap();
        assert_eq!(parsed.item_text_query().unwrap().item_text_id, 5);
    }

    #[test]
    pub(in crate::world) fn parse_world_auth_session_packet_decodes_typed_request() {
        let request = WorldAuthSessionRequest {
            client_build: 5875,
            account: "RUSTAUTH".to_string(),
            client_seed: 0x88776655,
            digest: [0x42; 20],
            addon_data: vec![9, 8, 7],
        };
        let mut body = Vec::new();
        request.write(&mut body);

        let parsed = parse_world_auth_session_packet(&body).unwrap();
        assert_eq!(parsed, request);
    }

    #[test]
    pub(in crate::world) fn parse_world_client_packet_decodes_auth_session_request() {
        let request = WorldAuthSessionRequest {
            client_build: 5875,
            account: "RUSTAUTH".to_string(),
            client_seed: 0x10203040,
            digest: [0x24; 20],
            addon_data: Vec::new(),
        };
        let mut body = Vec::new();
        request.write(&mut body);

        let parsed = parse_world_client_packet(0x01ED, &body).unwrap();
        assert_eq!(parsed.opcode(), 0x01ED);
        match parsed {
            ParsedWorldClientPacket::AuthSession(parsed) => assert_eq!(parsed, request),
            other => panic!("expected auth session packet, got {other:?}"),
        }
    }

    #[test]
    pub(in crate::world) fn parse_world_client_packet_keeps_unknown_opcodes_untyped() {
        let parsed = parse_world_client_packet(0x0ABC, &[1, 2, 3]).unwrap();
        assert_eq!(parsed.opcode(), 0x0ABC);
        assert!(parsed.ping().is_err());
    }

    #[test]
    pub(in crate::world) fn parse_world_client_packet_rejects_server_opcode_from_client() {
        let error = parse_world_client_packet(0x01DD, &[0; 4]).unwrap_err();
        assert!(error.to_string().contains("server opcode"));
    }
}
