pub mod session;

use std::net::SocketAddr;

use sqlx::mysql::MySqlPool;
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info, warn};

use self::session::AuthSession;

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
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut session = AuthSession::new(db_pool);
    let mut buf = vec![0u8; 4096];

    loop {
        let n = stream.read(&mut buf).await?;
        if n == 0 {
            info!("Auth client disconnected");
            return Ok(());
        }

        let data = &buf[..n];
        if data.is_empty() {
            continue;
        }

        // The first byte of every auth packet is the command opcode.
        let opcode = data[0];
        let response = match opcode {
            // CMD_AUTH_LOGON_CHALLENGE
            0x00 => session.handle_logon_challenge(data).await,
            // CMD_AUTH_LOGON_PROOF
            0x01 => session.handle_logon_proof(data).await,
            // CMD_REALM_LIST
            0x10 => session.handle_realm_list(data).await,
            other => {
                warn!("Unknown auth opcode: 0x{:02X}", other);
                continue;
            }
        };

        match response {
            Ok(response_bytes) => {
                stream.write_all(&response_bytes).await?;
            }
            Err(e) => {
                warn!("Error handling auth opcode 0x{:02X}: {}", opcode, e);
                // Send a generic failure response so the client does not hang.
                if opcode == 0x00 {
                    // CMD_AUTH_LOGON_CHALLENGE error: cmd + unk + error_code
                    stream.write_all(&[0x00, 0x00, 0x04]).await?;
                } else if opcode == 0x01 {
                    // CMD_AUTH_LOGON_PROOF error: cmd + error_code + padding
                    stream.write_all(&[0x01, 0x04, 0x00, 0x00]).await?;
                }
                return Err(e);
            }
        }
    }
}
