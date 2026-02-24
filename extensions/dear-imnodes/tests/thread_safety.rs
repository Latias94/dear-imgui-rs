use static_assertions::assert_not_impl_any;

#[test]
fn imnodes_context_not_send_sync() {
    assert_not_impl_any!(dear_imnodes::Context: Send, Sync);
    assert_not_impl_any!(dear_imnodes::EditorContext: Send, Sync);
    assert_not_impl_any!(dear_imnodes::NodesUi<'static>: Send, Sync);
    assert_not_impl_any!(dear_imnodes::NodeEditor<'static>: Send, Sync);
    assert_not_impl_any!(dear_imnodes::NodeToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imnodes::AttributeToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imnodes::ColorToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imnodes::StyleVarToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imnodes::AttributeFlagToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imnodes::PostEditor<'static>: Send, Sync);
}
