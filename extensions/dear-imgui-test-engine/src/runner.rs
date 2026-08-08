#[cfg(feature = "capture")]
use std::cell::{Cell, RefCell};
use std::convert::Infallible;
use std::error::Error;
#[cfg(feature = "capture")]
use std::ffi::c_void;
use std::fmt;
use std::num::NonZeroU64;
#[cfg(feature = "capture")]
use std::panic::{AssertUnwindSafe, catch_unwind};

use dear_imgui_rs::render::ReconciledFrame;
use dear_imgui_rs::{
    BackendFlags, Context, ContextBindingError, ContextId, FrameLifecycleState, FrameToken, Ui,
};

use crate::results::RunCompletion;
use crate::{
    AttachmentState, FrameDriverError, FrameDriverPhase, MainRenderOutcome, ResultSummary,
    RunFlags, RunMode, RunOutcome, RunReport, RunState, TestEngine, TestEngineError,
    TestFrameDriver, TestGroup,
};
#[cfg(feature = "capture")]
use crate::{
    CaptureProviderError, CaptureRequest, CapturingTestFrameDriver, Rgba8, TestEngineStatus,
};

const DEFAULT_FRAME_BUDGET: NonZeroU64 = NonZeroU64::new(10_000).unwrap();
const DEFAULT_CLEANUP_FRAME_BUDGET: NonZeroU64 = NonZeroU64::new(256).unwrap();

/// Control returned by the application UI callback after one frame has been built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerControl {
    /// Continue the bounded run.
    Continue,
    /// Finish the current frame, then abort and drain the native queue.
    Abort,
}

/// Failure produced by the virtual/headless render path.
#[derive(Debug)]
#[non_exhaustive]
pub enum HeadlessPrepareError {
    /// The Context advertises a managed-texture renderer, which headless mode cannot drive.
    ManagedRenderer,
}

impl fmt::Display for HeadlessPrepareError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManagedRenderer => formatter.write_str(
                "virtual presentation cannot drive a managed-texture renderer; use run_graphical with a renderer-owned synchronous consumer",
            ),
        }
    }
}

impl Error for HeadlessPrepareError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

struct VirtualFrameDriver;

impl TestFrameDriver for VirtualFrameDriver {
    type PreparedFrame<'frame> = ReconciledFrame<'frame>;
    type PrepareError = HeadlessPrepareError;
    type RenderError = Infallible;
    type PresentError = Infallible;

    fn prepare<'frame>(
        &mut self,
        frame: FrameToken<'frame>,
        _frame_index: u64,
    ) -> Result<Self::PreparedFrame<'frame>, Self::PrepareError> {
        if frame
            .ui()
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_TEXTURES)
        {
            return Err(HeadlessPrepareError::ManagedRenderer);
        }
        Ok(frame.render_legacy())
    }

    fn prepared_context_id(frame: &Self::PreparedFrame<'_>) -> ContextId {
        frame.context_id()
    }

    fn render_main(
        &mut self,
        frame: Self::PreparedFrame<'_>,
        _frame_index: u64,
    ) -> Result<MainRenderOutcome, Self::RenderError> {
        drop(frame);
        Ok(MainRenderOutcome::ReadyToPresent)
    }

    fn present(&mut self, _frame_index: u64) -> Result<(), Self::PresentError> {
        Ok(())
    }
}

/// Owns the only mutable access route to the caller's capture-capable driver for one run.
///
/// The native callback may re-enter Rust after `present` and before `post_swap` returns. The
/// driver therefore cannot be both an ordinary `&mut TestFrameDriver` and callback user data.
/// Runtime borrow checking makes every driver access exclusive and rejects callback re-entry.
#[cfg(feature = "capture")]
struct CaptureDriverSlot<'driver, Driver>
where
    Driver: CapturingTestFrameDriver,
{
    driver: RefCell<&'driver mut Driver>,
    current_frame: Cell<u64>,
    failure: RefCell<Option<(u64, CaptureProviderError<Driver::CaptureError>)>>,
}

#[cfg(feature = "capture")]
#[derive(Clone, Copy)]
struct NativeCaptureRequest {
    viewport_id: u32,
    origin: [i32; 2],
    size: [i32; 2],
    pixels: *mut u32,
}

