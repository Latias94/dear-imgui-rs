use crate::sys;
use dear_imgui_rs::{ContextBinding, Ui};
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
    imgui_binding: ContextBinding,
    alive: Rc<()>,
}

/// An editor context allows multiple independent editors
pub struct EditorContext {
    raw: *mut sys::ImNodesEditorContext,
    bound_ctx_raw: *mut sys::ImNodesContext,
    bound_ctx_alive: ImNodesContextAliveToken,
    imgui_binding: ContextBinding,
}

#[derive(Clone)]
pub(crate) struct ImNodesScope {
    imgui_binding: ContextBinding,
    ctx_raw: *mut sys::ImNodesContext,
    ctx_alive: ImNodesContextAliveToken,
    editor_raw: Option<*mut sys::ImNodesEditorContext>,
}

#[must_use = "dropping the guard restores the previous ImNodes and editor contexts"]
pub(crate) struct ImNodesScopeGuard {
    prev_ctx_raw: *mut sys::ImNodesContext,
    prev_editor_raw: *mut sys::ImNodesEditorContext,
    restore_ctx: bool,
    restore_editor: bool,
}

impl ImNodesScope {
    #[inline]
    pub(crate) fn with_bound_context<R>(&self, f: impl FnOnce() -> R) -> R {
        assert!(
            self.ctx_alive.is_alive(),
            "dear-imnodes: ImNodes context has been dropped"
        );
        self.imgui_binding.with_bound_context(|| {
            let _scope = ImNodesScopeGuard::bind(self.ctx_raw, self.editor_raw);
            f()
        })
    }
}

