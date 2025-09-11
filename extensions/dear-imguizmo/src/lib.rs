//! # Dear ImGuizmo
//!
//! High-level Rust bindings for ImGuizmo, a 3D gizmo library for Dear ImGui.
//!
//! ImGuizmo provides interactive 3D manipulation widgets (gizmos) for translation,
//! rotation, and scaling operations in 3D space.
//!
//! ## Features
//!
//! - **Translation gizmos**: Move objects in 3D space
//! - **Rotation gizmos**: Rotate objects around axes
//! - **Scale gizmos**: Scale objects uniformly or per-axis
//! - **View manipulation**: Interactive camera controls
//! - **Grid rendering**: Draw reference grids
//! - **Matrix utilities**: Decompose and recompose transformation matrices
//! - **Customizable styling**: Configure colors, sizes, and appearance
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use dear_imgui::*;
//! use dear_imguizmo::*;
//!
//! fn main() {
//!     let mut imgui_ctx = Context::create_or_panic();
//!     let mut gizmo_ctx = GuizmoContext::create(&imgui_ctx);
//!
//!     // In your render loop
//!     let ui = imgui_ctx.frame();
//!     let gizmo_ui = gizmo_ctx.get_ui(&ui);
//!
//!     // Set up the viewport
//!     gizmo_ui.set_rect(0.0, 0.0, 800.0, 600.0);
//!
//!     // Your transformation matrix
//!     let mut matrix = [1.0, 0.0, 0.0, 0.0,
//!                       0.0, 1.0, 0.0, 0.0,
//!                       0.0, 0.0, 1.0, 0.0,
//!                       0.0, 0.0, 0.0, 1.0];
//!
//!     // Camera matrices
//!     let view = get_view_matrix();
//!     let projection = get_projection_matrix();
//!
//!     // Manipulate the object
//!     if let Some(result) = gizmo_ui.manipulate(&view, &projection)
//!         .operation(Operation::TRANSLATE)
//!         .mode(Mode::WORLD)
//!         .matrix(&mut matrix)
//!         .build() {
//!         // Object was manipulated
//!         println!("Object moved!");
//!     }
//! }
//! ```

#![deny(missing_docs)]

use std::ffi::CString;
use std::ptr;

pub use dear_imgui as imgui;
pub use dear_imguizmo_sys as sys;

mod context;
mod gizmo;
mod style;
mod types;

pub use context::*;
pub use gizmo::*;
pub use style::*;
pub use types::*;

/// Re-export commonly used types from dear-imgui
pub use dear_imgui::{Context as ImGuiContext, Ui};

/// Result type for ImGuizmo operations
pub type Result<T> = std::result::Result<T, Error>;

/// Error types for ImGuizmo operations
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Invalid matrix data
    #[error("Invalid matrix data: {0}")]
    InvalidMatrix(String),

    /// Invalid operation
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    /// Context not initialized
    #[error("GuizmoContext not properly initialized")]
    ContextNotInitialized,
}

/// Convert a Rust string to a C string safely
pub(crate) fn to_cstring(s: &str) -> CString {
    CString::new(s).unwrap_or_else(|_| CString::new("").unwrap())
}

/// Convert an optional Rust string to a C string pointer
pub(crate) fn to_cstring_ptr(s: Option<&str>) -> *const i8 {
    match s {
        Some(s) => to_cstring(s).as_ptr(),
        None => ptr::null(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cstring_conversion() {
        let s = "test";
        let c_str = to_cstring(s);
        assert_eq!(c_str.to_str().unwrap(), s);
    }

    #[test]
    fn test_cstring_ptr_conversion() {
        let s = Some("test");
        let ptr = to_cstring_ptr(s);
        assert!(!ptr.is_null());

        let none_ptr = to_cstring_ptr(None);
        assert!(none_ptr.is_null());
    }
}
