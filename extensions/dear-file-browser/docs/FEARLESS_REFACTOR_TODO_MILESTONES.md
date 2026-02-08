# Fearless Refactor: TODO & Milestones (Trackable Roadmap)

This document is a detailed, trackable plan to refactor `dear-file-browser` into an IGFD-grade file dialog system while keeping a Rust-first architecture.

Conventions used here:

- **Milestone**: a deliverable increment; should be mergeable.
- **Epic**: a cohesive chunk of work within a milestone.
- **Task**: a concrete item with acceptance criteria.
- **Exit criteria**: what must be true to close the milestone.

---

## Status Snapshot (2026-02-08)

Current parity status vs IGFD (excluding C API by product decision):

- Completed parity scope (non-C API): host split (`draw_contents` + window/modal/embed), Open/Display split (`DialogManager`), places persistence, selection UX, filters (wildcard/regex/collections), save policies, file operations, file styles, thumbnails, custom pane, entry-id-first selection model.
- Recently completed:
  - explicit dialog lifecycle API: `FileDialogState::open/reopen/close/is_open` (commit: `c31ab9a`)
  - ID-first cleanup for rename/delete modal state (commit: `bfc9609`)
  - centralized selected path/count readback in core (`selected_entry_paths`, `selected_entry_counts`) (commit: `a21b0df`)
  - scan-time entry hook in core/state API (`set_scan_hook` / `clear_scan_hook`) with keep/drop + mutation tests (commit: `b03632b`)
  - host-level size constraints (`min_size` / `max_size`) for window/modal configs (commit: `591cb49`)
  - parity/deviation baseline document (`docs/IGFD_PARITY_AND_DEVIATIONS.md`) (commit: `975f1dc`)
  - unified host/content entrypoints (`*_with` API) to remove redundant method matrix (commit: `60b4a12`)
  - P2 performance/async architecture design doc (scan coordinator/runtime + generation model) (docs-only)
  - Stage E scan-policy tuning (`max_batches_per_tick` + tuned presets + budget sweep baseline)
  - IGFD-like path composer: breadcrumb style + edit toggle + devices/reset + quick path selection popups
  - IGFD-like path popup: separator quick-select popup is a path table (IGFD `m_DisplayPathPopup` semantics)
  - IGFD-like places: toolbar "Places" control + show/hide + splitter-resizable pane (Standard layout), popup access in Minimal layout
  - Places pane UX polish: per-group quick add (`+`), per-place edit button (`E`), double-click-to-navigate, right-click context actions
  - Demo: default to a repo image directory for visible thumbnails; use icon+text toolbar labels to avoid missing glyph confusion
- Remaining high-priority gaps (non-C API):
  - none in core feature parity scope
- P2 design baseline:
  - published `docs/FEARLESS_REFACTOR_P2_PERF_ASYNC_DESIGN.md` for async/incremental scanning architecture

Execution plan (next implementation wave):

1. P2 Stage F: publish migration snippets and rollout notes
2. P2 Stage G: extend benchmark matrix (batch size + mixed metadata)
3. P2 Stage H: evaluate projection delta path under partial scan
4. P3 UX/config parity wave (Rust-first, capability-first):
   - dockable custom pane (right/bottom) + resizable splitter
   - IGFD-classic UI preset (layout/labels/density)
   - explicit config toggles for “IGFD flags”-like behavior (disable new-folder, hide columns, natural sort toggle, etc.)
   - 1:1 chrome polish pass: places toggle/splitter, breadcrumb composer, header spacing, icon/font guidance

Implementation note:

- For UI/behavior parity tasks, prefer a source-diff workflow against `repo-ref/ImGuiFileDialog/ImGuiFileDialog.cpp`.
  Use `docs/IGFD_SOURCE_REFERENCE_MAP.md` to quickly locate the relevant IGFD function and our corresponding implementation.

---
## Milestone 0 — Baseline & Refactor Safety Net

Goal: establish safety nets and constraints before moving code.

### Epic 0.1 — Snapshot current behaviors

- [x] Task: Document current features & limitations in `extensions/dear-file-browser/README.md`
  - Acceptance: README has a clear "Current capabilities" section and known gaps.
- [x] Task: Add core-level regression tests where feasible (no ImGui)
  - Candidate: filter matching (case-insensitive), selection finalize rules, save name empty error.
  - Acceptance: `cargo test -p dear-file-browser` passes.
- [x] Task: Add a minimal ImGui smoke test for UI drawing
  - Acceptance: calling the widget in a headless ImGui context does not panic.

### Epic 0.2 — Refactor constraints

- [x] Task: Define feature flags upfront (placeholders ok)
  - Current:
    - `imgui` (ImGui backend; includes `regex`)
    - `native-rfd` (native OS dialogs via `rfd`)
    - `tracing` (internal spans)
    - `thumbnails-image` (optional decoding via `image`)
  - Planned (as-needed):
    - `style-callback` (dynamic file style provider)
    - `filters-igfd-parser` (parse IGFD string filter syntax like `Name{...}`)
  - Acceptance: `Cargo.toml` has a documented place for new feature switches and builds without optional deps.

