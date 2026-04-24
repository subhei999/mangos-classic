use sha1::{Digest, Sha1};
use sqlx::mysql::MySqlPool;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info, warn};
use wow_common::guid::{write_guid, HighGuid, ObjectGuid, PackedGuid};
use wow_crypto::HeaderCrypto;
use wow_db::CharacterEnumEntry;

const CMSG_CHAR_ENUM: u32 = 0x0037;
const CMSG_PLAYER_LOGIN: u32 = 0x003D;
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

    loop {
        match read_client_packet(&mut stream, Some(&mut header_crypto)).await {
            Ok((opcode, body)) => {
                info!(
                    opcode = format_args!("0x{opcode:04X}"),
                    bytes = body.len(),
                    "Received world packet after auth"
                );

                match opcode {
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
                        )
                        .await?;
                    }
                    CMSG_PING => {
                        handle_ping(&mut stream, &body, Some(&mut header_crypto)).await?;
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
                info!("World client disconnected or read failed: {}", e);
                return Ok(());
            }
        }
    }
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

async fn handle_player_login(
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
    account_id: u32,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
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
    send_enter_world_bootstrap(stream, character, Some(header_crypto)).await
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
