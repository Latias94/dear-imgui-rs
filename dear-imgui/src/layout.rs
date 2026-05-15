//! Layout and cursor helpers
//!
//! Spacing, separators, horizontal layout (`same_line`), grouping, cursor
//! positioning and clipping helpers. These functions help arrange widgets and
//! content within windows.
//!
//! Example:
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! ui.text("Left");
//! ui.same_line();
//! ui.text("Right");
//! ```
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions,
    clippy::unnecessary_cast
)]
use crate::Id;
use crate::Ui;
use crate::sys;
use std::ffi::c_void;

fn assert_finite_f32(caller: &str, name: &str, value: f32) {
    assert!(value.is_finite(), "{caller} {name} must be finite");
}

fn assert_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert!(
        value[0].is_finite() && value[1].is_finite(),
        "{caller} {name} must contain finite values"
    );
}

create_token!(
    /// Tracks a layout group that can be ended with `end` or by dropping.
    pub struct GroupToken<'ui>;

    /// Drops the layout group manually. You can also just allow this token
    /// to drop on its own.
    drop { unsafe { sys::igEndGroup() } }
);

create_token!(
    /// Tracks a pushed clip rect that will be popped on drop.
    pub struct ClipRectToken<'ui>;

    /// Pops a clip rect pushed with [`Ui::push_clip_rect`].
    drop { unsafe { sys::igPopClipRect() } }
);

create_token!(
    /// Tracks a stack-layout horizontal group.
    ///
    /// This wraps the repository-owned stack layout compatibility shim used by
    /// the upstream imgui-node-editor blueprints example. It is not part of the
    /// official Dear ImGui API.
    pub struct HorizontalStackLayoutToken<'ui>;

    /// Ends the stack-layout horizontal group.
    drop { unsafe { sys::ImGuiStack_EndHorizontal() } }
);

create_token!(
    /// Tracks a stack-layout vertical group.
    ///
    /// This wraps the repository-owned stack layout compatibility shim used by
    /// the upstream imgui-node-editor blueprints example. It is not part of the
    /// official Dear ImGui API.
    pub struct VerticalStackLayoutToken<'ui>;

    /// Ends the stack-layout vertical group.
    drop { unsafe { sys::ImGuiStack_EndVertical() } }
);

create_token!(
    /// Tracks a suspended stack layout and resumes it on drop.
    pub struct StackLayoutSuspensionToken<'ui>;

    /// Resumes a suspended stack layout.
    drop { unsafe { sys::ImGuiStack_ResumeLayout() } }
);

/// Identifier accepted by the stack layout compatibility helpers.
///
/// The pointer form mirrors the upstream `BeginHorizontal(id.AsPointer())`
/// usage from `imgui-node-editor`; the pointer is used only as an ID and is not
/// dereferenced.
#[derive(Clone, Copy, Debug)]
pub enum StackLayoutId<'a> {
    Str(&'a str),
    Ptr(*const c_void),
    Int(i32),
    Raw(Id),
}

impl<'a> StackLayoutId<'a> {
    /// Construct an ID from a pointer value.
    #[inline]
    pub const fn ptr(ptr: *const c_void) -> Self {
        Self::Ptr(ptr)
    }

    /// Construct an ID from a pointer-sized integer, matching upstream
    /// `NodeId::AsPointer()` / `PinId::AsPointer()` usage.
    #[inline]
    pub const fn pointer_value(value: usize) -> Self {
        Self::Ptr(value as *const c_void)
    }
}

impl<'a> From<&'a str> for StackLayoutId<'a> {
    #[inline]
    fn from(value: &'a str) -> Self {
        Self::Str(value)
    }
}

impl From<i32> for StackLayoutId<'_> {
    #[inline]
    fn from(value: i32) -> Self {
        Self::Int(value)
    }
}

impl From<Id> for StackLayoutId<'_> {
    #[inline]
    fn from(value: Id) -> Self {
        Self::Raw(value)
    }
}