#[cfg(feature = "capture")]
fn dispose_panic_payload_without_unwinding(mut payload: Box<dyn std::any::Any + Send>) {
    const MAX_DROP_ATTEMPTS: usize = 8;

    for _ in 0..MAX_DROP_ATTEMPTS {
        match catch_unwind(AssertUnwindSafe(|| drop(payload))) {
            Ok(()) => return,
            Err(next_payload) => payload = next_payload,
        }
    }

    // An adversarial Drop implementation can keep creating another panicking payload forever.
    // Retaining only the final payload is the bounded fallback that preserves the C ABI boundary
    // without aborting the host process. Ordinary payloads and finite destructor-panic chains are
    // reclaimed by the loop above.
    std::mem::forget(payload);
}

#[cfg(feature = "capture")]
impl<'driver, Driver> CaptureDriverSlot<'driver, Driver>
where
    Driver: CapturingTestFrameDriver,
{
    fn new(driver: &'driver mut Driver) -> Self {
        Self {
            driver: RefCell::new(driver),
            current_frame: Cell::new(1),
            failure: RefCell::new(None),
        }
    }

    fn set_current_frame(&self, frame_index: u64) {
        self.current_frame.set(frame_index);
    }

    fn take_failure(&mut self) -> Option<(u64, CaptureProviderError<Driver::CaptureError>)> {
        self.failure.get_mut().take()
    }

    fn record_failure(&self, failure: CaptureProviderError<Driver::CaptureError>) {
        if let Ok(mut slot) = self.failure.try_borrow_mut()
            && slot.is_none()
        {
            *slot = Some((self.current_frame.get(), failure));
        }
    }

    fn capture(
        &self,
        viewport_id: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        pixels: *mut u32,
    ) -> bool {
        if self
            .failure
            .try_borrow()
            .map_or(true, |failure| failure.is_some())
        {
            return false;
        }
        let Ok(mut driver) = self.driver.try_borrow_mut() else {
            self.record_failure(CaptureProviderError::InvalidRequest {
                detail: "framebuffer capture re-entered while a driver phase was active",
            });
            return false;
        };

        let request = NativeCaptureRequest {
            viewport_id,
            origin: [x, y],
            size: [width, height],
            pixels,
        };
        let result = catch_unwind(AssertUnwindSafe(|| {
            Self::capture_inner(&mut **driver, request)
        }));

        match result {
            Ok(Ok(())) => true,
            Ok(Err(failure)) => {
                self.record_failure(failure);
                false
            }
            Err(payload) => {
                dispose_panic_payload_without_unwinding(payload);
                self.record_failure(CaptureProviderError::Panicked);
                false
            }
        }
    }

    fn capture_inner(
        driver: &mut Driver,
        request: NativeCaptureRequest,
    ) -> Result<(), CaptureProviderError<Driver::CaptureError>> {
        let (Ok(width), Ok(height)) = (
            u32::try_from(request.size[0]),
            u32::try_from(request.size[1]),
        ) else {
            return Err(CaptureProviderError::InvalidRequest {
                detail: "width and height must be positive",
            });
        };
        if width == 0 || height == 0 {
            return Err(CaptureProviderError::InvalidRequest {
                detail: "width and height must be positive",
            });
        }
        let Some(pixel_count) = (width as usize).checked_mul(height as usize) else {
            return Err(CaptureProviderError::InvalidRequest {
                detail: "width multiplied by height overflowed usize",
            });
        };
        if pixel_count > isize::MAX as usize / std::mem::size_of::<Rgba8>() {
            return Err(CaptureProviderError::InvalidRequest {
                detail: "pixel buffer exceeds Rust slice limits",
            });
        }
        if request.pixels.is_null() {
            return Err(CaptureProviderError::InvalidRequest {
                detail: "pixel buffer must not be null",
            });
        }

        let pixels =
            unsafe { std::slice::from_raw_parts_mut(request.pixels.cast::<Rgba8>(), pixel_count) };
        let request =
            CaptureRequest::new(request.viewport_id, request.origin, [width, height], pixels);
        driver
            .capture_framebuffer(request)
            .map_err(CaptureProviderError::Driver)
    }
}

#[cfg(feature = "capture")]
struct CaptureDriverAdapter<'slot, 'driver, Driver>
where
    Driver: CapturingTestFrameDriver,
{
    slot: &'slot CaptureDriverSlot<'driver, Driver>,
}

