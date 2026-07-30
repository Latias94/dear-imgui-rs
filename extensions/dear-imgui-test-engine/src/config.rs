use bitflags::bitflags;
use dear_imgui_test_engine_sys as sys;

/// Whether successful framebuffer captures are written to their configured output path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureOutput {
    Save,
    Discard,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunSpeed {
    Fast = sys::ImGuiTestEngineRunSpeed_Fast,
    Normal = sys::ImGuiTestEngineRunSpeed_Normal,
    Cinematic = sys::ImGuiTestEngineRunSpeed_Cinematic,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerboseLevel {
    Silent = sys::ImGuiTestEngineVerboseLevel_Silent,
    Error = sys::ImGuiTestEngineVerboseLevel_Error,
    Warning = sys::ImGuiTestEngineVerboseLevel_Warning,
    Info = sys::ImGuiTestEngineVerboseLevel_Info,
    Debug = sys::ImGuiTestEngineVerboseLevel_Debug,
    Trace = sys::ImGuiTestEngineVerboseLevel_Trace,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Mouse = dear_imgui_rs::sys::ImGuiInputSource_Mouse,
    Keyboard = dear_imgui_rs::sys::ImGuiInputSource_Keyboard,
    Gamepad = dear_imgui_rs::sys::ImGuiInputSource_Gamepad,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestGroup {
    Tests = sys::ImGuiTestEngineGroup_Tests,
    Perfs = sys::ImGuiTestEngineGroup_Perfs,
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct RunFlags: u32 {
        const NONE = sys::ImGuiTestEngineRunFlags_None as u32;
        const GUI_FUNC_DISABLE = sys::ImGuiTestEngineRunFlags_GuiFuncDisable as u32;
        const GUI_FUNC_ONLY = sys::ImGuiTestEngineRunFlags_GuiFuncOnly as u32;
        const NO_SUCCESS_MSG = sys::ImGuiTestEngineRunFlags_NoSuccessMsg as u32;
        const ENABLE_RAW_INPUTS = sys::ImGuiTestEngineRunFlags_EnableRawInputs as u32;
        const RUN_FROM_GUI = sys::ImGuiTestEngineRunFlags_RunFromGui as u32;
        const RUN_FROM_COMMAND_LINE = sys::ImGuiTestEngineRunFlags_RunFromCommandLine as u32;
        const NO_ERROR = sys::ImGuiTestEngineRunFlags_NoError as u32;
        const SHARE_VARS = sys::ImGuiTestEngineRunFlags_ShareVars as u32;
        const SHARE_TEST_CONTEXT = sys::ImGuiTestEngineRunFlags_ShareTestContext as u32;
    }
}

#[cfg(feature = "capture")]
bitflags! {
    /// Capture behavior supported by [`crate::ScriptTest::capture_screenshot_window`].
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CaptureFlags: u32 {
        const NONE = sys::ImGuiTestEngineCaptureFlags_None as u32;
        const STITCH_ALL = sys::ImGuiTestEngineCaptureFlags_StitchAll as u32;
        const INCLUDE_OTHER_WINDOWS =
            sys::ImGuiTestEngineCaptureFlags_IncludeOtherWindows as u32;
        const INCLUDE_POPUPS = sys::ImGuiTestEngineCaptureFlags_IncludePopups as u32;
        const HIDE_MOUSE_CURSOR = sys::ImGuiTestEngineCaptureFlags_HideMouseCursor as u32;
    }
}
