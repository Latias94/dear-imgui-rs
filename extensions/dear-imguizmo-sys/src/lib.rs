//! Low-level FFI bindings for ImGuizmo via the cimguizmo C API
//!
//! This crate pairs with `dear-imgui-sys` and exposes raw bindings to the
//! ImGuizmo library using the cimguizmo C API. Prefer using the higher-level
//! `dear-imguizmo` crate for safe, idiomatic Rust wrappers.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unnecessary_transmutes)]
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(clippy::all)]
#![allow(unpredictable_function_pointer_comparisons)]

// Re-export Dear ImGui types for compatibility
pub use dear_imgui_sys::{ImDrawList, ImGuiContext, ImGuiID, ImVec2, ImVec4};

// Include generated bindings
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
