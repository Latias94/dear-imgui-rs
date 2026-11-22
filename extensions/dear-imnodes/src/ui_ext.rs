use dear_imgui_rs::Ui;

use crate::{Context, EditorContext, NodeEditorSetup, NodesUi};

/// Ui extension entry point for ImNodes
pub trait ImNodesExt {
    fn imnodes<'ui>(&'ui self, ctx: &'ui Context) -> NodesUi<'ui>;
    /// Prepares an explicit editor, or the context's default editor when `editor` is `None`.
    /// Submit a frame with [`NodeEditorSetup::with_nodes`].
    fn imnodes_editor<'ui>(
        &'ui self,
        ctx: &'ui Context,
        editor: Option<&EditorContext>,
    ) -> NodeEditorSetup<'ui>;
}

impl ImNodesExt for Ui {
    fn imnodes<'ui>(&'ui self, ctx: &'ui Context) -> NodesUi<'ui> {
        NodesUi::new(self, ctx)
    }

    fn imnodes_editor<'ui>(
        &'ui self,
        ctx: &'ui Context,
        editor: Option<&EditorContext>,
    ) -> NodeEditorSetup<'ui> {
        self.imnodes(ctx).editor(editor)
    }
}
