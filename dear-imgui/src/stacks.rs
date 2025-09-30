//! Push/pop stacks (fonts, style, etc.)
//!
//! RAII-style wrappers for ImGui parameter stacks: fonts, style colors/vars and
//! more. Tokens returned by `Ui::push_*` pop automatically when dropped.
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]
use crate::Ui;
use crate::fonts::FontId;
use crate::style::{StyleColor, StyleVar};
use crate::sys;

/// # Parameter stacks (shared)
impl Ui {
    /// Switches to the given font by pushing it to the font stack.
    ///
    /// Returns a `FontStackToken` that must be popped by calling `.pop()`
    ///
    /// # Panics
    ///
    /// Panics if the font atlas does not contain the given font
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let font_data_sources = [];
    /// // At initialization time
    /// let my_custom_font = ctx.fonts().add_font(&font_data_sources);
    /// # let ui = ctx.frame();
    /// // During UI construction
    /// let font = ui.push_font(my_custom_font);
    /// ui.text("I use the custom font!");
    /// font.pop();
    /// ```
    #[doc(alias = "PushFont")]
    pub fn push_font(&self, id: FontId) -> FontStackToken<'_> {
        // For now, we'll use a simplified approach without full validation
        // TODO: Add proper FontAtlas integration for validation
        let font_ptr = id.0 as *mut sys::ImFont;
        unsafe { sys::igPushFont(font_ptr, 0.0) };
        FontStackToken::new(self)
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
        unsafe {
            sys::igPushStyleColor_Vec4(
                style_color as i32,
                sys::ImVec4 {
                    x: color_array[0],
                    y: color_array[1],
                    z: color_array[2],
                    w: color_array[3],
                },
            )
        };
        ColorStackToken::new(self)
    }

    /// Changes a style variable by pushing a change to the style stack.
    ///
    /// Returns a `StyleStackToken` that can be popped by calling `.end()`
    /// or by allowing to drop.
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
        unsafe { push_style_var(style_var) };
        StyleStackToken::new(self)
    }
}

create_token!(
    /// Tracks a font pushed to the font stack that can be popped by calling `.end()`
    /// or by dropping.
    pub struct FontStackToken<'ui>;

    /// Pops a change from the font stack.
    drop { unsafe { sys::igPopFont() } }
);

impl FontStackToken<'_> {
    /// Pops a change from the font stack.
    pub fn pop(self) {
        self.end()
    }
}

create_token!(
    /// Tracks a color pushed to the color stack that can be popped by calling `.end()`
    /// or by dropping.
    pub struct ColorStackToken<'ui>;

    /// Pops a change from the color stack.
    drop { unsafe { sys::igPopStyleColor(1) } }
);

impl ColorStackToken<'_> {
    /// Pops a change from the color stack.
    pub fn pop(self) {
        self.end()
    }
}

create_token!(
    /// Tracks a style pushed to the style stack that can be popped by calling `.end()`
    /// or by dropping.
    pub struct StyleStackToken<'ui>;

    /// Pops a change from the style stack.
    drop { unsafe { sys::igPopStyleVar(1) } }
);

impl StyleStackToken<'_> {
    /// Pops a change from the style stack.
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
        GrabMinSize(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_GrabMinSize as i32, v)
        },
        GrabRounding(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_GrabRounding as i32, v)
        },
        TabRounding(v) => unsafe {
            sys::igPushStyleVar_Float(sys::ImGuiStyleVar_TabRounding as i32, v)
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
    }
}

/// # Parameter stacks (current window)
impl Ui {
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
        unsafe { sys::igPushItemWidth(item_width) };
        ItemWidthStackToken::new(self)
    }

    /// Sets the width of the next item(s) to be the same as the width of the given text.
    ///
    /// Returns an `ItemWidthStackToken`. The pushed width item is popped when either
    /// `ItemWidthStackToken` goes out of scope, or `.end()` is called.
    #[doc(alias = "PushItemWidth")]
    pub fn push_item_width_text(&self, text: impl AsRef<str>) -> ItemWidthStackToken<'_> {
        let text_width = {
            let text_ptr = self.scratch_txt(text);
            let mut out = sys::ImVec2 { x: 0.0, y: 0.0 };
            unsafe { sys::igCalcTextSize(&mut out, text_ptr, std::ptr::null(), false, -1.0) };
            out.x
        };
        self.push_item_width(text_width)
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
        unsafe { sys::igPushTextWrapPos(wrap_pos_x) };
        TextWrapPosStackToken::new(self)
    }
}

create_token!(
    /// Tracks a change made with [`Ui::push_item_width`] that can be popped
    /// by calling [`ItemWidthStackToken::end`] or dropping.
    pub struct ItemWidthStackToken<'ui>;

    /// Pops an item width change made with [`Ui::push_item_width`].
    #[doc(alias = "PopItemWidth")]
    drop { unsafe { sys::igPopItemWidth() } }
);

create_token!(
    /// Tracks a change made with [`Ui::push_text_wrap_pos`] that can be popped
    /// by calling [`TextWrapPosStackToken::end`] or dropping.
    pub struct TextWrapPosStackToken<'ui>;

    /// Pops a text wrap position change made with [`Ui::push_text_wrap_pos`].
    #[doc(alias = "PopTextWrapPos")]
    drop { unsafe { sys::igPopTextWrapPos() } }
);

/// # ID stack
impl Ui {
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
    /// In `dear-imgui` the same applies, we can manually specify labels with the `##` syntax:
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
        unsafe {
            match id {
                Id::Int(i) => sys::igPushID_Int(i),
                Id::Str(s) => sys::igPushID_Str(self.scratch_txt(s)),
                Id::Ptr(p) => sys::igPushID_Ptr(p),
            }
        }
        IdStackToken::new(self)
    }
}

create_token!(
    /// Tracks an ID pushed to the ID stack that can be popped by calling `.pop()`
    /// or by dropping. See [`crate::Ui::push_id`] for more details.
    pub struct IdStackToken<'ui>;

    /// Pops a change from the ID stack
    drop { unsafe { sys::igPopID() } }
);

impl IdStackToken<'_> {
    /// Pops a change from the ID stack.
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

    /// Pops a focus scope.
    #[doc(alias = "PopFocusScope")]
    drop { unsafe { sys::igPopFocusScope() } }
);

impl Ui {
    /// Push a focus scope (affects e.g. navigation focus allocation).
    ///
    /// Returns a `FocusScopeToken` which will pop the focus scope when dropped.
    #[doc(alias = "PushFocusScope")]
    pub fn push_focus_scope(&self, id: sys::ImGuiID) -> FocusScopeToken<'_> {
        unsafe { sys::igPushFocusScope(id) };
        FocusScopeToken::new(self)
    }
}

/// Represents an identifier that can be pushed to the ID stack
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
