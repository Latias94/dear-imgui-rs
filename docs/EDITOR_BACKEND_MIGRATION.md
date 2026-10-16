# Editor Backend Integration

An engine editor needs distinct identities for its UI, native windows, GPU bindings, and content.
The platform owners and WGPU renderer expose those facts without introducing an engine asset model.

## Choose the integration boundary

| Integration | Owns | Engine controls |
| --- | --- | --- |
| `dear-app` | Winit loop, native windows, WGPU route, recovery | Application hooks, engine recording, invalidation, external textures |
| Winit/SDL3 + WGPU owning route | Platform callbacks and secondary surfaces | Event loop, producer commands, submission, presentation, main surface |
| Detached WGPU snapshots | UI publication and a separate GPU executor | Bounded handoff, target generations, render thread, surfaces and native leases |

Use one platform owner and one renderer owner for a Context. Core snapshots do not make a Context
thread-safe. Native surfaces and their window lifetimes remain the host's responsibility when
using the detached executor.

## Keep the identities separate

| Value | Meaning |
| --- | --- |
| `NativeWindowInstanceId` | One registration of an exact native allocation; not reused after retirement |
| `NativeWindowTarget` | Allocation identity plus the viewport routing ID and main role captured at observation time |
| `SnapshotEpoch` | Context, renderer consumer generation, and ordered UI frame sequence |
| `ExternalTextureId` | One renderer's external view registration |
| `ExternalTextureBinding::revision` | View replacement under that registration; not the engine's content frame |
| Snapshot target resource generation | Host-declared replacement of the resources assigned to a viewport |

Store allocation IDs and binding revisions in history. A docking operation can change a live
window's viewport ID; recovery can recreate native windows for the same logical panel. Resolving
an old numeric routing ID against today's registry cannot reconstruct the historical target.
Keep engine asset IDs, content frame numbers, and fences in the engine's own records.

## Migrate Winit event dispatch

The event APIs no longer take a caller-supplied `Window` borrow:

```rust,ignore
let outcome = platform.handle_window_event(&mut context, window_id, &event)?;
// Full Event<T> loops use platform.handle_event(&mut context, &event).
if let Some(target) = outcome.target {
    engine.route_event(target.native, &event, outcome.capture);
}
```

`forwarded` identifies input streams actually injected into Dear ImGui during the call. `capture`
is recent UI intent, not an event-consumed verdict. `needs_frame` and `platform_action` describe
invalidation and close/geometry actions independently. Foreign windows produce an unowned result.
Retain the chosen recipient from a key/button press through its matching release.

For camera control, request pointer ownership using an exact native instance. Inspect the granted
mode and any failed preferred/fallback grab attempts. The backend suspends its cursor updates for
that window and reports release on escape, focus loss, close, retirement, or shutdown. Route raw
`DeviceEvent` motion to the engine; it has no intrinsic window target. See
[`wgpu_rtt_gameview`](../examples/04-integration/wgpu_rtt_gameview.rs).

## Control WGPU work explicitly

`prepare` now reconciles managed textures and updates native windows. It performs no surface
acquisition, recording, queue submission, or presentation. The former combined preparation flow
is replaced by these move-only capabilities:

```rust,ignore
let prepared = route.prepare(event_loop, frame)?; // SDL3: route.prepare(frame)
let recorded = route.record(prepared)?;
let (main, secondary_work) = recorded.split();
let pending = secondary_work.submit(engine_producer_commands)?;
let secondary_report = pending.present()?; // Or explicitly discard.
// Acquire the host-owned main surface, then call route.record_main(main, pass, extent).
```

Submission puts producer commands before every secondary UI consumer in one queue operation.
Recorded work retains commands, acquired frames, Surfaces, exact native windows, and their owner
chains. Submission transfers those leases into pending presentation. `present`, `discard`, and
Drop release an acquired frame before its Surface and native lease; Drop never submits or presents.
Retired work cannot be submitted. Already submitted work can finish presentation or discard even
after Context teardown. Pending work freezes native owner changes and delays reconfiguration of
the affected surface.

For a conventional loop, use `complete_secondary(prepared, producer_commands)`, then acquire the
main surface and call `record_main` with the returned `WgpuPreparedMainFrame`. A main acquisition
failure does not undo completed secondary work. Preparation/work errors carry partial reports
alongside their faults; retain both.

## Interpret observations precisely

