# Fearless Refactor: File Browser Architecture (IGFD-Grade)

This document proposes a "fearless refactor" of `dear-file-browser` to reach feature parity (or near-parity) with ImGuiFileDialog (IGFD) while staying idiomatic to Rust and compatible with `dear-imgui-rs`.

Scope: **the ImGui-embedded browser/dialog** (not OS-native dialogs via `rfd`, which remain a separate backend).

---

## 1. Context & Motivation

`dear-file-browser` started lightweight but is actively evolving toward an IGFD-grade dialog:

- `FileDialogCore` (domain logic) + `FileDialogUiState` (transient UI state).
- `DialogManager` for multi-instance Open/Display-style workflows.
- Places (devices + bookmarks) with export/import, custom panes, file styles, and thumbnails.
- Advanced filters (multi-layer extensions, wildcard `*`/`?`, `((...))` regex tokens) and natural sorting.

IGFD, in contrast, is a full dialog system with:

- Open/Display separation (many opens, one display per key).
- Multiple instances and strong configurability.
- Places/bookmarks with persistence.
- Custom pane callbacks that can block validation.
- File style rules (color/icon/font) by type/ext/regex/… and via callback.
- Thumbnails with GPU lifecycle callbacks.
- Rich selection and keyboard navigation behavior (Ctrl/Shift selection, Ctrl+A, type-to-select).
- Advanced filter syntax (collections, regex, multi-layer extensions, asterisk semantics).

This refactor aims to adopt IGFD’s **capabilities and concepts** while keeping:

- A Rust-first API surface.
- Renderer/backend agnostic thumbnail handling.
- Testable business logic (core) without depending on Dear ImGui types.

---

## 1.1 IGFD Parity Snapshot

This table maps IGFD’s headline feature list to the current ImGui backend of `dear-file-browser`.
The goal is *capability parity*, not necessarily API/flag parity.

Legend: ✅ done, 🟡 partial, ❌ missing / planned.

| IGFD capability | dear-file-browser | Notes / gaps |
|---|---:|---|
| Call/Display split + multi instances | ✅ | `DialogManager` + stable ids. |
| Custom pane that can block confirm | ✅ | `CustomPane` + `ConfirmGate`. |
| File styles (type/ext/name/contains/regex/…) | ✅ | Rule-based registry + callback-based provider + optional `font_token -> FontId` mapping in UI state. |
| Multi-selection (Ctrl/Shift) + Ctrl+A | ✅ | Supports optional “max N” cap (IGFD-style). |
| Keyboard navigation + type-to-select | ✅ | Works in list & grid. |
| Places: devices + bookmarks + custom groups | ✅ | Devices + bookmarks + user groups are editable (add/remove/rename); System group remains read-only. |
| Directory manual entry (right-click breadcrumb) | ✅ | Supported. |
| “Parallel directory” popup on path separator | ✅ | Supported via breadcrumb separator popup. |
| Confirm overwrite (Save) | ✅ | `SavePolicy` + modal prompt. |
| Result modes (path+name+selection, ext policies) | ✅ | `Selection { paths }` + convenience helpers: `file_path_name()`, `file_name()`, `selection_named_paths()`; extension policy matches IGFD intent. |
| Thumbnails + GPU lifecycle hooks | ✅ | Pipeline + LRU + optional `thumbnails-image` decoding + backend upload/destroy. |
| Embedded / custom host | ✅ | `draw_contents*()` + window host config + `show_modal*()` convenience. |
| Validation buttons tuning (placement/width/invert) | ✅ | `ValidationButtonsConfig` (align/order/width/labels). |
| Filter groups / collection syntax (`Name{...}`) | ✅ | `FileFilter::parse_igfd()` / `FileDialog::filters_igfd()` support `Name{...}` collections and simple lists. |
| Natural sort for extensions on demand | ✅ | “Ext” column supports sorting by full extension using natural order. |
| File operations (rename/delete/copy/paste, etc.) | ✅ | Rename + delete + copy/cut/paste exist (delete supports optional recursive directories; paste supports conflict modal with Overwrite/Skip/Keep Both). |

---

Parity status note (2026-02-06, non-C-API scope):

- Closed recently: explicit lifecycle helpers (`open/reopen/close/is_open`), ID-first rename/delete target model, selected path/count readback in core, scan-time entry hook (`set_scan_hook` / `clear_scan_hook`), link/symlink metadata+style parity (`EntryKind::Link`), host min/max constraints, and parity/deviation baseline docs.
- Remaining high-priority gaps: none in non-C-API core parity scope; only post-parity API polish remains.
- Product scope note: C API parity is intentionally excluded for this refactor wave.

