use std::env;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

static STATIC_CPP_STDLIB_LINK_EMITTED: AtomicBool = AtomicBool::new(false);

#[cfg(any(feature = "binding-spec", test))]
pub mod binding {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::Path;

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
    const CORE_HEADER_SHIMS: &[HeaderShim] = &[
        HeaderShim {
            name: "stdio.h",
            contents: r#"
#ifndef DEAR_IMGUI_RS_STDIO_H
#define DEAR_IMGUI_RS_STDIO_H
#ifndef DEAR_IMGUI_RS_SIZE_T_DEFINED
#define DEAR_IMGUI_RS_SIZE_T_DEFINED
typedef __SIZE_TYPE__ size_t;
#endif
typedef void FILE;
#endif
"#,
        },
        HeaderShim {
            name: "stdint.h",
            contents: r#"
#ifndef DEAR_IMGUI_RS_STDINT_H
#define DEAR_IMGUI_RS_STDINT_H
typedef __INT8_TYPE__ int8_t;
typedef __UINT8_TYPE__ uint8_t;
typedef __INT16_TYPE__ int16_t;
typedef __UINT16_TYPE__ uint16_t;
typedef __INT32_TYPE__ int32_t;
typedef __UINT32_TYPE__ uint32_t;
typedef __INT64_TYPE__ int64_t;
typedef __UINT64_TYPE__ uint64_t;
typedef __INTPTR_TYPE__ intptr_t;
typedef __UINTPTR_TYPE__ uintptr_t;
typedef __INTMAX_TYPE__ intmax_t;
typedef __UINTMAX_TYPE__ uintmax_t;
#endif
"#,
        },
        HeaderShim {
            name: "stdarg.h",
            contents: r#"
#ifndef DEAR_IMGUI_RS_STDARG_H
#define DEAR_IMGUI_RS_STDARG_H
typedef __builtin_va_list va_list;
#define va_start(args, last) __builtin_va_start(args, last)
#define va_end(args) __builtin_va_end(args)
#define va_arg(args, type) __builtin_va_arg(args, type)
#define va_copy(dest, src) __builtin_va_copy(dest, src)
#endif
"#,
        },
        HeaderShim {
            name: "stdbool.h",
            contents: r#"
#ifndef DEAR_IMGUI_RS_STDBOOL_H
#define DEAR_IMGUI_RS_STDBOOL_H
#ifndef __cplusplus
#define bool _Bool
#define true 1
#define false 0
#endif
#define __bool_true_false_are_defined 1
#endif
"#,
        },
    ];
    const CORE_TYPE_ALLOWLISTS: &[&str] = &["Im.*"];
    const CORE_VAR_ALLOWLISTS: &[&str] = &["Im.*"];
    const CORE_NATIVE_WINDOWS64_CLANG_ARGS: &[&str] =
        &["--target=x86_64-pc-windows-msvc", "-nostdinc"];
    const CORE_NATIVE_NON_WINDOWS_CLANG_ARGS: &[&str] =
        &["--target=x86_64-unknown-linux-gnu", "-nostdinc"];
    const CORE_WASM_CLANG_ARGS: &[&str] = &["--target=wasm32-unknown-unknown", "-nostdinc"];
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

