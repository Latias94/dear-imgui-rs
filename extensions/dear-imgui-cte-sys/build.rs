use std::{
    cell::OnceCell,
    env,
    path::{Path, PathBuf},
};

const ARTIFACT_SPEC: build_support::binding::ExtensionArtifactSpec =
    build_support::binding::ExtensionBinding::Cte.artifact_spec();
const CRATE_LABEL: &str = ARTIFACT_SPEC.sys_crate_name;
const LIBRARY_NAME: &str = ARTIFACT_SPEC.library_name;

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
    target: String,
    target_os: String,
    target_env: String,
    target_abi: String,
    target_arch: String,
    docs_rs: bool,
}

impl BuildConfig {
    fn new() -> Self {
        Self {
            manifest_dir: PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()),
            out_dir: PathBuf::from(env::var("OUT_DIR").unwrap()),
            target: env::var("TARGET").unwrap_or_default(),
            target_os: env::var("CARGO_CFG_TARGET_OS").unwrap_or_default(),
            target_env: env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default(),
            target_abi: env::var("CARGO_CFG_TARGET_ABI").unwrap_or_default(),
            target_arch: env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default(),
            docs_rs: env::var_os("DOCS_RS").is_some(),
        }
    }

    fn is_msvc_windows(&self) -> bool {
        self.target_os == "windows" && self.target_env == "msvc"
    }

    fn msvc_crt_suffix(&self) -> Option<&'static str> {
        self.is_msvc_windows()
            .then(|| build_support::msvc_crt_suffix_from_env(Some(&self.target_env)))
            .flatten()
    }

    fn use_static_crt(&self) -> bool {
        self.msvc_crt_suffix() == Some("mt")
    }
}

fn extension_artifact_profile(
    config: &BuildConfig,
    package_mode: bool,
) -> build_support::binding::ExtensionArtifactProfile {
    let crt = config.msvc_crt_suffix().unwrap_or_default();
    build_support::binding::extension_artifact_profile_from_env(
        build_support::binding::ExtensionBinding::Cte,
        &config.manifest_dir,
        env!("CARGO_PKG_VERSION"),
        &config.target,
        crt,
        &["wchar32"],
        package_mode,
    )
    .unwrap_or_else(|error| panic!("{CRATE_LABEL}: {error}"))
}

fn resolve_imgui_includes(config: &BuildConfig) -> (PathBuf, PathBuf) {
    let imgui = env::var_os("DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH")
        .or_else(|| env::var_os("DEP_DEAR_IMGUI_THIRD_PARTY"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            config
                .manifest_dir
                .join("../../dear-imgui-sys/third-party/cimgui/imgui")
        });
    let cimgui = env::var_os("DEP_DEAR_IMGUI_CIMGUI_INCLUDE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            config
                .manifest_dir
                .join("../../dear-imgui-sys/third-party/cimgui")
        });
    (imgui, cimgui)
}

fn native_binding_spec() -> &'static build_support::binding::CrateBindingSpec {
    build_support::binding::CrateBindingSpec::for_crate_and_target(env!("CARGO_PKG_NAME"), "native")
        .expect("missing dear-imgui-cte-sys native binding spec")
}

fn maintained_source_paths(
    config: &BuildConfig,
) -> build_support::source_inventory::MaintainedSourcePaths {
    build_support::source_inventory::MaintainedSourcePaths::for_crate(
        env!("CARGO_PKG_NAME"),
        config.manifest_dir.clone(),
    )
    .unwrap_or_else(|error| panic!("{CRATE_LABEL}: {error}"))
}

fn use_validated_pregenerated_bindings(out_dir: &Path, target: &str) -> bool {
    if build_support::parse_bool_env("DEAR_IMGUI_RS_REGEN_BINDINGS") {
        return false;
    }
    let spec = build_support::binding::CrateBindingSpec::for_crate_and_target(
        env!("CARGO_PKG_NAME"),
        target,
    )
    .unwrap_or_else(|| panic!("missing {CRATE_LABEL} {target} binding spec"));
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let checked_in = crate_root.join(spec.checked_in_path);
    if !checked_in.exists() {
        return false;
    }
    spec.copy_embedded_checked_in_to_out_dir(&crate_root, out_dir)
        .unwrap_or_else(|error| panic!("invalid pregenerated bindings: {error}"));
    println!(
        "cargo:warning=Using validated pregenerated bindings: {}",
        checked_in.display()
    );
    true
}

fn use_pregenerated_bindings(out_dir: &Path) -> bool {
    use_validated_pregenerated_bindings(out_dir, "native")
}

