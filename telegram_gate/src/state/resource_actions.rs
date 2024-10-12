//! [`Resource actions`](ResourceActions) state implementation.

use std::sync::Arc;

use base64::{engine::general_purpose::URL_SAFE, Engine as _};
use color_eyre::eyre::OptionExt;
#[cfg(not(test))]
use teloxide::{
    payloads::{
        EditMessageReplyMarkupSetters as _, EditMessageTextSetters as _, SendMessageSetters as _,
    },
    requests::Requester as _,
};
use teloxide::{types::MessageId, utils::markdown};
use tokio::sync::RwLock;
use tracing::debug;

use super::{
    delete_confirmation::DeleteConfirmation, resources_list::ResourcesList, Context,
    DisplayedResourceData,
};
use crate::{
    button::{self, Button},
    grpc,
    transition::{
        try_with_state, Destroy, FailedTransition, TransitionFailureReason, TryFromTransition,
    },
    TelegramMessageGettersExt as _,
};

/// State when bot is waiting for user to press some inline button
/// to make an action with a resource attached to a message.
#[derive(Debug, Clone)]
pub struct ResourceActions {
    /// Cached record data. Usable for transition back to
    /// [`super::resource_actions::ResourceActions`]
    record: grpc::Record,
    /// Currently displayed messages related to a resource.
    displayed_resource_data: Arc<RwLock<DisplayedResourceData>>,
}

impl Destroy for ResourceActions {
    async fn destroy(self, context: &Context) -> color_eyre::Result<()> {
        let Some(displayed_resource_data_lock) = Arc::into_inner(self.displayed_resource_data)
        else {
            debug!(
                "There are other strong references to `DisplayedResourceData`, skipping deletion"
            );
            return Ok(());
        };
        displayed_resource_data_lock
            .into_inner()
            .delete_messages(context)
            .await
    }
}

impl PartialEq for ResourceActions {
    /// [`Arc`] pointer comparison without accessing the inner value.
    fn eq(&self, other: &Self) -> bool {
        (&self.record, Arc::as_ptr(&self.displayed_resource_data))
            == (&other.record, Arc::as_ptr(&other.displayed_resource_data))
    }
}

impl Eq for ResourceActions {}

impl ResourceActions {
    /// Create a new [`ResourceActions`] state for tests.
    #[cfg(test)]
    pub fn test(displayed_resource_data: Arc<RwLock<DisplayedResourceData>>) -> Self {
        Self {
            record: grpc::Record {
                resource: Some(grpc::Resource {
                    name: "Test".to_owned(),
                }),
                encrypted_payload: b"unused".to_vec(),
                salt: b"unused".to_vec(),
            },
            displayed_resource_data,
        }
    }

    pub async fn from_resources_list(
        resources_list: ResourcesList,
        arbitrary_id: MessageId,
        record: grpc::Record,
        context: &Context,
    ) -> Result<Self, FailedTransition<ResourcesList>> {
        let actions_keyboard = Self::construct_actions_keyboard(&record, context);

        let resource_name = try_with_state!(
            resources_list,
            record
                .resource
                .as_ref()
                .map(|resource| resource.name.clone())
                .ok_or_eyre("Record doesn't contain a resource")
                .map_err(TransitionFailureReason::internal)
        );

        let cancel_message = try_with_state!(
            resources_list,
            context
                .bot()
                .send_message(context.chat_id(), "Type /cancel to go back.",)
                .reply_markup(teloxide::types::ReplyMarkup::kb_remove())
                .await
                .map_err(TransitionFailureReason::internal)
        );

        let message = try_with_state!(
            resources_list,
            context
                .bot()
                .send_message(
                    context.chat_id(),
                    Self::construct_choose_an_action_text(&resource_name),
                )
                .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                .reply_markup(actions_keyboard)
                .await
                .map_err(TransitionFailureReason::internal)
        );

        Ok(Self {
            record,
            displayed_resource_data: Arc::new(RwLock::new(DisplayedResourceData::new(
                arbitrary_id,
                cancel_message.id(),
                message.id(),
                resource_name,
            ))),
        })
    }

    /// Take record.
    pub fn take_record(self) -> grpc::Record {
        self.record
    }

    /// Get displayed resource data.
    pub fn displayed_resource_data(&self) -> Arc<RwLock<DisplayedResourceData>> {
        Arc::clone(&self.displayed_resource_data)
    }

    /// Construct text for a message with resource name and attached buttons with possible actions.
    fn construct_choose_an_action_text(resource_name: &str) -> String {
        format!(
            "🔑 {}\n\n\
             Choose an action:",
            markdown::bold(&markdown::escape(resource_name)),
        )
    }

