use super::*;
use wow_proto::world::WorldOpcode;
use wow_proto::{
    CharEnumEntryResponse, CharEnumEquipmentResponse, MsgQueryNextMailTimeResponse,
    ServerWorldPacket, SmsgChannelNotifyResponse, SmsgCharEnumResponse,
    SmsgGmTicketGetTicketResponse, SmsgQueryTimeResponse, SmsgUpdateAccountDataResponse,
};

pub(in crate::world) async fn handle_query_time(
    stream: &mut WorldPacketSink,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let unix_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as u32)
        .unwrap_or(0);
    let body = SmsgQueryTimeResponse { unix_time }.body();
    send_packet(
        stream,
        WorldOpcode::SmsgQueryTimeResponse as u16,
        &body,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn handle_request_account_data(
    stream: &mut WorldPacketSink,
    request: wow_proto::RequestAccountDataRequest,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let account_data_type = request.data_type;
    if account_data_type >= ACCOUNT_DATA_TYPES as u32 {
        warn!(
            account_data_type,
            "Ignoring invalid account data request type"
        );
        return Ok(());
    }
    let account_data = session
        .account
        .account_data
        .get(&account_data_type)
        .map(|entry| entry.data.as_slice())
        .unwrap_or_default();
    let compressed = if account_data.is_empty() {
        Vec::new()
    } else {
        zlib_compress(account_data)?
    };
    let response = SmsgUpdateAccountDataResponse {
        account_data_type,
        decompressed_size: account_data.len() as u32,
        compressed_data: compressed,
    }
    .body();
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateAccountData as u16,
        &response,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn handle_update_account_data(
    character_db_pool: &MySqlPool,
    account_id: u32,
    request: wow_proto::UpdateAccountDataRequest,
    session: &mut WorldSessionState,
) -> anyhow::Result<()> {
    let account_data_type = request.data_type;
    let decompressed_size = request.decompressed_size;
    if account_data_type >= ACCOUNT_DATA_TYPES as u32 {
        warn!(
            account_data_type,
            "Ignoring invalid account data update type"
        );
        return Ok(());
    }
    let data = if decompressed_size == 0 {
        Vec::new()
    } else {
        let mut data = zlib_decompress(&request.compressed_data, decompressed_size as usize)?;
        if data.last() == Some(&0) {
            data.pop();
        }
        data
    };

    if account_data_is_global(account_data_type) {
        wow_db::replace_global_account_data(
            character_db_pool,
            account_id,
            account_data_type,
            &data,
        )
        .await?;
    } else if let Some(character) = session.character.active_character.as_ref() {
        wow_db::replace_character_account_data(
            character_db_pool,
            character.guid,
            account_data_type,
            &data,
        )
        .await?;
    } else {
        warn!(
            account_data_type,
            account_id, "Ignoring per-character account data update with no active character"
        );
        return Ok(());
    }
    if data.is_empty() {
        session.account.account_data.remove(&account_data_type);
    } else {
        session.account.account_data.insert(
            account_data_type,
            AccountDataCache {
                time: current_unix_time(),
                data,
            },
        );
    }
    Ok(())
}

pub(in crate::world) async fn load_global_account_data_into_session(
    character_db_pool: &MySqlPool,
    account_id: u32,
    session: &mut WorldSessionState,
) -> anyhow::Result<()> {
    clear_account_data_mask(&mut session.account.account_data, GLOBAL_ACCOUNT_DATA_MASK);
    load_account_data_entries(
        &mut session.account.account_data,
        wow_db::get_global_account_data(character_db_pool, account_id).await?,
        GLOBAL_ACCOUNT_DATA_MASK,
    );
    Ok(())
}

pub(in crate::world) async fn load_character_account_data_into_session(
    character_db_pool: &MySqlPool,
    character_guid: u32,
    session: &mut WorldSessionState,
) -> anyhow::Result<()> {
    clear_account_data_mask(
        &mut session.account.account_data,
        PER_CHARACTER_ACCOUNT_DATA_MASK,
    );
    load_account_data_entries(
        &mut session.account.account_data,
        wow_db::get_character_account_data(character_db_pool, character_guid).await?,
        PER_CHARACTER_ACCOUNT_DATA_MASK,
    );
    Ok(())
}

pub(in crate::world) fn load_account_data_entries(
    account_data: &mut HashMap<u32, AccountDataCache>,
    entries: Vec<wow_db::AccountDataEntry>,
    mask: u32,
) {
    for entry in entries {
        if entry.data_type >= ACCOUNT_DATA_TYPES as u32 || mask & (1 << entry.data_type) == 0 {
            continue;
        }
        if entry.data.is_empty() {
            continue;
        }
        account_data.insert(
            entry.data_type,
            AccountDataCache {
                time: entry.time,
                data: entry.data,
            },
        );
    }
}

pub(in crate::world) fn clear_account_data_mask(
    account_data: &mut HashMap<u32, AccountDataCache>,
    mask: u32,
) {
    account_data.retain(|data_type, _| mask & (1 << *data_type) == 0);
}

pub(in crate::world) fn account_data_is_global(account_data_type: u32) -> bool {
    GLOBAL_ACCOUNT_DATA_MASK & (1 << account_data_type) != 0
}

pub(in crate::world) fn zlib_compress(data: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

pub(in crate::world) fn zlib_decompress(
    data: &[u8],
    expected_size: usize,
) -> anyhow::Result<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(data);
    let mut decoded = Vec::with_capacity(expected_size);
    decoder.read_to_end(&mut decoded)?;
    if decoded.len() != expected_size {
        anyhow::bail!(
            "account data decompressed size mismatch: expected {}, got {}",
            expected_size,
            decoded.len()
        );
    }
    Ok(decoded)
}

pub(in crate::world) fn current_unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

pub(in crate::world) async fn handle_gmticket_getticket(
    stream: &mut WorldPacketSink,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    send_packet(
        stream,
        WorldOpcode::SmsgGmTicketGetTicket as u16,
        &SmsgGmTicketGetTicketResponse { status: 0 }.body(),
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn handle_join_channel(
    stream: &mut WorldPacketSink,
    request: wow_proto::JoinChannelRequest,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if request.channel_name.is_empty() {
        return Ok(());
    }

    let response = build_channel_notify_you_joined_body(&request.channel_name);
    send_packet(
        stream,
        WorldOpcode::SmsgChannelNotify as u16,
        &response,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) fn handle_set_active_mover(
    request: wow_proto::SetActiveMoverRequest,
    session: &WorldSessionState,
) -> anyhow::Result<()> {
    let mover = ObjectGuid::from_raw(request.raw_guid);
    if let Some(character) = &session.character.active_character {
        if mover.counter() != character.guid {
            warn!(
                active_guid = character.guid,
                mover_guid = mover.counter(),
                "Client selected unexpected active mover"
            );
        }
    }
    Ok(())
}

pub(in crate::world) async fn handle_set_selection(
    shared_world: SharedWorldDeps<'_>,
    request: wow_proto::SetSelectionRequest,
    session: &mut WorldSessionState,
) -> anyhow::Result<()> {
    let selected_target = ObjectGuid::from_raw(request.raw_guid);
    let selected_target = if selected_target == ObjectGuid::EMPTY {
        None
    } else {
        Some(selected_target)
    };
    session.character.selected_target = selected_target;

    let Some(character) = &session.character.active_character else {
        return Ok(());
    };
    let packets = shared_world
        .maps
        .update_player_selection(character.position.map_id, character.guid, selected_target)
        .await?;
    shared_world.sessions.dispatch(packets).await;
    Ok(())
}

pub(in crate::world) async fn handle_set_target_obsolete(
    shared_world: SharedWorldDeps<'_>,
    request: wow_proto::SetTargetObsoleteRequest,
    session: &WorldSessionState,
) -> anyhow::Result<()> {
    let unit_target = ObjectGuid::from_raw(request.raw_guid);
    let unit_target = if unit_target == ObjectGuid::EMPTY {
        None
    } else {
        Some(unit_target)
    };

    let Some(character) = &session.character.active_character else {
        return Ok(());
    };
    let packets = shared_world
        .maps
        .update_player_target(character.position.map_id, character.guid, unit_target)
        .await?;
    shared_world.sessions.dispatch(packets).await;
    Ok(())
}

pub(in crate::world) async fn handle_stand_state_change(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    request: wow_proto::StandStateChangeRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let stand_state = request.stand_state;
    let Ok(stand_state) = u8::try_from(stand_state) else {
        return Ok(());
    };
    if !matches!(
        stand_state,
        PLAYER_STAND_STATE_STAND
            | PLAYER_STAND_STATE_SIT
            | PLAYER_STAND_STATE_SLEEP
            | PLAYER_STAND_STATE_KNEEL
    ) {
        return Ok(());
    }
    if stand_state == PLAYER_STAND_STATE_STAND {
        interrupt_player_consumable_auras(
            stream,
            shared_world.maps,
            shared_world.sessions,
            session,
            AURA_INTERRUPT_FLAG_STANDING_CANCELS,
            header_crypto,
        )
        .await?;
    }
    session.character.player_stand_state = stand_state;

    let Some(character) = &session.character.active_character else {
        return Ok(());
    };
    let packets = shared_world
        .maps
        .set_player_stand_state(character.position.map_id, character.guid, stand_state)
        .await?;
    shared_world.sessions.dispatch(packets).await;
    Ok(())
}

pub(in crate::world) async fn handle_query_next_mail_time(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let has_unread = if let Some(character) = &session.character.active_character {
        wow_db::character_has_unread_mail(character_db_pool, character.guid).await?
    } else {
        false
    };
    let body = build_query_next_mail_time_body(has_unread);
    send_packet(
        stream,
        WorldOpcode::MsgQueryNextMailTime as u16,
        &body,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) fn build_query_next_mail_time_body(has_unread: bool) -> Vec<u8> {
    let delay = if has_unread { 0.0f32 } else { -86400.0f32 };
    MsgQueryNextMailTimeResponse {
        delay_seconds: delay,
    }
    .body()
}

pub(in crate::world) const CHAT_YOU_JOINED_NOTICE: u8 = 0x02;
pub(in crate::world) const CHANNEL_FLAG_CUSTOM: u32 = 0x01;
pub(in crate::world) const CHANNEL_FLAG_TRADE: u32 = 0x04;
pub(in crate::world) const CHANNEL_FLAG_NOT_LFG: u32 = 0x08;
pub(in crate::world) const CHANNEL_FLAG_GENERAL: u32 = 0x10;
pub(in crate::world) const CHANNEL_FLAG_CITY: u32 = 0x20;
pub(in crate::world) const CHANNEL_FLAG_LFG: u32 = 0x40;

pub(in crate::world) fn build_channel_notify_you_joined_body(channel_name: &str) -> Vec<u8> {
    SmsgChannelNotifyResponse {
        notice: CHAT_YOU_JOINED_NOTICE,
        channel_name: channel_name.to_string(),
        flags: channel_join_flags(channel_name),
        channel_id: 0,
    }
    .body()
}

pub(in crate::world) fn channel_join_flags(channel_name: &str) -> u32 {
    let lowercase = channel_name.to_ascii_lowercase();
    if lowercase.starts_with("general") {
        CHANNEL_FLAG_GENERAL | CHANNEL_FLAG_NOT_LFG
    } else if lowercase.starts_with("trade") {
        CHANNEL_FLAG_CITY | CHANNEL_FLAG_GENERAL | CHANNEL_FLAG_NOT_LFG | CHANNEL_FLAG_TRADE
    } else if lowercase.starts_with("localdefense") || lowercase.starts_with("worlddefense") {
        CHANNEL_FLAG_GENERAL | CHANNEL_FLAG_NOT_LFG
    } else if lowercase.starts_with("guildrecruitment") {
        CHANNEL_FLAG_CITY | CHANNEL_FLAG_GENERAL | CHANNEL_FLAG_NOT_LFG
    } else if lowercase.starts_with("lookingforgroup") {
        CHANNEL_FLAG_LFG | CHANNEL_FLAG_GENERAL
    } else {
        CHANNEL_FLAG_CUSTOM
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(in crate::world) struct JumpInfo {
    pub(in crate::world) z_speed: f32,
    pub(in crate::world) cos_angle: f32,
    pub(in crate::world) sin_angle: f32,
    pub(in crate::world) xy_speed: f32,
}

impl Default for JumpInfo {
    fn default() -> Self {
        Self {
            z_speed: 0.0,
            cos_angle: 0.0,
            sin_angle: 0.0,
            xy_speed: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(in crate::world) struct MovementInfo {
    pub(in crate::world) flags: u32,
    pub(in crate::world) client_time: u32,
    pub(in crate::world) position: WorldPosition,
    pub(in crate::world) fall_time: u32,
    pub(in crate::world) jump: JumpInfo,
}

impl MovementInfo {
    pub(in crate::world) fn read(body: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = 0;
        let flags = read_u32(body, &mut cursor)?;
        let client_time = read_u32(body, &mut cursor)?;
        let x = read_f32(body, &mut cursor)?;
        let y = read_f32(body, &mut cursor)?;
        let z = read_f32(body, &mut cursor)?;
        let orientation = read_f32(body, &mut cursor)?;

        if flags & MOVEFLAG_ONTRANSPORT != 0 {
            cursor = cursor
                .checked_add(8 + 4 * 4)
                .ok_or_else(|| anyhow::anyhow!("movement transport cursor overflow"))?;
            ensure_available(body, cursor)?;
        }

        if flags & MOVEFLAG_SWIMMING != 0 {
            let _swim_pitch = read_f32(body, &mut cursor)?;
        }

        let fall_time = read_u32(body, &mut cursor)?;

        let jump = if flags & MOVEFLAG_JUMPING != 0 {
            JumpInfo {
                z_speed: read_f32(body, &mut cursor)?,
                cos_angle: read_f32(body, &mut cursor)?,
                sin_angle: read_f32(body, &mut cursor)?,
                xy_speed: read_f32(body, &mut cursor)?,
            }
        } else {
            JumpInfo::default()
        };

        if flags & MOVEFLAG_SPLINE_ELEVATION != 0 {
            let _spline_elevation = read_f32(body, &mut cursor)?;
        }

        Ok(Self {
            flags,
            client_time,
            position: WorldPosition::new(0, x, y, z, orientation),
            fall_time,
            jump,
        })
    }
}

pub(in crate::world) fn write_movement_info(
    body: &mut Vec<u8>,
    flags: u32,
    client_time: u32,
    position: WorldPosition,
    fall_time: u32,
    jump: &JumpInfo,
) {
    body.extend_from_slice(&flags.to_le_bytes());
    body.extend_from_slice(&client_time.to_le_bytes());
    body.extend_from_slice(&position.x.to_le_bytes());
    body.extend_from_slice(&position.y.to_le_bytes());
    body.extend_from_slice(&position.z.to_le_bytes());
    body.extend_from_slice(&position.orientation.to_le_bytes());
    body.extend_from_slice(&fall_time.to_le_bytes());
    if flags & MOVEFLAG_JUMPING != 0 {
        body.extend_from_slice(&jump.z_speed.to_le_bytes());
        body.extend_from_slice(&jump.cos_angle.to_le_bytes());
        body.extend_from_slice(&jump.sin_angle.to_le_bytes());
        body.extend_from_slice(&jump.xy_speed.to_le_bytes());
    }
}

pub(in crate::world) fn build_player_movement_broadcast_body(
    player_guid: u32,
    movement: &MovementInfo,
    server_time: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(9 + 28);
    PackedGuid::write(&mut body, ObjectGuid::new(HighGuid::Player, 0, player_guid))?;
    write_movement_info(
        &mut body,
        movement.flags,
        server_time,
        movement.position,
        movement.fall_time,
        &movement.jump,
    );
    Ok(body)
}

pub(in crate::world) fn read_u32(body: &[u8], cursor: &mut usize) -> anyhow::Result<u32> {
    ensure_available(body, *cursor + 4)?;
    let value = u32::from_le_bytes(body[*cursor..*cursor + 4].try_into()?);
    *cursor += 4;
    Ok(value)
}

#[cfg(test)]
pub(in crate::world) fn read_packed_guid(
    body: &[u8],
    cursor: &mut usize,
) -> anyhow::Result<ObjectGuid> {
    ensure_available(body, *cursor + 1)?;
    let mask = body[*cursor];
    let packed_len = 1 + mask.count_ones() as usize;
    ensure_available(body, *cursor + packed_len)?;
    let mut reader = Cursor::new(&body[*cursor..*cursor + packed_len]);
    let guid = PackedGuid::read(&mut reader)?;
    *cursor += packed_len;
    Ok(guid)
}

#[cfg(test)]
pub(in crate::world) fn read_c_string(body: &[u8], cursor: &mut usize) -> anyhow::Result<String> {
    let end = body[*cursor..]
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| anyhow::anyhow!("C string is not NUL-terminated"))?
        + *cursor;
    let value = String::from_utf8(body[*cursor..end].to_vec())?;
    *cursor = end + 1;
    Ok(value)
}

pub(in crate::world) fn read_f32(body: &[u8], cursor: &mut usize) -> anyhow::Result<f32> {
    ensure_available(body, *cursor + 4)?;
    let value = f32::from_le_bytes(body[*cursor..*cursor + 4].try_into()?);
    *cursor += 4;
    Ok(value)
}

pub(in crate::world) fn ensure_available(body: &[u8], end: usize) -> anyhow::Result<()> {
    if end > body.len() {
        anyhow::bail!(
            "movement packet truncated: need {} bytes, got {}",
            end,
            body.len()
        );
    }
    Ok(())
}

pub(in crate::world) fn is_movement_opcode(opcode: u32) -> bool {
    matches!(
        WorldOpcode::try_from(opcode).ok(),
        Some(
            WorldOpcode::MsgMoveStartForward
                | WorldOpcode::MsgMoveStartBackward
                | WorldOpcode::MsgMoveStop
                | WorldOpcode::MsgMoveStartStrafeLeft
                | WorldOpcode::MsgMoveStartStrafeRight
                | WorldOpcode::MsgMoveStopStrafe
                | WorldOpcode::MsgMoveJump
                | WorldOpcode::MsgMoveStartTurnLeft
                | WorldOpcode::MsgMoveStartTurnRight
                | WorldOpcode::MsgMoveStopTurn
                | WorldOpcode::MsgMoveStartPitchUp
                | WorldOpcode::MsgMoveStartPitchDown
                | WorldOpcode::MsgMoveStopPitch
                | WorldOpcode::MsgMoveSetRunMode
                | WorldOpcode::MsgMoveSetWalkMode
                | WorldOpcode::MsgMoveFallLand
                | WorldOpcode::MsgMoveStartSwim
                | WorldOpcode::MsgMoveStopSwim
                | WorldOpcode::MsgMoveSetFacing
                | WorldOpcode::MsgMoveSetPitch
                | WorldOpcode::MsgMoveHeartbeat
                | WorldOpcode::CmsgMoveFallReset
        )
    )
}

pub(in crate::world) fn is_expected_noop_opcode(opcode: u32) -> bool {
    matches!(
        WorldOpcode::try_from(opcode).ok(),
        Some(
            WorldOpcode::CmsgCancelTrade
                | WorldOpcode::CmsgZoneUpdate
                | WorldOpcode::CmsgMeetingStoneInfo
                | WorldOpcode::CmsgRequestRaidInfo
                | WorldOpcode::CmsgMoveTimeSkipped
                | WorldOpcode::CmsgForceRunSpeedChangeAck
                | WorldOpcode::CmsgForceMoveRootAck
                | WorldOpcode::CmsgForceMoveUnrootAck
                | WorldOpcode::CmsgBattlefieldStatus
        )
    )
}

pub(in crate::world) fn expected_noop_opcode_name(opcode: u32) -> &'static str {
    match WorldOpcode::try_from(opcode).ok() {
        Some(WorldOpcode::CmsgCancelTrade) => "CMSG_CANCEL_TRADE",
        Some(WorldOpcode::CmsgCancelCast) => "CMSG_CANCEL_CAST",
        Some(WorldOpcode::CmsgSetAmmo) => "CMSG_SET_AMMO",
        Some(WorldOpcode::CmsgCancelAutoRepeatSpell) => "CMSG_CANCEL_AUTO_REPEAT_SPELL",
        Some(WorldOpcode::CmsgZoneUpdate) => "CMSG_ZONEUPDATE",
        Some(WorldOpcode::CmsgSetActiveMover) => "CMSG_SET_ACTIVE_MOVER",
        Some(WorldOpcode::MsgQueryNextMailTime) => "MSG_QUERY_NEXT_MAIL_TIME",
        Some(WorldOpcode::CmsgMeetingStoneInfo) => "CMSG_MEETINGSTONE_INFO",
        Some(WorldOpcode::CmsgRequestRaidInfo) => "CMSG_REQUEST_RAID_INFO",
        Some(WorldOpcode::CmsgMoveTimeSkipped) => "CMSG_MOVE_TIME_SKIPPED",
        Some(WorldOpcode::CmsgForceRunSpeedChangeAck) => "CMSG_FORCE_RUN_SPEED_CHANGE_ACK",
        Some(WorldOpcode::CmsgForceMoveRootAck) => "CMSG_FORCE_MOVE_ROOT_ACK",
        Some(WorldOpcode::CmsgForceMoveUnrootAck) => "CMSG_FORCE_MOVE_UNROOT_ACK",
        Some(WorldOpcode::CmsgBattlefieldStatus) => "CMSG_BATTLEFIELD_STATUS",
        _ => "EXPECTED_NOOP",
    }
}

pub(in crate::world) fn movement_opcode_name(opcode: u32) -> &'static str {
    match WorldOpcode::try_from(opcode).ok() {
        Some(WorldOpcode::MsgMoveStartForward) => "MSG_MOVE_START_FORWARD",
        Some(WorldOpcode::MsgMoveStartBackward) => "MSG_MOVE_START_BACKWARD",
        Some(WorldOpcode::MsgMoveStop) => "MSG_MOVE_STOP",
        Some(WorldOpcode::MsgMoveStartStrafeLeft) => "MSG_MOVE_START_STRAFE_LEFT",
        Some(WorldOpcode::MsgMoveStartStrafeRight) => "MSG_MOVE_START_STRAFE_RIGHT",
        Some(WorldOpcode::MsgMoveStopStrafe) => "MSG_MOVE_STOP_STRAFE",
        Some(WorldOpcode::MsgMoveJump) => "MSG_MOVE_JUMP",
        Some(WorldOpcode::MsgMoveStartTurnLeft) => "MSG_MOVE_START_TURN_LEFT",
        Some(WorldOpcode::MsgMoveStartTurnRight) => "MSG_MOVE_START_TURN_RIGHT",
        Some(WorldOpcode::MsgMoveStopTurn) => "MSG_MOVE_STOP_TURN",
        Some(WorldOpcode::MsgMoveStartPitchUp) => "MSG_MOVE_START_PITCH_UP",
        Some(WorldOpcode::MsgMoveStartPitchDown) => "MSG_MOVE_START_PITCH_DOWN",
        Some(WorldOpcode::MsgMoveStopPitch) => "MSG_MOVE_STOP_PITCH",
        Some(WorldOpcode::MsgMoveSetRunMode) => "MSG_MOVE_SET_RUN_MODE",
        Some(WorldOpcode::MsgMoveSetWalkMode) => "MSG_MOVE_SET_WALK_MODE",
        Some(WorldOpcode::MsgMoveFallLand) => "MSG_MOVE_FALL_LAND",
        Some(WorldOpcode::MsgMoveStartSwim) => "MSG_MOVE_START_SWIM",
        Some(WorldOpcode::MsgMoveStopSwim) => "MSG_MOVE_STOP_SWIM",
        Some(WorldOpcode::MsgMoveSetFacing) => "MSG_MOVE_SET_FACING",
        Some(WorldOpcode::MsgMoveSetPitch) => "MSG_MOVE_SET_PITCH",
        Some(WorldOpcode::MsgMoveHeartbeat) => "MSG_MOVE_HEARTBEAT",
        Some(WorldOpcode::CmsgMoveFallReset) => "CMSG_MOVE_FALL_RESET",
        _ => "UNKNOWN_MOVEMENT",
    }
}

pub(in crate::world) fn build_char_enum_body(
    characters: &[CharacterEnumEntry],
) -> anyhow::Result<Vec<u8>> {
    if characters.len() > u8::MAX as usize {
        anyhow::bail!(
            "too many characters for SMSG_CHAR_ENUM: {}",
            characters.len()
        );
    }

    Ok(SmsgCharEnumResponse {
        characters: characters.iter().map(char_enum_entry_response).collect(),
    }
    .body())
}

pub(in crate::world) fn char_enum_entry_response(
    character: &CharacterEnumEntry,
) -> CharEnumEntryResponse {
    let show_pet =
        character.player_flags & PLAYER_FLAGS_GHOST == 0 && matches!(character.class, 3 | 9);
    let equipment = parse_equipment_cache(character.equipment_cache.as_deref())
        .into_iter()
        .map(|item_id| {
            starter_item_visual(item_id)
                .map(|visual| CharEnumEquipmentResponse {
                    display_id: visual.display_id,
                    inventory_type: visual.inventory_type,
                })
                .unwrap_or(CharEnumEquipmentResponse {
                    display_id: 0,
                    inventory_type: 0,
                })
        })
        .collect();
    CharEnumEntryResponse {
        guid: ObjectGuid::new(HighGuid::Player, 0, character.guid),
        name: character.name.clone(),
        race: character.race,
        class: character.class,
        gender: character.gender,
        player_bytes: character.player_bytes,
        player_bytes2: character.player_bytes2,
        level: character.level,
        zone: character.zone,
        map: character.map,
        x: character.position_x,
        y: character.position_y,
        z: character.position_z,
        guild_id: character.guildid.unwrap_or(0),
        flags: character_flags(character),
        first_login: character.at_login & AT_LOGIN_FIRST != 0,
        pet_display_id: if show_pet {
            character.pet_modelid.unwrap_or(0)
        } else {
            0
        },
        pet_level: if show_pet {
            character.pet_level.unwrap_or(0)
        } else {
            0
        },
        pet_family: 0,
        equipment,
    }
}

pub(in crate::world) fn write_c_string(body: &mut Vec<u8>, value: &str) {
    body.extend_from_slice(value.as_bytes());
    body.push(0);
}

pub(in crate::world) fn write_u32(body: &mut Vec<u8>, value: u32) {
    body.extend_from_slice(&value.to_le_bytes());
}

pub(in crate::world) fn write_i32(body: &mut Vec<u8>, value: i32) {
    body.extend_from_slice(&value.to_le_bytes());
}

pub(in crate::world) fn write_f32(body: &mut Vec<u8>, value: f32) {
    body.extend_from_slice(&value.to_le_bytes());
}

pub(in crate::world) fn character_flags(character: &CharacterEnumEntry) -> u32 {
    let mut flags = 0;
    if character.player_flags & PLAYER_FLAGS_HIDE_HELM != 0 {
        flags |= CHARACTER_FLAG_HIDE_HELM;
    }
    if character.player_flags & PLAYER_FLAGS_HIDE_CLOAK != 0 {
        flags |= CHARACTER_FLAG_HIDE_CLOAK;
    }
    if character.player_flags & PLAYER_FLAGS_GHOST != 0 {
        flags |= CHARACTER_FLAG_GHOST;
    }
    if character.at_login & AT_LOGIN_RENAME != 0 {
        flags |= CHARACTER_FLAG_RENAME;
    }
    flags
}

pub(in crate::world) async fn send_packet(
    stream: &mut WorldPacketSink,
    opcode: u16,
    body: &[u8],
    _header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    stream.send(opcode, body)
}

pub(in crate::world) async fn send_packet_direct<W>(
    stream: &mut W,
    opcode: u16,
    body: &[u8],
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let mut packet = Vec::with_capacity(4 + body.len());
    let size = (body.len() + 2) as u16;
    let mut header = [0u8; 4];
    header[0..2].copy_from_slice(&size.to_be_bytes());
    header[2..4].copy_from_slice(&opcode.to_le_bytes());
    if let Some(crypto) = header_crypto {
        crypto.encrypt(&mut header);
    }
    packet.extend_from_slice(&header);
    packet.extend_from_slice(body);
    stream.write_all(&packet).await?;
    crate::observability::record_world_packet_out(opcode);
    Ok(())
}

pub(in crate::world) async fn read_client_packet<R>(
    stream: &mut R,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<(u32, Vec<u8>)>
where
    R: AsyncRead + Unpin,
{
    let mut header = [0u8; 6];
    stream.read_exact(&mut header).await?;
    if let Some(crypto) = header_crypto {
        crypto.decrypt(&mut header);
    }

    let size = u16::from_be_bytes([header[0], header[1]]) as usize;
    let opcode = u32::from_le_bytes([header[2], header[3], header[4], header[5]]);

    if !(4..=0x2800).contains(&size) {
        anyhow::bail!("malformed world packet size {size} for opcode 0x{opcode:04X}");
    }

    let body_len = size - 4;
    let mut body = vec![0u8; body_len];
    if body_len > 0 {
        stream.read_exact(&mut body).await?;
    }

    crate::observability::record_world_packet_in(opcode);
    Ok((opcode, body))
}

pub(in crate::world) fn verify_world_digest(
    auth: &wow_proto::WorldAuthSessionRequest,
    session_key: &[u8; 40],
) -> bool {
    let mut hasher = Sha1::new();
    hasher.update(auth.account.as_bytes());
    hasher.update(0u32.to_le_bytes());
    hasher.update(auth.client_seed.to_le_bytes());
    hasher.update(SERVER_SEED.to_le_bytes());
    hasher.update(session_key);
    let digest: [u8; 20] = hasher.finalize().into();
    digest == auth.digest
}

pub(in crate::world) fn hex_to_array40(hex: &str) -> anyhow::Result<[u8; 40]> {
    let bytes = hex_to_vec(hex)?;
    if bytes.len() != 40 {
        anyhow::bail!("expected 40-byte session key, got {} bytes", bytes.len());
    }

    let mut out = [0u8; 40];
    out.copy_from_slice(&bytes);
    Ok(out)
}

pub(in crate::world) fn hex_to_vec(hex: &str) -> anyhow::Result<Vec<u8>> {
    let hex = hex.trim();
    if !hex.len().is_multiple_of(2) {
        anyhow::bail!("hex string has odd length");
    }

    let mut out = Vec::with_capacity(hex.len() / 2);
    for chunk in hex.as_bytes().chunks(2) {
        out.push((hex_nibble(chunk[0])? << 4) | hex_nibble(chunk[1])?);
    }
    Ok(out)
}

pub(in crate::world) fn hex_nibble(c: u8) -> anyhow::Result<u8> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => anyhow::bail!("invalid hex character 0x{c:02X}"),
    }
}
