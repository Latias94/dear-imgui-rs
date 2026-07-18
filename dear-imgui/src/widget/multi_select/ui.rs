use crate::{Ui, sys};

use super::MultiSelectOptions;
use super::basic_selection::BasicSelection;
use super::requests::MultiSelectResult;
use super::scope::MultiSelectScope;
use super::storage::MultiSelectIndexStorage;

impl Ui {
    /// Run an advanced multi-select block and return an owned copy of its final requests.
    ///
    /// The scope exposes only operations that are valid between `BeginMultiSelect()` and
    /// `EndMultiSelect()`. `EndMultiSelect()` runs exactly once even if `render` panics, and the
    /// returned [`MultiSelectResult`] contains no native pointers.
    #[doc(alias = "BeginMultiSelect", alias = "EndMultiSelect")]
    pub fn with_multi_select(
        &self,
        flags: impl Into<MultiSelectOptions>,
        selection_size: Option<i32>,
        items_count: usize,
        render: impl FnOnce(&mut MultiSelectScope<'_>),
    ) -> MultiSelectResult {
        let mut scope = MultiSelectScope::new(self, flags, selection_size, items_count);
        render(&mut scope);
        scope.finish()
    }

    /// Multi-select helper for index-based storage.
    ///
    /// This wraps `BeginMultiSelect()` / `EndMultiSelect()` and applies
    /// selection requests to an index-addressable selection container.
    ///
    /// Typical usage:
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let mut selected = vec![false; 128];
    ///
    /// ui.multi_select_indexed(&mut selected, MultiSelectOptions::new(), |ui, idx, is_selected| {
    ///     ui.text(format!(
    ///         "{} {}",
    ///         if is_selected { "[x]" } else { "[ ]" },
    ///         idx
    ///     ));
    /// });
    /// ```
    ///
    /// Notes:
    /// - `storage.len()` defines `items_count`.
    /// - This helper uses the "external storage" pattern where selection is
    ///   stored entirely on the application side.
    /// - Per-item selection toggles can be queried via
    ///   [`Ui::is_item_toggled_selection`].
    #[doc(alias = "SetNextItemSelectionUserData")]
    pub fn multi_select_indexed<S, F>(
        &self,
        storage: &mut S,
        flags: impl Into<MultiSelectOptions>,
        mut render_item: F,
    ) where
        S: MultiSelectIndexStorage,
        F: FnMut(&Ui, usize, bool),
    {
        let items_count = storage.len();
        let selection_size_i32 = storage
            .selected_count_hint()
            .and_then(|n| i32::try_from(n).ok())
            .unwrap_or(-1);

        let result =
            self.with_multi_select(flags, Some(selection_size_i32), items_count, |scope| {
                scope.apply_begin_requests_indexed(storage);

                for idx in 0..items_count {
                    scope.set_next_item_selection_user_data(idx as sys::ImGuiSelectionUserData);
                    let is_selected = storage.is_selected(idx);
                    render_item(self, idx, is_selected);
                }
            });
        result.apply_requests_indexed(storage);
    }

    /// Multi-select helper for index-based storage inside an active table.
    ///
    /// This is a convenience wrapper over [`Ui::multi_select_indexed`] that
    /// automatically advances table rows and starts each row at column 0.
    ///
    /// It expects to be called between `BeginTable`/`EndTable`.
    pub fn table_multi_select_indexed<S, F>(
        &self,
        storage: &mut S,
        flags: impl Into<MultiSelectOptions>,
        mut build_row: F,
    ) where
        S: MultiSelectIndexStorage,
        F: FnMut(&Ui, usize, bool),
    {
        let row_count = storage.len();
        let selection_size_i32 = storage
            .selected_count_hint()
            .and_then(|n| i32::try_from(n).ok())
            .unwrap_or(-1);

        let result = self.with_multi_select(flags, Some(selection_size_i32), row_count, |scope| {
            scope.apply_begin_requests_indexed(storage);

            for row in 0..row_count {
                scope.set_next_item_selection_user_data(row as sys::ImGuiSelectionUserData);
                self.table_next_row();
                self.table_next_column();

                let is_selected = storage.is_selected(row);
                build_row(self, row, is_selected);
            }
        });
        result.apply_requests_indexed(storage);
    }

    /// Multi-select helper using [`BasicSelection`] as underlying storage.
    ///
    /// This variant is suitable when items are naturally identified by `ImGuiID`
    /// (e.g. stable ids for rows or tree nodes).
    ///
    /// - `items_count`: number of items in the scope.
    /// - `id_at_index`: maps `[0, items_count)` to the corresponding item id.
    /// - `render_item`: called once per index to emit widgets for that item.
    pub fn multi_select_basic<G, F>(
        &self,
        selection: &mut BasicSelection,
        flags: impl Into<MultiSelectOptions>,
        items_count: usize,
        mut id_at_index: G,
        mut render_item: F,
    ) where
        G: FnMut(usize) -> crate::Id,
        F: FnMut(&Ui, usize, crate::Id, bool),
    {
        let selection_size_i32 = i32::try_from(selection.len()).unwrap_or(-1);

        let result =
            self.with_multi_select(flags, Some(selection_size_i32), items_count, |scope| {
                scope.apply_begin_requests_basic(selection, &mut id_at_index);

                for idx in 0..items_count {
                    scope.set_next_item_selection_user_data(idx as sys::ImGuiSelectionUserData);
                    let id = id_at_index(idx);
                    let is_selected = selection.contains(id);
                    render_item(self, idx, id, is_selected);
                }
            });
        result.apply_requests_basic(selection, id_at_index);
    }
}
