use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use dear_imgui_rs::{
    Context, ContextAttachmentTeardownError, ContextBinding, ContextBindingError, ContextDestroyed,
    ContextId, ContextTeardown,
};
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
#[cfg(target_os = "windows")]
use super::native_cursor_hittest::query_native_mouse_state;
#[cfg(target_os = "windows")]
use super::registry::viewport_id_for_native_window;
use super::registry::{
    apply_pending_geometry_refresh, preflight_viewport_ownership, register_runtime,
    request_geometry_refresh_for_window, secondary_viewport_windows, unregister_runtime,
};
use super::viewport_data::{init_main_viewport, preflight_main_viewport};
use crate::cursor::CursorSettings;
use crate::platform::{WinitPlatformControl, WinitPlatformError};

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
/// The lifetime is introduced by [`WinitPlatformRuntime::with_event_loop`], so this token and the
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
    fault: RefCell<Option<WinitPlatformError>>,
    monitor_ownership: RefCell<Option<MonitorOwnership>>,
    main_window: RefCell<Option<Arc<Window>>>,
    mouse_leave: Cell<MouseLeaveState>,
    input_ownership: RefCell<InputOwnership>,
    focus: RefCell<ContextFocusState>,
    pub(super) viewports: RefCell<Vec<super::registry::ViewportEntry>>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct MouseLeaveState {
    buttons_down: u8,
    pending: bool,
}

impl MouseLeaveState {
    pub(super) fn note_button(&mut self, button: dear_imgui_rs::input::MouseButton, pressed: bool) {
        let mask = 1_u8 << (button as u8);
        if pressed {
            self.buttons_down |= mask;
        } else {
            self.buttons_down &= !mask;
        }
    }

    pub(super) fn note_cursor_left(&mut self) {
        self.pending = true;
    }

    pub(super) fn note_cursor_available(&mut self) {
        self.pending = false;
    }

    pub(super) fn note_context_focus_lost(&mut self) {
        // Winit may not deliver button releases after the pointer or keyboard focus leaves every
        // window owned by this Context. Keep the delayed-leave state recoverable in that case.
        self.buttons_down = 0;
        self.pending = true;
    }

