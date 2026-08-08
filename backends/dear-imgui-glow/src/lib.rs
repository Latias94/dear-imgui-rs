//! Glow (OpenGL) renderer for Dear ImGui
//!
//! This crate provides a Glow-based renderer for Dear ImGui, allowing you to
//! render Dear ImGui interfaces using the Glow OpenGL abstraction.
//!
//! # Features
//!
//! - **Basic rendering**: Render Dear ImGui draw data using OpenGL
//! - **Texture support**: Handle font textures and user textures
//! - **Multi-viewport support**: Stable owning renderer runtime (feature-gated)
//! - **OpenGL compatibility**: Support for OpenGL 3.0+, OpenGL ES 3.0+, and WebGL 2
//! - **Runtime capabilities**: Select sampler objects and state paths from the live context
//! - **External texture ownership**: Preserve application-owned texture filtering and GL state
//! - **Raw callback state**: Expose scoped [`GlowRenderState`] access while callbacks execute
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
//! // let ui = imgui.frame();
//! // ui.text("Hello, world!");
//! // let frame = imgui.render(renderer.renderer_consumer().unwrap());
//! // renderer.render(frame).unwrap();
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
pub use state::{GlowRenderState, GlowRenderStateAccessError, GlowSamplerStrategy};
pub use texture::*;
pub use versions::*;

// Re-export multi-viewport support if enabled
#[cfg(feature = "multi-viewport")]
pub use renderer::multi_viewport;

pub type GlBuffer = <Context as HasContext>::Buffer;
pub type GlTexture = <Context as HasContext>::Texture;
pub type GlVertexArray = <Context as HasContext>::VertexArray;
pub type GlProgram = <Context as HasContext>::Program;
pub type GlSampler = <Context as HasContext>::Sampler;
pub type GlShader = <Context as HasContext>::Shader;
pub type GlUniformLocation = <Context as HasContext>::UniformLocation;

/// Convert a DrawVert slice to a byte slice
///
/// Safety notes:
/// - This intentionally does **not** accept arbitrary `T` to avoid accidentally
///   reading padding bytes from Rust-side structs (which could be uninitialized).
/// - `DrawVert` is a `#[repr(C)]` layout-compatible vertex type with no padding
///   (verified via the size check below).
#[inline]
fn draw_verts_as_bytes(slice: &[dear_imgui_rs::render::DrawVert]) -> &[u8] {
    const _: [(); 20] = [(); std::mem::size_of::<dear_imgui_rs::render::DrawVert>()];
    unsafe { std::slice::from_raw_parts(slice.as_ptr().cast::<u8>(), std::mem::size_of_val(slice)) }
}

/// Convert a DrawIdx slice to a byte slice.
#[inline]
fn draw_indices_as_bytes(slice: &[dear_imgui_rs::render::DrawIdx]) -> &[u8] {
    const _: [(); 2] = [(); std::mem::size_of::<dear_imgui_rs::render::DrawIdx>()];
    unsafe { std::slice::from_raw_parts(slice.as_ptr().cast::<u8>(), std::mem::size_of_val(slice)) }
}
