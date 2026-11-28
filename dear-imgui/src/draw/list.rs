use std::cell::RefCell;

use crate::scope::{NativeScopePop, NativeScopeToken};
use crate::sys;

use super::{
    CORNER_FLAGS, DrawFlags, ImColor32, assert_draw_flags, assert_non_negative_f32, count_to_i32,
    finite_vec2, finite_vec4,
};

mod geometry;
mod image;
#[cfg(test)]
mod tests;

pub use image::DrawListTextureToken;

thread_local! {
    static BORROWED_DRAW_LISTS: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
}

/// Object implementing the custom draw API.
///
/// Called from [`crate::Ui::with_window_draw_list`], [`crate::Ui::get_background_draw_list`] or [`crate::Ui::get_foreground_draw_list`].
/// Only one mutable wrapper can exist for the same raw draw list on the same thread at a time.
/// Reuse the existing wrapper, or drop it before requesting the same draw list again. The program
/// will panic when attempting to wrap the same draw list twice.
pub struct DrawListMut<'ui> {
    pub(super) draw_list: *mut sys::ImDrawList,
    pub(super) ui: Option<&'ui crate::Ui>,
    pub(super) provenance: DrawListProvenance,
}

#[derive(Clone, Copy)]
pub(super) enum DrawListProvenance {
    Frame,
    Window(crate::scope::WindowScope),
}

impl Drop for DrawListMut<'_> {
    fn drop(&mut self) {
        let ptr = self.draw_list as usize;
        BORROWED_DRAW_LISTS.with(|borrowed| {
            let mut borrowed = borrowed.borrow_mut();
            if let Some(index) = borrowed.iter().position(|&value| value == ptr) {
                borrowed.swap_remove(index);
            }
        });
    }
}

impl<'ui> DrawListMut<'ui> {
    fn borrow_draw_list(draw_list: *mut sys::ImDrawList) {
        assert!(
            !draw_list.is_null(),
            "DrawListMut::borrow_draw_list() received a null draw list"
        );
        let ptr = draw_list as usize;
        BORROWED_DRAW_LISTS.with(|borrowed| {
            let mut borrowed = borrowed.borrow_mut();
            if borrowed.contains(&ptr) {
                panic!(
                    "A DrawListMut is already in use for this draw list; reuse the existing wrapper or drop it before acquiring another"
                );
            }
            borrowed.push(ptr);
        });
    }

    fn from_raw(
        ui: &'ui crate::Ui,
        draw_list: *mut sys::ImDrawList,
        provenance: DrawListProvenance,
    ) -> Self {
        Self::borrow_draw_list(draw_list);
        Self {
            draw_list,
            ui: Some(ui),
            provenance,
        }
    }

    /// Wrap a raw ImDrawList pointer for the current Dear ImGui frame.
    ///
    /// # Safety
    ///
    /// `draw_list` must be a valid mutable draw-list pointer owned by the active
    /// Dear ImGui frame and remain valid for `'ui`. The caller must also ensure
    /// the pointer is not independently mutated while the returned wrapper is
    /// alive. Raw wrappers are treated as frame-scoped; the caller must enforce any narrower
    /// window or viewport provenance required by the native draw list.
    pub unsafe fn from_raw_mut(ui: &'ui crate::Ui, draw_list: *mut sys::ImDrawList) -> Self {
        Self::from_raw(ui, draw_list, DrawListProvenance::Frame)
    }

    pub(crate) fn window(ui: &'ui crate::Ui) -> Self {
        ui.run_with_bound_context(|| unsafe {
            let scope = ui
                .current_native_scope()
                .window()
                .expect("Ui::with_window_draw_list() requires a current window");
            Self::from_raw(
                ui,
                sys::igGetWindowDrawList(),
                DrawListProvenance::Window(scope),
            )
        })
    }

