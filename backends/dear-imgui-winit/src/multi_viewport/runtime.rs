use std::cell::{Cell, RefCell};
use std::collections::{HashSet, VecDeque};
use std::fmt;
use std::marker::PhantomData;
use std::rc::{Rc, Weak};
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

use dear_imgui_rs::{
    Context, ContextAttachmentTeardownError, ContextBinding, ContextBindingError, ContextDestroyed,
    ContextTeardown,
};
#[cfg(test)]
use winit::event::Event;
use winit::event_loop::ActiveEventLoop;
#[cfg(target_os = "linux")]
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Window, WindowId};

use super::callbacks::{
    MonitorOwnership, PlatformCallbackContract, PreparedMonitors, claim_platform_callbacks,
    has_owned_platform_callback_in_current_context, preflight_platform_callbacks,
    preflight_platform_window_destruction, prepare_monitors, publish_monitors, refresh_monitors,
    release_platform_callbacks, validate_platform_callback_contract,
};
use super::focus::{ContextFocusState, PlatformFocusState};
use super::native_cursor_hittest::focus_and_raise_window;
#[cfg(target_os = "windows")]
use super::native_cursor_hittest::query_native_mouse_state;
#[cfg(target_os = "windows")]
use super::registry::viewport_id_for_native_window;
use super::registry::{
    FailedViewport, apply_pending_geometry_refresh, preflight_viewport_ownership,
    reassert_failed_viewports, register_runtime, request_geometry_refresh_for_window,
    secondary_viewport_windows, unregister_runtime,
};
use super::viewport_data::{init_main_viewport, preflight_main_viewport};
use crate::cursor::CursorSettings;
use crate::platform::{WinitPlatformControl, WinitPlatformError};

mod input_state;
mod lifecycle;

#[cfg(test)]
pub(super) use self::input_state::ReleasedInput;
pub(super) use self::input_state::{InputOwnership, MouseLeaveState};
#[cfg(test)]
pub(super) use self::lifecycle::WinitPlatformRuntime;
#[cfg(test)]
pub(super) use self::lifecycle::validate_multi_viewport_hidpi_mode;
#[cfg(test)]
pub(super) use self::lifecycle::validate_window_system_for_test;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ConstructionStage {
    Attachment,
    Registry,
    MainViewport,
    Callbacks,
    Monitors,
    BackendFlags,
}

impl ConstructionStage {
    #[cfg(test)]
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Attachment => "attachment",
            Self::Registry => "registry",
            Self::MainViewport => "main viewport",
            Self::Callbacks => "callbacks",
            Self::Monitors => "monitors",
            Self::BackendFlags => "backend flags",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RuntimeState {
    Constructing,
    Attached,
    Faulted,
    ShuttingDown,
    Detached,
    ContextDestroyed,
}

struct QueuedPlatformFault {
    error: WinitPlatformError,
    terminal: bool,
}

fn invalidate_mouse_coordinate_cache(io: &mut dear_imgui_rs::Io) {
    let unavailable = [-f32::MAX, -f32::MAX];
    io.set_mouse_pos(unavailable);
    io.add_mouse_pos_event(unavailable);
    io.add_mouse_viewport_event(dear_imgui_rs::Id::default());
}

fn invalidate_raw_mouse_coordinate_cache(io: &mut dear_imgui_rs::sys::ImGuiIO) {
    io.MousePos = dear_imgui_rs::sys::ImVec2 {
        x: -f32::MAX,
        y: -f32::MAX,
    };
    io.MouseHoveredViewport = 0;
    unsafe {
        dear_imgui_rs::sys::ImGuiIO_AddMousePosEvent(io, -f32::MAX, -f32::MAX);
        dear_imgui_rs::sys::ImGuiIO_AddMouseViewportEvent(io, 0);
    }
}

#[cfg(test)]
pub(super) fn apply_raw_io_coordinate_contract_for_test(
    io: &mut dear_imgui_rs::sys::ImGuiIO,
    display_size: [f32; 2],
    framebuffer_scale: [f32; 2],
) {
    apply_raw_io_coordinate_contract(io, display_size, framebuffer_scale);
}

fn apply_raw_io_coordinate_contract(
    io: &mut dear_imgui_rs::sys::ImGuiIO,
    display_size: [f32; 2],
    framebuffer_scale: [f32; 2],
) {
    io.DisplaySize = dear_imgui_rs::sys::ImVec2 {
        x: display_size[0],
        y: display_size[1],
    };
    io.DisplayFramebufferScale = dear_imgui_rs::sys::ImVec2 {
        x: framebuffer_scale[0],
        y: framebuffer_scale[1],
    };
    invalidate_raw_mouse_coordinate_cache(io);
}

/// A closure-scoped view of Winit's active event loop.
///
/// The lifetime is introduced by [`crate::WinitPlatform::with_event_loop`], so this token and the
/// `ActiveEventLoop` reference it exposes cannot be returned from the closure.
pub struct EventLoopScope<'scope> {
    event_loop: &'scope ActiveEventLoop,
    _invariant: PhantomData<&'scope mut &'scope ()>,
}

