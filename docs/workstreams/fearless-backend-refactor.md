# Fearless Backend Refactor Workstream

Status: In Progress  
Last updated: 2026-03-26

## Summary

This workstream defines the backend architecture we want to grow into without forcing a later large rewrite.

The key decision is:

- keep `dear-imgui-rs` focused on safe core abstractions
- keep `dear-imgui-sys` as the shared low-level foundation
- stop pretending official `imgui_impl_*` entry points are a portable C ABI
- replace the current `raw_backend` idea with a deliberate `backend_shim` layer

This is still an incremental refactor. We are not rewriting every backend crate at once. We are fixing the low-level contract now so later backend work has a correct foundation.

## Problem Statement

The workspace direction is broadly healthy, but the backend low-level boundary was still underspecified:

- `dear-imgui-sys` owns core Dear ImGui / cimgui compilation and FFI
- `dear-imgui-rs` owns safe core abstractions such as `Context`, `PlatformIo`, texture management, and render snapshots
- several backend crates are already separated by responsibility
- `dear-imgui-sdl3` remains a mixed convenience crate that owns SDL3-specific build logic plus a small wrapper layer

PR #23 was useful because it surfaced a real need: downstream crates and third-party ecosystems need a shared low-level way to reuse official Dear ImGui backend implementations.

However, the first shape was still wrong in one important way:

- official `imgui_impl_*` entry points are C++ symbols, not a stable C ABI
- plain Rust `extern "C"` declarations for those upstream names are therefore not the right public contract

That means the missing abstraction is not "raw backend declarations" in the narrow sense. The missing abstraction is a shared shim ABI.

## Design Goals

- provide a reusable low-level backend surface in `dear-imgui-sys`
- keep `dear-imgui-rs` backend-neutral unless it owns a safe API
- let first-party backend crates and third-party integrations share the same low-level contract
- let `dear-imgui-sys` optionally compile self-contained official backend shims
- keep SDL3- or framework-specific ownership in the relevant backend crate
- make Android support possible even when there is no dedicated first-party Android crate yet

## Non-Goals

- we are not trying to route backend-specific feature toggles through `dear-imgui-rs`
- we are not trying to turn every backend into a `dear-imgui-sys` feature
- we are not trying to move SDL3 integration ownership into the core crates
- we are not trying to replace Rust-native renderers such as `wgpu`, `glow`, or `ash`

## Core Invariants

1. `dear-imgui-sys` owns the core Dear ImGui / cimgui ABI and may expose optional backend shim entry points.
2. `dear-imgui-rs` owns safe abstractions, not backend-specific raw feature routing.
3. Official backend code must only cross into Rust through a deliberate C shim ABI, not via direct declarations of upstream C++ symbols.
4. Self-contained official backend shims may be compiled by `dear-imgui-sys`, but framework-specific integration ownership still belongs to backend crates.
5. Safe feature exposure belongs in `dear-imgui-rs` only when the safe crate actually provides the corresponding safe API.

## Target Architecture

### Layer Responsibilities

| Layer | Owns | Does not own |
|---|---|---|
| `dear-imgui-sys` | core Dear ImGui/cimgui build, core FFI, optional backend shim ABI, upstream backend source packaging | SDL3 ownership, safe backend API |
| `dear-imgui-rs` | safe core API, `PlatformIo`, texture lifecycle, render snapshots, typed viewport/platform hooks | backend-specific build logic, backend feature passthrough |
| backend crates | platform/render integration, framework-specific wrapper code, safe or semi-safe API, docs/examples/tests | redefining the shared shim ABI once it exists |
| third-party ecosystems | engine/framework-specific integration choices on top of `dear-imgui-sys` and `dear-imgui-rs` | core crate boundary decisions |

### Backend Shim Module

The preferred low-level shape in `dear-imgui-sys` is:

```rust
pub mod backend_shim {
    pub mod win32;
    pub mod dx11;
    pub mod android;
    pub mod opengl3;
}
```

Important meaning:

- this is low-level and unsafe
- this is our shim ABI, not upstream's original ABI
- the module name should make it obvious that a C shim boundary exists

### Feature Model

The low-level features should live in `dear-imgui-sys` with names such as:

- `backend-shim-win32`
- `backend-shim-dx11`
- `backend-shim-android`
- `backend-shim-opengl3`

Rules:

- these features do not get re-exported by `dear-imgui-rs`
- they expose our shim ABI, not direct upstream `imgui_impl_*` declarations
- self-contained backends may be compiled by `dear-imgui-sys` behind these features
- framework-specific backends such as SDL3 still keep their integration ownership in the backend crate

### Type Strategy

Default rule: prefer the narrowest ABI-stable surface that downstream crates can build on.

Prefer:

- `*mut c_void`
- `*const c_void`
- `*const c_char`
- primitive integer types
- small local raw aliases where they improve clarity

Avoid forcing downstream crates onto platform crates from the core layer unless that is clearly worth the lock-in.

### Build Ownership

There are two backend categories and they should be treated differently.

Self-contained official backends:

- `imgui_impl_opengl3`
- `imgui_impl_android`
- `imgui_impl_win32`
- `imgui_impl_dx11`

These can be compiled by `dear-imgui-sys` as optional shim libraries because they only depend on upstream Dear ImGui backend code plus platform SDK headers/libs.

Framework-specific backends:

- `imgui_impl_sdl3`
- future wrappers tied to external ecosystem crates

These remain owned by the relevant backend crate because the crate must still own framework-specific build logic, header discovery, and safe API design.

### Packaging and ABI Boundary

There are three separate concerns:

1. sharing upstream backend source files
2. sharing a Rust-visible low-level ABI
3. deciding which crate compiles a given backend implementation

This workstream chooses:

- `dear-imgui-sys` packages upstream `imgui/backends/**`
- `dear-imgui-sys` exports metadata such as `IMGUI_BACKENDS_PATH`
- `dear-imgui-sys` may also compile selected self-contained backend shims
- the Rust-visible ABI is our shim ABI, not upstream's original symbol names

That lets us keep a shared low-level foundation without lying about the upstream ABI.

## How First-Party and Third-Party Ecosystems Use This

### Self-Contained Official Backends

For `opengl3`, `android`, `win32`, and `dx11`:

- depend on `dear-imgui-sys`
- enable the matching `backend-shim-*` feature
- call the shim ABI from Rust

This gives downstream users a stable low-level bridge without forcing each crate to rebuild the same shim layer.

### Framework-Specific Backend Crates

For `dear-imgui-sdl3` and similar crates:

- continue to own framework-specific compilation and wrapper code
- reuse `dear-imgui-sys` packaged upstream backend sources where helpful
- optionally consume a shared self-contained shim from `dear-imgui-sys` when that reduces duplication

This is the pattern we want for SDL3:

- SDL3 platform backend stays owned by `dear-imgui-sdl3`
- official OpenGL3 renderer can be shared via `dear-imgui-sys::backend_shim::opengl3`

### Third-Party Ecosystems

Third-party users should be able to choose either route:

- use `dear-imgui-rs` + safe core APIs to build a custom backend manually
- use `dear-imgui-sys` backend shims where a self-contained official backend already exists

That is especially important for Android. Even without a dedicated first-party Android crate, downstream users should still be able to build Android support on top of:

- `dear-imgui-rs` input/frame/render/texture APIs
- `dear-imgui-sys::backend_shim::android`
- `dear-imgui-sys::backend_shim::opengl3`

## Migration Strategy for Existing Workspace Backends

### `dear-imgui-winit`

Status: keep as-is.

- pure Rust platform backend
- no immediate `backend_shim` dependency needed

### `dear-imgui-wgpu`

Status: keep as-is.

- pure Rust renderer backend
- no immediate `backend_shim` dependency needed

### `dear-imgui-glow`

Status: keep as-is.

- pure Rust OpenGL renderer backend
- remains the preferred Rust-native OpenGL path

