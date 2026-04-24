/// Vanilla WoW 1.12.x packet header encryption/decryption.
///
/// This is NOT standard RC4. It is a custom streaming cipher that uses the
/// 40-byte SRP session key directly. After the client successfully
/// authenticates, both sides initialize this cipher and use it to encrypt
/// packet headers (size + opcode) for the rest of the session.
///
/// Server-to-client headers are 4 bytes: 2-byte big-endian size + 2-byte opcode.
/// Client-to-server headers are 6 bytes: 2-byte big-endian size + 4-byte opcode.
#[derive(Clone)]
pub struct HeaderCrypto {
    send_i: u8,
    send_j: u8,
    recv_i: u8,
    recv_j: u8,
    key: [u8; 40],
}

impl HeaderCrypto {
    /// Create a new `HeaderCrypto` from the 40-byte SRP session key.
    pub fn new(session_key: &[u8; 40]) -> Self {
        Self {
            send_i: 0,
            send_j: 0,
            recv_i: 0,
            recv_j: 0,
            key: *session_key,
        }
    }

    /// Encrypt a server-to-client packet header in place.
    ///
    /// `data` is typically 4 bytes: 2-byte big-endian size + 2-byte little-endian opcode.
    pub fn encrypt(&mut self, data: &mut [u8]) {
        let key_len = self.key.len() as u8;
        for byte in data.iter_mut() {
            self.send_i %= key_len;
            *byte = (*byte ^ self.key[self.send_i as usize]).wrapping_add(self.send_j);
            self.send_j = *byte;
            self.send_i = self.send_i.wrapping_add(1);
        }
    }

    /// Decrypt a client-to-server packet header in place.
    ///
    /// `data` is typically 6 bytes: 2-byte big-endian size + 4-byte little-endian opcode.
    pub fn decrypt(&mut self, data: &mut [u8]) {
        let key_len = self.key.len() as u8;
        for byte in data.iter_mut() {
            self.recv_i %= key_len;
            let orig = *byte;
            *byte = (*byte).wrapping_sub(self.recv_j) ^ self.key[self.recv_i as usize];
            self.recv_j = orig;
            self.recv_i = self.recv_i.wrapping_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 40] {
        let mut key = [0u8; 40];
        for (i, b) in key.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(7).wrapping_add(3);
        }
        key
    }

    #[test]
    fn encrypt_decrypt_round_trip() {
        // Simulate a server sending an encrypted header and a client decrypting it.
        let key = test_key();
        let mut server = HeaderCrypto::new(&key);
        let mut client = HeaderCrypto::new(&key);

        // Server-to-client: 4-byte header (size=0x0012, opcode=0x01EE).
        let original: [u8; 4] = [0x00, 0x12, 0xEE, 0x01];
        let mut data = original;

        server.encrypt(&mut data);
        // After encryption the bytes should differ from the original.
        assert_ne!(data, original);

        // The client decrypts using the same cipher state.
        // Note: decrypt uses recv_i/recv_j, which mirrors the server's send_i/send_j.
        client.decrypt(&mut data);
        assert_eq!(data, original);
    }

    #[test]
    fn decrypt_encrypt_round_trip() {
        // Simulate a client sending an encrypted header and a server decrypting it.
        let key = test_key();
        let mut client = HeaderCrypto::new(&key);
        let mut server = HeaderCrypto::new(&key);

        // Client-to-server: 6-byte header (size=0x0004, opcode=0x000001ED).
        let original: [u8; 6] = [0x00, 0x04, 0xED, 0x01, 0x00, 0x00];
        let mut data = original;

        // Client encrypts with its send (encrypt) path.
        client.encrypt(&mut data);
        assert_ne!(data, original);

        // Server decrypts with its recv (decrypt) path.
        server.decrypt(&mut data);
        assert_eq!(data, original);
    }

    #[test]
    fn multiple_packets_maintain_state() {
        let key = test_key();
        let mut encryptor = HeaderCrypto::new(&key);
        let mut decryptor = HeaderCrypto::new(&key);

        let packets: [[u8; 4]; 3] = [
            [0x00, 0x0A, 0x3C, 0x00],
            [0x00, 0x20, 0x1E, 0x01],
            [0x01, 0x00, 0xEE, 0x01],
        ];

        for original in &packets {
            let mut data = *original;
            encryptor.encrypt(&mut data);
            decryptor.decrypt(&mut data);
            assert_eq!(data, *original);
        }
    }

    #[test]
    fn encrypt_produces_expected_bytes() {
        // Deterministic test: given a known key and input, verify output.
        let key = [0xABu8; 40];
        let mut crypto = HeaderCrypto::new(&key);

        let mut data: [u8; 4] = [0x00, 0x12, 0xEE, 0x01];
        crypto.encrypt(&mut data);

        // Manual calculation for key = [0xAB; 40]:
        //   byte 0: (0x00 ^ 0xAB).wrapping_add(0x00) = 0xAB
        //   byte 1: (0x12 ^ 0xAB).wrapping_add(0xAB) = 0xB9 + 0xAB = 0x64
        //   byte 2: (0xEE ^ 0xAB).wrapping_add(0x64) = 0x45 + 0x64 = 0xA9
        //   byte 3: (0x01 ^ 0xAB).wrapping_add(0xA9) = 0xAA + 0xA9 = 0x53
        assert_eq!(data, [0xAB, 0x64, 0xA9, 0x53]);
    }

    #[test]
    fn key_index_wraps_around() {
        // Encrypt more than 40 bytes to ensure send_i wraps past key length.
        let key = test_key();
        let mut crypto = HeaderCrypto::new(&key);

        let mut data = [0u8; 50];
        // Should not panic even though we exceed the key length.
        crypto.encrypt(&mut data);
    }

    #[test]
    fn zero_length_data_is_noop() {
        let key = test_key();
        let mut crypto = HeaderCrypto::new(&key);
        let mut data: [u8; 0] = [];
        crypto.encrypt(&mut data);
        crypto.decrypt(&mut data);
        // State should remain at initial values.
        assert_eq!(crypto.send_i, 0);
        assert_eq!(crypto.recv_i, 0);
    }
}
