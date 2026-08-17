//! Preview, context-bound ImGuiColorTextEdit bindings through cimCTE.
//!
//! Persistent [`TextEditor`], [`TextDiff`], and [`Notifications`] values are created from a
//! [`dear_imgui_rs::Context`] and rendered through [`CteUiExt`] on that same Context:
//!
//! ```no_run
//! use dear_imgui_cte::{CteUiExt, Language, TextEditor};
//! use dear_imgui_rs::Context;
//!
//! # fn main() -> Result<(), dear_imgui_cte::CteError> {
//! let mut imgui = Context::create();
//! let mut editor = TextEditor::try_create(&imgui)?;
//! editor.set_text("int main() { return 0; }\n")?;
//! editor.set_language(Some(Language::Cpp));
//!
//! let ui = imgui.frame();
//! ui.text_editor(&mut editor, "Source")
//!     .size([640.0, 480.0])
//!     .build()?;
//! # Ok(())
//! # }
//! ```
//!
//! Native owners are neither [`Send`] nor [`Sync`]. A renderer built from another Context is
//! rejected before FFI, and applications should drop every CTE owner before its Context. If the
//! Context has already died, Drop deliberately leaks the native handle rather than dereferencing
//! invalid native state.
//!
//! Stateful callbacks are owned by their editor. Reentering the same mutable callback skips the
//! nested invocation; text filters validate a complete batch before mutation. A callback panic is
//! diagnosed and aborts rather than unwinding through native C++ frames. Trie autocomplete uses
//! upstream-exclusive callback slots, so conflicts are reported instead of silently replacing
//! user callbacks.
//!
//! Add [`dejavu_font_source`] to the managed font atlas before renderer initialization. Do not use
//! the raw cimCTE `SetDejavu` helper from safe integrations: it clears the complete atlas and
//! changes its loader outside renderer texture management.

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
