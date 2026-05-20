//! Standard widgets
//!
//! Collection of common Dear ImGui widgets exposed with an idiomatic Rust
//! API. Most widgets follow a small builder pattern for configuration, and
//! also provide convenience methods on [`Ui`].
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
//! `selectable`, `slider`, `tab`, `table`, `text`, `tooltip`, `tree`.
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
pub mod text;
pub mod tooltip;
pub mod tree;

// Re-export important types
pub use popup::{PopupContextFlags, PopupOpenFlags, PopupQueryFlags};
pub use table::{TableBgTarget, TableBuilder, TableColumnSetup};

// Widget implementations
pub use self::button::*;
pub use self::color::*;
pub use self::combo::*;
pub use self::drag::*;
pub use self::image::*;
pub use self::input::*;
pub use self::list_box::*;
pub use self::menu::*;
pub use self::misc::*;
pub use self::multi_select::*;
pub use self::plot::*;
pub use self::popup::*;
pub use self::progress::*;
pub use self::selectable::*;
pub use self::slider::*;
pub use self::tab::*;
pub use self::table::*;
pub use self::tooltip::*;
pub use self::tree::*;

// ButtonFlags is defined in misc.rs and re-exported
