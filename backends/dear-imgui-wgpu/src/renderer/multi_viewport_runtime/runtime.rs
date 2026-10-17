use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::fmt;
use std::rc::Rc;

use dear_imgui_rs::render::ReconciledFrame;
use dear_imgui_rs::{
    Context, ContextAttachment, ContextAttachmentError, ContextAttachmentLease,
    ContextAttachmentRole, ContextAttachmentTeardownError, ContextBinding, ContextBindingError,
    ContextDestroyed, ContextId, ContextLifecycle, ContextTeardown, FrameToken,
};
use thiserror::Error;

use super::callbacks::{
    claim_callbacks, destroy_renderer_viewport_resources, detect_runtime_contract_drift,
    preflight_callbacks, preflight_renderer_viewport_resources, release_callbacks,
    revoke_renderer_viewport_capability_if_owned,
};
use super::registry::{
    GlobalHandles, drop_orphaned_viewport_data, preflight_runtime, register_runtime,
    renderer_globals, unregister_runtime,
};
use super::trace::{FrameTraceState, ObservationKey, WgpuViewportFrameReport};
use super::work::{RecordedTarget, WgpuRecordedViewportWork, WgpuViewportWorkError};
use crate::{
    ExternalTextureBinding, ExternalTextureId, FramebufferExtent, GammaMode, RendererError,
    WgpuDrawRecording, WgpuFrameScope, WgpuFrameSkipReason, WgpuRenderer, WgpuViewportObservation,
};

struct WgpuRendererAttachmentMarker;

/// Failure to attach or operate an owning WGPU multi-viewport runtime.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WgpuViewportError {
    /// The Dear ImGui Context rejected the renderer attachment.
    #[error(transparent)]
    Attachment(#[from] ContextAttachmentError),
    /// The originating Context can no longer be entered normally.
    #[error(transparent)]
    Context(#[from] ContextBindingError),
    /// The underlying WGPU renderer operation failed.
    #[error(transparent)]
    Renderer(#[from] RendererError),
    /// The renderer and runtime Context identities differ.
    #[error("WGPU viewport runtime belongs to Context {expected:?}, not {actual:?}")]
    ContextMismatch {
        expected: ContextId,
        actual: ContextId,
    },
    /// A prepared frame was passed to a different runtime instance.
    #[error(
        "WGPU prepared frame belongs to another runtime instance (expected Context {expected:?}, frame Context {actual:?})"
    )]
    PreparedFrameRuntimeMismatch {
        expected: ContextId,
        actual: ContextId,
    },
    /// A callback entry was not running under this runtime's Context.
    #[error("the current Dear ImGui Context does not match WGPU runtime Context {expected:?}")]
    BoundContextMismatch { expected: ContextId },
    /// The renderer has not been initialized with GPU backend data.
    #[error("WGPU renderer is not initialized")]
    RendererNotInitialized,
    /// The renderer was initialized for another Dear ImGui Context.
    #[error("WGPU renderer is bound to a different Dear ImGui Context")]
    RendererContextMismatch,
    /// The renderer's bound Dear ImGui Context no longer exists.
    #[error("the Dear ImGui Context bound to this WGPU renderer has been dropped")]
    RendererContextDropped,
    /// Per-window surfaces require the WGPU instance used by the renderer.
    #[error("WGPU multi-viewport requires WgpuInitInfo::with_instance")]
    MissingInstance,
    /// Surface capability negotiation requires the WGPU adapter used by the renderer.
    #[error("WGPU multi-viewport requires WgpuInitInfo::with_adapter")]
    MissingAdapter,
    /// Renderer callbacks require an attached platform backend that supports viewports.
    #[error("WGPU multi-viewport requires an attached multi-viewport platform runtime")]
    PlatformBackendUnavailable,
    /// The typed platform owner supplied to the renderer belongs to another Context.
    #[error("WGPU viewport platform owner belongs to Context {actual:?}, not {expected:?}")]
    PlatformOwnerContextMismatch {
        expected: ContextId,
        actual: ContextId,
    },
    /// The typed Winit platform owner rejected renderer attachment.
    #[cfg(feature = "multi-viewport-winit")]
    #[error(transparent)]
    WinitPlatformOwner(#[from] dear_imgui_winit::WinitPlatformError),
    /// The typed SDL3 platform owner rejected renderer attachment.
    #[cfg(feature = "multi-viewport-sdl3")]
    #[error(transparent)]
    Sdl3PlatformOwner(#[from] dear_imgui_sdl3::Sdl3BackendError),
    /// A required platform callback is absent.
    #[error("required ImGuiPlatformIO callback `{callback}` is not installed")]
    PlatformCallbackUnavailable { callback: &'static str },
    /// PlatformIO itself is unavailable for the currently bound Context.
    #[error("the bound Dear ImGui Context has no PlatformIO")]
    PlatformIoUnavailable,
    /// The platform runtime has not published the main native window handle.
    #[error("the attached platform runtime has no main viewport window handle")]
    MainViewportHandleUnavailable,
    /// Another renderer already owns one renderer callback slot.
    #[error("ImGuiPlatformIO callback `{callback}` is already owned by another renderer")]
    RendererCallbackOccupied { callback: &'static str },
    /// Another renderer already advertises multi-viewport support.
    #[error("another renderer already advertises RENDERER_HAS_VIEWPORTS")]
    RendererViewportCapabilityOccupied,
    /// This runtime's renderer capability bit was cleared while it remained attached.
    #[error("WGPU renderer backend flag RENDERER_HAS_VIEWPORTS was removed while attached")]
    RendererViewportCapabilityLost,
    /// A callback claimed by this runtime was replaced while attached.
    #[error("WGPU renderer callback `{callback}` was replaced while the runtime was attached")]
    RendererCallbackReplaced { callback: &'static str },
    /// A secondary viewport already contains renderer-owned user data.
    #[error("a secondary viewport already has RendererUserData owned by another backend")]
    RendererUserDataOccupied,
    /// A callback observed non-null renderer data that is absent from this runtime's sidecar.
    #[error("WGPU callback `{callback}` observed foreign or unregistered RendererUserData")]
    RendererUserDataOwnershipLost { callback: &'static str },
    /// Existing platform windows would miss the renderer create callback.
    #[error("secondary platform windows already exist; destroy them before attaching WGPU")]
    PlatformWindowsAlreadyCreated,
    /// The aggregate size callback cannot be bridged safely by this Dear ImGui artifact.
    #[error("dear-imgui-sys was built without PlatformIO aggregate ABI hooks")]
    AggregateCallbackHooksUnavailable,
    /// The registry already contains a live runtime for this Context.
    #[error("a WGPU viewport runtime is already attached to this Context")]
    RuntimeAlreadyAttached,
    /// The owning runtime has shut down or Context-owned teardown has started.
    #[error("the WGPU viewport runtime is no longer attached")]
    RuntimeDetached,
    /// The renderer is already mutably borrowed by another runtime entry.
    #[error("WGPU renderer runtime is already active in `{callback}`")]
    CallbackReentered { callback: &'static str },
    /// A callback panic was contained at the C ABI boundary.
    #[error("WGPU renderer callback `{callback}` panicked")]
    CallbackPanicked { callback: &'static str },
    /// Dear ImGui passed an invalid viewport to a renderer callback.
    #[error("WGPU renderer callback `{callback}` received a null viewport")]
    InvalidViewport { callback: &'static str },
    /// Creating or configuring a viewport surface failed.
    #[error("WGPU viewport surface operation `{operation}` failed")]
    SurfaceOperationFailed { operation: &'static str },
    /// The renderer's target format is unavailable with its required secondary-surface encoding.
    #[error(
        "WGPU render target format {format:?} is unavailable for secondary viewports in {color_space}"
    )]
    UnsupportedSurfaceFormat {
        format: wgpu::TextureFormat,
        color_space: &'static str,
    },
    /// The renderer's multisample count cannot describe a WGPU render attachment.
    #[error("WGPU viewport attachments require a non-zero multisample count, received {count}")]
    InvalidMultisampleCount { count: u32 },
    /// A renderer attachment format does not support the configured sample count.
    #[error(
        "WGPU {attachment} format {format:?} does not support RENDER_ATTACHMENT at sample count {sample_count}"
    )]
    UnsupportedViewportAttachment {
        attachment: &'static str,
        format: wgpu::TextureFormat,
        sample_count: u32,
    },
    /// Surface acquisition returned a terminal result.
    #[error("WGPU viewport surface acquisition was rejected: {event}")]
    SurfaceRejected { event: &'static str },
    /// Native multi-viewport surfaces are unavailable on this target.
    #[error("WGPU native multi-viewport rendering is unavailable on this target")]
    UnsupportedTarget,
    /// A frame trace is already active for this runtime.
    #[error("a WGPU secondary-viewport frame trace is already active")]
    FrameTraceAlreadyActive,
}

/// One failure observed while preparing a WGPU multi-viewport route.
#[doc(hidden)]
#[derive(Debug)]
#[non_exhaustive]
pub enum WgpuViewportRouteFault<PlatformError> {
    /// Texture reconciliation or a renderer callback failed.
    Renderer(WgpuViewportError),
    /// The platform owner reported a deferred native-window fault.
    Platform(PlatformError),
}

impl<PlatformError> fmt::Display for WgpuViewportRouteFault<PlatformError>
where
    PlatformError: fmt::Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Renderer(error) => write!(formatter, "WGPU viewport route failed: {error}"),
            Self::Platform(error) => write!(formatter, "viewport platform route failed: {error}"),
        }
    }
}

impl<PlatformError> std::error::Error for WgpuViewportRouteFault<PlatformError>
where
    PlatformError: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Renderer(error) => Some(error),
            Self::Platform(error) => Some(error),
        }
    }
}

/// Ordered failures from one WGPU multi-viewport preparation transaction.
///
/// Renderer failures are reported before platform failures observed after the same native
/// callback pump. All failures from each source retain FIFO order.
#[doc(hidden)]
#[derive(Debug)]
pub struct WgpuViewportRouteError<PlatformError> {
    faults: Vec<WgpuViewportRouteFault<PlatformError>>,
    report: WgpuViewportFrameReport,
}

impl<PlatformError> WgpuViewportRouteError<PlatformError> {
    pub(crate) fn new(
        faults: Vec<WgpuViewportRouteFault<PlatformError>>,
        report: WgpuViewportFrameReport,
    ) -> Self {
        debug_assert!(!faults.is_empty());
        Self { faults, report }
    }

    /// Returns all observations completed before or alongside these faults.
    pub fn report(&self) -> &WgpuViewportFrameReport {
        &self.report
    }

    /// Returns the partial report and ordered faults without discarding either.
    pub fn into_parts(
        self,
    ) -> (
        WgpuViewportFrameReport,
        Vec<WgpuViewportRouteFault<PlatformError>>,
    ) {
        (self.report, self.faults)
    }

    /// Returns every route fault in reporting order.
    #[must_use]
    pub fn faults(&self) -> &[WgpuViewportRouteFault<PlatformError>] {
        &self.faults
    }

    /// Consumes the aggregate and returns every route fault in reporting order.
    #[must_use]
    pub fn into_faults(self) -> Vec<WgpuViewportRouteFault<PlatformError>> {
        self.faults
    }
}

impl<PlatformError> fmt::Display for WgpuViewportRouteError<PlatformError>
where
    PlatformError: fmt::Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let count = self.faults.len();
        write!(formatter, "{}", self.faults[0])?;
        if count > 1 {
            write!(formatter, " ({count} viewport route faults in total)")?;
        }
        Ok(())
    }
}

