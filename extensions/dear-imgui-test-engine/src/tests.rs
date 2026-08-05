use std::num::NonZeroU32;

use crate::results::RunCompletion;

use super::*;

#[test]
fn result_summary_is_derived_from_the_exact_run_manifest() {
    let tests = [
        RunTestResult::new("category".into(), "not-run".into(), RunTestStatus::NotRun),
        RunTestResult::new("category".into(), "queued".into(), RunTestStatus::Queued),
        RunTestResult::new("category".into(), "running".into(), RunTestStatus::Running),
        RunTestResult::new("category".into(), "success".into(), RunTestStatus::Success),
        RunTestResult::new("category".into(), "error".into(), RunTestStatus::Error),
        RunTestResult::new(
            "category".into(),
            "suspended".into(),
            RunTestStatus::Suspended,
        ),
    ];
    let summary = ResultSummary::from_tests(&tests);
    assert_eq!(summary.count_tested, 3);
    assert_eq!(summary.count_success, 1);
    assert_eq!(summary.count_in_queue, 2);
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
}

#[test]
fn a_non_empty_terminal_manifest_with_unfinished_tests_is_aborted_not_no_match() {
    let completion = RunCompletion {
        engine_id: EngineId::from_raw(1).expect("non-zero engine identity"),
        run_id: RunId::from_raw(1).expect("non-zero run identity"),
        summary: ResultSummary::default(),
        tests: vec![RunTestResult::new(
            "category".into(),
            "not-run".into(),
            RunTestStatus::NotRun,
        )],
    };

    assert_eq!(completion.natural_outcome(), RunOutcome::Aborted);
}

#[test]
fn a_failed_test_precedes_unfinished_tests_in_the_run_outcome() {
    let tests = vec![
        RunTestResult::new("category".into(), "failed".into(), RunTestStatus::Error),
        RunTestResult::new("category".into(), "not-run".into(), RunTestStatus::NotRun),
    ];
    let completion = RunCompletion {
        engine_id: EngineId::from_raw(1).expect("non-zero engine identity"),
        run_id: RunId::from_raw(1).expect("non-zero run identity"),
        summary: ResultSummary::from_tests(&tests),
        tests,
    };

    assert_eq!(completion.natural_outcome(), RunOutcome::Failed);
}
