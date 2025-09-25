use static_assertions::assert_not_impl_any;

#[test]
fn imnodes_context_not_send_sync() {
    assert_not_impl_any!(dear_imnodes::Context: Send, Sync);
    assert_not_impl_any!(dear_imnodes::EditorContext: Send, Sync);
}