    pub(crate) fn background(ui: &'ui crate::Ui) -> Self {
        ui.run_with_bound_context(|| {
            let viewport = unsafe { sys::igGetMainViewport() };
            Self::from_raw(
                ui,
                unsafe { sys::igGetBackgroundDrawList(viewport) },
                DrawListProvenance::Frame,
            )
        })
    }

    pub(crate) fn foreground(ui: &'ui crate::Ui) -> Self {
        ui.run_with_bound_context(|| {
            let viewport = unsafe { sys::igGetMainViewport() };
            Self::from_raw(
                ui,
                unsafe { sys::igGetForegroundDrawList_ViewportPtr(viewport) },
                DrawListProvenance::Frame,
            )
        })
    }

    pub(super) fn ui(&self) -> &crate::Ui {
        self.ui
            .expect("this draw-list operation requires a DrawListMut borrowed from Ui")
    }

    fn assert_bound_scope(&self, operation: &'static str) {
        if let DrawListProvenance::Window(scope) = self.provenance {
            assert!(
                self.ui().current_native_scope().window() == Some(scope),
                "{operation} requires the window Begin scope that created this DrawListMut"
            );
        }
    }

    pub(super) fn with_raw_bound<R>(
        &self,
        operation: &'static str,
        f: impl FnOnce(*mut sys::ImDrawList) -> R,
    ) -> R {
        #[cfg(test)]
        if self.ui.is_none() {
            return f(self.draw_list);
        }
        self.ui().run_with_bound_context(|| {
            self.assert_bound_scope(operation);
            f(self.draw_list)
        })
    }

    pub(super) fn is_window_scoped(&self) -> bool {
        matches!(self.provenance, DrawListProvenance::Window(_))
    }
}

/// Tracks a clip rectangle pushed onto a draw-list clip stack.
///
/// The clip rectangle is popped when the token is dropped or when [`Self::pop`]
/// is called. Tokens for the same draw list must finish in reverse creation order. A token from a
/// window draw list must also finish in the exact window `Begin` scope that created the draw-list
/// view. Prefer [`DrawListMut::with_clip_rect`] for ordinary scoped use.
#[must_use]
pub struct DrawListClipRectToken<'draw_list> {
    scope: crate::scope::NativeScopeToken<'draw_list>,
}

impl<'draw_list> DrawListClipRectToken<'draw_list> {
    fn new(
        ui: &'draw_list crate::Ui,
        draw_list: *mut sys::ImDrawList,
        window_scoped: bool,
    ) -> Self {
        Self {
            scope: ui.begin_native_scope(
                crate::scope::NativeScopePop::DrawListPopClipRect {
                    draw_list,
                    window_scoped,
                },
                "DrawListClipRectToken",
            ),
        }
    }

    /// Pop the clip rectangle immediately instead of waiting for drop.
    ///
    /// # Panics
    ///
    /// Panics before FFI if a later clip token for the same draw list is active or a window-scoped
    /// draw list is no longer in its originating window `Begin` scope.
    #[doc(alias = "PopClipRect")]
    pub fn pop(self) {}

    /// Pop the clip rectangle immediately instead of waiting for drop.
    ///
    /// # Panics
    ///
    /// Panics under the same conditions as [`Self::pop`].
    #[doc(alias = "PopClipRect")]
    pub fn end(self) {}
}

impl Drop for DrawListClipRectToken<'_> {
    fn drop(&mut self) {
        self.scope.finish();
    }
}

/// Represent the drawing interface within a call to `channels_split`.
pub struct ChannelsSplit<'ui> {
    pub(super) draw_list: &'ui DrawListMut<'ui>,
    pub(super) channels_count: usize,
}

impl<'ui> ChannelsSplit<'ui> {
    pub(super) fn new(draw_list: &'ui DrawListMut<'ui>, channels_count: usize) -> Self {
        Self {
            draw_list,
            channels_count,
        }
    }