---
## 2. Design Principles

1. **Decouple core logic from ImGui rendering**
   - Core should be unit-testable and deterministic.
2. **Decouple dialog contents from its host (window/modal/popup/embed)**
   - The same dialog can be embedded or hosted as a window/modal/popup.
3. **Open/Display split + multi-instance by default**
   - Enable multiple dialogs concurrently without singleton global state.
4. **Explicit extension points**
   - Custom panes, file styles, filesystem backends, thumbnails, places persistence.
5. **Feature-gated complexity**
   - Regex parsing, thumbnail decoding, async scanning should be optional cargo features.
6. **Performance by design**
   - Caching, incremental scanning, and an "asynchronous enumeration" path.

Non-goals (for early phases):

- Perfect pixel-identical UI to IGFD.
- Re-implement every IGFD flag 1:1; instead provide equivalent capabilities.

---

## 3. High-Level Architecture

### 3.1 Layering

```
┌─────────────────────────────────────────────────────────┐
│                      Host Layer                          │
│  WindowHost / ModalHost / PopupHost / EmbedHost          │
│  - sizes, focus rules, open/close, docking integration    │
└─────────────────────────────────────────────────────────┘
                ↓ calls
┌─────────────────────────────────────────────────────────┐
│                      UI Layer (ImGui)                    │
│  - translates ImGui input → core::Event                  │
│  - draws ViewModel → ImGui widgets                        │
│  - calls thumbnail renderer hooks (optional)              │
└─────────────────────────────────────────────────────────┘
                ↓ drives
┌─────────────────────────────────────────────────────────┐
│                      Core Layer (no ImGui)               │
│  DialogManager + FileDialogCore                          │
│  - state machine, selection model, filtering, sorting     │
│  - places store, persistence I/O model                    │
│  - filesystem abstraction                                │
│  - thumbnail pipeline state (decode/upload/destroy)       │
└─────────────────────────────────────────────────────────┘
```

### 3.2 Open/Display Split

`DialogManager` owns multiple dialog instances. You "open" a request and later "display" (drive + draw) per dialog.

- `open(req) -> DialogId`
- each frame: `dialog_mut(id)` + `ui.draw(dialog.view())` + `dialog.handle_event(event)`
- `take_result()` emits `Selection` only when finished.

This mirrors IGFD’s `OpenDialog(key)` + `Display(key)` pattern, but Rust-idiomatic.

---

## 4. Public API Sketch

### 4.1 Types

- `DialogId`: stable handle used by UI/host.
- `DialogMode`: OpenFile/OpenFiles/PickFolder/SaveFile.
- `OpenRequest`: initial configuration snapshot (path, filters, flags, etc.).
- `Selection`: selected paths + optional metadata.
- `DialogResult`: Ok(selection) or Err(cancelled/validation/…).

### 4.2 Manager

```rust
pub struct DialogManager {
    // multiple dialogs + shared resources (places store, thumbnail cache, etc.)
}

impl DialogManager {
    pub fn open(&mut self, req: OpenRequest) -> DialogId;
    pub fn close(&mut self, id: DialogId);
    pub fn dialog_mut(&mut self, id: DialogId) -> Option<&mut FileDialogCore>;
}
```

### 4.3 Core Dialog

```rust
pub struct FileDialogCore { /* ... */ }

impl FileDialogCore {
    pub fn handle_event(&mut self, ev: Event) -> Vec<Action>;
    pub fn view(&self) -> ViewModel;
    pub fn take_result(&mut self) -> Option<DialogResult>;
}
```

UI code becomes "thin": it translates ImGui input into `Event` and renders from `ViewModel`.

---

## 5. Core Modules (Proposed)

Suggested module layout (within `extensions/dear-file-browser/src/`):

- `core/manager.rs` — `DialogManager`, `DialogId`
- `core/dialog.rs` — `FileDialogCore` state machine
- `core/model.rs` — `Entry`, `EntryId`, `SelectionModel`, `SortSpec`, `FilterSpec`
- `core/fs.rs` — `FileSystem` trait + default `StdFileSystem`
- `core/places.rs` — `PlacesStore` + `PlacesGroup/PlaceItem` + (de)serialization models
- `core/style.rs` — `FileStyle` + `StyleProvider` + rule engine
- `core/pane.rs` — `CustomPane` trait + glue types + can-confirm semantics
- `core/thumbs.rs` — thumbnail state machine, caching, decode queue
- `ui/mod.rs` — ImGui-specific drawing + input mapping
- `hosts/*.rs` — window/modal/popup/embed hosts