#[cfg(feature = "capture")]
impl<Driver> TestFrameDriver for CaptureDriverAdapter<'_, '_, Driver>
where
    Driver: CapturingTestFrameDriver,
{
    type PreparedFrame<'frame> = Driver::PreparedFrame<'frame>;
    type PrepareError = Driver::PrepareError;
    type RenderError = Driver::RenderError;
    type PresentError = Driver::PresentError;

    fn prepare<'frame>(
        &mut self,
        frame: FrameToken<'frame>,
        frame_index: u64,
    ) -> Result<Self::PreparedFrame<'frame>, Self::PrepareError> {
        self.slot.driver.borrow_mut().prepare(frame, frame_index)
    }

    fn prepared_context_id(frame: &Self::PreparedFrame<'_>) -> ContextId {
        Driver::prepared_context_id(frame)
    }

    fn render_main(
        &mut self,
        frame: Self::PreparedFrame<'_>,
        frame_index: u64,
    ) -> Result<MainRenderOutcome, Self::RenderError> {
        let outcome = self
            .slot
            .driver
            .borrow_mut()
            .render_main(frame, frame_index)?;
        if outcome == MainRenderOutcome::ReadyToPresent {
            self.slot.set_current_frame(frame_index);
        }
        Ok(outcome)
    }

    fn present(&mut self, frame_index: u64) -> Result<(), Self::PresentError> {
        self.slot.driver.borrow_mut().present(frame_index)
    }
}

