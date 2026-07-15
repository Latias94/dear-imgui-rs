use crate::sys;
use crate::ui::Ui;

/// Tracks a tree node that can be popped by calling `.pop()` or by dropping
#[must_use]
#[doc(alias = "TreePop")]
pub struct TreeNodeToken<'ui> {
    _ui: &'ui Ui,
    pop_tree_node: bool,
}

impl<'ui> TreeNodeToken<'ui> {
    /// Creates a token for an explicitly pushed tree scope.
    pub(super) fn new(ui: &'ui Ui) -> Self {
        Self {
            _ui: ui,
            pop_tree_node: true,
        }
    }

    pub(super) fn from_tree_node(ui: &'ui Ui, pop_tree_node: bool) -> Self {
        Self {
            _ui: ui,
            pop_tree_node,
        }
    }

    /// Pops the tree node
    pub fn pop(self) {
        // The drop implementation will handle the actual popping
    }
}

impl Drop for TreeNodeToken<'_> {
    fn drop(&mut self) {
        self._ui.run_with_bound_context(|| unsafe {
            if self.pop_tree_node {
                sys::igTreePop();
            }
        });
    }
}
