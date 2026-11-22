use crate::{EditorContext, NodeEditorFrame};
use dear_imgui_rs::Ui;

/// Extension methods for starting an imgui-node-editor frame from a Dear ImGui [`Ui`].
pub trait NodeEditorUiExt {
    /// Draws a node editor in the current window. The editor ends when `draw` returns or unwinds.
    ///
    /// ```compile_fail
    /// use dear_node_editor::{EditorContext, NodeEditorUiExt};
    /// fn escape(ui: &dear_imgui_rs::Ui, ctx: &EditorContext) {
    ///     let _frame = ui.with_node_editor(ctx, "graph", [100.0; 2], |frame| frame);
    /// }
    /// ```
    fn with_node_editor<R>(
        &self,
        editor: &EditorContext,
        id: impl AsRef<str>,
        size: [f32; 2],
        draw: impl FnOnce(&NodeEditorFrame<'_>) -> R,
    ) -> R;

    /// Begins a manually balanced editor. Prefer [`Self::with_node_editor`] for scoped drawing.
    ///
    /// # Safety
    ///
    /// End or drop the returned frame exactly once in its original Dear ImGui frame, window
    /// Begin scope and drawing channel. Balance nested windows, tables, canvases and their
    /// drawing scopes before ending it, including during unwinding. Forgetting the frame
    /// leaves the host draw list and canvas in an unfinished state.
    unsafe fn node_editor<'ui>(
        &'ui self,
        editor: &'ui EditorContext,
        id: impl AsRef<str>,
        size: [f32; 2],
    ) -> NodeEditorFrame<'ui>;
}

impl NodeEditorUiExt for Ui {
    fn with_node_editor<R>(
        &self,
        editor: &EditorContext,
        id: impl AsRef<str>,
        size: [f32; 2],
        draw: impl FnOnce(&NodeEditorFrame<'_>) -> R,
    ) -> R {
        self.with_native_window_scope(|host| {
            let frame = NodeEditorFrame::new(self, editor, id, size, Some(host));
            host.run(|| draw(&frame))
        })
    }

    unsafe fn node_editor<'ui>(
        &'ui self,
        editor: &'ui EditorContext,
        id: impl AsRef<str>,
        size: [f32; 2],
    ) -> NodeEditorFrame<'ui> {
        NodeEditorFrame::new(self, editor, id, size, None)
    }
}
