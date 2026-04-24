//! Auth session state machine for the WoW 1.12.x login handshake.
//!
//! Protocol flow:
//!   1. Client sends CMD_AUTH_LOGON_CHALLENGE (0x00)
//!   2. Server responds with SRP6 parameters (B, g, N, s)
//!   3. Client sends CMD_AUTH_LOGON_PROOF (0x01) with A and M1
//!   4. Server verifies M1, responds with M2
//!   5. Client sends CMD_REALM_LIST (0x10) one or more times

use byteorder::{LittleEndian, WriteBytesExt};
use sqlx::mysql::MySqlPool;
use std::io::{Cursor, Read, Write};
use tracing::{debug, info, warn};

use wow_crypto::srp::SrpAuth;

// ---------------------------------------------------------------------------
// AuthState
// ---------------------------------------------------------------------------

/// States of the auth session state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthState {
    /// Waiting for CMD_AUTH_LOGON_CHALLENGE.
    Connected,
    /// Challenge sent, waiting for CMD_AUTH_LOGON_PROOF.
    ChallengeSent,
    /// Fully authenticated. Realm list queries are allowed.
    Authenticated,
}

// ---------------------------------------------------------------------------
// AuthSession
// ---------------------------------------------------------------------------

/// Per-connection authentication session.
///
/// Drives the SRP6 login handshake and realm-list exchange.
pub struct AuthSession {
    state: AuthState,
    db_pool: MySqlPool,
    /// SRP proof handle, populated after challenge generation.
    srp: Option<SrpAuth>,
    /// Normalised (uppercased) account name.
    username: Option<String>,
    /// Database account ID.
    account_id: Option<u32>,
}

impl AuthSession {
    /// Create a new session in the [`AuthState::Connected`] state.
    pub fn new(db_pool: MySqlPool) -> Self {
        Self {
            state: AuthState::Connected,
            db_pool,
            srp: None,
            username: None,
            account_id: None,
        }
    }

    /// Return the current state.
    pub fn state(&self) -> AuthState {
        self.state
    }

    // -----------------------------------------------------------------------
    // CMD_AUTH_LOGON_CHALLENGE (0x00)
    // -----------------------------------------------------------------------

    /// Parse CMD_AUTH_LOGON_CHALLENGE, look up the account in the database,
    /// generate the SRP6 challenge, and return the response bytes.
    ///
    /// Wire format (client -> server):
    /// ```text
    /// u8   cmd            = 0x00
    /// u8   error          = 0x03
    /// u16  size           (remaining packet length)
    /// u8[4] game_name     ("WoW\0" reversed)
    /// u8[3] version       (1, 12, 1)
    /// u16  build          (5875)
    /// u8[4] platform      ("x86\0" reversed)
    /// u8[4] os            ("Win\0" reversed)
    /// u8[4] locale        ("enUS" reversed)
    /// u32  timezone_bias
    /// u32  ip
    /// u8   name_len
    /// u8[N] account_name
    /// ```
    pub async fn handle_logon_challenge(&mut self, data: &[u8]) -> anyhow::Result<Vec<u8>> {
        if self.state != AuthState::Connected {
            anyhow::bail!("Unexpected logon challenge in state {:?}", self.state);
        }

        // Minimum size: 1+1+2 + 4+3+2+4+4+4+4+4+1 = 34 bytes (before name).
        if data.len() < 34 {
            anyhow::bail!("Logon challenge packet too short ({} bytes)", data.len());
        }

        let mut cursor = Cursor::new(data);
        // Skip cmd(1) + error(1) + size(2).
        cursor.set_position(4);
        // Skip game_name(4) + version(3) + build(2) + platform(4) + os(4) +
        // locale(4) + timezone_bias(4) + ip(4) = 29 bytes.
        cursor.set_position(4 + 29);

        let mut name_len_buf = [0u8; 1];
        cursor.read_exact(&mut name_len_buf)?;
        let name_len = name_len_buf[0] as usize;

        if cursor.position() as usize + name_len > data.len() {
            anyhow::bail!("Account name extends past packet boundary");
        }

        let mut name_bytes = vec![0u8; name_len];
        cursor.read_exact(&mut name_bytes)?;
        let username = wow_db::account::normalize_username(&String::from_utf8(name_bytes)?);

        debug!("Auth challenge for account: {}", username);

        // Look up the account.
        let account = wow_db::account::get_account_by_username(&self.db_pool, &username).await?;
        let account = match account {
            Some(a) => a,
            None => {
                warn!("Unknown account: {}", username);
                return Ok(build_challenge_error(0x04)); // WOW_FAIL_UNKNOWN_ACCOUNT
            }
        };

        // Check bans.
        if let Some(_ban) = wow_db::account::get_account_banned(&self.db_pool, account.id).await? {
            warn!("Banned account tried to log in: {}", username);
            return Ok(build_challenge_error(0x03)); // WOW_FAIL_BANNED
        }

        // Parse stored verifier (v) and salt (s) from hex strings in the DB.
        let v_bytes = hex_to_array32(&account.v)?;
        let s_bytes = hex_to_array32(&account.s)?;

        let srp = SrpAuth::from_database_values(&username, v_bytes, s_bytes)?;
        let challenge = srp.server_challenge();

        self.username = Some(username);
        self.account_id = Some(account.id);

        // Build response bytes.
        let mut resp = Vec::with_capacity(119);
        resp.write_u8(0x00)?; // cmd
        resp.write_u8(0x00)?; // unk
        resp.write_u8(0x00)?; // error = WOW_SUCCESS

        // B (server public key), 32 bytes.
        resp.write_all(&challenge.server_public_key)?;

        // g length + g.
        resp.write_u8(1)?;
        resp.write_u8(challenge.generator)?;

        // N length + N (large safe prime), 32 bytes.
        resp.write_u8(32)?;
        resp.write_all(&challenge.large_safe_prime)?;

        // s (salt), 32 bytes.
        resp.write_all(&challenge.salt)?;

        // CRC salt (server does not validate it), 16 zero bytes.
        resp.write_all(&[0u8; 16])?;

        // Security flags (none).
        resp.write_u8(0x00)?;

        self.srp = Some(srp);
        self.state = AuthState::ChallengeSent;

        Ok(resp)
    }

