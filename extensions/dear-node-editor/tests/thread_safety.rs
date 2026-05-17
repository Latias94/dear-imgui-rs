use static_assertions::assert_not_impl_any;

#[test]
fn thread_safety_node_editor_context_and_scope_tokens_not_send_sync() {
    assert_not_impl_any!(dear_node_editor::EditorContext: Send, Sync);
    assert_not_impl_any!(dear_node_editor::NodeEditorFrame<'static>: Send, Sync);
    assert_not_impl_any!(dear_node_editor::NodeToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_node_editor::PinToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_node_editor::GroupHintToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_node_editor::SuspensionToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_node_editor::CreateSession<'static>: Send, Sync);
    assert_not_impl_any!(dear_node_editor::DeleteSession<'static>: Send, Sync);
    assert_not_impl_any!(dear_node_editor::ShortcutSession<'static>: Send, Sync);
    assert_not_impl_any!(dear_node_editor::StyleColorToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_node_editor::StyleVarToken<'static>: Send, Sync);
}
