//! World server packet structures for WoW 1.12.x.
//!
//! These types are intentionally small at first. New opcode families should add
//! typed request/response structs here before handler logic parses raw bytes.

use bytes::{Buf, BufMut};
use std::io;
use wow_common::guid::{ObjectGuid, PackedGuid};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum WorldOpcode {
    CmsgCharCreate = 0x0036,
    CmsgCharEnum = 0x0037,
    CmsgCharDelete = 0x0038,
    SmsgCharCreate = 0x003A,
    SmsgCharEnum = 0x003B,
    SmsgCharDelete = 0x003C,
    CmsgPlayerLogin = 0x003D,
    SmsgCharacterLoginFailed = 0x0041,
    SmsgLoginSetTimeSpeed = 0x0042,
    CmsgPlayerLogout = 0x004A,
    CmsgLogoutRequest = 0x004B,
    SmsgLogoutResponse = 0x004C,
    SmsgLogoutComplete = 0x004D,
    CmsgLogoutCancel = 0x004E,
    SmsgLogoutCancelAck = 0x004F,
    CmsgNameQuery = 0x0050,
    SmsgNameQueryResponse = 0x0051,
    CmsgItemQuerySingle = 0x0056,
    SmsgItemQuerySingleResponse = 0x0058,
    CmsgPageTextQuery = 0x005A,
    SmsgPageTextQueryResponse = 0x005B,
    CmsgQuestQuery = 0x005C,
    SmsgQuestQueryResponse = 0x005D,
    CmsgGameObjectQuery = 0x005E,
    SmsgGameObjectQueryResponse = 0x005F,
    CmsgCreatureQuery = 0x0060,
    SmsgCreatureQueryResponse = 0x0061,
    CmsgGroupInvite = 0x006E,
    SmsgGroupInvite = 0x006F,
    CmsgGroupCancel = 0x0070,
    CmsgGroupAccept = 0x0072,
    CmsgGroupDecline = 0x0073,
    SmsgGroupDecline = 0x0074,
    CmsgGroupUninvite = 0x0075,
    CmsgGroupUninviteGuid = 0x0076,
    SmsgGroupUninvite = 0x0077,
    CmsgGroupSetLeader = 0x0078,
    SmsgGroupSetLeader = 0x0079,
    CmsgLootMethod = 0x007A,
    CmsgGroupDisband = 0x007B,
    SmsgGroupDestroyed = 0x007C,
    SmsgGroupList = 0x007D,
    SmsgPartyCommandResult = 0x007F,
    CmsgMessageChat = 0x0095,
    SmsgMessageChat = 0x0096,
    CmsgJoinChannel = 0x0097,
    SmsgChannelNotify = 0x0099,
    SmsgUpdateObject = 0x00A9,
    SmsgDestroyObject = 0x00AA,
    CmsgUseItem = 0x00AB,
    CmsgReadItem = 0x00AD,
    SmsgReadItemOk = 0x00AE,
    SmsgReadItemFailed = 0x00AF,
    SmsgItemCooldown = 0x00B0,
    CmsgGameObjUse = 0x00B1,
    CmsgAreaTrigger = 0x00B4,
    MsgMoveStartForward = 0x00B5,
    MsgMoveStartBackward = 0x00B6,
    MsgMoveStop = 0x00B7,
    MsgMoveStartStrafeLeft = 0x00B8,
    MsgMoveStartStrafeRight = 0x00B9,
    MsgMoveStopStrafe = 0x00BA,
    MsgMoveJump = 0x00BB,
    MsgMoveStartTurnLeft = 0x00BC,
    MsgMoveStartTurnRight = 0x00BD,
    MsgMoveStopTurn = 0x00BE,
    MsgMoveStartPitchUp = 0x00BF,
    MsgMoveStartPitchDown = 0x00C0,
    MsgMoveStopPitch = 0x00C1,
    MsgMoveSetRunMode = 0x00C2,
    MsgMoveSetWalkMode = 0x00C3,
    MsgMoveTeleportAck = 0x00C7,
    MsgMoveFallLand = 0x00C9,
    MsgMoveStartSwim = 0x00CA,
    MsgMoveStopSwim = 0x00CB,
    MsgMoveSetFacing = 0x00DA,
    MsgMoveSetPitch = 0x00DB,
    SmsgMonsterMove = 0x00DD,
    SmsgForceRunSpeedChange = 0x00E2,
    CmsgForceRunSpeedChangeAck = 0x00E3,
    SmsgForceMoveRoot = 0x00E8,
    CmsgForceMoveRootAck = 0x00E9,
    SmsgForceMoveUnroot = 0x00EA,
    CmsgForceMoveUnrootAck = 0x00EB,
    MsgMoveHeartbeat = 0x00EE,
    SmsgTriggerCinematic = 0x00FA,
    SmsgTutorialFlags = 0x00FD,
    CmsgTutorialFlag = 0x00FE,
    CmsgTutorialClear = 0x00FF,
    CmsgTutorialReset = 0x0100,
    CmsgStandStateChange = 0x0101,
    SmsgEmote = 0x0103,
    CmsgTextEmote = 0x0104,
    SmsgTextEmote = 0x0105,
    CmsgAutostoreLootItem = 0x0108,
    CmsgAutoequipItem = 0x010A,
    CmsgAutostoreBagItem = 0x010B,
    CmsgSwapItem = 0x010C,
    CmsgSwapInvItem = 0x010D,
    CmsgSplitItem = 0x010E,
    CmsgDestroyItem = 0x0111,
    SmsgInventoryChangeFailure = 0x0112,
    CmsgCancelTrade = 0x011C,
    SmsgInitializeFactions = 0x0122,
    SmsgSetFactionVisible = 0x0123,
    SmsgSetFactionStanding = 0x0124,
    SmsgSetProficiency = 0x0127,
    CmsgSetActionButton = 0x0128,
    SmsgActionButtons = 0x0129,
    SmsgInitialSpells = 0x012A,
    SmsgLearnedSpell = 0x012B,
    CmsgCastSpell = 0x012E,
    CmsgCancelCast = 0x012F,
    SmsgCastResult = 0x0130,
    SmsgSpellStart = 0x0131,
    SmsgSpellGo = 0x0132,
    SmsgSpellFailure = 0x0133,
    SmsgSpellCooldown = 0x0134,
    SmsgCooldownEvent = 0x0135,
    SmsgUpdateAuraDuration = 0x0137,
    MsgChannelStart = 0x0139,
    MsgChannelUpdate = 0x013A,
    CmsgSetSelection = 0x013D,
    CmsgSetTargetObsolete = 0x013E,
    CmsgAttackSwing = 0x0141,
    CmsgAttackStop = 0x0142,
    SmsgAttackStart = 0x0143,
    SmsgAttackStop = 0x0144,
    SmsgAttackSwingNotInRange = 0x0145,
    SmsgAttackSwingBadFacing = 0x0146,
    SmsgAttackSwingDeadTarget = 0x0148,
    SmsgAttackSwingCantAttack = 0x0149,
    SmsgAttackerStateUpdate = 0x014A,
    SmsgSpellHealLog = 0x0150,
    SmsgSpellEnergizeLog = 0x0151,
    SmsgBindpointUpdate = 0x0155,
    SmsgPlayerBound = 0x0158,
    CmsgRepopRequest = 0x015A,
    CmsgLoot = 0x015D,
    CmsgLootMoney = 0x015E,
    CmsgLootRelease = 0x015F,
    SmsgLootResponse = 0x0160,
    SmsgLootReleaseResponse = 0x0161,
    SmsgLootRemoved = 0x0162,
    SmsgLootMoneyNotify = 0x0163,
    SmsgLootClearMoney = 0x0165,
    SmsgItemPushResult = 0x0166,
    CmsgGossipHello = 0x017B,
    CmsgGossipSelectOption = 0x017C,
    SmsgGossipMessage = 0x017D,
    SmsgGossipComplete = 0x017E,
    CmsgNpcTextQuery = 0x017F,
    SmsgNpcTextUpdate = 0x0180,
    CmsgQuestgiverStatusQuery = 0x0182,
    SmsgQuestgiverStatus = 0x0183,
    CmsgQuestgiverHello = 0x0184,
    SmsgQuestgiverQuestList = 0x0185,
    CmsgQuestgiverQueryQuest = 0x0186,
    SmsgQuestgiverQuestDetails = 0x0188,
    CmsgQuestgiverAcceptQuest = 0x0189,
    CmsgQuestgiverCompleteQuest = 0x018A,
    SmsgQuestgiverRequestItems = 0x018B,
    CmsgQuestgiverRequestReward = 0x018C,
    SmsgQuestgiverOfferReward = 0x018D,
    CmsgQuestgiverChooseReward = 0x018E,
    SmsgQuestgiverQuestInvalid = 0x018F,
    CmsgQuestgiverCancel = 0x0190,
    SmsgQuestgiverQuestComplete = 0x0191,
    CmsgQuestlogRemoveQuest = 0x0194,
    SmsgQuestlogFull = 0x0195,
    SmsgQuestUpdateComplete = 0x0198,
    SmsgQuestUpdateAddKill = 0x0199,
    CmsgListInventory = 0x019E,
    SmsgListInventory = 0x019F,
    CmsgSellItem = 0x01A0,
    SmsgSellItem = 0x01A1,
    CmsgBuyItem = 0x01A2,
    CmsgBuyItemInSlot = 0x01A3,
    SmsgBuyItem = 0x01A4,
    SmsgBuyFailed = 0x01A5,
    SmsgShowTaxiNodes = 0x01A9,
    CmsgTaxiNodeStatusQuery = 0x01AA,
    SmsgTaxiNodeStatus = 0x01AB,
    CmsgTaxiQueryAvailableNodes = 0x01AC,
    CmsgActivateTaxi = 0x01AD,
    SmsgActivateTaxiReply = 0x01AE,
    SmsgNewTaxiPath = 0x01AF,
    CmsgTrainerList = 0x01B0,
    SmsgTrainerList = 0x01B1,
    CmsgTrainerBuySpell = 0x01B2,
    SmsgTrainerBuySucceeded = 0x01B3,
    SmsgTrainerBuyFailed = 0x01B4,
    CmsgBinderActivate = 0x01B5,
    CmsgBankerActivate = 0x01B7,
    SmsgShowBank = 0x01B8,
    CmsgBuyBankSlot = 0x01B9,
    SmsgBuyBankSlotResult = 0x01BA,
    CmsgQueryTime = 0x01CE,
    SmsgQueryTimeResponse = 0x01CF,
    SmsgLogXpGain = 0x01D0,
    CmsgReclaimCorpse = 0x01D2,
    SmsgLevelupInfo = 0x01D4,
    SmsgStartMirrorTimer = 0x01D9,
    SmsgStopMirrorTimer = 0x01DB,
    CmsgPing = 0x01DC,
    SmsgPong = 0x01DD,
    SmsgSpellDelayed = 0x01E2,
    SmsgAuthChallenge = 0x01EC,
    CmsgAuthSession = 0x01ED,
    SmsgAuthResponse = 0x01EE,
    SmsgPlaySpellVisual = 0x01F3,
    CmsgZoneUpdate = 0x01F4,
    SmsgPlaySpellImpact = 0x01F7,
    SmsgExplorationExperience = 0x01F8,
    SmsgEnvironmentalDamageLog = 0x01FC,
    SmsgAccountDataTimes = 0x0209,
    CmsgRequestAccountData = 0x020A,
    CmsgUpdateAccountData = 0x020B,
    SmsgUpdateAccountData = 0x020C,
    CmsgGmTicketGetTicket = 0x0211,
    SmsgGmTicketGetTicket = 0x0212,
    MsgCorpseQuery = 0x0216,
    CmsgSpiritHealerActivate = 0x021C,
    SmsgSetRestStart = 0x021E,
    SmsgLoginVerifyWorld = 0x0236,
    CmsgSendMail = 0x0238,
    SmsgSendMailResult = 0x0239,
    CmsgGetMailList = 0x023A,
    SmsgMailListResult = 0x023B,
    CmsgItemTextQuery = 0x0243,
    SmsgItemTextQueryResponse = 0x0244,
    CmsgMailTakeMoney = 0x0245,
    CmsgMailTakeItem = 0x0246,
    CmsgMailMarkAsRead = 0x0247,
    CmsgMailReturnToSender = 0x0248,
    CmsgMailDelete = 0x0249,
    CmsgMailCreateTextItem = 0x024A,
    SmsgSpellLogMiss = 0x024B,
    SmsgPeriodicAuraLog = 0x024E,
    SmsgSpellNonMeleeDamageLog = 0x0250,
    MsgAuctionHello = 0x0255,
    CmsgAuctionSellItem = 0x0256,
    CmsgAuctionRemoveItem = 0x0257,
    CmsgAuctionListItems = 0x0258,
    CmsgAuctionListOwnerItems = 0x0259,
    CmsgAuctionPlaceBid = 0x025A,
    SmsgAuctionCommandResult = 0x025B,
    SmsgAuctionListResult = 0x025C,
    SmsgAuctionOwnerListResult = 0x025D,
    SmsgAuctionBidderNotification = 0x025E,
    SmsgAuctionOwnerNotification = 0x025F,
    SmsgDispelFailed = 0x0262,
    CmsgAuctionListBidderItems = 0x0264,
    SmsgAuctionBidderListResult = 0x0265,
    CmsgSetAmmo = 0x0268,
    SmsgCorpseReclaimDelay = 0x0269,
    CmsgSetActiveMover = 0x026A,
    CmsgCancelAutoRepeatSpell = 0x026D,
    SmsgSpellDispelLog = 0x027B,
    CmsgGroupChangeSubGroup = 0x027E,
    CmsgRequestPartyMemberStats = 0x027F,
    CmsgAutostoreBankItem = 0x0282,
    CmsgAutobankItem = 0x0283,
    MsgQueryNextMailTime = 0x0284,
    SmsgReceivedMail = 0x0285,
    SmsgAuctionRemovedNotification = 0x028D,
    CmsgGroupRaidConvert = 0x028E,
    CmsgGroupAssistantLeader = 0x028F,
    CmsgBuybackItem = 0x0290,
    CmsgMeetingStoneInfo = 0x0296,
    SmsgStandStateUpdate = 0x029D,
    SmsgLootAllPassed = 0x029E,
    SmsgLootRollWon = 0x029F,
    CmsgLootRoll = 0x02A0,
    SmsgLootStartRoll = 0x02A1,
    SmsgLootRoll = 0x02A2,
    CmsgLootMasterGive = 0x02A3,
    SmsgLootMasterList = 0x02A4,
    SmsgSpellFailedOther = 0x02A6,
    SmsgInitWorldStates = 0x02C2,
    CmsgItemNameQuery = 0x02C4,
    SmsgItemNameQueryResponse = 0x02C5,
    CmsgMoveSplineDone = 0x02C9,
    CmsgMoveFallReset = 0x02CA,
    CmsgRequestRaidInfo = 0x02CD,
    CmsgMoveTimeSkipped = 0x02CE,
    CmsgBattlefieldStatus = 0x02D3,
    SmsgPartyMemberStatsFull = 0x02F2,
    SmsgSplineSetRunSpeed = 0x02FE,
    SmsgSplineSetRunBackSpeed = 0x02FF,
    SmsgSplineSetSwimSpeed = 0x0300,
    SmsgSplineSetWalkSpeed = 0x0301,
    SmsgSplineSetSwimBackSpeed = 0x0302,
    SmsgBinderConfirm = 0x02EB,
}

