use ndk_sys::{AInputEvent, ANativeWindow};

unsafe extern "C" {
    pub fn ImGui_ImplAndroid_Init(window: *mut ANativeWindow) -> bool;
    pub fn ImGui_ImplAndroid_HandleInputEvent(input_event: *const AInputEvent) -> i32;
    pub fn ImGui_ImplAndroid_Shutdown();
    pub fn ImGui_ImplAndroid_NewFrame();
}
