# Extensions: Best Practices

This guide documents how to build and design extensions that integrate well with our `dear-imgui-rs` workspace. It covers API style guidelines, layering, build scripts, and how to proceed when upstream only provides a C++ API.

## Goals

- Cross-platform, reproducible builds
- Safe, ergonomic Rust APIs
- Minimal FFI surface; keep C/C++ details in `-sys`

## API Style (align with dear-imgui)

Follow the `dear-imgui-rs` crate's style for a uniform developer experience:

- Entry via `Ui` extensions
  - Provide `Ui` extension methods to access your extension's per-frame UI (e.g., `ui.guizmo()` returning a `GizmoUi`).
- RAII tokens
  - For push/pop stacks and begin/end scopes, return tokens that pop/end on `Drop`. Offer an explicit `.pop()`/`.end()` as convenience.
- Builder pattern
  - Complex widgets/operations use a builder with `.build()` (e.g., `manipulate_config(...).operation(...).mode(...).build()`).
- Strongly-typed flags and enums
  - Use `bitflags` for mask types; use `#[repr(..)]` enums for discrete choices mapped to FFI types.
- Lifetimes and context
  - Bind accessors to `&Ui` lifetime when exposing global state (styles, draw lists) to avoid leaking `'static` references.
- Naming
  - Mirror upstream names at the `-sys` layer; prefer idiomatic Rust naming for the safe layer.

Example (ImGuizmo):

```rust
use dear_imgui_rs::Ui;
use dear_imguizmo::{GuizmoExt, Operation, Mode, DrawListTarget};

fn draw(ui: &Ui) {
    let giz = ui.guizmo();
    giz.set_drawlist(DrawListTarget::Window);
    giz.set_rect(0.0, 0.0, 800.0, 600.0);

    let view = [1.0; 16];
    let proj = [1.0; 16];
    let mut model = [1.0; 16];

    let _id = giz.push_id("cube-0");
    let used = giz
        .manipulate_config(&view, &proj, &mut model)
        .operation(Operation::TRANSLATE | Operation::ROTATE)
        .mode(Mode::World)
        .translate_snap([1.0, 1.0, 1.0])
        .build();
    if used { /* changed this frame */ }
}
```

Prefer this style for new features to remain consistent with the `dear-imgui-rs` crate. See `dear_imguizmo::graph` (pure-Rust GraphEditor) for a concrete example of a Ui extension plus builder API.

Naming convention for Ui extensions and entry points:

- `GuizmoExt::guizmo` -> returns `GizmoUi`
- `ImPlotExt::implot` -> returns `PlotUi<'_>` and takes a `PlotContext`
- `ImNodesExt::imnodes` -> returns `NodesUi<'_>` and takes a `Context`

## Context Binding to Dear ImGui

Extensions that keep their own global/context state must explicitly bind to the current Dear ImGui context before creating or using that state.

Prefer doing this inside your safe wrapper entry points (e.g. `Ui` extension methods and context constructors) so callers don't need to touch `-sys` APIs.

```rust
// Safe wrapper usage (recommended)
let implot = dear_implot::PlotContext::create(&imgui_ctx);
let giz = ui.guizmo(); // binds and calls BeginFrame
let imnodes = dear_imnodes::Context::create(&imgui_ctx);

// Low-level / -sys glue (only when building the wrapper)
// ImPlot context creation
unsafe { dear_implot_sys::ImPlot_SetImGuiContext(dear_imgui_sys::igGetCurrentContext()) }
let implot = dear_implot::PlotContext::create(&imgui_ctx);

// ImGuizmo per-frame setup
unsafe { dear_imguizmo_sys::ImGuizmo_SetImGuiContext(dear_imgui_sys::igGetCurrentContext()) }
dear_imguizmo::GuizmoContext::new().begin_frame(ui);

// ImNodes context creation
unsafe { dear_imnodes_sys::imnodes_SetImGuiContext(dear_imgui_sys::igGetCurrentContext()) }
let imnodes = dear_imnodes::Context::create(&imgui_ctx);
```

