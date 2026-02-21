use std::env;
use std::path::{Path, PathBuf};

// Asset-importer style build configuration and structure
#[derive(Clone, Debug)]
struct BuildConfig {
    manifest_dir: PathBuf,
    out_dir: PathBuf,
    target_os: String,
    target_env: String,
    target_arch: String,
    target_triple: String,
    profile: String,
    docs_rs: bool,
}

impl BuildConfig {
    fn new() -> Self {
        Self {
            manifest_dir: PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()),
            out_dir: PathBuf::from(env::var("OUT_DIR").unwrap()),
            target_os: env::var("CARGO_CFG_TARGET_OS").unwrap_or_default(),
            target_env: env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default(),
            target_arch: env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default(),
            target_triple: env::var("TARGET").unwrap_or_default(),
            profile: env::var("PROFILE").unwrap_or_else(|_| "release".to_string()),
            docs_rs: env::var("DOCS_RS").is_ok(),
        }
    }
    fn is_windows(&self) -> bool {
        self.target_os == "windows"
    }
    fn is_msvc(&self) -> bool {
        self.target_env == "msvc"
    }
    fn is_debug(&self) -> bool {
        self.profile == "debug"
    }
    fn use_static_crt(&self) -> bool {
        self.is_windows()
            && self.is_msvc()
            && env::var("CARGO_CFG_TARGET_FEATURE")
                .unwrap_or_default()
                .split(',')
                .any(|f| f == "crt-static")
    }
    fn cimgui_root(&self) -> PathBuf {
        self.manifest_dir.join("third-party/cimgui")
    }
    fn imgui_src(&self) -> PathBuf {
        self.cimgui_root().join("imgui")
    }
}

fn use_cmake_requested() -> bool {
    // Treat empty as not set to avoid accidental enabling on CI
    matches!(env::var("IMGUI_SYS_USE_CMAKE"), Ok(v) if !v.is_empty())
}

fn is_http_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

fn is_archive_urlish(s: &str) -> bool {
    s.ends_with(".tar.gz") || s.ends_with(".tgz")
}

