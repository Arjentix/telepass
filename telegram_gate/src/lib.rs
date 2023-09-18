//! Telegram Gate is a Telegram bot that allows user to access their passwords stored in Telepass.

use std::sync::Arc;

use cfg_if::cfg_if;
use mock_bot::{IdExt, UserExt};
#[cfg(not(test))]
use teloxide::payloads::SendMessageSetters;
use teloxide::prelude::*;

cfg_if! {
    if #[cfg(test)] {
        type Bot = mock_bot::MockBot;
        type TelegramMessage = crate::mock_bot::MockMessage;
        type PasswordStorageClient = grpc::MockPasswordStorageClient;
    } else {
        #[allow(clippy::missing_docs_in_private_items)]
        type Bot = teloxide::Bot;
        #[allow(clippy::missing_docs_in_private_items)]
        type TelegramMessage = teloxide::types::Message;
        #[allow(clippy::missing_docs_in_private_items)]
        pub type PasswordStorageClient =
            grpc::password_storage_client::PasswordStorageClient<tonic::transport::Channel>;
    }
}

pub mod button;
pub mod command;
pub mod context;
pub mod grpc;
pub mod message;
pub mod mock_bot;
pub mod state;