Rationale: it prevents querying ImGui state through a stale or null context. Record the owner's `ContextBinding`, enter it for native calls and destruction, and reject a `Ui` with a different `ContextId`. Use `ContextAttachment` when hooks must detach before Context destruction. Applications should drop extension owners before their Context; after Context death, avoid native destruction rather than dereferencing stale state. Dear ImGui's process-current Context and extensions' global state require serial use across contexts.

## Layering

```
  your-extension/
  your-extension-sys/      # Low-level FFI (C binding + bindgen)
    build.rs             # cc + bindgen, consumes the canonical binding spec
    src/lib.rs           # expose bindings module (prefer committed pregenerated bindings; if using OUT_DIR, keep include! in a dedicated module)
    third-party/           # upstream C API (git submodule)
  your-extension/          # High-level safe API
    src/lib.rs           # RAII tokens, builders, bitflags
```

## Build Scripts (`-sys`)

Use C bindings (cimgui family) + bindgen when available:

- Headers: point bindgen at the upstream C header (e.g., `cimplot.h`, `cimguizmo.h`)
- Includes: inherit from `dear-imgui-sys` via Cargo env
  - `DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH`
  - `DEP_DEAR_IMGUI_CIMGUI_INCLUDE_PATH`
  - `DEP_DEAR_IMGUI_DEFINE_*` metadata is consumed only through the canonical binding-spec helper;
    do not forward environment keys directly to `cc::Build`
- Sources: compile upstream C/C++ sources with `cc` (e.g., `cimplot.cpp`, `implot/*.cpp`)
- Base linking: do not duplicate linking of the base ImGui library; rely on `dear-imgui-sys` to emit the correct `cargo:rustc-link-lib` for `dear_imgui`/`cimgui`. Your `-sys` crate should only emit its own static lib.
- Blocklist ImGui types in bindgen (re-use dear-imgui-sys): ImVec2, ImVec4, ImGuiContext, ImDrawList, etc.
- WASM: supported extensions use checked-in import bindings for the fixed `imgui-sys-v1` provider on `wasm32-unknown-unknown`. Cargo skips C++ compilation; build the matching provider separately with Emscripten. Enable `wasm` on every highest safe dependency path. A new extension needs provider exports, source wiring, and WASM bindings before it can claim support.
- Checked-in bindings: use `CrateBindingSpec::copy_embedded_checked_in_to_out_dir` to validate provenance before copying to `OUT_DIR`. Regeneration is explicit (`DEAR_IMGUI_RS_REGEN_BINDINGS=1` plus `bindgen`) and requires headers and libclang.
- docs.rs skips native compilation. Ordinary offline source builds still require the maintained C++ sources and a compiler; pregenerated bindings do not replace the native library.

### Prebuilt Helper Crate

Centralize all prebuilt download/extract/naming logic via the shared helper crate:

- Depend on `dear-imgui-build-support` in your `-sys` crate as a build-dependency.
  Use the compatible stable requirement:
  ```toml
  [build-dependencies]
  build-support = { package = "dear-imgui-build-support", version = "0.20", features = ["binding-spec"] }
  ```

  Workspace members should inherit the repository catalog instead:

  ```toml
  [build-dependencies]
  build-support.workspace = true
  ```

- Use `CrateBindingSpec` and the maintained source inventory for bindings,
  defines, and source selection. Register a new extension in those catalogs
  before using the helpers.
- Use `ExtensionArtifactProfile` to validate a prebuilt directory before linking.
  Matching packages include a manifest identifying target, CRT, source pins,
  binding identity, feature profile, and the shared Dear ImGui core.
- Use `expected_lib_name`, `prebuilt_cache_root_from_env_or_target`, and
  `download_prebuilt` for naming and transport. Download/extraction does not
  validate ABI compatibility; the artifact profile performs that check.

Benefits:
- No direct HTTP client dependency in each extension crate
- Unified archive naming & manifest format
- Simpler CI packaging

### Build Modes

For extensions that support prebuilt artifacts, provide these routes:

- Source build (default)
- Local prebuilt: `*_SYS_LIB_DIR` -> validate the matching library and manifest, then emit search/link directives
- Remote/archive prebuilt: `*_SYS_PREBUILT_URL` -> download/extract with `prebuilt`, validate, then link

