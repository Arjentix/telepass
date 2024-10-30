//! [`Resources list`](ResourcesList) state implementation.

use std::fmt::Debug;

use color_eyre::eyre::Context as _;
use nonempty::NonEmpty;
use teloxide::types::{KeyboardButton, KeyboardMarkup};
#[cfg(not(test))]
use teloxide::{payloads::SendMessageSetters as _, requests::Requester as _};

use super::{
    delete_confirmation::DeleteConfirmation, main_menu::MainMenu,
    resource_actions::ResourceActions, Context,
};
use crate::{
    command, grpc,
    message::{self, Message},
    transition::{
        try_with_state, Destroy, FailedTransition, TransitionFailureReason, TryFromTransition,
    },
};

/// State when bot is waiting for user to input a resource name from list.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ResourcesList(());

impl ResourcesList {
    /// Create a new [`ResourcesList`] state for tests.
    #[cfg(test)]
    pub const fn test() -> Self {
        Self(())
    }

    /// Setup [`ResourcesList`] state from previous state.
    ///
    /// Constructs a keyboard with resources for all stored passwords.
    ///
    /// # Errors
    ///
    /// Fails if:
    /// - Unable to retrieve the list of stored resources;
    /// - There are no stored resources;
    /// - Unable to send a message.
    async fn from_state<P>(prev_state: P, context: &Context) -> Result<Self, FailedTransition<P>>
    where
        P: Debug + Send + Sync + 'static,
    {
        let resources_list = try_with_state!(prev_state, Self::from_state_impl(context).await);
        Ok(resources_list)
    }

    /// [`from_state()`](Self::from_state) analog with destroying previous state.
    async fn from_state_destroying<P>(
        prev_state: P,
        context: &Context,
    ) -> Result<Self, FailedTransition<P>>
    where
        P: Debug + Destroy + Send + Sync + 'static,
    {
        let resources_list = try_with_state!(prev_state, Self::from_state_impl(context).await);

        prev_state.destroy_and_log_err(context).await;
        Ok(resources_list)
    }

    /// [`from_state()`](Self::from_state) and
    /// [`from_state_destroying()`](Self::from_state_destroying) implementation.
    async fn from_state_impl(context: &Context) -> Result<Self, TransitionFailureReason> {
        let resources = context
            .storage_client()
            .lock()
            .await
            .list(grpc::Empty {})
            .await
            .wrap_err("Failed to retrieve the list of stored passwords")
            .map_err(TransitionFailureReason::internal)?
            .into_inner()
            .resources;

        let resources = NonEmpty::from_vec(resources).ok_or_else(|| {
            TransitionFailureReason::User("❎ There are no stored passwords yet.".to_owned())
        })?;

        Self::from_resources(
            resources,
            context,
            "👉 Choose a resource or type for search.",
        )
        .await
    }

    /// Construct [`ResourcesList`] state with given resources.
    async fn from_resources(
        resources: NonEmpty<grpc::Resource>,
        context: &Context,
        message: &'static str,
    ) -> Result<Self, TransitionFailureReason> {
        let buttons = resources
            .into_iter()
            .map(|resource| [KeyboardButton::new(format!("🔑 {}", resource.name))]);
        let keyboard = KeyboardMarkup::new(buttons).resize_keyboard();

        context
            .bot()
            .send_message(
                context.chat_id(),
                format!("{message}\n\nType /cancel to go back."),
            )
            .reply_markup(keyboard)
            .await
            .map_err(TransitionFailureReason::internal)?;

        Ok(Self(()))
    }
}

impl TryFromTransition<MainMenu, Message<message::kind::List>> for ResourcesList {
    type ErrorTarget = MainMenu;

    async fn try_from_transition(
        main_menu: MainMenu,
        _list: Message<message::kind::List>,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        Self::from_state(main_menu, context).await
    }
}

/// Result of [`arbitrary`](message::kind::Arbitrary) message sent on [`ResourcesList`] state.
pub enum SearchResultsOrResourceActions {
    /// List of found resources if arbitrary message was a search query.
    SearchResults(ResourcesList),
    /// Actions for a resource if arbitrary message was a resource name chosen from keyboard.
    ResourceActions(ResourceActions),
}

