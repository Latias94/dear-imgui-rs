#[cfg(feature = "binding-spec")]
use crate::binding;
#[cfg(feature = "download")]
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
#[cfg(feature = "download")]
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

static STATIC_CPP_STDLIB_LINK_EMITTED: AtomicBool = AtomicBool::new(false);

pub fn parse_bool_env(key: &str) -> bool {
    match env::var(key) {
        Ok(v) => matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"),
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

pub fn prebuilt_cpp_runtime_linkage(
    target_os: &str,
    target_env: &str,
    target_abi: &str,
) -> CppRuntimeLinkage {
    if target_env == "msvc" {
        return CppRuntimeLinkage::None;
    }
    if should_static_link_cpp_stdlib(target_os, target_env) {
        return CppRuntimeLinkage::StaticBundle(if target_abi == "llvm" {
            "c++"
        } else {
            "stdc++"
        });
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

pub fn emit_prebuilt_cpp_runtime_linkage(target_os: &str, target_env: &str, target_abi: &str) {
    match prebuilt_cpp_runtime_linkage(target_os, target_env, target_abi) {
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

pub fn configure_cpp_runtime_linkage(
    build: &mut cc::Build,
    target_os: &str,
    target_env: &str,
    target_abi: &str,
) {
    match prebuilt_cpp_runtime_linkage(target_os, target_env, target_abi) {
        CppRuntimeLinkage::None => {
            build.cpp_link_stdlib(None);
        }
        CppRuntimeLinkage::Dynamic(library) => {
            // Keep source builds and prebuilt consumers on the same deterministic platform
            // policy instead of allowing an untracked CXXSTDLIB override.
            build.cpp_link_stdlib(library);
        }
        CppRuntimeLinkage::StaticBundle(_) => {
            // `static:-bundle` must be emitted after the C++ archive. Emitting it here would
            // place the runtime before the archive on Cargo versions that preserve build-script
            // directive order, so GNU linkers would never revisit it to resolve C++ symbols.
            build.cpp_link_stdlib(None);
        }
    }
}

pub fn cpp_no_exceptions_flag(target_env: &str) -> &'static str {
    if target_env == "msvc" {
        "/EHs-c-"
    } else {
        "-fno-exceptions"
    }
}

pub fn configure_cpp_no_exceptions(build: &mut cc::Build, target_env: &str) {
    build.flag(cpp_no_exceptions_flag(target_env));
    if target_env == "msvc" {
        build.define("_HAS_EXCEPTIONS", Some("0"));
    }
}

pub fn compile_cpp_archive(
    build: &mut cc::Build,
    library: &str,
    target_os: &str,
    target_env: &str,
    target_abi: &str,
) {
    build.compile(library);

    if let CppRuntimeLinkage::StaticBundle(runtime) =
        prebuilt_cpp_runtime_linkage(target_os, target_env, target_abi)
    {
        // Repeat this after every archive rather than deduplicating it. A build script may emit
        // several C++ archives, and each archive must have a following runtime occurrence for
        // single-pass static linkers.
        println!("cargo:rustc-link-lib=static:-bundle={runtime}");
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
    println!("cargo:rerun-if-env-changed=BUILD_SUPPORT_GH_OWNER");
    println!("cargo:rerun-if-env-changed=BUILD_SUPPORT_GH_REPO");
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

/// Return the target/CRT directory used by the legacy extraction cache layout.
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
        println!("cargo:rerun-if-changed={}", archive_path.display());
        if !archive_path.is_file() {
            return Err(format!(
                "local prebuilt archive is not a file: {}",
                archive_path.display()
            ));
        }
        let bytes = read_local_prebuilt(archive_path)?;
        let key = local_content_key(&bytes);
        extract_archive_to_dir(
            &cache_root.join(format!("a-{key}")),
            lib_name,
            &bytes,
            &format!("fnv1a64:{key}"),
        )
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
fn extract_archive_to_dir(
    extract_dir: &Path,
    lib_name: &str,
    bytes: &[u8],
    fingerprint: &str,
) -> Result<PathBuf, String> {
    use std::io::Write as _;

    let parent = extract_dir.parent().ok_or_else(|| {
        format!(
            "prebuilt extraction directory has no parent: {}",
            extract_dir.display()
        )
    })?;
    fs::create_dir_all(parent).map_err(|error| format!("create {}: {error}", parent.display()))?;
    let _lock = lock_cache_file(&extract_dir.with_extension("lock"))?;
    const FINGERPRINT_FILE: &str = ".archive-fingerprint";
    let marker = extract_dir.join(FINGERPRINT_FILE);
    if let Some(lib_dir) = extracted_library_dir(extract_dir, lib_name)
        && is_plain_file(&marker)
        && fs::read_to_string(&marker).is_ok_and(|cached| cached == fingerprint)
    {
        return Ok(lib_dir);
    }

    validate_archive_members(bytes)?;
    let staging_dir = unique_staging_path(extract_dir);
    fs::create_dir(&staging_dir)
        .map_err(|error| format!("create {}: {error}", staging_dir.display()))?;
    let install = || -> Result<_, String> {
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(bytes));
        archive.set_overwrite(false);
        archive
            .unpack(&staging_dir)
            .map_err(|error| format!("unpack archive into {}: {error}", staging_dir.display()))?;
        let library_dir = extracted_library_dir(&staging_dir, lib_name)
            .ok_or("extracted archive did not contain the expected library and manifest")?;
        let uses_lib_subdir = library_dir != staging_dir;
        fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(staging_dir.join(FINGERPRINT_FILE))
            .and_then(|mut file| file.write_all(fingerprint.as_bytes()))
            .map_err(|error| format!("write extraction cache fingerprint: {error}"))?;

        match remove_cached_extraction(extract_dir) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "remove stale extraction {}: {error}",
                    extract_dir.display()
                ));
            }
        }
        fs::rename(&staging_dir, extract_dir).map_err(|error| {
            format!(
                "install extracted archive at {}: {error}",
                extract_dir.display()
            )
        })?;
        Ok(if uses_lib_subdir {
            extract_dir.join("lib")
        } else {
            extract_dir.to_path_buf()
        })
    };
    install().inspect_err(|_| {
        let _ = fs::remove_dir_all(&staging_dir);
    })
}

#[cfg(feature = "archive")]
fn remove_cached_extraction(path: &Path) -> std::io::Result<()> {
    let file_type = fs::symlink_metadata(path)?.file_type();
    if file_type.is_dir() {
        return fs::remove_dir_all(path);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileTypeExt as _;
        if file_type.is_symlink_dir() {
            return fs::remove_dir(path);
        }
    }
    fs::remove_file(path)
}

#[cfg(feature = "archive")]
fn validate_archive_members(bytes: &[u8]) -> Result<(), String> {
    use std::path::Component;

    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(bytes));
    let entries = archive
        .entries()
        .map_err(|error| format!("read tar entries: {error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("read tar entry: {error}"))?;
        let kind = entry.header().entry_type();
        let path = entry
            .path()
            .map_err(|error| format!("read tar entry path: {error}"))?;
        if !kind.is_file() && !kind.is_dir() {
            return Err(format!("unsupported tar entry type {kind:?} for {path:?}"));
        }
        if path.components().any(|component| match component {
            Component::CurDir => false,
            Component::Normal(name) => cfg!(windows) && name.as_encoded_bytes().contains(&b':'),
            _ => true,
        }) {
            return Err(format!("invalid tar entry path {path:?}"));
        }
    }
    Ok(())
}

fn read_local_prebuilt(path: &Path) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))
}

