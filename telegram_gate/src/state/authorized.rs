//! Module with [`Authorized`] states.

use std::{fmt::Debug, future::Future, sync::Arc};

use base64::{engine::general_purpose::URL_SAFE, Engine as _};
use color_eyre::eyre::WrapErr as _;
use drop_bomb::DebugDropBomb;
use teloxide::{
    types::{KeyboardButton, KeyboardMarkup, MessageId},
    utils::markdown,
};
use tokio::sync::RwLock;
use tracing::{debug, error};

use super::{
    button, command, message, try_with_state, unauthorized, Context, FailedTransition, From,
    TelegramMessageGettersExt as _, TransitionFailureReason, TryFromTransition,
};
#[cfg(not(test))]
use super::{
    EditMessageReplyMarkupSetters as _, EditMessageTextSetters as _, Requester as _,
    SendMessageSetters as _,
};
use crate::{button::Button, message::Message};

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
        super::State, debug, Arc, Authorized, AuthorizedBox, Context, Destroy,
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

/// Trait to gracefully destroy state.
///
/// Implementors with meaningful [`destroy()`](Destroy::destroy) might want to use [`DebugDropBomb`].
trait Destroy: Sized + Send {
    /// Destroy state.
    fn destroy(self, context: &Context) -> impl Future<Output = color_eyre::Result<()>> + Send;

    /// Destroy state and log error if it fails.
    fn destroy_and_log_err(self, context: &Context) -> impl Future<Output = ()> + Send {
        async {
            if let Err(error) = self.destroy(context).await {
                error!(?error, "Failed to destroy state");
            }
        }
    }
}

impl<K: Destroy + Send> Destroy for Authorized<K> {
    async fn destroy(self, context: &Context) -> color_eyre::Result<()> {
        self.kind.destroy(context).await
    }
}

