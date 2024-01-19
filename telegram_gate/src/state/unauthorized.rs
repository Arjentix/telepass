//! Module with [`Unauthorized`] states.

use teloxide::types::{KeyboardButton, KeyboardMarkup, KeyboardRemove};

use super::{
    async_trait, command, message, try_with_state, Context, FailedTransition, From,
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
    SecretPhrasePrompt(Unauthorized<kind::SecretPhrasePrompt>),
}

impl Default for UnauthorizedBox {
    fn default() -> Self {
        Self::Default(Unauthorized::default())
    }
}

#[cfg(test)]
#[allow(clippy::multiple_inherent_impl)]
impl UnauthorizedBox {
    #[must_use]
    pub fn default_test() -> Self {
        Self::Default(Unauthorized {
            admin_token: String::from("test"),
            kind: kind::Default,
        })
    }

    #[must_use]
    pub fn start() -> Self {
        Self::Start(Unauthorized {
            admin_token: String::from("test"),
            kind: kind::Start,
        })
    }

    #[must_use]
    pub fn secret_phrase_prompt() -> Self {
        Self::SecretPhrasePrompt(Unauthorized {
            admin_token: String::from("test"),
            kind: kind::SecretPhrasePrompt,
        })
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
    pub struct SecretPhrasePrompt;

    into_state!(Default, Start, SecretPhrasePrompt);
}

impl Unauthorized<kind::Start> {
    /// Create a new [Start](kind::Start) state and prepare dialog for it.
    async fn setup(context: &Context, admin_token: String) -> color_eyre::Result<Self> {
        let keyboard =
            KeyboardMarkup::new([[KeyboardButton::new(message::kind::SignIn.to_string())]])
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
        try_with_state!(
            default,
            Self::send_welcome_message(context)
                .await
                .map_err(TransitionFailureReason::internal)
        );

        let start = try_with_state!(
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
        let new_start = try_with_state!(
            start,
            Self::setup(context, start.admin_token.clone())
                .await
                .map_err(TransitionFailureReason::internal)
        );
        Ok(new_start)
    }
}

#[async_trait]
impl TryFromTransition<Unauthorized<kind::Start>, message::Message<message::kind::SignIn>>
    for Unauthorized<kind::SecretPhrasePrompt>
{
    type ErrorTarget = Unauthorized<kind::Start>;

    async fn try_from_transition(
        start: Unauthorized<kind::Start>,
        _sign_in: message::Message<message::kind::SignIn>,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        try_with_state!(
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
            kind: kind::SecretPhrasePrompt,
        })
    }
}

#[async_trait]
impl TryFromTransition<Unauthorized<kind::SecretPhrasePrompt>, command::Cancel>
    for Unauthorized<kind::Start>
{
    type ErrorTarget = Unauthorized<kind::SecretPhrasePrompt>;

    async fn try_from_transition(
        secret_phrase_prompt: Unauthorized<kind::SecretPhrasePrompt>,
        _cancel: command::Cancel,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        let start = try_with_state!(
            secret_phrase_prompt,
            Self::setup(context, secret_phrase_prompt.admin_token.clone())
                .await
                .map_err(TransitionFailureReason::internal)
        );
        Ok(start)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use teloxide::types::{KeyboardButton, KeyboardMarkup};

    use super::*;
    use crate::{
        button::ButtonBox,
        command::Command,
        message::MessageBox,
        state::State,
        test_utils::mock_bot::{MockBotBuilder, CHAT_ID},
    };

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
        use self::{button::*, command::*, message::*};

        panic!("You should never call this function, it's purpose is the static check only");

        // We don't need the actual values, we just need something to check match arms
        let unauthorized: UnauthorizedBox = unimplemented!();
        let cmd: Command = unimplemented!();
        let mes: MessageBox = unimplemented!();
        let button: ButtonBox = unimplemented!();

        // Will fail to compile if a new state or command will be added
        match (unauthorized, cmd) {
            (UnauthorizedBox::Default(_), Command::Help(_)) => default_help_success(),
            (UnauthorizedBox::Default(_), Command::Start(_)) => default_start_success(),
            (UnauthorizedBox::Default(_), Command::Cancel(_)) => default_cancel_failure(),
            (UnauthorizedBox::Start(_), Command::Help(_)) => start_help_success(),
            (UnauthorizedBox::Start(_), Command::Start(_)) => start_start_success(),
            (UnauthorizedBox::Start(_), Command::Cancel(_)) => start_cancel_failure(),
            (UnauthorizedBox::SecretPhrasePrompt(_), Command::Help(_)) => {
                secret_phrase_prompt_help_success()
            }
            (UnauthorizedBox::SecretPhrasePrompt(_), Command::Start(_)) => {
                secret_phrase_prompt_start_failure()
            }
            (UnauthorizedBox::SecretPhrasePrompt(_), Command::Cancel(_)) => {
                secret_phrase_prompt_cancel_success()
            }
        }

        // Will fail to compile if a new state or message will be added
        match (unauthorized, mes) {
            (UnauthorizedBox::Default(_), MessageBox::WebApp(_)) => default_web_app_failure(),
            (UnauthorizedBox::Default(_), MessageBox::SignIn(_)) => default_sign_in_failure(),
            (UnauthorizedBox::Default(_), MessageBox::List(_)) => default_list_failure(),
            (UnauthorizedBox::Default(_), MessageBox::Add(_)) => default_add_failure(),
            (UnauthorizedBox::Default(_), MessageBox::Arbitrary(_)) => default_arbitrary_failure(),
            (UnauthorizedBox::Start(_), MessageBox::WebApp(_)) => start_web_app_failure(),
            (UnauthorizedBox::Start(_), MessageBox::SignIn(_)) => start_sign_in_success(),
            (UnauthorizedBox::Start(_), MessageBox::List(_)) => start_list_failure(),
            (UnauthorizedBox::Start(_), MessageBox::Add(_)) => start_add_failure(),
            (UnauthorizedBox::Start(_), MessageBox::Arbitrary(_)) => start_arbitrary_failure(),
            (UnauthorizedBox::SecretPhrasePrompt(_), MessageBox::WebApp(_)) => {
                secret_phrase_prompt_web_app_failure()
            }
            (UnauthorizedBox::SecretPhrasePrompt(_), MessageBox::SignIn(_)) => {
                secret_phrase_prompt_sign_in_failure()
            }
            (UnauthorizedBox::SecretPhrasePrompt(_), MessageBox::List(_)) => {
                secret_phrase_prompt_list_failure()
            }
            (UnauthorizedBox::SecretPhrasePrompt(_), MessageBox::Add(_)) => {
                secret_phrase_prompt_add_failure()
            }
            (UnauthorizedBox::SecretPhrasePrompt(_), MessageBox::Arbitrary(_)) => {
                secret_phrase_prompt_wrong_arbitrary_failure();
                secret_phrase_prompt_right_arbitrary_success()
            }
        }

        // Will fail to compile if a new state or button will be added
        match (unauthorized, button) {
            (UnauthorizedBox::Default(_), ButtonBox::Delete(_)) => default_delete_failure(),
            (UnauthorizedBox::Default(_), ButtonBox::Yes(_)) => default_yes_failure(),
            (UnauthorizedBox::Default(_), ButtonBox::No(_)) => default_no_failure(),
            (UnauthorizedBox::Start(_), ButtonBox::Delete(_)) => start_delete_failure(),
            (UnauthorizedBox::Start(_), ButtonBox::Yes(_)) => start_yes_failure(),
            (UnauthorizedBox::Start(_), ButtonBox::No(_)) => start_no_failure(),
            (UnauthorizedBox::SecretPhrasePrompt(_), ButtonBox::Delete(_)) => {
                secret_phrase_prompt_delete_failure()
            }
            (UnauthorizedBox::SecretPhrasePrompt(_), ButtonBox::Yes(_)) => {
                secret_phrase_prompt_yes_failure()
            }
            (UnauthorizedBox::SecretPhrasePrompt(_), ButtonBox::No(_)) => {
                secret_phrase_prompt_no_failure()
            }
        }

        unreachable!()
    }

    mod command {
        //! Test names follow the rule: *state*_*command*_*success/failure*.

        use tokio::test;

        use super::*;
        use crate::test_utils::{test_help_success, test_unavailable_command};

        #[test]
        pub async fn default_help_success() {
            let default = State::Unauthorized(UnauthorizedBox::default_test());

            test_help_success(default).await
        }

        #[test]
        pub async fn default_start_success() {
            let default = State::Unauthorized(UnauthorizedBox::default_test());
            let start = Command::Start(crate::command::Start);

            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);
            mock_context.expect_bot().return_const(
                MockBotBuilder::new()
                    .expect_get_me()
                    .expect_send_message(
                        "👋🤖 Welcome to Test bot!\n\n\
                         I'll help you to manage your passwords."
                            .to_owned(),
                    )
                    .expect_into_future()
                    .expect_send_message("Please, sign in 🔐")
                    .expect_reply_markup(
                        KeyboardMarkup::new([[KeyboardButton::new(
                            crate::message::kind::SignIn.to_string(),
                        )]])
                        .resize_keyboard(Some(true)),
                    )
                    .expect_into_future()
                    .build(),
            );

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
            let default = State::Unauthorized(UnauthorizedBox::default_test());
            let cancel = Command::Cancel(crate::command::Cancel);

            test_unavailable_command(default, cancel).await
        }

        #[test]
        pub async fn start_help_success() {
            let start = State::Unauthorized(UnauthorizedBox::start());
            test_help_success(start).await
        }

        #[test]
        pub async fn start_start_success() {
            let start = State::Unauthorized(UnauthorizedBox::start());
            let start_cmd = Command::Start(crate::command::Start);

            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);
            mock_context.expect_bot().return_const(
                MockBotBuilder::new()
                    .expect_get_me()
                    .expect_send_message("Please, sign in 🔐")
                    .expect_reply_markup(
                        KeyboardMarkup::new([[KeyboardButton::new(
                            crate::message::kind::SignIn.to_string(),
                        )]])
                        .resize_keyboard(Some(true)),
                    )
                    .expect_into_future()
                    .build(),
            );

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
            let start = State::Unauthorized(UnauthorizedBox::start());
            let cancel = Command::Cancel(crate::command::Cancel);

            test_unavailable_command(start, cancel).await
        }

