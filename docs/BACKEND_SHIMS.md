# Native Backend Shims

`dear-imgui-sys::backend_shim` exposes repository-owned, unsafe C ABI wrappers
around selected official Dear ImGui backends. Core Rust bindings are generated
from cimgui; backend wrappers are maintained separately. A backend source being
vendored does not mean its wrapper is available.

## Available Native SDK Integrations

| Feature | Rust module | Targets | Application owns |
| --- | --- | --- | --- |
| `backend-shim-win32` | `win32` | Windows | HWND, event loop, message forwarding |
| `backend-shim-dx11` | `dx11` | Windows | D3D11 device/context, render targets, presentation |
| `backend-shim-dx12` | `dx12` | Windows | D3D12 device/direct queue, descriptor heap/allocator, command lists, barriers, fences |
| `backend-shim-osx` | `osx` | macOS | Cocoa NSView/window, main-thread event loop, autorelease pool |
| `backend-shim-metal` | `metal` | macOS, iOS | Metal device, render pass, command buffer/encoder, submission, autorelease pool |
| `backend-shim-android` | `android` | Android | NativeActivity/window/input, EGL lifecycle, packaging |
| `backend-shim-opengl3` | `opengl3` | Native targets | Current GL/GLES context, render target, presentation |

Native platform-specific modules are absent on other targets, allowing feature
unification across native platforms. All shim features are rejected on
`wasm32-unknown-unknown`; enabling the core `wasm` feature on a native target does
not select the browser ABI. `IMGUI_SYS_BUILD_MODE=bindings-only` cannot be combined with shims.
Core `prebuilt` mode (including legacy `SKIP_CC` with an explicit prebuilt selection) still permits local shim compilation; it does not promise a compiler-free build.
Core prebuilts do not contain backend libraries: the enabled shim and its
matching vendored upstream backend are always compiled locally.

Metal/Cocoa sources compile as Objective-C++ with ARC. Link dependencies come
from the platform SDK; no GLFW, SDL, or metal-cpp dependency is introduced.
macOS Cocoa retains upstream limitations for viewport decorations and parent
viewport handling. iOS platform input/window lifecycle remains application-owned.

## DirectX 12 Descriptor Boundary

Use `dx12::InitInfo` with the application's device, direct command queue,
shader-visible CBV/SRV/UAV descriptor heap, in-flight frame count, and numeric
DXGI render-target/depth formats. Zero is permitted for an unused depth format.
The shim requires both allocation and free callbacks, rather than the obsolete
single-font-descriptor initializer.

The initialization values are copied. Its `DescriptorAllocator` is borrowed and
must remain at a stable address until `dear_imgui_backend_dx12_shutdown` returns.
For example, store it in a `Box` together with stable user data. Allocation
returns a valid unused CPU/GPU descriptor pair through `usize`/`u64` output
pointers; free receives the same integer handles. Native D3D12 descriptor structs
and C++ callback conventions stay inside the shim. Callbacks must not unwind,
reenter the backend, or exhaust the allocator.

The application binds the SRV heap, render target, and necessary resource states
before drawing. Submit the command list and wait for GPU completion before
reusing in-flight resources or shutting down. A callback from shutdown may still
refer to the allocator's user data, so release it only after shutdown returns.

Run the WARP integration without a physical GPU or window:

```bash
cargo run -p dear-imgui-sys --example dx12_headless --features backend-shim-dx12
```

The example creates a software device, records and submits a Dear ImGui frame
to an offscreen target, uploads the dynamic font atlas, waits for its fence,
checks UI pixels by reading the target back, and verifies that descriptor allocations are released during shutdown. It uses
the raw sys API; it is not a safe `dear-imgui-rs` renderer adapter.

Windowed examples show platform integration and a complete frame loop:

```bash
# Windows: Win32 messages, DX11 device/swapchain, resize, and presentation.
cargo run -p dear-imgui-sys --example win32_dx11 --features backend-shim-win32,backend-shim-dx11

# Windows: interactive DX12 demo, descriptor allocation, barriers, and GPU fences.
cargo run -p dear-imgui-sys --example win32_dx12 --features backend-shim-win32,backend-shim-dx12

# Windows: an application-owned WGL context, Win32 messages, and OpenGL3 rendering.
cargo run -p dear-imgui-sys --example win32_opengl3 --features backend-shim-win32,backend-shim-opengl3

# macOS: Cocoa window/input and Metal drawable/command encoding.
cargo run -p dear-imgui-sys --example cocoa_metal --features backend-shim-osx,backend-shim-metal
```

Close the window to exit, or append `-- --frames=3` for a bounded manual smoke
run. These examples intentionally use single-window raw sys integration. They
show application-owned resize and teardown rather than an owning safe backend.
The DX12 and Metal examples wait for each submitted buffer to keep GPU lifetimes
clear; production applications normally use an in-flight ring. The DX12 window
uses hardware by default, falls back to software WARP when needed, and accepts
`-- --warp` to select WARP explicitly. Its control panel and the Dear ImGui demo
show interactive widgets, font-atlas uploads, resize, and presentation.

