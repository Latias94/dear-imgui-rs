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
use dear_imgui_rs::render::RenderedFrame;
use dear_imgui_rs::{Context, TextureData, TextureId, platform_io::Viewport};
use std::sync::Arc;
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::Window;

pub use super::vulkan_viewport::{
    AshViewportAttachError, AshViewportError, SurfaceSupportError, VulkanViewportConfig,
};
use crate::{Options, TextureRetirementBatch, TextureUpdateResult};

const PLATFORM_NAME_PREFIX: &str = "dear-imgui-winit ";

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

fn validate_platform(context: &Context) -> Result<(), AshViewportError> {
    let actual = context
        .io()
        .backend_platform_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "<unset>".to_string());
    if actual.starts_with(PLATFORM_NAME_PREFIX) {
        Ok(())
    } else {
        Err(AshViewportError::PlatformBackendMismatch {
            expected: "dear-imgui-winit",
            actual,
        })
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
    /// [`VulkanViewportConfig`]'s device-lineage contract. The runtime owns renderer address
    /// stability; moving this wrapper is safe.
    pub unsafe fn attach(
        context: &mut Context,
        renderer: AshRenderer,
        config: VulkanViewportConfig,
    ) -> Result<Self, AshViewportAttachError> {
        if let Err(error) = validate_platform(context) {
            return Err(AshViewportAttachError::new(error, renderer));
        }
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

    /// Reconcile textures and record one Context-owned frame.
    pub fn cmd_draw(
        &self,
        command_buffer: vk::CommandBuffer,
        frame: RenderedFrame<'_>,
    ) -> Result<Option<TextureRetirementBatch>, AshViewportError> {
        self.inner.cmd_draw(command_buffer, frame)
    }

    /// Return the highest managed-texture retirement batch still pending.
    pub fn pending_texture_retirement(
        &self,
    ) -> Result<Option<TextureRetirementBatch>, AshViewportError> {
        self.inner.pending_texture_retirement()
    }

    /// Block for device idle and complete managed-texture retirement.
    pub fn wait_for_texture_retirements(
        &self,
        batch: TextureRetirementBatch,
    ) -> Result<usize, AshViewportError> {
        self.inner.wait_for_texture_retirements(batch)
    }

    /// Complete managed-texture retirement after validating caller-provided fences are signaled.
    ///
    /// # Safety
    ///
    /// Every fence must belong to this renderer's device and together cover all uploads and draws
    /// on every queue which can reference textures through `batch`.
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

    pub fn register_texture_descriptor_set(
        &self,
        set: vk::DescriptorSet,
    ) -> Result<TextureId, AshViewportError> {
        self.inner.register_texture_descriptor_set(set)
    }

    pub fn remove_texture_descriptor_set(
        &self,
        texture: TextureId,
    ) -> Result<(), AshViewportError> {
        self.inner.remove_texture_descriptor_set(texture)
    }

    pub fn register_external_texture_with_sampler(
        &self,
        image_view: vk::ImageView,
        sampler: vk::Sampler,
    ) -> Result<TextureId, AshViewportError> {
        self.inner
            .register_external_texture_with_sampler(image_view, sampler)
    }

    pub fn update_external_texture_view(
        &self,
        texture: TextureId,
        image_view: vk::ImageView,
    ) -> Result<bool, AshViewportError> {
        self.inner.update_external_texture_view(texture, image_view)
    }

    /// Update an external texture view without blocking for device idle.
    ///
    /// # Safety
    ///
    /// No submitted or recorded command may still access this texture's descriptor set.
    pub unsafe fn update_external_texture_view_unchecked(
        &self,
        texture: TextureId,
        image_view: vk::ImageView,
    ) -> Result<bool, AshViewportError> {
        unsafe {
            self.inner
                .update_external_texture_view_unchecked(texture, image_view)
        }
    }

    pub fn update_external_texture_sampler(
        &self,
        texture: TextureId,
        sampler: vk::Sampler,
    ) -> Result<bool, AshViewportError> {
        self.inner.update_external_texture_sampler(texture, sampler)
    }

    /// Update an external sampler without blocking for device idle.
    ///
    /// # Safety
    ///
    /// No submitted or recorded command may still access this texture's descriptor set.
    pub unsafe fn update_external_texture_sampler_unchecked(
        &self,
        texture: TextureId,
        sampler: vk::Sampler,
    ) -> Result<bool, AshViewportError> {
        unsafe {
            self.inner
                .update_external_texture_sampler_unchecked(texture, sampler)
        }
    }

    pub fn unregister_texture(&self, texture: TextureId) -> Result<(), AshViewportError> {
        self.inner.unregister_texture(texture)
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
    pub fn shutdown(&mut self, context: &mut Context) -> Result<(), AshViewportError> {
        self.inner.shutdown(context)
    }

    /// Retry cleanup retained after Context-first teardown returned a recoverable wait error.
    pub fn retry_retained_cleanup(&mut self) -> Result<(), AshViewportError> {
        self.inner.retry_retained_cleanup()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrong_platform_name_is_rejected_before_callback_claim() {
        let _guard = vulkan_viewport::test_context_guard();
        let mut context = Context::create();
        context
            .set_platform_name(Some("imgui_impl_sdl3 (3.2.0; 3.2.0)".to_string()))
            .unwrap();

        assert!(matches!(
            validate_platform(&context),
            Err(AshViewportError::PlatformBackendMismatch { .. })
        ));
        assert!(context.platform_io().renderer_callbacks_are_empty());
    }
}
