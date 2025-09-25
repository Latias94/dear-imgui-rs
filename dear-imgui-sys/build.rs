use flate2::read::GzDecoder;
use std::env;
use std::fs;
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

fn main() {
    let cfg = BuildConfig::new();

    // Re-run triggers
    println!("cargo:rerun-if-changed=build.rs");
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

    // Native: always generate bindings from cimgui
    generate_bindings_native(&cfg);

    // Build strategy selection via features + env var override
    // Force native build when explicitly requested or when sandboxed
    // (we still prefer prebuilt if compatible, including freetype variants).
    let force_build =
        cfg!(feature = "build-from-source") || env::var("IMGUI_SYS_FORCE_BUILD").is_ok();

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
    } else if !linked_prebuilt {
        if env::var("IMGUI_SYS_SKIP_CC").is_ok() {
            println!("cargo:warning=Skipping C/C++ build due to IMGUI_SYS_SKIP_CC");
        }
        println!("cargo:warning=WASM target is not supported.");
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
    let bindings = bindgen::Builder::default()
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
            let cache_root = prebuilt_cache_root(cfg);
            if let Ok(lib_dir) =
                try_download_prebuilt(&cache_root, &url.to_string_lossy(), &cfg.target_env)
                && try_link_prebuilt(&lib_dir, &cfg.target_env)
            {
                println!(
                    "cargo:warning=Downloaded and using prebuilt dear_imgui from {}",
                    lib_dir.display()
                );
                linked = true;
            }
        }
        // Only attempt automatic release download when explicitly enabled.
        let allow_auto_prebuilt = matches!(
            env::var("IMGUI_SYS_USE_PREBUILT").ok().as_deref(),
            Some("1") | Some("true") | Some("yes")
        );
        if !linked
            && allow_auto_prebuilt
            && let Some(lib_dir) = try_download_prebuilt_from_release(cfg)
            && try_link_prebuilt(&lib_dir, &cfg.target_env)
        {
            println!(
                "cargo:warning=Downloaded and using prebuilt dear_imgui from release at {}",
                lib_dir.display()
            );
            linked = true;
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
        build.define("IMGUI_USE_WCHAR32", None);
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
    println!("cargo:DEFINE_IMGUITEST=0");
    if cfg.is_msvc() {
        println!("cargo:DEFINE_IMGUI_USE_WCHAR32=1");
    }
}

fn expected_lib_name(target_env: &str) -> &'static str {
    if target_env == "msvc" {
        "dear_imgui.lib"
    } else {
        "libdear_imgui.a"
    }
}

fn try_link_prebuilt(dir: &Path, target_env: &str) -> bool {
    let lib_name = expected_lib_name(target_env);
    let lib_path = dir.join(lib_name);
    if !lib_path.exists() {
        return false;
    }
    // If freetype feature is enabled, only accept prebuilt if manifest declares it
    if cfg!(feature = "freetype") {
        // Expect manifest.txt in parent of lib dir (tar layout: <extract>/lib/<lib>)
        if let Some(parent) = dir.parent() {
            let manifest = parent.join("manifest.txt");
            let mut ok = false;
            if manifest.exists()
                && let Ok(s) = std::fs::read_to_string(&manifest)
            {
                for line in s.lines() {
                    if let Some(rest) = line.strip_prefix("features=") {
                        ok = rest
                            .split(',')
                            .any(|f| f.trim().eq_ignore_ascii_case("freetype"));
                        break;
                    }
                }
            }
            if !ok {
                // Manifest missing or freetype not declared → refuse
                return false;
            }
        } else {
            return false;
        }
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
    println!("cargo:warning=Downloading prebuilt dear_imgui from {}", url);
    build_support::download_prebuilt(cache_root, url, lib_name, target_env)
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

    // Candidate archive names: prefer freetype variant when feature is enabled
    let mut candidates: Vec<String> = Vec::new();
    let target = &cfg.target_triple;
    #[cfg(feature = "freetype")]
    {
        candidates.push(build_support::compose_archive_name(
            "dear-imgui",
            &version,
            target,
            link_type,
            Some("-freetype"),
            crt,
        ));
        candidates.push(build_support::compose_archive_name(
            "dear-imgui",
            &version,
            target,
            link_type,
            Some("-freetype"),
            "",
        ));
    }
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
                    expected_lib_name(&cfg.target_env),
                ) {
                    return Some(lib_dir);
                }
            }
        }
    }

    let cache_root = prebuilt_cache_root(cfg);
    let urls = build_support::release_candidate_urls_default(&tags, &candidates);
    for url in urls {
        if let Ok(lib_dir) = try_download_prebuilt(&cache_root, &url, &cfg.target_env) {
            return Some(lib_dir);
        }
    }
    None
}

fn prebuilt_cache_root(cfg: &BuildConfig) -> PathBuf {
    if let Ok(dir) = env::var("IMGUI_SYS_CACHE_DIR") {
        return PathBuf::from(dir);
    }
    let target_dir = env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| cfg.manifest_dir.parent().unwrap().join("target"));
    target_dir.join("dear-imgui-prebuilt")
}

fn prebuilt_extract_dir_env(cache_root: &Path, target_env: &str) -> PathBuf {
    let target = env::var("TARGET").unwrap_or_default();
    let crt_suffix = if target_env == "msvc" {
        let tf = env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default();
        if tf.split(',').any(|f| f == "crt-static") {
            "-mt"
        } else {
            "-md"
        }
    } else {
        ""
    };
    cache_root
        .join(target)
        .join(format!("static{}", crt_suffix))
}

fn extract_archive_to_cache(
    archive_path: &Path,
    cache_root: &Path,
    lib_name: &str,
) -> Result<PathBuf, String> {
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let extract_dir = prebuilt_extract_dir_env(cache_root, &target_env);
    if extract_dir.exists() {
        let lib_dir = extract_dir.join("lib");
        if lib_dir.join(lib_name).exists() || extract_dir.join(lib_name).exists() {
            return Ok(lib_dir);
        }
        let _ = std::fs::remove_dir_all(&extract_dir);
    }
    fs::create_dir_all(&extract_dir)
        .map_err(|e| format!("create dir {}: {}", extract_dir.display(), e))?;
    let file = fs::File::open(archive_path)
        .map_err(|e| format!("open {}: {}", archive_path.display(), e))?;
    let mut archive = tar::Archive::new(GzDecoder::new(file));
    archive
        .unpack(&extract_dir)
        .map_err(|e| format!("unpack {}: {}", archive_path.display(), e))?;
    let lib_dir = extract_dir.join("lib");
    if lib_dir.join(lib_name).exists() {
        return Ok(lib_dir);
    }
    if extract_dir.join(lib_name).exists() {
        return Ok(extract_dir);
    }
    Err("extracted archive did not contain expected library".into())
}

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
    if cfg!(target_env = "msvc") {
        cfg.define("IMGUI_WCHAR32", "ON");
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
