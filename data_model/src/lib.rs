//! Crate with Telepass common data structures which are transferred between servicer.

use serde::{Deserialize, Serialize};

/// Data to store a new record.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NewRecord {
    /// Name of the resource.
    pub resource_name: String,
    /// Payload encrypted with master password.
    pub encrypted_payload: String,
    /// Salt applied to the hashed password.
    pub salt: String,
}
