use crate::{CteError, CteResult};
use dear_imgui_rs::{Context, ContextBinding, ContextId, ContextThreadLease, Ui};

pub(crate) struct CteContextBinding {
    inner: ContextBinding,
    _thread_lease: ContextThreadLease,
}

impl CteContextBinding {
    pub(crate) fn new(context: &Context) -> CteResult<Self> {
        Ok(Self {
            inner: context.binding(),
            _thread_lease: ContextThreadLease::acquire()?,
        })
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

    pub(crate) fn try_with_bound_context<R>(
        &self,
        operation: &'static str,
        f: impl FnOnce() -> R,
    ) -> CteResult<R> {
        self.inner
            .try_with_bound_context(f)
            .map_err(|source| CteError::ContextBinding { operation, source })
    }

    // The audited destructors release owned CTE objects via IM_DELETE. The thread
    // lease keeps the ImGui allocator on this thread even after the Context is gone.
    pub(crate) fn destroy(&self, mut destroy: impl FnMut()) {
        if self.inner.try_with_bound_context(&mut destroy).is_ok() {
            return;
        }
        destroy();
    }
}