        #[test]
        pub async fn secret_phrase_prompt_help_success() {
            let secret_phrase_prompt = State::Unauthorized(UnauthorizedBox::secret_phrase_prompt());
            test_help_success(secret_phrase_prompt).await
        }

        #[test]
        pub async fn secret_phrase_prompt_start_failure() {
            let secret_phrase_prompt = State::Unauthorized(UnauthorizedBox::secret_phrase_prompt());
            let start = Command::Start(crate::command::Start);

            test_unavailable_command(secret_phrase_prompt, start).await
        }

        #[test]
        pub async fn secret_phrase_prompt_cancel_success() {
            let secret_phrase_prompt = State::Unauthorized(UnauthorizedBox::secret_phrase_prompt());
            let cancel = Command::Cancel(crate::command::Cancel);

            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);
            mock_context.expect_bot().return_const(
                MockBotBuilder::new()
                    .expect_get_me()
                    .expect_send_message("Please, sign in 🔐")
                    .expect_reply_markup(
                        KeyboardMarkup::new([[KeyboardButton::new(
                            crate::message::kind::SignIn.to_string(),
                        )]])
                        .resize_keyboard(Some(true)),
                    )
                    .expect_into_future()
                    .build(),
            );

            let state = State::try_from_transition(secret_phrase_prompt, cancel, &mock_context)
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
        use crate::{
            state::authorized::AuthorizedBox,
            test_utils::{test_unexpected_message, web_app_test_url},
        };

