use wow_srp::normalized_string::NormalizedString;
use wow_srp::server::{SrpProof, SrpServer, SrpVerifier};
use wow_srp::{PublicKey, GENERATOR, LARGE_SAFE_PRIME_LENGTH, PROOF_LENGTH, SESSION_KEY_LENGTH};

/// Errors that can occur during SRP authentication.
#[derive(Debug, thiserror::Error)]
pub enum SrpError {
    #[error("invalid username or password string")]
    InvalidString,
    #[error("client proof verification failed")]
    ProofFailed,
    #[error("invalid public key from client")]
    InvalidPublicKey,
}

/// Manages the SRP6 authentication flow for a single client session.
///
/// The flow is:
///   1. Server creates a verifier from the stored username + password (or loads
///      existing salt/verifier from the database).
///   2. Server generates a challenge and sends `B`, `g`, `N`, `s` to the client.
///   3. Client sends back its proof (`A`, `M1`). Server verifies and produces
///      `M2` plus the session key.
pub struct SrpAuth {
    proof: SrpProof,
}

/// The server challenge values sent to the client in AUTH_LOGON_CHALLENGE.
pub struct ServerChallenge {
    /// Server public key (B), 32 bytes.
    pub server_public_key: [u8; LARGE_SAFE_PRIME_LENGTH as usize],
    /// Generator (g), always 7 for WoW.
    pub generator: u8,
    /// Large safe prime (N), 32 bytes.
    pub large_safe_prime: [u8; LARGE_SAFE_PRIME_LENGTH as usize],
    /// Salt (s), 32 bytes.
    pub salt: [u8; LARGE_SAFE_PRIME_LENGTH as usize],
}

/// The result of a successful client proof verification.
pub struct AuthResult {
    /// Server proof (M2) to send back to the client, 20 bytes.
    pub server_proof: [u8; PROOF_LENGTH as usize],
    /// The 40-byte session key used for header encryption.
    pub session_key: [u8; SESSION_KEY_LENGTH as usize],
}

impl SrpAuth {
    /// Create a fresh SRP verifier from a username and password, then
    /// immediately generate the server challenge.
    pub fn from_username_password(username: &str, password: &str) -> Result<Self, SrpError> {
        let username = NormalizedString::new(username).map_err(|_| SrpError::InvalidString)?;
        let password = NormalizedString::new(password).map_err(|_| SrpError::InvalidString)?;

        let verifier = SrpVerifier::from_username_and_password(username, password);
        let proof = verifier.into_proof();

        Ok(Self { proof })
    }

    /// Create an `SrpAuth` from a previously stored salt and password verifier.
    pub fn from_database_values(
        username: &str,
        password_verifier: [u8; LARGE_SAFE_PRIME_LENGTH as usize],
        salt: [u8; LARGE_SAFE_PRIME_LENGTH as usize],
    ) -> Result<Self, SrpError> {
        let username = NormalizedString::new(username).map_err(|_| SrpError::InvalidString)?;
        let verifier = SrpVerifier::from_database_values(username, password_verifier, salt);
        let proof = verifier.into_proof();

        Ok(Self { proof })
    }

    /// Returns the challenge values to send to the client.
    pub fn server_challenge(&self) -> ServerChallenge {
        ServerChallenge {
            server_public_key: *self.proof.server_public_key(),
            generator: GENERATOR,
            large_safe_prime: wow_srp::LARGE_SAFE_PRIME_LITTLE_ENDIAN,
            salt: *self.proof.salt(),
        }
    }

    /// Verify the client's proof and, on success, return the server proof (M2)
    /// and the session key.
    pub fn verify_client_proof(
        self,
        client_public_key: [u8; LARGE_SAFE_PRIME_LENGTH as usize],
        client_proof: [u8; PROOF_LENGTH as usize],
    ) -> Result<(AuthResult, SrpServer), SrpError> {
        let client_public_key =
            PublicKey::from_le_bytes(client_public_key).map_err(|_| SrpError::InvalidPublicKey)?;

        let (server, server_proof) = self
            .proof
            .into_server(client_public_key, client_proof)
            .map_err(|_| SrpError::ProofFailed)?;

        let result = AuthResult {
            server_proof,
            session_key: *server.session_key(),
        };

        Ok((result, server))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_auth_flow() {
        let auth = SrpAuth::from_username_password("TESTUSER", "TESTPASS").unwrap();
        let challenge = auth.server_challenge();

        assert_eq!(challenge.generator, 7);
        assert_eq!(
            challenge.large_safe_prime.len(),
            LARGE_SAFE_PRIME_LENGTH as usize
        );
    }

    #[test]
    fn invalid_username_rejected() {
        let result = SrpAuth::from_username_password("", "TESTPASS");
        assert!(result.is_err());
    }
}
