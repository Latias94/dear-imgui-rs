//! Shared native build policy. Artifact ABI validation remains with each sys crate.

use std::env;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// How a native core or extension obtains its library, independently of bindings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NativeBuildMode {
    #[default]
    Auto,
    Source,
    Prebuilt,
    BindingsOnly,
}

impl FromStr for NativeBuildMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "auto" => Ok(Self::Auto),
            "source" => Ok(Self::Source),
            "prebuilt" => Ok(Self::Prebuilt),
            "bindings-only" => Ok(Self::BindingsOnly),
            _ => Err(format!(
                "invalid build mode {value:?}; expected auto, source, prebuilt, or bindings-only"
            )),
        }
    }
}

/// An explicit selector is authoritative: acquisition errors never mean a cache miss.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrebuiltSource {
    LibraryDir(PathBuf),
    Archive(String),
    PackageDir(PathBuf),
}

impl PrebuiltSource {
    /// Stage an explicit selection. The caller must validate its artifact profile before linking.
    pub fn acquire(
        &self,
        cache_root: &Path,
        library_name: &str,
        target_env: &str,
        archive_names: &[String],
    ) -> Result<PathBuf, String> {
        let directory = match self {
            Self::LibraryDir(directory) => directory.clone(),
            Self::Archive(source) => {
                if !source.starts_with("http://") && !source.starts_with("https://") {
                    println!(
                        "cargo:rerun-if-changed={}",
                        source.strip_prefix("file://").unwrap_or(source)
                    );
                }
                crate::download_prebuilt(
                    &explicit_cache_root(cache_root, source),
                    source,
                    library_name,
                    target_env,
                )
                .map_err(|error| format!("explicit prebuilt {source:?}: {error}"))?
            }
            Self::PackageDir(directory) => {
                println!("cargo:rerun-if-changed={}", directory.display());
                let archive = archive_names.iter().map(|name| directory.join(name))
                    .find(|path| path.is_file())
                    .ok_or_else(|| format!("explicit package directory {} contains none of the expected archives: {}", directory.display(), archive_names.join(", ")))?;
                crate::extract_archive_to_cache(
                    &archive,
                    &explicit_cache_root(cache_root, &archive.to_string_lossy()),
                    library_name,
                )
                .map_err(|error| {
                    format!("explicit prebuilt archive {}: {error}", archive.display())
                })?
            }
        };
        require_library_dir(
            &directory,
            library_name,
            "explicit prebuilt library directory",
        )?;
        Ok(directory)
    }
}

/// Value inputs keep policy tests independent of the process environment.
#[derive(Clone, Debug, Default)]
struct NativeBuildOptions {
    mode: Option<NativeBuildMode>,
    skip_cc: bool,
    force_source: bool,
    source_required: bool,
    prebuilt_enabled: bool,
    prebuilt: Option<PrebuiltSource>,
    legacy_test_engine_skip: bool,
}

/// A normalized request; only `BindingsOnly` can intentionally omit a native library.
#[derive(Clone, Debug)]
pub struct NativeBuildPlan {
    mode: NativeBuildMode,
    prebuilt: Option<PrebuiltSource>,
}

impl NativeBuildOptions {
    fn is_legacy_test_engine_check(&self) -> bool {
        self.legacy_test_engine_skip
            && self.skip_cc
            && self.mode.is_none()
            && !self.force_source
            && !self.prebuilt_enabled
            && self.prebuilt.is_none()
    }

    fn requires_source(&self) -> bool {
        (self.source_required || self.force_source) && !self.is_legacy_test_engine_check()
    }