    pub(super) fn take_invalidation_due(&mut self) -> bool {
        if self.pending && self.buttons_down == 0 {
            self.pending = false;
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct InputOwnership {
    keys: HashMap<dear_imgui_rs::Key, WindowId>,
    mouse_buttons: HashMap<dear_imgui_rs::input::MouseButton, WindowId>,
    touch: Option<(u64, WindowId)>,
}

#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct ReleasedInput {
    pub(super) keys: Vec<dear_imgui_rs::Key>,
    pub(super) mouse_buttons: Vec<dear_imgui_rs::input::MouseButton>,
    pub(super) touch: bool,
}

impl InputOwnership {
    pub(super) fn note_key(&mut self, window_id: WindowId, key: dear_imgui_rs::Key, pressed: bool) {
        if pressed {
            self.keys.insert(key, window_id);
        } else {
            self.keys.remove(&key);
        }
    }

    pub(super) fn note_mouse_button(
        &mut self,
        window_id: WindowId,
        button: dear_imgui_rs::input::MouseButton,
        pressed: bool,
    ) {
        if pressed {
            self.mouse_buttons.insert(button, window_id);
        } else {
            self.mouse_buttons.remove(&button);
        }
    }

    pub(super) fn note_touch(
        &mut self,
        window_id: WindowId,
        touch_id: u64,
        phase: winit::event::TouchPhase,
    ) -> Option<crate::events::TouchAction> {
        let active_id = self.touch.map(|(touch_id, _)| touch_id);
        let (next_active, action) = crate::events::touch_transition(active_id, touch_id, phase);
        match action {
            Some(crate::events::TouchAction::Press) => {
                self.touch = next_active.map(|touch_id| (touch_id, window_id));
            }
            Some(crate::events::TouchAction::Release) => self.touch = None,
            Some(crate::events::TouchAction::Move) | None => {}
        }
        action
    }

    pub(super) fn retire_window(
        &mut self,
        window_id: WindowId,
        mouse_handoff: Option<WindowId>,
    ) -> ReleasedInput {
        let mut released = ReleasedInput::default();
        self.keys.retain(|key, owner| {
            if *owner == window_id {
                released.keys.push(*key);
                false
            } else {
                true
            }
        });
        self.mouse_buttons.retain(|button, owner| {
            if *owner == window_id {
                if let Some(mouse_handoff) = mouse_handoff {
                    *owner = mouse_handoff;
                    true
                } else {
                    released.mouse_buttons.push(*button);
                    false
                }
            } else {
                true
            }
        });
        if self.touch.is_some_and(|(_, owner)| owner == window_id) {
            if let Some(mouse_handoff) = mouse_handoff {
                if let Some((_, owner)) = self.touch.as_mut() {
                    *owner = mouse_handoff;
                }
            } else {
                self.touch = None;
                released.touch = true;
            }
        }
        released
    }

    fn clear(&mut self) {
        self.keys.clear();
        self.mouse_buttons.clear();
        self.touch = None;
    }
}

#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct ContextFocusState {
    focused_windows: HashSet<WindowId>,
    context_focused: bool,
    focus_loss_pending: bool,
}

impl ContextFocusState {
    pub(super) fn with_focused_window(window_id: Option<WindowId>) -> Self {
        // Dear ImGui treats a newly attached platform as focused until it receives an explicit
        // loss event. Start from that reported state even when Winit says the main window is
        // already unfocused, then reconcile the empty set at the next platform-frame boundary.
        let mut state = Self {
            context_focused: true,
            ..Self::default()
        };
        if let Some(window_id) = window_id {
            state.focused_windows.insert(window_id);
        }
        state
    }

    /// Records a native focus event and returns whether Dear ImGui needs a focus-gained event.
    pub(super) fn note_window_focus(&mut self, window_id: WindowId, focused: bool) -> bool {
        if focused {
            self.focused_windows.insert(window_id);
            self.focus_loss_pending = false;
            if !self.context_focused {
                self.context_focused = true;
                return true;
            }
        } else if self.focused_windows.remove(&window_id)
            && self.focused_windows.is_empty()
            && self.context_focused
        {
            // Focus transfers between native viewports commonly report the old window losing
            // focus before the new one gains it. Defer the Context-level loss until the next
            // platform-frame boundary so that transfer can cancel it.
            self.focus_loss_pending = true;
        }
        false
    }

