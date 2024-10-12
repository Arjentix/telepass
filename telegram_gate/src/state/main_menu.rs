//! [`Main menu`](MainMenu) state implementation.

#[cfg(not(test))]
use teloxide::{payloads::SendMessageSetters as _, requests::Requester as _};
use teloxide::{
    types::{KeyboardButton, KeyboardMarkup},
    utils::markdown,
};

use super::{delete_confirmation::DeleteConfirmation, resources_list::ResourcesList, Context};
use crate::{
    button::{self, Button},
    command,
    message::{self, Message},
    transition::{
        try_with_state, Destroy, FailedTransition, TransitionFailureReason, TryFromTransition,
    },
};

/// Main menu state.
///
/// Waits for user to input an action.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct MainMenu(());

impl MainMenu {
    /// Create a new [`MainMenu`] state for tests.
    #[cfg(test)]
    pub const fn test() -> Self {
        Self(())
    }

    /// Setup [`MainMenu`] state.
    ///
    /// Prints welcome message and constructs a keyboard with all supported actions.
    async fn setup<P>(prev_state: P, context: &Context) -> Result<Self, FailedTransition<P>>
    where
        P: Send,
    {
        let main_menu = try_with_state!(prev_state, Self::setup_impl(context).await);
        Ok(main_menu)
    }

    /// [`setup()`](Self::setup) analog with destroying previous state.
    async fn setup_destroying<P>(
        prev_state: P,
        context: &Context,
    ) -> Result<Self, FailedTransition<P>>
    where
        P: Destroy + Send,
    {
        let main_menu = try_with_state!(prev_state, Self::setup_impl(context).await);

        prev_state.destroy_and_log_err(context).await;
        Ok(main_menu)
    }

    /// [`setup()`](Self::setup) and [`setup_destroying()`](Self::setup_destroying) implementation.
    #[allow(clippy::expect_used, reason = "indicates programmer error")]
    async fn setup_impl(context: &Context) -> Result<Self, TransitionFailureReason> {
        let buttons = [
            [KeyboardButton::new(message::kind::List.to_string())],
            [KeyboardButton::new(message::kind::Add.to_string()).request(
                teloxide::types::ButtonRequest::WebApp(teloxide::types::WebAppInfo {
                    url: context
                        .web_app_url()
                        .clone()
                        .join("/submit")
                        .expect("Failed to join Web App url with `/show`"),
                }),
            )],
        ];
        let keyboard = KeyboardMarkup::new(buttons).resize_keyboard(true);

        context
            .bot()
            .send_message(context.chat_id(), "🏠 Welcome to the main menu.")
            .reply_markup(keyboard)
            .await
            .map_err(TransitionFailureReason::internal)?;

        Ok(Self(()))
    }
}

impl TryFromTransition<super::default::Default, command::Start> for MainMenu {
    type ErrorTarget = super::default::Default;

    async fn try_from_transition(
        default: super::default::Default,
        _start: command::Start,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        Self::setup(default, context).await
    }
}

impl TryFromTransition<ResourcesList, command::Cancel> for MainMenu {
    type ErrorTarget = ResourcesList;

    async fn try_from_transition(
        resources_list: ResourcesList,
        _cancel: command::Cancel,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        Self::setup(resources_list, context).await
    }
}

impl TryFromTransition<Self, Message<message::kind::WebApp>> for MainMenu {
    type ErrorTarget = Self;

    async fn try_from_transition(
        main_menu: Self,
        web_app_msg: Message<message::kind::WebApp>,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        let teloxide::types::WebAppData { data, button_text } = web_app_msg.kind.0;
        if button_text != message::kind::Add.to_string() {
            return Err(FailedTransition::user(
                main_menu,
                "Unexpected WebApp button text.",
            ));
        }

        let record: telepass_data_model::NewRecord = try_with_state!(
            main_menu,
            serde_json::from_str(&data).map_err(|_err| TransitionFailureReason::user(
                "Failed to parse a new record, your Telegram Client is probably invalid."
            ))
        );
        let record = crate::grpc::Record::from(record);

        try_with_state!(
            main_menu,
            context
                .storage_client()
                .lock()
                .await
                .add(record)
                .await
                .map_err(TransitionFailureReason::internal)
        );

        Ok(main_menu)
    }
}

impl TryFromTransition<DeleteConfirmation, Button<button::kind::Yes>> for MainMenu {
    type ErrorTarget = DeleteConfirmation;

    async fn try_from_transition(
        delete_confirmation: DeleteConfirmation,
        _yes: Button<button::kind::Yes>,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        let resource_name = delete_confirmation
            .displayed_resource_data()
            .read()
            .await
            .resource_name
            .clone();

        try_with_state!(
            delete_confirmation,
            context
                .storage_client()
                .lock()
                .await
                .delete(crate::grpc::Resource {
                    name: resource_name.clone(),
                })
                .await
                .map_err(TransitionFailureReason::internal)
        );

        try_with_state!(
            delete_confirmation,
            context
                .bot()
                .send_message(
                    context.chat_id(),
                    format!(
                        "✅ {} deleted\\.",
                        markdown::bold(&markdown::escape(&resource_name))
                    )
                )
                .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                .await
                .map_err(TransitionFailureReason::internal)
        );

        Self::setup_destroying(delete_confirmation, context).await
    }
}

