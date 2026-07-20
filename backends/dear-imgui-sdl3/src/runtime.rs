use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::{Rc, Weak};

#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
use dear_imgui_rs::RendererConsumer;
use dear_imgui_rs::{
    Context, ContextAttachment, ContextAttachmentLease, ContextAttachmentRole, ContextBinding,
    ContextDestroyed, ContextLifecycle, ContextTeardown, sys,
};

use crate::callback_ownership::{
    PlatformCallbackOwnership, PlatformClaimBaseline, ViewportPlatformState,
    preflight_platform_claim, restore_baseline_after_failed_initialization,
};
use crate::core::{Sdl3BackendError, shutdown_platform_impl};

struct Sdl3PlatformAttachmentMarker;
struct Sdl3RendererAttachmentMarker;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RuntimeState {
    Attached,
    ShuttingDown,
    Detached,
    ResourceDropped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeFault {
    CallbackReplaced(&'static str),
    PlatformStateReplaced(&'static str),
    CallbackPanicked(&'static str),
    ForeignPlatformUserData,
    ViewportCreationFailed,
    ShutdownPanicked(&'static str),
}

impl RuntimeFault {
    fn into_error(self) -> Sdl3BackendError {
        match self {
            Self::CallbackReplaced(callback) => {
                Sdl3BackendError::PlatformCallbackReplaced { callback }
            }
            Self::PlatformStateReplaced(field) => Sdl3BackendError::PlatformStateReplaced { field },
            Self::CallbackPanicked(callback) => {
                Sdl3BackendError::PlatformCallbackPanicked { callback }
            }
            Self::ForeignPlatformUserData => Sdl3BackendError::ForeignPlatformUserData,
            Self::ViewportCreationFailed => Sdl3BackendError::ViewportCreationFailed,
            Self::ShutdownPanicked(phase) => Sdl3BackendError::ShutdownPanicked { phase },
        }
    }
}

struct NativeLifecycle {
    renderer_shutdown: Option<Rc<dyn Fn()>>,
    platform_shutdown: Rc<dyn Fn()>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReleaseState {
    Pending,
    InProgress,
    Released,
}

impl ReleaseState {
    fn is_released(self) -> bool {
        self == Self::Released
    }
}

struct ReleaseGuard<'a> {
    state: &'a Cell<ReleaseState>,
}

impl<'a> ReleaseGuard<'a> {
    fn begin(state: &'a Cell<ReleaseState>) -> Option<Self> {
        match state.get() {
            ReleaseState::Pending => {
                state.set(ReleaseState::InProgress);
                Some(Self { state })
            }
            ReleaseState::InProgress | ReleaseState::Released => None,
        }
    }

    fn commit(self) {
        self.state.set(ReleaseState::Released);
    }
}

impl Drop for ReleaseGuard<'_> {
    fn drop(&mut self) {
        if self.state.get() == ReleaseState::InProgress {
            self.state.set(ReleaseState::Pending);
        }
    }
}

impl fmt::Debug for NativeLifecycle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeLifecycle")
            .field("has_renderer", &self.renderer_shutdown.is_some())
            .finish_non_exhaustive()
    }
}

pub(super) struct RuntimeControl {
    binding: ContextBinding,
    state: Cell<RuntimeState>,
    platform_initialized: Cell<bool>,
    renderer_initialized: Cell<bool>,
    renderer_release: Cell<ReleaseState>,
    platform_release: Cell<ReleaseState>,
    callback_teardown_active: Cell<bool>,
    platform_io_key: Cell<usize>,
    lifecycle: NativeLifecycle,
    callbacks: RefCell<Option<PlatformCallbackOwnership>>,
    owned_viewports: RefCell<HashMap<usize, ViewportPlatformState>>,
    faults: RefCell<VecDeque<RuntimeFault>>,
    reported_replacements: RefCell<HashSet<&'static str>>,
    #[cfg(test)]
    phase_log: RefCell<Vec<&'static str>>,
}

impl fmt::Debug for RuntimeControl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeControl")
            .field("context", &self.binding.id())
            .field("state", &self.state.get())
            .field("platform_initialized", &self.platform_initialized.get())
            .field("renderer_initialized", &self.renderer_initialized.get())
            .field("renderer_released", &self.renderer_released())
            .field("platform_released", &self.platform_released())
            .finish_non_exhaustive()
    }
}

impl RuntimeControl {
    fn new(
        context: &Context,
        renderer_shutdown: Option<Rc<dyn Fn()>>,
        platform_shutdown: Rc<dyn Fn()>,
    ) -> Self {
        Self {
            binding: context.binding(),
            state: Cell::new(RuntimeState::Attached),
            platform_initialized: Cell::new(false),
            renderer_initialized: Cell::new(false),
            renderer_release: Cell::new(if renderer_shutdown.is_none() {
                ReleaseState::Released
            } else {
                ReleaseState::Pending
            }),
            platform_release: Cell::new(ReleaseState::Pending),
            callback_teardown_active: Cell::new(false),
            platform_io_key: Cell::new(0),
            lifecycle: NativeLifecycle {
                renderer_shutdown,
                platform_shutdown,
            },
            callbacks: RefCell::new(None),
            owned_viewports: RefCell::new(HashMap::new()),
            faults: RefCell::new(VecDeque::new()),
            reported_replacements: RefCell::new(HashSet::new()),
            #[cfg(test)]
            phase_log: RefCell::new(Vec::new()),
        }
    }

    pub(super) fn binding(&self) -> &ContextBinding {
        &self.binding
    }

    pub(super) fn state(&self) -> RuntimeState {
        self.state.get()
    }

    fn begin_shutdown(&self) {
        if self.state.get() == RuntimeState::Attached {
            self.state.set(RuntimeState::ShuttingDown);
        }
    }

    fn finish_shutdown(&self) {
        if self.renderer_released()
            && self.platform_released()
            && self.state.get() != RuntimeState::ResourceDropped
        {
            self.state.set(RuntimeState::Detached);
        }
    }

    fn renderer_released(&self) -> bool {
        self.renderer_release.get().is_released()
    }

    fn platform_released(&self) -> bool {
        self.platform_release.get().is_released()
    }

    fn release_renderer_bound(&self) -> bool {
        if self.renderer_released() {
            return true;
        }
        let Some(release) = ReleaseGuard::begin(&self.renderer_release) else {
            return false;
        };
        #[cfg(test)]
        self.phase_log.borrow_mut().push("renderer");
        if self.renderer_initialized.get()
            && let Some(shutdown) = &self.lifecycle.renderer_shutdown
        {
            shutdown();
        }
        release.commit();
        self.renderer_initialized.set(false);
        self.finish_shutdown();
        true
    }