    /// Reconciles destroyed windows and returns whether the Context has now lost focus.
    pub(super) fn reconcile_owned_windows(&mut self, owned_windows: &HashSet<WindowId>) -> bool {
        self.focused_windows
            .retain(|window_id| owned_windows.contains(window_id));
        if self.context_focused && self.focused_windows.is_empty() {
            self.focus_loss_pending = true;
        }
        if self.focus_loss_pending && self.focused_windows.is_empty() {
            self.focus_loss_pending = false;
            self.context_focused = false;
            true
        } else {
            false
        }
    }
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
            fault: RefCell::new(None),
            monitor_ownership: RefCell::new(None),
            main_window: RefCell::new(Some(main_window)),
            mouse_leave: Cell::new(MouseLeaveState::default()),
            input_ownership: RefCell::new(InputOwnership::default()),
            focus: RefCell::new(focus),
            viewports: RefCell::new(Vec::new()),
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
            fault: RefCell::new(None),
            monitor_ownership: RefCell::new(None),
            main_window: RefCell::new(None),
            mouse_leave: Cell::new(MouseLeaveState::default()),
            input_ownership: RefCell::new(InputOwnership::default()),
            focus: RefCell::new(ContextFocusState::default()),
            viewports: RefCell::new(Vec::new()),
        }
    }

    pub(super) fn context_raw(&self) -> *mut dear_imgui_rs::sys::ImGuiContext {
        self.context_raw
    }

    pub(super) fn binding(&self) -> &ContextBinding {
        &self.binding
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
        let mut slot = self.fault.borrow_mut();
        if slot.is_none() {
            *slot = Some(fault);
        }
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
        self.mark_faulted();
        if let Ok(platform) = self.platform_control() {
            platform.fail_current_contract(fault);
        }
    }

    fn poll_fault(&self) -> Result<(), WinitPlatformError> {
        if let Ok(platform) = self.platform_control()
            && let Some(fault) = platform.terminal_fault()
        {
            return Err(fault);
        }
        match self.fault.borrow_mut().take() {
            Some(fault) => Err(fault),
            None => Ok(()),
        }
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

    fn shutdown_native(&self) -> Result<(), WinitPlatformError> {
        let callback_error = release_platform_callbacks(self);
        let deferred_fault = self.poll_fault();
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
        let entries = std::mem::take(&mut *self.viewports.borrow_mut());
        for entry in entries {
            entry.detach_and_drop();
        }
    }

    fn discard_all_viewports_without_touching_native(&self) {
        // Native viewport pointers may have been filtered from the public snapshot or invalidated
        // by core teardown. Dropping the entries releases only Rust-owned windows and sidecars.
        self.viewports.borrow_mut().clear();
    }

    pub(super) fn main_window(&self) -> Option<Arc<Window>> {
        self.main_window.borrow().clone()
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
        let focus_lost = self
            .focus
            .borrow_mut()
            .reconcile_owned_windows(&owned_windows);

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
                self.shutdown_native()
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

struct RuntimeConstruction<'a> {
    context: &'a mut Context,
    platform: Rc<WinitPlatformControl>,
    control: Rc<RuntimeControl>,
    #[cfg(test)]
    owned_platform: Option<crate::WinitPlatform>,
    prepared_monitors: Option<PreparedMonitors>,
    runtime_installed: bool,
    runtime_registered: bool,
    main_viewport_initialized: bool,
    callbacks_claimed: bool,
    committed: bool,
}

impl<'a> RuntimeConstruction<'a> {
    fn new(
        context: &'a mut Context,
        platform: Rc<WinitPlatformControl>,
        control: Rc<RuntimeControl>,
        #[cfg(test)] owned_platform: Option<crate::WinitPlatform>,
        prepared_monitors: PreparedMonitors,
    ) -> Self {
        Self {
            context,
            platform,
            control,
            #[cfg(test)]
            owned_platform,
            prepared_monitors: Some(prepared_monitors),
            runtime_installed: false,
            runtime_registered: false,
            main_viewport_initialized: false,
            callbacks_claimed: false,
            committed: false,
        }
    }

    fn commit(mut self) -> Result<WinitPlatformRuntime, WinitPlatformError> {
        self.control.state.set(RuntimeState::Attached);
        self.committed = true;
        Ok(WinitPlatformRuntime {
            control: Rc::clone(&self.control),
            platform: Rc::clone(&self.platform),
            #[cfg(test)]
            owned_platform: self.owned_platform.take(),
        })
    }
}

impl Drop for RuntimeConstruction<'_> {
    fn drop(&mut self) {
        if self.committed {
            return;
        }

        self.control.binding.with_bound_context(|| {
            let _ = self.control.restore_monitors_in_current_context();
            if self.callbacks_claimed {
                let _ = release_platform_callbacks(&self.control);
            }
            if self.main_viewport_initialized {
                self.control.drop_all_viewports();
            }
        });
        if self.runtime_registered {
            unregister_runtime(self.control.binding.id());
        }
        if self.runtime_installed {
            self.platform.clear_runtime(&self.control);
        }
        self.control.main_window.borrow_mut().take();
        self.control.state.set(RuntimeState::Detached);
    }
}

