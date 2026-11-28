//! Push/pop stacks (fonts, style, etc.)
//!
//! RAII-style wrappers for ImGui parameter stacks: fonts, style colors/vars and
//! more. Tokens returned by `Ui::push_*` pop automatically when dropped.
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions,
    clippy::unnecessary_cast
)]

use crate::fonts::FontId;
use crate::style::{
    StyleColor, StyleVar, StyleVarVec2, validate_style_color, validate_style_var,
    validate_style_var_component,
};
use crate::{Ui, sys};
use bitflags::bitflags;

create_token!(
    /// Tracks a font pushed to the font stack that can be popped by calling `.end()`
    /// or by dropping.
    #[doc(alias = "PopFont")]
    pub struct FontStackToken<'ui>;

    pop crate::scope::NativeScopePop::PopFont;

    /// Pops a change from the font stack.
    drop { unsafe { sys::igPopFont() } }
);

impl FontStackToken<'_> {
    /// Pops a change from the font stack.
    ///
    /// # Panics
    ///
    /// Panics under the same conditions as [`Self::end`].
    pub fn pop(self) {
        self.end()
    }
}

create_token!(
    /// Tracks an ID pushed to the ID stack that can be popped by calling `.pop()`
    /// or by dropping. See [`crate::Ui::push_id`] for more details.
    #[doc(alias = "PopID")]
    pub struct IdStackToken<'ui>;

    pop crate::scope::NativeScopePop::PopId;

    /// Pops a change from the ID stack
    drop { unsafe { sys::igPopID() } }
);

impl IdStackToken<'_> {
    /// Pops a change from the ID stack.
    ///
    /// # Panics
    ///
    /// Panics under the same conditions as [`Self::end`].
    pub fn pop(self) {
        self.end()
    }
}

// ============================================================================
// Focus scope stack
// ============================================================================

create_token!(
    /// Tracks a pushed focus scope, popped on drop.
    pub struct FocusScopeToken<'ui>;

    pop crate::scope::NativeScopePop::PopFocusScope;

    /// Pops a focus scope.
    #[doc(alias = "PopFocusScope")]
    drop { unsafe { sys::igPopFocusScope() } }
);

/// Input to [`Ui::push_id`], distinct from the resolved numeric [`crate::Id`].
#[derive(Copy, Clone, Debug)]
pub enum Id<'a> {
    /// Integer identifier
    Int(i32),
    /// String identifier
    Str(&'a str),
    /// Pointer identifier
    Ptr(*const std::ffi::c_void),
}

impl From<i32> for Id<'_> {
    fn from(i: i32) -> Self {
        Id::Int(i)
    }
}

impl From<usize> for Id<'_> {
    fn from(i: usize) -> Self {
        Id::Int(i as i32)
    }
}

impl<'a> From<&'a str> for Id<'a> {
    fn from(s: &'a str) -> Self {
        Id::Str(s)
    }
}

impl<'a> From<&'a String> for Id<'a> {
    fn from(s: &'a String) -> Self {
        Id::Str(s.as_str())
    }
}

impl<T> From<*const T> for Id<'_> {
    fn from(p: *const T) -> Self {
        Id::Ptr(p as *const std::ffi::c_void)
    }
}

impl<T> From<*mut T> for Id<'_> {
    fn from(p: *mut T) -> Self {
        Id::Ptr(p as *const std::ffi::c_void)
    }
}

bitflags! {
    /// Flags that can be applied to subsequently submitted items.
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ItemFlags: i32 {
        /// No item flags.
        const NONE = sys::ImGuiItemFlags_None as i32;
        /// Disable keyboard tabbing while retaining directional navigation.
        const NO_TAB_STOP = sys::ImGuiItemFlags_NoTabStop as i32;
        /// Disable keyboard and gamepad navigation.
        const NO_NAV = sys::ImGuiItemFlags_NoNav as i32;
        /// Prevent the item from receiving default navigation focus.
        const NO_NAV_DEFAULT_FOCUS = sys::ImGuiItemFlags_NoNavDefaultFocus as i32;
        /// Enable repeat behavior for button-like items.
        const BUTTON_REPEAT = sys::ImGuiItemFlags_ButtonRepeat as i32;
        /// Automatically close a parent popup after activating a menu item or selectable.
        const AUTO_CLOSE_POPUPS = sys::ImGuiItemFlags_AutoClosePopups as i32;
        /// Allow duplicate item IDs without a debug conflict warning.
        const ALLOW_DUPLICATE_ID = sys::ImGuiItemFlags_AllowDuplicateId as i32;
        /// Apply `InputText` keyboard edits to the backing value while typing.
        const LIVE_EDIT_ON_INPUT_TEXT = sys::ImGuiItemFlags_LiveEditOnInputText as i32;
        /// Apply scalar keyboard edits to the backing value while typing.
        const LIVE_EDIT_ON_INPUT_SCALAR = sys::ImGuiItemFlags_LiveEditOnInputScalar as i32;
        /// Apply text and scalar keyboard edits to backing values while typing.
        const LIVE_EDIT_ON_INPUT = sys::ImGuiItemFlags_LiveEditOnInput as i32;
        /// Display a mixed or indeterminate value, useful for multi-object property editors.
        const MIXED_VALUE = sys::ImGuiItemFlags_MixedValue as i32;
    }
}

