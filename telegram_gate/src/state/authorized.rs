//! Module with [`Authorized`] states.

use color_eyre::eyre::WrapErr as _;
use teloxide::types::{KeyboardButton, KeyboardMarkup};

use super::{
    async_trait, command, message, try_with_target, unauthorized, Context, FailedTransition, From,
    TransitionFailureReason, TryFromTransition,
};
#[cfg(not(test))]
use super::{Requester as _, SendMessageSetters as _};
use crate::button;

mod sealed {
    //! Module with [`Sealed`] and its implementations for authorized states.

    use super::*;

    /// Trait to prevent [`super::Marker`] implementation for types outside of
    /// [`authorized`](super) module.
    pub trait Sealed {}

    impl Sealed for Authorized<kind::MainMenu> {}
    impl Sealed for Authorized<kind::WaitingForResourceName> {}
    impl Sealed for Authorized<kind::WaitingForButtonPress> {}
}

/// Marker trait to identify *authorized* states.
pub trait Marker: sealed::Sealed {}

impl Marker for Authorized<kind::MainMenu> {}
impl Marker for Authorized<kind::WaitingForResourceName> {}
impl Marker for Authorized<kind::WaitingForButtonPress> {}

/// Enum with all possible authorized states.
#[derive(Debug, Clone, From, PartialEq, Eq)]
#[allow(clippy::module_name_repetitions, clippy::missing_docs_in_private_items)]
pub enum AuthorizedBox {
    MainMenu(Authorized<kind::MainMenu>),
    WaitingForResourceName(Authorized<kind::WaitingForResourceName>),
    WaitingForButtonPress(Authorized<kind::WaitingForButtonPress>),
}

/// Authorized state.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct Authorized<K> {
    /// Kind of an authorized state.
    kind: K,
}

pub mod kind {
    //! Module with [`Authorized`] kinds.

    use super::{super::State, Authorized, AuthorizedBox};

    /// Macro to implement conversion from concrete authorized state to the general [`State`].
    macro_rules! into_state {
            ($($kind_ty:ty),+ $(,)?) => {$(
                impl From<Authorized<$kind_ty>> for State {
                    fn from(value: Authorized<$kind_ty>) -> Self {
                        AuthorizedBox::from(value).into()
                    }
                }
            )+};
    }

    /// Main menu state kind.
    ///
    /// Waits for user to input an action.
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct MainMenu;

    /// Kind of a state when bot is waiting for user to input a resource name from list.
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct WaitingForResourceName;

    /// Kind of a state when bot is waiting for user to press some inline button
    /// to make an action with a resource attached to a message.
    #[derive(Debug, Clone)]
    pub struct WaitingForButtonPress {
        /// Message sent by user which triggered transition to this state.
        ///
        /// Should be cleared in any transition from this state.
        pub resource_request_message: teloxide::types::Message,

        /// Currently displayed message with help message about `/cancel` command.
        ///
        /// Should be cleared in any transition from this state.
        pub displayed_cancel_message: teloxide::types::Message,

        /// Currently displayed message with resource name and attached buttons.
        ///
        /// Should be cleared in any transition from this state.
        pub displayed_resource_message: teloxide::types::Message,
    }

    impl PartialEq for WaitingForButtonPress {
        /// Skipping [`Message`](teloxide::types::Message) fields,
        /// because they don't implement [`Eq`].
        fn eq(&self, _other: &Self) -> bool {
            true
        }
    }

    impl Eq for WaitingForButtonPress {}

    into_state!(MainMenu, WaitingForResourceName, WaitingForButtonPress);
}

impl Authorized<kind::MainMenu> {
    /// Setup [`Authorized`] state of [`MainMenu`](kind::MainMenu) kind.
    ///
    /// Prints welcome message and constructs a keyboard with all supported actions.
    async fn setup(context: &Context) -> color_eyre::Result<Self> {
        let buttons = [[KeyboardButton::new(message::kind::List.to_string())]];
        let keyboard = KeyboardMarkup::new(buttons).resize_keyboard(Some(true));

        context
            .bot()
            .send_message(context.chat_id(), "🏠 Welcome to the main menu.")
            .reply_markup(keyboard)
            .await?;

        Ok(Self {
            kind: kind::MainMenu,
        })
    }
}