/// Owning Winit platform runtime for Dear ImGui multi-viewport support.
///
/// The runtime shares the `WinitPlatform`'s Context-bound main-window owner, and owns secondary
/// viewport windows plus the platform callback claim. It does not install a second platform
/// attachment; Context teardown reaches it through the platform owner's single attachment.
/// Calling [`Context::destroy_platform_windows`] directly also shuts this runtime down. The base
/// Winit platform remains attached for single-window use; create a new runtime before resuming
/// multi-viewport work. Prefer [`Self::shutdown`] when the caller needs backend-specific errors.
pub struct WinitPlatformRuntime {
    control: Rc<RuntimeControl>,
    platform: Rc<WinitPlatformControl>,
    #[cfg(test)]
    // Test construction has no native Window, so the runtime keeps its synthetic base platform
    // owner alive and tears it down after the multi-viewport contract.
    owned_platform: Option<crate::WinitPlatform>,
}

impl WinitPlatformRuntime {
    /// Attaches Winit multi-viewport support to the already attached platform main window.
    ///
    /// The platform must use [`crate::HiDpiMode::Default`]. Locked and rounded modes remap the
    /// single-window coordinate space, while Winit's native platform-window callbacks operate in
    /// platform-native desktop coordinates and therefore cannot be mixed without incorrect input
    /// and window geometry.
    pub fn new(
        context: &mut Context,
        platform: &crate::WinitPlatform,
    ) -> Result<Self, WinitPlatformError> {
        if !dear_imgui_rs::sys::HAS_PLATFORM_IO_AGGREGATE_HOOKS {
            return Err(WinitPlatformError::AggregateCallbackHooksUnavailable);
        }
        validate_multi_viewport_hidpi_mode(platform.hidpi_mode())?;
        let platform_control = platform.control();
        platform_control.ensure_context(context)?;
        let main_window = platform_control.attached_window()?;
        platform_control.validate_operational_contract()?;

        preflight_window_system(&main_window)?;
        let prepared_monitors = prepare_monitors(context, &main_window)?;
        preflight_platform_callbacks(context)?;
        preflight_main_viewport(context)?;

        let control = Rc::new(RuntimeControl::new(
            context,
            &platform_control,
            Arc::clone(&main_window),
        ));
        let runtime = Self::construct(
            context,
            platform_control,
            control,
            #[cfg(test)]
            None,
            Some(Arc::clone(&main_window)),
            prepared_monitors,
            |_, _| Ok(()),
        )?;
        let io = context.io_mut();
        io.set_display_size(super::desktop_size_for_window(&main_window));
        io.set_display_framebuffer_scale(super::framebuffer_scale_for_window(&main_window));
        invalidate_mouse_coordinate_cache(io);
        Ok(runtime)
    }

    #[cfg(test)]
    pub(super) fn new_for_test(context: &mut Context) -> Result<Self, WinitPlatformError> {
        Self::new_for_test_with(context, vec![test_monitor()], |_, _| Ok(()))
    }

    #[cfg(test)]
    pub(super) fn new_for_test_with_platform(
        context: &mut Context,
        platform: &crate::WinitPlatform,
    ) -> Result<Self, WinitPlatformError> {
        let platform_control = platform.control();
        platform_control.ensure_context(context)?;
        platform_control.validate_operational_contract()?;
        let prepared_monitors =
            super::callbacks::prepare_monitors_for_test(context, vec![test_monitor()])?;
        preflight_platform_callbacks(context)?;
        preflight_main_viewport(context)?;
        let control = Rc::new(RuntimeControl::new_for_test(context, &platform_control));
        Self::construct(
            context,
            platform_control,
            control,
            None,
            None,
            prepared_monitors,
            |_, _| Ok(()),
        )
    }

