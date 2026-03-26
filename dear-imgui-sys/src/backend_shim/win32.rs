use std::ffi::c_void;

pub type Hwnd = *mut c_void;
pub type Wparam = usize;
pub type Lparam = isize;
pub type Lresult = isize;

unsafe extern "C" {
    pub fn dear_imgui_backend_win32_init(hwnd: Hwnd) -> bool;
    pub fn dear_imgui_backend_win32_init_for_opengl(hwnd: Hwnd) -> bool;
    pub fn dear_imgui_backend_win32_shutdown();
    pub fn dear_imgui_backend_win32_new_frame();
    pub fn dear_imgui_backend_win32_wnd_proc_handler(
        hwnd: Hwnd,
        msg: u32,
        wparam: Wparam,
        lparam: Lparam,
    ) -> Lresult;
    pub fn dear_imgui_backend_win32_enable_dpi_awareness();
    pub fn dear_imgui_backend_win32_get_dpi_scale_for_hwnd(hwnd: Hwnd) -> f32;
    pub fn dear_imgui_backend_win32_get_dpi_scale_for_monitor(monitor: *mut c_void) -> f32;
    pub fn dear_imgui_backend_win32_enable_alpha_compositing(hwnd: Hwnd);
}
