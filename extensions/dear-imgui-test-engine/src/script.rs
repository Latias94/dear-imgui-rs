use dear_imgui_rs::{
    KeyChord, KeyMods, MouseButton, TableColumnIndex, TableColumnRef, with_scratch_txt,
    with_scratch_txt_two,
};
use dear_imgui_test_engine_sys as sys;

use crate::error::ffi_status;
use crate::{InputMode, ScriptCount, ScriptLimit, TestEngineError, TestEngineResult};

const MAX_REFERENCE_BYTES: usize = 254;

pub(crate) struct Script {
    raw: *mut sys::ImGuiTestEngineScript,
}

impl Script {
    pub(crate) fn create() -> TestEngineResult<Self> {
        let mut raw = std::ptr::null_mut();
        let status = unsafe { sys::imgui_test_engine_script_create(&mut raw) };
        ffi_status("imgui_test_engine_script_create", status)?;
        if raw.is_null() {
            return Err(TestEngineError::InvalidNativeData {
                operation: "imgui_test_engine_script_create",
                detail: "successful creation returned a null script",
            });
        }
        Ok(Self { raw })
    }

    pub(crate) fn raw(&self) -> *mut sys::ImGuiTestEngineScript {
        self.raw
    }

    pub(crate) fn destroy(&mut self) -> TestEngineResult<()> {
        if self.raw.is_null() {
            return Ok(());
        }
        let status = unsafe { sys::imgui_test_engine_script_destroy(self.raw) };
        ffi_status("imgui_test_engine_script_destroy", status)?;
        self.raw = std::ptr::null_mut();
        Ok(())
    }

    pub(crate) fn disarm(&mut self) {
        self.raw = std::ptr::null_mut();
    }
}

impl Drop for Script {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

/// Builder for one Rust-owned native script.
///
/// Values are validated while the owning engine's [`dear_imgui_rs::ContextBinding`] is active.
pub struct ScriptTest<'a> {
    pub(super) script: &'a mut Script,
}

macro_rules! simple_ref_commands {
    ($($name:ident => $ffi:ident;)+) => {$(
        pub fn $name(&mut self, reference: &str) -> TestEngineResult<()> {
            validate_reference(stringify!($name), "reference", reference, true)?;
            let status = with_scratch_txt(reference, |pointer| unsafe {
                sys::$ffi(self.script.raw(), pointer)
            });
            ffi_status(stringify!($ffi), status)
        }
    )+};
}

macro_rules! no_arg_commands {
    ($($name:ident => $ffi:ident;)+) => {$(
        pub fn $name(&mut self) -> TestEngineResult<()> {
            let status = unsafe { sys::$ffi(self.script.raw()) };
            ffi_status(stringify!($ffi), status)
        }
    )+};
}

macro_rules! button_commands {
    ($($name:ident => $ffi:ident;)+) => {$(
        pub fn $name(&mut self, button: MouseButton) -> TestEngineResult<()> {
            let status = unsafe { sys::$ffi(self.script.raw(), button as i32) };
            ffi_status(stringify!($ffi), status)
        }
    )+};
}

macro_rules! char_commands {
    ($($name:ident => $ffi:ident;)+) => {$(
        pub fn $name(&mut self, chars: &str) -> TestEngineResult<()> {
            validate_c_string(stringify!($name), "chars", chars, true)?;
            let status = with_scratch_txt(chars, |pointer| unsafe {
                sys::$ffi(self.script.raw(), pointer)
            });
            ffi_status(stringify!($ffi), status)
        }
    )+};
}

macro_rules! wait_commands {
    ($($name:ident => $ffi:ident;)+) => {$(
        pub fn $name(
            &mut self,
            reference: &str,
            max_frames: ScriptCount,
        ) -> TestEngineResult<()> {
            validate_reference(stringify!($name), "reference", reference, true)?;
            let status = with_scratch_txt(reference, |pointer| unsafe {
                sys::$ffi(self.script.raw(), pointer, max_frames.raw())
            });
            ffi_status(stringify!($ffi), status)
        }
    )+};
}

