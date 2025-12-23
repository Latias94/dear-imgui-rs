//! Low-level FFI bindings for ImNodes via the cimnodes C API
//!
//! This crate provides raw, unsafe bindings to the ImNodes library using the
//! cimnodes C API, designed to work together with `dear-imgui-sys` (which uses
//! cimgui for Dear ImGui). This avoids C++ ABI issues and keeps builds
//! consistent across platforms and toolchains.
//!
//! This crate is typically not used directly. Prefer the high-level
//! `dear-imnodes` crate for safe bindings.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unnecessary_transmutes)]
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(clippy::all)]
// Bindgen can derive Eq/Hash for structs with function pointers; silence related warnings.
#![allow(unpredictable_function_pointer_comparisons)]

// Re-export ImGui types from dear-imgui-sys to ensure compatibility
pub use dear_imgui_sys::{ImDrawList, ImGuiContext, ImVec2, ImVec4};

// Include the generated bindings from bindgen
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