fn use_pregenerated_wasm_bindings(out_dir: &Path) -> bool {
    use_validated_pregenerated_bindings(out_dir, "wasm")
}

#[cfg(feature = "bindgen")]
fn generate_bindings(config: &BuildConfig, source_root: &Path, imgui: &Path, cimgui: &Path) {
    use build_support::binding::{CrateBindingIncludeRoot, CrateBindingLanguage};

    if config.target_arch == "wasm32" {
        if !cfg!(feature = "wasm") {
            panic!("{CRATE_LABEL}: wasm32 targets require the `wasm` feature");
        }
        if use_pregenerated_wasm_bindings(&config.out_dir) {
            return;
        }
        panic!(
            "{CRATE_LABEL}: missing pregenerated WASM bindings; run xtask verify-bindings --update"
        );
    }

    let spec = native_binding_spec();
    let profile = spec.profile;
    let mut builder = bindgen::Builder::default()
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .derive_default(profile.derives.default)
        .derive_debug(profile.derives.debug)
        .derive_copy(profile.derives.copy)
        .derive_eq(profile.derives.eq)
        .derive_partialeq(profile.derives.partial_eq)
        .derive_hash(profile.derives.hash)
        .prepend_enum_name(profile.prepend_enum_name)
        .layout_tests(profile.layout_tests)
        .allowlist_recursively(profile.allowlist_recursively);
    for input in spec.input_paths {
        builder = builder.header(config.manifest_dir.join(input).to_string_lossy());
    }
    for include in profile.include_paths {
        let root = match include.root {
            CrateBindingIncludeRoot::CoreCimgui => cimgui,
            CrateBindingIncludeRoot::CoreImgui => imgui,
            CrateBindingIncludeRoot::Source => source_root,
        };
        builder = builder.clang_arg(format!("-I{}", root.join(include.relative_path).display()));
    }
    for define in spec.binding_defines() {
        builder = builder.clang_arg(define.clang_arg());
    }
    for argument in spec.clang_args {
        builder = builder.clang_arg(*argument);
    }
    for pattern in profile.allowlisted_functions {
        builder = builder.allowlist_function(pattern);
    }
    for pattern in profile.allowlisted_types {
        builder = builder.allowlist_type(pattern);
    }
    for pattern in profile.allowlisted_vars {
        builder = builder.allowlist_var(pattern);
    }
    for pattern in profile.blocklisted_types {
        builder = builder.blocklist_type(pattern);
    }
    builder = if profile.language == CrateBindingLanguage::C {
        builder
    } else {
        builder
            .clang_arg("-x")
            .clang_arg("c++")
            .clang_arg(format!("-std={}", profile.language.id()))
    };
    let bindings = builder
        .generate()
        .expect("unable to generate cimCTE bindings");
    let output = config.out_dir.join("bindings.rs");
    bindings
        .write_to_file(&output)
        .expect("could not write cimCTE bindings");
    sanitize_bindings_file(&output);
}

#[cfg(feature = "bindgen")]
fn sanitize_bindings_file(path: &Path) {
    if let Ok(content) = std::fs::read_to_string(path) {
        let mut output = String::with_capacity(content.len());
        let mut skip_blank = false;
        for line in content.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("#![") {
                skip_blank = true;
                continue;
            }
            if skip_blank && trimmed.is_empty() {
                continue;
            }
            skip_blank = false;
            output.push_str(line);
            output.push('\n');
        }
        std::fs::write(path, output).expect("could not sanitize cimCTE bindings");
    }
}

#[cfg(not(feature = "bindgen"))]
fn generate_bindings(_config: &BuildConfig, _source_root: &Path, _imgui: &Path, _cimgui: &Path) {
    panic!(
        "{CRATE_LABEL}: regenerating bindings requires the `bindgen` feature and DEAR_IMGUI_RS_REGEN_BINDINGS=1"
    );
}

fn expected_lib_name(target_env: &str) -> String {
    build_support::expected_lib_name(target_env, LIBRARY_NAME)
}

fn try_link_prebuilt(
    directory: PathBuf,
    config: &BuildConfig,
    profile: &OnceCell<build_support::binding::ExtensionArtifactProfile>,
) -> bool {
    let library = directory.join(expected_lib_name(&config.target_env));
    if !library.exists() {
        return false;
    }
    profile
        .get_or_init(|| extension_artifact_profile(config, false))
        .validate_prebuilt_dir(&directory)
        .unwrap_or_else(|error| panic!("{CRATE_LABEL}: incompatible prebuilt artifact: {error}"));
    println!("cargo:rustc-link-search=native={}", directory.display());
    println!("cargo:rustc-link-lib=static={LIBRARY_NAME}");
    build_support::emit_prebuilt_cpp_runtime_linkage(
        &config.target_os,
        &config.target_env,
        &config.target_abi,
    );
    true
}