impl Authorized<kind::MainMenu> {
    /// Setup [`Authorized`] state of [`MainMenu`](kind::MainMenu) kind.
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
    #[allow(clippy::expect_used)]
    async fn setup_impl(context: &Context) -> Result<Self, TransitionFailureReason> {
        let buttons = [
            [KeyboardButton::new(message::kind::List.to_string())],
            [KeyboardButton::new(message::kind::Add.to_string()).request(
                teloxide::types::ButtonRequest::WebApp(teloxide::types::WebAppInfo {
                    url: context
                        .web_app_url()
                        .clone()
                        .join("/submit")
                        .expect("Failed to join Wep App url with `/show`"),
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

        Ok(Self {
            kind: kind::MainMenu,
        })
    }
}

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

        try_with_state!(
            secret_phrase_prompt,
            context
                .bot()
                .send_message(context.chat_id(), "✅ You've successfully signed in!")
                .await
                .map_err(TransitionFailureReason::internal)
        );

        Self::setup(secret_phrase_prompt, context).await
    }
}

impl TryFromTransition<Authorized<kind::ResourcesList>, command::Cancel>
    for Authorized<kind::MainMenu>
{
    type ErrorTarget = Authorized<kind::ResourcesList>;

    async fn try_from_transition(
        resources_list: Authorized<kind::ResourcesList>,
        _cancel: command::Cancel,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        Self::setup(resources_list, context).await
    }
}

impl TryFromTransition<Self, Message<message::kind::WebApp>> for Authorized<kind::MainMenu> {
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
                .storage_client_from_behalf(&main_menu)
                .lock()
                .await
                .add(record)
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
    async fn setup<P>(prev_state: P, context: &Context) -> Result<Self, FailedTransition<P>>
    where
        P: Marker + Debug + Send + Sync + 'static,
    {
        let resources_list =
            try_with_state!(prev_state, Self::setup_impl(&prev_state, context).await);
        Ok(resources_list)
    }

    /// [`setup()`](Self::setup) analog with destroying previous state.
    async fn setup_destroying<P>(
        prev_state: P,
        context: &Context,
    ) -> Result<Self, FailedTransition<P>>
    where
        P: Marker + Debug + Destroy + Send + Sync + 'static,
    {
        let resources_list =
            try_with_state!(prev_state, Self::setup_impl(&prev_state, context).await);

        prev_state.destroy_and_log_err(context).await;
        Ok(resources_list)
    }
    /// [`setup()`](Self::setup) and [`setup_destroying()`](Self::setup_destroying) implementation.
    async fn setup_impl<P>(
        prev_state: &P,
        context: &Context,
    ) -> Result<Self, TransitionFailureReason>
    where
        P: Marker + Debug + Send + Sync + 'static,
    {
        let resources = context
            .storage_client_from_behalf(prev_state)
            .lock()
            .await
            .list(crate::grpc::Empty {})
            .await
            .wrap_err("Failed to retrieve the list of stored passwords")
            .map_err(TransitionFailureReason::internal)?
            .into_inner()
            .resources;

        if resources.is_empty() {
            return Err(TransitionFailureReason::User(
                "❎ There are no stored passwords yet.".to_owned(),
            ));
        }

        let buttons = resources
            .into_iter()
            .map(|resource| [KeyboardButton::new(format!("🔑 {}", resource.name))]);
        let keyboard = KeyboardMarkup::new(buttons).resize_keyboard(true);

        context
            .bot()
            .send_message(
                context.chat_id(),
                "👉 Choose a resource.\n\n\
                 Type /cancel to go back.",
            )
            .reply_markup(keyboard)
            .await
            .map_err(TransitionFailureReason::internal)?;

        Ok(Self {
            kind: kind::ResourcesList,
        })
    }
}

impl TryFromTransition<Authorized<kind::MainMenu>, Message<message::kind::List>>
    for Authorized<kind::ResourcesList>
{
    type ErrorTarget = Authorized<kind::MainMenu>;

    async fn try_from_transition(
        main_menu: Authorized<kind::MainMenu>,
        _list: Message<message::kind::List>,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        Self::setup(main_menu, context).await
    }
}

impl TryFromTransition<Authorized<kind::ResourceActions>, command::Cancel>
    for Authorized<kind::ResourcesList>
{
    type ErrorTarget = Authorized<kind::ResourceActions>;

    async fn try_from_transition(
        resource_actions: Authorized<kind::ResourceActions>,
        _cancel: command::Cancel,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        Self::setup_destroying(resource_actions, context).await
    }
}

impl Authorized<kind::ResourceActions> {
    /// Construct text for a message with resource name and attached buttons with possible actions.
    fn construct_choose_an_action_text(resource_name: &str) -> String {
        format!(
            "🔑 {}\n\n\
             Choose an action:",
            markdown::bold(&markdown::escape(resource_name)),
        )
    }

    /// Construct keyboard with possible actions for a resource.
    #[allow(clippy::expect_used)]
    async fn construct_actions_keyboard<P>(
        prev_state: &P,
        resource_name: &str,
        context: &Context,
    ) -> Result<teloxide::types::InlineKeyboardMarkup, TransitionFailureReason>
    where
        P: Marker + Send + Sync + 'static,
    {
        let crate::grpc::Record {
            resource: _resource,
            encrypted_payload: payload,
            salt,
        } = context
            .storage_client_from_behalf(prev_state)
            .lock()
            .await
            .get(crate::grpc::Resource {
                name: resource_name.to_owned(),
            })
            .await
            .wrap_err_with(|| format!("Failed to retrieve `{resource_name}` record"))
            .map_err(TransitionFailureReason::internal)?
            .into_inner();

        let payload = URL_SAFE.encode(payload);
        let salt = URL_SAFE.encode(salt);

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
                        .join(&format!("/show?payload={payload}&salt={salt}"))
                        .expect("Failed to join Wep App url with `/show`"),
                },
            ),
        ]]);
        Ok(keyboard)
    }
}

