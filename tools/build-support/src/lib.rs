use std::env;
use std::path::{Path, PathBuf};

pub fn parse_bool_env(key: &str) -> bool {
    match env::var(key) {
        Ok(v) => matches!(
            v.as_str(),
            "1" | "true" | "yes" | "on" | "TRUE" | "YES" | "ON"
        ),
        Err(_) => false,
    }
}

pub fn msvc_crt_suffix_from_env(target_env: Option<&str>) -> Option<&'static str> {
    let is_msvc = match target_env {
        Some(s) => s == "msvc",
        None => matches!(
            env::var("CARGO_CFG_TARGET_ENV").ok().as_deref(),
            Some("msvc")
        ),
    };
    if !is_msvc {
        return None;
    }
    let tf = env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default();
    if tf.split(',').any(|f| f == "crt-static") {
        Some("mt")
    } else {
        Some("md")
    }
}

pub fn expected_lib_name(target_env: &str, base: &str) -> String {
    if target_env == "msvc" {
        format!("{}.lib", base)
    } else {
        format!("lib{}.a", base)
    }
}

pub fn compose_archive_name(
    crate_short: &str,
    version: &str,
    target: &str,
    link_type: &str,
    extra: Option<&str>,
    crt: &str,
) -> String {
    let extra = extra.unwrap_or("");
    if crt.is_empty() {
        if extra.is_empty() {
            format!(
                "{}-prebuilt-{}-{}-{}.tar.gz",
                crate_short, version, target, link_type
            )
        } else {
            format!(
                "{}-prebuilt-{}-{}-{}{}.tar.gz",
                crate_short, version, target, link_type, extra
            )
        }
    } else if extra.is_empty() {
        format!(
            "{}-prebuilt-{}-{}-{}-{}.tar.gz",
            crate_short, version, target, link_type, crt
        )
    } else {
        format!(
            "{}-prebuilt-{}-{}-{}{}-{}.tar.gz",
            crate_short, version, target, link_type, extra, crt
        )
    }
}

pub fn release_tags(crate_sys_name: &str, version: &str) -> [String; 2] {
    [
        format!("{}-v{}", crate_sys_name, version),
        format!("v{}", version),
    ]
}

pub fn compose_manifest_bytes(
    crate_short: &str,
    version: &str,
    target: &str,
    link_type: &str,
    crt: &str,
    features: Option<&str>,
) -> Vec<u8> {
    let mut buf = Vec::new();
    use std::io::Write;
    let _ = writeln!(
        &mut buf,
        "{} prebuilt\nversion={}\ntarget={}\nlink={}\ncrt={}",
        crate_short, version, target, link_type, crt
    );
    if let Some(f) = features
        && !f.is_empty()
    {
        let _ = writeln!(&mut buf, "features={}", f);
    }
    buf
}

pub fn release_candidate_urls(
    owner: &str,
    repo: &str,
    tags: &[String],
    names: &[String],
) -> Vec<String> {
    let mut out = Vec::with_capacity(tags.len() * names.len());
    for tag in tags {
        for name in names {
            out.push(format!(
                "https://github.com/{}/{}/releases/download/{}/{}",
                owner, repo, tag, name
            ));
        }
    }
    out
}

pub fn release_candidate_urls_default(tags: &[String], names: &[String]) -> Vec<String> {
    release_candidate_urls(DEFAULT_GITHUB_OWNER, DEFAULT_GITHUB_REPO, tags, names)
}

pub fn release_owner_repo() -> (String, String) {
    let owner =
        env::var("BUILD_SUPPORT_GH_OWNER").unwrap_or_else(|_| DEFAULT_GITHUB_OWNER.to_string());
    let repo =
        env::var("BUILD_SUPPORT_GH_REPO").unwrap_or_else(|_| DEFAULT_GITHUB_REPO.to_string());
    (owner, repo)
}

pub fn release_candidate_urls_env(tags: &[String], names: &[String]) -> Vec<String> {
    let (owner, repo) = release_owner_repo();
    release_candidate_urls(&owner, &repo, tags, names)
}

