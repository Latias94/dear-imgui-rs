use std::collections::HashMap;
use std::sync::Arc;

/// Kind of filesystem entry for styling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    /// Directory.
    Dir,
    /// File.
    File,
}

/// A style applied to an entry in the file list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileStyle {
    /// Optional text color (RGBA).
    pub text_color: Option<[f32; 4]>,
    /// Optional icon prefix rendered before the name.
    ///
    /// Note: if your font does not contain emoji glyphs, prefer ASCII icons like `"[DIR]"`.
    pub icon: Option<String>,
    /// Optional tooltip shown when the entry is hovered.
    pub tooltip: Option<String>,
    /// Optional font token resolved by UI via `FileDialogUiState::file_style_fonts`.
    pub font_token: Option<String>,
}

type FileStyleCallbackFn = dyn Fn(&str, EntryKind) -> Option<FileStyle> + Send + Sync + 'static;

/// Callback handle for dynamic style resolution.
#[derive(Clone)]
pub struct FileStyleCallback {
    inner: Arc<FileStyleCallbackFn>,
}

impl std::fmt::Debug for FileStyleCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileStyleCallback").finish_non_exhaustive()
    }
}

impl FileStyleCallback {
    /// Create a callback handle from a closure/function.
    pub fn new<F>(callback: F) -> Self
    where
        F: Fn(&str, EntryKind) -> Option<FileStyle> + Send + Sync + 'static,
    {
        Self {
            inner: Arc::new(callback),
        }
    }

    fn resolve(&self, name: &str, kind: EntryKind) -> Option<FileStyle> {
        (self.inner)(name, kind)
    }
}

/// Matcher for a style rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StyleMatcher {
    /// Match any directory.
    AnyDir,
    /// Match any file.
    AnyFile,
    /// Match a file extension (case-insensitive, without leading dot).
    Extension(String),
    /// Match by exact base name (case-insensitive).
    NameEquals(String),
    /// Match by base name substring (case-insensitive).
    NameContains(String),
    /// Match by base name glob (`*` / `?`, case-insensitive).
    NameGlob(String),
    /// Match by base name regex (case-insensitive).
    ///
    /// IGFD-style wrappers are accepted: `((...))`.
    /// The compiled regex is cached inside `FileStyleRegistry`.
    NameRegex(String),
}

impl StyleMatcher {
    fn matches(
        &self,
        name: &str,
        name_lower: &str,
        kind: EntryKind,
        regex_cache: &mut HashMap<String, regex::Regex>,
    ) -> bool {
        match self {
            Self::AnyDir => matches!(kind, EntryKind::Dir),
            Self::AnyFile => matches!(kind, EntryKind::File),
            Self::Extension(ext) => {
                if !matches!(kind, EntryKind::File) {
                    return false;
                }
                has_extension_suffix(name_lower, ext.as_str())
            }
            Self::NameEquals(needle) => name_lower == needle.as_str(),
            Self::NameContains(needle) => name_lower.contains(needle.as_str()),
            Self::NameGlob(pattern) => wildcard_match(pattern.as_str(), name_lower),
            Self::NameRegex(pattern) => {
                let key = pattern.clone();
                let re = match regex_cache.get(&key) {
                    Some(v) => v,
                    None => {
                        let raw = strip_igfd_regex_wrapping(pattern);
                        let built = regex::RegexBuilder::new(raw).case_insensitive(true).build();
                        let Ok(built) = built else {
                            return false;
                        };
                        regex_cache.insert(key.clone(), built);
                        regex_cache.get(&key).expect("inserted")
                    }
                };
                re.is_match(name)
            }
        }
    }
}

/// A single style rule (matcher + style).
#[derive(Clone, Debug, PartialEq)]
pub struct StyleRule {
    /// Matching predicate.
    pub matcher: StyleMatcher,
    /// Style to apply.
    pub style: FileStyle,
}

/// Registry of file styles applied in the in-UI file browser.
///
/// Rules are evaluated in insertion order. The first matching rule wins.
#[derive(Clone, Debug)]
pub struct FileStyleRegistry {
    /// Ordered rule list.
    pub rules: Vec<StyleRule>,
    /// Optional callback provider for dynamic style resolution.
    pub callback: Option<FileStyleCallback>,
    regex_cache: HashMap<String, regex::Regex>,
}

impl FileStyleRegistry {
    /// Invalidate cached compiled regex patterns.
    ///
    /// This is called automatically by `push_*` methods. If you mutate `rules` directly,
    /// call this before rendering.
    pub fn invalidate_caches(&mut self) {
        self.regex_cache.clear();
    }

    /// Add a rule.
    pub fn push_rule(&mut self, matcher: StyleMatcher, style: FileStyle) {
        let matcher = normalize_matcher(matcher);
        self.rules.push(StyleRule { matcher, style });
        self.invalidate_caches();
    }

