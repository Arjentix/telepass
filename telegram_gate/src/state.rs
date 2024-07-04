//! Contains strongly-typed states of the [`Dialogue`](super::Dialogue).

#![allow(clippy::non_ascii_literal)]

#[cfg(test)]
use std::sync::Arc;

use derive_more::From;
use drop_bomb::DebugDropBomb;
#[cfg(not(test))]
use teloxide::requests::Requester as _;
use teloxide::types::MessageId;
#[cfg(test)]
use tokio::sync::RwLock;
use tracing::debug;

#[mockall_double::double]
use crate::context::Context;
use crate::{
    button, command, message,
    transition::{try_with_state, FailedTransition, TransitionFailureReason, TryFromTransition},
};

mod default;
mod delete_confirmation;
mod main_menu;
mod resource_actions;
mod resources_list;

/// State of the dialogue.
#[allow(clippy::module_name_repetitions, clippy::missing_docs_in_private_items)]
#[derive(Debug, Clone, From, PartialEq, Eq)]
pub enum State {
    Default(default::Default),
    MainMenu(main_menu::MainMenu),
    ResourcesList(resources_list::ResourcesList),
    ResourceActions(resource_actions::ResourceActions),
    DeleteConfirmation(delete_confirmation::DeleteConfirmation),
}

#[cfg(test)]
impl State {
    #[must_use]
    pub const fn main_menu() -> Self {
        Self::MainMenu(main_menu::MainMenu::test())
    }

    #[must_use]
    pub const fn resources_list() -> Self {
        Self::ResourcesList(resources_list::ResourcesList::test())
    }

    #[must_use]
    pub fn resource_actions(allow_not_deleted_messages: bool) -> Self {
        Self::ResourceActions(resource_actions::ResourceActions::test(
            Self::create_displayed_resource_data(allow_not_deleted_messages),
        ))
    }

    #[must_use]
    pub fn delete_confirmation(allow_not_deleted_messages: bool) -> Self {
        Self::DeleteConfirmation(delete_confirmation::DeleteConfirmation::test(
            Self::create_displayed_resource_data(allow_not_deleted_messages),
        ))
    }

    fn create_displayed_resource_data(
        allow_not_deleted_messages: bool,
    ) -> Arc<RwLock<DisplayedResourceData>> {
        let mut data = DisplayedResourceData::new(
            MessageId(0),
            MessageId(0),
            MessageId(0),
            "test.resource.com".to_owned(),
        );

        if allow_not_deleted_messages {
            data.bomb.defuse();
        }

        Arc::new(RwLock::new(data))
    }
}

/// Data related to displayed resource with buttons.
///
/// # Panics
///
/// Dropping a value of this type without calling [`delete_messages()`](Self::delete_messages)
/// will raise a panic.
#[derive(Debug)]
pub struct DisplayedResourceData {
    /// Message sent by user containing exact resource request.
    pub resource_request_message_id: MessageId,
    /// Currently displayed message with help message about `/cancel` command.
    pub cancel_message_id: MessageId,
    /// Currently displayed message with resource name and attached buttons.
    pub resource_message_id: MessageId,
    /// Name of the requested resource.
    pub resource_name: String,
    /// Bomb to prevent dropping this type without deleting messages.
    bomb: DebugDropBomb,
}

impl DisplayedResourceData {
    /// Construct new [`DisplayedResourceData`].
    #[must_use]
    pub fn new(
        resource_request_message_id: MessageId,
        cancel_message_id: MessageId,
        resource_message_id: MessageId,
        resource_name: String,
    ) -> Self {
        Self {
            resource_request_message_id,
            cancel_message_id,
            resource_message_id,
            resource_name,
            bomb: Self::init_bomb(),
        }
    }

    /// Initialize [`DebugDropBomb`] with a message.
    fn init_bomb() -> DebugDropBomb {
        DebugDropBomb::new(
            "`DisplayedResourceData` messages should always \
              be deleted before dropping this type",
        )
    }

    /// Delete contained messages.
    ///
    /// # Errors
    ///
    /// Fails if there is an error while deleting messages.
    pub async fn delete_messages(mut self, context: &Context) -> color_eyre::Result<()> {
        self.bomb.defuse();

        for message_id in [
            self.resource_request_message_id,
            self.cancel_message_id,
            self.resource_message_id,
        ] {
            context
                .bot()
                .delete_message(context.chat_id(), message_id)
                .await?;
        }

        debug!("Displayed resource messages deleted");
        Ok(())
    }
}

