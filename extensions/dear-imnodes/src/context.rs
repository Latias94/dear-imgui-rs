use crate::sys;
use dear_imgui_rs::Ui;
use dear_imgui_rs::sys as imgui_sys;
use std::marker::PhantomData;
use std::rc::Rc;

mod editor;
mod global;
mod post;
mod tokens;
mod ui;

pub use post::PostEditor;
pub(crate) use tokens::AttrKind;
pub use tokens::{AttributeToken, NodeToken};

/// Global ImNodes context
pub struct Context {
    raw: *mut sys::ImNodesContext,
    imgui_ctx_raw: *mut imgui_sys::ImGuiContext,
    // ImNodes context interacts with Dear ImGui state and is not thread-safe.
    _not_send_sync: PhantomData<Rc<()>>,
}

/// An editor context allows multiple independent editors
pub struct EditorContext {
    raw: *mut sys::ImNodesEditorContext,
    // Editor contexts are also tied to global ImNodes/ImGui state and must not cross threads.
    _not_send_sync: PhantomData<Rc<()>>,
}

#[derive(Copy, Clone)]
pub(crate) struct ImNodesScope {
    imgui_ctx_raw: *mut imgui_sys::ImGuiContext,
    ctx_raw: *mut sys::ImNodesContext,
    editor_raw: Option<*mut sys::ImNodesEditorContext>,
}

impl ImNodesScope {
    #[inline]
    pub(crate) fn bind(self) {
        unsafe {
            sys::imnodes_SetImGuiContext(self.imgui_ctx_raw);
            sys::imnodes_SetCurrentContext(self.ctx_raw);
            if let Some(ed) = self.editor_raw {
                sys::imnodes_EditorContextSet(ed);
            }
        }
    }
}

/// Per-frame Ui extension entry point
pub struct NodesUi<'ui> {
    _ui: &'ui Ui,
    _ctx: &'ui Context,
}

/// RAII token for a node editor frame
pub struct NodeEditor<'ui> {
    _ui: &'ui Ui,
    _ctx: &'ui Context,
    scope: ImNodesScope,
    ended: bool,
    minimap_callbacks: Vec<Box<MiniMapCallbackHolder<'ui>>>,
}

struct MiniMapCallbackHolder<'a> {
    callback: Box<dyn FnMut(i32) + 'a>,
}
