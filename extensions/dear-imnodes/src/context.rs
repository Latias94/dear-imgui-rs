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

impl ImNodesScope {
    #[inline]
    pub(crate) fn bind(&self) {
        assert!(
            self.imgui_alive.is_alive(),
            "dear-imnodes: ImGui context has been dropped"
        );
        assert!(
            self.ctx_alive.is_alive(),
            "dear-imnodes: ImNodes context has been dropped"
        );
        assert_eq!(
            unsafe { imgui_sys::igGetCurrentContext() },
            self.imgui_ctx_raw,
            "dear-imnodes: ImNodes scope must be used with the currently-active ImGui context"
        );
        unsafe {
            sys::imnodes_SetImGuiContext(self.imgui_ctx_raw);
            sys::imnodes_SetCurrentContext(self.ctx_raw);
            match self.editor_raw {
                Some(ed) => sys::imnodes_EditorContextSet(ed),
                None => sys::imnodes_EditorContextResetToDefault(),
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
    callback: Box<dyn FnMut(i32) + 'a>,
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
        GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
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
    fn node_editor_drop_rejects_wrong_imgui_context_without_switching_current() {
        let _guard = test_guard();
        let mut imgui = ImGuiContext::create();
        prepare_imgui(&mut imgui);
        let raw_imgui = imgui.as_raw();
        let nodes = ImNodesContext::create(&imgui);
        let raw_nodes = nodes.as_raw();

        let ui = imgui.frame();
        let editor = ui.imnodes(&nodes).editor(None);

        unsafe { imgui_sys::igSetCurrentContext(ptr::null_mut()) };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(editor)));
        assert_eq!(unsafe { imgui_sys::igGetCurrentContext() }, ptr::null_mut());

        unsafe {
            imgui_sys::igSetCurrentContext(raw_imgui);
            sys::imnodes_SetImGuiContext(raw_imgui);
            sys::imnodes_SetCurrentContext(raw_nodes);
            sys::imnodes_EndNodeEditor();
        }
        let _ = imgui.render();

        let panic = result.expect_err("expected NodeEditor drop on wrong ImGui context to panic");
        let message = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&'static str>().copied())
            .unwrap_or("");
        assert!(
            message.contains("ImNodes scope must be used with the currently-active ImGui context")
        );
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
            let bound = nodes.bind_editor(&explicit_editor);
            bound.set_current();
        }

        drop(explicit_editor);

        assert_eq!(unsafe { sys::imnodes_GetCurrentContext() }, raw_nodes);
        let ui = imgui.frame();
        let editor = ui.imnodes(&nodes).editor(None);
        let _ = editor.end();
        let _ = imgui.render();
    }
}
