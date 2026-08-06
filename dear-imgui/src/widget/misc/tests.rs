fn setup_context() -> crate::Context {
    let mut ctx = crate::Context::create();
    let _ = ctx.font_atlas().build();
    ctx.io_mut().set_display_size([128.0, 128.0]);
    ctx.io_mut().set_delta_time(1.0 / 60.0);
    ctx
}

#[derive(Clone, Copy, Debug)]
struct DisabledNativeState {
    alpha: f32,
    disabled_alpha: f32,
    alpha_backup: f32,
    current_item_flags: i32,
    item_flags_stack_size: i32,
    style_var_stack_size: i32,
    disabled_stack_size: i16,
}

fn disabled_native_state(ui: &crate::Ui) -> DisabledNativeState {
    let context = ui.context_raw();
    ui.binding().with_bound_context(|| unsafe {
        DisabledNativeState {
            alpha: (*context).Style.Alpha,
            disabled_alpha: (*context).Style.DisabledAlpha,
            alpha_backup: (*context).DisabledAlphaBackup,
            current_item_flags: (*context).CurrentItemFlags,
            item_flags_stack_size: (*context).ItemFlagsStack.Size,
            style_var_stack_size: (*context).StyleVarStack.Size,
            disabled_stack_size: (*context).DisabledStackSize,
        }
    })
}

fn assert_alpha_eq(actual: f32, expected: f32) {
    let tolerance = f32::EPSILON * expected.abs().max(1.0) * 4.0;
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected alpha {expected}, got {actual}"
    );
}

fn assert_disabled_state_restored(actual: DisabledNativeState, baseline: DisabledNativeState) {
    assert_alpha_eq(actual.alpha, baseline.alpha);
    assert_eq!(actual.current_item_flags, baseline.current_item_flags);
    assert_eq!(actual.item_flags_stack_size, baseline.item_flags_stack_size);
    assert_eq!(actual.style_var_stack_size, baseline.style_var_stack_size);
    assert_eq!(actual.disabled_stack_size, baseline.disabled_stack_size);
}

#[test]
fn with_button_repeat_pops_after_panic() {
    let mut ctx = setup_context();
    let ui = ctx.frame();
    let raw_ctx = unsafe { crate::sys::igGetCurrentContext() };
    assert!(!raw_ctx.is_null());
    let initial_stack_size = unsafe { (*raw_ctx).ItemFlagsStack.Size };

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ui.with_button_repeat(true, || {
            assert_eq!(
                unsafe { (*raw_ctx).ItemFlagsStack.Size },
                initial_stack_size + 1
            );
            panic!("forced panic while button repeat is pushed");
        });
    }));

    assert!(result.is_err());
    assert_eq!(
        unsafe { (*raw_ctx).ItemFlagsStack.Size },
        initial_stack_size
    );
}

#[test]
fn disabled_scopes_model_only_the_layer_that_owns_alpha_restoration() {
    let mut ctx = setup_context();
    let ui = ctx.frame();

    ui.window("disabled_alpha_ownership").build(|| {
        let baseline = disabled_native_state(ui);

        let disabled_false = ui.begin_disabled_with_cond(false);
        let after_false = disabled_native_state(ui);
        assert_alpha_eq(after_false.alpha, baseline.alpha);
        assert_eq!(after_false.alpha_backup, baseline.alpha_backup);
        assert_eq!(after_false.current_item_flags, baseline.current_item_flags);
        assert_eq!(
            after_false.item_flags_stack_size,
            baseline.item_flags_stack_size + 1
        );
        assert_eq!(
            after_false.disabled_stack_size,
            baseline.disabled_stack_size + 1
        );
        drop(disabled_false);
        assert_disabled_state_restored(disabled_native_state(ui), baseline);

        let outer = ui.begin_disabled();
        let after_outer = disabled_native_state(ui);
        assert_alpha_eq(after_outer.alpha, baseline.alpha * baseline.disabled_alpha);
        assert_alpha_eq(after_outer.alpha_backup, baseline.alpha);

        let inner_true = ui.begin_disabled();
        let after_inner_true = disabled_native_state(ui);
        assert_alpha_eq(after_inner_true.alpha, after_outer.alpha);
        assert_alpha_eq(after_inner_true.alpha_backup, after_outer.alpha_backup);
        drop(inner_true);
        assert_alpha_eq(disabled_native_state(ui).alpha, after_outer.alpha);

        let inner_false = ui.begin_disabled_with_cond(false);
        let after_inner_false = disabled_native_state(ui);
        assert_alpha_eq(after_inner_false.alpha, after_outer.alpha);
        assert_alpha_eq(after_inner_false.alpha_backup, after_outer.alpha_backup);
        drop(inner_false);
        assert_alpha_eq(disabled_native_state(ui).alpha, after_outer.alpha);
        drop(outer);
        assert_disabled_state_restored(disabled_native_state(ui), baseline);

        let outer_false = ui.begin_disabled_with_cond(false);
        let inner_true = ui.begin_disabled();
        assert_alpha_eq(
            disabled_native_state(ui).alpha,
            baseline.alpha * baseline.disabled_alpha,
        );
        drop(inner_true);
        assert_alpha_eq(disabled_native_state(ui).alpha, baseline.alpha);
        drop(outer_false);
        assert_disabled_state_restored(disabled_native_state(ui), baseline);
    });
}