impl<'scope> EventLoopScope<'scope> {
    /// Returns the active event loop for work performed inside this scope.
    pub fn active_event_loop(&self) -> &'scope ActiveEventLoop {
        self.event_loop
    }
}

/// Result of one Winit viewport attempt.
///
/// The callback is skipped when the exact platform generation already has pending faults. When
/// it runs, its output is retained even if native callbacks report additional faults while the
/// event-loop capability is active. This keeps callback and platform failures as parallel outputs
/// instead of imposing nested `Result` precedence.
#[must_use = "split the callback output and deferred platform faults with into_parts"]
#[derive(Debug)]
pub struct WinitViewportAttempt<R> {
    output: Option<R>,
    faults: Vec<WinitPlatformError>,
}

impl<R> WinitViewportAttempt<R> {
    fn skipped(faults: Vec<WinitPlatformError>) -> Self {
        debug_assert!(!faults.is_empty());
        Self {
            output: None,
            faults,
        }
    }

    fn completed(output: R, faults: Vec<WinitPlatformError>) -> Self {
        Self {
            output: Some(output),
            faults,
        }
    }

    /// Splits the retained callback output from deferred platform faults.
    #[must_use]
    pub fn into_parts(self) -> (Option<R>, Vec<WinitPlatformError>) {
        (self.output, self.faults)
    }
}

/// Exact-generation Winit adapter retained by a first-party renderer route.
///
/// This is implementation plumbing between published backend crates, not a second platform
/// lifecycle owner. Applications continue to own and use [`crate::WinitPlatform`].
#[doc(hidden)]
pub struct WinitViewportRendererAdapter {
    control: Rc<RuntimeControl>,
    platform: Rc<WinitPlatformControl>,
}

impl fmt::Debug for WinitViewportRendererAdapter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WinitViewportRendererAdapter")
            .field("context", &self.control.binding().id())
            .finish_non_exhaustive()
    }
}

pub(crate) struct RuntimeControl {
    context_raw: *mut dear_imgui_rs::sys::ImGuiContext,
    binding: ContextBinding,
    platform: Weak<WinitPlatformControl>,
    state: Cell<RuntimeState>,
    event_loop: Cell<*const ActiveEventLoop>,
    teardown_callbacks_active: Cell<bool>,
    core_teardown_owns_callback_guard: Cell<bool>,
    platform_callback_contract: Cell<Option<PlatformCallbackContract>>,
    platform_callback_drift: Cell<Option<&'static str>>,
    faults: RefCell<VecDeque<QueuedPlatformFault>>,
    terminal_fault_recorded: Cell<bool>,
    monitor_ownership: RefCell<Option<MonitorOwnership>>,
    main_window: RefCell<Option<Arc<Window>>>,
    mouse_leave: Cell<MouseLeaveState>,
    input_ownership: RefCell<InputOwnership>,
    focus: RefCell<ContextFocusState>,
    platform_focus: Cell<PlatformFocusState>,
    pub(super) viewports: RefCell<Vec<super::registry::ViewportEntry>>,
    pub(super) failed_viewports: RefCell<Vec<FailedViewport>>,
}

impl RuntimeControl {
    fn new(
        context: &Context,
        platform: &Rc<WinitPlatformControl>,
        main_window: Arc<Window>,
    ) -> Self {
        let focus = ContextFocusState::with_focused_window(
            main_window.has_focus().then_some(main_window.id()),
        );
        Self {
            context_raw: context.as_raw(),
            binding: context.binding(),
            platform: Rc::downgrade(platform),
            state: Cell::new(RuntimeState::Constructing),
            event_loop: Cell::new(std::ptr::null()),
            teardown_callbacks_active: Cell::new(false),
            core_teardown_owns_callback_guard: Cell::new(false),
            platform_callback_contract: Cell::new(None),
            platform_callback_drift: Cell::new(None),
            faults: RefCell::new(VecDeque::new()),
            terminal_fault_recorded: Cell::new(false),
            monitor_ownership: RefCell::new(None),
            main_window: RefCell::new(Some(main_window)),
            mouse_leave: Cell::new(MouseLeaveState::default()),
            input_ownership: RefCell::new(InputOwnership::default()),
            focus: RefCell::new(focus),
            platform_focus: Cell::new(PlatformFocusState::default()),
            viewports: RefCell::new(Vec::new()),
            failed_viewports: RefCell::new(Vec::new()),
        }
    }

