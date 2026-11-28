use dear_imgui_rs::Ui;

use crate::{EditorContext, ImNodesContext, NodeEditorSetup, NodesUi};

/// Ui extension entry point for ImNodes
pub trait ImNodesExt {
    fn imnodes<'ui>(&'ui self, ctx: &'ui ImNodesContext) -> NodesUi<'ui>;
    /// Prepares an explicit editor, or the context's default editor when `editor` is `None`.
    /// Submit a frame with [`NodeEditorSetup::with_nodes`].
    fn imnodes_editor<'ui>(
        &'ui self,
        ctx: &'ui ImNodesContext,
        editor: Option<&EditorContext>,
    ) -> NodeEditorSetup<'ui>;
}

impl ImNodesExt for Ui {
    fn imnodes<'ui>(&'ui self, ctx: &'ui ImNodesContext) -> NodesUi<'ui> {
        NodesUi::new(self, ctx)
    }

    fn imnodes_editor<'ui>(
        &'ui self,
        ctx: &'ui ImNodesContext,
        editor: Option<&EditorContext>,
    ) -> NodeEditorSetup<'ui> {
        self.imnodes(ctx).editor(editor)
    }
}
