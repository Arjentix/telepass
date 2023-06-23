//! Module with [`Unauthorized`] states.

use eyre::eyre;
use teloxide::{
    payloads::SendMessageSetters as _,
    requests::Requester as _,
    types::{KeyboardButton, KeyboardMarkup, KeyboardRemove, User},
};

use super::{
    async_trait, authorized, command, try_with_target, Bot, ChatId, FailedTransition, From,
    MakeTransition,
};

mod button_text {
    //! Module with texts for keyboard buttons.

    pub const SIGN_IN: &str = "🔐 Sign in";
}

/// Unauthorized state. Corresponds to the beginning of the dialogue.
///
/// User becomes [authorized](super::authorized::Authorized) when they submit the corresponding admin token.
#[derive(Debug, Clone)]
#[must_use]
pub struct Unauthorized<K> {
    /// Secret token generated on every run.
    /// User should copy this token from logs and send to the bot in order to prove that they are the admin.
    pub admin_token: String,
    pub kind: K,
}

impl Default for Unauthorized<kind::Kind> {
    fn default() -> Self {
        Self {
            admin_token: String::from("qwerty"), // TODO: generate secret token
            kind: kind::Kind::default(),
        }
    }
}

#[async_trait]
impl MakeTransition<super::State, command::Command> for Unauthorized<kind::Kind> {
    type ErrorTarget = Self;

    async fn make_transition(
        self,
        cmd: command::Command,
        bot: Bot,
        chat_id: ChatId,
    ) -> Result<super::State, FailedTransition<Self>> {
        use kind::Kind;

        match (self.kind, cmd) {
            (Kind::Default(default), command::Command::Start(start_cmd)) => Unauthorized {
                admin_token: self.admin_token,
                kind: default,
            }
            .make_transition(start_cmd, bot, chat_id)
            .await
            .map(Into::into)
            .map_err(FailedTransition::transform),
            (Kind::Start(start), command::Command::Start(start_cmd)) => Unauthorized {
                admin_token: self.admin_token,
                kind: start,
            }
            .make_transition(start_cmd, bot, chat_id)
            .await
            .map(Into::into)
            .map_err(FailedTransition::transform),
            (Kind::Default(_) | Kind::Start(_) | Kind::WaitingForSecretPhrase(_), cmd) => {
                try_with_target!(
                    self,
                    bot.send_message(chat_id, "Unavailable command in the current state.")
                        .await
                );
                Err(FailedTransition {
                    target: self,
                    reason: eyre!("User sent unavalable command `{cmd:?}`"),
                })
            }
        }
    }
}

#[async_trait]
impl<'mes> MakeTransition<super::State, &'mes str> for Unauthorized<kind::Kind> {
    type ErrorTarget = Self;

    async fn make_transition(
        self,
        text: &'mes str,
        bot: Bot,
        chat_id: ChatId,
    ) -> Result<super::State, FailedTransition<Self>> {
        use kind::Kind;

        match self.kind {
            Kind::Start(start) => <_ as MakeTransition<
                Unauthorized<kind::WaitingForSecretPhrase>,
                &'mes str,
            >>::make_transition(
                Unauthorized {
                    admin_token: self.admin_token,
                    kind: start,
                },
                text,
                bot,
                chat_id,
            )
            .await
            .map(Into::into)
            .map_err(FailedTransition::transform),
            Kind::WaitingForSecretPhrase(waiting_for_secret_phrase) => Unauthorized {
                admin_token: self.admin_token,
                kind: waiting_for_secret_phrase,
            }
            .make_transition(text, bot, chat_id)
            .await
            .map(authorized::Authorized::from)
            .map(Into::into)
            .map_err(FailedTransition::transform),
            Kind::Default(_) => {
                try_with_target!(
                    self,
                    bot.send_message(
                        chat_id,
                        "Text messages are not allowed in the current state."
                    )
                    .await
                );
                Err(FailedTransition {
                    target: self,
                    reason: eyre!("User sent not allowed text `{text}`"),
                })
            }
        }
    }
}

pub mod kind {
    //! Module with [`Unauthorized`](Unauthorized) kinds.

    use super::{super::State, From, Unauthorized};

    /// Boxed sub-state of [`Unauthorized`].
    #[derive(Debug, Clone, Copy, From)]
    pub enum Kind {
        Default(Default),
        Start(Start),
        WaitingForSecretPhrase(WaitingForSecretPhrase),
    }

    impl std::default::Default for Kind {
        fn default() -> Self {
            Self::Default(Default)
        }
    }

    macro_rules! into_kind {
            ($($kind_ty:ty),+ $(,)?) => {$(
                impl From<Unauthorized<$kind_ty>> for Unauthorized<Kind> {
                    fn from(value: Unauthorized<$kind_ty>) -> Self {
                        Self {
                            admin_token: value.admin_token,
                            kind: Kind::from(value.kind)
                        }
                    }
                }

                impl From<Unauthorized<$kind_ty>> for State {
                    fn from(value: Unauthorized<$kind_ty>) -> Self {
                        Unauthorized::<Kind>::from(value).into()
                    }
                }
            )+};
        }