    /// Construct keyboard with possible actions for a resource.
    #[allow(clippy::expect_used, reason = "indicates programmer error")]
    fn construct_actions_keyboard(
        record: &grpc::Record,
        context: &Context,
    ) -> teloxide::types::InlineKeyboardMarkup {
        let payload = URL_SAFE.encode(&record.encrypted_payload);
        let salt = URL_SAFE.encode(&record.salt);

        let resource_name_param = record
            .resource
            .as_ref()
            .map(|resource| format!("resource_name={}&", resource.name))
            .unwrap_or_default();

        let keyboard = teloxide::types::InlineKeyboardMarkup::new([[
            teloxide::types::InlineKeyboardButton::callback(
                button::kind::Delete.to_string(),
                button::kind::Delete.to_string(),
            ),
            teloxide::types::InlineKeyboardButton::web_app(
                button::kind::Show.to_string(),
                teloxide::types::WebAppInfo {
                    url: context
                        .web_app_url()
                        .clone()
                        .join(&format!(
                            "/show?{resource_name_param}payload={payload}&salt={salt}",
                        ))
                        .expect("Failed to join Web App url with `/show`"),
                },
            ),
        ]]);
        keyboard
    }
}

impl TryFromTransition<DeleteConfirmation, Button<button::kind::No>> for ResourceActions {
    type ErrorTarget = DeleteConfirmation;

    async fn try_from_transition(
        delete_confirmation: DeleteConfirmation,
        _no: Button<button::kind::No>,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        let resource_message_id;
        let resource_name;
        {
            let displayed_resource_data = delete_confirmation.displayed_resource_data();
            let displayed_resource_data = displayed_resource_data.read().await;

            resource_message_id = displayed_resource_data.resource_message_id;
            resource_name = displayed_resource_data.resource_name.clone();
        }
        let choose_an_action_text = Self::construct_choose_an_action_text(&resource_name);

        let actions_keyboard =
            Self::construct_actions_keyboard(delete_confirmation.record(), context);

        try_with_state!(
            delete_confirmation,
            context
                .bot()
                .edit_message_text(
                    context.chat_id(),
                    resource_message_id,
                    choose_an_action_text,
                )
                .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                .await
                .map_err(TransitionFailureReason::internal)
        );

        try_with_state!(
            delete_confirmation,
            context
                .bot()
                .edit_message_reply_markup(context.chat_id(), resource_message_id)
                .reply_markup(actions_keyboard)
                .await
                .map_err(TransitionFailureReason::internal)
        );

        Ok(Self {
            displayed_resource_data: delete_confirmation.displayed_resource_data(),
            record: delete_confirmation.take_record(),
        })
    }
}

#[cfg(test)]
pub mod tests {
    #![allow(clippy::panic, clippy::unwrap_used, reason = "it's ok in tests")]

    pub mod command {
        use tokio::test;

        use crate::{
            command::Command,
            state::State,
            test_utils::{test_help_success, test_unavailable_command},
        };

        #[test]
        pub async fn help_success() {
            let resource_actions = State::resource_actions(true);

            test_help_success(resource_actions).await
        }

        #[test]
        pub async fn start_failure() {
            let resource_actions = State::resource_actions(true);
            let start = Command::start();

            test_unavailable_command(resource_actions, start).await
        }
    }

    pub mod message {
        use mockall::predicate;
        use tokio::test;

        use crate::{
            grpc,
            message::{Message, MessageBox},
            state::{Context, State},
            test_utils::{
                mock_bot::{MockBotBuilder, CHAT_ID},
                test_unexpected_message, web_app_test_url,
            },
            transition::TryFromTransition as _,
        };

        #[test]
        pub async fn from_resources_list_by_existing_resource_success() {
            const RESOURCE_MSG_ID: i32 = 40;
            const CANCEL_MSG_ID: i32 = 41;
            const RESOURCE_ACTIONS_MSG_ID: i32 = 42;

            let resources_list = State::resources_list();

            let resource_name_msg = MessageBox::Arbitrary(Message {
                id: teloxide::types::MessageId(RESOURCE_MSG_ID),
                kind: crate::message::kind::Arbitrary("🔑 test.resource.com".to_owned()),
            });

            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);
            mock_context
                .expect_web_app_url()
                .return_const(web_app_test_url());

            mock_context.expect_bot().return_const(
                MockBotBuilder::new()
                    .expect_send_message("Type /cancel to go back.")
                    .expect_reply_markup(teloxide::types::ReplyMarkup::kb_remove())
                    .expect_into_future_with_id(teloxide::types::MessageId(CANCEL_MSG_ID))
                    .expect_send_message(
                        "🔑 *test\\.resource\\.com*\n\n\
                         Choose an action:"
                            .to_owned(),
                    )
                    .expect_parse_mode(teloxide::types::ParseMode::MarkdownV2)
                    .expect_reply_markup(teloxide::types::InlineKeyboardMarkup::new([[
                        teloxide::types::InlineKeyboardButton::callback(
                            crate::button::kind::Delete.to_string(),
                            crate::button::kind::Delete.to_string(),
                        ),
                        teloxide::types::InlineKeyboardButton::web_app(
                            "👀 Show",
                            teloxide::types::WebAppInfo {
                                url: web_app_test_url()
                                    .join("/show?resource_name=test.resource.com&payload=dW51c2Vk&salt=dW51c2Vk")
                                    .unwrap(),
                            },
                        ),
                    ]]))
                    .expect_into_future_with_id(teloxide::types::MessageId(RESOURCE_ACTIONS_MSG_ID))
                    .build(),
            );

