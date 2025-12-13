use crate::sys;
use dear_imgui_rs::sys as imgui_sys;
use dear_imgui_rs::{Context as ImGuiContext, Ui};
use std::os::raw::c_void;

/// Global ImNodes context
pub struct Context {
    raw: *mut sys::ImNodesContext,
}

impl Context {
    /// Try to create a new ImNodes context bound to the current Dear ImGui context
    pub fn try_create(_imgui: &ImGuiContext) -> dear_imgui_rs::ImGuiResult<Self> {
        unsafe {
            sys::imnodes_SetImGuiContext(imgui_sys::igGetCurrentContext());
        }
        let raw = unsafe { sys::imnodes_CreateContext() };
        if raw.is_null() {
            return Err(dear_imgui_rs::ImGuiError::context_creation(
                "imnodes_CreateContext returned null",
            ));
        }
        unsafe { sys::imnodes_SetCurrentContext(raw) };
        Ok(Self { raw })
    }

    /// Create a new ImNodes context (panics on error)
    pub fn create(imgui: &ImGuiContext) -> Self {
        Self::try_create(imgui).expect("Failed to create ImNodes context")
    }

    /// Set as current ImNodes context
    pub fn set_as_current(&self) {
        unsafe { sys::imnodes_SetCurrentContext(self.raw) };
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe { sys::imnodes_DestroyContext(self.raw) };
        }
    }
}

// ImNodes context interacts with Dear ImGui state and is not thread-safe.

/// An editor context allows multiple independent editors
pub struct EditorContext {
    raw: *mut sys::ImNodesEditorContext,
}

impl EditorContext {
    pub fn try_create() -> dear_imgui_rs::ImGuiResult<Self> {
        let raw = unsafe { sys::imnodes_EditorContextCreate() };
        if raw.is_null() {
            return Err(dear_imgui_rs::ImGuiError::context_creation(
                "imnodes_EditorContextCreate returned null",
            ));
        }
        Ok(Self { raw })
    }

    pub fn create() -> Self {
        Self::try_create().expect("Failed to create ImNodes editor context")
    }

    pub fn set_current(&self) {
        unsafe { sys::imnodes_EditorContextSet(self.raw) };
    }

    pub fn get_panning(&self) -> [f32; 2] {
        unsafe { sys::imnodes_EditorContextSet(self.raw) };
        let out = unsafe { crate::compat_ffi::imnodes_EditorContextGetPanning() };
        [out.x, out.y]
    }

    pub fn reset_panning(&self, pos: [f32; 2]) {
        unsafe { sys::imnodes_EditorContextSet(self.raw) };
        unsafe {
            sys::imnodes_EditorContextResetPanning(sys::ImVec2_c {
                x: pos[0],
                y: pos[1],
            })
        };
    }

    pub fn move_to_node(&self, node_id: i32) {
        unsafe { sys::imnodes_EditorContextSet(self.raw) };
        unsafe { sys::imnodes_EditorContextMoveToNode(node_id) };
    }

    /// Save this editor's state to an INI string
    pub fn save_state_to_ini_string(&self) -> String {
        unsafe {
            let mut size: usize = 0;
            let ptr = sys::imnodes_SaveEditorStateToIniString(self.raw, &mut size as *mut usize);
            if ptr.is_null() || size == 0 {
                return String::new();
            }
            let slice = std::slice::from_raw_parts(ptr as *const u8, size);
            String::from_utf8_lossy(slice).into_owned()
        }
    }

    /// Load this editor's state from an INI string
    pub fn load_state_from_ini_string(&self, data: &str) {
        unsafe {
            sys::imnodes_LoadEditorStateFromIniString(
                self.raw,
                data.as_ptr() as *const i8,
                data.len(),
            )
        }
    }

    /// Save this editor's state directly to an INI file
    pub fn save_state_to_ini_file(&self, file_name: &str) {
        let c = std::ffi::CString::new(file_name).unwrap_or_default();
        unsafe { sys::imnodes_SaveEditorStateToIniFile(self.raw, c.as_ptr()) }
    }

    /// Load this editor's state from an INI file
    pub fn load_state_from_ini_file(&self, file_name: &str) {
        let c = std::ffi::CString::new(file_name).unwrap_or_default();
        unsafe { sys::imnodes_LoadEditorStateFromIniFile(self.raw, c.as_ptr()) }
    }
}