#[test]
fn disabled_and_alpha_style_scopes_restore_in_both_lifo_nesting_directions() {
    let mut ctx = setup_context();
    let ui = ctx.frame();

    ui.window("disabled_alpha_lifo").build(|| {
        let baseline = disabled_native_state(ui);
        let overridden_alpha = 0.4;

        let alpha = ui.push_style_var(crate::StyleVar::Alpha(overridden_alpha));
        assert_alpha_eq(disabled_native_state(ui).alpha, overridden_alpha);
        let disabled = ui.begin_disabled();
        assert_alpha_eq(
            disabled_native_state(ui).alpha,
            overridden_alpha * baseline.disabled_alpha,
        );
        drop(disabled);
        assert_alpha_eq(disabled_native_state(ui).alpha, overridden_alpha);
        drop(alpha);
        assert_disabled_state_restored(disabled_native_state(ui), baseline);

        let disabled = ui.begin_disabled();
        let disabled_alpha = baseline.alpha * baseline.disabled_alpha;
        assert_alpha_eq(disabled_native_state(ui).alpha, disabled_alpha);
        let alpha = ui.push_style_var(crate::StyleVar::Alpha(overridden_alpha));
        assert_alpha_eq(disabled_native_state(ui).alpha, overridden_alpha);
        drop(alpha);
        assert_alpha_eq(disabled_native_state(ui).alpha, disabled_alpha);
        drop(disabled);
        assert_disabled_state_restored(disabled_native_state(ui), baseline);
    });
}

#[test]
fn disabled_and_alpha_style_scopes_reject_cross_stack_order_before_ffi_and_recover() {
    let mut ctx = setup_context();
    let ui = ctx.frame();

    ui.window("disabled_alpha_invalid_order").build(|| {
        let baseline = disabled_native_state(ui);

        let alpha = ui.push_style_var(crate::StyleVar::Alpha(0.4));
        let disabled = ui.begin_disabled();
        let before_failure = disabled_native_state(ui);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(alpha)));
        assert!(result.is_err());
        let after_failure = disabled_native_state(ui);
        assert_alpha_eq(after_failure.alpha, before_failure.alpha);
        assert_eq!(
            after_failure.item_flags_stack_size,
            before_failure.item_flags_stack_size
        );
        assert_eq!(
            after_failure.style_var_stack_size,
            before_failure.style_var_stack_size
        );
        assert_eq!(
            after_failure.disabled_stack_size,
            before_failure.disabled_stack_size
        );
        drop(disabled);
        assert_disabled_state_restored(disabled_native_state(ui), baseline);

        let disabled = ui.begin_disabled();
        let alpha = ui.push_style_var(crate::StyleVar::Alpha(0.4));
        let before_failure = disabled_native_state(ui);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(disabled)));
        assert!(result.is_err());
        let after_failure = disabled_native_state(ui);
        assert_alpha_eq(after_failure.alpha, before_failure.alpha);
        assert_eq!(
            after_failure.item_flags_stack_size,
            before_failure.item_flags_stack_size
        );
        assert_eq!(
            after_failure.style_var_stack_size,
            before_failure.style_var_stack_size
        );
        assert_eq!(
            after_failure.disabled_stack_size,
            before_failure.disabled_stack_size
        );
        drop(alpha);
        assert_disabled_state_restored(disabled_native_state(ui), baseline);
    });
}

#[test]
fn unrelated_disabled_and_style_scopes_remain_independent() {
    let mut ctx = setup_context();
    let ui = ctx.frame();

    ui.window("disabled_alpha_independent_scopes").build(|| {
        let baseline = disabled_native_state(ui);

        let disabled_false = ui.begin_disabled_with_cond(false);
        let alpha = ui.push_style_var(crate::StyleVar::Alpha(0.4));
        drop(disabled_false);
        drop(alpha);
        assert_disabled_state_restored(disabled_native_state(ui), baseline);

        let alpha = ui.push_style_var(crate::StyleVar::Alpha(0.4));
        let disabled_false = ui.begin_disabled_with_cond(false);
        drop(alpha);
        drop(disabled_false);
        assert_disabled_state_restored(disabled_native_state(ui), baseline);

        let disabled = ui.begin_disabled();
        let rounding = ui.push_style_var(crate::StyleVar::WindowRounding(7.0));
        drop(disabled);
        drop(rounding);
        assert_disabled_state_restored(disabled_native_state(ui), baseline);

        let rounding = ui.push_style_var(crate::StyleVar::WindowRounding(7.0));
        let disabled = ui.begin_disabled();
        drop(rounding);
        drop(disabled);
        assert_disabled_state_restored(disabled_native_state(ui), baseline);
    });
}

#[test]
fn disabled_and_alpha_closure_scopes_restore_during_unwinding() {
    let mut ctx = setup_context();
    let ui = ctx.frame();

    ui.window("disabled_alpha_unwind").build(|| {
        let baseline = disabled_native_state(ui);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ui.with_disabled(|| {
                let _alpha = ui.push_style_var(crate::StyleVar::Alpha(0.4));
                panic!("forced panic inside disabled outer scope");
            });
        }));
        assert!(result.is_err());
        assert_disabled_state_restored(disabled_native_state(ui), baseline);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _alpha = ui.push_style_var(crate::StyleVar::Alpha(0.4));
            ui.with_disabled(|| panic!("forced panic inside alpha outer scope"));
        }));
        assert!(result.is_err());
        assert_disabled_state_restored(disabled_native_state(ui), baseline);
    });
}
