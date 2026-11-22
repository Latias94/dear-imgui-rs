use super::super::{NodeEditor, NodeToken, tokens::with_child};
use crate::{NodePosition, sys};

impl NodeEditor<'_> {
    /// Draws a node, its title bar, and its attributes on this editor canvas.
    ///
    /// Any configuration queued through [`crate::NodeEditorSetup`] is applied immediately before
    /// native `BeginNode`, so safe Rust cannot create a native node record without submitting it.
    pub fn node<R>(&self, id: crate::NodeId, draw: impl FnOnce(&NodeToken<'_>) -> R) -> R {
        with_child(
            self.ui,
            self.host,
            self.frame.scope,
            || {
                self.assert_submitting();
                assert!(
                    self.frame.submitted.borrow_mut().nodes.insert(id),
                    "dear-imnodes: node {id} was submitted more than once in one editor frame"
                );
                let options = self
                    .pending_node_options
                    .borrow_mut()
                    .remove(&id)
                    .unwrap_or_default();
                unsafe {
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
                }
            },
            sys::imnodes_EndNode,
            |host| draw(&NodeToken { editor: self, host }),
        )
    }

    fn assert_submitting(&self) {
        assert!(
            self.frame.minimap.get().is_none(),
            "dear-imnodes: submit nodes and links before the minimap"
        );
    }

    /// Draw a link between two pins submitted earlier in this editor frame.
    pub fn link(&self, id: crate::LinkId, start_attr: crate::PinId, end_attr: crate::PinId) {
        self.host.run(|| {
            self.assert_submitting();
            let attributes = self.attributes.borrow();
            for pin in [start_attr, end_attr] {
                assert!(
                    attributes.get(&pin).is_some_and(|attr| attr.is_pin()),
                    "dear-imnodes: link {id} requires a submitted input or output pin at {pin}"
                );
            }
            assert!(
                self.frame.submitted.borrow_mut().links.insert(id),
                "dear-imnodes: link {id} was submitted more than once in one editor frame"
            );
            self.frame.scope.with_bound_context(|| unsafe {
                sys::imnodes_Link(id.raw(), start_attr.raw(), end_attr.raw())
            });
        });
    }
}

fn vec2(value: [f32; 2]) -> sys::ImVec2_c {
    sys::ImVec2_c {
        x: value[0],
        y: value[1],
    }
}