    fn resolve(self, prefix: &str) -> Result<NativeBuildPlan, String> {
        if self.mode.is_some() && (self.skip_cc || self.force_source) {
            return Err(format!(
                "{prefix}_BUILD_MODE cannot be combined with enabled {prefix}_SKIP_CC or {prefix}_FORCE_BUILD; use one build-mode interface"
            ));
        }
        let source_required = self.requires_source();
        if source_required && self.skip_cc {
            return Err(format!(
                "{prefix}_SKIP_CC conflicts with a required source build (build-from-source, a source-only feature, packaging, or {prefix}_FORCE_BUILD)"
            ));
        }
        let requested = self.mode.unwrap_or_else(|| {
            if self.skip_cc {
                if self.prebuilt.is_some() || self.prebuilt_enabled {
                    NativeBuildMode::Prebuilt
                } else {
                    NativeBuildMode::BindingsOnly
                }
            } else {
                NativeBuildMode::Auto
            }
        });
        if source_required
            && matches!(
                requested,
                NativeBuildMode::Prebuilt | NativeBuildMode::BindingsOnly
            )
        {
            return Err(format!(
                "{prefix}_BUILD_MODE={requested:?} conflicts with a required source build; disable the source-only requirement or choose source"
            ));
        }
        if matches!(
            requested,
            NativeBuildMode::Source | NativeBuildMode::BindingsOnly
        ) && self.prebuilt.is_some()
        {
            return Err(format!(
                "{prefix}_BUILD_MODE={requested:?} cannot be combined with a prebuilt selector ({prefix}_LIB_DIR, {prefix}_PREBUILT_URL, or {prefix}_PACKAGE_DIR)"
            ));
        }
        let mode = if source_required {
            NativeBuildMode::Source
        } else {
            requested
        };
        if mode != NativeBuildMode::Source && !self.prebuilt_enabled {
            let needs_archive_support = match &self.prebuilt {
                Some(PrebuiltSource::PackageDir(_)) => true,
                Some(PrebuiltSource::Archive(source)) => {
                    source.starts_with("http://")
                        || source.starts_with("https://")
                        || source.ends_with(".tar.gz")
                        || source.ends_with(".tgz")
                }
                _ => false,
            };
            if needs_archive_support {
                return Err(format!(
                    "{prefix}: explicit archive/download selection requires feature `prebuilt`; enable it or use {prefix}_LIB_DIR with an extracted artifact"
                ));
            }
        }
        Ok(NativeBuildPlan {
            mode,
            prebuilt: if mode == NativeBuildMode::Source {
                None
            } else {
                self.prebuilt
            },
        })
    }
}

impl NativeBuildPlan {
    /// The resolved policy; callers cannot construct or mutate an unchecked plan.
    pub fn mode(&self) -> NativeBuildMode {
        self.mode
    }

    /// The authoritative artifact selector, if the resolved policy uses one.
    pub fn prebuilt_source(&self) -> Option<&PrebuiltSource> {
        self.prebuilt.as_ref()
    }

    /// Read legacy mode flags once, preserving source-feature priority in automatic mode.
    pub fn from_env(
        prefix: &str,
        source_required: bool,
        prebuilt_enabled: bool,
    ) -> Result<Self, String> {
        Self::read_env(prefix, source_required, prebuilt_enabled, false)
    }

    /// Resolve core settings, including its established dual-skip Test Engine check.
    /// The caller must disable that compatibility route for packaging and ABI probes.
    pub fn from_core_env(
        source_required: bool,
        prebuilt_enabled: bool,
        allow_test_engine_check: bool,
    ) -> Result<Self, String> {
        Self::read_env(
            "IMGUI_SYS",
            source_required,
            prebuilt_enabled,
            allow_test_engine_check,
        )
    }

