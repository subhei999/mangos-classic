async fn handle_query_time(
    stream: &mut WorldPacketSink,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let unix_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as u32)
        .unwrap_or(0);
    send_packet(
        stream,
        SMSG_QUERY_TIME_RESPONSE,
        &unix_time.to_le_bytes(),
        Some(header_crypto),
    )
    .await
}

async fn handle_request_account_data(
    stream: &mut WorldPacketSink,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if body.len() < 4 {
        anyhow::bail!(
            "CMSG_REQUEST_ACCOUNT_DATA payload too short: {} bytes",
            body.len()
        );
    }

    let account_data_type = u32::from_le_bytes(body[0..4].try_into()?);
    let mut response = Vec::with_capacity(8);
    response.extend_from_slice(&account_data_type.to_le_bytes());
    response.extend_from_slice(&0u32.to_le_bytes()); // empty decompressed payload
    send_packet(
        stream,
        SMSG_UPDATE_ACCOUNT_DATA,
        &response,
        Some(header_crypto),
    )
    .await
}

fn handle_update_account_data(body: &[u8]) {
    if body.len() >= 8 {
        let account_data_type = u32::from_le_bytes(body[0..4].try_into().unwrap_or_default());
        let decompressed_size = u32::from_le_bytes(body[4..8].try_into().unwrap_or_default());
        info!(
            account_data_type,
            decompressed_size,
            bytes = body.len(),
            "Ignoring account data update"
        );
    } else {
        info!(bytes = body.len(), "Ignoring truncated account data update");
    }
}

async fn handle_gmticket_getticket(
    stream: &mut WorldPacketSink,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    send_packet(
        stream,
        SMSG_GMTICKET_GETTICKET,
        &0u32.to_le_bytes(),
        Some(header_crypto),
    )
    .await
}

async fn handle_join_channel(
    stream: &mut WorldPacketSink,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let request = JoinChannelRequest::read(body)?;
    if request.channel_name.is_empty() {
        return Ok(());
    }

    let response = build_channel_notify_you_joined_body(&request.channel_name);
    send_packet(stream, SMSG_CHANNEL_NOTIFY, &response, Some(header_crypto)).await
}

