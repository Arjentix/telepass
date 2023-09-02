//! Module with all meaningful commands.

use std::{convert::Infallible, str::FromStr};

use teloxide::utils::command::BotCommands;

/// Commands supported by the bot.
#[derive(BotCommands, Debug, Clone, PartialEq, Eq)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
#[allow(clippy::missing_docs_in_private_items)]
pub enum Command {
    #[command(description = "display this text")]
    Help(Help),
    #[command(description = "command to start the bot")]
    Start(Start),
    #[command(description = "cancel current operation")]
    Cancel(Cancel),
}

/// Macro to create blank [`FromStr`] implementation for commands.
///
/// It's blank because [`BotCommands`] derive-macro treats [`FromStr`] impl
/// as extra arguments parsing which is not needed for our case.
macro_rules! blank_from_str {
    ($($command_ty:ty),+ $(,)?) => {$(
        impl FromStr for $command_ty {
            type Err = Infallible;

            #[inline]
            fn from_str(_s: &str) -> Result<Self, Self::Err> {
                Ok(Self)
            }
        }
    )+};
}

/// Help command.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Help;

/// Start bot command.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Start;

/// Cancel current operation command.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Cancel;

blank_from_str!(Help, Start, Cancel);
