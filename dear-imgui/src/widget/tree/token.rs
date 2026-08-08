use crate::scope::{NativeScopePop, NativeScopeToken};
use crate::ui::Ui;

/// Tracks a tree node that can be popped by calling `.pop()` or by dropping.
///
/// Tree-node tokens share Dear ImGui's ID stack with [`crate::Ui::push_id`]. Tokens on that stack
/// must finish in reverse creation order and in their originating window `Begin` scope. Prefer a
/// tree-node closure builder for ordinary use.
#[must_use]
#[doc(alias = "TreePop")]
pub struct TreeNodeToken<'ui> {
    scope: Option<NativeScopeToken<'ui>>,
}

impl<'ui> TreeNodeToken<'ui> {
    /// Creates a token for an explicitly pushed tree scope.
    pub(super) fn new(ui: &'ui Ui) -> Self {
        Self {
            scope: Some(ui.begin_native_scope(NativeScopePop::TreePop, "TreeNodeToken")),
        }
    }

    pub(super) fn from_tree_node(ui: &'ui Ui, pop_tree_node: bool) -> Self {
        Self {
            scope: pop_tree_node
                .then(|| ui.begin_native_scope(NativeScopePop::TreePop, "TreeNodeToken")),
        }
    }

    /// Pops the tree node
    ///
    /// # Panics
    ///
    /// Panics before FFI if a later tree-node or ID token is active, or if this token is outside
    /// its originating window `Begin` scope.
    pub fn pop(self) {
        // The drop implementation will handle the actual popping
    }
}

impl Drop for TreeNodeToken<'_> {
    fn drop(&mut self) {
        if let Some(scope) = &mut self.scope {
            scope.finish();
        }
    }
}
