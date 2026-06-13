use super::super::NodeEditor;
use crate::sys;
use dear_imgui_rs::MouseButton;
use dear_imgui_rs::sys as imgui_sys;

impl<'ui> NodeEditor<'ui> {
    fn with_style<R>(&self, f: impl FnOnce(&mut sys::ImNodesStyle) -> R) -> R {
        let _guard = self.bind();
        let style = unsafe { sys::imnodes_GetStyle() };
        assert!(
            !style.is_null(),
            "dear-imnodes: imnodes_GetStyle returned null"
        );
        unsafe { f(&mut *style) }
    }

    fn with_io<R>(&self, f: impl FnOnce(&mut sys::ImNodesIO) -> R) -> R {
        let _guard = self.bind();
        let io = unsafe { sys::imnodes_GetIO() };
        assert!(!io.is_null(), "dear-imnodes: imnodes_GetIO returned null");
        unsafe { f(&mut *io) }
    }

    /// Toggle style flags (GridLines, GridLinesPrimary, GridSnapping, NodeOutline)
    pub fn set_style_flag(&self, flag: crate::StyleFlags, enabled: bool) {
        self.with_style(|style| {
            let mut f = style.Flags as i32;
            let bit = flag.bits();
            if enabled {
                f |= bit;
            } else {
                f &= !bit;
            }
            style.Flags = f;
        });
    }

    /// Enable link detach with Ctrl by binding to ImGui IO KeyCtrl
    pub fn enable_link_detach_with_ctrl(&self) {
        self.with_io(|io| {
            let imgui_io = unsafe { imgui_sys::igGetIO_Nil() };
            assert!(
                !imgui_io.is_null(),
                "dear-imnodes: ImGui IO must be available"
            );
            unsafe {
                io.LinkDetachWithModifierClick.Modifier = std::ptr::addr_of!((*imgui_io).KeyCtrl);
            }
        });
    }
    /// Enable multiple select modifier as Ctrl
    pub fn enable_multiple_select_with_ctrl(&self) {
        self.with_io(|io| {
            let imgui_io = unsafe { imgui_sys::igGetIO_Nil() };
            assert!(
                !imgui_io.is_null(),
                "dear-imnodes: ImGui IO must be available"
            );
            unsafe {
                io.MultipleSelectModifier.Modifier = std::ptr::addr_of!((*imgui_io).KeyCtrl);
            }
        });
    }
    /// Enable multiple select modifier as Shift
    pub fn enable_multiple_select_with_shift(&self) {
        self.with_io(|io| {
            let imgui_io = unsafe { imgui_sys::igGetIO_Nil() };
            assert!(
                !imgui_io.is_null(),
                "dear-imnodes: ImGui IO must be available"
            );
            unsafe {
                io.MultipleSelectModifier.Modifier = std::ptr::addr_of!((*imgui_io).KeyShift);
            }
        });
    }
    /// Emulate three-button mouse with Alt
    pub fn emulate_three_button_mouse_with_alt(&self) {
        self.with_io(|io| {
            let imgui_io = unsafe { imgui_sys::igGetIO_Nil() };
            assert!(
                !imgui_io.is_null(),
                "dear-imnodes: ImGui IO must be available"
            );
            unsafe {
                io.EmulateThreeButtonMouse.Modifier = std::ptr::addr_of!((*imgui_io).KeyAlt);
            }
        });
    }
    /// IO tweaks
    pub fn set_alt_mouse_button(&self, button: MouseButton) {
        self.with_io(|io| {
            io.AltMouseButton = button as i32;
        });
    }
    pub fn set_auto_panning_speed(&self, speed: f32) {
        self.with_io(|io| {
            io.AutoPanningSpeed = speed;
        });
    }
    /// Style preset helpers
    pub fn style_colors_dark(&self) {
        self.with_style(|style| unsafe { sys::imnodes_StyleColorsDark(style) })
    }
    pub fn style_colors_classic(&self) {
        self.with_style(|style| unsafe { sys::imnodes_StyleColorsClassic(style) })
    }
    pub fn style_colors_light(&self) {
        self.with_style(|style| unsafe { sys::imnodes_StyleColorsLight(style) })
    }

    // state save/load moved to PostEditor

    /// Node positions in grid space
    pub fn set_node_pos_grid(&self, node_id: crate::NodeId, pos: [f32; 2]) {
        let _guard = self.bind();
        unsafe {
            sys::imnodes_SetNodeGridSpacePos(
                node_id.raw(),
                sys::ImVec2_c {
                    x: pos[0],
                    y: pos[1],
                },
            )
        }
    }

    pub fn get_node_pos_grid(&self, node_id: crate::NodeId) -> [f32; 2] {
        let _guard = self.bind();
        let out = unsafe { sys::imnodes_GetNodeGridSpacePos(node_id.raw()) };
        [out.x, out.y]
    }

