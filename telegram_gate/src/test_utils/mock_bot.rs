//! Module with mock structures to test [`Bot`](teloxide::Bot) usage.

#![allow(clippy::unimplemented, reason = "it's ok in tests")]
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

        pub fn edit_message_text<C, T>(
            &self,
            chat_id: C,
            message_id: teloxide::types::MessageId,
            text: T,
        ) -> MockEditMessageText
        where
            C: Into<teloxide::types::Recipient> + 'static,
            T: Into<String> + 'static;

        pub fn edit_message_reply_markup<C>(
            &self,
            chat_id: C,
            message_id: teloxide::types::MessageId,
        ) -> MockEditMessageReplyMarkup
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

    impl crate::UserExt for Me {
        fn user(&self) -> &teloxide::types::User;
    }
}

pub type MockError = std::convert::Infallible;

pub type MockMessageFuture = Ready<Result<MockMessage, MockError>>;

mock! {
    #[derive(Debug)]
    pub Message {}

    impl crate::TelegramMessageGettersExt for Message {
        fn id(&self) -> teloxide::types::MessageId;

        fn take_kind(self) -> teloxide::types::MessageKind;
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

mock! {
    pub EditMessageText {
        pub fn parse_mode(self, value: teloxide::types::ParseMode) -> Self;
    }

    impl IntoFuture for EditMessageText {
        type Output = <<MockEditMessageText as IntoFuture>::IntoFuture as Future>::Output;

        type IntoFuture = Ready<Result<MockMessage, std::convert::Infallible>>;

        fn into_future(self) -> <MockEditMessageText as IntoFuture>::IntoFuture;
    }
}

mock! {
    pub EditMessageReplyMarkup {
        pub fn reply_markup<T>(self, value: T) -> Self
        where
            T: Into<teloxide::types::ReplyMarkup> + 'static;
    }

    impl IntoFuture for EditMessageReplyMarkup {
        type Output = <<MockEditMessageReplyMarkup as IntoFuture>::IntoFuture as Future>::Output;

        type IntoFuture = Ready<Result<MockMessage, std::convert::Infallible>>;

        fn into_future(self) -> <MockEditMessageReplyMarkup as IntoFuture>::IntoFuture;
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
        pub const fn expect_edit_message_text<T>(
            self,
            message_id: teloxide::types::MessageId,
            message: T,
        ) -> MockEditMessageTextBuilder<T>
        where
            T: Into<String> + std::fmt::Debug + PartialEq + Send + Sync + 'static,
        {
            MockEditMessageTextBuilder::new(self, message_id, message)
        }

        #[must_use]
        pub const fn expect_edit_message_reply_markup(
            self,
            message_id: teloxide::types::MessageId,
        ) -> MockEditMessageReplyMarkupBuilder<NoReplyMarkup> {
            MockEditMessageReplyMarkupBuilder::new(self, message_id)
        }

        #[must_use]
        pub fn expect_delete_message(mut self, message_id: teloxide::types::MessageId) -> Self {
            self.mock_bot
                .expect_delete_message()
                .with(eq(CHAT_ID), eq(message_id));
            self
        }

        #[must_use]
        pub fn build(self) -> MockBot {
            self.mock_bot
        }
    }

    pub struct MockSendMessageBuilder<T, M = NoReplyMarkup>
    where
        T: Into<String> + PartialEq + std::fmt::Debug + Send + Sync + 'static,
        M: Into<teloxide::types::ReplyMarkup> + PartialEq + std::fmt::Debug + Send + Sync + 'static,
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
                Self::ParseMode(parse_mode) => MockSendMessageExpectation::ParseMode(parse_mode),
                Self::ReplyMarkup(_reply_markup) => {
                    unreachable!(
                        "Transforming one reply markup to another is not \
                             accessible thanks to `MockSendMessageBuilder`"
                    )
                }
            }
        }
    }

    pub struct MockEditMessageTextBuilder<T>
    where
        T: Into<String> + PartialEq + std::fmt::Debug + Send + Sync + 'static,
    {
        mock_bot_builder: MockBotBuilder,
        message_id: teloxide::types::MessageId,
        message: T,
        expectations: Vec<MockEditMessageTextExpectation>,
    }

    impl<T: Into<String> + PartialEq + std::fmt::Debug + Send + Sync + 'static>
        MockEditMessageTextBuilder<T>
    {
        const fn new(
            mock_bot_builder: MockBotBuilder,
            message_id: teloxide::types::MessageId,
            message: T,
        ) -> Self {
            Self {
                mock_bot_builder,
                message_id,
                message,
                expectations: Vec::new(),
            }
        }

        #[must_use]
        pub fn expect_parse_mode(mut self, parse_mode: teloxide::types::ParseMode) -> Self {
            self.expectations
                .push(MockEditMessageTextExpectation::ParseMode(parse_mode));
            self
        }

        #[must_use]
        pub fn expect_into_future(self) -> MockBotBuilder {
            let mut mock_edit_message_text_into_future = MockEditMessageText::default();
            mock_edit_message_text_into_future
                .expect_into_future()
                .return_const(ready(Ok(MockMessage::default())));

            self.build(mock_edit_message_text_into_future)
        }

        fn build(mut self, inner_mock_edit_message_text: MockEditMessageText) -> MockBotBuilder {
            let mock_edit_message_text = self.expectations.into_iter().rev().fold(
                inner_mock_edit_message_text,
                |mock_edit_message_text, expectation| {
                    let mut new_mock_edit_message_text = MockEditMessageText::default();
                    match expectation {
                        MockEditMessageTextExpectation::ParseMode(parse_mode) => {
                            new_mock_edit_message_text
                                .expect_parse_mode()
                                .with(eq(parse_mode))
                                .return_once(move |_parse_mode| mock_edit_message_text);
                        }
                    }
                    new_mock_edit_message_text
                },
            );

            self.mock_bot_builder
                .mock_bot
                .expect_edit_message_text()
                .with(eq(CHAT_ID), eq(self.message_id), eq(self.message))
                .return_once(|_chat_id, _message_id, _message| mock_edit_message_text);
            self.mock_bot_builder
        }
    }

    #[derive(Debug)]
    pub enum MockEditMessageTextExpectation {
        ParseMode(teloxide::types::ParseMode),
    }

    pub struct MockEditMessageReplyMarkupBuilder<M>
    where
        M: Into<teloxide::types::ReplyMarkup> + PartialEq + std::fmt::Debug + Send + Sync,
    {
        mock_bot_builder: MockBotBuilder,
        message_id: teloxide::types::MessageId,
        reply_markup: Option<M>,
    }

    impl MockEditMessageReplyMarkupBuilder<NoReplyMarkup> {
        const fn new(
            mock_bot_builder: MockBotBuilder,
            message_id: teloxide::types::MessageId,
        ) -> Self {
            Self {
                mock_bot_builder,
                message_id,
                reply_markup: None,
            }
        }

        #[must_use]
        pub fn expect_reply_markup<M>(self, reply_markup: M) -> MockEditMessageReplyMarkupBuilder<M>
        where
            M: Into<teloxide::types::ReplyMarkup> + PartialEq + std::fmt::Debug + Send + Sync,
        {
            MockEditMessageReplyMarkupBuilder {
                mock_bot_builder: self.mock_bot_builder,
                message_id: self.message_id,
                reply_markup: Some(reply_markup),
            }
        }
    }

    impl<
            M: Into<teloxide::types::ReplyMarkup>
                + PartialEq
                + std::fmt::Debug
                + Send
                + Sync
                + 'static,
        > MockEditMessageReplyMarkupBuilder<M>
    {
        #[must_use]
        pub fn expect_into_future(mut self) -> MockBotBuilder {
            let mut mock_edit_message_reply_markup_into_future =
                MockEditMessageReplyMarkup::default();
            mock_edit_message_reply_markup_into_future
                .expect_into_future()
                .return_const(ready(Ok(MockMessage::default())));

            let final_mock = if let Some(reply_markup) = self.reply_markup {
                let mut mock_edit_message_reply_markup = MockEditMessageReplyMarkup::default();
                mock_edit_message_reply_markup
                    .expect_reply_markup()
                    .with(eq(reply_markup))
                    .return_once(|_reply_markup| mock_edit_message_reply_markup_into_future);
                mock_edit_message_reply_markup
            } else {
                mock_edit_message_reply_markup_into_future
            };

            self.mock_bot_builder
                .mock_bot
                .expect_edit_message_reply_markup()
                .with(eq(CHAT_ID), eq(self.message_id))
                .return_once(|_chat_id, _message_id| final_mock);
            self.mock_bot_builder
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
        #![allow(clippy::unwrap_used, reason = "it's ok in tests")]

        use tokio::test;

        use super::*;
        use crate::{TelegramMessageGettersExt as _, UserExt as _};

        mod send_message {
            use tokio::test;

            use super::*;

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
            #[should_panic(expected = "MockBot::send_message(?, ?): \
                                No matching expectation found")]
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
            #[should_panic(
                expected = "MockSendMessage::parse_mode(Html): No matching expectation found"
            )]
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
            #[should_panic(expected = "MockSendMessage::reply_markup(?)")]
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
        }

        mod delete_message {
            use tokio::test;

            use super::*;

            #[test]
            async fn message_success() {
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
            #[should_panic(expected = "MockBot::delete_message(?, MessageId(107)): \
                                           No matching expectation found")]
            async fn wrong_message_failure() {
                let mock_bot = MockBotBuilder::new()
                    .expect_delete_message(teloxide::types::MessageId(72))
                    .build();

                mock_bot
                    .delete_message(CHAT_ID, teloxide::types::MessageId(107))
                    .await
                    .unwrap();
            }
        }

        mod edit_message_text {
            use tokio::test;

            use super::*;

            #[test]
            async fn edit_message_text_success() {
                let expected_message_id = teloxide::types::MessageId(72);
                let expected_message_text = "Test Message";

                let mock_bot = MockBotBuilder::new()
                    .expect_edit_message_text(expected_message_id, expected_message_text)
                    .expect_into_future()
                    .build();

                mock_bot
                    .edit_message_text(CHAT_ID, expected_message_id, expected_message_text)
                    .await
                    .unwrap();
            }

            #[test]
            #[should_panic(expected = "MockBot::edit_message_text(?, MessageId(107), ?): \
                                No matching expectation found")]
            async fn wrong_message_id_failure() {
                let expected_message_text = "Test Message";

                let mock_bot = MockBotBuilder::new()
                    .expect_edit_message_text(teloxide::types::MessageId(72), expected_message_text)
                    .expect_into_future()
                    .build();

                mock_bot
                    .edit_message_text(
                        CHAT_ID,
                        teloxide::types::MessageId(107),
                        expected_message_text,
                    )
                    .await
                    .unwrap();
            }

            #[test]
            #[should_panic(expected = "MockBot::edit_message_text(?, MessageId(72), ?): \
                                No matching expectation found")]
            async fn wrong_message_text_failure() {
                let expected_message_id = teloxide::types::MessageId(72);

                let mock_bot = MockBotBuilder::new()
                    .expect_edit_message_text(expected_message_id, "Test Message")
                    .expect_into_future()
                    .build();

                mock_bot
                    .edit_message_text(CHAT_ID, expected_message_id, "Wrong Test Message")
                    .await
                    .unwrap();
            }

            #[test]
            async fn parse_mode_success() {
                let expected_message_id = teloxide::types::MessageId(72);
                let expected_message_text = "Test Message";
                let expected_parse_mode = teloxide::types::ParseMode::MarkdownV2;

                let mock_bot = MockBotBuilder::new()
                    .expect_edit_message_text(expected_message_id, expected_message_text)
                    .expect_parse_mode(expected_parse_mode)
                    .expect_into_future()
                    .build();

                mock_bot
                    .edit_message_text(CHAT_ID, expected_message_id, expected_message_text)
                    .parse_mode(expected_parse_mode)
                    .await
                    .unwrap();
            }

            #[test]
            #[should_panic(
                expected = "MockEditMessageText::parse_mode(Html): No matching expectation found"
            )]
            async fn wrong_parse_mode_failure() {
                let expected_message_id = teloxide::types::MessageId(72);
                let expected_message_text = "Test Message";

                let mock_bot = MockBotBuilder::new()
                    .expect_edit_message_text(expected_message_id, expected_message_text)
                    .expect_parse_mode(teloxide::types::ParseMode::MarkdownV2)
                    .expect_into_future()
                    .build();

                mock_bot
                    .edit_message_text(CHAT_ID, expected_message_id, expected_message_text)
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .await
                    .unwrap();
            }
        }

        mod edit_message_reply_markup {
            use tokio::test;

            use super::*;

            #[test]
            async fn edit_message_reply_markup_success() {
                let expected_message_id = teloxide::types::MessageId(23);
                let expected_reply_markup = teloxide::types::InlineKeyboardMarkup::default();

                let mock_bot = MockBotBuilder::new()
                    .expect_edit_message_reply_markup(expected_message_id)
                    .expect_reply_markup(expected_reply_markup.clone())
                    .expect_into_future()
                    .build();

                mock_bot
                    .edit_message_reply_markup(CHAT_ID, expected_message_id)
                    .reply_markup(expected_reply_markup)
                    .await
                    .unwrap();
            }

            #[test]
            #[should_panic(expected = "MockEditMessageReplyMarkup::reply_markup(?): \
                    No matching expectation found")]
            async fn wrong_reply_markup_failure() {
                let expected_message_id = teloxide::types::MessageId(23);

                let mock_bot = MockBotBuilder::new()
                    .expect_edit_message_reply_markup(expected_message_id)
                    .expect_reply_markup(teloxide::types::InlineKeyboardMarkup::default())
                    .expect_into_future()
                    .build();

                mock_bot
                    .edit_message_reply_markup(CHAT_ID, expected_message_id)
                    .reply_markup(teloxide::types::InlineKeyboardMarkup::new([[
                        teloxide::types::InlineKeyboardButton::callback("Button", "Button"),
                    ]]))
                    .await
                    .unwrap();
            }
        }

        #[test]
        async fn get_me_success() {
            let mock_bot = MockBotBuilder::new().expect_get_me().build();

            let me = mock_bot.get_me().await.unwrap();
            assert_eq!(me.user().first_name, "Test");
        }
    }
}
