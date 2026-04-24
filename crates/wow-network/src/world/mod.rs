use sha1::{Digest, Sha1};
use sqlx::mysql::MySqlPool;
use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info, warn};
use wow_common::guid::{write_guid, HighGuid, ObjectGuid, PackedGuid};
use wow_common::position::WorldPosition;
use wow_crypto::HeaderCrypto;
use wow_db::{CharacterEnumEntry, CharacterNameQuery, NewCharacter};

const CMSG_CHAR_CREATE: u32 = 0x0036;
const CMSG_CHAR_ENUM: u32 = 0x0037;
const CMSG_PLAYER_LOGIN: u32 = 0x003D;
const SMSG_CHAR_CREATE: u16 = 0x003A;
const CMSG_PLAYER_LOGOUT: u32 = 0x004A;
const CMSG_LOGOUT_REQUEST: u32 = 0x004B;
const SMSG_LOGOUT_RESPONSE: u16 = 0x004C;
const SMSG_LOGOUT_COMPLETE: u16 = 0x004D;
const CMSG_LOGOUT_CANCEL: u32 = 0x004E;
const SMSG_LOGOUT_CANCEL_ACK: u16 = 0x004F;
const CMSG_NAME_QUERY: u32 = 0x0050;
const SMSG_NAME_QUERY_RESPONSE: u16 = 0x0051;
const CMSG_JOIN_CHANNEL: u32 = 0x0097;
const MSG_MOVE_START_FORWARD: u32 = 0x00B5;
const MSG_MOVE_START_BACKWARD: u32 = 0x00B6;
const MSG_MOVE_STOP: u32 = 0x00B7;
const MSG_MOVE_START_STRAFE_LEFT: u32 = 0x00B8;
const MSG_MOVE_START_STRAFE_RIGHT: u32 = 0x00B9;
const MSG_MOVE_STOP_STRAFE: u32 = 0x00BA;
const MSG_MOVE_JUMP: u32 = 0x00BB;
const MSG_MOVE_START_TURN_LEFT: u32 = 0x00BC;
const MSG_MOVE_START_TURN_RIGHT: u32 = 0x00BD;
const MSG_MOVE_STOP_TURN: u32 = 0x00BE;
const MSG_MOVE_START_PITCH_UP: u32 = 0x00BF;
const MSG_MOVE_START_PITCH_DOWN: u32 = 0x00C0;
const MSG_MOVE_STOP_PITCH: u32 = 0x00C1;
const MSG_MOVE_SET_RUN_MODE: u32 = 0x00C2;
const MSG_MOVE_SET_WALK_MODE: u32 = 0x00C3;
const MSG_MOVE_FALL_LAND: u32 = 0x00C9;
const MSG_MOVE_START_SWIM: u32 = 0x00CA;
const MSG_MOVE_STOP_SWIM: u32 = 0x00CB;
const MSG_MOVE_SET_FACING: u32 = 0x00DA;
const MSG_MOVE_SET_PITCH: u32 = 0x00DB;
const MSG_MOVE_HEARTBEAT: u32 = 0x00EE;
const CMSG_MOVE_FALL_RESET: u32 = 0x02CA;
const CMSG_TUTORIAL_FLAG: u32 = 0x00FE;
const CMSG_TUTORIAL_CLEAR: u32 = 0x00FF;
const CMSG_TUTORIAL_RESET: u32 = 0x0100;
const CMSG_CANCEL_TRADE: u32 = 0x011C;
const CMSG_SET_SELECTION: u32 = 0x013D;
const CMSG_QUERY_TIME: u32 = 0x01CE;
const SMSG_QUERY_TIME_RESPONSE: u16 = 0x01CF;
const CMSG_ZONEUPDATE: u32 = 0x01F4;
const CMSG_REQUEST_ACCOUNT_DATA: u32 = 0x020A;
const CMSG_UPDATE_ACCOUNT_DATA: u32 = 0x020B;
const SMSG_UPDATE_ACCOUNT_DATA: u16 = 0x020C;
const CMSG_GMTICKET_GETTICKET: u32 = 0x0211;
const SMSG_GMTICKET_GETTICKET: u16 = 0x0212;
const CMSG_SET_ACTIVE_MOVER: u32 = 0x026A;
const MSG_QUERY_NEXT_MAIL_TIME: u32 = 0x0284;
const CMSG_MEETINGSTONE_INFO: u32 = 0x0296;
const CMSG_REQUEST_RAID_INFO: u32 = 0x02CD;
const CMSG_MOVE_TIME_SKIPPED: u32 = 0x02CE;
const CMSG_BATTLEFIELD_STATUS: u32 = 0x02D3;
const SMSG_CHAR_ENUM: u16 = 0x003B;
const SMSG_CHARACTER_LOGIN_FAILED: u16 = 0x0041;
const SMSG_LOGIN_SETTIMESPEED: u16 = 0x0042;
const SMSG_TUTORIAL_FLAGS: u16 = 0x00FD;
const SMSG_UPDATE_OBJECT: u16 = 0x00A9;
const SMSG_ACTION_BUTTONS: u16 = 0x0129;
const SMSG_INITIAL_SPELLS: u16 = 0x012A;
const SMSG_BINDPOINTUPDATE: u16 = 0x0155;
const SMSG_ACCOUNT_DATA_TIMES: u16 = 0x0209;
const SMSG_LOGIN_VERIFY_WORLD: u16 = 0x0236;
const SMSG_INIT_WORLD_STATES: u16 = 0x02C2;
const SMSG_AUTH_CHALLENGE: u16 = 0x01EC;
const CMSG_AUTH_SESSION: u32 = 0x01ED;
const SMSG_AUTH_RESPONSE: u16 = 0x01EE;
const CMSG_PING: u32 = 0x01DC;
const SMSG_PONG: u16 = 0x01DD;
const AUTH_OK: u8 = 0x0C;
const AUTH_FAILED: u8 = 0x0D;
const AUTH_VERSION_MISMATCH: u8 = 0x14;
const AUTH_UNKNOWN_ACCOUNT: u8 = 0x15;
const CHAR_CREATE_SUCCESS: u8 = 0x2E;
const CHAR_CREATE_FAILED: u8 = 0x30;
const CHAR_CREATE_NAME_IN_USE: u8 = 0x31;
const CHAR_CREATE_SERVER_LIMIT: u8 = 0x34;
const CHAR_NAME_NO_NAME: u8 = 0x43;
const CHAR_NAME_TOO_SHORT: u8 = 0x44;
const CHAR_NAME_TOO_LONG: u8 = 0x45;
const CHAR_NAME_INVALID_CHARACTER: u8 = 0x46;
const CHAR_LOGIN_NO_CHARACTER: u8 = 0x05;

