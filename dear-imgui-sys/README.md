# dear-imgui-sys

Low-level Rust bindings for Dear ImGui via cimgui (C API) + bindgen.

## Overview

This crate provides unsafe Rust bindings to the Dear ImGui docking branch using the cimgui C API. Bindings are generated with bindgen from vendored headers, avoiding C++ ABI pitfalls and making cross-platform builds simpler.

## Key Features

- **cimgui-based bindings**: Generate from C API headers (no C++ ABI/MSVC quirks)
- **Docking Support**: Built against the docking branch
- **Windows-friendly**: Native builds prefer CMake (auto-detects VS/SDK)
- **Prebuilt Support**: Link a prebuilt static library instead of building locally
- **Docs.rs Offline**: Use pregenerated or offline-generated bindings

## Build & Link Options

You can choose one of the following strategies:

1) Prebuilt static library（recommended）
- Set `IMGUI_SYS_LIB_DIR=...` to the folder containing the static lib
  - Windows: `dear_imgui.lib`
  - Linux/macOS: `libdear_imgui.a`
- Or set `IMGUI_SYS_PREBUILT_URL=...` to a direct URL of the static lib
- Releases contain platform archives (include + static lib)

2) Native build from source
- Windows prefers CMake automatically; set `IMGUI_SYS_USE_CMAKE=1` to force CMake elsewhere
- Otherwise falls back to cc crate

Build examples:
- Windows (CMake auto):
  - Requirements: Visual Studio (C++ build tools), CMake
  - `cargo build -p dear-imgui-sys`
- Linux:
  - Requirements: build-essential, pkg-config, LLVM/Clang (for bindgen)
  - `sudo apt-get install -y build-essential pkg-config llvm`
  - `cargo build -p dear-imgui-sys`
- macOS:
  - Requirements: Xcode Command Line Tools
  - `xcode-select --install` (if needed)
  - `cargo build -p dear-imgui-sys`

3) Fast Rust-only iteration
- Set `IMGUI_SYS_SKIP_CC=1` to skip native C/C++ compilation while iterating on Rust code

## Docs.rs / Offline

When `DOCS_RS=1` is detected, the build script:
- Tries to use `src/bindings_pregenerated.rs` (if present)
- Else runs bindgen against vendored headers (offline, no network), and writes to `OUT_DIR/bindings.rs`
- Skips native linking

To refresh the pregenerated bindings locally:
```
IMGUI_SYS_SKIP_CC=1 cargo build -p dear-imgui-sys
cp target/debug/build/dear-imgui-sys-*/out/bindings.rs dear-imgui-sys/src/bindings_pregenerated.rs
```
或使用工具脚本：
```
python tools/update_cimgui_and_bindings.py --branch docking_inter
```

## WebAssembly Support

This crate provides comprehensive WebAssembly (WASM) support through the `wasm` feature flag. The implementation automatically handles the complexities of cross-compilation and provides a seamless experience for WASM development.

### WASM Notes

- Skips native C/C++ compilation for wasm targets
- Uses offline-generated bindings for type-checking

### Building for WASM

1. **Install WASM target**:
```bash
rustup target add wasm32-unknown-unknown
```

2. **Build for WASM**:
```bash
# Basic WASM build
cargo build --target wasm32-unknown-unknown --features wasm

# With additional features
cargo build --target wasm32-unknown-unknown --features "wasm,docking"

# Check compilation (faster)
cargo check --target wasm32-unknown-unknown --features wasm

# Build WASM example
cargo check --target wasm32-unknown-unknown --features wasm --example wasm_test
```

3. **Use the build script** (optional):
```bash
# Use the provided build script for automation
./build-wasm.sh
```

### WASM Feature Flags

```toml
[dependencies]
dear-imgui-sys = { version = "0.1.0", features = ["wasm"] }

# Or with additional features
dear-imgui-sys = { version = "0.1.0", features = ["wasm", "docking"] }
```

### Integration with wasm-bindgen

The generated WASM binaries are compatible with wasm-bindgen:

```bash
# Generate JavaScript bindings
wasm-bindgen --out-dir wasm --web target/wasm32-unknown-unknown/debug/your_app.wasm
```

### WASM Usage Example
`wasm` 目标依赖于你的渲染集成（WebGL/Canvas 等），本 crate 仅提供类型层面的可用性。

### Rendering in WASM

Since WASM doesn't have direct access to graphics APIs, you'll need to:

1. **Canvas API**: Render ImGui draw data to HTML5 Canvas through JavaScript
2. **WebGL Backend**: Implement a WebGL-based renderer for ImGui
3. **Existing Solutions**: Use existing WASM ImGui renderers or JavaScript bindings

### WASM-Specific Considerations

1. **No File System**: File operations are disabled by default in WASM builds
2. **No Threading**: Uses global context instead of thread-local storage
3. **Memory Management**: Ensure proper cleanup of ImGui contexts in WASM environment
4. **Performance**: WASM builds may have different performance characteristics
5. **Consistent API**: Uses the same `ImGui_*` naming convention for both native and WASM targets

## Usage

This is a low-level sys crate. Most users should use the higher-level `dear-imgui` crate instead, which provides safe Rust wrappers around these bindings.

```toml
[dependencies]
dear-imgui-sys = "0.1.0"

# For WASM targets
dear-imgui-sys = { version = "0.1.0", features = ["wasm"] }
```

