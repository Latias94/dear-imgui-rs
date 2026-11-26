use crate::{CanvasSizeMode, NodeId, SaveReasonFlags, sys};
use dear_imgui_rs::{
    MouseButton,
    ffi::{RawBox, RawCString},
};
use std::{
    cell::{Cell, UnsafeCell},
    collections::HashMap,
    ffi::{CString, NulError, c_char, c_void},
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
};

/// User-defined persistence hooks for an editor context.
///
/// The handler is owned by [`EditorContext`](crate::EditorContext), so every
/// callback remains valid until the native editor has been destroyed. Reentrant
/// calls into the same handler are ignored.
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
    invoking: Cell<bool>,
    data: UnsafeCell<CallbackData>,
}

struct CallbackData {
    handler: Box<dyn SettingsHandler>,
    pending_loads: HashMap<LoadTarget, Vec<u8>>,
}

impl CallbackState {
    fn new(handler: impl SettingsHandler + 'static) -> Self {
        Self {
            invoking: Cell::new(false),
            data: UnsafeCell::new(CallbackData {
                handler: Box::new(handler),
                pending_loads: HashMap::new(),
            }),
        }
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
enum LoadTarget {
    Editor,
    Node(NodeId),
}

impl CallbackData {
    /// Native code queries the size first, then supplies a buffer of that size.
    /// Keep each result separate and consume it only on the matching copy.
    ///
    /// # Safety
    ///
    /// A non-null destination must accept the size returned by the preceding
    /// query for this target and must not alias the stored bytes.
    unsafe fn load(&mut self, target: LoadTarget, destination: *mut c_char) -> usize {
        let pending = self.pending_loads.remove(&target);
        if destination.is_null() {
            let bytes = match target {
                LoadTarget::Editor => self.handler.load_settings(),
                LoadTarget::Node(node) => self.handler.load_node_settings(node),
            }
            .unwrap_or_default();
            let len = bytes.len();
            if len != 0 {
                self.pending_loads.insert(target, bytes);
            }
            len
        } else if let Some(bytes) = pending {
            unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), destination.cast(), bytes.len()) };
            bytes.len()
        } else {
            0
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
    pub(crate) settings_file: Option<RawCString>,
    pub(crate) callbacks: Option<RawBox<CallbackState>>,
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
        self.settings_file = Some(RawCString::new(CString::new(path.as_ref())?));
        Ok(self)
    }

    pub fn no_settings_file(mut self) -> Self {
        self.settings_file = None;
        self
    }