fn local_content_key(bytes: &[u8]) -> String {
    let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    format!("{hash:016x}")
}

fn lock_cache_file(path: &Path) -> Result<fs::File, String> {
    let file = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)
        .map_err(|error| format!("open cache lock {}: {error}", path.display()))?;
    file.lock()
        .map_err(|error| format!("lock cache {}: {error}", path.display()))?;
    Ok(file)
}

fn is_plain_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_file())
}

#[cfg(feature = "archive")]
fn extracted_library_dir(root: &Path, lib_name: &str) -> Option<PathBuf> {
    if !fs::symlink_metadata(root).is_ok_and(|metadata| metadata.is_dir())
        || !is_plain_file(&root.join("manifest.txt"))
    {
        return None;
    }
    let lib_dir = root.join("lib");
    if fs::symlink_metadata(&lib_dir).is_ok_and(|metadata| metadata.is_dir())
        && is_plain_file(&lib_dir.join(lib_name))
        && match fs::symlink_metadata(lib_dir.join("manifest.txt")) {
            Ok(metadata) => metadata.is_file(),
            Err(error) => error.kind() == std::io::ErrorKind::NotFound,
        }
    {
        Some(lib_dir)
    } else if is_plain_file(&root.join(lib_name)) {
        Some(root.to_path_buf())
    } else {
        None
    }
}

fn unique_staging_path(destination: &Path) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    static NEXT_STAGING_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let staging_id = NEXT_STAGING_ID.fetch_add(1, Ordering::Relaxed);
    destination.with_file_name(format!(
        ".tmp-{:x}-{nonce:x}-{staging_id:x}",
        std::process::id()
    ))
}

pub fn download_prebuilt(
    cache_root: &Path,
    url: &str,
    lib_name: &str,
    _target_env: &str,
) -> Result<PathBuf, String> {
    if let Some(path) = local_path_from_urlish(url) {
        if !path.is_file() {
            return Err(format!(
                "local prebuilt file does not exist: {}",
                path.display()
            ));
        }
        return stage_or_extract_local(cache_root, &path, lib_name);
    }

    #[cfg(feature = "download")]
    {
        for name in ["CARGO_NET_OFFLINE", ALLOW_INSECURE_HTTP_ENV, SHA256SUMS_ENV] {
            println!("cargo:rerun-if-env-changed={name}");
        }
        let checksums = env::var_os(SHA256SUMS_ENV).map(PathBuf::from);
        download_prebuilt_http(
            cache_root,
            url.trim(),
            lib_name,
            DownloadOptions {
                offline: is_offline(),
                allow_insecure_http: env::var_os(ALLOW_INSECURE_HTTP_ENV).as_deref()
                    == Some(std::ffi::OsStr::new("1")),
                checksums: checksums.as_deref(),
            },
        )
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
        return Some(file_url_path(rest));
    }

    (!trimmed.contains("://")).then(|| PathBuf::from(trimmed))
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

    println!("cargo:rerun-if-changed={}", path.display());
    if path
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s == lib_name)
    {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        return Ok(parent.to_path_buf());
    }

    let bytes = read_local_prebuilt(path)?;
    let dir = cache_root.join(format!("l-{}", local_content_key(&bytes)));
    fs::create_dir_all(cache_root)
        .map_err(|error| format!("create {}: {error}", cache_root.display()))?;
    let _lock = lock_cache_file(&dir.with_extension("lock"))?;
    fs::create_dir_all(&dir).map_err(|error| format!("create {}: {error}", dir.display()))?;
    if !fs::symlink_metadata(&dir).is_ok_and(|metadata| metadata.is_dir()) {
        return Err(format!("not a regular cache directory: {}", dir.display()));
    }
    let dst = dir.join(lib_name);
    if !is_plain_file(&dst) || !fs::read(&dst).is_ok_and(|cached| cached == bytes) {
        write_atomic(&dst, &bytes)?;
    }
    Ok(dir)
}

