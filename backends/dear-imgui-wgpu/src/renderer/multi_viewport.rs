//! Owning Winit/WGPU multi-viewport renderer route.

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

    /// ```compile_fail
    /// use dear_imgui_wgpu::multi_viewport::WinitViewportRuntime;
    /// let _ = WinitViewportRuntime::attach;
    /// ```
    struct RemovedRuntime;

    /// The prepared capability carries the report from the same secondary-viewport scope.
    ///
    /// ```
    /// use dear_imgui_wgpu::multi_viewport::{
    ///     WgpuPreparedMainFrame, WgpuViewportFrameReport,
    /// };
    ///
    /// fn secondary_report<'a>(
    ///     frame: &'a WgpuPreparedMainFrame<'_>,
    /// ) -> &'a WgpuViewportFrameReport {
    ///     frame.secondary_report()
    /// }
    /// ```
    ///
    /// Manual tracing and partially prepared frames are intentionally unavailable.
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::multi_viewport::WgpuViewportFrameTraceGuard;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::multi_viewport::WinitViewportRoute;
    /// let _ = WinitViewportRoute::begin_frame_trace;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::multi_viewport::WinitViewportRoute;
    /// let _ = WinitViewportRoute::prepare_context;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::multi_viewport::WinitViewportRoute;
    /// let _ = WinitViewportRoute::prepare_frame;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::multi_viewport::WinitViewportRoute;
    /// let _ = WinitViewportRoute::attach_unchecked;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::multi_viewport::WinitViewportRoute;
    /// let _ = WinitViewportRoute::poll_fault;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::multi_viewport::WinitViewportRoute;
    /// let _ = WinitViewportRoute::prepare_frame_unchecked;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::multi_viewport::WinitViewportRoute;
    /// let _ = WinitViewportRoute::with_renderer;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_rs::render::ReconciledFrame;
    /// use dear_imgui_wgpu::{FramebufferExtent, multi_viewport::WinitViewportRoute, wgpu};
    ///
    /// fn bypass_preparation(
    ///     route: &WinitViewportRoute,
    ///     frame: ReconciledFrame<'_>,
    ///     render_pass: &mut wgpu::RenderPass<'_>,
    ///     extent: FramebufferExtent,
    /// ) {
    ///     route.record_main(frame, render_pass, extent).unwrap();
    /// }
    /// ```
    struct PreparedTransaction;

    /// The old manual-trace report name is intentionally unavailable.
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::multi_viewport::WgpuViewportFrameTraceReport;
    /// ```
    struct RemovedTraceReportName;
}

use dear_imgui_rs::{Context, FrameToken};

use super::WgpuRenderer;
use super::multi_viewport_runtime::{
    OwningViewportRuntime, PreparationAttempt, PreparedMainFrame, finish_route_preparation,
    prepare_route_for_context,
};
pub use super::multi_viewport_runtime::{
    WgpuPendingPresentations, WgpuPreparedMainFrame, WgpuPreparedViewportFrame,
    WgpuRecordedViewportFrame, WgpuRecordedViewportWork, WgpuViewportAttachError,
    WgpuViewportError, WgpuViewportFrameReport, WgpuViewportWorkError,
};
use super::multi_viewport_runtime::{WgpuViewportRouteError, WgpuViewportRouteFault};
use crate::{
    ExternalTextureBinding, ExternalTextureId, FramebufferExtent, GammaMode,
    WgpuViewportObservation,
};
use dear_imgui_winit::{
    WinitPlatform, WinitPlatformError, multi_viewport::WinitViewportRendererAdapter,
};
use winit::event_loop::ActiveEventLoop;

/// One failure observed while preparing a Winit/WGPU multi-viewport route.
pub type WinitViewportRouteFault = WgpuViewportRouteFault<WinitPlatformError>;

/// Ordered failures from one Winit/WGPU multi-viewport preparation transaction.
pub type WinitViewportRouteError = WgpuViewportRouteError<WinitPlatformError>;

/// Owning WGPU renderer route for Winit multi-viewport applications.
///
/// The route consumes the renderer into stable boxed storage, captures the exact live Winit
/// platform generation, owns the Context renderer attachment and callback claim, and releases all
/// WGPU viewport resources before the Winit platform attachment enters its platform-window
/// teardown phase.
#[derive(Debug)]
pub struct WinitViewportRoute {
    inner: OwningViewportRuntime,
    platform: WinitViewportRendererAdapter,
}

