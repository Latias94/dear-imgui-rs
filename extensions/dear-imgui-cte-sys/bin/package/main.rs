use std::{
    env, fs,
    path::{Path, PathBuf},
};

use flate2::{Compression, write::GzEncoder};

fn expected_lib_name() -> &'static str {
    if cfg!(target_env = "msvc") {
        "dear_imgui_cte.lib"
    } else {
        "libdear_imgui_cte.a"
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let target = env!("DEAR_IMGUI_EXTENSION_ARTIFACT_TARGET");
    let crt = env!("DEAR_IMGUI_EXTENSION_ARTIFACT_CRT");
    if let Ok(requested) = env::var("CTE_SYS_PKG_CRT")
        && !requested.is_empty()
        && requested != crt
    {
        return Err(format!(
            "CTE_SYS_PKG_CRT declares {requested}, but the package binary was built for {crt}"
        )
        .into());
    }

    let package_dir = env::var("CTE_SYS_PACKAGE_DIR")
        .or_else(|_| env::var("IMGUI_SYS_PACKAGE_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("OUT_DIR")));
    fs::create_dir_all(&package_dir)?;
    let archive_name = include_str!(concat!(env!("OUT_DIR"), "/prebuilt-archive-name.txt")).trim();
    println!(
        "Packaging dear-imgui-cte for {target} into {}",
        package_dir.display()
    );

    let file = fs::File::create(package_dir.join(archive_name))?;
    let encoder = GzEncoder::new(file, Compression::best());
    let mut archive = tar::Builder::new(encoder);
    let source_root = manifest_dir.join("third-party/cimCTE");

    append_file(
        &mut archive,
        &source_root.join("cimCTE.h"),
        "include/cimCTE/cimCTE.h",
    )?;
    append_file(
        &mut archive,
        &manifest_dir.join("shim/cte_bridge.h"),
        "include/cimCTE/cte_bridge.h",
    )?;
    let text_editor_root = source_root.join("ImGuiColorTextEdit");
    for relative in [
        "dtl.h",
        "TextDiff.h",
        "TextEditor.h",
        "example/dejavu.h",
        "extras/Notifications.h",
        "extras/TrieAutoComplete.h",
    ] {
        append_file(
            &mut archive,
            &text_editor_root.join(relative),
            &format!("include/ImGuiColorTextEdit/{relative}"),
        )?;
    }

    let workspace_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .ok_or("dear-imgui-cte-sys must be inside the workspace")?;
    append_file_if_exists(
        &mut archive,
        &workspace_root.join("LICENSE-MIT"),
        "licenses/PROJECT-LICENSE-MIT",
    )?;
    append_file_if_exists(
        &mut archive,
        &workspace_root.join("LICENSE-APACHE"),
        "licenses/PROJECT-LICENSE-APACHE",
    )?;
    append_file_if_exists(
        &mut archive,
        &source_root.join("README.md"),
        "licenses/cimCTE-README.md",
    )?;
    append_file_if_exists(
        &mut archive,
        &source_root.join("ImGuiColorTextEdit/LICENSE"),
        "licenses/ImGuiColorTextEdit-LICENSE",
    )?;
    append_file_if_exists(
        &mut archive,
        &source_root.join("ImGuiColorTextEdit/README.md"),
        "licenses/ImGuiColorTextEdit-README.md",
    )?;

    let library = PathBuf::from(env!("OUT_DIR")).join(expected_lib_name());
    if !library.exists() {
        return Err(format!("static library not found at {}", library.display()).into());
    }
    append_file(
        &mut archive,
        &library,
        &format!("lib/{}", expected_lib_name()),
    )?;

    let manifest = include_bytes!(concat!(env!("OUT_DIR"), "/prebuilt-manifest.txt"));
    let mut header = tar::Header::new_gnu();
    header.set_size(manifest.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    archive.append_data(&mut header, "manifest.txt", manifest.as_slice())?;
    archive.finish()?;
    println!(
        "Package created: {}",
        package_dir.join(archive_name).display()
    );
    Ok(())
}

fn append_file_if_exists(
    archive: &mut tar::Builder<GzEncoder<fs::File>>,
    source: &Path,
    destination: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if source.exists() {
        append_file(archive, source, destination)?;
    }
    Ok(())
}

fn append_file(
    archive: &mut tar::Builder<GzEncoder<fs::File>>,
    source: &Path,
    destination: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = fs::File::open(source)?;
    archive.append_file(destination, &mut file)?;
    Ok(())
}
