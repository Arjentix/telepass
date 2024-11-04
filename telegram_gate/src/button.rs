//! Module with all supported buttons.
//!
//! Button means inline button attached to a message.

use derive_more::From;
use parse_display::{Display, FromStr};

use crate::TelegramMessage;

/// Enum with all possible buttons.
#[derive(Debug, Clone, From)]
pub enum ButtonBox {
    Delete(Button<kind::Delete>),
    Yes(Button<kind::Yes>),
    No(Button<kind::No>),
    Show(Button<kind::Show>),
}

impl ButtonBox {
    /// Create new [`ButtonBox`] from associated [`TelegramMessage`] and `data`.
    ///
    /// # Errors
    ///
    /// Fails if `data` does not correspond to any valid button [`kind`].
    #[expect(clippy::map_err_ignore, reason = "not interested in exact parse error")]
    pub fn new(
        message: TelegramMessage,
        data: &str,
    ) -> std::result::Result<Self, parse_display::ParseError> {
        Button::<kind::Delete>::new(message, data)
            .map(Into::into)
            .or_else(|(_, msg)| Button::<kind::Yes>::new(msg, data).map(Into::into))
            .or_else(|(_, msg)| Button::<kind::No>::new(msg, data).map(Into::into))
            .or_else(|(_, msg)| Button::<kind::Show>::new(msg, data).map(Into::into))
            .map_err(|_| parse_display::ParseError::with_message("Unexpected button data"))
    }
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::allow_attributes, reason = "false positive"))]
#[cfg_attr(
    test,
    allow(
        clippy::multiple_inherent_impl,
        reason = "better looking conditional compilation"
    )
)]
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

    #[must_use]
    pub fn show() -> Self {
        Self::Show(Button {
            message: TelegramMessage::default(),
            kind: kind::Show,
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

    /// "Show" button kind.
    #[derive(Debug, Display, Clone, FromStr)]
    #[display("👀 Show")]
    pub struct Show;
}

#[cfg(test)]
mod tests {
    #![expect(clippy::unwrap_used, reason = "it's ok in tests")]
    #![expect(clippy::non_ascii_literal, reason = "emojis are allowed")]

    use super::*;

    #[expect(
        dead_code,
        unreachable_code,
        unused_variables,
        clippy::unimplemented,
        clippy::diverging_sub_expression,
        clippy::panic,
        reason = "not needed as it's a static check"
    )]
    #[forbid(clippy::todo, clippy::wildcard_enum_match_arm)]
    fn tests_completeness_static_check() -> ! {
        panic!("You should never call this function, it's purpose is the static check only");

        let button: ButtonBox = unimplemented!();

        match button {
            ButtonBox::Delete(_) => parse_delete(),
            ButtonBox::Yes(_) => parse_yes(),
            ButtonBox::No(_) => parse_no(),
            ButtonBox::Show(_) => parse_show(),
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

    #[test]
    fn parse_show() {
        let message = TelegramMessage::default();
        let data = "👀 Show";

        let button = ButtonBox::new(message, data).unwrap();
        assert!(matches!(button, ButtonBox::Show(_)));
    }
}
