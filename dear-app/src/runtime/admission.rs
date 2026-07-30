use super::lifecycle::{LifecycleAction, SurfaceEvent};
use crate::RunError;

const RECOVERY_RETRY_BUDGET: usize = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SurfaceAcquisition<Frame> {
    Success(Frame),
    Suboptimal(Frame),
    Lost,
    Outdated,
    Timeout,
    Occluded,
    Validation,
    // WGPU 30 reports OOM through `Device::on_uncaptured_error`; this variant keeps the
    // injectable admission policy exhaustive for older/future surface APIs.
    #[cfg_attr(not(test), allow(dead_code))]
    OutOfMemory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AdmittedSurfaceFrame<Frame> {
    pub(crate) frame: Frame,
    pub(crate) reconfigure_after_present: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SurfaceSkipReason {
    Timeout,
    Occluded,
    RecoveryRetryExhausted(SurfaceEvent),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SurfaceAdmission<Frame> {
    Admitted(AdmittedSurfaceFrame<Frame>),
    Skipped(SurfaceSkipReason),
}

pub(crate) trait SurfaceAdmissionBackend {
    type Frame;

    fn acquire(&mut self) -> SurfaceAcquisition<Self::Frame>;
    fn record_event(&mut self, event: SurfaceEvent) -> LifecycleAction;
    fn recover(&mut self, action: LifecycleAction) -> Result<(), RunError>;
}

pub(crate) fn admit_surface_frame<Backend>(
    backend: &mut Backend,
) -> Result<SurfaceAdmission<Backend::Frame>, RunError>
where
    Backend: SurfaceAdmissionBackend,
{
    let mut recovery_retries = 0usize;
    loop {
        let acquisition = backend.acquire();
        let event = acquisition.event();
        let action = backend.record_event(event);
        match acquisition {
            SurfaceAcquisition::Success(frame) => {
                require_action(event, action, LifecycleAction::Render)?;
                return Ok(SurfaceAdmission::Admitted(AdmittedSurfaceFrame {
                    frame,
                    reconfigure_after_present: false,
                }));
            }
            SurfaceAcquisition::Suboptimal(frame) => {
                require_action(event, action, LifecycleAction::RenderAndReconfigure)?;
                return Ok(SurfaceAdmission::Admitted(AdmittedSurfaceFrame {
                    frame,
                    reconfigure_after_present: true,
                }));
            }
            SurfaceAcquisition::Lost | SurfaceAcquisition::Outdated => {
                let expected = match event {
                    SurfaceEvent::Lost => LifecycleAction::RecreateSurface,
                    SurfaceEvent::Outdated => LifecycleAction::ReconfigureSurface,
                    _ => unreachable!("the acquisition arm fixes the recovery event"),
                };
                require_action(event, action, expected)?;
                backend.recover(action)?;
                if recovery_retries == RECOVERY_RETRY_BUDGET {
                    return Ok(SurfaceAdmission::Skipped(
                        SurfaceSkipReason::RecoveryRetryExhausted(event),
                    ));
                }
                recovery_retries += 1;
            }
            SurfaceAcquisition::Timeout => {
                require_action(event, action, LifecycleAction::SkipFrame)?;
                return Ok(SurfaceAdmission::Skipped(SurfaceSkipReason::Timeout));
            }
            SurfaceAcquisition::Occluded => {
                require_action(event, action, LifecycleAction::SkipFrame)?;
                return Ok(SurfaceAdmission::Skipped(SurfaceSkipReason::Occluded));
            }
            SurfaceAcquisition::Validation => {
                require_action(event, action, LifecycleAction::Exit)?;
                return Err(RunError::SurfaceValidation);
            }
            SurfaceAcquisition::OutOfMemory => {
                require_action(event, action, LifecycleAction::Exit)?;
                return Err(RunError::GpuOutOfMemory {
                    message: "surface acquisition exhausted GPU memory".to_owned(),
                });
            }
        }
    }
}

pub(crate) fn dispatch_surface_frame<Frame, Output>(
    admission: SurfaceAdmission<Frame>,
    admitted_frame_count: &mut u64,
    drive: impl FnOnce(AdmittedSurfaceFrame<Frame>, u64) -> Result<Output, RunError>,
) -> Result<Option<Output>, RunError> {
    let admitted = match admission {
        SurfaceAdmission::Admitted(admitted) => admitted,
        SurfaceAdmission::Skipped(_) => return Ok(None),
    };
    let frame_index = admitted_frame_count
        .checked_add(1)
        .ok_or_else(|| RunError::Recovery {
            message: "admitted frame counter exhausted".to_owned(),
        })?;
    *admitted_frame_count = frame_index;
    drive(admitted, frame_index).map(Some)
}

pub(crate) fn settle_surface_presentation<Output>(
    result: Result<Output, RunError>,
    was_presented: bool,
    reconfigure_after_present: bool,
    reconfigure: impl FnOnce(),
) -> Result<Output, RunError> {
    if reconfigure_after_present && was_presented {
        reconfigure();
    }
    result
}

impl<Frame> SurfaceAcquisition<Frame> {
    const fn event(&self) -> SurfaceEvent {
        match self {
            Self::Success(_) => SurfaceEvent::Success,
            Self::Suboptimal(_) => SurfaceEvent::Suboptimal,
            Self::Lost => SurfaceEvent::Lost,
            Self::Outdated => SurfaceEvent::Outdated,
            Self::Timeout => SurfaceEvent::Timeout,
            Self::Occluded => SurfaceEvent::Occluded,
            Self::Validation => SurfaceEvent::Validation,
            Self::OutOfMemory => SurfaceEvent::OutOfMemory,
        }
    }
}

fn require_action(
    event: SurfaceEvent,
    actual: LifecycleAction,
    expected: LifecycleAction,
) -> Result<(), RunError> {
    if actual == expected {
        Ok(())
    } else {
        Err(RunError::Recovery {
            message: format!(
                "surface event {event:?} produced lifecycle action {actual:?}, expected {expected:?}"
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::{
        AdmittedSurfaceFrame, SurfaceAcquisition, SurfaceAdmission, SurfaceAdmissionBackend,
        SurfaceSkipReason, admit_surface_frame, dispatch_surface_frame,
        settle_surface_presentation,
    };
    use crate::RunError;
    use crate::runtime::lifecycle::{LifecycleAction, LifecycleMachine, SurfaceEvent};

    struct ProbeBackend {
        acquisitions: VecDeque<SurfaceAcquisition<u32>>,
        lifecycle: LifecycleMachine,
        events: Vec<String>,
        fail_recovery: bool,
    }

    impl ProbeBackend {
        fn new(acquisitions: impl IntoIterator<Item = SurfaceAcquisition<u32>>) -> Self {
            Self {
                acquisitions: acquisitions.into_iter().collect(),
                lifecycle: LifecycleMachine::new(),
                events: Vec::new(),
                fail_recovery: false,
            }
        }
    }

    impl SurfaceAdmissionBackend for ProbeBackend {
        type Frame = u32;

        fn acquire(&mut self) -> SurfaceAcquisition<Self::Frame> {
            self.events.push("acquire".to_owned());
            self.acquisitions
                .pop_front()
                .expect("the probe must provide every attempted acquisition")
        }

        fn record_event(&mut self, event: SurfaceEvent) -> LifecycleAction {
            self.events.push(format!("event:{event:?}"));
            self.lifecycle.surface_event(event)
        }

        fn recover(&mut self, action: LifecycleAction) -> Result<(), RunError> {
            self.events.push(format!("recover:{action:?}"));
            if self.fail_recovery {
                Err(RunError::Recovery {
                    message: "injected surface recovery failure".to_owned(),
                })
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn successful_and_suboptimal_frames_preserve_post_present_policy() {
        let mut success = ProbeBackend::new([SurfaceAcquisition::Success(7)]);
        assert_eq!(
            admit_surface_frame(&mut success).unwrap(),
            SurfaceAdmission::Admitted(AdmittedSurfaceFrame {
                frame: 7,
                reconfigure_after_present: false,
            })
        );

        let mut suboptimal = ProbeBackend::new([SurfaceAcquisition::Suboptimal(9)]);
        assert_eq!(
            admit_surface_frame(&mut suboptimal).unwrap(),
            SurfaceAdmission::Admitted(AdmittedSurfaceFrame {
                frame: 9,
                reconfigure_after_present: true,
            })
        );
    }

    #[test]
    fn timeout_and_occlusion_skip_without_recovery() {
        for (acquisition, expected) in [
            (SurfaceAcquisition::Timeout, SurfaceSkipReason::Timeout),
            (SurfaceAcquisition::Occluded, SurfaceSkipReason::Occluded),
        ] {
            let mut backend = ProbeBackend::new([acquisition]);
            assert_eq!(
                admit_surface_frame(&mut backend).unwrap(),
                SurfaceAdmission::Skipped(expected)
            );
            assert_eq!(backend.events.len(), 2);
        }
    }

    #[test]
    fn lost_and_outdated_recover_before_the_single_retry() {
        for (first, expected_action) in [
            (SurfaceAcquisition::Lost, LifecycleAction::RecreateSurface),
            (
                SurfaceAcquisition::Outdated,
                LifecycleAction::ReconfigureSurface,
            ),
        ] {
            let mut backend = ProbeBackend::new([first, SurfaceAcquisition::Success(11)]);
            assert_eq!(
                admit_surface_frame(&mut backend).unwrap(),
                SurfaceAdmission::Admitted(AdmittedSurfaceFrame {
                    frame: 11,
                    reconfigure_after_present: false,
                })
            );
            assert_eq!(backend.events[2], format!("recover:{expected_action:?}"));
            assert_eq!(backend.events[3], "acquire");
        }
    }

    #[test]
    fn repeated_recoverable_failure_is_bounded_and_prepares_the_next_redraw() {
        let mut backend =
            ProbeBackend::new([SurfaceAcquisition::Lost, SurfaceAcquisition::Outdated]);
        assert_eq!(
            admit_surface_frame(&mut backend).unwrap(),
            SurfaceAdmission::Skipped(SurfaceSkipReason::RecoveryRetryExhausted(
                SurfaceEvent::Outdated,
            ))
        );
        assert_eq!(
            backend
                .events
                .iter()
                .filter(|event| event.starts_with("recover:"))
                .count(),
            2
        );
    }

    #[test]
    fn validation_out_of_memory_and_recovery_failures_are_terminal_errors() {
        let mut validation = ProbeBackend::new([SurfaceAcquisition::Validation]);
        assert!(matches!(
            admit_surface_frame(&mut validation),
            Err(RunError::SurfaceValidation)
        ));

        let mut out_of_memory = ProbeBackend::new([SurfaceAcquisition::OutOfMemory]);
        assert!(matches!(
            admit_surface_frame(&mut out_of_memory),
            Err(RunError::GpuOutOfMemory { .. })
        ));

        let mut recovery = ProbeBackend::new([SurfaceAcquisition::Lost]);
        recovery.fail_recovery = true;
        assert!(
            admit_surface_frame(&mut recovery)
                .unwrap_err()
                .to_string()
                .contains("injected surface recovery failure")
        );
    }

    #[test]
    fn skipped_surfaces_do_not_advance_or_invoke_frame_work() {
        for reason in [SurfaceSkipReason::Timeout, SurfaceSkipReason::Occluded] {
            let mut admitted_frame_count = 17;
            let mut invoked = false;
            let dispatch = dispatch_surface_frame::<(), ()>(
                SurfaceAdmission::Skipped(reason),
                &mut admitted_frame_count,
                |_, _| {
                    invoked = true;
                    Ok(())
                },
            )
            .expect("skip dispatch");
            assert_eq!(dispatch, None);
            assert!(!invoked, "skipped surface entered per-frame work");
            assert_eq!(admitted_frame_count, 17);
        }
    }

    #[test]
    fn recovery_precedes_the_only_admitted_frame_and_frame_index() {
        let mut backend =
            ProbeBackend::new([SurfaceAcquisition::Lost, SurfaceAcquisition::Success(23)]);
        let admission = admit_surface_frame(&mut backend).expect("recovered admission");
        let mut admitted_frame_count = 4;
        let dispatch =
            dispatch_surface_frame(admission, &mut admitted_frame_count, |frame, index| {
                assert_eq!(frame.frame, 23);
                assert_eq!(index, 5);
                backend.events.push("ui".to_owned());
                backend.events.push("render".to_owned());
                backend.events.push("present".to_owned());
                Ok(index)
            })
            .expect("admitted dispatch");
        assert_eq!(dispatch, Some(5));
        assert_eq!(admitted_frame_count, 5);
        assert_eq!(
            backend.events,
            [
                "acquire",
                "event:Lost",
                "recover:RecreateSurface",
                "acquire",
                "event:Success",
                "ui",
                "render",
                "present",
            ]
        );
    }

    #[test]
    fn admitted_counter_overflow_fails_before_frame_work() {
        let mut admitted_frame_count = u64::MAX;
        let mut invoked = false;
        let error = dispatch_surface_frame(
            SurfaceAdmission::Admitted(AdmittedSurfaceFrame {
                frame: (),
                reconfigure_after_present: false,
            }),
            &mut admitted_frame_count,
            |_, _| {
                invoked = true;
                Ok(())
            },
        )
        .expect_err("counter overflow must be terminal");
        assert!(!invoked);
        assert!(error.to_string().contains("frame counter exhausted"));
    }

    #[test]
    fn suboptimal_reconfiguration_requires_an_actual_presentation() {
        for (was_presented, result_is_error, expected_reconfigures) in [
            (true, false, 1),
            (true, true, 1),
            (false, false, 0),
            (false, true, 0),
        ] {
            let mut reconfigures = 0;
            let result = if result_is_error {
                Err(RunError::Recovery {
                    message: "injected post-presentation failure".to_owned(),
                })
            } else {
                Ok(41)
            };
            let settled = settle_surface_presentation(result, was_presented, true, || {
                reconfigures += 1;
            });
            assert_eq!(settled.is_err(), result_is_error);
            assert_eq!(reconfigures, expected_reconfigures);
        }

        let mut reconfigures = 0;
        let settled = settle_surface_presentation(Ok(7), true, false, || reconfigures += 1);
        assert_eq!(settled.unwrap(), 7);
        assert_eq!(reconfigures, 0);
    }
}
