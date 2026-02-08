# IGFD Source Reference Map (UI/Behavior Parity Guide)

This document is a practical guide for **source-diff driven** work: we treat ImGuiFileDialog (IGFD) as the reference implementation for UI/behavior, and keep `dear-file-browser` (ImGui backend) at or above parity while staying Rust-first.

Reference source lives in this repo at:

- `repo-ref/ImGuiFileDialog/ImGuiFileDialog.cpp`

Non-goals:

- 1:1 API/flag compatibility with IGFD (C/C++).
- Matching exact internal data structures (we keep a Rust-first core/UI split).

## How to use this map

When working on a UI/UX gap:

1. Find the relevant IGFD function(s) and read the UI/control flow in context.
2. Locate the corresponding `dear-file-browser` code path.
3. Implement behavior parity (including layout sizing, enablement rules, and keyboard/mouse semantics).
4. Add/extend core tests for the domain rules whenever possible.
5. Update `docs/IGFD_PARITY_AND_DEVIATIONS.md` if any intentional deviation remains.

## Top-level frame structure

IGFD runs a fixed frame layout:

- `IGFD::FileDialog::m_DrawHeader()` → header/chrome (places toggle, new folder, path composer, display mode, search)
- `IGFD::FileDialog::m_DrawContent()` → main content panes (places, file list, side pane) with footer height reserved
- `IGFD::FileDialog::m_DrawFooter()` → filename field, filter combo, OK/Cancel; also measures `footerHeight`

`dear-file-browser` equivalent entrypoint:

- `extensions/dear-file-browser/src/ui/mod.rs` (UI orchestrator: sizing + pane composition + popup dispatch)

## Feature → IGFD → dear-file-browser mapping

### Header / chrome

- IGFD: `IGFD::FileDialog::m_DrawHeader()`
  - Places button + splitter toggle
  - Directory creation ("New Folder")
  - Path composer
  - Display mode toolbar (thumbnails/list)
  - Search bar
- dear-file-browser: `extensions/dear-file-browser/src/ui/mod.rs`
  - Header orchestration: `extensions/dear-file-browser/src/ui/header.rs`
  - Path composer + breadcrumbs: `extensions/dear-file-browser/src/ui/path_bar.rs`
  - IGFD path popup: `extensions/dear-file-browser/src/ui/igfd_path_popup.rs`

### Path composer (breadcrumbs)

- IGFD: `IGFD::FileManager::DrawPathComposer(...)`
  - Inline, framed region (not a child window)
  - Long paths: keep tail visible
  - Separator quick-select opens IGFD path popup
- dear-file-browser:
  - Composer layout: `extensions/dear-file-browser/src/ui/header.rs`
  - Breadcrumb drawing: `extensions/dear-file-browser/src/ui/path_bar.rs` (`draw_breadcrumbs`)
  - Tail visibility: end-aligned + clip-rect in classic header

### Separator quick-select popup (IGFD path popup)

- IGFD: `IGFD::FileDialog::m_DisplayPathPopup(ImVec2 vSize)`
  - Popup name: `"IGFD_Path_Popup"`
  - Size: `0.5 * content` (both axes)
  - Table with header `"File name"`, 1 column, `ScrollY`, row bg
  - Content: subdirectories under the selected breadcrumb segment
  - Filtering: uses the *global search tag* (`SearchManager::searchTag`)
- dear-file-browser:
  - Popup opener: breadcrumb separator click opens `"##igfd_path_popup"`
  - Popup render: `extensions/dear-file-browser/src/ui/igfd_path_popup.rs`
  - Filtering: uses `core.search` (shared search)

### Main content sizing (footer reservation)

- IGFD: `IGFD::FileDialog::m_DrawContent()`
  - `size = GetContentRegionAvail() - (0, footerHeight)`
- dear-file-browser:
  - Content region height uses last-frame measured footer height (no magic constants)
  - Orchestrated in `extensions/dear-file-browser/src/ui/mod.rs`

### File list view (table)

- IGFD: `IGFD::FileDialog::m_DrawFileListView(ImVec2 vSize)`
  - Columns: Name / Type / Size / Date
  - Sorting: table sort specs → `sortingField` and `SortFields(...)`
- dear-file-browser:
  - Table + thumbnails views: `extensions/dear-file-browser/src/ui/file_table.rs`
  - Sort wiring: table sort specs → `core.sort_by/sort_ascending`

### "Type" column semantics (filter-aware)

- IGFD: `IGFD::FileInfos::FinalizeFileTypeParsing(count_dots)` + type sort `FIELD_TYPE`
  - Multi-dot extraction depends on current filter dot depth (`FilterInfos::count_dots`)
- dear-file-browser:
  - `SortBy::Type` + filter-aware dot extraction derived from the active filter
  - `SortBy::Extension` remains available as a distinct "full extension" sort key

### Footer (filename field, filter combo, OK/Cancel)

- IGFD: `IGFD::FileDialog::m_DrawFooter()`
  - Open modes: filename input is editable; Enter triggers OK
  - Measures `footerHeight`
- dear-file-browser:
  - Footer renderer: `extensions/dear-file-browser/src/ui/footer.rs`
  - Footer is IGFD-like: editable file field in Open modes + measured height

## Known "IGFD-only" concepts (still useful to emulate)

- `m_CurrentPathDecomposition` and `ComposeNewPath(...)` (path segment handling)
- `dLGcountSelectionMax` (max selection cap; we support this in core)
- `NaturalSorting` flag (we expose `SortMode`)

## Practical parity checklist (quick)

When something "doesn't feel like IGFD", verify:

- Sizes derived from content region (no hard-coded pixels where IGFD uses proportional sizing).
- Path composer is inline + clipped, and tail is visible.
- Confirm button enablement matches confirmable state (selection or typed footer name/path).
- Table sorting is wired to visible columns (Type column sorts like Type, not full extension).
