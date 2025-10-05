//! Low-level FFI bindings for ImGuIZMO.quat via the cimguizmo_quat C API
//!
//! This crate pairs with `dear-imgui-sys` and exposes raw bindings to the
//! ImGuIZMO.quat library using the cimguizmo_quat C API. Prefer using the higher-level
//! `dear-imguizmo-quat` crate for safe, idiomatic Rust wrappers.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unnecessary_transmutes)]
#![allow(clippy::all)]
#![allow(unpredictable_function_pointer_comparisons)]

// Re-export Dear ImGui types for compatibility
pub use dear_imgui_sys::{ImGuiContext, ImGuiID, ImVec2, ImVec4};

// Include generated bindings
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
