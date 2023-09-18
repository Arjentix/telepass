//! Module with mock structures to test [`Bot`](teloxide::Bot) usage.

#![allow(clippy::unimplemented)]

/// Trait to extend [`teloxide::types::Me`] with `user()` method.
pub trait UserExt {
    /// Get user info.
    fn user(&self) -> &teloxide::types::User;
}

impl UserExt for teloxide::types::Me {
    fn user(&self) -> &teloxide::types::User {
        &self.user
    }
}

/// Trait to extend [`teloxide::types::Message`] with `id()` method.
pub trait IdExt {
    /// Get message id.
    fn id(&self) -> teloxide::types::MessageId;
}

impl IdExt for teloxide::types::Message {
    fn id(&self) -> teloxide::types::MessageId {
        self.id
    }
}

#[cfg(test)]
pub use inner::*;

#[cfg(test)]
#[allow(clippy::module_name_repetitions)]
#[allow(clippy::indexing_slicing)] // From `automock` macro expansion
mod inner {
    use std::future::{ready, Future, IntoFuture, Ready};

    pub use builder::*;
    use mockall::mock;

    /// Constant for test chat id.
    pub const CHAT_ID: teloxide::types::ChatId = teloxide::types::ChatId(0);

    mock! {
        pub Bot {
            pub fn send_message<C, T>(&self, chat_id: C, message: T) -> MockSendMessage
            where
                C: Into<teloxide::types::Recipient> + 'static,
                T: Into<String> + 'static;

            pub fn get_me(&self) -> MockGetMe;

            pub fn delete_message<C>(
                &self,
                chat_id: C,
                message_id: teloxide::types::MessageId
            ) -> MockDeleteMessage
            where
                C: Into<teloxide::types::Recipient> + 'static;
        }
    }

    mock! {
        pub SendMessage {
            pub fn reply_markup<T>(self, value: T) -> Self
            where
                T: Into<teloxide::types::ReplyMarkup> + 'static;

            pub fn parse_mode(self, value: teloxide::types::ParseMode) -> Self;
        }

        impl IntoFuture for SendMessage {
            type Output = <<MockSendMessage as IntoFuture>::IntoFuture as Future>::Output;

           type IntoFuture = MockMessageFuture;

            fn into_future(self) -> <MockSendMessage as IntoFuture>::IntoFuture;
        }
    }

    pub struct MockGetMe(pub MockMe);

    impl MockGetMe {
        #[must_use]
        pub const fn new(me: MockMe) -> Self {
            Self(me)
        }
    }

    impl IntoFuture for MockGetMe {
        type Output = <Self::IntoFuture as Future>::Output;

        type IntoFuture = MockMeFuture;

        fn into_future(self) -> Self::IntoFuture {
            ready(Ok(self.0))
        }
    }

    pub type MockMeFuture = Ready<Result<MockMe, MockError>>;

    mock! {
        pub Me {
            pub fn username(&self) -> &str;
        }

        impl super::UserExt for Me {
            fn user(&self) -> &teloxide::types::User;
        }
    }

    pub type MockError = std::convert::Infallible;

    pub type MockMessageFuture = Ready<Result<MockMessage, MockError>>;

    mock! {
        #[derive(Debug)]
        pub Message {
            pub fn text<'slf>(&'slf self) -> Option<&'slf str>;
        }

