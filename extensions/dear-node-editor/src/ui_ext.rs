use crate::{EditorContext, NodeEditorFrame};
use dear_imgui_rs::Ui;

/// Extension methods for starting an imgui-node-editor frame from a Dear ImGui [`Ui`].
pub trait NodeEditorUiExt {
    fn node_editor<'ui>(
        &'ui self,
        editor: &'ui EditorContext,
        id: impl AsRef<str>,
        size: [f32; 2],
    ) -> NodeEditorFrame<'ui>;
}

impl NodeEditorUiExt for Ui {
    fn node_editor<'ui>(
        &'ui self,
        editor: &'ui EditorContext,
        id: impl AsRef<str>,
        size: [f32; 2],
    ) -> NodeEditorFrame<'ui> {
        NodeEditorFrame::new(self, editor, id, size)
    }
}