Exit criteria:

- Tests exist and pass.
- A "known gaps" list exists for reference during parity work.

---

## Milestone 1 — Host/Contents Split (Window decoupling)

Goal: make the file browser renderable in different hosts without changing behavior yet.

### Epic 1.1 — Extract `draw_contents()`

- [x] Task: Create `ui::draw_contents(ui, state, ...)` that draws the current UI without creating a window.
  - Acceptance: behavior identical to today when wrapped by a window host.

### Epic 1.2 — Introduce hosts (minimal)

- [x] Task: Add a `WindowHost` wrapper that calls `draw_contents()`.
  - Acceptance: public API still provides the same `show()` UX.
- [x] Task: Add an `EmbedHost` example (in docs or examples) showing embedding in a parent window/child region.
  - Acceptance: example compiles and renders.

Exit criteria:

- No logic duplication between window and embedded render.

---

## Milestone 2 — Core/UI Split (Make core testable)

Goal: move domain logic out of ImGui rendering into a core module.

### Epic 2.1 — Create core models

- [x] Task: Extract `FileDialogCore` (no ImGui types) and drive it from UI
  - Acceptance: selection/navigation/filter/sort/save policies live in core and are unit-testable.
- [x] Task: Introduce stable identities (`EntryId`) + richer metadata (`FileMeta`) + `DirSnapshot`
  - Acceptance: selection/focus/anchor no longer depend on entry base names (handles duplicates and renames robustly).
- [x] Task: Switch selection storage to IDs (`IndexSet<EntryId>`) with `focused` + `anchor`
  - Acceptance: Ctrl/Shift range semantics are covered by unit tests and remain stable under sorting/filtering.

### Epic 2.2 — Event model

- [x] Task: Define `core::Event` enums and map ImGui input to them.
  - Acceptance: core can be driven by unit tests via events.

### Epic 2.3 — State machine

- [x] Task: Core produces a deterministic `Result` and supports overwrite confirmation flow
  - Acceptance: core can be driven without ImGui to reach ConfirmOverwrite/Finished.
- [x] Task: Add explicit `handle_event` + `ViewModel` outputs (optional but recommended)
  - Acceptance: UI becomes mostly “render view + translate input to events”.

Exit criteria:

- Core contains navigation/selection/filtering decisions.
- UI is mostly rendering + input translation.

---

## Milestone 3 — DialogManager (Open/Display split + multi-instance)

Goal: enable multiple dialogs concurrently and decouple "open" from "display".

### Epic 3.1 — `DialogManager` + `DialogId`

- [x] Task: Implement multi-instance open/close with stable ids (`DialogId`)
  - Acceptance:
    - you can open two dialogs with different configuration and drive them independently
    - API supports both `open_browser(mode)` and `open_browser_with_state(state)` styles
- [x] Task: Provide manager-driven display helpers (`show_browser*` / `draw_browser_contents`)
  - Acceptance: if a dialog produces a result (confirm/cancel), it is removed from the manager.
- [x] Task: add explicit state lifecycle helpers (`FileDialogState::open/reopen/close/is_open`)
  - Acceptance: reopening follows IGFD-style `OpenDialog -> Display -> Close` semantics.

### Epic 3.2 — OpenRequest / Result typing

Note: this started as an early design sketch for a request-driven manager API. The current implementation is state-driven
(`open_browser_with_state(FileDialogState)`), which already satisfies IGFD-style multi-instance workflows.

If we still want an IGFD-like “open with config” convenience, re-scope this epic to a pure-Rust helper layer:

- [ ] Task (optional): Define `OpenRequest` as a builder-friendly snapshot that can produce a `FileDialogState`
  - Acceptance: `DialogManager::open_browser_with_request(req)` is a convenience wrapper over `open_browser_with_state`.
- [ ] Task (optional): Define a minimal `DialogResult` wrapper (or keep `Selection` + `FileDialogError`)
  - Acceptance: no breaking changes; improves readability for manager-driven call sites.

Exit criteria:

- Open/Display separation is real: opening does not render; display drives a specific id.

---

## Milestone 4 — FileSystem Abstraction (IGFD IFileSystem equivalent)

Goal: allow alternative filesystem backends and system devices integration.

### Epic 4.1 — `FileSystem` trait + default implementation

- [x] Task: Add a minimal `FileSystem` trait (`read_dir`, `canonicalize`, `metadata`, `create_dir`)
  - Acceptance: default uses `std::fs` and tests can swap in a mock filesystem.

### Epic 4.2 — Replace direct `std::fs` usage in core/UI

- [x] Task: all filesystem accesses go through `FileSystem`
  - Acceptance: no `std::fs::read_dir` inside UI module.

### Epic 4.3 — Expand filesystem operations (parity enabler)