impl<PlatformError> std::error::Error for WgpuViewportRouteError<PlatformError>
where
    PlatformError: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.faults
            .first()
            .map(|fault| fault as &(dyn std::error::Error + 'static))
    }
}

/// Explicit preparation result; errors retain observations without exposing a drawable capability.
pub(crate) struct PreparationAttempt<Prepared> {
    pub(crate) result: Result<Prepared, Vec<WgpuViewportError>>,
    pub(crate) report: WgpuViewportFrameReport,
}

impl<Prepared> PreparationAttempt<Prepared> {
    fn failed(error: WgpuViewportError) -> Self {
        Self {
            result: Err(vec![error]),
            report: WgpuViewportFrameReport::default(),
        }
    }
}

pub(crate) fn finish_route_preparation<Prepared, PlatformError>(
    renderer_attempt: Option<PreparationAttempt<Prepared>>,
    renderer_faults: Vec<WgpuViewportError>,
    platform_faults: Vec<PlatformError>,
) -> Result<(Prepared, WgpuViewportFrameReport), WgpuViewportRouteError<PlatformError>> {
    let mut faults = Vec::new();
    let mut report = WgpuViewportFrameReport::default();
    let prepared = match renderer_attempt {
        Some(attempt) => {
            report = attempt.report;
            match attempt.result {
                Ok(prepared) => Some(prepared),
                Err(errors) => {
                    faults.extend(errors.into_iter().map(WgpuViewportRouteFault::Renderer));
                    None
                }
            }
        }
        None => None,
    };
    faults.extend(
        renderer_faults
            .into_iter()
            .map(WgpuViewportRouteFault::Renderer),
    );
    faults.extend(
        platform_faults
            .into_iter()
            .map(WgpuViewportRouteFault::Platform),
    );
    if !faults.is_empty() {
        return Err(WgpuViewportRouteError::new(faults, report));
    }
    match prepared {
        Some(prepared) => Ok((prepared, report)),
        None => Err(WgpuViewportRouteError::new(
            vec![WgpuViewportRouteFault::Renderer(
                WgpuViewportError::RuntimeDetached,
            )],
            report,
        )),
    }
}

/// Runs a public route transaction only when its frame belongs to the attached Context.
///
/// The callback contains every platform-adapter entry and fault-queue read. Keeping it behind this
/// pure identity check makes foreign-frame rejection observably side-effect free.
pub(crate) fn prepare_route_for_context<Prepared, PlatformError>(
    expected: ContextId,
    actual: ContextId,
    prepare: impl FnOnce() -> Result<Prepared, WgpuViewportRouteError<PlatformError>>,
) -> Result<Prepared, WgpuViewportRouteError<PlatformError>> {
    if expected != actual {
        return Err(WgpuViewportRouteError::new(
            vec![WgpuViewportRouteFault::Renderer(
                WgpuViewportError::ContextMismatch { expected, actual },
            )],
            WgpuViewportFrameReport::default(),
        ));
    }
    prepare()
}

/// Transactional attachment failure that returns the renderer unchanged.
pub struct WgpuViewportAttachError {
    error: WgpuViewportError,
    renderer: Box<WgpuRenderer>,
}

impl WgpuViewportAttachError {
    pub(crate) fn new(error: WgpuViewportError, renderer: WgpuRenderer) -> Self {
        Self {
            error,
            renderer: Box::new(renderer),
        }
    }

    /// Returns the reason attachment failed.
    pub fn error(&self) -> &WgpuViewportError {
        &self.error
    }

