use crate::context::ImNodesScope;
use crate::sys;
use std::cell::RefCell;

#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ColorElement {
    NodeBackground = sys::ImNodesCol_NodeBackground as u32,
    NodeBackgroundHovered = sys::ImNodesCol_NodeBackgroundHovered as u32,
    NodeBackgroundSelected = sys::ImNodesCol_NodeBackgroundSelected as u32,
    NodeOutline = sys::ImNodesCol_NodeOutline as u32,
    TitleBar = sys::ImNodesCol_TitleBar as u32,
    TitleBarHovered = sys::ImNodesCol_TitleBarHovered as u32,
    TitleBarSelected = sys::ImNodesCol_TitleBarSelected as u32,
    Link = sys::ImNodesCol_Link as u32,
    LinkHovered = sys::ImNodesCol_LinkHovered as u32,
    LinkSelected = sys::ImNodesCol_LinkSelected as u32,
    Pin = sys::ImNodesCol_Pin as u32,
    PinHovered = sys::ImNodesCol_PinHovered as u32,
    BoxSelector = sys::ImNodesCol_BoxSelector as u32,
    BoxSelectorOutline = sys::ImNodesCol_BoxSelectorOutline as u32,
    GridBackground = sys::ImNodesCol_GridBackground as u32,
    GridLine = sys::ImNodesCol_GridLine as u32,
    GridLinePrimary = sys::ImNodesCol_GridLinePrimary as u32,
    MiniMapBackground = sys::ImNodesCol_MiniMapBackground as u32,
    MiniMapBackgroundHovered = sys::ImNodesCol_MiniMapBackgroundHovered as u32,
    MiniMapOutline = sys::ImNodesCol_MiniMapOutline as u32,
    MiniMapOutlineHovered = sys::ImNodesCol_MiniMapOutlineHovered as u32,
    MiniMapNodeBackground = sys::ImNodesCol_MiniMapNodeBackground as u32,
    MiniMapNodeBackgroundHovered = sys::ImNodesCol_MiniMapNodeBackgroundHovered as u32,
    MiniMapNodeBackgroundSelected = sys::ImNodesCol_MiniMapNodeBackgroundSelected as u32,
    MiniMapNodeOutline = sys::ImNodesCol_MiniMapNodeOutline as u32,
    MiniMapLink = sys::ImNodesCol_MiniMapLink as u32,
    MiniMapLinkSelected = sys::ImNodesCol_MiniMapLinkSelected as u32,
    MiniMapCanvas = sys::ImNodesCol_MiniMapCanvas as u32,
    MiniMapCanvasOutline = sys::ImNodesCol_MiniMapCanvasOutline as u32,
}

impl ColorElement {
    /// Returns the ImNodes name of this color. Can be called without an editor or context.
    #[doc(alias = "GetStyleColorName")]
    #[doc(alias = "imnodes_GetStyleColorName")]
    pub fn name(self) -> &'static str {
        // SAFETY: Every variant maps to a valid ImNodesCol. ImNodes returns a
        // NUL-terminated string literal for each color, independent of context state.
        let name = unsafe {
            std::ffi::CStr::from_ptr(sys::imnodes_GetStyleColorName(self as sys::ImNodesCol))
        };
        name.to_str().expect("ImNodes color names are ASCII")
    }
}

// ImNodes keeps three independent stacks on the ImNodes context. A token can be dropped
// out of order or forgotten. Pop released entries from the top, then drain the rest before
// EndNodeEditor so temporary styles cannot carry over to another editor.
struct StyleStack<'a> {
    scope: &'a ImNodesScope<'a>,
    released: RefCell<Vec<bool>>,
    pop_native: fn(),
}

impl<'a> StyleStack<'a> {
    fn new(scope: &'a ImNodesScope<'a>, pop_native: fn()) -> Self {
        Self {
            scope,
            released: Default::default(),
            pop_native,
        }
    }

    fn push(&self, push_native: impl FnOnce()) -> StyleEntry<'_> {
        let mut released = self.released.borrow_mut();
        let index = released.len();
        self.scope.with_bound_context(push_native);
        released.push(false);
        StyleEntry { stack: self, index }
    }

    fn pop(&self, count: usize) {
        if count != 0 {
            self.scope.with_bound_context(|| {
                for _ in 0..count {
                    (self.pop_native)();
                }
            });
        }
    }
}

impl Drop for StyleStack<'_> {
    fn drop(&mut self) {
        let count = self.released.get_mut().len();
        self.pop(count);
    }
}

struct StyleEntry<'a> {
    stack: &'a StyleStack<'a>,
    index: usize,
}

impl Drop for StyleEntry<'_> {
    fn drop(&mut self) {
        let mut released = self.stack.released.borrow_mut();
        released[self.index] = true;
        let before = released.len();
        while released.last() == Some(&true) {
            released.pop();
        }
        self.stack.pop(before - released.len());
    }
}

pub(crate) struct StyleStacks<'a> {
    colors: StyleStack<'a>,
    vars: StyleStack<'a>,
    flags: StyleStack<'a>,
}

impl<'a> StyleStacks<'a> {
    pub(crate) fn new(scope: &'a ImNodesScope<'a>) -> Self {
        Self {
            colors: StyleStack::new(scope, || unsafe { sys::imnodes_PopColorStyle() }),
            vars: StyleStack::new(scope, || unsafe { sys::imnodes_PopStyleVar(1) }),
            flags: StyleStack::new(scope, || unsafe { sys::imnodes_PopAttributeFlag() }),
        }
    }
}

