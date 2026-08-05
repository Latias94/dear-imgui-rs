#[cfg(feature = "binding-spec")]
use crate::binding;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

static STATIC_CPP_STDLIB_LINK_EMITTED: AtomicBool = AtomicBool::new(false);

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

pub fn should_static_link_cpp_stdlib(target_os: &str, target_env: &str) -> bool {
    target_os == "windows" && target_env == "gnu"
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CppRuntimeLinkage {
    None,
    Dynamic(&'static str),
    StaticBundle(&'static str),
}

pub fn prebuilt_cpp_runtime_linkage(target_os: &str, target_env: &str) -> CppRuntimeLinkage {
    if target_env == "msvc" {
        return CppRuntimeLinkage::None;
    }
    if should_static_link_cpp_stdlib(target_os, target_env) {
        return CppRuntimeLinkage::StaticBundle("stdc++");
    }
    if matches!(
        target_os,
        "macos" | "ios" | "tvos" | "watchos" | "visionos" | "freebsd" | "openbsd" | "aix" | "wasi"
    ) || (target_os == "linux" && target_env == "ohos")
    {
        return CppRuntimeLinkage::Dynamic("c++");
    }
    if target_os == "android" {
        return CppRuntimeLinkage::Dynamic("c++_shared");
    }
    CppRuntimeLinkage::Dynamic("stdc++")
}

pub fn emit_prebuilt_cpp_runtime_linkage(target_os: &str, target_env: &str) {
    match prebuilt_cpp_runtime_linkage(target_os, target_env) {
        CppRuntimeLinkage::None => {}
        CppRuntimeLinkage::Dynamic(library) => {
            println!("cargo:rustc-link-lib={library}");
        }
        CppRuntimeLinkage::StaticBundle(library) => {
            if !STATIC_CPP_STDLIB_LINK_EMITTED.swap(true, Ordering::Relaxed) {
                println!("cargo:rustc-link-lib=static:-bundle={library}");
            }
        }
    }
}

pub fn configure_cpp_runtime_linkage(build: &mut cc::Build, target_os: &str, target_env: &str) {
    match prebuilt_cpp_runtime_linkage(target_os, target_env) {
        CppRuntimeLinkage::None => {
            build.cpp_link_stdlib(None);
        }
        CppRuntimeLinkage::Dynamic(library) => {
            // Keep source builds and prebuilt consumers on the same deterministic platform
            // policy instead of allowing an untracked CXXSTDLIB override.
            build.cpp_link_stdlib(library);
        }
        CppRuntimeLinkage::StaticBundle(_) => {
            build.cpp_link_stdlib(None);
            emit_prebuilt_cpp_runtime_linkage(target_os, target_env);
        }
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

#[cfg(feature = "binding-spec")]
pub fn compose_manifest_bytes_with_profile(profile: &binding::ArtifactProfile) -> Vec<u8> {
    profile.manifest_bytes()
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
        extract_archive_to_cache_impl(archive_path, cache_root, lib_name)
    }

    #[cfg(not(feature = "archive"))]
    {
        let _ = (archive_path, cache_root, lib_name);
        Err(
            "archive extraction disabled: enable feature `dear-imgui-build-support/archive`"
                .to_string(),
        )
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
    extract_archive_to_dir(archive_path, &extract_dir, lib_name, || {})
}

#[cfg(feature = "archive")]
fn extract_archive_to_dir(
    archive_path: &Path,
    extract_dir: &Path,
    lib_name: &str,
    after_lock: impl FnOnce(),
) -> Result<PathBuf, String> {
    let parent = extract_dir.parent().ok_or_else(|| {
        format!(
            "prebuilt extraction directory has no parent: {}",
            extract_dir.display()
        )
    })?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("create dir {}: {}", parent.display(), e))?;
    let lock_path = extraction_lock_path(extract_dir);
    let extraction_lock = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|error| format!("open extraction lock {}: {error}", lock_path.display()))?;
    extraction_lock
        .lock()
        .map_err(|error| format!("lock extraction cache {}: {error}", lock_path.display()))?;
    after_lock();

    if let Some(lib_dir) = extracted_library_dir(extract_dir, lib_name) {
        return Ok(lib_dir);
    }
    if extract_dir.exists() {
        let stale_dir = unique_staging_path(extract_dir);
        std::fs::rename(extract_dir, &stale_dir).map_err(|error| {
            format!(
                "retire stale extraction directory {}: {error}",
                extract_dir.display()
            )
        })?;
        std::fs::remove_dir_all(&stale_dir)
            .map_err(|error| format!("remove stale dir {}: {error}", stale_dir.display()))?;
    }
    let staging_dir = unique_staging_path(extract_dir);
    if staging_dir.exists() {
        let _ = std::fs::remove_dir_all(&staging_dir);
    }
    std::fs::create_dir_all(&staging_dir)
        .map_err(|e| format!("create dir {}: {}", staging_dir.display(), e))?;
    let file = std::fs::File::open(archive_path)
        .map_err(|e| format!("open {}: {}", archive_path.display(), e))?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
    if let Err(error) = archive.unpack(&staging_dir) {
        let _ = std::fs::remove_dir_all(&staging_dir);
        return Err(format!("unpack {}: {}", archive_path.display(), error));
    }
    let staged_lib_dir = extracted_library_dir(&staging_dir, lib_name).ok_or_else(|| {
        let _ = std::fs::remove_dir_all(&staging_dir);
        "extracted archive did not contain the expected library and manifest".to_owned()
    })?;
    let uses_lib_subdir = staged_lib_dir != staging_dir;

    match std::fs::rename(&staging_dir, extract_dir) {
        Ok(()) => Ok(if uses_lib_subdir {
            extract_dir.join("lib")
        } else {
            extract_dir.to_path_buf()
        }),
        Err(error) => {
            // Another process may have completed the same extraction first. Its atomic rename is
            // safe to reuse only after validating the expected library is present.
            if let Some(lib_dir) = extracted_library_dir(extract_dir, lib_name) {
                let _ = std::fs::remove_dir_all(&staging_dir);
                return Ok(lib_dir);
            }
            let _ = std::fs::remove_dir_all(&staging_dir);
            Err(format!(
                "install extracted archive at {}: {}",
                extract_dir.display(),
                error
            ))
        }
    }
}

#[cfg(feature = "archive")]
fn extraction_lock_path(extract_dir: &Path) -> PathBuf {
    let lock_name = extract_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("prebuilt");
    extract_dir.with_file_name(format!(".{lock_name}.extract.lock"))
}

#[cfg(feature = "archive")]
fn extracted_library_dir(root: &Path, lib_name: &str) -> Option<PathBuf> {
    if !root.join("manifest.txt").is_file() {
        return None;
    }
    let lib_dir = root.join("lib");
    if lib_dir.join(lib_name).is_file() {
        Some(lib_dir)
    } else if root.join(lib_name).is_file() {
        Some(root.to_path_buf())
    } else {
        None
    }
}

fn unique_staging_path(destination: &Path) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("prebuilt");
    destination.with_file_name(format!(".{name}.tmp-{}-{nonce}", std::process::id()))
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
        download_prebuilt_http(cache_root, url, lib_name)
    }

    #[cfg(not(feature = "download"))]
    {
        let _ = (cache_root, url, lib_name);
        Err(
            "download support disabled: enable feature `dear-imgui-build-support/download`"
                .to_string(),
        )
    }
}

fn local_path_from_urlish(url: &str) -> Option<PathBuf> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(rest) = trimmed.strip_prefix("file://") {
        let p = file_url_path(rest);
        if p.exists() {
            return Some(p);
        }
        return None;
    }

    let p = PathBuf::from(trimmed);
    if p.exists() { Some(p) } else { None }
}

