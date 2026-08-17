# dear-imgui-rs

[![Crates.io](https://img.shields.io/crates/v/dear-imgui-rs.svg)](https://crates.io/crates/dear-imgui-rs)
[![Documentation](https://docs.rs/dear-imgui-rs/badge.svg)](https://docs.rs/dear-imgui-rs)
[![Crates.io Downloads](https://img.shields.io/crates/d/dear-imgui-rs.svg)](https://crates.io/crates/dear-imgui-rs)
[![Made with Rust](https://img.shields.io/badge/made%20with-Rust-orange.svg)](https://www.rust-lang.org)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

`dear-imgui-rs` is a Rust bindings ecosystem for Dear ImGui, featuring docking support, WGPU/GL/Vulkan backends, and a rich set of extensions (ImPlot/ImPlot3D, ImGuizmo/ImGuIZMO.quat, ImNodes, imgui-node-editor, ImGuiColorTextEdit, ImGui Test Engine, file browser, reflection-based UI).

<p align="center">
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/game-engine-docking.png" alt="Docking" width="49%"/>
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/imguizmo-basic.png" alt="ImGuizmo" width="49%"/>
  <br/>
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/implot-basic.png" alt="ImPlot" width="49%"/>
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/imnodes-basic.png" alt="ImNodes" width="49%"/>
  <br/>
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/node-editor-showcase.png" alt="imgui-node-editor blueprints showcase" width="49%"/>
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/file_browser_imgui.png" alt="File Browser" width="49%"/>
  <br/>
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/imguizmo-quat-basic.png" alt="ImGuizmo.Quat" width="49%"/>
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/implot3d-basic.png" alt="ImPlot3D" width="49%"/>
  <br/>
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/wasm.png" alt="WASM" width="49%"/>
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/docking-sdl3-glow.png" alt="Docking" width="49%"/>

</p>

## What’s in this repo

- Core
  - `dear-imgui-sys` — low-level FFI via cimgui (docking branch), with pregenerated bindings for Dear ImGui v1.92.9b
  - `dear-imgui-rs` — safe, idiomatic Rust API (RAII + builder style similar to imgui-rs)
  - Backends: `dear-imgui-wgpu`, `dear-imgui-glow`, `dear-imgui-ash`, `dear-imgui-winit`, `dear-imgui-sdl3`, `dear-imgui-bevy`
    - `dear-imgui-bevy` is an experimental Bevy-native backend on Bevy `0.19.1`, with docking,
      texture interop, and native multi-viewport on supported targets.
  - `dear-app` — generation-aware Winit + WGPU application runtime (docking, themes, add-ons)
- Extensions
  - `dear-imguizmo` — 3D gizmo (cimguizmo C API) + a pure‑Rust GraphEditor
  - `dear-imnodes` — node editor (cimnodes C API)
  - `dear-node-editor` — richer native node editor (cimnodes_editor / imgui-node-editor)
  - `dear-implot` — plotting (cimplot C API)
  - `dear-implot3d` — 3D plotting (cimplot3d C API)
  - `dear-imguizmo-quat` — quaternion + 3D gizmo (cimguizmo_quat C API)
  - `dear-imgui-cte` — Preview context-bound code editor, text diff, autocomplete, and notifications (cimCTE / ImGuiColorTextEdit)
  - `dear-imgui-test-engine` — Dear ImGui UI automation/test runner integration
  - `dear-file-browser` — native dialogs (rfd) + pure ImGui in-UI file browser
  - `dear-imgui-reflect` — reflection-based UI helpers (auto-generate ImGui widgets from Rust types)

All crates are maintained together in this workspace.

## Hello, ImGui (Hello World)

```rust
use dear_imgui_rs::*;

let mut ctx = Context::create();
// If you are not using a platform backend (e.g. dear-imgui-winit / dear-imgui-sdl3),
// you must set `io.DisplaySize` before calling `Context::frame()`.
ctx.io_mut().set_display_size([300.0, 120.0]);
ctx.io_mut().set_delta_time(1.0 / 60.0);
ctx.font_atlas()
   .try_claim_legacy_renderer()
   .expect("the backend-free example uses the legacy font atlas")
   .build();

let ui = ctx.frame();
ui.window("Hello")
  .size([300.0, 120.0], Condition::FirstUseEver)
  .build(|| {
      ui.text("Hello, world!");
      if ui.button("Click me") { println!("clicked"); }
  });
// This backend-free snippet uses the explicit legacy path. A managed renderer instead retains a
// SynchronousRendererConsumer, calls `ctx.render(&consumer)`, reconciles every texture request,
// and draws the resulting ReconciledFrame.
let frame = ctx.render_legacy();
let _draw_data = frame.draw_data();
// Tip: pass `.opened(&mut open)` if you want a title-bar close button (X).

// Tip: For fallible creation, use `Context::try_create()`
```

## Migration Notes

The README focuses on current supported usage. For source-breaking migrations between releases,
read `CHANGELOG.md`; for release-train, dependency, MSRV, and backend compatibility baselines, see
`docs/COMPATIBILITY.md`.

The safe API intentionally encodes Dear ImGui FFI invariants in Rust types. Source breaks are
accepted when the previous safe wrapper shape could preserve stale handles, unchecked sizes,
invalid sentinels, wrong-context access, or other states that should remain outside safe Rust.

The 0.16 train is intentionally breaking. Alpha.1 introduced the main architecture migrations:

- legacy Columns to Tables and imperative `DockBuilder` code to declarative `DockLayout`;
- global reflection helpers to an owned `ReflectSession` and per-frame `Inspector`;
- borrowed file-browser filesystems to `FileDialogState`-owned blocking or background capabilities;
- callback access to shared `ContextBinding` and ordered Context attachments;
- borrowed texture pointers and pseudo-owned draw data to Context-owned `ManagedTextureId`, a
  one-use `RenderedFrame`, move-only `FrameSnapshot`, and request-bound renderer feedback;
- manual Winit/WGPU/Glow/Ash callback registration to owning runtimes with explicit,
  idempotent shutdown (WGPU attach is safe; Glow remains unsafe for current-GL-context and
  share-group lineage; Ash remains unsafe for raw Vulkan handle lineage);
- ad hoc Test Engine frame pumps to `TestRunner`, whose five product outcomes remain distinct
  from infrastructure errors;
- implicit WASM target selection to the explicit `wasm32-unknown-unknown` plus `wasm` feature contract; and
- `dear_imgui_sys::IMGUI_VERSION` to `BINDING_VERSION`, without a compatibility alias.

Alpha.2 replaces alpha.1's `RenderedFrame` with the linear `PendingFrame` -> `ReconciledFrame` protocol and replaces the mode-selecting `RendererConsumer` with separate synchronous and detached capabilities. It also hardens Context, Docking, multi-viewport renderer, Test Engine, and Bevy frame ownership. See the [alpha.2 migration table](CHANGELOG.md#0160-alpha2); applications migrating directly from 0.15 should also read the [alpha.1 migration guide](CHANGELOG.md#0160-alpha1).

## Examples

```text
# Fresh source checkout. The native -sys crates need vendored submodules.
git clone --recursive https://github.com/Latias94/dear-imgui-rs
cd dear-imgui-rs

# If you already cloned without --recursive, run this inside the repo.
git submodule update --init --recursive
```

Start with the application APIs, then move down to backend integration only when the application
needs to own its window, surface, render pass, or native multi-viewport route:

```text
cargo run -p dear-app --example hello
cargo run -p dear-imgui-examples --bin hello_world
cargo run -p dear-imgui-examples --bin fallible_frame
cargo run -p dear-imgui-examples --bin application_lifecycle
```

Focused safe API examples keep the host runtime out of the way:

```text
cargo run -p dear-imgui-examples --bin custom_font_minimal
cargo run -p dear-imgui-examples --bin managed_texture_minimal
cargo run -p dear-imgui-examples --bin dockspace_minimal
cargo run -p dear-imgui-examples --bin task_organizer
```

Each optional extension has a small copyable entry point; larger `*_showcase` binaries are kept
separate:

```text
cargo run -p dear-imgui-examples --bin implot_minimal --features implot
cargo run -p dear-imgui-examples --bin implot3d_minimal --features implot3d
cargo run -p dear-imgui-examples --bin imnodes_minimal --features imnodes
cargo run -p dear-imgui-examples --bin cte_minimal --features cte
cargo run -p dear-imgui-examples --bin cte_showcase --features cte
cargo run -p dear-imgui-examples --bin imguizmo_minimal --features imguizmo
cargo run -p dear-imgui-examples --bin imguizmo_quat_minimal --features imguizmo-quat
cargo run -p dear-imgui-examples --bin node_editor_minimal --features node-editor
```

The low-level renderer references expose the complete native lifecycle deliberately:

```text
cargo run -p dear-imgui-examples --bin winit_wgpu
cargo run -p dear-imgui-examples --bin winit_glow
cargo run -p dear-imgui-examples --bin winit_ash
cargo run -p dear-imgui-examples --bin multi_viewport_wgpu --features multi-viewport
cargo run -p dear-imgui-examples --bin sdl3_glow_multi_viewport --features sdl3-glow-multi-viewport
```

See [`examples/README.md`](examples/README.md) for the complete four-level catalog, including
feature flags, prerequisites, and the next example for every public binary. Private runtime probes
under `examples/ci/` are release evidence and are intentionally excluded from that learning path.

## Installation

The `0.16.0` stable release train uses the compatible `"0.16"` requirement across the workspace crates. Applications staying on `0.15.1` can use the [v0.15.1 README](https://github.com/Latias94/dear-imgui-rs/blob/v0.15.1/README.md) for its API and installation snippets.

### Core + Backends

```toml
[dependencies]
dear-imgui-rs = "0.16"
# Choose a backend + platform integration
dear-imgui-wgpu = "0.16"  # or dear-imgui-glow / dear-imgui-ash
dear-imgui-winit = "0.16" # or dear-imgui-sdl3
```

`dear-imgui-wgpu` 0.16 defaults to WGPU 30. WGPU 29, 28, and 27 remain available as separate, mutually exclusive compatibility features:

```toml
[dependencies]
dear-imgui-rs = "0.16"
dear-imgui-wgpu = { version = "0.16", default-features = false, features = ["wgpu-29"] }
dear-imgui-winit = "0.16"
```

Replace `wgpu-29` with `wgpu-28` or `wgpu-27` when integrating an older WGPU application.

### Application Runtime (Recommended for Quick Start)

```toml
[dependencies]
dear-app = "0.16" # State-owning Winit + WGPU runtime with docking support
```

### Apple Platform Examples

For Apple/mobile integration, use the repository-owned iOS smoke examples as
reference integrations:

- `examples-ios/dear-imgui-ios-smoke`
  - `dear-imgui-winit + dear-imgui-wgpu`
- `examples-ios/dear-imgui-ios-sdl3-smoke`
  - `dear-imgui-sdl3 + dear-imgui-wgpu`

These examples exist to validate and teach the integration boundary. They are
not a turn-key mobile runtime layer.

For Apple-specific integration notes and example boundaries, see
[`docs/workstreams/apple-platform-support.md`](docs/workstreams/apple-platform-support.md).
For the checked-in iOS smoke templates and a quick route-selection index, see
[`examples-ios/README.md`](examples-ios/README.md).

## Low-level Backend Shim And Android

Most users should stay on the safe backends (`dear-imgui-winit`,
`dear-imgui-sdl3`, `dear-imgui-wgpu`, `dear-imgui-glow`, `dear-imgui-ash`).
If you need to integrate a custom engine or renderer, start with
[`docs/CUSTOM_BACKENDS.md`](docs/CUSTOM_BACKENDS.md). The dependency-light synchronous contract is
also executable:

```text
cargo run -j 1 -p dear-imgui-rs --example custom_renderer_headless
```

For engine integrations or platform stacks that are not wrapped by a dedicated
crate yet, `dear-imgui-sys` can expose selected official backend pieces behind
`backend-shim-*` feature gates. These are repository-owned C shim entry points,
not direct promises about the upstream `imgui_impl_*` C++ ABI.

Example: low-level Android route without a dedicated Android convenience crate:

```toml
[dependencies]
dear-imgui-rs = "0.16"
dear-imgui-sys = { version = "0.16", features = ["backend-shim-android", "backend-shim-opengl3"] }
```

Recommended ownership split:

- `dear-imgui-rs` owns the safe core `Context`, `Io`, frame lifecycle, and draw
  data handling.
- `dear-imgui-sys::backend_shim::{android, opengl3}` exposes the low-level
  official backend pieces.
- The application still owns Android lifecycle glue, EGL / GLES context
  creation, packaging, and signing.

The repository includes a concrete template for this path at
`examples-android/dear-imgui-android-smoke/`. It is intentionally kept outside
the default workspace build so we can document and validate the Android route
without expanding the normal desktop/web CI matrix, and it is not intended to
be a separately published runtime crate.
For the current Android smoke-template overview, see
[`examples-android/README.md`](examples-android/README.md).

If your application already uses SDL3, prefer `dear-imgui-sdl3` as the higher
level Android integration direction. Even there, the application still owns SDL3
Android packaging, NDK toolchain configuration, and final APK / app-bundle
assembly.

### Extensions

```toml
[dependencies]
# Plotting
dear-implot = "0.16"   # 2D plotting
dear-implot3d = "0.16" # 3D plotting

# 3D Gizmos
dear-imguizmo = "0.16"      # Standard 3D gizmo + GraphEditor
dear-imguizmo-quat = "0.16" # Quaternion-based gizmo

# Node Editor
dear-imnodes = "0.16"
dear-node-editor = "0.16" # native-only; add feature "blueprints" for stack layout

# Test automation
dear-imgui-test-engine = "0.16"

# File Browser
dear-file-browser = "0.16" # Native dialogs + ImGui file browser

# Reflection-based UI helpers
dear-imgui-reflect = "0.16"
```

`dear-imgui-cte` is currently an Unreleased Preview on the main branch and is
not part of the published `0.16.0` crate set. Until it joins the next synchronized
release train, take the safe and sys crates from the same workspace checkout or
the same Git revision as the core crates.

### Reflection-based UI (dear-imgui-reflect)

`dear-imgui-reflect` lets you derive `ImGuiReflect` on your structs/enums and automatically get Dear ImGui editors for them. It is inspired by the C++ ImReflect library but implemented in pure Rust on top of `dear-imgui-rs`.

Typical flow:

```rust
use dear_imgui_reflect as reflect;
use reflect::{ImGuiReflect, ImGuiReflectExt};

#[derive(ImGuiReflect, Default)]
struct Settings {
    #[imgui(slider, min = 0, max = 100)]
    volume: i32,
    fullscreen: bool,
}

fn ui_frame(
    session: &reflect::ReflectSession,
    ui: &reflect::imgui::Ui,
    settings: &mut Settings,
) {
    let mut inspector = ui.inspector(session);
    inspector.input("Settings", settings);
}
```

## Build Strategy

- Default: build from source on all platforms. Prebuilt binaries are optional and off by default.
- Source builds from a repository checkout require initialized submodules because the native C/C++ sources live under `dear-imgui-sys/third-party/cimgui` and extension `third-party/` directories. Use `git clone --recursive ...` for a fresh checkout, or `git submodule update --init --recursive` inside an existing checkout.
- Windows source builds support `x86_64-pc-windows-gnullvm` and `aarch64-pc-windows-gnullvm` with an installed llvm-mingw toolchain; Visual Studio is not required for those targets. The x64 target is runtime/ABI tested in both its default and `+crt-static` modes. The Arm64 gnullvm target and `aarch64-pc-windows-msvc` `/MD` route are cross-link/PE-tested only and are not executed in CI.
- The gnullvm support claim covers the core and maintained C++ extension sys crates with their default native dependency set. It does not add gnullvm prebuilts or cover the SDL3 backend's external CMake dependency, FreeType, or other optional native packages.
- Windows core packages cover both MSVC CRT modes (MD/MT), with optional `freetype` and a distinct `stack-layout` artifact profile. Linux and macOS core archives are also eligible for opt-in release download on their supported targets; source builds remain the default everywhere.
- Opt-in core prebuilt download from Release: enable `dear-imgui-rs/prebuilt`, or `dear-imgui-sys/prebuilt` when depending on the low-level crate directly (the env toggle `IMGUI_SYS_USE_PREBUILT=1` is still accepted but requires that feature). `IMGUI_SYS_LIB_DIR` points to the static-library directory and requires its matching `manifest.txt` there or in the parent artifact root, while `IMGUI_SYS_PREBUILT_URL` should point to a package-tool-generated archive. Bare core `.a`/`.lib` files without adjacent provenance are rejected.
- Every accepted core prebuilt manifest records crate/version, target, link type, MSVC CRT, normalized features, cimgui and Dear ImGui revisions, and the binding-spec hash. Missing, unknown, duplicate, or mismatched fields reject the core artifact instead of falling back to an ABI guess. Extension `*-sys` crates retain their crate-specific prebuilt contracts.
- `dear-implot`, `dear-implot3d`, `dear-imnodes`, `dear-imguizmo`,
  `dear-imguizmo-quat`, `dear-imgui-cte`, and `dear-node-editor` forward both `prebuilt` and
  `build-from-source` through the core and their matching sys crate. Source wins
  if Cargo unifies both features. The first six also forward `wasm`;
  `dear-node-editor` remains native-only.

Test engine hooks (important):

- Enabling `dear-imgui-sys/test-engine` defines `IMGUI_ENABLE_TEST_ENGINE` and makes the ImGui objects reference hook symbols (e.g. `ImGuiTestEngineHook_*`).
  - Test-engine hooks are native source-only: the feature implies `build-from-source`, takes precedence if Cargo also unifies `prebuilt`, and is rejected for the WASM import provider.
  - When enabled, `dear-imgui-sys` also provides the hook symbols, so workspace feature-unification won't cause linker errors.
  - To actually run UI automation/tests, link `dear-imgui-test-engine` (or `dear-imgui-test-engine-sys`), which registers the real hook implementations at runtime.

Common env-var shapes used by `-sys` crates (consult each crate README for its exact contract):
- `<CRATE>_SYS_LIB_DIR` — explicit library directory; core `IMGUI_SYS_LIB_DIR` also requires the strict matching `manifest.txt` in that directory or its parent artifact root
- `<CRATE>_SYS_PREBUILT_URL` — explicit URL or local artifact path; for core use a packaged archive rather than a bare `.a/.lib` (HTTP(S) and `.tar.gz` extraction require feature `prebuilt`)
- `<CRATE>_SYS_USE_PREBUILT=1` — allow auto download from GitHub Releases (requires feature `prebuilt`)
- `<CRATE>_SYS_PACKAGE_DIR` — local dir with `.tar.gz` packages
- `<CRATE>_SYS_CACHE_DIR` — cache root for downloads/extraction
- `<CRATE>_SYS_SKIP_CC` — skip C/C++ compilation
- `<CRATE>_SYS_FORCE_BUILD` — force source build
- `IMPLOT_SYS_USE_CMAKE` — prefer CMake for `dear-implot-sys` when available; otherwise cc
- `CARGO_NET_OFFLINE=true` — forbid network; use only local packages or repo prebuilt

FreeType: enable once anywhere. Turning on `freetype` in `dear-implot`, `dear-imnodes`, `dear-node-editor`, `dear-imguizmo`, `dear-imguizmo-quat`, or `dear-imgui-test-engine` propagates to `dear-imgui-sys`. Source builds probe `pkg-config freetype2` first and then vcpkg's `freetype` port; if neither is available, the build fails instead of silently disabling FreeType. When using a prebuilt `dear-imgui-sys` with freetype, ensure the package manifest includes `features=freetype` (our packager writes this).

Blueprint stack layout is native-only and opt-in. Enable `dear-node-editor/blueprints` (or `dear-imgui-rs/stack-layout` for direct use) to select the patched Dear ImGui core artifact. Prebuilt manifests must match exactly: normal and freetype-only artifacts omit `stack-layout`, while blueprint artifacts declare it and use the `-stack-layout` archive suffix. When `freetype` is also enabled, the manifest and archive name declare both features. The four feature combinations (normal, freetype, stack-layout, and stack-layout + freetype) are never substituted for each other.

Quick examples (enable auto prebuilt download):

- Feature: `cargo build -p dear-imgui-rs --features prebuilt`
- Env (Unix): `IMGUI_SYS_USE_PREBUILT=1 cargo build -p dear-imgui-rs --features prebuilt`
- Env (Windows PowerShell): `$env:IMGUI_SYS_USE_PREBUILT='1'; cargo build -p dear-imgui-rs --features prebuilt`

## Compatibility (0.16.0)

The workspace follows a release-train model. The table below lists the combinations validated for the 0.16.0 release. See [docs/COMPATIBILITY.md](https://github.com/Latias94/dear-imgui-rs/blob/main/docs/COMPATIBILITY.md) for version history and compatibility policy.

Core

| Crate           | Version | Notes                                     |
|-----------------|---------|-------------------------------------------|
| dear-imgui-rs   | 0.16.0 | Safe Rust API over dear-imgui-sys     |
| dear-imgui-sys  | 0.16.0 | Dear ImGui v1.92.9b docking via cimgui |

Backends

| Crate            | Version | External deps     | Notes                          |
|------------------|---------|-------------------|--------------------------------|
| dear-imgui-wgpu  | 0.16.0 | wgpu = 30/29/28/27 | WebGPU renderer; WGPU 30 default, native Winit/SDL3 multi-viewport, browser single-window |
| dear-imgui-glow  | 0.16.0 | glow = 0.18       | OpenGL renderer (winit/glutin) |
| dear-imgui-ash   | 0.16.0 | ash = 0.38        | Native Vulkan renderer with Winit/SDL3 multi-viewport adapters |
| dear-imgui-winit | 0.16.0 | winit = 0.30.13   | Winit platform backend         |
| dear-imgui-sdl3  | 0.16.0 | sdl3 = 0.18.4     | SDL3 platform backend with optional official OpenGL3, SDLRenderer3, and SDLGPU3 renderers |
| dear-imgui-bevy  | 0.16.0 | Bevy = 0.19.1     | Experimental Bevy-native backend with docking, texture interop, and native multi-viewport |

Application Runtime

| Crate     | Version | Requires dear-imgui-rs | Notes                                            |
|-----------|---------|------------------------|--------------------------------------------------|
| dear-app  | 0.16.0 | 0.16.0       | Generation-aware Winit + WGPU application runtime |

Extensions

| Crate               | Version | Requires dear-imgui-rs | Sys crate                   | Notes                                  |
|---------------------|---------|------------------------|-----------------------------|----------------------------------------|
| dear-implot         | 0.16.0 | 0.16.0 | dear-implot-sys 0.16.0 | 2D plotting                         |
| dear-imnodes        | 0.16.0 | 0.16.0 | dear-imnodes-sys 0.16.0 | WASM-capable node editor            |
| dear-node-editor    | 0.16.0 | 0.16.0 | dear-node-editor-sys 0.16.0 | Native imgui-node-editor; optional blueprints profile |
| dear-imguizmo       | 0.16.0 | 0.16.0 | dear-imguizmo-sys 0.16.0 | 3D gizmo + GraphEditor              |
| dear-file-browser   | 0.16.0 | 0.16.0 | —                              | State-owned ImGui UI + native dialog backends |
| dear-implot3d       | 0.16.0 | 0.16.0 | dear-implot3d-sys 0.16.0 | 3D plotting                       |
| dear-imguizmo-quat  | 0.16.0 | 0.16.0 | dear-imguizmo-quat-sys 0.16.0 | Quaternion gizmo              |
| dear-imgui-test-engine | 0.16.0 | 0.16.0 | dear-imgui-test-engine-sys 0.16.0 | UI automation and test runner |
| dear-imgui-reflect  | 0.16.0 | 0.16.0 | —                              | Session-owned reflection UI              |

The workspace MSRV is Rust 1.92. The experimental Bevy backend requires Rust 1.95 because Bevy 0.19 does. Select exactly one WGPU major; `dear-app` follows the WGPU 30 default.

Maintenance rules

- Upgrade dear-imgui-sys together with all -sys extensions to avoid C ABI/API drift.
- dear-imgui-rs upgrades may require minor changes in backends/extensions if public APIs changed.
- Backend external deps (wgpu/winit/glow) have their own breaking cycles and may trigger a coordinated bump of the unified publishable release train.

### CI and Releases

- Dispatch `.github/workflows/release.yml` from `main` with the tag matching
  `workspace.package.version`. It requires successful CI for the exact commit,
  builds and consumes every prebuilt profile, publishes the crate train, and
  creates the tag and GitHub Release.
- Its reusable `.github/workflows/prebuilt-binaries.yml` job builds and then
  consumes the complete core plus six-safe-extension package set for Linux
  x86_64, macOS x86_64/aarch64, and Windows MSVC `/MD`/`/MT`. There is no
  selective `crates` input in the release contract.
- Packages use names such as
  `dear-<name>-prebuilt-<version>-<target>-static[-stack-layout][-freetype][-mt|-md].tar.gz`
  and embed the same candidate SHA in their manifests.
- A failed target or failed isolated consumer stops the release before any
  publishing credential is issued. Build logs and packages remain available as
  normal workflow artifacts for approximately 30 days.
- crates.io publication uses short-lived Trusted Publishing credentials from
  the protected `release` environment. Every prebuilt manifest records the
  exact candidate commit, and the GitHub Release includes `SHA256SUMS`.
- Release download URLs default to the owner/repository configured in
  `tools/build-support/src/lib.rs`. Override them with
  `BUILD_SUPPORT_GH_OWNER` and `BUILD_SUPPORT_GH_REPO`.

## Version & FFI

- FFI layer is generated from the cimgui `docking_inter` branch matching Dear ImGui v1.92.9b.
- Core cimgui calls cross a C ABI boundary, but callback-bearing `ImGuiPlatformIO` fields still have C++ compiler ABI-sensitive signatures. The repository-owned aggregate callback shims translate the seven `ImVec2`/`ImVec4` by-value slots to pointer/out-parameter C callbacks and are exercised on MSVC `/MD` and `/MT`.
- Checked-in bindings are target profiles, not one universal header snapshot: Windows 64-bit, supported non-Windows native targets, and the fixed browser import ABI each have a separate reproducible artifact.
- `BINDING_VERSION` is the Rust binding crate release version. It is the direct replacement for the old `IMGUI_VERSION` constant; use `igGetVersion()` to inspect the linked Dear ImGui runtime.
- The safe layer follows Rust ownership and RAII conventions; raw `dear-imgui-sys` remains the explicitly unsafe escape hatch.

## Main User-Facing Crates

```text
dear-imgui-rs/         # Safe Rust bindings (renamed from dear-imgui)
dear-imgui-sys/        # cimgui FFI (docking; ImGui v1.92.9b)
backends/
  dear-imgui-wgpu/     # WGPU renderer
  dear-imgui-glow/     # OpenGL renderer
  dear-imgui-ash/      # Vulkan/Ash renderer
  dear-imgui-winit/    # Winit platform
  dear-imgui-sdl3/     # SDL3 platform/renderers
  dear-imgui-bevy/     # Bevy integration
dear-app/              # State-owning application runtime (Winit + WGPU + docking + themes)
extensions/
  dear-imguizmo/       # ImGuizmo + pure‑Rust GraphEditor
  dear-imnodes/        # ImNodes (node editor)
  dear-node-editor/    # imgui-node-editor (native-only node editor)
  dear-implot/         # ImPlot (2D plotting)
  dear-implot3d/       # ImPlot3D (3D plotting)
  dear-imguizmo-quat/  # ImGuIZMO.quat (quaternion gizmo)
  dear-imgui-cte/      # Preview ImGuiColorTextEdit editor, diff, autocomplete, and notifications
  dear-imgui-test-engine/ # ImGui Test Engine integration
  dear-file-browser/   # File dialogs (rfd) + pure ImGui browser
  dear-imgui-reflect/  # Reflection-based UI helpers for dear-imgui-rs
  dear-imgui-reflect-derive/ # Derive macro for reflection-based inspectors
```

Native extension crates have adjacent low-level `*-sys` companions, and release
tooling lives under `tools/`; see the workspace manifest for the complete member
list.

## WebAssembly (WASM) support

The supported Rust target is exactly `wasm32-unknown-unknown`. Every dependency path to the core crate must explicitly enable `wasm`; the target alone no longer selects browser bindings:

```toml
[dependencies]
dear-imgui-rs = { git = "https://github.com/Latias94/dear-imgui-rs", branch = "main", features = ["wasm"] }
```

The Rust module imports cimgui from the fixed provider name `imgui-sys-v1`, and both modules share one `WebAssembly.Memory`. The provider name is part of the ABI and is not configurable. Provider ABI v1 includes the checked numeric formatting and parsing contract; v0 artifacts must be rebuilt rather than renamed or remapped. Builds for WASI, Rust Emscripten targets, missing `wasm` feature forwarding, `wasm + stack-layout`, `wasm + prebuilt`, or `wasm + test-engine` fail rather than falling back to another binding profile.

Quick start:

```bash
rustup target add wasm32-unknown-unknown

# Install wasm-bindgen-cli at the version recorded in Cargo.lock, plus wasm-tools.
cargo run -p xtask -- web-demo
cargo run -p xtask -- build-cimgui-provider

python3 -m http.server -d target/web-demo 8080
# Open http://127.0.0.1:8080
```

Pass a comma-separated extension list to the demo command, for example `cargo run -p xtask -- web-demo implot,imnodes` or `cargo run -p xtask -- web-demo cte`. Native multi-viewport, `dear-node-editor`, and the blueprints stack-layout profile are unavailable in the browser; use `dear-imnodes` for the current WASM node-editor route.

For binding verification, provider construction, feature-forwarding checks, and troubleshooting, see [`docs/WASM.md`](docs/WASM.md).

## Limitations

- **Multi-viewport support**
  - **SDL3 + OpenGL3**: supported via upstream C++ backends (`imgui_impl_sdl3` + `imgui_impl_opengl3`).
    - Example: `cargo run -p dear-imgui-examples --bin sdl3_opengl_multi_viewport --features multi-viewport,sdl3-opengl3`
  - **Winit/SDL3 + WGPU**: native-only owning renderer runtimes with WGPU 30 by default. Select exactly one platform route and call renderer shutdown before platform shutdown and object destruction; the runtime owns callback address stability.
    - Winit example: `cargo run -p dear-imgui-examples --bin multi_viewport_wgpu --features multi-viewport`
    - SDL3 example: `cargo run -p dear-imgui-examples --bin sdl3_wgpu_multi_viewport --features sdl3-wgpu-multi-viewport`
  - **Winit/SDL3 + Ash**: native-only Vulkan adapters share one owning callback/swapchain runtime for classic render-pass and dynamic-rendering routes. Attachment is unsafe only because raw Vulkan handle/device lineage cannot be proven; the runtime owns callback address stability and ordered shutdown.
    - Winit example: `cargo run -p dear-imgui-examples --bin multi_viewport_ash --features ash-winit-multi-viewport`
    - SDL3 example: `cargo run -p dear-imgui-examples --bin sdl3_ash_multi_viewport --features sdl3-ash-multi-viewport`
  - Call `Context::enable_multi_viewport()` for viewports. Enable `ConfigFlags::DOCKING_ENABLE` separately when the application also needs docking.
  - Bevy native viewports use the exact `bevy_winit::WINIT_WINDOWS` mapping and detached Winit monitor snapshots; ECS monitor geometry and raw handles are not identity fallbacks. A secondary window stays hidden until its mapping and native policy lease are ready, with `NO_INPUTS` and `NO_FOCUS_ON_CLICK` enforced at the window boundary. Query `ImguiNativeViewportSupport::get(context_id)` for coordinate capability and the `NativeMonitor` diagnostic batch for collection/provenance degradation; on Wayland the status is `GlobalDesktopCoordinatesUnavailable` and docking stays inside the host window.
  - **winit + OpenGL (glow/glutin)**: no official multi-viewport stack at the moment.
    Use SDL3 + OpenGL3 / SDL3 + Glow if you need multi-viewport OpenGL.
- **WebAssembly (WASM)**: Supported via the import-style build described above; some features
  (clipboard, raw draw callbacks, multi-viewport) remain disabled on wasm.
- **dear-node-editor**: First integration phase is native-only. Use `dear-imnodes` for the current
  wasm node-editor path.

## Related Projects

If you're working with graphics applications in Rust, you might also be interested in:

- **[asset-importer](https://github.com/Latias94/asset-importer)** - A comprehensive Rust binding for the latest [Assimp](https://github.com/assimp/assimp) 3D asset import library, providing robust 3D model loading capabilities for graphics applications
- **[boxdd](https://github.com/Latias94/boxdd)** - Safe, ergonomic Rust bindings for Box2D v3.

## Acknowledgments

This project builds upon the excellent work of several other projects:

- **[Dear ImGui](https://github.com/ocornut/imgui)** by Omar Cornut - The original C++ immediate mode GUI library
- **[cimgui](https://github.com/cimgui/cimgui)** - The C API layer used by the core Dear ImGui sys crate
- **[imgui-rs](https://github.com/imgui-rs/imgui-rs)** - Provided the API design patterns and inspiration for the Rust binding approach
- **[easy-imgui-rs](https://github.com/rodrigorc/easy-imgui-rs/)** by rodrigorc
- **[imgui-wgpu-rs](https://github.com/Yatekii/imgui-wgpu-rs/)** - Provided reference implementation for WGPU backend integration
- **[imgui-node-editor](https://github.com/thedmd/imgui-node-editor)** by Michał Cichoń - Native node editor implementation and blueprint-style example references
- **[cimnodes_editor](https://github.com/cimgui/cimnodes_editor)** - C wrapper used for the `dear-node-editor-sys` binding layer

## License

Dual-licensed under either of:

- Apache License, Version 2.0 (<http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license (<http://opensource.org/licenses/MIT>)

Vendored third-party native projects keep their own licenses. In particular,
`imgui-node-editor` is MIT-licensed, and the stack layout compatibility shim in
`dear-imgui-sys` is derived from its MIT-licensed vendored stack layout
extension. See the relevant `*-sys` README files and
`dear-imgui-sys/THIRD_PARTY_NOTICES.md` for details.