    fn release_platform_bound(&self) -> Result<(), Sdl3BackendError> {
        if self.platform_released() {
            self.finish_shutdown();
            return Ok(());
        }
        let Some(release) = ReleaseGuard::begin(&self.platform_release) else {
            return Ok(());
        };
        #[cfg(test)]
        self.phase_log.borrow_mut().push("platform");

        if !self.platform_initialized.get() {
            release.commit();
            self.finish_shutdown();
            return Ok(());
        }

        let restore = {
            let callbacks = self.callbacks.borrow();
            callbacks
                .as_ref()
                .map(|callbacks| unsafe { callbacks.prepare_shutdown(self) })
                .transpose()?
        };
        self.callback_teardown_active.set(true);
        struct CallbackTeardownGuard<'a>(&'a Cell<bool>);
        impl Drop for CallbackTeardownGuard<'_> {
            fn drop(&mut self) {
                self.0.set(false);
            }
        }
        let callback_guard = CallbackTeardownGuard(&self.callback_teardown_active);
        (self.lifecycle.platform_shutdown)();
        drop(callback_guard);

        // Native shutdown is the irreversible boundary: never call it twice,
        // even if restoring foreign callback state reports an error.
        release.commit();
        self.platform_initialized.set(false);
        let restore_result = if let Some(restore) = restore {
            let callbacks = self.callbacks.borrow();
            unsafe {
                callbacks
                    .as_ref()
                    .expect("initialized SDL3 runtime lost its callback claim")
                    .restore_after_shutdown(restore)
            }
        } else {
            Ok(())
        };
        unregister_runtime(self.platform_io_key.replace(0));
        self.callbacks.borrow_mut().take();
        self.owned_viewports.borrow_mut().clear();
        self.finish_shutdown();
        restore_result
    }

    fn release_renderer_explicit(&self) -> Result<(), Sdl3BackendError> {
        if self.renderer_released() {
            return Ok(());
        }
        let result = self.binding.try_with_bound_context(|| {
            catch_unwind(AssertUnwindSafe(|| self.release_renderer_bound()))
        })?;
        match result {
            Ok(true) => Ok(()),
            Ok(false) => Err(Sdl3BackendError::ShutdownInProgress {
                phase: "renderer resources",
            }),
            Err(_) => Err(Sdl3BackendError::ShutdownPanicked {
                phase: "renderer resources",
            }),
        }
    }

    fn release_platform_explicit(&self) -> Result<(), Sdl3BackendError> {
        if self.platform_released() {
            self.finish_shutdown();
            return Ok(());
        }
        let result = self.binding.try_with_bound_context(|| {
            catch_unwind(AssertUnwindSafe(|| self.release_platform_bound()))
        })?;
        result.unwrap_or(Err(Sdl3BackendError::ShutdownPanicked {
            phase: "platform windows",
        }))
    }

    fn shutdown_native_explicit(&self) -> Result<(), Sdl3BackendError> {
        self.begin_shutdown();
        let platform_result = self.release_platform_explicit();
        let platform_retry_result = if self.platform_released() {
            Ok(())
        } else {
            self.release_platform_explicit()
        };
        let renderer_result = if self.platform_released() {
            self.release_renderer_explicit()
        } else {
            Ok(())
        };
        let renderer_retry_result = if self.platform_released() && !self.renderer_released() {
            self.release_renderer_explicit()
        } else {
            Ok(())
        };
        first_error([
            platform_result.err(),
            platform_retry_result.err(),
            renderer_result.err(),
            renderer_retry_result.err(),
        ])
    }

    fn shutdown_best_effort(&self) {
        if matches!(
            self.state.get(),
            RuntimeState::Detached | RuntimeState::ResourceDropped
        ) {
            return;
        }
        self.begin_shutdown();
        let _ = self.binding.try_with_bound_context(|| {
            self.shutdown_bound_best_effort();
        });
    }

    fn shutdown_bound_best_effort(&self) {
        for _ in 0..2 {
            if self.platform_released() {
                break;
            }
            match catch_unwind(AssertUnwindSafe(|| self.release_platform_bound())) {
                Ok(Ok(())) => {}
                Ok(Err(_)) => break,
                Err(_) => self.record_shutdown_panicked("platform windows"),
            }
        }
        if !self.platform_released() {
            return;
        }
        for _ in 0..2 {
            if self.renderer_released() {
                break;
            }
            if catch_unwind(AssertUnwindSafe(|| self.release_renderer_bound())).is_err() {
                self.record_shutdown_panicked("renderer resources");
            }
        }
    }

    fn detect_callback_replacements(&self) {
        if self.state.get() != RuntimeState::Attached || !self.platform_initialized.get() {
            return;
        }
        let _ = self.binding.try_with_bound_context(|| {
            if let Some(callbacks) = self.callbacks.borrow().as_ref() {
                unsafe { callbacks.detect_replacements(self) };
            }
        });
    }

    pub(super) fn poll_fault(&self) -> Result<(), Sdl3BackendError> {
        self.detect_callback_replacements();
        match self.faults.borrow_mut().pop_front() {
            Some(fault) => Err(fault.into_error()),
            None => Ok(()),
        }
    }

    fn take_pending_fault(&self) -> Option<Sdl3BackendError> {
        self.detect_callback_replacements();
        self.faults
            .borrow_mut()
            .pop_front()
            .map(RuntimeFault::into_error)
    }

    pub(super) fn ensure_entry(&self, context: &Context) -> Result<(), Sdl3BackendError> {
        self.ensure_context(context)?;
        self.poll_fault()?;
        if self.state.get() != RuntimeState::Attached {
            return Err(Sdl3BackendError::RuntimeDetached);
        }
        Ok(())
    }

    pub(super) fn finish_entry(&self) -> Result<(), Sdl3BackendError> {
        self.poll_fault()
    }

    pub(super) fn ensure_context(&self, context: &Context) -> Result<(), Sdl3BackendError> {
        let expected = self.binding.id();
        let actual = context.id();
        if expected != actual {
            return Err(Sdl3BackendError::ContextMismatch { expected, actual });
        }
        Ok(())
    }

    pub(super) fn original_create_window(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)> {
        self.callbacks
            .borrow()
            .as_ref()
            .and_then(PlatformCallbackOwnership::original_create_window)
    }

    pub(super) fn original_destroy_window(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)> {
        self.callbacks
            .borrow()
            .as_ref()
            .and_then(PlatformCallbackOwnership::original_destroy_window)
    }

    pub(super) fn remember_owned_viewport(
        &self,
        viewport: *mut sys::ImGuiViewport,
        state: ViewportPlatformState,
    ) {
        self.owned_viewports
            .borrow_mut()
            .insert(viewport as usize, state);
    }

    pub(super) fn take_owned_viewport(
        &self,
        viewport: *mut sys::ImGuiViewport,
    ) -> Option<ViewportPlatformState> {
        self.owned_viewports
            .borrow_mut()
            .remove(&(viewport as usize))
    }

    fn record_fault(&self, fault: RuntimeFault) {
        if !self.faults.borrow().contains(&fault) {
            self.faults.borrow_mut().push_back(fault);
        }
    }

    pub(super) fn record_callback_replaced(&self, callback: &'static str) {
        if self.reported_replacements.borrow_mut().insert(callback) {
            self.record_fault(RuntimeFault::CallbackReplaced(callback));
        }
        self.begin_shutdown();
    }

    pub(super) fn record_platform_state_replaced(&self, field: &'static str) {
        if self.reported_replacements.borrow_mut().insert(field) {
            self.record_fault(RuntimeFault::PlatformStateReplaced(field));
        }
        self.begin_shutdown();
    }

    pub(super) fn record_callback_panicked(&self, callback: &'static str) {
        self.record_fault(RuntimeFault::CallbackPanicked(callback));
    }

    pub(super) fn record_foreign_platform_user_data(&self) {
        self.record_fault(RuntimeFault::ForeignPlatformUserData);
        self.begin_shutdown();
    }

    pub(super) fn record_viewport_creation_failed(&self) {
        self.record_fault(RuntimeFault::ViewportCreationFailed);
    }

    fn record_shutdown_panicked(&self, phase: &'static str) {
        self.record_fault(RuntimeFault::ShutdownPanicked(phase));
    }

    fn context_destroyed(&self) {
        unregister_runtime(self.platform_io_key.replace(0));
        self.callbacks.borrow_mut().take();
        self.owned_viewports.borrow_mut().clear();
        self.platform_initialized.set(false);
        self.renderer_initialized.set(false);
        self.renderer_release.set(ReleaseState::Released);
        self.platform_release.set(ReleaseState::Released);
        self.state.set(RuntimeState::Detached);
    }

    fn mark_owner_dropped(&self) {
        if self.state.get() == RuntimeState::Detached {
            self.state.set(RuntimeState::ResourceDropped);
        }
    }

    #[cfg(test)]
    fn phase_log(&self) -> Vec<&'static str> {
        self.phase_log.borrow().clone()
    }

    fn accepts_current_callback(&self) -> bool {
        if !self.platform_initialized.get() {
            return false;
        }
        match (self.state.get(), self.binding.lifecycle()) {
            (RuntimeState::Attached, ContextLifecycle::Alive) => true,
            (RuntimeState::ShuttingDown, ContextLifecycle::Alive | ContextLifecycle::Dropping) => {
                self.callback_teardown_active.get()
            }
            _ => false,
        }
    }
}

