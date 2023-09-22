//! Contains strongly-typed states of the [`Dialogue`](super::Dialogue).

#![allow(clippy::non_ascii_literal)]

use async_trait::async_trait;
use derive_more::From;

#[mockall_double::double]
use crate::context::Context;
use crate::{button, command, message, IdExt, TelegramMessage, UserExt};
#[cfg(not(test))]
use crate::{EditMessageReplyMarkupSetters, EditMessageTextSetters, Requester, SendMessageSetters};

pub mod authorized;
mod test_utils;
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

/// Reason of failed transition.
#[derive(Debug, thiserror::Error)]
pub enum TransitionFailureReason {
    /// User mistake.
    #[error("User error: {0}")]
    User(String),
    /// Internal error occurred.
    #[error("Internal error")]
    Internal(#[source] color_eyre::Report),
}

impl<T> FailedTransition<T> {
    /// Create new [`FailedTransition`] with user mistake.
    pub fn user<R: Into<String>>(target: T, reason: R) -> Self {
        Self {
            target,
            reason: TransitionFailureReason::user(reason),
        }
    }

    /// Create new [`FailedTransition`] with internal error.
    #[allow(dead_code)] // May be useful in future
    pub fn internal<E: Into<color_eyre::Report>>(target: T, error: E) -> Self {
        Self {
            target,
            reason: TransitionFailureReason::internal(error),
        }
    }

    /// Transform [`FailedTransition`] to another [`FailedTransition`] with different target type
    /// which can be created from `T`.
    pub fn transform<U: From<T>>(self) -> FailedTransition<U> {
        FailedTransition {
            target: self.target.into(),
            reason: self.reason,
        }
    }
}

impl TransitionFailureReason {
    /// Create new [`TransitionFailureReason`] with user mistake.
    pub fn user<R: Into<String>>(reason: R) -> Self {
        Self::User(reason.into())
    }

    /// Create new [`TransitionFailureReason`] with internal error.
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

    /// Try to perform a transition from `S` to `Self` by `B`.
    ///
    /// Returns possibly different state with fail reason if not succeed.
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

/// State of the dialogue.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, From, PartialEq, Eq)]
pub enum State {
    /// Unauthorized state.
    ///
    /// Means that user has not signed in yet.
    Unauthorized(unauthorized::UnauthorizedBox),
    /// Authorized state.
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
                    // SecretPhrasePrompt --/cancel-> Start
                    (
                        UnauthorizedBox::SecretPhrasePrompt(secret_phrase_prompt),
                        Command::Cancel(cancel),
                    ) => Unauthorized::<kind::Start>::try_from_transition(
                        secret_phrase_prompt,
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
                        | UnauthorizedBox::SecretPhrasePrompt(_)),
                        _cmd,
                    ) => Err(unavailable_command(some_unauthorized.into())),
                }
            }
            Self::Authorized(authorized) => {
                use authorized::{kind, Authorized, AuthorizedBox};

                match (authorized, cmd) {
                    // ResourcesList --/cancel-> MainMenu
                    (AuthorizedBox::ResourcesList(resources_list), Command::Cancel(cancel)) => {
                        Authorized::<kind::MainMenu>::try_from_transition(
                            resources_list,
                            cancel,
                            context,
                        )
                        .await
                        .map(Into::into)
                        .map_err(FailedTransition::transform)
                    }
                    // ResourceActions --/cancel-> ResourcesList
                    (AuthorizedBox::ResourceActions(resource_actions), Command::Cancel(cancel)) => {
                        Authorized::<kind::ResourcesList>::try_from_transition(
                            resource_actions,
                            cancel,
                            context,
                        )
                        .await
                        .map(Into::into)
                        .map_err(FailedTransition::transform)
                    }
                    // DeleteConfirmation --/cancel-> ResourcesList
                    (
                        AuthorizedBox::DeleteConfirmation(delete_confirmation),
                        Command::Cancel(cancel),
                    ) => Authorized::<kind::ResourcesList>::try_from_transition(
                        delete_confirmation,
                        cancel,
                        context,
                    )
                    .await
                    .map(Into::into)
                    .map_err(FailedTransition::transform),
                    // Unavailable command
                    (
                        some_authorized @ (AuthorizedBox::MainMenu(_)
                        | AuthorizedBox::ResourcesList(_)
                        | AuthorizedBox::ResourceActions(_)
                        | AuthorizedBox::DeleteConfirmation(_)),
                        _cmd,
                    ) => Err(unavailable_command(some_authorized.into())),
                }
            }
        }
    }
}

