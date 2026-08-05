//! Runtime initialization rollback and exactly-once shutdown coordination.

use super::state::{UiState, WindowState};
use crate::{Application, RunError, ShutdownContext};

pub(super) fn abort_runtime_initialization<A: Application>(
    application: &mut A,
    mut ui: UiState,
    window: WindowState,
    primary_error: RunError,
) -> RunError {
    super::state::preserve_initialization_error(primary_error, move || {
        let application_result = {
            let mut shutdown = ShutdownContext {
                imgui: &mut ui.context,
                window: &window.window,
                generation: None,
            };
            application.shutdown(&mut shutdown)
        };
        let platform_result = ui.release_platform_then_teardown_or_quarantine();
        drop(window);
        application_result.and(platform_result)
    })
}

#[derive(Default)]
pub(super) struct RuntimeShutdownErrors {
    pub(super) terminal_error: Option<RunError>,
    pub(super) shutdown_error: Option<RunError>,
}

pub(super) fn finish_runtime_shutdown(
    terminal_error: Option<RunError>,
    application_shutdown: impl FnOnce() -> Option<RunError>,
    release_backends: impl FnOnce() -> Result<(), RunError>,
) -> RuntimeShutdownErrors {
    // User-owned resources may still need the Context, but renderer and platform teardown must
    // proceed even when the hook reports an error. Backend release owns the Context fail-stop
    // decision and quarantines the complete graph if it cannot commit.
    let shutdown_error = application_shutdown();
    let release_result = release_backends();
    RuntimeShutdownErrors {
        terminal_error: terminal_error.or_else(|| release_result.err()),
        shutdown_error,
    }
}

#[derive(Default)]
pub(super) struct ShutdownCoordinator {
    started: bool,
    terminal_error: Option<RunError>,
    shutdown_error: Option<RunError>,
}

impl ShutdownCoordinator {
    pub(super) const fn started(&self) -> bool {
        self.started
    }

    pub(super) fn remember_error(&mut self, error: RunError) {
        if self.terminal_error.is_none() {
            self.terminal_error = Some(error);
        }
    }

    pub(super) fn shutdown_once<R, A>(
        &mut self,
        runtime: &mut Option<R>,
        application: &mut A,
        shutdown: impl FnOnce(R, &mut A) -> RuntimeShutdownErrors,
    ) {
        if self.started {
            return;
        }
        self.started = true;
        let errors = runtime
            .take()
            .map(|runtime| shutdown(runtime, application))
            .unwrap_or_default();
        if let Some(error) = errors.terminal_error {
            self.remember_error(error);
        }
        if self.shutdown_error.is_none() {
            self.shutdown_error = errors.shutdown_error;
        }
    }

    pub(super) fn take_terminal_error(&mut self) -> Option<RunError> {
        self.terminal_error.take()
    }

    pub(super) fn take_shutdown_error(&mut self) -> Option<RunError> {
        self.shutdown_error.take()
    }
}

pub(super) fn resolve_run_result(
    terminal_before_shutdown: Option<RunError>,
    event_loop_result: Result<(), winit::error::EventLoopError>,
    shutdown_error: Option<RunError>,
) -> Result<(), RunError> {
    if let Some(error) = terminal_before_shutdown {
        return Err(error);
    }
    if let Err(error) = event_loop_result {
        return Err(error.into());
    }
    match shutdown_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
