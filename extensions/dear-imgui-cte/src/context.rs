use crate::{CteError, CteResult};
use dear_imgui_rs::{Context, ContextBinding, ContextId, Ui};

#[derive(Clone)]
pub(crate) struct CteContextBinding {
    inner: ContextBinding,
}

impl CteContextBinding {
    pub(crate) fn new(context: &Context) -> Self {
        Self {
            inner: context.binding(),
        }
    }

    pub(crate) fn id(&self) -> ContextId {
        self.inner.id()
    }

    pub(crate) fn require_ui(&self, operation: &'static str, ui: &Ui) -> CteResult<()> {
        let actual = ui.context_id();
        let expected = self.id();
        if actual != expected {
            return Err(CteError::WrongContext {
                operation,
                expected,
                actual,
            });
        }
        Ok(())
    }

    pub(crate) fn with_bound_context<R>(
        &self,
        operation: &'static str,
        f: impl FnOnce() -> R,
    ) -> R {
        self.try_with_bound_context(operation, f)
            .unwrap_or_else(|error| panic!("{error}"))
    }

    pub(crate) fn try_with_bound_context<R>(
        &self,
        operation: &'static str,
        f: impl FnOnce() -> R,
    ) -> CteResult<R> {
        self.inner
            .try_with_bound_context(f)
            .map_err(|source| CteError::ContextBinding { operation, source })
    }
}