    #[cfg(test)]
    pub(super) fn new_for_test(context: &Context, platform: &Rc<WinitPlatformControl>) -> Self {
        Self {
            context_raw: context.as_raw(),
            binding: context.binding(),
            platform: Rc::downgrade(platform),
            state: Cell::new(RuntimeState::Constructing),
            event_loop: Cell::new(std::ptr::null()),
            teardown_callbacks_active: Cell::new(false),
            core_teardown_owns_callback_guard: Cell::new(false),
            platform_callback_contract: Cell::new(None),
            platform_callback_drift: Cell::new(None),
            faults: RefCell::new(VecDeque::new()),
            terminal_fault_recorded: Cell::new(false),
            monitor_ownership: RefCell::new(None),
            main_window: RefCell::new(None),
            mouse_leave: Cell::new(MouseLeaveState::default()),
            input_ownership: RefCell::new(InputOwnership::default()),
            focus: RefCell::new(ContextFocusState::default()),
            platform_focus: Cell::new(PlatformFocusState::default()),
            viewports: RefCell::new(Vec::new()),
            failed_viewports: RefCell::new(Vec::new()),
        }
    }

    pub(super) fn context_raw(&self) -> *mut dear_imgui_rs::sys::ImGuiContext {
        self.context_raw
    }

    pub(super) fn binding(&self) -> &ContextBinding {
        &self.binding
    }

    pub(super) fn ensure_context(&self, context: &Context) -> Result<(), WinitPlatformError> {
        if context.id() == self.binding.id() {
            Ok(())
        } else {
            Err(WinitPlatformError::ContextMismatch)
        }
    }

    pub(super) fn platform_control(&self) -> Result<Rc<WinitPlatformControl>, WinitPlatformError> {
        self.platform
            .upgrade()
            .ok_or(WinitPlatformError::RuntimeDetached)
    }

    pub(crate) fn validate_publication_contract_in_current_context(
        &self,
    ) -> Result<(), WinitPlatformError> {
        validate_platform_callback_contract(self)?;
        let platform_io = unsafe { dear_imgui_rs::sys::igGetPlatformIO_Nil() };
        if platform_io.is_null() {
            return Err(WinitPlatformError::ContextMismatch);
        }
        let monitors_match = self
            .monitor_ownership
            .borrow()
            .as_ref()
            .is_some_and(|ownership| unsafe { ownership.installed_matches(platform_io) });
        if !monitors_match {
            return Err(WinitPlatformError::PlatformStateReplaced {
                field: "PlatformIO.Monitors",
            });
        }
        unsafe { preflight_viewport_ownership(self, platform_io)? };
        Ok(())
    }

    pub(crate) fn owns_any_platform_callback_in_current_context(&self) -> bool {
        has_owned_platform_callback_in_current_context(self)
    }

    pub(super) fn state(&self) -> RuntimeState {
        self.state.get()
    }

    pub(super) fn is_callback_accessible(&self) -> bool {
        matches!(
            self.state.get(),
            RuntimeState::Attached | RuntimeState::ShuttingDown
        )
    }

    pub(crate) fn teardown_callbacks_active(&self) -> bool {
        self.teardown_callbacks_active.get()
    }

    pub(super) fn platform_callback_contract(&self) -> Option<PlatformCallbackContract> {
        self.platform_callback_contract.get()
    }

    fn install_platform_callback_contract(&self, contract: PlatformCallbackContract) {
        debug_assert!(self.platform_callback_contract.get().is_none());
        debug_assert!(self.platform_callback_drift.get().is_none());
        self.platform_callback_contract.set(Some(contract));
    }

