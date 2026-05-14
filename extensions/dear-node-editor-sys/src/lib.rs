//! Low-level FFI bindings for `imgui-node-editor`.
//!
//! This crate binds a repository-local C ABI shim (`dne_*`) layered over
//! `cimnodes_editor` / `imgui-node-editor`. The shim deliberately exposes
//! node, pin, and link IDs as `uintptr_t` values instead of the upstream C++
//! `NodeId*` / `PinId*` / `LinkId*` helper objects.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unnecessary_transmutes)]
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(clippy::all)]

pub use dear_imgui_sys::{
    ImDrawList, ImGuiContext, ImGuiMouseButton, ImVec2, ImVec2_c, ImVec4, ImVec4_c,
};

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
