//! Contains stronlgy-typed states of the [`Dialogue`](super::Dialogue).

use derive_more::{From, TryInto};

/// Error struct for [`MakeTransition::make_transition()`] function,
/// containing error target and reason of failure.
#[derive(Debug, thiserror::Error)]
#[error("Transition failed")]
pub struct FailedTransition<T> {
    /// Error target of transition.
    pub target: T,
    /// Failure reason.
    #[source]
    pub reason: eyre::Report,
}

/// Trait to make a transition from one state to another.
///
/// Transition will return `E` as an error target if transition failed.
// `E` defaults to `Self` which means that state will be unchanged if the
/// opposit is not specified.
pub trait MakeTransition<E = Self> {
    /// Event that makes transition possible.
    type By;

    /// Target state of succeed transition.
    type Target;

    /// Try to perfrom a transition from [`Self`] to [`Self::Target`].
    ///
    /// Rerturns possibly different state with fail reason if not succeed.
    ///
    /// # Errors
    ///
    /// Fails if failed to perform a transition. Concrete error depends on the implementation.
    fn make_transition(self, by: Self::By) -> Result<Self::Target, FailedTransition<E>>;
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, From, TryInto)]
pub enum StateBox {
    Unauthorized(Unauthorized<unauthorized::KindBox>),
    Authorized(Authorized),
}

impl Default for StateBox {
    fn default() -> Self {
        Self::Unauthorized(Unauthorized::default())
    }
}

/// Unauthorized state. Corresponds to the beginning of the dialogue.
///
/// User becomes [authorized](Authorized) when they submit the corresponding admin token.
#[derive(Debug, Clone)]
pub struct Unauthorized<K> {
    /// Secret token generated on every run.
    /// User should copy this token from logs and send to the bot in order to prove that they are admin.
    admin_token: String,
    kind: K,
}

impl Default for Unauthorized<unauthorized::KindBox> {
    fn default() -> Self {
        Self {
            admin_token: String::from("qwerty"), // TODO: generate secret token
            kind: unauthorized::KindBox::default(),
        }
    }
}

/// Auhtorized state.
#[derive(Debug, Default, Clone)]
pub struct Authorized;

pub mod unauthorized {
    //! Module with [`Unauthorized`](super::Unauthorized) states.

    use super::{From, TryInto};

    /// Boxed sub-state of [`Unauthorized`](super::Unauthorized).
    #[derive(Debug, Clone, Copy, From, TryInto)]
    pub enum KindBox {
        Start(Start),
    }

    impl Default for KindBox {
        fn default() -> Self {
            Self::Start(Start)
        }
    }

    /// Start of the dialog.
    #[derive(Debug, Default, Clone, Copy)]
    pub struct Start;

    impl TryFrom<super::Unauthorized<KindBox>> for super::Unauthorized<Start> {
        type Error = <Start as TryFrom<KindBox>>::Error;

        fn try_from(value: super::Unauthorized<KindBox>) -> Result<Self, Self::Error> {
            Ok(Self {
                admin_token: value.admin_token,
                kind: value.kind.try_into()?,
            })
        }
    }

    impl TryFrom<super::StateBox> for super::Unauthorized<Start> {
        type Error = <Self as TryFrom<super::Unauthorized<KindBox>>>::Error;

        fn try_from(value: super::StateBox) -> Result<Self, Self::Error> {
            super::Unauthorized::<KindBox>::try_from(value).and_then(TryInto::try_into)
        }
    }
}
