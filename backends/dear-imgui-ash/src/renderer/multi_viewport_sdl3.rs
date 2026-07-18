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
use dear_imgui_rs::{Context, TextureData, TextureId, platform_io::Viewport, sys};
use std::{ffi::c_void, sync::Arc};

pub use super::vulkan_viewport::{
    AshViewportAttachError, AshViewportError, SurfaceSupportError, VulkanViewportConfig,
};
use crate::{Options, TextureRetirementBatch, TextureUpdateResult};

type PlatformCreateVkSurfaceFn = unsafe extern "C" fn(
    vp: *mut sys::ImGuiViewport,
    vk_inst: sys::ImU64,
    vk_allocators: *const c_void,
    out_vk_surface: *mut sys::ImU64,
) -> std::os::raw::c_int;

const PLATFORM_NAME_PREFIX: &str = "imgui_impl_sdl3 (";

struct Sdl3SurfaceAdapter {
    create_surface: PlatformCreateVkSurfaceFn,
}

fn validate_platform(context: &Context) -> Result<PlatformCreateVkSurfaceFn, AshViewportError> {
    let actual = context
        .io()
        .backend_platform_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "<unset>".to_string());
    if !actual.starts_with(PLATFORM_NAME_PREFIX) {
        return Err(AshViewportError::PlatformBackendMismatch {
            expected: "imgui_impl_sdl3",
            actual,
        });
    }
    context
        .platform_io()
        .platform_create_vk_surface_raw()
        .ok_or(AshViewportError::PlatformCreateVkSurfaceUnavailable)
}

impl SurfaceAdapter for Sdl3SurfaceAdapter {
    unsafe fn create_surface(
        &self,
        _entry: &ash::Entry,
        instance: &ash::Instance,
        viewport: &mut Viewport,
    ) -> Result<vk::SurfaceKHR, SurfaceCreateError> {
        let mut out_surface: sys::ImU64 = 0;
        let code = unsafe {
            (self.create_surface)(
                viewport.as_raw_mut(),
                instance.handle().as_raw(),
                std::ptr::null(),
                &mut out_surface,
            )
        };
        if code != 0 || out_surface == 0 {
            return Err(SurfaceCreateError::PlatformCallbackFailed {
                code,
                surface: out_surface,
            });
        }
        Ok(vk::SurfaceKHR::from_raw(out_surface))
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
    /// [`VulkanViewportConfig`]'s device-lineage contract. The runtime owns renderer address
    /// stability; moving this wrapper is safe.
    pub unsafe fn attach(
        context: &mut Context,
        renderer: AshRenderer,
        config: VulkanViewportConfig,
    ) -> Result<Self, AshViewportAttachError> {
        let create_surface = match validate_platform(context) {
            Ok(callback) => callback,
            Err(error) => return Err(AshViewportAttachError::new(error, renderer)),
        };
        unsafe {
            vulkan_viewport::attach_with_adapter(
                renderer,
                context,
                config,
                Arc::new(Sdl3SurfaceAdapter { create_surface }),
            )
        }
        .map(|inner| Self { inner })
    }

    pub fn poll_fault(&self) -> Result<(), AshViewportError> {
        self.inner.poll_fault()
    }

    pub fn cmd_draw(
        &self,
        command_buffer: vk::CommandBuffer,
        frame: RenderedFrame<'_>,
    ) -> Result<Option<TextureRetirementBatch>, AshViewportError> {
        self.inner.cmd_draw(command_buffer, frame)
    }

    pub fn pending_texture_retirement(
        &self,
    ) -> Result<Option<TextureRetirementBatch>, AshViewportError> {
        self.inner.pending_texture_retirement()
    }

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

    pub fn shutdown(&mut self, context: &mut Context) -> Result<(), AshViewportError> {
        self.inner.shutdown(context)
    }

    pub fn retry_retained_cleanup(&mut self) -> Result<(), AshViewportError> {
        self.inner.retry_retained_cleanup()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrong_platform_name_fails_without_claiming_renderer_slots() {
        let _guard = vulkan_viewport::test_context_guard();
        let mut context = Context::create();
        context
            .set_platform_name(Some("dear-imgui-winit 0.16.0".to_string()))
            .unwrap();

        assert!(matches!(
            validate_platform(&context),
            Err(AshViewportError::PlatformBackendMismatch { .. })
        ));
        assert!(context.platform_io().renderer_callbacks_are_empty());
    }

    #[test]
    fn missing_platform_surface_callback_is_transactional() {
        let _guard = vulkan_viewport::test_context_guard();
        let mut context = Context::create();
        context
            .set_platform_name(Some("imgui_impl_sdl3 (3.2.0; 3.2.0)".to_string()))
            .unwrap();

        assert!(matches!(
            validate_platform(&context),
            Err(AshViewportError::PlatformCreateVkSurfaceUnavailable)
        ));
        assert!(context.platform_io().renderer_callbacks_are_empty());
    }
}
