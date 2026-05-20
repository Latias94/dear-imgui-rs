use dear_imgui_rs::{
    Context, ContextAliveToken, ImGuiError, ImGuiResult, Ui, with_scratch_txt, with_scratch_txt_two,
};
use dear_imgui_test_engine_sys as sys;
use std::{marker::PhantomData, rc::Rc};

use crate::{ResultSummary, RunFlags, RunSpeed, Script, ScriptTest, TestGroup, VerboseLevel};

/// Dear ImGui Test Engine context.
///
/// The upstream engine is not thread-safe; create and use it on the same thread as the target ImGui context.
pub struct TestEngine {
    pub(super) raw: *mut sys::ImGuiTestEngine,
    pub(super) bound_imgui_ctx_raw: Option<*mut dear_imgui_rs::sys::ImGuiContext>,
    pub(super) bound_imgui_alive: Option<ContextAliveToken>,
    pub(super) _not_send_sync: PhantomData<Rc<()>>,
}

impl TestEngine {
    fn assert_bound_imgui_alive(&self, caller: &str) {
        if let Some(alive) = &self.bound_imgui_alive {
            assert!(
                alive.is_alive(),
                "{caller} called after the bound dear_imgui_rs::Context was dropped"
            );
        }
    }

    /// Creates a new test engine context.
    pub fn try_create() -> ImGuiResult<Self> {
        let raw = unsafe { sys::imgui_test_engine_create_context() };
        if raw.is_null() {
            return Err(ImGuiError::context_creation(
                "imgui_test_engine_create_context returned null",
            ));
        }
        Ok(Self {
            raw,
            bound_imgui_ctx_raw: None,
            bound_imgui_alive: None,
            _not_send_sync: PhantomData,
        })
    }

    /// Creates a new test engine context.
    ///
    /// # Panics
    /// Panics if the underlying context creation fails.
    pub fn create() -> Self {
        Self::try_create().expect("Failed to create Dear ImGui Test Engine context")
    }

    pub fn as_raw(&self) -> *mut sys::ImGuiTestEngine {
        self.raw
    }

    pub fn is_bound(&self) -> bool {
        self.assert_bound_imgui_alive("TestEngine::is_bound()");
        unsafe { sys::imgui_test_engine_is_bound(self.raw) }
    }

    pub fn is_started(&self) -> bool {
        self.assert_bound_imgui_alive("TestEngine::is_started()");
        unsafe { sys::imgui_test_engine_is_started(self.raw) }
    }

    fn ui_context_target(&self) -> *mut dear_imgui_rs::sys::ImGuiContext {
        unsafe { sys::imgui_test_engine_get_ui_context_target(self.raw) }
    }

    /// Tries to start (bind) the test engine to an ImGui context.
    ///
    /// Calling this multiple times with the same context is a no-op.
    ///
    /// Returns an error if the engine is already bound to a different context, or if the engine
    /// was previously stopped but is still bound (call `shutdown()` to detach first).
    pub fn try_start(&mut self, imgui_ctx: &Context) -> ImGuiResult<()> {
        self.assert_bound_imgui_alive("TestEngine::try_start()");
        let ctx = imgui_ctx.as_raw();
        if ctx.is_null() {
            return Err(ImGuiError::invalid_operation(
                "TestEngine::try_start() called with a null ImGui context",
            ));
        }

        let bound = self.ui_context_target();
        if !bound.is_null() {
            if bound == ctx {
                if self.is_started() {
                    return Ok(());
                }
                return Err(ImGuiError::invalid_operation(
                    "TestEngine::try_start() called but the engine is already bound (and not started). \
                     Call TestEngine::shutdown() to detach, then start again.",
                ));
            }
            return Err(ImGuiError::invalid_operation(
                "TestEngine::try_start() called while already bound to a different ImGui context",
            ));
        }

        unsafe { sys::imgui_test_engine_start(self.raw, ctx) };
        self.bound_imgui_ctx_raw = Some(ctx);
        self.bound_imgui_alive = Some(imgui_ctx.alive_token());
        Ok(())
    }

