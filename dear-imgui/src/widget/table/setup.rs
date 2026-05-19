use crate::Id;
use crate::widget::table::assert_explicit_user_id;
use crate::widget::{TableColumnFlags, TableColumnIndent, TableColumnWidth};

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

    /// Sets the user ID
    pub fn user_id(mut self, id: Id) -> Self {
        self.user_id = Some(assert_explicit_user_id(id, "TableColumnSetup::user_id()"));
        self
    }
}
