# dear-imgui-build-support

Build helper utilities shared by `dear-imgui-sys` and its extensions.

This crate is intended for use in build scripts (`build.rs`) and internal packaging tools.
It centralizes common logic around naming prebuilt archives, generating manifests, and
downloading/extracting prebuilt static libraries.

The `0.23` release train requires Rust 1.95 or newer. Use `version = "0.23.0"` for the build dependency so the lockfile can select compatible patches starting at 0.23.0. The [0.22 native build policy](../../docs/COMPATIBILITY.md#0220-native-build-policy) applies to prebuilt selection and legacy environment flags.

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

## Download Verification and Cache

The `download` feature enables HTTPS downloads with `ureq` and rustls. Remote
`.tar.gz` and `.tgz` archives must match their filename entry in `SHA256SUMS`
from the same release directory. Set `DEAR_IMGUI_SHA256SUMS` to the path of a
local checksum file to override that manifest. A cached archive and checksum
file are verified without a network request, even in online mode.
With `CARGO_NET_OFFLINE=1`, a remote URL requires a cached archive and either
its cached release manifest or the local override. Automatic release discovery
can fall back to building from source when the archive or release manifest is
unavailable. A checksum verification failure still fails the build.
Set `DEAR_IMGUI_ALLOW_INSECURE_HTTP=1` only for local HTTP testing.

An explicit `*_SYS_CACHE_DIR` specifies the crate's cache root. Otherwise, the
helper uses a crate-specific subdirectory of `DEAR_IMGUI_CACHE_DIR` or defaults
to `OUT_DIR/dear-imgui-prebuilt-cache`.

Local paths (including `file://...`) and `*_SYS_PACKAGE_DIR` use local artifacts
without the release checksum file. Extracting local archives requires the
`archive` feature, which `download` also enables. Explicit `*_SYS_LIB_DIR`
selects an already extracted library. Sys crates validate the artifact profile
before linking in all cases.

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
