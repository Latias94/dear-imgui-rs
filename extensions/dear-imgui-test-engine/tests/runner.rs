use std::cell::RefCell;
use std::convert::Infallible;
use std::io;
use std::num::NonZeroU64;
use std::rc::Rc;
use std::sync::{Mutex, MutexGuard};

use dear_imgui_rs::{BackendFlags, Context, FrameLifecycleState};
use dear_imgui_test_engine::{
    RunOutcome, RunState, RunnerCallbackStage, RunnerControl, RunnerError, ScriptCount, TestEngine,
    TestEngineError, TestEngineStatus, TestRunner, raw,
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
    assert_eq!(passed.outcome, RunOutcome::Passed);
    assert_eq!(passed.summary.count_tested, 1);
    assert_eq!(passed.summary.count_success, 1);
    assert_eq!(passed.summary.count_in_queue, 0);
    assert!(passed.frames > 0);
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
        .frame_budget(nonzero(passed.frames))
        .run_headless(&mut boundary_context, no_error)
        .expect("terminal frame at the budget boundary must win over timeout");
    assert_eq!(boundary.outcome, RunOutcome::Passed);
    assert_eq!(boundary.frames, passed.frames);
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
    assert_eq!(failed.outcome, RunOutcome::Failed);
    assert_eq!(failed.summary.count_tested, 1);
    assert_eq!(failed.summary.count_success, 0);
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
    assert_eq!(no_match.outcome, RunOutcome::NoMatch);
    assert_eq!(no_match.summary.count_tested, 0);
    assert_eq!(no_match.summary.count_success, 0);
    assert_eq!(no_match.summary.count_in_queue, 0);
    assert_eq!(no_match.frames, 0);
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
    assert_eq!(timed_out.outcome, RunOutcome::TimedOut);
    assert_eq!(timed_out.summary.count_in_queue, 0);
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
    assert_eq!(aborted.outcome, RunOutcome::Aborted);
    assert_eq!(aborted.summary.count_in_queue, 0);
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
        RunnerError::Callback {
            stage: RunnerCallbackStage::ApplicationUi,
            frame: 1,
            ..
        }
    ));
    assert_ne!(
        callback_context.frame_lifecycle_state(),
        FrameLifecycleState::InFrame
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
    let render_error = TestRunner::new(&mut render_engine)
        .filter("render")
        .run_with_renderer(
            &mut render_context,
            |_, _| Ok::<_, io::Error>(RunnerControl::Continue),
            |_| Err::<(), _>(io::Error::other("injected render failure")),
        )
        .expect_err("render failure must not become an outcome");
    assert!(matches!(
        render_error,
        RunnerError::Callback {
            stage: RunnerCallbackStage::Render,
            frame: 1,
            ..
        }
    ));
    assert_ne!(
        render_context.frame_lifecycle_state(),
        FrameLifecycleState::InFrame
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
    let ui_events = events.clone();
    let render_events = events.clone();
    let report = TestRunner::new(&mut engine)
        .filter("order")
        .frame_budget(nonzero(64))
        .run_with_renderer(
            &mut context,
            move |_, frame| {
                ui_events.borrow_mut().push((frame, "ui"));
                Ok::<_, Infallible>(RunnerControl::Continue)
            },
            move |mut frame| {
                let frame_index = render_events
                    .borrow()
                    .last()
                    .expect("UI must run before render")
                    .0;
                render_events.borrow_mut().push((frame_index, "render"));
                frame
                    .reconcile_texture_feedback([])
                    .expect("empty feedback");
                Ok::<_, Infallible>(())
            },
        )
        .expect("ordered run");
    assert_eq!(report.outcome, RunOutcome::Passed);

    let events = events.borrow();
    assert_eq!(events.len() as u64, report.frames * 2);
    for (index, pair) in events.chunks_exact(2).enumerate() {
        let frame = index as u64 + 1;
        assert_eq!(pair, [(frame, "ui"), (frame, "render")]);
    }

    let mut arm_pre_swap_failure = true;
    let pre_swap_error = TestRunner::new(&mut engine)
        .filter("order")
        .frame_budget(nonzero(64))
        .run_with_renderer(&mut context, no_error, move |mut frame| {
            frame
                .reconcile_texture_feedback([])
                .expect("empty feedback");
            if std::mem::take(&mut arm_pre_swap_failure) {
                assert_eq!(
                    unsafe {
                        raw::imgui_test_engine_test_set_exception_injection(
                            raw::ImGuiTestEngineExceptionPoint_UpstreamCall,
                        )
                    },
                    raw::ImGuiTestEngineStatus_Success
                );
            }
            Ok::<_, Infallible>(())
        })
        .expect_err("pre-swap must execute after the renderer callback");
    assert!(matches!(
        pre_swap_error,
        RunnerError::TestEngine(TestEngineError::Ffi {
            operation: "imgui_test_engine_pre_swap",
            status: TestEngineStatus::Exception,
            ..
        })
    ));
    engine.shutdown().expect("order shutdown");
    drop(context);
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
            assert_eq!(report.outcome, RunOutcome::Passed);
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
        RunnerError::HeadlessTextureRequests {
            frame: 1,
            count: 1..
        }
    ));
    assert_eq!(engine.run_state(), RunState::Inactive);
    engine.shutdown().expect("texture shutdown");
    drop(context);
}
