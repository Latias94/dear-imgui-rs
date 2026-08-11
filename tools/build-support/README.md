# dear-imgui-build-support

Build helper utilities shared by `dear-imgui-sys` and its extensions.

This crate is intended for use in build scripts (`build.rs`) and internal packaging tools.
It centralizes common logic around naming prebuilt archives, generating manifests, and
downloading/extracting prebuilt static libraries.

## Features

- Compose archive names in a consistent scheme:
  `<crate>-prebuilt-<version>-<target>-<link>[<extra>][-<crt>].tar.gz`
- Compose a `manifest.txt` describing the prebuilt contents (version, target, link type, CRT, features)
- Download `.tar.gz` archives (HTTP(S), optional feature `download`) and extract to a cache
- Utility helpers to build candidate GitHub release URLs
- Optional native dependency discovery helpers behind `pkg-config` / `vcpkg`
  features. These are used by build scripts for FreeType and SDL3 header
  discovery, with a shared include-search helper for explicit include env vars,
  Cargo dependency metadata, pkg-config, vcpkg, and known system include roots.

## API Sketch

```rust
use build_support::{
    compose_archive_name,
    compose_manifest_bytes,
    expected_lib_name,
    release_tags,
    release_candidate_urls,
    download_prebuilt,
    extract_archive_to_cache,
    prebuilt_cache_root_from_env_or_target,
};

let name = compose_archive_name(
    "dear-imgui", env!("CARGO_PKG_VERSION"), target_triple, "static", None, crt_suffix,
);

let manifest = compose_manifest_bytes(
    "dear-imgui", env!("CARGO_PKG_VERSION"), target_triple, "static", crt_suffix, Some("freetype"),
);

let cache_root = prebuilt_cache_root_from_env_or_target(&manifest_dir, "IMGUI_SYS_CACHE_DIR", "dear-imgui-prebuilt");
let lib_name = expected_lib_name(target_env, "dear_imgui");
let lib_dir = download_prebuilt(&cache_root, url, &lib_name, target_env)?;
```

## Build-script target ABI migration

Build scripts must preserve Rust's complete target identity, including `CARGO_CFG_TARGET_ABI`. Populate `TargetFacts::target_abi`, `NativeAbiTarget::target_abi`, and `BuildRequestInput::target_abi` with the exact Rust cfg value (usually an empty string, and `llvm` for Windows gnullvm). `BuildRequest::new` carries that value into the public `BuildRequest::target_abi` field, so callers that construct or destructure `BuildRequest` directly must handle it. The field participates in strict target validation and deterministic binding/build-request hashes.

The shared C++ runtime helpers also require the ABI argument:

```rust
let target_abi = std::env::var("CARGO_CFG_TARGET_ABI").unwrap_or_default();

build_support::configure_cpp_runtime_linkage(
    &mut build,
    &target_os,
    &target_env,
    &target_abi,
);
let linkage = build_support::prebuilt_cpp_runtime_linkage(
    &target_os,
    &target_env,
    &target_abi,
);
build_support::emit_prebuilt_cpp_runtime_linkage(&target_os, &target_env, &target_abi);
```

On Windows, MSVC keeps its toolchain-default C++ runtime, GNU-GCC links static `stdc++`, and GNU/LLVM (`target_abi="llvm"`) links static `c++`. Callers should not add separate `c++abi`, unwind, or Windows system libraries to compensate for this decision.

## Blocking HTTP and TLS

HTTP(S) download support is behind the feature `download`, which enables `ureq` (with rustls).
By default, the crate does not pull in an HTTP client.

`download_prebuilt()` always accepts local file paths (including `file://...`) without requiring
the `download` feature. Note that extracting `.tar.gz` archives requires the feature `archive`
(enabled automatically by `download`).

## When to Use

- In `build.rs` of `dear-imgui-sys` and extension `-sys` crates to handle optional prebuilt flows.
- In internal packaging tools (e.g., `bin/package`) to ensure archive names and manifests are consistent.

## Docs.rs / Offline Builds

This crate is not required for docs.rs. For crates that generate bindings at build time, consider
checking in `src/bindings_pregenerated.rs` and copying/sanitizing it in `build.rs` during docs.rs builds.

## License

Dual-licensed under MIT or Apache-2.0.
