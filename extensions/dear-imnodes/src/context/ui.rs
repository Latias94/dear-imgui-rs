use super::{Context, EditorContext, NodeEditor, NodesUi};
use dear_imgui_rs::Ui;

impl<'ui> NodesUi<'ui> {
    pub(crate) fn new(ui: &'ui Ui, ctx: &'ui Context) -> Self {
        // Keep ImNodes bound to the ImGui context this ImNodes context was created with.
        // This avoids accidental cross-context use when users manage multiple ImGui contexts.
        assert_eq!(
            ui.context_id(),
            ctx.imgui_binding.id(),
            "dear-imnodes: NodesUi requires a Ui from the owning ImGui context"
        );
        Self { _ui: ui, _ctx: ctx }
    }

    /// Begin a node editor with an optional EditorContext
    pub fn editor(&self, editor: Option<&'ui EditorContext>) -> NodeEditor<'ui> {
        if let Some(editor) = editor {
            let owner = self._ctx.alive_token();
            assert!(
                editor.bound_ctx_alive.is_alive() && editor.bound_ctx_alive.same_context(&owner),
                "dear-imnodes: EditorContext is bound to a different or destroyed ImNodes context"
            );
            assert_eq!(
                editor.bound_ctx_raw, self._ctx.raw,
                "dear-imnodes: EditorContext is bound to a different ImNodes context"
            );
            assert_eq!(
                editor.imgui_binding.id(),
                self._ctx.imgui_binding.id(),
                "dear-imnodes: EditorContext is bound to a different ImGui context"
            );
        }
        NodeEditor::begin(self._ui, self._ctx, editor)
    }
}
