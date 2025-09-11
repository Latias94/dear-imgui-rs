//! Low-level FFI bindings for ImNodeFlow
//!
//! This crate provides unsafe, low-level bindings to ImNodeFlow, a node editor
//! library built on top of Dear ImGui. These bindings are designed to work
//! with the dear-imgui ecosystem rather than imgui-rs.
//!
//! For safe, high-level bindings, use the `dear-imnodeflow` crate instead.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(improper_ctypes)]

// Re-export types from dear-imgui-sys that ImNodeFlow uses
pub use dear_imgui_sys::{
    ImColor, ImDrawData, ImDrawList, ImFontAtlas, ImGuiCond, ImGuiContext, ImGuiDragDropFlags,
    ImGuiIO, ImGuiKey, ImGuiMouseButton, ImGuiStyle, ImTextureID, ImU32, ImVec2, ImVec4,
};

// Include the generated bindings
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

// Pin types
pub const PIN_TYPE_INPUT: i32 = 0;
pub const PIN_TYPE_OUTPUT: i32 = 1;

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn test_create_destroy_nodeflow() {
        unsafe {
            let inf = ImNodeFlow_CreateDefault();
            assert!(!inf.is_null());
            ImNodeFlow_Destroy(inf);
        }
    }

    #[test]
    fn test_create_destroy_named_nodeflow() {
        unsafe {
            let name = CString::new("Test").unwrap();
            let inf = ImNodeFlow_Create(name.as_ptr());
            assert!(!inf.is_null());
            ImNodeFlow_Destroy(inf);
        }
    }

    #[test]
    fn test_create_destroy_node() {
        unsafe {
            let node = BaseNode_Create();
            assert!(!node.is_null());
            BaseNode_Destroy(node);
        }
    }

    #[test]
    fn test_pin_styles() {
        unsafe {
            let cyan = PinStyle_Cyan();
            assert!(!cyan.is_null());

            let green = PinStyle_Green();
            assert!(!green.is_null());

            let blue = PinStyle_Blue();
            assert!(!blue.is_null());
        }
    }

    #[test]
    fn test_node_styles() {
        unsafe {
            let cyan = NodeStyle_Cyan();
            assert!(!cyan.is_null());

            let green = NodeStyle_Green();
            assert!(!green.is_null());

            let red = NodeStyle_Red();
            assert!(!red.is_null());
        }
    }
}
