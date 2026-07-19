use std::error::Error;
use std::fmt;
use std::num::NonZeroU64;

use dear_imgui_rs::render::{RenderedFrame, RendererConsumerError};
use dear_imgui_rs::{Context, ContextBindingError, ContextId, FrameLifecycleState, Ui};

use crate::{
    AttachmentState, ResultSummary, RunFlags, RunOutcome, RunReport, RunState, TestEngine,
    TestEngineError, TestGroup,
};

const DEFAULT_FRAME_BUDGET: NonZeroU64 = NonZeroU64::new(10_000).unwrap();
const DEFAULT_CLEANUP_FRAME_BUDGET: NonZeroU64 = NonZeroU64::new(256).unwrap();

/// Control returned by the application UI callback after one frame has been built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerControl {
    /// Continue the bounded run.
    Continue,
    /// Finish the current frame, then abort and drain the native queue.
    Abort,
}

/// Callback stage that produced an infrastructure error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RunnerCallbackStage {
    /// Application UI construction failed.
    ApplicationUi,
    /// The renderer callback failed while consuming a rendered frame.
    Render,
}

/// Infrastructure failure produced by [`TestRunner`].
///
/// Product outcomes such as assertion failures, no matches, timeouts, and explicit aborts are
/// returned as [`RunReport`] instead.
#[derive(Debug)]
#[non_exhaustive]
pub enum RunnerError<E> {
    /// A safe Test Engine operation failed.
    TestEngine(TestEngineError),
    /// The supplied Context is not the Context to which the engine is attached.
    ContextMismatch {
        expected: ContextId,
        actual: ContextId,
    },
    /// The attached Context could not be bound for the complete run.
    ContextBinding(ContextBindingError),
    /// The runner was entered while another owner held an open frame.
    FrameAlreadyOpen,
    /// An application or renderer callback failed.
    Callback {
        stage: RunnerCallbackStage,
        frame: u64,
        source: E,
    },
    /// Headless rendering cannot satisfy managed texture requests.
    HeadlessTextureRequests { frame: u64, count: usize },
    /// Headless reconciliation rejected an otherwise empty feedback set.
    HeadlessTextureFeedback {
        frame: u64,
        source: RendererConsumerError,
    },
    /// Native terminal counters contradicted the terminal state contract.
    InvalidTerminalSummary {
        summary: ResultSummary,
        detail: &'static str,
    },
    /// The native queue did not settle within the cleanup budget.
    CleanupDidNotSettle {
        requested: RunOutcome,
        cleanup_frames: u64,
    },
}

impl<E> From<TestEngineError> for RunnerError<E> {
    fn from(source: TestEngineError) -> Self {
        Self::TestEngine(source)
    }
}

impl<E: fmt::Display> fmt::Display for RunnerError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TestEngine(source) => {
                write!(formatter, "Test Engine infrastructure failed: {source}")
            }
            Self::ContextMismatch { expected, actual } => write!(
                formatter,
                "runner received Context {actual:?}, but the Test Engine is attached to {expected:?}"
            ),
            Self::ContextBinding(source) => {
                write!(
                    formatter,
                    "runner could not bind the attached Context: {source}"
                )
            }
            Self::FrameAlreadyOpen => {
                formatter.write_str("runner cannot start while a Dear ImGui frame is open")
            }
            Self::Callback {
                stage,
                frame,
                source,
            } => write!(
                formatter,
                "{stage:?} callback failed while pumping frame {frame}: {source}"
            ),
            Self::HeadlessTextureRequests { frame, count } => write!(
                formatter,
                "headless frame {frame} contains {count} managed texture request(s)"
            ),
            Self::HeadlessTextureFeedback { frame, source } => write!(
                formatter,
                "headless feedback reconciliation failed on frame {frame}: {source}"
            ),
            Self::InvalidTerminalSummary { summary, detail } => write!(
                formatter,
                "native terminal summary {summary:?} is invalid: {detail}"
            ),
            Self::CleanupDidNotSettle {
                requested,
                cleanup_frames,
            } => write!(
                formatter,
                "{requested:?} cleanup did not settle after {cleanup_frames} frame(s)"
            ),
        }
    }
}

impl<E> Error for RunnerError<E>
where
    E: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::TestEngine(source) => Some(source),
            Self::ContextBinding(source) => Some(source),
            Self::Callback { source, .. } => Some(source),
            Self::HeadlessTextureFeedback { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// One-shot bounded Test Engine pump.
///
/// The runner owns the complete frame sequence: application UI, frame render, renderer callback,
/// and `post_swap`. This prevents callers from accidentally reporting a terminal result while
/// omitting a required phase.
pub struct TestRunner<'engine> {
    engine: &'engine mut TestEngine,
    group: TestGroup,
    filter: Option<String>,
    run_flags: RunFlags,
    frame_budget: NonZeroU64,
    cleanup_frame_budget: NonZeroU64,
}

