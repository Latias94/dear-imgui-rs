use std::{
    env, fs,
    path::{Path, PathBuf},
};

use flate2::{Compression, write::GzEncoder};

fn expected_lib_name() -> &'static str {
    if cfg!(target_env = "msvc") {
        "dear_implot.lib"
    } else {
        "libdear_implot.a"
    }
}

fn sys_out_dir() -> PathBuf {
    PathBuf::from(env!("OUT_DIR"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    let target = env!("DEAR_IMGUI_EXTENSION_ARTIFACT_TARGET");
    let crate_version = env!("CARGO_PKG_VERSION");
    let crt = env!("DEAR_IMGUI_EXTENSION_ARTIFACT_CRT");
    if let Ok(v) = env::var("IMPLOT_SYS_PKG_CRT")
        && !v.is_empty()
        && v != crt
    {
        return Err(format!(
            "IMPLOT_SYS_PKG_CRT declares {v}, but this package binary was built for CRT profile {crt}"
        )
        .into());
    }

    let link_type = "static";

    let pkg_dir = env::var("IMPLOT_SYS_PACKAGE_DIR")
        .or_else(|_| env::var("IMGUI_SYS_PACKAGE_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env::var("OUT_DIR").unwrap()));
    fs::create_dir_all(&pkg_dir)?;

    let ar_name = include_str!(concat!(env!("OUT_DIR"), "/prebuilt-archive-name.txt")).trim();

    println!("Packaging dear-implot prebuilt:");
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

    // Include headers: implot headers + cimplot.h
    let cimplot_root = manifest_dir.join("third-party").join("cimplot");
    let implot_include = cimplot_root.join("implot");
    if implot_include.exists() {
        append_headers_only(
            &mut tar,
            &implot_include,
            "include/implot",
            &[
                "example_glfw_opengl3",
                ".github",
                "generator",
                "docs",
                "misc",
            ],
        )?;
        println!(
            "Added filtered include/implot headers from: {}",
            implot_include.display()
        );
    } else {
        eprintln!(
            "WARN: implot include dir not found: {}",
            implot_include.display()
        );
    }
    let cimplot_h = cimplot_root.join("cimplot.h");
    if cimplot_h.exists() {
        let mut f = fs::File::open(&cimplot_h)?;
        tar.append_file("include/cimplot/cimplot.h", &mut f)?;
        println!("Added include/cimplot/cimplot.h: {}", cimplot_h.display());
    } else {
        eprintln!("WARN: cimplot.h not found: {}", cimplot_h.display());
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
        &cimplot_root.join("LICENSE"),
        "licenses/cimplot-LICENSE",
    )?;
    append_license_if_exists(
        &mut tar,
        &cimplot_root.join("implot").join("LICENSE"),
        "licenses/implot-LICENSE",
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
