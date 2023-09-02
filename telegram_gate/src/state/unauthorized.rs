//! Module with [`Unauthorized`] states.

use teloxide::types::{KeyboardButton, KeyboardMarkup, KeyboardRemove};

use super::{
    async_trait, command, message, try_with_target, Context, FailedTransition, From,
    TransitionFailureReason, TryFromTransition, UserExt as _,
};
#[cfg(not(test))]
use super::{Requester as _, SendMessageSetters as _};

/// Enum with all possible authorized states.
#[derive(Debug, Clone, From, PartialEq, Eq)]
#[allow(clippy::module_name_repetitions, clippy::missing_docs_in_private_items)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct Unauthorized<K> {
    /// Secret token generated on every run.
    /// User should copy this token from logs and send to the bot in order to prove that they are the admin.
    pub(super) admin_token: String,
    /// Kind of an unauthorized state.
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

    /// Macro to implement conversion from concrete authorized state to the general [`State`].
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
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Default;

    /// Start of the dialog. Waiting for user signing in.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Start;

    /// Waiting for user to enter a secret phrase spawned in logs to prove that
    /// they are the admin.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct WaitingForSecretPhrase;

    into_state!(Default, Start, WaitingForSecretPhrase);
}

impl Unauthorized<kind::Start> {
    /// Create a new [Start](kind::Start) state and prepare dialog for it.
    async fn setup(context: &Context, admin_token: String) -> color_eyre::Result<Self> {
        let keyboard = KeyboardMarkup::new([[KeyboardButton::new(message::SignIn.to_string())]])
            .resize_keyboard(Some(true));

        context
            .bot()
            .send_message(context.chat_id(), "Please, sign in 🔐")
            .reply_markup(keyboard)
            .await?;

        Ok(Self {
            admin_token,
            kind: kind::Start,
        })
    }

