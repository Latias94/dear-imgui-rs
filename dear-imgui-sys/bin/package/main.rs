use build_support::{compose_archive_name, compose_manifest_bytes};
use flate2::{Compression, write::GzEncoder};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

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

fn sys_out_dir() -> PathBuf {
    // This path belongs to the exact Cargo feature profile that compiled this package binary.
    // Scanning target/build by mtime can select a stale artifact from another feature profile.
    PathBuf::from(env!("OUT_DIR"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap();

    let target = default_target_triple();
    let crate_version = env!("CARGO_PKG_VERSION").to_string();
    let target_os = std::env::consts::OS;
    let target_env = if cfg!(target_env = "msvc") {
        "msvc"
    } else if cfg!(target_env = "gnu") {
        "gnu"
    } else {
        ""
    };
    let mut crt = if target_os == "windows" && target_env == "msvc" {
        if cfg!(target_feature = "crt-static") {
            "mt"
        } else {
            "md"
        }
    } else {
        ""
    };
    if let Ok(v) = env::var("IMGUI_SYS_PKG_CRT")
        && !v.is_empty()
    {
        crt = Box::leak(v.into_boxed_str());
    }

    let link_type = "static"; // we package static lib

    // Package features (comma-separated), e.g. "wchar32,freetype".
    //
    // We always compile with `IMGUI_USE_WCHAR32`, so this is always declared to allow the sys
    // build script to reject ABI-incompatible prebuilts.
    // Artifact-changing features must match the Cargo profile that built this binary. The
    // optional environment list may add metadata but cannot claim an unavailable artifact.
    let explicit_features = env::var("IMGUI_SYS_PKG_FEATURES").unwrap_or_default();
    let mut features: Vec<String> = explicit_features
        .split(',')
        .map(|feature| feature.trim().to_ascii_lowercase())
        .filter(|feature| !feature.is_empty())
        .collect();
    for (feature, enabled) in [
        ("stack-layout", cfg!(feature = "stack-layout")),
        ("freetype", cfg!(feature = "freetype")),
        ("test-engine", cfg!(feature = "test-engine")),
    ] {
        let declared = features.iter().any(|candidate| candidate == feature);
        if declared && !enabled {
            return Err(format!(
                "IMGUI_SYS_PKG_FEATURES declares {feature}, but this package binary was built without feature `{feature}`"
            )
            .into());
        }
        if enabled && !declared {
            features.push(feature.to_string());
        }
    }
    for required in ["wchar32", "platform-io-aggregate-hooks"] {
        if !features.iter().any(|feature| feature == required) {
            features.push(required.to_string());
        }
    }
    features.sort_unstable();
    features.dedup();
    let features = features.join(",");

    let pkg_dir = PathBuf::from(
        env::var("IMGUI_SYS_PACKAGE_DIR").unwrap_or_else(|_| env!("OUT_DIR").to_string()),
    );
    fs::create_dir_all(&pkg_dir)?;

    let has_freetype = features.split(',').any(|f| f.trim() == "freetype");
    let has_test_engine = features.split(',').any(|f| f.trim() == "test-engine");
    let has_stack_layout = features.split(',').any(|f| f.trim() == "stack-layout");
    let mut suffix = String::new();
    if has_stack_layout {
        suffix.push_str("-stack-layout");
    }
    if has_freetype {
        suffix.push_str("-freetype");
    }
    if has_test_engine {
        suffix.push_str("-test-engine");
    }
    let suffix = if suffix.is_empty() {
        None
    } else {
        Some(suffix)
    };
    let ar_name = compose_archive_name(
        "dear-imgui",
        &crate_version,
        &target,
        link_type,
        suffix.as_deref(),
        crt,
    );

    println!("Packaging dear-imgui prebuilt:");
    println!("  Target: {}", target);
    println!("  Version: {}", crate_version);
    println!("  Link type: {}", link_type);
    if !crt.is_empty() {
        println!("  CRT: {}", crt);
    }
    println!("  Package dir: {}", pkg_dir.display());

    let sys_out = sys_out_dir();
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
    append_license_if_exists(
        &mut tar,
        &manifest_dir.join("THIRD_PARTY_NOTICES.md"),
        "licenses/dear-imgui-sys-THIRD_PARTY_NOTICES.md",
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
    let manifest_txt = compose_manifest_bytes(
        "dear-imgui",
        &crate_version,
        &target,
        link_type,
        crt,
        Some(&features),
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
