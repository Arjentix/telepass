//! Module with [`Authorized`] states.

use std::sync::Arc;

use color_eyre::eyre::WrapErr as _;
use drop_bomb::DebugDropBomb;
use teloxide::types::{KeyboardButton, KeyboardMarkup};
use tokio::sync::RwLock;
use tracing::{debug, warn};

use super::{
    async_trait, button, command, message, try_with_target, unauthorized, Context,
    FailedTransition, From, IdExt as _, TelegramMessage, TransitionFailureReason,
    TryFromTransition,
};
#[cfg(not(test))]
use super::{
    EditMessageReplyMarkupSetters as _, EditMessageTextSetters as _, Requester as _,
    SendMessageSetters as _,
};
use crate::button::Button;

mod sealed {
    //! Module with [`Sealed`] and its implementations for authorized states.

    use super::*;

    /// Trait to prevent [`super::Marker`] implementation for types outside of
    /// [`authorized`](super) module.
    pub trait Sealed {}

    impl Sealed for Authorized<kind::MainMenu> {}
    impl Sealed for Authorized<kind::ResourcesList> {}
    impl Sealed for Authorized<kind::ResourceActions> {}
    impl Sealed for Authorized<kind::DeleteConfirmation> {}
}

/// Marker trait to identify *authorized* states.
pub trait Marker: sealed::Sealed {}

impl Marker for Authorized<kind::MainMenu> {}
impl Marker for Authorized<kind::ResourcesList> {}
impl Marker for Authorized<kind::ResourceActions> {}
impl Marker for Authorized<kind::DeleteConfirmation> {}

/// Enum with all possible authorized states.
#[derive(Debug, Clone, From, PartialEq, Eq)]
#[allow(clippy::module_name_repetitions, clippy::missing_docs_in_private_items)]
pub enum AuthorizedBox {
    MainMenu(Authorized<kind::MainMenu>),
    ResourcesList(Authorized<kind::ResourcesList>),
    ResourceActions(Authorized<kind::ResourceActions>),
    DeleteConfirmation(Authorized<kind::DeleteConfirmation>),
}

#[cfg(test)]
impl AuthorizedBox {
    #[must_use]
    pub const fn main_menu() -> Self {
        Self::MainMenu(Authorized {
            kind: kind::MainMenu,
        })
    }

    #[must_use]
    pub const fn resources_list() -> Self {
        Self::ResourcesList(Authorized {
            kind: kind::ResourcesList,
        })
    }

    #[must_use]
    pub fn resource_actions(allow_not_deleted_messages: bool) -> Self {
        Self::ResourceActions(Authorized {
            kind: kind::ResourceActions(Self::create_displayed_resource_data(
                allow_not_deleted_messages,
            )),
        })
    }

    #[must_use]
    pub fn delete_confirmation(allow_not_deleted_messages: bool) -> Self {
        Self::DeleteConfirmation(Authorized {
            kind: kind::DeleteConfirmation(Self::create_displayed_resource_data(
                allow_not_deleted_messages,
            )),
        })
    }

    fn create_displayed_resource_data(
        allow_not_deleted_messages: bool,
    ) -> Arc<RwLock<DisplayedResourceData>> {
        let mut data = DisplayedResourceData::new(
            TelegramMessage::default(),
            TelegramMessage::default(),
            TelegramMessage::default(),
            "test resource".to_owned(),
        );

        if allow_not_deleted_messages {
            data.bomb.defuse();
        }

        Arc::new(RwLock::new(data))
    }
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

    use super::{
        super::State, async_trait, debug, Arc, Authorized, AuthorizedBox, Context, Destroy,
        DisplayedResourceData, RwLock,
    };

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

