use super::super::{AttrKind, AttributeToken, NodeEditor, NodeToken};
use crate::sys;

impl<'ui> NodeEditor<'ui> {
    /// Begin a node
    pub fn node(&self, id: i32) -> NodeToken<'_> {
        self.bind();
        unsafe { sys::imnodes_BeginNode(id) };
        NodeToken {
            scope: self.scope,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Begin an input attribute pin
    pub fn input_attr(&self, id: i32, shape: crate::PinShape) -> AttributeToken<'_> {
        self.bind();
        unsafe { sys::imnodes_BeginInputAttribute(id, shape as sys::ImNodesPinShape) };
        AttributeToken {
            kind: AttrKind::Input,
            scope: self.scope,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Begin an output attribute pin
    pub fn output_attr(&self, id: i32, shape: crate::PinShape) -> AttributeToken<'_> {
        self.bind();
        unsafe { sys::imnodes_BeginOutputAttribute(id, shape as i32) };
        AttributeToken {
            kind: AttrKind::Output,
            scope: self.scope,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Begin a static attribute region
    pub fn static_attr(&self, id: i32) -> AttributeToken<'_> {
        self.bind();
        unsafe { sys::imnodes_BeginStaticAttribute(id) };
        AttributeToken {
            kind: AttrKind::Static,
            scope: self.scope,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Draw a link between two attributes
    pub fn link(&self, id: i32, start_attr: i32, end_attr: i32) {
        self.bind();
        unsafe { sys::imnodes_Link(id, start_attr, end_attr) }
    }
}
