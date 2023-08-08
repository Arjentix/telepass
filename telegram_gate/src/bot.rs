//! Module with trait to replace direct [`Bot`](teloxide::Bot) usage.
//! Used for unit-tests.

use std::future::{Future, IntoFuture};

#[cfg(test)]
use mockall::automock;

/// Trait to reflect all used functions of [`Bot`].
/// If a new functionallity from [`Bot`] is needed, then it should be added in this trait.
///
/// Used for mocking in unit-tests.
///
/// [`Bot`]: teloxide::Bot
#[cfg_attr(test, automock(
    type Err=mock::Err;
    
    type Message=mock::Message;
    type MessageFuture = mock::MessageFuture;
    type SendMessage=mock::SendMessage;
    
    type Me=mock::Me;
    type MeFuture = mock::MeFuture;
    type GetMe=mock::GetMe;
))]
#[allow(clippy::module_name_repetitions)]
#[cfg_attr(test, allow(clippy::indexing_slicing))] // From `automock` macro expansion
pub trait BotTrait {
    type Err: std::error::Error + Send + Sync;

    type Message;
    type MessageFuture: Future<Output = Result<Self::Message, Self::Err>> + Send;
    type SendMessage: IntoFuture<IntoFuture = Self::MessageFuture> + MessageSetters + Send;

    fn send_message<C, T>(&self, chat_id: C, message: T) -> Self::SendMessage
    where
        C: Into<teloxide::types::Recipient> + 'static,
        T: Into<String> + 'static;

    type Me: MeGetters + Send;
    type MeFuture: Future<Output = Result<Self::Me, Self::Err>> + Send;
    type GetMe: IntoFuture<IntoFuture = Self::MeFuture> + Send;

    fn get_me(&self) -> Self::GetMe;
}

pub trait MessageSetters {
    fn reply_markup<T>(self, value: T) -> Self
    where
        T: Into<teloxide::types::ReplyMarkup>;
}

pub trait MeGetters {
    fn user(&self) -> &teloxide::types::User;
}

impl BotTrait for teloxide::Bot {
    type Err = teloxide::errors::RequestError;

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
        T: Into<teloxide::types::ReplyMarkup>,
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
mod mock {
    use super::{MeGetters, MessageSetters};
    use std::future::{ready, Future, IntoFuture, Ready};

    pub type Err = std::convert::Infallible;

    pub type Message = ();
    pub type MessageFuture = Ready<Result<Message, Err>>;

    #[derive(Default)]
    pub struct SendMessage;

    impl IntoFuture for SendMessage {
        type Output = <Self::IntoFuture as Future>::Output;

        type IntoFuture = MessageFuture;

        fn into_future(self) -> Self::IntoFuture {
            ready(Ok(()))
        }
    }

    impl MessageSetters for SendMessage {
        fn reply_markup<T>(self, _value: T) -> Self
        where
            T: Into<teloxide::types::ReplyMarkup>,
        {
            self
        }
    }

    pub struct Me(teloxide::types::User);

    impl Default for Me {
        fn default() -> Self {
            Self(teloxide::types::User {
                id: teloxide::types::UserId(0),
                is_bot: true,
                first_name: "Mock".to_owned(),
                last_name: None,
                username: None,
                language_code: None,
                is_premium: false,
                added_to_attachment_menu: false,
            })
        }
    }

    impl MeGetters for Me {
        fn user(&self) -> &teloxide::types::User {
            &self.0
        }
    }

    pub type MeFuture = Ready<Result<Me, Err>>;

    #[derive(Default)]
    pub struct GetMe;

    impl IntoFuture for GetMe {
        type Output = <Self::IntoFuture as Future>::Output;

        type IntoFuture = MeFuture;

        fn into_future(self) -> Self::IntoFuture {
            ready(Ok(Me::default()))
        }
    }
}
