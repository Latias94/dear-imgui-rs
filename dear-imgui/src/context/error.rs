use std::error::Error;
use std::fmt;

use thiserror::Error;

use super::{Context, ContextBindingError, SuspendedContext};

/// Reason an active Context could not be suspended.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum ContextSuspensionReason {
    /// A restorable Context binding scope is active.
    #[error("a Context binding scope is active")]
    BindingScopeActive,
    /// The Context being suspended is not the current native Context.
    #[error("the Context is not the current Context")]
    NotCurrent,
    /// A Dear ImGui frame is still open on the Context.
    #[error("a Dear ImGui frame is still open")]
    FrameOpen,
}

/// Failure to suspend a Context without losing its owner.
#[derive(Debug, Error)]
#[error("failed to suspend Context: {reason}")]
pub struct ContextSuspensionError {
    owner: Context,
    reason: ContextSuspensionReason,
}

impl ContextSuspensionError {
    pub(super) fn new(owner: Context, reason: ContextSuspensionReason) -> Self {
        Self { owner, reason }
    }

    /// Returns the reason suspension was rejected.
    pub fn reason(&self) -> ContextSuspensionReason {
        self.reason
    }

    /// Borrows the still-owned Context.
    pub fn owner(&self) -> &Context {
        &self.owner
    }

    /// Mutably borrows the still-owned Context.
    pub fn owner_mut(&mut self) -> &mut Context {
        &mut self.owner
    }

    /// Recovers the Context so the caller can repair the conflict and retry.
    pub fn into_owner(self) -> Context {
        self.owner
    }

    /// Splits the error into the retained owner and rejection reason.
    pub fn into_parts(self) -> (Context, ContextSuspensionReason) {
        (self.owner, self.reason)
    }
}

/// Reason a suspended Context could not be activated.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum ContextActivationReason {
    /// Another Context is already current.
    #[error("another Context is already active")]
    ContextAlreadyActive,
    /// A restorable Context binding scope is active.
    #[error("a Context binding scope is active")]
    BindingScopeActive,
}

/// Failure to activate a suspended Context without losing its owner.
#[derive(Debug, Error)]
#[error("failed to activate suspended Context: {reason}")]
pub struct ContextActivationError {
    owner: SuspendedContext,
    reason: ContextActivationReason,
}

impl ContextActivationError {
    pub(super) fn new(owner: SuspendedContext, reason: ContextActivationReason) -> Self {
        Self { owner, reason }
    }

    /// Returns the reason activation was rejected.
    pub fn reason(&self) -> ContextActivationReason {
        self.reason
    }

    /// Borrows the still-owned suspended Context.
    pub fn owner(&self) -> &SuspendedContext {
        &self.owner
    }

    /// Mutably borrows the still-owned suspended Context.
    pub fn owner_mut(&mut self) -> &mut SuspendedContext {
        &mut self.owner
    }

    /// Recovers the suspended Context so activation can be retried.
    pub fn into_owner(self) -> SuspendedContext {
        self.owner
    }

    /// Splits the error into the retained owner and rejection reason.
    pub fn into_parts(self) -> (SuspendedContext, ContextActivationReason) {
        (self.owner, self.reason)
    }
}

/// Failure to enter or finish a temporary active-Context scope.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum ContextScopeError {
    /// The Context could not be made current before the closure ran.
    #[error("failed to activate suspended Context: {0}")]
    Activation(#[source] ContextActivationReason),
    /// The closure returned success while leaving a Dear ImGui frame open.
    #[error("active Context closure returned success with an open frame")]
    FrameLeftOpen,
    /// The Context became unavailable while entering its binding capability.
    ///
    /// Safe ownership normally keeps the Context available. This variant preserves a binding
    /// failure caused by teardown or raw/unsafe lifecycle interference instead of panicking.
    #[error("suspended Context became unavailable: {0}")]
    ContextUnavailable(#[source] ContextBindingError),
}

impl ContextScopeError {
    /// Returns the activation rejection reason, if the closure was not entered.
    pub fn activation_reason(self) -> Option<ContextActivationReason> {
        match self {
            Self::Activation(reason) => Some(reason),
            _ => None,
        }
    }
}

/// Failure while temporarily activating a borrowed [`SuspendedContext`].
#[derive(Debug)]
#[non_exhaustive]
pub enum ScopedActivationError<E> {
    /// The temporary Context scope could not be entered or completed.
    Scope(ContextScopeError),
    /// The caller's closure returned an error.
    Closure(E),
}

impl<E> From<ContextScopeError> for ScopedActivationError<E> {
    fn from(error: ContextScopeError) -> Self {
        Self::Scope(error)
    }
}

impl<E> ScopedActivationError<E> {
    /// Maps only the caller-provided closure error.
    pub fn map_closure<F>(self, f: impl FnOnce(E) -> F) -> ScopedActivationError<F> {
        match self {
            Self::Scope(error) => ScopedActivationError::Scope(error),
            Self::Closure(error) => ScopedActivationError::Closure(f(error)),
        }
    }

    /// Returns the Context-scope failure, if the caller's closure did not return an error.
    pub fn scope_error(&self) -> Option<ContextScopeError> {
        match self {
            Self::Scope(error) => Some(*error),
            _ => None,
        }
    }

    /// Returns the activation rejection reason, if the closure was not entered.
    pub fn activation_reason(&self) -> Option<ContextActivationReason> {
        self.scope_error()
            .and_then(ContextScopeError::activation_reason)
    }

    /// Returns the caller-provided closure error, if one was returned.
    pub fn closure_error(&self) -> Option<&E> {
        match self {
            Self::Closure(error) => Some(error),
            _ => None,
        }
    }

    /// Extracts the caller-provided closure error or returns the Context-scope failure.
    pub fn into_closure_error(self) -> Result<E, ContextScopeError> {
        match self {
            Self::Closure(error) => Ok(error),
            Self::Scope(error) => Err(error),
        }
    }
}

impl<E: fmt::Display> fmt::Display for ScopedActivationError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scope(error) => error.fmt(f),
            Self::Closure(error) => write!(f, "active Context closure failed: {error}"),
        }
    }
}

impl<E> Error for ScopedActivationError<E>
where
    E: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Scope(error) => Some(error),
            Self::Closure(error) => Some(error),
        }
    }
}