impl TryFromTransition<ResourcesList, Message<message::kind::Arbitrary>>
    for SearchResultsOrResourceActions
{
    type ErrorTarget = ResourcesList;

    async fn try_from_transition(
        resources_list: ResourcesList,
        arbitrary: Message<message::kind::Arbitrary>,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        let resource_name = arbitrary.to_string();
        let resource_name = resource_name.strip_prefix("🔑 ").unwrap_or(&resource_name);

        let res = context
            .storage_client()
            .lock()
            .await
            .get(grpc::Resource {
                name: resource_name.to_owned(),
            })
            .await;

        match res {
            Ok(response) => Ok(Self::ResourceActions(
                ResourceActions::from_resources_list(
                    resources_list,
                    arbitrary.id,
                    response.into_inner(),
                    context,
                )
                .await?,
            )),
            Err(status) if status.code() == tonic::Code::NotFound => {
                let search_results = try_with_state!(
                    resources_list,
                    context
                        .storage_client()
                        .lock()
                        .await
                        .search(grpc::Resource {
                            name: resource_name.to_owned(),
                        })
                        .await
                        .wrap_err_with(|| format!("Failed to search for `{resource_name}`"))
                        .map_err(TransitionFailureReason::internal)
                );
                let search_results = search_results.into_inner().resources;
                let search_results = NonEmpty::from_vec(search_results).ok_or_else(|| {
                    FailedTransition::user(
                        resources_list,
                        "❎ No passwords found for a given query.",
                    )
                })?;

                let search_results_list = try_with_state!(
                    resources_list,
                    ResourcesList::from_resources(search_results, context,
                        "👉 The following resources were found, choose one of them or type for a new search.",
                    ).await
                );
                Ok(Self::SearchResults(search_results_list))
            }
            Err(status) => Err(FailedTransition::internal(resources_list, status)),
        }
    }
}

impl TryFromTransition<ResourceActions, command::Cancel> for ResourcesList {
    type ErrorTarget = ResourceActions;

    async fn try_from_transition(
        resource_actions: ResourceActions,
        _cancel: command::Cancel,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        Self::from_state_destroying(resource_actions, context).await
    }
}

impl TryFromTransition<DeleteConfirmation, command::Cancel> for ResourcesList {
    type ErrorTarget = DeleteConfirmation;

    async fn try_from_transition(
        delete_confirmation: DeleteConfirmation,
        _cancel: command::Cancel,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        Self::from_state_destroying(delete_confirmation, context).await
    }
}

#[cfg(test)]
pub mod tests {
    #![allow(clippy::unwrap_used, reason = "it's ok in tests")]

    pub mod command {
        use std::{future::ready, sync::Arc};

        use mockall::predicate;
        use teloxide::types::{KeyboardButton, KeyboardMarkup, MessageId};
        use tokio::{sync::RwLock, test};

        use crate::{
            command::Command,
            grpc,
            state::{
                delete_confirmation::DeleteConfirmation, resource_actions::ResourceActions,
                Context, DisplayedResourceData, State,
            },
            test_utils::{
                mock_bot::{MockBotBuilder, MockSendMessage, CHAT_ID},
                test_help_success, test_unavailable_command,
            },
            transition::TryFromTransition as _,
            TelegramMessage,
        };

        async fn test_resources_actions_setup(
            from_state: State,
            command: Command,
            mut mock_bot: crate::Bot,
        ) {
            const RESOURCE_NAMES: [&str; 3] = [
                "1.test.resource.com",
                "2.test.resource.com",
                "3.test.resource.com",
            ];

            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);