impl<'engine> TestRunner<'engine> {
    /// Creates a runner for one attached engine using conservative default budgets.
    pub fn new(engine: &'engine mut TestEngine) -> Self {
        Self {
            engine,
            group: TestGroup::Tests,
            filter: None,
            run_flags: RunFlags::NONE,
            frame_budget: DEFAULT_FRAME_BUDGET,
            cleanup_frame_budget: DEFAULT_CLEANUP_FRAME_BUDGET,
        }
    }

    /// Selects the native test group.
    #[must_use]
    pub fn group(mut self, group: TestGroup) -> Self {
        self.group = group;
        self
    }

    /// Restricts the queued tests using the native filter syntax.
    #[must_use]
    pub fn filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = Some(filter.into());
        self
    }

    /// Selects the native queue flags.
    #[must_use]
    pub fn run_flags(mut self, run_flags: RunFlags) -> Self {
        self.run_flags = run_flags;
        self
    }

    /// Sets the number of primary frames available to the run.
    #[must_use]
    pub fn frame_budget(mut self, frame_budget: NonZeroU64) -> Self {
        self.frame_budget = frame_budget;
        self
    }

    /// Sets the number of additional frames available to drain an abort or timeout.
    #[must_use]
    pub fn cleanup_frame_budget(mut self, cleanup_frame_budget: NonZeroU64) -> Self {
        self.cleanup_frame_budget = cleanup_frame_budget;
        self
    }

    /// Runs with an application callback and an explicit renderer callback.
    ///
    /// The renderer receives each [`RenderedFrame`] by value, giving it exclusive ownership of
    /// that frame's render lease. Callback failures are infrastructure errors.
    pub fn run_with_renderer<E, App, Render>(
        self,
        context: &mut Context,
        application_ui: App,
        mut render: Render,
    ) -> Result<RunReport, RunnerError<E>>
    where
        App: FnMut(&Ui, u64) -> Result<RunnerControl, E>,
        Render: for<'frame> FnMut(RenderedFrame<'frame>) -> Result<(), E>,
    {
        self.run_impl(context, application_ui, move |frame, frame_index| {
            render(frame).map_err(|source| RunnerError::Callback {
                stage: RunnerCallbackStage::Render,
                frame: frame_index,
                source,
            })
        })
    }

    /// Runs without a GPU renderer.
    ///
    /// Headless mode reconciles an empty feedback set for every render lease. A managed texture
    /// request is rejected as infrastructure failure because no backend exists to upload it.
    pub fn run_headless<E, App>(
        self,
        context: &mut Context,
        application_ui: App,
    ) -> Result<RunReport, RunnerError<E>>
    where
        App: FnMut(&Ui, u64) -> Result<RunnerControl, E>,
    {
        self.run_impl(context, application_ui, |mut frame, frame_index| {
            let request_count = frame.texture_requests().len();
            if request_count != 0 {
                return Err(RunnerError::HeadlessTextureRequests {
                    frame: frame_index,
                    count: request_count,
                });
            }
            frame.reconcile_texture_feedback([]).map_err(|source| {
                RunnerError::HeadlessTextureFeedback {
                    frame: frame_index,
                    source,
                }
            })?;
            Ok(())
        })
    }

    fn run_impl<E, App, Render>(
        mut self,
        context: &mut Context,
        mut application_ui: App,
        mut render: Render,
    ) -> Result<RunReport, RunnerError<E>>
    where
        App: FnMut(&Ui, u64) -> Result<RunnerControl, E>,
        Render: for<'frame> FnMut(RenderedFrame<'frame>, u64) -> Result<(), RunnerError<E>>,
    {
        let actual = context.id();
        let expected = self.engine.attached_context_id().ok_or_else(|| {
            RunnerError::TestEngine(TestEngineError::invalid_state(
                "TestRunner::run",
                self.engine.attachment_state(),
                self.engine.run_state(),
                "runner requires a live Test Engine attachment",
            ))
        })?;
        if actual != expected {
            return Err(RunnerError::ContextMismatch { expected, actual });
        }

        let binding = context.binding();
        match binding
            .try_with_bound_context(|| self.run_bound(context, &mut application_ui, &mut render))
        {
            Ok(Ok(report)) => Ok(report),
            Ok(Err(error)) => {
                if !matches!(error, RunnerError::FrameAlreadyOpen) {
                    self.best_effort_stop();
                }
                Err(error)
            }
            Err(source) => Err(RunnerError::ContextBinding(source)),
        }
    }

    fn run_bound<E, App, Render>(
        &mut self,
        context: &mut Context,
        application_ui: &mut App,
        render: &mut Render,
    ) -> Result<RunReport, RunnerError<E>>
    where
        App: FnMut(&Ui, u64) -> Result<RunnerControl, E>,
        Render: for<'frame> FnMut(RenderedFrame<'frame>, u64) -> Result<(), RunnerError<E>>,
    {
        self.require_closed_frame(context)?;
        self.engine
            .queue_tests(self.group, self.filter.as_deref(), self.run_flags)?;

        if let Some(summary) = self.take_terminal_summary()? {
            return self.natural_report(summary, 0, 0);
        }

        let mut frames = 0;
        for frame_index in 1..=self.frame_budget.get() {
            let control = self.pump_frame(context, frame_index, application_ui, render)?;
            frames = frame_index;

            if let Some(summary) = self.take_terminal_summary()? {
                return self.natural_report(summary, frames, 0);
            }

            if control == RunnerControl::Abort {
                return self.drain_requested(
                    context,
                    RunOutcome::Aborted,
                    frames,
                    application_ui,
                    render,
                );
            }
        }

        self.drain_requested(
            context,
            RunOutcome::TimedOut,
            frames,
            application_ui,
            render,
        )
    }

    fn pump_frame<E, App, Render>(
        &mut self,
        context: &mut Context,
        frame_index: u64,
        application_ui: &mut App,
        render: &mut Render,
    ) -> Result<RunnerControl, RunnerError<E>>
    where
        App: FnMut(&Ui, u64) -> Result<RunnerControl, E>,
        Render: for<'frame> FnMut(RenderedFrame<'frame>, u64) -> Result<(), RunnerError<E>>,
    {
        self.require_closed_frame(context)?;
        let frame = context.begin_frame();
        let control =
            application_ui(frame.ui(), frame_index).map_err(|source| RunnerError::Callback {
                stage: RunnerCallbackStage::ApplicationUi,
                frame: frame_index,
                source,
            })?;
        self.engine.show_windows(frame.ui(), None)?;
        render(frame.render(), frame_index)?;
        self.engine.post_swap()?;
        Ok(control)
    }

    fn drain_requested<E, App, Render>(
        &mut self,
        context: &mut Context,
        requested: RunOutcome,
        frames: u64,
        application_ui: &mut App,
        render: &mut Render,
    ) -> Result<RunReport, RunnerError<E>>
    where
        App: FnMut(&Ui, u64) -> Result<RunnerControl, E>,
        Render: for<'frame> FnMut(RenderedFrame<'frame>, u64) -> Result<(), RunnerError<E>>,
    {
        debug_assert!(matches!(
            requested,
            RunOutcome::TimedOut | RunOutcome::Aborted
        ));
        let mut cleanup_frames = 0;

        for _ in 0..self.cleanup_frame_budget.get() {
            if let Some(summary) = self.take_terminal_summary()? {
                self.validate_terminal_summary(summary)?;
                return Ok(RunReport {
                    outcome: requested,
                    summary,
                    frames: frames + cleanup_frames,
                    cleanup_frames,
                });
            }

            if self.engine.try_abort_engine()?
                && let Some(summary) = self.take_terminal_summary()?
            {
                self.validate_terminal_summary(summary)?;
                return Ok(RunReport {
                    outcome: requested,
                    summary,
                    frames: frames + cleanup_frames,
                    cleanup_frames,
                });
            }

            let frame_index = frames + cleanup_frames + 1;
            self.pump_frame(context, frame_index, application_ui, render)?;
            cleanup_frames += 1;

            if let Some(summary) = self.take_terminal_summary()? {
                self.validate_terminal_summary(summary)?;
                return Ok(RunReport {
                    outcome: requested,
                    summary,
                    frames: frames + cleanup_frames,
                    cleanup_frames,
                });
            }
        }

        Err(RunnerError::CleanupDidNotSettle {
            requested,
            cleanup_frames,
        })
    }

    fn require_closed_frame<E>(&self, context: &Context) -> Result<(), RunnerError<E>> {
        match context.frame_lifecycle_state() {
            FrameLifecycleState::Idle | FrameLifecycleState::Rendered => Ok(()),
            FrameLifecycleState::InFrame => Err(RunnerError::FrameAlreadyOpen),
        }
    }

    fn take_terminal_summary<E>(&mut self) -> Result<Option<ResultSummary>, RunnerError<E>> {
        if self.engine.run_state() != RunState::Terminal {
            return Ok(None);
        }
        self.engine.take_terminal_summary().map_err(Into::into)
    }

    fn natural_report<E>(
        &self,
        summary: ResultSummary,
        frames: u64,
        cleanup_frames: u64,
    ) -> Result<RunReport, RunnerError<E>> {
        self.validate_terminal_summary(summary)?;
        let outcome = if summary.count_tested == 0 {
            RunOutcome::NoMatch
        } else if summary.count_success == summary.count_tested {
            RunOutcome::Passed
        } else {
            RunOutcome::Failed
        };
        Ok(RunReport {
            outcome,
            summary,
            frames,
            cleanup_frames,
        })
    }

    fn validate_terminal_summary<E>(&self, summary: ResultSummary) -> Result<(), RunnerError<E>> {
        if summary.count_in_queue != 0 {
            return Err(RunnerError::InvalidTerminalSummary {
                summary,
                detail: "terminal state retained queued tests",
            });
        }
        if summary.count_success > summary.count_tested {
            return Err(RunnerError::InvalidTerminalSummary {
                summary,
                detail: "successful test count exceeded executed test count",
            });
        }
        Ok(())
    }

    fn best_effort_stop(&mut self) {
        if self.engine.attachment_state() == AttachmentState::Attached
            && self.engine.run_state() != RunState::Inactive
        {
            let _ = self.engine.stop();
        }
    }
}
