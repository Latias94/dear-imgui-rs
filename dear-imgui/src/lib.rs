//! # Dear ImGui - Rust Bindings with Docking Support
//!
//! High-level Rust bindings for Dear ImGui, the immediate mode GUI library.
//! This crate provides safe, idiomatic Rust bindings with docking support and
//! optional native multi-viewport primitives. Multi-viewport also requires a
//! supported platform and renderer backend; browser applications use one canvas.
//!
//! ## Features
//!
//! - Safe, idiomatic Rust API
//! - Docking and optional backend-specific native multi-viewport support
//! - Builder pattern for widgets
//! - Memory-safe string handling
//! - Integration with modern Rust graphics ecosystems
//!
//! ## Quick Start
//!
//! ```no_run
//! use dear_imgui_rs::prelude::*;
//!
//! let mut ctx = Context::create();
//! ctx.io_mut().set_display_size([640.0, 480.0]);
//! ctx.io_mut().set_delta_time(1.0 / 60.0);
//! ctx.font_atlas().try_claim_legacy_renderer()?.build();
//! let ui = ctx.frame();
//!
//! ui.window("Hello World")
//!     .size([300.0, 100.0], Condition::FirstUseEver)
//!     .build(|| {
//!         ui.text("Hello, world!");
//!         ui.text("This is Dear ImGui with docking support!");
//!     });
//! let frame = ctx.render_legacy();
//! let _draw_data = frame.draw_data();
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! This backend-free example only builds draw commands. A windowed application
//! also needs platform input and a renderer. Managed renderers own a consumer
//! and use the texture protocol below instead of building a legacy atlas.
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
//! ui.with_window_draw_list(|dl| {
//!     dl.add_line([0.0, 0.0], [100.0, 100.0], [1.0, 1.0, 1.0, 1.0]).build();
//!     // Also works with mint::Vector2<f32>
//!     let a = mint::Vector2 { x: 10.0, y: 20.0 };
//!     let b = mint::Vector2 { x: 30.0, y: 40.0 };
//!     dl.add_rect(a, b, [1.0, 0.0, 0.0, 1.0]).build();
//!     // And with glam::Vec2 when the `glam` feature is enabled
//!     #[cfg(feature = "glam")]
//!     {
//!         let a = glam::Vec2::new(10.0, 20.0);
//!         let b = glam::Vec2::new(30.0, 40.0);
//!         dl.add_rect(a, b, [0.0, 1.0, 0.0, 1.0]).build();
//!     }
//! });
//! # }
//! ```
//!
//! ## Textures (ImGui 1.92+)
//!
//! You can pass either a legacy `TextureId` or a Context-owned managed texture handle:
//!
//! ```no_run
//! # use dear_imgui_rs::*;
//! # fn demo(context: &mut Context) -> Result<(), texture::TextureDataError> {
//! // 1) Legacy handle
//! let tex_id = texture::TextureId::new(0x1234);
//! // 2) Transfer an owned texture into this Context.
//! let tex = texture::OwnedTextureData::from_pixels(
//!     texture::TextureFormat::RGBA32,
//!     256,
//!     256,
//!     &vec![255; 256 * 256 * 4],
//! )?;
//! let managed = context.register_texture(tex);
//! let ui = context.frame();
//! ui.image(tex_id, [64.0, 64.0]);
//! ui.image(managed, [256.0, 256.0]);
//! # Ok(())
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
//!   1. Create `OwnedTextureData` with `from_pixels(format, w, h, pixels)`; the payload length is
//!      validated exactly before native allocation.
//!   2. Transfer ownership with `Context::register_texture(tex)` and retain its handle.
//!   3. Mutate before a frame with `Context::try_with_texture_mut(handle, |tex| ...)`, using
//!      `replace_pixels()` for a full replacement or `update_subresource()` for a strided region.
//!   4. Use the handle in UI via `ui.image(handle, size)` or draw-list APIs.
//!   5. Call `Context::remove_texture(handle)` to begin generation-safe retirement.
//!   6. A renderer processes request-owned bytes from `PendingFrame::texture_requests()` or
//!      `FrameSnapshot::texture_requests()` and returns feedback created by each request.
//! - Alternatives: when you already have a GPU handle, pass `TextureId` directly.
//!
//! ## Renderer Integration (Modern Textures)
//!
//! When integrating a renderer backend (WGPU, OpenGL, etc.) with ImGui 1.92+:
//! - Create one `SynchronousRendererConsumer` or `DetachedRendererConsumer` from the Context and
//!   keep it alive with that renderer path. This explicitly claims managed font-atlas ownership.
//! - Set `BackendFlags::RENDERER_HAS_TEXTURES` before the first frame. Do not call
//!   `LegacyFontAtlas::build`; managed renderers build and upload the atlas from texture requests.
//! - Synchronous renderer APIs consume a Context-borrowed `PendingFrame`, reconcile it, then draw
//!   the resulting `ReconciledFrame`; detached renderers consume a move-only `FrameSnapshot`.
//! - Each frame, give every texture request one explicit `uploaded`, `destroyed`, `superseded`, or
//!   `retry` outcome.
//! - Reconcile synchronous feedback before rendering draw commands that depend on new IDs;
//!   `cargo run -j 1 -p dear-imgui-rs --example custom_renderer_headless` is the executable
//!   reference for the complete synchronous request, draw, and reset sequence.
//!   detached snapshots commit feedback when their GPU work is complete.
//! - Bind [`DrawCmdParams::texture_id`](render::DrawCmdParams::texture_id). Command iteration
//!   resolves the effective ID for both legacy and managed texture references.
//! - Before destroying the renderer's complete GPU texture map, call
//!   `Context::prepare_renderer_texture_reset`. Destroy the map only after preparation succeeds,
//!   then commit the permit before dropping the consumer.
//!
//! Pseudocode outline:
//! ```ignore
//! // 1) Configure context
//! let consumer = context.create_synchronous_renderer_consumer()?;
//! context.io_mut().set_backend_flags(
//!     context.io().backend_flags() | BackendFlags::RENDERER_HAS_TEXTURES,
//! );
//! let pending = context.render(&consumer);
//! let mut feedback = Vec::new();
//! for request in pending.texture_requests() {
//!     feedback.push(match request.operation() {
//!         TextureOp::Create { .. } | TextureOp::Update { .. } =>
//!             request.uploaded(upload_to_gpu(request))?,
//!         TextureOp::Destroy => {
//!             destroy_gpu_texture(request.texture());
//!             request.destroyed()?
//!         }
//!     });
//! }
//! let frame = pending.reconcile_texture_feedback(feedback)?;
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
//! drop(frame);
//!
//! // Shutdown only after every frame and its GPU work has completed.
//! let reset = context.prepare_renderer_texture_reset(&consumer)?;
//! destroy_all_gpu_textures();
//! reset.commit();
//! drop(consumer);
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
//! - Synchronous renderer backends turn a Context-borrowed `PendingFrame` into a drawable
//!   `ReconciledFrame`; detached renderers consume a move-only `FrameSnapshot` and commit
//!   request-bound feedback.
//! - `FontId` is a persistent, atlas-validated handle. It may be stored in style state, but
//!   `Ui::push_font`, `DrawListMut::add_text_with_font`, and `Ui::push_font_with_size` validate the
//!   active atlas before entering FFI. `FontAtlas::clear`,
//!   `clear_fonts`, and `remove_font` invalidate existing `FontId` values from that atlas.
//! - `Context::font_atlas()` returns `&FontAtlas`; use `font_atlas()` for startup-time font
//!   loading and atlas mutation.
//! - RAII tokens for windows, stacks, popups, tables, draw-list texture stacks, and extension scopes
//!   are UI/current-context scoped and `!Send + !Sync`. Drop them on the creating UI thread.
//! - Contexts and independently owned ImGui allocations share one OS thread. An extension that
//!   uses the ImGui allocator can hold a [`ContextThreadLease`] from allocation through destruction,
//!   including when its native objects outlive their originating Context.
//! - Use `Ui::with_state_storage` for a panic-safe, nested state-storage override. Both the
//!   replacement and its `StateStorage` view are confined to the closure.
//! - Use `Ui::with_multi_select` for advanced multi-select flows. It returns an owned
//!   `MultiSelectResult`; no view into Dear ImGui's temporary begin/end IO escapes the closure.
//!
//! ## Colors (ImU32 ABGR)
//!
//! Dear ImGui uses a packed 32-bit color in ABGR order for low-level APIs (aka `ImU32`).
//! When you need a packed color (e.g. `TableSetBgColor`), use [`Color::to_imgui_u32`]:
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
//! ## Text input
//!
//! Use `ui.input_text(label, &mut String)` or `ui.input_text_multiline(label, &mut String, size)`.
//! The widget stages a growable UTF-8 buffer and writes the edited text back to the `String`.
//! Use `.capacity_hint(bytes)` to reduce reallocations when editing large fields.

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
//!   - Raw: `unsafe DrawListMut::add_callback` allows passing a C callback and raw userdata; see
//!     method docs for safety requirements.

