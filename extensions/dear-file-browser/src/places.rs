use std::path::{Path, PathBuf};

/// A user-defined shortcut location (bookmark) shown in the left "Places" pane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bookmark {
    /// Display name shown in UI.
    pub label: String,
    /// Target directory path.
    pub path: PathBuf,
}

/// Storage for user-defined places (bookmarks).
///
/// This is intentionally filesystem-agnostic. Persistence/serialization will be
/// layered on top in later milestones.
#[derive(Clone, Debug, Default)]
pub struct Places {
    /// Bookmarked directories.
    pub bookmarks: Vec<Bookmark>,
}

impl Places {
    /// Adds a bookmark if the path is not already present.
    pub fn add_bookmark(&mut self, label: impl Into<String>, path: PathBuf) {
        if self.bookmarks.iter().any(|b| b.path == path) {
            return;
        }
        self.bookmarks.push(Bookmark {
            label: label.into(),
            path,
        });
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
}

fn default_label_for_path(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| path.display().to_string())
}

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
}