fn main() {
    let cfg = BuildConfig::new();

    // Re-run triggers
    println!("cargo:rerun-if-changed=build.rs");
    // Pregenerated bindings are copied into OUT_DIR when native toolchains are disabled.
    // Track them so `cargo check` picks up refreshed bindings immediately.
    println!("cargo:rerun-if-changed=src/bindings_pregenerated.rs");
    println!("cargo:rerun-if-changed=src/wasm_bindings_pregenerated.rs");
    println!("cargo:rerun-if-env-changed=IMGUI_SYS_LIB_DIR");
    println!("cargo:rerun-if-env-changed=IMGUI_SYS_SKIP_CC");
    println!("cargo:rerun-if-env-changed=IMGUI_SYS_FORCE_BUILD");
    println!("cargo:rerun-if-env-changed=IMGUI_SYS_PREBUILT_URL");
    println!("cargo:rerun-if-env-changed=IMGUI_SYS_USE_CMAKE");
    println!("cargo:rerun-if-env-changed=CARGO_NET_OFFLINE");

    // docs.rs: generate bindings only
    if cfg.docs_rs {
        docsrs_build(&cfg);
        return;
    }

    // Maintainer workflow: regenerate bindings via bindgen without requiring native compilation.
    if build_support::parse_bool_env("DEAR_IMGUI_RS_REGEN_BINDINGS") {
        generate_bindings_native(&cfg);
        export_include_paths(&cfg);
        return;
    }

    // Bindings: default to generating via bindgen, but allow skipping all native
    // toolchain usage (cc + bindgen) via IMGUI_SYS_SKIP_CC.
    if env::var("IMGUI_SYS_SKIP_CC").is_ok() {
        if !use_pregenerated_bindings(&cfg.out_dir) {
            panic!(
                "IMGUI_SYS_SKIP_CC is set but no pregenerated bindings were found. \
                 Please ensure src/bindings_pregenerated.rs exists, or unset IMGUI_SYS_SKIP_CC."
            );
        }
    } else {
        // Native: generate bindings from cimgui
        generate_bindings_native(&cfg);
    }

    // Build strategy selection via features + env var override
    // Force native build when explicitly requested or when sandboxed
    // (we still prefer prebuilt if compatible, including freetype variants).
    let force_build = cfg!(feature = "build-from-source")
        || cfg!(feature = "test-engine")
        || env::var("IMGUI_SYS_FORCE_BUILD").is_ok();

    // Try prebuilt dear_imgui first (static lib) unless force_build
    let linked_prebuilt = if force_build {
        false
    } else {
        try_link_prebuilt_all(&cfg)
    };

    // Build from sources when needed
    if !linked_prebuilt && env::var("IMGUI_SYS_SKIP_CC").is_err() {
        if cfg.target_arch == "wasm32" {
            // If targeting Emscripten, attempt to compile C/C++ (requires emsdk toolchain)
            if cfg.target_env == "emscripten" {
                build_with_cc_wasm(&cfg);
            } else {
                // Unknown-unknown skeleton: compile only when explicitly requested
                if env::var("IMGUI_SYS_WASM_CC").is_ok() {
                    build_with_cc_wasm(&cfg);
                } else {
                    println!(
                        "cargo:warning=WASM (unknown) skeleton: skipping native C/C++ build (set IMGUI_SYS_WASM_CC=1 to enable)"
                    );
                }
            }
        } else {
            // When freetype is enabled, prefer cc path as our CMake path doesn't wire FT includes/defines yet.
            if use_cmake_requested()
                && !cfg!(feature = "freetype")
                && build_with_cmake(&cfg.manifest_dir)
            {
                // CMake path prints link flags and search paths
            } else {
                build_with_cc_cfg(&cfg);
            }
        }
    } else if !linked_prebuilt && env::var("IMGUI_SYS_SKIP_CC").is_ok() {
        println!(
            "cargo:warning=IMGUI_SYS_SKIP_CC is set but no prebuilt dear_imgui library was linked; the Rust build will likely fail at link time."
        );
    }

    // Export include paths/defines for extensions
    export_include_paths(&cfg);
}

fn docsrs_build(cfg: &BuildConfig) {
    println!("cargo:warning=DOCS_RS detected: generating bindings, skipping native build");
    println!("cargo:rustc-cfg=docsrs");
    if use_pregenerated_bindings(&cfg.out_dir) {
        return;
    }
    let cimgui_root = cfg.cimgui_root();
    let imgui_src = cfg.imgui_src();
    // Expose include paths to dependent crates during docs.rs builds
    println!("cargo:IMGUI_INCLUDE_PATH={}", imgui_src.display());
    println!("cargo:CIMGUI_INCLUDE_PATH={}", cimgui_root.display());
    let mut bindings = bindgen::Builder::default()
        .header(cimgui_root.join("cimgui.h").to_string_lossy())
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .clang_arg(format!("-I{}", cimgui_root.display()))
        .clang_arg(format!("-I{}", imgui_src.display()))
        .allowlist_function("ig.*")
        .allowlist_function("Im.*")
        .allowlist_type("Im.*")
        .allowlist_var("Im.*")
        .clang_arg("-DCIMGUI_DEFINE_ENUMS_AND_STRUCTS")
        .derive_default(true)
        .derive_debug(true)
        .derive_copy(true)
        .derive_eq(true)
        .derive_partialeq(true)
        .derive_hash(true)
        .prepend_enum_name(false)
        .layout_tests(false);
    // Keep bindgen in sync with the compiled C++ library: we always enable `IMGUI_USE_WCHAR32`
    // so `ImWchar` is a 32-bit codepoint type.
    bindings = bindings.clang_arg("-DIMGUI_USE_WCHAR32");
    if cfg!(feature = "test-engine") {
        bindings = bindings.clang_arg("-DIMGUI_ENABLE_TEST_ENGINE");
    }
    let bindings = bindings
        .generate()
        .expect("Unable to generate bindings from cimgui.h (docs.rs)");
    let out = cfg.out_dir.join("bindings.rs");
    bindings
        .write_to_file(&out)
        .expect("Couldn't write bindings (docs.rs)!");
    sanitize_bindings_file(&out);
    println!("cargo:IMGUI_INCLUDE_PATH={}", cfg.imgui_src().display());
    println!("cargo:CIMGUI_INCLUDE_PATH={}", cfg.cimgui_root().display());
}

