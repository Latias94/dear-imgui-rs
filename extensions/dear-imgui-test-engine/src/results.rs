#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ResultSummary {
    pub count_tested: usize,
    pub count_success: usize,
    pub count_in_queue: usize,
}

pub(super) fn result_count_from_i32(caller: &str, raw: i32) -> usize {
    usize::try_from(raw).unwrap_or_else(|_| panic!("{caller} returned a negative count"))
}

impl ResultSummary {
    pub(super) fn from_raw(count_tested: i32, count_success: i32, count_in_queue: i32) -> Self {
        Self {
            count_tested: result_count_from_i32("ResultSummary::count_tested", count_tested),
            count_success: result_count_from_i32("ResultSummary::count_success", count_success),
            count_in_queue: result_count_from_i32("ResultSummary::count_in_queue", count_in_queue),
        }
    }
}
