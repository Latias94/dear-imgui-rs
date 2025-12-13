//! Dear ImNodes - Rust Bindings with Dear ImGui Compatibility
//!
//! High-level Rust bindings for ImNodes, the node editor for Dear ImGui.
//! This crate follows the same patterns as our `dear-implot` and `dear-imguizmo`
//! crates: Ui extensions, RAII tokens, and strongly-typed flags/enums.

use dear_imnodes_sys as sys;

// Similar to ImGui 1.92+ return-by-value changes, generated bindings for
// `dear-imnodes-sys` may exist in the build directory in both the legacy
// out-parameter form and the newer return-by-value form. rust-analyzer can end
// up indexing the wrong `OUT_DIR` and report spurious signature errors.
//
// Keep the high-level wrapper stable by calling local extern declarations for
// the return-by-value APIs we expose.
#[allow(non_snake_case)]
pub(crate) mod compat_ffi {
    use super::sys;

    unsafe extern "C" {
        pub fn imnodes_EditorContextGetPanning() -> sys::ImVec2;
        pub fn imnodes_GetNodeScreenSpacePos(node_id: i32) -> sys::ImVec2;
        pub fn imnodes_GetNodeEditorSpacePos(node_id: i32) -> sys::ImVec2;
        pub fn imnodes_GetNodeDimensions(node_id: i32) -> sys::ImVec2;
    }
}

mod context;
mod style;
mod types;
mod ui_ext;

pub use context::*;
pub use style::*;
pub use types::*;
pub use ui_ext::*;
