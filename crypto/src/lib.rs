//! Crate for passwords encryption and decryption used in Telepass.

#![allow(clippy::assertions_on_constants, reason = "health checks")]

use std::string::FromUtf8Error;

#[cfg(feature = "impls")]
use aes_gcm::{
    aead::{Aead, OsRng},
    aes::cipher::Unsigned,
    AeadCore, Aes256Gcm, Key, KeyInit as _, KeySizeUser, Nonce,
};
#[cfg(feature = "impls")]
use pbkdf2::{hmac::digest::OutputSizeUser, pbkdf2_hmac_array};
use serde::{Deserialize, Serialize};
#[cfg(feature = "impls")]
use sha2::Sha256;

/// Size of the salt in bytes.
pub const SALT_SIZE: usize = 12;

/// Health check.
#[cfg(feature = "impls")]
const _: () = assert!(
    <Aes256Gcm as AeadCore>::NonceSize::USIZE == SALT_SIZE,
    "Nonce size is not equal to the salt size"
);

/// Encryption salt.
pub type Salt = [u8; SALT_SIZE];

/// Output of encryption.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EncryptionOutput {
    /// Payload encrypted with a password.
    pub encrypted_payload: Vec<u8>,
    /// Salt used for encryption.
    pub salt: Salt,
}

/// Encryption / decryption error.
#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("Failed to encrypt payload")]
    Encryption,
    #[error("Failed to decrypt payload")]
    Decryption,
    #[error("Failed to parse decrypted payload as UTF-8")]
    Utf8(FromUtf8Error),
}

/// Result of encryption / decryption.
pub type Result<T, E = Error> = core::result::Result<T, E>;

/// Encrypt payload with password.
///
/// Not a pure feature, because it uses random number generator to generate a salt
///
/// # Errors
///
/// Any error from underlying libraries.
#[cfg(feature = "impls")]
pub fn encrypt(payload: &str, password: &str) -> Result<EncryptionOutput> {
    let key = derive_key(password);
    let cipher = Aes256Gcm::new(&key);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    let encrypted_payload = cipher
        .encrypt(&nonce, payload.as_bytes())
        .map_err(|_err| Error::Encryption)?;

    Ok(EncryptionOutput {
        encrypted_payload,
        salt: nonce.into(),
    })
}

/// Decrypt password with password and salt.
///
/// # Errors
///
/// Any error from underlying libraries.
#[cfg(feature = "impls")]
pub fn decrypt(
    EncryptionOutput {
        encrypted_payload,
        salt,
    }: EncryptionOutput,
    password: &str,
) -> Result<String> {
    let key = derive_key(password);
    let cipher = Aes256Gcm::new(&key);
    let nonce = Nonce::from(salt);

    let payload = cipher
        .decrypt(&nonce, encrypted_payload.as_slice())
        .map_err(|_err| Error::Decryption)?;

    String::from_utf8(payload).map_err(Error::Utf8)
}

/// Construct encryption key from string password.
#[cfg(feature = "impls")]
fn derive_key(password: &str) -> Key<Aes256Gcm> {
    /// Salt to be used for key derivation
    const KEY_DERIVATION_SALT: &[u8] = b"telepass_key_derivation_salt";
    /// Number of hashing rounds
    const KEY_DERIVATION_ITERATION_ROUNDS: u32 = 100_000;
    /// Size of the key in bytes
    const KEY_SIZE: usize = <<Aes256Gcm as KeySizeUser>::KeySize as Unsigned>::USIZE;

    /// Health check
    const _: () = assert!(
        KEY_SIZE == <<Sha256 as OutputSizeUser>::OutputSize as Unsigned>::USIZE,
        "AES 256 GCM and SHA 256 key size mismatch"
    );

    let key = pbkdf2_hmac_array::<sha2::Sha256, KEY_SIZE>(
        password.as_bytes(),
        KEY_DERIVATION_SALT,
        KEY_DERIVATION_ITERATION_ROUNDS,
    );
    key.into()
}

#[cfg(test)]
#[cfg(feature = "impls")]
mod tests {
    #![allow(clippy::expect_used, reason = "it's ok in tests")]

    use super::*;

    #[test]
    fn encrypt_and_decrypt_work() {
        let payload = "payload";
        let password = "password";

        let output = encrypt(payload, password).expect("Failed to encrypt payload");
        let decrypted_payload = decrypt(output, password).expect("Failed to decrypt payload");

        assert_eq!(payload, decrypted_payload);
    }

    #[test]
    fn encrypt_same_payload_with_different_passwords_gives_different_results() {
        let payload = "payload";

        let first_output =
            encrypt(payload, "password1").expect("Failed to encrypt payload first time");
        let second_output =
            encrypt(payload, "password2").expect("Failed to encrypt payload second time");

        assert_ne!(
            first_output.encrypted_payload,
            second_output.encrypted_payload
        );
        assert_ne!(first_output.salt, second_output.salt);
    }

    #[test]
    fn encrypt_same_payload_twice_gives_different_results() {
        let payload = "payload";
        let password = "password";

        let first_output =
            encrypt(payload, password).expect("Failed to encrypt payload first time");
        let second_output =
            encrypt(payload, password).expect("Failed to encrypt payload second time");

        assert_ne!(
            first_output.encrypted_payload,
            second_output.encrypted_payload
        );
        assert_ne!(first_output.salt, second_output.salt);
    }

    #[test]
    fn decrypt_with_wrong_password_fails() {
        let payload = "payload";

        let output = encrypt(payload, "password1").expect("Failed to encrypt payload");
        decrypt(output, "password2").expect_err("Decryption is expected to fail");
    }

    #[test]
    #[allow(clippy::indexing_slicing, reason = "it's ok in tests")]
    fn decrypt_with_wrong_payload_fails() {
        let payload = "payload";
        let password = "password";

        let mut output = encrypt(payload, password).expect("Failed to encrypt payload");
        output.encrypted_payload[0] = !output.encrypted_payload[0];
        decrypt(output, password).expect_err("Decryption is expected to fail");
    }

    #[test]
    fn decrypt_with_wrong_salt_fails() {
        let payload = "payload";
        let password = "password";

        let mut output = encrypt(payload, password).expect("Failed to encrypt payload");
        output.salt[0] = !output.salt[0];
        decrypt(output, password).expect_err("Decryption is expected to fail");
    }
}
