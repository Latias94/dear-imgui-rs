# dear-imgui-bevy

Bevy-native integration for `dear-imgui-rs`.

The backend owns Dear ImGui Contexts on Bevy's main thread, routes Bevy window input to them, moves one-use frame snapshots into the render world, and renders through Bevy cameras. It does not wrap the Winit or standalone WGPU backends: Bevy remains the owner of the app loop, windows, render schedules, images, and GPU lifetime.

## Requirements

| Component | Version |
| --- | --- |
| Rust | `1.95.0` or newer |
| Bevy | exactly `0.19.0` |
| dear-imgui-rs | `0.16.0-alpha.1` |

`dear-imgui-bevy` defaults to the renderer plus deterministic Bevy UI ordering. Native multi-viewport is supported through an explicit feature and runtime opt-in. WASM supports the normal and headless feature sets but cannot create native platform windows.

## Installation

Until `0.16.0-alpha.1` is published:

```toml
[dependencies]
bevy = "=0.19.0"
dear-imgui-bevy = { git = "https://github.com/Latias94/dear-imgui-rs", branch = "main" }
dear-imgui-rs = { git = "https://github.com/Latias94/dear-imgui-rs", branch = "main" }
```

After publication:

```toml
[dependencies]
bevy = "=0.19.0"
dear-imgui-bevy = "=0.16.0-alpha.1"
dear-imgui-rs = "=0.16.0-alpha.1"
```

For a headless integration that drives UI schedules without installing the Bevy renderer:

```toml
dear-imgui-bevy = { version = "=0.16.0-alpha.1", default-features = false }
```

## Quick Start

One primary window and one active primary-window camera are routed automatically:

```rust
use bevy::prelude::*;
use dear_imgui_bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, ImguiPlugin::default()))
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Camera2d);
        })
        .add_systems(ImguiPrimaryContextPass, tools_ui)
        .run();
}

fn tools_ui(imgui: ImguiUi) -> Result {
    let ui = imgui.ui()?;
    ui.window("Tools")
        .build(|| ui.text("Dear ImGui is drawing in Bevy."));
    Ok(())
}
```

The plugin opens and closes each frame around its Context schedule. UI systems borrow the active frame through `ImguiUi`; they must not call `Context::frame()` or `Context::render()` themselves.

## Feature Modes

| Mode | Cargo selection | Contract |
| --- | --- | --- |
| Default | default features | Renderer plus `bevy-ui`; Dear ImGui draws above Bevy UI by default. |
| Headless | `default-features = false` | All Context schedules and lifecycle; primary-Context input/capture only, without `RenderApp` extraction. |
| Render only | `default-features = false, features = ["render"]` | Renderer without a dependency on Bevy UI rendering. |
| Native multi-viewport | `features = ["multi-viewport"]` | Implies `render` and `bevy_winit`; runtime opt-in is still required. |
| WASM default | `features = ["wasm"]` | Default renderer/UI feature set using the explicit WASM core route. |
| WASM headless | `default-features = false, features = ["wasm"]` | Headless WASM integration. |

`multi-viewport` is native-only and intentionally fails to compile with `wasm32`. ImPlot, ImNodes, ImGuizmo, and other `Ui`-based extensions are normal application dependencies rather than backend features.

## Migrating from 0.15

| 0.15 shape | 0.16 replacement |
| --- | --- |
| `configure_example_context` and `ImguiOverlayCamera` | Spawn a normal camera and add `ImguiPlugin::default()`; the unique primary-window camera is automatic. |
| `ImguiBackendConfig` / `ImguiBackendStatus` | Configure startup through private-field `ImguiPluginConfig`; observe route and runtime failures through `ImguiDiagnostics` and `ImguiContexts::last_error`. |
| UI-only `ImguiContexts` system parameter | Use `ImguiUi` inside a Context schedule; use the non-send `ImguiContexts` registry only for creation, configuration, inspection, and removal. |
| `ImguiBevyTextures::register` / `unregister` and retained raw IDs | Hold a cloneable strong or weak `ImguiTexture` lease from `register_strong` / `register_weak`. |
| Public `ImguiInputState`, input systems/mappers, or writable `ImguiInputCapture` fields | Let `ImguiPlugin` translate messages; read `ImguiInputCapture` through aggregate/scoped queries or public run conditions. Call `aggregate()` when a copyable snapshot is needed. |
| `ImguiViewportWindow { viewport_id }`, `ImguiViewportCamera { viewport_id }`, direct field access, or `.copied()` marker queries | Query markers by reference and call `context_id()` / `viewport_id()`. The backend exclusively creates and repairs these identity projections. |
| Public begin/end schedules, renderer resources, or viewport queue access | Let the plugin drive frames; order custom passes through `ImguiRenderSystems` and observe only public route, diagnostic, capture, texture, and viewport identity/configuration types. |
| Wrapper extraction through `into_inner()` | Call `ImguiContexts::remove`, continue updating while it returns `RemovalPending`, and take the returned `SuspendedContext` after both worlds acknowledge release. |

