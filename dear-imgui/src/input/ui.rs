use super::{Key, KeyChord, MouseButton, NextItemShortcutOptions, ShortcutOptions};
use crate::sys;

// TODO: Add NavInput enum once we have proper constants in sys crate

impl crate::Ui {
    /// Check if a key is being held down
    #[doc(alias = "IsKeyDown")]
    pub fn is_key_down(&self, key: Key) -> bool {
        self.run_with_bound_context(|| unsafe { sys::igIsKeyDown_Nil(key as sys::ImGuiKey) })
    }

    /// Check if a key was pressed (went from !Down to Down)
    #[doc(alias = "IsKeyPressed")]
    pub fn is_key_pressed(&self, key: Key) -> bool {
        self.run_with_bound_context(|| unsafe {
            sys::igIsKeyPressed_Bool(key as sys::ImGuiKey, true)
        })
    }

    /// Check if a key was pressed (went from !Down to Down), with repeat
    #[doc(alias = "IsKeyPressed")]
    pub fn is_key_pressed_with_repeat(&self, key: Key, repeat: bool) -> bool {
        self.run_with_bound_context(|| unsafe {
            sys::igIsKeyPressed_Bool(key as sys::ImGuiKey, repeat)
        })
    }

    /// Check if a key was released (went from Down to !Down)
    #[doc(alias = "IsKeyReleased")]
    pub fn is_key_released(&self, key: Key) -> bool {
        self.run_with_bound_context(|| unsafe { sys::igIsKeyReleased_Nil(key as sys::ImGuiKey) })
    }

    /// Check if a key chord was pressed (e.g. `Ctrl+S`).
    #[doc(alias = "IsKeyChordPressed")]
    pub fn is_key_chord_pressed(&self, key_chord: KeyChord) -> bool {
        self.run_with_bound_context(|| unsafe { sys::igIsKeyChordPressed_Nil(key_chord.raw()) })
    }

    /// Call ImGui shortcut routing with default flags.
    #[doc(alias = "Shortcut")]
    pub fn shortcut(&self, key_chord: KeyChord) -> bool {
        self.shortcut_with_flags(key_chord, ShortcutOptions::new())
    }

    /// Call ImGui shortcut routing with explicit input options.
    #[doc(alias = "Shortcut")]
    pub fn shortcut_with_flags(
        &self,
        key_chord: KeyChord,
        flags: impl Into<ShortcutOptions>,
    ) -> bool {
        let flags = flags.into();
        self.run_with_bound_context(|| unsafe { sys::igShortcut_Nil(key_chord.raw(), flags.raw()) })
    }

    /// Set the next item's shortcut with default flags.
    #[doc(alias = "SetNextItemShortcut")]
    pub fn set_next_item_shortcut(&self, key_chord: KeyChord) {
        self.set_next_item_shortcut_with_flags(key_chord, NextItemShortcutOptions::new());
    }

    /// Set the next item's shortcut with explicit options.
    #[doc(alias = "SetNextItemShortcut")]
    pub fn set_next_item_shortcut_with_flags(
        &self,
        key_chord: KeyChord,
        flags: impl Into<NextItemShortcutOptions>,
    ) {
        let flags = flags.into();
        self.run_with_bound_context(|| unsafe {
            sys::igSetNextItemShortcut(key_chord.raw(), flags.raw())
        });
    }

    /// Overrides `io.WantCaptureKeyboard` for the next frame.
    #[doc(alias = "SetNextFrameWantCaptureKeyboard")]
    pub fn set_next_frame_want_capture_keyboard(&self, want_capture_keyboard: bool) {
        self.run_with_bound_context(|| unsafe {
            sys::igSetNextFrameWantCaptureKeyboard(want_capture_keyboard)
        });
    }

    /// Overrides `io.WantCaptureMouse` for the next frame.
    #[doc(alias = "SetNextFrameWantCaptureMouse")]
    pub fn set_next_frame_want_capture_mouse(&self, want_capture_mouse: bool) {
        self.run_with_bound_context(|| unsafe {
            sys::igSetNextFrameWantCaptureMouse(want_capture_mouse)
        });
    }

    /// Check if a mouse button is being held down
    #[doc(alias = "IsMouseDown")]
    pub fn is_mouse_down(&self, button: MouseButton) -> bool {
        self.run_with_bound_context(|| unsafe { sys::igIsMouseDown_Nil(button.into()) })
    }

    /// Check if a mouse button was clicked (went from !Down to Down)
    #[doc(alias = "IsMouseClicked")]
    pub fn is_mouse_clicked(&self, button: MouseButton) -> bool {
        self.run_with_bound_context(|| unsafe { sys::igIsMouseClicked_Bool(button.into(), false) })
    }

    /// Check if a mouse button was clicked, with repeat
    #[doc(alias = "IsMouseClicked")]
    pub fn is_mouse_clicked_with_repeat(&self, button: MouseButton, repeat: bool) -> bool {
        self.run_with_bound_context(|| unsafe { sys::igIsMouseClicked_Bool(button.into(), repeat) })
    }

    /// Check if a mouse button was released (went from Down to !Down)
    #[doc(alias = "IsMouseReleased")]
    pub fn is_mouse_released(&self, button: MouseButton) -> bool {
        self.run_with_bound_context(|| unsafe { sys::igIsMouseReleased_Nil(button.into()) })
    }

