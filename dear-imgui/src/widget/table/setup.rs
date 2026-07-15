use crate::Id;

use super::{TableColumnFlags, TableColumnIndent, TableColumnWidth, assert_explicit_user_id};

/// Table column setup information
#[derive(Clone, Debug)]
pub struct TableColumnSetup<Name> {
    pub name: Name,
    pub flags: TableColumnFlags,
    pub width: Option<TableColumnWidth>,
    pub indent: Option<TableColumnIndent>,
    pub user_id: Option<Id>,
}

impl<Name> TableColumnSetup<Name> {
    /// Creates a new table column setup
    pub fn new(name: Name) -> Self {
        Self {
            name,
            flags: TableColumnFlags::NONE,
            width: None,
            indent: None,
            user_id: None,
        }
    }

    /// Sets the column flags
    pub fn flags(mut self, flags: TableColumnFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Sets a fixed initial column width in pixels.
    pub fn fixed_width(mut self, width: f32) -> Self {
        self.width = Some(TableColumnWidth::Fixed(width));
        self
    }

    /// Sets an initial stretch weight for this column.
    pub fn stretch_weight(mut self, weight: f32) -> Self {
        self.width = Some(TableColumnWidth::Stretch(weight));
        self
    }

    /// Sets this column's indentation policy.
    pub fn indent(mut self, indent: TableColumnIndent) -> Self {
        self.indent = Some(indent);
        self
    }

    /// Enables or disables indentation for this column.
    pub fn indent_enabled(mut self, enabled: bool) -> Self {
        self.indent = Some(if enabled {
            TableColumnIndent::Enable
        } else {
            TableColumnIndent::Disable
        });
        self
    }

    /// Sets the non-zero user ID associated with this column.
    ///
    /// Accepts an [`Id`] or a `u32` value. Dear ImGui returns the value unchanged
    /// in table sort specifications; unlike [`Ui::push_id`](crate::Ui::push_id),
    /// it is not hashed through the ID stack. Omit this method to leave the user
    /// ID unspecified.
    pub fn user_id(mut self, id: impl Into<Id>) -> Self {
        self.user_id = Some(assert_explicit_user_id(id, "TableColumnSetup::user_id()"));
        self
    }

    pub(crate) fn map_name<M>(self, map: impl FnOnce(Name) -> M) -> TableColumnSetup<M> {
        TableColumnSetup {
            name: map(self.name),
            flags: self.flags,
            width: self.width,
            indent: self.indent,
            user_id: self.user_id,
        }
    }
}
