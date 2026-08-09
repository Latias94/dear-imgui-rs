//! Owning SDL3/WGPU multi-viewport renderer route.

#[cfg(doctest)]
mod removed_free_api_contracts {
    /// ```compile_fail
    /// use dear_imgui_wgpu::multi_viewport_sdl3::enable;
    /// ```
    struct Enable;

    /// ```compile_fail
    /// use dear_imgui_wgpu::multi_viewport_sdl3::shutdown_multi_viewport_support;
    /// ```
    struct Shutdown;

    /// ```compile_fail
    /// use dear_imgui_wgpu::multi_viewport_sdl3::Sdl3ViewportRuntime;
    /// let _ = Sdl3ViewportRuntime::attach;
    /// ```
    struct RemovedRuntime;

    /// The prepared capability carries the report from the same secondary-viewport scope.
    ///
    /// ```
    /// use dear_imgui_wgpu::multi_viewport_sdl3::{
    ///     WgpuPreparedViewportFrame, WgpuViewportFrameReport,
    /// };
    ///
    /// fn secondary_report<'a>(
    ///     frame: &'a WgpuPreparedViewportFrame<'_>,
    /// ) -> &'a WgpuViewportFrameReport {
    ///     frame.secondary_report()
    /// }
    /// ```
    ///
    /// Manual tracing and partially prepared frames are intentionally unavailable.
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::multi_viewport_sdl3::WgpuViewportFrameTraceGuard;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::multi_viewport_sdl3::Sdl3ViewportRoute;
    /// let _ = Sdl3ViewportRoute::begin_frame_trace;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::multi_viewport_sdl3::Sdl3ViewportRoute;
    /// let _ = Sdl3ViewportRoute::prepare_context;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::multi_viewport_sdl3::Sdl3ViewportRoute;
    /// let _ = Sdl3ViewportRoute::prepare_frame;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::multi_viewport_sdl3::Sdl3ViewportRoute;
    /// let _ = Sdl3ViewportRoute::attach_unchecked;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::multi_viewport_sdl3::Sdl3ViewportRoute;
    /// let _ = Sdl3ViewportRoute::poll_fault;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::multi_viewport_sdl3::Sdl3ViewportRoute;
    /// let _ = Sdl3ViewportRoute::prepare_frame_unchecked;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::multi_viewport_sdl3::Sdl3ViewportRoute;
    /// let _ = Sdl3ViewportRoute::with_renderer;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_rs::render::ReconciledFrame;
    /// use dear_imgui_wgpu::{FramebufferExtent, multi_viewport_sdl3::Sdl3ViewportRoute, wgpu};
    ///
    /// fn bypass_preparation(
    ///     route: &Sdl3ViewportRoute,
    ///     frame: ReconciledFrame<'_>,
    ///     render_pass: &mut wgpu::RenderPass<'_>,
    ///     extent: FramebufferExtent,
    /// ) {
    ///     route.render_main(frame, render_pass, extent).unwrap();
    /// }
    /// ```
    struct PreparedTransaction;

    /// The old manual-trace report name is intentionally unavailable.
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::multi_viewport_sdl3::WgpuViewportFrameTraceReport;
    /// ```
    struct RemovedTraceReportName;
}

use dear_imgui_rs::{Context, FrameToken};

use super::WgpuRenderer;
use super::multi_viewport_runtime::{
    OwningViewportRuntime, finish_route_preparation, prepare_route_for_context,
};
pub use super::multi_viewport_runtime::{
    WgpuPreparedViewportFrame, WgpuViewportAttachError, WgpuViewportError, WgpuViewportFrameReport,
};
use super::multi_viewport_runtime::{WgpuViewportRouteError, WgpuViewportRouteFault};
use crate::{ExternalTextureId, FramebufferExtent, GammaMode};
use dear_imgui_sdl3::{Sdl3BackendError, Sdl3PlatformBackend, Sdl3ViewportRendererAdapter};