fn is_archive_path(path: &Path) -> bool {
    let s = path.to_string_lossy().to_ascii_lowercase();
    s.ends_with(".tar.gz") || s.ends_with(".tgz")
}

#[cfg(feature = "download")]
const ALLOW_INSECURE_HTTP_ENV: &str = "DEAR_IMGUI_ALLOW_INSECURE_HTTP";
#[cfg(feature = "download")]
const SHA256SUMS_ENV: &str = "DEAR_IMGUI_SHA256SUMS";
#[cfg(feature = "download")]
const SHA256SUMS: &str = "SHA256SUMS";

#[cfg(feature = "download")]
struct DownloadOptions<'a> {
    offline: bool,
    allow_insecure_http: bool,
    checksums: Option<&'a Path>,
}

#[cfg(feature = "download")]
fn download_prebuilt_http(
    cache_root: &Path,
    url: &str,
    lib_name: &str,
    options: DownloadOptions<'_>,
) -> Result<PathBuf, String> {
    let uri = url
        .parse::<ureq::http::Uri>()
        .map_err(|error| format!("invalid prebuilt archive URL: {error}"))?;
    let (archive_name, checksums_url) =
        release_archive_location(&uri, options.allow_insecure_http)?;
    let cache = remote_cache_root(cache_root, url);
    let archive_path = cache.join("archive.tar.gz");
    let checksums_path = cache.join(SHA256SUMS);
    std::fs::create_dir_all(&cache)
        .map_err(|error| format!("create {}: {error}", cache.display()))?;
    let cache_lock = lock_cache_file(&cache.join(".download.lock"))?;
    let agent = (!options.offline).then(|| {
        let config = ureq::Agent::config_builder()
            .https_only(uri.scheme_str() == Some("https"))
            .timeout_global(Some(std::time::Duration::from_secs(300)))
            .build();
        ureq::Agent::new_with_config(config)
    });
    if let Some(path) = options.checksums {
        println!("cargo:rerun-if-changed={}", path.display());
    }

    let (bytes, downloaded) = match std::fs::read(&archive_path) {
        Ok(bytes) => (bytes, false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let agent = agent.as_ref().ok_or_else(|| {
                "CARGO_NET_OFFLINE is set and the requested prebuilt archive is not cached"
                    .to_owned()
            })?;
            let mut bytes = Vec::new();
            fetch_prebuilt(agent, url)
                .map_err(|error| format!("fetch prebuilt archive {archive_name}: {error}"))?
                .into_reader()
                .read_to_end(&mut bytes)
                .map_err(|error| format!("read prebuilt archive {archive_name}: {error}"))?;
            (bytes, true)
        }
        Err(error) => return Err(format!("read {}: {error}", archive_path.display())),
    };

    let (checksums, fetched_checksums) = if let Some(path) = options.checksums {
        let checksums = std::fs::read_to_string(path)
            .map_err(|error| format!("read {SHA256SUMS_ENV} file {}: {error}", path.display()))?;
        (checksums, false)
    } else {
        let cached = match (!downloaded)
            .then(|| std::fs::read_to_string(&checksums_path))
            .transpose()
        {
            Ok(cached) => cached,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(format!("read cached {}: {error}", checksums_path.display())),
        };
        if let Some(checksums) = cached {
            (checksums, false)
        } else {
            let agent = agent.as_ref().ok_or_else(|| {
                format!(
                    "CARGO_NET_OFFLINE is set and cached {SHA256SUMS} for {archive_name} is missing; set {SHA256SUMS_ENV} to a local checksum manifest"
                )
            })?;
            let checksums = fetch_prebuilt(agent, &checksums_url)
                .and_then(|mut body| body.read_to_string().map_err(|error| error.to_string()))
                .map_err(|error| {
                    format!("fetch release {SHA256SUMS} for {archive_name}: {error}")
                })?;
            (checksums, true)
        }
    };
    let digest = verify_archive_sha256(&bytes, archive_name, &checksums)?;
    if fetched_checksums {
        // Publish the manifest first so an offline reader cannot see a new archive without it.
        write_atomic(&checksums_path, checksums.as_bytes())?;
    }
    if downloaded {
        write_atomic(&archive_path, &bytes)?;
    }
    drop(cache_lock);
    let extract_dir = cache_root.join(format!("v-{}", &digest[..32]));
    extract_archive_to_dir(&extract_dir, lib_name, &bytes, &format!("sha256:{digest}"))
        .map_err(|error| format!("extract verified archive {archive_name}: {error}"))
}

#[cfg(feature = "download")]
fn release_archive_location(
    uri: &ureq::http::Uri,
    allow_insecure_http: bool,
) -> Result<(&str, String), String> {
    let scheme = match uri.scheme_str() {
        Some("https") => "https",
        Some("http") if allow_insecure_http => "http",
        _ => {
            return Err(format!(
                "prebuilt downloads require HTTPS; set {ALLOW_INSECURE_HTTP_ENV}=1 only for local HTTP testing"
            ));
        }
    };
    let authority = uri.authority().ok_or("prebuilt archive URL has no host")?;
    let (release_path, archive_name) = uri.path().rsplit_once('/').unwrap_or_default();
    if !is_archive_path(Path::new(archive_name)) {
        return Err("prebuilt download URL must identify a .tar.gz or .tgz archive".to_owned());
    }
    Ok((
        archive_name,
        format!("{scheme}://{authority}{release_path}/{SHA256SUMS}"),
    ))
}

