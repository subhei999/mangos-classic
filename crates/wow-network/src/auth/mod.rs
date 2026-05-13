pub mod session;

use std::net::SocketAddr;

use sqlx::mysql::MySqlPool;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{timeout, Duration};
use tracing::{error, info, warn};

use self::session::AuthSession;

const AUTH_SESSION_IO_TIMEOUT: Duration = Duration::from_secs(10);
const AUTH_LOGON_CHALLENGE_MIN_BODY: usize = 30;
const AUTH_LOGON_CHALLENGE_MAX_BODY: usize = 30 + 16;
const AUTH_LOGON_PROOF_SIZE: usize = 1 + 32 + 20 + 20 + 1 + 1;
const AUTH_RECONNECT_PROOF_SIZE: usize = 1 + 16 + 20 + 20 + 1;
const AUTH_REALM_LIST_SIZE: usize = 1 + 4;

// ---------------------------------------------------------------------------
// AuthServer
// ---------------------------------------------------------------------------

/// The authentication / login server.
///
/// Listens for incoming client connections on the configured address and
/// performs the SRP6 login handshake, realm-list exchange, etc.
pub struct AuthServer {
    bind_addr: SocketAddr,
    db_pool: MySqlPool,
}

impl AuthServer {
    /// Create a new `AuthServer` that will bind to `bind_addr` when [`run`](Self::run)
    /// is called.
    pub fn new(bind_addr: SocketAddr, db_pool: MySqlPool) -> Self {
        Self { bind_addr, db_pool }
    }