    /// Convenience: style all directories.
    pub fn push_dir_style(&mut self, style: FileStyle) {
        self.push_rule(StyleMatcher::AnyDir, style);
    }

    /// Convenience: style all files.
    pub fn push_file_style(&mut self, style: FileStyle) {
        self.push_rule(StyleMatcher::AnyFile, style);
    }

    /// Convenience: style a specific extension (case-insensitive, without leading dot).
    pub fn push_extension_style(&mut self, ext: impl AsRef<str>, style: FileStyle) {
        self.push_rule(StyleMatcher::Extension(ext.as_ref().to_string()), style);
    }

    /// Convenience: style a specific base name (case-insensitive).
    pub fn push_name_style(&mut self, name: impl AsRef<str>, style: FileStyle) {
        self.push_rule(StyleMatcher::NameEquals(name.as_ref().to_string()), style);
    }

    /// Convenience: style entries whose base name contains a substring (case-insensitive).
    pub fn push_name_contains_style(&mut self, needle: impl AsRef<str>, style: FileStyle) {
        self.push_rule(
            StyleMatcher::NameContains(needle.as_ref().to_string()),
            style,
        );
    }

    /// Convenience: style entries whose base name matches a glob (`*` / `?`, case-insensitive).
    pub fn push_name_glob_style(&mut self, pattern: impl AsRef<str>, style: FileStyle) {
        self.push_rule(StyleMatcher::NameGlob(pattern.as_ref().to_string()), style);
    }

    /// Convenience: style entries whose base name matches a regex (case-insensitive).
    ///
    /// IGFD-style wrappers are accepted: `((...))`.
    pub fn push_name_regex_style(&mut self, pattern: impl AsRef<str>, style: FileStyle) {
        self.push_rule(StyleMatcher::NameRegex(pattern.as_ref().to_string()), style);
    }

    /// Set a callback provider for dynamic style resolution.
    pub fn set_callback(&mut self, callback: FileStyleCallback) {
        self.callback = Some(callback);
    }

    /// Clear the callback provider.
    pub fn clear_callback(&mut self) {
        self.callback = None;
    }

    /// Resolve a style for an entry.
    pub fn style_for(&mut self, name: &str, kind: EntryKind) -> Option<&FileStyle> {
        let name_lower = name.to_lowercase();
        let rules = &self.rules;
        let regex_cache = &mut self.regex_cache;
        rules
            .iter()
            .find(|r| r.matcher.matches(name, &name_lower, kind, regex_cache))
            .map(|r| &r.style)
    }

    /// Resolve style as an owned value, checking callback first then static rules.
    pub fn style_for_owned(&mut self, name: &str, kind: EntryKind) -> Option<FileStyle> {
        if let Some(cb) = &self.callback {
            if let Some(style) = cb.resolve(name, kind) {
                return Some(style);
            }
        }
        self.style_for(name, kind).cloned()
    }
}

impl Default for FileStyleRegistry {
    fn default() -> Self {
        Self {
            rules: Vec::new(),
            callback: None,
            regex_cache: HashMap::new(),
        }
    }
}

fn normalize_matcher(m: StyleMatcher) -> StyleMatcher {
    match m {
        StyleMatcher::Extension(ext) => StyleMatcher::Extension(ext.to_lowercase()),
        StyleMatcher::NameEquals(name) => StyleMatcher::NameEquals(name.to_lowercase()),
        StyleMatcher::NameContains(needle) => StyleMatcher::NameContains(needle.to_lowercase()),
        StyleMatcher::NameGlob(pattern) => StyleMatcher::NameGlob(pattern.to_lowercase()),
        StyleMatcher::NameRegex(pattern) => StyleMatcher::NameRegex(pattern),
        other => other,
    }
}

fn strip_igfd_regex_wrapping(pattern: &str) -> &str {
    let t = pattern.trim();
    if t.starts_with("((") && t.ends_with("))") && t.len() >= 4 {
        &t[2..t.len() - 2]
    } else {
        t
    }
}