    /// Send welcome message to greet the user.
    async fn send_welcome_message(context: &Context) -> color_eyre::Result<()> {
        let bot = context.bot();

        let me = bot.get_me().await?;
        let bot_name = me.user().full_name();

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

#[async_trait]
impl TryFromTransition<Unauthorized<kind::Start>, message::SignIn>
    for Unauthorized<kind::WaitingForSecretPhrase>
{
    type ErrorTarget = Unauthorized<kind::Start>;

    async fn try_from_transition(
        start: Unauthorized<kind::Start>,
        _sign_in: message::SignIn,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        try_with_target!(
            start,
            context
                .bot()
                .send_message(
                    context.chat_id(),
                    "Please, enter the admin token spawned in the server logs.\n\n\
                     Type /cancel to go back.",
                )
                .reply_markup(KeyboardRemove::new())
                .await
                .map_err(TransitionFailureReason::internal)
        );

        Ok(Self {
            admin_token: start.admin_token,
            kind: kind::WaitingForSecretPhrase,
        })
    }
}

#[async_trait]
impl TryFromTransition<Unauthorized<kind::WaitingForSecretPhrase>, command::Cancel>
    for Unauthorized<kind::Start>
{
    type ErrorTarget = Unauthorized<kind::WaitingForSecretPhrase>;

    async fn try_from_transition(
        waiting_for_secret_phrase: Unauthorized<kind::WaitingForSecretPhrase>,
        _cancel: command::Cancel,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        let start = try_with_target!(
            waiting_for_secret_phrase,
            Self::setup(context, waiting_for_secret_phrase.admin_token.clone())
                .await
                .map_err(TransitionFailureReason::internal)
        );
        Ok(start)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use mockall::predicate;
    use teloxide::types::{ChatId, KeyboardButton, KeyboardMarkup, User};

    use super::*;
    use crate::{
        command::Command,
        message::Message,
        state::{test_utils::CHAT_ID, State},
        Bot, GetMe, Me, SendMessage,
    };

    fn user() -> User {
        User {
            id: teloxide::types::UserId(0),
            is_bot: false,
            first_name: String::from("Test"),
            last_name: None,
            username: None,
            language_code: None,
            is_premium: false,
            added_to_attachment_menu: false,
        }
    }

    #[allow(
        dead_code,
        unreachable_code,
        unused_variables,
        clippy::unimplemented,
        clippy::diverging_sub_expression,
        clippy::panic
    )]
    #[forbid(clippy::todo, clippy::wildcard_enum_match_arm)]
    fn tests_completeness_static_check() -> ! {
        use self::{command::*, message::*};

        panic!("You should never call this function, it's purpose is the static check only");

        // We don't need the actual values, we just need something to check match arms
        let unauthorized: UnauthorizedBox = unimplemented!();
        let cmd: Command = unimplemented!();
        let mes: Message = unimplemented!();

        // Will fail to compile if a new state or command will be added
        match (unauthorized, cmd) {
            (UnauthorizedBox::Default(_), Command::Help(_)) => default_help_success(),
            (UnauthorizedBox::Default(_), Command::Start(_)) => default_start_success(),
            (UnauthorizedBox::Default(_), Command::Cancel(_)) => default_cancel_failure(),
            (UnauthorizedBox::Start(_), Command::Help(_)) => start_help_success(),
            (UnauthorizedBox::Start(_), Command::Start(_)) => start_start_success(),
            (UnauthorizedBox::Start(_), Command::Cancel(_)) => start_cancel_failure(),
            (UnauthorizedBox::WaitingForSecretPhrase(_), Command::Help(_)) => {
                waiting_for_secret_phrase_help_success()
            }
            (UnauthorizedBox::WaitingForSecretPhrase(_), Command::Start(_)) => {
                waiting_for_secret_phrase_start_failure()
            }
            (UnauthorizedBox::WaitingForSecretPhrase(_), Command::Cancel(_)) => {
                waiting_for_secret_phrase_cancel_success()
            }
        }

        // Will fail to compile if a new state or message will be added
        match (unauthorized, mes) {
            (UnauthorizedBox::Default(_), Message::SignIn(_)) => default_sign_in_failure(),
            (UnauthorizedBox::Default(_), Message::List(_)) => default_list_failure(),
            (UnauthorizedBox::Default(_), Message::Arbitrary(_)) => default_arbitrary_failure(),
            (UnauthorizedBox::Start(_), Message::SignIn(_)) => start_sign_in_success(),
            (UnauthorizedBox::Start(_), Message::List(_)) => start_list_failure(),
            (UnauthorizedBox::Start(_), Message::Arbitrary(_)) => start_arbitrary_failure(),
            (UnauthorizedBox::WaitingForSecretPhrase(_), Message::SignIn(_)) => {
                waiting_for_secret_phrase_sign_in_failure()
            }
            (UnauthorizedBox::WaitingForSecretPhrase(_), Message::List(_)) => {
                waiting_for_secret_phrase_list_failure()
            }
            (UnauthorizedBox::WaitingForSecretPhrase(_), Message::Arbitrary(_)) => {
                waiting_for_secret_phrase_wrong_arbitrary_failure();
                waiting_for_secret_phrase_right_arbitrary_success()
            }
        }

        unreachable!()
    }

    mod command {
        //! Test names follow the rule: *state*_*command*_*success/failure*.

        use tokio::test;

        use super::*;
        use crate::state::test_utils::{test_help_success, test_unavailable_command};

        #[test]
        pub async fn default_help_success() {
            let default =
                State::Unauthorized(UnauthorizedBox::Default(Unauthorized::<kind::Default> {
                    admin_token: String::from("test"),
                    kind: kind::Default,
                }));

            test_help_success(default).await
        }

        #[test]
        pub async fn default_start_success() {
            let default =
                State::Unauthorized(UnauthorizedBox::Default(Unauthorized::<kind::Default> {
                    admin_token: String::from("test"),
                    kind: kind::Default,
                }));
            let start = Command::Start(crate::command::Start);

            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);

            let mut mock_bot = Bot::default();
            mock_bot
                .expect_send_message::<ChatId, String>()
                .with(
                    predicate::eq(CHAT_ID),
                    predicate::eq(
                        "👋🤖 Welcome to Test bot!\n\n\
                             I'll help you to manage your passwords."
                            .to_owned(),
                    ),
                )
                .returning(|_chat_id, _message| SendMessage::default());
            mock_bot
                .expect_send_message::<ChatId, &'static str>()
                .with(predicate::eq(CHAT_ID), predicate::eq("Please, sign in 🔐"))
                .returning(|_chat_id, _message| {
                    let mut mock_send_message = SendMessage::default();

                    let expected_keyboard = KeyboardMarkup::new([[KeyboardButton::new(
                        crate::message::SignIn.to_string(),
                    )]])
                    .resize_keyboard(Some(true));

                    mock_send_message
                        .expect_reply_markup::<KeyboardMarkup>()
                        .with(predicate::eq(expected_keyboard))
                        .returning(|_markup| SendMessage::default());
                    mock_send_message
                });
            mock_bot.expect_get_me().returning(|| {
                let mut mock_me = Me::default();
                mock_me.expect_user().return_const(user());
                GetMe::new(mock_me)
            });
            mock_context.expect_bot().return_const(mock_bot);

            let state = State::try_from_transition(default, start, &mock_context)
                .await
                .unwrap();
            assert!(matches!(
                state,
                State::Unauthorized(UnauthorizedBox::Start(_))
            ))
        }

        #[test]
        pub async fn default_cancel_failure() {
            let default =
                State::Unauthorized(UnauthorizedBox::Default(Unauthorized::<kind::Default> {
                    admin_token: String::from("test"),
                    kind: kind::Default,
                }));
            let cancel = Command::Cancel(crate::command::Cancel);

            test_unavailable_command(default, cancel).await
        }

        #[test]
        pub async fn start_help_success() {
            let start = State::Unauthorized(UnauthorizedBox::Start(Unauthorized::<kind::Start> {
                admin_token: String::from("test"),
                kind: kind::Start,
            }));
            test_help_success(start).await
        }

        #[test]
        pub async fn start_start_success() {
            let start = State::Unauthorized(UnauthorizedBox::Start(Unauthorized::<kind::Start> {
                admin_token: String::from("test"),
                kind: kind::Start,
            }));
            let start_cmd = Command::Start(crate::command::Start);

            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);

            let mut mock_bot = Bot::default();
            mock_bot
                .expect_send_message::<ChatId, &'static str>()
                .with(predicate::eq(CHAT_ID), predicate::eq("Please, sign in 🔐"))
                .returning(|_chat_id, _message| {
                    let mut mock_send_message = SendMessage::default();

                    let expected_keyboard = KeyboardMarkup::new([[KeyboardButton::new(
                        crate::message::SignIn.to_string(),
                    )]])
                    .resize_keyboard(Some(true));

                    mock_send_message
                        .expect_reply_markup::<KeyboardMarkup>()
                        .with(predicate::eq(expected_keyboard))
                        .returning(|_markup| SendMessage::default());
                    mock_send_message
                });
            mock_bot.expect_get_me().returning(|| {
                let mut mock_me = Me::default();
                mock_me.expect_user().return_const(user());
                GetMe::new(mock_me)
            });
            mock_context.expect_bot().return_const(mock_bot);

            let state = State::try_from_transition(start, start_cmd, &mock_context)
                .await
                .unwrap();
            assert!(matches!(
                state,
                State::Unauthorized(UnauthorizedBox::Start(_))
            ))
        }

