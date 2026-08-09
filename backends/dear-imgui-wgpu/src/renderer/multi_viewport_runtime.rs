//! Shared owning WGPU multi-viewport renderer runtime.

mod callbacks;
mod registry;
mod runtime;
mod surface;
mod trace;

#[cfg(test)]
mod tests;

pub(crate) use runtime::{
    OwningViewportRuntime, finish_route_preparation, prepare_route_for_context,
};
pub use runtime::{
    WgpuPreparedViewportFrame, WgpuViewportAttachError, WgpuViewportError, WgpuViewportRouteError,
    WgpuViewportRouteFault,
};
pub use trace::WgpuViewportFrameReport;

#[cfg(all(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
compile_error!(
    "Features `multi-viewport-winit` and `multi-viewport-sdl3` are mutually exclusive; enable only one."
);

#[cfg(all(not(feature = "multi-viewport-winit"), feature = "multi-viewport-sdl3"))]
use super::multi_viewport_sdl3_adapter as platform_adapter;
#[cfg(feature = "multi-viewport-winit")]
use super::multi_viewport_winit_adapter as platform_adapter;
