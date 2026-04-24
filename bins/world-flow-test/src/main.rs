use anyhow::{ensure, Context};
use bytes::BytesMut;
use sha1::{Digest, Sha1};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;
use wow_common::guid::{HighGuid, ObjectGuid};
use wow_crypto::HeaderCrypto;
use wow_proto::{
    AuthCommand, LogonChallengeRequest, LogonChallengeResponse, LogonProofRequest,
    LogonProofResponse,
};
use wow_srp::client::SrpClientChallenge;
use wow_srp::normalized_string::NormalizedString;
use wow_srp::server::SrpVerifier;
use wow_srp::PublicKey;

const LOGIN_DATABASE_URL: &str = "mysql://mangos:mangos@127.0.0.1:3307/realmd";
const CHARACTER_DATABASE_URL: &str = "mysql://mangos:mangos@127.0.0.1:3307/characters";
const WORLD_DATABASE_URL: &str = "mysql://mangos:mangos@127.0.0.1:3307/mangos";
const AUTH_ADDR: &str = "127.0.0.1:13724";
const WORLD_ADDR: &str = "127.0.0.1:18085";
const USERNAME: &str = "WORLDLIFE";
const OTHER_USERNAME: &str = "WORLDOTHER";
const PASSWORD: &str = "WORLDPASS";
const CHARACTER_NAME: &str = "Worldlife";
const OTHER_CHARACTER_NAME: &str = "Worldother";
const BUILD_1121: u16 = 5875;
const CLIENT_SEED: u32 = 0x1234_5678;
const REALM_ID: u32 = 1;

