use flate2::read::GzDecoder;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
struct BuildConfig {
    manifest_dir: PathBuf,
    out_dir: PathBuf,
    target_os: String,
    target_env: String,
    target_arch: String,
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
            docs_rs: env::var("DOCS_RS").is_ok(),
        }
    }
    fn is_msvc(&self) -> bool {
        self.target_env == "msvc"
    }
    fn is_windows(&self) -> bool {
        self.target_os == "windows"
    }
    fn use_static_crt(&self) -> bool {
        self.is_msvc()
            && self.is_windows()
            && env::var("CARGO_CFG_TARGET_FEATURE")
                .unwrap_or_default()
                .split(',')
                .any(|f| f == "crt-static")
    }
}

fn use_cmake_requested() -> bool {
    matches!(env::var("IMPLOT_SYS_USE_CMAKE"), Ok(v) if !v.is_empty())
}

fn resolve_imgui_includes(cfg: &BuildConfig) -> (PathBuf, PathBuf) {
    // Prefer paths exported by dear-imgui-sys build script (prefix comes from links = "dear-imgui")
    let imgui_src = env::var_os("DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH")
        .or_else(|| env::var_os("DEP_DEAR_IMGUI_THIRD_PARTY"))
        .map(PathBuf::from)
        .unwrap_or_else(|| cfg.manifest_dir.join("../../dear-imgui-sys/imgui"));
    let cimgui_root = env::var_os("DEP_DEAR_IMGUI_CIMGUI_INCLUDE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            cfg.manifest_dir
                .join("../../dear-imgui-sys/third-party/cimgui")
        });
    (imgui_src, cimgui_root)
}

fn generate_bindings(cfg: &BuildConfig, cimplot_root: &Path, imgui_src: &Path, cimgui_root: &Path) {
    let bindings = bindgen::Builder::default()
        .header(cimplot_root.join("cimplot.h").to_string_lossy())
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .allowlist_function("ImPlot.*")
        .allowlist_type("ImPlot.*")
        .allowlist_var("ImPlot.*")
        .allowlist_var("IMPLOT_.*")
        .blocklist_type("ImVec2")
        .blocklist_type("ImVec4")
        .blocklist_type("ImGuiCond")
        .blocklist_type("ImTextureID")
        .blocklist_type("ImGuiContext")
        .blocklist_type("ImDrawList")
        .blocklist_type("ImGuiMouseButton")
        .blocklist_type("ImGuiDragDropFlags")
        .blocklist_type("ImGuiIO")
        .blocklist_type("ImFontAtlas")
        .blocklist_type("ImDrawData")
        .blocklist_type("ImGuiStyle")
        .blocklist_type("ImGuiKeyModFlags")
        .derive_default(true)
        .derive_debug(true)
        .derive_copy(true)
        .derive_eq(true)
        .derive_partialeq(true)
        .derive_hash(true)
        .prepend_enum_name(false)
        .layout_tests(false)
        .clang_arg(format!("-I{}", imgui_src.display()))
        .clang_arg(format!("-I{}", cimgui_root.display()))
        .clang_arg(format!("-I{}", cimplot_root.display()))
        .clang_arg(format!("-I{}", cimplot_root.join("implot").display()))
        .clang_arg("-DCIMGUI_DEFINE_ENUMS_AND_STRUCTS")
        .clang_arg("-x")
        .clang_arg("c++")
        .clang_arg("-std=c++17")
        .generate()
        .expect("Unable to generate bindings");
    let out = cfg.out_dir.join("bindings.rs");
    bindings
        .write_to_file(&out)
        .expect("Couldn't write bindings!");
    sanitize_bindings_file(&out);
}