impl ScriptTest<'_> {
    simple_ref_commands! {
        set_ref => imgui_test_engine_script_set_ref;
        item_click => imgui_test_engine_script_item_click;
        item_double_click => imgui_test_engine_script_item_double_click;
        item_open => imgui_test_engine_script_item_open;
        item_close => imgui_test_engine_script_item_close;
        item_check => imgui_test_engine_script_item_check;
        item_uncheck => imgui_test_engine_script_item_uncheck;
        mouse_move => imgui_test_engine_script_mouse_move;
        scroll_to_item_x => imgui_test_engine_script_scroll_to_item_x;
        scroll_to_item_y => imgui_test_engine_script_scroll_to_item_y;
        scroll_to_top => imgui_test_engine_script_scroll_to_top;
        scroll_to_bottom => imgui_test_engine_script_scroll_to_bottom;
        tab_close => imgui_test_engine_script_tab_close;
        combo_click => imgui_test_engine_script_combo_click;
        combo_click_all => imgui_test_engine_script_combo_click_all;
        menu_click => imgui_test_engine_script_menu_click;
        menu_check => imgui_test_engine_script_menu_check;
        menu_uncheck => imgui_test_engine_script_menu_uncheck;
        menu_check_all => imgui_test_engine_script_menu_check_all;
        menu_uncheck_all => imgui_test_engine_script_menu_uncheck_all;
        nav_move_to => imgui_test_engine_script_nav_move_to;
        window_close => imgui_test_engine_script_window_close;
        window_focus => imgui_test_engine_script_window_focus;
        window_bring_to_front => imgui_test_engine_script_window_bring_to_front;
        assert_item_exists => imgui_test_engine_script_assert_item_exists;
        assert_item_visible => imgui_test_engine_script_assert_item_visible;
        assert_item_checked => imgui_test_engine_script_assert_item_checked;
        assert_item_opened => imgui_test_engine_script_assert_item_opened;
    }

    no_arg_commands! {
        mouse_move_to_void => imgui_test_engine_script_mouse_move_to_void;
        nav_activate => imgui_test_engine_script_nav_activate;
        nav_input => imgui_test_engine_script_nav_input;
    }

    button_commands! {
        mouse_click => imgui_test_engine_script_mouse_click;
        mouse_double_click => imgui_test_engine_script_mouse_double_click;
        mouse_down => imgui_test_engine_script_mouse_down;
        mouse_up => imgui_test_engine_script_mouse_up;
        mouse_lift_drag_threshold => imgui_test_engine_script_mouse_lift_drag_threshold;
    }

    char_commands! {
        key_chars => imgui_test_engine_script_key_chars;
        key_chars_append => imgui_test_engine_script_key_chars_append;
        key_chars_append_enter => imgui_test_engine_script_key_chars_append_enter;
        key_chars_replace => imgui_test_engine_script_key_chars_replace;
        key_chars_replace_enter => imgui_test_engine_script_key_chars_replace_enter;
    }

    wait_commands! {
        wait_for_item => imgui_test_engine_script_wait_for_item;
        wait_for_item_visible => imgui_test_engine_script_wait_for_item_visible;
        wait_for_item_checked => imgui_test_engine_script_wait_for_item_checked;
        wait_for_item_opened => imgui_test_engine_script_wait_for_item_opened;
    }

    pub fn item_click_with_button(
        &mut self,
        reference: &str,
        button: MouseButton,
    ) -> TestEngineResult<()> {
        validate_reference("item_click_with_button", "reference", reference, true)?;
        let status = with_scratch_txt(reference, |pointer| unsafe {
            sys::imgui_test_engine_script_item_click_with_button(
                self.script.raw(),
                pointer,
                button as i32,
            )
        });
        ffi_status("imgui_test_engine_script_item_click_with_button", status)
    }

    pub fn item_set_opened(&mut self, reference: &str, opened: bool) -> TestEngineResult<()> {
        if opened {
            self.item_open(reference)
        } else {
            self.item_close(reference)
        }
    }

    pub fn item_set_checked(&mut self, reference: &str, checked: bool) -> TestEngineResult<()> {
        if checked {
            self.item_check(reference)
        } else {
            self.item_uncheck(reference)
        }
    }

    pub fn item_input_int(&mut self, reference: &str, value: i32) -> TestEngineResult<()> {
        validate_reference("item_input_int", "reference", reference, true)?;
        let status = with_scratch_txt(reference, |pointer| unsafe {
            sys::imgui_test_engine_script_item_input_int(self.script.raw(), pointer, value)
        });
        ffi_status("imgui_test_engine_script_item_input_int", status)
    }

    pub fn item_input_str(&mut self, reference: &str, value: &str) -> TestEngineResult<()> {
        validate_reference("item_input_str", "reference", reference, true)?;
        validate_c_string("item_input_str", "value", value, true)?;
        let status = with_scratch_txt_two(reference, value, |reference_ptr, value_ptr| unsafe {
            sys::imgui_test_engine_script_item_input_str(
                self.script.raw(),
                reference_ptr,
                value_ptr,
            )
        });
        ffi_status("imgui_test_engine_script_item_input_str", status)
    }

    pub fn mouse_move_to_pos(&mut self, x: f32, y: f32) -> TestEngineResult<()> {
        validate_finite_pair("mouse_move_to_pos", "position", x, y)?;
        ffi_status("imgui_test_engine_script_mouse_move_to_pos", unsafe {
            sys::imgui_test_engine_script_mouse_move_to_pos(self.script.raw(), x, y)
        })
    }

    pub fn mouse_teleport_to_pos(&mut self, x: f32, y: f32) -> TestEngineResult<()> {
        validate_finite_pair("mouse_teleport_to_pos", "position", x, y)?;
        ffi_status("imgui_test_engine_script_mouse_teleport_to_pos", unsafe {
            sys::imgui_test_engine_script_mouse_teleport_to_pos(self.script.raw(), x, y)
        })
    }

    pub fn mouse_click_multi(
        &mut self,
        button: MouseButton,
        count: ScriptCount,
    ) -> TestEngineResult<()> {
        ffi_status("imgui_test_engine_script_mouse_click_multi", unsafe {
            sys::imgui_test_engine_script_mouse_click_multi(
                self.script.raw(),
                button as i32,
                count.raw(),
            )
        })
    }

    pub fn mouse_drag_with_delta(
        &mut self,
        dx: f32,
        dy: f32,
        button: MouseButton,
    ) -> TestEngineResult<()> {
        validate_finite_pair("mouse_drag_with_delta", "delta", dx, dy)?;
        ffi_status("imgui_test_engine_script_mouse_drag_with_delta", unsafe {
            sys::imgui_test_engine_script_mouse_drag_with_delta(
                self.script.raw(),
                dx,
                dy,
                button as i32,
            )
        })
    }

    pub fn mouse_click_on_void(
        &mut self,
        button: MouseButton,
        count: ScriptCount,
    ) -> TestEngineResult<()> {
        ffi_status("imgui_test_engine_script_mouse_click_on_void", unsafe {
            sys::imgui_test_engine_script_mouse_click_on_void(
                self.script.raw(),
                button as i32,
                count.raw(),
            )
        })
    }

    pub fn mouse_wheel(&mut self, dx: f32, dy: f32) -> TestEngineResult<()> {
        validate_finite_pair("mouse_wheel", "delta", dx, dy)?;
        ffi_status("imgui_test_engine_script_mouse_wheel", unsafe {
            sys::imgui_test_engine_script_mouse_wheel(self.script.raw(), dx, dy)
        })
    }

    pub fn key_down(&mut self, key_chord: KeyChord) -> TestEngineResult<()> {
        ffi_status("imgui_test_engine_script_key_down", unsafe {
            sys::imgui_test_engine_script_key_down(self.script.raw(), key_chord.raw())
        })
    }

    pub fn key_up(&mut self, key_chord: KeyChord) -> TestEngineResult<()> {
        ffi_status("imgui_test_engine_script_key_up", unsafe {
            sys::imgui_test_engine_script_key_up(self.script.raw(), key_chord.raw())
        })
    }

    pub fn key_press(&mut self, key_chord: KeyChord, count: ScriptCount) -> TestEngineResult<()> {
        ffi_status("imgui_test_engine_script_key_press", unsafe {
            sys::imgui_test_engine_script_key_press(self.script.raw(), key_chord.raw(), count.raw())
        })
    }

    pub fn key_hold(&mut self, key_chord: KeyChord, seconds: f32) -> TestEngineResult<()> {
        validate_nonnegative("key_hold", "seconds", seconds)?;
        ffi_status("imgui_test_engine_script_key_hold", unsafe {
            sys::imgui_test_engine_script_key_hold(self.script.raw(), key_chord.raw(), seconds)
        })
    }

    pub fn item_hold(&mut self, reference: &str, seconds: f32) -> TestEngineResult<()> {
        validate_reference("item_hold", "reference", reference, true)?;
        validate_nonnegative("item_hold", "seconds", seconds)?;
        let status = with_scratch_txt(reference, |pointer| unsafe {
            sys::imgui_test_engine_script_item_hold(self.script.raw(), pointer, seconds)
        });
        ffi_status("imgui_test_engine_script_item_hold", status)
    }

    pub fn item_hold_for_frames(
        &mut self,
        reference: &str,
        frames: ScriptCount,
    ) -> TestEngineResult<()> {
        validate_reference("item_hold_for_frames", "reference", reference, true)?;
        let status = with_scratch_txt(reference, |pointer| unsafe {
            sys::imgui_test_engine_script_item_hold_for_frames(
                self.script.raw(),
                pointer,
                frames.raw(),
            )
        });
        ffi_status("imgui_test_engine_script_item_hold_for_frames", status)
    }

    pub fn item_drag_over_and_hold(
        &mut self,
        source: &str,
        destination: &str,
    ) -> TestEngineResult<()> {
        validate_reference("item_drag_over_and_hold", "source", source, true)?;
        validate_reference("item_drag_over_and_hold", "destination", destination, true)?;
        let status = with_scratch_txt_two(source, destination, |source_ptr, dest_ptr| unsafe {
            sys::imgui_test_engine_script_item_drag_over_and_hold(
                self.script.raw(),
                source_ptr,
                dest_ptr,
            )
        });
        ffi_status("imgui_test_engine_script_item_drag_over_and_hold", status)
    }

    pub fn item_drag_and_drop(
        &mut self,
        source: &str,
        destination: &str,
        button: MouseButton,
    ) -> TestEngineResult<()> {
        validate_reference("item_drag_and_drop", "source", source, true)?;
        validate_reference("item_drag_and_drop", "destination", destination, true)?;
        let status = with_scratch_txt_two(source, destination, |source_ptr, dest_ptr| unsafe {
            sys::imgui_test_engine_script_item_drag_and_drop(
                self.script.raw(),
                source_ptr,
                dest_ptr,
                button as i32,
            )
        });
        ffi_status("imgui_test_engine_script_item_drag_and_drop", status)
    }

    pub fn item_drag_with_delta(
        &mut self,
        reference: &str,
        dx: f32,
        dy: f32,
    ) -> TestEngineResult<()> {
        validate_reference("item_drag_with_delta", "reference", reference, true)?;
        validate_finite_pair("item_drag_with_delta", "delta", dx, dy)?;
        let status = with_scratch_txt(reference, |pointer| unsafe {
            sys::imgui_test_engine_script_item_drag_with_delta(self.script.raw(), pointer, dx, dy)
        });
        ffi_status("imgui_test_engine_script_item_drag_with_delta", status)
    }

    pub fn scroll_to_x(&mut self, reference: &str, value: f32) -> TestEngineResult<()> {
        self.ref_finite_command(
            "scroll_to_x",
            reference,
            value,
            sys::imgui_test_engine_script_scroll_to_x,
        )
    }

    pub fn scroll_to_y(&mut self, reference: &str, value: f32) -> TestEngineResult<()> {
        self.ref_finite_command(
            "scroll_to_y",
            reference,
            value,
            sys::imgui_test_engine_script_scroll_to_y,
        )
    }

    pub fn scroll_to_pos_x(&mut self, window: &str, value: f32) -> TestEngineResult<()> {
        self.ref_finite_command(
            "scroll_to_pos_x",
            window,
            value,
            sys::imgui_test_engine_script_scroll_to_pos_x,
        )
    }

    pub fn scroll_to_pos_y(&mut self, window: &str, value: f32) -> TestEngineResult<()> {
        self.ref_finite_command(
            "scroll_to_pos_y",
            window,
            value,
            sys::imgui_test_engine_script_scroll_to_pos_y,
        )
    }

    pub fn item_open_all(
        &mut self,
        parent: &str,
        depth: ScriptLimit,
        passes: ScriptLimit,
    ) -> TestEngineResult<()> {
        self.item_all_command(
            "item_open_all",
            parent,
            depth,
            passes,
            sys::imgui_test_engine_script_item_open_all,
        )
    }

    pub fn item_close_all(
        &mut self,
        parent: &str,
        depth: ScriptLimit,
        passes: ScriptLimit,
    ) -> TestEngineResult<()> {
        self.item_all_command(
            "item_close_all",
            parent,
            depth,
            passes,
            sys::imgui_test_engine_script_item_close_all,
        )
    }

    pub fn table_click_header(
        &mut self,
        table: &str,
        label: &str,
        key_mods: KeyMods,
    ) -> TestEngineResult<()> {
        validate_table_text("table_click_header", "table", table)?;
        validate_table_text("table_click_header", "label", label)?;
        let status = with_scratch_txt_two(table, label, |table_ptr, label_ptr| unsafe {
            sys::imgui_test_engine_script_table_click_header(
                self.script.raw(),
                table_ptr,
                label_ptr,
                key_mods.bits(),
            )
        });
        ffi_status("imgui_test_engine_script_table_click_header", status)
    }

    pub fn table_open_context_menu(
        &mut self,
        table: &str,
        column: impl Into<TableColumnRef>,
    ) -> TestEngineResult<()> {
        validate_table_text("table_open_context_menu", "table", table)?;
        let column = table_column_ref("table_open_context_menu", column.into())?;
        let status = with_scratch_txt(table, |table_ptr| unsafe {
            sys::imgui_test_engine_script_table_open_context_menu(
                self.script.raw(),
                table_ptr,
                column,
            )
        });
        ffi_status("imgui_test_engine_script_table_open_context_menu", status)
    }

    pub fn table_set_column_enabled(
        &mut self,
        table: &str,
        column: impl Into<TableColumnIndex>,
        enabled: bool,
    ) -> TestEngineResult<()> {
        validate_table_text("table_set_column_enabled", "table", table)?;
        let column = table_column_index("table_set_column_enabled", column.into())?;
        let status = with_scratch_txt(table, |table_ptr| unsafe {
            sys::imgui_test_engine_script_table_set_column_enabled(
                self.script.raw(),
                table_ptr,
                column,
                enabled,
            )
        });
        ffi_status("imgui_test_engine_script_table_set_column_enabled", status)
    }

    pub fn table_set_column_enabled_by_label(
        &mut self,
        table: &str,
        label: &str,
        enabled: bool,
    ) -> TestEngineResult<()> {
        validate_table_text("table_set_column_enabled_by_label", "table", table)?;
        validate_table_text("table_set_column_enabled_by_label", "label", label)?;
        let status = with_scratch_txt_two(table, label, |table_ptr, label_ptr| unsafe {
            sys::imgui_test_engine_script_table_set_column_enabled_by_label(
                self.script.raw(),
                table_ptr,
                label_ptr,
                enabled,
            )
        });
        ffi_status(
            "imgui_test_engine_script_table_set_column_enabled_by_label",
            status,
        )
    }

    pub fn table_resize_column(
        &mut self,
        table: &str,
        column: impl Into<TableColumnIndex>,
        width: f32,
    ) -> TestEngineResult<()> {
        validate_table_text("table_resize_column", "table", table)?;
        validate_nonnegative("table_resize_column", "width", width)?;
        let column = table_column_index("table_resize_column", column.into())?;
        let status = with_scratch_txt(table, |table_ptr| unsafe {
            sys::imgui_test_engine_script_table_resize_column(
                self.script.raw(),
                table_ptr,
                column,
                width,
            )
        });
        ffi_status("imgui_test_engine_script_table_resize_column", status)
    }

    pub fn set_input_mode(&mut self, mode: InputMode) -> TestEngineResult<()> {
        ffi_status("imgui_test_engine_script_set_input_mode", unsafe {
            sys::imgui_test_engine_script_set_input_mode(self.script.raw(), mode as i32)
        })
    }

    pub fn window_collapse(&mut self, window: &str, collapsed: bool) -> TestEngineResult<()> {
        validate_reference("window_collapse", "window", window, true)?;
        let status = with_scratch_txt(window, |window_ptr| unsafe {
            sys::imgui_test_engine_script_window_collapse(self.script.raw(), window_ptr, collapsed)
        });
        ffi_status("imgui_test_engine_script_window_collapse", status)
    }

    pub fn window_move(&mut self, window: &str, x: f32, y: f32) -> TestEngineResult<()> {
        validate_reference("window_move", "window", window, true)?;
        validate_finite_pair("window_move", "position", x, y)?;
        let status = with_scratch_txt(window, |window_ptr| unsafe {
            sys::imgui_test_engine_script_window_move(self.script.raw(), window_ptr, x, y)
        });
        ffi_status("imgui_test_engine_script_window_move", status)
    }

    pub fn window_resize(&mut self, window: &str, width: f32, height: f32) -> TestEngineResult<()> {
        validate_reference("window_resize", "window", window, true)?;
        validate_nonnegative("window_resize", "width", width)?;
        validate_nonnegative("window_resize", "height", height)?;
        let status = with_scratch_txt(window, |window_ptr| unsafe {
            sys::imgui_test_engine_script_window_resize(
                self.script.raw(),
                window_ptr,
                width,
                height,
            )
        });
        ffi_status("imgui_test_engine_script_window_resize", status)
    }

    pub fn sleep_seconds(&mut self, seconds: f32) -> TestEngineResult<()> {
        validate_nonnegative("sleep_seconds", "seconds", seconds)?;
        ffi_status("imgui_test_engine_script_sleep", unsafe {
            sys::imgui_test_engine_script_sleep(self.script.raw(), seconds)
        })
    }

    pub fn assert_item_read_int_eq(
        &mut self,
        reference: &str,
        expected: i32,
    ) -> TestEngineResult<()> {
        validate_reference("assert_item_read_int_eq", "reference", reference, true)?;
        let status = with_scratch_txt(reference, |reference_ptr| unsafe {
            sys::imgui_test_engine_script_assert_item_read_int_eq(
                self.script.raw(),
                reference_ptr,
                expected,
            )
        });
        ffi_status("imgui_test_engine_script_assert_item_read_int_eq", status)
    }

    pub fn assert_item_read_str_eq(
        &mut self,
        reference: &str,
        expected: &str,
    ) -> TestEngineResult<()> {
        validate_reference("assert_item_read_str_eq", "reference", reference, true)?;
        validate_c_string("assert_item_read_str_eq", "expected", expected, true)?;
        let status =
            with_scratch_txt_two(reference, expected, |reference_ptr, expected_ptr| unsafe {
                sys::imgui_test_engine_script_assert_item_read_str_eq(
                    self.script.raw(),
                    reference_ptr,
                    expected_ptr,
                )
            });
        ffi_status("imgui_test_engine_script_assert_item_read_str_eq", status)
    }

    pub fn assert_item_read_float_eq(
        &mut self,
        reference: &str,
        expected: f32,
        epsilon: f32,
    ) -> TestEngineResult<()> {
        validate_reference("assert_item_read_float_eq", "reference", reference, true)?;
        validate_finite("assert_item_read_float_eq", "expected", expected)?;
        validate_nonnegative("assert_item_read_float_eq", "epsilon", epsilon)?;
        let status = with_scratch_txt(reference, |reference_ptr| unsafe {
            sys::imgui_test_engine_script_assert_item_read_float_eq(
                self.script.raw(),
                reference_ptr,
                expected,
                epsilon,
            )
        });
        ffi_status("imgui_test_engine_script_assert_item_read_float_eq", status)
    }

    pub fn input_text_replace(
        &mut self,
        reference: &str,
        text: &str,
        submit_enter: bool,
    ) -> TestEngineResult<()> {
        self.item_click(reference)?;
        if submit_enter {
            self.key_chars_replace_enter(text)
        } else {
            self.key_chars_replace(text)
        }
    }

    pub fn yield_frames(&mut self, frames: ScriptCount) -> TestEngineResult<()> {
        ffi_status("imgui_test_engine_script_yield", unsafe {
            sys::imgui_test_engine_script_yield(self.script.raw(), frames.raw())
        })
    }

    fn ref_finite_command(
        &mut self,
        operation: &'static str,
        reference: &str,
        value: f32,
        call: unsafe extern "C" fn(
            *mut sys::ImGuiTestEngineScript,
            *const std::os::raw::c_char,
            f32,
        ) -> sys::ImGuiTestEngineStatus,
    ) -> TestEngineResult<()> {
        validate_reference(operation, "reference", reference, true)?;
        validate_finite(operation, "value", value)?;
        let status = with_scratch_txt(reference, |reference_ptr| unsafe {
            call(self.script.raw(), reference_ptr, value)
        });
        ffi_status(operation, status)
    }

    fn item_all_command(
        &mut self,
        operation: &'static str,
        parent: &str,
        depth: ScriptLimit,
        passes: ScriptLimit,
        call: unsafe extern "C" fn(
            *mut sys::ImGuiTestEngineScript,
            *const std::os::raw::c_char,
            i32,
            i32,
        ) -> sys::ImGuiTestEngineStatus,
    ) -> TestEngineResult<()> {
        validate_reference(operation, "parent", parent, true)?;
        let status = with_scratch_txt(parent, |parent_ptr| unsafe {
            call(self.script.raw(), parent_ptr, depth.raw(), passes.raw())
        });
        ffi_status(operation, status)
    }
}