            let mut mock_storage_client = crate::PasswordStorageClient::default();
            mock_storage_client
                .expect_get::<grpc::Resource>()
                .with(predicate::eq(grpc::Resource {
                    name: "test.resource.com".to_owned(),
                }))
                .returning(|resource| {
                    Ok(tonic::Response::new(grpc::Record {
                        resource: Some(resource),
                        encrypted_payload: b"unused".to_vec(),
                        salt: b"unused".to_vec(),
                    }))
                });
            mock_context
                .expect_storage_client()
                .return_const(tokio::sync::Mutex::new(mock_storage_client));

            let state =
                State::try_from_transition(resources_list, resource_name_msg, &mock_context)
                    .await
                    .unwrap();
            let State::ResourceActions(resource_actions) = state else {
                panic!("Expected `State::ResourceActions`, got {state:?}");
            };
            resource_actions
                .displayed_resource_data
                .write()
                .await
                .bomb
                .defuse();
        }

        #[test]
        pub async fn web_app_failure() {
            let resource_actions = State::resource_actions(true);
            let web_app = MessageBox::web_app("data".to_owned(), "button_text".to_owned());

            test_unexpected_message(resource_actions, web_app).await
        }

        #[test]
        pub async fn list_failure() {
            let resource_actions = State::resource_actions(true);

            let list = MessageBox::list();

            test_unexpected_message(resource_actions, list).await
        }

        #[test]
        pub async fn add_failure() {
            let resource_actions = State::resource_actions(true);

            let add = MessageBox::add();

            test_unexpected_message(resource_actions, add).await
        }

        #[test]
        pub async fn arbitrary_failure() {
            let resource_actions = State::resource_actions(true);

            let arbitrary = MessageBox::arbitrary("Test arbitrary message");

            test_unexpected_message(resource_actions, arbitrary).await
        }
    }

    pub mod button {
        use std::sync::Arc;

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
        };

        #[test]
        pub async fn show_failure() {
            let resource_actions = State::resource_actions(true);
            let show_button = ButtonBox::show();

            test_unexpected_button(resource_actions, show_button).await;
        }

        #[test]
        pub async fn yes_failure() {
            let resource_actions = State::resource_actions(true);
            let yes_button = ButtonBox::yes();

            test_unexpected_button(resource_actions, yes_button).await;
        }

        #[test]
        pub async fn no_failure() {
            let resource_actions = State::resource_actions(true);
            let no_button = ButtonBox::no();

            test_unexpected_button(resource_actions, no_button).await;
        }

        #[test]
        pub async fn from_delete_confirmation_by_no_success() {
            let resource_message_id = teloxide::types::MessageId(602);

            let delete_confirmation = State::DeleteConfirmation(
                DeleteConfirmation::test(Arc::new(RwLock::new(DisplayedResourceData::new(
                    teloxide::types::MessageId(600),
                    teloxide::types::MessageId(602),
                    resource_message_id,
                    "test.resource.com".to_owned(),
                ))))
                .await,
            );
            let no_button = ButtonBox::no();

            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);
            mock_context
                .expect_web_app_url()
                .return_const(web_app_test_url());

            mock_context.expect_bot().return_const(
                MockBotBuilder::new()
                    .expect_edit_message_text(
                        resource_message_id,
                        "🔑 *test\\.resource\\.com*\n\n\
                         Choose an action:"
                            .to_owned(),
                    )
                    .expect_parse_mode(teloxide::types::ParseMode::MarkdownV2)
                    .expect_into_future()
                    .expect_edit_message_reply_markup(resource_message_id)
                    .expect_reply_markup(teloxide::types::InlineKeyboardMarkup::new([[
                        teloxide::types::InlineKeyboardButton::callback(
                            crate::button::kind::Delete.to_string(),
                            crate::button::kind::Delete.to_string(),
                        ),
                        teloxide::types::InlineKeyboardButton::web_app(
                            "👀 Show",
                            teloxide::types::WebAppInfo {
                                url: web_app_test_url()
                                    .join("/show?resource_name=test.resource.com&payload=dW51c2Vk&salt=dW51c2Vk")
                                    .unwrap(),
                            },
                        ),
                    ]]))
                    .expect_into_future()
                    .build(),
            );

            let state = State::try_from_transition(delete_confirmation, no_button, &mock_context)
                .await
                .unwrap();
            let State::ResourceActions(resource_actions) = state else {
                panic!("Expected `State::ResourceActions`, got {state:?}");
            };
            resource_actions
                .displayed_resource_data
                .write()
                .await
                .bomb
                .defuse();
        }
    }
}
