use sha1::{Digest, Sha1};
use sqlx::mysql::MySqlPool;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use wow_common::guid::{write_guid, HighGuid, ObjectGuid, PackedGuid};
use wow_common::position::WorldPosition;
use wow_crypto::HeaderCrypto;
use wow_db::{
    CharacterAction, CharacterDeleteOptions, CharacterEnumEntry, CharacterInventoryItem,
    CharacterNameQuery, CharacterSpell, NewCharacter, PlayerWorldStats,
};

const CMSG_CHAR_CREATE: u32 = 0x0036;
const CMSG_CHAR_ENUM: u32 = 0x0037;
const CMSG_CHAR_DELETE: u32 = 0x0038;
const CMSG_PLAYER_LOGIN: u32 = 0x003D;
const SMSG_CHAR_CREATE: u16 = 0x003A;
const SMSG_CHAR_DELETE: u16 = 0x003C;
const CMSG_PLAYER_LOGOUT: u32 = 0x004A;
const CMSG_LOGOUT_REQUEST: u32 = 0x004B;
const SMSG_LOGOUT_RESPONSE: u16 = 0x004C;
const SMSG_LOGOUT_COMPLETE: u16 = 0x004D;
const CMSG_LOGOUT_CANCEL: u32 = 0x004E;
const SMSG_LOGOUT_CANCEL_ACK: u16 = 0x004F;
const CMSG_NAME_QUERY: u32 = 0x0050;
const SMSG_NAME_QUERY_RESPONSE: u16 = 0x0051;
const CMSG_ITEM_QUERY_SINGLE: u32 = 0x0056;
const SMSG_ITEM_QUERY_SINGLE_RESPONSE: u16 = 0x0058;
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
const SMSG_INITIALIZE_FACTIONS: u16 = 0x0122;
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
const CHAR_DELETE_SUCCESS: u8 = 0x39;
const CHAR_DELETE_FAILED: u8 = 0x3A;
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
const TYPEID_ITEM: u8 = 1;
const TYPEID_PLAYER: u8 = 4;
const TYPEMASK_OBJECT_ITEM: u32 = 0x0003;
const TYPEMASK_OBJECT_UNIT_PLAYER: u32 = 0x0019;
const UPDATE_TYPE_CREATE_OBJECT: u8 = 2;
const UPDATE_TYPE_CREATE_OBJECT2: u8 = 3;
const UPDATEFLAG_SELF: u8 = 0x01;
const UPDATEFLAG_ALL: u8 = 0x10;
const UPDATEFLAG_LIVING: u8 = 0x20;
const UPDATEFLAG_HAS_POSITION: u8 = 0x40;
const ITEM_END_FIELDS: usize = 0x30;
const PLAYER_END_FIELDS: usize = 0x502;
const MOVEFLAG_JUMPING: u32 = 0x0000_2000;
const MOVEFLAG_SWIMMING: u32 = 0x0020_0000;
const MOVEFLAG_ONTRANSPORT: u32 = 0x0200_0000;
const MOVEFLAG_SPLINE_ELEVATION: u32 = 0x0400_0000;
const REALM_ID: u32 = 1;
const MAX_CHARACTERS_PER_REALM: u8 = 10;
const FORM_BATTLESTANCE: u8 = 0x11;
const EQUIPMENT_SLOT_END: u8 = 19;
const POWER_MANA: u8 = 0;
const POWER_RAGE: u8 = 1;
const POWER_FOCUS: u8 = 2;
const POWER_ENERGY: u8 = 3;
const POWER_HAPPINESS: u8 = 4;
const POWER_RAGE_DEFAULT: u32 = 1000;
const POWER_ENERGY_DEFAULT: u32 = 100;
const REPUTATION_LIST_SLOTS: usize = 64;
const UNIT_FLAG_PLAYER_CONTROLLED: u32 = 0x0000_0008;
const UNIT_FIELD_HEALTH: usize = 0x016;
const UNIT_FIELD_POWER1: usize = 0x017;
const UNIT_FIELD_POWER2: usize = 0x018;
const UNIT_FIELD_POWER3: usize = 0x019;
const UNIT_FIELD_POWER4: usize = 0x01A;
const UNIT_FIELD_POWER5: usize = 0x01B;
const UNIT_FIELD_MAXHEALTH: usize = 0x01C;
const UNIT_FIELD_MAXPOWER1: usize = 0x01D;
const UNIT_FIELD_MAXPOWER2: usize = 0x01E;
const UNIT_FIELD_MAXPOWER3: usize = 0x01F;
const UNIT_FIELD_MAXPOWER4: usize = 0x020;
const UNIT_FIELD_MAXPOWER5: usize = 0x021;
const UNIT_FIELD_LEVEL: usize = 0x022;
const UNIT_FIELD_FACTIONTEMPLATE: usize = 0x023;
const UNIT_FIELD_BYTES_0: usize = 0x024;
const UNIT_FIELD_FLAGS: usize = 0x02E;
const UNIT_FIELD_BASEATTACKTIME: usize = 0x07E;
const UNIT_FIELD_RANGEDATTACKTIME: usize = 0x080;
const UNIT_FIELD_BOUNDINGRADIUS: usize = 0x081;
const UNIT_FIELD_COMBATREACH: usize = 0x082;
const UNIT_FIELD_DISPLAYID: usize = 0x083;
const UNIT_FIELD_NATIVEDISPLAYID: usize = 0x084;
const UNIT_FIELD_MINDAMAGE: usize = 0x086;
const UNIT_FIELD_MAXDAMAGE: usize = 0x087;
const UNIT_FIELD_BYTES_1: usize = 0x08A;
const UNIT_MOD_CAST_SPEED: usize = 0x091;
const UNIT_FIELD_STAT0: usize = 0x096;
const UNIT_FIELD_BASE_MANA: usize = 0x0A2;
const UNIT_FIELD_BASE_HEALTH: usize = 0x0A3;
const UNIT_FIELD_BYTES_2: usize = 0x0A4;
const UNIT_FIELD_ATTACK_POWER: usize = 0x0A5;
const UNIT_FIELD_ATTACK_POWER_MULTIPLIER: usize = 0x0A7;
const UNIT_FIELD_RANGED_ATTACK_POWER: usize = 0x0A8;
const UNIT_FIELD_RANGED_ATTACK_POWER_MULTIPLIER: usize = 0x0AA;
const UNIT_FIELD_POWER_COST_MULTIPLIER: usize = 0x0B4;
const PLAYER_FLAGS_FIELD: usize = 0x0BE;
const PLAYER_BYTES: usize = 0x0C1;
const PLAYER_BYTES_2: usize = 0x0C2;
const PLAYER_BYTES_3: usize = 0x0C3;
const PLAYER_FIELD_INV_SLOT_HEAD: usize = 0x1E6;
const PLAYER_FIELD_PACK_SLOT_1: usize = 0x214;
const PLAYER_XP: usize = 0x2CC;
const PLAYER_NEXT_LEVEL_XP: usize = 0x2CD;
const PLAYER_FIELD_COINAGE: usize = 0x498;
const PLAYER_FIELD_MOD_DAMAGE_DONE_POS: usize = 0x4B1;
const PLAYER_FIELD_MOD_DAMAGE_DONE_NEG: usize = 0x4B8;
const PLAYER_FIELD_MOD_DAMAGE_DONE_PCT: usize = 0x4BF;
const PLAYER_FIELD_BYTES: usize = 0x4C6;
const INVENTORY_SLOT_BAG_0: u8 = 0;
const INVENTORY_SLOT_ITEM_START: u8 = 23;
const INVENTORY_SLOT_ITEM_END: u8 = 39;

