use super::super::{
    ActiveEditor, EditorContext, ImNodesContext, NodeEditor, NodeEditorSetup, PostEditor,
};
use crate::{style::StyleStacks, sys};
use dear_imgui_rs::Ui;

impl<'ui> NodeEditorSetup<'ui> {
    pub(crate) fn new(
        ui: &'ui Ui,
        ctx: &'ui ImNodesContext,
        editor: Option<&EditorContext>,
    ) -> Self {
        Self {
            ui,
            scope: ctx.scope(editor),
            pending_node_options: Default::default(),
        }
    }

    /// Draws the nodes and captures interactions in the window hosting this editor.
    /// The editor and its child scopes end on return or unwind.
    ///
    /// ```compile_fail
    /// use dear_imnodes::{NodeEditor, NodeEditorSetup};
    /// fn escape<'a>(setup: NodeEditorSetup<'a>) -> &'a NodeEditor<'a> {
    ///     let mut saved = None;
    ///     setup.with_nodes(|editor| saved = Some(editor));
    ///     saved.unwrap()
    /// }
    /// ```
    pub fn with_nodes(self, draw: impl FnOnce(&NodeEditor<'_>)) -> PostEditor<'ui> {
        let Self {
            ui,
            scope,
            pending_node_options,
        } = self;
        scope
            .ctx
            .assert_no_active_frame("NodeEditorSetup::with_nodes()");
        ui.with_native_window_scope(|_| {
            // BeginNodeEditor opens a native child window. Protect its parent until EndChild,
            // and bind the drawing API to the child's own window and draw-list splitter.
            let mut minimap_hovered = Vec::new();
            {
                scope.with_bound_context(|| {
                    scope.ctx.frame_active.set(true);
                    unsafe { sys::imnodes_BeginNodeEditor() };
                });
                let frame = ActiveEditor {
                    scope: &scope,
                    submitted: Default::default(),
                    minimap: Default::default(),
                    minimap_hovered: &mut minimap_hovered,
                };
                ui.with_native_window_scope(|host| {
                    let editor = NodeEditor {
                        ui,
                        host,
                        frame: &frame,
                        pending_node_options,
                        attributes: Default::default(),
                        styles: StyleStacks::new(&scope),
                    };
                    draw(&editor);
                });
            }
            PostEditor::capture(scope, minimap_hovered)
        })
    }

    /// Draws an empty editor and returns its post-editor queries.
    pub fn end(self) -> PostEditor<'ui> {
        self.with_nodes(|_| {})
    }

    #[inline]
    pub(crate) fn with_bound_context<R>(&self, f: impl FnOnce() -> R) -> R {
        self.scope
            .ctx
            .assert_no_active_frame("NodeEditorSetup configuration");
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
