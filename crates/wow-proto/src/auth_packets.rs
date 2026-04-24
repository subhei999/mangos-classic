//! Auth server (realmd) packet structures for WoW 1.12.x Classic.
//!
//! These packets are used during the login/realm-selection flow before the client
//! connects to the world server.

use bytes::{Buf, BufMut, BytesMut};
use std::io;

// ---------------------------------------------------------------------------
// AuthCommand
// ---------------------------------------------------------------------------

/// Authentication command byte that starts every auth packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AuthCommand {
    LogonChallenge = 0x00,
    LogonProof = 0x01,
    ReconnectChallenge = 0x02,
    ReconnectProof = 0x03,
    RealmList = 0x10,
}

impl TryFrom<u8> for AuthCommand {
    type Error = io::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(AuthCommand::LogonChallenge),
            0x01 => Ok(AuthCommand::LogonProof),
            0x02 => Ok(AuthCommand::ReconnectChallenge),
            0x03 => Ok(AuthCommand::ReconnectProof),
            0x10 => Ok(AuthCommand::RealmList),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unknown auth command: 0x{value:02X}"),
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// LogonChallengeRequest
// ---------------------------------------------------------------------------

/// Client -> Auth: logon challenge (CMD_AUTH_LOGON_CHALLENGE_C).
#[derive(Debug, Clone)]
pub struct LogonChallengeRequest {
    pub cmd: AuthCommand,
    pub error: u8,
    pub size: u16,
    pub game_name: [u8; 4],
    pub version_major: u8,
    pub version_minor: u8,
    pub version_patch: u8,
    pub build: u16,
    pub platform: [u8; 4],
    pub os: [u8; 4],
    pub country: [u8; 4],
    pub timezone_bias: u32,
    pub ip: [u8; 4],
    pub account_name: String,
}

impl LogonChallengeRequest {
    /// Minimum packet size (everything except the variable-length account name).
    pub const MIN_SIZE: usize = 1 + 1 + 2 + 4 + 3 + 2 + 4 + 4 + 4 + 4 + 4 + 1;

    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        if buf.remaining() < Self::MIN_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "logon challenge request too short",
            ));
        }

        let cmd = AuthCommand::try_from(buf.get_u8())?;
        let error = buf.get_u8();
        let size = buf.get_u16_le();

        let mut game_name = [0u8; 4];
        buf.copy_to_slice(&mut game_name);

        let version_major = buf.get_u8();
        let version_minor = buf.get_u8();
        let version_patch = buf.get_u8();
        let build = buf.get_u16_le();

        let mut platform = [0u8; 4];
        buf.copy_to_slice(&mut platform);

        let mut os = [0u8; 4];
        buf.copy_to_slice(&mut os);

        let mut country = [0u8; 4];
        buf.copy_to_slice(&mut country);

        let timezone_bias = buf.get_u32_le();

        let mut ip = [0u8; 4];
        buf.copy_to_slice(&mut ip);

        let name_len = buf.get_u8() as usize;
        if buf.remaining() < name_len {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "logon challenge request: account name truncated",
            ));
        }
        let mut name_bytes = vec![0u8; name_len];
        buf.copy_to_slice(&mut name_bytes);
        let account_name =
            String::from_utf8(name_bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        Ok(Self {
            cmd,
            error,
            size,
            game_name,
            version_major,
            version_minor,
            version_patch,
            build,
            platform,
            os,
            country,
            timezone_bias,
            ip,
            account_name,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.cmd as u8);
        buf.put_u8(self.error);
        buf.put_u16_le(self.size);
        buf.put_slice(&self.game_name);
        buf.put_u8(self.version_major);
        buf.put_u8(self.version_minor);
        buf.put_u8(self.version_patch);
        buf.put_u16_le(self.build);
        buf.put_slice(&self.platform);
        buf.put_slice(&self.os);
        buf.put_slice(&self.country);
        buf.put_u32_le(self.timezone_bias);
        buf.put_slice(&self.ip);
        buf.put_u8(self.account_name.len() as u8);
        buf.put_slice(self.account_name.as_bytes());
    }
}

