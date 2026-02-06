# Fearless Refactor P2 Design: Performance and Async Enumeration

This document defines the post-parity (P2) technical design for `dear-file-browser`.
It assumes non-C-API parity with ImGuiFileDialog (IGFD) is complete and focuses on scalability, responsiveness, and long-term maintainability.

Last updated: 2026-02-06

---

## 1. Why P2 Exists

Parity work delivered feature completeness for non-C-API scope, but large-scale usage still has operational bottlenecks:

- Long directory listing latency for 10k+ entries.
- UI stalls when scan and metadata work happen on the UI thread.
- Repeated per-frame filter/search normalization overhead.
- Limited observability for scan/projection cost.

P2 keeps user-visible semantics stable while introducing an architecture that can scale to large repositories and custom filesystems.

---

## 2. Goals and Non-Goals

### 2.1 Goals

1. Keep UI responsive during expensive scans.
2. Support incremental/background enumeration with deterministic behavior.
3. Preserve selection and confirmation semantics across partial updates.
4. Add measurable tracing/counters for scan and projection cost.
5. Keep a Rust-first, testable, backend-agnostic core.

### 2.2 Non-Goals

1. C API compatibility.
2. Pixel-identical IGFD internals.
3. One-shot async rewrite of the full crate.
4. Replacing all modules in one patch.

---

## 3. Design Principles

1. **Eventual consistency with deterministic state**
   - Partial results are allowed; confirm/selection decisions remain deterministic.
2. **Generation-based ownership**
   - Each rescan creates a new generation; stale batches are always discarded.
3. **Incremental-first, sync-safe fallback**
   - Keep synchronous runtime for tests/minimal hosts; async runtime is optional.
4. **No heavy filesystem walk in UI thread**
   - UI thread polls/applies bounded batches and renders snapshots.
5. **Composable modules with strict boundaries**
   - Scan, projection, and selection reconciliation are independent units.

---

## 4. Proposed Runtime Topology

```text
Ui Frame
  -> FileDialogCore::tick_scan()
     -> ScanCoordinator (request lifecycle + generation)
     -> ScanRuntime (sync worker abstraction)
     -> SnapshotStore (raw entries and version)
     -> ViewProjector (filter/sort/search -> visible ids)
     -> SelectionReconciler (anchor/focus/selected stability)
```

The topology allows incremental adoption: `ScanRuntime` can remain synchronous until worker runtime is enabled.

---

## 5. Component Contracts

### 5.1 `ScanCoordinator`

Responsibilities:

- Accept scan triggers (`cwd` change, manual refresh, policy update).
- Assign generation IDs.
- Cancel/supersede old scan requests.
- Surface scan status (`Idle`, `Scanning`, `Partial`, `Complete`, `Failed`).

Invariants:

- At most one active generation is authoritative.
- Any batch not matching current generation is ignored.

### 5.2 `ScanRuntime`

Modes:

- `SyncRuntime`: same semantics as current baseline.
- `WorkerRuntime`: background enumeration returning incremental batches.

Suggested trait:

```rust
trait ScanRuntime {
    fn request(&mut self, req: ScanRequest);
    fn poll_batch(&mut self) -> Option<ScanBatch>;
    fn cancel_generation(&mut self, generation: u64);
}
```

### 5.3 `SnapshotStore`

Stores authoritative raw scan output:

- `generation`
- `entries_by_id`
- `ordered_ids`
- cached normalization fields (lowercased name/ext, optional tokens)

### 5.4 `ViewProjector`

Transforms snapshot to visible ordering using:

- hidden-file policy
- mode eligibility (open file/open folder/save)
- filter/search query
- sort policy

Rule: avoid full rebuild if effective projector keys do not change.

### 5.5 `SelectionReconciler`

Responsibilities:

- Keep selected IDs if still resolvable.
- Drop unresolved IDs deterministically.
- Preserve focus/anchor when possible.
- Enforce `max_selection` after projection updates.

---

## 6. Data Model Extensions

### 6.1 `ScanRequest`

Fields:

- `generation: u64`
- `cwd: PathBuf`
- `scan_policy: ScanPolicy`
- `submitted_at`

### 6.2 `ScanBatch`

Fields:

- `generation: u64`
- `kind: Begin | Entries(Vec<FsEntry>) | Complete | Error(String)`
- `is_final: bool`

### 6.3 `ScanPolicy`

```rust
enum ScanPolicy {
    Sync,
    Incremental {
        batch_entries: usize,
        max_batches_per_tick: usize,
    },
    Background {
        batch_entries: usize,
        max_batches_per_tick: usize,
    },
}
```

Policy is optional and host-configurable with sane defaults (`ScanPolicy::tuned_incremental()` / `ScanPolicy::tuned_background()`).

