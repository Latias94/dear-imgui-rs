use windows::Win32::Foundation::*;

unsafe extern "C" {
    #[link_name = "ImGui_ImplWin32Fix_Init"]
    pub fn ImGui_ImplWin32_Init(hwnd: HWND) -> bool;
    #[link_name = "ImGui_ImplWin32Fix_InitForOpenGL"]
    pub fn ImGui_ImplWin32_InitForOpenGL(hwnd: HWND) -> bool;
    #[link_name = "ImGui_ImplWin32Fix_Shutdown"]
    pub fn ImGui_ImplWin32_Shutdown();
    #[link_name = "ImGui_ImplWin32Fix_NewFrame"]
    pub fn ImGui_ImplWin32_NewFrame();
    #[link_name = "ImGui_ImplWin32Fix_WndProcHandler"]
    pub fn ImGui_ImplWin32_WndProcHandler(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT;
    #[link_name = "ImGui_ImplWin32Fix_EnableDpiAwareness"]
    pub fn ImGui_ImplWin32_EnableDpiAwareness();
    #[link_name = "ImGui_ImplWin32Fix_GetDpiScaleForHwnd"]
    pub fn ImGui_ImplWin32_GetDpiScaleForHwnd(hwnd: HWND) -> f32;
    #[link_name = "ImGui_ImplWin32Fix_GetDpiScaleForMonitor"]
    pub fn ImGui_ImplWin32_GetDpiScaleForMonitor(monitor: *mut std::ffi::c_void) -> f32;
    #[link_name = "ImGui_ImplWin32Fix_EnableAlphaCompositing"]
    pub fn ImGui_ImplWin32_EnableAlphaCompositing(hwnd: HWND);
}