impl PartialEq for DisplayedResourceData {
    /// Skipping [`MessageId`] fields because they don't implement [`Eq`].
    fn eq(&self, other: &Self) -> bool {
        self.resource_name == other.resource_name
    }
}

impl Eq for DisplayedResourceData {}

impl Default for State {
    fn default() -> Self {
        Self::Default(self::default::Default)
    }
}

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

        match (from, cmd) {
            // Default --/start-> MainMenu
            (Self::Default(default), Command::Start(start)) => {
                main_menu::MainMenu::try_from_transition(default, start, context)
                    .await
                    .map(Into::into)
                    .map_err(FailedTransition::transform)
            }
            // ResourcesList --/cancel-> MainMenu
            (Self::ResourcesList(resources_list), Command::Cancel(cancel)) => {
                main_menu::MainMenu::try_from_transition(resources_list, cancel, context)
                    .await
                    .map(Into::into)
                    .map_err(FailedTransition::transform)
            }
            // ResourceActions --/cancel-> ResourcesList
            (Self::ResourceActions(resource_actions), Command::Cancel(cancel)) => {
                resources_list::ResourcesList::try_from_transition(
                    resource_actions,
                    cancel,
                    context,
                )
                .await
                .map(Into::into)
                .map_err(FailedTransition::transform)
            }
            // DeleteConfirmation --/cancel-> ResourcesList
            (Self::DeleteConfirmation(delete_confirmation), Command::Cancel(cancel)) => {
                resources_list::ResourcesList::try_from_transition(
                    delete_confirmation,
                    cancel,
                    context,
                )
                .await
                .map(Into::into)
                .map_err(FailedTransition::transform)
            }
            // Unavailable command
            (
                some_state @ (Self::Default(_)
                | Self::MainMenu(_)
                | Self::ResourcesList(_)
                | Self::ResourceActions(_)
                | Self::DeleteConfirmation(_)),
                _cmd,
            ) => Err(unavailable_command(some_state)),
        }
    }
}

impl TryFromTransition<Self, message::MessageBox> for State {
    type ErrorTarget = Self;

    async fn try_from_transition(
        state: Self,
        msg: message::MessageBox,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self>> {
        use message::MessageBox;

        let unexpected_message =
            |s: Self| FailedTransition::user(s, "Unexpected message in the current state.");

        match (state, msg) {
            // MainMenu --list-> ResourcesList
            (Self::MainMenu(main_menu), MessageBox::List(list)) => {
                resources_list::ResourcesList::try_from_transition(main_menu, list, context)
                    .await
                    .map(Into::into)
                    .map_err(FailedTransition::transform)
            }
            // MainMenu --Add (WebApp)-> MainMenu
            (Self::MainMenu(main_menu), MessageBox::WebApp(web_app)) => {
                main_menu::MainMenu::try_from_transition(main_menu, web_app, context)
                    .await
                    .map(Into::into)
                    .map_err(FailedTransition::transform)
            }
            // ResourcesList --arbitrary-> ResourceActions
            (Self::ResourcesList(resources_list), MessageBox::Arbitrary(arbitrary)) => {
                resource_actions::ResourceActions::try_from_transition(
                    resources_list,
                    arbitrary,
                    context,
                )
                .await
                .map(Into::into)
                .map_err(FailedTransition::transform)
            }
            // Unexpected message
            (
                some_state @ (Self::Default(_)
                | Self::MainMenu(_)
                | Self::ResourcesList(_)
                | Self::ResourceActions(_)
                | Self::DeleteConfirmation(_)),
                _msg,
            ) => Err(unexpected_message(some_state)),
        }
    }
}

impl TryFromTransition<Self, button::ButtonBox> for State {
    type ErrorTarget = Self;

