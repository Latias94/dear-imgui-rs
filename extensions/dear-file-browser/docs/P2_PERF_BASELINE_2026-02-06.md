# P2 Synthetic Performance Baseline (2026-02-06)

This document records synthetic large-directory performance data for P2 Stage D and Stage E.

## Scope

- Crate: `dear-file-browser`
- Focus: scan + projection path on synthetic directory entries
- Profiles:
  - `ScanPolicy::Sync`
  - `ScanPolicy::Incremental { batch_entries: 512, max_batches_per_tick: 1 }`
  - `ScanPolicy::Incremental { batch_entries: 512, max_batches_per_tick: N }` sweep (`N=1/2/4`)

## Method

Ignored perf tests:

- `dialog_core::tests::perf_baseline_large_directory_scan_profiles`
- `dialog_core::tests::perf_baseline_incremental_budget_sweep`

Command used:

- `cargo nextest run -p dear-file-browser --run-ignored only perf_baseline --no-capture`

Notes:

- Results are from test profile (debug, non-release), mainly for relative comparison across refactors.
- Synthetic data is generated in-memory by test helper `make_synthetic_fs_entries(count)`.
- Absolute numbers vary across machines/runs; trend direction is the primary signal.

## Stage D Baseline Results (Sync vs Incremental)

| Entry count | Sync scan (ms) | Incremental scan (ms) | Incremental ticks | Batch size | Max batches/tick |
|---:|---:|---:|---:|---:|---:|
| 10,000 | 7 | 51 | 22 | 512 | 1 |
| 50,000 | 36 | 1046 | 100 | 512 | 1 |

Raw output excerpt:

- `PERF_BASELINE entry_count=10000 sync_ms=7 incremental_ms=51 incremental_ticks=22 batch_entries=512`
- `PERF_BASELINE entry_count=50000 sync_ms=36 incremental_ms=1046 incremental_ticks=100 batch_entries=512`

## Stage E Budget Sweep (50k entries)

| Entry count | Batch size | Max batches/tick | Incremental scan (ms) | Incremental ticks |
|---:|---:|---:|---:|---:|
| 50,000 | 512 | 1 | 1523 | 100 |
| 50,000 | 512 | 2 | 574 | 50 |
| 50,000 | 512 | 4 | 286 | 25 |

Raw output excerpt:

- `PERF_SWEEP entry_count=50000 incremental_ms=1523 incremental_ticks=100 batch_entries=512 max_batches_per_tick=1`
- `PERF_SWEEP entry_count=50000 incremental_ms=574 incremental_ticks=50 batch_entries=512 max_batches_per_tick=2`
- `PERF_SWEEP entry_count=50000 incremental_ms=286 incremental_ticks=25 batch_entries=512 max_batches_per_tick=4`

## Interpretation

- Increasing `max_batches_per_tick` reduces total completion time and required ticks significantly.
- Higher per-tick budgets increase one-frame workload, so this is a throughput vs frame pacing tradeoff.
- Current tuned recommendation is:
  - `batch_entries = 512`
  - `max_batches_per_tick = 2`

## Next Benchmarks

- Add `batch_entries` sensitivity sweep (e.g. `128/256/512/1024`) under the tuned apply budget.
- Add mixed file/dir/link metadata scenarios.
- Add projection-heavy cases (search/filter/sort churn under partial scan).