pub fn is_offline() -> bool {
    match env::var("CARGO_NET_OFFLINE") {
        Ok(v) => matches!(
            v.as_str(),
            "1" | "true" | "yes" | "on" | "TRUE" | "YES" | "ON"
        ),
        Err(_) => false,
    }
}

pub fn prebuilt_extract_dir_env(cache_root: &Path, target_env: &str) -> PathBuf {
    let target = env::var("TARGET").unwrap_or_default();
    let crt_suffix = if target_env == "msvc" {
        let tf = env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default();
        if tf.split(',').any(|f| f == "crt-static") {
            "-mt"
        } else {
            "-md"
        }
    } else {
        ""
    };
    cache_root
        .join(target)
        .join(format!("static{}", crt_suffix))
}

pub fn extract_archive_to_cache(
    archive_path: &Path,
    cache_root: &Path,
    lib_name: &str,
) -> Result<PathBuf, String> {
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let extract_dir = prebuilt_extract_dir_env(cache_root, &target_env);
    if extract_dir.exists() {
        let lib_dir = extract_dir.join("lib");
        if lib_dir.join(lib_name).exists() || extract_dir.join(lib_name).exists() {
            return Ok(lib_dir);
        }
        let _ = std::fs::remove_dir_all(&extract_dir);
    }
    std::fs::create_dir_all(&extract_dir)
        .map_err(|e| format!("create dir {}: {}", extract_dir.display(), e))?;
    let file = std::fs::File::open(archive_path)
        .map_err(|e| format!("open {}: {}", archive_path.display(), e))?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
    archive
        .unpack(&extract_dir)
        .map_err(|e| format!("unpack {}: {}", archive_path.display(), e))?;
    let lib_dir = extract_dir.join("lib");
    if lib_dir.join(lib_name).exists() {
        return Ok(lib_dir);
    }
    if extract_dir.join(lib_name).exists() {
        return Ok(extract_dir);
    }
    Err("extracted archive did not contain expected library".into())
}

pub fn download_prebuilt(
    cache_root: &Path,
    url: &str,
    lib_name: &str,
    _target_env: &str,
) -> Result<PathBuf, String> {
    let dl_dir = cache_root.join("download");
    let _ = std::fs::create_dir_all(&dl_dir);

    if url.ends_with(".tar.gz") || url.ends_with(".tgz") {
        let fname = url.split('/').next_back().unwrap_or("prebuilt.tar.gz");
        let archive_path = dl_dir.join(fname);
        if !archive_path.exists() {
            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .map_err(|e| format!("create http client: {}", e))?;
            let resp = client
                .get(url)
                .send()
                .map_err(|e| format!("http get: {}", e))?;
            if !resp.status().is_success() {
                return Err(format!("http status {}", resp.status()));
            }
            let bytes = resp.bytes().map_err(|e| format!("read body: {}", e))?;
            std::fs::write(&archive_path, &bytes)
                .map_err(|e| format!("write {}: {}", archive_path.display(), e))?;
        }
        return extract_archive_to_cache(&archive_path, cache_root, lib_name);
    }

    let dst = dl_dir.join(lib_name);
    if dst.exists() {
        return Ok(dl_dir);
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("http client: {}", e))?;
    let resp = client
        .get(url)
        .send()
        .map_err(|e| format!("http get: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("http status {}", resp.status()));
    }
    let bytes = resp.bytes().map_err(|e| format!("read body: {}", e))?;
    std::fs::write(&dst, &bytes).map_err(|e| format!("write {}: {}", dst.display(), e))?;
    Ok(dl_dir)
}

pub fn prebuilt_cache_root_from_env_or_target(
    manifest_dir: &Path,
    cache_env_var: &str,
    folder: &str,
) -> PathBuf {
    if let Ok(dir) = env::var(cache_env_var) {
        return PathBuf::from(dir);
    }
    let target_dir = env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| manifest_dir.parent().unwrap().join("target"));
    target_dir.join(folder)
}
pub const DEFAULT_GITHUB_OWNER: &str = "Latias94";
pub const DEFAULT_GITHUB_REPO: &str = "dear-imgui";
