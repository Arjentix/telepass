//! Module with all supported buttons.
//!
//! Button means inline button attached to a message.

#![allow(clippy::non_ascii_literal)]

use derive_more::From;
use parse_display::{Display, FromStr};

use crate::TelegramMessage;

/// Enum with all possible buttons.
#[derive(Debug, Clone, From)]
#[allow(clippy::module_name_repetitions, clippy::missing_docs_in_private_items)]
pub enum ButtonBox {
    Delete(Button<kind::Delete>),
    Yes(Button<kind::Yes>),
    No(Button<kind::No>),
}

impl ButtonBox {
    /// Create new [`ButtonBox`] from associated [`TelegramMessage`] and `data`.
    ///
    /// # Errors
    ///
    /// Fails if `data` does not correspond to any valid button [`kind`].
    #[allow(clippy::map_err_ignore)]
    pub fn new(
        message: TelegramMessage,
        data: &str,
    ) -> std::result::Result<Self, parse_display::ParseError> {
        Button::<kind::Delete>::new(message, data)
            .map(Into::into)
            .or_else(|(_, msg)| Button::<kind::Yes>::new(msg, data).map(Into::into))
            .or_else(|(_, msg)| Button::<kind::No>::new(msg, data).map(Into::into))
            .map_err(|_| parse_display::ParseError::with_message("Unexpected button data"))
    }
}

/// Button type generic over button kind
#[derive(Debug, Clone)]
pub struct Button<K> {
    /// Message button being attached to.
    pub message: TelegramMessage,
    /// Button kind.
    pub kind: K,
}

impl<K: std::str::FromStr<Err = E>, E> Button<K> {
    /// Create new [`Button`] from associated [`TelegramMessage`] and attached `data`.
    ///
    /// # Errors
    ///
    /// Fails if `data` does not correspond to a provided [`kind`].
    fn new(message: TelegramMessage, data: &str) -> Result<Self, (E, TelegramMessage)> {
        match K::from_str(data) {
            Ok(kind) => Ok(Self { message, kind }),
            Err(err) => Err((err, message)),
        }
    }
}

pub mod kind {
    //! Module with all possible button kinds.

    use super::*;

    /// "Delete" button kind.
    #[derive(Debug, Display, Clone, FromStr)]
    #[display("🗑 Delete")]
    pub struct Delete;

    /// "Yes" button kind.
    #[derive(Debug, Display, Clone, FromStr)]
    #[display("✅ Yes")]
    pub struct Yes;

    /// "No" button kind.
    #[derive(Debug, Display, Clone, FromStr)]
    #[display("❌ No")]
    pub struct No;
}