Note: this can start inside the existing `src/ui/mod.rs` by extraction and gradual migration.

---

## 6. Data Model

### 6.1 Directory Entries

Represent each entry with stable identity and metadata.

- `EntryId`:
  - stable within a directory snapshot.
  - recommended: hash of `(full_path, file_type, size, modified)` or a `PathBuf`-based key with a per-snapshot counter.
- `FileMeta`:
  - `name`, `path`, `is_dir`, `is_symlink`, `size`, `modified`, `kind`, `extension_layers`, etc.

Avoid storing "selected = Vec<String>" by name. Use `EntryId` + map to `FileMeta`.

### 6.2 Selection Model (IGFD-grade)

Maintain:

- `selected: IndexSet<EntryId>`
- `focused: Option<EntryId>`
- `anchor: Option<EntryId>` (for shift-range)
- `last_click_index: Option<usize>` (optional)

Operations:

- `toggle(id)` (Ctrl-click)
- `select_range(anchor, id)` (Shift-click)
- `select_all(visible_entries)` (Ctrl+A)
- `clear()`

### 6.3 Dialog State Machine

Core states (minimum):

- `Open` (normal browsing)
- `ConfirmOverwrite { target: PathBuf }` (save mode)
- `Finished(DialogResult)`

Transitions are driven via `Event`s.

---

## 7. Event Model (UI → Core)

ImGui input should translate into domain events:

- navigation:
  - `NavigateUp`, `NavigateInto(EntryId)`, `NavigateTo(PathBuf)`
- selection:
  - `ClickEntry { id, modifiers }`, `DoubleClickEntry { id }`
  - `SelectAll`
- filtering/search:
  - `SetSearch(String)`, `SetActiveFilter(FilterId)`, `SetShowHidden(bool)`
- sorting:
  - `SetSort(SortSpec)`
- confirm/cancel:
  - `Confirm`, `Cancel`
  - `ConfirmOverwriteYes`, `ConfirmOverwriteNo`
- keyboard helper:
  - `TypeToSelectChar(char)` (optional)

This makes core testable without Dear ImGui.

---

## 8. Hosts (Window / Modal / Popup / Embed)

### 8.1 Why hosts?

Currently, `show()` always creates a window. IGFD supports:

- modal / non-modal
- embedding into a parent frame
- "NoDialog" mode (only content; host provided by caller)

### 8.2 Host responsibilities

- Decide whether/how to call `ui::draw_contents()`
- Provide min/max size constraints and default sizes
- Coordinate focus rules (initial focus, path edit focus, search focus)
- Provide unique ImGui IDs / keys

Minimal API sketch:

```rust
pub trait DialogHost {
    fn draw(&mut self, ui: &Ui, dialog: &mut FileDialogCore) -> Option<DialogResult>;
}
```

---

## 9. Places System (Bookmarks / Devices / Custom Groups)

### 9.1 Data model

- `PlacesStore` contains ordered groups.
- Each group has:
  - `user_editable`
  - `open_by_default`
  - `display_order`
  - items: (label, path, optional icon)

### 9.2 Persistence

Core provides:

- `serialize() -> String` (JSON or RON recommended)
- `deserialize(&str) -> Result<()>`

Caller decides where to store (app config file, etc.).

### 9.3 System devices

Expose via `FileSystem::drives()` / `FileSystem::devices()` to populate a default group.

---

## 10. Custom Pane (Extension Widgets)

### 10.1 Requirements

- Render arbitrary ImGui widgets based on current filter / selection / user data.
- Pane can set `can_confirm = false` to block validation.
- Pane can display inline validation errors/hints.

### 10.2 Rust API shape

Provide both:

- trait object: `Box<dyn CustomPane>`
- function callback: `FnMut(&PaneCtx, &Ui) -> PaneResult`

Core stores "pane outputs" each frame to decide confirm enablement.

---

## 11. File Styles (Color / Icon / Font / Tooltip)

### 11.1 Supported matchers

Should cover IGFD equivalents:

- by type: file/dir/link
- by extension (multi-layer extension support)
- by full name
- by substring
- by regex (feature-gated)
- by callback (dynamic)

### 11.2 Rendering integration

Core decides style metadata (logical). UI maps:

- icon string to displayed label or icon font glyph
- font id to actual ImGui font pointer (if app provides mapping)

---

## 12. Thumbnails (Agnostic Pipeline)

