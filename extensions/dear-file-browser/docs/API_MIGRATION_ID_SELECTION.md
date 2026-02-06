# Selection API Migration (EntryId-first)

This document describes the breaking API changes introduced by the ID-first selection refactor in `dear-file-browser`.

## Why this change

Selection, focus, and range anchor are now modeled by stable `EntryId` values, instead of mutable base-name strings.
This makes selection behavior deterministic under sorting/filtering and robust against duplicate names.

## Breaking changes

### `CoreEvent`

Removed name-based variants:

- `CoreEvent::FocusAndSelectByName(String)`
- `CoreEvent::ReplaceSelectionByNames(Vec<String>)`

Added ID-based variant:

- `CoreEvent::ReplaceSelectionByIds(Vec<EntryId>)`

`CoreEvent::FocusAndSelectById(EntryId)` remains the canonical focus/select event.

### `FileDialogCore` public API

Removed public name-mutating methods:

- `focus_and_select_by_name(...)`
- `replace_selection_by_names(...)`

Added ID-first methods:

- `focus_and_select_by_id(EntryId)`
- `replace_selection_by_ids<I: IntoIterator<Item = EntryId>>(...)`
- `selected_entry_ids() -> Vec<EntryId>`
- `focused_entry_id() -> Option<EntryId>`
- `entry_id_by_name(&str) -> Option<EntryId>`
- `entry_name_by_id(EntryId) -> Option<&str>`

`selected_names()` and `first_selected_name()` are still available as read-only, derived views.

## Migration examples

### 1) Event-driven replacement

Before:

```rust
core.handle_event(CoreEvent::ReplaceSelectionByNames(vec!["a.txt".into()]));
```

After:

```rust
let id = core.entry_id_by_name("a.txt").expect("entry id");
core.handle_event(CoreEvent::ReplaceSelectionByIds(vec![id]));
```

### 2) Direct API replacement

Before:

```rust
core.focus_and_select_by_name("new_folder");
```

After:

```rust
if let Some(id) = core.entry_id_by_name("new_folder") {
    core.focus_and_select_by_id(id);
}
```

### 3) Read selection from IDs

```rust
for id in core.selected_entry_ids() {
    if let Some(name) = core.entry_name_by_id(id) {
        // use name/path rendering here
    }
}
```

## Notes

- Internal UI workflows that temporarily operate on names (e.g. create/rename/paste before next rescan) still use crate-private helpers.
- External API callers should treat `EntryId` as the source of truth for selection state.
