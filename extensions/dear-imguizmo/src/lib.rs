//! # Dear ImGuizmo - Pure Rust Implementation
//!
//! A pure Rust implementation of ImGuizmo, a 3D gizmo manipulation library for Dear ImGui.
//! This crate provides comprehensive 3D transformation tools without requiring C++ FFI bindings.
//!
//! ## Features
//!
//! - **Pure Rust**: No C++ dependencies or FFI bindings
//! - **Translation gizmos**: Move objects in 3D space with visual feedback
//! - **Rotation gizmos**: Rotate objects around axes with arc visualization
//! - **Scale gizmos**: Scale objects uniformly or per-axis
//! - **View manipulation**: Interactive camera controls with cube view
//! - **Grid rendering**: Draw reference grids for spatial orientation
//! - **Matrix utilities**: Powered by glam for efficient math operations
//! - **Customizable styling**: Configure colors, sizes, and appearance
//! - **Extensions**: ImSequencer, ImCurveEdit, and GraphEditor modules
//! - **Error handling**: Comprehensive error handling with thiserror
//! - **Logging**: Integrated tracing support for debugging
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use dear_imgui::*;
//! use dear_imguizmo::*;
//! use glam::Mat4;
//!
//! fn main() -> Result<()> {
//!     let mut imgui_ctx = Context::create();
//!     let mut gizmo_ctx = GuizmoContext::new();
//!
//!     // In your render loop
//!     let ui = imgui_ctx.frame();
//!     let gizmo_ui = gizmo_ctx.get_ui(&ui);
//!
//!     // Your transformation matrix
//!     let mut transform = Mat4::IDENTITY;
//!
//!     // Camera matrices
//!     let view = Mat4::look_at_rh(
//!         glam::Vec3::new(0.0, 0.0, 5.0),
//!         glam::Vec3::ZERO,
//!         glam::Vec3::Y,
//!     );
//!     let projection = Mat4::perspective_rh(
//!         45.0_f32.to_radians(),
//!         16.0 / 9.0,
//!         0.1,
//!         100.0,
//!     );
//!
//!     // Set the viewport
//!     gizmo_ui.set_rect(0.0, 0.0, 800.0, 600.0);
//!
//!     // Manipulate the object
//!     if gizmo_ui.manipulate(
//!         &view,
//!         &projection,
//!         Operation::TRANSLATE,
//!         Mode::WORLD,
//!         &mut transform,
//!     )? {
//!         // Object was manipulated
//!         println!("Object moved!");
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Advanced Usage
//!
//! ```rust,no_run
//! use dear_imgui::*;
//! use dear_imguizmo::*;
//! use glam::{Mat4, Vec3};
//!
//! fn advanced_example() -> Result<()> {
//!     let mut imgui_ctx = Context::create();
//!     let mut gizmo_ctx = GuizmoContext::new();
//!
//!     let ui = imgui_ctx.frame();
//!     let gizmo_ui = gizmo_ctx.get_ui(&ui);
//!
//!     // Configure style
//!     let mut style = gizmo_ui.get_style();
//!     style.translation_line_thickness = 4.0;
//!     style.colors[ColorElement::DirectionX as usize] = [1.0, 0.0, 0.0, 1.0];
//!     gizmo_ui.set_style(&style);
//!
//!     // Enable snapping
//!     let snap = [1.0, 1.0, 1.0]; // 1 unit snap
//!
//!     let mut transform = Mat4::IDENTITY;
//!     let view = Mat4::look_at_rh(Vec3::new(5.0, 5.0, 5.0), Vec3::ZERO, Vec3::Y);
//!     let projection = Mat4::perspective_rh(45.0_f32.to_radians(), 16.0/9.0, 0.1, 100.0);
//!
//!     // Multi-operation gizmo
//!     let operation = Operation::TRANSLATE | Operation::ROTATE | Operation::SCALE_UNIFORM;
//!
//!     if gizmo_ui.manipulate_with_snap(
//!         &view,
//!         &projection,
//!         operation,
//!         Mode::LOCAL,
//!         &mut transform,
//!         Some(&snap),
//!     )? {
//!         println!("Transform changed with snapping!");
//!     }
//!
//!     // Draw helper grid
//!     gizmo_ui.draw_grid(&view, &projection, &Mat4::IDENTITY, 10.0);
//!
//!     // View manipulation cube
//!     let mut view_matrix = view;
//!     gizmo_ui.view_manipulate(
//!         &mut view_matrix,
//!         8.0, // distance
//!         [650.0, 50.0], // position
//!         [128.0, 128.0], // size
//!         0x10101010, // background color
//!     );
//!
//!     Ok(())
//! }
//! ```

#![deny(missing_docs)]
#![warn(clippy::all)]
#![allow(clippy::too_many_arguments)] // ImGuizmo naturally has many parameters

// Re-export essential dependencies
pub use dear_imgui as imgui;
pub use glam;

// Core modules
mod context;
mod error;
mod math;
mod style;
mod types;
mod utils;

// Feature modules
mod draw;
mod gizmo;
mod interaction;
mod view;

// Extension modules
pub mod extensions;

// Re-export public API
pub use context::*;
pub use error::*;
pub use math::*;
pub use style::*;
pub use types::*;
pub use utils::*;

// Re-export commonly used types from dear-imgui
pub use dear_imgui::{Context as ImGuiContext, Ui};

/// Result type for ImGuizmo operations
pub type Result<T> = std::result::Result<T, GuizmoError>;

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Check if the library was compiled with tracing support
pub const HAS_TRACING: bool = cfg!(feature = "tracing");
