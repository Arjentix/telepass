//! This module contains strongly-typed messages user can send.

use derive_more::From;
use parse_display::{Display, FromStr};

use crate::TelegramMessage;

/// Enum with all possible messages.
#[derive(Debug, Display, Clone, From)]
#[display("{}")]
#[allow(clippy::module_name_repetitions)]
pub enum MessageBox {
    /// "Sign in" message.
    SignIn(Message<kind::SignIn>),
    /// "List" message.
    List(Message<kind::List>),
    /// Any arbitrary message. Parsing will always fallback to this if nothing else matched.
    Arbitrary(Message<kind::Arbitrary>),
}

impl MessageBox {
    pub fn new(inner: TelegramMessage) -> Self {
        Message::<kind::SignIn>::new(inner)
            .map(Into::into)
            .or_else(|(_, msg)| Message::<kind::List>::new(msg).map(Into::into))
            .or_else(|(_, msg)| Message::<kind::Arbitrary>::new(msg).map(Into::into))
            .unwrap_or_else(|_: (std::convert::Infallible, _)| unreachable!())
    }
}

#[cfg(test)]
#[allow(clippy::multiple_inherent_impl)]
impl MessageBox {
    #[must_use]
    pub fn sign_in() -> Self {
        Self::SignIn(Message {
            inner: TelegramMessage::default(),
            kind: kind::SignIn,
        })
    }

    #[must_use]
    pub fn list() -> Self {
        Self::List(Message {
            inner: TelegramMessage::default(),
            kind: kind::List,
        })
    }

    #[must_use]
    pub fn arbitrary(text: &'static str) -> Self {
        let mut mock_inner = TelegramMessage::default();
        mock_inner.expect_text().return_const(text);
        Self::Arbitrary(Message {
            inner: mock_inner,
            kind: kind::Arbitrary,
        })
    }
}

/// Message struct generic over message kind.
#[derive(Debug, Clone)]
pub struct Message<K> {
    /// Original Telegram message.
    pub inner: TelegramMessage,
    /// Message kind.
    pub kind: K,
}

impl<K> std::fmt::Display for Message<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = self.inner.text().unwrap_or_default();
        write!(f, "{text}")
    }
}

impl<K: std::str::FromStr<Err = E>, E> Message<K> {
    /// Create new [`Message`] from associated [`TelegramMessage`].
    ///
    /// # Errors
    ///
    /// Fails if message does not correspond to a provided [`kind`].
    fn new(inner: TelegramMessage) -> std::result::Result<Self, (E, TelegramMessage)> {
        match K::from_str(inner.text().unwrap_or_default()) {
            Ok(kind) => Ok(Self { inner, kind }),
            Err(err) => Err((err, inner)),
        }
    }
}

pub mod kind {
    //! Module with all possible [`Message`] kinds.

    #![allow(clippy::non_ascii_literal)]

    use super::*;

    /// "Sign in" message.
    #[derive(Debug, Display, Clone, FromStr)]
    #[display("🔐 Sign in")]
    pub struct SignIn;

    /// "List" message.
    #[derive(Debug, Display, Clone, FromStr)]
    #[display("🗒 List")]
    pub struct List;

    /// Any arbitrary message.
    #[derive(Debug, Clone)]
    pub struct Arbitrary;

    impl std::str::FromStr for Arbitrary {
        type Err = std::convert::Infallible;

        #[inline]
        fn from_str(_s: &str) -> Result<Self, Self::Err> {
            Ok(Self)
        }
    }
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

        let message: MessageBox = unimplemented!();

        match message {
            MessageBox::SignIn(_) => parse_sign_in(),
            MessageBox::List(_) => parse_list(),
            MessageBox::Arbitrary(_) => parse_arbitrary(),
        }

        unreachable!()
    }

    #[test]
    fn parse_sign_in() {
        let mut tg_message = TelegramMessage::default();
        tg_message.expect_text().return_const("🔐 Sign in");

        let message = MessageBox::new(tg_message);
        assert!(matches!(message, MessageBox::SignIn(_)));
    }

    #[test]
    fn parse_list() {
        let mut tg_message = TelegramMessage::default();
        tg_message.expect_text().return_const("🗒 List");

        let message = MessageBox::new(tg_message);
        assert!(matches!(message, MessageBox::List(_)));
    }

    #[test]
    fn parse_arbitrary() {
        let mut tg_message = TelegramMessage::default();
        tg_message
            .expect_text()
            .return_const("Any random string here");

        let message = MessageBox::new(tg_message);
        assert!(matches!(message, MessageBox::Arbitrary(_)));
    }
}
