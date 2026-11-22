//! Pointer ABI conversions for typed viewport callbacks. Native backends save and reinstall these
//! function pointers, so each pointer must continue to call its original function after a slot
//! is replaced. The C++ shim handles every by-value C++ aggregate.

use super::Viewport;
use crate::sys;
use std::ffi::{c_char, c_void};

macro_rules! viewport_callback {
    ($name:ident($($arg:ty),*) $(-> $result:ty)?) => {
        pub(super) fn $name(
            callback: Option<unsafe extern "C" fn(*mut Viewport $(, $arg)*) $(-> $result)?>,
        ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport $(, $arg)*) $(-> $result)?> {
            type Typed = unsafe extern "C" fn(*mut Viewport $(, $arg)*) $(-> $result)?;
            type Raw = unsafe extern "C" fn(*mut sys::ImGuiViewport $(, $arg)*) $(-> $result)?;
            // SAFETY: Only the pointee of the first thin pointer changes. Viewport is a
            // transparent view of ImGuiViewport. All other parameters, return type and ABI match.
            callback.map(|f| unsafe { std::mem::transmute::<Typed, Raw>(f) })
        }
    };
}

viewport_callback!(window());
viewport_callback!(predicate() -> bool);
viewport_callback!(dpi_scale() -> f32);
viewport_callback!(title(*const c_char));
viewport_callback!(alpha(f32));
viewport_callback!(render(*mut c_void));
viewport_callback!(vec2_output(*mut sys::ImVec2));
viewport_callback!(vec4_output(*mut sys::ImVec4));

pub(super) fn vec2_input(
    callback: Option<unsafe extern "C" fn(*mut Viewport, &sys::ImVec2)>,
) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *const sys::ImVec2)> {
    type Typed = unsafe extern "C" fn(*mut Viewport, &sys::ImVec2);
    type Raw = unsafe extern "C" fn(*mut sys::ImGuiViewport, *const sys::ImVec2);
    // SAFETY: The C++ thunk passes a pointer to its live, aligned local ImVec2 for the entire
    // call. The shared reference has the same ABI as that pointer, and the viewport is transparent.
    callback.map(|f| unsafe { std::mem::transmute::<Typed, Raw>(f) })
}
