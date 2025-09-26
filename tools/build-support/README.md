# dear-imgui-build-support

Build helper utilities shared by `dear-imgui-sys` and its extensions.

This crate is intended for use in build scripts (`build.rs`) and internal packaging tools.
It centralizes common logic around naming prebuilt archives, generating manifests, and
downloading/extracting prebuilt static libraries.

## Features

- Compose archive names in a consistent scheme:
  `<crate>-prebuilt-<version>-<target>-<link>[<extra>][-<crt>].tar.gz`
- Compose a `manifest.txt` describing the prebuilt contents (version, target, link type, CRT, features)
- Download `.tar.gz` archives (blocking reqwest + rustls) and extract to a cache
- Utility helpers to build candidate GitHub release URLs

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

## Blocking HTTP and TLS

This crate enables `reqwest` with the `blocking` and `rustls-tls` features. Build scripts use
blocking I/O to download prebuilt archives when requested by environment variables or features.

## When to Use

- In `build.rs` of `dear-imgui-sys` and extension `-sys` crates to handle optional prebuilt flows.
- In internal packaging tools (e.g., `bin/package`) to ensure archive names and manifests are consistent.

## Docs.rs / Offline Builds

This crate is not required for docs.rs. For crates that generate bindings at build time, consider
checking in `src/bindings_pregenerated.rs` and copying/sanitizing it in `build.rs` during docs.rs builds.

## License

Dual-licensed under MIT or Apache-2.0.