        #[test]
        pub async fn default_web_app_failure() {
            let default = State::Unauthorized(UnauthorizedBox::default_test());
            let web_app = MessageBox::web_app("data".to_owned(), "button_text".to_owned());

            test_unexpected_message(default, web_app).await
        }

        #[test]
        pub async fn default_add_failure() {
            let default = State::Unauthorized(UnauthorizedBox::default_test());
            let add = MessageBox::add();

            test_unexpected_message(default, add).await
        }

        #[test]
        pub async fn default_list_failure() {
            let default = State::Unauthorized(UnauthorizedBox::default_test());
            let list = MessageBox::list();

            test_unexpected_message(default, list).await
        }

        #[test]
        pub async fn default_sign_in_failure() {
            let default = State::Unauthorized(UnauthorizedBox::default_test());
            let sign_in = MessageBox::sign_in();

            test_unexpected_message(default, sign_in).await
        }

        #[test]
        pub async fn default_arbitrary_failure() {
            let default = State::Unauthorized(UnauthorizedBox::default_test());
            let arbitrary = MessageBox::arbitrary("Test arbitrary message");

            test_unexpected_message(default, arbitrary).await
        }

        #[test]
        pub async fn start_web_app_failure() {
            let start = State::Unauthorized(UnauthorizedBox::start());
            let web_app = MessageBox::web_app("data".to_owned(), "button_text".to_owned());

            test_unexpected_message(start, web_app).await
        }

