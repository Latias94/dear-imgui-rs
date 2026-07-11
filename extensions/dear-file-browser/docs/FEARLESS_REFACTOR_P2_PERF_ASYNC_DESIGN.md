# Background Scan Runtime

This document describes the implemented directory scan architecture for
`dear-file-browser`. It replaces the old "incremental" runtime, which performed a
complete synchronous `read_dir` on the UI thread and only sliced the resulting
snapshot afterward.

## Runtime Contract

`FileDialogState` owns both the filesystem capability and `FileDialogCore`. The UI
draw API no longer accepts a frame-borrowed `&dyn FileSystem`.

```text
native Background
  Arc<dyn FileSystem + Send + Sync>
    -> per-dialog ScanWorker
    -> coalesced scan session
    -> FileSystem::visit_dir
    -> bounded raw FsEntry channel
    -> UI poll budget
    -> scan hook
    -> DirEntry conversion
    -> projection and selection reconciliation

Blocking / wasm
  Box<dyn FileSystem>
    -> caller-thread visit_dir
    -> the same raw-batch application path
```

The public policy has two truthful modes:

```rust
pub enum ScanPolicy {
    Blocking,
    Background {
        batch_entries: usize,
        max_batches_per_tick: usize,
    },
}
```

`Background` means filesystem enumeration actually happens on a native worker. It
does not mean that a complete UI-thread snapshot is queued and sliced later.

## Filesystem Boundary

Directory enumeration is streaming and object-safe:

```rust
fn visit_dir(
    &self,
    dir: &Path,
    visit: &mut dyn FnMut(FsEntry) -> ScanVisit,
) -> io::Result<()>;
```

Implementations must stop when the visitor returns `ScanVisit::Stop`. This is the
cooperative cancellation boundary. It avoids forcing custom virtual or JS-backed
filesystems to expose internal iterators or worker tokens.

Native background construction requires
`Arc<dyn FileSystem + Send + Sync>`. Blocking construction accepts
`Box<dyn FileSystem>` and therefore preserves non-`Send` implementations. On
`wasm32`, selecting `Background` returns
`FileDialogError::BackgroundScanUnsupported`; the runtime never starts a thread or
silently downgrades the policy.

## Worker Lifecycle

Each native `ScanRuntime` owns one `ScanWorker` and a bounded session queue. The
worker owns its wake channel, cooperative cancellation state, and `JoinHandle`. A
dialog runtime has:

- one bounded raw-batch `sync_channel`,
- at most one running request,
- at most one pending request, and
- an atomic cooperative cancel flag for the running generation.

The raw-batch channel is sized from the first background policy used by the
runtime and remains stable for that runtime's lifetime. Later policy changes alter
the UI apply budget without joining an in-flight worker just to resize its queue.

Starting a new generation cancels the running generation and replaces the pending
request. Repeated navigation therefore coalesces to the latest destination without
retaining every intermediate filesystem/path or waiting for the old call. If a
filesystem is blocked before it can observe cancellation, the latest request starts
when that call returns; navigation and submission still return immediately.

`ScanRuntime::drop` marks its session closed, requests cancellation, closes the
worker wake channel, and joins the worker after releasing the session mutex. No
worker outlives its runtime. A filesystem blocked in an uninterruptible call delays
destruction of that dialog until the call returns, but cannot occupy capacity needed
by another dialog's worker.

The bounded sender retries while checking cancellation. This prevents a worker
from remaining blocked forever when its receiver is dropped or its generation is
superseded.

## Correctness Rules

- A generation is authoritative only while it equals `FileDialogCore`'s current
  generation.
- Superseded entries and errors are ignored even if a custom filesystem observes
  cancellation late.
- Workers transport raw `FsEntry` values only.
- User scan hooks execute while the UI thread applies a raw batch, before creating
  `DirEntry` values.
- At most `max_batches_per_tick` messages are applied in one UI tick.
- Partial batches are filtered independently, then inserted into a `BTreeSet`
  ordered by the active view key and a stable arrival sequence. Batch ingestion
  does not traverse, move, clone, or re-filter the accumulated view; the full
  index is rebuilt only when view inputs change.
- A terminal `Complete` message reconciles unresolved selection without forcing a
  redundant full-snapshot projection.
- Selection IDs remain unresolved during partial data and are reconciled when the
  generation completes.

## Auxiliary UI

The address-bar completion callback and breadcrumb quick-select popup do not run
directory enumeration. Completion uses directories already present in the current
projection. Breadcrumb quick-select uses known recent paths. This prevents an
otherwise-background dialog from reintroducing synchronous directory I/O through
an auxiliary widget.

## Verification

The regression suite uses channel/condition-variable handshakes rather than timing
sleeps. It proves:

- worker submission returns before a gated `visit_dir` finishes,
- filesystem I/O runs on a worker while scan hooks run on the polling thread,
- batch application respects the configured per-tick budget,
- a 4,096-entry scan applies 128-entry projection deltas without full-snapshot
  rebuilds between batches and bounds measured comparisons and historical-entry
  visits by `O(N log N)`,
- repeated blocked generations retain only the latest pending request,
- stale entries and errors cannot replace the new cwd, status, or selection,
- cancellation reaches the streaming visitor, and
- runtime drop requests cancellation, waits for its worker, and finishes once a
  blocked filesystem is released, and
- blocked workers belonging to several dialogs do not starve a fresh dialog scan.

Run:

```bash
cargo nextest run -p dear-file-browser
```