    /// State before start of the dialog.
    /// Immediately transforms to [`Start`] after first `/start` command.
    #[derive(Debug, Clone, Copy)]
    pub struct Default;

    /// Start of the dialog. Waiting for user signing in.
    #[derive(Debug, Clone, Copy)]
    pub struct Start;

    /// Waiting for user to enter a secret phrase spawned in logs to prove that
    //// they are the admin.
    #[derive(Debug, Clone, Copy)]
    pub struct WaitingForSecretPhrase;

    into_kind!(Default, Start, WaitingForSecretPhrase,);
}

impl Unauthorized<kind::Start> {
    async fn setup(bot: Bot, chat_id: ChatId, admin_token: String) -> eyre::Result<Self> {
        let keyboard = KeyboardMarkup::new([[KeyboardButton::new(button_text::SIGN_IN)]]);

        bot.send_message(chat_id, format!("Please, sign in 🔐"))
            .reply_markup(keyboard)
            .await?;

        Ok(Self {
            admin_token,
            kind: kind::Start,
        })
    }
}

impl Unauthorized<kind::WaitingForSecretPhrase> {
    async fn setup(bot: Bot, chat_id: ChatId, admin_token: String) -> eyre::Result<Self> {
        bot.send_message(
            chat_id,
            "Please, enter the admin token spawned in server logs.",
        )
        .reply_markup(KeyboardRemove::new())
        .await?;

        Ok(Self {
            admin_token,
            kind: kind::WaitingForSecretPhrase,
        })
    }
}

#[async_trait]
impl MakeTransition<Unauthorized<kind::Start>, command::Start> for Unauthorized<kind::Default> {
    type ErrorTarget = Self;

    async fn make_transition(
        self,
        _start_cmd: command::Start,
        bot: Bot,
        chat_id: ChatId,
    ) -> Result<Unauthorized<kind::Start>, FailedTransition<Self::ErrorTarget>> {
        let User {
            mut first_name,
            last_name,
            ..
        } = try_with_target!(self, bot.get_me().await).user;
        let bot_name = {
            first_name.push_str(&last_name.unwrap_or_default());
            first_name
        };

        try_with_target!(
            self,
            bot.send_message(
                chat_id,
                format!(
                    "👋🤖 Welcome to {bot_name} bot!\n\n\
                     I'll help you to manage your passwords."
                )
            )
            .await
        );

        let start = try_with_target!(
            self,
            Unauthorized::<kind::Start>::setup(bot, chat_id, self.admin_token.clone()).await
        );
        Ok(start)
    }
}

#[async_trait]
impl MakeTransition<Self, command::Start> for Unauthorized<kind::Start> {
    type ErrorTarget = Self;

    async fn make_transition(
        self,
        _start_cmd: command::Start,
        bot: Bot,
        chat_id: ChatId,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        let start = try_with_target!(
            self,
            Self::setup(bot, chat_id, self.admin_token.clone()).await
        );
        Ok(start)
    }
}

#[async_trait]
impl<'mes> MakeTransition<Unauthorized<kind::WaitingForSecretPhrase>, &'mes str>
    for Unauthorized<kind::Start>
{
    type ErrorTarget = Self;

    async fn make_transition(
        self,
        text: &'mes str,
        bot: Bot,
        chat_id: ChatId,
    ) -> Result<Unauthorized<kind::WaitingForSecretPhrase>, FailedTransition<Self::ErrorTarget>>
    {
        if text != button_text::SIGN_IN {
            return Err(FailedTransition {
                target: self,
                reason: eyre!(
                    "Expected `{}` input, but `{text}` found",
                    button_text::SIGN_IN
                ),
            });
        }

        let waiting_for_secret_phrase = try_with_target!(
            self,
            Unauthorized::<kind::WaitingForSecretPhrase>::setup(
                bot,
                chat_id,
                self.admin_token.clone()
            )
            .await
        );
        Ok(waiting_for_secret_phrase)
    }
}

#[async_trait]
impl<'mes> MakeTransition<authorized::MainMenu, &'mes str>
    for Unauthorized<kind::WaitingForSecretPhrase>
{
    type ErrorTarget = Self;

    async fn make_transition(
        self,
        text: &'mes str,
        bot: Bot,
        chat_id: ChatId,
    ) -> Result<authorized::MainMenu, FailedTransition<Self::ErrorTarget>> {
        if text != self.admin_token {
            try_with_target!(
                self,
                bot.send_message(chat_id, "❎ Invalid token. Please, try again.")
                    .await
            );
            return Err(FailedTransition {
                target: self,
                reason: eyre!("User sent invalid token"),
            });
        }

        try_with_target!(
            self,
            bot.send_message(chat_id, "✅ You've successfully signed in!")
                .await
        );

        let main_menu = try_with_target!(self, authorized::MainMenu::setup(bot, chat_id).await);
        Ok(main_menu)
    }
}
