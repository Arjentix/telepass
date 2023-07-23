//! Contains stronlgy-typed states of the [`Dialogue`](super::Dialogue).

#![allow(clippy::non_ascii_literal)]

use async_trait::async_trait;
use derive_more::From;
use teloxide::requests::Requester as _;

use super::command;
use crate::context::Context;

pub mod authorized;
pub mod unauthorized;

/// Error struct for [`MakeTransition::make_transition()`] function,
/// containing error target and reason of failure.
#[derive(Debug, thiserror::Error)]
#[error("Transition failed")]
pub struct FailedTransition<T> {
    /// Error target of transition.
    pub target: T,
    /// Failure reason.
    #[source]
    pub reason: TransitionFailureReason,
}

#[derive(Debug, thiserror::Error)]
pub enum TransitionFailureReason {
    #[error("User error: {0}")]
    User(String),
    #[error("Internal error")]
    Internal(#[source] color_eyre::Report),
}

impl<T> FailedTransition<T> {
    pub fn user<R: Into<String>>(target: T, reason: R) -> Self {
        Self {
            target,
            reason: TransitionFailureReason::user(reason),
        }
    }

    #[allow(dead_code)] // May be usefull in future
    pub fn internal<E: Into<color_eyre::Report>>(target: T, error: E) -> Self {
        Self {
            target,
            reason: TransitionFailureReason::internal(error),
        }
    }

    pub fn transform<U: From<T>>(self) -> FailedTransition<U> {
        FailedTransition {
            target: self.target.into(),
            reason: self.reason,
        }
    }
}

impl TransitionFailureReason {
    pub fn user<R: Into<String>>(reason: R) -> Self {
        Self::User(reason.into())
    }

    pub fn internal<E: Into<color_eyre::Report>>(error: E) -> Self {
        Self::Internal(error.into())
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
            Err(err) => {
                return Err(FailedTransition {
                    target: $target,
                    reason: err,
                })
            }
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
        context: &Context,
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
        context: &Context,
    ) -> Result<Self, FailedTransition<Self>> {
        if let command::Command::Help(help) = cmd {
            return self.make_transition(help, context).await;
        }

        match self {
            Self::Unauthorized(unauthorized) => unauthorized
                .make_transition(cmd, context)
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
        context: &Context,
    ) -> Result<Self, FailedTransition<Self>> {
        match self {
            Self::Unauthorized(unauthorized) => unauthorized
                .make_transition(text, context)
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
        context: &Context,
    ) -> Result<Self, FailedTransition<Self>> {
        use teloxide::utils::command::BotCommands as _;

        try_with_target!(
            self,
            context
                .bot()
                .send_message(
                    context.chat_id(),
                    command::Command::descriptions().to_string()
                )
                .await
                .map_err(TransitionFailureReason::internal)
        );
        Ok(self)
    }
}
