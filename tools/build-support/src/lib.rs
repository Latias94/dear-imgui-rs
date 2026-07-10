use std::env;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

static STATIC_CPP_STDLIB_LINK_EMITTED: AtomicBool = AtomicBool::new(false);

#[cfg(any(feature = "binding-spec", test))]
pub mod binding {
    use std::collections::{BTreeMap, BTreeSet};

    pub const CORE_BUILD_ENV_VARS: &[&str] = &[
        "BUILD_SUPPORT_GH_OWNER",
        "BUILD_SUPPORT_GH_REPO",
        "BINDGEN_EXTRA_CLANG_ARGS",
        "CARGO_CFG_TARGET_ARCH",
        "CARGO_CFG_TARGET_ENDIAN",
        "CARGO_CFG_TARGET_ENV",
        "CARGO_CFG_TARGET_FEATURE",
        "CARGO_CFG_TARGET_OS",
        "CARGO_CFG_TARGET_POINTER_WIDTH",
        "CARGO_NET_OFFLINE",
        "CARGO_TARGET_DIR",
        "DEAR_IMGUI_RS_REGEN_BINDINGS",
        "DOCS_RS",
        "IMGUI_SYS_CACHE_DIR",
        "IMGUI_SYS_FORCE_BUILD",
        "IMGUI_SYS_LIB_DIR",
        "IMGUI_SYS_PACKAGE_DIR",
        "IMGUI_SYS_PREBUILT_URL",
        "IMGUI_SYS_SKIP_CC",
        "IMGUI_SYS_USE_PREBUILT",
        "PROFILE",
        "TARGET",
    ];

    const CORE_FUNCTION_ALLOWLISTS: &[&str] = &["ig.*", "Im.*"];
    const CORE_FUNCTION_BLOCKLISTS: &[&str] = &[
        "ImGuiPlatformIO_Set_Platform_GetWindowPos",
        "ImGuiPlatformIO_Set_Platform_GetWindowSize",
        "ImGuiTextBuffer_appendfv",
        "igBulletTextV",
        "igDebugLogV",
        "igImFormatStringToTempBufferV",
        "igImFormatStringV",
        "igLabelTextV",
        "igLogTextV",
        "igSetItemTooltipV",
        "igSetTooltipV",
        "igTextAlignedV",
        "igTextColoredV",
        "igTextDisabledV",
        "igTextV",
        "igTextWrappedV",
        "igTreeNodeExV_Ptr",
        "igTreeNodeExV_Str",
        "igTreeNodeV_Ptr",
        "igTreeNodeV_Str",
    ];
    const CORE_TYPE_BLOCKLISTS: &[&str] = &["FILE", "ImGuiDockNode", ".*va_list.*"];
    const CORE_OPAQUE_TYPES: &[&str] = &["ImGuiDockNode"];
    const CORE_RAW_LINES: &[&str] = &[
        "pub type FILE = ::std::os::raw::c_void;",
        "pub type ImU64 = ::std::os::raw::c_ulonglong;",
        "#[repr(C)]\npub struct ImGuiDockNode { _unused: [u8; 0] }",
    ];
    const CORE_SIGNED_ENUM_ALIASES: &[&str] = &[
        "ImGuiContextHookType",
        "ImGuiDockNodeState",
        "ImGuiInputEventType",
        "ImGuiInputSource",
        "ImGuiKey",
        "ImGuiLocKey",
        "ImGuiMouseSource",
        "ImGuiNavLayer",
        "ImGuiPlotType",
        "ImGuiPopupPositionPolicy",
        "ImGuiSelectionRequestType",
        "ImGuiSortDirection",
        "ImGuiWindowDockStyleCol",
        "ImTextureFormat",
        "ImTextureStatus",
        "ImWcharClass",
    ];
    const CORE_HEADER_PREAMBLE: &str = "";
    const CORE_HEADER_SHIMS: &[HeaderShim] = &[HeaderShim {
        name: "stdio.h",
        contents: r#"
#ifndef DEAR_IMGUI_RS_STDIO_H
#define DEAR_IMGUI_RS_STDIO_H
typedef __SIZE_TYPE__ size_t;
typedef void FILE;
#endif
"#,
    }];
    const CORE_TYPE_ALLOWLISTS: &[&str] = &["Im.*"];
    const CORE_VAR_ALLOWLISTS: &[&str] = &["Im.*"];
    const CORE_NATIVE_WINDOWS64_CLANG_ARGS: &[&str] = &["--target=x86_64-pc-windows-msvc"];
    const CORE_NATIVE_NON_WINDOWS_CLANG_ARGS: &[&str] = &["--target=x86_64-unknown-linux-gnu"];
    const CORE_WASM_CLANG_ARGS: &[&str] = &["--target=wasm32-unknown-unknown"];
    const CORE_NATIVE_DEFINES: &[&str] = &["CIMGUI_DEFINE_ENUMS_AND_STRUCTS", "IMGUI_USE_WCHAR32"];
    const CORE_WASM_DEFINES: &[&str] = &[
        "CIMGUI_DEFINE_ENUMS_AND_STRUCTS",
        "IMGUI_DISABLE_FILE_FUNCTIONS",
        "IMGUI_DISABLE_OSX_FUNCTIONS",
        "IMGUI_DISABLE_WIN32_FUNCTIONS",
        "IMGUI_USE_WCHAR32",
    ];
    const CORE_INCLUDE_PATHS: &[&str] = &[".", "imgui"];
    pub const CORE_BINDGEN_GENERATOR: &str = "rust-bindgen 0.72.1";
    pub const BINDGEN_EXTRA_CLANG_ARGS_PREFIX: &str = "BINDGEN_EXTRA_CLANG_ARGS";
    pub const CORE_WASM_TARGET: &str = "wasm32-unknown-unknown";

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum NativeAbiProfile {
        Windows64,
        NonWindows,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct TargetFacts<'a> {
        pub triple: &'a str,
        pub os: &'a str,
        pub env: &'a str,
        pub arch: &'a str,
        pub endian: &'a str,
        pub pointer_width: &'a str,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct NativeAbiTarget {
        pub rust_target: &'static str,
        pub clang_target: &'static str,
        pub os: &'static str,
        pub env: &'static str,
        pub arch: &'static str,
        pub endian: &'static str,
        pub pointer_width: &'static str,
    }

    const fn native_target(
        rust_target: &'static str,
        clang_target: &'static str,
        os: &'static str,
        env: &'static str,
        arch: &'static str,
        pointer_width: &'static str,
    ) -> NativeAbiTarget {
        NativeAbiTarget {
            rust_target,
            clang_target,
            os,
            env,
            arch,
            endian: "little",
            pointer_width,
        }
    }

    const WINDOWS64_TARGETS: &[NativeAbiTarget] = &[
        native_target(
            "x86_64-pc-windows-msvc",
            "x86_64-pc-windows-msvc",
            "windows",
            "msvc",
            "x86_64",
            "64",
        ),
        native_target(
            "aarch64-pc-windows-msvc",
            "aarch64-pc-windows-msvc",
            "windows",
            "msvc",
            "aarch64",
            "64",
        ),
        native_target(
            "x86_64-pc-windows-gnu",
            "x86_64-w64-windows-gnu",
            "windows",
            "gnu",
            "x86_64",
            "64",
        ),
    ];

    const NON_WINDOWS_TARGETS: &[NativeAbiTarget] = &[
        native_target(
            "x86_64-unknown-linux-gnu",
            "x86_64-unknown-linux-gnu",
            "linux",
            "gnu",
            "x86_64",
            "64",
        ),
        native_target(
            "aarch64-unknown-linux-gnu",
            "aarch64-unknown-linux-gnu",
            "linux",
            "gnu",
            "aarch64",
            "64",
        ),
        native_target(
            "x86_64-unknown-linux-musl",
            "x86_64-unknown-linux-musl",
            "linux",
            "musl",
            "x86_64",
            "64",
        ),
        native_target(
            "aarch64-unknown-linux-musl",
            "aarch64-unknown-linux-musl",
            "linux",
            "musl",
            "aarch64",
            "64",
        ),
        native_target(
            "i686-unknown-linux-gnu",
            "i686-unknown-linux-gnu",
            "linux",
            "gnu",
            "x86",
            "32",
        ),
        native_target(
            "i686-unknown-linux-musl",
            "i686-unknown-linux-musl",
            "linux",
            "musl",
            "x86",
            "32",
        ),
        native_target(
            "x86_64-apple-darwin",
            "x86_64-apple-darwin",
            "macos",
            "",
            "x86_64",
            "64",
        ),
        native_target(
            "aarch64-apple-darwin",
            "arm64-apple-darwin",
            "macos",
            "",
            "aarch64",
            "64",
        ),
        native_target(
            "aarch64-apple-ios",
            "arm64-apple-ios",
            "ios",
            "",
            "aarch64",
            "64",
        ),
        native_target(
            "aarch64-apple-ios-sim",
            "arm64-apple-ios-simulator",
            "ios",
            "sim",
            "aarch64",
            "64",
        ),
        native_target(
            "x86_64-apple-ios",
            "x86_64-apple-ios",
            "ios",
            "sim",
            "x86_64",
            "64",
        ),
        native_target(
            "aarch64-linux-android",
            "aarch64-linux-android",
            "android",
            "",
            "aarch64",
            "64",
        ),
        native_target(
            "x86_64-linux-android",
            "x86_64-linux-android",
            "android",
            "",
            "x86_64",
            "64",
        ),
        native_target(
            "i686-linux-android",
            "i686-linux-android",
            "android",
            "",
            "x86",
            "32",
        ),
        native_target(
            "armv7-linux-androideabi",
            "armv7-linux-androideabi",
            "android",
            "",
            "arm",
            "32",
        ),
        native_target(
            "armv7-unknown-linux-gnueabihf",
            "armv7-unknown-linux-gnueabihf",
            "linux",
            "gnu",
            "arm",
            "32",
        ),
        native_target(
            "armv7-unknown-linux-musleabihf",
            "armv7-unknown-linux-musleabihf",
            "linux",
            "musl",
            "arm",
            "32",
        ),
    ];

    impl NativeAbiProfile {
        pub fn for_target(target: TargetFacts<'_>) -> Result<Self, String> {
            for profile in [Self::Windows64, Self::NonWindows] {
                if let Some(expected) = profile
                    .compatibility_targets()
                    .iter()
                    .find(|candidate| candidate.rust_target == target.triple)
                {
                    if (
                        target.os,
                        target.env,
                        target.arch,
                        target.endian,
                        target.pointer_width,
                    ) == (
                        expected.os,
                        expected.env,
                        expected.arch,
                        expected.endian,
                        expected.pointer_width,
                    ) {
                        return Ok(profile);
                    }
                    return Err(format!(
                        "Dear ImGui target facts do not match Rust target {}: \
                         os={}, env={}, arch={}, endian={}, pointer_width={}",
                        target.triple,
                        target.os,
                        target.env,
                        target.arch,
                        target.endian,
                        target.pointer_width
                    ));
                }
            }
            Err(format!(
                "unsupported Dear ImGui pregenerated binding Rust target: {}",
                target.triple
            ))
        }

        pub const fn id(self) -> &'static str {
            match self {
                Self::Windows64 => "windows64",
                Self::NonWindows => "non-windows",
            }
        }

