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

pub fn prebuilt_manifest_features(dir: &Path) -> Option<Vec<String>> {
    let mut candidates = Vec::with_capacity(2);
    candidates.push(dir.join("manifest.txt"));
    if let Some(parent) = dir.parent() {
        candidates.push(parent.join("manifest.txt"));
    }

    for manifest in candidates {
        let Ok(s) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        for line in s.lines() {
            if let Some(rest) = line.strip_prefix("features=") {
                let features = rest
                    .split(',')
                    .map(|f| f.trim().to_ascii_lowercase())
                    .filter(|f| !f.is_empty())
                    .collect::<Vec<_>>();
                return Some(features);
            }
        }
        return Some(Vec::new());
    }

    None
}

pub fn prebuilt_manifest_has_feature(dir: &Path, feature: &str) -> bool {
    let feature = feature.trim().to_ascii_lowercase();
    let Some(features) = prebuilt_manifest_features(dir) else {
        return false;
    };
    features.iter().any(|f| f == &feature)
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
    #[cfg(feature = "archive")]
    {
        return extract_archive_to_cache_impl(archive_path, cache_root, lib_name);
    }

    #[cfg(not(feature = "archive"))]
    {
        let _ = (archive_path, cache_root, lib_name);
        return Err(
            "archive extraction disabled: enable feature `dear-imgui-build-support/archive`"
                .to_string(),
        );
    }
}

#[cfg(feature = "archive")]
fn extract_archive_to_cache_impl(
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
    if let Some(path) = local_path_from_urlish(url) {
        return stage_or_extract_local(cache_root, &path, lib_name);
    }

    #[cfg(feature = "download")]
    {
        return download_prebuilt_http(cache_root, url, lib_name);
    }

    #[cfg(not(feature = "download"))]
    {
        let _ = (cache_root, url, lib_name);
        return Err(
            "download support disabled: enable feature `dear-imgui-build-support/download`"
                .to_string(),
        );
    }
}

fn local_path_from_urlish(url: &str) -> Option<PathBuf> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(rest) = trimmed.strip_prefix("file://") {
        // Accept both file:///C:/... and file://C:/...
        let rest = rest.trim_start_matches('/');
        let p = PathBuf::from(rest);
        if p.exists() {
            return Some(p);
        }
        return None;
    }

    let p = PathBuf::from(trimmed);
    if p.exists() { Some(p) } else { None }
}

fn stage_or_extract_local(
    cache_root: &Path,
    path: &Path,
    lib_name: &str,
) -> Result<PathBuf, String> {
    if is_archive_path(path) {
        return extract_archive_to_cache(path, cache_root, lib_name);
    }

    if path
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s == lib_name)
    {
        let Some(parent) = path.parent() else {
            return Err("local prebuilt path had no parent directory".to_string());
        };
        return Ok(parent.to_path_buf());
    }

    let dl_dir = cache_root.join("download");
    let _ = std::fs::create_dir_all(&dl_dir);
    let dst = dl_dir.join(lib_name);
    if !dst.exists() {
        std::fs::copy(path, &dst).map_err(|e| format!("copy {}: {}", path.display(), e))?;
    }
    Ok(dl_dir)
}

fn is_archive_path(path: &Path) -> bool {
    let s = path.to_string_lossy().to_ascii_lowercase();
    s.ends_with(".tar.gz") || s.ends_with(".tgz")
}

#[cfg(feature = "download")]
fn download_prebuilt_http(cache_root: &Path, url: &str, lib_name: &str) -> Result<PathBuf, String> {
    let dl_dir = cache_root.join("download");
    let _ = std::fs::create_dir_all(&dl_dir);

    if url.ends_with(".tar.gz") || url.ends_with(".tgz") {
        let fname = url.split('/').next_back().unwrap_or("prebuilt.tar.gz");
        let archive_path = dl_dir.join(fname);
        if !archive_path.exists() {
            let config = ureq::Agent::config_builder()
                .timeout_global(Some(std::time::Duration::from_secs(300)))
                .build();
            let agent = ureq::Agent::new_with_config(config);
            let resp = agent
                .get(url)
                .call()
                .map_err(|e| format!("http get: {}", e))?;
            let status = resp.status();
            if !status.is_success() {
                return Err(format!("http status {}", status));
            }
            let mut reader = resp.into_body().into_reader();
            let mut bytes = Vec::new();
            use std::io::Read as _;
            reader
                .read_to_end(&mut bytes)
                .map_err(|e| format!("read body: {}", e))?;
            std::fs::write(&archive_path, &bytes)
                .map_err(|e| format!("write {}: {}", archive_path.display(), e))?;
        }
        return extract_archive_to_cache(&archive_path, cache_root, lib_name);
    }

    let dst = dl_dir.join(lib_name);
    if dst.exists() {
        return Ok(dl_dir);
    }
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(120)))
        .build();
    let agent = ureq::Agent::new_with_config(config);
    let resp = agent
        .get(url)
        .call()
        .map_err(|e| format!("http get: {}", e))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("http status {}", status));
    }
    let mut reader = resp.into_body().into_reader();
    let mut bytes = Vec::new();
    use std::io::Read as _;
    reader
        .read_to_end(&mut bytes)
        .map_err(|e| format!("read body: {}", e))?;
    std::fs::write(&dst, &bytes).map_err(|e| format!("write {}: {}", dst.display(), e))?;
    Ok(dl_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_tmp_dir(suffix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "dear-imgui-build-support-test-{}-{}-{}",
            std::process::id(),
            nanos,
            suffix
        ))
    }

    #[test]
    fn prebuilt_manifest_has_feature_checks_parent_manifest_for_lib_dir() {
        let root = unique_tmp_dir("parent");
        let lib_dir = root.join("lib");
        std::fs::create_dir_all(&lib_dir).unwrap();
        std::fs::write(
            root.join("manifest.txt"),
            "crate prebuilt\nfeatures=wchar32,freetype\n",
        )
        .unwrap();

        assert!(prebuilt_manifest_has_feature(&lib_dir, "wchar32"));
        assert!(prebuilt_manifest_has_feature(&lib_dir, "freetype"));
        assert!(!prebuilt_manifest_has_feature(&lib_dir, "nope"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn prebuilt_manifest_has_feature_checks_manifest_in_dir() {
        let root = unique_tmp_dir("self");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("manifest.txt"), "features=wchar32\n").unwrap();

        assert!(prebuilt_manifest_has_feature(&root, "wchar32"));
        assert!(!prebuilt_manifest_has_feature(&root, "freetype"));

        let _ = std::fs::remove_dir_all(&root);
    }
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