struct PlatformAttachment {
    control: Rc<RuntimeControl>,
}

impl ContextAttachment for PlatformAttachment {
    fn quiesce(&self, _context: &ContextTeardown<'_>) {
        self.control.begin_shutdown();
    }

    fn release_platform_windows(&self, context: &ContextTeardown<'_>) {
        self.control.begin_shutdown();
        let result = catch_unwind(AssertUnwindSafe(|| {
            context.with_bound_context(|| {
                self.control.shutdown_bound_best_effort();
            });
        }));
        if result.is_err() {
            self.control.record_shutdown_panicked("platform windows");
        }
    }

    fn context_destroyed(&self, _context: ContextDestroyed) {
        self.control.context_destroyed();
    }
}

struct RendererAttachment {
    control: Rc<RuntimeControl>,
}

impl ContextAttachment for RendererAttachment {
    fn release_renderer_resources(&self, _context: &ContextTeardown<'_>) {
        // The official SDL renderer backends call DestroyPlatformWindows() from their own
        // shutdown paths. Keep renderer callbacks alive through the platform phase, where the
        // paired platform and renderer backends can be released in their required order.
        self.control.begin_shutdown();
    }
}

pub(super) struct RuntimeRegistration {
    control: Rc<RuntimeControl>,
    baseline: Option<PlatformClaimBaseline>,
    platform_attachment: Option<ContextAttachmentLease>,
    renderer_attachment: Option<ContextAttachmentLease>,
}

impl fmt::Debug for RuntimeRegistration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeRegistration")
            .field("control", &self.control)
            .field("native_initialization_pending", &self.baseline.is_some())
            .field("platform_attached", &self.platform_attachment.is_some())
            .field("renderer_attached", &self.renderer_attachment.is_some())
            .finish()
    }
}

impl RuntimeRegistration {
    pub(super) fn prepare(
        context: &mut Context,
        renderer_shutdown: Option<fn()>,
    ) -> Result<Self, Sdl3BackendError> {
        let baseline = preflight_platform_claim(context)?;
        let renderer_shutdown = renderer_shutdown.map(|shutdown| Rc::new(shutdown) as Rc<dyn Fn()>);
        let control = Rc::new(RuntimeControl::new(
            context,
            renderer_shutdown,
            Rc::new(shutdown_platform_impl),
        ));
        let mut platform_attachment = context.register_attachment::<Sdl3PlatformAttachmentMarker>(
            ContextAttachmentRole::Platform,
            Rc::new(PlatformAttachment {
                control: Rc::clone(&control),
            }),
        )?;
        let renderer_attachment = if control.lifecycle.renderer_shutdown.is_some() {
            match context.register_attachment::<Sdl3RendererAttachmentMarker>(
                ContextAttachmentRole::Renderer,
                Rc::new(RendererAttachment {
                    control: Rc::clone(&control),
                }),
            ) {
                Ok(attachment) => Some(attachment),
                Err(error) => {
                    platform_attachment.detach();
                    return Err(error.into());
                }
            }
        } else {
            None
        };

        Ok(Self {
            control,
            baseline: Some(baseline),
            platform_attachment: Some(platform_attachment),
            renderer_attachment,
        })
    }

    pub(super) fn finish_native_initialization(
        &mut self,
        context: &Context,
    ) -> Result<(), Sdl3BackendError> {
        self.control.ensure_context(context)?;
        let baseline = self
            .baseline
            .as_ref()
            .expect("SDL3 native initialization was already completed")
            .snapshot();
        self.control.platform_initialized.set(true);
        self.control
            .renderer_initialized
            .set(self.control.lifecycle.renderer_shutdown.is_some());
        let claim_result = self.control.binding.try_with_bound_context(|| unsafe {
            PlatformCallbackOwnership::claim(&self.control, baseline)
        });
        match claim_result {
            Ok(Ok(ownership)) => {
                self.baseline.take();
                self.control.callbacks.borrow_mut().replace(ownership);
                Ok(())
            }
            Ok(Err(error)) => {
                self.rollback_claim_failure();
                Err(error)
            }
            Err(error) => {
                self.rollback_claim_failure();
                Err(error.into())
            }
        }
    }

    fn rollback_claim_failure(&mut self) {
        let baseline = self.baseline.take();
        self.control.begin_shutdown();
        let _ = self.control.binding.try_with_bound_context(|| {
            let _ = self.control.release_platform_bound();
            if self.control.platform_released() {
                self.control.release_renderer_bound();
            }
            if let Some(baseline) = baseline {
                unsafe { restore_baseline_after_failed_initialization(baseline) };
            }
        });
        self.detach_attachments();
    }

    pub(super) fn native_initialization_failed(&mut self) {
        if let Some(baseline) = self.baseline.take() {
            let _ = self.control.binding.try_with_bound_context(|| unsafe {
                restore_baseline_after_failed_initialization(baseline)
            });
        }
        self.control.platform_initialized.set(false);
        self.control.renderer_initialized.set(false);
        self.control.renderer_release.set(ReleaseState::Released);
        self.control.platform_release.set(ReleaseState::Released);
        self.control.state.set(RuntimeState::Detached);
        self.detach_attachments();
    }

