use std::collections::HashSet;

/// Index-based selection storage for multi-select helpers.
///
/// Implement this trait for your selection container (e.g. `Vec<bool>`,
/// `Vec<MyItem { selected: bool }>` or a custom type) to use
/// [`crate::Ui::multi_select_indexed`].
pub trait MultiSelectIndexStorage {
    /// Total number of items in the selection scope.
    fn len(&self) -> usize;

    /// Returns `true` if the selection scope is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns whether item at `index` is currently selected.
    fn is_selected(&self, index: usize) -> bool;

    /// Updates selection state for item at `index`.
    fn set_selected(&mut self, index: usize, selected: bool);

    /// Optional hint for current selection size.
    ///
    /// If provided, this is forwarded to `BeginMultiSelect()` to improve the
    /// behavior of shortcuts such as `ImGuiMultiSelectFlags_ClearOnEscape`.
    /// When `None` (default), the size is treated as "unknown".
    fn selected_count_hint(&self) -> Option<usize> {
        None
    }
}

impl MultiSelectIndexStorage for Vec<bool> {
    fn len(&self) -> usize {
        self.len()
    }

    fn is_selected(&self, index: usize) -> bool {
        self.get(index).copied().unwrap_or(false)
    }

    fn set_selected(&mut self, index: usize, selected: bool) {
        if index < self.len() {
            self[index] = selected;
        }
    }

    fn selected_count_hint(&self) -> Option<usize> {
        // For typical lists this is cheap enough; callers with large datasets
        // can implement the trait manually with a more efficient counter.
        Some(self.iter().filter(|&&b| b).count())
    }
}

impl MultiSelectIndexStorage for &mut [bool] {
    fn len(&self) -> usize {
        (**self).len()
    }

    fn is_selected(&self, index: usize) -> bool {
        self.get(index).copied().unwrap_or(false)
    }

    fn set_selected(&mut self, index: usize, selected: bool) {
        if index < self.len() {
            self[index] = selected;
        }
    }

    fn selected_count_hint(&self) -> Option<usize> {
        Some(self.iter().filter(|&&b| b).count())
    }
}

/// Index-based selection storage backed by a key slice + `HashSet` of selected keys.
///
/// This is convenient when your application stores selection as a set of
/// arbitrary keys (e.g. `HashSet<u32>` or `HashSet<MyId>`), but you still
/// want to drive a multi-select scope using contiguous indices.
pub struct KeySetSelection<'a, K>
where
    K: Eq + std::hash::Hash + Copy,
{
    keys: &'a [K],
    selected: &'a mut HashSet<K>,
}

impl<'a, K> KeySetSelection<'a, K>
where
    K: Eq + std::hash::Hash + Copy,
{
    /// Create a new index-based view over a key slice and a selection set.
    ///
    /// - `keys`: stable index->key mapping (e.g. your backing array).
    /// - `selected`: set of currently selected keys.
    pub fn new(keys: &'a [K], selected: &'a mut HashSet<K>) -> Self {
        Self { keys, selected }
    }
}

impl<'a, K> MultiSelectIndexStorage for KeySetSelection<'a, K>
where
    K: Eq + std::hash::Hash + Copy,
{
    fn len(&self) -> usize {
        self.keys.len()
    }

    fn is_selected(&self, index: usize) -> bool {
        self.keys
            .get(index)
            .map(|k| self.selected.contains(k))
            .unwrap_or(false)
    }

    fn set_selected(&mut self, index: usize, selected: bool) {
        if let Some(&key) = self.keys.get(index) {
            if selected {
                self.selected.insert(key);
            } else {
                self.selected.remove(&key);
            }
        }
    }

    fn selected_count_hint(&self) -> Option<usize> {
        Some(self.selected.len())
    }
}
