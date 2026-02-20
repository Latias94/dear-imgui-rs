//! Low-level FFI bindings for Dear ImGui Test Engine.
//!
//! This crate provides raw bindings to a small C shim over the upstream C++
//! `imgui_test_engine` API. Prefer `dear-imgui-test-engine` for a safer,
//! idiomatic wrapper.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unnecessary_transmutes)]
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(clippy::all)]

pub use dear_imgui_sys::ImGuiContext;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