    pub(super) fn control(&self) -> &RuntimeControl {
        &self.control
    }

    pub(super) fn poll_fault(&self) -> Result<(), Sdl3BackendError> {
        self.control.poll_fault()
    }

    pub(super) fn shutdown_platform(
        &mut self,
        context: &mut Context,
    ) -> Result<(), Sdl3BackendError> {
        self.control.ensure_context(context)?;
        let pending = self.control.take_pending_fault();
        let shutdown_result = self.control.shutdown_native_explicit();
        if matches!(self.control.state(), RuntimeState::Detached) {
            self.detach_attachments();
        }
        first_error([pending, shutdown_result.err()])
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    pub(super) fn shutdown_renderer(
        &mut self,
        context: &mut Context,
        consumer: Option<&RendererConsumer>,
        after_renderer_release: impl FnOnce(),
    ) -> Result<(), Sdl3BackendError> {
        self.control.ensure_context(context)?;
        let pending = self.control.take_pending_fault();
        if matches!(
            self.control.state(),
            RuntimeState::Detached | RuntimeState::ResourceDropped
        ) {
            self.detach_attachments();
            return first_error([pending, None, None, None]);
        }

        let shutdown_result = self.control.shutdown_native_explicit();
        let reset_result = if self.control.renderer_released() {
            after_renderer_release();
            consumer
                .map(|consumer| context.reset_renderer_texture_bindings(consumer))
                .transpose()
                .map(|_| ())
                .map_err(Into::into)
        } else {
            Ok(())
        };
        if matches!(self.control.state(), RuntimeState::Detached) {
            self.detach_attachments();
        }
        first_error([pending, shutdown_result.err(), reset_result.err()])
    }

    fn detach_attachments(&mut self) {
        if let Some(mut renderer) = self.renderer_attachment.take() {
            renderer.detach();
        }
        if let Some(mut platform) = self.platform_attachment.take() {
            platform.detach();
        }
    }
}

impl Drop for RuntimeRegistration {
    fn drop(&mut self) {
        self.control.shutdown_best_effort();
        self.detach_attachments();
        self.control.mark_owner_dropped();
    }
}

fn first_error<const N: usize>(
    errors: [Option<Sdl3BackendError>; N],
) -> Result<(), Sdl3BackendError> {
    errors.into_iter().flatten().next().map_or(Ok(()), Err)
}

thread_local! {
    static RUNTIMES: RefCell<HashMap<usize, Weak<RuntimeControl>>> = RefCell::new(HashMap::new());
}

pub(super) fn register_runtime(control: &Rc<RuntimeControl>) {
    let key = unsafe { sys::igGetPlatformIO_Nil() as usize };
    control.platform_io_key.set(key);
    RUNTIMES.with(|runtimes| {
        runtimes.borrow_mut().insert(key, Rc::downgrade(control));
    });
}

pub(super) fn unregister_runtime(key: usize) {
    if key == 0 {
        return;
    }
    RUNTIMES.with(|runtimes| {
        runtimes.borrow_mut().remove(&key);
    });
}

pub(super) fn with_current_runtime<R>(callback: impl FnOnce(&RuntimeControl) -> R) -> Option<R> {
    let key = unsafe { sys::igGetPlatformIO_Nil() as usize };
    RUNTIMES.with(|runtimes| {
        let control = runtimes.borrow().get(&key).cloned()?.upgrade()?;
        control
            .accepts_current_callback()
            .then(|| callback(&control))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::callback_ownership::{
        create_window_callback_for_test, destroy_window_callback_for_test,
    };

    const OWNED_BACKEND_DATA: usize = 0x101;
    const FOREIGN_BACKEND_DATA: usize = 0x102;
    const OWNED_PLATFORM_DATA: usize = 0x201;
    const FOREIGN_PLATFORM_DATA: usize = 0x202;
    const OWNED_VIEWPORT_DATA: usize = 0x301;
    const FOREIGN_VIEWPORT_DATA: usize = 0x302;
    const OWNED_VIEWPORT_HANDLE: usize = 0x401;
    const FOREIGN_VIEWPORT_HANDLE: usize = 0x402;
    static OWNED_BACKEND_NAME: &[u8] = b"SDL3-test\0";
    static FOREIGN_BACKEND_NAME: &[u8] = b"foreign-test\0";

    thread_local! {
        static DESTROY_OBSERVED_USER_DATA: Cell<usize> = const { Cell::new(0) };
    }

    unsafe extern "C" fn synthetic_create_window(viewport: *mut sys::ImGuiViewport) {
        if let Some(viewport) = unsafe { viewport.as_mut() } {
            viewport.PlatformUserData = OWNED_VIEWPORT_DATA as *mut _;
            viewport.PlatformHandle = OWNED_VIEWPORT_HANDLE as *mut _;
        }
    }

    unsafe extern "C" fn synthetic_destroy_window(viewport: *mut sys::ImGuiViewport) {
        if let Some(viewport) = unsafe { viewport.as_mut() } {
            DESTROY_OBSERVED_USER_DATA
                .with(|observed| observed.set(viewport.PlatformUserData as usize));
            viewport.PlatformUserData = std::ptr::null_mut();
            viewport.PlatformHandle = std::ptr::null_mut();
            viewport.PlatformHandleRaw = std::ptr::null_mut();
        }
    }

    unsafe extern "C" fn foreign_create_window(_viewport: *mut sys::ImGuiViewport) {}

    unsafe extern "C" fn failing_create_window(_viewport: *mut sys::ImGuiViewport) {}

    fn registration_with_lifecycle(
        context: &mut Context,
        renderer_shutdown: Option<Rc<dyn Fn()>>,
        platform_shutdown: Rc<dyn Fn()>,
    ) -> RuntimeRegistration {
        let baseline = preflight_platform_claim(context).unwrap();
        let control = Rc::new(RuntimeControl::new(
            context,
            renderer_shutdown,
            platform_shutdown,
        ));
        let platform_attachment = context
            .register_attachment::<Sdl3PlatformAttachmentMarker>(
                ContextAttachmentRole::Platform,
                Rc::new(PlatformAttachment {
                    control: Rc::clone(&control),
                }),
            )
            .unwrap();
        let renderer_attachment = control.lifecycle.renderer_shutdown.as_ref().map(|_| {
            context
                .register_attachment::<Sdl3RendererAttachmentMarker>(
                    ContextAttachmentRole::Renderer,
                    Rc::new(RendererAttachment {
                        control: Rc::clone(&control),
                    }),
                )
                .unwrap()
        });
        RuntimeRegistration {
            control,
            baseline: Some(baseline),
            platform_attachment: Some(platform_attachment),
            renderer_attachment,
        }
    }

    fn test_registration(
        context: &mut Context,
        renderer_count: Rc<Cell<usize>>,
        platform_count: Rc<Cell<usize>>,
    ) -> RuntimeRegistration {
        let registration = registration_with_lifecycle(
            context,
            Some({
                let renderer_count = Rc::clone(&renderer_count);
                Rc::new(move || renderer_count.set(renderer_count.get() + 1))
            }),
            {
                let platform_count = Rc::clone(&platform_count);
                Rc::new(move || platform_count.set(platform_count.get() + 1))
            },
        );
        registration.control.platform_initialized.set(true);
        registration.control.renderer_initialized.set(true);
        registration
    }

    fn synthetic_claimed_registration(
        context: &mut Context,
        platform_count: Rc<Cell<usize>>,
        observed_backend_data: Rc<Cell<usize>>,
        observed_main_viewport_data: Rc<Cell<usize>>,
        create_window: unsafe extern "C" fn(*mut sys::ImGuiViewport),
    ) -> RuntimeRegistration {
        synthetic_claimed_registration_with_renderer(
            context,
            None,
            None,
            platform_count,
            observed_backend_data,
            observed_main_viewport_data,
            create_window,
        )
    }

    fn synthetic_claimed_registration_with_renderer(
        context: &mut Context,
        renderer_shutdown: Option<Rc<dyn Fn()>>,
        platform_shutdown_hook: Option<Rc<dyn Fn()>>,
        platform_count: Rc<Cell<usize>>,
        observed_backend_data: Rc<Cell<usize>>,
        observed_main_viewport_data: Rc<Cell<usize>>,
        create_window: unsafe extern "C" fn(*mut sys::ImGuiViewport),
    ) -> RuntimeRegistration {
        let mut registration = registration_with_lifecycle(
            context,
            renderer_shutdown,
            Rc::new({
                let platform_count = Rc::clone(&platform_count);
                let observed_backend_data = Rc::clone(&observed_backend_data);
                let observed_main_viewport_data = Rc::clone(&observed_main_viewport_data);
                let platform_shutdown_hook = platform_shutdown_hook.clone();
                move || unsafe {
                    platform_count.set(platform_count.get() + 1);
                    if let Some(hook) = &platform_shutdown_hook {
                        hook();
                    }
                    let io = sys::igGetIO_Nil();
                    let platform_io = sys::igGetPlatformIO_Nil();
                    let main_viewport = sys::igGetMainViewport();
                    observed_backend_data.set((*io).BackendPlatformUserData as usize);
                    observed_main_viewport_data.set((*main_viewport).PlatformUserData as usize);

                    sys::ImGuiPlatformIO_ClearPlatformHandlers(platform_io);
                    (*io).BackendPlatformUserData = std::ptr::null_mut();
                    (*io).BackendPlatformName = std::ptr::null();
                    (*main_viewport).PlatformUserData = std::ptr::null_mut();
                    (*main_viewport).PlatformHandle = std::ptr::null_mut();
                    (*main_viewport).PlatformHandleRaw = std::ptr::null_mut();
                }
            }),
        );

        context.binding().with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            let platform_io = sys::igGetPlatformIO_Nil();
            let main_viewport = sys::igGetMainViewport();
            (*io).BackendPlatformUserData = OWNED_BACKEND_DATA as *mut _;
            (*io).BackendPlatformName = OWNED_BACKEND_NAME.as_ptr().cast();
            (*platform_io).Platform_CreateWindow = Some(create_window);
            (*platform_io).Platform_DestroyWindow = Some(synthetic_destroy_window);
            (*platform_io).Platform_ClipboardUserData = OWNED_PLATFORM_DATA as *mut _;
            (*main_viewport).PlatformUserData = OWNED_VIEWPORT_DATA as *mut _;
            (*main_viewport).PlatformHandle = OWNED_VIEWPORT_HANDLE as *mut _;
        });

        let baseline = registration.baseline.take().unwrap();
        let ownership = context.binding().with_bound_context(|| unsafe {
            PlatformCallbackOwnership::claim(&registration.control, baseline).unwrap()
        });
        registration
            .control
            .callbacks
            .borrow_mut()
            .replace(ownership);
        registration.control.platform_initialized.set(true);
        registration
            .control
            .renderer_initialized
            .set(registration.control.lifecycle.renderer_shutdown.is_some());
        registration
    }

    fn registry_contains(key: usize) -> bool {
        RUNTIMES.with(|runtimes| runtimes.borrow().contains_key(&key))
    }

    struct TeardownPhaseObserver {
        renderer_count: Rc<Cell<usize>>,
        platform_count: Rc<Cell<usize>>,
        renderer_phase_counts: Rc<Cell<(usize, usize)>>,
        platform_phase_counts: Rc<Cell<(usize, usize)>>,
    }

    impl ContextAttachment for TeardownPhaseObserver {
        fn release_renderer_resources(&self, _context: &ContextTeardown<'_>) {
            self.renderer_phase_counts
                .set((self.renderer_count.get(), self.platform_count.get()));
        }

        fn release_platform_windows(&self, _context: &ContextTeardown<'_>) {
            self.platform_phase_counts
                .set((self.renderer_count.get(), self.platform_count.get()));
        }
    }

    struct TeardownPhaseObserverMarker;

    #[test]
    fn context_first_shutdown_runs_each_phase_once_in_order() {
        let _guard = crate::tests::test_guard();
        let renderer_count = Rc::new(Cell::new(0));
        let platform_count = Rc::new(Cell::new(0));
        let mut context = Context::create();
        let runtime = test_registration(
            &mut context,
            Rc::clone(&renderer_count),
            Rc::clone(&platform_count),
        );
        let renderer_phase_counts = Rc::new(Cell::new((usize::MAX, usize::MAX)));
        let platform_phase_counts = Rc::new(Cell::new((usize::MAX, usize::MAX)));
        let _observer = context
            .register_attachment::<TeardownPhaseObserverMarker>(
                ContextAttachmentRole::Extension,
                Rc::new(TeardownPhaseObserver {
                    renderer_count: Rc::clone(&renderer_count),
                    platform_count: Rc::clone(&platform_count),
                    renderer_phase_counts: Rc::clone(&renderer_phase_counts),
                    platform_phase_counts: Rc::clone(&platform_phase_counts),
                }),
            )
            .unwrap();
        let control = Rc::clone(&runtime.control);

        drop(context);

        assert_eq!(renderer_phase_counts.get(), (0, 0));
        assert_eq!(platform_phase_counts.get(), (1, 1));
        assert_eq!(renderer_count.get(), 1);
        assert_eq!(platform_count.get(), 1);
        assert_eq!(control.phase_log(), ["platform", "renderer"]);
        assert_eq!(control.state(), RuntimeState::Detached);
        drop(runtime);
        assert_eq!(control.state(), RuntimeState::ResourceDropped);
        assert_eq!(renderer_count.get(), 1);
        assert_eq!(platform_count.get(), 1);
    }

    #[test]
    fn wrapper_first_and_repeated_shutdown_are_idempotent_after_move() {
        let _guard = crate::tests::test_guard();
        let renderer_count = Rc::new(Cell::new(0));
        let platform_count = Rc::new(Cell::new(0));
        let mut context = Context::create();
        let runtime = test_registration(
            &mut context,
            Rc::clone(&renderer_count),
            Rc::clone(&platform_count),
        );
        let control_address = Rc::as_ptr(&runtime.control);
        let mut slot = Some(runtime);
        let mut moved = slot.take().unwrap();
        assert_eq!(Rc::as_ptr(&moved.control), control_address);

        moved.shutdown_platform(&mut context).unwrap();
        moved.shutdown_platform(&mut context).unwrap();

        assert_eq!(renderer_count.get(), 1);
        assert_eq!(platform_count.get(), 1);
        assert_eq!(moved.control.phase_log(), ["platform", "renderer"]);
        drop(moved);
        drop(context);
        assert_eq!(renderer_count.get(), 1);
        assert_eq!(platform_count.get(), 1);
    }

    #[test]
    fn platform_shutdown_keeps_platform_destroy_callbacks_live() {
        let _guard = crate::tests::test_guard();
        let mut context = Context::create();
        let viewport = Rc::new(RefCell::new(sys::ImGuiViewport::default()));
        let platform_shutdown_hook: Rc<dyn Fn()> = {
            let viewport = Rc::clone(&viewport);
            Rc::new(move || unsafe {
                destroy_window_callback_for_test(&mut *viewport.borrow_mut());
            })
        };
        let platform_count = Rc::new(Cell::new(0));
        let observed_backend_data = Rc::new(Cell::new(0));
        let observed_main_viewport_data = Rc::new(Cell::new(0));
        let mut runtime = synthetic_claimed_registration_with_renderer(
            &mut context,
            Some(Rc::new(|| {})),
            Some(platform_shutdown_hook),
            Rc::clone(&platform_count),
            observed_backend_data,
            observed_main_viewport_data,
            synthetic_create_window,
        );

        context.binding().with_bound_context(|| unsafe {
            create_window_callback_for_test(&mut *viewport.borrow_mut());
        });
        assert_eq!(
            viewport.borrow().PlatformUserData as usize,
            OWNED_VIEWPORT_DATA
        );

        runtime.shutdown_platform(&mut context).unwrap();

        assert!(viewport.borrow().PlatformUserData.is_null());
        assert!(viewport.borrow().PlatformHandle.is_null());
        assert_eq!(platform_count.get(), 1);
    }

    #[test]
    fn wrapper_drop_releases_each_phase_once() {
        let _guard = crate::tests::test_guard();
        let renderer_count = Rc::new(Cell::new(0));
        let platform_count = Rc::new(Cell::new(0));
        let mut context = Context::create();
        let runtime = test_registration(
            &mut context,
            Rc::clone(&renderer_count),
            Rc::clone(&platform_count),
        );
        let control = Rc::clone(&runtime.control);

        drop(runtime);

        assert_eq!(renderer_count.get(), 1);
        assert_eq!(platform_count.get(), 1);
        assert_eq!(control.phase_log(), ["platform", "renderer"]);
        assert_eq!(control.state(), RuntimeState::ResourceDropped);
        drop(context);
        assert_eq!(renderer_count.get(), 1);
        assert_eq!(platform_count.get(), 1);
    }

    #[test]
    fn wrapper_drop_releases_platform_then_retries_renderer_panic() {
        let _guard = crate::tests::test_guard();
        let renderer_count = Rc::new(Cell::new(0));
        let platform_count = Rc::new(Cell::new(0));
        let mut context = Context::create();
        let runtime = registration_with_lifecycle(
            &mut context,
            Some({
                let renderer_count = Rc::clone(&renderer_count);
                Rc::new(move || {
                    let attempt = renderer_count.get() + 1;
                    renderer_count.set(attempt);
                    if attempt == 1 {
                        panic!("synthetic renderer shutdown failure");
                    }
                })
            }),
            {
                let platform_count = Rc::clone(&platform_count);
                Rc::new(move || platform_count.set(platform_count.get() + 1))
            },
        );
        runtime.control.platform_initialized.set(true);
        runtime.control.renderer_initialized.set(true);
        let control = Rc::clone(&runtime.control);

        assert!(catch_unwind(AssertUnwindSafe(|| drop(runtime))).is_ok());

        assert_eq!(renderer_count.get(), 2);
        assert_eq!(platform_count.get(), 1);
        assert_eq!(control.phase_log(), ["platform", "renderer", "renderer"]);
        assert_eq!(control.state(), RuntimeState::ResourceDropped);
    }

    #[test]
    fn explicit_shutdown_reports_renderer_panic_after_completing_cleanup() {
        let _guard = crate::tests::test_guard();
        let renderer_count = Rc::new(Cell::new(0));
        let platform_count = Rc::new(Cell::new(0));
        let mut context = Context::create();
        let mut runtime = registration_with_lifecycle(
            &mut context,
            Some({
                let renderer_count = Rc::clone(&renderer_count);
                Rc::new(move || {
                    let attempt = renderer_count.get() + 1;
                    renderer_count.set(attempt);
                    if attempt == 1 {
                        panic!("synthetic explicit renderer failure");
                    }
                })
            }),
            {
                let platform_count = Rc::clone(&platform_count);
                Rc::new(move || platform_count.set(platform_count.get() + 1))
            },
        );
        runtime.control.platform_initialized.set(true);
        runtime.control.renderer_initialized.set(true);

        assert!(matches!(
            runtime.shutdown_platform(&mut context),
            Err(Sdl3BackendError::ShutdownPanicked {
                phase: "renderer resources"
            })
        ));
        assert_eq!(renderer_count.get(), 2);
        assert_eq!(platform_count.get(), 1);
        assert_eq!(runtime.control.state(), RuntimeState::Detached);
        runtime.poll_fault().unwrap();
        runtime.shutdown_platform(&mut context).unwrap();
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    #[test]
    fn renderer_shutdown_retries_before_texture_cleanup_after_platform_release() {
        let _guard = crate::tests::test_guard();
        let renderer_count = Rc::new(Cell::new(0));
        let platform_count = Rc::new(Cell::new(0));
        let cleanup_count = Rc::new(Cell::new(0));
        let mut context = Context::create();
        let consumer = context.create_renderer_consumer().unwrap();
        let mut runtime = registration_with_lifecycle(
            &mut context,
            Some({
                let renderer_count = Rc::clone(&renderer_count);
                Rc::new(move || {
                    let attempt = renderer_count.get() + 1;
                    renderer_count.set(attempt);
                    if attempt == 1 {
                        panic!("synthetic composite renderer failure");
                    }
                })
            }),
            {
                let platform_count = Rc::clone(&platform_count);
                Rc::new(move || platform_count.set(platform_count.get() + 1))
            },
        );
        runtime.control.platform_initialized.set(true);
        runtime.control.renderer_initialized.set(true);

        let result = runtime.shutdown_renderer(&mut context, Some(&consumer), {
            let renderer_count = Rc::clone(&renderer_count);
            let platform_count = Rc::clone(&platform_count);
            let cleanup_count = Rc::clone(&cleanup_count);
            move || {
                assert_eq!(renderer_count.get(), 2);
                assert_eq!(platform_count.get(), 1);
                cleanup_count.set(cleanup_count.get() + 1);
            }
        });

        assert!(matches!(
            result,
            Err(Sdl3BackendError::ShutdownPanicked {
                phase: "renderer resources"
            })
        ));
        assert_eq!(cleanup_count.get(), 1);
        assert_eq!(platform_count.get(), 1);
        assert_eq!(runtime.control.state(), RuntimeState::Detached);
    }

    #[test]
    fn context_teardown_releases_platform_then_retries_panicked_renderer() {
        let _guard = crate::tests::test_guard();
        let renderer_count = Rc::new(Cell::new(0));
        let platform_count = Rc::new(Cell::new(0));
        let mut context = Context::create();
        let runtime = registration_with_lifecycle(
            &mut context,
            Some({
                let renderer_count = Rc::clone(&renderer_count);
                Rc::new(move || {
                    let attempt = renderer_count.get() + 1;
                    renderer_count.set(attempt);
                    if attempt == 1 {
                        panic!("synthetic renderer teardown failure");
                    }
                })
            }),
            {
                let platform_count = Rc::clone(&platform_count);
                Rc::new(move || platform_count.set(platform_count.get() + 1))
            },
        );
        runtime.control.platform_initialized.set(true);
        runtime.control.renderer_initialized.set(true);
        let control = Rc::clone(&runtime.control);

        drop(context);

        assert_eq!(renderer_count.get(), 2);
        assert_eq!(platform_count.get(), 1);
        assert_eq!(control.phase_log(), ["platform", "renderer", "renderer"]);
        assert_eq!(control.state(), RuntimeState::Detached);
        assert!(matches!(
            runtime.poll_fault(),
            Err(Sdl3BackendError::ShutdownPanicked {
                phase: "renderer resources"
            })
        ));
        drop(runtime);
        assert_eq!(control.state(), RuntimeState::ResourceDropped);
    }

    #[test]
    fn platform_phase_does_not_drop_renderer_global_state() {
        let _guard = crate::tests::test_guard();
        let renderer_count = Rc::new(Cell::new(0));
        let platform_count = Rc::new(Cell::new(0));
        let mut context = Context::create();
        let runtime = test_registration(
            &mut context,
            Rc::clone(&renderer_count),
            Rc::clone(&platform_count),
        );

        runtime.control.begin_shutdown();
        context
            .binding()
            .with_bound_context(|| runtime.control.release_platform_bound().unwrap());

        assert_eq!(runtime.control.phase_log(), ["platform"]);
        assert_eq!(renderer_count.get(), 0);
        assert_eq!(platform_count.get(), 1);

        context
            .binding()
            .with_bound_context(|| runtime.control.release_renderer_bound());
        assert_eq!(runtime.control.phase_log(), ["platform", "renderer"]);
        assert_eq!(renderer_count.get(), 1);
    }

    #[test]
    fn owned_callback_state_restores_baseline_without_dangling_native_data() {
        let _guard = crate::tests::test_guard();
        let mut context = Context::create();
        let baseline_clipboard = context.binding().with_bound_context(|| unsafe {
            let platform_io = sys::igGetPlatformIO_Nil();
            (
                (*platform_io).Platform_GetClipboardTextFn,
                (*platform_io).Platform_ClipboardUserData,
            )
        });
        let platform_count = Rc::new(Cell::new(0));
        let observed_backend_data = Rc::new(Cell::new(0));
        let observed_main_viewport_data = Rc::new(Cell::new(0));
        let mut runtime = synthetic_claimed_registration(
            &mut context,
            Rc::clone(&platform_count),
            Rc::clone(&observed_backend_data),
            Rc::clone(&observed_main_viewport_data),
            synthetic_create_window,
        );
        let registry_key = runtime.control.platform_io_key.get();
        assert!(registry_contains(registry_key));

        runtime.shutdown_platform(&mut context).unwrap();

        assert_eq!(platform_count.get(), 1);
        assert_eq!(observed_backend_data.get(), OWNED_BACKEND_DATA);
        assert_eq!(observed_main_viewport_data.get(), OWNED_VIEWPORT_DATA);
        assert_eq!(runtime.control.platform_io_key.get(), 0);
        assert!(!registry_contains(registry_key));
        assert!(with_current_runtime(|_| ()).is_none());
        context.binding().with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            let platform_io = sys::igGetPlatformIO_Nil();
            let main_viewport = sys::igGetMainViewport();
            assert!((*io).BackendPlatformUserData.is_null());
            assert!((*io).BackendPlatformName.is_null());
            assert!((*platform_io).Platform_CreateWindow.is_none());
            assert_eq!(
                (*platform_io).Platform_ClipboardUserData,
                baseline_clipboard.1
            );
            match (
                (*platform_io).Platform_GetClipboardTextFn,
                baseline_clipboard.0,
            ) {
                (Some(actual), Some(expected)) => assert!(std::ptr::fn_addr_eq(actual, expected)),
                (None, None) => {}
                _ => panic!("clipboard callback baseline was not restored"),
            }
            assert!((*main_viewport).PlatformUserData.is_null());
            assert!((*main_viewport).PlatformHandle.is_null());
        });
    }

