//! Telegram Gate controls the bot using Telegram API.

#![allow(clippy::panic)]
#![cfg_attr(test, allow(clippy::items_after_test_module))] // Triggered by `mockall`

use std::sync::Arc;

use cfg_if::cfg_if;
use color_eyre::Result;
#[mockall_double::double]
use context::Context;
use mock_bot::UserExt;
use state::{State, TryFromTransition};
use teloxide::{
    dispatching::dialogue::{InMemStorage, Storage},
    prelude::*,
};
use tokio::sync::Mutex;
use tracing::{error, info, instrument, warn};

use crate::state::TransitionFailureReason;

cfg_if! {
    if #[cfg(test)] {
        type Bot = mock_bot::MockBot;
        type SendMessage = mock_bot::MockSendMessage;
        type Me = mock_bot::MockMe;
        type GetMe = mock_bot::MockGetMe;
        type PasswordStorageClient = grpc::MockPasswordStorageClient;
    } else {
        use color_eyre::eyre::WrapErr as _;
        use dotenvy::dotenv;
        use tonic::transport::Channel;
        use tracing::Level;
        use tracing_subscriber::{filter::LevelFilter, EnvFilter, FmtSubscriber};
        use teloxide::payloads::SendMessageSetters;

        #[allow(clippy::missing_docs_in_private_items)]
        type Bot = teloxide::Bot;
        #[allow(clippy::missing_docs_in_private_items)]
        type Me = teloxide::types::Me;
        #[allow(clippy::missing_docs_in_private_items)]
        type PasswordStorageClient =
            grpc::password_storage_client::PasswordStorageClient<tonic::transport::Channel>;
    }
}

mod command;
mod context;
mod grpc;
mod message;
mod mock_bot;
mod state;

#[tokio::main]
async fn main() -> Result<()> {
    // Real `main()` is disabled for test build to avoid errors with `MockBot`
    cfg_if! {
        if #[cfg(test)] {
            Ok(())
        } else {
            main_impl().await
        }
    }
}

/// Implementation of the real [`main()`] function.
#[cfg(not(test))]
async fn main_impl() -> Result<()> {
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

#[instrument(skip(bot, me, storage_client))]
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

    let context = Context::new(bot, chat_id, storage_client);

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
                        .send_message(chat_id, "Internal error occurred, check the server logs.")
                        .await?;
                    error!(?reason, "Internal error occurred");
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

/// Setup [`PasswordStorageClient`] from environment variables.
///
/// Initialized secured connection if `tls` feature is enabled.
#[cfg(not(test))]
async fn setup_storage_client() -> Result<PasswordStorageClient> {
    /// URL of the Password Storage service to connect to
    const PASSWORD_STORAGE_URL_ENV_VAR: &str = "PASSWORD_STORAGE_URL";

    #[allow(clippy::expect_used)]
    let password_storage_url = std::env::var(PASSWORD_STORAGE_URL_ENV_VAR).wrap_err(format!(
        "Expected `{PASSWORD_STORAGE_URL_ENV_VAR}` environment variable"
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

/// Prepare TLS configuration for `gRPC` client.
#[cfg(all(not(test), feature = "tls"))]
fn prepare_tls_config() -> color_eyre::Result<tonic::transport::ClientTlsConfig> {
    use std::path::PathBuf;

    use tonic::transport::{Certificate, ClientTlsConfig, Identity};

    let certs_dir = PathBuf::from_iter([std::env!("CARGO_MANIFEST_DIR"), "..", "certs"]);
    let client_cert_path = certs_dir.join("telegram_gate.crt");
    let client_key_path = certs_dir.join("telegram_gate.key");
    let server_ca_cert_path = certs_dir.join("root_ca.crt");

    let client_cert = std::fs::read_to_string(&client_cert_path).wrap_err_with(|| {
        format!(
            "Failed to read client certificate at path: {}",
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
            "Failed to read server certificate at path: {}",
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
#[cfg(not(test))]
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