bitflags! {
    /// Flags recorded for the last submitted item.
    ///
    /// This includes the public flags accepted by [`Ui::push_item_flag`] plus
    /// read-only state such as [`Self::DISABLED`]. Unknown internal bits are
    /// retained when returned by [`Ui::item_flags`].
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ItemStateFlags: i32 {
        /// No item flags.
        const NONE = sys::ImGuiItemFlags_None as i32;
        /// Keyboard tabbing was disabled while retaining directional navigation.
        const NO_TAB_STOP = sys::ImGuiItemFlags_NoTabStop as i32;
        /// Keyboard and gamepad navigation was disabled.
        const NO_NAV = sys::ImGuiItemFlags_NoNav as i32;
        /// The item could not receive default navigation focus.
        const NO_NAV_DEFAULT_FOCUS = sys::ImGuiItemFlags_NoNavDefaultFocus as i32;
        /// Repeat behavior was enabled for the item.
        const BUTTON_REPEAT = sys::ImGuiItemFlags_ButtonRepeat as i32;
        /// The item could automatically close its parent popup.
        const AUTO_CLOSE_POPUPS = sys::ImGuiItemFlags_AutoClosePopups as i32;
        /// Duplicate IDs were allowed for the item.
        const ALLOW_DUPLICATE_ID = sys::ImGuiItemFlags_AllowDuplicateId as i32;
        /// `InputText` keyboard edits were applied while typing.
        const LIVE_EDIT_ON_INPUT_TEXT = sys::ImGuiItemFlags_LiveEditOnInputText as i32;
        /// Scalar keyboard edits were applied while typing.
        const LIVE_EDIT_ON_INPUT_SCALAR = sys::ImGuiItemFlags_LiveEditOnInputScalar as i32;
        /// Text and scalar keyboard edits were applied while typing.
        const LIVE_EDIT_ON_INPUT = sys::ImGuiItemFlags_LiveEditOnInput as i32;
        /// The item represented a mixed or indeterminate value.
        const MIXED_VALUE = sys::ImGuiItemFlags_MixedValue as i32;
        /// The last item was disabled.
        const DISABLED = sys::ImGuiItemFlags_Disabled as i32;
    }
}

impl Default for ItemFlags {
    fn default() -> Self {
        Self::NONE
    }
}

impl Default for ItemStateFlags {
    fn default() -> Self {
        Self::NONE
    }
}

impl From<ItemFlags> for ItemStateFlags {
    fn from(flags: ItemFlags) -> Self {
        Self::from_bits_retain(flags.bits())
    }
}

create_token!(
    /// Tracks item flags pushed with [`Ui::push_item_flag`].
    pub struct ItemFlagStackToken<'ui>;

    pop crate::scope::NativeScopePop::PopItemFlag;

    /// Pops item flags pushed with [`Ui::push_item_flag`].
    #[doc(alias = "PopItemFlag")]
    drop { unsafe { sys::igPopItemFlag() } }
);

impl ItemFlagStackToken<'_> {
    /// Pops the item flag scope.
    ///
    /// # Panics
    ///
    /// Panics under the same conditions as [`Self::end`].
    pub fn pop(self) {
        self.end()
    }
}

create_token!(
    /// Tracks a change made with [`Ui::push_item_width`] that can be popped
    /// by calling [`ItemWidthStackToken::end`] or dropping.
    #[doc(alias = "PopItemWidth")]
    pub struct ItemWidthStackToken<'ui>;

    pop crate::scope::NativeScopePop::PopItemWidth;

    /// Pops an item width change made with [`Ui::push_item_width`].
    #[doc(alias = "PopItemWidth")]
    drop { unsafe { sys::igPopItemWidth() } }
);

