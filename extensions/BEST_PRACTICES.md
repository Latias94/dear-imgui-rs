# Extensions: Best Practices

This guide documents how to build and design extensions that integrate well with our `dear-imgui` workspace. It covers API style guidelines, layering, build scripts, and how to proceed when upstream only provides a C++ API.

## Goals

- Cross-platform, reproducible builds
- Safe, ergonomic Rust APIs
- Minimal FFI surface; keep C/C++ details in `-sys`

## API Style (align with dear-imgui)

Follow the `dear-imgui` crate's style for a uniform developer experience:

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
use dear_imgui::Ui;
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

Prefer this style for new features to remain consistent with the `dear-imgui` crate. See `dear_imguizmo::graph` (pure-Rust GraphEditor) for a concrete example of a Ui extension plus builder API.

Naming convention for Ui extensions and entry points:

- `GuizmoExt::guizmo` -> returns `GizmoUi`
- `ImPlotExt::implot` -> returns `PlotUi<'_>` and takes a `PlotContext`
- `ImNodesExt::imnodes` -> returns `NodesUi<'_>` and takes a `Context`

## Context Binding to Dear ImGui

Extensions that keep their own global/context state must explicitly bind to the current Dear ImGui context before creating or using that state. Do this at the moment you create/use the extension context or begin its frame:

```rust
// ImPlot context creation
unsafe { dear_implot_sys::ImPlot_SetImGuiContext(dear_imgui_sys::igGetCurrentContext()) }
let implot = dear_implot::PlotContext::create(&imgui_ctx);

// ImGuizmo per-frame setup
unsafe { dear_imguizmo_sys::ImGuizmo_SetImGuiContext(dear_imgui_sys::igGetCurrentContext()) }
dear_imguizmo::GuizmoContext::new().begin_frame(&ui);

// ImNodes context creation
unsafe { dear_imnodes_sys::imnodes_SetImGuiContext(dear_imgui_sys::igGetCurrentContext()) }
let imnodes = dear_imnodes::Context::create(&imgui_ctx);
```

Rationale: it prevents undefined behavior from querying ImGui state through a stale or null context inside the extension.

## Layering