impl Drop for EditorContext {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe { sys::imnodes_EditorContextFree(self.raw) };
        }
    }
}

// EditorContext is also not thread-safe to move/share across threads.

/// Per-frame Ui extension entry point
pub struct NodesUi<'ui> {
    _ui: &'ui Ui,
    _ctx: &'ui Context,
}

impl<'ui> NodesUi<'ui> {
    pub(crate) fn new(ui: &'ui Ui, ctx: &'ui Context) -> Self {
        // ensure current context
        ctx.set_as_current();
        Self { _ui: ui, _ctx: ctx }
    }

    /// Begin a node editor with an optional EditorContext
    pub fn editor(&self, editor: Option<&'ui EditorContext>) -> NodeEditor<'ui> {
        NodeEditor::begin(self._ui, editor)
    }
}

/// RAII token for a node editor frame
pub struct NodeEditor<'ui> {
    _ui: &'ui Ui,
    ended: bool,
}

impl<'ui> NodeEditor<'ui> {
    pub(crate) fn begin(ui: &'ui Ui, editor: Option<&EditorContext>) -> Self {
        if let Some(ed) = editor {
            unsafe { sys::imnodes_EditorContextSet(ed.raw) }
        }
        unsafe { sys::imnodes_BeginNodeEditor() };
        Self {
            _ui: ui,
            ended: false,
        }
    }

    /// Draw a minimap in the editor
    pub fn minimap(&self, size_fraction: f32, location: crate::MiniMapLocation) {
        unsafe {
            sys::imnodes_MiniMap(
                size_fraction,
                location as sys::ImNodesMiniMapLocation,
                None,
                std::ptr::null_mut(),
            )
        }
    }

    /// Draw a minimap with a node-hover callback (invoked during this call)
    pub fn minimap_with_callback<F: FnMut(i32)>(
        &self,
        size_fraction: f32,
        location: crate::MiniMapLocation,
        callback: &mut F,
    ) {
        unsafe extern "C" fn trampoline(node_id: i32, user: *mut c_void) {
            unsafe {
                let closure = &mut *(user as *mut &mut dyn FnMut(i32));
                (closure)(node_id);
            }
        }
        let mut cb_obj: &mut dyn FnMut(i32) = callback;
        let user_ptr = &mut cb_obj as *mut _ as *mut c_void;
        unsafe {
            sys::imnodes_MiniMap(
                size_fraction,
                location as sys::ImNodesMiniMapLocation,
                Some(trampoline),
                user_ptr,
            )
        }
    }

