//! Canonical maintained-source and WebAssembly provider contracts.

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

const INVENTORY_SCHEMA: &str = "dear-imgui-maintained-sources-v2";
const WASM_MAGIC_AND_VERSION: &[u8; 8] = b"\0asm\x01\0\0\0";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceInventoryError {
    message: String,
}

impl SourceInventoryError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for SourceInventoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for SourceInventoryError {}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceInventory {
    pub schema: String,
    pub wasm_import_module: String,
    pub sources: Vec<MaintainedSource>,
    pub nested_submodules: Vec<NestedSubmodule>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MaintainedSource {
    pub id: String,
    pub crate_name: String,
    pub crate_root: String,
    pub source_root: String,
    pub api_contract: ApiContractSpec,
    pub files: Vec<MaintainedSourceFile>,
    pub native_required_files: Vec<String>,
    pub archive_sentinels: Vec<String>,
    pub provider: Option<WasmProviderSpec>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ApiContractSpec {
    CimguiGenerator { locations: Vec<String> },
    RustBindings { path: String },
}

impl ApiContractSpec {
    fn validate(&self) -> Result<(), SourceInventoryError> {
        match self {
            Self::CimguiGenerator { locations } => {
                if locations.is_empty() {
                    return Err(SourceInventoryError::new(
                        "source.api_contract.locations must not be empty",
                    ));
                }
                validate_unique_c_symbols("source.api_contract.locations", locations)
            }
            Self::RustBindings { path } => {
                validate_relative_path("source.api_contract.path", path)?;
                if !path.ends_with(".rs") {
                    return Err(SourceInventoryError::new(
                        "source.api_contract.path must name a .rs file",
                    ));
                }
                Ok(())
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MaintainedSourceFile {
    pub id: String,
    pub canonical: String,
    pub alternates: Vec<String>,
    pub provider_transform: Option<ProviderTransform>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderTransform {
    Direct,
    PatchImguiCore,
    PatchImguiDemo,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WasmProviderSpec {
    pub wasm_bindings: String,
    pub symbol_prefixes: Vec<String>,
    pub required_exports: Vec<String>,
    pub include_dirs: Vec<String>,
    pub source_files: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NestedSubmodule {
    pub parent: String,
    pub path: String,
    pub shallow: bool,
    pub package: bool,
    pub package_order: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedProviderSource<'a> {
    pub id: &'a str,
    pub path: PathBuf,
    pub transform: ProviderTransform,
}

#[derive(Clone, Debug)]
pub struct MaintainedSourcePaths {
    source: &'static MaintainedSource,
    crate_root: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WasmImportContract {
    pub module: String,
    pub symbols: BTreeSet<String>,
}

impl SourceInventory {
    pub fn parse(contents: &str) -> Result<Self, SourceInventoryError> {
        let inventory = serde_json::from_str::<Self>(contents).map_err(|error| {
            SourceInventoryError::new(format!("invalid maintained-source inventory JSON: {error}"))
        })?;
        inventory.validate()?;
        Ok(inventory)
    }

    pub fn try_embedded() -> Result<&'static Self, SourceInventoryError> {
        static INVENTORY: OnceLock<Result<SourceInventory, SourceInventoryError>> = OnceLock::new();
        match INVENTORY.get_or_init(|| Self::parse(include_str!("../maintained_sources.json"))) {
            Ok(inventory) => Ok(inventory),
            Err(error) => Err(error.clone()),
        }
    }

    pub fn embedded() -> &'static Self {
        Self::try_embedded().unwrap_or_else(|error| {
            panic!("embedded maintained-source inventory is invalid: {error}")
        })
    }

    pub fn source_by_id(&self, id: &str) -> Option<&MaintainedSource> {
        self.sources.iter().find(|source| source.id == id)
    }

    pub fn source_by_crate(&self, crate_name: &str) -> Option<&MaintainedSource> {
        self.sources
            .iter()
            .find(|source| source.crate_name == crate_name)
    }

    pub fn require_source_by_crate(
        &self,
        crate_name: &str,
    ) -> Result<&MaintainedSource, SourceInventoryError> {
        self.source_by_crate(crate_name).ok_or_else(|| {
            SourceInventoryError::new(format!(
                "crate {crate_name:?} is absent from the maintained-source inventory"
            ))
        })
    }

    pub fn validate(&self) -> Result<(), SourceInventoryError> {
        if self.schema != INVENTORY_SCHEMA {
            return Err(SourceInventoryError::new(format!(
                "unsupported maintained-source inventory schema {:?}; expected {INVENTORY_SCHEMA:?}",
                self.schema
            )));
        }
        validate_identifier("wasm_import_module", &self.wasm_import_module)?;
        if self.sources.is_empty() {
            return Err(SourceInventoryError::new(
                "maintained-source inventory must contain at least one source",
            ));
        }

        let mut source_ids = BTreeSet::new();
        let mut crate_names = BTreeSet::new();
        let mut crate_roots = BTreeSet::new();
        for source in &self.sources {
            validate_identifier("source.id", &source.id)?;
            validate_identifier("source.crate_name", &source.crate_name)?;
            validate_relative_path("source.crate_root", &source.crate_root)?;
            validate_relative_path("source.source_root", &source.source_root)?;
            source.api_contract.validate()?;
            insert_unique(&mut source_ids, &source.id, "source id")?;
            insert_unique(&mut crate_names, &source.crate_name, "source crate name")?;
            insert_unique(&mut crate_roots, &source.crate_root, "source crate root")?;
            source.validate()?;
        }

        let mut submodule_locations = BTreeSet::new();
        let mut package_orders = BTreeSet::new();
        for submodule in &self.nested_submodules {
            validate_relative_path("nested_submodule.parent", &submodule.parent)?;
            validate_relative_path("nested_submodule.path", &submodule.path)?;
            let location = format!("{}/{}", submodule.parent, submodule.path);
            insert_unique(
                &mut submodule_locations,
                &location,
                "nested submodule location",
            )?;
            match (submodule.package, submodule.package_order) {
                (true, Some(order)) => {
                    insert_unique(
                        &mut package_orders,
                        &order,
                        "nested submodule package order",
                    )?;
                }
                (false, None) => {}
                _ => {
                    return Err(SourceInventoryError::new(format!(
                        "nested submodule {location:?} must set package_order exactly when package is true"
                    )));
                }
            }
        }
        Ok(())
    }
}

impl MaintainedSource {
    pub fn file(&self, id: &str) -> Option<&MaintainedSourceFile> {
        self.files.iter().find(|file| file.id == id)
    }

    pub fn resolve_source_root(
        &self,
        manifest_dir: impl AsRef<Path>,
    ) -> Result<PathBuf, SourceInventoryError> {
        let path = manifest_dir.as_ref().join(&self.source_root);
        if !path.is_dir() {
            return Err(SourceInventoryError::new(format!(
                "maintained source {:?} for crate {:?} is missing directory {}",
                self.id,
                self.crate_name,
                path.display()
            )));
        }
        Ok(path)
    }

    pub fn resolve_file(
        &self,
        manifest_dir: impl AsRef<Path>,
        file_id: &str,
    ) -> Result<PathBuf, SourceInventoryError> {
        let file = self.file(file_id).ok_or_else(|| {
            SourceInventoryError::new(format!(
                "maintained source {:?} does not define file id {file_id:?}",
                self.id
            ))
        })?;
        let manifest_dir = manifest_dir.as_ref();
        let candidates = std::iter::once(&file.canonical)
            .chain(file.alternates.iter())
            .map(|relative| manifest_dir.join(relative))
            .collect::<Vec<_>>();
        let existing = candidates
            .iter()
            .filter(|candidate| candidate.is_file())
            .collect::<Vec<_>>();
        match existing.as_slice() {
            [path] => Ok((*path).clone()),
            [] => Err(SourceInventoryError::new(format!(
                "maintained source {:?} file {file_id:?} is missing; expected exactly one of: {}",
                self.id,
                display_paths(candidates.iter().map(PathBuf::as_path))
            ))),
            _ => Err(SourceInventoryError::new(format!(
                "maintained source {:?} file {file_id:?} is ambiguous; found multiple supported paths: {}",
                self.id,
                display_paths(existing.iter().map(|path| path.as_path()))
            ))),
        }
    }

    pub fn validate_native_sources(
        &self,
        manifest_dir: impl AsRef<Path>,
    ) -> Result<Vec<PathBuf>, SourceInventoryError> {
        let manifest_dir = manifest_dir.as_ref();
        self.resolve_source_root(manifest_dir)?;
        self.native_required_files
            .iter()
            .map(|file_id| self.resolve_file(manifest_dir, file_id))
            .collect()
    }

    pub fn provider_sources(
        &self,
        manifest_dir: impl AsRef<Path>,
    ) -> Result<Vec<ResolvedProviderSource<'_>>, SourceInventoryError> {
        let provider = self.provider.as_ref().ok_or_else(|| {
            SourceInventoryError::new(format!(
                "maintained source {:?} does not participate in the WebAssembly provider",
                self.id
            ))
        })?;
        provider
            .source_files
            .iter()
            .map(|file_id| {
                let file = self
                    .file(file_id)
                    .expect("validated provider file reference");
                let transform = file
                    .provider_transform
                    .expect("validated provider transform");
                Ok(ResolvedProviderSource {
                    id: file_id,
                    path: self.resolve_file(manifest_dir.as_ref(), file_id)?,
                    transform,
                })
            })
            .collect()
    }

    fn validate(&self) -> Result<(), SourceInventoryError> {
        if self.files.is_empty() {
            return Err(SourceInventoryError::new(format!(
                "maintained source {:?} must define at least one file",
                self.id
            )));
        }
        let mut file_ids = BTreeSet::new();
        let mut source_paths = BTreeSet::new();
        let mut files_by_id = BTreeMap::new();
        for file in &self.files {
            validate_identifier("source.files[].id", &file.id)?;
            insert_unique(&mut file_ids, &file.id, "file id")?;
            validate_relative_path("source.files[].canonical", &file.canonical)?;
            insert_unique(&mut source_paths, &file.canonical, "source file path")?;
            let mut local_paths = BTreeSet::from([file.canonical.as_str()]);
            for alternate in &file.alternates {
                validate_relative_path("source.files[].alternates[]", alternate)?;
                insert_unique(
                    &mut local_paths,
                    &alternate.as_str(),
                    "source file candidate path",
                )?;
                insert_unique(&mut source_paths, alternate, "source file path")?;
            }
            files_by_id.insert(file.id.as_str(), file);
        }
        validate_file_references(
            &self.id,
            "native_required_files",
            &self.native_required_files,
            &files_by_id,
        )?;
        validate_file_references(
            &self.id,
            "archive_sentinels",
            &self.archive_sentinels,
            &files_by_id,
        )?;

        match &self.provider {
            Some(provider) => provider.validate(self, &files_by_id)?,
            None => {
                if let Some(file) = self
                    .files
                    .iter()
                    .find(|file| file.provider_transform.is_some())
                {
                    return Err(SourceInventoryError::new(format!(
                        "maintained source {:?} file {:?} declares a provider transform without a provider",
                        self.id, file.id
                    )));
                }
            }
        }
        Ok(())
    }
}

impl MaintainedSourcePaths {
    pub fn for_crate(
        crate_name: &str,
        crate_root: impl Into<PathBuf>,
    ) -> Result<Self, SourceInventoryError> {
        Ok(Self {
            source: SourceInventory::embedded().require_source_by_crate(crate_name)?,
            crate_root: crate_root.into(),
        })
    }

    pub fn source(&self) -> &'static MaintainedSource {
        self.source
    }

    pub fn crate_root(&self) -> &Path {
        &self.crate_root
    }

    pub fn source_root(&self) -> Result<PathBuf, SourceInventoryError> {
        self.source.resolve_source_root(&self.crate_root)
    }

    pub fn file(&self, file_id: &str) -> Result<PathBuf, SourceInventoryError> {
        self.source.resolve_file(&self.crate_root, file_id)
    }

    pub fn validate_native(&self) -> Result<Vec<PathBuf>, SourceInventoryError> {
        self.source.validate_native_sources(&self.crate_root)
    }

    pub fn native_candidate_paths(&self) -> Vec<PathBuf> {
        self.candidate_paths(self.source.native_required_files.iter().map(String::as_str))
    }

    pub fn all_candidate_paths(&self) -> Vec<PathBuf> {
        self.candidate_paths(self.source.files.iter().map(|file| file.id.as_str()))
    }

    fn candidate_paths<'a>(&self, file_ids: impl IntoIterator<Item = &'a str>) -> Vec<PathBuf> {
        file_ids
            .into_iter()
            .flat_map(|file_id| {
                let file = self
                    .source
                    .file(file_id)
                    .expect("validated source file reference");
                std::iter::once(&file.canonical)
                    .chain(file.alternates.iter())
                    .map(|relative| self.crate_root.join(relative))
            })
            .collect()
    }
}

impl WasmProviderSpec {
    fn validate(
        &self,
        source: &MaintainedSource,
        files_by_id: &BTreeMap<&str, &MaintainedSourceFile>,
    ) -> Result<(), SourceInventoryError> {
        validate_relative_path("provider.wasm_bindings", &self.wasm_bindings)?;
        validate_file_references(
            &source.id,
            "provider.source_files",
            &self.source_files,
            files_by_id,
        )?;
        let provider_files = self
            .source_files
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        for file_id in &self.source_files {
            if files_by_id[file_id.as_str()].provider_transform.is_none() {
                return Err(SourceInventoryError::new(format!(
                    "maintained source {:?} provider file {file_id:?} is missing provider_transform",
                    source.id
                )));
            }
        }
        if let Some(file) = source.files.iter().find(|file| {
            file.provider_transform.is_some() && !provider_files.contains(file.id.as_str())
        }) {
            return Err(SourceInventoryError::new(format!(
                "maintained source {:?} file {:?} declares a provider transform but is absent from provider.source_files",
                source.id, file.id
            )));
        }

        validate_unique_c_symbols("provider.symbol_prefixes", &self.symbol_prefixes)?;
        validate_unique_c_symbols("provider.required_exports", &self.required_exports)?;
        let mut include_dirs = BTreeSet::new();
        for include_dir in &self.include_dirs {
            validate_relative_path("provider.include_dirs[]", include_dir)?;
            insert_unique(&mut include_dirs, include_dir, "provider include directory")?;
        }
        Ok(())
    }
}

pub fn parse_wasm_imports(
    contents: &str,
    expected_module: &str,
) -> Result<WasmImportContract, SourceInventoryError> {
    let mut active_module = None::<String>;
    let mut pending_link_name = None::<String>;
    let mut symbols = BTreeSet::new();

    for (line_index, line) in contents.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("#[link(wasm_import_module") {
            if active_module.is_some() {
                return Err(SourceInventoryError::new(format!(
                    "nested wasm_import_module attribute at line {}",
                    line_index + 1
                )));
            }
            let module = quoted_attribute_value(trimmed).ok_or_else(|| {
                SourceInventoryError::new(format!(
                    "malformed wasm_import_module attribute at line {}",
                    line_index + 1
                ))
            })?;
            if module != expected_module {
                return Err(SourceInventoryError::new(format!(
                    "unexpected WebAssembly import module {module:?} at line {}; expected {expected_module:?}",
                    line_index + 1
                )));
            }
            active_module = Some(module.to_owned());
            continue;
        }
        if active_module.is_none() {
            continue;
        }
        if trimmed.starts_with("#[link_name") {
            pending_link_name = Some(
                quoted_attribute_value(trimmed)
                    .ok_or_else(|| {
                        SourceInventoryError::new(format!(
                            "malformed link_name attribute at line {}",
                            line_index + 1
                        ))
                    })?
                    .to_owned(),
            );
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("pub fn ") {
            let rust_name = rest
                .split(|character: char| character == '(' || character.is_whitespace())
                .next()
                .filter(|name| !name.is_empty())
                .ok_or_else(|| {
                    SourceInventoryError::new(format!(
                        "malformed imported function at line {}",
                        line_index + 1
                    ))
                })?;
            let symbol = pending_link_name
                .take()
                .unwrap_or_else(|| rust_name.to_owned());
            validate_c_symbol("WebAssembly import symbol", &symbol)?;
            if !symbols.insert(symbol.clone()) {
                return Err(SourceInventoryError::new(format!(
                    "duplicate WebAssembly import symbol {symbol:?}"
                )));
            }
            continue;
        }
        if trimmed == "}" {
            if pending_link_name.is_some() {
                return Err(SourceInventoryError::new(format!(
                    "link_name attribute at line {} is not attached to an imported function",
                    line_index + 1
                )));
            }
            active_module = None;
        }
    }

    if active_module.is_some() {
        return Err(SourceInventoryError::new(
            "unterminated WebAssembly import block",
        ));
    }
    if symbols.is_empty() {
        return Err(SourceInventoryError::new(
            "WebAssembly binding source does not declare any imported functions",
        ));
    }
    Ok(WasmImportContract {
        module: expected_module.to_owned(),
        symbols,
    })
}

pub fn parse_wasm_exports(bytes: &[u8]) -> Result<BTreeSet<String>, SourceInventoryError> {
    if bytes.get(..WASM_MAGIC_AND_VERSION.len()) != Some(WASM_MAGIC_AND_VERSION) {
        return Err(SourceInventoryError::new(
            "invalid WebAssembly magic or version",
        ));
    }
    let mut offset = WASM_MAGIC_AND_VERSION.len();
    let mut exports = BTreeSet::new();
    let mut export_names = BTreeSet::new();
    while offset < bytes.len() {
        let section_id = read_byte(bytes, &mut offset, "section id")?;
        let section_size = read_u32_leb(bytes, &mut offset, "section size")? as usize;
        let section_end = offset.checked_add(section_size).ok_or_else(|| {
            SourceInventoryError::new("WebAssembly section size overflows address space")
        })?;
        if section_end > bytes.len() {
            return Err(SourceInventoryError::new(
                "WebAssembly section extends beyond end of file",
            ));
        }
        if section_id == 7 {
            let mut export_offset = offset;
            let export_count = read_u32_leb(bytes, &mut export_offset, "export count")?;
            for _ in 0..export_count {
                let name_length =
                    read_u32_leb(bytes, &mut export_offset, "export name length")? as usize;
                let name_end = export_offset.checked_add(name_length).ok_or_else(|| {
                    SourceInventoryError::new("WebAssembly export name length overflows")
                })?;
                if name_end > section_end {
                    return Err(SourceInventoryError::new(
                        "WebAssembly export name extends beyond export section",
                    ));
                }
                let name = std::str::from_utf8(&bytes[export_offset..name_end])
                    .map_err(|_| SourceInventoryError::new("WebAssembly export name is not UTF-8"))?
                    .to_owned();
                export_offset = name_end;
                let kind = read_byte(bytes, &mut export_offset, "export kind")?;
                if kind > 4 {
                    return Err(SourceInventoryError::new(format!(
                        "unsupported WebAssembly export kind {kind}"
                    )));
                }
                let _index = read_u32_leb(bytes, &mut export_offset, "export index")?;
                if !export_names.insert(name.clone()) {
                    return Err(SourceInventoryError::new(format!(
                        "duplicate WebAssembly export {name:?}"
                    )));
                }
                if kind == 0 {
                    exports.insert(name);
                }
            }
            if export_offset != section_end {
                return Err(SourceInventoryError::new(
                    "WebAssembly export section contains trailing bytes",
                ));
            }
        }
        offset = section_end;
    }
    Ok(exports)
}

pub fn verify_wasm_exports<'a>(
    actual: &BTreeSet<String>,
    required: impl IntoIterator<Item = &'a str>,
) -> Result<(), SourceInventoryError> {
    let missing = required
        .into_iter()
        .filter(|symbol| !actual.contains(*symbol))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(SourceInventoryError::new(format!(
            "WebAssembly provider is missing required exports: {}",
            missing.join(", ")
        )))
    }
}

pub fn verify_emscripten_wasm_exports<'a>(
    actual: &BTreeSet<String>,
    required: impl IntoIterator<Item = &'a str>,
) -> Result<(), SourceInventoryError> {
    let missing = required
        .into_iter()
        .filter(|symbol| {
            !actual.contains(*symbol) && !actual.contains(format!("_{symbol}").as_str())
        })
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(SourceInventoryError::new(format!(
            "Emscripten provider is missing required exports: {}",
            missing.join(", ")
        )))
    }
}

fn validate_file_references(
    source_id: &str,
    field: &str,
    references: &[String],
    files_by_id: &BTreeMap<&str, &MaintainedSourceFile>,
) -> Result<(), SourceInventoryError> {
    let mut seen = BTreeSet::new();
    for file_id in references {
        if !files_by_id.contains_key(file_id.as_str()) {
            return Err(SourceInventoryError::new(format!(
                "maintained source {source_id:?} {field} references unknown file id {file_id:?}"
            )));
        }
        insert_unique(&mut seen, file_id, field)?;
    }
    Ok(())
}

fn validate_unique_c_symbols(field: &str, values: &[String]) -> Result<(), SourceInventoryError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_c_symbol(field, value)?;
        insert_unique(&mut seen, value, field)?;
    }
    Ok(())
}

