use crate::sys;
use dear_imgui_rs::{ContextBinding, ContextThreadLease, NativeWindowScope, Ui};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::{Rc, Weak};

mod editor;
mod global;
mod post;
mod tokens;
mod ui;

pub use global::BoundEditor;
pub use post::PostEditor;
pub use tokens::NodeToken;

/// Global ImNodes context
pub struct ImNodesContext {
    raw: *mut sys::ImNodesContext,
    imgui_binding: ContextBinding,
    alive: Rc<()>,
    frame_active: Cell<bool>,
    default_editor_submission: RefCell<Rc<SubmittedObjects>>,
    thread_lease: Rc<ContextThreadLease>,
}

/// An editor context allows multiple independent editors
pub struct EditorContext {
    inner: Rc<EditorContextInner>,
}

struct EditorContextInner {
    raw: *mut sys::ImNodesEditorContext,
    bound_ctx_raw: *mut sys::ImNodesContext,
    bound_ctx_alive: ImNodesContextAliveToken,
    imgui_binding: ContextBinding,
    last_submission: RefCell<Rc<SubmittedObjects>>,
    _thread_lease: Rc<ContextThreadLease>,
}

#[derive(Default)]
struct SubmittedObjects {
    nodes: BTreeSet<crate::NodeId>,
    links: BTreeSet<crate::LinkId>,
}

pub(crate) struct ImNodesScope<'ctx> {
    ctx: &'ctx ImNodesContext,
    editor_lease: Option<Rc<EditorContextInner>>,
}

#[must_use = "dropping the guard restores the previous ImNodes and editor contexts"]
pub(crate) struct ImNodesScopeGuard {
    _context: ImNodesContextGuard,
}

impl ImNodesScope<'_> {
    fn last_submission(&self) -> &RefCell<Rc<SubmittedObjects>> {
        self.editor_lease
            .as_ref()
            .map_or(&self.ctx.default_editor_submission, |editor| {
                &editor.last_submission
            })
    }

    #[inline]
    pub(crate) fn with_bound_context<R>(&self, f: impl FnOnce() -> R) -> R {
        self.ctx.imgui_binding.with_bound_context(|| {
            let _scope = ImNodesScopeGuard::bind(
                self.ctx.raw,
                self.editor_lease.as_ref().map(|editor| editor.raw),
            );
            f()
        })
    }
}

impl ImNodesScopeGuard {
    fn bind(
        ctx_raw: *mut sys::ImNodesContext,
        editor_raw: Option<*mut sys::ImNodesEditorContext>,
    ) -> Self {
        let context = ImNodesContextGuard::bind(ctx_raw);
        // EditorCtx belongs to the target ImNodes context. Restore its slot before
        // the context guard switches GImNodes back to the caller's context.
        SavedBinding::Editor(unsafe { sys::imnodes_EditorContextGetCurrent() }).save();
        unsafe {
            match editor_raw {
                Some(editor) => sys::imnodes_EditorContextSet(editor),
                None => sys::imnodes_EditorContextResetToDefault(),
            }
        }
        Self { _context: context }
    }
}

impl Drop for ImNodesScopeGuard {
    fn drop(&mut self) {
        SavedBinding::restore_last();
    }
}

// Destruction invalidates pending restorations before an address can be reused.
#[derive(Clone, Copy, PartialEq)]
enum SavedBinding {
    Context(*mut sys::ImNodesContext),
    Editor(*mut sys::ImNodesEditorContext),
    DefaultEditor,
}

thread_local! {
    static SAVED_BINDINGS: RefCell<Vec<SavedBinding>> = const { RefCell::new(Vec::new()) };
}

impl SavedBinding {
    fn save(self) {
        SAVED_BINDINGS.with_borrow_mut(|stack| stack.push(self));
    }

    fn restore_last() {
        let binding = SAVED_BINDINGS.with_borrow_mut(|stack| stack.pop().unwrap());
        unsafe {
            match binding {
                Self::Context(ctx) => sys::imnodes_SetCurrentContext(ctx),
                Self::Editor(editor) => sys::imnodes_EditorContextSet(editor),
                Self::DefaultEditor => sys::imnodes_EditorContextResetToDefault(),
            }
        }
    }

    fn invalidate(self, fallback: Self) {
        SAVED_BINDINGS.with_borrow_mut(|stack| {
            for binding in stack.iter_mut().filter(|binding| **binding == self) {
                *binding = fallback;
            }
        });
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
    restore: bool,
}

impl ImNodesContextGuard {
    pub(crate) fn bind(ctx: *mut sys::ImNodesContext) -> Self {
        let prev = unsafe { sys::imnodes_GetCurrentContext() };
        let restore = prev != ctx;
        if restore {
            SavedBinding::Context(prev).save();
            unsafe { sys::imnodes_SetCurrentContext(ctx) };
        }
        Self { restore }
    }
}

impl Drop for ImNodesContextGuard {
    fn drop(&mut self) {
        if self.restore {
            SavedBinding::restore_last();
        }
    }
}

/// Per-frame Ui extension entry point
pub struct NodesUi<'ui> {
    ui: &'ui Ui,
    ctx: &'ui ImNodesContext,
}

/// Pre-submission phase of a node editor frame.
///
/// Configure persistent editor IO/style and queue per-node mutations before calling
/// [`Self::with_nodes`]. Queued mutations do not touch native node records until their matching
/// [`NodeEditor::node`] call submits the node.
#[must_use = "call with_nodes() to draw the editor, or end() to draw an empty editor"]
pub struct NodeEditorSetup<'ui> {
    ui: &'ui Ui,
    scope: ImNodesScope<'ui>,
    pending_node_options: RefCell<BTreeMap<crate::NodeId, crate::NodeOptions>>,
}

