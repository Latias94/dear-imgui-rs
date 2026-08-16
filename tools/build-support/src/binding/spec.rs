use super::hash::StableHash;
use super::validation::{validate_git_revision, validate_stable_hash};
use super::{BindingFormatter, BindingRustEdition, DerivePolicy, HeaderShim};
use crate::source_inventory::SourceInventory;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::LazyLock;

pub const CRATE_BINDING_METADATA_SECTION: &str = "package.metadata.dear-imgui-binding";
pub const CRATE_BINDING_PROVENANCE_PREFIX: &str = "// dear-imgui-rs-binding-provenance-v1";
pub const RELEASE_CANDIDATE_SHA_ENV: &str = "DEAR_IMGUI_RS_CANDIDATE_SHA";
pub const RELEASE_CORE_ARTIFACT_IDENTITY_HASH_ENV: &str = "DEAR_IMGUI_CORE_ARTIFACT_IDENTITY_HASH";
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
    Cte,
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
            Self::Cte => ExtensionArtifactSpec {
                extension_id: "cte",
                safe_crate_name: "dear-imgui-cte",
                sys_crate_name: "dear-imgui-cte-sys",
                archive_stem: "dear-imgui-cte",
                library_name: "dear_imgui_cte",
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
            Self::Native => CRATE_NATIVE_DEFINES,
            Self::WasmImport { .. } => CRATE_WASM_DEFINES,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CrateBindingLanguage {
    C,
    Cxx17,
    Cxx20,
}

impl CrateBindingLanguage {
    pub const fn id(self) -> &'static str {
        match self {
            Self::C => "c",
            Self::Cxx17 => "c++17",
            Self::Cxx20 => "c++20",
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

/// One preprocessor definition emitted by a canonical crate binding profile.
///
/// A definition without an explicit value is represented as a flag.  Values
/// that spell a disabled boolean are discarded before they cross a build
/// script boundary, because `-DNAME=0` still satisfies `#ifdef NAME`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrateBindingDefine<'a> {
    pub name: &'a str,
    pub value: Option<&'a str>,
}

impl<'a> CrateBindingDefine<'a> {
    pub fn from_definition(definition: &'a str) -> Option<Self> {
        let (name, value) = definition.split_once('=').unwrap_or((definition, ""));
        Self::from_metadata(name, value)
    }

    pub fn from_metadata(name: &'a str, value: &'a str) -> Option<Self> {
        let value = value.trim();
        if ["0", "false", "off", "no"]
            .iter()
            .any(|disabled| value.eq_ignore_ascii_case(disabled))
        {
            return None;
        }
        Some(Self {
            name,
            value: (!value.is_empty()).then_some(value),
        })
    }

    pub fn clang_arg(self) -> String {
        match self.value {
            Some(value) => format!("-D{}={value}", self.name),
            None => format!("-D{}", self.name),
        }
    }

    pub fn applies_to_native_compilation(self) -> bool {
        // Wrapper C++ includes imgui.h first; asking cimgui.h to redeclare
        // those enums is valid for bindgen's C view but not for compilation.
        self.name != "CIMGUI_DEFINE_ENUMS_AND_STRUCTS"
    }
}

/// An owned preprocessor definition resolved for an extension build.
///
/// Owned definitions let build-script backends consume the same canonical
/// and propagated define stream without borrowing environment storage.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct OwnedCrateBindingDefine {
    pub name: String,
    pub value: Option<String>,
}

impl OwnedCrateBindingDefine {
    pub fn clang_arg(&self) -> String {
        match self.value.as_deref() {
            Some(value) => format!("-D{}={value}", self.name),
            None => format!("-D{}", self.name),
        }
    }
}

impl<'a> From<CrateBindingDefine<'a>> for OwnedCrateBindingDefine {
    fn from(define: CrateBindingDefine<'a>) -> Self {
        Self {
            name: define.name.to_owned(),
            value: define.value.map(str::to_owned),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrateBindingSpec {
    pub owner: BindingOwner,
    pub crate_name: &'static str,
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
        &MAINTAINED_CRATE_BINDING_SPECS[..]
    }

    /// Return the single ordered target/profile define stream consumed by
    /// bindgen and native extension compilation.
    pub fn binding_defines(&self) -> impl Iterator<Item = CrateBindingDefine<'static>> + '_ {
        self.profile
            .clang_defines
            .iter()
            .chain(self.target.clang_defines())
            .filter_map(|definition| CrateBindingDefine::from_definition(definition))
    }

    pub fn native_binding_defines(&self) -> impl Iterator<Item = CrateBindingDefine<'static>> + '_ {
        self.binding_defines()
            .filter(|define| define.applies_to_native_compilation())
    }

    pub fn apply_native_binding_defines(&self, build: &mut cc::Build) {
        for define in self.native_binding_defines() {
            build.define(define.name, define.value);
        }
    }

    pub fn apply_extension_binding_defines<I, K, V>(&self, build: &mut cc::Build, environment: I)
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        for define in self.resolved_extension_binding_defines(environment) {
            build.define(&define.name, define.value.as_deref());
        }
    }

    /// Resolve the native canonical defines and propagated core defines for
    /// an extension build.
    ///
    /// Canonical definitions retain profile order. Propagated definitions
    /// are filtered for disabled boolean values, excluded when canonical,
    /// then sorted and deduplicated to preserve the historical build
    /// contract across compiler backends.
    pub fn resolved_extension_binding_defines<I, K, V>(
        &self,
        environment: I,
    ) -> Vec<OwnedCrateBindingDefine>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let canonical = self
            .native_binding_defines()
            .map(|define| define.name)
            .collect::<BTreeSet<_>>();
        let mut resolved = self
            .native_binding_defines()
            .map(OwnedCrateBindingDefine::from)
            .collect::<Vec<_>>();
        let mut propagated = environment
            .into_iter()
            .filter_map(|(key, value)| {
                let name = key
                    .as_ref()
                    .strip_prefix("DEP_DEAR_IMGUI_SYS_DEFINE_")
                    .or_else(|| key.as_ref().strip_prefix("DEP_DEAR_IMGUI_DEFINE_"))?;
                CrateBindingDefine::from_metadata(name, value.as_ref()).and_then(|define| {
                    (!canonical.contains(define.name))
                        .then(|| OwnedCrateBindingDefine::from(define))
                })
            })
            .collect::<Vec<_>>();
        propagated.sort_unstable();
        propagated.dedup();
        resolved.extend(propagated);
        resolved
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

    pub fn maintained_source(&self) -> &'static crate::source_inventory::MaintainedSource {
        crate::source_inventory::SourceInventory::embedded()
            .source_by_crate(self.crate_name)
            .unwrap_or_else(|| {
                panic!(
                    "binding spec crate {:?} is absent from the maintained-source inventory",
                    self.crate_name
                )
            })
    }

    pub fn crate_root(&self) -> &'static str {
        &self.maintained_source().crate_root
    }

    pub fn source_root(&self) -> &'static str {
        &self.maintained_source().source_root
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
        hash.field("crate_root", self.crate_root());
        hash.field("source_root", self.source_root());
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
        self.validate_checked_in_with_source_revisions(
            &CrateBindingSourceRevisions::single(source_revision),
            inputs,
            checked_in,
        )
    }

    fn validate_checked_in_with_source_revisions<I, P, C>(
        &self,
        source_revisions: &CrateBindingSourceRevisions,
        inputs: I,
        checked_in: &str,
    ) -> Result<(), String>
    where
        I: IntoIterator<Item = (P, C)>,
        P: AsRef<str>,
        C: AsRef<str>,
    {
        let actual = self.validate_embedded_with_source_revisions(source_revisions, checked_in)?;
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
        let expected = CrateBindingProvenance::new_with_source_revisions(
            self,
            source_revisions.clone(),
            inputs,
            body,
        );
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
        self.validate_embedded_with_source_revisions(
            &CrateBindingSourceRevisions::single(source_revision),
            checked_in,
        )
    }

    fn validate_embedded_with_source_revisions(
        &self,
        source_revisions: &CrateBindingSourceRevisions,
        checked_in: &str,
    ) -> Result<CrateBindingProvenance, String> {
        source_revisions.validate()?;
        let (marker, body) = split_binding_provenance(checked_in)?;
        validate_required_symbols(self, body)?;
        let actual = CrateBindingProvenance::parse(marker)?;
        if marker != actual.marker() {
            return Err("binding provenance marker is not in canonical form".to_owned());
        }
        let expected = CrateBindingProvenance {
            crate_name: self.crate_name.to_owned(),
            target: self.target.id().to_owned(),
            source_revision: source_revisions.source_revision.clone(),
            nested_source_revision: source_revisions.nested_source_revision.clone(),
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
        let source_revisions = parse_crate_binding_source_revisions(&manifest)?;
        let checked_in_path = crate_root.join(self.checked_in_path);
        let checked_in = std::fs::read_to_string(&checked_in_path)
            .map_err(|error| format!("read {}: {error}", checked_in_path.display()))?;
        self.validate_embedded_with_source_revisions(&source_revisions, &checked_in)?;
        Ok(checked_in)
    }

    pub fn load_and_validate_provenance(
        &self,
        crate_root: &Path,
    ) -> Result<CrateBindingProvenance, String> {
        let manifest_path = crate_root.join("Cargo.toml");
        let manifest = std::fs::read_to_string(&manifest_path)
            .map_err(|error| format!("read {}: {error}", manifest_path.display()))?;
        let source_revisions = parse_crate_binding_source_revisions(&manifest)?;
        let checked_in_path = crate_root.join(self.checked_in_path);
        let checked_in = std::fs::read_to_string(&checked_in_path)
            .map_err(|error| format!("read {}: {error}", checked_in_path.display()))?;
        self.validate_embedded_with_source_revisions(&source_revisions, &checked_in)
    }

    pub fn load_and_validate_full(&self, crate_root: &Path) -> Result<String, String> {
        let manifest_path = crate_root.join("Cargo.toml");
        let manifest = std::fs::read_to_string(&manifest_path)
            .map_err(|error| format!("read {}: {error}", manifest_path.display()))?;
        let source_revisions = parse_crate_binding_source_revisions(&manifest)?;
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
        self.validate_checked_in_with_source_revisions(
            &source_revisions,
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
        let source_revisions = parse_crate_binding_source_revisions(&manifest)?;
        let mut inputs = Vec::with_capacity(self.input_paths.len());
        for relative_path in self.input_paths {
            let path = crate_root.join(relative_path);
            let content = std::fs::read_to_string(&path)
                .map_err(|error| format!("read {}: {error}", path.display()))?;
            inputs.push((*relative_path, content));
        }
        let (_, body) = split_optional_binding_provenance(generated)?;
        validate_required_symbols(self, body)?;
        Ok(CrateBindingProvenance::new_with_source_revisions(
            self,
            source_revisions,
            inputs
                .iter()
                .map(|(path, content)| (*path, content.as_str())),
            body,
        )
        .embed(body))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrateBindingSourceRevisions {
    pub source_revision: String,
    pub nested_source_revision: Option<String>,
}

impl CrateBindingSourceRevisions {
    pub fn new<S, N>(source_revision: S, nested_source_revision: Option<N>) -> Self
    where
        S: Into<String>,
        N: Into<String>,
    {
        Self {
            source_revision: source_revision.into(),
            nested_source_revision: nested_source_revision.map(Into::into),
        }
    }

    pub fn single(source_revision: impl Into<String>) -> Self {
        Self {
            source_revision: source_revision.into(),
            nested_source_revision: None,
        }
    }

    fn validate(&self) -> Result<(), String> {
        validate_git_revision("source-revision", &self.source_revision)?;
        if let Some(revision) = &self.nested_source_revision {
            validate_git_revision("nested-source-revision", revision)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrateBindingProvenance {
    pub crate_name: String,
    pub target: String,
    pub source_revision: String,
    pub nested_source_revision: Option<String>,
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
        Self::new_with_source_revisions(
            spec,
            CrateBindingSourceRevisions::single(source_revision),
            inputs,
            binding_body,
        )
    }

    pub fn new_with_source_revisions<I, P, C>(
        spec: &CrateBindingSpec,
        source_revisions: CrateBindingSourceRevisions,
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
            source_revision: source_revisions.source_revision,
            nested_source_revision: source_revisions.nested_source_revision,
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
                "crate" | "target" | "source" | "nested" | "spec" | "inputs" | "output"
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
        if let Some(revision) = values.get("nested") {
            validate_git_revision("binding provenance nested source", revision)?;
        }
        for hash in ["spec", "inputs", "output"] {
            validate_stable_hash(hash, values[hash])?;
        }
        Ok(Self {
            crate_name: values["crate"].to_owned(),
            target: values["target"].to_owned(),
            source_revision: values["source"].to_owned(),
            nested_source_revision: values.get("nested").map(|revision| (*revision).to_owned()),
            spec_hash: values["spec"].to_owned(),
            input_hash: values["inputs"].to_owned(),
            output_hash: values["output"].to_owned(),
        })
    }

    pub fn identity_hash(&self) -> String {
        let mut hash = StableHash::new();
        hash.field(
            "schema",
            if self.nested_source_revision.is_some() {
                "crate-binding-identity-v2"
            } else {
                "crate-binding-identity-v1"
            },
        );
        hash.field("crate_name", &self.crate_name);
        hash.field("target", &self.target);
        hash.field("source_revision", &self.source_revision);
        if let Some(revision) = &self.nested_source_revision {
            hash.field("nested_source_revision", revision);
        }
        hash.field("spec_hash", &self.spec_hash);
        hash.field("input_hash", &self.input_hash);
        hash.field("output_hash", &self.output_hash);
        hash.finish()
    }

    pub fn marker(&self) -> String {
        let nested = self
            .nested_source_revision
            .as_ref()
            .map(|revision| format!(" nested={revision}"))
            .unwrap_or_default();
        format!(
            "{CRATE_BINDING_PROVENANCE_PREFIX} crate={} target={} source={}{} spec={} inputs={} output={}",
            self.crate_name,
            self.target,
            self.source_revision,
            nested,
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
        if let Some(revision) = &provenance.nested_source_revision {
            validate_git_revision("extension binding nested source", revision)?;
        }
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

pub fn parse_crate_binding_source_revision(content: &str) -> Result<String, String> {
    Ok(parse_crate_binding_source_revisions(content)?.source_revision)
}

pub fn parse_crate_binding_source_revisions(
    content: &str,
) -> Result<CrateBindingSourceRevisions, String> {
    let mut current_section = "";
    let mut revision = None;
    let mut nested_revision = None;
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
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .ok_or_else(|| {
                format!("{key} in [{CRATE_BINDING_METADATA_SECTION}] must be a quoted string")
            })?;
        let destination = match key {
            "source-revision" => &mut revision,
            "nested-source-revision" => &mut nested_revision,
            _ => {
                return Err(format!(
                    "unknown key {key} in [{CRATE_BINDING_METADATA_SECTION}]"
                ));
            }
        };
        if destination.replace(value.to_owned()).is_some() {
            return Err(format!(
                "duplicate {key} in [{CRATE_BINDING_METADATA_SECTION}]"
            ));
        }
    }
    let revision = revision
        .ok_or_else(|| format!("missing source-revision in [{CRATE_BINDING_METADATA_SECTION}]"))?;
    let revisions = CrateBindingSourceRevisions {
        source_revision: revision,
        nested_source_revision: nested_revision,
    };
    revisions.validate()?;
    Ok(revisions)
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
const CRATE_NATIVE_DEFINES: &[&str] = &["IMGUI_DISABLE_OBSOLETE_FUNCTIONS"];
const CRATE_WASM_DEFINES: &[&str] = &[
    "IMGUI_DISABLE_FILE_FUNCTIONS",
    "IMGUI_DISABLE_OBSOLETE_FUNCTIONS",
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

const CTE_INPUTS: &[&str] = &["third-party/cimCTE/cimCTE.h", "shim/cte_bridge.h"];
const CTE_INCLUDES: &[CrateBindingInclude] = &[
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
        relative_path: "ImGuiColorTextEdit",
    },
];
const CTE_DEFINES: &[&str] = &["IMGUI_USE_WCHAR32", "CIMGUI_DEFINE_ENUMS_AND_STRUCTS"];
const CTE_FUNCTIONS: &[&str] = &[
    "TextEditor_.*",
    "TextDiff_.*",
    "DocPos_.*",
    "DocSelection_.*",
    "VisPos_.*",
    "Glyph_.*",
    "Iterator_.*",
    "Language_.*",
    "Notifications_.*",
    "Palette_.*",
    "TrieAutoComplete_.*",
    "CodePoint_.*",
    "GetDejavu",
    "SetDejavu",
    "dear_imgui_cte_.*",
];
const CTE_TYPES: &[&str] = &[
    "(TextEditor|TextDiff|DocPos.*|DocSelection.*|VisPos.*|Glyph|Iterator|Language|Notifications|Palette|TrieAutoComplete|CodePoint|Change|Decorator|CustomCaret|PopupData|AutoComplete.*|LineBreakConfig|Color|BreakOption|Scroll|Type)",
    "DearImGuiCte.*",
];
const CTE_BLOCKLIST_TYPES: &[&str] = &[
    "ImDrawList",
    "ImGuiContext",
    "ImGuiChildFlags",
    "ImGuiWindowFlags",
    "ImGuiKeyChord",
    "ImTextureID",
    "ImU32",
    "ImWchar",
    "ImVec2",
    "ImVec2_c",
    "ImVec4",
    "ImVec4_c",
];
const CTE_PROFILE: CrateBindgenProfile = crate_profile(
    CrateBindingLanguage::Cxx20,
    CTE_INCLUDES,
    CTE_DEFINES,
    CrateBindingSymbols {
        functions: CTE_FUNCTIONS,
        types: CTE_TYPES,
        vars: &[],
        blocked_types: CTE_BLOCKLIST_TYPES,
    },
    DEFAULT_CRATE_DERIVES,
    false,
);
const CTE_SYMBOLS: &[&str] = &[
    "TextEditor_TextEditor",
    "TextEditor_destroy",
    "TextEditor_Render",
    "TextEditor_GetText_alloc",
    "TextEditor_GetText_free",
    "TextDiff_TextDiff",
    "TextDiff_destroy",
    "TextDiff_Render",
    "DocPos_DocPos_Nil",
    "DocSelection_DocSelection_Nil",
    "VisPos_VisPos_Nil",
    "Glyph_Glyph_Nil",
    "Iterator_Iterator_Nil",
    "TrieAutoComplete_TrieAutoComplete",
    "TrieAutoComplete_destroy",
    "Notifications_Notifications",
    "Notifications_destroy",
    "Palette_Palette",
    "Palette_destroy",
    "Language_Cpp",
    "CodePoint_write",
    "GetDejavu",
    "SetDejavu",
    "dear_imgui_cte_set_change_callback",
    "dear_imgui_cte_set_transaction_callback",
    "dear_imgui_cte_set_insert_callback",
    "dear_imgui_cte_set_delete_callback",
    "dear_imgui_cte_iterate_line_data",
    "dear_imgui_cte_set_line_decorator",
    "dear_imgui_cte_set_custom_caret_callback",
    "dear_imgui_cte_set_line_number_context_callback",
    "dear_imgui_cte_set_text_context_callback",
    "dear_imgui_cte_set_text_hover_callback",
    "dear_imgui_cte_set_language_change_callback",
    "dear_imgui_cte_iterate_identifiers",
    "dear_imgui_cte_filter_selections",
    "dear_imgui_cte_filter_lines",
    "dear_imgui_cte_clear_callbacks",
    "dear_imgui_cte_autocomplete_config_create",
    "dear_imgui_cte_autocomplete_config_destroy",
    "dear_imgui_cte_autocomplete_config_set_callback",
    "dear_imgui_cte_text_editor_set_autocomplete_config",
    "dear_imgui_cte_text_editor_set_autocomplete_suggestions",
    "dear_imgui_cte_autocomplete_state_get_search_term",
    "dear_imgui_cte_autocomplete_state_get_range",
    "dear_imgui_cte_autocomplete_state_get_context",
    "dear_imgui_cte_autocomplete_state_clear_suggestions",
    "dear_imgui_cte_autocomplete_state_add_suggestion",
    "dear_imgui_cte_autocomplete_state_set_promise",
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
const IMGUIZMO_QUAT_DEFINES: &[&str] = &["IMGUI_USE_WCHAR32", "CIMGUI_DEFINE_ENUMS_AND_STRUCTS"];
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
    input_paths: &'static [&'static str],
    profile: CrateBindgenProfile,
    required_symbols: &'static [&'static str],
}

const fn source_spec(
    owner: BindingOwner,
    crate_name: &'static str,
    input_paths: &'static [&'static str],
    profile: CrateBindgenProfile,
    required_symbols: &'static [&'static str],
) -> CrateBindingSourceSpec {
    CrateBindingSourceSpec {
        owner,
        crate_name,
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
    TEST_ENGINE_INPUTS,
    TEST_ENGINE_PROFILE,
    TEST_ENGINE_SYMBOLS,
);
const IMPLOT_SOURCE: CrateBindingSourceSpec = source_spec(
    BindingOwner::Extension(ExtensionBinding::ImPlot),
    "dear-implot-sys",
    IMPLOT_INPUTS,
    IMPLOT_PROFILE,
    IMPLOT_SYMBOLS,
);
const IMPLOT3D_SOURCE: CrateBindingSourceSpec = source_spec(
    BindingOwner::Extension(ExtensionBinding::ImPlot3d),
    "dear-implot3d-sys",
    IMPLOT3D_INPUTS,
    IMPLOT3D_PROFILE,
    IMPLOT3D_SYMBOLS,
);
const IMNODES_SOURCE: CrateBindingSourceSpec = source_spec(
    BindingOwner::Extension(ExtensionBinding::ImNodes),
    "dear-imnodes-sys",
    IMNODES_INPUTS,
    IMNODES_PROFILE,
    IMNODES_SYMBOLS,
);
const CTE_SOURCE: CrateBindingSourceSpec = source_spec(
    BindingOwner::Extension(ExtensionBinding::Cte),
    "dear-imgui-cte-sys",
    CTE_INPUTS,
    CTE_PROFILE,
    CTE_SYMBOLS,
);
const NODE_EDITOR_SOURCE: CrateBindingSourceSpec = source_spec(
    BindingOwner::Extension(ExtensionBinding::NodeEditor),
    "dear-node-editor-sys",
    NODE_EDITOR_INPUTS,
    NODE_EDITOR_PROFILE,
    NODE_EDITOR_SYMBOLS,
);
const IMGUIZMO_SOURCE: CrateBindingSourceSpec = source_spec(
    BindingOwner::Extension(ExtensionBinding::ImGuizmo),
    "dear-imguizmo-sys",
    IMGUIZMO_INPUTS,
    IMGUIZMO_PROFILE,
    IMGUIZMO_SYMBOLS,
);
const IMGUIZMO_QUAT_SOURCE: CrateBindingSourceSpec = source_spec(
    BindingOwner::Extension(ExtensionBinding::ImGuizmoQuat),
    "dear-imguizmo-quat-sys",
    IMGUIZMO_QUAT_INPUTS,
    IMGUIZMO_QUAT_PROFILE,
    IMGUIZMO_QUAT_SYMBOLS,
);

static MAINTAINED_CRATE_BINDING_SPECS: LazyLock<[CrateBindingSpec; 14]> = LazyLock::new(|| {
    let wasm_import_module = SourceInventory::embedded().wasm_import_module.as_str();
    [
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
                module_name: wasm_import_module,
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
                module_name: wasm_import_module,
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
                module_name: wasm_import_module,
            },
        ),
        spec(
            CTE_SOURCE,
            "src/bindings_pregenerated.rs",
            CrateBindingTarget::Native,
        ),
        spec(
            CTE_SOURCE,
            "src/wasm_bindings_pregenerated.rs",
            CrateBindingTarget::WasmImport {
                module_name: wasm_import_module,
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
                module_name: wasm_import_module,
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
                module_name: wasm_import_module,
            },
        ),
    ]
});
