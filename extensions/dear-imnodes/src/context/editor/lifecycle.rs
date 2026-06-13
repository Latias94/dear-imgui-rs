use super::super::{Context, EditorContext, ImNodesScope, NodeEditor};
use crate::sys;
use dear_imgui_rs::Ui;

impl<'ui> NodeEditor<'ui> {
    pub(crate) fn begin(ui: &'ui Ui, ctx: &'ui Context, editor: Option<&EditorContext>) -> Self {
        let scope = ImNodesScope {
            imgui_ctx_raw: ctx.imgui_ctx_raw,
            imgui_alive: ctx.imgui_alive.clone(),
            ctx_raw: ctx.raw,
            ctx_alive: ctx.alive_token(),
            editor_raw: editor.map(|ed| ed.raw),
        };
        let _guard = scope.bind();
        unsafe { sys::imnodes_BeginNodeEditor() };
        Self {
            _ui: ui,
            _ctx: ctx,
            scope,
            ended: false,
            minimap_callbacks: Vec::new(),
        }
    }

    #[inline]
    pub(crate) fn bind(&self) -> super::super::ImNodesScopeGuard {
        self.scope.bind()
    }

    #[inline]
    pub(crate) fn scope(&self) -> ImNodesScope {
        self.scope.clone()
    }
}