    /// Macro to implement [`Destroy`] returning `Ok(())` for concrete authorized state.
    macro_rules! noop_destroy {
        ($($kind_ty:ty),+ $(,)?) => {$(
            #[async_trait]
            impl Destroy for $kind_ty {
                async fn destroy(self, _context: &Context) -> color_eyre::Result<()> {
                    Ok(())
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
    pub struct ResourcesList;

    /// Kind of a state when bot is waiting for user to press some inline button
    /// to make an action with a resource attached to a message.
    #[derive(Debug, Clone)]
    pub struct ResourceActions(pub Arc<RwLock<DisplayedResourceData>>);

    #[async_trait]
    impl Destroy for ResourceActions {
        async fn destroy(self, context: &Context) -> color_eyre::Result<()> {
            let Some(displayed_resource_data_lock) = Arc::into_inner(self.0) else {
                debug!("There are other strong references to `DisplayedResourceData`, skipping deletion");
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
            Arc::as_ptr(&self.0) == Arc::as_ptr(&other.0)
        }
    }

    impl Eq for ResourceActions {}

    /// Kind of a state when bot is waiting for user to confirm resource deletion
    /// or to cancel the operation.
    #[derive(Debug, Clone)]
    pub struct DeleteConfirmation(pub Arc<RwLock<DisplayedResourceData>>);

    #[async_trait]
    impl Destroy for DeleteConfirmation {
        async fn destroy(self, context: &Context) -> color_eyre::Result<()> {
            let Some(displayed_resource_data_lock) = Arc::into_inner(self.0) else {
                debug!("There are other strong references to `DisplayedResourceData`, skipping deletion");
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
            Arc::as_ptr(&self.0) == Arc::as_ptr(&other.0)
        }
    }

    impl Eq for DeleteConfirmation {}

    into_state!(MainMenu, ResourcesList, ResourceActions, DeleteConfirmation);
    noop_destroy!(MainMenu, ResourcesList);
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
    pub resource_request_message: TelegramMessage,
    /// Currently displayed message with help message about `/cancel` command.
    pub cancel_message: TelegramMessage,
    /// Currently displayed message with resource name and attached buttons.
    pub resource_message: TelegramMessage,
    /// Name of the requested resource.
    pub resource_name: String,
    /// Bomb to prevent dropping this type without deleting messages.
    bomb: DebugDropBomb,
}

impl DisplayedResourceData {
    /// Construct new [`DisplayedResourceData`].
    #[must_use]
    pub fn new(
        resource_request_message: TelegramMessage,
        cancel_message: TelegramMessage,
        resource_message: TelegramMessage,
        resource_name: String,
    ) -> Self {
        Self {
            resource_request_message,
            cancel_message,
            resource_message,
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
            self.resource_request_message.id(),
            self.cancel_message.id(),
            self.resource_message.id(),
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
    /// Skipping [`TelegramMessage`] fields because they don't implement [`Eq`].
    fn eq(&self, other: &Self) -> bool {
        self.resource_name == other.resource_name
    }
}

impl Eq for DisplayedResourceData {}

/// Trait to gracefully destroy state.
///
/// Implementors with meaningful [`destroy()`](Destroy::destroy) might want to use [`DebugDropBomb`].
#[async_trait]
trait Destroy {
    async fn destroy(self, context: &Context) -> color_eyre::Result<()>;
}

#[async_trait]
impl<K: Destroy + Send> Destroy for Authorized<K> {
    async fn destroy(self, context: &Context) -> color_eyre::Result<()> {
        self.kind.destroy(context).await
    }
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
        unauthorized::Unauthorized<unauthorized::kind::SecretPhrasePrompt>,
        message::Message<message::kind::Arbitrary>,
    > for Authorized<kind::MainMenu>
{
    type ErrorTarget = unauthorized::Unauthorized<unauthorized::kind::SecretPhrasePrompt>;

    async fn try_from_transition(
        secret_phrase_prompt: unauthorized::Unauthorized<unauthorized::kind::SecretPhrasePrompt>,
        arbitrary: message::Message<message::kind::Arbitrary>,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        let admin_token_candidate = arbitrary.to_string();
        if admin_token_candidate != secret_phrase_prompt.admin_token {
            return Err(FailedTransition::user(
                secret_phrase_prompt,
                "❎ Invalid token. Please, try again.",
            ));
        }

        try_with_target!(
            secret_phrase_prompt,
            context
                .bot()
                .send_message(context.chat_id(), "✅ You've successfully signed in!")
                .await
                .map_err(TransitionFailureReason::internal)
        );

        let main_menu = try_with_target!(
            secret_phrase_prompt,
            Self::setup(context)
                .await
                .map_err(TransitionFailureReason::internal)
        );
        Ok(main_menu)
    }
}

#[async_trait]
impl TryFromTransition<Authorized<kind::ResourcesList>, command::Cancel>
    for Authorized<kind::MainMenu>
{
    type ErrorTarget = Authorized<kind::ResourcesList>;

    async fn try_from_transition(
        resources_list: Authorized<kind::ResourcesList>,
        _cancel: command::Cancel,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        let main_menu = try_with_target!(
            resources_list,
            Self::setup(context)
                .await
                .map_err(TransitionFailureReason::internal)
        );
        Ok(main_menu)
    }
}

impl Authorized<kind::ResourcesList> {
    /// Setup [`Authorized`] state of [`ResourcesList`](kind::ResourcesList) kind.
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
        A: Marker + Send + Sync + Destroy + 'static,
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

        if let Err(error) = prev_state.destroy(context).await {
            warn!(?error, "Failed to destroy previous state");
        }
        Ok(Self {
            kind: kind::ResourcesList,
        })
    }
}

#[async_trait]
impl TryFromTransition<Authorized<kind::MainMenu>, message::Message<message::kind::List>>
    for Authorized<kind::ResourcesList>
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
impl TryFromTransition<Authorized<kind::ResourceActions>, command::Cancel>
    for Authorized<kind::ResourcesList>
{
    type ErrorTarget = Authorized<kind::ResourceActions>;

    async fn try_from_transition(
        resource_actions: Authorized<kind::ResourceActions>,
        _cancel: command::Cancel,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        Self::setup(resource_actions, context).await
    }
}

#[async_trait]
impl TryFromTransition<Authorized<kind::ResourcesList>, message::Message<message::kind::Arbitrary>>
    for Authorized<kind::ResourceActions>
{
    type ErrorTarget = Authorized<kind::ResourcesList>;

    async fn try_from_transition(
        resources_list: Authorized<kind::ResourcesList>,
        arbitrary: message::Message<message::kind::Arbitrary>,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        let resource_name = arbitrary.to_string();
        let resource_name = resource_name.strip_prefix("🔑 ").unwrap_or(&resource_name);

        let res = {
            let mut storage_client_lock = context
                .storage_client_from_behalf(&resources_list)
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
                    resources_list,
                    "❎ Resource not found.",
                )),
                _ => Err(FailedTransition::internal(resources_list, status)),
            };
        }

        let cancel_message = try_with_target!(
            resources_list,
            context
                .bot()
                .send_message(context.chat_id(), "Type /cancel to go back.",)
                .reply_markup(teloxide::types::ReplyMarkup::kb_remove())
                .await
                .map_err(TransitionFailureReason::internal)
        );

        let message = try_with_target!(
            resources_list,
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
            kind: kind::ResourceActions(Arc::new(RwLock::new(DisplayedResourceData::new(
                arbitrary.inner,
                cancel_message,
                message,
                resource_name.to_owned(),
            )))),
        })
    }
}

#[async_trait]
impl TryFromTransition<Authorized<kind::ResourceActions>, Button<button::kind::Delete>>
    for Authorized<kind::DeleteConfirmation>
{
    type ErrorTarget = Authorized<kind::ResourceActions>;

    async fn try_from_transition(
        resource_actions: Authorized<kind::ResourceActions>,
        _delete_button: Button<button::kind::Delete>,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        let resource_message_id = resource_actions.kind.0.read().await.resource_message.id();

        try_with_target!(
            resource_actions,
            context
                .bot()
                .edit_message_text(
                    context.chat_id(),
                    resource_message_id,
                    format!(
                        "🗑 Delete *{}* forever?",
                        resource_actions.kind.0.read().await.resource_name,
                    )
                )
                .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                .await
                .map_err(TransitionFailureReason::internal)
        );

        try_with_target!(
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
            kind: kind::DeleteConfirmation(resource_actions.kind.0),
        })
    }
}

#[async_trait]
impl TryFromTransition<Authorized<kind::DeleteConfirmation>, command::Cancel>
    for Authorized<kind::ResourcesList>
{
    type ErrorTarget = Authorized<kind::DeleteConfirmation>;

    async fn try_from_transition(
        delete_confirmation: Authorized<kind::DeleteConfirmation>,
        _cancel: command::Cancel,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        Self::setup(delete_confirmation, context).await
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use mockall::predicate;

    use super::*;
    use crate::{
        button::ButtonBox,
        command::Command,
        message::MessageBox,
        mock_bot::{MockBotBuilder, CHAT_ID},
        state::State,
    };

    #[allow(
        dead_code,
        unreachable_code,
        unused_variables,
        clippy::unimplemented,
        clippy::diverging_sub_expression
    )]
    #[forbid(clippy::todo, clippy::wildcard_enum_match_arm)]
    fn tests_completeness_static_check() -> ! {
        use self::{button::*, command::*, message::*};

        panic!("You should never call this function, it's purpose is the static check only");

        // We don't need the actual values, we just need something to check match arms
        let authorized: AuthorizedBox = unimplemented!();
        let cmd: Command = unimplemented!();
        let mes: MessageBox = unimplemented!();
        let button: ButtonBox = unimplemented!();

        // Will fail to compile if a new state or command will be added
        match (authorized, cmd) {
            (AuthorizedBox::MainMenu(_), Command::Help(_)) => main_menu_help_success(),
            (AuthorizedBox::MainMenu(_), Command::Start(_)) => main_menu_start_failure(),
            (AuthorizedBox::MainMenu(_), Command::Cancel(_)) => main_menu_cancel_failure(),
            (AuthorizedBox::ResourcesList(_), Command::Help(_)) => resources_list_help_success(),
            (AuthorizedBox::ResourcesList(_), Command::Start(_)) => resources_list_start_failure(),
            (AuthorizedBox::ResourcesList(_), Command::Cancel(_)) => {
                resources_list_cancel_success()
            }
            (AuthorizedBox::ResourceActions(_), Command::Help(_)) => {
                resource_actions_help_success()
            }
            (AuthorizedBox::ResourceActions(_), Command::Start(_)) => {
                resource_actions_start_failure()
            }
            (AuthorizedBox::ResourceActions(_), Command::Cancel(_)) => {
                resource_actions_cancel_success()
            }
            (AuthorizedBox::DeleteConfirmation(_), Command::Help(_)) => {
                delete_confirmation_help_success()
            }
            (AuthorizedBox::DeleteConfirmation(_), Command::Start(_)) => {
                delete_confirmation_start_failure()
            }
            (AuthorizedBox::DeleteConfirmation(_), Command::Cancel(_)) => {
                delete_confirmation_cancel_success()
            }
        }

        // Will fail to compile if a new state or message will be added
        match (authorized, mes) {
            (AuthorizedBox::MainMenu(_), MessageBox::SignIn(_)) => main_menu_sign_in_failure(),
            (AuthorizedBox::MainMenu(_), MessageBox::List(_)) => main_menu_list_success(),
            (AuthorizedBox::MainMenu(_), MessageBox::Arbitrary(_)) => main_menu_arbitrary_failure(),
            (AuthorizedBox::ResourcesList(_), MessageBox::SignIn(_)) => {
                resources_list_sign_in_failure()
            }
            (AuthorizedBox::ResourcesList(_), MessageBox::List(_)) => resources_list_list_failure(),
            (AuthorizedBox::ResourcesList(_), MessageBox::Arbitrary(_)) => {
                resources_list_right_arbitrary_success();
                resources_list_wrong_arbitrary_failure()
            }
            (AuthorizedBox::ResourceActions(_), MessageBox::SignIn(_)) => {
                resource_actions_sign_in_failure()
            }
            (AuthorizedBox::ResourceActions(_), MessageBox::List(_)) => {
                resource_actions_list_failure()
            }
            (AuthorizedBox::ResourceActions(_), MessageBox::Arbitrary(_)) => {
                resource_actions_arbitrary_failure()
            }
            (AuthorizedBox::DeleteConfirmation(_), MessageBox::SignIn(_)) => {
                delete_confirmation_sign_in_failure()
            }
            (AuthorizedBox::DeleteConfirmation(_), MessageBox::List(_)) => {
                delete_confirmation_list_failure()
            }
            (AuthorizedBox::DeleteConfirmation(_), MessageBox::Arbitrary(_)) => {
                delete_confirmation_arbitrary_failure()
            }
        }

        // Will fail to compile if a new state or button will be added
        match (authorized, button) {
            (AuthorizedBox::MainMenu(_), ButtonBox::Delete(_)) => main_menu_delete_failure(),
            (AuthorizedBox::MainMenu(_), ButtonBox::Yes(_)) => main_menu_yes_failure(),
            (AuthorizedBox::MainMenu(_), ButtonBox::No(_)) => main_menu_no_failure(),
            (AuthorizedBox::ResourcesList(_), ButtonBox::Delete(_)) => {
                resources_list_delete_failure()
            }
            (AuthorizedBox::ResourcesList(_), ButtonBox::Yes(_)) => resources_list_yes_failure(),
            (AuthorizedBox::ResourcesList(_), ButtonBox::No(_)) => resources_list_no_failure(),
            (AuthorizedBox::ResourceActions(_), ButtonBox::Delete(_)) => {
                resource_actions_delete_success()
            }
            (AuthorizedBox::ResourceActions(_), ButtonBox::Yes(_)) => {
                resource_actions_yes_failure()
            }
            (AuthorizedBox::ResourceActions(_), ButtonBox::No(_)) => resource_actions_no_failure(),
            (AuthorizedBox::DeleteConfirmation(_), ButtonBox::Delete(_)) => {
                delete_confirmation_delete_failure()
            }
            (AuthorizedBox::DeleteConfirmation(_), ButtonBox::Yes(_)) => {
                delete_confirmation_yes_failure()
            }
            (AuthorizedBox::DeleteConfirmation(_), ButtonBox::No(_)) => {
                delete_confirmation_no_failure()
            }
        }

        unreachable!()
    }

    mod command {
        //! Test names follow the rule: *state*_*command*_*success/failure*.

        use std::future::ready;

        use teloxide::types::MessageId;
        use tokio::test;

        use super::*;
        use crate::{
            mock_bot::MockSendMessage,
            state::test_utils::{test_help_success, test_unavailable_command},
        };

        async fn test_resources_actions_setup<A: Marker + 'static>(
            from_state: State,
            command: Command,
            mut mock_bot: crate::Bot,
        ) {
            const RESOURCE_NAMES: [&str; 3] =
                ["Test Resource 1", "Test Resource 2", "Test Resource 3"];

            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);

            mock_bot
                .expect_send_message::<_, &'static str>()
                .with(
                    predicate::eq(CHAT_ID),
                    predicate::eq(
                        "👉 Choose a resource.\n\n\
                         Type /cancel to go back.",
                    ),
                )
                .return_once(|_, _| {
                    let mut mock_send_message = MockSendMessage::default();

                    let expected_buttons = RESOURCE_NAMES
                        .into_iter()
                        .map(|name| [KeyboardButton::new(format!("🔑 {}", name))]);
                    let expected_keyboard =
                        KeyboardMarkup::new(expected_buttons).resize_keyboard(Some(true));

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
                .expect_storage_client_from_behalf::<A>()
                .return_const(tokio::sync::Mutex::new(mock_storage_client));

            let state = State::try_from_transition(from_state, command, &mock_context)
                .await
                .unwrap();
            assert!(matches!(
                state,
                State::Authorized(AuthorizedBox::ResourcesList(_))
            ))
        }

        #[test]
        pub async fn main_menu_help_success() {
            let main_menu = State::Authorized(AuthorizedBox::main_menu());

            test_help_success(main_menu).await
        }

        #[test]
        pub async fn main_menu_start_failure() {
            let main_menu = State::Authorized(AuthorizedBox::main_menu());
            let start = Command::Start(crate::command::Start);

            test_unavailable_command(main_menu, start).await
        }

        #[test]
        pub async fn main_menu_cancel_failure() {
            let main_menu = State::Authorized(AuthorizedBox::main_menu());
            let cancel = Command::Cancel(crate::command::Cancel);

            test_unavailable_command(main_menu, cancel).await
        }

        #[test]
        pub async fn resources_list_help_success() {
            let resources_list = State::Authorized(AuthorizedBox::resources_list());

            test_help_success(resources_list).await
        }

        #[test]
        pub async fn resources_list_start_failure() {
            let resources_list = State::Authorized(AuthorizedBox::resources_list());
            let start = Command::Start(crate::command::Start);

            test_unavailable_command(resources_list, start).await
        }

        #[test]
        pub async fn resources_list_cancel_success() {
            let resources_list = State::Authorized(AuthorizedBox::resources_list());
            let cancel = Command::Cancel(crate::command::Cancel);

            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);
            mock_context.expect_bot().return_const(
                MockBotBuilder::new()
                    .expect_send_message("🏠 Welcome to the main menu.")
                    .expect_reply_markup(
                        KeyboardMarkup::new([[KeyboardButton::new(
                            crate::message::kind::List.to_string(),
                        )]])
                        .resize_keyboard(true),
                    )
                    .expect_into_future()
                    .build(),
            );

            let state = State::try_from_transition(resources_list, cancel, &mock_context)
                .await
                .unwrap();
            assert!(matches!(
                state,
                State::Authorized(AuthorizedBox::MainMenu(_))
            ))
        }

        #[test]
        pub async fn resource_actions_help_success() {
            let resource_actions = State::Authorized(AuthorizedBox::resource_actions(true));

            test_help_success(resource_actions).await
        }

        #[test]
        pub async fn resource_actions_start_failure() {
            let resource_actions = State::Authorized(AuthorizedBox::resource_actions(true));
            let start = Command::Start(crate::command::Start);

            test_unavailable_command(resource_actions, start).await
        }

        #[test]
        pub async fn resource_actions_cancel_success() {
            const REQUEST_MESSAGE_ID: i32 = 100;
            const CANCEL_MESSAGE_ID: i32 = 101;
            const RESOURCE_MESSAGE_ID: i32 = 102;

            let mut resource_request_message = TelegramMessage::default();
            resource_request_message
                .expect_id()
                .return_const(teloxide::types::MessageId(REQUEST_MESSAGE_ID));

            let mut displayed_cancel_message = TelegramMessage::default();
            displayed_cancel_message
                .expect_id()
                .return_const(teloxide::types::MessageId(CANCEL_MESSAGE_ID));

            let mut displayed_resource_message = TelegramMessage::default();
            displayed_resource_message
                .expect_id()
                .return_const(teloxide::types::MessageId(RESOURCE_MESSAGE_ID));

            let resource_actions = State::Authorized(AuthorizedBox::ResourceActions(Authorized {
                kind: kind::ResourceActions(Arc::new(RwLock::new(DisplayedResourceData::new(
                    resource_request_message,
                    displayed_cancel_message,
                    displayed_resource_message,
                    "Test resource".to_owned(),
                )))),
            }));

            let cancel = Command::Cancel(crate::command::Cancel);

            let mock_bot = MockBotBuilder::new()
                .expect_delete_message(MessageId(REQUEST_MESSAGE_ID))
                .expect_delete_message(MessageId(CANCEL_MESSAGE_ID))
                .expect_delete_message(MessageId(RESOURCE_MESSAGE_ID))
                .build();

            test_resources_actions_setup::<Authorized<kind::ResourceActions>>(
                resource_actions,
                cancel,
                mock_bot,
            )
            .await
        }

        #[test]
        pub async fn delete_confirmation_help_success() {
            let delete_confirmation = State::Authorized(AuthorizedBox::delete_confirmation(true));

            test_help_success(delete_confirmation).await
        }

        #[test]
        pub async fn delete_confirmation_start_failure() {
            let delete_confirmation = State::Authorized(AuthorizedBox::delete_confirmation(true));
            let start = Command::Start(crate::command::Start);

            test_unavailable_command(delete_confirmation, start).await
        }

        #[test]
        pub async fn delete_confirmation_cancel_success() {
            const REQUEST_MESSAGE_ID: i32 = 100;
            const CANCEL_MESSAGE_ID: i32 = 101;
            const RESOURCE_MESSAGE_ID: i32 = 102;

            let mut resource_request_message = TelegramMessage::default();
            resource_request_message
                .expect_id()
                .return_const(teloxide::types::MessageId(REQUEST_MESSAGE_ID));

            let mut displayed_cancel_message = TelegramMessage::default();
            displayed_cancel_message
                .expect_id()
                .return_const(teloxide::types::MessageId(CANCEL_MESSAGE_ID));

            let mut displayed_resource_message = TelegramMessage::default();
            displayed_resource_message
                .expect_id()
                .return_const(teloxide::types::MessageId(RESOURCE_MESSAGE_ID));

            let delete_confirmation =
                State::Authorized(AuthorizedBox::DeleteConfirmation(Authorized {
                    kind: kind::DeleteConfirmation(Arc::new(RwLock::new(
                        DisplayedResourceData::new(
                            resource_request_message,
                            displayed_cancel_message,
                            displayed_resource_message,
                            "Test resource".to_owned(),
                        ),
                    ))),
                }));

            let cancel = Command::Cancel(crate::command::Cancel);

            let mock_bot = MockBotBuilder::new()
                .expect_delete_message(MessageId(REQUEST_MESSAGE_ID))
                .expect_delete_message(MessageId(CANCEL_MESSAGE_ID))
                .expect_delete_message(MessageId(RESOURCE_MESSAGE_ID))
                .build();

            test_resources_actions_setup::<Authorized<kind::DeleteConfirmation>>(
                delete_confirmation,
                cancel,
                mock_bot,
            )
            .await
        }
    }

    mod message {
        use tokio::test;

        use super::*;
        use crate::state::test_utils::test_unexpected_message;

        #[test]
        pub async fn main_menu_sign_in_failure() {
            let main_menu = State::Authorized(AuthorizedBox::main_menu());
            let sign_in = MessageBox::sign_in();

            test_unexpected_message(main_menu, sign_in).await
        }

        #[test]
        pub async fn main_menu_list_success() {
            const RESOURCE_NAMES: [&str; 3] =
                ["Test Resource 1", "Test Resource 2", "Test Resource 3"];

            let main_menu = State::Authorized(AuthorizedBox::main_menu());
            let list = MessageBox::list();

            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);

            let expected_buttons = RESOURCE_NAMES
                .into_iter()
                .map(|name| [KeyboardButton::new(format!("🔑 {}", name))]);
            let expected_keyboard =
                KeyboardMarkup::new(expected_buttons).resize_keyboard(Some(true));

            mock_context.expect_bot().return_const(
                MockBotBuilder::new()
                    .expect_send_message(
                        "👉 Choose a resource.\n\n\
                         Type /cancel to go back.",
                    )
                    .expect_reply_markup(expected_keyboard)
                    .expect_into_future()
                    .build(),
            );

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
                State::Authorized(AuthorizedBox::ResourcesList(_))
            ))
        }