impl ImNodesScopeGuard {
    fn bind(
        ctx_raw: *mut sys::ImNodesContext,
        editor_raw: Option<*mut sys::ImNodesEditorContext>,
    ) -> Self {
        let prev_ctx_raw = unsafe { sys::imnodes_GetCurrentContext() };
        let prev_editor_raw = unsafe { sys::imnodes_EditorContextGetCurrent() };
        let target_editor_raw = editor_raw.unwrap_or(std::ptr::null_mut());
        let restore_ctx = prev_ctx_raw != ctx_raw;
        let restore_editor = prev_editor_raw != target_editor_raw;
        unsafe {
            if restore_ctx {
                sys::imnodes_SetCurrentContext(ctx_raw);
            }
            match editor_raw {
                Some(editor) => sys::imnodes_EditorContextSet(editor),
                None => sys::imnodes_EditorContextResetToDefault(),
            }
        }
        Self {
            prev_ctx_raw,
            prev_editor_raw,
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

    #[inline]
    pub(crate) fn same_context(&self, other: &Self) -> bool {
        self.0.ptr_eq(&other.0)
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
        if restore {
            unsafe { sys::imnodes_SetCurrentContext(ctx) };
        }
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
    // Each callback address is handed to native code until EndNodeEditor; boxing keeps it stable
    // when later callbacks grow the Vec.
    #[allow(clippy::vec_box)]
    minimap_callbacks: Vec<Box<MiniMapCallbackHolder<'ui>>>,
}

struct MiniMapCallbackHolder<'a> {
    callback: Box<dyn FnMut(crate::NodeId) + 'a>,
}

#[cfg(test)]
mod tests {
    use super::{Context as ImNodesContext, EditorContext, ImNodesScope, sys};
    use crate::ImNodesExt;
    use dear_imgui_rs::sys as imgui_sys;
    use dear_imgui_rs::{BackendFlags, Context as ImGuiContext};
    use std::cell::{Cell, RefCell};
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::ptr;
    use std::rc::Rc;
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
    fn bindings_preserve_imgui_identity() {
        let _guard = test_guard();
        let mut imgui = ImGuiContext::create();
        prepare_imgui(&mut imgui);
        let nodes = ImNodesContext::create(&imgui);
        let editor = nodes.create_editor_context();

        assert_eq!(nodes.imgui_context_id(), imgui.id());
        assert_eq!(editor.imgui_context_id(), imgui.id());
    }

    #[test]
    fn editor_from_destroyed_imnodes_owner_is_rejected() {
        let _guard = test_guard();
        let mut imgui = ImGuiContext::create();
        prepare_imgui(&mut imgui);
        let nodes_a = ImNodesContext::create(&imgui);
        let editor_a = nodes_a.create_editor_context();
        drop(nodes_a);
        let nodes_b = ImNodesContext::create(&imgui);

        let result = catch_unwind(AssertUnwindSafe(|| {
            let _ = nodes_b.bind_editor(&editor_a);
        }));
        assert!(result.is_err());

        drop(editor_a);
        drop(nodes_b);
    }

    #[test]
    fn panic_restores_previous_imgui_imnodes_and_editor_contexts() {
        let _guard = test_guard();
        let mut imgui_a = ImGuiContext::create();
        prepare_imgui(&mut imgui_a);
        let raw_imgui_a = imgui_a.as_raw();
        let nodes_a = ImNodesContext::create(&imgui_a);
        let raw_nodes_a = nodes_a.raw;
        let editor_a = nodes_a.create_editor_context();
        let raw_editor_a = editor_a.raw;
        let scope_a = ImNodesScope {
            imgui_binding: nodes_a.imgui_binding.clone(),
            ctx_raw: raw_nodes_a,
            ctx_alive: nodes_a.alive_token(),
            editor_raw: Some(raw_editor_a),
        };
        let suspended_a = imgui_a.suspend();

        let mut imgui_b = ImGuiContext::create();
        prepare_imgui(&mut imgui_b);
        let raw_imgui_b = imgui_b.as_raw();
        let nodes_b = ImNodesContext::create(&imgui_b);
        let raw_nodes_b = nodes_b.raw;
        let editor_b = nodes_b.create_editor_context();
        let raw_editor_b = editor_b.raw;
        unsafe {
            sys::imnodes_SetCurrentContext(raw_nodes_b);
            sys::imnodes_EditorContextSet(raw_editor_b);
        }

        let result = catch_unwind(AssertUnwindSafe(|| {
            scope_a.with_bound_context(|| {
                assert_eq!(unsafe { imgui_sys::igGetCurrentContext() }, raw_imgui_a);
                assert_eq!(unsafe { sys::imnodes_GetCurrentContext() }, raw_nodes_a);
                assert_eq!(
                    unsafe { sys::imnodes_EditorContextGetCurrent() },
                    raw_editor_a
                );
                panic!("intentional binding panic");
            });
        }));

        assert!(result.is_err());
        assert_eq!(unsafe { imgui_sys::igGetCurrentContext() }, raw_imgui_b);
        assert_eq!(unsafe { sys::imnodes_GetCurrentContext() }, raw_nodes_b);
        assert_eq!(
            unsafe { sys::imnodes_EditorContextGetCurrent() },
            raw_editor_b
        );

        drop(editor_b);
        drop(nodes_b);
        drop(imgui_b);
        let imgui_a = suspended_a.activate().expect("context A should reactivate");
        drop(editor_a);
        drop(nodes_a);
        drop(imgui_a);
    }

    #[test]
    fn dead_imgui_context_rejects_calls_but_imnodes_resources_still_drop() {
        let _guard = test_guard();
        let mut imgui = ImGuiContext::create();
        prepare_imgui(&mut imgui);
        let nodes = ImNodesContext::create(&imgui);
        let editor = nodes.create_editor_context();
        drop(imgui);

        let result = catch_unwind(AssertUnwindSafe(|| {
            let _ = nodes.bind_editor(&editor).get_panning();
        }));
        assert!(result.is_err());

        drop(editor);
        drop(nodes);
    }

    #[test]
    fn dead_imnodes_drop_preserves_another_current_context() {
        let _guard = test_guard();
        let mut imgui_a = ImGuiContext::create();
        prepare_imgui(&mut imgui_a);
        let nodes_a = ImNodesContext::create(&imgui_a);
        let editor_a = nodes_a.create_editor_context();
        let suspended_a = imgui_a.suspend();

        let mut imgui_b = ImGuiContext::create();
        prepare_imgui(&mut imgui_b);
        let raw_imgui_b = imgui_b.as_raw();
        let nodes_b = ImNodesContext::create(&imgui_b);
        let raw_nodes_b = nodes_b.raw;
        let editor_b = nodes_b.create_editor_context();
        let raw_editor_b = editor_b.raw;
        unsafe {
            sys::imnodes_SetCurrentContext(raw_nodes_b);
            sys::imnodes_EditorContextSet(raw_editor_b);
        }

        drop(suspended_a);
        drop(editor_a);
        drop(nodes_a);

        assert_eq!(unsafe { imgui_sys::igGetCurrentContext() }, raw_imgui_b);
        assert_eq!(unsafe { sys::imnodes_GetCurrentContext() }, raw_nodes_b);
        assert_eq!(
            unsafe { sys::imnodes_EditorContextGetCurrent() },
            raw_editor_b
        );

        drop(editor_b);
        drop(nodes_b);
        drop(imgui_b);
    }

    #[test]
    fn imnodes_resources_drop_during_imgui_teardown_without_rebinding() {
        struct Marker;
        struct DropImNodesOnQuiesce {
            editor: RefCell<Option<EditorContext>>,
            context: RefCell<Option<ImNodesContext>>,
            quiesced: Cell<bool>,
        }

        impl dear_imgui_rs::ContextAttachment for DropImNodesOnQuiesce {
            fn quiesce(
                &self,
                _context: &dear_imgui_rs::ContextTeardown<'_>,
            ) -> Result<(), dear_imgui_rs::ContextAttachmentTeardownError> {
                drop(self.editor.borrow_mut().take());
                drop(self.context.borrow_mut().take());
                self.quiesced.set(true);
                Ok(())
            }
        }

        let _guard = test_guard();
        let mut imgui = ImGuiContext::create();
        prepare_imgui(&mut imgui);
        let nodes = ImNodesContext::create(&imgui);
        let editor = nodes.create_editor_context();
        let attachment = Rc::new(DropImNodesOnQuiesce {
            editor: RefCell::new(Some(editor)),
            context: RefCell::new(Some(nodes)),
            quiesced: Cell::new(false),
        });
        let _lease = imgui
            .register_attachment::<Marker>(
                dear_imgui_rs::ContextAttachmentRole::Extension,
                attachment.clone(),
            )
            .expect("attachment should register");

        drop(imgui);

        assert!(attachment.quiesced.get());
        assert!(attachment.editor.borrow().is_none());
        assert!(attachment.context.borrow().is_none());
    }

    #[test]
    fn ui_from_another_imgui_context_is_rejected_by_identity() {
        let _guard = test_guard();
        let mut imgui_a = ImGuiContext::create();
        prepare_imgui(&mut imgui_a);
        let nodes = ImNodesContext::create(&imgui_a);
        let suspended_a = imgui_a.suspend();

        let mut imgui_b = ImGuiContext::create();
        prepare_imgui(&mut imgui_b);
        let frame = imgui_b.begin_frame();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _ = frame.ui().imnodes(&nodes);
        }));
        assert!(result.is_err());
        drop(frame);

