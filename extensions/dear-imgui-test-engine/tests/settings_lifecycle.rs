use std::alloc::{Layout, alloc, dealloc};
use std::convert::Infallible;
use std::ffi::c_void;
use std::fs;
use std::sync::{Mutex, MutexGuard, OnceLock};

use dear_imgui_rs::{
    Condition, Context, ContextLifecycle, FrameToken, SuspendedContext, TableColumnIndex,
    TableFlags, render::ReconciledFrame,
};
use dear_imgui_test_engine::{
    AttachmentState, BuiltInTestSuite, CaptureOutput, FrameDriverError, MainRenderOutcome,
    RunFlags, RunSpeed, RunState, ScriptCount, TestEngine, TestEngineError, TestEngineResult,
    TestEngineStatus, TestFrameDriver, TestGroup, TestRunner, VerboseLevel, raw,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());
static REUSE_ALLOCATOR: OnceLock<Mutex<Option<ReuseAllocatorState>>> = OnceLock::new();

const ALLOCATION_HEADER_SIZE: usize = 16;
const ALLOCATION_ALIGNMENT: usize = 16;

struct VirtualFrameDriver;

impl TestFrameDriver for VirtualFrameDriver {
    type PreparedFrame<'frame> = ReconciledFrame<'frame>;
    type PrepareError = Infallible;
    type RenderError = Infallible;
    type PresentError = Infallible;

