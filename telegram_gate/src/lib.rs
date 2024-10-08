//! Telegram Gate is a Telegram bot that allows user to access their passwords stored in Telepass.

use std::sync::Arc;

use cfg_if::cfg_if;
use teloxide::prelude::*;

cfg_if! {
    if #[cfg(test)] {
        pub type Bot = test_utils::mock_bot::MockBot;
        pub type TelegramMessage = test_utils::mock_bot::MockMessage;
        pub type PasswordStorageClient = grpc::MockPasswordStorageClient;
    } else {
        pub type Bot = teloxide::Bot;
        pub type TelegramMessage = teloxide::types::Message;
        pub type PasswordStorageClient =
            grpc::password_storage_client::PasswordStorageClient<tonic::transport::Channel>;
    }
}

pub mod button;
pub mod command;
pub mod context;
pub mod grpc;
pub mod message;
pub mod state;
pub(crate) mod test_utils;
pub mod transition;

/// Trait to extend [`teloxide::types::Me`] with `user()` method.
pub trait UserExt {
    /// Get user info.
    fn user(&self) -> &teloxide::types::User;
}

impl UserExt for teloxide::types::Me {
    fn user(&self) -> &teloxide::types::User {
        &self.user
    }
}

/// Trait to extend [`teloxide::types::Message`] with field getters.
pub trait TelegramMessageGettersExt {
    /// Get message id.
    fn id(&self) -> teloxide::types::MessageId;

    /// Get message kind.
    fn take_kind(self) -> teloxide::types::MessageKind;
}

impl TelegramMessageGettersExt for teloxide::types::Message {
    fn id(&self) -> teloxide::types::MessageId {
        self.id
    }

    fn take_kind(self) -> teloxide::types::MessageKind {
        self.kind
    }
}
