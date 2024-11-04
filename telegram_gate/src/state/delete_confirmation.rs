//! [`Delete confirmation`](DeleteConfirmation) state implementation.

use std::sync::Arc;

use teloxide::utils::markdown;
#[cfg(not(test))]
use teloxide::{
    payloads::{EditMessageReplyMarkupSetters as _, EditMessageTextSetters as _},
    requests::Requester as _,
};
use tokio::sync::RwLock;
use tracing::debug;

use super::{resource_actions::ResourceActions, Context, DisplayedResourceData};
use crate::{
    button::{self, Button},
    grpc,
    transition::{
        try_with_state, Destroy, FailedTransition, TransitionFailureReason, TryFromTransition,
    },
};

/// State when bot is waiting for user to confirm resource deletion
/// or to cancel the operation.
#[derive(Debug, Clone)]
pub struct DeleteConfirmation {
    /// Cached record data. Usable for transition back to
    /// [`super::resource_actions::ResourceActions`]
    record: grpc::Record,
    /// Currently displayed messages related to a resource.
    displayed_resource_data: Arc<RwLock<DisplayedResourceData>>,
}

impl DeleteConfirmation {
    /// Create a new [`DeleteConfirmation`] state for tests.
    #[cfg(test)]
    pub async fn test(displayed_resource_data: Arc<RwLock<DisplayedResourceData>>) -> Self {
        let resource_name = displayed_resource_data.read().await.resource_name.clone();
        Self {
            record: grpc::Record {
                resource: Some(grpc::Resource {
                    name: resource_name,
                }),
                encrypted_payload: b"unused".to_vec(),
                salt: b"unused".to_vec(),
            },
            displayed_resource_data,
        }
    }

    /// Get record.
    pub const fn record(&self) -> &grpc::Record {
        &self.record
    }

    /// Take record.
    pub fn take_record(self) -> grpc::Record {
        self.record
    }

    /// Get displayed resource data.
    pub fn displayed_resource_data(&self) -> Arc<RwLock<DisplayedResourceData>> {
        Arc::clone(&self.displayed_resource_data)
    }
}

impl Destroy for DeleteConfirmation {
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

impl PartialEq for DeleteConfirmation {
    /// [`Arc`] pointer comparison without accessing the inner value.
    fn eq(&self, other: &Self) -> bool {
        (&self.record, Arc::as_ptr(&self.displayed_resource_data))
            == (&other.record, Arc::as_ptr(&other.displayed_resource_data))
    }
}

impl Eq for DeleteConfirmation {}

impl TryFromTransition<ResourceActions, Button<button::kind::Delete>> for DeleteConfirmation {
    type ErrorTarget = ResourceActions;

    async fn try_from_transition(
        resource_actions: ResourceActions,
        _delete_button: Button<button::kind::Delete>,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        let resource_message_id = resource_actions
            .displayed_resource_data()
            .read()
            .await
            .resource_message_id;

        try_with_state!(
            resource_actions,
            context
                .bot()
                .edit_message_text(
                    context.chat_id(),
                    resource_message_id,
                    format!(
                        "🗑 Delete {} forever?",
                        markdown::bold(&markdown::escape(
                            &resource_actions
                                .displayed_resource_data()
                                .read()
                                .await
                                .resource_name
                        ))
                    )
                )
                .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                .await
                .map_err(TransitionFailureReason::internal)
        );

        try_with_state!(
            resource_actions,
            context
                .bot()
                .edit_message_reply_markup(context.chat_id(), resource_message_id)
                .reply_markup(teloxide::types::InlineKeyboardMarkup::new([[
                    button::kind::Yes.to_string(),
                    button::kind::No.to_string()
                ]
                .map(
                    |button_data| teloxide::types::InlineKeyboardButton::callback(
                        button_data.clone(),
                        button_data
                    )
                )]))
                .await
                .map_err(TransitionFailureReason::internal)
        );

        Ok(Self {
            displayed_resource_data: resource_actions.displayed_resource_data(),
            record: resource_actions.take_record(),
        })
    }
}

#[cfg(test)]
pub mod tests {
    #![expect(clippy::panic, clippy::unwrap_used, reason = "it's ok in tests")]

