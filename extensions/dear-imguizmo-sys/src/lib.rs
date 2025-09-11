//! Low-level FFI bindings for ImGuizmo
//!
//! This crate provides raw, unsafe bindings to the ImGuizmo C++ library.
//! For a safe, idiomatic Rust API, use the `dear-imguizmo` crate instead.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(clippy::all)]

// Re-export types from dear-imgui-sys that ImGuizmo uses
pub use dear_imgui_sys::{
    ImDrawList, ImFontAtlas, ImGuiCond, ImGuiContext, ImGuiDragDropFlags, ImGuiID, ImGuiIO,
    ImGuiMouseButton, ImGuiStyle, ImGuiWindow, ImTextureID, ImU32, ImVec2, ImVec4,
};

// Include the generated bindings
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

// Re-export commonly used types and constants
pub use ImGuizmo_COLOR as COLOR;
pub use ImGuizmo_MODE as MODE;
pub use ImGuizmo_OPERATION as OPERATION;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        // Test that constants are properly defined
        assert_ne!(TRANSLATE_X, TRANSLATE_Y);
        assert_ne!(LOCAL, WORLD);
        assert_ne!(DIRECTION_X, DIRECTION_Y);
    }
}
