//! Contains stronlgy-typed states of the [`Dialogue`](super::Dialogue).

#![allow(clippy::non_ascii_literal)]

use async_trait::async_trait;
use derive_more::From;

#[mockall_double::double]
use crate::context::Context;
use crate::{
    bot::{BotTrait, MeGetters, MessageSetters},
    command, message,
};

pub mod authorized;
pub mod unauthorized;

/// Error struct for [`TryFromTransition::try_from_transition()`] function,
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

/// Trait to create state from another *state* `S` using event *B* *by* which transition is possible.
///
/// Will return [`Self::ErrorTarget`] as an error target state if transition failed.
#[async_trait]
pub trait TryFromTransition<S, B>: Sized {
    /// Target state which will be returned on failed transition attempt.
    type ErrorTarget;

    /// Try to peform a transition from `S` to `Self` by `B`.
    ///
    /// Rerturns possibly different state with fail reason if not succeed.
    ///
    /// # Errors
    ///
    /// Fails if failed to perform a transition. Concrete error depends on the implementation.
    async fn try_from_transition(
        from: S,
        by: B,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>>
    where
        B: 'async_trait; // Lifetime from `async_trait` macro expansion
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, From, PartialEq, Eq)]
pub enum State {
    Unauthorized(unauthorized::UnauthorizedBox),
    Authorized(authorized::AuthorizedBox),
}

impl Default for State {
    fn default() -> Self {
        Self::Unauthorized(unauthorized::UnauthorizedBox::default())
    }
}

#[async_trait]
impl TryFromTransition<Self, command::Command> for State {
    type ErrorTarget = Self;

    async fn try_from_transition(
        from: Self,
        cmd: command::Command,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self>> {
        use command::Command;

        if let Command::Help(help) = cmd {
            return Self::try_from_transition(from, help, context).await;
        }

        let unavailable_command =
            |s: Self| FailedTransition::user(s, "Unavailable command in the current state.");

        match from {
            Self::Unauthorized(unauthorized) => {
                use unauthorized::{kind, Unauthorized, UnauthorizedBox};

                match (unauthorized, cmd) {
                    // Default --/start-> Start
                    (UnauthorizedBox::Default(default), Command::Start(start)) => {
                        Unauthorized::<kind::Start>::try_from_transition(default, start, context)
                            .await
                            .map(Into::into)
                            .map_err(FailedTransition::transform)
                    }
                    // Start --/start-> Start
                    (UnauthorizedBox::Start(start), Command::Start(start_cmd)) => {
                        Unauthorized::<kind::Start>::try_from_transition(start, start_cmd, context)
                            .await
                            .map(Into::into)
                            .map_err(FailedTransition::transform)
                    }
                    // WaitingForSecretPhrase --/cancel-> Start
                    (
                        UnauthorizedBox::WaitingForSecretPhrase(waiting_for_secret_phrase),
                        Command::Cancel(cancel),
                    ) => Unauthorized::<kind::Start>::try_from_transition(
                        waiting_for_secret_phrase,
                        cancel,
                        context,
                    )
                    .await
                    .map(Into::into)
                    .map_err(FailedTransition::transform),
                    // Unavailable command
                    (
                        some_unauthorized @ (UnauthorizedBox::Default(_)
                        | UnauthorizedBox::Start(_)
                        | UnauthorizedBox::WaitingForSecretPhrase(_)),
                        _cmd,
                    ) => Err(unavailable_command(some_unauthorized.into())),
                }
            }
            // Unavailable command
            Self::Authorized(authorized) => Err(unavailable_command(authorized.into())),
        }
    }
}

#[async_trait]
impl TryFromTransition<Self, message::Message> for State {
    type ErrorTarget = Self;

    async fn try_from_transition(
        state: Self,
        mes: message::Message,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self>> {
        use authorized::Authorized;
        use message::Message;
        use unauthorized::{Unauthorized, UnauthorizedBox};

        let unexpected_message =
            |s: Self| FailedTransition::user(s, "Unexpected message in the current state.");

        match state {
            Self::Unauthorized(unauthorized) => match (unauthorized, mes) {
                // Start --sign in-> WaitingForSecretPhrase
                (UnauthorizedBox::Start(start), Message::SignIn(sign_in)) => {
                    Unauthorized::<unauthorized::kind::WaitingForSecretPhrase>::try_from_transition(
                        start, sign_in, context,
                    )
                    .await
                    .map(Into::into)
                    .map_err(FailedTransition::transform)
                }
                // WaitingForSecretPhrase --secret phrase-> MainMenu
                (
                    UnauthorizedBox::WaitingForSecretPhrase(waiting_for_secret_prhase),
                    Message::Arbitrary(arbitrary),
                ) => Authorized::<authorized::kind::MainMenu>::try_from_transition(
                    waiting_for_secret_prhase,
                    arbitrary,
                    context,
                )
                .await
                .map(Into::into)
                .map_err(FailedTransition::transform),
                // Text messages are not allowed
                (
                    some_unauthorized @ (UnauthorizedBox::Default(_)
                    | UnauthorizedBox::Start(_)
                    | UnauthorizedBox::WaitingForSecretPhrase(_)),
                    _mes,
                ) => Err(unexpected_message(some_unauthorized.into())),
            },
            // Unexpected message in the current state
            Self::Authorized(authorized) => Err(unexpected_message(authorized.into())),
        }
    }
}

#[async_trait]
impl<T: Into<State> + Send> TryFromTransition<Self, command::Help> for T {
    type ErrorTarget = Self;

    async fn try_from_transition(
        state: T,
        _help: command::Help,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self>> {
        use teloxide::utils::command::BotCommands as _;

        try_with_target!(
            state,
            context
                .bot()
                .send_message(
                    context.chat_id(),
                    command::Command::descriptions().to_string()
                )
                .await
                .map_err(TransitionFailureReason::internal)
        );
        Ok(state)
    }
}