    fn read_env(
        prefix: &str,
        source_required: bool,
        prebuilt_enabled: bool,
        allow_test_engine_check: bool,
    ) -> Result<Self, String> {
        for suffix in [
            "BUILD_MODE",
            "SKIP_CC",
            "FORCE_BUILD",
            "LIB_DIR",
            "PREBUILT_URL",
            "PACKAGE_DIR",
            "USE_PREBUILT",
        ] {
            println!("cargo:rerun-if-env-changed={prefix}_{suffix}");
        }
        let read_string = |suffix| -> Result<Option<String>, String> {
            let name = format!("{prefix}_{suffix}");
            match env::var(&name) {
                Ok(value) => Ok(Some(value)),
                Err(env::VarError::NotPresent) => Ok(None),
                Err(error) => Err(format!("{name}: {error}")),
            }
        };
        let mode = read_string("BUILD_MODE")?
            .map(|value| {
                value
                    .parse()
                    .map_err(|error| format!("{prefix}_BUILD_MODE: {error}"))
            })
            .transpose()?;
        let skip_cc = parse_flag(
            &format!("{prefix}_SKIP_CC"),
            read_string("SKIP_CC")?.as_deref(),
        )?;
        let force_source = parse_flag(
            &format!("{prefix}_FORCE_BUILD"),
            read_string("FORCE_BUILD")?.as_deref(),
        )?;
        let use_prebuilt = parse_flag(
            &format!("{prefix}_USE_PREBUILT"),
            read_string("USE_PREBUILT")?.as_deref(),
        )?;
        let mut options = NativeBuildOptions {
            mode,
            skip_cc,
            force_source,
            source_required,
            prebuilt_enabled,
            prebuilt: None,
            legacy_test_engine_skip: allow_test_engine_check
                && crate::parse_bool_env("IMGUI_TEST_ENGINE_SYS_SKIP_CC"),
        };
        let raw_selectors = ["LIB_DIR", "PREBUILT_URL", "PACKAGE_DIR"]
            .map(|suffix| env::var_os(format!("{prefix}_{suffix}")));
        options.legacy_test_engine_skip &= raw_selectors.iter().all(Option::is_none);
        if use_prebuilt
            && !prebuilt_enabled
            && !options.requires_source()
            && !matches!(
                mode,
                Some(NativeBuildMode::Source | NativeBuildMode::BindingsOnly)
            )
        {
            return Err(format!(
                "{prefix}_USE_PREBUILT requires feature `prebuilt`; enable it or use {prefix}_LIB_DIR with an extracted artifact"
            ));
        }
        if options.requires_source() && mode.unwrap_or_default() == NativeBuildMode::Auto {
            return options.resolve(prefix);
        }
        let mut selectors = Vec::new();
        let [lib_dir, prebuilt_url, package_dir] = raw_selectors;
        if let Some(directory) = lib_dir {
            if directory.is_empty() {
                return Err(format!("{prefix}_LIB_DIR must not be empty"));
            }
            selectors.push(PrebuiltSource::LibraryDir(directory.into()));
        }
        if let Some(source) = prebuilt_url {
            let source = source.into_string().map_err(|value| {
                format!(
                    "{prefix}_PREBUILT_URL: {}",
                    env::VarError::NotUnicode(value)
                )
            })?;
            if source.trim().is_empty() {
                return Err(format!("{prefix}_PREBUILT_URL must not be empty"));
            }
            selectors.push(PrebuiltSource::Archive(source));
        }
        if let Some(directory) = package_dir {
            if directory.is_empty() {
                return Err(format!("{prefix}_PACKAGE_DIR must not be empty"));
            }
            selectors.push(PrebuiltSource::PackageDir(directory.into()));
        }
        if selectors.len() > 1 {
            return Err(format!(
                "set only one of {prefix}_LIB_DIR, {prefix}_PREBUILT_URL, and {prefix}_PACKAGE_DIR"
            ));
        }
        options.prebuilt = selectors.pop();
        options.resolve(prefix)
    }

    /// Automatic discovery may miss; a strict prebuilt request must not fall back.
    pub fn finish_prebuilt_lookup(&self, linked: bool) -> Result<bool, String> {
        if !linked && self.mode == NativeBuildMode::Prebuilt {
            Err("no compatible prebuilt library was found; set the matching *_LIB_DIR to the extracted lib directory, provide an archive with feature `prebuilt`, or choose source mode".into())
        } else {
            Ok(linked)
        }
    }
}

fn parse_flag(name: &str, value: Option<&str>) -> Result<bool, String> {
    match value.map(str::to_ascii_lowercase).as_deref() {
        None | Some("0" | "false" | "no" | "off") => Ok(false),
        Some("1" | "true" | "yes" | "on") => Ok(true),
        _ => Err(format!(
            "{name} must be a boolean (1/0, true/false, yes/no, on/off)"
        )),
    }
}

fn explicit_cache_root(root: &Path, source: &str) -> PathBuf {
    let hash = source
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
        });
    root.join(format!("explicit-{hash:016x}"))
}

