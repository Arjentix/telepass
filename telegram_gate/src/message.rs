//! This module contains strongly-typed messages user can send.

#![allow(clippy::non_ascii_literal)]

use derive_more::From;
use parse_display::{Display, FromStr};

/// Enum with all possible messages.
#[derive(Debug, Display, Clone, From)]
#[display("{}")]
pub enum Message {
    /// "Sign in" message
    SignIn(SignIn),
    /// "List" message
    List(List),
    /// Any arbitrary message. Parsing will always fallback to this if nothing else matched.
    Arbitrary(Arbitrary),
}

impl std::str::FromStr for Message {
    /// This conversion never fails because of [`Arbitrary`].
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        SignIn::from_str(s)
            .map(Into::into)
            .or_else(|_| List::from_str(s).map(Into::into))
            .or_else(|_| Arbitrary::from_str(s).map(Into::into))
    }
}

/// "Sign in" message.
#[derive(Debug, Display, Clone, FromStr)]
#[display("🔐 Sign in")]
pub struct SignIn;

/// "List" message.
#[derive(Debug, Display, Clone, FromStr)]
#[display("🗒 List")]
pub struct List;

/// Any arbitrary message.
#[derive(Debug, Display, Clone)]
pub struct Arbitrary(pub String);

impl std::str::FromStr for Arbitrary {
    /// This conversion never fails.
    type Err = std::convert::Infallible;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_owned()))
    }
}
