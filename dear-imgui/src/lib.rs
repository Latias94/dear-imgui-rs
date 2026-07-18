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
//! use dear_imgui_rs::*;
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
//! - With the optional `glam` feature, `glam::Vec2` directly (via `impl From<glam::Vec2> for ImVec2` in `dear-imgui-sys`)
//!
//! Example:
//! ```no_run
//! # use dear_imgui_rs::*;
//! # fn demo(ui: &Ui) {
//! let dl = ui.get_window_draw_list();
//! dl.add_line([0.0, 0.0], [100.0, 100.0], [1.0, 1.0, 1.0, 1.0]).build();
//! // Also works with mint::Vector2<f32>
//! let a = mint::Vector2 { x: 10.0, y: 20.0 };
//! let b = mint::Vector2 { x: 30.0, y: 40.0 };
//! dl.add_rect(a, b, [1.0, 0.0, 0.0, 1.0]).build();
//! // And with glam::Vec2 when the `glam` feature is enabled
//! #[cfg(feature = "glam")]
//! {
//!     let a = glam::Vec2::new(10.0, 20.0);
//!     let b = glam::Vec2::new(30.0, 40.0);
//!     dl.add_rect(a, b, [0.0, 1.0, 0.0, 1.0]).build();
//! }
//! # }
//! ```
//!
//! ## Textures (ImGui 1.92+)
//!
//! You can pass either a legacy `TextureId` or a Context-owned managed texture handle:
//!
//! ```no_run
//! # use dear_imgui_rs::*;
//! # fn demo(context: &mut Context) {
//! // 1) Legacy handle
//! let tex_id = texture::TextureId::new(0x1234);
//! // 2) Transfer an owned texture into this Context.
//! let mut tex = texture::OwnedTextureData::new();
//! tex.create(texture::TextureFormat::RGBA32, 256, 256);
//! tex.set_data(&vec![255; 256 * 256 * 4]);
//! let managed = context.register_texture(tex);
//! let ui = context.frame();
//! ui.image(tex_id, [64.0, 64.0]);
//! ui.image(managed, [256.0, 256.0]);
//! # }
//! ```
//!
//! `TextureRef<'tex>` is pointer-free for user textures. It stores either a legacy value handle, a
//! Context/slot/generation managed identity, or an internal font-atlas reference backed by an owner
//! lease. The owning `Ui` resolves managed handles immediately before FFI and rejects foreign,
//! stale, or retiring handles first.
//!
//! ### Texture Management Guide
//!
//! - Concepts:
//!   - `TextureId`: legacy plain handle (e.g., GL texture name, Vk descriptor).
//!   - `OwnedTextureData`: transferable CPU-side texture allocation prepared before registration.
//!   - `ManagedTextureId`: opaque Context/slot/generation identity used by widgets and draw lists.
//!   - `ManagedTextureRef` / `ManagedTextureMut`: non-escaping Context-scoped inspection and pixel
//!     updates which do not expose native pointers or renderer-owned fields.
//!   - `TextureRef<'tex>`: logical image source constructed from `TextureId`, `ManagedTextureId`,
//!     or an owner-backed font-atlas texture lease.
//! - Basic flow:
//!   1. Create `OwnedTextureData` and call `create(format, w, h)` to allocate pixels.
//!   2. Fill pixels with `set_data()`; registration preserves the initial create request.
//!   3. Transfer ownership with `Context::register_texture(tex)` and retain its handle.
//!   4. Mutate before a frame with `Context::with_texture_mut(handle, |tex| ...)`.
//!   5. Use the handle in UI via `ui.image(handle, size)` or draw-list APIs.
//!   6. Call `Context::remove_texture(handle)` to begin generation-safe retirement.
//!   7. A renderer processes request-owned bytes from `RenderedFrame::texture_requests()` or
//!      `FrameSnapshot::texture_requests()` and returns feedback created by each request.
//! - Alternatives: when you already have a GPU handle, pass `TextureId` directly.
//!
//! ## Renderer Integration (Modern Textures)
//!
//! When integrating a renderer backend (WGPU, OpenGL, etc.) with ImGui 1.92+:
//! - Set `BackendFlags::RENDERER_HAS_TEXTURES` on the ImGui `Io` before building the font atlas.
//! - Create one `RendererConsumer` from the Context and keep it alive with the renderer.
//! - Synchronous renderer APIs consume a Context-borrowed `RenderedFrame`; detached renderers
//!   consume a move-only `FrameSnapshot`.
//! - Each frame, handle every `TextureOp::Create`, `Update`, and `Destroy`, then create feedback
//!   through `TextureRequest::uploaded` or `TextureRequest::destroyed`.
//! - Reconcile synchronous feedback before rendering draw commands that depend on new IDs;
//!   detached snapshots commit feedback when their GPU work is complete.
//! - Bind [`DrawCmdParams::texture_id`](render::DrawCmdParams::texture_id). Command iteration
//!   resolves the effective ID for both legacy and managed texture references.
//! - After destroying every renderer-owned GPU texture, call
//!   `Context::reset_renderer_texture_bindings` before dropping the consumer.
//!
//! Pseudocode outline:
//! ```ignore
//! // 1) Configure context
//! io.backend_flags |= BackendFlags::RENDERER_HAS_TEXTURES;
//!
//! let consumer = context.create_renderer_consumer()?;
//! let mut frame = context.render();
//! let mut feedback = Vec::new();
//! for request in frame.texture_requests() {
//!     feedback.push(match request.operation() {
//!         TextureOp::Create { .. } | TextureOp::Update { .. } =>
//!             request.uploaded(upload_to_gpu(request))?,
//!         TextureOp::Destroy => {
//!             destroy_gpu_texture(request.texture());
//!             request.destroyed()?
//!         }
//!     });
//! }
//! frame.reconcile_texture_feedback(feedback)?;
//!
//! // Rendering uses IDs resolved by the owning Context.
//! for draw_list in frame.draw_data().draw_lists() {
//!   for cmd in draw_list.commands() {
//!     match cmd {
//!         Elements { cmd_params, .. } => {
//!             bind_texture(cmd_params.texture_id);
//!             draw(cmd_params);
//!         }
//!         _ => { /* ... */ }
//!     }
//!   }
//! }
//! ```
//!
//! For thread-safe render work, register one renderer consumer and capture a Context-created,
//! move-only `render::FrameSnapshot`.
//!
//! ## Safe API Migration Notes
//!
//! The safe layer intentionally rejects old patterns that depended on hidden C current-context or
//! aliasing state:
//!
//! - Use `TextureId` for legacy handles and `ManagedTextureId` for Context-owned textures.
//! - Borrowed `&mut TextureData` is intentionally not an image source; transfer ownership with
//!   `Context::register_texture` and mutate it through a Context-scoped closure.
//! - Synchronous renderer backends consume a Context-borrowed `RenderedFrame`; detached renderers
//!   consume a move-only `FrameSnapshot` and commit request-bound feedback.
//! - `FontId` is a persistent, atlas-validated handle. It may be stored in style state, but
//!   `Ui::push_font`, `DrawListMut::add_text_with_font`, and `Ui::push_font_with_size` validate the
//!   active atlas before entering FFI. `FontAtlas::clear`,
//!   `clear_fonts`, and `remove_font` invalidate existing `FontId` values from that atlas.
//! - `Context::font_atlas()` returns `&FontAtlas`; use `font_atlas()` for startup-time font
//!   loading and atlas mutation.
//! - RAII tokens for windows, stacks, popups, tables, draw-list texture stacks, and extension scopes
//!   are UI/current-context scoped and `!Send + !Sync`. Drop them on the creating UI thread.
//! - `Ui::push_state_storage` returns `StateStorageToken<'ui, 'storage>`, so the storage must outlive
//!   the token that restores the previous storage.
//!
//! ## Colors (ImU32 ABGR)
//!
//! Dear ImGui uses a packed 32-bit color in ABGR order for low-level APIs (aka `ImU32`).
//! When you need a packed color (e.g. `TableSetBgColor`), use `colors::Color::to_imgui_u32()`:
//!
//! ```no_run
//! # use dear_imgui_rs::*;
//! # fn demo(ui: &Ui) {
//! // Pack RGBA floats to ImGui ABGR (ImU32)
//! let abgr = Color::rgb(1.0, 0.0, 0.0).to_imgui_u32();
//! ui.table_set_cell_bg_color_u32(abgr, TableColumnRef::Current);
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
//!   - Internally stage a growable UTF�? buffer for the call and copy the
//!     edited bytes back into your `String` afterwards.
//!   - For very large fields, use `.capacity_hint(bytes)` on the builder to
//!     reduce reallocations, e.g.:
//!     ```no_run
//!     # use dear_imgui_rs::*;
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
//!     widget edits �?no copy before/after the call.
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
//!   - `push_clip_rect`, `push_clip_rect_full_screen`, `with_clip_rect`, `clip_rect_min`,
//!     `clip_rect_max`.
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
#![cfg_attr(test, allow(clippy::float_cmp, deprecated))]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]