impl WinitViewportRoute {
    /// Transactionally attaches an initialized renderer to an active Winit platform runtime.
    ///
    /// Failure returns the unchanged renderer through [`WgpuViewportAttachError`]. The renderer
    /// must have been created for `context` with both `WgpuInitInfo::with_instance` and
    /// `WgpuInitInfo::with_adapter`.
    pub fn attach(
        context: &mut Context,
        platform: &WinitPlatform,
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
                    WgpuViewportError::WinitPlatformOwner(error),
                    renderer,
                ));
            }
        };
        OwningViewportRuntime::attach(context, renderer, platform.clone())
            .map(|inner| Self { inner, platform })
    }

    /// Reconciles an open UI frame and updates native windows without recording, submitting, or presenting.
    ///
    /// The platform owner lends the active event loop only for native-window creation and geometry
    /// updates. No surface is acquired and no GPU work is recorded, submitted, or presented here.
    /// Renderer and platform callback faults are returned together. A frame from another
    /// Context is rejected before the event-loop adapter or either deferred-fault queue is entered.
    pub fn prepare<'frame>(
        &self,
        event_loop: &ActiveEventLoop,
        frame: FrameToken<'frame>,
    ) -> Result<WgpuPreparedViewportFrame<'frame>, WinitViewportRouteError> {
        let actual = frame.ui().context_id();
        prepare_route_for_context(self.inner.context_id(), actual, || {
            self.prepare_with_platform(event_loop, || self.inner.prepare_frame(frame))
        })
    }

    /// Acquires and records secondary targets without submitting or presenting them.
    pub fn record<'frame>(
        &self,
        frame: WgpuPreparedViewportFrame<'frame>,
    ) -> Result<WgpuRecordedViewportFrame<'frame>, WgpuViewportWorkError> {
        if frame.context_id() != self.inner.context_id() {
            // Preserve the same side-effect-free foreign-Context gate used by prepare.
            return self.inner.record(frame);
        }
        let (attempt, faults) = self.platform.run(|| self.inner.record(frame)).into_parts();
        self.inner.finish_recording_attempt(
            attempt,
            faults
                .into_iter()
                .map(WgpuViewportError::WinitPlatformOwner)
                .collect(),
        )
    }

    /// Runs the explicit record/submit/present pipeline and retains its secondary report.
    ///
    /// Engine producer buffers are submitted before UI consumer buffers on the renderer queue.
    pub fn complete_secondary<'frame>(
        &self,
        frame: WgpuPreparedViewportFrame<'frame>,
        producer_commands: impl IntoIterator<Item = wgpu::CommandBuffer>,
    ) -> Result<WgpuPreparedMainFrame<'frame>, WgpuViewportWorkError> {
        self.record(frame)?.complete_secondary(producer_commands)
    }

    /// Records the main viewport into an application-owned pass without claiming submission.
    pub fn record_main(
        &self,
        frame: WgpuPreparedMainFrame<'_>,
        render_pass: &mut wgpu::RenderPass<'_>,
        framebuffer_extent: FramebufferExtent,
    ) -> Result<WgpuViewportObservation, WgpuViewportWorkError> {
        self.inner
            .record_main(frame, render_pass, framebuffer_extent)
    }

    fn prepare_with_platform<'frame>(
        &self,
        event_loop: &ActiveEventLoop,
        prepare: impl FnOnce() -> PreparationAttempt<PreparedMainFrame<'frame>>,
    ) -> Result<WgpuPreparedViewportFrame<'frame>, WinitViewportRouteError> {
        debug_assert_eq!(self.platform.context_id(), self.inner.context_id());

        let (renderer_result, platform_faults) = self
            .platform
            .with_event_loop(event_loop, |_| prepare())
            .into_parts();
        finish_route_preparation(renderer_result, self.inner.drain_faults(), platform_faults)
            .map(|(prepared, report)| WgpuPreparedViewportFrame::new(prepared, report))
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

    /// Captures the current external view-mapping revision without borrowing the renderer.
    pub fn external_texture_binding(
        &self,
        texture: ExternalTextureId,
    ) -> Result<ExternalTextureBinding, WgpuViewportError> {
        self.inner.external_texture_binding(texture)
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
    /// Platform windows remain owned by `WinitPlatform` and are released only in its
    /// platform-window phase.
    pub fn shutdown(&mut self, context: &mut Context) -> Result<(), WgpuViewportError> {
        self.inner.shutdown(context)
    }
}