    /// Returns the renderer so the caller can retry, use it for one viewport, or destroy it.
    pub fn into_renderer(self) -> WgpuRenderer {
        *self.renderer
    }

    /// Returns both the typed failure and the unchanged renderer.
    pub fn into_parts(self) -> (WgpuViewportError, WgpuRenderer) {
        (self.error, *self.renderer)
    }
}

impl fmt::Debug for WgpuViewportAttachError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WgpuViewportAttachError")
            .field("error", &self.error)
            .field("renderer", &"returned to caller")
            .finish()
    }
}

impl fmt::Display for WgpuViewportAttachError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for WgpuViewportAttachError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RuntimeState {
    Constructing,
    Attached,
    ShuttingDown,
    Detached,
    ResourceDropped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CallbackState {
    Unclaimed,
    Claimed,
    Released,
}

enum ShutdownAction<'a> {
    Quiesce,
    Explicit(&'a mut Context),
}

/// Exact identity for one attached renderer runtime.
///
/// The identity is intentionally reference-counted with [`Rc`] because the owning runtime and
/// every prepared frame remain on the UI thread.
#[derive(Debug)]
struct RuntimeIdentity;

impl RuntimeIdentity {
    fn new() -> Rc<Self> {
        Rc::new(Self)
    }
}

pub(super) struct RuntimeControl {
    context_raw: *mut dear_imgui_rs::sys::ImGuiContext,
    binding: ContextBinding,
    identity: Rc<RuntimeIdentity>,
    state: Cell<RuntimeState>,
    renderer: RefCell<Option<Box<WgpuRenderer>>>,
    globals: RefCell<Option<GlobalHandles>>,
    attachment: RefCell<Option<ContextAttachmentLease>>,
    callback_state: Cell<CallbackState>,
    faults: RefCell<RuntimeFaults>,
    frame_trace: RefCell<FrameTraceState>,
    #[cfg(test)]
    panic_next_callback: Cell<bool>,
    #[cfg(test)]
    fail_next_viewport_cleanup: Cell<bool>,
    #[cfg(test)]
    transitions: RefCell<Vec<&'static str>>,
}

#[derive(Default)]
struct RuntimeFaults {
    pending: VecDeque<WgpuViewportError>,
    terminal_recorded: bool,
}

impl RuntimeFaults {
    fn record_terminal(&mut self, fault: WgpuViewportError) {
        if !self.terminal_recorded {
            self.pending.push_back(fault);
            self.terminal_recorded = true;
        }
    }

    fn record_non_terminal(&mut self, fault: WgpuViewportError) {
        self.pending.push_back(fault);
    }

    fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    fn take_next(&mut self) -> Option<WgpuViewportError> {
        self.pending.pop_front()
    }

    fn drain(&mut self) -> Vec<WgpuViewportError> {
        self.pending.drain(..).collect()
    }
}

impl fmt::Debug for RuntimeControl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeControl")
            .field("context", &self.binding.id())
            .field("state", &self.state.get())
            .field("has_renderer", &self.renderer.borrow().is_some())
            .field("callback_state", &self.callback_state.get())
            .finish_non_exhaustive()
    }
}

impl RuntimeControl {
    fn new(context: &Context, renderer: WgpuRenderer, globals: Option<GlobalHandles>) -> Self {
        Self {
            context_raw: context.as_raw(),
            binding: context.binding(),
            identity: RuntimeIdentity::new(),
            state: Cell::new(RuntimeState::Constructing),
            renderer: RefCell::new(Some(Box::new(renderer))),
            globals: RefCell::new(globals),
            attachment: RefCell::new(None),
            callback_state: Cell::new(CallbackState::Unclaimed),
            faults: RefCell::new(RuntimeFaults::default()),
            frame_trace: RefCell::new(FrameTraceState::default()),
            #[cfg(test)]
            panic_next_callback: Cell::new(false),
            #[cfg(test)]
            fail_next_viewport_cleanup: Cell::new(false),
            #[cfg(test)]
            transitions: RefCell::new(Vec::new()),
        }
    }

    pub(super) fn context_raw(&self) -> *mut dear_imgui_rs::sys::ImGuiContext {
        self.context_raw
    }

    pub(super) fn binding(&self) -> &ContextBinding {
        &self.binding
    }

    pub(super) fn globals(&self) -> Option<GlobalHandles> {
        self.globals.borrow().clone()
    }

    pub(super) fn is_callback_accessible(&self) -> bool {
        self.state.get() == RuntimeState::Attached
            && self.callback_state.get() != CallbackState::Released
    }

    pub(super) fn is_cleanup_callback_accessible(&self) -> bool {
        matches!(
            self.state.get(),
            RuntimeState::Attached | RuntimeState::ShuttingDown
        ) && self.callback_state.get() == CallbackState::Claimed
    }

    pub(super) fn should_detect_callback_drift(&self) -> bool {
        self.state.get() == RuntimeState::Attached
            && self.callback_state.get() == CallbackState::Claimed
    }

    pub(super) fn callback_released(&self) -> bool {
        self.callback_state.get() == CallbackState::Released
    }

    pub(super) fn mark_callback_claimed(&self) {
        self.callback_state.set(CallbackState::Claimed);
    }

    pub(super) fn mark_callback_released(&self) {
        self.callback_state.set(CallbackState::Released);
        unregister_runtime(self.binding.id());
    }