        drop(imgui_b);
        let imgui_a = suspended_a.activate().expect("context A should reactivate");
        drop(nodes);
        drop(imgui_a);
    }

    #[test]
    fn node_editor_drop_restores_previous_imgui_context() {
        let _guard = test_guard();
        let mut imgui = ImGuiContext::create();
        prepare_imgui(&mut imgui);
        let raw_imgui = imgui.as_raw();
        let nodes = ImNodesContext::create(&imgui);

        let frame = imgui.begin_frame();
        let editor = frame.ui().imnodes(&nodes).editor(None);

        unsafe { imgui_sys::igSetCurrentContext(ptr::null_mut()) };
        drop(editor);
        assert_eq!(unsafe { imgui_sys::igGetCurrentContext() }, ptr::null_mut());

        unsafe { imgui_sys::igSetCurrentContext(raw_imgui) };
        drop(frame);
    }

    #[test]
    fn editor_none_resets_previous_explicit_editor_to_default() {
        let _guard = test_guard();
        let mut imgui = ImGuiContext::create();
        prepare_imgui(&mut imgui);
        let nodes = ImNodesContext::create(&imgui);
        let explicit_editor = nodes.create_editor_context();

        {
            let frame = imgui.begin_frame();
            let editor = frame.ui().imnodes(&nodes).editor(Some(&explicit_editor));
            let _ = editor.end();
        }

        drop(explicit_editor);

        {
            let frame = imgui.begin_frame();
            let editor = frame.ui().imnodes(&nodes).editor(None);
            let _ = editor.end();
        }
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
        let frame = imgui.begin_frame();
        let editor = frame.ui().imnodes(&nodes).editor(None);
        let _ = editor.end();
    }
}
