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
