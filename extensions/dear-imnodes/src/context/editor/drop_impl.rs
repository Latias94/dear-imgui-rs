use super::super::NodeEditor;
use crate::sys;

impl Drop for NodeEditor<'_> {
    fn drop(&mut self) {
        if !self.ended {
            let _guard = self.bind();
            unsafe { sys::imnodes_EndNodeEditor() };
            self.ended = true;
        }
    }
}
