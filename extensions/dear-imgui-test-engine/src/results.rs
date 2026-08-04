use crate::{EngineId, RunId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ResultSummary {
    pub count_tested: usize,
    pub count_success: usize,
    pub count_in_queue: usize,
}

/// Native terminal state captured for one exact queued test.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RunTestStatus {
    /// The test was selected but did not complete, usually because the run was aborted.
    NotRun,
    /// The test completed successfully.
    Success,
    /// The test is waiting in the native queue.
    Queued,
    /// The test is currently executing.
    Running,
    /// The test completed with an assertion or runtime error.
    Error,
    /// The test function is paused by the native debugger integration.
    Suspended,
}

impl RunTestStatus {
    pub(crate) fn try_from_raw(raw: i32) -> crate::TestEngineResult<Self> {
        match raw {
            dear_imgui_test_engine_sys::ImGuiTestEngineTestStatus_Unknown => Ok(Self::NotRun),
            dear_imgui_test_engine_sys::ImGuiTestEngineTestStatus_Success => Ok(Self::Success),
            dear_imgui_test_engine_sys::ImGuiTestEngineTestStatus_Queued => Ok(Self::Queued),
            dear_imgui_test_engine_sys::ImGuiTestEngineTestStatus_Running => Ok(Self::Running),
            dear_imgui_test_engine_sys::ImGuiTestEngineTestStatus_Error => Ok(Self::Error),
            dear_imgui_test_engine_sys::ImGuiTestEngineTestStatus_Suspended => Ok(Self::Suspended),
            _ => Err(crate::TestEngineError::InvalidNativeData {
                operation: "imgui_test_engine_get_run_test",
                detail: "run test status was outside the native enum domain",
            }),
        }
    }
}

/// Exact identity and terminal state of one test selected by a run.
#[derive(Debug, Eq, PartialEq)]
pub struct RunTestResult {
    category: String,
    name: String,
    status: RunTestStatus,
}

impl RunTestResult {
    pub(crate) fn new(category: String, name: String, status: RunTestStatus) -> Self {
        Self {
            category,
            name,
            status,
        }
    }

    #[must_use]
    pub fn category(&self) -> &str {
        &self.category
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn status(&self) -> RunTestStatus {
        self.status
    }
}

pub(crate) struct RunCompletion {
    pub(crate) engine_id: EngineId,
    pub(crate) run_id: RunId,
    pub(crate) summary: ResultSummary,
    pub(crate) tests: Vec<RunTestResult>,
}

impl RunCompletion {
    pub(crate) fn natural_outcome(&self) -> RunOutcome {
        if self.tests.is_empty() {
            RunOutcome::NoMatch
        } else if self
            .tests
            .iter()
            .any(|test| matches!(test.status, RunTestStatus::Error | RunTestStatus::Suspended))
        {
            RunOutcome::Failed
        } else if self
            .tests
            .iter()
            .any(|test| test.status == RunTestStatus::NotRun)
        {
            RunOutcome::Aborted
        } else if self
            .tests
            .iter()
            .all(|test| test.status == RunTestStatus::Success)
        {
            RunOutcome::Passed
        } else {
            RunOutcome::Failed
        }
    }
}

/// Terminal outcome of one bounded Test Engine run.
///
/// Infrastructure failures are returned as [`crate::RunnerError`] and never represented here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RunOutcome {
    /// At least one test ran and every test succeeded.
    Passed,
    /// At least one test ran and one or more tests failed.
    Failed,
    /// The queue reached terminal state without selecting a test.
    NoMatch,
    /// The primary frame budget expired and the runner subsequently drained the queue.
    TimedOut,
    /// The application requested an abort and the runner subsequently drained the queue.
    Aborted,
}

impl RunOutcome {
    /// Returns true only for a non-empty, fully successful run.
    #[must_use]
    pub const fn is_passed(self) -> bool {
        matches!(self, Self::Passed)
    }
}

/// Structured terminal report for one exact Test Engine queue operation.
///
/// The report is move-only because it is also the capability accepted by built-in suite
/// validation. Its engine, run, and selected-test identities cannot be forged through safe code.
#[derive(Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct RunReport {
    engine_id: EngineId,
    run_id: RunId,
    tests: Vec<RunTestResult>,
    outcome: RunOutcome,
    summary: ResultSummary,
    frames: u64,
    cleanup_frames: u64,
    mode: crate::RunMode,
}

impl RunReport {
    pub(crate) fn new(
        completion: RunCompletion,
        outcome: RunOutcome,
        frames: u64,
        cleanup_frames: u64,
        mode: crate::RunMode,
    ) -> Self {
        Self {
            engine_id: completion.engine_id,
            run_id: completion.run_id,
            tests: completion.tests,
            outcome,
            summary: completion.summary,
            frames,
            cleanup_frames,
            mode,
        }
    }

    #[must_use]
    pub const fn engine_id(&self) -> EngineId {
        self.engine_id
    }

    #[must_use]
    pub const fn run_id(&self) -> RunId {
        self.run_id
    }

    #[must_use]
    pub fn tests(&self) -> &[RunTestResult] {
        &self.tests
    }

    #[must_use]
    pub const fn outcome(&self) -> RunOutcome {
        self.outcome
    }

    #[must_use]
    pub const fn summary(&self) -> ResultSummary {
        self.summary
    }

    #[must_use]
    pub const fn frames(&self) -> u64 {
        self.frames
    }

    #[must_use]
    pub const fn cleanup_frames(&self) -> u64 {
        self.cleanup_frames
    }

    /// Returns the selected presentation mode.
    #[must_use]
    pub const fn mode(&self) -> crate::RunMode {
        self.mode
    }

    pub(crate) fn parts(&self) -> (EngineId, RunId, &[RunTestResult], RunOutcome, ResultSummary) {
        (
            self.engine_id,
            self.run_id,
            &self.tests,
            self.outcome,
            self.summary,
        )
    }
}

impl ResultSummary {
    pub(crate) fn from_tests(tests: &[RunTestResult]) -> Self {
        let mut summary = Self::default();
        for test in tests {
            match test.status {
                RunTestStatus::NotRun => {}
                RunTestStatus::Queued | RunTestStatus::Running => summary.count_in_queue += 1,
                RunTestStatus::Success => {
                    summary.count_tested += 1;
                    summary.count_success += 1;
                }
                RunTestStatus::Error | RunTestStatus::Suspended => summary.count_tested += 1,
            }
        }
        summary
    }
}
