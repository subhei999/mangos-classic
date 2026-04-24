use anyhow::{ensure, Context};
use bytes::{BufMut, BytesMut};
use sqlx::mysql::MySqlPoolOptions;
use std::io::{Read, Write};
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
const PASSWORD: &str = "RUSTPASS";
const DATABASE_URL: &str = "mysql://mangos:mangos@127.0.0.1:3307/realmd";
const AUTH_ADDR: &str = "127.0.0.1:13724";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    seed_account().await?;
    let realms = complete_auth_flow()?;

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

    println!(
        "auth flow compatibility check passed: authenticated {USERNAME} and received {} realm(s)",
        realms.realms.len()
    );

    Ok(())
}

async fn seed_account() -> anyhow::Result<()> {
    let pool = MySqlPoolOptions::new()
        .max_connections(1)
        .connect(DATABASE_URL)
        .await
        .context("connect to local realmd database")?;

    let username = NormalizedString::new(USERNAME).context("normalize test username")?;
    let password = NormalizedString::new(PASSWORD).context("normalize test password")?;
    let verifier = SrpVerifier::from_username_and_password(username, password);
    let v = bytes_to_hex(verifier.password_verifier());
    let s = bytes_to_hex(verifier.salt());

    sqlx::query(
        "INSERT INTO account (username, gmlevel, sessionkey, v, s, email, locked, expansion, locale, os) \
         VALUES (?, 0, '', ?, ?, '', 0, 0, '', 'Win') \
         ON DUPLICATE KEY UPDATE sessionkey = '', v = VALUES(v), s = VALUES(s), locked = 0, os = 'Win'",
    )
    .bind(USERNAME)
    .bind(v)
    .bind(s)
    .execute(&pool)
    .await
    .context("seed auth compatibility account")?;

    Ok(())
}

fn complete_auth_flow() -> anyhow::Result<RealmListResponse> {
    let mut stream = TcpStream::connect(AUTH_ADDR).context("connect to authserver")?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .context("set read timeout")?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .context("set write timeout")?;

    let challenge_request = logon_challenge_request();
    stream
        .write_all(&challenge_request)
        .context("send logon challenge request")?;

    let mut challenge_bytes = [0u8; LogonChallengeResponse::SIZE];
    stream
        .read_exact(&mut challenge_bytes)
        .context("read logon challenge response")?;
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
        NormalizedString::new(USERNAME).context("normalize username for client")?,
        NormalizedString::new(PASSWORD).context("normalize password for client")?,
        challenge.g,
        challenge.n,
        PublicKey::from_le_bytes(challenge.server_public).context("server public key")?,
        challenge.salt,
    );

    let proof_request = LogonProofRequest {
        cmd: AuthCommand::LogonProof,
        client_public: *client.client_public_key(),
        m1: *client.client_proof(),
        crc_hash: [0; 20],
        num_keys: 0,
        security_flags: 0,
    };
    let mut proof_request_bytes = BytesMut::new();
    proof_request.write(&mut proof_request_bytes);
    stream
        .write_all(&proof_request_bytes)
        .context("send logon proof request")?;

    let mut proof_bytes = [0u8; LogonProofResponse::SIZE];
    stream
        .read_exact(&mut proof_bytes)
        .context("read logon proof response")?;
    let proof =
        LogonProofResponse::read(&mut &proof_bytes[..]).context("decode logon proof response")?;

    ensure!(proof.cmd == AuthCommand::LogonProof);
    ensure!(proof.error == 0, "proof failed with {}", proof.error);
    let _authenticated = client
        .verify_server_proof(proof.m2)
        .context("verify server proof")?;

    let realm_request = RealmListRequest {
        cmd: AuthCommand::RealmList,
        padding: 0,
    };
    let mut realm_request_bytes = BytesMut::new();
    realm_request.write(&mut realm_request_bytes);
    stream
        .write_all(&realm_request_bytes)
        .context("send realm list request")?;

    let mut header = [0u8; 3];
    stream
        .read_exact(&mut header)
        .context("read realm list response header")?;
    ensure!(header[0] == AuthCommand::RealmList as u8);
    let body_len = u16::from_le_bytes([header[1], header[2]]) as usize;
    let mut body = vec![0u8; body_len];
    stream
        .read_exact(&mut body)
        .context("read realm list response body")?;

    let mut full_response = BytesMut::new();
    full_response.put_slice(&header);
    full_response.put_slice(&body);

    RealmListResponse::read(&mut &full_response[..]).context("decode realm list response")
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
        build: 5875,
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

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
