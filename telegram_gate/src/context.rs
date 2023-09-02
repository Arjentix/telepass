//! Module with [`Context`] structure to pass values and dependencies between different states.

#![allow(clippy::indexing_slicing)]

#[cfg(test)]
use mockall::automock;

use super::{state, Arc, Bot, ChatId, PasswordStorageClient};

/// Context to pass values and dependencies between different states.
pub struct Context {
    /// Telegram bot instance. Mocked in tests.
    bot: Bot,
    /// Chat identifier.
    chat_id: ChatId,
    /// Client to interact with password storage service.
    storage_client: Arc<tokio::sync::Mutex<PasswordStorageClient>>,
}

#[cfg_attr(test, automock)]
impl Context {
    /// Construct new [`Context`].
    #[cfg_attr(not(test), inline)]
    pub fn new(
        bot: Bot,
        chat_id: ChatId,
        storage_client: Arc<tokio::sync::Mutex<PasswordStorageClient>>,
    ) -> Self {
        Self {
            bot,
            chat_id,
            storage_client,
        }
    }

    /// Get bot.
    #[allow(clippy::must_use_candidate, clippy::missing_const_for_fn)] // Due to issues in mockall
    #[cfg_attr(not(test), inline)]
    pub fn bot(&self) -> &Bot {
        &self.bot
    }

    /// Get chat id.
    #[allow(clippy::must_use_candidate, clippy::missing_const_for_fn)] // Due to issues in mockall
    #[cfg_attr(not(test), inline)]
    pub fn chat_id(&self) -> ChatId {
        self.chat_id
    }

    /// Get storage client proving that it's done only by an authorized state.
    ///
    /// The idea is that if caller side has [`Authorized`](state::authorized::Authorized) instance
    /// then it's eligible to get [`PasswordStorageClient`].
    #[cfg_attr(test, allow(clippy::used_underscore_binding))]
    #[cfg_attr(not(test), inline)]
    pub fn storage_client_from_behalf<A>(
        &self,
        _authorized: &A,
    ) -> &tokio::sync::Mutex<PasswordStorageClient>
    where
        A: state::authorized::Marker + 'static,
    {
        &self.storage_client
    }
}
