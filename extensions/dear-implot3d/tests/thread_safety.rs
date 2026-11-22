use static_assertions::assert_not_impl_any;

#[test]
fn plot_context_and_borrowed_capabilities_are_thread_bound() {
    assert_not_impl_any!(dear_implot3d::Plot3DContext: Send, Sync);
    assert_not_impl_any!(dear_implot3d::Plot3DUi<'static>: Send, Sync);
    assert_not_impl_any!(dear_implot3d::Plot3DBuilder<'static>: Send, Sync);
    assert_not_impl_any!(dear_implot3d::Plot3DScope<'static>: Send, Sync);
    assert_not_impl_any!(dear_implot3d::Plot3DSetup<'static>: Send, Sync);
}
