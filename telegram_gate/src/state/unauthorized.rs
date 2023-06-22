//! Module with [`Unauthorized`] states.

use std::convert::Infallible;

use eyre::eyre;
use teloxide::requests::Requester as _;

use super::{
    async_trait, authorized, command, try_with_target, Bot, ChatId, FailedTransition, From,
    MakeTransition,
};

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
            (Kind::Start(start), command::Command::Start(start_cmd)) => {
                <_ as MakeTransition<Unauthorized<kind::Start>, command::Start>>::make_transition(
                    Unauthorized {
                        admin_token: self.admin_token,
                        kind: start,
                    },
                    start_cmd,
                    bot,
                    chat_id,
                )
                .await
                .map(Into::into)
                .map_err(|_infallible: FailedTransition<Infallible>| panic!("Infallible"))
            }
            (Kind::Start(_) | Kind::WaitingForSecretPhrase(_), cmd) => {
                try_with_target!(
                    self,
                    bot.send_message(chat_id, "Unavailable command in the current state")
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
            .map_err(FailedTransition::transfrom),
            Kind::WaitingForSecretPhrase(waiting_for_secret_phrase) => Unauthorized {
                admin_token: self.admin_token,
                kind: waiting_for_secret_phrase,
            }
            .make_transition(text, bot, chat_id)
            .await
            .map(authorized::Authorized::from)
            .map(Into::into)
            .map_err(FailedTransition::transfrom),
        }
    }
}

pub mod kind {
    //! Module with [`Unauthorized`](Unauthorized) kinds.

    use super::{super::State, From, Unauthorized};

    /// Boxed sub-state of [`Unauthorized`].
    #[derive(Debug, Clone, Copy, From)]
    pub enum Kind {
        Start(Start),
        WaitingForSecretPhrase(WaitingForSecretPhrase),
    }

    impl Default for Kind {
        fn default() -> Self {
            Self::Start(Start)
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

    /// Start of the dialog. Waiting for user signing in.
    #[derive(Debug, Clone, Copy)]
    pub struct Start;

    /// Waiting for user to enter a secret phrase spawned in logs to prove that
    //// they are the admin.
    #[derive(Debug, Clone, Copy)]
    pub struct WaitingForSecretPhrase;

    into_kind!(Start, WaitingForSecretPhrase,);
}

#[async_trait]
impl MakeTransition<Self, command::Start> for Unauthorized<kind::Start> {
    type ErrorTarget = Infallible;

    async fn make_transition(
        self,
        _start_cmd: command::Start,
        _bot: Bot,
        _chat_id: ChatId,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        // TODO: Keyboard
        Ok(self)
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
        const SIGN_IN: &str = "🔐 Sign in";

        if text != SIGN_IN {
            return Err(FailedTransition {
                target: self,
                reason: eyre!("Expected `{SIGN_IN}` input, but `{text}` found"),
            });
        }

        try_with_target!(
            self,
            bot.send_message(
                chat_id,
                "Please, enter the admin token spawned in server logs",
            )
            .await
        );

        Ok(Unauthorized {
            admin_token: self.admin_token,
            kind: kind::WaitingForSecretPhrase,
        })
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
            try_with_target!(self, bot.send_message(chat_id, "Invalid token").await);
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
