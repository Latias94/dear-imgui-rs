#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
use std::env;
#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
use std::path::{Path, PathBuf};

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeDependency {
    pub include_paths: Vec<PathBuf>,
    pub source: String,
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
#[derive(Clone, Copy, Debug)]
pub struct PackageSearchConfig {
    pub use_pkg_config: bool,
    pub use_vcpkg: bool,
    pub emit_cargo_metadata: bool,
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
#[derive(Clone, Copy, Debug)]
pub struct Sdl3SearchConfig<'a> {
    pub out_dir: &'a Path,
    pub target_os: &'a str,
    pub use_pkg_config: bool,
    pub use_vcpkg: bool,
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
#[derive(Clone, Copy, Debug)]
pub struct NativeIncludeSearchConfig<'a> {
    pub explicit_include_envs: &'a [&'a str],
    pub dependency_include_envs: &'a [&'a str],
    pub dependency_out_dir_envs: &'a [&'a str],
    pub cargo_target_include_prefix: Option<&'a str>,
    pub out_dir: Option<&'a Path>,
    pub required_header: &'a str,
    pub pkg_config_package: Option<&'a str>,
    pub vcpkg_package: Option<&'a str>,
    pub target_os: &'a str,
    pub use_pkg_config: bool,
    pub use_vcpkg: bool,
    pub emit_cargo_metadata: bool,
    pub print_system_libs: bool,
    pub copy_vcpkg_dlls: bool,
    pub known_include_roots: &'a [PathBuf],
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
pub fn find_native_include_paths(
    config: NativeIncludeSearchConfig<'_>,
) -> Result<NativeDependency, String> {
    find_native_include_paths_inner(config).map_err(|message| {
        format!(
            "could not find include paths containing `{}`. {message}",
            config.required_header
        )
    })
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
pub fn find_freetype(config: PackageSearchConfig) -> Result<NativeDependency, String> {
    let mut attempts = Vec::new();
    emit_pkg_config_rerun_vars("FREETYPE2");
    emit_vcpkg_rerun_vars("FREETYPE");
    let target_os = cargo_target_os();
    let target_env = cargo_target_env();
    let use_vcpkg = should_use_vcpkg(config.use_vcpkg, &target_os, &target_env);

    if config.use_pkg_config {
        if let Some(found) =
            probe_pkg_config_package("freetype2", config.emit_cargo_metadata, true, &mut attempts)
        {
            return Ok(found);
        }
    } else {
        attempts.push("pkg-config feature disabled".to_string());
    }

    if use_vcpkg {
        if let Some(found) = probe_vcpkg_package(
            "freetype",
            config.emit_cargo_metadata,
            config.emit_cargo_metadata,
            &mut attempts,
        ) {
            return Ok(found);
        }
    } else {
        push_vcpkg_skip_attempt(&mut attempts, config.use_vcpkg, &target_os, &target_env);
    }

    let install_hint = if use_vcpkg {
        "Install FreeType development files with pkg-config metadata, or install \
         the vcpkg `freetype` port and set VCPKG_ROOT/VCPKGRS_DYNAMIC as \
         required by vcpkg."
    } else {
        "Install FreeType development files with pkg-config metadata."
    };
    Err(format!(
        "could not find FreeType. Tried {}. {install_hint}",
        attempts.join("; "),
    ))
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
pub fn find_sdl3_include_paths(config: Sdl3SearchConfig<'_>) -> Result<NativeDependency, String> {
    emit_pkg_config_rerun_vars("SDL3");
    emit_vcpkg_rerun_vars("SDL3");
    let known_include_roots = known_sdl3_include_roots(config.target_os);
    find_native_include_paths_inner(NativeIncludeSearchConfig {
        explicit_include_envs: &["SDL3_INCLUDE_DIR"],
        dependency_include_envs: &["DEP_SDL3_INCLUDE_PATH", "DEP_SDL3_INCLUDE_DIR"],
        dependency_out_dir_envs: &["DEP_SDL3_OUT_DIR"],
        cargo_target_include_prefix: Some("sdl3-sys-"),
        out_dir: Some(config.out_dir),
        required_header: "SDL3/SDL.h",
        pkg_config_package: Some("sdl3"),
        vcpkg_package: Some("sdl3"),
        target_os: config.target_os,
        use_pkg_config: config.use_pkg_config,
        use_vcpkg: config.use_vcpkg,
        emit_cargo_metadata: false,
        print_system_libs: false,
        copy_vcpkg_dlls: false,
        known_include_roots: &known_include_roots,
    })
    .map_err(|message| format!("could not find SDL3 headers. {message}"))
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
fn find_native_include_paths_inner(
    config: NativeIncludeSearchConfig<'_>,
) -> Result<NativeDependency, String> {
    let mut attempts = Vec::new();

    for var in config.explicit_include_envs {
        if let Ok(dir) = env::var(var) {
            let root = PathBuf::from(&dir);
            if include_root_has_header(&root, config.required_header) {
                return Ok(NativeDependency {
                    include_paths: vec![root],
                    source: format!("{var}={dir}"),
                });
            }
            return Err(format!(
                "{var} is set to `{dir}`, but `{}` was not found under it",
                root.join(config.required_header).display()
            ));
        }
    }

    for var in config.dependency_include_envs {
        if let Ok(dir) = env::var(var) {
            let root = PathBuf::from(&dir);
            if include_root_has_header(&root, config.required_header) {
                return Ok(NativeDependency {
                    include_paths: vec![root],
                    source: format!("{var}={dir}"),
                });
            }
            attempts.push(format!(
                "{var}={dir}, but {} was not found",
                root.join(config.required_header).display()
            ));
        }
    }

    for var in config.dependency_out_dir_envs {
        if let Ok(out_dir) = env::var(var) {
            let include_root = PathBuf::from(&out_dir).join("include");
            if include_root_has_header(&include_root, config.required_header) {
                return Ok(NativeDependency {
                    include_paths: vec![include_root],
                    source: format!("{var}={out_dir}"),
                });
            }
            attempts.push(format!(
                "{var}={out_dir}, but {} was not found",
                include_root.join(config.required_header).display()
            ));
        }
    }

    if let (Some(prefix), Some(out_dir)) = (config.cargo_target_include_prefix, config.out_dir)
        && let Some(include_root) =
            find_cargo_target_include(out_dir, prefix, config.required_header)
    {
        return Ok(NativeDependency {
            include_paths: vec![include_root.clone()],
            source: format!("Cargo target dir={}", include_root.display()),
        });
    }

    if config.use_pkg_config {
        if let Some(package) = config.pkg_config_package
            && let Some(found) = probe_pkg_config_package(
                package,
                config.emit_cargo_metadata,
                config.print_system_libs,
                &mut attempts,
            )
        {
            return Ok(found);
        }
    } else {
        attempts.push("pkg-config feature disabled".to_string());
    }

    let target_env = cargo_target_env();
    let use_vcpkg = should_use_vcpkg(config.use_vcpkg, config.target_os, &target_env);
    if use_vcpkg {
        if let Some(package) = config.vcpkg_package
            && let Some(found) = probe_vcpkg_package(
                package,
                config.emit_cargo_metadata,
                config.copy_vcpkg_dlls,
                &mut attempts,
            )
        {
            return Ok(found);
        }
    } else {
        push_vcpkg_skip_attempt(
            &mut attempts,
            config.use_vcpkg,
            config.target_os,
            &target_env,
        );
    }

    for candidate in config.known_include_roots {
        if include_root_has_header(candidate, config.required_header) {
            return Ok(NativeDependency {
                include_paths: vec![candidate.clone()],
                source: format!("known include path {}", candidate.display()),
            });
        }
        attempts.push(format!(
            "known include path {} did not contain {}",
            candidate.display(),
            config.required_header
        ));
    }

    Err(format!("Tried {}.", attempts.join("; ")))
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
fn probe_pkg_config_package(
    package: &str,
    emit_cargo_metadata: bool,
    print_system_libs: bool,
    attempts: &mut Vec<String>,
) -> Option<NativeDependency> {
    #[cfg(feature = "pkg-config")]
    {
        let mut pkg = pkg_config::Config::new();
        pkg.cargo_metadata(emit_cargo_metadata)
            .print_system_libs(print_system_libs);
        match pkg.probe(package) {
            Ok(lib) => {
                return Some(NativeDependency {
                    include_paths: lib.include_paths,
                    source: format!("pkg-config ({package})"),
                });
            }
            Err(err) => attempts.push(format!("pkg-config {package}: {err}")),
        }
    }
    #[cfg(not(feature = "pkg-config"))]
    {
        let _ = (package, emit_cargo_metadata, print_system_libs);
        attempts.push("pkg-config support was not compiled into build-support".to_string());
    }
    None
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
fn probe_vcpkg_package(
    package: &str,
    emit_cargo_metadata: bool,
    copy_dlls: bool,
    attempts: &mut Vec<String>,
) -> Option<NativeDependency> {
    #[cfg(feature = "vcpkg")]
    {
        let mut vcpkg_config = vcpkg::Config::new();
        vcpkg_config
            .cargo_metadata(emit_cargo_metadata)
            .copy_dlls(copy_dlls);
        match vcpkg_config.find_package(package) {
            Ok(lib) => {
                return Some(NativeDependency {
                    include_paths: lib.include_paths,
                    source: format!("vcpkg ({package})"),
                });
            }
            Err(err) => attempts.push(format!("vcpkg {package}: {err}")),
        }
    }
    #[cfg(not(feature = "vcpkg"))]
    {
        let _ = (package, emit_cargo_metadata, copy_dlls);
        attempts.push("vcpkg support was not compiled into build-support".to_string());
    }
    None
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
fn include_root_has_header(root: &Path, header: &str) -> bool {
    root.join(header).exists()
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
fn should_use_vcpkg(use_vcpkg: bool, target_os: &str, target_env: &str) -> bool {
    use_vcpkg && target_os == "windows" && target_env == "msvc"
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
fn cargo_target_os() -> String {
    env::var("CARGO_CFG_TARGET_OS").unwrap_or_default()
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
fn cargo_target_env() -> String {
    env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default()
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
fn push_vcpkg_skip_attempt(
    attempts: &mut Vec<String>,
    use_vcpkg: bool,
    target_os: &str,
    target_env: &str,
) {
    if use_vcpkg {
        let target = target_label(target_os, target_env);
        attempts.push(format!(
            "vcpkg skipped for target {target}: automatic vcpkg \
             discovery is only enabled for Windows MSVC targets"
        ));
    } else {
        attempts.push("vcpkg feature disabled".to_string());
    }
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
fn target_label(target_os: &str, target_env: &str) -> String {
    if target_env.is_empty() {
        target_os.to_string()
    } else {
        format!("{target_os}-{target_env}")
    }
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
fn known_sdl3_include_roots(target_os: &str) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if matches!(target_os, "macos" | "ios") {
        roots.extend([
            PathBuf::from("/opt/homebrew/include"),
            PathBuf::from("/usr/local/include"),
            PathBuf::from("/opt/local/include"),
        ]);
    } else if matches!(target_os, "linux" | "freebsd" | "openbsd" | "netbsd") {
        roots.extend([
            PathBuf::from("/usr/include"),
            PathBuf::from("/usr/local/include"),
            PathBuf::from("/opt/local/include"),
        ]);
    }
    roots
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
fn find_cargo_target_include(out_dir: &Path, dir_prefix: &str, header: &str) -> Option<PathBuf> {
    let mut cargo_build_dir = out_dir.to_path_buf();
    while cargo_build_dir
        .file_name()
        .is_some_and(|name| name != "build")
    {
        if !cargo_build_dir.pop() {
            return None;
        }
    }

    if cargo_build_dir
        .file_name()
        .is_none_or(|name| name != "build")
    {
        return None;
    }

    let entries = std::fs::read_dir(&cargo_build_dir).ok()?;
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        if !file_name.to_string_lossy().starts_with(dir_prefix) {
            continue;
        }

        let include_root = entry.path().join("out/include");
        if include_root_has_header(&include_root, header) {
            return Some(include_root);
        }
    }

    None
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
fn emit_pkg_config_rerun_vars(package_env_stem: &str) {
    for var in [
        "PKG_CONFIG",
        "PKG_CONFIG_PATH",
        "PKG_CONFIG_LIBDIR",
        "PKG_CONFIG_SYSROOT_DIR",
    ] {
        println!("cargo:rerun-if-env-changed={var}");
    }
    println!("cargo:rerun-if-env-changed={package_env_stem}_NO_PKG_CONFIG");
}

#[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
fn emit_vcpkg_rerun_vars(port_env_stem: &str) {
    for var in [
        "VCPKG_ROOT",
        "VCPKGRS_TRIPLET",
        "VCPKGRS_DYNAMIC",
        "VCPKGRS_DISABLE",
    ] {
        println!("cargo:rerun-if-env-changed={var}");
    }
    println!("cargo:rerun-if-env-changed=VCPKGRS_NO_{port_env_stem}");
}

#[cfg(all(test, any(feature = "pkg-config", feature = "vcpkg")))]
mod tests {
    use super::{
        NativeDependency, PackageSearchConfig, Sdl3SearchConfig, find_freetype,
        find_sdl3_include_paths, should_use_vcpkg, target_label,
    };
    use std::env;
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static SDL3_ENV_LOCK: Mutex<()> = Mutex::new(());

    const SDL3_ENV_VARS: [&str; 4] = [
        "SDL3_INCLUDE_DIR",
        "DEP_SDL3_INCLUDE_PATH",
        "DEP_SDL3_INCLUDE_DIR",
        "DEP_SDL3_OUT_DIR",
    ];

    const TARGET_ENV_VARS: [&str; 2] = ["CARGO_CFG_TARGET_OS", "CARGO_CFG_TARGET_ENV"];

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

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    fn lock_sdl3_env() -> std::sync::MutexGuard<'static, ()> {
        SDL3_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    struct EnvSnapshot {
        saved: Vec<(&'static str, Option<OsString>)>,
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    impl EnvSnapshot {
        fn clear_sdl3_vars() -> Self {
            Self::save_and_clear(&SDL3_ENV_VARS)
        }

        fn clear_target_vars() -> Self {
            Self::save_and_clear(&TARGET_ENV_VARS)
        }

        fn clear_sdl3_and_target_vars() -> Self {
            Self::save_and_clear(&[
                "SDL3_INCLUDE_DIR",
                "DEP_SDL3_INCLUDE_PATH",
                "DEP_SDL3_INCLUDE_DIR",
                "DEP_SDL3_OUT_DIR",
                "CARGO_CFG_TARGET_OS",
                "CARGO_CFG_TARGET_ENV",
            ])
        }

        fn save_and_clear(vars: &[&'static str]) -> Self {
            let saved = vars
                .iter()
                .copied()
                .map(|var| (var, env::var_os(var)))
                .collect::<Vec<_>>();

            for (var, _) in &saved {
                unsafe {
                    env::remove_var(var);
                }
            }

            Self { saved }
        }
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    impl Drop for EnvSnapshot {
        fn drop(&mut self) {
            for (var, value) in &self.saved {
                unsafe {
                    match value {
                        Some(value) => env::set_var(var, value),
                        None => env::remove_var(var),
                    }
                }
            }
        }
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    fn set_env_var(name: &str, value: &Path) {
        unsafe {
            env::set_var(name, value);
        }
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    fn set_env_str(name: &str, value: &str) {
        unsafe {
            env::set_var(name, value);
        }
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    fn set_cargo_target(os: &str, target_env: &str) {
        set_env_str("CARGO_CFG_TARGET_OS", os);
        set_env_str("CARGO_CFG_TARGET_ENV", target_env);
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    fn write_sdl3_header(include_root: &Path) {
        let header_dir = include_root.join("SDL3");
        std::fs::create_dir_all(&header_dir).unwrap();
        std::fs::write(header_dir.join("SDL.h"), "/* test SDL3 header */\n").unwrap();
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    fn sdl3_search(out_dir: &Path) -> NativeDependency {
        find_sdl3_include_paths(Sdl3SearchConfig {
            out_dir,
            target_os: "unknown-test-os",
            use_pkg_config: false,
            use_vcpkg: false,
        })
        .unwrap()
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    #[test]
    fn freetype_skips_vcpkg_on_non_windows_msvc_targets() {
        let _lock = lock_sdl3_env();
        let _env = EnvSnapshot::clear_target_vars();
        set_cargo_target("linux", "gnu");

        let err = find_freetype(PackageSearchConfig {
            use_pkg_config: false,
            use_vcpkg: true,
            emit_cargo_metadata: false,
        })
        .unwrap_err();

        assert!(err.contains("vcpkg skipped for target linux-gnu"));
        assert!(err.contains("Install FreeType development files with pkg-config metadata."));
        assert!(!err.contains("VCPKG_ROOT"));
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    #[test]
    fn vcpkg_discovery_is_limited_to_windows_msvc_targets() {
        assert!(should_use_vcpkg(true, "windows", "msvc"));
        assert!(!should_use_vcpkg(false, "windows", "msvc"));
        assert!(!should_use_vcpkg(true, "windows", "gnu"));
        assert!(!should_use_vcpkg(true, "linux", "gnu"));
        assert!(!should_use_vcpkg(true, "macos", ""));
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    #[test]
    fn target_label_omits_empty_target_env() {
        assert_eq!(target_label("macos", ""), "macos");
        assert_eq!(target_label("linux", "gnu"), "linux-gnu");
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    #[test]
    fn sdl3_include_dir_takes_precedence_over_dep_vars() {
        let _lock = lock_sdl3_env();
        let _env = EnvSnapshot::clear_sdl3_vars();
        let root = unique_tmp_dir("sdl3-include-dir");
        let include_dir = root.join("explicit");
        let dep_include_dir = root.join("dep-include");
        write_sdl3_header(&include_dir);
        write_sdl3_header(&dep_include_dir);
        set_env_var("SDL3_INCLUDE_DIR", &include_dir);
        set_env_var("DEP_SDL3_INCLUDE_PATH", &dep_include_dir);

        let found = sdl3_search(&root.join("target/debug/build/current/out"));

        assert_eq!(found.include_paths, vec![include_dir.clone()]);
        assert!(found.source.starts_with("SDL3_INCLUDE_DIR="));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    #[test]
    fn sdl3_dep_include_path_takes_precedence_over_dep_include_dir() {
        let _lock = lock_sdl3_env();
        let _env = EnvSnapshot::clear_sdl3_vars();
        let root = unique_tmp_dir("sdl3-dep-include-path");
        let dep_include_path = root.join("dep-include-path");
        let dep_include_dir = root.join("dep-include-dir");
        write_sdl3_header(&dep_include_path);
        write_sdl3_header(&dep_include_dir);
        set_env_var("DEP_SDL3_INCLUDE_PATH", &dep_include_path);
        set_env_var("DEP_SDL3_INCLUDE_DIR", &dep_include_dir);

        let found = sdl3_search(&root.join("target/debug/build/current/out"));

        assert_eq!(found.include_paths, vec![dep_include_path.clone()]);
        assert!(found.source.starts_with("DEP_SDL3_INCLUDE_PATH="));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    #[test]
    fn sdl3_dep_include_dir_takes_precedence_over_dep_out_dir() {
        let _lock = lock_sdl3_env();
        let _env = EnvSnapshot::clear_sdl3_vars();
        let root = unique_tmp_dir("sdl3-dep-include-dir");
        let dep_include_dir = root.join("dep-include-dir");
        let dep_out_dir = root.join("dep-out");
        write_sdl3_header(&dep_include_dir);
        write_sdl3_header(&dep_out_dir.join("include"));
        set_env_var("DEP_SDL3_INCLUDE_DIR", &dep_include_dir);
        set_env_var("DEP_SDL3_OUT_DIR", &dep_out_dir);

        let found = sdl3_search(&root.join("target/debug/build/current/out"));

        assert_eq!(found.include_paths, vec![dep_include_dir.clone()]);
        assert!(found.source.starts_with("DEP_SDL3_INCLUDE_DIR="));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    #[test]
    fn sdl3_dep_out_dir_uses_include_child() {
        let _lock = lock_sdl3_env();
        let _env = EnvSnapshot::clear_sdl3_vars();
        let root = unique_tmp_dir("sdl3-dep-out-dir");
        let dep_out_dir = root.join("dep-out");
        let include_root = dep_out_dir.join("include");
        write_sdl3_header(&include_root);
        set_env_var("DEP_SDL3_OUT_DIR", &dep_out_dir);

        let found = sdl3_search(&root.join("target/debug/build/current/out"));

        assert_eq!(found.include_paths, vec![include_root]);
        assert!(found.source.starts_with("DEP_SDL3_OUT_DIR="));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    #[test]
    fn sdl3_cargo_target_include_is_used_after_env_vars() {
        let _lock = lock_sdl3_env();
        let _env = EnvSnapshot::clear_sdl3_vars();
        let root = unique_tmp_dir("sdl3-target-dir");
        let build_dir = root.join("target").join("debug").join("build");
        let current_out_dir = build_dir.join("current-crate").join("out");
        let sdl3_include_root = build_dir.join("sdl3-sys-test").join("out").join("include");
        std::fs::create_dir_all(&current_out_dir).unwrap();
        write_sdl3_header(&sdl3_include_root);

        let found = sdl3_search(&current_out_dir);

        assert_eq!(found.include_paths, vec![sdl3_include_root.clone()]);
        assert!(found.source.starts_with("Cargo target dir="));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    #[test]
    fn sdl3_include_dir_fails_fast_when_explicit_path_is_invalid() {
        let _lock = lock_sdl3_env();
        let _env = EnvSnapshot::clear_sdl3_vars();
        let root = unique_tmp_dir("sdl3-invalid-explicit");
        let invalid_include_dir = root.join("invalid");
        let dep_include_dir = root.join("dep-include");
        std::fs::create_dir_all(&invalid_include_dir).unwrap();
        write_sdl3_header(&dep_include_dir);
        set_env_var("SDL3_INCLUDE_DIR", &invalid_include_dir);
        set_env_var("DEP_SDL3_INCLUDE_PATH", &dep_include_dir);

        let err = find_sdl3_include_paths(Sdl3SearchConfig {
            out_dir: &root.join("target/debug/build/current/out"),
            target_os: "unknown-test-os",
            use_pkg_config: false,
            use_vcpkg: false,
        })
        .unwrap_err();

        assert!(err.contains("SDL3_INCLUDE_DIR is set"));
        assert!(err.contains("SDL3/SDL.h"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(any(feature = "pkg-config", feature = "vcpkg"))]
    #[test]
    fn sdl3_skips_vcpkg_on_non_windows_msvc_targets() {
        let _lock = lock_sdl3_env();
        let _env = EnvSnapshot::clear_sdl3_and_target_vars();
        set_cargo_target("linux", "gnu");
        let root = unique_tmp_dir("sdl3-linux-vcpkg-skip");

        let err = find_sdl3_include_paths(Sdl3SearchConfig {
            out_dir: &root.join("target/debug/build/current/out"),
            target_os: "linux",
            use_pkg_config: false,
            use_vcpkg: true,
        })
        .unwrap_err();

        assert!(err.contains("vcpkg skipped for target linux-gnu"));
        assert!(!err.contains("vcpkg sdl3:"));

        let _ = std::fs::remove_dir_all(&root);
    }
}