/// # Cursor / Layout
impl Ui {
    /// Renders a separator (generally horizontal).
    ///
    /// This becomes a vertical separator inside a menu bar or in horizontal layout mode.
    #[doc(alias = "Separator")]
    pub fn separator(&self) {
        unsafe { sys::igSeparator() }
    }

    /// Renders a separator with text.
    #[doc(alias = "SeparatorText")]
    pub fn separator_with_text(&self, text: impl AsRef<str>) {
        unsafe { sys::igSeparatorText(self.scratch_txt(text)) }
    }

    /// Creates a vertical separator
    #[doc(alias = "SeparatorEx")]
    pub fn separator_vertical(&self) {
        unsafe { sys::igSeparatorEx(sys::ImGuiSeparatorFlags_Vertical as i32, 1.0) }
    }

    /// Creates a horizontal separator
    #[doc(alias = "SeparatorEx")]
    pub fn separator_horizontal(&self) {
        unsafe { sys::igSeparatorEx(sys::ImGuiSeparatorFlags_Horizontal as i32, 1.0) }
    }

    /// Call between widgets or groups to layout them horizontally.
    ///
    /// X position is given in window coordinates.
    ///
    /// This is equivalent to calling [same_line_with_pos](Self::same_line_with_pos)
    /// with the `pos` set to 0.0, which uses `Style::item_spacing`.
    #[doc(alias = "SameLine")]
    pub fn same_line(&self) {
        self.same_line_with_pos(0.0);
    }

    /// Call between widgets or groups to layout them horizontally.
    ///
    /// X position is given in window coordinates.
    ///
    /// This is equivalent to calling [same_line_with_spacing](Self::same_line_with_spacing)
    /// with the `spacing` set to -1.0, which means no extra spacing.
    #[doc(alias = "SameLine")]
    pub fn same_line_with_pos(&self, pos_x: f32) {
        self.same_line_with_spacing(pos_x, -1.0)
    }

    /// Call between widgets or groups to layout them horizontally.
    ///
    /// X position is given in window coordinates.
    #[doc(alias = "SameLine")]
    pub fn same_line_with_spacing(&self, pos_x: f32, spacing_w: f32) {
        assert_finite_f32("Ui::same_line_with_spacing()", "pos_x", pos_x);
        assert_finite_f32("Ui::same_line_with_spacing()", "spacing_w", spacing_w);
        unsafe { sys::igSameLine(pos_x, spacing_w) }
    }

    /// Undo a `same_line` call or force a new line when in horizontal layout mode
    #[doc(alias = "NewLine")]
    pub fn new_line(&self) {
        unsafe { sys::igNewLine() }
    }

    /// Adds vertical spacing
    #[doc(alias = "Spacing")]
    pub fn spacing(&self) {
        unsafe { sys::igSpacing() }
    }

    /// Fills a space of `size` in pixels with nothing on the current window.
    ///
    /// Can be used to move the cursor on the window.
    #[doc(alias = "Dummy")]
    pub fn dummy(&self, size: impl Into<[f32; 2]>) {
        let size = size.into();
        assert_finite_vec2("Ui::dummy()", "size", size);
        let size_vec: sys::ImVec2 = size.into();
        unsafe { sys::igDummy(size_vec) }
    }

    /// Moves content position to the right by `Style::indent_spacing`
    ///
    /// This is equivalent to [indent_by](Self::indent_by) with `width` set to
    /// `Style::indent_spacing`.
    #[doc(alias = "Indent")]
    pub fn indent(&self) {
        self.indent_by(0.0)
    }

    /// Moves content position to the right by `width`
    #[doc(alias = "Indent")]
    pub fn indent_by(&self, width: f32) {
        assert_finite_f32("Ui::indent_by()", "width", width);
        unsafe { sys::igIndent(width) };
    }

