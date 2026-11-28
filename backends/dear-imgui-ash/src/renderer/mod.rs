//! Vulkan (Ash) renderer implementation.

#[cfg(doctest)]
mod removed_renderer_api_contracts {
    /// ```compile_fail
    /// use ash::{Device, Instance, vk};
    /// use dear_imgui_ash::{AshRenderer, Options};
    /// use dear_imgui_rs::Context;
    ///
    /// # fn old_constructor(
    /// #     instance: &Instance,
    /// #     physical_device: vk::PhysicalDevice,
    /// #     device: Device,
    /// #     queue: vk::Queue,
    /// #     command_pool: vk::CommandPool,
    /// #     render_pass: vk::RenderPass,
    /// #     context: &mut Context,
    /// # ) {
    /// let _ = unsafe {
    ///     AshRenderer::with_default_allocator(
    ///         instance,
    ///         physical_device,
    ///         device,
    ///         queue,
    ///         command_pool,
    ///         render_pass,
    ///         context,
    ///         Some(Options::default()),
    ///     )
    /// };
    /// # }
    /// ```
    struct PositionalConstructor;

    /// ```compile_fail
    /// use ash::vk;
    /// use dear_imgui_ash::AshRenderer;
    /// # fn old(renderer: &mut AshRenderer, set: vk::DescriptorSet) {
    /// let _ = unsafe { renderer.register_texture_descriptor_set(set) };
    /// # }
    /// ```
    struct RawDescriptorSetRegistration;

    /// ```compile_fail
    /// use ash::vk;
    /// use dear_imgui_ash::AshRenderer;
    /// # fn old(renderer: &mut AshRenderer, view: vk::ImageView, sampler: vk::Sampler) {
    /// let _ = unsafe { renderer.register_external_texture_with_sampler(view, sampler) };
    /// # }
    /// ```
    struct CombinedImageSamplerRegistration;

    /// ```compile_fail
    /// use ash::vk;
    /// use dear_imgui_ash::AshRenderer;
    /// use dear_imgui_rs::TextureId;
    /// # fn old(renderer: &mut AshRenderer, texture: TextureId, view: vk::ImageView) {
    /// let _ = unsafe { renderer.update_external_texture_view(texture, view) };
    /// # }
    /// ```
    struct ViewOnlyUpdate;

    /// ```compile_fail
    /// use ash::vk;
    /// use dear_imgui_ash::AshRenderer;
    /// use dear_imgui_rs::TextureId;
    /// # fn old(renderer: &mut AshRenderer, texture: TextureId, sampler: vk::Sampler) {
    /// let _ = unsafe { renderer.update_external_texture_sampler(texture, sampler) };
    /// # }
    /// ```
    struct SamplerUpdate;

    /// ```compile_fail
    /// use ash::vk;
    /// use dear_imgui_ash::Options;
    ///
    /// let _ = Options {
    ///     texture_format: vk::Format::D32_SFLOAT,
    ///     ..Options::default()
    /// };
    /// ```
    struct ArbitraryManagedTextureFormat;
}

mod allocator;
mod callbacks;
mod context_state;
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
mod retirement;
mod shaders;
#[cfg(test)]
mod tests;
mod texture;
mod uploads;
mod vulkan;
#[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
mod vulkan_viewport;

use crate::TextureUpdateResult;
use crate::{RendererError, RendererResult};
use ash::Instance;
use ash::{Device, vk};
use dear_imgui_backend_support::ScopedRenderState as RendererRenderStateGuard;
#[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
use dear_imgui_rs::ViewportFlags;
use dear_imgui_rs::render::{
    PendingFrame, ReconciledFrame, SnapshotTextureId, SynchronousRendererConsumer, TextureFeedback,
    TextureOp, TextureRequest, TextureUploadRect,
};
use dear_imgui_rs::{Context, FrameError};
use dear_imgui_rs::{TextureData, TextureFormat as ImGuiTextureFormat, TextureId, TextureStatus};
use std::collections::HashMap;

use self::allocator::{Allocate, Allocator, Memory};
pub use self::callbacks::{AshRenderState, AshRenderStateAccessError};
use self::callbacks::{
    AshRenderStateStorage, draw_callback_reset_render_state, draw_callback_set_sampler_linear,
    draw_callback_set_sampler_nearest,
};
use self::context_state::RendererContextState;
pub use self::core::AshRenderer;
use self::core::RendererAllocator;
use self::draw::Frames;
#[cfg(feature = "dynamic-rendering")]
pub use self::options::DynamicRendering;
#[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
use self::options::RenderMode;
pub use self::options::{AshRendererConfig, Options, RenderTarget};
#[cfg(all(
    any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"),
    feature = "render-pass"
))]
use self::pipeline::create_viewport_render_pass;
#[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
use self::pipeline::viewport_attachment_load_op;
#[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
use self::pipeline::{ViewportPipeline, ViewportRenderTarget, is_srgb_format};
pub use self::retirement::TextureRetirementBatch;
use self::retirement::{RetirementQueue, RetirementRequest, RetirementReservation};
use self::texture::TextureManager;
use self::uploads::{UploadRing, grown_buffer_capacity};
use self::vulkan::*;

fn map_renderer_render_state_error(error: FrameError) -> RendererError {
    match error {
        FrameError::MissingPlatformIo => RendererError::InvalidRenderState(
            "bound Dear ImGui context has no PlatformIO".to_owned(),
        ),
        FrameError::AlreadyOccupied => RendererError::InvalidRenderState(
            "Dear ImGui Renderer_RenderState is already occupied".to_owned(),
        ),
        FrameError::Drift => RendererError::RendererStateDrift {
            field: "Renderer_RenderState",
        },
        error => RendererError::Frame(error),
    }
}

fn platform_io_for_current_context() -> RendererResult<*mut dear_imgui_rs::sys::ImGuiPlatformIO> {
    let context = unsafe { dear_imgui_rs::sys::igGetCurrentContext() };
    let platform_io = unsafe { dear_imgui_rs::sys::igGetPlatformIO_ContextPtr(context) };
    if platform_io.is_null() {
        Err(RendererError::InvalidRenderState(
            "bound Dear ImGui context has no PlatformIO".to_owned(),
        ))
    } else {
        Ok(platform_io)
    }
}