const CMSG_CHAR_CREATE: u32 = 0x0036;
const CMSG_CHAR_ENUM: u32 = 0x0037;
const CMSG_CHAR_DELETE: u32 = 0x0038;
const CMSG_AUTH_SESSION: u32 = 0x01ED;
const SMSG_CHAR_CREATE: u32 = 0x003A;
const SMSG_CHAR_ENUM: u32 = 0x003B;
const SMSG_CHAR_DELETE: u32 = 0x003C;
const SMSG_AUTH_CHALLENGE: u32 = 0x01EC;
const SMSG_AUTH_RESPONSE: u32 = 0x01EE;
const AUTH_OK: u8 = 0x0C;
const CHAR_CREATE_SUCCESS: u8 = 0x2E;
const CHAR_CREATE_FAILED: u8 = 0x30;
const CHAR_CREATE_NAME_IN_USE: u8 = 0x31;
const CHAR_CREATE_SERVER_LIMIT: u8 = 0x34;
const CHAR_DELETE_SUCCESS: u8 = 0x39;
const CHAR_DELETE_FAILED: u8 = 0x3A;
const CHAR_NAME_TOO_SHORT: u8 = 0x44;
const CHAR_NAME_INVALID_CHARACTER: u8 = 0x46;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let login_pool = connect(LOGIN_DATABASE_URL).await?;
    let character_pool = connect(CHARACTER_DATABASE_URL).await?;
    let world_pool = connect(WORLD_DATABASE_URL).await?;

    let account_id = seed_account(&login_pool, USERNAME, PASSWORD).await?;
    let other_account_id = seed_account(&login_pool, OTHER_USERNAME, PASSWORD).await?;
    cleanup_account(&login_pool, &character_pool, account_id).await?;
    cleanup_account(&login_pool, &character_pool, other_account_id).await?;

    complete_auth_flow()?;
    let session_key = fetch_session_key(&login_pool).await?;

    let mut world = WorldClient::connect(&session_key)?;
    let initial = world.char_enum()?;
    ensure!(
        !initial
            .iter()
            .any(|character| character.name == CHARACTER_NAME),
        "test character was visible before create"
    );

    world.expect_create_result("A", human_warrior_attributes(), CHAR_NAME_TOO_SHORT)?;
    assert_count_row(&login_pool, account_id, 0).await?;
    world.expect_create_result(
        "Bad1",
        human_warrior_attributes(),
        CHAR_NAME_INVALID_CHARACTER,
    )?;
    assert_count_row(&login_pool, account_id, 0).await?;
    world.expect_create_result("Badcombo", [1, 7, 0, 0, 0, 0, 0, 0, 0], CHAR_CREATE_FAILED)?;
    assert_count_row(&login_pool, account_id, 0).await?;
    world.expect_delete_body_result(&[1, 2, 3], CHAR_DELETE_FAILED)?;
    assert_count_row(&login_pool, account_id, 0).await?;

    world.expect_create_result(
        CHARACTER_NAME,
        human_warrior_attributes(),
        CHAR_CREATE_SUCCESS,
    )?;
    let after_create = world.char_enum()?;
    let created = after_create
        .iter()
        .find(|character| character.name == CHARACTER_NAME)
        .context("created character was missing from SMSG_CHAR_ENUM")?;
    ensure!(
        wow_db::character_count_for_account(&character_pool, account_id).await? == 1,
        "character DB count did not refresh after packet create"
    );
    assert_count_row(&login_pool, account_id, 1).await?;
    ensure!(
        wow_db::get_character_inventory_items(&character_pool, created.guid)
            .await?
            .iter()
            .any(|item| item.item_template == 6948),
        "packet-created character did not receive starter inventory"
    );

    world.expect_create_result(
        CHARACTER_NAME,
        human_warrior_attributes(),
        CHAR_CREATE_NAME_IN_USE,
    )?;
    assert_count_row(&login_pool, account_id, 1).await?;

    let other_character = wow_db::create_character(
        &character_pool,
        &world_pool,
        wow_db::NewCharacter {
            account_id: other_account_id,
            name: OTHER_CHARACTER_NAME.to_string(),
            race: 1,
            class: 1,
            gender: 0,
            skin: 0,
            face: 0,
            hair_style: 0,
            hair_color: 0,
            facial_hair: 0,
        },
    )
    .await?;
    wow_db::refresh_realm_character_count(&login_pool, &character_pool, other_account_id, REALM_ID)
        .await?;
    world.expect_delete_character_result(other_character.guid, CHAR_DELETE_FAILED)?;
    ensure!(
        wow_db::get_character_enum_entries(&character_pool, other_account_id)
            .await?
            .iter()
            .any(|character| character.guid == other_character.guid),
        "other account character was deleted by packet delete"
    );
    assert_count_row(&login_pool, account_id, 1).await?;

    seed_limit_characters(&character_pool, &world_pool, account_id).await?;
    wow_db::refresh_realm_character_count(&login_pool, &character_pool, account_id, REALM_ID)
        .await?;
    assert_count_row(&login_pool, account_id, 10).await?;
    world.expect_create_result(
        "Limitfull",
        human_warrior_attributes(),
        CHAR_CREATE_SERVER_LIMIT,
    )?;
    assert_count_row(&login_pool, account_id, 10).await?;

    clear_guild_fixture(&character_pool).await?;
    seed_guild_leader_fixture(&character_pool, created.guid).await?;
    world.expect_delete_character_result(created.guid, CHAR_DELETE_FAILED)?;
    ensure!(
        world
            .char_enum()?
            .iter()
            .any(|character| character.guid == created.guid),
        "guild leader disappeared after rejected delete"
    );
    ensure!(
        wow_db::character_count_for_account(&character_pool, account_id).await? == 10,
        "guild leader delete rejection changed character count"
    );
    assert_count_row(&login_pool, account_id, 10).await?;
    clear_guild_fixture(&character_pool).await?;

    seed_guild_member_fixture(&character_pool, created.guid).await?;
    world.expect_delete_character_result(created.guid, CHAR_DELETE_SUCCESS)?;
    let after_delete = world.char_enum()?;
    ensure!(
        !after_delete
            .iter()
            .any(|character| character.name == CHARACTER_NAME),
        "deleted character was still present in SMSG_CHAR_ENUM"
    );
    ensure!(
        wow_db::character_count_for_account(&character_pool, account_id).await? == 9,
        "character DB count did not refresh after packet delete"
    );
    assert_count_row(&login_pool, account_id, 9).await?;

    let leaked: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM characters WHERE name = ?")
        .bind(CHARACTER_NAME)
        .fetch_one(&character_pool)
        .await?;
    ensure!(leaked == 0, "deleted packet-flow character row remained");
    assert_guild_member_cleanup(&character_pool, created.guid).await?;

    cleanup_account(&login_pool, &character_pool, account_id).await?;
    cleanup_account(&login_pool, &character_pool, other_account_id).await?;

    drop(world_pool);
    println!(
        "world flow check passed: auth session, create/delete happy path, negative create/delete cases, guild leader rejection, guild cleanup, enum/count refresh"
    );
    Ok(())
}