    pub mod command {
        use tokio::test;

        use crate::{
            command::Command,
            state::State,
            test_utils::{test_help_success, test_unavailable_command},
        };

        #[test]
        pub async fn help_success() {
            let delete_confirmation = State::delete_confirmation(true).await;

            test_help_success(delete_confirmation).await
        }

        #[test]
        pub async fn start_failure() {
            let delete_confirmation = State::delete_confirmation(true).await;
            let start = Command::start();

            test_unavailable_command(delete_confirmation, start).await
        }
    }

    pub mod message {
        use tokio::test;

        use crate::{message::MessageBox, state::State, test_utils::test_unexpected_message};

        #[test]
        pub async fn web_app_failure() {
            let delete_confirmation = State::delete_confirmation(true).await;
            let web_app = MessageBox::web_app("data".to_owned(), "button_text".to_owned());

            test_unexpected_message(delete_confirmation, web_app).await
        }

        #[test]
        pub async fn list_failure() {
            let delete_confirmation = State::delete_confirmation(true).await;
            let list = MessageBox::list();

            test_unexpected_message(delete_confirmation, list).await
        }

        #[test]
        pub async fn add_failure() {
            let delete_confirmation = State::delete_confirmation(true).await;
            let add = MessageBox::add();

            test_unexpected_message(delete_confirmation, add).await
        }

        #[test]
        pub async fn arbitrary_failure() {
            let delete_confirmation = State::delete_confirmation(true).await;
            let arbitrary = MessageBox::arbitrary("Test arbitrary message");

            test_unexpected_message(delete_confirmation, arbitrary).await
        }
    }

    pub mod button {
        use std::sync::Arc;

        use tokio::{sync::RwLock, test};

        use crate::{
            button::ButtonBox,
            state::{resource_actions::ResourceActions, Context, DisplayedResourceData, State},
            test_utils::{
                mock_bot::{MockBotBuilder, CHAT_ID},
                test_unexpected_button,
            },
            transition::TryFromTransition as _,
        };

        #[test]
        pub async fn from_resource_actions_by_delete_success() {
            let resource_message_id = teloxide::types::MessageId(578);

            let resource_actions = State::ResourceActions(ResourceActions::test(Arc::new(
                RwLock::new(DisplayedResourceData::new(
                    teloxide::types::MessageId(576),
                    teloxide::types::MessageId(577),
                    resource_message_id,
                    "test.resource.com".to_owned(),
                )),
            )));
            let delete_button = ButtonBox::delete();

            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);

            let expected_reply_markup = teloxide::types::InlineKeyboardMarkup::new([[
                crate::button::kind::Yes.to_string(),
                crate::button::kind::No.to_string(),
            ]
            .map(|button_data| {
                teloxide::types::InlineKeyboardButton::callback(button_data.clone(), button_data)
            })]);
            mock_context.expect_bot().return_const(
                MockBotBuilder::new()
                    .expect_edit_message_text(
                        resource_message_id,
                        "🗑 Delete *test\\.resource\\.com* forever?".to_owned(),
                    )
                    .expect_parse_mode(teloxide::types::ParseMode::MarkdownV2)
                    .expect_into_future()
                    .expect_edit_message_reply_markup(resource_message_id)
                    .expect_reply_markup(expected_reply_markup)
                    .expect_into_future()
                    .build(),
            );

            let state = State::try_from_transition(resource_actions, delete_button, &mock_context)
                .await
                .unwrap();
            let State::DeleteConfirmation(delete_confirmation) = state else {
                panic!("Expected `State::DeleteConfirmation`, got {state:?}");
            };
            delete_confirmation
                .displayed_resource_data
                .write()
                .await
                .bomb
                .defuse();
        }

        #[test]
        pub async fn show_failure() {
            let delete_confirmation = State::delete_confirmation(true).await;
            let show_button = ButtonBox::show();

            test_unexpected_button(delete_confirmation, show_button).await;
        }

        #[test]
        pub async fn delete_failure() {
            let delete_confirmation = State::delete_confirmation(true).await;
            let delete_button = ButtonBox::delete();

            test_unexpected_button(delete_confirmation, delete_button).await;
        }
    }
}