struct ActiveEditor<'a> {
    scope: &'a ImNodesScope<'a>,
    submitted: RefCell<SubmittedObjects>,
    minimap: Cell<Option<(f32, crate::MiniMapLocation)>>,
    minimap_hovered: &'a mut Vec<crate::NodeId>,
}

/// The node-submission phase of an editor, borrowed inside [`NodeEditorSetup::with_nodes`].
///
/// Phase-sensitive node mutations are intentionally unavailable here because upstream ImNodes
/// requires them to run before the first `BeginNode`/`EndNode` pair. The type boundary prevents
/// safe Rust from invalidating ImNodes' submitted-node bookkeeping.
///
/// ```compile_fail
/// fn mutate_after_submission_started(
///     editor: &dear_imnodes::NodeEditor<'_>,
///     node: dear_imnodes::NodeId,
/// ) {
///     editor.set_node_pos_grid(node, [10.0, 20.0]);
/// }
/// ```
///
/// ```compile_fail
/// fn begin_attribute_outside_a_node(
///     editor: &dear_imnodes::NodeEditor<'_>,
///     pin: dear_imnodes::PinId,
/// ) {
///     editor.input_attr(pin, dear_imnodes::PinShape::CircleFilled, || {});
/// }
/// ```
pub struct NodeEditor<'a> {
    ui: &'a Ui,
    host: &'a NativeWindowScope<'a>,
    frame: &'a ActiveEditor<'a>,
    pending_node_options: RefCell<BTreeMap<crate::NodeId, crate::NodeOptions>>,
    attributes: RefCell<BTreeMap<crate::PinId, tokens::AttributeKind>>,
    pub(crate) styles: crate::style::StyleStacks<'a>,
}

#[cfg(test)]
mod tests {
    use super::{EditorContext, ImNodesContext, ImNodesContextGuard, sys};
    use crate::ImNodesExt;
    use dear_imgui_rs::Context as ImGuiContext;
    use dear_imgui_rs::sys as imgui_sys;
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
        imgui
            .font_atlas()
            .try_claim_legacy_renderer()
            .expect("headless test requires the legacy font-atlas capability")
            .build();
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
        let raw_editor_a = editor_a.raw();
        let default_a = {
            let _current = ImNodesContextGuard::bind(raw_nodes_a);
            unsafe { sys::imnodes_EditorContextGetCurrent() }
        };
        let scope_a = nodes_a.scope(Some(&editor_a));
        drop(editor_a);

        let mut imgui_b = ImGuiContext::create();
        prepare_imgui(&mut imgui_b);
        let raw_imgui_b = imgui_b.as_raw();
        let nodes_b = ImNodesContext::create(&imgui_b);
        let raw_nodes_b = nodes_b.raw;
        let editor_b = nodes_b.create_editor_context();
        let raw_editor_b = editor_b.raw();
        unsafe {
            imgui_sys::igSetCurrentContext(raw_imgui_b);
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
                let _ = nodes_b.bind_editor(&editor_b).get_panning();
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
        {
            let _current = ImNodesContextGuard::bind(raw_nodes_a);
            assert_eq!(unsafe { sys::imnodes_EditorContextGetCurrent() }, default_a);
        }

        drop(scope_a);
        drop(editor_b);
        drop(nodes_b);
        drop(imgui_b);
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

        drop(nodes);
        assert!(
            std::thread::spawn(|| dear_imgui_rs::ContextThreadLease::acquire().is_err())
                .join()
                .unwrap()
        );
        drop(editor);
    }

    #[test]
    fn dead_imnodes_drop_preserves_another_current_context() {
        let _guard = test_guard();
        let mut imgui_a = ImGuiContext::create();
        prepare_imgui(&mut imgui_a);
        let nodes_a = ImNodesContext::create(&imgui_a);
        let editor_a = nodes_a.create_editor_context();

        let mut imgui_b = ImGuiContext::create();
        prepare_imgui(&mut imgui_b);
        let raw_imgui_b = imgui_b.as_raw();
        let nodes_b = ImNodesContext::create(&imgui_b);
        let raw_nodes_b = nodes_b.raw;
        let editor_b = nodes_b.create_editor_context();
        let raw_editor_b = editor_b.raw();
        unsafe {
            imgui_sys::igSetCurrentContext(raw_imgui_b);
            sys::imnodes_SetCurrentContext(raw_nodes_b);
            sys::imnodes_EditorContextSet(raw_editor_b);
        }

        drop(imgui_a);
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

        let mut imgui_b = ImGuiContext::create();
        prepare_imgui(&mut imgui_b);
        let frame = imgui_b.begin_frame();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _ = frame.ui().imnodes(&nodes);
        }));
        assert!(result.is_err());
        drop(frame);

        drop(imgui_b);
        drop(nodes);
        drop(imgui_a);
    }

    #[test]
    fn scoped_editor_restores_previous_imgui_context() {
        let _guard = test_guard();
        let mut imgui = ImGuiContext::create();
        prepare_imgui(&mut imgui);
        let raw_imgui = imgui.as_raw();
        let nodes = ImNodesContext::create(&imgui);

        let frame = imgui.begin_frame();
        let setup = frame.ui().imnodes(&nodes).editor(None);

        unsafe { imgui_sys::igSetCurrentContext(ptr::null_mut()) };
        setup.with_nodes(|editor| editor.node(crate::NodeId::new(1), |_| {}));
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
