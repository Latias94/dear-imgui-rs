#[test]
fn metrics_counts_are_usize_and_reject_negative_raw_values() {
    let mut ctx = crate::Context::create();
    let io = ctx.io_mut();

    io.inner_mut().MetricsRenderVertices = 11;
    io.inner_mut().MetricsRenderIndices = 22;
    io.inner_mut().MetricsRenderWindows = 3;
    io.inner_mut().MetricsActiveWindows = 4;

    assert_eq!(io.metrics_render_vertices(), 11);
    assert_eq!(io.metrics_render_indices(), 22);
    assert_eq!(io.metrics_render_windows(), 3);
    assert_eq!(io.metrics_active_windows(), 4);

    io.inner_mut().MetricsRenderVertices = -1;
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = io.metrics_render_vertices();
        }))
        .is_err()
    );

    io.inner_mut().MetricsRenderVertices = 0;
    io.inner_mut().MetricsRenderIndices = -1;
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = io.metrics_render_indices();
        }))
        .is_err()
    );

    io.inner_mut().MetricsRenderIndices = 0;
    io.inner_mut().MetricsRenderWindows = -1;
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = io.metrics_render_windows();
        }))
        .is_err()
    );

    io.inner_mut().MetricsRenderWindows = 0;
    io.inner_mut().MetricsActiveWindows = -1;
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = io.metrics_active_windows();
        }))
        .is_err()
    );
}

#[test]
fn mouse_hovered_viewport_round_trips_through_io() {
    let mut ctx = crate::Context::create();
    let io = ctx.io_mut();
    let viewport_id = crate::Id::from(0x1234);

    io.set_mouse_hovered_viewport(viewport_id);

    assert_eq!(io.mouse_hovered_viewport(), viewport_id);
}