    /// Begin a node
    pub fn node(&self, id: i32) -> NodeToken<'_> {
        unsafe { sys::imnodes_BeginNode(id) };
        NodeToken {
            _phantom: std::marker::PhantomData,
        }
    }

    /// Begin an input attribute pin
    pub fn input_attr(&self, id: i32, shape: crate::PinShape) -> AttributeToken<'_> {
        unsafe { sys::imnodes_BeginInputAttribute(id, shape as sys::ImNodesPinShape) };
        AttributeToken {
            kind: AttrKind::Input,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Begin an output attribute pin
    pub fn output_attr(&self, id: i32, shape: crate::PinShape) -> AttributeToken<'_> {
        unsafe { sys::imnodes_BeginOutputAttribute(id, shape as i32) };
        AttributeToken {
            kind: AttrKind::Output,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Begin a static attribute region
    pub fn static_attr(&self, id: i32) -> AttributeToken<'_> {
        unsafe { sys::imnodes_BeginStaticAttribute(id) };
        AttributeToken {
            kind: AttrKind::Static,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Draw a link between two attributes
    pub fn link(&self, id: i32, start_attr: i32, end_attr: i32) {
        unsafe { sys::imnodes_Link(id, start_attr, end_attr) }
    }

    /// Query if a link was created this frame (attribute-id version)
    pub fn is_link_created(&self) -> Option<crate::LinkCreated> {
        let mut start_attr = 0i32;
        let mut end_attr = 0i32;
        let mut from_snap = false;
        let created = unsafe {
            sys::imnodes_IsLinkCreated_BoolPtr(
                &mut start_attr as *mut i32,
                &mut end_attr as *mut i32,
                &mut from_snap as *mut bool,
            )
        };
        if created {
            Some(crate::LinkCreated {
                start_attr,
                end_attr,
                from_snap,
            })
        } else {
            None
        }
    }

    /// Query link created with node ids and attribute ids
    pub fn is_link_created_with_nodes(&self) -> Option<crate::LinkCreatedEx> {
        let mut start_node = 0i32;
        let mut start_attr = 0i32;
        let mut end_node = 0i32;
        let mut end_attr = 0i32;
        let mut from_snap = false;
        let created = unsafe {
            sys::imnodes_IsLinkCreated_IntPtr(
                &mut start_node as *mut i32,
                &mut start_attr as *mut i32,
                &mut end_node as *mut i32,
                &mut end_attr as *mut i32,
                &mut from_snap as *mut bool,
            )
        };
        if created {
            Some(crate::LinkCreatedEx {
                start_node,
                start_attr,
                end_node,
                end_attr,
                from_snap,
            })
        } else {
            None
        }
    }

    /// Selection helpers
    pub fn selected_nodes(&self) -> Vec<i32> {
        let n = unsafe { sys::imnodes_NumSelectedNodes() };
        if n <= 0 {
            return Vec::new();
        }
        let mut buf = vec![0i32; n as usize];
        unsafe { sys::imnodes_GetSelectedNodes(buf.as_mut_ptr()) };
        buf
    }

    pub fn selected_links(&self) -> Vec<i32> {
        let n = unsafe { sys::imnodes_NumSelectedLinks() };
        if n <= 0 {
            return Vec::new();
        }
        let mut buf = vec![0i32; n as usize];
        unsafe { sys::imnodes_GetSelectedLinks(buf.as_mut_ptr()) };
        buf
    }

    pub fn clear_selection(&self) {
        unsafe {
            sys::imnodes_ClearNodeSelection_Nil();
            sys::imnodes_ClearLinkSelection_Nil();
        }
    }

    /// Hover queries
    pub fn is_editor_hovered(&self) -> bool {
        unsafe { sys::imnodes_IsEditorHovered() }
    }
    pub fn hovered_node(&self) -> Option<i32> {
        let mut id = 0;
        if unsafe { sys::imnodes_IsNodeHovered(&mut id) } {
            Some(id)
        } else {
            None
        }
    }
    pub fn hovered_link(&self) -> Option<i32> {
        let mut id = 0;
        if unsafe { sys::imnodes_IsLinkHovered(&mut id) } {
            Some(id)
        } else {
            None
        }
    }
    pub fn hovered_pin(&self) -> Option<i32> {
        let mut id = 0;
        if unsafe { sys::imnodes_IsPinHovered(&mut id) } {
            Some(id)
        } else {
            None
        }
    }

    /// Attribute active state
    pub fn is_attribute_active(&self) -> bool {
        unsafe { sys::imnodes_IsAttributeActive() }
    }
    pub fn any_attribute_active(&self) -> Option<i32> {
        let mut id = 0;
        if unsafe { sys::imnodes_IsAnyAttributeActive(&mut id) } {
            Some(id)
        } else {
            None
        }
    }

    /// Link drag/drop lifecycle
    pub fn is_link_started(&self) -> Option<i32> {
        let mut id = 0;
        if unsafe { sys::imnodes_IsLinkStarted(&mut id) } {
            Some(id)
        } else {
            None
        }
    }
    pub fn is_link_dropped(&self, including_detached: bool) -> Option<i32> {
        let mut id = 0;
        if unsafe { sys::imnodes_IsLinkDropped(&mut id, including_detached) } {
            Some(id)
        } else {
            None
        }
    }

    /// Toggle style flags (GridLines, GridLinesPrimary, GridSnapping, NodeOutline)
    pub fn set_style_flag(&self, flag: crate::StyleFlags, enabled: bool) {
        unsafe {
            let style = &mut *sys::imnodes_GetStyle();
            let mut f = style.Flags as i32;
            let bit = flag.bits();
            if enabled {
                f |= bit;
            } else {
                f &= !bit;
            }
            style.Flags = f;
        }
    }

    /// Enable link detach with Ctrl by binding to ImGui IO KeyCtrl
    pub fn enable_link_detach_with_ctrl(&self) {
        unsafe {
            let io = &mut *sys::imnodes_GetIO();
            io.LinkDetachWithModifierClick.Modifier = sys::getIOKeyCtrlPtr();
        }
    }
    /// Enable multiple select modifier as Ctrl
    pub fn enable_multiple_select_with_ctrl(&self) {
        unsafe {
            let io = &mut *sys::imnodes_GetIO();
            io.MultipleSelectModifier.Modifier = sys::getIOKeyCtrlPtr();
        }
    }
    /// Enable multiple select modifier as Shift
    pub fn enable_multiple_select_with_shift(&self) {
        unsafe {
            let io = &mut *sys::imnodes_GetIO();
            io.MultipleSelectModifier.Modifier = sys::imnodes_getIOKeyShiftPtr();
        }
    }
    /// Emulate three-button mouse with Alt
    pub fn emulate_three_button_mouse_with_alt(&self) {
        unsafe {
            let io = &mut *sys::imnodes_GetIO();
            io.EmulateThreeButtonMouse.Modifier = sys::imnodes_getIOKeyAltPtr();
        }
    }
    /// IO tweaks
    pub fn set_alt_mouse_button(&self, button: i32) {
        unsafe {
            (*sys::imnodes_GetIO()).AltMouseButton = button;
        }
    }
    pub fn set_auto_panning_speed(&self, speed: f32) {
        unsafe {
            (*sys::imnodes_GetIO()).AutoPanningSpeed = speed;
        }
    }
    /// Style preset helpers
    pub fn style_colors_dark(&self) {
        unsafe { sys::imnodes_StyleColorsDark(sys::imnodes_GetStyle()) }
    }
    pub fn style_colors_classic(&self) {
        unsafe { sys::imnodes_StyleColorsClassic(sys::imnodes_GetStyle()) }
    }
    pub fn style_colors_light(&self) {
        unsafe { sys::imnodes_StyleColorsLight(sys::imnodes_GetStyle()) }
    }

    // state save/load moved to PostEditor

    /// Node positions in grid space
    pub fn set_node_pos_grid(&self, node_id: i32, pos: [f32; 2]) {
        unsafe {
            sys::imnodes_SetNodeGridSpacePos(
                node_id,
                sys::ImVec2_c {
                    x: pos[0],
                    y: pos[1],
                },
            )
        }
    }

    pub fn get_node_pos_grid(&self, node_id: i32) -> [f32; 2] {
        let out = unsafe { sys::imnodes_GetNodeGridSpacePos(node_id) };
        [out.x, out.y]
    }

    /// Persistent style setters
    pub fn set_grid_spacing(&self, spacing: f32) {
        unsafe {
            (*sys::imnodes_GetStyle()).GridSpacing = spacing;
        }
    }
    pub fn set_link_thickness(&self, thickness: f32) {
        unsafe {
            (*sys::imnodes_GetStyle()).LinkThickness = thickness;
        }
    }
    pub fn set_node_corner_rounding(&self, rounding: f32) {
        unsafe {
            (*sys::imnodes_GetStyle()).NodeCornerRounding = rounding;
        }
    }
    pub fn set_node_padding(&self, padding: [f32; 2]) {
        unsafe {
            (*sys::imnodes_GetStyle()).NodePadding = sys::ImVec2_c {
                x: padding[0],
                y: padding[1],
            };
        }
    }
    pub fn set_pin_circle_radius(&self, r: f32) {
        unsafe {
            (*sys::imnodes_GetStyle()).PinCircleRadius = r;
        }
    }
    pub fn set_pin_quad_side_length(&self, v: f32) {
        unsafe {
            (*sys::imnodes_GetStyle()).PinQuadSideLength = v;
        }
    }
    pub fn set_pin_triangle_side_length(&self, v: f32) {
        unsafe {
            (*sys::imnodes_GetStyle()).PinTriangleSideLength = v;
        }
    }
    pub fn set_pin_line_thickness(&self, v: f32) {
        unsafe {
            (*sys::imnodes_GetStyle()).PinLineThickness = v;
        }
    }
    pub fn set_pin_hover_radius(&self, v: f32) {
        unsafe {
            (*sys::imnodes_GetStyle()).PinHoverRadius = v;
        }
    }
    pub fn set_pin_offset(&self, offset: f32) {
        unsafe {
            (*sys::imnodes_GetStyle()).PinOffset = offset;
        }
    }
    pub fn set_link_hover_distance(&self, v: f32) {
        unsafe {
            (*sys::imnodes_GetStyle()).LinkHoverDistance = v;
        }
    }
    pub fn set_link_line_segments_per_length(&self, v: f32) {
        unsafe {
            (*sys::imnodes_GetStyle()).LinkLineSegmentsPerLength = v;
        }
    }
    pub fn set_node_border_thickness(&self, v: f32) {
        unsafe {
            (*sys::imnodes_GetStyle()).NodeBorderThickness = v;
        }
    }
    pub fn set_minimap_padding(&self, padding: [f32; 2]) {
        unsafe {
            (*sys::imnodes_GetStyle()).MiniMapPadding = sys::ImVec2_c {
                x: padding[0],
                y: padding[1],
            };
        }
    }
    pub fn set_minimap_offset(&self, offset: [f32; 2]) {
        unsafe {
            (*sys::imnodes_GetStyle()).MiniMapOffset = sys::ImVec2_c {
                x: offset[0],
                y: offset[1],
            };
        }
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
        unsafe { (*sys::imnodes_GetStyle()).Colors[elem as u32 as usize] = abgr };
    }

    /// Get a style color as RGBA floats [0,1]
    pub fn get_color(&self, elem: crate::style::ColorElement) -> [f32; 4] {
        let col = unsafe { (*sys::imnodes_GetStyle()).Colors[elem as u32 as usize] };
        let out = unsafe { imgui_sys::igColorConvertU32ToFloat4(col) };
        [out.x, out.y, out.z, out.w]
    }

    /// Node positions in screen/editor space
    pub fn set_node_pos_screen(&self, node_id: i32, pos: [f32; 2]) {
        unsafe {
            sys::imnodes_SetNodeScreenSpacePos(
                node_id,
                sys::ImVec2_c {
                    x: pos[0],
                    y: pos[1],
                },
            )
        }
    }
    pub fn set_node_pos_editor(&self, node_id: i32, pos: [f32; 2]) {
        unsafe {
            sys::imnodes_SetNodeEditorSpacePos(
                node_id,
                sys::ImVec2_c {
                    x: pos[0],
                    y: pos[1],
                },
            )
        }
    }
    pub fn get_node_pos_screen(&self, node_id: i32) -> [f32; 2] {
        let out = unsafe { crate::compat_ffi::imnodes_GetNodeScreenSpacePos(node_id) };
        [out.x, out.y]
    }
    pub fn get_node_pos_editor(&self, node_id: i32) -> [f32; 2] {
        let out = unsafe { crate::compat_ffi::imnodes_GetNodeEditorSpacePos(node_id) };
        [out.x, out.y]
    }

    /// Node drag/size helpers
    pub fn set_node_draggable(&self, node_id: i32, draggable: bool) {
        unsafe { sys::imnodes_SetNodeDraggable(node_id, draggable) }
    }
    pub fn snap_node_to_grid(&self, node_id: i32) {
        unsafe { sys::imnodes_SnapNodeToGrid(node_id) }
    }
    pub fn get_node_dimensions(&self, node_id: i32) -> [f32; 2] {
        let out = unsafe { crate::compat_ffi::imnodes_GetNodeDimensions(node_id) };
        [out.x, out.y]
    }

    /// Check if a link was destroyed this frame, returning its id
    pub fn is_link_destroyed(&self) -> Option<i32> {
        let mut id = 0i32;
        let destroyed = unsafe { sys::imnodes_IsLinkDestroyed(&mut id as *mut i32) };
        if destroyed { Some(id) } else { None }
    }
}

impl<'ui> Drop for NodeEditor<'ui> {
    fn drop(&mut self) {
        if !self.ended {
            unsafe { sys::imnodes_EndNodeEditor() };
            self.ended = true;
        }
    }
}

/// RAII token for a node block
pub struct NodeToken<'a> {
    pub(crate) _phantom: std::marker::PhantomData<&'a ()>,
}
impl<'a> NodeToken<'a> {
    pub fn title_bar<F: FnOnce()>(&self, f: F) {
        unsafe { sys::imnodes_BeginNodeTitleBar() };
        f();
        unsafe { sys::imnodes_EndNodeTitleBar() };
    }
    pub fn end(self) {}
}
impl<'a> Drop for NodeToken<'a> {
    fn drop(&mut self) {
        unsafe { sys::imnodes_EndNode() }
    }
}

pub(crate) enum AttrKind {
    Input,
    Output,
    Static,
}

pub struct AttributeToken<'a> {
    pub(crate) kind: AttrKind,
    pub(crate) _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> AttributeToken<'a> {
    pub fn end(self) {}
}

