use crate::Id;
use crate::ui::Ui;
use crate::widget::table::{TableColumnSetup, assert_explicit_user_id};
use crate::widget::{
    TableColumnFlags, TableColumnIndent, TableColumnWidth, TableFlags, TableOptions,
    TableSizingPolicy,
};
use std::borrow::Cow;

/// Builder for ImGui tables with columns + headers + sizing/freeze options.
#[derive(Debug)]
pub struct TableBuilder<'ui> {
    ui: &'ui Ui,
    id: Cow<'ui, str>,
    flags: TableFlags,
    sizing_policy: Option<TableSizingPolicy>,
    outer_size: [f32; 2],
    inner_width: f32,
    columns: Vec<TableColumnSetup<Cow<'ui, str>>>,
    use_headers: bool,
    freeze: Option<(usize, usize)>,
}

impl<'ui> TableBuilder<'ui> {
    /// Create a new TableBuilder. Prefer using `Ui::table("id")`.
    pub fn new(ui: &'ui Ui, str_id: impl Into<Cow<'ui, str>>) -> Self {
        Self {
            ui,
            id: str_id.into(),
            flags: TableFlags::NONE,
            sizing_policy: None,
            outer_size: [0.0, 0.0],
            inner_width: 0.0,
            columns: Vec::new(),
            use_headers: false,
            freeze: None,
        }
    }

    /// Set table flags
    pub fn flags(mut self, flags: TableFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set the table sizing policy.
    pub fn sizing_policy(mut self, policy: TableSizingPolicy) -> Self {
        self.sizing_policy = Some(policy);
        self
    }

    /// Set outer size (width, height). Default [0,0]
    pub fn outer_size(mut self, size: [f32; 2]) -> Self {
        self.outer_size = size;
        self
    }

    /// Set inner width. Default 0.0
    pub fn inner_width(mut self, width: f32) -> Self {
        self.inner_width = width;
        self
    }

    /// Freeze columns/rows so they stay visible when scrolling
    pub fn freeze(mut self, frozen_cols: usize, frozen_rows: usize) -> Self {
        self.freeze = Some((frozen_cols, frozen_rows));
        self
    }

    /// Begin defining a column using a chainable ColumnBuilder.
    /// Call `.done()` to return to the TableBuilder.
    pub fn column(self, name: impl Into<Cow<'ui, str>>) -> ColumnBuilder<'ui> {
        ColumnBuilder::new(self, name)
    }

    /// Replace columns with provided list
    pub fn columns<Name: Into<Cow<'ui, str>>>(
        mut self,
        cols: impl IntoIterator<Item = TableColumnSetup<Name>>,
    ) -> Self {
        self.columns.clear();
        for c in cols {
            self.columns.push(TableColumnSetup {
                name: c.name.into(),
                flags: c.flags,
                width: c.width,
                indent: c.indent,
                user_id: c.user_id,
            });
        }
        self
    }

    /// Add a single column setup
    pub fn add_column<Name: Into<Cow<'ui, str>>>(mut self, col: TableColumnSetup<Name>) -> Self {
        self.columns.push(TableColumnSetup {
            name: col.name.into(),
            flags: col.flags,
            width: col.width,
            indent: col.indent,
            user_id: col.user_id,
        });
        self
    }

    /// Auto submit headers row from `TableSetupColumn()` entries
    pub fn headers(mut self, enabled: bool) -> Self {
        self.use_headers = enabled;
        self
    }

    /// Build the table and run a closure to emit rows/cells
    pub fn build(self, f: impl FnOnce(&Ui)) {
        let mut options = TableOptions::from(self.flags);
        if let Some(policy) = self.sizing_policy {
            options = options.sizing_policy(policy);
        }
        let Some(token) = self.ui.begin_table_with_sizing(
            self.id.as_ref(),
            self.columns.len(),
            options,
            self.outer_size,
            self.inner_width,
        ) else {
            return;
        };

        if let Some((fc, fr)) = self.freeze {
            self.ui.table_setup_scroll_freeze(fc, fr);
        }

        if !self.columns.is_empty() {
            for col in &self.columns {
                self.ui.table_setup_column_with_indent(
                    col.name.as_ref(),
                    col.flags,
                    col.width,
                    col.indent,
                    col.user_id,
                );
            }
            if self.use_headers {
                self.ui.table_headers_row();
            }
        }

        f(self.ui);

        // drop token to end table
        token.end();
    }
}

/// Chainable builder for a single column. Use `.done()` to return to the table builder.
#[derive(Debug)]
pub struct ColumnBuilder<'ui> {
    parent: TableBuilder<'ui>,
    name: Cow<'ui, str>,
    flags: TableColumnFlags,
    width: Option<TableColumnWidth>,
    indent: Option<TableColumnIndent>,
    user_id: Option<Id>,
}

impl<'ui> ColumnBuilder<'ui> {
    fn new(parent: TableBuilder<'ui>, name: impl Into<Cow<'ui, str>>) -> Self {
        Self {
            parent,
            name: name.into(),
            flags: TableColumnFlags::NONE,
            width: None,
            indent: None,
            user_id: None,
        }
    }

    /// Set column flags.
    pub fn flags(mut self, flags: TableColumnFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set fixed width or stretch weight (ImGui uses same field for both).
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(TableColumnWidth::Fixed(width));
        self
    }

    /// Alias of `width()` to express stretch weights.
    pub fn weight(mut self, weight: f32) -> Self {
        self.width = Some(TableColumnWidth::Stretch(weight));
        self
    }

    /// Set this column's indentation policy.
    pub fn indent(mut self, indent: TableColumnIndent) -> Self {
        self.indent = Some(indent);
        self
    }

    /// Enable or disable indentation for this column.
    pub fn indent_enabled(mut self, enabled: bool) -> Self {
        self.indent = Some(if enabled {
            TableColumnIndent::Enable
        } else {
            TableColumnIndent::Disable
        });
        self
    }

    /// Toggle angled header flag.
    pub fn angled_header(mut self, enabled: bool) -> Self {
        if enabled {
            self.flags.insert(TableColumnFlags::ANGLED_HEADER);
        } else {
            self.flags.remove(TableColumnFlags::ANGLED_HEADER);
        }
        self
    }

    /// Set user id for this column.
    pub fn user_id(mut self, id: Id) -> Self {
        self.user_id = Some(assert_explicit_user_id(id, "ColumnBuilder::user_id()"));
        self
    }

    /// Finish this column and return to the table builder.
    pub fn done(mut self) -> TableBuilder<'ui> {
        self.parent.columns.push(TableColumnSetup {
            name: self.name,
            flags: self.flags,
            width: self.width,
            indent: self.indent,
            user_id: self.user_id,
        });
        self.parent
    }
}
