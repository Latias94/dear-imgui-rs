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
    -> fixed process-wide executor
    -> coalesced per-dialog scan session
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

Native scans use a fixed four-thread process-wide executor and a bounded session
queue. A dialog runtime owns a scan session with:

- one bounded raw-batch `sync_channel`,
- at most one running request,
- at most one pending request, and
- an atomic cooperative cancel flag for the running generation.

Starting a new generation cancels the running generation and replaces the pending
request. Repeated navigation therefore coalesces to the latest destination without
creating another thread or retaining every intermediate filesystem/path. If a
filesystem is blocked before it can observe cancellation, the latest request starts
when that call returns; navigation and submission still return immediately.

`ScanRuntime::drop` marks the session closed and drops its receiver without joining.
The fixed executor owns worker lifetimes, observes the disconnected receiver, and
releases the closed session after any in-flight filesystem call returns. This keeps
UI teardown non-blocking while placing a hard bound on threads and queued sessions.

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
- repeated blocked generations retain only the latest pending request,
- stale entries and errors cannot replace the new cwd, status, or selection,
- cancellation reaches the streaming visitor, and
- runtime drop returns before a blocked filesystem is released.

Run:

```bash
cargo nextest run -p dear-file-browser
```
