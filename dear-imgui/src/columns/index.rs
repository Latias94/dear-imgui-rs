#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Concrete zero-based legacy Columns API column index.
///
/// This represents a real column only. Dear ImGui's `-1` current-column sentinel
/// is represented by [`OldColumnRef::Current`] and [`OldColumnOffsetRef::Current`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct OldColumnIndex(usize);

impl OldColumnIndex {
    /// The first legacy column.
    pub const ZERO: Self = Self(0);

    /// Create a legacy column index from a Rust `usize`.
    #[inline]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Return the zero-based Rust index.
    #[inline]
    pub const fn get(self) -> usize {
        self.0
    }

    #[inline]
    pub(super) fn into_i32(self, caller: &str) -> i32 {
        i32::try_from(self.0).unwrap_or_else(|_| {
            panic!("{caller} column index exceeded Dear ImGui's i32 range");
        })
    }

    #[inline]
    pub(super) fn from_i32(raw: i32, caller: &str) -> Self {
        assert!(raw >= 0, "{caller} returned a negative column index");
        Self(usize::try_from(raw).expect("non-negative column index must fit usize"))
    }
}

impl From<usize> for OldColumnIndex {
    #[inline]
    fn from(index: usize) -> Self {
        Self::new(index)
    }
}

impl From<OldColumnIndex> for usize {
    #[inline]
    fn from(index: OldColumnIndex) -> Self {
        index.get()
    }
}

impl PartialEq<usize> for OldColumnIndex {
    #[inline]
    fn eq(&self, other: &usize) -> bool {
        self.get() == *other
    }
}

impl PartialEq<OldColumnIndex> for usize {
    #[inline]
    fn eq(&self, other: &OldColumnIndex) -> bool {
        *self == other.get()
    }
}

/// Legacy column selector for APIs that accept Dear ImGui's current-column sentinel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum OldColumnRef {
    /// Use the current legacy column.
    #[default]
    Current,
    /// Use a concrete legacy column index.
    Index(OldColumnIndex),
}

impl OldColumnRef {
    /// Current legacy column.
    pub const CURRENT: Self = Self::Current;

    /// Select a concrete legacy column.
    #[inline]
    pub const fn index(index: OldColumnIndex) -> Self {
        Self::Index(index)
    }
}

impl From<OldColumnIndex> for OldColumnRef {
    #[inline]
    fn from(index: OldColumnIndex) -> Self {
        Self::Index(index)
    }
}

impl From<usize> for OldColumnRef {
    #[inline]
    fn from(index: usize) -> Self {
        Self::Index(OldColumnIndex::new(index))
    }
}

/// Legacy column offset-line selector.
///
/// Offset APIs operate on column boundary lines. Concrete column indices select
/// the start line for that column, while [`OldColumnOffsetRef::Trailing`] selects
/// the right-most line after the final column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum OldColumnOffsetRef {
    /// Use the current legacy column boundary.
    #[default]
    Current,
    /// Use the start boundary for a concrete legacy column.
    Column(OldColumnIndex),
    /// Use the right-most boundary after the final legacy column.
    Trailing,
}

impl OldColumnOffsetRef {
    /// Current legacy column boundary.
    pub const CURRENT: Self = Self::Current;

    /// Right-most boundary after the final legacy column.
    pub const TRAILING: Self = Self::Trailing;

    /// Select the start boundary for a concrete legacy column.
    #[inline]
    pub const fn column(index: OldColumnIndex) -> Self {
        Self::Column(index)
    }
}

impl From<OldColumnIndex> for OldColumnOffsetRef {
    #[inline]
    fn from(index: OldColumnIndex) -> Self {
        Self::Column(index)
    }
}

impl From<usize> for OldColumnOffsetRef {
    #[inline]
    fn from(index: usize) -> Self {
        Self::Column(OldColumnIndex::new(index))
    }
}
