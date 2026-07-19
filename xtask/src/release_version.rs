use anyhow::{Context, Result, bail, ensure};
use semver::Version;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::NamedTempFile;
use toml_edit::{DocumentMut, Formatted, Item, Table, Value};

const DEPENDENCY_SECTIONS: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

#[derive(Clone, Debug, PartialEq, Eq)]
struct Options {
    version: Version,
    dry_run: bool,
}

impl Options {
    fn parse(args: &[String]) -> Result<Self> {
        let raw = args
            .first()
            .context("usage: xtask release-version <semver> [--dry-run]")?;
        let version =
            Version::parse(raw).with_context(|| format!("invalid release version {raw:?}"))?;
        let mut dry_run = false;
        for argument in &args[1..] {
            match argument.as_str() {
                "--dry-run" if !dry_run => dry_run = true,
                "--dry-run" => bail!("--dry-run may only be specified once"),
                _ => bail!("unknown release-version option: {argument}"),
            }
        }
        Ok(Self { version, dry_run })
    }
}

#[derive(Debug)]
struct Member {
    relative_manifest: PathBuf,
    manifest_path: PathBuf,
    directory: PathBuf,
    name: String,
    publishable: bool,
    document: DocumentMut,
}

#[derive(Debug)]
struct PrivatePackagePolicy {
    directory: PathBuf,
    version: String,
}

#[derive(Debug)]
struct ReleasePolicy {
    core_package: String,
    private_packages: BTreeMap<String, PrivatePackagePolicy>,
}

#[derive(Debug)]
struct Graph {
    root: PathBuf,
    root_manifest: PathBuf,
    root_source: String,
    root_document: DocumentMut,
    release_policy: ReleasePolicy,
    members: Vec<Member>,
}

impl Graph {
    fn load(root: &Path) -> Result<Self> {
        let root = fs::canonicalize(root)
            .with_context(|| format!("canonicalize workspace root {}", root.display()))?;
        let root_manifest = root.join("Cargo.toml");
        let root_source = read_source(&root_manifest)?;
        let root_document = parse_document(&root_source, &root_manifest)?;
        let release_policy = release_policy(&root_document, &root_manifest)?;
        let member_paths = workspace_member_paths(&root_document)?;

        let mut seen_directories = BTreeSet::new();
        let mut members = Vec::with_capacity(member_paths.len());
        for member_path in member_paths {
            let directory = fs::canonicalize(root.join(&member_path)).with_context(|| {
                format!("canonicalize workspace member {}", member_path.display())
            })?;
            ensure!(
                seen_directories.insert(directory.clone()),
                "duplicate workspace member path: {}",
                member_path.display()
            );
            let relative_manifest = member_path.join("Cargo.toml");
            let manifest_path = root.join(&relative_manifest);
            let document = parse_document(&read_source(&manifest_path)?, &manifest_path)?;
            let package = package_table(&document, &manifest_path)?;
            let name = package
                .get("name")
                .and_then(Item::as_str)
                .with_context(|| format!("{} is missing package.name", manifest_path.display()))?
                .to_owned();
            let publishable = package_is_publishable(package);
            members.push(Member {
                relative_manifest,
                manifest_path,
                directory,
                name,
                publishable,
                document,
            });
        }
        Ok(Self {
            root,
            root_manifest,
            root_source,
            root_document,
            release_policy,
            members,
        })
    }

    fn git_paths(&self) -> impl Iterator<Item = &Path> {
        std::iter::once(Path::new("Cargo.toml")).chain(
            self.members
                .iter()
                .map(|member| member.relative_manifest.as_path()),
        )
    }
}

#[derive(Debug)]
struct Plan {
    graph: Graph,
    current: Version,
    target: Version,
    requirement: String,
    updated_root: String,
    edge_count: usize,
    publishable_count: usize,
    private_count: usize,
}

impl Plan {
    fn build(root: &Path, target: Version) -> Result<Self> {
        let graph = Graph::load(root)?;
        let current = validate_graph(&graph, &graph.root_document)?.version;
        ensure!(
            target >= current,
            "release target {target} must not be older than current workspace version {current}"
        );
        let requirement = release_requirement(&target)?;
        let mut updated = parse_document(&graph.root_source, &graph.root_manifest)?;
        update_root(&mut updated, &target.to_string(), &requirement)?;
        let updated_root = updated.to_string();
        let rendered = parse_document(&updated_root, &graph.root_manifest)?;
        let validation = validate_graph(&graph, &rendered)?;
        ensure!(
            validation.version == target,
            "rendered release graph resolved to {}, expected {target}",
            validation.version
        );
        Ok(Self {
            graph,
            current,
            target,
            requirement,
            updated_root,
            edge_count: validation.edge_count,
            publishable_count: validation.publishable_count,
            private_count: validation.private_count,
        })
    }

