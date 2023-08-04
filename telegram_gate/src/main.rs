#![allow(clippy::panic)]

use std::sync::Arc;

use color_eyre::{eyre::WrapErr as _, Result};
use dotenvy::dotenv;
use state::{State, TryFromTransition};
use teloxide::{
    dispatching::dialogue::{InMemStorage, Storage},
    prelude::*,
    types::Me,
};
use tokio::sync::Mutex;
use tonic::transport::Channel;
use tracing::{error, info, instrument, warn, Level};
use tracing_subscriber::{filter::LevelFilter, EnvFilter, FmtSubscriber};

use crate::state::TransitionFailureReason;

type PasswordStorageClient =
    grpc::password_storage_client::PasswordStorageClient<tonic::transport::Channel>;

pub mod grpc {
    //! Module with `gRPC` client for `password_storage` service

    #![allow(clippy::empty_structs_with_brackets)]
    #![allow(clippy::similar_names)]
    #![allow(clippy::default_trait_access)]
    #![allow(clippy::too_many_lines)]
    #![allow(clippy::clone_on_ref_ptr)]
    #![allow(clippy::shadow_unrelated)]
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::missing_errors_doc)]
    #![allow(clippy::future_not_send)]

    tonic::include_proto!("password_storage");
}

mod state;

pub mod command {
    //! Module with all meaningful commands.

    use std::{convert::Infallible, str::FromStr};

    use teloxide::utils::command::BotCommands;

    #[derive(BotCommands, Debug, Clone, PartialEq, Eq)]
    #[command(
        rename_rule = "lowercase",
        description = "These commands are supported:"
    )]
    pub enum Command {
        #[command(description = "display this text")]
        Help(Help),
        #[command(description = "command to start the bot")]
        Start(Start),
    }

    macro_rules! blank_from_str {
        ($($command_ty:ty),+ $(,)?) => {$(
            impl FromStr for $command_ty {
                type Err = Infallible;

                fn from_str(_s: &str) -> Result<Self, Self::Err> {
                    Ok(Self)
                }
            }
        )+};
    }

    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct Help;

    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct Start;

    blank_from_str!(Help, Start,);
}

pub mod message {
    //! This module contains strongly-typed messages user can send.

    #![allow(clippy::non_ascii_literal)]

    use derive_more::From;
    use parse_display::{Display, FromStr};

    /// Enum with all possible messages.
    #[derive(Debug, Display, Clone, From)]
    #[display("{}")]
    pub enum Message {
        /// "Sign in" message
        SignIn(SignIn),
        /// Any arbitrary message. Parsing will aways fallback to this if nothing else mathed.
        Arbitrary(Arbitrary),
    }

    impl std::str::FromStr for Message {
        /// This conversion never fails because of [`Arbitrary`].
        type Err = std::convert::Infallible;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            SignIn::from_str(s)
                .map(Into::into)
                .or_else(|_| Arbitrary::from_str(s).map(Into::into))
        }
    }

    /// "Sign in" message.
    #[derive(Debug, Display, Clone, FromStr)]
    #[display("🔐 Sign in")]
    pub struct SignIn;

    /// Any arbitrary message.
    #[derive(Debug, Display, Clone)]
    pub struct Arbitrary(pub String);

    impl std::str::FromStr for Arbitrary {
        /// This conversion never fails.
        type Err = std::convert::Infallible;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            Ok(Self(s.to_owned()))
        }
    }
}

pub mod context {
    //! Module with [`Context`] structure to pass values and dependencies between different states.

    use super::{state, Arc, Bot, ChatId, Mutex, PasswordStorageClient};

    pub struct Context {
        bot: Bot,
        chat_id: ChatId,
        storage_client: Arc<Mutex<PasswordStorageClient>>,
    }

    impl Context {
        /// Construct new [`Context`].
        pub fn new(
            bot: Bot,
            chat_id: ChatId,
            storage_client: Arc<Mutex<PasswordStorageClient>>,
        ) -> Self {
            Self {
                bot,
                chat_id,
                storage_client,
            }
        }

        /// Get bot.
        pub const fn bot(&self) -> &Bot {
            &self.bot
        }

        /// Get chat id.
        #[allow(clippy::must_use_candidate)]
        pub const fn chat_id(&self) -> ChatId {
            self.chat_id
        }