- [x] Task: extend `FileSystem` with basic file operations:
  - `rename`, `remove_file`, `remove_dir`
  - Acceptance: the UI can implement rename/delete/new-folder without `std::fs` calls.
- [x] Task: add recursive directory delete (opt-in)
  - Add `remove_dir_all` to `FileSystem`
  - Expose a "Recursive" toggle in the Delete confirmation modal
  - Acceptance: deleting a non-empty directory succeeds when recursive is enabled (covered by a test).
- [ ] Task: extend `FileSystem` with “quality of life” operations (as needed):
  - `create_dir_all` (for recursive mkdir)
  - `exists` convenience (or richer `metadata`)
  - Acceptance: advanced file operations don’t require leaking platform-specific IO details into UI.

Exit criteria:

- Swapping in a mock filesystem in tests is possible.

---

## Milestone 5 — Places & Persistence (Bookmarks)

Goal: implement an IGFD-like Places pane with groups and persistence.

### Epic 5.1 — Data model + serialization

- [x] Task: `PlacesStore`, `PlacesGroup`, `PlaceItem`
- [x] Task: `serialize()/deserialize()` (compact string format for now)
  - Acceptance: roundtrip test passes.

### Epic 5.2 — UI pane

- [x] Task: left pane renders groups; groups can be expanded/collapsed
- [x] Task: user-editable groups support add/remove/rename groups/items (minimum viable)
  - Acceptance: interactive editing works without panics; operations are reflected in store.
  - Notes: System places are read-only; user places are editable.

### Epic 5.3 — System devices integration

- [x] Task: default "Devices" group in Places
  - Acceptance: Windows shows drives; other OS provide at least root/home entries (best-effort).

Exit criteria:

- Places can be persisted by the application and restored.

---

## Milestone 6 — Selection UX Parity (Ctrl/Shift, Ctrl+A, keyboard, type-to-select)

Goal: bring selection and keyboard navigation close to IGFD.

### Epic 6.1 — Selection model

- [x] Task: switch to `selected: IndexSet<EntryId>` + `focused` + `anchor`
  - Acceptance: unit tests cover selection semantics.

### Epic 6.2 — Multi-select gestures

- [x] Task: Ctrl-click toggles
- [x] Task: Shift-click selects ranges
- [x] Task: Ctrl+A selects all visible
  - Acceptance: works with filtering/search (visible set only).
- [x] Task: support a selection cap (`max_selection`)
  - Notes:
    - IGFD supports `0 => infinite`, `1 => single`, `n => n files`.
    - We should support this independently from `DialogMode` (i.e. `OpenFiles` + cap = `n`).
  - Acceptance:
    - core never holds more than `max_selection` selected file entries
    - confirm is blocked (or selection is auto-trimmed deterministically) when cap would be exceeded
    - unit tests cover Ctrl/Shift behavior under the cap

### Epic 6.3 — Keyboard navigation

- [x] Task: up/down moves focus; Enter navigates/confirm; Backspace up-dir
- [x] Task: type-to-select buffer (optional)
  - Acceptance: behavior documented and tested where possible.

Exit criteria:

- Selection feels like an OS dialog for common operations.

---

## Milestone 7 — Save Mode Policies (Confirm Overwrite + Extension Policy)

Goal: add IGFD-like save behavior.

### Epic 7.1 — SavePolicy

- [x] Task: implement `SavePolicy { confirm_overwrite, extension_policy }`
- [x] Task: implement `ExtensionPolicy` modes (KeepUser, AddIfMissing, OverwriteByFilter)
  - Acceptance: unit tests for each policy.

### Epic 7.2 — Confirm overwrite state

- [x] Task: core enters `ConfirmOverwrite` state if target exists
- [x] Task: UI shows confirmation modal/popup controlled by host
  - Acceptance: can cancel overwrite and return to dialog without losing state.

### Epic 7.3 — Result convenience modes

- [x] Task: add IGFD-style convenience accessors on `Selection`
  - `file_path_name()` (GetFilePathName-like)
  - `file_name()` (GetFileName-like)
  - `selection_named_paths()` (GetSelection-like)
  - Acceptance: helper APIs are covered by unit tests and documented in README.

Exit criteria:

- Save mode matches expected behavior for overwrite confirmation and extension handling.

---

## Milestone 8 — Filter Engine (IGFD-compatible syntax, feature-gated)

Goal: support richer filter syntax and multi-layer extensions.

### Epic 8.1 — Multi-layer extension handling

- [x] Task: update matching to support `.a.b.c` suffixes and non-trivial extensions
  - Acceptance: tests cover `.vcxproj.filters` style.

### Epic 8.2 — IGFD-style filter parser (optional)

- [x] Task: support `*`/`?` wildcard tokens and `((...))` regex tokens in `FileFilter` (ImGui)
  - Acceptance: `".vcx.*"` and `"((^imgui_.*\\.rs$))"` match as expected.
