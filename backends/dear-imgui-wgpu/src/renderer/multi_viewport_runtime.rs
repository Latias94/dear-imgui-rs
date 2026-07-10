//! Shared WGPU multi-viewport renderer runtime.

mod callbacks;
mod registry;
mod surface;

#[cfg(test)]
mod tests;

use super::WgpuRenderer;
use dear_imgui_rs::Context;

#[cfg(all(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
compile_error!(
    "Features `multi-viewport-winit` and `multi-viewport-sdl3` are mutually exclusive; enable only one."
);

#[cfg(all(not(feature = "multi-viewport-winit"), feature = "multi-viewport-sdl3"))]
use super::multi_viewport_sdl3_adapter as platform_adapter;
#[cfg(feature = "multi-viewport-winit")]
use super::multi_viewport_winit_adapter as platform_adapter;

/// Failure to enable WGPU multi-viewport renderer callbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CallbackOwnershipError {
    /// Another renderer backend still owns at least one `Renderer_*` callback slot.
    #[error("ImGuiPlatformIO renderer callbacks are already owned by another backend")]
    RendererCallbacksOccupied,
    /// An active runtime no longer owns the complete callback table required for teardown.
    #[error("WGPU renderer callbacks were replaced while multi-viewport remained active")]
    RendererCallbacksReplaced,
    /// A secondary viewport already contains renderer-owned user data.
    #[error("a secondary viewport already has RendererUserData owned by another backend")]
    RendererUserDataOccupied,
    /// Existing platform windows would not receive the newly installed create callback.
    #[error(
        "secondary platform windows already exist; destroy them before enabling WGPU multi-viewport"
    )]
    PlatformWindowsAlreadyCreated,
    /// The current Dear ImGui artifact cannot safely bridge the aggregate size callback.
    #[error("dear-imgui-sys was built without PlatformIO aggregate ABI hooks")]
    AggregateCallbackHooksUnavailable,
    /// The renderer has not been initialized with GPU backend data.
    #[error("WGPU renderer is not initialized")]
    RendererNotInitialized,
    /// Per-window surfaces require the WGPU instance used by the renderer.
    #[error("WGPU multi-viewport requires WgpuInitInfo::with_instance")]
    MissingInstance,
    /// Surface capability negotiation requires the WGPU adapter used by the renderer.
    #[error("WGPU multi-viewport requires WgpuInitInfo::with_adapter")]
    MissingAdapter,
    /// Renderer callbacks require a platform backend that already supports viewports.
    #[error("the active platform backend does not advertise multi-viewport support")]
    PlatformBackendUnavailable,
    /// A callback is currently using the renderer registered for this context.
    #[error("cannot replace the WGPU renderer while a viewport callback is active")]
    RendererCallbackActive,
    /// Live secondary viewports still hold surfaces created from the previous renderer.
    #[error("cannot replace the WGPU renderer while live viewport surfaces exist")]
    LiveViewportRendererRebind,
    /// This context already has an active WGPU multi-viewport registration.
    #[error(
        "WGPU multi-viewport is already enabled for this context; shut it down before enabling again"
    )]
    AlreadyEnabled,
    /// One renderer instance cannot back multiple ImGui contexts.
    #[error("this WGPU renderer is already registered with another ImGui context")]
    RendererAlreadyRegistered,
    /// Native multi-viewport surfaces are unavailable on this target.
    #[error("WGPU native multi-viewport rendering is unavailable on this target")]
    UnsupportedTarget,
}

/// Enables WGPU multi-viewport renderer callbacks.
///
/// # Safety
///
/// `renderer` must remain at the same address until [`shutdown_multi_viewport_support`] completes.
/// All callbacks and renderer access must occur on the enabling thread. While enabled, callers must
/// not concurrently access, reinitialize, shut down, move, or drop the renderer, or replace any
/// viewport's `RendererUserData`.
pub unsafe fn enable(
    renderer: &mut WgpuRenderer,
    context: &mut Context,
) -> Result<(), CallbackOwnershipError> {
    unsafe { callbacks::enable(renderer, context) }
}

pub(crate) fn clear_for_drop(renderer: *mut WgpuRenderer) {
    registry::remove_renderer_state_for_renderer(renderer);
}

/// Destroys secondary platform windows before releasing renderer state and callbacks.
///
/// This operation is transactional with respect to callback ownership. If another backend has
/// replaced any renderer callback, no platform windows or runtime state are changed.
pub fn shutdown_multi_viewport_support(
    context: &mut Context,
) -> Result<(), CallbackOwnershipError> {
    callbacks::shutdown_multi_viewport_support(context)
}