    fn set_state(&self, state: RuntimeState) {
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

    pub(super) fn begin_shutdown(&self) {
        if matches!(
            self.state.get(),
            RuntimeState::Constructing | RuntimeState::Attached
        ) {
            self.set_state(RuntimeState::ShuttingDown);
        }
    }

    fn mark_detached(&self) {
        if !matches!(
            self.state.get(),
            RuntimeState::Detached | RuntimeState::ResourceDropped
        ) {
            self.set_state(RuntimeState::Detached);
        }
        unregister_runtime(self.binding.id());
    }

    pub(super) fn record_fault(&self, fault: WgpuViewportError) {
        self.faults.borrow_mut().record_non_terminal(fault);
    }

    fn record_terminal_fault(&self, fault: WgpuViewportError) {
        self.faults.borrow_mut().record_terminal(fault);
    }

    fn begin_frame_trace(&self, scope: WgpuFrameScope) -> Result<(), WgpuViewportError> {
        self.ensure_entry()?;
        if self.frame_trace.borrow_mut().begin(scope) {
            Ok(())
        } else {
            Err(WgpuViewportError::FrameTraceAlreadyActive)
        }
    }

    fn finish_frame_trace(&self) -> WgpuViewportFrameReport {
        self.frame_trace.borrow_mut().finish()
    }

    fn finish_recording_trace(&self) -> (WgpuViewportFrameReport, Vec<RecordedTarget>) {
        self.frame_trace.borrow_mut().finish_recording()
    }

    pub(super) fn push_recorded(&self, target: RecordedTarget) {
        self.frame_trace.borrow_mut().push_recorded(target);
    }

    pub(super) fn observe_dependencies(
        &self,
        key: Option<ObservationKey>,
        dependencies: crate::WgpuDrawDependencies,
    ) {
        self.frame_trace
            .borrow_mut()
            .dependencies(key, dependencies);
    }

    fn abort_frame_trace(&self) {
        self.frame_trace.borrow_mut().abort();
    }

    pub(super) fn start_observation(
        &self,
        target: dear_imgui_rs::NativeWindowTarget,
    ) -> Option<ObservationKey> {
        self.frame_trace.borrow_mut().start(target)
    }

    pub(super) fn observe_target(
        &self,
        key: Option<ObservationKey>,
        target: dear_imgui_rs::NativeWindowTarget,
    ) {
        self.frame_trace.borrow_mut().target(key, target);
    }

    pub(super) fn observation_scope(&self) -> Option<WgpuFrameScope> {
        self.frame_trace.borrow().scope()
    }

    pub(super) fn observe_recording(
        &self,
        key: Option<ObservationKey>,
        recording: WgpuDrawRecording,
    ) {
        self.frame_trace.borrow_mut().recording(key, recording);
    }

    #[cfg(test)]
    pub(super) fn observe_stage(&self, key: Option<ObservationKey>, stage: crate::WgpuFrameStage) {
        self.frame_trace.borrow_mut().stage(key, stage);
    }

    pub(super) fn observe_skip(&self, key: Option<ObservationKey>, reason: WgpuFrameSkipReason) {
        self.frame_trace.borrow_mut().skip(key, reason);
    }

    pub(super) fn observe_failure(
        &self,
        key: Option<ObservationKey>,
        operation: &'static str,
        error: &impl fmt::Display,
    ) {
        self.frame_trace.borrow_mut().fail(key, operation, error);
    }

    pub(super) fn record_runtime_contract_fault(&self, fault: WgpuViewportError) {
        let _ = self.binding.try_with_bound_context(|| {
            revoke_renderer_viewport_capability_if_owned(self);
        });
        self.record_terminal_fault(fault);
        self.begin_shutdown();
    }

    /// Returns whether this runtime can still prove a WGPU core renderer publication on the
    /// bound Context. A reentrant mutable borrow is not proof of ownership, so it preserves the
    /// shared capability bit until a later teardown can inspect the exact publications.
    pub(super) fn owns_core_renderer_publication_bound(&self) -> bool {
        let Ok(renderer) = self.renderer.try_borrow() else {
            return false;
        };
        renderer
            .as_deref()
            .is_some_and(WgpuRenderer::owns_context_publication_bound)
    }

    pub(super) fn record_entry_fault(&self, fault: WgpuViewportError) {
        if matches!(
            &fault,
            WgpuViewportError::Renderer(RendererError::RendererStateDrift { .. })
                | WgpuViewportError::RendererUserDataOwnershipLost { .. }
                | WgpuViewportError::CallbackPanicked { .. }
                | WgpuViewportError::SurfaceRejected { .. }
        ) {
            self.record_runtime_contract_fault(fault);
        } else {
            self.record_fault(fault);
        }
    }

    pub(super) fn core_renderer_contract_fault(&self) -> Option<WgpuViewportError> {
        let renderer = match self.renderer.try_borrow() {
            Ok(renderer) => renderer,
            Err(_) => {
                return Some(WgpuViewportError::CallbackReentered {
                    callback: "renderer contract validation",
                });
            }
        };
        let renderer = renderer
            .as_deref()
            .ok_or(WgpuViewportError::RuntimeDetached);
        match renderer {
            Ok(renderer) => renderer.ensure_renderer_contract().err().map(Into::into),
            Err(error) => Some(error),
        }
    }

    fn detect_and_take_fault(&self) -> Option<WgpuViewportError> {
        detect_runtime_contract_drift(self);
        self.faults.borrow_mut().take_next()
    }

    fn detect_and_drain_faults(&self) -> Vec<WgpuViewportError> {
        detect_runtime_contract_drift(self);
        self.faults.borrow_mut().drain()
    }

    fn ensure_context(&self, context: &Context) -> Result<(), WgpuViewportError> {
        if context.id() == self.binding.id() {
            Ok(())
        } else {
            Err(WgpuViewportError::ContextMismatch {
                expected: self.binding.id(),
                actual: context.id(),
            })
        }
    }

    fn ensure_entry(&self) -> Result<(), WgpuViewportError> {
        if let Some(fault) = self.detect_and_take_fault() {
            return Err(fault);
        }
        if self.state.get() == RuntimeState::Attached {
            Ok(())
        } else {
            Err(WgpuViewportError::RuntimeDetached)
        }
    }

    fn finish_entry(&self) -> Result<(), WgpuViewportError> {
        self.detect_and_take_fault().map_or(Ok(()), Err)
    }

    fn with_renderer_mut<R>(
        &self,
        callback: impl FnOnce(&mut WgpuRenderer) -> Result<R, WgpuViewportError>,
    ) -> Result<R, WgpuViewportError> {
        self.ensure_entry()?;
        let result = {
            let mut renderer = self.renderer.try_borrow_mut().map_err(|_| {
                WgpuViewportError::CallbackReentered {
                    callback: "Rust runtime entry",
                }
            })?;
            let renderer = renderer
                .as_deref_mut()
                .ok_or(WgpuViewportError::RuntimeDetached)?;
            callback(renderer)
        };
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                if matches!(
                    &error,
                    WgpuViewportError::Renderer(RendererError::RendererStateDrift { .. })
                ) {
                    self.record_runtime_contract_fault(error);
                    return Err(self
                        .detect_and_take_fault()
                        .unwrap_or(WgpuViewportError::RuntimeDetached));
                }
                return Err(error);
            }
        };
        self.finish_entry()?;
        Ok(result)
    }

    pub(super) fn with_renderer_callback(
        &self,
        callback_name: &'static str,
        callback: impl FnOnce(&mut WgpuRenderer, &GlobalHandles) -> Result<(), WgpuViewportError>,
    ) -> Result<(), WgpuViewportError> {
        detect_runtime_contract_drift(self);
        if self.state.get() != RuntimeState::Attached || self.faults.borrow().has_pending() {
            return Err(WgpuViewportError::RuntimeDetached);
        }
        let mut renderer =
            self.renderer
                .try_borrow_mut()
                .map_err(|_| WgpuViewportError::CallbackReentered {
                    callback: callback_name,
                })?;
        let renderer = renderer
            .as_deref_mut()
            .ok_or(WgpuViewportError::RuntimeDetached)?;
        let globals = self.globals().ok_or(WgpuViewportError::RuntimeDetached)?;
        callback(renderer, &globals)
    }

    fn preflight_renderer_shutdown(&self, context: &mut Context) -> Result<(), WgpuViewportError> {
        let renderer =
            self.renderer
                .try_borrow()
                .map_err(|_| WgpuViewportError::CallbackReentered {
                    callback: "WGPU viewport runtime shutdown preflight",
                })?;
        renderer
            .as_deref()
            .ok_or(WgpuViewportError::RuntimeDetached)?
            .preflight_shutdown(context)
            .map_err(Into::into)
    }

    fn release_renderer_explicit(&self, context: &mut Context) -> Result<(), WgpuViewportError> {
        if self.renderer.borrow().is_none() {
            self.globals.borrow_mut().take();
            self.set_state(RuntimeState::ResourceDropped);
            return Ok(());
        }
        let mut renderer =
            self.renderer
                .try_borrow_mut()
                .map_err(|_| WgpuViewportError::CallbackReentered {
                    callback: "WGPU viewport runtime shutdown",
                })?;
        renderer
            .as_deref_mut()
            .ok_or(WgpuViewportError::RuntimeDetached)?
            .shutdown(context)?;
        let renderer = renderer.take();
        drop(renderer);
        self.globals.borrow_mut().take();
        self.set_state(RuntimeState::ResourceDropped);
        Ok(())
    }

    fn release_renderer_during_context_teardown(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        if self.state.get() == RuntimeState::ResourceDropped {
            return Ok(());
        }

        // Do not acquire the Context reset transaction until every visible sidecar still proves
        // exact ownership. This is read-only, so either failure leaves callback publication,
        // renderer resources, and the consumer intact for Context's fail-stop path.
        preflight_renderer_viewport_resources(self).map_err(|error| {
            let message = error.to_string();
            self.record_fault(error);
            ContextAttachmentTeardownError::new(message)
        })?;

        if self.renderer.borrow().is_none() {
            self.mark_detached();
            self.globals.borrow_mut().take();
            self.set_state(RuntimeState::ResourceDropped);
            return Ok(());
        }

        let mut renderer_slot = self.renderer.try_borrow_mut().map_err(|_| {
            ContextAttachmentTeardownError::new(
                "WGPU renderer was reentered during Context renderer-resource teardown",
            )
        })?;
        let renderer = renderer_slot.as_deref_mut().ok_or_else(|| {
            ContextAttachmentTeardownError::new(
                "WGPU renderer disappeared during Context renderer-resource teardown",
            )
        })?;

        renderer.shutdown_during_context_teardown(context, || {
            self.begin_shutdown();
            destroy_renderer_viewport_resources(self).map_err(|error| {
                let message = error.to_string();
                self.record_fault(error);
                ContextAttachmentTeardownError::new(message)
            })?;
            release_callbacks(self).map_err(|error| {
                let message = error.to_string();
                self.record_fault(error);
                ContextAttachmentTeardownError::new(message)
            })?;
            self.mark_detached();
            Ok(())
        })?;

        let renderer = renderer_slot.take();
        drop(renderer);
        self.globals.borrow_mut().take();
        self.set_state(RuntimeState::ResourceDropped);
        Ok(())
    }

    fn shutdown_once(&self, mut action: ShutdownAction<'_>) -> Result<(), WgpuViewportError> {
        if self.state.get() == RuntimeState::ResourceDropped {
            self.detach_attachment();
            return Ok(());
        }

        // An explicit shutdown is retryable. Validate that no live renderer epoch prevents the
        // reset before mutating viewport sidecars, surfaces, callbacks, or runtime state.
        if let ShutdownAction::Explicit(context) = &mut action {
            self.preflight_renderer_shutdown(context)?;
        }

        // A foreign write to one sidecar makes the active DestroyWindow callback the only safe
        // reclaim path. Verify every reachable slot before changing the runtime state, dropping
        // any sidecar, or clearing callback publication.
        preflight_renderer_viewport_resources(self)?;

        self.begin_shutdown();
        let viewport_result = if matches!(action, ShutdownAction::Quiesce) {
            Ok(())
        } else {
            destroy_renderer_viewport_resources(self)
        };
        let mut viewport_error = viewport_result.err();
        if matches!(action, ShutdownAction::Explicit(_))
            && let Some(error) = viewport_error.take()
        {
            return Err(error);
        }
        let callback_result = release_callbacks(self);

        match action {
            ShutdownAction::Quiesce => callback_result,
            ShutdownAction::Explicit(context) => {
                self.mark_detached();
                let renderer_result = self.release_renderer_explicit(context);
                if self.state.get() == RuntimeState::ResourceDropped {
                    self.detach_attachment();
                }
                first_error([viewport_error, callback_result.err(), renderer_result.err()])
            }
        }
    }

    fn owner_dropped(&self) {
        if self.state.get() == RuntimeState::ResourceDropped {
            return;
        }
        match self.binding.lifecycle() {
            // Drop lacks an exclusive `&mut Context`, so it cannot prepare and commit the
            // renderer-texture reset transaction. Keep the renderer, sidecars, callback table,
            // and attachment alive for Context's ordered terminal teardown.
            ContextLifecycle::Alive => self.defer_attachment_to_context(),
            // Context already owns this attachment and is currently executing its teardown
            // phases. Entering native code here would violate that ordering.
            ContextLifecycle::Dropping => {}
            // Context teardown normally calls `context_destroyed` before the wrapper can be
            // released. This idempotent fallback touches only Rust-owned allocations and never
            // tries to make a destroyed native Context current.
            ContextLifecycle::NativeDestroyed => self.mark_context_destroyed(),
            _ => {}
        }
    }

    fn store_attachment(&self, attachment: ContextAttachmentLease) {
        self.attachment.borrow_mut().replace(attachment);
    }

    fn detach_attachment(&self) {
        if let Some(mut attachment) = self.attachment.borrow_mut().take() {
            let _ = attachment
                .detach()
                .expect("a renderer attachment cannot have a platform release dependency");
        }
    }

    fn defer_attachment_to_context(&self) {
        if let Some(attachment) = self.attachment.borrow_mut().take() {
            attachment.defer_to_context();
        }
    }

    fn recover_renderer(&self) -> WgpuRenderer {
        self.globals.borrow_mut().take();
        *self
            .renderer
            .borrow_mut()
            .take()
            .expect("failed WGPU runtime construction lost its renderer")
    }

    fn mark_context_destroyed(&self) {
        unregister_runtime(self.binding.id());
        // Native viewport slots are no longer touched after Context destruction, but the sidecar
        // still owns every remaining renderer allocation.
        drop_orphaned_viewport_data(self.binding.id());
        if self.renderer.borrow().is_some() {
            let mut renderer = self.renderer.borrow_mut();
            if let Some(renderer) = renderer.as_deref_mut() {
                renderer.shutdown_after_context_destroyed();
            }
            renderer.take();
            self.globals.borrow_mut().take();
        }
        self.attachment.borrow_mut().take();
        self.set_state(RuntimeState::ResourceDropped);
    }

    #[cfg(test)]
    pub(super) fn state(&self) -> RuntimeState {
        self.state.get()
    }

    #[cfg(test)]
    pub(super) fn borrow_renderer_for_test(
        &self,
    ) -> std::cell::RefMut<'_, Option<Box<WgpuRenderer>>> {
        self.renderer.borrow_mut()
    }

    #[cfg(test)]
    pub(super) fn has_renderer_for_test(&self) -> bool {
        self.renderer.borrow().is_some()
    }

    #[cfg(test)]
    pub(super) fn renderer_address_for_test(&self) -> *const WgpuRenderer {
        self.renderer
            .borrow()
            .as_deref()
            .map_or(std::ptr::null(), std::ptr::from_ref)
    }

    #[cfg(test)]
    pub(super) fn panic_next_callback_for_test(&self) {
        self.panic_next_callback.set(true);
    }

    #[cfg(test)]
    pub(super) fn maybe_panic_callback_for_test(&self) {
        assert!(
            !self.panic_next_callback.replace(false),
            "injected WGPU viewport callback panic"
        );
    }

    #[cfg(test)]
    pub(super) fn fail_next_viewport_cleanup_for_test(&self) {
        self.fail_next_viewport_cleanup.set(true);
    }

    #[cfg(test)]
    pub(super) fn take_viewport_cleanup_failure_for_test(&self) -> bool {
        self.fail_next_viewport_cleanup.replace(false)
    }

    #[cfg(test)]
    pub(super) fn transition_log_for_test(&self) -> Vec<&'static str> {
        self.transitions.borrow().clone()
    }
}