- [x] Task: `FileFilter::parse_igfd(str)` with:
  - collections: `Name{...}`
  - commas with parentheses rules
  - regex `((...))` tokens preserved verbatim
  - Acceptance: parser unit tests cover examples from IGFD docs.

Exit criteria:

- Users can express IGFD-like filters or keep the simple extension list API.

---

## Milestone 9 — FileStyle System (Icons/Colors/Fonts/Tooltips)

Goal: implement IGFD-grade file styling rules.

### Epic 9.1 — Style data model

- [x] Task: `FileStyle { text_color, icon, tooltip, font_token }`
- [x] Task: matching rules: by type + ext (first-match wins)
  - Acceptance: rule engine tests.
- [x] Task: extend matching rules: by name/contains/glob/regex
- [x] Task: add optional font mapping (`font_token -> FontId`)

### Epic 9.2 — Callback-based style provider

- [x] Task: allow callback-based style provider in `FileStyleRegistry`
  - Acceptance: callback is evaluated before static rules and can style dynamically.

Exit criteria:

- UI can show folder icons, per-extension colors, and custom font mapping.

---

## Milestone 10 — Custom Pane (Per-filter widgets that can block confirm)

Goal: implement IGFD’s signature feature.

### Epic 10.1 — Pane trait & context

- [x] Task: define `CustomPane` trait and `PaneCtx`
  - includes: active filter, selection, current dir, user_data

### Epic 10.2 — Confirm gating

- [x] Task: pane returns `can_confirm` with optional message
  - Acceptance: confirm button disabled and provides feedback.

Exit criteria:

- Applications can inject arbitrary UI and validation logic.

---

## Milestone 11 — Thumbnails (Agnostic GPU lifecycle)

Goal: add thumbnails without binding to a specific renderer backend.

### Epic 11.1 — Thumbnail core pipeline

- [x] Task: define thumbnail states and LRU cache
- [x] Task: visible-driven decode requests (only decode what’s needed)
  - Acceptance: large directories do not decode everything eagerly.

### Epic 11.2 — Renderer interface

- [x] Task: define `ThumbnailRenderer` trait (upload/destroy)
- [x] Task: UI calls `maintain(renderer)` each frame (when a backend is provided)

### Epic 11.4 — Grid view

- [x] Task: add thumbnail grid view + view mode toggle (List ↔ Grid)
  - Acceptance: selection, double-click, and thumbnail requesting work in grid mode.

### Epic 11.3 — Optional decoding implementation

- [x] Task: feature `thumbnails-image` uses `image` crate for decode
  - Acceptance: decoding is feature-gated; crate builds without it.

Exit criteria:

- Thumbnails can be displayed and GPU resources are cleaned up predictably.

---

## Milestone 12 — Polish & Parity Checklist

Goal: close remaining UX gaps vs IGFD and document final API.

### Epic 12.1 — Breadcrumb advanced interactions

- [x] Task: right-click path segment manual entry
- [x] Task: separator popup for parallel directory selection

### Epic 12.2 — Directory creation (optional)

- [x] Task: "New Folder" action with validation and error handling
- [x] Task: after creating, auto-select + reveal the new folder in the list/grid
  - Acceptance: next frame scrolls the list to the created folder and keyboard navigation continues from it.
  - Follow-up note: add a first-class toggle to disable the “New Folder” UI for read-only browsing hosts.

### Epic 12.3 — Documentation & migration notes

- [x] Task: document new API and fully remove old name-based selection APIs (see `docs/API_MIGRATION_ID_SELECTION.md`)
- [ ] Task: add examples covering:
  - embedded host
  - custom pane
  - file styles
  - places persistence
  - thumbnails (if enabled)

Exit criteria:

- A parity checklist is completed with remaining known deviations documented.

---

## Milestone 13 — Validation Buttons & Host Polish

Goal: reach IGFD-grade “dialog feel” by making the bottom action row configurable and closing host gaps.

### Epic 13.1 — Validation buttons layout/config

- [x] Task: add a `ValidationButtonsConfig` to UI state:
  - placement (left/right), order (Ok/Cancel vs Cancel/Ok), inversion, per-button width
  - label overrides (e.g. "Open", "Save", "Select")
  - Acceptance: a demo shows IGFD-like button tuning without forking UI code.

### Epic 13.2 — First-class modal host (optional convenience)

- [x] Task: add `show_modal_*()` (via `ModalHostConfig`) that hosts `draw_contents()` inside a modal popup
  - Acceptance: callers can get IGFD-style modal behavior without manually wiring popup open/close logic.

Exit criteria:

- “Ok/Cancel” row is configurable and can match IGFD UX expectations.

---

## Milestone 14 — File Operations (Rename/Delete/Clipboard)

Goal: close the largest feature gap vs IGFD by adding core-supported file operations.

### Epic 14.1 — Rename

- [x] Task: add an inline rename flow (F2 / context menu)
  - Acceptance:
    - rename uses `FileSystem::rename` (no direct `std::fs`)
    - selection + focus transfer to the renamed entry
    - handles collision / invalid name with user-visible error

