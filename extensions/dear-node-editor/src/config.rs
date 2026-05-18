use crate::{CanvasSizeMode, NodeId, SaveReasonFlags, sys};
use dear_imgui_rs::MouseButton;
use std::{
    ffi::{CString, NulError, c_char, c_void},
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
};

/// User-defined persistence hooks for an editor context.
///
/// The handler is owned by [`EditorContext`](crate::EditorContext), so every
/// callback remains valid until the native editor has been destroyed.
pub trait SettingsHandler {
    fn begin_save_session(&mut self) {}
    fn end_save_session(&mut self) {}

    fn save_settings(&mut self, _data: &[u8], _reason: SaveReasonFlags) -> bool {
        false
    }

    fn load_settings(&mut self) -> Option<Vec<u8>> {
        None
    }

    fn save_node_settings(
        &mut self,
        _node: NodeId,
        _data: &[u8],
        _reason: SaveReasonFlags,
    ) -> bool {
        false
    }

    fn load_node_settings(&mut self, _node: NodeId) -> Option<Vec<u8>> {
        None
    }
}

pub(crate) struct CallbackState {
    handler: Box<dyn SettingsHandler>,
    scratch: Vec<u8>,
}

impl CallbackState {
    pub(crate) fn new(handler: Box<dyn SettingsHandler>) -> Self {
        Self {
            handler,
            scratch: Vec::new(),
        }
    }
}

/// Immutable view of the configuration used to create an editor context.
///
/// Callback function pointers and native storage pointers are intentionally not
/// exposed. The snapshot only reports whether a settings handler was installed.
#[derive(Clone, Debug, PartialEq)]
pub struct EditorConfigSnapshot {
    pub settings_file: Option<String>,
    pub has_settings_handler: bool,
    pub custom_zoom_levels: Vec<f32>,
    pub canvas_size_mode: CanvasSizeMode,
    pub drag_button: MouseButton,
    pub select_button: MouseButton,
    pub navigate_button: MouseButton,
    pub context_menu_button: MouseButton,
    pub enable_smooth_zoom: bool,
    pub smooth_zoom_power: f32,
}

/// Configuration used when creating an editor context.
pub struct EditorConfig {
    pub(crate) settings_file: Option<CString>,
    pub(crate) callbacks: Option<Box<CallbackState>>,
    pub(crate) custom_zoom_levels: Vec<f32>,
    pub(crate) canvas_size_mode: CanvasSizeMode,
    pub(crate) drag_button: MouseButton,
    pub(crate) select_button: MouseButton,
    pub(crate) navigate_button: MouseButton,
    pub(crate) context_menu_button: MouseButton,
    pub(crate) enable_smooth_zoom: bool,
    pub(crate) smooth_zoom_power: f32,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            settings_file: None,
            callbacks: None,
            custom_zoom_levels: Vec::new(),
            canvas_size_mode: CanvasSizeMode::FitVerticalView,
            drag_button: MouseButton::Left,
            select_button: MouseButton::Left,
            navigate_button: MouseButton::Right,
            context_menu_button: MouseButton::Right,
            enable_smooth_zoom: false,
            smooth_zoom_power: if cfg!(target_os = "macos") { 1.1 } else { 1.3 },
        }
    }
}