impl ContextAttachment for RuntimeControl {
    fn quiesce(&self, context: &ContextTeardown<'_>) -> Result<(), ContextAttachmentTeardownError> {
        context.with_bound_context(|| {
            self.shutdown_once(ShutdownAction::Quiesce)
                .map_err(|error| {
                    let message = error.to_string();
                    self.record_fault(error);
                    ContextAttachmentTeardownError::new(message)
                })
        })
    }

    fn release_renderer_resources(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        context.with_bound_context(|| self.release_renderer_during_context_teardown(context))
    }

    fn context_destroyed(&self, _context: ContextDestroyed) {
        self.mark_context_destroyed();
    }
}

/// Backend-local owning runtime shared by the Winit and SDL3 typed wrappers.
pub(crate) struct OwningViewportRuntime {
    control: Rc<RuntimeControl>,
}

/// A non-nestable trace scope for one secondary-viewport rendering pass.
///
/// Call [`Self::finish`] before acquiring or presenting the application's main surface to obtain
/// a report that proves which secondary surfaces completed renderer submission and presentation
/// within this scope. Dropping the guard discards the partial trace.
#[must_use = "finish the frame trace to obtain its report"]
pub(super) struct FrameTraceGuard<'runtime> {
    control: &'runtime RuntimeControl,
    active: bool,
}