    // -----------------------------------------------------------------------
    // CMD_AUTH_LOGON_PROOF (0x01)
    // -----------------------------------------------------------------------

    /// Parse CMD_AUTH_LOGON_PROOF, verify the client proof M1, compute M2,
    /// persist the session key to the database, and return the response.
    ///
    /// Wire format (client -> server):
    /// ```text
    /// u8    cmd             = 0x01
    /// u8[32] A              (client public key)
    /// u8[20] M1             (client proof)
    /// u8[20] crc_hash
    /// u8    number_of_keys
    /// u8    security_flags
    /// ```
    pub async fn handle_logon_proof(&mut self, data: &[u8]) -> anyhow::Result<Vec<u8>> {
        if self.state != AuthState::ChallengeSent {
            anyhow::bail!("Unexpected logon proof in state {:?}", self.state);
        }

        // 1 + 32 + 20 + 20 + 1 + 1 = 75 bytes minimum.
        if data.len() < 75 {
            anyhow::bail!("Logon proof packet too short ({} bytes)", data.len());
        }

        let mut a = [0u8; 32];
        a.copy_from_slice(&data[1..33]);

        let mut m1 = [0u8; 20];
        m1.copy_from_slice(&data[33..53]);

        let srp = self
            .srp
            .take()
            .ok_or_else(|| anyhow::anyhow!("SRP state missing during proof"))?;

        let (auth_result, _server) = match srp.verify_client_proof(a, m1) {
            Ok(r) => r,
            Err(_) => {
                warn!("Client proof verification failed");
                self.state = AuthState::Connected;
                return Ok(build_proof_error(0x04)); // WOW_FAIL_UNKNOWN_ACCOUNT
            }
        };

        // Persist the session key.
        let session_key_hex = bytes_to_hex(&auth_result.session_key);
        let account_id = self.account_id.unwrap();

        // Re-fetch account to get the current v/s (they do not change, but the
        // DB schema stores them alongside the session key).
        let account = wow_db::account::get_account_by_username(
            &self.db_pool,
            self.username.as_deref().unwrap(),
        )
        .await?
        .ok_or_else(|| anyhow::anyhow!("Account vanished during proof"))?;

        wow_db::account::update_session_key(
            &self.db_pool,
            account_id,
            &session_key_hex,
            &account.v,
            &account.s,
        )
        .await?;

        info!(
            "Account '{}' authenticated successfully",
            self.username.as_deref().unwrap_or("?")
        );

        // Build the success response.
        let mut resp = Vec::with_capacity(32);
        resp.write_u8(0x01)?; // cmd
        resp.write_u8(0x00)?; // error = WOW_SUCCESS
        resp.write_all(&auth_result.server_proof)?; // M2, 20 bytes
        resp.write_u32::<LittleEndian>(0x00)?; // classic 1.12.x login_flags

        self.state = AuthState::Authenticated;

        Ok(resp)
    }