#[cfg(feature = "download")]
fn remote_cache_root(cache_root: &Path, url: &str) -> PathBuf {
    let key = format!("{:x}", Sha256::digest(url.as_bytes()));
    cache_root.join(format!("d-{}", &key[..32]))
}

#[cfg(feature = "download")]
fn fetch_prebuilt(agent: &ureq::Agent, url: &str) -> Result<ureq::Body, String> {
    let response = agent.get(url).call().map_err(|error| match error {
        ureq::Error::RequireHttpsOnly(_) => "prebuilt downloads and redirects require HTTPS".into(),
        error => error.to_string(),
    })?;
    if !response.status().is_success() {
        return Err(format!("HTTP status {}", response.status()));
    }
    Ok(response.into_body())
}

#[cfg(feature = "download")]
fn verify_archive_sha256(
    bytes: &[u8],
    archive_name: &str,
    checksums: &str,
) -> Result<String, String> {
    let mut expected = None;
    for line in checksums.lines() {
        let Some((digest, filename)) = line.split_once([' ', '\t']) else {
            continue;
        };
        let filename = filename.trim_start_matches([' ', '\t']);
        if filename.strip_prefix('*').unwrap_or(filename) != archive_name {
            continue;
        }
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!(
                "invalid SHA-256 digest in {SHA256SUMS} for {archive_name}"
            ));
        }
        if expected.replace(digest).is_some() {
            return Err(format!(
                "duplicate entry in {SHA256SUMS} for {archive_name}"
            ));
        }
    }
    let expected =
        expected.ok_or_else(|| format!("{SHA256SUMS} has no entry for {archive_name}"))?;
    let actual = format!("{:x}", Sha256::digest(bytes));
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(format!(
            "SHA-256 mismatch for {archive_name}: expected {expected}, got {actual}"
        ));
    }
    Ok(actual)
}

fn write_atomic(destination: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write as _;

    let staging = unique_staging_path(destination);
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&staging)
        .map_err(|error| format!("create {}: {error}", staging.display()))?;
    let written = file.write_all(bytes);
    drop(file);
    if let Err(error) = written {
        let _ = std::fs::remove_file(&staging);
        return Err(format!("write {}: {error}", staging.display()));
    }
    match std::fs::rename(&staging, destination) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = std::fs::remove_file(&staging);
            Err(format!(
                "install downloaded artifact at {}: {error}",
                destination.display()
            ))
        }
    }
}

/// Resolve a build-script cache under an explicit override or Cargo's output directory.
/// The manifest directory argument is retained for compatibility with existing callers.
pub fn prebuilt_cache_root_from_env_or_target(
    _manifest_dir: &Path,
    cache_env_var: &str,
    folder: &str,
) -> PathBuf {
    println!("cargo:rerun-if-env-changed={cache_env_var}");
    println!("cargo:rerun-if-env-changed=DEAR_IMGUI_CACHE_DIR");
    resolve_prebuilt_cache_root(cache_env_var, folder, |key| env::var_os(key))
        .expect("Cargo must set an absolute OUT_DIR when no prebuilt cache override is configured")
}

fn resolve_prebuilt_cache_root(
    cache_env_var: &str,
    folder: &str,
    read_env: impl Fn(&str) -> Option<std::ffi::OsString>,
) -> Option<PathBuf> {
    let read_path = |key| {
        read_env(key)
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from)
    };
    read_path(cache_env_var)
        .or_else(|| read_path("DEAR_IMGUI_CACHE_DIR").map(|dir| dir.join(folder)))
        .or_else(|| {
            read_path("OUT_DIR")
                .filter(|dir| dir.is_absolute())
                .map(|dir| dir.join("pb"))
        })
}

pub const DEFAULT_GITHUB_OWNER: &str = "Latias94";
pub const DEFAULT_GITHUB_REPO: &str = "dear-imgui-rs";