create_token!(
    /// Tracks a change made with [`Ui::push_text_wrap_pos`] that can be popped
    /// by calling [`TextWrapPosStackToken::end`] or dropping.
    #[doc(alias = "PopTextWrapPos")]
    pub struct TextWrapPosStackToken<'ui>;

    pop crate::scope::NativeScopePop::PopTextWrap;

    /// Pops a text wrap position change made with [`Ui::push_text_wrap_pos`].
    #[doc(alias = "PopTextWrapPos")]
    drop { unsafe { sys::igPopTextWrapPos() } }
);

create_token!(
    /// Tracks a color pushed to the color stack that can be popped by calling `.end()`
    /// or by dropping.
    #[doc(alias = "PopStyleColor")]
    pub struct ColorStackToken<'ui>;

    pop crate::scope::NativeScopePop::PopStyleColor;

    /// Pops a change from the color stack.
    drop { unsafe { sys::igPopStyleColor(1) } }
);

impl ColorStackToken<'_> {
    /// Pops a change from the color stack.
    ///
    /// # Panics
    ///
    /// Panics under the same conditions as [`Self::end`].
    pub fn pop(self) {
        self.end()
    }
}

create_token!(
    /// Tracks a style pushed to the style stack that can be popped by calling `.end()`
    /// or by dropping.
    #[doc(alias = "PopStyleVar")]
    pub struct StyleStackToken<'ui>;

    pop crate::scope::NativeScopePop::PopStyleVar { is_alpha: false };

    /// Pops a change from the style stack.
    drop { unsafe { sys::igPopStyleVar(1) } }
);