    /// Bind the TCP listener and run the accept loop.
    ///
    /// Each incoming connection is spawned into its own tokio task. This method
    /// runs indefinitely until the process is shut down.
    pub async fn run(&self) -> anyhow::Result<()> {
        let listener = TcpListener::bind(self.bind_addr).await?;
        info!("Auth server listening on {}", self.bind_addr);

        loop {
            match listener.accept().await {
                Ok((socket, peer)) => {
                    info!(%peer, "Accepted auth connection");
                    let pool = self.db_pool.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_client(socket, pool).await {
                            warn!(%peer, "Auth session ended with error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept auth connection: {}", e);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Per-connection handler
// ---------------------------------------------------------------------------

/// Per-connection handler for auth clients.
///
/// Reads packets from the stream and drives them through the [`AuthSession`]
/// state machine until the client disconnects or an unrecoverable error occurs.
pub async fn handle_client(mut stream: TcpStream, db_pool: MySqlPool) -> anyhow::Result<()> {
    let mut session = AuthSession::new(db_pool);

    loop {
        let data = match timeout(AUTH_SESSION_IO_TIMEOUT, read_auth_frame(&mut stream)).await {
            Ok(Ok(data)) => data,
            Ok(Err(error)) if is_clean_auth_disconnect(&error) => {
                info!("Auth client disconnected");
                return Ok(());
            }
            Ok(Err(error)) => return Err(error),
            Err(_) => anyhow::bail!("auth client timed out waiting for packet"),
        };

        // The first byte of every auth packet is the command opcode.
        let opcode = data[0];
        let response = match opcode {
            // CMD_AUTH_LOGON_CHALLENGE
            0x00 => session.handle_logon_challenge(&data).await,
            // CMD_AUTH_LOGON_PROOF
            0x01 => session.handle_logon_proof(&data).await,
            // CMD_AUTH_RECONNECT_CHALLENGE
            0x02 => session.handle_reconnect_challenge(&data).await,
            // CMD_REALM_LIST
            0x10 => session.handle_realm_list(&data).await,
            other => {
                warn!("Unknown auth opcode: 0x{:02X}", other);
                return Ok(());
            }
        };

        match response {
            Ok(response_bytes) => {
                timeout(AUTH_SESSION_IO_TIMEOUT, stream.write_all(&response_bytes)).await??;
                if session.take_close_after_response() {
                    return Ok(());
                }
            }
            Err(e) => {
                warn!("Error handling auth opcode 0x{:02X}: {}", opcode, e);
                // Send a generic failure response so the client does not hang.
                if opcode == 0x00 {
                    // CMD_AUTH_LOGON_CHALLENGE error: cmd + unk + error_code
                    timeout(
                        AUTH_SESSION_IO_TIMEOUT,
                        stream.write_all(&[0x00, 0x00, 0x04]),
                    )
                    .await??;
                } else if opcode == 0x01 {
                    // CMD_AUTH_LOGON_PROOF error: cmd + error_code + padding
                    timeout(
                        AUTH_SESSION_IO_TIMEOUT,
                        stream.write_all(&[0x01, 0x04, 0x00, 0x00]),
                    )
                    .await??;
                }
                return Err(e);
            }
        }
    }
}

async fn read_auth_frame<R>(stream: &mut R) -> anyhow::Result<Vec<u8>>
where
    R: AsyncRead + Unpin,
{
    let mut opcode = [0u8; 1];
    stream.read_exact(&mut opcode).await?;

    match opcode[0] {
        0x00 | 0x02 => read_auth_challenge_frame(stream, opcode[0]).await,
        0x01 => read_fixed_auth_frame(stream, opcode[0], AUTH_LOGON_PROOF_SIZE).await,
        0x03 => read_fixed_auth_frame(stream, opcode[0], AUTH_RECONNECT_PROOF_SIZE).await,
        0x10 => read_fixed_auth_frame(stream, opcode[0], AUTH_REALM_LIST_SIZE).await,
        other => anyhow::bail!("unknown auth opcode: 0x{other:02X}"),
    }
}

async fn read_auth_challenge_frame<R>(stream: &mut R, opcode: u8) -> anyhow::Result<Vec<u8>>
where
    R: AsyncRead + Unpin,
{
    let mut header_tail = [0u8; 3];
    stream.read_exact(&mut header_tail).await?;
    let body_len = u16::from_le_bytes([header_tail[1], header_tail[2]]) as usize;
    if !(AUTH_LOGON_CHALLENGE_MIN_BODY..=AUTH_LOGON_CHALLENGE_MAX_BODY).contains(&body_len) {
        anyhow::bail!("malformed auth challenge body length {body_len} for opcode 0x{opcode:02X}");
    }

    let mut frame = Vec::with_capacity(1 + header_tail.len() + body_len);
    frame.push(opcode);
    frame.extend_from_slice(&header_tail);
    frame.resize(1 + header_tail.len() + body_len, 0);
    stream.read_exact(&mut frame[4..]).await?;
    Ok(frame)
}

async fn read_fixed_auth_frame<R>(
    stream: &mut R,
    opcode: u8,
    frame_len: usize,
) -> anyhow::Result<Vec<u8>>
where
    R: AsyncRead + Unpin,
{
    debug_assert!(frame_len >= 1);
    let mut frame = vec![0u8; frame_len];
    frame[0] = opcode;
    stream.read_exact(&mut frame[1..]).await?;
    Ok(frame)
}

fn is_clean_auth_disconnect(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<std::io::Error>()
        .is_some_and(|io_error| {
            matches!(
                io_error.kind(),
                std::io::ErrorKind::UnexpectedEof
                    | std::io::ErrorKind::ConnectionAborted
                    | std::io::ErrorKind::ConnectionReset
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{duplex, AsyncWriteExt};

    fn logon_challenge(username: &str) -> Vec<u8> {
        let mut frame = Vec::with_capacity(34 + username.len());
        frame.push(0x00);
        frame.push(0x00);
        frame.extend_from_slice(&(30 + username.len() as u16).to_le_bytes());
        frame.extend_from_slice(b"WoW\0");
        frame.extend_from_slice(&[1, 12, 1]);
        frame.extend_from_slice(&5875u16.to_le_bytes());
        frame.extend_from_slice(b"x86\0");
        frame.extend_from_slice(b"Win\0");
        frame.extend_from_slice(b"enUS");
        frame.extend_from_slice(&0u32.to_le_bytes());
        frame.extend_from_slice(&[127, 0, 0, 1]);
        frame.push(username.len() as u8);
        frame.extend_from_slice(username.as_bytes());
        frame
    }

    #[tokio::test]
    async fn auth_frame_reader_preserves_fragmented_logon_challenge() {
        let request = logon_challenge("RUSTAUTH");
        let (mut client, mut server) = duplex(128);
        let first = request[..7].to_vec();
        let second = request[7..].to_vec();
        tokio::spawn(async move {
            client.write_all(&first).await.unwrap();
            client.write_all(&second).await.unwrap();
        });

        let frame = read_auth_frame(&mut server).await.unwrap();
        assert_eq!(frame, request);
    }

    #[tokio::test]
    async fn auth_frame_reader_leaves_coalesced_packet_for_next_read() {
        let challenge = logon_challenge("RUSTAUTH");
        let realm_list = vec![0x10, 0, 0, 0, 0];
        let (mut client, mut server) = duplex(128);
        client.write_all(&challenge).await.unwrap();
        client.write_all(&realm_list).await.unwrap();

        let first = read_auth_frame(&mut server).await.unwrap();
        let second = read_auth_frame(&mut server).await.unwrap();
        assert_eq!(first, challenge);
        assert_eq!(second, realm_list);
    }

    #[tokio::test]
    async fn auth_frame_reader_rejects_oversized_challenge() {
        let mut request = logon_challenge("RUSTAUTH");
        request[2..4].copy_from_slice(&1024u16.to_le_bytes());
        let (mut client, mut server) = duplex(2048);
        client.write_all(&request).await.unwrap();

        let error = read_auth_frame(&mut server).await.unwrap_err();
        assert!(error.to_string().contains("malformed auth challenge"));
    }
}