    #[test]
    fn context_first_claimed_runtime_unregisters_callback_registry() {
        let _guard = crate::tests::test_guard();
        let mut context = Context::create();
        let platform_count = Rc::new(Cell::new(0));
        let observed_backend_data = Rc::new(Cell::new(0));
        let observed_main_viewport_data = Rc::new(Cell::new(0));
        let runtime = synthetic_claimed_registration(
            &mut context,
            Rc::clone(&platform_count),
            Rc::clone(&observed_backend_data),
            Rc::clone(&observed_main_viewport_data),
            synthetic_create_window,
        );
        let control = Rc::clone(&runtime.control);
        let registry_key = control.platform_io_key.get();

        drop(context);

        assert_eq!(platform_count.get(), 1);
        assert_eq!(observed_backend_data.get(), OWNED_BACKEND_DATA);
        assert_eq!(observed_main_viewport_data.get(), OWNED_VIEWPORT_DATA);
        assert!(!registry_contains(registry_key));
        assert_eq!(control.platform_io_key.get(), 0);
        assert_eq!(control.state(), RuntimeState::Detached);
        drop(runtime);
        assert_eq!(control.state(), RuntimeState::ResourceDropped);
    }

    #[test]
    fn failed_viewport_creation_is_reported_on_next_rust_entry() {
        let _guard = crate::tests::test_guard();
        let mut context = Context::create();
        let platform_count = Rc::new(Cell::new(0));
        let observed_backend_data = Rc::new(Cell::new(0));
        let observed_main_viewport_data = Rc::new(Cell::new(0));
        let mut runtime = synthetic_claimed_registration(
            &mut context,
            Rc::clone(&platform_count),
            observed_backend_data,
            observed_main_viewport_data,
            failing_create_window,
        );
        let mut viewport = sys::ImGuiViewport::default();

        context
            .binding()
            .with_bound_context(|| unsafe { create_window_callback_for_test(&mut viewport) });

        assert!(viewport.PlatformRequestClose);
        assert!(matches!(
            runtime.poll_fault(),
            Err(Sdl3BackendError::ViewportCreationFailed)
        ));
        runtime.shutdown_platform(&mut context).unwrap();
        assert_eq!(platform_count.get(), 1);
    }

