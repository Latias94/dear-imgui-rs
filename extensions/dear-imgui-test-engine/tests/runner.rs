use std::cell::{Cell, RefCell};
use std::convert::Infallible;
#[cfg(feature = "capture")]
use std::ffi::CStr;
use std::ffi::c_void;
use std::io;
use std::num::NonZeroU64;
#[cfg(feature = "capture")]
use std::panic::panic_any;
use std::rc::Rc;
#[cfg(feature = "capture")]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};

use dear_imgui_rs::{
    BackendFlags, Context, FrameLifecycleState,
    render::{ReconciledFrame, RenderedFrame, RendererConsumerError},
};
#[cfg(feature = "capture")]
use dear_imgui_test_engine::{
    CaptureFlags, CaptureOutput, CaptureProviderError, CaptureRequest, CapturingTestFrameDriver,
    Rgba8, RunFlags, TestGroup,
};
use dear_imgui_test_engine::{
    FrameDriverError, HeadlessRenderError, RunMode, RunOutcome, RunState, RunTestStatus,
    RunnerControl, RunnerError, ScriptCount, TestEngine, TestEngineError, TestEngineStatus,
    TestFrameDriver, TestRunner, raw,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

fn test_lock() -> MutexGuard<'static, ()> {
    TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn context() -> Context {
    let mut context = Context::create();
    assert!(context.font_atlas().build());
    context.io_mut().set_display_size([128.0, 128.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context
}

fn attached_engine(context: &mut Context) -> TestEngine {
    let mut engine = TestEngine::create().expect("engine");
    engine.start(context).expect("start");
    engine
}

fn nonzero(value: u64) -> NonZeroU64 {
    NonZeroU64::new(value).expect("test budget must be non-zero")
}

fn no_error(_: &dear_imgui_rs::Ui, _: u64) -> Result<RunnerControl, Infallible> {
    Ok(RunnerControl::Continue)
}

unsafe extern "C" fn record_presentation_trace(
    event: raw::ImGuiTestEnginePresentationEvent,
    user_data: *mut c_void,
) {
    let Some(events) = (unsafe {
        user_data
            .cast::<RefCell<Vec<(u64, &'static str)>>>()
            .as_ref()
    }) else {
        return;
    };
    let phase = match event {
        raw::ImGuiTestEnginePresentationEvent_PreSwap => "pre-swap",
        raw::ImGuiTestEnginePresentationEvent_PostSwap => "post-swap",
        _ => return,
    };
    if let Ok(mut events) = events.try_borrow_mut() {
        events.push((0, phase));
    }
}

unsafe extern "C" fn inject_failure_after_completed_post_swap(
    event: raw::ImGuiTestEnginePresentationEvent,
    _user_data: *mut c_void,
) {
    if event == raw::ImGuiTestEnginePresentationEvent_PostSwap {
        let _ = unsafe {
            raw::imgui_test_engine_test_set_exception_injection(
                raw::ImGuiTestEngineExceptionPoint_UpstreamCall,
            )
        };
    }
}

#[cfg(feature = "capture")]
unsafe extern "C" fn arm_interactive_capture_after_post_swap(
    event: raw::ImGuiTestEnginePresentationEvent,
    user_data: *mut c_void,
) {
    if event == raw::ImGuiTestEnginePresentationEvent_PostSwap {
        let _ = unsafe {
            raw::imgui_test_engine_test_set_interactive_capture_state(user_data.cast(), true, true)
        };
    }
}

fn set_presentation_trace(
    engine: &TestEngine,
    events: Option<&Rc<RefCell<Vec<(u64, &'static str)>>>>,
) {
    let (callback, user_data) = match events {
        Some(events) => (
            Some(record_presentation_trace as unsafe extern "C" fn(_, _)),
            Rc::as_ptr(events).cast_mut().cast(),
        ),
        None => (None, std::ptr::null_mut()),
    };
    assert_eq!(
        unsafe {
            raw::imgui_test_engine_test_set_presentation_trace(engine.as_raw(), callback, user_data)
        },
        raw::ImGuiTestEngineStatus_Success
    );
}

#[cfg(feature = "capture")]
fn assert_capture_provider_cleared(engine: &TestEngine) {
    let mut installed = true;
    assert_eq!(
        unsafe { raw::imgui_test_engine_has_capture_provider(engine.as_raw(), &mut installed) },
        raw::ImGuiTestEngineStatus_Success
    );
    assert!(!installed, "run-scoped capture provider was not cleared");
}

#[cfg(feature = "capture")]
fn capture_state(engine: &TestEngine) -> raw::ImGuiTestEngineCaptureState_c {
    let mut state = raw::ImGuiTestEngineCaptureState_c::default();
    assert_eq!(
        unsafe { raw::imgui_test_engine_test_get_capture_state(engine.as_raw(), &mut state) },
        raw::ImGuiTestEngineStatus_Success
    );
    state
}

#[cfg(feature = "capture")]
fn assert_capture_state_clear(engine: &TestEngine) {
    let state = capture_state(engine);
    assert!(!state.ProviderInstalled, "capture state: {state:?}");
    assert!(!state.PresentationPending, "capture state: {state:?}");
    assert!(!state.CaptureAbortRequested, "capture state: {state:?}");
    assert!(!state.CaptureWaitPending, "capture state: {state:?}");
    assert!(!state.ContextCapturing, "capture state: {state:?}");
    assert!(!state.ToolCapturing, "capture state: {state:?}");
    assert!(!state.ToolPicking, "capture state: {state:?}");
    assert!(!state.IoCapturing, "capture state: {state:?}");
    assert!(!state.EngineAbort, "capture state: {state:?}");
    assert!(!state.CaptureRollbackValid, "capture state: {state:?}");
    assert!(!state.HiddenWindowBackupValid, "capture state: {state:?}");
    assert!(!state.ScreenshotConfigActive, "capture state: {state:?}");
    assert!(!state.WindowMoveConfigActive, "capture state: {state:?}");
    assert!(!state.VideoConfigActive, "capture state: {state:?}");
}

#[cfg(feature = "capture")]
fn capture_ui(ui: &dear_imgui_rs::Ui, _: u64) -> Result<RunnerControl, Infallible> {
    ui.window("Capture Target")
        .size([96.0, 72.0], dear_imgui_rs::Condition::Always)
        .build(|| ui.text("capture provider contract"));
    ui.window("Foreign Window")
        .build(|| ui.text("capture cancellation must restore visibility"));
    Ok(RunnerControl::Continue)
}

#[cfg(feature = "capture")]
fn hidden_frames_for_render_only(name: &CStr) -> i8 {
    let window = unsafe { dear_imgui_rs::sys::igFindWindowByName(name.as_ptr()) };
    assert!(!window.is_null(), "expected window was not submitted");
    unsafe { (*window).HiddenFramesForRenderOnly }
}

#[cfg(feature = "capture")]
fn register_capture_script(engine: &mut TestEngine, name: &str) {
    engine
        .add_script_test("runner_capture", name, |script| {
            script.yield_frames(ScriptCount::new(2)?)?;
            script.capture_screenshot_window("//Capture Target", CaptureFlags::NONE)?;
            script.yield_frames(ScriptCount::new(2)?)
        })
        .expect("capture script");
    engine
        .set_capture_output(CaptureOutput::Discard)
        .expect("discard capture output");
}

#[cfg(feature = "capture")]
fn register_immediate_capture_script(engine: &mut TestEngine, name: &str) {
    engine
        .add_script_test("runner_capture", name, |script| {
            script.capture_screenshot_window("//Capture Target", CaptureFlags::NONE)
        })
        .expect("immediate capture script");
    engine
        .set_capture_output(CaptureOutput::Discard)
        .expect("discard capture output");
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DriverFault {
    None,
    Render,
    PreSwap,
    Present,
    PostSwap,
}

struct RecordingDriver {
    events: Rc<RefCell<Vec<(u64, &'static str)>>>,
    fault: DriverFault,
    fault_armed: bool,
}

impl RecordingDriver {
    fn new(events: Rc<RefCell<Vec<(u64, &'static str)>>>, fault: DriverFault) -> Self {
        Self {
            events,
            fault,
            fault_armed: false,
        }
    }

    fn arm_upstream_failure(&mut self) {
        assert!(!self.fault_armed, "failure may be injected only once");
        assert_eq!(
            unsafe {
                raw::imgui_test_engine_test_set_exception_injection(
                    raw::ImGuiTestEngineExceptionPoint_UpstreamCall,
                )
            },
            raw::ImGuiTestEngineStatus_Success
        );
        self.fault_armed = true;
    }
}

impl TestFrameDriver for RecordingDriver {
    type RenderError = io::Error;
    type PresentError = io::Error;

    fn render<'frame>(
        &mut self,
        mut frame: RenderedFrame<'frame>,
        frame_index: u64,
    ) -> Result<ReconciledFrame<'frame>, Self::RenderError> {
        self.events.borrow_mut().push((frame_index, "render"));
        if self.fault == DriverFault::Render {
            return Err(io::Error::other("injected render failure"));
        }
        frame
            .reconcile_texture_feedback([])
            .expect("empty feedback");
        if self.fault == DriverFault::PreSwap {
            self.arm_upstream_failure();
        }
        frame.into_reconciled().map_err(io::Error::other)
    }

    fn present(&mut self, frame_index: u64) -> Result<(), Self::PresentError> {
        self.events.borrow_mut().push((frame_index, "present"));
        match self.fault {
            DriverFault::Present => Err(io::Error::other("injected present failure")),
            DriverFault::PostSwap => {
                self.arm_upstream_failure();
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

#[cfg(feature = "capture")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CaptureFault {
    None,
    Error,
    Panic,
    PanicWithPanickingPayload,
    ErrorAndStopFailure,
}

#[cfg(feature = "capture")]
struct PanickingPayload;

#[cfg(feature = "capture")]
static PANICKING_PAYLOAD_DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

#[cfg(feature = "capture")]
impl Drop for PanickingPayload {
    fn drop(&mut self) {
        PANICKING_PAYLOAD_DROP_COUNT.fetch_add(1, Ordering::AcqRel);
        panic!("panic payload drop");
    }
}

#[cfg(feature = "capture")]
struct CapturingDriver {
    captures: Rc<RefCell<Vec<(u32, [i32; 2], [u32; 2], usize)>>>,
    last_presented: Rc<Cell<u64>>,
    frame_fault: DriverFault,
    fault: CaptureFault,
    fault_armed: bool,
}

#[cfg(feature = "capture")]
impl CapturingDriver {
    fn arm_upstream_failure(&mut self) {
        assert!(!self.fault_armed, "failure may be injected only once");
        assert_eq!(
            unsafe {
                raw::imgui_test_engine_test_set_exception_injection(
                    raw::ImGuiTestEngineExceptionPoint_UpstreamCall,
                )
            },
            raw::ImGuiTestEngineStatus_Success
        );
        self.fault_armed = true;
    }
}

#[cfg(feature = "capture")]
impl TestFrameDriver for CapturingDriver {
    type RenderError = io::Error;
    type PresentError = io::Error;

    fn render<'frame>(
        &mut self,
        mut frame: RenderedFrame<'frame>,
        _frame_index: u64,
    ) -> Result<ReconciledFrame<'frame>, Self::RenderError> {
        if self.frame_fault == DriverFault::Render {
            return Err(io::Error::other("capture-run render failure"));
        }
        frame
            .reconcile_texture_feedback([])
            .expect("empty capture-driver feedback");
        if self.frame_fault == DriverFault::PreSwap {
            self.arm_upstream_failure();
        }
        frame.into_reconciled().map_err(io::Error::other)
    }

    fn present(&mut self, frame_index: u64) -> Result<(), Self::PresentError> {
        self.last_presented.set(frame_index);
        match self.frame_fault {
            DriverFault::Present => Err(io::Error::other("capture-run present failure")),
            DriverFault::PostSwap => {
                self.arm_upstream_failure();
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

#[cfg(feature = "capture")]
impl CapturingTestFrameDriver for CapturingDriver {
    type CaptureError = io::Error;

    fn capture_framebuffer(
        &mut self,
        mut request: CaptureRequest<'_>,
    ) -> Result<(), Self::CaptureError> {
        match self.fault {
            CaptureFault::Error => return Err(io::Error::other("capture rejected")),
            CaptureFault::Panic => panic!("capture panic"),
            CaptureFault::PanicWithPanickingPayload => panic_any(PanickingPayload),
            CaptureFault::ErrorAndStopFailure => {
                assert_eq!(
                    unsafe {
                        raw::imgui_test_engine_test_set_exception_injection(
                            raw::ImGuiTestEngineExceptionPoint_UpstreamCall,
                        )
                    },
                    raw::ImGuiTestEngineStatus_Success
                );
                return Err(io::Error::other("capture and teardown rejected"));
            }
            CaptureFault::None => {}
        }
        let viewport_id = request.viewport_id();
        let origin = request.origin();
        let size = request.size();
        let pixels = request.pixels_mut();
        for pixel in &mut *pixels {
            *pixel = Rgba8 {
                r: 0x12,
                g: 0x34,
                b: 0x56,
                a: 0xff,
            };
        }
        self.captures
            .borrow_mut()
            .push((viewport_id, origin, size, pixels.len()));
        Ok(())
    }
}

#[test]
fn runner_distinguishes_pass_failure_and_no_match() {
    let _guard = test_lock();

    let mut pass_context = context();
    let mut pass_engine = attached_engine(&mut pass_context);
    pass_engine
        .add_script_test("runner", "pass", |script| {
            script.yield_frames(ScriptCount::new(2)?)
        })
        .expect("passing script");
    let passed = TestRunner::new(&mut pass_engine)
        .filter("pass")
        .frame_budget(nonzero(64))
        .run_headless(&mut pass_context, no_error)
        .expect("passing run");
    assert_eq!(passed.outcome(), RunOutcome::Passed);
    assert_eq!(passed.summary().count_tested, 1);
    assert_eq!(passed.summary().count_success, 1);
    assert_eq!(passed.summary().count_in_queue, 0);
    assert_eq!(passed.tests().len(), 1);
    assert_eq!(passed.tests()[0].category(), "runner");
    assert_eq!(passed.tests()[0].name(), "pass");
    assert_eq!(passed.tests()[0].status(), RunTestStatus::Success);
    assert_eq!(passed.mode(), RunMode::Headless);
    assert!(passed.frames() > 0);
    let no_match_after_pass = TestRunner::new(&mut pass_engine)
        .filter("does-not-match-after-pass")
        .run_headless(&mut pass_context, no_error)
        .expect("historical success must not pollute a no-match run");
    assert_eq!(no_match_after_pass.outcome(), RunOutcome::NoMatch);
    assert_eq!(no_match_after_pass.summary().count_tested, 0);
    assert_eq!(no_match_after_pass.summary().count_success, 0);
    assert!(no_match_after_pass.tests().is_empty());
    assert_ne!(passed.run_id(), no_match_after_pass.run_id());
    pass_engine.shutdown().expect("pass shutdown");
    drop(pass_context);

    let mut boundary_context = context();
    let mut boundary_engine = attached_engine(&mut boundary_context);
    boundary_engine
        .add_script_test("runner", "boundary", |script| {
            script.yield_frames(ScriptCount::new(2)?)
        })
        .expect("boundary script");
    let boundary = TestRunner::new(&mut boundary_engine)
        .filter("boundary")
        .frame_budget(nonzero(passed.frames()))
        .run_headless(&mut boundary_context, no_error)
        .expect("terminal frame at the budget boundary must win over timeout");
    assert_eq!(boundary.outcome(), RunOutcome::Passed);
    assert_eq!(boundary.frames(), passed.frames());
    boundary_engine.shutdown().expect("boundary shutdown");
    drop(boundary_context);

    let mut failure_context = context();
    let mut failure_engine = attached_engine(&mut failure_context);
    failure_engine
        .add_script_test("runner", "failure", |script| {
            script.set_ref("Failure Host")?;
            script.table_set_column_enabled_by_label("Missing Table", "Column", true)
        })
        .expect("failing script");
    let failed = TestRunner::new(&mut failure_engine)
        .filter("failure")
        .frame_budget(nonzero(64))
        .run_headless(&mut failure_context, |ui, _| {
            ui.window("Failure Host")
                .build(|| ui.text("No table is intentionally created"));
            Ok::<_, Infallible>(RunnerControl::Continue)
        })
        .expect("script assertion is a product outcome");
    assert_eq!(failed.outcome(), RunOutcome::Failed);
    assert_eq!(failed.summary().count_tested, 1);
    assert_eq!(failed.summary().count_success, 0);
    assert_eq!(failed.tests()[0].status(), RunTestStatus::Error);
    failure_engine.shutdown().expect("failure shutdown");
    drop(failure_context);

    let mut no_match_context = context();
    let mut no_match_engine = attached_engine(&mut no_match_context);
    no_match_engine
        .add_script_test("runner", "registered", |script| {
            script.yield_frames(ScriptCount::new(1)?)
        })
        .expect("registered script");
    let no_match = TestRunner::new(&mut no_match_engine)
        .filter("does-not-match-any-test")
        .run_headless(&mut no_match_context, no_error)
        .expect("no match is a product outcome");
    assert_eq!(no_match.outcome(), RunOutcome::NoMatch);
    assert_eq!(no_match.summary().count_tested, 0);
    assert_eq!(no_match.summary().count_success, 0);
    assert_eq!(no_match.summary().count_in_queue, 0);
    assert_eq!(no_match.frames(), 0);
    no_match_engine.shutdown().expect("no-match shutdown");
    drop(no_match_context);
}

#[test]
fn runner_distinguishes_timeout_and_explicit_abort_after_cleanup() {
    let _guard = test_lock();

    let mut timeout_context = context();
    let mut timeout_engine = attached_engine(&mut timeout_context);
    timeout_engine
        .add_script_test("runner", "timeout", |script| {
            script.yield_frames(ScriptCount::new(10_000)?)
        })
        .expect("timeout script");
    let timed_out = TestRunner::new(&mut timeout_engine)
        .filter("timeout")
        .frame_budget(nonzero(1))
        .cleanup_frame_budget(nonzero(64))
        .run_headless(&mut timeout_context, no_error)
        .expect("timeout cleanup must settle");
    assert_eq!(timed_out.outcome(), RunOutcome::TimedOut);
    assert_eq!(timed_out.summary().count_in_queue, 0);
    assert_eq!(timed_out.tests()[0].status(), RunTestStatus::NotRun);
    timeout_engine.shutdown().expect("timeout shutdown");
    drop(timeout_context);

    let mut abort_context = context();
    let mut abort_engine = attached_engine(&mut abort_context);
    abort_engine
        .add_script_test("runner", "abort", |script| {
            script.yield_frames(ScriptCount::new(10_000)?)
        })
        .expect("abort script");
    let aborted = TestRunner::new(&mut abort_engine)
        .filter("abort")
        .frame_budget(nonzero(64))
        .cleanup_frame_budget(nonzero(64))
        .run_headless(&mut abort_context, |_, frame| {
            Ok::<_, Infallible>(if frame == 1 {
                RunnerControl::Abort
            } else {
                RunnerControl::Continue
            })
        })
        .expect("abort cleanup must settle");
    assert_eq!(aborted.outcome(), RunOutcome::Aborted);
    assert_eq!(aborted.summary().count_in_queue, 0);
    assert_eq!(aborted.tests()[0].status(), RunTestStatus::NotRun);
    abort_engine.shutdown().expect("abort shutdown");
    drop(abort_context);
}

#[test]
fn ffi_and_callback_failures_remain_infrastructure_errors() {
    let _guard = test_lock();

    let mut ffi_context = context();
    let mut ffi_engine = attached_engine(&mut ffi_context);
    ffi_engine
        .add_script_test("runner", "ffi", |script| {
            script.yield_frames(ScriptCount::new(1)?)
        })
        .expect("ffi script");
    assert_eq!(
        unsafe {
            raw::imgui_test_engine_test_set_exception_injection(
                raw::ImGuiTestEngineExceptionPoint_UpstreamCall,
            )
        },
        raw::ImGuiTestEngineStatus_Success
    );
    let ffi_error = TestRunner::new(&mut ffi_engine)
        .filter("ffi")
        .run_headless(&mut ffi_context, no_error)
        .expect_err("injected FFI failure must not become an outcome");
    assert!(matches!(
        ffi_error,
        RunnerError::TestEngine(TestEngineError::Ffi {
            status: TestEngineStatus::Exception,
            ..
        })
    ));
    assert_eq!(ffi_engine.run_state(), RunState::Inactive);
    ffi_engine.shutdown().expect("ffi shutdown");
    drop(ffi_context);

    let mut callback_context = context();
    let mut callback_engine = attached_engine(&mut callback_context);
    callback_engine
        .add_script_test("runner", "callback", |script| {
            script.yield_frames(ScriptCount::new(10)?)
        })
        .expect("callback script");
    let callback_error = TestRunner::new(&mut callback_engine)
        .filter("callback")
        .run_headless(&mut callback_context, |_, _| {
            Err::<RunnerControl, _>(io::Error::other("injected callback failure"))
        })
        .expect_err("callback failure must not become an outcome");
    assert!(matches!(
        callback_error,
        RunnerError::ApplicationUi { frame: 1, .. }
    ));
    assert_eq!(
        callback_context.frame_lifecycle_state(),
        FrameLifecycleState::Idle
    );
    assert_eq!(callback_engine.run_state(), RunState::Inactive);
    callback_engine.shutdown().expect("callback shutdown");
    drop(callback_context);

    let mut render_context = context();
    let mut render_engine = attached_engine(&mut render_context);
    render_engine
        .add_script_test("runner", "render", |script| {
            script.yield_frames(ScriptCount::new(10)?)
        })
        .expect("render script");
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut driver = RecordingDriver::new(events, DriverFault::Render);
    let render_error = TestRunner::new(&mut render_engine)
        .filter("render")
        .run_graphical(
            &mut render_context,
            |_, _| Ok::<_, io::Error>(RunnerControl::Continue),
            &mut driver,
        )
        .expect_err("render failure must not become an outcome");
    assert!(matches!(
        render_error,
        RunnerError::FrameDriver {
            frame: 1,
            source: FrameDriverError::Render(_),
        }
    ));
    assert_eq!(
        render_context.frame_lifecycle_state(),
        FrameLifecycleState::Rendered
    );
    assert_eq!(render_engine.run_state(), RunState::Inactive);
    render_engine.shutdown().expect("render shutdown");
    drop(render_context);
}

#[test]
fn runner_pumps_ui_render_and_swap_boundaries_once_per_frame_in_order() {
    let _guard = test_lock();
    let mut context = context();
    let mut engine = attached_engine(&mut context);
    engine
        .add_script_test("runner", "order", |script| {
            script.yield_frames(ScriptCount::new(3)?)
        })
        .expect("ordered script");

    let events = Rc::new(RefCell::new(Vec::new()));
    set_presentation_trace(&engine, Some(&events));
    let ui_events = events.clone();
    let mut driver = RecordingDriver::new(events.clone(), DriverFault::None);
    let report = TestRunner::new(&mut engine)
        .filter("order")
        .frame_budget(nonzero(64))
        .run_graphical(
            &mut context,
            move |_, frame| {
                ui_events.borrow_mut().push((frame, "ui"));
                Ok::<_, Infallible>(RunnerControl::Continue)
            },
            &mut driver,
        )
        .expect("ordered run");
    assert_eq!(report.outcome(), RunOutcome::Passed);
    assert_eq!(report.mode(), RunMode::Graphical);

    let events = events.borrow();
    assert_eq!(events.len() as u64, report.frames() * 5);
    for (index, phases) in events.chunks_exact(5).enumerate() {
        let frame = index as u64 + 1;
        assert_eq!(
            phases,
            [
                (frame, "ui"),
                (frame, "render"),
                (0, "pre-swap"),
                (frame, "present"),
                (0, "post-swap"),
            ]
        );
    }
    drop(events);
    set_presentation_trace(&engine, None);
    engine.shutdown().expect("order shutdown");
    drop(context);
}

fn assert_driver_failure(
    fault: DriverFault,
    expected_phase: dear_imgui_test_engine::FrameDriverPhase,
) {
    let mut context = context();
    let mut engine = attached_engine(&mut context);
    engine
        .add_script_test("runner", "phase", |script| {
            script.yield_frames(ScriptCount::new(10)?)
        })
        .expect("phase script");

    let events = Rc::new(RefCell::new(Vec::new()));
    set_presentation_trace(&engine, Some(&events));
    let ui_events = events.clone();
    let mut driver = RecordingDriver::new(events.clone(), fault);
    let error = TestRunner::new(&mut engine)
        .filter("phase")
        .frame_budget(nonzero(64))
        .run_graphical(
            &mut context,
            move |_, frame| {
                ui_events.borrow_mut().push((frame, "ui"));
                Ok::<_, Infallible>(RunnerControl::Continue)
            },
            &mut driver,
        )
        .expect_err("injected frame-driver failure");

    let source = match error {
        RunnerError::FrameDriver { frame: 1, source } => source,
        other => panic!("unexpected runner error: {other:?}"),
    };
    assert_eq!(source.phase(), Some(expected_phase));
    assert!(source.abort_error().is_none());
    match fault {
        DriverFault::Render => assert!(matches!(source, FrameDriverError::Render(_))),
        DriverFault::PreSwap => assert!(matches!(
            source,
            FrameDriverError::PreSwap(TestEngineError::Ffi {
                operation: "imgui_test_engine_pre_swap",
                status: TestEngineStatus::Exception,
                ..
            })
        )),
        DriverFault::Present => assert!(matches!(source, FrameDriverError::Present { .. })),
        DriverFault::PostSwap => assert!(matches!(
            source,
            FrameDriverError::PostSwap {
                source: TestEngineError::Ffi {
                    operation: "imgui_test_engine_post_swap",
                    status: TestEngineStatus::Exception,
                    ..
                },
                ..
            }
        )),
        DriverFault::None => panic!("failure helper requires a fault"),
    }

    let events = events.borrow();
    let expected = match fault {
        DriverFault::Render => vec![(1, "ui"), (1, "render")],
        DriverFault::PreSwap => vec![(1, "ui"), (1, "render")],
        DriverFault::Present | DriverFault::PostSwap => {
            vec![(1, "ui"), (1, "render"), (0, "pre-swap"), (1, "present")]
        }
        DriverFault::None => unreachable!(),
    };
    assert_eq!(*events, expected);
    drop(events);
    assert_eq!(
        context.frame_lifecycle_state(),
        FrameLifecycleState::Rendered
    );
    assert_eq!(engine.run_state(), RunState::Inactive);
    set_presentation_trace(&engine, None);
    engine.shutdown().expect("phase shutdown");
    assert_eq!(
        engine.attachment_state(),
        dear_imgui_test_engine::AttachmentState::Destroyed
    );
    drop(context);
}

#[test]
fn runner_errors_name_every_frame_driver_phase_and_teardown_deterministically() {
    let _guard = test_lock();
    use dear_imgui_test_engine::FrameDriverPhase;

    assert_driver_failure(DriverFault::Render, FrameDriverPhase::Render);
    assert_driver_failure(DriverFault::PreSwap, FrameDriverPhase::PreSwap);
    assert_driver_failure(DriverFault::Present, FrameDriverPhase::Present);
    assert_driver_failure(DriverFault::PostSwap, FrameDriverPhase::PostSwap);
}

#[test]
fn runner_rejects_an_unreconciled_render_lease_before_presentation() {
    struct UnreconciledDriver {
        present_calls: Rc<Cell<usize>>,
    }

    impl TestFrameDriver for UnreconciledDriver {
        type RenderError = RendererConsumerError;
        type PresentError = Infallible;

        fn render<'frame>(
            &mut self,
            frame: RenderedFrame<'frame>,
            _frame_index: u64,
        ) -> Result<ReconciledFrame<'frame>, Self::RenderError> {
            frame.into_reconciled()
        }

        fn present(&mut self, _frame_index: u64) -> Result<(), Self::PresentError> {
            self.present_calls.set(self.present_calls.get() + 1);
            Ok(())
        }
    }

    let _guard = test_lock();
    let mut context = context();
    let mut engine = attached_engine(&mut context);
    engine
        .add_script_test("runner", "unreconciled", |script| {
            script.yield_frames(ScriptCount::new(1)?)
        })
        .expect("unreconciled script");
    let present_calls = Rc::new(Cell::new(0));
    let mut driver = UnreconciledDriver {
        present_calls: present_calls.clone(),
    };

    let error = TestRunner::new(&mut engine)
        .filter("unreconciled")
        .run_graphical(&mut context, no_error, &mut driver)
        .expect_err("an unreconciled lease must fail before presentation");
    assert!(matches!(
        error,
        RunnerError::FrameDriver {
            frame: 1,
            source: FrameDriverError::Render(RendererConsumerError::FrameNotReconciled { .. }),
        }
    ));
    assert_eq!(present_calls.get(), 0);
    assert_eq!(
        context.frame_lifecycle_state(),
        FrameLifecycleState::Rendered
    );
    assert_eq!(engine.run_state(), RunState::Inactive);
    engine.shutdown().expect("unreconciled shutdown");
}

#[test]
fn completed_post_swap_is_not_aborted_when_run_state_refresh_fails() {
    let _guard = test_lock();
    let mut context = context();
    let mut engine = attached_engine(&mut context);
    engine
        .add_script_test("runner", "refresh", |script| {
            script.yield_frames(ScriptCount::new(1)?)
        })
        .expect("refresh script");
    assert_eq!(
        unsafe {
            raw::imgui_test_engine_test_set_presentation_trace(
                engine.as_raw(),
                Some(inject_failure_after_completed_post_swap),
                std::ptr::null_mut(),
            )
        },
        raw::ImGuiTestEngineStatus_Success
    );
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut driver = RecordingDriver::new(events, DriverFault::None);

    let error = TestRunner::new(&mut engine)
        .filter("refresh")
        .run_graphical(&mut context, no_error, &mut driver)
        .expect_err("run-state refresh fault must be reported");
    assert!(matches!(
        error,
        RunnerError::FrameDriver {
            frame: 1,
            source: FrameDriverError::PostSwap {
                source: TestEngineError::Ffi {
                    operation: "imgui_test_engine_is_running_tests",
                    status: TestEngineStatus::Exception,
                    ..
                },
                abort_error: None,
            },
        }
    ));
    set_presentation_trace(&engine, None);
    assert_eq!(engine.run_state(), RunState::Inactive);
    engine.shutdown().expect("refresh failure shutdown");
}

#[cfg(feature = "capture")]
#[test]
fn direct_frame_drive_clears_capture_abort_after_the_engine_settles() {
    let _guard = test_lock();
    let mut context = context();
    let mut engine = attached_engine(&mut context);
    engine
        .add_script_test("runner", "direct-abort", |script| {
            script.yield_frames(ScriptCount::new(20)?)
        })
        .expect("direct-drive script");
    engine
        .queue_tests(TestGroup::Tests, Some("direct-abort"), RunFlags::NONE)
        .expect("queue direct-drive script");

    let ui = context.frame();
    engine.show_windows(ui, None).expect("show engine windows");
    let mut failing = RecordingDriver::new(Rc::new(RefCell::new(Vec::new())), DriverFault::Present);
    assert!(matches!(
        engine.drive_frame(context.render(), 1, &mut failing),
        Err(FrameDriverError::Present { .. })
    ));
    assert!(capture_state(&engine).CaptureAbortRequested);

    let mut recovered = RecordingDriver::new(Rc::new(RefCell::new(Vec::new())), DriverFault::None);
    for frame_index in 2..=65 {
        let ui = context.frame();
        engine.show_windows(ui, None).expect("show engine windows");
        engine
            .drive_frame(context.render(), frame_index, &mut recovered)
            .expect("cleanup frame settles the aborted presentation");
        if !capture_state(&engine).CaptureAbortRequested {
            break;
        }
    }
    assert_capture_state_clear(&engine);

    engine.stop().expect("stop direct-drive engine");
    engine.shutdown().expect("direct-drive shutdown");
}

#[test]
fn runner_rejects_wrong_or_open_context_and_restores_nested_current_context() {
    let _guard = test_lock();

    let foreign = Context::create();
    let foreign_binding = foreign.binding();
    let foreign_raw = foreign.as_raw();
    let foreign_suspended = foreign.suspend();

    let mut owner_context = context();
    let owner_raw = owner_context.as_raw();
    let mut engine = attached_engine(&mut owner_context);
    engine
        .add_script_test("runner", "context", |script| {
            script.yield_frames(ScriptCount::new(1)?)
        })
        .expect("context script");

    owner_context.frame().text("already open");
    let open_error = TestRunner::new(&mut engine)
        .filter("context")
        .run_headless(&mut owner_context, no_error)
        .expect_err("open frame must be rejected");
    assert!(matches!(open_error, RunnerError::FrameAlreadyOpen));
    drop(owner_context.render());
    assert_eq!(engine.run_state(), RunState::Ready);

    foreign_binding
        .try_with_bound_context(|| {
            assert_eq!(
                unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
                foreign_raw
            );
            let report = TestRunner::new(&mut engine)
                .filter("context")
                .run_headless(&mut owner_context, no_error)
                .expect("nested runner");
            assert_eq!(report.outcome(), RunOutcome::Passed);
            assert_eq!(
                unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
                foreign_raw
            );
        })
        .expect("foreign binding");
    assert_eq!(
        unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
        owner_raw
    );
    drop(foreign_suspended);

    engine.shutdown().expect("context shutdown");
    drop(owner_context);

    let mut attached_context = context();
    let mut mismatch_engine = attached_engine(&mut attached_context);
    let suspended_attached = attached_context.suspend();
    let mut wrong_context = context();
    let mismatch = TestRunner::new(&mut mismatch_engine)
        .run_headless(&mut wrong_context, no_error)
        .expect_err("wrong Context must be rejected before queueing");
    assert!(matches!(mismatch, RunnerError::ContextMismatch { .. }));
    assert_eq!(mismatch_engine.run_state(), RunState::Ready);
    drop(wrong_context);
    let attached_context = suspended_attached.activate().expect("reactivate owner");
    mismatch_engine.shutdown().expect("mismatch shutdown");
    drop(attached_context);
}

#[test]
fn headless_runner_rejects_managed_texture_requests_without_abandoning_silently() {
    let _guard = test_lock();
    let mut context = Context::create();
    let _consumer = context
        .create_renderer_consumer()
        .expect("renderer consumer");
    let _ = context.font_atlas().add_font_default(None);
    context.io_mut().set_display_size([128.0, 128.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context
        .io_mut()
        .set_backend_flags(BackendFlags::RENDERER_HAS_TEXTURES);
    let mut engine = attached_engine(&mut context);
    engine
        .add_script_test("runner", "texture", |script| {
            script.yield_frames(ScriptCount::new(1)?)
        })
        .expect("texture script");

    let error = TestRunner::new(&mut engine)
        .filter("texture")
        .run_headless(&mut context, no_error)
        .expect_err("headless mode cannot upload managed textures");
    assert!(matches!(
        error,
        RunnerError::FrameDriver {
            frame: 1,
            source: FrameDriverError::Render(HeadlessRenderError::ManagedTextureRequests {
                count: 1..
            }),
        }
    ));
    assert_eq!(engine.run_state(), RunState::Inactive);
    engine.shutdown().expect("texture shutdown");
    drop(context);
}

#[cfg(feature = "capture")]
#[test]
fn graphical_capture_provider_is_run_scoped_and_reports_region_metadata() {
    let _guard = test_lock();
    let mut context = context();
    let mut engine = attached_engine(&mut context);
    register_capture_script(&mut engine, "success");
    let captures = Rc::new(RefCell::new(Vec::new()));
    let mut driver = CapturingDriver {
        captures: captures.clone(),
        last_presented: Rc::new(Cell::new(0)),
        frame_fault: DriverFault::None,
        fault: CaptureFault::None,
        fault_armed: false,
    };

    let report = TestRunner::new(&mut engine)
        .filter("success")
        .frame_budget(nonzero(64))
        .run_graphical_with_capture(&mut context, capture_ui, &mut driver)
        .expect("capturing run");

    assert_eq!(report.outcome(), RunOutcome::Passed);
    assert_eq!(report.mode(), RunMode::GraphicalWithCapture);
    assert_capture_provider_cleared(&engine);
    let captures = captures.borrow();
    assert!(!captures.is_empty());
    for &(_viewport_id, _origin, [width, height], pixel_count) in captures.iter() {
        assert!(width > 0 && height > 0);
        assert_eq!(pixel_count, width as usize * height as usize);
    }

    engine.shutdown().expect("shutdown");
}

#[cfg(feature = "capture")]
#[test]
fn capture_provider_errors_and_panics_do_not_cross_the_ffi_boundary() {
    let _guard = test_lock();
    PANICKING_PAYLOAD_DROP_COUNT.store(0, Ordering::Release);
    for fault in [
        CaptureFault::Error,
        CaptureFault::Panic,
        CaptureFault::PanicWithPanickingPayload,
    ] {
        let mut context = context();
        let mut engine = attached_engine(&mut context);
        register_capture_script(&mut engine, "failure");
        let last_presented = Rc::new(Cell::new(0));
        let mut driver = CapturingDriver {
            captures: Rc::new(RefCell::new(Vec::new())),
            last_presented: last_presented.clone(),
            frame_fault: DriverFault::None,
            fault,
            fault_armed: false,
        };

        let error = TestRunner::new(&mut engine)
            .filter("failure")
            .frame_budget(nonzero(64))
            .run_graphical_with_capture(&mut context, capture_ui, &mut driver)
            .expect_err("capture failure must be typed");
        let (frame, phase, source) = match error {
            RunnerError::Capture {
                frame,
                phase,
                source,
            } => (frame, phase, source),
            other => panic!("unexpected capture error: {other:?}"),
        };
        assert_eq!(frame, last_presented.get());
        assert_eq!(
            phase,
            Some(dear_imgui_test_engine::FrameDriverPhase::PostSwap)
        );
        match (fault, source) {
            (CaptureFault::Error, CaptureProviderError::Driver(_))
            | (CaptureFault::Panic, CaptureProviderError::Panicked)
            | (CaptureFault::PanicWithPanickingPayload, CaptureProviderError::Panicked) => {}
            (_, other) => panic!("unexpected capture source: {other:?}"),
        }
        assert_eq!(engine.run_state(), RunState::Inactive);
        assert_capture_provider_cleared(&engine);
        assert_eq!(
            context.frame_lifecycle_state(),
            FrameLifecycleState::Rendered
        );
        engine.shutdown().expect("shutdown after capture failure");
    }
    assert_eq!(PANICKING_PAYLOAD_DROP_COUNT.load(Ordering::Acquire), 1);
}

#[cfg(feature = "capture")]
#[test]
fn capture_failure_preserves_a_secondary_teardown_error() {
    let _guard = test_lock();
    let mut context = context();
    let mut engine = attached_engine(&mut context);
    register_capture_script(&mut engine, "teardown");
    let last_presented = Rc::new(Cell::new(0));
    let mut driver = CapturingDriver {
        captures: Rc::new(RefCell::new(Vec::new())),
        last_presented: last_presented.clone(),
        frame_fault: DriverFault::None,
        fault: CaptureFault::ErrorAndStopFailure,
        fault_armed: false,
    };

    let error = TestRunner::new(&mut engine)
        .filter("teardown")
        .frame_budget(nonzero(64))
        .run_graphical_with_capture(&mut context, capture_ui, &mut driver)
        .expect_err("capture and teardown failures must both be retained");
    let RunnerError::Teardown { primary, source } = error else {
        panic!("unexpected combined error: {error:?}");
    };
    assert!(matches!(
        *primary,
        RunnerError::Capture {
            frame,
            phase: Some(dear_imgui_test_engine::FrameDriverPhase::PostSwap),
            source: CaptureProviderError::Driver(_),
        } if frame == last_presented.get()
    ));
    assert!(matches!(
        source,
        TestEngineError::Ffi {
            operation: "imgui_test_engine_stop",
            status: TestEngineStatus::Exception,
            ..
        }
    ));
    assert_ne!(engine.run_state(), RunState::Inactive);
    assert_capture_provider_cleared(&engine);
    let state = capture_state(&engine);
    assert!(
        state.CaptureWaitPending,
        "state after failed stop: {state:?}"
    );
    assert!(
        state.CaptureAbortRequested,
        "state after failed stop: {state:?}"
    );
    assert!(state.EngineAbort, "state after failed stop: {state:?}");
    engine.stop().expect("retry stop after one-shot failure");
    engine.shutdown().expect("shutdown after retry");
}

#[cfg(feature = "capture")]
#[test]
fn pending_capture_waits_are_cancelled_for_every_frame_driver_failure() {
    let _guard = test_lock();
    for frame_fault in [
        DriverFault::Render,
        DriverFault::PreSwap,
        DriverFault::Present,
        DriverFault::PostSwap,
    ] {
        let mut context = context();
        let mut engine = attached_engine(&mut context);
        register_immediate_capture_script(&mut engine, "cancel");
        let mut driver = CapturingDriver {
            captures: Rc::new(RefCell::new(Vec::new())),
            last_presented: Rc::new(Cell::new(0)),
            frame_fault,
            fault: CaptureFault::None,
            fault_armed: false,
        };
        let original_hidden = Rc::new(Cell::new(None));
        let original_hidden_for_ui = original_hidden.clone();

        let error = TestRunner::new(&mut engine)
            .filter("cancel")
            .frame_budget(nonzero(64))
            .run_graphical_with_capture(
                &mut context,
                move |ui, frame| {
                    let control = capture_ui(ui, frame)?;
                    if original_hidden_for_ui.get().is_none() {
                        original_hidden_for_ui
                            .set(Some(hidden_frames_for_render_only(c"Foreign Window")));
                    }
                    Ok::<_, Infallible>(control)
                },
                &mut driver,
            )
            .expect_err("frame failure must cancel the pending capture wait");
        assert!(matches!(
            (frame_fault, error),
            (
                DriverFault::Render,
                RunnerError::FrameDriver {
                    frame: 1,
                    source: FrameDriverError::Render(_),
                },
            ) | (
                DriverFault::Present,
                RunnerError::FrameDriver {
                    frame: 1,
                    source: FrameDriverError::Present { .. },
                },
            ) | (
                DriverFault::PreSwap,
                RunnerError::FrameDriver {
                    frame: 1,
                    source: FrameDriverError::PreSwap(_),
                },
            ) | (
                DriverFault::PostSwap,
                RunnerError::FrameDriver {
                    frame: 1,
                    source: FrameDriverError::PostSwap { .. },
                },
            )
        ));
        assert_eq!(
            Some(hidden_frames_for_render_only(c"Foreign Window")),
            original_hidden.get(),
            "capture cancellation changed the foreign window visibility countdown"
        );
        assert_eq!(engine.run_state(), RunState::Inactive);
        assert_capture_provider_cleared(&engine);
        assert_capture_state_clear(&engine);
        engine
            .shutdown()
            .expect("shutdown after capture cancellation");
    }
}

#[cfg(feature = "capture")]
#[test]
fn clearing_an_interactive_capture_provider_does_not_abort_the_next_run() {
    let _guard = test_lock();
    let mut context = context();
    let mut engine = attached_engine(&mut context);
    for name in ["first", "second"] {
        engine
            .add_script_test("runner_interactive", name, |script| {
                script.yield_frames(ScriptCount::new(1)?)
            })
            .expect("interactive cleanup script");
    }
    assert_eq!(
        unsafe {
            raw::imgui_test_engine_test_set_presentation_trace(
                engine.as_raw(),
                Some(arm_interactive_capture_after_post_swap),
                engine.as_raw().cast(),
            )
        },
        raw::ImGuiTestEngineStatus_Success
    );
    let mut driver = CapturingDriver {
        captures: Rc::new(RefCell::new(Vec::new())),
        last_presented: Rc::new(Cell::new(0)),
        frame_fault: DriverFault::None,
        fault: CaptureFault::None,
        fault_armed: false,
    };

    let first = TestRunner::new(&mut engine)
        .filter("first")
        .run_graphical_with_capture(&mut context, no_error, &mut driver)
        .expect("first run with interactive capture state");
    assert_eq!(first.outcome(), RunOutcome::Passed);
    assert_eq!(first.summary().count_tested, 1);
    set_presentation_trace(&engine, None);
    assert_capture_state_clear(&engine);

    let second = TestRunner::new(&mut engine)
        .filter("second")
        .run_headless(&mut context, no_error)
        .expect("capture-only cleanup must not abort the next run");
    assert_eq!(second.outcome(), RunOutcome::Passed);
    assert_eq!(second.summary().count_tested, 1);
    assert_ne!(first.run_id(), second.run_id());
    engine.shutdown().expect("interactive cleanup shutdown");
}

#[cfg(feature = "capture")]
#[test]
fn headless_capture_request_is_unavailable_without_asserting_or_hanging() {
    let _guard = test_lock();
    let mut context = context();
    let mut engine = attached_engine(&mut context);
    register_capture_script(&mut engine, "unavailable");

    let report = TestRunner::new(&mut engine)
        .filter("unavailable")
        .frame_budget(nonzero(64))
        .run_headless(&mut context, capture_ui)
        .expect("missing capture provider must fail closed without native assertion");

    assert_eq!(report.mode(), RunMode::Headless);
    assert_eq!(report.outcome(), RunOutcome::Failed);
    engine.shutdown().expect("shutdown");
}
