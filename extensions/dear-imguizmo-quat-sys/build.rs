use std::{env, path::Path, path::PathBuf};

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

fn extension_artifact_profile(
    cfg: &BuildConfig,
    package_mode: bool,
) -> build_support::binding::ExtensionArtifactProfile {
    let mut features = vec!["wchar32"];
    if cfg!(feature = "freetype") {
        features.push("freetype");
    }
    let target = env::var("TARGET").unwrap_or_default();
    let crt = if cfg.is_windows() && cfg.is_msvc() {
        if cfg.use_static_crt() { "mt" } else { "md" }
    } else {
        ""
    };
    build_support::binding::extension_artifact_profile_from_env(
        build_support::binding::ExtensionBinding::ImGuizmoQuat,
        &cfg.manifest_dir,
        env!("CARGO_PKG_VERSION"),
        &target,
        crt,
        &features,
        package_mode,
    )
    .unwrap_or_else(|error| panic!("dear-imguizmo-quat-sys: {error}"))
}

fn resolve_imgui_includes(cfg: &BuildConfig) -> (PathBuf, PathBuf) {
    // Prefer paths exported by dear-imgui-sys build script (prefix comes from links = "dear-imgui")
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

fn native_binding_spec() -> &'static build_support::binding::CrateBindingSpec {
    build_support::binding::CrateBindingSpec::for_crate_and_target(env!("CARGO_PKG_NAME"), "native")
        .expect("missing dear-imguizmo-quat-sys native binding spec")
}

fn maintained_source_paths(
    cfg: &BuildConfig,
) -> build_support::source_inventory::MaintainedSourcePaths {
    build_support::source_inventory::MaintainedSourcePaths::for_crate(
        env!("CARGO_PKG_NAME"),
        cfg.manifest_dir.clone(),
    )
    .unwrap_or_else(|error| panic!("dear-imguizmo-quat-sys: {error}"))
}

#[cfg(feature = "bindgen")]
fn apply_bindgen_defines(mut builder: bindgen::Builder) -> bindgen::Builder {
    for define in native_binding_spec().binding_defines() {
        builder = builder.clang_arg(define.clang_arg());
    }
    builder
}

fn use_pregenerated_bindings(out_dir: &Path) -> bool {
    use_validated_pregenerated_bindings(out_dir, "native")
}

fn use_pregenerated_wasm_bindings(out_dir: &Path) -> bool {
    use_validated_pregenerated_bindings(out_dir, "wasm")
}

fn use_validated_pregenerated_bindings(out_dir: &Path, target: &str) -> bool {
    if build_support::parse_bool_env("DEAR_IMGUI_RS_REGEN_BINDINGS") {
        return false;
    }

    let spec = build_support::binding::CrateBindingSpec::for_crate_and_target(
        env!("CARGO_PKG_NAME"),
        target,
    )
    .unwrap_or_else(|| panic!("missing {} {target} binding spec", env!("CARGO_PKG_NAME")));
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let preg = crate_root.join(spec.checked_in_path);
    if !preg.exists() {
        return false;
    }
    spec.copy_embedded_checked_in_to_out_dir(&crate_root, out_dir)
        .unwrap_or_else(|error| panic!("invalid pregenerated bindings: {error}"));
    println!(
        "cargo:warning=Using validated pregenerated bindings: {}",
        preg.display()
    );
    true
}

#[cfg(feature = "bindgen")]
fn sanitize_bindings_file(path: &Path) {
    if let Ok(content) = std::fs::read_to_string(path) {
        let sanitized = sanitize_bindings_string(&content);
        let _ = std::fs::write(path, sanitized);
    }
}

#[cfg(feature = "bindgen")]
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

