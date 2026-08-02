//! Owning SDL3/Ash multi-viewport renderer runtime.

#[cfg(doctest)]
mod removed_free_api_contracts {
    /// ```compile_fail
    /// use dear_imgui_ash::multi_viewport_sdl3::enable;
    /// ```
    struct Enable;

    /// ```compile_fail
    /// use dear_imgui_ash::multi_viewport_sdl3::shutdown_multi_viewport_support;
    /// ```
    struct Shutdown;
}

use super::AshRenderer;
use super::vulkan_viewport::{self, OwningViewportRuntime, SurfaceAdapter, SurfaceCreateError};
use ash::vk::{self, Handle};
use dear_imgui_rs::render::RenderedFrame;
use dear_imgui_rs::{Context, TextureData, TextureId, platform_io::Viewport};
use dear_imgui_sdl3::{Sdl3PlatformBackend, Sdl3VulkanSurfaceProvider};
use std::sync::Arc;

pub use super::vulkan_viewport::{
    AshViewportAttachError, AshViewportError, AshViewportFrameReport, AshViewportFrameTrace,
    PresentModePolicy, SurfaceFormatPolicy, SurfaceSupportError, ViewportSwapchainPolicy,
    VulkanViewportConfig,
};
use crate::{Options, TextureRetirementBatch, TextureUpdateResult};

struct Sdl3SurfaceAdapter {
    provider: Sdl3VulkanSurfaceProvider,
}

impl SurfaceAdapter for Sdl3SurfaceAdapter {
    unsafe fn create_surface(
        &self,
        _entry: &ash::Entry,
        instance: &ash::Instance,
        viewport: &mut Viewport,
    ) -> Result<vk::SurfaceKHR, SurfaceCreateError> {
        let surface = unsafe {
            self.provider
                .create_surface(viewport, instance.handle().as_raw())
        }?;
        Ok(vk::SurfaceKHR::from_raw(surface))
    }
}

/// Owning Ash renderer runtime for the SDL3 Vulkan multi-viewport route.
#[derive(Debug)]
pub struct Sdl3ViewportRuntime {
    inner: OwningViewportRuntime,
}

impl Sdl3ViewportRuntime {
    /// Transactionally attach an initialized renderer to an SDL3 Vulkan platform runtime.
    ///
    /// # Safety
    ///
    /// Every raw Vulkan handle and queue family in `config` must satisfy
    /// [`VulkanViewportConfig`]'s device-lineage and external host-synchronization contracts. The
    /// runtime owns renderer address stability; moving this wrapper is safe.
    pub unsafe fn attach(
        context: &mut Context,
        platform: &Sdl3PlatformBackend,
        renderer: AshRenderer,
        config: VulkanViewportConfig,
    ) -> Result<Self, AshViewportAttachError> {
        let provider = match platform.acquire_vulkan_surface_provider(context) {
            Ok(provider) => provider,
            Err(error) => return Err(AshViewportAttachError::new(error.into(), renderer)),
        };
        unsafe {
            vulkan_viewport::attach_with_adapter(
                renderer,
                context,
                config,
                Arc::new(Sdl3SurfaceAdapter { provider }),
            )
        }
        .map(|inner| Self { inner })
    }

    pub fn poll_fault(&self) -> Result<(), AshViewportError> {
        self.inner.poll_fault()
    }

