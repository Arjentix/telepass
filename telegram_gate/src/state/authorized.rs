//! Module with [`Authorized`] states.

use teloxide::{
    payloads::SendMessageSetters as _,
    requests::Requester as _,
    types::{ChatId, KeyboardButton, KeyboardMarkup},
    Bot,
};

use super::From;

type PasswordStorageClient =
    crate::grpc::password_storage_client::PasswordStorageClient<tonic::transport::Channel>;

/// Auhtorized state.
#[derive(Debug, Clone, From)]
#[must_use]
pub struct Authorized<K> {
    storage_client: PasswordStorageClient,
    pub kind: K,
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
                            storage_client: value.storage_client,
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
    pub async fn setup(bot: Bot, chat_id: ChatId) -> color_eyre::Result<Self> {
        const PASSWORD_STORAGE_URL_ENV_VAR: &str = "PASSWORD_STORAGE_URL";

        // TODO: Need to check this at start-up time
        #[allow(clippy::expect_used)]
        let storage_service_ip = std::env::var(PASSWORD_STORAGE_URL_ENV_VAR).expect(&format!(
            "Exepcted `{PASSWORD_STORAGE_URL_ENV_VAR}` environment variable"
        ));
        let mut storage_client = PasswordStorageClient::connect(storage_service_ip).await?;

        let resources = storage_client
            .list(crate::grpc::Empty {})
            .await?
            .into_inner();
        let buttons = resources
            .resources
            .into_iter()
            .map(|resource| [KeyboardButton::new(format!("🔑 {}", resource.name))]);
        let keyboard = KeyboardMarkup::new(buttons).resize_keyboard(Some(true));

        bot.send_message(chat_id, "🏠 Welcome to the main menu.")
            .reply_markup(keyboard)
            .await?;

        Ok(Self {
            storage_client,
            kind: kind::MainMenu,
        })
    }
}