## Potential Improvements

While our current solution works well, there are several areas where we could enhance the approach:

### 1. **Automated Detection**

```rust
// Future: Automatically detect problematic functions
fn needs_abi_fix(function: &Function) -> bool {
    function.returns_small_cpp_class() &&
    function.has_non_trivial_members() &&
    target_env == "msvc"
}
```

### 2. **Better Error Messages**

```rust
// Future: Provide clear guidance when ABI issues are detected
#[cfg(target_env = "msvc")]
compile_error!(
    "Function {} requires ABI fix. Add to msvc_blocklist.txt and create wrapper.",
    function_name
);
```

### 3. **Cross-Platform ABI Fixes**

Currently we only handle MSVC, but Linux and other platforms have similar issues. A comprehensive solution would:

- Detect non-trivial C++ types on all platforms
- Generate appropriate wrapper functions automatically
- Provide consistent behavior across all targets

### 4. **Upstream Contributions**

The ideal long-term solution would be improvements to `bindgen` itself:

- Better C++ ABI detection
- Automatic wrapper generation for problematic functions
- Platform-specific ABI handling

### 5. **Alternative Approaches**

#### Option A: Full Opaque Types

```rust
// Make all ImVec2-returning functions opaque
.opaque_type("ImVec2")
.blocklist_function(".*GetContentRegionAvail.*")
```

#### Option B: Custom ABI Annotations

```cpp
// Hypothetical: Explicit ABI annotations
extern "C" __attribute__((sysv_abi)) ImVec2_pod GetContentRegionAvail_pod();
```

#### Option C: Rust-Native Implementations

```rust
// Reimplement problematic functions in pure Rust
pub fn get_content_region_avail() -> [f32; 2] {
    // Direct implementation using ImGui internals
}
```

## Comparison with Other Solutions

| Approach | Pros | Cons | Maintenance |
|----------|------|------|-------------|
| **Our Solution** | ✅ Precise, Type-safe | ⚠️ Manual setup | 🟡 Medium |
| **Full Opaque** | ✅ Simple, Universal | ❌ Loses type info | 🟢 Low |
| **Phantom Data** | ✅ Forces stack return | ❌ Affects all types | 🟡 Medium |
| **Pure Rust** | ✅ No ABI issues | ❌ Reimplementation work | 🔴 High |

## Specific Improvements We Could Make

### 1. **Automated Wrapper Generation**

Instead of manually maintaining `hack_msvc.cpp`, we could generate it automatically:

```rust
// build.rs enhancement
fn generate_msvc_wrappers(functions: &[&str]) -> String {
    let mut code = String::new();
    for func in functions {
        if returns_imvec2(func) {
            code.push_str(&format!(
                "ImVec2_rr ImGui_{}() {{ return _rr(ImGui::{}()); }}\n",
                func, func
            ));
        }
    }
    code
}
```

### 2. **Better Function Detection**

We could automatically detect which functions need fixes by parsing Dear ImGui headers:

```rust
// Automatically find ImVec2-returning functions
fn find_problematic_functions() -> Vec<String> {
    // Parse imgui.h and find all functions returning ImVec2
    // This would eliminate the need for manual blocklist maintenance
}
```

### 3. **Runtime ABI Validation**

Add runtime checks to ensure our fixes work correctly:

```rust
#[cfg(all(test, target_env = "msvc"))]
mod abi_tests {
    #[test]
    fn test_content_region_avail_abi() {
        // Verify that our wrapper returns the same values as direct calls
        // This would catch ABI regressions
    }
}
```

### 4. **Cross-Platform Extension**

Extend the solution to handle Linux ABI issues:

```cpp
// Linux-specific wrappers for non-trivial types
#ifdef __linux__
extern "C" {
    void ImGui_GetContentRegionAvail_linux(ImVec2* out) {
        *out = ImGui::GetContentRegionAvail();
    }
}
#endif
```

## Summary

Our implementation represents a **best-practice solution** for handling C++ ABI compatibility issues in Rust FFI bindings:

### ✅ **What We Do Well**

- **Surgical Precision**: Only fixes problematic functions, leaving the rest of the API untouched
- **Type Safety**: Maintains Rust's type system guarantees through proper conversions
- **Platform Awareness**: Conditional compilation ensures fixes only apply where needed
- **Proven Approach**: Based on the successful easy-imgui-rs implementation
- **Clear Documentation**: Comprehensive explanation of the problem and solution

### 🚀 **Future Enhancements**

- **Automated Detection**: Generate wrapper functions automatically from header analysis
- **Cross-Platform Support**: Extend fixes to Linux and other platforms with similar issues
- **Runtime Validation**: Add tests to ensure ABI compatibility across different environments
- **Upstream Integration**: Contribute improvements back to the bindgen project

### 🎯 **Why This Matters**

C++ ABI compatibility is a fundamental challenge when creating Rust bindings for C++ libraries. Our solution provides:

- **Reliability**: Eliminates crashes and memory corruption
- **Maintainability**: Clear structure that's easy to understand and extend
- **Performance**: No runtime overhead, compile-time solution
- **Compatibility**: Works across different MSVC versions and configurations

This approach can serve as a template for other Rust projects facing similar C++ FFI challenges.

## License

This project follows the same license as Dear ImGui itself. See the Dear ImGui repository for license details.
