//! Build-script entry point for the ImGui extensions that share the core ABI.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use crate::binding::{
    BindingOwner::{self, Extension, TestEngine},
    CrateBindingSpec, DEP_CORE_ARTIFACT_IDENTITY_HASH_ENV, DEP_CORE_CANDIDATE_SHA_ENV,
    ExtensionArtifactProfile,
    ExtensionBinding::{self, Cte, ImGuizmo, ImGuizmoQuat, ImNodes, ImPlot, ImPlot3d, NodeEditor},
    RELEASE_CANDIDATE_SHA_ENV, RELEASE_CORE_ARTIFACT_IDENTITY_HASH_ENV,
    extension_artifact_profile_from_env,
};
use crate::native_build::{NativeBuildMode, NativeBuildPlan, is_packaging};
use crate::source_inventory::MaintainedSourcePaths;

const REGEN_BINDINGS_ENV: &str = "DEAR_IMGUI_RS_REGEN_BINDINGS";
const TEST_ENGINE_SKIP_CC_ENV: &str = "IMGUI_TEST_ENGINE_SYS_SKIP_CC";

pub struct ExtensionSysSpec {
    pub owner: BindingOwner,
    pub env_prefix: &'static str,
    // Capture the caller's feature state before Cargo unifies build-support features.
    pub bindgen: bool,
    pub wasm: bool,
    pub prebuilt: bool,
    pub build_from_source: bool,
    pub artifact_features: &'static [(&'static str, bool)],
    pub cpp_profile: CppProfile,
    pub includes: &'static [CppInclude],
    pub force_include: Option<&'static str>,
    pub defines: &'static [(&'static str, &'static str)],
    pub overlays: &'static [(&'static str, SourceOverlay)],
    /// Inventory entries required by the build that are included by other translation units.
    pub include_only: &'static [&'static str],
    pub rerun_paths: &'static [&'static str],
    pub probe: Option<NativeProbe>,
    pub source_hooks: Option<SourceHooks>,
}

impl ExtensionSysSpec {
    pub const fn new(owner: BindingOwner, env_prefix: &'static str) -> Self {
        Self {
            owner,
            env_prefix,
            bindgen: false,
            wasm: false,
            prebuilt: false,
            build_from_source: false,
            artifact_features: &[],
            cpp_profile: CppProfile::Standard,
            includes: &[],
            force_include: None,
            defines: &[],
            overlays: &[],
            include_only: &[],
            rerun_paths: &[],
            probe: None,
            source_hooks: None,
        }
    }
}

#[derive(Clone, Copy)]
pub enum CppProfile {
    Standard,
    ImPlot,
    Cte,
    CrtOnly,
}

