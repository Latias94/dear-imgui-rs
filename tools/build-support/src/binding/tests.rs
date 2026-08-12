use super::{
    ArtifactProfile, ArtifactProfileInput, BindingOwner, BindingSpec, BuildRequest,
    BuildRequestInput, CORE_BUILD_ENV_VARS, CORE_WASM_TARGET, CoreArtifactIdentity,
    CrateBindingDefine, CrateBindingInclude, CrateBindingIncludeRoot, CrateBindingLanguage,
    CrateBindingProvenance, CrateBindingSpec, CrateBindingTarget, ExtensionArtifactProfile,
    ExtensionArtifactProfileInput, ExtensionBinding, ExtensionBindingIdentity, HeaderShim,
    NativeAbiProfile, SourceRevisions, TargetFacts, bindgen_rerun_env_vars,
    core_source_contract_hash, is_supported_wasm_target, parse_crate_binding_source_revision,
    validate_bindgen_environment, validate_wasm_feature_contract,
};
use crate::SAFE_DEMO_FONT_BOUNDARY_ARTIFACT_FEATURE;
use crate::source_inventory::SourceInventory;
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
    request_with_abi_and_env("", values)
}

fn request_with_abi_and_env(target_abi: &str, values: Vec<(&str, Option<&str>)>) -> BuildRequest {
    BuildRequest::new(BuildRequestInput {
        target_triple: "x86_64-pc-windows-msvc",
        target_os: "windows",
        target_env: "msvc",
        target_abi,
        target_arch: "x86_64",
        target_endian: "little",
        target_pointer_width: "64",
        cargo_profile: "release",
        artifact_features: vec![
            "platform-io-aggregate-hooks-v3",
            SAFE_DEMO_FONT_BOUNDARY_ARTIFACT_FEATURE,
            "wchar32",
        ],
        environment: values,
    })
}

fn profile() -> ArtifactProfile {
    ArtifactProfile::new(ArtifactProfileInput {
        crate_name: "dear-imgui",
        version: "0.15.1",
        target: "x86_64-pc-windows-msvc",
        link_type: "static",
        crt: "md",
        features: [
            "platform-io-aggregate-hooks-v3",
            SAFE_DEMO_FONT_BOUNDARY_ARTIFACT_FEATURE,
            "wchar32",
        ],
        source_revisions: SourceRevisions::new(
            "1261b231939fc210032f30c4ee8a8f0440372237",
            "b61e56346a92cfcaf1f43a545ca37b0b32239654",
        ),
        binding_spec_hash: BindingSpec::core_native(NativeAbiProfile::Windows64)
            .deterministic_hash(),
        source_contract_hash: core_source_contract_hash(),
    })
}

