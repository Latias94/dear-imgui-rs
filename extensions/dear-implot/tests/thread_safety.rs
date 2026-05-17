use static_assertions::assert_not_impl_any;

#[test]
fn thread_safety_implot_context_not_send_sync() {
    assert_not_impl_any!(dear_implot::PlotContext: Send, Sync);
    assert_not_impl_any!(dear_implot::PlotUi<'static>: Send, Sync);
}

#[test]
fn thread_safety_implot_scope_tokens_not_send_sync() {
    assert_not_impl_any!(dear_implot::PlotToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_implot::SubplotToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_implot::MultiAxisToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_implot::LegendToken: Send, Sync);
    assert_not_impl_any!(dear_implot::StyleVarToken: Send, Sync);
    assert_not_impl_any!(dear_implot::StyleColorToken: Send, Sync);
    assert_not_impl_any!(dear_implot::AxisFormatterToken: Send, Sync);
    assert_not_impl_any!(dear_implot::AxisTransformToken: Send, Sync);
}
