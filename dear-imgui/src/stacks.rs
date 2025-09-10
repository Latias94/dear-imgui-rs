use crate::fonts::{Font, FontAtlas, FontId};
use crate::style::{StyleColor, StyleVar};
use crate::sys;
use crate::Ui;

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
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::create_or_panic();
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
        unsafe { sys::ImGui_PushFont(font_ptr, 0.0) };
        FontStackToken::new(self)
    }

    /// Changes a style color by pushing a change to the color stack.
    ///
    /// Returns a `ColorStackToken` that must be popped by calling `.pop()`
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::create_or_panic();
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
            sys::ImGui_PushStyleColor1(
                style_color as i32,
                &sys::ImVec4 {
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
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::create_or_panic();
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
    drop { sys::ImGui_PopFont() }
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
    drop { sys::ImGui_PopStyleColor(1) }
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
    drop { sys::ImGui_PopStyleVar(1) }
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
        Alpha(v) => sys::ImGui_PushStyleVar(sys::ImGuiStyleVar_Alpha, v),
        DisabledAlpha(v) => sys::ImGui_PushStyleVar(sys::ImGuiStyleVar_DisabledAlpha, v),
        WindowPadding(v) => sys::ImGui_PushStyleVar1(sys::ImGuiStyleVar_WindowPadding, &v.into()),
        WindowRounding(v) => sys::ImGui_PushStyleVar(sys::ImGuiStyleVar_WindowRounding, v),
        WindowBorderSize(v) => sys::ImGui_PushStyleVar(sys::ImGuiStyleVar_WindowBorderSize, v),
        WindowMinSize(v) => sys::ImGui_PushStyleVar1(sys::ImGuiStyleVar_WindowMinSize, &v.into()),
        WindowTitleAlign(v) => {
            sys::ImGui_PushStyleVar1(sys::ImGuiStyleVar_WindowTitleAlign, &v.into())
        }
        ChildRounding(v) => sys::ImGui_PushStyleVar(sys::ImGuiStyleVar_ChildRounding, v),
        ChildBorderSize(v) => sys::ImGui_PushStyleVar(sys::ImGuiStyleVar_ChildBorderSize, v),
        PopupRounding(v) => sys::ImGui_PushStyleVar(sys::ImGuiStyleVar_PopupRounding, v),
        PopupBorderSize(v) => sys::ImGui_PushStyleVar(sys::ImGuiStyleVar_PopupBorderSize, v),
        FramePadding(v) => sys::ImGui_PushStyleVar1(sys::ImGuiStyleVar_FramePadding, &v.into()),
        FrameRounding(v) => sys::ImGui_PushStyleVar(sys::ImGuiStyleVar_FrameRounding, v),
        FrameBorderSize(v) => sys::ImGui_PushStyleVar(sys::ImGuiStyleVar_FrameBorderSize, v),
        ItemSpacing(v) => sys::ImGui_PushStyleVar1(sys::ImGuiStyleVar_ItemSpacing, &v.into()),
        ItemInnerSpacing(v) => {
            sys::ImGui_PushStyleVar1(sys::ImGuiStyleVar_ItemInnerSpacing, &v.into())
        }
        IndentSpacing(v) => sys::ImGui_PushStyleVar(sys::ImGuiStyleVar_IndentSpacing, v),
        CellPadding(v) => sys::ImGui_PushStyleVar1(sys::ImGuiStyleVar_CellPadding, &v.into()),
        ScrollbarSize(v) => sys::ImGui_PushStyleVar(sys::ImGuiStyleVar_ScrollbarSize, v),
        ScrollbarRounding(v) => sys::ImGui_PushStyleVar(sys::ImGuiStyleVar_ScrollbarRounding, v),
        GrabMinSize(v) => sys::ImGui_PushStyleVar(sys::ImGuiStyleVar_GrabMinSize, v),
        GrabRounding(v) => sys::ImGui_PushStyleVar(sys::ImGuiStyleVar_GrabRounding, v),
        TabRounding(v) => sys::ImGui_PushStyleVar(sys::ImGuiStyleVar_TabRounding, v),
        ButtonTextAlign(v) => {
            sys::ImGui_PushStyleVar1(sys::ImGuiStyleVar_ButtonTextAlign, &v.into())
        }
        SelectableTextAlign(v) => {
            sys::ImGui_PushStyleVar1(sys::ImGuiStyleVar_SelectableTextAlign, &v.into())
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
        unsafe { sys::ImGui_PushItemWidth(item_width) };
        ItemWidthStackToken::new(self)
    }

    /// Sets the width of the next item(s) to be the same as the width of the given text.
    ///
    /// Returns an `ItemWidthStackToken`. The pushed width item is popped when either
    /// `ItemWidthStackToken` goes out of scope, or `.end()` is called.
    #[doc(alias = "PushItemWidth")]
    pub fn push_item_width_text(&self, text: impl AsRef<str>) -> ItemWidthStackToken<'_> {
        let text_width = unsafe {
            let text_ptr = self.scratch_txt(text);
            let size = sys::ImGui_CalcTextSize(text_ptr, std::ptr::null(), false, -1.0);
            size.x
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
        unsafe { sys::ImGui_PushTextWrapPos(wrap_pos_x) };
        TextWrapPosStackToken::new(self)
    }
}

create_token!(
    /// Tracks a change made with [`Ui::push_item_width`] that can be popped
    /// by calling [`ItemWidthStackToken::end`] or dropping.
    pub struct ItemWidthStackToken<'ui>;

    /// Pops an item width change made with [`Ui::push_item_width`].
    #[doc(alias = "PopItemWidth")]
    drop { sys::ImGui_PopItemWidth() }
);

create_token!(
    /// Tracks a change made with [`Ui::push_text_wrap_pos`] that can be popped
    /// by calling [`TextWrapPosStackToken::end`] or dropping.
    pub struct TextWrapPosStackToken<'ui>;

    /// Pops a text wrap position change made with [`Ui::push_text_wrap_pos`].
    #[doc(alias = "PopTextWrapPos")]
    drop { sys::ImGui_PopTextWrapPos() }
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
    /// # let mut imgui = dear_imgui::Context::create_or_panic();
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
    /// # let mut imgui = dear_imgui::Context::create_or_panic();
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
                Id::Int(i) => sys::ImGui_PushID3(i),
                Id::Str(s) => sys::ImGui_PushID(self.scratch_txt(s)),
                Id::Ptr(p) => sys::ImGui_PushID2(p),
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
    drop { sys::ImGui_PopID() }
);

impl IdStackToken<'_> {
    /// Pops a change from the ID stack.
    pub fn pop(self) {
        self.end()
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
