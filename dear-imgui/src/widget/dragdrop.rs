use crate::ui::Ui;
use dear_imgui_sys as sys;
/// Drag and drop widgets
///
/// This module contains all drag and drop related UI components.
use std::ffi::CString;

/// # Widgets: Drag and Drop
impl<'frame> Ui<'frame> {
    /// Begin a drag and drop source
    ///
    /// Returns `true` if a drag operation is active and payload should be set.
    /// Must call `end_drag_drop_source()` if this returns true.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.button("Drag me!");
    /// if ui.begin_drag_drop_source() {
    ///     ui.set_drag_drop_payload("MY_DATA", b"Hello, World!");
    ///     ui.text("Dragging: Hello, World!");
    ///     ui.end_drag_drop_source();
    /// }
    /// # });
    /// ```
    pub fn begin_drag_drop_source(&mut self) -> bool {
        unsafe {
            sys::ImGui_BeginDragDropSource(0) // Default flags
        }
    }

    /// Begin a drag and drop source with flags
    ///
    /// Returns `true` if a drag operation is active and payload should be set.
    /// Must call `end_drag_drop_source()` if this returns true.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.button("Drag me!");
    /// if ui.begin_drag_drop_source_with_flags(1) { // ImGuiDragDropFlags_SourceNoPreviewTooltip
    ///     ui.set_drag_drop_payload("MY_DATA", b"Data");
    ///     ui.end_drag_drop_source();
    /// }
    /// # });
    /// ```
    pub fn begin_drag_drop_source_with_flags(&mut self, flags: i32) -> bool {
        unsafe { sys::ImGui_BeginDragDropSource(flags) }
    }

    /// Set drag and drop payload
    ///
    /// Call this within a drag source to set the data being dragged.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.begin_drag_drop_source() {
    ///     let data = "Hello, World!";
    ///     ui.set_drag_drop_payload("TEXT", data.as_bytes());
    ///     ui.end_drag_drop_source();
    /// }
    /// # });
    /// ```
    pub fn set_drag_drop_payload(&mut self, type_name: impl AsRef<str>, data: &[u8]) -> bool {
        let type_name = type_name.as_ref();
        let c_type_name = CString::new(type_name).unwrap_or_default();
        unsafe {
            sys::ImGui_SetDragDropPayload(
                c_type_name.as_ptr(),
                data.as_ptr() as *const std::ffi::c_void,
                data.len(),
                0, // Default condition (always set)
            )
        }
    }

    /// End drag and drop source (must be called after begin_drag_drop_source returns true)
    pub fn end_drag_drop_source(&mut self) {
        unsafe {
            sys::ImGui_EndDragDropSource();
        }
    }

    /// Begin a drag and drop target
    ///
    /// Returns `true` if a drag operation is hovering over this target.
    /// Must call `end_drag_drop_target()` if this returns true.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.button("Drop here!");
    /// if ui.begin_drag_drop_target() {
    ///     if let Some(payload) = ui.accept_drag_drop_payload("MY_DATA") {
    ///         println!("Received data: {:?}", payload);
    ///     }
    ///     ui.end_drag_drop_target();
    /// }
    /// # });
    /// ```
    pub fn begin_drag_drop_target(&mut self) -> bool {
        unsafe { sys::ImGui_BeginDragDropTarget() }
    }

    /// Accept drag and drop payload
    ///
    /// Call this within a drag target to accept dropped data.
    /// Returns the payload data if the drop was accepted.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.begin_drag_drop_target() {
    ///     if let Some(payload) = ui.accept_drag_drop_payload("TEXT") {
    ///         if let Ok(text) = std::str::from_utf8(payload) {
    ///             println!("Dropped text: {}", text);
    ///         }
    ///     }
    ///     ui.end_drag_drop_target();
    /// }
    /// # });
    /// ```
    pub fn accept_drag_drop_payload(&mut self, type_name: impl AsRef<str>) -> Option<&[u8]> {
        let type_name = type_name.as_ref();
        let c_type_name = CString::new(type_name).unwrap_or_default();
        unsafe {
            let payload_ptr = sys::ImGui_AcceptDragDropPayload(
                c_type_name.as_ptr(),
                0, // Default flags
            );
            if payload_ptr.is_null() {
                None
            } else {
                let payload = &*payload_ptr;
                if payload.Data.is_null() || payload.DataSize == 0 {
                    None
                } else {
                    Some(std::slice::from_raw_parts(
                        payload.Data as *const u8,
                        payload.DataSize as usize,
                    ))
                }
            }
        }
    }

    /// Accept drag and drop payload with flags
    ///
    /// Call this within a drag target to accept dropped data with custom flags.
    /// Returns the payload data if the drop was accepted.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.begin_drag_drop_target() {
    ///     // Accept payload only when mouse is released
    ///     if let Some(payload) = ui.accept_drag_drop_payload_with_flags("DATA", 1) {
    ///         println!("Data dropped: {:?}", payload);
    ///     }
    ///     ui.end_drag_drop_target();
    /// }
    /// # });
    /// ```
    pub fn accept_drag_drop_payload_with_flags(
        &mut self,
        type_name: impl AsRef<str>,
        flags: i32,
    ) -> Option<&[u8]> {
        let type_name = type_name.as_ref();
        let c_type_name = CString::new(type_name).unwrap_or_default();
        unsafe {
            let payload_ptr = sys::ImGui_AcceptDragDropPayload(c_type_name.as_ptr(), flags);
            if payload_ptr.is_null() {
                None
            } else {
                let payload = &*payload_ptr;
                if payload.Data.is_null() || payload.DataSize == 0 {
                    None
                } else {
                    Some(std::slice::from_raw_parts(
                        payload.Data as *const u8,
                        payload.DataSize as usize,
                    ))
                }
            }
        }
    }

    /// End drag and drop target (must be called after begin_drag_drop_target returns true)
    pub fn end_drag_drop_target(&mut self) {
        unsafe {
            sys::ImGui_EndDragDropTarget();
        }
    }

    /// Get the current drag and drop payload
    ///
    /// Returns the current payload without accepting it (for preview purposes).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.begin_drag_drop_target() {
    ///     if let Some(payload) = ui.get_drag_drop_payload() {
    ///         ui.text("Preview: dragging data");
    ///     }
    ///     ui.end_drag_drop_target();
    /// }
    /// # });
    /// ```
    pub fn get_drag_drop_payload(&mut self) -> Option<&[u8]> {
        unsafe {
            let payload_ptr = sys::ImGui_GetDragDropPayload();
            if payload_ptr.is_null() {
                None
            } else {
                let payload = &*payload_ptr;
                if payload.Data.is_null() || payload.DataSize == 0 {
                    None
                } else {
                    Some(std::slice::from_raw_parts(
                        payload.Data as *const u8,
                        payload.DataSize as usize,
                    ))
                }
            }
        }
    }
}
