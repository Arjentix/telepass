//! [`Default`] state implementation.

/// State when bot is waiting for user to start the bot.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Default;

#[cfg(test)]
pub mod tests {
    pub mod command {
        use tokio::test;

        use crate::{
            command::Command,
            state::State,
            test_utils::{test_help_success, test_unavailable_command},
        };

        #[test]
        pub async fn help_success() {
            let default = State::default();

            test_help_success(default).await
        }

        #[test]
        pub async fn cancel_failure() {
            let default = State::default();
            let cancel = Command::cancel();

            test_unavailable_command(default, cancel).await
        }
    }

    pub mod message {
        use tokio::test;

        use crate::{message::MessageBox, state::State, test_utils::test_unexpected_message};

        #[test]
        pub async fn web_app_failure() {
            let default = State::default();
            let web_app = MessageBox::web_app("data".to_owned(), "button_text".to_owned());

            test_unexpected_message(default, web_app).await
        }

        #[test]
        pub async fn add_failure() {
            let default = State::default();
            let add = MessageBox::add();

            test_unexpected_message(default, add).await
        }

        #[test]
        pub async fn list_failure() {
            let default = State::default();
            let list = MessageBox::list();

            test_unexpected_message(default, list).await
        }

        #[test]
        pub async fn arbitrary_failure() {
            let default = State::default();
            let arbitrary = MessageBox::arbitrary("Test arbitrary message");

            test_unexpected_message(default, arbitrary).await
        }
    }

    pub mod button {
        use tokio::test;

        use crate::{button::ButtonBox, state::State, test_utils::test_unexpected_button};

        #[test]
        pub async fn delete_failure() {
            let default = State::default();
            let delete_button = ButtonBox::delete();

            test_unexpected_button(default, delete_button).await;
        }

        #[test]
        pub async fn yes_failure() {
            let default = State::default();
            let yes_button = ButtonBox::yes();

            test_unexpected_button(default, yes_button).await;
        }

        #[test]
        pub async fn no_failure() {
            let default = State::default();
            let no_button = ButtonBox::no();

            test_unexpected_button(default, no_button).await;
        }

        #[test]
        pub async fn show_failure() {
            let default = State::default();
            let show_button = ButtonBox::show();

            test_unexpected_button(default, show_button).await;
        }
    }
}
