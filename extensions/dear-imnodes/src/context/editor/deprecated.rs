use super::super::NodeEditor;
use crate::sys;

impl<'ui> NodeEditor<'ui> {
    #[deprecated(note = "Call `editor.end()` and use the returned `PostEditor` handle.")]
    pub fn is_link_created(&self) -> Option<crate::LinkCreated> {
        self.bind();
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
    #[deprecated(note = "Call `editor.end()` and use the returned `PostEditor` handle.")]
    pub fn is_link_created_with_nodes(&self) -> Option<crate::LinkCreatedEx> {
        self.bind();
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
    #[deprecated(note = "Call `editor.end()` and use the returned `PostEditor` handle.")]
    pub fn selected_nodes(&self) -> Vec<i32> {
        self.bind();
        let n = unsafe { sys::imnodes_NumSelectedNodes() };
        if n <= 0 {
            return Vec::new();
        }
        let mut buf = vec![0i32; n as usize];
        unsafe { sys::imnodes_GetSelectedNodes(buf.as_mut_ptr()) };
        buf
    }

    #[deprecated(note = "Call `editor.end()` and use the returned `PostEditor` handle.")]
    pub fn selected_links(&self) -> Vec<i32> {
        self.bind();
        let n = unsafe { sys::imnodes_NumSelectedLinks() };
        if n <= 0 {
            return Vec::new();
        }
        let mut buf = vec![0i32; n as usize];
        unsafe { sys::imnodes_GetSelectedLinks(buf.as_mut_ptr()) };
        buf
    }

    #[deprecated(note = "Call `editor.end()` and use the returned `PostEditor` handle.")]
    pub fn clear_selection(&self) {
        self.bind();
        unsafe {
            sys::imnodes_ClearNodeSelection_Nil();
            sys::imnodes_ClearLinkSelection_Nil();
        }
    }

    /// Hover queries
    #[deprecated(note = "Call `editor.end()` and use the returned `PostEditor` handle.")]
    pub fn is_editor_hovered(&self) -> bool {
        self.bind();
        unsafe { sys::imnodes_IsEditorHovered() }
    }
    #[deprecated(note = "Call `editor.end()` and use the returned `PostEditor` handle.")]
    pub fn hovered_node(&self) -> Option<i32> {
        self.bind();
        let mut id = 0;
        if unsafe { sys::imnodes_IsNodeHovered(&mut id) } {
            Some(id)
        } else {
            None
        }
    }
    #[deprecated(note = "Call `editor.end()` and use the returned `PostEditor` handle.")]
    pub fn hovered_link(&self) -> Option<i32> {
        self.bind();
        let mut id = 0;
        if unsafe { sys::imnodes_IsLinkHovered(&mut id) } {
            Some(id)
        } else {
            None
        }
    }
    #[deprecated(note = "Call `editor.end()` and use the returned `PostEditor` handle.")]
    pub fn hovered_pin(&self) -> Option<i32> {
        self.bind();
        let mut id = 0;
        if unsafe { sys::imnodes_IsPinHovered(&mut id) } {
            Some(id)
        } else {
            None
        }
    }

    /// Attribute active state
    #[deprecated(note = "Call `editor.end()` and use the returned `PostEditor` handle.")]
    pub fn is_attribute_active(&self) -> bool {
        self.bind();
        unsafe { sys::imnodes_IsAttributeActive() }
    }
    #[deprecated(note = "Call `editor.end()` and use the returned `PostEditor` handle.")]
    pub fn any_attribute_active(&self) -> Option<i32> {
        self.bind();
        let mut id = 0;
        if unsafe { sys::imnodes_IsAnyAttributeActive(&mut id) } {
            Some(id)
        } else {
            None
        }
    }

    /// Link drag/drop lifecycle
    #[deprecated(note = "Call `editor.end()` and use the returned `PostEditor` handle.")]
    pub fn is_link_started(&self) -> Option<i32> {
        self.bind();
        let mut id = 0;
        if unsafe { sys::imnodes_IsLinkStarted(&mut id) } {
            Some(id)
        } else {
            None
        }
    }
    #[deprecated(note = "Call `editor.end()` and use the returned `PostEditor` handle.")]
    pub fn is_link_dropped(&self, including_detached: bool) -> Option<i32> {
        self.bind();
        let mut id = 0;
        if unsafe { sys::imnodes_IsLinkDropped(&mut id, including_detached) } {
            Some(id)
        } else {
            None
        }
    }

    #[deprecated(note = "Call `editor.end()` and use the returned `PostEditor` handle.")]
    pub fn is_link_destroyed(&self) -> Option<i32> {
        self.bind();
        let mut id = 0i32;
        let destroyed = unsafe { sys::imnodes_IsLinkDestroyed(&mut id as *mut i32) };
        if destroyed { Some(id) } else { None }
    }
}