#[cfg(feature = "capture")]
unsafe extern "C" fn capture_driver_trampoline<Driver>(
    viewport_id: u32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    pixels: *mut u32,
    user_data: *mut c_void,
) -> bool
where
    Driver: CapturingTestFrameDriver,
{
    let Some(slot) = (unsafe { user_data.cast::<CaptureDriverSlot<'_, Driver>>().as_ref() }) else {
        return false;
    };
    slot.capture(viewport_id, x, y, width, height, pixels)
}

/// Infrastructure failure produced by [`TestRunner`].
///
/// Application UI, frame preparation, main-target rendering, and presentation retain independent
/// error types. Product outcomes such as assertion failures, no matches, timeouts, and explicit
/// aborts are returned as [`RunReport`] instead.
#[derive(Debug)]
#[non_exhaustive]
pub enum RunnerError<
    ApplicationError,
    PrepareError,
    RenderError,
    PresentError,
    CaptureError = Infallible,
> {
    /// A safe Test Engine operation failed.
    TestEngine(TestEngineError),
    /// The supplied Context is not the Context to which the engine is attached.
    ContextMismatch {
        expected: ContextId,
        actual: ContextId,
    },
    /// The attached Context could not be bound for the complete run.
    ContextBinding(ContextBindingError),
    /// The runner was entered while another owner held an open frame.
    FrameAlreadyOpen,
    /// Application UI construction failed.
    ApplicationUi {
        frame: u64,
        source: ApplicationError,
    },
    /// One backend frame phase or native presentation hook failed.
    FrameDriver {
        frame: u64,
        source: FrameDriverError<PrepareError, RenderError, PresentError>,
    },
    /// The run-scoped framebuffer provider rejected or could not validate a capture request.
    Capture {
        frame: u64,
        /// Presentation phase that surfaced the native capture failure, when one exists.
        phase: Option<FrameDriverPhase>,
        source: CaptureError,
    },
    /// Stopping after another infrastructure failure also failed.
    Teardown {
        primary: Box<Self>,
        source: TestEngineError,
    },
    /// Native terminal counters contradicted the terminal state contract.
    InvalidTerminalSummary {
        summary: ResultSummary,
        detail: &'static str,
    },
    /// The native queue did not settle within the cleanup budget.
    CleanupDidNotSettle {
        requested: RunOutcome,
        cleanup_frames: u64,
    },
}

/// Infrastructure failure produced by [`TestRunner::run_headless`].
///
/// Headless runs use the built-in virtual renderer and an infallible virtual presentation step,
/// so callers only need to name their application UI error.
pub type HeadlessRunnerError<ApplicationError> =
    RunnerError<ApplicationError, HeadlessPrepareError, Infallible, Infallible>;

/// Infrastructure failure produced by [`TestRunner::run_graphical_with_capture`].
#[cfg(feature = "capture")]
pub type CapturingRunnerError<
    ApplicationError,
    PrepareError,
    RenderError,
    PresentError,
    CaptureError,
> = RunnerError<
    ApplicationError,
    PrepareError,
    RenderError,
    PresentError,
    CaptureProviderError<CaptureError>,
>;

/// Result produced by a capturing graphical run for a concrete frame driver.
#[cfg(feature = "capture")]
pub type CapturingDriverRunResult<ApplicationError, Driver> = Result<
    RunReport,
    CapturingRunnerError<
        ApplicationError,
        <Driver as TestFrameDriver>::PrepareError,
        <Driver as TestFrameDriver>::RenderError,
        <Driver as TestFrameDriver>::PresentError,
        <Driver as CapturingTestFrameDriver>::CaptureError,
    >,
>;

impl<ApplicationError, PrepareError, RenderError, PresentError, CaptureError> From<TestEngineError>
    for RunnerError<ApplicationError, PrepareError, RenderError, PresentError, CaptureError>
{
    fn from(source: TestEngineError) -> Self {
        Self::TestEngine(source)
    }
}

impl<ApplicationError, PrepareError, RenderError, PresentError, CaptureError> fmt::Display
    for RunnerError<ApplicationError, PrepareError, RenderError, PresentError, CaptureError>
where
    ApplicationError: fmt::Display,
    PrepareError: fmt::Display,
    RenderError: fmt::Display,
    PresentError: fmt::Display,
    CaptureError: fmt::Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TestEngine(source) => {
                write!(formatter, "Test Engine infrastructure failed: {source}")
            }
            Self::ContextMismatch { expected, actual } => write!(
                formatter,
                "runner received Context {actual:?}, but the Test Engine is attached to {expected:?}"
            ),
            Self::ContextBinding(source) => {
                write!(
                    formatter,
                    "runner could not bind the attached Context: {source}"
                )
            }
            Self::FrameAlreadyOpen => {
                formatter.write_str("runner cannot start while a Dear ImGui frame is open")
            }
            Self::ApplicationUi { frame, source } => {
                write!(
                    formatter,
                    "application UI failed on frame {frame}: {source}"
                )
            }
            Self::FrameDriver { frame, source } => {
                write!(formatter, "frame driver failed on frame {frame}: {source}")
            }
            Self::Capture {
                frame,
                phase: Some(phase),
                source,
            } => write!(
                formatter,
                "framebuffer capture failed during {phase:?} on frame {frame}: {source}"
            ),
            Self::Capture {
                frame,
                phase: None,
                source,
            } => {
                write!(
                    formatter,
                    "framebuffer capture failed on frame {frame}: {source}"
                )
            }
            Self::Teardown { primary, source } => write!(
                formatter,
                "{primary}; stopping the Test Engine after that failure also failed: {source}"
            ),
            Self::InvalidTerminalSummary { summary, detail } => write!(
                formatter,
                "native terminal summary {summary:?} is invalid: {detail}"
            ),
            Self::CleanupDidNotSettle {
                requested,
                cleanup_frames,
            } => write!(
                formatter,
                "{requested:?} cleanup did not settle after {cleanup_frames} frame(s)"
            ),
        }
    }
}

impl<ApplicationError, PrepareError, RenderError, PresentError, CaptureError> Error
    for RunnerError<ApplicationError, PrepareError, RenderError, PresentError, CaptureError>
where
    ApplicationError: Error + 'static,
    PrepareError: Error + 'static,
    RenderError: Error + 'static,
    PresentError: Error + 'static,
    CaptureError: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::TestEngine(source) => Some(source),
            Self::ContextBinding(source) => Some(source),
            Self::ApplicationUi { source, .. } => Some(source),
            Self::FrameDriver { source, .. } => Some(source),
            Self::Capture { source, .. } => Some(source),
            Self::Teardown { primary, .. } => Some(primary),
            _ => None,
        }
    }
}

impl<ApplicationError, PrepareError, RenderError, PresentError, CaptureError>
    RunnerError<ApplicationError, PrepareError, RenderError, PresentError, CaptureError>
{
    /// Returns the presentation phase that surfaced a framebuffer capture failure.
    #[must_use]
    pub const fn capture_phase(&self) -> Option<FrameDriverPhase> {
        match self {
            Self::Capture { phase, .. } => *phase,
            _ => None,
        }
    }

    /// Returns a secondary cleanup failure when the primary failure is preserved by `Teardown`.
    #[must_use]
    pub const fn teardown_error(&self) -> Option<&TestEngineError> {
        match self {
            Self::Teardown { source, .. } => Some(source),
            _ => None,
        }
    }
}

