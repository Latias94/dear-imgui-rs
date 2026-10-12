//! Raw Win32 + OpenGL3 integration using an application-owned WGL context.

#[cfg(windows)]
#[path = "support/win32.rs"]
mod native_window;

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use dear_imgui_sys::{
        self as sys,
        backend_shim::{opengl3, win32},
    };
    use std::{ffi::CStr, ptr, time::Duration};
    use windows::{
        Win32::Graphics::{
            Gdi::{GetDC, HDC, ReleaseDC},
            OpenGL::*,
        },
        core::w,
    };

    struct GlContext<'a> {
        window: &'a native_window::Window,
        dc: HDC,
        rc: HGLRC,
    }
    impl Drop for GlContext<'_> {
        fn drop(&mut self) {
            // SAFETY: ImGui/backend guards have already dropped; HWND remains live.
            unsafe {
                let _ = wglMakeCurrent(HDC::default(), HGLRC::default());
                if !self.rc.0.is_null() {
                    let _ = wglDeleteContext(self.rc);
                }
                let _ = ReleaseDC(Some(self.window.0), self.dc);
            }
        }
    }
    struct Context {
        raw: *mut sys::ImGuiContext,
        platform: bool,
        renderer: bool,
    }
    impl Drop for Context {
        fn drop(&mut self) {
            // SAFETY: the application keeps WGL current and HWND alive until this guard drops.
            unsafe {
                sys::igSetCurrentContext(self.raw);
                if self.renderer {
                    opengl3::dear_imgui_backend_opengl3_shutdown();
                }
                if self.platform {
                    win32::dear_imgui_backend_win32_shutdown();
                }
                sys::igDestroyContext(self.raw);
            }
        }
    }

    // SAFETY: this thread owns HWND, WGL, and the raw ImGui context. Guards enforce ordered teardown.
    unsafe {
        win32::dear_imgui_backend_win32_enable_dpi_awareness();
        let window = native_window::Window::new(
            w!("DearImGuiSysWin32OpenGL3"),
            w!("Dear ImGui - raw Win32 / OpenGL3 shims"),
        )?;
        let dc = GetDC(Some(window.0));
        if dc.0.is_null() {
            return Err(windows::core::Error::from_thread());
        }
        let mut gl = GlContext {
            window: &window,
            dc,
            rc: HGLRC::default(),
        };
        let format = PIXELFORMATDESCRIPTOR {
            nSize: std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u16,
            nVersion: 1,
            dwFlags: PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER,
            iPixelType: PFD_TYPE_RGBA,
            cColorBits: 32,
            cDepthBits: 24,
            cStencilBits: 8,
            ..Default::default()
        };
        let index = ChoosePixelFormat(dc, &format);
        if index == 0 {
            return Err(windows::core::Error::from_thread());
        }
        SetPixelFormat(dc, index, &format)?;
        gl.rc = wglCreateContext(dc)?;
        wglMakeCurrent(dc, gl.rc)?;
        // The platform's compatibility context exposes the installed driver's GL version.
        // The Microsoft fallback only supports GL 1.1, which cannot render this backend.
        let version = glGetString(GL_VERSION);
        assert!(
            !version.is_null(),
            "a current WGL context must report its version"
        );
        let version = CStr::from_ptr(version.cast()).to_string_lossy();
        let major = version
            .split('.')
            .next()
            .and_then(|major| major.parse::<u32>().ok());
        assert!(
            major.is_some_and(|major| major >= 3),
            "an OpenGL 3+ driver is required; found {version}"
        );

        let raw = sys::igCreateContext(ptr::null_mut());
        assert!(!raw.is_null());
        let mut context = Context {
            raw,
            platform: false,
            renderer: false,
        };
        (*sys::igGetIO_Nil()).IniFilename = ptr::null();
        (*sys::igGetIO_Nil()).ConfigFlags |= sys::ImGuiConfigFlags_NavEnableKeyboard;
        assert!(win32::dear_imgui_backend_win32_init_for_opengl(window.0.0));
        context.platform = true;
        assert!(opengl3::dear_imgui_backend_opengl3_init(
            c"#version 130".as_ptr()
        ));
        context.renderer = true;
        window.show();
        let limit = std::env::args().find_map(|arg| {
            arg.strip_prefix("--frames=")
                .and_then(|value| value.parse::<u32>().ok())
        });
        let mut frames = 0;
        while window.pump_events() {
            let (width, height) = window.client_size()?;
            if width == 0 || height == 0 {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            opengl3::dear_imgui_backend_opengl3_new_frame();
            win32::dear_imgui_backend_win32_new_frame();
            sys::igNewFrame();
            if sys::igBegin(c"Native backend shims".as_ptr(), ptr::null_mut(), 0) {
                sys::igTextUnformatted(
                    c"The application owns HWND, WGL, resize, and presentation.".as_ptr(),
                    ptr::null(),
                );
                sys::igButton(c"Click me".as_ptr(), sys::ImVec2_c { x: 0.0, y: 0.0 });
            }
            sys::igEnd();
            sys::igRender();
            glViewport(0, 0, width as i32, height as i32);
            glClearColor(0.08, 0.1, 0.14, 1.0);
            glClear(GL_COLOR_BUFFER_BIT);
            opengl3::dear_imgui_backend_opengl3_render_draw_data(sys::igGetDrawData());
            SwapBuffers(dc)?;
            frames += 1;
            if limit.is_some_and(|limit| frames >= limit) {
                break;
            }
            std::thread::sleep(Duration::from_millis(16));
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("win32_opengl3 requires Windows");
}