const SERVER_SEED: u32 = 0xC0DEC0DE;
const PLAYER_FLAGS_GHOST: u32 = 0x0000_0010;
const PLAYER_FLAGS_HIDE_HELM: u32 = 0x0000_0400;
const PLAYER_FLAGS_HIDE_CLOAK: u32 = 0x0000_0800;
const CHARACTER_FLAG_HIDE_HELM: u32 = 0x0000_0400;
const CHARACTER_FLAG_HIDE_CLOAK: u32 = 0x0000_0800;
const CHARACTER_FLAG_GHOST: u32 = 0x0000_2000;
const CHARACTER_FLAG_RENAME: u32 = 0x0000_4000;
const AT_LOGIN_RENAME: u32 = 0x01;
const AT_LOGIN_FIRST: u32 = 0x20;
const ENUM_EQUIPMENT_SLOTS: usize = 20;
const ACCOUNT_DATA_TYPES: usize = 8;
const MD5_DIGEST_LEN: usize = 16;
const MAX_ACTION_BUTTONS: usize = 120;
const TYPEID_PLAYER: u8 = 4;
const TYPEMASK_OBJECT_UNIT_PLAYER: u32 = 0x0019;
const UPDATE_TYPE_CREATE_OBJECT2: u8 = 3;
const UPDATEFLAG_SELF: u8 = 0x01;
const UPDATEFLAG_ALL: u8 = 0x10;
const UPDATEFLAG_LIVING: u8 = 0x20;
const UPDATEFLAG_HAS_POSITION: u8 = 0x40;
const PLAYER_END_FIELDS: usize = 0x502;
const MOVEFLAG_JUMPING: u32 = 0x0000_2000;
const MOVEFLAG_SWIMMING: u32 = 0x0020_0000;
const MOVEFLAG_ONTRANSPORT: u32 = 0x0200_0000;
const MOVEFLAG_SPLINE_ELEVATION: u32 = 0x0400_0000;
const REALM_ID: u32 = 1;
const MAX_CHARACTERS_PER_REALM: u8 = 10;

pub struct WorldServer {
    bind_addr: SocketAddr,
    login_db_pool: MySqlPool,
    character_db_pool: MySqlPool,
}