### Epic 14.2 — Delete

- [x] Task: add a delete flow (Del / context menu) with confirmation
  - Acceptance:
    - delete uses `remove_file` / `remove_dir` and shows a confirmation modal
    - after delete, directory cache is invalidated and selection is updated deterministically
  - Notes:
    - directory deletion supports an opt-in recursive toggle

### Epic 14.3 — Clipboard (Copy/Cut/Paste)

- [x] Task: implement copy/cut/paste (Ctrl+C / Ctrl+X / Ctrl+V + context menu)
  - Acceptance:
    - paste uses `FileSystem` operations only (no direct `std::fs`)
    - copy allocates a unique name in the destination directory (`(copy)`, `(copy 2)`, ...)
    - cut clears the clipboard after a successful paste
- [x] Task: add paste conflict resolution modal
  - Acceptance:
    - when target exists, user can choose `Overwrite` / `Skip` / `Keep Both`
    - modal supports `Apply to all conflicts` for batch pastes
    - overwrite path uses recursive delete semantics for directories

Exit criteria:

- Users can do common file management tasks without leaving the dialog.

---

## Milestone 15 — Sorting & Columns (Extension, Type, Natural)

Goal: match IGFD’s “natural sorting for filenames and extension on demand” and improve scanability.

### Epic 15.1 — Sort by extension

- [x] Task: add `SortBy::Extension` (multi-layer aware)
  - Acceptance: `.tar.gz` and `.vcxproj.filters` sort as expected (natural order used).

### Epic 15.2 - Optional columns & tuning

- [x] Task: add column visibility knobs (Size/Modified/Preview) and persist per dialog instance
  - Acceptance: callers can hide "Modified" and run a compact list view.
- [x] Task: add one-click list column layouts (`Compact` / `Balanced`)
  - Acceptance: presets update visibility + order only and preserve runtime `weight_overrides`.

### Epic 15.3 — IGFD "Type" column semantics (filter-aware)

- [x] Task: add `SortBy::Type` (IGFD-style, filter-aware dot depth)
  - Acceptance:
    - With active filter `gz`, `archive.tar.gz` displays `.gz` in the "Type" column.
    - With active filter `tar.gz`, `archive.tar.gz` displays `.tar.gz` in the "Type" column.
    - Sorting by the "Type" column matches what is displayed.
  - Notes:
    - `SortBy::Extension` remains available for "full extension" sorting (always `.tar.gz`).

Exit criteria:

- Sorting and list columns can be tuned to closely resemble IGFD setups.

---

## Milestone 16 - Final Non-C-API Parity Closure

Goal: close remaining feature gaps vs IGFD while keeping a Rust-first API.

### Epic 16.1 - Scan-time entry hook (IGFD `userFileAttributes` equivalent)

- [x] Task: add a scan hook API to mutate/drop entries during directory scan
  - Scope:
    - callback can adjust entry metadata (e.g. size/modified/name/path)
    - callback can drop an entry before it reaches filter/sort/view
  - Acceptance:
    - hook runs in core scan pipeline (filesystem-agnostic)
    - invalid mutations are handled safely
    - unit tests cover keep/drop and metadata mutation behavior
  - Notes:
    - public API exposed on both `FileDialogCore` and `FileDialogState`

### Epic 16.2 - Link/Symlink parity in metadata + style

- [x] Task: extend entry metadata with link/symlink kind
  - Scope:
    - extend filesystem metadata surface
    - add `EntryKind::Link` (or equivalent) to style matcher
  - Acceptance:
    - link entries can be styled separately from files/dirs
    - existing file/dir style matching behavior stays stable
  - Notes:
    - `FsEntry`/`FsMetadata` now include symlink metadata
    - `EntryKind::Link` + `StyleMatcher::AnyLink` are available in style registry

### Epic 16.3 - Host constraints parity

- [x] Task: add dialog size constraints to host configs
  - Scope:
    - `WindowHostConfig` / `ModalHostConfig` support `min_size` and `max_size`
  - Acceptance:
    - constraints are applied consistently in window and modal hosts
    - documented with a compact usage example
  - Notes:
    - supports one-sided constraints (`min`-only / `max`-only) with safe normalization

### Epic 16.4 - API polish and docs

- [x] Task: publish a concise parity/deviation doc (non-C-API scope)
  - Acceptance:
    - remaining deviations are explicit and intentional
    - roadmap reflects realistic post-parity improvements
  - Notes:
    - published as `docs/IGFD_PARITY_AND_DEVIATIONS.md`
    - host/content API entrypoints were simplified to `draw_contents_with` / `show_windowed_with` / `show_modal_with`

Exit criteria:

- High-priority non-C-API gaps are either closed or explicitly deferred with rationale.
- Current status: closed in core scope; only post-parity optimization items remain.

---

## Milestone 17 - P2 Performance and Async Enumeration Foundation

Goal: keep parity behavior stable while making large-directory navigation responsive and observable.