    pub const CRATE_BINDING_METADATA_SECTION: &str = "package.metadata.dear-imgui-binding";
    pub const CRATE_BINDING_PROVENANCE_PREFIX: &str = "// dear-imgui-rs-binding-provenance-v1";
    pub const RELEASE_CANDIDATE_SHA_ENV: &str = "DEAR_IMGUI_RS_CANDIDATE_SHA";
    pub const RELEASE_CORE_ARTIFACT_IDENTITY_HASH_ENV: &str =
        "DEAR_IMGUI_CORE_ARTIFACT_IDENTITY_HASH";
    pub const DEP_CORE_ARTIFACT_IDENTITY_HASH_ENV: &str = "DEP_DEAR_IMGUI_ARTIFACT_IDENTITY_HASH";
    pub const DEP_CORE_CANDIDATE_SHA_ENV: &str = "DEP_DEAR_IMGUI_CANDIDATE_SHA";
    pub const CANONICAL_BINDGEN_VERSION: &str = "0.72.1";
    pub const CANONICAL_BINDING_LIBCLANG_VERSION: (u32, u32) = (14, 0);
    pub const CANONICAL_BINDING_RUSTC_VERSION: &str = "rustc 1.95.0";
    pub const CANONICAL_BINDING_RUSTFMT_VERSION: &str = "rustfmt 1.9.0-stable";

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub enum ExtensionBinding {
        ImPlot,
        ImPlot3d,
        ImNodes,
        NodeEditor,
        ImGuizmo,
        ImGuizmoQuat,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct ExtensionArtifactSpec {
        pub extension_id: &'static str,
        pub safe_crate_name: &'static str,
        pub sys_crate_name: &'static str,
        pub archive_stem: &'static str,
        pub library_name: &'static str,
    }

    impl ExtensionBinding {
        pub const fn artifact_spec(self) -> ExtensionArtifactSpec {
            match self {
                Self::ImPlot => ExtensionArtifactSpec {
                    extension_id: "implot",
                    safe_crate_name: "dear-implot",
                    sys_crate_name: "dear-implot-sys",
                    archive_stem: "dear-implot",
                    library_name: "dear_implot",
                },
                Self::ImPlot3d => ExtensionArtifactSpec {
                    extension_id: "implot3d",
                    safe_crate_name: "dear-implot3d",
                    sys_crate_name: "dear-implot3d-sys",
                    archive_stem: "dear-implot3d",
                    library_name: "dear_implot3d",
                },
                Self::ImNodes => ExtensionArtifactSpec {
                    extension_id: "imnodes",
                    safe_crate_name: "dear-imnodes",
                    sys_crate_name: "dear-imnodes-sys",
                    archive_stem: "dear-imnodes",
                    library_name: "dear_imnodes",
                },
                Self::NodeEditor => ExtensionArtifactSpec {
                    extension_id: "node-editor",
                    safe_crate_name: "dear-node-editor",
                    sys_crate_name: "dear-node-editor-sys",
                    archive_stem: "dear-node-editor",
                    library_name: "dear_node_editor",
                },
                Self::ImGuizmo => ExtensionArtifactSpec {
                    extension_id: "imguizmo",
                    safe_crate_name: "dear-imguizmo",
                    sys_crate_name: "dear-imguizmo-sys",
                    archive_stem: "dear-imguizmo",
                    library_name: "dear_imguizmo",
                },
                Self::ImGuizmoQuat => ExtensionArtifactSpec {
                    extension_id: "imguizmo-quat",
                    safe_crate_name: "dear-imguizmo-quat",
                    sys_crate_name: "dear-imguizmo-quat-sys",
                    archive_stem: "dear-imguizmo-quat",
                    library_name: "dear_imguizmo_quat",
                },
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub enum BindingOwner {
        TestEngine,
        Extension(ExtensionBinding),
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum CrateBindingTarget {
        Native,
        WasmImport { module_name: &'static str },
    }

    impl CrateBindingTarget {
        pub const fn id(self) -> &'static str {
            match self {
                Self::Native => "native",
                Self::WasmImport { .. } => "wasm",
            }
        }

        pub const fn import_module(self) -> Option<&'static str> {
            match self {
                Self::Native => None,
                Self::WasmImport { module_name } => Some(module_name),
            }
        }

        pub const fn clang_defines(self) -> &'static [&'static str] {
            match self {
                Self::Native => &[],
                Self::WasmImport { .. } => CRATE_WASM_DEFINES,
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum CrateBindingLanguage {
        C,
        Cxx17,
    }

    impl CrateBindingLanguage {
        const fn id(self) -> &'static str {
            match self {
                Self::C => "c",
                Self::Cxx17 => "c++17",
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum CrateBindingIncludeRoot {
        CoreCimgui,
        CoreImgui,
        Source,
    }

    impl CrateBindingIncludeRoot {
        const fn id(self) -> &'static str {
            match self {
                Self::CoreCimgui => "core-cimgui",
                Self::CoreImgui => "core-imgui",
                Self::Source => "source",
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct CrateBindingInclude {
        pub root: CrateBindingIncludeRoot,
        pub relative_path: &'static str,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct CrateBindgenProfile {
        pub formatter: BindingFormatter,
        pub rust_edition: BindingRustEdition,
        pub derives: DerivePolicy,
        pub prepend_enum_name: bool,
        pub layout_tests: bool,
        pub allowlist_recursively: bool,
        pub language: CrateBindingLanguage,
        pub include_paths: &'static [CrateBindingInclude],
        pub clang_defines: &'static [&'static str],
        pub allowlisted_functions: &'static [&'static str],
        pub allowlisted_types: &'static [&'static str],
        pub allowlisted_vars: &'static [&'static str],
        pub blocklisted_types: &'static [&'static str],
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct CrateBindingSpec {
        pub owner: BindingOwner,
        pub crate_name: &'static str,
        pub crate_root: &'static str,
        pub source_root: &'static str,
        pub input_paths: &'static [&'static str],
        pub header_shims: &'static [HeaderShim],
        pub clang_args: &'static [&'static str],
        pub profile: CrateBindgenProfile,
        pub required_symbols: &'static [&'static str],
        pub checked_in_path: &'static str,
        pub target: CrateBindingTarget,
    }

    impl CrateBindingSpec {
        pub fn maintained() -> &'static [Self] {
            MAINTAINED_CRATE_BINDING_SPECS
        }

        pub fn for_owner(owner: BindingOwner) -> impl Iterator<Item = &'static Self> {
            Self::maintained()
                .iter()
                .filter(move |spec| spec.owner == owner)
        }

        pub fn for_crate_and_target(crate_name: &str, target_id: &str) -> Option<&'static Self> {
            Self::maintained()
                .iter()
                .find(|spec| spec.crate_name == crate_name && spec.target.id() == target_id)
        }

        pub fn deterministic_hash(&self) -> String {
            let mut hash = StableHash::new();
            hash.field("schema", "crate-binding-spec-v2");
            hash.field("bindgen_version", CANONICAL_BINDGEN_VERSION);
            hash.field(
                "libclang_version",
                &format!(
                    "{}.{}",
                    CANONICAL_BINDING_LIBCLANG_VERSION.0, CANONICAL_BINDING_LIBCLANG_VERSION.1
                ),
            );
            hash.field("rustc_version", CANONICAL_BINDING_RUSTC_VERSION);
            hash.field("rustfmt_version", CANONICAL_BINDING_RUSTFMT_VERSION);
            hash.field("crate_name", self.crate_name);
            hash.field("crate_root", self.crate_root);
            hash.field("source_root", self.source_root);
            hash.fields("input_paths", self.input_paths);
            hash.begin_list("header_shims", self.header_shims.len());
            for (index, shim) in self.header_shims.iter().enumerate() {
                hash.list_item(index);
                hash.field("name", shim.name);
                hash.field("contents", shim.contents);
            }
            hash.fields("clang_args", self.clang_args);
            hash.fields("target_clang_defines", self.target.clang_defines());
            hash.field(
                "formatter",
                match self.profile.formatter {
                    BindingFormatter::Rustfmt => "rustfmt",
                },
            );
            hash.field(
                "rust_edition",
                match self.profile.rust_edition {
                    BindingRustEdition::Rust2021 => "rust-edition-2021",
                },
            );
            hash.bool_field("derive_default", self.profile.derives.default);
            hash.bool_field("derive_debug", self.profile.derives.debug);
            hash.bool_field("derive_copy", self.profile.derives.copy);
            hash.bool_field("derive_eq", self.profile.derives.eq);
            hash.bool_field("derive_partial_eq", self.profile.derives.partial_eq);
            hash.bool_field("derive_hash", self.profile.derives.hash);
            hash.bool_field("prepend_enum_name", self.profile.prepend_enum_name);
            hash.bool_field("layout_tests", self.profile.layout_tests);
            hash.bool_field("allowlist_recursively", self.profile.allowlist_recursively);
            hash.field("language", self.profile.language.id());
            hash.begin_list("include_paths", self.profile.include_paths.len());
            for (index, include) in self.profile.include_paths.iter().enumerate() {
                hash.list_item(index);
                hash.field("root", include.root.id());
                hash.field("relative_path", include.relative_path);
            }
            hash.fields("clang_defines", self.profile.clang_defines);
            hash.fields("allowlisted_functions", self.profile.allowlisted_functions);
            hash.fields("allowlisted_types", self.profile.allowlisted_types);
            hash.fields("allowlisted_vars", self.profile.allowlisted_vars);
            hash.fields("blocklisted_types", self.profile.blocklisted_types);
            hash.fields("required_symbols", self.required_symbols);
            hash.field("checked_in_path", self.checked_in_path);
            hash.field("target", self.target.id());
            hash.field(
                "wasm_import_module",
                self.target.import_module().unwrap_or_default(),
            );
            hash.finish()
        }

        pub fn validate_checked_in<I, P, C>(
            &self,
            source_revision: &str,
            inputs: I,
            checked_in: &str,
        ) -> Result<(), String>
        where
            I: IntoIterator<Item = (P, C)>,
            P: AsRef<str>,
            C: AsRef<str>,
        {
            let actual = self.validate_embedded(source_revision, checked_in)?;
            let inputs = inputs
                .into_iter()
                .map(|(path, content)| (path.as_ref().to_owned(), content.as_ref().to_owned()))
                .collect::<Vec<_>>();
            let actual_paths = inputs
                .iter()
                .map(|(path, _)| path.as_str())
                .collect::<BTreeSet<_>>();
            let expected_paths = self.input_paths.iter().copied().collect::<BTreeSet<_>>();
            if actual_paths != expected_paths || inputs.len() != self.input_paths.len() {
                return Err(format!(
                    "{} {} binding inputs mismatch: expected {:?}, found {:?}",
                    self.crate_name,
                    self.target.id(),
                    expected_paths,
                    actual_paths
                ));
            }

            let (_, body) = split_binding_provenance(checked_in)?;
            let expected = CrateBindingProvenance::new(self, source_revision, inputs, body);
            if actual != expected {
                return Err(format!(
                    "{} {} binding provenance mismatch: expected {:?}, found {:?}",
                    self.crate_name,
                    self.target.id(),
                    expected.marker(),
                    actual.marker()
                ));
            }
            Ok(())
        }

        pub fn validate_embedded(
            &self,
            source_revision: &str,
            checked_in: &str,
        ) -> Result<CrateBindingProvenance, String> {
            validate_git_revision("source-revision", source_revision)?;
            let (marker, body) = split_binding_provenance(checked_in)?;
            validate_required_symbols(self, body)?;
            let actual = CrateBindingProvenance::parse(marker)?;
            if marker != actual.marker() {
                return Err("binding provenance marker is not in canonical form".to_owned());
            }
            let expected = CrateBindingProvenance {
                crate_name: self.crate_name.to_owned(),
                target: self.target.id().to_owned(),
                source_revision: source_revision.to_owned(),
                spec_hash: self.deterministic_hash(),
                input_hash: actual.input_hash.clone(),
                output_hash: crate_binding_output_hash(body),
            };
            if actual != expected {
                return Err(format!(
                    "{} {} embedded binding provenance mismatch: expected {:?}, found {:?}",
                    self.crate_name,
                    self.target.id(),
                    expected.marker(),
                    actual.marker()
                ));
            }
            Ok(actual)
        }

        pub fn load_and_validate_embedded(&self, crate_root: &Path) -> Result<String, String> {
            let manifest_path = crate_root.join("Cargo.toml");
            let manifest = std::fs::read_to_string(&manifest_path)
                .map_err(|error| format!("read {}: {error}", manifest_path.display()))?;
            let revision = parse_crate_binding_source_revision(&manifest)?;
            let checked_in_path = crate_root.join(self.checked_in_path);
            let checked_in = std::fs::read_to_string(&checked_in_path)
                .map_err(|error| format!("read {}: {error}", checked_in_path.display()))?;
            self.validate_embedded(&revision, &checked_in)?;
            Ok(checked_in)
        }

        pub fn load_and_validate_provenance(
            &self,
            crate_root: &Path,
        ) -> Result<CrateBindingProvenance, String> {
            let manifest_path = crate_root.join("Cargo.toml");
            let manifest = std::fs::read_to_string(&manifest_path)
                .map_err(|error| format!("read {}: {error}", manifest_path.display()))?;
            let revision = parse_crate_binding_source_revision(&manifest)?;
            let checked_in_path = crate_root.join(self.checked_in_path);
            let checked_in = std::fs::read_to_string(&checked_in_path)
                .map_err(|error| format!("read {}: {error}", checked_in_path.display()))?;
            self.validate_embedded(&revision, &checked_in)
        }

        pub fn load_and_validate_full(&self, crate_root: &Path) -> Result<String, String> {
            let manifest_path = crate_root.join("Cargo.toml");
            let manifest = std::fs::read_to_string(&manifest_path)
                .map_err(|error| format!("read {}: {error}", manifest_path.display()))?;
            let revision = parse_crate_binding_source_revision(&manifest)?;
            let mut inputs = Vec::with_capacity(self.input_paths.len());
            for relative_path in self.input_paths {
                let path = crate_root.join(relative_path);
                let content = std::fs::read_to_string(&path)
                    .map_err(|error| format!("read {}: {error}", path.display()))?;
                inputs.push((*relative_path, content));
            }
            let checked_in_path = crate_root.join(self.checked_in_path);
            let checked_in = std::fs::read_to_string(&checked_in_path)
                .map_err(|error| format!("read {}: {error}", checked_in_path.display()))?;
            self.validate_checked_in(
                &revision,
                inputs
                    .iter()
                    .map(|(path, content)| (*path, content.as_str())),
                &checked_in,
            )?;
            Ok(checked_in)
        }

        pub fn copy_embedded_checked_in_to_out_dir(
            &self,
            crate_root: &Path,
            out_dir: &Path,
        ) -> Result<(), String> {
            let checked_in = self.load_and_validate_embedded(crate_root)?;
            let output = out_dir.join("bindings.rs");
            std::fs::write(&output, checked_in)
                .map_err(|error| format!("write {}: {error}", output.display()))
        }

        pub fn stamp(&self, crate_root: &Path, generated: &str) -> Result<String, String> {
            let manifest_path = crate_root.join("Cargo.toml");
            let manifest = std::fs::read_to_string(&manifest_path)
                .map_err(|error| format!("read {}: {error}", manifest_path.display()))?;
            let revision = parse_crate_binding_source_revision(&manifest)?;
            let mut inputs = Vec::with_capacity(self.input_paths.len());
            for relative_path in self.input_paths {
                let path = crate_root.join(relative_path);
                let content = std::fs::read_to_string(&path)
                    .map_err(|error| format!("read {}: {error}", path.display()))?;
                inputs.push((*relative_path, content));
            }
            let (_, body) = split_optional_binding_provenance(generated)?;
            validate_required_symbols(self, body)?;
            Ok(CrateBindingProvenance::new(
                self,
                revision,
                inputs
                    .iter()
                    .map(|(path, content)| (*path, content.as_str())),
                body,
            )
            .embed(body))
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct CrateBindingProvenance {
        pub crate_name: String,
        pub target: String,
        pub source_revision: String,
        pub spec_hash: String,
        pub input_hash: String,
        pub output_hash: String,
    }

    impl CrateBindingProvenance {
        pub fn new<I, P, C>(
            spec: &CrateBindingSpec,
            source_revision: impl Into<String>,
            inputs: I,
            binding_body: &str,
        ) -> Self
        where
            I: IntoIterator<Item = (P, C)>,
            P: AsRef<str>,
            C: AsRef<str>,
        {
            let mut inputs = inputs
                .into_iter()
                .map(|(path, content)| {
                    (
                        path.as_ref().to_owned(),
                        normalize_newlines(content.as_ref()),
                    )
                })
                .collect::<Vec<_>>();
            inputs.sort_unstable_by(|left, right| left.0.cmp(&right.0));

            let mut input_hash = StableHash::new();
            input_hash.field("schema", "crate-binding-inputs-v1");
            input_hash.begin_list("inputs", inputs.len());
            for (index, (path, content)) in inputs.iter().enumerate() {
                input_hash.list_item(index);
                input_hash.field("path", path);
                input_hash.field("content", content);
            }

            Self {
                crate_name: spec.crate_name.to_owned(),
                target: spec.target.id().to_owned(),
                source_revision: source_revision.into(),
                spec_hash: spec.deterministic_hash(),
                input_hash: input_hash.finish(),
                output_hash: crate_binding_output_hash(binding_body),
            }
        }

        pub fn parse(marker: &str) -> Result<Self, String> {
            let fields = marker
                .strip_prefix(CRATE_BINDING_PROVENANCE_PREFIX)
                .and_then(|suffix| suffix.strip_prefix(' '))
                .ok_or_else(|| "invalid crate binding provenance prefix".to_owned())?;
            let mut values = BTreeMap::new();
            for field in fields.split_ascii_whitespace() {
                let (name, value) = field
                    .split_once('=')
                    .ok_or_else(|| format!("malformed binding provenance field {field:?}"))?;
                if !matches!(
                    name,
                    "crate" | "target" | "source" | "spec" | "inputs" | "output"
                ) {
                    return Err(format!("unknown binding provenance field {name}"));
                }
                if value.is_empty() {
                    return Err(format!("binding provenance field {name} is empty"));
                }
                if values.insert(name, value).is_some() {
                    return Err(format!("duplicate binding provenance field {name}"));
                }
            }
            for required in ["crate", "target", "source", "spec", "inputs", "output"] {
                if !values.contains_key(required) {
                    return Err(format!("missing binding provenance field {required}"));
                }
            }
            validate_git_revision("binding provenance source", values["source"])?;
            for hash in ["spec", "inputs", "output"] {
                validate_stable_hash(hash, values[hash])?;
            }
            Ok(Self {
                crate_name: values["crate"].to_owned(),
                target: values["target"].to_owned(),
                source_revision: values["source"].to_owned(),
                spec_hash: values["spec"].to_owned(),
                input_hash: values["inputs"].to_owned(),
                output_hash: values["output"].to_owned(),
            })
        }

        pub fn identity_hash(&self) -> String {
            let mut hash = StableHash::new();
            hash.field("schema", "crate-binding-identity-v1");
            hash.field("crate_name", &self.crate_name);
            hash.field("target", &self.target);
            hash.field("source_revision", &self.source_revision);
            hash.field("spec_hash", &self.spec_hash);
            hash.field("input_hash", &self.input_hash);
            hash.field("output_hash", &self.output_hash);
            hash.finish()
        }

        pub fn marker(&self) -> String {
            format!(
                "{CRATE_BINDING_PROVENANCE_PREFIX} crate={} target={} source={} spec={} inputs={} output={}",
                self.crate_name,
                self.target,
                self.source_revision,
                self.spec_hash,
                self.input_hash,
                self.output_hash
            )
        }

        pub fn embed(&self, binding_body: &str) -> String {
            format!("{}\n{}", self.marker(), binding_body)
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ExtensionBindingIdentity {
        extension: ExtensionBinding,
        provenance: CrateBindingProvenance,
    }

    impl ExtensionBindingIdentity {
        pub fn new(
            extension: ExtensionBinding,
            spec: &CrateBindingSpec,
            provenance: CrateBindingProvenance,
        ) -> Result<Self, String> {
            let artifact = extension.artifact_spec();
            if spec.owner != BindingOwner::Extension(extension) {
                return Err(format!(
                    "{} artifact requires its matching extension binding owner",
                    artifact.sys_crate_name
                ));
            }
            if spec.target != CrateBindingTarget::Native {
                return Err(format!(
                    "{} prebuilt artifacts require native bindings",
                    artifact.sys_crate_name
                ));
            }
            if spec.crate_name != artifact.sys_crate_name {
                return Err(format!(
                    "extension artifact sys crate mismatch: expected {}, found {}",
                    artifact.sys_crate_name, spec.crate_name
                ));
            }
            let expected_spec_hash = spec.deterministic_hash();
            let expected = [
                ("crate", provenance.crate_name.as_str(), spec.crate_name),
                ("target", provenance.target.as_str(), spec.target.id()),
                (
                    "spec",
                    provenance.spec_hash.as_str(),
                    expected_spec_hash.as_str(),
                ),
            ];
            for (field, actual, expected) in expected {
                if actual != expected {
                    return Err(format!(
                        "extension binding {field} mismatch: expected {expected:?}, found {actual:?}"
                    ));
                }
            }
            validate_git_revision("extension binding source", &provenance.source_revision)?;
            validate_stable_hash("extension binding spec", &provenance.spec_hash)?;
            validate_stable_hash("extension binding inputs", &provenance.input_hash)?;
            validate_stable_hash("extension binding output", &provenance.output_hash)?;
            Ok(Self {
                extension,
                provenance,
            })
        }

        pub const fn extension(&self) -> ExtensionBinding {
            self.extension
        }

        pub fn provenance(&self) -> &CrateBindingProvenance {
            &self.provenance
        }

        pub fn deterministic_hash(&self) -> String {
            let mut hash = StableHash::new();
            hash.field("schema", "extension-binding-identity-v1");
            hash.field("extension", self.extension.artifact_spec().extension_id);
            hash.field("provenance", &self.provenance.identity_hash());
            hash.finish()
        }
    }

    fn validate_required_symbols(spec: &CrateBindingSpec, body: &str) -> Result<(), String> {
        for symbol in spec.required_symbols {
            if !body.contains(&format!("pub fn {symbol}(")) {
                return Err(format!(
                    "{} {} bindings are missing required symbol {symbol}",
                    spec.crate_name,
                    spec.target.id()
                ));
            }
        }
        Ok(())
    }

    fn crate_binding_output_hash(binding_body: &str) -> String {
        let mut output_hash = StableHash::new();
        output_hash.field("schema", "crate-binding-output-v1");
        output_hash.field("content", &normalize_newlines(binding_body));
        output_hash.finish()
    }

    fn validate_stable_hash(name: &str, value: &str) -> Result<(), String> {
        let Some(hex) = value.strip_prefix("fnv1a64:") else {
            return Err(format!(
                "binding provenance {name} hash has an invalid prefix"
            ));
        };
        if hex.len() == 16 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            Ok(())
        } else {
            Err(format!(
                "binding provenance {name} hash must contain 16 ASCII hexadecimal characters"
            ))
        }
    }

    pub fn parse_crate_binding_source_revision(content: &str) -> Result<String, String> {
        let mut current_section = "";
        let mut revision = None;
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
            if current_section != CRATE_BINDING_METADATA_SECTION {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            if key != "source-revision" {
                return Err(format!(
                    "unknown key {key} in [{CRATE_BINDING_METADATA_SECTION}]"
                ));
            }
            let value = value.trim();
            let value = value
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
                .ok_or_else(|| {
                    format!(
                        "source-revision in [{CRATE_BINDING_METADATA_SECTION}] must be a quoted string"
                    )
                })?;
            if revision.replace(value.to_owned()).is_some() {
                return Err(format!(
                    "duplicate source-revision in [{CRATE_BINDING_METADATA_SECTION}]"
                ));
            }
        }
        let revision = revision.ok_or_else(|| {
            format!("missing source-revision in [{CRATE_BINDING_METADATA_SECTION}]")
        })?;
        validate_git_revision("source-revision", &revision)?;
        Ok(revision)
    }

    fn split_binding_provenance(content: &str) -> Result<(&str, &str), String> {
        let (marker, body) = content
            .split_once('\n')
            .ok_or_else(|| "checked-in bindings are missing a provenance body".to_owned())?;
        let marker = marker.trim_end_matches('\r');
        if !marker.starts_with(CRATE_BINDING_PROVENANCE_PREFIX) {
            return Err("checked-in bindings are missing crate binding provenance".to_owned());
        }
        if body
            .lines()
            .any(|line| line.starts_with(CRATE_BINDING_PROVENANCE_PREFIX))
        {
            return Err("checked-in bindings contain duplicate provenance markers".to_owned());
        }
        Ok((marker, body))
    }

    fn split_optional_binding_provenance(content: &str) -> Result<(Option<&str>, &str), String> {
        if content.starts_with(CRATE_BINDING_PROVENANCE_PREFIX) {
            split_binding_provenance(content).map(|(marker, body)| (Some(marker), body))
        } else if content
            .lines()
            .any(|line| line.starts_with(CRATE_BINDING_PROVENANCE_PREFIX))
        {
            Err("binding provenance marker must be the first line".to_owned())
        } else {
            Ok((None, content))
        }
    }

    fn normalize_newlines(content: &str) -> String {
        content.replace("\r\n", "\n").replace('\r', "\n")
    }

    const WASM_IMPORT_MODULE: &str = "imgui-sys-v0";
    const CRATE_NATIVE_CLANG_ARGS: &[&str] = &["--target=x86_64-pc-windows-msvc", "-nostdinc"];
    const CRATE_WASM_CLANG_ARGS: &[&str] = &["--target=wasm32-unknown-unknown", "-nostdinc"];
    const CRATE_HEADER_SHIMS: &[HeaderShim] = &[
        HeaderShim {
            name: "stdio.h",
            contents: r#"
#ifndef DEAR_IMGUI_RS_CRATE_STDIO_H
#define DEAR_IMGUI_RS_CRATE_STDIO_H
#include <stdarg.h>
#ifndef DEAR_IMGUI_RS_SIZE_T_DEFINED
#define DEAR_IMGUI_RS_SIZE_T_DEFINED
typedef __SIZE_TYPE__ size_t;
#endif
typedef void FILE;
#endif
"#,
        },
        HeaderShim {
            name: "stddef.h",
            contents: r#"
#ifndef DEAR_IMGUI_RS_CRATE_STDDEF_H
#define DEAR_IMGUI_RS_CRATE_STDDEF_H
#ifndef DEAR_IMGUI_RS_SIZE_T_DEFINED
#define DEAR_IMGUI_RS_SIZE_T_DEFINED
typedef __SIZE_TYPE__ size_t;
#endif
typedef __PTRDIFF_TYPE__ ptrdiff_t;
#endif
"#,
        },
        HeaderShim {
            name: "stdint.h",
            contents: r#"
#ifndef DEAR_IMGUI_RS_CRATE_STDINT_H
#define DEAR_IMGUI_RS_CRATE_STDINT_H
typedef __INT8_TYPE__ int8_t;
typedef __UINT8_TYPE__ uint8_t;
typedef __INT16_TYPE__ int16_t;
typedef __UINT16_TYPE__ uint16_t;
typedef __INT32_TYPE__ int32_t;
typedef __UINT32_TYPE__ uint32_t;
typedef __INT64_TYPE__ int64_t;
typedef __UINT64_TYPE__ uint64_t;
typedef __INTPTR_TYPE__ intptr_t;
typedef __UINTPTR_TYPE__ uintptr_t;
typedef __INTMAX_TYPE__ intmax_t;
typedef __UINTMAX_TYPE__ uintmax_t;
#endif
"#,
        },
        HeaderShim {
            name: "stdarg.h",
            contents: r#"
#ifndef DEAR_IMGUI_RS_CRATE_STDARG_H
#define DEAR_IMGUI_RS_CRATE_STDARG_H
typedef __builtin_va_list va_list;
#define va_start(args, last) __builtin_va_start(args, last)
#define va_end(args) __builtin_va_end(args)
#define va_arg(args, type) __builtin_va_arg(args, type)
#define va_copy(dest, src) __builtin_va_copy(dest, src)
#endif
"#,
        },
        HeaderShim {
            name: "stdbool.h",
            contents: r#"
#ifndef DEAR_IMGUI_RS_CRATE_STDBOOL_H
#define DEAR_IMGUI_RS_CRATE_STDBOOL_H
#ifndef __cplusplus
#define bool _Bool
#define true 1
#define false 0
#endif
#define __bool_true_false_are_defined 1
#endif
"#,
        },
        HeaderShim {
            name: "time.h",
            contents: r#"
#ifndef DEAR_IMGUI_RS_CRATE_TIME_H
#define DEAR_IMGUI_RS_CRATE_TIME_H
typedef __INT64_TYPE__ time_t;
struct tm {
    int tm_sec;
    int tm_min;
    int tm_hour;
    int tm_mday;
    int tm_mon;
    int tm_year;
    int tm_wday;
    int tm_yday;
    int tm_isdst;
};
#endif
"#,
        },
    ];
    const CRATE_WASM_DEFINES: &[&str] = &[
        "IMGUI_DISABLE_FILE_FUNCTIONS",
        "IMGUI_DISABLE_OSX_FUNCTIONS",
        "IMGUI_DISABLE_WIN32_FUNCTIONS",
    ];
    const DEFAULT_CRATE_DERIVES: DerivePolicy = DerivePolicy {
        default: true,
        debug: true,
        copy: true,
        eq: true,
        partial_eq: true,
        hash: true,
    };
    const NODE_EDITOR_DERIVES: DerivePolicy = DerivePolicy {
        default: true,
        debug: true,
        copy: true,
        eq: false,
        partial_eq: false,
        hash: false,
    };

    #[derive(Clone, Copy)]
    struct CrateBindingSymbols {
        functions: &'static [&'static str],
        types: &'static [&'static str],
        vars: &'static [&'static str],
        blocked_types: &'static [&'static str],
    }

    const fn crate_profile(
        language: CrateBindingLanguage,
        include_paths: &'static [CrateBindingInclude],
        clang_defines: &'static [&'static str],
        symbols: CrateBindingSymbols,
        derives: DerivePolicy,
        allowlist_recursively: bool,
    ) -> CrateBindgenProfile {
        CrateBindgenProfile {
            formatter: BindingFormatter::Rustfmt,
            rust_edition: BindingRustEdition::Rust2021,
            derives,
            prepend_enum_name: false,
            layout_tests: false,
            allowlist_recursively,
            language,
            include_paths,
            clang_defines,
            allowlisted_functions: symbols.functions,
            allowlisted_types: symbols.types,
            allowlisted_vars: symbols.vars,
            blocklisted_types: symbols.blocked_types,
        }
    }

    const TEST_ENGINE_INPUTS: &[&str] = &["shim/cimgui_test_engine.h"];
    const TEST_ENGINE_FUNCTIONS: &[&str] = &["imgui_test_engine_.*"];
    const TEST_ENGINE_TYPES: &[&str] = &["ImGuiTestEngine.*"];
    const TEST_ENGINE_VARS: &[&str] = &["ImGuiTestEngine.*"];
    const TEST_ENGINE_BLOCKLIST_TYPES: &[&str] = &["ImGuiContext"];
    const TEST_ENGINE_PROFILE: CrateBindgenProfile = crate_profile(
        CrateBindingLanguage::C,
        &[],
        &[],
        CrateBindingSymbols {
            functions: TEST_ENGINE_FUNCTIONS,
            types: TEST_ENGINE_TYPES,
            vars: TEST_ENGINE_VARS,
            blocked_types: TEST_ENGINE_BLOCKLIST_TYPES,
        },
        DEFAULT_CRATE_DERIVES,
        true,
    );
    const TEST_ENGINE_SYMBOLS: &[&str] = &[
        "imgui_test_engine_create_context",
        "imgui_test_engine_register_script_test",
    ];

    const IMPLOT_INPUTS: &[&str] = &["third-party/cimplot/cimplot.h"];
    const IMPLOT_INCLUDES: &[CrateBindingInclude] = &[
        CrateBindingInclude {
            root: CrateBindingIncludeRoot::CoreImgui,
            relative_path: "",
        },
        CrateBindingInclude {
            root: CrateBindingIncludeRoot::CoreCimgui,
            relative_path: "",
        },
        CrateBindingInclude {
            root: CrateBindingIncludeRoot::Source,
            relative_path: "",
        },
        CrateBindingInclude {
            root: CrateBindingIncludeRoot::Source,
            relative_path: "implot",
        },
    ];
    const IMPLOT_DEFINES: &[&str] = &[
        "IMGUI_USE_WCHAR32",
        "CIMGUI_DEFINE_ENUMS_AND_STRUCTS",
        "CIMGUI_VARGS0",
    ];
    const IMPLOT_FUNCTIONS: &[&str] = &["ImPlot.*"];
    const IMPLOT_TYPES: &[&str] = &["ImPlot.*", "ImWchar32"];
    const IMPLOT_VARS: &[&str] = &["ImPlot.*", "IMPLOT_.*"];
    const IMPLOT_BLOCKLIST_TYPES: &[&str] = &[
        "ImVec2",
        "ImVec4",
        "ImGuiCond",
        "ImTextureID",
        "ImGuiContext",
        "ImDrawList",
        "ImGuiMouseButton",
        "ImGuiDragDropFlags",
        "ImGuiIO",
        "ImFontAtlas",
        "ImDrawData",
        "ImGuiStyle",
        "ImGuiKeyModFlags",
    ];
    const IMPLOT_PROFILE: CrateBindgenProfile = crate_profile(
        CrateBindingLanguage::Cxx17,
        IMPLOT_INCLUDES,
        IMPLOT_DEFINES,
        CrateBindingSymbols {
            functions: IMPLOT_FUNCTIONS,
            types: IMPLOT_TYPES,
            vars: IMPLOT_VARS,
            blocked_types: IMPLOT_BLOCKLIST_TYPES,
        },
        DEFAULT_CRATE_DERIVES,
        true,
    );
    const IMPLOT_SYMBOLS: &[&str] = &[
        "ImPlot_Annotation_Str0",
        "ImPlot_TagX_Str0",
        "ImPlot_TagY_Str0",
        "ImPlot_GetPlotPos",
        "ImPlot_GetPlotSize",
    ];

    const IMPLOT3D_INPUTS: &[&str] = &["third-party/cimplot3d/cimplot3d.h"];
    const IMPLOT3D_INCLUDES: &[CrateBindingInclude] = &[
        CrateBindingInclude {
            root: CrateBindingIncludeRoot::CoreCimgui,
            relative_path: "",
        },
        CrateBindingInclude {
            root: CrateBindingIncludeRoot::CoreImgui,
            relative_path: "",
        },
        CrateBindingInclude {
            root: CrateBindingIncludeRoot::Source,
            relative_path: "",
        },
        CrateBindingInclude {
            root: CrateBindingIncludeRoot::Source,
            relative_path: "implot3d",
        },
    ];
    const IMPLOT3D_DEFINES: &[&str] = &["IMGUI_USE_WCHAR32", "CIMGUI_DEFINE_ENUMS_AND_STRUCTS"];
    const IMPLOT3D_FUNCTIONS: &[&str] = &["ImPlot3D_.*"];
    const IMPLOT3D_TYPES: &[&str] = &["ImPlot3D.*", "ImWchar32"];
    const IMPLOT3D_VARS: &[&str] = &["ImPlot3D.*"];
    const IMPLOT3D_BLOCKLIST_TYPES: &[&str] = &[
        "ImVec2",
        "ImVec4",
        "ImGuiContext",
        "ImDrawList",
        "ImGuiID",
        "ImTextureID",
    ];
    const IMPLOT3D_PROFILE: CrateBindgenProfile = crate_profile(
        CrateBindingLanguage::Cxx17,
        IMPLOT3D_INCLUDES,
        IMPLOT3D_DEFINES,
        CrateBindingSymbols {
            functions: IMPLOT3D_FUNCTIONS,
            types: IMPLOT3D_TYPES,
            vars: IMPLOT3D_VARS,
            blocked_types: IMPLOT3D_BLOCKLIST_TYPES,
        },
        DEFAULT_CRATE_DERIVES,
        true,
    );
    const IMPLOT3D_SYMBOLS: &[&str] = &[
        "ImPlot3D_PlotToPixels_double",
        "ImPlot3D_GetPlotRectPos",
        "ImPlot3D_GetPlotRectSize",
        "ImPlot3D_NextColormapColor",
        "ImPlot3D_GetColormapColor",
    ];

    const IMNODES_INPUTS: &[&str] = &["third-party/cimnodes/cimnodes.h", "shim/imnodes_extra.h"];
    const IMNODES_INCLUDES: &[CrateBindingInclude] = &[
        CrateBindingInclude {
            root: CrateBindingIncludeRoot::CoreCimgui,
            relative_path: "",
        },
        CrateBindingInclude {
            root: CrateBindingIncludeRoot::CoreImgui,
            relative_path: "",
        },
        CrateBindingInclude {
            root: CrateBindingIncludeRoot::Source,
            relative_path: "",
        },
        CrateBindingInclude {
            root: CrateBindingIncludeRoot::Source,
            relative_path: "imnodes",
        },
    ];
    const IMNODES_DEFINES: &[&str] = &["IMGUI_USE_WCHAR32", "CIMGUI_DEFINE_ENUMS_AND_STRUCTS"];
    const IMNODES_FUNCTIONS: &[&str] = &[
        "imnodes_.*",
        "EmulateThreeButtonMouse_.*",
        "LinkDetachWithModifierClick_.*",
        "MultipleSelectModifier_.*",
        "getIOKeyCtrlPtr",
        "imnodes_getIOKeyShiftPtr",
        "imnodes_getIOKeyAltPtr",
    ];
    const IMNODES_TYPES: &[&str] = &["ImNodes.*", "ImWchar32"];
    const IMNODES_VARS: &[&str] = &["ImNodes.*"];
    const IMNODES_BLOCKLIST_TYPES: &[&str] = &["ImVec2", "ImVec4", "ImGuiContext", "ImDrawList"];
    const IMNODES_PROFILE: CrateBindgenProfile = crate_profile(
        CrateBindingLanguage::Cxx17,
        IMNODES_INCLUDES,
        IMNODES_DEFINES,
        CrateBindingSymbols {
            functions: IMNODES_FUNCTIONS,
            types: IMNODES_TYPES,
            vars: IMNODES_VARS,
            blocked_types: IMNODES_BLOCKLIST_TYPES,
        },
        DEFAULT_CRATE_DERIVES,
        true,
    );
    const IMNODES_SYMBOLS: &[&str] = &[
        "imnodes_EditorContextGetPanning",
        "imnodes_getIOKeyShiftPtr",
        "imnodes_getIOKeyAltPtr",
        "imnodes_EditorContextResetToDefault",
        "imnodes_EditorContextGetCurrent",
        "imnodes_EditorContextResetToDefaultIfCurrent",
        "imnodes_GetNodeScreenSpacePos",
        "imnodes_GetNodeEditorSpacePos",
        "imnodes_GetNodeDimensions",
    ];

    const NODE_EDITOR_INPUTS: &[&str] = &["shim/node_editor_extra.h"];
    const NODE_EDITOR_INCLUDES: &[CrateBindingInclude] = &[
        CrateBindingInclude {
            root: CrateBindingIncludeRoot::CoreCimgui,
            relative_path: "",
        },
        CrateBindingInclude {
            root: CrateBindingIncludeRoot::CoreImgui,
            relative_path: "",
        },
        CrateBindingInclude {
            root: CrateBindingIncludeRoot::Source,
            relative_path: "",
        },
        CrateBindingInclude {
            root: CrateBindingIncludeRoot::Source,
            relative_path: "imgui-node-editor",
        },
    ];
    const NODE_EDITOR_DEFINES: &[&str] = &["IMGUI_USE_WCHAR32", "CIMGUI_DEFINE_ENUMS_AND_STRUCTS"];
    const NODE_EDITOR_FUNCTIONS: &[&str] = &["dne_.*"];
    const NODE_EDITOR_TYPES: &[&str] = &["Dne.*"];
    const NODE_EDITOR_VARS: &[&str] = &["DNE_.*"];
    const NODE_EDITOR_BLOCKLIST_TYPES: &[&str] = &["Im.*"];
    const NODE_EDITOR_PROFILE: CrateBindgenProfile = crate_profile(
        CrateBindingLanguage::Cxx17,
        NODE_EDITOR_INCLUDES,
        NODE_EDITOR_DEFINES,
        CrateBindingSymbols {
            functions: NODE_EDITOR_FUNCTIONS,
            types: NODE_EDITOR_TYPES,
            vars: NODE_EDITOR_VARS,
            blocked_types: NODE_EDITOR_BLOCKLIST_TYPES,
        },
        NODE_EDITOR_DERIVES,
        false,
    );
    const NODE_EDITOR_SYMBOLS: &[&str] = &["dne_create_editor", "dne_pin_rect"];

    const IMGUIZMO_INPUTS: &[&str] = &["third-party/cimguizmo/cimguizmo.h"];
    const IMGUIZMO_INCLUDES: &[CrateBindingInclude] = &[
        CrateBindingInclude {
            root: CrateBindingIncludeRoot::CoreCimgui,
            relative_path: "",
        },
        CrateBindingInclude {
            root: CrateBindingIncludeRoot::CoreImgui,
            relative_path: "",
        },
        CrateBindingInclude {
            root: CrateBindingIncludeRoot::Source,
            relative_path: "",
        },
        CrateBindingInclude {
            root: CrateBindingIncludeRoot::Source,
            relative_path: "ImGuizmo",
        },
        CrateBindingInclude {
            root: CrateBindingIncludeRoot::Source,
            relative_path: "ImGuizmo/src",
        },
    ];
    const IMGUIZMO_DEFINES: &[&str] = &["IMGUI_USE_WCHAR32", "CIMGUI_DEFINE_ENUMS_AND_STRUCTS"];
    const IMGUIZMO_FUNCTIONS: &[&str] = &["ImGuizmo_.*", "Style_.*"];
    const IMGUIZMO_TYPES: &[&str] = &["(Style|COLOR|MODE|OPERATION)", "ImWchar32"];
    const IMGUIZMO_VARS: &[&str] =
        &["(COLOR|MODE|OPERATION|COUNT|TRANSLATE.*|ROTATE.*|SCALE.*|UNIVERSAL)"];
    const IMGUIZMO_BLOCKLIST_TYPES: &[&str] =
        &["ImVec2", "ImVec4", "ImGuiContext", "ImDrawList", "ImGuiID"];
    const IMGUIZMO_PROFILE: CrateBindgenProfile = crate_profile(
        CrateBindingLanguage::Cxx17,
        IMGUIZMO_INCLUDES,
        IMGUIZMO_DEFINES,
        CrateBindingSymbols {
            functions: IMGUIZMO_FUNCTIONS,
            types: IMGUIZMO_TYPES,
            vars: IMGUIZMO_VARS,
            blocked_types: IMGUIZMO_BLOCKLIST_TYPES,
        },
        DEFAULT_CRATE_DERIVES,
        true,
    );
    const IMGUIZMO_SYMBOLS: &[&str] = &["ImGuizmo_BeginFrame", "ImGuizmo_Manipulate"];

    const IMGUIZMO_QUAT_INPUTS: &[&str] = &["third-party/cimguizmo_quat/cimguizmo_quat.h"];
    const IMGUIZMO_QUAT_INCLUDES: &[CrateBindingInclude] = &[
        CrateBindingInclude {
            root: CrateBindingIncludeRoot::CoreCimgui,
            relative_path: "",
        },
        CrateBindingInclude {
            root: CrateBindingIncludeRoot::CoreImgui,
            relative_path: "",
        },
        CrateBindingInclude {
            root: CrateBindingIncludeRoot::Source,
            relative_path: "",
        },
        CrateBindingInclude {
            root: CrateBindingIncludeRoot::Source,
            relative_path: "imGuIZMO.quat/imguizmo_quat",
        },
    ];
    const IMGUIZMO_QUAT_DEFINES: &[&str] =
        &["IMGUI_USE_WCHAR32", "CIMGUI_DEFINE_ENUMS_AND_STRUCTS"];
    const IMGUIZMO_QUAT_FUNCTIONS: &[&str] = &["imguiGizmo_.*", "iggizmo3D_.*", "(mat4|quat)_.*"];
    const IMGUIZMO_QUAT_BLOCKLIST_TYPES: &[&str] =
        &["ImVec2", "ImDrawList", "ImGuiContext", "ImGuiID", "ImVec4"];
    const IMGUIZMO_QUAT_PROFILE: CrateBindgenProfile = crate_profile(
        CrateBindingLanguage::Cxx17,
        IMGUIZMO_QUAT_INCLUDES,
        IMGUIZMO_QUAT_DEFINES,
        CrateBindingSymbols {
            functions: IMGUIZMO_QUAT_FUNCTIONS,
            types: &[],
            vars: &[],
            blocked_types: IMGUIZMO_QUAT_BLOCKLIST_TYPES,
        },
        DEFAULT_CRATE_DERIVES,
        true,
    );
    const IMGUIZMO_QUAT_SYMBOLS: &[&str] = &["imguiGizmo_buildPlane", "quat_cast"];

    #[derive(Clone, Copy)]
    struct CrateBindingSourceSpec {
        owner: BindingOwner,
        crate_name: &'static str,
        crate_root: &'static str,
        source_root: &'static str,
        input_paths: &'static [&'static str],
        profile: CrateBindgenProfile,
        required_symbols: &'static [&'static str],
    }

    const fn source_spec(
        owner: BindingOwner,
        crate_name: &'static str,
        crate_root: &'static str,
        source_root: &'static str,
        input_paths: &'static [&'static str],
        profile: CrateBindgenProfile,
        required_symbols: &'static [&'static str],
    ) -> CrateBindingSourceSpec {
        CrateBindingSourceSpec {
            owner,
            crate_name,
            crate_root,
            source_root,
            input_paths,
            profile,
            required_symbols,
        }
    }

    const fn spec(
        source: CrateBindingSourceSpec,
        checked_in_path: &'static str,
        target: CrateBindingTarget,
    ) -> CrateBindingSpec {
        let clang_args = match target {
            CrateBindingTarget::Native => CRATE_NATIVE_CLANG_ARGS,
            CrateBindingTarget::WasmImport { .. } => CRATE_WASM_CLANG_ARGS,
        };
        CrateBindingSpec {
            owner: source.owner,
            crate_name: source.crate_name,
            crate_root: source.crate_root,
            source_root: source.source_root,
            input_paths: source.input_paths,
            header_shims: CRATE_HEADER_SHIMS,
            clang_args,
            profile: source.profile,
            required_symbols: source.required_symbols,
            checked_in_path,
            target,
        }
    }

    const TEST_ENGINE_SOURCE: CrateBindingSourceSpec = source_spec(
        BindingOwner::TestEngine,
        "dear-imgui-test-engine-sys",
        "extensions/dear-imgui-test-engine-sys",
        "third-party/imgui_test_engine",
        TEST_ENGINE_INPUTS,
        TEST_ENGINE_PROFILE,
        TEST_ENGINE_SYMBOLS,
    );
    const IMPLOT_SOURCE: CrateBindingSourceSpec = source_spec(
        BindingOwner::Extension(ExtensionBinding::ImPlot),
        "dear-implot-sys",
        "extensions/dear-implot-sys",
        "third-party/cimplot",
        IMPLOT_INPUTS,
        IMPLOT_PROFILE,
        IMPLOT_SYMBOLS,
    );
    const IMPLOT3D_SOURCE: CrateBindingSourceSpec = source_spec(
        BindingOwner::Extension(ExtensionBinding::ImPlot3d),
        "dear-implot3d-sys",
        "extensions/dear-implot3d-sys",
        "third-party/cimplot3d",
        IMPLOT3D_INPUTS,
        IMPLOT3D_PROFILE,
        IMPLOT3D_SYMBOLS,
    );
    const IMNODES_SOURCE: CrateBindingSourceSpec = source_spec(
        BindingOwner::Extension(ExtensionBinding::ImNodes),
        "dear-imnodes-sys",
        "extensions/dear-imnodes-sys",
        "third-party/cimnodes",
        IMNODES_INPUTS,
        IMNODES_PROFILE,
        IMNODES_SYMBOLS,
    );
    const NODE_EDITOR_SOURCE: CrateBindingSourceSpec = source_spec(
        BindingOwner::Extension(ExtensionBinding::NodeEditor),
        "dear-node-editor-sys",
        "extensions/dear-node-editor-sys",
        "third-party/cimnodes_editor",
        NODE_EDITOR_INPUTS,
        NODE_EDITOR_PROFILE,
        NODE_EDITOR_SYMBOLS,
    );
    const IMGUIZMO_SOURCE: CrateBindingSourceSpec = source_spec(
        BindingOwner::Extension(ExtensionBinding::ImGuizmo),
        "dear-imguizmo-sys",
        "extensions/dear-imguizmo-sys",
        "third-party/cimguizmo",
        IMGUIZMO_INPUTS,
        IMGUIZMO_PROFILE,
        IMGUIZMO_SYMBOLS,
    );
    const IMGUIZMO_QUAT_SOURCE: CrateBindingSourceSpec = source_spec(
        BindingOwner::Extension(ExtensionBinding::ImGuizmoQuat),
        "dear-imguizmo-quat-sys",
        "extensions/dear-imguizmo-quat-sys",
        "third-party/cimguizmo_quat",
        IMGUIZMO_QUAT_INPUTS,
        IMGUIZMO_QUAT_PROFILE,
        IMGUIZMO_QUAT_SYMBOLS,
    );

    const MAINTAINED_CRATE_BINDING_SPECS: &[CrateBindingSpec] = &[
        spec(
            TEST_ENGINE_SOURCE,
            "src/bindings_pregenerated.rs",
            CrateBindingTarget::Native,
        ),
        spec(
            IMPLOT_SOURCE,
            "src/bindings_pregenerated.rs",
            CrateBindingTarget::Native,
        ),
        spec(
            IMPLOT_SOURCE,
            "src/wasm_bindings_pregenerated.rs",
            CrateBindingTarget::WasmImport {
                module_name: WASM_IMPORT_MODULE,
            },
        ),
        spec(
            IMPLOT3D_SOURCE,
            "src/bindings_pregenerated.rs",
            CrateBindingTarget::Native,
        ),
        spec(
            IMPLOT3D_SOURCE,
            "src/wasm_bindings_pregenerated.rs",
            CrateBindingTarget::WasmImport {
                module_name: WASM_IMPORT_MODULE,
            },
        ),
        spec(
            IMNODES_SOURCE,
            "src/bindings_pregenerated.rs",
            CrateBindingTarget::Native,
        ),
        spec(
            IMNODES_SOURCE,
            "src/wasm_bindings_pregenerated.rs",
            CrateBindingTarget::WasmImport {
                module_name: WASM_IMPORT_MODULE,
            },
        ),
        spec(
            NODE_EDITOR_SOURCE,
            "src/bindings_pregenerated.rs",
            CrateBindingTarget::Native,
        ),
        spec(
            IMGUIZMO_SOURCE,
            "src/bindings_pregenerated.rs",
            CrateBindingTarget::Native,
        ),
        spec(
            IMGUIZMO_SOURCE,
            "src/wasm_bindings_pregenerated.rs",
            CrateBindingTarget::WasmImport {
                module_name: WASM_IMPORT_MODULE,
            },
        ),
        spec(
            IMGUIZMO_QUAT_SOURCE,
            "src/bindings_pregenerated.rs",
            CrateBindingTarget::Native,
        ),
        spec(
            IMGUIZMO_QUAT_SOURCE,
            "src/wasm_bindings_pregenerated.rs",
            CrateBindingTarget::WasmImport {
                module_name: WASM_IMPORT_MODULE,
            },
        ),
    ];

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

        pub fn release_manifest_bytes(&self, candidate_sha: &str) -> Result<Vec<u8>, String> {
            let identity = CoreArtifactIdentity::new(self, candidate_sha)?;
            Ok(format!(
                "{} prebuilt\nversion={}\ncandidate_sha={}\ntarget={}\nlink={}\ncrt={}\nfeatures={}\ncimgui_revision={}\nimgui_revision={}\nbinding_spec_hash={}\n",
                self.crate_name,
                self.version,
                identity.candidate_sha,
                self.target,
                self.link_type,
                self.crt,
                self.features.join(","),
                self.source_revisions.cimgui,
                self.source_revisions.imgui,
                self.binding_spec_hash,
            )
            .into_bytes())
        }

        pub fn validate_release_manifest_bytes(
            &self,
            bytes: &[u8],
        ) -> Result<CoreArtifactIdentity, String> {
            let manifest = ParsedManifest::parse(bytes)?;
            let features = self.features.join(",");
            let candidate_sha = manifest.field("candidate_sha").unwrap_or_default();
            let expected = [
                ("crate_name", self.crate_name.as_str()),
                ("version", self.version.as_str()),
                ("candidate_sha", candidate_sha),
                ("target", self.target.as_str()),
                ("link", self.link_type.as_str()),
                ("crt", self.crt.as_str()),
                ("features", features.as_str()),
                ("cimgui_revision", self.source_revisions.cimgui.as_str()),
                ("imgui_revision", self.source_revisions.imgui.as_str()),
                ("binding_spec_hash", self.binding_spec_hash.as_str()),
            ];
            validate_manifest_fields(&manifest, &expected)?;
            CoreArtifactIdentity::new(self, candidate_sha)
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct CoreArtifactIdentity {
        pub profile_hash: String,
        pub candidate_sha: String,
    }

    impl CoreArtifactIdentity {
        pub fn new(profile: &ArtifactProfile, candidate_sha: &str) -> Result<Self, String> {
            validate_git_revision("release candidate SHA", candidate_sha)?;
            if candidate_sha != candidate_sha.to_ascii_lowercase() {
                return Err("release candidate SHA must use lowercase hexadecimal".to_owned());
            }
            let profile_hash = profile.deterministic_hash();
            validate_stable_hash("core artifact profile", &profile_hash)?;
            Ok(Self {
                profile_hash,
                candidate_sha: candidate_sha.to_owned(),
            })
        }

        pub fn deterministic_hash(&self) -> String {
            let mut hash = StableHash::new();
            hash.field("schema", "core-artifact-identity-v1");
            hash.field("profile", &self.profile_hash);
            hash.field("candidate_sha", &self.candidate_sha);
            hash.finish()
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ExtensionArtifactProfile {
        pub extension: ExtensionBinding,
        pub safe_crate_name: String,
        pub sys_crate_name: String,
        pub library_name: String,
        pub archive_name: String,
        pub version: String,
        pub candidate_sha: String,
        pub target: String,
        pub link_type: String,
        pub crt: String,
        pub features: Vec<String>,
        pub core_artifact_identity_hash: String,
        pub extension_binding_identity_hash: String,
    }

    pub struct ExtensionArtifactProfileInput<'a> {
        pub extension: ExtensionBinding,
        pub crate_root: &'a Path,
        pub version: &'a str,
        pub candidate_sha: &'a str,
        pub target: &'a str,
        pub link_type: &'a str,
        pub crt: &'a str,
        pub features: &'a [&'a str],
        pub core_candidate_sha: &'a str,
        pub core_artifact_identity_hash: &'a str,
    }

    struct ResolvedExtensionArtifactProfileInput {
        version: String,
        candidate_sha: String,
        target: String,
        link_type: String,
        crt: String,
        features: Vec<String>,
        core_candidate_sha: String,
        core_artifact_identity_hash: String,
    }

    impl ExtensionArtifactProfileInput<'_> {
        pub fn build_for_consumer(&self) -> Result<ExtensionArtifactProfile, String> {
            self.build_with_binding_validation(false)
        }

        pub fn build_for_package(&self) -> Result<ExtensionArtifactProfile, String> {
            self.build_with_binding_validation(true)
        }

        fn build_with_binding_validation(
            &self,
            validate_full_binding: bool,
        ) -> Result<ExtensionArtifactProfile, String> {
            let owner = BindingOwner::Extension(self.extension);
            let spec = CrateBindingSpec::for_owner(owner)
                .find(|spec| spec.target == CrateBindingTarget::Native)
                .ok_or_else(|| {
                    format!(
                        "missing native binding spec for {}",
                        self.extension.artifact_spec().sys_crate_name
                    )
                })?;
            if validate_full_binding {
                spec.load_and_validate_full(self.crate_root)?;
            }
            let provenance = spec.load_and_validate_provenance(self.crate_root)?;
            let identity = ExtensionBindingIdentity::new(self.extension, spec, provenance)?;
            ExtensionArtifactProfile::from_resolved_input(
                ResolvedExtensionArtifactProfileInput {
                    version: self.version.to_owned(),
                    candidate_sha: self.candidate_sha.to_owned(),
                    target: self.target.to_owned(),
                    link_type: self.link_type.to_owned(),
                    crt: self.crt.to_owned(),
                    features: normalize_values(self.features.iter().copied()),
                    core_candidate_sha: self.core_candidate_sha.to_owned(),
                    core_artifact_identity_hash: self.core_artifact_identity_hash.to_owned(),
                },
                identity,
            )
        }
    }

    pub fn extension_artifact_profile_from_env(
        extension: ExtensionBinding,
        crate_root: &Path,
        version: &str,
        target: &str,
        crt: &str,
        features: &[&str],
        package_mode: bool,
    ) -> Result<ExtensionArtifactProfile, String> {
        let candidate_env = if package_mode {
            RELEASE_CANDIDATE_SHA_ENV
        } else {
            DEP_CORE_CANDIDATE_SHA_ENV
        };
        let candidate_sha = std::env::var(candidate_env).map_err(|_| {
            format!(
                "{} prebuilt route requires validated candidate metadata {candidate_env}",
                extension.artifact_spec().sys_crate_name
            )
        })?;
        let core_hash_env = if package_mode {
            RELEASE_CORE_ARTIFACT_IDENTITY_HASH_ENV
        } else {
            DEP_CORE_ARTIFACT_IDENTITY_HASH_ENV
        };
        let core_artifact_identity_hash = std::env::var(core_hash_env).map_err(|_| {
            format!(
                "{} prebuilt route requires validated core artifact metadata {core_hash_env}",
                extension.artifact_spec().sys_crate_name
            )
        })?;
        let input = ExtensionArtifactProfileInput {
            extension,
            crate_root,
            version,
            candidate_sha: &candidate_sha,
            target,
            link_type: "static",
            crt,
            features,
            core_candidate_sha: &candidate_sha,
            core_artifact_identity_hash: &core_artifact_identity_hash,
        };
        if package_mode {
            input.build_for_package()
        } else {
            input.build_for_consumer()
        }
    }

    impl ExtensionArtifactProfile {
        pub fn new<I, S>(
            core_profile: &ArtifactProfile,
            candidate_sha: impl Into<String>,
            features: I,
            binding_identity: ExtensionBindingIdentity,
        ) -> Result<Self, String>
        where
            I: IntoIterator<Item = S>,
            S: AsRef<str>,
        {
            let features = normalize_values(features);
            let extension = binding_identity.extension();
            if core_profile.crate_name != "dear-imgui" {
                return Err(format!(
                    "extension artifact requires a dear-imgui core profile, found {:?}",
                    core_profile.crate_name
                ));
            }
            validate_git_revision(
                "core cimgui revision",
                &core_profile.source_revisions.cimgui,
            )?;
            validate_git_revision("core imgui revision", &core_profile.source_revisions.imgui)?;
            validate_stable_hash("core binding spec", &core_profile.binding_spec_hash)?;
            if core_profile
                .features
                .iter()
                .any(|feature| feature == "test-engine")
            {
                return Err(
                    "Test Engine is source-only and cannot enter an extension artifact profile"
                        .to_owned(),
                );
            }
            validate_extension_artifact_features(extension, &features)?;
            for feature in &features {
                if !core_profile.features.contains(feature) {
                    return Err(format!(
                        "extension artifact feature {feature:?} is absent from the core artifact profile"
                    ));
                }
            }
            let candidate_sha = candidate_sha.into();
            let core_identity = CoreArtifactIdentity::new(core_profile, &candidate_sha)?;
            Self::from_resolved_input(
                ResolvedExtensionArtifactProfileInput {
                    version: core_profile.version.clone(),
                    candidate_sha,
                    target: core_profile.target.clone(),
                    link_type: core_profile.link_type.clone(),
                    crt: core_profile.crt.clone(),
                    features,
                    core_candidate_sha: core_identity.candidate_sha.clone(),
                    core_artifact_identity_hash: core_identity.deterministic_hash(),
                },
                binding_identity,
            )
        }

        fn from_resolved_input(
            input: ResolvedExtensionArtifactProfileInput,
            binding_identity: ExtensionBindingIdentity,
        ) -> Result<Self, String> {
            let ResolvedExtensionArtifactProfileInput {
                version,
                candidate_sha,
                target,
                link_type,
                crt,
                features,
                core_candidate_sha,
                core_artifact_identity_hash,
            } = input;
            let extension = binding_identity.extension();
            let artifact = extension.artifact_spec();

            validate_git_revision("release candidate SHA", &candidate_sha)?;
            validate_git_revision("core release candidate SHA", &core_candidate_sha)?;
            if candidate_sha != candidate_sha.to_ascii_lowercase()
                || core_candidate_sha != core_candidate_sha.to_ascii_lowercase()
            {
                return Err("artifact candidate SHA must use lowercase hexadecimal".to_owned());
            }
            if candidate_sha != core_candidate_sha {
                return Err(format!(
                    "extension candidate SHA mismatch: expected core candidate {core_candidate_sha}, found {candidate_sha}"
                ));
            }
            validate_stable_hash("core artifact identity", &core_artifact_identity_hash)?;
            if version.is_empty() {
                return Err("extension artifact version cannot be empty".to_owned());
            }
            if target.starts_with("wasm32") {
                return Err(format!(
                    "{} prebuilt artifacts are native-only and cannot target {target}",
                    artifact.sys_crate_name
                ));
            }
            if link_type != "static" {
                return Err(format!(
                    "{} prebuilt artifact link type must be static",
                    artifact.sys_crate_name
                ));
            }
            if !matches!(crt.as_str(), "" | "md" | "mt") {
                return Err(format!("unsupported extension artifact CRT {crt:?}"));
            }
            let target_is_msvc = target.ends_with("-msvc");
            if target_is_msvc == crt.is_empty() {
                return Err(format!(
                    "extension artifact CRT {crt:?} does not match target {target:?}"
                ));
            }
            validate_extension_artifact_features(extension, &features)?;
            let archive_name =
                extension_archive_name(extension, &version, &target, &link_type, &crt, &features);
            Ok(Self {
                extension,
                safe_crate_name: artifact.safe_crate_name.to_owned(),
                sys_crate_name: artifact.sys_crate_name.to_owned(),
                library_name: artifact.library_name.to_owned(),
                archive_name,
                version,
                candidate_sha,
                target,
                link_type,
                crt,
                features,
                core_artifact_identity_hash,
                extension_binding_identity_hash: binding_identity.deterministic_hash(),
            })
        }

        pub fn deterministic_hash(&self) -> String {
            let mut hash = StableHash::new();
            hash.field("schema", "extension-artifact-profile-v2");
            hash.field("extension", self.extension.artifact_spec().extension_id);
            hash.field("safe_crate", &self.safe_crate_name);
            hash.field("sys_crate", &self.sys_crate_name);
            hash.field("library", &self.library_name);
            hash.field("archive", &self.archive_name);
            hash.field("version", &self.version);
            hash.field("candidate_sha", &self.candidate_sha);
            hash.field("target", &self.target);
            hash.field("link_type", &self.link_type);
            hash.field("crt", &self.crt);
            hash.fields("features", &self.features);
            hash.field("core_artifact_identity", &self.core_artifact_identity_hash);
            hash.field(
                "extension_binding_identity",
                &self.extension_binding_identity_hash,
            );
            hash.finish()
        }

        pub fn cache_key(&self) -> String {
            self.deterministic_hash().replace(':', "-")
        }

        pub fn manifest_bytes(&self) -> Vec<u8> {
            format!(
                "{} prebuilt\nversion={}\ncandidate_sha={}\ntarget={}\nlink={}\ncrt={}\nfeatures={}\nextension={}\nsafe_crate={}\nlibrary={}\narchive={}\ncore_artifact_identity={}\nextension_binding_identity={}\n",
                self.sys_crate_name,
                self.version,
                self.candidate_sha,
                self.target,
                self.link_type,
                self.crt,
                self.features.join(","),
                self.extension.artifact_spec().extension_id,
                self.safe_crate_name,
                self.library_name,
                self.archive_name,
                self.core_artifact_identity_hash,
                self.extension_binding_identity_hash,
            )
            .into_bytes()
        }

        pub fn validate_manifest_bytes(&self, bytes: &[u8]) -> Result<(), String> {
            let manifest = ParsedManifest::parse(bytes)?;
            let features = self.features.join(",");
            let extension = self.extension.artifact_spec().extension_id;
            let expected = [
                ("crate_name", self.sys_crate_name.as_str()),
                ("version", self.version.as_str()),
                ("candidate_sha", self.candidate_sha.as_str()),
                ("target", self.target.as_str()),
                ("link", self.link_type.as_str()),
                ("crt", self.crt.as_str()),
                ("features", features.as_str()),
                ("extension", extension),
                ("safe_crate", self.safe_crate_name.as_str()),
                ("library", self.library_name.as_str()),
                ("archive", self.archive_name.as_str()),
                (
                    "core_artifact_identity",
                    self.core_artifact_identity_hash.as_str(),
                ),
                (
                    "extension_binding_identity",
                    self.extension_binding_identity_hash.as_str(),
                ),
            ];
            validate_manifest_fields(&manifest, &expected)
        }

        pub fn validate_prebuilt_dir(&self, dir: &Path) -> Result<(), String> {
            let mut candidates = vec![dir.join("manifest.txt")];
            if let Some(parent) = dir.parent() {
                candidates.push(parent.join("manifest.txt"));
            }
            for manifest in candidates {
                match std::fs::read(&manifest) {
                    Ok(bytes) => return self.validate_manifest_bytes(&bytes),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(format!("read {}: {error}", manifest.display()));
                    }
                }
            }
            Err(format!(
                "manifest.txt was not found beside extension prebuilt library directory {}",
                dir.display()
            ))
        }

        pub fn write_package_metadata(&self, out_dir: &Path) -> Result<(), String> {
            let archive_path = out_dir.join("prebuilt-archive-name.txt");
            std::fs::write(&archive_path, &self.archive_name)
                .map_err(|error| format!("write {}: {error}", archive_path.display()))?;
            let manifest_path = out_dir.join("prebuilt-manifest.txt");
            std::fs::write(&manifest_path, self.manifest_bytes())
                .map_err(|error| format!("write {}: {error}", manifest_path.display()))
        }
    }

    fn validate_extension_artifact_features(
        extension: ExtensionBinding,
        features: &[String],
    ) -> Result<(), String> {
        if !features.iter().any(|feature| feature == "wchar32") {
            return Err("extension artifact features must include wchar32".to_owned());
        }
        let allowed = match extension {
            ExtensionBinding::ImPlot3d => &["wchar32"][..],
            ExtensionBinding::NodeEditor => &["freetype", "stack-layout", "wchar32"][..],
            ExtensionBinding::ImPlot
            | ExtensionBinding::ImNodes
            | ExtensionBinding::ImGuizmo
            | ExtensionBinding::ImGuizmoQuat => &["freetype", "wchar32"][..],
        };
        let unsupported = features
            .iter()
            .filter(|feature| !allowed.contains(&feature.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if unsupported.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "unsupported {} artifact features: {}",
                extension.artifact_spec().sys_crate_name,
                unsupported.join(", ")
            ))
        }
    }

    fn extension_archive_name(
        extension: ExtensionBinding,
        version: &str,
        target: &str,
        link_type: &str,
        crt: &str,
        features: &[String],
    ) -> String {
        let suffix = ["stack-layout", "freetype"]
            .into_iter()
            .filter(|feature| features.iter().any(|actual| actual == feature))
            .collect::<Vec<_>>()
            .join("-");
        let suffix = (!suffix.is_empty()).then(|| format!("-{suffix}"));
        super::compose_archive_name(
            extension.artifact_spec().archive_stem,
            version,
            target,
            link_type,
            suffix.as_deref(),
            crt,
        )
    }

    fn validate_manifest_fields(
        manifest: &ParsedManifest,
        expected: &[(&str, &str)],
    ) -> Result<(), String> {
        for &(field, expected) in expected {
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
    let staging = unique_staging_path(destination);
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
        ArtifactProfile, BindingOwner, BindingSpec, BuildRequest, BuildRequestInput,
        CORE_BUILD_ENV_VARS, CORE_WASM_TARGET, CoreArtifactIdentity, CrateBindingInclude,
        CrateBindingIncludeRoot, CrateBindingLanguage, CrateBindingProvenance, CrateBindingSpec,
        CrateBindingTarget, ExtensionArtifactProfile, ExtensionArtifactProfileInput,
        ExtensionBinding, ExtensionBindingIdentity, HeaderShim, NativeAbiProfile, SourceRevisions,
        TargetFacts, bindgen_rerun_env_vars, is_supported_wasm_target,
        parse_crate_binding_source_revision, validate_bindgen_environment,
        validate_wasm_feature_contract,
    };
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let unique = format!(
                "dear-imgui-binding-test-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            );
            let path = std::env::temp_dir().join(unique);
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

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
            SourceRevisions::new(
                "1261b231939fc210032f30c4ee8a8f0440372237",
                "b61e56346a92cfcaf1f43a545ca37b0b32239654",
            ),
            BindingSpec::core_native(NativeAbiProfile::Windows64).deterministic_hash(),
        )
    }

    fn extension_core_profile() -> ArtifactProfile {
        ArtifactProfile::new(
            "dear-imgui",
            "0.16.0",
            "x86_64-pc-windows-msvc",
            "static",
            "md",
            [
                "platform-io-aggregate-hooks",
                "wchar32",
                "freetype",
                "stack-layout",
            ],
            SourceRevisions::new(
                "1261b231939fc210032f30c4ee8a8f0440372237",
                "b61e56346a92cfcaf1f43a545ca37b0b32239654",
            ),
            BindingSpec::core_native(NativeAbiProfile::Windows64).deterministic_hash(),
        )
    }

    fn extension_spec(extension: ExtensionBinding) -> &'static CrateBindingSpec {
        CrateBindingSpec::for_owner(BindingOwner::Extension(extension))
            .find(|spec| spec.target == CrateBindingTarget::Native)
            .unwrap()
    }

    fn extension_provenance(spec: &CrateBindingSpec) -> CrateBindingProvenance {
        CrateBindingProvenance {
            crate_name: spec.crate_name.to_owned(),
            target: spec.target.id().to_owned(),
            source_revision: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            spec_hash: spec.deterministic_hash(),
            input_hash: "fnv1a64:1111111111111111".to_owned(),
            output_hash: "fnv1a64:2222222222222222".to_owned(),
        }
    }

    fn extension_identity(extension: ExtensionBinding) -> ExtensionBindingIdentity {
        let spec = extension_spec(extension);
        ExtensionBindingIdentity::new(extension, spec, extension_provenance(spec)).unwrap()
    }

    fn write_extension_binding_fixture(directory: &Path, extension: ExtensionBinding) -> PathBuf {
        const REVISION: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let spec = extension_spec(extension);
        fs::write(
            directory.join("Cargo.toml"),
            format!("[package.metadata.dear-imgui-binding]\nsource-revision = \"{REVISION}\"\n"),
        )
        .unwrap();
        let inputs = spec
            .input_paths
            .iter()
            .map(|path| (*path, format!("fixture input for {path}\n")))
            .collect::<Vec<_>>();
        for (path, contents) in &inputs {
            let destination = directory.join(path);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::write(destination, contents).unwrap();
        }
        let body = spec
            .required_symbols
            .iter()
            .map(|symbol| format!("pub fn {symbol}();"))
            .collect::<Vec<_>>()
            .join("\n");
        let provenance = CrateBindingProvenance::new(
            spec,
            REVISION,
            inputs
                .iter()
                .map(|(path, contents)| (*path, contents.as_str())),
            &body,
        );
        let checked_in = directory.join(spec.checked_in_path);
        fs::create_dir_all(checked_in.parent().unwrap()).unwrap();
        fs::write(checked_in, provenance.embed(&body)).unwrap();
        directory.join(spec.input_paths[0])
    }

    fn extension_profile(
        extension: ExtensionBinding,
        features: &[&str],
    ) -> ExtensionArtifactProfile {
        let core = extension_core_profile();
        ExtensionArtifactProfile::new(
            &core,
            "cccccccccccccccccccccccccccccccccccccccc",
            features,
            extension_identity(extension),
        )
        .unwrap()
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
    fn core_binding_specs_pin_every_direct_standard_header() {
        let expected_headers = ["stdio.h", "stdint.h", "stdarg.h", "stdbool.h"];
        let specs = [
            BindingSpec::core_native(NativeAbiProfile::Windows64),
            BindingSpec::core_native(NativeAbiProfile::NonWindows),
            BindingSpec::core_wasm("imgui-sys-v0"),
        ];

        for spec in specs {
            assert_eq!(
                spec.header_shims
                    .iter()
                    .map(|shim| shim.name)
                    .collect::<Vec<_>>(),
                expected_headers
            );
            assert_eq!(
                spec.clang_args
                    .iter()
                    .filter(|arg| **arg == "-nostdinc")
                    .count(),
                1,
                "system include fallback must stay disabled"
            );
            assert!(
                spec.header_shims
                    .iter()
                    .find(|shim| shim.name == "stdint.h")
                    .unwrap()
                    .contents
                    .contains("__INTPTR_TYPE__")
            );
            assert!(
                spec.header_shims
                    .iter()
                    .find(|shim| shim.name == "stdarg.h")
                    .unwrap()
                    .contents
                    .contains("__builtin_va_list")
            );
        }
    }

    #[test]
    fn header_shim_contents_participate_in_binding_spec_identity() {
        const FIRST_SHIM: &[HeaderShim] = &[HeaderShim {
            name: "stdint.h",
            contents: "typedef __INT32_TYPE__ int32_t;",
        }];
        const SECOND_SHIM: &[HeaderShim] = &[HeaderShim {
            name: "stdint.h",
            contents: "typedef signed int int32_t;",
        }];

        let mut first = BindingSpec::core_native(NativeAbiProfile::NonWindows);
        first.header_shims = FIRST_SHIM;
        let mut second = first.clone();
        second.header_shims = SECOND_SHIM;

        assert_ne!(first.deterministic_hash(), second.deterministic_hash());
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
    fn core_artifact_profile_hash_has_a_cross_language_test_vector() {
        let profile = ArtifactProfile::new(
            "dear-imgui",
            "0.16.0",
            "x86_64-pc-windows-msvc",
            "static",
            "md",
            ["platform-io-aggregate-hooks", "wchar32"],
            SourceRevisions::new(
                "1261b231939fc210032f30c4ee8a8f0440372237",
                "b61e56346a92cfcaf1f43a545ca37b0b32239654",
            ),
            "fnv1a64:0123456789abcdef",
        );
        assert_eq!(profile.deterministic_hash(), "fnv1a64:1c6bc757a0743a80");
    }

    #[test]
    fn core_release_manifest_anchors_candidate_and_fails_closed() {
        const CANDIDATE: &str = "cccccccccccccccccccccccccccccccccccccccc";
        let expected = profile();
        let manifest = expected.release_manifest_bytes(CANDIDATE).unwrap();
        let identity = expected.validate_release_manifest_bytes(&manifest).unwrap();

        assert_eq!(
            identity,
            CoreArtifactIdentity::new(&expected, CANDIDATE).unwrap()
        );
        assert_eq!(identity.profile_hash, expected.deterministic_hash());
        assert_ne!(
            identity.deterministic_hash(),
            CoreArtifactIdentity::new(&expected, "dddddddddddddddddddddddddddddddddddddddd",)
                .unwrap()
                .deterministic_hash(),
        );

        let legacy = expected.manifest_bytes();
        let error = expected
            .validate_release_manifest_bytes(&legacy)
            .unwrap_err();
        assert!(error.contains("candidate_sha"), "unexpected error: {error}");

        let uppercase = String::from_utf8(manifest)
            .unwrap()
            .replace(CANDIDATE, "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC");
        let error = expected
            .validate_release_manifest_bytes(uppercase.as_bytes())
            .unwrap_err();
        assert!(error.contains("lowercase"), "unexpected error: {error}");
    }

    #[test]
    fn six_extension_artifact_specs_have_unique_exact_identities() {
        let extensions = [
            ExtensionBinding::ImPlot,
            ExtensionBinding::ImPlot3d,
            ExtensionBinding::ImNodes,
            ExtensionBinding::NodeEditor,
            ExtensionBinding::ImGuizmo,
            ExtensionBinding::ImGuizmoQuat,
        ];
        let specs = extensions.map(ExtensionBinding::artifact_spec);

        for field in [
            specs
                .iter()
                .map(|spec| spec.extension_id)
                .collect::<BTreeSet<_>>(),
            specs
                .iter()
                .map(|spec| spec.safe_crate_name)
                .collect::<BTreeSet<_>>(),
            specs
                .iter()
                .map(|spec| spec.sys_crate_name)
                .collect::<BTreeSet<_>>(),
            specs
                .iter()
                .map(|spec| spec.archive_stem)
                .collect::<BTreeSet<_>>(),
            specs
                .iter()
                .map(|spec| spec.library_name)
                .collect::<BTreeSet<_>>(),
        ] {
            assert_eq!(field.len(), extensions.len());
        }
        assert_eq!(
            ExtensionBinding::NodeEditor.artifact_spec().sys_crate_name,
            "dear-node-editor-sys"
        );
        assert_eq!(
            ExtensionBinding::ImGuizmoQuat.artifact_spec().library_name,
            "dear_imguizmo_quat"
        );
    }

    #[test]
    fn extension_artifact_manifest_is_canonical_and_fail_closed() {
        let expected = extension_profile(
            ExtensionBinding::NodeEditor,
            &["wchar32", "freetype", "stack-layout", "wchar32"],
        );
        assert_eq!(expected.features, ["freetype", "stack-layout", "wchar32"]);
        assert!(expected.cache_key().starts_with("fnv1a64-"));
        assert!(!expected.cache_key().contains(':'));
        let other_candidate = ExtensionArtifactProfile::new(
            &extension_core_profile(),
            "dddddddddddddddddddddddddddddddddddddddd",
            ["freetype", "stack-layout", "wchar32"],
            extension_identity(ExtensionBinding::NodeEditor),
        )
        .unwrap();
        assert_ne!(expected.cache_key(), other_candidate.cache_key());
        assert_eq!(
            expected.archive_name,
            "dear-node-editor-prebuilt-0.16.0-x86_64-pc-windows-msvc-static-stack-layout-freetype-md.tar.gz"
        );
        let manifest = expected.manifest_bytes();
        expected.validate_manifest_bytes(&manifest).unwrap();

        for (field, replacement) in [
            ("candidate_sha", "dddddddddddddddddddddddddddddddddddddddd"),
            ("target", "aarch64-pc-windows-msvc"),
            ("crt", "mt"),
            ("features", "freetype,wchar32"),
            ("extension", "imnodes"),
            ("safe_crate", "dear-imnodes"),
            ("library", "dear_imnodes"),
            ("archive", "foreign.tar.gz"),
            ("core_artifact_identity", "fnv1a64:1111111111111111"),
            ("extension_binding_identity", "fnv1a64:3333333333333333"),
        ] {
            let changed = String::from_utf8(manifest.clone())
                .unwrap()
                .lines()
                .map(|line| {
                    line.strip_prefix(&format!("{field}="))
                        .map_or_else(|| line.to_owned(), |_| format!("{field}={replacement}"))
                })
                .collect::<Vec<_>>()
                .join("\n");
            let error = expected
                .validate_manifest_bytes(changed.as_bytes())
                .unwrap_err();
            assert!(
                error.contains(field),
                "unexpected error for {field}: {error}"
            );
        }

        let foreign = String::from_utf8(manifest.clone()).unwrap().replacen(
            "dear-node-editor-sys prebuilt",
            "dear-imnodes-sys prebuilt",
            1,
        );
        assert!(
            expected
                .validate_manifest_bytes(foreign.as_bytes())
                .unwrap_err()
                .contains("crate_name")
        );

        let mut unknown = manifest.clone();
        unknown.extend_from_slice(b"test_engine_identity=forbidden\n");
        assert!(
            expected
                .validate_manifest_bytes(&unknown)
                .unwrap_err()
                .contains("unknown fields")
        );

        let mut duplicate = manifest.clone();
        duplicate.extend_from_slice(b"candidate_sha=cccccccccccccccccccccccccccccccccccccccc\n");
        assert!(
            expected
                .validate_manifest_bytes(&duplicate)
                .unwrap_err()
                .contains("repeats field candidate_sha")
        );

        let missing = String::from_utf8(manifest)
            .unwrap()
            .lines()
            .filter(|line| !line.starts_with("candidate_sha="))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            expected
                .validate_manifest_bytes(missing.as_bytes())
                .unwrap_err()
                .contains("candidate_sha")
        );
    }

    #[test]
    fn extension_artifact_profile_rejects_invalid_routes_and_test_engine() {
        let core = extension_core_profile();
        let identity = extension_identity(ExtensionBinding::ImPlot);
        let construct = |candidate: &str, features: &[&str], core: &ArtifactProfile| {
            ExtensionArtifactProfile::new(core, candidate, features, identity.clone())
        };

        assert!(
            construct("ambient-or-empty", &["wchar32"], &core)
                .unwrap_err()
                .contains("candidate")
        );
        let mut wasm_core = core.clone();
        wasm_core.target = "wasm32-unknown-unknown".to_owned();
        wasm_core.crt.clear();
        wasm_core.binding_spec_hash = BindingSpec::core_wasm("imgui-sys-v0").deterministic_hash();
        assert!(
            construct(
                "cccccccccccccccccccccccccccccccccccccccc",
                &["wchar32"],
                &wasm_core,
            )
            .unwrap_err()
            .contains("native-only")
        );
        assert!(
            construct(
                "cccccccccccccccccccccccccccccccccccccccc",
                &["wchar32", "stack-layout"],
                &core,
            )
            .unwrap_err()
            .contains("unsupported")
        );

        let mut test_engine_core = core.clone();
        test_engine_core.features.push("test-engine".to_owned());
        assert!(
            construct(
                "cccccccccccccccccccccccccccccccccccccccc",
                &["wchar32"],
                &test_engine_core,
            )
            .unwrap_err()
            .contains("Test Engine")
        );

        let test_engine_spec = CrateBindingSpec::for_owner(BindingOwner::TestEngine)
            .find(|spec| spec.target == CrateBindingTarget::Native)
            .unwrap();
        let error = ExtensionBindingIdentity::new(
            ExtensionBinding::ImPlot,
            test_engine_spec,
            extension_provenance(test_engine_spec),
        )
        .unwrap_err();
        assert!(error.contains("matching extension binding owner"));

        let wasm_spec =
            CrateBindingSpec::for_owner(BindingOwner::Extension(ExtensionBinding::ImPlot))
                .find(|spec| matches!(spec.target, CrateBindingTarget::WasmImport { .. }))
                .unwrap();
        let error = ExtensionBindingIdentity::new(
            ExtensionBinding::ImPlot,
            wasm_spec,
            extension_provenance(wasm_spec),
        )
        .unwrap_err();
        assert!(error.contains("native bindings"));
    }

    #[test]
    fn extension_package_profile_recomputes_binding_inputs() {
        const CANDIDATE: &str = "cccccccccccccccccccccccccccccccccccccccc";
        let directory = TestDirectory::new();
        let changed_input =
            write_extension_binding_fixture(directory.path(), ExtensionBinding::ImPlot);
        let core = extension_core_profile();
        let core_identity = CoreArtifactIdentity::new(&core, CANDIDATE).unwrap();
        let core_identity_hash = core_identity.deterministic_hash();
        let input = ExtensionArtifactProfileInput {
            extension: ExtensionBinding::ImPlot,
            crate_root: directory.path(),
            version: &core.version,
            candidate_sha: CANDIDATE,
            target: &core.target,
            link_type: &core.link_type,
            crt: &core.crt,
            features: &["wchar32"],
            core_candidate_sha: CANDIDATE,
            core_artifact_identity_hash: &core_identity_hash,
        };

        input.build_for_package().unwrap();
        input.build_for_consumer().unwrap();
        fs::write(changed_input, "changed upstream or shim input\n").unwrap();

        let error = input.build_for_package().unwrap_err();
        assert!(
            error.contains("provenance mismatch"),
            "unexpected error: {error}"
        );
        input.build_for_consumer().unwrap();
    }

    #[test]
    fn extension_binding_identity_covers_upstream_inputs_shims_and_output() {
        const CHANGED_SHIMS: &[HeaderShim] = &[HeaderShim {
            name: "stdint.h",
            contents: "typedef changed shim contract;",
        }];
        let extension = ExtensionBinding::ImPlot;
        let spec = extension_spec(extension);
        let baseline = extension_identity(extension).deterministic_hash();

        for mutate in [
            |provenance: &mut CrateBindingProvenance| {
                provenance.source_revision = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned();
            },
            |provenance: &mut CrateBindingProvenance| {
                provenance.input_hash = "fnv1a64:3333333333333333".to_owned();
            },
            |provenance: &mut CrateBindingProvenance| {
                provenance.output_hash = "fnv1a64:4444444444444444".to_owned();
            },
        ] {
            let mut provenance = extension_provenance(spec);
            mutate(&mut provenance);
            let changed = ExtensionBindingIdentity::new(extension, spec, provenance).unwrap();
            assert_ne!(baseline, changed.deterministic_hash());
        }

        let mut changed_spec = *spec;
        changed_spec.header_shims = CHANGED_SHIMS;
        let changed = ExtensionBindingIdentity::new(
            extension,
            &changed_spec,
            extension_provenance(&changed_spec),
        )
        .unwrap();
        assert_ne!(baseline, changed.deterministic_hash());
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

    #[test]
    fn maintained_binding_specs_cover_test_engine_and_six_extensions() {
        let specs = CrateBindingSpec::maintained();
        let owners = specs
            .iter()
            .map(|spec| spec.owner)
            .collect::<std::collections::BTreeSet<_>>();

        assert!(owners.contains(&BindingOwner::TestEngine));
        for extension in [
            ExtensionBinding::ImPlot,
            ExtensionBinding::ImPlot3d,
            ExtensionBinding::ImNodes,
            ExtensionBinding::NodeEditor,
            ExtensionBinding::ImGuizmo,
            ExtensionBinding::ImGuizmoQuat,
        ] {
            assert!(owners.contains(&BindingOwner::Extension(extension)));
        }
        assert_eq!(owners.len(), 7);
        assert_eq!(
            specs
                .iter()
                .filter(|spec| {
                    spec.owner == BindingOwner::Extension(ExtensionBinding::NodeEditor)
                })
                .count(),
            1,
            "node-editor is the only native-only maintained extension"
        );
        assert_eq!(specs.len(), 12);
    }

    #[test]
    fn maintained_crate_binding_specs_are_independent_of_host_headers() {
        for spec in CrateBindingSpec::maintained() {
            assert!(spec.clang_args.contains(&"-nostdinc"));
            let expected_target = match spec.target {
                CrateBindingTarget::Native => "--target=x86_64-pc-windows-msvc",
                CrateBindingTarget::WasmImport { .. } => "--target=wasm32-unknown-unknown",
            };
            assert!(spec.clang_args.contains(&expected_target));

            for required_header in [
                "stdio.h",
                "stddef.h",
                "stdint.h",
                "stdarg.h",
                "stdbool.h",
                "time.h",
            ] {
                assert!(
                    spec.header_shims
                        .iter()
                        .any(|shim| shim.name == required_header),
                    "{} {} is missing the {required_header} shim",
                    spec.crate_name,
                    spec.target.id()
                );
            }
        }
    }

    #[test]
    fn crate_binding_provenance_covers_source_inputs_spec_target_and_output() {
        let spec = CrateBindingSpec::for_owner(BindingOwner::Extension(ExtensionBinding::ImPlot))
            .find(|spec| spec.target.id() == "native")
            .unwrap();
        let base = CrateBindingProvenance::new(
            spec,
            "1".repeat(40),
            [("third-party/cimplot/cimplot.h", "header-a")],
            "pub fn ImPlot_GetPlotPos();\n",
        );

        assert_eq!(base, base.clone());
        assert_ne!(
            base,
            CrateBindingProvenance::new(
                spec,
                "2".repeat(40),
                [("third-party/cimplot/cimplot.h", "header-a")],
                "pub fn ImPlot_GetPlotPos();\n",
            )
        );
        assert_ne!(
            base,
            CrateBindingProvenance::new(
                spec,
                "1".repeat(40),
                [("third-party/cimplot/cimplot.h", "header-b")],
                "pub fn ImPlot_GetPlotPos();\n",
            )
        );
        assert_ne!(
            base,
            CrateBindingProvenance::new(
                spec,
                "1".repeat(40),
                [("third-party/cimplot/cimplot.h", "header-a")],
                "pub fn ImPlot_GetPlotSize();\n",
            )
        );
    }

    #[test]
    fn test_engine_binding_provenance_never_changes_core_artifact_identity() {
        let artifact = profile();
        let spec = CrateBindingSpec::for_owner(BindingOwner::TestEngine)
            .next()
            .unwrap();
        let first = CrateBindingProvenance::new(
            spec,
            "a".repeat(40),
            [("shim/cimgui_test_engine.h", "first")],
            "pub fn imgui_test_engine_create_context();\n",
        );
        let second = CrateBindingProvenance::new(
            spec,
            "b".repeat(40),
            [("shim/cimgui_test_engine.h", "second")],
            "pub fn imgui_test_engine_create_context();\n",
        );

        assert_ne!(first, second);
        assert_ne!(first.identity_hash(), second.identity_hash());
        assert_eq!(
            artifact.deterministic_hash(),
            profile().deterministic_hash()
        );
        assert_eq!(artifact.manifest_bytes(), profile().manifest_bytes());
        assert!(
            !String::from_utf8(artifact.manifest_bytes())
                .unwrap()
                .contains("test_engine")
        );
    }

    #[test]
    fn checked_in_binding_validation_rejects_missing_symbols_and_provenance_drift() {
        let spec = CrateBindingSpec::for_owner(BindingOwner::Extension(ExtensionBinding::ImPlot))
            .find(|spec| spec.target.id() == "wasm")
            .unwrap();
        let revision = "1".repeat(40);
        let inputs = [("third-party/cimplot/cimplot.h", "header")];
        let body = spec
            .required_symbols
            .iter()
            .map(|symbol| format!("pub fn {symbol}();\n"))
            .collect::<String>();
        let provenance = CrateBindingProvenance::new(spec, &revision, inputs, &body);
        let checked_in = provenance.embed(&body);

        spec.validate_checked_in(&revision, inputs, &checked_in)
            .unwrap();

        let missing = checked_in.replace("pub fn ImPlot_TagY_Str0();\n", "");
        assert!(
            spec.validate_checked_in(&revision, inputs, &missing)
                .is_err()
        );

        let drifted = checked_in.replace(
            "pub fn ImPlot_TagX_Str0();",
            "pub fn ImPlot_TagX_Str0(i: i32);",
        );
        assert!(
            spec.validate_checked_in(&revision, inputs, &drifted)
                .is_err()
        );
    }

    #[test]
    fn crate_binding_metadata_rejects_missing_and_duplicate_revisions() {
        let missing = "[package.metadata.dear-imgui-binding]\n";
        assert!(parse_crate_binding_source_revision(missing).is_err());

        let duplicate = format!(
            "[package.metadata.dear-imgui-binding]\nsource-revision = \"{}\"\nsource-revision = \"{}\"\n",
            "1".repeat(40),
            "2".repeat(40)
        );
        assert!(parse_crate_binding_source_revision(&duplicate).is_err());
    }

    #[test]
    fn embedded_validation_rejects_missing_duplicate_and_malformed_fields() {
        let spec = CrateBindingSpec::for_owner(BindingOwner::Extension(ExtensionBinding::ImPlot))
            .find(|spec| spec.target == CrateBindingTarget::Native)
            .unwrap();
        let revision = "1".repeat(40);
        let inputs = [("third-party/cimplot/cimplot.h", "header")];
        let body = spec
            .required_symbols
            .iter()
            .map(|symbol| format!("pub fn {symbol}();\n"))
            .collect::<String>();
        let provenance = CrateBindingProvenance::new(spec, &revision, inputs, &body);

        let missing = provenance
            .marker()
            .replace(&format!(" inputs={}", provenance.input_hash), "");
        assert!(
            spec.validate_embedded(&revision, &format!("{missing}\n{body}"))
                .is_err()
        );

        let duplicate = provenance.marker().replace(
            &format!(" crate={}", provenance.crate_name),
            &format!(
                " crate={} crate={}",
                provenance.crate_name, provenance.crate_name
            ),
        );
        assert!(
            spec.validate_embedded(&revision, &format!("{duplicate}\n{body}"))
                .is_err()
        );

        let malformed = provenance
            .marker()
            .replace(" spec=fnv1a64:", " spec=sha256:");
        assert!(
            spec.validate_embedded(&revision, &format!("{malformed}\n{body}"))
                .is_err()
        );
    }

    #[test]
    fn offline_validation_succeeds_without_upstream_binding_inputs() {
        let spec = CrateBindingSpec::for_owner(BindingOwner::Extension(ExtensionBinding::ImPlot))
            .find(|spec| spec.target == CrateBindingTarget::Native)
            .unwrap();
        let revision = "1".repeat(40);
        let inputs = [("third-party/cimplot/cimplot.h", "canonical header")];
        let body = spec
            .required_symbols
            .iter()
            .map(|symbol| format!("pub fn {symbol}();\n"))
            .collect::<String>();
        let checked_in = CrateBindingProvenance::new(spec, &revision, inputs, &body).embed(&body);
        let crate_dir = TestDirectory::new();
        fs::write(
            crate_dir.path().join("Cargo.toml"),
            format!("[package.metadata.dear-imgui-binding]\nsource-revision = \"{revision}\"\n"),
        )
        .unwrap();
        let checked_in_path = crate_dir.path().join(spec.checked_in_path);
        fs::create_dir_all(checked_in_path.parent().unwrap()).unwrap();
        fs::write(&checked_in_path, &checked_in).unwrap();

        assert_eq!(
            spec.load_and_validate_embedded(crate_dir.path()).unwrap(),
            checked_in
        );
        assert!(spec.load_and_validate_full(crate_dir.path()).is_err());

        let out_dir = crate_dir.path().join("out");
        fs::create_dir(&out_dir).unwrap();
        spec.copy_embedded_checked_in_to_out_dir(crate_dir.path(), &out_dir)
            .unwrap();
        assert_eq!(
            fs::read_to_string(out_dir.join("bindings.rs")).unwrap(),
            checked_in
        );
    }

    #[test]
    fn structured_profile_and_target_fields_define_spec_identity() {
        let spec = *CrateBindingSpec::for_owner(BindingOwner::Extension(ExtensionBinding::ImPlot))
            .find(|spec| spec.target == CrateBindingTarget::Native)
            .unwrap();
        let base = spec.deterministic_hash();
        let mut changed = spec;

        changed.profile.language = CrateBindingLanguage::C;
        assert_ne!(changed.deterministic_hash(), base);
        changed = spec;
        changed.profile.include_paths = &[CrateBindingInclude {
            root: CrateBindingIncludeRoot::Source,
            relative_path: "different",
        }];
        assert_ne!(changed.deterministic_hash(), base);
        changed = spec;
        changed.profile.clang_defines = &["DIFFERENT_DEFINE"];
        assert_ne!(changed.deterministic_hash(), base);
        changed = spec;
        changed.profile.allowlisted_functions = &["DifferentFunction.*"];
        assert_ne!(changed.deterministic_hash(), base);
        changed = spec;
        changed.profile.allowlisted_types = &["DifferentType.*"];
        assert_ne!(changed.deterministic_hash(), base);
        changed = spec;
        changed.profile.allowlisted_vars = &["DIFFERENT_VAR.*"];
        assert_ne!(changed.deterministic_hash(), base);
        changed = spec;
        changed.profile.blocklisted_types = &["DifferentBlockedType"];
        assert_ne!(changed.deterministic_hash(), base);
        changed = spec;
        changed.profile.derives.eq = false;
        assert_ne!(changed.deterministic_hash(), base);
        changed = spec;
        changed.profile.layout_tests = true;
        assert_ne!(changed.deterministic_hash(), base);
        changed = spec;
        changed.profile.allowlist_recursively = false;
        assert_ne!(changed.deterministic_hash(), base);

        let wasm = CrateBindingSpec::for_owner(BindingOwner::Extension(ExtensionBinding::ImPlot))
            .find(|candidate| candidate.target.id() == "wasm")
            .unwrap();
        assert_ne!(wasm.deterministic_hash(), base);

        let revision = "1".repeat(40);
        let inputs = [("third-party/cimplot/cimplot.h", "header")];
        let body = spec
            .required_symbols
            .iter()
            .map(|symbol| format!("pub fn {symbol}();\n"))
            .collect::<String>();
        let checked_in = CrateBindingProvenance::new(&spec, &revision, inputs, &body).embed(&body);
        assert!(changed.validate_embedded(&revision, &checked_in).is_err());
        assert!(wasm.validate_embedded(&revision, &checked_in).is_err());
    }

    #[test]
    fn provenance_changes_are_isolated_to_the_owning_crate_profiles() {
        fn identities(changed_crate: Option<&str>) -> Vec<(String, String, String)> {
            CrateBindingSpec::maintained()
                .iter()
                .map(|spec| {
                    let revision = if changed_crate == Some(spec.crate_name) {
                        "2".repeat(40)
                    } else {
                        "1".repeat(40)
                    };
                    let inputs = spec
                        .input_paths
                        .iter()
                        .map(|path| (*path, format!("content:{path}")))
                        .collect::<Vec<_>>();
                    let body = spec
                        .required_symbols
                        .iter()
                        .map(|symbol| format!("pub fn {symbol}();\n"))
                        .collect::<String>();
                    let provenance = CrateBindingProvenance::new(
                        spec,
                        revision,
                        inputs
                            .iter()
                            .map(|(path, content)| (*path, content.as_str())),
                        &body,
                    );
                    (
                        spec.crate_name.to_owned(),
                        spec.target.id().to_owned(),
                        provenance.identity_hash(),
                    )
                })
                .collect()
        }

        let before = identities(None);
        let after = identities(Some("dear-implot-sys"));
        let changed = before
            .iter()
            .zip(after.iter())
            .filter(|(left, right)| left.2 != right.2)
            .map(|(_, right)| (right.0.as_str(), right.1.as_str()))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            changed,
            BTreeSet::from([("dear-implot-sys", "native"), ("dear-implot-sys", "wasm")])
        );
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