            mock_bot
                .expect_send_message::<_, String>()
                .with(
                    predicate::eq(CHAT_ID),
                    predicate::eq(
                        "👉 Choose a resource or type for search.\n\n\
                         Type /cancel to go back."
                            .to_owned(),
                    ),
                )
                .return_once(|_, _| {
                    let mut mock_send_message = MockSendMessage::default();

                    let expected_buttons = RESOURCE_NAMES
                        .into_iter()
                        .map(|name| [KeyboardButton::new(format!("🔑 {name}"))]);
                    let expected_keyboard = KeyboardMarkup::new(expected_buttons).resize_keyboard();

                    mock_send_message
                        .expect_reply_markup()
                        .with(predicate::eq(expected_keyboard))
                        .return_once(|_| {
                            let mut inner_mock_send_message = MockSendMessage::default();
                            inner_mock_send_message
                                .expect_into_future()
                                .return_const(ready(Ok(TelegramMessage::default())));
                            inner_mock_send_message
                        });

                    mock_send_message
                });
            mock_context.expect_bot().return_const(mock_bot);

            let mut mock_storage_client = crate::PasswordStorageClient::default();
            mock_storage_client
                .expect_list::<grpc::Empty>()
                .with(predicate::eq(grpc::Empty {}))
                .returning(|_empty| {
                    let resources = RESOURCE_NAMES
                        .into_iter()
                        .map(ToOwned::to_owned)
                        .map(|name| grpc::Resource { name })
                        .collect();
                    Ok(tonic::Response::new(grpc::ListOfResources { resources }))
                });
            mock_context
                .expect_storage_client()
                .return_const(tokio::sync::Mutex::new(mock_storage_client));

            let state = State::try_from_transition(from_state, command, &mock_context)
                .await
                .unwrap();
            assert!(matches!(state, State::ResourcesList(_)))
        }

        #[test]
        pub async fn help_success() {
            let resources_list = State::resources_list();

            test_help_success(resources_list).await
        }

        #[test]
        pub async fn start_failure() {
            let resources_list = State::resources_list();
            let start = Command::Start(crate::command::Start);

            test_unavailable_command(resources_list, start).await
        }

        #[test]
        pub async fn from_resource_actions_by_cancel_success() {
            const REQUEST_MESSAGE_ID: i32 = 100;
            const CANCEL_MESSAGE_ID: i32 = 101;
            const RESOURCE_MESSAGE_ID: i32 = 102;

            let resource_request_message_id = teloxide::types::MessageId(REQUEST_MESSAGE_ID);
            let displayed_cancel_message_id = teloxide::types::MessageId(CANCEL_MESSAGE_ID);
            let displayed_resource_message_id = teloxide::types::MessageId(RESOURCE_MESSAGE_ID);

            let resource_actions = State::ResourceActions(ResourceActions::test(Arc::new(
                RwLock::new(DisplayedResourceData::new(
                    resource_request_message_id,
                    displayed_cancel_message_id,
                    displayed_resource_message_id,
                    "test.resource.com".to_owned(),
                )),
            )));

            let cancel = Command::Cancel(crate::command::Cancel);

            let mock_bot = MockBotBuilder::new()
                .expect_delete_message(MessageId(REQUEST_MESSAGE_ID))
                .expect_delete_message(MessageId(CANCEL_MESSAGE_ID))
                .expect_delete_message(MessageId(RESOURCE_MESSAGE_ID))
                .build();

            test_resources_actions_setup(resource_actions, cancel, mock_bot).await
        }

