//! Module with trait to replace direct [`Bot`](teloxide::Bot) usage.
//! Used for unit-tests.

#![allow(clippy::module_name_repetitions)]
#![cfg_attr(test, allow(clippy::indexing_slicing))] // From `automock` macro expansion

use std::future::{Future, IntoFuture};

#[cfg(test)]
pub use helper_types::*;
#[cfg(test)]
use mockall::automock;

/// Trait to reflect all used functions of [`Bot`].
/// If a new functionallity from [`Bot`] is needed, then it should be added in this trait.
///
/// Used for mocking in unit-tests.
///
/// [`Bot`]: teloxide::Bot
#[cfg_attr(test, automock(
    type Error = MockError;

    type Message = MockMessage;
    type MessageFuture = MockMessageFuture;
    type SendMessage = MockSendMessage;

    type Me = MockMe;
    type MeFuture = MockMeFuture;
    type GetMe = MockGetMe;
))]
pub trait BotTrait {
    type Error: std::error::Error + Send + Sync;

    type Message;
    type MessageFuture: Future<Output = Result<Self::Message, Self::Error>> + Send;
    type SendMessage: IntoFuture<IntoFuture = Self::MessageFuture> + MessageSetters + Send;

    fn send_message<C, T>(&self, chat_id: C, message: T) -> Self::SendMessage
    where
        C: Into<teloxide::types::Recipient> + 'static,
        T: Into<String> + 'static;

    type Me: MeGetters + Send;
    type MeFuture: Future<Output = Result<Self::Me, Self::Error>> + Send;
    type GetMe: IntoFuture<IntoFuture = Self::MeFuture> + Send;

    fn get_me(&self) -> Self::GetMe;
}

#[cfg(test)]
pub type MockBot = MockBotTrait;

pub trait MessageSetters {
    fn reply_markup<T>(self, value: T) -> Self
    where
        T: Into<teloxide::types::ReplyMarkup> + 'static;
}

pub trait MeGetters {
    fn user(&self) -> &teloxide::types::User;
}

impl BotTrait for teloxide::Bot {
    type Error = teloxide::errors::RequestError;

    type Message = teloxide::types::Message;
    type MessageFuture = <Self::SendMessage as IntoFuture>::IntoFuture;
    type SendMessage = <Self as teloxide::requests::Requester>::SendMessage;

    fn send_message<C, T>(&self, chat_id: C, message: T) -> Self::SendMessage
    where
        C: Into<teloxide::types::Recipient> + 'static,
        T: Into<String> + 'static,
    {
        <Self as teloxide::requests::Requester>::send_message(self, chat_id, message)
    }

    type Me = teloxide::types::Me;
    type MeFuture = <Self::GetMe as IntoFuture>::IntoFuture;
    type GetMe = <Self as teloxide::requests::Requester>::GetMe;

    fn get_me(&self) -> Self::GetMe {
        <Self as teloxide::requests::Requester>::get_me(self)
    }
}

impl MessageSetters for teloxide::requests::JsonRequest<teloxide::payloads::SendMessage> {
    fn reply_markup<T>(self, value: T) -> Self
    where
        T: Into<teloxide::types::ReplyMarkup> + 'static,
    {
        <Self as teloxide::payloads::SendMessageSetters>::reply_markup(self, value)
    }
}

impl MeGetters for teloxide::types::Me {
    fn user(&self) -> &teloxide::types::User {
        &self.user
    }
}

#[cfg(test)]
mod helper_types {
    //! Module with helper mock types for [`MockBot`](super::MockBot).

    use std::future::{ready, Future, IntoFuture, Ready};

    use mockall::mock;

    use super::{MeGetters, MessageSetters};

    pub type MockError = std::convert::Infallible;

    pub type MockMessage = ();
    pub type MockMessageFuture = Ready<Result<MockMessage, MockError>>;

    pub type MockMeFuture = Ready<Result<MockMe, MockError>>;

    // Using `mock!` only for trait which is usefull to check in tests
    mock! {
        pub SendMessage {}

        impl MessageSetters for SendMessage {
            fn reply_markup<T>(self, value: T) -> Self
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

    // Using `mock!` only for trait which is usefull to check in tests
    mock! {
        pub Me {}

        impl MeGetters for Me {
            fn user(&self) -> &teloxide::types::User;
        }
    }

    #[derive(Default)]
    pub struct MockGetMe;

    impl IntoFuture for MockGetMe {
        type Output = <Self::IntoFuture as Future>::Output;

        type IntoFuture = MockMeFuture;

        fn into_future(self) -> Self::IntoFuture {
            ready(Ok(MockMe::default()))
        }
    }
}