#[cfg(feature = "bindgen")]
fn generate_bindings(cfg: &BuildConfig, quat_root: &Path, imgui_src: &Path, cimgui_root: &Path) {
    // For wasm32 targets, rely on pregenerated import-style bindings that import
    // from the shared imgui-sys-v0 provider instead of running bindgen here.
    if cfg.target_arch == "wasm32" {
        if !cfg!(feature = "wasm") {
            panic!(
                "dear-imguizmo-quat-sys: building for wasm32 requires the `wasm` feature.\n\
                 Enable it in your Cargo.toml: features = [\"wasm\"]"
            );
        }
        if use_pregenerated_wasm_bindings(&cfg.out_dir) {
            println!("cargo:warning=Using pregenerated wasm bindings for dear-imguizmo-quat-sys");
            return;
        }
        panic!(
            "dear-imguizmo-quat-sys: wasm32 target detected but src/wasm_bindings_pregenerated.rs not found.\n\
             Run: cargo run -p xtask -- wasm-bindgen-imguizmo-quat"
        );
    }

    let header = quat_root.join("cimguizmo_quat.h");
    let imguizmo_quat_inc = quat_root.join("imGuIZMO.quat").join("imguizmo_quat");

    let builder = bindgen::Builder::default()
        .header(header.to_string_lossy())
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .allowlist_function("imguiGizmo_.*")
        .allowlist_function("iggizmo3D_.*")
        .allowlist_function("(mat4|quat)_.*")
        .blocklist_type("ImVec2")
        .blocklist_type("ImDrawList")
        .blocklist_type("ImGuiContext")
        .blocklist_type("ImGuiID")
        .blocklist_type("ImVec4")
        .clang_arg(format!("-I{}", cimgui_root.display()))
        .clang_arg(format!("-I{}", imgui_src.display()))
        .clang_arg(format!("-I{}", quat_root.display()))
        .clang_arg(format!("-I{}", imguizmo_quat_inc.display()));
    let bindings = apply_bindgen_defines(builder)
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
        .expect("Unable to generate cimguizmo_quat bindings");
    let out = cfg.out_dir.join("bindings.rs");
    bindings
        .write_to_file(&out)
        .expect("Couldn't write cimguizmo_quat bindings!");
    sanitize_bindings_file(&out);
}

#[cfg(not(feature = "bindgen"))]
fn generate_bindings(
    _cfg: &BuildConfig,
    _quat_root: &Path,
    _imgui_src: &Path,
    _cimgui_root: &Path,
) {
    panic!(
        "dear-imguizmo-quat-sys: regenerating bindings requires the `bindgen` feature. \
         Re-run with `--features bindgen` and DEAR_IMGUI_RS_REGEN_BINDINGS=1."
    );
}

fn docsrs_build(cfg: &BuildConfig, quat_root: &Path, imgui_src: &Path, cimgui_root: &Path) {
    println!("cargo:warning=DOCS_RS detected: generating bindings, skipping native build");
    println!("cargo:rustc-cfg=docsrs");

    if use_pregenerated_bindings(&cfg.out_dir) {
        return;
    }

    // Fallback: try to generate bindings from headers if available
    if !imgui_src.exists() || !cimgui_root.exists() || !quat_root.exists() {
        panic!(
            "DOCS_RS build: Required headers not found and no pregenerated bindings present.\n\
             Please add src/bindings_pregenerated.rs (full bindgen output) to enable docs.rs builds.\n\
             Run: cargo build -p dear-imguizmo-quat-sys && cp target/debug/build/dear-imguizmo-quat-sys-*/out/bindings.rs extensions/dear-imguizmo-quat-sys/src/bindings_pregenerated.rs"
        );
    }

    generate_bindings(cfg, quat_root, imgui_src, cimgui_root);
}

fn expected_lib_name(target_env: &str) -> &'static str {
    if target_env == "msvc" {
        "dear_imguizmo_quat.lib"
    } else {
        "libdear_imguizmo_quat.a"
    }
}