    // -----------------------------------------------------------------------
    // CMD_REALM_LIST (0x10)
    // -----------------------------------------------------------------------

    /// Query the realm list from the database and build the response packet.
    ///
    /// Wire format (client -> server):
    /// ```text
    /// u8   cmd      = 0x10
    /// u32  padding  (unused)
    /// ```
    pub async fn handle_realm_list(&self, _data: &[u8]) -> anyhow::Result<Vec<u8>> {
        if self.state != AuthState::Authenticated {
            anyhow::bail!("Realm list requested before authentication");
        }

        let realms = wow_db::realm::get_realm_list(&self.db_pool).await?;
        let account_id = self.account_id.unwrap();
        let char_counts = wow_db::realm::get_realm_characters(&self.db_pool, account_id).await?;

        // Build the realm-list body so we can compute its size.
        let mut body = Vec::with_capacity(256);

        // u32 padding.
        body.write_u32::<LittleEndian>(0)?;

        // Number of realms (u8).
        let num_realms = realms.len().min(255) as u8;
        body.write_u8(num_realms)?;

        for realm in &realms {
            // Realm type (icon): u32 in 1.12.x wire format.
            body.write_u32::<LittleEndian>(realm.icon as u32)?;

            // Realm flags: u8.
            body.write_u8(realm.realmflags)?;

            // Realm name: NUL-terminated.
            body.write_all(realm.name.as_bytes())?;
            body.write_u8(0)?;

            // Address: "ip:port" NUL-terminated.
            let addr = wow_db::realm::realm_address(&realm.address, realm.port);
            body.write_all(addr.as_bytes())?;
            body.write_u8(0)?;

            // Population: f32 as LE bytes.
            body.write_u32::<LittleEndian>(realm.population.to_bits())?;

            // Number of characters this account has on this realm.
            let num_chars = char_counts
                .iter()
                .find(|(rid, _)| *rid == realm.id)
                .map(|(_, c)| *c)
                .unwrap_or(0);
            body.write_u8(num_chars)?;

            // Timezone: u8.
            body.write_u8(realm.timezone)?;

            // Realm ID: u8.
            body.write_u8(realm.id as u8)?;
        }

        // Footer padding (u16).
        body.write_u16::<LittleEndian>(0x0002)?;

        // Assemble the full packet.
        let mut resp = Vec::with_capacity(body.len() + 3);
        resp.write_u8(0x10)?; // cmd
        resp.write_u16::<LittleEndian>(body.len() as u16)?; // body size
        resp.write_all(&body)?;

        Ok(resp)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a CMD_AUTH_LOGON_CHALLENGE error response.
fn build_challenge_error(error_code: u8) -> Vec<u8> {
    vec![0x00, 0x00, error_code]
}

/// Build a CMD_AUTH_LOGON_PROOF error response.
fn build_proof_error(error_code: u8) -> Vec<u8> {
    vec![0x01, error_code, 0x00, 0x00]
}

/// Parse a 64-character hex string into a 32-byte array.
fn hex_to_array32(hex: &str) -> anyhow::Result<[u8; 32]> {
    let hex = hex.trim();
    if hex.len() != 64 {
        anyhow::bail!(
            "Expected 64-char hex string for 32 bytes, got {} chars",
            hex.len()
        );
    }
    let mut out = [0u8; 32];
    for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
        out[i] = (hex_nibble(chunk[0])? << 4) | hex_nibble(chunk[1])?;
    }
    Ok(out)
}

fn hex_nibble(c: u8) -> anyhow::Result<u8> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => anyhow::bail!("Invalid hex character: 0x{:02X}", c),
    }
}

/// Convert a byte slice to a lowercase hex string.
fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        let original = [0xAB; 32];
        let hex = bytes_to_hex(&original);
        let decoded = hex_to_array32(&hex).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn hex_nibble_values() {
        assert_eq!(hex_nibble(b'0').unwrap(), 0);
        assert_eq!(hex_nibble(b'9').unwrap(), 9);
        assert_eq!(hex_nibble(b'a').unwrap(), 10);
        assert_eq!(hex_nibble(b'f').unwrap(), 15);
        assert_eq!(hex_nibble(b'A').unwrap(), 10);
        assert!(hex_nibble(b'g').is_err());
    }

    #[test]
    fn challenge_error_format() {
        let err = build_challenge_error(0x04);
        assert_eq!(err, vec![0x00, 0x00, 0x04]);
    }

    #[test]
    fn proof_error_format() {
        let err = build_proof_error(0x04);
        assert_eq!(err, vec![0x01, 0x04, 0x00, 0x00]);
    }
}
