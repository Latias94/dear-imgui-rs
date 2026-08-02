use crate::sys;
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;

use super::{EditorScope, ImNodesScope};

/// RAII token for an active node block.
pub struct NodeToken<'a> {
    pub(crate) scope: ImNodesScope,
    pub(crate) state: &'a Cell<EditorScope>,
    pub(crate) submitted_pins: &'a RefCell<BTreeSet<crate::PinId>>,
    pub(crate) ended: bool,
}

impl NodeToken<'_> {
    pub fn title_bar<F: FnOnce()>(&self, f: F) {
        require_scope(self.state, EditorScope::Node, "NodeToken::title_bar()");
        self.scope
            .with_bound_context(|| unsafe { sys::imnodes_BeginNodeTitleBar() });
        self.state.set(EditorScope::TitleBar);
        let _title_bar = TitleBarToken {
            scope: self.scope.clone(),
            state: self.state,
        };
        f();
    }

    /// Begin an input pin within this node.
    pub fn input_attr(&self, id: crate::PinId, shape: crate::PinShape) -> AttributeToken<'_> {
        self.begin_attribute(id, AttrKind::Input, Some(shape))
    }

    /// Begin an output pin within this node.
    pub fn output_attr(&self, id: crate::PinId, shape: crate::PinShape) -> AttributeToken<'_> {
        self.begin_attribute(id, AttrKind::Output, Some(shape))
    }

    /// Begin a static attribute within this node.
    pub fn static_attr(&self, id: crate::PinId) -> AttributeToken<'_> {
        self.begin_attribute(id, AttrKind::Static, None)
    }

    fn begin_attribute(
        &self,
        id: crate::PinId,
        kind: AttrKind,
        shape: Option<crate::PinShape>,
    ) -> AttributeToken<'_> {
        require_scope(self.state, EditorScope::Node, kind.begin_caller());
        assert!(
            self.submitted_pins.borrow_mut().insert(id),
            "dear-imnodes: pin {id} was submitted more than once in one editor frame"
        );

        self.scope.with_bound_context(|| unsafe {
            match kind {
                AttrKind::Input => sys::imnodes_BeginInputAttribute(
                    id.raw(),
                    shape.expect("input pin shape") as sys::ImNodesPinShape,
                ),
                AttrKind::Output => sys::imnodes_BeginOutputAttribute(
                    id.raw(),
                    shape.expect("output pin shape") as sys::ImNodesPinShape,
                ),
                AttrKind::Static => sys::imnodes_BeginStaticAttribute(id.raw()),
            }
        });
        self.state.set(kind.scope());
        AttributeToken {
            kind,
            scope: self.scope.clone(),
            state: self.state,
            ended: false,
        }
    }

    pub fn end(mut self) {
        self.end_inner();
    }

    fn end_inner(&mut self) {
        if !self.ended {
            close_node_scope(&self.scope, self.state);
            self.ended = true;
        }
    }
}

impl Drop for NodeToken<'_> {
    fn drop(&mut self) {
        self.end_inner();
    }
}

struct TitleBarToken<'a> {
    scope: ImNodesScope,
    state: &'a Cell<EditorScope>,
}

impl Drop for TitleBarToken<'_> {
    fn drop(&mut self) {
        if self.state.get() == EditorScope::TitleBar {
            self.scope
                .with_bound_context(|| unsafe { sys::imnodes_EndNodeTitleBar() });
            self.state.set(EditorScope::Node);
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum AttrKind {
    Input,
    Output,
    Static,
}

impl AttrKind {
    fn scope(self) -> EditorScope {
        match self {
            Self::Input => EditorScope::InputAttribute,
            Self::Output => EditorScope::OutputAttribute,
            Self::Static => EditorScope::StaticAttribute,
        }
    }

    fn begin_caller(self) -> &'static str {
        match self {
            Self::Input => "NodeToken::input_attr()",
            Self::Output => "NodeToken::output_attr()",
            Self::Static => "NodeToken::static_attr()",
        }
    }

    unsafe fn end_native(self) {
        match self {
            Self::Input => unsafe { sys::imnodes_EndInputAttribute() },
            Self::Output => unsafe { sys::imnodes_EndOutputAttribute() },
            Self::Static => unsafe { sys::imnodes_EndStaticAttribute() },
        }
    }
}

pub struct AttributeToken<'a> {
    kind: AttrKind,
    scope: ImNodesScope,
    state: &'a Cell<EditorScope>,
    ended: bool,
}

impl AttributeToken<'_> {
    pub fn end(mut self) {
        self.end_inner();
    }

    fn end_inner(&mut self) {
        if !self.ended && self.state.get() == self.kind.scope() {
            self.scope
                .with_bound_context(|| unsafe { self.kind.end_native() });
            self.state.set(EditorScope::Node);
        }
        self.ended = true;
    }
}

impl Drop for AttributeToken<'_> {
    fn drop(&mut self) {
        self.end_inner();
    }
}

pub(crate) fn close_node_scope(scope: &ImNodesScope, state: &Cell<EditorScope>) {
    scope.with_bound_context(|| unsafe {
        match state.get() {
            EditorScope::TitleBar => {
                sys::imnodes_EndNodeTitleBar();
                state.set(EditorScope::Node);
            }
            EditorScope::InputAttribute => {
                sys::imnodes_EndInputAttribute();
                state.set(EditorScope::Node);
            }
            EditorScope::OutputAttribute => {
                sys::imnodes_EndOutputAttribute();
                state.set(EditorScope::Node);
            }
            EditorScope::StaticAttribute => {
                sys::imnodes_EndStaticAttribute();
                state.set(EditorScope::Node);
            }
            _ => {}
        }
        if state.get() == EditorScope::Node {
            sys::imnodes_EndNode();
            state.set(EditorScope::Editor);
        }
    });
}

fn require_scope(state: &Cell<EditorScope>, expected: EditorScope, caller: &str) {
    let actual = state.get();
    assert_eq!(
        actual, expected,
        "dear-imnodes: {caller} requires {expected:?} scope, but {actual:?} is active"
    );
}
