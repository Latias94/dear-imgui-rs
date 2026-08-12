use super::hash::StableHash;
use super::spec::BindingOwner;
use super::validation::{validate_git_revision, validate_stable_hash};
use super::{
    CrateBindingSpec, CrateBindingTarget, DEP_CORE_ARTIFACT_IDENTITY_HASH_ENV,
    DEP_CORE_CANDIDATE_SHA_ENV, ExtensionBinding, ExtensionBindingIdentity,
    RELEASE_CANDIDATE_SHA_ENV, RELEASE_CORE_ARTIFACT_IDENTITY_HASH_ENV,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildRequestInput<'a> {
    pub target_triple: &'a str,
    pub target_os: &'a str,
    pub target_env: &'a str,
    pub target_abi: &'a str,
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
    pub target_abi: String,
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
            target_abi: input.target_abi.to_owned(),
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
        hash.field("schema", "core-build-request-v3");
        hash.field("target_triple", &self.target_triple);
        hash.field("target_os", &self.target_os);
        hash.field("target_env", &self.target_env);
        hash.field("target_abi", &self.target_abi);
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
    pub source_contract_hash: String,
}

pub struct ArtifactProfileInput<'a, I> {
    pub crate_name: &'a str,
    pub version: &'a str,
    pub target: &'a str,
    pub link_type: &'a str,
    pub crt: &'a str,
    pub features: I,
    pub source_revisions: SourceRevisions,
    pub binding_spec_hash: String,
    pub source_contract_hash: String,
}

/// Hash the repository-owned source transforms applied to the core native
/// and WebAssembly providers.
///
/// Transform versions are explicit review boundaries: changing a transform
/// requires bumping its version here, which invalidates older prebuilt
/// artifacts even when their upstream revisions and bindings are unchanged.
pub fn core_source_contract_hash() -> String {
    use crate::source_inventory::{ProviderTransform, SourceInventory};

    fn transform_contract(transform: ProviderTransform) -> (&'static str, &'static str) {
        match transform {
            ProviderTransform::Direct => ("direct", "v1"),
            ProviderTransform::PatchImguiCore => ("patch-imgui-core", "safe-demo-boundary-v1"),
            ProviderTransform::PatchImguiDemo => ("patch-imgui-demo", "safe-demo-boundary-v1"),
            ProviderTransform::PatchImguiWidgetsNumericConversions => (
                "patch-imgui-widgets-numeric-conversions",
                "defined-numeric-conversions-v2",
            ),
            ProviderTransform::PatchImnodesFileIo => {
                ("patch-imnodes-file-io", "imgui-file-handle-v1")
            }
        }
    }

    let inventory = SourceInventory::embedded();
    let core = inventory
        .require_source_by_crate("dear-imgui-sys")
        .expect("embedded source inventory must contain dear-imgui-sys");
    let mut transforms = core
        .files
        .iter()
        .filter_map(|file| {
            file.provider_transform.map(|transform| {
                let (name, version) = transform_contract(transform);
                (file.id.as_str(), name, version)
            })
        })
        .collect::<Vec<_>>();
    transforms.sort_unstable();

    let mut hash = StableHash::new();
    hash.field("schema", "core-source-contract-v1");
    hash.field("inventory_schema", &inventory.schema);
    hash.field("source", &core.id);
    hash.begin_list("maintained_transforms", transforms.len());
    for (index, (file, transform, version)) in transforms.into_iter().enumerate() {
        hash.list_item(index);
        hash.field("file", file);
        hash.field("transform", transform);
        hash.field("version", version);
    }
    // The stack-layout patch is native-only and therefore is not represented
    // by the WebAssembly provider transform inventory.
    hash.field(
        "native_stack_layout_transform",
        "imgui-core-stack-layout-v1",
    );
    hash.field(
        "native_platform_io_contract",
        "aggregate-hooks-v3+live-viewport-address-v1",
    );
    hash.finish()
}

impl ArtifactProfile {
    pub fn new<I, S>(input: ArtifactProfileInput<'_, I>) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self {
            crate_name: input.crate_name.to_owned(),
            version: input.version.to_owned(),
            target: input.target.to_owned(),
            link_type: input.link_type.to_owned(),
            crt: input.crt.to_owned(),
            features: normalize_values(input.features),
            source_revisions: input.source_revisions,
            binding_spec_hash: input.binding_spec_hash,
            source_contract_hash: input.source_contract_hash,
        }
    }

    pub fn deterministic_hash(&self) -> String {
        let mut hash = StableHash::new();
        hash.field("schema", "core-artifact-profile-v3");
        hash.field("crate_name", &self.crate_name);
        hash.field("version", &self.version);
        hash.field("target", &self.target);
        hash.field("link_type", &self.link_type);
        hash.field("crt", &self.crt);
        hash.fields("features", &self.features);
        hash.field("cimgui_revision", &self.source_revisions.cimgui);
        hash.field("imgui_revision", &self.source_revisions.imgui);
        hash.field("binding_spec_hash", &self.binding_spec_hash);
        hash.field("source_contract_hash", &self.source_contract_hash);
        hash.finish()
    }

    pub fn manifest_bytes(&self) -> Vec<u8> {
        format!(
            "{} prebuilt\nversion={}\ntarget={}\nlink={}\ncrt={}\nfeatures={}\ncimgui_revision={}\nimgui_revision={}\nbinding_spec_hash={}\nsource_contract_hash={}\n",
            self.crate_name,
            self.version,
            self.target,
            self.link_type,
            self.crt,
            self.features.join(","),
            self.source_revisions.cimgui,
            self.source_revisions.imgui,
            self.binding_spec_hash,
            self.source_contract_hash,
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
            ("source_contract_hash", self.source_contract_hash.as_str()),
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
            "{} prebuilt\nversion={}\ncandidate_sha={}\ntarget={}\nlink={}\ncrt={}\nfeatures={}\ncimgui_revision={}\nimgui_revision={}\nbinding_spec_hash={}\nsource_contract_hash={}\n",
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
            self.source_contract_hash,
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
            ("source_contract_hash", self.source_contract_hash.as_str()),
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
    crate::compose_archive_name(
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
