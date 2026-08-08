//! Owning multi-viewport runtime for the Glow renderer.

#[cfg(doctest)]
mod prepared_route_contracts {
    /// Prepared frames carry their Context identity without exposing drawable data.
    ///
    /// ```
    /// use dear_imgui_glow::multi_viewport::GlowPreparedViewportFrame;
    /// use dear_imgui_rs::ContextId;
    ///
    /// fn prepared_context(frame: &GlowPreparedViewportFrame<'_>) -> ContextId {
    ///     frame.context_id()
    /// }
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_glow::multi_viewport::GlowPreparedViewportFrame;
    /// fn draw_too_early(frame: &GlowPreparedViewportFrame<'_>) {
    ///     let _ = frame.draw_data();
    /// }
    /// ```
    ///
    /// Manual tracing and partially composed render routes are intentionally unavailable.
    ///
    /// ```compile_fail
    /// use dear_imgui_glow::multi_viewport::GlowViewportFrameTrace;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_glow::multi_viewport::GlowViewportRuntime;
    /// let _ = GlowViewportRuntime::begin_frame_trace;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_glow::multi_viewport::GlowViewportRuntime;
    /// let _ = GlowViewportRuntime::render_context_with_platform_windows;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_glow::multi_viewport::GlowViewportRuntime;
    /// let _ = GlowViewportRuntime::with_renderer;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_glow::multi_viewport::GlowViewportRuntime;
    /// let _ = GlowViewportRuntime::poll_fault;
    /// ```
    struct PreparedRoute;
}

mod callbacks;
mod registry;
mod runtime;

pub(super) use self::callbacks::renderer_render_window_sys;
pub use self::runtime::{
    GlowPreparedViewportFrame, GlowRenderedViewportFrame, GlowViewportAttachError,
    GlowViewportError, GlowViewportFrameReport, GlowViewportRouteError, GlowViewportRouteFault,
    GlowViewportRuntime,
};

#[cfg(test)]
mod tests;