fn generate_bindings_native(cfg: &BuildConfig) {
    // For wasm targets, prefer pregenerated bindings to avoid requiring a C sysroot
    if cfg.target_arch == "wasm32" && use_pregenerated_bindings(&cfg.out_dir) {
        // Expose include paths to dependent crates during wasm builds
        println!("cargo:IMGUI_INCLUDE_PATH={}", cfg.imgui_src().display());
        println!("cargo:CIMGUI_INCLUDE_PATH={}", cfg.cimgui_root().display());
        return;
    }

    let cimgui_root = cfg.cimgui_root();
    let imgui_src = cfg.imgui_src();
    let mut bindings = bindgen::Builder::default()
        .header(cimgui_root.join("cimgui.h").to_string_lossy())
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .clang_arg(format!("-I{}", cimgui_root.display()))
        .clang_arg(format!("-I{}", imgui_src.display()))
        .allowlist_function("ig.*")
        .allowlist_function("Im.*")
        .allowlist_type("Im.*")
        .allowlist_var("Im.*")
        .clang_arg("-DCIMGUI_DEFINE_ENUMS_AND_STRUCTS")
        .derive_default(true)
        .derive_debug(true)
        .derive_copy(true)
        .derive_eq(true)
        .derive_partialeq(true)
        .derive_hash(true)
        .prepend_enum_name(false)
        .layout_tests(false);
    // Keep bindgen in sync with the compiled C++ library: we always enable `IMGUI_USE_WCHAR32`
    // so `ImWchar` is a 32-bit codepoint type.
    bindings = bindings.clang_arg("-DIMGUI_USE_WCHAR32");
    if cfg!(feature = "test-engine") {
        bindings = bindings.clang_arg("-DIMGUI_ENABLE_TEST_ENGINE");
    }
    #[cfg(feature = "freetype")]
    if let Ok(freetype) = pkg_config::probe_library("freetype2") {
        // Mirror CMake behavior: when building with FreeType, also keep stb_truetype enabled
        // so wrappers referencing stb helpers stay valid.
        bindings = bindings
            .clang_arg("-DIMGUI_ENABLE_FREETYPE=1")
            .clang_arg("-DIMGUI_ENABLE_STB_TRUETYPE=1");
        for include in &freetype.include_paths {
            bindings = bindings.clang_args(["-I", &include.display().to_string()]);
        }
    }
    // WASM-friendly: disable file/OS-specific functions in bindings when targeting wasm
    if cfg.target_arch == "wasm32" {
        bindings = bindings
            .clang_arg("-DIMGUI_DISABLE_FILE_FUNCTIONS")
            .clang_arg("-DIMGUI_DISABLE_OSX_FUNCTIONS")
            .clang_arg("-DIMGUI_DISABLE_WIN32_FUNCTIONS");
    }
    let bindings = bindings
        .generate()
        .expect("Unable to generate bindings from cimgui.h");
    let out = cfg.out_dir.join("bindings.rs");
    bindings
        .write_to_file(&out)
        .expect("Couldn't write bindings!");
    sanitize_bindings_file(&out);
}

