//! Module with [`Unauthorized`] states.

use teloxide::{
    payloads::SendMessageSetters as _,
    requests::Requester as _,
    types::{KeyboardButton, KeyboardMarkup, KeyboardRemove, User},
};

use super::{
    async_trait, command, try_with_target, Context, FailedTransition, From,
    TransitionFailureReason, TryFromTransition,
};

mod button_text {
    //! Module with texts for keyboard buttons.

    pub const SIGN_IN: &str = "🔐 Sign in";
}

/// Enum with all possible authorized states.
#[derive(Debug, Clone, From)]
#[allow(clippy::module_name_repetitions)]
pub enum UnauthorizedBox {
    Default(Unauthorized<kind::Default>),
    Start(Unauthorized<kind::Start>),
    WaitingForSecretPhrase(Unauthorized<kind::WaitingForSecretPhrase>),
}

impl Default for UnauthorizedBox {
    fn default() -> Self {
        Self::Default(Unauthorized::default())
    }
}

/// Unauthorized state. Corresponds to the beginning of the dialogue.
///
/// User becomes [authorized](super::authorized::Authorized) when they submit the corresponding admin token.
#[derive(Debug, Clone)]
#[must_use]
pub struct Unauthorized<K> {
    /// Secret token generated on every run.
    /// User should copy this token from logs and send to the bot in order to prove that they are the admin.
    pub(super) admin_token: String,
    pub kind: K,
}

impl Default for Unauthorized<kind::Default> {
    fn default() -> Self {
        Self {
            admin_token: String::from("qwerty"), // TODO: random admin token
            kind: kind::Default,
        }
    }
}

pub mod kind {
    //! Module with [`Unauthorized`] kinds.

    use super::{super::State, Unauthorized, UnauthorizedBox};

    macro_rules! into_state {
            ($($kind_ty:ty),+ $(,)?) => {$(
                impl From<Unauthorized<$kind_ty>> for State {
                    fn from(value: Unauthorized<$kind_ty>) -> Self {
                        UnauthorizedBox::from(value).into()
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
    /// they are the admin.
    #[derive(Debug, Clone, Copy)]
    pub struct WaitingForSecretPhrase;

    into_state!(Default, Start, WaitingForSecretPhrase);
}

impl Unauthorized<kind::Start> {
    async fn setup(context: &Context, admin_token: String) -> color_eyre::Result<Self> {
        let keyboard = KeyboardMarkup::new([[KeyboardButton::new(button_text::SIGN_IN)]])
            .resize_keyboard(Some(true));

        context
            .bot()
            .send_message(context.chat_id(), format!("Please, sign in 🔐"))
            .reply_markup(keyboard)
            .await?;

        Ok(Self {
            admin_token,
            kind: kind::Start,
        })
    }

    async fn send_welcome_message(context: &Context) -> color_eyre::Result<()> {
        let bot = context.bot();

        let User {
            mut first_name,
            last_name,
            ..
        } = bot.get_me().await?.user;

        let bot_name = {
            first_name.push_str(&last_name.unwrap_or_default());
            first_name
        };

        bot.send_message(
            context.chat_id(),
            format!(
                "👋🤖 Welcome to {bot_name} bot!\n\n\
                 I'll help you to manage your passwords."
            ),
        )
        .await?;
        Ok(())
    }
}

#[async_trait]
impl TryFromTransition<Unauthorized<kind::Default>, command::Start> for Unauthorized<kind::Start> {
    type ErrorTarget = Unauthorized<kind::Default>;

    async fn try_from_transition(
        default: Unauthorized<kind::Default>,
        _start_cmd: command::Start,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        try_with_target!(
            default,
            Self::send_welcome_message(context)
                .await
                .map_err(TransitionFailureReason::internal)
        );

        let start = try_with_target!(
            default,
            Self::setup(context, default.admin_token.clone())
                .await
                .map_err(TransitionFailureReason::internal)
        );
        Ok(start)
    }
}

#[async_trait]
impl TryFromTransition<Self, command::Start> for Unauthorized<kind::Start> {
    type ErrorTarget = Self;

    async fn try_from_transition(
        start: Self,
        _start_cmd: command::Start,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        let new_start = try_with_target!(
            start,
            Self::setup(context, start.admin_token.clone())
                .await
                .map_err(TransitionFailureReason::internal)
        );
        Ok(new_start)
    }
}

impl Unauthorized<kind::WaitingForSecretPhrase> {
    async fn setup(context: &Context, admin_token: String) -> color_eyre::Result<Self> {
        context
            .bot()
            .send_message(
                context.chat_id(),
                "Please, enter the admin token spawned in the server logs.",
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
impl<'mes> TryFromTransition<Unauthorized<kind::Start>, &'mes str>
    for Unauthorized<kind::WaitingForSecretPhrase>
{
    type ErrorTarget = Unauthorized<kind::Start>;

    async fn try_from_transition(
        start: Unauthorized<kind::Start>,
        text: &'mes str,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        if text != button_text::SIGN_IN {
            return Err(FailedTransition::user(
                start,
                "Please jump on the button bellow",
            ));
        }

        let waiting_for_secret_phrase = try_with_target!(
            start,
            Self::setup(context, start.admin_token.clone())
                .await
                .map_err(TransitionFailureReason::internal)
        );
        Ok(waiting_for_secret_phrase)
    }
}
