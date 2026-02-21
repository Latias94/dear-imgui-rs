//! Dear ImGui Test Engine bindings for `dear-imgui-rs`.
//!
//! This crate wraps `dear-imgui-test-engine-sys` with a small safe API for
//! engine lifetime management and per-frame UI integration.

use bitflags::bitflags;
use dear_imgui_rs::{
    Context, ImGuiError, ImGuiResult, KeyChord, Ui, with_scratch_txt, with_scratch_txt_two,
};
use dear_imgui_test_engine_sys as sys;
use std::{marker::PhantomData, rc::Rc};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ResultSummary {
    pub count_tested: i32,
    pub count_success: i32,
    pub count_in_queue: i32,
}

/// Dear ImGui Test Engine context.
///
/// The upstream engine is not thread-safe; create and use it on the same thread as the target ImGui context.
pub struct TestEngine {
    raw: *mut sys::ImGuiTestEngine,
    _not_send_sync: PhantomData<Rc<()>>,
}

struct Script {
    raw: *mut sys::ImGuiTestEngineScript,
}

impl Script {
    fn create() -> ImGuiResult<Self> {
        let raw = unsafe { sys::imgui_test_engine_script_create() };
        if raw.is_null() {
            return Err(ImGuiError::invalid_operation(
                "imgui_test_engine_script_create returned null",
            ));
        }
        Ok(Self { raw })
    }

    fn into_raw(mut self) -> *mut sys::ImGuiTestEngineScript {
        let raw = self.raw;
        self.raw = std::ptr::null_mut();
        raw
    }
}

impl Drop for Script {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe { sys::imgui_test_engine_script_destroy(self.raw) };
            self.raw = std::ptr::null_mut();
        }
    }
}

pub struct ScriptTest<'a> {
    script: &'a mut Script,
}

