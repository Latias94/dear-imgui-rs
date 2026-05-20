fn setup_context() -> crate::Context {
    let mut ctx = crate::Context::create();
    let _ = ctx.font_atlas_mut().build();
    ctx.io_mut().set_display_size([128.0, 128.0]);
    ctx.io_mut().set_delta_time(1.0 / 60.0);
    ctx
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
