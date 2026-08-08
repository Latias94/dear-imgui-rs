//! Owning SDL3/Ash multi-viewport route.

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

    /// The prepared capability carries same-scope secondary evidence. Retirement state stays
    /// opaque inside the move-only completion capability.
    ///
    /// ```
    /// use dear_imgui_ash::multi_viewport_sdl3::{
    ///     AshPreparedViewportFrame, AshViewportFrameCompletion, AshViewportFrameReport,
    /// };
    ///
    /// fn preparation_evidence<'a>(
    ///     frame: &'a AshPreparedViewportFrame<'_>,
    /// ) -> &'a AshViewportFrameReport {
    ///     frame.secondary_report()
    /// }
    ///
    /// fn preserve_retirement(frame: AshPreparedViewportFrame<'_>) -> AshViewportFrameCompletion {
    ///     frame.skip_main()
    /// }
    /// ```
    ///
    /// Manual tracing and partially prepared main-viewport entry points are intentionally absent.
    ///
    /// ```compile_fail
    /// use dear_imgui_ash::multi_viewport_sdl3::AshViewportFrameTrace;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_ash::multi_viewport_sdl3::Sdl3ViewportRoute;
    /// let _ = Sdl3ViewportRoute::begin_frame_trace;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_ash::multi_viewport_sdl3::Sdl3ViewportRoute;
    /// let _ = Sdl3ViewportRoute::cmd_draw_reconciled;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_ash::multi_viewport_sdl3::Sdl3ViewportRoute;
    /// let _ = Sdl3ViewportRoute::cmd_draw;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_ash::multi_viewport_sdl3::AshPreparedViewportFrame;
    /// fn expose_batch(frame: &AshPreparedViewportFrame<'_>) {
    ///     let _ = frame.texture_retirement();
    /// }
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_ash::multi_viewport_sdl3::Sdl3ViewportRuntime;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_ash::multi_viewport_sdl3::AshViewportFrameCompletion;
    /// fn duplicate(completion: AshViewportFrameCompletion) {
    ///     let first = completion;
    ///     let second = completion;
    ///     drop((first, second));
    /// }
    /// ```
    struct PreparedTransaction;
}

use super::AshRenderer;
use super::vulkan_viewport::{self, OwningViewportRuntime, SurfaceAdapter, SurfaceCreateError};
use ash::vk::{self, Handle};
use dear_imgui_rs::{Context, FrameToken, TextureData, TextureId, platform_io::Viewport};
use dear_imgui_sdl3::{
    Sdl3PlatformBackend, Sdl3ViewportRendererAdapter, Sdl3VulkanSurfaceProvider,
};
use std::sync::Arc;

pub use super::vulkan_viewport::{
    AshPreparedViewportFrame, AshViewportAttachError, AshViewportError, AshViewportFrameCompletion,
    AshViewportFrameReport, AshViewportRouteError, AshViewportRouteFault, PresentModePolicy,
    SurfaceFormatPolicy, SurfaceSupportError, ViewportSwapchainPolicy, VulkanViewportConfig,
};
use crate::{Options, TextureUpdateResult};

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

/// Owning SDL3/Ash multi-viewport route.
#[derive(Debug)]
pub struct Sdl3ViewportRoute {
    inner: OwningViewportRuntime,
    platform: Sdl3ViewportRendererAdapter,
}

impl Sdl3ViewportRoute {
    /// Transactionally attach an initialized renderer to an SDL3 Vulkan platform runtime.
    ///
    /// # Safety
    ///
    /// Every raw Vulkan handle and queue family in `config` must satisfy
    /// [`VulkanViewportConfig`]'s device-lineage and external host-synchronization contracts. The
    /// route owns renderer address stability; moving this wrapper is safe.
    pub unsafe fn attach(
        context: &mut Context,
        platform: &Sdl3PlatformBackend,
        renderer: AshRenderer,
        config: VulkanViewportConfig,
    ) -> Result<Self, AshViewportAttachError> {
        let platform_adapter = match platform.viewport_renderer_adapter(context) {
            Ok(platform) => platform,
            Err(error) => return Err(AshViewportAttachError::new(error.into(), renderer)),
        };
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
        .map(|inner| Self {
            inner,
            platform: platform_adapter,
        })
    }

    /// Consumes an open frame, reconciles textures, and completes secondary viewports.
    ///
    /// Call this before acquiring the application's main surface. Every renderer and platform
    /// callback fault raised by the exact SDL3 platform generation is returned together without
    /// losing FIFO order within either source. A frame from another Context is rejected before
    /// entering the SDL3 platform scope or consuming any deferred route fault.
    pub fn prepare<'ctx>(
        &self,
        frame: FrameToken<'ctx>,
    ) -> Result<
        AshPreparedViewportFrame<'ctx>,
        AshViewportRouteError<dear_imgui_sdl3::Sdl3BackendError>,
    > {
        let actual = frame.ui().context_id();
        self.inner.prepare_route_for_context(actual, || {
            debug_assert_eq!(self.platform.context_id(), self.inner.context_id());
            self.platform.run(|| self.inner.prepare(frame)).into_parts()
        })
    }

    /// Record the main viewport from a frame whose secondary viewports are already complete.
    ///
    /// # Safety
    ///
    /// `command_buffer` must satisfy [`AshRenderer::cmd_draw`]. Queue submission and the GPU
    /// completion proof remain caller-owned. Pass the returned capability to
    /// [`Self::wait_for_frame_completion`] or [`Self::complete_frame_with_fences`] only after every
    /// relevant upload, secondary draw, and main draw has completed.
    pub unsafe fn cmd_draw_main(
        &self,
        command_buffer: vk::CommandBuffer,
        prepared: AshPreparedViewportFrame<'_>,
    ) -> Result<AshViewportFrameCompletion, AshViewportError> {
        unsafe { self.inner.cmd_draw_main(command_buffer, prepared) }
    }

    /// Wait for device idle and complete this frame's managed-texture retirement.
    ///
    /// When no resources are pending, the capability is consumed and the method returns zero
    /// without waiting. Otherwise the returned count includes superseded update images and
    /// resources pending a logical destroy.
    /// Recorded but unsubmitted command buffers that reference released resources must not be
    /// submitted afterwards; see [`AshRenderer::cmd_draw`].
    pub fn wait_for_frame_completion(
        &self,
        completion: AshViewportFrameCompletion,
    ) -> Result<usize, AshViewportError> {
        self.inner.wait_for_frame_completion(completion)
    }

    /// Complete this frame's managed-texture retirement after validating fences are signaled.
    ///
    /// The count includes superseded update images and resources pending a logical destroy.
    ///
    /// # Safety
    ///
    /// Every fence must belong to this renderer's device and together cover every queue operation
    /// that can reference resources associated with `completion`, including uploads, secondary
    /// draws, and the main draw when it was recorded. No recorded command buffer which references
    /// released resources may be submitted afterwards.
    pub unsafe fn complete_frame_with_fences(
        &self,
        completion: AshViewportFrameCompletion,
        fences: &[vk::Fence],
    ) -> Result<usize, AshViewportError> {
        unsafe { self.inner.complete_frame_with_fences(completion, fences) }
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
