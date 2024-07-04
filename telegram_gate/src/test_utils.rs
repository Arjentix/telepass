//! Test utilities.

#![cfg(test)]
#![allow(clippy::unwrap_used)]

use mock_bot::{MockBotBuilder, CHAT_ID};
use teloxide::utils::command::BotCommands as _;
use url::Url;

#[mockall_double::double]
use crate::context::Context;
use crate::{
    button::ButtonBox,
    command::Command,
    message::MessageBox,
    state::*,
    transition::{TransitionFailureReason, TryFromTransition as _},
};

pub mod mock_bot;

/// Test that [`Command::Help`] is handled correctly for `state`.
pub async fn test_help_success(state: State) {
    let help = Command::Help(crate::command::Help);

    let mut mock_context = Context::default();
    mock_context.expect_chat_id().return_const(CHAT_ID);
    mock_context.expect_bot().return_const(
        MockBotBuilder::new()
            .expect_send_message(Command::descriptions().to_string())
            .expect_into_future()
            .build(),
    );

    let new_state = State::try_from_transition(state.clone(), help, &mock_context)
        .await
        .unwrap();

    assert_eq!(state, new_state);
}

/// Test that `cmd` is not available for `state`.
pub async fn test_unavailable_command(state: State, cmd: Command) {
    let mock_context = Context::default();

    let err = State::try_from_transition(state.clone(), cmd, &mock_context)
        .await
        .unwrap_err();
    assert!(matches!(
        err.reason,
        TransitionFailureReason::User(user_mistake) if user_mistake == "Unavailable command in the current state.",
    ));
    assert_eq!(err.target, state)
}

/// Test that `msg` is not expected for `state`.
pub async fn test_unexpected_message(state: State, msg: MessageBox) {
    let mock_context = Context::default();

    let err = State::try_from_transition(state.clone(), msg, &mock_context)
        .await
        .unwrap_err();
    assert!(matches!(
        err.reason,
        TransitionFailureReason::User(user_mistake) if user_mistake == "Unexpected message in the current state.",
    ));
    assert_eq!(err.target, state)
}

/// Test that `btn` is not expected for `state`.
pub async fn test_unexpected_button(state: State, btn: ButtonBox) {
    let mock_context = Context::default();

    let err = State::try_from_transition(state.clone(), btn, &mock_context)
        .await
        .unwrap_err();
    assert!(matches!(
        err.reason,
        TransitionFailureReason::User(user_mistake) if user_mistake == "Unexpected button action in the current state.",
    ));
    assert_eq!(err.target, state)
}

pub fn web_app_test_url() -> Url {
    Url::parse("http://localhost:8081").unwrap()
}