    /// Moves content position to the left by `Style::indent_spacing`
    ///
    /// This is equivalent to [unindent_by](Self::unindent_by) with `width` set to
    /// `Style::indent_spacing`.
    #[doc(alias = "Unindent")]
    pub fn unindent(&self) {
        self.unindent_by(0.0)
    }

    /// Moves content position to the left by `width`
    #[doc(alias = "Unindent")]
    pub fn unindent_by(&self, width: f32) {
        assert_finite_f32("Ui::unindent_by()", "width", width);
        unsafe { sys::igUnindent(width) };
    }

    /// Creates a layout group and starts appending to it.
    ///
    /// Returns a `GroupToken` that must be ended by calling `.end()`.
    #[doc(alias = "BeginGroup")]
    pub fn begin_group(&self) -> GroupToken<'_> {
        unsafe { sys::igBeginGroup() };
        GroupToken::new(self)
    }

    /// Creates a layout group and runs a closure to construct the contents.
    ///
    /// May be useful to handle the same mouse event on a group of items, for example.
    #[doc(alias = "BeginGroup")]
    pub fn group<R, F: FnOnce() -> R>(&self, f: F) -> R {
        let group = self.begin_group();
        let result = f();
        group.end();
        result
    }

    /// Starts a stack-layout horizontal group.
    ///
    /// This is a compatibility helper for examples and utilities that follow
    /// `imgui-node-editor`'s blueprint builder. It is backed by a local shim,
    /// because Dear ImGui itself does not ship `BeginHorizontal`.
    #[doc(alias = "BeginHorizontal")]
    pub fn begin_horizontal_stack_layout<'ui, 'id>(
        &'ui self,
        id: impl Into<StackLayoutId<'id>>,
        size: impl Into<[f32; 2]>,
        align: f32,
    ) -> HorizontalStackLayoutToken<'ui> {
        let size = size.into();
        assert_finite_vec2("Ui::begin_horizontal_stack_layout()", "size", size);
        assert!(
            align.is_finite(),
            "Ui::begin_horizontal_stack_layout() align must be finite"
        );
        let size = sys::ImVec2::from(size);
        unsafe {
            match id.into() {
                StackLayoutId::Str(value) => {
                    sys::ImGuiStack_BeginHorizontal_Str(self.scratch_txt(value), size, align);
                }
                StackLayoutId::Ptr(value) => {
                    sys::ImGuiStack_BeginHorizontal_Ptr(value, size, align);
                }
                StackLayoutId::Int(value) => {
                    sys::ImGuiStack_BeginHorizontal_Int(value, size, align);
                }
                StackLayoutId::Raw(value) => {
                    sys::ImGuiStack_BeginHorizontal_Id(value.raw(), size, align);
                }
            }
        }
        HorizontalStackLayoutToken::new(self)
    }

    /// Starts a stack-layout horizontal group.
    ///
    /// Alias for [`Self::begin_horizontal_stack_layout`] with the upstream
    /// naming used by imgui-node-editor examples.
    #[doc(alias = "BeginHorizontal")]
    pub fn begin_horizontal<'ui, 'id>(
        &'ui self,
        id: impl Into<StackLayoutId<'id>>,
        size: impl Into<[f32; 2]>,
        align: f32,
    ) -> HorizontalStackLayoutToken<'ui> {
        self.begin_horizontal_stack_layout(id, size, align)
    }

    /// Runs a closure inside a stack-layout horizontal group.
    #[doc(alias = "BeginHorizontal", alias = "EndHorizontal")]
    pub fn horizontal_stack_layout<'id, R>(
        &self,
        id: impl Into<StackLayoutId<'id>>,
        size: impl Into<[f32; 2]>,
        align: f32,
        f: impl FnOnce() -> R,
    ) -> R {
        let token = self.begin_horizontal_stack_layout(id, size, align);
        let result = f();
        token.end();
        result
    }

    /// Runs a closure inside a stack-layout horizontal group.
    ///
    /// Alias for [`Self::horizontal_stack_layout`].
    #[doc(alias = "BeginHorizontal", alias = "EndHorizontal")]
    pub fn horizontal<'id, R>(
        &self,
        id: impl Into<StackLayoutId<'id>>,
        size: impl Into<[f32; 2]>,
        align: f32,
        f: impl FnOnce() -> R,
    ) -> R {
        self.horizontal_stack_layout(id, size, align, f)
    }

    /// Starts a stack-layout vertical group.
    ///
    /// This is a compatibility helper for examples and utilities that follow
    /// `imgui-node-editor`'s blueprint builder. It is backed by a local shim,
    /// because Dear ImGui itself does not ship `BeginVertical`.
    #[doc(alias = "BeginVertical")]
    pub fn begin_vertical_stack_layout<'ui, 'id>(
        &'ui self,
        id: impl Into<StackLayoutId<'id>>,
        size: impl Into<[f32; 2]>,
        align: f32,
    ) -> VerticalStackLayoutToken<'ui> {
        let size = size.into();
        assert_finite_vec2("Ui::begin_vertical_stack_layout()", "size", size);
        assert!(
            align.is_finite(),
            "Ui::begin_vertical_stack_layout() align must be finite"
        );
        let size = sys::ImVec2::from(size);
        unsafe {
            match id.into() {
                StackLayoutId::Str(value) => {
                    sys::ImGuiStack_BeginVertical_Str(self.scratch_txt(value), size, align);
                }
                StackLayoutId::Ptr(value) => {
                    sys::ImGuiStack_BeginVertical_Ptr(value, size, align);
                }
                StackLayoutId::Int(value) => {
                    sys::ImGuiStack_BeginVertical_Int(value, size, align);
                }
                StackLayoutId::Raw(value) => {
                    sys::ImGuiStack_BeginVertical_Id(value.raw(), size, align);
                }
            }
        }
        VerticalStackLayoutToken::new(self)
    }

    /// Starts a stack-layout vertical group.
    ///
    /// Alias for [`Self::begin_vertical_stack_layout`] with the upstream naming
    /// used by imgui-node-editor examples.
    #[doc(alias = "BeginVertical")]
    pub fn begin_vertical<'ui, 'id>(
        &'ui self,
        id: impl Into<StackLayoutId<'id>>,
        size: impl Into<[f32; 2]>,
        align: f32,
    ) -> VerticalStackLayoutToken<'ui> {
        self.begin_vertical_stack_layout(id, size, align)
    }

    /// Runs a closure inside a stack-layout vertical group.
    #[doc(alias = "BeginVertical", alias = "EndVertical")]
    pub fn vertical_stack_layout<'id, R>(
        &self,
        id: impl Into<StackLayoutId<'id>>,
        size: impl Into<[f32; 2]>,
        align: f32,
        f: impl FnOnce() -> R,
    ) -> R {
        let token = self.begin_vertical_stack_layout(id, size, align);
        let result = f();
        token.end();
        result
    }

    /// Runs a closure inside a stack-layout vertical group.
    ///
    /// Alias for [`Self::vertical_stack_layout`].
    #[doc(alias = "BeginVertical", alias = "EndVertical")]
    pub fn vertical<'id, R>(
        &self,
        id: impl Into<StackLayoutId<'id>>,
        size: impl Into<[f32; 2]>,
        align: f32,
        f: impl FnOnce() -> R,
    ) -> R {
        self.vertical_stack_layout(id, size, align, f)
    }

    /// Inserts a spring into the current stack layout.
    ///
    /// `weight <= 0.0` reserves only spacing. `spacing < 0.0` uses the current
    /// style item spacing along the layout axis, matching the upstream stack
    /// layout extension semantics.
    #[doc(alias = "Spring")]
    pub fn stack_layout_spring(&self, weight: f32, spacing: f32) {
        assert!(
            weight.is_finite(),
            "Ui::stack_layout_spring() weight must be finite"
        );
        assert!(
            spacing.is_finite(),
            "Ui::stack_layout_spring() spacing must be finite"
        );
        unsafe { sys::ImGuiStack_Spring(weight, spacing) }
    }

    /// Inserts a spring into the current stack layout.
    ///
    /// Alias for [`Self::stack_layout_spring`] with the upstream naming used by
    /// imgui-node-editor examples.
    #[doc(alias = "Spring")]
    pub fn spring(&self, weight: f32, spacing: f32) {
        self.stack_layout_spring(weight, spacing)
    }

    /// Suspends the current stack layout until the returned token is dropped.
    #[doc(alias = "SuspendLayout")]
    pub fn suspend_stack_layout(&self) -> StackLayoutSuspensionToken<'_> {
        unsafe { sys::ImGuiStack_SuspendLayout() };
        StackLayoutSuspensionToken::new(self)
    }

    /// Returns the cursor position (in window coordinates)
    #[doc(alias = "GetCursorPos")]
    pub fn cursor_pos(&self) -> [f32; 2] {
        let pos = unsafe { sys::igGetCursorPos() };
        [pos.x, pos.y]
    }

    /// Returns the cursor position (in absolute screen coordinates)
    #[doc(alias = "GetCursorScreenPos")]
    pub fn cursor_screen_pos(&self) -> [f32; 2] {
        let pos = unsafe { sys::igGetCursorScreenPos() };
        [pos.x, pos.y]
    }

    /// Sets the cursor position (in window coordinates)
    #[doc(alias = "SetCursorPos")]
    pub fn set_cursor_pos(&self, pos: impl Into<[f32; 2]>) {
        let pos_array = pos.into();
        assert_finite_vec2("Ui::set_cursor_pos()", "position", pos_array);
        let pos_vec = sys::ImVec2 {
            x: pos_array[0],
            y: pos_array[1],
        };
        unsafe { sys::igSetCursorPos(pos_vec) };
    }

    /// Sets the cursor position (in absolute screen coordinates)
    #[doc(alias = "SetCursorScreenPos")]
    pub fn set_cursor_screen_pos(&self, pos: impl Into<[f32; 2]>) {
        let pos_array = pos.into();
        assert_finite_vec2("Ui::set_cursor_screen_pos()", "position", pos_array);
        let pos_vec = sys::ImVec2 {
            x: pos_array[0],
            y: pos_array[1],
        };
        unsafe { sys::igSetCursorScreenPos(pos_vec) };
    }

    /// Returns the X cursor position (in window coordinates)
    #[doc(alias = "GetCursorPosX")]
    pub fn cursor_pos_x(&self) -> f32 {
        unsafe { sys::igGetCursorPosX() }
    }

    /// Returns the Y cursor position (in window coordinates)
    #[doc(alias = "GetCursorPosY")]
    pub fn cursor_pos_y(&self) -> f32 {
        unsafe { sys::igGetCursorPosY() }
    }

    /// Sets the X cursor position (in window coordinates)
    #[doc(alias = "SetCursorPosX")]
    pub fn set_cursor_pos_x(&self, x: f32) {
        assert_finite_f32("Ui::set_cursor_pos_x()", "x", x);
        unsafe { sys::igSetCursorPosX(x) };
    }

    /// Sets the Y cursor position (in window coordinates)
    #[doc(alias = "SetCursorPosY")]
    pub fn set_cursor_pos_y(&self, y: f32) {
        assert_finite_f32("Ui::set_cursor_pos_y()", "y", y);
        unsafe { sys::igSetCursorPosY(y) };
    }

    /// Returns the initial cursor position (in window coordinates)
    #[doc(alias = "GetCursorStartPos")]
    pub fn cursor_start_pos(&self) -> [f32; 2] {
        let pos = unsafe { sys::igGetCursorStartPos() };
        [pos.x, pos.y]
    }
}

