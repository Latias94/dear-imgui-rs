use super::{Context, EditorContext, ImNodesScope, NodeEditor, NodesUi};
use dear_imgui_rs::Ui;
use dear_imgui_rs::sys as imgui_sys;

impl<'ui> NodesUi<'ui> {
    pub(crate) fn new(ui: &'ui Ui, ctx: &'ui Context) -> Self {
        // Keep ImNodes bound to the ImGui context this ImNodes context was created with.
        // This avoids accidental cross-context use when users manage multiple ImGui contexts.
        let ui_ctx_raw = ui.with_bound_context(|| unsafe { imgui_sys::igGetCurrentContext() });
        assert_eq!(
            ui_ctx_raw, ctx.imgui_ctx_raw,
            "dear-imnodes: NodesUi requires a Ui from the owning ImGui context"
        );
        let scope = ImNodesScope {
            imgui_ctx_raw: ctx.imgui_ctx_raw,
            imgui_alive: ctx.imgui_alive.clone(),
            ctx_raw: ctx.raw,
            ctx_alive: ctx.alive_token(),
            editor_raw: None,
        };
        let _guard = scope.bind();
        Self { _ui: ui, _ctx: ctx }
    }

    /// Begin a node editor with an optional EditorContext
    pub fn editor(&self, editor: Option<&'ui EditorContext>) -> NodeEditor<'ui> {
        if let Some(editor) = editor {
            if let Some(bound) = editor.bound_ctx_raw {
                assert_eq!(
                    bound, self._ctx.raw,
                    "dear-imnodes: EditorContext is bound to a different ImNodes context"
                );
            }
            if let Some(bound) = editor.bound_imgui_ctx_raw {
                assert_eq!(
                    bound, self._ctx.imgui_ctx_raw,
                    "dear-imnodes: EditorContext is bound to a different ImGui context"
                );
            }
        }
        NodeEditor::begin(self._ui, self._ctx, editor)
    }
}