impl<'a> Drop for AttributeToken<'a> {
    fn drop(&mut self) {
        unsafe {
            match self.kind {
                AttrKind::Input => sys::imnodes_EndInputAttribute(),
                AttrKind::Output => sys::imnodes_EndOutputAttribute(),
                AttrKind::Static => sys::imnodes_EndStaticAttribute(),
            }
        }
    }
}

/// Post-editor queries (must be called after EndNodeEditor)
pub struct PostEditor;

impl<'ui> NodeEditor<'ui> {
    /// Explicitly end the node editor and return post-editor query handle
    pub fn end(mut self) -> PostEditor {
        if !self.ended {
            unsafe { sys::imnodes_EndNodeEditor() };
            self.ended = true;
        }
        PostEditor
    }
}

impl PostEditor {
    /// Save current editor state to an INI string
    pub fn save_state_to_ini_string(&self) -> String {
        // Safety: ImNodes returns a pointer to an internal, null-terminated INI
        // buffer and writes its size into `size`. The pointer remains valid
        // until the next save/load call on the same editor, which we do not
        // perform while this slice is alive.
        unsafe {
            let mut size: usize = 0;
            let ptr = sys::imnodes_SaveCurrentEditorStateToIniString(&mut size as *mut usize);
            if ptr.is_null() || size == 0 {
                return String::new();
            }
            let slice = std::slice::from_raw_parts(ptr as *const u8, size);
            String::from_utf8_lossy(slice).into_owned()
        }
    }