    async fn try_from_transition(
        state: Self,
        button: button::ButtonBox,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self>> {
        use button::ButtonBox;

        let unexpected_button =
            |s: Self| FailedTransition::user(s, "Unexpected button action in the current state.");

        match (state, button) {
            // ResourceActions --[delete]-> DeleteConfirmation
            (Self::ResourceActions(resource_actions), ButtonBox::Delete(delete)) => {
                delete_confirmation::DeleteConfirmation::try_from_transition(
                    resource_actions,
                    delete,
                    context,
                )
                .await
                .map(Into::into)
                .map_err(FailedTransition::transform)
            }
            // DeleteConfirmation --[yes]-> MainMenu
            (Self::DeleteConfirmation(delete_confirmation), ButtonBox::Yes(yes)) => {
                main_menu::MainMenu::try_from_transition(delete_confirmation, yes, context)
                    .await
                    .map(Into::into)
                    .map_err(FailedTransition::transform)
            }
            // DeleteConfirmation --[no]-> ResourceActions
            (Self::DeleteConfirmation(delete_confirmation), ButtonBox::No(no)) => {
                resource_actions::ResourceActions::try_from_transition(
                    delete_confirmation,
                    no,
                    context,
                )
                .await
                .map(Into::into)
                .map_err(FailedTransition::transform)
            }
            // Unexpected button
            (
                some_state @ (Self::Default(_)
                | Self::MainMenu(_)
                | Self::ResourcesList(_)
                | Self::ResourceActions(_)
                | Self::DeleteConfirmation(_)),
                _button,
            ) => Err(unexpected_button(some_state)),
        }
    }
}

impl<T: Into<State> + Send> TryFromTransition<Self, command::Help> for T {
    type ErrorTarget = Self;