    fn is_idempotent(&self) -> bool {
        self.updated_root == self.graph.root_source
    }
}

pub(crate) fn run(root: &Path, args: &[String]) -> Result<()> {
    let options = Options::parse(args)?;
    let plan = Plan::build(root, options.version)?;
    if plan.is_idempotent() {
        eprintln!(
            "release graph is already at {} ({} publishable packages, {} internal edges)",
            plan.target, plan.publishable_count, plan.edge_count
        );
        return Ok(());
    }
    let action = if options.dry_run {
        "would update"
    } else {
        "updating"
    };
    eprintln!(
        "{action} workspace release {} -> {}; internal requirement {}; root Cargo.toml only",
        plan.current, plan.target, plan.requirement
    );
    eprintln!(
        "validated {} publishable packages, {} private packages, and {} internal edges",
        plan.publishable_count, plan.private_count, plan.edge_count
    );
    if options.dry_run {
        return Ok(());
    }
    apply(&plan, true, || Ok(()))
}

struct Validation {
    version: Version,
    edge_count: usize,
    publishable_count: usize,
    private_count: usize,
}

struct CatalogEntry {
    package: String,
    directory: PathBuf,
}

fn validate_graph(graph: &Graph, root: &DocumentMut) -> Result<Validation> {
    let version = workspace_version(root, &graph.root_manifest)?;
    let requirement = release_requirement(&version)?;
    let expected_private = &graph.release_policy.private_packages;
    let mut publishable_by_directory = BTreeMap::new();
    let mut private_count = 0;
    let mut names = BTreeSet::new();
    let mut core_is_publishable = false;

    for member in &graph.members {
        ensure!(
            names.insert(member.name.as_str()),
            "duplicate package name: {}",
            member.name
        );
        let package = package_table(&member.document, &member.manifest_path)?;
        if member.name == graph.release_policy.core_package {
            core_is_publishable = member.publishable;
        }
        if member.publishable {
            ensure!(
                package
                    .get("version")
                    .and_then(|item| item.get("workspace"))
                    .and_then(Item::as_bool)
                    == Some(true),
                "publishable package {} must use package.version.workspace = true",
                member.name
            );
            publishable_by_directory.insert(member.directory.clone(), member.name.clone());
        } else {
            private_count += 1;
            let expected = expected_private
                .get(member.name.as_str())
                .with_context(|| format!("unexpected non-publishable package {}", member.name))?;
            ensure!(
                member.relative_manifest.parent() == Some(expected.directory.as_path()),
                "private package {} must remain at {}",
                member.name,
                expected.directory.display()
            );
            ensure!(
                package.get("version").and_then(Item::as_str) == Some(expected.version.as_str()),
                "private package {} must remain at {}",
                member.name,
                expected.version
            );
        }
    }
    ensure!(
        core_is_publishable,
        "release core package {} must exist and be publishable",
        graph.release_policy.core_package
    );
    ensure!(
        private_count == expected_private.len(),
        "release graph contains {private_count} private packages, but policy declares {}",
        expected_private.len()
    );

    let dependencies = root
        .get("workspace")
        .and_then(|item| item.get("dependencies"))
        .and_then(Item::as_table)
        .context("root Cargo.toml is missing [workspace.dependencies]")?;
    let mut catalog = BTreeMap::new();
    let mut catalog_directories = BTreeSet::new();
    for (key, item) in dependencies.iter() {
        let Some(relative_path) = dependency_string(item, "path") else {
            continue;
        };
        ensure!(
            dependency_bool(item, "optional").is_none(),
            "workspace dependency {key} cannot be optional"
        );
        let directory = fs::canonicalize(graph.root.join(relative_path)).with_context(|| {
            format!("canonicalize workspace dependency {key} path {relative_path}")
        })?;
        ensure!(
            catalog_directories.insert(directory.clone()),
            "workspace dependency path is duplicated: {relative_path}"
        );
        let expected_package = publishable_by_directory.get(&directory).with_context(|| {
            format!("workspace dependency {key} path does not identify a publishable member")
        })?;
        let package = dependency_string(item, "package").unwrap_or(key);
        ensure!(
            package == expected_package,
            "workspace dependency {key} names {package}, but its path contains {expected_package}"
        );
        if package == "dear-imgui-build-support" {
            ensure!(
                key == "build-support"
                    && dependency_string(item, "package") == Some("dear-imgui-build-support"),
                "dear-imgui-build-support must use the build-support catalog alias"
            );
        } else {
            ensure!(
                dependency_string(item, "package").is_none(),
                "workspace dependency {key} must not rename package {package}"
            );
        }
        ensure!(
            dependency_string(item, "version") == Some(requirement.as_str()),
            "workspace dependency {key} must use version {requirement}"
        );
        if let Some(default_features) = dependency_bool(item, "default-features") {
            ensure!(
                !default_features,
                "workspace dependency {key} can only disable default features"
            );
        }
        if package == "dear-imgui-test-engine-sys" {
            ensure!(
                dependency_bool(item, "default-features") == Some(false),
                "dear-imgui-test-engine-sys must disable default features in the root catalog"
            );
        }
        let fields = item
            .as_table_like()
            .with_context(|| format!("workspace dependency {key} must be a table"))?;
        for (field, _) in fields.iter() {
            let allowed = matches!(field, "path" | "version" | "default-features")
                || (key == "build-support" && field == "package");
            ensure!(
                allowed,
                "workspace dependency {key} has forbidden root field {field}; consumer features belong in member manifests"
            );
        }
        catalog.insert(
            key.to_owned(),
            CatalogEntry {
                package: package.to_owned(),
                directory,
            },
        );
    }
    ensure!(
        catalog.len() == publishable_by_directory.len(),
        "root release catalog must contain one entry per publishable package (expected {}, found {})",
        publishable_by_directory.len(),
        catalog.len()
    );
    for (directory, package) in &publishable_by_directory {
        ensure!(
            catalog_directories.contains(directory),
            "publishable package {package} is missing from the root release catalog"
        );
    }

    let package_names = catalog
        .values()
        .map(|entry| entry.package.as_str())
        .collect::<BTreeSet<_>>();
    let directories = catalog
        .values()
        .map(|entry| entry.directory.clone())
        .collect::<BTreeSet<_>>();
    let mut edge_count = 0;
    for member in &graph.members {
        edge_count += scan_manifest_dependencies(
            member.document.as_table(),
            &member.directory,
            &member.name,
            &catalog,
            &package_names,
            &directories,
        )?;
    }
    Ok(Validation {
        version,
        edge_count,
        publishable_count: publishable_by_directory.len(),
        private_count,
    })
}