// ---------------------------------------------------------------------------
// LogonChallengeResponse
// ---------------------------------------------------------------------------

/// Auth -> Client: logon challenge response (CMD_AUTH_LOGON_CHALLENGE_S).
#[derive(Debug, Clone)]
pub struct LogonChallengeResponse {
    pub cmd: AuthCommand,
    pub unk: u8,
    pub error: u8,
    /// Server public ephemeral value B (32 bytes).
    pub server_public: [u8; 32],
    pub g_len: u8,
    pub g: u8,
    pub n_len: u8,
    /// Safe prime N (32 bytes, little-endian).
    pub n: [u8; 32],
    /// Salt (32 bytes).
    pub salt: [u8; 32],
    /// CRC salt (16 bytes).
    pub crc_salt: [u8; 16],
    pub security_flags: u8,
}

impl LogonChallengeResponse {
    pub const SIZE: usize = 1 + 1 + 1 + 32 + 1 + 1 + 1 + 32 + 32 + 16 + 1;

    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        if buf.remaining() < Self::SIZE {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "logon challenge response too short",
            ));
        }

        let cmd = AuthCommand::try_from(buf.get_u8())?;
        let unk = buf.get_u8();
        let error = buf.get_u8();

        // If error != 0, the remaining fields are absent in the real protocol.
        // We read them unconditionally here; callers should check `error` first.
        let mut server_public = [0u8; 32];
        buf.copy_to_slice(&mut server_public);

        let g_len = buf.get_u8();
        let g = buf.get_u8();
        let n_len = buf.get_u8();

        let mut n = [0u8; 32];
        buf.copy_to_slice(&mut n);

        let mut salt = [0u8; 32];
        buf.copy_to_slice(&mut salt);

        let mut crc_salt = [0u8; 16];
        buf.copy_to_slice(&mut crc_salt);

        let security_flags = buf.get_u8();

        Ok(Self {
            cmd,
            unk,
            error,
            server_public,
            g_len,
            g,
            n_len,
            n,
            salt,
            crc_salt,
            security_flags,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.cmd as u8);
        buf.put_u8(self.unk);
        buf.put_u8(self.error);
        buf.put_slice(&self.server_public);
        buf.put_u8(self.g_len);
        buf.put_u8(self.g);
        buf.put_u8(self.n_len);
        buf.put_slice(&self.n);
        buf.put_slice(&self.salt);
        buf.put_slice(&self.crc_salt);
        buf.put_u8(self.security_flags);
    }
}

// ---------------------------------------------------------------------------
// LogonProofRequest
// ---------------------------------------------------------------------------

/// Client -> Auth: logon proof (CMD_AUTH_LOGON_PROOF_C).
#[derive(Debug, Clone)]
pub struct LogonProofRequest {
    pub cmd: AuthCommand,
    /// Client public ephemeral value A (32 bytes).
    pub client_public: [u8; 32],
    /// Client proof M1 (20 bytes, SHA-1).
    pub m1: [u8; 20],
    /// CRC hash (20 bytes).
    pub crc_hash: [u8; 20],
    pub num_keys: u8,
    pub security_flags: u8,
}

impl LogonProofRequest {
    pub const SIZE: usize = 1 + 32 + 20 + 20 + 1 + 1;

    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        if buf.remaining() < Self::SIZE {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "logon proof request too short",
            ));
        }

        let cmd = AuthCommand::try_from(buf.get_u8())?;

        let mut client_public = [0u8; 32];
        buf.copy_to_slice(&mut client_public);

        let mut m1 = [0u8; 20];
        buf.copy_to_slice(&mut m1);

        let mut crc_hash = [0u8; 20];
        buf.copy_to_slice(&mut crc_hash);

        let num_keys = buf.get_u8();
        let security_flags = buf.get_u8();

        Ok(Self {
            cmd,
            client_public,
            m1,
            crc_hash,
            num_keys,
            security_flags,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.cmd as u8);
        buf.put_slice(&self.client_public);
        buf.put_slice(&self.m1);
        buf.put_slice(&self.crc_hash);
        buf.put_u8(self.num_keys);
        buf.put_u8(self.security_flags);
    }
}

