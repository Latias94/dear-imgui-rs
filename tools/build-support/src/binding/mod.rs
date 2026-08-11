use std::collections::BTreeSet;

pub const CORE_BUILD_ENV_VARS: &[&str] = &[
    "BUILD_SUPPORT_GH_OWNER",
    "BUILD_SUPPORT_GH_REPO",
    "BINDGEN_EXTRA_CLANG_ARGS",
    "CARGO_CFG_TARGET_ABI",
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
const CORE_NATIVE_WINDOWS64_CLANG_ARGS: &[&str] = &["--target=x86_64-pc-windows-msvc", "-nostdinc"];
const CORE_NATIVE_NON_WINDOWS_CLANG_ARGS: &[&str] =
    &["--target=x86_64-unknown-linux-gnu", "-nostdinc"];
const CORE_WASM_CLANG_ARGS: &[&str] = &["--target=wasm32-unknown-unknown", "-nostdinc"];
const CORE_NATIVE_DEFINES: &[&str] = &[
    "CIMGUI_DEFINE_ENUMS_AND_STRUCTS",
    "IMGUI_DISABLE_OBSOLETE_FUNCTIONS",
    "IMGUI_USE_WCHAR32",
];
const CORE_WASM_DEFINES: &[&str] = &[
    "CIMGUI_DEFINE_ENUMS_AND_STRUCTS",
    "IMGUI_DISABLE_FILE_FUNCTIONS",
    "IMGUI_DISABLE_OBSOLETE_FUNCTIONS",
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
    pub target_abi: &'a str,
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
    pub target_abi: &'static str,
    pub arch: &'static str,
    pub endian: &'static str,
    pub pointer_width: &'static str,
}

const fn native_target(
    rust_target: &'static str,
    clang_target: &'static str,
    os: &'static str,
    env: &'static str,
    target_abi: &'static str,
    arch: &'static str,
    pointer_width: &'static str,
) -> NativeAbiTarget {
    NativeAbiTarget {
        rust_target,
        clang_target,
        os,
        env,
        target_abi,
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
        "",
        "x86_64",
        "64",
    ),
    native_target(
        "aarch64-pc-windows-msvc",
        "aarch64-pc-windows-msvc",
        "windows",
        "msvc",
        "",
        "aarch64",
        "64",
    ),
    native_target(
        "x86_64-pc-windows-gnu",
        "x86_64-w64-windows-gnu",
        "windows",
        "gnu",
        "",
        "x86_64",
        "64",
    ),
    native_target(
        "x86_64-pc-windows-gnullvm",
        "x86_64-pc-windows-gnu",
        "windows",
        "gnu",
        "llvm",
        "x86_64",
        "64",
    ),
    native_target(
        "aarch64-pc-windows-gnullvm",
        "aarch64-pc-windows-gnu",
        "windows",
        "gnu",
        "llvm",
        "aarch64",
        "64",
    ),
];

const NON_WINDOWS_TARGETS: &[NativeAbiTarget] = &[
    native_target(
        "x86_64-unknown-linux-gnu",
        "x86_64-unknown-linux-gnu",
        "linux",
        "gnu",
        "",
        "x86_64",
        "64",
    ),
    native_target(
        "aarch64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "linux",
        "gnu",
        "",
        "aarch64",
        "64",
    ),
    native_target(
        "x86_64-unknown-linux-musl",
        "x86_64-unknown-linux-musl",
        "linux",
        "musl",
        "",
        "x86_64",
        "64",
    ),
    native_target(
        "aarch64-unknown-linux-musl",
        "aarch64-unknown-linux-musl",
        "linux",
        "musl",
        "",
        "aarch64",
        "64",
    ),
    native_target(
        "i686-unknown-linux-gnu",
        "i686-unknown-linux-gnu",
        "linux",
        "gnu",
        "",
        "x86",
        "32",
    ),
    native_target(
        "i686-unknown-linux-musl",
        "i686-unknown-linux-musl",
        "linux",
        "musl",
        "",
        "x86",
        "32",
    ),
    native_target(
        "x86_64-apple-darwin",
        "x86_64-apple-darwin",
        "macos",
        "",
        "",
        "x86_64",
        "64",
    ),
    native_target(
        "aarch64-apple-darwin",
        "arm64-apple-darwin",
        "macos",
        "",
        "",
        "aarch64",
        "64",
    ),
    native_target(
        "aarch64-apple-ios",
        "arm64-apple-ios",
        "ios",
        "",
        "",
        "aarch64",
        "64",
    ),
    native_target(
        "aarch64-apple-ios-sim",
        "arm64-apple-ios-simulator",
        "ios",
        "sim",
        "sim",
        "aarch64",
        "64",
    ),
    native_target(
        "x86_64-apple-ios",
        "x86_64-apple-ios",
        "ios",
        "sim",
        "sim",
        "x86_64",
        "64",
    ),
    native_target(
        "aarch64-linux-android",
        "aarch64-linux-android",
        "android",
        "",
        "",
        "aarch64",
        "64",
    ),
    native_target(
        "x86_64-linux-android",
        "x86_64-linux-android",
        "android",
        "",
        "",
        "x86_64",
        "64",
    ),
    native_target(
        "i686-linux-android",
        "i686-linux-android",
        "android",
        "",
        "",
        "x86",
        "32",
    ),
    native_target(
        "armv7-linux-androideabi",
        "armv7-linux-androideabi",
        "android",
        "",
        "eabi",
        "arm",
        "32",
    ),
    native_target(
        "armv7-unknown-linux-gnueabihf",
        "armv7-unknown-linux-gnueabihf",
        "linux",
        "gnu",
        "eabihf",
        "arm",
        "32",
    ),
    native_target(
        "armv7-unknown-linux-musleabihf",
        "armv7-unknown-linux-musleabihf",
        "linux",
        "musl",
        "eabihf",
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
                    target.target_abi,
                    target.arch,
                    target.endian,
                    target.pointer_width,
                ) == (
                    expected.os,
                    expected.env,
                    expected.target_abi,
                    expected.arch,
                    expected.endian,
                    expected.pointer_width,
                ) {
                    return Ok(profile);
                }
                return Err(format!(
                    "Dear ImGui target facts do not match Rust target {}: \
                     expected os={}, env={}, abi={}, arch={}, endian={}, pointer_width={}; \
                     got os={}, env={}, abi={}, arch={}, endian={}, pointer_width={}",
                    target.triple,
                    expected.os,
                    expected.env,
                    expected.target_abi,
                    expected.arch,
                    expected.endian,
                    expected.pointer_width,
                    target.os,
                    target.env,
                    target.target_abi,
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
        hash.field("schema", "core-binding-spec-v3");
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
                    hash.field("target_abi", target.target_abi);
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
                let signed_alias = self
                    .signed_enum_aliases
                    .iter()
                    .any(|alias| trimmed == format!("pub type {alias} = ::std::os::raw::c_uint;"));
                if trimmed.starts_with("pub type Im")
                    && (trimmed.ends_with("_ = ::std::os::raw::c_uint;") || signed_alias)
                {
                    output.push_str(
                        &line.replace(" = ::std::os::raw::c_uint;", " = ::std::os::raw::c_int;"),
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
        let generator_banner = format!("automatically generated by {}", self.generator_contract);
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

mod artifact;
mod hash;
mod spec;
mod toolchain;
mod validation;

pub use artifact::*;
use hash::StableHash;
pub use spec::*;
pub use toolchain::*;

#[cfg(test)]
mod tests;
