use std::path::{Path, PathBuf};

/// A user-defined shortcut location (bookmark) shown in the left "Places" pane.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Bookmark {
    /// Display name shown in UI.
    pub label: String,
    /// Target directory path.
    pub path: PathBuf,
}

impl Bookmark {
    /// Creates a new bookmark.
    pub fn new(label: impl Into<String>, path: PathBuf) -> Self {
        Self {
            label: label.into(),
            path,
        }
    }
}

/// Storage for user-defined places (bookmarks).
///
/// This is intentionally filesystem-agnostic. Persistence/serialization will be
/// layered on top in later milestones.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct Places {
    /// Bookmarked directories.
    pub bookmarks: Vec<Bookmark>,
}

impl Places {
    /// Creates an empty places store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if there are no bookmarks.
    pub fn is_empty(&self) -> bool {
        self.bookmarks.is_empty()
    }

    /// Adds a bookmark if the path is not already present.
    pub fn add_bookmark(&mut self, label: impl Into<String>, path: PathBuf) {
        if self.bookmarks.iter().any(|b| b.path == path) {
            return;
        }
        self.bookmarks.push(Bookmark::new(label, path));
    }

    /// Adds a bookmark using a default label derived from the path.
    pub fn add_bookmark_path(&mut self, path: PathBuf) {
        let label = default_label_for_path(&path);
        self.add_bookmark(label, path);
    }

    /// Removes a bookmark by exact path match. Returns whether a bookmark was removed.
    pub fn remove_bookmark_path(&mut self, path: &Path) -> bool {
        let Some(i) = self.bookmarks.iter().position(|b| b.path == path) else {
            return false;
        };
        self.bookmarks.remove(i);
        true
    }

    /// Serializes bookmarks into a compact, line-based format.
    ///
    /// Each line is `label<TAB>path` with escaped special characters.
    /// The resulting string can be persisted by the caller.
    ///
    /// This is intentionally dependency-free (no serde). The format is stable
    /// and forward-compatible: unknown/empty lines should be ignored on parse.
    pub fn serialize_compact(&self) -> String {
        let mut out = String::new();
        for bm in &self.bookmarks {
            let label = escape_field(&bm.label);
            let path = escape_field(&bm.path.display().to_string());
            out.push_str(&label);
            out.push('\t');
            out.push_str(&path);
            out.push('\n');
        }
        out
    }

    /// Deserializes bookmarks from the compact format produced by
    /// [`Places::serialize_compact`].
    ///
    /// Invalid lines are ignored; a best-effort `Places` is returned.
    pub fn deserialize_compact(input: &str) -> Result<Self, PlacesDeserializeError> {
        let mut places = Places::new();
        for (line_idx, raw_line) in input.lines().enumerate() {
            let line = raw_line.trim_end_matches('\r');
            if line.trim().is_empty() {
                continue;
            }
            let Some((raw_label, raw_path)) = line.split_once('\t') else {
                return Err(PlacesDeserializeError {
                    line: line_idx + 1,
                    message: "missing TAB separator".into(),
                });
            };

            let label = unescape_field(raw_label).map_err(|msg| PlacesDeserializeError {
                line: line_idx + 1,
                message: format!("label: {msg}"),
            })?;
            let path_s = unescape_field(raw_path).map_err(|msg| PlacesDeserializeError {
                line: line_idx + 1,
                message: format!("path: {msg}"),
            })?;
            let path = PathBuf::from(path_s);
            if path.as_os_str().is_empty() {
                continue;
            }
            let label = if label.trim().is_empty() {
                default_label_for_path(&path)
            } else {
                label
            };
            places.add_bookmark(label, path);
        }
        Ok(places)
    }
}

fn default_label_for_path(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| path.display().to_string())
}

fn escape_field(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(ch),
        }
    }
    out
}

fn unescape_field(s: &str) -> Result<String, &'static str> {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        let Some(esc) = chars.next() else {
            return Err("dangling escape");
        };
        match esc {
            '\\' => out.push('\\'),
            't' => out.push('\t'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            _ => return Err("unknown escape"),
        }
    }
    Ok(out)
}

/// Error returned by [`Places::deserialize_compact`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacesDeserializeError {
    /// 1-based line number where the error happened.
    pub line: usize,
    /// Human-readable error message.
    pub message: String,
}

impl std::fmt::Display for PlacesDeserializeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "places deserialize error at line {}: {}",
            self.line, self.message
        )
    }
}

impl std::error::Error for PlacesDeserializeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_bookmark_dedupes_by_path() {
        let mut p = Places::default();
        p.add_bookmark("A", PathBuf::from("x"));
        p.add_bookmark("B", PathBuf::from("x"));
        assert_eq!(p.bookmarks.len(), 1);
        assert_eq!(p.bookmarks[0].label, "A");
    }

    #[test]
    fn remove_bookmark_by_path() {
        let mut p = Places::default();
        p.add_bookmark("A", PathBuf::from("x"));
        assert!(p.remove_bookmark_path(Path::new("x")));
        assert!(!p.remove_bookmark_path(Path::new("x")));
        assert!(p.bookmarks.is_empty());
    }

    #[test]
    fn compact_roundtrip_escapes_fields() {
        let mut p = Places::new();
        p.add_bookmark("a\tb", PathBuf::from("C:\\x\\y"));
        p.add_bookmark("line\nbreak", PathBuf::from("/tmp/z"));
        let s = p.serialize_compact();

        let p2 = Places::deserialize_compact(&s).unwrap();
        assert_eq!(p2.bookmarks.len(), 2);
        assert_eq!(p2.bookmarks[0].label, "a\tb");
        assert_eq!(p2.bookmarks[1].label, "line\nbreak");
    }

    #[test]
    fn compact_parse_rejects_missing_separator() {
        let err = Places::deserialize_compact("abc").unwrap_err();
        assert_eq!(err.line, 1);
    }
}
