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

    fn is_windows(&self) -> bool {
        self.target_os == "windows"
    }

    fn is_msvc(&self) -> bool {
        self.target_env == "msvc"
    }

    fn use_static_crt(&self) -> bool {
        self.is_windows()
            && self.is_msvc()
            && env::var("CARGO_CFG_TARGET_FEATURE")
                .unwrap_or_default()
                .split(',')
                .any(|f| f == "crt-static")
    }

    fn is_wasm(&self) -> bool {
        self.target_arch == "wasm32"
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

fn sanitize_bindings_file(path: &Path) {
    if let Ok(content) = std::fs::read_to_string(path) {
        let _ = std::fs::write(path, sanitize_bindings_string(&content));
    }
}

fn parse_bool_env(name: &str) -> bool {
    matches!(
        env::var(name).ok().as_deref(),
        Some("1") | Some("true") | Some("yes") | Some("on")
    )
}

fn generate_bindings(cfg: &BuildConfig) {
    let header = cfg.manifest_dir.join("shim/cimgui_test_engine.h");
    let bindings = bindgen::Builder::default()
        .header(header.to_string_lossy())
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .allowlist_function("imgui_test_engine_.*")
        .allowlist_type("ImGuiTestEngine.*")
        .allowlist_var("ImGuiTestEngine.*")
        .blocklist_type("ImGuiContext")
        .derive_default(true)
        .derive_debug(true)
        .derive_copy(true)
        .derive_eq(true)
        .derive_partialeq(true)
        .derive_hash(true)
        .prepend_enum_name(false)
        .layout_tests(false)
        .generate()
        .expect("Unable to generate imgui_test_engine bindings");

    let out = cfg.out_dir.join("bindings.rs");
    bindings
        .write_to_file(&out)
        .expect("Couldn't write imgui_test_engine bindings");
    sanitize_bindings_file(&out);
}

fn build_with_cc(cfg: &BuildConfig, test_engine_root: &Path, imgui_src: &Path, cimgui_root: &Path) {
    let mut build = cc::Build::new();
    build.cpp(true).std("c++17");

    for (k, v) in env::vars() {
        if let Some(suffix) = k.strip_prefix("DEP_DEAR_IMGUI_DEFINE_") {
            build.define(suffix, v.as_str());
        }
    }

    build.define("IMGUI_ENABLE_TEST_ENGINE", None);
    build.define("IMGUI_USE_WCHAR32", None);
    build.define(
        "IMGUI_TEST_ENGINE_ENABLE_CAPTURE",
        Some(if cfg!(feature = "capture") { "1" } else { "0" }),
    );
    build.define(
        "IMGUI_TEST_ENGINE_ENABLE_COROUTINE_STDTHREAD_IMPL",
        Some("1"),
    );

    // Avoid link-time conflicts and workspace feature-unification footguns:
    // dear-imgui-sys provides the public hook symbols, while this crate builds upstream
    // implementations under renamed identifiers and registers them at runtime.
    build.define(
        "ImGuiTestEngineHook_ItemAdd",
        Some("DearImGuiRs_ImGuiTestEngineHook_ItemAdd_Impl"),
    );
    build.define(
        "ImGuiTestEngineHook_ItemInfo",
        Some("DearImGuiRs_ImGuiTestEngineHook_ItemInfo_Impl"),
    );
    build.define(
        "ImGuiTestEngineHook_Log",
        Some("DearImGuiRs_ImGuiTestEngineHook_Log_Impl"),
    );
    build.define(
        "ImGuiTestEngine_FindItemDebugLabel",
        Some("DearImGuiRs_ImGuiTestEngine_FindItemDebugLabel_Impl"),
    );

    build.include(imgui_src);
    build.include(cimgui_root);
    build.include(test_engine_root);
    build.include(test_engine_root.join("thirdparty"));
    build.include(cfg.manifest_dir.join("shim"));

    build.file(test_engine_root.join("imgui_capture_tool.cpp"));
    build.file(test_engine_root.join("imgui_te_context.cpp"));
    build.file(test_engine_root.join("imgui_te_coroutine.cpp"));
    build.file(test_engine_root.join("imgui_te_engine.cpp"));
    build.file(test_engine_root.join("imgui_te_exporters.cpp"));
    build.file(test_engine_root.join("imgui_te_perftool.cpp"));
    build.file(test_engine_root.join("imgui_te_ui.cpp"));
    build.file(test_engine_root.join("imgui_te_utils.cpp"));
    build.file(cfg.manifest_dir.join("shim/cimgui_test_engine.cpp"));
    build.file(cfg.manifest_dir.join("shim/default_tests.cpp"));
    build.file(
        cfg.manifest_dir
            .join("shim/imgui_test_engine_hooks_register.cpp"),
    );
    build.file(cfg.manifest_dir.join("shim/script_tests.cpp"));

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

    build.compile("dear_imgui_test_engine");
}

fn ensure_imgui_test_engine_enabled() {
    let enabled = env::var("DEP_DEAR_IMGUI_DEFINE_IMGUI_ENABLE_TEST_ENGINE")
        .or_else(|_| env::var("DEP_DEAR_IMGUI_DEFINE_IMGUITEST"))
        .unwrap_or_else(|_| "0".to_string());

    if enabled != "1" {
        panic!(
            "dear-imgui-test-engine-sys requires dear-imgui-sys to be compiled with IMGUI_ENABLE_TEST_ENGINE. \
             Enable the `test-engine` feature on dear-imgui-sys/dear-imgui-rs."
        );
    }
}

fn use_pregenerated_bindings(out_dir: &Path) -> bool {
    if parse_bool_env("DEAR_IMGUI_RS_REGEN_BINDINGS") {
        return false;
    }

    let preg = Path::new("src").join("bindings_pregenerated.rs");
    if !preg.exists() {
        return false;
    }

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
}

fn docsrs_build(cfg: &BuildConfig) {
    println!("cargo:warning=DOCS_RS detected: generating bindings, skipping native build");
    println!("cargo:rustc-cfg=docsrs");

    if use_pregenerated_bindings(&cfg.out_dir) {
        return;
    }

    let (imgui_src, cimgui_root) = resolve_imgui_includes(cfg);
    let test_engine_root = cfg
        .manifest_dir
        .join("third-party/imgui_test_engine/imgui_test_engine");

    if imgui_src.exists() && cimgui_root.exists() && test_engine_root.exists() {
        ensure_imgui_test_engine_enabled();
        generate_bindings(cfg);
        return;
    }

    panic!(
        "DOCS_RS build: Required headers not found and no pregenerated bindings present.\n\
         Please add src/bindings_pregenerated.rs (full bindgen output) to enable docs.rs builds.\n\
         Run: cargo build -p dear-imgui-test-engine-sys && cp target/debug/build/dear-imgui-test-engine-sys-*/out/bindings.rs extensions/dear-imgui-test-engine-sys/src/bindings_pregenerated.rs"
    );
}

fn main() {
    let cfg = BuildConfig::new();

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/bindings_pregenerated.rs");
    println!("cargo:rerun-if-changed=shim/cimgui_test_engine.h");
    println!("cargo:rerun-if-changed=shim/cimgui_test_engine.cpp");
    println!("cargo:rerun-if-changed=shim/default_tests.cpp");
    println!("cargo:rerun-if-changed=shim/imgui_test_engine_hooks_register.cpp");
    println!("cargo:rerun-if-changed=shim/script_tests.cpp");
    println!(
        "cargo:rerun-if-changed=third-party/imgui_test_engine/imgui_test_engine/imgui_capture_tool.cpp"
    );
    println!(
        "cargo:rerun-if-changed=third-party/imgui_test_engine/imgui_test_engine/imgui_te_context.cpp"
    );
    println!(
        "cargo:rerun-if-changed=third-party/imgui_test_engine/imgui_test_engine/imgui_te_coroutine.cpp"
    );
    println!(
        "cargo:rerun-if-changed=third-party/imgui_test_engine/imgui_test_engine/imgui_te_engine.cpp"
    );
    println!(
        "cargo:rerun-if-changed=third-party/imgui_test_engine/imgui_test_engine/imgui_te_exporters.cpp"
    );
    println!(
        "cargo:rerun-if-changed=third-party/imgui_test_engine/imgui_test_engine/imgui_te_perftool.cpp"
    );
    println!(
        "cargo:rerun-if-changed=third-party/imgui_test_engine/imgui_test_engine/imgui_te_ui.cpp"
    );
    println!(
        "cargo:rerun-if-changed=third-party/imgui_test_engine/imgui_test_engine/imgui_te_utils.cpp"
    );
    println!("cargo:rerun-if-env-changed=DEP_DEAR_IMGUI_DEFINE_IMGUI_ENABLE_TEST_ENGINE");
    println!("cargo:rerun-if-env-changed=DEP_DEAR_IMGUI_DEFINE_IMGUITEST");
    println!("cargo:rerun-if-env-changed=DEAR_IMGUI_RS_REGEN_BINDINGS");
    println!("cargo:rerun-if-env-changed=IMGUI_TEST_ENGINE_SYS_SKIP_CC");

    if cfg.docs_rs {
        docsrs_build(&cfg);
        return;
    }

    if cfg.is_wasm() {
        panic!(
            "dear-imgui-test-engine-sys does not support wasm32 targets. \
             The upstream imgui_test_engine is a native-only library (threads, OS interaction, capture tooling)."
        );
    }

    // Allow skipping native compilation even if submodules/sources are not available.
    // This is useful for cross-target `cargo check` or constrained environments.
    if env::var("IMGUI_TEST_ENGINE_SYS_SKIP_CC").is_ok() {
        if !use_pregenerated_bindings(&cfg.out_dir) {
            panic!(
                "IMGUI_TEST_ENGINE_SYS_SKIP_CC is set but no pregenerated bindings were found. \
                 Please ensure src/bindings_pregenerated.rs exists, or unset IMGUI_TEST_ENGINE_SYS_SKIP_CC."
            );
        }
        return;
    }

    let (imgui_src, cimgui_root) = resolve_imgui_includes(&cfg);
    let test_engine_root = cfg
        .manifest_dir
        .join("third-party/imgui_test_engine/imgui_test_engine");

    if !imgui_src.exists() {
        panic!("ImGui source not found at {:?}", imgui_src);
    }
    if !test_engine_root.exists() {
        panic!(
            "imgui_test_engine sources not found at {:?}. Did you init submodules?",
            test_engine_root
        );
    }

    ensure_imgui_test_engine_enabled();

    if parse_bool_env("DEAR_IMGUI_RS_REGEN_BINDINGS") {
        generate_bindings(&cfg);
        return;
    }

    generate_bindings(&cfg);
    build_with_cc(&cfg, &test_engine_root, &imgui_src, &cimgui_root);
}