pub struct WorldServer {
    bind_addr: SocketAddr,
    login_db_pool: MySqlPool,
    character_db_pool: MySqlPool,
    world_db_pool: MySqlPool,
    runtime_state: WorldRuntimeState,
}

type OnlineCharacters = Arc<Mutex<HashSet<u32>>>;

#[derive(Clone)]
struct WorldRuntimeState {
    online_characters: OnlineCharacters,
    delete_options: CharacterDeleteOptions,
}

impl WorldServer {
    pub fn new(
        bind_addr: SocketAddr,
        login_db_pool: MySqlPool,
        character_db_pool: MySqlPool,
        world_db_pool: MySqlPool,
        delete_options: CharacterDeleteOptions,
    ) -> Self {
        Self {
            bind_addr,
            login_db_pool,
            character_db_pool,
            world_db_pool,
            runtime_state: WorldRuntimeState {
                online_characters: Arc::new(Mutex::new(HashSet::new())),
                delete_options,
            },
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
                    let world_pool = self.world_db_pool.clone();
                    let runtime_state = self.runtime_state.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_client(
                            socket,
                            login_pool,
                            character_pool,
                            world_pool,
                            runtime_state,
                        )
                        .await
                        {
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
    world_db_pool: MySqlPool,
    runtime_state: WorldRuntimeState,
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
                        send_char_enum(&mut stream, &characters, Some(&mut header_crypto)).await?;
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
                    CMSG_QUERY_TIME => {
                        handle_query_time(&mut stream, &mut header_crypto).await?;
                    }
                    CMSG_REQUEST_ACCOUNT_DATA => {
                        handle_request_account_data(&mut stream, &body, &mut header_crypto).await?;
                    }
                    CMSG_UPDATE_ACCOUNT_DATA => {
                        handle_update_account_data(&body);
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
                            &runtime_state.online_characters,
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
                unregister_active_character(&runtime_state.online_characters, &mut session).await;
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

async fn handle_char_delete(
    stream: &mut TcpStream,
    login_db_pool: &MySqlPool,
    character_db_pool: &MySqlPool,
    account_id: u32,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
    runtime_state: &WorldRuntimeState,
) -> anyhow::Result<()> {
    if body.len() != 8 {
        warn!("Rejected malformed CMSG_CHAR_DELETE bytes={}", body.len());
        return send_char_delete_result(stream, CHAR_DELETE_FAILED, Some(header_crypto)).await;
    }

    let raw_guid = u64::from_le_bytes(body.try_into()?);
    let guid = ObjectGuid::from_raw(raw_guid).counter();
    if runtime_state.online_characters.lock().await.contains(&guid) {
        warn!(account_id, guid, "Rejected loaded character delete");
        return send_char_delete_result(stream, CHAR_DELETE_FAILED, Some(header_crypto)).await;
    }
    if wow_db::is_guild_leader(character_db_pool, guid).await? {
        warn!(account_id, guid, "Rejected guild leader character delete");
        return send_char_delete_result(stream, CHAR_DELETE_FAILED, Some(header_crypto)).await;
    }

    let deleted = wow_db::delete_character_with_options(
        character_db_pool,
        account_id,
        guid,
        runtime_state.delete_options,
    )
    .await?;
    if deleted {
        let count = wow_db::refresh_realm_character_count(
            login_db_pool,
            character_db_pool,
            account_id,
            REALM_ID,
        )
        .await?;
        info!(account_id, guid, count, "Deleted character");
        send_char_delete_result(stream, CHAR_DELETE_SUCCESS, Some(header_crypto)).await
    } else {
        warn!(account_id, guid, "Rejected character delete");
        send_char_delete_result(stream, CHAR_DELETE_FAILED, Some(header_crypto)).await
    }
}

async fn send_char_delete_result(
    stream: &mut TcpStream,
    result: u8,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    send_packet(stream, SMSG_CHAR_DELETE, &[result], header_crypto).await
}

async fn handle_char_create(
    stream: &mut TcpStream,
    login_db_pool: &MySqlPool,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
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
        world_db_pool,
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

    let new_count = wow_db::refresh_realm_character_count(
        login_db_pool,
        character_db_pool,
        account_id,
        REALM_ID,
    )
    .await?;

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
    deps: PlayerLoginDeps<'_>,
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
    let characters = wow_db::get_character_enum_entries(deps.character_db_pool, account_id).await?;
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

    if deps
        .online_characters
        .lock()
        .await
        .contains(&character.guid)
    {
        warn!(
            account_id,
            guid = character.guid,
            "Character login rejected: character already loaded"
        );
        send_packet(
            stream,
            SMSG_CHARACTER_LOGIN_FAILED,
            &[CHAR_LOGIN_NO_CHARACTER],
            Some(header_crypto),
        )
        .await?;
        return Ok(());
    }

    info!(
        account_id,
        guid = character.guid,
        name = %character.name,
        map = character.map,
        "Character login selected"
    );
    unregister_active_character(deps.online_characters, session).await;
    deps.online_characters.lock().await.insert(character.guid);
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
    let inventory =
        wow_db::get_character_inventory_items(deps.character_db_pool, character.guid).await?;
    let world_stats = wow_db::get_player_world_stats(
        deps.world_db_pool,
        character.race,
        character.class,
        character.level,
    )
    .await?;
    let tutorial_flags = wow_db::get_tutorial_flags(deps.character_db_pool, account_id).await?;
    if character.cinematic == 0 || character.at_login & AT_LOGIN_FIRST != 0 {
        let rows = wow_db::mark_character_first_login_seen(
            deps.character_db_pool,
            account_id,
            character.guid,
        )
        .await?;
        if rows == 0 {
            warn!(
                account_id,
                guid = character.guid,
                "No character row updated while marking first-login state seen"
            );
        }
    }

    send_enter_world_bootstrap(
        stream,
        deps.character_db_pool,
        character,
        &inventory,
        &world_stats,
        &tutorial_flags,
        Some(header_crypto),
    )
    .await?;

    Ok(())
}

struct PlayerLoginDeps<'a> {
    character_db_pool: &'a MySqlPool,
    world_db_pool: &'a MySqlPool,
    online_characters: &'a OnlineCharacters,
}

async fn handle_logout_request(
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
    account_id: u32,
    header_crypto: &mut HeaderCrypto,
    session: &mut WorldSessionState,
    online_characters: &OnlineCharacters,
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
    unregister_active_character(online_characters, session).await;
    Ok(())
}

async fn unregister_active_character(
    online_characters: &OnlineCharacters,
    session: &mut WorldSessionState,
) {
    if let Some(character) = session.active_character.take() {
        online_characters.lock().await.remove(&character.guid);
    }
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
    character_db_pool: &MySqlPool,
    character: &CharacterEnumEntry,
    inventory: &[CharacterInventoryItem],
    world_stats: &PlayerWorldStats,
    tutorial_flags: &[u32; 8],
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let mut header_crypto = header_crypto;
    send_login_verify_world(stream, character, header_crypto.as_deref_mut()).await?;
    send_account_data_times(stream, header_crypto.as_deref_mut()).await?;
    send_bindpoint_update(stream, character, header_crypto.as_deref_mut()).await?;
    send_tutorial_flags(stream, tutorial_flags, header_crypto.as_deref_mut()).await?;
    let spells = wow_db::get_character_spells(character_db_pool, character.guid).await?;
    send_initial_spells(stream, &spells, header_crypto.as_deref_mut()).await?;
    let actions = wow_db::get_character_actions(character_db_pool, character.guid).await?;
    send_action_buttons(stream, &actions, header_crypto.as_deref_mut()).await?;
    send_initial_reputations(stream, header_crypto.as_deref_mut()).await?;
    send_login_set_time_speed(stream, header_crypto.as_deref_mut()).await?;
    send_init_world_states(stream, character, header_crypto.as_deref_mut()).await?;
    send_self_spawn_update(stream, character, inventory, world_stats, header_crypto).await?;
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
    tutorial_flags: &[u32; 8],
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_tutorial_flags_body(tutorial_flags);
    send_packet(stream, SMSG_TUTORIAL_FLAGS, &body, header_crypto).await
}

fn build_tutorial_flags_body(tutorial_flags: &[u32; 8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(tutorial_flags.len() * 4);
    for flag in tutorial_flags {
        body.extend_from_slice(&flag.to_le_bytes());
    }
    body
}

async fn handle_tutorial_flag(
    character_db_pool: &MySqlPool,
    account_id: u32,
    body: &[u8],
) -> anyhow::Result<()> {
    if body.len() < 4 {
        warn!(
            account_id,
            bytes = body.len(),
            "Ignoring malformed tutorial flag"
        );
        return Ok(());
    }

    let flag = u32::from_le_bytes(body[0..4].try_into()?);
    let mut tutorials = wow_db::get_tutorial_flags(character_db_pool, account_id).await?;
    if !apply_tutorial_flag(&mut tutorials, flag) {
        warn!(account_id, flag, "Ignoring out-of-range tutorial flag");
        return Ok(());
    }

    wow_db::save_tutorial_flags(character_db_pool, account_id, tutorials).await?;
    Ok(())
}

fn apply_tutorial_flag(tutorials: &mut [u32; 8], flag: u32) -> bool {
    let index = (flag / 32) as usize;
    if index >= tutorials.len() {
        return false;
    }

    tutorials[index] |= 1u32 << (flag % 32);
    true
}

async fn handle_tutorial_clear(
    character_db_pool: &MySqlPool,
    account_id: u32,
) -> anyhow::Result<()> {
    wow_db::save_tutorial_flags(character_db_pool, account_id, [u32::MAX; 8]).await?;
    Ok(())
}

async fn handle_tutorial_reset(
    character_db_pool: &MySqlPool,
    account_id: u32,
) -> anyhow::Result<()> {
    wow_db::save_tutorial_flags(character_db_pool, account_id, [0; 8]).await?;
    Ok(())
}

async fn send_initial_spells(
    stream: &mut TcpStream,
    spells: &[CharacterSpell],
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_initial_spells_body(spells);
    send_packet(stream, SMSG_INITIAL_SPELLS, &body, header_crypto).await
}

fn build_initial_spells_body(spells: &[CharacterSpell]) -> Vec<u8> {
    let active_spells = spells
        .iter()
        .filter(|spell| spell.active != 0 && spell.disabled == 0)
        .count();
    let mut body = Vec::with_capacity(5 + active_spells * 4);
    body.push(0); // unknown flags byte
    body.extend_from_slice(&(active_spells as u16).to_le_bytes());
    for spell in spells
        .iter()
        .filter(|spell| spell.active != 0 && spell.disabled == 0)
    {
        body.extend_from_slice(&(spell.spell as u16).to_le_bytes());
        body.extend_from_slice(&0u16.to_le_bytes()); // CMaNGOS writes zero, not an action slot.
    }
    body.extend_from_slice(&0u16.to_le_bytes()); // cooldown count
    body
}

async fn send_action_buttons(
    stream: &mut TcpStream,
    actions: &[CharacterAction],
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_action_buttons_body(actions);
    send_packet(stream, SMSG_ACTION_BUTTONS, &body, header_crypto).await
}

fn build_action_buttons_body(actions: &[CharacterAction]) -> Vec<u8> {
    let mut buttons = vec![0u32; MAX_ACTION_BUTTONS];
    for action in actions {
        if (action.button as usize) < MAX_ACTION_BUTTONS {
            buttons[action.button as usize] = pack_action_button(action.action, action.action_type);
        }
    }

    let mut body = Vec::with_capacity(MAX_ACTION_BUTTONS * 4);
    for button in buttons {
        body.extend_from_slice(&button.to_le_bytes());
    }
    body
}

fn pack_action_button(action: u32, action_type: u8) -> u32 {
    action | ((action_type as u32) << 24)
}

async fn send_initial_reputations(
    stream: &mut TcpStream,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_initial_reputations_body();
    send_packet(stream, SMSG_INITIALIZE_FACTIONS, &body, header_crypto).await
}

fn build_initial_reputations_body() -> Vec<u8> {
    let mut body = Vec::with_capacity(4 + REPUTATION_LIST_SLOTS * 5);
    body.extend_from_slice(&(REPUTATION_LIST_SLOTS as u32).to_le_bytes());
    for _ in 0..REPUTATION_LIST_SLOTS {
        body.push(0);
        body.extend_from_slice(&0u32.to_le_bytes());
    }
    body
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
    inventory: &[CharacterInventoryItem],
    world_stats: &PlayerWorldStats,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_self_spawn_update_body(character, inventory, world_stats)?;
    info!(
        guid = character.guid,
        name = %character.name,
        bytes = body.len(),
        "Sending minimal self spawn update"
    );
    send_packet(stream, SMSG_UPDATE_OBJECT, &body, header_crypto).await
}

fn build_self_spawn_update_body(
    character: &CharacterEnumEntry,
    inventory: &[CharacterInventoryItem],
    world_stats: &PlayerWorldStats,
) -> anyhow::Result<Vec<u8>> {
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

    write_minimal_player_update_values(&mut block, guid, character, inventory, world_stats)?;

    let item_blocks = build_backpack_item_create_blocks(character, inventory)?;
    let block_count = 1 + item_blocks.len() as u32;
    let mut body = Vec::with_capacity(5 + block.len());
    body.extend_from_slice(&block_count.to_le_bytes());
    body.push(0); // has transport
    body.extend_from_slice(&block);
    for item_block in item_blocks {
        body.extend_from_slice(&item_block);
    }
    Ok(body)
}

fn write_minimal_player_update_values(
    body: &mut Vec<u8>,
    guid: ObjectGuid,
    character: &CharacterEnumEntry,
    inventory: &[CharacterInventoryItem],
    world_stats: &PlayerWorldStats,
) -> anyhow::Result<()> {
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, 0x000, guid.raw() as u32)?;
    set_update_value(&mut values, 0x001, (guid.raw() >> 32) as u32)?;
    set_update_value(&mut values, 0x002, TYPEMASK_OBJECT_UNIT_PLAYER)?;
    set_update_value(&mut values, 0x004, 1.0f32.to_bits())?;
    set_player_vital_update_values(&mut values, character, world_stats)?;
    set_update_value(&mut values, UNIT_FIELD_LEVEL, character.level as u32)?;
    set_update_value(
        &mut values,
        UNIT_FIELD_FACTIONTEMPLATE,
        faction_for_race(character.race),
    )?;
    set_update_value(&mut values, UNIT_FIELD_BYTES_0, unit_bytes_0(character))?;
    set_update_value(&mut values, UNIT_FIELD_FLAGS, UNIT_FLAG_PLAYER_CONTROLLED)?;
    set_update_value(&mut values, UNIT_FIELD_BASEATTACKTIME, 2000)?;
    set_update_value(&mut values, UNIT_FIELD_BASEATTACKTIME + 1, 2000)?;
    set_update_value(&mut values, UNIT_FIELD_RANGEDATTACKTIME, 2000)?;
    set_update_value(&mut values, UNIT_FIELD_BOUNDINGRADIUS, 0.389f32.to_bits())?;
    set_update_value(&mut values, UNIT_FIELD_COMBATREACH, 1.5f32.to_bits())?;
    set_update_value(
        &mut values,
        UNIT_FIELD_DISPLAYID,
        display_id_for_character(character),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_NATIVEDISPLAYID,
        display_id_for_character(character),
    )?;
    set_update_value(&mut values, UNIT_FIELD_MINDAMAGE, 0.0f32.to_bits())?;
    set_update_value(&mut values, UNIT_FIELD_MAXDAMAGE, 0.0f32.to_bits())?;
    set_update_value(&mut values, UNIT_FIELD_BYTES_1, unit_bytes_1(character))?;
    set_update_value(&mut values, UNIT_MOD_CAST_SPEED, 1.0f32.to_bits())?;
    set_player_stat_update_values(&mut values, world_stats)?;
    set_update_value(&mut values, UNIT_FIELD_BYTES_2, unit_bytes_2())?;
    set_update_value(&mut values, UNIT_FIELD_ATTACK_POWER, 0)?;
    set_update_value(
        &mut values,
        UNIT_FIELD_ATTACK_POWER_MULTIPLIER,
        0.0f32.to_bits(),
    )?;
    set_update_value(&mut values, UNIT_FIELD_RANGED_ATTACK_POWER, 0)?;
    set_update_value(
        &mut values,
        UNIT_FIELD_RANGED_ATTACK_POWER_MULTIPLIER,
        0.0f32.to_bits(),
    )?;
    for index in UNIT_FIELD_POWER_COST_MULTIPLIER..UNIT_FIELD_POWER_COST_MULTIPLIER + 7 {
        set_update_value(&mut values, index, 0.0f32.to_bits())?;
    }
    set_update_value(&mut values, PLAYER_FLAGS_FIELD, character.player_flags)?;
    set_update_value(&mut values, PLAYER_BYTES, character.player_bytes)?;
    set_update_value(&mut values, PLAYER_BYTES_2, character.player_bytes2)?;
    set_update_value(&mut values, PLAYER_BYTES_3, 0)?;
    set_visible_item_update_values(&mut values, character)?;
    set_inventory_slot_update_values(&mut values, inventory)?;
    set_update_value(&mut values, PLAYER_XP, 0)?;
    set_update_value(&mut values, PLAYER_NEXT_LEVEL_XP, world_stats.next_level_xp)?;
    set_update_value(&mut values, PLAYER_FIELD_COINAGE, character.money)?;
    set_player_damage_mod_update_values(&mut values)?;
    set_update_value(&mut values, PLAYER_FIELD_BYTES, 0)?;

    write_update_values(body, &values)?;

    Ok(())
}

fn set_player_vital_update_values(
    values: &mut [Option<u32>],
    character: &CharacterEnumEntry,
    world_stats: &PlayerWorldStats,
) -> anyhow::Result<()> {
    let max_health = character.health.max(world_stats.max_health());
    let max_mana = world_stats.max_mana();
    let power1 = if character.power1 > 0 {
        character.power1
    } else {
        max_mana
    };
    let power2 = character
        .power2
        .min(create_power_for_class_power(character.class, POWER_RAGE));
    let power4 = if character.power4 > 0 {
        character.power4
    } else {
        create_power_for_class_power(character.class, POWER_ENERGY)
    };

    set_update_value(values, UNIT_FIELD_HEALTH, max_health)?;
    set_update_value(values, UNIT_FIELD_POWER1, power1)?;
    set_update_value(values, UNIT_FIELD_POWER2, power2)?;
    set_update_value(
        values,
        UNIT_FIELD_POWER3,
        create_power_for_class_power(character.class, POWER_FOCUS),
    )?;
    set_update_value(values, UNIT_FIELD_POWER4, power4)?;
    set_update_value(
        values,
        UNIT_FIELD_POWER5,
        create_power_for_class_power(character.class, POWER_HAPPINESS),
    )?;
    set_update_value(values, UNIT_FIELD_MAXHEALTH, max_health)?;
    set_update_value(values, UNIT_FIELD_MAXPOWER1, max_mana)?;
    set_update_value(
        values,
        UNIT_FIELD_MAXPOWER2,
        create_power_for_class_power(character.class, POWER_RAGE),
    )?;
    set_update_value(
        values,
        UNIT_FIELD_MAXPOWER3,
        create_power_for_class_power(character.class, POWER_FOCUS),
    )?;
    set_update_value(
        values,
        UNIT_FIELD_MAXPOWER4,
        create_power_for_class_power(character.class, POWER_ENERGY),
    )?;
    set_update_value(
        values,
        UNIT_FIELD_MAXPOWER5,
        create_power_for_class_power(character.class, POWER_HAPPINESS),
    )?;
    set_update_value(values, UNIT_FIELD_BASE_MANA, world_stats.base_mana)?;
    set_update_value(values, UNIT_FIELD_BASE_HEALTH, world_stats.base_health)?;

    Ok(())
}

fn set_player_stat_update_values(
    values: &mut [Option<u32>],
    world_stats: &PlayerWorldStats,
) -> anyhow::Result<()> {
    for (offset, stat) in world_stats.stats.into_iter().enumerate() {
        set_update_value(values, UNIT_FIELD_STAT0 + offset, stat)?;
    }

    Ok(())
}

fn set_player_damage_mod_update_values(values: &mut [Option<u32>]) -> anyhow::Result<()> {
    for index in PLAYER_FIELD_MOD_DAMAGE_DONE_POS..PLAYER_FIELD_MOD_DAMAGE_DONE_POS + 7 {
        set_update_value(values, index, 0)?;
    }
    for index in PLAYER_FIELD_MOD_DAMAGE_DONE_NEG..PLAYER_FIELD_MOD_DAMAGE_DONE_NEG + 7 {
        set_update_value(values, index, 0)?;
    }
    for index in PLAYER_FIELD_MOD_DAMAGE_DONE_PCT..PLAYER_FIELD_MOD_DAMAGE_DONE_PCT + 7 {
        set_update_value(values, index, 1.0f32.to_bits())?;
    }

    Ok(())
}

fn write_update_values(body: &mut Vec<u8>, values: &[Option<u32>]) -> anyhow::Result<()> {
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

fn unit_bytes_1(character: &CharacterEnumEntry) -> u32 {
    let pet_loyalty = match character.class {
        1 | 8 => 0xEE, // CMaNGOS initializes this for rage and mana users.
        _ => 0,
    };
    let shapeshift_form = match character.class {
        1 => FORM_BATTLESTANCE,
        _ => 0,
    };

    ((pet_loyalty as u32) << 8) | ((shapeshift_form as u32) << 16)
}

fn unit_bytes_2() -> u32 {
    (0x08 | 0x20) << 8
}

fn create_power_for_class_power(class: u8, power: u8) -> u32 {
    match (class, power) {
        (_, POWER_MANA) => 0,
        (1, POWER_RAGE) => POWER_RAGE_DEFAULT,
        (4, POWER_ENERGY) => POWER_ENERGY_DEFAULT,
        _ => 0,
    }
}

fn set_visible_item_update_values(
    values: &mut [Option<u32>],
    character: &CharacterEnumEntry,
) -> anyhow::Result<()> {
    let equipment = parse_equipment_cache(character.equipment_cache.as_deref());
    for (slot, item_id) in equipment.iter().take(19).enumerate() {
        if *item_id == 0 {
            continue;
        }

        let visible_base = 0x104 + slot * 12;
        set_update_value(values, visible_base, *item_id)?;
    }

    Ok(())
}

fn set_inventory_slot_update_values(
    values: &mut [Option<u32>],
    inventory: &[CharacterInventoryItem],
) -> anyhow::Result<()> {
    for item in inventory {
        if item.bag != INVENTORY_SLOT_BAG_0 as u32 {
            continue;
        }

        let Some(field) = inventory_slot_update_field(item.slot) else {
            continue;
        };
        let guid = ObjectGuid::new(HighGuid::Item, 0, item.item);
        set_update_value(values, field, guid.raw() as u32)?;
        set_update_value(values, field + 1, (guid.raw() >> 32) as u32)?;
    }

    Ok(())
}

fn inventory_slot_update_field(slot: u8) -> Option<usize> {
    match slot {
        0..EQUIPMENT_SLOT_END => Some(PLAYER_FIELD_INV_SLOT_HEAD + slot as usize * 2),
        INVENTORY_SLOT_ITEM_START..=INVENTORY_SLOT_ITEM_END => {
            Some(PLAYER_FIELD_PACK_SLOT_1 + (slot - INVENTORY_SLOT_ITEM_START) as usize * 2)
        }
        _ => None,
    }
}

fn build_backpack_item_create_blocks(
    character: &CharacterEnumEntry,
    inventory: &[CharacterInventoryItem],
) -> anyhow::Result<Vec<Vec<u8>>> {
    let owner_guid = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let mut blocks = Vec::new();

    for item in inventory {
        if item.bag != INVENTORY_SLOT_BAG_0 as u32 {
            continue;
        }

        if !(INVENTORY_SLOT_ITEM_START..=INVENTORY_SLOT_ITEM_END).contains(&item.slot) {
            continue;
        }

        blocks.push(build_item_create_update_block(owner_guid, item)?);
    }

    Ok(blocks)
}

fn build_item_create_update_block(
    owner_guid: ObjectGuid,
    item: &CharacterInventoryItem,
) -> anyhow::Result<Vec<u8>> {
    let item_guid = ObjectGuid::new(HighGuid::Item, 0, item.item);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_CREATE_OBJECT);
    PackedGuid::write(&mut block, item_guid)?;
    block.push(TYPEID_ITEM);
    block.push(UPDATEFLAG_ALL);
    block.extend_from_slice(&1u32.to_le_bytes());

    let mut values = vec![None; ITEM_END_FIELDS];
    set_update_value(&mut values, 0x000, item_guid.raw() as u32)?;
    set_update_value(&mut values, 0x001, (item_guid.raw() >> 32) as u32)?;
    set_update_value(&mut values, 0x002, TYPEMASK_OBJECT_ITEM)?;
    set_update_value(&mut values, 0x003, item.item_template)?;
    set_update_value(&mut values, 0x004, 1.0f32.to_bits())?;
    set_update_value(&mut values, 0x006, owner_guid.raw() as u32)?;
    set_update_value(&mut values, 0x007, (owner_guid.raw() >> 32) as u32)?;
    set_update_value(&mut values, 0x008, owner_guid.raw() as u32)?;
    set_update_value(&mut values, 0x009, (owner_guid.raw() >> 32) as u32)?;
    set_update_value(&mut values, 0x00E, item.count)?;
    set_update_value(&mut values, 0x02E, item.durability)?;
    set_update_value(&mut values, 0x02F, item.durability)?;
    write_update_values(&mut block, &values)?;

    Ok(block)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StarterItemVisual {
    display_id: u32,
    inventory_type: u8,
}

fn parse_equipment_cache(cache: Option<&str>) -> [u32; ENUM_EQUIPMENT_SLOTS] {
    let mut equipment = [0u32; ENUM_EQUIPMENT_SLOTS];
    let Some(cache) = cache else {
        return equipment;
    };

    for (slot, chunk) in cache
        .split_whitespace()
        .filter_map(|value| value.parse::<u32>().ok())
        .collect::<Vec<_>>()
        .chunks(2)
        .take(ENUM_EQUIPMENT_SLOTS)
        .enumerate()
    {
        if let Some(item_id) = chunk.first() {
            equipment[slot] = *item_id;
        }
    }

    equipment
}

fn starter_item_visual(item_id: u32) -> Option<StarterItemVisual> {
    match item_id {
        25 => Some(StarterItemVisual {
            display_id: 1542,
            inventory_type: 21,
        }),
        38 => Some(StarterItemVisual {
            display_id: 9891,
            inventory_type: 4,
        }),
        39 => Some(StarterItemVisual {
            display_id: 9892,
            inventory_type: 7,
        }),
        40 => Some(StarterItemVisual {
            display_id: 10141,
            inventory_type: 8,
        }),
        2362 => Some(StarterItemVisual {
            display_id: 18730,
            inventory_type: 14,
        }),
        _ => None,
    }
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
        (2, 0) => 51,
        (2, 1) => 52,
        (3, 0) => 53,
        (3, 1) => 54,
        (4, 0) => 55,
        (4, 1) => 56,
        (5, 0) => 57,
        (5, 1) => 58,
        (6, 0) => 59,
        (6, 1) => 60,
        (7, 0) => 1563,
        (7, 1) => 1564,
        (8, 0) => 1478,
        (8, 1) => 1479,
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

async fn handle_item_query_single(
    stream: &mut TcpStream,
    world_db_pool: &MySqlPool,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if body.len() < 4 {
        anyhow::bail!(
            "CMSG_ITEM_QUERY_SINGLE payload too short: {} bytes",
            body.len()
        );
    }

    let item = u32::from_le_bytes(body[0..4].try_into()?);
    let template = wow_db::get_item_template_query(world_db_pool, item).await?;
    info!(
        item,
        found = template.is_some(),
        "Answering item template query"
    );
    let response = build_item_query_single_response(item, template.as_ref());
    send_packet(
        stream,
        SMSG_ITEM_QUERY_SINGLE_RESPONSE,
        &response,
        Some(header_crypto),
    )
    .await
}

fn build_item_query_single_response(
    item: u32,
    template: Option<&wow_db::ItemTemplateQuery>,
) -> Vec<u8> {
    let Some(template) = template else {
        return (item | 0x8000_0000).to_le_bytes().to_vec();
    };

    let mut body = Vec::with_capacity(600);
    write_u32(&mut body, template.entry);
    write_u32(&mut body, template.class);
    write_u32(&mut body, item_query_subclass(template));
    write_c_string(&mut body, &template.name);
    body.push(0);
    body.push(0);
    body.push(0);
    write_u32(&mut body, template.displayid);
    write_u32(&mut body, template.quality);
    write_u32(&mut body, template.flags);
    write_u32(&mut body, template.buy_price);
    write_u32(&mut body, template.sell_price);
    write_u32(&mut body, template.inventory_type);
    write_i32(&mut body, template.allowable_class);
    write_i32(&mut body, template.allowable_race);
    write_u32(&mut body, template.item_level);
    write_u32(&mut body, template.required_level);
    write_u32(&mut body, template.required_skill);
    write_u32(&mut body, template.required_skill_rank);
    write_u32(&mut body, template.required_spell);
    write_u32(&mut body, template.required_honor_rank);
    write_u32(&mut body, template.required_city_rank);
    write_u32(&mut body, template.required_reputation_faction);
    write_u32(
        &mut body,
        if template.required_reputation_faction > 0 {
            template.required_reputation_rank
        } else {
            0
        },
    );
    write_u32(&mut body, template.max_count);
    write_u32(&mut body, template.stackable);
    write_u32(&mut body, template.container_slots);

    for _ in 0..10 {
        write_u32(&mut body, 0);
        write_u32(&mut body, 0);
    }
    for _ in 0..5 {
        write_f32(&mut body, 0.0);
        write_f32(&mut body, 0.0);
        write_u32(&mut body, 0);
    }

    write_u32(&mut body, template.armor);
    write_u32(&mut body, template.holy_res);
    write_u32(&mut body, template.fire_res);
    write_u32(&mut body, template.nature_res);
    write_u32(&mut body, template.frost_res);
    write_u32(&mut body, template.shadow_res);
    write_u32(&mut body, template.arcane_res);
    write_u32(&mut body, template.delay);
    write_u32(&mut body, template.ammo_type);
    write_f32(&mut body, template.ranged_mod_range);

    for _ in 0..5 {
        write_u32(&mut body, 0);
        write_u32(&mut body, 0);
        write_u32(&mut body, 0);
        write_u32(&mut body, u32::MAX);
        write_u32(&mut body, 0);
        write_u32(&mut body, u32::MAX);
    }

    write_u32(&mut body, template.bonding);
    write_c_string(&mut body, &template.description);
    write_u32(&mut body, template.page_text);
    write_u32(&mut body, template.language_id);
    write_u32(&mut body, template.page_material);
    write_u32(&mut body, template.start_quest);
    write_u32(&mut body, template.lock_id);
    write_i32(&mut body, template.material);
    write_u32(&mut body, template.sheath);
    write_u32(&mut body, template.random_property);
    write_u32(&mut body, template.block);
    write_u32(&mut body, template.itemset);
    write_u32(&mut body, template.max_durability);
    write_u32(&mut body, template.area);
    write_i32(&mut body, template.map);
    write_i32(&mut body, template.bag_family);
    body
}

fn item_query_subclass(template: &wow_db::ItemTemplateQuery) -> u32 {
    if template.class == 0 {
        0
    } else {
        template.subclass
    }
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

    fn decode_update_values(body: &[u8]) -> Vec<Option<u32>> {
        let block_count = body[0] as usize;
        let mask_start = 1;
        let mut value_cursor = mask_start + block_count * 4;
        let mut values = vec![None; block_count * 32];

        for (index, value_slot) in values.iter_mut().enumerate() {
            let mask_offset = mask_start + (index / 32) * 4;
            let mask = u32::from_le_bytes(
                body[mask_offset..mask_offset + 4]
                    .try_into()
                    .expect("update mask block"),
            );
            if mask & (1 << (index % 32)) == 0 {
                continue;
            }

            let value = u32::from_le_bytes(
                body[value_cursor..value_cursor + 4]
                    .try_into()
                    .expect("update value"),
            );
            *value_slot = Some(value);
            value_cursor += 4;
        }

        values
    }

    fn test_character(race: u8, class: u8) -> CharacterEnumEntry {
        CharacterEnumEntry {
            guid: 7,
            name: "Ada".to_string(),
            race,
            class,
            gender: 0,
            player_bytes: 0x0403_0201,
            player_bytes2: 5,
            level: 1,
            zone: 12,
            map: 0,
            position_x: -8949.95,
            position_y: -132.493,
            position_z: 83.5312,
            orientation: 0.0,
            guildid: None,
            player_flags: 0,
            at_login: 0,
            money: 12345,
            cinematic: 0,
            health: 0,
            power1: 0,
            power2: 0,
            power3: 0,
            power4: 0,
            power5: 0,
            pet_entry: None,
            pet_modelid: None,
            pet_level: None,
            equipment_cache: None,
        }
    }

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
            money: 0,
            cinematic: 0,
            health: 20,
            power1: 0,
            power2: 0,
            power3: 0,
            power4: 0,
            power5: 0,
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
            money: 0,
            cinematic: 0,
            health: 20,
            power1: 0,
            power2: 0,
            power3: 0,
            power4: 0,
            power5: 0,
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
        let body = build_initial_spells_body(&[]);
        assert_eq!(body, [0, 0, 0, 0, 0]);
    }

    #[test]
    fn initial_spells_include_active_enabled_spells() {
        let body = build_initial_spells_body(&[
            CharacterSpell {
                spell: 78,
                active: 1,
                disabled: 0,
            },
            CharacterSpell {
                spell: 81,
                active: 0,
                disabled: 0,
            },
            CharacterSpell {
                spell: 107,
                active: 1,
                disabled: 1,
            },
        ]);

        assert_eq!(body, [0, 1, 0, 78, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn action_buttons_pack_cmangos_action_type_layout() {
        let body = build_action_buttons_body(&[
            CharacterAction {
                button: 0,
                action: 6603,
                action_type: 0,
            },
            CharacterAction {
                button: 11,
                action: 117,
                action_type: 128,
            },
        ]);

        assert_eq!(body.len(), MAX_ACTION_BUTTONS * 4);
        assert_eq!(&body[0..4], &6603u32.to_le_bytes());
        assert_eq!(&body[44..48], &(0x8000_0075u32).to_le_bytes());
    }

    #[test]
    fn warrior_unit_bytes_set_battle_stance_for_stance_action_bar() {
        let character = CharacterEnumEntry {
            guid: 7,
            name: "Ada".to_string(),
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
            orientation: 0.0,
            guildid: None,
            player_flags: 0,
            at_login: 0,
            money: 0,
            cinematic: 0,
            health: 20,
            power1: 0,
            power2: 0,
            power3: 0,
            power4: 0,
            power5: 0,
            pet_entry: None,
            pet_modelid: None,
            pet_level: None,
            equipment_cache: None,
        };

        assert_eq!(unit_bytes_1(&character), 0x0011_EE00);
    }

    #[test]
    fn self_spawn_update_includes_cmangos_player_vitals_and_defaults() {
        let guid = ObjectGuid::new(HighGuid::Player, 0, 7);
        let character = test_character(1, 1);

        let mut body = Vec::new();
        let world_stats = PlayerWorldStats {
            base_health: 20,
            base_mana: 0,
            stats: [23, 20, 22, 20, 21],
            next_level_xp: 400,
        };

        write_minimal_player_update_values(&mut body, guid, &character, &[], &world_stats).unwrap();
        let values = decode_update_values(&body);

        assert_eq!(values[UNIT_FIELD_HEALTH], Some(60));
        assert_eq!(values[UNIT_FIELD_MAXHEALTH], Some(60));
        assert_eq!(values[UNIT_FIELD_MAXPOWER2], Some(1000));
        assert_eq!(values[UNIT_FIELD_LEVEL], Some(1));
        assert_eq!(values[UNIT_FIELD_FLAGS], Some(UNIT_FLAG_PLAYER_CONTROLLED));
        assert_eq!(values[UNIT_FIELD_BASEATTACKTIME], Some(2000));
        assert_eq!(values[UNIT_FIELD_BASEATTACKTIME + 1], Some(2000));
        assert_eq!(values[UNIT_FIELD_STAT0], Some(23));
        assert_eq!(values[UNIT_FIELD_STAT0 + 1], Some(20));
        assert_eq!(values[UNIT_FIELD_STAT0 + 2], Some(22));
        assert_eq!(values[UNIT_FIELD_BASE_HEALTH], Some(20));
        assert_eq!(values[UNIT_FIELD_BASE_MANA], Some(0));
        assert_eq!(values[PLAYER_NEXT_LEVEL_XP], Some(400));
        assert_eq!(values[UNIT_FIELD_BYTES_2], Some(unit_bytes_2()));
        assert_eq!(values[PLAYER_FIELD_COINAGE], Some(12345));
        assert_eq!(
            values[PLAYER_FIELD_MOD_DAMAGE_DONE_PCT],
            Some(1.0f32.to_bits())
        );
    }

    #[test]
    fn class_power_defaults_match_cmangos_create_powers() {
        let guid = ObjectGuid::new(HighGuid::Player, 0, 7);

        let mut body = Vec::new();
        let mage_stats = PlayerWorldStats {
            base_health: 31,
            base_mana: 100,
            stats: [15, 23, 19, 26, 22],
            next_level_xp: 400,
        };
        write_minimal_player_update_values(
            &mut body,
            guid,
            &test_character(7, 8),
            &[],
            &mage_stats,
        )
        .unwrap();
        let values = decode_update_values(&body);
        assert_eq!(values[UNIT_FIELD_POWER1], Some(210));
        assert_eq!(values[UNIT_FIELD_MAXPOWER1], Some(210));
        assert_eq!(values[UNIT_FIELD_MAXPOWER2], Some(0));
        assert_eq!(values[UNIT_FIELD_MAXPOWER4], Some(0));

        let mut body = Vec::new();
        let rogue_stats = PlayerWorldStats {
            base_health: 25,
            base_mana: 0,
            stats: [21, 23, 21, 20, 20],
            next_level_xp: 400,
        };
        write_minimal_player_update_values(
            &mut body,
            guid,
            &test_character(1, 4),
            &[],
            &rogue_stats,
        )
        .unwrap();
        let values = decode_update_values(&body);
        assert_eq!(values[UNIT_FIELD_POWER4], Some(POWER_ENERGY_DEFAULT));
        assert_eq!(values[UNIT_FIELD_MAXPOWER4], Some(POWER_ENERGY_DEFAULT));
        assert_eq!(values[UNIT_FIELD_MAXPOWER2], Some(0));
    }

    #[test]
    fn initial_reputations_packet_matches_cmangos_empty_shape() {
        let body = build_initial_reputations_body();

        assert_eq!(body.len(), 4 + REPUTATION_LIST_SLOTS * 5);
        assert_eq!(&body[0..4], &(REPUTATION_LIST_SLOTS as u32).to_le_bytes());
        assert!(body[4..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn tutorial_flags_packet_serializes_account_state() {
        let body =
            build_tutorial_flags_body(&[1, 0x8000_0000, 0x0102_0304, 0, 0, 0, 0, 0xFFFF_FFFF]);

        assert_eq!(body.len(), 8 * 4);
        assert_eq!(&body[0..4], &1u32.to_le_bytes());
        assert_eq!(&body[4..8], &0x8000_0000u32.to_le_bytes());
        assert_eq!(&body[8..12], &0x0102_0304u32.to_le_bytes());
        assert_eq!(&body[28..32], &0xFFFF_FFFFu32.to_le_bytes());
    }

    #[test]
    fn tutorial_flag_updates_match_cmangos_word_bits() {
        let mut tutorials = [0u32; 8];

        assert!(apply_tutorial_flag(&mut tutorials, 0));
        assert!(apply_tutorial_flag(&mut tutorials, 33));
        assert!(apply_tutorial_flag(&mut tutorials, 255));
        assert!(!apply_tutorial_flag(&mut tutorials, 256));

        assert_eq!(tutorials[0], 1);
        assert_eq!(tutorials[1], 2);
        assert_eq!(tutorials[7], 0x8000_0000);
    }

    #[test]
    fn parses_equipment_cache_item_ids() {
        let equipment = parse_equipment_cache(Some("0 0 0 0 0 0 38 0 0 0 0 0 39 0"));

        assert_eq!(equipment[3], 38);
        assert_eq!(equipment[6], 39);
    }

    #[test]
    fn maps_inventory_slots_to_player_update_guid_fields() {
        assert_eq!(
            inventory_slot_update_field(3),
            Some(PLAYER_FIELD_INV_SLOT_HEAD + 6)
        );
        assert_eq!(
            inventory_slot_update_field(23),
            Some(PLAYER_FIELD_PACK_SLOT_1)
        );
        assert_eq!(inventory_slot_update_field(40), None);
    }

    #[test]
    fn writes_inventory_item_guid_update_values() {
        let mut values = vec![None; PLAYER_END_FIELDS];
        let item = CharacterInventoryItem {
            bag: 0,
            slot: 15,
            item: 42,
            item_template: 25,
            count: 1,
            durability: 10,
        };

        set_inventory_slot_update_values(&mut values, &[item]).unwrap();

        let guid = ObjectGuid::new(HighGuid::Item, 0, 42);
        let field = PLAYER_FIELD_INV_SLOT_HEAD + 15 * 2;
        assert_eq!(values[field], Some(guid.raw() as u32));
        assert_eq!(values[field + 1], Some((guid.raw() >> 32) as u32));
    }

    #[test]
    fn builds_create_blocks_for_backpack_items_only() {
        let character = CharacterEnumEntry {
            guid: 11,
            name: "Tester".to_string(),
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
            orientation: 0.0,
            guildid: None,
            player_flags: 0,
            at_login: 0,
            money: 0,
            cinematic: 0,
            health: 20,
            power1: 0,
            power2: 0,
            power3: 0,
            power4: 0,
            power5: 0,
            pet_entry: None,
            pet_modelid: None,
            pet_level: None,
            equipment_cache: None,
        };
        let items = [
            CharacterInventoryItem {
                bag: 0,
                slot: 16,
                item: 40,
                item_template: 2362,
                count: 1,
                durability: 18,
            },
            CharacterInventoryItem {
                bag: 0,
                slot: 24,
                item: 41,
                item_template: 6948,
                count: 1,
                durability: 0,
            },
        ];

        let blocks = build_backpack_item_create_blocks(&character, &items).unwrap();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0][0], UPDATE_TYPE_CREATE_OBJECT);
        assert_eq!(blocks[0][4], TYPEID_ITEM);
        assert_eq!(blocks[0][5], UPDATEFLAG_ALL);
    }

    #[test]
    fn starter_item_visuals_cover_human_warrior_equipment() {
        assert_eq!(
            starter_item_visual(25),
            Some(StarterItemVisual {
                display_id: 1542,
                inventory_type: 21
            })
        );
        assert_eq!(
            starter_item_visual(2362),
            Some(StarterItemVisual {
                display_id: 18730,
                inventory_type: 14
            })
        );
    }

    #[test]
    fn maps_classic_race_gender_display_ids() {
        let mut character = CharacterEnumEntry {
            guid: 7,
            name: "Ada".to_string(),
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
            orientation: 0.0,
            guildid: None,
            player_flags: 0,
            at_login: 0,
            money: 0,
            cinematic: 0,
            health: 20,
            power1: 0,
            power2: 0,
            power3: 0,
            power4: 0,
            power5: 0,
            pet_entry: None,
            pet_modelid: None,
            pet_level: None,
            equipment_cache: None,
        };

        for (race, male_display, female_display) in [
            (1, 49, 50),
            (2, 51, 52),
            (3, 53, 54),
            (4, 55, 56),
            (5, 57, 58),
            (6, 59, 60),
            (7, 1563, 1564),
            (8, 1478, 1479),
        ] {
            character.race = race;
            character.gender = 0;
            assert_eq!(display_id_for_character(&character), male_display);
            character.gender = 1;
            assert_eq!(display_id_for_character(&character), female_display);
        }
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

        for opcode in [CMSG_TUTORIAL_FLAG, CMSG_TUTORIAL_CLEAR, CMSG_TUTORIAL_RESET] {
            assert!(
                !is_expected_noop_opcode(opcode),
                "tutorial opcode 0x{opcode:04X} should be handled, not ignored"
            );
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