/// Reconciled UI frame whose native windows were updated, with no acquired or recorded targets.
///
/// Pass this move-only capability to the same route's `record` method. Dropping it abandons the
/// frame without submitting or presenting anything.
#[must_use = "record or explicitly drop the prepared viewport frame"]
pub struct WgpuPreparedViewportFrame<'frame> {
    prepared: PreparedMainFrame<'frame>,
    report: WgpuViewportFrameReport,
}

pub(crate) struct PreparedMainFrame<'frame> {
    frame: ReconciledFrame<'frame>,
    runtime: Rc<RuntimeIdentity>,
}

impl<'frame> WgpuPreparedViewportFrame<'frame> {
    pub(crate) fn new(
        prepared: PreparedMainFrame<'frame>,
        report: WgpuViewportFrameReport,
    ) -> Self {
        Self { prepared, report }
    }
    /// Context that owns the frame.
    pub fn context_id(&self) -> ContextId {
        self.prepared.frame.context_id()
    }
    /// Observations from native-window preparation, before recording or submission.
    pub fn report(&self) -> &WgpuViewportFrameReport {
        &self.report
    }
}

/// Secondary command buffers plus the still-borrowed main UI frame.
///
/// Split this value to submit/present secondary work independently of main-surface acquisition.
#[must_use = "split, complete, or explicitly discard the recorded frame"]
pub struct WgpuRecordedViewportFrame<'frame> {
    main: WgpuPreparedMainFrame<'frame>,
    secondary: WgpuRecordedViewportWork,
}

impl<'frame> WgpuRecordedViewportFrame<'frame> {
    /// Keeps the main Context borrow separate from owned secondary command buffers and leases.
    pub fn split(self) -> (WgpuPreparedMainFrame<'frame>, WgpuRecordedViewportWork) {
        (self.main, self.secondary)
    }
    /// Observations after recording, before submission.
    pub fn report(&self) -> &WgpuViewportFrameReport {
        self.secondary.report()
    }
    /// Convenience driver using the exact same recording/submission/presentation ownership model.
    pub fn complete_secondary(
        self,
        producer_commands: impl IntoIterator<Item = wgpu::CommandBuffer>,
    ) -> Result<WgpuPreparedMainFrame<'frame>, WgpuViewportWorkError> {
        let (mut main, work) = self.split();
        let pending = work.submit(producer_commands)?;
        main.secondary = pending.present()?;
        Ok(main)
    }
}

/// Main UI frame after secondary recording, optionally carrying completed secondary evidence.
#[must_use = "record or explicitly drop the prepared main frame"]
pub struct WgpuPreparedMainFrame<'frame> {
    prepared: PreparedMainFrame<'frame>,
    secondary: WgpuViewportFrameReport,
}

impl WgpuPreparedMainFrame<'_> {
    /// Context that owns the main frame.
    pub fn context_id(&self) -> ContextId {
        self.prepared.frame.context_id()
    }
    /// Secondary observations captured when the capability was split or completed.
    ///
    /// With an explicit split, later work observations are returned by that work's pending value;
    /// this frozen copy is not updated implicitly.
    pub fn secondary_report(&self) -> &WgpuViewportFrameReport {
        &self.secondary
    }
    /// Abandons main recording and retains the detached secondary observations.
    pub fn into_secondary_report(self) -> WgpuViewportFrameReport {
        self.secondary
    }
}

impl FrameTraceGuard<'_> {
    fn finish_recording(mut self) -> (WgpuViewportFrameReport, Vec<RecordedTarget>) {
        let result = self.control.finish_recording_trace();
        self.active = false;
        result
    }

    /// Ends the trace and returns its normalized, same-scope submission evidence.
    pub(super) fn finish(mut self) -> WgpuViewportFrameReport {
        let report = self.control.finish_frame_trace();
        self.active = false;
        report
    }
}

impl Drop for FrameTraceGuard<'_> {
    fn drop(&mut self) {
        if self.active {
            self.control.abort_frame_trace();
        }
    }
}

impl fmt::Debug for OwningViewportRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OwningViewportRuntime")
            .field("control", &self.control)
            .finish()
    }
}