impl<'ui> StyleStackToken<'ui> {
    fn new_for(ui: &'ui Ui, is_alpha: bool) -> Self {
        Self {
            scope: ui.begin_native_scope(
                crate::scope::NativeScopePop::PopStyleVar { is_alpha },
                "StyleStackToken",
            ),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Pops a change from the style stack.
    ///
    /// # Panics
    ///
    /// Panics under the same conditions as [`Self::end`].
    pub fn pop(self) {
        self.end()
    }
}

/// Helper function to push style variables
unsafe fn push_style_var(style_var: StyleVar) {
    use StyleVar::*;
    match style_var {
        Alpha(v) => unsafe { sys::igPushStyleVar_Float(sys::ImGuiStyleVar_Alpha as i32, v) },
        DisabledAlpha(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_DisabledAlpha as i32, v)
        },
        WindowPadding(v) => {
            let p: [f32; 2] = v;
            let vec = sys::ImVec2 { x: p[0], y: p[1] };
            unsafe { sys::igPushStyleVar_Vec2(sys::ImGuiStyleVar_WindowPadding as i32, vec) }
        }
        WindowRounding(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_WindowRounding as i32, v)
        },
        WindowBorderSize(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_WindowBorderSize as i32, v)
        },
        WindowMinSize(v) => {
            let p: [f32; 2] = v;
            let vec = sys::ImVec2 { x: p[0], y: p[1] };
            unsafe { sys::igPushStyleVar_Vec2(sys::ImGuiStyleVar_WindowMinSize as i32, vec) }
        }
        WindowTitleAlign(v) => {
            let p: [f32; 2] = v;
            let vec = sys::ImVec2 { x: p[0], y: p[1] };
            unsafe { sys::igPushStyleVar_Vec2(sys::ImGuiStyleVar_WindowTitleAlign as i32, vec) }
        }
        ChildRounding(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_ChildRounding as i32, v)
        },
        ChildBorderSize(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_ChildBorderSize as i32, v)
        },
        PopupRounding(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_PopupRounding as i32, v)
        },
        PopupBorderSize(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_PopupBorderSize as i32, v)
        },
        FramePadding(v) => {
            let p: [f32; 2] = v;
            let vec = sys::ImVec2 { x: p[0], y: p[1] };
            unsafe { sys::igPushStyleVar_Vec2(sys::ImGuiStyleVar_FramePadding as i32, vec) }
        }
        FrameRounding(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_FrameRounding as i32, v)
        },
        FrameBorderSize(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_FrameBorderSize as i32, v)
        },
        ItemSpacing(v) => {
            let p: [f32; 2] = v;
            let vec = sys::ImVec2 { x: p[0], y: p[1] };
            unsafe { sys::igPushStyleVar_Vec2(sys::ImGuiStyleVar_ItemSpacing as i32, vec) }
        }
        ItemInnerSpacing(v) => {
            let p: [f32; 2] = v;
            let vec = sys::ImVec2 { x: p[0], y: p[1] };
            unsafe { sys::igPushStyleVar_Vec2(sys::ImGuiStyleVar_ItemInnerSpacing as i32, vec) }
        }
        IndentSpacing(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_IndentSpacing as i32, v)
        },
        CellPadding(v) => {
            let p: [f32; 2] = v;
            let vec = sys::ImVec2 { x: p[0], y: p[1] };
            unsafe { sys::igPushStyleVar_Vec2(sys::ImGuiStyleVar_CellPadding as i32, vec) }
        }
        ScrollbarSize(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_ScrollbarSize as i32, v)
        },
        ScrollbarRounding(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_ScrollbarRounding as i32, v)
        },
        ScrollbarPadding(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_ScrollbarPadding as i32, v)
        },
        GrabMinSize(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_GrabMinSize as i32, v)
        },
        GrabRounding(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_GrabRounding as i32, v)
        },
        ImageRounding(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_ImageRounding as i32, v)
        },
        ImageBorderSize(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_ImageBorderSize as i32, v)
        },
        TabRounding(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_TabRounding as i32, v)
        },
        TabBorderSize(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_TabBorderSize as i32, v)
        },
        TabMinWidthBase(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_TabMinWidthBase as i32, v)
        },
        TabMinWidthShrink(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_TabMinWidthShrink as i32, v)
        },
        TabBarBorderSize(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_TabBarBorderSize as i32, v)
        },
        TabBarOverlineSize(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_TabBarOverlineSize as i32, v)
        },
        TableAngledHeadersAngle(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_TableAngledHeadersAngle as i32, v)
        },
        TableAngledHeadersTextAlign(v) => {
            let p: [f32; 2] = v;
            let vec = sys::ImVec2 { x: p[0], y: p[1] };
            unsafe {
                sys::igPushStyleVar_Vec2(sys::ImGuiStyleVar_TableAngledHeadersTextAlign as i32, vec)
            }
        }
        TreeLinesSize(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_TreeLinesSize as i32, v)
        },
        TreeLinesRounding(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_TreeLinesRounding as i32, v)
        },
        MenuItemRounding(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_MenuItemRounding as i32, v)
        },
        SelectableRounding(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_SelectableRounding as i32, v)
        },
        DragDropTargetRounding(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_DragDropTargetRounding as i32, v)
        },
        ButtonTextAlign(v) => {
            let p: [f32; 2] = v;
            let vec = sys::ImVec2 { x: p[0], y: p[1] };
            unsafe { sys::igPushStyleVar_Vec2(sys::ImGuiStyleVar_ButtonTextAlign as i32, vec) }
        }
        SelectableTextAlign(v) => {
            let p: [f32; 2] = v;
            let vec = sys::ImVec2 { x: p[0], y: p[1] };
            unsafe { sys::igPushStyleVar_Vec2(sys::ImGuiStyleVar_SelectableTextAlign as i32, vec) }
        }
        SeparatorSize(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_SeparatorSize as i32, v)
        },
        SeparatorTextBorderSize(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_SeparatorTextBorderSize as i32, v)
        },
        SeparatorTextAlign(v) => {
            let p: [f32; 2] = v;
            let vec = sys::ImVec2 { x: p[0], y: p[1] };
            unsafe { sys::igPushStyleVar_Vec2(sys::ImGuiStyleVar_SeparatorTextAlign as i32, vec) }
        }
        SeparatorTextPadding(v) => {
            let p: [f32; 2] = v;
            let vec = sys::ImVec2 { x: p[0], y: p[1] };
            unsafe { sys::igPushStyleVar_Vec2(sys::ImGuiStyleVar_SeparatorTextPadding as i32, vec) }
        }
        DockingSeparatorSize(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_DockingSeparatorSize as i32, v)
        },
    }
}