fn try_link_prebuilt_all(cfg: &BuildConfig) -> bool {
    let mut linked = false;
    if cfg.target_arch != "wasm32" {
        if let Some(lib_dir) = env::var_os("IMGUI_SYS_LIB_DIR") {
            let lib_dir = PathBuf::from(lib_dir);
            if try_link_prebuilt(&lib_dir, &cfg.target_env) {
                println!(
                    "cargo:warning=Using prebuilt dear_imgui from {}",
                    lib_dir.display()
                );
                linked = true;
            }
        }
        if !linked && let Some(url) = env::var_os("IMGUI_SYS_PREBUILT_URL") {
            let url = url.to_string_lossy();
            if (is_http_url(&url) || is_archive_urlish(&url)) && !cfg!(feature = "prebuilt") {
                println!(
                    "cargo:warning=IMGUI_SYS_PREBUILT_URL is an HTTP(S) URL or a .tar.gz archive, but feature `prebuilt` is disabled; \
                     enable it to allow downloads/extraction (e.g. `cargo build -p dear-imgui-sys --features prebuilt`) \
                     or use IMGUI_SYS_LIB_DIR / repo prebuilts instead."
                );
            } else {
                let cache_root = prebuilt_cache_root(cfg);
                if let Ok(lib_dir) = try_download_prebuilt(&cache_root, &url, &cfg.target_env)
                    && try_link_prebuilt(&lib_dir, &cfg.target_env)
                {
                    println!(
                        "cargo:warning=Downloaded and using prebuilt dear_imgui from {}",
                        lib_dir.display()
                    );
                    linked = true;
                }
            }
        }
        // Only attempt automatic release download when explicitly enabled.
        let allow_feature = cfg!(feature = "prebuilt");
        let allow_env = matches!(
            env::var("IMGUI_SYS_USE_PREBUILT").ok().as_deref(),
            Some("1") | Some("true") | Some("yes")
        );
        if allow_env && !allow_feature {
            println!(
                "cargo:warning=IMGUI_SYS_USE_PREBUILT is set, but feature `prebuilt` is disabled; \
                 downloads are unavailable without enabling the feature (e.g. `cargo build -p dear-imgui-sys --features prebuilt`)."
            );
        }
        let allow_auto_prebuilt = allow_feature;
        if !linked && allow_auto_prebuilt {
            let source = match (allow_feature, allow_env) {
                (true, true) => "feature+env",
                (true, false) => "feature",
                _ => "",
            };
            let (owner, repo) = build_support::release_owner_repo();
            println!(
                "cargo:warning=auto-prebuilt enabled (dear-imgui-sys): source={}, repo={}/{}",
                source, owner, repo
            );
            if let Some(lib_dir) = try_download_prebuilt_from_release(cfg)
                && try_link_prebuilt(&lib_dir, &cfg.target_env)
            {
                println!(
                    "cargo:warning=Downloaded and using prebuilt dear_imgui from release at {}",
                    lib_dir.display()
                );
                linked = true;
            }
        }
        if !linked {
            let repo_prebuilt = cfg
                .manifest_dir
                .join("third-party")
                .join("prebuilt")
                .join(&cfg.target_triple);
            if try_link_prebuilt(&repo_prebuilt, &cfg.target_env) {
                println!(
                    "cargo:warning=Using repo prebuilt dear_imgui from {}",
                    repo_prebuilt.display()
                );
                linked = true;
            }
        }
    }
    linked
}