impl ScriptTest<'_> {
    pub fn set_ref(&mut self, r#ref: &str) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "set_ref contained interior NUL",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_set_ref(self.script.raw, ptr)
        });
        Ok(())
    }

    pub fn item_click(&mut self, r#ref: &str) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "item_click contained interior NUL",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_item_click(self.script.raw, ptr)
        });
        Ok(())
    }

    pub fn item_double_click(&mut self, r#ref: &str) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "item_double_click contained interior NUL",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_item_double_click(self.script.raw, ptr)
        });
        Ok(())
    }

    pub fn item_open(&mut self, r#ref: &str) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "item_open contained interior NUL",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_item_open(self.script.raw, ptr)
        });
        Ok(())
    }

    pub fn item_close(&mut self, r#ref: &str) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "item_close contained interior NUL",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_item_close(self.script.raw, ptr)
        });
        Ok(())
    }

    pub fn item_set_opened(&mut self, r#ref: &str, opened: bool) -> ImGuiResult<()> {
        if opened {
            self.item_open(r#ref)
        } else {
            self.item_close(r#ref)
        }
    }

    pub fn item_check(&mut self, r#ref: &str) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "item_check contained interior NUL",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_item_check(self.script.raw, ptr)
        });
        Ok(())
    }

    pub fn item_set_checked(&mut self, r#ref: &str, checked: bool) -> ImGuiResult<()> {
        if checked {
            self.item_check(r#ref)
        } else {
            self.item_uncheck(r#ref)
        }
    }

    pub fn item_uncheck(&mut self, r#ref: &str) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "item_uncheck contained interior NUL",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_item_uncheck(self.script.raw, ptr)
        });
        Ok(())
    }

    pub fn item_input_int(&mut self, r#ref: &str, v: i32) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "item_input_int contained interior NUL",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_item_input_int(self.script.raw, ptr, v)
        });
        Ok(())
    }

    pub fn item_input_str(&mut self, r#ref: &str, v: &str) -> ImGuiResult<()> {
        if r#ref.contains('\0') || v.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "item_input_str contained interior NUL",
            ));
        }
        with_scratch_txt_two(r#ref, v, |ref_ptr, v_ptr| unsafe {
            sys::imgui_test_engine_script_item_input_str(self.script.raw, ref_ptr, v_ptr)
        });
        Ok(())
    }

    pub fn mouse_move(&mut self, r#ref: &str) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "mouse_move contained interior NUL",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_mouse_move(self.script.raw, ptr)
        });
        Ok(())
    }

    pub fn mouse_move_to_void(&mut self) {
        unsafe { sys::imgui_test_engine_script_mouse_move_to_void(self.script.raw) };
    }

    pub fn mouse_click_on_void(&mut self, button: i32, count: i32) -> ImGuiResult<()> {
        if button < 0 {
            return Err(ImGuiError::invalid_operation(
                "mouse_click_on_void button must be >= 0",
            ));
        }
        if count < 1 {
            return Err(ImGuiError::invalid_operation(
                "mouse_click_on_void count must be >= 1",
            ));
        }
        unsafe {
            sys::imgui_test_engine_script_mouse_click_on_void(self.script.raw, button, count)
        };
        Ok(())
    }

    pub fn mouse_wheel(&mut self, dx: f32, dy: f32) -> ImGuiResult<()> {
        if !dx.is_finite() || !dy.is_finite() {
            return Err(ImGuiError::invalid_operation(
                "mouse_wheel requires finite values",
            ));
        }
        unsafe { sys::imgui_test_engine_script_mouse_wheel(self.script.raw, dx, dy) };
        Ok(())
    }

    pub fn key_down(&mut self, key_chord: KeyChord) {
        unsafe { sys::imgui_test_engine_script_key_down(self.script.raw, key_chord.raw()) };
    }

    pub fn key_up(&mut self, key_chord: KeyChord) {
        unsafe { sys::imgui_test_engine_script_key_up(self.script.raw, key_chord.raw()) };
    }

    pub fn key_press(&mut self, key_chord: KeyChord, count: i32) -> ImGuiResult<()> {
        if count < 1 {
            return Err(ImGuiError::invalid_operation(
                "key_press count must be >= 1",
            ));
        }
        unsafe { sys::imgui_test_engine_script_key_press(self.script.raw, key_chord.raw(), count) };
        Ok(())
    }

    pub fn key_hold(&mut self, key_chord: KeyChord, seconds: f32) -> ImGuiResult<()> {
        if !seconds.is_finite() || seconds < 0.0 {
            return Err(ImGuiError::invalid_operation(
                "key_hold requires a finite non-negative value",
            ));
        }
        unsafe {
            sys::imgui_test_engine_script_key_hold(self.script.raw, key_chord.raw(), seconds)
        };
        Ok(())
    }

    pub fn key_chars(&mut self, chars: &str) -> ImGuiResult<()> {
        if chars.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "key_chars contained interior NUL",
            ));
        }
        with_scratch_txt(chars, |ptr| unsafe {
            sys::imgui_test_engine_script_key_chars(self.script.raw, ptr)
        });
        Ok(())
    }

    pub fn key_chars_append(&mut self, chars: &str) -> ImGuiResult<()> {
        if chars.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "key_chars_append contained interior NUL",
            ));
        }
        with_scratch_txt(chars, |ptr| unsafe {
            sys::imgui_test_engine_script_key_chars_append(self.script.raw, ptr)
        });
        Ok(())
    }

    pub fn key_chars_append_enter(&mut self, chars: &str) -> ImGuiResult<()> {
        if chars.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "key_chars_append_enter contained interior NUL",
            ));
        }
        with_scratch_txt(chars, |ptr| unsafe {
            sys::imgui_test_engine_script_key_chars_append_enter(self.script.raw, ptr)
        });
        Ok(())
    }

    pub fn key_chars_replace(&mut self, chars: &str) -> ImGuiResult<()> {
        if chars.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "key_chars_replace contained interior NUL",
            ));
        }
        with_scratch_txt(chars, |ptr| unsafe {
            sys::imgui_test_engine_script_key_chars_replace(self.script.raw, ptr)
        });
        Ok(())
    }

    pub fn key_chars_replace_enter(&mut self, chars: &str) -> ImGuiResult<()> {
        if chars.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "key_chars_replace_enter contained interior NUL",
            ));
        }
        with_scratch_txt(chars, |ptr| unsafe {
            sys::imgui_test_engine_script_key_chars_replace_enter(self.script.raw, ptr)
        });
        Ok(())
    }

    pub fn item_hold(&mut self, r#ref: &str, seconds: f32) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "item_hold contained interior NUL",
            ));
        }
        if !seconds.is_finite() || seconds < 0.0 {
            return Err(ImGuiError::invalid_operation(
                "item_hold requires a finite non-negative value",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_item_hold(self.script.raw, ptr, seconds)
        });
        Ok(())
    }

    pub fn item_hold_for_frames(&mut self, r#ref: &str, frames: i32) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "item_hold_for_frames contained interior NUL",
            ));
        }
        if frames < 1 {
            return Err(ImGuiError::invalid_operation(
                "item_hold_for_frames frames must be >= 1",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_item_hold_for_frames(self.script.raw, ptr, frames)
        });
        Ok(())
    }

    pub fn item_drag_with_delta(&mut self, r#ref: &str, dx: f32, dy: f32) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "item_drag_with_delta contained interior NUL",
            ));
        }
        if !dx.is_finite() || !dy.is_finite() {
            return Err(ImGuiError::invalid_operation(
                "item_drag_with_delta requires finite values",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_item_drag_with_delta(self.script.raw, ptr, dx, dy)
        });
        Ok(())
    }

    pub fn scroll_to_item_x(&mut self, r#ref: &str) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "scroll_to_item_x contained interior NUL",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_scroll_to_item_x(self.script.raw, ptr)
        });
        Ok(())
    }

    pub fn scroll_to_item_y(&mut self, r#ref: &str) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "scroll_to_item_y contained interior NUL",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_scroll_to_item_y(self.script.raw, ptr)
        });
        Ok(())
    }

    pub fn scroll_to_top(&mut self, r#ref: &str) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "scroll_to_top contained interior NUL",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_scroll_to_top(self.script.raw, ptr)
        });
        Ok(())
    }

    pub fn scroll_to_bottom(&mut self, r#ref: &str) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "scroll_to_bottom contained interior NUL",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_scroll_to_bottom(self.script.raw, ptr)
        });
        Ok(())
    }

    pub fn tab_close(&mut self, r#ref: &str) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "tab_close contained interior NUL",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_tab_close(self.script.raw, ptr)
        });
        Ok(())
    }

    pub fn combo_click(&mut self, r#ref: &str) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "combo_click contained interior NUL",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_combo_click(self.script.raw, ptr)
        });
        Ok(())
    }

    pub fn combo_click_all(&mut self, r#ref: &str) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "combo_click_all contained interior NUL",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_combo_click_all(self.script.raw, ptr)
        });
        Ok(())
    }

    pub fn menu_click(&mut self, r#ref: &str) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "menu_click contained interior NUL",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_menu_click(self.script.raw, ptr)
        });
        Ok(())
    }

    pub fn menu_check(&mut self, r#ref: &str) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "menu_check contained interior NUL",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_menu_check(self.script.raw, ptr)
        });
        Ok(())
    }

    pub fn menu_uncheck(&mut self, r#ref: &str) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "menu_uncheck contained interior NUL",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_menu_uncheck(self.script.raw, ptr)
        });
        Ok(())
    }

    pub fn sleep_seconds(&mut self, seconds: f32) -> ImGuiResult<()> {
        if !seconds.is_finite() || seconds < 0.0 {
            return Err(ImGuiError::invalid_operation(
                "sleep_seconds requires a finite non-negative value",
            ));
        }
        unsafe { sys::imgui_test_engine_script_sleep(self.script.raw, seconds) };
        Ok(())
    }

    pub fn assert_item_exists(&mut self, r#ref: &str) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "assert_item_exists contained interior NUL",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_assert_item_exists(self.script.raw, ptr)
        });
        Ok(())
    }

    pub fn assert_item_visible(&mut self, r#ref: &str) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "assert_item_visible contained interior NUL",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_assert_item_visible(self.script.raw, ptr)
        });
        Ok(())
    }

    pub fn assert_item_read_int_eq(&mut self, r#ref: &str, expected: i32) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "assert_item_read_int_eq contained interior NUL",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_assert_item_read_int_eq(self.script.raw, ptr, expected)
        });
        Ok(())
    }

    pub fn assert_item_read_str_eq(&mut self, r#ref: &str, expected: &str) -> ImGuiResult<()> {
        if r#ref.contains('\0') || expected.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "assert_item_read_str_eq contained interior NUL",
            ));
        }
        with_scratch_txt_two(r#ref, expected, |ref_ptr, expected_ptr| unsafe {
            sys::imgui_test_engine_script_assert_item_read_str_eq(
                self.script.raw,
                ref_ptr,
                expected_ptr,
            )
        });
        Ok(())
    }

    pub fn assert_item_read_float_eq(
        &mut self,
        r#ref: &str,
        expected: f32,
        epsilon: f32,
    ) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "assert_item_read_float_eq contained interior NUL",
            ));
        }
        if !expected.is_finite() || !epsilon.is_finite() || epsilon < 0.0 {
            return Err(ImGuiError::invalid_operation(
                "assert_item_read_float_eq requires finite expected and finite non-negative epsilon",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_assert_item_read_float_eq(
                self.script.raw,
                ptr,
                expected,
                epsilon,
            )
        });
        Ok(())
    }

    pub fn assert_item_checked(&mut self, r#ref: &str) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "assert_item_checked contained interior NUL",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_assert_item_checked(self.script.raw, ptr)
        });
        Ok(())
    }

    pub fn assert_item_opened(&mut self, r#ref: &str) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "assert_item_opened contained interior NUL",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_assert_item_opened(self.script.raw, ptr)
        });
        Ok(())
    }

    pub fn wait_for_item(&mut self, r#ref: &str, max_frames: i32) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "wait_for_item contained interior NUL",
            ));
        }
        if max_frames < 1 {
            return Err(ImGuiError::invalid_operation(
                "wait_for_item max_frames must be >= 1",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_wait_for_item(self.script.raw, ptr, max_frames)
        });
        Ok(())
    }

    pub fn wait_for_item_visible(&mut self, r#ref: &str, max_frames: i32) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "wait_for_item_visible contained interior NUL",
            ));
        }
        if max_frames < 1 {
            return Err(ImGuiError::invalid_operation(
                "wait_for_item_visible max_frames must be >= 1",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_wait_for_item_visible(self.script.raw, ptr, max_frames)
        });
        Ok(())
    }

    pub fn wait_for_item_checked(&mut self, r#ref: &str, max_frames: i32) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "wait_for_item_checked contained interior NUL",
            ));
        }
        if max_frames < 1 {
            return Err(ImGuiError::invalid_operation(
                "wait_for_item_checked max_frames must be >= 1",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_wait_for_item_checked(self.script.raw, ptr, max_frames)
        });
        Ok(())
    }

    pub fn wait_for_item_opened(&mut self, r#ref: &str, max_frames: i32) -> ImGuiResult<()> {
        if r#ref.contains('\0') {
            return Err(ImGuiError::invalid_operation(
                "wait_for_item_opened contained interior NUL",
            ));
        }
        if max_frames < 1 {
            return Err(ImGuiError::invalid_operation(
                "wait_for_item_opened max_frames must be >= 1",
            ));
        }
        with_scratch_txt(r#ref, |ptr| unsafe {
            sys::imgui_test_engine_script_wait_for_item_opened(self.script.raw, ptr, max_frames)
        });
        Ok(())
    }

    pub fn input_text_replace(
        &mut self,
        r#ref: &str,
        text: &str,
        submit_enter: bool,
    ) -> ImGuiResult<()> {
        self.item_click(r#ref)?;
        if submit_enter {
            self.key_chars_replace_enter(text)?;
        } else {
            self.key_chars_replace(text)?;
        }
        Ok(())
    }

    pub fn yield_frames(&mut self, frames: i32) {
        unsafe { sys::imgui_test_engine_script_yield(self.script.raw, frames) };
    }
}