#[must_use = "dropping the token restores the previous color"]
pub struct ColorToken<'a>(StyleEntry<'a>);
impl ColorToken<'_> {
    pub fn pop(self) {
        drop(self.0);
    }
}

#[must_use = "dropping the token restores the previous style value"]
pub struct StyleVarToken<'a>(StyleEntry<'a>);
impl StyleVarToken<'_> {
    pub fn pop(self) {
        drop(self.0);
    }
}

pub enum StyleVarValue {
    Float(f32),
    Vec2([f32; 2]),
}

#[must_use = "dropping the token restores the previous attribute flags"]
pub struct AttributeFlagToken<'a>(StyleEntry<'a>);
impl AttributeFlagToken<'_> {
    pub fn pop(self) {
        drop(self.0);
    }
}

impl crate::NodeEditor<'_> {
    pub fn push_attribute_flag(&self, flag: crate::AttributeFlags) -> AttributeFlagToken<'_> {
        AttributeFlagToken(
            self.styles
                .flags
                .push(|| unsafe { sys::imnodes_PushAttributeFlag(flag.bits()) }),
        )
    }

    pub fn push_color(&self, elem: ColorElement, color: [f32; 4]) -> ColorToken<'_> {
        let col = rgba_to_abgr_u32(color);
        ColorToken(
            self.styles
                .colors
                .push(|| unsafe { sys::imnodes_PushColorStyle(elem as i32, col) }),
        )
    }

    /// Pushes a temporary style value. The padding and minimap offset variables take a vector.
    pub fn push_style_var(&self, var: crate::StyleVar, value: StyleVarValue) -> StyleVarToken<'_> {
        use crate::StyleVar;
        let takes_vec2 = matches!(
            var,
            StyleVar::NodePadding | StyleVar::MiniMapPadding | StyleVar::MiniMapOffset
        );
        assert_eq!(
            takes_vec2,
            matches!(value, StyleVarValue::Vec2(_)),
            "dear-imnodes: incorrect value type for {var:?}"
        );
        StyleVarToken(self.styles.vars.push(|| unsafe {
            match value {
                StyleVarValue::Float(value) => {
                    if var == StyleVar::GridSpacing {
                        validate_grid_spacing(value);
                    }
                    sys::imnodes_PushStyleVar_Float(var as i32, value);
                }
                StyleVarValue::Vec2([x, y]) => {
                    sys::imnodes_PushStyleVar_Vec2(var as i32, sys::ImVec2_c { x, y })
                }
            }
        }))
    }

    pub fn push_style_var_f32(&self, var: crate::StyleVar, value: f32) -> StyleVarToken<'_> {
        self.push_style_var(var, StyleVarValue::Float(value))
    }

    pub fn push_style_var_vec2(&self, var: crate::StyleVar, value: [f32; 2]) -> StyleVarToken<'_> {
        self.push_style_var(var, StyleVarValue::Vec2(value))
    }
}

pub(crate) fn validate_grid_spacing(spacing: f32) {
    assert!(
        spacing.is_finite() && spacing > 0.0,
        "dear-imnodes: grid spacing must be finite and positive"
    );
}

/// Convert RGBA floats [0,1] to ImGui-packed ABGR (u32)
pub fn rgba_to_abgr_u32(rgba: [f32; 4]) -> u32 {
    let [r, g, b, a] = rgba.map(|component| (component.clamp(0.0, 1.0) * 255.0 + 0.5) as u32);
    r | (g << 8) | (b << 16) | (a << 24)
}

/// Convert ImGui-packed ABGR (u32) to RGBA floats [0,1]
pub fn abgr_u32_to_rgba(col: u32) -> [f32; 4] {
    const SCALE: f32 = 1.0 / 255.0;
    [
        (col & 0xff) as f32 * SCALE,
        ((col >> 8) & 0xff) as f32 * SCALE,
        ((col >> 16) & 0xff) as f32 * SCALE,
        ((col >> 24) & 0xff) as f32 * SCALE,
    ]
}

#[cfg(test)]
mod tests {
    use super::{ColorElement, abgr_u32_to_rgba, rgba_to_abgr_u32};

    #[test]
    fn native_color_names_are_available_without_a_context() {
        let background: &'static str = ColorElement::NodeBackground.name();
        assert_eq!(ColorElement::GridLinePrimary.name(), "GridLinePrimary");
        assert_eq!(
            ColorElement::MiniMapCanvasOutline.name(),
            "MiniMapCanvasOutline"
        );
        assert_eq!(background, "NodeBackground");
    }

    #[test]
    fn color_conversion_matches_default_imgui_packing() {
        assert_eq!(rgba_to_abgr_u32([1.0, 0.5, 0.0, 1.0]), 0xff00_80ff);
        assert_eq!(rgba_to_abgr_u32([-1.0, 2.0, 0.0, 1.0]), 0xff00_ff00);

        let rgba = abgr_u32_to_rgba(0x8040_20ff);
        let expected = [1.0, 32.0 / 255.0, 64.0 / 255.0, 128.0 / 255.0];
        for (actual, expected) in rgba.into_iter().zip(expected) {
            assert!((actual - expected).abs() <= f32::EPSILON);
        }
    }
}
