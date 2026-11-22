use super::super::ActiveEditor;
use crate::sys;
use std::rc::Rc;

impl Drop for ActiveEditor<'_> {
    fn drop(&mut self) {
        let submitted = std::mem::take(self.submitted.get_mut());
        let last = self.scope.last_submission();
        self.scope.with_bound_context(|| unsafe {
            // Selection stores pool indices. Remove retired IDs before EndNodeEditor recycles them.
            let previous = last.borrow();
            for id in previous.nodes.difference(&submitted.nodes) {
                if sys::imnodes_IsNodeSelected(id.raw()) {
                    sys::imnodes_ClearNodeSelection_Int(id.raw());
                }
            }
            for id in previous.links.difference(&submitted.links) {
                if sys::imnodes_IsLinkSelected(id.raw()) {
                    sys::imnodes_ClearLinkSelection_Int(id.raw());
                }
            }
            super::minimap::end_editor(self.minimap.get(), self.minimap_hovered);
        });
        last.replace(Rc::new(submitted));
        self.scope.ctx.frame_active.set(false);
    }
}
