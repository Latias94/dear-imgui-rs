//! Safe bindings for `imgui-node-editor`.
//!
//! This crate is a richer node-editor companion to `dear-imnodes`. It is backed
//! by `cimnodes_editor` / `imgui-node-editor`, but exposes Rust-side IDs as
//! pointer-sized newtypes instead of upstream C++ ID helper objects.

mod config;
mod context;
mod frame;
mod style;
mod types;
mod ui_ext;

pub use config::*;
pub use context::*;
pub use frame::*;
pub use style::*;
pub use types::*;
pub use ui_ext::*;

pub(crate) use dear_node_editor_sys as sys;

#[inline]
pub(crate) fn vec2(value: [f32; 2]) -> sys::ImVec2_c {
    sys::ImVec2_c {
        x: value[0],
        y: value[1],
    }
}

#[inline]
pub(crate) fn vec4(value: [f32; 4]) -> sys::ImVec4_c {
    sys::ImVec4_c {
        x: value[0],
        y: value[1],
        z: value[2],
        w: value[3],
    }
}

#[inline]
pub(crate) fn from_vec2(value: sys::ImVec2_c) -> [f32; 2] {
    [value.x, value.y]
}