    /// Starts (binds) the test engine to an ImGui context.
    ///
    /// Calling this multiple times with the same context is a no-op.
    ///
    /// # Panics
    /// Panics if called while already started with a different ImGui context.
    pub fn start(&mut self, imgui_ctx: &Context) {
        self.try_start(imgui_ctx)
            .expect("Failed to start Dear ImGui Test Engine context");
    }

    /// Stops the test coroutine and exports results, but keeps the engine bound to the ImGui context.
    pub fn stop(&mut self) {
        self.assert_bound_imgui_alive("TestEngine::stop()");
        unsafe { sys::imgui_test_engine_stop(self.raw) };
    }

    /// Stops (if needed) and detaches the engine from the bound ImGui context.
    ///
    /// This is the most ergonomic shutdown path for Rust applications: it avoids relying on drop order
    /// between `Context` and `TestEngine`.
    pub fn shutdown(&mut self) {
        self.assert_bound_imgui_alive("TestEngine::shutdown()");
        unsafe { sys::imgui_test_engine_unbind(self.raw) };
        self.bound_imgui_ctx_raw = None;
        self.bound_imgui_alive = None;
    }

    pub fn post_swap(&mut self) {
        self.assert_bound_imgui_alive("TestEngine::post_swap()");
        unsafe { sys::imgui_test_engine_post_swap(self.raw) };
    }

    pub fn show_windows(&mut self, _ui: &Ui, opened: Option<&mut bool>) {
        self.assert_bound_imgui_alive("TestEngine::show_windows()");
        let ptr = opened.map_or(std::ptr::null_mut(), |b| b as *mut bool);
        unsafe { sys::imgui_test_engine_show_windows(self.raw, ptr) };
    }

    /// Registers a small set of built-in demo tests (useful to validate integration).
    pub fn register_default_tests(&mut self) {
        self.assert_bound_imgui_alive("TestEngine::register_default_tests()");
        unsafe { sys::imgui_test_engine_register_default_tests(self.raw) };
    }

