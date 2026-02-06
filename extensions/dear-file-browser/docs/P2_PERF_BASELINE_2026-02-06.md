# P2 Synthetic Performance Baseline (2026-02-06)

This document records the first synthetic large-directory baseline for P2 Stage D.

## Scope

- Crate: `dear-file-browser`
- Focus: scan + projection path on synthetic directory entries
- Profiles:
  - `ScanPolicy::Sync`
  - `ScanPolicy::Incremental { batch_entries: 512 }`

## Method

Ignored perf test:

- `dialog_core::tests::perf_baseline_large_directory_scan_profiles`

Command used:

- `cargo nextest run -p dear-file-browser --run-ignored only perf_baseline_large_directory_scan_profiles --no-capture`

Notes:

- Results are from test profile (debug, non-release), mainly for relative comparison across refactors.
- Synthetic data is generated in-memory by test helper `make_synthetic_fs_entries(count)`.

## Baseline Results

| Entry count | Sync scan (ms) | Incremental scan (ms) | Incremental ticks | Batch size |
|---:|---:|---:|---:|---:|
| 10,000 | 10 | 80 | 22 | 512 |
| 50,000 | 51 | 1142 | 100 | 512 |

Raw output excerpt:

- `PERF_BASELINE entry_count=10000 sync_ms=10 incremental_ms=80 incremental_ticks=22 batch_entries=512`
- `PERF_BASELINE entry_count=50000 sync_ms=51 incremental_ms=1142 incremental_ticks=100 batch_entries=512`

## Interpretation

- Sync mode has lower end-to-end completion time in this synthetic setup, but it does all work in one refresh call.
- Incremental mode spreads work across multiple ticks (`22` / `100`), improving frame pacing and responsiveness potential.
- Future optimization should target lowering incremental total time while preserving bounded per-tick work.

## Next Benchmarks

- Add `batch_entries` sensitivity sweep (e.g. `128/256/512/1024`).
- Add mixed file/dir/link metadata scenarios.
- Add projection-heavy cases (search/filter/sort churn under partial scan).