async fn connect(url: &str) -> anyhow::Result<MySqlPool> {
    Ok(MySqlPoolOptions::new()
        .max_connections(2)
        .connect(url)
        .await?)
}

async fn seed_account(
    login_pool: &MySqlPool,
    username: &str,
    password: &str,
) -> anyhow::Result<u32> {
    let verifier = SrpVerifier::from_username_and_password(
        NormalizedString::new(username)?,
        NormalizedString::new(password)?,
    );

    sqlx::query(
        "INSERT INTO account (username, gmlevel, sessionkey, v, s, email, locked, expansion, locale, os) \
         VALUES (?, 0, '', ?, ?, '', 0, 0, '', 'Win') \
         ON DUPLICATE KEY UPDATE sessionkey = '', v = VALUES(v), s = VALUES(s), locked = 0, os = 'Win'",
    )
    .bind(username)
    .bind(bytes_to_hex(verifier.password_verifier()))
    .bind(bytes_to_hex(verifier.salt()))
    .execute(login_pool)
    .await
    .context("seed world-flow account")?;

    let account_id = sqlx::query_scalar("SELECT id FROM account WHERE username = ?")
        .bind(username)
        .fetch_one(login_pool)
        .await?;
    Ok(account_id)
}

async fn cleanup_account(
    login_pool: &MySqlPool,
    character_pool: &MySqlPool,
    account_id: u32,
) -> anyhow::Result<()> {
    let characters = wow_db::get_character_enum_entries(character_pool, account_id).await?;
    for character in characters {
        wow_db::delete_character(character_pool, account_id, character.guid).await?;
    }
    wow_db::refresh_realm_character_count(login_pool, character_pool, account_id, REALM_ID).await?;
    Ok(())
}

async fn seed_limit_characters(
    character_pool: &MySqlPool,
    world_pool: &MySqlPool,
    account_id: u32,
) -> anyhow::Result<()> {
    for name in [
        "Limita", "Limitb", "Limitc", "Limitd", "Limite", "Limitf", "Limitg", "Limith", "Limiti",
    ] {
        wow_db::create_character(
            character_pool,
            world_pool,
            wow_db::NewCharacter {
                account_id,
                name: name.to_string(),
                race: 1,
                class: 1,
                gender: 0,
                skin: 0,
                face: 0,
                hair_style: 0,
                hair_color: 0,
                facial_hair: 0,
            },
        )
        .await?;
    }
    Ok(())
}

async fn clear_guild_fixture(character_pool: &MySqlPool) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM guild_member WHERE guildid = ?")
        .bind(90_001u32)
        .execute(character_pool)
        .await?;
    sqlx::query("DELETE FROM guild WHERE guildid = ?")
        .bind(90_001u32)
        .execute(character_pool)
        .await?;
    Ok(())
}

async fn seed_guild_leader_fixture(
    character_pool: &MySqlPool,
    leader_guid: u32,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO guild \
         (guildid, name, leaderguid, EmblemStyle, EmblemColor, BorderStyle, BorderColor, \
          BackgroundColor, info, motd, createdate) \
         VALUES (?, 'World Flow Guild', ?, 0, 0, 0, 0, 0, '', '', UNIX_TIMESTAMP())",
    )
    .bind(90_001u32)
    .bind(leader_guid)
    .execute(character_pool)
    .await?;
    sqlx::query(
        "INSERT INTO guild_member (guildid, guid, rank, pnote, offnote) VALUES (?, ?, 0, '', '')",
    )
    .bind(90_001u32)
    .bind(leader_guid)
    .execute(character_pool)
    .await?;
    Ok(())
}

async fn seed_guild_member_fixture(
    character_pool: &MySqlPool,
    member_guid: u32,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO guild \
         (guildid, name, leaderguid, EmblemStyle, EmblemColor, BorderStyle, BorderColor, \
          BackgroundColor, info, motd, createdate) \
         VALUES (?, 'World Flow Guild', 999999, 0, 0, 0, 0, 0, '', '', UNIX_TIMESTAMP())",
    )
    .bind(90_001u32)
    .execute(character_pool)
    .await?;
    sqlx::query(
        "INSERT INTO guild_member (guildid, guid, rank, pnote, offnote) VALUES (?, ?, 1, '', '')",
    )
    .bind(90_001u32)
    .bind(member_guid)
    .execute(character_pool)
    .await?;
    sqlx::query(
        "INSERT INTO guild_eventlog \
         (guildid, LogGuid, EventType, PlayerGuid1, PlayerGuid2, NewRank, TimeStamp) \
         VALUES (?, 1, 1, ?, 0, 1, UNIX_TIMESTAMP()), \
                (?, 2, 2, 0, ?, 1, UNIX_TIMESTAMP())",
    )
    .bind(90_001u32)
    .bind(member_guid)
    .bind(90_001u32)
    .bind(member_guid)
    .execute(character_pool)
    .await?;
    Ok(())
}

