//! Immediate drawing helpers (DrawList)
//!
//! Safe wrappers over Dear ImGui draw lists plus optional low-level primitives
//! for custom geometry. Prefer high-level builders; resort to `prim_*` only
//! when you need exact control and understand the safety requirements.
//!
//! Example (basic drawing):
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! let dl = ui.get_window_draw_list();
//! dl.add_line([10.0, 10.0], [100.0, 100.0], [1.0, 1.0, 1.0, 1.0])
//!     .thickness(2.0)
//!     .build();
//! dl.add_text([12.0, 12.0], [1.0, 0.8, 0.2, 1.0], "Hello DrawList");
//! ```
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions,
    clippy::unnecessary_cast
)]
mod callback;
mod channels;
mod color;
mod counts;
mod list;
mod primitives;
mod util;

#[allow(unused_imports)]
pub use callback::Callback;
#[allow(unused_imports)]
pub use channels::ChannelsSplit;
pub use color::ImColor32;
pub use counts::{
    DrawCornerFlags, DrawListFlags, DrawNgonSegmentCount, DrawSegmentCount, PolylineFlags,
};
#[allow(unused_imports)]
pub use list::DrawListClipRectToken;
pub use list::{DrawListMut, DrawListTextureToken};
#[allow(unused_imports)]
pub use primitives::{BezierCurve, Circle, Line, Polyline, Rect, Triangle};