        #[test]
        pub async fn main_menu_arbitrary_failure() {
            let main_menu = State::Authorized(AuthorizedBox::main_menu());
            let arbitrary = MessageBox::arbitrary("Test arbitrary message");

            test_unexpected_message(main_menu, arbitrary).await
        }

        #[test]
        pub async fn resources_list_sign_in_failure() {
            let resources_list = State::Authorized(AuthorizedBox::resources_list());
            let sign_in = MessageBox::sign_in();

            test_unexpected_message(resources_list, sign_in).await
        }

        #[test]
        pub async fn resources_list_list_failure() {
            let resources_list = State::Authorized(AuthorizedBox::resources_list());
            let list = MessageBox::list();

            test_unexpected_message(resources_list, list).await
        }

        #[test]
        pub async fn resources_list_right_arbitrary_success() {
            const RESOURCE_MSG_ID: i32 = 40;
            const CANCEL_MSG_ID: i32 = 41;
            const RESOURCE_ACTIONS_MSG_ID: i32 = 42;

            let resources_list = State::Authorized(AuthorizedBox::resources_list());

            let mut mock_inner_message = TelegramMessage::default();
            mock_inner_message
                .expect_text()
                .return_const("🔑 TestResource");
            mock_inner_message
                .expect_id()
                .return_const(teloxide::types::MessageId(RESOURCE_MSG_ID));
            let resource_name_msg = MessageBox::Arbitrary(crate::message::Message {
                inner: mock_inner_message,
                kind: crate::message::kind::Arbitrary,
            });

            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);
            mock_context.expect_bot().return_const(
                MockBotBuilder::new()
                    .expect_send_message("Type /cancel to go back.")
                    .expect_reply_markup(teloxide::types::ReplyMarkup::kb_remove())
                    .expect_into_future_with_id(teloxide::types::MessageId(CANCEL_MSG_ID))
                    .expect_send_message(
                        "🔑 *TestResource*\n\n\
                         Choose an action:"
                            .to_owned(),
                    )
                    .expect_parse_mode(teloxide::types::ParseMode::MarkdownV2)
                    .expect_reply_markup(teloxide::types::ReplyMarkup::inline_kb([[
                        teloxide::types::InlineKeyboardButton::callback(
                            crate::button::kind::Delete.to_string(),
                            crate::button::kind::Delete.to_string(),
                        ),
                    ]]))
                    .expect_into_future_with_id(teloxide::types::MessageId(RESOURCE_ACTIONS_MSG_ID))
                    .build(),
            );

