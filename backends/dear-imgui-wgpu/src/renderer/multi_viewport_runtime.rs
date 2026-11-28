//! Shared owning WGPU multi-viewport renderer runtime.

mod callbacks;
mod platform;
mod registry;
mod runtime;
mod surface;
mod trace;
mod work;

#[cfg(test)]
mod tests;

pub(crate) use platform::ViewportPlatform;
pub(crate) use runtime::{
    OwningViewportRuntime, PreparationAttempt, PreparedMainFrame, finish_route_preparation,
    prepare_route_for_context,
};
pub use runtime::{
    WgpuPreparedMainFrame, WgpuPreparedViewportFrame, WgpuRecordedViewportFrame,
    WgpuViewportAttachError, WgpuViewportError, WgpuViewportRouteError, WgpuViewportRouteFault,
};
pub use trace::WgpuViewportFrameReport;
pub use work::{WgpuPendingPresentations, WgpuRecordedViewportWork, WgpuViewportWorkError};