#[cfg(test)]
mod tests {
    use super::*;
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
            prebuilt_cpp_runtime_linkage("windows", "msvc", ""),
            CppRuntimeLinkage::None
        );
        assert_eq!(
            prebuilt_cpp_runtime_linkage("windows", "gnu", ""),
            CppRuntimeLinkage::StaticBundle("stdc++")
        );
        assert_eq!(
            prebuilt_cpp_runtime_linkage("windows", "gnu", "llvm"),
            CppRuntimeLinkage::StaticBundle("c++")
        );
        assert_eq!(
            prebuilt_cpp_runtime_linkage("macos", "", ""),
            CppRuntimeLinkage::Dynamic("c++")
        );
        assert_eq!(
            prebuilt_cpp_runtime_linkage("linux", "gnu", "eabihf"),
            CppRuntimeLinkage::Dynamic("stdc++")
        );
        assert_eq!(
            prebuilt_cpp_runtime_linkage("android", "", ""),
            CppRuntimeLinkage::Dynamic("c++_shared")
        );
    }

    #[test]
    fn no_exception_flags_match_msvc_and_gnu_style_compilers() {
        assert_eq!(cpp_no_exceptions_flag("msvc"), "/EHs-c-");
        assert_eq!(cpp_no_exceptions_flag("gnu"), "-fno-exceptions");
        assert_eq!(cpp_no_exceptions_flag(""), "-fno-exceptions");
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
    fn default_release_candidates_use_the_publishing_repository() {
        let names = ["core.tar.gz".to_owned()];
        let tags = release_tags("dear-imgui-sys", "1.2.3");
        assert_eq!(
            release_candidate_urls_default(&tags, &names),
            [
                "https://github.com/Latias94/dear-imgui-rs/releases/download/dear-imgui-sys-v1.2.3/core.tar.gz",
                "https://github.com/Latias94/dear-imgui-rs/releases/download/v1.2.3/core.tar.gz",
            ]
        );
    }

    #[test]
    fn default_cache_can_be_created_under_out_dir_with_an_unwritable_source_target() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("installed-source");
        std::fs::create_dir_all(source.join("crate")).unwrap();
        let blocked_target = source.join("target");
        std::fs::write(&blocked_target, b"cannot contain cache files").unwrap();
        let out_dir = root.path().join("out");
        let cache = resolve_prebuilt_cache_root("TEST_SYS_CACHE_DIR", "test-prebuilt", |key| {
            (key == "OUT_DIR").then(|| out_dir.as_os_str().to_owned())
        })
        .unwrap();

        assert_eq!(cache, out_dir.join("pb"));
        std::fs::create_dir_all(&cache).unwrap();
        assert_eq!(
            std::fs::read(&blocked_target).unwrap(),
            b"cannot contain cache files"
        );
    }

    #[test]
    fn cache_overrides_keep_crate_precedence_and_namespace_the_shared_root() {
        let root = tempfile::tempdir().unwrap();
        let crate_override = root.path().join("explicit");
        let shared_override = root.path().join("shared");
        let resolve = |use_crate_override| {
            resolve_prebuilt_cache_root("TEST_SYS_CACHE_DIR", "test-prebuilt", |key| match key {
                "TEST_SYS_CACHE_DIR" if use_crate_override => Some(crate_override.clone().into()),
                "DEAR_IMGUI_CACHE_DIR" => Some(shared_override.clone().into()),
                _ => None,
            })
            .unwrap()
        };

        assert_eq!(resolve(true), crate_override);
        assert_eq!(resolve(false), shared_override.join("test-prebuilt"));
        assert!(
            resolve_prebuilt_cache_root("TEST_SYS_CACHE_DIR", "test-prebuilt", |_| None).is_none()
        );
    }

    #[cfg(feature = "download")]
    const ARCHIVE_URL: &str = "https://example.invalid/releases/download/v1.2.3/core.tar.gz";

    #[cfg(feature = "download")]
    const OFFLINE: DownloadOptions<'static> = DownloadOptions {
        offline: true,
        allow_insecure_http: false,
        checksums: None,
    };

    #[cfg(feature = "download")]
    #[test]
    fn plain_http_is_rejected_before_using_the_cache() {
        let root = tempfile::tempdir().unwrap();
        let url = "http://127.0.0.1:9/core.tar.gz";
        let error = download_prebuilt_http(root.path(), url, "libcore.a", OFFLINE).unwrap_err();
        assert!(error.contains("require HTTPS"), "{error}");
        assert!(!remote_cache_root(root.path(), url).exists());

        let uri = url.parse().unwrap();
        assert_eq!(
            release_archive_location(&uri, true).unwrap(),
            ("core.tar.gz", "http://127.0.0.1:9/SHA256SUMS".into()),
        );
    }

    #[cfg(feature = "download")]
    #[test]
    fn release_checksum_url_uses_the_same_tag_and_the_archive_path() {
        let uri = format!("{ARCHIVE_URL}?download=1").parse().unwrap();
        assert_eq!(
            release_archive_location(&uri, false).unwrap(),
            (
                "core.tar.gz",
                "https://example.invalid/releases/download/v1.2.3/SHA256SUMS".into(),
            ),
        );
    }

    #[cfg(feature = "download")]
    #[test]
    fn sha256sums_requires_one_exact_archive_entry() {
        let digest = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        let manifest = format!(
            "{digest}  other-core.tar.gz\n{} *core.tar.gz\r\n",
            digest.to_ascii_uppercase()
        );
        assert_eq!(
            verify_archive_sha256(b"abc", "core.tar.gz", &manifest).unwrap(),
            digest
        );
        for manifest in [
            format!("{digest}  other-core.tar.gz\n"),
            "invalid  core.tar.gz\n".to_owned(),
            format!("{manifest}{digest}  core.tar.gz\n"),
        ] {
            assert!(verify_archive_sha256(b"abc", "core.tar.gz", &manifest).is_err());
        }
    }

    #[cfg(feature = "download")]
    #[test]
    fn offline_download_requires_both_a_cached_archive_and_sha256sums() {
        let root = tempfile::tempdir().unwrap();
        let download = || download_prebuilt_http(root.path(), ARCHIVE_URL, "libcore.a", OFFLINE);
        let error = download().unwrap_err();
        assert!(error.contains("CARGO_NET_OFFLINE"), "{error}");

        let cache = remote_cache_root(root.path(), ARCHIVE_URL);
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::write(
            cache.join("archive.tar.gz"),
            packaged_archive_bytes(b"test"),
        )
        .unwrap();
        let error = download().unwrap_err();
        assert!(error.contains("SHA256SUMS"), "{error}");
    }

    #[cfg(feature = "download")]
    #[test]
    fn an_online_cache_hit_does_not_contact_the_release_server() {
        let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let url = format!("http://{address}/core.tar.gz");
        let root = tempfile::tempdir().unwrap();
        let cache = remote_cache_root(root.path(), &url);
        let bytes = packaged_archive_bytes(b"test");
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::write(cache.join("archive.tar.gz"), &bytes).unwrap();
        std::fs::write(
            cache.join(SHA256SUMS),
            format!("{:x}  core.tar.gz\n", Sha256::digest(&bytes)),
        )
        .unwrap();

        let lib_dir = download_prebuilt_http(
            root.path(),
            &url,
            "libcore.a",
            DownloadOptions {
                offline: false,
                allow_insecure_http: true,
                checksums: None,
            },
        )
        .unwrap();
        assert_eq!(std::fs::read(lib_dir.join("libcore.a")).unwrap(), b"test");
    }

    #[cfg(feature = "download")]
    #[test]
    fn cached_archive_is_reverified_before_reusing_its_extraction() {
        let root = tempfile::tempdir().unwrap();
        let cache = remote_cache_root(root.path(), ARCHIVE_URL);
        let bytes = packaged_archive_bytes(b"test");
        let archive_path = cache.join("archive.tar.gz");
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::write(&archive_path, &bytes).unwrap();
        std::fs::write(
            cache.join(SHA256SUMS),
            format!("{:x}  core.tar.gz\n", Sha256::digest(&bytes)),
        )
        .unwrap();
        let download = || download_prebuilt_http(root.path(), ARCHIVE_URL, "libcore.a", OFFLINE);
        let lib_dir = download().unwrap();
        assert_eq!(std::fs::read(lib_dir.join("libcore.a")).unwrap(), b"test");

        std::fs::write(archive_path, packaged_archive_bytes(b"different")).unwrap();
        let error = download().unwrap_err();
        assert!(error.contains("SHA-256 mismatch"), "{error}");
    }

    #[cfg(feature = "download")]
    #[test]
    fn checksum_rejection_does_not_emit_a_fatal_cargo_directive() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "prebuilt::tests::cached_archive_is_reverified_before_reusing_its_extraction",
                "--nocapture",
            ])
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("1 passed"),
            "{stdout}"
        );
        assert!(
            !stdout.lines().any(|line| {
                line.starts_with("cargo::error=") || line.starts_with("cargo:error=")
            }),
            "{stdout}"
        );
    }

    #[cfg(feature = "download")]
    #[test]
    fn offline_download_uses_the_explicit_local_checksum_manifest() {
        let root = tempfile::tempdir().unwrap();
        let cache = remote_cache_root(root.path(), ARCHIVE_URL);
        let bytes = packaged_archive_bytes(b"test");
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::write(cache.join("archive.tar.gz"), &bytes).unwrap();
        let checksums = root.path().join("offline-sums");
        std::fs::write(
            &checksums,
            format!("{:x}  core.tar.gz\n", Sha256::digest(&bytes)),
        )
        .unwrap();

        let lib_dir = download_prebuilt_http(
            root.path(),
            ARCHIVE_URL,
            "libcore.a",
            DownloadOptions {
                checksums: Some(&checksums),
                ..OFFLINE
            },
        )
        .unwrap();
        assert_eq!(std::fs::read(lib_dir.join("libcore.a")).unwrap(), b"test");
        assert!(!cache.join(SHA256SUMS).exists());
    }

    #[cfg(all(windows, feature = "download"))]
    #[test]
    fn explicit_download_and_extraction_fit_under_windows_max_path() {
        use crate::native_build::explicit_cache_root;
        use std::os::windows::ffi::OsStrExt as _;

        let root = tempfile::tempdir().unwrap();
        let path_len = |path: &Path| path.as_os_str().encode_wide().count();
        let cargo_out = Path::new(r"target\release\build\dear-imgui-sys-0123456789abcdef\out");
        let padding = 130_usize.saturating_sub(path_len(&root.path().join(cargo_out)) + 1);
        let out_dir = root.path().join("x".repeat(padding)).join(cargo_out);
        let default = resolve_prebuilt_cache_root("TEST_SYS_CACHE_DIR", "test-prebuilt", |key| {
            (key == "OUT_DIR").then(|| out_dir.as_os_str().to_owned())
        })
        .unwrap();
        let profile_cache = default.join("fnv1a64-0123456789abcdef");
        let cache_root = explicit_cache_root(&profile_cache, ARCHIVE_URL);
        let assert_fits = |path: &Path| {
            let tail = path.strip_prefix(&out_dir).unwrap();
            assert!(130 + 1 + path_len(tail) < 260, "{}", path.display());
        };
        let cache = remote_cache_root(&cache_root, ARCHIVE_URL);
        let library_name = "dear_imgui_test_engine.lib";
        let bytes = archive_bytes(&[
            ("lib", tar::EntryType::Directory, b""),
            (
                &format!("lib/{library_name}"),
                tar::EntryType::Regular,
                b"current",
            ),
            ("manifest.txt", tar::EntryType::Regular, b"features=\n"),
        ]);
        let digest = format!("{:x}", Sha256::digest(&bytes));
        fs::create_dir_all(&cache).unwrap();
        let archive = cache.join("archive.tar.gz");
        let checksums = cache.join(SHA256SUMS);
        for path in [&archive, &checksums] {
            assert_fits(path);
            assert_fits(&unique_staging_path(path));
        }
        write_atomic(&archive, &bytes).unwrap();
        write_atomic(&checksums, format!("{digest}  core.tar.gz\n").as_bytes()).unwrap();

        let legacy = cache_root.join("verified").join(&digest);
        fs::create_dir_all(legacy.join("lib")).unwrap();
        let legacy_library = legacy.join("lib").join(library_name);
        fs::write(&legacy_library, b"legacy").unwrap();
        fs::write(legacy.join("manifest.txt"), b"features=\n").unwrap();
        fs::write(
            legacy.join(".archive-fingerprint"),
            format!("sha256:{digest}"),
        )
        .unwrap();

        let lib_dir =
            download_prebuilt_http(&cache_root, ARCHIVE_URL, library_name, OFFLINE).unwrap();
        let library = lib_dir.join(library_name);
        assert_fits(&library);
        assert_fits(
            &unique_staging_path(lib_dir.parent().unwrap())
                .join("lib")
                .join(library_name),
        );
        assert_eq!(fs::read(&library).unwrap(), b"current");
        assert_eq!(fs::read(&legacy_library).unwrap(), b"legacy");
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

    #[test]
    fn renamed_local_library_refreshes_even_with_the_same_size_and_mtime() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("artifact.a");
        let cache = root.path().join("cache");
        let stage = || download_prebuilt(&cache, path.to_str().unwrap(), "libcore.a", "").unwrap();
        std::fs::write(&path, b"first").unwrap();
        let first = stage();
        let mtime = path.metadata().unwrap().modified().unwrap();

        std::fs::write(&path, b"other").unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(mtime)
            .unwrap();
        let second = stage();
        assert_eq!(std::fs::read(second.join("libcore.a")).unwrap(), b"other");
        assert_eq!(std::fs::read(first.join("libcore.a")).unwrap(), b"first");
        fs::write(second.join("libcore.a"), b"damaged").unwrap();
        assert_eq!(stage(), second);
        assert_eq!(fs::read(second.join("libcore.a")).unwrap(), b"other");
    }

    #[test]
    fn competing_local_writers_use_complete_content_snapshots() {
        let root = tempfile::tempdir().unwrap();
        let cache = root.path().join("cache");
        let inputs = [b"first", b"other", b"first"];
        let sources: Vec<_> = inputs
            .iter()
            .enumerate()
            .map(|(index, bytes)| {
                let source = root.path().join(format!("input{index}.a"));
                fs::write(&source, bytes).unwrap();
                source
            })
            .collect();
        let barrier = std::sync::Barrier::new(inputs.len());
        let directories: Vec<_> = std::thread::scope(|scope| {
            sources
                .iter()
                .map(|source| {
                    scope.spawn(|| {
                        barrier.wait();
                        stage_or_extract_local(&cache, source, "libcore.a").unwrap()
                    })
                })
                .collect::<Vec<_>>()
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect()
        });
        assert_eq!(directories[0], directories[2]);
        assert_ne!(directories[0], directories[1]);
        for (directory, bytes) in directories.iter().zip(inputs) {
            assert_eq!(fs::read(directory.join("libcore.a")).unwrap(), bytes);
        }
    }

    #[cfg(feature = "archive")]
    fn archive_bytes(entries: &[(&str, tar::EntryType, &[u8])]) -> Vec<u8> {
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        let mut archive = tar::Builder::new(encoder);
        for &(name, kind, bytes) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(kind);
            header.set_size(bytes.len() as u64);
            if kind.is_gnu_sparse() {
                header
                    .as_gnu_mut()
                    .unwrap()
                    .set_real_size(bytes.len() as u64);
            }
            header.set_mode(if kind.is_dir() { 0o755 } else { 0o644 });
            header.set_cksum();
            archive.append_data(&mut header, name, bytes).unwrap();
        }
        archive.into_inner().unwrap().finish().unwrap()
    }

    #[cfg(feature = "archive")]
    fn packaged_archive_bytes(contents: &[u8]) -> Vec<u8> {
        archive_bytes(&[
            ("lib", tar::EntryType::Directory, b""),
            ("lib/libcore.a", tar::EntryType::Regular, contents),
            (
                "manifest.txt",
                tar::EntryType::Regular,
                b"features=wchar32\n",
            ),
        ])
    }

    #[cfg(feature = "archive")]
    #[test]
    fn archives_reject_special_members_without_publishing_an_extraction() {
        for kind in [
            tar::EntryType::Fifo,
            tar::EntryType::Char,
            tar::EntryType::Block,
            tar::EntryType::Symlink,
            tar::EntryType::Link,
            tar::EntryType::Continuous,
            tar::EntryType::GNUSparse,
            tar::EntryType::new(b'V'),
        ] {
            let root = tempfile::tempdir().unwrap();
            let cache = root.path().join("cache");
            let path = root.path().join("core.tar.gz");
            let bytes = archive_bytes(&[
                ("libcore.a", tar::EntryType::Regular, b"library"),
                ("manifest.txt", tar::EntryType::Regular, b"features=\n"),
                ("special", kind, b""),
            ]);
            std::fs::write(&path, bytes).unwrap();
            let error = extract_archive_to_cache(&path, &cache, "libcore.a").unwrap_err();
            assert!(error.contains("unsupported tar entry type"), "{error}");
            let publishes_directory = fs::read_dir(&cache)
                .unwrap()
                .any(|entry| entry.unwrap().file_type().unwrap().is_dir());
            assert!(!publishes_directory);
        }
    }

    #[cfg(feature = "archive")]
    #[test]
    fn local_package_dir_accepts_a_package_without_release_checksums() {
        let root = tempfile::tempdir().unwrap();
        let package_dir = root.path().join("packages");
        std::fs::create_dir_all(&package_dir).unwrap();
        let name = "core.tar.gz";
        std::fs::write(package_dir.join(name), packaged_archive_bytes(b"local")).unwrap();

        let lib_dir = crate::native_build::PrebuiltSource::PackageDir(package_dir)
            .acquire(&root.path().join("cache"), "libcore.a", "", &[name.into()])
            .unwrap();
        assert_eq!(std::fs::read(lib_dir.join("libcore.a")).unwrap(), b"local");
        assert!(prebuilt_manifest_has_feature(&lib_dir, "wchar32"));
    }

    #[cfg(feature = "archive")]
    #[test]
    fn archive_replacement_at_the_same_path_refreshes_the_extraction() {
        let root = tempfile::tempdir().unwrap();
        let archive_path = root.path().join("artifact.tar.gz");
        let cache = root.path().join("cache");
        std::fs::write(&archive_path, packaged_archive_bytes(b"first")).unwrap();
        let first = extract_archive_to_cache(&archive_path, &cache, "libcore.a").unwrap();
        assert_eq!(std::fs::read(first.join("libcore.a")).unwrap(), b"first");
        std::fs::write(&archive_path, packaged_archive_bytes(b"second")).unwrap();
        let second = extract_archive_to_cache(&archive_path, &cache, "libcore.a").unwrap();
        assert_eq!(std::fs::read(second.join("libcore.a")).unwrap(), b"second");
        assert_eq!(std::fs::read(first.join("libcore.a")).unwrap(), b"first");
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
        let root = tempfile::tempdir().unwrap();
        let extract_dir = root.path().join("cache/shared");
        let bytes = packaged_archive_bytes(b"archive");
        fs::create_dir_all(&extract_dir).unwrap();
        fs::write(extract_dir.join("stale.partial"), b"partial").unwrap();
        let barrier = std::sync::Barrier::new(2);
        let extract = || {
            barrier.wait();
            extract_archive_to_dir(&extract_dir, "libcore.a", &bytes, "fingerprint").unwrap()
        };
        let (first_lib_dir, second_lib_dir) = std::thread::scope(|scope| {
            let worker = scope.spawn(extract);
            let library_dir = extract();
            (worker.join().unwrap(), library_dir)
        });
        assert_eq!(first_lib_dir, second_lib_dir);
        assert_eq!(
            fs::read(second_lib_dir.join("libcore.a")).unwrap(),
            b"archive"
        );
        assert!(!extract_dir.join("stale.partial").exists());
    }

    #[cfg(feature = "archive")]
    #[test]
    fn cached_archive_replaces_a_file_at_the_extraction_root() {
        let root = tempfile::tempdir().unwrap();
        let cache = root.path().join("cache");
        fs::write(&cache, b"stale file").unwrap();
        let bytes = packaged_archive_bytes(b"archive");
        let lib_dir = extract_archive_to_dir(&cache, "libcore.a", &bytes, "fingerprint").unwrap();
        assert_eq!(fs::read(lib_dir.join("libcore.a")).unwrap(), b"archive");
    }

    #[cfg(all(any(unix, windows), feature = "archive"))]
    #[test]
    fn cached_archive_does_not_reuse_symlinked_paths() {
        #[cfg(unix)]
        use std::os::unix::fs::{symlink as symlink_dir, symlink as symlink_file};
        #[cfg(windows)]
        use std::os::windows::fs::{symlink_dir, symlink_file};

        let root = tempfile::tempdir().unwrap();
        let outside = root.path().join("other");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("libcore.a"), b"other").unwrap();
        fs::write(outside.join("manifest.txt"), b"features=other\n").unwrap();
        let cache = root.path().join("cache");
        let bytes = packaged_archive_bytes(b"archive");
        let extract =
            || extract_archive_to_dir(&cache, "libcore.a", &bytes, "fingerprint").unwrap();
        let library_dir = extract();

        let library = library_dir.join("libcore.a");
        fs::remove_file(&library).unwrap();
        if let Err(error) = symlink_file(outside.join("libcore.a"), &library) {
            #[cfg(windows)]
            if error.raw_os_error() == Some(1314) {
                eprintln!("Skipping symlink checks without Windows symlink privileges");
                return;
            }
            panic!("{error}");
        }
        assert_eq!(extract(), library_dir);
        assert_eq!(fs::read(&library).unwrap(), b"archive");

        let secondary_manifest = library_dir.join("manifest.txt");
        symlink_file(outside.join("manifest.txt"), &secondary_manifest).unwrap();
        assert_eq!(extract(), library_dir);
        assert!(!secondary_manifest.exists());

        fs::remove_dir_all(&library_dir).unwrap();
        symlink_dir(&outside, &library_dir).unwrap();
        assert_eq!(fs::read(extract().join("libcore.a")).unwrap(), b"archive");

        fs::write(outside.join(".archive-fingerprint"), b"fingerprint").unwrap();
        fs::remove_dir_all(&cache).unwrap();
        symlink_dir(&outside, &cache).unwrap();
        assert_eq!(fs::read(extract().join("libcore.a")).unwrap(), b"archive");

        fs::remove_dir_all(&cache).unwrap();
        symlink_file(outside.join("libcore.a"), &cache).unwrap();
        assert_eq!(fs::read(extract().join("libcore.a")).unwrap(), b"archive");
        assert_eq!(fs::read(outside.join("libcore.a")).unwrap(), b"other");
    }

    #[cfg(all(windows, feature = "archive"))]
    #[test]
    fn cached_archive_replaces_a_junction_without_touching_its_target() {
        let root = tempfile::tempdir().unwrap();
        let outside = root.path().join("other");
        let outside_library = extract_archive_to_dir(
            &outside,
            "libcore.a",
            &packaged_archive_bytes(b"other"),
            "fingerprint",
        )
        .unwrap()
        .join("libcore.a");
        let result = std::process::Command::new("cmd.exe")
            .current_dir(root.path())
            .args(["/D", "/C", "mklink", "/J", "cache", "other"])
            .output()
            .unwrap();
        assert!(result.status.success(), "{result:?}");

        let cache = root.path().join("cache");
        let lib_dir = extract_archive_to_dir(
            &cache,
            "libcore.a",
            &packaged_archive_bytes(b"archive"),
            "fingerprint",
        )
        .unwrap();
        assert_eq!(fs::read(lib_dir.join("libcore.a")).unwrap(), b"archive");
        assert_eq!(fs::read(outside_library).unwrap(), b"other");
    }

    #[test]
    fn stage_or_extract_local_resolves_bare_relative_library_filename_to_dot() {
        let root = tempfile::tempdir().unwrap();
        let resolved = stage_or_extract_local(root.path(), Path::new("libcore.a"), "libcore.a")
            .expect("bare relative filename matching lib_name should resolve to `.`");
        assert_eq!(resolved, PathBuf::from("."));
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