fn try_link_prebuilt(dir: PathBuf, cfg: &BuildConfig) -> bool {
    let lib_name = expected_lib_name(&cfg.target_env);
    let lib_path = dir.join(lib_name);
    if !lib_path.exists() {
        return false;
    }
    extension_artifact_profile(cfg, false)
        .validate_prebuilt_dir(&dir)
        .unwrap_or_else(|error| {
            panic!("dear-imguizmo-quat-sys: incompatible prebuilt artifact: {error}")
        });
    println!("cargo:rustc-link-search=native={}", dir.display());
    println!("cargo:rustc-link-lib=static=dear_imguizmo_quat");
    true
}

fn prebuilt_cache_root(cfg: &BuildConfig) -> PathBuf {
    build_support::prebuilt_cache_root_from_env_or_target(
        &cfg.manifest_dir,
        "IMGUIZMO_QUAT_SYS_CACHE_DIR",
        "dear-imguizmo-quat-prebuilt",
    )
    .join(extension_artifact_profile(cfg, false).cache_key())
}

fn try_download_prebuilt(
    cache_root: &Path,
    url: &str,
    target_env: &str,
) -> Result<PathBuf, String> {
    let lib_name = expected_lib_name(target_env);
    if is_http_url(url) {
        println!(
            "cargo:warning=Downloading prebuilt dear_imguizmo_quat from {}",
            url
        );
    } else {
        println!(
            "cargo:warning=Using prebuilt dear_imguizmo_quat from {}",
            url
        );
    }
    build_support::download_prebuilt(cache_root, url, lib_name, target_env)
}

