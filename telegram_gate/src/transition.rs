//! Module with [`TryFromTransition`] trait and related error handling.

use std::future::Future;

use tracing::error;

#[mockall_double::double]
use crate::context::Context;

/// Error struct for [`TryFromTransition::try_from_transition()`] function,
/// containing error target state and reason of failure.
#[derive(Debug, thiserror::Error)]
#[error("Transition failed")]
pub struct FailedTransition<T> {
    /// Error target state of transition.
    pub target: T,
    /// Failure reason.
    #[source]
    pub reason: TransitionFailureReason,
}

/// Reason of failed transition.
#[derive(Debug, thiserror::Error)]
pub enum TransitionFailureReason {
    /// User mistake.
    #[error("User error: {0}")]
    User(String),
    /// Internal error occurred.
    #[error("Internal error")]
    Internal(#[source] color_eyre::Report),
}

impl<T> FailedTransition<T> {
    /// Create new [`FailedTransition`] with user mistake.
    pub fn user<R: Into<String>>(target: T, reason: R) -> Self {
        Self {
            target,
            reason: TransitionFailureReason::user(reason),
        }
    }

    /// Create new [`FailedTransition`] with internal error.
    pub fn internal<E: Into<color_eyre::Report>>(target: T, error: E) -> Self {
        Self {
            target,
            reason: TransitionFailureReason::internal(error),
        }
    }

    /// Transform [`FailedTransition`] to another [`FailedTransition`] with different target type
    /// which can be created from `T`.
    pub fn transform<U: From<T>>(self) -> FailedTransition<U> {
        FailedTransition {
            target: self.target.into(),
            reason: self.reason,
        }
    }
}

impl TransitionFailureReason {
    /// Create new [`TransitionFailureReason`] with user mistake.
    pub fn user<R: Into<String>>(reason: R) -> Self {
        Self::User(reason.into())
    }

    /// Create new [`TransitionFailureReason`] with internal error.
    pub fn internal<E: Into<color_eyre::Report>>(error: E) -> Self {
        Self::Internal(error.into())
    }
}

/// Macro which works similar to [`try!`], but packs errors into
/// [`FailedTransition`] with provided target `state`.
///
/// It's needed because basic `?` operator triggers `use of moved value` due to
/// lack of control flow understanding.
macro_rules! try_with_state {
    ($state:ident, $e:expr) => {{
        let value = $e;
        match value {
            Ok(ok) => ok,
            Err(err) => {
                return Err(FailedTransition {
                    target: $state,
                    reason: err,
                })
            }
        }
    }};
}

pub(crate) use try_with_state;

/// Trait to create state from another *state* `S` using event *B* *by* which transition is possible.
///
/// Will return [`Self::ErrorTarget`] as an error target state if transition failed.
pub trait TryFromTransition<S, B>: Sized + Send {
    /// Target state which will be returned on failed transition attempt.
    type ErrorTarget;

    /// Try to perform a transition from `S` to `Self` by `B`.
    ///
    /// Returns possibly different state with fail reason if not succeed.
    ///
    /// # Errors
    ///
    /// Fails if failed to perform a transition. Concrete error depends on the implementation.
    fn try_from_transition(
        from: S,
        by: B,
        context: &Context,
    ) -> impl Future<Output = Result<Self, FailedTransition<Self::ErrorTarget>>> + Send;
}

/// Trait to gracefully destroy state.
///
/// Implementors with meaningful [`destroy()`](Destroy::destroy) might want to use [`drop_bomb::DebugDropBomb`].
pub trait Destroy: Sized + Send {
    /// Destroy state.
    fn destroy(self, context: &Context) -> impl Future<Output = color_eyre::Result<()>> + Send;

    /// Destroy state and log error if it fails.
    fn destroy_and_log_err(self, context: &Context) -> impl Future<Output = ()> + Send {
        async {
            if let Err(error) = self.destroy(context).await {
                error!(?error, "Failed to destroy state");
            }
        }
    }
}
