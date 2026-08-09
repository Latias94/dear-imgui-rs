use std::cell::{Cell, RefCell};
use std::collections::HashSet;

use dear_imgui_rs::{Context, ContextBinding, Id, platform_io::PlatformIo};

use super::super::registry::{GlobalHandles, unregister_runtime};
use super::super::trace::{AshViewportFrameReport, FrameTraceState};
use super::{
    AshViewportError, CallbackState, RendererStorage, RuntimeControl, RuntimeFaults,
    RuntimeIdentity, RuntimeState,
};
use crate::AshRenderer;
#[cfg(test)]
use crate::RendererError;

impl RuntimeControl {
    pub(super) fn new(context: &Context, renderer: AshRenderer, globals: GlobalHandles) -> Self {
        Self::new_with_storage(
            context,
            RendererStorage::Real(Box::new(renderer)),
            Some(globals),
        )
    }

    pub(super) fn new_with_storage(
        context: &Context,
        renderer: RendererStorage,
        globals: Option<GlobalHandles>,
    ) -> Self {
        Self {
            context_raw: context.as_raw(),
            binding: context.binding(),
            identity: RuntimeIdentity::new(),
            state: Cell::new(RuntimeState::Constructing),
            renderer: RefCell::new(Some(renderer)),
            globals: RefCell::new(globals),
            attachment: RefCell::new(None),
            callback_state: Cell::new(CallbackState::Unclaimed),
            failed_viewports: RefCell::new(HashSet::new()),
            retained_viewports: RefCell::new(Vec::new()),
            faults: RefCell::new(RuntimeFaults::default()),
            frame_trace: RefCell::new(FrameTraceState::default()),
            #[cfg(test)]
            panic_next_callback: Cell::new(false),
            #[cfg(test)]
            callback_probe_count: Cell::new(0),
            #[cfg(test)]
            transitions: RefCell::new(Vec::new()),
            #[cfg(test)]
            renderer_contract_fault: Cell::new(None),
        }
    }

    pub(in super::super) fn context_raw(&self) -> *mut dear_imgui_rs::sys::ImGuiContext {
        self.context_raw
    }

    pub(in super::super) fn binding(&self) -> &ContextBinding {
        &self.binding
    }

    pub(in super::super) fn is_callback_accessible(&self) -> bool {
        self.state.get() == RuntimeState::Attached
            && self.callback_state.get() != CallbackState::Released
    }

    pub(in super::super) fn is_cleanup_callback_accessible(&self) -> bool {
        matches!(
            self.state.get(),
            RuntimeState::Attached | RuntimeState::ShuttingDown
        ) && self.callback_state.get() == CallbackState::Claimed
    }

    pub(in super::super) fn can_enter_callback(&self) -> bool {
        self.is_callback_accessible()
            && self
                .faults
                .try_borrow()
                .is_ok_and(|faults| !faults.has_pending())
    }

    pub(in super::super) fn should_validate_runtime_contract(&self) -> bool {
        self.state.get() == RuntimeState::Attached
            && self.callback_state.get() == CallbackState::Claimed
    }

    pub(in super::super) fn callback_released(&self) -> bool {
        self.callback_state.get() == CallbackState::Released
    }

    pub(in super::super) fn mark_callback_claimed(&self) {
        self.callback_state.set(CallbackState::Claimed);
    }

    pub(in super::super) fn mark_callback_released(&self) {
        self.callback_state.set(CallbackState::Released);
        unregister_runtime(self.binding.id());
    }

    pub(super) fn set_state(&self, state: RuntimeState) {
        #[cfg(test)]
        let previous = self.state.replace(state);
        #[cfg(not(test))]
        self.state.set(state);
        #[cfg(test)]
        if previous != state {
            match state {
                RuntimeState::ShuttingDown => self.transitions.borrow_mut().push("ShuttingDown"),
                RuntimeState::Detached => self.transitions.borrow_mut().push("Detached"),
                RuntimeState::ResourceDropped => {
                    self.transitions.borrow_mut().push("ResourceDropped");
                }
                RuntimeState::Constructing | RuntimeState::Attached => {}
            }
        }
    }

    pub(in super::super) fn begin_shutdown(&self) {
        if matches!(
            self.state.get(),
            RuntimeState::Constructing | RuntimeState::Attached
        ) {
            self.set_state(RuntimeState::ShuttingDown);
        }
    }