fn try_download_prebuilt_from_release(cfg: &BuildConfig) -> Option<PathBuf> {
    let profile = extension_artifact_profile(cfg, false);
    let tags = build_support::release_tags("dear-imguizmo-quat-sys", &profile.version);
    if let Ok(pkg_dir) = env::var("IMGUIZMO_QUAT_SYS_PACKAGE_DIR") {
        let pkg_dir = PathBuf::from(pkg_dir);
        let archive_path = pkg_dir.join(&profile.archive_name);
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
    if build_support::is_offline() {
        return None;
    }
    let cache_root = prebuilt_cache_root(cfg);
    let urls = build_support::release_candidate_urls_env(&tags, &[profile.archive_name]);
    for url in urls {
        if let Ok(lib_dir) = try_download_prebuilt(&cache_root, &url, &cfg.target_env) {
            return Some(lib_dir);
        }
    }
    None
}

fn try_link_prebuilt_all(cfg: &BuildConfig) -> bool {
    let target_env = &cfg.target_env;
    if let Ok(dir) = env::var("IMGUIZMO_QUAT_SYS_LIB_DIR") {
        if try_link_prebuilt(PathBuf::from(dir.clone()), cfg) {
            return true;
        }
        println!(
            "cargo:warning=IMGUIZMO_QUAT_SYS_LIB_DIR set but library not found in {}",
            dir
        );
    }
    if let Ok(url) = env::var("IMGUIZMO_QUAT_SYS_PREBUILT_URL") {
        if (is_http_url(&url) || is_archive_urlish(&url)) && !cfg!(feature = "prebuilt") {
            println!(
                "cargo:warning=IMGUIZMO_QUAT_SYS_PREBUILT_URL is an HTTP(S) URL or a .tar.gz archive, but feature `prebuilt` is disabled; \
                 enable it to allow downloads/extraction (e.g. `cargo build -p dear-imguizmo-quat-sys --features prebuilt`) \
                 or use IMGUIZMO_QUAT_SYS_LIB_DIR instead."
            );
            return false;
        }
        let cache_root = prebuilt_cache_root(cfg);
        if let Ok(dir) = try_download_prebuilt(&cache_root, &url, target_env)
            && try_link_prebuilt(dir.clone(), cfg)
        {
            return true;
        }
    } else {
        // Only attempt automatic release download when explicitly enabled.
        let allow_feature = cfg!(feature = "prebuilt");
        let allow_env = matches!(
            env::var("IMGUIZMO_QUAT_SYS_USE_PREBUILT").ok().as_deref(),
            Some("1") | Some("true") | Some("yes")
        );
        if allow_env && !allow_feature {
            println!(
                "cargo:warning=IMGUIZMO_QUAT_SYS_USE_PREBUILT is set, but feature `prebuilt` is disabled; \
                 downloads are unavailable without enabling the feature (e.g. `cargo build -p dear-imguizmo-quat-sys --features prebuilt`)."
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
                "cargo:warning=auto-prebuilt enabled (dear-imguizmo-quat-sys): source={}, repo={}/{}",
                source, owner, repo
            );
            if let Some(dir) = try_download_prebuilt_from_release(cfg)
                && try_link_prebuilt(dir.clone(), cfg)
            {
                return true;
            }
        }
    }
    false
}

fn build_with_cc(
    cfg: &BuildConfig,
    sources: &build_support::source_inventory::MaintainedSourcePaths,
    quat_root: &Path,
    imgui_src: &Path,
    cimgui_root: &Path,
) {
    let imguizmo_quat_inc = quat_root.join("imGuIZMO.quat").join("imguizmo_quat");

    let mut build = cc::Build::new();
    build.cpp(true).std("c++17");
    build_support::configure_cpp_runtime_linkage(&mut build, &cfg.target_os, &cfg.target_env);
    native_binding_spec().apply_extension_binding_defines(&mut build, env::vars());
    build.include(imgui_src);
    build.include(cimgui_root);
    build.include(quat_root);
    build.include(&imguizmo_quat_inc);

    // cimguizmo_quat wrapper
    build.file(
        sources
            .file("wrapper")
            .unwrap_or_else(|error| panic!("dear-imguizmo-quat-sys: {error}")),
    );
    // The upstream forwarding translation unit includes imGuIZMOquat.cpp itself.
    // Compiling that implementation separately violates the one-definition rule and fails
    // deterministic provider links even when native archive extraction happens to mask it.
    build.file(
        sources
            .file("core")
            .unwrap_or_else(|error| panic!("dear-imguizmo-quat-sys: {error}")),
    );

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
    build.compile("dear_imguizmo_quat");
}

fn main() {
    let cfg = BuildConfig::new();
    let sources = maintained_source_paths(&cfg);

    // Rerun hints
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/bindings_pregenerated.rs");
    println!("cargo:rerun-if-changed=src/wasm_bindings_pregenerated.rs");
    println!("cargo:rerun-if-changed=third-party/cimguizmo_quat/cimguizmo_quat.h");
    for path in sources.native_candidate_paths() {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    println!("cargo:rerun-if-changed=../../dear-imgui-sys");
    println!("cargo:rerun-if-env-changed=IMGUIZMO_QUAT_SYS_LIB_DIR");
    println!("cargo:rerun-if-env-changed=IMGUIZMO_QUAT_SYS_SKIP_CC");
    println!("cargo:rerun-if-env-changed=IMGUIZMO_QUAT_SYS_PREBUILT_URL");
    println!("cargo:rerun-if-env-changed=IMGUIZMO_QUAT_SYS_FORCE_BUILD");
    println!("cargo:rerun-if-env-changed=IMGUIZMO_QUAT_SYS_CACHE_DIR");
    println!("cargo:rerun-if-env-changed=DEAR_IMGUI_RS_REGEN_BINDINGS");
    println!("cargo:rerun-if-env-changed=DEAR_IMGUI_RS_CANDIDATE_SHA");
    println!("cargo:rerun-if-env-changed=DEAR_IMGUI_CORE_ARTIFACT_IDENTITY_HASH");
    println!("cargo:rerun-if-env-changed=DEP_DEAR_IMGUI_ARTIFACT_IDENTITY_HASH");
    println!("cargo:rerun-if-env-changed=DEP_DEAR_IMGUI_CANDIDATE_SHA");
    println!("cargo:rerun-if-env-changed=DOCS_RS");

    if cfg!(feature = "package-bin") {
        let profile = extension_artifact_profile(&cfg, true);
        profile
            .write_package_metadata(&cfg.out_dir)
            .unwrap_or_else(|error| panic!("dear-imguizmo-quat-sys: {error}"));
        println!(
            "cargo:rustc-env=DEAR_IMGUI_EXTENSION_ARTIFACT_TARGET={}",
            profile.target
        );
        println!(
            "cargo:rustc-env=DEAR_IMGUI_EXTENSION_ARTIFACT_CRT={}",
            profile.crt
        );
    }

    let (imgui_src, cimgui_root) = resolve_imgui_includes(&cfg);
    let quat_root = sources
        .source_root()
        .unwrap_or_else(|error| panic!("dear-imguizmo-quat-sys: {error}"));
    if cfg.docs_rs {
        docsrs_build(&cfg, &quat_root, &imgui_src, &cimgui_root);
        return;
    }

    if build_support::parse_bool_env("DEAR_IMGUI_RS_REGEN_BINDINGS") {
        if !imgui_src.exists() {
            panic!("ImGui include not found at {:?}", imgui_src);
        }
        if !cimgui_root.exists() {
            panic!("cimgui root not found at {:?}", cimgui_root);
        }
        if !quat_root.exists() {
            panic!(
                "cimguizmo_quat root not found at {:?}. Did you init submodules?",
                quat_root
            );
        }
        generate_bindings(&cfg, &quat_root, &imgui_src, &cimgui_root);
        return;
    }

    if env::var("IMGUIZMO_QUAT_SYS_SKIP_CC").is_ok() {
        let ok = if cfg.target_arch == "wasm32" {
            use_pregenerated_wasm_bindings(&cfg.out_dir)
        } else {
            use_pregenerated_bindings(&cfg.out_dir)
        };
        if !ok {
            panic!(
                "IMGUIZMO_QUAT_SYS_SKIP_CC is set but no pregenerated bindings were found. \
                 Please ensure src/bindings_pregenerated.rs exists, or unset IMGUIZMO_QUAT_SYS_SKIP_CC."
            );
        }
        let _ = try_link_prebuilt_all(&cfg);
        return;
    }

    if !imgui_src.exists() {
        panic!("ImGui include not found at {:?}", imgui_src);
    }
    if !cimgui_root.exists() {
        panic!("cimgui root not found at {:?}", cimgui_root);
    }
    if !quat_root.exists() {
        panic!(
            "cimguizmo_quat root not found at {:?}. Did you init submodules?",
            quat_root
        );
    }

    let bindings_ready = if cfg.target_arch == "wasm32" {
        if !cfg!(feature = "wasm") {
            panic!(
                "dear-imguizmo-quat-sys: building for wasm32 requires the `wasm` feature.\n\
                 Enable it in your Cargo.toml: features = [\"wasm\"]"
            );
        }
        use_pregenerated_wasm_bindings(&cfg.out_dir)
    } else {
        use_pregenerated_bindings(&cfg.out_dir)
    };
    if !bindings_ready {
        generate_bindings(&cfg, &quat_root, &imgui_src, &cimgui_root);
    }

    // Link/build native
    let force_build = cfg!(feature = "package-bin")
        || cfg!(feature = "build-from-source")
        || env::var("IMGUIZMO_QUAT_SYS_FORCE_BUILD").is_ok();
    let linked_prebuilt = if force_build {
        false
    } else {
        try_link_prebuilt_all(&cfg)
    };
    if cfg.target_arch != "wasm32" {
        if !cfg.docs_rs && !linked_prebuilt && env::var("IMGUIZMO_QUAT_SYS_SKIP_CC").is_err() {
            sources
                .validate_native()
                .unwrap_or_else(|error| panic!("dear-imguizmo-quat-sys: {error}"));
            build_with_cc(&cfg, &sources, &quat_root, &imgui_src, &cimgui_root);
        }
    } else {
        println!(
            "cargo:warning=Skipping native ImGuIZMO.quat build for wasm32 (using import-style wasm bindings)"
        );
    }
}
