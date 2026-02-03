//! Vulkan (Ash) renderer backend for Dear ImGui.
//!
//! This crate provides a Vulkan renderer using the `ash` bindings.
//!
//! ## Reference
//!
//! This backend is inspired by:
//! - <https://github.com/adrien-ben/imgui-rs-vulkan-renderer>
//!
//! ## Target support
//!
//! Native targets only. On `wasm32`, this crate provides a stub implementation
//! that always returns `RendererError::UnsupportedTarget`.

mod error;
pub use error::*;

mod texture;
pub use texture::*;

#[cfg(not(target_arch = "wasm32"))]
mod renderer;
#[cfg(not(target_arch = "wasm32"))]
pub use renderer::*;

#[cfg(target_arch = "wasm32")]
mod wasm_stub {
    use super::RendererError;

    /// Stub renderer for `wasm32` targets.
    #[derive(Debug, Default)]
    pub struct AshRenderer;

    impl AshRenderer {
        pub fn new() -> Result<Self, RendererError> {
            Err(RendererError::UnsupportedTarget)
        }
    }

    #[derive(Debug, Default, Clone, Copy)]
    pub struct Options;
}

#[cfg(target_arch = "wasm32")]
pub use wasm_stub::*;
