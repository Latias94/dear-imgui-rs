use super::{Context, EditorContext, NodeEditorSetup, NodesUi};
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
    pub fn editor(&self, editor: Option<&EditorContext>) -> NodeEditorSetup<'ui> {
        NodeEditorSetup::begin(self._ui, self._ctx, editor)
    }
}
