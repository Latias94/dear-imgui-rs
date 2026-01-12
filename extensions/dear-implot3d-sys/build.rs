use std::{
    env,
    path::{Path, PathBuf},
};

fn is_http_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

fn is_archive_urlish(s: &str) -> bool {
    s.ends_with(".tar.gz") || s.ends_with(".tgz")
}

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

fn resolve_imgui_includes(cfg: &BuildConfig) -> (PathBuf, PathBuf) {
    let imgui_src = env::var_os("DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH")
        .or_else(|| env::var_os("DEP_DEAR_IMGUI_THIRD_PARTY"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            cfg.manifest_dir
                .join("../../dear-imgui-sys/third-party/cimgui/imgui")
        });
    let cimgui_root = env::var_os("DEP_DEAR_IMGUI_CIMGUI_INCLUDE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            cfg.manifest_dir
                .join("../../dear-imgui-sys/third-party/cimgui")
        });
    (imgui_src, cimgui_root)
}

fn use_pregenerated_bindings(out_dir: &Path) -> bool {
    if build_support::parse_bool_env("DEAR_IMGUI_RS_REGEN_BINDINGS") {
        return false;
    }

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

fn use_pregenerated_wasm_bindings(out_dir: &Path) -> bool {
    if build_support::parse_bool_env("DEAR_IMGUI_RS_REGEN_BINDINGS") {
        return false;
    }

    let preg = Path::new("src").join("wasm_bindings_pregenerated.rs");
    if preg.exists() {
        match std::fs::read_to_string(&preg).and_then(|content| {
            let sanitized = sanitize_bindings_string(&content);
            std::fs::write(out_dir.join("bindings.rs"), sanitized)
        }) {
            Ok(()) => {
                println!(
                    "cargo:warning=Using pregenerated wasm bindings: {}",
                    preg.display()
                );
                true
            }
            Err(e) => {
                println!(
                    "cargo:warning=Failed to write pregenerated wasm bindings: {}",
                    e
                );
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

fn generate_bindings(
    cfg: &BuildConfig,
    cimplot3d_root: &Path,
    imgui_src: &Path,
    cimgui_root: &Path,
) {
    // For wasm32 targets, rely on pregenerated import-style bindings that
    // import symbols from the shared imgui-sys-v0 provider instead of running
    // bindgen here (which requires a native C/C++ sysroot).
    if cfg.target_arch == "wasm32" {
        if !cfg!(feature = "wasm") {
            panic!(
                "dear-implot3d-sys: building for wasm32 requires the `wasm` feature.\n\
                 Enable it in your Cargo.toml: features = [\"wasm\"]"
            );
        }
        if use_pregenerated_wasm_bindings(&cfg.out_dir) {
            println!("cargo:warning=Using pregenerated wasm bindings for dear-implot3d-sys");
            return;
        }
        panic!(
            "dear-implot3d-sys: wasm32 target detected but src/wasm_bindings_pregenerated.rs not found.\n\
             Run: cargo run -p xtask -- wasm-bindgen-implot3d imgui-sys-v0"
        );
    }

    let bindings = bindgen::Builder::default()
        .header(cimplot3d_root.join("cimplot3d.h").to_string_lossy())
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .allowlist_function("ImPlot3D_.*")
        .allowlist_type("ImPlot3D.*")
        .allowlist_type("ImWchar32")
        .allowlist_var("ImPlot3D.*")
        .blocklist_type("ImVec2")
        .blocklist_type("ImVec4")
        .blocklist_type("ImGuiContext")
        .blocklist_type("ImDrawList")
        .blocklist_type("ImGuiID")
        .blocklist_type("ImTextureID")
        .clang_arg(format!("-I{}", cimgui_root.display()))
        .clang_arg(format!("-I{}", imgui_src.display()))
        .clang_arg(format!("-I{}", cimplot3d_root.display()))
        .clang_arg(format!("-I{}", cimplot3d_root.join("implot3d").display()))
        .clang_arg("-DIMGUI_USE_WCHAR32")
        .clang_arg("-DCIMGUI_DEFINE_ENUMS_AND_STRUCTS")
        .derive_default(true)
        .derive_debug(true)
        .derive_copy(true)
        .derive_eq(true)
        .derive_partialeq(true)
        .derive_hash(true)
        .prepend_enum_name(false)
        .layout_tests(false)
        .clang_arg("-x")
        .clang_arg("c++")
        .clang_arg("-std=c++17")
        .generate()
        .expect("Unable to generate implot3d bindings");
    let out = cfg.out_dir.join("bindings.rs");
    bindings
        .write_to_file(&out)
        .expect("Couldn't write bindings!");
    sanitize_bindings_file(&out);
}

fn expected_lib_name(target_env: &str) -> &'static str {
    if target_env == "msvc" {
        "dear_implot3d.lib"
    } else {
        "libdear_implot3d.a"
    }
}

fn prebuilt_manifest_has_feature(dir: &Path, feature: &str) -> bool {
    let mut candidates = Vec::with_capacity(2);
    candidates.push(dir.join("manifest.txt"));
    if let Some(parent) = dir.parent() {
        candidates.push(parent.join("manifest.txt"));
    }
    let Some(s) = candidates
        .into_iter()
        .find_map(|p| std::fs::read_to_string(&p).ok())
    else {
        return false;
    };
    let feature = feature.trim().to_ascii_lowercase();
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("features=") {
            return rest
                .split(',')
                .map(|f| f.trim().to_ascii_lowercase())
                .any(|f| f == feature);
        }
    }
    false
}

fn try_link_prebuilt(dir: PathBuf, target_env: &str) -> bool {
    let lib_name = expected_lib_name(target_env);
    let lib_path = dir.join(lib_name);
    if !lib_path.exists() {
        return false;
    }
    if !prebuilt_manifest_has_feature(&dir, "wchar32") {
        return false;
    }
    println!("cargo:rustc-link-search=native={}", dir.display());
    println!("cargo:rustc-link-lib=static=dear_implot3d");
    true
}

fn prebuilt_cache_root(cfg: &BuildConfig) -> PathBuf {
    build_support::prebuilt_cache_root_from_env_or_target(
        &cfg.manifest_dir,
        "IMPLOT3D_SYS_CACHE_DIR",
        "dear-implot3d-prebuilt",
    )
}

fn try_download_prebuilt(
    cache_root: &Path,
    url: &str,
    target_env: &str,
) -> Result<PathBuf, String> {
    let lib_name = expected_lib_name(target_env);
    if is_http_url(url) {
        println!(
            "cargo:warning=Downloading prebuilt dear_implot3d from {}",
            url
        );
    } else {
        println!("cargo:warning=Using prebuilt dear_implot3d from {}", url);
    }
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
    let target = env::var("TARGET").unwrap_or_default();
    let archive_name = build_support::compose_archive_name(
        "dear-implot3d",
        &version,
        &target,
        link_type,
        None,
        crt,
    );
    let archive_name_no_crt = build_support::compose_archive_name(
        "dear-implot3d",
        &version,
        &target,
        link_type,
        None,
        "",
    );
    let tags = build_support::release_tags("dear-implot3d-sys", &version);
    if let Ok(pkg_dir) = env::var("IMPLOT3D_SYS_PACKAGE_DIR") {
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

fn try_link_prebuilt_all(cfg: &BuildConfig) -> bool {
    let target_env = &cfg.target_env;
    if let Ok(dir) = env::var("IMPLOT3D_SYS_LIB_DIR") {
        if try_link_prebuilt(PathBuf::from(dir.clone()), target_env) {
            return true;
        }
        println!(
            "cargo:warning=IMPLOT3D_SYS_LIB_DIR set but library not found in {}",
            dir
        );
    }
    if let Ok(url) = env::var("IMPLOT3D_SYS_PREBUILT_URL") {
        if (is_http_url(&url) || is_archive_urlish(&url)) && !cfg!(feature = "prebuilt") {
            println!(
                "cargo:warning=IMPLOT3D_SYS_PREBUILT_URL is an HTTP(S) URL or a .tar.gz archive, but feature `prebuilt` is disabled; \
                 enable it to allow downloads/extraction (e.g. `cargo build -p dear-implot3d-sys --features prebuilt`) \
                 or use IMPLOT3D_SYS_LIB_DIR instead."
            );
            return false;
        }
        let cache_root = prebuilt_cache_root(cfg);
        if let Ok(dir) = try_download_prebuilt(&cache_root, &url, target_env)
            && try_link_prebuilt(dir.clone(), target_env)
        {
            return true;
        }
    } else {
        let allow_feature = cfg!(feature = "prebuilt");
        let allow_env = matches!(
            env::var("IMPLOT3D_SYS_USE_PREBUILT").ok().as_deref(),
            Some("1") | Some("true") | Some("yes")
        );
        if allow_env && !allow_feature {
            println!(
                "cargo:warning=IMPLOT3D_SYS_USE_PREBUILT is set, but feature `prebuilt` is disabled; \
                 downloads are unavailable without enabling the feature (e.g. `cargo build -p dear-implot3d-sys --features prebuilt`)."
            );
        }
        let allow_auto_prebuilt = allow_feature;
        if allow_auto_prebuilt {
            let source = match (allow_feature, allow_env) {
                (true, true) => "feature+env",
                (true, false) => "feature",
                _ => "",
            };
            let (owner, repo) = build_support::release_owner_repo();
            println!(
                "cargo:warning=auto-prebuilt enabled (dear-implot3d-sys): source={}, repo={}/{}",
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

fn build_with_cc(cfg: &BuildConfig, cimplot3d_root: &Path, imgui_src: &Path, cimgui_root: &Path) {
    let cimplot3d_cpp = cimplot3d_root.join("cimplot3d.cpp");

    let mut build = cc::Build::new();
    if cfg.target_arch == "wasm32" {
        build.define("IMGUI_DISABLE_DEFAULT_SHELL_FUNCTIONS", "1");
    } else {
        build.cpp(true).std("c++17");
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
        let profile = env::var("PROFILE").unwrap_or_else(|_| "release".to_string());
        if profile == "debug" {
            build.debug(true).opt_level(0);
        } else {
            build.debug(false).opt_level(2);
        }
        build.flag("/D_ITERATOR_DEBUG_LEVEL=0");
    }

    for (k, v) in env::vars() {
        let suffix = k
            .strip_prefix("DEP_DEAR_IMGUI_SYS_DEFINE_")
            .or_else(|| k.strip_prefix("DEP_DEAR_IMGUI_DEFINE_"));
        if let Some(suffix) = suffix {
            build.define(suffix, v.as_str());
        }
    }

    build.define("IMGUI_DEFINE_MATH_OPERATORS", Some("1"));
    build.define("IMGUI_USE_WCHAR32", None);
    build.include(imgui_src);
    build.include(cimgui_root);
    build.include(cimplot3d_root);
    build.include(cimplot3d_root.join("implot3d"));

    build.file(cimplot3d_cpp);
    build.file(cimplot3d_root.join("implot3d/implot3d.cpp"));
    build.file(cimplot3d_root.join("implot3d/implot3d_items.cpp"));
    build.file(cimplot3d_root.join("implot3d/implot3d_meshes.cpp"));
    build.file(cimplot3d_root.join("implot3d/implot3d_demo.cpp"));

    build.compile("dear_implot3d");
}

fn docsrs_build(cfg: &BuildConfig, cimplot3d_root: &Path, imgui_src: &Path, cimgui_root: &Path) {
    println!("cargo:warning=DOCS_RS detected: generating bindings, skipping native build");
    println!("cargo:rustc-cfg=docsrs");
    if use_pregenerated_bindings(&cfg.out_dir) {
        return;
    }
    if !imgui_src.exists() || !cimgui_root.exists() || !cimplot3d_root.exists() {
        panic!(
            "DOCS_RS build: Required headers not found and no pregenerated bindings present.\n\
             Please add src/bindings_pregenerated.rs (full bindgen output) to enable docs.rs builds.\n\
             Run: cargo build -p dear-implot3d-sys && cp target/debug/build/dear-implot3d-sys-*/out/bindings.rs extensions/dear-implot3d-sys/src/bindings_pregenerated.rs"
        );
    }
    generate_bindings(cfg, cimplot3d_root, imgui_src, cimgui_root);
    println!("cargo:IMGUI_INCLUDE_PATH={}", imgui_src.display());
    println!("cargo:CIMGUI_INCLUDE_PATH={}", cimgui_root.display());
}

fn main() {
    let cfg = BuildConfig::new();
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=third-party/cimplot3d/cimplot3d.h");
    println!("cargo:rerun-if-changed=third-party/cimplot3d/cimplot3d.cpp");
    println!("cargo:rerun-if-changed=third-party/cimplot3d/implot3d/implot3d.h");
    println!("cargo:rerun-if-changed=third-party/cimplot3d/implot3d/implot3d.cpp");
    println!("cargo:rerun-if-changed=third-party/cimplot3d/implot3d/implot3d_items.cpp");
    println!("cargo:rerun-if-changed=third-party/cimplot3d/implot3d/implot3d_meshes.cpp");
    println!("cargo:rerun-if-changed=third-party/cimplot3d/implot3d/implot3d_demo.cpp");
    println!("cargo:rerun-if-changed=../../dear-imgui-sys");
    println!("cargo:rerun-if-env-changed=IMPLOT3D_SYS_LIB_DIR");
    println!("cargo:rerun-if-env-changed=IMPLOT3D_SYS_SKIP_CC");
    println!("cargo:rerun-if-env-changed=IMPLOT3D_SYS_PREBUILT_URL");
    println!("cargo:rerun-if-env-changed=IMPLOT3D_SYS_FORCE_BUILD");

    let (imgui_src, cimgui_root) = resolve_imgui_includes(&cfg);
    let cimplot3d_root = cfg.manifest_dir.join("third-party/cimplot3d");

    if cfg.docs_rs {
        docsrs_build(&cfg, &cimplot3d_root, &imgui_src, &cimgui_root);
        return;
    }

    if !imgui_src.exists() {
        panic!("ImGui source not found at {:?}", imgui_src);
    }
    if !cimplot3d_root.exists() {
        panic!(
            "cimplot3d root not found at {:?}. Did you init submodules?",
            cimplot3d_root
        );
    }

    generate_bindings(&cfg, &cimplot3d_root, &imgui_src, &cimgui_root);

    let force_build =
        cfg!(feature = "build-from-source") || env::var("IMPLOT3D_SYS_FORCE_BUILD").is_ok();
    let linked_prebuilt = if force_build {
        false
    } else {
        try_link_prebuilt_all(&cfg)
    };
    if cfg.target_arch != "wasm32" {
        if !cfg.docs_rs && !linked_prebuilt && env::var("IMPLOT3D_SYS_SKIP_CC").is_err() {
            build_with_cc(&cfg, &cimplot3d_root, &imgui_src, &cimgui_root);
        }
    } else {
        println!(
            "cargo:warning=Skipping native ImPlot3D build for wasm32 (using import-style wasm bindings)"
        );
    }
}