### `dear-imgui-ash`

Status: keep as-is.

- pure Rust Vulkan renderer backend
- no immediate `backend_shim` dependency needed

### `dear-imgui-sdl3`

Status: transitional exception, but now with a cleaner target direction.

Migration direction:

- keep SDL3 platform ownership in this crate
- keep SDL3 header discovery and SDL-specific wrapper code here
- stop duplicating the official OpenGL3 wrapper when `dear-imgui-sys` already provides an `opengl3` shim
- add Android target dependency support for the `sdl3` crate
- document Android as "supported integration path, not zero-config turn-key path"

Important:

- this workstream still does not require an immediate large rewrite of `dear-imgui-sdl3`
- the first valuable cleanup is reducing duplicated low-level OpenGL3 wrapper code, not moving SDL3 ownership into `dear-imgui-sys`

## Android Direction

Android support should be designed around two truths:

1. users already can build a custom Android backend with `dear-imgui-rs` safe core APIs
2. upstream itself suggests SDL or GLFW on Android when full-featured integration is desired

Recommended direction:

- short term: make `dear-imgui-sys::backend_shim::android` and `backend_shim::opengl3` available
- short term: make `dear-imgui-sdl3` buildable on Android targets from the Cargo/dependency perspective
- short term: keep Android SDL3 acquisition owned by the consuming application; do not force `sdl3/build-from-source` from the backend crate itself
- short term: support both Android routes for `dear-imgui-sdl3` users:
  - app-provided SDL3 headers/integration via `SDL3_INCLUDE_DIR`
  - app-owned `sdl3/build-from-source` via Cargo feature unification
- medium term: decide whether a dedicated first-party Android crate is worth maintaining
- long term: prefer SDL3 as the convenience path on Android when that route is mature enough

## Multi-Viewport Boundary

Multi-viewport remains the main place where platform and renderer layers genuinely cooperate.

That is expected.

The rule is not "no coupling". The rule is:

- runtime/lifecycle coupling is acceptable
- the core crate boundary should stay minimal and explicit

Preferred ownership:

- platform backends own windows, native handles, input, focus, DPI, IME, and `PlatformUserData`
- renderer backends own swapchains, framebuffers, GPU state, textures, and `RendererUserData`
- coordination happens through `PlatformIo` callbacks plus narrow native-handle bridges

## Work Plan

### Milestone 0: Boundary Freeze

Goal: align the architecture before more code churn spreads.

Deliverables:

- this document reflects the `backend_shim` direction
- maintainer agreement that official backend C++ symbols are not exposed directly
- maintainer agreement that `dear-imgui-rs` does not re-export backend shim features

Exit criteria:

- documented decision to replace `raw_backend` terminology with `backend_shim`
- documented split between self-contained official backends and framework-specific backends

### Milestone 1: Introduce `backend_shim` in `dear-imgui-sys`

Goal: land the corrected low-level boundary with minimal blast radius.

Deliverables:

- `dear-imgui-sys::backend_shim`
- `backend-shim-*` feature naming
- no `dear-imgui-rs` passthrough
- no direct Rust declarations of upstream `imgui_impl_*` symbol names

Exit criteria:

- downstream users can depend on `dear-imgui-sys` and enable backend shim features directly
- the low-level ABI exposed to Rust is clearly our shim ABI

### Milestone 2: Compile Self-Contained Official Shims in `dear-imgui-sys`

Goal: make the shared shim layer actually reusable.

Deliverables:

- optional `opengl3` shim build in `dear-imgui-sys`
- optional `android` shim build in `dear-imgui-sys`
- optional `win32` and `dx11` shim builds in `dear-imgui-sys`
- shared cargo metadata for upstream backend and shim paths where useful

Exit criteria:

- self-contained official backends no longer require every downstream crate to rebuild the same wrapper layer
- backend shim features remain opt-in

### Milestone 3: Migrate Existing Consumers Incrementally

Goal: dogfood the new boundary without a big-bang backend rewrite.

