# Fearless Refactor: TODO & Milestones (Trackable Roadmap)

This document is a detailed, trackable plan to refactor `dear-file-browser` into an IGFD-grade file dialog system while keeping a Rust-first architecture.

Conventions used here:

- **Milestone**: a deliverable increment; should be mergeable.
- **Epic**: a cohesive chunk of work within a milestone.
- **Task**: a concrete item with acceptance criteria.
- **Exit criteria**: what must be true to close the milestone.

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

- [ ] Task: Define feature flags upfront (placeholders ok)
  - `regex-filters` (optional)
  - `thumbnails` (optional)
  - `thumbnails-image` (optional, depends on `image`)
  - Acceptance: `Cargo.toml` documents future flags (even if unused initially).

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

- [ ] Task: Introduce `EntryId`, `FileMeta`, `DirSnapshot`
  - Acceptance: UI no longer stores entries as ad-hoc structs local to rendering.

### Epic 2.2 — Event model

- [ ] Task: Define `core::Event` enums and map ImGui input to them.
  - Acceptance: core can be driven by unit tests via events.

### Epic 2.3 — State machine

- [ ] Task: Create `FileDialogCore` with `handle_event`, `view`, `take_result`.
  - Acceptance: existing UI uses core view model to render the main table and breadcrumb/path edit.

Exit criteria:

- Core contains navigation/selection/filtering decisions.
- UI is mostly rendering + input translation.

---

## Milestone 3 — DialogManager (Open/Display split + multi-instance)

Goal: enable multiple dialogs concurrently and decouple "open" from "display".

### Epic 3.1 — `DialogManager` + `DialogId`

- [x] Task: Implement `DialogManager::open(req) -> DialogId`
  - Acceptance: you can open two dialogs with different configuration and drive them independently.

### Epic 3.2 — OpenRequest / Result typing

- [ ] Task: Define `OpenRequest` (mode, start dir, filters, policies, flags)
  - Acceptance: request is immutable initial config; runtime changes live in core state.
- [ ] Task: Define `DialogResult` and unify `Selection`
  - Acceptance: matches both ImGui backend and rfd backend result shapes.

Exit criteria:

- Open/Display separation is real: opening does not render; display drives a specific id.

---

## Milestone 4 — FileSystem Abstraction (IGFD IFileSystem equivalent)

Goal: allow alternative filesystem backends and system devices integration.

### Epic 4.1 — `FileSystem` trait + default implementation

- [x] Task: Add `FileSystem` trait (read_dir, drives/devices, canonicalize, exists, is_dir, mkdir if needed)
  - Acceptance: default uses `std::fs`.

### Epic 4.2 — Replace direct `std::fs` usage in core/UI

- [x] Task: all filesystem accesses go through `FileSystem`
  - Acceptance: no `std::fs::read_dir` inside UI module.

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
- [ ] Task: user-editable groups support add/remove/rename items (minimum viable)
  - Acceptance: interactive editing works without panics; operations are reflected in store.

### Epic 5.3 — System devices integration

- [x] Task: default "Devices" group from `FileSystem::drives()/devices()`
  - Acceptance: Windows shows drives; other OS provide at least root/home entries.

Exit criteria:

- Places can be persisted by the application and restored.

---

## Milestone 6 — Selection UX Parity (Ctrl/Shift, Ctrl+A, keyboard, type-to-select)

Goal: bring selection and keyboard navigation close to IGFD.

### Epic 6.1 — Selection model

- [ ] Task: switch to `selected: IndexSet<EntryId>` + `focused` + `anchor`
  - Acceptance: unit tests cover selection semantics.

### Epic 6.2 — Multi-select gestures

- [x] Task: Ctrl-click toggles
- [x] Task: Shift-click selects ranges
- [x] Task: Ctrl+A selects all visible
  - Acceptance: works with filtering/search (visible set only).

### Epic 6.3 — Keyboard navigation

- [x] Task: up/down moves focus; Enter navigates/confirm; Backspace up-dir
- [ ] Task: type-to-select buffer (optional)
  - Acceptance: behavior documented and tested where possible.

Exit criteria:

- Selection feels like an OS dialog for common operations.

---

## Milestone 7 — Save Mode Policies (Confirm Overwrite + Extension Policy)

Goal: add IGFD-like save behavior.

