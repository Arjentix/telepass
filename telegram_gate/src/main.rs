#![allow(clippy::panic)]

use std::sync::Arc;

use color_eyre::{eyre::WrapErr as _, Result};
use dotenvy::dotenv;
use state::{MakeTransition, State};
use teloxide::{
    dispatching::dialogue::{InMemStorage, Storage},
    prelude::*,
    types::Me,
};
use tracing::{error, info, instrument, warn, Level};
use tracing_subscriber::{filter::LevelFilter, EnvFilter, FmtSubscriber};

use crate::state::TransitionFailureReason;

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

#[tokio::main]
async fn main() -> Result<()> {
    init_logger().wrap_err("Failed to initialize logger")?;

    info!("Hello from Telepass Telegram Gate!");

    dotenv().wrap_err("Failed to load `.env` file")?;

    let bot = Bot::from_env();

    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint(message_handler))
        .branch(Update::filter_callback_query().endpoint(callback_handler));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![InMemStorage::<State>::new()])
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

    #[allow(clippy::option_if_let_else)]
    let res = if let Ok(command) = command::Command::parse(text, me.username()) {
        <State as MakeTransition<State, command::Command>>::make_transition(
            state,
            command,
            bot.clone(),
            chat_id,
        )
    } else {
        <State as MakeTransition<State, &str>>::make_transition(state, text, bot.clone(), chat_id)
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
                    bot.send_message(chat_id, reason).await?;
                }
                TransitionFailureReason::Internal(reason) => {
                    bot.send_message(chat_id, "Internal error occured, check the server logs.")
                        .await?;
                    error!(?reason, "Internal error occured");
                }
            }

            failed_transition.target
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
