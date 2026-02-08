# IGFD Parity and Deviations (Non-C-API Scope)

This document tracks capability parity between `dear-file-browser` (ImGui backend) and ImGuiFileDialog (IGFD), focused on **non-C-API** behavior.

Last updated: 2026-02-08

## Scope

- Included: ImGui-hosted dialog behavior, core state machine, styling, filtering, places, file operations, thumbnails, host constraints.
- Excluded: C API shape and API-level compatibility with IGFD C/C++ interfaces.

## Capability Parity Matrix (Non-C-API)

| Capability | Status | Notes |
|---|---|---|
| Open/Display split + multi-instance | Done | `DialogManager` + stable ids. |
| Explicit lifecycle helpers | Done | `open/reopen/close/is_open`. |
| Host flexibility (window/modal/embed/popup-hosted) | Done | `draw_contents*`, `show_windowed*`, `show_modal*`. |
| Places and persistence | Done | Devices + bookmarks + editable groups + compact import/export (v1), with per-group metadata and optional separators. |
| Places pane (toggle + splitter) | Done | Toolbar "Places" control + Standard-layout splitter-resizable pane; Minimal layout uses a popup. |
| Selection UX | Done | Ctrl/Shift, Ctrl+A, keyboard navigation, type-to-select, max-selection cap. |
| Filters and parsing | Done | Extension/wildcard/regex/collection parsing with IGFD-style semantics. |
| Type column semantics | Done | IGFD-style filter-aware "Type" (dot depth derived from active filter) via `SortBy::Type`; full extension sorting remains available via `SortBy::Extension`. |
| Save policies | Done | Confirm-overwrite + extension policy. |
| File operations | Done | Rename/delete/copy/cut/paste with conflict resolution. |
| File styles | Done | Rule + callback styles with `Dir/File/Link` support. |
| Scan-time entry callback | Done | Core/state scan hook (`set_scan_hook` / `clear_scan_hook`). |
| Thumbnails | Done | Decode/upload/destroy lifecycle + grid/list integration. |
| Address bar path input | Done | Always-visible Path input + Go + submit-on-Enter; Ctrl+L focuses; history + Tab completion. |
| Breadcrumb path composer | Done | Optional breadcrumb composer with ellipsis compression, edit toggle, devices/reset actions, and optional quick path selection popups (IGFD-like path table popup sized from content). |
| Path composer tail visibility | Done | Breadcrumbs in the framed composer are end-aligned so the rightmost segments stay visible for long paths. |
| Footer file field (Open) | Done | IGFD-like editable footer field; typing a file name/path can confirm Open when no selection exists. |
| Pick folder selection | Done | PickFolder confirms the selected directory when present; otherwise defaults to current directory. |
| Confirm button enablement | Done | Confirm button is disabled until the core has something confirmable (selection or typed footer path; non-empty save name). |
| IGFD-like header layout | Done | `HeaderStyle::IgfdClassic` + `apply_igfd_classic_preset()` provide a closer header order (Places + New Folder + Path + View + Search). |
| Navigation history | Done | Back/Forward/Up/Refresh with history stacks; Alt+Left/Right + F5 shortcuts. |
| Host min/max constraints | Done | `WindowHostConfig` and `ModalHostConfig` support `min_size` / `max_size`. |

## UI Fidelity Parity (IGFD-look & micro-interactions)

Capability parity does **not** imply pixel-identical UI. This section tracks the remaining work to make the ImGui backend **feel** like IGFD when using `HeaderStyle::IgfdClassic`.

| UI detail | Status | Notes |
|---|---|---|
| Classic header compact labels (`R`, `E`, `FL/TL/TG`) | Done | IGFD uses single-letter/short labels to stay usable under narrow widths. |
| Breadcrumb right-click semantics (enter edit, do not navigate) | Done | IGFD right-click on crumb/separator activates path edit buffer without changing cwd until Enter. |
| Breadcrumb separator behavior parity | Done | Click: open path popup; right-click: activate edit at parent segment (IGFD). |
| Root separator duplication (`F:\\\\...`, `//home`) | Done | RootDir is not rendered as a separate crumb after a drive/UNC prefix; separator between root and first segment is skipped. |
| Header narrow-width no-overlap | Planned | Prefer single-row; fall back to stacked without cursor backtracking overlap. |
| Places visuals (spacing, right-aligned edit buttons) | Partial | Behavior parity is mostly there; visual fidelity still evolving. |
| Thumbnails views (row height, padding, selection highlight) | Partial | Functionality parity exists; pixel-level alignment still evolving. |
| Context menus parity (path / file list items) | Partial | Major actions exist; IGFD-style menu grouping and labels TBD. |

## Intentional Deviations (By Design)

1. Rust-first typed API instead of 1:1 flag mirroring
   - We prioritize typed enums/structs and explicit methods over direct C-style flag bags.

2. Unified result model
   - `Selection { paths: Vec<PathBuf> }` is canonical.
   - IGFD-like helpers are provided as convenience, not separate result primitives.

3. Callback surfaces are Rust-native
   - Style and scan hooks are closure-based and type-safe.
   - This intentionally differs from C function-pointer conventions.

4. C API excluded from this parity wave
   - Explicit product decision; tracked separately if needed.

## Platform Notes

- Symlink metadata depends on filesystem/platform behavior.
- For directory symlinks, behavior follows underlying filesystem metadata (`is_dir` + `is_symlink`).

## Verification Baseline

- Formatting: `cargo fmt`
- Tests: `cargo nextest run -p dear-file-browser`

## Reference implementation

We keep a copy of ImGuiFileDialog source under `repo-ref/ImGuiFileDialog` and treat it as the reference for ImGui-backend UI/behavior parity work.
See `docs/IGFD_SOURCE_REFERENCE_MAP.md` for a practical function map.

## Post-Parity Backlog (P2)

- Published: `docs/FEARLESS_REFACTOR_P2_PERF_ASYNC_DESIGN.md` (ScanCoordinator/ScanRuntime + generation-safe incremental scanning model).
- Completed: Stage A scaffolding (`ScanPolicy`, `ScanRequest`, `ScanBatch`, `ScanStatus`) and generation-owned sync scan flow in `FileDialogCore`.
- Completed: Stage B runtime abstraction (`ScanRuntime` with `SyncRuntime`/`WorkerRuntime`) and stale-generation supersession behavior.
- Completed: Stage C bounded per-frame batch apply + incremental selection reconciliation stability.
- Completed: Stage D tracing metrics/events (`scan.requested`, `scan.batch_applied`, `scan.completed`, `scan.dropped_stale_batch`, `projector.rebuild`).
- Completed: Stage D synthetic baseline tests for `10k/50k` entries (`docs/P2_PERF_BASELINE_2026-02-06.md`).
- Completed: Stage E policy tuning (`max_batches_per_tick` + tuned presets + sweep baseline).
- Next: publish migration snippets and extend benchmark matrix (batch size + mixed metadata).
- Continue UX polish and migration snippets without changing parity guarantees.
