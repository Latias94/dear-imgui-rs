use std::{
    env,
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

#[cfg(feature = "bindgen")]
fn generate_bindings(
    cfg: &BuildConfig,
    node_editor_root: &Path,
    imgui_src: &Path,
    cimgui_root: &Path,
) {
    if cfg.target_arch == "wasm32" {
        panic!("dear-node-editor-sys: wasm32 is not supported in the first native-only release");
    }

    let bindings = bindgen::Builder::default()
        .header(
            cfg.manifest_dir
                .join("shim/node_editor_extra.h")
                .to_string_lossy(),
        )
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .allowlist_function("dne_.*")
        .allowlist_type("Dne.*")
        .allowlist_var("DNE_.*")
        .blocklist_type("ImVec2")
        .blocklist_type("ImVec2_c")
        .blocklist_type("ImVec4")
        .blocklist_type("ImVec4_c")
        .blocklist_type("ImGuiContext")
        .blocklist_type("ImGuiMouseButton")
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
        ))
        .clang_arg("-DIMGUI_USE_WCHAR32")
        .clang_arg("-DCIMGUI_DEFINE_ENUMS_AND_STRUCTS")
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

#[cfg(feature = "bindgen")]
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

fn expected_lib_name(target_env: &str) -> String {
    build_support::expected_lib_name(target_env, "dear_node_editor")
}

fn try_link_prebuilt(dir: PathBuf, target_env: &str) -> bool {
    let lib_name = expected_lib_name(target_env);
    if !dir.join(&lib_name).exists() {
        return false;
    }
    if !build_support::prebuilt_manifest_has_feature(&dir, "wchar32") {
        return false;
    }
    println!("cargo:rustc-link-search=native={}", dir.display());
    println!("cargo:rustc-link-lib=static=dear_node_editor");
    true
}

fn try_link_prebuilt_all(cfg: &BuildConfig) -> bool {
    let target_env = &cfg.target_env;
    if let Ok(dir) = env::var("NODE_EDITOR_SYS_LIB_DIR") {
        if try_link_prebuilt(PathBuf::from(dir.clone()), target_env) {
            return true;
        }
        println!(
            "cargo:warning=NODE_EDITOR_SYS_LIB_DIR set but library not found in {}",
            dir
        );
    }
    if let Ok(url) = env::var("NODE_EDITOR_SYS_PREBUILT_URL") {
        if !cfg!(feature = "prebuilt") && (url.starts_with("http") || url.ends_with(".tar.gz")) {
            println!(
                "cargo:warning=NODE_EDITOR_SYS_PREBUILT_URL needs feature `prebuilt` for downloads or archives"
            );
            return false;
        }
        let cache_root = prebuilt_cache_root(cfg);
        if let Ok(dir) = build_support::download_prebuilt(
            &cache_root,
            &url,
            &expected_lib_name(target_env),
            target_env,
        ) && try_link_prebuilt(dir, target_env)
        {
            return true;
        }
    } else if cfg!(feature = "prebuilt")
        && matches!(
            env::var("NODE_EDITOR_SYS_USE_PREBUILT").ok().as_deref(),
            Some("1") | Some("true") | Some("yes")
        )
    {
        if let Some(dir) = try_download_prebuilt_from_release(cfg)
            && try_link_prebuilt(dir, target_env)
        {
            return true;
        }
    }
    false
}

fn prebuilt_cache_root(cfg: &BuildConfig) -> PathBuf {
    build_support::prebuilt_cache_root_from_env_or_target(
        &cfg.manifest_dir,
        "NODE_EDITOR_SYS_PREBUILT_CACHE",
        "dear-node-editor-sys-prebuilt",
    )
}

fn try_download_prebuilt_from_release(cfg: &BuildConfig) -> Option<PathBuf> {
    if build_support::is_offline() {
        return None;
    }
    let target = env::var("TARGET").ok()?;
    let version = env::var("CARGO_PKG_VERSION").ok()?;
    let crt = build_support::msvc_crt_suffix_from_env(Some(&cfg.target_env)).unwrap_or("");
    let archive = build_support::compose_archive_name(
        "dear-node-editor",
        &version,
        &target,
        "static",
        None,
        crt,
    );
    let tags = build_support::release_tags("dear-node-editor-sys", &version);
    let urls = build_support::release_candidate_urls_env(&tags, &[archive]);
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

fn build_with_cc(cfg: &BuildConfig, node_editor_root: &Path, imgui_src: &Path, cimgui_root: &Path) {
    let mut build = cc::Build::new();
    build.cpp(true).std("c++17");
    for (k, v) in env::vars() {
        if let Some(suffix) = k.strip_prefix("DEP_DEAR_IMGUI_DEFINE_") {
            build.define(suffix, v.as_str());
        }
    }
    build.include(imgui_src);
    build.include(cimgui_root);
    build.include(node_editor_root);
    build.include(node_editor_root.join("imgui-node-editor"));
    build.include(cfg.manifest_dir.join("shim"));
    build.define("IMGUI_USE_WCHAR32", None);
    // Keep ImGui internal ABI macros in lockstep with dear-imgui-sys. imgui-node-editor includes
    // imgui_internal.h, so local-only layout-affecting defines can corrupt the shared context.

    build.file(node_editor_root.join("cimnodes_editor.cpp"));
    build.file(node_editor_root.join("imgui-node-editor/imgui_node_editor.cpp"));
    build.file(node_editor_root.join("imgui-node-editor/imgui_node_editor_api.cpp"));
    build.file(node_editor_root.join("imgui-node-editor/imgui_canvas.cpp"));
    build.file(node_editor_root.join("imgui-node-editor/crude_json.cpp"));
    build.file(cfg.manifest_dir.join("shim/node_editor_extra.cpp"));

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

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/bindings_pregenerated.rs");
    println!("cargo:rerun-if-changed=shim/node_editor_extra.h");
    println!("cargo:rerun-if-changed=shim/node_editor_extra.cpp");
    println!("cargo:rerun-if-changed=third-party/cimnodes_editor/cimnodes_editor.h");
    println!("cargo:rerun-if-changed=third-party/cimnodes_editor/cimnodes_editor.cpp");
    println!(
        "cargo:rerun-if-changed=third-party/cimnodes_editor/imgui-node-editor/imgui_node_editor.h"
    );
    println!(
        "cargo:rerun-if-changed=third-party/cimnodes_editor/imgui-node-editor/imgui_node_editor.cpp"
    );
    println!("cargo:rerun-if-env-changed=NODE_EDITOR_SYS_LIB_DIR");
    println!("cargo:rerun-if-env-changed=NODE_EDITOR_SYS_SKIP_CC");
    println!("cargo:rerun-if-env-changed=NODE_EDITOR_SYS_PREBUILT_URL");
    println!("cargo:rerun-if-env-changed=NODE_EDITOR_SYS_FORCE_BUILD");
    println!("cargo:rerun-if-env-changed=NODE_EDITOR_SYS_USE_PREBUILT");
    println!("cargo:rerun-if-env-changed=DEAR_IMGUI_RS_REGEN_BINDINGS");

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

    let (imgui_src, cimgui_root) = resolve_imgui_includes(&cfg);
    let node_editor_root = cfg.manifest_dir.join("third-party/cimnodes_editor");

    if !imgui_src.exists() {
        panic!("ImGui include not found at {:?}", imgui_src);
    }
    if !node_editor_root.exists() {
        panic!(
            "cimnodes_editor root not found at {:?}. Did you init submodules?",
            node_editor_root
        );
    }

    if build_support::parse_bool_env("DEAR_IMGUI_RS_REGEN_BINDINGS") {
        generate_bindings(&cfg, &node_editor_root, &imgui_src, &cimgui_root);
        return;
    }

    if env::var("NODE_EDITOR_SYS_SKIP_CC").is_ok() {
        if !use_pregenerated_bindings(&cfg.out_dir) {
            panic!("NODE_EDITOR_SYS_SKIP_CC is set but no pregenerated bindings were found");
        }
        let _ = try_link_prebuilt_all(&cfg);
        return;
    }

    if cfg.target_arch == "wasm32" {
        panic!("dear-node-editor-sys: wasm32 is not supported in the first native-only release");
    }

    let bindings_ready = use_pregenerated_bindings(&cfg.out_dir);
    if !bindings_ready {
        generate_bindings(&cfg, &node_editor_root, &imgui_src, &cimgui_root);
    }

    let force_build =
        cfg!(feature = "build-from-source") || env::var("NODE_EDITOR_SYS_FORCE_BUILD").is_ok();
    let linked = if force_build {
        false
    } else {
        try_link_prebuilt_all(&cfg)
    };
    if !linked {
        build_with_cc(&cfg, &node_editor_root, &imgui_src, &cimgui_root);
    }
}
