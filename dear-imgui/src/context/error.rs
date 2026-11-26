use super::ContextBindingError;

/// Failure at the boundary of a scoped Context operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum ContextScopeError {
    /// A scope cannot take over a frame that was already open.
    #[error("the Context already has an open frame")]
    FrameAlreadyOpen,
    /// The operation returned success with a frame still open.
    #[error("the Context operation returned success with an open frame")]
    FrameLeftOpen,
    /// The native Context could not be bound.
    #[error("the Context is unavailable: {0}")]
    ContextUnavailable(#[source] ContextBindingError),
}

/// Either the scoped Context operation or its caller's closure failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ContextOperationError<E> {
    /// The scope could not be entered or finished.
    #[error(transparent)]
    Scope(#[from] ContextScopeError),
    /// The caller's closure returned an error.
    #[error("the Context operation failed: {0}")]
    Closure(E),
}

impl<E> ContextOperationError<E> {
    /// Maps the caller's error, preserving Context-scope errors.
    pub fn map_closure<F>(self, f: impl FnOnce(E) -> F) -> ContextOperationError<F> {
        match self {
            Self::Scope(error) => ContextOperationError::Scope(error),
            Self::Closure(error) => ContextOperationError::Closure(f(error)),
        }
    }

    /// Returns the Context-scope failure, if any.
    pub fn scope_error(&self) -> Option<ContextScopeError> {
        match self {
            Self::Scope(error) => Some(*error),
            Self::Closure(_) => None,
        }
    }

    /// Borrows the caller's error, if any.
    pub fn closure_error(&self) -> Option<&E> {
        match self {
            Self::Scope(_) => None,
            Self::Closure(error) => Some(error),
        }
    }

    /// Extracts the caller's error or returns the Context-scope failure.
    pub fn into_closure_error(self) -> Result<E, ContextScopeError> {
        match self {
            Self::Scope(error) => Err(error),
            Self::Closure(error) => Ok(error),
        }
    }
}
