//! Client-side encryption for the sync vault.
//!
//! Everything in here runs on the client only. The sync server never sees the
//! passphrase, the derived key, or any plaintext: it stores opaque blobs keyed
//! by a vault id that is itself derived from the passphrase.
//!
//! Scheme (v1):
//! - master = Argon2id(passphrase, salt = SHA-256("gpui-todo/e2ee/v1/" || vault_name))
//! - enc_key  = HKDF-SHA256(master, info = "gpui-todo/e2ee/v1/enc-key")   (32 bytes)
//! - vault_id = HKDF-SHA256(master, info = "gpui-todo/e2ee/v1/vault-id")  (16 bytes, hex)
//! - blob = 0x01 || nonce (24 bytes, random) || XChaCha20-Poly1305(enc_key, nonce, plaintext, aad)
//!   where aad = "gpui-todo/e2ee/v1/" || vault_id, so a blob cannot be moved to another vault.

use argon2::Argon2;
use chacha20poly1305::{
    KeyInit, XChaCha20Poly1305, XNonce,
    aead::{Aead, Payload},
};
use hkdf::Hkdf;
use sha2::{Digest, Sha256};

const FORMAT_VERSION: u8 = 1;
const NONCE_LEN: usize = 24;
const DOMAIN: &str = "gpui-todo/e2ee/v1/";

#[derive(Debug)]
pub enum CryptoError {
    Malformed,
    UnsupportedVersion(u8),
    /// Wrong key, or the blob was tampered with.
    Decrypt,
}

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CryptoError::Malformed => write!(f, "malformed ciphertext"),
            CryptoError::UnsupportedVersion(v) => write!(f, "unsupported format version {v}"),
            CryptoError::Decrypt => write!(f, "decryption failed (wrong key or tampered data)"),
        }
    }
}

impl std::error::Error for CryptoError {}

pub struct VaultKey {
    cipher: XChaCha20Poly1305,
    vault_id: String,
}

impl VaultKey {
    /// Derives the encryption key and vault id from a vault name and passphrase.
    /// Argon2id is deliberately slow (tens to hundreds of milliseconds).
    pub fn derive(vault_name: &str, passphrase: &str) -> Self {
        let salt = Sha256::digest(format!("{DOMAIN}{vault_name}").as_bytes());
        let mut master = [0u8; 32];
        Argon2::default()
            .hash_password_into(passphrase.as_bytes(), &salt, &mut master)
            .expect("argon2 parameters are valid");

        let hk = Hkdf::<Sha256>::new(None, &master);
        let mut enc_key = [0u8; 32];
        hk.expand(format!("{DOMAIN}enc-key").as_bytes(), &mut enc_key)
            .expect("32 bytes is a valid HKDF output length");
        let mut id = [0u8; 16];
        hk.expand(format!("{DOMAIN}vault-id").as_bytes(), &mut id)
            .expect("16 bytes is a valid HKDF output length");

        let cipher = XChaCha20Poly1305::new_from_slice(&enc_key).expect("key is 32 bytes");
        master.fill(0);
        enc_key.fill(0);

        Self {
            cipher,
            vault_id: to_hex(&id),
        }
    }

    pub fn vault_id(&self) -> &str {
        &self.vault_id
    }

    fn aad(&self) -> Vec<u8> {
        format!("{DOMAIN}{}", self.vault_id).into_bytes()
    }

    pub fn seal(&self, plaintext: &[u8]) -> Vec<u8> {
        let nonce: [u8; NONCE_LEN] = rand::random();
        let aad = self.aad();
        let ciphertext = self
            .cipher
            .encrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: plaintext,
                    aad: &aad,
                },
            )
            .expect("encryption with a valid key cannot fail");

        let mut blob = Vec::with_capacity(1 + NONCE_LEN + ciphertext.len());
        blob.push(FORMAT_VERSION);
        blob.extend_from_slice(&nonce);
        blob.extend_from_slice(&ciphertext);
        blob
    }

    pub fn open(&self, blob: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let (&version, rest) = blob.split_first().ok_or(CryptoError::Malformed)?;
        if version != FORMAT_VERSION {
            return Err(CryptoError::UnsupportedVersion(version));
        }
        if rest.len() < NONCE_LEN {
            return Err(CryptoError::Malformed);
        }
        let (nonce, ciphertext) = rest.split_at(NONCE_LEN);
        let nonce: [u8; NONCE_LEN] = nonce.try_into().map_err(|_| CryptoError::Malformed)?;
        let aad = self.aad();
        self.cipher
            .decrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: ciphertext,
                    aad: &aad,
                },
            )
            .map_err(|_| CryptoError::Decrypt)
    }
}

pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let key = VaultKey::derive("default", "correct horse battery staple");
        let blob = key.seal(b"hello");
        assert_ne!(&blob[1 + NONCE_LEN..], b"hello");
        assert_eq!(key.open(&blob).unwrap(), b"hello");
    }

    #[test]
    fn same_inputs_give_same_vault() {
        let a = VaultKey::derive("default", "pw");
        let b = VaultKey::derive("default", "pw");
        assert_eq!(a.vault_id(), b.vault_id());
        assert_eq!(b.open(&a.seal(b"x")).unwrap(), b"x");
    }

    #[test]
    fn nonce_is_random() {
        let key = VaultKey::derive("default", "pw");
        assert_ne!(key.seal(b"same"), key.seal(b"same"));
    }

    #[test]
    fn wrong_passphrase_cannot_decrypt() {
        let a = VaultKey::derive("default", "pw");
        let b = VaultKey::derive("default", "other");
        assert_ne!(a.vault_id(), b.vault_id());
        assert!(matches!(b.open(&a.seal(b"x")), Err(CryptoError::Decrypt)));
    }

    #[test]
    fn tampering_is_detected() {
        let key = VaultKey::derive("default", "pw");
        let mut blob = key.seal(b"hello");
        *blob.last_mut().unwrap() ^= 1;
        assert!(matches!(key.open(&blob), Err(CryptoError::Decrypt)));
    }
}
