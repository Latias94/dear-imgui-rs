use crate::sys;
use dear_imgui_rs::Ui;
use dear_imgui_rs::sys as imgui_sys;
use std::marker::PhantomData;
use std::rc::{Rc, Weak};

mod editor;
mod global;
mod post;
mod tokens;
mod ui;

pub use global::BoundEditor;
pub use post::PostEditor;
pub(crate) use tokens::AttrKind;
pub use tokens::{AttributeToken, NodeToken};

/// Global ImNodes context
pub struct Context {
    raw: *mut sys::ImNodesContext,
    imgui_ctx_raw: *mut imgui_sys::ImGuiContext,
    imgui_alive: dear_imgui_rs::ContextAliveToken,
    alive: Rc<()>,
    // ImNodes context interacts with Dear ImGui state and is not thread-safe.
    _not_send_sync: PhantomData<Rc<()>>,
}

/// An editor context allows multiple independent editors
pub struct EditorContext {
    raw: *mut sys::ImNodesEditorContext,
    bound_ctx_raw: Option<*mut sys::ImNodesContext>,
    bound_ctx_alive: Option<ImNodesContextAliveToken>,
    bound_imgui_ctx_raw: Option<*mut imgui_sys::ImGuiContext>,
    bound_imgui_alive: Option<dear_imgui_rs::ContextAliveToken>,
    // Editor contexts are also tied to global ImNodes/ImGui state and must not cross threads.
    _not_send_sync: PhantomData<Rc<()>>,
}

#[derive(Clone)]
pub(crate) struct ImNodesScope {
    imgui_ctx_raw: *mut imgui_sys::ImGuiContext,
    imgui_alive: dear_imgui_rs::ContextAliveToken,
    ctx_raw: *mut sys::ImNodesContext,
    ctx_alive: ImNodesContextAliveToken,
    editor_raw: Option<*mut sys::ImNodesEditorContext>,
}

#[must_use = "dropping the guard restores the previous Dear ImGui/ImNodes contexts"]
pub(crate) struct ImNodesScopeGuard {
    prev_imgui_ctx_raw: *mut imgui_sys::ImGuiContext,
    prev_ctx_raw: *mut sys::ImNodesContext,
    prev_editor_raw: *mut sys::ImNodesEditorContext,
    restore_imgui: bool,
    restore_ctx: bool,
    restore_editor: bool,
}

impl ImNodesScope {
    #[inline]
    pub(crate) fn bind(&self) -> ImNodesScopeGuard {
        assert!(
            self.imgui_alive.is_alive(),
            "dear-imnodes: ImGui context has been dropped"
        );
        assert!(
            self.ctx_alive.is_alive(),
            "dear-imnodes: ImNodes context has been dropped"
        );
        let prev_imgui_ctx_raw = unsafe { imgui_sys::igGetCurrentContext() };
        let prev_ctx_raw = unsafe { sys::imnodes_GetCurrentContext() };
        let prev_editor_raw = unsafe { sys::imnodes_EditorContextGetCurrent() };
        let restore_imgui = prev_imgui_ctx_raw != self.imgui_ctx_raw;
        let restore_ctx = prev_ctx_raw != self.ctx_raw;
        let restore_editor = prev_editor_raw != self.editor_raw.unwrap_or(std::ptr::null_mut());
        unsafe {
            if restore_imgui && imgui_sys::igGetCurrentContext() != self.imgui_ctx_raw {
                imgui_sys::igSetCurrentContext(self.imgui_ctx_raw);
            }
            sys::imnodes_SetImGuiContext(self.imgui_ctx_raw);
            if restore_ctx && sys::imnodes_GetCurrentContext() != self.ctx_raw {
                sys::imnodes_SetCurrentContext(self.ctx_raw);
            }
            match self.editor_raw {
                Some(ed) => sys::imnodes_EditorContextSet(ed),
                None => sys::imnodes_EditorContextResetToDefault(),
            }
        }
        ImNodesScopeGuard {
            prev_imgui_ctx_raw,
            prev_ctx_raw,
            prev_editor_raw,
            restore_imgui,
            restore_ctx,
            restore_editor,
        }
    }
}

