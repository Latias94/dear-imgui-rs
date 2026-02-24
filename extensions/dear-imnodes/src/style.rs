use crate::context::ImNodesScope;
use crate::sys;
use dear_imgui_rs::sys as imgui_sys;
use std::marker::PhantomData;

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

pub struct ColorToken<'a> {
    scope: ImNodesScope,
    _phantom: PhantomData<&'a crate::Context>,
}
impl<'a> ColorToken<'a> {
    pub fn pop(self) {}
}
impl Drop for ColorToken<'_> {
    fn drop(&mut self) {
        unsafe {
            self.scope.bind();
            sys::imnodes_PopColorStyle();
        }
    }
}

pub struct StyleVarToken<'a> {
    scope: ImNodesScope,
    _phantom: PhantomData<&'a crate::Context>,
}
impl<'a> StyleVarToken<'a> {
    pub fn pop(self) {}
}
impl Drop for StyleVarToken<'_> {
    fn drop(&mut self) {
        unsafe {
            self.scope.bind();
            sys::imnodes_PopStyleVar(1);
        }
    }
}

pub enum StyleVarValue {
    Float(f32),
    Vec2([f32; 2]),
}

pub struct AttributeFlagToken<'a> {
    scope: ImNodesScope,
    _phantom: PhantomData<&'a crate::Context>,
}
impl<'a> AttributeFlagToken<'a> {
    pub fn pop(self) {}
}
impl Drop for AttributeFlagToken<'_> {
    fn drop(&mut self) {
        unsafe {
            self.scope.bind();
            sys::imnodes_PopAttributeFlag();
        }
    }
}

/// Style helpers available from NodeEditor
impl<'ui> crate::NodeEditor<'ui> {
    pub fn push_attribute_flag(&self, flag: crate::AttributeFlags) -> AttributeFlagToken<'_> {
        self.bind();
        unsafe { sys::imnodes_PushAttributeFlag(flag.bits()) };
        AttributeFlagToken {
            scope: self.scope(),
            _phantom: PhantomData,
        }
    }

    pub fn push_color(&self, elem: ColorElement, color: [f32; 4]) -> ColorToken<'_> {
        self.bind();
        // Use Dear ImGui's helper for packing RGBA -> ABGR (u32)
        let col = unsafe {
            imgui_sys::igColorConvertFloat4ToU32(imgui_sys::ImVec4 {
                x: color[0],
                y: color[1],
                z: color[2],
                w: color[3],
            })
        };
        unsafe { sys::imnodes_PushColorStyle(elem as i32, col) };
        ColorToken {
            scope: self.scope(),
            _phantom: PhantomData,
        }
    }

    pub fn push_style_var(&self, var: crate::StyleVar, value: StyleVarValue) -> StyleVarToken<'_> {
        self.bind();
        match value {
            StyleVarValue::Float(v) => unsafe { sys::imnodes_PushStyleVar_Float(var as i32, v) },
            StyleVarValue::Vec2(v) => unsafe {
                sys::imnodes_PushStyleVar_Vec2(var as i32, sys::ImVec2_c { x: v[0], y: v[1] })
            },
        }
        StyleVarToken {
            scope: self.scope(),
            _phantom: PhantomData,
        }
    }

    pub fn push_style_var_f32(&self, var: i32, value: f32) -> StyleVarToken<'_> {
        let max = sys::ImNodesStyleVar_COUNT as i32;
        assert!(
            (0..max).contains(&var),
            "dear-imnodes: invalid style var index {var}"
        );
        self.bind();
        unsafe { sys::imnodes_PushStyleVar_Float(var, value) };
        StyleVarToken {
            scope: self.scope(),
            _phantom: PhantomData,
        }
    }

    pub fn push_style_var_vec2(&self, var: i32, value: [f32; 2]) -> StyleVarToken<'_> {
        let max = sys::ImNodesStyleVar_COUNT as i32;
        assert!(
            (0..max).contains(&var),
            "dear-imnodes: invalid style var index {var}"
        );
        self.bind();
        unsafe {
            sys::imnodes_PushStyleVar_Vec2(
                var,
                sys::ImVec2_c {
                    x: value[0],
                    y: value[1],
                },
            )
        };
        StyleVarToken {
            scope: self.scope(),
            _phantom: PhantomData,
        }
    }
}

/// Convert RGBA floats [0,1] to ImGui-packed ABGR (u32)
pub fn rgba_to_abgr_u32(rgba: [f32; 4]) -> u32 {
    unsafe {
        imgui_sys::igColorConvertFloat4ToU32(imgui_sys::ImVec4 {
            x: rgba[0],
            y: rgba[1],
            z: rgba[2],
            w: rgba[3],
        }) as u32
    }
}

/// Convert ImGui-packed ABGR (u32) to RGBA floats [0,1]
pub fn abgr_u32_to_rgba(col: u32) -> [f32; 4] {
    let out = unsafe { imgui_sys::igColorConvertU32ToFloat4(col) };
    [out.x, out.y, out.z, out.w]
}