    #[test]
    fn foreign_callback_and_user_data_replacements_survive_shutdown() {
        let _guard = crate::tests::test_guard();
        let mut context = Context::create();
        let baseline_clipboard_user_data = context.binding().with_bound_context(|| unsafe {
            (*sys::igGetPlatformIO_Nil()).Platform_ClipboardUserData
        });
        let platform_count = Rc::new(Cell::new(0));
        let observed_backend_data = Rc::new(Cell::new(0));
        let observed_main_viewport_data = Rc::new(Cell::new(0));
        let mut runtime = synthetic_claimed_registration(
            &mut context,
            Rc::clone(&platform_count),
            Rc::clone(&observed_backend_data),
            Rc::clone(&observed_main_viewport_data),
            synthetic_create_window,
        );
        let registry_key = runtime.control.platform_io_key.get();

        context.binding().with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            let platform_io = sys::igGetPlatformIO_Nil();
            let main_viewport = sys::igGetMainViewport();
            (*io).BackendPlatformUserData = FOREIGN_BACKEND_DATA as *mut _;
            (*io).BackendPlatformName = FOREIGN_BACKEND_NAME.as_ptr().cast();
            (*platform_io).Platform_CreateWindow = Some(foreign_create_window);
            (*platform_io).Platform_ClipboardUserData = FOREIGN_PLATFORM_DATA as *mut _;
            (*main_viewport).PlatformUserData = FOREIGN_VIEWPORT_DATA as *mut _;
            (*main_viewport).PlatformHandle = FOREIGN_VIEWPORT_HANDLE as *mut _;
        });

        assert!(matches!(
            runtime.shutdown_platform(&mut context),
            Err(Sdl3BackendError::PlatformCallbackReplaced {
                callback: "Platform_CreateWindow"
            })
        ));

        assert_eq!(platform_count.get(), 1);
        assert_eq!(observed_backend_data.get(), OWNED_BACKEND_DATA);
        assert_eq!(observed_main_viewport_data.get(), OWNED_VIEWPORT_DATA);
        assert!(!registry_contains(registry_key));
        context.binding().with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            let platform_io = sys::igGetPlatformIO_Nil();
            let main_viewport = sys::igGetMainViewport();
            assert_eq!((*io).BackendPlatformUserData as usize, FOREIGN_BACKEND_DATA);
            assert_eq!(
                (*io).BackendPlatformName,
                FOREIGN_BACKEND_NAME.as_ptr().cast()
            );
            assert_eq!(
                (*platform_io).Platform_ClipboardUserData as usize,
                FOREIGN_PLATFORM_DATA
            );
            assert!(std::ptr::fn_addr_eq(
                (*platform_io).Platform_CreateWindow.unwrap(),
                foreign_create_window as unsafe extern "C" fn(*mut sys::ImGuiViewport)
            ));
            assert_eq!(
                (*main_viewport).PlatformUserData as usize,
                FOREIGN_VIEWPORT_DATA
            );
            assert_eq!(
                (*main_viewport).PlatformHandle as usize,
                FOREIGN_VIEWPORT_HANDLE
            );
        });
        assert!(matches!(
            runtime.poll_fault(),
            Err(Sdl3BackendError::PlatformStateReplaced {
                field: "Platform_ClipboardUserData"
            })
        ));
        assert!(matches!(
            runtime.poll_fault(),
            Err(Sdl3BackendError::PlatformStateReplaced {
                field: "BackendPlatformUserData"
            })
        ));
        assert!(matches!(
            runtime.poll_fault(),
            Err(Sdl3BackendError::PlatformStateReplaced {
                field: "BackendPlatformName"
            })
        ));
        assert!(matches!(
            runtime.poll_fault(),
            Err(Sdl3BackendError::ForeignPlatformUserData)
        ));
        assert!(matches!(
            runtime.poll_fault(),
            Err(Sdl3BackendError::PlatformStateReplaced {
                field: "MainViewport.PlatformHandle"
            })
        ));
        runtime.poll_fault().unwrap();

        // The synthetic foreign owner now performs its own shutdown before the
        // Context is dropped.
        context.binding().with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            let platform_io = sys::igGetPlatformIO_Nil();
            let main_viewport = sys::igGetMainViewport();
            (*io).BackendPlatformUserData = std::ptr::null_mut();
            (*io).BackendPlatformName = std::ptr::null();
            (*platform_io).Platform_CreateWindow = None;
            (*platform_io).Platform_ClipboardUserData = baseline_clipboard_user_data;
            (*main_viewport).PlatformUserData = std::ptr::null_mut();
            (*main_viewport).PlatformHandle = std::ptr::null_mut();
        });
    }

    #[test]
    fn destroy_callback_frees_owned_data_and_preserves_foreign_replacement() {
        let _guard = crate::tests::test_guard();
        DESTROY_OBSERVED_USER_DATA.with(|observed| observed.set(0));
        let mut context = Context::create();
        let platform_count = Rc::new(Cell::new(0));
        let observed_backend_data = Rc::new(Cell::new(0));
        let observed_main_viewport_data = Rc::new(Cell::new(0));
        let mut runtime = synthetic_claimed_registration(
            &mut context,
            Rc::clone(&platform_count),
            observed_backend_data,
            observed_main_viewport_data,
            synthetic_create_window,
        );
        let mut viewport = sys::ImGuiViewport::default();
        context
            .binding()
            .with_bound_context(|| unsafe { create_window_callback_for_test(&mut viewport) });
        assert_eq!(viewport.PlatformUserData as usize, OWNED_VIEWPORT_DATA);
        assert_eq!(viewport.PlatformHandle as usize, OWNED_VIEWPORT_HANDLE);
        viewport.PlatformUserData = FOREIGN_VIEWPORT_DATA as *mut _;
        viewport.PlatformHandle = FOREIGN_VIEWPORT_HANDLE as *mut _;

        context
            .binding()
            .with_bound_context(|| unsafe { destroy_window_callback_for_test(&mut viewport) });

        assert_eq!(
            DESTROY_OBSERVED_USER_DATA.with(Cell::get),
            OWNED_VIEWPORT_DATA
        );
        assert_eq!(viewport.PlatformUserData as usize, FOREIGN_VIEWPORT_DATA);
        assert_eq!(viewport.PlatformHandle as usize, FOREIGN_VIEWPORT_HANDLE);
        assert!(matches!(
            runtime.poll_fault(),
            Err(Sdl3BackendError::ForeignPlatformUserData)
        ));
        assert!(matches!(
            runtime.poll_fault(),
            Err(Sdl3BackendError::PlatformStateReplaced {
                field: "Viewport.PlatformHandle"
            })
        ));
        runtime.shutdown_platform(&mut context).unwrap();
        assert_eq!(platform_count.get(), 1);
    }
}
