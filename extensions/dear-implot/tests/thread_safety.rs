use static_assertions::assert_not_impl_any;

#[test]
fn implot_context_not_send_sync() {
    assert_not_impl_any!(dear_implot::PlotContext: Send, Sync);
}
