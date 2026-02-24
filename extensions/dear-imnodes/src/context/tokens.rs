use crate::sys;

use super::{Context, ImNodesScope};

/// RAII token for a node block
pub struct NodeToken<'a> {
    pub(super) scope: ImNodesScope,
    pub(crate) _phantom: std::marker::PhantomData<&'a Context>,
}

impl NodeToken<'_> {
    pub fn title_bar<F: FnOnce()>(&self, f: F) {
        struct TitleBarToken {
            scope: ImNodesScope,
        }

        impl Drop for TitleBarToken {
            fn drop(&mut self) {
                unsafe {
                    self.scope.bind();
                    sys::imnodes_EndNodeTitleBar();
                }
            }
        }

        unsafe {
            self.scope.bind();
            sys::imnodes_BeginNodeTitleBar();
        }
        let _title_bar = TitleBarToken {
            scope: self.scope.clone(),
        };
        f();
    }

    pub fn end(self) {}
}

impl Drop for NodeToken<'_> {
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

impl AttributeToken<'_> {
    pub fn end(self) {}
}

impl Drop for AttributeToken<'_> {
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
