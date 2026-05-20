use super::*;
use dear_imgui_rs::Context;

#[test]
fn result_summary_counts_are_checked_usize_counts() {
    let summary = ResultSummary::from_raw(3, 2, 1);
    let count_tested: usize = summary.count_tested;
    let count_success: usize = summary.count_success;
    let count_in_queue: usize = summary.count_in_queue;

    assert_eq!(count_tested, 3);
    assert_eq!(count_success, 2);
    assert_eq!(count_in_queue, 1);

    assert!(
        std::panic::catch_unwind(|| ResultSummary::from_raw(-1, 0, 0)).is_err(),
        "negative tested counts must not cross the safe API boundary"
    );
    assert!(
        std::panic::catch_unwind(|| ResultSummary::from_raw(0, -1, 0)).is_err(),
        "negative success counts must not cross the safe API boundary"
    );
    assert!(
        std::panic::catch_unwind(|| ResultSummary::from_raw(0, 0, -1)).is_err(),
        "negative queue counts must not cross the safe API boundary"
    );
}

#[test]
fn script_count_rejects_zero_and_overflow_before_ffi() {
    assert_eq!(ScriptCount::new(1).raw(), 1);
    assert!(std::panic::catch_unwind(|| ScriptCount::new(0)).is_err());
    assert!(std::panic::catch_unwind(|| ScriptCount::new(i32::MAX as u32 + 1)).is_err());
}

#[test]
fn script_limit_preserves_all_sentinel_and_rejects_overflow() {
    assert_eq!(ScriptLimit::ALL.raw(), -1);
    assert_eq!(ScriptLimit::new(0).raw(), 0);
    assert_eq!(ScriptLimit::new(3).raw(), 3);
    assert!(std::panic::catch_unwind(|| ScriptLimit::new(i32::MAX as u32 + 1)).is_err());
}

#[test]
fn test_engine_methods_reject_dropped_bound_imgui_context_before_ffi() {
    let mut ctx = Context::create();
    let _ = ctx.font_atlas_mut().build();

    let mut engine = TestEngine::create();
    engine.start(&ctx);
    drop(ctx);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = engine.result_summary();
    }));

    assert!(result.is_err());
}

#[test]
fn test_engine_methods_remain_available_after_explicit_shutdown() {
    let mut ctx = Context::create();
    let _ = ctx.font_atlas_mut().build();

    let mut engine = TestEngine::create();
    engine.start(&ctx);
    engine.shutdown();
    drop(ctx);

    assert!(!engine.is_bound());
    let _ = engine.result_summary();
}

#[test]
fn test_engine_try_start_rejects_rebinding_after_bound_context_drop() {
    let mut ctx_a = Context::create();
    let _ = ctx_a.font_atlas_mut().build();

    let mut engine = TestEngine::create();
    engine.start(&ctx_a);
    drop(ctx_a);

    let ctx_b = Context::create();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = engine.try_start(&ctx_b);
    }));

    assert!(result.is_err());
}
