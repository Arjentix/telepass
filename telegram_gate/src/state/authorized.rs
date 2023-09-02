//! Module with [`Authorized`] states.

use teloxide::types::{KeyboardButton, KeyboardMarkup};

use super::{
    async_trait, message, try_with_target, unauthorized, Context, FailedTransition, From,
    TransitionFailureReason, TryFromTransition,
};
#[cfg(not(test))]
use super::{Requester as _, SendMessageSetters as _};

mod sealed {
    //! Module with [`Sealed`] and its implementations for authorized states.

    use super::*;

    /// Trait to prevent [`super::Marker`] implementation for types outside of
    /// [`authorized`](super) module.
    pub trait Sealed {}

    impl Sealed for Authorized<kind::MainMenu> {}
}

/// Marker trait to identify *authorized* states.
pub trait Marker: sealed::Sealed {}

impl Marker for Authorized<kind::MainMenu> {}

/// Enum with all possible authorized states.
#[derive(Debug, Clone, From, PartialEq, Eq)]
#[allow(clippy::module_name_repetitions, clippy::missing_docs_in_private_items)]
pub enum AuthorizedBox {
    MainMenu(Authorized<kind::MainMenu>),
}

/// Authorized state.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct Authorized<K> {
    /// Kind of an authorized state.
    _kind: K,
}

pub mod kind {
    //! Module with [`Authorized`] kinds.

    use super::{super::State, Authorized, AuthorizedBox};

    /// Macro to implement conversion from concrete authorized state to the general [`State`].
    macro_rules! into_state {
            ($($kind_ty:ty),+ $(,)?) => {$(
                impl From<Authorized<$kind_ty>> for State {
                    fn from(value: Authorized<$kind_ty>) -> Self {
                        AuthorizedBox::from(value).into()
                    }
                }
            )+};
    }

    /// Main menu state kind.
    ///
    /// Waits for user to input an action.
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct MainMenu;

    into_state!(MainMenu);
}

impl Authorized<kind::MainMenu> {
    /// Setup [`Authorized`] state of [`MainMenu`](kind::MainMenu) kind.
    ///
    /// Prints welcome message and constructs a keyboard with all supported actions.
    async fn setup(context: &Context) -> color_eyre::Result<Self> {
        let main_menu = Self {
            _kind: kind::MainMenu,
        };

        let resources = {
            let storage_client = context.storage_client_from_behalf(&main_menu);
            let mut storage_client_lock = storage_client.lock().await;

            storage_client_lock
                .list(crate::grpc::Empty {})
                .await?
                .into_inner()
        };

        let buttons = resources
            .resources
            .into_iter()
            .map(|resource| [KeyboardButton::new(format!("🔑 {}", resource.name))]);
        let keyboard = KeyboardMarkup::new(buttons).resize_keyboard(Some(true));

        context
            .bot()
            .send_message(context.chat_id(), "🏠 Welcome to the main menu.")
            .reply_markup(keyboard)
            .await?;

        Ok(main_menu)
    }
}

#[async_trait]
impl
    TryFromTransition<
        unauthorized::Unauthorized<unauthorized::kind::WaitingForSecretPhrase>,
        message::Arbitrary,
    > for Authorized<kind::MainMenu>
{
    type ErrorTarget = unauthorized::Unauthorized<unauthorized::kind::WaitingForSecretPhrase>;

    async fn try_from_transition(
        waiting_for_secret_phrase: unauthorized::Unauthorized<
            unauthorized::kind::WaitingForSecretPhrase,
        >,
        message::Arbitrary(admin_token_candidate): message::Arbitrary,
        context: &Context,
    ) -> Result<Self, FailedTransition<Self::ErrorTarget>> {
        if admin_token_candidate != waiting_for_secret_phrase.admin_token {
            return Err(FailedTransition::user(
                waiting_for_secret_phrase,
                "❎ Invalid token. Please, try again.",
            ));
        }

        try_with_target!(
            waiting_for_secret_phrase,
            context
                .bot()
                .send_message(context.chat_id(), "✅ You've successfully signed in!")
                .await
                .map_err(TransitionFailureReason::internal)
        );

        let main_menu = try_with_target!(
            waiting_for_secret_phrase,
            Self::setup(context)
                .await
                .map_err(TransitionFailureReason::internal)
        );
        Ok(main_menu)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{command::Command, message::Message, state::State};

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
        let authorized: AuthorizedBox = unimplemented!();
        let cmd: Command = unimplemented!();
        let mes: Message = unimplemented!();

        // Will fail to compile if a new state or command will be added
        match (authorized, cmd) {
            (AuthorizedBox::MainMenu(_), Command::Help(_)) => main_menu_help_success(),
            (AuthorizedBox::MainMenu(_), Command::Start(_)) => main_menu_start_failure(),
            (AuthorizedBox::MainMenu(_), Command::Cancel(_)) => main_menu_cancel_failure(),
        }

        // Will fail to compile if a new state or message will be added
        match (authorized, mes) {
            (AuthorizedBox::MainMenu(_), Message::SignIn(_)) => main_menu_sign_in_failure(),
            (AuthorizedBox::MainMenu(_), Message::Arbitrary(_)) => main_menu_arbitrary_failure(),
        }

        unreachable!()
    }

    mod command {
        //! Test names follow the rule: *state*_*command*_*success/failure*.

        use tokio::test;

        use super::*;
        use crate::state::test_utils::{test_help_success, test_unavailable_command};

        #[test]
        pub async fn main_menu_help_success() {
            let main_menu = State::Authorized(AuthorizedBox::MainMenu(Authorized {
                _kind: kind::MainMenu,
            }));

            test_help_success(main_menu).await
        }

        #[test]
        pub async fn main_menu_start_failure() {
            let main_menu = State::Authorized(AuthorizedBox::MainMenu(Authorized {
                _kind: kind::MainMenu,
            }));
            let start = Command::Start(crate::command::Start);

            test_unavailable_command(main_menu, start).await
        }

        #[test]
        pub async fn main_menu_cancel_failure() {
            let main_menu = State::Authorized(AuthorizedBox::MainMenu(Authorized {
                _kind: kind::MainMenu,
            }));
            let cancel = Command::Cancel(crate::command::Cancel);

            test_unavailable_command(main_menu, cancel).await
        }
    }

    mod message {
        use tokio::test;

        use super::*;
        use crate::state::test_utils::test_unexpected_message;

        #[test]
        pub async fn main_menu_sign_in_failure() {
            let main_menu = State::Authorized(AuthorizedBox::MainMenu(Authorized {
                _kind: kind::MainMenu,
            }));
            let sign_in = Message::SignIn(crate::message::SignIn);

            test_unexpected_message(main_menu, sign_in).await
        }

        #[test]
        pub async fn main_menu_arbitrary_failure() {
            let main_menu = State::Authorized(AuthorizedBox::MainMenu(Authorized {
                _kind: kind::MainMenu,
            }));
            let arbitrary = Message::Arbitrary(crate::message::Arbitrary(
                "Test arbitrary message".to_owned(),
            ));

            test_unexpected_message(main_menu, arbitrary).await
        }
    }
}