fn scan_manifest_dependencies(
    table: &Table,
    member_directory: &Path,
    member_name: &str,
    catalog: &BTreeMap<String, CatalogEntry>,
    package_names: &BTreeSet<&str>,
    directories: &BTreeSet<PathBuf>,
) -> Result<usize> {
    let mut count = scan_dependency_sections(
        table,
        member_directory,
        member_name,
        catalog,
        package_names,
        directories,
    )?;
    let Some(targets) = table.get("target") else {
        return Ok(count);
    };
    let targets = targets
        .as_table()
        .with_context(|| format!("{member_name} has a non-table [target] section"))?;
    for (target, item) in targets.iter() {
        let target_table = item
            .as_table()
            .with_context(|| format!("{member_name} has a non-table [target.{target}] section"))?;
        count += scan_dependency_sections(
            target_table,
            member_directory,
            member_name,
            catalog,
            package_names,
            directories,
        )?;
    }
    Ok(count)
}

fn scan_dependency_sections(
    table: &Table,
    member_directory: &Path,
    member_name: &str,
    catalog: &BTreeMap<String, CatalogEntry>,
    package_names: &BTreeSet<&str>,
    directories: &BTreeSet<PathBuf>,
) -> Result<usize> {
    let mut count = 0;
    for section in DEPENDENCY_SECTIONS {
        let Some(item) = table.get(section) else {
            continue;
        };
        let dependencies = item
            .as_table()
            .with_context(|| format!("{member_name} has a non-table [{section}] section"))?;
        for (dependency_key, dependency) in dependencies.iter() {
            let declared_package =
                dependency_string(dependency, "package").unwrap_or(dependency_key);
            let path_is_internal = dependency_string(dependency, "path")
                .map(|path| fs::canonicalize(member_directory.join(path)))
                .transpose()
                .with_context(|| format!("canonicalize {member_name}:{dependency_key} path"))?
                .is_some_and(|path| directories.contains(&path));
            let internal = catalog.contains_key(dependency_key)
                || package_names.contains(declared_package)
                || path_is_internal;
            if internal {
                count += 1;
                ensure!(
                    catalog.contains_key(dependency_key),
                    "{member_name}:{dependency_key} bypasses its root catalog alias"
                );
                ensure!(
                    dependency_bool(dependency, "workspace") == Some(true),
                    "{member_name}:{dependency_key} must inherit the root workspace dependency"
                );
                let overlay = dependency.as_table_like().with_context(|| {
                    format!("{member_name}:{dependency_key} must use workspace = true")
                })?;
                for (field, _) in overlay.iter() {
                    ensure!(
                        matches!(field, "workspace" | "optional" | "features"),
                        "{member_name}:{dependency_key} may only overlay optional/features; found {field}"
                    );
                }
            }
        }
    }
    Ok(count)
}