#[async_trait]
impl
    TryFromTransition<
        unauthorized::Unauthorized<unauthorized::kind::WaitingForSecretPhrase>,
        message::Message<message::kind::Arbitrary>,
    > for Authorized<kind::MainMenu>
{
    type ErrorTarget = unauthorized::Unauthorized<unauthorized::kind::WaitingForSecretPhrase>;

    async fn try_from_transition(
        waiting_for_secret_phrase: unauthorized::Unauthorized<
            unauthorized::kind::WaitingForSecretPhrase,
        >,
        arbitrary: message::Message<message::kind::Arbitrary>,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        let admin_token_candidate = arbitrary.to_string();
        if admin_token_candidate != waiting_for_secret_phrase.admin_token {
            return Err(FailedTransition::user(
                waiting_for_secret_phrase,
                "❎ Invalid token. Please, try again.",
            ));
        }

        try_with_target!(
            waiting_for_secret_phrase,
            context
                .bot()
                .send_message(context.chat_id(), "✅ You've successfully signed in!")
                .await
                .map_err(TransitionFailureReason::internal)
        );

        let main_menu = try_with_target!(
            waiting_for_secret_phrase,
            Self::setup(context)
                .await
                .map_err(TransitionFailureReason::internal)
        );
        Ok(main_menu)
    }
}

#[async_trait]
impl TryFromTransition<Authorized<kind::WaitingForResourceName>, command::Cancel>
    for Authorized<kind::MainMenu>
{
    type ErrorTarget = Authorized<kind::WaitingForResourceName>;

    async fn try_from_transition(
        waiting_for_resource_name: Authorized<kind::WaitingForResourceName>,
        _cancel: command::Cancel,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        let main_menu = try_with_target!(
            waiting_for_resource_name,
            Self::setup(context)
                .await
                .map_err(TransitionFailureReason::internal)
        );
        Ok(main_menu)
    }
}

impl Authorized<kind::WaitingForResourceName> {
    /// Setup [`Authorized`] state of [`WaitingForResourceName`](kind::WaitingForResourceName) kind.
    ///
    /// Constructs a keyboard with resources for all stored passwords.
    ///
    /// # Errors
    ///
    /// Fails if:
    /// - Unable to retrieve the list of stored resources;
    /// - There are not stored resources;
    /// - Unable to send a message.
    async fn setup<A>(prev_state: A, context: &Context) -> Result<Self, FailedTransition<A>>
    where
        A: Marker + Send + Sync + 'static,
    {
        let resources = {
            let mut storage_client_lock =
                context.storage_client_from_behalf(&prev_state).lock().await;

            try_with_target!(
                prev_state,
                storage_client_lock
                    .list(crate::grpc::Empty {})
                    .await
                    .wrap_err("Failed to retrieve the list of stored passwords")
                    .map_err(TransitionFailureReason::internal)
            )
            .into_inner()
            .resources
        };

        if resources.is_empty() {
            return Err(FailedTransition::user(
                prev_state,
                "❎ There are no stored passwords yet.",
            ));
        }

        let buttons = resources
            .into_iter()
            .map(|resource| [KeyboardButton::new(format!("🔑 {}", resource.name))]);
        let keyboard = KeyboardMarkup::new(buttons).resize_keyboard(true);

        try_with_target!(
            prev_state,
            context
                .bot()
                .send_message(
                    context.chat_id(),
                    "👉 Choose a resource.\n\n\
                     Type /cancel to go back."
                )
                .reply_markup(keyboard)
                .await
                .map_err(TransitionFailureReason::internal)
        );

        Ok(Self {
            kind: kind::WaitingForResourceName,
        })
    }
}

#[async_trait]
impl TryFromTransition<Authorized<kind::MainMenu>, message::Message<message::kind::List>>
    for Authorized<kind::WaitingForResourceName>
{
    type ErrorTarget = Authorized<kind::MainMenu>;

    async fn try_from_transition(
        main_menu: Authorized<kind::MainMenu>,
        _list: message::Message<message::kind::List>,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        Self::setup(main_menu, context).await
    }
}

