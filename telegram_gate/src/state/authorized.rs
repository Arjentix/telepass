//! Module with [`Authorized`] states.

use teloxide::{requests::Requester as _, types::ChatId, Bot};

use super::From;

/// Auhtorized state.
#[derive(Debug, Clone, From)]
#[must_use]
pub enum Authorized {
    MainMenu(MainMenu),
}

/// Main menu state.
///
/// Waits for user to input an action.
#[derive(Debug, Copy, Clone)]
pub struct MainMenu;

impl MainMenu {
    /// Setup [`MainMenu`].
    ///
    /// Prints welcome message and constructs a keyboard with all supported actions.
    pub async fn setup(bot: Bot, chat_id: ChatId) -> eyre::Result<Self> {
        bot.send_message(chat_id, "🏠 Welcome to the main menu.")
            .await?;

        // TODO: Keyboard

        Ok(Self)
    }
}