impl Ui {
    /// Switches to the given font at its configured reference size.
    ///
    /// Dear ImGui 1.92 can rasterize a font at multiple sizes. This convenience
    /// method preserves the pre-1.92 behavior by using the reference size
    /// supplied when the font was added. Use [`Ui::push_font_with_size`] to
    /// preserve the current size or select another runtime size explicitly.
    /// A font without a reference size also preserves the current size.
    ///
    /// Returns a `FontStackToken` that must be popped by calling `.pop()`
    ///
    /// # Panics
    ///
    /// Panics before calling Dear ImGui if the `FontId` came from a different atlas,
    /// was invalidated by font atlas mutation, or is no longer present in the
    /// current context's atlas.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let font_data_sources = [];
    /// // At initialization time
    /// let my_custom_font = ctx.font_atlas().add_font(&font_data_sources);
    /// # let ui = ctx.frame();
    /// // During UI construction
    /// let font = ui.push_font(my_custom_font);
    /// ui.text("I use the custom font!");
    /// font.pop();
    /// ```
    #[doc(alias = "PushFont")]
    pub fn push_font(&self, id: FontId) -> FontStackToken<'_> {
        self.run_with_bound_context(|| unsafe {
            let font_ptr =
                crate::fonts::validate_font_id_for_current_context(id, "Ui::push_font()");
            sys::igPushFont(font_ptr, (*font_ptr).LegacySize);
        });
        FontStackToken::new(self)
    }

    /// Pushes an identifier to the ID stack.
    ///
    /// Returns an `IdStackToken` that can be popped by calling `.end()`
    /// or by dropping manually.
    ///
    /// # Examples
    /// Dear ImGui uses labels to uniquely identify widgets. For a good explanation, see this part of the [Dear ImGui FAQ][faq]
    ///
    /// [faq]: https://github.com/ocornut/imgui/blob/v1.84.2/docs/FAQ.md#q-why-is-my-widget-not-reacting-when-i-click-on-it
    ///
    /// In `dear-imgui-rs` the same applies, we can manually specify labels with the `##` syntax:
    ///
    /// ```no_run
    /// # let mut imgui = dear_imgui_rs::Context::create();
    /// # let ui = imgui.frame();
    ///
    /// ui.button("Click##button1");
    /// ui.button("Click##button2");
    /// ```
    ///
    /// But sometimes we want to create widgets in a loop, or we want to avoid
    /// having to manually give each widget a unique label. In these cases, we can
    /// push an ID to the ID stack:
    ///
    /// ```no_run
    /// # let mut imgui = dear_imgui_rs::Context::create();
    /// # let ui = imgui.frame();
    ///
    /// for i in 0..10 {
    ///     let _id = ui.push_id(i);
    ///     ui.button("Click");
    /// }
    /// ```
    #[doc(alias = "PushID")]
    pub fn push_id<'a, T: Into<Id<'a>>>(&self, id: T) -> IdStackToken<'_> {
        let id = id.into();
        self.run_with_bound_context(|| unsafe {
            match id {
                Id::Int(i) => sys::igPushID_Int(i),
                Id::Str(s) => {
                    let start = if s.is_empty() {
                        c"".as_ptr()
                    } else {
                        s.as_ptr().cast()
                    };
                    sys::igPushID_StrStr(start, start.add(s.len()));
                }
                Id::Ptr(p) => sys::igPushID_Ptr(p),
            }
        });
        IdStackToken::new(self)
    }

    /// Push a focus scope (affects e.g. navigation focus allocation).
    ///
    /// Returns a `FocusScopeToken` which will pop the focus scope when dropped.
    #[doc(alias = "PushFocusScope")]
    pub fn push_focus_scope(&self, id: crate::Id) -> FocusScopeToken<'_> {
        self.run_with_bound_context(|| unsafe { sys::igPushFocusScope(id.raw()) });
        FocusScopeToken::new(self)
    }

    /// Returns the flags recorded for the last submitted item.
    ///
    /// Unknown bits introduced by newer Dear ImGui versions are retained.
    #[doc(alias = "GetItemFlags")]
    pub fn item_flags(&self) -> ItemStateFlags {
        self.run_with_bound_context(|| unsafe {
            ItemStateFlags::from_bits_retain(sys::igGetItemFlags())
        })
    }

    /// Enables or disables flags for subsequently submitted items.
    ///
    /// The returned token restores the previous item flags when dropped.
    #[doc(alias = "PushItemFlag")]
    pub fn push_item_flag(&self, flags: ItemFlags, enabled: bool) -> ItemFlagStackToken<'_> {
        self.run_with_bound_context(|| unsafe { sys::igPushItemFlag(flags.bits(), enabled) });
        ItemFlagStackToken::new(self)
    }

    /// Runs `f` with the requested item flags enabled or disabled.
    ///
    /// The previous flags are restored even if `f` panics.
    #[doc(alias = "PushItemFlag", alias = "PopItemFlag")]
    pub fn with_item_flag<R>(&self, flags: ItemFlags, enabled: bool, f: impl FnOnce() -> R) -> R {
        let flags = self.push_item_flag(flags, enabled);
        let result = f();
        drop(flags);
        result
    }

    /// Changes the item width by pushing a change to the item width stack.
    ///
    /// Returns an `ItemWidthStackToken`. The pushed width item is popped when either
    /// `ItemWidthStackToken` goes out of scope, or `.end()` is called.
    ///
    /// - `> 0.0`: width is `item_width` pixels
    /// - `= 0.0`: default to ~2/3 of window width
    /// - `< 0.0`: `item_width` pixels relative to the right of window (-1.0 always aligns width to
    ///   the right side)
    #[doc(alias = "PushItemWidth")]
    pub fn push_item_width(&self, item_width: f32) -> ItemWidthStackToken<'_> {
        self.run_with_bound_context(|| unsafe { sys::igPushItemWidth(item_width) });
        ItemWidthStackToken::new(self)
    }

    /// Sets the width of the next item(s) to be the same as the width of the given text.
    ///
    /// Text is measured with [`Ui::calc_text_size`] using its default options.
    ///
    /// Returns an `ItemWidthStackToken`. The pushed width item is popped when either
    /// `ItemWidthStackToken` goes out of scope, or `.end()` is called.
    #[doc(alias = "PushItemWidth")]
    pub fn push_item_width_text(&self, text: impl AsRef<str>) -> ItemWidthStackToken<'_> {
        let text = text.as_ref();
        self.run_with_bound_context(|| unsafe {
            let text_width = self.calc_text_size_bound(text, false, -1.0)[0];
            sys::igPushItemWidth(text_width);
        });
        ItemWidthStackToken::new(self)
    }

    /// Sets the position where text will wrap around.
    ///
    /// Returns a `TextWrapPosStackToken`. The pushed wrap position is popped when either
    /// `TextWrapPosStackToken` goes out of scope, or `.end()` is called.
    ///
    /// - `wrap_pos_x < 0.0`: no wrapping
    /// - `wrap_pos_x = 0.0`: wrap to end of window (or column)
    /// - `wrap_pos_x > 0.0`: wrap at `wrap_pos_x` position in window local space
    #[doc(alias = "PushTextWrapPos")]
    pub fn push_text_wrap_pos(&self, wrap_pos_x: f32) -> TextWrapPosStackToken<'_> {
        self.run_with_bound_context(|| unsafe { sys::igPushTextWrapPos(wrap_pos_x) });
        TextWrapPosStackToken::new(self)
    }

    /// Changes a style color by pushing a change to the color stack.
    ///
    /// Returns a `ColorStackToken` that must be popped by calling `.pop()`
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
    /// let color = ui.push_style_color(StyleColor::Text, RED);
    /// ui.text("I'm red!");
    /// color.pop();
    /// ```
    #[doc(alias = "PushStyleColor")]
    pub fn push_style_color(
        &self,
        style_color: StyleColor,
        color: impl Into<[f32; 4]>,
    ) -> ColorStackToken<'_> {
        let color_array = color.into();
        validate_style_color("Ui::push_style_color()", "color", color_array);
        self.run_with_bound_context(|| unsafe {
            sys::igPushStyleColor_Vec4(
                style_color as i32,
                sys::ImVec4 {
                    x: color_array[0],
                    y: color_array[1],
                    z: color_array[2],
                    w: color_array[3],
                },
            )
        });
        ColorStackToken::new(self)
    }

    /// Changes a style variable by pushing a change to the style stack.
    ///
    /// Returns a `StyleStackToken` that can be popped by calling `.end()`
    /// or by allowing to drop.
    ///
    /// `StyleVar::Alpha` participates in the same restoration order as an effective disabled
    /// scope. Keep those two scopes lexically nested, or use a closure helper, so the saved alpha
    /// value is restored by the operation that owns it.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let style = ui.push_style_var(StyleVar::Alpha(0.2));
    /// ui.text("I'm transparent!");
    /// style.pop();
    /// ```
    #[doc(alias = "PushStyleVar")]
    pub fn push_style_var(&self, style_var: StyleVar) -> StyleStackToken<'_> {
        validate_style_var("Ui::push_style_var()", style_var);
        let is_alpha = matches!(style_var, StyleVar::Alpha(_));
        self.run_with_bound_context(|| unsafe { push_style_var(style_var) });
        StyleStackToken::new_for(self, is_alpha)
    }

    /// Overrides the X component of a two-component style variable.
    #[doc(alias = "PushStyleVarX")]
    pub fn push_style_var_x(&self, style_var: StyleVarVec2, value: f32) -> StyleStackToken<'_> {
        validate_style_var_component("Ui::push_style_var_x()", style_var, value);
        self.run_with_bound_context(|| unsafe { sys::igPushStyleVarX(style_var.raw(), value) });
        StyleStackToken::new(self)
    }

    /// Overrides the Y component of a two-component style variable.
    #[doc(alias = "PushStyleVarY")]
    pub fn push_style_var_y(&self, style_var: StyleVarVec2, value: f32) -> StyleStackToken<'_> {
        validate_style_var_component("Ui::push_style_var_y()", style_var, value);
        self.run_with_bound_context(|| unsafe { sys::igPushStyleVarY(style_var.raw(), value) });
        StyleStackToken::new(self)
    }
}

