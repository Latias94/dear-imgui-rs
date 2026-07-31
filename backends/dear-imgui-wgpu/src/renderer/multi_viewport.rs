//! Owning Winit/WGPU multi-viewport renderer runtime.

#[cfg(doctest)]
mod removed_free_api_contracts {
    /// ```compile_fail
    /// use dear_imgui_wgpu::multi_viewport::enable;
    /// ```
    struct Enable;

    /// ```compile_fail
    /// use dear_imgui_wgpu::multi_viewport::shutdown_multi_viewport_support;
    /// ```
    struct Shutdown;
}

use dear_imgui_rs::Context;
use dear_imgui_rs::render::{ReconciledFrame, RenderedFrame};

use super::WgpuRenderer;
use super::multi_viewport_runtime::OwningViewportRuntime;
pub use super::multi_viewport_runtime::{
    WgpuViewportAttachError, WgpuViewportError, WgpuViewportFrameTraceGuard,
    WgpuViewportFrameTraceReport,
};
use crate::{ExternalTextureId, GammaMode};
use dear_imgui_winit::multi_viewport::WinitPlatformRuntime;

/// Owning WGPU renderer runtime for the Winit multi-viewport route.
///
/// The runtime consumes the renderer into stable boxed storage, owns the Context renderer
/// attachment and callback claim, and releases all WGPU viewport resources before the Winit
/// platform attachment enters its platform-window teardown phase.
#[derive(Debug)]
pub struct WinitViewportRuntime {
    inner: OwningViewportRuntime,
}

impl WinitViewportRuntime {
    /// Transactionally attaches an initialized renderer to an active Winit platform runtime.
    ///
    /// Failure returns the unchanged renderer through [`WgpuViewportAttachError`]. The renderer
    /// must have been created for `context` with both `WgpuInitInfo::with_instance` and
    /// `WgpuInitInfo::with_adapter`.
    pub fn attach(
        context: &mut Context,
        platform: &WinitPlatformRuntime,
        renderer: WgpuRenderer,
    ) -> Result<Self, WgpuViewportAttachError> {
        if platform.context_id() != context.id() {
            return Err(WgpuViewportAttachError::new(
                WgpuViewportError::PlatformOwnerContextMismatch {
                    expected: context.id(),
                    actual: platform.context_id(),
                },
                renderer,
            ));
        }
        if let Err(error) = platform.validate_renderer_owner(context) {
            return Err(WgpuViewportAttachError::new(
                WgpuViewportError::WinitPlatformOwner(error),
                renderer,
            ));
        }
        OwningViewportRuntime::attach(context, renderer).map(|inner| Self { inner })
    }

    /// Attaches a WGPU renderer to a custom platform that follows the Winit viewport handle
    /// contract.
    ///
    /// # Safety
    ///
    /// The current Context must have a live Winit-compatible platform runtime. Every viewport's
    /// `PlatformHandle` must point to its live `winit::Window`, and the platform must keep those
    /// windows alive until the renderer runtime has released its callbacks and resources. Prefer
    /// [`Self::attach`] for the built-in [`WinitPlatformRuntime`] owner.
    pub unsafe fn attach_unchecked(
        context: &mut Context,
        renderer: WgpuRenderer,
    ) -> Result<Self, WgpuViewportAttachError> {
        OwningViewportRuntime::attach(context, renderer).map(|inner| Self { inner })
    }

    /// Returns and clears the oldest deferred callback or ownership fault.
    pub fn poll_fault(&self) -> Result<(), WgpuViewportError> {
        self.inner.poll_fault()
    }

    /// Begins a non-nestable trace of real secondary-viewport GPU work.
    ///
    /// Finish the returned guard immediately after rendering platform windows and before
    /// acquiring the application's main surface. The report then provides same-scope evidence
    /// for secondary command submission and presentation. Dropping the guard discards its
    /// partial observations.
    pub fn begin_frame_trace(&self) -> Result<WgpuViewportFrameTraceGuard<'_>, WgpuViewportError> {
        self.inner.begin_frame_trace()
    }

    /// Prepares renderer device objects for a new frame.
    pub fn new_frame(&self) -> Result<(), WgpuViewportError> {
        self.inner.new_frame()
    }

    /// Applies managed-texture requests before platform-window rendering.
    pub fn reconcile_frame(&self, frame: &mut RenderedFrame<'_>) -> Result<(), WgpuViewportError> {
        self.inner.reconcile_frame(frame)
    }

    /// Consumes and renders one Context-owned frame.
    pub fn render(
        &self,
        frame: RenderedFrame<'_>,
        render_pass: &mut wgpu::RenderPass<'_>,
    ) -> Result<(), WgpuViewportError> {
        self.inner.render(frame, render_pass)
    }

    /// Renders one frame and returns its reconciliation proof to a presentation owner.
    pub fn render_reconciled<'frame>(
        &self,
        frame: RenderedFrame<'frame>,
        render_pass: &mut wgpu::RenderPass<'_>,
    ) -> Result<ReconciledFrame<'frame>, WgpuViewportError> {
        self.inner.render_reconciled(frame, render_pass)
    }

    /// Finalizes and renders the bound Context's current frame.
    pub fn render_context(
        &self,
        context: &mut Context,
        render_pass: &mut wgpu::RenderPass<'_>,
    ) -> Result<(), WgpuViewportError> {
        self.inner.render_context(context, render_pass)
    }

    /// Consumes and renders one Context-owned frame with explicit framebuffer dimensions.
    pub fn render_with_fb_size(
        &self,
        frame: RenderedFrame<'_>,
        render_pass: &mut wgpu::RenderPass<'_>,
        width: u32,
        height: u32,
    ) -> Result<(), WgpuViewportError> {
        self.inner
            .render_with_fb_size(frame, render_pass, width, height)
    }

    /// Renders one frame at explicit dimensions and returns its reconciliation proof.
    pub fn render_with_fb_size_reconciled<'frame>(
        &self,
        frame: RenderedFrame<'frame>,
        render_pass: &mut wgpu::RenderPass<'_>,
        width: u32,
        height: u32,
    ) -> Result<ReconciledFrame<'frame>, WgpuViewportError> {
        self.inner
            .render_with_fb_size_reconciled(frame, render_pass, width, height)
    }

    /// Finalizes and renders the bound Context with explicit framebuffer dimensions.
    pub fn render_context_with_fb_size(
        &self,
        context: &mut Context,
        render_pass: &mut wgpu::RenderPass<'_>,
        width: u32,
        height: u32,
    ) -> Result<(), WgpuViewportError> {
        self.inner
            .render_context_with_fb_size(context, render_pass, width, height)
    }

    /// Invalidates renderer-owned device objects while preserving external texture handles.
    pub fn invalidate_device_objects(
        &self,
        context: &mut Context,
    ) -> Result<(), WgpuViewportError> {
        self.inner.invalidate_device_objects(context)
    }

    /// Runs a read-only, non-escaping renderer inspection.
    pub fn with_renderer<R>(
        &self,
        callback: impl FnOnce(&WgpuRenderer) -> R,
    ) -> Result<R, WgpuViewportError> {
        self.inner.with_renderer(callback)
    }

    /// Sets the renderer gamma policy.
    pub fn set_gamma_mode(&self, mode: GammaMode) -> Result<(), WgpuViewportError> {
        self.inner.set_gamma_mode(mode)
    }

    /// Sets the clear color used by secondary viewport surfaces.
    pub fn set_viewport_clear_color(&self, color: wgpu::Color) -> Result<(), WgpuViewportError> {
        self.inner.set_viewport_clear_color(color)
    }

    /// Registers an application-owned external WGPU texture.
    pub fn register_external_texture(
        &self,
        view: &wgpu::TextureView,
    ) -> Result<ExternalTextureId, WgpuViewportError> {
        self.inner.register_external_texture(view)
    }

    /// Updates the view of a registered application-owned texture.
    pub fn update_external_texture(
        &self,
        texture: ExternalTextureId,
        view: &wgpu::TextureView,
    ) -> Result<(), WgpuViewportError> {
        self.inner.update_external_texture(texture, view)
    }

    /// Unregisters an application-owned external texture.
    pub fn unregister_external_texture(
        &self,
        texture: ExternalTextureId,
    ) -> Result<(), WgpuViewportError> {
        self.inner.unregister_external_texture(texture)
    }

    /// Explicitly releases renderer callbacks and WGPU resources.
    ///
    /// This operation is idempotent. If managed-texture epochs are still outstanding, it retains
    /// the renderer and attachment so the caller can finish or abandon those epochs and retry.
    /// Platform windows remain owned by `WinitPlatformRuntime` and are released only in its
    /// platform-window phase.
    pub fn shutdown(&mut self, context: &mut Context) -> Result<(), WgpuViewportError> {
        self.inner.shutdown(context)
    }
}
