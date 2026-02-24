use super::super::NodeEditor;
use crate::sys;

impl<'ui> Drop for NodeEditor<'ui> {
    fn drop(&mut self) {
        if !self.ended {
            self.bind();
            unsafe { sys::imnodes_EndNodeEditor() };
            self.ended = true;
        }
    }
}
