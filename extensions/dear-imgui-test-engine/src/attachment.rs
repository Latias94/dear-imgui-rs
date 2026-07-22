use std::cell::{Cell, RefCell};

use dear_imgui_rs::{
    ContextAttachment, ContextAttachmentTeardownError, ContextBinding, ContextDestroyed, ContextId,
    ContextTeardown,
};
use dear_imgui_test_engine_sys as sys;

use crate::error::ffi_status;
use crate::{AttachmentState, RunState, TestEngineError};

pub(crate) struct TestEngineAttachmentMarker;

pub(crate) struct AttachmentControl {
    raw: Cell<*mut sys::ImGuiTestEngine>,
    attachment: Cell<AttachmentState>,
    run: Cell<RunState>,
    context_id: Cell<Option<ContextId>>,
    binding: RefCell<Option<ContextBinding>>,
    teardown_error: RefCell<Option<TestEngineError>>,
}

impl AttachmentControl {
    pub(crate) fn new(raw: *mut sys::ImGuiTestEngine) -> Self {
        Self {
            raw: Cell::new(raw),
            attachment: Cell::new(AttachmentState::Detached),
            run: Cell::new(RunState::Inactive),
            context_id: Cell::new(None),
            binding: RefCell::new(None),
            teardown_error: RefCell::new(None),
        }
    }

    pub(crate) fn raw(&self) -> *mut sys::ImGuiTestEngine {
        self.raw.get()
    }

    pub(crate) fn attachment_state(&self) -> AttachmentState {
        self.attachment.get()
    }

    pub(crate) fn run_state(&self) -> RunState {
        self.run.get()
    }

    pub(crate) fn set_run_state(&self, state: RunState) {
        self.run.set(state);
    }

    pub(crate) fn context_id(&self) -> Option<ContextId> {
        self.context_id.get()
    }

    pub(crate) fn binding(&self) -> Option<ContextBinding> {
        self.binding.borrow().clone()
    }

    pub(crate) fn reserve(&self, binding: ContextBinding) {
        debug_assert_eq!(self.attachment.get(), AttachmentState::Detached);
        self.context_id.set(Some(binding.id()));
        self.binding.replace(Some(binding));
        self.attachment.set(AttachmentState::Reserved);
    }

    pub(crate) fn commit_start(&self) {
        debug_assert_eq!(self.attachment.get(), AttachmentState::Reserved);
        self.attachment.set(AttachmentState::Attached);
        self.run.set(RunState::Ready);
    }

    pub(crate) fn rollback_start(&self) {
        debug_assert_eq!(self.attachment.get(), AttachmentState::Reserved);
        self.context_id.set(None);
        self.binding.replace(None);
        self.attachment.set(AttachmentState::Detached);
        self.run.set(RunState::Inactive);
    }

    pub(crate) fn mark_detached(&self) {
        self.attachment.set(AttachmentState::Detached);
        self.run.set(RunState::Inactive);
    }

    pub(crate) fn mark_destroyed(&self) {
        self.raw.set(std::ptr::null_mut());
        self.context_id.set(None);
        self.binding.replace(None);
        self.attachment.set(AttachmentState::Destroyed);
        self.run.set(RunState::Inactive);
    }

    pub(crate) fn take_teardown_error(&self) -> Option<TestEngineError> {
        self.teardown_error.borrow_mut().take()
    }

    fn remember_teardown_status(
        &self,
        operation: &'static str,
        status: sys::ImGuiTestEngineStatus,
    ) -> Result<(), ContextAttachmentTeardownError> {
        if let Err(error) = ffi_status(operation, status) {
            let message = error.to_string();
            let mut teardown_error = self.teardown_error.borrow_mut();
            if teardown_error.is_none() {
                *teardown_error = Some(error);
            }
            return Err(ContextAttachmentTeardownError::new(message));
        }
        Ok(())
    }
}

impl ContextAttachment for AttachmentControl {
    fn quiesce(&self, context: &ContextTeardown<'_>) -> Result<(), ContextAttachmentTeardownError> {
        if self.attachment.get() != AttachmentState::Attached {
            return Ok(());
        }

        self.attachment.set(AttachmentState::ContextDropping);
        self.run.set(RunState::Inactive);
        let raw = self.raw.get();
        if raw.is_null() {
            return Ok(());
        }

        context.with_bound_context(|| {
            let mut started = false;
            let query_status = unsafe { sys::imgui_test_engine_is_started(raw, &mut started) };
            if query_status != sys::ImGuiTestEngineStatus_Success {
                return self.remember_teardown_status("imgui_test_engine_is_started", query_status);
            }
            if started {
                let stop_status = unsafe { sys::imgui_test_engine_stop(raw) };
                self.remember_teardown_status("imgui_test_engine_stop", stop_status)?;
            }
            Ok(())
        })
    }

    fn context_destroyed(&self, context: ContextDestroyed) {
        if self.context_id.get().is_some_and(|id| id == context.id()) {
            self.binding.replace(None);
            self.attachment.set(AttachmentState::ContextDestroyed);
            self.run.set(RunState::Inactive);
        }
    }
}
