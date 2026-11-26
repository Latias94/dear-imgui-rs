#[cfg(feature = "binding-spec")]
use crate::binding;
#[cfg(feature = "download")]
use sha2::{Digest, Sha256};
use std::env;
#[cfg(feature = "archive")]
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
    extract_archive_to_dir(&extract_dir, lib_name, || open_local_archive(archive_path))
}

#[cfg(feature = "archive")]
fn extract_archive_to_dir<R: Read>(
    extract_dir: &Path,
    lib_name: &str,
    open_archive: impl FnOnce() -> Result<(R, String), String>,
) -> Result<PathBuf, String> {
    let parent = extract_dir.parent().ok_or_else(|| {
        format!(
            "prebuilt extraction directory has no parent: {}",
            extract_dir.display()
        )
    })?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("create dir {}: {}", parent.display(), e))?;
    let _extraction_lock = lock_cache_file(&extraction_lock_path(extract_dir))?;
    let (reader, fingerprint) = open_archive()?;
    let fingerprint_file = ".archive-fingerprint";
    if std::fs::read_to_string(extract_dir.join(fingerprint_file))
        .ok()
        .as_deref()
        == Some(fingerprint.as_str())
        && let Some(lib_dir) = extracted_library_dir(extract_dir, lib_name)
    {
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
    std::fs::create_dir(&staging_dir)
        .map_err(|e| format!("create dir {}: {}", staging_dir.display(), e))?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(reader));
    if let Err(error) = archive.unpack(&staging_dir) {
        let _ = std::fs::remove_dir_all(&staging_dir);
        return Err(format!(
            "unpack archive into {}: {error}",
            extract_dir.display()
        ));
    }
    let staged_lib_dir = extracted_library_dir(&staging_dir, lib_name).ok_or_else(|| {
        let _ = std::fs::remove_dir_all(&staging_dir);
        "extracted archive did not contain the expected library and manifest".to_owned()
    })?;
    let uses_lib_subdir = staged_lib_dir != staging_dir;
    if let Err(error) = std::fs::write(staging_dir.join(fingerprint_file), &fingerprint) {
        let _ = std::fs::remove_dir_all(&staging_dir);
        return Err(format!("write extraction cache fingerprint: {error}"));
    }

    match std::fs::rename(&staging_dir, extract_dir) {
        Ok(()) => Ok(if uses_lib_subdir {
            extract_dir.join("lib")
        } else {
            extract_dir.to_path_buf()
        }),
        Err(error) => {
            // Another process may have completed the same extraction first. Its atomic rename is
            // safe to reuse only after validating the expected library is present.
            if std::fs::read_to_string(extract_dir.join(fingerprint_file))
                .ok()
                .as_deref()
                == Some(fingerprint.as_str())
                && let Some(lib_dir) = extracted_library_dir(extract_dir, lib_name)
            {
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
fn open_local_archive(path: &Path) -> Result<(std::fs::File, String), String> {
    use std::io::Seek as _;
    let mut file =
        std::fs::File::open(path).map_err(|error| format!("open {}: {error}", path.display()))?;
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("read {}: {error}", path.display()))?;
        if count == 0 {
            break;
        }
        for byte in &buffer[..count] {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    file.rewind()
        .map_err(|error| format!("rewind {}: {error}", path.display()))?;
    // Local packages use a cache freshness key. Remote archives carry a verified digest.
    Ok((file, format!("fnv1a64:{hash:016x}")))
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
fn lock_cache_file(path: &Path) -> Result<std::fs::File, String> {
    let file = std::fs::OpenOptions::new()
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
    static NEXT_STAGING_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let staging_id = NEXT_STAGING_ID.fetch_add(1, Ordering::Relaxed);
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("prebuilt");
    destination.with_file_name(format!(
        ".{name}.tmp-{}-{nonce}-{staging_id}",
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
        println!("cargo:rerun-if-changed={}", path.display());
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

    let verification_failed = |error: &String| {
        println!("cargo::error=Prebuilt verification failed: {error}");
    };
    let (checksums, fetched_checksums) = if let Some(path) = options.checksums {
        let checksums = std::fs::read_to_string(path)
            .map_err(|error| format!("read {SHA256SUMS_ENV} file {}: {error}", path.display()))
            .inspect_err(verification_failed)?;
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
    let digest =
        verify_archive_sha256(&bytes, archive_name, &checksums).inspect_err(verification_failed)?;
    if fetched_checksums {
        // Publish the manifest first so an offline reader cannot see a new archive without it.
        write_atomic(&checksums_path, checksums.as_bytes())?;
    }
    if downloaded {
        write_atomic(&archive_path, &bytes)?;
    }
    drop(cache_lock);
    let extract_dir = cache_root.join("verified").join(&digest);
    extract_archive_to_dir(&extract_dir, lib_name, || {
        Ok((bytes.as_slice(), format!("sha256:{digest}")))
    })
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
    cache_root
        .join("download")
        .join(format!("{:x}", Sha256::digest(url.as_bytes())))
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
                .map(|dir| dir.join("dear-imgui-prebuilt-cache"))
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

        assert_eq!(cache, out_dir.join("dear-imgui-prebuilt-cache"));
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
        assert!(!root.path().join("download").exists());

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
    fn packaged_archive_bytes(contents: &[u8]) -> Vec<u8> {
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        let mut archive = tar::Builder::new(encoder);
        for (name, bytes) in [
            ("lib/libcore.a", contents),
            ("manifest.txt", b"features=wchar32\n".as_slice()),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            archive.append_data(&mut header, name, bytes).unwrap();
        }
        archive.into_inner().unwrap().finish().unwrap()
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
            extract_archive_to_dir(&first_extract, "libdear_imgui.a", || {
                locked_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                open_local_archive(&first_archive)
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
        let second_lib_dir = extract_archive_to_dir(&extract_dir, "libdear_imgui.a", || {
            open_local_archive(&archive_path)
        })
        .unwrap();

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