fn build_with_cc_cfg(cfg: &BuildConfig) {
    let mut build = cc::Build::new();
    build.cpp(true).std("c++17");
    let cimgui_root = cfg.cimgui_root();
    let imgui_src = cfg.imgui_src();
    build.include(&cimgui_root);
    build.include(&imgui_src);
    build.file(imgui_src.join("imgui.cpp"));
    build.file(imgui_src.join("imgui_draw.cpp"));
    build.file(imgui_src.join("imgui_widgets.cpp"));
    build.file(imgui_src.join("imgui_tables.cpp"));
    // Include official demo/metrics/debug windows for native builds so symbols like
    // ImGui::ShowDemoWindow/ShowAboutWindow/ShowStyleEditor resolve.
    // This is excluded from the WASM single‑module path below.
    build.file(imgui_src.join("imgui_demo.cpp"));
    build.file(cimgui_root.join("cimgui.cpp"));
    build.define("IMGUI_USE_WCHAR32", None);
    if cfg!(feature = "test-engine") {
        build.define("IMGUI_ENABLE_TEST_ENGINE", None);
    }
    if cfg.is_msvc() && cfg.is_windows() {
        build.flag("/EHsc");
        let use_static = cfg.use_static_crt();
        build.static_crt(use_static);
        if use_static {
            build.flag("/MT");
        } else {
            build.flag("/MD");
        }
        if cfg.is_debug() {
            build.debug(true);
            build.opt_level(0);
        } else {
            build.debug(false);
            build.opt_level(2);
        }
        build.flag("/D_ITERATOR_DEBUG_LEVEL=0");
    }
    #[cfg(feature = "freetype")]
    if let Ok(freetype) = pkg_config::probe_library("freetype2") {
        // Enable both FreeType and stb_truetype backends.
        // ImGui 1.92 gates stb_truetype helpers (e.g. ImFontAtlasGetFontLoaderForStbTruetype)
        // behind IMGUI_ENABLE_STB_TRUETYPE, while FreeType is selected when IMGUI_ENABLE_FREETYPE is defined.
        // Defining both keeps the stb_ symbols available for cimgui wrappers while still defaulting to FreeType.
        build.define("IMGUI_ENABLE_FREETYPE", Some("1"));
        build.define("IMGUI_ENABLE_STB_TRUETYPE", Some("1"));
        for include in &freetype.include_paths {
            build.include(include.display().to_string());
        }
        build.file(cfg.imgui_src().join("misc/freetype/imgui_freetype.cpp"));
    }
    build.compile("dear_imgui");
}

fn export_include_paths(cfg: &BuildConfig) {
    println!("cargo:THIRD_PARTY={}", cfg.imgui_src().display());
    println!("cargo:IMGUI_INCLUDE_PATH={}", cfg.imgui_src().display());
    println!("cargo:CIMGUI_INCLUDE_PATH={}", cfg.cimgui_root().display());
    println!(
        "cargo:DEFINE_IMGUI_ENABLE_TEST_ENGINE={}",
        if cfg!(feature = "test-engine") {
            "1"
        } else {
            "0"
        }
    );
    println!(
        "cargo:DEFINE_IMGUITEST={}",
        if cfg!(feature = "test-engine") {
            "1"
        } else {
            "0"
        }
    );
    println!("cargo:DEFINE_IMGUI_USE_WCHAR32=1");
}

fn expected_lib_name(target_env: &str) -> String {
    build_support::expected_lib_name(target_env, "dear_imgui")
}

fn prebuilt_manifest_features(dir: &Path) -> Option<Vec<String>> {
    let mut candidates = Vec::with_capacity(2);
    candidates.push(dir.join("manifest.txt"));
    if let Some(parent) = dir.parent() {
        candidates.push(parent.join("manifest.txt"));
    }

    for manifest in candidates {
        let Ok(s) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        for line in s.lines() {
            if let Some(rest) = line.strip_prefix("features=") {
                return Some(
                    rest.split(',')
                        .map(|f| f.trim().to_ascii_lowercase())
                        .filter(|f| !f.is_empty())
                        .collect(),
                );
            }
        }
        return Some(Vec::new());
    }

    None
}

fn prebuilt_manifest_has_feature(dir: &Path, feature: &str) -> bool {
    let feature = feature.trim().to_ascii_lowercase();
    let Some(features) = prebuilt_manifest_features(dir) else {
        return false;
    };
    features.iter().any(|f| f == &feature)
}