Reference design: `docs/FEARLESS_REFACTOR_P2_PERF_ASYNC_DESIGN.md`

### Epic 17.1 - Scan pipeline scaffolding (Stage A)

- [x] Task: introduce scan contracts in core (`ScanPolicy`, `ScanRequest`, `ScanBatch`, `ScanStatus`)
  - Acceptance:
    - types are internal-first and unit-testable
    - default behavior remains synchronous and deterministic
- [x] Task: add generation-based request ownership in `FileDialogCore`
  - Acceptance:
    - each rescan increments generation
    - stale generations are ignored in batch apply path

### Epic 17.2 - Optional worker runtime (Stage B)

- [x] Task: add `ScanRuntime` abstraction with `SyncRuntime` + `WorkerRuntime`
  - Acceptance:
    - sync path remains baseline fallback
    - worker path can emit partial batches
- [x] Task: add cancellation + stale-batch drop guarantees
  - Acceptance:
    - switching directories quickly does not leak stale entries
    - tests cover `N` -> `N+1` request supersession

### Epic 17.3 - Projection/selection stability under partial data (Stage C)

- [x] Task: add bounded per-frame batch apply budget
  - Acceptance:
    - UI frame budget remains stable under large scans
- [x] Task: stabilize selection/anchor/focus reconciliation during incremental updates
  - Acceptance:
    - selected IDs persist when entries remain resolvable
    - unresolved IDs are dropped deterministically

### Epic 17.4 - Observability and tuning (Stage D)

- [x] Task: add tracing events for scan/projection lifecycle
  - Acceptance:
    - key events include `scan.requested`, `scan.batch_applied`, `scan.completed`, `scan.dropped_stale_batch`
- [x] Task: add optional synthetic performance tests (10k+/50k entries)
  - Acceptance:
    - baseline numbers are recorded for before/after comparisons

### Epic 17.5 - Incremental defaults and budget tuning (Stage E)

- [x] Task: extend `ScanPolicy` incremental mode with `max_batches_per_tick`
  - Acceptance:
    - hosts can tune throughput vs frame pacing explicitly
    - normalization guards keep `batch_entries` and `max_batches_per_tick` >= 1
- [x] Task: publish tuned presets and baseline sweep (`1/2/4` batches per tick)
  - Acceptance:
    - `ScanPolicy::tuned_incremental()` available
    - benchmark record updated with sweep table and recommendation

Exit criteria:

- Incremental scan mode is available and generation-safe.
- Large directory scans no longer block UI interaction.
- Scan/projection costs are observable via tracing metrics.
- Incremental policy has documented tuned presets for throughput/frame-pacing tradeoff.

---

## Milestone 18 - P3 UX & Config Parity (IGFD-like “feel”, Rust-first)

Goal: keep capability parity while reducing “migration friction” and achieving an IGFD-like UX without adopting a C/C++ API.

This milestone is intentionally UI/config focused. It should not regress the existing non-C-API parity guarantees.

### Epic 18.1 - Dockable custom pane (right/bottom)

- [x] Task: support `CustomPaneDock::Bottom(height)` (current) and `CustomPaneDock::Right(width)`
  - Acceptance:
    - right-docked pane is resizable via splitter
    - confirm gating works identically for both docks
    - pane receives the same `CustomPaneCtx` and selection snapshot

### Epic 18.2 - IGFD-classic UI preset (one-liner)

- [x] Task: introduce an “IGFD classic” preset for `FileDialogUiState`
  - Acceptance:
    - a single call can apply labels/layout defaults close to IGFD (toolbar density, button labels, column defaults)
    - preset is opt-in and does not change existing defaults silently
  - Notes:
    - implemented as `FileDialogUiState::apply_igfd_classic_preset()` and `FileDialogState::apply_igfd_classic_preset()`

### Epic 18.3 - Explicit config toggles for IGFD-flag-like behavior

- [x] Task: add UI/core toggles for common IGFD flags (typed, Rust-first)
  - Candidates:
    - disable “New Folder” action
    - allow hiding Extension/Size/Modified/Preview columns (full column-hide parity)
    - natural sort toggle (Natural vs Lexicographic) for perf-sensitive hosts
  - Acceptance: toggles are discoverable in `FileDialogState` and/or `FileDialog` builder and covered by unit tests.

### Epic 18.4 - Places UX parity improvements (optional)

- [x] Task: support per-group metadata and separators
  - Candidates:
    - group display ordering
    - default-opened flag
    - place separators (thickness) similar to IGFD
  - Acceptance:
    - serialization uses a strict versioned compact format (`v1`) and remains forward-extensible
    - no backward compatibility requirement for pre-refactor persistence strings

### Epic 18.5 - View modes and density (optional)

- [x] Task: add a “thumbnails list” view + lightweight density presets (Rust-first)
  - Done:
    - `FileListViewMode::ThumbnailsList` (list-with-thumbs between List and Grid)
    - thumb density presets (`S/M/L`) wired to `thumbnail_size`
    - grid-only sort combo (since table headers are absent)
  - Acceptance: selection + keyboard/type-to-select remains stable across modes.

