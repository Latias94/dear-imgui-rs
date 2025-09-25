use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

use flate2::{Compression, write::GzEncoder};

fn expected_lib_name() -> &'static str {
    if cfg!(target_env = "msvc") {
        "dear_imgui.lib"
    } else {
        "libdear_imgui.a"
    }
}

fn default_target_triple() -> String {
    // Try env first
    if let Ok(t) = env::var("TARGET") {
        return t;
    }
    if let Ok(t) = env::var("CARGO_CFG_TARGET_TRIPLE") {
        return t;
    }
    // Fallback compose
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
                if name.starts_with("dear-imgui-sys-") {
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
            "No dear-imgui-sys build out directories found under {}",
            build_root.display()
        ));
    }
    candidates.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    let from_dir = candidates.pop().unwrap();
    Ok(from_dir)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir.parent().unwrap();
    let _out_dir = PathBuf::from(
        env::var("OUT_DIR").unwrap_or_else(|_| workspace_root.join("target").display().to_string()),
    );

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
    if let Ok(v) = env::var("IMGUI_SYS_PKG_CRT") {
        if !v.is_empty() {
            crt = Box::leak(v.into_boxed_str());
        }
    }

    let link_type = "static"; // we package static lib

    // Package features (comma-separated), e.g. "freetype"
    // Prefer explicit env override, otherwise infer from cargo feature env.
    let mut features = env::var("IMGUI_SYS_PKG_FEATURES").unwrap_or_default();
    if features.is_empty() {
        let mut v = Vec::new();
        if env::var("CARGO_FEATURE_FREETYPE").is_ok() {
            v.push("freetype");
        }
        features = v.join(",");
    }

    let pkg_dir = PathBuf::from(
        env::var("IMGUI_SYS_PACKAGE_DIR").unwrap_or_else(|_| env::var("OUT_DIR").unwrap()),
    );
    fs::create_dir_all(&pkg_dir)?;

    let ft_suffix = if features.split(',').any(|f| f.trim() == "freetype") {
        "-freetype"
    } else {
        ""
    };
    let ar_name = if crt.is_empty() {
        format!(
            "dear-imgui-prebuilt-{}-{}-{}{}.tar.gz",
            crate_version, target, link_type, ft_suffix
        )
    } else {
        format!(
            "dear-imgui-prebuilt-{}-{}-{}{}-{}.tar.gz",
            crate_version, target, link_type, ft_suffix, crt
        )
    };

    println!("Packaging dear-imgui prebuilt:");
    println!("  Target: {}", target);
    println!("  Version: {}", crate_version);
    println!("  Link type: {}", link_type);
    if !crt.is_empty() {
        println!("  CRT: {}", crt);
    }
    println!("  Package dir: {}", pkg_dir.display());

    let sys_out = locate_sys_out_dir(workspace_root, &target)?;
    println!("Using sys build out dir: {}", sys_out.display());

    // Create tar.gz
    let file = fs::File::create(pkg_dir.join(&ar_name))?;
    let enc = GzEncoder::new(file, Compression::best());
    let mut tar = tar::Builder::new(enc);

    // Include headers: imgui headers + cimgui.h
    let cimgui_root = manifest_dir.join("third-party").join("cimgui");
    let imgui_include = cimgui_root.join("imgui");
    if imgui_include.exists() {
        // Only include header files, exclude heavy folders (examples, docs, backends, misc, .github)
        append_headers_only(
            &mut tar,
            &imgui_include,
            "include/imgui",
            &["examples", "docs", "backends", "misc", ".github"],
        )?;
        println!(
            "Added filtered include/imgui headers from: {}",
            imgui_include.display()
        );
    } else {
        eprintln!(
            "WARN: imgui include dir not found: {}",
            imgui_include.display()
        );
    }
    let cimgui_h = cimgui_root.join("cimgui.h");
    if cimgui_h.exists() {
        // Place at include/cimgui/cimgui.h
        let mut f = fs::File::open(&cimgui_h)?;
        tar.append_file("include/cimgui/cimgui.h", &mut f)?;
        println!("Added include/cimgui/cimgui.h: {}", cimgui_h.display());
    } else {
        eprintln!("WARN: cimgui.h not found: {}", cimgui_h.display());
    }

    // Licenses (project + third-party)
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
        &cimgui_root.join("imgui").join("LICENSE.txt"),
        "licenses/imgui-LICENSE.txt",
    )?;
    append_license_if_exists(
        &mut tar,
        &cimgui_root.join("LICENSE"),
        "licenses/cimgui-LICENSE",
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

    // Add simple manifest txt
    let mut manifest_txt = vec![];
    writeln!(
        &mut manifest_txt,
        "dear-imgui prebuilt\nversion={}\ntarget={}\nlink={}\ncrt={}",
        crate_version, target, link_type, crt
    )?;
    writeln!(&mut manifest_txt, "features={}", features)?;
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
            if let std::path::Component::Normal(os) = comp {
                if let Some(name) = os.to_str() {
                    if exclude_dirs.iter().any(|e| e == &name) {
                        return true;
                    }
                }
            }
        }
        false
    }
    let mut stack = vec![src_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if excluded(&dir.strip_prefix(src_dir).unwrap_or(&dir), exclude_dirs) && dir != *src_dir {
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