fn handle_set_active_mover(body: &[u8], session: &WorldSessionState) -> anyhow::Result<()> {
    if body.len() != 8 {
        anyhow::bail!(
            "CMSG_SET_ACTIVE_MOVER payload must be 8 bytes, got {}",
            body.len()
        );
    }

    let raw_guid = u64::from_le_bytes(body.try_into()?);
    let mover = ObjectGuid::from_raw(raw_guid);
    if let Some(character) = &session.active_character {
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

async fn handle_set_selection(
    shared_world: SharedWorldDeps<'_>,
    body: &[u8],
    session: &mut WorldSessionState,
) -> anyhow::Result<()> {
    let selected_target = read_packet_guid(body, "CMSG_SET_SELECTION")?;
    let selected_target = if selected_target == ObjectGuid::EMPTY {
        None
    } else {
        Some(selected_target)
    };
    session.selected_target = selected_target;

    let Some(character) = &session.active_character else {
        return Ok(());
    };
    let packets = shared_world
        .maps
        .update_player_selection(character.position.map_id, character.guid, selected_target)
        .await?;
    shared_world.sessions.dispatch(packets).await;
    Ok(())
}

async fn handle_set_target_obsolete(
    shared_world: SharedWorldDeps<'_>,
    body: &[u8],
    session: &WorldSessionState,
) -> anyhow::Result<()> {
    let unit_target = read_packet_guid(body, "CMSG_SET_TARGET_OBSOLETE")?;
    let unit_target = if unit_target == ObjectGuid::EMPTY {
        None
    } else {
        Some(unit_target)
    };

    let Some(character) = &session.active_character else {
        return Ok(());
    };
    let packets = shared_world
        .maps
        .update_player_target(character.position.map_id, character.guid, unit_target)
        .await?;
    shared_world.sessions.dispatch(packets).await;
    Ok(())
}

async fn handle_stand_state_change(
    shared_world: SharedWorldDeps<'_>,
    body: &[u8],
    session: &mut WorldSessionState,
) -> anyhow::Result<()> {
    if body.len() < 4 {
        anyhow::bail!("CMSG_STANDSTATECHANGE payload must include a stand state");
    }
    let mut cursor = 0;
    let stand_state = read_u32(body, &mut cursor)?;
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
    session.player_stand_state = stand_state;

    let Some(character) = &session.active_character else {
        return Ok(());
    };
    let packets = shared_world
        .maps
        .set_player_stand_state(character.position.map_id, character.guid, stand_state)
        .await?;
    shared_world.sessions.dispatch(packets).await;
    Ok(())
}

async fn handle_query_next_mail_time(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let has_unread = if let Some(character) = &session.active_character {
        wow_db::character_has_unread_mail(character_db_pool, character.guid).await?
    } else {
        false
    };
    let body = build_query_next_mail_time_body(has_unread);
    send_packet(
        stream,
        MSG_QUERY_NEXT_MAIL_TIME as u16,
        &body,
        Some(header_crypto),
    )
    .await
}

fn build_query_next_mail_time_body(has_unread: bool) -> Vec<u8> {
    let delay = if has_unread { 0.0f32 } else { -86400.0f32 };
    delay.to_le_bytes().to_vec()
}

const CHAT_YOU_JOINED_NOTICE: u8 = 0x02;
const CHANNEL_FLAG_CUSTOM: u32 = 0x01;
const CHANNEL_FLAG_TRADE: u32 = 0x04;
const CHANNEL_FLAG_NOT_LFG: u32 = 0x08;
const CHANNEL_FLAG_GENERAL: u32 = 0x10;
const CHANNEL_FLAG_CITY: u32 = 0x20;
const CHANNEL_FLAG_LFG: u32 = 0x40;

#[derive(Debug, Clone, PartialEq, Eq)]
struct JoinChannelRequest {
    channel_name: String,
    password: String,
}

impl JoinChannelRequest {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = 0;
        let channel_name = read_c_string(body, &mut cursor)?;
        let password = if cursor < body.len() {
            read_c_string(body, &mut cursor)?
        } else {
            String::new()
        };
        Ok(Self {
            channel_name,
            password,
        })
    }
}

fn build_channel_notify_you_joined_body(channel_name: &str) -> Vec<u8> {
    let mut body = Vec::with_capacity(1 + channel_name.len() + 1 + 4 + 4);
    body.push(CHAT_YOU_JOINED_NOTICE);
    write_c_string(&mut body, channel_name);
    write_u32(&mut body, channel_join_flags(channel_name));
    write_u32(&mut body, 0);
    body
}

fn channel_join_flags(channel_name: &str) -> u32 {
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
struct JumpInfo {
    z_speed: f32,
    cos_angle: f32,
    sin_angle: f32,
    xy_speed: f32,
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
struct MovementInfo {
    flags: u32,
    client_time: u32,
    position: WorldPosition,
    fall_time: u32,
    jump: JumpInfo,
}

impl MovementInfo {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
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

fn write_movement_info(
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

fn build_player_movement_broadcast_body(
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

fn read_u32(body: &[u8], cursor: &mut usize) -> anyhow::Result<u32> {
    ensure_available(body, *cursor + 4)?;
    let value = u32::from_le_bytes(body[*cursor..*cursor + 4].try_into()?);
    *cursor += 4;
    Ok(value)
}

fn read_u16(body: &[u8], cursor: &mut usize) -> anyhow::Result<u16> {
    ensure_available(body, *cursor + 2)?;
    let value = u16::from_le_bytes(body[*cursor..*cursor + 2].try_into()?);
    *cursor += 2;
    Ok(value)
}

fn read_packed_guid(body: &[u8], cursor: &mut usize) -> anyhow::Result<ObjectGuid> {
    ensure_available(body, *cursor + 1)?;
    let mask = body[*cursor];
    let packed_len = 1 + mask.count_ones() as usize;
    ensure_available(body, *cursor + packed_len)?;
    let mut reader = Cursor::new(&body[*cursor..*cursor + packed_len]);
    let guid = PackedGuid::read(&mut reader)?;
    *cursor += packed_len;
    Ok(guid)
}

fn read_c_string(body: &[u8], cursor: &mut usize) -> anyhow::Result<String> {
    let end = body[*cursor..]
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| anyhow::anyhow!("C string is not NUL-terminated"))?
        + *cursor;
    let value = String::from_utf8(body[*cursor..end].to_vec())?;
    *cursor = end + 1;
    Ok(value)
}

fn read_f32(body: &[u8], cursor: &mut usize) -> anyhow::Result<f32> {
    ensure_available(body, *cursor + 4)?;
    let value = f32::from_le_bytes(body[*cursor..*cursor + 4].try_into()?);
    *cursor += 4;
    Ok(value)
}

fn ensure_available(body: &[u8], end: usize) -> anyhow::Result<()> {
    if end > body.len() {
        anyhow::bail!(
            "movement packet truncated: need {} bytes, got {}",
            end,
            body.len()
        );
    }
    Ok(())
}

fn is_movement_opcode(opcode: u32) -> bool {
    matches!(
        opcode,
        MSG_MOVE_START_FORWARD
            | MSG_MOVE_START_BACKWARD
            | MSG_MOVE_STOP
            | MSG_MOVE_START_STRAFE_LEFT
            | MSG_MOVE_START_STRAFE_RIGHT
            | MSG_MOVE_STOP_STRAFE
            | MSG_MOVE_JUMP
            | MSG_MOVE_START_TURN_LEFT
            | MSG_MOVE_START_TURN_RIGHT
            | MSG_MOVE_STOP_TURN
            | MSG_MOVE_START_PITCH_UP
            | MSG_MOVE_START_PITCH_DOWN
            | MSG_MOVE_STOP_PITCH
            | MSG_MOVE_SET_RUN_MODE
            | MSG_MOVE_SET_WALK_MODE
            | MSG_MOVE_FALL_LAND
            | MSG_MOVE_START_SWIM
            | MSG_MOVE_STOP_SWIM
            | MSG_MOVE_SET_FACING
            | MSG_MOVE_SET_PITCH
            | MSG_MOVE_HEARTBEAT
            | CMSG_MOVE_FALL_RESET
    )
}

fn is_expected_noop_opcode(opcode: u32) -> bool {
    matches!(
        opcode,
        CMSG_CANCEL_TRADE
            | CMSG_ZONEUPDATE
            | CMSG_MEETINGSTONE_INFO
            | CMSG_REQUEST_RAID_INFO
            | CMSG_MOVE_TIME_SKIPPED
            | CMSG_BATTLEFIELD_STATUS
    )
}

fn expected_noop_opcode_name(opcode: u32) -> &'static str {
    match opcode {
        CMSG_CANCEL_TRADE => "CMSG_CANCEL_TRADE",
        CMSG_CANCEL_CAST => "CMSG_CANCEL_CAST",
        CMSG_CANCEL_AUTO_REPEAT_SPELL => "CMSG_CANCEL_AUTO_REPEAT_SPELL",
        CMSG_ZONEUPDATE => "CMSG_ZONEUPDATE",
        CMSG_SET_ACTIVE_MOVER => "CMSG_SET_ACTIVE_MOVER",
        MSG_QUERY_NEXT_MAIL_TIME => "MSG_QUERY_NEXT_MAIL_TIME",
        CMSG_MEETINGSTONE_INFO => "CMSG_MEETINGSTONE_INFO",
        CMSG_REQUEST_RAID_INFO => "CMSG_REQUEST_RAID_INFO",
        CMSG_MOVE_TIME_SKIPPED => "CMSG_MOVE_TIME_SKIPPED",
        CMSG_BATTLEFIELD_STATUS => "CMSG_BATTLEFIELD_STATUS",
        _ => "EXPECTED_NOOP",
    }
}

fn movement_opcode_name(opcode: u32) -> &'static str {
    match opcode {
        MSG_MOVE_START_FORWARD => "MSG_MOVE_START_FORWARD",
        MSG_MOVE_START_BACKWARD => "MSG_MOVE_START_BACKWARD",
        MSG_MOVE_STOP => "MSG_MOVE_STOP",
        MSG_MOVE_START_STRAFE_LEFT => "MSG_MOVE_START_STRAFE_LEFT",
        MSG_MOVE_START_STRAFE_RIGHT => "MSG_MOVE_START_STRAFE_RIGHT",
        MSG_MOVE_STOP_STRAFE => "MSG_MOVE_STOP_STRAFE",
        MSG_MOVE_JUMP => "MSG_MOVE_JUMP",
        MSG_MOVE_START_TURN_LEFT => "MSG_MOVE_START_TURN_LEFT",
        MSG_MOVE_START_TURN_RIGHT => "MSG_MOVE_START_TURN_RIGHT",
        MSG_MOVE_STOP_TURN => "MSG_MOVE_STOP_TURN",
        MSG_MOVE_START_PITCH_UP => "MSG_MOVE_START_PITCH_UP",
        MSG_MOVE_START_PITCH_DOWN => "MSG_MOVE_START_PITCH_DOWN",
        MSG_MOVE_STOP_PITCH => "MSG_MOVE_STOP_PITCH",
        MSG_MOVE_SET_RUN_MODE => "MSG_MOVE_SET_RUN_MODE",
        MSG_MOVE_SET_WALK_MODE => "MSG_MOVE_SET_WALK_MODE",
        MSG_MOVE_FALL_LAND => "MSG_MOVE_FALL_LAND",
        MSG_MOVE_START_SWIM => "MSG_MOVE_START_SWIM",
        MSG_MOVE_STOP_SWIM => "MSG_MOVE_STOP_SWIM",
        MSG_MOVE_SET_FACING => "MSG_MOVE_SET_FACING",
        MSG_MOVE_SET_PITCH => "MSG_MOVE_SET_PITCH",
        MSG_MOVE_HEARTBEAT => "MSG_MOVE_HEARTBEAT",
        CMSG_MOVE_FALL_RESET => "CMSG_MOVE_FALL_RESET",
        _ => "UNKNOWN_MOVEMENT",
    }
}

fn build_char_enum_body(characters: &[CharacterEnumEntry]) -> anyhow::Result<Vec<u8>> {
    if characters.len() > u8::MAX as usize {
        anyhow::bail!(
            "too many characters for SMSG_CHAR_ENUM: {}",
            characters.len()
        );
    }

    let mut body = Vec::with_capacity(1 + characters.len() * 90);
    body.push(characters.len() as u8);

    for character in characters {
        write_character_enum_entry(&mut body, character)?;
    }

    Ok(body)
}

fn write_character_enum_entry(
    body: &mut Vec<u8>,
    character: &CharacterEnumEntry,
) -> anyhow::Result<()> {
    let guid = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    write_guid(body, guid)?;
    write_c_string(body, &character.name);
    body.push(character.race);
    body.push(character.class);
    body.push(character.gender);

    body.push((character.player_bytes & 0xFF) as u8);
    body.push(((character.player_bytes >> 8) & 0xFF) as u8);
    body.push(((character.player_bytes >> 16) & 0xFF) as u8);
    body.push(((character.player_bytes >> 24) & 0xFF) as u8);
    body.push((character.player_bytes2 & 0xFF) as u8);

    body.push(character.level);
    body.extend_from_slice(&character.zone.to_le_bytes());
    body.extend_from_slice(&character.map.to_le_bytes());
    body.extend_from_slice(&character.position_x.to_le_bytes());
    body.extend_from_slice(&character.position_y.to_le_bytes());
    body.extend_from_slice(&character.position_z.to_le_bytes());
    body.extend_from_slice(&character.guildid.unwrap_or(0).to_le_bytes());
    body.extend_from_slice(&character_flags(character).to_le_bytes());
    body.push(if character.at_login & AT_LOGIN_FIRST != 0 {
        1
    } else {
        0
    });

    let show_pet =
        character.player_flags & PLAYER_FLAGS_GHOST == 0 && matches!(character.class, 3 | 9);
    let pet_display_id = if show_pet {
        character.pet_modelid.unwrap_or(0)
    } else {
        0
    };
    let pet_level = if show_pet {
        character.pet_level.unwrap_or(0)
    } else {
        0
    };
    body.extend_from_slice(&pet_display_id.to_le_bytes());
    body.extend_from_slice(&pet_level.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes()); // pet family requires creature template data.

    let equipment = parse_equipment_cache(character.equipment_cache.as_deref());
    for item_id in equipment {
        if let Some(visual) = starter_item_visual(item_id) {
            body.extend_from_slice(&visual.display_id.to_le_bytes());
            body.push(visual.inventory_type);
        } else {
            body.extend_from_slice(&0u32.to_le_bytes());
            body.push(0);
        }
    }

    Ok(())
}

fn write_c_string(body: &mut Vec<u8>, value: &str) {
    body.extend_from_slice(value.as_bytes());
    body.push(0);
}

fn write_u32(body: &mut Vec<u8>, value: u32) {
    body.extend_from_slice(&value.to_le_bytes());
}

fn write_i32(body: &mut Vec<u8>, value: i32) {
    body.extend_from_slice(&value.to_le_bytes());
}

fn write_f32(body: &mut Vec<u8>, value: f32) {
    body.extend_from_slice(&value.to_le_bytes());
}

fn character_flags(character: &CharacterEnumEntry) -> u32 {
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

async fn send_packet(
    stream: &mut WorldPacketSink,
    opcode: u16,
    body: &[u8],
    _header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    stream.send(opcode, body)
}

async fn send_packet_direct<W>(
    stream: &mut W,
    opcode: u16,
    body: &[u8],
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let size = (body.len() + 2) as u16;
    let mut packet = Vec::with_capacity(4 + body.len());
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

async fn read_client_packet<R>(
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

#[derive(Debug)]
struct AuthSessionPacket {
    client_build: u32,
    account: String,
    client_seed: u32,
    digest: [u8; 20],
    addon_data: Vec<u8>,
}

impl AuthSessionPacket {
    fn read(payload: &[u8]) -> anyhow::Result<Self> {
        if payload.len() < 4 + 4 + 1 + 4 + 20 {
            anyhow::bail!(
                "CMSG_AUTH_SESSION payload too short: {} bytes",
                payload.len()
            );
        }

        let client_build = u32::from_le_bytes(payload[0..4].try_into()?);
        let _unk2 = u32::from_le_bytes(payload[4..8].try_into()?);

        let mut cursor = 8;
        let account_end = payload[cursor..]
            .iter()
            .position(|b| *b == 0)
            .ok_or_else(|| anyhow::anyhow!("CMSG_AUTH_SESSION account is not NUL-terminated"))?
            + cursor;
        let account = String::from_utf8(payload[cursor..account_end].to_vec())?;
        cursor = account_end + 1;

        if payload.len() < cursor + 4 + 20 {
            anyhow::bail!("CMSG_AUTH_SESSION truncated after account");
        }

        let client_seed = u32::from_le_bytes(payload[cursor..cursor + 4].try_into()?);
        cursor += 4;

        let mut digest = [0u8; 20];
        digest.copy_from_slice(&payload[cursor..cursor + 20]);
        cursor += 20;

        let addon_data = payload[cursor..].to_vec();

        Ok(Self {
            client_build,
            account,
            client_seed,
            digest,
            addon_data,
        })
    }
}

fn verify_world_digest(auth: &AuthSessionPacket, session_key: &[u8; 40]) -> bool {
    let mut hasher = Sha1::new();
    hasher.update(auth.account.as_bytes());
    hasher.update(0u32.to_le_bytes());
    hasher.update(auth.client_seed.to_le_bytes());
    hasher.update(SERVER_SEED.to_le_bytes());
    hasher.update(session_key);
    let digest: [u8; 20] = hasher.finalize().into();
    digest == auth.digest
}

fn hex_to_array40(hex: &str) -> anyhow::Result<[u8; 40]> {
    let bytes = hex_to_vec(hex)?;
    if bytes.len() != 40 {
        anyhow::bail!("expected 40-byte session key, got {} bytes", bytes.len());
    }

    let mut out = [0u8; 40];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn hex_to_vec(hex: &str) -> anyhow::Result<Vec<u8>> {
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

fn hex_nibble(c: u8) -> anyhow::Result<u8> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => anyhow::bail!("invalid hex character 0x{c:02X}"),
    }
}

