use super::super::{Context, EditorContext, ImNodesScope, NodeEditor};
use crate::sys;
use dear_imgui_rs::Ui;

impl<'ui> NodeEditor<'ui> {
    pub(crate) fn begin(ui: &'ui Ui, ctx: &'ui Context, editor: Option<&EditorContext>) -> Self {
        let scope = ImNodesScope {
            imgui_binding: ctx.imgui_binding.clone(),
            ctx_raw: ctx.raw,
            ctx_alive: ctx.alive_token(),
            editor_raw: editor.map(|ed| ed.raw),
        };
        scope.with_bound_context(|| unsafe { sys::imnodes_BeginNodeEditor() });
        Self {
            _ui: ui,
            _ctx: ctx,
            scope,
            ended: false,
            minimap_callbacks: Vec::new(),
        }
    }

    #[inline]
    pub(crate) fn with_bound_context<R>(&self, f: impl FnOnce() -> R) -> R {
        self.scope.with_bound_context(f)
    }

    #[inline]
    pub(crate) fn scope(&self) -> ImNodesScope {
        self.scope.clone()
    }
}