impl TestEngine {
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
        unsafe { sys::imgui_test_engine_is_bound(self.raw) }
    }

    pub fn is_started(&self) -> bool {
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
        unsafe { sys::imgui_test_engine_stop(self.raw) };
    }

    /// Stops (if needed) and detaches the engine from the bound ImGui context.
    ///
    /// This is the most ergonomic shutdown path for Rust applications: it avoids relying on drop order
    /// between `Context` and `TestEngine`.
    pub fn shutdown(&mut self) {
        unsafe { sys::imgui_test_engine_unbind(self.raw) };
    }

    pub fn post_swap(&mut self) {
        unsafe { sys::imgui_test_engine_post_swap(self.raw) };
    }

    pub fn show_windows(&mut self, _ui: &Ui, opened: Option<&mut bool>) {
        let ptr = opened.map_or(std::ptr::null_mut(), |b| b as *mut bool);
        unsafe { sys::imgui_test_engine_show_windows(self.raw, ptr) };
    }

    /// Registers a small set of built-in demo tests (useful to validate integration).
    pub fn register_default_tests(&mut self) {
        unsafe { sys::imgui_test_engine_register_default_tests(self.raw) };
    }

    /// Registers a script-driven test.
    ///
    /// Script tests do not provide a GUI function: they are meant to drive your application's existing UI.
    pub fn add_script_test<F>(&mut self, category: &str, name: &str, build: F) -> ImGuiResult<()>
    where
        F: FnOnce(&mut ScriptTest<'_>) -> ImGuiResult<()>,
    {
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
            unsafe { sys::imgui_test_engine_destroy_context(self.raw) };
            self.raw = std::ptr::null_mut();
        }
    }
}
