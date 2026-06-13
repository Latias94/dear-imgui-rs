use super::{Context, ImNodesScope, NodeEditor};
use crate::sys;
use dear_imgui_rs::Ui;
use std::marker::PhantomData;
use std::os::raw::c_char;
use std::rc::Rc;

/// Post-editor queries (must be called after EndNodeEditor)
pub struct PostEditor<'ui> {
    #[allow(dead_code)]
    pub(super) _ui: &'ui Ui,
    #[allow(dead_code)]
    pub(super) _ctx: &'ui Context,
    pub(super) scope: ImNodesScope,
    pub(super) editor_hovered: bool,
    pub(super) hovered_node: Option<crate::NodeId>,
    pub(super) hovered_link: Option<crate::LinkId>,
    pub(super) hovered_pin: Option<crate::PinId>,
    pub(super) link_created: Option<crate::LinkCreated>,
    pub(super) link_created_ex: Option<crate::LinkCreatedEx>,
    pub(super) link_destroyed: Option<crate::LinkId>,
    pub(super) any_attribute_active: Option<crate::PinId>,
    pub(super) link_started: Option<crate::PinId>,
    pub(super) link_dropped_excluding_detached: Option<crate::PinId>,
    pub(super) link_dropped_including_detached: Option<crate::PinId>,
    pub(super) _not_send_sync: PhantomData<Rc<()>>,
}

impl<'ui> NodeEditor<'ui> {
    /// Explicitly end the node editor and return post-editor query handle
    pub fn end(mut self) -> PostEditor<'ui> {
        let _guard = self.bind();
        if !self.ended {
            unsafe { sys::imnodes_EndNodeEditor() };
            self.ended = true;
        }

        // Capture hover state immediately after EndNodeEditor while the current ImGui window
        // is still the editor host window. This avoids calling ImNodes hover queries later
        // from a different window (e.g. a popup), which can lead to inconsistent behavior.
        let editor_hovered = unsafe { sys::imnodes_IsEditorHovered() };
        let mut hovered_node = 0i32;
        let hovered_node = if unsafe { sys::imnodes_IsNodeHovered(&mut hovered_node) } {
            Some(crate::NodeId::new(hovered_node))
        } else {
            None
        };
        let mut hovered_link = 0i32;
        let hovered_link = if unsafe { sys::imnodes_IsLinkHovered(&mut hovered_link) } {
            Some(crate::LinkId::new(hovered_link))
        } else {
            None
        };
        let mut hovered_pin = 0i32;
        let hovered_pin = if unsafe { sys::imnodes_IsPinHovered(&mut hovered_pin) } {
            Some(crate::PinId::new(hovered_pin))
        } else {
            None
        };

        // Capture post-editor interaction events immediately after EndNodeEditor for the same reason
        // as hover state (avoid calling these queries from a different ImGui window later in the frame).
        let link_created_ex = {
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
                    start_node: crate::NodeId::new(start_node),
                    start_attr: crate::PinId::new(start_attr),
                    end_node: crate::NodeId::new(end_node),
                    end_attr: crate::PinId::new(end_attr),
                    from_snap,
                })
            } else {
                None
            }
        };
        let link_created = link_created_ex.map(|ex| crate::LinkCreated {
            start_attr: ex.start_attr,
            end_attr: ex.end_attr,
            from_snap: ex.from_snap,
        });

        let link_destroyed = {
            let mut id = 0i32;
            if unsafe { sys::imnodes_IsLinkDestroyed(&mut id as *mut i32) } {
                Some(crate::LinkId::new(id))
            } else {
                None
            }
        };

        let any_attribute_active = {
            let mut id = 0i32;
            if unsafe { sys::imnodes_IsAnyAttributeActive(&mut id) } {
                Some(crate::PinId::new(id))
            } else {
                None
            }
        };

        let link_started = {
            let mut id = 0i32;
            if unsafe { sys::imnodes_IsLinkStarted(&mut id) } {
                Some(crate::PinId::new(id))
            } else {
                None
            }
        };

        // Only call `IsLinkDropped` twice if the first query returned false, to avoid any
        // potential "consume-on-true" behavior in upstream implementations.
        let link_dropped_excluding_detached = {
            let mut id = 0i32;
            if unsafe { sys::imnodes_IsLinkDropped(&mut id, false) } {
                Some(crate::PinId::new(id))
            } else {
                None
            }
        };
        let link_dropped_including_detached = if let Some(id) = link_dropped_excluding_detached {
            Some(id)
        } else {
            let mut id = 0i32;
            if unsafe { sys::imnodes_IsLinkDropped(&mut id, true) } {
                Some(crate::PinId::new(id))
            } else {
                None
            }
        };

        PostEditor {
            _ui: self._ui,
            _ctx: self._ctx,
            scope: self.scope.clone(),
            editor_hovered,
            hovered_node,
            hovered_link,
            hovered_pin,
            link_created,
            link_created_ex,
            link_destroyed,
            any_attribute_active,
            link_started,
            link_dropped_excluding_detached,
            link_dropped_including_detached,
            _not_send_sync: PhantomData,
        }
    }
}

impl<'ui> PostEditor<'ui> {
    #[inline]
    fn bind(&self) -> super::ImNodesScopeGuard {
        self.scope.bind()
    }

    /// Save current editor state to an INI string
    pub fn save_state_to_ini_string(&self) -> String {
        // Safety: ImNodes returns a pointer to an internal, null-terminated INI
        // buffer and writes its size into `size`. The pointer remains valid
        // until the next save/load call on the same editor, which we do not
        // perform while this slice is alive.
        unsafe {
            let _guard = self.bind();
            let mut size: usize = 0;
            let ptr = sys::imnodes_SaveCurrentEditorStateToIniString(&mut size as *mut usize);
            if ptr.is_null() || size == 0 {
                return String::new();
            }
            let mut slice = std::slice::from_raw_parts(ptr as *const u8, size);
            if slice.last() == Some(&0) {
                slice = &slice[..slice.len().saturating_sub(1)];
            }
            String::from_utf8_lossy(slice).into_owned()
        }
    }

