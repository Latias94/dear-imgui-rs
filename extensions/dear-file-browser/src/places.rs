use std::path::{Path, PathBuf};

/// Place entry origin.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PlaceOrigin {
    /// Added by the application/user and intended to be persisted.
    User,
    /// Added by the library/application code (e.g. system drives).
    Code,
}

impl PlaceOrigin {
    fn as_compact_char(self) -> char {
        match self {
            PlaceOrigin::User => 'u',
            PlaceOrigin::Code => 'c',
        }
    }

    fn from_compact_char(ch: char) -> Option<Self> {
        match ch {
            'u' => Some(PlaceOrigin::User),
            'c' => Some(PlaceOrigin::Code),
            _ => None,
        }
    }
}

/// A single place entry shown in the left "Places" pane.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Place {
    /// Display name shown in UI.
    pub label: String,
    /// Target directory path.
    pub path: PathBuf,
    /// Origin of the entry (user vs code).
    pub origin: PlaceOrigin,
    /// Optional UI separator thickness (in pixels).
    ///
    /// When set, this item is treated as a non-interactive separator instead of a navigable place.
    pub separator_thickness: Option<u32>,
}

impl Place {
    /// Creates a new place entry.
    pub fn new(label: impl Into<String>, path: PathBuf, origin: PlaceOrigin) -> Self {
        Self {
            label: label.into(),
            path,
            origin,
            separator_thickness: None,
        }
    }

    /// Convenience constructor for a user-defined place.
    pub fn user(label: impl Into<String>, path: PathBuf) -> Self {
        Self::new(label, path, PlaceOrigin::User)
    }

    /// Convenience constructor for a code-defined place.
    pub fn code(label: impl Into<String>, path: PathBuf) -> Self {
        Self::new(label, path, PlaceOrigin::Code)
    }

    /// Creates a non-interactive separator row.
    pub fn separator(thickness: u32) -> Self {
        Self {
            label: String::new(),
            path: PathBuf::new(),
            origin: PlaceOrigin::User,
            separator_thickness: Some(thickness.max(1)),
        }
    }

    /// Returns whether this item is a separator row.
    pub fn is_separator(&self) -> bool {
        self.separator_thickness.is_some()
    }
}

/// A group of places (e.g. "System", "Bookmarks").
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct PlaceGroup {
    /// Group title shown in UI.
    pub label: String,
    /// Display order used by UI (lower comes first). Ties are resolved by label.
    pub display_order: usize,
    /// Whether this group should be expanded by default.
    pub default_opened: bool,
    /// Places in this group.
    pub places: Vec<Place>,
}

impl PlaceGroup {
    /// Creates a new group.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            display_order: 1000,
            default_opened: false,
            places: Vec::new(),
        }
    }
}

/// Options for serializing places.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct PlacesSerializeOptions {
    /// Whether to include code-defined places.
    pub include_code_places: bool,
}

/// Options for merging places.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct PlacesMergeOptions {
    /// If `true`, imported group metadata (`display_order`, `default_opened`) overwrites the
    /// destination group's metadata when labels match.
    pub overwrite_group_metadata: bool,
}

/// Storage for user-defined and code-defined places.
///
/// This is intentionally dependency-free (no serde). The compact persistence
/// format is designed to be stable and forward-compatible.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Places {
    /// Places groups shown in UI.
    pub groups: Vec<PlaceGroup>,
}

impl Places {
    /// Default system group label.
    pub const SYSTEM_GROUP: &'static str = "System";
    /// Default bookmarks group label.
    pub const BOOKMARKS_GROUP: &'static str = "Bookmarks";

    /// Creates a places store with default groups and system entries.
    pub fn new() -> Self {
        let mut p = Self { groups: Vec::new() };
        p.ensure_default_groups();
        p.refresh_system_places();
        p
    }

    /// Returns `true` if there are no places at all.
    pub fn is_empty(&self) -> bool {
        self.groups.iter().all(|g| g.places.is_empty())
    }

