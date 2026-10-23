use build_support::native_build::{NativeBuildMode, NativeBuildPlan};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use build_support::binding::{
    ArtifactProfile, ArtifactProfileInput, BindingSpec, BuildRequest, BuildRequestInput,
    CORE_BUILD_ENV_VARS, CoreArtifactIdentity, CrateBindingDefine, NativeAbiProfile,
    RELEASE_CANDIDATE_SHA_ENV, SourceRevisions, TargetFacts, bindgen_rerun_env_vars,
    core_source_contract_hash, is_supported_wasm_target, validate_wasm_feature_contract,
};

const PLATFORM_IO_AGGREGATE_HOOKS_FEATURE: &str = "platform-io-aggregate-hooks-v3";

enum CoreBuildOutcome {
    Source,
    Prebuilt,
    BindingsOnly,
    WasmImports,
}

impl CoreBuildOutcome {
    fn label(&self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Prebuilt => "prebuilt",
            Self::BindingsOnly => "bindings-only",
            Self::WasmImports => "wasm-imports",
        }
    }
}

fn core_wasm_import_module() -> &'static str {
    &build_support::source_inventory::SourceInventory::embedded().wasm_import_module
}

#[derive(Clone, Debug)]
struct BuildConfig {
    manifest_dir: PathBuf,
    out_dir: PathBuf,
    target_os: String,
    target_env: String,
    target_abi: String,
    target_arch: String,
    target_endian: String,
    target_pointer_width: String,
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
            target_abi: env::var("CARGO_CFG_TARGET_ABI").unwrap_or_default(),
            target_arch: env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default(),
            target_endian: env::var("CARGO_CFG_TARGET_ENDIAN").unwrap_or_default(),
            target_pointer_width: env::var("CARGO_CFG_TARGET_POINTER_WIDTH").unwrap_or_default(),
            target_triple: env::var("TARGET").unwrap_or_default(),
            profile: env::var("PROFILE").unwrap_or_else(|_| "release".to_string()),
            docs_rs: env::var("DOCS_RS").is_ok(),
        }
    }
    fn is_windows(&self) -> bool {
        self.target_os == "windows"
    }
    fn is_core_wasm_target(&self) -> bool {
        is_supported_wasm_target(&self.target_triple)
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
    fn maintained_source_paths(&self) -> build_support::source_inventory::MaintainedSourcePaths {
        build_support::source_inventory::MaintainedSourcePaths::for_crate(
            env!("CARGO_PKG_NAME"),
            self.manifest_dir.clone(),
        )
        .unwrap_or_else(|error| panic!("dear-imgui-sys: {error}"))
    }
    fn cimgui_root(&self) -> PathBuf {
        self.maintained_source_paths()
            .source_root()
            .unwrap_or_else(|error| panic!("dear-imgui-sys: {error}"))
    }
    fn imgui_src(&self) -> PathBuf {
        self.cimgui_root().join("imgui")
    }
    fn crt_profile(&self) -> &'static str {
        if self.is_windows() && self.is_msvc() {
            if self.use_static_crt() { "mt" } else { "md" }
        } else {
            ""
        }
    }
    fn artifact_features(&self) -> Vec<&'static str> {
        let mut features = vec![
            PLATFORM_IO_AGGREGATE_HOOKS_FEATURE,
            build_support::SAFE_DEMO_FONT_BOUNDARY_ARTIFACT_FEATURE,
            "wchar32",
        ];
        if cfg!(feature = "stack-layout") {
            features.push("stack-layout");
        }
        if cfg!(feature = "freetype") {
            features.push("freetype");
        }
        if cfg!(feature = "test-engine") {
            features.push("test-engine");
        }
        features
    }
    fn artifact_suffix(&self) -> String {
        let mut suffix = String::new();
        if cfg!(feature = "stack-layout") {
            suffix.push_str("-stack-layout");
        }
        if cfg!(feature = "freetype") {
            suffix.push_str("-freetype");
        }
        suffix
    }
    fn archive_name(&self, crt: &str) -> String {
        let suffix = self.artifact_suffix();
        build_support::compose_archive_name(
            "dear-imgui",
            env!("CARGO_PKG_VERSION"),
            &self.target_triple,
            "static",
            (!suffix.is_empty()).then_some(suffix.as_str()),
            crt,
        )
    }
    fn source_revisions(&self) -> SourceRevisions {
        SourceRevisions::from_cargo_manifest(include_str!("Cargo.toml")).unwrap_or_else(|error| {
            panic!("invalid checked-in Dear ImGui source metadata: {error}")
        })
    }
    fn native_abi_profile(&self) -> NativeAbiProfile {
        NativeAbiProfile::for_target(TargetFacts {
            triple: &self.target_triple,
            os: &self.target_os,
            env: &self.target_env,
            target_abi: &self.target_abi,
            arch: &self.target_arch,
            endian: &self.target_endian,
            pointer_width: &self.target_pointer_width,
        })
        .unwrap_or_else(|error| panic!("dear-imgui-sys: {error}"))
    }
    fn binding_spec(&self) -> BindingSpec {
        if self.is_core_wasm_target() {
            BindingSpec::core_wasm(core_wasm_import_module())
        } else {
            BindingSpec::core_native(self.native_abi_profile())
        }
    }
    fn artifact_profile(&self) -> ArtifactProfile {
        ArtifactProfile::new(ArtifactProfileInput {
            crate_name: "dear-imgui",
            version: env!("CARGO_PKG_VERSION"),
            target: &self.target_triple,
            link_type: "static",
            crt: self.crt_profile(),
            features: self.artifact_features(),
            source_revisions: self.source_revisions(),
            binding_spec_hash: self.binding_spec().deterministic_hash(),
            source_contract_hash: core_source_contract_hash(),
        })
    }
    fn build_request(&self) -> BuildRequest {
        let artifact_features = self.artifact_features();
        let environment = CORE_BUILD_ENV_VARS
            .iter()
            .copied()
            .map(|name| (name, env::var(name).ok()))
            .collect::<Vec<_>>();
        BuildRequest::new(BuildRequestInput {
            target_triple: &self.target_triple,
            target_os: &self.target_os,
            target_env: &self.target_env,
            target_abi: &self.target_abi,
            target_arch: &self.target_arch,
            target_endian: &self.target_endian,
            target_pointer_width: &self.target_pointer_width,
            cargo_profile: &self.profile,
            artifact_features,
            environment: environment
                .iter()
                .map(|(name, value)| (*name, value.as_deref()))
                .collect(),
        })
    }
}

fn write_package_metadata(cfg: &BuildConfig, profile: &ArtifactProfile) {
    let candidate_sha = env::var(RELEASE_CANDIDATE_SHA_ENV).unwrap_or_else(|_| {
        panic!("dear-imgui-sys: feature `package-bin` requires {RELEASE_CANDIDATE_SHA_ENV}")
    });
    let archive_name = cfg.archive_name(cfg.crt_profile());
    fs::write(cfg.out_dir.join("prebuilt-archive-name.txt"), archive_name)
        .expect("failed to write package archive name");
    fs::write(
        cfg.out_dir.join("prebuilt-manifest.txt"),
        profile
            .release_manifest_bytes(&candidate_sha)
            .unwrap_or_else(|error| panic!("dear-imgui-sys: {error}")),
    )
    .expect("failed to write package manifest");
}

fn main() {
    let cfg = BuildConfig::new();
    let sources = cfg.maintained_source_paths();
    if cfg!(all(feature = "package-bin", feature = "test-engine")) {
        panic!(
            "dear-imgui-sys: feature `test-engine` is source-only and cannot be packaged as a prebuilt core artifact"
        );
    }
    if cfg!(all(feature = "package-bin", feature = "abi-probe")) {
        panic!(
            "dear-imgui-sys: feature `abi-probe` is CI-only and cannot be packaged as a prebuilt core artifact"
        );
    }
    validate_wasm_feature_contract(&cfg.target_triple, cfg!(feature = "wasm"))
        .unwrap_or_else(|error| panic!("dear-imgui-sys: {error}"));
    let skip_cc = build_support::parse_bool_env("IMGUI_SYS_SKIP_CC");
    if cfg!(feature = "abi-probe") && (cfg.is_core_wasm_target() || skip_cc) {
        panic!(
            "dear-imgui-sys: feature `abi-probe` requires a native source build and cannot be combined with WASM or IMGUI_SYS_SKIP_CC"
        );
    }
    let artifact_profile = cfg.artifact_profile();
    let build_request = cfg.build_request();
    if cfg!(feature = "package-bin") {
        write_package_metadata(&cfg, &artifact_profile);
    }

    // Re-run triggers
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rustc-check-cfg=cfg(dear_imgui_rs_native_symbols)");
    println!("cargo:rustc-check-cfg=cfg(dear_imgui_rs_platform_io_hooks)");
    println!("cargo:rustc-check-cfg=cfg(dear_imgui_rs_wasm_import_target)");
    if cfg.is_core_wasm_target() {
        println!("cargo:rustc-cfg=dear_imgui_rs_wasm_import_target");
    }
    // Pregenerated bindings are copied into OUT_DIR when native toolchains are disabled.
    // Track them so `cargo check` picks up refreshed bindings immediately.
    println!("cargo:rerun-if-changed=src/bindings_pregenerated.rs");
    println!("cargo:rerun-if-changed=src/bindings_pregenerated_windows.rs");
    println!("cargo:rerun-if-changed=src/wasm_bindings_pregenerated.rs");
    println!("cargo:rerun-if-changed=src/stack_layout_imgui_externs.cpp.inc");
    println!("cargo:rerun-if-changed=src/stack_layout_imgui_item_add.cpp.inc");
    println!("cargo:rerun-if-changed=src/stack_layout_imgui_item_size.cpp.inc");
    println!("cargo:rerun-if-changed=src/stack_layout_imgui_item_size_horizontal_compat.cpp.inc");
    println!("cargo:rerun-if-changed=backend-shims");
    for path in sources.all_candidate_paths() {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    // Pregenerated bindings bypass bindgen's header tracking, but native objects
    // still depend on these headers.
    let cimgui_root = cfg.manifest_dir.join(&sources.source().source_root);
    println!(
        "cargo:rerun-if-changed={}",
        cimgui_root.join("cimgui.h").display()
    );
    let imgui_src = cimgui_root.join("imgui");
    for header in [
        "imgui.h",
        "imgui_internal.h",
        "imconfig.h",
        "imstb_rectpack.h",
        "imstb_textedit.h",
        "imstb_truetype.h",
    ] {
        println!(
            "cargo:rerun-if-changed={}",
            imgui_src.join(header).display()
        );
    }
    if cfg!(feature = "freetype") {
        println!(
            "cargo:rerun-if-changed={}",
            imgui_src.join("misc/freetype/imgui_freetype.h").display()
        );
    }
    for name in CORE_BUILD_ENV_VARS {
        println!("cargo:rerun-if-env-changed={name}");
    }
    println!("cargo:rerun-if-env-changed={RELEASE_CANDIDATE_SHA_ENV}");
    for name in bindgen_rerun_env_vars(&cfg.target_triple) {
        println!("cargo:rerun-if-env-changed={name}");
    }
    println!(
        "cargo:rustc-env=DEAR_IMGUI_ARTIFACT_TARGET={}",
        artifact_profile.target
    );
    println!(
        "cargo:rustc-env=DEAR_IMGUI_ARTIFACT_CRT={}",
        artifact_profile.crt
    );
    println!(
        "cargo:rustc-env=DEAR_IMGUI_CIMGUI_REVISION={}",
        artifact_profile.source_revisions.cimgui
    );
    println!(
        "cargo:rustc-env=DEAR_IMGUI_IMGUI_REVISION={}",
        artifact_profile.source_revisions.imgui
    );
    println!(
        "cargo:rustc-env=DEAR_IMGUI_BINDING_SPEC_HASH={}",
        artifact_profile.binding_spec_hash
    );
    println!(
        "cargo:BUILD_REQUEST_HASH={}",
        build_request.deterministic_hash()
    );
    if cfg.is_core_wasm_target() && any_backend_shim_enabled() {
        panic!("backend-shim-* features are native-only and cannot be combined with WASM");
    }
    // docs.rs: prepare bindings and downstream metadata without compiling C++.
    if cfg.docs_rs {
        docsrs_build(&cfg);
        return;
    }

    // Maintainer workflow: regenerate bindings via bindgen without requiring native compilation.
    if build_support::parse_bool_env("DEAR_IMGUI_RS_REGEN_BINDINGS") {
        if cfg.is_core_wasm_target() {
            panic!(
                "dear-imgui-sys: WASM bindings are generated through the import-provider adapter; \
                 run `cargo run -p xtask -- wasm-bindgen` (provider: {})",
                core_wasm_import_module()
            );
        }
        generate_bindings_native(&cfg);
        export_include_paths(&cfg);
        return;
    }

    let native_plan = if cfg.is_core_wasm_target() {
        None
    } else {
        Some(
            NativeBuildPlan::from_core_env(
                cfg!(feature = "build-from-source") || cfg!(feature = "package-bin"),
                cfg!(feature = "prebuilt"),
                cfg!(feature = "test-engine")
                    && !cfg!(feature = "package-bin")
                    && !cfg!(feature = "abi-probe"),
            )
            .unwrap_or_else(|error| panic!("dear-imgui-sys: {error}")),
        )
    };
    let bindings_only = native_plan
        .as_ref()
        .is_some_and(|plan| plan.mode() == NativeBuildMode::BindingsOnly);
    if bindings_only && any_backend_shim_enabled() {
        panic!(
            "bindings-only mode is incompatible with backend-shim-* features; disable shims or choose a native build mode"
        );
    }

    // Native artifacts do not require regenerating the checked-in Rust bindings.
    if !use_pregenerated_bindings(&cfg) {
        if bindings_only
            || native_plan
                .as_ref()
                .is_some_and(|plan| plan.mode() == NativeBuildMode::Prebuilt)
        {
            panic!("the selected build mode requires checked-in pregenerated bindings");
        }
        generate_bindings_native(&cfg);
    }

    let prebuilt = match native_plan.as_ref() {
        Some(plan)
            if matches!(
                plan.mode(),
                NativeBuildMode::Auto | NativeBuildMode::Prebuilt
            ) =>
        {
            let artifact = find_prebuilt_all(&cfg, plan);
            plan.finish_prebuilt_lookup(artifact.is_some())
                .unwrap_or_else(|error| panic!("dear-imgui-sys: {error}"));
            artifact
        }
        _ => None,
    };

    // Static link order remains backend-shim first, core dear_imgui second.
    if !bindings_only && !cfg.is_core_wasm_target() {
        build_backend_shims(&cfg);
    }

    let outcome = match native_plan.as_ref() {
        None => CoreBuildOutcome::WasmImports,
        Some(plan) => match plan.mode() {
            NativeBuildMode::BindingsOnly => CoreBuildOutcome::BindingsOnly,
            NativeBuildMode::Source => {
                build_with_cc_cfg(&cfg, &sources);
                CoreBuildOutcome::Source
            }
            NativeBuildMode::Auto | NativeBuildMode::Prebuilt => {
                if let Some(artifact) = prebuilt {
                    link_prebuilt(&artifact, &cfg);
                    CoreBuildOutcome::Prebuilt
                } else {
                    build_with_cc_cfg(&cfg, &sources);
                    CoreBuildOutcome::Source
                }
            }
        },
    };
    println!("cargo:NATIVE_MODE={}", outcome.label());
    if matches!(
        outcome,
        CoreBuildOutcome::Source | CoreBuildOutcome::Prebuilt
    ) {
        println!("cargo:rustc-cfg=dear_imgui_rs_native_symbols");
        println!("cargo:rustc-cfg=dear_imgui_rs_platform_io_hooks");
    }

    // ImGui core includes default platform handlers on Windows (clipboard, IME, open-in-shell)
    // which call Win32 APIs. MSVC honors the `#pragma comment(lib, ...)` directives in imgui.cpp,
    // but MinGW/GNU toolchains do not, so we must link these system libraries explicitly.
    //
    // Ref: https://github.com/Latias94/dear-imgui-rs/issues/20
    if cfg.is_windows() && !cfg.is_core_wasm_target() {
        println!("cargo:rustc-link-lib=user32"); // OpenClipboard, etc.
        println!("cargo:rustc-link-lib=kernel32"); // MultiByteToWideChar, etc.
        println!("cargo:rustc-link-lib=shell32"); // ShellExecuteW
        println!("cargo:rustc-link-lib=imm32"); // IME support (ImmGetContext, etc.)
    }

    // Export include paths/defines for extensions
    export_include_paths(&cfg);
}

fn docsrs_build(cfg: &BuildConfig) {
    println!("cargo:warning=DOCS_RS detected: preparing bindings, skipping native build");
    println!("cargo:rustc-cfg=docsrs");
    export_include_paths(cfg);
    if use_pregenerated_bindings(cfg) {
        return;
    }
    generate_bindings_native(cfg);
}

#[cfg(feature = "bindgen")]
fn generate_bindings_native(cfg: &BuildConfig) {
    assert_canonical_bindgen_environment();
    let spec = BindingSpec::core_native(cfg.native_abi_profile());
    let bindings = core_bindgen_builder(cfg, &spec)
        .generate()
        .expect("Unable to generate bindings from cimgui.h");
    let out = cfg.out_dir.join("bindings.rs");
    bindings
        .write_to_file(&out)
        .expect("Couldn't write bindings!");
    let content = std::fs::read_to_string(&out)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", out.display()));
    let content = spec.sanitize(&content);
    spec.validate_generated_bindings(&content)
        .unwrap_or_else(|error| panic!("invalid native bindings: {error}"));
    std::fs::write(&out, content)
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", out.display()));
}

#[cfg(feature = "bindgen")]
fn core_bindgen_builder(cfg: &BuildConfig, spec: &BindingSpec) -> bindgen::Builder {
    let cimgui_root = cfg.cimgui_root();
    let shim_root = cfg
        .out_dir
        .join("binding-spec-headers")
        .join(spec.deterministic_hash().replace(':', "-"));
    std::fs::create_dir_all(&shim_root).unwrap_or_else(|error| {
        panic!(
            "failed to create binding header shim dir {}: {error}",
            shim_root.display()
        )
    });
    for shim in spec.header_shims {
        std::fs::write(shim_root.join(shim.name), shim.contents).unwrap_or_else(|error| {
            panic!("failed to write binding header shim {}: {error}", shim.name)
        });
    }
    let wrapper = format!("{}\n#include \"{}\"\n", spec.header_preamble, spec.header);
    let mut builder = bindgen::Builder::default()
        .header_contents("dear_imgui_rs_bindings.h", &wrapper)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()));
    builder = match spec.formatter {
        build_support::binding::BindingFormatter::Rustfmt => {
            builder.formatter(bindgen::Formatter::Rustfmt)
        }
    };
    builder = match spec.rust_edition {
        build_support::binding::BindingRustEdition::Rust2021 => {
            builder.rust_edition(bindgen::RustEdition::Edition2021)
        }
    };
    builder = builder.clang_arg(format!("-I{}", shim_root.display()));
    for include in spec.include_paths {
        builder = builder.clang_arg(format!("-I{}", cimgui_root.join(include).display()));
    }
    for arg in spec.clang_args {
        builder = builder.clang_arg(*arg);
    }
    for define in spec.clang_defines {
        builder = builder.clang_arg(format!("-D{define}"));
    }
    for pattern in spec.allowlisted_functions {
        builder = builder.allowlist_function(pattern);
    }
    for pattern in spec.blocklisted_functions {
        builder = builder.blocklist_function(pattern);
    }
    for pattern in spec.allowlisted_types {
        builder = builder.allowlist_type(pattern);
    }
    for pattern in spec.blocklisted_types {
        builder = builder.blocklist_type(pattern);
    }
    for pattern in spec.allowlisted_vars {
        builder = builder.allowlist_var(pattern);
    }
    for line in spec.raw_lines {
        builder = builder.raw_line(*line);
    }
    builder
        .derive_default(spec.derives.default)
        .derive_debug(spec.derives.debug)
        .derive_copy(spec.derives.copy)
        .derive_eq(spec.derives.eq)
        .derive_partialeq(spec.derives.partial_eq)
        .derive_hash(spec.derives.hash)
        .prepend_enum_name(spec.prepend_enum_name)
        .layout_tests(spec.layout_tests)
}

#[cfg(feature = "bindgen")]
fn assert_canonical_bindgen_environment() {
    let names = env::vars_os()
        .map(|(name, _)| name.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    build_support::binding::validate_bindgen_environment(&names)
        .unwrap_or_else(|error| panic!("dear-imgui-sys: {error}"));
}

#[cfg(feature = "freetype")]
fn find_freetype_dependency(emit_cargo_metadata: bool) -> build_support::NativeDependency {
    let dependency = build_support::find_freetype(build_support::PackageSearchConfig {
        use_pkg_config: cfg!(feature = "pkg-config"),
        use_vcpkg: cfg!(feature = "vcpkg"),
        emit_cargo_metadata,
    })
    .unwrap_or_else(|message| panic!("dear-imgui-sys: {message}"));
    println!(
        "cargo:warning=dear-imgui-sys: using FreeType from {}",
        dependency.source
    );
    dependency
}

struct PrebuiltCore {
    directory: PathBuf,
    identity: CoreArtifactIdentity,
}

fn find_prebuilt_all(cfg: &BuildConfig, plan: &NativeBuildPlan) -> Option<PrebuiltCore> {
    if let Some(source) = plan.prebuilt_source() {
        let mut names = vec![cfg.archive_name(cfg.crt_profile())];
        let fallback = cfg.archive_name("");
        if fallback != names[0] {
            names.push(fallback);
        }
        let directory = source
            .acquire(
                &prebuilt_cache_root(cfg),
                &expected_lib_name(&cfg.target_env),
                &cfg.target_env,
                &names,
            )
            .unwrap_or_else(|error| panic!("dear-imgui-sys: {error}"));
        let identity =
            validate_prebuilt_artifact_profile(&directory, cfg).unwrap_or_else(|error| {
                panic!("dear-imgui-sys: selected an incompatible dear_imgui artifact: {error}")
            });
        return Some(PrebuiltCore {
            directory,
            identity,
        });
    }
    if cfg!(feature = "prebuilt")
        && let Some(directory) = try_download_prebuilt_from_release(cfg)
        && let Some(artifact) = find_prebuilt(&directory, cfg)
    {
        return Some(artifact);
    }
    let repository_directory = cfg
        .manifest_dir
        .join("third-party/prebuilt")
        .join(&cfg.target_triple);
    find_prebuilt(&repository_directory, cfg)
}

fn build_with_cc_cfg(
    cfg: &BuildConfig,
    sources: &build_support::source_inventory::MaintainedSourcePaths,
) {
    sources
        .validate_native()
        .unwrap_or_else(|error| panic!("dear-imgui-sys: {error}"));
    let cimgui_root = cfg.cimgui_root();
    let mut build = new_native_cpp_build(cfg);
    build.include(&cimgui_root);
    let imgui_core = sources
        .file("imgui-core")
        .unwrap_or_else(|error| panic!("dear-imgui-sys: {error}"));
    let imgui_cpp = if cfg!(feature = "stack-layout") {
        let patched = write_stack_layout_patched_imgui_cpp(cfg, &imgui_core);
        build.file(
            sources
                .file("stack-layout-shim")
                .unwrap_or_else(|error| panic!("dear-imgui-sys: {error}")),
        );
        patched
    } else {
        imgui_core
    };
    build.file(write_safe_demo_patched_imgui_cpp(cfg, &imgui_cpp));
    build.file(write_numeric_patched_imgui_widgets_cpp(
        cfg,
        &sources
            .file("imgui-widgets")
            .unwrap_or_else(|error| panic!("dear-imgui-sys: {error}")),
    ));
    for file_id in [
        "imgui-draw",
        "imgui-tables",
        "demo-window-shim",
        "dock-builder-shim",
        "platform-io-hooks",
    ] {
        build.file(
            sources
                .file(file_id)
                .unwrap_or_else(|error| panic!("dear-imgui-sys: {error}")),
        );
    }
    // Include official demo/metrics/debug windows for native builds so symbols like
    // ImGui::ShowDemoWindow/ShowAboutWindow/ShowStyleEditor resolve.
    // The WASM provider is compiled separately by xtask.
    build.file(write_safe_demo_patched_imgui_demo_cpp(
        cfg,
        &sources
            .file("imgui-demo")
            .unwrap_or_else(|error| panic!("dear-imgui-sys: {error}")),
    ));
    build.file(
        sources
            .file("cimgui-wrapper")
            .unwrap_or_else(|error| panic!("dear-imgui-sys: {error}")),
    );
    if cfg!(feature = "test-engine") {
        build.define("IMGUI_ENABLE_TEST_ENGINE", None);
        build.file(
            sources
                .file("test-engine-hooks")
                .unwrap_or_else(|error| panic!("dear-imgui-sys: {error}")),
        );
    }
    if cfg!(feature = "abi-probe") {
        build.define("DEAR_IMGUI_RS_ABI_PROBE", Some("1"));
    }
    #[cfg(feature = "freetype")]
    {
        let freetype = find_freetype_dependency(true);
        // Enable both FreeType and stb_truetype backends.
        // ImGui 1.92 gates stb_truetype helpers (e.g. ImFontAtlasGetFontLoaderForStbTruetype)
        // behind IMGUI_ENABLE_STB_TRUETYPE, while FreeType is selected when IMGUI_ENABLE_FREETYPE is defined.
        // Defining both keeps the stb_ symbols available for cimgui wrappers while still defaulting to FreeType.
        build.define("IMGUI_ENABLE_FREETYPE", Some("1"));
        build.define("IMGUI_ENABLE_STB_TRUETYPE", Some("1"));
        for include in &freetype.include_paths {
            build.include(include.display().to_string());
        }
        build.file(
            sources
                .file("freetype")
                .unwrap_or_else(|error| panic!("dear-imgui-sys: {error}")),
        );
    }

    build_support::compile_cpp_archive(
        &mut build,
        "dear_imgui",
        &cfg.target_os,
        &cfg.target_env,
        &cfg.target_abi,
    );
}

fn write_safe_demo_patched_imgui_cpp(cfg: &BuildConfig, imgui_cpp: &Path) -> PathBuf {
    let source = std::fs::read_to_string(imgui_cpp)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", imgui_cpp.display()));
    let patched = build_support::patch_imgui_cpp_for_safe_demo(&source).unwrap_or_else(|error| {
        panic!(
            "failed to patch {} for safe demo windows: {error}",
            imgui_cpp.display()
        )
    });
    let out = cfg.out_dir.join("imgui_safe_demo_patched.cpp");
    std::fs::write(&out, patched)
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", out.display()));
    out
}

fn write_safe_demo_patched_imgui_demo_cpp(cfg: &BuildConfig, imgui_demo_cpp: &Path) -> PathBuf {
    let source = std::fs::read_to_string(imgui_demo_cpp)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", imgui_demo_cpp.display()));
    let patched =
        build_support::patch_imgui_demo_cpp_for_safe_demo(&source).unwrap_or_else(|error| {
            panic!(
                "failed to patch {} for safe demo windows: {error}",
                imgui_demo_cpp.display()
            )
        });
    let out = cfg.out_dir.join("imgui_demo_safe_demo_patched.cpp");
    std::fs::write(&out, patched)
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", out.display()));
    out
}

fn write_numeric_patched_imgui_widgets_cpp(cfg: &BuildConfig, imgui_widgets_cpp: &Path) -> PathBuf {
    let source = std::fs::read_to_string(imgui_widgets_cpp)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", imgui_widgets_cpp.display()));
    let patched = build_support::patch_imgui_widgets_cpp_for_defined_numeric_conversions(&source)
        .unwrap_or_else(|error| {
            panic!(
                "failed to patch {} for defined numeric conversions: {error}",
                imgui_widgets_cpp.display()
            )
        });
    let out = cfg.out_dir.join("imgui_widgets_numeric_patched.cpp");
    std::fs::write(&out, patched)
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", out.display()));
    out
}

// imgui-node-editor's stack layout extension is not a standalone widget layer:
// it also patches Dear ImGui's ItemSize() and ItemAdd() internals so regular
// widgets can participate in BeginHorizontal/BeginVertical/Spring measurement.
// Keep the checked-out cimgui submodule untouched and patch only the OUT_DIR
// copy of imgui.cpp, with marker checks that fail loudly when upstream changes.
fn write_stack_layout_patched_imgui_cpp(cfg: &BuildConfig, imgui_cpp: &Path) -> PathBuf {
    let source = std::fs::read_to_string(imgui_cpp)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", imgui_cpp.display()));
    let item_size_marker = "void ImGui::ItemSize(const ImVec2& size, float text_baseline_y)";
    let item_size_pos = source.find(item_size_marker).unwrap_or_else(|| {
        panic!(
            "failed to patch {}: ItemSize marker not found",
            imgui_cpp.display()
        )
    });
    let item_add_marker = "bool ImGui::ItemAdd(";
    let item_add_pos = source.find(item_add_marker).unwrap_or_else(|| {
        panic!(
            "failed to patch {}: ItemAdd marker not found",
            imgui_cpp.display()
        )
    });
    let status_marker = "    g.LastItemData.StatusFlags = ImGuiItemStatusFlags_None;";
    let status_pos = source.find(status_marker).unwrap_or_else(|| {
        panic!(
            "failed to patch {}: LastItemData status marker not found",
            imgui_cpp.display()
        )
    });
    assert!(
        status_pos > item_add_pos,
        "failed to patch {}: LastItemData marker appears before ItemAdd",
        imgui_cpp.display()
    );
    let horizontal_if_marker = "    if (window->DC.LayoutType == ImGuiLayoutType_Horizontal)";
    let horizontal_if_pos = source[item_size_pos..]
        .find(horizontal_if_marker)
        .map(|offset| item_size_pos + offset)
        .unwrap_or_else(|| {
            panic!(
                "failed to patch {}: ItemSize horizontal-layout if marker not found",
                imgui_cpp.display()
            )
        });
    let same_line_marker = "        SameLine();";
    let horizontal_end_pos = source[horizontal_if_pos..]
        .find(same_line_marker)
        .map(|offset| horizontal_if_pos + offset + same_line_marker.len())
        .unwrap_or_else(|| {
            panic!(
                "failed to patch {}: ItemSize horizontal-layout SameLine marker not found",
                imgui_cpp.display()
            )
        });
    if source[item_size_pos..horizontal_if_pos].find('}').is_some() {
        panic!(
            "failed to patch {}: ItemSize horizontal-layout marker is outside ItemSize",
            imgui_cpp.display()
        )
    }
    let skip_items_if_marker = "    if (window->SkipItems)";
    let skip_items_if_pos = source[item_size_pos..]
        .find(skip_items_if_marker)
        .map(|offset| item_size_pos + offset)
        .unwrap_or_else(|| {
            panic!(
                "failed to patch {}: ItemSize SkipItems marker not found",
                imgui_cpp.display()
            )
        });
    let skip_items_return_marker = "        return;";
    let item_size_early_insert_pos = source[skip_items_if_pos..]
        .find(skip_items_return_marker)
        .map(|offset| skip_items_if_pos + offset + skip_items_return_marker.len())
        .unwrap_or_else(|| {
            panic!(
                "failed to patch {}: ItemSize SkipItems return marker not found",
                imgui_cpp.display()
            )
        });
    if source[item_size_pos..skip_items_if_pos].find('}').is_some() {
        panic!(
            "failed to patch {}: ItemSize SkipItems marker is outside ItemSize",
            imgui_cpp.display()
        )
    }

    let extern_pos = item_size_pos.min(item_add_pos);
    let item_add_insert_pos = status_pos + status_marker.len();
    let extern_declarations = include_str!("src/stack_layout_imgui_externs.cpp.inc");
    let item_size_early_branch = include_str!("src/stack_layout_imgui_item_size.cpp.inc");
    let item_size_replacement =
        include_str!("src/stack_layout_imgui_item_size_horizontal_compat.cpp.inc");
    let item_add_hook = include_str!("src/stack_layout_imgui_item_add.cpp.inc");

    let mut edits = vec![
        (extern_pos, extern_pos, extern_declarations),
        (
            item_size_early_insert_pos,
            item_size_early_insert_pos,
            item_size_early_branch,
        ),
        (horizontal_if_pos, horizontal_end_pos, item_size_replacement),
        (item_add_insert_pos, item_add_insert_pos, item_add_hook),
    ];
    edits.sort_by_key(|(start, _, _)| *start);

    let mut patched = String::with_capacity(source.len() + 1800);
    let mut cursor = 0;
    for (start, end, replacement) in edits {
        assert!(
            start >= cursor,
            "failed to patch {}: generated edits overlap",
            imgui_cpp.display()
        );
        patched.push_str(&source[cursor..start]);
        patched.push_str(replacement);
        cursor = end;
    }
    patched.push_str(&source[cursor..]);

    let out = cfg.out_dir.join("imgui_stack_layout_patched.cpp");
    std::fs::write(&out, patched)
        .unwrap_or_else(|e| panic!("failed to write {}: {e}", out.display()));
    out
}

fn any_backend_shim_enabled() -> bool {
    backend_shim_specs().iter().any(|spec| spec.enabled)
}

fn new_native_cpp_build(cfg: &BuildConfig) -> cc::Build {
    let mut build = cc::Build::new();
    build.cpp(true).std("c++17");
    build_support::configure_cpp_runtime_linkage(
        &mut build,
        &cfg.target_os,
        &cfg.target_env,
        &cfg.target_abi,
    );
    build.include(cfg.imgui_src());
    build.define("IMGUI_DISABLE_OBSOLETE_FUNCTIONS", None);
    build.define("IMGUI_USE_WCHAR32", None);
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
    build
}

fn build_backend_shims(cfg: &BuildConfig) {
    if !any_backend_shim_enabled() {
        return;
    }

    // Track auxiliary backend headers too, including the embedded OpenGL loader.
    println!(
        "cargo:rerun-if-changed={}",
        cfg.imgui_src().join("backends").display()
    );
    for spec in backend_shim_specs() {
        if spec.enabled && backend_shim_target_matches(spec.target, cfg) {
            build_backend_shim_from_spec(cfg, spec);
        }
    }
}

#[cfg(not(feature = "bindgen"))]
fn generate_bindings_native(_cfg: &BuildConfig) {
    panic!(
        "dear-imgui-sys: regenerating bindings requires the `bindgen` feature. \
         Re-run with `--features bindgen` and DEAR_IMGUI_RS_REGEN_BINDINGS=1."
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BackendShimTarget {
    Any,
    Android,
    Apple,
    Linux,
    Macos,
    Windows,
}

#[derive(Clone, Copy, Debug)]
struct BackendShimLinkLib {
    target: BackendShimTarget,
    name: &'static str,
}

#[derive(Clone, Copy, Debug)]
struct BackendShimSpec {
    enabled: bool,
    target: BackendShimTarget,
    upstream_source: &'static str,
    shim_source: &'static str,
    output_lib: &'static str,
    link_libs: &'static [BackendShimLinkLib],
}

const OPENGL3_BACKEND_LINK_LIBS: &[BackendShimLinkLib] = &[
    BackendShimLinkLib {
        target: BackendShimTarget::Linux,
        name: "dl",
    },
    BackendShimLinkLib {
        target: BackendShimTarget::Android,
        name: "dl",
    },
    // The official OpenGL3 backend references GLES entry points directly.
    // Link the Android GLES loader explicitly so NativeActivity can load the
    // final cdylib before the application creates its own EGL/GLES context.
    BackendShimLinkLib {
        target: BackendShimTarget::Android,
        name: "GLESv3",
    },
];

const WIN32_BACKEND_LINK_LIBS: &[BackendShimLinkLib] = &[
    BackendShimLinkLib {
        target: BackendShimTarget::Any,
        name: "gdi32",
    },
    BackendShimLinkLib {
        target: BackendShimTarget::Any,
        name: "dwmapi",
    },
];

const DX11_BACKEND_LINK_LIBS: &[BackendShimLinkLib] = &[
    BackendShimLinkLib {
        target: BackendShimTarget::Any,
        name: "d3d11",
    },
    BackendShimLinkLib {
        target: BackendShimTarget::Any,
        name: "dxgi",
    },
    BackendShimLinkLib {
        target: BackendShimTarget::Any,
        name: "d3dcompiler",
    },
];

const DX12_BACKEND_LINK_LIBS: &[BackendShimLinkLib] = &[
    BackendShimLinkLib {
        target: BackendShimTarget::Any,
        name: "d3d12",
    },
    BackendShimLinkLib {
        target: BackendShimTarget::Any,
        name: "dxgi",
    },
    BackendShimLinkLib {
        target: BackendShimTarget::Any,
        name: "d3dcompiler",
    },
];

const METAL_BACKEND_LINK_LIBS: &[BackendShimLinkLib] = &[
    BackendShimLinkLib {
        target: BackendShimTarget::Any,
        name: "framework=Foundation",
    },
    BackendShimLinkLib {
        target: BackendShimTarget::Any,
        name: "framework=Metal",
    },
    BackendShimLinkLib {
        target: BackendShimTarget::Any,
        name: "framework=QuartzCore",
    },
    BackendShimLinkLib {
        target: BackendShimTarget::Macos,
        name: "framework=Cocoa",
    },
];

const OSX_BACKEND_LINK_LIBS: &[BackendShimLinkLib] = &[
    BackendShimLinkLib {
        target: BackendShimTarget::Any,
        name: "framework=Cocoa",
    },
    BackendShimLinkLib {
        target: BackendShimTarget::Any,
        name: "framework=Carbon",
    },
    BackendShimLinkLib {
        target: BackendShimTarget::Any,
        name: "framework=GameController",
    },
];

fn backend_shim_specs() -> &'static [BackendShimSpec] {
    &[
        BackendShimSpec {
            enabled: cfg!(feature = "backend-shim-opengl3"),
            target: BackendShimTarget::Any,
            upstream_source: "imgui_impl_opengl3.cpp",
            shim_source: "opengl3.cpp",
            output_lib: "dear_imgui_backend_opengl3",
            link_libs: OPENGL3_BACKEND_LINK_LIBS,
        },
        BackendShimSpec {
            enabled: cfg!(feature = "backend-shim-android"),
            target: BackendShimTarget::Android,
            upstream_source: "imgui_impl_android.cpp",
            shim_source: "android.cpp",
            output_lib: "dear_imgui_backend_android",
            link_libs: &[],
        },
        BackendShimSpec {
            enabled: cfg!(feature = "backend-shim-win32"),
            target: BackendShimTarget::Windows,
            upstream_source: "imgui_impl_win32.cpp",
            shim_source: "win32.cpp",
            output_lib: "dear_imgui_backend_win32",
            link_libs: WIN32_BACKEND_LINK_LIBS,
        },
        BackendShimSpec {
            enabled: cfg!(feature = "backend-shim-dx11"),
            target: BackendShimTarget::Windows,
            upstream_source: "imgui_impl_dx11.cpp",
            shim_source: "dx11.cpp",
            output_lib: "dear_imgui_backend_dx11",
            link_libs: DX11_BACKEND_LINK_LIBS,
        },
        BackendShimSpec {
            enabled: cfg!(feature = "backend-shim-dx12"),
            target: BackendShimTarget::Windows,
            upstream_source: "imgui_impl_dx12.cpp",
            shim_source: "dx12.cpp",
            output_lib: "dear_imgui_backend_dx12",
            link_libs: DX12_BACKEND_LINK_LIBS,
        },
        BackendShimSpec {
            enabled: cfg!(feature = "backend-shim-metal"),
            target: BackendShimTarget::Apple,
            upstream_source: "imgui_impl_metal.mm",
            shim_source: "metal.mm",
            output_lib: "dear_imgui_backend_metal",
            link_libs: METAL_BACKEND_LINK_LIBS,
        },
        BackendShimSpec {
            enabled: cfg!(feature = "backend-shim-osx"),
            target: BackendShimTarget::Macos,
            upstream_source: "imgui_impl_osx.mm",
            shim_source: "osx.mm",
            output_lib: "dear_imgui_backend_osx",
            link_libs: OSX_BACKEND_LINK_LIBS,
        },
    ]
}

fn backend_shim_target_matches(target: BackendShimTarget, cfg: &BuildConfig) -> bool {
    match target {
        BackendShimTarget::Any => true,
        BackendShimTarget::Android => cfg.target_os == "android",
        BackendShimTarget::Apple => matches!(cfg.target_os.as_str(), "macos" | "ios"),
        BackendShimTarget::Linux => cfg.target_os == "linux",
        BackendShimTarget::Macos => cfg.target_os == "macos",
        BackendShimTarget::Windows => cfg.is_windows(),
    }
}

fn build_backend_shim_from_spec(cfg: &BuildConfig, spec: &BackendShimSpec) {
    let imgui_src = cfg.imgui_src();
    let shim_root = cfg.manifest_dir.join("backend-shims");
    let mut build = new_native_cpp_build(cfg);
    build.file(imgui_src.join("backends").join(spec.upstream_source));
    build.file(shim_root.join(spec.shim_source));
    if spec.upstream_source.ends_with(".mm") {
        build.flag("-fobjc-arc");
    }
    build_support::compile_cpp_archive(
        &mut build,
        spec.output_lib,
        &cfg.target_os,
        &cfg.target_env,
        &cfg.target_abi,
    );

    for link_lib in spec.link_libs {
        if backend_shim_target_matches(link_lib.target, cfg) {
            println!("cargo:rustc-link-lib={}", link_lib.name);
        }
    }
}

fn export_include_paths(cfg: &BuildConfig) {
    // Export canonical paths even when docs.rs only needs pregenerated bindings.
    // Native compilation validates the source directory separately.
    let cimgui_root = cfg
        .manifest_dir
        .join(&cfg.maintained_source_paths().source().source_root);
    let imgui_src = cimgui_root.join("imgui");
    println!("cargo:THIRD_PARTY={}", imgui_src.display());
    println!("cargo:IMGUI_INCLUDE_PATH={}", imgui_src.display());
    println!(
        "cargo:IMGUI_BACKENDS_PATH={}",
        imgui_src.join("backends").display()
    );
    println!("cargo:CIMGUI_INCLUDE_PATH={}", cimgui_root.display());
    println!(
        "cargo:IMGUI_BACKEND_SHIMS_PATH={}",
        cfg.manifest_dir.join("backend-shims").display()
    );
    for define in cfg
        .binding_spec()
        .clang_defines
        .iter()
        .filter_map(|definition| CrateBindingDefine::from_definition(definition))
        .filter(|define| define.applies_to_native_compilation())
    {
        println!(
            "cargo:DEFINE_{}={}",
            define.name,
            define.value.unwrap_or("1")
        );
    }
    if cfg!(feature = "test-engine") {
        println!("cargo:DEFINE_IMGUI_ENABLE_TEST_ENGINE=1");
        println!("cargo:DEFINE_IMGUITEST=1");
    }
}

fn expected_lib_name(target_env: &str) -> String {
    build_support::expected_lib_name(target_env, "dear_imgui")
}

fn read_prebuilt_manifest(dir: &Path) -> Result<Vec<u8>, String> {
    let mut candidates = vec![dir.join("manifest.txt")];
    if let Some(parent) = dir.parent() {
        candidates.push(parent.join("manifest.txt"));
    }
    for manifest in &candidates {
        // Watching a nonexistent file makes Cargo rebuild on every invocation.
        let watch = if manifest.is_file() {
            manifest.as_path()
        } else {
            manifest.parent().unwrap()
        };
        println!("cargo:rerun-if-changed={}", watch.display());
    }
    for manifest in candidates {
        match std::fs::read(&manifest) {
            Ok(content) => {
                return Ok(content);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!("failed to read {}: {error}", manifest.display()));
            }
        }
    }
    Err(format!(
        "manifest.txt was not found beside prebuilt library directory {}",
        dir.display()
    ))
}

fn validate_prebuilt_artifact_profile(
    dir: &Path,
    cfg: &BuildConfig,
) -> Result<CoreArtifactIdentity, String> {
    let profile = cfg.artifact_profile();
    let manifest = read_prebuilt_manifest(dir)?;
    if manifest_declares_artifact_feature(&manifest, "platform-io-aggregate-hooks-v2") {
        return Err(format!(
            "artifact declares obsolete platform-io-aggregate-hooks-v2; regenerate the archive with {PLATFORM_IO_AGGREGATE_HOOKS_FEATURE} or remove the prebuilt override and use a source build"
        ));
    }
    let identity = profile.validate_release_manifest_bytes(&manifest)?;
    if let Ok(expected_candidate) = env::var(RELEASE_CANDIDATE_SHA_ENV) {
        let expected_identity = CoreArtifactIdentity::new(&profile, &expected_candidate)?;
        if identity != expected_identity {
            return Err(format!(
                "artifact candidate mismatch: expected {}, found {}",
                expected_identity.candidate_sha, identity.candidate_sha
            ));
        }
    }
    Ok(identity)
}

fn manifest_declares_artifact_feature(manifest: &[u8], feature: &str) -> bool {
    std::str::from_utf8(manifest)
        .ok()
        .and_then(|manifest| {
            manifest
                .lines()
                .find_map(|line| line.strip_prefix("features="))
        })
        .is_some_and(|features| features.split(',').any(|value| value == feature))
}

fn find_prebuilt(dir: &Path, cfg: &BuildConfig) -> Option<PrebuiltCore> {
    let lib_name = expected_lib_name(&cfg.target_env);
    let lib_path = dir.join(lib_name.as_str());
    if !lib_path.exists() {
        return None;
    }

    let identity = match validate_prebuilt_artifact_profile(dir, cfg) {
        Ok(identity) => identity,
        Err(error) => {
            println!(
                "cargo:warning=Rejecting incompatible dear_imgui prebuilt at {}: {error}",
                dir.display()
            );
            return None;
        }
    };
    Some(PrebuiltCore {
        directory: dir.to_owned(),
        identity,
    })
}

fn link_prebuilt(artifact: &PrebuiltCore, cfg: &BuildConfig) {
    let dir = &artifact.directory;
    let identity = &artifact.identity;
    let lib_path = dir.join(expected_lib_name(&cfg.target_env));
    println!(
        "cargo:warning=Using prebuilt dear_imgui from {}",
        dir.display()
    );
    println!("cargo:rerun-if-changed={}", lib_path.display());
    println!("cargo:rustc-link-search=native={}", dir.display());
    println!("cargo:rustc-link-lib=static=dear_imgui");
    build_support::emit_prebuilt_cpp_runtime_linkage(
        &cfg.target_os,
        &cfg.target_env,
        &cfg.target_abi,
    );
    #[cfg(feature = "freetype")]
    {
        // A freetype-enabled dear_imgui static prebuilt still references the
        // FreeType library. Emit the same native link metadata as source builds.
        let _ = find_freetype_dependency(true);
    }
    println!("cargo:ARTIFACT_PROFILE_HASH={}", identity.profile_hash);
    println!(
        "cargo:ARTIFACT_IDENTITY_HASH={}",
        identity.deterministic_hash()
    );
    println!("cargo:CANDIDATE_SHA={}", identity.candidate_sha);
}

fn try_download_prebuilt(
    cache_root: &Path,
    url: &str,
    target_env: &str,
) -> Result<PathBuf, String> {
    let lib_name = expected_lib_name(target_env);
    if url.starts_with("http://") || url.starts_with("https://") {
        println!("cargo:warning=Downloading prebuilt dear_imgui from {}", url);
    } else {
        println!("cargo:warning=Using prebuilt dear_imgui from {}", url);
    }
    build_support::download_prebuilt(cache_root, url, lib_name.as_str(), target_env)
}

fn try_download_prebuilt_from_release(cfg: &BuildConfig) -> Option<PathBuf> {
    let version = env::var("CARGO_PKG_VERSION").unwrap_or_default();

    // Candidate archive names match the complete native profile. We still validate the manifest
    // in `try_link_prebuilt()`, but distinct names prevent a normal build from even trying a
    // patched stack-layout artifact (and vice versa).
    let mut candidates = vec![cfg.archive_name(cfg.crt_profile())];
    let crt_fallback = cfg.archive_name("");
    if crt_fallback != candidates[0] {
        candidates.push(crt_fallback);
    }

    let tags = build_support::release_tags("dear-imgui-sys", &version);

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
    let root = build_support::prebuilt_cache_root_from_env_or_target(
        &cfg.manifest_dir,
        "IMGUI_SYS_CACHE_DIR",
        "dear-imgui-prebuilt",
    );
    let mut profile = if cfg!(feature = "stack-layout") {
        "stack-layout".to_string()
    } else {
        "standard".to_string()
    };
    if cfg!(feature = "freetype") {
        profile.push_str("+freetype");
    }
    if cfg!(feature = "test-engine") {
        profile.push_str("+test-engine");
    }
    let identity = cfg
        .artifact_profile()
        .deterministic_hash()
        .replace(':', "-");
    root.join(profile).join(identity)
}

fn use_pregenerated_bindings(cfg: &BuildConfig) -> bool {
    if build_support::parse_bool_env("DEAR_IMGUI_RS_REGEN_BINDINGS") {
        return false;
    }

    let (pregenerated, spec) = if cfg.is_core_wasm_target() {
        (
            Path::new("src").join("wasm_bindings_pregenerated.rs"),
            BindingSpec::core_wasm(core_wasm_import_module()),
        )
    } else {
        let profile = cfg.native_abi_profile();
        (
            Path::new(profile.pregenerated_file()).to_path_buf(),
            BindingSpec::core_native(profile),
        )
    };

    if !pregenerated.exists() {
        return false;
    }
    let content = match std::fs::read_to_string(&pregenerated) {
        Ok(content) => spec.sanitize(&content),
        Err(error) => {
            println!(
                "cargo:warning=Failed to read pregenerated bindings {}: {error}",
                pregenerated.display()
            );
            return false;
        }
    };
    spec.validate_generated_bindings(&content)
        .unwrap_or_else(|error| panic!("invalid {}: {error}", pregenerated.display()));
    if let Err(error) = std::fs::write(cfg.out_dir.join("bindings.rs"), content) {
        println!("cargo:warning=Failed to write pregenerated bindings: {error}");
        return false;
    }
    println!(
        "cargo:warning=Using pregenerated bindings: {}",
        pregenerated.display()
    );
    true
}