struct FramePump<'run, ApplicationUi, Driver> {
    context: &'run mut Context,
    mode: RunMode,
    application_ui: &'run mut ApplicationUi,
    driver: &'run mut Driver,
}

/// One-shot bounded Test Engine pump.
///
/// The runner owns application UI, preparation, main-target rendering, pre-swap, presentation, and
/// post-swap ordering. A single [`TestFrameDriver`] owns all backend phases without
/// shared-mutability workarounds.
pub struct TestRunner<'engine> {
    engine: &'engine mut TestEngine,
    group: TestGroup,
    filter: Option<String>,
    run_flags: RunFlags,
    frame_budget: NonZeroU64,
    cleanup_frame_budget: NonZeroU64,
}

impl<'engine> TestRunner<'engine> {
    /// Creates a runner for one attached engine using conservative default budgets.
    pub fn new(engine: &'engine mut TestEngine) -> Self {
        Self {
            engine,
            group: TestGroup::Tests,
            filter: None,
            run_flags: RunFlags::NONE,
            frame_budget: DEFAULT_FRAME_BUDGET,
            cleanup_frame_budget: DEFAULT_CLEANUP_FRAME_BUDGET,
        }
    }

    /// Selects the native test group.
    #[must_use]
    pub fn group(mut self, group: TestGroup) -> Self {
        self.group = group;
        self
    }

