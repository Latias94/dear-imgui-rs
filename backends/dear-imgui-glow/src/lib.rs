//! Glow (OpenGL) renderer for Dear ImGui
//!
//! This crate provides a Glow-based renderer for Dear ImGui, allowing you to
//! render Dear ImGui interfaces using the Glow OpenGL abstraction.
//!
//! # Features
//!
//! - **Basic rendering**: Render Dear ImGui draw data using OpenGL
//! - **Texture support**: Handle font textures and user textures
//! - **Multi-viewport support**: Support for multiple windows (feature-gated)
//! - **OpenGL compatibility**: Support for OpenGL 2.1+ and OpenGL ES 2.0+
//!
//! # Example
//!
//! ```rust,no_run
//! use dear_imgui_rs::Context;
//! use dear_imgui_glow::GlowRenderer;
//! use glow::HasContext;
//!
//! // Initialize your OpenGL context and Dear ImGui context
//! let gl = unsafe { glow::Context::from_loader_function(|s| {
//!     // Your OpenGL loader function
//!     std::ptr::null()
//! }) };
//! let mut imgui = Context::create();
//!
//! // Create the renderer (simple usage)
//! let mut renderer = GlowRenderer::new(gl, &mut imgui).unwrap();
//!
//! // In your render loop:
//! // imgui.new_frame();
//! // ... build your UI ...
//! // let draw_data = imgui.render();
//! // renderer.render(draw_data).unwrap();
//! ```

// Re-export glow to make it easier for users to use the correct version.
pub use glow;
use glow::{Context, HasContext};

mod error;
mod renderer;
mod shaders;
mod state;
mod texture;
mod versions;

pub use error::*;
pub use renderer::*;
pub use texture::*;
pub use versions::*;

// Re-export multi-viewport support if enabled
#[cfg(feature = "multi-viewport")]
pub use renderer::multi_viewport;

pub type GlBuffer = <Context as HasContext>::Buffer;
pub type GlTexture = <Context as HasContext>::Texture;
pub type GlVertexArray = <Context as HasContext>::VertexArray;
pub type GlProgram = <Context as HasContext>::Program;
pub type GlUniformLocation = <Context as HasContext>::UniformLocation;

/// Convert a slice to a byte slice
#[inline]
fn to_byte_slice<T>(slice: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(slice.as_ptr() as *const u8, std::mem::size_of_val(slice)) }
}

/// Debug message helper for OpenGL debugging
#[cfg(feature = "debug_message_insert_support")]
fn gl_debug_message(gl: &Context, message: &str) {
    unsafe {
        gl.debug_message_insert(
            glow::DEBUG_SOURCE_APPLICATION,
            glow::DEBUG_TYPE_MARKER,
            0,
            glow::DEBUG_SEVERITY_NOTIFICATION,
            message,
        );
    }
}

#[cfg(not(feature = "debug_message_insert_support"))]
fn gl_debug_message(_gl: &Context, _message: &str) {
    // No-op when debug messages are not supported
}
