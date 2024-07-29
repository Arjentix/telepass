//! Telegram Gate controls the bot using Telegram API.

#![cfg(feature = "executable")]
#![allow(clippy::panic, reason = "indicates programmer error")]

use std::{str::FromStr as _, sync::Arc};

use color_eyre::{
    eyre::{eyre, WrapErr as _},
    Result,
};
use dotenvy::dotenv;
use telepass_telegram_gate::{
    button::ButtonBox,
    command, context, message,
    state::State,
    transition::{FailedTransition, TransitionFailureReason, TryFromTransition},
    PasswordStorageClient, TelegramMessage,
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
use url::Url;

#[tokio::main]
async fn main() -> Result<()> {
    init_logger().wrap_err("Failed to initialize logger")?;

    info!("Hello from Telepass Telegram Gate!");

    let _ignored = dotenv();

    let bot = Bot::from_env();
    let web_app_url = Arc::new(read_web_app_url_from_env()?);
    let storage_client = Arc::new(Mutex::new(setup_storage_client().await?));
    let owner_user_id = read_owner_user_id_from_env()?;

    let handler = dptree::entry()
        .branch(
            Update::filter_message()
                .filter(move |msg: teloxide::types::Message| {
                    if !msg.chat.is_private() {
                        return false;
                    }

                    let Some(owner_user_id) = owner_user_id else {
                        return true; // Allow anyone if no owner specified
                    };

                    let owner_chat_id = teloxide::types::ChatId::from(owner_user_id);
                    if msg.chat.id != owner_chat_id {
                        warn!(?msg, "Someone has tried to access the bot, access denied");
                        return false;
                    }

                    true
                })
                .endpoint(message_handler),
        )
        .branch(Update::filter_callback_query().endpoint(button_callback_handler));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![
            InMemStorage::<State>::new(),
            Arc::clone(&web_app_url),
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
    web_app_url: Arc<Url>,
    storage_client: Arc<Mutex<PasswordStorageClient>>,
) -> color_eyre::Result<()> {
    info!("Handling message");

    let chat_id = msg.chat.id;

    let Some(command_or_message) = parse_command_or_message(msg, me.username()) else {
        bot.send_message(chat_id, "Unsupported message").await?;
        return Ok(());
    };

    let state = drain_state(Arc::clone(&state_storage), chat_id).await?;

    let end_state = {
        let context = context::Context::new(bot, chat_id, web_app_url, storage_client);

        let res = match command_or_message {
            CommandOrMessage::Command(command) => {
                <State as TryFromTransition<State, command::Command>>::try_from_transition(
                    state, command, &context,
                )
                .await
            }
            CommandOrMessage::Message(message) => {
                <State as TryFromTransition<State, message::MessageBox>>::try_from_transition(
                    state, message, &context,
                )
                .await
            }
        };

        // See: https://rust-lang.github.io/rust-clippy/master/index.html#/large_futures
        Box::pin(unwrap_state(res, &context)).await
    };

    Storage::update_dialogue(state_storage, chat_id, end_state)
        .await
        .map_err(Into::into)
}

#[instrument(skip(bot, state_storage, storage_client))]
#[allow(clippy::significant_drop_tightening, reason = "false positive")]
async fn button_callback_handler(
    bot: Bot,
    query: CallbackQuery,
    state_storage: Arc<InMemStorage<State>>,
    web_app_url: Arc<Url>,
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
        let context = context::Context::new(bot, chat_id, web_app_url, storage_client);
        let res = State::try_from_transition(state, button, &context).await;
        // See: https://rust-lang.github.io/rust-clippy/master/index.html#/large_futures
        Box::pin(unwrap_state(res, &context)).await
    };

    Storage::update_dialogue(state_storage, chat_id, end_state)
        .await
        .map_err(Into::into)
}

/// Enum with either [`command::Command`] or [`message::Message`] to be [`parsed`](parse_command_or_message).
enum CommandOrMessage {
    /// Command variant.
    Command(command::Command),
    /// Message variant.
    Message(message::MessageBox),
}

/// Try to parse [`command::Command`] or [`message::Message`] if first failed.
///
/// Returns [`None`] if message is unsupported.
fn parse_command_or_message(msg: TelegramMessage, bot_name: &str) -> Option<CommandOrMessage> {
    use teloxide::utils::command::BotCommands as _;

    msg.text()
        .and_then(|text| {
            command::Command::parse(text, bot_name)
                .map(CommandOrMessage::Command)
                .ok()
        })
        .or_else(|| message::MessageBox::new(msg).map(CommandOrMessage::Message))
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

/// Read web-app url from environment variable.
fn read_web_app_url_from_env() -> Result<Url> {
    /// URL of the Web App service to connect to
    const WEB_APP_URL_ENV_VAR: &str = "WEB_APP_URL";

    let web_app_url = read_env_var(WEB_APP_URL_ENV_VAR)?;

    Url::parse(&web_app_url)
        .wrap_err_with(|| format!("Failed to parse `{WEB_APP_URL_ENV_VAR}` environment variable"))
}

/// Read owner user id from environment variable.
///
/// Returns `Ok(None)` if not specified.
fn read_owner_user_id_from_env() -> Result<Option<UserId>> {
    /// Id of the owner account to access the bot
    const OWNER_USER_ID_ENV_VAR: &str = "OWNER_USER_ID";

    match std::env::var(OWNER_USER_ID_ENV_VAR) {
        Ok(var) if var.is_empty() => {
            warn!("`{OWNER_USER_ID_ENV_VAR}` environment variable is empty, allowing access to anyone");
            Ok(None)
        }
        Ok(var) => {
            let id = teloxide::types::UserId(u64::from_str(&var).wrap_err_with(|| {
                format!("Failed to parse `{OWNER_USER_ID_ENV_VAR}` environment variable as `u64`")
            })?);
            info!(%id, "Access granted only for");
            Ok(Some(id))
        }
        Err(std::env::VarError::NotPresent) => {
            warn!("`{OWNER_USER_ID_ENV_VAR}` environment variable is not set, allowing access to anyone");
            Ok(None)
        }
        Err(std::env::VarError::NotUnicode(_)) => Err(eyre!(
            "`{OWNER_USER_ID_ENV_VAR}` environment variable is not in unicode format"
        )),
    }
}

/// Setup [`PasswordStorageClient`] from environment variables.
///
/// Initialized secured connection if `tls` feature is enabled.
async fn setup_storage_client() -> Result<PasswordStorageClient> {
    let password_storage_url = read_env_var("PASSWORD_STORAGE_URL")?;

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
fn prepare_tls_config() -> Result<tonic::transport::ClientTlsConfig> {
    use tonic::transport::{Certificate, ClientTlsConfig, Identity};

    let client_cert_path = read_env_var("TELEGRAM_GATE_TLS_CERT_PATH")?;
    let client_key_path = read_env_var("TELEGRAM_GATE_TLS_KEY_PATH")?;
    let server_ca_cert_path = read_env_var("ROOT_CA_CERT_PATH")?;

    let client_cert = std::fs::read_to_string(&client_cert_path).wrap_err_with(|| {
        format!("Failed to read client certificate at path: {client_cert_path}",)
    })?;
    let client_key = std::fs::read_to_string(&client_key_path)
        .wrap_err_with(|| format!("Failed to read client key at path: {client_key_path}"))?;
    let client_identity = Identity::from_pem(client_cert, client_key);

    let server_ca_cert = std::fs::read_to_string(&server_ca_cert_path).wrap_err_with(|| {
        format!("Failed to read server certificate at path: {server_ca_cert_path}",)
    })?;
    let server_ca_cert = Certificate::from_pem(server_ca_cert);

    Ok(ClientTlsConfig::new()
        .domain_name("password_storage")
        .ca_certificate(server_ca_cert)
        .identity(client_identity))
}

/// Read `var` environment variable.
fn read_env_var(var: &str) -> Result<String> {
    std::env::var(var).wrap_err_with(|| format!("Expected `{var}` environment variable"))
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