    pub(super) fn platform_callback_drift(&self) -> Option<&'static str> {
        self.platform_callback_drift.get()
    }

    pub(super) fn record_platform_callback_drift(&self, callback: &'static str) {
        if self.platform_callback_drift.get().is_none() {
            self.platform_callback_drift.set(Some(callback));
        }
    }

    pub(super) fn clear_platform_callback_contract(&self) {
        self.platform_callback_contract.set(None);
        self.platform_callback_drift.set(None);
    }

    pub(super) fn active_event_loop(&self) -> Option<&ActiveEventLoop> {
        let event_loop = self.event_loop.get();
        if event_loop.is_null() {
            None
        } else {
            // SAFETY: only `with_event_loop` sets this pointer, and its restoration guard clears or
            // restores it before the borrowed ActiveEventLoop can leave the closure.
            Some(unsafe { &*event_loop })
        }
    }

    pub(super) fn record_fault(&self, fault: WinitPlatformError) {
        self.faults.borrow_mut().push_back(QueuedPlatformFault {
            error: fault,
            terminal: false,
        });
    }

    pub(crate) fn mark_faulted(&self) {
        if matches!(
            self.state.get(),
            RuntimeState::Constructing | RuntimeState::Attached
        ) {
            self.state.set(RuntimeState::Faulted);
            self.event_loop.set(std::ptr::null());
        }
    }

    pub(super) fn record_terminal_fault(&self, fault: WinitPlatformError) {
        if self.terminal_fault_recorded.get() {
            return;
        }
        if let Ok(platform) = self.platform_control()
            && platform.terminal_fault().is_some()
        {
            self.terminal_fault_recorded.set(true);
            self.mark_faulted();
            return;
        }
        self.terminal_fault_recorded.set(true);
        self.faults.borrow_mut().push_back(QueuedPlatformFault {
            error: fault.clone(),
            terminal: true,
        });
        self.mark_faulted();
        if let Ok(platform) = self.platform_control() {
            platform.fail_current_contract(fault);
        }
    }

    pub(crate) fn poll_fault(&self) -> Result<(), WinitPlatformError> {
        if let Some(fault) = self.faults.borrow_mut().pop_front() {
            return Err(fault.error);
        }
        if let Ok(platform) = self.platform_control()
            && let Some(fault) = platform.terminal_fault()
        {
            return Err(fault);
        }
        Ok(())
    }

    pub(crate) fn take_retryable_shutdown_fault(&self) -> Option<WinitPlatformError> {
        let mut faults = self.faults.borrow_mut();
        if faults.front().is_some_and(|fault| !fault.terminal) {
            faults.pop_front().map(|fault| fault.error)
        } else {
            None
        }
    }

    pub(crate) fn drain_faults(&self) -> Vec<WinitPlatformError> {
        let queued = self.faults.borrow_mut().drain(..).collect::<Vec<_>>();
        let terminal_was_queued = queued.iter().any(|fault| fault.terminal);
        let mut faults = queued
            .into_iter()
            .map(|fault| fault.error)
            .collect::<Vec<_>>();
        if let Ok(platform) = self.platform_control()
            && let Some(terminal) = platform.terminal_fault()
            && !terminal_was_queued
        {
            faults.push(terminal);
        }
        faults
    }

    fn enter_event_loop<R>(
        &self,
        event_loop: &ActiveEventLoop,
        callback: impl for<'scope> FnOnce(EventLoopScope<'scope>) -> R,
    ) -> R {
        let previous = self
            .event_loop
            .replace(event_loop as *const ActiveEventLoop);
        let _restore = EventLoopRestore {
            control: self,
            previous,
        };
        callback(EventLoopScope {
            event_loop,
            _invariant: PhantomData,
        })
    }

    #[cfg(test)]
    pub(super) fn enter_event_loop_pointer_for_test<R>(
        &self,
        event_loop: *const ActiveEventLoop,
        callback: impl FnOnce() -> R,
    ) -> R {
        let previous = self.event_loop.replace(event_loop);
        let _restore = EventLoopRestore {
            control: self,
            previous,
        };
        callback()
    }

    #[cfg(test)]
    pub(super) fn event_loop_pointer_for_test(&self) -> *const ActiveEventLoop {
        self.event_loop.get()
    }

    fn begin_shutdown(&self) -> bool {
        match self.state.get() {
            RuntimeState::Constructing | RuntimeState::Attached | RuntimeState::Faulted => {
                self.state.set(RuntimeState::ShuttingDown);
                self.event_loop.set(std::ptr::null());
                true
            }
            RuntimeState::ShuttingDown
            | RuntimeState::Detached
            | RuntimeState::ContextDestroyed => false,
        }
    }

    fn finish_shutdown(&self) {
        self.event_loop.set(std::ptr::null());
        self.teardown_callbacks_active.set(false);
        self.core_teardown_owns_callback_guard.set(false);
        self.clear_platform_callback_contract();
        unregister_runtime(self.binding.id());
        self.main_window.borrow_mut().take();
        self.state.set(RuntimeState::Detached);
    }

    fn install_monitor_ownership(&self, ownership: MonitorOwnership) {
        let replaced = self.monitor_ownership.borrow_mut().replace(ownership);
        debug_assert!(replaced.is_none());
    }

    fn restore_monitors_in_current_context(&self) -> Result<(), WinitPlatformError> {
        if unsafe { dear_imgui_rs::sys::igGetCurrentContext() } != self.context_raw {
            return Err(WinitPlatformError::ContextMismatch);
        }
        let Some(ownership) = self.monitor_ownership.borrow_mut().take() else {
            return Ok(());
        };
        unsafe {
            let raw = dear_imgui_rs::sys::igGetPlatformIO_Nil();
            if raw.is_null() {
                ownership.context_destroyed();
            } else {
                ownership.restore_if_owned(raw);
            }
        }
        Ok(())
    }

    fn restore_single_window_io_in_current_context(&self) -> Result<(), WinitPlatformError> {
        if unsafe { dear_imgui_rs::sys::igGetCurrentContext() } != self.context_raw {
            return Err(WinitPlatformError::ContextMismatch);
        }
        let Some(main_window) = self.main_window() else {
            return Ok(());
        };
        let io = unsafe { dear_imgui_rs::sys::igGetIO_ContextPtr(self.context_raw) };
        let Some(io) = (unsafe { io.as_mut() }) else {
            return Err(WinitPlatformError::ContextMismatch);
        };
        let (display_size, framebuffer_scale) = super::single_window_display_metrics(&main_window);
        apply_raw_io_coordinate_contract(io, display_size, framebuffer_scale);
        Ok(())
    }

    fn discard_prior_monitors_after_context_destroyed(&self) {
        if let Some(ownership) = self.monitor_ownership.borrow_mut().take() {
            unsafe { ownership.context_destroyed() };
        }
    }

    fn shutdown_native_with_fault_policy(
        &self,
        report_deferred_fault: bool,
    ) -> Result<(), WinitPlatformError> {
        let callback_error = release_platform_callbacks(self);
        let deferred_fault = if report_deferred_fault {
            self.poll_fault()
        } else {
            Ok(())
        };
        // `DestroyPlatformWindows` has already visited the complete internal viewport list.
        // Residual entries may refer to viewports that were filtered out of the public
        // `PlatformIO.Viewports` snapshot or have since been deleted, so release only their Rust
        // sidecars here and never dereference their retained addresses.
        self.discard_all_viewports_without_touching_native();
        let input_error = self
            .main_window()
            .map(|window| self.retire_window_input(window.id(), None))
            .unwrap_or(Ok(()));
        let monitor_error = self.restore_monitors_in_current_context();
        let io_error = self.restore_single_window_io_in_current_context();
        self.finish_shutdown();
        deferred_fault
            .and(callback_error)
            .and(input_error)
            .and(monitor_error)
            .and(io_error)
    }

    fn shutdown_native(&self) -> Result<(), WinitPlatformError> {
        self.shutdown_native_with_fault_policy(true)
    }

    fn shutdown_native_for_context_drop(&self) -> Result<(), WinitPlatformError> {
        self.shutdown_native_with_fault_policy(false)
    }

    fn shutdown_explicit(&self, context: &mut Context) -> Result<(), WinitPlatformError> {
        let shutdown_result = self
            .binding
            .try_with_bound_context(|| match self.state.get() {
                RuntimeState::Constructing | RuntimeState::Attached | RuntimeState::Faulted => {
                    context.end_frame();
                    preflight_platform_window_destruction(self)?;
                    if !self.begin_shutdown() {
                        return self.poll_fault();
                    }
                    self.teardown_callbacks_active.set(true);
                    let _restore = TeardownCallbackRestore { control: self };
                    context.destroy_platform_windows()?;
                    self.shutdown_native()
                }
                RuntimeState::ShuttingDown => self.poll_fault(),
                RuntimeState::Detached | RuntimeState::ContextDestroyed => Ok(()),
            });
        match shutdown_result {
            Ok(result) => result,
            Err(ContextBindingError::Dropping | ContextBindingError::NativeDestroyed) => {
                // Context-owned teardown either has taken over or has already completed.
                Ok(())
            }
            Err(error) => Err(error.into()),
        }
    }

    pub(crate) fn begin_context_platform_window_teardown(&self) -> Result<(), WinitPlatformError> {
        if self.teardown_callbacks_active() {
            return Ok(());
        }

        match self.state.get() {
            RuntimeState::Attached => {
                preflight_platform_window_destruction(self)?;
                if !self.begin_shutdown() {
                    return self
                        .poll_fault()
                        .and(Err(WinitPlatformError::RuntimeDetached));
                }
                self.teardown_callbacks_active.set(true);
                self.core_teardown_owns_callback_guard.set(true);
                Ok(())
            }
            RuntimeState::Detached | RuntimeState::ContextDestroyed => Ok(()),
            RuntimeState::Constructing | RuntimeState::ShuttingDown | RuntimeState::Faulted => self
                .poll_fault()
                .and(Err(WinitPlatformError::RuntimeDetached)),
        }
    }

    pub(crate) fn finish_context_platform_window_teardown(&self) -> Result<(), WinitPlatformError> {
        if !self.core_teardown_owns_callback_guard.get() {
            return Ok(());
        }

        struct CoreTeardownCallbackRestore<'a> {
            control: &'a RuntimeControl,
        }

        impl Drop for CoreTeardownCallbackRestore<'_> {
            fn drop(&mut self) {
                self.control.teardown_callbacks_active.set(false);
                self.control.core_teardown_owns_callback_guard.set(false);
            }
        }

        let _restore = CoreTeardownCallbackRestore { control: self };
        self.shutdown_native()
    }

    pub(super) fn drop_all_viewports(&self) {
        self.failed_viewports.borrow_mut().clear();
        let entries = std::mem::take(&mut *self.viewports.borrow_mut());
        for entry in entries {
            entry.detach_and_drop();
        }
    }

    fn discard_all_viewports_without_touching_native(&self) {
        // Native viewport pointers may have been filtered from the public snapshot or invalidated
        // by core teardown. Dropping the entries releases only Rust-owned windows and sidecars.
        self.failed_viewports.borrow_mut().clear();
        self.viewports.borrow_mut().clear();
    }

    pub(crate) fn main_window(&self) -> Option<Arc<Window>> {
        self.main_window.borrow().clone()
    }

    fn window_for_id(&self, window_id: WindowId) -> Option<Arc<Window>> {
        self.main_window()
            .filter(|window| window.id() == window_id)
            .or_else(|| {
                secondary_viewport_windows(self)
                    .into_iter()
                    .find(|window| window.id() == window_id)
            })
    }

    pub(crate) fn request_platform_window_focus(&self, window_id: WindowId) {
        let mut state = self.platform_focus.get();
        state.request(window_id, Instant::now());
        self.platform_focus.set(state);
    }

    pub(super) fn cancel_platform_window_focus(&self, window_id: WindowId) {
        let mut state = self.platform_focus.get();
        state.cancel(window_id);
        self.platform_focus.set(state);
    }

    pub(crate) fn platform_window_focus(&self, window_id: WindowId, native_focused: bool) -> bool {
        self.platform_focus
            .get()
            .effective_focus(Instant::now(), window_id, native_focused)
    }

    pub(crate) fn note_key(&self, window_id: WindowId, key: dear_imgui_rs::Key, pressed: bool) {
        self.input_ownership
            .borrow_mut()
            .note_key(window_id, key, pressed);
    }

    pub(crate) fn note_mouse_button(
        &self,
        window_id: WindowId,
        button: dear_imgui_rs::input::MouseButton,
        pressed: bool,
    ) {
        self.input_ownership
            .borrow_mut()
            .note_mouse_button(window_id, button, pressed);
        let mut state = self.mouse_leave.get();
        state.note_button(button, pressed);
        self.mouse_leave.set(state);
    }

    pub(crate) fn note_touch(
        &self,
        window_id: WindowId,
        touch_id: u64,
        phase: winit::event::TouchPhase,
    ) -> Option<crate::events::TouchAction> {
        self.input_ownership
            .borrow_mut()
            .note_touch(window_id, touch_id, phase)
    }

    pub(crate) fn retire_window_input(
        &self,
        window_id: WindowId,
        mouse_handoff: Option<WindowId>,
    ) -> Result<(), WinitPlatformError> {
        if unsafe { dear_imgui_rs::sys::igGetCurrentContext() } != self.context_raw {
            return Err(WinitPlatformError::ContextMismatch);
        }
        let io = unsafe { dear_imgui_rs::sys::igGetIO_Nil() };
        if io.is_null() {
            return Err(WinitPlatformError::ContextMismatch);
        }

        let released = self
            .input_ownership
            .borrow_mut()
            .retire_window(window_id, mouse_handoff);
        let mut mouse_leave = self.mouse_leave.get();
        for key in released.keys {
            unsafe { dear_imgui_rs::sys::ImGuiIO_AddKeyEvent(io, key.into(), false) };
        }
        for button in released.mouse_buttons {
            mouse_leave.note_button(button, false);
            unsafe {
                dear_imgui_rs::sys::ImGuiIO_AddMouseSourceEvent(
                    io,
                    dear_imgui_rs::input::MouseSource::Mouse.into(),
                );
                dear_imgui_rs::sys::ImGuiIO_AddMouseButtonEvent(io, button.into(), false);
            }
        }
        if released.touch {
            unsafe {
                dear_imgui_rs::sys::ImGuiIO_AddMouseSourceEvent(
                    io,
                    dear_imgui_rs::input::MouseSource::TouchScreen.into(),
                );
                dear_imgui_rs::sys::ImGuiIO_AddMouseButtonEvent(
                    io,
                    dear_imgui_rs::input::MouseButton::Left.into(),
                    false,
                );
            }
        }
        self.mouse_leave.set(mouse_leave);
        Ok(())
    }

    pub(crate) fn note_cursor_left(&self) {
        let mut state = self.mouse_leave.get();
        state.note_cursor_left();
        self.mouse_leave.set(state);
    }

    pub(crate) fn note_cursor_available(&self) {
        let mut state = self.mouse_leave.get();
        state.note_cursor_available();
        self.mouse_leave.set(state);
    }

    pub(crate) fn note_window_focus(
        &self,
        window_id: WindowId,
        focused: bool,
        context: &mut Context,
    ) {
        let mut platform_focus = self.platform_focus.get();
        platform_focus.note_native_event(focused);
        self.platform_focus.set(platform_focus);
        if self
            .focus
            .borrow_mut()
            .note_window_focus(window_id, focused)
        {
            context.io_mut().add_focus_event(true);
        }
    }

    pub(crate) fn note_window_geometry(&self, window_id: WindowId, position: bool, size: bool) {
        request_geometry_refresh_for_window(self, window_id, position, size);
    }

    pub(crate) fn reconcile_geometry_state(&self) {
        apply_pending_geometry_refresh(self);
    }

    pub(crate) fn reconcile_input_state(&self, context: &mut Context) {
        let mut owned_windows = HashSet::new();
        if let Some(main_window) = self.main_window() {
            owned_windows.insert(main_window.id());
        }
        owned_windows.extend(
            secondary_viewport_windows(self)
                .into_iter()
                .map(|window| window.id()),
        );

        let now = Instant::now();
        let mut platform_focus = self.platform_focus.get();
        let retry_focus = platform_focus.advance(now, &owned_windows);
        self.platform_focus.set(platform_focus);
        if let Some(window_id) = retry_focus
            && let Some(window) = self.window_for_id(window_id)
            && let Err(error) = focus_and_raise_window(&window)
        {
            self.cancel_platform_window_focus(window_id);
            self.record_fault(error);
        }
        let platform_focus_pending = self
            .platform_focus
            .get()
            .has_pending_for_owned_window(now, &owned_windows);
        let focus_lost = self
            .focus
            .borrow_mut()
            .reconcile_owned_windows(&owned_windows, platform_focus_pending);

        let mut mouse_leave = self.mouse_leave.get();
        if focus_lost {
            self.input_ownership.borrow_mut().clear();
            mouse_leave.note_context_focus_lost();
        }
        let invalidation_due = mouse_leave.take_invalidation_due();
        self.mouse_leave.set(mouse_leave);

        if focus_lost || invalidation_due {
            let io = context.io_mut();
            if focus_lost {
                io.add_focus_event(false);
            }
            if invalidation_due {
                io.add_mouse_pos_event([-f32::MAX, -f32::MAX]);
                io.add_mouse_viewport_event(dear_imgui_rs::Id::default());
            }
        }
    }

    pub(crate) fn apply_cursor_settings(&self, settings: CursorSettings) {
        for window in secondary_viewport_windows(self) {
            settings.apply(&window);
        }
    }

    pub(crate) fn set_ime_allowed(&self, allowed: bool) {
        for window in secondary_viewport_windows(self) {
            window.set_ime_allowed(allowed);
        }
    }

    pub(crate) fn refresh_monitors(&self, context: &Context) -> Result<(), WinitPlatformError> {
        let Some(main_window) = self.main_window() else {
            return Err(WinitPlatformError::RuntimeDetached);
        };
        self.binding.try_with_bound_context(|| {
            let mut ownership = self.monitor_ownership.borrow_mut();
            let Some(ownership) = ownership.as_mut() else {
                return Err(WinitPlatformError::RuntimeDetached);
            };
            if refresh_monitors(context, &main_window, ownership)? {
                super::mvlog("[winit-mv] refreshed monitor topology");
            }
            Ok(())
        })?
    }

    pub(crate) fn monitor_publication_report(
        &self,
    ) -> Result<super::WinitMonitorPublicationReport, WinitPlatformError> {
        if self.state() != RuntimeState::Attached {
            return Err(WinitPlatformError::RuntimeDetached);
        }
        self.monitor_ownership
            .borrow()
            .as_ref()
            .map(MonitorOwnership::report)
            .ok_or(WinitPlatformError::RuntimeDetached)
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn refresh_native_mouse(
        &self,
        context: &mut Context,
    ) -> Result<(), WinitPlatformError> {
        self.binding.try_with_bound_context(|| {
            let mouse = query_native_mouse_state();
            let hovered_viewport = mouse
                .and_then(|state| state.hovered_window)
                .and_then(|window| viewport_id_for_native_window(self, window))
                .unwrap_or_default();
            let app_is_focused = mouse
                .and_then(|state| state.focused_window)
                .and_then(|window| viewport_id_for_native_window(self, window))
                .is_some();

            let io = context.io_mut();
            if app_is_focused && let Some(mouse) = mouse {
                io.add_mouse_pos_event([mouse.position[0] as f32, mouse.position[1] as f32]);
            }
            io.add_mouse_viewport_event(dear_imgui_rs::Id::from(hovered_viewport));
            Ok(())
        })?
    }

    #[cfg(test)]
    pub(super) fn refresh_monitors_for_test(
        &self,
        context: &Context,
        monitors: &[dear_imgui_rs::sys::ImGuiPlatformMonitor],
    ) -> Result<bool, WinitPlatformError> {
        self.binding.try_with_bound_context(|| {
            let mut ownership = self.monitor_ownership.borrow_mut();
            let Some(ownership) = ownership.as_mut() else {
                return Err(WinitPlatformError::RuntimeDetached);
            };
            super::callbacks::refresh_monitors_for_test(context, monitors, ownership)
        })?
    }

    #[cfg(test)]
    pub(super) fn refresh_monitor_snapshots_for_test(
        &self,
        context: &Context,
        snapshots: Option<Vec<crate::native_support::MonitorSnapshot>>,
    ) -> Result<bool, WinitPlatformError> {
        self.binding.try_with_bound_context(|| {
            let mut ownership = self.monitor_ownership.borrow_mut();
            let Some(ownership) = ownership.as_mut() else {
                return Err(WinitPlatformError::RuntimeDetached);
            };
            super::callbacks::refresh_monitor_snapshots_for_test(context, snapshots, ownership)
        })?
    }
}