// ============================================================================
// Metrics helpers & clip rect stack
// ============================================================================

impl Ui {
    /// Return ~ FontSize.
    #[doc(alias = "GetTextLineHeight")]
    pub fn text_line_height(&self) -> f32 {
        unsafe { sys::igGetTextLineHeight() }
    }

    /// Return ~ FontSize + style.ItemSpacing.y.
    #[doc(alias = "GetTextLineHeightWithSpacing")]
    pub fn text_line_height_with_spacing(&self) -> f32 {
        unsafe { sys::igGetTextLineHeightWithSpacing() }
    }

    /// Return ~ FontSize + style.FramePadding.y * 2.
    #[doc(alias = "GetFrameHeight")]
    pub fn frame_height(&self) -> f32 {
        unsafe { sys::igGetFrameHeight() }
    }

    /// Return ~ FontSize + style.FramePadding.y * 2 + style.ItemSpacing.y.
    #[doc(alias = "GetFrameHeightWithSpacing")]
    pub fn frame_height_with_spacing(&self) -> f32 {
        unsafe { sys::igGetFrameHeightWithSpacing() }
    }

    /// Push a clipping rectangle in screen space.
    #[doc(alias = "PushClipRect")]
    pub fn push_clip_rect(
        &self,
        min: impl Into<[f32; 2]>,
        max: impl Into<[f32; 2]>,
        intersect_with_current: bool,
    ) {
        let min = min.into();
        let max = max.into();
        assert_finite_vec2("Ui::push_clip_rect()", "min", min);
        assert_finite_vec2("Ui::push_clip_rect()", "max", max);
        let min_v = sys::ImVec2 {
            x: min[0],
            y: min[1],
        };
        let max_v = sys::ImVec2 {
            x: max[0],
            y: max[1],
        };
        unsafe { sys::igPushClipRect(min_v, max_v, intersect_with_current) }
    }

