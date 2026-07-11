//! Winit + WGPU application runtime for `dear-imgui-rs`.
//!
//! The runtime keeps the main window, Dear ImGui context, and user [`Application`] stable while
//! replacing only GPU-owned state after a device-loss signal.
//!
//! ```no_run
//! use dear_app::{AppConfig, Application, FrameContext, RunError};
//!
//! struct Hello;
//!
//! impl Application for Hello {
//!     fn frame(&mut self, context: &mut FrameContext<'_, '_>) -> Result<(), RunError> {
//!         context.ui().window("Hello").build(|| context.ui().text("Hello, world!"));
//!         Ok(())
//!     }
//! }
//!
//! dear_app::run(AppConfig::default(), Hello)?;
//! # Ok::<(), RunError>(())
//! ```

mod application;
mod config;
mod runtime;

pub use application::{
    AddOns, Application, DockingApi, EventContext, ExternalTextureError, ExternalTextureHandle,
    FrameContext, GpuApi, GpuContext, GpuGeneration, InitContext, RunError, ShutdownContext,
};
pub use config::{
    AddOnsConfig, AppConfig, DockingConfig, RedrawMode, Theme, WgpuConfig, WgpuPreset,
};
pub use wgpu;

/// Runs one persistent application until the event loop exits.
pub fn run<A: Application + 'static>(config: AppConfig, application: A) -> Result<(), RunError> {
    runtime::run(config, application)
}
