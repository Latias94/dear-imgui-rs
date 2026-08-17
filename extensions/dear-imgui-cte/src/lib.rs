//! Safe, context-bound ImGuiColorTextEdit bindings.
//!
//! Stateful callbacks are owned by their [`TextEditor`]. Reentering the same mutable
//! callback skips the nested invocation; text filters preserve the original input and
//! return an error. A callback panic is diagnosed and aborts rather than unwinding through
//! native C++ frames.

mod autocomplete;
mod callbacks;
mod context;
mod error;
mod font;
mod language;
mod notifications;
mod palette;
mod text_diff;
mod text_editor;
mod types;
mod ui;
mod validation;

pub use autocomplete::{
    AutocompleteConfig, AutocompleteContext, AutocompleteRequest, TrieAutocomplete,
};
pub use callbacks::{CaretEvent, DecoratorEvent, PopupEvent, TextChange, TextChangeKind};
pub use error::{CteError, CteResult};
pub use font::dejavu_font_source;
pub use language::Language;
pub use notifications::{NotificationType, Notifications, NotificationsRenderer};
pub use palette::{Palette, PaletteColor};
pub use text_diff::{TextDiff, TextDiffRenderer};
pub use text_editor::{TextEditor, TextEditorRenderer};
pub use types::{
    MiddleMouseMode, Position, ScrollAlignment, SearchOptions, Selection, SquiggleKind,
    VisualPosition,
};
pub use ui::CteUiExt;

pub(crate) use dear_imgui_cte_sys as sys;