    /// Check if a mouse button was double-clicked
    #[doc(alias = "IsMouseDoubleClicked")]
    pub fn is_mouse_double_clicked(&self, button: MouseButton) -> bool {
        self.run_with_bound_context(|| unsafe { sys::igIsMouseDoubleClicked_Nil(button.into()) })
    }

    /// Returns `true` if the mouse position is valid (not NaN).
    ///
    /// This checks the current mouse position as known by Dear ImGui.
    #[doc(alias = "IsMousePosValid")]
    pub fn is_mouse_pos_valid(&self) -> bool {
        self.run_with_bound_context(|| unsafe { sys::igIsMousePosValid(std::ptr::null()) })
    }

    /// Returns `true` if the provided mouse position is valid (not NaN).
    #[doc(alias = "IsMousePosValid")]
    pub fn is_mouse_pos_valid_at(&self, pos: [f32; 2]) -> bool {
        let v = sys::ImVec2_c {
            x: pos[0],
            y: pos[1],
        };
        self.run_with_bound_context(|| unsafe {
            sys::igIsMousePosValid(&v as *const sys::ImVec2_c)
        })
    }

    /// Returns `true` if the mouse button was released and the given delay has passed.
    #[doc(alias = "IsMouseReleasedWithDelay")]
    pub fn is_mouse_released_with_delay(&self, button: MouseButton, delay: f32) -> bool {
        self.run_with_bound_context(|| unsafe {
            sys::igIsMouseReleasedWithDelay(button.into(), delay)
        })
    }

    /// Get mouse position in screen coordinates
    #[doc(alias = "GetMousePos")]
    pub fn mouse_pos(&self) -> [f32; 2] {
        let pos = self.run_with_bound_context(|| unsafe { sys::igGetMousePos() });
        [pos.x, pos.y]
    }

    /// Get mouse position when a specific button was clicked
    #[doc(alias = "GetMousePosOnOpeningCurrentPopup")]
    pub fn mouse_pos_on_opening_current_popup(&self) -> [f32; 2] {
        let pos =
            self.run_with_bound_context(|| unsafe { sys::igGetMousePosOnOpeningCurrentPopup() });
        [pos.x, pos.y]
    }

    /// Check if mouse is hovering given rectangle
    #[doc(alias = "IsMouseHoveringRect")]
    pub fn is_mouse_hovering_rect(&self, r_min: [f32; 2], r_max: [f32; 2]) -> bool {
        self.run_with_bound_context(|| unsafe {
            sys::igIsMouseHoveringRect(
                sys::ImVec2::new(r_min[0], r_min[1]),
                sys::ImVec2::new(r_max[0], r_max[1]),
                true,
            )
        })
    }

    /// Check if mouse is hovering given rectangle (with clipping test)
    #[doc(alias = "IsMouseHoveringRect")]
    pub fn is_mouse_hovering_rect_with_clip(
        &self,
        r_min: [f32; 2],
        r_max: [f32; 2],
        clip: bool,
    ) -> bool {
        self.run_with_bound_context(|| unsafe {
            sys::igIsMouseHoveringRect(
                sys::ImVec2::new(r_min[0], r_min[1]),
                sys::ImVec2::new(r_max[0], r_max[1]),
                clip,
            )
        })
    }

    /// Check if mouse is dragging
    #[doc(alias = "IsMouseDragging")]
    pub fn is_mouse_dragging(&self, button: MouseButton) -> bool {
        self.run_with_bound_context(|| unsafe { sys::igIsMouseDragging(button as i32, -1.0) })
    }

    /// Check if mouse is dragging with threshold
    #[doc(alias = "IsMouseDragging")]
    pub fn is_mouse_dragging_with_threshold(
        &self,
        button: MouseButton,
        lock_threshold: f32,
    ) -> bool {
        self.run_with_bound_context(|| unsafe {
            sys::igIsMouseDragging(button as i32, lock_threshold)
        })
    }

    /// Get mouse drag delta
    #[doc(alias = "GetMouseDragDelta")]
    pub fn mouse_drag_delta(&self, button: MouseButton) -> [f32; 2] {
        let delta = self
            .run_with_bound_context(|| unsafe { sys::igGetMouseDragDelta(button as i32, -1.0) });
        [delta.x, delta.y]
    }

    /// Get mouse drag delta with threshold
    #[doc(alias = "GetMouseDragDelta")]
    pub fn mouse_drag_delta_with_threshold(
        &self,
        button: MouseButton,
        lock_threshold: f32,
    ) -> [f32; 2] {
        let delta = self.run_with_bound_context(|| unsafe {
            sys::igGetMouseDragDelta(button as i32, lock_threshold)
        });
        [delta.x, delta.y]
    }

    /// Reset mouse drag delta for a specific button
    #[doc(alias = "ResetMouseDragDelta")]
    pub fn reset_mouse_drag_delta(&self, button: MouseButton) {
        self.run_with_bound_context(|| unsafe { sys::igResetMouseDragDelta(button as i32) });
    }

    /// Returns true if the last item toggled its selection state in a multi-select scope.
    ///
    /// This only makes sense when used between `BeginMultiSelect()` /
    /// `EndMultiSelect()` (or helpers built on top of them).
    #[doc(alias = "IsItemToggledSelection")]
    pub fn is_item_toggled_selection(&self) -> bool {
        self.run_with_bound_context(|| unsafe { sys::igIsItemToggledSelection() })
    }
}