fn update_root(document: &mut DocumentMut, version: &str, requirement: &str) -> Result<()> {
    let workspace = document
        .get_mut("workspace")
        .and_then(Item::as_table_mut)
        .context("root Cargo.toml is missing [workspace]")?;
    let package = workspace
        .get_mut("package")
        .and_then(Item::as_table_mut)
        .context("root Cargo.toml is missing [workspace.package]")?;
    replace_item_string(
        package
            .get_mut("version")
            .context("root Cargo.toml is missing workspace.package.version")?,
        version,
        "workspace.package.version",
    )?;
    let dependencies = workspace
        .get_mut("dependencies")
        .and_then(Item::as_table_mut)
        .context("root Cargo.toml is missing [workspace.dependencies]")?;
    for (key, dependency) in dependencies.iter_mut() {
        if dependency_string(dependency, "path").is_none() {
            continue;
        }
        let label = format!("workspace dependency {} version", key.get());
        if let Some(table) = dependency.as_inline_table_mut() {
            replace_value_string(
                table
                    .get_mut("version")
                    .with_context(|| format!("{label} is missing"))?,
                requirement,
                &label,
            )?;
        } else if let Some(table) = dependency.as_table_mut() {
            replace_item_string(
                table
                    .get_mut("version")
                    .with_context(|| format!("{label} is missing"))?,
                requirement,
                &label,
            )?;
        } else {
            bail!("{} must be a table", key.get());
        }
    }
    Ok(())
}

fn replace_item_string(item: &mut Item, replacement: &str, label: &str) -> Result<()> {
    let value = item
        .as_value_mut()
        .with_context(|| format!("{label} must be a string"))?;
    replace_value_string(value, replacement, label)
}

fn replace_value_string(value: &mut Value, replacement: &str, label: &str) -> Result<()> {
    let Value::String(existing) = value else {
        bail!("{label} must be a string");
    };
    if existing.value() != replacement {
        let decor = existing.decor().clone();
        let mut updated = Formatted::new(replacement.to_owned());
        *updated.decor_mut() = decor;
        *existing = updated;
    }
    Ok(())
}

fn apply(
    plan: &Plan,
    require_clean: bool,
    before_final_check: impl FnOnce() -> Result<()>,
) -> Result<()> {
    if require_clean {
        ensure_git_clean(&plan.graph)?;
    }
    let lock_directory = plan.graph.root.join("target");
    fs::create_dir_all(&lock_directory).context("create target directory for release lock")?;
    let lock_path = lock_directory.join("xtask-release-version.lock");
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .with_context(|| format!("open release lock {}", lock_path.display()))?;
    lock.try_lock()
        .context("another release-version process is active")?;
    verify_root_snapshot(plan)?;

    let parent = plan
        .graph
        .root_manifest
        .parent()
        .context("root Cargo.toml has no parent")?;
    let mut temporary =
        NamedTempFile::new_in(parent).context("create release manifest temp file")?;
    temporary
        .as_file()
        .set_permissions(fs::metadata(&plan.graph.root_manifest)?.permissions())?;
    temporary.write_all(plan.updated_root.as_bytes())?;
    temporary.flush()?;
    temporary.as_file().sync_all()?;
    before_final_check()?;
    verify_root_snapshot(plan)?;

    // The lock serializes this tool and the byte check preserves edits observed before persist.
    // It intentionally does not claim a universal CAS against non-cooperating external renames.
    temporary
        .persist(&plan.graph.root_manifest)
        .map_err(|error| {
            anyhow::anyhow!(
                "atomically replace {}: {}",
                plan.graph.root_manifest.display(),
                error.error
            )
        })?;
    Ok(())
}

