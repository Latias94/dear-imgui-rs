use super::super::{
    Context, EditorContext, EditorScope, ImNodesScope, NodeEditor, NodeEditorSetup,
    SubmittedObjects, tokens::close_node_scope,
};
use crate::sys;
use dear_imgui_rs::Ui;
use std::rc::Rc;

impl<'ui> NodeEditorSetup<'ui> {
    pub(crate) fn begin(ui: &'ui Ui, ctx: &'ui Context, editor: Option<&EditorContext>) -> Self {
        let scope = ctx.scope(editor);
        Self {
            _ui: ui,
            _ctx: ctx,
            scope,
            pending_node_options: Default::default(),
        }
    }

    /// Enter the node-submission phase.
    ///
    /// Persistent IO/style configuration is applied before native `BeginNodeEditor`. Deferred
    /// node mutations are applied later, immediately before their matching node submission.
    pub fn begin_nodes(self) -> NodeEditor<'ui> {
        self._ctx
            .assert_no_active_frame("NodeEditorSetup::begin_nodes()");
        self.scope
            .with_bound_context(|| unsafe { sys::imnodes_BeginNodeEditor() });
        self._ctx.frame_active.set(true);
        NodeEditor {
            _ui: self._ui,
            _ctx: self._ctx,
            scope: self.scope,
            ended: false,
            state: std::cell::Cell::new(EditorScope::Editor),
            pending_node_options: self.pending_node_options,
            submitted_nodes: Default::default(),
            submitted_pins: Default::default(),
            submitted_links: Default::default(),
            finalizing: std::cell::Cell::new(false),
            frame_active: &self._ctx.frame_active,
            minimap_callbacks: Vec::new(),
        }
    }

    #[inline]
    pub(crate) fn with_bound_context<R>(&self, f: impl FnOnce() -> R) -> R {
        self.scope.with_bound_context(f)
    }

    pub(crate) fn update_node_options(
        &self,
        node_id: crate::NodeId,
        update: impl FnOnce(&mut crate::NodeOptions),
    ) {
        update(
            self.pending_node_options
                .borrow_mut()
                .entry(node_id)
                .or_default(),
        );
    }
}

impl<'ui> NodeEditor<'ui> {
    #[inline]
    pub(crate) fn with_bound_context<R>(&self, f: impl FnOnce() -> R) -> R {
        self.scope.with_bound_context(f)
    }

    #[inline]
    pub(crate) fn scope(&self) -> ImNodesScope {
        self.scope.clone()
    }

    #[inline]
    pub(crate) fn require_scope(&self, expected: EditorScope, caller: &str) {
        let actual = self.state.get();
        assert_eq!(
            actual, expected,
            "dear-imnodes: {caller} requires {expected:?} scope, but {actual:?} is active"
        );
    }

    pub(crate) fn finish_native(&mut self) {
        if self.ended {
            return;
        }
        close_node_scope(&self.scope, &self.state);
        let submitted = SubmittedObjects {
            nodes: std::mem::take(self.submitted_nodes.get_mut()),
            links: std::mem::take(self.submitted_links.get_mut()),
        };
        let last = self.scope.last_submission(self._ctx);
        self.with_bound_context(|| unsafe {
            // EndNodeEditor recycles missing objects' pool slots. Selection stores slot
            // indices, so remove those entries while the previous IDs still resolve.
            let previous = last.borrow();
            for id in previous.nodes.difference(&submitted.nodes) {
                if sys::imnodes_IsNodeSelected(id.raw()) {
                    sys::imnodes_ClearNodeSelection_Int(id.raw());
                }
            }
            for id in previous.links.difference(&submitted.links) {
                if sys::imnodes_IsLinkSelected(id.raw()) {
                    sys::imnodes_ClearLinkSelection_Int(id.raw());
                }
            }
            sys::imnodes_EndNodeEditor();
        });
        last.replace(Rc::new(submitted));
        self.ended = true;
        self.state.set(EditorScope::Ended);
        self.frame_active.set(false);
    }
}
