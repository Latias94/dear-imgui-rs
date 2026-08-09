//! IO: inputs, configuration and backend capabilities
//!
//! This module wraps Dear ImGui's `ImGuiIO` and related flag types. Access the
//! per-frame IO object via [`Ui::io`], then read inputs or tweak configuration
//! and backend capability flags.
//!
//! Example: enable docking and multi-viewports, and set renderer flags.
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! // Claim managed atlas ownership before advertising managed textures.
//! let _consumer = ctx.create_synchronous_renderer_consumer().unwrap();
//! // Configure IO before starting the first frame.
//! let io = ctx.io_mut();
//! io.set_config_flags(io.config_flags() | ConfigFlags::DOCKING_ENABLE | ConfigFlags::VIEWPORTS_ENABLE);
//! io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_TEXTURES);
//! ```
//!
//! Font scaling is style state rather than IO compatibility state:
//! ```compile_fail
//! # let mut ctx = dear_imgui_rs::Context::create();
//! ctx.io_mut().set_font_global_scale(1.25);
//! ```
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]

mod backend;
mod config;
mod core;
mod display;
mod events;
mod flags;
mod input_state;
mod metrics;
mod mouse;
mod settings;
#[cfg(test)]
mod tests;
mod validation;

pub use core::Io;
pub use flags::{BackendFlags, ConfigFlags, ViewportFlags, WindowClassViewportFlags};

pub(crate) use flags::{validate_backend_flags, validate_config_flags};
pub(crate) use validation::{
    assert_display_framebuffer_scale, assert_display_size, assert_finite_f32, assert_finite_vec2,
    assert_memory_compact_timer, assert_non_negative_f32, assert_positive_f32,
    metric_count_from_i32,
};