impl Drop for ImNodesScopeGuard {
    fn drop(&mut self) {
        unsafe {
            if self.restore_ctx {
                sys::imnodes_SetCurrentContext(self.prev_ctx_raw);
            }
            if self.restore_editor {
                if self.prev_editor_raw.is_null() {
                    sys::imnodes_EditorContextResetToDefault();
                } else {
                    sys::imnodes_EditorContextSet(self.prev_editor_raw);
                }
            }
            if self.restore_imgui {
                sys::imnodes_SetImGuiContext(self.prev_imgui_ctx_raw);
                imgui_sys::igSetCurrentContext(self.prev_imgui_ctx_raw);
            }
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ImNodesContextAliveToken(Weak<()>);

impl ImNodesContextAliveToken {
    #[inline]
    pub(crate) fn is_alive(&self) -> bool {
        self.0.upgrade().is_some()
    }
}

pub(crate) struct ImGuiContextGuard {
    prev: *mut imgui_sys::ImGuiContext,
    restore: bool,
}

impl ImGuiContextGuard {
    pub(crate) fn bind(ctx: *mut imgui_sys::ImGuiContext) -> Self {
        let prev = unsafe { imgui_sys::igGetCurrentContext() };
        let restore = prev != ctx;
        unsafe {
            if restore {
                imgui_sys::igSetCurrentContext(ctx);
            }
            sys::imnodes_SetImGuiContext(ctx);
        }
        Self { prev, restore }
    }
}

impl Drop for ImGuiContextGuard {
    fn drop(&mut self) {
        if self.restore {
            unsafe {
                imgui_sys::igSetCurrentContext(self.prev);
                sys::imnodes_SetImGuiContext(self.prev);
            };
        }
    }
}

pub(crate) struct ImNodesContextGuard {
    prev: *mut sys::ImNodesContext,
    restore: bool,
}

impl ImNodesContextGuard {
    pub(crate) fn bind(ctx: *mut sys::ImNodesContext) -> Self {
        let prev = unsafe { sys::imnodes_GetCurrentContext() };
        let restore = prev != ctx;
        unsafe { sys::imnodes_SetCurrentContext(ctx) };
        Self { prev, restore }
    }
}

impl Drop for ImNodesContextGuard {
    fn drop(&mut self) {
        if self.restore {
            unsafe { sys::imnodes_SetCurrentContext(self.prev) };
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
    callback: Box<dyn FnMut(crate::NodeId) + 'a>,
}

#[cfg(test)]
mod tests {
    use super::{Context as ImNodesContext, sys};
    use crate::ImNodesExt;
    use dear_imgui_rs::sys as imgui_sys;
    use dear_imgui_rs::{BackendFlags, Context as ImGuiContext};
    use std::ptr;
    use std::sync::{Mutex, OnceLock};

    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|err| err.into_inner())
    }

    fn prepare_imgui(imgui: &mut ImGuiContext) {
        let io = imgui.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
        io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_TEXTURES);
    }

    #[test]
    fn context_drop_restores_previous_imgui_context() {
        let _guard = test_guard();
        let mut imgui = ImGuiContext::create();
        prepare_imgui(&mut imgui);
        let raw_imgui = imgui.as_raw();
        let nodes = ImNodesContext::create(&imgui);

        unsafe { imgui_sys::igSetCurrentContext(ptr::null_mut()) };
        drop(nodes);

        assert_eq!(unsafe { imgui_sys::igGetCurrentContext() }, ptr::null_mut());
        unsafe { imgui_sys::igSetCurrentContext(raw_imgui) };
    }

    #[test]
    fn context_create_and_drop_restore_previous_imnodes_context() {
        let _guard = test_guard();
        let mut imgui = ImGuiContext::create();
        prepare_imgui(&mut imgui);
        let nodes_a = ImNodesContext::create(&imgui);
        let raw_a = nodes_a.as_raw();
        unsafe { sys::imnodes_SetCurrentContext(raw_a) };

        let nodes_b = ImNodesContext::create(&imgui);

        assert_eq!(unsafe { sys::imnodes_GetCurrentContext() }, raw_a);

        unsafe { sys::imnodes_SetCurrentContext(raw_a) };
        drop(nodes_b);

        assert_eq!(unsafe { sys::imnodes_GetCurrentContext() }, raw_a);
        drop(nodes_a);
    }

    #[test]
    fn node_editor_drop_restores_previous_imgui_context() {
        let _guard = test_guard();
        let mut imgui = ImGuiContext::create();
        prepare_imgui(&mut imgui);
        let raw_imgui = imgui.as_raw();
        let nodes = ImNodesContext::create(&imgui);

        let ui = imgui.frame();
        let editor = ui.imnodes(&nodes).editor(None);

        unsafe { imgui_sys::igSetCurrentContext(ptr::null_mut()) };
        drop(editor);
        assert_eq!(unsafe { imgui_sys::igGetCurrentContext() }, ptr::null_mut());

        unsafe { imgui_sys::igSetCurrentContext(raw_imgui) };
        let _ = imgui.render();
    }

    #[test]
    fn editor_none_resets_previous_explicit_editor_to_default() {
        let _guard = test_guard();
        let mut imgui = ImGuiContext::create();
        prepare_imgui(&mut imgui);
        let nodes = ImNodesContext::create(&imgui);
        let explicit_editor = nodes.create_editor_context();

        {
            let ui = imgui.frame();
            let editor = ui.imnodes(&nodes).editor(Some(&explicit_editor));
            let _ = editor.end();
        }
        let _ = imgui.render();

        drop(explicit_editor);

        {
            let ui = imgui.frame();
            let editor = ui.imnodes(&nodes).editor(None);
            let _ = editor.end();
        }
        let _ = imgui.render();
    }

    #[test]
    fn dropping_current_explicit_editor_resets_to_default() {
        let _guard = test_guard();
        let mut imgui = ImGuiContext::create();
        prepare_imgui(&mut imgui);
        let nodes = ImNodesContext::create(&imgui);
        let explicit_editor = nodes.create_editor_context();
        let raw_nodes = nodes.as_raw();

        {
            let _bound = nodes.bind_editor(&explicit_editor);
        }

        drop(explicit_editor);

        assert_eq!(unsafe { sys::imnodes_GetCurrentContext() }, raw_nodes);
        let ui = imgui.frame();
        let editor = ui.imnodes(&nodes).editor(None);
        let _ = editor.end();
        let _ = imgui.render();
    }
}
