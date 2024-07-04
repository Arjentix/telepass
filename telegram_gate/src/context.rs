//! Module with [`Context`] structure to pass values and dependencies between different states.

#![allow(clippy::indexing_slicing)]

#[cfg(test)]
use mockall::automock;
use url::Url;

use super::{Arc, Bot, ChatId, PasswordStorageClient};

/// Context to pass values and dependencies between different states.
pub struct Context {
    /// Telegram bot instance. Mocked in tests.
    bot: Bot,
    /// Chat identifier.
    chat_id: ChatId,
    /// URL ot the web app frontend.
    web_app_url: Arc<Url>,
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
        web_app_url: Arc<Url>,
        storage_client: Arc<tokio::sync::Mutex<PasswordStorageClient>>,
    ) -> Self {
        Self {
            bot,
            chat_id,
            web_app_url,
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

    /// Get web-app url.
    #[allow(clippy::must_use_candidate, clippy::missing_const_for_fn)] // Due to issues in mockall
    #[cfg_attr(not(test), inline)]
    pub fn web_app_url(&self) -> &Url {
        &self.web_app_url
    }

    /// Get password storage client.
    #[allow(clippy::must_use_candidate)] // Due to issues in mockall
    #[cfg_attr(not(test), inline)]
    pub fn storage_client(&self) -> &tokio::sync::Mutex<PasswordStorageClient> {
        &self.storage_client
    }
}
