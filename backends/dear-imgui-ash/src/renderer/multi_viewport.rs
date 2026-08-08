//! Owning Winit/Ash multi-viewport route.

#[cfg(doctest)]
mod removed_free_api_contracts {
    /// ```compile_fail
    /// use dear_imgui_ash::multi_viewport::enable;
    /// ```
    struct Enable;

    /// ```compile_fail
    /// use dear_imgui_ash::multi_viewport::shutdown_multi_viewport_support;
    /// ```
    struct Shutdown;

    /// The prepared capability carries same-scope secondary evidence. Retirement state stays
    /// opaque inside the move-only completion capability.
    ///
    /// ```
    /// use dear_imgui_ash::multi_viewport::{
    ///     AshPreparedViewportFrame, AshViewportFrameCompletion, AshViewportFrameReport,
    /// };
    ///
    /// fn skip_main<'a>(
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
    /// use dear_imgui_ash::multi_viewport::AshViewportFrameTrace;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_ash::multi_viewport::WinitViewportRoute;
    /// let _ = WinitViewportRoute::begin_frame_trace;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_ash::multi_viewport::WinitViewportRoute;
    /// let _ = WinitViewportRoute::cmd_draw_reconciled;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_ash::multi_viewport::WinitViewportRoute;
    /// let _ = WinitViewportRoute::cmd_draw;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_ash::multi_viewport::AshPreparedViewportFrame;
    /// fn expose_batch(frame: &AshPreparedViewportFrame<'_>) {
    ///     let _ = frame.texture_retirement();
    /// }
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_ash::multi_viewport::WinitViewportRuntime;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_ash::multi_viewport::AshViewportFrameCompletion;
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
use ash::vk;
use dear_imgui_rs::{Context, FrameToken, TextureData, TextureId, platform_io::Viewport};
use dear_imgui_winit::{WinitPlatform, multi_viewport::WinitViewportRendererAdapter};
use std::sync::Arc;
use winit::event_loop::ActiveEventLoop;
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::Window;

pub use super::vulkan_viewport::{
    AshPreparedViewportFrame, AshViewportAttachError, AshViewportError, AshViewportFrameCompletion,
    AshViewportFrameReport, AshViewportRouteError, AshViewportRouteFault, PresentModePolicy,
    SurfaceFormatPolicy, SurfaceSupportError, ViewportSwapchainPolicy, VulkanViewportConfig,
};
use crate::{Options, TextureUpdateResult};

struct WinitSurfaceAdapter;

impl SurfaceAdapter for WinitSurfaceAdapter {
    unsafe fn create_surface(
        &self,
        entry: &ash::Entry,
        instance: &ash::Instance,
        viewport: &mut Viewport,
    ) -> Result<vk::SurfaceKHR, SurfaceCreateError> {
        let window_ptr = viewport.platform_handle();
        if window_ptr.is_null() {
            return Err(SurfaceCreateError::MissingPlatformHandle);
        }
        let window = unsafe { &*(window_ptr as *const Window) };
        let display = window
            .display_handle()
            .map_err(|_| SurfaceCreateError::DisplayHandleUnavailable)?;
        let window_handle = window
            .window_handle()
            .map_err(|_| SurfaceCreateError::WindowHandleUnavailable)?;

        unsafe {
            ash_window::create_surface(
                entry,
                instance,
                display.as_raw(),
                window_handle.as_raw(),
                None,
            )
            .map_err(Into::into)
        }
    }
}

/// Owning Winit/Ash multi-viewport route.
///
/// The route consumes the renderer into stable boxed storage, retains the exact Winit platform
/// generation, and owns renderer callbacks, secondary swapchains, and deferred renderer faults.
#[derive(Debug)]
pub struct WinitViewportRoute {
    inner: OwningViewportRuntime,
    platform: WinitViewportRendererAdapter,
}

impl WinitViewportRoute {
    /// Transactionally attach an initialized renderer to a Winit platform runtime.
    ///
    /// Failure returns the renderer through [`AshViewportAttachError`].
    ///
    /// # Safety
    ///
    /// Every raw Vulkan handle and queue family in `config` must satisfy
    /// [`VulkanViewportConfig`]'s device-lineage and external host-synchronization contracts. The
    /// route owns renderer address stability; moving this wrapper is safe.
    pub unsafe fn attach(
        context: &mut Context,
        platform: &WinitPlatform,
        renderer: AshRenderer,
        config: VulkanViewportConfig,
    ) -> Result<Self, AshViewportAttachError> {
        if platform.context_id() != context.id() {
            return Err(AshViewportAttachError::new(
                AshViewportError::PlatformOwnerContextMismatch {
                    backend: "Winit",
                    expected: context.id(),
                    actual: platform.context_id(),
                },
                renderer,
            ));
        }
        let platform = match platform.viewport_renderer_adapter(context) {
            Ok(platform) => platform,
            Err(error) => {
                return Err(AshViewportAttachError::new(
                    AshViewportError::WinitPlatform(error),
                    renderer,
                ));
            }
        };
        unsafe {
            vulkan_viewport::attach_with_adapter(
                renderer,
                context,
                config,
                Arc::new(WinitSurfaceAdapter),
            )
        }
        .map(|inner| Self { inner, platform })
    }

    /// Consumes an open frame, reconciles textures, and completes secondary viewports.
    ///
    /// Call this before acquiring the application's main surface. The exact Winit platform
    /// generation lends the active event loop only for this transaction. Every renderer and
    /// platform callback fault raised by the route is returned together without losing FIFO order
    /// within either source. A frame from another Context is rejected before entering the Winit
    /// event-loop scope or consuming any deferred route fault.
    pub fn prepare<'ctx>(
        &self,
        event_loop: &ActiveEventLoop,
        frame: FrameToken<'ctx>,
    ) -> Result<
        AshPreparedViewportFrame<'ctx>,
        AshViewportRouteError<dear_imgui_winit::WinitPlatformError>,
    > {
        let actual = frame.ui().context_id();
        self.inner.prepare_route_for_context(actual, || {
            debug_assert_eq!(self.platform.context_id(), self.inner.context_id());
            self.platform
                .with_event_loop(event_loop, |_| self.inner.prepare(frame))
                .into_parts()
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

    /// Explicitly release renderer callbacks, secondary resources, and the Ash renderer.
    ///
    /// Recorded but unsubmitted command buffers from this renderer must not be submitted after
    /// shutdown; see [`AshRenderer::cmd_draw`].
    pub fn shutdown(&mut self, context: &mut Context) -> Result<(), AshViewportError> {
        self.inner.shutdown(context)
    }

    /// Retry cleanup retained after Context-first teardown returned a recoverable wait error.
    pub fn retry_retained_cleanup(&mut self) -> Result<(), AshViewportError> {
        self.inner.retry_retained_cleanup()
    }
}
