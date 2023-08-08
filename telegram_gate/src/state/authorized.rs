//! Module with [`Authorized`] states.

use teloxide::types::{KeyboardButton, KeyboardMarkup};

use super::{
    async_trait, message, try_with_target, unauthorized, BotTrait as _, Context, FailedTransition,
    From, MessageSetters as _, TransitionFailureReason, TryFromTransition,
};

mod sealed {
    use super::*;

    pub trait Sealed {}

    impl Sealed for Authorized<kind::MainMenu> {}
}

/// Marker trait to identify *authorized* states
pub trait Marker: sealed::Sealed {}

impl Marker for Authorized<kind::MainMenu> {}

/// Enum with all possible authorized states.
#[derive(Debug, Clone, From)]
#[allow(clippy::module_name_repetitions)]
pub enum AuthorizedBox {
    MainMenu(Authorized<kind::MainMenu>),
}

/// Auhtorized state.
#[derive(Debug, Clone)]
#[must_use]
pub struct Authorized<K> {
    _kind: K,
}

pub mod kind {
    //! Module with [`Authorized`] kinds.

    use super::{super::State, Authorized, AuthorizedBox};

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
    #[derive(Debug, Copy, Clone)]
    pub struct MainMenu;

    into_state!(MainMenu);
}

impl Authorized<kind::MainMenu> {
    /// Setup [`Authorized`] state of [`MainMenu`](kind::MainMenu) kind.
    ///
    /// Prints welcome message and constructs a keyboard with all supported actions.
    async fn setup(context: &Context) -> color_eyre::Result<Self> {
        let main_menu = Self {
            _kind: kind::MainMenu,
        };

        let resources = {
            let storage_client = context.storage_client_from_behalf(&main_menu);
            let mut storage_client_lock = storage_client.lock().await;

            storage_client_lock
                .list(crate::grpc::Empty {})
                .await?
                .into_inner()
        };

        let buttons = resources
            .resources
            .into_iter()
            .map(|resource| [KeyboardButton::new(format!("🔑 {}", resource.name))]);
        let keyboard = KeyboardMarkup::new(buttons).resize_keyboard(Some(true));

        context
            .bot()
            .send_message(context.chat_id(), "🏠 Welcome to the main menu.")
            .reply_markup(keyboard)
            .await?;

        Ok(main_menu)
    }
}

#[async_trait]
impl
    TryFromTransition<
        unauthorized::Unauthorized<unauthorized::kind::WaitingForSecretPhrase>,
        message::Arbitrary,
    > for Authorized<kind::MainMenu>
{
    type ErrorTarget = unauthorized::Unauthorized<unauthorized::kind::WaitingForSecretPhrase>;

    async fn try_from_transition(
        waiting_for_secret_phrase: unauthorized::Unauthorized<
            unauthorized::kind::WaitingForSecretPhrase,
        >,
        message::Arbitrary(admin_token_candiate): message::Arbitrary,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        if admin_token_candiate != waiting_for_secret_phrase.admin_token {
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