    /// Ensures the default groups exist.
    pub fn ensure_default_groups(&mut self) {
        self.ensure_group(Self::SYSTEM_GROUP);
        self.ensure_group(Self::BOOKMARKS_GROUP);

        if let Some(g) = self
            .groups
            .iter_mut()
            .find(|g| g.label == Self::SYSTEM_GROUP)
        {
            g.display_order = 0;
            g.default_opened = true;
        }
        if let Some(g) = self
            .groups
            .iter_mut()
            .find(|g| g.label == Self::BOOKMARKS_GROUP)
        {
            g.display_order = 10;
            g.default_opened = true;
        }
    }

    /// Rebuilds the system places group (home/root/drives).
    ///
    /// This is a best-effort operation and may produce different results across
    /// platforms.
    pub fn refresh_system_places(&mut self) {
        let group = self.ensure_group_mut(Self::SYSTEM_GROUP);
        group.places.clear();

        if let Some(home) = home_dir() {
            group.places.push(Place::code("Home", home));
        }

        group.places.push(Place::code(
            "Root",
            PathBuf::from(std::path::MAIN_SEPARATOR.to_string()),
        ));

        #[cfg(target_os = "windows")]
        {
            for d in windows_drives() {
                group.places.push(Place::code(d.clone(), PathBuf::from(d)));
            }
        }
    }

    /// Adds a place to a group if its path isn't already present in that group.
    pub fn add_place(&mut self, group_label: impl Into<String>, place: Place) {
        let group_label = group_label.into();
        let group = self.ensure_group_mut(&group_label);
        if !place.is_separator() && group.places.iter().any(|p| p.path == place.path) {
            return;
        }
        group.places.push(place);
    }

    /// Adds a separator row to a group.
    pub fn add_place_separator(&mut self, group_label: impl Into<String>, thickness: u32) {
        self.add_place(group_label, Place::separator(thickness));
    }

    /// Adds a bookmark (user place) into the default bookmarks group.
    pub fn add_bookmark(&mut self, label: impl Into<String>, path: PathBuf) {
        self.add_place(Self::BOOKMARKS_GROUP, Place::user(label, path));
    }

    /// Adds a bookmark using a default label derived from the path.
    pub fn add_bookmark_path(&mut self, path: PathBuf) {
        let label = default_label_for_path(&path);
        self.add_bookmark(label, path);
    }

    /// Removes a place by exact path match from the given group.
    pub fn remove_place_path(&mut self, group_label: &str, path: &Path) -> bool {
        let Some(g) = self.groups.iter_mut().find(|g| g.label == group_label) else {
            return false;
        };
        let Some(i) = g.places.iter().position(|p| p.path == path) else {
            return false;
        };
        g.places.remove(i);
        true
    }

    /// Adds a group if it does not exist.
    /// Returns `true` if the group was added.
    pub fn add_group(&mut self, label: impl Into<String>) -> bool {
        let label = label.into();
        if self.groups.iter().any(|g| g.label == label) {
            return false;
        }
        let mut g = PlaceGroup::new(label);
        let max_order = self
            .groups
            .iter()
            .filter(|g| g.label != Self::SYSTEM_GROUP)
            .map(|g| g.display_order)
            .max()
            .unwrap_or(100);
        g.display_order = max_order.saturating_add(1);
        self.groups.push(g);
        true
    }

    /// Removes a group by exact label match.
    /// Returns `true` if a group was removed.
    pub fn remove_group(&mut self, label: &str) -> bool {
        let Some(i) = self.groups.iter().position(|g| g.label == label) else {
            return false;
        };
        self.groups.remove(i);
        true
    }

    /// Renames a group by exact label match.
    /// Returns `true` if the group was found and renamed.
    pub fn rename_group(&mut self, from: &str, to: impl Into<String>) -> bool {
        let to = to.into();
        if self.groups.iter().any(|g| g.label == to) {
            return false;
        }
        let Some(g) = self.groups.iter_mut().find(|g| g.label == from) else {
            return false;
        };
        g.label = to;
        true
    }

    /// Edits a place identified by its current path within a group.
    ///
    /// Returns `true` if a place was found and updated.
    pub fn edit_place_by_path(
        &mut self,
        group_label: &str,
        from_path: &Path,
        new_label: impl Into<String>,
        new_path: PathBuf,
    ) -> bool {
        let Some(g) = self.groups.iter_mut().find(|g| g.label == group_label) else {
            return false;
        };
        let Some(i) = g.places.iter().position(|p| p.path == from_path) else {
            return false;
        };
        g.places[i].label = new_label.into();
        g.places[i].path = new_path;
        true
    }

