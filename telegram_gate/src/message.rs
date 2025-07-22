//! This module contains strongly-typed messages user can send.

use std::str::FromStr as _;

use derive_more::From;
use parse_display::{Display, FromStr};
use teloxide::types::{MessageId, MessageKind};

use crate::{TelegramMessage, TelegramMessageGettersExt as _};

/// Enum with all possible messages.
#[derive(Debug, Display, Clone, From)]
#[display("{}")]
pub enum MessageBox {
    /// Message from a Web App.
    WebApp(Message<kind::WebApp>),
    /// "Add" message.
    Add(Message<kind::Add>),
    /// "List" message.
    List(Message<kind::List>),
    /// Any arbitrary text message. Parsing will always fallback to this if nothing else matched.
    Arbitrary(Message<kind::Arbitrary>),
}

impl MessageBox {
    /// Construct new [`MessageBox`].
    ///
    /// Returns [`None`] if message is of unsupported kind.
    #[must_use]
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "only exact variants are needed"
    )]
    pub fn new(msg: TelegramMessage) -> Option<Self> {
        let id = msg.id();
        match msg.take_kind() {
            MessageKind::WebAppData(data) => Some(
                Message {
                    id,
                    kind: kind::WebApp(data.web_app_data),
                }
                .into(),
            ),
            MessageKind::Common(teloxide::types::MessageCommon {
                media_kind:
                    teloxide::types::MediaKind::Text(teloxide::types::MediaText { text, .. }),
                ..
            }) => Some(
                kind::Add::from_str(&text)
                    .map(|add| Message::new(id, add).into())
                    .or_else(|_| {
                        kind::List::from_str(&text).map(|list| Message::new(id, list).into())
                    })
                    .unwrap_or_else(|_| Message::new(id, kind::Arbitrary(text)).into()),
            ),
            _ => None,
        }
    }
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::allow_attributes, reason = "false positive"))]
#[cfg_attr(
    test,
    allow(
        clippy::multiple_inherent_impl,
        reason = "better looking conditional compilation"
    )
)]
impl MessageBox {
    #[must_use]
    pub const fn web_app(data: String, button_text: String) -> Self {
        Self::WebApp(Message {
            id: MessageId(0),
            kind: kind::WebApp(teloxide::types::WebAppData { data, button_text }),
        })
    }

    #[must_use]
    pub const fn list() -> Self {
        Self::List(Message {
            id: MessageId(0),
            kind: kind::List,
        })
    }

    #[must_use]
    pub const fn add() -> Self {
        Self::Add(Message {
            id: MessageId(0),
            kind: kind::Add,
        })
    }

    #[must_use]
    pub fn arbitrary(text: &'static str) -> Self {
        Self::Arbitrary(Message {
            id: MessageId(0),
            kind: kind::Arbitrary(text.to_owned()), // TODO: Redundant cloning
        })
    }
}

/// Message struct generic over message kind.
#[derive(derive_more::Constructor, Debug, Clone)]
pub struct Message<K> {
    /// Original Telegram message.
    pub id: MessageId,
    /// Message kind.
    pub kind: K,
}

impl<K: std::fmt::Display> std::fmt::Display for Message<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.kind)
    }
}

pub mod kind {
    //! Module with all possible [`Message`] kinds.

    use super::*;

    /// Message from a Web App.
    #[derive(Debug, Clone)]
    pub struct WebApp(pub teloxide::types::WebAppData);

    /// "Add" message.
    #[derive(Debug, Display, Copy, Clone, FromStr)]
    #[display("🆕 Add")]
    pub struct Add;

    /// "List" message.
    #[derive(Debug, Display, Copy, Clone, FromStr)]
    #[display("🗒 List")]
    pub struct List;

    /// Any arbitrary message.
    #[derive(Debug, Clone, Display)]
    #[display("{0}")]
    pub struct Arbitrary(pub String);
}

#[cfg(test)]
mod tests {
    #![expect(clippy::non_ascii_literal, reason = "emojis are allowed")]

    use super::*;

    #[expect(
        dead_code,
        unreachable_code,
        unused_variables,
        clippy::unimplemented,
        clippy::diverging_sub_expression,
        clippy::panic,
        reason = "not needed as it's a static check"
    )]
    #[forbid(clippy::todo, clippy::wildcard_enum_match_arm)]
    fn tests_completeness_static_check() -> ! {
        panic!("You should never call this function, it's purpose is the static check only");

        let message: MessageBox = unimplemented!();

        match message {
            MessageBox::WebApp(_) => parse_web_app(),
            MessageBox::Add(_) => parse_add(),
            MessageBox::List(_) => parse_list(),
            MessageBox::Arbitrary(_) => parse_arbitrary(),
        }

        unreachable!()
    }

    fn text_tg_message(text: String) -> TelegramMessage {
        let mut tg_message = TelegramMessage::default();
        tg_message
            .expect_take_kind()
            .return_const(teloxide::types::MessageKind::Common(
                teloxide::types::MessageCommon {
                    author_signature: None,
                    reply_to_message: None,
                    edit_date: None,
                    media_kind: teloxide::types::MediaKind::Text(teloxide::types::MediaText {
                        text,
                        entities: Vec::default(),
                        link_preview_options: None,
                    }),
                    reply_markup: None,
                    is_automatic_forward: false,
                    has_protected_content: false,
                    forward_origin: None,
                    external_reply: None,
                    quote: None,
                    paid_star_count: None,
                    effect_id: None,
                    reply_to_story: None,
                    sender_boost_count: None,
                    is_from_offline: false,
                    business_connection_id: None,
                },
            ));
        tg_message.expect_id().return_const(MessageId(0));
        tg_message
    }

    #[test]
    fn parse_web_app() {
        let mut tg_message = TelegramMessage::default();
        let data = teloxide::types::WebAppData {
            data: String::from("test_data"),
            button_text: String::from("test_button"),
        };
        tg_message
            .expect_take_kind()
            .return_const(MessageKind::WebAppData(
                teloxide::types::MessageWebAppData {
                    web_app_data: data.clone(),
                },
            ));
        tg_message.expect_id().return_const(MessageId(0));

        let message = MessageBox::new(tg_message);
        assert!(
            matches!(message, Some(MessageBox::WebApp(Message {kind: kind::WebApp(d), .. })) if d == data)
        );
    }

    #[test]
    fn parse_add() {
        let tg_message = text_tg_message("🆕 Add".to_owned());

        let message = MessageBox::new(tg_message);
        assert!(matches!(message, Some(MessageBox::Add(_))));
    }

    #[test]
    fn parse_list() {
        let tg_message = text_tg_message("🗒 List".to_owned());

        let message = MessageBox::new(tg_message);
        assert!(matches!(message, Some(MessageBox::List(_))));
    }

    #[test]
    fn parse_arbitrary() {
        let tg_message = text_tg_message("Any random string here".to_owned());

        let message = MessageBox::new(tg_message);
        assert!(matches!(message, Some(MessageBox::Arbitrary(_))));
    }
}