#[cfg(test)]
mod font_tests {
    const ROBOTO_MEDIUM: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/Roboto-Medium.ttf"
    ));

    #[test]
    fn push_font_uses_the_size_supplied_when_the_font_was_added() {
        let mut ctx = crate::Context::create();
        let small = ctx
            .font_atlas()
            .add_font(&[crate::FontSource::default_font_with_size(13.0)]);
        let large = ctx
            .font_atlas()
            .add_font(&[crate::FontSource::default_font_with_size(29.0)]);
        assert_eq!(small.reference_size(), Some(13.0));
        assert_eq!(large.reference_size(), Some(29.0));
        ctx.font_atlas()
            .try_claim_legacy_renderer()
            .expect("legacy renderer font atlas should be available")
            .build();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);

        let ui = ctx.frame();
        assert_eq!(ui.current_font(), small);
        assert_eq!(ui.current_font_size(), 13.0);

        {
            let _font = ui.push_font(large);
            assert_eq!(ui.current_font(), large);
            assert_eq!(ui.current_font_size(), 29.0);
        }

        assert_eq!(ui.current_font(), small);
        assert_eq!(ui.current_font_size(), 13.0);
    }

    #[test]
    fn push_font_preserves_current_size_without_a_reference_size() {
        let mut ctx = crate::Context::create();
        let _consumer = ctx
            .create_synchronous_renderer_consumer()
            .expect("the managed renderer consumer should attach");
        let small = ctx
            .font_atlas()
            .add_font(&[crate::FontSource::default_font_with_size(13.0)]);
        // SAFETY: the vendored bytes contain the complete, unmodified Roboto Medium TTF.
        let dynamic = ctx
            .font_atlas()
            .add_font(&[unsafe { crate::FontSource::ttf_data(ROBOTO_MEDIUM) }]);
        assert_eq!(dynamic.reference_size(), None);
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        ctx.io_mut()
            .set_backend_flags(crate::BackendFlags::RENDERER_HAS_TEXTURES);

        let ui = ctx.frame();
        assert_eq!(ui.current_font(), small);
        assert_eq!(ui.current_font_size(), 13.0);

        let _font = ui.push_font(dynamic);
        assert_eq!(ui.current_font(), dynamic);
        assert_eq!(ui.current_font_size(), 13.0);
    }
}