### Epic 7.1 — SavePolicy

- [ ] Task: implement `SavePolicy { confirm_overwrite, extension_policy }`
- [ ] Task: implement `ExtensionPolicy` modes (KeepUser, AddIfMissing, OverwriteByFilter)
  - Acceptance: unit tests for each policy.

### Epic 7.2 — Confirm overwrite state

- [ ] Task: core enters `ConfirmOverwrite` state if target exists
- [ ] Task: UI shows confirmation modal/popup controlled by host
  - Acceptance: can cancel overwrite and return to dialog without losing state.

Exit criteria:

- Save mode matches expected behavior for overwrite confirmation and extension handling.

---

## Milestone 8 — Filter Engine (IGFD-compatible syntax, feature-gated)

Goal: support richer filter syntax and multi-layer extensions.

### Epic 8.1 — Multi-layer extension handling

- [ ] Task: update matching to support `.a.b.c` suffixes and non-trivial extensions
  - Acceptance: tests cover `.vcxproj.filters` style.

### Epic 8.2 — IGFD-style filter parser (optional)

- [ ] Task: `FilterSpec::parse_igfd(str)` with:
  - collections: `Name{...}`
  - commas with parentheses rules
  - regex `((...))` behind `regex-filters`
  - Acceptance: parser unit tests cover examples from IGFD docs.

Exit criteria:

- Users can express IGFD-like filters or keep the simple extension list API.

---

## Milestone 9 — FileStyle System (Icons/Colors/Fonts/Tooltips)

Goal: implement IGFD-grade file styling rules.

### Epic 9.1 — Style data model

- [ ] Task: `FileStyle { color, icon, font_id, tooltip }`
- [ ] Task: matching rules: by type/ext/name/contains
  - Acceptance: rule engine tests.

### Epic 9.2 — Callback-based style provider

- [ ] Task: allow `Fn(&FileMeta) -> Option<FileStyle>` (feature: `style-callback`)
  - Acceptance: can style dynamically by file size/date/etc.

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

- [ ] Task: define thumbnail states and LRU cache
- [ ] Task: visible-driven decode requests (only decode what’s needed)
  - Acceptance: large directories do not decode everything eagerly.

### Epic 11.2 — Renderer interface

- [ ] Task: define `ThumbnailRenderer` trait (upload/destroy)
- [ ] Task: UI calls `maintain(renderer)` each frame

### Epic 11.3 — Optional decoding implementation

- [ ] Task: feature `thumbnails-image` uses `image` crate for decode
  - Acceptance: decoding is feature-gated; crate builds without it.

Exit criteria:

- Thumbnails can be displayed and GPU resources are cleaned up predictably.

---

## Milestone 12 — Polish & Parity Checklist

Goal: close remaining UX gaps vs IGFD and document final API.

### Epic 12.1 — Breadcrumb advanced interactions

- [ ] Task: right-click path segment manual entry
- [ ] Task: separator popup for parallel directory selection

### Epic 12.2 — Directory creation (optional)

- [ ] Task: "New Folder" action with validation and error handling

### Epic 12.3 — Documentation & migration notes

- [ ] Task: document new API, and deprecate old APIs with a clear migration path
- [ ] Task: add examples covering:
  - embedded host
  - custom pane
  - file styles
  - places persistence
  - thumbnails (if enabled)

Exit criteria:

- A parity checklist is completed with remaining known deviations documented.

---

## Parity Checklist (IGFD → dear-file-browser)

Use this as a tracking table for final validation.

- [ ] Multiple dialogs concurrently (manager + ids)
- [ ] Host flexibility: window/modal/popup/embed
- [ ] Places: groups + editable + persistence + devices
- [ ] Selection: ctrl/shift, ctrl+a, keyboard navigation, type-to-select
- [ ] Save: confirm overwrite + extension policy
- [ ] Filters: collections + regex (optional) + multi-layer extensions
- [ ] File styles: by type/ext/name/regex + callback
- [ ] Custom pane: per filter + blocks confirm
- [ ] Thumbnails: decode + GPU lifecycle + grid view

---

## Tracking Template (Per-MR / Per-PR)

Copy/paste for each refactor PR:

- Goal:
- Scope:
- User-visible changes:
- Risk:
- Tests:
- Follow-ups:
