use std::{
    env,
    path::{Path, PathBuf},
};

fn is_http_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn is_archive_urlish(value: &str) -> bool {
    value.ends_with(".tar.gz") || value.ends_with(".tgz")
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
    if cfg!(feature = "stack-layout") {
        features.push("stack-layout");
    }
    let target = env::var("TARGET").unwrap_or_default();
    let crt = if cfg.is_windows() && cfg.is_msvc() {
        if cfg.use_static_crt() { "mt" } else { "md" }
    } else {
        ""
    };
    build_support::binding::extension_artifact_profile_from_env(
        build_support::binding::ExtensionBinding::NodeEditor,
        &cfg.manifest_dir,
        env!("CARGO_PKG_VERSION"),
        &target,
        crt,
        &features,
        package_mode,
    )
    .unwrap_or_else(|error| panic!("dear-node-editor-sys: {error}"))
}

fn panic_wasm_unsupported() -> ! {
    panic!(
        "dear-node-editor-sys is native-only in this integration phase. \
         wasm32 support needs a complete cimnodes_editor/imgui-node-editor integration for \
         this workspace's import-style imgui-sys-v0 WASM provider: pregenerated wasm bindings, \
         provider exports, Emscripten source wiring, and web demo/smoke coverage. \
         Use dear-imnodes for the current wasm node-editor path."
    );
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

fn native_binding_spec() -> &'static build_support::binding::CrateBindingSpec {
    build_support::binding::CrateBindingSpec::for_crate_and_target(env!("CARGO_PKG_NAME"), "native")
        .expect("missing dear-node-editor-sys native binding spec")
}

fn maintained_source_paths(
    cfg: &BuildConfig,
) -> build_support::source_inventory::MaintainedSourcePaths {
    build_support::source_inventory::MaintainedSourcePaths::for_crate(
        env!("CARGO_PKG_NAME"),
        cfg.manifest_dir.clone(),
    )
    .unwrap_or_else(|error| panic!("dear-node-editor-sys: {error}"))
}

#[cfg(feature = "bindgen")]
fn apply_bindgen_defines(mut builder: bindgen::Builder) -> bindgen::Builder {
    for define in native_binding_spec().binding_defines() {
        builder = builder.clang_arg(define.clang_arg());
    }
    builder
}

#[cfg(feature = "bindgen")]
fn generate_bindings(
    cfg: &BuildConfig,
    node_editor_root: &Path,
    imgui_src: &Path,
    cimgui_root: &Path,
) {
    if cfg.target_arch == "wasm32" {
        panic_wasm_unsupported();
    }

    let builder = bindgen::Builder::default()
        .header(
            cfg.manifest_dir
                .join("shim/node_editor_extra.h")
                .to_string_lossy(),
        )
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .allowlist_recursively(false)
        .allowlist_function("dne_.*")
        .allowlist_type("Dne.*")
        .allowlist_var("DNE_.*")
        .blocklist_type("Im.*")
        .derive_default(true)
        .derive_debug(true)
        .derive_copy(true)
        .derive_eq(false)
        .derive_partialeq(false)
        .derive_hash(false)
        .prepend_enum_name(false)
        .layout_tests(false)
        .clang_arg(format!("-I{}", cimgui_root.display()))
        .clang_arg(format!("-I{}", imgui_src.display()))
        .clang_arg(format!("-I{}", node_editor_root.display()))
        .clang_arg(format!(
            "-I{}",
            node_editor_root.join("imgui-node-editor").display()
        ));
    let bindings = apply_bindgen_defines(builder)
        .clang_arg("-x")
        .clang_arg("c++")
        .clang_arg("-std=c++17")
        .generate()
        .expect("Unable to generate dear-node-editor bindings");

    let out = cfg.out_dir.join("bindings.rs");
    bindings
        .write_to_file(&out)
        .expect("Couldn't write dear-node-editor bindings");
    sanitize_bindings_file(&out);
}

#[cfg(not(feature = "bindgen"))]
fn generate_bindings(
    _cfg: &BuildConfig,
    _node_editor_root: &Path,
    _imgui_src: &Path,
    _cimgui_root: &Path,
) {
    panic!(
        "dear-node-editor-sys: regenerating bindings requires the `bindgen` feature. \
         Re-run with `--features bindgen` and DEAR_IMGUI_RS_REGEN_BINDINGS=1."
    );
}

fn use_pregenerated_bindings(out_dir: &Path) -> bool {
    if build_support::parse_bool_env("DEAR_IMGUI_RS_REGEN_BINDINGS") {
        return false;
    }

    let spec = build_support::binding::CrateBindingSpec::for_crate_and_target(
        env!("CARGO_PKG_NAME"),
        "native",
    )
    .expect("missing node-editor native binding spec");
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let preg = crate_root.join(spec.checked_in_path);
    if !preg.exists() {
        return false;
    }
    spec.copy_embedded_checked_in_to_out_dir(&crate_root, out_dir)
        .unwrap_or_else(|error| panic!("invalid pregenerated node-editor bindings: {error}"));
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

fn expected_lib_name(target_env: &str) -> String {
    build_support::expected_lib_name(target_env, "dear_node_editor")
}

fn try_link_prebuilt(dir: PathBuf, cfg: &BuildConfig) -> bool {
    let lib_name = expected_lib_name(&cfg.target_env);
    if !dir.join(&lib_name).exists() {
        return false;
    }
    extension_artifact_profile(cfg, false)
        .validate_prebuilt_dir(&dir)
        .unwrap_or_else(|error| {
            panic!("dear-node-editor-sys: incompatible prebuilt artifact: {error}")
        });
    println!("cargo:rustc-link-search=native={}", dir.display());
    println!("cargo:rustc-link-lib=static=dear_node_editor");
    true
}

fn try_link_prebuilt_all(cfg: &BuildConfig) -> bool {
    let target_env = &cfg.target_env;
    if let Ok(dir) = env::var("NODE_EDITOR_SYS_LIB_DIR") {
        if try_link_prebuilt(PathBuf::from(dir.clone()), cfg) {
            return true;
        }
        println!(
            "cargo:warning=NODE_EDITOR_SYS_LIB_DIR set but library not found in {}",
            dir
        );
    }
    if let Ok(url) = env::var("NODE_EDITOR_SYS_PREBUILT_URL") {
        if !cfg!(feature = "prebuilt") && (is_http_url(&url) || is_archive_urlish(&url)) {
            println!(
                "cargo:warning=NODE_EDITOR_SYS_PREBUILT_URL is an HTTP(S) URL or archive, but feature `prebuilt` is disabled; enable it for downloads/extraction or use NODE_EDITOR_SYS_LIB_DIR"
            );
            return false;
        }
        let cache_root = prebuilt_cache_root(cfg);
        if let Ok(dir) = build_support::download_prebuilt(
            &cache_root,
            &url,
            &expected_lib_name(target_env),
            target_env,
        ) && try_link_prebuilt(dir, cfg)
        {
            return true;
        }
    } else {
        let allow_feature = cfg!(feature = "prebuilt");
        let allow_env = matches!(
            env::var("NODE_EDITOR_SYS_USE_PREBUILT").ok().as_deref(),
            Some("1") | Some("true") | Some("yes")
        );
        if allow_env && !allow_feature {
            println!(
                "cargo:warning=NODE_EDITOR_SYS_USE_PREBUILT is set, but feature `prebuilt` is disabled"
            );
        }
        if allow_feature {
            let source = if allow_env { "feature+env" } else { "feature" };
            let (owner, repo) = build_support::release_owner_repo();
            println!(
                "cargo:warning=auto-prebuilt enabled (dear-node-editor-sys): source={}, repo={}/{}",
                source, owner, repo
            );
            if let Some(dir) = try_download_prebuilt_from_release(cfg)
                && try_link_prebuilt(dir, cfg)
            {
                return true;
            }
        }
    }
    false
}

fn prebuilt_cache_root(cfg: &BuildConfig) -> PathBuf {
    build_support::prebuilt_cache_root_from_env_or_target(
        &cfg.manifest_dir,
        "NODE_EDITOR_SYS_CACHE_DIR",
        "dear-node-editor-prebuilt",
    )
    .join(extension_artifact_profile(cfg, false).cache_key())
}

fn try_download_prebuilt_from_release(cfg: &BuildConfig) -> Option<PathBuf> {
    let profile = extension_artifact_profile(cfg, false);
    let tags = build_support::release_tags("dear-node-editor-sys", &profile.version);
    if let Ok(package_dir) = env::var("NODE_EDITOR_SYS_PACKAGE_DIR") {
        let archive_path = PathBuf::from(package_dir).join(&profile.archive_name);
        if archive_path.exists() {
            let cache_root = prebuilt_cache_root(cfg);
            if let Ok(lib_dir) = build_support::extract_archive_to_cache(
                &archive_path,
                &cache_root,
                &expected_lib_name(&cfg.target_env),
            ) {
                return Some(lib_dir);
            }
        }
    }
    if build_support::is_offline() {
        return None;
    }
    let urls = build_support::release_candidate_urls_env(&tags, &[profile.archive_name]);
    let cache_root = prebuilt_cache_root(cfg);
    let lib_name = expected_lib_name(&cfg.target_env);
    for url in urls {
        if let Ok(dir) =
            build_support::download_prebuilt(&cache_root, &url, &lib_name, &cfg.target_env)
        {
            return Some(dir);
        }
    }
    None
}

fn build_with_cc(
    cfg: &BuildConfig,
    sources: &build_support::source_inventory::MaintainedSourcePaths,
    node_editor_root: &Path,
    imgui_src: &Path,
    cimgui_root: &Path,
) {
    let mut build = cc::Build::new();
    build.cpp(true).std("c++17");
    build_support::configure_cpp_runtime_linkage(&mut build, &cfg.target_os, &cfg.target_env);
    native_binding_spec().apply_extension_binding_defines(&mut build, env::vars());
    build.include(imgui_src);
    build.include(cimgui_root);
    build.include(node_editor_root);
    build.include(node_editor_root.join("imgui-node-editor"));
    build.include(cfg.manifest_dir.join("shim"));
    // Keep ImGui internal ABI macros in lockstep with dear-imgui-sys. imgui-node-editor includes
    // imgui_internal.h, so local-only layout-affecting defines can corrupt the shared context.

    for file_id in ["wrapper", "core", "api", "canvas", "json", "shim"] {
        build.file(
            sources
                .file(file_id)
                .unwrap_or_else(|error| panic!("dear-node-editor-sys: {error}")),
        );
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

    build.compile("dear_node_editor");
}

fn main() {
    let cfg = BuildConfig::new();
    let sources = maintained_source_paths(&cfg);

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/bindings_pregenerated.rs");
    println!("cargo:rerun-if-changed=shim/node_editor_extra.h");
    println!("cargo:rerun-if-changed=third-party/cimnodes_editor/cimnodes_editor.h");
    println!(
        "cargo:rerun-if-changed=third-party/cimnodes_editor/imgui-node-editor/imgui_node_editor.h"
    );
    for path in sources.native_candidate_paths() {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    println!("cargo:rerun-if-env-changed=NODE_EDITOR_SYS_LIB_DIR");
    println!("cargo:rerun-if-env-changed=NODE_EDITOR_SYS_SKIP_CC");
    println!("cargo:rerun-if-env-changed=NODE_EDITOR_SYS_PREBUILT_URL");
    println!("cargo:rerun-if-env-changed=NODE_EDITOR_SYS_FORCE_BUILD");
    println!("cargo:rerun-if-env-changed=NODE_EDITOR_SYS_CACHE_DIR");
    println!("cargo:rerun-if-env-changed=NODE_EDITOR_SYS_USE_PREBUILT");
    println!("cargo:rerun-if-env-changed=NODE_EDITOR_SYS_PACKAGE_DIR");
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
            .unwrap_or_else(|error| panic!("dear-node-editor-sys: {error}"));
        println!(
            "cargo:rustc-env=DEAR_IMGUI_EXTENSION_ARTIFACT_TARGET={}",
            profile.target
        );
        println!(
            "cargo:rustc-env=DEAR_IMGUI_EXTENSION_ARTIFACT_CRT={}",
            profile.crt
        );
    }

    if cfg.docs_rs {
        println!(
            "cargo:warning=DOCS_RS detected: using pregenerated bindings, skipping native build"
        );
        println!("cargo:rustc-cfg=docsrs");
        if !use_pregenerated_bindings(&cfg.out_dir) {
            panic!("DOCS_RS build requires src/bindings_pregenerated.rs");
        }
        return;
    }

    let regen_bindings = build_support::parse_bool_env("DEAR_IMGUI_RS_REGEN_BINDINGS");
    if !regen_bindings && env::var("NODE_EDITOR_SYS_SKIP_CC").is_ok() {
        if !use_pregenerated_bindings(&cfg.out_dir) {
            panic!("NODE_EDITOR_SYS_SKIP_CC is set but no pregenerated bindings were found");
        }
        let _ = try_link_prebuilt_all(&cfg);
        return;
    }

    let (imgui_src, cimgui_root) = resolve_imgui_includes(&cfg);
    let node_editor_root = sources
        .source_root()
        .unwrap_or_else(|error| panic!("dear-node-editor-sys: {error}"));

    if !imgui_src.exists() {
        panic!("ImGui include not found at {:?}", imgui_src);
    }
    if !node_editor_root.exists() {
        panic!(
            "cimnodes_editor root not found at {:?}. Did you init submodules?",
            node_editor_root
        );
    }

    if regen_bindings {
        generate_bindings(&cfg, &node_editor_root, &imgui_src, &cimgui_root);
        return;
    }

    if cfg.target_arch == "wasm32" {
        panic_wasm_unsupported();
    }

    let bindings_ready = use_pregenerated_bindings(&cfg.out_dir);
    if !bindings_ready {
        generate_bindings(&cfg, &node_editor_root, &imgui_src, &cimgui_root);
    }

    let force_build = cfg!(feature = "package-bin")
        || cfg!(feature = "build-from-source")
        || env::var("NODE_EDITOR_SYS_FORCE_BUILD").is_ok();
    let linked = if force_build {
        false
    } else {
        try_link_prebuilt_all(&cfg)
    };
    if !linked {
        sources
            .validate_native()
            .unwrap_or_else(|error| panic!("dear-node-editor-sys: {error}"));
        build_with_cc(&cfg, &sources, &node_editor_root, &imgui_src, &cimgui_root);
    }
}
