//! Crate with Telepass common data structures which are transferred between servicer.

use serde::{Deserialize, Serialize};
pub use telepass_crypto as crypto;

/// Data to store a new record.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NewRecord {
    /// Name of the resource.
    pub resource_name: String,
    pub encryption_output: crypto::EncryptionOutput,
}
