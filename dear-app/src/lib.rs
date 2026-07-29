//! Winit + WGPU application runtime for `dear-imgui-rs`.
//!
//! The runtime keeps the main window, Dear ImGui context, and user [`Application`] stable while
//! replacing only GPU-owned state after a device-loss signal.
//!
//! ```no_run
//! use dear_app::AppConfig;
//!
//! dear_app::run_ui(AppConfig::default(), |ui| {
//!     ui.window("Hello").build(|| ui.text("Hello, world!"));
//! })?;
//! # Ok::<(), dear_app::RunError>(())
//! ```

mod application;
mod config;
mod runtime;

pub use application::{
    AddOns, Application, DockingApi, EventContext, ExternalTextureError, ExternalTextureHandle,
    FrameContext, GpuApi, GpuContext, GpuGeneration, InitContext, PrepareFrameContext,
    PresentContext, RunError, ShutdownContext,
};
pub use config::{
    AddOnsConfig, AppConfig, DockingConfig, RedrawMode, Theme, WgpuConfig, WgpuPreset,
};
pub use dear_imgui_rs as imgui;
pub use wgpu;

/// Runs one persistent application until the event loop exits.
pub fn run<A: Application + 'static>(config: AppConfig, application: A) -> Result<(), RunError> {
    runtime::run(config, application)
}

/// Runs an application whose persistent state is captured by one UI closure.
///
/// This is the smallest entry point for applications that only build UI. Use [`run`] with an
/// [`Application`] implementation when initialization, events, GPU resources, or teardown hooks
/// are required. Both entry points use the same runtime and recovery state machine.
pub fn run_ui<F>(config: AppConfig, ui: F) -> Result<(), RunError>
where
    F: FnMut(&imgui::Ui) + 'static,
{
    struct UiApplication<F> {
        ui: F,
    }

    impl<F> Application for UiApplication<F>
    where
        F: FnMut(&imgui::Ui) + 'static,
    {
        fn frame(&mut self, context: &mut FrameContext<'_>) -> Result<(), RunError> {
            (self.ui)(context.ui());
            Ok(())
        }
    }

    run(config, UiApplication { ui })
}