fn verify_root_snapshot(plan: &Plan) -> Result<()> {
    ensure!(
        read_source(&plan.graph.root_manifest)? == plan.graph.root_source,
        "root Cargo.toml changed concurrently; preserving external edit"
    );
    Ok(())
}

fn ensure_git_clean(graph: &Graph) -> Result<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(&graph.root)
        .args(["status", "--porcelain=v1", "--untracked-files=all", "--"])
        .args(graph.git_paths())
        .output()
        .context("run git status for release graph manifests")?;
    validate_git_status(output.status.success(), &output.stdout, &output.stderr)
}

fn validate_git_status(success: bool, stdout: &[u8], stderr: &[u8]) -> Result<()> {
    ensure!(
        success,
        "git status failed while checking release manifests: {}",
        String::from_utf8_lossy(stderr).trim()
    );
    let dirty = String::from_utf8_lossy(stdout);
    ensure!(
        dirty.trim().is_empty(),
        "release graph manifests must be clean before version update:\n{}",
        dirty.trim_end()
    );
    Ok(())
}

fn release_policy(document: &DocumentMut, path: &Path) -> Result<ReleasePolicy> {
    let policy = document
        .get("workspace")
        .and_then(|item| item.get("metadata"))
        .and_then(|item| item.get("dear-imgui-release"))
        .and_then(Item::as_table)
        .with_context(|| {
            format!(
                "{} is missing [workspace.metadata.dear-imgui-release]",
                path.display()
            )
        })?;
    for (field, _) in policy.iter() {
        ensure!(
            matches!(field, "core-package" | "private-packages"),
            "release policy contains unknown field {field}"
        );
    }
    let core_package = policy
        .get("core-package")
        .and_then(Item::as_str)
        .filter(|name| !name.is_empty())
        .context("release policy core-package must be a non-empty string")?
        .to_owned();
    let private_table = policy
        .get("private-packages")
        .and_then(Item::as_table)
        .context("release policy private-packages must be a table")?;
    ensure!(
        !private_table.is_empty(),
        "release policy must declare at least one private package"
    );
    let mut private_packages = BTreeMap::new();
    for (name, item) in private_table.iter() {
        let fields = item
            .as_table_like()
            .with_context(|| format!("private package policy {name} must be a table"))?;
        for (field, _) in fields.iter() {
            ensure!(
                matches!(field, "path" | "version"),
                "private package policy {name} contains unknown field {field}"
            );
        }
        let directory = PathBuf::from(
            dependency_string(item, "path")
                .with_context(|| format!("private package policy {name} is missing path"))?,
        );
        ensure!(
            !directory.as_os_str().is_empty()
                && !directory.is_absolute()
                && !directory
                    .components()
                    .any(|component| component == std::path::Component::ParentDir),
            "private package policy {name} path must stay within the workspace"
        );
        let private_version = dependency_string(item, "version")
            .with_context(|| format!("private package policy {name} is missing version"))?;
        Version::parse(private_version)
            .with_context(|| format!("private package policy {name} has invalid version"))?;
        private_packages.insert(
            name.to_owned(),
            PrivatePackagePolicy {
                directory,
                version: private_version.to_owned(),
            },
        );
    }
    ensure!(
        !private_packages.contains_key(&core_package),
        "release core package {core_package} cannot be private"
    );
    Ok(ReleasePolicy {
        core_package,
        private_packages,
    })
}

fn workspace_member_paths(document: &DocumentMut) -> Result<Vec<PathBuf>> {
    document
        .get("workspace")
        .and_then(|item| item.get("members"))
        .and_then(Item::as_array)
        .context("root Cargo.toml is missing workspace.members")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(PathBuf::from)
                .context("workspace.members entries must be strings")
        })
        .collect()
}

fn workspace_version(document: &DocumentMut, path: &Path) -> Result<Version> {
    let raw = document
        .get("workspace")
        .and_then(|item| item.get("package"))
        .and_then(|item| item.get("version"))
        .and_then(Item::as_str)
        .with_context(|| format!("{} is missing workspace.package.version", path.display()))?;
    Version::parse(raw).with_context(|| format!("invalid workspace.package.version {raw:?}"))
}

fn release_requirement(version: &Version) -> Result<String> {
    ensure!(
        version.build.is_empty(),
        "release version {version} cannot contain build metadata because Cargo ignores it in dependency requirements"
    );
    Ok(if version.pre.is_empty() {
        format!("{}.{}", version.major, version.minor)
    } else {
        format!("={version}")
    })
}

fn package_table<'a>(document: &'a DocumentMut, path: &Path) -> Result<&'a Table> {
    document
        .get("package")
        .and_then(Item::as_table)
        .with_context(|| format!("{} is missing [package]", path.display()))
}

fn package_is_publishable(package: &Table) -> bool {
    !matches!(package.get("publish"), Some(item) if item.as_bool() == Some(false) || item.as_array().is_some_and(|array| array.is_empty()))
}

fn dependency_string<'a>(item: &'a Item, field: &str) -> Option<&'a str> {
    item.as_table()
        .and_then(|table| table.get(field))
        .and_then(Item::as_str)
        .or_else(|| {
            item.as_inline_table()
                .and_then(|table| table.get(field))
                .and_then(Value::as_str)
        })
}

fn dependency_bool(item: &Item, field: &str) -> Option<bool> {
    item.as_table()
        .and_then(|table| table.get(field))
        .and_then(Item::as_bool)
        .or_else(|| {
            item.as_inline_table()
                .and_then(|table| table.get(field))
                .and_then(Value::as_bool)
        })
}

fn read_source(path: &Path) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("read {}", path.display()))
}

fn parse_document(source: &str, path: &Path) -> Result<DocumentMut> {
    source
        .parse::<DocumentMut>()
        .with_context(|| format!("parse {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write as _;

    struct Fixture {
        directory: tempfile::TempDir,
        manifests: Vec<PathBuf>,
    }

    impl Fixture {
        fn new() -> Self {
            let directory = tempfile::tempdir().unwrap();
            let root = directory.path();
            let mut members = String::new();
            let mut catalog = String::new();
            let mut manifests = Vec::new();
            let mut packages = (0..25)
                .map(|index| format!("pkg-{index:02}"))
                .collect::<Vec<_>>();
            packages.push("dear-imgui-build-support".into());
            packages.push("dear-imgui-test-engine-sys".into());

            for (index, package) in packages.iter().enumerate() {
                let path = format!("members/p{index:02}");
                writeln!(members, "    \"{path}\",").unwrap();
                let key = if package == "dear-imgui-build-support" {
                    "build-support"
                } else {
                    package
                };
                let package_field = if key == package {
                    String::new()
                } else {
                    format!("package = \"{package}\", ")
                };
                let defaults = if package == "dear-imgui-test-engine-sys" {
                    ", default-features = false"
                } else {
                    ""
                };
                writeln!(
                    catalog,
                    "{key} = {{ {package_field}version = \"0.16\", path = \"{path}\"{defaults} }}"
                )
                .unwrap();
                let manifest = root.join(&path).join("Cargo.toml");
                fs::create_dir_all(manifest.parent().unwrap()).unwrap();
                let edges = if index == 0 {
                    "[dependencies]\npkg-01.workspace = true\n[dev-dependencies]\npkg-02.workspace = true\n[build-dependencies]\nbuild-support.workspace = true\n[target.'cfg(unix)'.dependencies]\npkg-03.workspace = true\n"
                } else {
                    ""
                };
                fs::write(
                    &manifest,
                    format!("[package]\nname = \"{package}\"\nversion.workspace = true\n{edges}"),
                )
                .unwrap();
                manifests.push(manifest);
            }
            for (path, package) in [
                ("examples", "dear-imgui-examples"),
                ("examples-wasm", "dear-imgui-web-demo"),
                ("xtask", "xtask"),
            ] {
                writeln!(members, "    \"{path}\",").unwrap();
                let manifest = root.join(path).join("Cargo.toml");
                fs::create_dir_all(manifest.parent().unwrap()).unwrap();
                fs::write(
                    &manifest,
                    format!(
                        "[package]\nname = \"{package}\"\nversion = \"0.1.0\"\npublish = false\n"
                    ),
                )
                .unwrap();
                manifests.push(manifest);
            }
            fs::write(
                root.join("Cargo.toml"),
                format!(
                    "[workspace]\nmembers = [\n{members}]\n\
[workspace.package]\nversion = \"0.16.0\" # release\n\
[workspace.metadata.dear-imgui-release]\ncore-package = \"pkg-00\"\n\
[workspace.metadata.dear-imgui-release.private-packages]\n\
dear-imgui-examples = {{ path = \"examples\", version = \"0.1.0\" }}\n\
dear-imgui-web-demo = {{ path = \"examples-wasm\", version = \"0.1.0\" }}\n\
xtask = {{ path = \"xtask\", version = \"0.1.0\" }}\n\
[workspace.dependencies]\n{catalog}"
                ),
            )
            .unwrap();
            Self {
                directory,
                manifests,
            }
        }

        fn root(&self) -> &Path {
            self.directory.path()
        }
        fn root_manifest(&self) -> PathBuf {
            self.root().join("Cargo.toml")
        }
    }

    #[test]
    fn parses_semver_and_options() {
        let options = Options::parse(&["0.17.0-alpha.1+ci.7".into(), "--dry-run".into()]).unwrap();
        assert!(options.dry_run);
        assert!(Options::parse(&["0.17".into()]).is_err());
        assert!(Options::parse(&["0.17.0-alpha..1".into()]).is_err());
        assert!(Options::parse(&["0.17.0".into(), "--unknown".into()]).is_err());
    }

    #[test]
    fn plans_cargo_compatible_stable_and_prerelease_requirements() {
        for (target, requirement) in [("0.17.0", "0.17"), ("0.17.0-alpha.1", "=0.17.0-alpha.1")] {
            let fixture = Fixture::new();
            let plan = Plan::build(fixture.root(), Version::parse(target).unwrap()).unwrap();
            assert!(
                plan.updated_root
                    .contains(&format!("version = \"{target}\" # release"))
            );
            assert_eq!(
                plan.updated_root
                    .matches(&format!("version = \"{requirement}\""))
                    .count(),
                27
            );
            assert!(
                semver::VersionReq::parse(&plan.requirement)
                    .unwrap()
                    .matches(&Version::parse(target).unwrap())
            );
        }
    }

    #[test]
    fn rejects_build_metadata_and_downgrades() {
        let fixture = Fixture::new();
        assert!(
            Plan::build(fixture.root(), Version::parse("0.17.0+build.7").unwrap())
                .unwrap_err()
                .to_string()
                .contains("Cargo ignores")
        );

        let fixture = Fixture::new();
        assert!(
            Plan::build(fixture.root(), Version::parse("0.15.0").unwrap())
                .unwrap_err()
                .to_string()
                .contains("must not be older")
        );
    }

    #[test]
    fn current_release_is_idempotent() {
        let fixture = Fixture::new();
        assert!(
            Plan::build(fixture.root(), Version::parse("0.16.0").unwrap())
                .unwrap()
                .is_idempotent()
        );
    }

    #[test]
    fn rejects_member_drift_and_catalog_omission() {
        let fixture = Fixture::new();
        let manifest = &fixture.manifests[0];
        fs::write(
            manifest,
            fs::read_to_string(manifest)
                .unwrap()
                .replace("version.workspace = true", "version = \"0.16.0\""),
        )
        .unwrap();
        assert!(
            Plan::build(fixture.root(), Version::parse("0.17.0").unwrap())
                .unwrap_err()
                .to_string()
                .contains("version.workspace")
        );

        let fixture = Fixture::new();
        let root = fixture.root_manifest();
        let source = fs::read_to_string(&root)
            .unwrap()
            .lines()
            .filter(|line| !line.starts_with("pkg-24 ="))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(root, source).unwrap();
        assert!(
            Plan::build(fixture.root(), Version::parse("0.17.0").unwrap())
                .unwrap_err()
                .to_string()
                .contains("one entry per publishable package")
        );
    }

    #[test]
    fn release_policy_owns_private_package_identity_path_and_version() {
        let fixture = Fixture::new();
        let root = fixture.root_manifest();
        let source = fs::read_to_string(&root)
            .unwrap()
            .replace("path = \"examples\"", "path = \"wrong-examples\"");
        fs::write(&root, source).unwrap();
        assert!(
            Plan::build(fixture.root(), Version::parse("0.17.0").unwrap())
                .unwrap_err()
                .to_string()
                .contains("must remain at wrong-examples")
        );

        let fixture = Fixture::new();
        let root = fixture.root_manifest();
        let source = fs::read_to_string(&root)
            .unwrap()
            .replace("xtask = { path = \"xtask\", version = \"0.1.0\" }", "");
        fs::write(&root, source).unwrap();
        assert!(
            Plan::build(fixture.root(), Version::parse("0.17.0").unwrap())
                .unwrap_err()
                .to_string()
                .contains("unexpected non-publishable package xtask")
        );
    }

    #[test]
    fn rejects_internal_edge_that_bypasses_workspace() {
        let fixture = Fixture::new();
        let manifest = &fixture.manifests[0];
        let source = fs::read_to_string(manifest).unwrap().replace(
            "pkg-01.workspace = true",
            "pkg-01 = { path = \"../p01\", version = \"0.16\" }",
        );
        fs::write(manifest, source).unwrap();
        assert!(
            Plan::build(fixture.root(), Version::parse("0.17.0").unwrap())
                .unwrap_err()
                .to_string()
                .contains("must inherit")
        );

        let fixture = Fixture::new();
        let manifest = &fixture.manifests[0];
        let source = fs::read_to_string(manifest).unwrap().replace(
            "pkg-03.workspace = true",
            "pkg-03 = { path = \"../p03\", version = \"0.16\" }",
        );
        fs::write(manifest, source).unwrap();
        assert!(
            Plan::build(fixture.root(), Version::parse("0.17.0").unwrap())
                .unwrap_err()
                .to_string()
                .contains("must inherit")
        );
    }

    #[test]
    fn ignores_dependency_shaped_package_metadata() {
        let fixture = Fixture::new();
        let manifest = &fixture.manifests[0];
        let mut source = fs::read_to_string(manifest).unwrap();
        source.push_str("[package.metadata.fixture.dependencies]\npkg-01 = \"not-a-cargo-edge\"\n");
        fs::write(manifest, source).unwrap();

        let plan = Plan::build(fixture.root(), Version::parse("0.17.0").unwrap()).unwrap();
        assert_eq!(plan.edge_count, 4);
    }

    #[test]
    fn rejects_consumer_features_in_root_catalog() {
        let fixture = Fixture::new();
        let root = fixture.root_manifest();
        let source = fs::read_to_string(&root).unwrap().replace(
            "pkg-01 = { version = \"0.16\", path = \"members/p01\" }",
            "pkg-01 = { version = \"0.16\", path = \"members/p01\", features = [\"leak\"] }",
        );
        fs::write(root, source).unwrap();
        assert!(
            Plan::build(fixture.root(), Version::parse("0.17.0").unwrap())
                .unwrap_err()
                .to_string()
                .contains("forbidden root field")
        );
    }

    #[test]
    fn allows_disabling_default_features_in_root_catalog() {
        let fixture = Fixture::new();
        let root = fixture.root_manifest();
        let source = fs::read_to_string(&root).unwrap().replace(
            "pkg-01 = { version = \"0.16\", path = \"members/p01\" }",
            "pkg-01 = { version = \"0.16\", path = \"members/p01\", default-features = false }",
        );
        fs::write(root, source).unwrap();

        Plan::build(fixture.root(), Version::parse("0.17.0").unwrap()).unwrap();
    }

    #[test]
    fn rejects_enabling_default_features_in_root_catalog() {
        let fixture = Fixture::new();
        let root = fixture.root_manifest();
        let source = fs::read_to_string(&root).unwrap().replace(
            "pkg-01 = { version = \"0.16\", path = \"members/p01\" }",
            "pkg-01 = { version = \"0.16\", path = \"members/p01\", default-features = true }",
        );
        fs::write(root, source).unwrap();
        assert!(
            Plan::build(fixture.root(), Version::parse("0.17.0").unwrap())
                .unwrap_err()
                .to_string()
                .contains("can only disable default features")
        );
    }

    #[test]
    fn apply_changes_only_root_and_rejects_concurrent_root_edit() {
        let fixture = Fixture::new();
        let member_sources = fixture
            .manifests
            .iter()
            .map(fs::read)
            .collect::<std::io::Result<Vec<_>>>()
            .unwrap();
        let plan = Plan::build(fixture.root(), Version::parse("0.17.0").unwrap()).unwrap();
        apply(&plan, false, || Ok(())).unwrap();
        for (path, expected) in fixture.manifests.iter().zip(member_sources) {
            assert_eq!(fs::read(path).unwrap(), expected);
        }

        let fixture = Fixture::new();
        let root = fixture.root_manifest();
        let plan = Plan::build(fixture.root(), Version::parse("0.17.0").unwrap()).unwrap();
        let error = apply(&plan, false, || {
            fs::write(
                &root,
                format!("{}\n# concurrent edit\n", fs::read_to_string(&root)?),
            )?;
            Ok(())
        })
        .unwrap_err();
        assert!(error.to_string().contains("changed concurrently"));
        assert!(
            fs::read_to_string(root)
                .unwrap()
                .contains("concurrent edit")
        );
    }

    #[test]
    fn dirty_git_status_is_fail_closed() {
        assert!(
            validate_git_status(true, b" M Cargo.toml\n", b"")
                .unwrap_err()
                .to_string()
                .contains("must be clean")
        );
        assert!(validate_git_status(false, b"", b"fatal").is_err());
    }
}