    /// Persistent style setters
    pub fn set_grid_spacing(&self, spacing: f32) {
        self.with_style(|style| style.GridSpacing = spacing);
    }
    pub fn set_link_thickness(&self, thickness: f32) {
        self.with_style(|style| style.LinkThickness = thickness);
    }
    pub fn set_node_corner_rounding(&self, rounding: f32) {
        self.with_style(|style| style.NodeCornerRounding = rounding);
    }
    pub fn set_node_padding(&self, padding: [f32; 2]) {
        self.with_style(|style| {
            style.NodePadding = sys::ImVec2_c {
                x: padding[0],
                y: padding[1],
            };
        });
    }
    pub fn set_pin_circle_radius(&self, r: f32) {
        self.with_style(|style| style.PinCircleRadius = r);
    }
    pub fn set_pin_quad_side_length(&self, v: f32) {
        self.with_style(|style| style.PinQuadSideLength = v);
    }
    pub fn set_pin_triangle_side_length(&self, v: f32) {
        self.with_style(|style| style.PinTriangleSideLength = v);
    }
    pub fn set_pin_line_thickness(&self, v: f32) {
        self.with_style(|style| style.PinLineThickness = v);
    }
    pub fn set_pin_hover_radius(&self, v: f32) {
        self.with_style(|style| style.PinHoverRadius = v);
    }
    pub fn set_pin_offset(&self, offset: f32) {
        self.with_style(|style| style.PinOffset = offset);
    }
    pub fn set_link_hover_distance(&self, v: f32) {
        self.with_style(|style| style.LinkHoverDistance = v);
    }
    pub fn set_link_line_segments_per_length(&self, v: f32) {
        self.with_style(|style| style.LinkLineSegmentsPerLength = v);
    }
    pub fn set_node_border_thickness(&self, v: f32) {
        self.with_style(|style| style.NodeBorderThickness = v);
    }
    pub fn set_minimap_padding(&self, padding: [f32; 2]) {
        self.with_style(|style| {
            style.MiniMapPadding = sys::ImVec2_c {
                x: padding[0],
                y: padding[1],
            };
        });
    }
    pub fn set_minimap_offset(&self, offset: [f32; 2]) {
        self.with_style(|style| {
            style.MiniMapOffset = sys::ImVec2_c {
                x: offset[0],
                y: offset[1],
            };
        });
    }

    pub fn set_color(&self, elem: crate::style::ColorElement, color: [f32; 4]) {
        let abgr = unsafe {
            imgui_sys::igColorConvertFloat4ToU32(imgui_sys::ImVec4 {
                x: color[0],
                y: color[1],
                z: color[2],
                w: color[3],
            })
        };
        self.with_style(|style| style.Colors[elem as u32 as usize] = abgr);
    }

    /// Get a style color as RGBA floats [0,1]
    pub fn get_color(&self, elem: crate::style::ColorElement) -> [f32; 4] {
        let col = self.with_style(|style| style.Colors[elem as u32 as usize]);
        let out = unsafe { imgui_sys::igColorConvertU32ToFloat4(col) };
        [out.x, out.y, out.z, out.w]
    }

    /// Node positions in screen/editor space
    pub fn set_node_pos_screen(&self, node_id: crate::NodeId, pos: [f32; 2]) {
        let _guard = self.bind();
        unsafe {
            sys::imnodes_SetNodeScreenSpacePos(
                node_id.raw(),
                sys::ImVec2_c {
                    x: pos[0],
                    y: pos[1],
                },
            )
        }
    }
    pub fn set_node_pos_editor(&self, node_id: crate::NodeId, pos: [f32; 2]) {
        let _guard = self.bind();
        unsafe {
            sys::imnodes_SetNodeEditorSpacePos(
                node_id.raw(),
                sys::ImVec2_c {
                    x: pos[0],
                    y: pos[1],
                },
            )
        }
    }
    pub fn get_node_pos_screen(&self, node_id: crate::NodeId) -> [f32; 2] {
        let _guard = self.bind();
        let out = unsafe { crate::compat_ffi::imnodes_GetNodeScreenSpacePos(node_id.raw()) };
        [out.x, out.y]
    }
    pub fn get_node_pos_editor(&self, node_id: crate::NodeId) -> [f32; 2] {
        let _guard = self.bind();
        let out = unsafe { crate::compat_ffi::imnodes_GetNodeEditorSpacePos(node_id.raw()) };
        [out.x, out.y]
    }

    /// Node drag/size helpers
    pub fn set_node_draggable(&self, node_id: crate::NodeId, draggable: bool) {
        let _guard = self.bind();
        unsafe { sys::imnodes_SetNodeDraggable(node_id.raw(), draggable) }
    }
    pub fn snap_node_to_grid(&self, node_id: crate::NodeId) {
        let _guard = self.bind();
        unsafe { sys::imnodes_SnapNodeToGrid(node_id.raw()) }
    }
    pub fn get_node_dimensions(&self, node_id: crate::NodeId) -> [f32; 2] {
        let _guard = self.bind();
        let out = unsafe { crate::compat_ffi::imnodes_GetNodeDimensions(node_id.raw()) };
        [out.x, out.y]
    }
}
