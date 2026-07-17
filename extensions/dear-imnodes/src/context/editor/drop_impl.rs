use super::super::NodeEditor;
use crate::sys;

impl Drop for NodeEditor<'_> {
    fn drop(&mut self) {
        if !self.ended {
            self.with_bound_context(|| unsafe { sys::imnodes_EndNodeEditor() });
            self.ended = true;
        }
    }
}