    #[cfg(test)]
    pub(super) fn new_for_test_with(
        context: &mut Context,
        monitors: Vec<dear_imgui_rs::sys::ImGuiPlatformMonitor>,
        checkpoint: impl FnMut(ConstructionStage, &mut Context) -> Result<(), WinitPlatformError>,
    ) -> Result<Self, WinitPlatformError> {
        let platform = crate::WinitPlatform::new(context)?;
        let platform_control = platform.control();
        let prepared_monitors = super::callbacks::prepare_monitors_for_test(context, monitors)?;
        preflight_platform_callbacks(context)?;
        preflight_main_viewport(context)?;
        let control = Rc::new(RuntimeControl::new_for_test(context, &platform_control));
        Self::construct(
            context,
            platform_control,
            control,
            Some(platform),
            None,
            prepared_monitors,
            checkpoint,
        )
    }

    fn construct(
        context: &mut Context,
        platform: Rc<WinitPlatformControl>,
        control: Rc<RuntimeControl>,
        #[cfg(test)] owned_platform: Option<crate::WinitPlatform>,
        main_window: Option<Arc<Window>>,
        prepared_monitors: PreparedMonitors,
        mut checkpoint: impl FnMut(ConstructionStage, &mut Context) -> Result<(), WinitPlatformError>,
    ) -> Result<Self, WinitPlatformError> {
        let mut transaction = RuntimeConstruction::new(
            context,
            platform,
            control,
            #[cfg(test)]
            owned_platform,
            prepared_monitors,
        );

        transaction
            .platform
            .install_runtime(Rc::clone(&transaction.control))?;
        transaction.runtime_installed = true;
        checkpoint(ConstructionStage::Attachment, transaction.context)?;

        register_runtime(&transaction.control);
        transaction.runtime_registered = true;
        checkpoint(ConstructionStage::Registry, transaction.context)?;

        if let Some(main_window) = main_window {
            init_main_viewport(&transaction.control, main_window)?;
            transaction.main_viewport_initialized = true;
        }
        checkpoint(ConstructionStage::MainViewport, transaction.context)?;

        let callback_contract = claim_platform_callbacks(transaction.context);
        transaction
            .control
            .install_platform_callback_contract(callback_contract);
        transaction.callbacks_claimed = true;
        checkpoint(ConstructionStage::Callbacks, transaction.context)?;

        let prepared_monitors = transaction
            .prepared_monitors
            .take()
            .expect("monitor storage is present until publication");
        let ownership = publish_monitors(transaction.context, prepared_monitors);
        transaction.control.install_monitor_ownership(ownership);
        checkpoint(ConstructionStage::Monitors, transaction.context)?;

        claim_backend_flags(&transaction.control, transaction.context);
        checkpoint(ConstructionStage::BackendFlags, transaction.context)?;

        transaction.commit()
    }

    /// Returns the runtime-owned main window.
    pub fn main_window(&self) -> Result<Arc<Window>, WinitPlatformError> {
        self.control
            .main_window()
            .ok_or(WinitPlatformError::RuntimeDetached)
    }

    /// Returns the Dear ImGui Context identity owned by this platform runtime.
    pub fn context_id(&self) -> ContextId {
        self.control.binding.id()
    }

    /// Validates that this runtime still owns the active Winit viewport platform contract.
    ///
    /// Renderer backends use this before interpreting `PlatformHandle` values as Winit windows.
    /// A runtime that has shut down cannot validate even if another platform later attaches to
    /// the same Context.
    pub fn validate_renderer_owner(&self, context: &Context) -> Result<(), WinitPlatformError> {
        self.ensure_context(context)?;
        self.poll_fault()?;
        self.ensure_attached()
    }

    #[cfg(test)]
    pub(super) fn owned_platform_for_test_mut(&mut self) -> &mut crate::WinitPlatform {
        self.owned_platform
            .as_mut()
            .expect("test runtimes always retain their synthetic platform owner")
    }

