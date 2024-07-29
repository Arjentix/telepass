//! Module with all supported commands.

use std::{convert::Infallible, str::FromStr};

use teloxide::utils::command::BotCommands;

/// Commands supported by the bot.
#[derive(BotCommands, Debug, Clone, PartialEq, Eq)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
pub enum Command {
    #[command(description = "display this text")]
    Help(Help),
    #[command(description = "command to start the bot")]
    Start(Start),
    #[command(description = "cancel current operation")]
    Cancel(Cancel),
}

#[cfg(test)]
impl Command {
    #[must_use]
    pub const fn help() -> Self {
        Self::Help(Help)
    }

    #[must_use]
    pub const fn start() -> Self {
        Self::Start(Start)
    }

    #[must_use]
    pub const fn cancel() -> Self {
        Self::Cancel(Cancel)
    }
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "it's ok in tests")]

    use super::*;

    #[allow(
        dead_code,
        unreachable_code,
        unused_variables,
        clippy::unimplemented,
        clippy::diverging_sub_expression,
        clippy::panic,
        reason = "not needed as it's a static check"
    )]
    #[forbid(clippy::todo, clippy::wildcard_enum_match_arm)]
    fn tests_completeness_static_check() -> ! {
        panic!("You should never call this function, it's purpose is the static check only");

        let command: Command = unimplemented!();

        match command {
            Command::Help(_) => parse_help(),
            Command::Start(_) => parse_start(),
            Command::Cancel(_) => parse_cancel(),
        }

        unreachable!()
    }

    #[test]
    fn parse_help() {
        let command = Command::parse("/help", "test_bot_name").unwrap();
        assert!(matches!(command, Command::Help(_)));
    }

    #[test]
    fn parse_start() {
        let command = Command::parse("/start", "test_bot_name").unwrap();
        assert!(matches!(command, Command::Start(_)));
    }

    #[test]
    fn parse_cancel() {
        let command = Command::parse("/cancel", "test_bot_name").unwrap();
        assert!(matches!(command, Command::Cancel(_)));
    }
}