    pub fn settings_handler(mut self, handler: impl SettingsHandler + 'static) -> Self {
        self.callbacks = Some(RawBox::new(CallbackState::new(handler)));
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
                .map(|path| path.as_c_str().to_string_lossy().into_owned()),
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

    pub(crate) fn as_sys(&self) -> sys::DneConfig {
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
                .as_ref()
                .map_or(ptr::null_mut(), |state| state.as_ptr().cast()),
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

struct InvocationGuard<'a>(&'a Cell<bool>);

impl Drop for InvocationGuard<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

/// The registered pointer must still belong to a live editor on its owning thread.
/// A nested callback may access the guard cell, but must not touch the borrowed data.
unsafe fn invoke<R: Default>(
    user_pointer: *mut c_void,
    callback: impl FnOnce(&mut CallbackData) -> R,
) -> R {
    let state = user_pointer.cast::<CallbackState>();
    if state.is_null() {
        return R::default();
    }
    let invoking = unsafe { &*ptr::addr_of!((*state).invoking) };
    if invoking.replace(true) {
        return R::default();
    }
    let _invocation = InvocationGuard(invoking);
    match catch_unwind(AssertUnwindSafe(|| {
        let data = UnsafeCell::raw_get(unsafe { ptr::addr_of!((*state).data) });
        callback(unsafe { &mut *data })
    })) {
        Ok(value) => value,
        Err(payload) => {
            if let Err(drop_panic) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
                std::mem::forget(drop_panic);
            }
            R::default()
        }
    }
}

unsafe extern "C" fn begin_save_session(user_pointer: *mut c_void) {
    unsafe { invoke(user_pointer, |state| state.handler.begin_save_session()) };
}

unsafe extern "C" fn end_save_session(user_pointer: *mut c_void) {
    unsafe { invoke(user_pointer, |state| state.handler.end_save_session()) };
}

/// A non-empty data range must be valid and readable for this call.
unsafe fn with_save_data(
    data: *const c_char,
    size: usize,
    reason: sys::DneSaveReasonFlags,
    save: impl FnOnce(&[u8], SaveReasonFlags) -> bool,
) -> bool {
    let bytes = if size == 0 {
        &[]
    } else if data.is_null() || size > isize::MAX as usize {
        return false;
    } else {
        unsafe { std::slice::from_raw_parts(data.cast(), size) }
    };
    save(bytes, SaveReasonFlags::from_bits_retain(reason as u32))
}

unsafe extern "C" fn save_settings(
    data: *const c_char,
    size: usize,
    reason: sys::DneSaveReasonFlags,
    user_pointer: *mut c_void,
) -> bool {
    unsafe {
        invoke(user_pointer, |state| {
            with_save_data(data, size, reason, |bytes, reason| {
                state.handler.save_settings(bytes, reason)
            })
        })
    }
}

unsafe extern "C" fn load_settings(data: *mut c_char, user_pointer: *mut c_void) -> usize {
    unsafe { invoke(user_pointer, |state| state.load(LoadTarget::Editor, data)) }
}

unsafe extern "C" fn save_node_settings(
    node_id: usize,
    data: *const c_char,
    size: usize,
    reason: sys::DneSaveReasonFlags,
    user_pointer: *mut c_void,
) -> bool {
    unsafe {
        invoke(user_pointer, |state| {
            with_save_data(data, size, reason, |bytes, reason| {
                state
                    .handler
                    .save_node_settings(NodeId(node_id), bytes, reason)
            })
        })
    }
}

unsafe extern "C" fn load_node_settings(
    node_id: usize,
    data: *mut c_char,
    user_pointer: *mut c_void,
) -> usize {
    unsafe {
        invoke(user_pointer, |state| {
            state.load(LoadTarget::Node(NodeId(node_id)), data)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{panic::panic_any, rc::Rc};

    #[test]
    fn load_queries_keep_separate_owned_snapshots_across_config_moves() {
        struct Settings;

        impl SettingsHandler for Settings {
            fn load_settings(&mut self) -> Option<Vec<u8>> {
                Some(vec![1])
            }

            fn load_node_settings(&mut self, node: NodeId) -> Option<Vec<u8>> {
                Some(vec![node.raw() as u8; node.raw()])
            }
        }

        let config = EditorConfig::new()
            .settings_file("editor.json")
            .unwrap()
            .settings_handler(Settings);
        let native = config.as_sys();
        let moved = Box::new(config);
        let load = native.load_settings.unwrap();
        let load_node = native.load_node_settings.unwrap();
        let state = native.user_pointer;
        let mut editor = [0u8; 3];
        let mut first_node = [0u8; 3];
        let mut second_node = [0u8; 3];

        unsafe {
            assert_eq!(
                std::ffi::CStr::from_ptr(native.settings_file),
                c"editor.json"
            );
            assert_eq!(load(ptr::null_mut(), state), 1);
            assert_eq!(load_node(2, ptr::null_mut(), state), 2);
            assert_eq!(load_node(3, ptr::null_mut(), state), 3);
            assert_eq!(load(editor.as_mut_ptr().cast(), state), 1);
            assert_eq!(load(editor.as_mut_ptr().cast(), state), 0);
            assert_eq!(load_node(2, first_node.as_mut_ptr().cast(), state), 2);
            assert_eq!(load_node(3, second_node.as_mut_ptr().cast(), state), 3);
        }
        assert_eq!(editor, [1, 0, 0]);
        assert_eq!(first_node, [2, 2, 0]);
        assert_eq!(second_node, [3; 3]);
        drop(moved);
    }

    #[test]
    fn reentrant_callbacks_are_rejected_and_panic_does_not_lock_the_handler() {
        struct PanicOnDrop;

        impl Drop for PanicOnDrop {
            fn drop(&mut self) {
                panic!("panic in payload destructor");
            }
        }

        struct Reentrant {
            pointer: Rc<Cell<*mut c_void>>,
            first_call: bool,
        }

        impl SettingsHandler for Reentrant {
            fn load_settings(&mut self) -> Option<Vec<u8>> {
                assert_eq!(
                    unsafe { load_node_settings(1, ptr::null_mut(), self.pointer.get()) },
                    0
                );
                if std::mem::take(&mut self.first_call) {
                    panic_any(PanicOnDrop);
                }
                Some(vec![7])
            }

            fn load_node_settings(&mut self, _node: NodeId) -> Option<Vec<u8>> {
                Some(vec![9])
            }
        }

        let pointer = Rc::new(Cell::new(ptr::null_mut()));
        let config = EditorConfig::new().settings_handler(Reentrant {
            pointer: pointer.clone(),
            first_call: true,
        });
        pointer.set(config.as_sys().user_pointer);

        let state = pointer.get();
        let mut data = [0u8];
        unsafe {
            assert_eq!(load_settings(ptr::null_mut(), state), 0);
            assert_eq!(load_settings(data.as_mut_ptr().cast(), state), 0);
            assert_eq!(load_settings(ptr::null_mut(), state), 1);
            assert_eq!(load_settings(data.as_mut_ptr().cast(), state), 1);
            assert_eq!(load_node_settings(1, ptr::null_mut(), state), 1);
        }
        assert_eq!(data, [7]);
    }
}
