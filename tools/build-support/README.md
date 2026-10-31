# dear-imgui-build-support

Build helper utilities shared by `dear-imgui-sys` and its extensions.

This crate is intended for use in build scripts (`build.rs`) and internal packaging tools.
It centralizes common logic around naming prebuilt archives, generating manifests, and
downloading/extracting prebuilt static libraries.

The `0.22` release train requires Rust 1.95 or newer. Build scripts should require at least the matching full release version (`0.22.0`, allowing later compatible patches) so a lockfile cannot retain an older helper without the required APIs. Review the shared [native build policy migration](../../docs/COMPATIBILITY.md#0220-native-build-policy).

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

The simple `compose_manifest_bytes` helper above does not create the strict
provenance manifest required by the core and extension sys consumers. Enable
`binding-spec` and use `binding::ArtifactProfile` / `CoreArtifactIdentity` for
core artifacts and `binding::ExtensionArtifactProfile` for extensions; those
profiles bind source revisions, binding and source-contract hashes, and the
release candidate. `download_prebuilt` stages files but does not establish that
they are compatible: validate the matching profile before emitting link flags.
Use the repository package tools to produce release-compatible archives.

## Native Build Policy

`native_build::NativeBuildPlan::from_env(prefix, source_required, prebuilt_enabled)` reads native mode settings once and emits their rerun triggers.
It normalizes the new `BUILD_MODE` setting and legacy `SKIP_CC` / `FORCE_BUILD` flags without performing downloads or invoking a compiler.
Raw inputs and the value-only resolver are private; sys crates inspect a validated plan through `mode()` and `prebuilt_source()` rather than constructing or mutating its fields.
The core uses `NativeBuildPlan::from_core_env` to normalize the existing Test Engine dual-skip route at the same input adapter, without re-reading mode flags in its build script.

Sys crates keep their target-specific compilation, ABI validation, and link order.
In automatic or strict-prebuilt mode, an explicit `PrebuiltSource` is acquired with `acquire`; the returned directory must still pass the crate's existing artifact-profile validation before any link directives are emitted.
Acquisition errors are fatal for explicit inputs. `finish_prebuilt_lookup` permits an automatic cache miss but rejects one in strict-prebuilt mode.
Explicit sources use separate cache namespaces. Local archive replacement invalidates extraction through a content freshness key under the existing extraction lock; offline HTTP routes may reuse cached downloads but never start a network request.
Core and extension manifest readers track both candidate locations, watching the parent directory for absent candidates to catch newly added overrides without perpetual rebuilds.
No generic provider registry, build graph, or binary ABI inference is involved.

Core mode changes participate in the existing build-request hash.
Docs.rs, binding regeneration, and browser WASM imports remain separate paths and must not accidentally invoke native policy acquisition.

## Build-script Target ABI Migration

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
build_support::compile_cpp_archive(
    &mut build,
    "native_archive",
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

On Windows, MSVC keeps its toolchain-default C++ runtime, GNU-GCC links static `stdc++`, and GNU/LLVM (`target_abi="llvm"`) links static `c++`. Source builds must finish with `compile_cpp_archive` instead of calling `cc::Build::compile` directly so the static runtime follows every emitted archive. Prebuilt callers emit the runtime immediately after their own `rustc-link-lib` directive. Callers should not add separate `c++abi`, unwind, or Windows system libraries to compensate for this decision.

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

The sys crates still compile this helper as a build dependency on docs.rs.
Their `DOCS_RS` routes select validated, target-appropriate checked-in bindings
and skip native compilation. Those routes must work without source submodules,
Git metadata, or network access; do not regenerate bindings from unversioned
headers in a consumer build.

## License

Dual-licensed under MIT or Apache-2.0.