fn try_link_prebuilt(dir: &Path, target_env: &str) -> bool {
    let lib_name = expected_lib_name(target_env);
    let lib_path = dir.join(lib_name.as_str());
    if !lib_path.exists() {
        return false;
    }

    // Prebuilt ABI guard: we always compile with `IMGUI_USE_WCHAR32`, so we must not link a
    // wchar16 prebuilt. Enforce this via the package manifest.
    if !prebuilt_manifest_has_feature(dir, "wchar32") {
        return false;
    }
    // If freetype feature is enabled, only accept prebuilt if manifest declares it
    if cfg!(feature = "freetype") && !prebuilt_manifest_has_feature(dir, "freetype") {
        return false;
    }
    // If test-engine feature is enabled, only accept prebuilt if manifest declares it.
    //
    // Without this, cargo could silently use a non-test-engine prebuilt while the Rust side
    // expects IMGUI_ENABLE_TEST_ENGINE hooks to be present, leading to confusing runtime failures.
    if cfg!(feature = "test-engine") && !prebuilt_manifest_has_feature(dir, "test-engine") {
        return false;
    }
    // Conversely, if test-engine is disabled, reject prebuilts that were built with it enabled:
    // those objects reference hook symbols that won't be linked, causing undefined symbols at link time.
    if !cfg!(feature = "test-engine") && prebuilt_manifest_has_feature(dir, "test-engine") {
        return false;
    }
    println!("cargo:rustc-link-search=native={}", dir.display());
    println!("cargo:rustc-link-lib=static=dear_imgui");
    true
}

// Minimal WASM (skeleton) build: compile cimgui + imgui with WASM-friendly defines.
// This will be extended with proper toolchain flags in future iterations.
fn build_with_cc_wasm(cfg: &BuildConfig) {
    let mut build = cc::Build::new();
    build.cpp(true).std("c++17");
    let cimgui_root = cfg.cimgui_root();
    let imgui_src = cfg.imgui_src();
    build.include(&cimgui_root);
    build.include(&imgui_src);
    build.file(imgui_src.join("imgui.cpp"));
    build.file(imgui_src.join("imgui_draw.cpp"));
    build.file(imgui_src.join("imgui_widgets.cpp"));
    build.file(imgui_src.join("imgui_tables.cpp"));
    build.file(imgui_src.join("imgui_demo.cpp"));
    build.file(cimgui_root.join("cimgui.cpp"));

    // If EMSDK is available, prefer its upstream clang++ for wasm32-unknown-unknown objects
    if let Ok(emsdk) = std::env::var("EMSDK") {
        let mut clangpp = PathBuf::from(emsdk.clone());
        // EMSDK/upstream/bin/clang++
        clangpp.push("upstream");
        clangpp.push("bin");
        clangpp.push(if cfg!(windows) {
            "clang++.exe"
        } else {
            "clang++"
        });
        if clangpp.exists() {
            build.compiler(clangpp);
            build.flag("-target");
            build.flag("wasm32-unknown-unknown");
            // Use emscripten sysroot headers to resolve <string.h>, etc.
            let mut sysroot = PathBuf::from(&emsdk);
            sysroot.push("upstream");
            sysroot.push("emscripten");
            sysroot.push("cache");
            sysroot.push("sysroot");
            if sysroot.exists() {
                build.flag(format!("--sysroot={}", sysroot.display()));
            }
        }
    } else {
        // Fallback: ask clang to target wasm32
        build.flag("-target");
        build.flag("wasm32-unknown-unknown");
    }

    // WASM-friendly defines
    build.define("IMGUI_DISABLE_FILE_FUNCTIONS", None);
    build.define("IMGUI_DISABLE_OSX_FUNCTIONS", None);
    build.define("IMGUI_DISABLE_WIN32_FUNCTIONS", None);
    build.define("IMGUI_USE_WCHAR32", None);
    if cfg!(feature = "test-engine") {
        build.define("IMGUI_ENABLE_TEST_ENGINE", None);
    }

    // Avoid exceptions/RTTI
    build.flag_if_supported("-fno-exceptions");
    build.flag_if_supported("-fno-rtti");

    // Do not link the C++ standard library for wasm32-unknown-unknown
    build.cpp_link_stdlib(None);

    build.compile("dear_imgui");
}

fn try_download_prebuilt(
    cache_root: &Path,
    url: &str,
    target_env: &str,
) -> Result<PathBuf, String> {
    let lib_name = expected_lib_name(target_env);
    if is_http_url(url) {
        println!("cargo:warning=Downloading prebuilt dear_imgui from {}", url);
    } else {
        println!("cargo:warning=Using prebuilt dear_imgui from {}", url);
    }
    build_support::download_prebuilt(cache_root, url, lib_name.as_str(), target_env)
}