impl OwningViewportRuntime {
    pub(super) fn begin_frame_trace(
        &self,
        scope: WgpuFrameScope,
    ) -> Result<FrameTraceGuard<'_>, WgpuViewportError> {
        self.control.begin_frame_trace(scope)?;
        Ok(FrameTraceGuard {
            control: self.control.as_ref(),
            active: true,
        })
    }

    pub(crate) fn attach(
        context: &mut Context,
        renderer: WgpuRenderer,
        platform: super::platform_adapter::PlatformOwner,
    ) -> Result<Self, WgpuViewportAttachError> {
        // The Context-owned attachment is the first ownership gate. In particular, a wrapper
        // that was dropped without explicit shutdown leaves this runtime alive for Context
        // teardown, so a replacement must be rejected without inspecting or mutating its
        // renderer argument.
        if let Err(error) = preflight_runtime(context.id()) {
            return Err(WgpuViewportAttachError::new(error, renderer));
        }
        if let Err(error) = renderer.ensure_context_matches(context) {
            let error = match error {
                RendererError::ContextDropped => WgpuViewportError::RendererContextDropped,
                RendererError::ContextMismatch => WgpuViewportError::RendererContextMismatch,
                other => WgpuViewportError::Renderer(other),
            };
            return Err(WgpuViewportAttachError::new(error, renderer));
        }
        if let Err(error) = renderer.ensure_renderer_contract() {
            return Err(WgpuViewportAttachError::new(error.into(), renderer));
        }
        let globals = match renderer_globals(&renderer, platform) {
            Ok(globals) => globals,
            Err(error) => return Err(WgpuViewportAttachError::new(error, renderer)),
        };
        Self::attach_preflighted(context, renderer, Some(globals))
    }

    fn attach_preflighted(
        context: &mut Context,
        renderer: WgpuRenderer,
        globals: Option<GlobalHandles>,
    ) -> Result<Self, WgpuViewportAttachError> {
        if let Err(error) = preflight_callbacks(context) {
            return Err(WgpuViewportAttachError::new(error, renderer));
        }
        if let Err(error) = preflight_runtime(context.id()) {
            return Err(WgpuViewportAttachError::new(error, renderer));
        }

        if let Err(error) = renderer.ensure_renderer_contract() {
            return Err(WgpuViewportAttachError::new(error.into(), renderer));
        }
        let control = Rc::new(RuntimeControl::new(context, renderer, globals));
        let attachment = match context.register_attachment::<WgpuRendererAttachmentMarker>(
            ContextAttachmentRole::Renderer,
            Rc::clone(&control) as Rc<dyn ContextAttachment>,
        ) {
            Ok(attachment) => attachment,
            Err(error) => {
                let renderer = control.recover_renderer();
                return Err(WgpuViewportAttachError::new(error.into(), renderer));
            }
        };
        control.store_attachment(attachment);
        register_runtime(&control);
        claim_callbacks(&control, context);
        control.set_state(RuntimeState::Attached);
        Ok(Self { control })
    }

    #[cfg(test)]
    pub(super) fn attach_for_test(
        context: &mut Context,
        mut renderer: WgpuRenderer,
    ) -> Result<Self, WgpuViewportAttachError> {
        if let Err(error) = preflight_runtime(context.id()) {
            return Err(WgpuViewportAttachError::new(error, renderer));
        }
        if renderer.context_state.is_none() {
            let (flags, _) = match WgpuRenderer::configure_imgui_context(context) {
                Ok(configured) => configured,
                Err(error) => {
                    return Err(WgpuViewportAttachError::new(error.into(), renderer));
                }
            };
            if let Err(error) = renderer.bind_context(context, flags) {
                return Err(WgpuViewportAttachError::new(error.into(), renderer));
            }
            renderer.renderer_consumer = match context.create_synchronous_renderer_consumer() {
                Ok(consumer) => Some(consumer),
                Err(error) => {
                    return Err(WgpuViewportAttachError::new(
                        RendererError::from(error).into(),
                        renderer,
                    ));
                }
            };
        }
        Self::attach_preflighted(context, renderer, None)
    }

    #[cfg(test)]
    pub(crate) fn poll_fault(&self) -> Result<(), WgpuViewportError> {
        self.control.detect_and_take_fault().map_or(Ok(()), Err)
    }

    pub(crate) fn drain_faults(&self) -> Vec<WgpuViewportError> {
        self.control.detect_and_drain_faults()
    }

    pub(crate) fn context_id(&self) -> ContextId {
        self.control.binding().id()
    }

    /// Rejects a frame whose UI belongs to another Context before entering any renderer-owned
    /// state. Route adapters call this before their platform transaction, and `prepare_frame`
    /// repeats it at the owning runtime boundary.
    pub(crate) fn ensure_frame_context(&self, actual: ContextId) -> Result<(), WgpuViewportError> {
        let expected = self.context_id();
        if expected == actual {
            Ok(())
        } else {
            Err(WgpuViewportError::ContextMismatch { expected, actual })
        }
    }

    pub(crate) fn prepare_frame<'frame>(
        &self,
        frame: FrameToken<'frame>,
    ) -> PreparationAttempt<PreparedMainFrame<'frame>> {
        if let Err(error) = self.ensure_frame_context(frame.ui().context_id()) {
            return PreparationAttempt::failed(error);
        }
        let frame = self.control.with_renderer_mut(|renderer| {
            let frame = frame
                .try_render(renderer.renderer_consumer()?)
                .map_err(RendererError::from)?;
            renderer
                .reconcile_frame(frame)
                .map_err(WgpuViewportError::from)
        });
        match frame {
            Ok(frame) => self.prepare_reconciled(frame),
            Err(error) => PreparationAttempt::failed(error),
        }
    }

    fn prepare_reconciled<'frame>(
        &self,
        mut frame: ReconciledFrame<'frame>,
    ) -> PreparationAttempt<PreparedMainFrame<'frame>> {
        let scope = WgpuFrameScope {
            context_id: frame.context_id(),
            epoch: frame.epoch(),
            frame_count: frame.draw_data().frame_count(),
        };
        let faults = self.drain_faults();
        if !faults.is_empty() {
            return PreparationAttempt {
                result: Err(faults),
                report: WgpuViewportFrameReport {
                    scope: Some(scope),
                    observations: Vec::new(),
                },
            };
        }
        let trace = match self.begin_frame_trace(scope) {
            Ok(trace) => trace,
            Err(error) => return PreparationAttempt::failed(error),
        };
        let updated = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            frame.update_platform_windows()
        }));
        let report = trace.finish();
        let mut faults = self.drain_faults();
        if updated.is_err() {
            faults.push(WgpuViewportError::CallbackPanicked {
                callback: "update platform windows",
            });
        }
        let result = if faults.is_empty() {
            Ok(PreparedMainFrame {
                frame,
                runtime: Rc::clone(&self.control.identity),
            })
        } else {
            Err(faults)
        };
        PreparationAttempt { result, report }
    }

    pub(crate) fn record<'frame>(
        &self,
        prepared: WgpuPreparedViewportFrame<'frame>,
    ) -> Result<WgpuRecordedViewportFrame<'frame>, WgpuViewportWorkError> {
        if let Err(error) =
            self.ensure_prepared_runtime(&prepared.prepared.runtime, prepared.context_id())
        {
            return Err(WgpuViewportWorkError::new(prepared.report, vec![error]));
        }
        let scope = WgpuFrameScope {
            context_id: prepared.context_id(),
            epoch: prepared.prepared.frame.epoch(),
            frame_count: prepared.prepared.frame.draw_data().frame_count(),
        };
        let queue = match self.control.with_renderer_mut(|renderer| {
            Ok(renderer
                .gpu
                .backend_data
                .as_ref()
                .ok_or(WgpuViewportError::RendererNotInitialized)?
                .queue
                .clone())
        }) {
            Ok(queue) => queue,
            Err(error) => return Err(WgpuViewportWorkError::new(prepared.report, vec![error])),
        };
        let trace = match self.begin_frame_trace(scope) {
            Ok(trace) => trace,
            Err(error) => return Err(WgpuViewportWorkError::new(prepared.report, vec![error])),
        };
        let mut main = prepared.prepared;
        let recording = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            main.frame.render_platform_windows_default()
        }));
        let (mut report, recorded) = trace.finish_recording();
        let mut faults = self.drain_faults();
        if recording.is_err() {
            faults.push(WgpuViewportError::CallbackPanicked {
                callback: "record platform windows",
            });
        }
        if !faults.is_empty() {
            for target in &recorded {
                report.discard(target.observation);
            }
            return Err(WgpuViewportWorkError::new(report, faults));
        }
        Ok(WgpuRecordedViewportFrame {
            main: WgpuPreparedMainFrame {
                prepared: main,
                secondary: report.clone(),
            },
            secondary: WgpuRecordedViewportWork::new(queue, recorded, report),
        })
    }

    pub(crate) fn finish_recording_attempt<'frame>(
        &self,
        attempt: Option<Result<WgpuRecordedViewportFrame<'frame>, WgpuViewportWorkError>>,
        platform_faults: Vec<WgpuViewportError>,
    ) -> Result<WgpuRecordedViewportFrame<'frame>, WgpuViewportWorkError> {
        let mut extra_faults = self.drain_faults();
        extra_faults.extend(platform_faults);
        match attempt {
            Some(Ok(recorded)) if extra_faults.is_empty() => Ok(recorded),
            Some(Ok(recorded)) => {
                let (_, work) = recorded.split();
                let report = work.discard();
                Err(WgpuViewportWorkError::new(report, extra_faults))
            }
            Some(Err(error)) => {
                let (report, mut faults) = error.into_parts();
                faults.extend(extra_faults);
                Err(WgpuViewportWorkError::new(report, faults))
            }
            None => {
                if extra_faults.is_empty() {
                    extra_faults.push(WgpuViewportError::RuntimeDetached);
                }
                Err(WgpuViewportWorkError::new(
                    WgpuViewportFrameReport::default(),
                    extra_faults,
                ))
            }
        }
    }

    fn ensure_prepared_runtime(
        &self,
        runtime: &Rc<RuntimeIdentity>,
        actual: ContextId,
    ) -> Result<(), WgpuViewportError> {
        if Rc::ptr_eq(&self.control.identity, runtime) {
            Ok(())
        } else {
            Err(WgpuViewportError::PreparedFrameRuntimeMismatch {
                expected: self.context_id(),
                actual,
            })
        }
    }

    pub(crate) fn record_main(
        &self,
        prepared: WgpuPreparedMainFrame<'_>,
        render_pass: &mut wgpu::RenderPass<'_>,
        framebuffer_extent: FramebufferExtent,
    ) -> Result<WgpuViewportObservation, WgpuViewportWorkError> {
        let mut report = prepared.secondary;
        let PreparedMainFrame { frame, runtime } = prepared.prepared;
        let scope = WgpuFrameScope {
            context_id: frame.context_id(),
            epoch: frame.epoch(),
            frame_count: frame.draw_data().frame_count(),
        };
        if let Err(error) = self.ensure_prepared_runtime(&runtime, frame.context_id()) {
            return Err(WgpuViewportWorkError::new(report, vec![error]));
        }
        let viewport = frame.draw_data().owner_viewport();
        if viewport.is_null() {
            return Err(WgpuViewportWorkError::new(
                report,
                vec![WgpuViewportError::InvalidViewport {
                    callback: "record_main",
                }],
            ));
        }
        // SAFETY: the reconciled frame retains the Context and its main viewport allocation.
        let viewport_id = unsafe { (*viewport).ID };
        let target = self
            .control
            .globals()
            .ok_or(WgpuViewportError::RuntimeDetached)
            .and_then(|globals| {
                super::platform_adapter::target_for_viewport(
                    &globals.platform,
                    dear_imgui_rs::Id::from(viewport_id),
                )?
                .ok_or(WgpuViewportError::MainViewportHandleUnavailable)
            });
        let target = match target {
            Ok(target) => target,
            Err(error) => return Err(WgpuViewportWorkError::new(report, vec![error])),
        };
        let mut dependencies = crate::WgpuDrawDependencies::default();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.control.with_renderer_mut(|renderer| {
                renderer
                    .render_reconciled_observed(
                        frame,
                        render_pass,
                        framebuffer_extent,
                        &mut dependencies,
                    )
                    .map_err(Into::into)
            })
        }));
        let result = result.unwrap_or_else(|_| {
            Err(WgpuViewportError::CallbackPanicked {
                callback: "record main viewport",
            })
        });
        match result {
            Ok(recording) => Ok(WgpuViewportObservation::from_recording(target, recording)),
            Err(error) => {
                report.observations.push(WgpuViewportObservation {
                    target,
                    scope,
                    dependencies,
                    stage: crate::WgpuFrameStage::Unrecorded,
                    outcome: crate::WgpuFrameOutcome::Failed {
                        operation: "record main viewport",
                        message: error.to_string(),
                    },
                });
                Err(WgpuViewportWorkError::new(report, vec![error]))
            }
        }
    }

    pub(crate) fn invalidate_device_objects(
        &self,
        context: &mut Context,
    ) -> Result<(), WgpuViewportError> {
        self.control.ensure_context(context)?;
        self.control.with_renderer_mut(|renderer| {
            renderer
                .invalidate_device_objects(context)
                .map_err(Into::into)
        })
    }

    pub(crate) fn set_gamma_mode(&self, mode: GammaMode) -> Result<(), WgpuViewportError> {
        self.control.with_renderer_mut(|renderer| {
            renderer.set_gamma_mode(mode);
            Ok(())
        })
    }

    pub(crate) fn set_viewport_clear_color(
        &self,
        color: wgpu::Color,
    ) -> Result<(), WgpuViewportError> {
        self.control.with_renderer_mut(|renderer| {
            renderer.set_viewport_clear_color(color);
            Ok(())
        })
    }

    pub(crate) fn external_texture_binding(
        &self,
        texture: ExternalTextureId,
    ) -> Result<ExternalTextureBinding, WgpuViewportError> {
        self.control.with_renderer_mut(|renderer| {
            renderer
                .external_texture_binding(texture)
                .map_err(Into::into)
        })
    }

    pub(crate) fn register_external_texture(
        &self,
        view: &wgpu::TextureView,
    ) -> Result<ExternalTextureId, WgpuViewportError> {
        self.control.with_renderer_mut(|renderer| {
            renderer.register_external_texture(view).map_err(Into::into)
        })
    }

    pub(crate) fn update_external_texture(
        &self,
        texture: ExternalTextureId,
        view: &wgpu::TextureView,
    ) -> Result<(), WgpuViewportError> {
        self.control.with_renderer_mut(|renderer| {
            renderer
                .update_external_texture(texture, view)
                .map_err(Into::into)
        })
    }

    pub(crate) fn unregister_external_texture(
        &self,
        texture: ExternalTextureId,
    ) -> Result<(), WgpuViewportError> {
        self.control.with_renderer_mut(|renderer| {
            renderer
                .unregister_external_texture(texture)
                .map_err(Into::into)
        })
    }

    pub(crate) fn shutdown(&mut self, context: &mut Context) -> Result<(), WgpuViewportError> {
        self.control.ensure_context(context)?;
        let pending = self.control.detect_and_take_fault();
        let binding = self.control.binding.clone();
        let result = binding
            .try_with_bound_context(|| {
                self.control
                    .shutdown_once(ShutdownAction::Explicit(context))
            })
            .map_err(Into::into)
            .and_then(|result| result);
        match (pending, result) {
            (Some(fault), Err(shutdown_error)) => {
                self.control.record_fault(shutdown_error);
                Err(fault)
            }
            (Some(fault), Ok(())) => Err(fault),
            (None, result) => result,
        }
    }

    #[cfg(test)]
    pub(super) fn main_frame_for_test<'frame>(
        &self,
        frame: FrameToken<'frame>,
        secondary: WgpuViewportFrameReport,
    ) -> WgpuPreparedMainFrame<'frame> {
        let frame = self
            .control
            .with_renderer_mut(|renderer| {
                let pending = frame
                    .try_render(renderer.renderer_consumer()?)
                    .map_err(RendererError::from)?;
                renderer.reconcile_frame(pending).map_err(Into::into)
            })
            .expect("the test's real WGPU renderer must reconcile its managed frame");
        WgpuPreparedMainFrame {
            prepared: PreparedMainFrame {
                frame,
                runtime: Rc::clone(&self.control.identity),
            },
            secondary,
        }
    }

    #[cfg(test)]
    pub(super) fn renderer_address_for_test(&self) -> *const WgpuRenderer {
        self.control.renderer_address_for_test()
    }

    #[cfg(test)]
    pub(super) fn state_for_test(&self) -> RuntimeState {
        self.control.state()
    }

    #[cfg(test)]
    pub(super) fn transition_log_for_test(&self) -> Vec<&'static str> {
        self.control.transition_log_for_test()
    }

    #[cfg(test)]
    pub(super) fn control_for_test(&self) -> Rc<RuntimeControl> {
        Rc::clone(&self.control)
    }

    #[cfg(test)]
    pub(super) fn ensure_runtime_identity_for_test(
        &self,
        other: &Self,
    ) -> Result<(), WgpuViewportError> {
        self.ensure_prepared_runtime(&other.control.identity, other.context_id())
    }

    #[cfg(test)]
    pub(super) fn panic_next_callback_for_test(&self) {
        self.control.panic_next_callback_for_test();
    }

    #[cfg(test)]
    pub(super) fn fail_next_viewport_cleanup_for_test(&self) {
        self.control.fail_next_viewport_cleanup_for_test();
    }
}

impl Drop for OwningViewportRuntime {
    fn drop(&mut self) {
        self.control.owner_dropped();
    }
}

fn first_error<const N: usize>(
    errors: [Option<WgpuViewportError>; N],
) -> Result<(), WgpuViewportError> {
    errors.into_iter().flatten().next().map_or(Ok(()), Err)
}
