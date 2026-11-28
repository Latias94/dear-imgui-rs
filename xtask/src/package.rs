use anyhow::{Context, Result, bail, ensure};
use build_support::binding::{
    ExtensionBinding, RELEASE_CANDIDATE_SHA_ENV, RELEASE_CORE_ARTIFACT_IDENTITY_HASH_ENV,
};
use build_support::native_build::package_crate_env;
use flate2::{Compression, GzBuilder};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const CORE: &str = "dear-imgui-sys";
const EXTENSIONS: [ExtensionBinding; 7] = [
    ExtensionBinding::ImPlot,
    ExtensionBinding::ImPlot3d,
    ExtensionBinding::ImNodes,
    ExtensionBinding::NodeEditor,
    ExtensionBinding::ImGuizmo,
    ExtensionBinding::ImGuizmoQuat,
    ExtensionBinding::Cte,
];

struct Options<'a> {
    selection: &'a str,
    features: Vec<&'a str>,
    target: Option<&'a str>,
    crt: Option<&'a str>,
    out_dir: PathBuf,
}

impl<'a> Options<'a> {
    fn parse(root: &Path, args: &'a [String]) -> Result<Self> {
        let mut args = args.iter().map(String::as_str);
        let selection = args
            .next()
            .context("package requires a sys crate or --all")?;
        let mut options = Self {
            selection,
            features: Vec::new(),
            target: None,
            crt: None,
            out_dir: root.join("target/prebuilt"),
        };
        while let Some(option) = args.next() {
            let value = args
                .next()
                .with_context(|| format!("{option} requires a value"))?;
            match option {
                "--features" => options
                    .features
                    .extend(value.split(',').filter(|s| !s.is_empty())),
                "--target" => options.target = Some(value),
                "--crt" => options.crt = Some(value),
                "--out-dir" => options.out_dir = std::path::absolute(value)?,
                _ => bail!("unknown package option {option:?}"),
            }
        }
        options.features.sort_unstable();
        options.features.dedup();
        if let Some(crt) = options.crt {
            ensure!(
                matches!(crt, "md" | "mt"),
                "expected CRT md or mt, found {crt:?}"
            );
        }
        Ok(options)
    }
}

pub(super) fn run(root: &Path, args: &[String]) -> Result<()> {
    let options = Options::parse(root, args)?;
    let all = options.selection == "--all";
    let extension = EXTENSIONS
        .into_iter()
        .find(|extension| extension.artifact_spec().sys_crate_name == options.selection);
    ensure!(
        all || options.selection == CORE || extension.is_some(),
        "{} has no native prebuilt package route",
        options.selection
    );
    for feature in &options.features {
        match *feature {
            "freetype" | "stack-layout" => {}
            "test-engine" => bail!(
                "feature `test-engine` is source-only and cannot be packaged as a prebuilt core artifact"
            ),
            "abi-probe" => bail!(
                "feature `abi-probe` is CI-only and cannot be packaged as a prebuilt core artifact"
            ),
            _ => bail!("{feature:?} is not a native package feature"),
        }
    }
    if let Some(extension) = extension {
        ensure!(
            supports_features(extension, &options.features),
            "{} cannot be packaged with {:?}",
            options.selection,
            options.features,
        );
    }

    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let metadata = Command::new(&cargo)
        .current_dir(root)
        .args(["metadata", "--format-version=1", "--no-deps", "--locked"])
        .stderr(Stdio::inherit())
        .output()?;
    ensure!(metadata.status.success(), "cargo metadata failed");
    let metadata: Value = serde_json::from_slice(&metadata.stdout)?;
    let candidate = match std::env::var_os(RELEASE_CANDIDATE_SHA_ENV) {
        Some(value) => value,
        None => {
            let output = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(root)
                .output()?;
            ensure!(
                output.status.success(),
                "set {RELEASE_CANDIDATE_SHA_ENV} to the source commit SHA"
            );
            String::from_utf8(output.stdout)?.trim().into()
        }
    };
    let builder = NativeBuilder {
        root,
        options: &options,
        cargo: &cargo,
        candidate: &candidate,
        metadata: &metadata,
    };
    fs::create_dir_all(&options.out_dir)?;
    let staging = tempfile::tempdir_in(&options.out_dir)?;
    let mut core_identity = std::env::var_os(RELEASE_CORE_ARTIFACT_IDENTITY_HASH_ENV);
    let mut archives = Vec::new();
    if extension.is_none() || core_identity.is_none() {
        let core = builder.build(CORE, "dear_imgui", None)?;
        core_identity = Some(
            fs::read_to_string(core.out_dir.join("prebuilt-artifact-identity.txt"))?
                .trim()
                .into(),
        );
        if extension.is_none() {
            archives.push(core.write(root, staging.path())?);
        }
    }
    let extensions = EXTENSIONS.into_iter().filter(|&item| {
        extension == Some(item) || all && supports_features(item, &options.features)
    });
    for extension in extensions {
        let spec = extension.artifact_spec();
        let package = builder.build(
            spec.sys_crate_name,
            spec.library_name,
            core_identity.as_deref(),
        )?;
        archives.push(package.write(root, staging.path())?);
    }

    let output_lock = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(options.out_dir.join(".package.lock"))?;
    output_lock.lock()?;
    for (archive, name) in archives {
        let destination = options.out_dir.join(name);
        archive.persist(&destination)?;
        println!("{}", destination.display());
    }
    write_checksums(&options.out_dir)
}