#[async_trait]
impl TryFromTransition<Authorized<kind::WaitingForButtonPress>, command::Cancel>
    for Authorized<kind::WaitingForResourceName>
{
    type ErrorTarget = Authorized<kind::WaitingForButtonPress>;

    async fn try_from_transition(
        waiting_for_button_press: Authorized<kind::WaitingForButtonPress>,
        _cancel: command::Cancel,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        for message_id in [
            waiting_for_button_press.kind.resource_request_message.id,
            waiting_for_button_press.kind.displayed_cancel_message.id,
            waiting_for_button_press.kind.displayed_resource_message.id,
        ] {
            try_with_target!(
                waiting_for_button_press,
                context
                    .bot()
                    .delete_message(context.chat_id(), message_id)
                    .await
                    .map_err(TransitionFailureReason::internal)
            );
        }
        Self::setup(waiting_for_button_press, context).await
    }
}

#[async_trait]
impl
    TryFromTransition<
        Authorized<kind::WaitingForResourceName>,
        message::Message<message::kind::Arbitrary>,
    > for Authorized<kind::WaitingForButtonPress>
{
    type ErrorTarget = Authorized<kind::WaitingForResourceName>;

    async fn try_from_transition(
        waiting_for_resource_name: Authorized<kind::WaitingForResourceName>,
        arbitrary: message::Message<message::kind::Arbitrary>,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        let resource_name = arbitrary.to_string();
        let resource_name = resource_name.strip_prefix("🔑 ").unwrap_or(&resource_name);

        let res = {
            let mut storage_client_lock = context
                .storage_client_from_behalf(&waiting_for_resource_name)
                .lock()
                .await;

            storage_client_lock
                .get(crate::grpc::Resource {
                    name: resource_name.to_owned(),
                })
                .await
        };

        #[allow(clippy::wildcard_enum_match_arm)]
        if let Err(status) = res {
            return match status.code() {
                tonic::Code::NotFound => Err(FailedTransition::user(
                    waiting_for_resource_name,
                    "❎ Resource not found.",
                )),
                _ => Err(FailedTransition::internal(
                    waiting_for_resource_name,
                    status,
                )),
            };
        }

        let cancel_message = try_with_target!(
            waiting_for_resource_name,
            context
                .bot()
                .send_message(context.chat_id(), "Type /cancel to go back.",)
                .reply_markup(teloxide::types::ReplyMarkup::kb_remove())
                .await
                .map_err(TransitionFailureReason::internal)
        );

        let message = try_with_target!(
            waiting_for_resource_name,
            context
                .bot()
                .send_message(
                    context.chat_id(),
                    format!(
                        "🔑 *{resource_name}*\n\n\
                         Choose an action:"
                    )
                )
                .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                .reply_markup(teloxide::types::ReplyMarkup::inline_kb([[
                    teloxide::types::InlineKeyboardButton::callback(
                        button::kind::Delete.to_string(),
                        button::kind::Delete.to_string(),
                    )
                ]]))
                .await
                .map_err(TransitionFailureReason::internal)
        );

        Ok(Self {
            kind: kind::WaitingForButtonPress {
                resource_request_message: arbitrary.inner,
                displayed_cancel_message: cancel_message,
                displayed_resource_message: message,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use mockall::predicate;
    use teloxide::types::ChatId;

    use super::*;
    use crate::{
        command::Command,
        message::MessageBox,
        state::{test_utils::CHAT_ID, State},
        Bot, SendMessage,
    };

    #[allow(
        dead_code,
        unreachable_code,
        unused_variables,
        clippy::unimplemented,
        clippy::diverging_sub_expression,
        clippy::panic
    )]
    #[forbid(clippy::todo, clippy::wildcard_enum_match_arm)]
    fn tests_completeness_static_check() -> ! {
        use self::{command::*, message::*};

        panic!("You should never call this function, it's purpose is the static check only");

        // We don't need the actual values, we just need something to check match arms
        let authorized: AuthorizedBox = unimplemented!();
        let cmd: Command = unimplemented!();
        let mes: MessageBox = unimplemented!();

        // Will fail to compile if a new state or command will be added
        match (authorized, cmd) {
            (AuthorizedBox::MainMenu(_), Command::Help(_)) => main_menu_help_success(),
            (AuthorizedBox::MainMenu(_), Command::Start(_)) => main_menu_start_failure(),
            (AuthorizedBox::MainMenu(_), Command::Cancel(_)) => main_menu_cancel_failure(),
            (AuthorizedBox::WaitingForResourceName(_), Command::Help(_)) => {
                waiting_for_resource_name_help_success()
            }
            (AuthorizedBox::WaitingForResourceName(_), Command::Start(_)) => {
                waiting_for_resource_name_start_failure()
            }
            (AuthorizedBox::WaitingForResourceName(_), Command::Cancel(_)) => {
                waiting_for_resource_name_cancel_success()
            }
        }

        // Will fail to compile if a new state or message will be added
        match (authorized, mes) {
            (AuthorizedBox::MainMenu(_), MessageBox::SignIn(_)) => main_menu_sign_in_failure(),
            (AuthorizedBox::MainMenu(_), MessageBox::List(_)) => main_menu_list_success(),
            (AuthorizedBox::MainMenu(_), MessageBox::Arbitrary(_)) => main_menu_arbitrary_failure(),
            (AuthorizedBox::WaitingForResourceName(_), MessageBox::SignIn(_)) => {
                waiting_for_resource_name_sign_in_failure()
            }
            (AuthorizedBox::WaitingForResourceName(_), MessageBox::List(_)) => {
                waiting_for_resource_name_list_failure()
            }
            (AuthorizedBox::WaitingForResourceName(_), MessageBox::Arbitrary(_)) => {
                waiting_for_resource_name_arbitrary_failure()
            }
        }

        unreachable!()
    }

    mod command {
        //! Test names follow the rule: *state*_*command*_*success/failure*.

        use tokio::test;

        use super::*;
        use crate::state::test_utils::{test_help_success, test_unavailable_command};

        #[test]
        pub async fn main_menu_help_success() {
            let main_menu = State::Authorized(AuthorizedBox::MainMenu(Authorized {
                kind: kind::MainMenu,
            }));

            test_help_success(main_menu).await
        }

        #[test]
        pub async fn main_menu_start_failure() {
            let main_menu = State::Authorized(AuthorizedBox::MainMenu(Authorized {
                kind: kind::MainMenu,
            }));
            let start = Command::Start(crate::command::Start);

            test_unavailable_command(main_menu, start).await
        }

        #[test]
        pub async fn main_menu_cancel_failure() {
            let main_menu = State::Authorized(AuthorizedBox::MainMenu(Authorized {
                kind: kind::MainMenu,
            }));
            let cancel = Command::Cancel(crate::command::Cancel);

            test_unavailable_command(main_menu, cancel).await
        }

        #[test]
        pub async fn waiting_for_resource_name_help_success() {
            let waiting_for_resource_name =
                State::Authorized(AuthorizedBox::WaitingForResourceName(Authorized {
                    kind: kind::WaitingForResourceName,
                }));

            test_help_success(waiting_for_resource_name).await
        }

        #[test]
        pub async fn waiting_for_resource_name_start_failure() {
            let waiting_for_resource_name =
                State::Authorized(AuthorizedBox::WaitingForResourceName(Authorized {
                    kind: kind::WaitingForResourceName,
                }));
            let start = Command::Start(crate::command::Start);

            test_unavailable_command(waiting_for_resource_name, start).await
        }

        #[test]
        pub async fn waiting_for_resource_name_cancel_success() {
            let waiting_for_resource_name =
                State::Authorized(AuthorizedBox::WaitingForResourceName(Authorized {
                    kind: kind::WaitingForResourceName,
                }));
            let cancel = Command::Cancel(crate::command::Cancel);

            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);

            let mut mock_bot = Bot::default();
            mock_bot
                .expect_send_message::<ChatId, &'static str>()
                .with(
                    predicate::eq(CHAT_ID),
                    predicate::eq("🏠 Welcome to the main menu."),
                )
                .returning(|_chat_id, _message| {
                    let mut mock_send_message = SendMessage::default();

                    let expected_keyboard = KeyboardMarkup::new([[KeyboardButton::new(
                        crate::message::List.to_string(),
                    )]])
                    .resize_keyboard(Some(true));

                    mock_send_message
                        .expect_reply_markup::<KeyboardMarkup>()
                        .with(predicate::eq(expected_keyboard))
                        .returning(|_markup| SendMessage::default());
                    mock_send_message
                });
            mock_context.expect_bot().return_const(mock_bot);

            let state =
                State::try_from_transition(waiting_for_resource_name, cancel, &mock_context)
                    .await
                    .unwrap();
            assert!(matches!(
                state,
                State::Authorized(AuthorizedBox::MainMenu(_))
            ))
        }
    }

    mod message {
        use tokio::test;

        use super::*;
        use crate::state::test_utils::test_unexpected_message;

        #[test]
        pub async fn main_menu_sign_in_failure() {
            let main_menu = State::Authorized(AuthorizedBox::MainMenu(Authorized {
                kind: kind::MainMenu,
            }));
            let sign_in = MessageBox::SignIn(crate::message::SignIn);

            test_unexpected_message(main_menu, sign_in).await
        }

        #[test]
        pub async fn main_menu_list_success() {
            const RESOURCE_NAMES: [&str; 3] =
                ["Test Resource 1", "Test Resource 2", "Test Resource 3"];

            let main_menu = State::Authorized(AuthorizedBox::MainMenu(Authorized {
                kind: kind::MainMenu,
            }));
            let list = MessageBox::List(crate::message::List);

            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);

            let mut mock_bot = Bot::default();
            mock_bot
                .expect_send_message::<ChatId, &'static str>()
                .with(
                    predicate::eq(CHAT_ID),
                    predicate::eq(
                        "👉 Choose a resource.\n\n\
                                   Type /cancel to go back.",
                    ),
                )
                .returning(|_chat_id, _message| {
                    let mut mock_send_message = SendMessage::default();

                    let expected_buttons = RESOURCE_NAMES
                        .into_iter()
                        .map(|name| [KeyboardButton::new(format!("🔑 {}", name))]);
                    let expected_keyboard =
                        KeyboardMarkup::new(expected_buttons).resize_keyboard(Some(true));

                    mock_send_message
                        .expect_reply_markup::<KeyboardMarkup>()
                        .with(predicate::eq(expected_keyboard))
                        .returning(|_markup| SendMessage::default());
                    mock_send_message
                });
            mock_context.expect_bot().return_const(mock_bot);

            let mut mock_storage_client = crate::PasswordStorageClient::default();
            mock_storage_client
                .expect_list::<crate::grpc::Empty>()
                .with(predicate::eq(crate::grpc::Empty {}))
                .returning(|_empty| {
                    let resources = RESOURCE_NAMES
                        .into_iter()
                        .map(ToOwned::to_owned)
                        .map(|name| crate::grpc::Resource { name })
                        .collect();
                    Ok(tonic::Response::new(crate::grpc::ListOfResources {
                        resources,
                    }))
                });
            mock_context
                .expect_storage_client_from_behalf::<Authorized<kind::MainMenu>>()
                .return_const(tokio::sync::Mutex::new(mock_storage_client));

            let state = State::try_from_transition(main_menu, list, &mock_context)
                .await
                .unwrap();
            assert!(matches!(
                state,
                State::Authorized(AuthorizedBox::WaitingForResourceName(_))
            ))
        }

        #[test]
        pub async fn main_menu_arbitrary_failure() {
            let main_menu = State::Authorized(AuthorizedBox::MainMenu(Authorized {
                kind: kind::MainMenu,
            }));
            let arbitrary = MessageBox::Arbitrary(crate::message::Arbitrary(
                "Test arbitrary message".to_owned(),
            ));

            test_unexpected_message(main_menu, arbitrary).await
        }

        #[test]
        pub async fn waiting_for_resource_name_sign_in_failure() {
            let waiting_for_resource_name =
                State::Authorized(AuthorizedBox::WaitingForResourceName(Authorized {
                    kind: kind::WaitingForResourceName,
                }));
            let sign_in = MessageBox::SignIn(crate::message::SignIn);

            test_unexpected_message(waiting_for_resource_name, sign_in).await
        }

        #[test]
        pub async fn waiting_for_resource_name_list_failure() {
            let waiting_for_resource_name =
                State::Authorized(AuthorizedBox::WaitingForResourceName(Authorized {
                    kind: kind::WaitingForResourceName,
                }));
            let list = MessageBox::List(crate::message::List);

            test_unexpected_message(waiting_for_resource_name, list).await
        }

        #[test]
        pub async fn waiting_for_resource_name_arbitrary_failure() {
            let waiting_for_resource_name =
                State::Authorized(AuthorizedBox::WaitingForResourceName(Authorized {
                    kind: kind::WaitingForResourceName,
                }));
            let arbitrary = MessageBox::Arbitrary(crate::message::Arbitrary(
                "Test arbitrary message".to_owned(),
            ));

            test_unexpected_message(waiting_for_resource_name, arbitrary).await
        }
    }
}