fn try_link_prebuilt_all(cfg: &BuildConfig) -> bool {
    let target_env = &cfg.target_env;
    if let Ok(dir) = env::var("IMPLOT_SYS_LIB_DIR") {
        if try_link_prebuilt(PathBuf::from(dir), target_env) {
            return true;
        }
        println!(
            "cargo:warning=IMPLOT_SYS_LIB_DIR set but no library found; falling back to build"
        );
    }
    if let Ok(url) = env::var("IMPLOT_SYS_PREBUILT_URL") {
        let cache_root = prebuilt_cache_root(cfg);
        if let Ok(dir) = try_download_prebuilt(&cache_root, &url, target_env) {
            if try_link_prebuilt(dir.clone(), target_env) {
                return true;
            }
            println!(
                "cargo:warning=Downloaded prebuilt library but failed to link from {}",
                dir.display()
            );
        }
    } else {
        let allow_feature = cfg!(feature = "prebuilt");
        let allow_env = matches!(
            env::var("IMPLOT_SYS_USE_PREBUILT").ok().as_deref(),
            Some("1") | Some("true") | Some("yes")
        );
        let allow_auto_prebuilt = allow_feature || allow_env;
        if allow_auto_prebuilt {
            let source = match (allow_feature, allow_env) {
                (true, true) => "feature+env",
                (true, false) => "feature",
                (false, true) => "env",
                _ => "",
            };
            let (owner, repo) = build_support::release_owner_repo();
            println!(
                "cargo:warning=auto-prebuilt enabled (dear-implot-sys): source={}, repo={}/{}",
                source, owner, repo
            );
            if let Some(dir) = try_download_prebuilt_from_release(cfg)
                && try_link_prebuilt(dir.clone(), target_env)
            {
                return true;
            }
        }
    }
    false
}

fn build_with_cc(cfg: &BuildConfig, cimplot_root: &Path, imgui_src: &Path, cimgui_root: &Path) {
    let mut build = cc::Build::new();
    if cfg.target_arch == "wasm32" {
        build.define("IMGUI_DISABLE_DEFAULT_SHELL_FUNCTIONS", "1");
    } else {
        build.cpp(true).std("c++17");
    }

    // MSVC flags align with dear-imgui-sys
    if cfg.is_msvc() && cfg.is_windows() {
        build.flag("/EHsc");
        let use_static = cfg.use_static_crt();
        build.static_crt(use_static);
        if use_static {
            build.flag("/MT");
        } else {
            build.flag("/MD");
        }
        let profile = env::var("PROFILE").unwrap_or_else(|_| "release".to_string());
        if profile == "debug" {
            build.debug(true).opt_level(0);
        } else {
            build.debug(false).opt_level(2);
        }
        build.flag("/D_ITERATOR_DEBUG_LEVEL=0");
    }

    // Inherit dear-imgui defines
    for (k, v) in env::vars() {
        if let Some(suffix) = k.strip_prefix("DEP_DEAR_IMGUI_DEFINE_") {
            build.define(suffix, v.as_str());
        }
    }

    // Includes and defines
    build.define("IMGUI_DEFINE_MATH_OPERATORS", Some("1"));
    if cfg.is_msvc() {
        build.define("IMGUI_USE_WCHAR32", None);
    }
    build.include(imgui_src);
    build.include(cimgui_root);
    build.include(cimplot_root);
    build.include(cimplot_root.join("implot"));

    // Sources
    build.file(cimplot_root.join("cimplot.cpp"));
    build.file(cimplot_root.join("implot/implot.cpp"));
    build.file(cimplot_root.join("implot/implot_items.cpp"));
    build.file(cimplot_root.join("implot/implot_demo.cpp"));

    build.compile("dear_implot");
}