    pub(super) fn mark_detached(&self) {
        if !matches!(
            self.state.get(),
            RuntimeState::Detached | RuntimeState::ResourceDropped
        ) {
            self.set_state(RuntimeState::Detached);
        }
        unregister_runtime(self.binding.id());
    }

    pub(super) fn begin_frame_trace(&self) -> Result<(), AshViewportError> {
        self.ensure_entry()?;
        if self.frame_trace.borrow_mut().begin() {
            Ok(())
        } else {
            Err(AshViewportError::FrameTraceAlreadyActive)
        }
    }

    pub(super) fn finish_frame_trace(&self) -> AshViewportFrameReport {
        self.frame_trace.borrow_mut().finish()
    }

    pub(super) fn abort_frame_trace(&self) {
        self.frame_trace.borrow_mut().abort();
    }

    pub(in super::super) fn record_viewport_render_submitted(&self, viewport_id: Id) {
        self.frame_trace
            .borrow_mut()
            .record_render_submitted(viewport_id);
    }

    pub(in super::super) fn record_viewport_present_submitted(&self, viewport_id: Id) {
        self.frame_trace
            .borrow_mut()
            .record_present_submitted(viewport_id);
    }

    pub(in super::super) fn validate_renderer_contract(&self) -> Result<(), AshViewportError> {
        #[cfg(test)]
        if let Some(field) = self.renderer_contract_fault.get() {
            return Err(RendererError::RendererStateReplaced { field }.into());
        }
        let renderer =
            self.renderer
                .try_borrow()
                .map_err(|_| AshViewportError::CallbackReentered {
                    callback: "validate renderer runtime contract",
                })?;
        renderer
            .as_ref()
            .ok_or(AshViewportError::RuntimeDetached)?
            .ensure_operational()
    }

    /// Returns whether the current Context still has an exact core renderer publication owned by
    /// this runtime. A callback-table takeover may replace every viewport callback while leaving
    /// a partially owned core lease behind; in that case failure handling must still revoke the
    /// viewport capability. If ownership cannot be proven, callers preserve the shared bit.
    pub(in super::super) fn owns_core_renderer_publication(
        &self,
        platform_io: &PlatformIo,
    ) -> bool {
        let Ok(renderer) = self.renderer.try_borrow() else {
            return false;
        };
        match renderer.as_ref() {
            Some(RendererStorage::Real(renderer)) => renderer
                .context_state
                .owns_core_publication_bound(platform_io),
            #[cfg(test)]
            Some(RendererStorage::Fake { .. }) | None => false,
            #[cfg(not(test))]
            None => false,
        }
    }

    pub(super) fn ensure_context(&self, context: &Context) -> Result<(), AshViewportError> {
        if context.id() == self.binding.id() {
            Ok(())
        } else {
            Err(AshViewportError::ContextMismatch {
                expected: self.binding.id(),
                actual: context.id(),
            })
        }
    }

    fn ensure_entry(&self) -> Result<(), AshViewportError> {
        if let Some(fault) = self.detect_and_take_fault() {
            return Err(fault);
        }
        if self.state.get() == RuntimeState::Attached {
            Ok(())
        } else {
            Err(AshViewportError::RuntimeDetached)
        }
    }

    fn finish_entry(&self) -> Result<(), AshViewportError> {
        self.detect_and_take_fault().map_or(Ok(()), Err)
    }

    pub(super) fn with_renderer_mut<R>(
        &self,
        callback_name: &'static str,
        callback: impl FnOnce(&mut AshRenderer) -> Result<R, AshViewportError>,
    ) -> Result<R, AshViewportError> {
        self.ensure_entry()?;
        let result = {
            let mut renderer = self.renderer.try_borrow_mut().map_err(|_| {
                AshViewportError::CallbackReentered {
                    callback: callback_name,
                }
            })?;
            let renderer = renderer
                .as_mut()
                .and_then(RendererStorage::real_mut)
                .ok_or(AshViewportError::RuntimeDetached)?;
            renderer.ensure_operational()?;
            callback(renderer)
        }?;
        self.finish_entry()?;
        Ok(result)
    }

    pub(super) fn with_renderer<R>(
        &self,
        callback: impl FnOnce(&AshRenderer) -> R,
    ) -> Result<R, AshViewportError> {
        self.ensure_entry()?;
        let result = {
            let renderer =
                self.renderer
                    .try_borrow()
                    .map_err(|_| AshViewportError::CallbackReentered {
                        callback: "Rust runtime entry",
                    })?;
            let renderer = match renderer.as_ref() {
                Some(RendererStorage::Real(renderer)) => renderer.as_ref(),
                #[cfg(test)]
                Some(RendererStorage::Fake { .. }) | None => {
                    return Err(AshViewportError::RuntimeDetached);
                }
                #[cfg(not(test))]
                None => return Err(AshViewportError::RuntimeDetached),
            };
            renderer.ensure_operational()?;
            callback(renderer)
        };
        self.finish_entry()?;
        Ok(result)
    }