    /// Change the current channel. Panics if `channel_index >= channels_count` or a native
    /// extension is using this draw list's splitter.
    #[doc(alias = "ChannelsSetCurrent")]
    pub fn set_current(&self, channel_index: usize) {
        assert!(
            channel_index < self.channels_count,
            "Channel index {} out of range {}",
            channel_index,
            self.channels_count
        );
        let channel_index_i32 = count_to_i32(
            "ChannelsSplit::set_current()",
            "channel_index",
            channel_index,
        );
        self.draw_list
            .with_raw_bound("ChannelsSplit::set_current()", |draw_list| {
                self.draw_list.assert_splitter_available();
                unsafe { sys::ImDrawList_ChannelsSetCurrent(draw_list, channel_index_i32) };
            });
    }
}

struct DrawFlagsGuard<'draw_list> {
    draw_list: *mut sys::ImDrawList,
    scope: Option<crate::scope::NativeScopeToken<'draw_list>>,
}

impl Drop for DrawFlagsGuard<'_> {
    fn drop(&mut self) {
        if let Some(scope) = self.scope.as_mut() {
            scope.finish();
        } else {
            // Standalone draw lists used by internal native tests have no Ui scope.
            unsafe { sys::ImDrawList_PopDrawFlag(self.draw_list) };
        }
    }
}

struct ChannelsSplitMergeGuard<'ui>(NativeScopeToken<'ui>);

impl Drop for ChannelsSplitMergeGuard<'_> {
    fn drop(&mut self) {
        self.0.finish();
    }
}

impl<'ui> DrawListMut<'ui> {
    /// Push a clip rectangle, optionally intersecting with the current clip rect.
    #[doc(alias = "PushClipRect")]
    pub fn push_clip_rect(
        &self,
        clip_rect_min: impl Into<sys::ImVec2>,
        clip_rect_max: impl Into<sys::ImVec2>,
        intersect_with_current: bool,
    ) -> DrawListClipRectToken<'_> {
        let clip_rect_min = finite_vec2(
            "DrawListMut::push_clip_rect()",
            "clip_rect_min",
            clip_rect_min,
        );
        let clip_rect_max = finite_vec2(
            "DrawListMut::push_clip_rect()",
            "clip_rect_max",
            clip_rect_max,
        );

