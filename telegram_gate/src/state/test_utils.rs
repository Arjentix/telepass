//! Test utilities used by [`unauthorized`] and [`authorized`] submodules.

#![cfg(test)]
#![allow(clippy::unwrap_used)]

use mockall::predicate;
use teloxide::{types::ChatId, utils::command::BotCommands as _};

use super::*;
use crate::{command::Command, message::MessageBox, Bot, SendMessage};

/// Constant for test chat id.
pub const CHAT_ID: ChatId = ChatId(0);

/// Test that [`Command::Help`] is handled correctly for `state`.
pub async fn test_help_success(state: State) {
    let help = Command::Help(crate::command::Help);

    let mut mock_context = Context::default();
    mock_context.expect_chat_id().return_const(CHAT_ID);

    let mut mock_bot = Bot::default();
    mock_bot
        .expect_send_message::<ChatId, String>()
        .with(
            predicate::eq(CHAT_ID),
            predicate::eq(Command::descriptions().to_string()),
        )
        .returning(|_chat_id, _message| SendMessage::default());
    mock_context.expect_bot().return_const(mock_bot);

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