    pub(in super::super) fn with_renderer_callback<R>(
        &self,
        callback_name: &'static str,
        callback: impl FnOnce(&mut AshRenderer, &GlobalHandles) -> Result<R, AshViewportError>,
    ) -> Result<R, AshViewportError> {
        let mut renderer =
            self.renderer
                .try_borrow_mut()
                .map_err(|_| AshViewportError::CallbackReentered {
                    callback: callback_name,
                })?;
        let renderer = renderer
            .as_mut()
            .and_then(RendererStorage::real_mut)
            .ok_or(AshViewportError::RuntimeDetached)?;
        renderer.ensure_operational()?;
        let globals =
            self.globals
                .try_borrow()
                .map_err(|_| AshViewportError::CallbackReentered {
                    callback: callback_name,
                })?;
        let globals = globals.as_ref().ok_or(AshViewportError::RuntimeDetached)?;
        callback(renderer, globals)
    }

    pub(in super::super) fn with_renderer_teardown<R>(
        &self,
        callback: impl FnOnce(&mut AshRenderer, &GlobalHandles) -> Result<R, AshViewportError>,
    ) -> Result<Option<R>, AshViewportError> {
        let mut storage =
            self.renderer
                .try_borrow_mut()
                .map_err(|_| AshViewportError::CallbackReentered {
                    callback: "Ash viewport runtime shutdown",
                })?;
        let Some(storage) = storage.as_mut() else {
            return Ok(None);
        };
        #[cfg(test)]
        if matches!(storage, RendererStorage::Fake { .. }) {
            return Ok(None);
        }
        let renderer = storage
            .real_mut()
            .ok_or(AshViewportError::RuntimeDetached)?;
        let globals =
            self.globals
                .try_borrow()
                .map_err(|_| AshViewportError::CallbackReentered {
                    callback: "Ash viewport runtime shutdown",
                })?;
        let globals = globals.as_ref().ok_or(AshViewportError::RuntimeDetached)?;
        callback(renderer, globals).map(Some)
    }

    #[cfg(test)]
    pub(in super::super) fn state(&self) -> RuntimeState {
        self.state.get()
    }

    #[cfg(test)]
    pub(super) fn renderer_address_for_test(&self) -> *const () {
        self.renderer
            .borrow()
            .as_ref()
            .map_or(std::ptr::null(), RendererStorage::address)
    }

    #[cfg(test)]
    pub(super) fn panic_next_callback_for_test(&self) {
        self.panic_next_callback.set(true);
    }

    #[cfg(test)]
    pub(in super::super) fn maybe_panic_callback_for_test(&self) {
        assert!(
            !self.panic_next_callback.replace(false),
            "injected Ash viewport callback panic"
        );
    }

    #[cfg(test)]
    pub(in super::super) fn probe_renderer_storage_for_test(&self) -> Result<(), AshViewportError> {
        let storage =
            self.renderer
                .try_borrow()
                .map_err(|_| AshViewportError::CallbackReentered {
                    callback: "injected stable-storage probe",
                })?;
        let storage = storage.as_ref().ok_or(AshViewportError::RuntimeDetached)?;
        assert!(!storage.address().is_null());
        self.callback_probe_count
            .set(self.callback_probe_count.get() + 1);
        Ok(())
    }

    #[cfg(test)]
    pub(in super::super) fn replace_renderer_contract_for_test(&self, field: &'static str) {
        self.renderer_contract_fault.set(Some(field));
    }

    #[cfg(test)]
    pub(super) fn trigger_reentrant_entry_for_test(&self) {
        let _borrow = self.renderer.borrow_mut();
        let error = self
            .with_renderer_callback("injected reentry", |_renderer, _globals| Ok(()))
            .unwrap_err();
        self.record_fault(error);
    }

    #[cfg(test)]
    pub(super) fn transition_log_for_test(&self) -> Vec<&'static str> {
        self.transitions.borrow().clone()
    }

    #[cfg(test)]
    pub(super) fn callback_probe_count_for_test(&self) -> usize {
        self.callback_probe_count.get()
    }
}
