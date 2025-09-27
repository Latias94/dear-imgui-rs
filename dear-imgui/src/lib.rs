//! # Dear ImGui - Rust Bindings with Docking Support
//!
//! High-level Rust bindings for Dear ImGui, the immediate mode GUI library.
//! This crate provides safe, idiomatic Rust bindings with full support for
//! docking and multi-viewport features.
//!
//! ## Features
//!
//! - Safe, idiomatic Rust API
//! - Full docking and multi-viewport support
//! - Builder pattern for widgets
//! - Memory-safe string handling
//! - Integration with modern Rust graphics ecosystems
//!
//! ## Quick Start
//!
//! ```no_run
//! use dear_imgui::*;
//!
//! let mut ctx = Context::create();
//! let ui = ctx.frame();
//!
//! ui.window("Hello World")
//!     .size([300.0, 100.0], Condition::FirstUseEver)
//!     .build(|| {
//!         ui.text("Hello, world!");
//!         ui.text("This is Dear ImGui with docking support!");
//!     });
//! ```
//!
//! ## Math Interop (mint/glam)
//!
//! Many drawing and coordinate-taking APIs accept `impl Into<sys::ImVec2>` so you can pass:
//! - `[f32; 2]` or `(f32, f32)`
//! - `dear_imgui_sys::ImVec2`
//! - `mint::Vector2<f32>` (via `dear-imgui-sys` conversions)
//! - With optional integrations, `glam::Vec2` via your own `Into<ImVec2>`
//!
//! Example:
//! ```no_run
//! # use dear_imgui::*;
//! # fn demo(ui: &Ui) {
//! let dl = ui.get_window_draw_list();
//! dl.add_line([0.0, 0.0], [100.0, 100.0], [1.0, 1.0, 1.0, 1.0]).build();
//! // Also works with mint::Vector2<f32>
//! let a = mint::Vector2 { x: 10.0, y: 20.0 };
//! let b = mint::Vector2 { x: 30.0, y: 40.0 };
//! dl.add_rect(a, b, [1.0, 0.0, 0.0, 1.0]).build();
//! # }
//! ```
//!
//! ## Textures (ImGui 1.92+)
//!
//! You can pass either a legacy `TextureId` or an ImGui-managed `TextureData` (preferred):
//!
//! ```no_run
//! # use dear_imgui::*;
//! # fn demo(ui: &Ui) {
//! // 1) Legacy handle
//! let tex_id = texture::TextureId::new(0x1234);
//! ui.image(tex_id, [64.0, 64.0]);
//!
//! // 2) Managed texture (created/updated/destroyed via DrawData::textures())
//! let mut tex = texture::TextureData::new();
//! tex.create(texture::TextureFormat::RGBA32, 256, 256);
//! // fill pixels / request updates ...
//! ui.image(&mut *tex, [256.0, 256.0]);
//! # }
//! ```
//!
//! Lifetime note: when using `&TextureData`, ensure it remains alive through rendering of the frame.
//!
//! ### Texture Management Guide
//!
//! - Concepts:
//!   - `TextureId`: legacy plain handle (e.g., GL texture name, Vk descriptor).
//!   - `TextureData`: managed CPU-side description with status flags and pixel buffer.
//!   - `TextureRef`: a small wrapper used by widgets/drawlist, constructed from either of the above.
//! - Basic flow:
//!   1. Create `TextureData` and call `create(format, w, h)` to allocate pixels.
//!   2. Fill/modify pixels; call `set_status(WantCreate)` for initial upload, or `WantUpdates` with
//!      `UpdateRect` for sub-updates. `TextureData::set_data()` is a convenience which copies data and
//!      marks an update.
//!   3. Use the texture in UI via `ui.image(&mut tex, size)` or drawlist APIs.
//!   4. In your renderer, during `render()`, iterate `DrawData::textures()` and honor the requests
//!      (Create/Update/Destroy), then set status back to `OK`/`Destroyed`.
//! - Alternatives: when you already have a GPU handle, pass `TextureId` directly.
//!
//! ## Renderer Integration (Modern Textures)
//!
//! When integrating a renderer backend (WGPU, OpenGL, etc.) with ImGui 1.92+:
//! - Set `BackendFlags::RENDERER_HAS_TEXTURES` on the ImGui `Io` before building the font atlas.
//! - Each frame, iterate `DrawData::textures()` and honor all requests:
//!   - `WantCreate`: create a GPU texture, upload pixels, assign a non-zero TexID back to ImGui, then set status to `OK`.
//!   - `WantUpdates`: upload pending `UpdateRect`s, then set status to `OK`.
//!   - `WantDestroy`: delete/free the GPU texture and set status to `Destroyed`.
//! - When binding textures for draw commands, do not rely only on `DrawCmdParams.texture_id`.
//!   With the modern system it may be `0`. Resolve the effective id at bind time using
//!   `ImDrawCmd_GetTexID(raw_cmd)` along with your renderer state.
//! - Optional: some backends perform a font-atlas fallback upload on initialization.
//!   This affects only the font texture for the first frame; user textures go through
//!   the modern `ImTextureData` path.
//!
//! Pseudocode outline:
//! ```ignore
//! // 1) Configure context
//! io.backend_flags |= BackendFlags::RENDERER_HAS_TEXTURES;
//!
//! // 2) Per-frame: handle texture requests
//! for tex in draw_data.textures() {
//!     match tex.status() {
//!         WantCreate => { create_gpu_tex(tex); tex.set_tex_id(id); tex.set_ok(); }
//!         WantUpdates => { upload_rects(tex); tex.set_ok(); }
//!         WantDestroy => { destroy_gpu_tex(tex); tex.set_destroyed(); }
//!         _ => {}
//!     }
//! }
//!
//! // 3) Rendering: resolve texture at bind-time
//! for cmd in draw_list.commands() {
//!     match cmd {
//!         Elements { cmd_params, raw_cmd, .. } => {
//!             let effective = unsafe { sys::ImDrawCmd_GetTexID(raw_cmd) };
//!             bind_texture(effective);
//!             draw(cmd_params);
//!         }
//!         _ => { /* ... */ }
//!     }
//! }
//! ```
//!
//! ## Colors (ImU32 ABGR)
//!
//! Dear ImGui uses a packed 32-bit color in ABGR order for low-level APIs (aka `ImU32`).
//! When you need a packed color (e.g. `TableSetBgColor`), use `colors::Color::to_imgui_u32()`:
//!
//! ```no_run
//! # use dear_imgui::*;
//! # fn demo(ui: &Ui) {
//! // Pack RGBA floats to ImGui ABGR (ImU32)
//! let abgr = Color::rgb(1.0, 0.0, 0.0).to_imgui_u32();
//! ui.table_set_bg_color_u32(TableBgTarget::CellBg, abgr, -1);
//! # }
//! ```
//!
//! For draw-list helpers you can continue to pass `[f32;4]` or use `draw::ImColor32` which
//! represents the same ABGR packed value in a convenient wrapper.
//!
//! ## Text Input (String vs ImString)
//!
//! This crate offers two ways to edit text:
//! - String-backed builders: `ui.input_text(label, &mut String)` and
//!   `ui.input_text_multiline(label, &mut String, size)`.
//!   - Internally stage a growable UTF‑8 buffer for the call and copy the
//!     edited bytes back into your `String` afterwards.
//!   - For very large fields, use `.capacity_hint(bytes)` on the builder to
//!     reduce reallocations, e.g.:
//!     ```no_run
//!     # use dear_imgui::*;
//!     # fn demo(ui: &Ui, big: &mut String) {
//!     ui.input_text("Big", big)
//!         .capacity_hint(64 * 1024)
//!         .build();
//!     # }
//!     ```
//! - ImString-backed builders: `ui.input_text_imstr(label, &mut ImString)` and
//!   `ui.input_text_multiline_imstr(label, &mut ImString, size)`.
//!   - Zero‑copy: pass your `ImString` buffer directly to ImGui.
//!   - Uses ImGui's `CallbackResize` under the hood to grow the same buffer the
//!     widget edits — no copy before/after the call.
//!
//! Choose String for convenience (especially for small/medium inputs). Prefer
//! ImString when you want to avoid copies for large or frequently edited text.

