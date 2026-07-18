use std::num::NonZeroU32;

use super::*;

#[test]
fn result_summary_rejects_negative_native_counts() {
    let summary = ResultSummary::try_from_raw(3, 2, 1).expect("valid summary");
    assert_eq!(summary.count_tested, 3);
    assert_eq!(summary.count_success, 2);
    assert_eq!(summary.count_in_queue, 1);

    for counts in [(-1, 0, 0), (0, -1, 0), (0, 0, -1)] {
        let error = ResultSummary::try_from_raw(counts.0, counts.1, counts.2)
            .expect_err("negative native counts must be rejected");
        assert!(matches!(error, TestEngineError::InvalidNativeData { .. }));
    }
}

#[test]
fn script_count_rejects_zero_and_native_overflow_without_panicking() {
    assert_eq!(ScriptCount::new(1).expect("valid count").raw(), 1);
    assert!(matches!(
        ScriptCount::new(0),
        Err(TestEngineError::InvalidInput {
            argument: "count",
            ..
        })
    ));
    assert!(matches!(
        ScriptCount::new(i32::MAX as u32 + 1),
        Err(TestEngineError::InvalidInput {
            argument: "count",
            ..
        })
    ));
}

#[test]
fn script_limit_models_only_all_or_a_positive_native_count() {
    assert_eq!(ScriptLimit::ALL.raw(), -1);
    assert_eq!(ScriptLimit::new(3).expect("valid limit").raw(), 3);
    assert_eq!(
        ScriptLimit::from_nonzero(NonZeroU32::new(4).expect("non-zero"))
            .expect("valid limit")
            .raw(),
        4
    );
    assert!(matches!(
        ScriptLimit::new(0),
        Err(TestEngineError::InvalidInput {
            argument: "limit",
            ..
        })
    ));
    assert!(matches!(
        ScriptLimit::new(i32::MAX as u32 + 1),
        Err(TestEngineError::InvalidInput {
            argument: "limit",
            ..
        })
    ));
}

#[test]
fn queued_and_running_states_reject_another_queue_until_terminal_is_consumed() {
    assert!(RunState::Ready.accepts_queue());
    assert!(!RunState::Queued.accepts_queue());
    assert!(!RunState::Running.accepts_queue());
    assert!(!RunState::Terminal.accepts_queue());
    assert_eq!(
        RunState::Terminal.after_terminal_consumed(),
        RunState::Ready
    );
}