fn supports_features(extension: ExtensionBinding, features: &[&str]) -> bool {
    features.iter().all(|&feature| match feature {
        "stack-layout" => extension == ExtensionBinding::NodeEditor,
        "freetype" => !matches!(
            extension,
            ExtensionBinding::Cte | ExtensionBinding::ImPlot3d
        ),
        _ => false,
    })
}

struct Package {
    crate_name: &'static str,
    out_dir: PathBuf,
    archive_name: String,
    library_name: String,
}

struct NativeBuilder<'a> {
    root: &'a Path,
    options: &'a Options<'a>,
    cargo: &'a OsStr,
    candidate: &'a OsStr,
    metadata: &'a Value,
}

impl NativeBuilder<'_> {
    fn build(
        &self,
        crate_name: &'static str,
        library_name: &str,
        core_identity: Option<&OsStr>,
    ) -> Result<Package> {
        let package_id = self.metadata["packages"]
            .as_array()
            .and_then(|packages| {
                packages
                    .iter()
                    .find(|package| package["name"] == crate_name)
            })
            .and_then(|package| package["id"].as_str())
            .with_context(|| format!("{crate_name} is missing from cargo metadata"))?;

        let mut build = Command::new(self.cargo);
        build
            .current_dir(self.root)
            .args([
                "build",
                "--release",
                "--locked",
                "--message-format=json-render-diagnostics",
            ])
            .args(["--package", crate_name, "--no-default-features"])
            .env(package_crate_env(crate_name), "1")
            .env(RELEASE_CANDIDATE_SHA_ENV, self.candidate)
            .stderr(Stdio::inherit());
        if let Some(identity) = core_identity {
            build.env(RELEASE_CORE_ARTIFACT_IDENTITY_HASH_ENV, identity);
        }
        if let Some(target) = self.options.target {
            build.args(["--target", target]);
        }
        if !self.options.features.is_empty() {
            build.arg("--features").arg(self.options.features.join(","));
        }
        let output = build
            .output()
            .with_context(|| format!("build {crate_name}"))?;
        ensure!(output.status.success(), "{crate_name} source build failed");

        // Use Cargo's exact build-script result for this package and feature set.
        let mut out_dir = None;
        for line in output
            .stdout
            .split(|&byte| byte == b'\n')
            .filter(|line| !line.is_empty())
        {
            let message: Value =
                serde_json::from_slice(line).context("read Cargo build message")?;
            if message["reason"] == "build-script-executed" && message["package_id"] == package_id {
                ensure!(
                    out_dir.is_none(),
                    "Cargo reported multiple build outputs for {crate_name}"
                );
                out_dir = message["out_dir"].as_str().map(PathBuf::from);
            }
        }
        let out_dir = out_dir.with_context(|| {
            format!("Cargo did not report an output directory for {crate_name}")
        })?;
        let archive_name = fs::read_to_string(out_dir.join("prebuilt-archive-name.txt"))?
            .trim()
            .to_owned();
        ensure!(
            archive_name.ends_with(".tar.gz") && !archive_name.contains(['/', '\\', ':']),
            "invalid archive name {archive_name:?}"
        );
        let manifest = fs::read_to_string(out_dir.join("prebuilt-manifest.txt"))?;
        let read_manifest = |key: &str| {
            manifest.lines().find_map(|line| {
                let (name, value) = line.split_once('=')?;
                (name == key).then_some(value)
            })
        };
        let target = read_manifest("target").context("package manifest is missing its target")?;
        if let Some(crt) = self.options.crt {
            ensure!(
                read_manifest("crt") == Some(crt),
                "{crate_name} was built for the wrong CRT profile (expected {crt}); \
                 check Cargo's `crt-static` target feature"
            );
        }
        let target_env = if target.ends_with("-msvc") {
            "msvc"
        } else {
            ""
        };
        Ok(Package {
            crate_name,
            out_dir,
            archive_name,
            library_name: build_support::expected_lib_name(target_env, library_name),
        })
    }
}

#[derive(Default)]
struct ArchiveFiles(BTreeMap<String, PathBuf>);

impl ArchiveFiles {
    fn add(&mut self, source: PathBuf, destination: impl Into<String>) {
        self.0.insert(destination.into(), source);
    }

    fn add_optional(&mut self, source: PathBuf, destination: &str) {
        if source.is_file() {
            self.add(source, destination);
        }
    }

    fn tree(&mut self, source: &Path, destination: &str, headers_only: bool) -> io::Result<()> {
        const EXCLUDED: &[&str] = &[
            ".github",
            "backends",
            "basic_examples",
            "bin",
            "docs",
            "example",
            "examples",
            "external",
            "generator",
            "img",
            "Images",
            "misc",
            "old_examples",
            "vcpkg",
        ];
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let name = entry.file_name().into_string().map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "non-UTF-8 archive input name")
            })?;
            let kind = entry.file_type()?;
            let path = format!("{destination}/{name}");
            if kind.is_dir() && (!headers_only || !EXCLUDED.contains(&name.as_str())) {
                self.tree(&entry.path(), &path, headers_only)?;
            } else if kind.is_file() && (!headers_only || name.to_ascii_lowercase().ends_with(".h"))
            {
                self.add(entry.path(), path);
            }
        }
        Ok(())
    }

    fn write(&self, output: impl Write) -> Result<()> {
        let encoder = GzBuilder::new().mtime(0).write(output, Compression::best());
        let mut archive = tar::Builder::new(encoder);
        for (path, source) in &self.0 {
            let mut file =
                File::open(source).with_context(|| format!("open {}", source.display()))?;
            let mut header = tar::Header::new_gnu();
            header.set_mode(0o644);
            header.set_mtime(0);
            header.set_uid(0);
            header.set_gid(0);
            header.set_size(file.metadata()?.len());
            header.set_cksum();
            if path.starts_with("lib/") && path.ends_with(".lib") {
                let mut library = Vec::new();
                file.read_to_end(&mut library)?;
                normalize_coff_archive(&mut library)
                    .with_context(|| format!("normalize {}", source.display()))?;
                archive.append_data(&mut header, path, library.as_slice())?;
            } else {
                archive.append_data(&mut header, path, &mut file)?;
            }
        }
        archive.into_inner()?.finish()?;
        Ok(())
    }
}

fn normalize_coff_archive(library: &mut [u8]) -> Result<()> {
    // MSVC LIB records build times in the archive headers even when the compiler emits
    // reproducible objects. Preserve the member bytes and offsets used by the linker index.
    const MAGIC: &[u8] = b"!<arch>\n";
    ensure!(library.starts_with(MAGIC), "expected a COFF static library");
    let mut members = &mut library[MAGIC.len()..];
    while !members.is_empty() {
        let (header, contents) = members
            .split_at_mut_checked(60)
            .context("truncated COFF archive header")?;
        ensure!(&header[58..60] == b"`\n", "invalid COFF archive header");
        let size = std::str::from_utf8(&header[48..58])?
            .trim()
            .parse::<usize>()?;
        ensure!(size <= contents.len(), "truncated COFF archive member");
        members = contents
            .get_mut(size + size % 2..)
            .context("missing COFF archive member padding")?;
        header[16..28].copy_from_slice(b"0           ");
    }
    Ok(())
}

impl Package {
    fn write(&self, root: &Path, output: &Path) -> Result<(tempfile::NamedTempFile, String)> {
        let source = super::maintained_source(self.crate_name)?;
        let crate_root = root.join(&source.crate_root);
        let vendor = source.resolve_source_root(&crate_root)?;
        let mut files = ArchiveFiles::default();
        files.add(root.join("LICENSE-MIT"), "licenses/PROJECT-LICENSE-MIT");
        files.add(
            root.join("LICENSE-APACHE"),
            "licenses/PROJECT-LICENSE-APACHE",
        );
        files.add(
            self.out_dir.join(&self.library_name),
            format!("lib/{}", self.library_name),
        );
        files.add(self.out_dir.join("prebuilt-manifest.txt"), "manifest.txt");
        add_native_sources(&mut files, self.crate_name, &crate_root, &vendor)?;
        let mut staging = tempfile::NamedTempFile::new_in(output)?;
        files.write(staging.as_file_mut())?;
        Ok((staging, self.archive_name.clone()))
    }
}

fn add_native_sources(
    files: &mut ArchiveFiles,
    crate_name: &str,
    crate_root: &Path,
    vendor: &Path,
) -> Result<()> {
    let (wrapper, upstream, upstream_license) = match crate_name {
        CORE => ("cimgui", "imgui", "LICENSE.txt"),
        "dear-implot-sys" => ("cimplot", "implot", "LICENSE"),
        "dear-implot3d-sys" => ("cimplot3d", "implot3d", "LICENSE"),
        "dear-imnodes-sys" => ("cimnodes", "imnodes", "LICENSE.md"),
        "dear-node-editor-sys" => ("cimnodes_editor", "imgui-node-editor", "LICENSE"),
        "dear-imguizmo-sys" => ("cimguizmo", "ImGuizmo", "LICENSE"),
        "dear-imguizmo-quat-sys" => ("cimguizmo_quat", "imGuIZMO.quat", "license.txt"),
        "dear-imgui-cte-sys" => ("cimCTE", "ImGuiColorTextEdit", "LICENSE"),
        _ => bail!("unknown package source layout {crate_name}"),
    };
    files.add(
        vendor.join(format!("{wrapper}.h")),
        format!("include/{wrapper}/{wrapper}.h"),
    );
    let upstream_root = vendor.join(upstream);
    let wrapper_license = match wrapper {
        "cimguizmo_quat" => "license.txt",
        "cimCTE" => "README.md",
        _ => "LICENSE",
    };
    let wrapper_destination = if wrapper_license == "README.md" {
        "README.md"
    } else {
        "LICENSE"
    };
    files.add_optional(
        vendor.join(wrapper_license),
        &format!("licenses/{wrapper}-{wrapper_destination}"),
    );
    files.add_optional(
        upstream_root.join(upstream_license),
        &format!(
            "licenses/{upstream}-{}",
            if upstream_license == "license.txt" {
                "LICENSE"
            } else {
                upstream_license
            }
        ),
    );

    let shim = match wrapper {
        "cimnodes" => Some(("imnodes_extra.h", "imnodes")),
        "cimnodes_editor" => Some(("node_editor_extra.h", "dear-node-editor")),
        "cimguizmo" => Some(("imguizmo_extra.h", "cimguizmo")),
        "cimCTE" => Some(("cte_bridge.h", "cimCTE")),
        _ => None,
    };
    if let Some((file, directory)) = shim {
        files.add(
            crate_root.join("shim").join(file),
            format!("include/{directory}/{file}"),
        );
    }
    match wrapper {
        "cimCTE" => {
            for header in [
                "dtl.h",
                "TextDiff.h",
                "TextEditor.h",
                "TextEditorLanguages.h",
                "example/dejavu.h",
                "example/notosans.h",
                "extras/TrieAutoComplete.h",
            ] {
                files.add(
                    upstream_root.join(header),
                    format!("include/{upstream}/{header}"),
                );
            }
            files.tree(
                &upstream_root.join("languages"),
                &format!("include/{upstream}/languages"),
                false,
            )?;
            for font in ["dejavu.lic", "notosans.lic"] {
                files.add(
                    upstream_root.join("example").join(font),
                    format!("licenses/{font}"),
                );
            }
            files.add_optional(
                upstream_root.join("README.md"),
                "licenses/ImGuiColorTextEdit-README.md",
            );
        }
        "cimplot3d" => {
            for header in ["implot3d.h", "implot3d_internal.h"] {
                files.add(
                    upstream_root.join(header),
                    format!("include/{upstream}/{header}"),
                );
            }
        }
        "cimguizmo_quat" => files.tree(
            &upstream_root.join("imguizmo_quat"),
            "include/imguizmo_quat",
            true,
        )?,
        _ => files.tree(&upstream_root, &format!("include/{upstream}"), true)?,
    }
    if crate_name == CORE {
        files.add(
            crate_root.join("THIRD_PARTY_NOTICES.md"),
            "licenses/dear-imgui-sys-THIRD_PARTY_NOTICES.md",
        );
    }
    Ok(())
}

fn write_checksums(output: &Path) -> Result<()> {
    let mut names = Vec::new();
    for entry in fs::read_dir(output)? {
        let entry = entry?;
        if let Ok(name) = entry.file_name().into_string()
            && name.ends_with(".tar.gz")
            && entry.file_type()?.is_file()
        {
            names.push(name);
        }
    }
    names.sort_unstable();
    let mut staging = tempfile::NamedTempFile::new_in(output)?;
    for name in names {
        let mut digest = Sha256::new();
        io::copy(&mut File::open(output.join(&name))?, &mut digest)?;
        writeln!(staging, "{:x}  {name}", digest.finalize())?;
    }
    staging.persist(output.join("SHA256SUMS"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coff_normalization_preserves_payload_and_index_offsets() -> Result<()> {
        let mut library = b"!<arch>\n".to_vec();
        for (name, payload) in [("/", b"index".as_slice()), ("main.o/", b"object")] {
            writeln!(
                library,
                "{name:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`",
                1_795_850_420,
                0,
                0,
                "644",
                payload.len(),
            )?;
            library.extend(payload);
            if payload.len() % 2 != 0 {
                library.push(b'\n');
            }
        }
        let mut expected = library.clone();
        for offset in [8, 74] {
            expected[offset + 16..offset + 28].copy_from_slice(b"0           ");
        }
        normalize_coff_archive(&mut library)?;
        assert_eq!(library, expected);
        assert!(normalize_coff_archive(&mut library[..80]).is_err());
        Ok(())
    }

    #[test]
    fn archives_have_deterministic_bytes_and_normalized_members() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        fs::write(&a, b"alpha")?;
        fs::write(&b, b"beta")?;
        let mut files = ArchiveFiles::default();
        files.add(b.clone(), "lib/b");
        files.add(a.clone(), "include/a");
        let mut first = Vec::new();
        files.write(&mut first)?;
        let mut reversed = ArchiveFiles::default();
        reversed.add(a, "include/a");
        reversed.add(b, "lib/b");
        let mut second = Vec::new();
        reversed.write(&mut second)?;
        assert_eq!(first, second);

        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(first.as_slice()));
        let mut paths = Vec::new();
        for entry in archive.entries()? {
            let entry = entry?;
            assert_eq!(entry.header().mtime()?, 0);
            assert_eq!(entry.header().mode()?, 0o644);
            paths.push(entry.path()?.into_owned());
        }
        assert_eq!(paths, [PathBuf::from("include/a"), PathBuf::from("lib/b")]);
        fs::write(dir.path().join("sample.tar.gz"), &first)?;
        write_checksums(dir.path())?;
        assert_eq!(
            fs::read_to_string(dir.path().join("SHA256SUMS"))?,
            format!("{:x}  sample.tar.gz\n", Sha256::digest(first))
        );
        Ok(())
    }
}
