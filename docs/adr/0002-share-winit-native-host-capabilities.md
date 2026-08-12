# Share native host capabilities without sharing a platform runtime

Status: accepted

`dear-imgui-winit` exposes a feature-gated `native_support` module for the
small set of operating-system facts that both its own multi-viewport runtime
and `dear-imgui-bevy` need: owned monitor snapshots, native work-area
provenance, and an exact-window Windows policy lease.

The module is a capability layer, not a second platform backend. It does not
own a Dear ImGui context, `ImGuiPlatformIO`, a Winit event loop, a Bevy ECS
entity, a viewport registry, or a renderer route. Winit and Bevy remain
independent platform owners and convert the shared native facts inside their
own lifecycle and coordinate models.

## Considered Options

- Keep separate Win32, AppKit, and X11 queries in the Winit and Bevy backends.
- Move the native queries into a new platform-support crate.
- Let Bevy construct or wrap the complete Winit multi-viewport runtime.
- Expose a narrow feature from `dear-imgui-winit` without sharing runtime
  ownership.

## Decision

The `native-platform-support` feature exposes only owned, validated facts and
opaque leases:

- Windows monitor work areas come from `GetMonitorInfoW` and `rcWork`.
- macOS work areas are local insets derived from `NSScreen.frame` and
  `visibleFrame`, then applied to Winit's physical monitor rectangle.
- X11 `_NET_WORKAREA` remains desktop-scoped evidence. This capability layer
  does not currently claim a per-monitor reduction, so X11 snapshots always
  record a conservative `FullMain(AmbiguousDesktopScope)` fallback. A future
  exact implementation must first prove monitor attribution from the same
  Winit host and native desktop.
- Wayland and unavailable or invalid native sources record a conservative
  full-main fallback rather than fabricated global desktop facts.
- Windows focus and pointer-input policy is installed through a non-cloneable,
  thread-affine lease bound to one exact Winit window. Installation and updates
  verify `GetWindowThreadProcessId` against `GetCurrentThreadId`; a mismatch is
  rejected as `WrongWindowThread` rather than issuing a cross-thread USER32
  call.

The complete Winit multi-viewport feature implies this support feature. Bevy
may enable the support feature alone and must not instantiate or expose the
Winit platform runtime.

## Consequences

- Native source failures are explicit through owned provenance and typed
  collection errors; platform owners decide when to retain their last complete
  publication.
- `collect_monitor_snapshots` enumerates monitors through the supplied
  `Window`, so the host display and monitor handles come from one Winit source
  and refresh generation. `MonitorIdentity` is a detached value; it never
  retains or compares a live `MonitorHandle` after collection.
- Monitor cache, primary ordering, coordinate conversion, transactional
  `ImGuiPlatformIO::Monitors` publication, and window retirement remain in the
  owning backend.
- The Windows lease can be installed only from a borrowed exact `Window`, is
  neither `Clone`, `Send`, nor `Sync`, and tolerates lease-first or
  native-window-first destruction without freeing callback state prematurely.
  A failed removal deliberately retains the callback-owned `Arc` until
  `WM_NCDESTROY`; the destruction callback enters `Destroying` before calling
  `DefSubclassProc` and releases that raw reference at most once.
- A separate crate would add a public dependency boundary without removing a
  real ownership boundary: every supported consumer already depends on Winit.
- Sharing the complete runtime would violate Bevy's ownership of its event
  loop, ECS schedule, native windows, and render pipeline.