fn extension_core_profile() -> ArtifactProfile {
    ArtifactProfile::new(ArtifactProfileInput {
        crate_name: "dear-imgui",
        version: "0.16.0",
        target: "x86_64-pc-windows-msvc",
        link_type: "static",
        crt: "md",
        features: [
            "platform-io-aggregate-hooks-v3",
            SAFE_DEMO_FONT_BOUNDARY_ARTIFACT_FEATURE,
            "wchar32",
            "freetype",
            "stack-layout",
        ],
        source_revisions: SourceRevisions::new(
            "1261b231939fc210032f30c4ee8a8f0440372237",
            "b61e56346a92cfcaf1f43a545ca37b0b32239654",
        ),
        binding_spec_hash: BindingSpec::core_native(NativeAbiProfile::Windows64)
            .deterministic_hash(),
        source_contract_hash: core_source_contract_hash(),
    })
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

fn extension_profile(extension: ExtensionBinding, features: &[&str]) -> ExtensionArtifactProfile {
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
    let wasm = BindingSpec::core_wasm("imgui-sys-v1");

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
    let non_windows = BindingSpec::core_native(NativeAbiProfile::NonWindows);
    let wasm = BindingSpec::core_wasm("imgui-sys-v1");

    assert_eq!(native.deterministic_hash(), native.deterministic_hash());
    assert_eq!(native.deterministic_hash(), "fnv1a64:b7fbb34dde098a66");
    assert_eq!(non_windows.deterministic_hash(), "fnv1a64:56a8814db35b4c70");
    assert_ne!(
        native.deterministic_hash(),
        non_windows.deterministic_hash()
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
        BindingSpec::core_wasm("imgui-sys-v1"),
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
                    target_abi: target.target_abi,
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
    assert_eq!(windows_gnu.target_abi, "");

    for (rust_target, expected_abi) in [
        ("aarch64-apple-ios-sim", "sim"),
        ("x86_64-apple-ios", "sim"),
        ("armv7-linux-androideabi", "eabi"),
        ("armv7-unknown-linux-gnueabihf", "eabihf"),
        ("armv7-unknown-linux-musleabihf", "eabihf"),
    ] {
        let target = NativeAbiProfile::NonWindows
            .compatibility_targets()
            .iter()
            .find(|target| target.rust_target == rust_target)
            .unwrap();
        assert_eq!(target.target_abi, expected_abi, "{rust_target}");
    }

    for facts in [
        TargetFacts {
            triple: "aarch64-pc-windows-gnu",
            os: "windows",
            env: "gnu",
            target_abi: "",
            arch: "aarch64",
            endian: "little",
            pointer_width: "64",
        },
        TargetFacts {
            triple: "armv5te-unknown-linux-gnueabi",
            os: "linux",
            env: "gnu",
            target_abi: "eabi",
            arch: "arm",
            endian: "little",
            pointer_width: "32",
        },
        TargetFacts {
            triple: "armv7-unknown-linux-gnu",
            os: "linux",
            env: "gnu",
            target_abi: "",
            arch: "arm",
            endian: "little",
            pointer_width: "32",
        },
        TargetFacts {
            triple: "aarch64-apple-ios-macabi",
            os: "ios",
            env: "macabi",
            target_abi: "macabi",
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
            target_abi: "",
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
            target_abi: "",
            arch: "x86_64",
            endian: "little",
            pointer_width: "64",
        })
        .is_err()
    );

    let error = NativeAbiProfile::for_target(TargetFacts {
        triple: "aarch64-apple-ios-sim",
        os: "ios",
        env: "sim",
        target_abi: "",
        arch: "aarch64",
        endian: "little",
        pointer_width: "64",
    })
    .unwrap_err();
    assert!(
        error.contains("expected os=ios, env=sim, abi=sim"),
        "{error}"
    );
    assert!(error.contains("got os=ios, env=sim, abi="), "{error}");
}

#[test]
fn windows_gnullvm_targets_use_the_verified_windows64_profile() {
    assert_eq!(
        NativeAbiProfile::Windows64
            .compatibility_targets()
            .iter()
            .map(|target| target.rust_target)
            .collect::<Vec<_>>(),
        [
            "x86_64-pc-windows-msvc",
            "aarch64-pc-windows-msvc",
            "x86_64-pc-windows-gnu",
            "x86_64-pc-windows-gnullvm",
            "aarch64-pc-windows-gnullvm",
        ],
    );

    for (rust_target, clang_target, arch) in [
        (
            "x86_64-pc-windows-gnullvm",
            "x86_64-pc-windows-gnu",
            "x86_64",
        ),
        (
            "aarch64-pc-windows-gnullvm",
            "aarch64-pc-windows-gnu",
            "aarch64",
        ),
    ] {
        let target = NativeAbiProfile::Windows64
            .compatibility_targets()
            .iter()
            .find(|target| target.rust_target == rust_target)
            .unwrap_or_else(|| panic!("missing exact compatibility target {rust_target}"));
        assert_eq!(target.clang_target, clang_target, "{rust_target}");
        assert_eq!(target.os, "windows", "{rust_target}");
        assert_eq!(target.env, "gnu", "{rust_target}");
        assert_eq!(target.target_abi, "llvm", "{rust_target}");
        assert_eq!(target.arch, arch, "{rust_target}");
        assert_eq!(target.endian, "little", "{rust_target}");
        assert_eq!(target.pointer_width, "64", "{rust_target}");

        assert_eq!(
            NativeAbiProfile::for_target(TargetFacts {
                triple: rust_target,
                os: "windows",
                env: "gnu",
                target_abi: "llvm",
                arch,
                endian: "little",
                pointer_width: "64",
            })
            .unwrap(),
            NativeAbiProfile::Windows64,
        );

        let error = NativeAbiProfile::for_target(TargetFacts {
            triple: rust_target,
            os: "windows",
            env: "gnu",
            target_abi: "",
            arch,
            endian: "little",
            pointer_width: "64",
        })
        .unwrap_err();
        assert!(error.contains("expected os=windows, env=gnu, abi=llvm"));
        assert!(error.contains("got os=windows, env=gnu, abi="));
    }
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
        BindingSpec::core_wasm("imgui-sys-v1").sanitize(source),
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
    assert!(CORE_BUILD_ENV_VARS.contains(&"CARGO_CFG_TARGET_ABI"));

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
        target_abi: "",
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
        target_abi: "",
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
fn target_abi_participates_in_build_request_identity() {
    let empty_abi = request_with_abi_and_env("", Vec::new());
    let llvm_abi = request_with_abi_and_env("llvm", Vec::new());

    assert_eq!(empty_abi.target_abi, "");
    assert_eq!(llvm_abi.target_abi, "llvm");
    assert_ne!(
        empty_abi.deterministic_hash(),
        llvm_abi.deterministic_hash()
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
    let mut changed_source_contract = expected.clone();
    changed_source_contract.source_contract_hash = "fnv1a64:0000000000000000".to_owned();
    assert_ne!(
        expected.deterministic_hash(),
        changed_source_contract.deterministic_hash(),
        "source transforms must participate in the artifact profile hash"
    );

    for (field, replacement) in [
        ("cimgui_revision", "wrong-cimgui"),
        ("imgui_revision", "wrong-imgui"),
        ("binding_spec_hash", "wrong-binding-hash"),
        ("source_contract_hash", "wrong-source-contract"),
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
    let profile = ArtifactProfile::new(ArtifactProfileInput {
        crate_name: "dear-imgui",
        version: "0.16.0",
        target: "x86_64-pc-windows-msvc",
        link_type: "static",
        crt: "md",
        features: [
            "platform-io-aggregate-hooks-v3",
            SAFE_DEMO_FONT_BOUNDARY_ARTIFACT_FEATURE,
            "wchar32",
        ],
        source_revisions: SourceRevisions::new(
            "1261b231939fc210032f30c4ee8a8f0440372237",
            "b61e56346a92cfcaf1f43a545ca37b0b32239654",
        ),
        binding_spec_hash: "fnv1a64:0123456789abcdef".to_owned(),
        source_contract_hash: "fnv1a64:fedcba9876543210".to_owned(),
    });
    assert_eq!(profile.deterministic_hash(), "fnv1a64:24ddf59332529d1d");
}

#[test]
fn core_source_contract_hash_covers_versioned_maintained_transforms() {
    assert_eq!(core_source_contract_hash(), "fnv1a64:f533ea188de3ac6c");
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

    let missing_source_contract =
        String::from_utf8(expected.release_manifest_bytes(CANDIDATE).unwrap())
            .unwrap()
            .lines()
            .filter(|line| !line.starts_with("source_contract_hash="))
            .collect::<Vec<_>>()
            .join("\n");
    let error = expected
        .validate_release_manifest_bytes(missing_source_contract.as_bytes())
        .unwrap_err();
    assert!(
        error.contains("source_contract_hash"),
        "unexpected error: {error}"
    );

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
    wasm_core.binding_spec_hash = BindingSpec::core_wasm("imgui-sys-v1").deterministic_hash();
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

    let wasm_spec = CrateBindingSpec::for_owner(BindingOwner::Extension(ExtensionBinding::ImPlot))
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
    let changed_input = write_extension_binding_fixture(directory.path(), ExtensionBinding::ImPlot);
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
            .filter(|spec| { spec.owner == BindingOwner::Extension(ExtensionBinding::NodeEditor) })
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
fn maintained_wasm_specs_use_the_inventory_import_module() {
    let expected = SourceInventory::embedded().wasm_import_module.as_str();
    let actual = CrateBindingSpec::maintained()
        .iter()
        .filter_map(|spec| spec.target.import_module())
        .collect::<BTreeSet<_>>();

    assert_eq!(actual, BTreeSet::from([expected]));
}

#[test]
fn maintained_crate_binding_specs_emit_one_ordered_define_profile() {
    for spec in CrateBindingSpec::maintained() {
        let expected = spec
            .profile
            .clang_defines
            .iter()
            .chain(spec.target.clang_defines())
            .map(|define| format!("-D{define}"))
            .collect::<Vec<_>>();
        let actual = spec
            .binding_defines()
            .map(CrateBindingDefine::clang_arg)
            .collect::<Vec<_>>();
        let native = spec
            .native_binding_defines()
            .map(CrateBindingDefine::clang_arg)
            .collect::<Vec<_>>();

        assert_eq!(actual, expected, "{} {}", spec.crate_name, spec.target.id());
        assert_eq!(
            native,
            expected
                .iter()
                .filter(|define| *define != "-DCIMGUI_DEFINE_ENUMS_AND_STRUCTS")
                .cloned()
                .collect::<Vec<_>>(),
            "{} {} native define projection",
            spec.crate_name,
            spec.target.id()
        );
        if spec.target == CrateBindingTarget::Native {
            assert!(
                actual
                    .iter()
                    .any(|arg| arg == "-DIMGUI_DISABLE_OBSOLETE_FUNCTIONS"),
                "{} native regeneration omitted the core layout define",
                spec.crate_name
            );
        }
    }
}

#[test]
fn disabled_boolean_defines_are_absent_from_binding_and_metadata_routes() {
    let mut spec = *CrateBindingSpec::for_owner(BindingOwner::Extension(ExtensionBinding::ImPlot))
        .find(|spec| spec.target == CrateBindingTarget::Native)
        .unwrap();
    spec.profile.clang_defines = &["ENABLED=1", "ZERO=0", "FALSE=false", "OFF=OFF"];

    assert_eq!(
        spec.binding_defines()
            .map(CrateBindingDefine::clang_arg)
            .collect::<Vec<_>>(),
        ["-DENABLED=1", "-DIMGUI_DISABLE_OBSOLETE_FUNCTIONS",]
    );

    for value in ["0", "false", "FALSE", "off", "OFF", "no", "NO"] {
        assert_eq!(CrateBindingDefine::from_metadata("DISABLED", value), None);
    }
    for (value, expected) in [
        ("", "-DENABLED"),
        ("1", "-DENABLED=1"),
        ("true", "-DENABLED=true"),
        ("custom", "-DENABLED=custom"),
    ] {
        assert_eq!(
            CrateBindingDefine::from_metadata("ENABLED", value)
                .unwrap()
                .clang_arg(),
            expected
        );
    }
}

#[test]
fn extension_define_resolution_is_backend_neutral_and_deterministic() {
    let spec = CrateBindingSpec::for_owner(BindingOwner::Extension(ExtensionBinding::ImPlot))
        .find(|spec| spec.target == CrateBindingTarget::Native)
        .unwrap();
    let mut expected = spec
        .native_binding_defines()
        .map(CrateBindingDefine::clang_arg)
        .collect::<Vec<_>>();
    expected.extend(["-DALPHA".to_owned(), "-DZETA=2".to_owned()]);

    let actual = spec
        .resolved_extension_binding_defines([
            ("IGNORED", "1"),
            ("DEP_DEAR_IMGUI_SYS_DEFINE_ZETA", "2"),
            ("DEP_DEAR_IMGUI_DEFINE_ALPHA", ""),
            ("DEP_DEAR_IMGUI_SYS_DEFINE_DISABLED", "false"),
            ("DEP_DEAR_IMGUI_DEFINE_IMGUI_USE_WCHAR32", "override"),
            ("DEP_DEAR_IMGUI_DEFINE_ZETA", "2"),
            ("DEP_DEAR_IMGUI_SYS_DEFINE_ZERO", "0"),
        ])
        .iter()
        .map(|define| define.clang_arg())
        .collect::<Vec<_>>();

    assert_eq!(actual, expected);
}

#[test]
fn maintained_extension_build_routes_do_not_hard_code_bindgen_defines() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for crate_name in [
        "dear-imgui-test-engine-sys",
        "dear-imguizmo-quat-sys",
        "dear-imguizmo-sys",
        "dear-imnodes-sys",
        "dear-implot-sys",
        "dear-implot3d-sys",
        "dear-node-editor-sys",
    ] {
        let path = workspace_root
            .join("extensions")
            .join(crate_name)
            .join("build.rs");
        let source = fs::read_to_string(&path).unwrap();
        assert!(
            source.contains("binding_defines()"),
            "{} does not consume the canonical binding define profile",
            path.display()
        );
        assert!(
            source.contains("apply_extension_binding_defines"),
            "{} does not derive native defines from the canonical profile",
            path.display()
        );
        assert!(
            !source.contains(".clang_arg(\"-D"),
            "{} still hard-codes a bindgen define",
            path.display()
        );
    }

    let implot_build = fs::read_to_string(
        workspace_root
            .join("extensions")
            .join("dear-implot-sys")
            .join("build.rs"),
    )
    .unwrap();
    assert!(implot_build.contains("resolved_extension_binding_defines(env::vars())"));
    assert!(!implot_build.contains("strip_prefix(\"DEP_DEAR_IMGUI"));

    let core_build = fs::read_to_string(workspace_root.join("dear-imgui-sys/build.rs")).unwrap();
    assert!(!core_build.contains("cargo:DEFINE_IMGUI_ENABLE_TEST_ENGINE={}"));
    assert!(!core_build.contains("cargo:DEFINE_IMGUITEST={}"));
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
