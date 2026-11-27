//! Standard widgets
//!
//! Collection of common Dear ImGui widgets exposed with an idiomatic Rust
//! API. Most widgets follow a small builder pattern for configuration, and
//! also provide convenience methods on [`crate::Ui`].
//!
//! Examples:
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! // Buttons
//! if ui.button("Click me") { /* ... */ }
//!
//! // Sliders
//! let mut value = 0.5f32;
//! ui.slider_f32("Value", &mut value, 0.0, 1.0);
//!
//! // Inputs
//! let mut text = String::new();
//! ui.input_text("Name", &mut text).build();
//! ```
//!
//! Submodules group related widgets: `button`, `color`, `combo`, `drag`,
//! `image`, `input`, `list_box`, `menu`, `misc`, `plot`, `popup`, `progress`,
//! `selectable`, `slider`, `tab`, `table`, `tooltip`, `tree`.
//!

pub mod button;
pub mod color;
pub mod combo;
pub mod drag;
pub mod image;
pub mod input;
pub mod list_box;
pub mod menu;
pub mod misc;
pub mod multi_select;
pub mod plot;
pub mod popup;
pub mod progress;
pub mod selectable;
pub mod slider;
pub mod tab;
pub mod table;
mod text;
pub mod tooltip;
pub mod tree;

pub use self::button::Button;
pub use self::color::{
    ColorButton, ColorButtonFlags, ColorButtonOptions, ColorDataType, ColorDisplayMode, ColorEdit3,
    ColorEdit4, ColorEditFlags, ColorEditOptions, ColorInputMode, ColorPicker3, ColorPicker4,
    ColorPickerDisplayFlags, ColorPickerFlags, ColorPickerMode, ColorPickerOptions,
};
pub use self::combo::{
    ComboBox, ComboBoxFlags, ComboBoxHeight, ComboBoxOptions, ComboBoxPreviewMode, ComboBoxToken,
};
pub use self::drag::{Drag, DragFlags, DragRange};
pub use self::image::{Image, ImageButton};
pub use self::input::{
    HistoryDirection, InputDouble, InputFloat, InputFloat2, InputFloat3, InputFloat4, InputInt,
    InputInt2, InputInt3, InputInt4, InputScalar, InputScalarN, InputText, InputTextCallback,
    InputTextCallbackHandler, InputTextMultiline, InputTextMultilineWithCb, PassthroughCallback,
    TextCallbackData,
};
pub use self::list_box::ListBox;
pub use self::menu::{MainMenuBarToken, MenuBarToken, MenuToken};
pub use self::misc::{
    ButtonFlags, ButtonRepeatToken, DisabledToken, InvisibleButtonMouseButtons,
    InvisibleButtonOptions,
};
pub use self::multi_select::{
    BasicSelection, BasicSelectionIter, KeySetSelection, MultiSelectBoxSelect,
    MultiSelectClickPolicy, MultiSelectFlags, MultiSelectIndexStorage, MultiSelectOptions,
    MultiSelectRangeDirection, MultiSelectRequest, MultiSelectResult, MultiSelectScope,
    MultiSelectScopeKind, MultiSelectUserData,
};
pub use self::plot::{PlotHistogram, PlotLines, PlotValueOffset};
pub use self::popup::{
    ModalPopup, ModalPopupToken, PopupContextFlags, PopupContextMouseButton, PopupContextOptions,
    PopupOpenFlags, PopupQueryFlags, PopupToken,
};
pub use self::progress::ProgressBar;
pub use self::selectable::{Selectable, SelectableFlags};
pub use self::slider::{AngleSlider, Slider, SliderFlags, VerticalSlider};
pub use self::tab::{
    TabBar, TabBarFittingPolicy, TabBarFlags, TabBarOptions, TabBarToken, TabItem, TabItemFlags,
    TabItemOptions, TabItemPlacement, TabItemToken,
};
pub use self::table::{
    ColumnBuilder, SortDirection, TableBgTarget, TableBuilder, TableColumnFlags, TableColumnIndent,
    TableColumnIndex, TableColumnRef, TableColumnSetup, TableColumnSortSpec, TableColumnStateFlags,
    TableColumnUserData, TableColumnWidth, TableContextMenuTarget, TableFlags, TableHeaderData,
    TableHoveredColumn, TableHoveredRow, TableOptions, TableRowFlags, TableRowIndex,
    TableSizingPolicy, TableSortSpecs, TableToken,
};
pub use self::tooltip::TooltipToken;
pub use self::tree::{TreeNode, TreeNodeFlags, TreeNodeId, TreeNodeToken};
