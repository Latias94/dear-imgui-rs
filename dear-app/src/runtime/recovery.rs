#[cfg(test)]
use super::lifecycle::RuntimeState;
use super::lifecycle::{LifecycleAction, LifecycleMachine, SurfaceEvent};
use crate::{GpuGeneration, RunError};

pub(crate) trait OwnedGpuGeneration {
    fn generation(&self) -> GpuGeneration;
    fn teardown(self);
}

pub(crate) trait RecoveryEffects<G> {
    fn gpu_lost(&mut self, generation: &mut G) -> Result<(), RunError>;
    fn invalidate_resources(&mut self, generation: &mut G) -> Result<(), RunError>;
    fn gpu_recreated(&mut self, generation: &mut G) -> Result<(), RunError>;
}

pub(crate) trait RuntimeFactory<E> {
    type Candidate: OwnedGpuGeneration;

    fn create(
        &mut self,
        environment: &mut E,
        generation: GpuGeneration,
    ) -> Result<Self::Candidate, RunError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RecoveryOutcome {
    Ignored,
    Recovered(GpuGeneration),
    Failed,
}

pub(crate) struct RuntimeGenerations<G: OwnedGpuGeneration> {
    lifecycle: LifecycleMachine,
    current: Option<G>,
}

impl<G: OwnedGpuGeneration> RuntimeGenerations<G> {
    pub(crate) fn new(initial: G) -> Result<Self, RunError> {
        let actual = initial.generation();
        if actual != GpuGeneration::INITIAL {
            initial.teardown();
            return Err(RunError::Recovery {
                message: format!(
                    "initial GPU generation must be {}, got {}",
                    GpuGeneration::INITIAL.get(),
                    actual.get()
                ),
            });
        }
        Ok(Self {
            lifecycle: LifecycleMachine::new(),
            current: Some(initial),
        })
    }

    pub(crate) fn current(&self) -> Option<&G> {
        self.current.as_ref()
    }

    pub(crate) fn current_mut(&mut self) -> Option<&mut G> {
        self.current.as_mut()
    }

    pub(crate) fn current_generation(&self) -> Option<GpuGeneration> {
        self.current.as_ref().map(OwnedGpuGeneration::generation)
    }

    pub(crate) fn surface_event(&mut self, event: SurfaceEvent) -> LifecycleAction {
        self.lifecycle.surface_event(event)
    }

    pub(crate) fn recover<E, F>(
        &mut self,
        signal_generation: GpuGeneration,
        environment: &mut E,
        factory: &mut F,
    ) -> RecoveryOutcome
    where
        E: RecoveryEffects<G>,
        F: RuntimeFactory<E, Candidate = G>,
    {
        if self.lifecycle.device_lost(signal_generation) != LifecycleAction::RecoverGpu {
            return RecoveryOutcome::Ignored;
        }

        match self.recover_active(signal_generation, environment, factory) {
            Ok(generation) => RecoveryOutcome::Recovered(generation),
            Err(error) => {
                self.lifecycle.fail(error);
                RecoveryOutcome::Failed
            }
        }
    }

    fn recover_active<E, F>(
        &mut self,
        signal_generation: GpuGeneration,
        environment: &mut E,
        factory: &mut F,
    ) -> Result<GpuGeneration, RunError>
    where
        E: RecoveryEffects<G>,
        F: RuntimeFactory<E, Candidate = G>,
    {
        let next = self
            .lifecycle
            .pending_generation()
            .map_err(|error| recovery_transition_error("allocate", error))?;
        {
            let current = self.current.as_mut().ok_or_else(|| RunError::Recovery {
                message: "device loss received without an active GPU generation".to_owned(),
            })?;
            if current.generation() != signal_generation {
                return Err(RunError::Recovery {
                    message: "lifecycle and GPU generation slot diverged before recovery"
                        .to_owned(),
                });
            }
            environment.gpu_lost(current)?;
            environment.invalidate_resources(current)?;
        }

        let old = self.current.take().ok_or_else(|| RunError::Recovery {
            message: "GPU generation disappeared before teardown".to_owned(),
        })?;
        old.teardown();

        let candidate = factory.create(environment, next)?;
        let candidate_generation = candidate.generation();
        if candidate_generation != next {
            candidate.teardown();
            return Err(RunError::Recovery {
                message: format!(
                    "factory built GPU generation {}, expected {}",
                    candidate_generation.get(),
                    next.get()
                ),
            });
        }

        self.current = Some(candidate);
        let committed = match self.lifecycle.recovery_succeeded() {
            Ok(generation) => generation,
            Err(error) => {
                self.teardown_current();
                return Err(recovery_transition_error("commit", error));
            }
        };
        if committed != next || self.current_generation() != Some(committed) {
            self.teardown_current();
            return Err(RunError::Recovery {
                message: "committed GPU generation differs from the candidate generation"
                    .to_owned(),
            });
        }

        let current = self.current.as_mut().ok_or_else(|| RunError::Recovery {
            message: "GPU generation disappeared before the ready notification".to_owned(),
        })?;
        environment.gpu_recreated(current)?;
        Ok(committed)
    }