impl WorldServer {
    pub fn new(
        bind_addr: SocketAddr,
        login_db_pool: MySqlPool,
        character_db_pool: MySqlPool,
    ) -> Self {
        Self {
            bind_addr,
            login_db_pool,
            character_db_pool,
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
                    tokio::spawn(async move {
                        if let Err(e) = handle_client(socket, login_pool, character_pool).await {
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
) -> anyhow::Result<()> {
    send_packet(
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

    let mut header_crypto = HeaderCrypto::new(&session_key);
    send_auth_ok(&mut stream, Some(&mut header_crypto)).await?;
    let mut session = WorldSessionState::default();

    loop {
        match read_client_packet(&mut stream, Some(&mut header_crypto)).await {
            Ok((opcode, body)) => {
                info!(
                    opcode = format_args!("0x{opcode:04X}"),
                    bytes = body.len(),
                    "Received world packet after auth"
                );

                match opcode {
                    CMSG_CHAR_CREATE => {
                        handle_char_create(
                            &mut stream,
                            &login_db_pool,
                            &character_db_pool,
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
                        send_char_enum(&mut stream, &characters, Some(&mut header_crypto)).await?;
                    }
                    CMSG_PLAYER_LOGIN => {
                        handle_player_login(
                            &mut stream,
                            &character_db_pool,
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
                    CMSG_QUERY_TIME => {
                        handle_query_time(&mut stream, &mut header_crypto).await?;
                    }
                    CMSG_REQUEST_ACCOUNT_DATA => {
                        handle_request_account_data(&mut stream, &body, &mut header_crypto).await?;
                    }
                    CMSG_UPDATE_ACCOUNT_DATA => {
                        handle_update_account_data(&body);
                    }
                    CMSG_GMTICKET_GETTICKET => {
                        handle_gmticket_getticket(&mut stream, &mut header_crypto).await?;
                    }
                    CMSG_LOGOUT_REQUEST => {
                        handle_logout_request(
                            &mut stream,
                            &character_db_pool,
                            account.id,
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
                        handle_movement(opcode, &body, &mut session)?;
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
            }
            Err(e) => {
                persist_active_character_position(&character_db_pool, account.id, &session).await?;
                info!("World client disconnected or read failed: {}", e);
                return Ok(());
            }
        }
    }
}

#[derive(Debug, Default)]
struct WorldSessionState {
    active_character: Option<ActiveCharacter>,
}

#[derive(Debug)]
struct ActiveCharacter {
    guid: u32,
    name: String,
    position: WorldPosition,
    movement_flags: u32,
    client_time: u32,
    fall_time: u32,
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
    send_packet(stream, SMSG_AUTH_RESPONSE, &[response], None).await
}

async fn send_auth_ok(
    stream: &mut TcpStream,
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
    stream: &mut TcpStream,
    characters: &[CharacterEnumEntry],
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_char_enum_body(characters)?;
    send_packet(stream, SMSG_CHAR_ENUM, &body, header_crypto).await
}

async fn handle_char_create(
    stream: &mut TcpStream,
    login_db_pool: &MySqlPool,
    character_db_pool: &MySqlPool,
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

    let new_count = char_count.saturating_add(1);
    wow_db::set_realm_character_count(login_db_pool, account_id, REALM_ID, new_count).await?;

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
    stream: &mut TcpStream,
    result: u8,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    send_packet(stream, SMSG_CHAR_CREATE, &[result], header_crypto).await
}

async fn handle_player_login(
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
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
    let characters = wow_db::get_character_enum_entries(character_db_pool, account_id).await?;
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

    info!(
        account_id,
        guid = character.guid,
        name = %character.name,
        map = character.map,
        "Character login selected"
    );
    session.active_character = Some(ActiveCharacter {
        guid: character.guid,
        name: character.name.clone(),
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
    send_enter_world_bootstrap(stream, character, Some(header_crypto)).await
}

async fn handle_logout_request(
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
    account_id: u32,
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
    persist_active_character_position(character_db_pool, account_id, session).await?;
    session.active_character = None;
    Ok(())
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
    stream: &mut TcpStream,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    send_packet(stream, SMSG_LOGOUT_CANCEL_ACK, &[], Some(header_crypto)).await
}

fn handle_movement(
    opcode: u32,
    body: &[u8],
    session: &mut WorldSessionState,
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
        info!(
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
    } else {
        warn!(
            opcode = movement_opcode_name(opcode),
            "Received movement packet before character login"
        );
    }
    Ok(())
}

async fn send_enter_world_bootstrap(
    stream: &mut TcpStream,
    character: &CharacterEnumEntry,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let mut header_crypto = header_crypto;
    send_login_verify_world(stream, character, header_crypto.as_deref_mut()).await?;
    send_account_data_times(stream, header_crypto.as_deref_mut()).await?;
    send_bindpoint_update(stream, character, header_crypto.as_deref_mut()).await?;
    send_tutorial_flags(stream, header_crypto.as_deref_mut()).await?;
    send_initial_spells(stream, header_crypto.as_deref_mut()).await?;
    send_action_buttons(stream, header_crypto.as_deref_mut()).await?;
    send_login_set_time_speed(stream, header_crypto.as_deref_mut()).await?;
    send_init_world_states(stream, character, header_crypto.as_deref_mut()).await?;
    send_self_spawn_update(stream, character, header_crypto).await?;
    Ok(())
}

async fn send_login_verify_world(
    stream: &mut TcpStream,
    character: &CharacterEnumEntry,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let mut body = Vec::with_capacity(20);
    body.extend_from_slice(&character.map.to_le_bytes());
    body.extend_from_slice(&character.position_x.to_le_bytes());
    body.extend_from_slice(&character.position_y.to_le_bytes());
    body.extend_from_slice(&character.position_z.to_le_bytes());
    body.extend_from_slice(&character.orientation.to_le_bytes());
    send_packet(stream, SMSG_LOGIN_VERIFY_WORLD, &body, header_crypto).await
}

async fn send_account_data_times(
    stream: &mut TcpStream,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = vec![0u8; ACCOUNT_DATA_TYPES * MD5_DIGEST_LEN];
    send_packet(stream, SMSG_ACCOUNT_DATA_TIMES, &body, header_crypto).await
}

async fn send_bindpoint_update(
    stream: &mut TcpStream,
    character: &CharacterEnumEntry,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let mut body = Vec::with_capacity(20);
    body.extend_from_slice(&character.position_x.to_le_bytes());
    body.extend_from_slice(&character.position_y.to_le_bytes());
    body.extend_from_slice(&character.position_z.to_le_bytes());
    body.extend_from_slice(&character.map.to_le_bytes());
    body.extend_from_slice(&character.zone.to_le_bytes());
    send_packet(stream, SMSG_BINDPOINTUPDATE, &body, header_crypto).await
}

async fn send_tutorial_flags(
    stream: &mut TcpStream,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = vec![0u8; 8 * 4];
    send_packet(stream, SMSG_TUTORIAL_FLAGS, &body, header_crypto).await
}

async fn send_initial_spells(
    stream: &mut TcpStream,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let mut body = Vec::with_capacity(5);
    body.push(0); // unknown flags byte
    body.extend_from_slice(&0u16.to_le_bytes()); // spell count
    body.extend_from_slice(&0u16.to_le_bytes()); // cooldown count
    send_packet(stream, SMSG_INITIAL_SPELLS, &body, header_crypto).await
}

async fn send_action_buttons(
    stream: &mut TcpStream,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = vec![0u8; MAX_ACTION_BUTTONS * 4];
    send_packet(stream, SMSG_ACTION_BUTTONS, &body, header_crypto).await
}

async fn send_login_set_time_speed(
    stream: &mut TcpStream,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let mut body = Vec::with_capacity(8);
    body.extend_from_slice(&0u32.to_le_bytes()); // packed server time placeholder
    body.extend_from_slice(&0.01666667f32.to_le_bytes());
    send_packet(stream, SMSG_LOGIN_SETTIMESPEED, &body, header_crypto).await
}

async fn send_init_world_states(
    stream: &mut TcpStream,
    character: &CharacterEnumEntry,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let mut body = Vec::with_capacity(16);
    body.extend_from_slice(&character.map.to_le_bytes());
    body.extend_from_slice(&character.zone.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes()); // area id, unknown for this skeleton
    body.extend_from_slice(&0u32.to_le_bytes()); // world state count
    send_packet(stream, SMSG_INIT_WORLD_STATES, &body, header_crypto).await
}

async fn send_self_spawn_update(
    stream: &mut TcpStream,
    character: &CharacterEnumEntry,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_self_spawn_update_body(character)?;
    info!(
        guid = character.guid,
        name = %character.name,
        bytes = body.len(),
        "Sending minimal self spawn update"
    );
    send_packet(stream, SMSG_UPDATE_OBJECT, &body, header_crypto).await
}

fn build_self_spawn_update_body(character: &CharacterEnumEntry) -> anyhow::Result<Vec<u8>> {
    let guid = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_CREATE_OBJECT2);
    PackedGuid::write(&mut block, guid)?;
    block.push(TYPEID_PLAYER);

    block.push(UPDATEFLAG_SELF | UPDATEFLAG_ALL | UPDATEFLAG_LIVING | UPDATEFLAG_HAS_POSITION);
    block.extend_from_slice(&0u32.to_le_bytes()); // movement flags
    block.extend_from_slice(&0u32.to_le_bytes()); // server time placeholder
    block.extend_from_slice(&character.position_x.to_le_bytes());
    block.extend_from_slice(&character.position_y.to_le_bytes());
    block.extend_from_slice(&character.position_z.to_le_bytes());
    block.extend_from_slice(&character.orientation.to_le_bytes());
    block.extend_from_slice(&0u32.to_le_bytes()); // fall time
    block.extend_from_slice(&2.5f32.to_le_bytes()); // walk
    block.extend_from_slice(&7.0f32.to_le_bytes()); // run
    block.extend_from_slice(&4.5f32.to_le_bytes()); // run back
    block.extend_from_slice(&4.722222f32.to_le_bytes()); // swim
    block.extend_from_slice(&2.5f32.to_le_bytes()); // swim back
    block.extend_from_slice(&std::f32::consts::PI.to_le_bytes()); // turn rate
    block.extend_from_slice(&1u32.to_le_bytes()); // UPDATEFLAG_ALL payload

    write_minimal_player_update_values(&mut block, guid, character)?;

    let mut body = Vec::with_capacity(5 + block.len());
    body.extend_from_slice(&1u32.to_le_bytes()); // update block count
    body.push(0); // has transport
    body.extend_from_slice(&block);
    Ok(body)
}

fn write_minimal_player_update_values(
    body: &mut Vec<u8>,
    guid: ObjectGuid,
    character: &CharacterEnumEntry,
) -> anyhow::Result<()> {
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, 0x000, guid.raw() as u32)?;
    set_update_value(&mut values, 0x001, (guid.raw() >> 32) as u32)?;
    set_update_value(&mut values, 0x002, TYPEMASK_OBJECT_UNIT_PLAYER)?;
    set_update_value(&mut values, 0x004, 1.0f32.to_bits())?;
    set_update_value(&mut values, 0x016, 1)?;
    set_update_value(&mut values, 0x01C, 1)?;
    set_update_value(&mut values, 0x022, character.level as u32)?;
    set_update_value(&mut values, 0x023, faction_for_race(character.race))?;
    set_update_value(&mut values, 0x024, unit_bytes_0(character))?;
    set_update_value(&mut values, 0x081, 0.389f32.to_bits())?;
    set_update_value(&mut values, 0x082, 1.5f32.to_bits())?;
    set_update_value(&mut values, 0x083, display_id_for_character(character))?;
    set_update_value(&mut values, 0x084, display_id_for_character(character))?;
    set_update_value(&mut values, 0x091, 1.0f32.to_bits())?;
    set_update_value(&mut values, 0x0BE, character.player_flags)?;
    set_update_value(&mut values, 0x0C1, character.player_bytes)?;
    set_update_value(&mut values, 0x0C2, character.player_bytes2)?;
    set_update_value(&mut values, 0x0C3, 0)?;
    set_update_value(&mut values, 0x2CC, 0)?;
    set_update_value(&mut values, 0x2CD, 400)?;
    set_update_value(&mut values, 0x4C8, 0)?;

    let block_count = values.len().div_ceil(32);
    body.push(block_count as u8);
    let mask_start = body.len();
    body.resize(mask_start + block_count * 4, 0);

    for (index, value) in values.iter().enumerate() {
        if let Some(value) = value {
            let block = index / 32;
            let bit = index % 32;
            let offset = mask_start + block * 4;
            let mut mask = u32::from_le_bytes(body[offset..offset + 4].try_into()?);
            mask |= 1u32 << bit;
            body[offset..offset + 4].copy_from_slice(&mask.to_le_bytes());
            body.extend_from_slice(&value.to_le_bytes());
        }
    }

    Ok(())
}

fn set_update_value(values: &mut [Option<u32>], index: usize, value: u32) -> anyhow::Result<()> {
    if index >= values.len() {
        anyhow::bail!("update field index {index} exceeds player field count");
    }
    values[index] = Some(value);
    Ok(())
}

fn unit_bytes_0(character: &CharacterEnumEntry) -> u32 {
    let power_type = match character.class {
        1 => 1, // warrior rage
        4 => 3, // rogue energy
        _ => 0, // mana
    };
    character.race as u32
        | ((character.class as u32) << 8)
        | ((character.gender as u32) << 16)
        | (power_type << 24)
}

fn faction_for_race(race: u8) -> u32 {
    match race {
        1 | 3 | 4 | 7 => 1,
        2 | 5 | 6 | 8 => 2,
        _ => 1,
    }
}

fn display_id_for_character(character: &CharacterEnumEntry) -> u32 {
    match (character.race, character.gender) {
        (1, 0) => 49,
        (1, 1) => 50,
        _ => 49,
    }
}

async fn handle_ping(
    stream: &mut TcpStream,
    body: &[u8],
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    if body.len() < 4 {
        anyhow::bail!("CMSG_PING payload too short: {} bytes", body.len());
    }

    let ping = u32::from_le_bytes(body[0..4].try_into()?);
    send_packet(stream, SMSG_PONG, &ping.to_le_bytes(), header_crypto).await
}

async fn handle_name_query(
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if body.len() != 8 {
        anyhow::bail!(
            "CMSG_NAME_QUERY payload must be 8 bytes, got {}",
            body.len()
        );
    }

    let raw_guid = u64::from_le_bytes(body.try_into()?);
    let guid = ObjectGuid::from_raw(raw_guid);
    let character_guid = guid.counter();
    let character = wow_db::get_character_name_query(character_db_pool, character_guid).await?;
    let response = build_name_query_response(raw_guid, character.as_ref());
    send_packet(
        stream,
        SMSG_NAME_QUERY_RESPONSE,
        &response,
        Some(header_crypto),
    )
    .await
}

fn build_name_query_response(
    requested_guid: u64,
    character: Option<&CharacterNameQuery>,
) -> Vec<u8> {
    let mut body = Vec::with_capacity(8 + 1 + 1 + 12);
    match character {
        Some(character) => {
            let guid = ObjectGuid::new(HighGuid::Player, 0, character.guid);
            body.extend_from_slice(&guid.raw().to_le_bytes());
            write_c_string(&mut body, &character.name);
            body.push(0); // realm name
            body.extend_from_slice(&(character.race as u32).to_le_bytes());
            body.extend_from_slice(&(character.gender as u32).to_le_bytes());
            body.extend_from_slice(&(character.class as u32).to_le_bytes());
        }
        None => {
            body.extend_from_slice(&requested_guid.to_le_bytes());
            write_c_string(&mut body, "Unknown");
            body.push(0); // realm name
            body.extend_from_slice(&0u32.to_le_bytes());
            body.extend_from_slice(&0u32.to_le_bytes());
            body.extend_from_slice(&0u32.to_le_bytes());
        }
    }
    body
}

async fn handle_query_time(
    stream: &mut TcpStream,
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
    stream: &mut TcpStream,
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
    stream: &mut TcpStream,
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

#[derive(Debug, Clone, PartialEq)]
struct MovementInfo {
    flags: u32,
    client_time: u32,
    position: WorldPosition,
    fall_time: u32,
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

        if flags & MOVEFLAG_JUMPING != 0 {
            let _jump_z_speed = read_f32(body, &mut cursor)?;
            let _jump_cos_angle = read_f32(body, &mut cursor)?;
            let _jump_sin_angle = read_f32(body, &mut cursor)?;
            let _jump_xy_speed = read_f32(body, &mut cursor)?;
        }

        if flags & MOVEFLAG_SPLINE_ELEVATION != 0 {
            let _spline_elevation = read_f32(body, &mut cursor)?;
        }

        Ok(Self {
            flags,
            client_time,
            position: WorldPosition::new(0, x, y, z, orientation),
            fall_time,
        })
    }
}

fn read_u32(body: &[u8], cursor: &mut usize) -> anyhow::Result<u32> {
    ensure_available(body, *cursor + 4)?;
    let value = u32::from_le_bytes(body[*cursor..*cursor + 4].try_into()?);
    *cursor += 4;
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
        CMSG_JOIN_CHANNEL
            | CMSG_TUTORIAL_FLAG
            | CMSG_TUTORIAL_CLEAR
            | CMSG_TUTORIAL_RESET
            | CMSG_CANCEL_TRADE
            | CMSG_SET_SELECTION
            | CMSG_ZONEUPDATE
            | CMSG_SET_ACTIVE_MOVER
            | MSG_QUERY_NEXT_MAIL_TIME
            | CMSG_MEETINGSTONE_INFO
            | CMSG_REQUEST_RAID_INFO
            | CMSG_MOVE_TIME_SKIPPED
            | CMSG_BATTLEFIELD_STATUS
    )
}

fn expected_noop_opcode_name(opcode: u32) -> &'static str {
    match opcode {
        CMSG_JOIN_CHANNEL => "CMSG_JOIN_CHANNEL",
        CMSG_TUTORIAL_FLAG => "CMSG_TUTORIAL_FLAG",
        CMSG_TUTORIAL_CLEAR => "CMSG_TUTORIAL_CLEAR",
        CMSG_TUTORIAL_RESET => "CMSG_TUTORIAL_RESET",
        CMSG_CANCEL_TRADE => "CMSG_CANCEL_TRADE",
        CMSG_SET_SELECTION => "CMSG_SET_SELECTION",
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

    for _ in 0..ENUM_EQUIPMENT_SLOTS {
        body.extend_from_slice(&0u32.to_le_bytes()); // display id
        body.push(0); // inventory type
    }

    Ok(())
}

fn write_c_string(body: &mut Vec<u8>, value: &str) {
    body.extend_from_slice(value.as_bytes());
    body.push(0);
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
    stream: &mut TcpStream,
    opcode: u16,
    body: &[u8],
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
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
    Ok(())
}

async fn read_client_packet(
    stream: &mut TcpStream,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<(u32, Vec<u8>)> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_packet_header_matches_world_shape() {
        let mut packet = Vec::new();
        packet.extend_from_slice(&(4u16 + 2).to_be_bytes());
        packet.extend_from_slice(&SMSG_AUTH_CHALLENGE.to_le_bytes());
        packet.extend_from_slice(&SERVER_SEED.to_le_bytes());

        assert_eq!(&packet[0..2], &[0x00, 0x06]);
        assert_eq!(&packet[2..4], &[0xEC, 0x01]);
        assert_eq!(packet.len(), 8);
    }

    #[test]
    fn empty_char_enum_packet_shape() {
        let body = build_char_enum_body(&[]).unwrap();
        let mut packet = Vec::new();
        packet.extend_from_slice(&(body.len() as u16 + 2).to_be_bytes());
        packet.extend_from_slice(&SMSG_CHAR_ENUM.to_le_bytes());
        packet.extend_from_slice(&body);

        assert_eq!(packet, [0x00, 0x03, 0x3B, 0x00, 0x00]);
    }

    #[test]
    fn parses_char_create_packet() {
        let mut body = Vec::new();
        body.extend_from_slice(b"Testname\0");
        body.extend_from_slice(&[1, 1, 0, 2, 3, 4, 5, 6, 0]);

        let packet = CharCreatePacket::read(&body).unwrap();

        assert_eq!(packet.name, "Testname");
        assert_eq!(packet.race, 1);
        assert_eq!(packet.class, 1);
        assert_eq!(packet.gender, 0);
        assert_eq!(packet.skin, 2);
        assert_eq!(packet.face, 3);
        assert_eq!(packet.hair_style, 4);
        assert_eq!(packet.hair_color, 5);
        assert_eq!(packet.facial_hair, 6);
        assert_eq!(packet.outfit_id, 0);
    }

    #[test]
    fn name_query_response_matches_cmangos_shape() {
        let character = CharacterNameQuery {
            guid: 7,
            name: "Rusty".to_string(),
            race: 1,
            gender: 0,
            class: 1,
        };
        let body = build_name_query_response(7, Some(&character));

        assert_eq!(&body[0..8], &7u64.to_le_bytes());
        assert_eq!(&body[8..14], b"Rusty\0");
        assert_eq!(body[14], 0);
        assert_eq!(&body[15..19], &1u32.to_le_bytes());
        assert_eq!(&body[19..23], &0u32.to_le_bytes());
        assert_eq!(&body[23..27], &1u32.to_le_bytes());
    }

    #[test]
    fn normalizes_character_names_like_cmangos_create_path() {
        assert_eq!(normalize_character_name("rUSTY").unwrap(), "Rusty");
        assert_eq!(normalize_character_name("").unwrap_err(), CHAR_NAME_NO_NAME);
        assert_eq!(
            normalize_character_name("A").unwrap_err(),
            CHAR_NAME_TOO_SHORT
        );
        assert_eq!(
            normalize_character_name("Thirteenchars").unwrap_err(),
            CHAR_NAME_TOO_LONG
        );
        assert_eq!(
            normalize_character_name("Bad1").unwrap_err(),
            CHAR_NAME_INVALID_CHARACTER
        );
    }

    #[test]
    fn validates_classic_race_class_pairs() {
        assert!(is_valid_race_class(1, 1));
        assert!(is_valid_race_class(7, 8));
        assert!(!is_valid_race_class(1, 7));
        assert!(!is_valid_race_class(9, 1));
    }

    #[test]
    fn serializes_character_enum_entry() {
        let body = build_char_enum_body(&[CharacterEnumEntry {
            guid: 7,
            name: "Rustone".to_string(),
            race: 1,
            class: 1,
            gender: 0,
            player_bytes: 0x0403_0201,
            player_bytes2: 0x0000_0005,
            level: 1,
            zone: 12,
            map: 0,
            position_x: -8949.95,
            position_y: -132.493,
            position_z: 83.5312,
            orientation: 0.0,
            guildid: Some(0),
            player_flags: PLAYER_FLAGS_HIDE_HELM,
            at_login: AT_LOGIN_FIRST,
            pet_entry: None,
            pet_modelid: None,
            pet_level: None,
            equipment_cache: None,
        }])
        .unwrap();

        assert_eq!(body[0], 1);
        assert_eq!(&body[1..9], &7u64.to_le_bytes());
        assert_eq!(&body[9..17], b"Rustone\0");
        assert_eq!(body[17], 1);
        assert_eq!(body[18], 1);
        assert_eq!(body[19], 0);
        assert_eq!(body[20], 1);
        assert_eq!(body[21], 2);
        assert_eq!(body[22], 3);
        assert_eq!(body[23], 4);
        assert_eq!(body[24], 5);
        assert_eq!(
            body.len(),
            1 + 8 + 8 + 1 + 1 + 1 + 5 + 1 + 4 + 4 + 12 + 4 + 4 + 1 + 12 + 100
        );
    }

    #[test]
    fn login_verify_world_packet_shape() {
        let character = CharacterEnumEntry {
            guid: 7,
            name: "Rustone".to_string(),
            race: 1,
            class: 1,
            gender: 0,
            player_bytes: 0,
            player_bytes2: 0,
            level: 1,
            zone: 12,
            map: 0,
            position_x: -8949.95,
            position_y: -132.493,
            position_z: 83.5312,
            orientation: 1.25,
            guildid: None,
            player_flags: 0,
            at_login: 0,
            pet_entry: None,
            pet_modelid: None,
            pet_level: None,
            equipment_cache: None,
        };

        let mut body = Vec::new();
        body.extend_from_slice(&character.map.to_le_bytes());
        body.extend_from_slice(&character.position_x.to_le_bytes());
        body.extend_from_slice(&character.position_y.to_le_bytes());
        body.extend_from_slice(&character.position_z.to_le_bytes());
        body.extend_from_slice(&character.orientation.to_le_bytes());

        assert_eq!(body.len(), 20);
        assert_eq!(&body[0..4], &0u32.to_le_bytes());
        assert_eq!(&body[16..20], &1.25f32.to_le_bytes());
    }

    #[test]
    fn empty_initial_spells_shape() {
        let mut body = Vec::new();
        body.push(0);
        body.extend_from_slice(&0u16.to_le_bytes());
        body.extend_from_slice(&0u16.to_le_bytes());

        assert_eq!(body, [0, 0, 0, 0, 0]);
    }

    #[test]
    fn parses_basic_movement_info() {
        let mut body = Vec::new();
        body.extend_from_slice(&0x0000_0001u32.to_le_bytes());
        body.extend_from_slice(&1234u32.to_le_bytes());
        body.extend_from_slice(&1.25f32.to_le_bytes());
        body.extend_from_slice(&2.5f32.to_le_bytes());
        body.extend_from_slice(&3.75f32.to_le_bytes());
        body.extend_from_slice(&1.0f32.to_le_bytes());
        body.extend_from_slice(&456u32.to_le_bytes());

        let movement = MovementInfo::read(&body).unwrap();

        assert_eq!(movement.flags, 1);
        assert_eq!(movement.client_time, 1234);
        assert_eq!(movement.position.x, 1.25);
        assert_eq!(movement.position.y, 2.5);
        assert_eq!(movement.position.z, 3.75);
        assert_eq!(movement.position.orientation, 1.0);
        assert_eq!(movement.fall_time, 456);
    }

    #[test]
    fn parses_jump_movement_info() {
        let mut body = Vec::new();
        body.extend_from_slice(&MOVEFLAG_JUMPING.to_le_bytes());
        body.extend_from_slice(&1234u32.to_le_bytes());
        body.extend_from_slice(&1.25f32.to_le_bytes());
        body.extend_from_slice(&2.5f32.to_le_bytes());
        body.extend_from_slice(&3.75f32.to_le_bytes());
        body.extend_from_slice(&1.0f32.to_le_bytes());
        body.extend_from_slice(&456u32.to_le_bytes());
        body.extend_from_slice(&7.0f32.to_le_bytes());
        body.extend_from_slice(&0.0f32.to_le_bytes());
        body.extend_from_slice(&1.0f32.to_le_bytes());
        body.extend_from_slice(&4.5f32.to_le_bytes());

        let movement = MovementInfo::read(&body).unwrap();

        assert_eq!(movement.flags, MOVEFLAG_JUMPING);
        assert_eq!(movement.fall_time, 456);
        assert_eq!(movement.position.z, 3.75);
    }

    #[test]
    fn movement_info_rejects_truncated_payload() {
        let err = MovementInfo::read(&[0; 8]).unwrap_err().to_string();
        assert!(err.contains("movement packet truncated"));
    }

    #[test]
    fn recognizes_observed_movement_opcodes() {
        for opcode in [
            0x00B5, 0x00B7, 0x00B8, 0x00B9, 0x00BA, 0x00BB, 0x00BD, 0x00BE, 0x00C9, 0x00DA, 0x00EE,
        ] {
            assert!(is_movement_opcode(opcode), "opcode 0x{opcode:04X}");
        }
    }

    #[test]
    fn recognizes_expected_world_bootstrap_noise() {
        for opcode in [
            CMSG_JOIN_CHANNEL,
            CMSG_TUTORIAL_FLAG,
            CMSG_CANCEL_TRADE,
            CMSG_ZONEUPDATE,
            CMSG_SET_ACTIVE_MOVER,
            MSG_QUERY_NEXT_MAIL_TIME,
            CMSG_MEETINGSTONE_INFO,
            CMSG_REQUEST_RAID_INFO,
            CMSG_MOVE_TIME_SKIPPED,
            CMSG_BATTLEFIELD_STATUS,
        ] {
            assert!(is_expected_noop_opcode(opcode), "opcode 0x{opcode:04X}");
        }
    }

    #[test]
    fn parses_auth_session_packet() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&5875u32.to_le_bytes());
        payload.extend_from_slice(&0u32.to_le_bytes());
        payload.extend_from_slice(b"RUSTAUTH\0");
        payload.extend_from_slice(&0xAABBCCDDu32.to_le_bytes());
        payload.extend_from_slice(&[0x11; 20]);
        payload.extend_from_slice(&[0x22, 0x33]);

        let auth = AuthSessionPacket::read(&payload).unwrap();
        assert_eq!(auth.client_build, 5875);
        assert_eq!(auth.account, "RUSTAUTH");
        assert_eq!(auth.client_seed, 0xAABBCCDD);
        assert_eq!(auth.digest, [0x11; 20]);
        assert_eq!(auth.addon_data, [0x22, 0x33]);
    }
}