    fn prepare<'frame>(
        &mut self,
        frame: FrameToken<'frame>,
        _frame_index: u64,
    ) -> Result<Self::PreparedFrame<'frame>, Self::PrepareError> {
        Ok(frame.render_legacy())
    }

    fn prepared_context_id(frame: &Self::PreparedFrame<'_>) -> dear_imgui_rs::ContextId {
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

#[derive(Default)]
struct ReuseAllocatorState {
    capture_first: bool,
    target: usize,
    target_size: usize,
    target_live: bool,
    reuse_armed: bool,
}

struct ReuseAllocatorFixture {
    previous_alloc: dear_imgui_rs::sys::ImGuiMemAllocFunc,
    previous_free: dear_imgui_rs::sys::ImGuiMemFreeFunc,
    previous_user_data: *mut c_void,
}

impl ReuseAllocatorFixture {
    fn install() -> Self {
        let mut previous_alloc = None;
        let mut previous_free = None;
        let mut previous_user_data = std::ptr::null_mut();
        unsafe {
            dear_imgui_rs::sys::igGetAllocatorFunctions(
                &mut previous_alloc,
                &mut previous_free,
                &mut previous_user_data,
            );
        }
        REUSE_ALLOCATOR
            .get_or_init(|| Mutex::new(None))
            .lock()
            .expect("reuse allocator lock")
            .replace(ReuseAllocatorState {
                capture_first: true,
                ..ReuseAllocatorState::default()
            });
        unsafe {
            dear_imgui_rs::sys::igSetAllocatorFunctions(
                Some(reuse_alloc),
                Some(reuse_free),
                std::ptr::null_mut(),
            );
        }
        Self {
            previous_alloc,
            previous_free,
            previous_user_data,
        }
    }

    fn arm_reuse(&self) {
        let mut state = REUSE_ALLOCATOR
            .get()
            .expect("reuse allocator state")
            .lock()
            .expect("reuse allocator lock");
        let state = state.as_mut().expect("installed reuse allocator");
        assert_ne!(state.target, 0, "first allocation was not captured");
        assert!(
            !state.target_live,
            "captured Context allocation is still live"
        );
        state.reuse_armed = true;
    }
}

impl Drop for ReuseAllocatorFixture {
    fn drop(&mut self) {
        unsafe {
            dear_imgui_rs::sys::igSetAllocatorFunctions(
                self.previous_alloc,
                self.previous_free,
                self.previous_user_data,
            );
        }
        let state = REUSE_ALLOCATOR
            .get()
            .expect("reuse allocator state")
            .lock()
            .expect("reuse allocator lock")
            .take()
            .expect("installed reuse allocator");
        assert!(!state.target_live, "reused allocation leaked past fixture");
        if state.target != 0 {
            unsafe {
                let base = (state.target as *mut u8).sub(ALLOCATION_HEADER_SIZE);
                dealloc(
                    base,
                    allocation_layout(state.target_size).expect("target layout"),
                );
            }
        }
    }
}

unsafe extern "C" fn reuse_alloc(size: usize, _: *mut c_void) -> *mut c_void {
    let Some(lock) = REUSE_ALLOCATOR.get() else {
        return std::ptr::null_mut();
    };
    let Ok(mut guard) = lock.lock() else {
        return std::ptr::null_mut();
    };
    let Some(state) = guard.as_mut() else {
        return std::ptr::null_mut();
    };
    let size = size.max(1);
    if state.reuse_armed && !state.target_live && size == state.target_size {
        state.reuse_armed = false;
        state.target_live = true;
        return state.target as *mut c_void;
    }

    let Some(layout) = allocation_layout(size) else {
        return std::ptr::null_mut();
    };
    let base = unsafe { alloc(layout) };
    if base.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { base.cast::<usize>().write(size) };
    let result = unsafe { base.add(ALLOCATION_HEADER_SIZE) };
    if state.capture_first {
        state.capture_first = false;
        state.target = result as usize;
        state.target_size = size;
        state.target_live = true;
    }
    result.cast()
}

unsafe extern "C" fn reuse_free(pointer: *mut c_void, _: *mut c_void) {
    if pointer.is_null() {
        return;
    }
    let Some(lock) = REUSE_ALLOCATOR.get() else {
        return;
    };
    let Ok(mut guard) = lock.lock() else {
        return;
    };
    let Some(state) = guard.as_mut() else {
        return;
    };
    if pointer as usize == state.target {
        state.target_live = false;
        return;
    }

    let base = unsafe { pointer.cast::<u8>().sub(ALLOCATION_HEADER_SIZE) };
    let size = unsafe { base.cast::<usize>().read() };
    if let Some(layout) = allocation_layout(size) {
        unsafe { dealloc(base, layout) };
    }
}

fn allocation_layout(size: usize) -> Option<Layout> {
    Layout::from_size_align(
        size.checked_add(ALLOCATION_HEADER_SIZE)?,
        ALLOCATION_ALIGNMENT,
    )
    .ok()
}

fn test_lock() -> MutexGuard<'static, ()> {
    TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn context() -> Context {
    let mut context = Context::create();
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("headless test requires the legacy font-atlas capability")
        .build();
    context.io_mut().set_display_size([128.0, 128.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context
}

fn reset_counters() {
    assert_eq!(
        unsafe { raw::imgui_test_engine_test_reset_lifecycle_counters() },
        raw::ImGuiTestEngineStatus_Success
    );
}

fn counters() -> raw::ImGuiTestEngineLifecycleCounters_c {
    let mut counters = raw::ImGuiTestEngineLifecycleCounters_c::default();
    assert_eq!(
        unsafe { raw::imgui_test_engine_test_get_lifecycle_counters(&mut counters) },
        raw::ImGuiTestEngineStatus_Success
    );
    counters
}

fn inject(point: raw::ImGuiTestEngineExceptionPoint) {
    assert_eq!(
        unsafe { raw::imgui_test_engine_test_set_exception_injection(point) },
        raw::ImGuiTestEngineStatus_Success
    );
}

fn assert_invalid_state<T: std::fmt::Debug>(result: TestEngineResult<T>) {
    assert!(matches!(result, Err(TestEngineError::InvalidState { .. })));
}

fn assert_frame_driver_invalid_state(engine: &mut TestEngine, frame: FrameToken<'_>) {
    let error = engine
        .drive_frame(frame, 0, &mut VirtualFrameDriver)
        .expect_err("frame driver state must be rejected");
    assert!(matches!(
        error,
        FrameDriverError::Context(TestEngineError::InvalidState { .. })
    ));
}

#[test]
fn one_context_accepts_one_engine_and_start_failure_rolls_back_the_slot() {
    let _guard = test_lock();
    reset_counters();
    let mut context = context();

    let mut first = TestEngine::create().expect("first engine");
    inject(raw::ImGuiTestEngineExceptionPoint_PostBind);
    let error = first
        .start(&mut context)
        .expect_err("injected native start must fail");
    assert_eq!(error.status(), Some(TestEngineStatus::Exception));
    assert!(
        error
            .diagnostic()
            .is_some_and(|text| text.contains("injected"))
    );
    assert_eq!(first.attachment_state(), AttachmentState::Detached);

    let mut attached = TestEngine::create().expect("attached engine");
    attached.start(&mut context).expect("slot was rolled back");
    let mut rejected = TestEngine::create().expect("rejected engine");
    assert!(matches!(
        rejected.start(&mut context),
        Err(TestEngineError::Attachment { .. })
    ));

    rejected.shutdown().expect("destroy rejected engine");
    attached.shutdown().expect("shutdown attached engine");
    first.shutdown().expect("destroy failed-start engine");
    drop(context);

    let counters = counters();
    assert_eq!(counters.EnginesCreated, 3);
    assert_eq!(counters.EnginesStarted, 1);
    assert_eq!(counters.EnginesDestroyed, 3);
    assert_eq!(counters.EnginesUnbound, 1);
}

#[test]
fn engine_first_and_context_first_shutdown_destroy_exactly_once() {
    let _guard = test_lock();
    reset_counters();

    let mut first_context = context();
    let mut first_engine = TestEngine::create().expect("engine-first engine");
    first_engine
        .start(&mut first_context)
        .expect("engine-first start");
    first_engine.shutdown().expect("first shutdown");
    first_engine.shutdown().expect("idempotent shutdown");
    let mut replacement = TestEngine::create().expect("replacement engine");
    replacement
        .start(&mut first_context)
        .expect("engine-first detach must release the Context slot");
    replacement.shutdown().expect("replacement shutdown");
    drop(first_context);

    let mut second_context = context();
    let mut second_engine = TestEngine::create().expect("context-first engine");
    second_engine
        .start(&mut second_context)
        .expect("context-first start");
    drop(second_context);
    assert_eq!(
        second_engine.attachment_state(),
        AttachmentState::ContextDestroyed
    );
    second_engine.shutdown().expect("destroy detached engine");
    second_engine
        .shutdown()
        .expect("idempotent detached shutdown");

    let counters = counters();
    assert_eq!(counters.EnginesCreated, 3);
    assert_eq!(counters.EnginesStarted, 3);
    assert_eq!(counters.EnginesStopped, 3);
    assert_eq!(counters.EnginesUnbound, 3);
    assert_eq!(counters.EnginesDestroyed, 3);
}

#[test]
fn script_registration_failure_keeps_rust_ownership_until_native_success() {
    let _guard = test_lock();
    reset_counters();
    let mut context = context();
    let mut engine = TestEngine::create().expect("engine");
    engine.start(&mut context).expect("start");

    let error = engine
        .add_script_test("ownership", "injected", |_| {
            inject(raw::ImGuiTestEngineExceptionPoint_UpstreamCall);
            Ok(())
        })
        .expect_err("registration injection must fail");
    assert_eq!(error.status(), Some(TestEngineStatus::Exception));
    let counters = counters();
    assert_eq!(counters.ScriptsCreated, 1);
    assert_eq!(counters.ScriptsRegistered, 0);
    assert_eq!(counters.ScriptsDestroyed, 1);

    engine.shutdown().expect("shutdown");
    drop(context);
}

#[test]
fn script_inputs_are_rejected_before_native_vector_growth() {
    let _guard = test_lock();
    reset_counters();
    let mut context = context();
    let mut engine = TestEngine::create().expect("engine");
    engine.start(&mut context).expect("start");

    engine
        .add_script_test("validation", "safe", |script| {
            assert!(matches!(
                script.table_click_header("", "Column", dear_imgui_rs::KeyMods::empty()),
                Err(TestEngineError::InvalidInput {
                    argument: "table",
                    ..
                })
            ));
            assert!(matches!(
                script.table_click_header("Table", "", dear_imgui_rs::KeyMods::empty()),
                Err(TestEngineError::InvalidInput {
                    argument: "label",
                    ..
                })
            ));
            assert!(matches!(
                script.table_resize_column(
                    "Table",
                    TableColumnIndex::new(i32::MAX as usize + 1),
                    10.0,
                ),
                Err(TestEngineError::InvalidInput {
                    argument: "column",
                    ..
                })
            ));
            assert!(matches!(
                script.table_resize_column("Table", TableColumnIndex::ZERO, f32::NAN),
                Err(TestEngineError::InvalidInput {
                    argument: "width",
                    ..
                })
            ));
            assert!(matches!(
                script.table_resize_column_by_label("Table", "", 10.0),
                Err(TestEngineError::InvalidInput {
                    argument: "label",
                    ..
                })
            ));
            assert!(matches!(
                script.table_resize_column_by_label("Table", "Column", f32::INFINITY),
                Err(TestEngineError::InvalidInput {
                    argument: "width",
                    ..
                })
            ));
            assert!(matches!(
                script.window_resize("Window", -1.0, 1.0),
                Err(TestEngineError::InvalidInput {
                    argument: "width",
                    ..
                })
            ));
            assert!(matches!(
                script.mouse_move_to_pos(f32::INFINITY, 0.0),
                Err(TestEngineError::InvalidInput {
                    argument: "position",
                    ..
                })
            ));
            assert!(matches!(
                script.item_click("contains\0nul"),
                Err(TestEngineError::InvalidInput { .. })
            ));
            assert!(matches!(
                script.dock_into("", "Window"),
                Err(TestEngineError::InvalidInput {
                    argument: "source",
                    ..
                })
            ));
            assert!(matches!(
                script.dock_into("Window", ""),
                Err(TestEngineError::InvalidInput {
                    argument: "destination",
                    ..
                })
            ));
            script.yield_frames(ScriptCount::new(1)?)
        })
        .expect("valid command remains registerable");

    let snapshot = counters();
    assert_eq!(snapshot.ScriptsCreated, 1);
    assert_eq!(snapshot.ScriptsRegistered, 1);
    engine.shutdown().expect("shutdown");
    drop(context);
    assert_eq!(counters().ScriptsDestroyed, 1);
}

#[test]
fn show_windows_validates_context_identity_and_native_frame_scope() {
    let _guard = test_lock();
    reset_counters();
    let mut context_a = context();
    let mut engine = TestEngine::create().expect("engine");
    engine.start(&mut context_a).expect("start");

    let stale_ui = {
        let ui = context_a.frame();
        engine.show_windows(ui, None).expect("valid attached Ui");
        ui as *const dear_imgui_rs::Ui
    };
    drop(context_a.render_legacy());
    // Ui is stable storage owned by the still-live Context. This intentionally bypasses only the
    // frame borrow so the safe wrapper can prove native WithinFrameScope is checked at runtime.
    let error = unsafe { engine.show_windows(&*stale_ui, None) }
        .expect_err("rendered frame is outside native WithinFrameScope");
    assert!(matches!(error, TestEngineError::FrameNotActive { .. }));

    let suspended_a = context_a.suspend_or_panic();
    let mut context_b = SuspendedContext::create()
        .activate()
        .expect("Context B activation");
    context_b
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("headless test requires the legacy font-atlas capability")
        .build();
    context_b.io_mut().set_display_size([128.0, 128.0]);
    context_b.io_mut().set_delta_time(1.0 / 60.0);
    let ui_b = context_b.frame();
    let error = engine
        .show_windows(ui_b, None)
        .expect_err("foreign Ui must be rejected before binding");
    assert!(matches!(error, TestEngineError::ContextMismatch { .. }));
    drop(context_b.render_legacy());
    let suspended_b = context_b.suspend_or_panic();
    let context_a = suspended_a.activate().expect("Context A activation");

    engine.shutdown().expect("shutdown");
    drop(context_a);
    drop(suspended_b);
}

#[test]
fn queued_and_running_runs_reject_requeue_until_terminal_summary_is_consumed() {
    let _guard = test_lock();
    reset_counters();
    let mut context = context();
    let mut engine = TestEngine::create().expect("engine");
    engine.start(&mut context).expect("start");
    engine
        .add_script_test("state", "long_enough", |script| {
            script.yield_frames(ScriptCount::new(3)?)
        })
        .expect("script registration");
    engine
        .queue_tests(TestGroup::Tests, Some("long_enough"), RunFlags::NONE)
        .expect("initial queue");
    assert_eq!(engine.run_state(), RunState::Queued);
    assert!(matches!(
        engine.queue_tests(TestGroup::Tests, None, RunFlags::NONE),
        Err(TestEngineError::InvalidState {
            run: RunState::Queued,
            ..
        })
    ));

    let mut observed_running = false;
    for _ in 0..32 {
        let frame = context.begin_frame();
        let ui = frame.ui();
        ui.text("run-state host");
        observed_running |= engine.is_running_tests().expect("running query");
        if engine.run_state() == RunState::Running {
            assert!(matches!(
                engine.queue_tests(TestGroup::Tests, None, RunFlags::NONE),
                Err(TestEngineError::InvalidState {
                    run: RunState::Running,
                    ..
                })
            ));
        }
        engine
            .drive_frame(frame, 0, &mut VirtualFrameDriver)
            .expect("virtual frame");
        if engine.run_state() == RunState::Terminal {
            break;
        }
    }
    assert!(observed_running, "native run never entered Running");
    assert_eq!(engine.run_state(), RunState::Terminal);
    assert!(matches!(
        engine.queue_tests(TestGroup::Tests, None, RunFlags::NONE),
        Err(TestEngineError::InvalidState {
            run: RunState::Terminal,
            ..
        })
    ));

    let summary = engine
        .take_terminal_summary()
        .expect("terminal query")
        .expect("terminal summary");
    assert_eq!(summary.count_tested, 1);
    assert_eq!(engine.run_state(), RunState::Ready);
    engine
        .queue_tests(TestGroup::Tests, Some("long_enough"), RunFlags::NONE)
        .expect("queue after terminal consumption");

    engine.shutdown().expect("shutdown queued run");
    drop(context);
}

#[test]
fn context_first_teardown_saves_test_engine_settings_once_before_tombstoning() {
    let _guard = test_lock();
    reset_counters();
    let settings_path = std::env::temp_dir().join(format!(
        "dear-imgui-test-engine-settings-{}.ini",
        std::process::id()
    ));
    let _ = fs::remove_file(&settings_path);

    let mut context = context();
    context
        .set_ini_filename(Some(settings_path.clone()))
        .expect("set ini filename");
    let mut engine = TestEngine::create().expect("engine");
    engine.start(&mut context).expect("start");
    engine
        .set_capture_output(CaptureOutput::Discard)
        .expect("mutate saved Test Engine setting");
    let frame = context.begin_frame();
    frame.ui().text("load and dirty settings");
    drop(frame.render_legacy());

    drop(context);
    assert_eq!(engine.attachment_state(), AttachmentState::ContextDestroyed);
    let settings = fs::read_to_string(&settings_path).expect("saved ini file");
    assert_eq!(settings.matches("[TestEngine][Data]").count(), 1);
    assert!(settings.contains("CaptureEnabled=0"));

    engine.shutdown().expect("destroy detached engine");
    assert_eq!(counters().EnginesUnbound, 1);
    let _ = fs::remove_file(settings_path);
}

#[test]
fn reused_raw_context_address_gets_a_fresh_identity_and_attachment_slot() {
    let _guard = test_lock();
    reset_counters();
    let allocator = ReuseAllocatorFixture::install();

    let mut context_a = context();
    let raw_a = context_a.as_raw();
    let id_a = context_a.id();
    let stale_binding = context_a.binding();
    let mut engine_a = TestEngine::create().expect("engine A");
    engine_a.start(&mut context_a).expect("attach engine A");
    drop(context_a);
    engine_a
        .shutdown()
        .expect("destroy engine A after Context A");
    drop(engine_a);
    assert_eq!(stale_binding.lifecycle(), ContextLifecycle::NativeDestroyed);

    allocator.arm_reuse();
    let mut context_b = context();
    assert_eq!(
        context_b.as_raw(),
        raw_a,
        "fixture must reuse the raw address"
    );
    assert_ne!(
        context_b.id(),
        id_a,
        "Context identity must be generation-based"
    );
    let mut engine_b = TestEngine::create().expect("engine B");
    engine_b
        .start(&mut context_b)
        .expect("old attachment tombstone must not block Context B");
    assert_ne!(
        stale_binding.id(),
        context_b.id(),
        "the old binding must not identify the reused Context"
    );

    engine_b.shutdown().expect("shutdown engine B");
    drop(context_b);
    drop(engine_b);
    drop(allocator);
}

#[test]
fn public_engine_methods_enforce_the_attachment_and_run_state_matrix() {
    let _guard = test_lock();
    reset_counters();

    let mut detached = TestEngine::create().expect("detached engine");
    assert!(!detached.is_bound());
    assert!(!detached.is_started().expect("detached query"));
    assert_invalid_state(detached.result_summary());
    assert_invalid_state(detached.take_terminal_summary());
    assert_invalid_state(detached.register_default_tests());
    assert_invalid_state(detached.add_script_test("state", "detached", |_| Ok(())));
    assert_invalid_state(detached.queue_all_tests());
    let mut detached_probe = context();
    let detached_frame = detached_probe.begin_frame();
    detached_frame.ui().text("detached driver state probe");
    assert_frame_driver_invalid_state(&mut detached, detached_frame);
    drop(detached_probe);
    assert_invalid_state(detached.stop());
    assert_invalid_state(detached.try_abort_engine());
    assert_invalid_state(detached.abort_current_test());
    assert_invalid_state(detached.is_test_queue_empty());
    assert_invalid_state(detached.is_running_tests());
    assert_invalid_state(detached.is_requesting_max_app_speed());
    assert_invalid_state(detached.set_run_speed(RunSpeed::Fast));
    assert_invalid_state(detached.set_verbose_level(VerboseLevel::Info));
    assert_invalid_state(detached.set_verbose_level_on_error(VerboseLevel::Debug));
    assert_invalid_state(detached.set_log_to_tty(true));
    assert_invalid_state(detached.set_capture_output(CaptureOutput::Discard));
    assert_invalid_state(detached.install_default_crash_handler());

    let mut stopped_context = context();
    detached
        .start(&mut stopped_context)
        .expect("start to Ready");
    assert_eq!(detached.run_state(), RunState::Ready);
    assert_invalid_state(detached.try_abort_engine());
    assert_invalid_state(detached.abort_current_test());
    assert!(
        detached
            .take_terminal_summary()
            .expect("Ready terminal poll")
            .is_none()
    );
    detached
        .set_run_speed(RunSpeed::Fast)
        .expect("Ready config");
    detached
        .set_verbose_level(VerboseLevel::Debug)
        .expect("Ready config");
    detached
        .set_verbose_level_on_error(VerboseLevel::Trace)
        .expect("Ready config");
    detached.set_log_to_tty(false).expect("Ready config");
    detached
        .set_capture_output(CaptureOutput::Discard)
        .expect("Ready config");
    detached.stop().expect("Ready to Inactive");
    assert_eq!(detached.run_state(), RunState::Inactive);
    assert_invalid_state(detached.result_summary());
    assert_invalid_state(detached.take_terminal_summary());
    assert_invalid_state(detached.queue_all_tests());
    // The stopped engine retains hooks on `stopped_context`. Do not open another native frame on
    // that Context after stopping solely to manufacture a rejected driver capability; use an
    // unattached probe so this assertion exercises only the engine state gate.
    let suspended_stopped_context = stopped_context.suspend_or_panic();
    let mut inactive_probe = context();
    let inactive_frame = inactive_probe.begin_frame();
    inactive_frame.ui().text("inactive driver state probe");
    assert_frame_driver_invalid_state(&mut detached, inactive_frame);
    let suspended_inactive_probe = inactive_probe.suspend_or_panic();
    let stopped_context = suspended_stopped_context
        .activate()
        .expect("reactivate stopped Context for shutdown");
    assert_invalid_state(detached.stop());
    assert_invalid_state(detached.is_test_queue_empty());
    assert_invalid_state(detached.is_running_tests());
    assert_invalid_state(detached.is_requesting_max_app_speed());
    assert_invalid_state(detached.set_run_speed(RunSpeed::Normal));
    detached.shutdown().expect("Inactive shutdown");
    assert_eq!(detached.attachment_state(), AttachmentState::Destroyed);
    detached
        .shutdown()
        .expect("Destroyed shutdown is idempotent");
    drop(stopped_context);
    drop(suspended_inactive_probe);

    let mut terminal_context = context();
    let mut terminal = TestEngine::create().expect("terminal engine");
    terminal
        .start(&mut terminal_context)
        .expect("terminal start");
    terminal.queue_all_tests().expect("no-match queue");
    assert_eq!(terminal.run_state(), RunState::Terminal);
    assert_invalid_state(terminal.queue_all_tests());
    assert_invalid_state(terminal.set_capture_output(CaptureOutput::Save));
    terminal
        .take_terminal_summary()
        .expect("terminal consume")
        .expect("terminal summary");
    assert_eq!(terminal.run_state(), RunState::Ready);
    drop(terminal_context);
    assert_eq!(
        terminal.attachment_state(),
        AttachmentState::ContextDestroyed
    );
    assert_invalid_state(terminal.result_summary());
    assert_invalid_state(terminal.take_terminal_summary());
    let mut destroyed_probe = context();
    let destroyed_frame = destroyed_probe.begin_frame();
    destroyed_frame
        .ui()
        .text("context-destroyed driver state probe");
    assert_frame_driver_invalid_state(&mut terminal, destroyed_frame);
    drop(destroyed_probe);
    terminal.shutdown().expect("ContextDestroyed shutdown");
}

#[test]
fn process_binding_rejects_a_second_suspended_context_until_unbind() {
    let _guard = test_lock();
    let mut context_a = context();
    let mut engine_a = TestEngine::create().expect("engine A");
    engine_a.start(&mut context_a).expect("start A");
    let suspended_a = context_a.suspend_or_panic();

    let mut context_b = SuspendedContext::create().activate().expect("activate B");
    let mut engine_b = TestEngine::create().expect("engine B");
    let occupied = engine_b
        .start(&mut context_b)
        .expect_err("the process lease must reject B");
    assert_eq!(occupied.status(), Some(TestEngineStatus::BindingOccupied));

    let suspended_b = context_b.suspend_or_panic();
    let context_a = suspended_a.activate().expect("reactivate A");
    engine_a.stop().expect("stop A");
    let suspended_a = context_a.suspend_or_panic();
    let mut context_b = suspended_b.activate().expect("reactivate B");
    let occupied = engine_b
        .start(&mut context_b)
        .expect_err("stop must retain the process lease");
    assert_eq!(occupied.status(), Some(TestEngineStatus::BindingOccupied));

    let suspended_b = context_b.suspend_or_panic();
    let context_a = suspended_a.activate().expect("activate A for shutdown");
    engine_a.shutdown().expect("unbind A");
    let suspended_a = context_a.suspend_or_panic();
    let mut context_b = suspended_b.activate().expect("activate B after unbind");
    engine_b.start(&mut context_b).expect("start B");
    engine_b.shutdown().expect("shutdown B");
    drop(context_b);
    drop(suspended_a);
}

#[test]
fn suspended_context_destruction_releases_the_process_binding() {
    let _guard = test_lock();
    let mut context_a = context();
    let mut engine_a = TestEngine::create().expect("engine A");
    engine_a.start(&mut context_a).expect("start A");
    let suspended_a = context_a.suspend_or_panic();

    let mut context_b = SuspendedContext::create().activate().expect("activate B");
    let mut engine_b = TestEngine::create().expect("engine B");
    let occupied = engine_b
        .start(&mut context_b)
        .expect_err("A still owns the process binding");
    assert_eq!(occupied.status(), Some(TestEngineStatus::BindingOccupied));

    drop(suspended_a);
    assert_eq!(
        engine_a.attachment_state(),
        AttachmentState::ContextDestroyed
    );
    engine_b
        .start(&mut context_b)
        .expect("Context-first teardown released the binding");
    engine_b.shutdown().expect("shutdown B");
    engine_a.shutdown().expect("destroy detached A");
    drop(context_b);
}

#[test]
fn built_in_suite_registration_validates_manifests_and_per_test_results() {
    let _guard = test_lock();
    let mut context = context();
    let mut engine = TestEngine::create().expect("engine");
    engine.start(&mut context).expect("start engine");

    inject(raw::ImGuiTestEngineExceptionPoint_SuiteRegistrationAfterFirstTest);
    let interrupted = engine.register_builtin_test_suite(BuiltInTestSuite::NativeDefaults);
    assert!(matches!(
        interrupted,
        Err(TestEngineError::Ffi {
            status: TestEngineStatus::Exception,
            ..
        })
    ));
    let defaults = engine
        .register_builtin_test_suite(BuiltInTestSuite::NativeDefaults)
        .expect("register native defaults");
    assert_eq!(defaults.test_count(), 2);
    assert_eq!(
        defaults
            .test_names()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        BuiltInTestSuite::NativeDefaults.expected_test_names()
    );
    engine
        .queue_tests(
            TestGroup::Tests,
            Some("no-such-default-test"),
            RunFlags::NONE,
        )
        .expect("queue no-match run");
    let unexecuted = engine.take_terminal_test_suite_result(&defaults);
    assert!(matches!(
        unexecuted,
        Err(TestEngineError::UnexpectedTestSuiteResult {
            exact_manifest: false,
            ..
        })
    ));

    let docking = engine
        .register_builtin_test_suite(BuiltInTestSuite::UpstreamDocking)
        .expect("register upstream docking suite");
    assert_eq!(docking.test_count(), 39);
    assert_eq!(
        docking.test_names().first().map(String::as_str),
        Some("docking_move_does_not_dock")
    );
    assert_eq!(
        docking.test_names().last().map(String::as_str),
        Some("docking_settings_invalid_1")
    );

    let duplicate = engine.register_builtin_test_suite(BuiltInTestSuite::NativeDefaults);
    assert!(matches!(
        duplicate,
        Err(TestEngineError::Ffi {
            status: TestEngineStatus::InvalidState,
            ..
        })
    ));

    let viewport = engine.register_builtin_test_suite(BuiltInTestSuite::UpstreamViewports);
    assert!(matches!(
        viewport,
        Err(TestEngineError::Ffi {
            status: TestEngineStatus::Unsupported,
            ..
        })
    ));

    engine.shutdown().expect("shutdown engine");
    drop(context);
}

#[test]
fn stale_suite_tokens_cannot_cross_engine_generations() {
    let _guard = test_lock();
    let mut first_context = context();
    let mut first_engine = TestEngine::create().expect("first engine");
    first_engine
        .start(&mut first_context)
        .expect("start first engine");
    let stale_suite = first_engine
        .register_builtin_test_suite(BuiltInTestSuite::NativeDefaults)
        .expect("register first suite");
    let first_id = first_engine.id();
    first_engine.shutdown().expect("shutdown first engine");
    drop(first_context);

    let mut second_context = context();
    let mut second_engine = TestEngine::create().expect("second engine");
    assert_ne!(first_id, second_engine.id());
    second_engine
        .start(&mut second_context)
        .expect("start second engine");
    second_engine
        .register_builtin_test_suite(BuiltInTestSuite::NativeDefaults)
        .expect("register second suite");
    second_engine
        .queue_tests(TestGroup::Tests, Some("no-such-test"), RunFlags::NONE)
        .expect("queue no-match run");
    assert!(matches!(
        second_engine.take_terminal_test_suite_result(&stale_suite),
        Err(TestEngineError::InvalidInput { .. })
    ));
    second_engine
        .take_terminal_summary()
        .expect("consume second run")
        .expect("terminal second run");
    second_engine.shutdown().expect("shutdown second engine");
    drop(second_context);
}

#[test]
fn suite_validation_requires_the_exact_run_manifest() {
    let _guard = test_lock();
    let mut context = context();
    let mut engine = TestEngine::create().expect("engine");
    engine.start(&mut context).expect("start engine");
    let suite = engine
        .register_builtin_test_suite(BuiltInTestSuite::NativeDefaults)
        .expect("register defaults");

    let full = TestRunner::new(&mut engine)
        .run_headless(&mut context, |_, _| {
            Ok::<_, Infallible>(dear_imgui_test_engine::RunnerControl::Continue)
        })
        .expect("run full suite");
    engine
        .validate_registered_test_suite(&suite, &full)
        .expect("validate exact full run");

    let stale_full = TestRunner::new(&mut engine)
        .run_headless(&mut context, |_, _| {
            Ok::<_, Infallible>(dear_imgui_test_engine::RunnerControl::Continue)
        })
        .expect("run full suite before a newer generation");
    let partial = TestRunner::new(&mut engine)
        .filter("basic_interaction")
        .run_headless(&mut context, |_, _| {
            Ok::<_, Infallible>(dear_imgui_test_engine::RunnerControl::Continue)
        })
        .expect("run one suite member");
    assert_eq!(partial.summary().count_tested, 1);
    assert!(matches!(
        engine.validate_registered_test_suite(&suite, &stale_full),
        Err(TestEngineError::InvalidInput { .. })
    ));
    assert!(matches!(
        engine.validate_registered_test_suite(&suite, &partial),
        Err(TestEngineError::UnexpectedTestSuiteResult {
            exact_manifest: false,
            ..
        })
    ));

    engine.shutdown().expect("shutdown engine");
    drop(context);
}

#[test]
fn missing_table_lookup_is_a_failed_test_not_an_infrastructure_error() {
    let _guard = test_lock();
    reset_counters();
    let mut context = context();
    let mut engine = TestEngine::create().expect("engine");
    engine.start(&mut context).expect("start");
    engine
        .add_script_test("table", "missing", |script| {
            script.set_ref("Table Host")?;
            script.table_resize_column_by_label("Missing Table", "Column", 100.0)
        })
        .expect("table lookup command is valid infrastructure");
    engine
        .queue_tests(TestGroup::Tests, None, RunFlags::NONE)
        .expect("queue missing-table test");

    for _ in 0..64 {
        let frame = context.begin_frame();
        let ui = frame.ui();
        ui.window("Table Host")
            .build(|| ui.text("No table is intentionally created"));
        engine
            .drive_frame(frame, 0, &mut VirtualFrameDriver)
            .expect("virtual frame remains infrastructure-successful");
        if engine.run_state() == RunState::Terminal {
            break;
        }
    }
    assert_eq!(engine.run_state(), RunState::Terminal);
    let summary = engine
        .take_terminal_summary()
        .expect("terminal summary is infrastructure-successful")
        .expect("terminal report");
    assert_eq!(summary.count_tested, 1);
    assert_eq!(summary.count_success, 0);

    engine.shutdown().expect("shutdown");
    drop(context);
}

#[test]
fn table_resize_by_label_succeeds_for_a_resizable_table() {
    let _guard = test_lock();
    reset_counters();
    let mut context = context();
    context.io_mut().set_display_size([640.0, 480.0]);
    let mut engine = TestEngine::create().expect("engine");
    engine.start(&mut context).expect("start");
    engine
        .add_script_test("table", "resize_by_label", |script| {
            script.set_ref("Table Host")?;
            script.yield_frames(ScriptCount::new(2)?)?;
            script.table_resize_column_by_label("Resize Table", "Name", 96.0)?;
            script.yield_frames(ScriptCount::new(2)?)
        })
        .expect("table resize script registration");
    engine
        .queue_tests(TestGroup::Tests, Some("resize_by_label"), RunFlags::NONE)
        .expect("queue table resize test");

    for _ in 0..64 {
        let frame = context.begin_frame();
        let ui = frame.ui();
        ui.window("Table Host")
            .size([420.0, 240.0], Condition::Always)
            .build(|| {
                ui.table("Resize Table")
                    .flags(TableFlags::RESIZABLE | TableFlags::BORDERS)
                    .column("Name")
                    .width(64.0)
                    .done()
                    .column("Value")
                    .width(64.0)
                    .done()
                    .headers(true)
                    .build(|ui| {
                        ui.table_next_row();
                        ui.table_next_column();
                        ui.text("row");
                        ui.table_next_column();
                        ui.text("value");
                    });
            });
        engine
            .drive_frame(frame, 0, &mut VirtualFrameDriver)
            .expect("virtual frame");
        if engine.run_state() == RunState::Terminal {
            break;
        }
    }

    assert_eq!(engine.run_state(), RunState::Terminal);
    let summary = engine
        .take_terminal_summary()
        .expect("terminal summary")
        .expect("terminal report");
    assert_eq!(summary.count_tested, 1);
    assert_eq!(summary.count_success, 1);

    engine.shutdown().expect("shutdown");
    drop(context);
}

#[test]
fn drop_retries_one_shot_native_failures_without_losing_the_engine() {
    let _guard = test_lock();
    reset_counters();

    let mut attached_context = context();
    let mut attached_engine = TestEngine::create().expect("attached engine");
    attached_engine
        .start(&mut attached_context)
        .expect("attached start");
    inject(raw::ImGuiTestEngineExceptionPoint_UpstreamCall);
    drop(attached_engine);
    drop(attached_context);
    let attached = counters();
    assert_eq!(attached.EnginesCreated, 1);
    assert_eq!(attached.EnginesStopped, 1);
    assert_eq!(attached.EnginesUnbound, 1);
    assert_eq!(attached.EnginesDestroyed, 1);

    reset_counters();
    let mut context_first = context();
    let mut detached_engine = TestEngine::create().expect("context-first engine");
    detached_engine
        .start(&mut context_first)
        .expect("context-first start");
    drop(context_first);
    inject(raw::ImGuiTestEngineExceptionPoint_UpstreamCall);
    drop(detached_engine);
    let detached = counters();
    assert_eq!(detached.EnginesCreated, 1);
    assert_eq!(detached.EnginesStopped, 1);
    assert_eq!(detached.EnginesUnbound, 1);
    assert_eq!(detached.EnginesDestroyed, 1);
}