        #[test]
        pub async fn start_sign_in_success() {
            let start = State::Unauthorized(UnauthorizedBox::start());
            let sign_in = MessageBox::sign_in();

            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);
            mock_context.expect_bot().return_const(
                MockBotBuilder::new()
                    .expect_send_message(
                        "Please, enter the admin token spawned in the server logs.\n\n\
                         Type /cancel to go back.",
                    )
                    .expect_reply_markup(KeyboardRemove::new())
                    .expect_into_future()
                    .build(),
            );

            let state = State::try_from_transition(start, sign_in, &mock_context)
                .await
                .unwrap();
            assert!(matches!(
                state,
                State::Unauthorized(UnauthorizedBox::SecretPhrasePrompt(_))
            ))
        }

        #[test]
        pub async fn start_list_failure() {
            let start = State::Unauthorized(UnauthorizedBox::start());
            let list = MessageBox::list();

            test_unexpected_message(start, list).await
        }

        #[test]
        pub async fn start_add_failure() {
            let start = State::Unauthorized(UnauthorizedBox::start());
            let add = MessageBox::add();

            test_unexpected_message(start, add).await
        }

        #[test]
        pub async fn start_arbitrary_failure() {
            let start = State::Unauthorized(UnauthorizedBox::start());
            let arbitrary = MessageBox::arbitrary("Test arbitrary message");

            test_unexpected_message(start, arbitrary).await
        }

        #[test]
        pub async fn secret_phrase_prompt_web_app_failure() {
            let secret_phrase_prompt = State::Unauthorized(UnauthorizedBox::secret_phrase_prompt());
            let web_app = MessageBox::web_app("data".to_owned(), "button_text".to_owned());

            test_unexpected_message(secret_phrase_prompt, web_app).await
        }

        #[test]
        pub async fn secret_phrase_prompt_sign_in_failure() {
            let secret_phrase_prompt = State::Unauthorized(UnauthorizedBox::secret_phrase_prompt());
            let sign_in = MessageBox::sign_in();

            test_unexpected_message(secret_phrase_prompt, sign_in).await
        }

        #[test]
        pub async fn secret_phrase_prompt_list_failure() {
            let secret_phrase_prompt = State::Unauthorized(UnauthorizedBox::secret_phrase_prompt());
            let list = MessageBox::list();

            test_unexpected_message(secret_phrase_prompt, list).await
        }

        #[test]
        pub async fn secret_phrase_prompt_add_failure() {
            let secret_phrase_prompt = State::Unauthorized(UnauthorizedBox::secret_phrase_prompt());
            let add = MessageBox::add();

            test_unexpected_message(secret_phrase_prompt, add).await
        }

        #[test]
        pub async fn secret_phrase_prompt_wrong_arbitrary_failure() {
            let secret_phrase_prompt = State::Unauthorized(UnauthorizedBox::secret_phrase_prompt());
            let wrong_arbitrary = MessageBox::arbitrary("Wrong test phrase");

            let mock_context = Context::default();

            let err =
                State::try_from_transition(secret_phrase_prompt, wrong_arbitrary, &mock_context)
                    .await
                    .unwrap_err();
            assert!(matches!(
                err.reason,
                TransitionFailureReason::User(user_mistake) if user_mistake == "❎ Invalid token. Please, try again.",
            ));
            assert!(matches!(
                err.target,
                State::Unauthorized(UnauthorizedBox::SecretPhrasePrompt(_))
            ))
        }

        #[test]
        pub async fn secret_phrase_prompt_right_arbitrary_success() {
            let secret_phrase_prompt = State::Unauthorized(UnauthorizedBox::secret_phrase_prompt());
            let right_arbitrary = MessageBox::arbitrary("test");

            let expected_buttons = [
                [KeyboardButton::new(crate::message::kind::List.to_string())],
                [
                    KeyboardButton::new(crate::message::kind::Add.to_string()).request(
                        teloxide::types::ButtonRequest::WebApp(teloxide::types::WebAppInfo {
                            url: web_app_test_url(),
                        }),
                    ),
                ],
            ];
            let expected_keyboard = KeyboardMarkup::new(expected_buttons).resize_keyboard(true);
            let mut mock_context = Context::default();
            mock_context.expect_chat_id().return_const(CHAT_ID);
            mock_context
                .expect_web_app_url()
                .return_const(web_app_test_url());
            mock_context.expect_bot().return_const(
                MockBotBuilder::new()
                    .expect_send_message("✅ You've successfully signed in!")
                    .expect_into_future()
                    .expect_send_message("🏠 Welcome to the main menu.")
                    .expect_reply_markup(expected_keyboard)
                    .expect_into_future()
                    .build(),
            );

            let state =
                State::try_from_transition(secret_phrase_prompt, right_arbitrary, &mock_context)
                    .await
                    .unwrap();
            assert!(matches!(
                state,
                State::Authorized(AuthorizedBox::MainMenu(_))
            ))
        }
    }

    mod button {
        use tokio::test;

        use super::*;
        use crate::test_utils::test_unexpected_button;

        #[test]
        pub async fn default_delete_failure() {
            let default = State::Unauthorized(UnauthorizedBox::default_test());
            let delete = ButtonBox::delete();

            test_unexpected_button(default, delete).await
        }

        #[test]
        pub async fn default_yes_failure() {
            let default = State::Unauthorized(UnauthorizedBox::default_test());
            let yes = ButtonBox::yes();

            test_unexpected_button(default, yes).await
        }

        #[test]
        pub async fn default_no_failure() {
            let default = State::Unauthorized(UnauthorizedBox::default_test());
            let no = ButtonBox::no();

            test_unexpected_button(default, no).await
        }

        #[test]
        pub async fn start_delete_failure() {
            let start = State::Unauthorized(UnauthorizedBox::start());
            let delete = ButtonBox::delete();

            test_unexpected_button(start, delete).await
        }

        #[test]
        pub async fn start_yes_failure() {
            let start = State::Unauthorized(UnauthorizedBox::start());
            let yes = ButtonBox::yes();

            test_unexpected_button(start, yes).await
        }

        #[test]
        pub async fn start_no_failure() {
            let start = State::Unauthorized(UnauthorizedBox::start());
            let no = ButtonBox::no();

            test_unexpected_button(start, no).await
        }

        #[test]
        pub async fn secret_phrase_prompt_delete_failure() {
            let secret_phrase_prompt = State::Unauthorized(UnauthorizedBox::secret_phrase_prompt());
            let delete = ButtonBox::delete();

            test_unexpected_button(secret_phrase_prompt, delete).await
        }

        #[test]
        pub async fn secret_phrase_prompt_yes_failure() {
            let secret_phrase_prompt = State::Unauthorized(UnauthorizedBox::secret_phrase_prompt());
            let yes = ButtonBox::yes();

            test_unexpected_button(secret_phrase_prompt, yes).await
        }

        #[test]
        pub async fn secret_phrase_prompt_no_failure() {
            let secret_phrase_prompt = State::Unauthorized(UnauthorizedBox::secret_phrase_prompt());
            let no = ButtonBox::no();

            test_unexpected_button(secret_phrase_prompt, no).await
        }
    }
}
