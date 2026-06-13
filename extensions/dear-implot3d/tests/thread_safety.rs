use static_assertions::assert_not_impl_any;

#[test]
fn thread_safety_implot3d_context_and_scope_tokens_not_send_sync() {
    assert_not_impl_any!(dear_implot3d::Plot3DContext: Send, Sync);
    assert_not_impl_any!(dear_implot3d::Plot3DUi<'static>: Send, Sync);
    assert_not_impl_any!(dear_implot3d::Plot3DBuilder<'static>: Send, Sync);
    assert_not_impl_any!(dear_implot3d::Plot3DToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_implot3d::StyleVarToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_implot3d::StyleColorToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_implot3d::ColormapToken<'static>: Send, Sync);
}