fn validate_c_symbol(field: &str, value: &str) -> Result<(), SourceInventoryError> {
    let mut characters = value.chars();
    let valid_start = characters
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic());
    if !valid_start
        || characters.any(|character| character != '_' && !character.is_ascii_alphanumeric())
    {
        return Err(SourceInventoryError::new(format!(
            "{field} must be a portable C symbol, got {value:?}"
        )));
    }
    Ok(())
}

fn validate_identifier(field: &str, value: &str) -> Result<(), SourceInventoryError> {
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        return Err(SourceInventoryError::new(format!(
            "{field} must be a non-empty identifier without whitespace, got {value:?}"
        )));
    }
    Ok(())
}

fn validate_relative_path(field: &str, value: &str) -> Result<(), SourceInventoryError> {
    let path = Path::new(value);
    let has_invalid_component = path.components().any(|component| {
        matches!(
            component,
            Component::Prefix(_) | Component::RootDir | Component::ParentDir | Component::CurDir
        )
    });
    if value.is_empty()
        || value.contains('\\')
        || value.contains(':')
        || value.split('/').any(str::is_empty)
        || has_invalid_component
    {
        return Err(SourceInventoryError::new(format!(
            "{field} must be a normalized forward-slash relative path, got {value:?}"
        )));
    }
    Ok(())
}