impl TryFrom<u32> for WorldOpcode {
    type Error = io::Error;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0x0036 => Ok(Self::CmsgCharCreate),
            0x0037 => Ok(Self::CmsgCharEnum),
            0x0038 => Ok(Self::CmsgCharDelete),
            0x003A => Ok(Self::SmsgCharCreate),
            0x003B => Ok(Self::SmsgCharEnum),
            0x003C => Ok(Self::SmsgCharDelete),
            0x003D => Ok(Self::CmsgPlayerLogin),
            0x0041 => Ok(Self::SmsgCharacterLoginFailed),
            0x0042 => Ok(Self::SmsgLoginSetTimeSpeed),
            0x004A => Ok(Self::CmsgPlayerLogout),
            0x004B => Ok(Self::CmsgLogoutRequest),
            0x004C => Ok(Self::SmsgLogoutResponse),
            0x004D => Ok(Self::SmsgLogoutComplete),
            0x004E => Ok(Self::CmsgLogoutCancel),
            0x004F => Ok(Self::SmsgLogoutCancelAck),
            0x0050 => Ok(Self::CmsgNameQuery),
            0x0051 => Ok(Self::SmsgNameQueryResponse),
            0x0056 => Ok(Self::CmsgItemQuerySingle),
            0x0058 => Ok(Self::SmsgItemQuerySingleResponse),
            0x005A => Ok(Self::CmsgPageTextQuery),
            0x005B => Ok(Self::SmsgPageTextQueryResponse),
            0x005C => Ok(Self::CmsgQuestQuery),
            0x005D => Ok(Self::SmsgQuestQueryResponse),
            0x005E => Ok(Self::CmsgGameObjectQuery),
            0x005F => Ok(Self::SmsgGameObjectQueryResponse),
            0x0060 => Ok(Self::CmsgCreatureQuery),
            0x0061 => Ok(Self::SmsgCreatureQueryResponse),
            0x006E => Ok(Self::CmsgGroupInvite),
            0x006F => Ok(Self::SmsgGroupInvite),
            0x0070 => Ok(Self::CmsgGroupCancel),
            0x0072 => Ok(Self::CmsgGroupAccept),
            0x0073 => Ok(Self::CmsgGroupDecline),
            0x0074 => Ok(Self::SmsgGroupDecline),
            0x0075 => Ok(Self::CmsgGroupUninvite),
            0x0076 => Ok(Self::CmsgGroupUninviteGuid),
            0x0077 => Ok(Self::SmsgGroupUninvite),
            0x0078 => Ok(Self::CmsgGroupSetLeader),
            0x0079 => Ok(Self::SmsgGroupSetLeader),
            0x007A => Ok(Self::CmsgLootMethod),
            0x007B => Ok(Self::CmsgGroupDisband),
            0x007C => Ok(Self::SmsgGroupDestroyed),
            0x007D => Ok(Self::SmsgGroupList),
            0x007F => Ok(Self::SmsgPartyCommandResult),
            0x0095 => Ok(Self::CmsgMessageChat),
            0x0096 => Ok(Self::SmsgMessageChat),
            0x0097 => Ok(Self::CmsgJoinChannel),
            0x0099 => Ok(Self::SmsgChannelNotify),
            0x00A9 => Ok(Self::SmsgUpdateObject),
            0x00AA => Ok(Self::SmsgDestroyObject),
            0x00AB => Ok(Self::CmsgUseItem),
            0x00AD => Ok(Self::CmsgReadItem),
            0x00AE => Ok(Self::SmsgReadItemOk),
            0x00AF => Ok(Self::SmsgReadItemFailed),
            0x00B0 => Ok(Self::SmsgItemCooldown),
            0x00B1 => Ok(Self::CmsgGameObjUse),
            0x00B4 => Ok(Self::CmsgAreaTrigger),
            0x00B5 => Ok(Self::MsgMoveStartForward),
            0x00B6 => Ok(Self::MsgMoveStartBackward),
            0x00B7 => Ok(Self::MsgMoveStop),
            0x00B8 => Ok(Self::MsgMoveStartStrafeLeft),
            0x00B9 => Ok(Self::MsgMoveStartStrafeRight),
            0x00BA => Ok(Self::MsgMoveStopStrafe),
            0x00BB => Ok(Self::MsgMoveJump),
            0x00BC => Ok(Self::MsgMoveStartTurnLeft),
            0x00BD => Ok(Self::MsgMoveStartTurnRight),
            0x00BE => Ok(Self::MsgMoveStopTurn),
            0x00BF => Ok(Self::MsgMoveStartPitchUp),
            0x00C0 => Ok(Self::MsgMoveStartPitchDown),
            0x00C1 => Ok(Self::MsgMoveStopPitch),
            0x00C2 => Ok(Self::MsgMoveSetRunMode),
            0x00C3 => Ok(Self::MsgMoveSetWalkMode),
            0x00C7 => Ok(Self::MsgMoveTeleportAck),
            0x00C9 => Ok(Self::MsgMoveFallLand),
            0x00CA => Ok(Self::MsgMoveStartSwim),
            0x00CB => Ok(Self::MsgMoveStopSwim),
            0x00DA => Ok(Self::MsgMoveSetFacing),
            0x00DB => Ok(Self::MsgMoveSetPitch),
            0x00DD => Ok(Self::SmsgMonsterMove),
            0x00E2 => Ok(Self::SmsgForceRunSpeedChange),
            0x00E3 => Ok(Self::CmsgForceRunSpeedChangeAck),
            0x00E8 => Ok(Self::SmsgForceMoveRoot),
            0x00E9 => Ok(Self::CmsgForceMoveRootAck),
            0x00EA => Ok(Self::SmsgForceMoveUnroot),
            0x00EB => Ok(Self::CmsgForceMoveUnrootAck),
            0x00EE => Ok(Self::MsgMoveHeartbeat),
            0x00FA => Ok(Self::SmsgTriggerCinematic),
            0x00FD => Ok(Self::SmsgTutorialFlags),
            0x00FE => Ok(Self::CmsgTutorialFlag),
            0x00FF => Ok(Self::CmsgTutorialClear),
            0x0100 => Ok(Self::CmsgTutorialReset),
            0x0101 => Ok(Self::CmsgStandStateChange),
            0x0103 => Ok(Self::SmsgEmote),
            0x0104 => Ok(Self::CmsgTextEmote),
            0x0105 => Ok(Self::SmsgTextEmote),
            0x0108 => Ok(Self::CmsgAutostoreLootItem),
            0x010A => Ok(Self::CmsgAutoequipItem),
            0x010B => Ok(Self::CmsgAutostoreBagItem),
            0x010C => Ok(Self::CmsgSwapItem),
            0x010D => Ok(Self::CmsgSwapInvItem),
            0x010E => Ok(Self::CmsgSplitItem),
            0x0111 => Ok(Self::CmsgDestroyItem),
            0x0112 => Ok(Self::SmsgInventoryChangeFailure),
            0x011C => Ok(Self::CmsgCancelTrade),
            0x0122 => Ok(Self::SmsgInitializeFactions),
            0x0123 => Ok(Self::SmsgSetFactionVisible),
            0x0124 => Ok(Self::SmsgSetFactionStanding),
            0x0127 => Ok(Self::SmsgSetProficiency),
            0x0128 => Ok(Self::CmsgSetActionButton),
            0x0129 => Ok(Self::SmsgActionButtons),
            0x012A => Ok(Self::SmsgInitialSpells),
            0x012B => Ok(Self::SmsgLearnedSpell),
            0x012E => Ok(Self::CmsgCastSpell),
            0x012F => Ok(Self::CmsgCancelCast),
            0x0130 => Ok(Self::SmsgCastResult),
            0x0131 => Ok(Self::SmsgSpellStart),
            0x0132 => Ok(Self::SmsgSpellGo),
            0x0133 => Ok(Self::SmsgSpellFailure),
            0x0134 => Ok(Self::SmsgSpellCooldown),
            0x0135 => Ok(Self::SmsgCooldownEvent),
            0x0137 => Ok(Self::SmsgUpdateAuraDuration),
            0x0139 => Ok(Self::MsgChannelStart),
            0x013A => Ok(Self::MsgChannelUpdate),
            0x013D => Ok(Self::CmsgSetSelection),
            0x013E => Ok(Self::CmsgSetTargetObsolete),
            0x0141 => Ok(Self::CmsgAttackSwing),
            0x0142 => Ok(Self::CmsgAttackStop),
            0x0143 => Ok(Self::SmsgAttackStart),
            0x0144 => Ok(Self::SmsgAttackStop),
            0x0145 => Ok(Self::SmsgAttackSwingNotInRange),
            0x0146 => Ok(Self::SmsgAttackSwingBadFacing),
            0x0148 => Ok(Self::SmsgAttackSwingDeadTarget),
            0x0149 => Ok(Self::SmsgAttackSwingCantAttack),
            0x014A => Ok(Self::SmsgAttackerStateUpdate),
            0x0150 => Ok(Self::SmsgSpellHealLog),
            0x0151 => Ok(Self::SmsgSpellEnergizeLog),
            0x0155 => Ok(Self::SmsgBindpointUpdate),
            0x0158 => Ok(Self::SmsgPlayerBound),
            0x015A => Ok(Self::CmsgRepopRequest),
            0x015D => Ok(Self::CmsgLoot),
            0x015E => Ok(Self::CmsgLootMoney),
            0x015F => Ok(Self::CmsgLootRelease),
            0x0160 => Ok(Self::SmsgLootResponse),
            0x0161 => Ok(Self::SmsgLootReleaseResponse),
            0x0162 => Ok(Self::SmsgLootRemoved),
            0x0163 => Ok(Self::SmsgLootMoneyNotify),
            0x0165 => Ok(Self::SmsgLootClearMoney),
            0x0166 => Ok(Self::SmsgItemPushResult),
            0x017B => Ok(Self::CmsgGossipHello),
            0x017C => Ok(Self::CmsgGossipSelectOption),
            0x017D => Ok(Self::SmsgGossipMessage),
            0x017E => Ok(Self::SmsgGossipComplete),
            0x017F => Ok(Self::CmsgNpcTextQuery),
            0x0180 => Ok(Self::SmsgNpcTextUpdate),
            0x0182 => Ok(Self::CmsgQuestgiverStatusQuery),
            0x0183 => Ok(Self::SmsgQuestgiverStatus),
            0x0184 => Ok(Self::CmsgQuestgiverHello),
            0x0185 => Ok(Self::SmsgQuestgiverQuestList),
            0x0186 => Ok(Self::CmsgQuestgiverQueryQuest),
            0x0188 => Ok(Self::SmsgQuestgiverQuestDetails),
            0x0189 => Ok(Self::CmsgQuestgiverAcceptQuest),
            0x018A => Ok(Self::CmsgQuestgiverCompleteQuest),
            0x018B => Ok(Self::SmsgQuestgiverRequestItems),
            0x018C => Ok(Self::CmsgQuestgiverRequestReward),
            0x018D => Ok(Self::SmsgQuestgiverOfferReward),
            0x018E => Ok(Self::CmsgQuestgiverChooseReward),
            0x018F => Ok(Self::SmsgQuestgiverQuestInvalid),
            0x0190 => Ok(Self::CmsgQuestgiverCancel),
            0x0191 => Ok(Self::SmsgQuestgiverQuestComplete),
            0x0194 => Ok(Self::CmsgQuestlogRemoveQuest),
            0x0195 => Ok(Self::SmsgQuestlogFull),
            0x0198 => Ok(Self::SmsgQuestUpdateComplete),
            0x0199 => Ok(Self::SmsgQuestUpdateAddKill),
            0x019E => Ok(Self::CmsgListInventory),
            0x019F => Ok(Self::SmsgListInventory),
            0x01A0 => Ok(Self::CmsgSellItem),
            0x01A1 => Ok(Self::SmsgSellItem),
            0x01A2 => Ok(Self::CmsgBuyItem),
            0x01A3 => Ok(Self::CmsgBuyItemInSlot),
            0x01A4 => Ok(Self::SmsgBuyItem),
            0x01A5 => Ok(Self::SmsgBuyFailed),
            0x01A9 => Ok(Self::SmsgShowTaxiNodes),
            0x01AA => Ok(Self::CmsgTaxiNodeStatusQuery),
            0x01AB => Ok(Self::SmsgTaxiNodeStatus),
            0x01AC => Ok(Self::CmsgTaxiQueryAvailableNodes),
            0x01AD => Ok(Self::CmsgActivateTaxi),
            0x01AE => Ok(Self::SmsgActivateTaxiReply),
            0x01AF => Ok(Self::SmsgNewTaxiPath),
            0x01B0 => Ok(Self::CmsgTrainerList),
            0x01B1 => Ok(Self::SmsgTrainerList),
            0x01B2 => Ok(Self::CmsgTrainerBuySpell),
            0x01B3 => Ok(Self::SmsgTrainerBuySucceeded),
            0x01B4 => Ok(Self::SmsgTrainerBuyFailed),
            0x01B5 => Ok(Self::CmsgBinderActivate),
            0x01B7 => Ok(Self::CmsgBankerActivate),
            0x01B8 => Ok(Self::SmsgShowBank),
            0x01B9 => Ok(Self::CmsgBuyBankSlot),
            0x01BA => Ok(Self::SmsgBuyBankSlotResult),
            0x01CE => Ok(Self::CmsgQueryTime),
            0x01CF => Ok(Self::SmsgQueryTimeResponse),
            0x01D0 => Ok(Self::SmsgLogXpGain),
            0x01D2 => Ok(Self::CmsgReclaimCorpse),
            0x01D4 => Ok(Self::SmsgLevelupInfo),
            0x01D9 => Ok(Self::SmsgStartMirrorTimer),
            0x01DB => Ok(Self::SmsgStopMirrorTimer),
            0x01DC => Ok(Self::CmsgPing),
            0x01DD => Ok(Self::SmsgPong),
            0x01E2 => Ok(Self::SmsgSpellDelayed),
            0x01EC => Ok(Self::SmsgAuthChallenge),
            0x01ED => Ok(Self::CmsgAuthSession),
            0x01EE => Ok(Self::SmsgAuthResponse),
            0x01F3 => Ok(Self::SmsgPlaySpellVisual),
            0x01F4 => Ok(Self::CmsgZoneUpdate),
            0x01F7 => Ok(Self::SmsgPlaySpellImpact),
            0x01F8 => Ok(Self::SmsgExplorationExperience),
            0x01FC => Ok(Self::SmsgEnvironmentalDamageLog),
            0x0209 => Ok(Self::SmsgAccountDataTimes),
            0x020A => Ok(Self::CmsgRequestAccountData),
            0x020B => Ok(Self::CmsgUpdateAccountData),
            0x020C => Ok(Self::SmsgUpdateAccountData),
            0x0211 => Ok(Self::CmsgGmTicketGetTicket),
            0x0212 => Ok(Self::SmsgGmTicketGetTicket),
            0x0216 => Ok(Self::MsgCorpseQuery),
            0x021C => Ok(Self::CmsgSpiritHealerActivate),
            0x021E => Ok(Self::SmsgSetRestStart),
            0x0236 => Ok(Self::SmsgLoginVerifyWorld),
            0x0238 => Ok(Self::CmsgSendMail),
            0x0239 => Ok(Self::SmsgSendMailResult),
            0x023A => Ok(Self::CmsgGetMailList),
            0x023B => Ok(Self::SmsgMailListResult),
            0x0243 => Ok(Self::CmsgItemTextQuery),
            0x0244 => Ok(Self::SmsgItemTextQueryResponse),
            0x0245 => Ok(Self::CmsgMailTakeMoney),
            0x0246 => Ok(Self::CmsgMailTakeItem),
            0x0247 => Ok(Self::CmsgMailMarkAsRead),
            0x0248 => Ok(Self::CmsgMailReturnToSender),
            0x0249 => Ok(Self::CmsgMailDelete),
            0x024A => Ok(Self::CmsgMailCreateTextItem),
            0x024B => Ok(Self::SmsgSpellLogMiss),
            0x024E => Ok(Self::SmsgPeriodicAuraLog),
            0x0250 => Ok(Self::SmsgSpellNonMeleeDamageLog),
            0x0255 => Ok(Self::MsgAuctionHello),
            0x0256 => Ok(Self::CmsgAuctionSellItem),
            0x0257 => Ok(Self::CmsgAuctionRemoveItem),
            0x0258 => Ok(Self::CmsgAuctionListItems),
            0x0259 => Ok(Self::CmsgAuctionListOwnerItems),
            0x025A => Ok(Self::CmsgAuctionPlaceBid),
            0x025B => Ok(Self::SmsgAuctionCommandResult),
            0x025C => Ok(Self::SmsgAuctionListResult),
            0x025D => Ok(Self::SmsgAuctionOwnerListResult),
            0x025E => Ok(Self::SmsgAuctionBidderNotification),
            0x025F => Ok(Self::SmsgAuctionOwnerNotification),
            0x0262 => Ok(Self::SmsgDispelFailed),
            0x0264 => Ok(Self::CmsgAuctionListBidderItems),
            0x0265 => Ok(Self::SmsgAuctionBidderListResult),
            0x0268 => Ok(Self::CmsgSetAmmo),
            0x0269 => Ok(Self::SmsgCorpseReclaimDelay),
            0x026A => Ok(Self::CmsgSetActiveMover),
            0x026D => Ok(Self::CmsgCancelAutoRepeatSpell),
            0x027B => Ok(Self::SmsgSpellDispelLog),
            0x027E => Ok(Self::CmsgGroupChangeSubGroup),
            0x027F => Ok(Self::CmsgRequestPartyMemberStats),
            0x0282 => Ok(Self::CmsgAutostoreBankItem),
            0x0283 => Ok(Self::CmsgAutobankItem),
            0x0284 => Ok(Self::MsgQueryNextMailTime),
            0x0285 => Ok(Self::SmsgReceivedMail),
            0x028D => Ok(Self::SmsgAuctionRemovedNotification),
            0x028E => Ok(Self::CmsgGroupRaidConvert),
            0x028F => Ok(Self::CmsgGroupAssistantLeader),
            0x0290 => Ok(Self::CmsgBuybackItem),
            0x0296 => Ok(Self::CmsgMeetingStoneInfo),
            0x029D => Ok(Self::SmsgStandStateUpdate),
            0x029E => Ok(Self::SmsgLootAllPassed),
            0x029F => Ok(Self::SmsgLootRollWon),
            0x02A0 => Ok(Self::CmsgLootRoll),
            0x02A1 => Ok(Self::SmsgLootStartRoll),
            0x02A2 => Ok(Self::SmsgLootRoll),
            0x02A3 => Ok(Self::CmsgLootMasterGive),
            0x02A4 => Ok(Self::SmsgLootMasterList),
            0x02A6 => Ok(Self::SmsgSpellFailedOther),
            0x02C2 => Ok(Self::SmsgInitWorldStates),
            0x02C4 => Ok(Self::CmsgItemNameQuery),
            0x02C5 => Ok(Self::SmsgItemNameQueryResponse),
            0x02C9 => Ok(Self::CmsgMoveSplineDone),
            0x02CA => Ok(Self::CmsgMoveFallReset),
            0x02CD => Ok(Self::CmsgRequestRaidInfo),
            0x02CE => Ok(Self::CmsgMoveTimeSkipped),
            0x02D3 => Ok(Self::CmsgBattlefieldStatus),
            0x02F2 => Ok(Self::SmsgPartyMemberStatsFull),
            0x02EB => Ok(Self::SmsgBinderConfirm),
            0x02FE => Ok(Self::SmsgSplineSetRunSpeed),
            0x02FF => Ok(Self::SmsgSplineSetRunBackSpeed),
            0x0300 => Ok(Self::SmsgSplineSetSwimSpeed),
            0x0301 => Ok(Self::SmsgSplineSetWalkSpeed),
            0x0302 => Ok(Self::SmsgSplineSetSwimBackSpeed),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unknown world opcode: 0x{value:04X}"),
            )),
        }
    }
}

impl From<WorldOpcode> for u32 {
    fn from(value: WorldOpcode) -> Self {
        value as u32
    }
}

impl WorldOpcode {
    pub fn is_server_only(self) -> bool {
        matches!(
            self,
            Self::SmsgCharCreate
                | Self::SmsgCharEnum
                | Self::SmsgCharDelete
                | Self::SmsgCharacterLoginFailed
                | Self::SmsgLoginSetTimeSpeed
                | Self::SmsgLogoutResponse
                | Self::SmsgLogoutComplete
                | Self::SmsgLogoutCancelAck
                | Self::SmsgNameQueryResponse
                | Self::SmsgItemQuerySingleResponse
                | Self::SmsgPageTextQueryResponse
                | Self::SmsgQuestQueryResponse
                | Self::SmsgGameObjectQueryResponse
                | Self::SmsgCreatureQueryResponse
                | Self::SmsgGroupInvite
                | Self::SmsgGroupDecline
                | Self::SmsgGroupUninvite
                | Self::SmsgGroupSetLeader
                | Self::SmsgGroupDestroyed
                | Self::SmsgGroupList
                | Self::SmsgPartyCommandResult
                | Self::SmsgMessageChat
                | Self::SmsgChannelNotify
                | Self::SmsgUpdateObject
                | Self::SmsgDestroyObject
                | Self::SmsgReadItemOk
                | Self::SmsgReadItemFailed
                | Self::SmsgItemCooldown
                | Self::SmsgMonsterMove
                | Self::SmsgForceRunSpeedChange
                | Self::SmsgForceMoveRoot
                | Self::SmsgForceMoveUnroot
                | Self::SmsgTriggerCinematic
                | Self::SmsgTutorialFlags
                | Self::SmsgEmote
                | Self::SmsgTextEmote
                | Self::SmsgInventoryChangeFailure
                | Self::SmsgInitializeFactions
                | Self::SmsgSetFactionVisible
                | Self::SmsgSetFactionStanding
                | Self::SmsgSetProficiency
                | Self::SmsgActionButtons
                | Self::SmsgInitialSpells
                | Self::SmsgLearnedSpell
                | Self::SmsgCastResult
                | Self::SmsgSpellStart
                | Self::SmsgSpellGo
                | Self::SmsgSpellFailure
                | Self::SmsgSpellCooldown
                | Self::SmsgCooldownEvent
                | Self::SmsgUpdateAuraDuration
                | Self::MsgChannelStart
                | Self::MsgChannelUpdate
                | Self::SmsgAttackStart
                | Self::SmsgAttackStop
                | Self::SmsgAttackSwingNotInRange
                | Self::SmsgAttackSwingBadFacing
                | Self::SmsgAttackSwingDeadTarget
                | Self::SmsgAttackSwingCantAttack
                | Self::SmsgAttackerStateUpdate
                | Self::SmsgSpellHealLog
                | Self::SmsgSpellEnergizeLog
                | Self::SmsgBindpointUpdate
                | Self::SmsgPlayerBound
                | Self::SmsgLootResponse
                | Self::SmsgLootReleaseResponse
                | Self::SmsgLootRemoved
                | Self::SmsgLootMoneyNotify
                | Self::SmsgLootClearMoney
                | Self::SmsgItemPushResult
                | Self::SmsgGossipMessage
                | Self::SmsgGossipComplete
                | Self::SmsgNpcTextUpdate
                | Self::SmsgQuestgiverStatus
                | Self::SmsgQuestgiverQuestList
                | Self::SmsgQuestgiverQuestDetails
                | Self::SmsgQuestgiverRequestItems
                | Self::SmsgQuestgiverOfferReward
                | Self::SmsgQuestgiverQuestInvalid
                | Self::SmsgQuestgiverQuestComplete
                | Self::SmsgQuestlogFull
                | Self::SmsgQuestUpdateComplete
                | Self::SmsgQuestUpdateAddKill
                | Self::SmsgListInventory
                | Self::SmsgSellItem
                | Self::SmsgBuyItem
                | Self::SmsgBuyFailed
                | Self::SmsgShowTaxiNodes
                | Self::SmsgTaxiNodeStatus
                | Self::SmsgActivateTaxiReply
                | Self::SmsgNewTaxiPath
                | Self::SmsgTrainerList
                | Self::SmsgTrainerBuySucceeded
                | Self::SmsgTrainerBuyFailed
                | Self::SmsgBinderConfirm
                | Self::SmsgShowBank
                | Self::SmsgBuyBankSlotResult
                | Self::SmsgQueryTimeResponse
                | Self::SmsgLogXpGain
                | Self::SmsgLevelupInfo
                | Self::SmsgStartMirrorTimer
                | Self::SmsgStopMirrorTimer
                | Self::SmsgPong
                | Self::SmsgSpellDelayed
                | Self::SmsgAuthChallenge
                | Self::SmsgAuthResponse
                | Self::SmsgPlaySpellVisual
                | Self::SmsgPlaySpellImpact
                | Self::SmsgExplorationExperience
                | Self::SmsgEnvironmentalDamageLog
                | Self::SmsgAccountDataTimes
                | Self::SmsgUpdateAccountData
                | Self::SmsgGmTicketGetTicket
                | Self::SmsgSetRestStart
                | Self::SmsgLoginVerifyWorld
                | Self::SmsgSendMailResult
                | Self::SmsgMailListResult
                | Self::SmsgItemTextQueryResponse
                | Self::SmsgSpellLogMiss
                | Self::SmsgPeriodicAuraLog
                | Self::SmsgSpellNonMeleeDamageLog
                | Self::MsgAuctionHello
                | Self::SmsgAuctionCommandResult
                | Self::SmsgAuctionListResult
                | Self::SmsgAuctionOwnerListResult
                | Self::SmsgAuctionBidderNotification
                | Self::SmsgAuctionOwnerNotification
                | Self::SmsgDispelFailed
                | Self::SmsgAuctionBidderListResult
                | Self::SmsgCorpseReclaimDelay
                | Self::SmsgSpellDispelLog
                | Self::SmsgReceivedMail
                | Self::SmsgAuctionRemovedNotification
                | Self::SmsgStandStateUpdate
                | Self::SmsgLootAllPassed
                | Self::SmsgLootRollWon
                | Self::SmsgLootStartRoll
                | Self::SmsgLootRoll
                | Self::SmsgLootMasterList
                | Self::SmsgSpellFailedOther
                | Self::SmsgInitWorldStates
                | Self::SmsgItemNameQueryResponse
                | Self::SmsgPartyMemberStatsFull
                | Self::SmsgSplineSetRunSpeed
                | Self::SmsgSplineSetRunBackSpeed
                | Self::SmsgSplineSetSwimSpeed
                | Self::SmsgSplineSetWalkSpeed
                | Self::SmsgSplineSetSwimBackSpeed
        )
    }
}

fn ensure_remaining(buf: &impl Buf, size: usize, packet_name: &str) -> io::Result<()> {
    if buf.remaining() < size {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!("{packet_name} payload too short"),
        ));
    }
    Ok(())
}

fn ensure_exact_remaining(buf: &impl Buf, size: usize, packet_name: &str) -> io::Result<()> {
    if buf.remaining() != size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{packet_name} payload must be {size} bytes"),
        ));
    }
    Ok(())
}

fn read_u32_request(buf: &mut impl Buf, packet_name: &str) -> io::Result<u32> {
    ensure_remaining(buf, 4, packet_name)?;
    Ok(buf.get_u32_le())
}

fn read_guid_request(buf: &mut impl Buf, packet_name: &str) -> io::Result<u64> {
    ensure_remaining(buf, 8, packet_name)?;
    Ok(buf.get_u64_le())
}

fn read_packed_guid_request(buf: &mut impl Buf, packet_name: &str) -> io::Result<ObjectGuid> {
    ensure_remaining(buf, 1, packet_name)?;
    let mask = buf.get_u8();
    let mut value = 0u64;
    for i in 0..8u8 {
        if mask & (1 << i) != 0 {
            ensure_remaining(buf, 1, packet_name)?;
            value |= (buf.get_u8() as u64) << (i * 8);
        }
    }
    Ok(ObjectGuid::from_raw(value))
}

fn write_packed_guid(buf: &mut impl BufMut, guid: ObjectGuid) -> io::Result<()> {
    let mut packed = Vec::with_capacity(PackedGuid::packed_size(guid));
    PackedGuid::write(&mut packed, guid)?;
    buf.put_slice(&packed);
    Ok(())
}

fn read_c_string_request(buf: &mut impl Buf, packet_name: &str) -> io::Result<String> {
    let mut bytes = Vec::new();
    while buf.has_remaining() {
        let byte = buf.get_u8();
        if byte == 0 {
            return String::from_utf8(bytes)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error));
        }
        bytes.push(byte);
    }
    Err(io::Error::new(
        io::ErrorKind::UnexpectedEof,
        format!("{packet_name} string is not NUL-terminated"),
    ))
}

fn write_c_string(buf: &mut impl BufMut, value: &str) {
    buf.put_slice(value.as_bytes());
    buf.put_u8(0);
}

fn put_packed_guid(buf: &mut impl BufMut, guid: ObjectGuid) {
    let value = guid.raw();
    let mut mask = 0u8;
    let mut bytes = [0u8; 8];
    let mut count = 0usize;
    for i in 0..8u8 {
        let byte = ((value >> (i * 8)) & 0xFF) as u8;
        if byte != 0 {
            mask |= 1 << i;
            bytes[count] = byte;
            count += 1;
        }
    }
    buf.put_u8(mask);
    buf.put_slice(&bytes[..count]);
}

pub trait ServerWorldPacket {
    const OPCODE: WorldOpcode;

    fn write_body(&self, buf: &mut impl BufMut);

    fn body(&self) -> Vec<u8> {
        let mut body = Vec::new();
        self.write_body(&mut body);
        body
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AreaTriggerRequest {
    pub trigger_id: u32,
}

impl AreaTriggerRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        if buf.remaining() < 4 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "CMSG_AREATRIGGER missing trigger id",
            ));
        }
        Ok(Self {
            trigger_id: buf.get_u32_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.trigger_id);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZoneUpdateRequest {
    pub zone_id: u32,
}

impl ZoneUpdateRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        if buf.remaining() < 4 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "CMSG_ZONEUPDATE missing zone id",
            ));
        }
        Ok(Self {
            zone_id: buf.get_u32_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.zone_id);
    }
}

macro_rules! empty_request {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
        pub struct $name;

        impl $name {
            pub fn read(_buf: &mut impl Buf) -> io::Result<Self> {
                Ok(Self)
            }

            pub fn write(&self, _buf: &mut impl BufMut) {}
        }
    };
}

macro_rules! guid_request {
    ($name:ident, $packet_name:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name {
            pub raw_guid: u64,
        }

        impl $name {
            pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
                Ok(Self {
                    raw_guid: read_guid_request(buf, $packet_name)?,
                })
            }

            pub fn write(&self, buf: &mut impl BufMut) {
                buf.put_u64_le(self.raw_guid);
            }
        }
    };
}

empty_request!(QueryTimeRequest);
empty_request!(TutorialClearRequest);
empty_request!(TutorialResetRequest);
empty_request!(GmTicketGetTicketRequest);
empty_request!(QueryNextMailTimeRequest);
empty_request!(LogoutRequest);
empty_request!(LogoutCancelRequest);
empty_request!(PlayerLogoutRequest);
empty_request!(GroupCancelRequest);
empty_request!(GroupAcceptRequest);
empty_request!(GroupDeclineRequest);
empty_request!(GroupRaidConvertRequest);
empty_request!(GroupDisbandRequest);
empty_request!(CharEnumRequest);
empty_request!(AttackStopRequest);
empty_request!(CancelCastRequest);
empty_request!(CancelAutoRepeatSpellRequest);
empty_request!(QuestgiverCancelRequest);
empty_request!(RepopRequest);
empty_request!(CorpseQueryRequest);
empty_request!(LootMoneyRequest);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendMailRequest {
    pub mailbox_raw_guid: u64,
    pub receiver: String,
    pub subject: String,
    pub body: String,
    pub stationery: u32,
    pub unknown1: u32,
    pub item_raw_guid: u64,
    pub money: u32,
    pub cod: u32,
    pub unknown2: u64,
    pub unknown3: u8,
}

impl SendMailRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 8, "CMSG_SEND_MAIL")?;
        let mailbox_raw_guid = buf.get_u64_le();
        let receiver = read_c_string_request(buf, "CMSG_SEND_MAIL")?;
        let subject = read_c_string_request(buf, "CMSG_SEND_MAIL")?;
        let body = read_c_string_request(buf, "CMSG_SEND_MAIL")?;
        ensure_remaining(buf, 29, "CMSG_SEND_MAIL")?;
        Ok(Self {
            mailbox_raw_guid,
            receiver,
            subject,
            body,
            stationery: buf.get_u32_le(),
            unknown1: buf.get_u32_le(),
            item_raw_guid: buf.get_u64_le(),
            money: buf.get_u32_le(),
            cod: buf.get_u32_le(),
            unknown2: buf.get_u64_le(),
            unknown3: buf.get_u8(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.mailbox_raw_guid);
        write_c_string(buf, &self.receiver);
        write_c_string(buf, &self.subject);
        write_c_string(buf, &self.body);
        buf.put_u32_le(self.stationery);
        buf.put_u32_le(self.unknown1);
        buf.put_u64_le(self.item_raw_guid);
        buf.put_u32_le(self.money);
        buf.put_u32_le(self.cod);
        buf.put_u64_le(self.unknown2);
        buf.put_u8(self.unknown3);
    }
}