        pub const fn canonical_clang_target(self) -> &'static str {
            match self {
                Self::Windows64 => "x86_64-pc-windows-msvc",
                Self::NonWindows => "x86_64-unknown-linux-gnu",
            }
        }

        pub const fn pregenerated_file(self) -> &'static str {
            match self {
                Self::Windows64 => "src/bindings_pregenerated_windows.rs",
                Self::NonWindows => "src/bindings_pregenerated.rs",
            }
        }

        pub const fn compatibility_targets(self) -> &'static [NativeAbiTarget] {
            match self {
                Self::Windows64 => WINDOWS64_TARGETS,
                Self::NonWindows => NON_WINDOWS_TARGETS,
            }
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum BindingTarget {
        Native { profile: NativeAbiProfile },
        WasmImport { module_name: String },
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct DerivePolicy {
        pub default: bool,
        pub debug: bool,
        pub copy: bool,
        pub eq: bool,
        pub partial_eq: bool,
        pub hash: bool,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct SanitizationPolicy {
        pub remove_inner_attributes: bool,
        pub remove_following_blank_line: bool,
        pub deduplicate_raw_lines: bool,
        pub normalize_imgui_enum_repr: bool,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct HeaderShim {
        pub name: &'static str,
        pub contents: &'static str,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum BindingFormatter {
        Rustfmt,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum BindingRustEdition {
        Rust2021,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct BindingSpec {
        pub header: &'static str,
        pub header_preamble: &'static str,
        pub header_shims: &'static [HeaderShim],
        pub include_paths: &'static [&'static str],
        pub clang_args: &'static [&'static str],
        pub clang_defines: &'static [&'static str],
        pub allowlisted_functions: &'static [&'static str],
        pub blocklisted_functions: &'static [&'static str],
        pub allowlisted_types: &'static [&'static str],
        pub blocklisted_types: &'static [&'static str],
        pub opaque_types: &'static [&'static str],
        pub allowlisted_vars: &'static [&'static str],
        pub raw_lines: &'static [&'static str],
        pub signed_enum_aliases: &'static [&'static str],
        pub generator_contract: &'static str,
        pub formatter: BindingFormatter,
        pub rust_edition: BindingRustEdition,
        pub derives: DerivePolicy,
        pub prepend_enum_name: bool,
        pub layout_tests: bool,
        pub sanitization: SanitizationPolicy,
        pub target: BindingTarget,
    }

    impl BindingSpec {
        pub fn core_native(profile: NativeAbiProfile) -> Self {
            let clang_args = match profile {
                NativeAbiProfile::Windows64 => CORE_NATIVE_WINDOWS64_CLANG_ARGS,
                NativeAbiProfile::NonWindows => CORE_NATIVE_NON_WINDOWS_CLANG_ARGS,
            };
            Self::core(
                BindingTarget::Native { profile },
                clang_args,
                CORE_NATIVE_DEFINES,
            )
        }

        pub fn core_wasm(module_name: impl Into<String>) -> Self {
            Self::core(
                BindingTarget::WasmImport {
                    module_name: module_name.into(),
                },
                CORE_WASM_CLANG_ARGS,
                CORE_WASM_DEFINES,
            )
        }

        fn core(
            target: BindingTarget,
            clang_args: &'static [&'static str],
            clang_defines: &'static [&'static str],
        ) -> Self {
            Self {
                header: "cimgui.h",
                header_preamble: CORE_HEADER_PREAMBLE,
                header_shims: CORE_HEADER_SHIMS,
                include_paths: CORE_INCLUDE_PATHS,
                clang_args,
                clang_defines,
                allowlisted_functions: CORE_FUNCTION_ALLOWLISTS,
                blocklisted_functions: CORE_FUNCTION_BLOCKLISTS,
                allowlisted_types: CORE_TYPE_ALLOWLISTS,
                blocklisted_types: CORE_TYPE_BLOCKLISTS,
                opaque_types: CORE_OPAQUE_TYPES,
                allowlisted_vars: CORE_VAR_ALLOWLISTS,
                raw_lines: CORE_RAW_LINES,
                signed_enum_aliases: CORE_SIGNED_ENUM_ALIASES,
                generator_contract: CORE_BINDGEN_GENERATOR,
                formatter: BindingFormatter::Rustfmt,
                rust_edition: BindingRustEdition::Rust2021,
                derives: DerivePolicy {
                    default: true,
                    debug: true,
                    copy: true,
                    eq: true,
                    partial_eq: true,
                    hash: true,
                },
                prepend_enum_name: false,
                layout_tests: false,
                sanitization: SanitizationPolicy {
                    remove_inner_attributes: true,
                    remove_following_blank_line: true,
                    deduplicate_raw_lines: true,
                    normalize_imgui_enum_repr: true,
                },
                target,
            }
        }

        pub fn forbidden_symbols(&self) -> &'static [&'static str] {
            self.blocklisted_functions
        }

        pub fn deterministic_hash(&self) -> String {
            let mut hash = StableHash::new();
            hash.field("schema", "core-binding-spec-v2");
            hash.field("header", self.header);
            hash.field("header_preamble", self.header_preamble);
            hash.begin_list("header_shims", self.header_shims.len());
            for (index, shim) in self.header_shims.iter().enumerate() {
                hash.list_item(index);
                hash.field("name", shim.name);
                hash.field("contents", shim.contents);
            }
            hash.fields("include_paths", self.include_paths);
            hash.fields("clang_args", self.clang_args);
            hash.fields("clang_defines", self.clang_defines);
            hash.fields("allowlisted_functions", self.allowlisted_functions);
            hash.fields("blocklisted_functions", self.blocklisted_functions);
            hash.fields("allowlisted_types", self.allowlisted_types);
            hash.fields("blocklisted_types", self.blocklisted_types);
            hash.fields("opaque_types", self.opaque_types);
            hash.fields("allowlisted_vars", self.allowlisted_vars);
            hash.fields("raw_lines", self.raw_lines);
            hash.fields("signed_enum_aliases", self.signed_enum_aliases);
            hash.field("generator_contract", self.generator_contract);
            hash.field(
                "formatter",
                match self.formatter {
                    BindingFormatter::Rustfmt => "rustfmt",
                },
            );
            hash.field(
                "rust_edition",
                match self.rust_edition {
                    BindingRustEdition::Rust2021 => "rust-edition-2021",
                },
            );
            hash.bool_field("derive_default", self.derives.default);
            hash.bool_field("derive_debug", self.derives.debug);
            hash.bool_field("derive_copy", self.derives.copy);
            hash.bool_field("derive_eq", self.derives.eq);
            hash.bool_field("derive_partial_eq", self.derives.partial_eq);
            hash.bool_field("derive_hash", self.derives.hash);
            hash.bool_field("prepend_enum_name", self.prepend_enum_name);
            hash.bool_field("layout_tests", self.layout_tests);
            hash.bool_field(
                "sanitize_remove_inner_attributes",
                self.sanitization.remove_inner_attributes,
            );
            hash.bool_field(
                "sanitize_remove_following_blank_line",
                self.sanitization.remove_following_blank_line,
            );
            hash.bool_field(
                "sanitize_deduplicate_raw_lines",
                self.sanitization.deduplicate_raw_lines,
            );
            hash.bool_field(
                "sanitize_normalize_imgui_enum_repr",
                self.sanitization.normalize_imgui_enum_repr,
            );
            match &self.target {
                BindingTarget::Native { profile } => {
                    hash.field("target_kind", "native");
                    hash.field("native_profile", profile.id());
                    let targets = profile.compatibility_targets();
                    hash.begin_list("native_compatibility_targets", targets.len());
                    for (index, target) in targets.iter().enumerate() {
                        hash.list_item(index);
                        hash.field("rust_target", target.rust_target);
                        hash.field("clang_target", target.clang_target);
                        hash.field("target_os", target.os);
                        hash.field("target_env", target.env);
                        hash.field("target_arch", target.arch);
                        hash.field("target_endian", target.endian);
                        hash.field("target_pointer_width", target.pointer_width);
                    }
                }
                BindingTarget::WasmImport { module_name } => {
                    hash.field("target_kind", "wasm-import");
                    hash.field("wasm_target", CORE_WASM_TARGET);
                    hash.field("wasm_module_name", module_name);
                }
            }
            hash.finish()
        }

        pub fn sanitize(&self, content: &str) -> String {
            let mut output = String::with_capacity(content.len());
            let mut skip_next_blank = false;
            let mut seen_raw_lines = BTreeSet::new();
            for line in content.lines() {
                let trimmed = line.trim_start();
                if self.sanitization.remove_inner_attributes && trimmed.starts_with("#![") {
                    skip_next_blank = self.sanitization.remove_following_blank_line;
                    continue;
                }
                if skip_next_blank && trimmed.is_empty() {
                    continue;
                }
                skip_next_blank = false;
                if self.sanitization.deduplicate_raw_lines
                    && self.raw_lines.contains(&trimmed)
                    && !seen_raw_lines.insert(trimmed.to_owned())
                {
                    continue;
                }
                if self.sanitization.normalize_imgui_enum_repr {
                    let signed_alias = self.signed_enum_aliases.iter().any(|alias| {
                        trimmed == format!("pub type {alias} = ::std::os::raw::c_uint;")
                    });
                    if trimmed.starts_with("pub type Im")
                        && (trimmed.ends_with("_ = ::std::os::raw::c_uint;") || signed_alias)
                    {
                        output.push_str(
                            &line
                                .replace(" = ::std::os::raw::c_uint;", " = ::std::os::raw::c_int;"),
                        );
                    } else {
                        output.push_str(line);
                    }
                } else {
                    output.push_str(line);
                }
                output.push('\n');
            }
            output
        }

        pub fn validate_generated_bindings(&self, content: &str) -> Result<(), String> {
            let generator_banner =
                format!("automatically generated by {}", self.generator_contract);
            if !content.contains(&generator_banner) {
                return Err(format!(
                    "generated bindings do not declare generator contract {}",
                    self.generator_contract
                ));
            }
            for opaque in self.opaque_types {
                for by_value in [
                    format!(": {opaque},"),
                    format!(": {opaque})"),
                    format!("-> {opaque}"),
                    format!("[{opaque};"),
                ] {
                    if content.contains(&by_value) {
                        return Err(format!(
                            "opaque type {opaque} appears in a by-value generated ABI"
                        ));
                    }
                }
            }
            let forbidden = self
                .forbidden_symbols()
                .iter()
                .copied()
                .filter(|symbol| content.contains(symbol))
                .collect::<Vec<_>>();
            if forbidden.is_empty() {
                Ok(())
            } else {
                Err(format!(
                    "generated bindings expose forbidden raw symbols: {}",
                    forbidden.join(", ")
                ))
            }
        }
    }

    pub fn bindgen_rerun_env_vars(target: &str) -> Vec<String> {
        let mut names = vec![BINDGEN_EXTRA_CLANG_ARGS_PREFIX.to_owned()];
        if !target.is_empty() {
            names.push(format!("{BINDGEN_EXTRA_CLANG_ARGS_PREFIX}_{target}"));
            names.push(format!(
                "{BINDGEN_EXTRA_CLANG_ARGS_PREFIX}_{}",
                target.replace('-', "_")
            ));
        }
        names.sort_unstable();
        names.dedup();
        names
    }

    pub fn validate_bindgen_environment<I, K>(names: I) -> Result<(), String>
    where
        I: IntoIterator<Item = K>,
        K: AsRef<str>,
    {
        let mut forbidden = names
            .into_iter()
            .map(|name| name.as_ref().to_owned())
            .filter(|name| name.starts_with(BINDGEN_EXTRA_CLANG_ARGS_PREFIX))
            .collect::<Vec<_>>();
        forbidden.sort_unstable();
        forbidden.dedup();
        if forbidden.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "canonical core binding generation rejects environment overrides: {}",
                forbidden.join(", ")
            ))
        }
    }

    pub fn is_supported_wasm_target(target_triple: &str) -> bool {
        target_triple == CORE_WASM_TARGET
    }

    pub fn validate_wasm_feature_contract(
        target_triple: &str,
        wasm_feature_enabled: bool,
    ) -> Result<(), String> {
        if is_supported_wasm_target(target_triple) {
            if wasm_feature_enabled {
                Ok(())
            } else {
                Err(format!(
                    "{CORE_WASM_TARGET} requires the explicit `wasm` feature"
                ))
            }
        } else if target_triple.starts_with("wasm32") {
            Err(format!(
                "unsupported Dear ImGui WASM target `{target_triple}`; \
                 only `{CORE_WASM_TARGET}` with the `wasm` feature is supported"
            ))
        } else {
            Ok(())
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct BuildRequestInput<'a> {
        pub target_triple: &'a str,
        pub target_os: &'a str,
        pub target_env: &'a str,
        pub target_arch: &'a str,
        pub target_endian: &'a str,
        pub target_pointer_width: &'a str,
        pub cargo_profile: &'a str,
        pub artifact_features: Vec<&'a str>,
        pub environment: Vec<(&'a str, Option<&'a str>)>,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct BuildRequest {
        pub target_triple: String,
        pub target_os: String,
        pub target_env: String,
        pub target_arch: String,
        pub target_endian: String,
        pub target_pointer_width: String,
        pub cargo_profile: String,
        pub artifact_features: Vec<String>,
        pub environment: Vec<(String, Option<String>)>,
    }

    impl BuildRequest {
        pub fn new(input: BuildRequestInput<'_>) -> Self {
            let mut artifact_features = normalize_values(input.artifact_features);
            artifact_features.sort_unstable();

            let mut environment = input
                .environment
                .into_iter()
                .map(|(name, value)| (name.to_owned(), value.map(str::to_owned)))
                .collect::<Vec<_>>();
            environment.sort_unstable_by(|left, right| left.0.cmp(&right.0));
            environment.dedup_by(|left, right| left.0 == right.0);

            Self {
                target_triple: input.target_triple.to_owned(),
                target_os: input.target_os.to_owned(),
                target_env: input.target_env.to_owned(),
                target_arch: input.target_arch.to_owned(),
                target_endian: input.target_endian.to_owned(),
                target_pointer_width: input.target_pointer_width.to_owned(),
                cargo_profile: input.cargo_profile.to_owned(),
                artifact_features,
                environment,
            }
        }

        pub fn deterministic_hash(&self) -> String {
            let mut hash = StableHash::new();
            hash.field("schema", "core-build-request-v2");
            hash.field("target_triple", &self.target_triple);
            hash.field("target_os", &self.target_os);
            hash.field("target_env", &self.target_env);
            hash.field("target_arch", &self.target_arch);
            hash.field("target_endian", &self.target_endian);
            hash.field("target_pointer_width", &self.target_pointer_width);
            hash.field("cargo_profile", &self.cargo_profile);
            hash.fields("artifact_features", &self.artifact_features);
            hash.begin_list("environment", self.environment.len());
            for (index, (name, value)) in self.environment.iter().enumerate() {
                hash.list_item(index);
                hash.field("name", name);
                match value {
                    Some(value) => {
                        hash.field("state", "set");
                        hash.field("value", value);
                    }
                    None => hash.field("state", "unset"),
                }
            }
            hash.finish()
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct SourceRevisions {
        pub cimgui: String,
        pub imgui: String,
    }

    impl SourceRevisions {
        pub fn new(cimgui: impl Into<String>, imgui: impl Into<String>) -> Self {
            Self {
                cimgui: cimgui.into(),
                imgui: imgui.into(),
            }
        }

        pub fn from_cargo_manifest(content: &str) -> Result<Self, String> {
            const SECTION: &str = "package.metadata.dear-imgui-sources";
            let mut current_section = "";
            let mut values = BTreeMap::new();

            for line in content.lines() {
                let line = line.split('#').next().unwrap_or_default().trim();
                if line.is_empty() {
                    continue;
                }
                if let Some(section) = line
                    .strip_prefix('[')
                    .and_then(|line| line.strip_suffix(']'))
                {
                    current_section = section.trim();
                    continue;
                }
                if current_section != SECTION {
                    continue;
                }
                let Some((key, value)) = line.split_once('=') else {
                    continue;
                };
                let key = key.trim();
                if !matches!(key, "cimgui-revision" | "imgui-revision") {
                    return Err(format!("unknown key {key} in [{SECTION}]"));
                }
                let value = value.trim();
                let value = value
                    .strip_prefix('"')
                    .and_then(|value| value.strip_suffix('"'))
                    .ok_or_else(|| format!("{key} in [{SECTION}] must be a quoted string"))?;
                if values.insert(key.to_owned(), value.to_owned()).is_some() {
                    return Err(format!("duplicate key {key} in [{SECTION}]"));
                }
            }

            let cimgui = values
                .remove("cimgui-revision")
                .ok_or_else(|| format!("missing cimgui-revision in [{SECTION}]"))?;
            let imgui = values
                .remove("imgui-revision")
                .ok_or_else(|| format!("missing imgui-revision in [{SECTION}]"))?;
            validate_git_revision("cimgui-revision", &cimgui)?;
            validate_git_revision("imgui-revision", &imgui)?;
            Ok(Self { cimgui, imgui })
        }
    }

    fn validate_git_revision(name: &str, value: &str) -> Result<(), String> {
        if value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            Ok(())
        } else {
            Err(format!(
                "{name} must be exactly 40 ASCII hexadecimal characters"
            ))
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ArtifactProfile {
        pub crate_name: String,
        pub version: String,
        pub target: String,
        pub link_type: String,
        pub crt: String,
        pub features: Vec<String>,
        pub source_revisions: SourceRevisions,
        pub binding_spec_hash: String,
    }

    impl ArtifactProfile {
        #[allow(clippy::too_many_arguments)]
        pub fn new<I, S>(
            crate_name: impl Into<String>,
            version: impl Into<String>,
            target: impl Into<String>,
            link_type: impl Into<String>,
            crt: impl Into<String>,
            features: I,
            source_revisions: SourceRevisions,
            binding_spec_hash: impl Into<String>,
        ) -> Self
        where
            I: IntoIterator<Item = S>,
            S: AsRef<str>,
        {
            Self {
                crate_name: crate_name.into(),
                version: version.into(),
                target: target.into(),
                link_type: link_type.into(),
                crt: crt.into(),
                features: normalize_values(features),
                source_revisions,
                binding_spec_hash: binding_spec_hash.into(),
            }
        }

        pub fn deterministic_hash(&self) -> String {
            let mut hash = StableHash::new();
            hash.field("schema", "core-artifact-profile-v2");
            hash.field("crate_name", &self.crate_name);
            hash.field("version", &self.version);
            hash.field("target", &self.target);
            hash.field("link_type", &self.link_type);
            hash.field("crt", &self.crt);
            hash.fields("features", &self.features);
            hash.field("cimgui_revision", &self.source_revisions.cimgui);
            hash.field("imgui_revision", &self.source_revisions.imgui);
            hash.field("binding_spec_hash", &self.binding_spec_hash);
            hash.finish()
        }

        pub fn manifest_bytes(&self) -> Vec<u8> {
            format!(
                "{} prebuilt\nversion={}\ntarget={}\nlink={}\ncrt={}\nfeatures={}\ncimgui_revision={}\nimgui_revision={}\nbinding_spec_hash={}\n",
                self.crate_name,
                self.version,
                self.target,
                self.link_type,
                self.crt,
                self.features.join(","),
                self.source_revisions.cimgui,
                self.source_revisions.imgui,
                self.binding_spec_hash,
            )
            .into_bytes()
        }

        pub fn validate_manifest_bytes(&self, bytes: &[u8]) -> Result<(), String> {
            let manifest = ParsedManifest::parse(bytes)?;
            let features = self.features.join(",");
            let expected = [
                ("crate_name", self.crate_name.as_str()),
                ("version", self.version.as_str()),
                ("target", self.target.as_str()),
                ("link", self.link_type.as_str()),
                ("crt", self.crt.as_str()),
                ("features", features.as_str()),
                ("cimgui_revision", self.source_revisions.cimgui.as_str()),
                ("imgui_revision", self.source_revisions.imgui.as_str()),
                ("binding_spec_hash", self.binding_spec_hash.as_str()),
            ];

            for &(field, expected) in &expected {
                let actual = manifest.field(field);
                if actual != Some(expected) {
                    return Err(format!(
                        "artifact manifest {field} mismatch: expected {expected:?}, found {actual:?}"
                    ));
                }
            }
            let expected_fields = expected
                .iter()
                .map(|(field, _)| *field)
                .collect::<BTreeSet<_>>();
            let unknown_fields = manifest
                .fields
                .keys()
                .map(String::as_str)
                .filter(|field| !expected_fields.contains(field))
                .collect::<Vec<_>>();
            if !unknown_fields.is_empty() {
                return Err(format!(
                    "artifact manifest contains unknown fields: {}",
                    unknown_fields.join(", ")
                ));
            }
            Ok(())
        }
    }

    struct ParsedManifest {
        fields: BTreeMap<String, String>,
    }

    impl ParsedManifest {
        fn parse(bytes: &[u8]) -> Result<Self, String> {
            let content = std::str::from_utf8(bytes)
                .map_err(|error| format!("artifact manifest is not UTF-8: {error}"))?;
            let mut lines = content.lines();
            let heading = lines
                .next()
                .ok_or_else(|| "artifact manifest is empty".to_owned())?;
            let crate_name = heading
                .strip_suffix(" prebuilt")
                .filter(|name| !name.is_empty())
                .ok_or_else(|| "artifact manifest has an invalid heading".to_owned())?;
            let mut fields = BTreeMap::from([("crate_name".to_owned(), crate_name.to_owned())]);
            for line in lines {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let (key, value) = line
                    .split_once('=')
                    .ok_or_else(|| format!("artifact manifest has an invalid line: {line}"))?;
                if fields.insert(key.to_owned(), value.to_owned()).is_some() {
                    return Err(format!("artifact manifest repeats field {key}"));
                }
            }
            Ok(Self { fields })
        }

        fn field(&self, field: &str) -> Option<&str> {
            self.fields.get(field).map(String::as_str)
        }
    }

    fn normalize_values<I, S>(values: I) -> Vec<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut values = values
            .into_iter()
            .map(|value| value.as_ref().trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        values.sort_unstable();
        values.dedup();
        values
    }

    struct StableHash(u64);

    impl StableHash {
        const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIME: u64 = 0x0000_0100_0000_01b3;

        fn new() -> Self {
            Self(Self::OFFSET)
        }

        fn field(&mut self, label: &str, value: &str) {
            self.bytes(b"field");
            self.string(label);
            self.string(value);
        }

        fn bool_field(&mut self, label: &str, value: bool) {
            self.field(label, if value { "true" } else { "false" });
        }

        fn begin_list(&mut self, label: &str, len: usize) {
            self.bytes(b"list");
            self.string(label);
            self.u64(len as u64);
        }

        fn list_item(&mut self, index: usize) {
            self.bytes(b"item");
            self.u64(index as u64);
        }

        fn fields<I, S>(&mut self, label: &str, values: I)
        where
            I: IntoIterator<Item = S>,
            S: AsRef<str>,
        {
            let values = values.into_iter().collect::<Vec<_>>();
            self.begin_list(label, values.len());
            for (index, value) in values.into_iter().enumerate() {
                self.list_item(index);
                self.string(value.as_ref());
            }
        }

        fn string(&mut self, value: &str) {
            self.u64(value.len() as u64);
            self.bytes(value.as_bytes());
        }

        fn u64(&mut self, value: u64) {
            self.bytes(&value.to_le_bytes());
        }

        fn bytes(&mut self, bytes: &[u8]) {
            for byte in bytes {
                self.0 ^= u64::from(*byte);
                self.0 = self.0.wrapping_mul(Self::PRIME);
            }
        }

        fn finish(self) -> String {
            format!("fnv1a64:{:016x}", self.0)
        }
    }

    #[cfg(test)]
    mod stable_hash_tests {
        use super::StableHash;

        #[test]
        fn canonical_encoding_distinguishes_labels_and_list_boundaries() {
            let mut separated = StableHash::new();
            separated.fields("include_paths", ["a"]);
            separated.fields("clang_args", ["b"]);

            let mut shifted = StableHash::new();
            shifted.fields("include_paths", ["a", "b"]);
            shifted.fields("clang_args", std::iter::empty::<&str>());
            assert_ne!(separated.finish(), shifted.finish());

            let mut first_label = StableHash::new();
            first_label.field("first", "same-value");
            let mut second_label = StableHash::new();
            second_label.field("second", "same-value");
            assert_ne!(first_label.finish(), second_label.finish());
        }
    }
}

pub fn parse_bool_env(key: &str) -> bool {
    match env::var(key) {
        Ok(v) => matches!(
            v.as_str(),
            "1" | "true" | "yes" | "on" | "TRUE" | "YES" | "ON"
        ),
        Err(_) => false,
    }
}

pub fn msvc_crt_suffix_from_env(target_env: Option<&str>) -> Option<&'static str> {
    let is_msvc = match target_env {
        Some(s) => s == "msvc",
        None => matches!(
            env::var("CARGO_CFG_TARGET_ENV").ok().as_deref(),
            Some("msvc")
        ),
    };
    if !is_msvc {
        return None;
    }
    let tf = env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default();
    if tf.split(',').any(|f| f == "crt-static") {
        Some("mt")
    } else {
        Some("md")
    }
}

pub fn should_static_link_cpp_stdlib(target_os: &str, target_env: &str) -> bool {
    target_os == "windows" && target_env == "gnu"
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CppRuntimeLinkage {
    None,
    Dynamic(&'static str),
    StaticBundle(&'static str),
}

pub fn prebuilt_cpp_runtime_linkage(target_os: &str, target_env: &str) -> CppRuntimeLinkage {
    if target_env == "msvc" {
        return CppRuntimeLinkage::None;
    }
    if should_static_link_cpp_stdlib(target_os, target_env) {
        return CppRuntimeLinkage::StaticBundle("stdc++");
    }
    if matches!(
        target_os,
        "macos" | "ios" | "tvos" | "watchos" | "visionos" | "freebsd" | "openbsd" | "aix" | "wasi"
    ) || (target_os == "linux" && target_env == "ohos")
    {
        return CppRuntimeLinkage::Dynamic("c++");
    }
    if target_os == "android" {
        return CppRuntimeLinkage::Dynamic("c++_shared");
    }
    CppRuntimeLinkage::Dynamic("stdc++")
}

pub fn emit_prebuilt_cpp_runtime_linkage(target_os: &str, target_env: &str) {
    match prebuilt_cpp_runtime_linkage(target_os, target_env) {
        CppRuntimeLinkage::None => {}
        CppRuntimeLinkage::Dynamic(library) => {
            println!("cargo:rustc-link-lib={library}");
        }
        CppRuntimeLinkage::StaticBundle(library) => {
            if !STATIC_CPP_STDLIB_LINK_EMITTED.swap(true, Ordering::Relaxed) {
                println!("cargo:rustc-link-lib=static:-bundle={library}");
            }
        }
    }
}

pub fn configure_cpp_runtime_linkage(build: &mut cc::Build, target_os: &str, target_env: &str) {
    match prebuilt_cpp_runtime_linkage(target_os, target_env) {
        CppRuntimeLinkage::None => {
            build.cpp_link_stdlib(None);
        }
        CppRuntimeLinkage::Dynamic(library) => {
            // Keep source builds and prebuilt consumers on the same deterministic platform
            // policy instead of allowing an untracked CXXSTDLIB override.
            build.cpp_link_stdlib(library);
        }
        CppRuntimeLinkage::StaticBundle(_) => {
            build.cpp_link_stdlib(None);
            emit_prebuilt_cpp_runtime_linkage(target_os, target_env);
        }
    }
}

pub fn expected_lib_name(target_env: &str, base: &str) -> String {
    if target_env == "msvc" {
        format!("{}.lib", base)
    } else {
        format!("lib{}.a", base)
    }
}

pub fn compose_archive_name(
    crate_short: &str,
    version: &str,
    target: &str,
    link_type: &str,
    extra: Option<&str>,
    crt: &str,
) -> String {
    let extra = extra.unwrap_or("");
    if crt.is_empty() {
        if extra.is_empty() {
            format!(
                "{}-prebuilt-{}-{}-{}.tar.gz",
                crate_short, version, target, link_type
            )
        } else {
            format!(
                "{}-prebuilt-{}-{}-{}{}.tar.gz",
                crate_short, version, target, link_type, extra
            )
        }
    } else if extra.is_empty() {
        format!(
            "{}-prebuilt-{}-{}-{}-{}.tar.gz",
            crate_short, version, target, link_type, crt
        )
    } else {
        format!(
            "{}-prebuilt-{}-{}-{}{}-{}.tar.gz",
            crate_short, version, target, link_type, extra, crt
        )
    }
}

pub fn release_tags(crate_sys_name: &str, version: &str) -> [String; 2] {
    [
        format!("{}-v{}", crate_sys_name, version),
        format!("v{}", version),
    ]
}

pub fn compose_manifest_bytes(
    crate_short: &str,
    version: &str,
    target: &str,
    link_type: &str,
    crt: &str,
    features: Option<&str>,
) -> Vec<u8> {
    let mut buf = Vec::new();
    use std::io::Write;
    let _ = writeln!(
        &mut buf,
        "{} prebuilt\nversion={}\ntarget={}\nlink={}\ncrt={}",
        crate_short, version, target, link_type, crt
    );
    if let Some(f) = features
        && !f.is_empty()
    {
        let _ = writeln!(&mut buf, "features={}", f);
    }
    buf
}

#[cfg(feature = "binding-spec")]
pub fn compose_manifest_bytes_with_profile(profile: &binding::ArtifactProfile) -> Vec<u8> {
    profile.manifest_bytes()
}

pub fn prebuilt_manifest_features(dir: &Path) -> Option<Vec<String>> {
    let mut candidates = Vec::with_capacity(2);
    candidates.push(dir.join("manifest.txt"));
    if let Some(parent) = dir.parent() {
        candidates.push(parent.join("manifest.txt"));
    }

    for manifest in candidates {
        let Ok(s) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        for line in s.lines() {
            if let Some(rest) = line.strip_prefix("features=") {
                let features = rest
                    .split(',')
                    .map(|f| f.trim().to_ascii_lowercase())
                    .filter(|f| !f.is_empty())
                    .collect::<Vec<_>>();
                return Some(features);
            }
        }
        return Some(Vec::new());
    }

    None
}

pub fn prebuilt_manifest_has_feature(dir: &Path, feature: &str) -> bool {
    let feature = feature.trim().to_ascii_lowercase();
    let Some(features) = prebuilt_manifest_features(dir) else {
        return false;
    };
    features.iter().any(|f| f == &feature)
}

pub fn release_candidate_urls(
    owner: &str,
    repo: &str,
    tags: &[String],
    names: &[String],
) -> Vec<String> {
    let mut out = Vec::with_capacity(tags.len() * names.len());
    for tag in tags {
        for name in names {
            out.push(format!(
                "https://github.com/{}/{}/releases/download/{}/{}",
                owner, repo, tag, name
            ));
        }
    }
    out
}

pub fn release_candidate_urls_default(tags: &[String], names: &[String]) -> Vec<String> {
    release_candidate_urls(DEFAULT_GITHUB_OWNER, DEFAULT_GITHUB_REPO, tags, names)
}

pub fn release_owner_repo() -> (String, String) {
    let owner =
        env::var("BUILD_SUPPORT_GH_OWNER").unwrap_or_else(|_| DEFAULT_GITHUB_OWNER.to_string());
    let repo =
        env::var("BUILD_SUPPORT_GH_REPO").unwrap_or_else(|_| DEFAULT_GITHUB_REPO.to_string());
    (owner, repo)
}

pub fn release_candidate_urls_env(tags: &[String], names: &[String]) -> Vec<String> {
    let (owner, repo) = release_owner_repo();
    release_candidate_urls(&owner, &repo, tags, names)
}

pub fn is_offline() -> bool {
    match env::var("CARGO_NET_OFFLINE") {
        Ok(v) => matches!(
            v.as_str(),
            "1" | "true" | "yes" | "on" | "TRUE" | "YES" | "ON"
        ),
        Err(_) => false,
    }
}

pub fn prebuilt_extract_dir_env(cache_root: &Path, target_env: &str) -> PathBuf {
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

pub fn extract_archive_to_cache(
    archive_path: &Path,
    cache_root: &Path,
    lib_name: &str,
) -> Result<PathBuf, String> {
    #[cfg(feature = "archive")]
    {
        extract_archive_to_cache_impl(archive_path, cache_root, lib_name)
    }

    #[cfg(not(feature = "archive"))]
    {
        let _ = (archive_path, cache_root, lib_name);
        Err(
            "archive extraction disabled: enable feature `dear-imgui-build-support/archive`"
                .to_string(),
        )
    }
}

#[cfg(feature = "archive")]
fn extract_archive_to_cache_impl(
    archive_path: &Path,
    cache_root: &Path,
    lib_name: &str,
) -> Result<PathBuf, String> {
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let extract_dir = prebuilt_extract_dir_env(cache_root, &target_env);
    extract_archive_to_dir(archive_path, &extract_dir, lib_name, || {})
}

#[cfg(feature = "archive")]
fn extract_archive_to_dir(
    archive_path: &Path,
    extract_dir: &Path,
    lib_name: &str,
    after_lock: impl FnOnce(),
) -> Result<PathBuf, String> {
    let parent = extract_dir.parent().ok_or_else(|| {
        format!(
            "prebuilt extraction directory has no parent: {}",
            extract_dir.display()
        )
    })?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("create dir {}: {}", parent.display(), e))?;
    let lock_path = extraction_lock_path(extract_dir);
    let extraction_lock = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|error| format!("open extraction lock {}: {error}", lock_path.display()))?;
    extraction_lock
        .lock()
        .map_err(|error| format!("lock extraction cache {}: {error}", lock_path.display()))?;
    after_lock();

    if let Some(lib_dir) = extracted_library_dir(extract_dir, lib_name) {
        return Ok(lib_dir);
    }
    if extract_dir.exists() {
        let stale_dir = unique_staging_path(extract_dir);
        std::fs::rename(extract_dir, &stale_dir).map_err(|error| {
            format!(
                "retire stale extraction directory {}: {error}",
                extract_dir.display()
            )
        })?;
        std::fs::remove_dir_all(&stale_dir)
            .map_err(|error| format!("remove stale dir {}: {error}", stale_dir.display()))?;
    }
    let staging_dir = unique_staging_path(extract_dir);
    if staging_dir.exists() {
        let _ = std::fs::remove_dir_all(&staging_dir);
    }
    std::fs::create_dir_all(&staging_dir)
        .map_err(|e| format!("create dir {}: {}", staging_dir.display(), e))?;
    let file = std::fs::File::open(archive_path)
        .map_err(|e| format!("open {}: {}", archive_path.display(), e))?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
    if let Err(error) = archive.unpack(&staging_dir) {
        let _ = std::fs::remove_dir_all(&staging_dir);
        return Err(format!("unpack {}: {}", archive_path.display(), error));
    }
    let staged_lib_dir = extracted_library_dir(&staging_dir, lib_name).ok_or_else(|| {
        let _ = std::fs::remove_dir_all(&staging_dir);
        "extracted archive did not contain the expected library and manifest".to_owned()
    })?;
    let uses_lib_subdir = staged_lib_dir != staging_dir;

    match std::fs::rename(&staging_dir, extract_dir) {
        Ok(()) => Ok(if uses_lib_subdir {
            extract_dir.join("lib")
        } else {
            extract_dir.to_path_buf()
        }),
        Err(error) => {
            // Another process may have completed the same extraction first. Its atomic rename is
            // safe to reuse only after validating the expected library is present.
            if let Some(lib_dir) = extracted_library_dir(extract_dir, lib_name) {
                let _ = std::fs::remove_dir_all(&staging_dir);
                return Ok(lib_dir);
            }
            let _ = std::fs::remove_dir_all(&staging_dir);
            Err(format!(
                "install extracted archive at {}: {}",
                extract_dir.display(),
                error
            ))
        }
    }
}

#[cfg(feature = "archive")]
fn extraction_lock_path(extract_dir: &Path) -> PathBuf {
    let lock_name = extract_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("prebuilt");
    extract_dir.with_file_name(format!(".{lock_name}.extract.lock"))
}

#[cfg(feature = "archive")]
fn extracted_library_dir(root: &Path, lib_name: &str) -> Option<PathBuf> {
    if !root.join("manifest.txt").is_file() {
        return None;
    }
    let lib_dir = root.join("lib");
    if lib_dir.join(lib_name).is_file() {
        Some(lib_dir)
    } else if root.join(lib_name).is_file() {
        Some(root.to_path_buf())
    } else {
        None
    }
}

#[cfg(feature = "archive")]
fn unique_staging_path(destination: &Path) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("prebuilt");
    destination.with_file_name(format!(".{name}.tmp-{}-{nonce}", std::process::id()))
}

pub fn download_prebuilt(
    cache_root: &Path,
    url: &str,
    lib_name: &str,
    _target_env: &str,
) -> Result<PathBuf, String> {
    if let Some(path) = local_path_from_urlish(url) {
        return stage_or_extract_local(cache_root, &path, lib_name);
    }

    #[cfg(feature = "download")]
    {
        download_prebuilt_http(cache_root, url, lib_name)
    }

    #[cfg(not(feature = "download"))]
    {
        let _ = (cache_root, url, lib_name);
        Err(
            "download support disabled: enable feature `dear-imgui-build-support/download`"
                .to_string(),
        )
    }
}

fn local_path_from_urlish(url: &str) -> Option<PathBuf> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(rest) = trimmed.strip_prefix("file://") {
        let p = file_url_path(rest);
        if p.exists() {
            return Some(p);
        }
        return None;
    }

    let p = PathBuf::from(trimmed);
    if p.exists() { Some(p) } else { None }
}

fn file_url_path(rest: &str) -> PathBuf {
    #[cfg(windows)]
    {
        // Windows accepts both file:///C:/... and file://C:/.... Remove exactly the URL root
        // slash only when the remainder starts with a drive letter; preserve UNC prefixes.
        let bytes = rest.as_bytes();
        if bytes.len() >= 3
            && bytes[0] == b'/'
            && bytes[1].is_ascii_alphabetic()
            && bytes[2] == b':'
        {
            return PathBuf::from(&rest[1..]);
        }
    }

    // On Unix the leading slash is the filesystem root and must not be discarded.
    PathBuf::from(rest)
}

fn stage_or_extract_local(
    cache_root: &Path,
    path: &Path,
    lib_name: &str,
) -> Result<PathBuf, String> {
    if is_archive_path(path) {
        return extract_archive_to_cache(path, cache_root, lib_name);
    }

    if path
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s == lib_name)
    {
        let Some(parent) = path.parent() else {
            return Err("local prebuilt path had no parent directory".to_string());
        };
        return Ok(parent.to_path_buf());
    }

    let dl_dir = cache_root.join("download");
    let _ = std::fs::create_dir_all(&dl_dir);
    let dst = dl_dir.join(lib_name);
    if !dst.exists() {
        let bytes = std::fs::read(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
        write_atomic(&dst, &bytes)?;
    }
    Ok(dl_dir)
}

fn is_archive_path(path: &Path) -> bool {
    let s = path.to_string_lossy().to_ascii_lowercase();
    s.ends_with(".tar.gz") || s.ends_with(".tgz")
}

#[cfg(feature = "download")]
fn download_prebuilt_http(cache_root: &Path, url: &str, lib_name: &str) -> Result<PathBuf, String> {
    let dl_dir = cache_root.join("download");
    let _ = std::fs::create_dir_all(&dl_dir);

    if url.ends_with(".tar.gz") || url.ends_with(".tgz") {
        let fname = url.split('/').next_back().unwrap_or("prebuilt.tar.gz");
        let archive_path = dl_dir.join(fname);
        if !archive_path.exists() {
            let config = ureq::Agent::config_builder()
                .timeout_global(Some(std::time::Duration::from_secs(300)))
                .build();
            let agent = ureq::Agent::new_with_config(config);
            let resp = agent
                .get(url)
                .call()
                .map_err(|e| format!("http get: {}", e))?;
            let status = resp.status();
            if !status.is_success() {
                return Err(format!("http status {}", status));
            }
            let mut reader = resp.into_body().into_reader();
            let mut bytes = Vec::new();
            use std::io::Read as _;
            reader
                .read_to_end(&mut bytes)
                .map_err(|e| format!("read body: {}", e))?;
            write_atomic(&archive_path, &bytes)?;
        }
        return extract_archive_to_cache(&archive_path, cache_root, lib_name);
    }

    let dst = dl_dir.join(lib_name);
    if dst.exists() {
        return Ok(dl_dir);
    }
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(120)))
        .build();
    let agent = ureq::Agent::new_with_config(config);
    let resp = agent
        .get(url)
        .call()
        .map_err(|e| format!("http get: {}", e))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("http status {}", status));
    }
    let mut reader = resp.into_body().into_reader();
    let mut bytes = Vec::new();
    use std::io::Read as _;
    reader
        .read_to_end(&mut bytes)
        .map_err(|e| format!("read body: {}", e))?;
    write_atomic(&dst, &bytes)?;
    Ok(dl_dir)
}

