//! Safe, context-bound ImGuiColorTextEdit bindings.

mod context;
mod error;
mod language;
mod palette;
mod text_editor;
mod types;

pub use error::{CteError, CteResult};
pub use language::Language;
pub use palette::{Palette, PaletteColor};
pub use text_editor::{CteUiExt, TextEditor, TextEditorRenderer};
pub use types::{
    MiddleMouseMode, Position, ScrollAlignment, SearchOptions, Selection, SquiggleKind,
    VisualPosition,
};

pub(crate) use dear_imgui_cte_sys as sys;

#[inline]
pub(crate) fn vec2(value: [f32; 2]) -> sys::ImVec2_c {
    sys::ImVec2_c {
        x: value[0],
        y: value[1],
    }
}