guid_request!(GetMailListRequest, "CMSG_GET_MAIL_LIST");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MailIdRequest {
    pub mailbox_raw_guid: u64,
    pub mail_id: u32,
}

impl MailIdRequest {
    pub fn read(buf: &mut impl Buf, packet_name: &str) -> io::Result<Self> {
        ensure_exact_remaining(buf, 12, packet_name)?;
        Ok(Self {
            mailbox_raw_guid: buf.get_u64_le(),
            mail_id: buf.get_u32_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.mailbox_raw_guid);
        buf.put_u32_le(self.mail_id);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MailCreateTextItemRequest {
    pub mailbox_raw_guid: u64,
    pub mail_id: u32,
    pub mail_template_id: u32,
}

impl MailCreateTextItemRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_exact_remaining(buf, 16, "CMSG_MAIL_CREATE_TEXT_ITEM")?;
        Ok(Self {
            mailbox_raw_guid: buf.get_u64_le(),
            mail_id: buf.get_u32_le(),
            mail_template_id: buf.get_u32_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.mailbox_raw_guid);
        buf.put_u32_le(self.mail_id);
        buf.put_u32_le(self.mail_template_id);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuctionHelloRequest {
    pub auctioneer_raw_guid: u64,
}

impl AuctionHelloRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_exact_remaining(buf, 8, "CMSG_AUCTION_HELLO")?;
        Ok(Self {
            auctioneer_raw_guid: buf.get_u64_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.auctioneer_raw_guid);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuctionSellItemRequest {
    pub auctioneer_raw_guid: u64,
    pub item_raw_guid: u64,
    pub bid: u32,
    pub buyout: u32,
    pub duration_minutes: u32,
}

impl AuctionSellItemRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_exact_remaining(buf, 28, "CMSG_AUCTION_SELL_ITEM")?;
        Ok(Self {
            auctioneer_raw_guid: buf.get_u64_le(),
            item_raw_guid: buf.get_u64_le(),
            bid: buf.get_u32_le(),
            buyout: buf.get_u32_le(),
            duration_minutes: buf.get_u32_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.auctioneer_raw_guid);
        buf.put_u64_le(self.item_raw_guid);
        buf.put_u32_le(self.bid);
        buf.put_u32_le(self.buyout);
        buf.put_u32_le(self.duration_minutes);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuctionRemoveItemRequest {
    pub auctioneer_raw_guid: u64,
    pub auction_id: u32,
}

impl AuctionRemoveItemRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_exact_remaining(buf, 12, "CMSG_AUCTION_REMOVE_ITEM")?;
        Ok(Self {
            auctioneer_raw_guid: buf.get_u64_le(),
            auction_id: buf.get_u32_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.auctioneer_raw_guid);
        buf.put_u32_le(self.auction_id);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuctionPlaceBidRequest {
    pub auctioneer_raw_guid: u64,
    pub auction_id: u32,
    pub price: u32,
}

impl AuctionPlaceBidRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_exact_remaining(buf, 16, "CMSG_AUCTION_PLACE_BID")?;
        Ok(Self {
            auctioneer_raw_guid: buf.get_u64_le(),
            auction_id: buf.get_u32_le(),
            price: buf.get_u32_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.auctioneer_raw_guid);
        buf.put_u32_le(self.auction_id);
        buf.put_u32_le(self.price);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuctionListItemsRequest {
    pub auctioneer_raw_guid: u64,
    pub list_from: u32,
    pub searched_name: String,
    pub level_min: u8,
    pub level_max: u8,
    pub inventory_type: u32,
    pub item_class: u32,
    pub item_subclass: u32,
    pub quality: u32,
    pub usable: u8,
}

impl AuctionListItemsRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 8, "CMSG_AUCTION_LIST_ITEMS")?;
        let auctioneer_raw_guid = buf.get_u64_le();
        ensure_remaining(buf, 4, "CMSG_AUCTION_LIST_ITEMS")?;
        let list_from = buf.get_u32_le();
        let searched_name = read_c_string_request(buf, "CMSG_AUCTION_LIST_ITEMS")?;
        ensure_exact_remaining(buf, 19, "CMSG_AUCTION_LIST_ITEMS")?;
        Ok(Self {
            auctioneer_raw_guid,
            list_from,
            searched_name,
            level_min: buf.get_u8(),
            level_max: buf.get_u8(),
            inventory_type: buf.get_u32_le(),
            item_class: buf.get_u32_le(),
            item_subclass: buf.get_u32_le(),
            quality: buf.get_u32_le(),
            usable: buf.get_u8(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.auctioneer_raw_guid);
        buf.put_u32_le(self.list_from);
        write_c_string(buf, &self.searched_name);
        buf.put_u8(self.level_min);
        buf.put_u8(self.level_max);
        buf.put_u32_le(self.inventory_type);
        buf.put_u32_le(self.item_class);
        buf.put_u32_le(self.item_subclass);
        buf.put_u32_le(self.quality);
        buf.put_u8(self.usable);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuctionListOwnerItemsRequest {
    pub auctioneer_raw_guid: u64,
    pub list_from: u32,
}

impl AuctionListOwnerItemsRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_exact_remaining(buf, 12, "CMSG_AUCTION_LIST_OWNER_ITEMS")?;
        Ok(Self {
            auctioneer_raw_guid: buf.get_u64_le(),
            list_from: buf.get_u32_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.auctioneer_raw_guid);
        buf.put_u32_le(self.list_from);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuctionListBidderItemsRequest {
    pub auctioneer_raw_guid: u64,
    pub list_from: u32,
    pub outbid_auction_ids: Vec<u32>,
}

impl AuctionListBidderItemsRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 16, "CMSG_AUCTION_LIST_BIDDER_ITEMS")?;
        let auctioneer_raw_guid = buf.get_u64_le();
        let list_from = buf.get_u32_le();
        let outbid_count = buf.get_u32_le() as usize;
        ensure_exact_remaining(
            buf,
            outbid_count * std::mem::size_of::<u32>(),
            "CMSG_AUCTION_LIST_BIDDER_ITEMS",
        )?;
        let mut outbid_auction_ids = Vec::with_capacity(outbid_count);
        for _ in 0..outbid_count {
            outbid_auction_ids.push(buf.get_u32_le());
        }
        Ok(Self {
            auctioneer_raw_guid,
            list_from,
            outbid_auction_ids,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.auctioneer_raw_guid);
        buf.put_u32_le(self.list_from);
        buf.put_u32_le(self.outbid_auction_ids.len() as u32);
        for auction_id in &self.outbid_auction_ids {
            buf.put_u32_le(*auction_id);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgAuctionHelloResponse {
    pub auctioneer: ObjectGuid,
    pub house_id: u32,
}

impl ServerWorldPacket for SmsgAuctionHelloResponse {
    const OPCODE: WorldOpcode = WorldOpcode::MsgAuctionHello;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.auctioneer.raw());
        buf.put_u32_le(self.house_id);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgAuctionCommandResultResponse {
    pub auction_id: u32,
    pub action: u32,
    pub error_code: u32,
    pub bid_min_outbid: Option<u32>,
    pub inventory_error: Option<u32>,
    pub higher_bidder: Option<ObjectGuid>,
    pub higher_bid: Option<u32>,
    pub higher_min_outbid: Option<u32>,
}

impl ServerWorldPacket for SmsgAuctionCommandResultResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgAuctionCommandResult;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.auction_id);
        buf.put_u32_le(self.action);
        buf.put_u32_le(self.error_code);
        match (self.action, self.error_code) {
            (2, 0) => buf.put_u32_le(self.bid_min_outbid.unwrap_or(0)),
            (_, 1) => buf.put_u32_le(self.inventory_error.unwrap_or(0)),
            (_, 5) => {
                buf.put_u64_le(self.higher_bidder.unwrap_or(ObjectGuid::from_raw(0)).raw());
                buf.put_u32_le(self.higher_bid.unwrap_or(0));
                buf.put_u32_le(self.higher_min_outbid.unwrap_or(0));
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgAuctionBidderNotificationResponse {
    pub house_id: u32,
    pub auction_id: u32,
    pub bidder: ObjectGuid,
    pub bid_or_zero_if_won: u32,
    pub min_outbid: u32,
    pub item_template: u32,
    pub item_random_property_id: i32,
}

impl ServerWorldPacket for SmsgAuctionBidderNotificationResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgAuctionBidderNotification;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.house_id);
        buf.put_u32_le(self.auction_id);
        buf.put_u64_le(self.bidder.raw());
        buf.put_u32_le(self.bid_or_zero_if_won);
        buf.put_u32_le(self.min_outbid);
        buf.put_u32_le(self.item_template);
        buf.put_i32_le(self.item_random_property_id);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgAuctionOwnerNotificationResponse {
    pub auction_id: u32,
    pub bid: u32,
    pub min_outbid: u32,
    pub bidder: ObjectGuid,
    pub item_template: u32,
    pub item_random_property_id: i32,
}

impl ServerWorldPacket for SmsgAuctionOwnerNotificationResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgAuctionOwnerNotification;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.auction_id);
        buf.put_u32_le(self.bid);
        buf.put_u32_le(self.min_outbid);
        buf.put_u64_le(self.bidder.raw());
        buf.put_u32_le(self.item_template);
        buf.put_i32_le(self.item_random_property_id);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgAuctionRemovedNotificationResponse {
    pub auction_id: u32,
    pub item_template: u32,
    pub item_random_property_id: i32,
}

impl ServerWorldPacket for SmsgAuctionRemovedNotificationResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgAuctionRemovedNotification;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.auction_id);
        buf.put_u32_le(self.item_template);
        buf.put_i32_le(self.item_random_property_id);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuctionInfoResponse {
    pub id: u32,
    pub item: u32,
    pub enchantment: u32,
    pub random_property_id: u32,
    pub suffix_factor: u32,
    pub count: u32,
    pub charges: u32,
    pub owner: ObjectGuid,
    pub start_bid: u32,
    pub min_outbid: u32,
    pub buyout: u32,
    pub time_left_millis: u32,
    pub bidder: ObjectGuid,
    pub current_bid: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgAuctionListResultResponse {
    pub auctions: Vec<AuctionInfoResponse>,
    pub total_count: u32,
}

impl ServerWorldPacket for SmsgAuctionListResultResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgAuctionListResult;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.auctions.len() as u32);
        for auction in &self.auctions {
            write_auction_info(buf, auction);
        }
        buf.put_u32_le(self.total_count);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgAuctionOwnerListResultResponse {
    pub auctions: Vec<AuctionInfoResponse>,
    pub total_count: u32,
}

impl ServerWorldPacket for SmsgAuctionOwnerListResultResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgAuctionOwnerListResult;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.auctions.len() as u32);
        for auction in &self.auctions {
            write_auction_info(buf, auction);
        }
        buf.put_u32_le(self.total_count);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgAuctionBidderListResultResponse {
    pub auctions: Vec<AuctionInfoResponse>,
    pub total_count: u32,
}

impl ServerWorldPacket for SmsgAuctionBidderListResultResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgAuctionBidderListResult;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.auctions.len() as u32);
        for auction in &self.auctions {
            write_auction_info(buf, auction);
        }
        buf.put_u32_le(self.total_count);
    }
}

fn write_auction_info(buf: &mut impl BufMut, auction: &AuctionInfoResponse) {
    buf.put_u32_le(auction.id);
    buf.put_u32_le(auction.item);
    buf.put_u32_le(auction.enchantment);
    buf.put_u32_le(auction.random_property_id);
    buf.put_u32_le(auction.suffix_factor);
    buf.put_u32_le(auction.count);
    buf.put_u32_le(auction.charges);
    buf.put_u64_le(auction.owner.raw());
    buf.put_u32_le(auction.start_bid);
    buf.put_u32_le(auction.min_outbid);
    buf.put_u32_le(auction.buyout);
    buf.put_u32_le(auction.time_left_millis);
    buf.put_u64_le(auction.bidder.raw());
    buf.put_u32_le(auction.current_bid);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemTextQueryRequest {
    pub item_text_id: u32,
    pub mail_id_or_item_guid: u32,
    pub unknown: u32,
}

impl ItemTextQueryRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_exact_remaining(buf, 12, "CMSG_ITEM_TEXT_QUERY")?;
        Ok(Self {
            item_text_id: buf.get_u32_le(),
            mail_id_or_item_guid: buf.get_u32_le(),
            unknown: buf.get_u32_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.item_text_id);
        buf.put_u32_le(self.mail_id_or_item_guid);
        buf.put_u32_le(self.unknown);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetAmmoRequest {
    pub item: u32,
}

impl SetAmmoRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 4, "CMSG_SET_AMMO")?;
        Ok(Self {
            item: buf.get_u32_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.item);
    }
}

guid_request!(CharDeleteRequest, "CMSG_CHAR_DELETE");
guid_request!(PlayerLoginRequest, "CMSG_PLAYER_LOGIN");
guid_request!(GameObjectUseRequest, "CMSG_GAMEOBJ_USE");
guid_request!(GossipHelloRequest, "CMSG_GOSSIP_HELLO");
guid_request!(QuestgiverStatusQueryRequest, "CMSG_QUESTGIVER_STATUS_QUERY");
guid_request!(QuestgiverHelloRequest, "CMSG_QUESTGIVER_HELLO");
guid_request!(ListInventoryRequest, "CMSG_LIST_INVENTORY");
guid_request!(TaxiNodeStatusQueryRequest, "CMSG_TAXINODE_STATUS_QUERY");
guid_request!(
    TaxiQueryAvailableNodesRequest,
    "CMSG_TAXIQUERYAVAILABLENODES"
);
guid_request!(BinderActivateRequest, "CMSG_BINDER_ACTIVATE");
guid_request!(TrainerListRequest, "CMSG_TRAINER_LIST");
guid_request!(GroupUninviteGuidRequest, "CMSG_GROUP_UNINVITE_GUID");
guid_request!(GroupSetLeaderRequest, "CMSG_GROUP_SET_LEADER");
guid_request!(AttackSwingRequest, "CMSG_ATTACKSWING");
guid_request!(LootRequest, "CMSG_LOOT");
guid_request!(LootReleaseRequest, "CMSG_LOOT_RELEASE");
guid_request!(SpiritHealerActivateRequest, "CMSG_SPIRIT_HEALER_ACTIVATE");
guid_request!(
    RequestPartyMemberStatsRequest,
    "CMSG_REQUEST_PARTY_MEMBER_STATS"
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadItemRequest {
    pub bag: u8,
    pub slot: u8,
}

impl ReadItemRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 2, "CMSG_READ_ITEM")?;
        Ok(Self {
            bag: buf.get_u8(),
            slot: buf.get_u8(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.bag);
        buf.put_u8(self.slot);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageTextQueryRequest {
    pub page_text_id: u32,
    pub item_raw_guid: u64,
}

impl PageTextQueryRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 12, "CMSG_PAGE_TEXT_QUERY")?;
        Ok(Self {
            page_text_id: buf.get_u32_le(),
            item_raw_guid: buf.get_u64_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.page_text_id);
        buf.put_u64_le(self.item_raw_guid);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoveTeleportAckRequest {
    pub player: ObjectGuid,
    pub counter: u32,
    pub client_time: u32,
}

impl MoveTeleportAckRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        let player = read_packed_guid_request(buf, "MSG_MOVE_TELEPORT_ACK")?;
        ensure_remaining(buf, 8, "MSG_MOVE_TELEPORT_ACK")?;
        Ok(Self {
            player,
            counter: buf.get_u32_le(),
            client_time: buf.get_u32_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) -> io::Result<()> {
        write_packed_guid(buf, self.player)?;
        buf.put_u32_le(self.counter);
        buf.put_u32_le(self.client_time);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharCreateRequest {
    pub name: String,
    pub race: u8,
    pub class: u8,
    pub gender: u8,
    pub skin: u8,
    pub face: u8,
    pub hair_style: u8,
    pub hair_color: u8,
    pub facial_hair: u8,
    pub outfit_id: u8,
}

impl CharCreateRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        let name = read_c_string_request(buf, "CMSG_CHAR_CREATE")?;
        ensure_remaining(buf, 9, "CMSG_CHAR_CREATE")?;
        Ok(Self {
            name,
            race: buf.get_u8(),
            class: buf.get_u8(),
            gender: buf.get_u8(),
            skin: buf.get_u8(),
            face: buf.get_u8(),
            hair_style: buf.get_u8(),
            hair_color: buf.get_u8(),
            facial_hair: buf.get_u8(),
            outfit_id: buf.get_u8(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        write_c_string(buf, &self.name);
        buf.put_u8(self.race);
        buf.put_u8(self.class);
        buf.put_u8(self.gender);
        buf.put_u8(self.skin);
        buf.put_u8(self.face);
        buf.put_u8(self.hair_style);
        buf.put_u8(self.hair_color);
        buf.put_u8(self.facial_hair);
        buf.put_u8(self.outfit_id);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageChatRequest {
    pub chat_type: u32,
    pub language: u32,
    pub message: String,
}

impl MessageChatRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 8, "CMSG_MESSAGECHAT")?;
        Ok(Self {
            chat_type: buf.get_u32_le(),
            language: buf.get_u32_le(),
            message: read_c_string_request(buf, "CMSG_MESSAGECHAT")?,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.chat_type);
        buf.put_u32_le(self.language);
        write_c_string(buf, &self.message);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinChannelRequest {
    pub channel_name: String,
    pub password: String,
}

impl JoinChannelRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        let channel_name = read_c_string_request(buf, "CMSG_JOIN_CHANNEL")?;
        let password = if buf.has_remaining() {
            read_c_string_request(buf, "CMSG_JOIN_CHANNEL")?
        } else {
            String::new()
        };
        Ok(Self {
            channel_name,
            password,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        write_c_string(buf, &self.channel_name);
        if !self.password.is_empty() {
            write_c_string(buf, &self.password);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextEmoteRequest {
    pub text_emote: u32,
    pub emote_num: u32,
    pub target_raw_guid: u64,
}

impl TextEmoteRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 16, "CMSG_TEXT_EMOTE")?;
        Ok(Self {
            text_emote: buf.get_u32_le(),
            emote_num: buf.get_u32_le(),
            target_raw_guid: buf.get_u64_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.text_emote);
        buf.put_u32_le(self.emote_num);
        buf.put_u64_le(self.target_raw_guid);
    }
}

pub const SPELL_CAST_TARGET_UNIT: u16 = 0x0002;
pub const SPELL_CAST_TARGET_SOURCE_LOCATION: u16 = 0x0020;
pub const SPELL_CAST_TARGET_DEST_LOCATION: u16 = 0x0040;
pub const SPELL_CAST_TARGET_UNIT_ENEMY: u16 = 0x0080;
pub const SPELL_CAST_TARGET_GAMEOBJECT: u16 = 0x0800;
pub const SPELL_CAST_TARGET_LOCKED: u16 = 0x4000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpellTargetLocation {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpellCastTargets {
    pub target_mask: u16,
    pub unit_target: Option<ObjectGuid>,
    pub gameobject_target: Option<ObjectGuid>,
    pub source_location: Option<SpellTargetLocation>,
    pub destination: Option<SpellTargetLocation>,
}

impl SpellCastTargets {
    pub fn empty() -> Self {
        Self {
            target_mask: 0,
            unit_target: None,
            gameobject_target: None,
            source_location: None,
            destination: None,
        }
    }

    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 2, "SpellCastTargets")?;
        let target_mask = buf.get_u16_le();
        let unit_target =
            if target_mask & (SPELL_CAST_TARGET_UNIT | SPELL_CAST_TARGET_UNIT_ENEMY) != 0 {
                Some(read_packed_guid_request(buf, "SpellCastTargets")?)
            } else {
                None
            };
        let gameobject_target =
            if target_mask & (SPELL_CAST_TARGET_GAMEOBJECT | SPELL_CAST_TARGET_LOCKED) != 0 {
                Some(read_packed_guid_request(buf, "SpellCastTargets")?)
            } else {
                None
            };
        let source_location = if target_mask & SPELL_CAST_TARGET_SOURCE_LOCATION != 0 {
            ensure_remaining(buf, 12, "SpellCastTargets source location")?;
            Some(SpellTargetLocation {
                x: buf.get_f32_le(),
                y: buf.get_f32_le(),
                z: buf.get_f32_le(),
            })
        } else {
            None
        };
        let destination = if target_mask & SPELL_CAST_TARGET_DEST_LOCATION != 0 {
            ensure_remaining(buf, 12, "SpellCastTargets destination")?;
            Some(SpellTargetLocation {
                x: buf.get_f32_le(),
                y: buf.get_f32_le(),
                z: buf.get_f32_le(),
            })
        } else {
            None
        };
        Ok(Self {
            target_mask,
            unit_target,
            gameobject_target,
            source_location,
            destination,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) -> io::Result<()> {
        let target_mask = normalized_spell_cast_target_mask(self);
        buf.put_u16_le(target_mask);
        if target_mask & SPELL_CAST_TARGET_UNIT != 0 {
            write_packed_guid(buf, self.unit_target.unwrap_or(ObjectGuid::EMPTY))?;
        }
        if target_mask & (SPELL_CAST_TARGET_GAMEOBJECT | SPELL_CAST_TARGET_LOCKED) != 0 {
            write_packed_guid(buf, self.gameobject_target.unwrap_or(ObjectGuid::EMPTY))?;
        }
        if target_mask & SPELL_CAST_TARGET_SOURCE_LOCATION != 0 {
            let location = self.source_location.unwrap_or(SpellTargetLocation {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            });
            buf.put_f32_le(location.x);
            buf.put_f32_le(location.y);
            buf.put_f32_le(location.z);
        }
        if target_mask & SPELL_CAST_TARGET_DEST_LOCATION != 0 {
            let location = self.destination.unwrap_or(SpellTargetLocation {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            });
            buf.put_f32_le(location.x);
            buf.put_f32_le(location.y);
            buf.put_f32_le(location.z);
        }
        Ok(())
    }
}

fn normalized_spell_cast_target_mask(targets: &SpellCastTargets) -> u16 {
    if targets.target_mask & SPELL_CAST_TARGET_UNIT_ENEMY != 0 && targets.unit_target.is_some() {
        targets.target_mask | SPELL_CAST_TARGET_UNIT
    } else {
        targets.target_mask
    }
}

impl Default for SpellCastTargets {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CastSpellRequest {
    pub spell_id: u32,
    pub targets: SpellCastTargets,
}

impl CastSpellRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 4, "CMSG_CAST_SPELL")?;
        Ok(Self {
            spell_id: buf.get_u32_le(),
            targets: SpellCastTargets::read(buf)?,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) -> io::Result<()> {
        buf.put_u32_le(self.spell_id);
        self.targets.write(buf)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UseItemRequest {
    pub bag: u8,
    pub slot: u8,
    pub spell_index: u8,
    pub targets: SpellCastTargets,
}

impl UseItemRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 3, "CMSG_USE_ITEM")?;
        let bag = buf.get_u8();
        let slot = buf.get_u8();
        let spell_index = buf.get_u8();
        let targets = if buf.has_remaining() {
            SpellCastTargets::read(buf)?
        } else {
            SpellCastTargets::empty()
        };
        Ok(Self {
            bag,
            slot,
            spell_index,
            targets,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) -> io::Result<()> {
        buf.put_u8(self.bag);
        buf.put_u8(self.slot);
        buf.put_u8(self.spell_index);
        self.targets.write(buf)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryMoveClientRequest {
    AutoEquip {
        src_bag: u8,
        src_slot: u8,
    },
    AutoStoreBag {
        src_bag: u8,
        src_slot: u8,
        dst_bag: u8,
    },
    SwapItem {
        dst_bag: u8,
        dst_slot: u8,
        src_bag: u8,
        src_slot: u8,
    },
    SwapInvItem {
        src_slot: u8,
        dst_slot: u8,
    },
}

impl InventoryMoveClientRequest {
    pub fn read_auto_equip(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 2, "CMSG_AUTOEQUIP_ITEM")?;
        Ok(Self::AutoEquip {
            src_bag: buf.get_u8(),
            src_slot: buf.get_u8(),
        })
    }

    pub fn read_auto_store_bag(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 3, "CMSG_AUTOSTORE_BAG_ITEM")?;
        Ok(Self::AutoStoreBag {
            src_bag: buf.get_u8(),
            src_slot: buf.get_u8(),
            dst_bag: buf.get_u8(),
        })
    }

    pub fn read_swap_item(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 4, "CMSG_SWAP_ITEM")?;
        Ok(Self::SwapItem {
            dst_bag: buf.get_u8(),
            dst_slot: buf.get_u8(),
            src_bag: buf.get_u8(),
            src_slot: buf.get_u8(),
        })
    }

    pub fn read_swap_inv_item(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 2, "CMSG_SWAP_INV_ITEM")?;
        Ok(Self::SwapInvItem {
            src_slot: buf.get_u8(),
            dst_slot: buf.get_u8(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DestroyItemRequest {
    pub bag: u8,
    pub slot: u8,
    pub count: u8,
}

impl DestroyItemRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 6, "CMSG_DESTROYITEM")?;
        Ok(Self {
            bag: buf.get_u8(),
            slot: buf.get_u8(),
            count: buf.get_u8(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.bag);
        buf.put_u8(self.slot);
        buf.put_u8(self.count);
        buf.put_slice(&[0, 0, 0]);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitItemRequest {
    pub src_bag: u8,
    pub src_slot: u8,
    pub dst_bag: u8,
    pub dst_slot: u8,
    pub count: u8,
}

impl SplitItemRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 5, "CMSG_SPLIT_ITEM")?;
        Ok(Self {
            src_bag: buf.get_u8(),
            src_slot: buf.get_u8(),
            dst_bag: buf.get_u8(),
            dst_slot: buf.get_u8(),
            count: buf.get_u8(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.src_bag);
        buf.put_u8(self.src_slot);
        buf.put_u8(self.dst_bag);
        buf.put_u8(self.dst_slot);
        buf.put_u8(self.count);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GossipSelectOptionRequest {
    pub raw_guid: u64,
    pub option: u32,
}

impl GossipSelectOptionRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 12, "CMSG_GOSSIP_SELECT_OPTION")?;
        Ok(Self {
            raw_guid: buf.get_u64_le(),
            option: buf.get_u32_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.raw_guid);
        buf.put_u32_le(self.option);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestgiverQuestRequest {
    pub raw_guid: u64,
    pub quest: u32,
}

impl QuestgiverQuestRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 12, "CMSG_QUESTGIVER_*_QUEST")?;
        Ok(Self {
            raw_guid: buf.get_u64_le(),
            quest: buf.get_u32_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.raw_guid);
        buf.put_u32_le(self.quest);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestRewardRequest {
    pub raw_guid: u64,
    pub quest: u32,
    pub reward: u32,
}

impl QuestRewardRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 16, "CMSG_QUESTGIVER_CHOOSE_REWARD")?;
        Ok(Self {
            raw_guid: buf.get_u64_le(),
            quest: buf.get_u32_le(),
            reward: buf.get_u32_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.raw_guid);
        buf.put_u32_le(self.quest);
        buf.put_u32_le(self.reward);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestLogRemoveQuestRequest {
    pub slot: u8,
}

impl QuestLogRemoveQuestRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 1, "CMSG_QUESTLOG_REMOVE_QUEST")?;
        Ok(Self { slot: buf.get_u8() })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.slot);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SellItemRequest {
    pub vendor_raw_guid: u64,
    pub item_raw_guid: u64,
    pub count: u8,
}

impl SellItemRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 17, "CMSG_SELL_ITEM")?;
        Ok(Self {
            vendor_raw_guid: buf.get_u64_le(),
            item_raw_guid: buf.get_u64_le(),
            count: buf.get_u8(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.vendor_raw_guid);
        buf.put_u64_le(self.item_raw_guid);
        buf.put_u8(self.count);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuybackItemRequest {
    pub vendor_raw_guid: u64,
    pub slot: u8,
}

impl BuybackItemRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 12, "CMSG_BUYBACK_ITEM")?;
        let vendor_raw_guid = buf.get_u64_le();
        let slot = buf.get_u32_le() as u8;
        Ok(Self {
            vendor_raw_guid,
            slot,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.vendor_raw_guid);
        buf.put_u32_le(self.slot as u32);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuyItemRequest {
    pub vendor_raw_guid: u64,
    pub item: u32,
    pub count: u8,
}

impl BuyItemRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 14, "CMSG_BUY_ITEM")?;
        Ok(Self {
            vendor_raw_guid: buf.get_u64_le(),
            item: buf.get_u32_le(),
            count: buf.get_u8(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.vendor_raw_guid);
        buf.put_u32_le(self.item);
        buf.put_u8(self.count);
        buf.put_u8(0);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuyItemInSlotRequest {
    pub vendor_raw_guid: u64,
    pub item: u32,
    pub bag_raw_guid: u64,
    pub bag_slot: u8,
    pub count: u8,
}

impl BuyItemInSlotRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 22, "CMSG_BUY_ITEM_IN_SLOT")?;
        Ok(Self {
            vendor_raw_guid: buf.get_u64_le(),
            item: buf.get_u32_le(),
            bag_raw_guid: buf.get_u64_le(),
            bag_slot: buf.get_u8(),
            count: buf.get_u8(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.vendor_raw_guid);
        buf.put_u32_le(self.item);
        buf.put_u64_le(self.bag_raw_guid);
        buf.put_u8(self.bag_slot);
        buf.put_u8(self.count);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrainerBuySpellRequest {
    pub trainer_raw_guid: u64,
    pub spell: u32,
}

impl TrainerBuySpellRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 12, "CMSG_TRAINER_BUY_SPELL")?;
        Ok(Self {
            trainer_raw_guid: buf.get_u64_le(),
            spell: buf.get_u32_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.trainer_raw_guid);
        buf.put_u32_le(self.spell);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BankerActivateRequest {
    pub banker_raw_guid: u64,
}

impl BankerActivateRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        Ok(Self {
            banker_raw_guid: read_guid_request(buf, "CMSG_BANKER_ACTIVATE")?,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.banker_raw_guid);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuyBankSlotRequest {
    pub banker_raw_guid: u64,
}

impl BuyBankSlotRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        Ok(Self {
            banker_raw_guid: read_guid_request(buf, "CMSG_BUY_BANK_SLOT")?,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.banker_raw_guid);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BankItemRequest {
    pub src_bag: u8,
    pub src_slot: u8,
}

impl BankItemRequest {
    pub fn read_auto_bank(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_exact_remaining(buf, 2, "CMSG_AUTOBANK_ITEM")?;
        Ok(Self {
            src_bag: buf.get_u8(),
            src_slot: buf.get_u8(),
        })
    }

    pub fn read_auto_store_bank(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_exact_remaining(buf, 2, "CMSG_AUTOSTORE_BANK_ITEM")?;
        Ok(Self {
            src_bag: buf.get_u8(),
            src_slot: buf.get_u8(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.src_bag);
        buf.put_u8(self.src_slot);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReclaimCorpseRequest {
    pub requested_corpse_raw_guid: Option<u64>,
}

impl ReclaimCorpseRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        let requested_corpse_raw_guid = if buf.remaining() >= 8 {
            Some(buf.get_u64_le())
        } else {
            None
        };
        Ok(Self {
            requested_corpse_raw_guid,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        if let Some(raw_guid) = self.requested_corpse_raw_guid {
            buf.put_u64_le(raw_guid);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutostoreLootItemRequest {
    pub loot_slot: u8,
}

impl AutostoreLootItemRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 1, "CMSG_AUTOSTORE_LOOT_ITEM")?;
        Ok(Self {
            loot_slot: buf.get_u8(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.loot_slot);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LootRollRequest {
    pub loot_raw_guid: u64,
    pub loot_slot: u8,
    pub vote: u8,
}

impl LootRollRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 13, "CMSG_LOOT_ROLL")?;
        Ok(Self {
            loot_raw_guid: buf.get_u64_le(),
            loot_slot: buf.get_u32_le() as u8,
            vote: buf.get_u8(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.loot_raw_guid);
        buf.put_u32_le(self.loot_slot as u32);
        buf.put_u8(self.vote);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LootMasterGiveRequest {
    pub loot_raw_guid: u64,
    pub loot_slot: u8,
    pub target_raw_guid: u64,
}

impl LootMasterGiveRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 17, "CMSG_LOOT_MASTER_GIVE")?;
        Ok(Self {
            loot_raw_guid: buf.get_u64_le(),
            loot_slot: buf.get_u8(),
            target_raw_guid: buf.get_u64_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.loot_raw_guid);
        buf.put_u8(self.loot_slot);
        buf.put_u64_le(self.target_raw_guid);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupInviteRequest {
    pub member_name: String,
}

impl GroupInviteRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        Ok(Self {
            member_name: read_c_string_request(buf, "CMSG_GROUP_INVITE")?,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        write_c_string(buf, &self.member_name);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupUninviteRequest {
    pub member_name: String,
}

impl GroupUninviteRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        Ok(Self {
            member_name: read_c_string_request(buf, "CMSG_GROUP_UNINVITE")?,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        write_c_string(buf, &self.member_name);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupChangeSubGroupRequest {
    pub member_name: String,
    pub subgroup: u8,
}

impl GroupChangeSubGroupRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        let member_name = read_c_string_request(buf, "CMSG_GROUP_CHANGE_SUB_GROUP")?;
        ensure_remaining(buf, 1, "CMSG_GROUP_CHANGE_SUB_GROUP")?;
        Ok(Self {
            member_name,
            subgroup: buf.get_u8(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        write_c_string(buf, &self.member_name);
        buf.put_u8(self.subgroup);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupAssistantLeaderRequest {
    pub raw_guid: u64,
    pub enabled: bool,
}

impl GroupAssistantLeaderRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 9, "CMSG_GROUP_ASSISTANT_LEADER")?;
        Ok(Self {
            raw_guid: buf.get_u64_le(),
            enabled: buf.get_u8() != 0,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.raw_guid);
        buf.put_u8(u8::from(self.enabled));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LootMethodRequest {
    pub loot_method: u32,
    pub master_looter_raw_guid: u64,
    pub loot_threshold: u32,
}

impl LootMethodRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 16, "CMSG_LOOT_METHOD")?;
        Ok(Self {
            loot_method: buf.get_u32_le(),
            master_looter_raw_guid: buf.get_u64_le(),
            loot_threshold: buf.get_u32_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.loot_method);
        buf.put_u64_le(self.master_looter_raw_guid);
        buf.put_u32_le(self.loot_threshold);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestQueryRequest {
    pub quest_id: u32,
}

impl QuestQueryRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        Ok(Self {
            quest_id: read_u32_request(buf, "CMSG_QUEST_QUERY")?,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.quest_id);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreatureQueryRequest {
    pub entry: u32,
    pub raw_guid: u64,
}

impl CreatureQueryRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 12, "CMSG_CREATURE_QUERY")?;
        Ok(Self {
            entry: buf.get_u32_le(),
            raw_guid: buf.get_u64_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.entry);
        buf.put_u64_le(self.raw_guid);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameObjectQueryRequest {
    pub entry: u32,
    pub raw_guid: u64,
}

impl GameObjectQueryRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 12, "CMSG_GAMEOBJECT_QUERY")?;
        Ok(Self {
            entry: buf.get_u32_le(),
            raw_guid: buf.get_u64_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.entry);
        buf.put_u64_le(self.raw_guid);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NpcTextQueryRequest {
    pub text_id: u32,
    pub raw_guid: u64,
}

impl NpcTextQueryRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 12, "CMSG_NPC_TEXT_QUERY")?;
        Ok(Self {
            text_id: buf.get_u32_le(),
            raw_guid: buf.get_u64_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.text_id);
        buf.put_u64_le(self.raw_guid);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NameQueryRequest {
    pub raw_guid: u64,
}

impl NameQueryRequest {
    pub const SIZE: usize = 8;

    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_exact_remaining(buf, Self::SIZE, "CMSG_NAME_QUERY")?;
        Ok(Self {
            raw_guid: buf.get_u64_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.raw_guid);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemQuerySingleRequest {
    pub item_id: u32,
}

impl ItemQuerySingleRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        Ok(Self {
            item_id: read_u32_request(buf, "CMSG_ITEM_QUERY_SINGLE")?,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.item_id);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemNameQueryRequest {
    pub item_id: u32,
}

impl ItemNameQueryRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        Ok(Self {
            item_id: read_u32_request(buf, "CMSG_ITEM_NAME_QUERY")?,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.item_id);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestAccountDataRequest {
    pub data_type: u32,
}

impl RequestAccountDataRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        Ok(Self {
            data_type: read_u32_request(buf, "CMSG_REQUEST_ACCOUNT_DATA")?,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.data_type);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateAccountDataRequest {
    pub data_type: u32,
    pub decompressed_size: u32,
    pub compressed_data: Vec<u8>,
}

impl UpdateAccountDataRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_remaining(buf, 8, "CMSG_UPDATE_ACCOUNT_DATA")?;
        let data_type = buf.get_u32_le();
        let decompressed_size = buf.get_u32_le();
        let mut compressed_data = vec![0u8; buf.remaining()];
        buf.copy_to_slice(&mut compressed_data);
        Ok(Self {
            data_type,
            decompressed_size,
            compressed_data,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.data_type);
        buf.put_u32_le(self.decompressed_size);
        buf.put_slice(&self.compressed_data);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TutorialFlagRequest {
    pub flag: u32,
}

impl TutorialFlagRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        Ok(Self {
            flag: read_u32_request(buf, "CMSG_TUTORIAL_FLAG")?,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.flag);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StandStateChangeRequest {
    pub stand_state: u32,
}

impl StandStateChangeRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        Ok(Self {
            stand_state: read_u32_request(buf, "CMSG_STANDSTATECHANGE")?,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.stand_state);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetActionButtonRequest {
    pub button: u8,
    pub packed_data: u32,
}

impl SetActionButtonRequest {
    pub const SIZE: usize = 5;

    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_exact_remaining(buf, Self::SIZE, "CMSG_SET_ACTION_BUTTON")?;
        Ok(Self {
            button: buf.get_u8(),
            packed_data: buf.get_u32_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.button);
        buf.put_u32_le(self.packed_data);
    }

    pub fn action(self) -> u32 {
        self.packed_data & 0x00FF_FFFF
    }

    pub fn action_type(self) -> u8 {
        ((self.packed_data & 0xFF00_0000) >> 24) as u8
    }

    pub fn removes_binding(self) -> bool {
        self.packed_data == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetSelectionRequest {
    pub raw_guid: u64,
}

impl SetSelectionRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        Ok(Self {
            raw_guid: read_guid_request(buf, "CMSG_SET_SELECTION")?,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.raw_guid);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetTargetObsoleteRequest {
    pub raw_guid: u64,
}

impl SetTargetObsoleteRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        Ok(Self {
            raw_guid: read_guid_request(buf, "CMSG_SET_TARGET_OBSOLETE")?,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.raw_guid);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetActiveMoverRequest {
    pub raw_guid: u64,
}

impl SetActiveMoverRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        Ok(Self {
            raw_guid: read_guid_request(buf, "CMSG_SET_ACTIVE_MOVER")?,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.raw_guid);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldAuthSessionRequest {
    pub client_build: u32,
    pub account: String,
    pub client_seed: u32,
    pub digest: [u8; 20],
    pub addon_data: Vec<u8>,
}

impl WorldAuthSessionRequest {
    pub const MIN_SIZE: usize = 4 + 4 + 1 + 4 + 20;

    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        if buf.remaining() < Self::MIN_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "CMSG_AUTH_SESSION payload too short",
            ));
        }

        let client_build = buf.get_u32_le();
        let _unk2 = buf.get_u32_le();
        let mut account_bytes = Vec::new();
        let mut account_terminated = false;
        while buf.has_remaining() {
            let byte = buf.get_u8();
            if byte == 0 {
                account_terminated = true;
                break;
            }
            account_bytes.push(byte);
        }
        if !account_terminated {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "CMSG_AUTH_SESSION account is not NUL-terminated",
            ));
        }
        let account = String::from_utf8(account_bytes)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

        if buf.remaining() < 4 + 20 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "CMSG_AUTH_SESSION truncated after account",
            ));
        }

        let client_seed = buf.get_u32_le();
        let mut digest = [0u8; 20];
        buf.copy_to_slice(&mut digest);
        let mut addon_data = vec![0u8; buf.remaining()];
        buf.copy_to_slice(&mut addon_data);

        Ok(Self {
            client_build,
            account,
            client_seed,
            digest,
            addon_data,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.client_build);
        buf.put_u32_le(0);
        buf.put_slice(self.account.as_bytes());
        buf.put_u8(0);
        buf.put_u32_le(self.client_seed);
        buf.put_slice(&self.digest);
        buf.put_slice(&self.addon_data);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PingRequest {
    pub sequence: u32,
}

impl PingRequest {
    pub const SIZE: usize = 4;

    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        if buf.remaining() < Self::SIZE {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "CMSG_PING payload too short",
            ));
        }
        Ok(Self {
            sequence: buf.get_u32_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.sequence);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PongResponse {
    pub sequence: u32,
}

impl PongResponse {
    pub const OPCODE: WorldOpcode = WorldOpcode::SmsgPong;
    pub const SIZE: usize = 4;

    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        if buf.remaining() < Self::SIZE {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "SMSG_PONG payload too short",
            ));
        }
        Ok(Self {
            sequence: buf.get_u32_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.sequence);
    }

    pub fn to_body(self) -> [u8; Self::SIZE] {
        self.sequence.to_le_bytes()
    }
}

impl ServerWorldPacket for PongResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgPong;

    fn write_body(&self, buf: &mut impl BufMut) {
        self.write(buf);
    }
}

impl From<PingRequest> for PongResponse {
    fn from(request: PingRequest) -> Self {
        Self {
            sequence: request.sequence,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldLocationResponse {
    pub map_id: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub orientation: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MovementJumpResponse {
    pub z_speed: f32,
    pub cos_angle: f32,
    pub sin_angle: f32,
    pub xy_speed: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MovementInfoResponse {
    pub flags: u32,
    pub client_time: u32,
    pub position: WorldLocationResponse,
    pub fall_time: u32,
    pub jump: Option<MovementJumpResponse>,
}

fn write_movement_info(buf: &mut impl BufMut, movement: &MovementInfoResponse) {
    buf.put_u32_le(movement.flags);
    buf.put_u32_le(movement.client_time);
    buf.put_f32_le(movement.position.x);
    buf.put_f32_le(movement.position.y);
    buf.put_f32_le(movement.position.z);
    buf.put_f32_le(movement.position.orientation);
    buf.put_u32_le(movement.fall_time);
    if let Some(jump) = movement.jump {
        buf.put_f32_le(jump.z_speed);
        buf.put_f32_le(jump.cos_angle);
        buf.put_f32_le(jump.sin_angle);
        buf.put_f32_le(jump.xy_speed);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MsgMoveTeleportAckResponse {
    pub player: ObjectGuid,
    pub counter: u32,
    pub movement: MovementInfoResponse,
}

impl ServerWorldPacket for MsgMoveTeleportAckResponse {
    const OPCODE: WorldOpcode = WorldOpcode::MsgMoveTeleportAck;

    fn write_body(&self, buf: &mut impl BufMut) {
        put_packed_guid(buf, self.player);
        buf.put_u32_le(self.counter);
        write_movement_info(buf, &self.movement);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmsgMonsterMoveStopResponse {
    pub guid: ObjectGuid,
    pub position: WorldLocationResponse,
    pub spline_id: u32,
    pub move_type: u8,
}

impl ServerWorldPacket for SmsgMonsterMoveStopResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgMonsterMove;

    fn write_body(&self, buf: &mut impl BufMut) {
        put_packed_guid(buf, self.guid);
        buf.put_f32_le(self.position.x);
        buf.put_f32_le(self.position.y);
        buf.put_f32_le(self.position.z);
        buf.put_u32_le(self.spline_id);
        buf.put_u8(self.move_type);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SmsgMonsterMovePathResponse {
    pub guid: ObjectGuid,
    pub start: WorldLocationResponse,
    pub path: Vec<WorldLocationResponse>,
    pub spline_id: u32,
    pub duration_ms: u32,
    pub facing_target: Option<ObjectGuid>,
    pub move_type_normal: u8,
    pub move_type_facing_target: u8,
    pub run_spline_flag: u32,
    pub run: bool,
    pub catmull_rom: bool,
}

impl ServerWorldPacket for SmsgMonsterMovePathResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgMonsterMove;

    fn write_body(&self, buf: &mut impl BufMut) {
        put_packed_guid(buf, self.guid);
        buf.put_f32_le(self.start.x);
        buf.put_f32_le(self.start.y);
        buf.put_f32_le(self.start.z);
        buf.put_u32_le(self.spline_id);
        if let Some(target) = self.facing_target {
            buf.put_u8(self.move_type_facing_target);
            buf.put_u64_le(target.raw());
        } else {
            buf.put_u8(self.move_type_normal);
        }
        buf.put_u32_le(if self.run { self.run_spline_flag } else { 0 });
        buf.put_u32_le(self.duration_ms);
        if self.catmull_rom {
            buf.put_u32_le(self.path.len() as u32);
            for point in &self.path {
                buf.put_f32_le(point.x);
                buf.put_f32_le(point.y);
                buf.put_f32_le(point.z);
            }
            return;
        }
        let Some(destination) = self.path.last().copied() else {
            buf.put_u32_le(0);
            return;
        };
        let mut offsets = Vec::new();
        for point in &self.path[..self.path.len().saturating_sub(1)] {
            let offset_x = destination.x - point.x;
            let offset_y = destination.y - point.y;
            let offset_z = destination.z - point.z;
            if (offset_x * offset_x) + (offset_y * offset_y) + (offset_z * offset_z) < 0.5 {
                continue;
            }
            offsets.push(pack_monster_move_xyz_offset(offset_x, offset_y, offset_z));
        }
        buf.put_u32_le(1 + offsets.len() as u32);
        buf.put_f32_le(destination.x);
        buf.put_f32_le(destination.y);
        buf.put_f32_le(destination.z);
        for offset in offsets {
            buf.put_u32_le(offset);
        }
    }
}

pub fn pack_monster_move_xyz_offset(x: f32, y: f32, z: f32) -> u32 {
    let mut packed = 0;
    packed |= ((x / 0.25) as i32 as u32) & 0x7FF;
    packed |= (((y / 0.25) as i32 as u32) & 0x7FF) << 11;
    packed |= (((z / 0.25) as i32 as u32) & 0x3FF) << 22;
    packed
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SplineSetSpeedResponse {
    pub guid: ObjectGuid,
    pub speed: f32,
}

impl SplineSetSpeedResponse {
    pub fn body(&self) -> Vec<u8> {
        let mut body = Vec::new();
        put_packed_guid(&mut body, self.guid);
        body.put_f32_le(self.speed);
        body
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgForceMoveRootResponse {
    pub player: ObjectGuid,
    pub counter: u32,
}

impl ServerWorldPacket for SmsgForceMoveRootResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgForceMoveRoot;

    fn write_body(&self, buf: &mut impl BufMut) {
        put_packed_guid(buf, self.player);
        buf.put_u32_le(self.counter);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgForceMoveUnrootResponse {
    pub player: ObjectGuid,
    pub counter: u32,
}

impl ServerWorldPacket for SmsgForceMoveUnrootResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgForceMoveUnroot;

    fn write_body(&self, buf: &mut impl BufMut) {
        put_packed_guid(buf, self.player);
        buf.put_u32_le(self.counter);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgCorpseReclaimDelayResponse {
    pub delay_millis: u32,
}

impl ServerWorldPacket for SmsgCorpseReclaimDelayResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgCorpseReclaimDelay;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.delay_millis);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MsgCorpseQueryResponse {
    pub corpse_position: Option<WorldLocationResponse>,
}

impl ServerWorldPacket for MsgCorpseQueryResponse {
    const OPCODE: WorldOpcode = WorldOpcode::MsgCorpseQuery;

    fn write_body(&self, buf: &mut impl BufMut) {
        let Some(position) = self.corpse_position else {
            buf.put_u8(0);
            return;
        };
        buf.put_u8(1);
        buf.put_i32_le(position.map_id as i32);
        buf.put_f32_le(position.x);
        buf.put_f32_le(position.y);
        buf.put_f32_le(position.z);
        buf.put_u32_le(position.map_id);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgLogXpGainResponse {
    pub source: Option<ObjectGuid>,
    pub given_xp: u32,
    pub base_xp: u32,
}

impl ServerWorldPacket for SmsgLogXpGainResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgLogXpGain;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.source.map_or(0, ObjectGuid::raw));
        buf.put_u32_le(self.given_xp);
        buf.put_u8(u8::from(self.source.is_none()));
        if self.source.is_some() {
            buf.put_u32_le(self.base_xp);
            buf.put_f32_le(1.0);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgLevelupInfoResponse {
    pub new_level: u8,
    pub health_delta: i32,
    pub mana_delta: i32,
    pub power_deltas: [u32; 4],
    pub stat_deltas: Vec<i32>,
}

impl ServerWorldPacket for SmsgLevelupInfoResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgLevelupInfo;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.new_level as u32);
        buf.put_i32_le(self.health_delta);
        buf.put_i32_le(self.mana_delta);
        for delta in self.power_deltas {
            buf.put_u32_le(delta);
        }
        for delta in &self.stat_deltas {
            buf.put_i32_le(*delta);
        }
    }
}

fn write_spell_cast_targets_body(buf: &mut impl BufMut, targets: &SpellCastTargets) {
    let target_mask = normalized_spell_cast_target_mask(targets);
    buf.put_u16_le(target_mask);
    if target_mask & SPELL_CAST_TARGET_UNIT != 0 {
        put_packed_guid(buf, targets.unit_target.unwrap_or(ObjectGuid::EMPTY));
    }
    if target_mask & (SPELL_CAST_TARGET_GAMEOBJECT | SPELL_CAST_TARGET_LOCKED) != 0 {
        put_packed_guid(buf, targets.gameobject_target.unwrap_or(ObjectGuid::EMPTY));
    }
    if target_mask & SPELL_CAST_TARGET_SOURCE_LOCATION != 0 {
        let location = targets.source_location.unwrap_or(SpellTargetLocation {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        });
        buf.put_f32_le(location.x);
        buf.put_f32_le(location.y);
        buf.put_f32_le(location.z);
    }
    if target_mask & SPELL_CAST_TARGET_DEST_LOCATION != 0 {
        let location = targets.destination.unwrap_or(SpellTargetLocation {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        });
        buf.put_f32_le(location.x);
        buf.put_f32_le(location.y);
        buf.put_f32_le(location.z);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgAttackStartResponse {
    pub attacker: ObjectGuid,
    pub victim: ObjectGuid,
}

impl ServerWorldPacket for SmsgAttackStartResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgAttackStart;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.attacker.raw());
        buf.put_u64_le(self.victim.raw());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgAttackStopResponse {
    pub attacker: ObjectGuid,
    pub victim: ObjectGuid,
    pub attacker_dead: bool,
}

impl ServerWorldPacket for SmsgAttackStopResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgAttackStop;

    fn write_body(&self, buf: &mut impl BufMut) {
        put_packed_guid(buf, self.attacker);
        put_packed_guid(buf, self.victim);
        buf.put_u32_le(u32::from(self.attacker_dead));
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmsgAttackerStateUpdateResponse {
    pub hit_info: u32,
    pub attacker: ObjectGuid,
    pub victim: ObjectGuid,
    pub total_damage: u32,
    pub school: u32,
    pub school_damage: u32,
    pub absorbed: u32,
    pub resisted: i32,
    pub victim_state: u32,
    pub spell_id: u32,
    pub blocked: u32,
}

impl ServerWorldPacket for SmsgAttackerStateUpdateResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgAttackerStateUpdate;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.hit_info);
        put_packed_guid(buf, self.attacker);
        put_packed_guid(buf, self.victim);
        buf.put_u32_le(self.total_damage);
        buf.put_u8(1);
        buf.put_u32_le(self.school);
        buf.put_f32_le(self.school_damage as f32);
        buf.put_u32_le(self.school_damage);
        buf.put_u32_le(self.absorbed);
        buf.put_i32_le(self.resisted);
        buf.put_u32_le(self.victim_state);
        buf.put_u32_le(0);
        buf.put_u32_le(self.spell_id);
        buf.put_u32_le(self.blocked);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgSpellNonMeleeDamageLogResponse {
    pub attacker: ObjectGuid,
    pub target: ObjectGuid,
    pub spell_id: u32,
    pub damage: u32,
    pub school: u8,
    pub absorb: u32,
    pub resist: i32,
    pub periodic: bool,
    pub blocked: u32,
    pub hit_info: u32,
}

impl ServerWorldPacket for SmsgSpellNonMeleeDamageLogResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgSpellNonMeleeDamageLog;

    fn write_body(&self, buf: &mut impl BufMut) {
        put_packed_guid(buf, self.target);
        put_packed_guid(buf, self.attacker);
        buf.put_u32_le(self.spell_id);
        buf.put_u32_le(self.damage);
        buf.put_u8(self.school);
        buf.put_u32_le(self.absorb);
        buf.put_i32_le(self.resist);
        buf.put_u8(u8::from(self.periodic));
        buf.put_u8(0);
        buf.put_u32_le(self.blocked);
        buf.put_u32_le(self.hit_info);
        buf.put_u8(0);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgEnvironmentalDamageLogResponse {
    pub player: ObjectGuid,
    pub damage_type: u8,
    pub damage: u32,
    pub absorbed: u32,
    pub resisted: u32,
}

impl ServerWorldPacket for SmsgEnvironmentalDamageLogResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgEnvironmentalDamageLog;

    fn write_body(&self, buf: &mut impl BufMut) {
        put_packed_guid(buf, self.player);
        buf.put_u8(self.damage_type);
        buf.put_u32_le(self.damage);
        buf.put_u32_le(self.absorbed);
        buf.put_u32_le(self.resisted);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgSpellLogMissResponse {
    pub caster: ObjectGuid,
    pub target: ObjectGuid,
    pub spell_id: u32,
    pub miss_info: u8,
}

impl ServerWorldPacket for SmsgSpellLogMissResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgSpellLogMiss;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.spell_id);
        buf.put_u64_le(self.caster.raw());
        buf.put_u8(0);
        buf.put_u32_le(1);
        buf.put_u64_le(self.target.raw());
        buf.put_u8(self.miss_info);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgSpellHealLogResponse {
    pub caster: ObjectGuid,
    pub target: ObjectGuid,
    pub spell_id: u32,
    pub heal: u32,
    pub critical: bool,
}

impl ServerWorldPacket for SmsgSpellHealLogResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgSpellHealLog;

    fn write_body(&self, buf: &mut impl BufMut) {
        put_packed_guid(buf, self.target);
        put_packed_guid(buf, self.caster);
        buf.put_u32_le(self.spell_id);
        buf.put_u32_le(self.heal);
        buf.put_u8(u8::from(self.critical));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgSpellEnergizeLogResponse {
    pub caster: ObjectGuid,
    pub target: ObjectGuid,
    pub spell_id: u32,
    pub power_type: u32,
    pub amount: u32,
}

impl ServerWorldPacket for SmsgSpellEnergizeLogResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgSpellEnergizeLog;

    fn write_body(&self, buf: &mut impl BufMut) {
        put_packed_guid(buf, self.target);
        put_packed_guid(buf, self.caster);
        buf.put_u32_le(self.spell_id);
        buf.put_u32_le(self.power_type);
        buf.put_u32_le(self.amount);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgSpellFailureResponse {
    pub caster: ObjectGuid,
    pub spell_id: u32,
    pub result: u8,
}

impl ServerWorldPacket for SmsgSpellFailureResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgSpellFailure;

    fn write_body(&self, buf: &mut impl BufMut) {
        put_packed_guid(buf, self.caster);
        buf.put_u32_le(self.spell_id);
        buf.put_u8(self.result);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgSpellFailedOtherResponse {
    pub caster: ObjectGuid,
    pub spell_id: u32,
}

impl ServerWorldPacket for SmsgSpellFailedOtherResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgSpellFailedOther;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.caster.raw());
        buf.put_u32_le(self.spell_id);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgSpellDelayedResponse {
    pub caster: ObjectGuid,
    pub delay_millis: u32,
}

impl ServerWorldPacket for SmsgSpellDelayedResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgSpellDelayed;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.caster.raw());
        buf.put_u32_le(self.delay_millis);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgUpdateAuraDurationResponse {
    pub slot: u8,
    pub remaining_millis: u32,
}

impl ServerWorldPacket for SmsgUpdateAuraDurationResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgUpdateAuraDuration;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.slot);
        buf.put_u32_le(self.remaining_millis);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgCastResultResponse {
    pub spell_id: u32,
    pub status: u8,
    pub failure: Option<u8>,
}

impl ServerWorldPacket for SmsgCastResultResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgCastResult;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.spell_id);
        buf.put_u8(self.status);
        if let Some(failure) = self.failure {
            buf.put_u8(failure);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpellAmmoVisual {
    pub display_id: u32,
    pub inventory_type: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmsgSpellGoMissTarget {
    pub target: ObjectGuid,
    pub miss_info: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SmsgSpellGoResponse {
    pub source: ObjectGuid,
    pub caster: ObjectGuid,
    pub spell_id: u32,
    pub cast_flags: u16,
    pub targets: SpellCastTargets,
    pub hit_targets: Vec<ObjectGuid>,
    pub miss_targets: Vec<SmsgSpellGoMissTarget>,
    pub ammo: Option<SpellAmmoVisual>,
}

impl ServerWorldPacket for SmsgSpellGoResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgSpellGo;

    fn write_body(&self, buf: &mut impl BufMut) {
        put_packed_guid(buf, self.source);
        put_packed_guid(buf, self.caster);
        buf.put_u32_le(self.spell_id);
        buf.put_u16_le(self.cast_flags);
        buf.put_u8(self.hit_targets.len() as u8);
        for target in &self.hit_targets {
            buf.put_u64_le(target.raw());
        }
        buf.put_u8(self.miss_targets.len() as u8);
        for miss in &self.miss_targets {
            buf.put_u64_le(miss.target.raw());
            buf.put_u8(miss.miss_info);
        }
        write_spell_cast_targets_body(buf, &self.targets);
        if let Some(ammo) = self.ammo {
            buf.put_u32_le(ammo.display_id);
            buf.put_u32_le(ammo.inventory_type);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmsgSpellStartResponse {
    pub source: ObjectGuid,
    pub caster: ObjectGuid,
    pub spell_id: u32,
    pub cast_flags: u16,
    pub cast_time_ms: u32,
    pub targets: SpellCastTargets,
    pub ammo: Option<SpellAmmoVisual>,
}

impl ServerWorldPacket for SmsgSpellStartResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgSpellStart;

    fn write_body(&self, buf: &mut impl BufMut) {
        put_packed_guid(buf, self.source);
        put_packed_guid(buf, self.caster);
        buf.put_u32_le(self.spell_id);
        buf.put_u16_le(self.cast_flags);
        buf.put_u32_le(self.cast_time_ms);
        write_spell_cast_targets_body(buf, &self.targets);
        if let Some(ammo) = self.ammo {
            buf.put_u32_le(ammo.display_id);
            buf.put_u32_le(ammo.inventory_type);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LootItemResponse {
    pub slot: u8,
    pub item: u32,
    pub count: u32,
    pub display_id: u32,
    pub random_suffix: u32,
    pub random_property: u32,
    pub slot_type: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgLootResponse {
    pub target: ObjectGuid,
    pub loot_type: u8,
    pub money: u32,
    pub items: Vec<LootItemResponse>,
}

impl ServerWorldPacket for SmsgLootResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgLootResponse;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.target.raw());
        buf.put_u8(self.loot_type);
        buf.put_u32_le(self.money);
        buf.put_u8(self.items.len().min(u8::MAX as usize) as u8);
        for item in self.items.iter().take(u8::MAX as usize) {
            buf.put_u8(item.slot);
            buf.put_u32_le(item.item);
            buf.put_u32_le(item.count);
            buf.put_u32_le(item.display_id);
            buf.put_u32_le(item.random_suffix);
            buf.put_u32_le(item.random_property);
            buf.put_u8(item.slot_type);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgLootErrorResponse {
    pub target: ObjectGuid,
    pub loot_type: u8,
    pub error: u8,
}

impl ServerWorldPacket for SmsgLootErrorResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgLootResponse;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.target.raw());
        buf.put_u8(self.loot_type);
        buf.put_u8(self.error);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgLootReleaseResponse {
    pub target: ObjectGuid,
    pub released: bool,
}

impl ServerWorldPacket for SmsgLootReleaseResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgLootReleaseResponse;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.target.raw());
        buf.put_u8(u8::from(self.released));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgLootMasterListResponse {
    pub members: Vec<ObjectGuid>,
}

impl ServerWorldPacket for SmsgLootMasterListResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgLootMasterList;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.members.len().min(u8::MAX as usize) as u8);
        for member in self.members.iter().take(u8::MAX as usize) {
            buf.put_u64_le(member.raw());
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LootRollItemResponse {
    pub loot_guid: ObjectGuid,
    pub loot_slot: u8,
    pub item: u32,
    pub random_suffix: u32,
    pub random_property: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgLootStartRollResponse {
    pub item: LootRollItemResponse,
    pub roll_time_millis: u32,
    pub vote_mask: u8,
}

impl ServerWorldPacket for SmsgLootStartRollResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgLootStartRoll;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.item.loot_guid.raw());
        buf.put_u32_le(self.item.loot_slot as u32);
        buf.put_u32_le(self.item.item);
        buf.put_u32_le(self.item.random_suffix);
        buf.put_u32_le(self.item.random_property);
        buf.put_u32_le(self.roll_time_millis);
        buf.put_u8(self.vote_mask);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgLootRollResponse {
    pub item: LootRollItemResponse,
    pub roller: ObjectGuid,
    pub roll_number: u8,
    pub roll_type: u8,
    pub auto_pass: u8,
}

impl ServerWorldPacket for SmsgLootRollResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgLootRoll;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.item.loot_guid.raw());
        buf.put_u32_le(self.item.loot_slot as u32);
        buf.put_u64_le(self.roller.raw());
        buf.put_u32_le(self.item.item);
        buf.put_u32_le(self.item.random_suffix);
        buf.put_u32_le(self.item.random_property);
        buf.put_u8(self.roll_number);
        buf.put_u8(self.roll_type);
        buf.put_u8(self.auto_pass);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgLootRollWonResponse {
    pub item: LootRollItemResponse,
    pub winner: ObjectGuid,
    pub roll_number: u8,
    pub vote: u8,
}

impl ServerWorldPacket for SmsgLootRollWonResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgLootRollWon;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.item.loot_guid.raw());
        buf.put_u32_le(self.item.loot_slot as u32);
        buf.put_u32_le(self.item.item);
        buf.put_u32_le(self.item.random_suffix);
        buf.put_u32_le(self.item.random_property);
        buf.put_u64_le(self.winner.raw());
        buf.put_u8(self.roll_number);
        buf.put_u8(self.vote);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgLootAllPassedResponse {
    pub item: LootRollItemResponse,
}

impl ServerWorldPacket for SmsgLootAllPassedResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgLootAllPassed;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.item.loot_guid.raw());
        buf.put_u32_le(self.item.loot_slot as u32);
        buf.put_u32_le(self.item.item);
        buf.put_u32_le(self.item.random_suffix);
        buf.put_u32_le(self.item.random_property);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SingleByteResponse {
    pub result: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgCharCreateResponse {
    pub result: u8,
}

impl ServerWorldPacket for SmsgCharCreateResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgCharCreate;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.result);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgCharDeleteResponse {
    pub result: u8,
}

impl ServerWorldPacket for SmsgCharDeleteResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgCharDelete;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.result);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgCharacterLoginFailedResponse {
    pub result: u8,
}

impl ServerWorldPacket for SmsgCharacterLoginFailedResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgCharacterLoginFailed;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.result);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgAuthResponse {
    pub result: u8,
    pub billing_time_remaining: u32,
    pub billing_plan_flags: u8,
    pub billing_time_rested: u32,
    pub expansion: u8,
}

impl SmsgAuthResponse {
    pub fn ok() -> Self {
        Self {
            result: 0x0C,
            billing_time_remaining: 0,
            billing_plan_flags: 0,
            billing_time_rested: 0,
            expansion: 0,
        }
    }
}

impl ServerWorldPacket for SmsgAuthResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgAuthResponse;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.result);
        if self.result == 0x0C {
            buf.put_u32_le(self.billing_time_remaining);
            buf.put_u8(self.billing_plan_flags);
            buf.put_u32_le(self.billing_time_rested);
            buf.put_u8(self.expansion);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgLogoutResponse {
    pub failure_reason: u32,
    pub instant_logout: bool,
}

impl ServerWorldPacket for SmsgLogoutResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgLogoutResponse;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.failure_reason);
        buf.put_u8(u8::from(self.instant_logout));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SmsgLogoutCompleteResponse;

impl ServerWorldPacket for SmsgLogoutCompleteResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgLogoutComplete;

    fn write_body(&self, _buf: &mut impl BufMut) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SmsgLogoutCancelAckResponse;

impl ServerWorldPacket for SmsgLogoutCancelAckResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgLogoutCancelAck;

    fn write_body(&self, _buf: &mut impl BufMut) {}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharEnumEquipmentResponse {
    pub display_id: u32,
    pub inventory_type: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CharEnumEntryResponse {
    pub guid: ObjectGuid,
    pub name: String,
    pub race: u8,
    pub class: u8,
    pub gender: u8,
    pub player_bytes: u32,
    pub player_bytes2: u32,
    pub level: u8,
    pub zone: u32,
    pub map: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub guild_id: u32,
    pub flags: u32,
    pub first_login: bool,
    pub pet_display_id: u32,
    pub pet_level: u32,
    pub pet_family: u32,
    pub equipment: Vec<CharEnumEquipmentResponse>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SmsgCharEnumResponse {
    pub characters: Vec<CharEnumEntryResponse>,
}

impl ServerWorldPacket for SmsgCharEnumResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgCharEnum;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.characters.len().min(u8::MAX as usize) as u8);
        for character in self.characters.iter().take(u8::MAX as usize) {
            buf.put_u64_le(character.guid.raw());
            write_c_string(buf, &character.name);
            buf.put_u8(character.race);
            buf.put_u8(character.class);
            buf.put_u8(character.gender);
            buf.put_u8((character.player_bytes & 0xFF) as u8);
            buf.put_u8(((character.player_bytes >> 8) & 0xFF) as u8);
            buf.put_u8(((character.player_bytes >> 16) & 0xFF) as u8);
            buf.put_u8(((character.player_bytes >> 24) & 0xFF) as u8);
            buf.put_u8((character.player_bytes2 & 0xFF) as u8);
            buf.put_u8(character.level);
            buf.put_u32_le(character.zone);
            buf.put_u32_le(character.map);
            buf.put_f32_le(character.x);
            buf.put_f32_le(character.y);
            buf.put_f32_le(character.z);
            buf.put_u32_le(character.guild_id);
            buf.put_u32_le(character.flags);
            buf.put_u8(u8::from(character.first_login));
            buf.put_u32_le(character.pet_display_id);
            buf.put_u32_le(character.pet_level);
            buf.put_u32_le(character.pet_family);
            for visual in &character.equipment {
                buf.put_u32_le(visual.display_id);
                buf.put_u8(visual.inventory_type);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmsgLoginVerifyWorldResponse {
    pub map: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub orientation: f32,
}

impl ServerWorldPacket for SmsgLoginVerifyWorldResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgLoginVerifyWorld;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.map);
        buf.put_f32_le(self.x);
        buf.put_f32_le(self.y);
        buf.put_f32_le(self.z);
        buf.put_f32_le(self.orientation);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmsgBindpointUpdateResponse {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub map: u32,
    pub zone: u32,
}

impl ServerWorldPacket for SmsgBindpointUpdateResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgBindpointUpdate;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_f32_le(self.x);
        buf.put_f32_le(self.y);
        buf.put_f32_le(self.z);
        buf.put_u32_le(self.map);
        buf.put_u32_le(self.zone);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgPlayerBoundResponse {
    pub caster: ObjectGuid,
    pub area: u32,
}

impl ServerWorldPacket for SmsgPlayerBoundResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgPlayerBound;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.caster.raw());
        buf.put_u32_le(self.area);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgAccountDataTimesResponse {
    pub digests: Vec<[u8; 16]>,
}

impl ServerWorldPacket for SmsgAccountDataTimesResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgAccountDataTimes;

    fn write_body(&self, buf: &mut impl BufMut) {
        for digest in &self.digests {
            buf.put_slice(digest);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgTutorialFlagsResponse {
    pub flags: [u32; 8],
}

impl ServerWorldPacket for SmsgTutorialFlagsResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgTutorialFlags;

    fn write_body(&self, buf: &mut impl BufMut) {
        for flag in self.flags {
            buf.put_u32_le(flag);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgSetRestStartResponse {
    pub rest_start: u32,
}

impl ServerWorldPacket for SmsgSetRestStartResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgSetRestStart;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.rest_start);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgSetProficiencyResponse {
    pub item_class: u8,
    pub item_subclass_mask: u32,
}

impl ServerWorldPacket for SmsgSetProficiencyResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgSetProficiency;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.item_class);
        buf.put_u32_le(self.item_subclass_mask);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgInitialSpellsResponse {
    pub spells: Vec<u32>,
}

impl ServerWorldPacket for SmsgInitialSpellsResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgInitialSpells;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u8(0);
        buf.put_u16_le(self.spells.len().min(u16::MAX as usize) as u16);
        for spell in self.spells.iter().take(u16::MAX as usize) {
            buf.put_u16_le(*spell as u16);
            buf.put_u16_le(0);
        }
        buf.put_u16_le(0);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgActionButtonsResponse {
    pub buttons: Vec<u32>,
}

impl ServerWorldPacket for SmsgActionButtonsResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgActionButtons;

    fn write_body(&self, buf: &mut impl BufMut) {
        for button in &self.buttons {
            buf.put_u32_le(*button);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FactionStandingResponse {
    pub flags: u8,
    pub standing: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgInitializeFactionsResponse {
    pub slots: Vec<FactionStandingResponse>,
}

impl ServerWorldPacket for SmsgInitializeFactionsResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgInitializeFactions;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.slots.len() as u32);
        for slot in &self.slots {
            buf.put_u8(slot.flags);
            buf.put_i32_le(slot.standing);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmsgLoginSetTimeSpeedResponse {
    pub packed_server_time: u32,
    pub game_speed: f32,
}

impl ServerWorldPacket for SmsgLoginSetTimeSpeedResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgLoginSetTimeSpeed;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.packed_server_time);
        buf.put_f32_le(self.game_speed);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgTriggerCinematicResponse {
    pub sequence: u32,
}

impl ServerWorldPacket for SmsgTriggerCinematicResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgTriggerCinematic;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.sequence);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldStateResponse {
    pub field: u32,
    pub value: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgInitWorldStatesResponse {
    pub map: u32,
    pub zone: u32,
    pub area: u32,
    pub states: Vec<WorldStateResponse>,
}

impl ServerWorldPacket for SmsgInitWorldStatesResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgInitWorldStates;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.map);
        buf.put_u32_le(self.zone);
        buf.put_u32_le(self.area);
        buf.put_u32_le(self.states.len() as u32);
        for state in &self.states {
            buf.put_u32_le(state.field);
            buf.put_u32_le(state.value);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgNameQueryResponse {
    pub guid: ObjectGuid,
    pub name: String,
    pub realm_name: String,
    pub race: u32,
    pub gender: u32,
    pub class: u32,
}

impl ServerWorldPacket for SmsgNameQueryResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgNameQueryResponse;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.guid.raw());
        write_c_string(buf, &self.name);
        write_c_string(buf, &self.realm_name);
        buf.put_u32_le(self.race);
        buf.put_u32_le(self.gender);
        buf.put_u32_le(self.class);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgMessageChatResponse {
    pub chat_type: u8,
    pub language: u32,
    pub sender: ObjectGuid,
    pub target: Option<ObjectGuid>,
    pub message: String,
    pub tag: u8,
}

impl ServerWorldPacket for SmsgMessageChatResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgMessageChat;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.chat_type);
        buf.put_u32_le(self.language);
        buf.put_u64_le(self.sender.raw());
        if let Some(target) = self.target {
            buf.put_u64_le(target.raw());
        }
        buf.put_u32_le((self.message.len() + 1) as u32);
        write_c_string(buf, &self.message);
        buf.put_u8(self.tag);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgTextEmoteResponse {
    pub sender: ObjectGuid,
    pub text_emote: u32,
    pub emote_num: u32,
    pub target_name: String,
}

impl ServerWorldPacket for SmsgTextEmoteResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgTextEmote;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.sender.raw());
        buf.put_u32_le(self.text_emote);
        buf.put_u32_le(self.emote_num);
        buf.put_u32_le((self.target_name.len() + 1) as u32);
        write_c_string(buf, &self.target_name);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgEmoteResponse {
    pub emote: u32,
    pub sender: ObjectGuid,
}

impl ServerWorldPacket for SmsgEmoteResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgEmote;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.emote);
        buf.put_u64_le(self.sender.raw());
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MsgQueryNextMailTimeResponse {
    pub delay_seconds: f32,
}

impl ServerWorldPacket for MsgQueryNextMailTimeResponse {
    const OPCODE: WorldOpcode = WorldOpcode::MsgQueryNextMailTime;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_f32_le(self.delay_seconds);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgSendMailResultResponse {
    pub mail_id: u32,
    pub action: u32,
    pub error: u32,
    pub equip_error: Option<u32>,
    pub taken_item: Option<(u32, u32)>,
}

impl ServerWorldPacket for SmsgSendMailResultResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgSendMailResult;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.mail_id);
        buf.put_u32_le(self.action);
        buf.put_u32_le(self.error);
        if let Some(equip_error) = self.equip_error {
            buf.put_u32_le(equip_error);
        } else if self.action == 2 {
            let (item, count) = self.taken_item.unwrap_or((0, 0));
            buf.put_u32_le(item);
            buf.put_u32_le(count);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MailListItemResponse {
    pub mail_id: u32,
    pub message_type: u8,
    pub sender_raw_guid: Option<u64>,
    pub sender_entry: Option<u32>,
    pub subject: String,
    pub item_text_id: u32,
    pub package_id: u32,
    pub stationery: u32,
    pub item_entry: u32,
    pub item_enchantment: u32,
    pub item_random_property_id: i32,
    pub item_suffix_factor: u32,
    pub item_count: u8,
    pub item_charges: u32,
    pub item_max_durability: u32,
    pub item_durability: u32,
    pub money: u32,
    pub cod: u32,
    pub checked: u32,
    pub expire_delay_days: f32,
    pub mail_template_id: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SmsgMailListResultResponse {
    pub mails: Vec<MailListItemResponse>,
}

impl ServerWorldPacket for SmsgMailListResultResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgMailListResult;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.mails.len().min(u8::MAX as usize) as u8);
        for mail in self.mails.iter().take(u8::MAX as usize) {
            buf.put_u32_le(mail.mail_id);
            buf.put_u8(mail.message_type);
            if let Some(sender_guid) = mail.sender_raw_guid {
                buf.put_u64_le(sender_guid);
            } else if let Some(sender_entry) = mail.sender_entry {
                buf.put_u32_le(sender_entry);
            }
            write_c_string(buf, &mail.subject);
            buf.put_u32_le(mail.item_text_id);
            buf.put_u32_le(mail.package_id);
            buf.put_u32_le(mail.stationery);
            buf.put_u32_le(mail.item_entry);
            buf.put_u32_le(mail.item_enchantment);
            buf.put_i32_le(mail.item_random_property_id);
            buf.put_u32_le(mail.item_suffix_factor);
            buf.put_u8(mail.item_count);
            buf.put_u32_le(mail.item_charges);
            buf.put_u32_le(mail.item_max_durability);
            buf.put_u32_le(mail.item_durability);
            buf.put_u32_le(mail.money);
            buf.put_u32_le(mail.cod);
            buf.put_u32_le(mail.checked);
            buf.put_f32_le(mail.expire_delay_days);
            buf.put_u32_le(mail.mail_template_id);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgItemTextQueryResponse {
    pub item_text_id: u32,
    pub text: String,
}

impl ServerWorldPacket for SmsgItemTextQueryResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgItemTextQueryResponse;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.item_text_id);
        write_c_string(buf, &self.text);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgReceivedMailResponse {
    pub delay: u32,
}

impl ServerWorldPacket for SmsgReceivedMailResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgReceivedMail;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.delay);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgChannelNotifyResponse {
    pub notice: u8,
    pub channel_name: String,
    pub flags: u32,
    pub channel_id: u32,
}

impl ServerWorldPacket for SmsgChannelNotifyResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgChannelNotify;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.notice);
        write_c_string(buf, &self.channel_name);
        buf.put_u32_le(self.flags);
        buf.put_u32_le(self.channel_id);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgQueryTimeResponse {
    pub unix_time: u32,
}

impl ServerWorldPacket for SmsgQueryTimeResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgQueryTimeResponse;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.unix_time);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgUpdateAccountDataResponse {
    pub account_data_type: u32,
    pub decompressed_size: u32,
    pub compressed_data: Vec<u8>,
}

impl ServerWorldPacket for SmsgUpdateAccountDataResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgUpdateAccountData;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.account_data_type);
        buf.put_u32_le(self.decompressed_size);
        buf.put_slice(&self.compressed_data);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgGmTicketGetTicketResponse {
    pub status: u32,
}

impl ServerWorldPacket for SmsgGmTicketGetTicketResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgGmTicketGetTicket;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.status);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameOnlyResponse {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgGroupInviteResponse {
    pub name: String,
}

impl ServerWorldPacket for SmsgGroupInviteResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgGroupInvite;

    fn write_body(&self, buf: &mut impl BufMut) {
        write_c_string(buf, &self.name);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgGroupSetLeaderResponse {
    pub name: String,
}

impl ServerWorldPacket for SmsgGroupSetLeaderResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgGroupSetLeader;

    fn write_body(&self, buf: &mut impl BufMut) {
        write_c_string(buf, &self.name);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgPartyCommandResultResponse {
    pub operation: u32,
    pub member: String,
    pub result: u32,
}

impl ServerWorldPacket for SmsgPartyCommandResultResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgPartyCommandResult;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.operation);
        write_c_string(buf, &self.member);
        buf.put_u32_le(self.result);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupMemberResponse {
    pub guid: ObjectGuid,
    pub group_flags: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupListMemberResponse {
    pub name: String,
    pub guid: ObjectGuid,
    pub online: u8,
    pub group_flags: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgGroupListResponse {
    pub raid: bool,
    pub receiver_group_flags: u8,
    pub members: Vec<GroupListMemberResponse>,
    pub leader: ObjectGuid,
    pub loot_method: Option<u8>,
    pub master_looter: ObjectGuid,
    pub loot_threshold: u8,
}

impl ServerWorldPacket for SmsgGroupListResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgGroupList;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u8(u8::from(self.raid));
        buf.put_u8(self.receiver_group_flags);
        buf.put_u32_le(self.members.len() as u32);
        for member in &self.members {
            write_c_string(buf, &member.name);
            buf.put_u64_le(member.guid.raw());
            buf.put_u8(member.online);
            buf.put_u8(member.group_flags);
        }
        buf.put_u64_le(self.leader.raw());
        if let Some(loot_method) = self.loot_method {
            buf.put_u8(loot_method);
            buf.put_u64_le(self.master_looter.raw());
            buf.put_u8(self.loot_threshold);
            buf.put_u8(0);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SmsgEmptyGroupListResponse;

impl ServerWorldPacket for SmsgEmptyGroupListResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgGroupList;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_slice(&[0; 24]);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PartyMemberStatsResponse {
    pub requested: ObjectGuid,
    pub update_flags: u32,
    pub status: u8,
    pub health: Option<u16>,
    pub max_health: Option<u16>,
    pub power_type: Option<u8>,
    pub power: Option<u16>,
    pub max_power: Option<u16>,
    pub level: Option<u16>,
    pub map: Option<u16>,
    pub x: Option<u16>,
    pub y: Option<u16>,
    pub aura_mask: Option<u32>,
    pub pet_guid: Option<u16>,
}

impl ServerWorldPacket for PartyMemberStatsResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgPartyMemberStatsFull;

    fn write_body(&self, buf: &mut impl BufMut) {
        put_packed_guid(buf, self.requested);
        buf.put_u32_le(self.update_flags);
        buf.put_u8(self.status);
        if let Some(health) = self.health {
            buf.put_u16_le(health);
            buf.put_u16_le(self.max_health.unwrap_or(0));
            buf.put_u8(self.power_type.unwrap_or(0));
            buf.put_u16_le(self.power.unwrap_or(0));
            buf.put_u16_le(self.max_power.unwrap_or(0));
            buf.put_u16_le(self.level.unwrap_or(0));
            buf.put_u16_le(self.map.unwrap_or(0));
            buf.put_u16_le(self.x.unwrap_or(0));
            buf.put_u16_le(self.y.unwrap_or(0));
            buf.put_u32_le(self.aura_mask.unwrap_or(0));
            buf.put_u16_le(self.pet_guid.unwrap_or(0));
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GossipOption {
    pub option_index: u32,
    pub icon: u8,
    pub coded: u8,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgGossipMessageResponse {
    pub guid: ObjectGuid,
    pub text_id: u32,
    pub options: Vec<GossipOption>,
    pub quest_option_count: u32,
}

impl ServerWorldPacket for SmsgGossipMessageResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgGossipMessage;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.guid.raw());
        buf.put_u32_le(self.text_id);
        buf.put_u32_le(self.options.len() as u32);
        for option in &self.options {
            buf.put_u32_le(option.option_index);
            buf.put_u8(option.icon);
            buf.put_u8(option.coded);
            write_c_string(buf, &option.text);
        }
        buf.put_u32_le(self.quest_option_count);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgNpcTextUpdateResponse {
    pub text_id: u32,
    pub primary_text: String,
}

impl ServerWorldPacket for SmsgNpcTextUpdateResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgNpcTextUpdate;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.text_id);
        for index in 0..8 {
            buf.put_f32_le(if index == 0 { 1.0 } else { 0.0 });
            let text = if index == 0 { &self.primary_text } else { "" };
            write_c_string(buf, text);
            write_c_string(buf, text);
            buf.put_u32_le(0);
            for _ in 0..3 {
                buf.put_u32_le(0);
                buf.put_u32_le(0);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgReadItemOkResponse {
    pub item: ObjectGuid,
}

impl ServerWorldPacket for SmsgReadItemOkResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgReadItemOk;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.item.raw());
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgReadItemFailedResponse {
    pub item: ObjectGuid,
}

impl ServerWorldPacket for SmsgReadItemFailedResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgReadItemFailed;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.item.raw());
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgPageTextQueryResponse {
    pub page_text_id: u32,
    pub text: String,
    pub next_page_text_id: u32,
}

impl ServerWorldPacket for SmsgPageTextQueryResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgPageTextQueryResponse;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.page_text_id);
        write_c_string(buf, &self.text);
        buf.put_u32_le(self.next_page_text_id);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgExplorationExperienceResponse {
    pub area: u32,
    pub experience: u32,
}

impl ServerWorldPacket for SmsgExplorationExperienceResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgExplorationExperience;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.area);
        buf.put_u32_le(self.experience);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VendorListItemResponse {
    pub item: u32,
    pub display: u32,
    pub max_count: u32,
    pub price: u32,
    pub durability: u32,
    pub buy_count: u32,
}

impl VendorListItemResponse {
    fn available_count(self) -> u32 {
        if self.max_count == 0 {
            u32::MAX
        } else {
            self.max_count
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgListInventoryResponse {
    pub vendor_guid: ObjectGuid,
    pub items: Vec<VendorListItemResponse>,
}

impl ServerWorldPacket for SmsgListInventoryResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgListInventory;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.vendor_guid.raw());
        if self.items.is_empty() {
            buf.put_u8(0);
            buf.put_u8(0);
            return;
        }
        buf.put_u8(self.items.len().min(128) as u8);
        for (index, item) in self.items.iter().take(128).enumerate() {
            buf.put_u32_le((index + 1) as u32);
            buf.put_u32_le(item.item);
            buf.put_u32_le(item.display);
            buf.put_u32_le(item.available_count());
            buf.put_u32_le(item.price);
            buf.put_u32_le(item.durability);
            buf.put_u32_le(item.buy_count);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgBuyItemResponse {
    pub vendor_guid: ObjectGuid,
    pub vendor_slot: u32,
    pub remaining_count: u32,
    pub count: u8,
}

impl ServerWorldPacket for SmsgBuyItemResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgBuyItem;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.vendor_guid.raw());
        buf.put_u32_le(self.vendor_slot);
        buf.put_u32_le(self.remaining_count);
        buf.put_u32_le(self.count as u32);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgInventoryChangeFailureResponse {
    pub result: u8,
    pub required_level: Option<u32>,
    pub item_guid: Option<ObjectGuid>,
    pub item2_guid: Option<ObjectGuid>,
}

impl ServerWorldPacket for SmsgInventoryChangeFailureResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgInventoryChangeFailure;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.result);
        if let Some(required_level) = self.required_level {
            buf.put_u32_le(required_level);
        }
        buf.put_u64_le(self.item_guid.map(ObjectGuid::raw).unwrap_or(0));
        buf.put_u64_le(self.item2_guid.map(ObjectGuid::raw).unwrap_or(0));
        buf.put_u8(0);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgBuyFailedResponse {
    pub vendor_guid: ObjectGuid,
    pub item: u32,
    pub result: u8,
}

impl ServerWorldPacket for SmsgBuyFailedResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgBuyFailed;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.vendor_guid.raw());
        buf.put_u32_le(self.item);
        buf.put_u8(self.result);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActivateTaxiRequest {
    pub raw_guid: u64,
    pub source_node: u32,
    pub destination_node: u32,
}

impl ActivateTaxiRequest {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        ensure_exact_remaining(buf, 16, "CMSG_ACTIVATETAXI")?;
        Ok(Self {
            raw_guid: buf.get_u64_le(),
            source_node: buf.get_u32_le(),
            destination_node: buf.get_u32_le(),
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.raw_guid);
        buf.put_u32_le(self.source_node);
        buf.put_u32_le(self.destination_node);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgTaxiNodeStatusResponse {
    pub taxi_master: ObjectGuid,
    pub known: bool,
}

impl ServerWorldPacket for SmsgTaxiNodeStatusResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgTaxiNodeStatus;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.taxi_master.raw());
        buf.put_u8(u8::from(self.known));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgShowTaxiNodesResponse {
    pub taxi_master: ObjectGuid,
    pub current_node: u32,
    pub taximask: [u32; 8],
}

impl ServerWorldPacket for SmsgShowTaxiNodesResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgShowTaxiNodes;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(1);
        buf.put_u64_le(self.taxi_master.raw());
        buf.put_u32_le(self.current_node);
        for mask in self.taximask {
            buf.put_u32_le(mask);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgActivateTaxiReplyResponse {
    pub reply: u32,
}

impl ServerWorldPacket for SmsgActivateTaxiReplyResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgActivateTaxiReply;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.reply);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgNewTaxiPathResponse;

impl ServerWorldPacket for SmsgNewTaxiPathResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgNewTaxiPath;

    fn write_body(&self, _buf: &mut impl BufMut) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgBinderConfirmResponse {
    pub innkeeper: ObjectGuid,
}

impl ServerWorldPacket for SmsgBinderConfirmResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgBinderConfirm;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.innkeeper.raw());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgSellItemResponse {
    pub vendor_guid: ObjectGuid,
    pub item_guid: ObjectGuid,
    pub result: u8,
}

impl ServerWorldPacket for SmsgSellItemResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgSellItem;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.vendor_guid.raw());
        buf.put_u64_le(self.item_guid.raw());
        buf.put_u8(self.result);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrainerListSpellResponse {
    pub spell: u32,
    pub state: u8,
    pub cost: u32,
    pub req_level: u8,
    pub req_skill: u32,
    pub req_skill_value: u32,
    pub req_ability: [u32; 3],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgTrainerListResponse {
    pub trainer: ObjectGuid,
    pub trainer_type: u32,
    pub spells: Vec<TrainerListSpellResponse>,
    pub greeting: String,
}

impl ServerWorldPacket for SmsgTrainerListResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgTrainerList;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.trainer.raw());
        buf.put_u32_le(self.trainer_type);
        buf.put_u32_le(self.spells.len() as u32);
        for spell in &self.spells {
            buf.put_u32_le(spell.spell);
            buf.put_u8(spell.state);
            buf.put_u32_le(spell.cost);
            buf.put_u32_le(0);
            buf.put_u32_le(0);
            buf.put_u8(spell.req_level);
            buf.put_u32_le(spell.req_skill);
            buf.put_u32_le(spell.req_skill_value);
            for ability in spell.req_ability {
                buf.put_u32_le(ability);
            }
        }
        write_c_string(buf, &self.greeting);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgTrainerBuySucceededResponse {
    pub trainer: ObjectGuid,
    pub spell: u32,
}

impl ServerWorldPacket for SmsgTrainerBuySucceededResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgTrainerBuySucceeded;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.trainer.raw());
        buf.put_u32_le(self.spell);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgTrainerBuyFailedResponse {
    pub trainer: ObjectGuid,
    pub spell: u32,
    pub reason: u32,
}

impl ServerWorldPacket for SmsgTrainerBuyFailedResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgTrainerBuyFailed;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.trainer.raw());
        buf.put_u32_le(self.spell);
        buf.put_u32_le(self.reason);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgShowBankResponse {
    pub banker: ObjectGuid,
}

impl ServerWorldPacket for SmsgShowBankResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgShowBank;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.banker.raw());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgBuyBankSlotResultResponse {
    pub result: u32,
}

impl ServerWorldPacket for SmsgBuyBankSlotResultResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgBuyBankSlotResult;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.result);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgLearnedSpellResponse {
    pub spell: u32,
}

impl ServerWorldPacket for SmsgLearnedSpellResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgLearnedSpell;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.spell);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgPlaySpellVisualResponse {
    pub guid: ObjectGuid,
    pub spell_visual_kit: u32,
}

impl ServerWorldPacket for SmsgPlaySpellVisualResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgPlaySpellVisual;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.guid.raw());
        buf.put_u32_le(self.spell_visual_kit);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgPlaySpellImpactResponse {
    pub guid: ObjectGuid,
    pub spell_visual_kit: u32,
}

impl ServerWorldPacket for SmsgPlaySpellImpactResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgPlaySpellImpact;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.guid.raw());
        buf.put_u32_le(self.spell_visual_kit);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestRewardItem {
    pub item_id: u32,
    pub count: u32,
    pub display_id: u32,
}

fn write_quest_reward_items(buf: &mut impl BufMut, items: &[QuestRewardItem]) {
    buf.put_u32_le(items.len() as u32);
    for item in items {
        buf.put_u32_le(item.item_id);
        buf.put_u32_le(item.count);
        buf.put_u32_le(item.display_id);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestListResponseItem {
    pub quest_id: u32,
    pub dialog_status: u32,
    pub quest_level: u32,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgQuestgiverStatusResponse {
    pub guid: ObjectGuid,
    pub status: u32,
}

impl ServerWorldPacket for SmsgQuestgiverStatusResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgQuestgiverStatus;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.guid.raw());
        buf.put_u32_le(self.status);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgQuestgiverQuestListResponse {
    pub guid: ObjectGuid,
    pub greeting: String,
    pub player_emote_delay: u32,
    pub npc_emote: u32,
    pub quests: Vec<QuestListResponseItem>,
}

impl ServerWorldPacket for SmsgQuestgiverQuestListResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgQuestgiverQuestList;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.guid.raw());
        write_c_string(buf, &self.greeting);
        buf.put_u32_le(self.player_emote_delay);
        buf.put_u32_le(self.npc_emote);
        buf.put_u8(self.quests.len().min(u8::MAX as usize) as u8);
        for item in self.quests.iter().take(u8::MAX as usize) {
            buf.put_u32_le(item.quest_id);
            buf.put_u32_le(item.dialog_status);
            buf.put_u32_le(item.quest_level);
            write_c_string(buf, &item.title);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestDetailsEmote {
    pub emote: u32,
    pub delay: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgQuestgiverQuestDetailsResponse {
    pub guid: ObjectGuid,
    pub quest_id: u32,
    pub title: String,
    pub details: String,
    pub objectives: String,
    pub activate_accept: u32,
    pub choice_items: Vec<QuestRewardItem>,
    pub reward_items: Vec<QuestRewardItem>,
    pub reward_money: u32,
    pub reward_spell: u32,
    pub emotes: Vec<QuestDetailsEmote>,
}

impl ServerWorldPacket for SmsgQuestgiverQuestDetailsResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgQuestgiverQuestDetails;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.guid.raw());
        buf.put_u32_le(self.quest_id);
        write_c_string(buf, &self.title);
        write_c_string(buf, &self.details);
        write_c_string(buf, &self.objectives);
        buf.put_u32_le(self.activate_accept);
        write_quest_reward_items(buf, &self.choice_items);
        write_quest_reward_items(buf, &self.reward_items);
        buf.put_u32_le(self.reward_money);
        buf.put_u32_le(self.reward_spell);
        buf.put_u32_le(self.emotes.len() as u32);
        for emote in &self.emotes {
            buf.put_u32_le(emote.emote);
            buf.put_u32_le(emote.delay);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuestPoint {
    pub map_id: u32,
    pub x: f32,
    pub y: f32,
    pub opt: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestObjectiveRequirement {
    pub wire_entry: u32,
    pub required_count: u32,
    pub item_id: u32,
    pub item_count: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SmsgQuestQueryResponse {
    pub quest_id: u32,
    pub method: u32,
    pub quest_level: u32,
    pub zone_or_sort: u32,
    pub quest_type: u32,
    pub rep_objective_faction: u32,
    pub rep_objective_value: u32,
    pub next_quest_in_chain: u32,
    pub reward_money: u32,
    pub reward_money_max_level: u32,
    pub reward_spell: u32,
    pub source_item_id: u32,
    pub quest_flags: u32,
    pub reward_items: [u32; 4],
    pub reward_item_counts: [u32; 4],
    pub choice_items: [u32; 6],
    pub choice_item_counts: [u32; 6],
    pub point: QuestPoint,
    pub title: String,
    pub objectives: String,
    pub details: String,
    pub end_text: String,
    pub requirements: [QuestObjectiveRequirement; 4],
    pub objective_text: [String; 4],
}

impl ServerWorldPacket for SmsgQuestQueryResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgQuestQueryResponse;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.quest_id);
        buf.put_u32_le(self.method);
        buf.put_u32_le(self.quest_level);
        buf.put_u32_le(self.zone_or_sort);
        buf.put_u32_le(self.quest_type);
        buf.put_u32_le(self.rep_objective_faction);
        buf.put_u32_le(self.rep_objective_value);
        buf.put_u32_le(0);
        buf.put_u32_le(0);
        buf.put_u32_le(self.next_quest_in_chain);
        buf.put_u32_le(self.reward_money);
        buf.put_u32_le(self.reward_money_max_level);
        buf.put_u32_le(self.reward_spell);
        buf.put_u32_le(self.source_item_id);
        buf.put_u32_le(self.quest_flags);
        for index in 0..4 {
            buf.put_u32_le(self.reward_items[index]);
            buf.put_u32_le(self.reward_item_counts[index]);
        }
        for index in 0..6 {
            buf.put_u32_le(self.choice_items[index]);
            buf.put_u32_le(self.choice_item_counts[index]);
        }
        buf.put_u32_le(self.point.map_id);
        buf.put_f32_le(self.point.x);
        buf.put_f32_le(self.point.y);
        buf.put_u32_le(self.point.opt);
        write_c_string(buf, &self.title);
        write_c_string(buf, &self.objectives);
        write_c_string(buf, &self.details);
        write_c_string(buf, &self.end_text);
        for requirement in &self.requirements {
            buf.put_u32_le(requirement.wire_entry);
            buf.put_u32_le(requirement.required_count);
            buf.put_u32_le(requirement.item_id);
            buf.put_u32_le(requirement.item_count);
        }
        for text in &self.objective_text {
            write_c_string(buf, text);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgQuestgiverRequestItemsResponse {
    pub guid: ObjectGuid,
    pub quest_id: u32,
    pub title: String,
    pub request_items_text: String,
    pub emote_delay: u32,
    pub emote: u32,
    pub close_on_cancel: u32,
    pub required_money: u32,
    pub required_items: Vec<QuestRewardItem>,
    pub required_reward_button: u32,
    pub complete_reward_button: u32,
    pub incomplete_reward_button: u32,
    pub completion_style: u32,
}

impl ServerWorldPacket for SmsgQuestgiverRequestItemsResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgQuestgiverRequestItems;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.guid.raw());
        buf.put_u32_le(self.quest_id);
        write_c_string(buf, &self.title);
        write_c_string(buf, &self.request_items_text);
        buf.put_u32_le(self.emote_delay);
        buf.put_u32_le(self.emote);
        buf.put_u32_le(self.close_on_cancel);
        buf.put_u32_le(self.required_money);
        write_quest_reward_items(buf, &self.required_items);
        buf.put_u32_le(self.required_reward_button);
        buf.put_u32_le(self.complete_reward_button);
        buf.put_u32_le(self.incomplete_reward_button);
        buf.put_u32_le(self.completion_style);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestOfferRewardEmote {
    pub delay: u32,
    pub emote: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgQuestgiverOfferRewardResponse {
    pub guid: ObjectGuid,
    pub quest_id: u32,
    pub title: String,
    pub offer_reward_text: String,
    pub enable_next: u32,
    pub emotes: Vec<QuestOfferRewardEmote>,
    pub choice_items: Vec<QuestRewardItem>,
    pub reward_items: Vec<QuestRewardItem>,
    pub reward_money: u32,
    pub reward_spell: u32,
    pub reward_spell_cast: u32,
}

impl ServerWorldPacket for SmsgQuestgiverOfferRewardResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgQuestgiverOfferReward;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.guid.raw());
        buf.put_u32_le(self.quest_id);
        write_c_string(buf, &self.title);
        write_c_string(buf, &self.offer_reward_text);
        buf.put_u32_le(self.enable_next);
        buf.put_u32_le(self.emotes.len() as u32);
        for emote in &self.emotes {
            buf.put_u32_le(emote.delay);
            buf.put_u32_le(emote.emote);
        }
        write_quest_reward_items(buf, &self.choice_items);
        write_quest_reward_items(buf, &self.reward_items);
        buf.put_u32_le(self.reward_money);
        buf.put_u32_le(self.reward_spell);
        buf.put_u32_le(self.reward_spell_cast);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestCompleteRewardItem {
    pub item_id: u32,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgQuestgiverQuestCompleteResponse {
    pub quest_id: u32,
    pub completion_type: u32,
    pub reward_xp: u32,
    pub reward_money: u32,
    pub reward_items: Vec<QuestCompleteRewardItem>,
}

impl ServerWorldPacket for SmsgQuestgiverQuestCompleteResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgQuestgiverQuestComplete;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.quest_id);
        buf.put_u32_le(self.completion_type);
        buf.put_u32_le(self.reward_xp);
        buf.put_u32_le(self.reward_money);
        buf.put_u32_le(self.reward_items.len() as u32);
        for item in &self.reward_items {
            buf.put_u32_le(item.item_id);
            buf.put_u32_le(item.count);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsgQuestUpdateAddKillResponse {
    pub quest_id: u32,
    pub objective: u32,
    pub count: u32,
    pub required_count: u32,
    pub killed_guid: ObjectGuid,
}

impl ServerWorldPacket for SmsgQuestUpdateAddKillResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgQuestUpdateAddKill;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.quest_id);
        buf.put_u32_le(self.objective);
        buf.put_u32_le(self.count);
        buf.put_u32_le(self.required_count);
        buf.put_u64_le(self.killed_guid.raw());
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsgUpdateObjectResponse {
    pub blocks: Vec<Vec<u8>>,
}

impl ServerWorldPacket for SmsgUpdateObjectResponse {
    const OPCODE: WorldOpcode = WorldOpcode::SmsgUpdateObject;

    fn write_body(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.blocks.len() as u32);
        buf.put_u8(0);
        for block in &self.blocks {
            buf.put_slice(block);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;

    #[test]
    fn ping_request_roundtrip() {
        let request = PingRequest {
            sequence: 0xAABBCCDD,
        };
        let mut bytes = BytesMut::new();
        request.write(&mut bytes);

        let decoded = PingRequest::read(&mut &bytes[..]).unwrap();
        assert_eq!(decoded, request);
    }

    #[test]
    fn pong_response_roundtrip() {
        let response = PongResponse {
            sequence: 0x01020304,
        };
        let mut bytes = BytesMut::new();
        response.write(&mut bytes);

        let decoded = PongResponse::read(&mut &bytes[..]).unwrap();
        assert_eq!(decoded, response);
        assert_eq!(response.to_body(), [0x04, 0x03, 0x02, 0x01]);
        assert_eq!(response.body(), response.to_body());
        assert_eq!(
            PongResponse::OPCODE,
            <PongResponse as ServerWorldPacket>::OPCODE
        );
    }

    #[test]
    fn gossip_message_response_writes_cmangos_layout() {
        let response = SmsgGossipMessageResponse {
            guid: ObjectGuid::from_raw(0x1122_3344_5566_7788),
            text_id: 42,
            options: vec![GossipOption {
                option_index: 7,
                icon: 1,
                coded: 0,
                text: "Train me".to_string(),
            }],
            quest_option_count: 0,
        };

        let mut expected = Vec::new();
        expected.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
        expected.extend_from_slice(&42u32.to_le_bytes());
        expected.extend_from_slice(&1u32.to_le_bytes());
        expected.extend_from_slice(&7u32.to_le_bytes());
        expected.push(1);
        expected.push(0);
        expected.extend_from_slice(b"Train me\0");
        expected.extend_from_slice(&0u32.to_le_bytes());

        assert_eq!(response.body(), expected);
    }

    #[test]
    fn npc_text_update_response_writes_primary_text_slot() {
        let response = SmsgNpcTextUpdateResponse {
            text_id: 9,
            primary_text: "Hello".to_string(),
        };

        let body = response.body();
        let mut expected_prefix = Vec::new();
        expected_prefix.extend_from_slice(&9u32.to_le_bytes());
        expected_prefix.extend_from_slice(&1.0f32.to_le_bytes());
        expected_prefix.extend_from_slice(b"Hello\0Hello\0");

        assert!(body.starts_with(&expected_prefix));
        assert_eq!(body.len(), 4 + 8 * (4 + 1 + 1 + 4 + 24) + 10);
    }

    #[test]
    fn list_inventory_response_writes_empty_and_item_layouts() {
        let vendor_guid = ObjectGuid::from_raw(0x0102_0304_0506_0708);
        let empty = SmsgListInventoryResponse {
            vendor_guid,
            items: Vec::new(),
        };
        let mut expected_empty = Vec::new();
        expected_empty.extend_from_slice(&vendor_guid.raw().to_le_bytes());
        expected_empty.extend_from_slice(&[0, 0]);
        assert_eq!(empty.body(), expected_empty);

        let response = SmsgListInventoryResponse {
            vendor_guid,
            items: vec![VendorListItemResponse {
                item: 25,
                display: 99,
                max_count: 0,
                price: 7,
                durability: 12,
                buy_count: 1,
            }],
        };
        let mut expected = Vec::new();
        expected.extend_from_slice(&vendor_guid.raw().to_le_bytes());
        expected.push(1);
        expected.extend_from_slice(&1u32.to_le_bytes());
        expected.extend_from_slice(&25u32.to_le_bytes());
        expected.extend_from_slice(&99u32.to_le_bytes());
        expected.extend_from_slice(&u32::MAX.to_le_bytes());
        expected.extend_from_slice(&7u32.to_le_bytes());
        expected.extend_from_slice(&12u32.to_le_bytes());
        expected.extend_from_slice(&1u32.to_le_bytes());
        assert_eq!(response.body(), expected);
    }

    #[test]
    fn taxi_packets_match_cmangos_layouts() {
        let taxi_master = ObjectGuid::from_raw(0x1122_3344_5566_7788);
        let request = ActivateTaxiRequest {
            raw_guid: taxi_master.raw(),
            source_node: 2,
            destination_node: 3,
        };
        let mut bytes = BytesMut::new();
        request.write(&mut bytes);
        assert_eq!(ActivateTaxiRequest::read(&mut &bytes[..]).unwrap(), request);

        let mut expected_status = Vec::new();
        expected_status.extend_from_slice(&taxi_master.raw().to_le_bytes());
        expected_status.push(1);
        assert_eq!(
            SmsgTaxiNodeStatusResponse {
                taxi_master,
                known: true
            }
            .body(),
            expected_status
        );

        let menu = SmsgShowTaxiNodesResponse {
            taxi_master,
            current_node: 2,
            taximask: [1, 2, 3, 4, 5, 6, 7, 8],
        };
        let mut expected_menu = Vec::new();
        expected_menu.extend_from_slice(&1u32.to_le_bytes());
        expected_menu.extend_from_slice(&taxi_master.raw().to_le_bytes());
        expected_menu.extend_from_slice(&2u32.to_le_bytes());
        for mask in 1u32..=8 {
            expected_menu.extend_from_slice(&mask.to_le_bytes());
        }
        assert_eq!(menu.body(), expected_menu);

        assert_eq!(SmsgNewTaxiPathResponse.body(), Vec::<u8>::new());
        assert_eq!(
            SmsgActivateTaxiReplyResponse { reply: 6 }.body(),
            6u32.to_le_bytes()
        );
        assert_eq!(
            SmsgBinderConfirmResponse {
                innkeeper: taxi_master
            }
            .body(),
            {
                let mut expected = Vec::new();
                expected.extend_from_slice(&taxi_master.raw().to_le_bytes());
                expected
            }
        );
        assert_eq!(
            SmsgPlayerBoundResponse {
                caster: taxi_master,
                area: 87,
            }
            .body(),
            {
                let mut expected = Vec::new();
                expected.extend_from_slice(&taxi_master.raw().to_le_bytes());
                expected.extend_from_slice(&87u32.to_le_bytes());
                expected
            }
        );
    }

    #[test]
    fn mail_result_response_matches_cmangos_optional_payloads() {
        let ok = SmsgSendMailResultResponse {
            mail_id: 7,
            action: 0,
            error: 0,
            equip_error: None,
            taken_item: None,
        };
        let mut expected = Vec::new();
        expected.extend_from_slice(&7u32.to_le_bytes());
        expected.extend_from_slice(&0u32.to_le_bytes());
        expected.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(ok.body(), expected);

        let item_taken = SmsgSendMailResultResponse {
            mail_id: 8,
            action: 2,
            error: 0,
            equip_error: None,
            taken_item: Some((6948, 1)),
        };
        expected.clear();
        expected.extend_from_slice(&8u32.to_le_bytes());
        expected.extend_from_slice(&2u32.to_le_bytes());
        expected.extend_from_slice(&0u32.to_le_bytes());
        expected.extend_from_slice(&6948u32.to_le_bytes());
        expected.extend_from_slice(&1u32.to_le_bytes());
        assert_eq!(item_taken.body(), expected);
    }

    #[test]
    fn mail_list_response_writes_vanilla_single_attachment_layout() {
        let sender = ObjectGuid::new(wow_common::guid::HighGuid::Player, 0, 55);
        let response = SmsgMailListResultResponse {
            mails: vec![MailListItemResponse {
                mail_id: 9,
                message_type: 0,
                sender_raw_guid: Some(sender.raw()),
                sender_entry: None,
                subject: "subject".to_string(),
                item_text_id: 12,
                package_id: 0,
                stationery: 41,
                item_entry: 6948,
                item_enchantment: 0,
                item_random_property_id: -7,
                item_suffix_factor: 0,
                item_count: 1,
                item_charges: 0,
                item_max_durability: 10,
                item_durability: 8,
                money: 123,
                cod: 0,
                checked: 0x10,
                expire_delay_days: 29.5,
                mail_template_id: 0,
            }],
        };

        let body = response.body();
        let mut cursor = 0;
        assert_eq!(body[cursor], 1);
        cursor += 1;
        assert_eq!(
            u32::from_le_bytes(body[cursor..cursor + 4].try_into().unwrap()),
            9
        );
        cursor += 5;
        assert_eq!(
            u64::from_le_bytes(body[cursor..cursor + 8].try_into().unwrap()),
            sender.raw()
        );
        cursor += 8;
        assert_eq!(&body[cursor..cursor + 8], b"subject\0");
        cursor += 8 + 4 + 4 + 4;
        assert_eq!(
            u32::from_le_bytes(body[cursor..cursor + 4].try_into().unwrap()),
            6948
        );
    }

    #[test]
    fn inventory_change_failure_response_writes_optional_required_level() {
        let item = ObjectGuid::from_raw(0x0102_0304_0506_0708);
        let response = SmsgInventoryChangeFailureResponse {
            result: 1,
            required_level: Some(6),
            item_guid: Some(item),
            item2_guid: None,
        };
        let mut expected = vec![1];
        expected.extend_from_slice(&6u32.to_le_bytes());
        expected.extend_from_slice(&item.raw().to_le_bytes());
        expected.extend_from_slice(&0u64.to_le_bytes());
        expected.push(0);
        assert_eq!(response.body(), expected);
    }

    #[test]
    fn trainer_list_response_writes_spell_rows_and_greeting() {
        let trainer = ObjectGuid::from_raw(0x8877_6655_4433_2211);
        let response = SmsgTrainerListResponse {
            trainer,
            trainer_type: 0,
            spells: vec![TrainerListSpellResponse {
                spell: 6673,
                state: 0,
                cost: 10,
                req_level: 1,
                req_skill: 0,
                req_skill_value: 0,
                req_ability: [0, 0, 0],
            }],
            greeting: "Ready.".to_string(),
        };
        let mut expected = Vec::new();
        expected.extend_from_slice(&trainer.raw().to_le_bytes());
        expected.extend_from_slice(&0u32.to_le_bytes());
        expected.extend_from_slice(&1u32.to_le_bytes());
        expected.extend_from_slice(&6673u32.to_le_bytes());
        expected.push(0);
        expected.extend_from_slice(&10u32.to_le_bytes());
        expected.extend_from_slice(&0u32.to_le_bytes());
        expected.extend_from_slice(&0u32.to_le_bytes());
        expected.push(1);
        expected.extend_from_slice(&0u32.to_le_bytes());
        expected.extend_from_slice(&0u32.to_le_bytes());
        expected.extend_from_slice(&0u32.to_le_bytes());
        expected.extend_from_slice(&0u32.to_le_bytes());
        expected.extend_from_slice(&0u32.to_le_bytes());
        expected.extend_from_slice(b"Ready.\0");

        assert_eq!(response.body(), expected);
    }

    #[test]
    fn questgiver_quest_list_response_writes_dialog_rows() {
        let guid = ObjectGuid::from_raw(0x0102_0304_0506_0708);
        let response = SmsgQuestgiverQuestListResponse {
            guid,
            greeting: "Greetings.".to_string(),
            player_emote_delay: 0,
            npc_emote: 0,
            quests: vec![QuestListResponseItem {
                quest_id: 33,
                dialog_status: 4,
                quest_level: 6,
                title: "Wolves".to_string(),
            }],
        };
        let mut expected = Vec::new();
        expected.extend_from_slice(&guid.raw().to_le_bytes());
        expected.extend_from_slice(b"Greetings.\0");
        expected.extend_from_slice(&0u32.to_le_bytes());
        expected.extend_from_slice(&0u32.to_le_bytes());
        expected.push(1);
        expected.extend_from_slice(&33u32.to_le_bytes());
        expected.extend_from_slice(&4u32.to_le_bytes());
        expected.extend_from_slice(&6u32.to_le_bytes());
        expected.extend_from_slice(b"Wolves\0");

        assert_eq!(response.body(), expected);
    }

    #[test]
    fn questgiver_details_response_writes_reward_items_and_emotes() {
        let guid = ObjectGuid::from_raw(0x1111_2222_3333_4444);
        let response = SmsgQuestgiverQuestDetailsResponse {
            guid,
            quest_id: 12,
            title: "Title".to_string(),
            details: "Details".to_string(),
            objectives: "Objectives".to_string(),
            activate_accept: 1,
            choice_items: vec![QuestRewardItem {
                item_id: 25,
                count: 1,
                display_id: 99,
            }],
            reward_items: Vec::new(),
            reward_money: 7,
            reward_spell: 0,
            emotes: vec![QuestDetailsEmote { emote: 1, delay: 2 }],
        };
        let mut expected = Vec::new();
        expected.extend_from_slice(&guid.raw().to_le_bytes());
        expected.extend_from_slice(&12u32.to_le_bytes());
        expected.extend_from_slice(b"Title\0Details\0Objectives\0");
        expected.extend_from_slice(&1u32.to_le_bytes());
        expected.extend_from_slice(&1u32.to_le_bytes());
        expected.extend_from_slice(&25u32.to_le_bytes());
        expected.extend_from_slice(&1u32.to_le_bytes());
        expected.extend_from_slice(&99u32.to_le_bytes());
        expected.extend_from_slice(&0u32.to_le_bytes());
        expected.extend_from_slice(&7u32.to_le_bytes());
        expected.extend_from_slice(&0u32.to_le_bytes());
        expected.extend_from_slice(&1u32.to_le_bytes());
        expected.extend_from_slice(&1u32.to_le_bytes());
        expected.extend_from_slice(&2u32.to_le_bytes());

        assert_eq!(response.body(), expected);
    }

    #[test]
    fn quest_complete_and_kill_responses_write_expected_layouts() {
        let complete = SmsgQuestgiverQuestCompleteResponse {
            quest_id: 8,
            completion_type: 3,
            reward_xp: 40,
            reward_money: 12,
            reward_items: vec![QuestCompleteRewardItem {
                item_id: 25,
                count: 2,
            }],
        };
        let mut expected_complete = Vec::new();
        expected_complete.extend_from_slice(&8u32.to_le_bytes());
        expected_complete.extend_from_slice(&3u32.to_le_bytes());
        expected_complete.extend_from_slice(&40u32.to_le_bytes());
        expected_complete.extend_from_slice(&12u32.to_le_bytes());
        expected_complete.extend_from_slice(&1u32.to_le_bytes());
        expected_complete.extend_from_slice(&25u32.to_le_bytes());
        expected_complete.extend_from_slice(&2u32.to_le_bytes());
        assert_eq!(complete.body(), expected_complete);

        let guid = ObjectGuid::from_raw(0xAABB_CCDD_EEFF_0011);
        let kill = SmsgQuestUpdateAddKillResponse {
            quest_id: 8,
            objective: 0x8000_0006,
            count: 3,
            required_count: 6,
            killed_guid: guid,
        };
        let mut expected_kill = Vec::new();
        expected_kill.extend_from_slice(&8u32.to_le_bytes());
        expected_kill.extend_from_slice(&0x8000_0006u32.to_le_bytes());
        expected_kill.extend_from_slice(&3u32.to_le_bytes());
        expected_kill.extend_from_slice(&6u32.to_le_bytes());
        expected_kill.extend_from_slice(&guid.raw().to_le_bytes());
        assert_eq!(kill.body(), expected_kill);
    }

    #[test]
    fn update_object_response_wraps_prebuilt_blocks() {
        let response = SmsgUpdateObjectResponse {
            blocks: vec![vec![0x01, 0x02], vec![0x03]],
        };
        assert_eq!(response.body(), vec![2, 0, 0, 0, 0, 1, 2, 3]);
    }

    #[test]
    fn session_bootstrap_responses_write_expected_layouts() {
        let login = SmsgLoginVerifyWorldResponse {
            map: 1,
            x: 2.0,
            y: 3.0,
            z: 4.0,
            orientation: 5.0,
        };
        let mut expected_login = Vec::new();
        expected_login.extend_from_slice(&1u32.to_le_bytes());
        expected_login.extend_from_slice(&2.0f32.to_le_bytes());
        expected_login.extend_from_slice(&3.0f32.to_le_bytes());
        expected_login.extend_from_slice(&4.0f32.to_le_bytes());
        expected_login.extend_from_slice(&5.0f32.to_le_bytes());
        assert_eq!(login.body(), expected_login);

        let spells = SmsgInitialSpellsResponse {
            spells: vec![6673, 78],
        };
        assert_eq!(
            spells.body(),
            vec![0, 2, 0, 17, 26, 0, 0, 78, 0, 0, 0, 0, 0]
        );

        let actions = SmsgActionButtonsResponse {
            buttons: vec![0x0102_0304, 0],
        };
        assert_eq!(actions.body(), vec![4, 3, 2, 1, 0, 0, 0, 0]);

        let factions = SmsgInitializeFactionsResponse {
            slots: vec![FactionStandingResponse {
                flags: 1,
                standing: -42,
            }],
        };
        let mut expected_factions = Vec::new();
        expected_factions.extend_from_slice(&1u32.to_le_bytes());
        expected_factions.push(1);
        expected_factions.extend_from_slice(&(-42i32).to_le_bytes());
        assert_eq!(factions.body(), expected_factions);
    }

    #[test]
    fn chat_and_group_responses_write_expected_layouts() {
        let sender = ObjectGuid::from_raw(0x0102_0304_0506_0708);
        let chat = SmsgMessageChatResponse {
            chat_type: 0,
            language: 7,
            sender,
            target: Some(sender),
            message: "hi".to_string(),
            tag: 0,
        };
        let mut expected_chat = vec![0];
        expected_chat.extend_from_slice(&7u32.to_le_bytes());
        expected_chat.extend_from_slice(&sender.raw().to_le_bytes());
        expected_chat.extend_from_slice(&sender.raw().to_le_bytes());
        expected_chat.extend_from_slice(&3u32.to_le_bytes());
        expected_chat.extend_from_slice(b"hi\0");
        expected_chat.push(0);
        assert_eq!(chat.body(), expected_chat);

        let command = SmsgPartyCommandResultResponse {
            operation: 2,
            member: "Alyx".to_string(),
            result: 0,
        };
        let mut expected_command = Vec::new();
        expected_command.extend_from_slice(&2u32.to_le_bytes());
        expected_command.extend_from_slice(b"Alyx\0");
        expected_command.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(command.body(), expected_command);

        let group = SmsgGroupListResponse {
            raid: false,
            receiver_group_flags: 0,
            members: vec![GroupListMemberResponse {
                name: "Breen".to_string(),
                guid: sender,
                online: 1,
                group_flags: 0,
            }],
            leader: sender,
            loot_method: Some(2),
            master_looter: sender,
            loot_threshold: 3,
        };
        let mut expected_group = vec![0, 0];
        expected_group.extend_from_slice(&1u32.to_le_bytes());
        expected_group.extend_from_slice(b"Breen\0");
        expected_group.extend_from_slice(&sender.raw().to_le_bytes());
        expected_group.extend_from_slice(&[1, 0]);
        expected_group.extend_from_slice(&sender.raw().to_le_bytes());
        expected_group.push(2);
        expected_group.extend_from_slice(&sender.raw().to_le_bytes());
        expected_group.extend_from_slice(&[3, 0]);
        assert_eq!(group.body(), expected_group);
    }

    #[test]
    fn movement_death_progression_responses_write_expected_layouts() {
        let guid = ObjectGuid::from_raw(0x0100_0000_0000_00AA);
        let stop = SmsgMonsterMoveStopResponse {
            guid,
            position: WorldLocationResponse {
                map_id: 1,
                x: 1.0,
                y: 2.0,
                z: 3.0,
                orientation: 0.0,
            },
            spline_id: 9,
            move_type: 1,
        };
        let mut expected_stop = Vec::new();
        put_packed_guid(&mut expected_stop, guid);
        expected_stop.extend_from_slice(&1.0f32.to_le_bytes());
        expected_stop.extend_from_slice(&2.0f32.to_le_bytes());
        expected_stop.extend_from_slice(&3.0f32.to_le_bytes());
        expected_stop.extend_from_slice(&9u32.to_le_bytes());
        expected_stop.push(1);
        assert_eq!(stop.body(), expected_stop);

        let taxi_path = SmsgMonsterMovePathResponse {
            guid,
            start: WorldLocationResponse {
                map_id: 0,
                x: 1.0,
                y: 2.0,
                z: 3.0,
                orientation: 0.0,
            },
            path: vec![
                WorldLocationResponse {
                    map_id: 0,
                    x: 4.0,
                    y: 5.0,
                    z: 6.0,
                    orientation: 0.0,
                },
                WorldLocationResponse {
                    map_id: 0,
                    x: 7.0,
                    y: 8.0,
                    z: 9.0,
                    orientation: 0.0,
                },
            ],
            spline_id: 77,
            duration_ms: 1234,
            facing_target: None,
            move_type_normal: 0,
            move_type_facing_target: 3,
            run_spline_flag: 0x300,
            run: true,
            catmull_rom: true,
        };
        let mut expected_taxi = Vec::new();
        put_packed_guid(&mut expected_taxi, guid);
        for value in [1.0f32, 2.0, 3.0] {
            expected_taxi.extend_from_slice(&value.to_le_bytes());
        }
        expected_taxi.extend_from_slice(&77u32.to_le_bytes());
        expected_taxi.push(0);
        expected_taxi.extend_from_slice(&0x300u32.to_le_bytes());
        expected_taxi.extend_from_slice(&1234u32.to_le_bytes());
        expected_taxi.extend_from_slice(&2u32.to_le_bytes());
        for value in [4.0f32, 5.0, 6.0, 7.0, 8.0, 9.0] {
            expected_taxi.extend_from_slice(&value.to_le_bytes());
        }
        assert_eq!(taxi_path.body(), expected_taxi);

        let corpse = MsgCorpseQueryResponse {
            corpse_position: Some(WorldLocationResponse {
                map_id: 0,
                x: -1.0,
                y: 2.0,
                z: 3.0,
                orientation: 0.0,
            }),
        };
        let mut expected_corpse = vec![1];
        expected_corpse.extend_from_slice(&0i32.to_le_bytes());
        expected_corpse.extend_from_slice(&(-1.0f32).to_le_bytes());
        expected_corpse.extend_from_slice(&2.0f32.to_le_bytes());
        expected_corpse.extend_from_slice(&3.0f32.to_le_bytes());
        expected_corpse.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(corpse.body(), expected_corpse);
        assert_eq!(
            MsgCorpseQueryResponse {
                corpse_position: None
            }
            .body(),
            vec![0]
        );

        let xp = SmsgLogXpGainResponse {
            source: Some(guid),
            given_xp: 52,
            base_xp: 52,
        };
        let mut expected_xp = Vec::new();
        expected_xp.extend_from_slice(&guid.raw().to_le_bytes());
        expected_xp.extend_from_slice(&52u32.to_le_bytes());
        expected_xp.push(0);
        expected_xp.extend_from_slice(&52u32.to_le_bytes());
        expected_xp.extend_from_slice(&1.0f32.to_le_bytes());
        assert_eq!(xp.body(), expected_xp);
    }

    #[test]
    fn combat_spell_and_loot_responses_write_expected_layouts() {
        let caster = ObjectGuid::from_raw(0x0100_0000_0000_0001);
        let target = ObjectGuid::from_raw(0x0100_0000_0000_0002);

        let attack = SmsgAttackStartResponse {
            attacker: caster,
            victim: target,
        };
        let mut expected_attack = Vec::new();
        expected_attack.extend_from_slice(&caster.raw().to_le_bytes());
        expected_attack.extend_from_slice(&target.raw().to_le_bytes());
        assert_eq!(attack.body(), expected_attack);

        let cast = SmsgCastResultResponse {
            spell_id: 78,
            status: 2,
            failure: Some(5),
        };
        assert_eq!(cast.body(), vec![78, 0, 0, 0, 2, 5]);

        let targets = SpellCastTargets {
            target_mask: SPELL_CAST_TARGET_UNIT,
            unit_target: Some(target),
            gameobject_target: None,
            source_location: None,
            destination: None,
        };
        let go = SmsgSpellGoResponse {
            source: caster,
            caster,
            spell_id: 78,
            cast_flags: 0x0100,
            targets,
            hit_targets: vec![target],
            miss_targets: Vec::new(),
            ammo: None,
        };
        let mut expected_go = Vec::new();
        put_packed_guid(&mut expected_go, caster);
        put_packed_guid(&mut expected_go, caster);
        expected_go.extend_from_slice(&78u32.to_le_bytes());
        expected_go.extend_from_slice(&0x0100u16.to_le_bytes());
        expected_go.push(1);
        expected_go.extend_from_slice(&target.raw().to_le_bytes());
        expected_go.push(0);
        expected_go.extend_from_slice(&SPELL_CAST_TARGET_UNIT.to_le_bytes());
        put_packed_guid(&mut expected_go, target);
        assert_eq!(go.body(), expected_go);

        let hostile_targets = SpellCastTargets {
            target_mask: SPELL_CAST_TARGET_UNIT_ENEMY,
            unit_target: Some(target),
            gameobject_target: None,
            source_location: None,
            destination: None,
        };
        let hostile_go = SmsgSpellGoResponse {
            source: caster,
            caster,
            spell_id: 78,
            cast_flags: 0x0100,
            targets: hostile_targets,
            hit_targets: vec![target],
            miss_targets: Vec::new(),
            ammo: None,
        };
        let hostile_body = hostile_go.body();
        let hostile_mask_offset = PackedGuid::packed_size(caster) * 2 + 4 + 2 + 1 + 8 + 1;
        assert_eq!(
            u16::from_le_bytes(
                hostile_body[hostile_mask_offset..hostile_mask_offset + 2]
                    .try_into()
                    .unwrap()
            ),
            SPELL_CAST_TARGET_UNIT | SPELL_CAST_TARGET_UNIT_ENEMY
        );

        let loot = SmsgLootResponse {
            target: caster,
            loot_type: 2,
            money: 7,
            items: vec![LootItemResponse {
                slot: 0,
                item: 25,
                count: 1,
                display_id: 99,
                random_suffix: 0,
                random_property: 0,
                slot_type: 1,
            }],
        };
        let mut expected_loot = Vec::new();
        expected_loot.extend_from_slice(&caster.raw().to_le_bytes());
        expected_loot.push(2);
        expected_loot.extend_from_slice(&7u32.to_le_bytes());
        expected_loot.push(1);
        expected_loot.push(0);
        expected_loot.extend_from_slice(&25u32.to_le_bytes());
        expected_loot.extend_from_slice(&1u32.to_le_bytes());
        expected_loot.extend_from_slice(&99u32.to_le_bytes());
        expected_loot.extend_from_slice(&0u32.to_le_bytes());
        expected_loot.extend_from_slice(&0u32.to_le_bytes());
        expected_loot.push(1);
        assert_eq!(loot.body(), expected_loot);
    }

    #[test]
    fn world_opcode_known_values_are_stable() {
        assert_eq!(
            WorldOpcode::try_from(0x004A).unwrap(),
            WorldOpcode::CmsgPlayerLogout
        );
        assert_eq!(
            WorldOpcode::try_from(0x0050).unwrap(),
            WorldOpcode::CmsgNameQuery
        );
        assert_eq!(
            WorldOpcode::try_from(0x00FE).unwrap(),
            WorldOpcode::CmsgTutorialFlag
        );
        assert_eq!(
            WorldOpcode::try_from(0x005C).unwrap(),
            WorldOpcode::CmsgQuestQuery
        );
        assert_eq!(
            WorldOpcode::try_from(0x00A9).unwrap(),
            WorldOpcode::SmsgUpdateObject
        );
        assert_eq!(
            WorldOpcode::try_from(0x00B0).unwrap(),
            WorldOpcode::SmsgItemCooldown
        );
        assert_eq!(
            WorldOpcode::try_from(0x0112).unwrap(),
            WorldOpcode::SmsgInventoryChangeFailure
        );
        assert_eq!(
            WorldOpcode::try_from(0x005D).unwrap(),
            WorldOpcode::SmsgQuestQueryResponse
        );
        assert_eq!(
            WorldOpcode::try_from(0x0060).unwrap(),
            WorldOpcode::CmsgCreatureQuery
        );
        assert_eq!(
            WorldOpcode::try_from(0x006E).unwrap(),
            WorldOpcode::CmsgGroupInvite
        );
        assert_eq!(
            WorldOpcode::try_from(0x007A).unwrap(),
            WorldOpcode::CmsgLootMethod
        );
        assert_eq!(
            WorldOpcode::try_from(0x0128).unwrap(),
            WorldOpcode::CmsgSetActionButton
        );
        assert_eq!(
            WorldOpcode::try_from(0x017D).unwrap(),
            WorldOpcode::SmsgGossipMessage
        );
        assert_eq!(
            WorldOpcode::try_from(0x0188).unwrap(),
            WorldOpcode::SmsgQuestgiverQuestDetails
        );
        assert_eq!(
            WorldOpcode::try_from(0x019F).unwrap(),
            WorldOpcode::SmsgListInventory
        );
        assert_eq!(
            WorldOpcode::try_from(0x01B1).unwrap(),
            WorldOpcode::SmsgTrainerList
        );
        assert_eq!(
            WorldOpcode::try_from(0x0255).unwrap(),
            WorldOpcode::MsgAuctionHello
        );
        assert_eq!(
            WorldOpcode::try_from(0x0258).unwrap(),
            WorldOpcode::CmsgAuctionListItems
        );
        assert_eq!(
            WorldOpcode::try_from(0x025C).unwrap(),
            WorldOpcode::SmsgAuctionListResult
        );
        assert_eq!(
            WorldOpcode::try_from(0x025D).unwrap(),
            WorldOpcode::SmsgAuctionOwnerListResult
        );
        assert_eq!(
            WorldOpcode::try_from(0x0265).unwrap(),
            WorldOpcode::SmsgAuctionBidderListResult
        );
        assert_eq!(
            WorldOpcode::try_from(0x017B).unwrap(),
            WorldOpcode::CmsgGossipHello
        );
        assert_eq!(
            WorldOpcode::try_from(0x019E).unwrap(),
            WorldOpcode::CmsgListInventory
        );
        assert_eq!(
            WorldOpcode::try_from(0x013D).unwrap(),
            WorldOpcode::CmsgSetSelection
        );
        assert_eq!(
            WorldOpcode::try_from(0x020A).unwrap(),
            WorldOpcode::CmsgRequestAccountData
        );
        assert_eq!(
            WorldOpcode::try_from(0x027E).unwrap(),
            WorldOpcode::CmsgGroupChangeSubGroup
        );
        assert_eq!(
            WorldOpcode::try_from(0x028F).unwrap(),
            WorldOpcode::CmsgGroupAssistantLeader
        );
        assert_eq!(
            WorldOpcode::try_from(0x01ED).unwrap(),
            WorldOpcode::CmsgAuthSession
        );
        assert_eq!(
            WorldOpcode::try_from(0x01DC).unwrap(),
            WorldOpcode::CmsgPing
        );
        assert_eq!(u32::from(WorldOpcode::SmsgPong), 0x01DD);
        assert!(WorldOpcode::try_from(0xFFFF).is_err());
    }

    #[test]
    fn fixed_world_requests_roundtrip() {
        let name = NameQueryRequest {
            raw_guid: 0x0102_0304_0506_0708,
        };
        let mut bytes = BytesMut::new();
        name.write(&mut bytes);
        assert_eq!(NameQueryRequest::read(&mut &bytes[..]).unwrap(), name);

        let item = ItemQuerySingleRequest { item_id: 6948 };
        bytes.clear();
        item.write(&mut bytes);
        assert_eq!(ItemQuerySingleRequest::read(&mut &bytes[..]).unwrap(), item);

        let item_name = ItemNameQueryRequest { item_id: 25 };
        bytes.clear();
        item_name.write(&mut bytes);
        assert_eq!(
            ItemNameQueryRequest::read(&mut &bytes[..]).unwrap(),
            item_name
        );

        let account_data = RequestAccountDataRequest { data_type: 2 };
        bytes.clear();
        account_data.write(&mut bytes);
        assert_eq!(
            RequestAccountDataRequest::read(&mut &bytes[..]).unwrap(),
            account_data
        );

        let tutorial = TutorialFlagRequest { flag: 42 };
        bytes.clear();
        tutorial.write(&mut bytes);
        assert_eq!(
            TutorialFlagRequest::read(&mut &bytes[..]).unwrap(),
            tutorial
        );

        let stand = StandStateChangeRequest { stand_state: 1 };
        bytes.clear();
        stand.write(&mut bytes);
        assert_eq!(
            StandStateChangeRequest::read(&mut &bytes[..]).unwrap(),
            stand
        );

        let action = SetActionButtonRequest {
            button: 7,
            packed_data: 0x8000_0019,
        };
        bytes.clear();
        action.write(&mut bytes);
        assert_eq!(
            SetActionButtonRequest::read(&mut &bytes[..]).unwrap(),
            action
        );
        assert_eq!(action.action(), 25);
        assert_eq!(action.action_type(), 0x80);

        let selection = SetSelectionRequest {
            raw_guid: 0xAABB_CCDD_EEFF_0011,
        };
        bytes.clear();
        selection.write(&mut bytes);
        assert_eq!(
            SetSelectionRequest::read(&mut &bytes[..]).unwrap(),
            selection
        );

        let target = SetTargetObsoleteRequest {
            raw_guid: 0x1122_3344_5566_7788,
        };
        bytes.clear();
        target.write(&mut bytes);
        assert_eq!(
            SetTargetObsoleteRequest::read(&mut &bytes[..]).unwrap(),
            target
        );

        let mover = SetActiveMoverRequest {
            raw_guid: 0x8877_6655_4433_2211,
        };
        bytes.clear();
        mover.write(&mut bytes);
        assert_eq!(SetActiveMoverRequest::read(&mut &bytes[..]).unwrap(), mover);

        let quest = QuestQueryRequest { quest_id: 33 };
        bytes.clear();
        quest.write(&mut bytes);
        assert_eq!(QuestQueryRequest::read(&mut &bytes[..]).unwrap(), quest);

        let creature = CreatureQueryRequest {
            entry: 3101,
            raw_guid: 0x1000_0000_0000_0C1D,
        };
        bytes.clear();
        creature.write(&mut bytes);
        assert_eq!(
            CreatureQueryRequest::read(&mut &bytes[..]).unwrap(),
            creature
        );

        let gameobject = GameObjectQueryRequest {
            entry: 55,
            raw_guid: 0xF110_0000_0000_0037,
        };
        bytes.clear();
        gameobject.write(&mut bytes);
        assert_eq!(
            GameObjectQueryRequest::read(&mut &bytes[..]).unwrap(),
            gameobject
        );

        let npc_text = NpcTextQueryRequest {
            text_id: 68,
            raw_guid: 0xF130_0000_0000_0037,
        };
        bytes.clear();
        npc_text.write(&mut bytes);
        assert_eq!(
            NpcTextQueryRequest::read(&mut &bytes[..]).unwrap(),
            npc_text
        );

        let gossip = GossipHelloRequest {
            raw_guid: 0xF130_0000_0000_0037,
        };
        bytes.clear();
        gossip.write(&mut bytes);
        assert_eq!(GossipHelloRequest::read(&mut &bytes[..]).unwrap(), gossip);

        let auction_search = AuctionListItemsRequest {
            auctioneer_raw_guid: 0xF130_0000_0000_0042,
            list_from: 50,
            searched_name: "sword".to_string(),
            level_min: 10,
            level_max: 20,
            inventory_type: 13,
            item_class: 2,
            item_subclass: 7,
            quality: 2,
            usable: 1,
        };
        bytes.clear();
        auction_search.write(&mut bytes);
        assert_eq!(
            AuctionListItemsRequest::read(&mut &bytes[..]).unwrap(),
            auction_search
        );

        let auction_hello = AuctionHelloRequest {
            auctioneer_raw_guid: 0xF130_0000_0000_0042,
        };
        bytes.clear();
        auction_hello.write(&mut bytes);
        assert_eq!(
            AuctionHelloRequest::read(&mut &bytes[..]).unwrap(),
            auction_hello
        );

        let auction_sell = AuctionSellItemRequest {
            auctioneer_raw_guid: 0xF130_0000_0000_0042,
            item_raw_guid: 0x4000_0000_0000_0091,
            bid: 1_000,
            buyout: 2_000,
            duration_minutes: 120,
        };
        bytes.clear();
        auction_sell.write(&mut bytes);
        assert_eq!(
            AuctionSellItemRequest::read(&mut &bytes[..]).unwrap(),
            auction_sell
        );

        let auction_remove = AuctionRemoveItemRequest {
            auctioneer_raw_guid: 0xF130_0000_0000_0042,
            auction_id: 77,
        };
        bytes.clear();
        auction_remove.write(&mut bytes);
        assert_eq!(
            AuctionRemoveItemRequest::read(&mut &bytes[..]).unwrap(),
            auction_remove
        );

        let auction_bid = AuctionPlaceBidRequest {
            auctioneer_raw_guid: 0xF130_0000_0000_0042,
            auction_id: 77,
            price: 1_500,
        };
        bytes.clear();
        auction_bid.write(&mut bytes);
        assert_eq!(
            AuctionPlaceBidRequest::read(&mut &bytes[..]).unwrap(),
            auction_bid
        );

        let owner_items = AuctionListOwnerItemsRequest {
            auctioneer_raw_guid: 0xF130_0000_0000_0042,
            list_from: 100,
        };
        bytes.clear();
        owner_items.write(&mut bytes);
        assert_eq!(
            AuctionListOwnerItemsRequest::read(&mut &bytes[..]).unwrap(),
            owner_items
        );

        let bidder_items = AuctionListBidderItemsRequest {
            auctioneer_raw_guid: 0xF130_0000_0000_0042,
            list_from: 0,
            outbid_auction_ids: vec![17, 29],
        };
        bytes.clear();
        bidder_items.write(&mut bytes);
        assert_eq!(
            AuctionListBidderItemsRequest::read(&mut &bytes[..]).unwrap(),
            bidder_items
        );

        let invite = GroupInviteRequest {
            member_name: "Ada".to_string(),
        };
        bytes.clear();
        invite.write(&mut bytes);
        assert_eq!(GroupInviteRequest::read(&mut &bytes[..]).unwrap(), invite);

        let uninvite = GroupUninviteRequest {
            member_name: "Grace".to_string(),
        };
        bytes.clear();
        uninvite.write(&mut bytes);
        assert_eq!(
            GroupUninviteRequest::read(&mut &bytes[..]).unwrap(),
            uninvite
        );

        let subgroup = GroupChangeSubGroupRequest {
            member_name: "Linus".to_string(),
            subgroup: 3,
        };
        bytes.clear();
        subgroup.write(&mut bytes);
        assert_eq!(
            GroupChangeSubGroupRequest::read(&mut &bytes[..]).unwrap(),
            subgroup
        );

        let assistant = GroupAssistantLeaderRequest {
            raw_guid: 0xAABB_CCDD_EEFF_0011,
            enabled: true,
        };
        bytes.clear();
        assistant.write(&mut bytes);
        assert_eq!(
            GroupAssistantLeaderRequest::read(&mut &bytes[..]).unwrap(),
            assistant
        );

        let loot_method = LootMethodRequest {
            loot_method: 2,
            master_looter_raw_guid: 0x0102_0304_0506_0708,
            loot_threshold: 3,
        };
        bytes.clear();
        loot_method.write(&mut bytes);
        assert_eq!(
            LootMethodRequest::read(&mut &bytes[..]).unwrap(),
            loot_method
        );
    }

    #[test]
    fn group_string_request_requires_nul_termination() {
        let error = GroupInviteRequest::read(&mut &b"Ada"[..]).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn update_account_data_request_roundtrip() {
        let request = UpdateAccountDataRequest {
            data_type: 3,
            decompressed_size: 12,
            compressed_data: vec![1, 2, 3, 4],
        };
        let mut bytes = BytesMut::new();
        request.write(&mut bytes);

        let decoded = UpdateAccountDataRequest::read(&mut &bytes[..]).unwrap();
        assert_eq!(decoded, request);
    }

    #[test]
    fn world_auth_session_request_roundtrip() {
        let request = WorldAuthSessionRequest {
            client_build: 5875,
            account: "RUSTAUTH".to_string(),
            client_seed: 0x11223344,
            digest: [0xAB; 20],
            addon_data: vec![1, 2, 3],
        };
        let mut bytes = BytesMut::new();
        request.write(&mut bytes);

        let decoded = WorldAuthSessionRequest::read(&mut &bytes[..]).unwrap();
        assert_eq!(decoded, request);
    }

    #[test]
    fn world_auth_session_requires_nul_terminated_account() {
        let bytes = [1u8; WorldAuthSessionRequest::MIN_SIZE];
        let error = WorldAuthSessionRequest::read(&mut &bytes[..]).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn auction_packets_match_vanilla_shapes() {
        let auctioneer = ObjectGuid::new(wow_common::guid::HighGuid::Unit, 7, 42);
        let hello = SmsgAuctionHelloResponse {
            auctioneer,
            house_id: 7,
        };
        let hello_body = hello.body();
        assert_eq!(hello_body.len(), 12);
        assert_eq!(&hello_body[0..8], &auctioneer.raw().to_le_bytes());
        assert_eq!(&hello_body[8..12], &7u32.to_le_bytes());

        let command = SmsgAuctionCommandResultResponse {
            auction_id: 77,
            action: 0,
            error_code: 1,
            bid_min_outbid: None,
            inventory_error: Some(2),
            higher_bidder: None,
            higher_bid: None,
            higher_min_outbid: None,
        };
        assert_eq!(
            command.body(),
            [77u32, 0, 1, 2]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>()
        );

        let bid_ok = SmsgAuctionCommandResultResponse {
            auction_id: 77,
            action: 2,
            error_code: 0,
            bid_min_outbid: Some(5),
            inventory_error: None,
            higher_bidder: None,
            higher_bid: None,
            higher_min_outbid: None,
        };
        assert_eq!(
            bid_ok.body(),
            [77u32, 2, 0, 5]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>()
        );

        let removed = SmsgAuctionRemovedNotificationResponse {
            auction_id: 77,
            item_template: 744,
            item_random_property_id: -15,
        };
        assert_eq!(
            removed.body(),
            [77u32, 744, u32::MAX - 14]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>()
        );

        let auction = AuctionInfoResponse {
            id: 77,
            item: 744,
            enchantment: 1900,
            random_property_id: 0,
            suffix_factor: 0,
            count: 3,
            charges: u32::MAX,
            owner: ObjectGuid::new(wow_common::guid::HighGuid::Player, 0, 11),
            start_bid: 100,
            min_outbid: 5,
            buyout: 500,
            time_left_millis: 60_000,
            bidder: ObjectGuid::new(wow_common::guid::HighGuid::Player, 0, 12),
            current_bid: 125,
        };
        let body = SmsgAuctionListResultResponse {
            auctions: vec![auction],
            total_count: 1,
        }
        .body();
        assert_eq!(&body[0..4], &1u32.to_le_bytes());
        assert_eq!(&body[4..8], &77u32.to_le_bytes());
        assert_eq!(&body[8..12], &744u32.to_le_bytes());
        assert_eq!(&body[28..32], &u32::MAX.to_le_bytes());
        assert_eq!(&body[32..40], &auction.owner.raw().to_le_bytes());
        assert_eq!(&body[40..44], &100u32.to_le_bytes());
        assert_eq!(&body[44..48], &5u32.to_le_bytes());
        assert_eq!(&body[48..52], &500u32.to_le_bytes());
        assert_eq!(&body[52..56], &60_000u32.to_le_bytes());
        assert_eq!(&body[56..64], &auction.bidder.raw().to_le_bytes());
        assert_eq!(&body[64..68], &125u32.to_le_bytes());
        assert_eq!(&body[68..72], &1u32.to_le_bytes());
    }
}