            let mut mock_storage_client = crate::PasswordStorageClient::default();
            mock_storage_client
                .expect_get::<crate::grpc::Resource>()
                .with(predicate::eq(crate::grpc::Resource {
                    name: "TestResource".to_owned(),
                }))
                .returning(|resource| {
                    Ok(tonic::Response::new(crate::grpc::Record {
                        resource: Some(resource),
                        passhash: "unused".to_owned(),
                        salt: "unused".to_owned(),
                    }))
                });
            mock_context
                .expect_storage_client_from_behalf::<Authorized<kind::ResourcesList>>()
                .return_const(tokio::sync::Mutex::new(mock_storage_client));

            let state =
                State::try_from_transition(resources_list, resource_name_msg, &mock_context)
                    .await
                    .unwrap();
            let State::Authorized(AuthorizedBox::ResourceActions(resource_actions)) = state else {
                panic!("Expected `Authorized::ResourceActions`, got {state:?}");
            };
            resource_actions.kind.0.write().await.bomb.defuse();
        }

        #[test]
        pub async fn resources_list_wrong_arbitrary_failure() {
            let resources_list = State::Authorized(AuthorizedBox::resources_list());

            let wrong_resource_name_msg = MessageBox::arbitrary("🔑 WrongTestResource");

            let mut mock_context = Context::default();

            let mut mock_storage_client = crate::PasswordStorageClient::default();
            mock_storage_client
                .expect_get::<crate::grpc::Resource>()
                .with(predicate::eq(crate::grpc::Resource {
                    name: "WrongTestResource".to_owned(),
                }))
                .returning(|_resource| {
                    Err(tonic::Status::not_found("WrongTestResource not found"))
                });
            mock_context
                .expect_storage_client_from_behalf::<Authorized<kind::ResourcesList>>()
                .return_const(tokio::sync::Mutex::new(mock_storage_client));

            let err = State::try_from_transition(
                resources_list.clone(),
                wrong_resource_name_msg,
                &mock_context,
            )
            .await
            .unwrap_err();
            assert!(matches!(
                err.reason,
                TransitionFailureReason::User(user_mistake) if user_mistake == "❎ Resource not found.",
            ));
            assert_eq!(err.target, resources_list)
        }

        #[test]
        pub async fn resource_actions_sign_in_failure() {
            let resource_actions = State::Authorized(AuthorizedBox::resource_actions(true));

            let sign_in = MessageBox::sign_in();

            test_unexpected_message(resource_actions, sign_in).await
        }

        #[test]
        pub async fn resource_actions_list_failure() {
            let resource_actions = State::Authorized(AuthorizedBox::resource_actions(true));

            let list = MessageBox::list();

            test_unexpected_message(resource_actions, list).await
        }

        #[test]
        pub async fn resource_actions_arbitrary_failure() {
            let resource_actions = State::Authorized(AuthorizedBox::resource_actions(true));

            let arbitrary = MessageBox::arbitrary("Test arbitrary message");

            test_unexpected_message(resource_actions, arbitrary).await
        }

        #[test]
        pub async fn delete_confirmation_sign_in_failure() {
            let delete_confirmation = State::Authorized(AuthorizedBox::delete_confirmation(true));

            let sign_in = MessageBox::sign_in();

            test_unexpected_message(delete_confirmation, sign_in).await
        }

        #[test]
        pub async fn delete_confirmation_list_failure() {
            let delete_confirmation = State::Authorized(AuthorizedBox::delete_confirmation(true));

            let list = MessageBox::list();

            test_unexpected_message(delete_confirmation, list).await
        }

        #[test]
        pub async fn delete_confirmation_arbitrary_failure() {
            let delete_confirmation = State::Authorized(AuthorizedBox::delete_confirmation(true));

            let arbitrary = MessageBox::arbitrary("Test arbitrary message");

            test_unexpected_message(delete_confirmation, arbitrary).await
        }
    }

    mod button {
        use tokio::test;

        use super::*;
        use crate::state::test_utils::test_unexpected_button;

        #[test]
        pub async fn main_menu_delete_failure() {
            let main_menu = State::Authorized(AuthorizedBox::main_menu());
            let delete_button = ButtonBox::delete();

            test_unexpected_button(main_menu, delete_button).await;
        }

        #[test]
        pub async fn main_menu_yes_failure() {
            let main_menu = State::Authorized(AuthorizedBox::main_menu());
            let yes_button = ButtonBox::yes();

            test_unexpected_button(main_menu, yes_button).await;
        }

        #[test]
        pub async fn main_menu_no_failure() {
            let main_menu = State::Authorized(AuthorizedBox::main_menu());
            let no_button = ButtonBox::no();

            test_unexpected_button(main_menu, no_button).await;
        }

        #[test]
        pub async fn resources_list_delete_failure() {
            let resources_list = State::Authorized(AuthorizedBox::resources_list());
            let delete_button = ButtonBox::delete();

            test_unexpected_button(resources_list, delete_button).await;
        }

        #[test]
        pub async fn resources_list_yes_failure() {
            let resources_list = State::Authorized(AuthorizedBox::resources_list());
            let yes_button = ButtonBox::yes();

            test_unexpected_button(resources_list, yes_button).await;
        }

        #[test]
        pub async fn resources_list_no_failure() {
            let resources_list = State::Authorized(AuthorizedBox::resources_list());
            let no_button = ButtonBox::no();

            test_unexpected_button(resources_list, no_button).await;
        }

        #[test]
        pub async fn resource_actions_delete_success() {
            let resource_message_id = teloxide::types::MessageId(578);

            let mut resource_message = TelegramMessage::default();
            resource_message
                .expect_id()
                .return_const(resource_message_id);

            let resource_actions = State::Authorized(AuthorizedBox::ResourceActions(Authorized {
                kind: kind::ResourceActions(Arc::new(RwLock::new(DisplayedResourceData::new(
                    TelegramMessage::default(),
                    TelegramMessage::default(),
                    resource_message,
                    "Test resource".to_owned(),
                )))),
            }));
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
                        "🗑 Delete *Test resource* forever?".to_owned(),
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
            let State::Authorized(AuthorizedBox::DeleteConfirmation(delete_confirmation)) = state else {
                panic!("Expected `Authorized::DeleteConfirmation`, got {state:?}");
            };
            delete_confirmation.kind.0.write().await.bomb.defuse();
        }

        #[test]
        pub async fn resource_actions_yes_failure() {
            let resource_actions = State::Authorized(AuthorizedBox::resource_actions(true));
            let yes_button = ButtonBox::yes();

            test_unexpected_button(resource_actions, yes_button).await;
        }

        #[test]
        pub async fn resource_actions_no_failure() {
            let resource_actions = State::Authorized(AuthorizedBox::resource_actions(true));
            let no_button = ButtonBox::no();

            test_unexpected_button(resource_actions, no_button).await;
        }

        #[test]
        pub async fn delete_confirmation_delete_failure() {
            let delete_confirmation = State::Authorized(AuthorizedBox::delete_confirmation(true));
            let delete_button = ButtonBox::delete();

            test_unexpected_button(delete_confirmation, delete_button).await;
        }

        #[test]
        pub async fn delete_confirmation_yes_failure() {
            let delete_confirmation = State::Authorized(AuthorizedBox::delete_confirmation(true));
            let yes_button = ButtonBox::yes();

            test_unexpected_button(delete_confirmation, yes_button).await;
        }

        #[test]
        pub async fn delete_confirmation_no_failure() {
            let delete_confirmation = State::Authorized(AuthorizedBox::delete_confirmation(true));
            let no_button = ButtonBox::no();

            test_unexpected_button(delete_confirmation, no_button).await;
        }
    }
}