fn has_extension_suffix(name_lower: &str, ext: &str) -> bool {
    let ext = ext.trim().trim_start_matches('.');
    if ext.is_empty() {
        return false;
    }
    if !name_lower.ends_with(ext) {
        return false;
    }
    let prefix_len = name_lower.len() - ext.len();
    if prefix_len == 0 {
        return false;
    }
    name_lower.as_bytes()[prefix_len - 1] == b'.'
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
    let p = pattern.as_bytes();
    let t = text.as_bytes();
    let (mut pi, mut ti) = (0usize, 0usize);
    let mut star_pi: Option<usize> = None;
    let mut star_ti: usize = 0;

    while ti < t.len() {
        if pi < p.len() && (p[pi] == b'?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
            continue;
        }
        if pi < p.len() && p[pi] == b'*' {
            star_pi = Some(pi);
            pi += 1;
            star_ti = ti;
            continue;
        }
        if let Some(sp) = star_pi {
            pi = sp + 1;
            star_ti += 1;
            ti = star_ti;
            continue;
        }
        return false;
    }

    while pi < p.len() && p[pi] == b'*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_match_is_case_insensitive() {
        let mut reg = FileStyleRegistry::default();
        reg.push_extension_style(
            "PNG",
            FileStyle {
                text_color: Some([1.0, 0.0, 0.0, 1.0]),
                icon: None,
                tooltip: None,
                font_token: None,
            },
        );
        assert!(
            reg.style_for("a.png", EntryKind::File)
                .and_then(|s| s.text_color)
                .is_some()
        );
        assert!(
            reg.style_for("a.PNG", EntryKind::File)
                .and_then(|s| s.text_color)
                .is_some()
        );
        assert!(reg.style_for("a.png", EntryKind::Dir).is_none());
    }

    #[test]
    fn first_match_wins() {
        let mut reg = FileStyleRegistry::default();
        reg.push_file_style(FileStyle {
            text_color: Some([0.0, 1.0, 0.0, 1.0]),
            icon: None,
            tooltip: None,
            font_token: None,
        });
        reg.push_extension_style(
            "txt",
            FileStyle {
                text_color: Some([1.0, 0.0, 0.0, 1.0]),
                icon: None,
                tooltip: None,
                font_token: None,
            },
        );
        let s = reg.style_for("a.txt", EntryKind::File).unwrap();
        assert_eq!(s.text_color, Some([0.0, 1.0, 0.0, 1.0]));
    }

    #[test]
    fn name_contains_matches_case_insensitively() {
        let mut reg = FileStyleRegistry::default();
        reg.push_name_contains_style(
            "read",
            FileStyle {
                text_color: Some([0.0, 0.0, 1.0, 1.0]),
                icon: None,
                tooltip: None,
                font_token: None,
            },
        );
        assert!(reg.style_for("README.md", EntryKind::File).is_some());
        assert!(reg.style_for("readme.txt", EntryKind::File).is_some());
        assert!(reg.style_for("notes.txt", EntryKind::File).is_none());
    }

    #[test]
    fn name_glob_matches_case_insensitively() {
        let mut reg = FileStyleRegistry::default();
        reg.push_name_glob_style(
            "imgui_*.rs",
            FileStyle {
                text_color: Some([0.2, 0.8, 0.2, 1.0]),
                icon: None,
                tooltip: None,
                font_token: None,
            },
        );
        assert!(reg.style_for("imgui_demo.rs", EntryKind::File).is_some());
        assert!(reg.style_for("ImGui_demo.RS", EntryKind::File).is_some());
        assert!(reg.style_for("demo_imgui.rs", EntryKind::File).is_none());
    }

    #[test]
    fn name_regex_matches_case_insensitively() {
        let mut reg = FileStyleRegistry::default();
        reg.push_name_regex_style(
            r"((^imgui_.*\.rs$))",
            FileStyle {
                text_color: Some([0.9, 0.6, 0.2, 1.0]),
                icon: None,
                tooltip: None,
                font_token: None,
            },
        );
        assert!(reg.style_for("imgui_demo.rs", EntryKind::File).is_some());
        assert!(reg.style_for("ImGui_demo.RS", EntryKind::File).is_some());
        assert!(reg.style_for("demo_imgui.rs", EntryKind::File).is_none());
    }

    #[test]
    fn callback_takes_precedence_over_rules() {
        let mut reg = FileStyleRegistry::default();
        reg.push_file_style(FileStyle {
            text_color: Some([0.0, 1.0, 0.0, 1.0]),
            icon: Some("[R]".into()),
            tooltip: None,
            font_token: None,
        });
        reg.set_callback(FileStyleCallback::new(|name, kind| {
            if matches!(kind, EntryKind::File) && name.eq_ignore_ascii_case("a.txt") {
                Some(FileStyle {
                    text_color: Some([1.0, 0.0, 0.0, 1.0]),
                    icon: Some("[C]".into()),
                    tooltip: Some("from callback".into()),
                    font_token: Some("icon".into()),
                })
            } else {
                None
            }
        }));

        let s = reg.style_for_owned("a.txt", EntryKind::File).unwrap();
        assert_eq!(s.icon.as_deref(), Some("[C]"));
        assert_eq!(s.tooltip.as_deref(), Some("from callback"));
        assert_eq!(s.font_token.as_deref(), Some("icon"));
    }

    #[test]
    fn callback_falls_back_to_rules_when_none() {
        let mut reg = FileStyleRegistry::default();
        reg.push_name_style(
            "readme.md",
            FileStyle {
                text_color: Some([0.1, 0.2, 0.3, 1.0]),
                icon: Some("[DOC]".into()),
                tooltip: None,
                font_token: None,
            },
        );
        reg.set_callback(FileStyleCallback::new(|_, _| None));

        let s = reg.style_for_owned("README.md", EntryKind::File).unwrap();
        assert_eq!(s.icon.as_deref(), Some("[DOC]"));
    }
}
