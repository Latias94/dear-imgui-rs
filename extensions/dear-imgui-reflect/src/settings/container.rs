use super::*;

/// Settings controlling how `Vec<T>` containers are edited.
///
/// These correspond conceptually to ImReflect's `insertable` / `removable` /
/// `reorderable` mixins for `std::vector<T>`.
#[derive(Clone, Debug)]
pub struct VecSettings {
    /// Whether insertion of new elements is allowed (via `+` button).
    pub insertable: bool,
    /// Whether removal of elements is allowed (via `-` button).
    pub removable: bool,
    /// Whether elements can be reordered using drag-and-drop handles.
    pub reorderable: bool,
    /// Whether the vector contents are wrapped in a collapsible tree node.
    pub dropdown: bool,
}

impl Default for VecSettings {
    fn default() -> Self {
        Self::editable()
    }
}

impl VecSettings {
    /// Fully editable vector: insertion/removal and reordering enabled, wrapped
    /// in a dropdown. This corresponds to the default ImReflect behavior for
    /// `std::vector<T>`.
    pub fn editable() -> Self {
        Self {
            insertable: true,
            removable: true,
            reorderable: true,
            dropdown: true,
        }
    }

    /// Reorder-only vector: disable insertion/removal, keep drag-to-reorder
    /// handles enabled. This mirrors an ImReflect-style "reorderable only"
    /// configuration.
    pub fn reorder_only() -> Self {
        Self {
            insertable: false,
            removable: false,
            reorderable: true,
            dropdown: true,
        }
    }

    /// Fixed vector: no insertion, removal, or reordering. The contents are
    /// still editable unless combined with `read_only`.
    pub fn fixed() -> Self {
        Self {
            insertable: false,
            removable: false,
            reorderable: false,
            dropdown: true,
        }
    }
}

/// Settings controlling how fixed-size arrays like `[T; N]` are edited.
#[derive(Clone, Debug)]
pub struct ArraySettings {
    /// Whether the array contents are wrapped in a collapsible tree node.
    pub dropdown: bool,
    /// Whether elements can be reordered within the array.
    pub reorderable: bool,
}

impl Default for ArraySettings {
    fn default() -> Self {
        Self {
            dropdown: true,
            reorderable: true,
        }
    }
}

impl ArraySettings {
    /// Fully editable array: elements can be reordered via drag handles.
    pub fn editable() -> Self {
        Self {
            dropdown: true,
            reorderable: true,
        }
    }

    /// Fixed-order array: reordering disabled, but still rendered in a
    /// dropdown. This mirrors an ImReflect-style "no reorder" array.
    pub fn fixed_order() -> Self {
        Self {
            dropdown: true,
            reorderable: false,
        }
    }
}

/// Settings controlling how string-keyed maps like `HashMap<String, V>` and
/// `BTreeMap<String, V>` are edited.
#[derive(Clone, Debug)]
pub struct MapSettings {
    /// Whether the map contents are wrapped in a collapsible tree node.
    pub dropdown: bool,
    /// Whether insertion of new entries is allowed (via `+` button).
    pub insertable: bool,
    /// Whether removal of entries is allowed (via `-` button next to each row).
    pub removable: bool,
    /// Whether entries are rendered inside an ImGui table for better alignment.
    pub use_table: bool,
    /// Number of columns to use when `use_table` is true (at least 3).
    ///
    /// The first column is reserved for the row handle/context menu, the
    /// second for the key, and the third for the value. Larger values widen
    /// the table but currently do not change semantics.
    pub columns: usize,
}

impl Default for MapSettings {
    fn default() -> Self {
        Self::editable()
    }
}

impl MapSettings {
    /// Fully editable map: insertion/removal enabled, optional table layout.
    pub fn editable() -> Self {
        Self {
            dropdown: true,
            insertable: true,
            removable: true,
            use_table: false,
            columns: 3,
        }
    }

    /// Const-map: insertion and removal disabled, values are still editable
    /// unless combined with `read_only`. Uses a table layout by default for
    /// better alignment.
    pub fn const_map() -> Self {
        Self {
            dropdown: true,
            insertable: false,
            removable: false,
            use_table: true,
            columns: 3,
        }
    }
}
