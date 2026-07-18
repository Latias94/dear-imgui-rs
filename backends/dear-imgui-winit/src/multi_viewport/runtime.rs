use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;

use dear_imgui_rs::{
    Context, ContextAttachment, ContextAttachmentError, ContextAttachmentLease,
    ContextAttachmentRole, ContextBinding, ContextBindingError, ContextDestroyed, ContextTeardown,
};
use thiserror::Error;
use winit::event::Event;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use super::callbacks::{
    claim_platform_callbacks, preflight_platform_callbacks, release_platform_callbacks,
    setup_monitors,
};
use super::registry::{register_runtime, unregister_runtime};
use super::viewport_data::{init_main_viewport, preflight_main_viewport};

struct WinitPlatformAttachmentMarker;

/// Failure to attach or operate a Winit multi-viewport platform runtime.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum WinitPlatformError {
    /// The Dear ImGui Context rejected the platform attachment.
    #[error(transparent)]
    Attachment(#[from] ContextAttachmentError),
    /// The originating Dear ImGui Context can no longer be entered normally.
    #[error(transparent)]
    Context(#[from] ContextBindingError),
    /// The Context passed to an operation is not the runtime's Context.
    #[error("the Winit platform runtime belongs to a different Dear ImGui context")]
    ContextMismatch,
    /// The build artifact lacks the aggregate callback bridge required by this backend.
    #[error("dear-imgui-sys was built without PlatformIO aggregate ABI hooks")]
    AggregateCallbackHooksUnavailable,
    /// Another platform backend already owns one of the required callback slots.
    #[error("ImGuiPlatformIO callback `{callback}` is already owned by another platform backend")]
    PlatformCallbackOccupied { callback: &'static str },
    /// A callback installed by this runtime was replaced while it remained attached.
    #[error("Winit platform callback `{callback}` was replaced while the runtime was attached")]
    PlatformCallbackReplaced { callback: &'static str },
    /// A viewport already has platform data owned by another backend.
    #[error("viewport platform data or handle is already owned by another platform backend")]
    ForeignPlatformUserData,
    /// Dear ImGui requested a new viewport outside a scoped Winit event-loop entry.
    #[error("Winit viewport creation requires WinitPlatformRuntime::with_event_loop")]
    EventLoopUnavailable,
    /// Winit failed to create a secondary viewport window.
    #[error("Winit failed to create a secondary viewport window: {message}")]
    WindowCreation { message: String },
    /// A Rust platform callback panicked; the panic was contained at the C ABI boundary.
    #[error("Winit platform callback `{callback}` panicked")]
    CallbackPanicked { callback: &'static str },
    /// The owning runtime has already shut down.
    #[error("the Winit platform runtime is no longer attached")]
    RuntimeDetached,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RuntimeState {
    Constructing,
    Attached,
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

pub(super) struct RuntimeControl {
    context_raw: *mut dear_imgui_rs::sys::ImGuiContext,
    binding: ContextBinding,
    state: Cell<RuntimeState>,
    event_loop: Cell<*const ActiveEventLoop>,
    teardown_callbacks_active: Cell<bool>,
    fault: RefCell<Option<WinitPlatformError>>,
    prior_backend_flags: dear_imgui_rs::BackendFlags,
    main_window: RefCell<Option<Arc<Window>>>,
    pub(super) viewports: RefCell<Vec<super::registry::ViewportEntry>>,
}

impl RuntimeControl {
    fn new(context: &Context, main_window: Arc<Window>) -> Self {
        Self {
            context_raw: context.as_raw(),
            binding: context.binding(),
            state: Cell::new(RuntimeState::Constructing),
            event_loop: Cell::new(std::ptr::null()),
            teardown_callbacks_active: Cell::new(false),
            fault: RefCell::new(None),
            prior_backend_flags: context.io().backend_flags(),
            main_window: RefCell::new(Some(main_window)),
            viewports: RefCell::new(Vec::new()),
        }
    }

    #[cfg(test)]
    pub(super) fn new_for_test(context: &Context) -> Self {
        Self {
            context_raw: context.as_raw(),
            binding: context.binding(),
            state: Cell::new(RuntimeState::Constructing),
            event_loop: Cell::new(std::ptr::null()),
            teardown_callbacks_active: Cell::new(false),
            fault: RefCell::new(None),
            prior_backend_flags: context.io().backend_flags(),
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

    pub(super) fn state(&self) -> RuntimeState {
        self.state.get()
    }

    pub(super) fn prior_backend_flags(&self) -> dear_imgui_rs::BackendFlags {
        self.prior_backend_flags
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

    fn poll_fault(&self) -> Result<(), WinitPlatformError> {
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
            RuntimeState::Constructing | RuntimeState::Attached => {
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
        unregister_runtime(self.binding.id());
        self.main_window.borrow_mut().take();
        self.state.set(RuntimeState::Detached);
    }

    fn shutdown_native(&self) -> Result<(), WinitPlatformError> {
        let callback_error = release_platform_callbacks(self);
        self.drop_all_viewports();
        self.finish_shutdown();
        callback_error
    }

    fn shutdown_explicit(&self) -> Result<(), WinitPlatformError> {
        if !self.begin_shutdown() {
            return match self.state.get() {
                RuntimeState::Detached | RuntimeState::ContextDestroyed => Ok(()),
                RuntimeState::ShuttingDown => self.poll_fault(),
                RuntimeState::Constructing | RuntimeState::Attached => unreachable!(),
            };
        }

        let shutdown_result = self.binding.try_with_bound_context(|| unsafe {
            dear_imgui_rs::sys::igDestroyPlatformWindows();
            self.shutdown_native()
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

impl ContextAttachment for RuntimeControl {
    fn quiesce(&self, _context: &ContextTeardown<'_>) {
        self.begin_shutdown();
    }

    fn release_platform_windows(&self, context: &ContextTeardown<'_>) {
        if !matches!(self.state.get(), RuntimeState::ShuttingDown) {
            return;
        }

        context.with_bound_context(|| {
            self.teardown_callbacks_active.set(true);
            let _restore = TeardownCallbackRestore { control: self };
            unsafe { dear_imgui_rs::sys::igDestroyPlatformWindows() };
            if let Err(error) = self.shutdown_native() {
                self.record_fault(error);
            }
        });
    }

    fn context_destroyed(&self, _context: ContextDestroyed) {
        self.event_loop.set(std::ptr::null());
        self.teardown_callbacks_active.set(false);
        unregister_runtime(self.binding.id());
        self.drop_all_viewports_after_context_destroyed();
        self.main_window.borrow_mut().take();
        self.state.set(RuntimeState::ContextDestroyed);
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

/// Owning Winit platform runtime for Dear ImGui multi-viewport support.
///
/// The runtime owns the main window, all secondary viewport windows, the platform callback claim,
/// and the Context attachment that orders platform-window teardown after renderer teardown.
pub struct WinitPlatformRuntime {
    control: Rc<RuntimeControl>,
    attachment: Option<ContextAttachmentLease>,
}

impl WinitPlatformRuntime {
    /// Attaches Winit multi-viewport support to `context` and takes shared ownership of the main
    /// application window.
    pub fn new(
        context: &mut Context,
        main_window: Arc<Window>,
    ) -> Result<Self, WinitPlatformError> {
        if !dear_imgui_rs::sys::HAS_PLATFORM_IO_AGGREGATE_HOOKS {
            return Err(WinitPlatformError::AggregateCallbackHooksUnavailable);
        }

        preflight_platform_callbacks(context)?;
        preflight_main_viewport(context)?;

        let control = Rc::new(RuntimeControl::new(context, Arc::clone(&main_window)));
        let mut attachment = context.register_attachment::<WinitPlatformAttachmentMarker>(
            ContextAttachmentRole::Platform,
            Rc::clone(&control) as Rc<dyn ContextAttachment>,
        )?;

        register_runtime(&control);
        if let Err(error) = init_main_viewport(&control, main_window) {
            unregister_runtime(control.binding.id());
            attachment.detach();
            return Err(error);
        }
        claim_platform_callbacks(context);
        setup_monitors(&control, context);
        claim_backend_flags(&control, context);
        control.state.set(RuntimeState::Attached);

        Ok(Self {
            control,
            attachment: Some(attachment),
        })
    }

    #[cfg(test)]
    pub(super) fn new_for_test(context: &mut Context) -> Result<Self, WinitPlatformError> {
        preflight_platform_callbacks(context)?;
        let control = Rc::new(RuntimeControl::new_for_test(context));
        let attachment = context.register_attachment::<WinitPlatformAttachmentMarker>(
            ContextAttachmentRole::Platform,
            Rc::clone(&control) as Rc<dyn ContextAttachment>,
        )?;
        register_runtime(&control);
        claim_platform_callbacks(context);
        claim_backend_flags(&control, context);
        control.state.set(RuntimeState::Attached);
        Ok(Self {
            control,
            attachment: Some(attachment),
        })
    }

    /// Returns the runtime-owned main window.
    pub fn main_window(&self) -> Result<Arc<Window>, WinitPlatformError> {
        self.control
            .main_window()
            .ok_or(WinitPlatformError::RuntimeDetached)
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
        if self.control.state.get() != RuntimeState::Attached {
            return Err(WinitPlatformError::RuntimeDetached);
        }
        let result = self.control.enter_event_loop(event_loop, callback);
        self.poll_fault()?;
        Ok(result)
    }

    /// Returns and clears the oldest pending callback fault.
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
        self.ensure_context(context)?;
        self.poll_fault()?;
        self.ensure_attached()?;
        let consumed = super::events::handle_event(self, platform, context, event);
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
    /// The operation is idempotent. Use this path when shutdown errors need to be reported; Drop
    /// performs the same cleanup on a best-effort basis.
    pub fn shutdown(&mut self) -> Result<(), WinitPlatformError> {
        let pending_fault = self.control.poll_fault().err();
        let result = self.control.shutdown_explicit();
        if !matches!(self.control.state(), RuntimeState::Attached)
            && let Some(mut attachment) = self.attachment.take()
        {
            attachment.detach();
        }
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
        if self.control.state() == RuntimeState::Attached {
            Ok(())
        } else {
            Err(WinitPlatformError::RuntimeDetached)
        }
    }

    pub(super) fn control(&self) -> &Rc<RuntimeControl> {
        &self.control
    }
}

impl Drop for WinitPlatformRuntime {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
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