fn file_url_path(rest: &str) -> PathBuf {
    #[cfg(windows)]
    {
        // Windows accepts both file:///C:/... and file://C:/.... Remove exactly the URL root
        // slash only when the remainder starts with a drive letter; preserve UNC prefixes.
        let bytes = rest.as_bytes();
        if bytes.len() >= 3
            && bytes[0] == b'/'
            && bytes[1].is_ascii_alphabetic()
            && bytes[2] == b':'
        {
            return PathBuf::from(&rest[1..]);
        }
    }

    // On Unix the leading slash is the filesystem root and must not be discarded.
    PathBuf::from(rest)
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
        let bytes = std::fs::read(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
        write_atomic(&dst, &bytes)?;
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
            write_atomic(&archive_path, &bytes)?;
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
    write_atomic(&dst, &bytes)?;
    Ok(dl_dir)
}

fn write_atomic(destination: &Path, bytes: &[u8]) -> Result<(), String> {
    let staging = unique_staging_path(destination);
    std::fs::write(&staging, bytes)
        .map_err(|error| format!("write {}: {error}", staging.display()))?;
    match std::fs::rename(&staging, destination) {
        Ok(()) => Ok(()),
        Err(_error) if destination.is_file() => {
            let _ = std::fs::remove_file(&staging);
            Ok(())
        }
        Err(error) => {
            let _ = std::fs::remove_file(&staging);
            Err(format!(
                "install downloaded artifact at {}: {error}",
                destination.display()
            ))
        }
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

#[cfg(test)]
mod tests {
    use super::{
        CppRuntimeLinkage, extract_archive_to_cache, file_url_path, local_path_from_urlish,
        prebuilt_cpp_runtime_linkage, prebuilt_manifest_has_feature, should_static_link_cpp_stdlib,
    };
    #[cfg(feature = "archive")]
    use super::{extract_archive_to_dir, extraction_lock_path};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn static_cpp_stdlib_is_limited_to_windows_gnu_targets() {
        assert!(should_static_link_cpp_stdlib("windows", "gnu"));
        assert!(!should_static_link_cpp_stdlib("windows", "msvc"));
        assert!(!should_static_link_cpp_stdlib("linux", "gnu"));
        assert!(!should_static_link_cpp_stdlib("macos", ""));
    }

    #[test]
    fn prebuilt_cpp_runtime_matches_cc_defaults_and_windows_policy() {
        assert_eq!(
            prebuilt_cpp_runtime_linkage("windows", "msvc"),
            CppRuntimeLinkage::None
        );
        assert_eq!(
            prebuilt_cpp_runtime_linkage("windows", "gnu"),
            CppRuntimeLinkage::StaticBundle("stdc++")
        );
        assert_eq!(
            prebuilt_cpp_runtime_linkage("macos", ""),
            CppRuntimeLinkage::Dynamic("c++")
        );
        assert_eq!(
            prebuilt_cpp_runtime_linkage("linux", "gnu"),
            CppRuntimeLinkage::Dynamic("stdc++")
        );
        assert_eq!(
            prebuilt_cpp_runtime_linkage("android", ""),
            CppRuntimeLinkage::Dynamic("c++_shared")
        );
    }

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

    #[test]
    fn absolute_file_url_preserves_the_filesystem_root() {
        let root = unique_tmp_dir("absolute-file-url");
        std::fs::create_dir_all(&root).unwrap();
        let archive = root.join("artifact.tar.gz");
        std::fs::write(&archive, b"test").unwrap();
        let url = format!("file://{}", archive.display());

        assert_eq!(local_path_from_urlish(&url), Some(archive.clone()));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn plain_local_prebuilt_path_remains_supported() {
        let root = unique_tmp_dir("plain-local-path");
        std::fs::create_dir_all(&root).unwrap();
        let archive = root.join("artifact.tar.gz");
        std::fs::write(&archive, b"test").unwrap();

        assert_eq!(
            local_path_from_urlish(archive.to_str().unwrap()),
            Some(archive.clone())
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(feature = "archive")]
    #[test]
    fn archive_extraction_is_installed_from_atomic_staging() {
        use std::io::Write as _;

        let root = unique_tmp_dir("atomic-archive");
        let source = root.join("source");
        let cache = root.join("cache");
        std::fs::create_dir_all(source.join("lib")).unwrap();
        std::fs::write(source.join("lib/libdear_imgui.a"), b"archive").unwrap();
        std::fs::write(source.join("manifest.txt"), b"features=wchar32\n").unwrap();
        let archive_path = root.join("artifact.tar.gz");
        let file = std::fs::File::create(&archive_path).unwrap();
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
        let mut archive = tar::Builder::new(encoder);
        archive
            .append_path_with_name(source.join("lib/libdear_imgui.a"), "lib/libdear_imgui.a")
            .unwrap();
        archive
            .append_path_with_name(source.join("manifest.txt"), "manifest.txt")
            .unwrap();
        archive
            .into_inner()
            .unwrap()
            .finish()
            .unwrap()
            .flush()
            .unwrap();

        let lib_dir = extract_archive_to_cache(&archive_path, &cache, "libdear_imgui.a").unwrap();

        assert_eq!(
            std::fs::read(lib_dir.join("libdear_imgui.a")).unwrap(),
            b"archive"
        );
        assert!(
            std::fs::read_dir(lib_dir.parent().unwrap())
                .unwrap()
                .all(|entry| !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .contains(".tmp-"))
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(feature = "archive")]
    #[test]
    fn archive_extraction_lock_serializes_check_cleanup_and_install() {
        use std::io::Write as _;
        use std::sync::mpsc;

        let root = unique_tmp_dir("locked-archive");
        let source = root.join("source");
        let extract_dir = root.join("cache/shared");
        std::fs::create_dir_all(source.join("lib")).unwrap();
        std::fs::create_dir_all(&extract_dir).unwrap();
        std::fs::write(extract_dir.join("stale.partial"), b"partial").unwrap();
        std::fs::write(source.join("lib/libdear_imgui.a"), b"archive").unwrap();
        std::fs::write(source.join("manifest.txt"), b"features=wchar32\n").unwrap();
        let archive_path = root.join("artifact.tar.gz");
        let file = std::fs::File::create(&archive_path).unwrap();
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
        let mut archive = tar::Builder::new(encoder);
        archive
            .append_path_with_name(source.join("lib/libdear_imgui.a"), "lib/libdear_imgui.a")
            .unwrap();
        archive
            .append_path_with_name(source.join("manifest.txt"), "manifest.txt")
            .unwrap();
        archive
            .into_inner()
            .unwrap()
            .finish()
            .unwrap()
            .flush()
            .unwrap();

        let (locked_tx, locked_rx) = mpsc::sync_channel(0);
        let (release_tx, release_rx) = mpsc::sync_channel(0);
        let first_archive = archive_path.clone();
        let first_extract = extract_dir.clone();
        let first = std::thread::spawn(move || {
            extract_archive_to_dir(&first_archive, &first_extract, "libdear_imgui.a", || {
                locked_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            })
        });
        locked_rx.recv().unwrap();

        let lock_path = extraction_lock_path(&extract_dir);
        let competing_lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_path)
            .unwrap();
        assert!(matches!(
            competing_lock.try_lock(),
            Err(std::fs::TryLockError::WouldBlock)
        ));

        release_tx.send(()).unwrap();
        let first_lib_dir = first.join().unwrap().unwrap();
        let second_lib_dir =
            extract_archive_to_dir(&archive_path, &extract_dir, "libdear_imgui.a", || {}).unwrap();

        assert_eq!(first_lib_dir, second_lib_dir);
        assert_eq!(
            std::fs::read(second_lib_dir.join("libdear_imgui.a")).unwrap(),
            b"archive"
        );
        assert!(!extract_dir.join("stale.partial").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(windows)]
    #[test]
    fn windows_file_url_removes_only_the_drive_root_slash() {
        assert_eq!(
            file_url_path("/C:/artifacts/dear-imgui.tar.gz"),
            PathBuf::from("C:/artifacts/dear-imgui.tar.gz")
        );
        assert_eq!(
            file_url_path("//server/share/dear-imgui.tar.gz"),
            PathBuf::from("//server/share/dear-imgui.tar.gz")
        );
    }
}
