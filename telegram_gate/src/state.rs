//! Contains stronlgy-typed states of the [`Dialogue`](super::Dialogue).

#![allow(clippy::non_ascii_literal)]

use async_trait::async_trait;
use derive_more::From;
use teloxide::{requests::Requester as _, types::ChatId, Bot};

use super::command;

mod authorized;
mod unauthorized;

/// Error struct for [`MakeTransition::make_transition()`] function,
/// containing error target and reason of failure.
#[derive(Debug, thiserror::Error)]
#[error("Transition failed")]
pub struct FailedTransition<T> {
    /// Error target of transition.
    pub target: T,
    /// Failure reason.
    #[source]
    pub reason: eyre::Report,
}

impl<T> FailedTransition<T> {
    pub fn from_err<E: Into<eyre::Report>>(target: T, error: E) -> Self {
        Self {
            target,
            reason: error.into(),
        }
    }

    pub fn transform<U: From<T>>(self) -> FailedTransition<U> {
        FailedTransition {
            target: self.target.into(),
            reason: self.reason,
        }
    }
}

/// Macro which works similar to [`try!`], but packs errors into
/// [`FailedTransition`] with provided `target`.
///
/// It's needed because basic `?` operator triggers `use of moved value` due to
/// lack of control flow understanding.
macro_rules! try_with_target {
    ($target:ident, $e:expr) => {{
        let value = $e;
        match value {
            Ok(ok) => ok,
            Err(err) => return Err(FailedTransition::from_err($target, err)),
        }
    }};
}

pub(crate) use try_with_target;

/// Trait to make a transition from one state to another.
///
/// # Generics
///
/// - `T` - means the end *target* state of successfull transition.
/// - `B` - means the event *by* which transition is possible.
///
/// Transition will return [`Self::ErrorTarget`] as an error target if transition failed.
#[async_trait]
pub trait MakeTransition<T, B> {
    /// Target which will be returned on failed transition attempt.
    type ErrorTarget;

    /// Try to perfrom a transition from [`Self`] to `T`.
    ///
    /// Rerturns possibly different state with fail reason if not succeed.
    ///
    /// # Errors
    ///
    /// Fails if failed to perform a transition. Concrete error depends on the implementation.
    async fn make_transition(
        self,
        by: B,
        bot: Bot,
        chat_id: ChatId,
    ) -> Result<T, FailedTransition<Self::ErrorTarget>>
    where
        B: 'async_trait; // Lifetime from `async_trait` macro expansion
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, From)]
pub enum State {
    Unauthorized(unauthorized::Unauthorized<unauthorized::kind::Kind>),
    Authorized(authorized::Authorized<authorized::kind::Kind>),
}

impl Default for State {
    fn default() -> Self {
        Self::Unauthorized(unauthorized::Unauthorized::default())
    }
}

#[async_trait]
impl MakeTransition<Self, command::Command> for State {
    type ErrorTarget = Self;

    async fn make_transition(
        self,
        cmd: command::Command,
        bot: Bot,
        chat_id: ChatId,
    ) -> Result<Self, FailedTransition<Self>> {
        if let command::Command::Help(help) = cmd {
            return self.make_transition(help, bot, chat_id).await;
        }

        match self {
            Self::Unauthorized(unauthorized) => unauthorized
                .make_transition(cmd, bot, chat_id)
                .await
                .map_err(FailedTransition::transform),
            Self::Authorized(_) => todo!(),
        }
    }
}

#[async_trait]
impl<'mes> MakeTransition<Self, &'mes str> for State {
    type ErrorTarget = Self;

    async fn make_transition(
        self,
        text: &'mes str,
        bot: Bot,
        chat_id: ChatId,
    ) -> Result<Self, FailedTransition<Self>> {
        match self {
            Self::Unauthorized(unauthorized) => unauthorized
                .make_transition(text, bot, chat_id)
                .await
                .map_err(FailedTransition::transform),
            Self::Authorized(_) => todo!(),
        }
    }
}

#[async_trait]
impl<T: Into<State> + Send> MakeTransition<Self, command::Help> for T {
    type ErrorTarget = Self;

    async fn make_transition(
        self,
        _help: command::Help,
        bot: Bot,
        chat_id: ChatId,
    ) -> Result<Self, FailedTransition<Self>> {
        use teloxide::utils::command::BotCommands as _;

        try_with_target!(
            self,
            bot.send_message(chat_id, command::Command::descriptions().to_string())
                .await
        );
        Ok(self)
    }
}