#[cfg(test)]
pub mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, reason = "it's ok in tests")]

    pub mod command {
        use teloxide::types::{KeyboardButton, KeyboardMarkup};
        use tokio::test;

        use crate::{
            command::Command,
            state::{Context, State},
            test_utils::{
                mock_bot::{MockBotBuilder, CHAT_ID},
                test_help_success, test_unavailable_command, web_app_test_url,
            },
            transition::TryFromTransition as _,
        };

        async fn test_main_menu_setup(state: State, cmd: Command) {
            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);
            mock_context
                .expect_web_app_url()
                .return_const(web_app_test_url());
            mock_context.expect_bot().return_const(
                MockBotBuilder::new()
                    .expect_send_message("🏠 Welcome to the main menu.")
                    .expect_reply_markup(
                        KeyboardMarkup::new([
                            [KeyboardButton::new(crate::message::kind::List.to_string())],
                            [
                                KeyboardButton::new(crate::message::kind::Add.to_string()).request(
                                    teloxide::types::ButtonRequest::WebApp(
                                        teloxide::types::WebAppInfo {
                                            url: web_app_test_url().join("/submit").unwrap(),
                                        },
                                    ),
                                ),
                            ],
                        ])
                        .resize_keyboard(true),
                    )
                    .expect_into_future()
                    .build(),
            );

            let state = State::try_from_transition(state, cmd, &mock_context)
                .await
                .unwrap();
            assert!(matches!(state, State::MainMenu(_)))
        }

        #[test]
        pub async fn help_success() {
            let main_menu = State::main_menu();

            test_help_success(main_menu).await
        }

        #[test]
        pub async fn start_failure() {
            let main_menu = State::main_menu();
            let start = Command::start();

            test_unavailable_command(main_menu, start).await
        }

        #[test]
        pub async fn cancel_failure() {
            let main_menu = State::main_menu();
            let cancel = Command::cancel();

            test_unavailable_command(main_menu, cancel).await
        }

        #[test]
        pub async fn from_default_by_start_success() {
            let default = State::default();
            let start = Command::start();

            test_main_menu_setup(default, start).await
        }

        #[test]
        pub async fn from_resources_list_by_cancel_success() {
            let resources_list = State::resources_list();
            let cancel = Command::cancel();

            test_main_menu_setup(resources_list, cancel).await
        }
    }

    pub mod message {
        use mockall::predicate;
        use tokio::test;

        use crate::{
            message::MessageBox,
            state::{Context, State},
            test_utils::{mock_bot::MockBotBuilder, test_unexpected_message},
            transition::{TransitionFailureReason, TryFromTransition as _},
        };

        #[test]
        pub async fn web_app_success() {
            let main_menu = State::main_menu();

            let record = telepass_data_model::NewRecord {
                resource_name: "test.resource.com".to_owned(),
                encryption_output: telepass_data_model::crypto::EncryptionOutput {
                    encrypted_payload: b"SomeSecret".to_vec(),
                    salt: [1; telepass_data_model::crypto::SALT_SIZE],
                },
            };
            let web_app = MessageBox::web_app(
                serde_json::to_string(&record).expect("Failed to serialize record"),
                "🆕 Add".to_owned(),
            );

            let mut mock_context = Context::default();

            mock_context
                .expect_bot()
                .return_const(MockBotBuilder::new().build());

            let mut mock_storage_client = crate::PasswordStorageClient::default();
            mock_storage_client
                .expect_add::<crate::grpc::Record>()
                .with(predicate::eq(crate::grpc::Record::from(record)))
                .returning(|_record| Ok(tonic::Response::new(crate::grpc::Response {})));

            mock_context
                .expect_storage_client()
                .return_const(tokio::sync::Mutex::new(mock_storage_client));

            let state = State::try_from_transition(main_menu.clone(), web_app, &mock_context)
                .await
                .unwrap();
            assert_eq!(state, main_menu)
        }

        #[test]
        pub async fn web_app_wrong_data_failure() {
            let main_menu = State::main_menu();

            let record_json = serde_json::json!({
                "wrong_field": "test.resource.com",
                "encrypted_payload": "SomeSecret",
                "salt": "TestSalt"
            });
            let web_app = MessageBox::web_app(record_json.to_string(), "🆕 Add".to_owned());

            let mock_context = Context::default();

            let err = State::try_from_transition(main_menu.clone(), web_app, &mock_context)
                .await
                .unwrap_err();
            assert!(matches!(
                err.reason,
                TransitionFailureReason::User(message) if message == "Failed to parse a new record, your Telegram Client is probably invalid.",
            ))
        }

        #[test]
        pub async fn web_app_wrong_button_text_failure() {
            let main_menu = State::main_menu();

            let record_json = serde_json::json!({
                "resource_name": "test.resource.com",
                "encrypted_payload": "SomeSecret",
                "salt": "TestSalt"
            });
            let web_app =
                MessageBox::web_app(record_json.to_string(), "Wrong Button Text".to_owned());

            let mock_context = Context::default();

            let err = State::try_from_transition(main_menu.clone(), web_app, &mock_context)
                .await
                .unwrap_err();
            assert!(matches!(
                err.reason,
                TransitionFailureReason::User(message) if message == "Unexpected WebApp button text.",
            ))
        }

        #[test]
        pub async fn add_failure() {
            let main_menu = State::main_menu();
            let add = MessageBox::add();

            test_unexpected_message(main_menu, add).await
        }

        #[test]
        pub async fn arbitrary_failure() {
            let main_menu = State::main_menu();
            let arbitrary = MessageBox::arbitrary("Test arbitrary message");

            test_unexpected_message(main_menu, arbitrary).await
        }
    }

    pub mod button {
        use std::sync::Arc;

        use mockall::predicate;
        use teloxide::types::{KeyboardButton, KeyboardMarkup, MessageId};
        use tokio::{sync::RwLock, test};

        use crate::{
            button::ButtonBox,
            state::{
                delete_confirmation::DeleteConfirmation, Context, DisplayedResourceData, State,
            },
            test_utils::{
                mock_bot::{MockBotBuilder, CHAT_ID},
                test_unexpected_button, web_app_test_url,
            },
            transition::TryFromTransition as _,
            PasswordStorageClient,
        };

        #[test]
        pub async fn delete_failure() {
            let main_menu = State::main_menu();
            let delete_button = ButtonBox::delete();

            test_unexpected_button(main_menu, delete_button).await;
        }

        #[test]
        pub async fn yes_failure() {
            let main_menu = State::main_menu();
            let yes_button = ButtonBox::yes();

            test_unexpected_button(main_menu, yes_button).await;
        }

        #[test]
        pub async fn no_failure() {
            let main_menu = State::main_menu();
            let no_button = ButtonBox::no();

            test_unexpected_button(main_menu, no_button).await;
        }

        #[test]
        pub async fn show_failure() {
            let main_menu = State::main_menu();
            let show_button = ButtonBox::show();

            test_unexpected_button(main_menu, show_button).await;
        }

        #[test]
        pub async fn from_delete_confirmation_by_yes_success() {
            const REQUEST_MESSAGE_ID: i32 = 200;
            const CANCEL_MESSAGE_ID: i32 = 201;
            const RESOURCE_MESSAGE_ID: i32 = 202;

            let delete_confirmation = State::DeleteConfirmation(
                DeleteConfirmation::test(Arc::new(RwLock::new(DisplayedResourceData::new(
                    teloxide::types::MessageId(REQUEST_MESSAGE_ID),
                    teloxide::types::MessageId(CANCEL_MESSAGE_ID),
                    teloxide::types::MessageId(RESOURCE_MESSAGE_ID),
                    "test.resource.com".to_owned(),
                ))))
                .await,
            );

            let yes_button = ButtonBox::yes();

            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);
            mock_context
                .expect_web_app_url()
                .return_const(web_app_test_url());
            mock_context.expect_bot().return_const(
                MockBotBuilder::new()
                    .expect_send_message("✅ *test\\.resource\\.com* deleted\\.".to_owned())
                    .expect_parse_mode(teloxide::types::ParseMode::MarkdownV2)
                    .expect_into_future()
                    .expect_send_message("🏠 Welcome to the main menu.")
                    .expect_reply_markup(
                        KeyboardMarkup::new([
                            [KeyboardButton::new(crate::message::kind::List.to_string())],
                            [
                                KeyboardButton::new(crate::message::kind::Add.to_string()).request(
                                    teloxide::types::ButtonRequest::WebApp(
                                        teloxide::types::WebAppInfo {
                                            url: web_app_test_url().join("/submit").unwrap(),
                                        },
                                    ),
                                ),
                            ],
                        ])
                        .resize_keyboard(true),
                    )
                    .expect_into_future()
                    .expect_delete_message(MessageId(REQUEST_MESSAGE_ID))
                    .expect_delete_message(MessageId(CANCEL_MESSAGE_ID))
                    .expect_delete_message(MessageId(RESOURCE_MESSAGE_ID))
                    .build(),
            );

            let mut mock_storage_client = PasswordStorageClient::default();
            mock_storage_client
                .expect_delete()
                .with(predicate::eq(crate::grpc::Resource {
                    name: "test.resource.com".to_owned(),
                }))
                .returning(|_resource| Ok(tonic::Response::new(crate::grpc::Response {})));
            mock_context
                .expect_storage_client()
                .return_const(tokio::sync::Mutex::new(mock_storage_client));

            let state = State::try_from_transition(delete_confirmation, yes_button, &mock_context)
                .await
                .unwrap();
            assert!(matches!(state, State::MainMenu(_)))
        }
    }
}