    /// Restricts the queued tests using the native filter syntax.
    #[must_use]
    pub fn filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = Some(filter.into());
        self
    }

    /// Selects the native queue flags.
    #[must_use]
    pub fn run_flags(mut self, run_flags: RunFlags) -> Self {
        self.run_flags = run_flags;
        self
    }

    /// Sets the number of primary frames available to the run.
    #[must_use]
    pub fn frame_budget(mut self, frame_budget: NonZeroU64) -> Self {
        self.frame_budget = frame_budget;
        self
    }

    /// Sets the number of additional frames available to drain an abort or timeout.
    #[must_use]
    pub fn cleanup_frame_budget(mut self, cleanup_frame_budget: NonZeroU64) -> Self {
        self.cleanup_frame_budget = cleanup_frame_budget;
        self
    }

    /// Runs with one external-surface frame driver.
    ///
    /// [`RunMode::Graphical`] reports the selected integration path. It is not independent evidence
    /// that an operating-system swapchain displayed pixels; authoritative runtime gates must obtain
    /// that evidence from a trusted backend.
    pub fn run_graphical<ApplicationError, ApplicationUi, Driver>(
        self,
        context: &mut Context,
        application_ui: ApplicationUi,
        driver: &mut Driver,
    ) -> Result<
        RunReport,
        RunnerError<
            ApplicationError,
            Driver::PrepareError,
            Driver::RenderError,
            Driver::PresentError,
        >,
    >
    where
        ApplicationUi: FnMut(&Ui, u64) -> Result<RunnerControl, ApplicationError>,
        Driver: TestFrameDriver,
    {
        self.run_impl(context, RunMode::Graphical, application_ui, driver)
    }

    /// Runs with an external-surface driver that owns framebuffer readback capability.
    ///
    /// The provider is installed for the complete native queue lifetime and is removed before this
    /// method releases its mutable borrow of `driver`. Driver errors and panics are contained at the
    /// callback boundary and returned as [`RunnerError::Capture`].
    #[cfg(feature = "capture")]
    pub fn run_graphical_with_capture<ApplicationError, ApplicationUi, Driver>(
        self,
        context: &mut Context,
        application_ui: ApplicationUi,
        driver: &mut Driver,
    ) -> CapturingDriverRunResult<ApplicationError, Driver>
    where
        ApplicationUi: FnMut(&Ui, u64) -> Result<RunnerControl, ApplicationError>,
        Driver: CapturingTestFrameDriver,
    {
        let mut slot = Box::new(CaptureDriverSlot::new(driver));
        let user_data = (&mut *slot as *mut CaptureDriverSlot<'_, Driver>).cast();
        let mut provider = self
            .engine
            .install_capture_provider(Some(capture_driver_trampoline::<Driver>), user_data)?;
        let result = {
            let mut adapter = CaptureDriverAdapter { slot: &slot };
            self.run_impl(
                context,
                RunMode::GraphicalWithCapture,
                application_ui,
                &mut adapter,
            )
        };
        let result = match (result, slot.take_failure()) {
            (Ok(_), Some((frame, source))) => Err(RunnerError::Capture {
                frame,
                phase: None,
                source,
            }),
            (Err(error), Some((frame, source))) => {
                Err(Self::replace_capture_failure_marker(error, frame, source))
            }
            (result, None) => result,
        };
        match (result, provider.clear()) {
            (Ok(report), Ok(())) => Ok(report),
            (Err(error), Ok(())) => Err(error),
            (Ok(_), Err(source)) => Err(RunnerError::TestEngine(source)),
            (Err(primary), Err(source)) => Err(RunnerError::Teardown {
                primary: Box::new(primary),
                source,
            }),
        }
    }

    /// Runs through an explicit virtual/no-surface presentation path.
    ///
    /// Virtual mode consumes each frame through the explicit legacy renderer path. Contexts that
    /// advertise a managed-texture renderer are rejected because headless mode does not own a
    /// renderer consumer or a backend capable of reconciling texture requests.
    pub fn run_headless<ApplicationError, ApplicationUi>(
        self,
        context: &mut Context,
        application_ui: ApplicationUi,
    ) -> Result<RunReport, HeadlessRunnerError<ApplicationError>>
    where
        ApplicationUi: FnMut(&Ui, u64) -> Result<RunnerControl, ApplicationError>,
    {
        self.run_impl(
            context,
            RunMode::Headless,
            application_ui,
            &mut VirtualFrameDriver,
        )
    }

    fn run_impl<ApplicationError, ApplicationUi, Driver, CaptureError>(
        mut self,
        context: &mut Context,
        mode: RunMode,
        mut application_ui: ApplicationUi,
        driver: &mut Driver,
    ) -> Result<
        RunReport,
        RunnerError<
            ApplicationError,
            Driver::PrepareError,
            Driver::RenderError,
            Driver::PresentError,
            CaptureError,
        >,
    >
    where
        ApplicationUi: FnMut(&Ui, u64) -> Result<RunnerControl, ApplicationError>,
        Driver: TestFrameDriver,
    {
        let actual = context.id();
        let expected = self.engine.attached_context_id().ok_or_else(|| {
            RunnerError::TestEngine(TestEngineError::invalid_state(
                "TestRunner::run",
                self.engine.attachment_state(),
                self.engine.run_state(),
                "runner requires a live Test Engine attachment",
            ))
        })?;
        if actual != expected {
            return Err(RunnerError::ContextMismatch { expected, actual });
        }

        let binding = context.binding();
        match binding
            .try_with_bound_context(|| self.run_bound(context, mode, &mut application_ui, driver))
        {
            Ok(Ok(report)) => Ok(report),
            Ok(Err(error)) if matches!(error, RunnerError::FrameAlreadyOpen) => Err(error),
            Ok(Err(error)) => match self.stop_after_failure() {
                Ok(()) => Err(error),
                Err(source) => Err(RunnerError::Teardown {
                    primary: Box::new(error),
                    source,
                }),
            },
            Err(source) => Err(RunnerError::ContextBinding(source)),
        }
    }

    fn run_bound<ApplicationError, ApplicationUi, Driver, CaptureError>(
        &mut self,
        context: &mut Context,
        mode: RunMode,
        application_ui: &mut ApplicationUi,
        driver: &mut Driver,
    ) -> Result<
        RunReport,
        RunnerError<
            ApplicationError,
            Driver::PrepareError,
            Driver::RenderError,
            Driver::PresentError,
            CaptureError,
        >,
    >
    where
        ApplicationUi: FnMut(&Ui, u64) -> Result<RunnerControl, ApplicationError>,
        Driver: TestFrameDriver,
    {
        self.require_closed_frame(context)?;
        self.engine
            .queue_tests(self.group, self.filter.as_deref(), self.run_flags)?;

        if let Some(completion) = self.take_terminal_run()? {
            return self.natural_report(completion, 0, 0, mode);
        }

        let mut pump = FramePump {
            context,
            mode,
            application_ui,
            driver,
        };
        let mut frames = 0;
        for frame_index in 1..=self.frame_budget.get() {
            let control = self.pump_frame(frame_index, &mut pump)?;
            frames = frame_index;

            if let Some(completion) = self.take_terminal_run()? {
                return self.natural_report(completion, frames, 0, pump.mode);
            }

            if control == RunnerControl::Abort {
                return self.drain_requested(RunOutcome::Aborted, frames, &mut pump);
            }
        }

        self.drain_requested(RunOutcome::TimedOut, frames, &mut pump)
    }

    fn pump_frame<ApplicationError, ApplicationUi, Driver, CaptureError>(
        &mut self,
        frame_index: u64,
        pump: &mut FramePump<'_, ApplicationUi, Driver>,
    ) -> Result<
        RunnerControl,
        RunnerError<
            ApplicationError,
            Driver::PrepareError,
            Driver::RenderError,
            Driver::PresentError,
            CaptureError,
        >,
    >
    where
        ApplicationUi: FnMut(&Ui, u64) -> Result<RunnerControl, ApplicationError>,
        Driver: TestFrameDriver,
    {
        self.require_closed_frame(pump.context)?;
        let frame = pump.context.begin_frame();
        let control = (pump.application_ui)(frame.ui(), frame_index).map_err(|source| {
            RunnerError::ApplicationUi {
                frame: frame_index,
                source,
            }
        })?;
        self.engine.show_windows(frame.ui(), None)?;
        self.engine
            .drive_frame(frame, frame_index, pump.driver)
            .map_err(|source| RunnerError::FrameDriver {
                frame: frame_index,
                source,
            })?;
        Ok(control)
    }

    fn drain_requested<ApplicationError, ApplicationUi, Driver, CaptureError>(
        &mut self,
        requested: RunOutcome,
        frames: u64,
        pump: &mut FramePump<'_, ApplicationUi, Driver>,
    ) -> Result<
        RunReport,
        RunnerError<
            ApplicationError,
            Driver::PrepareError,
            Driver::RenderError,
            Driver::PresentError,
            CaptureError,
        >,
    >
    where
        ApplicationUi: FnMut(&Ui, u64) -> Result<RunnerControl, ApplicationError>,
        Driver: TestFrameDriver,
    {
        debug_assert!(matches!(
            requested,
            RunOutcome::TimedOut | RunOutcome::Aborted
        ));
        let mut cleanup_frames = 0;

        for _ in 0..self.cleanup_frame_budget.get() {
            if let Some(completion) = self.take_terminal_run()? {
                return self.requested_report(
                    requested,
                    completion,
                    frames,
                    cleanup_frames,
                    pump.mode,
                );
            }

            if self.engine.try_abort_engine()?
                && let Some(completion) = self.take_terminal_run()?
            {
                return self.requested_report(
                    requested,
                    completion,
                    frames,
                    cleanup_frames,
                    pump.mode,
                );
            }

            let frame_index = frames + cleanup_frames + 1;
            self.pump_frame(frame_index, pump)?;
            cleanup_frames += 1;

            if let Some(completion) = self.take_terminal_run()? {
                return self.requested_report(
                    requested,
                    completion,
                    frames,
                    cleanup_frames,
                    pump.mode,
                );
            }
        }

        Err(RunnerError::CleanupDidNotSettle {
            requested,
            cleanup_frames,
        })
    }

    fn requested_report<ApplicationError, PrepareError, RenderError, PresentError, CaptureError>(
        &self,
        outcome: RunOutcome,
        completion: RunCompletion,
        frames: u64,
        cleanup_frames: u64,
        mode: RunMode,
    ) -> Result<
        RunReport,
        RunnerError<ApplicationError, PrepareError, RenderError, PresentError, CaptureError>,
    > {
        self.validate_terminal_summary(completion.summary)?;
        Ok(RunReport::new(
            completion,
            outcome,
            frames + cleanup_frames,
            cleanup_frames,
            mode,
        ))
    }

    fn require_closed_frame<
        ApplicationError,
        PrepareError,
        RenderError,
        PresentError,
        CaptureError,
    >(
        &self,
        context: &Context,
    ) -> Result<
        (),
        RunnerError<ApplicationError, PrepareError, RenderError, PresentError, CaptureError>,
    > {
        match context.frame_lifecycle_state() {
            FrameLifecycleState::Idle | FrameLifecycleState::Rendered => Ok(()),
            FrameLifecycleState::InFrame => Err(RunnerError::FrameAlreadyOpen),
        }
    }

    fn take_terminal_run<
        ApplicationError,
        PrepareError,
        RenderError,
        PresentError,
        CaptureError,
    >(
        &mut self,
    ) -> Result<
        Option<RunCompletion>,
        RunnerError<ApplicationError, PrepareError, RenderError, PresentError, CaptureError>,
    > {
        if self.engine.run_state() != RunState::Terminal {
            return Ok(None);
        }
        self.engine.take_terminal_run().map_err(Into::into)
    }

    fn natural_report<ApplicationError, PrepareError, RenderError, PresentError, CaptureError>(
        &self,
        completion: RunCompletion,
        frames: u64,
        cleanup_frames: u64,
        mode: RunMode,
    ) -> Result<
        RunReport,
        RunnerError<ApplicationError, PrepareError, RenderError, PresentError, CaptureError>,
    > {
        let summary = completion.summary;
        self.validate_terminal_summary(summary)?;
        let outcome = completion.natural_outcome();
        Ok(RunReport::new(
            completion,
            outcome,
            frames,
            cleanup_frames,
            mode,
        ))
    }

    fn validate_terminal_summary<
        ApplicationError,
        PrepareError,
        RenderError,
        PresentError,
        CaptureError,
    >(
        &self,
        summary: ResultSummary,
    ) -> Result<
        (),
        RunnerError<ApplicationError, PrepareError, RenderError, PresentError, CaptureError>,
    > {
        if summary.count_in_queue != 0 {
            return Err(RunnerError::InvalidTerminalSummary {
                summary,
                detail: "terminal state retained queued tests",
            });
        }
        if summary.count_success > summary.count_tested {
            return Err(RunnerError::InvalidTerminalSummary {
                summary,
                detail: "successful test count exceeded executed test count",
            });
        }
        Ok(())
    }

    fn stop_after_failure(&mut self) -> Result<(), TestEngineError> {
        if self.engine.attachment_state() == AttachmentState::Attached
            && self.engine.run_state() != RunState::Inactive
        {
            self.engine.stop()?;
        }
        Ok(())
    }

    #[cfg(feature = "capture")]
    fn replace_capture_failure_marker<
        ApplicationError,
        PrepareError,
        RenderError,
        PresentError,
        CaptureError,
    >(
        error: RunnerError<
            ApplicationError,
            PrepareError,
            RenderError,
            PresentError,
            CaptureProviderError<CaptureError>,
        >,
        frame: u64,
        source: CaptureProviderError<CaptureError>,
    ) -> RunnerError<
        ApplicationError,
        PrepareError,
        RenderError,
        PresentError,
        CaptureProviderError<CaptureError>,
    > {
        match error {
            RunnerError::FrameDriver {
                source: frame_error,
                ..
            } if Self::frame_driver_is_capture_failure(&frame_error) => RunnerError::Capture {
                frame,
                phase: frame_error.phase(),
                source,
            },
            RunnerError::TestEngine(error) if Self::test_engine_is_capture_failure(&error) => {
                RunnerError::Capture {
                    frame,
                    phase: None,
                    source,
                }
            }
            RunnerError::Teardown {
                primary,
                source: teardown,
            } => {
                let primary = Self::replace_capture_failure_marker(*primary, frame, source);
                RunnerError::Teardown {
                    primary: Box::new(primary),
                    source: teardown,
                }
            }
            error => error,
        }
    }

    #[cfg(feature = "capture")]
    fn frame_driver_is_capture_failure<PrepareError, RenderError, PresentError>(
        error: &FrameDriverError<PrepareError, RenderError, PresentError>,
    ) -> bool {
        matches!(
            error,
            FrameDriverError::PostSwap { source, .. }
                if Self::test_engine_is_capture_failure(source)
        )
    }

    #[cfg(feature = "capture")]
    fn test_engine_is_capture_failure(error: &TestEngineError) -> bool {
        matches!(
            error,
            TestEngineError::Ffi {
                status: TestEngineStatus::CaptureFailed,
                ..
            }
        )
    }
}