#![deny(rust_2018_idioms)]
#![warn(unreachable_pub)]
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

pub use self::clipboard::{ClipboardBackend, DummyClipboardBackend};
#[cfg(feature = "system-clipboard")]
pub use self::clipboard::{SystemClipboard, SystemClipboardError};
pub use self::colors::Color;
pub use self::context::{
    Context, ContextAliveToken, ContextAttachment, ContextAttachmentDetachError,
    ContextAttachmentError, ContextAttachmentHandle, ContextAttachmentLease,
    ContextAttachmentPhase, ContextAttachmentRole, ContextAttachmentTeardownError, ContextBinding,
    ContextBindingError, ContextDestroyed, ContextId, ContextLifecycle, ContextOperationError,
    ContextPlatformAttachmentRelease, ContextPlatformAttachmentReleaseError,
    ContextPlatformWindowTeardown, ContextPlatformWindowTeardownError, ContextScopeError,
    ContextTeardown, ContextThreadLease, FrameLifecycleStamp, FrameLifecycleState,
    FramePrepareOptions, FrameResult, FrameToken, ImePreedit, ImePreeditError,
    RendererTextureReset,
};
pub use self::dock_layout::{DockLayout, DockLayoutApply, DockSplit, DockspaceError};
pub use self::dock_space::{
    DockNodeFlags, DockspaceBuilder, WindowClass, WindowClassDockNodeFlags, WindowClassError,
    WindowClassParentViewport,
};
pub use self::drag_drop::{
    DragDropPayload, DragDropPayloadCond, DragDropPayloadEmpty, DragDropPayloadPod, DragDropSource,
    DragDropSourceFlags, DragDropSourceTooltip, DragDropTarget, DragDropTargetFlags,
    PayloadIsWrongType,
};
pub use self::draw::{
    DrawFlags, DrawListMut, DrawListTextureToken, DrawNgonSegmentCount, DrawSegmentCount,
    ImColor32, RawDrawCallback,
};
pub use self::error::{ImGuiError, ImGuiResult, SafeStringConversion};
pub use self::fonts::{
    BakedFont, CustomRectData, CustomRectId, CustomRectSnapshot, FontAtlas, FontAtlasFlags,
    FontAtlasLoaderError, FontAtlasModeError, FontAtlasTexture, FontConfig, FontId, FontLoadError,
    FontLoader, FontLoaderFlags, FontSource, Glyph, LegacyFontAtlas, SharedFontAtlas,
    StbTrueTypeFontData, StbTrueTypeFontError, StbTrueTypeFontLoadError,
};
pub use self::ini_settings::{
    IniSessionDate, IniSessionDateError, IniSettingsRetention, IniSettingsRetentionError,
};
pub use self::input::{
    InputScalarFlags, InputTextFlags, InputTextMultilineFlags, ItemKeyOwnerFlags, Key, KeyChord,
    KeyMods, MouseButton, MouseCursor, MouseSource, NextItemShortcutFlags, NextItemShortcutOptions,
    ShortcutFlags, ShortcutGlobalRouteFlags, ShortcutOptions, ShortcutRoute,
};
pub use self::io::{BackendFlags, ConfigFlags, Io, ViewportFlags, WindowClassViewportFlags};
pub use self::layout::{ClipRectToken, GroupToken, IndentToken};
#[cfg(feature = "stack-layout")]
pub use self::layout::{
    HorizontalStackLayoutToken, StackLayoutId, StackLayoutSuspensionToken, VerticalStackLayoutToken,
};
pub use self::list_clipper::{
    ListClipper, ListClipperIterator, ListClipperToken, UnknownCountListClipper,
    UnknownCountListClipperToken,
};
pub use self::numeric_format::{NumericFormat, NumericFormatError};
pub use self::platform_io::{
    NativeWindowIdentityExhausted, NativeWindowInstanceId, NativeWindowTarget, PlatformIo, Viewport,
};
pub use self::render::{
    DetachedRendererConsumer, DrawCmd, DrawCmdIterator, DrawCmdParams, DrawCmdSnapshot, DrawData,
    DrawDataSnapshot, DrawIdx, DrawList, DrawListIterator, DrawListSnapshot, DrawRequirements,
    DrawVert, FrameSnapshot, PendingFrame, RawCallbackCommand, ReconciledFrame,
    RendererConsumerCapability, RendererConsumerError, RendererRenderStateGuard,
    RendererRenderStateGuardError, SnapshotCommitError, SnapshotCompletionProgress, SnapshotEpoch,
    SnapshotError, SnapshotTextureId, SynchronousRendererConsumer, TextureBinding, TextureFeedback,
    TextureFeedbackError, TextureOp, TextureRequest, TextureRequestKind, TextureUploadIdentity,
    TextureUploadRect, ViewportDrawDataSnapshot,
};
pub use self::stacks::{
    ColorStackToken, FocusScopeToken, FontStackToken, IdStackToken, ItemFlagStackToken, ItemFlags,
    ItemStateFlags, ItemWidthStackToken, StyleStackToken, TextWrapPosStackToken,
};
pub use self::state_storage::{OwnedStateStorage, StateStorage};
pub use self::string::{
    UiBuffer, with_scratch_txt, with_scratch_txt_slice, with_scratch_txt_slice_with_opt,
    with_scratch_txt_three, with_scratch_txt_two,
};
pub use self::style::{
    ColorOverride, Direction, Style, StyleColor, StyleTweaks, StyleVar, StyleVarVec2, TableTheme,
    Theme, ThemePreset, TreeLineMode, WindowTheme,
};
pub use self::text_filter::TextFilter;
pub use self::texture::{
    ManagedTextureError, ManagedTextureId, ManagedTextureMut, ManagedTextureMutationError,
    ManagedTextureRef, OwnedTextureData, RawTextureId, TextureData, TextureDataError,
    TextureFormat, TextureId, TextureRect, TextureRef, TextureRegion, TextureStatus,
    TextureSubresource, get_format_bytes_per_pixel, get_format_name, get_status_name,
};
pub use self::ui::{NativeWindowScope, Ui};
pub use self::utils::{
    FocusedFlags, ItemHoveredFlags, LogAutoOpenDepth, TooltipHoveredFlags, WindowHoveredFlags,
};
pub use self::widget::{
    AngleSlider, BasicSelection, BasicSelectionIter, Button, ButtonFlags, ButtonRepeatToken,
    ColorButton, ColorButtonFlags, ColorButtonOptions, ColorDataType, ColorDisplayMode, ColorEdit3,
    ColorEdit4, ColorEditFlags, ColorEditOptions, ColorInputMode, ColorPicker3, ColorPicker4,
    ColorPickerDisplayFlags, ColorPickerFlags, ColorPickerMode, ColorPickerOptions, ColumnBuilder,
    ComboBox, ComboBoxFlags, ComboBoxHeight, ComboBoxOptions, ComboBoxPreviewMode, ComboBoxToken,
    DisabledToken, Drag, DragFlags, DragRange, HistoryDirection, Image, ImageButton, InputDouble,
    InputFloat, InputFloat2, InputFloat3, InputFloat4, InputInt, InputInt2, InputInt3, InputInt4,
    InputScalar, InputScalarN, InputText, InputTextCallback, InputTextCallbackHandler,
    InputTextMultiline, InputTextMultilineWithCb, InvisibleButtonMouseButtons,
    InvisibleButtonOptions, KeySetSelection, ListBox, MainMenuBarToken, MenuBarToken, MenuToken,
    ModalPopup, ModalPopupToken, MultiSelectBoxSelect, MultiSelectClickPolicy, MultiSelectFlags,
    MultiSelectIndexStorage, MultiSelectOptions, MultiSelectRangeDirection, MultiSelectRequest,
    MultiSelectResult, MultiSelectScope, MultiSelectScopeKind, MultiSelectUserData,
    PassthroughCallback, PlotHistogram, PlotLines, PlotValueOffset, PopupContextFlags,
    PopupContextMouseButton, PopupContextOptions, PopupOpenFlags, PopupQueryFlags, PopupToken,
    ProgressBar, Selectable, SelectableFlags, Slider, SliderFlags, SortDirection, TabBar,
    TabBarFittingPolicy, TabBarFlags, TabBarOptions, TabBarToken, TabItem, TabItemFlags,
    TabItemOptions, TabItemPlacement, TabItemToken, TableBgTarget, TableBuilder, TableColumnFlags,
    TableColumnIndent, TableColumnIndex, TableColumnRef, TableColumnSetup, TableColumnSortSpec,
    TableColumnStateFlags, TableColumnUserData, TableColumnWidth, TableContextMenuTarget,
    TableFlags, TableHeaderData, TableHoveredColumn, TableHoveredRow, TableOptions, TableRowFlags,
    TableRowIndex, TableSizingPolicy, TableSortSpecs, TableToken, TextCallbackData, TooltipToken,
    TreeNode, TreeNodeFlags, TreeNodeId, TreeNodeToken, VerticalSlider,
};
pub use self::window::{
    ChildFlags, ChildWindow, Window, WindowFlags, WindowKey, WindowKeyError, WindowLabel,
};

/// The everyday types used to create a Context and submit widgets.
pub mod prelude {
    pub use crate::{Color, Condition, Context, Id, Ui, WindowFlags};
}

// Internal modules
mod clipboard;
mod colors;
mod context;
mod dock_layout;
mod dock_space;
pub mod draw;
mod error;
pub mod ffi;
pub mod fonts;
mod ini_settings;
pub mod input;
pub mod internal;
mod io;
mod list_clipper;
pub mod math;
mod numeric_format;
pub mod platform_io;
pub mod render;
mod scope;
mod state_storage;
mod string;
mod style;
pub mod texture;
// Token system for resource management
#[macro_use]
mod tokens;
mod drag_drop;
mod layout;
pub mod stacks;
mod text_filter;
mod ui;
mod utils;
mod validation;
pub mod widget;
mod window;

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

#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::{Mutex, MutexGuard, OnceLock};

    pub(crate) fn imgui_context_guard() -> MutexGuard<'static, ()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
    }
}
