use super::{ImNodesScope, NodeEditor};
use crate::{PinId, PinShape, sys};
use dear_imgui_rs::{NativeWindowScope, Ui};
use std::collections::btree_map::Entry;

/// A node borrowed for the duration of [`NodeEditor::node`].
/// Each title or attribute closes before rendering returns to the node.
///
/// ```compile_fail
/// use dear_imnodes::{NodeEditor, NodeId, NodeToken};
/// fn escape<'a>(editor: &'a NodeEditor<'_>) -> &'a NodeToken<'a> {
///     editor.node(NodeId::new(1), |node| node)
/// }
/// ```
pub struct NodeToken<'a> {
    pub(super) editor: &'a NodeEditor<'a>,
    pub(super) host: &'a NativeWindowScope<'a>,
}

impl NodeToken<'_> {
    pub fn title_bar<R>(&self, draw: impl FnOnce() -> R) -> R {
        with_child(
            self.editor.ui,
            self.host,
            self.editor.frame.scope,
            || unsafe { sys::imnodes_BeginNodeTitleBar() },
            sys::imnodes_EndNodeTitleBar,
            |_| draw(),
        )
    }

    /// Draws the contents of an input pin.
    pub fn input_attr<R>(&self, id: PinId, shape: PinShape, draw: impl FnOnce() -> R) -> R {
        self.attribute(id, AttributeKind::Input(shape), draw)
    }

    /// Draws the contents of an output pin.
    pub fn output_attr<R>(&self, id: PinId, shape: PinShape, draw: impl FnOnce() -> R) -> R {
        self.attribute(id, AttributeKind::Output(shape), draw)
    }

    /// Draws an attribute without a link endpoint.
    pub fn static_attr<R>(&self, id: PinId, draw: impl FnOnce() -> R) -> R {
        self.attribute(id, AttributeKind::Static, draw)
    }

    fn attribute<R>(&self, id: PinId, kind: AttributeKind, draw: impl FnOnce() -> R) -> R {
        let end = match kind {
            AttributeKind::Input(_) => sys::imnodes_EndInputAttribute,
            AttributeKind::Output(_) => sys::imnodes_EndOutputAttribute,
            AttributeKind::Static => sys::imnodes_EndStaticAttribute,
        };
        with_child(
            self.editor.ui,
            self.host,
            self.editor.frame.scope,
            || {
                let mut attributes = self.editor.attributes.borrow_mut();
                let Entry::Vacant(entry) = attributes.entry(id) else {
                    panic!(
                        "dear-imnodes: attribute {id} was submitted more than once in one editor frame"
                    );
                };
                entry.insert(kind);
                unsafe {
                    match kind {
                        AttributeKind::Input(shape) => {
                            sys::imnodes_BeginInputAttribute(id.raw(), shape as _)
                        }
                        AttributeKind::Output(shape) => {
                            sys::imnodes_BeginOutputAttribute(id.raw(), shape as _)
                        }
                        AttributeKind::Static => sys::imnodes_BeginStaticAttribute(id.raw()),
                    }
                }
            },
            end,
            |_| draw(),
        )
    }
}

#[derive(Copy, Clone)]
pub(super) enum AttributeKind {
    Input(PinShape),
    Output(PinShape),
    Static,
}

impl AttributeKind {
    pub(super) fn is_pin(self) -> bool {
        !matches!(self, Self::Static)
    }
}

struct EndChildScope<'a> {
    scope: &'a ImNodesScope<'a>,
    end: unsafe extern "C" fn(),
}

impl Drop for EndChildScope<'_> {
    fn drop(&mut self) {
        self.scope.with_bound_context(|| unsafe { (self.end)() });
    }
}

pub(super) fn with_child<R>(
    ui: &Ui,
    parent: &NativeWindowScope<'_>,
    scope: &ImNodesScope<'_>,
    begin: impl FnOnce(),
    end: unsafe extern "C" fn(),
    draw: impl FnOnce(&NativeWindowScope<'_>) -> R,
) -> R {
    parent.run(|| {
        scope.with_bound_context(begin);
        let _end = EndChildScope { scope, end };
        // A distinct identity keeps captured parent handles from re-entering a node or
        // attribute. Its boundary restores application windows/groups/IDs before native End.
        ui.with_native_window_scope(draw)
    })
}