Document route precedence. `build-from-source` and `*_SYS_FORCE_BUILD` force source builds; a local directory override precedes an explicit archive URL, which precedes automatic release lookup. Choose one override at a time. Test Engine is source-only and does not implement the prebuilt routes.

Tip: allow automated downloads from GitHub Releases by generating candidate URLs via
`build_support::release_candidate_urls_env()` and trying them in order.

Naming convention for static libs:

- Windows/MSVC: `<name>.lib` (e.g., `dear_implot.lib`)
- Unix: `lib<name>.a` (e.g., `libdear_implot.a`)

## Crate and Env Var Conventions

- Crate names: `dear-<ext>-sys` (FFI) and `dear-<ext>` (safe)
- Cargo `links`: `dear_<ext>`; native static library name follows the same
- Env vars: `<EXT>_SYS_LIB_DIR`, `<EXT>_SYS_PREBUILT_URL`, `<EXT>_SYS_SKIP_CC`, optional `<EXT>_SYS_USE_PREBUILT`, `<EXT>_SYS_FORCE_BUILD`
- Features: `prebuilt`, `build-from-source` (opt-in toggles) and passthroughs like `freetype` to `dear-imgui-sys`
- Optional: `package-bin` gates the internal `bin/package` helper for producing release archives consistent with `tools/build-support`; it is a maintainer tool, not an application dependency feature.

## High-Level API Design

- RAII lifetimes: return tokens to guarantee paired begin/end calls
- Builders: prefer fluent configuration for complex options
- Bitflags vs enums:
  - Use `bitflags` for C flag masks (e.g., plotting flags, gizmo operations)
  - Use `enum` for discrete choices (e.g., coordinate mode, placement)
- Error handling: return `Result<_, Error>` when user input can be invalid (length mismatch, unsupported combination)

## Data Interop

- Core math types: re-use `dear-imgui-sys` (`ImVec2`, `ImVec4`) at FFI boundaries
- Conversions:
  - Provide `From<mint::Vector2<f32>> for ImVec2` / `From<mint::Vector4<f32>> for ImVec4` (already in `dear-imgui-sys`)
  - Optionally support popular math crates in the high-level layer (e.g., `glam::Mat4`) without leaking them into FFI

## FFI Casting: Prefer sys typedefs over raw i32/u32

When calling into your `-sys` crate, always cast enums/flags/axes to the exact typedefs emitted by bindgen (from your `sys` crate), not to raw `i32`/`u32`. This avoids cross‑platform mismatches when cbindgen/bindgen chooses signed vs unsigned types or different widths.

Examples (ImPlot):

```rust
// Do this:
unsafe {
    // Axis is sys::ImAxis (c_int); flags are sys::ImPlotAxisFlags (c_int)
    sys::ImPlot_SetupAxis(axis as sys::ImAxis, label_ptr, flags.bits() as sys::ImPlotAxisFlags);

    // Conditions are sys::ImPlotCond
    sys::ImPlot_SetupAxisLimits(axis as sys::ImAxis, min, max, cond as sys::ImPlotCond);

    // Drag tool flags are sys::ImPlotDragToolFlags
    sys::ImPlot_DragLineX(id, x, color4, thickness, flags.bits() as sys::ImPlotDragToolFlags, &mut clicked, &mut hovered, &mut held);
}

// Avoid this:
// sys::ImPlot_SetupAxis(axis as i32, label_ptr, flags.bits() as i32)
// sys::ImPlot_SetupAxisLimits(axis as i32, min, max, cond as i32)
```

Examples (Dear ImGui):

```rust
// Mouse cursor: use sys::ImGuiMouseCursor
unsafe {
    let c: sys::ImGuiMouseCursor = cursor.map(|m| m as sys::ImGuiMouseCursor).unwrap_or(sys::ImGuiMouseCursor_None);
    sys::igSetMouseCursor(c);
}

// Item key owner: use sys::ImGuiKey
unsafe {
    let k: sys::ImGuiKey = key as sys::ImGuiKey;
    sys::igSetItemKeyOwner_Nil(k);
}
```

