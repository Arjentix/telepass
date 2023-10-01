//! Module with all supported buttons.
//!
//! Button means inline button attached to a message.

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

#[cfg(test)]
#[allow(clippy::multiple_inherent_impl)]
impl ButtonBox {
    #[must_use]
    pub fn delete() -> Self {
        Self::Delete(Button {
            message: TelegramMessage::default(),
            kind: kind::Delete,
        })
    }

    #[must_use]
    pub fn yes() -> Self {
        Self::Yes(Button {
            message: TelegramMessage::default(),
            kind: kind::Yes,
        })
    }

    #[must_use]
    pub fn no() -> Self {
        Self::No(Button {
            message: TelegramMessage::default(),
            kind: kind::No,
        })
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

    #![allow(clippy::non_ascii_literal)]

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

#[cfg(test)]
mod tests {
    #![allow(clippy::non_ascii_literal, clippy::unwrap_used)]

    use super::*;

    #[allow(
        dead_code,
        unreachable_code,
        unused_variables,
        clippy::unimplemented,
        clippy::diverging_sub_expression,
        clippy::panic
    )]
    #[forbid(clippy::todo, clippy::wildcard_enum_match_arm)]
    fn tests_completeness_static_check() -> ! {
        panic!("You should never call this function, it's purpose is the static check only");

        let button: ButtonBox = unimplemented!();

        match button {
            ButtonBox::Delete(_) => parse_delete(),
            ButtonBox::Yes(_) => parse_yes(),
            ButtonBox::No(_) => parse_no(),
        }

        unreachable!()
    }

    #[test]
    fn parse_delete() {
        let message = TelegramMessage::default();
        let data = "🗑 Delete";

        let button = ButtonBox::new(message, data).unwrap();
        assert!(matches!(button, ButtonBox::Delete(_)));
    }

    #[test]
    fn parse_yes() {
        let message = TelegramMessage::default();
        let data = "✅ Yes";

        let button = ButtonBox::new(message, data).unwrap();
        assert!(matches!(button, ButtonBox::Yes(_)));
    }

    #[test]
    fn parse_no() {
        let message = TelegramMessage::default();
        let data = "❌ No";

        let button = ButtonBox::new(message, data).unwrap();
        assert!(matches!(button, ButtonBox::No(_)));
    }
}
