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

use std::{convert::Infallible, error::Error, marker::PhantomData};

mod application;
mod config;
mod runtime;

pub use application::{
    AddOns, Application, ApplicationStage, DockingApi, EventContext, ExternalTextureError,
    ExternalTextureHandle, FrameContext, GpuApi, GpuContext, GpuGeneration, InitContext,
    InitializedContext, PrepareFrameContext, RunError, ShutdownContext,
};
pub use config::{
    AddOnsConfig, AppConfig, DockingConfig, RedrawMode, Theme, WgpuConfig, WgpuPreset,
};
pub use dear_imgui_rs as imgui;
#[cfg(feature = "test-engine")]
pub use dear_imgui_test_engine as test_engine;
pub use wgpu;

/// Runs one persistent application until the event loop exits.
pub fn run<A: Application + 'static>(config: AppConfig, application: A) -> Result<(), RunError> {
    runtime::run(config, application)
}

/// Runs an application with one fallible, exit-capable frame closure.
///
/// The closure value is retained by the runtime for the complete event-loop lifetime, so captured
/// state persists between frames and across GPU generation recovery. [`FrameContext`] exposes the
/// current UI, compiled add-ons, GPU generation, and [`FrameContext::request_exit`]. Returning an
/// error preserves its [`Error::source`] chain and attributes it to [`ApplicationStage::Frame`].
/// Requesting exit is a normal control signal and returns `Ok(())` after exactly-once shutdown.
///
/// Use [`run_ui`] when the callback only needs [`imgui::Ui`]. Use [`run`] when initialization,
/// event, GPU recovery, or shutdown hooks are required.
pub fn run_frame<F, E>(config: AppConfig, frame: F) -> Result<(), RunError>
where
    F: for<'frame> FnMut(&mut FrameContext<'frame>) -> Result<(), E> + 'static,
    E: Error + 'static,
{
    run(config, FrameApplication::<F, E>::new(frame))
}

struct FrameApplication<F, E> {
    frame: F,
    _error: PhantomData<fn() -> E>,
}

impl<F, E> FrameApplication<F, E>
where
    E: Error + 'static,
{
    fn new(frame: F) -> Self {
        Self {
            frame,
            _error: PhantomData,
        }
    }

    fn invoke<C>(&mut self, context: &mut C) -> Result<(), RunError>
    where
        F: FnMut(&mut C) -> Result<(), E>,
    {
        (self.frame)(context)
            .map_err(|source| RunError::application(ApplicationStage::Frame, source))
    }
}

impl<F, E> Application for FrameApplication<F, E>
where
    F: for<'frame> FnMut(&mut FrameContext<'frame>) -> Result<(), E> + 'static,
    E: Error + 'static,
{
    fn frame(&mut self, context: &mut FrameContext<'_>) -> Result<(), RunError> {
        self.invoke(context)
    }
}

/// Runs an application whose persistent state is captured by one UI closure.
///
/// This is the smallest entry point for applications that only build UI. Move to [`run_frame`]
/// when the closure needs fallibility, exit control, add-ons, or the current GPU generation. Use
/// [`run`] with an [`Application`] implementation when initialization, events, GPU recovery, or
/// teardown hooks are required. All three entries use the same runtime and recovery state machine.
pub fn run_ui<F>(config: AppConfig, mut ui: F) -> Result<(), RunError>
where
    F: FnMut(&imgui::Ui) + 'static,
{
    run_frame(config, move |context| {
        (ui)(context.ui());
        Ok::<(), Infallible>(())
    })
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, error::Error as _, rc::Rc};

    use thiserror::Error;

    use super::{ApplicationStage, FrameApplication, RunError};

    #[derive(Debug, Error)]
    #[error("injected frame failure")]
    struct FrameFailure;

    #[derive(Default)]
    struct ProbeFrameContext {
        exit_requested: bool,
    }

    #[test]
    fn frame_application_retains_closure_state_between_calls() {
        let calls = Rc::new(Cell::new(0));
        let observed_calls = Rc::clone(&calls);
        let mut application = FrameApplication::<_, FrameFailure>::new(
            move |value: &mut usize| -> Result<(), FrameFailure> {
                let call = observed_calls.get() + 1;
                observed_calls.set(call);
                *value += call;
                Ok(())
            },
        );
        let mut value = 0;

        application.invoke(&mut value).unwrap();
        application.invoke(&mut value).unwrap();

        assert_eq!(calls.get(), 2);
        assert_eq!(value, 3);
    }

    #[test]
    fn frame_application_preserves_user_error_as_the_stage_source() {
        let mut application =
            FrameApplication::<_, FrameFailure>::new(|_context: &mut ()| Err(FrameFailure));

        let mut context = ();
        let error = application.invoke(&mut context).unwrap_err();

        assert!(matches!(
            &error,
            RunError::Application {
                stage: ApplicationStage::Frame,
                ..
            }
        ));
        assert!(
            error
                .source()
                .and_then(|source| source.downcast_ref::<FrameFailure>())
                .is_some()
        );
    }

    #[test]
    fn exit_request_is_successful_control_flow() {
        let mut application =
            FrameApplication::<_, FrameFailure>::new(|context: &mut ProbeFrameContext| {
                context.exit_requested = true;
                Ok(())
            });
        let mut context = ProbeFrameContext::default();

        assert!(application.invoke(&mut context).is_ok());
        assert!(context.exit_requested);
    }

    #[test]
    fn frame_error_remains_primary_after_an_exit_request() {
        let mut application =
            FrameApplication::<_, FrameFailure>::new(|context: &mut ProbeFrameContext| {
                context.exit_requested = true;
                Err(FrameFailure)
            });
        let mut context = ProbeFrameContext::default();

        let error = application.invoke(&mut context).unwrap_err();

        assert!(context.exit_requested);
        assert_eq!(error.application_stage(), Some(ApplicationStage::Frame));
    }
}