### Epic 18.6 - Address bar path input (file-dialog feel)

- [x] Task: make path navigation feel like a native file dialog while staying Rust-first
  - Done:
    - always-visible path input ("address bar") with `Go` + submit-on-Enter
    - Ctrl+L focuses the path input (keeps keyboard workflow)
    - relative paths resolve against the dialog cwd (not process cwd)
    - breadcrumbs remain available and can be used alongside manual typing
  - Acceptance: Enter to confirm selection does not trigger while editing the path input.

Exit criteria:

- Hosts can achieve an IGFD-like UX with opt-in presets/toggles.
- No regressions in existing parity and P2 performance behavior.

---

## Milestone 19 - File Dialog Chrome Polish (IGFD-like feel)

Goal: keep feature parity while making the dialog **feel** like a mature file dialog (navigation chrome, path UX, and layout details).

This milestone is UI/UX heavy and may change widget layout and transient UI state. It should not require any IGFD C/C++ API compatibility.

### Epic 19.1 - Navigation toolbar (Back/Forward/Up/Refresh)

- [x] Task: add optional navigation history and exposed actions
  - Done:
    - `CoreEvent::NavigateBack` / `NavigateForward` / `NavigateUp` / `Refresh`
    - toolbar buttons (Back/Forward/Up/Refresh) with disabled states
    - keyboard shortcuts: Alt+Left/Right (back/forward), Backspace (up), F5 (refresh)
  - Acceptance:
    - navigation via breadcrumbs/path typing updates history consistently
    - selection is cleared consistently after navigation (current behavior)

### Epic 19.2 - Address bar UX (history + completion)

- [x] Task: make the Path address bar feel like a real address bar
  - Done:
    - drop-down history (recently visited paths)
    - inline completion for directory segments (Tab)
    - input history cycling (Up/Down arrows) while editing the Path field
  - Acceptance:
    - completion is explicit (Tab) and only reads the target directory when requested
    - relative path behavior is stable across platforms

### Epic 19.3 - Toolbar density + iconography (optional)

- [x] Task: provide an IGFD-like compact top toolbar without sacrificing Rust-first defaults
  - Done:
    - `ToolbarConfig { density, icons, show_tooltips }` on `FileDialogUiState`
    - optional icon labels for common chrome actions (host-provided glyphs)
    - a single density knob that maps to FramePadding/ItemSpacing/ItemInnerSpacing in the chrome
  - Acceptance:
    - layout remains usable at 600px width and below (density helps)
    - chrome avoids hard-coded widths that break on CJK fonts

### Epic 19.4 - Bottom action row parity (Save/Open ergonomics)

- [x] Task: tune the bottom row to match common file dialog expectations
  - Done:
    - Save filename field + extension policy hints + overwrite confirmation hint
    - Open footer file field is editable (IGFD-like) and supports typed file name/path confirm when no selection exists
    - status text ("N items", selection count, current filter, scan state)
    - filter selector moved to the action row (file dialog style)
    - content region height derives from measured footer height (no hard-coded pixel constants)
    - confirm button is disabled until the dialog is confirmable (selection or typed path; non-empty save name)
  - Acceptance:
    - a demo can match IGFD’s "save" flow without forking UI code

### Epic 19.5 - Breadcrumb path composer (IGFD-style)

- [x] Task: support an IGFD-like breadcrumb composer in the Path bar
  - Done:
    - `PathBarStyle::Breadcrumbs` with auto-compression and ellipsis popup navigation
    - IGFD-like "Edit" toggle semantics (`path_input_mode`) + Ctrl+L to enter edit mode
    - optional quick parallel path selection popups (breadcrumb separators)
    - devices popup + reset-to-open-directory shortcut in breadcrumb mode
    - framed inline path composer (no child window) + end-aligned breadcrumbs for long path visibility
  - Acceptance:
    - composer remains usable on long paths (scroll-to-end + compression)
    - Ctrl+L consistently activates text edit mode for both path bar styles

### Epic 19.6 - Places pane (toggle + splitter)

- [x] Task: make Places look/behave closer to IGFD while staying Rust-first
  - Done:
    - toolbar "Places" control:
      - Standard layout: show/hide the places pane
      - Minimal layout: open a popup places view
    - Standard layout: splitter-resizable places pane width
    - Places pane UX polish:
      - per-group quick add (`+`) and per-place edit button (`E`)
      - double-click-to-navigate, hover tooltip shows full path
      - right-click context actions and modal-driven edit/import/export flows
  - Acceptance:
    - places pane can be hidden without losing navigation access
    - pane width is stable across resizes and does not break narrow layouts

Exit criteria:

- The dialog has navigation chrome that users expect from file dialogs.
- Address bar supports history or completion (at least one).
- Layout remains robust for narrow windows and CJK fonts.

---