/// One failure observed while preparing an SDL3/WGPU multi-viewport route.
pub type Sdl3ViewportRouteFault = WgpuViewportRouteFault<Sdl3BackendError>;

/// Ordered failures from one SDL3/WGPU multi-viewport preparation transaction.
pub type Sdl3ViewportRouteError = WgpuViewportRouteError<Sdl3BackendError>;

/// Owning WGPU renderer route for SDL3 multi-viewport applications.
///
/// The route consumes the renderer into stable boxed storage, captures the exact live SDL3
/// platform generation, owns the Context renderer attachment and callback claim, and releases all
/// WGPU viewport resources before the SDL3 platform attachment enters its platform-window
/// teardown phase.
#[derive(Debug)]
pub struct Sdl3ViewportRoute {
    inner: OwningViewportRuntime,
    platform: Sdl3ViewportRendererAdapter,
}

impl Sdl3ViewportRoute {
    /// Transactionally attaches an initialized renderer to an active SDL3 platform runtime.
    ///
    /// Failure returns the unchanged renderer through [`WgpuViewportAttachError`]. The renderer
    /// must have been created for `context` with both `WgpuInitInfo::with_instance` and
    /// `WgpuInitInfo::with_adapter`.
    pub fn attach(
        context: &mut Context,
        platform: &Sdl3PlatformBackend,
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
        let platform = match platform.viewport_renderer_adapter(context) {
            Ok(platform) => platform,
            Err(error) => {
                return Err(WgpuViewportAttachError::new(
                    WgpuViewportError::Sdl3PlatformOwner(error),
                    renderer,
                ));
            }
        };
        OwningViewportRuntime::attach(context, renderer).map(|inner| Self { inner, platform })
    }

    /// Consumes an open frame, reconciles textures, and completes secondary viewports.
    ///
    /// Call this before acquiring the application's main surface. A temporary main-surface
    /// failure may discard the returned capability without losing secondary completion. A frame
    /// from another Context is rejected before the SDL3 adapter or either deferred-fault queue is
    /// entered.
    pub fn prepare<'frame>(
        &self,
        frame: FrameToken<'frame>,
    ) -> Result<WgpuPreparedViewportFrame<'frame>, Sdl3ViewportRouteError> {
        let actual = frame.ui().context_id();
        prepare_route_for_context(self.inner.context_id(), actual, || {
            self.prepare_with_platform(|| self.inner.prepare_frame(frame))
        })
    }

    /// Renders the main viewport after secondary work completed during preparation.
    pub fn render_main(
        &self,
        frame: WgpuPreparedViewportFrame<'_>,
        render_pass: &mut wgpu::RenderPass<'_>,
        framebuffer_extent: FramebufferExtent,
    ) -> Result<(), WgpuViewportError> {
        self.inner
            .render_main(frame, render_pass, framebuffer_extent)
    }

    fn prepare_with_platform<'frame>(
        &self,
        prepare: impl FnOnce() -> Result<WgpuPreparedViewportFrame<'frame>, WgpuViewportError>,
    ) -> Result<WgpuPreparedViewportFrame<'frame>, Sdl3ViewportRouteError> {
        debug_assert_eq!(self.platform.context_id(), self.inner.context_id());

        let (renderer_result, platform_faults) = self.platform.run(prepare).into_parts();
        finish_route_preparation(renderer_result, self.inner.drain_faults(), platform_faults)
    }

    /// Invalidates renderer-owned device objects while preserving external texture handles.
    pub fn invalidate_device_objects(
        &self,
        context: &mut Context,
    ) -> Result<(), WgpuViewportError> {
        self.inner.invalidate_device_objects(context)
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
    /// Platform windows remain owned by `Sdl3PlatformBackend` and are released only in its
    /// platform-window phase.
    pub fn shutdown(&mut self, context: &mut Context) -> Result<(), WgpuViewportError> {
        self.inner.shutdown(context)
    }
}
