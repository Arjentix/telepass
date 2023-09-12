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

    use mockall::mock;

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
}