        #[test]
        pub async fn from_delete_confirmation_by_cancel_success() {
            const REQUEST_MESSAGE_ID: i32 = 100;
            const CANCEL_MESSAGE_ID: i32 = 101;
            const RESOURCE_MESSAGE_ID: i32 = 102;

            let resource_request_message_id = teloxide::types::MessageId(REQUEST_MESSAGE_ID);
            let displayed_cancel_message_id = teloxide::types::MessageId(CANCEL_MESSAGE_ID);
            let displayed_resource_message_id = teloxide::types::MessageId(RESOURCE_MESSAGE_ID);

            let delete_confirmation = State::DeleteConfirmation(
                DeleteConfirmation::test(Arc::new(RwLock::new(DisplayedResourceData::new(
                    resource_request_message_id,
                    displayed_cancel_message_id,
                    displayed_resource_message_id,
                    "test.resource.com".to_owned(),
                ))))
                .await,
            );

            let cancel = Command::Cancel(crate::command::Cancel);

            let mock_bot = MockBotBuilder::new()
                .expect_delete_message(MessageId(REQUEST_MESSAGE_ID))
                .expect_delete_message(MessageId(CANCEL_MESSAGE_ID))
                .expect_delete_message(MessageId(RESOURCE_MESSAGE_ID))
                .build();

            test_resources_actions_setup(delete_confirmation, cancel, mock_bot).await
        }
    }

    pub mod message {
        use mockall::predicate;
        use teloxide::types::{KeyboardButton, KeyboardMarkup};
        use tokio::test;

        use crate::{
            grpc,
            message::MessageBox,
            state::{Context, State},
            test_utils::{
                mock_bot::{MockBotBuilder, CHAT_ID},
                test_unexpected_message,
            },
            transition::{TransitionFailureReason, TryFromTransition as _},
        };

        #[test]
        pub async fn web_app_failure() {
            let resources_list = State::resources_list();
            let web_app = MessageBox::web_app("data".to_owned(), "button_text".to_owned());

            test_unexpected_message(resources_list, web_app).await
        }

        #[test]
        pub async fn list_failure() {
            let resources_list = State::resources_list();
            let list = MessageBox::list();

            test_unexpected_message(resources_list, list).await
        }

        #[test]
        pub async fn add_failure() {
            let resources_list = State::resources_list();
            let add = MessageBox::add();

            test_unexpected_message(resources_list, add).await
        }

        #[test]
        pub async fn from_main_menu_by_list_success() {
            const RESOURCE_NAMES: [&str; 3] = [
                "1.test.resource.com",
                "2.test.resource.com",
                "3.test.resource.com",
            ];

            let main_menu = State::main_menu();
            let list = MessageBox::list();

            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);

            let expected_buttons = RESOURCE_NAMES
                .into_iter()
                .map(|name| [KeyboardButton::new(format!("🔑 {name}"))]);
            let expected_keyboard = KeyboardMarkup::new(expected_buttons).resize_keyboard();

            mock_context.expect_bot().return_const(
                MockBotBuilder::new()
                    .expect_send_message(
                        "👉 Choose a resource or type for search.\n\n\
                         Type /cancel to go back."
                            .to_owned(),
                    )
                    .expect_reply_markup(expected_keyboard)
                    .expect_into_future()
                    .build(),
            );

            let mut mock_storage_client = crate::PasswordStorageClient::default();
            mock_storage_client
                .expect_list::<grpc::Empty>()
                .with(predicate::eq(grpc::Empty {}))
                .returning(|_empty| {
                    let resources = RESOURCE_NAMES
                        .into_iter()
                        .map(ToOwned::to_owned)
                        .map(|name| grpc::Resource { name })
                        .collect();
                    Ok(tonic::Response::new(grpc::ListOfResources { resources }))
                });
            mock_context
                .expect_storage_client()
                .return_const(tokio::sync::Mutex::new(mock_storage_client));

            let state = State::try_from_transition(main_menu, list, &mock_context)
                .await
                .unwrap();
            assert!(matches!(state, State::ResourcesList(_)))
        }

        #[test]
        pub async fn from_main_menu_by_list_with_empty_user_failure() {
            let main_menu = State::main_menu();
            let list = MessageBox::list();

            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);

            let mut mock_storage_client = crate::PasswordStorageClient::default();
            mock_storage_client
                .expect_list::<grpc::Empty>()
                .with(predicate::eq(grpc::Empty {}))
                .returning(|_empty| {
                    Ok(tonic::Response::new(grpc::ListOfResources {
                        resources: Vec::new(),
                    }))
                });
            mock_context
                .expect_storage_client()
                .return_const(tokio::sync::Mutex::new(mock_storage_client));

            let err = State::try_from_transition(main_menu, list, &mock_context)
                .await
                .unwrap_err();
            assert!(matches!(
                err.reason,
                TransitionFailureReason::User(message) if message == "❎ There are no stored passwords yet."))
        }

        #[test]
        pub async fn from_resources_list_by_successful_search_success() {
            const FOUND_RESOURCE_NAMES: [&str; 2] =
                ["1.search.test.resource.com", "2.search.test.resource.com"];

            let resources_list = State::resources_list();
            let search_resource_name_msg = MessageBox::arbitrary("search.test.resource.com");

            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);

            let expected_buttons = FOUND_RESOURCE_NAMES
                .into_iter()
                .map(|name| [KeyboardButton::new(format!("🔑 {name}"))]);
            let expected_keyboard = KeyboardMarkup::new(expected_buttons).resize_keyboard();

            let mock_bot = MockBotBuilder::new()
                .expect_send_message(
                    "👉 The following resources were found, choose one of them \
                     or type for a new search.\n\nType /cancel to go back."
                        .to_owned(),
                )
                .expect_reply_markup(expected_keyboard)
                .expect_into_future()
                .build();
            mock_context.expect_bot().return_const(mock_bot);

            let mut mock_storage_client = crate::PasswordStorageClient::default();
            mock_storage_client
                .expect_get::<crate::grpc::Resource>()
                .with(predicate::eq(crate::grpc::Resource {
                    name: "search.test.resource.com".to_owned(),
                }))
                .returning(|_resource| {
                    Err(tonic::Status::not_found(
                        "search.test.resource.com not found",
                    ))
                });
            mock_storage_client
                .expect_search::<grpc::Resource>()
                .with(predicate::eq(grpc::Resource {
                    name: "search.test.resource.com".to_owned(),
                }))
                .returning(|_resource| {
                    Ok(tonic::Response::new(grpc::ListOfResources {
                        resources: FOUND_RESOURCE_NAMES
                            .into_iter()
                            .map(|name| grpc::Resource {
                                name: name.to_owned(),
                            })
                            .collect(),
                    }))
                });
            mock_context
                .expect_storage_client()
                .return_const(tokio::sync::Mutex::new(mock_storage_client));

            let state =
                State::try_from_transition(resources_list, search_resource_name_msg, &mock_context)
                    .await
                    .unwrap();
            assert!(matches!(state, State::ResourcesList(_)))
        }

        #[test]
        pub async fn from_resources_list_by_failed_search_failure() {
            let resources_list = State::resources_list();
            let search_resource_name_msg = MessageBox::arbitrary("search.test.resource.com");

            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);

            let mut mock_storage_client = crate::PasswordStorageClient::default();
            mock_storage_client
                .expect_get::<crate::grpc::Resource>()
                .with(predicate::eq(crate::grpc::Resource {
                    name: "search.test.resource.com".to_owned(),
                }))
                .returning(|_resource| {
                    Err(tonic::Status::not_found(
                        "search.test.resource.com not found",
                    ))
                });
            mock_storage_client
                .expect_search::<grpc::Resource>()
                .with(predicate::eq(grpc::Resource {
                    name: "search.test.resource.com".to_owned(),
                }))
                .returning(|_resource| {
                    Ok(tonic::Response::new(grpc::ListOfResources {
                        resources: Vec::new(),
                    }))
                });
            mock_context
                .expect_storage_client()
                .return_const(tokio::sync::Mutex::new(mock_storage_client));

            let err =
                State::try_from_transition(resources_list, search_resource_name_msg, &mock_context)
                    .await
                    .unwrap_err();
            assert!(matches!(
                err.reason,
                TransitionFailureReason::User(message) if message == "❎ No passwords found for a given query."))
        }
    }

    pub mod button {
        use tokio::test;

        use crate::{button::ButtonBox, state::State, test_utils::test_unexpected_button};

        #[test]
        pub async fn show_failure() {
            let resources_list = State::resources_list();
            let show_button = ButtonBox::show();

            test_unexpected_button(resources_list, show_button).await;
        }

        #[test]
        pub async fn delete_failure() {
            let resources_list = State::resources_list();
            let delete_button = ButtonBox::delete();

            test_unexpected_button(resources_list, delete_button).await;
        }

        #[test]
        pub async fn yes_failure() {
            let resources_list = State::resources_list();
            let yes_button = ButtonBox::yes();

            test_unexpected_button(resources_list, yes_button).await;
        }

        #[test]
        pub async fn no_failure() {
            let resources_list = State::resources_list();
            let no_button = ButtonBox::no();

            test_unexpected_button(resources_list, no_button).await;
        }
    }
}