    /// Runs `callback` while viewport callbacks may access `event_loop`.
    ///
    /// Nested scopes restore the outer event loop. Any platform callback fault is returned only
    /// after the Rust closure regains control, so no unwind crosses the native callback boundary.
    ///
    /// ```compile_fail
    /// use dear_imgui_winit::multi_viewport::WinitPlatformRuntime;
    /// use winit::event_loop::ActiveEventLoop;
    ///
    /// fn leak_event_loop<'a>(
    ///     runtime: &WinitPlatformRuntime,
    ///     event_loop: &'a ActiveEventLoop,
    /// ) -> &'a ActiveEventLoop {
    ///     runtime
    ///         .with_event_loop(event_loop, |scope| scope.active_event_loop())
    ///         .unwrap()
    /// }
    /// ```
    pub fn with_event_loop<R>(
        &self,
        event_loop: &ActiveEventLoop,
        callback: impl for<'scope> FnOnce(EventLoopScope<'scope>) -> R,
    ) -> Result<R, WinitPlatformError> {
        self.poll_fault()?;
        self.ensure_attached()?;
        let result = self.control.enter_event_loop(event_loop, callback);
        self.poll_fault()?;
        Ok(result)
    }

    /// Returns and clears the oldest retryable callback fault.
    ///
    /// Contract drift and callback panics are terminal and remain observable until shutdown.
    pub fn poll_fault(&self) -> Result<(), WinitPlatformError> {
        self.control.poll_fault()
    }

    /// Routes a Winit event to the main window and any Dear ImGui secondary viewport.
    pub fn handle_event<T>(
        &self,
        platform: &mut crate::WinitPlatform,
        context: &mut Context,
        event: &Event<T>,
    ) -> Result<bool, WinitPlatformError> {
        validate_multi_viewport_hidpi_mode(platform.hidpi_mode())?;
        self.ensure_context(context)?;
        self.poll_fault()?;
        self.ensure_attached()?;
        let consumed = super::events::handle_event(self, platform, context, event)?;
        self.poll_fault()?;
        Ok(consumed)
    }

    /// Routes a Winit event only to Dear ImGui-created secondary viewports.
    pub fn route_secondary_event<T>(
        &self,
        context: &mut Context,
        event: &Event<T>,
    ) -> Result<bool, WinitPlatformError> {
        self.ensure_context(context)?;
        self.poll_fault()?;
        self.ensure_attached()?;
        let consumed = super::events::route_secondary_event(&self.control, context, event);
        self.poll_fault()?;
        Ok(consumed)
    }

    /// Explicitly releases platform callbacks and windows.
    ///
    /// The operation is idempotent. The explicit Context lets the core close an open frame before
    /// any platform callback or native window state is released. Dropping the runtime without a
    /// Context defers native cleanup to the Context attachment instead. An active renderer
    /// attachment rejects shutdown before the frame or native state changes.
    pub fn shutdown(&mut self, context: &mut Context) -> Result<(), WinitPlatformError> {
        self.ensure_context(context)?;
        if matches!(
            self.control.state(),
            RuntimeState::Detached | RuntimeState::ContextDestroyed
        ) {
            return Ok(());
        }
        let attachment = self.platform.attachment_handle()?;
        let (pending_fault, result, released) = {
            let mut release = context.prepare_platform_attachment_release(&attachment)?;
            let context = release.context_mut();
            let pending_fault = self.control.poll_fault().err();
            let result = self.control.shutdown_explicit(context);
            let released = matches!(
                self.control.state(),
                RuntimeState::Detached | RuntimeState::ContextDestroyed
            );
            (pending_fault, result, released)
        };
        if released {
            self.platform.clear_runtime(&self.control);
        }
        #[cfg(test)]
        let result = if released {
            if let Some(platform) = self.owned_platform.as_mut() {
                let platform_result = platform.shutdown(context);
                match (result, platform_result) {
                    (Err(primary), Err(secondary)) => {
                        self.control.record_fault(secondary);
                        Err(primary)
                    }
                    (Ok(()), platform_result) => platform_result,
                    (result, Ok(())) => result,
                }
            } else {
                result
            }
        } else {
            result
        };
        match (pending_fault, result) {
            (Some(fault), Err(shutdown_error)) => {
                self.control.record_fault(shutdown_error);
                Err(fault)
            }
            (Some(fault), Ok(())) => Err(fault),
            (None, result) => result,
        }
    }

    fn ensure_context(&self, context: &Context) -> Result<(), WinitPlatformError> {
        if context.id() == self.control.binding.id() {
            Ok(())
        } else {
            Err(WinitPlatformError::ContextMismatch)
        }
    }

    fn ensure_attached(&self) -> Result<(), WinitPlatformError> {
        if self.control.state() != RuntimeState::Attached {
            return self
                .control
                .poll_fault()
                .and(Err(WinitPlatformError::RuntimeDetached));
        }
        self.control
            .platform_control()?
            .validate_operational_contract()
    }

    pub(super) fn control(&self) -> &Rc<RuntimeControl> {
        &self.control
    }
}

