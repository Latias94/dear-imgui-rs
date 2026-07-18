//! Vulkan (Ash) renderer implementation.

#[cfg(all(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
compile_error!(
    "dear-imgui-ash cannot enable both `multi-viewport-winit` and `multi-viewport-sdl3`; select one platform surface adapter"
);

mod allocator;
mod callbacks;
mod core;
mod draw;
mod lifecycle;
#[cfg(all(feature = "multi-viewport-winit", not(feature = "multi-viewport-sdl3")))]
pub mod multi_viewport;
#[cfg(all(feature = "multi-viewport-sdl3", not(feature = "multi-viewport-winit")))]
pub mod multi_viewport_sdl3;
mod options;
#[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
mod pipeline;
mod retirement;
mod shaders;
#[cfg(test)]
mod tests;
mod texture;
mod uploads;
mod vulkan;
#[cfg(all(
    any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"),
    not(all(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))
))]
mod vulkan_viewport;

use crate::TextureUpdateResult;
use crate::{RendererError, RendererResult};
#[cfg(not(any(feature = "gpu-allocator", feature = "vk-mem")))]
use ash::Instance;
use ash::{Device, vk};
#[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
use dear_imgui_rs::ViewportFlags;
use dear_imgui_rs::render::{
    RenderedFrame, RendererConsumer, SnapshotTextureId, TextureFeedback, TextureOp, TextureRequest,
    TextureUploadRect,
};
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
#[cfg(all(
    any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"),
    not(feature = "dynamic-rendering")
))]
use self::pipeline::create_viewport_render_pass;
#[cfg(all(
    any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"),
    not(all(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))
))]
use self::pipeline::viewport_attachment_load_op;
#[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
use self::pipeline::{ViewportPipeline, is_srgb_format};
pub use self::retirement::TextureRetirementBatch;
use self::retirement::{RetirementQueue, RetirementRequest};
use self::texture::{PendingTextureUpdate, TextureManager};
use self::uploads::{
    InFlightUpload, ManagedUploadDecision, ManagedUploadTracker, UploadSignature,
    finish_destroy_upload_gate,
};
use self::vulkan::*;