fn insert_unique<T>(
    values: &mut BTreeSet<T>,
    value: &T,
    description: &str,
) -> Result<(), SourceInventoryError>
where
    T: Clone + fmt::Debug + Ord,
{
    if values.insert(value.clone()) {
        Ok(())
    } else {
        Err(SourceInventoryError::new(format!(
            "duplicate {description}: {value:?}"
        )))
    }
}

fn display_paths<'a>(paths: impl IntoIterator<Item = &'a Path>) -> String {
    paths
        .into_iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn quoted_attribute_value(attribute: &str) -> Option<&str> {
    let start = attribute.find('"')? + 1;
    let end = attribute[start..].find('"')? + start;
    Some(&attribute[start..end])
}

fn read_byte(
    bytes: &[u8],
    offset: &mut usize,
    description: &str,
) -> Result<u8, SourceInventoryError> {
    let byte = bytes.get(*offset).copied().ok_or_else(|| {
        SourceInventoryError::new(format!(
            "unexpected end of WebAssembly while reading {description}"
        ))
    })?;
    *offset += 1;
    Ok(byte)
}

fn read_u32_leb(
    bytes: &[u8],
    offset: &mut usize,
    description: &str,
) -> Result<u32, SourceInventoryError> {
    let mut value = 0_u32;
    for shift in (0..35).step_by(7) {
        let byte = read_byte(bytes, offset, description)?;
        if shift == 28 && byte & 0xf0 != 0 {
            return Err(SourceInventoryError::new(format!(
                "overflowing unsigned LEB128 while reading {description}"
            )));
        }
        value |= u32::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(SourceInventoryError::new(format!(
        "unterminated unsigned LEB128 while reading {description}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn wasm_with_typed_exports(exports: &[(&str, u8)]) -> Vec<u8> {
        fn encode_u32(mut value: u32, output: &mut Vec<u8>) {
            loop {
                let mut byte = (value & 0x7f) as u8;
                value >>= 7;
                if value != 0 {
                    byte |= 0x80;
                }
                output.push(byte);
                if value == 0 {
                    break;
                }
            }
        }

        let mut payload = Vec::new();
        encode_u32(exports.len() as u32, &mut payload);
        for (index, (name, kind)) in exports.iter().enumerate() {
            encode_u32(name.len() as u32, &mut payload);
            payload.extend_from_slice(name.as_bytes());
            payload.push(*kind);
            encode_u32(index as u32, &mut payload);
        }
        let mut wasm = WASM_MAGIC_AND_VERSION.to_vec();
        wasm.push(7);
        encode_u32(payload.len() as u32, &mut wasm);
        wasm.extend(payload);
        wasm
    }

    fn wasm_with_exports(names: &[&str]) -> Vec<u8> {
        let exports = names.iter().map(|name| (*name, 0)).collect::<Vec<_>>();
        wasm_with_typed_exports(&exports)
    }

    #[test]
    fn embedded_inventory_is_strict_and_deterministic() {
        let first = SourceInventory::parse(include_str!("../maintained_sources.json")).unwrap();
        let second = SourceInventory::parse(include_str!("../maintained_sources.json")).unwrap();
        assert_eq!(first, second);
        assert_eq!(SourceInventory::embedded(), &first);
    }

    #[test]
    fn api_contract_provider_is_mandatory_and_fails_closed() {
        let mut inventory = SourceInventory::embedded().clone();
        inventory.sources[0].api_contract = ApiContractSpec::CimguiGenerator {
            locations: Vec::new(),
        };
        assert!(
            inventory
                .validate()
                .unwrap_err()
                .to_string()
                .contains("api_contract.locations must not be empty")
        );

        let mut inventory = SourceInventory::embedded().clone();
        inventory.sources[0].api_contract = ApiContractSpec::RustBindings {
            path: "../bindings.rs".into(),
        };
        assert!(
            inventory
                .validate()
                .unwrap_err()
                .to_string()
                .contains("api_contract.path")
        );

        let mut inventory = SourceInventory::embedded().clone();
        inventory.sources[0].api_contract = ApiContractSpec::RustBindings {
            path: "src/bindings.txt".into(),
        };
        assert!(
            inventory
                .validate()
                .unwrap_err()
                .to_string()
                .contains("must name a .rs file")
        );
    }

    #[test]
    fn every_native_source_resolves_from_the_checked_in_inventory() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for source in &SourceInventory::embedded().sources {
            let crate_root = workspace.join(&source.crate_root);
            source
                .validate_native_sources(&crate_root)
                .unwrap_or_else(|error| {
                    panic!(
                        "{} native source contract failed: {error}",
                        source.crate_name
                    )
                });
        }
    }

    #[test]
    fn resolve_file_requires_exactly_one_supported_path() {
        let temp = tempfile::tempdir().unwrap();
        let source = MaintainedSource {
            id: "test".into(),
            crate_name: "test-sys".into(),
            crate_root: "test-sys".into(),
            source_root: "third-party/source".into(),
            api_contract: ApiContractSpec::CimguiGenerator {
                locations: vec!["test".into()],
            },
            files: vec![MaintainedSourceFile {
                id: "implementation".into(),
                canonical: "third-party/source/src/implementation.cpp".into(),
                alternates: vec!["third-party/source/implementation.cpp".into()],
                provider_transform: Some(ProviderTransform::Direct),
            }],
            native_required_files: vec!["implementation".into()],
            archive_sentinels: vec!["implementation".into()],
            provider: Some(WasmProviderSpec {
                wasm_bindings: "src/wasm.rs".into(),
                symbol_prefixes: vec!["Test".into()],
                required_exports: Vec::new(),
                include_dirs: vec!["third-party/source".into()],
                source_files: vec!["implementation".into()],
            }),
        };

        let missing = source
            .resolve_file(temp.path(), "implementation")
            .unwrap_err();
        assert!(missing.to_string().contains("expected exactly one of"));

        let canonical = temp
            .path()
            .join("third-party/source/src/implementation.cpp");
        fs::create_dir_all(canonical.parent().unwrap()).unwrap();
        fs::write(&canonical, "canonical").unwrap();
        assert_eq!(
            source.resolve_file(temp.path(), "implementation").unwrap(),
            canonical
        );

        let alternate = temp.path().join("third-party/source/implementation.cpp");
        fs::write(&alternate, "alternate").unwrap();
        let ambiguous = source
            .resolve_file(temp.path(), "implementation")
            .unwrap_err();
        assert!(ambiguous.to_string().contains("ambiguous"));
    }

    #[test]
    fn parse_wasm_imports_rejects_an_unexpected_module() {
        let source = r#"
#[link(wasm_import_module = "wrong-module")]
unsafe extern "C" {
    pub fn igTest();
}
"#;
        let error = parse_wasm_imports(source, "imgui-sys-v0").unwrap_err();
        assert!(
            error
                .to_string()
                .contains("unexpected WebAssembly import module")
        );
    }

    #[test]
    fn parse_wasm_imports_rejects_non_c_export_names() {
        let source = r#"
#[link(wasm_import_module = "imgui-sys-v0")]
unsafe extern "C" {
    #[link_name = "igTest\";globalThis.injected=true;//"]
    pub fn igTest();
}
"#;
        let error = parse_wasm_imports(source, "imgui-sys-v0").unwrap_err();
        assert!(error.to_string().contains("portable C symbol"));
    }

    #[test]
    fn parse_wasm_exports_is_structural_and_reports_missing_symbols() {
        let exports = parse_wasm_exports(&wasm_with_exports(&["igTest", "ImGui_Test"])).unwrap();
        assert!(exports.contains("igTest"));
        assert!(exports.contains("ImGui_Test"));
        verify_wasm_exports(&exports, ["igTest"]).unwrap();
        let error = verify_wasm_exports(&exports, ["ImGuizmo_ComputeMouseRay"]).unwrap_err();
        assert!(error.to_string().contains("ImGuizmo_ComputeMouseRay"));
        let non_function = parse_wasm_exports(&wasm_with_typed_exports(&[("igTest", 2)])).unwrap();
        assert!(verify_wasm_exports(&non_function, ["igTest"]).is_err());
        assert!(parse_wasm_exports(b"not-wasm").is_err());
    }

    #[test]
    fn emscripten_export_verification_accepts_its_leading_underscore_abi() {
        let exports = BTreeSet::from(["_igTest".to_owned()]);
        verify_emscripten_wasm_exports(&exports, ["igTest"]).unwrap();
        assert!(verify_emscripten_wasm_exports(&exports, ["igMissing"]).is_err());
    }

    #[test]
    fn checked_in_imguizmo_contract_includes_compute_mouse_ray() {
        let inventory = SourceInventory::embedded();
        let source = inventory.source_by_id("imguizmo").unwrap();
        let provider = source.provider.as_ref().unwrap();
        assert!(
            provider
                .required_exports
                .iter()
                .any(|symbol| symbol == "ImGuizmo_ComputeMouseRay")
        );
        let bindings =
            include_str!("../../../extensions/dear-imguizmo-sys/src/wasm_bindings_pregenerated.rs");
        let imports = parse_wasm_imports(bindings, &inventory.wasm_import_module).unwrap();
        assert!(imports.symbols.contains("ImGuizmo_ComputeMouseRay"));
    }
}
