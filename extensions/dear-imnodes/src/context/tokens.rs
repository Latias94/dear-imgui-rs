use crate::sys;

use super::{Context, ImNodesScope};

/// RAII token for a node block
pub struct NodeToken<'a> {
    pub(super) scope: ImNodesScope,
    pub(crate) _phantom: std::marker::PhantomData<&'a Context>,
}

impl<'a> NodeToken<'a> {
    pub fn title_bar<F: FnOnce()>(&self, f: F) {
        unsafe {
            self.scope.bind();
            sys::imnodes_BeginNodeTitleBar();
        }
        f();
        unsafe {
            self.scope.bind();
            sys::imnodes_EndNodeTitleBar();
        }
    }

    pub fn end(self) {}
}

impl<'a> Drop for NodeToken<'a> {
    fn drop(&mut self) {
        unsafe {
            self.scope.bind();
            sys::imnodes_EndNode();
        }
    }
}

pub(crate) enum AttrKind {
    Input,
    Output,
    Static,
}

pub struct AttributeToken<'a> {
    pub(crate) kind: AttrKind,
    pub(super) scope: ImNodesScope,
    pub(crate) _phantom: std::marker::PhantomData<&'a Context>,
}

impl<'a> AttributeToken<'a> {
    pub fn end(self) {}
}

impl<'a> Drop for AttributeToken<'a> {
    fn drop(&mut self) {
        unsafe {
            self.scope.bind();
            match self.kind {
                AttrKind::Input => sys::imnodes_EndInputAttribute(),
                AttrKind::Output => sys::imnodes_EndOutputAttribute(),
                AttrKind::Static => sys::imnodes_EndStaticAttribute(),
            }
        }
    }
}
