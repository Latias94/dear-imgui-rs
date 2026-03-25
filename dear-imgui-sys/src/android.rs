use std::ffi::c_void;

unsafe extern "C" {
    pub fn ImGui_ImplAndroid_Init(window: *mut c_void) -> bool;
    pub fn ImGui_ImplAndroid_HandleInputEvent(input_event: *const c_void) -> i32;
    pub fn ImGui_ImplAndroid_Shutdown();
    pub fn ImGui_ImplAndroid_NewFrame();
}
