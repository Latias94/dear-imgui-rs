use crate::sys;

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

pub struct ColorToken;
impl ColorToken {
    pub fn pop(self) {}
}
impl Drop for ColorToken {
    fn drop(&mut self) {
        unsafe { sys::imnodes_PopColorStyle() };
    }
}

pub struct StyleVarToken;
impl StyleVarToken {
    pub fn pop(self) {}
}
impl Drop for StyleVarToken {
    fn drop(&mut self) {
        unsafe { sys::imnodes_PopStyleVar(1) };
    }
}

pub enum StyleVarValue {
    Float(f32),
    Vec2([f32; 2]),
}

pub struct AttributeFlagToken;
impl AttributeFlagToken {
    pub fn pop(self) {}
}
impl Drop for AttributeFlagToken {
    fn drop(&mut self) {
        unsafe { sys::imnodes_PopAttributeFlag() };
    }
}

/// Style helpers available from NodeEditor
impl<'ui> crate::NodeEditor<'ui> {
    pub fn push_attribute_flag(&self, flag: crate::AttributeFlags) -> AttributeFlagToken {
        unsafe { sys::imnodes_PushAttributeFlag(flag.bits()) };
        AttributeFlagToken
    }

    pub fn push_color(&self, elem: ColorElement, color: [f32; 4]) -> ColorToken {
        // ImNodes expects packed ABGR u32 (same convention as ImGui ColorConvertFloat4ToU32)
        let rgba = color;
        let r = (rgba[0].clamp(0.0, 1.0) * 255.0) as u32;
        let g = (rgba[1].clamp(0.0, 1.0) * 255.0) as u32;
        let b = (rgba[2].clamp(0.0, 1.0) * 255.0) as u32;
        let a = (rgba[3].clamp(0.0, 1.0) * 255.0) as u32;
        let abgr = (a << 24) | (b << 16) | (g << 8) | r;
        unsafe { sys::imnodes_PushColorStyle(elem as i32, abgr) };
        ColorToken
    }

    pub fn push_style_var(&self, var: crate::StyleVar, value: StyleVarValue) -> StyleVarToken {
        match value {
            StyleVarValue::Float(v) => unsafe { sys::imnodes_PushStyleVar_Float(var as i32, v) },
            StyleVarValue::Vec2(v) => unsafe {
                sys::imnodes_PushStyleVar_Vec2(var as i32, sys::ImVec2 { x: v[0], y: v[1] })
            },
        }
        StyleVarToken
    }

    pub fn push_style_var_f32(&self, var: i32, value: f32) -> StyleVarToken {
        unsafe { sys::imnodes_PushStyleVar_Float(var, value) };
        StyleVarToken
    }

    pub fn push_style_var_vec2(&self, var: i32, value: [f32; 2]) -> StyleVarToken {
        unsafe {
            sys::imnodes_PushStyleVar_Vec2(
                var,
                sys::ImVec2 {
                    x: value[0],
                    y: value[1],
                },
            )
        };
        StyleVarToken
    }
}