fn main() {
    let cfg = BuildConfig::new();

    // Rerun hints
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=third-party/cimplot/cimplot.h");
    println!("cargo:rerun-if-changed=third-party/cimplot/cimplot.cpp");
    println!("cargo:rerun-if-changed=third-party/cimplot/implot/implot.h");
    println!("cargo:rerun-if-changed=third-party/cimplot/implot/implot.cpp");
    println!("cargo:rerun-if-changed=third-party/cimplot/implot/implot_items.cpp");
    println!("cargo:rerun-if-changed=../../dear-imgui-sys");
    println!("cargo:rerun-if-env-changed=IMPLOT_SYS_LIB_DIR");
    println!("cargo:rerun-if-env-changed=IMPLOT_SYS_SKIP_CC");
    println!("cargo:rerun-if-env-changed=IMPLOT_SYS_PREBUILT_URL");
    println!("cargo:rerun-if-env-changed=IMPLOT_SYS_FORCE_BUILD");
    println!("cargo:rerun-if-env-changed=IMPLOT_SYS_USE_CMAKE");

    let (imgui_src, cimgui_root) = resolve_imgui_includes(&cfg);
    let cimplot_root = cfg.manifest_dir.join("third-party/cimplot");
    if !imgui_src.exists() {
        panic!(
            "ImGui source not found at {:?}. Did you forget to initialize git submodules?",
            imgui_src
        );
    }
    if !cimplot_root.exists() {
        panic!(
            "cimplot source not found at {:?}. Did you forget to initialize git submodules?",
            cimplot_root
        );
    }

    // Generate bindings
    generate_bindings(&cfg, &cimplot_root, &imgui_src, &cimgui_root);

    // Features: build-from-source forces source build; prebuilt is opt-in
    let force_build =
        cfg!(feature = "build-from-source") || env::var("IMPLOT_SYS_FORCE_BUILD").is_ok();
    let linked_prebuilt = if force_build {
        false
    } else {
        try_link_prebuilt_all(&cfg)
    };
    if !cfg.docs_rs
        && (force_build || (!linked_prebuilt && env::var("IMPLOT_SYS_SKIP_CC").is_err()))
    {
        if use_cmake_requested() && build_with_cmake(&cfg, &cimplot_root) {
            // built via CMake
        } else {
            build_with_cc(&cfg, &cimplot_root, &imgui_src, &cimgui_root);
        }
    } else if cfg.docs_rs {
        docsrs_build(&cfg, &cimplot_root, &imgui_src, &cimgui_root);
    }
}

fn docsrs_build(cfg: &BuildConfig, cimplot_root: &Path, imgui_src: &Path, cimgui_root: &Path) {
    println!("cargo:warning=DOCS_RS detected: generating bindings, skipping native build");
    println!("cargo:rustc-cfg=docsrs");

    if use_pregenerated_bindings(&cfg.out_dir) {
        return;
    }

    // Fallback: try to generate bindings from headers if available
    if !imgui_src.exists() || !cimgui_root.exists() || !cimplot_root.exists() {
        panic!(
            "DOCS_RS build: Required headers not found and no pregenerated bindings present.\n\
             Please add src/bindings_pregenerated.rs (full bindgen output) to enable docs.rs builds.\n\
             Run: cargo build -p dear-implot-sys && cp target/debug/build/dear-implot-sys-*/out/bindings.rs extensions/dear-implot-sys/src/bindings_pregenerated.rs"
        );
    }

    generate_bindings(cfg, cimplot_root, imgui_src, cimgui_root);
    println!("cargo:IMGUI_INCLUDE_PATH={}", imgui_src.display());
    println!("cargo:CIMGUI_INCLUDE_PATH={}", cimgui_root.display());
}

fn use_pregenerated_bindings(out_dir: &Path) -> bool {
    let preg = Path::new("src").join("bindings_pregenerated.rs");
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
                true
            }
            Err(e) => {
                println!("cargo:warning=Failed to write pregenerated bindings: {}", e);
                false
            }
        }
    } else {
        false
    }
}

fn sanitize_bindings_file(path: &Path) {
    if let Ok(content) = std::fs::read_to_string(path) {
        let sanitized = sanitize_bindings_string(&content);
        let _ = std::fs::write(path, sanitized);
    }
}