        #[test]
        pub async fn start_cancel_failure() {
            let start = State::Unauthorized(UnauthorizedBox::Start(Unauthorized::<kind::Start> {
                admin_token: String::from("test"),
                kind: kind::Start,
            }));
            let cancel = Command::Cancel(crate::command::Cancel);

            test_unavailable_command(start, cancel).await
        }

        #[test]
        pub async fn waiting_for_secret_phrase_help_success() {
            let waiting_for_secret_phrase =
                State::Unauthorized(UnauthorizedBox::WaitingForSecretPhrase(Unauthorized::<
                    kind::WaitingForSecretPhrase,
                > {
                    admin_token: String::from("test"),
                    kind: kind::WaitingForSecretPhrase,
                }));
            test_help_success(waiting_for_secret_phrase).await
        }

        #[test]
        pub async fn waiting_for_secret_phrase_start_failure() {
            let waiting_for_secret_phrase =
                State::Unauthorized(UnauthorizedBox::WaitingForSecretPhrase(Unauthorized::<
                    kind::WaitingForSecretPhrase,
                > {
                    admin_token: String::from("test"),
                    kind: kind::WaitingForSecretPhrase,
                }));
            let start = Command::Start(crate::command::Start);

            test_unavailable_command(waiting_for_secret_phrase, start).await
        }

        #[test]
        pub async fn waiting_for_secret_phrase_cancel_success() {
            let waiting_for_secret_phrase =
                State::Unauthorized(UnauthorizedBox::WaitingForSecretPhrase(Unauthorized::<
                    kind::WaitingForSecretPhrase,
                > {
                    admin_token: String::from("test"),
                    kind: kind::WaitingForSecretPhrase,
                }));
            let cancel = Command::Cancel(crate::command::Cancel);

            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);