    async fn try_from_transition(
        state: T,
        _help: command::Help,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self>> {
        use teloxide::utils::command::BotCommands as _;

        try_with_state!(
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[allow(
        dead_code,
        unreachable_code,
        unused_variables,
        clippy::unimplemented,
        clippy::diverging_sub_expression,
        clippy::too_many_lines
    )]
    #[forbid(clippy::todo, clippy::wildcard_enum_match_arm)]
    fn tests_completeness_static_check() -> ! {
        use self::{button::*, command::*, message::*};

        panic!("You should never call this function, it's purpose is the static check only");

        // We don't need the actual values, we just need something to check match arms
        let state: State = unimplemented!();
        let cmd: Command = unimplemented!();
        let msg: MessageBox = unimplemented!();
        let button: ButtonBox = unimplemented!();

        // Will fail to compile if a new state or command will be added
        match (state, cmd) {
            (State::Default(_), Command::Help(_)) => default::tests::command::help_success(),
            (State::Default(_), Command::Start(_)) => {
                main_menu::tests::command::from_default_by_start_success()
            }
            (State::Default(_), Command::Cancel(_)) => default::tests::command::cancel_failure(),
            (State::MainMenu(_), Command::Help(_)) => main_menu::tests::command::help_success(),
            (State::MainMenu(_), Command::Start(_)) => main_menu::tests::command::start_failure(),
            (State::MainMenu(_), Command::Cancel(_)) => main_menu::tests::command::cancel_failure(),
            (State::ResourcesList(_), Command::Help(_)) => {
                resources_list::tests::command::help_success()
            }
            (State::ResourcesList(_), Command::Start(_)) => {
                resources_list::tests::command::start_failure()
            }
            (State::ResourcesList(_), Command::Cancel(_)) => {
                main_menu::tests::command::from_resources_list_by_cancel_success()
            }
            (State::ResourceActions(_), Command::Help(_)) => {
                resource_actions::tests::command::help_success()
            }
            (State::ResourceActions(_), Command::Start(_)) => {
                resource_actions::tests::command::start_failure()
            }
            (State::ResourceActions(_), Command::Cancel(_)) => {
                resources_list::tests::command::from_resource_actions_by_cancel_success()
            }
            (State::DeleteConfirmation(_), Command::Help(_)) => {
                delete_confirmation::tests::command::help_success()
            }
            (State::DeleteConfirmation(_), Command::Start(_)) => {
                delete_confirmation::tests::command::start_failure()
            }
            (State::DeleteConfirmation(_), Command::Cancel(_)) => {
                resources_list::tests::command::from_delete_confirmation_by_cancel_success()
            }
        }

        // Will fail to compile if a new state or message will be added
        match (state, msg) {
            (State::Default(_), MessageBox::WebApp(_)) => {
                default::tests::message::web_app_failure()
            }
            (State::Default(_), MessageBox::Add(_)) => default::tests::message::add_failure(),
            (State::Default(_), MessageBox::List(_)) => default::tests::message::list_failure(),
            (State::Default(_), MessageBox::Arbitrary(_)) => {
                default::tests::message::arbitrary_failure()
            }
            (State::MainMenu(_), MessageBox::WebApp(_)) => {
                main_menu::tests::message::web_app_success();
                main_menu::tests::message::web_app_wrong_button_text_failure();
                main_menu::tests::message::web_app_wrong_data_failure()
            }
            (State::MainMenu(_), MessageBox::Add(_)) => main_menu::tests::message::add_failure(),
            (State::MainMenu(_), MessageBox::List(_)) => {
                resources_list::tests::message::from_main_menu_by_list_success();
                resources_list::tests::message::from_main_menu_by_list_with_empty_user_failure();
            }
            (State::MainMenu(_), MessageBox::Arbitrary(_)) => {
                main_menu::tests::message::arbitrary_failure()
            }
            (State::ResourcesList(_), MessageBox::WebApp(_)) => {
                resources_list::tests::message::web_app_failure()
            }
            (State::ResourcesList(_), MessageBox::Add(_)) => {
                resources_list::tests::message::add_failure()
            }
            (State::ResourcesList(_), MessageBox::List(_)) => {
                resources_list::tests::message::list_failure()
            }
            (State::ResourcesList(_), MessageBox::Arbitrary(_)) => {
                resource_actions::tests::message::from_resources_list_by_right_arbitrary_success();
                resource_actions::tests::message::from_resources_list_by_wrong_arbitrary_failure();
            }
            (State::ResourceActions(_), MessageBox::WebApp(_)) => {
                resource_actions::tests::message::web_app_failure()
            }
            (State::ResourceActions(_), MessageBox::Add(_)) => {
                resource_actions::tests::message::add_failure()
            }
            (State::ResourceActions(_), MessageBox::List(_)) => {
                resource_actions::tests::message::list_failure()
            }
            (State::ResourceActions(_), MessageBox::Arbitrary(_)) => {
                resource_actions::tests::message::arbitrary_failure()
            }
            (State::DeleteConfirmation(_), MessageBox::WebApp(_)) => {
                delete_confirmation::tests::message::web_app_failure()
            }
            (State::DeleteConfirmation(_), MessageBox::Add(_)) => {
                delete_confirmation::tests::message::add_failure()
            }
            (State::DeleteConfirmation(_), MessageBox::List(_)) => {
                delete_confirmation::tests::message::list_failure()
            }
            (State::DeleteConfirmation(_), MessageBox::Arbitrary(_)) => {
                delete_confirmation::tests::message::arbitrary_failure()
            }
        }

        // Will fail to compile if a new state or button will be added
        match (state, button) {
            (State::Default(_), ButtonBox::Delete(_)) => default::tests::button::delete_failure(),
            (State::Default(_), ButtonBox::Yes(_)) => default::tests::button::yes_failure(),
            (State::Default(_), ButtonBox::No(_)) => default::tests::button::no_failure(),
            (State::Default(_), ButtonBox::Show(_)) => default::tests::button::show_failure(),
            (State::MainMenu(_), ButtonBox::Delete(_)) => {
                main_menu::tests::button::delete_failure()
            }
            (State::MainMenu(_), ButtonBox::Yes(_)) => main_menu::tests::button::yes_failure(),
            (State::MainMenu(_), ButtonBox::No(_)) => main_menu::tests::button::no_failure(),
            (State::MainMenu(_), ButtonBox::Show(_)) => main_menu::tests::button::show_failure(),
            (State::ResourcesList(_), ButtonBox::Delete(_)) => {
                resources_list::tests::button::delete_failure()
            }
            (State::ResourcesList(_), ButtonBox::Yes(_)) => {
                resources_list::tests::button::yes_failure()
            }
            (State::ResourcesList(_), ButtonBox::No(_)) => {
                resources_list::tests::button::no_failure()
            }
            (State::ResourcesList(_), ButtonBox::Show(_)) => {
                resources_list::tests::button::show_failure()
            }
            (State::ResourceActions(_), ButtonBox::Delete(_)) => {
                delete_confirmation::tests::button::from_resource_actions_by_delete_success()
            }
            (State::ResourceActions(_), ButtonBox::Yes(_)) => {
                resource_actions::tests::button::yes_failure()
            }
            (State::ResourceActions(_), ButtonBox::No(_)) => {
                resource_actions::tests::button::no_failure()
            }
            (State::ResourceActions(_), ButtonBox::Show(_)) => {
                resource_actions::tests::button::show_failure()
            }
            (State::DeleteConfirmation(_), ButtonBox::Delete(_)) => {
                delete_confirmation::tests::button::delete_failure()
            }
            (State::DeleteConfirmation(_), ButtonBox::Yes(_)) => {
                main_menu::tests::button::from_delete_confirmation_by_yes_success()
            }
            (State::DeleteConfirmation(_), ButtonBox::No(_)) => {
                resource_actions::tests::button::from_delete_confirmation_by_no_success()
            }
            (State::DeleteConfirmation(_), ButtonBox::Show(_)) => {
                delete_confirmation::tests::button::show_failure()
            }
        }

        unreachable!()
    }
}
