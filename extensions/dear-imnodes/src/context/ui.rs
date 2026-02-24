use super::{Context, EditorContext, ImNodesScope, NodeEditor, NodesUi};
use dear_imgui_rs::Ui;
use dear_imgui_rs::sys as imgui_sys;

impl<'ui> NodesUi<'ui> {
    pub(crate) fn new(ui: &'ui Ui, ctx: &'ui Context) -> Self {
        // Keep ImNodes bound to the ImGui context this ImNodes context was created with.
        // This avoids accidental cross-context use when users manage multiple ImGui contexts.
        debug_assert_eq!(
            unsafe { imgui_sys::igGetCurrentContext() },
            ctx.imgui_ctx_raw,
            "dear-imnodes: NodesUi must be used with the currently-active ImGui context"
        );
        let scope = ImNodesScope {
            imgui_ctx_raw: ctx.imgui_ctx_raw,
            imgui_alive: ctx.imgui_alive.clone(),
            ctx_raw: ctx.raw,
            editor_raw: None,
        };
        scope.bind();
        Self { _ui: ui, _ctx: ctx }
    }

    /// Begin a node editor with an optional EditorContext
    pub fn editor(&self, editor: Option<&'ui EditorContext>) -> NodeEditor<'ui> {
        NodeEditor::begin(self._ui, self._ctx, editor)
    }
}