        /// Get storage client proving that it's done only by an authorized state.
        ///
        /// The idea is that if caller side has [`Authorized`](state::authorized::Authorized) instance
        /// then it's eligible to get [`PasswordStorageClient`].
        pub fn storage_client_from_behalf(
            &self,
            _authorized: &impl state::authorized::Marker,
        ) -> &Mutex<PasswordStorageClient> {
            &self.storage_client
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    init_logger().wrap_err("Failed to initialize logger")?;

    info!("Hello from Telepass Telegram Gate!");

    dotenv().wrap_err("Failed to load `.env` file")?;

    let bot = Bot::from_env();
    let storage_client = Arc::new(Mutex::new(setup_storage_client().await?));

    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint(message_handler))
        .branch(Update::filter_callback_query().endpoint(callback_handler));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![
            InMemStorage::<State>::new(),
            Arc::clone(&storage_client)
        ])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}

#[instrument(skip(bot, me))]
async fn message_handler(
    bot: Bot,
    msg: Message,
    me: Me,
    state_storage: Arc<InMemStorage<State>>,
    storage_client: Arc<Mutex<PasswordStorageClient>>,
) -> Result<(), color_eyre::Report> {
    use teloxide::utils::command::BotCommands as _;

    info!("Handling message");

    let Some(text) = msg.text() else {
        bot.send_message(msg.chat.id, "Only text messages are supported").await?;
        return Ok(());
    };

    let chat_id = msg.chat.id;
    let state = Storage::get_dialogue(Arc::clone(&state_storage), chat_id)
        .await?
        .unwrap_or_default();

    let context = context::Context::new(bot, chat_id, storage_client);

    #[allow(clippy::option_if_let_else)]
    let res = if let Ok(command) = command::Command::parse(text, me.username()) {
        <State as TryFromTransition<State, command::Command>>::try_from_transition(
            state, command, &context,
        )
    } else {
        use std::str::FromStr as _;

        #[allow(clippy::expect_used)]
        let mes =
            message::Message::from_str(text).expect("Message parsing from text is infallible");
        <State as TryFromTransition<State, message::Message>>::try_from_transition(
            state, mes, &context,
        )
    }
    .await;

    let end_state = match res {
        Ok(new_state) => {
            info!(?new_state, "Transition succeed");
            new_state
        }
        Err(failed_transition) => {
            let failure_reason = failed_transition.reason;
            match failure_reason {
                TransitionFailureReason::User(reason) => {
                    context.bot().send_message(chat_id, reason).await?;
                }
                TransitionFailureReason::Internal(reason) => {
                    context
                        .bot()
                        .send_message(chat_id, "Internal error occured, check the server logs.")
                        .await?;
                    error!(?reason, "Internal error occured");
                }
            }

            let old_state = failed_transition.target;
            info!(?old_state, "Transition failed");
            old_state
        }
    };

    Storage::update_dialogue(state_storage, chat_id, end_state)
        .await
        .map_err(Into::into)
}

#[instrument(skip(bot))]
async fn callback_handler(bot: Bot, q: CallbackQuery) -> Result<(), color_eyre::Report> {
    info!("Handling callback");

    // Tell telegram that we've seen this query, to remove loading icons from the clients
    bot.answer_callback_query(q.id).await?;

    let Some(_msg) = q.message else {
        warn!("No message in button callback");
        return Ok(())
    };

    // TODO: match all transitions by button

    Ok(())
}

async fn setup_storage_client() -> Result<PasswordStorageClient> {
    const PASSWORD_STORAGE_URL_ENV_VAR: &str = "PASSWORD_STORAGE_URL";

    #[allow(clippy::expect_used)]
    let password_storage_url = std::env::var(PASSWORD_STORAGE_URL_ENV_VAR).wrap_err(format!(
        "Exepcted `{PASSWORD_STORAGE_URL_ENV_VAR}` environment variable"
    ))?;

    let channel = Channel::from_shared(password_storage_url.clone())
        .wrap_err("Failed to initialize password_storage connection channel")?;

    #[cfg(feature = "tls")]
    let channel = {
        let channel = channel
            .tls_config(prepare_tls_config().wrap_err("Failed to prepare TLS configuration")?)
            .wrap_err("Failed to configure TLS for endpoint")?;
        tracing::info!("TLS Client Auth enabled");
        channel
    };

    let channel = channel
        .connect()
        .await
        .wrap_err("Failed to connect to the password_storage service")?;
    info!(%password_storage_url, "Successfully connected to the password_storage service");

    Ok(PasswordStorageClient::new(channel))
}

#[cfg(feature = "tls")]
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

/// Initialize logger.
fn init_logger() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .finish();
    tracing::subscriber::set_global_default(subscriber).wrap_err("Failed to set global logger")
}