pub enum CppInclude {
    Crate(&'static str),
    Source(&'static str),
}

pub enum SourceOverlay {
    ImPlot,
    ImPlot3d,
    Text {
        output: &'static str,
        patch: fn(&str) -> Result<String, String>,
    },
}

pub struct NativeProbe {
    pub feature: &'static str,
    pub enabled: bool,
    pub source: &'static str,
    pub library: &'static str,
    pub cpp_profile: CppProfile,
}

pub struct SourceHooks {
    pub check: fn(&ExtensionBuildContext<'_>, bool) -> Result<(), String>,
    pub build: fn(&ExtensionBuildContext<'_>) -> Result<(), String>,
}

pub struct ExtensionBuildContext<'a> {
    spec: &'a ExtensionSysSpec,
    binding: &'static CrateBindingSpec,
    source_root: PathBuf,
    pub sources: MaintainedSourcePaths,
    pub out_dir: PathBuf,
    pub imgui: PathBuf,
    pub cimgui: PathBuf,
    pub target_os: String,
    pub target_env: String,
    pub target_abi: String,
    target: String,
    wasm_target: bool,
    docs_rs: bool,
    regenerate: bool,
    crt: &'static str,
}

/// Resolve the binding and native-library routes for one sys crate.
pub fn build_extension_sys(spec: ExtensionSysSpec) {
    let crate_label = match spec.owner {
        Extension(extension) => extension.artifact_spec().sys_crate_name,
        TestEngine => "dear-imgui-test-engine-sys",
    };
    let packaging = is_packaging(crate_label);
    ExtensionBuildContext::new(&spec, crate_label)
        .and_then(|context| context.run(packaging))
        .unwrap_or_else(|error| panic!("{crate_label}: {error}"));
}

impl<'a> ExtensionBuildContext<'a> {
    fn new(spec: &'a ExtensionSysSpec, crate_label: &str) -> Result<Self, String> {
        let binding = CrateBindingSpec::for_crate_and_target(crate_label, "native")
            .ok_or_else(|| "missing native binding spec".to_owned())?;
        let crate_root = PathBuf::from(
            env::var_os("CARGO_MANIFEST_DIR").expect("Cargo must set CARGO_MANIFEST_DIR"),
        );
        let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo must set OUT_DIR"));
        let sources = MaintainedSourcePaths::for_crate(crate_label, crate_root)
            .map_err(|error| error.to_string())?;
        let source_root = sources.crate_root().join(&sources.source().source_root);
        let local_cimgui = sources
            .crate_root()
            .join("../../dear-imgui-sys/third-party/cimgui");
        let imgui = env::var_os("DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH")
            .or_else(|| env::var_os("DEP_DEAR_IMGUI_THIRD_PARTY"))
            .map_or_else(|| local_cimgui.join("imgui"), PathBuf::from);
        let cimgui =
            env::var_os("DEP_DEAR_IMGUI_CIMGUI_INCLUDE_PATH").map_or(local_cimgui, PathBuf::from);
        let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
        let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
        let crt = if target_os == "windows" {
            crate::msvc_crt_suffix_from_env(Some(&target_env)).unwrap_or_default()
        } else {
            ""
        };
        let docs_rs = if spec.owner == Extension(Cte) {
            env::var_os("DOCS_RS").is_some()
        } else {
            env::var("DOCS_RS").is_ok()
        };
        let regenerate = if spec.owner == TestEngine {
            matches!(
                env::var(REGEN_BINDINGS_ENV).as_deref(),
                Ok("1" | "true" | "yes" | "on")
            )
        } else {
            crate::parse_bool_env(REGEN_BINDINGS_ENV)
        };
        Ok(Self {
            spec,
            binding,
            source_root,
            sources,
            out_dir,
            imgui,
            cimgui,
            target_os,
            target_env,
            target_abi: env::var("CARGO_CFG_TARGET_ABI").unwrap_or_default(),
            target: env::var("TARGET").unwrap_or_default(),
            wasm_target: env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32"),
            docs_rs,
            regenerate,
            crt,
        })
    }

    fn run(&self, packaging: bool) -> Result<(), String> {
        self.emit_rerun_directives();
        if packaging {
            if self.spec.owner == TestEngine {
                return Err(
                    "Test Engine is source-only and cannot be packaged as a prebuilt artifact"
                        .into(),
                );
            }
            if self.docs_rs || self.wasm_target || self.regenerate {
                return Err("packaging requires a native source build".into());
            }
        }
        if self.docs_rs {
            return self.build_docs();
        }
        if self.spec.owner == TestEngine && self.wasm_target {
            return self.build_wasm();
        }
        // Test Engine has a presence-based legacy check-only route of its own.
        if self.spec.owner == TestEngine && env::var(TEST_ENGINE_SKIP_CC_ENV).is_ok() {
            return self.require_native_bindings(
                "IMGUI_TEST_ENGINE_SYS_SKIP_CC is set but no pregenerated bindings were found. \
                 Ensure src/bindings_pregenerated.rs exists, or unset IMGUI_TEST_ENGINE_SYS_SKIP_CC.",
            );
        }
        if self.regenerate {
            self.check_source_paths(false)?;
            return self.generate_bindings();
        }

        if self.wasm_target {
            return self.build_wasm();
        }

        let mut package_profile = None;
        if let Extension(extension) = self.spec.owner {
            let plan = NativeBuildPlan::from_env(
                self.spec.env_prefix,
                packaging || self.spec.build_from_source,
                self.spec.prebuilt,
            )?;
            if let Some(probe) = self.spec.probe.as_ref().filter(|probe| probe.enabled) {
                if plan.mode() == NativeBuildMode::BindingsOnly {
                    return Err(format!(
                        "feature `{}` requires native compilation",
                        probe.feature
                    ));
                }
                let mut build = self.extension_cpp_build(probe.cpp_profile);
                build.file(self.sources.crate_root().join(probe.source));
                self.compile_archive(&mut build, probe.library);
            }
            match plan.mode() {
                NativeBuildMode::BindingsOnly => {
                    return self
                        .require_native_bindings("bindings-only requires pregenerated bindings");
                }
                NativeBuildMode::Auto | NativeBuildMode::Prebuilt => {
                    if plan.finish_prebuilt_lookup(self.try_link_prebuilt(extension, &plan)?)? {
                        return self.require_native_bindings(
                            "prebuilt mode requires pregenerated bindings",
                        );
                    }
                }
                NativeBuildMode::Source => {}
            }
            if packaging {
                package_profile = Some(self.artifact_profile(extension, true)?);
            }
        }

        self.check_source_paths(false)?;
        if !self.copy_bindings(self.binding)? {
            self.generate_bindings()?;
        }
        self.build_native()?;
        if let Some(profile) = package_profile {
            profile.write_package_metadata(&self.out_dir)?;
            println!(
                "cargo:rustc-env=DEAR_IMGUI_EXTENSION_ARTIFACT_TARGET={}",
                profile.target
            );
            println!(
                "cargo:rustc-env=DEAR_IMGUI_EXTENSION_ARTIFACT_CRT={}",
                profile.crt
            );
        }
        Ok(())
    }

    fn emit_rerun_directives(&self) {
        let crate_root = self.sources.crate_root();
        for relative in ["build.rs", "Cargo.toml"]
            .into_iter()
            .chain(self.spec.rerun_paths.iter().copied())
        {
            watch_existing(&crate_root.join(relative));
        }
        for binding in CrateBindingSpec::for_owner(self.spec.owner) {
            let path = crate_root.join(binding.checked_in_path);
            // Detect a newly added checked-in binding after generating a local fallback.
            watch_existing(if path.exists() {
                &path
            } else {
                path.parent().expect("binding parent")
            });
            for header in binding.input_paths {
                watch_existing(&crate_root.join(header));
            }
        }
        for path in self.sources.native_candidate_paths() {
            watch_existing(&path);
        }
        if let Some(probe) = &self.spec.probe {
            watch_existing(&crate_root.join(probe.source));
        }
        for header in [
            "imgui.h",
            "imgui_internal.h",
            "imconfig.h",
            "imstb_rectpack.h",
            "imstb_textedit.h",
            "imstb_truetype.h",
        ] {
            watch_existing(&self.imgui.join(header));
        }
        watch_existing(&self.cimgui.join("cimgui.h"));
        for variable in [
            "DOCS_RS",
            REGEN_BINDINGS_ENV,
            "CARGO_CFG_TARGET_ABI",
            "DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH",
            "DEP_DEAR_IMGUI_THIRD_PARTY",
            "DEP_DEAR_IMGUI_CIMGUI_INCLUDE_PATH",
        ] {
            println!("cargo:rerun-if-env-changed={variable}");
        }
        if self.spec.owner == TestEngine {
            watch_existing(&self.imgui.join("backends/imgui_impl_null.cpp"));
            for variable in [
                TEST_ENGINE_SKIP_CC_ENV,
                "DEP_DEAR_IMGUI_DEFINE_IMGUI_ENABLE_TEST_ENGINE",
                "DEP_DEAR_IMGUI_DEFINE_IMGUITEST",
            ] {
                println!("cargo:rerun-if-env-changed={variable}");
            }
        } else {
            for variable in [
                RELEASE_CANDIDATE_SHA_ENV,
                RELEASE_CORE_ARTIFACT_IDENTITY_HASH_ENV,
                DEP_CORE_ARTIFACT_IDENTITY_HASH_ENV,
                DEP_CORE_CANDIDATE_SHA_ENV,
            ] {
                println!("cargo:rerun-if-env-changed={variable}");
            }
        }
    }

    fn copy_bindings(&self, binding: &CrateBindingSpec) -> Result<bool, String> {
        let crate_root = self.sources.crate_root();
        let input = crate_root.join(binding.checked_in_path);
        if self.regenerate || !input.exists() {
            return Ok(false);
        }
        binding
            .copy_embedded_checked_in_to_out_dir(crate_root, &self.out_dir)
            .map_err(|error| format!("invalid pregenerated bindings: {error}"))?;
        println!(
            "cargo:warning=Using validated pregenerated bindings: {}",
            input.display()
        );
        Ok(true)
    }

    fn require_native_bindings(&self, message: &str) -> Result<(), String> {
        if self.copy_bindings(self.binding)? {
            Ok(())
        } else {
            Err(message.to_owned())
        }
    }

    fn build_wasm(&self) -> Result<(), String> {
        let binding = CrateBindingSpec::for_crate_and_target(self.binding.crate_name, "wasm")
            .ok_or_else(|| match self.spec.owner {
                Extension(NodeEditor) => "this crate is native-only; use dear-imnodes for wasm32 node editors",
                _ => "this crate does not support wasm32 targets; the upstream Test Engine is native-only",
            }.to_owned())?;
        if !self.spec.wasm {
            return Err("wasm32 targets require the `wasm` feature".into());
        }
        if self.copy_bindings(binding)? {
            return Ok(());
        }
        Err("WASM requires pregenerated bindings. Use xtask to generate them and unset DEAR_IMGUI_RS_REGEN_BINDINGS for the WASM build.".into())
    }

    fn build_docs(&self) -> Result<(), String> {
        println!("cargo:rustc-cfg=docsrs");
        if self.copy_bindings(self.binding)? {
            return Ok(());
        }
        if self.spec.owner == Extension(NodeEditor) {
            return Err("DOCS_RS build requires src/bindings_pregenerated.rs".into());
        }
        self.check_source_paths(true)?;
        self.generate_bindings()?;
        if matches!(self.spec.owner, Extension(ImPlot | ImPlot3d | ImNodes)) {
            println!("cargo:IMGUI_INCLUDE_PATH={}", self.imgui.display());
            println!("cargo:CIMGUI_INCLUDE_PATH={}", self.cimgui.display());
        }
        Ok(())
    }

    fn check_source_paths(&self, docs_rs: bool) -> Result<(), String> {
        if let Some(hooks) = &self.spec.source_hooks {
            return (hooks.check)(self, docs_rs);
        }
        if self.spec.owner == Extension(Cte) {
            return Ok(());
        }
        let needs_cimgui = docs_rs || matches!(self.spec.owner, Extension(ImGuizmo | ImGuizmoQuat));
        if !self.imgui.exists()
            || !self.source_root.exists()
            || (needs_cimgui && !self.cimgui.exists())
        {
            return Err(format!(
                "required source directories missing (ImGui: {}, cimgui: {}, extension: {}). \
                 Initialize the git submodules or provide the exported ImGui include paths.",
                self.imgui.display(),
                self.cimgui.display(),
                self.source_root.display(),
            ));
        }
        Ok(())
    }

    fn generate_bindings(&self) -> Result<(), String> {
        // build-support's optional dependency may be enabled by a different sys crate.
        if !self.spec.bindgen || !cfg!(feature = "bindgen") {
            return Err("regenerating bindings requires the `bindgen` feature. \
                        Re-run with `--features bindgen` and DEAR_IMGUI_RS_REGEN_BINDINGS=1."
                .into());
        }
        if self.wasm_target && self.spec.owner != TestEngine {
            return self.build_wasm();
        }
        #[cfg(feature = "bindgen")]
        return generate_native_bindings(self);
        #[cfg(not(feature = "bindgen"))]
        unreachable!("checked bindgen feature above");
    }

    fn artifact_profile(
        &self,
        extension: ExtensionBinding,
        packaging: bool,
    ) -> Result<ExtensionArtifactProfile, String> {
        let mut features = vec!["wchar32"];
        features.extend(
            self.spec
                .artifact_features
                .iter()
                .filter_map(|&(feature, enabled)| enabled.then_some(feature)),
        );
        extension_artifact_profile_from_env(
            extension,
            self.sources.crate_root(),
            &env::var("CARGO_PKG_VERSION").expect("Cargo must set CARGO_PKG_VERSION"),
            &self.target,
            self.crt,
            &features,
            packaging,
        )
    }

    fn try_link_prebuilt(
        &self,
        extension: ExtensionBinding,
        plan: &NativeBuildPlan,
    ) -> Result<bool, String> {
        let explicit = plan.prebuilt_source();
        if explicit.is_none()
            && (!self.spec.prebuilt || env::var_os(DEP_CORE_ARTIFACT_IDENTITY_HASH_ENV).is_none())
        {
            return Ok(false);
        }
        let profile = self.artifact_profile(extension, false)?;
        let library = crate::expected_lib_name(&self.target_env, &profile.library_name);
        let archives = std::slice::from_ref(&profile.archive_name);
        let cache = crate::prebuilt_cache_root_from_env_or_target(
            self.sources.crate_root(),
            &format!("{}_CACHE_DIR", self.spec.env_prefix),
            &format!("{}-prebuilt", profile.safe_crate_name),
        )
        .join(profile.cache_key());
        let directory = if let Some(source) = explicit {
            source.acquire(&cache, &library, &self.target_env, archives)?
        } else {
            let tags = crate::release_tags(&profile.sys_crate_name, &profile.version);
            let directory = crate::release_candidate_urls_env(&tags, archives)
                .into_iter()
                .find_map(|url| {
                    match crate::download_prebuilt(&cache, &url, &library, &self.target_env) {
                        Ok(directory) => Some(directory),
                        Err(error) => {
                            println!("cargo:warning=Skipping prebuilt candidate {url}: {error}");
                            None
                        }
                    }
                });
            let Some(directory) = directory else {
                return Ok(false);
            };
            directory
        };
        let validation = if directory.join(&library).is_file() {
            profile.validate_prebuilt_dir(&directory)
        } else {
            Err(format!(
                "prebuilt library {library} disappeared from {}",
                directory.display()
            ))
        };
        if let Err(error) = validation {
            if explicit.is_some() {
                return Err(format!("incompatible explicit prebuilt artifact: {error}"));
            }
            println!("cargo:warning=Rejecting incompatible automatic prebuilt: {error}");
            return Ok(false);
        }
        println!("cargo:rustc-link-search=native={}", directory.display());
        println!("cargo:rustc-link-lib=static={}", profile.library_name);
        if extension == Cte {
            crate::emit_prebuilt_cpp_runtime_linkage(
                &self.target_os,
                &self.target_env,
                &self.target_abi,
            );
        }
        Ok(true)
    }

    fn build_native(&self) -> Result<(), String> {
        let sources = self
            .sources
            .validate_native()
            .map_err(|error| error.to_string())?;
        if let Some(hooks) = &self.spec.source_hooks {
            return (hooks.build)(self);
        }
        let Extension(extension) = self.spec.owner else {
            return Err("the source-only Test Engine build hook is missing".into());
        };
        let mut build = self.extension_cpp_build(self.spec.cpp_profile);
        for (id, source) in self
            .sources
            .source()
            .native_required_files
            .iter()
            .zip(sources)
        {
            if self.spec.include_only.contains(&id.as_str()) {
                continue;
            }
            if let Some((_, overlay)) = self
                .spec
                .overlays
                .iter()
                .find(|(file_id, _)| *file_id == id.as_str())
            {
                build.file(match overlay {
                    SourceOverlay::ImPlot => {
                        let file_name = source
                            .file_name()
                            .and_then(|name| name.to_str())
                            .ok_or("invalid ImPlot source name")?;
                        patch_cpp_file(
                            &source,
                            self.out_dir.join(format!("implot-{id}-imgui-193.cpp")),
                            |contents| crate::patch_implot_cpp_for_imgui(contents, file_name),
                        )?
                    }
                    SourceOverlay::ImPlot3d => {
                        crate::stage_implot3d_draw_flags(&source, &self.out_dir)?
                    }
                    SourceOverlay::Text { output, patch } => {
                        patch_cpp_file(&source, self.out_dir.join(output), *patch)?
                    }
                });
            } else {
                build.file(source);
            }
        }
        self.compile_archive(&mut build, extension.artifact_spec().library_name);
        Ok(())
    }

    fn extension_cpp_build(&self, profile: CppProfile) -> cc::Build {
        let mut build = self.cpp_build(profile);
        build
            .include(&self.imgui)
            .include(&self.cimgui)
            .include(&self.source_root);
        for include in self.spec.includes {
            build.include(match include {
                CppInclude::Crate(path) => self.sources.crate_root().join(path),
                CppInclude::Source(path) => self.source_root.join(path),
            });
        }
        for &(name, value) in self.spec.defines {
            build.define(name, Some(value));
        }
        if let Some(header) = self.spec.force_include {
            if self.target_env == "msvc" {
                build.flag(format!("/FI{header}"));
            } else {
                build.flag("-include").flag(header);
            }
        }
        build
    }

    pub fn cpp_build(&self, profile: CppProfile) -> cc::Build {
        let mut build = cc::Build::new();
        let language = if self.spec.owner == TestEngine {
            "c++17"
        } else {
            self.binding.profile.language.id()
        };
        build.cpp(true).std(language);
        crate::configure_cpp_runtime_linkage(
            &mut build,
            &self.target_os,
            &self.target_env,
            &self.target_abi,
        );
        self.binding
            .apply_extension_binding_defines(&mut build, env::vars());
        if matches!(profile, CppProfile::Cte) {
            crate::configure_cpp_no_exceptions(&mut build, &self.target_env);
        }
        if self.crt.is_empty() {
            return build;
        }
        let static_crt = self.crt == "mt";
        build.static_crt(static_crt);
        if matches!(profile, CppProfile::CrtOnly) {
            return build;
        }
        if env::var("PROFILE").as_deref() == Ok("debug") {
            build.debug(true).opt_level(0);
        } else {
            build.debug(false).opt_level(2);
        }
        if !matches!(profile, CppProfile::Cte) {
            build
                .flag("/EHsc")
                .flag(if static_crt { "/MT" } else { "/MD" });
        }
        if matches!(profile, CppProfile::ImPlot) {
            build.define("_ITERATOR_DEBUG_LEVEL", Some("0"));
        } else {
            build.flag("/D_ITERATOR_DEBUG_LEVEL=0");
        }
        if matches!(profile, CppProfile::Cte) {
            build.flag("/utf-8");
        }
        build
    }

    pub fn compile_archive(&self, build: &mut cc::Build, library: &str) {
        crate::compile_cpp_archive(
            build,
            library,
            &self.target_os,
            &self.target_env,
            &self.target_abi,
        );
    }
}

fn watch_existing(path: &Path) {
    if path.exists() {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

pub fn patch_cpp_file<E: std::fmt::Display>(
    input: &Path,
    output: PathBuf,
    patch: impl FnOnce(&str) -> Result<String, E>,
) -> Result<PathBuf, String> {
    let contents =
        fs::read_to_string(input).map_err(|error| format!("read {}: {error}", input.display()))?;
    let contents =
        patch(&contents).map_err(|error| format!("patch {}: {error}", input.display()))?;
    fs::write(&output, contents).map_err(|error| format!("write {}: {error}", output.display()))?;
    Ok(output)
}

#[cfg(feature = "bindgen")]
fn generate_native_bindings(context: &ExtensionBuildContext<'_>) -> Result<(), String> {
    use crate::binding::{CrateBindingIncludeRoot, CrateBindingLanguage};

    let binding = context.binding;
    let profile = binding.profile;
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
    for header in binding.input_paths {
        builder = builder.header(context.sources.crate_root().join(header).to_string_lossy());
    }
    for include in profile.include_paths {
        let root = match include.root {
            CrateBindingIncludeRoot::CoreCimgui => &context.cimgui,
            CrateBindingIncludeRoot::CoreImgui => &context.imgui,
            CrateBindingIncludeRoot::Source => &context.source_root,
        };
        builder = builder.clang_arg(format!("-I{}", root.join(include.relative_path).display()));
    }
    for define in binding.binding_defines() {
        builder = builder.clang_arg(define.clang_arg());
    }
    if context.spec.owner == Extension(Cte) {
        // CTE has always used the fixed target and -nostdinc. Provide the same
        // fallback headers as xtask, after caller-supplied include directories.
        let shims = context.out_dir.join("bindgen-headers");
        fs::create_dir_all(&shims)
            .map_err(|error| format!("create {}: {error}", shims.display()))?;
        for shim in binding.header_shims {
            let path = shims.join(shim.name);
            // CargoCallbacks tracks included files, so keep unchanged shim timestamps.
            if !fs::read(&path).is_ok_and(|bytes| bytes == shim.contents.as_bytes()) {
                fs::write(&path, shim.contents)
                    .map_err(|error| format!("write {}: {error}", path.display()))?;
            }
        }
        builder = builder
            .clang_args(binding.clang_args.iter().copied())
            .clang_arg("-idirafter")
            .clang_arg(shims.to_string_lossy());
    }
    for pattern in profile.allowlisted_functions {
        builder = builder.allowlist_function(*pattern);
    }
    for pattern in profile.allowlisted_types {
        builder = builder.allowlist_type(*pattern);
    }
    for pattern in profile.allowlisted_vars {
        builder = builder.allowlist_var(*pattern);
    }
    for pattern in profile.blocklisted_types {
        builder = builder.blocklist_type(*pattern);
    }
    if profile.language != CrateBindingLanguage::C {
        builder = builder
            .clang_arg("-x")
            .clang_arg("c++")
            .clang_arg(format!("-std={}", profile.language.id()));
    }
    let bindings = builder
        .generate()
        .map_err(|error| format!("generate bindings: {error}"))?;
    let mut bytes = Vec::new();
    bindings
        .write(&mut bytes)
        .map_err(|error| format!("format bindings: {error}"))?;
    let contents = String::from_utf8(bytes).map_err(|error| format!("decode bindings: {error}"))?;
    let mut output = String::with_capacity(contents.len());
    let mut skip_blank = false;
    for line in contents.lines() {
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
    let path = context.out_dir.join("bindings.rs");
    fs::write(&path, output).map_err(|error| format!("write {}: {error}", path.display()))
}
