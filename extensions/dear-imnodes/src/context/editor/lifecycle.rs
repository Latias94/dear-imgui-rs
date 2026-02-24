use super::super::{Context, EditorContext, ImNodesScope, NodeEditor};
use crate::sys;
use dear_imgui_rs::Ui;

impl<'ui> NodeEditor<'ui> {
    pub(crate) fn begin(ui: &'ui Ui, ctx: &'ui Context, editor: Option<&EditorContext>) -> Self {
        let scope = ImNodesScope {
            imgui_ctx_raw: ctx.imgui_ctx_raw,
            ctx_raw: ctx.raw,
            editor_raw: editor.map(|ed| ed.raw),
        };
        scope.bind();
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
    pub(crate) fn bind(&self) {
        self.scope.bind();
    }

    #[inline]
    pub(super) fn style_ptr(&self) -> *mut sys::ImNodesStyle {
        self.bind();
        let ptr = unsafe { sys::imnodes_GetStyle() };
        assert!(
            !ptr.is_null(),
            "dear-imnodes: imnodes_GetStyle returned null"
        );
        ptr
    }

    #[inline]
    pub(super) fn io_ptr(&self) -> *mut sys::ImNodesIO {
        self.bind();
        let ptr = unsafe { sys::imnodes_GetIO() };
        assert!(!ptr.is_null(), "dear-imnodes: imnodes_GetIO returned null");
        ptr
    }
}