// Re-export the sys crate for advanced users
pub extern crate dear_imgui_sys as sys;

/// Strongly-typed wrapper around ImGuiID.
///
/// This avoids leaking the `sys` type in safe APIs and improves clarity
/// when passing/returning identifiers (e.g., dock ids, viewport ids).
///
/// Construct an explicit user-defined ID with [`Id::from`] and a `u32`. When
/// the ID should follow Dear ImGui's current ID stack, prefer [`Ui::get_id`].
///
/// ```
/// # use dear_imgui_rs::Id;
/// let id = Id::from(42_u32);
/// assert_eq!(id.raw(), 42);
/// ```
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
pub struct Id(pub(crate) sys::ImGuiID);

impl Id {
    /// Returns the raw ImGuiID value.
    pub fn raw(self) -> sys::ImGuiID {
        self.0
    }
}

impl From<sys::ImGuiID> for Id {
    fn from(v: sys::ImGuiID) -> Self {
        Id(v)
    }
}

impl From<Id> for sys::ImGuiID {
    fn from(v: Id) -> Self {
        v.0
    }
}

// Note: do not add From<u32> or From<Id> for u32 here to avoid
// overlapping/conflicting impls on platforms where ImGuiID == u32.

/// Condition for setting window/widget properties
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(i32)]
#[allow(clippy::unnecessary_cast)]
pub enum Condition {
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
pub use self::state_storage::*;
pub use self::string::*;
pub use self::style::*;
pub use self::ui::*;
// Re-export utility flags/types for convenience
pub use self::utils::{
    FocusedFlags, ItemHoveredFlags, LogAutoOpenDepth, TooltipHoveredFlags, WindowHoveredFlags,
};

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

// Internal modules
mod clipboard;
mod colors;
mod context;
mod dock_layout;
mod dock_space;
mod draw;
mod error;
pub mod fonts;
pub mod input;
pub mod internal;
mod io;
mod list_clipper;
pub mod platform_io;
pub mod render;
mod state_storage;
mod string;
mod style;
pub mod texture;
// Token system for resource management
#[macro_use]
mod tokens;
mod ui;
mod utils;
// mod math;
mod widget;
mod window;

// Stack management
mod stacks;

// Layout and cursor control
mod layout;

// Drag and drop system
mod drag_drop;

// Text filtering system
mod text_filter;

// Logging utilities

#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::{Mutex, MutexGuard, OnceLock};

    pub(crate) fn imgui_context_guard() -> MutexGuard<'static, ()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
    }
}

// Re-export public API
pub use colors::*;
pub use dock_layout::*;
pub use dock_space::*;
// Export draw-list helpers for extensions and downstream custom drawing.
pub use draw::{
    DrawCornerFlags, DrawListFlags, DrawListMut, DrawListTextureToken, DrawNgonSegmentCount,
    DrawSegmentCount, PolylineFlags,
};
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
        if version_ptr.is_null() {
            return "Unknown";
        }
        std::ffi::CStr::from_ptr(version_ptr)
            .to_str()
            .unwrap_or("Unknown")
    }
}
