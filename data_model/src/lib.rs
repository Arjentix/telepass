//! Crate with Telepass common data structures which are transferred between servicer.

use serde::{Deserialize, Serialize};

/// Data to store a new password.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NewPassword {
    /// Name of the resource.
    pub resource: String,
    /// Password encrypted with master password.
    pub encrypted_password: String,
    /// Salt applied to the hashed password.
    pub salt: String,
}