For every drawable external image, observations associate a captured native target with the UI
epoch and external binding revision after validation and clipping. Empty or clipped commands do
not create dependencies. Arbitrary live callbacks mark dependency knowledge as incomplete.

`Recorded`, `Submitted`, and `PresentationRequested` describe observed API boundaries. They do
not prove GPU completion, compositor acceptance, or visible pixels. Submission indices support
host-owned fence tracking. Store skip, failure, and discard outcomes independently of the highest
observed stage, including for partially successful frames.

## Move rendering to a worker

`WgpuSnapshotRenderer` stays on the UI thread and owns the existing core detached consumer.
`WgpuSnapshotExecutor` owns GPU state and can move to a native render thread. Send core-owned
`FrameSnapshot` values with frozen target capabilities through a bounded channel. Register or
replace targets on the executor whenever render-target resources or native registrations change;
queued capabilities from older generations are rejected.

Prepare a submission, record the captured targets into host-owned passes, and finish its feedback.
All managed requests receive an outcome even after a partial failure. Dropping prepared work
commits applied feedback without submitting commands. Core completion means CPU processing ended,
not GPU completion. Raw live callbacks are rejected during snapshot capture. Join the worker and
reconcile completions before explicit UI-owner shutdown. See
[`threaded_snapshot_wgpu`](../examples/04-integration/threaded_snapshot_wgpu.rs).

Native metadata in a detached target is supplied by the platform integration. It is not a native
window lease. The host must retire its executor target and retain actual surfaces/windows for any
work it records; the executor cannot consult the UI thread's native registry.

## Use the convenience runtime

Enable `dear-app/multi-viewport` and set `AppConfig::multi_viewport = true` explicitly. The default
remains single-window. Full `AppConfig` literals must initialize the new `multi_viewport` and
`device_events` fields, or use `..Default::default()`; raw device events default to
`DeviceEvents::WhenFocused`. `Application::record_render` records engine producers before UI consumers;
`viewport_frame` receives secondary and main observations, including earlier secondary success
when main acquisition fails. Event hooks receive the actual native window and structured outcome.

Use `AppWakeHandle` for coalesced background invalidation or one-shot deadlines in Wait mode.
`device_event` exposes raw device input independently of windows. On GPU loss, rebuild engine
resources in `gpu_recreated`; the runtime preserves Context/application state and main allocation
identity, while retiring and recreating secondary allocations with new IDs.

## Desktop services and texture inspection

SDL owner-bound callback handoffs copy file/text drop payloads, source, supplied coordinates, and
observed native metadata before callback storage expires. Resolve the observation as current,
retired, or unowned; an SDL numeric ID alone does not establish ownership. Standalone handoffs keep
payloads without claiming a native registration. Winit exposes owned hover/drop/cancel events.

The optional `dear-imgui-rs/system-clipboard` adapter implements the existing `ClipboardBackend`.
Initialize it explicitly with `SystemClipboard::new()?`; default builds add no clipboard service.
Direct text operations return errors, while the trait bridge retains its existing optional-read
and infallible-write behavior. SDL also provides its platform-native clipboard callbacks.

Depth, integer IDs, HDR, array layers, and mips require explicit engine conversion to a supported
UI image. The renderer does not reinterpret arbitrary source formats. The
[`wgpu_texture_inspector`](../examples/04-integration/wgpu_texture_inspector.rs) example produces
real source formats, converts them to RGBA8, preserves registrations across resize, and reports
native-target/binding observations. Its GPU readback test checks the conversion values.

## Verification and platform limits

See [renderer route evidence](renderer-route-evidence.md) for native coverage. WGPU major features
27/28/29/30 are mutually exclusive; Winit and SDL3 multi-viewport routes are also mutually exclusive.
Native parent, desktop positioning, transparency, and mutable policy support follow each platform's
reported capabilities. Unknown native state remains unknown. Windows runtime evidence does not
establish Cocoa, X11, or Wayland behavior.

Native pending-frame present, discard, and Drop tests passed with explicit Vulkan-only and
DX12-only Instances. On the tested Windows/NVIDIA system, WGPU 30.0.1 multi-backend
`Backends::PRIMARY` teardown can terminate the process after discarding a submitted surface frame;
a pure WGPU diagnostic reproduces the failure without this crate's native leases. The cause is
unresolved. See [the WGPU validation notes](../backends/dear-imgui-wgpu/README.md#native-discard-teardown-validation)
for the tested driver and explicit reproducer. The renderer preserves the host's backend policy
and does not implicitly present discarded frames.