            let mut mock_bot = Bot::default();
            mock_bot
                .expect_send_message::<ChatId, &'static str>()
                .with(predicate::eq(CHAT_ID), predicate::eq("Please, sign in 🔐"))
                .returning(|_chat_id, _message| {
                    let mut mock_send_message = SendMessage::default();

                    let expected_keyboard = KeyboardMarkup::new([[KeyboardButton::new(
                        crate::message::SignIn.to_string(),
                    )]])
                    .resize_keyboard(Some(true));

                    mock_send_message
                        .expect_reply_markup::<KeyboardMarkup>()
                        .with(predicate::eq(expected_keyboard))
                        .returning(|_markup| SendMessage::default());
                    mock_send_message
                });
            mock_bot.expect_get_me().returning(|| {
                let mut mock_me = Me::default();
                mock_me.expect_user().return_const(user());
                GetMe::new(mock_me)
            });
            mock_context.expect_bot().return_const(mock_bot);

            let state =
                State::try_from_transition(waiting_for_secret_phrase, cancel, &mock_context)
                    .await
                    .unwrap();
            assert!(matches!(
                state,
                State::Unauthorized(UnauthorizedBox::Start(_))
            ))
        }
    }

    mod message {
        use tokio::test;

        use super::*;
        use crate::state::{authorized::AuthorizedBox, test_utils::test_unexpected_message};

        #[test]
        pub async fn default_sign_in_failure() {
            let default =
                State::Unauthorized(UnauthorizedBox::Default(Unauthorized::<kind::Default> {
                    admin_token: String::from("test"),
                    kind: kind::Default,
                }));
            let sign_in = Message::SignIn(crate::message::SignIn);

            test_unexpected_message(default, sign_in).await
        }

        #[test]
        pub async fn default_list_failure() {
            let default =
                State::Unauthorized(UnauthorizedBox::Default(Unauthorized::<kind::Default> {
                    admin_token: String::from("test"),
                    kind: kind::Default,
                }));
            let list = Message::List(crate::message::List);

            test_unexpected_message(default, list).await
        }

        #[test]
        pub async fn default_arbitrary_failure() {
            let default =
                State::Unauthorized(UnauthorizedBox::Default(Unauthorized::<kind::Default> {
                    admin_token: String::from("test"),
                    kind: kind::Default,
                }));
            let arbitrary = Message::Arbitrary(crate::message::Arbitrary(
                "Test arbitrary message".to_owned(),
            ));

            test_unexpected_message(default, arbitrary).await
        }

        #[test]
        pub async fn start_sign_in_success() {
            let start = State::Unauthorized(UnauthorizedBox::Start(Unauthorized::<kind::Start> {
                admin_token: String::from("test"),
                kind: kind::Start,
            }));
            let sign_in = Message::SignIn(crate::message::SignIn);

            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);

            let mut mock_bot = Bot::default();
            mock_bot
                .expect_send_message::<ChatId, &'static str>()
                .with(
                    predicate::eq(CHAT_ID),
                    predicate::eq(
                        "Please, enter the admin token spawned in the server logs.\n\n\
                             Type /cancel to go back.",
                    ),
                )
                .returning(|_chat_id, _message| {
                    let mut mock_send_message = SendMessage::default();

                    let expected_keyboard = KeyboardRemove::new();

                    mock_send_message
                        .expect_reply_markup::<KeyboardRemove>()
                        .with(predicate::eq(expected_keyboard))
                        .returning(|_markup| SendMessage::default());
                    mock_send_message
                });
            mock_context.expect_bot().return_const(mock_bot);

            let state = State::try_from_transition(start, sign_in, &mock_context)
                .await
                .unwrap();
            assert!(matches!(
                state,
                State::Unauthorized(UnauthorizedBox::WaitingForSecretPhrase(_))
            ))
        }

        #[test]
        pub async fn start_list_failure() {
            let start = State::Unauthorized(UnauthorizedBox::Start(Unauthorized::<kind::Start> {
                admin_token: String::from("test"),
                kind: kind::Start,
            }));
            let list = Message::List(crate::message::List);

            test_unexpected_message(start, list).await
        }

        #[test]
        pub async fn start_arbitrary_failure() {
            let start = State::Unauthorized(UnauthorizedBox::Start(Unauthorized::<kind::Start> {
                admin_token: String::from("test"),
                kind: kind::Start,
            }));
            let arbitrary = Message::Arbitrary(crate::message::Arbitrary(
                "Test arbitrary message".to_owned(),
            ));

            test_unexpected_message(start, arbitrary).await
        }

        #[test]
        pub async fn waiting_for_secret_phrase_sign_in_failure() {
            let waiting_for_secret_phrase =
                State::Unauthorized(UnauthorizedBox::WaitingForSecretPhrase(Unauthorized::<
                    kind::WaitingForSecretPhrase,
                > {
                    admin_token: String::from("test"),
                    kind: kind::WaitingForSecretPhrase,
                }));
            let sign_in = Message::SignIn(crate::message::SignIn);

            test_unexpected_message(waiting_for_secret_phrase, sign_in).await
        }

        #[test]
        pub async fn waiting_for_secret_phrase_list_failure() {
            let waiting_for_secret_phrase =
                State::Unauthorized(UnauthorizedBox::WaitingForSecretPhrase(Unauthorized::<
                    kind::WaitingForSecretPhrase,
                > {
                    admin_token: String::from("test"),
                    kind: kind::WaitingForSecretPhrase,
                }));
            let list = Message::List(crate::message::List);

            test_unexpected_message(waiting_for_secret_phrase, list).await
        }

        #[test]
        pub async fn waiting_for_secret_phrase_wrong_arbitrary_failure() {
            let waiting_for_secret_phrase =
                State::Unauthorized(UnauthorizedBox::WaitingForSecretPhrase(Unauthorized::<
                    kind::WaitingForSecretPhrase,
                > {
                    admin_token: String::from("test"),
                    kind: kind::WaitingForSecretPhrase,
                }));
            let wrong_arbitrary =
                Message::Arbitrary(crate::message::Arbitrary("Wrong test phrase".to_owned()));

            let mock_context = Context::default();

            let err = State::try_from_transition(
                waiting_for_secret_phrase,
                wrong_arbitrary,
                &mock_context,
            )
            .await
            .unwrap_err();
            assert!(matches!(
                err.reason,
                TransitionFailureReason::User(user_mistake) if user_mistake == "❎ Invalid token. Please, try again.",
            ));
            assert!(matches!(
                err.target,
                State::Unauthorized(UnauthorizedBox::WaitingForSecretPhrase(_))
            ))
        }

        #[test]
        pub async fn waiting_for_secret_phrase_right_arbitrary_success() {
            let waiting_for_secret_phrase =
                State::Unauthorized(UnauthorizedBox::WaitingForSecretPhrase(Unauthorized::<
                    kind::WaitingForSecretPhrase,
                > {
                    admin_token: String::from("test"),
                    kind: kind::WaitingForSecretPhrase,
                }));
            let right_arbitrary = Message::Arbitrary(crate::message::Arbitrary("test".to_owned()));

            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);

            let mut mock_bot = Bot::default();
            mock_bot
                .expect_send_message::<ChatId, &'static str>()
                .with(
                    predicate::eq(CHAT_ID),
                    predicate::eq("✅ You've successfully signed in!"),
                )
                .returning(|_chat_id, _message| SendMessage::default());
            mock_bot
                .expect_send_message::<ChatId, &'static str>()
                .with(
                    predicate::eq(CHAT_ID),
                    predicate::eq("🏠 Welcome to the main menu."),
                )
                .returning(|_chat_id, _message| {
                    let mut mock_send_message = SendMessage::default();

                    let expected_buttons =
                        [[KeyboardButton::new(crate::message::List.to_string())]];
                    let expected_keyboard =
                        KeyboardMarkup::new(expected_buttons).resize_keyboard(Some(true));

                    mock_send_message
                        .expect_reply_markup::<KeyboardMarkup>()
                        .with(predicate::eq(expected_keyboard))
                        .returning(|_markup| SendMessage::default());
                    mock_send_message
                });
            mock_context.expect_bot().return_const(mock_bot);

            let state = State::try_from_transition(
                waiting_for_secret_phrase,
                right_arbitrary,
                &mock_context,
            )
            .await
            .unwrap();
            assert!(matches!(
                state,
                State::Authorized(AuthorizedBox::MainMenu(_))
            ))
        }
    }
}
