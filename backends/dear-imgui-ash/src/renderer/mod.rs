//! Vulkan (Ash) renderer implementation.

mod allocator;
mod callbacks;
mod core;
mod draw;
mod lifecycle;
#[cfg(feature = "multi-viewport-winit")]
pub mod multi_viewport;
#[cfg(feature = "multi-viewport-sdl3")]
pub mod multi_viewport_sdl3;
mod options;
#[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
mod pipeline;
mod shaders;
#[cfg(test)]
mod tests;
mod texture;
mod uploads;
mod vulkan;

use crate::TextureUpdateResult;
use crate::{RendererError, RendererResult};
#[cfg(not(any(feature = "gpu-allocator", feature = "vk-mem")))]
use ash::Instance;
use ash::{Device, vk};
use dear_imgui_rs::{BackendFlags, Context};
use dear_imgui_rs::{TextureData, TextureFormat as ImGuiTextureFormat, TextureId, TextureStatus};
use std::collections::{HashMap, VecDeque};

use self::allocator::{Allocate, Allocator, Memory};
use self::callbacks::draw_callback_reset_render_state;
pub use self::core::AshRenderer;
use self::draw::Frames;
#[cfg(feature = "dynamic-rendering")]
pub use self::options::DynamicRendering;
pub use self::options::Options;
#[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
use self::pipeline::{ViewportPipeline, create_viewport_render_pass, is_srgb_format};
use self::texture::{PendingTextureCreate, PendingTextureUpdate, TextureManager};
use self::uploads::InFlightUpload;
use self::vulkan::*;
