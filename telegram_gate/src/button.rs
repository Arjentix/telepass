//! Module with all supported buttons.
//!
//! Button means inline button attached to a message.

#![allow(clippy::non_ascii_literal)]

use derive_more::From;
use parse_display::{Display, FromStr};
use teloxide::types::Message;

/// Enum with all possible buttons.
#[derive(Debug, Clone, From)]
#[allow(clippy::module_name_repetitions, clippy::missing_docs_in_private_items)]
pub enum ButtonBox {
    Delete(Button<kind::Delete>),
}

impl ButtonBox {
    /// Create new [`ButtonBox`] from associated [`Message`] and `data`.
    ///
    /// # Errors
    ///
    /// Fails if `data` does not correspond to any valid button [`kind`].
    pub fn new(
        message: Message,
        data: &str,
    ) -> std::result::Result<Self, parse_display::ParseError> {
        use std::str::FromStr as _;

        let kind = kind::Delete::from_str(data)?;
        Ok(Self::Delete(Button { message, kind }))
    }
}

#[derive(Debug, Clone)]
pub struct Button<K> {
    pub message: Message,
    /// Button kind.
    pub kind: K,
}

pub mod kind {
    //! Module with all possible button kinds.

    use super::*;

    /// "Delete" button kind.
    #[derive(Debug, Display, Clone, FromStr)]
    #[display("❌ Delete")]
    pub struct Delete;
}