// ---------------------------------------------------------------------------
// LogonProofResponse
// ---------------------------------------------------------------------------

/// Auth -> Client: logon proof response (CMD_AUTH_LOGON_PROOF_S).
#[derive(Debug, Clone)]
pub struct LogonProofResponse {
    pub cmd: AuthCommand,
    pub error: u8,
    /// Server proof M2 (20 bytes, SHA-1).
    pub m2: [u8; 20],
    /// Classic 1.12.x login flags. CMaNGOS writes this as a single u32 for
    /// builds 5875, 6005, and 6141.
    pub login_flags: u32,
}

impl LogonProofResponse {
    pub const SIZE: usize = 1 + 1 + 20 + 4;

    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        if buf.remaining() < Self::SIZE {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "logon proof response too short",
            ));
        }

        let cmd = AuthCommand::try_from(buf.get_u8())?;
        let error = buf.get_u8();

        let mut m2 = [0u8; 20];
        buf.copy_to_slice(&mut m2);

        let login_flags = buf.get_u32_le();

        Ok(Self {
            cmd,
            error,
            m2,
            login_flags,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.cmd as u8);
        buf.put_u8(self.error);
        buf.put_slice(&self.m2);
        buf.put_u32_le(self.login_flags);
    }
}

// ---------------------------------------------------------------------------
// RealmList
// ---------------------------------------------------------------------------

/// Client -> Auth: realm list request (CMD_REALM_LIST_C).
#[derive(Debug, Clone)]
pub struct RealmListRequest {
    pub cmd: AuthCommand,
    pub padding: u32,
}

impl RealmListRequest {
    pub const SIZE: usize = 1 + 4;

    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        if buf.remaining() < Self::SIZE {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "realm list request too short",
            ));
        }
        let cmd = AuthCommand::try_from(buf.get_u8())?;
        let padding = buf.get_u32_le();
        Ok(Self { cmd, padding })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.cmd as u8);
        buf.put_u32_le(self.padding);
    }
}

/// Information about a single realm.
#[derive(Debug, Clone)]
pub struct RealmInfo {
    /// Realm type: 0 = Normal, 1 = PvP, 6 = RP, 8 = RPPvP.
    /// Classic 1.12.x sends this as a u32.
    pub realm_type: u32,
    /// Realm flags (e.g. 0x00 = online, 0x02 = offline).
    pub flags: u8,
    /// Realm name (NUL-terminated C string in the wire format).
    pub name: String,
    /// Address string, e.g. "127.0.0.1:8085".
    pub address: String,
    /// Population level (float32, 0.0 = low, 1.0 = medium, 2.0 = high).
    pub population: f32,
    /// Number of characters the account has on this realm.
    pub characters: u8,
    /// Timezone / category.
    pub timezone: u8,
    /// Realm ID.
    pub realm_id: u8,
}

impl RealmInfo {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        if buf.remaining() < 10 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "realm info too short",
            ));
        }

        let realm_type = buf.get_u32_le();
        let flags = buf.get_u8();

        let name = read_cstring(buf)?;
        let address = read_cstring(buf)?;

        if buf.remaining() < 4 + 1 + 1 + 1 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "realm info truncated",
            ));
        }

        let population = buf.get_f32_le();
        let characters = buf.get_u8();
        let timezone = buf.get_u8();
        let realm_id = buf.get_u8();

        Ok(Self {
            realm_type,
            flags,
            name,
            address,
            population,
            characters,
            timezone,
            realm_id,
        })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.realm_type);
        buf.put_u8(self.flags);
        write_cstring(buf, &self.name);
        write_cstring(buf, &self.address);
        buf.put_f32_le(self.population);
        buf.put_u8(self.characters);
        buf.put_u8(self.timezone);
        buf.put_u8(self.realm_id);
    }
}