    pub(crate) fn fail(&mut self, error: RunError) {
        self.lifecycle.fail(error);
    }

    pub(crate) fn shutdown(&mut self) {
        self.teardown_current();
        self.lifecycle.shutdown();
    }

    pub(crate) fn take_terminal_error(&mut self) -> Option<RunError> {
        self.lifecycle.take_terminal_error()
    }

    pub(crate) fn terminal_error(&self) -> Option<&RunError> {
        self.lifecycle.terminal_error()
    }

    fn teardown_current(&mut self) {
        if let Some(generation) = self.current.take() {
            generation.teardown();
        }
    }

    #[cfg(test)]
    pub(crate) fn state(&self) -> RuntimeState {
        self.lifecycle.state()
    }
}

impl<G: OwnedGpuGeneration> Drop for RuntimeGenerations<G> {
    fn drop(&mut self) {
        self.teardown_current();
    }
}

fn recovery_transition_error(
    operation: &'static str,
    error: super::lifecycle::TransitionError,
) -> RunError {
    RunError::Recovery {
        message: format!("cannot {operation} GPU generation: {error:?}"),
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::BTreeSet, rc::Rc};

    use super::{
        OwnedGpuGeneration, RecoveryEffects, RecoveryOutcome, RuntimeFactory, RuntimeGenerations,
    };
    use crate::runtime::lifecycle::RuntimeState;
    use crate::{GpuGeneration, RunError};

    #[derive(Default)]
    struct ProbeState {
        events: Vec<String>,
        alive_generations: BTreeSet<u64>,
    }

    struct ProbeGeneration {
        id: GpuGeneration,
        state: Rc<RefCell<ProbeState>>,
        released: bool,
        cleanup_event: &'static str,
    }

    impl ProbeGeneration {
        fn new(
            id: GpuGeneration,
            state: Rc<RefCell<ProbeState>>,
            cleanup_event: &'static str,
        ) -> Self {
            assert!(state.borrow_mut().alive_generations.insert(id.get()));
            Self {
                id,
                state,
                released: false,
                cleanup_event,
            }
        }
    }

    impl Drop for ProbeGeneration {
        fn drop(&mut self) {
            if self.released {
                return;
            }
            let mut state = self.state.borrow_mut();
            state.alive_generations.remove(&self.id.get());
            state
                .events
                .push(format!("{}:{}", self.cleanup_event, self.id.get()));
        }
    }

    impl OwnedGpuGeneration for ProbeGeneration {
        fn generation(&self) -> GpuGeneration {
            self.id
        }

        fn teardown(mut self) {
            self.released = true;
            let mut state = self.state.borrow_mut();
            state.alive_generations.remove(&self.id.get());
            state.events.push(format!("teardown:{}", self.id.get()));
        }
    }

    struct ProbeEnvironment {
        state: Rc<RefCell<ProbeState>>,
        stable_window: Box<u8>,
        stable_ui: Box<u8>,
        fail_on: Option<EffectFailure>,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum EffectFailure {
        GpuLost,
        InvalidateResources,
        GpuRecreated,
    }

    impl RecoveryEffects<ProbeGeneration> for ProbeEnvironment {
        fn gpu_lost(&mut self, generation: &mut ProbeGeneration) -> Result<(), RunError> {
            self.state
                .borrow_mut()
                .events
                .push(format!("gpu_lost:{}", generation.id.get()));
            if self.fail_on == Some(EffectFailure::GpuLost) {
                return Err(RunError::Recovery {
                    message: "injected gpu_lost failure".to_owned(),
                });
            }
            Ok(())
        }

        fn invalidate_resources(
            &mut self,
            generation: &mut ProbeGeneration,
        ) -> Result<(), RunError> {
            self.state
                .borrow_mut()
                .events
                .push(format!("managed_reset:{}", generation.id.get()));
            if self.fail_on == Some(EffectFailure::InvalidateResources) {
                return Err(RunError::Recovery {
                    message: "injected invalidation failure".to_owned(),
                });
            }
            Ok(())
        }

        fn gpu_recreated(&mut self, generation: &mut ProbeGeneration) -> Result<(), RunError> {
            self.state
                .borrow_mut()
                .events
                .push(format!("gpu_recreated:{}", generation.id.get()));
            if self.fail_on == Some(EffectFailure::GpuRecreated) {
                return Err(RunError::Recovery {
                    message: "injected gpu_recreated failure".to_owned(),
                });
            }
            Ok(())
        }
    }

    struct ProbeFactory {
        state: Rc<RefCell<ProbeState>>,
        fail_on: Option<GpuGeneration>,
    }

    impl RuntimeFactory<ProbeEnvironment> for ProbeFactory {
        type Candidate = ProbeGeneration;

        fn create(
            &mut self,
            _environment: &mut ProbeEnvironment,
            generation: GpuGeneration,
        ) -> Result<Self::Candidate, RunError> {
            self.state
                .borrow_mut()
                .events
                .push(format!("candidate_build:{}", generation.get()));
            let candidate =
                ProbeGeneration::new(generation, Rc::clone(&self.state), "candidate_cleanup");
            if self.fail_on == Some(generation) {
                return Err(RunError::Recovery {
                    message: "injected candidate failure".to_owned(),
                });
            }
            Ok(candidate)
        }
    }

    fn fixture(
        factory_failure: Option<GpuGeneration>,
        effect_failure: Option<EffectFailure>,
    ) -> (
        Rc<RefCell<ProbeState>>,
        RuntimeGenerations<ProbeGeneration>,
        ProbeEnvironment,
        ProbeFactory,
    ) {
        let state = Rc::new(RefCell::new(ProbeState::default()));
        let initial =
            ProbeGeneration::new(GpuGeneration::INITIAL, Rc::clone(&state), "initial_cleanup");
        let generations = RuntimeGenerations::new(initial).unwrap();
        let environment = ProbeEnvironment {
            state: Rc::clone(&state),
            stable_window: Box::new(1),
            stable_ui: Box::new(2),
            fail_on: effect_failure,
        };
        let factory = ProbeFactory {
            state: Rc::clone(&state),
            fail_on: factory_failure,
        };
        (state, generations, environment, factory)
    }

    #[test]
    fn owner_runs_real_teardown_commit_and_generation_transition() {
        let (state, mut generations, mut environment, mut factory) = fixture(None, None);
        let window_identity = std::ptr::from_ref(environment.stable_window.as_ref());
        let ui_identity = std::ptr::from_ref(environment.stable_ui.as_ref());
        let next = GpuGeneration::INITIAL.checked_next().unwrap();

        assert_eq!(
            generations.recover(GpuGeneration::INITIAL, &mut environment, &mut factory),
            RecoveryOutcome::Recovered(next)
        );

        assert_eq!(generations.current_generation(), Some(next));
        assert_eq!(generations.state(), RuntimeState::Running(next));
        assert_eq!(state.borrow().alive_generations, BTreeSet::from([1]));
        assert_eq!(
            state.borrow().events,
            [
                "gpu_lost:0",
                "managed_reset:0",
                "teardown:0",
                "candidate_build:1",
                "gpu_recreated:1",
            ]
        );
        assert_eq!(
            window_identity,
            std::ptr::from_ref(environment.stable_window.as_ref())
        );
        assert_eq!(
            ui_identity,
            std::ptr::from_ref(environment.stable_ui.as_ref())
        );

        generations.shutdown();
        assert_eq!(generations.state(), RuntimeState::Shutdown);
        assert!(state.borrow().alive_generations.is_empty());
        assert_eq!(state.borrow().events.last().unwrap(), "teardown:1");
        let event_count = state.borrow().events.len();
        drop(generations);
        assert_eq!(state.borrow().events.len(), event_count);
    }

    #[test]
    fn candidate_build_failure_cleans_every_gpu_generation_and_becomes_terminal() {
        let next = GpuGeneration::INITIAL.checked_next().unwrap();
        let (state, mut generations, mut environment, mut factory) = fixture(Some(next), None);

        assert_eq!(
            generations.recover(GpuGeneration::INITIAL, &mut environment, &mut factory),
            RecoveryOutcome::Failed
        );

        assert_eq!(generations.current_generation(), None);
        assert_eq!(generations.state(), RuntimeState::Failed);
        assert!(state.borrow().alive_generations.is_empty());
        assert_eq!(
            state.borrow().events,
            [
                "gpu_lost:0",
                "managed_reset:0",
                "teardown:0",
                "candidate_build:1",
                "candidate_cleanup:1",
            ]
        );
        let error = generations
            .take_terminal_error()
            .expect("the recovery owner must retain the factory error");
        assert!(error.to_string().contains("injected candidate failure"));
    }

    #[test]
    fn owner_can_recover_again_and_ignores_the_disappeared_generation() {
        let (state, mut generations, mut environment, mut factory) = fixture(None, None);
        let generation_one = GpuGeneration::INITIAL.checked_next().unwrap();
        let generation_two = generation_one.checked_next().unwrap();

        assert_eq!(
            generations.recover(GpuGeneration::INITIAL, &mut environment, &mut factory),
            RecoveryOutcome::Recovered(generation_one)
        );
        let event_count = state.borrow().events.len();
        assert_eq!(
            generations.recover(GpuGeneration::INITIAL, &mut environment, &mut factory),
            RecoveryOutcome::Ignored
        );
        assert_eq!(state.borrow().events.len(), event_count);

        assert_eq!(
            generations.recover(generation_one, &mut environment, &mut factory),
            RecoveryOutcome::Recovered(generation_two)
        );

        assert_eq!(generations.current_generation(), Some(generation_two));
        assert_eq!(generations.state(), RuntimeState::Running(generation_two));
        assert_eq!(state.borrow().alive_generations, BTreeSet::from([2]));
        assert_eq!(
            state.borrow().events,
            [
                "gpu_lost:0",
                "managed_reset:0",
                "teardown:0",
                "candidate_build:1",
                "gpu_recreated:1",
                "gpu_lost:1",
                "managed_reset:1",
                "teardown:1",
                "candidate_build:2",
                "gpu_recreated:2",
            ]
        );
    }

    fn assert_events(state: &Rc<RefCell<ProbeState>>, expected: &[&str]) {
        let state = state.borrow();
        let actual = state.events.iter().map(String::as_str).collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }

    fn assert_effect_failure(
        failure: EffectFailure,
        expected_current: GpuGeneration,
        expected_before_shutdown: &[&str],
        expected_after_shutdown: &[&str],
        expected_error: &str,
    ) {
        let (state, mut generations, mut environment, mut factory) = fixture(None, Some(failure));

        assert_eq!(
            generations.recover(GpuGeneration::INITIAL, &mut environment, &mut factory),
            RecoveryOutcome::Failed
        );
        assert_eq!(generations.state(), RuntimeState::Failed);
        assert_eq!(
            generations.current_generation(),
            Some(expected_current),
            "failure point: {failure:?}"
        );
        assert_eq!(
            state.borrow().alive_generations,
            BTreeSet::from([expected_current.get()]),
            "failure point: {failure:?}"
        );
        assert_events(&state, expected_before_shutdown);

        generations.fail(RunError::Recovery {
            message: "secondary failure must not replace the first".to_owned(),
        });
        let error = generations
            .take_terminal_error()
            .expect("the recovery owner must retain the first effect error");
        assert!(error.to_string().contains(expected_error));

        generations.shutdown();
        assert_eq!(generations.state(), RuntimeState::Shutdown);
        assert_eq!(generations.current_generation(), None);
        assert!(state.borrow().alive_generations.is_empty());
        assert_events(&state, expected_after_shutdown);

        let event_count = state.borrow().events.len();
        drop(generations);
        assert_eq!(state.borrow().events.len(), event_count);
    }

    #[test]
    fn gpu_lost_failure_keeps_then_tears_down_the_old_generation_once() {
        assert_effect_failure(
            EffectFailure::GpuLost,
            GpuGeneration::INITIAL,
            &["gpu_lost:0"],
            &["gpu_lost:0", "teardown:0"],
            "injected gpu_lost failure",
        );
    }

    #[test]
    fn invalidation_failure_never_builds_or_notifies_a_candidate() {
        assert_effect_failure(
            EffectFailure::InvalidateResources,
            GpuGeneration::INITIAL,
            &["gpu_lost:0", "managed_reset:0"],
            &["gpu_lost:0", "managed_reset:0", "teardown:0"],
            "injected invalidation failure",
        );
    }

    #[test]
    fn gpu_recreated_failure_tears_down_each_generation_once() {
        let replacement = GpuGeneration::INITIAL.checked_next().unwrap();
        assert_effect_failure(
            EffectFailure::GpuRecreated,
            replacement,
            &[
                "gpu_lost:0",
                "managed_reset:0",
                "teardown:0",
                "candidate_build:1",
                "gpu_recreated:1",
            ],
            &[
                "gpu_lost:0",
                "managed_reset:0",
                "teardown:0",
                "candidate_build:1",
                "gpu_recreated:1",
                "teardown:1",
            ],
            "injected gpu_recreated failure",
        );
    }
}