    /// Serializes places into a compact, line-based format.
    ///
    /// Format (v1):
    /// - First non-empty line: `v1`
    /// - Group header: `g<TAB>group<TAB>order<TAB>opened`
    /// - Place entry: `p<TAB>group<TAB>origin<TAB>label<TAB>path`
    /// - Separator: `s<TAB>group<TAB>thickness`
    ///
    /// All string fields are escaped and separated by tabs.
    pub fn serialize_compact(&self, opts: PlacesSerializeOptions) -> String {
        let mut out = String::new();
        out.push_str("v1\n");

        let mut groups = self.groups.clone();
        groups.retain(|g| g.label != Self::SYSTEM_GROUP);
        groups.sort_by(|a, b| {
            a.display_order
                .cmp(&b.display_order)
                .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
        });

        for g in &groups {
            out.push_str("g\t");
            out.push_str(&escape_field(&g.label));
            out.push('\t');
            out.push_str(&g.display_order.to_string());
            out.push('\t');
            out.push_str(if g.default_opened { "1" } else { "0" });
            out.push('\n');

            for p in &g.places {
                if let Some(thickness) = p.separator_thickness {
                    out.push_str("s\t");
                    out.push_str(&escape_field(&g.label));
                    out.push('\t');
                    out.push_str(&thickness.to_string());
                    out.push('\n');
                    continue;
                }
                if !opts.include_code_places && p.origin == PlaceOrigin::Code {
                    continue;
                }
                out.push_str("p\t");
                out.push_str(&escape_field(&g.label));
                out.push('\t');
                out.push(p.origin.as_compact_char());
                out.push('\t');
                out.push_str(&escape_field(&p.label));
                out.push('\t');
                out.push_str(&escape_field(&p.path.display().to_string()));
                out.push('\n');
            }
        }
        out
    }

