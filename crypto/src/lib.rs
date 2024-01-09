//! Crate for passwords encryption and decryption used in Telepass.

/// Output of encryption.
pub struct EncryptionOutput {
    /// Password encrypted with a master password.
    pub encrypted_password: String,
    /// Salt used for encryption.
    pub salt: String,
}

/// Encryption / decryption error.
#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {}

/// Result of encryption / decryption.
pub type Result<T, E = Error> = core::result::Result<T, E>;

/// Encrypt password with master password.
///
/// Not a pure feature, because it uses random number generator to generate a salt
///
/// # Errors
///
/// Any error from underlying libraries.
pub fn encrypt(_password: &str, _master_password: &str) -> Result<EncryptionOutput> {
    // TODO
    Ok(EncryptionOutput {
        encrypted_password: String::new(),
        salt: String::new(),
    })
}

/// Decrypt password with master password and salt.
///
/// # Errors
///
/// Any error from underlying libraries.
#[allow(clippy::missing_const_for_fn)] // TODO: remove when implemented
pub fn decrypt(_encrypted_password: &str, _master_password: &str, _salt: &str) -> Result<String> {
    // TODO
    Ok(String::new())
}