    /// Starts a non-nestable trace of secondary-viewport Vulkan submissions.
    pub fn begin_frame_trace(&self) -> Result<AshViewportFrameTrace<'_>, AshViewportError> {
        self.inner.begin_frame_trace()
    }

    /// Reconcile managed texture requests before any secondary viewport can draw this frame.
    pub fn prepare_frame(
        &self,
        frame: &mut RenderedFrame<'_>,
    ) -> Result<Option<TextureRetirementBatch>, AshViewportError> {
        self.inner.prepare_frame(frame)
    }

    /// Records the main viewport through the owned renderer.
    ///
    /// # Safety
    ///
    /// `command_buffer` must satisfy [`AshRenderer::cmd_draw`].
    pub unsafe fn cmd_draw(
        &self,
        command_buffer: vk::CommandBuffer,
        frame: RenderedFrame<'_>,
    ) -> Result<Option<TextureRetirementBatch>, AshViewportError> {
        unsafe { self.inner.cmd_draw(command_buffer, frame) }
    }

    /// Return the highest managed-texture resource retirement batch still pending.
    pub fn pending_texture_retirement(
        &self,
    ) -> Result<Option<TextureRetirementBatch>, AshViewportError> {
        self.inner.pending_texture_retirement()
    }

    /// Block for device idle and complete managed-texture resource retirement.
    ///
    /// The count includes superseded update images and resources pending a logical destroy.
    /// Recorded but unsubmitted command buffers that reference released resources must not be
    /// submitted afterwards; see [`AshRenderer::cmd_draw`].
    pub fn wait_for_texture_retirements(
        &self,
        batch: TextureRetirementBatch,
    ) -> Result<usize, AshViewportError> {
        self.inner.wait_for_texture_retirements(batch)
    }

    /// Complete managed-texture resource retirement after validating fences are signaled.
    ///
    /// The count includes superseded update images and resources pending a logical destroy.
    ///
    /// # Safety
    ///
    /// Every fence must belong to this renderer's device and together cover all uploads and draws
    /// on every queue which can reference textures through `batch`. No recorded command buffer
    /// which references released resources may be submitted afterwards.
    pub unsafe fn complete_texture_retirements_with_fences(
        &self,
        batch: TextureRetirementBatch,
        fences: &[vk::Fence],
    ) -> Result<usize, AshViewportError> {
        unsafe {
            self.inner
                .complete_texture_retirements_with_fences(batch, fences)
        }
    }

    pub fn options(&self) -> Result<Options, AshViewportError> {
        self.inner.options()
    }

    pub fn set_viewport_clear_color(&self, color: [f32; 4]) -> Result<(), AshViewportError> {
        self.inner.set_viewport_clear_color(color)
    }

    pub fn viewport_clear_color(&self) -> Result<[f32; 4], AshViewportError> {
        self.inner.viewport_clear_color()
    }

    /// Run a read-only, non-escaping renderer inspection.
    pub fn with_renderer<R>(
        &self,
        callback: impl FnOnce(&AshRenderer) -> R,
    ) -> Result<R, AshViewportError> {
        self.inner.with_renderer(callback)
    }

    /// Register an application-owned sampled image with the shared viewport renderer.
    ///
    /// # Safety
    ///
    /// The image view must belong to this renderer's device and satisfy the lifetime and layout
    /// contract documented by [`AshRenderer::register_external_texture`].
    pub unsafe fn register_external_texture(
        &self,
        image_view: vk::ImageView,
        image_layout: vk::ImageLayout,
    ) -> Result<TextureId, AshViewportError> {
        unsafe {
            self.inner
                .register_external_texture(image_view, image_layout)
        }
    }

    /// Update an application-owned sampled image after waiting for device idle.
    ///
    /// # Safety
    ///
    /// The new image view must satisfy [`AshRenderer::update_external_texture`].
    pub unsafe fn update_external_texture(
        &self,
        texture: TextureId,
        image_view: vk::ImageView,
        image_layout: vk::ImageLayout,
    ) -> Result<bool, AshViewportError> {
        unsafe {
            self.inner
                .update_external_texture(texture, image_view, image_layout)
        }
    }

    /// Update an external sampled image without blocking for device idle.
    ///
    /// # Safety
    ///
    /// No submitted or recorded command may still access this texture's descriptor set.
    pub unsafe fn update_external_texture_unchecked(
        &self,
        texture: TextureId,
        image_view: vk::ImageView,
        image_layout: vk::ImageLayout,
    ) -> Result<bool, AshViewportError> {
        unsafe {
            self.inner
                .update_external_texture_unchecked(texture, image_view, image_layout)
        }
    }

    /// Unregister a texture after waiting for submitted device work.
    ///
    /// # Safety
    ///
    /// No recorded command buffer that references this texture may be submitted after this call.
    pub unsafe fn unregister_texture(&self, texture: TextureId) -> Result<(), AshViewportError> {
        unsafe { self.inner.unregister_texture(texture) }
    }

    /// Unregister a texture without blocking for device idle.
    ///
    /// # Safety
    ///
    /// No submitted or recorded command may still access this texture's descriptor set.
    pub unsafe fn unregister_texture_unchecked(
        &self,
        texture: TextureId,
    ) -> Result<(), AshViewportError> {
        unsafe { self.inner.unregister_texture_unchecked(texture) }
    }

    /// Synchronize and apply one legacy texture transition.
    ///
    /// Recorded but unsubmitted command buffers that reference replaced resources must not be
    /// submitted afterwards; see [`AshRenderer::cmd_draw`].
    pub fn update_texture(
        &self,
        texture: &TextureData,
    ) -> Result<TextureUpdateResult, AshViewportError> {
        self.inner.update_texture(texture)
    }

    /// Apply a legacy texture transition without renderer-managed synchronization.
    ///
    /// # Safety
    ///
    /// Earlier users must be complete, and upload completion or ordering must be proven before
    /// using a created or updated texture.
    pub unsafe fn update_texture_unchecked(
        &self,
        texture: &TextureData,
    ) -> Result<TextureUpdateResult, AshViewportError> {
        unsafe { self.inner.update_texture_unchecked(texture) }
    }

    /// Release renderer callbacks, secondary resources, and the Ash renderer.
    ///
    /// Recorded but unsubmitted command buffers from this renderer must not be submitted after
    /// shutdown; see [`AshRenderer::cmd_draw`].
    pub fn shutdown(&mut self, context: &mut Context) -> Result<(), AshViewportError> {
        self.inner.shutdown(context)
    }

    pub fn retry_retained_cleanup(&mut self) -> Result<(), AshViewportError> {
        self.inner.retry_retained_cleanup()
    }
}
