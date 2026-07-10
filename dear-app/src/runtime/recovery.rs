use thiserror::Error;

use crate::{GpuGeneration, RunError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RecoveryPhase {
    Started,
    LostNotified,
    ResourcesInvalidated,
    OldGpuTornDown,
    CandidateBuilt,
    CandidateCommitted,
    GenerationAdvanced,
    ReadyNotified,
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("invalid GPU recovery order: expected {expected:?}, got {actual:?}")]
struct RecoveryOrderError {
    expected: RecoveryPhase,
    actual: RecoveryPhase,
}

struct RecoveryJournal {
    phase: RecoveryPhase,
}

impl RecoveryJournal {
    const fn new() -> Self {
        Self {
            phase: RecoveryPhase::Started,
        }
    }

    fn advance(
        &mut self,
        expected_current: RecoveryPhase,
        next: RecoveryPhase,
    ) -> Result<(), RecoveryOrderError> {
        if self.phase != expected_current {
            return Err(RecoveryOrderError {
                expected: expected_current,
                actual: self.phase,
            });
        }
        self.phase = next;
        Ok(())
    }
}

pub(crate) trait RecoveryHooks {
    type Candidate;

    fn pending_generation(&self) -> Result<GpuGeneration, RunError>;
    fn gpu_lost(&mut self) -> Result<(), RunError>;
    fn invalidate_resources(&mut self) -> Result<(), RunError>;
    fn teardown_old_gpu(&mut self);
    fn build_candidate(&mut self, generation: GpuGeneration) -> Result<Self::Candidate, RunError>;
    fn commit_candidate(&mut self, candidate: Self::Candidate);
    fn advance_generation(&mut self) -> Result<GpuGeneration, RunError>;
    fn gpu_recreated(&mut self, generation: GpuGeneration) -> Result<(), RunError>;
    fn recovery_failed(&mut self);
}

pub(crate) fn execute_recovery<H: RecoveryHooks>(hooks: &mut H) -> Result<(), RunError> {
    let mut journal = RecoveryJournal::new();
    hooks.gpu_lost()?;
    advance(
        &mut journal,
        RecoveryPhase::Started,
        RecoveryPhase::LostNotified,
    )?;

    hooks.invalidate_resources()?;
    advance(
        &mut journal,
        RecoveryPhase::LostNotified,
        RecoveryPhase::ResourcesInvalidated,
    )?;

    hooks.teardown_old_gpu();
    advance(
        &mut journal,
        RecoveryPhase::ResourcesInvalidated,
        RecoveryPhase::OldGpuTornDown,
    )?;

    let next = hooks.pending_generation()?;
    let candidate = match hooks.build_candidate(next) {
        Ok(candidate) => candidate,
        Err(error) => {
            hooks.recovery_failed();
            return Err(error);
        }
    };
    advance(
        &mut journal,
        RecoveryPhase::OldGpuTornDown,
        RecoveryPhase::CandidateBuilt,
    )?;

    hooks.commit_candidate(candidate);
    advance(
        &mut journal,
        RecoveryPhase::CandidateBuilt,
        RecoveryPhase::CandidateCommitted,
    )?;

    let generation = hooks.advance_generation()?;
    if generation != next {
        return Err(RunError::Recovery {
            message: "committed GPU generation differs from the candidate generation".to_owned(),
        });
    }
    advance(
        &mut journal,
        RecoveryPhase::CandidateCommitted,
        RecoveryPhase::GenerationAdvanced,
    )?;

    hooks.gpu_recreated(generation)?;
    advance(
        &mut journal,
        RecoveryPhase::GenerationAdvanced,
        RecoveryPhase::ReadyNotified,
    )?;
    Ok(())
}

fn advance(
    journal: &mut RecoveryJournal,
    current: RecoveryPhase,
    next: RecoveryPhase,
) -> Result<(), RunError> {
    journal
        .advance(current, next)
        .map_err(|error| RunError::Recovery {
            message: error.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::{RecoveryHooks, execute_recovery};
    use crate::runtime::state::RuntimeFactory;
    use crate::{GpuGeneration, RunError};

    struct Candidate {
        events: Rc<RefCell<Vec<&'static str>>>,
        committed: bool,
    }

    impl Drop for Candidate {
        fn drop(&mut self) {
            if !self.committed {
                self.events.borrow_mut().push("candidate_cleanup");
            }
        }
    }

    struct FakeFactory {
        events: Rc<RefCell<Vec<&'static str>>>,
        fail_candidate: bool,
    }

    impl RuntimeFactory for FakeFactory {
        type Candidate = Candidate;

        fn create(&mut self, _generation: GpuGeneration) -> Result<Self::Candidate, RunError> {
            self.events.borrow_mut().push("candidate_build");
            let candidate = Candidate {
                events: Rc::clone(&self.events),
                committed: false,
            };
            if self.fail_candidate {
                return Err(RunError::Recovery {
                    message: "injected candidate failure".to_owned(),
                });
            }
            Ok(candidate)
        }
    }

    struct FakeHooks {
        events: Rc<RefCell<Vec<&'static str>>>,
        factory: FakeFactory,
        candidate: Option<Candidate>,
        generation: GpuGeneration,
        stable_window: Box<u8>,
        stable_ui: Box<u8>,
    }

    impl FakeHooks {
        fn new(fail_candidate: bool) -> Self {
            let events = Rc::new(RefCell::new(Vec::new()));
            Self {
                factory: FakeFactory {
                    events: Rc::clone(&events),
                    fail_candidate,
                },
                events,
                candidate: None,
                generation: GpuGeneration::INITIAL,
                stable_window: Box::new(1),
                stable_ui: Box::new(2),
            }
        }
    }

    impl RecoveryHooks for FakeHooks {
        type Candidate = Candidate;

        fn pending_generation(&self) -> Result<GpuGeneration, RunError> {
            self.generation
                .checked_next()
                .ok_or_else(|| RunError::Recovery {
                    message: "generation exhausted".to_owned(),
                })
        }

        fn gpu_lost(&mut self) -> Result<(), RunError> {
            self.events.borrow_mut().push("gpu_lost");
            Ok(())
        }

        fn invalidate_resources(&mut self) -> Result<(), RunError> {
            self.events.borrow_mut().push("managed_reset");
            Ok(())
        }

        fn teardown_old_gpu(&mut self) {
            self.events.borrow_mut().push("old_teardown");
        }

        fn build_candidate(
            &mut self,
            generation: GpuGeneration,
        ) -> Result<Self::Candidate, RunError> {
            self.factory.create(generation)
        }

        fn commit_candidate(&mut self, mut candidate: Self::Candidate) {
            self.events.borrow_mut().push("candidate_commit");
            candidate.committed = true;
            self.candidate = Some(candidate);
        }

        fn advance_generation(&mut self) -> Result<GpuGeneration, RunError> {
            self.events.borrow_mut().push("generation_advance");
            self.generation = self
                .generation
                .checked_next()
                .ok_or_else(|| RunError::Recovery {
                    message: "generation exhausted".to_owned(),
                })?;
            Ok(self.generation)
        }

        fn gpu_recreated(&mut self, _generation: GpuGeneration) -> Result<(), RunError> {
            self.events.borrow_mut().push("gpu_recreated");
            Ok(())
        }

        fn recovery_failed(&mut self) {
            self.events.borrow_mut().push("recovery_failed");
        }
    }

    #[test]
    fn coordinator_runs_the_production_recovery_order() {
        let mut hooks = FakeHooks::new(false);
        let window_identity = std::ptr::from_ref(hooks.stable_window.as_ref());
        let ui_identity = std::ptr::from_ref(hooks.stable_ui.as_ref());
        execute_recovery(&mut hooks).unwrap();

        assert_eq!(
            *hooks.events.borrow(),
            [
                "gpu_lost",
                "managed_reset",
                "old_teardown",
                "candidate_build",
                "candidate_commit",
                "generation_advance",
                "gpu_recreated",
            ]
        );
        assert_eq!(
            window_identity,
            std::ptr::from_ref(hooks.stable_window.as_ref())
        );
        assert_eq!(ui_identity, std::ptr::from_ref(hooks.stable_ui.as_ref()));
    }

    #[test]
    fn failed_candidate_is_cleaned_without_commit_or_ready_notification() {
        let mut hooks = FakeHooks::new(true);
        assert!(execute_recovery(&mut hooks).is_err());

        assert_eq!(
            *hooks.events.borrow(),
            [
                "gpu_lost",
                "managed_reset",
                "old_teardown",
                "candidate_build",
                "candidate_cleanup",
                "recovery_failed",
            ]
        );
        assert!(hooks.candidate.is_none());
    }
}