    /// Load editor state from an INI string
    pub fn load_state_from_ini_string(&self, data: &str) {
        // Safety: ImNodes expects a pointer to a valid UTF-8 buffer and its
        // length; `data.as_ptr()` and `data.len()` satisfy this for the
        // duration of the call.
        unsafe {
            sys::imnodes_LoadCurrentEditorStateFromIniString(
                data.as_ptr() as *const i8,
                data.len(),
            );
        }
    }
    /// Save/Load current editor state to/from INI file
    pub fn save_state_to_ini_file(&self, file_name: &str) {
        let c = std::ffi::CString::new(file_name).unwrap_or_default();
        // Safety: `CString` guarantees a NUL-terminated string for the
        // duration of the call; ImNodes reads it as a const char*.
        unsafe { sys::imnodes_SaveCurrentEditorStateToIniFile(c.as_ptr()) }
    }
    pub fn load_state_from_ini_file(&self, file_name: &str) {
        let c = std::ffi::CString::new(file_name).unwrap_or_default();
        // Safety: see `save_state_to_ini_file`.
        unsafe { sys::imnodes_LoadCurrentEditorStateFromIniFile(c.as_ptr()) }
    }
    /// Selection helpers per id
    pub fn select_node(&self, node_id: i32) {
        unsafe { sys::imnodes_SelectNode(node_id) }
    }
    pub fn clear_node_selection_of(&self, node_id: i32) {
        unsafe { sys::imnodes_ClearNodeSelection_Int(node_id) }
    }
    pub fn is_node_selected(&self, node_id: i32) -> bool {
        unsafe { sys::imnodes_IsNodeSelected(node_id) }
    }
    pub fn select_link(&self, link_id: i32) {
        unsafe { sys::imnodes_SelectLink(link_id) }
    }
    pub fn clear_link_selection_of(&self, link_id: i32) {
        unsafe { sys::imnodes_ClearLinkSelection_Int(link_id) }
    }
    pub fn is_link_selected(&self, link_id: i32) -> bool {
        unsafe { sys::imnodes_IsLinkSelected(link_id) }
    }
    pub fn selected_nodes(&self) -> Vec<i32> {
        // Safety: ImNodes returns the current count of selected nodes, and
        // `GetSelectedNodes` writes exactly that many IDs into the buffer.
        let n = unsafe { sys::imnodes_NumSelectedNodes() };
        if n <= 0 {
            return Vec::new();
        }
        let mut buf = vec![0i32; n as usize];
        unsafe { sys::imnodes_GetSelectedNodes(buf.as_mut_ptr()) };
        buf
    }