### 12.1 Separation of concerns

- Core:
  - decides which entries need thumbnails
  - stores decode state and LRU
  - requests decode work
- UI/Host:
  - owns GPU texture creation/destruction via a `ThumbnailRenderer`

### 12.2 States

Per entry:

- `None`
- `QueuedDecode`
- `Decoded { rgba, w, h }`
- `Uploaded { tex_id }`
- `Failed`

### 12.3 GPU lifecycle

UI/Host calls `thumbs.maintain(renderer)` each frame:

- upload decoded images that are visible
- destroy evicted textures

This matches IGFD’s `ManageGPUThumbnails()` concept.

---

## 13. Filtering Engine (IGFD-Compatible, Rust-Friendly)

Provide:

1) "simple" filters (extensions list)
2) optional parser for IGFD-like syntax:
   - collections: `Name{.png,.jpg}`
   - regex: `((...))`
   - multi-layer extensions: `.vcxproj.filters`
   - advanced `*` handling (prefer globset; fallback to regex if enabled)

Core compiles filters into matchers for fast evaluation.

---

## 14. Save Mode Policies (Confirm Overwrite + Extension Policy)

Introduce `SavePolicy`:

- `confirm_overwrite: bool`
- `extension_policy: ExtensionPolicy`
  - `KeepUser`
  - `AddIfMissing`
  - `OverwriteByFilter` (IGFD-like result modes)

When confirming in save mode:

- compute target path (including extension policy)
- if exists and policy requires confirm → enter `ConfirmOverwrite` state

---

## 15. Performance Considerations

Detailed post-parity implementation design (scan coordinator/runtime, generation invalidation,
partial batches, bounded per-frame work, observability) is documented in:

- `docs/FEARLESS_REFACTOR_P2_PERF_ASYNC_DESIGN.md`

### 15.1 Directory enumeration

Baseline:

- keep a cached snapshot of `Vec<FileMeta>` for current dir
- recompute only on:
  - dir changed
  - refresh requested
  - show_hidden toggled (could be a filtered view instead)

Advanced (optional):

- background enumeration / incremental fill:
  - return partial results to keep UI responsive
  - ensure deterministic ordering by sorting once complete (or stable insert)

### 15.2 Large directories

- avoid allocating strings repeatedly (interning / reuse buffers)
- avoid `to_lowercase()` on every frame; precompute lowercase name or use case-folding cache

---

## 16. Migration Strategy (Breaking Changes Allowed)

This refactor assumes **no backward compatibility requirement**. The goal is to converge quickly on an IGFD-grade mental model (core/state/host separation) without maintaining legacy wrappers.

Phase the migration:

1) extract current rendering into `ui::draw_contents_*()` with a host wrapper
2) introduce `FileDialogCore` + `FileDialogState` (core + UI-only state)
3) add `DialogManager` and a stable open/display API surface
4) iterate on parity features (custom pane, file styles, thumbnails, advanced filters)

### 16.2 Mapping from current code

Current:

- `extensions/dear-file-browser/src/ui/mod.rs`
  - contains state + rendering + filesystem scanning

Proposed:

- keep UI code in `ui/` but remove filesystem and domain logic to `core/`
- convert `selected: Vec<String>` to selection model

---

## 17. Testing Strategy

Core-only tests (no ImGui):

- selection model (Ctrl/Shift behavior, Ctrl+A)
- filter matching (extensions, multi-layer, regex feature)
- save policy (extension policy + overwrite confirm state)
- places (serialize/deserialize)
- event-driven state machine (confirm/cancel transitions)

UI smoke tests:

- minimal: build an ImGui context and call `draw_contents()` to ensure no panic

Performance tests (optional):

- benchmark directory scanning and matching on large synthetic sets

---

## 18. Risks & Trade-offs

- Thumbnail abstraction is the hardest part: requires careful API to stay backend-agnostic.
- Regex/glob compatibility with IGFD is complex; should be feature-gated and added incrementally.
- Full IGFD parity is large; the milestone plan must prioritize user-visible wins (Places, selection UX, custom pane).

---

## 19. Acceptance Criteria (Definition of Done)

This architecture is considered successful when:

- Dialog contents can be hosted as window/modal/popup/embed without duplicating logic.
- Multiple dialogs can be open concurrently via `DialogManager`.
- Places are persistent, editable, and can show system devices.
- Custom pane + file styles exist and can block confirmation.
- Thumbnail pipeline exists and does not hard-depend on any specific renderer backend.
- Core logic is unit-test covered and does not depend on ImGui types.
