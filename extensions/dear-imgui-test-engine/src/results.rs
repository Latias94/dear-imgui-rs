#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ResultSummary {
    pub count_tested: usize,
    pub count_success: usize,
    pub count_in_queue: usize,
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
    /// The queue reached terminal state without executing a test.
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

/// Structured terminal report produced by [`crate::TestRunner`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunReport {
    /// Terminal product outcome.
    pub outcome: RunOutcome,
    /// Native counters captured after the queue reached terminal state.
    pub summary: ResultSummary,
    /// Total number of complete frames pumped by the runner.
    pub frames: u64,
    /// Number of `frames` used to drain an abort or timeout.
    pub cleanup_frames: u64,
}

impl ResultSummary {
    pub(super) fn try_from_raw(
        count_tested: i32,
        count_success: i32,
        count_in_queue: i32,
    ) -> crate::TestEngineResult<Self> {
        let count_tested = usize::try_from(count_tested).map_err(|_| {
            crate::TestEngineError::InvalidNativeData {
                operation: "imgui_test_engine_get_result_summary",
                detail: "CountTested was negative",
            }
        })?;
        let count_success = usize::try_from(count_success).map_err(|_| {
            crate::TestEngineError::InvalidNativeData {
                operation: "imgui_test_engine_get_result_summary",
                detail: "CountSuccess was negative",
            }
        })?;
        let count_in_queue = usize::try_from(count_in_queue).map_err(|_| {
            crate::TestEngineError::InvalidNativeData {
                operation: "imgui_test_engine_get_result_summary",
                detail: "CountInQueue was negative",
            }
        })?;
        Ok(Self {
            count_tested,
            count_success,
            count_in_queue,
        })
    }
}