    /// Registers a script-driven test.
    ///
    /// Script tests do not provide a GUI function: they are meant to drive your application's existing UI.
    pub fn add_script_test<F>(&mut self, category: &str, name: &str, build: F) -> ImGuiResult<()>
    where
        F: FnOnce(&mut ScriptTest<'_>) -> ImGuiResult<()>,
    {
        self.assert_bound_imgui_alive("TestEngine::add_script_test()");
        if category.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "add_script_test category contained interior NUL",
            ));
        }
        if name.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "add_script_test name contained interior NUL",
            ));
        }

        let mut script = Script::create()?;
        build(&mut ScriptTest {
            script: &mut script,
        })?;
        let script_raw = script.into_raw();

        with_scratch_txt_two(category, name, |cat_ptr, name_ptr| unsafe {
            sys::imgui_test_engine_register_script_test(self.raw, cat_ptr, name_ptr, script_raw)
        });

        Ok(())
    }

    pub fn queue_tests(
        &mut self,
        group: TestGroup,
        filter: Option<&str>,
        run_flags: RunFlags,
    ) -> ImGuiResult<()> {
        self.assert_bound_imgui_alive("TestEngine::queue_tests()");
        if let Some(filter) = filter {
            if filter.contains('\0') {
                return Err(ImGuiError::invalid_operation(
                    "queue_tests filter contained interior NUL",
                ));
            }
            with_scratch_txt(filter, |ptr| unsafe {
                sys::imgui_test_engine_queue_tests(
                    self.raw,
                    group as sys::ImGuiTestEngineGroup,
                    ptr,
                    run_flags.bits() as i32,
                )
            });
            return Ok(());
        }

        unsafe {
            sys::imgui_test_engine_queue_tests(
                self.raw,
                group as sys::ImGuiTestEngineGroup,
                std::ptr::null(),
                run_flags.bits() as i32,
            )
        };
        Ok(())
    }

    pub fn queue_all_tests(&mut self) {
        let _ = self.queue_tests(TestGroup::Tests, None, RunFlags::NONE);
    }

    /// Returns a best-effort snapshot of test results.
    ///
    /// Note: upstream asserts if queried while a test is running; our sys shim
    /// intentionally avoids aborting and will count `Running` tests as "remaining".
    pub fn result_summary(&self) -> ResultSummary {
        self.assert_bound_imgui_alive("TestEngine::result_summary()");
        let mut raw = sys::ImGuiTestEngineResultSummary_c {
            CountTested: 0,
            CountSuccess: 0,
            CountInQueue: 0,
        };
        unsafe { sys::imgui_test_engine_get_result_summary(self.raw, &mut raw) };
        ResultSummary::from_raw(raw.CountTested, raw.CountSuccess, raw.CountInQueue)
    }

    pub fn is_test_queue_empty(&self) -> bool {
        self.assert_bound_imgui_alive("TestEngine::is_test_queue_empty()");
        unsafe { sys::imgui_test_engine_is_test_queue_empty(self.raw) }
    }

    pub fn is_running_tests(&self) -> bool {
        self.assert_bound_imgui_alive("TestEngine::is_running_tests()");
        unsafe { sys::imgui_test_engine_is_running_tests(self.raw) }
    }

    pub fn is_requesting_max_app_speed(&self) -> bool {
        self.assert_bound_imgui_alive("TestEngine::is_requesting_max_app_speed()");
        unsafe { sys::imgui_test_engine_is_requesting_max_app_speed(self.raw) }
    }

    pub fn try_abort_engine(&mut self) -> bool {
        self.assert_bound_imgui_alive("TestEngine::try_abort_engine()");
        unsafe { sys::imgui_test_engine_try_abort_engine(self.raw) }
    }

    pub fn abort_current_test(&mut self) {
        self.assert_bound_imgui_alive("TestEngine::abort_current_test()");
        unsafe { sys::imgui_test_engine_abort_current_test(self.raw) };
    }

    pub fn set_run_speed(&mut self, speed: RunSpeed) {
        self.assert_bound_imgui_alive("TestEngine::set_run_speed()");
        unsafe {
            sys::imgui_test_engine_set_run_speed(self.raw, speed as sys::ImGuiTestEngineRunSpeed)
        };
    }

    pub fn set_verbose_level(&mut self, level: VerboseLevel) {
        self.assert_bound_imgui_alive("TestEngine::set_verbose_level()");
        unsafe {
            sys::imgui_test_engine_set_verbose_level(
                self.raw,
                level as sys::ImGuiTestEngineVerboseLevel,
            )
        };
    }

    pub fn set_capture_enabled(&mut self, enabled: bool) {
        self.assert_bound_imgui_alive("TestEngine::set_capture_enabled()");
        unsafe { sys::imgui_test_engine_set_capture_enabled(self.raw, enabled) };
    }

    pub fn install_default_crash_handler() {
        unsafe { sys::imgui_test_engine_install_default_crash_handler() };
    }
}

impl Drop for TestEngine {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            if let Some(alive) = &self.bound_imgui_alive {
                if !alive.is_alive() {
                    // Avoid calling into ImGui allocators after the context has been dropped.
                    // Best-effort: leak the test engine context instead of risking UB.
                    return;
                }
            }
            if self.ui_context_target().is_null() {
                self.bound_imgui_ctx_raw = None;
                self.bound_imgui_alive = None;
            }
            if !self.ui_context_target().is_null() {
                if let Some(bound) = self.bound_imgui_ctx_raw {
                    unsafe {
                        let prev = dear_imgui_rs::sys::igGetCurrentContext();
                        dear_imgui_rs::sys::igSetCurrentContext(bound);
                        sys::imgui_test_engine_unbind(self.raw);
                        dear_imgui_rs::sys::igSetCurrentContext(prev);
                    }
                } else {
                    unsafe { sys::imgui_test_engine_unbind(self.raw) };
                }
            }
            unsafe { sys::imgui_test_engine_destroy_context(self.raw) };
            self.raw = std::ptr::null_mut();
        }
    }
}
