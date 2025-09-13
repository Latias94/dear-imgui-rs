// Windows-specific function wrappers to handle ABI issues
// This module provides C-style wrappers for functions that have ABI issues on Windows

use crate::*;

// For now, we'll provide placeholder implementations
// These should be replaced with actual function calls once the FFI is working

#[cfg(target_os = "windows")]
pub mod windows {
    use super::*;

    // Real implementations using the actual Dear ImGui functions
    pub unsafe fn get_version() -> *const std::os::raw::c_char {
        crate::ImGui_GetVersion()
    }

    pub unsafe fn create_context(shared_font_atlas: *mut ImFontAtlas) -> *mut ImGuiContext {
        crate::ImGui_CreateContext(shared_font_atlas)
    }

    pub unsafe fn destroy_context(ctx: *mut ImGuiContext) {
        crate::ImGui_DestroyContext(ctx)
    }

    pub unsafe fn get_current_context() -> *mut ImGuiContext {
        crate::ImGui_GetCurrentContext()
    }

    pub unsafe fn set_current_context(ctx: *mut ImGuiContext) {
        crate::ImGui_SetCurrentContext(ctx)
    }

    pub unsafe fn get_io() -> *mut ImGuiIO {
        crate::ImGui_GetIO()
    }

    pub unsafe fn get_style() -> *mut ImGuiStyle {
        crate::ImGui_GetStyle()
    }

    pub unsafe fn new_frame() {
        crate::ImGui_NewFrame()
    }

    pub unsafe fn render() {
        crate::ImGui_Render()
    }

    pub unsafe fn get_draw_data() -> *mut ImDrawData {
        crate::ImGui_GetDrawData()
    }

    pub unsafe fn text_unformatted(
        text: *const std::os::raw::c_char,
        text_end: *const std::os::raw::c_char,
    ) {
        crate::ImGui_TextUnformatted(text, text_end)
    }

    pub unsafe fn button(label: *const std::os::raw::c_char, size: ImVec2) -> bool {
        crate::ImGui_Button(label, &size)
    }

    pub unsafe fn begin(
        name: *const std::os::raw::c_char,
        p_open: *mut bool,
        flags: ImGuiWindowFlags,
    ) -> bool {
        crate::ImGui_Begin(name, p_open, flags)
    }

    pub unsafe fn end() {
        crate::ImGui_End()
    }

    pub unsafe fn set_next_window_size(size: ImVec2, cond: ImGuiCond) {
        crate::ImGui_SetNextWindowSize(&size, cond)
    }

    pub unsafe fn set_next_window_pos(pos: ImVec2, cond: ImGuiCond, pivot: ImVec2) {
        crate::ImGui_SetNextWindowPos(&pos, cond, &pivot)
    }

    pub unsafe fn set_next_window_size_constraints(
        size_min: ImVec2,
        size_max: ImVec2,
        custom_callback: Option<unsafe extern "C" fn(*mut ImGuiSizeCallbackData)>,
        custom_callback_data: *mut std::os::raw::c_void,
    ) {
        crate::ImGui_SetNextWindowSizeConstraints(
            &size_min,
            &size_max,
            custom_callback,
            custom_callback_data,
        )
    }
}

#[cfg(all(not(target_os = "windows"), feature = "wasm"))]
pub mod wasm {
    use super::*;

    // For WASM, use the same ImGui_* function names as native for consistency
    pub use crate::ImGui_Begin as begin;
    pub use crate::ImGui_Button as button;
    pub use crate::ImGui_CreateContext as create_context;
    pub use crate::ImGui_DestroyContext as destroy_context;
    pub use crate::ImGui_End as end;
    pub use crate::ImGui_GetCurrentContext as get_current_context;
    pub use crate::ImGui_GetDrawData as get_draw_data;
    pub use crate::ImGui_GetIO as get_io;
    pub use crate::ImGui_GetStyle as get_style;
    pub use crate::ImGui_GetVersion as get_version;
    pub use crate::ImGui_NewFrame as new_frame;
    pub use crate::ImGui_Render as render;
    pub use crate::ImGui_SetCurrentContext as set_current_context;
    pub use crate::ImGui_SetNextWindowPos as set_next_window_pos;
    pub use crate::ImGui_SetNextWindowSize as set_next_window_size;
    pub use crate::ImGui_SetNextWindowSizeConstraints as set_next_window_size_constraints;
    pub use crate::ImGui_Text as text;
    pub use crate::ImGui_TextUnformatted as text_unformatted;
}

#[cfg(all(not(target_os = "windows"), not(feature = "wasm")))]
pub mod unix {
    use super::*;

    // On non-Windows native platforms, use the ImGui_* function names
    pub use crate::ImGui_Begin as begin;
    pub use crate::ImGui_Button as button;
    pub use crate::ImGui_CreateContext as create_context;
    pub use crate::ImGui_DestroyContext as destroy_context;
    pub use crate::ImGui_End as end;
    pub use crate::ImGui_GetCurrentContext as get_current_context;
    pub use crate::ImGui_GetDrawData as get_draw_data;
    pub use crate::ImGui_GetIO as get_io;
    pub use crate::ImGui_GetStyle as get_style;
    pub use crate::ImGui_GetVersion as get_version;
    pub use crate::ImGui_NewFrame as new_frame;
    pub use crate::ImGui_Render as render;
    pub use crate::ImGui_SetCurrentContext as set_current_context;
    pub use crate::ImGui_SetNextWindowPos as set_next_window_pos;
    pub use crate::ImGui_SetNextWindowSize as set_next_window_size;
    pub use crate::ImGui_SetNextWindowSizeConstraints as set_next_window_size_constraints;
    pub use crate::ImGui_TextUnformatted as text_unformatted;
}

// Public API that automatically selects the right implementation
#[cfg(target_os = "windows")]
pub use windows::*;

#[cfg(all(not(target_os = "windows"), feature = "wasm"))]
pub use wasm::*;

#[cfg(all(not(target_os = "windows"), not(feature = "wasm")))]
pub use unix::*;