### 6.4 `ScanStatus`

```rust
enum ScanStatus {
    Idle,
    Scanning { generation: u64 },
    Partial { generation: u64, loaded: usize },
    Complete { generation: u64, loaded: usize },
    Failed { generation: u64, message: String },
}
```

---

## 7. Async Enumeration Semantics

### 7.1 Worker lifecycle

1. Core emits `ScanRequest(generation = N)`.
2. Runtime starts or updates worker for generation `N`.
3. Worker emits `Begin`, `Entries` batches, and `Complete` or `Error`.
4. Core polls batches each frame and applies only generation `N`.
5. New request `N+1` cancels `N` best-effort; stale batches are dropped.

### 7.2 Cancellation guarantees

- Runtime cancellation is best-effort.
- Correctness is guaranteed by generation filtering in core.
- Stale batches must never mutate current snapshot.

### 7.3 UI-facing behavior

- Show lightweight loading hint during scan.
- Allow list growth with partial batches.
- Confirm action operates on latest consistent projection only.

---

## 8. Performance Strategy

1. **Normalization cache**
   - Compute lowercase name/extension tokens once per entry.
2. **Projection memoization**
   - Rebuild projector output only on effective key change.
3. **Delta-friendly append path**
   - For incremental scan, project appended entries without full rebuild where safe.
4. **Bounded frame workload**
   - Apply at most `N` batches or `M` entries per frame.
   - Tuned preset currently uses `batch_entries=512` and `max_batches_per_tick=2`.
5. **Cheap instrumentation hooks**
   - Measure scan, projection, and confirm path with low overhead.

---

## 9. Observability

Suggested events:

- `scan.requested` (generation, cwd)
- `scan.batch_applied` (generation, entries)
- `scan.completed` (generation, total_entries, duration_ms)
- `scan.dropped_stale_batch` (generation)
- `projector.rebuild` (reason, visible_entries, duration_us)

When `tracing` feature is enabled, expose structured events/spans for debugging and benchmarking.

---

## 10. Public API Evolution (Rust-first)

Potential additive APIs:

- `FileDialogUiState::scan_policy`
- `FileDialogCore::scan_status()`
- `FileDialogCore::request_rescan(reason)`

Fearless-mode simplification (breaking allowed):

- Remove deprecated wrappers and keep unified `*_with` entrypoints.

---

## 11. Rollout Stages (Mapped to Milestone 17)

### Stage A: scaffolding (Epic 17.1)

- Add `ScanPolicy`, `ScanRequest`, `ScanBatch`, `ScanStatus`.
- Keep runtime synchronous to preserve behavior.

### Stage B: worker runtime (Epic 17.2)

- Add optional `WorkerRuntime` behind feature/config gate.
- Validate generation filtering and stale-batch safety.

### Stage C: projection/selection robustness (Epic 17.3)

- Add bounded per-frame apply.
- Improve delta projection and selection reconciliation.

### Stage D: instrumentation and baseline capture (Epic 17.4)

- Add tracing events/counters.
- Capture synthetic baseline for 10k/50k directories.

### Stage E: policy tuning and apply-budget controls (Epic 17.5)

- Add `max_batches_per_tick` to incremental/background policies.
- Publish tuned presets and benchmark sweep for apply-budget tradeoffs.

---

## 12. Risks and Mitigations

1. **Race/stale state bugs**
   - Mitigation: strict generation ownership + property/integration tests.
2. **Selection instability under partial scans**
   - Mitigation: centralized `SelectionReconciler` rules.
3. **Filesystem edge-case variance by platform**
   - Mitigation: preserve strict `FileSystem` abstraction + fake-FS tests.
4. **Premature complexity**
   - Mitigation: staged delivery and sync fallback baseline.

---

## 13. Test Strategy

### 13.1 Unit tests (core)

- Stale batch ignored by generation mismatch.
- Generation switch correctness (`N` superseded by `N+1`).
- Partial scan keeps deterministic selection.
- Scan policy transitions are safe.

### 13.2 Integration tests

- Worker partial -> complete sequence.
- Fast directory switching with cancellation.
- Confirm behavior consistency while scan is partial.

### 13.3 Performance tests (opt-in)

- Synthetic 10k and 50k directory datasets.
- Frame budget stability under incremental append.
- Baseline + Stage E budget sweep: `docs/P2_PERF_BASELINE_2026-02-06.md`.

---

## 14. Definition of Done (P2 Architecture)

P2 is considered complete when:

- Incremental/background scanning works without UI stalls for large directories.
- Selection and confirmation semantics remain deterministic.
- Stale scan batches cannot corrupt active snapshot.
- Tracing clearly exposes scan and projection cost.
- Sync fallback remains available and covered by tests.
