//! Low-level FFI bindings for cimCTE and ImGuiColorTextEdit.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unnecessary_transmutes)]
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(clippy::all)]
#![allow(unpredictable_function_pointer_comparisons)]

pub use dear_imgui_sys::{
    ImDrawList, ImGuiChildFlags, ImGuiContext, ImGuiKeyChord, ImGuiWindowFlags, ImTextureID, ImU32,
    ImVec2, ImVec2_c, ImVec4, ImVec4_c, ImWchar,
};

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