#[async_trait]
impl TryFromTransition<Self, message::MessageBox> for State {
    type ErrorTarget = Self;

    async fn try_from_transition(
        state: Self,
        msg: message::MessageBox,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self>> {
        use authorized::{Authorized, AuthorizedBox};
        use message::MessageBox;
        use unauthorized::{Unauthorized, UnauthorizedBox};

        let unexpected_message =
            |s: Self| FailedTransition::user(s, "Unexpected message in the current state.");

        match state {
            Self::Unauthorized(unauthorized) => match (unauthorized, msg) {
                // Start --sign in-> SecretPhrasePrompt
                (UnauthorizedBox::Start(start), MessageBox::SignIn(sign_in)) => {
                    Unauthorized::<unauthorized::kind::SecretPhrasePrompt>::try_from_transition(
                        start, sign_in, context,
                    )
                    .await
                    .map(Into::into)
                    .map_err(FailedTransition::transform)
                }
                // SecretPhrasePrompt --secret phrase-> MainMenu
                (
                    UnauthorizedBox::SecretPhrasePrompt(secret_phrase_prompt),
                    MessageBox::Arbitrary(arbitrary),
                ) => Authorized::<authorized::kind::MainMenu>::try_from_transition(
                    secret_phrase_prompt,
                    arbitrary,
                    context,
                )
                .await
                .map(Into::into)
                .map_err(FailedTransition::transform),
                // Unexpected message
                (
                    some_unauthorized @ (UnauthorizedBox::Default(_)
                    | UnauthorizedBox::Start(_)
                    | UnauthorizedBox::SecretPhrasePrompt(_)),
                    _msg,
                ) => Err(unexpected_message(some_unauthorized.into())),
            },
            Self::Authorized(authorized) => match (authorized, msg) {
                // MainMenu --list-> ResourcesList
                (AuthorizedBox::MainMenu(main_menu), MessageBox::List(list)) => {
                    Authorized::<authorized::kind::ResourcesList>::try_from_transition(
                        main_menu, list, context,
                    )
                    .await
                    .map(Into::into)
                    .map_err(FailedTransition::transform)
                }
                // ResourcesList --arbitrary-> ResourceActions
                (
                    AuthorizedBox::ResourcesList(resources_list),
                    MessageBox::Arbitrary(arbitrary),
                ) => Authorized::<authorized::kind::ResourceActions>::try_from_transition(
                    resources_list,
                    arbitrary,
                    context,
                )
                .await
                .map(Into::into)
                .map_err(FailedTransition::transform),
                // Unexpected message
                (
                    some_authorized @ (AuthorizedBox::MainMenu(_)
                    | AuthorizedBox::ResourcesList(_)
                    | AuthorizedBox::ResourceActions(_)
                    | AuthorizedBox::DeleteConfirmation(_)),
                    _msg,
                ) => Err(unexpected_message(some_authorized.into())),
            },
        }
    }
}

#[async_trait]
impl TryFromTransition<Self, button::ButtonBox> for State {
    type ErrorTarget = Self;

    async fn try_from_transition(
        state: Self,
        button: button::ButtonBox,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self>> {
        use authorized::{Authorized, AuthorizedBox};
        use button::ButtonBox;

        let unexpected_button =
            |s: Self| FailedTransition::user(s, "Unexpected button action in the current state.");

        match state {
            // Unexpected button
            Self::Unauthorized(_) => Err(unexpected_button(state)),
            Self::Authorized(authorized) => match (authorized, button) {
                // ResourcesList --[delete]-> DeleteConfirmation
                (AuthorizedBox::ResourceActions(resource_actions), ButtonBox::Delete(delete)) => {
                    Authorized::<authorized::kind::DeleteConfirmation>::try_from_transition(
                        resource_actions,
                        delete,
                        context,
                    )
                    .await
                    .map(Into::into)
                    .map_err(FailedTransition::transform)
                }
                // Unexpected button
                (
                    some_authorized @ (AuthorizedBox::MainMenu(_)
                    | AuthorizedBox::ResourcesList(_)
                    | AuthorizedBox::ResourceActions(_)
                    | AuthorizedBox::DeleteConfirmation(_)),
                    _button,
                ) => Err(unexpected_button(some_authorized.into())),
            },
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
