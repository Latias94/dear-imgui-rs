use std::ffi::c_void;

pub type Hwnd = *mut c_void;
pub type Wparam = usize;
pub type Lparam = isize;
pub type Lresult = isize;

unsafe extern "C" {
    pub fn ImGui_ImplWin32_Init(hwnd: Hwnd) -> bool;
    pub fn ImGui_ImplWin32_InitForOpenGL(hwnd: Hwnd) -> bool;
    pub fn ImGui_ImplWin32_Shutdown();
    pub fn ImGui_ImplWin32_NewFrame();
    pub fn ImGui_ImplWin32_WndProcHandler(
        hwnd: Hwnd,
        msg: u32,
        wparam: Wparam,
        lparam: Lparam,
    ) -> Lresult;
    pub fn ImGui_ImplWin32_EnableDpiAwareness();
    pub fn ImGui_ImplWin32_GetDpiScaleForHwnd(hwnd: Hwnd) -> f32;
    pub fn ImGui_ImplWin32_GetDpiScaleForMonitor(monitor: *mut c_void) -> f32;
    pub fn ImGui_ImplWin32_EnableAlphaCompositing(hwnd: Hwnd);
}