fn validate_window_system(
    is_supported_desktop: bool,
    is_wayland: bool,
) -> Result<(), WinitPlatformError> {
    if !is_supported_desktop {
        return Err(WinitPlatformError::UnsupportedWindowSystem {
            target: std::env::consts::OS,
        });
    }
    if is_wayland {
        Err(WinitPlatformError::WaylandUnsupported)
    } else {
        Ok(())
    }
}

pub(super) fn validate_multi_viewport_hidpi_mode(
    mode: crate::HiDpiMode,
) -> Result<(), WinitPlatformError> {
    if mode == crate::HiDpiMode::Default {
        Ok(())
    } else {
        Err(WinitPlatformError::CustomHiDpiModeUnsupported)
    }
}

#[cfg(target_os = "linux")]
fn preflight_window_system(window: &Window) -> Result<(), WinitPlatformError> {
    let handle = window
        .window_handle()
        .map_err(|error| WinitPlatformError::WindowOperation {
            operation: "query raw window handle",
            message: error.to_string(),
        })?
        .as_raw();
    validate_window_system(true, matches!(handle, RawWindowHandle::Wayland(_)))
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn preflight_window_system(_window: &Window) -> Result<(), WinitPlatformError> {
    validate_window_system(true, false)
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn preflight_window_system(_window: &Window) -> Result<(), WinitPlatformError> {
    validate_window_system(false, false)
}

#[cfg(test)]
pub(super) fn validate_window_system_for_test(
    is_supported_desktop: bool,
    is_wayland: bool,
) -> Result<(), WinitPlatformError> {
    validate_window_system(is_supported_desktop, is_wayland)
}

fn claim_backend_flags(control: &RuntimeControl, context: &mut Context) {
    control.binding().with_bound_context(|| {
        let io = context.io_mut();
        io.set_backend_flags(io.backend_flags() | crate::platform::WINIT_VIEWPORT_FLAGS);
    });
}

#[cfg(test)]
fn test_monitor() -> dear_imgui_rs::sys::ImGuiPlatformMonitor {
    dear_imgui_rs::sys::ImGuiPlatformMonitor {
        MainPos: dear_imgui_rs::sys::ImVec2 { x: 0.0, y: 0.0 },
        MainSize: dear_imgui_rs::sys::ImVec2 {
            x: 1920.0,
            y: 1080.0,
        },
        WorkPos: dear_imgui_rs::sys::ImVec2 { x: 0.0, y: 0.0 },
        WorkSize: dear_imgui_rs::sys::ImVec2 {
            x: 1920.0,
            y: 1040.0,
        },
        DpiScale: 1.0,
        PlatformHandle: std::ptr::null_mut(),
    }
}
