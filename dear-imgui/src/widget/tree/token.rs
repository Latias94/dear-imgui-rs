use crate::sys;
use crate::ui::Ui;

/// Tracks a tree node that can be popped by calling `.pop()` or by dropping
#[must_use]
pub struct TreeNodeToken<'ui> {
    _ui: &'ui Ui,
}

impl<'ui> TreeNodeToken<'ui> {
    /// Creates a new tree node token
    pub(super) fn new(ui: &'ui Ui) -> Self {
        TreeNodeToken { _ui: ui }
    }

    /// Pops the tree node
    pub fn pop(self) {
        // The drop implementation will handle the actual popping
    }
}

impl Drop for TreeNodeToken<'_> {
    fn drop(&mut self) {
        self._ui
            .run_with_bound_context(|| unsafe { sys::igTreePop() });
    }
}
