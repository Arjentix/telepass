//! Telegram Gate controls the bot using Telegram API.

#![allow(clippy::panic)]
#![cfg_attr(test, allow(clippy::items_after_test_module))] // Triggered by `mockall`

use std::sync::Arc;

use color_eyre::{eyre::WrapErr as _, Result};
use dotenvy::dotenv;
use telepass_telegram_gate::{
    button::ButtonBox,
    command, context, message,
    state::{FailedTransition, State, TransitionFailureReason, TryFromTransition},
    PasswordStorageClient,
};
use teloxide::{
    dispatching::dialogue::{InMemStorage, Storage},
    prelude::*,
    types::Me,
};
use tokio::sync::Mutex;
use tonic::transport::Channel;
use tracing::{error, info, instrument, warn, Level};
use tracing_subscriber::{filter::LevelFilter, EnvFilter, FmtSubscriber};

#[tokio::main]
async fn main() -> Result<()> {
    init_logger().wrap_err("Failed to initialize logger")?;

    info!("Hello from Telepass Telegram Gate!");

    dotenv().wrap_err("Failed to load `.env` file")?;

    let bot = Bot::from_env();
    let storage_client = Arc::new(Mutex::new(setup_storage_client().await?));

    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint(message_handler))
        .branch(Update::filter_callback_query().endpoint(button_callback_handler));

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

#[instrument(skip(bot, me, state_storage, storage_client))]
async fn message_handler(
    bot: Bot,
    msg: teloxide::types::Message,
    me: Me,
    state_storage: Arc<InMemStorage<State>>,
    storage_client: Arc<Mutex<PasswordStorageClient>>,
) -> color_eyre::Result<()> {
    use teloxide::utils::command::BotCommands as _;

    info!("Handling message");

    let Some(text) = msg.text() else {
        bot.send_message(msg.chat.id, "Only text messages are supported")
            .await?;
        return Ok(());
    };

    let chat_id = msg.chat.id;
    let state = drain_state(Arc::clone(&state_storage), chat_id).await?;

    let end_state = {
        let context = context::Context::new(bot, chat_id, storage_client);

        #[allow(clippy::option_if_let_else)]
        let res = if let Ok(command) = command::Command::parse(text, me.username()) {
            <State as TryFromTransition<State, command::Command>>::try_from_transition(
                state, command, &context,
            )
        } else {
            let msg = message::MessageBox::new(msg);
            <State as TryFromTransition<State, message::MessageBox>>::try_from_transition(
                state, msg, &context,
            )
        }
        .await;

        // See: https://rust-lang.github.io/rust-clippy/master/index.html#/large_futures
        Box::pin(unwrap_state(res, &context)).await
    };

    Storage::update_dialogue(state_storage, chat_id, end_state)
        .await
        .map_err(Into::into)
}

#[instrument(skip(bot, state_storage, storage_client))]
async fn button_callback_handler(
    bot: Bot,
    query: CallbackQuery,
    state_storage: Arc<InMemStorage<State>>,
    storage_client: Arc<Mutex<PasswordStorageClient>>,
) -> color_eyre::Result<()> {
    info!("Handling button callback");

    // Tell telegram that we've seen this query, to remove loading icons from the clients
    bot.answer_callback_query(query.id).await?;

    let Some(message) = query.message else {
        warn!("No message in button callback");
        return Ok(());
    };

    let Some(data) = query.data else {
        warn!("No data in button callback");
        return Ok(());
    };

    let chat_id = message.chat.id;
    let state = drain_state(Arc::clone(&state_storage), chat_id).await?;

    let button = match ButtonBox::new(message, &data) {
        Ok(button) => button,
        Err(error) => {
            warn!(?error, "Failed to parse button data");
            return Ok(());
        }
    };

    let end_state = {
        let context = context::Context::new(bot, chat_id, storage_client);
        let res = State::try_from_transition(state, button, &context).await;
        // See: https://rust-lang.github.io/rust-clippy/master/index.html#/large_futures
        Box::pin(unwrap_state(res, &context)).await
    };

    Storage::update_dialogue(state_storage, chat_id, end_state)
        .await
        .map_err(Into::into)
}

/// Get [`State`] from [`Storage`] and remove it to not to have clones.
async fn drain_state(
    state_storage: Arc<InMemStorage<State>>,
    chat_id: ChatId,
) -> color_eyre::Result<State> {
    let state = Storage::get_dialogue(Arc::clone(&state_storage), chat_id)
        .await?
        .unwrap_or_default();

    let _ignore_if_not_exists = Storage::remove_dialogue(state_storage, chat_id).await;

    Ok(state)
}

/// Unpack [`State`] from [`Result`] sending message to the user.
async fn unwrap_state(
    res: Result<State, FailedTransition<State>>,
    context: &context::Context,
) -> State {
    let chat_id = context.chat_id();

    match res {
        Ok(new_state) => {
            info!(?new_state, "Transition succeed");
            new_state
        }
        Err(failed_transition) => {
            let failure_reason = failed_transition.reason;
            match failure_reason {
                TransitionFailureReason::User(reason) => {
                    let _ignored = context.bot().send_message(chat_id, reason).await;
                }
                TransitionFailureReason::Internal(reason) => {
                    let _ignored = context
                        .bot()
                        .send_message(chat_id, "Internal error occurred, check the server logs.")
                        .await;
                    error!(?reason, "Internal error occurred");
                }
            }

            let old_state = failed_transition.target;
            info!(?old_state, "Transition failed");
            old_state
        }
    }
}

/// Setup [`PasswordStorageClient`] from environment variables.
///
/// Initialized secured connection if `tls` feature is enabled.
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
