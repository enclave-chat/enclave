use chacha20poly1305::{aead::Aead, ChaCha20Poly1305, Nonce};

pub struct SessionCipher {
    cipher: ChaCha20Poly1305,
    send_counter: u64,
    recv_counter: u64,
}

impl SessionCipher {
    pub fn new(cipher: ChaCha20Poly1305) -> Self {
        Self {
            cipher,
            send_counter: 0,
            recv_counter: 0,
        }
    }

    pub fn next_send_nonce(&mut self) -> [u8; 12] {
        let mut nonce = [0u8; 12];
        nonce[..8].copy_from_slice(&self.send_counter.to_be_bytes());
        // top bit distinguishes "send" direction from "recv" direction,
        // so client-send and server-send counters never collide even if
        // both happened to reach the same numeric value
        nonce[11] |= 0b1000_0000;
        self.send_counter += 1;
        nonce
    }

    pub fn next_recv_nonce(&mut self) -> [u8; 12] {
        let mut nonce = [0u8; 12];
        nonce[..8].copy_from_slice(&self.recv_counter.to_be_bytes());
        self.recv_counter += 1;
        nonce
    }

    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, String> {
        let nonce_bytes = self.next_send_nonce();
        let nonce = Nonce::try_from(nonce_bytes).map_err(|v| v.to_string())?;

        let ciphertext = self
            .cipher
            .encrypt(&nonce, plaintext)
            .map_err(|v| v.to_string())?;

        // prepend the nonce so the other side can reconstruct it on decrypt
        let mut out = nonce_bytes.to_vec();
        out.extend(ciphertext);
        Ok(out)
    }

    pub fn decrypt(&mut self, data: &[u8]) -> Result<Vec<u8>, String> {
        if data.len() < 12 {
            return Err("message too short to contain a nonce".to_string());
        }
        let (nonce_bytes, ciphertext) = data.split_at(12);
        let nonce = Nonce::try_from(nonce_bytes).map_err(|v| v.to_string())?;

        self.cipher
            .decrypt(&nonce, ciphertext)
            .map_err(|v| v.to_string())
    }
}
