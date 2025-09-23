# Extensions: Best Practices

This guide documents how to build and design extensions that integrate well with our `dear-imgui` workspace.

## Goals

- Cross‑platform, reproducible builds
- Safe, ergonomic Rust APIs
- Minimal FFI surface; keep C/C++ details in `-sys`

## Layering

```text
your-extension/
├─ your-extension-sys/      # Low-level FFI (C binding + bindgen)
│  ├─ build.rs              # cc + bindgen, inherits DEP_DEAR_IMGUI_* paths/defines
│  ├─ src/lib.rs            # include!(concat!(OUT_DIR, "/bindings.rs"))
│  └─ third-party/…         # upstream C API (git submodule)
└─ your-extension/          # High-level safe API
   └─ src/lib.rs            # RAII tokens, builders, bitflags
```

## Build Scripts (`-sys`)

Use C bindings (cimgui family) + bindgen:

- Headers: point bindgen at the upstream C header (e.g., `cimplot.h`, `cimguizmo.h`)
- Includes: inherit from `dear-imgui-sys` via Cargo env
  - `DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH`
  - `DEP_DEAR_IMGUI_CIMGUI_INCLUDE_PATH`
  - `DEP_DEAR_IMGUI_DEFINE_*` (propagate as `build.define(key, val)`) 
- Sources: compile upstream C/C++ sources with `cc` (e.g., `cimplot.cpp`, `implot/*.cpp`)
- Base linking: do not duplicate linking of the base ImGui library; rely on `dear-imgui-sys` to emit the correct `cargo:rustc-link-lib` for `dear_imgui`/`cimgui`. Your `-sys` crate should only emit its own static lib.
- Blocklist ImGui types in bindgen (re-use `dear-imgui-sys`): `ImVec2`, `ImVec4`, `ImGuiContext`, `ImDrawList`, …
- WASM: generally exclude native build; emit bindings only if you need docs.rs builds

### Build Modes

Provide three modes (opt‑in via env):

- Source build (default)
- System/prebuilt: `*_SYS_LIB_DIR` → add `cargo:rustc-link-search` and `-l static=<name>`
- Remote prebuilt: `*_SYS_PREBUILT_URL` → download to `OUT_DIR/prebuilt/`

Naming convention for static libs:

- Windows/MSVC: `<name>.lib` (e.g., `dear_implot.lib`)
- Unix: `lib<name>.a` (e.g., `libdear_implot.a`)

## High‑Level API Design

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

## Example: Minimal `build.rs` Sketch

```rust
let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

// Resolve include paths from dear-imgui-sys
let imgui_src = PathBuf::from(env::var("DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH").unwrap());
let cimgui_root = PathBuf::from(env::var("DEP_DEAR_IMGUI_CIMGUI_INCLUDE_PATH").unwrap());
let third_party = manifest_dir.join("third-party/your-upstream");

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
- [ ] Examples are feature‑gated in the root `examples/` crate