fn try_download_prebuilt_from_release(cfg: &BuildConfig) -> Option<PathBuf> {
    if build_support::is_offline() {
        return None;
    }

    let version = env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let link_type = "static";
    let crt = if cfg.is_windows() && cfg.is_msvc() {
        if cfg.use_static_crt() { "mt" } else { "md" }
    } else {
        ""
    };

    // Candidate archive names: match enabled features exactly (e.g. -freetype, -test-engine,
    // -freetype-test-engine). We still validate the manifest in `try_link_prebuilt()`, but this
    // avoids downloading/trying obviously incompatible prebuilts.
    let mut candidates: Vec<String> = Vec::new();
    let target = &cfg.target_triple;
    let mut suffix = String::new();
    if cfg!(feature = "freetype") {
        suffix.push_str("-freetype");
    }
    if cfg!(feature = "test-engine") {
        suffix.push_str("-test-engine");
    }
    if suffix.is_empty() {
        candidates.push(build_support::compose_archive_name(
            "dear-imgui",
            &version,
            target,
            link_type,
            None,
            crt,
        ));
        candidates.push(build_support::compose_archive_name(
            "dear-imgui",
            &version,
            target,
            link_type,
            None,
            "",
        ));
    } else {
        candidates.push(build_support::compose_archive_name(
            "dear-imgui",
            &version,
            target,
            link_type,
            Some(suffix.as_str()),
            crt,
        ));
        candidates.push(build_support::compose_archive_name(
            "dear-imgui",
            &version,
            target,
            link_type,
            Some(suffix.as_str()),
            "",
        ));
    }

    let tags = build_support::release_tags("dear-imgui-sys", &version);

    // Try local package dir first
    if let Ok(pkg_dir) = env::var("IMGUI_SYS_PACKAGE_DIR") {
        let pkg_dir = PathBuf::from(pkg_dir);
        for name in &candidates {
            let archive_path = pkg_dir.join(name);
            if archive_path.exists() {
                let cache_root = prebuilt_cache_root(cfg);
                if let Ok(lib_dir) = build_support::extract_archive_to_cache(
                    &archive_path,
                    &cache_root,
                    expected_lib_name(&cfg.target_env).as_str(),
                ) {
                    return Some(lib_dir);
                }
            }
        }
    }

    let cache_root = prebuilt_cache_root(cfg);
    let urls = build_support::release_candidate_urls_env(&tags, &candidates);
    for url in urls {
        if let Ok(lib_dir) = try_download_prebuilt(&cache_root, &url, &cfg.target_env) {
            return Some(lib_dir);
        }
    }
    None
}

fn prebuilt_cache_root(cfg: &BuildConfig) -> PathBuf {
    build_support::prebuilt_cache_root_from_env_or_target(
        &cfg.manifest_dir,
        "IMGUI_SYS_CACHE_DIR",
        "dear-imgui-prebuilt",
    )
}

// (removed duplicate prebuilt_extract_dir_env/extract_archive_to_cache; using build_support equivalents)