//! ## Low-level Draw APIs
//!
//! Draw list wrappers expose both high-level primitives and some low-level building blocks:
//!
//! - Concave polygons (ImGui 1.92+):
//!   - `DrawListMut::add_concave_poly_filled(&[P], color)` fills an arbitrary concave polygon.
//!   - `DrawListMut::path_fill_concave(color)` fills the current path using the concave tessellator.
//!   - Note: requires Dear ImGui 1.92 or newer in `dear-imgui-sys`.
//!
//! - Channels splitting:
//!   - `DrawListMut::channels_split(count, |channels| { ... })` splits draw into multiple channels
//!     and automatically merges on scope exit. Call `channels.set_current(i)` to select a channel.
//!
//! - Clipping helpers:
//!   - `push_clip_rect`, `push_clip_rect_full_screen`, `pop_clip_rect`, `with_clip_rect`,
//!     `clip_rect_min`, `clip_rect_max`.
//!
//! - Unsafe prim API (for custom geometry):
//!   - `prim_reserve`, `prim_unreserve`, `prim_rect`, `prim_rect_uv`, `prim_quad_uv`,
//!     `prim_write_vtx`, `prim_write_idx`, `prim_vtx`.
//!   - Safety: these mirror ImGui's low-level geometry functions. Callers must respect vertex/index
//!     counts, write exactly the reserved amounts, and ensure valid topology. Prefer high-level
//!     helpers unless you need exact control.
//!
//! - Callbacks during draw:
//!   - Safe builder: `DrawListMut::add_callback_safe(|| { ... }).build()` registers an `FnOnce()`
//!     that runs when the draw list is rendered. Resources captured by the closure are freed when
//!     the callback runs. If the draw list is never rendered, the callback will not run and its
//!     resources won't be reclaimed.
//!   - Raw: `unsafe DrawListMut::add_callback` allows passing a C callback and raw userdata; see
//!     method docs for safety requirements.