fn prebuilt_cache_root(
    config: &BuildConfig,
    profile: &build_support::binding::ExtensionArtifactProfile,
) -> PathBuf {
    build_support::prebuilt_cache_root_from_env_or_target(
        &config.manifest_dir,
        "CTE_SYS_CACHE_DIR",
        "dear-imgui-cte-prebuilt",
    )
    .join(profile.cache_key())
}

fn try_download_prebuilt(
    cache_root: &Path,
    url: &str,
    target_env: &str,
) -> Result<PathBuf, String> {
    let library_name = expected_lib_name(target_env);
    build_support::download_prebuilt(cache_root, url, &library_name, target_env)
}

fn try_download_prebuilt_from_release(
    config: &BuildConfig,
    profile: &OnceCell<build_support::binding::ExtensionArtifactProfile>,
) -> Option<PathBuf> {
    let profile = profile.get_or_init(|| extension_artifact_profile(config, false));
    let tags = build_support::release_tags(CRATE_LABEL, &profile.version);
    if let Ok(package_dir) = env::var("CTE_SYS_PACKAGE_DIR") {
        let archive = PathBuf::from(package_dir).join(&profile.archive_name);
        let library_name = expected_lib_name(&config.target_env);
        if archive.exists()
            && let Ok(library_dir) = build_support::extract_archive_to_cache(
                &archive,
                &prebuilt_cache_root(config, profile),
                &library_name,
            )
        {
            return Some(library_dir);
        }
    }
    if build_support::is_offline() {
        return None;
    }
    let cache_root = prebuilt_cache_root(config, profile);
    for url in build_support::release_candidate_urls_env(
        &tags,
        std::slice::from_ref(&profile.archive_name),
    ) {
        if let Ok(directory) = try_download_prebuilt(&cache_root, &url, &config.target_env) {
            return Some(directory);
        }
    }
    None
}

fn try_link_prebuilt_all(config: &BuildConfig) -> bool {
    let profile = OnceCell::new();
    if let Ok(directory) = env::var("CTE_SYS_LIB_DIR") {
        if try_link_prebuilt(PathBuf::from(&directory), config, &profile) {
            return true;
        }
        println!("cargo:warning=CTE_SYS_LIB_DIR set but library not found in {directory}");
    }
    if let Ok(url) = env::var("CTE_SYS_PREBUILT_URL") {
        if (is_http_url(&url) || is_archive_urlish(&url)) && !cfg!(feature = "prebuilt") {
            println!(
                "cargo:warning=CTE_SYS_PREBUILT_URL requires the `prebuilt` feature for downloads or archive extraction"
            );
            return false;
        }
        let profile_value = profile.get_or_init(|| extension_artifact_profile(config, false));
        if let Ok(directory) = try_download_prebuilt(
            &prebuilt_cache_root(config, profile_value),
            &url,
            &config.target_env,
        ) && try_link_prebuilt(directory, config, &profile)
        {
            return true;
        }
    } else {
        let feature_enabled = cfg!(feature = "prebuilt");
        let env_enabled = build_support::parse_bool_env("CTE_SYS_USE_PREBUILT");
        if env_enabled && !feature_enabled {
            println!(
                "cargo:warning=CTE_SYS_USE_PREBUILT is set but the `prebuilt` feature is disabled"
            );
        }
        if feature_enabled
            && let Some(directory) = try_download_prebuilt_from_release(config, &profile)
            && try_link_prebuilt(directory, config, &profile)
        {
            return true;
        }
    }
    false
}

fn build_with_cc(
    config: &BuildConfig,
    native_sources: &[PathBuf],
    source_root: &Path,
    imgui: &Path,
    cimgui: &Path,
) {
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std(native_binding_spec().profile.language.id());
    build_support::configure_cpp_runtime_linkage(
        &mut build,
        &config.target_os,
        &config.target_env,
        &config.target_abi,
    );
    build_support::configure_cpp_no_exceptions(&mut build, &config.target_env);

    if config.is_msvc_windows() {
        let static_crt = config.use_static_crt();
        build.static_crt(static_crt);
        if env::var("PROFILE").as_deref() == Ok("debug") {
            build.debug(true).opt_level(0);
        } else {
            build.debug(false).opt_level(2);
        }
        build.flag("/D_ITERATOR_DEBUG_LEVEL=0");
    }

    native_binding_spec().apply_extension_binding_defines(&mut build, env::vars());
    build
        .include(imgui)
        .include(cimgui)
        .include(source_root)
        .include(source_root.join("ImGuiColorTextEdit"));
    for source in native_sources {
        build.file(source);
    }
    build_support::compile_cpp_archive(
        &mut build,
        LIBRARY_NAME,
        &config.target_os,
        &config.target_env,
        &config.target_abi,
    );
}