    /// Deserializes places from the compact format produced by
    /// [`Places::serialize_compact`].
    pub fn deserialize_compact(input: &str) -> Result<Self, PlacesDeserializeError> {
        let mut places = Places { groups: Vec::new() };
        let mut version_ok = false;

        for (line_idx, raw_line) in input.lines().enumerate() {
            let line_no = line_idx + 1;
            let line = raw_line.trim_end_matches('\r');
            if line.trim().is_empty() {
                continue;
            }

            if !version_ok {
                if line == "v1" {
                    version_ok = true;
                    continue;
                }
                return Err(PlacesDeserializeError {
                    line: line_no,
                    message: "missing or unsupported version token".into(),
                });
            }

            let (kind, rest) = line
                .split_once('\t')
                .ok_or_else(|| PlacesDeserializeError {
                    line: line_no,
                    message: "missing kind field".into(),
                })?;

            match kind {
                "g" => {
                    let (raw_group, rest) =
                        rest.split_once('\t')
                            .ok_or_else(|| PlacesDeserializeError {
                                line: line_no,
                                message: "missing group field".into(),
                            })?;
                    let (raw_order, raw_opened) =
                        rest.split_once('\t')
                            .ok_or_else(|| PlacesDeserializeError {
                                line: line_no,
                                message: "missing group metadata fields".into(),
                            })?;
                    let group_label =
                        unescape_field(raw_group).map_err(|msg| PlacesDeserializeError {
                            line: line_no,
                            message: format!("group: {msg}"),
                        })?;
                    if group_label == Places::SYSTEM_GROUP {
                        continue;
                    }
                    let order = raw_order
                        .parse::<usize>()
                        .map_err(|_| PlacesDeserializeError {
                            line: line_no,
                            message: "invalid group order field".into(),
                        })?;
                    let opened = match raw_opened {
                        "0" => false,
                        "1" => true,
                        _ => {
                            return Err(PlacesDeserializeError {
                                line: line_no,
                                message: "invalid group opened field".into(),
                            });
                        }
                    };
                    let group = places.ensure_group_mut(&group_label);
                    group.display_order = order;
                    group.default_opened = opened;
                }
                "p" => {
                    let (raw_group, rest) =
                        rest.split_once('\t')
                            .ok_or_else(|| PlacesDeserializeError {
                                line: line_no,
                                message: "missing group field".into(),
                            })?;
                    let (raw_origin, rest) =
                        rest.split_once('\t')
                            .ok_or_else(|| PlacesDeserializeError {
                                line: line_no,
                                message: "missing origin field".into(),
                            })?;
                    let (raw_label, raw_path) =
                        rest.split_once('\t')
                            .ok_or_else(|| PlacesDeserializeError {
                                line: line_no,
                                message: "missing label/path fields".into(),
                            })?;

                    let group_label =
                        unescape_field(raw_group).map_err(|msg| PlacesDeserializeError {
                            line: line_no,
                            message: format!("group: {msg}"),
                        })?;
                    if group_label == Places::SYSTEM_GROUP {
                        continue;
                    }
                    let origin_ch =
                        raw_origin
                            .chars()
                            .next()
                            .ok_or_else(|| PlacesDeserializeError {
                                line: line_no,
                                message: "empty origin field".into(),
                            })?;
                    let origin = PlaceOrigin::from_compact_char(origin_ch).ok_or_else(|| {
                        PlacesDeserializeError {
                            line: line_no,
                            message: "invalid origin field".into(),
                        }
                    })?;
                    let label =
                        unescape_field(raw_label).map_err(|msg| PlacesDeserializeError {
                            line: line_no,
                            message: format!("label: {msg}"),
                        })?;
                    let path_s =
                        unescape_field(raw_path).map_err(|msg| PlacesDeserializeError {
                            line: line_no,
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
                    places.add_place(group_label, Place::new(label, path, origin));
                }
                "s" => {
                    let (raw_group, raw_thickness) =
                        rest.split_once('\t')
                            .ok_or_else(|| PlacesDeserializeError {
                                line: line_no,
                                message: "missing group/thickness fields".into(),
                            })?;
                    let group_label =
                        unescape_field(raw_group).map_err(|msg| PlacesDeserializeError {
                            line: line_no,
                            message: format!("group: {msg}"),
                        })?;
                    if group_label == Places::SYSTEM_GROUP {
                        continue;
                    }
                    let thickness =
                        raw_thickness
                            .parse::<u32>()
                            .map_err(|_| PlacesDeserializeError {
                                line: line_no,
                                message: "invalid separator thickness field".into(),
                            })?;
                    places.add_place_separator(group_label, thickness);
                }
                other => {
                    return Err(PlacesDeserializeError {
                        line: line_no,
                        message: format!("unknown kind `{other}`"),
                    });
                }
            }
        }

        if !version_ok {
            return Err(PlacesDeserializeError {
                line: 1,
                message: "missing or unsupported version token".into(),
            });
        }

        // Always ensure System + Bookmarks groups exist, and refresh System to match the
        // current machine (drives, home, etc.).
        places.ensure_default_groups();
        places.refresh_system_places();
        Ok(places)
    }

    /// Merges another places store into `self`.
    ///
    /// - Groups are merged by label.
    /// - Places are merged via [`Places::add_place`] (dedupes by path; separators are always added).
    /// - The system group is never merged (it is machine-specific).
    pub fn merge_from(&mut self, other: Places, opts: PlacesMergeOptions) {
        for g in other.groups {
            if g.label == Self::SYSTEM_GROUP {
                continue;
            }

            let label = g.label.clone();
            let dst = self.ensure_group_mut(&label);
            if opts.overwrite_group_metadata {
                dst.display_order = g.display_order;
                dst.default_opened = g.default_opened;
            }
            for place in g.places {
                self.add_place(label.clone(), place);
            }
        }
    }

    fn ensure_group(&mut self, label: &str) {
        if self.groups.iter().any(|g| g.label == label) {
            return;
        }
        self.groups.push(PlaceGroup::new(label));
    }

    fn ensure_group_mut(&mut self, label: &str) -> &mut PlaceGroup {
        if !self.groups.iter().any(|g| g.label == label) {
            self.groups.push(PlaceGroup::new(label));
        }
        self.groups
            .iter_mut()
            .find(|g| g.label == label)
            .expect("group exists")
    }
}

impl Default for Places {
    fn default() -> Self {
        Places::new()
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
}

#[cfg(target_os = "windows")]
fn windows_drives() -> Vec<String> {
    let mut v = Vec::new();
    for c in b'A'..=b'Z' {
        let s = format!("{}:\\", c as char);
        if Path::new(&s).exists() {
            v.push(s);
        }
    }
    v
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
        let mut p = Places::new();
        p.add_bookmark("A", PathBuf::from("x"));
        p.add_bookmark("B", PathBuf::from("x"));
        let g = p
            .groups
            .iter()
            .find(|g| g.label == Places::BOOKMARKS_GROUP)
            .unwrap();
        assert_eq!(g.places.len(), 1);
        assert_eq!(g.places[0].label, "A");
    }

    #[test]
    fn remove_bookmark_by_path() {
        let mut p = Places::new();
        p.add_bookmark("A", PathBuf::from("x"));
        assert!(p.remove_place_path(Places::BOOKMARKS_GROUP, Path::new("x")));
        assert!(!p.remove_place_path(Places::BOOKMARKS_GROUP, Path::new("x")));
    }

    #[test]
    fn compact_roundtrip_escapes_fields() {
        let mut p = Places::new();
        p.groups.clear();
        p.add_place("G\t1", Place::user("a\tb", PathBuf::from("C:\\x\\y")));
        p.add_place("G\t2", Place::code("line\nbreak", PathBuf::from("/tmp/z")));
        p.add_place_separator("G\t2", 2);
        let s = p.serialize_compact(PlacesSerializeOptions {
            include_code_places: true,
        });

        let p2 = Places::deserialize_compact(&s).unwrap();
        let g1 = p2.groups.iter().find(|g| g.label == "G\t1").unwrap();
        assert_eq!(g1.places[0].label, "a\tb");
        let g2 = p2.groups.iter().find(|g| g.label == "G\t2").unwrap();
        assert_eq!(g2.places[0].label, "line\nbreak");
        assert!(g2.places.iter().any(|p| p.is_separator()));
    }

    #[test]
    fn compact_parse_rejects_missing_separator() {
        let err = Places::deserialize_compact("abc").unwrap_err();
        assert_eq!(err.line, 1);
    }

    #[test]
    fn compact_roundtrip_preserves_group_metadata() {
        let mut p = Places::new();
        p.groups.clear();
        p.add_group("G1");
        p.add_group("G2");
        p.ensure_default_groups();
        let g1 = p.groups.iter_mut().find(|g| g.label == "G1").unwrap();
        g1.display_order = 42;
        g1.default_opened = true;

        let s = p.serialize_compact(PlacesSerializeOptions {
            include_code_places: false,
        });
        let p2 = Places::deserialize_compact(&s).unwrap();
        let g1 = p2.groups.iter().find(|g| g.label == "G1").unwrap();
        assert_eq!(g1.display_order, 42);
        assert!(g1.default_opened);
    }

    #[test]
    fn group_add_rename_remove_roundtrip() {
        let mut p = Places::new();
        assert!(p.add_group("MyGroup"));
        assert!(!p.add_group("MyGroup"));

        assert!(p.rename_group("MyGroup", "MyGroup2"));
        assert!(!p.rename_group("MyGroup2", Places::SYSTEM_GROUP));
        assert!(!p.rename_group("Missing", "X"));

        assert!(p.remove_group("MyGroup2"));
        assert!(!p.remove_group("MyGroup2"));
    }

    #[test]
    fn edit_place_by_path_updates_label_and_path() {
        let mut p = Places::new();
        p.groups.clear();
        p.add_place("G", Place::user("A", PathBuf::from("/tmp/a")));
        assert!(p.edit_place_by_path("G", Path::new("/tmp/a"), "B", PathBuf::from("/tmp/b")));
        let g = p.groups.iter().find(|g| g.label == "G").unwrap();
        assert_eq!(g.places.len(), 1);
        assert_eq!(g.places[0].label, "B");
        assert_eq!(g.places[0].path, PathBuf::from("/tmp/b"));
        assert!(!p.edit_place_by_path(
            "G",
            Path::new("/tmp/missing"),
            "C",
            PathBuf::from("/tmp/c")
        ));
    }
}
