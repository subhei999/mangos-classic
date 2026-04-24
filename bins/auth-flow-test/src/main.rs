use anyhow::{ensure, Context};
use bytes::{BufMut, BytesMut};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::time::Duration;
use wow_proto::{
    AuthCommand, LogonChallengeRequest, LogonChallengeResponse, LogonProofRequest,
    LogonProofResponse, RealmListRequest, RealmListResponse,
};
use wow_srp::client::SrpClientChallenge;
use wow_srp::normalized_string::NormalizedString;
use wow_srp::server::SrpVerifier;
use wow_srp::PublicKey;

const USERNAME: &str = "RUSTAUTH";
const BAD_PROOF_USERNAME: &str = "RUSTBADPROOF";
const BANNED_USERNAME: &str = "RUSTBANNED";
const UNKNOWN_USERNAME: &str = "RUSTUNKNOWN";
const UNSUPPORTED_BUILD_USERNAME: &str = "RUSTBUILD";
const PASSWORD: &str = "RUSTPASS";
const BAD_PASSWORD: &str = "WRONGPASS";
const BUILD_1121: u16 = 5875;
const UNSUPPORTED_BUILD: u16 = 12340;
const DATABASE_URL: &str = "mysql://mangos:mangos@127.0.0.1:3307/realmd";
const AUTH_ADDR: &str = "127.0.0.1:13724";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pool = MySqlPoolOptions::new()
        .max_connections(1)
        .connect(DATABASE_URL)
        .await
        .context("connect to local realmd database")?;

    seed_account(&pool, USERNAME, PASSWORD, false).await?;
    seed_account(&pool, BAD_PROOF_USERNAME, PASSWORD, false).await?;
    seed_account(&pool, BANNED_USERNAME, PASSWORD, true).await?;
    seed_account(&pool, UNSUPPORTED_BUILD_USERNAME, PASSWORD, false).await?;

    let realms = complete_auth_flow(USERNAME, PASSWORD, BUILD_1121)?;
    ensure!(!realms.realms.is_empty(), "realm list was empty");
    ensure!(
        realms.realms.iter().any(|realm| realm.name == "MaNGOS"),
        "expected seeded MaNGOS realm in realm list, got {:?}",
        realms
            .realms
            .iter()
            .map(|realm| realm.name.as_str())
            .collect::<Vec<_>>()
    );

    assert_unknown_account()?;
    assert_bad_proof(&pool).await?;
    assert_banned_account()?;
    assert_realm_list_before_auth_rejected()?;
    assert_unsupported_build()?;

    println!(
        "auth flow compatibility check passed: success, common failures, and realm-list gating"
    );

    Ok(())
}

async fn seed_account(
    pool: &MySqlPool,
    username: &str,
    password: &str,
    banned: bool,
) -> anyhow::Result<()> {
    let normalized_username = NormalizedString::new(username).context("normalize test username")?;
    let normalized_password = NormalizedString::new(password).context("normalize test password")?;
    let verifier =
        SrpVerifier::from_username_and_password(normalized_username, normalized_password);
    let v = bytes_to_hex(verifier.password_verifier());
    let s = bytes_to_hex(verifier.salt());

    sqlx::query(
        "INSERT INTO account (username, gmlevel, sessionkey, v, s, email, locked, expansion, locale, os) \
         VALUES (?, 0, '', ?, ?, '', 0, 0, '', 'Win') \
         ON DUPLICATE KEY UPDATE sessionkey = '', v = VALUES(v), s = VALUES(s), locked = 0, os = 'Win'",
    )
    .bind(username)
    .bind(v)
    .bind(s)
    .execute(pool)
    .await
    .with_context(|| format!("seed auth compatibility account {username}"))?;

    let account_id: u32 = sqlx::query_scalar("SELECT id FROM account WHERE username = ?")
        .bind(username)
        .fetch_one(pool)
        .await
        .with_context(|| format!("fetch seeded account id for {username}"))?;

    sqlx::query("DELETE FROM account_banned WHERE account_id = ?")
        .bind(account_id)
        .execute(pool)
        .await
        .with_context(|| format!("clear stale bans for {username}"))?;

    if banned {
        sqlx::query(
            "INSERT INTO account_banned \
             (account_id, banned_at, expires_at, banned_by, reason, active) \
             VALUES (?, UNIX_TIMESTAMP(), UNIX_TIMESTAMP(), 'auth-flow-test', 'compatibility test', 1)",
        )
        .bind(account_id)
        .execute(pool)
        .await
        .with_context(|| format!("seed active permanent ban for {username}"))?;
    }

    Ok(())
}

