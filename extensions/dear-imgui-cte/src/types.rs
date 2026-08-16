use crate::sys;

/// A document position expressed as a zero-based line and glyph column.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }

    pub(crate) const fn into_raw(self) -> sys::DocPos_c {
        sys::DocPos_c {
            line: self.line,
            index: self.column,
        }
    }

    pub(crate) const fn from_raw(value: sys::DocPos_c) -> Self {
        Self::new(value.line, value.index)
    }
}

/// A half-open document selection.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct Selection {
    pub start: Position,
    pub end: Position,
}

impl Selection {
    pub const fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    pub const fn is_ordered(self) -> bool {
        self.start.line < self.end.line
            || (self.start.line == self.end.line && self.start.column <= self.end.column)
    }

    pub const fn is_empty(self) -> bool {
        self.start.line == self.end.line && self.start.column == self.end.column
    }

    pub(crate) const fn into_raw(self) -> sys::DocSelection_c {
        sys::DocSelection_c {
            start: self.start.into_raw(),
            end: self.end.into_raw(),
        }
    }

    pub(crate) const fn from_raw(value: sys::DocSelection_c) -> Self {
        Self::new(
            Position::from_raw(value.start),
            Position::from_raw(value.end),
        )
    }
}

/// A visual row and column after wrapping and folding are applied.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct VisualPosition {
    pub row: usize,
    pub column: usize,
}

impl VisualPosition {
    pub const fn new(row: usize, column: usize) -> Self {
        Self { row, column }
    }

    pub(crate) const fn into_raw(self) -> sys::VisPos_c {
        sys::VisPos_c {
            row: self.row,
            column: self.column,
        }
    }

    pub(crate) const fn from_raw(value: sys::VisPos_c) -> Self {
        Self::new(value.row, value.column)
    }
}

/// Vertical alignment used when scrolling a line into view.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum ScrollAlignment {
    #[default]
    Top,
    Middle,
    Bottom,
}

impl ScrollAlignment {
    pub(crate) const fn into_raw(self) -> sys::Scroll {
        match self {
            Self::Top => sys::alignTop,
            Self::Middle => sys::alignMiddle,
            Self::Bottom => sys::alignBottom,
        }
    }
}

/// Middle-mouse interaction behavior.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum MiddleMouseMode {
    #[default]
    Pan,
    Scroll,
}

/// Matching options used by occurrence search operations.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct SearchOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
}

/// Application-defined diagnostic category for a squiggle.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct SquiggleKind(usize);

impl SquiggleKind {
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    pub const fn get(self) -> usize {
        self.0
    }
}