async fn assert_guild_member_cleanup(character_pool: &MySqlPool, guid: u32) -> anyhow::Result<()> {
    let member_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM guild_member WHERE guid = ?")
        .bind(guid)
        .fetch_one(character_pool)
        .await?;
    let event_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM guild_eventlog WHERE PlayerGuid1 = ? OR PlayerGuid2 = ?",
    )
    .bind(guid)
    .bind(guid)
    .fetch_one(character_pool)
    .await?;

    ensure!(member_rows == 0, "guild_member row remained after delete");
    ensure!(event_rows == 0, "guild_eventlog rows remained after delete");
    Ok(())
}

fn complete_auth_flow() -> anyhow::Result<()> {
    let mut stream = connect_blocking(AUTH_ADDR)?;
    let (challenge, client) = perform_challenge(&mut stream)?;
    ensure!(
        challenge.error == 0,
        "auth challenge failed with {}",
        challenge.error
    );

    let proof = send_proof(&mut stream, &client)?;
    ensure!(proof.cmd == AuthCommand::LogonProof);
    ensure!(proof.error == 0, "auth proof failed with {}", proof.error);
    client.verify_server_proof(proof.m2)?;
    Ok(())
}

fn perform_challenge(
    stream: &mut TcpStream,
) -> anyhow::Result<(LogonChallengeResponse, SrpClientChallenge)> {
    stream.write_all(&logon_challenge_request())?;

    let challenge_bytes = read_exact_vec(stream, LogonChallengeResponse::SIZE)?;
    let challenge = LogonChallengeResponse::read(&mut &challenge_bytes[..])?;
    ensure!(challenge.cmd == AuthCommand::LogonChallenge);
    ensure!(challenge.g_len == 1, "unexpected generator length");
    ensure!(challenge.n_len == 32, "unexpected safe-prime length");

    let client = SrpClientChallenge::new(
        NormalizedString::new(USERNAME)?,
        NormalizedString::new(PASSWORD)?,
        challenge.g,
        challenge.n,
        PublicKey::from_le_bytes(challenge.server_public)?,
        challenge.salt,
    );

    Ok((challenge, client))
}

fn send_proof(
    stream: &mut TcpStream,
    client: &SrpClientChallenge,
) -> anyhow::Result<LogonProofResponse> {
    let proof_request = LogonProofRequest {
        cmd: AuthCommand::LogonProof,
        client_public: *client.client_public_key(),
        m1: *client.client_proof(),
        crc_hash: [0; 20],
        num_keys: 0,
        security_flags: 0,
    };
    let mut proof_bytes = BytesMut::new();
    proof_request.write(&mut proof_bytes);
    stream.write_all(&proof_bytes)?;

    let response = read_exact_vec(stream, LogonProofResponse::SIZE)?;
    Ok(LogonProofResponse::read(&mut &response[..])?)
}

async fn fetch_session_key(login_pool: &MySqlPool) -> anyhow::Result<[u8; 40]> {
    let session_key: String =
        sqlx::query_scalar("SELECT sessionkey FROM account WHERE username = ?")
            .bind(USERNAME)
            .fetch_one(login_pool)
            .await?;
    hex_to_array40(&session_key)
}

struct WorldClient {
    stream: TcpStream,
    crypto: HeaderCrypto,
}