        impl super::IdExt for Message {
            fn id(&self) -> teloxide::types::MessageId;
        }
    }

    impl Clone for MockMessage {
        fn clone(&self) -> Self {
            Self::default()
        }
    }

    #[derive(Default)]
    pub struct MockDeleteMessage;

    impl IntoFuture for MockDeleteMessage {
        type Output = <Self::IntoFuture as Future>::Output;

        type IntoFuture = Ready<Result<(), std::convert::Infallible>>;

        fn into_future(self) -> Self::IntoFuture {
            ready(Ok(()))
        }
    }

    mod builder {
        use mockall::predicate::eq;

        use super::*;

        #[derive(Default)]
        pub struct MockBotBuilder {
            mock_bot: MockBot,
        }

        impl MockBotBuilder {
            #[must_use]
            pub fn new() -> Self {
                Self::default()
            }

            #[must_use]
            pub fn expect_get_me(mut self) -> Self {
                self.mock_bot.expect_get_me().return_once(|| {
                    let mut mock_me = MockMe::default();
                    mock_me.expect_user().return_const(teloxide::types::User {
                        id: teloxide::types::UserId(0),
                        is_bot: false,
                        first_name: String::from("Test"),
                        last_name: None,
                        username: None,
                        language_code: None,
                        is_premium: false,
                        added_to_attachment_menu: false,
                    });
                    MockGetMe::new(mock_me)
                });
                self
            }

            #[must_use]
            pub const fn expect_send_message<T>(
                self,
                message: T,
            ) -> MockSendMessageBuilder<T, NoReplyMarkup>
            where
                T: Into<String> + std::fmt::Debug + PartialEq + Send + Sync + 'static,
            {
                MockSendMessageBuilder::new(self, message)
            }

            #[must_use]
            pub fn expect_delete_message(mut self, message_id: teloxide::types::MessageId) -> Self {
                self.mock_bot
                    .expect_delete_message()
                    .with(eq(CHAT_ID), eq(message_id));
                self
            }

            #[must_use]
            #[allow(clippy::missing_const_for_fn)] // False positive
            pub fn build(self) -> MockBot {
                self.mock_bot
            }
        }

        pub struct MockSendMessageBuilder<T, M = NoReplyMarkup>
        where
            T: Into<String> + PartialEq + std::fmt::Debug + Send + Sync + 'static,
            M: Into<teloxide::types::ReplyMarkup>
                + PartialEq
                + std::fmt::Debug
                + Send
                + Sync
                + 'static,
        {
            mock_bot_builder: MockBotBuilder,
            message: T,
            expectations: Vec<MockSendMessageExpectation<M>>,
        }

        impl<
                T: Into<String> + PartialEq + std::fmt::Debug + Send + Sync + 'static,
                M: Into<teloxide::types::ReplyMarkup>
                    + PartialEq
                    + std::fmt::Debug
                    + Send
                    + Sync
                    + 'static,
            > MockSendMessageBuilder<T, M>
        {
            #[must_use]
            pub const fn new(mock_bot_builder: MockBotBuilder, message: T) -> Self {
                Self {
                    mock_bot_builder,
                    message,
                    expectations: Vec::new(),
                }
            }

            #[must_use]
            fn add_expectation(mut self, expectation: MockSendMessageExpectation<M>) -> Self {
                self.expectations.push(expectation);
                self
            }

            #[must_use]
            pub fn expect_parse_mode(self, parse_mode: teloxide::types::ParseMode) -> Self {
                self.add_expectation(MockSendMessageExpectation::ParseMode(parse_mode))
            }

            #[must_use]
            pub fn expect_into_future(self) -> MockBotBuilder {
                let mut mock_send_message_into_future = MockSendMessage::default();
                mock_send_message_into_future
                    .expect_into_future()
                    .return_const(ready(Ok(MockMessage::default())));

                self.build(mock_send_message_into_future)
            }

            #[must_use]
            pub fn expect_into_future_with_id(
                self,
                message_id: teloxide::types::MessageId,
            ) -> MockBotBuilder {
                let mut mock_send_message_into_future = MockSendMessage::default();
                mock_send_message_into_future
                    .expect_into_future()
                    .return_once(move || {
                        let mut mock_message = MockMessage::default();
                        mock_message.expect_id().return_const(message_id);
                        ready(Ok(mock_message))
                    });

                self.build(mock_send_message_into_future)
            }

            fn build(mut self, inner_mock_send_message: MockSendMessage) -> MockBotBuilder {
                let mock_send_message = self.expectations.into_iter().rev().fold(
                    inner_mock_send_message,
                    |mock_send_message, expectation| {
                        let mut new_mock_send_message = MockSendMessage::default();
                        match expectation {
                            MockSendMessageExpectation::ParseMode(parse_mode) => {
                                new_mock_send_message
                                    .expect_parse_mode()
                                    .with(eq(parse_mode))
                                    .return_once(move |_parse_mode| mock_send_message);
                            }
                            MockSendMessageExpectation::ReplyMarkup(reply_markup) => {
                                new_mock_send_message
                                    .expect_reply_markup::<M>()
                                    .with(eq(reply_markup))
                                    .return_once(move |_reply_markup| mock_send_message);
                            }
                        }
                        new_mock_send_message
                    },
                );

                self.mock_bot_builder
                    .mock_bot
                    .expect_send_message()
                    .with(eq(CHAT_ID), eq(self.message))
                    .return_once(|_chat_id, _message| mock_send_message);
                self.mock_bot_builder
            }
        }

        impl<T: Into<String> + PartialEq + std::fmt::Debug + Send + Sync + 'static>
            MockSendMessageBuilder<T, NoReplyMarkup>
        {
            #[must_use]
            pub fn expect_reply_markup<M>(self, reply_markup: M) -> MockSendMessageBuilder<T, M>
            where
                M: Into<teloxide::types::ReplyMarkup>
                    + PartialEq
                    + std::fmt::Debug
                    + Send
                    + Sync
                    + 'static,
            {
                let builder = MockSendMessageBuilder {
                    mock_bot_builder: self.mock_bot_builder,
                    message: self.message,
                    expectations: self
                        .expectations
                        .into_iter()
                        .map(MockSendMessageExpectation::transform)
                        .collect(),
                };
                builder.add_expectation(MockSendMessageExpectation::ReplyMarkup(reply_markup))
            }
        }

        #[derive(Debug)]
        pub enum MockSendMessageExpectation<M>
        where
            M: Into<teloxide::types::ReplyMarkup> + PartialEq + std::fmt::Debug + Send + Sync,
        {
            ParseMode(teloxide::types::ParseMode),
            ReplyMarkup(M),
        }

        impl<M> MockSendMessageExpectation<M>
        where
            M: Into<teloxide::types::ReplyMarkup> + PartialEq + std::fmt::Debug + Send + Sync,
        {
            #[must_use]
            fn transform<M2>(self) -> MockSendMessageExpectation<M2>
            where
                M2: Into<teloxide::types::ReplyMarkup> + PartialEq + std::fmt::Debug + Send + Sync,
            {
                match self {
                    Self::ParseMode(parse_mode) => {
                        MockSendMessageExpectation::ParseMode(parse_mode)
                    }
                    Self::ReplyMarkup(_reply_markup) => {
                        unreachable!(
                            "Transforming one reply markup to another is not \
                             accessible thanks to `MockSendMessageBuilder`"
                        )
                    }
                }
            }
        }

        #[derive(Debug, PartialEq, Eq)]
        pub enum NoReplyMarkup {}

        impl From<NoReplyMarkup> for teloxide::types::ReplyMarkup {
            fn from(_: NoReplyMarkup) -> Self {
                unreachable!("`NoReplyMarkup` cannot be instantiated to a value")
            }
        }

        mod tests {
            #![allow(clippy::unwrap_used)]

            use tokio::test;

            use super::*;
            use crate::mock_bot::{IdExt as _, UserExt};

            #[test]
            async fn message_success() {
                let mock_bot = MockBotBuilder::new()
                    .expect_send_message("Test Message")
                    .expect_into_future()
                    .build();

                mock_bot
                    .send_message(CHAT_ID, "Test Message")
                    .await
                    .unwrap();
            }

            #[test]
            async fn several_messages_success() {
                let mock_bot = MockBotBuilder::new()
                    .expect_send_message("Test Message 1")
                    .expect_into_future()
                    .expect_send_message("Test Message 2")
                    .expect_into_future()
                    .build();

                mock_bot
                    .send_message(CHAT_ID, "Test Message 1")
                    .await
                    .unwrap();

                mock_bot
                    .send_message(CHAT_ID, "Test Message 2")
                    .await
                    .unwrap();
            }

            #[test]
            #[should_panic]
            async fn wrong_message_failure() {
                let mock_bot = MockBotBuilder::new()
                    .expect_send_message("Test Message")
                    .expect_into_future()
                    .build();

                mock_bot
                    .send_message(CHAT_ID, "Wrong Test Message")
                    .await
                    .unwrap();
            }

            #[test]
            async fn parse_mode_success() {
                let mock_bot = MockBotBuilder::new()
                    .expect_send_message("Test Message")
                    .expect_parse_mode(teloxide::types::ParseMode::MarkdownV2)
                    .expect_into_future()
                    .build();

                mock_bot
                    .send_message(CHAT_ID, "Test Message")
                    .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                    .await
                    .unwrap();
            }

            #[test]
            #[should_panic]
            async fn wrong_parse_mode_failure() {
                let mock_bot = MockBotBuilder::new()
                    .expect_send_message("Test Message")
                    .expect_parse_mode(teloxide::types::ParseMode::MarkdownV2)
                    .expect_into_future()
                    .build();

                mock_bot
                    .send_message(CHAT_ID, "Test Message")
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .await
                    .unwrap();
            }

            #[test]
            async fn reply_markup_success() {
                use teloxide::types::{KeyboardButton, KeyboardMarkup};

                let keyboard_markup = KeyboardMarkup::new([[KeyboardButton::new("Test Button")]]);

                let mock_bot = MockBotBuilder::new()
                    .expect_send_message("Test Message")
                    .expect_reply_markup::<KeyboardMarkup>(keyboard_markup.clone())
                    .expect_into_future()
                    .build();

                mock_bot
                    .send_message(CHAT_ID, "Test Message")
                    .reply_markup(keyboard_markup)
                    .await
                    .unwrap();
            }

            #[test]
            #[should_panic]
            async fn wrong_reply_markup_failure() {
                use teloxide::types::{KeyboardButton, KeyboardMarkup};

                let mock_bot = MockBotBuilder::new()
                    .expect_send_message("Test Message")
                    .expect_reply_markup::<KeyboardMarkup>(KeyboardMarkup::new([[
                        KeyboardButton::new("Expected Button"),
                    ]]))
                    .expect_into_future()
                    .build();

                mock_bot
                    .send_message(CHAT_ID, "Test Message")
                    .reply_markup(KeyboardMarkup::new([[KeyboardButton::new(
                        "Unexpected Button",
                    )]]))
                    .await
                    .unwrap();
            }

            #[test]
            async fn message_id_success() {
                let expected_message_id = teloxide::types::MessageId(25);

                let mock_bot = MockBotBuilder::new()
                    .expect_send_message("Test Message")
                    .expect_into_future_with_id(expected_message_id)
                    .build();

                let message_id = mock_bot
                    .send_message(CHAT_ID, "Test Message")
                    .await
                    .unwrap()
                    .id();
                assert_eq!(message_id, expected_message_id);
            }

            #[test]
            async fn delete_message_success() {
                let expected_message_id = teloxide::types::MessageId(72);

                let mock_bot = MockBotBuilder::new()
                    .expect_delete_message(expected_message_id)
                    .build();

                mock_bot
                    .delete_message(CHAT_ID, expected_message_id)
                    .await
                    .unwrap();
            }

            #[test]
            #[should_panic]
            async fn wrong_delete_message_failure() {
                let mock_bot = MockBotBuilder::new()
                    .expect_delete_message(teloxide::types::MessageId(72))
                    .build();

                mock_bot
                    .delete_message(CHAT_ID, teloxide::types::MessageId(107))
                    .await
                    .unwrap();
            }

            #[test]
            async fn get_me_success() {
                let mock_bot = MockBotBuilder::new().expect_get_me().build();

                let me = mock_bot.get_me().await.unwrap();
                assert_eq!(me.user().first_name, "Test");
            }
        }
    }
}