fn complete_auth_flow(
    username: &str,
    password: &str,
    build: u16,
) -> anyhow::Result<RealmListResponse> {
    let mut stream = connect()?;
    let (challenge, client) = perform_challenge(&mut stream, username, password, build)?;
    ensure!(
        challenge.error == 0,
        "challenge failed with {}",
        challenge.error
    );

    let proof = send_proof(&mut stream, &client, false)?;
    ensure!(proof.cmd == AuthCommand::LogonProof);
    ensure!(proof.error == 0, "proof failed with {}", proof.error);
    let _authenticated = client
        .verify_server_proof(proof.m2)
        .context("verify server proof")?;

    request_realm_list(&mut stream)
}

fn assert_unknown_account() -> anyhow::Result<()> {
    let mut stream = connect()?;
    stream
        .write_all(&logon_challenge_request(UNKNOWN_USERNAME, BUILD_1121))
        .context("send unknown-account challenge request")?;

    let response = read_exact_vec(&mut stream, 3).context("read unknown-account response")?;
    ensure!(
        response == vec![AuthCommand::LogonChallenge as u8, 0x00, 0x04],
        "unknown account response was {:02X?}",
        response
    );
    Ok(())
}

async fn assert_bad_proof(pool: &MySqlPool) -> anyhow::Result<()> {
    let mut stream = connect()?;
    let (_challenge, client) =
        perform_challenge(&mut stream, BAD_PROOF_USERNAME, BAD_PASSWORD, BUILD_1121)?;

    write_proof_request(&mut stream, &client, false)?;
    let response = read_exact_vec(&mut stream, 2).context("read bad-proof response")?;
    ensure!(
        response == vec![AuthCommand::LogonProof as u8, 0x04],
        "bad proof response was {:02X?}",
        response
    );

    let sessionkey: Option<String> =
        sqlx::query_scalar("SELECT NULLIF(sessionkey, '') FROM account WHERE username = ?")
            .bind(BAD_PROOF_USERNAME)
            .fetch_one(pool)
            .await
            .context("fetch bad-proof account session key")?;
    ensure!(
        sessionkey.is_none(),
        "bad proof unexpectedly authenticated and wrote a session key"
    );
    Ok(())
}

fn assert_banned_account() -> anyhow::Result<()> {
    let mut stream = connect()?;
    stream
        .write_all(&logon_challenge_request(BANNED_USERNAME, BUILD_1121))
        .context("send banned-account challenge request")?;

    let response = read_exact_vec(&mut stream, 3).context("read banned-account response")?;
    ensure!(
        response == vec![AuthCommand::LogonChallenge as u8, 0x00, 0x03],
        "banned account response was {:02X?}",
        response
    );
    Ok(())
}

fn assert_realm_list_before_auth_rejected() -> anyhow::Result<()> {
    let mut stream = connect()?;
    let mut request = BytesMut::new();
    RealmListRequest {
        cmd: AuthCommand::RealmList,
        padding: 0,
    }
    .write(&mut request);
    stream
        .write_all(&request)
        .context("send pre-auth realm-list request")?;

    let mut b = [0u8; 1];
    match stream.read(&mut b) {
        Ok(0) => Ok(()),
        Err(e)
            if matches!(
                e.kind(),
                ErrorKind::ConnectionAborted
                    | ErrorKind::ConnectionReset
                    | ErrorKind::UnexpectedEof
                    | ErrorKind::TimedOut
                    | ErrorKind::WouldBlock
            ) =>
        {
            Ok(())
        }
        Ok(n) => anyhow::bail!(
            "pre-auth realm-list unexpectedly returned {} byte(s), first byte 0x{:02X}",
            n,
            b[0]
        ),
        Err(e) => Err(e).context("read pre-auth realm-list response"),
    }
}

fn assert_unsupported_build() -> anyhow::Result<()> {
    let mut stream = connect()?;
    let (_challenge, client) = perform_challenge(
        &mut stream,
        UNSUPPORTED_BUILD_USERNAME,
        PASSWORD,
        UNSUPPORTED_BUILD,
    )?;

    write_proof_request(&mut stream, &client, false)?;
    let response = read_exact_vec(&mut stream, 3).context("read unsupported-build response")?;
    ensure!(
        response == vec![AuthCommand::LogonChallenge as u8, 0x00, 0x09],
        "unsupported-build response was {:02X?}",
        response
    );

    let mut b = [0u8; 1];
    match stream.read(&mut b) {
        Ok(0) => Ok(()),
        Err(e)
            if matches!(
                e.kind(),
                ErrorKind::ConnectionAborted
                    | ErrorKind::ConnectionReset
                    | ErrorKind::UnexpectedEof
                    | ErrorKind::TimedOut
                    | ErrorKind::WouldBlock
            ) =>
        {
            Ok(())
        }
        Ok(n) => anyhow::bail!(
            "unsupported build connection stayed readable with {} byte(s), first byte 0x{:02X}",
            n,
            b[0]
        ),
        Err(e) => Err(e).context("read after unsupported-build response"),
    }
}

