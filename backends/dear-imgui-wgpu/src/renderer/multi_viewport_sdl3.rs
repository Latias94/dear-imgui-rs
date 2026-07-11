//! SDL3 adapter entry points for WGPU multi-viewport rendering.

pub use super::multi_viewport_runtime::CallbackOwnershipError;
use super::{WgpuRenderer, multi_viewport_runtime};
use dear_imgui_rs::Context;

/// Enables WGPU renderer callbacks for SDL3-created platform windows.
/// Call this after initializing SDL3 viewport support and before creating secondary platform
/// windows.
///
/// # Safety
///
/// `renderer` must remain at the same address until [`shutdown_multi_viewport_support`] returns.
/// Storing the renderer in a `Box` is the simplest way to uphold this requirement. The
/// active platform backend must be SDL3, and each viewport platform handle must identify a live
/// SDL window until the renderer destroy callback completes. While enabled, all callbacks and
/// renderer access must stay on one thread; the renderer must not be concurrently accessed,
/// reinitialized, shut down, moved, or dropped, and viewport `RendererUserData` must not be
/// replaced. Both the context and renderer must remain alive. Call
/// [`shutdown_multi_viewport_support`] before either one is dropped.
pub unsafe fn enable(
    renderer: &mut WgpuRenderer,
    imgui_context: &mut Context,
) -> Result<(), CallbackOwnershipError> {
    unsafe { multi_viewport_runtime::enable(renderer, imgui_context) }
}

/// Destroys platform windows before disabling WGPU renderer callbacks.
pub fn shutdown_multi_viewport_support(
    imgui_context: &mut Context,
) -> Result<(), CallbackOwnershipError> {
    multi_viewport_runtime::shutdown_multi_viewport_support(imgui_context)
}

pub(crate) fn clear_for_drop(renderer: *mut WgpuRenderer) {
    multi_viewport_runtime::clear_for_drop(renderer);
}
