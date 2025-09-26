use std::{
    env, fs,
    path::{Path, PathBuf},
};

use build_support::{compose_archive_name, compose_manifest_bytes};
use flate2::{Compression, write::GzEncoder};

fn expected_lib_name() -> &'static str {
    if cfg!(target_env = "msvc") {
        "dear_imguizmo.lib"
    } else {
        "libdear_imguizmo.a"
    }
}

fn default_target_triple() -> String {
    if let Ok(t) = env::var("TARGET") {
        return t;
    }
    if let Ok(t) = env::var("CARGO_CFG_TARGET_TRIPLE") {
        return t;
    }
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;
    match os {
        "windows" => format!("{}-pc-windows-msvc", arch),
        "macos" => format!("{}-apple-darwin", arch),
        "linux" => format!("{}-unknown-linux-gnu", arch),
        _ => format!("{}-unknown-{}", arch, os),
    }
}

fn locate_sys_out_dir(workspace_root: &std::path::Path, target: &str) -> Result<PathBuf, String> {
    let profile = env::var("PROFILE").unwrap_or_else(|_| "release".into());
    let target_dir = env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| workspace_root.join("target"));
    let build_root = target_dir.join(target).join(&profile).join("build");
    if !build_root.exists() {
        return Err(format!("Build root not found at {}", build_root.display()));
    }
    let mut candidates: Vec<PathBuf> = match std::fs::read_dir(&build_root) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let p = e.path();
                let name = p.file_name()?.to_string_lossy().to_string();
                if name.starts_with("dear-imguizmo-sys-") {
                    let out = p.join("out");
                    if out.exists() { Some(out) } else { None }
                } else {
                    None
                }
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    if candidates.is_empty() {
        return Err(format!(
            "No dear-imguizmo-sys build out directories found under {}",
            build_root.display()
        ));
    }
    candidates.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    let from_dir = candidates.pop().unwrap();
    Ok(from_dir)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir.parent().and_then(|p| p.parent()).unwrap();

    let target = default_target_triple();
    let crate_version = env::var("CARGO_PKG_VERSION").unwrap();
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let target_features = env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default();
    let mut crt = if target_os == "windows" && target_env == "msvc" {
        if target_features.split(',').any(|f| f == "crt-static") {
            "mt"
        } else {
            "md"
        }
    } else {
        ""
    };
    if let Ok(v) = env::var("IMGUIZMO_SYS_PKG_CRT")
        && !v.is_empty()
    {
        crt = Box::leak(v.into_boxed_str());
    }

    let link_type = "static";

    let pkg_dir = env::var("IMGUIZMO_SYS_PACKAGE_DIR")
        .or_else(|_| env::var("IMGUI_SYS_PACKAGE_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env::var("OUT_DIR").unwrap()));
    fs::create_dir_all(&pkg_dir)?;

    let ar_name = compose_archive_name(
        "dear-imguizmo",
        &crate_version,
        &target,
        link_type,
        None,
        crt,
    );

    println!("Packaging dear-imguizmo prebuilt:");
    println!("  Target: {}", target);
    println!("  Version: {}", crate_version);
    println!("  Link type: {}", link_type);
    if !crt.is_empty() {
        println!("  CRT: {}", crt);
    }
    println!("  Package dir: {}", pkg_dir.display());

    let sys_out = locate_sys_out_dir(workspace_root, &target)?;
    println!("Using sys build out dir: {}", sys_out.display());

    let file = fs::File::create(pkg_dir.join(&ar_name))?;
    let enc = GzEncoder::new(file, Compression::best());
    let mut tar = tar::Builder::new(enc);

    // Include headers: ImGuizmo headers + cimguizmo.h
    let cimguizmo_root = manifest_dir.join("third-party").join("cimguizmo");
    let imguizmo_include = cimguizmo_root.join("ImGuizmo");
    if imguizmo_include.exists() {
        append_headers_only(
            &mut tar,
            &imguizmo_include,
            "include/ImGuizmo",
            &[
                "example",
                "examples",
                ".github",
                "generator",
                "docs",
                "misc",
            ],
        )?;
        println!(
            "Added filtered include/ImGuizmo headers from: {}",
            imguizmo_include.display()
        );
    } else {
        eprintln!(
            "WARN: ImGuizmo include dir not found: {}",
            imguizmo_include.display()
        );
    }
    let cimguizmo_h = cimguizmo_root.join("cimguizmo.h");
    if cimguizmo_h.exists() {
        let mut f = fs::File::open(&cimguizmo_h)?;
        tar.append_file("include/cimguizmo/cimguizmo.h", &mut f)?;
        println!(
            "Added include/cimguizmo/cimguizmo.h: {}",
            cimguizmo_h.display()
        );
    } else {
        eprintln!("WARN: cimguizmo.h not found: {}", cimguizmo_h.display());
    }

    // Licenses (project + third-party)
    let workspace_root = manifest_dir.parent().and_then(|p| p.parent()).unwrap();
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
        &cimguizmo_root.join("LICENSE"),
        "licenses/cimguizmo-LICENSE",
    )?;
    append_license_if_exists(
        &mut tar,
        &cimguizmo_root.join("ImGuizmo").join("LICENSE"),
        "licenses/ImGuizmo-LICENSE",
    )?;

    // Include library
    let lib_name = expected_lib_name();
    let lib_path = sys_out.join(lib_name);
    if !lib_path.exists() {
        return Err(format!("Static library not found at {}", lib_path.display()).into());
    }
    let mut f = fs::File::open(&lib_path)?;
    tar.append_file(format!("lib/{}", lib_name), &mut f)?;
    println!("Added lib: {}", lib_path.display());

    // manifest
    let manifest_txt = compose_manifest_bytes(
        "dear-imguizmo",
        &crate_version,
        &target,
        link_type,
        crt,
        None,
    );
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
    tar: &mut tar::Builder<flate2::write::GzEncoder<fs::File>>,
    src_dir: &Path,
    dst_root: &str,
    exclude_dirs: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    fn excluded(path: &Path, exclude_dirs: &[&str]) -> bool {
        for comp in path.components() {
            if let std::path::Component::Normal(os) = comp
                && let Some(name) = os.to_str()
                && exclude_dirs.iter().any(|e| e == &name)
            {
                return true;
            }
        }
        false
    }
    let mut stack = vec![src_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if excluded(dir.strip_prefix(src_dir).unwrap_or(&dir), exclude_dirs) && dir != *src_dir {
            continue;
        }
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let p = entry.path();
            let rel = p.strip_prefix(src_dir).unwrap();
            if p.is_dir() {
                if !excluded(rel, exclude_dirs) {
                    stack.push(p);
                }
            } else if p
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("h"))
                .unwrap_or(false)
            {
                let mut f = fs::File::open(&p)?;
                let dst_path = format!("{}/{}", dst_root, rel.display());
                tar.append_file(dst_path, &mut f)?;
            }
        }
    }
    Ok(())
}

fn append_license_if_exists(
    tar: &mut tar::Builder<flate2::write::GzEncoder<fs::File>>,
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