    /// Pop a clipping rectangle from the stack.
    #[doc(alias = "PopClipRect")]
    pub fn pop_clip_rect(&self) {
        unsafe { sys::igPopClipRect() }
    }

    /// Run a closure with a clip rect pushed and automatically popped.
    pub fn with_clip_rect<R>(
        &self,
        min: impl Into<[f32; 2]>,
        max: impl Into<[f32; 2]>,
        intersect_with_current: bool,
        f: impl FnOnce() -> R,
    ) -> R {
        self.push_clip_rect(min, max, intersect_with_current);
        let _t = ClipRectToken::new(self);
        f()
    }

    /// Returns true if the specified rectangle (min,max) is visible (not clipped).
    #[doc(alias = "IsRectVisible")]
    pub fn is_rect_visible_min_max(
        &self,
        rect_min: impl Into<[f32; 2]>,
        rect_max: impl Into<[f32; 2]>,
    ) -> bool {
        let mn = rect_min.into();
        let mx = rect_max.into();
        assert_finite_vec2("Ui::is_rect_visible_min_max()", "rect_min", mn);
        assert_finite_vec2("Ui::is_rect_visible_min_max()", "rect_max", mx);
        let mn_v = sys::ImVec2 { x: mn[0], y: mn[1] };
        let mx_v = sys::ImVec2 { x: mx[0], y: mx[1] };
        unsafe { sys::igIsRectVisible_Vec2(mn_v, mx_v) }
    }

    /// Returns true if a rectangle of given size at the current cursor pos is visible.
    #[doc(alias = "IsRectVisible")]
    pub fn is_rect_visible_with_size(&self, size: impl Into<[f32; 2]>) -> bool {
        let s = size.into();
        assert_finite_vec2("Ui::is_rect_visible_with_size()", "size", s);
        let v = sys::ImVec2 { x: s[0], y: s[1] };
        unsafe { sys::igIsRectVisible_Nil(v) }
    }

    /// Vertically align upcoming text baseline to FramePadding.y (align text to framed items).
    #[doc(alias = "AlignTextToFramePadding")]
    pub fn align_text_to_frame_padding(&self) {
        unsafe { sys::igAlignTextToFramePadding() }
    }
}