    pub fn selected_links(&self) -> Vec<i32> {
        // Safety: ImNodes returns the current count of selected links, and
        // `GetSelectedLinks` writes exactly that many IDs into the buffer.
        let n = unsafe { sys::imnodes_NumSelectedLinks() };
        if n <= 0 {
            return Vec::new();
        }
        let mut buf = vec![0i32; n as usize];
        unsafe { sys::imnodes_GetSelectedLinks(buf.as_mut_ptr()) };
        buf
    }

    pub fn clear_selection(&self) {
        unsafe {
            sys::imnodes_ClearNodeSelection_Nil();
            sys::imnodes_ClearLinkSelection_Nil();
        }
    }

    pub fn is_link_created(&self) -> Option<crate::LinkCreated> {
        let mut start_attr = 0i32;
        let mut end_attr = 0i32;
        let mut from_snap = false;
        let created = unsafe {
            sys::imnodes_IsLinkCreated_BoolPtr(
                &mut start_attr as *mut i32,
                &mut end_attr as *mut i32,
                &mut from_snap as *mut bool,
            )
        };
        if created {
            Some(crate::LinkCreated {
                start_attr,
                end_attr,
                from_snap,
            })
        } else {
            None
        }
    }

    pub fn is_link_created_with_nodes(&self) -> Option<crate::LinkCreatedEx> {
        let mut start_node = 0i32;
        let mut start_attr = 0i32;
        let mut end_node = 0i32;
        let mut end_attr = 0i32;
        let mut from_snap = false;
        let created = unsafe {
            sys::imnodes_IsLinkCreated_IntPtr(
                &mut start_node as *mut i32,
                &mut start_attr as *mut i32,
                &mut end_node as *mut i32,
                &mut end_attr as *mut i32,
                &mut from_snap as *mut bool,
            )
        };
        if created {
            Some(crate::LinkCreatedEx {
                start_node,
                start_attr,
                end_node,
                end_attr,
                from_snap,
            })
        } else {
            None
        }
    }