```
your-extension/
  your-extension-sys/      # Low-level FFI (C binding + bindgen)
    build.rs             # cc + bindgen, inherits DEP_DEAR_IMGUI_* paths/defines
    src/lib.rs           # include!(concat!(OUT_DIR, "/bindings.rs"))
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
  - `DEP_DEAR_IMGUI_DEFINE_*` (propagate as `build.define(key, val)`)
- Sources: compile upstream C/C++ sources with `cc` (e.g., `cimplot.cpp`, `implot/*.cpp`)
- Base linking: do not duplicate linking of the base ImGui library; rely on `dear-imgui-sys` to emit the correct `cargo:rustc-link-lib` for `dear_imgui`/`cimgui`. Your `-sys` crate should only emit its own static lib.
- Blocklist ImGui types in bindgen (re-use dear-imgui-sys): ImVec2, ImVec4, ImGuiContext, ImDrawList, etc.
- WASM: generally exclude native build; emit bindings only if you need docs.rs builds
- Docs.rs/offline builds: commit a full `src/bindings_pregenerated.rs` and have `build.rs` copy/sanitize it to `OUT_DIR/bindings.rs` when headers or toolchains are not available

### Prebuilt Helper Crate

Centralize all prebuilt download/extract/naming logic via the shared helper crate:

- Depend on `dear-imgui-build-support` in your `-sys` crate as a build-dependency:
  ```toml
  [build-dependencies]
  build-support = { package = "dear-imgui-build-support", version = "0.1" }
  ```

- In `build.rs`, prefer the helpers instead of duplicating logic:
  ```rust
  let lib_name = build_support::expected_lib_name(&cfg.target_env, "dear_your_ext");
  let cache_root = build_support::prebuilt_cache_root_from_env_or_target(
      &cfg.manifest_dir, "YOUR_EXT_SYS_CACHE_DIR", "dear-your-ext-prebuilt",
  );
  if let Ok(dir) = build_support::download_prebuilt(&cache_root, &url, &lib_name, &cfg.target_env) {
      // link from prebuilt dir
  }
  ```

Benefits:
- No direct `reqwest` in each extension crate
- Unified archive naming & manifest format
- Simpler CI packaging

### Build Modes

Provide three modes (opt-in via env):

- Source build (default)
- System/prebuilt: `*_SYS_LIB_DIR` -> add `cargo:rustc-link-search` and `-l static=<name>`
- Remote prebuilt: `*_SYS_PREBUILT_URL` -> download to `OUT_DIR/prebuilt/`

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
- Optional: `bin/package` helper for producing release archives consistent with `tools/build-support`

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

## Example: Minimal `build.rs` Sketch

```rust
let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

// Resolve include paths from dear-imgui-sys
let imgui_src = PathBuf::from(env::var("DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH").unwrap());
let cimgui_root = PathBuf::from(env::var("DEP_DEAR_IMGUI_CIMGUI_INCLUDE_PATH").unwrap());
let third_party = manifest_dir.join("third-party/           # upstream C API (git submodule)our-upstream");

// Generate bindings
let bindings = bindgen::Builder::default()
    .header(third_party.join("your_c_api.h").to_string_lossy())
    .clang_arg(format!("-I{}", imgui_src.display()))
    .clang_arg(format!("-I{}", cimgui_root.display()))
    .clang_arg(format!("-I{}", third_party.display()))
    .blocklist_type("ImVec2").blocklist_type("ImVec4").blocklist_type("ImGuiContext")
    .derive_default(true).derive_debug(true).derive_copy(true)
    .layout_tests(false)
    .generate().unwrap();
bindings.write_to_file(out_path.join("bindings.rs")).unwrap();

// Compile sources
let mut build = cc::Build::new();
build.cpp(true).std("c++17");
for (k, v) in env::vars() { if let Some(s) = k.strip_prefix("DEP_DEAR_IMGUI_DEFINE_") { build.define(s, v.as_str()); } }
build.include(&imgui_src).include(&cimgui_root).include(&third_party);
build.file(third_party.join("your_c_api.cpp"));
build.compile("dear_your_ext");

// Try prebuilt if requested (optional)
// let cache_root = build_support::prebuilt_cache_root_from_env_or_target(&manifest_dir, "YOUR_EXT_SYS_CACHE_DIR", "dear-your-ext-prebuilt");
// let _ = build_support::download_prebuilt(&cache_root, url, build_support::expected_lib_name(&target_env, "dear_your_ext"), &target_env);
```

### MSVC (Windows) parity

When targeting MSVC, match Rust's CRT selection and exception model to avoid ABI/debug iterator mismatches:

```rust
let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
if target_env == "msvc" && target_os == "windows" {
    build.flag("/EHsc");
    let target_features = env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default();
    let use_static_crt = target_features.split(',').any(|f| f == "crt-static");
    build.static_crt(use_static_crt);
    if use_static_crt { build.flag("/MT"); } else { build.flag("/MD"); }
    let profile = env::var("PROFILE").unwrap_or_else(|_| "release".to_string());
    if profile == "debug" { build.debug(true); build.opt_level(0); } else { build.debug(false); build.opt_level(2); }
    build.flag("/D_ITERATOR_DEBUG_LEVEL=0");
}
```

## Checklist

- [ ] `-sys` builds with source, system/prebuilt, and remote prebuilt
- [ ] Inherit `DEP_DEAR_IMGUI_*` include paths and defines
- [ ] Blocklist overlapping ImGui types in bindgen
- [ ] High-level uses bitflags for masks, enums for discrete choices
- [ ] Conversions for common math types (mint, optional glam)
- [ ] Examples are feature-gated in the root `examples/` crate
- [ ] Context binding calls to ImGui are in place (`*_SetImGuiContext`)
- [ ] Basic thread-safety assertions for context types (e.g., `assert_not_impl_any!(Context: Send, Sync)`) via `static_assertions`

## When C API Is Missing but C++ Exists

Sometimes upstream provides functionality only in the C++ API (e.g., ImGuizmo GraphEditor). We have two approaches:

1) Prefer a pure Rust reimplementation (recommended)

- Design a Rust-idiomatic API aligned with `dear-imgui` (Ui extensions, RAII tokens, builders). Our `dear_imguizmo::graph` module follows this approach.
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
  - MSVC: match CRT (`/MT` vs `/MD`) and enable `/EHsc`; set `_ITERATOR_DEBUG_LEVEL=0` for debug parity (see MSVC parity section).
  - WASM: avoid C++ unless toolchain is guaranteed; prefer Rust path.

Decision guideline:

- If feature is UI-composable and not too complex -> choose (1) Rust.
- If behavior is intricate and parity is required quickly -> choose (2) C wrapper, with a clear minimal C API and tests.

Document in your crate README which approach is used and why, and provide a migration path if switching strategies in future.





