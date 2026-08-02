use dear_imgui_rs::Ui;

use crate::{Context, EditorContext, NodeEditorSetup, NodesUi};

/// Ui extension entry point for ImNodes
pub trait ImNodesExt {
    fn imnodes<'ui>(&'ui self, ctx: &'ui Context) -> NodesUi<'ui>;
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

// editor() is implemented in context.rs to access private fields
