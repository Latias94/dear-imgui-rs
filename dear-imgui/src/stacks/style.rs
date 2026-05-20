use crate::style::{StyleColor, StyleVar, validate_style_color, validate_style_var};
use crate::{Ui, sys};

impl Ui {
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
        validate_style_var("Ui::push_style_var()", style_var);
        unsafe { push_style_var(style_var) };
        StyleStackToken::new(self)
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