## Integration Model

### Contexts and Schedules

`ImguiContexts` owns every Dear ImGui Context in deterministic order. The primary Context runs `ImguiPrimaryContextPass`. Create an independent Context with a unique Bevy schedule:

```rust
#[derive(ScheduleLabel, Clone, Debug, Eq, PartialEq, Hash)]
struct InspectorPass;

let inspector = contexts.create(ImguiContextConfig::new(InspectorPass))?;
```

Register UI systems directly in that schedule. Use `contexts.configure(context_id, |context| ...)` only outside an active frame for font, ini, style, or other Context configuration. A live UI schedule cannot mutate or remove any registered Context through the safe API.

### Fonts

Configure a Context's font atlas through the non-send registry, normally from a `Startup` system:

```rust
const ROBOTO_MEDIUM: &[u8] = include_bytes!("../assets/Roboto-Medium.ttf");

fn configure_fonts(mut contexts: NonSendMut<ImguiContexts>) -> Result {
    let primary = contexts
        .primary_id()
        .ok_or("ImguiPlugin should install a primary Context before Startup")?;

    contexts.configure(primary, |context| {
        // SAFETY: the embedded bytes contain a complete, valid TTF and remain
        // unchanged for the duration of this call.
        let source = unsafe { FontSource::ttf_data_with_size(ROBOTO_MEDIUM, 18.0) };
        context.font_atlas().add_font(&[source])
    })?;
    Ok(())
}
```

External TTF/OTF, compressed TTF, and Base85 font sources use `unsafe` constructors because the native font loaders cannot prove that arbitrary input is complete and valid within the supplied buffer. Keep that boundary next to a trusted application asset whose completeness and loader validity the application guarantees. The renderer handles managed font-atlas texture updates; do not call `FontAtlas::build()` manually. Store returned `FontId` values in a non-send Bevy resource when they are needed by later UI systems.

The complete example embeds a custom font, retains its `FontId`, and selects it with `Ui::push_font`:

```console
cargo run -p dear-imgui-bevy --example custom_font
```

### Render and Input Routes

The primary Context automatically targets the unique eligible primary-window camera. Advanced cases declare route components on ordinary ECS entities:

```rust
commands.spawn((
    ImguiRenderRoute::new(context_id, camera),
    ImguiInputRoute::from_camera(context_id, camera),
));
```

Render and input routes are independent. An image or manual-texture render target never receives OS input implicitly; use `ImguiInputRoute::logical(context_id, host_window, region)` when a displayed image should be interactive. Ambiguous, stale, conflicting, or unsupported routes fail closed and publish structured entries through `ImguiDiagnostics` instead of broadcasting one Context to arbitrary cameras or windows.

### Composition Order

`ImguiRenderSystems::{BeforeOverlay, Overlay, AfterOverlay}` are installed in both `Core2d` and `Core3d` after scene post-processing and before upscaling. Custom passes can order themselves against these sets. A pass in `AfterOverlay` must preserve the current single-sample result; resolving an older MSAA attachment would overwrite the UI.

With the default `bevy-ui` feature, Dear ImGui is above Bevy UI. Reverse the order explicitly when needed:

```rust
ImguiPlugin::default()
    .with_ui_render_order(ImguiUiRenderOrder::BevyUiAboveImgui)
```

The renderer writes the final single-sample attachment, so the overlay composes consistently with 1x/4x MSAA, LDR/HDR cameras, Bevy UI, and ordered custom post-processing.

### Input Capture

The backend forwards Bevy messages without consuming them. Treat `ImguiInputCapture` as a gameplay policy hint:

```rust
fn gameplay_enabled(capture: Res<ImguiInputCapture>) -> bool {
    !capture.primary().wants_keyboard_input()
}
```

Use `primary()`, `context(context_id)`, or `window(entity)` according to the ownership scope. Focus, sticky key/button release, cursor, IME, camera viewports, and native platform windows are tracked per Context and host window.

With `default-features = false`, the primary Context still receives primary-window input and capture updates. Additional headless Context schedules run normally but remain non-interactive; explicit multi-Context input routing requires the `render` feature.

### Bevy Images

Register a Bevy `Image` through `ImguiBevyTextures::register_strong` or `register_weak`. The returned `ImguiTexture` is a cloneable RAII lease and can be passed directly to `ui.image(...)`; do not cache or manually recycle its raw `TextureId`.

