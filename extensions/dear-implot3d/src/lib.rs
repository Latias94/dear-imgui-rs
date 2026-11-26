//! Dear ImPlot3D - Rust bindings (high level)
//!
//! Safe wrapper over `dear-implot3d-sys`, designed to integrate with
//! `dear-imgui-rs`. Mirrors `dear-implot` design: context + Ui facade,
//! builder-style helpers, optional `mint` inputs.
//!
//! # Quick Start
//!
//! ```no_run
//! use dear_imgui_rs::*;
//! use dear_implot3d::*;
//!
//! let mut imgui_ctx = Context::create();
//! let plot3d_ctx = Plot3DContext::create(&imgui_ctx);
//!
//! // In your main loop:
//! let ui = imgui_ctx.frame();
//! let plot_ui = ui.implot3d(&plot3d_ctx);
//!
//! plot_ui.begin_plot("3D Plot").build(|plot| {
//!     let xs = [0.0, 1.0, 2.0];
//!     let ys = [0.0, 1.0, 0.0];
//!     let zs = [0.0, 0.5, 1.0];
//!     plot.plot_line_f32("Line", &xs, &ys, &zs, Line3DFlags::NONE);
//! });
//! ```
//!
//! # Features
//!
//! - **mint**: Enable support for `mint` math types (Point3, Vector3)
//!
//! # Architecture
//!
//! This crate follows the same design patterns as `dear-implot`:
//! - `Plot3DContext`: Manages the ImPlot3D context (create once)
//! - `Plot3DUi`: Per-frame plot builders and context style scopes
//! - `Plot3DSetup`: Axis and box configuration before native setup locks
//! - `Plot3DScope`: Item submission and plot queries within the drawing closure
//! - Builder pattern: Fluent API for configuring plots
//! - Type-safe flags: Using `bitflags!` for compile-time safety

pub(crate) use dear_imgui_rs::sys as imgui_sys;
pub use dear_imgui_rs::{Context, Ui};
pub(crate) use dear_implot3d_sys as sys;

mod builder;
mod context;
mod demos;
mod flags;
mod image_builder;
mod item_style;
mod layout;
mod mesh_builder;
pub mod meshes;
pub mod plots;
mod style;
mod surface_builder;
mod ui;

mod axis;

pub use builder::Plot3DBuilder;
pub use context::Plot3DContext;
pub use flags::*;
pub use image_builder::{Image3DByAxesBuilder, Image3DByCornersBuilder};
pub use item_style::*;
pub use layout::{Plot3DDataLayout, Plot3DDataOffset, Plot3DDataStride};
pub use mesh_builder::Mesh3DBuilder;
pub use plots::*;
pub use style::*;
pub use surface_builder::Surface3DBuilder;
pub use ui::{ImPlot3DExt, Plot3DScope, Plot3DSetup, Plot3DUi};

pub(crate) use layout::{axis_tick_count_to_i32, imvec2, imvec4, len_i32};
