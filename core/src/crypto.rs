//! Optional transport encryption.
//!
//! A shared LAN passphrase is stretched with Argon2id into a 32-byte key. The
//! dialing peer picks a random 16-byte session salt and proves knowledge of the
//! key in `hello`; the acceptor proves it back in `hello_ack`. After that every
//! frame is sent as a `sealed` envelope holding an XChaCha20-Poly1305
//! ciphertext of a complete inner envelope. See `docs/PROTOCOL.md`.

use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    Key, XChaCha20Poly1305, XNonce,
};
use rand::RngCore;
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::protocol::Sealed;

pub const SALT_LEN: usize = 16;
pub const NONCE_LEN: usize = 24;
const HELLO_AAD: &[u8] = b"lanchat/hello-v1";
const FRAME_AAD: &[u8] = b"lanchat/frame-v1";

// ~19 MiB, 2 iterations, 1 lane: interactive-login cost, fine for a LAN
// handshake that happens once per connection.
const ARGON_M_COST: u32 = 19_456;
const ARGON_T_COST: u32 = 2;
const ARGON_P_COST: u32 = 1;

pub fn new_salt() -> [u8; SALT_LEN] {
    let mut salt = [0u8; SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    salt
}

pub fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; 32]> {
    let params = Params::new(ARGON_M_COST, ARGON_T_COST, ARGON_P_COST, Some(32))
        .map_err(|e| CoreError::Crypto(format!("argon2 params: {e}")))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; 32];
    argon
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|e| CoreError::Crypto(format!("argon2: {e}")))?;
    Ok(key)
}

#[derive(Clone)]
pub struct SessionCipher {
    key: [u8; 32],
}

impl std::fmt::Debug for SessionCipher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SessionCipher(redacted)")
    }
}

impl SessionCipher {
    pub fn from_passphrase(passphrase: &str, salt: &[u8]) -> Result<Self> {
        Ok(Self {
            key: derive_key(passphrase, salt)?,
        })
    }

    pub fn from_key(key: [u8; 32]) -> Self {
        Self { key }
    }

    pub fn seal(&self, plaintext: &[u8], aad: &[u8]) -> Result<Sealed> {
        let cipher = XChaCha20Poly1305::new(Key::from_slice(&self.key));
        let mut nonce_bytes = [0u8; NONCE_LEN];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = XNonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(
                nonce,
                Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .map_err(|_| CoreError::Crypto("encrypt failed".into()))?;
        Ok(Sealed {
            nonce_b64: B64.encode(nonce_bytes),
            data_b64: B64.encode(ciphertext),
        })
    }

    pub fn seal_frame(&self, plaintext: &[u8]) -> Result<Sealed> {
        self.seal(plaintext, FRAME_AAD)
    }

    pub fn open_frame(&self, sealed: &Sealed) -> Result<Vec<u8>> {
        self.open(sealed, FRAME_AAD)
    }

    pub fn open(&self, sealed: &Sealed, aad: &[u8]) -> Result<Vec<u8>> {
        let nonce_bytes = B64
            .decode(&sealed.nonce_b64)
            .map_err(|e| CoreError::Crypto(format!("bad nonce: {e}")))?;
        if nonce_bytes.len() != NONCE_LEN {
            return Err(CoreError::Crypto("bad nonce length".into()));
        }
        let ciphertext = B64
            .decode(&sealed.data_b64)
            .map_err(|e| CoreError::Crypto(format!("bad ciphertext: {e}")))?;
        let cipher = XChaCha20Poly1305::new(Key::from_slice(&self.key));
        cipher
            .decrypt(
                XNonce::from_slice(&nonce_bytes),
                Payload {
                    msg: &ciphertext,
                    aad,
                },
            )
            .map_err(|_| CoreError::Crypto("decryption failed (wrong passphrase or tampered frame)".into()))
    }

    /// Dialing peer's proof: binds the identity pair into the handshake.
    pub fn hello_proof(&self, initiator: Uuid, responder: Uuid) -> Result<Sealed> {
        let msg = format!("lanchat-v1:hello:{initiator}:{responder}");
        self.seal(msg.as_bytes(), HELLO_AAD)
    }

    pub fn ack_proof(&self, responder: Uuid, initiator: Uuid) -> Result<Sealed> {
        let msg = format!("lanchat-v1:hello_ack:{responder}:{initiator}");
        self.seal(msg.as_bytes(), HELLO_AAD)
    }

    pub fn verify_hello_proof(
        &self,
        proof: &Sealed,
        initiator: Uuid,
        responder: Uuid,
    ) -> Result<()> {
        let plain = self.open(proof, HELLO_AAD)?;
        let expected = format!("lanchat-v1:hello:{initiator}:{responder}");
        if plain == expected.as_bytes() {
            Ok(())
        } else {
            Err(CoreError::Crypto("hello proof mismatch".into()))
        }
    }

    pub fn verify_ack_proof(&self, proof: &Sealed, responder: Uuid, initiator: Uuid) -> Result<()> {
        let plain = self.open(proof, HELLO_AAD)?;
        let expected = format!("lanchat-v1:hello_ack:{responder}:{initiator}");
        if plain == expected.as_bytes() {
            Ok(())
        } else {
            Err(CoreError::Crypto("hello_ack proof mismatch".into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_and_open_roundtrip() {
        let salt = new_salt();
        let cipher = SessionCipher::from_passphrase("correct horse battery", &salt).unwrap();
        let sealed = cipher.seal_frame(b"hello lan").unwrap();
        assert_eq!(cipher.open_frame(&sealed).unwrap(), b"hello lan");
    }

    #[test]
    fn wrong_passphrase_fails() {
        let salt = new_salt();
        let a = SessionCipher::from_passphrase("passphrase-a", &salt).unwrap();
        let b = SessionCipher::from_passphrase("passphrase-b", &salt).unwrap();
        let sealed = a.seal_frame(b"secret").unwrap();
        assert!(b.open_frame(&sealed).is_err());
    }

    #[test]
    fn tampering_fails() {
        let salt = new_salt();
        let cipher = SessionCipher::from_passphrase("passphrase", &salt).unwrap();
        let mut sealed = cipher.seal_frame(b"secret").unwrap();
        let mut bytes = B64.decode(&sealed.data_b64).unwrap();
        bytes[0] ^= 0xFF;
        sealed.data_b64 = B64.encode(bytes);
        assert!(cipher.open_frame(&sealed).is_err());
    }

    #[test]
    fn handshake_proofs_bind_identity() {
        let salt = new_salt();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let cipher = SessionCipher::from_passphrase("lan-party", &salt).unwrap();
        let proof = cipher.hello_proof(a, b).unwrap();
        assert!(cipher.verify_hello_proof(&proof, a, b).is_ok());
        assert!(cipher.verify_hello_proof(&proof, b, a).is_err());
        let ack = cipher.ack_proof(b, a).unwrap();
        assert!(cipher.verify_ack_proof(&ack, b, a).is_ok());
    }
}