All three Win32 examples request a 960-by-640 logical-pixel client area, scale it
for the window's monitor, and constrain the initial window to that monitor's work
area. Font size, widget padding, and initial panel geometry follow the monitor's
DPI. Moving between monitors restores the original style before applying the new
absolute scale; the official Win32 backend applies `WM_DPICHANGED`'s suggested
window rectangle. Dynamic font-atlas uploads keep the fonts sharp at each scale.

The existing [Android NativeActivity example](../examples-android/dear-imgui-android-smoke/README.md)
combines Android and OpenGL ES 3 shims. It owns EGL, native-window references,
touch/keyboard translation, pause/resume, and window recreation. Build it with
the Android NDK and package it with `cargo-apk2`; it is not a desktop `cargo run`
binary. Together these examples exercise every sys-owned shim:

| Shims | Reference |
| --- | --- |
| Win32 + DX11 | [`win32_dx11.rs`](../dear-imgui-sys/examples/win32_dx11.rs) |
| Win32 + DX12 | [`win32_dx12.rs`](../dear-imgui-sys/examples/win32_dx12.rs) |
| DX12 | [`dx12_headless.rs`](../dear-imgui-sys/examples/dx12_headless.rs) |
| Win32 + OpenGL3 | [`win32_opengl3.rs`](../dear-imgui-sys/examples/win32_opengl3.rs) |
| Cocoa + Metal | [`cocoa_metal.rs`](../dear-imgui-sys/examples/cocoa_metal.rs) |
| Android + OpenGL ES 3 | [`android-smoke/src/lib.rs`](../examples-android/dear-imgui-android-smoke/src/lib.rs) |

## Frame and Ownership Contracts

Only one platform backend and one renderer backend may own the corresponding
slots of a Dear ImGui context. Keep that context current for backend calls and
shut down the renderer before the platform backend, context, and native objects.
The current ImGui context is process-global in this build. Serialize access
across contexts and threads, rather than independently locking each context.
Shims do not add synchronization or owning runtimes. The official Android
backend also stores window/timing globally and permits only one active instance
per process; keep its borrowed native window alive until shutdown returns.

Device-object entry points remain useful for device/context reset. Metal is an
exception to automatic restoration: after `destroy_device_objects`, explicitly
call `create_device_objects` with the original device before rendering again,
or perform shutdown/reinitialization. New-frame alone does not rebuild every
destroyed Metal object.

### Why the Shim Stays Small

The shim adapts C++ symbols, aggregate callback conventions, and Objective-C
ownership into a repository-owned C ABI. Application-owned windows, event
loops, GPU resources, and submission belong to the caller. An owning renderer
adapter belongs in a backend crate, where its interface can enforce those
lifetimes and the managed-frame protocol together.

DX12's borrowed allocator avoids a global callback registry and any reliance
on upstream private renderer-state layout. An owning adapter can hide its
stable storage behind a `Box`; the sys ABI makes that requirement explicit.
Build definitions use one registry for enabled-feature detection, compilation,
target selection, and linkage. We keep declarations explicit in Rust and C++
instead of maintaining a second binding generator for a small hand-written ABI.

The shims may mutate native `ImDrawData` and `ImTextureData`. Passing safe managed
draw data to an upstream renderer is not sufficient integration. A safe adapter
must process `PendingFrame::texture_requests`, reconcile request-bound feedback,
then draw the resulting `ReconciledFrame`. The SDL3 adapter is the maintained
reference. See [the custom backend guide](CUSTOM_BACKENDS.md).

## Support Decisions

| Other official backend | Current route / reason |
| --- | --- |
| Vulkan | `dear-imgui-ash` provides a Rust renderer; a duplicate C++ shim adds a Vulkan SDK/loader contract without a demonstrated integration requirement |
| WebGPU | `dear-imgui-wgpu` provides a Rust renderer; the upstream C API requires a separately selected native WebGPU SDK |
| SDL3 / SDLRenderer3 / SDLGPU3 | `dear-imgui-sdl3` owns framework discovery, compilation, and safe integration |
| GLFW | Keep GLFW discovery and linkage in a dedicated backend/application; it is not a platform SDK dependency |
| DX9 / DX10 / OpenGL2 / SDL2 / GLUT / Allegro5 | Legacy or framework-specific paths; no new first-party wrappers planned without a concrete application requirement |
| Metal 4 | Requires Apple Silicon and macOS 26+; keep the broadly compatible Metal path until there is a specific Metal 4 integration requirement |
| Null | Existing headless core/custom-renderer paths cover tests; a second backend lifecycle is unnecessary for now |

## Verification

Native CI links every exposed entry point for Windows DX11/DX12/Win32/OpenGL3,
macOS Metal/Cocoa/OpenGL3, and Linux OpenGL3. Windows also executes the WARP
example through an integration test. Apple symbol tests require the real SDK;
they do not start AppKit or claim GPU rendering coverage. iOS Metal is compiled
against the simulator SDK in macOS CI. Android CI builds and links the standalone
NativeActivity/OpenGL ES example with the NDK and lints its Rust integration;
it does not run an emulator or validate APK packaging.
