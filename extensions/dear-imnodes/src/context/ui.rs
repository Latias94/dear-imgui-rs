use super::{EditorContext, ImNodesContext, NodeEditorSetup, NodesUi};
use dear_imgui_rs::Ui;

impl<'ui> NodesUi<'ui> {
    pub(crate) fn new(ui: &'ui Ui, ctx: &'ui ImNodesContext) -> Self {
        // Keep ImNodes bound to the ImGui context this ImNodes context was created with.
        // This avoids accidental cross-context use when users manage multiple ImGui contexts.
        assert_eq!(
            ui.context_id(),
            ctx.imgui_binding.id(),
            "dear-imnodes: NodesUi requires a Ui from the owning ImGui context"
        );
        Self { ui, ctx }
    }

    /// Prepares the selected editor. Drawing begins inside [`NodeEditorSetup::with_nodes`].
    pub fn editor(&self, editor: Option<&EditorContext>) -> NodeEditorSetup<'ui> {
        NodeEditorSetup::new(self.ui, self.ctx, editor)
    }
}