fn validate_c_string(
    operation: &'static str,
    argument: &'static str,
    value: &str,
    allow_empty: bool,
) -> TestEngineResult<()> {
    if value.contains('\0') {
        return Err(TestEngineError::invalid_input(
            operation,
            argument,
            "string contains an interior NUL byte",
        ));
    }
    if !allow_empty && value.is_empty() {
        return Err(TestEngineError::invalid_input(
            operation,
            argument,
            "string must not be empty",
        ));
    }
    Ok(())
}

fn validate_reference(
    operation: &'static str,
    argument: &'static str,
    value: &str,
    allow_empty: bool,
) -> TestEngineResult<()> {
    validate_c_string(operation, argument, value, allow_empty)?;
    if value.len() > MAX_REFERENCE_BYTES {
        return Err(TestEngineError::invalid_input(
            operation,
            argument,
            "reference exceeds the native 254-byte limit",
        ));
    }
    Ok(())
}

fn validate_table_text(
    operation: &'static str,
    argument: &'static str,
    value: &str,
) -> TestEngineResult<()> {
    validate_reference(operation, argument, value, false)
}

fn validate_finite(
    operation: &'static str,
    argument: &'static str,
    value: f32,
) -> TestEngineResult<()> {
    if !value.is_finite() {
        return Err(TestEngineError::invalid_input(
            operation,
            argument,
            "value must be finite",
        ));
    }
    Ok(())
}

fn validate_finite_pair(
    operation: &'static str,
    argument: &'static str,
    x: f32,
    y: f32,
) -> TestEngineResult<()> {
    if !x.is_finite() || !y.is_finite() {
        return Err(TestEngineError::invalid_input(
            operation,
            argument,
            "both values must be finite",
        ));
    }
    Ok(())
}

fn validate_nonnegative(
    operation: &'static str,
    argument: &'static str,
    value: f32,
) -> TestEngineResult<()> {
    validate_finite(operation, argument, value)?;
    if value < 0.0 {
        return Err(TestEngineError::invalid_input(
            operation,
            argument,
            "value must not be negative",
        ));
    }
    Ok(())
}

fn table_column_index(operation: &'static str, column: TableColumnIndex) -> TestEngineResult<i32> {
    column.get().try_into().map_err(|_| {
        TestEngineError::invalid_input(
            operation,
            "column",
            "column index exceeds the native i32 range",
        )
    })
}

fn table_column_ref(operation: &'static str, column: TableColumnRef) -> TestEngineResult<i32> {
    match column {
        TableColumnRef::Current => Ok(-1),
        TableColumnRef::Index(index) => table_column_index(operation, index),
    }
}