impl TryFromTransition<Authorized<kind::ResourcesList>, Message<message::kind::Arbitrary>>
    for Authorized<kind::ResourceActions>
{
    type ErrorTarget = Authorized<kind::ResourcesList>;

    async fn try_from_transition(
        resources_list: Authorized<kind::ResourcesList>,
        arbitrary: Message<message::kind::Arbitrary>,
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

        let cancel_message = try_with_state!(
            resources_list,
            context
                .bot()
                .send_message(context.chat_id(), "Type /cancel to go back.",)
                .reply_markup(teloxide::types::ReplyMarkup::kb_remove())
                .await
                .map_err(TransitionFailureReason::internal)
        );

        let actions_keyboard = try_with_state!(
            resources_list,
            Self::construct_actions_keyboard(&resources_list, resource_name, context).await
        );

        let message = try_with_state!(
            resources_list,
            context
                .bot()
                .send_message(
                    context.chat_id(),
                    Self::construct_choose_an_action_text(resource_name),
                )
                .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                .reply_markup(actions_keyboard)
                .await
                .map_err(TransitionFailureReason::internal)
        );

        Ok(Self {
            kind: kind::ResourceActions(Arc::new(RwLock::new(DisplayedResourceData::new(
                arbitrary.id,
                cancel_message.id(),
                message.id(),
                resource_name.to_owned(),
            )))),
        })
    }
}

impl TryFromTransition<Authorized<kind::ResourceActions>, Button<button::kind::Delete>>
    for Authorized<kind::DeleteConfirmation>
{
    type ErrorTarget = Authorized<kind::ResourceActions>;

    async fn try_from_transition(
        resource_actions: Authorized<kind::ResourceActions>,
        _delete_button: Button<button::kind::Delete>,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        let resource_message_id = resource_actions.kind.0.read().await.resource_message_id;

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
                            &resource_actions.kind.0.read().await.resource_name
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
            kind: kind::DeleteConfirmation(resource_actions.kind.0),
        })
    }
}

impl TryFromTransition<Authorized<kind::DeleteConfirmation>, command::Cancel>
    for Authorized<kind::ResourcesList>
{
    type ErrorTarget = Authorized<kind::DeleteConfirmation>;

    async fn try_from_transition(
        delete_confirmation: Authorized<kind::DeleteConfirmation>,
        _cancel: command::Cancel,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        Self::setup_destroying(delete_confirmation, context).await
    }
}

impl TryFromTransition<Authorized<kind::DeleteConfirmation>, Button<button::kind::No>>
    for Authorized<kind::ResourceActions>
{
    type ErrorTarget = Authorized<kind::DeleteConfirmation>;

    async fn try_from_transition(
        delete_confirmation: Authorized<kind::DeleteConfirmation>,
        _no: Button<button::kind::No>,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        let resource_message_id;
        let resource_name;
        {
            let displayed_resource_data = delete_confirmation.kind.0.read().await;

            resource_message_id = displayed_resource_data.resource_message_id;
            resource_name = displayed_resource_data.resource_name.clone();
        }
        let choose_an_action_text = Self::construct_choose_an_action_text(&resource_name);
        let actions_keyboard = try_with_state!(
            delete_confirmation,
            Self::construct_actions_keyboard(&delete_confirmation, &resource_name, context).await
        );

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
            kind: kind::ResourceActions(delete_confirmation.kind.0),
        })
    }
}

