//! Dear ImGui Test Engine bindings for `dear-imgui-rs`.
//!
//! This crate wraps `dear-imgui-test-engine-sys` with a small safe API for
//! engine lifetime management and per-frame UI integration.

use bitflags::bitflags;
use dear_imgui_rs::{Context, ImGuiError, ImGuiResult, Ui, with_scratch_txt};
use dear_imgui_test_engine_sys as sys;

pub use dear_imgui_test_engine_sys as raw;

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunSpeed {
    Fast = sys::ImGuiTestEngineRunSpeed_Fast as i32,
    Normal = sys::ImGuiTestEngineRunSpeed_Normal as i32,
    Cinematic = sys::ImGuiTestEngineRunSpeed_Cinematic as i32,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerboseLevel {
    Silent = sys::ImGuiTestEngineVerboseLevel_Silent as i32,
    Error = sys::ImGuiTestEngineVerboseLevel_Error as i32,
    Warning = sys::ImGuiTestEngineVerboseLevel_Warning as i32,
    Info = sys::ImGuiTestEngineVerboseLevel_Info as i32,
    Debug = sys::ImGuiTestEngineVerboseLevel_Debug as i32,
    Trace = sys::ImGuiTestEngineVerboseLevel_Trace as i32,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestGroup {
    Unknown = sys::ImGuiTestEngineGroup_Unknown,
    Tests = sys::ImGuiTestEngineGroup_Tests,
    Perfs = sys::ImGuiTestEngineGroup_Perfs,
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct RunFlags: u32 {
        const NONE = sys::ImGuiTestEngineRunFlags_None;
        const GUI_FUNC_DISABLE = sys::ImGuiTestEngineRunFlags_GuiFuncDisable;
        const GUI_FUNC_ONLY = sys::ImGuiTestEngineRunFlags_GuiFuncOnly;
        const NO_SUCCESS_MSG = sys::ImGuiTestEngineRunFlags_NoSuccessMsg;
        const ENABLE_RAW_INPUTS = sys::ImGuiTestEngineRunFlags_EnableRawInputs;
        const RUN_FROM_GUI = sys::ImGuiTestEngineRunFlags_RunFromGui;
        const RUN_FROM_COMMAND_LINE = sys::ImGuiTestEngineRunFlags_RunFromCommandLine;
        const NO_ERROR = sys::ImGuiTestEngineRunFlags_NoError;
        const SHARE_VARS = sys::ImGuiTestEngineRunFlags_ShareVars;
        const SHARE_TEST_CONTEXT = sys::ImGuiTestEngineRunFlags_ShareTestContext;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ResultSummary {
    pub count_tested: i32,
    pub count_success: i32,
    pub count_in_queue: i32,
}

pub struct TestEngine {
    raw: *mut sys::ImGuiTestEngine,
    started: bool,
}

impl TestEngine {
    pub fn try_create() -> ImGuiResult<Self> {
        let raw = unsafe { sys::imgui_test_engine_create_context() };
        if raw.is_null() {
            return Err(ImGuiError::context_creation(
                "imgui_test_engine_create_context returned null",
            ));
        }
        Ok(Self {
            raw,
            started: false,
        })
    }

    pub fn create() -> Self {
        Self::try_create().expect("Failed to create Dear ImGui Test Engine context")
    }

    pub fn as_raw(&self) -> *mut sys::ImGuiTestEngine {
        self.raw
    }

    pub fn start(&mut self, _imgui_ctx: &Context) {
        if self.started {
            return;
        }
        unsafe {
            sys::imgui_test_engine_start(self.raw, dear_imgui_rs::sys::igGetCurrentContext())
        };
        self.started = true;
    }

    pub fn stop(&mut self) {
        if !self.started {
            return;
        }
        unsafe { sys::imgui_test_engine_stop(self.raw) };
        self.started = false;
    }

    pub fn post_swap(&mut self) {
        unsafe { sys::imgui_test_engine_post_swap(self.raw) };
    }

    pub fn show_windows(&mut self, _ui: &Ui, opened: Option<&mut bool>) {
        let ptr = opened.map_or(std::ptr::null_mut(), |b| b as *mut bool);
        unsafe { sys::imgui_test_engine_show_windows(self.raw, ptr) };
    }

    pub fn queue_tests(
        &mut self,
        group: TestGroup,
        filter: Option<&str>,
        run_flags: RunFlags,
    ) -> ImGuiResult<()> {
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

    pub fn result_summary(&self) -> ResultSummary {
        let mut raw = sys::ImGuiTestEngineResultSummary_c {
            CountTested: 0,
            CountSuccess: 0,
            CountInQueue: 0,
        };
        unsafe { sys::imgui_test_engine_get_result_summary(self.raw, &mut raw) };
        ResultSummary {
            count_tested: raw.CountTested,
            count_success: raw.CountSuccess,
            count_in_queue: raw.CountInQueue,
        }
    }

    pub fn is_test_queue_empty(&self) -> bool {
        unsafe { sys::imgui_test_engine_is_test_queue_empty(self.raw) }
    }

    pub fn is_running_tests(&self) -> bool {
        unsafe { sys::imgui_test_engine_is_running_tests(self.raw) }
    }

    pub fn is_requesting_max_app_speed(&self) -> bool {
        unsafe { sys::imgui_test_engine_is_requesting_max_app_speed(self.raw) }
    }

    pub fn try_abort_engine(&mut self) -> bool {
        unsafe { sys::imgui_test_engine_try_abort_engine(self.raw) }
    }

    pub fn abort_current_test(&mut self) {
        unsafe { sys::imgui_test_engine_abort_current_test(self.raw) };
    }

    pub fn set_run_speed(&mut self, speed: RunSpeed) {
        unsafe {
            sys::imgui_test_engine_set_run_speed(self.raw, speed as sys::ImGuiTestEngineRunSpeed)
        };
    }

    pub fn set_verbose_level(&mut self, level: VerboseLevel) {
        unsafe {
            sys::imgui_test_engine_set_verbose_level(
                self.raw,
                level as sys::ImGuiTestEngineVerboseLevel,
            )
        };
    }

    pub fn set_capture_enabled(&mut self, enabled: bool) {
        unsafe { sys::imgui_test_engine_set_capture_enabled(self.raw, enabled) };
    }

    pub fn install_default_crash_handler() {
        unsafe { sys::imgui_test_engine_install_default_crash_handler() };
    }
}

impl Drop for TestEngine {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            if self.started {
                unsafe { sys::imgui_test_engine_stop(self.raw) };
                self.started = false;
            }
            unsafe { sys::imgui_test_engine_destroy_context(self.raw) };
            self.raw = std::ptr::null_mut();
        }
    }
}