        self.with_raw_bound("DrawListMut::push_clip_rect()", |draw_list| {
            unsafe {
                sys::ImDrawList_PushClipRect(
                    draw_list,
                    clip_rect_min,
                    clip_rect_max,
                    intersect_with_current,
                )
            };
            DrawListClipRectToken::new(self.ui(), draw_list, self.is_window_scoped())
        })
    }

    /// Push a full-screen clip rectangle.
    #[doc(alias = "PushClipRectFullScreen")]
    pub fn push_clip_rect_full_screen(&self) -> DrawListClipRectToken<'_> {
        self.with_raw_bound("DrawListMut::push_clip_rect_full_screen()", |draw_list| {
            unsafe { sys::ImDrawList_PushClipRectFullScreen(draw_list) };
            DrawListClipRectToken::new(self.ui(), draw_list, self.is_window_scoped())
        })
    }

    /// Get current minimum clip rectangle point.
    pub fn clip_rect_min(&self) -> [f32; 2] {
        self.with_raw_bound("DrawListMut::clip_rect_min()", |draw_list| unsafe {
            sys::ImDrawList_GetClipRectMin(draw_list).into()
        })
    }

    /// Get current maximum clip rectangle point.
    pub fn clip_rect_max(&self) -> [f32; 2] {
        self.with_raw_bound("DrawListMut::clip_rect_max()", |draw_list| unsafe {
            sys::ImDrawList_GetClipRectMax(draw_list).into()
        })
    }

    /// Convenience: push a clip rect, run f, pop.
    pub fn with_clip_rect<F, R>(
        &self,
        clip_rect_min: impl Into<sys::ImVec2>,
        clip_rect_max: impl Into<sys::ImVec2>,
        f: F,
    ) -> R
    where
        F: FnOnce() -> R,
    {
        let clip_rect = self.push_clip_rect(clip_rect_min, clip_rect_max, false);
        let result = f();
        drop(clip_rect);
        result
    }

    pub(in crate::draw) fn assert_splitter_available(&self) {
        if let Some(ui) = self.ui {
            ui.assert_draw_list_splitter_available(self.draw_list);
        }
    }

    /// Split draw into multiple channels and merge automatically at the end of the closure.
    ///
    /// # Panics
    ///
    /// Panics if the count is outside `1..=i32::MAX`, this draw list is already split,
    /// or a native extension is using its splitter.
    #[doc(alias = "ChannelsSplit")]
    pub fn channels_split(&self, channels_count: usize, f: impl FnOnce(&ChannelsSplit<'_>)) {
        assert!(channels_count > 0, "channels_count must be greater than 0");
        let channels_count_i32 = count_to_i32(
            "DrawListMut::channels_split()",
            "channels_count",
            channels_count,
        );

        let ui = self.ui();
        ui.with_bound_context(|| {
            self.assert_bound_scope("DrawListMut::channels_split()");
            self.assert_splitter_available();
            let draw_list = self.draw_list;
            let splitter = unsafe { &(*draw_list)._Splitter };
            assert!(
                splitter._Current == 0 && splitter._Count <= 1,
                "DrawListMut::channels_split() does not support nested splits for the same draw list"
            );

            unsafe { sys::ImDrawList_ChannelsSplit(draw_list, channels_count_i32) };
            let _merge_guard = ChannelsSplitMergeGuard(ui.begin_native_scope(
                NativeScopePop::DrawListMergeChannels {
                    draw_list,
                    window_scoped: self.is_window_scoped(),
                },
                "DrawListMut::channels_split()",
            ));
            f(&ChannelsSplit::new(self, channels_count));
        });
    }

    /// Return the currently enabled public drawing flags.
    pub fn flags(&self) -> DrawFlags {
        self.with_raw_bound("DrawListMut::flags()", |draw_list| {
            DrawFlags::from_bits_truncate(unsafe { (*draw_list).Flags } as u32)
        })
    }

    /// Enable or disable drawing options while `f` runs, then restore prior defaults.
    ///
    /// The scope may configure anti-aliasing, legacy stroke positioning, text pixel
    /// snapping, rounded corners, and texture/vertex-offset options. Other stroke
    /// positions, closure, joins, and caps must be configured on individual primitives.
    ///
    /// Scopes nest and restore their state during unwinding. Dear ImGui currently
    /// supports three nested flag scopes per draw list; exceeding its capacity panics
    /// before touching the native stack. Unknown flags and unsupported scope options
    /// also panic before mutation. Enabling rounding flags that conflict with the
    /// current scope panics as well; disable the prior rounding mode first when
    /// switching modes. A window draw list must remain in its originating window
    /// scope until the closure returns. Vertex offsets require renderer support.
    ///
    /// ```no_run
    /// # use dear_imgui_rs::{DrawFlags, DrawListMut};
    /// # fn draw(draw_list: &DrawListMut<'_>) {
    /// draw_list.with_draw_flags(DrawFlags::AA_LINE_ENDS, true, || {
    ///     draw_list.add_line([10.0, 10.0], [30.0, 20.0], [1.0; 4])
    ///         .thickness(4.0)
    ///         .flags(DrawFlags::CAP_SQUARE)
    ///         .build();
    /// });
    /// # }
    /// ```
    #[doc(alias = "PushDrawFlag", alias = "PopDrawFlag")]
    pub fn with_draw_flags<R>(&self, flags: DrawFlags, enabled: bool, f: impl FnOnce() -> R) -> R {
        const CALLER: &str = "DrawListMut::with_draw_flags()";
        assert_draw_flags(
            CALLER,
            flags,
            DrawFlags::from_bits_retain(sys::ImDrawFlags_AllowInPushScope_ as u32),
        );
        let stroke = flags.bits() & sys::ImDrawFlags_StrokeMask_ as u32;
        assert!(
            stroke == 0 || stroke == DrawFlags::STROKE_LEGACY.bits(),
            "DrawListMut::with_draw_flags() only supports STROKE_LEGACY as a scoped stroke position"
        );
        let guard = self.with_raw_bound(CALLER, |draw_list| unsafe {
            let current = DrawFlags::from_bits_truncate((*draw_list).Flags as u32);
            let effective = if enabled {
                current | flags
            } else {
                current.difference(flags)
            };
            assert_draw_flags(CALLER, effective & CORNER_FLAGS, CORNER_FLAGS);
            let stack = &(*draw_list)._DrawFlagsStack;
            assert!(
                usize::from(stack.Size) < stack.LocalData.len(),
                "DrawListMut::with_draw_flags() exceeded the native draw-flag scope capacity"
            );
            sys::ImDrawList_PushDrawFlag(draw_list, flags.bits() as sys::ImDrawFlags, enabled);
            DrawFlagsGuard {
                draw_list,
                scope: self.ui.map(|ui| {
                    ui.begin_native_scope(
                        crate::scope::NativeScopePop::DrawListPopDrawFlag {
                            draw_list,
                            window_scoped: self.is_window_scoped(),
                        },
                        "DrawListDrawFlagScope",
                    )
                }),
            }
        });
        let result = f();
        drop(guard);
        result
    }

    /// Disable text pixel snapping while `f` runs, then restore draw-list defaults.
    ///
    /// Restoration also occurs if `f` panics. See [`Self::with_draw_flags`] for
    /// nesting limits and other configurable drawing options.
    pub fn with_text_no_pixel_snap<R>(&self, f: impl FnOnce() -> R) -> R {
        self.with_draw_flags(DrawFlags::TEXT_NO_PIXEL_SNAP, true, f)
    }

    /// Draw a text whose upper-left corner is at point `pos`.
    pub fn add_text(
        &self,
        pos: impl Into<sys::ImVec2>,
        col: impl Into<ImColor32>,
        text: impl AsRef<str>,
    ) {
        use std::os::raw::c_char;

        let text = text.as_ref();
        let pos = finite_vec2("DrawListMut::add_text()", "pos", pos);
        let col = col.into();

        self.with_raw_bound("DrawListMut::add_text()", |draw_list| unsafe {
            let start = text.as_ptr().cast::<c_char>();
            let end = start.add(text.len());
            sys::ImDrawList_AddText_Vec2(draw_list, pos, col.into(), start, end);
        });
    }

    /// Draw text with an explicit font and optional fine CPU clip rectangle.
    ///
    /// This mirrors Dear ImGui's `ImDrawList::AddText(ImFont*, ...)` overload.
    #[doc(alias = "AddText")]
    #[allow(
        clippy::too_many_arguments,
        reason = "mirrors the ImDrawList::AddText overload"
    )]
    pub fn add_text_with_font(
        &self,
        font: crate::fonts::FontId,
        font_size: f32,
        pos: impl Into<sys::ImVec2>,
        col: impl Into<ImColor32>,
        text: impl AsRef<str>,
        wrap_width: f32,
        cpu_fine_clip_rect: Option<[f32; 4]>,
    ) {
        use std::os::raw::c_char;
        let text = text.as_ref();
        let pos = finite_vec2("DrawListMut::add_text_with_font()", "pos", pos);
        assert_non_negative_f32("DrawListMut::add_text_with_font()", "font_size", font_size);
        assert_non_negative_f32(
            "DrawListMut::add_text_with_font()",
            "wrap_width",
            wrap_width,
        );
        let col = col.into();
        let clip_vec4 = cpu_fine_clip_rect
            .map(|r| finite_vec4("DrawListMut::add_text_with_font()", "cpu_fine_clip_rect", r));
        let clip_ptr = match clip_vec4.as_ref() {
            Some(v) => v as *const sys::ImVec4,
            None => std::ptr::null(),
        };

        self.with_raw_bound("DrawListMut::add_text_with_font()", |draw_list| unsafe {
            let font_ptr = crate::fonts::validate_font_id_for_context(
                font,
                self.ui().context_raw(),
                "DrawListMut::add_text_with_font()",
            );
            let start = text.as_ptr().cast::<c_char>();
            let end = start.add(text.len());
            sys::ImDrawList_AddText_FontPtr(
                draw_list,
                font_ptr,
                font_size,
                pos,
                col.into(),
                start,
                end,
                wrap_width,
                clip_ptr,
            );
        });
    }
}

