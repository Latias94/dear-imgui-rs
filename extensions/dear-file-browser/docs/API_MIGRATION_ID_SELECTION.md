# Selection API Migration (EntryId-only)

This document describes the breaking API changes introduced by the EntryId-only selection refactor in `dear-file-browser`.

## Why this change

Selection, focus, and range anchor are now modeled by stable `EntryId` values instead of base-name strings.

Benefits:

- deterministic behavior under sorting/filtering
- robust handling for duplicate names
- cleaner state model without name-based fallback mirrors

## Breaking changes

### `CoreEvent`

Removed name-based variants:

- `CoreEvent::FocusAndSelectByName(String)`
- `CoreEvent::ReplaceSelectionByNames(Vec<String>)`

Canonical write-path variants:

- `CoreEvent::FocusAndSelectById(EntryId)`
- `CoreEvent::ReplaceSelectionByIds(Vec<EntryId>)`

### `FileDialogCore` API

Removed name-based mutating APIs:

- `focus_and_select_by_name(...)`
- `replace_selection_by_names(...)`
- `entry_id_by_name(&str) -> Option<EntryId>`
- `entry_name_by_id(EntryId) -> Option<&str>`

Canonical ID-first APIs:

- `focus_and_select_by_id(EntryId)`
- `replace_selection_by_ids<I: IntoIterator<Item = EntryId>>(...)`
- `selected_entry_ids() -> Vec<EntryId>`
- `focused_entry_id() -> Option<EntryId>`

Read-only name view APIs remain:

- `selected_names() -> Vec<String>`
- `first_selected_name() -> Option<&str>`

## Migration patterns

### 1) Event-driven replacement

Before:

```rust
core.handle_event(CoreEvent::ReplaceSelectionByNames(vec!["a.txt".into()]));
```

After:

```rust
let id = EntryId::from_path(&core.cwd.join("a.txt"));
core.handle_event(CoreEvent::ReplaceSelectionByIds(vec![id]));
```

### 2) Direct API replacement

Before:

```rust
core.focus_and_select_by_name("new_folder");
```

After:

```rust
let id = EntryId::from_path(&core.cwd.join("new_folder"));
core.focus_and_select_by_id(id);
```

### 3) Batch selection replacement

Before:

```rust
core.replace_selection_by_names(vec!["a.txt".into(), "b.txt".into()]);
```

After:

```rust
let ids = ["a.txt", "b.txt"]
    .iter()
    .map(|name| EntryId::from_path(&core.cwd.join(name)))
    .collect::<Vec<_>>();
core.replace_selection_by_ids(ids);
```

## Notes

- There is no name-based compatibility layer anymore.
- For create/rename/paste flows, select by `EntryId::from_path(...)` immediately and let next rescan resolve display names.
- `selected_names()` is a derived view from current snapshot; if an ID is not visible in the current snapshot, it is omitted from that read view while `selected_entry_ids()` still remains the source of truth.
