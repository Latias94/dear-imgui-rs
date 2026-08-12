# Build the Bevy integration as a Bevy-native backend

Status: accepted

We will build `dear-imgui-bevy` as a Bevy-native backend that consumes Bevy's window/input/render abstractions instead of stacking `dear-imgui-winit` and `dear-imgui-wgpu` inside Bevy. This deliberately accepts core `dear-imgui-rs` lifecycle refactoring because Bevy owns the ECS schedule, winit event loop, WGPU device, render world, and pipelined rendering model; treating Bevy as just another external window plus renderer pair would fight those ownership boundaries and make editor-style integrations harder to evolve.

## Considered Options

- Wrap `dear-imgui-winit` plus `dear-imgui-wgpu` in a Bevy plugin.
- Implement a Bevy-native backend with its own Bevy RenderApp pipeline and extraction path.

## Consequences

- `dear-imgui-bevy` may need a higher Bevy-coupled MSRV and feature policy than the core crates.
- The core crate should expose lifecycle and snapshot APIs that make engine-managed immediate-mode frames explicit.
- The Bevy backend should make ImPlot, ImNodes, node editor, ImGuizmo, and other `Ui`-based extensions work inside the same ImGui frame rather than creating separate plugin ecosystems.

## Native viewport contract

The Bevy backend does not treat an ECS `Window` or `Monitor` as proof that its
native Winit object is available. For every platform viewport, the stable
`ImguiViewportInstanceId` owns the lifecycle state; the current ECS `Entity`
is only a lookup key into `bevy_winit::WINIT_WINDOWS`. A viewport follows this
order:

`EcsPending -> NativePending -> PolicyInstalling -> ReadyHidden -> ReadyVisible`.

`Show` records intent and never reveals a window by itself. The backend reveals
it only after an exact Winit mapping is borrowed on the Winit thread and the
native policy is ready. On Windows, `NO_INPUTS` maps to transparent pointer
hit-testing and `NO_FOCUS_ON_CLICK` maps to no mouse activation. A policy or
handle replacement first hides the window and releases the old lease. Entity
recycling and HWND reuse therefore cannot inherit a previous viewport's lease.

Monitor publication is a complete detached transaction from the exact host
Winit window. It carries main/work rectangles, scale, identity, and work-area
provenance. Bevy converts that batch to its coordinate model and replaces
`PlatformIO::Monitors` atomically. A missing host mapping or failed collection
does not publish an ECS-monitor or host-window approximation; the last complete
publication may remain installed while native platform viewports stay disabled.

Retirement is lease-first: destroy intents are applied, native policy leases
are released, Winit mappings are retired while wrappers remain available for
render/ECS drain, and only then are PlatformIO ownership and Context attachments
released. The same sequence is idempotent for native-window-first destruction.