fn build_with_cmake(manifest_dir: &Path) -> bool {
    let cimgui_root = manifest_dir.join("third-party/cimgui");
    if !cimgui_root.join("CMakeLists.txt").exists() {
        return false;
    }
    println!("cargo:warning=Building cimgui with CMake");
    let mut cfg = cmake::Config::new(&cimgui_root);
    cfg.define("IMGUI_STATIC", "ON");
    // Profile selection (RelWithDebInfo on MSVC when cargo debug)
    let profile = env::var("PROFILE").unwrap_or_else(|_| "release".into());
    let cmake_profile = if cfg!(target_env = "msvc") && profile == "debug" {
        "RelWithDebInfo"
    } else if profile == "debug" {
        "Debug"
    } else {
        "Release"
    };
    cfg.profile(cmake_profile);
    cfg.define("IMGUI_WCHAR32", "ON");
    if cfg!(target_env = "msvc") {
        let target_features = env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default();
        let use_static_crt = target_features.split(',').any(|f| f == "crt-static");
        let msvc_runtime = if use_static_crt {
            "MultiThreaded"
        } else {
            "MultiThreadedDLL"
        };
        cfg.define("CMAKE_MSVC_RUNTIME_LIBRARY", msvc_runtime);
    }
    let dst = cfg.build();
    // Gather lib dirs
    let mut lib_dirs = vec![
        dst.join("lib"),
        dst.join("build"),
        dst.clone(),
        dst.join("build").join("RelWithDebInfo"),
        dst.join("build").join("Release"),
        dst.join("build").join("Debug"),
        dst.join("RelWithDebInfo"),
        dst.join("Release"),
        dst.join("Debug"),
    ];
    let mut found = false;
    for lib_dir in lib_dirs.drain(..) {
        if lib_dir.exists() {
            println!("cargo:rustc-link-search=native={}", lib_dir.display());
            found = true;
            #[cfg(not(target_env = "msvc"))]
            {
                let bare = lib_dir.join("cimgui.a");
                let with_prefix = lib_dir.join("libcimgui.a");
                if bare.exists() && !with_prefix.exists() {
                    let _ = std::fs::copy(&bare, &with_prefix);
                }
            }
        }
    }
    if !found {
        println!("cargo:warning=Could not locate CMake lib output dir; linking may fail");
    }
    if cfg!(target_env = "msvc") {
        println!("cargo:rustc-link-lib=static=cimgui");
    } else {
        println!("cargo:rustc-link-lib=static=:cimgui.a");
    }
    println!(
        "cargo:IMGUI_INCLUDE_PATH={}",
        cimgui_root.join("imgui").display()
    );
    println!("cargo:CIMGUI_INCLUDE_PATH={}", cimgui_root.display());
    true
}

fn use_pregenerated_bindings(out_dir: &Path) -> bool {
    if build_support::parse_bool_env("DEAR_IMGUI_RS_REGEN_BINDINGS") {
        return false;
    }

    // Prefer wasm pregenerated bindings when targeting wasm32, unless single-module is requested
    let single_module = std::env::var("IMGUI_SYS_SINGLE_MODULE")
        .ok()
        .map(|v| v == "1")
        .unwrap_or(false);
    let is_wasm = std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32");
    let candidates = if is_wasm && !single_module {
        vec![
            Path::new("src").join("wasm_bindings_pregenerated.rs"),
            Path::new("src").join("bindings_pregenerated.rs"),
        ]
    } else {
        vec![Path::new("src").join("bindings_pregenerated.rs")]
    };

    for preg in candidates {
        if preg.exists() {
            match std::fs::read_to_string(&preg).and_then(|content| {
                let sanitized = sanitize_bindings_string(&content);
                std::fs::write(out_dir.join("bindings.rs"), sanitized)
            }) {
                Ok(()) => {
                    println!(
                        "cargo:warning=Using pregenerated bindings: {}",
                        preg.display()
                    );
                    return true;
                }
                Err(e) => {
                    println!("cargo:warning=Failed to write pregenerated bindings: {}", e);
                    return false;
                }
            }
        }
    }
    false
}

fn sanitize_bindings_file(path: &Path) {
    if let Ok(content) = std::fs::read_to_string(path) {
        let sanitized = sanitize_bindings_string(&content);
        let _ = std::fs::write(path, sanitized);
    }
}

fn sanitize_bindings_string(content: &str) -> String {
    // Remove any inner attributes like #![allow(...)] which may be emitted by bindgen
    // and can be rejected depending on include context. Also drop an immediate blank
    // line following such attributes to keep the file tidy.
    let mut out = String::with_capacity(content.len());
    let mut skip_next_blank = false;
    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("#![") {
            skip_next_blank = true;
            continue; // drop this line
        }
        if skip_next_blank {
            if trimmed.is_empty() {
                continue;
            }
            skip_next_blank = false;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}
