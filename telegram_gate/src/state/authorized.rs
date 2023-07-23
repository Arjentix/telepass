//! Module with [`Authorized`] states.

use teloxide::{
    payloads::SendMessageSetters as _,
    requests::Requester as _,
    types::{KeyboardButton, KeyboardMarkup},
};

use super::{Context, From};

/// Auhtorized state.
#[derive(Debug, Clone, From)]
#[must_use]
pub struct Authorized<K> {
    kind: K,
}

pub mod kind {
    //! Module with [`Authorized`] kinds.

    use super::{super::State, Authorized, From};

    /// Enum with all kinds of [`Authorized`].
    #[derive(Debug, Clone, From)]
    pub enum Kind {
        MainMenu(MainMenu),
    }

    macro_rules! into_kind {
            ($($kind_ty:ty),+ $(,)?) => {$(
                impl From<Authorized<$kind_ty>> for Authorized<Kind> {
                    fn from(value: Authorized<$kind_ty>) -> Self {
                        Self {
                            kind: Kind::from(value.kind)
                        }
                    }
                }

                impl From<Authorized<$kind_ty>> for State {
                    fn from(value: Authorized<$kind_ty>) -> Self {
                        Authorized::<Kind>::from(value).into()
                    }
                }
            )+};
    }

    /// Main menu state kind.
    ///
    /// Waits for user to input an action.
    #[derive(Debug, Copy, Clone)]
    pub struct MainMenu;

    into_kind!(MainMenu,);
}

impl Authorized<kind::MainMenu> {
    /// Setup [`Authorized`] state of [`MainMenu`](kind::MainMenu) kind.
    ///
    /// Prints welcome message and constructs a keyboard with all supported actions.
    pub async fn setup(context: &Context) -> color_eyre::Result<Self> {
        let main_menu = Self {
            kind: kind::MainMenu,
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