    pub fn is_link_destroyed(&self) -> Option<i32> {
        let mut id = 0i32;
        let destroyed = unsafe { sys::imnodes_IsLinkDestroyed(&mut id as *mut i32) };
        if destroyed { Some(id) } else { None }
    }

    pub fn is_editor_hovered(&self) -> bool {
        unsafe { sys::imnodes_IsEditorHovered() }
    }
    pub fn hovered_node(&self) -> Option<i32> {
        let mut id = 0;
        if unsafe { sys::imnodes_IsNodeHovered(&mut id) } {
            Some(id)
        } else {
            None
        }
    }
    pub fn hovered_link(&self) -> Option<i32> {
        let mut id = 0;
        if unsafe { sys::imnodes_IsLinkHovered(&mut id) } {
            Some(id)
        } else {
            None
        }
    }
    pub fn hovered_pin(&self) -> Option<i32> {
        let mut id = 0;
        if unsafe { sys::imnodes_IsPinHovered(&mut id) } {
            Some(id)
        } else {
            None
        }
    }
    pub fn is_attribute_active(&self) -> bool {
        unsafe { sys::imnodes_IsAttributeActive() }
    }
    pub fn any_attribute_active(&self) -> Option<i32> {
        let mut id = 0;
        if unsafe { sys::imnodes_IsAnyAttributeActive(&mut id) } {
            Some(id)
        } else {
            None
        }
    }
    pub fn is_link_started(&self) -> Option<i32> {
        let mut id = 0;
        if unsafe { sys::imnodes_IsLinkStarted(&mut id) } {
            Some(id)
        } else {
            None
        }
    }
    pub fn is_link_dropped(&self, including_detached: bool) -> Option<i32> {
        let mut id = 0;
        if unsafe { sys::imnodes_IsLinkDropped(&mut id, including_detached) } {
            Some(id)
        } else {
            None
        }
    }
}