fn write_atomic(destination: &Path, bytes: &[u8]) -> Result<(), String> {
    let staging = unique_file_staging_path(destination);
    std::fs::write(&staging, bytes)
        .map_err(|error| format!("write {}: {error}", staging.display()))?;
    match std::fs::rename(&staging, destination) {
        Ok(()) => Ok(()),
        Err(_error) if destination.is_file() => {
            let _ = std::fs::remove_file(&staging);
            Ok(())
        }
        Err(error) => {
            let _ = std::fs::remove_file(&staging);
            Err(format!(
                "install downloaded artifact at {}: {error}",
                destination.display()
            ))
        }
    }
}

fn unique_file_staging_path(destination: &Path) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("prebuilt");
    destination.with_file_name(format!(".{name}.tmp-{}-{nonce}", std::process::id()))
}

pub fn prebuilt_cache_root_from_env_or_target(
    manifest_dir: &Path,
    cache_env_var: &str,
    folder: &str,
) -> PathBuf {
    if let Ok(dir) = env::var(cache_env_var) {
        return PathBuf::from(dir);
    }
    let target_dir = env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| manifest_dir.parent().unwrap().join("target"));
    target_dir.join(folder)
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeDependency {
    pub include_paths: Vec<PathBuf>,
    pub source: String,
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
#[derive(Clone, Copy, Debug)]
pub struct PackageSearchConfig {
    pub use_pkg_config: bool,
    pub use_vcpkg: bool,
    pub emit_cargo_metadata: bool,
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
#[derive(Clone, Copy, Debug)]
pub struct Sdl3SearchConfig<'a> {
    pub out_dir: &'a Path,
    pub target_os: &'a str,
    pub use_pkg_config: bool,
    pub use_vcpkg: bool,
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
#[derive(Clone, Copy, Debug)]
pub struct NativeIncludeSearchConfig<'a> {
    pub explicit_include_envs: &'a [&'a str],
    pub dependency_include_envs: &'a [&'a str],
    pub dependency_out_dir_envs: &'a [&'a str],
    pub cargo_target_include_prefix: Option<&'a str>,
    pub out_dir: Option<&'a Path>,
    pub required_header: &'a str,
    pub pkg_config_package: Option<&'a str>,
    pub vcpkg_package: Option<&'a str>,
    pub target_os: &'a str,
    pub use_pkg_config: bool,
    pub use_vcpkg: bool,
    pub emit_cargo_metadata: bool,
    pub print_system_libs: bool,
    pub copy_vcpkg_dlls: bool,
    pub known_include_roots: &'a [PathBuf],
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
pub fn find_native_include_paths(
    config: NativeIncludeSearchConfig<'_>,
) -> Result<NativeDependency, String> {
    find_native_include_paths_inner(config).map_err(|message| {
        format!(
            "could not find include paths containing `{}`. {message}",
            config.required_header
        )
    })
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
pub fn find_freetype(config: PackageSearchConfig) -> Result<NativeDependency, String> {
    let mut attempts = Vec::new();
    emit_pkg_config_rerun_vars("FREETYPE2");
    emit_vcpkg_rerun_vars("FREETYPE");
    let target_os = cargo_target_os();
    let target_env = cargo_target_env();
    let use_vcpkg = should_use_vcpkg(config.use_vcpkg, &target_os, &target_env);

    if config.use_pkg_config {
        if let Some(found) =
            probe_pkg_config_package("freetype2", config.emit_cargo_metadata, true, &mut attempts)
        {
            return Ok(found);
        }
    } else {
        attempts.push("pkg-config feature disabled".to_string());
    }

    if use_vcpkg {
        if let Some(found) = probe_vcpkg_package(
            "freetype",
            config.emit_cargo_metadata,
            config.emit_cargo_metadata,
            &mut attempts,
        ) {
            return Ok(found);
        }
    } else {
        push_vcpkg_skip_attempt(&mut attempts, config.use_vcpkg, &target_os, &target_env);
    }

    let install_hint = if use_vcpkg {
        "Install FreeType development files with pkg-config metadata, or install \
         the vcpkg `freetype` port and set VCPKG_ROOT/VCPKGRS_DYNAMIC as \
         required by vcpkg."
    } else {
        "Install FreeType development files with pkg-config metadata."
    };
    Err(format!(
        "could not find FreeType. Tried {}. {install_hint}",
        attempts.join("; "),
    ))
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
pub fn find_sdl3_include_paths(config: Sdl3SearchConfig<'_>) -> Result<NativeDependency, String> {
    emit_pkg_config_rerun_vars("SDL3");
    emit_vcpkg_rerun_vars("SDL3");
    let known_include_roots = known_sdl3_include_roots(config.target_os);
    find_native_include_paths_inner(NativeIncludeSearchConfig {
        explicit_include_envs: &["SDL3_INCLUDE_DIR"],
        dependency_include_envs: &["DEP_SDL3_INCLUDE_PATH", "DEP_SDL3_INCLUDE_DIR"],
        dependency_out_dir_envs: &["DEP_SDL3_OUT_DIR"],
        cargo_target_include_prefix: Some("sdl3-sys-"),
        out_dir: Some(config.out_dir),
        required_header: "SDL3/SDL.h",
        pkg_config_package: Some("sdl3"),
        vcpkg_package: Some("sdl3"),
        target_os: config.target_os,
        use_pkg_config: config.use_pkg_config,
        use_vcpkg: config.use_vcpkg,
        emit_cargo_metadata: false,
        print_system_libs: false,
        copy_vcpkg_dlls: false,
        known_include_roots: &known_include_roots,
    })
    .map_err(|message| format!("could not find SDL3 headers. {message}"))
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
fn find_native_include_paths_inner(
    config: NativeIncludeSearchConfig<'_>,
) -> Result<NativeDependency, String> {
    let mut attempts = Vec::new();

    for var in config.explicit_include_envs {
        if let Ok(dir) = env::var(var) {
            let root = PathBuf::from(&dir);
            if include_root_has_header(&root, config.required_header) {
                return Ok(NativeDependency {
                    include_paths: vec![root],
                    source: format!("{var}={dir}"),
                });
            }
            return Err(format!(
                "{var} is set to `{dir}`, but `{}` was not found under it",
                root.join(config.required_header).display()
            ));
        }
    }

    for var in config.dependency_include_envs {
        if let Ok(dir) = env::var(var) {
            let root = PathBuf::from(&dir);
            if include_root_has_header(&root, config.required_header) {
                return Ok(NativeDependency {
                    include_paths: vec![root],
                    source: format!("{var}={dir}"),
                });
            }
            attempts.push(format!(
                "{var}={dir}, but {} was not found",
                root.join(config.required_header).display()
            ));
        }
    }

    for var in config.dependency_out_dir_envs {
        if let Ok(out_dir) = env::var(var) {
            let include_root = PathBuf::from(&out_dir).join("include");
            if include_root_has_header(&include_root, config.required_header) {
                return Ok(NativeDependency {
                    include_paths: vec![include_root],
                    source: format!("{var}={out_dir}"),
                });
            }
            attempts.push(format!(
                "{var}={out_dir}, but {} was not found",
                include_root.join(config.required_header).display()
            ));
        }
    }

    if let (Some(prefix), Some(out_dir)) = (config.cargo_target_include_prefix, config.out_dir)
        && let Some(include_root) =
            find_cargo_target_include(out_dir, prefix, config.required_header)
    {
        return Ok(NativeDependency {
            include_paths: vec![include_root.clone()],
            source: format!("Cargo target dir={}", include_root.display()),
        });
    }

    if config.use_pkg_config {
        if let Some(package) = config.pkg_config_package
            && let Some(found) = probe_pkg_config_package(
                package,
                config.emit_cargo_metadata,
                config.print_system_libs,
                &mut attempts,
            )
        {
            return Ok(found);
        }
    } else {
        attempts.push("pkg-config feature disabled".to_string());
    }

    let target_env = cargo_target_env();
    let use_vcpkg = should_use_vcpkg(config.use_vcpkg, config.target_os, &target_env);
    if use_vcpkg {
        if let Some(package) = config.vcpkg_package
            && let Some(found) = probe_vcpkg_package(
                package,
                config.emit_cargo_metadata,
                config.copy_vcpkg_dlls,
                &mut attempts,
            )
        {
            return Ok(found);
        }
    } else {
        push_vcpkg_skip_attempt(
            &mut attempts,
            config.use_vcpkg,
            config.target_os,
            &target_env,
        );
    }

    for candidate in config.known_include_roots {
        if include_root_has_header(candidate, config.required_header) {
            return Ok(NativeDependency {
                include_paths: vec![candidate.clone()],
                source: format!("known include path {}", candidate.display()),
            });
        }
        attempts.push(format!(
            "known include path {} did not contain {}",
            candidate.display(),
            config.required_header
        ));
    }

    Err(format!("Tried {}.", attempts.join("; ")))
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
fn probe_pkg_config_package(
    package: &str,
    emit_cargo_metadata: bool,
    print_system_libs: bool,
    attempts: &mut Vec<String>,
) -> Option<NativeDependency> {
    #[cfg(feature = "pkg-config")]
    {
        let mut pkg = pkg_config::Config::new();
        pkg.cargo_metadata(emit_cargo_metadata)
            .print_system_libs(print_system_libs);
        match pkg.probe(package) {
            Ok(lib) => {
                return Some(NativeDependency {
                    include_paths: lib.include_paths,
                    source: format!("pkg-config ({package})"),
                });
            }
            Err(err) => attempts.push(format!("pkg-config {package}: {err}")),
        }
    }
    #[cfg(not(feature = "pkg-config"))]
    {
        let _ = (package, emit_cargo_metadata, print_system_libs);
        attempts.push("pkg-config support was not compiled into build-support".to_string());
    }
    None
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
fn probe_vcpkg_package(
    package: &str,
    emit_cargo_metadata: bool,
    copy_dlls: bool,
    attempts: &mut Vec<String>,
) -> Option<NativeDependency> {
    #[cfg(feature = "vcpkg")]
    {
        let mut vcpkg_config = vcpkg::Config::new();
        vcpkg_config
            .cargo_metadata(emit_cargo_metadata)
            .copy_dlls(copy_dlls);
        match vcpkg_config.find_package(package) {
            Ok(lib) => {
                return Some(NativeDependency {
                    include_paths: lib.include_paths,
                    source: format!("vcpkg ({package})"),
                });
            }
            Err(err) => attempts.push(format!("vcpkg {package}: {err}")),
        }
    }
    #[cfg(not(feature = "vcpkg"))]
    {
        let _ = (package, emit_cargo_metadata, copy_dlls);
        attempts.push("vcpkg support was not compiled into build-support".to_string());
    }
    None
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
fn include_root_has_header(root: &Path, header: &str) -> bool {
    root.join(header).exists()
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
fn should_use_vcpkg(use_vcpkg: bool, target_os: &str, target_env: &str) -> bool {
    use_vcpkg && target_os == "windows" && target_env == "msvc"
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
fn cargo_target_os() -> String {
    env::var("CARGO_CFG_TARGET_OS").unwrap_or_default()
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
fn cargo_target_env() -> String {
    env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default()
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
fn push_vcpkg_skip_attempt(
    attempts: &mut Vec<String>,
    use_vcpkg: bool,
    target_os: &str,
    target_env: &str,
) {
    if use_vcpkg {
        let target = target_label(target_os, target_env);
        attempts.push(format!(
            "vcpkg skipped for target {target}: automatic vcpkg \
             discovery is only enabled for Windows MSVC targets"
        ));
    } else {
        attempts.push("vcpkg feature disabled".to_string());
    }
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
fn target_label(target_os: &str, target_env: &str) -> String {
    if target_env.is_empty() {
        target_os.to_string()
    } else {
        format!("{target_os}-{target_env}")
    }
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
fn known_sdl3_include_roots(target_os: &str) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if matches!(target_os, "macos" | "ios") {
        roots.extend([
            PathBuf::from("/opt/homebrew/include"),
            PathBuf::from("/usr/local/include"),
            PathBuf::from("/opt/local/include"),
        ]);
    } else if matches!(target_os, "linux" | "freebsd" | "openbsd" | "netbsd") {
        roots.extend([
            PathBuf::from("/usr/include"),
            PathBuf::from("/usr/local/include"),
            PathBuf::from("/opt/local/include"),
        ]);
    }
    roots
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
fn find_cargo_target_include(out_dir: &Path, dir_prefix: &str, header: &str) -> Option<PathBuf> {
    let mut cargo_build_dir = out_dir.to_path_buf();
    while cargo_build_dir
        .file_name()
        .is_some_and(|name| name != "build")
    {
        if !cargo_build_dir.pop() {
            return None;
        }
    }

    if cargo_build_dir
        .file_name()
        .is_none_or(|name| name != "build")
    {
        return None;
    }

    let entries = std::fs::read_dir(&cargo_build_dir).ok()?;
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        if !file_name.to_string_lossy().starts_with(dir_prefix) {
            continue;
        }

        let include_root = entry.path().join("out/include");
        if include_root_has_header(&include_root, header) {
            return Some(include_root);
        }
    }

    None
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
fn emit_pkg_config_rerun_vars(package_env_stem: &str) {
    for var in [
        "PKG_CONFIG",
        "PKG_CONFIG_PATH",
        "PKG_CONFIG_LIBDIR",
        "PKG_CONFIG_SYSROOT_DIR",
    ] {
        println!("cargo:rerun-if-env-changed={var}");
    }
    println!("cargo:rerun-if-env-changed={package_env_stem}_NO_PKG_CONFIG");
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
fn emit_vcpkg_rerun_vars(port_env_stem: &str) {
    for var in [
        "VCPKG_ROOT",
        "VCPKGRS_TRIPLET",
        "VCPKGRS_DYNAMIC",
        "VCPKGRS_DISABLE",
    ] {
        println!("cargo:rerun-if-env-changed={var}");
    }
    println!("cargo:rerun-if-env-changed=VCPKGRS_NO_{port_env_stem}");
}

pub const DEFAULT_GITHUB_OWNER: &str = "Latias94";
pub const DEFAULT_GITHUB_REPO: &str = "dear-imgui";

#[cfg(test)]
mod binding_contract_tests {
    use super::binding::{
        ArtifactProfile, BindingSpec, BuildRequest, BuildRequestInput, CORE_BUILD_ENV_VARS,
        CORE_WASM_TARGET, NativeAbiProfile, SourceRevisions, TargetFacts, bindgen_rerun_env_vars,
        is_supported_wasm_target, validate_bindgen_environment, validate_wasm_feature_contract,
    };

    fn request_with_env(values: Vec<(&str, Option<&str>)>) -> BuildRequest {
        BuildRequest::new(BuildRequestInput {
            target_triple: "x86_64-pc-windows-msvc",
            target_os: "windows",
            target_env: "msvc",
            target_arch: "x86_64",
            target_endian: "little",
            target_pointer_width: "64",
            cargo_profile: "release",
            artifact_features: vec!["platform-io-aggregate-hooks", "wchar32"],
            environment: values,
        })
    }

    fn profile() -> ArtifactProfile {
        ArtifactProfile::new(
            "dear-imgui",
            "0.15.1",
            "x86_64-pc-windows-msvc",
            "static",
            "md",
            ["platform-io-aggregate-hooks", "wchar32"],
            SourceRevisions::new("cimgui-revision", "imgui-revision"),
            BindingSpec::core_native(NativeAbiProfile::Windows64).deterministic_hash(),
        )
    }

    #[test]
    fn native_and_wasm_share_forbidden_aggregate_helpers() {
        let native = BindingSpec::core_native(NativeAbiProfile::Windows64);
        let wasm = BindingSpec::core_wasm("imgui-sys-v0");

        assert_eq!(native.blocklisted_functions, wasm.blocklisted_functions);
        assert_eq!(native.forbidden_symbols(), wasm.forbidden_symbols());
        assert!(
            native
                .forbidden_symbols()
                .contains(&"ImGuiPlatformIO_Set_Platform_GetWindowPos")
        );
        assert!(
            native
                .forbidden_symbols()
                .contains(&"ImGuiPlatformIO_Set_Platform_GetWindowSize")
        );
    }

    #[test]
    fn binding_spec_hash_is_stable_and_covers_target_policy() {
        let native = BindingSpec::core_native(NativeAbiProfile::Windows64);
        let wasm = BindingSpec::core_wasm("imgui-sys-v0");

        assert_eq!(native.deterministic_hash(), native.deterministic_hash());
        assert_ne!(
            native.deterministic_hash(),
            BindingSpec::core_native(NativeAbiProfile::NonWindows).deterministic_hash()
        );
        assert_ne!(native.deterministic_hash(), wasm.deterministic_hash());
        assert_ne!(
            wasm.deterministic_hash(),
            BindingSpec::core_wasm("different-provider").deterministic_hash()
        );

        let mut separated = BindingSpec::core_native(NativeAbiProfile::NonWindows);
        separated.include_paths = &["boundary-a"];
        separated.clang_args = &["boundary-b"];
        let mut shifted = separated.clone();
        shifted.include_paths = &["boundary-a", "boundary-b"];
        shifted.clang_args = &[];
        assert_ne!(
            separated.deterministic_hash(),
            shifted.deterministic_hash(),
            "list field boundaries must participate in the canonical hash"
        );
    }

    #[test]
    fn native_abi_profiles_cover_only_the_verified_target_matrix() {
        for profile in [NativeAbiProfile::Windows64, NativeAbiProfile::NonWindows] {
            for target in profile.compatibility_targets() {
                assert_eq!(
                    NativeAbiProfile::for_target(TargetFacts {
                        triple: target.rust_target,
                        os: target.os,
                        env: target.env,
                        arch: target.arch,
                        endian: target.endian,
                        pointer_width: target.pointer_width,
                    })
                    .unwrap(),
                    profile
                );
            }
        }

        let windows_gnu = NativeAbiProfile::Windows64
            .compatibility_targets()
            .iter()
            .find(|target| target.rust_target == "x86_64-pc-windows-gnu")
            .unwrap();
        assert_eq!(windows_gnu.clang_target, "x86_64-w64-windows-gnu");

        for facts in [
            TargetFacts {
                triple: "aarch64-pc-windows-gnu",
                os: "windows",
                env: "gnu",
                arch: "aarch64",
                endian: "little",
                pointer_width: "64",
            },
            TargetFacts {
                triple: "armv5te-unknown-linux-gnueabi",
                os: "linux",
                env: "gnu",
                arch: "arm",
                endian: "little",
                pointer_width: "32",
            },
            TargetFacts {
                triple: "armv7-unknown-linux-gnu",
                os: "linux",
                env: "gnu",
                arch: "arm",
                endian: "little",
                pointer_width: "32",
            },
            TargetFacts {
                triple: "aarch64-apple-ios-macabi",
                os: "ios",
                env: "macabi",
                arch: "aarch64",
                endian: "little",
                pointer_width: "64",
            },
        ] {
            assert!(NativeAbiProfile::for_target(facts).is_err());
        }

        assert!(
            NativeAbiProfile::for_target(TargetFacts {
                triple: "aarch64-unknown-linux-gnu",
                os: "linux",
                env: "gnu",
                arch: "aarch64",
                endian: "big",
                pointer_width: "64",
            })
            .is_err()
        );
        assert!(
            NativeAbiProfile::for_target(TargetFacts {
                triple: "x86_64-unknown-freebsd",
                os: "freebsd",
                env: "",
                arch: "x86_64",
                endian: "little",
                pointer_width: "64",
            })
            .is_err()
        );
    }

    #[test]
    fn canonical_bindgen_environment_rejects_every_extra_clang_arg_route() {
        assert!(validate_bindgen_environment(["PATH", "HOME"]).is_ok());
        for name in bindgen_rerun_env_vars("x86_64-pc-windows-msvc") {
            let error = validate_bindgen_environment([name.as_str()]).unwrap_err();
            assert!(error.contains(&name), "unexpected error: {error}");
        }
        assert!(validate_bindgen_environment(["BINDGEN_EXTRA_CLANG_ARGS_other_target"]).is_err());
    }

    #[test]
    fn wasm_target_requires_the_explicit_provider_feature() {
        assert!(is_supported_wasm_target(CORE_WASM_TARGET));
        assert!(validate_wasm_feature_contract(CORE_WASM_TARGET, false).is_err());
        assert!(validate_wasm_feature_contract(CORE_WASM_TARGET, true).is_ok());
        for unsupported in [
            "wasm32-wasip1",
            "wasm32-wasip2",
            "wasm32-unknown-emscripten",
        ] {
            assert!(!is_supported_wasm_target(unsupported));
            assert!(validate_wasm_feature_contract(unsupported, false).is_err());
            assert!(validate_wasm_feature_contract(unsupported, true).is_err());
        }
        assert!(validate_wasm_feature_contract("x86_64-unknown-linux-gnu", false).is_ok());
        assert!(validate_wasm_feature_contract("x86_64-unknown-linux-gnu", true).is_ok());
    }

    #[test]
    fn sanitization_is_part_of_the_shared_contract() {
        let source = "#![allow(dead_code)]\n\npub const VALUE: u32 = 1;\n";
        let expected = "pub const VALUE: u32 = 1;\n";

        assert_eq!(
            BindingSpec::core_native(NativeAbiProfile::Windows64).sanitize(source),
            expected
        );
        assert_eq!(
            BindingSpec::core_wasm("imgui-sys-v0").sanitize(source),
            expected
        );
    }

    #[test]
    fn sanitization_policies_are_independent() {
        let source = concat!(
            "#![allow(dead_code)]\n",
            "\n",
            "pub type ImU64 = ::std::os::raw::c_ulonglong;\n",
            "pub type ImU64 = ::std::os::raw::c_ulonglong;\n",
            "pub type ImGuiKey = ::std::os::raw::c_uint;\n",
        );
        let mut spec = BindingSpec::core_native(NativeAbiProfile::NonWindows);
        spec.sanitization.remove_inner_attributes = false;
        assert_eq!(
            spec.sanitize(source),
            concat!(
                "#![allow(dead_code)]\n",
                "\n",
                "pub type ImU64 = ::std::os::raw::c_ulonglong;\n",
                "pub type ImGuiKey = ::std::os::raw::c_int;\n",
            )
        );

        spec.sanitization.remove_inner_attributes = true;
        spec.sanitization.remove_following_blank_line = false;
        spec.sanitization.deduplicate_raw_lines = false;
        spec.sanitization.normalize_imgui_enum_repr = false;
        assert_eq!(
            spec.sanitize(source),
            concat!(
                "\n",
                "pub type ImU64 = ::std::os::raw::c_ulonglong;\n",
                "pub type ImU64 = ::std::os::raw::c_ulonglong;\n",
                "pub type ImGuiKey = ::std::os::raw::c_uint;\n",
            )
        );
    }

    #[test]
    fn every_supported_environment_input_changes_the_build_request() {
        let baseline = request_with_env(
            CORE_BUILD_ENV_VARS
                .iter()
                .copied()
                .map(|name| (name, None))
                .collect(),
        );

        for changed in CORE_BUILD_ENV_VARS {
            let request = request_with_env(
                CORE_BUILD_ENV_VARS
                    .iter()
                    .copied()
                    .map(|name| (name, (name == *changed).then_some("changed")))
                    .collect(),
            );
            assert_ne!(
                baseline.deterministic_hash(),
                request.deterministic_hash(),
                "environment input {changed} did not affect BuildRequest"
            );
        }

        let separated = BuildRequest::new(BuildRequestInput {
            target_triple: "x86_64-unknown-linux-gnu",
            target_os: "linux",
            target_env: "gnu",
            target_arch: "x86_64",
            target_endian: "little",
            target_pointer_width: "64",
            cargo_profile: "release",
            artifact_features: vec!["alpha"],
            environment: vec![("beta", None)],
        });
        let shifted = BuildRequest::new(BuildRequestInput {
            target_triple: "x86_64-unknown-linux-gnu",
            target_os: "linux",
            target_env: "gnu",
            target_arch: "x86_64",
            target_endian: "little",
            target_pointer_width: "64",
            cargo_profile: "release",
            artifact_features: vec!["alpha", "beta", "unset"],
            environment: vec![],
        });
        assert_ne!(
            separated.deterministic_hash(),
            shifted.deterministic_hash(),
            "feature/environment list boundaries must participate in BuildRequest hashes"
        );
    }

    #[test]
    fn artifact_manifest_round_trips_and_rejects_every_provenance_mismatch() {
        let expected = profile();
        let manifest = expected.manifest_bytes();
        expected.validate_manifest_bytes(&manifest).unwrap();

        let mut changed_hash_field = expected.clone();
        changed_hash_field.binding_spec_hash = expected.version.clone();
        changed_hash_field.version = expected.binding_spec_hash.clone();
        assert_ne!(
            expected.deterministic_hash(),
            changed_hash_field.deterministic_hash(),
            "ArtifactProfile field roles must participate in the canonical hash"
        );

        for (field, replacement) in [
            ("cimgui_revision", "wrong-cimgui"),
            ("imgui_revision", "wrong-imgui"),
            ("binding_spec_hash", "wrong-binding-hash"),
            ("crt", "mt"),
            ("features", "stack-layout,wchar32"),
        ] {
            let manifest = String::from_utf8(expected.manifest_bytes()).unwrap();
            let manifest = manifest
                .lines()
                .map(|line| {
                    line.strip_prefix(&format!("{field}="))
                        .map_or_else(|| line.to_owned(), |_| format!("{field}={replacement}"))
                })
                .collect::<Vec<_>>()
                .join("\n");
            let error = expected
                .validate_manifest_bytes(manifest.as_bytes())
                .unwrap_err();
            assert!(
                error.contains(field),
                "unexpected error for {field}: {error}"
            );
        }

        let mut manifest = String::from_utf8(expected.manifest_bytes()).unwrap();
        manifest.push_str("unexpected_field=unexpected-value\n");
        let error = expected
            .validate_manifest_bytes(manifest.as_bytes())
            .unwrap_err();
        assert!(
            error.contains("unknown fields"),
            "unexpected error: {error}"
        );
        assert!(
            error.contains("unexpected_field"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn source_revisions_are_read_from_packaged_cargo_metadata() {
        const CIMGUI: &str = "1261b231939fc210032f30c4ee8a8f0440372237";
        const IMGUI: &str = "b61e56346a92cfcaf1f43a545ca37b0b32239654";
        let manifest = r#"
[package]
name = "dear-imgui-sys"

[package.metadata.dear-imgui-sources]
cimgui-revision = "1261b231939fc210032f30c4ee8a8f0440372237"
imgui-revision = "b61e56346a92cfcaf1f43a545ca37b0b32239654"
"#;

        assert_eq!(
            SourceRevisions::from_cargo_manifest(manifest).unwrap(),
            SourceRevisions::new(CIMGUI, IMGUI)
        );

        for invalid in [
            manifest.replace(IMGUI, "short"),
            manifest.replace(
                "imgui-revision =",
                "imgui-revision = \"b61e56346a92cfcaf1f43a545ca37b0b32239654\"\nimgui-revision =",
            ),
            manifest.replace("imgui-revision =", "exclude = []\nimgui-revision ="),
        ] {
            assert!(SourceRevisions::from_cargo_manifest(&invalid).is_err());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    use std::ffi::OsString;
    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    static SDL3_ENV_LOCK: Mutex<()> = Mutex::new(());

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    const SDL3_ENV_VARS: [&str; 4] = [
        "SDL3_INCLUDE_DIR",
        "DEP_SDL3_INCLUDE_PATH",
        "DEP_SDL3_INCLUDE_DIR",
        "DEP_SDL3_OUT_DIR",
    ];

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    const TARGET_ENV_VARS: [&str; 2] = ["CARGO_CFG_TARGET_OS", "CARGO_CFG_TARGET_ENV"];

    #[test]
    fn static_cpp_stdlib_is_limited_to_windows_gnu_targets() {
        assert!(should_static_link_cpp_stdlib("windows", "gnu"));
        assert!(!should_static_link_cpp_stdlib("windows", "msvc"));
        assert!(!should_static_link_cpp_stdlib("linux", "gnu"));
        assert!(!should_static_link_cpp_stdlib("macos", ""));
    }

    #[test]
    fn prebuilt_cpp_runtime_matches_cc_defaults_and_windows_policy() {
        assert_eq!(
            prebuilt_cpp_runtime_linkage("windows", "msvc"),
            CppRuntimeLinkage::None
        );
        assert_eq!(
            prebuilt_cpp_runtime_linkage("windows", "gnu"),
            CppRuntimeLinkage::StaticBundle("stdc++")
        );
        assert_eq!(
            prebuilt_cpp_runtime_linkage("macos", ""),
            CppRuntimeLinkage::Dynamic("c++")
        );
        assert_eq!(
            prebuilt_cpp_runtime_linkage("linux", "gnu"),
            CppRuntimeLinkage::Dynamic("stdc++")
        );
        assert_eq!(
            prebuilt_cpp_runtime_linkage("android", ""),
            CppRuntimeLinkage::Dynamic("c++_shared")
        );
    }

    fn unique_tmp_dir(suffix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "dear-imgui-build-support-test-{}-{}-{}",
            std::process::id(),
            nanos,
            suffix
        ))
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    fn lock_sdl3_env() -> std::sync::MutexGuard<'static, ()> {
        SDL3_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    struct EnvSnapshot {
        saved: Vec<(&'static str, Option<OsString>)>,
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    impl EnvSnapshot {
        fn clear_sdl3_vars() -> Self {
            Self::save_and_clear(&SDL3_ENV_VARS)
        }

        fn clear_target_vars() -> Self {
            Self::save_and_clear(&TARGET_ENV_VARS)
        }

        fn clear_sdl3_and_target_vars() -> Self {
            Self::save_and_clear(&[
                "SDL3_INCLUDE_DIR",
                "DEP_SDL3_INCLUDE_PATH",
                "DEP_SDL3_INCLUDE_DIR",
                "DEP_SDL3_OUT_DIR",
                "CARGO_CFG_TARGET_OS",
                "CARGO_CFG_TARGET_ENV",
            ])
        }

        fn save_and_clear(vars: &[&'static str]) -> Self {
            let saved = vars
                .iter()
                .copied()
                .map(|var| (var, env::var_os(var)))
                .collect::<Vec<_>>();

            for (var, _) in &saved {
                unsafe {
                    env::remove_var(var);
                }
            }

            Self { saved }
        }
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    impl Drop for EnvSnapshot {
        fn drop(&mut self) {
            for (var, value) in &self.saved {
                unsafe {
                    match value {
                        Some(value) => env::set_var(var, value),
                        None => env::remove_var(var),
                    }
                }
            }
        }
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    fn set_env_var(name: &str, value: &Path) {
        unsafe {
            env::set_var(name, value);
        }
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    fn set_env_str(name: &str, value: &str) {
        unsafe {
            env::set_var(name, value);
        }
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    fn set_cargo_target(os: &str, target_env: &str) {
        set_env_str("CARGO_CFG_TARGET_OS", os);
        set_env_str("CARGO_CFG_TARGET_ENV", target_env);
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    fn write_sdl3_header(include_root: &Path) {
        let header_dir = include_root.join("SDL3");
        std::fs::create_dir_all(&header_dir).unwrap();
        std::fs::write(header_dir.join("SDL.h"), "/* test SDL3 header */\n").unwrap();
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    fn sdl3_search(out_dir: &Path) -> NativeDependency {
        find_sdl3_include_paths(Sdl3SearchConfig {
            out_dir,
            target_os: "unknown-test-os",
            use_pkg_config: false,
            use_vcpkg: false,
        })
        .unwrap()
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    #[test]
    fn freetype_skips_vcpkg_on_non_windows_msvc_targets() {
        let _lock = lock_sdl3_env();
        let _env = EnvSnapshot::clear_target_vars();
        set_cargo_target("linux", "gnu");

        let err = find_freetype(PackageSearchConfig {
            use_pkg_config: false,
            use_vcpkg: true,
            emit_cargo_metadata: false,
        })
        .unwrap_err();

        assert!(err.contains("vcpkg skipped for target linux-gnu"));
        assert!(err.contains("Install FreeType development files with pkg-config metadata."));
        assert!(!err.contains("VCPKG_ROOT"));
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    #[test]
    fn vcpkg_discovery_is_limited_to_windows_msvc_targets() {
        assert!(should_use_vcpkg(true, "windows", "msvc"));
        assert!(!should_use_vcpkg(false, "windows", "msvc"));
        assert!(!should_use_vcpkg(true, "windows", "gnu"));
        assert!(!should_use_vcpkg(true, "linux", "gnu"));
        assert!(!should_use_vcpkg(true, "macos", ""));
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    #[test]
    fn target_label_omits_empty_target_env() {
        assert_eq!(target_label("macos", ""), "macos");
        assert_eq!(target_label("linux", "gnu"), "linux-gnu");
    }

    #[test]
    fn prebuilt_manifest_has_feature_checks_parent_manifest_for_lib_dir() {
        let root = unique_tmp_dir("parent");
        let lib_dir = root.join("lib");
        std::fs::create_dir_all(&lib_dir).unwrap();
        std::fs::write(
            root.join("manifest.txt"),
            "crate prebuilt\nfeatures=wchar32,freetype\n",
        )
        .unwrap();

        assert!(prebuilt_manifest_has_feature(&lib_dir, "wchar32"));
        assert!(prebuilt_manifest_has_feature(&lib_dir, "freetype"));
        assert!(!prebuilt_manifest_has_feature(&lib_dir, "nope"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn prebuilt_manifest_has_feature_checks_manifest_in_dir() {
        let root = unique_tmp_dir("self");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("manifest.txt"), "features=wchar32\n").unwrap();

        assert!(prebuilt_manifest_has_feature(&root, "wchar32"));
        assert!(!prebuilt_manifest_has_feature(&root, "freetype"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn absolute_file_url_preserves_the_filesystem_root() {
        let root = unique_tmp_dir("absolute-file-url");
        std::fs::create_dir_all(&root).unwrap();
        let archive = root.join("artifact.tar.gz");
        std::fs::write(&archive, b"test").unwrap();
        let url = format!("file://{}", archive.display());

        assert_eq!(local_path_from_urlish(&url), Some(archive.clone()));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn plain_local_prebuilt_path_remains_supported() {
        let root = unique_tmp_dir("plain-local-path");
        std::fs::create_dir_all(&root).unwrap();
        let archive = root.join("artifact.tar.gz");
        std::fs::write(&archive, b"test").unwrap();

        assert_eq!(
            local_path_from_urlish(archive.to_str().unwrap()),
            Some(archive.clone())
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(feature = "archive")]
    #[test]
    fn archive_extraction_is_installed_from_atomic_staging() {
        use std::io::Write as _;

        let root = unique_tmp_dir("atomic-archive");
        let source = root.join("source");
        let cache = root.join("cache");
        std::fs::create_dir_all(source.join("lib")).unwrap();
        std::fs::write(source.join("lib/libdear_imgui.a"), b"archive").unwrap();
        std::fs::write(source.join("manifest.txt"), b"features=wchar32\n").unwrap();
        let archive_path = root.join("artifact.tar.gz");
        let file = std::fs::File::create(&archive_path).unwrap();
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
        let mut archive = tar::Builder::new(encoder);
        archive
            .append_path_with_name(source.join("lib/libdear_imgui.a"), "lib/libdear_imgui.a")
            .unwrap();
        archive
            .append_path_with_name(source.join("manifest.txt"), "manifest.txt")
            .unwrap();
        archive
            .into_inner()
            .unwrap()
            .finish()
            .unwrap()
            .flush()
            .unwrap();

        let lib_dir = extract_archive_to_cache(&archive_path, &cache, "libdear_imgui.a").unwrap();

        assert_eq!(
            std::fs::read(lib_dir.join("libdear_imgui.a")).unwrap(),
            b"archive"
        );
        assert!(
            std::fs::read_dir(lib_dir.parent().unwrap())
                .unwrap()
                .all(|entry| !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .contains(".tmp-"))
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(feature = "archive")]
    #[test]
    fn archive_extraction_lock_serializes_check_cleanup_and_install() {
        use std::io::Write as _;
        use std::sync::mpsc;

        let root = unique_tmp_dir("locked-archive");
        let source = root.join("source");
        let extract_dir = root.join("cache/shared");
        std::fs::create_dir_all(source.join("lib")).unwrap();
        std::fs::create_dir_all(&extract_dir).unwrap();
        std::fs::write(extract_dir.join("stale.partial"), b"partial").unwrap();
        std::fs::write(source.join("lib/libdear_imgui.a"), b"archive").unwrap();
        std::fs::write(source.join("manifest.txt"), b"features=wchar32\n").unwrap();
        let archive_path = root.join("artifact.tar.gz");
        let file = std::fs::File::create(&archive_path).unwrap();
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
        let mut archive = tar::Builder::new(encoder);
        archive
            .append_path_with_name(source.join("lib/libdear_imgui.a"), "lib/libdear_imgui.a")
            .unwrap();
        archive
            .append_path_with_name(source.join("manifest.txt"), "manifest.txt")
            .unwrap();
        archive
            .into_inner()
            .unwrap()
            .finish()
            .unwrap()
            .flush()
            .unwrap();

        let (locked_tx, locked_rx) = mpsc::sync_channel(0);
        let (release_tx, release_rx) = mpsc::sync_channel(0);
        let first_archive = archive_path.clone();
        let first_extract = extract_dir.clone();
        let first = std::thread::spawn(move || {
            extract_archive_to_dir(&first_archive, &first_extract, "libdear_imgui.a", || {
                locked_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            })
        });
        locked_rx.recv().unwrap();

        let lock_path = extraction_lock_path(&extract_dir);
        let competing_lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_path)
            .unwrap();
        assert!(matches!(
            competing_lock.try_lock(),
            Err(std::fs::TryLockError::WouldBlock)
        ));

        release_tx.send(()).unwrap();
        let first_lib_dir = first.join().unwrap().unwrap();
        let second_lib_dir =
            extract_archive_to_dir(&archive_path, &extract_dir, "libdear_imgui.a", || {}).unwrap();

        assert_eq!(first_lib_dir, second_lib_dir);
        assert_eq!(
            std::fs::read(second_lib_dir.join("libdear_imgui.a")).unwrap(),
            b"archive"
        );
        assert!(!extract_dir.join("stale.partial").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(windows)]
    #[test]
    fn windows_file_url_removes_only_the_drive_root_slash() {
        assert_eq!(
            file_url_path("/C:/artifacts/dear-imgui.tar.gz"),
            PathBuf::from("C:/artifacts/dear-imgui.tar.gz")
        );
        assert_eq!(
            file_url_path("//server/share/dear-imgui.tar.gz"),
            PathBuf::from("//server/share/dear-imgui.tar.gz")
        );
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    #[test]
    fn sdl3_include_dir_takes_precedence_over_dep_vars() {
        let _lock = lock_sdl3_env();
        let _env = EnvSnapshot::clear_sdl3_vars();
        let root = unique_tmp_dir("sdl3-include-dir");
        let include_dir = root.join("explicit");
        let dep_include_dir = root.join("dep-include");
        write_sdl3_header(&include_dir);
        write_sdl3_header(&dep_include_dir);
        set_env_var("SDL3_INCLUDE_DIR", &include_dir);
        set_env_var("DEP_SDL3_INCLUDE_PATH", &dep_include_dir);

        let found = sdl3_search(&root.join("target/debug/build/current/out"));

        assert_eq!(found.include_paths, vec![include_dir.clone()]);
        assert!(found.source.starts_with("SDL3_INCLUDE_DIR="));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    #[test]
    fn sdl3_dep_include_path_takes_precedence_over_dep_include_dir() {
        let _lock = lock_sdl3_env();
        let _env = EnvSnapshot::clear_sdl3_vars();
        let root = unique_tmp_dir("sdl3-dep-include-path");
        let dep_include_path = root.join("dep-include-path");
        let dep_include_dir = root.join("dep-include-dir");
        write_sdl3_header(&dep_include_path);
        write_sdl3_header(&dep_include_dir);
        set_env_var("DEP_SDL3_INCLUDE_PATH", &dep_include_path);
        set_env_var("DEP_SDL3_INCLUDE_DIR", &dep_include_dir);

        let found = sdl3_search(&root.join("target/debug/build/current/out"));

        assert_eq!(found.include_paths, vec![dep_include_path.clone()]);
        assert!(found.source.starts_with("DEP_SDL3_INCLUDE_PATH="));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    #[test]
    fn sdl3_dep_include_dir_takes_precedence_over_dep_out_dir() {
        let _lock = lock_sdl3_env();
        let _env = EnvSnapshot::clear_sdl3_vars();
        let root = unique_tmp_dir("sdl3-dep-include-dir");
        let dep_include_dir = root.join("dep-include-dir");
        let dep_out_dir = root.join("dep-out");
        write_sdl3_header(&dep_include_dir);
        write_sdl3_header(&dep_out_dir.join("include"));
        set_env_var("DEP_SDL3_INCLUDE_DIR", &dep_include_dir);
        set_env_var("DEP_SDL3_OUT_DIR", &dep_out_dir);

        let found = sdl3_search(&root.join("target/debug/build/current/out"));

        assert_eq!(found.include_paths, vec![dep_include_dir.clone()]);
        assert!(found.source.starts_with("DEP_SDL3_INCLUDE_DIR="));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    #[test]
    fn sdl3_dep_out_dir_uses_include_child() {
        let _lock = lock_sdl3_env();
        let _env = EnvSnapshot::clear_sdl3_vars();
        let root = unique_tmp_dir("sdl3-dep-out-dir");
        let dep_out_dir = root.join("dep-out");
        let include_root = dep_out_dir.join("include");
        write_sdl3_header(&include_root);
        set_env_var("DEP_SDL3_OUT_DIR", &dep_out_dir);

        let found = sdl3_search(&root.join("target/debug/build/current/out"));

        assert_eq!(found.include_paths, vec![include_root]);
        assert!(found.source.starts_with("DEP_SDL3_OUT_DIR="));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    #[test]
    fn sdl3_cargo_target_include_is_used_after_env_vars() {
        let _lock = lock_sdl3_env();
        let _env = EnvSnapshot::clear_sdl3_vars();
        let root = unique_tmp_dir("sdl3-target-dir");
        let build_dir = root.join("target").join("debug").join("build");
        let current_out_dir = build_dir.join("current-crate").join("out");
        let sdl3_include_root = build_dir.join("sdl3-sys-test").join("out").join("include");
        std::fs::create_dir_all(&current_out_dir).unwrap();
        write_sdl3_header(&sdl3_include_root);

        let found = sdl3_search(&current_out_dir);

        assert_eq!(found.include_paths, vec![sdl3_include_root.clone()]);
        assert!(found.source.starts_with("Cargo target dir="));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    #[test]
    fn sdl3_include_dir_fails_fast_when_explicit_path_is_invalid() {
        let _lock = lock_sdl3_env();
        let _env = EnvSnapshot::clear_sdl3_vars();
        let root = unique_tmp_dir("sdl3-invalid-explicit");
        let invalid_include_dir = root.join("invalid");
        let dep_include_dir = root.join("dep-include");
        std::fs::create_dir_all(&invalid_include_dir).unwrap();
        write_sdl3_header(&dep_include_dir);
        set_env_var("SDL3_INCLUDE_DIR", &invalid_include_dir);
        set_env_var("DEP_SDL3_INCLUDE_PATH", &dep_include_dir);

        let err = find_sdl3_include_paths(Sdl3SearchConfig {
            out_dir: &root.join("target/debug/build/current/out"),
            target_os: "unknown-test-os",
            use_pkg_config: false,
            use_vcpkg: false,
        })
        .unwrap_err();

        assert!(err.contains("SDL3_INCLUDE_DIR is set"));
        assert!(err.contains("SDL3/SDL.h"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    #[test]
    fn sdl3_skips_vcpkg_on_non_windows_msvc_targets() {
        let _lock = lock_sdl3_env();
        let _env = EnvSnapshot::clear_sdl3_and_target_vars();
        set_cargo_target("linux", "gnu");
        let root = unique_tmp_dir("sdl3-linux-vcpkg-skip");

        let err = find_sdl3_include_paths(Sdl3SearchConfig {
            out_dir: &root.join("target/debug/build/current/out"),
            target_os: "linux",
            use_pkg_config: false,
            use_vcpkg: true,
        })
        .unwrap_err();

        assert!(err.contains("vcpkg skipped for target linux-gnu"));
        assert!(!err.contains("vcpkg sdl3:"));

        let _ = std::fs::remove_dir_all(&root);
    }
}
