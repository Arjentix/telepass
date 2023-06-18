use std::{error::Error, sync::Arc};

use color_eyre::{eyre::WrapErr as _, Result};
use dotenvy::dotenv;
use state::StateBox;
use teloxide::{
    dispatching::dialogue::InMemStorage, prelude::*, types::Me, utils::command::BotCommands,
};
use tracing::{info, instrument, warn, Level};
use tracing_subscriber::{filter::LevelFilter, EnvFilter, FmtSubscriber};

mod state;

#[derive(BotCommands, Debug, Clone, PartialEq, Eq)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "display this text")]
    Help,
    #[command(description = "command to start the bot")]
    Start,
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
        .dependencies(dptree::deps![InMemStorage::<StateBox>::new()])
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
    state: Arc<InMemStorage<StateBox>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    info!("Handling message");

    let Some(text) = msg.text() else {
        bot.send_message(msg.chat.id, "Only text messages are supported").await?;
        return Ok(());
    };

    #[allow(clippy::option_if_let_else)]
    if let Ok(_command) = Command::parse(text, me.username()) {
        // TODO: match all transitions by command
    } else {
        // TODO: match all transitions by text
    }

    Ok(())
}

#[instrument(skip(bot))]
async fn callback_handler(bot: Bot, q: CallbackQuery) -> Result<(), Box<dyn Error + Send + Sync>> {
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
