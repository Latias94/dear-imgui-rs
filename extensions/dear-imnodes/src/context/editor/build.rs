use super::super::{EditorScope, NodeEditor, NodeToken};
use crate::{NodePosition, sys};

impl<'ui> NodeEditor<'ui> {
    /// Begin submitting a node.
    ///
    /// Any configuration queued through [`crate::NodeEditorSetup`] is applied immediately before
    /// native `BeginNode`, so safe Rust cannot create a native node record without submitting it.
    pub fn node(&self, id: crate::NodeId) -> NodeToken<'_> {
        self.require_scope(EditorScope::Editor, "NodeEditor::node()");
        assert!(
            !self.finalizing.get(),
            "dear-imnodes: NodeEditor::node() cannot run after minimap finalization"
        );
        assert!(
            self.submitted_nodes.borrow_mut().insert(id),
            "dear-imnodes: node {id} was submitted more than once in one editor frame"
        );

        let options = self
            .pending_node_options
            .borrow_mut()
            .remove(&id)
            .unwrap_or_default();
        self.with_bound_context(|| unsafe {
            if let Some(position) = options.position {
                match position {
                    NodePosition::Grid(position) => {
                        sys::imnodes_SetNodeGridSpacePos(id.raw(), vec2(position));
                    }
                    NodePosition::Editor(position) => {
                        sys::imnodes_SetNodeEditorSpacePos(id.raw(), vec2(position));
                    }
                    NodePosition::Screen(position) => {
                        sys::imnodes_SetNodeScreenSpacePos(id.raw(), vec2(position));
                    }
                }
            }
            if let Some(draggable) = options.draggable {
                sys::imnodes_SetNodeDraggable(id.raw(), draggable);
            }
            if options.snap_to_grid {
                sys::imnodes_SnapNodeToGrid(id.raw());
            }
            sys::imnodes_BeginNode(id.raw());
        });

        self.state.set(EditorScope::Node);
        NodeToken {
            scope: self.scope(),
            state: &self.state,
            submitted_pins: &self.submitted_pins,
            ended: false,
        }
    }

    /// Draw a link between two pins submitted earlier in this editor frame.
    pub fn link(&self, id: crate::LinkId, start_attr: crate::PinId, end_attr: crate::PinId) {
        self.require_scope(EditorScope::Editor, "NodeEditor::link()");
        assert!(
            !self.finalizing.get(),
            "dear-imnodes: NodeEditor::link() cannot run after minimap finalization"
        );
        let submitted_pins = self.submitted_pins.borrow();
        assert!(
            submitted_pins.contains(&start_attr),
            "dear-imnodes: link {id} references unsubmitted start pin {start_attr}"
        );
        assert!(
            submitted_pins.contains(&end_attr),
            "dear-imnodes: link {id} references unsubmitted end pin {end_attr}"
        );
        drop(submitted_pins);
        assert!(
            self.submitted_links.borrow_mut().insert(id),
            "dear-imnodes: link {id} was submitted more than once in one editor frame"
        );

        self.with_bound_context(|| unsafe {
            sys::imnodes_Link(id.raw(), start_attr.raw(), end_attr.raw())
        });
    }
}

fn vec2(value: [f32; 2]) -> sys::ImVec2_c {
    sys::ImVec2_c {
        x: value[0],
        y: value[1],
    }
}
