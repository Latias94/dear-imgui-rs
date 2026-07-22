use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use dear_imgui_rs::{
    Context, ContextAttachmentTeardownError, ContextBinding, ContextBindingError, ContextDestroyed,
    ContextTeardown,
};
use winit::event::Event;
use winit::event_loop::ActiveEventLoop;
#[cfg(target_os = "linux")]
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

use super::callbacks::{
    MonitorOwnership, PlatformCallbackContract, PreparedMonitors, claim_platform_callbacks,
    has_owned_platform_callback_in_current_context, preflight_platform_callbacks,
    preflight_platform_window_destruction, prepare_monitors, publish_monitors,
    release_platform_callbacks, validate_platform_callback_contract,
};
use super::registry::{preflight_viewport_ownership, register_runtime, unregister_runtime};
use super::viewport_data::{init_main_viewport, preflight_main_viewport};
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
    platform_callback_contract: Cell<Option<PlatformCallbackContract>>,
    platform_callback_drift: Cell<Option<&'static str>>,
    fault: RefCell<Option<WinitPlatformError>>,
    monitor_ownership: RefCell<Option<MonitorOwnership>>,
    main_window: RefCell<Option<Arc<Window>>>,
    pub(super) viewports: RefCell<Vec<super::registry::ViewportEntry>>,
}

impl RuntimeControl {
    fn new(
        context: &Context,
        platform: &Rc<WinitPlatformControl>,
        main_window: Arc<Window>,
    ) -> Self {
        Self {
            context_raw: context.as_raw(),
            binding: context.binding(),
            platform: Rc::downgrade(platform),
            state: Cell::new(RuntimeState::Constructing),
            event_loop: Cell::new(std::ptr::null()),
            teardown_callbacks_active: Cell::new(false),
            platform_callback_contract: Cell::new(None),
            platform_callback_drift: Cell::new(None),
            fault: RefCell::new(None),
            monitor_ownership: RefCell::new(None),
            main_window: RefCell::new(Some(main_window)),
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
            platform_callback_contract: Cell::new(None),
            platform_callback_drift: Cell::new(None),
            fault: RefCell::new(None),
            monitor_ownership: RefCell::new(None),
            main_window: RefCell::new(None),
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

    pub(super) fn teardown_callbacks_active(&self) -> bool {
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

    fn discard_prior_monitors_after_context_destroyed(&self) {
        if let Some(ownership) = self.monitor_ownership.borrow_mut().take() {
            unsafe { ownership.context_destroyed() };
        }
    }

    fn shutdown_native(&self) -> Result<(), WinitPlatformError> {
        let callback_error = release_platform_callbacks(self);
        let deferred_fault = self.poll_fault();
        self.drop_all_viewports();
        let monitor_error = self.restore_monitors_in_current_context();
        self.finish_shutdown();
        deferred_fault.and(callback_error).and(monitor_error)
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
                    context.destroy_platform_windows();
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

    pub(super) fn drop_all_viewports(&self) {
        let entries = std::mem::take(&mut *self.viewports.borrow_mut());
        for entry in entries {
            entry.detach_and_drop();
        }
    }

    fn drop_all_viewports_after_context_destroyed(&self) {
        // Native viewport pointers are invalid now. Dropping the entries releases only their
        // Rust-owned windows and sidecars without dereferencing those pointers.
        self.viewports.borrow_mut().clear();
    }

    pub(super) fn main_window(&self) -> Option<Arc<Window>> {
        self.main_window.borrow().clone()
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
        self.clear_platform_callback_contract();
        unregister_runtime(self.binding.id());
        self.drop_all_viewports_after_context_destroyed();
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
    /// desktop logical coordinates and therefore cannot be mixed without incorrect input and
    /// window geometry.
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
        Self::construct(
            context,
            platform_control,
            control,
            #[cfg(test)]
            None,
            Some(main_window),
            prepared_monitors,
            |_, _| Ok(()),
        )
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
    /// Context defers native cleanup to the Context attachment instead.
    pub fn shutdown(&mut self, context: &mut Context) -> Result<(), WinitPlatformError> {
        self.ensure_context(context)?;
        let pending_fault = self.control.poll_fault().err();
        let result = self.control.shutdown_explicit(context);
        let released = matches!(
            self.control.state(),
            RuntimeState::Detached | RuntimeState::ContextDestroyed
        );
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
        io.set_backend_flags(
            io.backend_flags()
                | dear_imgui_rs::BackendFlags::PLATFORM_HAS_VIEWPORTS
                | dear_imgui_rs::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT,
        );
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