impl RuntimeControl {
    pub(crate) fn quiesce_from_platform(&self) {
        self.begin_shutdown();
    }

    pub(crate) fn release_from_platform_teardown(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        if !matches!(self.state.get(), RuntimeState::ShuttingDown) {
            return Ok(());
        }

        context
            .with_bound_context(|| {
                preflight_platform_window_destruction(self)?;
                self.teardown_callbacks_active.set(true);
                let _restore = TeardownCallbackRestore { control: self };
                unsafe { dear_imgui_rs::sys::igDestroyPlatformWindows() };
                self.shutdown_native_for_context_drop()
            })
            .map_err(|error| {
                let message = error.to_string();
                self.record_fault(error);
                ContextAttachmentTeardownError::new(message)
            })
    }

    pub(crate) fn context_destroyed_from_platform(&self, _context: ContextDestroyed) {
        self.event_loop.set(std::ptr::null());
        self.teardown_callbacks_active.set(false);
        self.core_teardown_owns_callback_guard.set(false);
        self.clear_platform_callback_contract();
        unregister_runtime(self.binding.id());
        self.discard_all_viewports_without_touching_native();
        self.discard_prior_monitors_after_context_destroyed();
        self.main_window.borrow_mut().take();
        self.state.set(RuntimeState::ContextDestroyed);
    }