Candidate order:

1. migrate `dear-imgui-sdl3` OpenGL3 path to `dear-imgui-sys::backend_shim::opengl3`
2. add Android target dependency support in `dear-imgui-sdl3`
3. decide whether `android` should stay low-level only for a while or grow a dedicated convenience crate later
4. evaluate `win32` / `dx11` first-party wrappers only if we are willing to maintain them

Exit criteria:

- at least one in-tree backend crate consumes the new shim layer
- Android has a documented low-level path plus a credible SDL3-based direction

### Milestone 4: Ecosystem Cleanup

Goal: reduce duplication and align terminology after the low-level boundary is proven.

Deliverables:

- updated docs language around backend shims vs safe backend crates
- reduced duplicated wrapper code where `dear-imgui-sys` already provides a shared shim
- clearer guidance for third-party backend authors

Exit criteria:

- the workspace no longer mixes "raw declarations", "upstream C++ symbols", and "shared shim ABI" as if they were the same thing

## TODO

### Immediate

- [x] Freeze the direction around `backend_shim` instead of `raw_backend`
- [x] Rename `dear-imgui-sys` low-level module to `backend_shim`
- [x] Rename feature gates to `backend-shim-*`
- [x] Update docs and README language to describe shim ABI instead of direct upstream backend declarations
- [x] Keep `dear-imgui-rs` free of backend shim feature passthrough

### Short Term

- [x] Implement `backend_shim::opengl3` in `dear-imgui-sys`
- [x] Implement `backend_shim::android` in `dear-imgui-sys`
- [x] Implement `backend_shim::win32` in `dear-imgui-sys`
- [x] Implement `backend_shim::dx11` in `dear-imgui-sys`
- [x] Export or document shim-path metadata where it improves downstream ergonomics
- [x] Migrate `dear-imgui-sdl3` OpenGL3 renderer path to the shared sys shim
- [x] Add Android target dependency support to `dear-imgui-sdl3`

### Medium Term

- [x] Document the exact ownership rule for self-contained backends vs framework-specific backends
- [x] Document Android support recipes for both custom backends and SDL3-based integrations
- [x] Add a standalone `examples-android/dear-imgui-android-smoke` template so the low-level Android route is documented without changing default workspace builds
- [x] Document that SDL3 Android build-from-source still requires app-owned ABI / CMake / Ninja configuration
- [x] Prove that the standalone Android smoke template can build a debug APK via `cargo-apk2` without changing the default workspace build matrix
- [x] Document and script release signing / per-ABI packaging for the standalone Android smoke template
- [ ] Decide whether a dedicated first-party Android convenience crate is worth maintaining
- [ ] Decide whether first-party `win32` / `dx11` convenience crates are worth maintaining

### Long Term

- [ ] Converge backend naming, docs, and examples around the final architecture
- [ ] Keep adding backends without reopening the core boundary debate

## Risks and Mitigations

### Risk: `backend_shim` still gets confused with upstream ABI

Mitigation:

- use repository-owned shim symbol names
- document that official backend C++ symbols are never the Rust-facing contract

### Risk: `dear-imgui-sys` grows into a dumping ground for every backend

Mitigation:

- only compile self-contained official backends here
- keep SDL3/framework-specific ownership in backend crates

### Risk: Android support remains vague

Mitigation:

- document the existing custom-backend path explicitly
- treat SDL3 Android as a supported integration direction, not a magic auto-setup promise

### Risk: the refactor stalls at the document stage

Mitigation:

- use `opengl3` as the first complete end-to-end slice
- dogfood the new shim layer immediately in `dear-imgui-sdl3`

## Success Criteria

This workstream is successful if:

- new backend proposals no longer reopen the "should we expose upstream C++ symbols directly?" debate
- `dear-imgui-sys` exposes a correct low-level backend shim surface
- `dear-imgui-rs` stays focused on safe core abstractions
- backend crates and third-party ecosystems can both build on the same low-level foundation
- Android remains possible for downstream users even before every convenience crate exists