impl EditorConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn settings_file(mut self, path: impl AsRef<str>) -> Result<Self, NulError> {
        self.settings_file = Some(CString::new(path.as_ref())?);
        Ok(self)
    }

    pub fn no_settings_file(mut self) -> Self {
        self.settings_file = None;
        self
    }

    pub fn settings_handler(mut self, handler: impl SettingsHandler + 'static) -> Self {
        self.callbacks = Some(Box::new(CallbackState::new(Box::new(handler))));
        self
    }

    pub fn canvas_size_mode(mut self, mode: CanvasSizeMode) -> Self {
        self.canvas_size_mode = mode;
        self
    }

    pub fn custom_zoom_levels(mut self, levels: impl Into<Vec<f32>>) -> Self {
        let levels = levels.into();
        assert!(
            levels.iter().all(|value| value.is_finite() && *value > 0.0),
            "custom zoom levels must be positive finite values"
        );
        assert!(
            levels.windows(2).all(|pair| pair[0] < pair[1]),
            "custom zoom levels must be strictly increasing"
        );
        assert!(
            levels.len() <= i32::MAX as usize,
            "custom zoom levels exceed i32::MAX"
        );
        self.custom_zoom_levels = levels;
        self
    }

    pub fn drag_button(mut self, button: MouseButton) -> Self {
        self.drag_button = button;
        self
    }

    pub fn select_button(mut self, button: MouseButton) -> Self {
        self.select_button = button;
        self
    }

    pub fn navigate_button(mut self, button: MouseButton) -> Self {
        self.navigate_button = button;
        self
    }

    pub fn context_menu_button(mut self, button: MouseButton) -> Self {
        self.context_menu_button = button;
        self
    }

    pub fn smooth_zoom(mut self, enabled: bool, power: f32) -> Self {
        assert!(
            power.is_finite() && power > 0.0,
            "smooth zoom power must be positive"
        );
        self.enable_smooth_zoom = enabled;
        self.smooth_zoom_power = power;
        self
    }

    pub fn snapshot(&self) -> EditorConfigSnapshot {
        EditorConfigSnapshot {
            settings_file: self
                .settings_file
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            has_settings_handler: self.callbacks.is_some(),
            custom_zoom_levels: self.custom_zoom_levels.clone(),
            canvas_size_mode: self.canvas_size_mode,
            drag_button: self.drag_button,
            select_button: self.select_button,
            navigate_button: self.navigate_button,
            context_menu_button: self.context_menu_button,
            enable_smooth_zoom: self.enable_smooth_zoom,
            smooth_zoom_power: self.smooth_zoom_power,
        }
    }

    pub(crate) fn to_sys(&mut self) -> sys::DneConfig {
        let has_callbacks = self.callbacks.is_some();
        sys::DneConfig {
            settings_file: self
                .settings_file
                .as_ref()
                .map_or(ptr::null(), |s| s.as_ptr()),
            begin_save_session: if has_callbacks {
                Some(begin_save_session)
            } else {
                None
            },
            end_save_session: if has_callbacks {
                Some(end_save_session)
            } else {
                None
            },
            save_settings: if has_callbacks {
                Some(save_settings)
            } else {
                None
            },
            load_settings: if has_callbacks {
                Some(load_settings)
            } else {
                None
            },
            save_node_settings: if has_callbacks {
                Some(save_node_settings)
            } else {
                None
            },
            load_node_settings: if has_callbacks {
                Some(load_node_settings)
            } else {
                None
            },
            user_pointer: self
                .callbacks
                .as_deref_mut()
                .map_or(ptr::null_mut(), |state| state as *mut _ as *mut c_void),
            custom_zoom_levels: if self.custom_zoom_levels.is_empty() {
                ptr::null()
            } else {
                self.custom_zoom_levels.as_ptr()
            },
            custom_zoom_level_count: self.custom_zoom_levels.len() as i32,
            canvas_size_mode: self.canvas_size_mode.raw(),
            drag_button_index: self.drag_button as i32,
            select_button_index: self.select_button as i32,
            navigate_button_index: self.navigate_button as i32,
            context_menu_button_index: self.context_menu_button as i32,
            enable_smooth_zoom: self.enable_smooth_zoom,
            smooth_zoom_power: self.smooth_zoom_power,
        }
    }
}

unsafe fn callback_state<'a>(user_pointer: *mut c_void) -> Option<&'a mut CallbackState> {
    if user_pointer.is_null() {
        None
    } else {
        Some(unsafe { &mut *(user_pointer as *mut CallbackState) })
    }
}

unsafe extern "C" fn begin_save_session(user_pointer: *mut c_void) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if let Some(state) = unsafe { callback_state(user_pointer) } {
            state.handler.begin_save_session();
        }
    }));
}

unsafe extern "C" fn end_save_session(user_pointer: *mut c_void) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if let Some(state) = unsafe { callback_state(user_pointer) } {
            state.handler.end_save_session();
        }
    }));
}

unsafe extern "C" fn save_settings(
    data: *const c_char,
    size: usize,
    reason: sys::DneSaveReasonFlags,
    user_pointer: *mut c_void,
) -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(state) = (unsafe { callback_state(user_pointer) }) else {
            return false;
        };
        let bytes = if data.is_null() || size == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(data as *const u8, size) }
        };
        state
            .handler
            .save_settings(bytes, SaveReasonFlags::from_bits_retain(reason as u32))
    }))
    .unwrap_or(false)
}

unsafe extern "C" fn load_settings(data: *mut c_char, user_pointer: *mut c_void) -> usize {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(state) = (unsafe { callback_state(user_pointer) }) else {
            return 0;
        };
        if data.is_null() {
            state.scratch = state.handler.load_settings().unwrap_or_default();
            state.scratch.len()
        } else {
            unsafe {
                ptr::copy_nonoverlapping(
                    state.scratch.as_ptr(),
                    data as *mut u8,
                    state.scratch.len(),
                );
            }
            state.scratch.len()
        }
    }))
    .unwrap_or(0)
}

unsafe extern "C" fn save_node_settings(
    node_id: usize,
    data: *const c_char,
    size: usize,
    reason: sys::DneSaveReasonFlags,
    user_pointer: *mut c_void,
) -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(state) = (unsafe { callback_state(user_pointer) }) else {
            return false;
        };
        let bytes = if data.is_null() || size == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(data as *const u8, size) }
        };
        state.handler.save_node_settings(
            NodeId(node_id),
            bytes,
            SaveReasonFlags::from_bits_retain(reason as u32),
        )
    }))
    .unwrap_or(false)
}

unsafe extern "C" fn load_node_settings(
    node_id: usize,
    data: *mut c_char,
    user_pointer: *mut c_void,
) -> usize {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(state) = (unsafe { callback_state(user_pointer) }) else {
            return 0;
        };
        if data.is_null() {
            state.scratch = state
                .handler
                .load_node_settings(NodeId(node_id))
                .unwrap_or_default();
            state.scratch.len()
        } else {
            unsafe {
                ptr::copy_nonoverlapping(
                    state.scratch.as_ptr(),
                    data as *mut u8,
                    state.scratch.len(),
                );
            }
            state.scratch.len()
        }
    }))
    .unwrap_or(0)
}
