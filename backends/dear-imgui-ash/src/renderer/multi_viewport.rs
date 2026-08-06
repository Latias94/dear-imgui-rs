//! Owning Winit/Ash multi-viewport renderer runtime.

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
}

use super::AshRenderer;
use super::vulkan_viewport::{self, OwningViewportRuntime, SurfaceAdapter, SurfaceCreateError};
use ash::vk;
use dear_imgui_rs::render::{PendingFrame, ReconciledFrame};
use dear_imgui_rs::{Context, TextureData, TextureId, platform_io::Viewport};
use dear_imgui_winit::multi_viewport::WinitPlatformRuntime as WinitPlatformOwner;
use std::sync::Arc;
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::Window;

pub use super::vulkan_viewport::{
    AshViewportAttachError, AshViewportError, AshViewportFrameReport, AshViewportFrameTrace,
    PresentModePolicy, SurfaceFormatPolicy, SurfaceSupportError, ViewportSwapchainPolicy,
    VulkanViewportConfig,
};
use crate::{Options, TextureRetirementBatch, TextureUpdateResult};

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

/// Owning Ash renderer runtime for the Winit multi-viewport route.
///
/// The runtime consumes the renderer into stable boxed storage and owns the renderer attachment,
/// callback claim, secondary swapchains, and deferred callback errors.
#[derive(Debug)]
pub struct WinitViewportRuntime {
    inner: OwningViewportRuntime,
}

impl WinitViewportRuntime {
    /// Transactionally attach an initialized renderer to a Winit platform runtime.
    ///
    /// Failure returns the renderer through [`AshViewportAttachError`].
    ///
    /// # Safety
    ///
    /// Every raw Vulkan handle and queue family in `config` must satisfy
    /// [`VulkanViewportConfig`]'s device-lineage and external host-synchronization contracts. The
    /// runtime owns renderer address stability; moving this wrapper is safe.
    pub unsafe fn attach(
        context: &mut Context,
        platform: &WinitPlatformOwner,
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
        if let Err(error) = platform.validate_renderer_owner(context) {
            return Err(AshViewportAttachError::new(
                AshViewportError::WinitPlatform(error),
                renderer,
            ));
        }
        unsafe { Self::attach_unchecked(context, renderer, config) }
    }

    /// Attaches an Ash renderer to a custom platform that follows the Winit viewport-handle
    /// contract.
    ///
    /// # Safety
    ///
    /// Every raw Vulkan handle and queue family in `config` must satisfy
    /// [`VulkanViewportConfig`]'s device-lineage and external host-synchronization contracts. The
    /// current Context must also have a live Winit-compatible platform runtime. Every viewport's
    /// `PlatformHandle` must point to its live `winit::Window`, and the platform must keep those
    /// windows alive until this renderer runtime has released its callbacks and resources. Prefer
    /// [`Self::attach`] for the built-in
    /// [`WinitPlatformRuntime`](dear_imgui_winit::multi_viewport::WinitPlatformRuntime).
    pub unsafe fn attach_unchecked(
        context: &mut Context,
        renderer: AshRenderer,
        config: VulkanViewportConfig,
    ) -> Result<Self, AshViewportAttachError> {
        unsafe {
            vulkan_viewport::attach_with_adapter(
                renderer,
                context,
                config,
                Arc::new(WinitSurfaceAdapter),
            )
        }
        .map(|inner| Self { inner })
    }

    /// Return and clear the oldest deferred callback or ownership fault.
    pub fn poll_fault(&self) -> Result<(), AshViewportError> {
        self.inner.poll_fault()
    }

    /// Starts a non-nestable trace of secondary-viewport Vulkan submissions.
    pub fn begin_frame_trace(&self) -> Result<AshViewportFrameTrace<'_>, AshViewportError> {
        self.inner.begin_frame_trace()
    }

    /// Reconcile managed texture requests before any secondary viewport can draw this frame.
    pub fn prepare_frame<'ctx>(
        &self,
        frame: PendingFrame<'ctx>,
    ) -> Result<(ReconciledFrame<'ctx>, Option<TextureRetirementBatch>), AshViewportError> {
        self.inner.prepare_frame(frame)
    }

    /// Finalize and reconcile the runtime's Context before secondary viewport rendering.
    pub fn prepare_context<'ctx>(
        &self,
        context: &'ctx mut Context,
    ) -> Result<(ReconciledFrame<'ctx>, Option<TextureRetirementBatch>), AshViewportError> {
        self.inner.prepare_context(context)
    }

    /// Reconcile textures and record one Context-owned frame.
    ///
    /// # Safety
    ///
    /// `command_buffer` must satisfy [`AshRenderer::cmd_draw`].
    pub unsafe fn cmd_draw(
        &self,
        command_buffer: vk::CommandBuffer,
        frame: PendingFrame<'_>,
    ) -> Result<Option<TextureRetirementBatch>, AshViewportError> {
        unsafe { self.inner.cmd_draw(command_buffer, frame) }
    }

    /// Record a main viewport after [`Self::prepare_frame`] reconciled this frame.
    ///
    /// # Safety
    ///
    /// `command_buffer` must satisfy [`AshRenderer::cmd_draw`], and `frame` must come from this
    /// runtime's [`Self::prepare_frame`] call for the current Dear ImGui frame.
    pub unsafe fn cmd_draw_reconciled(
        &self,
        command_buffer: vk::CommandBuffer,
        frame: ReconciledFrame<'_>,
    ) -> Result<Option<TextureRetirementBatch>, AshViewportError> {
        unsafe { self.inner.cmd_draw_reconciled(command_buffer, frame) }
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
