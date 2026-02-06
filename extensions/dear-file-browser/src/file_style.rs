use std::path::Path;

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
}

impl StyleMatcher {
    fn matches(&self, name: &str, kind: EntryKind) -> bool {
        match self {
            Self::AnyDir => matches!(kind, EntryKind::Dir),
            Self::AnyFile => matches!(kind, EntryKind::File),
            Self::Extension(ext) => {
                if !matches!(kind, EntryKind::File) {
                    return false;
                }
                let actual = Path::new(name)
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_lowercase());
                match actual {
                    Some(a) => a == ext.as_str(),
                    None => false,
                }
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
#[derive(Clone, Debug, Default)]
pub struct FileStyleRegistry {
    /// Ordered rule list.
    pub rules: Vec<StyleRule>,
}

impl FileStyleRegistry {
    /// Add a rule.
    pub fn push_rule(&mut self, matcher: StyleMatcher, style: FileStyle) {
        self.rules.push(StyleRule { matcher, style });
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
        self.push_rule(StyleMatcher::Extension(ext.as_ref().to_lowercase()), style);
    }

    /// Resolve a style for an entry.
    pub fn style_for(&self, name: &str, kind: EntryKind) -> Option<&FileStyle> {
        self.rules
            .iter()
            .find(|r| r.matcher.matches(name, kind))
            .map(|r| &r.style)
    }
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
        });
        reg.push_extension_style(
            "txt",
            FileStyle {
                text_color: Some([1.0, 0.0, 0.0, 1.0]),
                icon: None,
                tooltip: None,
            },
        );
        let s = reg.style_for("a.txt", EntryKind::File).unwrap();
        assert_eq!(s.text_color, Some([0.0, 1.0, 0.0, 1.0]));
    }
}