/// Diagnose an extraction root without silently changing the user's explicit selection.
fn require_library_dir(directory: &Path, library_name: &str, label: &str) -> Result<(), String> {
    let library = directory.join(library_name);
    println!("cargo:rerun-if-changed={}", library.display());
    if library.is_file() {
        return Ok(());
    }
    let hint = if directory.join("lib").join(library_name).is_file() {
        format!("; point to {} instead", directory.join("lib").display())
    } else {
        String::new()
    };
    Err(format!(
        "{label} {} does not contain {library_name}{hint}",
        directory.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> NativeBuildOptions {
        NativeBuildOptions::default()
    }

    #[test]
    fn auto_is_source_capable_without_a_download_opt_in() {
        assert_eq!(
            options().resolve("IMGUI_SYS").unwrap().mode,
            NativeBuildMode::Auto
        );
    }

    #[test]
    fn legacy_skip_without_artifact_intent_is_bindings_only() {
        let input = NativeBuildOptions {
            skip_cc: true,
            ..options()
        };
        assert_eq!(
            input.resolve("IMGUI_SYS").unwrap().mode,
            NativeBuildMode::BindingsOnly
        );
    }

    #[test]
    fn legacy_skip_with_artifact_intent_is_strict_prebuilt() {
        for input in [
            NativeBuildOptions {
                skip_cc: true,
                prebuilt_enabled: true,
                ..options()
            },
            NativeBuildOptions {
                skip_cc: true,
                prebuilt: Some(PrebuiltSource::LibraryDir("artifact/lib".into())),
                ..options()
            },
        ] {
            let plan = input.resolve("IMGUI_SYS").unwrap();
            assert_eq!(plan.mode, NativeBuildMode::Prebuilt);
            assert!(
                plan.finish_prebuilt_lookup(false)
                    .unwrap_err()
                    .contains("no compatible prebuilt")
            );
        }
    }

    #[test]
    fn conflicting_source_and_skip_fail_before_acquisition() {
        for input in [
            NativeBuildOptions {
                skip_cc: true,
                source_required: true,
                ..options()
            },
            NativeBuildOptions {
                skip_cc: true,
                force_source: true,
                ..options()
            },
        ] {
            let error = input.resolve("IMGUI_SYS").unwrap_err();
            assert!(error.contains("SKIP_CC"));
            assert!(error.contains("source"));
        }
    }

    #[test]
    fn test_engine_dual_skip_is_only_a_bindings_only_compatibility_route() {
        let input = NativeBuildOptions {
            skip_cc: true,
            source_required: true,
            legacy_test_engine_skip: true,
            ..options()
        };
        assert_eq!(
            input.clone().resolve("IMGUI_SYS").unwrap().mode(),
            NativeBuildMode::BindingsOnly
        );
        for conflicting in [
            NativeBuildOptions {
                legacy_test_engine_skip: false,
                ..input.clone()
            },
            NativeBuildOptions {
                mode: Some(NativeBuildMode::Auto),
                ..input.clone()
            },
            NativeBuildOptions {
                force_source: true,
                ..input.clone()
            },
            NativeBuildOptions {
                prebuilt_enabled: true,
                ..input.clone()
            },
            NativeBuildOptions {
                prebuilt: Some(PrebuiltSource::LibraryDir("artifact/lib".into())),
                ..input
            },
        ] {
            assert!(conflicting.resolve("IMGUI_SYS").is_err());
        }
    }

    #[test]
    fn archive_selection_requires_download_capability_but_direct_libraries_do_not() {
        for source in [
            PrebuiltSource::Archive("artifact.tar.gz".into()),
            PrebuiltSource::Archive("https://example.invalid/artifact".into()),
            PrebuiltSource::PackageDir("packages".into()),
        ] {
            let input = NativeBuildOptions {
                prebuilt: Some(source),
                ..options()
            };
            assert!(
                input
                    .clone()
                    .resolve("IMGUI_SYS")
                    .unwrap_err()
                    .contains("requires feature `prebuilt`")
            );
            assert!(
                NativeBuildOptions {
                    prebuilt_enabled: true,
                    ..input
                }
                .resolve("IMGUI_SYS")
                .is_ok()
            );
        }
        assert!(
            NativeBuildOptions {
                prebuilt: Some(PrebuiltSource::LibraryDir("artifact/lib".into())),
                ..options()
            }
            .resolve("IMGUI_SYS")
            .is_ok()
        );
    }

    #[test]
    fn unified_source_feature_wins_over_download_and_stale_selector_in_auto() {
        let plan = NativeBuildOptions {
            source_required: true,
            prebuilt_enabled: true,
            prebuilt: Some(PrebuiltSource::LibraryDir("missing".into())),
            ..options()
        }
        .resolve("IMGUI_SYS")
        .unwrap();
        assert_eq!(plan.mode, NativeBuildMode::Source);
        assert!(plan.prebuilt.is_none());
    }

    #[test]
    fn explicit_modes_reject_source_only_requirements() {
        for mode in [NativeBuildMode::Prebuilt, NativeBuildMode::BindingsOnly] {
            assert!(
                NativeBuildOptions {
                    mode: Some(mode),
                    source_required: true,
                    ..options()
                }
                .resolve("IMGUI_SYS")
                .is_err()
            );
        }
    }

    #[test]
    fn explicit_mode_rejects_legacy_mode_flags() {
        assert!(
            NativeBuildOptions {
                mode: Some(NativeBuildMode::Auto),
                skip_cc: true,
                ..options()
            }
            .resolve("IMGUI_SYS")
            .is_err()
        );
        assert!(
            NativeBuildOptions {
                mode: Some(NativeBuildMode::Source),
                force_source: true,
                ..options()
            }
            .resolve("IMGUI_SYS")
            .is_err()
        );
    }

    #[test]
    fn non_prebuilt_modes_reject_explicit_artifact_selectors() {
        for mode in [NativeBuildMode::Source, NativeBuildMode::BindingsOnly] {
            assert!(
                NativeBuildOptions {
                    mode: Some(mode),
                    prebuilt: Some(PrebuiltSource::LibraryDir("artifact/lib".into())),
                    ..options()
                }
                .resolve("IMGUI_SYS")
                .is_err()
            );
        }
    }

    #[test]
    fn modes_and_boolean_values_are_not_presence_tests() {
        assert_eq!(
            "prebuilt".parse::<NativeBuildMode>().unwrap(),
            NativeBuildMode::Prebuilt
        );
        assert!("typo".parse::<NativeBuildMode>().is_err());
        for value in ["0", "false", "False", "no", "No", "off", "Off"] {
            assert!(!parse_flag("IMGUI_SYS_SKIP_CC", Some(value)).unwrap());
        }
        for value in ["1", "true", "True", "yes", "Yes", "on", "On"] {
            assert!(parse_flag("IMGUI_SYS_SKIP_CC", Some(value)).unwrap());
        }
        assert!(parse_flag("IMGUI_SYS_SKIP_CC", Some("maybe")).is_err());
    }

    #[test]
    fn explicit_library_directory_reports_extraction_root_hint() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("lib")).unwrap();
        std::fs::write(root.path().join("lib/libdear_imgui.a"), []).unwrap();
        let error =
            require_library_dir(root.path(), "libdear_imgui.a", "IMGUI_SYS_LIB_DIR").unwrap_err();
        assert!(error.contains("does not contain"));
        assert!(error.contains(&root.path().join("lib").display().to_string()));
        require_library_dir(
            &root.path().join("lib"),
            "libdear_imgui.a",
            "IMGUI_SYS_LIB_DIR",
        )
        .unwrap();
    }

    #[test]
    fn missing_explicit_package_never_becomes_a_download() {
        let root = tempfile::tempdir().unwrap();
        let source = PrebuiltSource::PackageDir(root.path().to_owned());
        let error = source
            .acquire(
                root.path(),
                "libdear_imgui.a",
                "",
                &["missing.tar.gz".into()],
            )
            .unwrap_err();
        assert!(error.contains("missing.tar.gz"));
    }

    #[test]
    fn explicit_cache_identity_does_not_mix_sources_with_the_same_archive_name() {
        let root = Path::new("cache");
        assert_ne!(
            explicit_cache_root(root, "https://one.example/core.tar.gz"),
            explicit_cache_root(root, "https://two.example/core.tar.gz")
        );
        assert_eq!(
            explicit_cache_root(root, "same"),
            explicit_cache_root(root, "same")
        );
    }
}