## Milestone 20 - Host Integration & Demos (polish)

Goal: make it easy for hosts to adopt the ImGui backend with "batteries included" examples and clear integration surfaces.

### Epic 20.1 - Thumbnails integration cookbook

- [x] Task: document and stabilize thumbnail backend integration
  - Done:
    - `docs/THUMBNAILS_INTEGRATION_COOKBOOK.md`
    - reference implementation in `examples/04-integration/file_browser_imgui.rs` (Glow)
  - Acceptance: docs provide a copyable reference without requiring engine-specific glue.

### Epic 20.2 - Demo parity pack (optional)

- [x] Task: ship one example that looks/feels close to IGFD out of the box
  - Done:
    - `examples/04-integration/file_browser_imgui.rs` applies IGFD classic preset + compact chrome + thumbnails backend
    - curated filters and places defaults
  - Acceptance: screenshot/video demonstrates IGFD-like look and interactions.

Exit criteria:

- At least one example demonstrates thumbnails properly.
- Documentation covers the "host responsibilities" (thumbnails backend, icons/fonts, FS abstraction).

---

## Milestone 21 - IGFD UI Fidelity (source-diff driven)

Goal: keep the already-achieved **capability parity** while aligning the ImGui backend’s **visual language and micro-interactions** with IGFD as closely as practical.

Policy:

- IGFD source (`repo-ref/ImGuiFileDialog/ImGuiFileDialog.cpp`) is the reference for UI flow and widget semantics.
- We do **not** chase 1:1 flags or API shape; we chase user-visible behavior and layout robustness.
- Prefer derived sizing from content region; avoid hard-coded pixels unless IGFD does so intentionally.

### Epic 21.1 - IgfdClassic header compactness (no overlap under narrow widths)

- [ ] Task: match IGFD classic labels and tooltips
  - `resetButtonString` → `R`
  - `editPathButtonString` → `E`
  - `DisplayMode_*` → `FL/TL/TG`
  - Acceptance: narrow window never overlaps view buttons with path controls; tooltips remain discoverable.
- [ ] Task: align classic header separators and ordering with `m_DrawHeader()`
  - Acceptance: order is consistent with IGFD: Places / New Folder / Path composer / Display mode / Search.

### Epic 21.2 - Breadcrumb micro-interaction parity

- [ ] Task: breadcrumb right-click matches IGFD
  - Acceptance: right-click activates path edit buffer at that segment (or parent for separators) without navigating until Enter.
- [ ] Task: separator quick-select parity
  - Acceptance: click opens path popup; right-click selects parent segment for edit (IGFD `m_SetCurrentPath` behavior).

### Epic 21.3 - Places pane visual fidelity

- [ ] Task: tighten spacing + align edit affordances (IGFD-like)
  - Acceptance: group rows and per-place edit buttons remain readable at compact density; no overlap in minimal widths.
- [ ] Task: match separator thickness and group ordering defaults
  - Acceptance: separators are visually distinct and consistent; ordering mirrors IGFD’s “Bookmarks/Devices” mental model.

### Epic 21.4 - File list / thumbnails look & feel

- [ ] Task: thumbnails list row height + padding parity
  - Acceptance: TL rows align with IGFD-like image height; selection highlight and hover feel consistent.
- [ ] Task: grid selection + sort affordances parity
  - Acceptance: grid view communicates sort state clearly and remains keyboard-friendly.

Exit criteria:

- IgfdClassic header stays usable at small widths (no overlap, predictable stacking).
- Breadcrumb interactions match IGFD (right-click edit semantics, separator behavior).
- Places + file list visuals are “close enough” that users recognize the IGFD mental model immediately.

## Parity Checklist (IGFD → dear-file-browser)

Use this as a tracking table for final validation.

- [x] Multiple dialogs concurrently (manager + ids)
- [x] Open/display lifecycle helpers on state (`open/reopen/close/is_open`)
- [x] Host flexibility: window/modal/popup/embed (modal can be caller-hosted via `draw_contents`)
- [x] Places: groups + editable + persistence + devices
- [x] Selection: ctrl/shift, ctrl+a, keyboard navigation, type-to-select
- [x] Sorting: natural name ordering (e.g. `file2` < `file10`)
- [x] Save: confirm overwrite + extension policy
- [x] Filters: collections + regex (optional) + multi-layer extensions
- [x] File operations: rename/delete/copy/cut/paste (recursive delete opt-in)
- [x] File styles: by type/ext/name/regex + callback + optional font mapping
- [x] Custom pane: per filter + blocks confirm
- [x] Thumbnails: decode + GPU lifecycle + grid view
- [x] Scan-time entry callback parity (userFileAttributes-like)
- [x] Link/symlink-specific metadata + style parity
- [x] Window/modal min-max constraints parity

---

## Tracking Template (Per-MR / Per-PR)

Copy/paste for each refactor PR:

- Goal:
- Scope:
- User-visible changes:
- Risk:
- Tests:
- Follow-ups:
