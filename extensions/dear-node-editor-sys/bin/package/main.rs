use flate2::{Compression, write::GzEncoder};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn expected_lib_name() -> &'static str {
    if cfg!(target_env = "msvc") {
        "dear_node_editor.lib"
    } else {
        "libdear_node_editor.a"
    }
}

fn sys_out_dir() -> PathBuf {
    PathBuf::from(env!("OUT_DIR"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir.parent().and_then(|p| p.parent()).unwrap();
    let target = env!("DEAR_IMGUI_EXTENSION_ARTIFACT_TARGET");
    let crate_version = env!("CARGO_PKG_VERSION");
    let crt = env!("DEAR_IMGUI_EXTENSION_ARTIFACT_CRT");
    if let Ok(v) = env::var("NODE_EDITOR_SYS_PKG_CRT")
        && !v.is_empty()
        && v != crt
    {
        return Err(format!(
            "NODE_EDITOR_SYS_PKG_CRT declares {v}, but this package binary was built for CRT profile {crt}"
        )
        .into());
    }

    let link_type = "static";

    let pkg_dir = env::var("NODE_EDITOR_SYS_PACKAGE_DIR")
        .or_else(|_| env::var("IMGUI_SYS_PACKAGE_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env::var("OUT_DIR").unwrap()));
    fs::create_dir_all(&pkg_dir)?;

    let ar_name = include_str!(concat!(env!("OUT_DIR"), "/prebuilt-archive-name.txt")).trim();

    println!("Packaging dear-node-editor prebuilt:");
    println!("  Target: {}", target);
    println!("  Version: {}", crate_version);
    println!("  Link type: {}", link_type);
    if !crt.is_empty() {
        println!("  CRT: {}", crt);
    }
    println!("  Package dir: {}", pkg_dir.display());

    let sys_out = sys_out_dir();
    println!("Using sys build out dir: {}", sys_out.display());

    let file = fs::File::create(pkg_dir.join(&ar_name))?;
    let enc = GzEncoder::new(file, Compression::best());
    let mut tar = tar::Builder::new(enc);

    let cimnodes_editor_root = manifest_dir.join("third-party").join("cimnodes_editor");
    let node_editor_include = cimnodes_editor_root.join("imgui-node-editor");
    append_headers_only(
        &mut tar,
        &node_editor_include,
        "include/imgui-node-editor",
        &[".github", "docs", "examples", "external", "misc"],
    )?;

    append_file_if_exists(
        &mut tar,
        &cimnodes_editor_root.join("cimnodes_editor.h"),
        "include/cimnodes_editor/cimnodes_editor.h",
    )?;
    append_file_if_exists(
        &mut tar,
        &manifest_dir.join("shim").join("node_editor_extra.h"),
        "include/dear-node-editor/node_editor_extra.h",
    )?;

    append_license_if_exists(
        &mut tar,
        &workspace_root.join("LICENSE-MIT"),
        "licenses/PROJECT-LICENSE-MIT",
    )?;
    append_license_if_exists(
        &mut tar,
        &workspace_root.join("LICENSE-APACHE"),
        "licenses/PROJECT-LICENSE-APACHE",
    )?;
    append_license_if_exists(
        &mut tar,
        &node_editor_include.join("LICENSE"),
        "licenses/imgui-node-editor-LICENSE",
    )?;

    let lib_name = expected_lib_name();
    let lib_path = sys_out.join(lib_name);
    if !lib_path.exists() {
        return Err(format!("Static library not found at {}", lib_path.display()).into());
    }
    let mut f = fs::File::open(&lib_path)?;
    tar.append_file(format!("lib/{}", lib_name), &mut f)?;
    println!("Added lib: {}", lib_path.display());

    let manifest_txt = include_bytes!(concat!(env!("OUT_DIR"), "/prebuilt-manifest.txt"));
    let mut hdr = tar::Header::new_gnu();
    hdr.set_size(manifest_txt.len() as u64);
    hdr.set_mode(0o644);
    hdr.set_cksum();
    tar.append_data(&mut hdr, "manifest.txt", manifest_txt.as_slice())?;

    tar.finish()?;
    println!("Package created: {}", pkg_dir.join(&ar_name).display());
    Ok(())
}

fn append_headers_only(
    tar: &mut tar::Builder<GzEncoder<fs::File>>,
    src_dir: &Path,
    dst_root: &str,
    exclude_dirs: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    if !src_dir.exists() {
        eprintln!("WARN: header dir not found: {}", src_dir.display());
        return Ok(());
    }

    let mut stack = vec![src_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let p = entry.path();
            let rel = p.strip_prefix(src_dir).unwrap();
            if excluded(rel, exclude_dirs) {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
            } else if p
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("h") || s.eq_ignore_ascii_case("inl"))
                .unwrap_or(false)
            {
                append_file_if_exists(tar, &p, &format!("{}/{}", dst_root, rel.display()))?;
            }
        }
    }
    Ok(())
}

fn excluded(path: &Path, exclude_dirs: &[&str]) -> bool {
    path.components().any(|comp| {
        if let std::path::Component::Normal(os) = comp {
            os.to_str()
                .is_some_and(|name| exclude_dirs.iter().any(|e| e == &name))
        } else {
            false
        }
    })
}

fn append_file_if_exists(
    tar: &mut tar::Builder<GzEncoder<fs::File>>,
    src: &Path,
    dst: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if src.exists() {
        let mut f = fs::File::open(src)?;
        tar.append_file(dst, &mut f)?;
        println!("Added file: {} => {}", src.display(), dst);
    } else {
        eprintln!("WARN: file missing: {}", src.display());
    }
    Ok(())
}

fn append_license_if_exists(
    tar: &mut tar::Builder<GzEncoder<fs::File>>,
    src: &Path,
    dst: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if src.exists() {
        let mut f = fs::File::open(src)?;
        let mut hdr = tar::Header::new_gnu();
        hdr.set_size(f.metadata()?.len());
        hdr.set_mode(0o644);
        hdr.set_cksum();
        tar.append_data(&mut hdr, dst, &mut f)?;
        println!("Added license: {} => {}", src.display(), dst);
    } else {
        eprintln!("WARN: license file missing: {}", src.display());
    }
    Ok(())
}
