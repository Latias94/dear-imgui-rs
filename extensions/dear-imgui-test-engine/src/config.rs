use bitflags::bitflags;
use dear_imgui_test_engine_sys as sys;

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
pub enum InputMode {
    Mouse = dear_imgui_rs::sys::ImGuiInputSource_Mouse as i32,
    Keyboard = dear_imgui_rs::sys::ImGuiInputSource_Keyboard as i32,
    Gamepad = dear_imgui_rs::sys::ImGuiInputSource_Gamepad as i32,
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