    pub(crate) fn shutdown_from_platform(
        &self,
        context: &mut Context,
    ) -> Result<(), WinitPlatformError> {
        if let Some(fault) = self.take_retryable_shutdown_fault() {
            return Err(fault);
        }
        self.shutdown_explicit(context)
    }

    pub(crate) fn is_released(&self) -> bool {
        matches!(
            self.state.get(),
            RuntimeState::Detached | RuntimeState::ContextDestroyed
        )
    }
}

struct EventLoopRestore<'a> {
    control: &'a RuntimeControl,
    previous: *const ActiveEventLoop,
}

impl Drop for EventLoopRestore<'_> {
    fn drop(&mut self) {
        // Dear ImGui clears platform request flags after invoking every callback. Reassert failed
        // viewport closure only after the entire event-loop operation has returned.
        reassert_failed_viewports(self.control);
        self.control.event_loop.set(self.previous);
    }
}

struct TeardownCallbackRestore<'a> {
    control: &'a RuntimeControl,
}

impl Drop for TeardownCallbackRestore<'_> {
    fn drop(&mut self) {
        self.control.teardown_callbacks_active.set(false);
    }
}

#[cfg(test)]
mod viewport_attempt_tests {
    use super::*;

    #[test]
    fn callback_result_and_platform_faults_remain_parallel_outputs() {
        let faults = vec![WinitPlatformError::WindowOperation {
            operation: "test callback",
            message: "platform failed".to_owned(),
        }];
        let attempt = WinitViewportAttempt::completed(Err::<(), _>("renderer failed"), faults);

        let (output, faults) = attempt.into_parts();

        assert_eq!(output, Some(Err("renderer failed")));
        assert_eq!(faults.len(), 1);
        assert!(matches!(
            faults[0],
            WinitPlatformError::WindowOperation {
                operation: "test callback",
                ..
            }
        ));
    }
}