fn docsrs_build(config: &BuildConfig, source_root: &Path, imgui: &Path, cimgui: &Path) {
    println!("cargo:rustc-cfg=docsrs");
    if !use_pregenerated_bindings(&config.out_dir) {
        generate_bindings(config, source_root, imgui, cimgui);
    }
}

fn emit_rerun_directives(sources: &build_support::source_inventory::MaintainedSourcePaths) {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/bindings_pregenerated.rs");
    println!("cargo:rerun-if-changed=src/wasm_bindings_pregenerated.rs");
    println!("cargo:rerun-if-changed=shim/cte_bridge.h");
    for path in sources.native_candidate_paths() {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    println!("cargo:rerun-if-changed=../../dear-imgui-sys");
    for variable in [
        "CTE_SYS_LIB_DIR",
        "CTE_SYS_SKIP_CC",
        "CTE_SYS_PREBUILT_URL",
        "CTE_SYS_FORCE_BUILD",
        "CTE_SYS_CACHE_DIR",
        "CTE_SYS_PACKAGE_DIR",
        "CTE_SYS_USE_PREBUILT",
        "DEAR_IMGUI_RS_REGEN_BINDINGS",
        "DEAR_IMGUI_RS_CANDIDATE_SHA",
        "DEAR_IMGUI_CORE_ARTIFACT_IDENTITY_HASH",
        "DEP_DEAR_IMGUI_ARTIFACT_IDENTITY_HASH",
        "DEP_DEAR_IMGUI_CANDIDATE_SHA",
        "CARGO_CFG_TARGET_ABI",
        "DOCS_RS",
    ] {
        println!("cargo:rerun-if-env-changed={variable}");
    }
}

fn main() {
    let config = BuildConfig::new();
    let sources = maintained_source_paths(&config);
    emit_rerun_directives(&sources);

    if cfg!(feature = "package-bin") {
        let profile = extension_artifact_profile(&config, true);
        profile
            .write_package_metadata(&config.out_dir)
            .unwrap_or_else(|error| panic!("{CRATE_LABEL}: {error}"));
        println!(
            "cargo:rustc-env=DEAR_IMGUI_EXTENSION_ARTIFACT_TARGET={}",
            profile.target
        );
        println!(
            "cargo:rustc-env=DEAR_IMGUI_EXTENSION_ARTIFACT_CRT={}",
            profile.crt
        );
    }

    let (imgui, cimgui) = resolve_imgui_includes(&config);
    let source_root = sources
        .source_root()
        .unwrap_or_else(|error| panic!("{CRATE_LABEL}: {error}"));
    if config.docs_rs {
        docsrs_build(&config, &source_root, &imgui, &cimgui);
        return;
    }

    if build_support::parse_bool_env("DEAR_IMGUI_RS_REGEN_BINDINGS") {
        generate_bindings(&config, &source_root, &imgui, &cimgui);
        return;
    }

    let skip_cc = env::var_os("CTE_SYS_SKIP_CC").is_some();
    let bindings_ready = if config.target_arch == "wasm32" {
        if !cfg!(feature = "wasm") {
            panic!("{CRATE_LABEL}: wasm32 targets require the `wasm` feature");
        }
        use_pregenerated_wasm_bindings(&config.out_dir)
    } else {
        use_pregenerated_bindings(&config.out_dir)
    };
    if !bindings_ready {
        assert!(!skip_cc, "CTE_SYS_SKIP_CC requires pregenerated bindings");
        generate_bindings(&config, &source_root, &imgui, &cimgui);
    }
    if config.target_arch == "wasm32" {
        println!("cargo:warning=Skipping native cimCTE build for wasm32");
        return;
    }
    if skip_cc {
        let _ = try_link_prebuilt_all(&config);
        return;
    }

    let force_source = cfg!(feature = "package-bin")
        || cfg!(feature = "build-from-source")
        || build_support::parse_bool_env("CTE_SYS_FORCE_BUILD");
    let linked_prebuilt = !force_source && try_link_prebuilt_all(&config);
    if !linked_prebuilt {
        let native_sources = sources
            .validate_native()
            .unwrap_or_else(|error| panic!("{CRATE_LABEL}: {error}"));
        build_with_cc(&config, &native_sources, &source_root, &imgui, &cimgui);
    }
}