#[cfg(test)]
mod item_tests {
    use super::*;

    fn setup_context() -> crate::Context {
        let mut ctx = crate::Context::create();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        ctx.font_atlas()
            .try_claim_legacy_renderer()
            .expect("legacy renderer font atlas should be available")
            .build();
        ctx
    }

    #[test]
    fn item_flag_scope_is_typed_and_restores_previous_flags() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("item_flags").build(|| {
            ui.with_item_flag(
                ItemFlags::NO_NAV
                    | ItemFlags::ALLOW_DUPLICATE_ID
                    | ItemFlags::LIVE_EDIT_ON_INPUT_SCALAR
                    | ItemFlags::MIXED_VALUE,
                true,
                || {
                    ui.button("scoped");
                    let flags = ui.item_flags();
                    assert!(flags.contains(ItemStateFlags::NO_NAV));
                    assert!(flags.contains(ItemStateFlags::ALLOW_DUPLICATE_ID));
                    assert!(flags.contains(ItemStateFlags::LIVE_EDIT_ON_INPUT_SCALAR));
                    assert!(flags.contains(ItemStateFlags::MIXED_VALUE));
                },
            );

            ui.button("restored");
            let flags = ui.item_flags();
            assert!(!flags.contains(ItemStateFlags::NO_NAV));
            assert!(!flags.contains(ItemStateFlags::ALLOW_DUPLICATE_ID));
            assert!(!flags.contains(ItemStateFlags::LIVE_EDIT_ON_INPUT_SCALAR));
            assert!(!flags.contains(ItemStateFlags::MIXED_VALUE));
        });
    }

    #[test]
    fn item_flag_scope_can_clear_defaults_and_reports_disabled_items() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("item_flag_values").build(|| {
            ui.button("default");
            assert!(ui.item_flags().contains(ItemStateFlags::AUTO_CLOSE_POPUPS));

            {
                let _flags = ui.push_item_flag(ItemFlags::AUTO_CLOSE_POPUPS, false);
                ui.button("without_auto_close");
                assert!(!ui.item_flags().contains(ItemStateFlags::AUTO_CLOSE_POPUPS));
            }

            ui.button("restored_default");
            assert!(ui.item_flags().contains(ItemStateFlags::AUTO_CLOSE_POPUPS));

            {
                let _disabled = ui.begin_disabled();
                ui.button("disabled");
                assert!(ui.item_flags().contains(ItemStateFlags::DISABLED));
            }
        });
    }

    #[test]
    fn disabled_and_item_flag_tokens_share_one_strict_native_stack() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("item_flag_ordering").build(|| {
            let flag = ui.push_item_flag(ItemFlags::NO_NAV, true);
            let disabled = ui.begin_disabled();

            assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(flag))).is_err());
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ui.button("blocked while recovery is pending");
                }))
                .is_err()
            );

            drop(disabled);
            ui.button("recovered");
            assert!(!ui.item_flags().contains(ItemStateFlags::NO_NAV));
            assert!(!ui.item_flags().contains(ItemStateFlags::DISABLED));
        });
    }

    #[test]
    fn closure_scope_drops_its_return_value_before_recovering_an_outer_token() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("item_flag_closure_ordering").build(|| {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.with_item_flag(ItemFlags::NO_NAV, true, || {
                    ui.push_item_flag(ItemFlags::BUTTON_REPEAT, true)
                });
            }));
            assert!(result.is_err());

            ui.button("recovered after closure result cleanup");
            let flags = ui.item_flags();
            assert!(!flags.contains(ItemStateFlags::NO_NAV));
            assert!(!flags.contains(ItemStateFlags::BUTTON_REPEAT));
        });
    }
}

#[cfg(test)]
mod layout_tests {
    fn setup_context() -> crate::Context {
        let mut ctx = crate::Context::create();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        ctx.font_atlas()
            .try_claim_legacy_renderer()
            .expect("legacy renderer font atlas should be available")
            .build();
        ctx
    }

    #[test]
    fn text_item_width_scope_matches_measurement_and_restores_previous_width() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("text_item_width").build(|| {
            let original_width = ui.calc_item_width();
            let text_width = ui.calc_text_size("measured width")[0];

            {
                let _width = ui.push_item_width_text("measured width");
                assert_eq!(ui.calc_item_width(), text_width);
            }

            assert_eq!(ui.calc_item_width(), original_width);
        });
    }
}

#[cfg(test)]
mod style_tests {
    use super::*;

    fn setup_context() -> crate::Context {
        let mut ctx = crate::Context::create();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        ctx.font_atlas()
            .try_claim_legacy_renderer()
            .expect("legacy renderer font atlas should be available")
            .build();
        ctx
    }

    #[test]
    fn style_var_component_scopes_are_typed_and_restore_values() {
        let mut ctx = setup_context();
        let ui = ctx.frame();
        let original = ui.clone_style().window_padding();

        {
            let _x = ui.push_style_var_x(StyleVarVec2::WindowPadding, 17.0);
            assert_eq!(ui.clone_style().window_padding(), [17.0, original[1]]);
        }
        assert_eq!(ui.clone_style().window_padding(), original);

        {
            let _y = ui.push_style_var_y(StyleVarVec2::WindowPadding, 23.0);
            assert_eq!(ui.clone_style().window_padding(), [original[0], 23.0]);
        }
        assert_eq!(ui.clone_style().window_padding(), original);
    }

    #[test]
    fn style_var_components_validate_values_before_ffi() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        for value in [-1.0, f32::NAN, f32::INFINITY] {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _token = ui.push_style_var_x(StyleVarVec2::WindowPadding, value);
                }))
                .is_err()
            );
        }
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _token = ui.push_style_var_y(StyleVarVec2::WindowTitleAlign, 1.1);
            }))
            .is_err()
        );
    }
}
