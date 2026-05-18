use super::super::{AttrKind, AttributeToken, NodeEditor, NodeToken};
use crate::sys;

impl<'ui> NodeEditor<'ui> {
    /// Begin a node
    pub fn node(&self, id: crate::NodeId) -> NodeToken<'_> {
        self.bind();
        unsafe { sys::imnodes_BeginNode(id.raw()) };
        NodeToken {
            scope: self.scope(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Begin an input attribute pin
    pub fn input_attr(&self, id: crate::PinId, shape: crate::PinShape) -> AttributeToken<'_> {
        self.bind();
        unsafe { sys::imnodes_BeginInputAttribute(id.raw(), shape as sys::ImNodesPinShape) };
        AttributeToken {
            kind: AttrKind::Input,
            scope: self.scope(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Begin an output attribute pin
    pub fn output_attr(&self, id: crate::PinId, shape: crate::PinShape) -> AttributeToken<'_> {
        self.bind();
        unsafe { sys::imnodes_BeginOutputAttribute(id.raw(), shape as sys::ImNodesPinShape) };
        AttributeToken {
            kind: AttrKind::Output,
            scope: self.scope(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Begin a static attribute region
    pub fn static_attr(&self, id: crate::PinId) -> AttributeToken<'_> {
        self.bind();
        unsafe { sys::imnodes_BeginStaticAttribute(id.raw()) };
        AttributeToken {
            kind: AttrKind::Static,
            scope: self.scope(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Draw a link between two attributes
    pub fn link(&self, id: crate::LinkId, start_attr: crate::PinId, end_attr: crate::PinId) {
        self.bind();
        unsafe { sys::imnodes_Link(id.raw(), start_attr.raw(), end_attr.raw()) }
    }
}