    /// Load editor state from an INI string
    pub fn load_state_from_ini_string(&self, data: &str) {
        // Safety: ImNodes expects a pointer to a valid UTF-8 buffer and its
        // length; `data.as_ptr()` and `data.len()` satisfy this for the
        // duration of the call.
        unsafe {
            let _guard = self.bind();
            sys::imnodes_LoadCurrentEditorStateFromIniString(
                data.as_ptr() as *const c_char,
                data.len(),
            );
        }
    }

    /// Save/Load current editor state to/from INI file
    pub fn save_state_to_ini_file(&self, file_name: &str) {
        let file_name = if file_name.contains('\0') {
            ""
        } else {
            file_name
        };
        // Safety: ImNodes reads a NUL-terminated string for the duration of the call.
        let _guard = self.bind();
        dear_imgui_rs::with_scratch_txt(file_name, |ptr| unsafe {
            sys::imnodes_SaveCurrentEditorStateToIniFile(ptr)
        })
    }

    pub fn load_state_from_ini_file(&self, file_name: &str) {
        let file_name = if file_name.contains('\0') {
            ""
        } else {
            file_name
        };
        // Safety: see `save_state_to_ini_file`.
        let _guard = self.bind();
        dear_imgui_rs::with_scratch_txt(file_name, |ptr| unsafe {
            sys::imnodes_LoadCurrentEditorStateFromIniFile(ptr)
        })
    }

    /// Selection helpers per id
    pub fn select_node(&self, node_id: crate::NodeId) {
        let _guard = self.bind();
        unsafe { sys::imnodes_SelectNode(node_id.raw()) }
    }

    pub fn clear_node_selection_of(&self, node_id: crate::NodeId) {
        let _guard = self.bind();
        unsafe { sys::imnodes_ClearNodeSelection_Int(node_id.raw()) }
    }

    pub fn is_node_selected(&self, node_id: crate::NodeId) -> bool {
        let _guard = self.bind();
        unsafe { sys::imnodes_IsNodeSelected(node_id.raw()) }
    }

    pub fn select_link(&self, link_id: crate::LinkId) {
        let _guard = self.bind();
        unsafe { sys::imnodes_SelectLink(link_id.raw()) }
    }

    pub fn clear_link_selection_of(&self, link_id: crate::LinkId) {
        let _guard = self.bind();
        unsafe { sys::imnodes_ClearLinkSelection_Int(link_id.raw()) }
    }

    pub fn is_link_selected(&self, link_id: crate::LinkId) -> bool {
        let _guard = self.bind();
        unsafe { sys::imnodes_IsLinkSelected(link_id.raw()) }
    }

    pub fn selected_nodes(&self) -> Vec<crate::NodeId> {
        // Safety: ImNodes returns the current count of selected nodes, and
        // `GetSelectedNodes` writes exactly that many IDs into the buffer.
        let _guard = self.bind();
        let n = unsafe { sys::imnodes_NumSelectedNodes() };
        if n <= 0 {
            return Vec::new();
        }
        let mut buf = vec![0i32; n as usize];
        unsafe { sys::imnodes_GetSelectedNodes(buf.as_mut_ptr()) };
        buf.into_iter().map(crate::NodeId::new).collect()
    }

    pub fn selected_links(&self) -> Vec<crate::LinkId> {
        // Safety: ImNodes returns the current count of selected links, and
        // `GetSelectedLinks` writes exactly that many IDs into the buffer.
        let _guard = self.bind();
        let n = unsafe { sys::imnodes_NumSelectedLinks() };
        if n <= 0 {
            return Vec::new();
        }
        let mut buf = vec![0i32; n as usize];
        unsafe { sys::imnodes_GetSelectedLinks(buf.as_mut_ptr()) };
        buf.into_iter().map(crate::LinkId::new).collect()
    }

    pub fn clear_selection(&self) {
        let _guard = self.bind();
        unsafe {
            sys::imnodes_ClearNodeSelection_Nil();
            sys::imnodes_ClearLinkSelection_Nil();
        }
    }

    pub fn is_link_created(&self) -> Option<crate::LinkCreated> {
        self.link_created
    }

    pub fn is_link_created_with_nodes(&self) -> Option<crate::LinkCreatedEx> {
        self.link_created_ex
    }

    pub fn is_link_destroyed(&self) -> Option<crate::LinkId> {
        self.link_destroyed
    }

    pub fn is_editor_hovered(&self) -> bool {
        self.editor_hovered
    }

    pub fn hovered_node(&self) -> Option<crate::NodeId> {
        self.hovered_node
    }

    pub fn hovered_link(&self) -> Option<crate::LinkId> {
        self.hovered_link
    }

    pub fn hovered_pin(&self) -> Option<crate::PinId> {
        self.hovered_pin
    }

    /// Set a node's position in screen space for the current editor context.
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

    /// Set a node's position in grid space for the current editor context.
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

    pub fn is_attribute_active(&self) -> bool {
        self.any_attribute_active.is_some()
    }

    pub fn any_attribute_active(&self) -> Option<crate::PinId> {
        self.any_attribute_active
    }

    pub fn is_link_started(&self) -> Option<crate::PinId> {
        self.link_started
    }

    pub fn is_link_dropped(&self, including_detached: bool) -> Option<crate::PinId> {
        if including_detached {
            self.link_dropped_including_detached
        } else {
            self.link_dropped_excluding_detached
        }
    }
}