#![deny(rust_2018_idioms)]
#![cfg_attr(test, allow(clippy::float_cmp))]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]

// Re-export the sys crate for advanced users
pub extern crate dear_imgui_sys as sys;

/// Condition for setting window/widget properties
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(i32)]
pub enum Condition {
    /// Never apply the setting
    Never = -1,
    /// Set the variable always
    Always = sys::ImGuiCond_Always as i32,
    /// Set the variable once per runtime session (only the first call will succeed)
    Once = sys::ImGuiCond_Once as i32,
    /// Set the variable if the object/window has no persistently saved data (no entry in .ini file)
    FirstUseEver = sys::ImGuiCond_FirstUseEver as i32,
    /// Set the variable if the object/window is appearing after being hidden/inactive (or the first time)
    Appearing = sys::ImGuiCond_Appearing as i32,
}

// use std::cell;
// use std::os::raw::c_char;

// Core modules
pub use self::clipboard::{ClipboardBackend, DummyClipboardBackend};
pub use self::context::*;
// Note: draw types are now in render module
pub use self::fonts::*;
pub use self::input::*;
pub use self::io::*;
pub use self::platform_io::*;
pub use self::string::*;
pub use self::style::*;
pub use self::ui::*;
// Re-export utility flags/types for convenience
pub use self::utils::HoveredFlags;

// Utility modules
pub use self::list_clipper::*;
// pub use self::math::*;

// Widget modules
pub use self::widget::*;
pub use self::window::*;

// Stack management
pub use self::stacks::*;

// Layout and cursor control
pub use self::layout::*;

// Drag and drop system
pub use self::drag_drop::*;

// Text filtering system
pub use self::text_filter::*;

// Column layout system (included in layout module)
pub use self::columns::*;

// Internal modules
mod clipboard;
mod colors;
mod context;
mod dock_builder;
mod dock_space;
mod draw;
mod error;
mod fonts;
pub mod input;
pub mod internal;
mod io;
mod list_clipper;
pub mod platform_io;
pub mod render;
mod string;
mod style;
pub mod texture;
mod ui;
mod utils;
#[cfg(feature = "multi-viewport")]
pub mod viewport_backend;
// mod math;
mod widget;
mod window;

// Token system for resource management
#[macro_use]
mod tokens;

// Stack management
mod stacks;

// Layout and cursor control
mod layout;

// Drag and drop system
mod drag_drop;

// Text filtering system
mod text_filter;

// Column layout system
mod columns;

// Logging utilities
pub mod logging;

// Re-export public API
pub use colors::*;
pub use dock_builder::*;
pub use dock_space::*;
// Export DrawListMut for extensions
pub use draw::DrawListMut;
pub use error::*;
// Note: draw types are now in render module, no need to export draw::*
pub use render::*;
pub use texture::*;

// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Check if docking features are available
pub const HAS_DOCKING: bool = sys::HAS_DOCKING;

/// Check if FreeType font rasterizer support is compiled in
pub const HAS_FREETYPE: bool = sys::HAS_FREETYPE;

/// Check if WASM support is compiled in (sys layer)
pub const HAS_WASM: bool = sys::HAS_WASM;

/// Returns the underlying Dear ImGui library version
#[doc(alias = "GetVersion")]
pub fn dear_imgui_version() -> &'static str {
    unsafe {
        let version_ptr = sys::igGetVersion();

        let bytes = std::ffi::CStr::from_ptr(version_ptr).to_bytes();
        std::str::from_utf8_unchecked(bytes)
    }
}
