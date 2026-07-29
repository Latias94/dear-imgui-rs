use super::{TableColumnFlags, TableColumnIndent, TableColumnWidth};

/// Opaque application data associated with a table column.
///
/// Dear ImGui copies this value unchanged into table sort specifications. Zero is a valid value;
/// it is not treated as an absent ID or hashed through Dear ImGui's widget ID stack.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TableColumnUserData(u32);

impl TableColumnUserData {
    /// Creates opaque table column user data.
    #[inline]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the unchanged opaque value.
    #[inline]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl From<u32> for TableColumnUserData {
    #[inline]
    fn from(value: u32) -> Self {
        Self::new(value)
    }
}

impl From<TableColumnUserData> for u32 {
    #[inline]
    fn from(value: TableColumnUserData) -> Self {
        value.get()
    }
}

/// Table column setup information
#[derive(Clone, Debug)]
pub struct TableColumnSetup<Name> {
    pub name: Name,
    pub flags: TableColumnFlags,
    pub width: Option<TableColumnWidth>,
    pub indent: Option<TableColumnIndent>,
    pub user_data: TableColumnUserData,
}

impl<Name> TableColumnSetup<Name> {
    /// Creates a new table column setup
    pub fn new(name: Name) -> Self {
        Self {
            name,
            flags: TableColumnFlags::NONE,
            width: None,
            indent: None,
            user_data: TableColumnUserData::default(),
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

    /// Sets the opaque user data associated with this column.
    ///
    /// Dear ImGui returns the value unchanged in table sort specifications. The default is zero,
    /// which remains ordinary application data rather than an absence sentinel.
    ///
    /// The old `user_id` API intentionally has no compatibility alias because widget IDs and
    /// opaque column data have different semantics.
    ///
    /// ```compile_fail
    /// # use dear_imgui_rs::TableColumnSetup;
    /// let _ = TableColumnSetup::new("column").user_id(1_u32);
    /// ```
    pub fn user_data(mut self, user_data: impl Into<TableColumnUserData>) -> Self {
        self.user_data = user_data.into();
        self
    }

    pub(crate) fn map_name<M>(self, map: impl FnOnce(Name) -> M) -> TableColumnSetup<M> {
        TableColumnSetup {
            name: map(self.name),
            flags: self.flags,
            width: self.width,
            indent: self.indent,
            user_data: self.user_data,
        }
    }
}