/// Non-null native callback accepted by [`DrawListMut::add_callback`].
pub type RawDrawCallback =
    unsafe extern "C" fn(parent_list: *const sys::ImDrawList, command: *const sys::ImDrawCmd);

impl<'ui> DrawListMut<'ui> {
    /// Insert a raw draw callback.
    ///
    /// # Safety
    ///
    /// - `callback` must not unwind across the FFI boundary.
    /// - When `userdata_size` is zero, Dear ImGui stores `userdata` verbatim. Any non-null pointer
    ///   must remain valid until a renderer executes the draw command. A renderer may reject or
    ///   discard callback-bearing data, so this mode must not rely on callback execution to reclaim
    ///   uniquely owned Rust memory.
    /// - When `userdata_size` is non-zero, `userdata` must point to that many readable bytes for
    ///   this call. Dear ImGui copies the bytes synchronously and later supplies a pointer into its
    ///   internal byte buffer. The callback must not free that pointer or assume it is aligned for
    ///   the original Rust type; copy the bytes out or use unaligned reads.
    /// - The callback and userdata interpretation must remain ABI-compatible with every renderer
    ///   that may consume the draw list.
    /// - Arbitrary Rust function pointers are not supported by the import-style WASM provider;
    ///   only provider-native callbacks with a proven shared callback table may be used there.
    ///
    /// ```compile_fail
    /// # use dear_imgui_rs::Context;
    /// # let mut context = Context::create();
    /// # let ui = context.frame();
    /// unsafe extern "C" fn callback(
    ///     _list: *const dear_imgui_rs::sys::ImDrawList,
    ///     _command: *const dear_imgui_rs::sys::ImDrawCmd,
    /// ) {}
    /// ui.with_window_draw_list(|draw_list| {
    ///     draw_list.add_callback(callback, std::ptr::null_mut(), 0);
    /// });
    /// ```
    ///
    /// Rust-owned closure callbacks are intentionally unavailable: a draw command may be dropped
    /// without ever reaching a renderer, so the command stream cannot guarantee closure cleanup.
    ///
    /// ```compile_fail
    /// # use dear_imgui_rs::Context;
    /// # let mut context = Context::create();
    /// # let ui = context.frame();
    /// ui.with_window_draw_list(|draw_list| draw_list.add_callback_safe(|| {}).build());
    /// ```
    #[doc(alias = "AddCallback")]
    pub unsafe fn add_callback(
        &self,
        callback: RawDrawCallback,
        userdata: *mut std::os::raw::c_void,
        userdata_size: usize,
    ) {
        assert!(
            userdata_size < (1usize << 31),
            "DrawListMut::add_callback() userdata_size must be smaller than 2^31 bytes"
        );
        assert!(
            userdata_size == 0 || !userdata.is_null(),
            "DrawListMut::add_callback() copied userdata must not be null"
        );
        self.with_raw_bound("DrawListMut::add_callback()", |draw_list| unsafe {
            sys::ImDrawList_AddCallback(draw_list, Some(callback), userdata, userdata_size)
        });
    }

    /// Insert a new draw command (forces a new draw call boundary).
    #[doc(alias = "AddDrawCmd")]
    pub fn add_draw_cmd(&self) {
        self.with_raw_bound("DrawListMut::add_draw_cmd()", |draw_list| unsafe {
            sys::ImDrawList_AddDrawCmd(draw_list)
        });
    }
}