fn connect() -> anyhow::Result<TcpStream> {
    let stream = TcpStream::connect(AUTH_ADDR).context("connect to authserver")?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .context("set read timeout")?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .context("set write timeout")?;
    Ok(stream)
}

fn perform_challenge(
    stream: &mut TcpStream,
    username: &str,
    password: &str,
    build: u16,
) -> anyhow::Result<(LogonChallengeResponse, SrpClientChallenge)> {
    stream
        .write_all(&logon_challenge_request(username, build))
        .context("send logon challenge request")?;

    let challenge_bytes =
        read_exact_vec(stream, LogonChallengeResponse::SIZE).context("read challenge response")?;
    let challenge = LogonChallengeResponse::read(&mut &challenge_bytes[..])
        .context("decode logon challenge response")?;

    ensure!(challenge.cmd == AuthCommand::LogonChallenge);
    ensure!(
        challenge.error == 0,
        "challenge failed with {}",
        challenge.error
    );
    ensure!(challenge.g_len == 1, "unexpected generator length");
    ensure!(challenge.n_len == 32, "unexpected safe-prime length");

    let client = SrpClientChallenge::new(
        NormalizedString::new(username).context("normalize username for client")?,
        NormalizedString::new(password).context("normalize password for client")?,
        challenge.g,
        challenge.n,
        PublicKey::from_le_bytes(challenge.server_public).context("server public key")?,
        challenge.salt,
    );

    Ok((challenge, client))
}

fn send_proof(
    stream: &mut TcpStream,
    client: &SrpClientChallenge,
    corrupt: bool,
) -> anyhow::Result<LogonProofResponse> {
    write_proof_request(stream, client, corrupt)?;
    let proof_bytes =
        read_exact_vec(stream, LogonProofResponse::SIZE).context("read logon proof response")?;
    LogonProofResponse::read(&mut &proof_bytes[..]).context("decode logon proof response")
}

fn write_proof_request(
    stream: &mut TcpStream,
    client: &SrpClientChallenge,
    corrupt: bool,
) -> anyhow::Result<()> {
    let mut m1 = *client.client_proof();
    if corrupt {
        m1[0] ^= 0xFF;
    }

    let proof_request = LogonProofRequest {
        cmd: AuthCommand::LogonProof,
        client_public: *client.client_public_key(),
        m1,
        crc_hash: [0; 20],
        num_keys: 0,
        security_flags: 0,
    };
    let mut proof_request_bytes = BytesMut::new();
    proof_request.write(&mut proof_request_bytes);
    stream
        .write_all(&proof_request_bytes)
        .context("send logon proof request")?;
    Ok(())
}

fn request_realm_list(stream: &mut TcpStream) -> anyhow::Result<RealmListResponse> {
    let realm_request = RealmListRequest {
        cmd: AuthCommand::RealmList,
        padding: 0,
    };
    let mut realm_request_bytes = BytesMut::new();
    realm_request.write(&mut realm_request_bytes);
    stream
        .write_all(&realm_request_bytes)
        .context("send realm list request")?;

    let header = read_exact_vec(stream, 3).context("read realm list response header")?;
    ensure!(header[0] == AuthCommand::RealmList as u8);
    let body_len = u16::from_le_bytes([header[1], header[2]]) as usize;
    let body = read_exact_vec(stream, body_len).context("read realm list response body")?;

    let mut full_response = BytesMut::new();
    full_response.put_slice(&header);
    full_response.put_slice(&body);

    RealmListResponse::read(&mut &full_response[..]).context("decode realm list response")
}

fn read_exact_vec(stream: &mut TcpStream, len: usize) -> anyhow::Result<Vec<u8>> {
    let mut bytes = vec![0u8; len];
    stream.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn logon_challenge_request(username: &str, build: u16) -> Vec<u8> {
    let request = LogonChallengeRequest {
        cmd: AuthCommand::LogonChallenge,
        error: 0,
        size: 30 + username.len() as u16,
        game_name: *b"WoW\0",
        version_major: 1,
        version_minor: 12,
        version_patch: 1,
        build,
        platform: *b"x86\0",
        os: *b"Win\0",
        country: *b"enUS",
        timezone_bias: 0,
        ip: [127, 0, 0, 1],
        account_name: username.to_string(),
    };

    let mut bytes = BytesMut::new();
    request.write(&mut bytes);
    bytes.to_vec()
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