impl WorldClient {
    fn connect(session_key: &[u8; 40]) -> anyhow::Result<Self> {
        let mut stream = connect_blocking(WORLD_ADDR)?;
        let (opcode, body) = read_server_packet(&mut stream, None)?;
        ensure!(opcode == SMSG_AUTH_CHALLENGE, "expected auth challenge");
        ensure!(body.len() == 4, "world auth challenge body was malformed");
        let server_seed = u32::from_le_bytes(body.as_slice().try_into()?);

        let auth_body = auth_session_body(session_key, server_seed);
        write_client_packet(&mut stream, CMSG_AUTH_SESSION, &auth_body, None)?;

        let mut crypto = HeaderCrypto::new(session_key);
        let (opcode, body) = read_server_packet(&mut stream, Some(&mut crypto))?;
        ensure!(opcode == SMSG_AUTH_RESPONSE, "expected SMSG_AUTH_RESPONSE");
        ensure!(
            body.first() == Some(&AUTH_OK),
            "world auth failed with body {:02X?}",
            body
        );

        Ok(Self { stream, crypto })
    }

    fn char_enum(&mut self) -> anyhow::Result<Vec<EnumCharacter>> {
        write_client_packet(
            &mut self.stream,
            CMSG_CHAR_ENUM,
            &[],
            Some(&mut self.crypto),
        )?;
        let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(opcode == SMSG_CHAR_ENUM, "expected SMSG_CHAR_ENUM");
        parse_char_enum(&body)
    }

    fn expect_create_result(
        &mut self,
        name: &str,
        attributes: [u8; 9],
        expected: u8,
    ) -> anyhow::Result<()> {
        let mut body = Vec::new();
        body.extend_from_slice(name.as_bytes());
        body.push(0);
        body.extend_from_slice(&attributes);
        write_client_packet(
            &mut self.stream,
            CMSG_CHAR_CREATE,
            &body,
            Some(&mut self.crypto),
        )?;
        let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(opcode == SMSG_CHAR_CREATE, "expected SMSG_CHAR_CREATE");
        ensure!(
            body == [expected],
            "character create returned {:02X?}, expected 0x{expected:02X}",
            body
        );
        Ok(())
    }

    fn expect_delete_character_result(&mut self, guid: u32, expected: u8) -> anyhow::Result<()> {
        let guid = ObjectGuid::new(HighGuid::Player, 0, guid);
        self.expect_delete_body_result(&guid.raw().to_le_bytes(), expected)
    }

    fn expect_delete_body_result(&mut self, body: &[u8], expected: u8) -> anyhow::Result<()> {
        write_client_packet(
            &mut self.stream,
            CMSG_CHAR_DELETE,
            body,
            Some(&mut self.crypto),
        )?;
        let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(opcode == SMSG_CHAR_DELETE, "expected SMSG_CHAR_DELETE");
        ensure!(
            body == [expected],
            "character delete returned {:02X?}, expected 0x{expected:02X}",
            body
        );
        Ok(())
    }
}

fn human_warrior_attributes() -> [u8; 9] {
    [1, 1, 0, 0, 0, 0, 0, 0, 0]
}

#[derive(Debug)]
struct EnumCharacter {
    guid: u32,
    name: String,
}

fn parse_char_enum(body: &[u8]) -> anyhow::Result<Vec<EnumCharacter>> {
    ensure!(!body.is_empty(), "empty SMSG_CHAR_ENUM body");
    let count = body[0] as usize;
    let mut cursor = 1;
    let mut characters = Vec::with_capacity(count);

    for _ in 0..count {
        ensure_available(body, cursor + 8)?;
        let raw_guid = u64::from_le_bytes(body[cursor..cursor + 8].try_into()?);
        cursor += 8;

        let name_end = body[cursor..]
            .iter()
            .position(|b| *b == 0)
            .ok_or_else(|| anyhow::anyhow!("character enum name is not NUL-terminated"))?
            + cursor;
        let name = String::from_utf8(body[cursor..name_end].to_vec())?;
        cursor = name_end + 1;

        ensure_available(
            body,
            cursor + 3 + 5 + 1 + 4 + 4 + 12 + 4 + 4 + 1 + 12 + 20 * 5,
        )?;
        cursor += 3; // race, class, gender
        cursor += 5; // appearance bytes
        cursor += 1; // level
        cursor += 4; // zone
        cursor += 4; // map
        cursor += 12; // position
        cursor += 4; // guild
        cursor += 4; // flags
        cursor += 1; // first login
        cursor += 12; // pet display, level, family
        cursor += 20 * 5; // equipment display id + inventory type

        characters.push(EnumCharacter {
            guid: ObjectGuid::from_raw(raw_guid).counter(),
            name,
        });
    }

    ensure!(
        cursor == body.len(),
        "SMSG_CHAR_ENUM had trailing bytes: parsed {cursor}, len {}",
        body.len()
    );
    Ok(characters)
}

