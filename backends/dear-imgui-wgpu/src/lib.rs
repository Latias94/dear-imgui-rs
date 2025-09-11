//! WGPU backend for Dear ImGui
//!
//! This crate provides a WGPU-based renderer for Dear ImGui, allowing you to
//! render Dear ImGui interfaces using the WGPU graphics API.
//!
//! # Features
//!
//! - **Modern texture management**: Full integration with Dear ImGui's ImTextureData system
//! - **Gamma correction**: Automatic sRGB format detection and gamma correction
//! - **Multi-frame buffering**: Support for multiple frames in flight
//! - **Device object management**: Proper handling of device loss and recovery
//! - **Multi-viewport support**: Support for multiple windows (feature-gated)
//!
//! # Example
//!
//! ```rust,no_run
//! use dear_imgui::Context;
//! use dear_imgui_wgpu::{WgpuRenderer, WgpuInitInfo};
//! use wgpu::*;
//!
//! // Initialize WGPU device and queue
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let instance = Instance::new(&InstanceDescriptor::default());
//! let adapter = instance.request_adapter(&RequestAdapterOptions::default()).await.unwrap();
//! let (device, queue) = adapter.request_device(&DeviceDescriptor::default()).await?;
//!
//! // Create Dear ImGui context
//! let mut imgui = Context::create_or_panic();
//!
//! // Create renderer
//! let init_info = WgpuInitInfo::new(device, queue, TextureFormat::Bgra8UnormSrgb);
//! let mut renderer = WgpuRenderer::new();
//! renderer.init(init_info)?;
//! renderer.configure_imgui_context(&mut imgui);
//! renderer.prepare_font_atlas(&mut imgui)?;
//!
//! // In your render loop:
//! // imgui.new_frame();
//! // ... build your UI ...
//! // let draw_data = imgui.render();
//! // renderer.render_draw_data(&draw_data, &mut render_pass)?;
//! # Ok(())
//! # }
//! ```

// Module declarations
mod data;
mod error;
mod frame_resources;
mod render_resources;
mod renderer;
mod shaders;
mod texture;
mod uniforms;

// Re-exports
pub use data::*;
pub use error::*;
pub use frame_resources::*;
pub use render_resources::*;
pub use renderer::*;
pub use shaders::*;
pub use texture::*;
pub use uniforms::*;

// Legacy compatibility exports
#[deprecated(note = "Use the new modular API instead")]
pub use crate::texture::WgpuTexture;

#[deprecated(note = "Use WgpuTextureManager instead")]
pub use crate::texture::WgpuTextureManager as WgpuTextureMap;