fn sanitize_bindings_string(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut skip_next_blank = false;
    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("#![") {
            skip_next_blank = true;
            continue;
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

fn build_with_cmake(cfg: &BuildConfig, cimplot_root: &Path) -> bool {
    let cmake_lists = cimplot_root.join("CMakeLists.txt");
    if !cmake_lists.exists() {
        return false;
    }
    println!("cargo:warning=Building cimplot with CMake");
    let mut c = cmake::Config::new(cimplot_root);
    c.define("IMGUI_STATIC", "ON");
    let profile = env::var("PROFILE").unwrap_or_else(|_| "release".into());
    let cmake_profile = if cfg.is_msvc() && cfg.is_windows() && profile == "debug" {
        "RelWithDebInfo"
    } else if profile == "debug" {
        "Debug"
    } else {
        "Release"
    };
    c.profile(cmake_profile);
    if cfg.is_msvc() && cfg.is_windows() {
        let tf = env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default();
        let use_static = tf.split(',').any(|f| f == "crt-static");
        let msvc_runtime = if use_static {
            "MultiThreaded"
        } else {
            "MultiThreadedDLL"
        };
        c.define("CMAKE_MSVC_RUNTIME_LIBRARY", msvc_runtime);
    }
    let dst = c.build();
    let candidates = [
        dst.join("lib"),
        dst.join("build"),
        dst.clone(),
        dst.join("build").join("Release"),
        dst.join("build").join("RelWithDebInfo"),
        dst.join("build").join("Debug"),
        dst.join("Release"),
        dst.join("RelWithDebInfo"),
        dst.join("Debug"),
    ];
    let mut found = false;
    for lib_dir in &candidates {
        if lib_dir.exists() {
            println!("cargo:rustc-link-search=native={}", lib_dir.display());
            found = true;
        }
    }
    if !found {
        println!("cargo:warning=Could not locate CMake lib output dir; linking may fail");
    }
    println!("cargo:rustc-link-lib=static=cimplot");
    true
}

fn expected_lib_name(target_env: &str) -> &'static str {
    if target_env == "msvc" {
        "dear_implot.lib"
    } else {
        "libdear_implot.a"
    }
}

fn try_link_prebuilt(dir: PathBuf, target_env: &str) -> bool {
    let lib_name = expected_lib_name(target_env);
    let lib_path = dir.join(lib_name.as_str());
    if !lib_path.exists() {
        println!(
            "cargo:warning=prebuilt dear_implot not found at {}",
            lib_path.display()
        );
        return false;
    }
    println!("cargo:rustc-link-search=native={}", dir.display());
    println!("cargo:rustc-link-lib=static=dear_implot");
    true
}

fn expected_lib_name(target_env: &str) -> String {
    build_support::expected_lib_name(target_env, "dear_implot")
}

fn try_download_prebuilt(
    cache_root: &PathBuf,
    url: &str,
    target_env: &str,
) -> Result<PathBuf, String> {
    let lib_name = expected_lib_name(target_env);
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
    let target = env::var("TARGET").unwrap_or_default();
    let archive_name =
        build_support::compose_archive_name("dear-implot", &version, &target, link_type, None, crt);
    let archive_name_no_crt =
        build_support::compose_archive_name("dear-implot", &version, &target, link_type, None, "");
    let tags = build_support::release_tags("dear-implot-sys", &version);
    if let Ok(pkg_dir) = env::var("IMPLOT_SYS_PACKAGE_DIR") {
        let pkg_dir = PathBuf::from(pkg_dir);
        for cand in [archive_name.clone(), archive_name_no_crt.clone()] {
            let archive_path = pkg_dir.join(&cand);
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
    let names = vec![archive_name, archive_name_no_crt];
    let urls = build_support::release_candidate_urls_env(&tags, &names);
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
        "IMPLOT_SYS_CACHE_DIR",
        "dear-implot-prebuilt",
    )
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