fn auth_session_body(session_key: &[u8; 40], server_seed: u32) -> Vec<u8> {
    let mut hasher = Sha1::new();
    hasher.update(USERNAME.as_bytes());
    hasher.update(0u32.to_le_bytes());
    hasher.update(CLIENT_SEED.to_le_bytes());
    hasher.update(server_seed.to_le_bytes());
    hasher.update(session_key);
    let digest: [u8; 20] = hasher.finalize().into();

    let mut body = Vec::new();
    body.extend_from_slice(&(BUILD_1121 as u32).to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes());
    body.extend_from_slice(USERNAME.as_bytes());
    body.push(0);
    body.extend_from_slice(&CLIENT_SEED.to_le_bytes());
    body.extend_from_slice(&digest);
    body
}

fn write_client_packet(
    stream: &mut TcpStream,
    opcode: u32,
    body: &[u8],
    crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let size = (body.len() + 4) as u16;
    let mut header = [0u8; 6];
    header[0..2].copy_from_slice(&size.to_be_bytes());
    header[2..6].copy_from_slice(&opcode.to_le_bytes());
    if let Some(crypto) = crypto {
        crypto.encrypt(&mut header);
    }
    stream.write_all(&header)?;
    stream.write_all(body)?;
    Ok(())
}

fn read_server_packet(
    stream: &mut TcpStream,
    crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<(u32, Vec<u8>)> {
    let mut header = [0u8; 4];
    stream.read_exact(&mut header)?;
    if let Some(crypto) = crypto {
        crypto.decrypt(&mut header);
    }

    let size = u16::from_be_bytes([header[0], header[1]]) as usize;
    let opcode = u16::from_le_bytes([header[2], header[3]]) as u32;
    ensure!(
        (2..=0x2800).contains(&size),
        "malformed server packet size {size}"
    );
    let body_len = size - 2;
    let body = read_exact_vec(stream, body_len)?;
    Ok((opcode, body))
}

async fn assert_count_row(
    login_pool: &MySqlPool,
    account_id: u32,
    expected: u8,
) -> anyhow::Result<()> {
    let actual: Option<u8> =
        sqlx::query_scalar("SELECT numchars FROM realmcharacters WHERE realmid = ? AND acctid = ?")
            .bind(REALM_ID)
            .bind(account_id)
            .fetch_optional(login_pool)
            .await?;
    ensure!(
        actual == Some(expected),
        "realmcharacters count for acctid={account_id} was {:?}, expected {expected}",
        actual
    );
    Ok(())
}

fn connect_blocking(addr: &str) -> anyhow::Result<TcpStream> {
    let stream = TcpStream::connect(addr).with_context(|| format!("connect to {addr}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    Ok(stream)
}

fn logon_challenge_request() -> Vec<u8> {
    let request = LogonChallengeRequest {
        cmd: AuthCommand::LogonChallenge,
        error: 0,
        size: 30 + USERNAME.len() as u16,
        game_name: *b"WoW\0",
        version_major: 1,
        version_minor: 12,
        version_patch: 1,
        build: BUILD_1121,
        platform: *b"x86\0",
        os: *b"Win\0",
        country: *b"enUS",
        timezone_bias: 0,
        ip: [127, 0, 0, 1],
        account_name: USERNAME.to_string(),
    };

    let mut bytes = BytesMut::new();
    request.write(&mut bytes);
    bytes.to_vec()
}

fn read_exact_vec(stream: &mut TcpStream, len: usize) -> anyhow::Result<Vec<u8>> {
    let mut bytes = vec![0u8; len];
    stream.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn ensure_available(body: &[u8], end: usize) -> anyhow::Result<()> {
    ensure!(
        end <= body.len(),
        "packet truncated: need {end} bytes, got {}",
        body.len()
    );
    Ok(())
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_to_array40(hex: &str) -> anyhow::Result<[u8; 40]> {
    let bytes = hex_to_vec(hex)?;
    ensure!(bytes.len() == 40, "expected 40-byte session key");
    let mut out = [0u8; 40];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn hex_to_vec(hex: &str) -> anyhow::Result<Vec<u8>> {
    let hex = hex.trim();
    ensure!(hex.len().is_multiple_of(2), "hex string has odd length");
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
