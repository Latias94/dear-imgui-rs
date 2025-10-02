use dear_imgui_rs::Ui;

use crate::{Context, EditorContext, NodeEditor, NodesUi};

/// Ui extension entry point for ImNodes
pub trait ImNodesExt {
    fn imnodes<'ui>(&'ui self, ctx: &'ui Context) -> NodesUi<'ui>;
    fn imnodes_editor<'ui>(
        &'ui self,
        ctx: &'ui Context,
        editor: Option<&'ui EditorContext>,
    ) -> NodeEditor<'ui>;
}

impl ImNodesExt for Ui {
    fn imnodes<'ui>(&'ui self, ctx: &'ui Context) -> NodesUi<'ui> {
        NodesUi::new(self, ctx)
    }

    fn imnodes_editor<'ui>(
        &'ui self,
        ctx: &'ui Context,
        editor: Option<&'ui EditorContext>,
    ) -> NodeEditor<'ui> {
        self.imnodes(ctx).editor(editor)
    }
}

// editor() is implemented in context.rs to access private fields