Guideline:

- For each FFI function, consult your `-sys` crate’s `bindings_pregenerated.rs` (or `OUT_DIR/bindings.rs`) to find the expected typedefs.
- Cast your high‑level flags/enums via `.bits()` or `as` to that typedef (e.g., `as sys::ImPlotAxisFlags`) instead of raw `i32`/`u32`.
- For counts (lengths), casting to `i32` is acceptable when the binding expects `c_int`; prefer using the exact alias if it exists.

This keeps your extension portable across compilers/targets where C typedefs differ.

## Build Script References

Start from a maintained build script rather than an incomplete download or
bindgen sketch:

- [`dear-implot-sys/build.rs`](dear-implot-sys/build.rs): shared-core source builds,
  checked-in bindings, canonical defines, and verified prebuilt artifacts.
- [`dear-imgui-cte-sys/build.rs`](dear-imgui-cte-sys/build.rs): C++20 and a
  repository-owned no-unwind bridge.
- [`dear-imgui-test-engine-sys/build.rs`](dear-imgui-test-engine-sys/build.rs):
  source-only integration with exception translation at exported boundaries.

On MSVC, match Rust's CRT selection using the workspace helpers and preserve the
core's iterator-debug ABI. Exceptions must be caught before crossing the C ABI,
or disabled for a bridge that intentionally terminates on allocation failure.
`/EHsc` alone does not make an exported C entry point safe to unwind through.

## Checklist

- [ ] `-sys` validates every supported source/prebuilt route and explicitly rejects unsupported targets
- [ ] Inherit `DEP_DEAR_IMGUI_*` include paths and consume defines through `CrateBindingSpec`
- [ ] Blocklist overlapping ImGui types in bindgen
- [ ] High-level uses bitflags for masks, enums for discrete choices
- [ ] Conversions for common math types (mint, optional glam)
- [ ] Examples are feature-gated in the root `examples/` crate
- [ ] Context binding calls to ImGui are in place (`*_SetImGuiContext`)
- [ ] Basic thread-safety assertions for context types (e.g., `assert_not_impl_any!(Context: Send, Sync)`) via `static_assertions`

## When C API Is Missing but C++ Exists

Sometimes upstream provides functionality only in the C++ API (e.g., ImGuizmo GraphEditor). We have two approaches:

1) Prefer a pure Rust reimplementation (recommended)

- Design a Rust-idiomatic API aligned with `dear-imgui-rs` (Ui extensions, RAII tokens, builders). Our `dear_imguizmo::graph` module follows this approach.
- Use Dear ImGui drawing/input to replicate behavior.
- Pros: no extra toolchains or ABI issues; consistent style; easier maintenance and testing; better WASM/Android support.
- Cons: initial implementation effort; behavior may diverge slightly from upstream until parity is reached.

2) Add a thin C wrapper over the C++ code in your `-sys` crate

- Write a minimal `extern "C"` surface (opaque handles, create/destroy, methods)
  - Example pattern:
  - `struct GraphEditor;` exposed as `typedef struct GraphEditor GraphEditor;`
  - `GraphEditor* ge_create(); void ge_destroy(GraphEditor*); void ge_draw(GraphEditor*, ...);`
- Build with `cc::Build::new().cpp(true)` and pin exceptions/RTTI model; disable exceptions in wrapper where possible.
- Do NOT expose C++ types/headers to bindgen directly; only expose the C wrapper header.
- Ownership & safety rules:
  - Opaque pointers owned by Rust via `NonNull<T>` newtype; implement `Drop` to call `destroy`.
  - Avoid callbacks across FFI unless necessary; prefer pull APIs.
- Platform notes:
  - MSVC: match the core CRT and iterator-debug ABI, and translate exceptions or disable them before exposing the C ABI (see Build Script References).
  - WASM: avoid C++ unless toolchain is guaranteed; prefer Rust path.

Decision guideline:

- If feature is UI-composable and not too complex -> choose (1) Rust.
- If behavior is intricate and parity is required quickly -> choose (2) C wrapper, with a clear minimal C API and tests.

Document in your crate README which approach is used and why, and provide a migration path if switching strategies in future.