A strong lease retains the image asset. The final lease drop withdraws the mapping, waits for render-world acknowledgement, and only then recycles its slot. Snapshots already in flight keep the mapping required by that frame.

### Context Removal

`ImguiContexts::remove` starts Context-local retirement. It returns `ImguiContextError::RemovalPending` while native window entities or render-world resources are still releasing; keep updating the app and retry. Other Contexts continue framing during that wait.

Drop paths never reach into ECS or the render world. Removing the registry queues complete owners for app-local retirement. Long-running hosts that require deterministic cleanup should explicitly remove Contexts and drive updates until removal succeeds before destroying the Bevy `App`.

### Docking and Native Multi-Viewport

Docking is enabled for the primary Context by default and can be changed with `ImguiPluginConfig::with_docking`. Native platform windows require both:

```rust
ImguiPlugin::new(
    ImguiPluginConfig::default().with_multi_viewport(true),
)
```

and the Cargo feature:

```console
cargo run -p dear-imgui-bevy --example game_engine --features multi-viewport
```

Each secondary viewport is scoped by `(ContextId, ViewportId)` and owns a Bevy window/camera pair. The backend handles mixed DPI, focus, cursor/IME feedback, transparent-window policy, callback ownership, renderer recovery, and ordered shutdown. Configure secondary-window presentation with `ImguiPluginConfig::with_viewport_window(ImguiViewportWindowConfig)`. Public viewport components are backend-owned, read-only identity projections: query them by reference and use `context_id()` / `viewport_id()`. The bridge queue and callback storage remain private.

Every Context that calls `ImguiContextConfig::with_multi_viewport(true)` requires the native `multi-viewport` Cargo feature. Admission returns `ImguiContextError::NativeMultiViewportUnavailable` when the selected target or feature set cannot provide native windows.

## Examples

Start with the first five examples for copy-runnable integration patterns:

| Example | Run command | Demonstrates |
| --- | --- | --- |
| [`simple`](examples/basic/simple.rs) | `cargo run -p dear-imgui-bevy --example simple` | Minimal default overlay with no marker or helper setup. |
| [`custom_font`](examples/basic/custom_font.rs) | `cargo run -p dear-imgui-bevy --example custom_font` | Outside-frame atlas configuration and non-send `FontId` storage. |
| [`custom_post_process`](examples/advanced/custom_post_process.rs) | `cargo run -p dear-imgui-bevy --example custom_post_process` | Public overlay ordering with post-processing, MSAA, HDR, and Bevy UI composition. |
| [`multiple_contexts`](examples/advanced/multiple_contexts.rs) | `cargo run -p dear-imgui-bevy --example multiple_contexts` | Independent Context schedules, windows, cameras, input routes, capture, and retryable teardown. |
| [`render_to_image`](examples/advanced/render_to_image.rs) | `cargo run -p dear-imgui-bevy --example render_to_image` | Offscreen Context, Bevy image lease, and explicit logical input mapping. |
| [`app_integration`](examples/app/app_integration.rs) | `cargo run -p dear-imgui-bevy --example app_integration` | Gameplay/editor integration using capture policy. |
| [`game_engine`](examples/game_engine/game_engine.rs) | `cargo run -p dear-imgui-bevy --example game_engine` | Docked editor surface and scene texture interop; add `--features multi-viewport` for native windows. |
| [`ecosystem`](examples/ecosystem/ecosystem.rs) | `cargo run -p dear-imgui-bevy --example ecosystem` | ImPlot, ImNodes, and ImGuizmo in one Context schedule. |
| [`bevy_plot_controls`](examples/ecosystem/bevy_plot_controls.rs) | `cargo run -p dear-imgui-bevy --example bevy_plot_controls` | Bevy scene controlled through ImPlot UI. |

## Current Boundaries

- Clipboard integration remains application-provided.
- Dear ImGui widgets do not generate Bevy accessibility nodes.
- File drop, gamepad navigation, and Bevy picking integration are not currently part of the backend.
- WASM builds are supported, but runtime IME and clipboard behavior depend on the browser host.

## Development Checks

```console
cargo nextest run -p dear-imgui-bevy --no-default-features
cargo nextest run -p dear-imgui-bevy
cargo check -p dear-imgui-bevy --all-targets --no-default-features --features render
cargo nextest run -p dear-imgui-bevy --features multi-viewport
cargo clippy -p dear-imgui-bevy --all-targets --features multi-viewport --no-deps -- -D warnings
cargo check -p dear-imgui-bevy --target wasm32-unknown-unknown --no-default-features --features wasm
cargo check -p dear-imgui-bevy --target wasm32-unknown-unknown --features wasm
cargo test --doc -p dear-imgui-bevy --features multi-viewport
cargo package -p dear-imgui-bevy
```