/// Auth -> Client: realm list response (CMD_REALM_LIST_S).
#[derive(Debug, Clone)]
pub struct RealmListResponse {
    pub cmd: AuthCommand,
    /// Total size of the body that follows (u16), filled in by `write`.
    pub realms: Vec<RealmInfo>,
}

impl RealmListResponse {
    pub fn read(buf: &mut impl Buf) -> io::Result<Self> {
        if buf.remaining() < 1 + 2 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "realm list response too short",
            ));
        }

        let cmd = AuthCommand::try_from(buf.get_u8())?;
        let _size = buf.get_u16_le(); // body size (we don't enforce it here)

        if buf.remaining() < 4 + 1 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "realm list response header truncated",
            ));
        }

        let _padding = buf.get_u32_le(); // always 0
        let num_realms = buf.get_u8() as usize;

        let mut realms = Vec::with_capacity(num_realms);
        for _ in 0..num_realms {
            realms.push(RealmInfo::read(buf)?);
        }

        // 2-byte footer padding
        if buf.remaining() >= 2 {
            let _footer = buf.get_u16_le();
        }

        Ok(Self { cmd, realms })
    }

    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.cmd as u8);

        // Build body into a temporary buffer so we can compute the size field.
        let mut body = BytesMut::new();
        body.put_u32_le(0); // padding
        body.put_u8(self.realms.len() as u8);
        for realm in &self.realms {
            realm.write(&mut body);
        }
        body.put_u16_le(0x0002); // classic footer padding used by CMaNGOS

        buf.put_u16_le(body.len() as u16);
        buf.put_slice(&body);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Read a NUL-terminated C string from a buffer.
fn read_cstring(buf: &mut impl Buf) -> io::Result<String> {
    let mut bytes = Vec::new();
    loop {
        if !buf.has_remaining() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "unterminated C string",
            ));
        }
        let b = buf.get_u8();
        if b == 0 {
            break;
        }
        bytes.push(b);
    }
    String::from_utf8(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Write a NUL-terminated C string into a buffer.
fn write_cstring(buf: &mut impl BufMut, s: &str) {
    buf.put_slice(s.as_bytes());
    buf.put_u8(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logon_proof_response_roundtrip() {
        let resp = LogonProofResponse {
            cmd: AuthCommand::LogonProof,
            error: 0,
            m2: [0xAA; 20],
            login_flags: 0x00800000,
        };

        let mut buf = BytesMut::new();
        resp.write(&mut buf);
        let frozen = buf.freeze();
        let mut cursor = &frozen[..];
        let decoded = LogonProofResponse::read(&mut cursor).unwrap();

        assert_eq!(decoded.cmd, AuthCommand::LogonProof);
        assert_eq!(decoded.error, 0);
        assert_eq!(decoded.m2, [0xAA; 20]);
        assert_eq!(decoded.login_flags, 0x00800000);
    }

    #[test]
    fn realm_list_roundtrip() {
        let resp = RealmListResponse {
            cmd: AuthCommand::RealmList,
            realms: vec![RealmInfo {
                realm_type: 0,
                flags: 0,
                name: "Test Realm".into(),
                address: "127.0.0.1:8085".into(),
                population: 1.0,
                characters: 2,
                timezone: 1,
                realm_id: 1,
            }],
        };

        let mut buf = BytesMut::new();
        resp.write(&mut buf);
        let frozen = buf.freeze();
        let mut cursor = &frozen[..];
        let decoded = RealmListResponse::read(&mut cursor).unwrap();

        assert_eq!(decoded.realms.len(), 1);
        assert_eq!(decoded.realms[0].name, "Test Realm");
        assert_eq!(decoded.realms[0].address, "127.0.0.1:8085");
        assert_eq!(decoded.realms[0].characters, 2);
    }
}
