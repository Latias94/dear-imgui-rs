#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ResultSummary {
    pub count_tested: usize,
    pub count_success: usize,
    pub count_in_queue: usize,
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