impl TryFromTransition<Authorized<kind::DeleteConfirmation>, Button<button::kind::Yes>>
    for Authorized<kind::MainMenu>
{
    type ErrorTarget = Authorized<kind::DeleteConfirmation>;

    async fn try_from_transition(
        delete_confirmation: Authorized<kind::DeleteConfirmation>,
        _yes: Button<button::kind::Yes>,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        let resource_name = delete_confirmation
            .kind
            .0
            .read()
            .await
            .resource_name
            .clone();

        try_with_state!(
            delete_confirmation,
            context
                .storage_client_from_behalf(&delete_confirmation)
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
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use mockall::predicate;

    use super::*;
    use crate::{
        button::ButtonBox,
        command::Command,
        message::MessageBox,
        state::State,
        test_utils::mock_bot::{MockBotBuilder, CHAT_ID},
        TelegramMessage,
    };

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
            (AuthorizedBox::MainMenu(_), MessageBox::WebApp(_)) => {
                main_menu_web_app_success();
                main_menu_web_app_wrong_button_text_failure();
                main_menu_web_app_wrong_data_failure()
            }
            (AuthorizedBox::MainMenu(_), MessageBox::SignIn(_)) => main_menu_sign_in_failure(),
            (AuthorizedBox::MainMenu(_), MessageBox::List(_)) => {
                main_menu_list_success();
                main_menu_list_empty_user_failure()
            }
            (AuthorizedBox::MainMenu(_), MessageBox::Add(_)) => main_menu_add_failure(),
            (AuthorizedBox::MainMenu(_), MessageBox::Arbitrary(_)) => main_menu_arbitrary_failure(),
            (AuthorizedBox::ResourcesList(_), MessageBox::WebApp(_)) => {
                resources_list_web_app_failure()
            }
            (AuthorizedBox::ResourcesList(_), MessageBox::SignIn(_)) => {
                resources_list_sign_in_failure()
            }
            (AuthorizedBox::ResourcesList(_), MessageBox::List(_)) => resources_list_list_failure(),
            (AuthorizedBox::ResourcesList(_), MessageBox::Add(_)) => resources_list_add_failure(),
            (AuthorizedBox::ResourcesList(_), MessageBox::Arbitrary(_)) => {
                resources_list_right_arbitrary_success();
                resources_list_wrong_arbitrary_failure()
            }
            (AuthorizedBox::ResourceActions(_), MessageBox::WebApp(_)) => {
                resource_actions_web_app_failure()
            }
            (AuthorizedBox::ResourceActions(_), MessageBox::SignIn(_)) => {
                resource_actions_sign_in_failure()
            }
            (AuthorizedBox::ResourceActions(_), MessageBox::List(_)) => {
                resource_actions_list_failure()
            }
            (AuthorizedBox::ResourceActions(_), MessageBox::Add(_)) => {
                resource_actions_add_failure()
            }
            (AuthorizedBox::ResourceActions(_), MessageBox::Arbitrary(_)) => {
                resource_actions_arbitrary_failure()
            }
            (AuthorizedBox::DeleteConfirmation(_), MessageBox::WebApp(_)) => {
                delete_confirmation_web_app_failure()
            }
            (AuthorizedBox::DeleteConfirmation(_), MessageBox::SignIn(_)) => {
                delete_confirmation_sign_in_failure()
            }
            (AuthorizedBox::DeleteConfirmation(_), MessageBox::List(_)) => {
                delete_confirmation_list_failure()
            }
            (AuthorizedBox::DeleteConfirmation(_), MessageBox::Add(_)) => {
                delete_confirmation_add_failure()
            }
            (AuthorizedBox::DeleteConfirmation(_), MessageBox::Arbitrary(_)) => {
                delete_confirmation_arbitrary_failure()
            }
        }

        // Will fail to compile if a new state or button will be added
        match (authorized, button) {
            (AuthorizedBox::MainMenu(_), ButtonBox::Show(_)) => main_menu_show_failure(),
            (AuthorizedBox::MainMenu(_), ButtonBox::Delete(_)) => main_menu_delete_failure(),
            (AuthorizedBox::MainMenu(_), ButtonBox::Yes(_)) => main_menu_yes_failure(),
            (AuthorizedBox::MainMenu(_), ButtonBox::No(_)) => main_menu_no_failure(),
            (AuthorizedBox::ResourcesList(_), ButtonBox::Show(_)) => resources_list_show_failure(),
            (AuthorizedBox::ResourcesList(_), ButtonBox::Delete(_)) => {
                resources_list_delete_failure()
            }
            (AuthorizedBox::ResourcesList(_), ButtonBox::Yes(_)) => resources_list_yes_failure(),
            (AuthorizedBox::ResourcesList(_), ButtonBox::No(_)) => resources_list_no_failure(),
            (AuthorizedBox::ResourceActions(_), ButtonBox::Show(_)) => {
                resource_actions_show_failure()
            }
            (AuthorizedBox::ResourceActions(_), ButtonBox::Delete(_)) => {
                resource_actions_delete_success()
            }
            (AuthorizedBox::ResourceActions(_), ButtonBox::Yes(_)) => {
                resource_actions_yes_failure()
            }
            (AuthorizedBox::ResourceActions(_), ButtonBox::No(_)) => resource_actions_no_failure(),
            (AuthorizedBox::DeleteConfirmation(_), ButtonBox::Show(_)) => {
                delete_confirmation_show_failure()
            }
            (AuthorizedBox::DeleteConfirmation(_), ButtonBox::Delete(_)) => {
                delete_confirmation_delete_failure()
            }
            (AuthorizedBox::DeleteConfirmation(_), ButtonBox::Yes(_)) => {
                delete_confirmation_yes_success()
            }
            (AuthorizedBox::DeleteConfirmation(_), ButtonBox::No(_)) => {
                delete_confirmation_no_success()
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
        use crate::test_utils::{
            mock_bot::MockSendMessage, test_help_success, test_unavailable_command,
            web_app_test_url,
        };

        async fn test_resources_actions_setup<A: Marker + 'static>(
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
                        .map(|name| [KeyboardButton::new(format!("🔑 {name}"))]);
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

            let resource_request_message_id = teloxide::types::MessageId(REQUEST_MESSAGE_ID);
            let displayed_cancel_message_id = teloxide::types::MessageId(CANCEL_MESSAGE_ID);
            let displayed_resource_message_id = teloxide::types::MessageId(RESOURCE_MESSAGE_ID);

            let resource_actions = State::Authorized(AuthorizedBox::ResourceActions(Authorized {
                kind: kind::ResourceActions(Arc::new(RwLock::new(DisplayedResourceData::new(
                    resource_request_message_id,
                    displayed_cancel_message_id,
                    displayed_resource_message_id,
                    "test.resource.com".to_owned(),
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

            let resource_request_message_id = teloxide::types::MessageId(REQUEST_MESSAGE_ID);
            let displayed_cancel_message_id = teloxide::types::MessageId(CANCEL_MESSAGE_ID);
            let displayed_resource_message_id = teloxide::types::MessageId(RESOURCE_MESSAGE_ID);

            let delete_confirmation =
                State::Authorized(AuthorizedBox::DeleteConfirmation(Authorized {
                    kind: kind::DeleteConfirmation(Arc::new(RwLock::new(
                        DisplayedResourceData::new(
                            resource_request_message_id,
                            displayed_cancel_message_id,
                            displayed_resource_message_id,
                            "test.resource.com".to_owned(),
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
        use crate::test_utils::{test_unexpected_message, web_app_test_url};

        #[test]
        pub async fn main_menu_web_app_success() {
            let main_menu = State::Authorized(AuthorizedBox::main_menu());

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
                .expect_storage_client_from_behalf::<Authorized<kind::MainMenu>>()
                .return_const(tokio::sync::Mutex::new(mock_storage_client));

            let state = State::try_from_transition(main_menu.clone(), web_app, &mock_context)
                .await
                .unwrap();
            assert_eq!(state, main_menu)
        }

        #[test]
        pub async fn main_menu_web_app_wrong_button_text_failure() {
            let main_menu = State::Authorized(AuthorizedBox::main_menu());

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
        pub async fn main_menu_web_app_wrong_data_failure() {
            let main_menu = State::Authorized(AuthorizedBox::main_menu());

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
        pub async fn main_menu_sign_in_failure() {
            let main_menu = State::Authorized(AuthorizedBox::main_menu());
            let sign_in = MessageBox::sign_in();

            test_unexpected_message(main_menu, sign_in).await
        }

        #[test]
        pub async fn main_menu_list_success() {
            const RESOURCE_NAMES: [&str; 3] = [
                "1.test.resource.com",
                "2.test.resource.com",
                "3.test.resource.com",
            ];

            let main_menu = State::Authorized(AuthorizedBox::main_menu());
            let list = MessageBox::list();

            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);

            let expected_buttons = RESOURCE_NAMES
                .into_iter()
                .map(|name| [KeyboardButton::new(format!("🔑 {name}"))]);
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
        pub async fn main_menu_list_empty_user_failure() {
            let main_menu = State::Authorized(AuthorizedBox::main_menu());
            let list = MessageBox::list();

            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);

            let mut mock_storage_client = crate::PasswordStorageClient::default();
            mock_storage_client
                .expect_list::<crate::grpc::Empty>()
                .with(predicate::eq(crate::grpc::Empty {}))
                .returning(|_empty| {
                    Ok(tonic::Response::new(crate::grpc::ListOfResources {
                        resources: Vec::new(),
                    }))
                });
            mock_context
                .expect_storage_client_from_behalf::<Authorized<kind::MainMenu>>()
                .return_const(tokio::sync::Mutex::new(mock_storage_client));

            let err = State::try_from_transition(main_menu, list, &mock_context)
                .await
                .unwrap_err();
            assert!(matches!(
                err.reason,
                TransitionFailureReason::User(message) if message == "❎ There are no stored passwords yet."))
        }

        #[test]
        pub async fn main_menu_add_failure() {
            let main_menu = State::Authorized(AuthorizedBox::main_menu());
            let add = MessageBox::add();

            test_unexpected_message(main_menu, add).await
        }

        #[test]
        pub async fn main_menu_arbitrary_failure() {
            let main_menu = State::Authorized(AuthorizedBox::main_menu());
            let arbitrary = MessageBox::arbitrary("Test arbitrary message");

            test_unexpected_message(main_menu, arbitrary).await
        }

        #[test]
        pub async fn resources_list_web_app_failure() {
            let resources_list = State::Authorized(AuthorizedBox::resources_list());
            let web_app = MessageBox::web_app("data".to_owned(), "button_text".to_owned());

            test_unexpected_message(resources_list, web_app).await
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
        pub async fn resources_list_add_failure() {
            let resources_list = State::Authorized(AuthorizedBox::resources_list());
            let add = MessageBox::add();

            test_unexpected_message(resources_list, add).await
        }

        #[test]
        pub async fn resources_list_right_arbitrary_success() {
            const RESOURCE_MSG_ID: i32 = 40;
            const CANCEL_MSG_ID: i32 = 41;
            const RESOURCE_ACTIONS_MSG_ID: i32 = 42;

            let resources_list = State::Authorized(AuthorizedBox::resources_list());

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
                                    .join("/show?payload=dW51c2Vk&salt=dW51c2Vk")
                                    .unwrap(),
                            },
                        ),
                    ]]))
                    .expect_into_future_with_id(teloxide::types::MessageId(RESOURCE_ACTIONS_MSG_ID))
                    .build(),
            );

            let mut mock_storage_client = crate::PasswordStorageClient::default();
            mock_storage_client
                .expect_get::<crate::grpc::Resource>()
                .with(predicate::eq(crate::grpc::Resource {
                    name: "test.resource.com".to_owned(),
                }))
                .returning(|resource| {
                    Ok(tonic::Response::new(crate::grpc::Record {
                        resource: Some(resource),
                        encrypted_payload: b"unused".to_vec(),
                        salt: b"unused".to_vec(),
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

            let wrong_resource_name_msg = MessageBox::arbitrary("🔑 wrong.test.resource.com");

            let mut mock_context = Context::default();

            let mut mock_storage_client = crate::PasswordStorageClient::default();
            mock_storage_client
                .expect_get::<crate::grpc::Resource>()
                .with(predicate::eq(crate::grpc::Resource {
                    name: "wrong.test.resource.com".to_owned(),
                }))
                .returning(|_resource| {
                    Err(tonic::Status::not_found(
                        "wrong.test.resource.com not found",
                    ))
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
        pub async fn resource_actions_web_app_failure() {
            let resource_actions = State::Authorized(AuthorizedBox::resource_actions(true));
            let web_app = MessageBox::web_app("data".to_owned(), "button_text".to_owned());

            test_unexpected_message(resource_actions, web_app).await
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
        pub async fn resource_actions_add_failure() {
            let resource_actions = State::Authorized(AuthorizedBox::resource_actions(true));

            let add = MessageBox::add();

            test_unexpected_message(resource_actions, add).await
        }

        #[test]
        pub async fn resource_actions_arbitrary_failure() {
            let resource_actions = State::Authorized(AuthorizedBox::resource_actions(true));

            let arbitrary = MessageBox::arbitrary("Test arbitrary message");

            test_unexpected_message(resource_actions, arbitrary).await
        }

        #[test]
        pub async fn delete_confirmation_web_app_failure() {
            let delete_confirmation = State::Authorized(AuthorizedBox::delete_confirmation(true));
            let web_app = MessageBox::web_app("data".to_owned(), "button_text".to_owned());

            test_unexpected_message(delete_confirmation, web_app).await
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
        pub async fn delete_confirmation_add_failure() {
            let delete_confirmation = State::Authorized(AuthorizedBox::delete_confirmation(true));

            let add = MessageBox::add();

            test_unexpected_message(delete_confirmation, add).await
        }

        #[test]
        pub async fn delete_confirmation_arbitrary_failure() {
            let delete_confirmation = State::Authorized(AuthorizedBox::delete_confirmation(true));

            let arbitrary = MessageBox::arbitrary("Test arbitrary message");

            test_unexpected_message(delete_confirmation, arbitrary).await
        }
    }

    mod button {
        use mockall::predicate::eq;
        use teloxide::types::MessageId;
        use tokio::test;

        use super::*;
        use crate::{
            test_utils::{test_unexpected_button, web_app_test_url},
            PasswordStorageClient,
        };

        #[test]
        pub async fn main_menu_show_failure() {
            let main_menu = State::Authorized(AuthorizedBox::main_menu());
            let show_button = ButtonBox::show();

            test_unexpected_button(main_menu, show_button).await;
        }

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
        pub async fn resources_list_show_failure() {
            let resources_list = State::Authorized(AuthorizedBox::resources_list());
            let show_button = ButtonBox::show();

            test_unexpected_button(resources_list, show_button).await;
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
        pub async fn resource_actions_show_failure() {
            let resource_actions = State::Authorized(AuthorizedBox::resource_actions(true));
            let show_button = ButtonBox::show();

            test_unexpected_button(resource_actions, show_button).await;
        }

        #[test]
        pub async fn resource_actions_delete_success() {
            let resource_message_id = teloxide::types::MessageId(578);

            let resource_actions = State::Authorized(AuthorizedBox::ResourceActions(Authorized {
                kind: kind::ResourceActions(Arc::new(RwLock::new(DisplayedResourceData::new(
                    teloxide::types::MessageId(576),
                    teloxide::types::MessageId(577),
                    resource_message_id,
                    "test.resource.com".to_owned(),
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
            let State::Authorized(AuthorizedBox::DeleteConfirmation(delete_confirmation)) = state
            else {
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
        pub async fn delete_confirmation_show_failure() {
            let delete_confirmation = State::Authorized(AuthorizedBox::delete_confirmation(true));
            let show_button = ButtonBox::show();

            test_unexpected_button(delete_confirmation, show_button).await;
        }

        #[test]
        pub async fn delete_confirmation_delete_failure() {
            let delete_confirmation = State::Authorized(AuthorizedBox::delete_confirmation(true));
            let delete_button = ButtonBox::delete();

            test_unexpected_button(delete_confirmation, delete_button).await;
        }

        #[test]
        pub async fn delete_confirmation_yes_success() {
            const REQUEST_MESSAGE_ID: i32 = 200;
            const CANCEL_MESSAGE_ID: i32 = 201;
            const RESOURCE_MESSAGE_ID: i32 = 202;

            let delete_confirmation =
                State::Authorized(AuthorizedBox::DeleteConfirmation(Authorized {
                    kind: kind::DeleteConfirmation(Arc::new(RwLock::new(
                        DisplayedResourceData::new(
                            teloxide::types::MessageId(REQUEST_MESSAGE_ID),
                            teloxide::types::MessageId(CANCEL_MESSAGE_ID),
                            teloxide::types::MessageId(RESOURCE_MESSAGE_ID),
                            "test.resource.com".to_owned(),
                        ),
                    ))),
                }));

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
                .with(eq(crate::grpc::Resource {
                    name: "test.resource.com".to_owned(),
                }))
                .returning(|_resource| Ok(tonic::Response::new(crate::grpc::Response {})));
            mock_context
                .expect_storage_client_from_behalf::<Authorized<kind::DeleteConfirmation>>()
                .return_const(tokio::sync::Mutex::new(mock_storage_client));

            let state = State::try_from_transition(delete_confirmation, yes_button, &mock_context)
                .await
                .unwrap();
            assert!(matches!(
                state,
                State::Authorized(AuthorizedBox::MainMenu(_))
            ))
        }

        #[test]
        pub async fn delete_confirmation_no_success() {
            let resource_message_id = teloxide::types::MessageId(602);

            let delete_confirmation =
                State::Authorized(AuthorizedBox::DeleteConfirmation(Authorized {
                    kind: kind::DeleteConfirmation(Arc::new(RwLock::new(
                        DisplayedResourceData::new(
                            teloxide::types::MessageId(600),
                            teloxide::types::MessageId(602),
                            resource_message_id,
                            "test.resource.com".to_owned(),
                        ),
                    ))),
                }));
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
                                    .join("/show?payload=dW51c2Vk&salt=dW51c2Vk")
                                    .unwrap(),
                            },
                        ),
                    ]]))
                    .expect_into_future()
                    .build(),
            );

            let mut mock_storage_client = crate::PasswordStorageClient::default();
            mock_storage_client
                .expect_get::<crate::grpc::Resource>()
                .with(predicate::eq(crate::grpc::Resource {
                    name: "test.resource.com".to_owned(),
                }))
                .returning(|resource| {
                    Ok(tonic::Response::new(crate::grpc::Record {
                        resource: Some(resource),
                        encrypted_payload: b"unused".to_vec(),
                        salt: b"unused".to_vec(),
                    }))
                });
            mock_context
                .expect_storage_client_from_behalf::<Authorized<kind::DeleteConfirmation>>()
                .return_const(tokio::sync::Mutex::new(mock_storage_client));

            let state = State::try_from_transition(delete_confirmation, no_button, &mock_context)
                .await
                .unwrap();
            let State::Authorized(AuthorizedBox::ResourceActions(resource_actions)) = state else {
                panic!("Expected `Authorized::ResourceActions`, got {state:?}");
            };
            resource_actions.kind.0.write().await.bomb.defuse();
        }
    }
}
