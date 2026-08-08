use crate::{GpuGeneration, RunError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SurfaceEvent {
    Success,
    Suboptimal,
    Lost,
    Outdated,
    Timeout,
    Occluded,
    Validation,
    OutOfMemory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LifecycleAction {
    Render,
    RenderAndReconfigure,
    ReconfigureSurface,
    RecreateSurface,
    SkipFrame,
    RecoverGpu,
    Exit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeState {
    Running(GpuGeneration),
    Recovering(GpuGeneration),
    Failed,
    Shutdown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TransitionError {
    RecoveryNotActive { state: RuntimeState },
    GenerationExhausted,
}

pub(crate) struct LifecycleMachine {
    state: RuntimeState,
    terminal_error: Option<RunError>,
}

impl LifecycleMachine {
    pub(crate) fn new() -> Self {
        Self {
            state: RuntimeState::Running(GpuGeneration::INITIAL),
            terminal_error: None,
        }
    }

    #[cfg(test)]
    const fn new_at(generation: GpuGeneration) -> Self {
        Self {
            state: RuntimeState::Running(generation),
            terminal_error: None,
        }
    }

    #[cfg(test)]
    pub(crate) const fn state(&self) -> RuntimeState {
        self.state
    }

    pub(crate) fn surface_event(&mut self, event: SurfaceEvent) -> LifecycleAction {
        if !matches!(self.state, RuntimeState::Running(_)) {
            return LifecycleAction::SkipFrame;
        }

        match event {
            SurfaceEvent::Success => LifecycleAction::Render,
            SurfaceEvent::Suboptimal => LifecycleAction::RenderAndReconfigure,
            SurfaceEvent::Lost => LifecycleAction::RecreateSurface,
            SurfaceEvent::Outdated => LifecycleAction::ReconfigureSurface,
            SurfaceEvent::Timeout | SurfaceEvent::Occluded => LifecycleAction::SkipFrame,
            SurfaceEvent::Validation | SurfaceEvent::OutOfMemory => {
                self.state = RuntimeState::Failed;
                LifecycleAction::Exit
            }
        }
    }

    pub(crate) fn device_lost(&mut self, signal_generation: GpuGeneration) -> LifecycleAction {
        let RuntimeState::Running(current_generation) = self.state else {
            return LifecycleAction::SkipFrame;
        };
        if signal_generation != current_generation {
            return LifecycleAction::SkipFrame;
        }

        self.state = RuntimeState::Recovering(current_generation);
        LifecycleAction::RecoverGpu
    }

    pub(crate) fn pending_generation(&self) -> Result<GpuGeneration, TransitionError> {
        let RuntimeState::Recovering(previous) = self.state else {
            return Err(TransitionError::RecoveryNotActive { state: self.state });
        };
        previous
            .checked_next()
            .ok_or(TransitionError::GenerationExhausted)
    }

    pub(crate) fn recovery_succeeded(&mut self) -> Result<GpuGeneration, TransitionError> {
        let next = match self.pending_generation() {
            Ok(next) => next,
            Err(error) => {
                if error == TransitionError::GenerationExhausted {
                    self.state = RuntimeState::Failed;
                }
                return Err(error);
            }
        };
        self.state = RuntimeState::Running(next);
        Ok(next)
    }

    pub(crate) fn mark_failed(&mut self) {
        self.state = RuntimeState::Failed;
    }

    pub(crate) fn shutdown(&mut self) {
        self.state = RuntimeState::Shutdown;
    }

    pub(crate) fn fail(&mut self, error: RunError) {
        self.mark_failed();
        if self.terminal_error.is_none() {
            self.terminal_error = Some(error);
        }
    }

    pub(crate) fn take_terminal_error(&mut self) -> Option<RunError> {
        self.terminal_error.take()
    }

    pub(crate) fn terminal_error(&self) -> Option<&RunError> {
        self.terminal_error.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::{LifecycleAction, LifecycleMachine, RuntimeState, SurfaceEvent, TransitionError};
    use crate::{GpuGeneration, RunError};

    #[test]
    fn surface_events_never_start_gpu_recovery() {
        let mut lifecycle = LifecycleMachine::new();

        assert_eq!(
            lifecycle.surface_event(SurfaceEvent::Success),
            LifecycleAction::Render
        );
        assert_eq!(
            lifecycle.surface_event(SurfaceEvent::Suboptimal),
            LifecycleAction::RenderAndReconfigure
        );
        assert_eq!(
            lifecycle.surface_event(SurfaceEvent::Lost),
            LifecycleAction::RecreateSurface
        );
        assert_eq!(
            lifecycle.surface_event(SurfaceEvent::Outdated),
            LifecycleAction::ReconfigureSurface
        );
        assert_eq!(
            lifecycle.surface_event(SurfaceEvent::Timeout),
            LifecycleAction::SkipFrame
        );
        assert_eq!(
            lifecycle.surface_event(SurfaceEvent::Occluded),
            LifecycleAction::SkipFrame
        );
        assert_eq!(
            lifecycle.surface_event(SurfaceEvent::Validation),
            LifecycleAction::Exit
        );
        assert_eq!(lifecycle.state(), RuntimeState::Failed);

        let mut out_of_memory = LifecycleMachine::new();
        assert_eq!(
            out_of_memory.surface_event(SurfaceEvent::OutOfMemory),
            LifecycleAction::Exit
        );
        assert_eq!(out_of_memory.state(), RuntimeState::Failed);
    }

    #[test]
    fn only_current_device_loss_replaces_the_gpu_generation() {
        let mut lifecycle = LifecycleMachine::new();

        assert_eq!(
            lifecycle.device_lost(GpuGeneration::INITIAL),
            LifecycleAction::RecoverGpu
        );
        assert_eq!(
            lifecycle.state(),
            RuntimeState::Recovering(GpuGeneration::INITIAL)
        );
        let next = GpuGeneration::INITIAL.checked_next().unwrap();
        assert_eq!(lifecycle.pending_generation(), Ok(next));
        assert_eq!(lifecycle.recovery_succeeded(), Ok(next));
        assert_eq!(lifecycle.state(), RuntimeState::Running(next));
    }

    #[test]
    fn late_device_loss_from_an_old_generation_has_no_effect() {
        let mut lifecycle = LifecycleMachine::new();
        assert_eq!(
            lifecycle.device_lost(GpuGeneration::INITIAL),
            LifecycleAction::RecoverGpu
        );
        let current = lifecycle.recovery_succeeded().unwrap();

        assert_eq!(
            lifecycle.device_lost(GpuGeneration::INITIAL),
            LifecycleAction::SkipFrame
        );
        assert_eq!(lifecycle.state(), RuntimeState::Running(current));
    }

    #[test]
    fn a_failed_replacement_becomes_terminal() {
        let mut lifecycle = LifecycleMachine::new();

        assert_eq!(
            lifecycle.device_lost(GpuGeneration::INITIAL),
            LifecycleAction::RecoverGpu
        );
        lifecycle.mark_failed();
        assert_eq!(lifecycle.state(), RuntimeState::Failed);
    }

    #[test]
    fn illegal_recovery_completion_is_explicit() {
        let mut lifecycle = LifecycleMachine::new();

        assert_eq!(
            lifecycle.recovery_succeeded(),
            Err(TransitionError::RecoveryNotActive {
                state: RuntimeState::Running(GpuGeneration::INITIAL),
            })
        );
    }

    #[test]
    fn generation_overflow_is_terminal_instead_of_reusing_an_epoch() {
        let max_generation = GpuGeneration(u64::MAX);
        let mut lifecycle = LifecycleMachine::new_at(max_generation);
        assert_eq!(
            lifecycle.device_lost(max_generation),
            LifecycleAction::RecoverGpu
        );

        assert_eq!(
            lifecycle.recovery_succeeded(),
            Err(TransitionError::GenerationExhausted)
        );
        assert_eq!(lifecycle.state(), RuntimeState::Failed);
    }

    #[test]
    fn shutdown_is_idempotent_and_blocks_future_work() {
        let mut lifecycle = LifecycleMachine::new();

        lifecycle.shutdown();
        lifecycle.shutdown();
        assert_eq!(lifecycle.state(), RuntimeState::Shutdown);
        assert_eq!(
            lifecycle.device_lost(GpuGeneration::INITIAL),
            LifecycleAction::SkipFrame
        );
    }

    #[test]
    fn lifecycle_owns_and_preserves_the_first_terminal_error() {
        let mut lifecycle = LifecycleMachine::new();
        lifecycle.fail(RunError::GpuValidation {
            message: "primary failure".to_owned(),
        });
        lifecycle.fail(RunError::GpuInternal {
            message: "secondary failure".to_owned(),
        });
        lifecycle.shutdown();

        let error = lifecycle
            .take_terminal_error()
            .expect("the first terminal error should be retained");
        assert_eq!(
            error.to_string(),
            "WGPU reported an uncaptured validation error: primary failure"
        );
        assert!(lifecycle.take_terminal_error().is_none());
    }
}
