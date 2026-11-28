use static_assertions::assert_not_impl_any;

#[test]
fn thread_safety_implot_context_not_send_sync() {
    assert_not_impl_any!(dear_implot::PlotContext: Send, Sync);
    assert_not_impl_any!(dear_implot::PlotUi<'static>: Send, Sync);
}

#[test]
fn plot_capabilities_stay_on_the_owner_thread() {
    assert_not_impl_any!(dear_implot::PlotSetup<'static, 'static>: Send, Sync);
    assert_not_impl_any!(dear_implot::PlotScope<'static>: Send, Sync);
}
