//! Module with mock structures to test [`Bot`](teloxide::Bot) usage.

#![cfg(test)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::indexing_slicing)] // From `automock` macro expansion

use std::future::{ready, Future, IntoFuture, Ready};

use mockall::mock;

mock! {
    pub Bot {
        pub fn send_message<C, T>(&self, chat_id: C, message: T) -> MockSendMessage
        where
            C: Into<teloxide::types::Recipient> + 'static,
            T: Into<String> + 'static;

        pub fn get_me(&self) -> MockGetMe;
    }
}

// Methods used in non-testable places
impl MockBot {
    pub fn answer_callback_query<C>(
        &self,
        _callback_query_id: C,
    ) -> teloxide::requests::JsonRequest<teloxide::payloads::AnswerCallbackQuery>
    where
        C: Into<String>,
    {
        unreachable!()
    }
}

mock! {
    pub SendMessage {
        pub fn reply_markup<T>(self, value: T) -> Self
        where
            T: Into<teloxide::types::ReplyMarkup> + 'static;
    }
}

impl IntoFuture for MockSendMessage {
    type Output = <MockMessageFuture as Future>::Output;

    type IntoFuture = MockMessageFuture;

    fn into_future(self) -> Self::IntoFuture {
        ready(Ok(()))
    }
}

pub struct MockGetMe(pub MockMe);

impl MockGetMe {
    pub fn new(me: MockMe) -> Self {
        MockGetMe(me)
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
        pub fn user(&self) -> &teloxide::types::User;

        pub fn username(&self) -> &str;
    }
}

pub type MockError = std::convert::Infallible;

pub type MockMessageFuture = Ready<Result<MockMessage, MockError>>;
pub type MockMessage = ();
