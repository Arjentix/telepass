//! Module with [`Authorized`] states.

use color_eyre::eyre::WrapErr as _;
use teloxide::{
    payloads::SendMessageSetters as _,
    requests::Requester as _,
    types::{ChatId, KeyboardButton, KeyboardMarkup},
    Bot,
};
use tonic::transport::Channel;

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
        let mut storage_client = Self::setup_storage_client().await?;

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

    async fn setup_storage_client() -> color_eyre::Result<PasswordStorageClient> {
        const PASSWORD_STORAGE_URL_ENV_VAR: &str = "PASSWORD_STORAGE_URL";

        // TODO: Need to check this at start-up time
        #[allow(clippy::expect_used)]
        let storage_service_ip = std::env::var(PASSWORD_STORAGE_URL_ENV_VAR).expect(&format!(
            "Exepcted `{PASSWORD_STORAGE_URL_ENV_VAR}` environment variable"
        ));

        let channel = Channel::from_shared(storage_service_ip)
            .wrap_err("Failed to initialize password_storage connection channel")?;

        #[cfg(feature = "client_auth")]
        let channel = {
            let channel = channel
                .tls_config(
                    Self::prepare_tls_config().wrap_err("Failed to prepare TLS configuration")?,
                )
                .wrap_err("Failed to configure TLS for endpoint")?;
            tracing::info!("TLS Client Auth enabled");
            channel
        };

        let channel = channel
            .connect()
            .await
            .wrap_err("Failed to connect to the password_storage service")?;

        Ok(PasswordStorageClient::new(channel))
    }

    #[cfg(feature = "client_auth")]
    fn prepare_tls_config() -> color_eyre::Result<tonic::transport::ClientTlsConfig> {
        use std::path::PathBuf;

        use tonic::transport::{Certificate, ClientTlsConfig, Identity};

        let certs_dir = PathBuf::from_iter([std::env!("CARGO_MANIFEST_DIR"), "..", "certs"]);
        let client_cert_path = certs_dir.join("telegram_gate.crt");
        let client_key_path = certs_dir.join("telegram_gate.key");
        let server_ca_cert_path = certs_dir.join("root_ca.crt");

        let client_cert = std::fs::read_to_string(&client_cert_path).wrap_err_with(|| {
            format!(
                "Failed to read client certifiacte at path: {}",
                client_cert_path.display()
            )
        })?;
        let client_key = std::fs::read_to_string(&client_key_path).wrap_err_with(|| {
            format!(
                "Failed to read client key at path: {}",
                client_key_path.display()
            )
        })?;
        let client_identity = Identity::from_pem(client_cert, client_key);

        let server_ca_cert = std::fs::read_to_string(&server_ca_cert_path).wrap_err_with(|| {
            format!(
                "Failed to read server certifiacte at path: {}",
                server_ca_cert_path.display()
            )
        })?;
        let server_ca_cert = Certificate::from_pem(server_ca_cert);

        Ok(ClientTlsConfig::new()
            .domain_name("password_storage")
            .ca_certificate(server_ca_cert)
            .identity(client_identity))
    }
}
