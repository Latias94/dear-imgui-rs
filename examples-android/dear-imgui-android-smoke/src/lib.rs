use std::ffi::{CStr, CString, c_uint, c_void};
use std::ptr;
use std::time::Duration;

use android_activity::input::{Axis, InputEvent, KeyAction, Keycode, MotionAction, ToolType};
use android_activity::{AndroidApp, InputStatus, MainEvent, PollEvent};
use dear_imgui_sys as sys;
use dear_imgui_sys::backend_shim::{android as android_backend, opengl3 as opengl3_backend};
use log::{error, info};
use ndk::hardware_buffer_format::HardwareBufferFormat;
use ndk::native_window::NativeWindow;

type EGLint = i32;
type EGLBoolean = u32;
type EGLDisplay = *mut c_void;
type EGLConfig = *mut c_void;
type EGLContext = *mut c_void;
type EGLSurface = *mut c_void;
type EGLNativeDisplayType = *mut c_void;
type EGLNativeWindowType = *mut c_void;

const EGL_TRUE: EGLBoolean = 1;
const EGL_DEFAULT_DISPLAY: EGLNativeDisplayType = ptr::null_mut();
const EGL_NO_DISPLAY: EGLDisplay = ptr::null_mut();
const EGL_NO_CONTEXT: EGLContext = ptr::null_mut();
const EGL_NO_SURFACE: EGLSurface = ptr::null_mut();
const EGL_NONE: EGLint = 0x3038;
const EGL_RED_SIZE: EGLint = 0x3024;
const EGL_GREEN_SIZE: EGLint = 0x3023;
const EGL_BLUE_SIZE: EGLint = 0x3022;
const EGL_ALPHA_SIZE: EGLint = 0x3021;
const EGL_DEPTH_SIZE: EGLint = 0x3025;
const EGL_SURFACE_TYPE: EGLint = 0x3033;
const EGL_WINDOW_BIT: EGLint = 0x0004;
const EGL_RENDERABLE_TYPE: EGLint = 0x3040;
const EGL_OPENGL_ES3_BIT: EGLint = 0x0040;
const EGL_NATIVE_VISUAL_ID: EGLint = 0x302E;
const EGL_CONTEXT_CLIENT_VERSION: EGLint = 0x3098;

const GL_COLOR_BUFFER_BIT: c_uint = 0x0000_4000;

#[link(name = "EGL")]
unsafe extern "C" {
    fn eglGetDisplay(display_id: EGLNativeDisplayType) -> EGLDisplay;
    fn eglGetError() -> EGLint;
    fn eglInitialize(display: EGLDisplay, major: *mut EGLint, minor: *mut EGLint) -> EGLBoolean;
    fn eglChooseConfig(
        display: EGLDisplay,
        attrib_list: *const EGLint,
        configs: *mut EGLConfig,
        config_size: EGLint,
        num_config: *mut EGLint,
    ) -> EGLBoolean;
    fn eglGetConfigAttrib(
        display: EGLDisplay,
        config: EGLConfig,
        attribute: EGLint,
        value: *mut EGLint,
    ) -> EGLBoolean;
    fn eglCreateContext(
        display: EGLDisplay,
        config: EGLConfig,
        share_context: EGLContext,
        attrib_list: *const EGLint,
    ) -> EGLContext;
    fn eglCreateWindowSurface(
        display: EGLDisplay,
        config: EGLConfig,
        win: EGLNativeWindowType,
        attrib_list: *const EGLint,
    ) -> EGLSurface;
    fn eglMakeCurrent(
        display: EGLDisplay,
        draw: EGLSurface,
        read: EGLSurface,
        context: EGLContext,
    ) -> EGLBoolean;
    fn eglSwapBuffers(display: EGLDisplay, surface: EGLSurface) -> EGLBoolean;
    fn eglDestroyContext(display: EGLDisplay, context: EGLContext) -> EGLBoolean;
    fn eglDestroySurface(display: EGLDisplay, surface: EGLSurface) -> EGLBoolean;
    fn eglTerminate(display: EGLDisplay) -> EGLBoolean;
}

#[link(name = "GLESv3")]
unsafe extern "C" {
    fn glViewport(x: i32, y: i32, width: i32, height: i32);
    fn glClearColor(red: f32, green: f32, blue: f32, alpha: f32);
    fn glClear(mask: c_uint);
}

struct EglState {
    display: EGLDisplay,
    context: EGLContext,
    surface: EGLSurface,
    // The platform backend borrows this window until shutdown.
    window: NativeWindow,
}

impl EglState {
    fn create(window: NativeWindow) -> Result<Self, String> {
        let display = unsafe { eglGetDisplay(EGL_DEFAULT_DISPLAY) };
        if display == EGL_NO_DISPLAY {
            return Err(format!(
                "eglGetDisplay(EGL_DEFAULT_DISPLAY) failed with EGL error {}",
                format_egl_error(unsafe { eglGetError() })
            ));
        }

        if let Err(err) = egl_check(
            unsafe { eglInitialize(display, ptr::null_mut(), ptr::null_mut()) },
            "eglInitialize",
        ) {
            unsafe {
                let _ = eglTerminate(display);
            }
            return Err(err);
        }

        let config_attributes = [
            EGL_BLUE_SIZE,
            8,
            EGL_GREEN_SIZE,
            8,
            EGL_RED_SIZE,
            8,
            EGL_ALPHA_SIZE,
            8,
            EGL_DEPTH_SIZE,
            24,
            EGL_SURFACE_TYPE,
            EGL_WINDOW_BIT,
            EGL_RENDERABLE_TYPE,
            EGL_OPENGL_ES3_BIT,
            EGL_NONE,
        ];

        let mut config_count = 0;
        if let Err(err) = egl_check(
            unsafe {
                eglChooseConfig(
                    display,
                    config_attributes.as_ptr(),
                    ptr::null_mut(),
                    0,
                    &mut config_count,
                )
            },
            "eglChooseConfig(count)",
        ) {
            unsafe {
                let _ = eglTerminate(display);
            }
            return Err(err);
        }

        if config_count == 0 {
            unsafe {
                let _ = eglTerminate(display);
            }
            return Err("eglChooseConfig returned no matching EGL config".to_string());
        }

        let mut config = ptr::null_mut();
        if let Err(err) = egl_check(
            unsafe {
                eglChooseConfig(
                    display,
                    config_attributes.as_ptr(),
                    &mut config,
                    1,
                    &mut config_count,
                )
            },
            "eglChooseConfig(select)",
        ) {
            unsafe {
                let _ = eglTerminate(display);
            }
            return Err(err);
        }

        let mut visual_id = 0;
        if let Err(err) = egl_check(
            unsafe { eglGetConfigAttrib(display, config, EGL_NATIVE_VISUAL_ID, &mut visual_id) },
            "eglGetConfigAttrib(EGL_NATIVE_VISUAL_ID)",
        ) {
            unsafe {
                let _ = eglTerminate(display);
            }
            return Err(err);
        }

        if let Err(err) =
            window.set_buffers_geometry(0, 0, Some(HardwareBufferFormat::from(visual_id)))
        {
            unsafe {
                let _ = eglTerminate(display);
            }
            return Err(format!(
                "ANativeWindow_setBuffersGeometry failed for visual {visual_id}: {err}"
            ));
        }

        let context_attributes = [EGL_CONTEXT_CLIENT_VERSION, 3, EGL_NONE];
        let context = unsafe {
            eglCreateContext(display, config, EGL_NO_CONTEXT, context_attributes.as_ptr())
        };
        if context == EGL_NO_CONTEXT {
            let error = unsafe { eglGetError() };
            unsafe {
                let _ = eglTerminate(display);
            }
            return Err(format!(
                "eglCreateContext failed with EGL error {}",
                format_egl_error(error)
            ));
        }

        let surface = unsafe {
            eglCreateWindowSurface(display, config, window.ptr().as_ptr().cast(), ptr::null())
        };
        if surface == EGL_NO_SURFACE {
            let error = unsafe { eglGetError() };
            unsafe {
                let _ = eglDestroyContext(display, context);
                let _ = eglTerminate(display);
            }
            return Err(format!(
                "eglCreateWindowSurface failed with EGL error {}",
                format_egl_error(error)
            ));
        }

        if let Err(err) = egl_check(
            unsafe { eglMakeCurrent(display, surface, surface, context) },
            "eglMakeCurrent",
        ) {
            unsafe {
                let _ = eglDestroySurface(display, surface);
                let _ = eglDestroyContext(display, context);
                let _ = eglTerminate(display);
            }
            return Err(err);
        }

        Ok(Self {
            display,
            context,
            surface,
            window,
        })
    }

    fn swap_buffers(&self) -> Result<(), String> {
        egl_check(
            unsafe { eglSwapBuffers(self.display, self.surface) },
            "eglSwapBuffers",
        )
    }

    fn destroy(self) {
        unsafe {
            if self.display != EGL_NO_DISPLAY {
                let _ =
                    eglMakeCurrent(self.display, EGL_NO_SURFACE, EGL_NO_SURFACE, EGL_NO_CONTEXT);

                if self.context != EGL_NO_CONTEXT {
                    let _ = eglDestroyContext(self.display, self.context);
                }

                if self.surface != EGL_NO_SURFACE {
                    let _ = eglDestroySurface(self.display, self.surface);
                }

                let _ = eglTerminate(self.display);
            }
        }
    }
}

struct SmokeApp {
    context: *mut sys::ImGuiContext,
    platform_ready: bool,
    renderer_ready: bool,
    resumed: bool,
    show_demo_window: bool,
    clear_color: [f32; 4],
    tap_count: u32,
}

impl SmokeApp {
    fn new() -> Self {
        // The upstream renderer owns texture updates directly, so the entire frame
        // uses raw sys calls rather than the safe core's renderer-consumer protocol.
        let context = unsafe { sys::igCreateContext(ptr::null_mut()) };
        assert!(!context.is_null(), "create Dear ImGui context");
        let style_scale = 2.0;
        unsafe {
            let io = sys::igGetIO_Nil();
            (*io).IniFilename = ptr::null();
            (*io).ConfigFlags |=
                sys::ImGuiConfigFlags_IsTouchScreen | sys::ImGuiConfigFlags_NavEnableKeyboard;
            let style = sys::igGetStyle();
            (*style).FontScaleDpi = style_scale;
            sys::ImGuiStyle_ScaleAllSizes(style, style_scale);
        }

        Self {
            context,
            platform_ready: false,
            renderer_ready: false,
            resumed: false,
            show_demo_window: true,
            clear_color: [0.18, 0.24, 0.32, 1.0],
            tap_count: 0,
        }
    }

    fn bind_window(&mut self, window: &NativeWindow) -> Result<(), &'static str> {
        unsafe {
            sys::igSetCurrentContext(self.context);
            if !android_backend::dear_imgui_backend_android_init(window.ptr().as_ptr().cast()) {
                return Err("ImGui Android backend init failed");
            }
        }

        self.platform_ready = true;
        Ok(())
    }

    fn unbind_window(&mut self) {
        unsafe { sys::igSetCurrentContext(self.context) };
        if self.renderer_ready {
            unsafe {
                opengl3_backend::dear_imgui_backend_opengl3_shutdown();
            }
            self.renderer_ready = false;
        }

        if self.platform_ready {
            unsafe {
                android_backend::dear_imgui_backend_android_shutdown();
            }
            self.platform_ready = false;
        }
    }

    fn init_renderer(&mut self) -> Result<(), &'static str> {
        if self.renderer_ready {
            return Ok(());
        }

        if !unsafe { opengl3_backend::dear_imgui_backend_opengl3_init(c"#version 300 es".as_ptr()) }
        {
            return Err("ImGui OpenGL3 backend init failed");
        }

        self.renderer_ready = true;
        Ok(())
    }

    fn can_render(&self) -> bool {
        self.resumed && self.platform_ready && self.renderer_ready
    }

    fn prepare_frame(&mut self) {
        unsafe {
            sys::igSetCurrentContext(self.context);
            // Android supplies DisplaySize, framebuffer scale, and DeltaTime.
            android_backend::dear_imgui_backend_android_new_frame();
            opengl3_backend::dear_imgui_backend_opengl3_new_frame();
            // Exclude time spent paused from the first resumed frame.
            (*sys::igGetIO_Nil()).DeltaTime = (*sys::igGetIO_Nil()).DeltaTime.min(0.1);
        }
    }

    fn set_focus(&mut self, focused: bool) {
        unsafe {
            sys::igSetCurrentContext(self.context);
            sys::ImGuiIO_AddFocusEvent(sys::igGetIO_Nil(), focused);
        }
    }

    fn handle_input(&mut self, event: &InputEvent<'_>) -> InputStatus {
        unsafe { sys::igSetCurrentContext(self.context) };
        match event {
            InputEvent::MotionEvent(event) => self.handle_motion_event(event),
            InputEvent::KeyEvent(event) => self.handle_key_event(event),
            InputEvent::TextEvent(state) => {
                let io = unsafe { sys::igGetIO_Nil() };
                for ch in state.text.chars() {
                    if ch != '\0' {
                        unsafe { sys::ImGuiIO_AddInputCharacter(io, ch as u32) };
                    }
                }
                InputStatus::Handled
            }
            _ => InputStatus::Unhandled,
        }
    }

    fn handle_motion_event(
        &mut self,
        event: &android_activity::input::MotionEvent<'_>,
    ) -> InputStatus {
        if event.pointer_count() == 0 {
            return InputStatus::Unhandled;
        }
        let pointer_index = event
            .pointer_index()
            .min(event.pointer_count().saturating_sub(1));
        let pointer = event.pointer_at_index(pointer_index);

        let io = unsafe { sys::igGetIO_Nil() };
        unsafe {
            sys::ImGuiIO_AddMouseSourceEvent(io, map_mouse_source(pointer.tool_type()));
            sys::ImGuiIO_AddMousePosEvent(io, pointer.x(), pointer.y());
        }

        match event.action() {
            MotionAction::Down | MotionAction::PointerDown => {
                unsafe { sys::ImGuiIO_AddMouseButtonEvent(io, 0, true) };
                InputStatus::Handled
            }
            MotionAction::Up | MotionAction::PointerUp | MotionAction::Cancel => {
                unsafe {
                    sys::ImGuiIO_AddMouseButtonEvent(io, 0, false);
                    if map_mouse_source(pointer.tool_type()) == sys::ImGuiMouseSource_TouchScreen {
                        sys::ImGuiIO_AddMousePosEvent(io, -f32::MAX, -f32::MAX);
                    }
                }
                InputStatus::Handled
            }
            MotionAction::Move | MotionAction::HoverMove => InputStatus::Handled,
            MotionAction::Scroll => {
                unsafe {
                    sys::ImGuiIO_AddMouseWheelEvent(
                        io,
                        pointer.axis_value(Axis::Hscroll),
                        pointer.axis_value(Axis::Vscroll),
                    );
                }
                InputStatus::Handled
            }
            _ => InputStatus::Unhandled,
        }
    }

    fn handle_key_event(&mut self, event: &android_activity::input::KeyEvent<'_>) -> InputStatus {
        let io = unsafe { sys::igGetIO_Nil() };
        unsafe { update_modifiers(io, event.meta_state()) };

        let down = match event.action() {
            KeyAction::Down => true,
            KeyAction::Up => false,
            _ => return InputStatus::Unhandled,
        };
        if let Some(key) = map_keycode(event.key_code()) {
            unsafe { sys::ImGuiIO_AddKeyEvent(io, key, down) };
            return InputStatus::Handled;
        }

        InputStatus::Unhandled
    }

    fn render_frame(&mut self, window: &NativeWindow) {
        // No Context/Ui/DrawData borrow is held while the native renderer mutates
        // ImDrawData::Textures. It performs upstream managed-texture processing.
        unsafe {
            sys::igNewFrame();
            sys::igSetNextWindowSize(
                sys::ImVec2 { x: 520.0, y: 360.0 },
                sys::ImGuiCond_FirstUseEver,
            );
            if sys::igBegin(c"Dear ImGui Android Smoke".as_ptr(), ptr::null_mut(), 0) {
                text(c"Low-level Android route: dear-imgui-sys backend shims");
                sys::igSeparator();
                let io = sys::igGetIO_Nil();
                let display_size = (*io).DisplaySize;
                text_owned(format!(
                    "Display size: {:.0} x {:.0}",
                    display_size.x, display_size.y
                ));
                text(c"Android platform + OpenGL3 renderer ready");
                text_owned(format!("Framerate: {:.1} FPS", (*io).Framerate));
                sys::igSeparator();
                sys::igCheckbox(
                    c"Show Dear ImGui demo window".as_ptr(),
                    &mut self.show_demo_window,
                );
                sys::igColorEdit4(c"Clear color".as_ptr(), self.clear_color.as_mut_ptr(), 0);
                if sys::igButton(c"Tap me".as_ptr(), sys::ImVec2 { x: 0.0, y: 0.0 }) {
                    self.tap_count += 1;
                }
                sys::igSameLine(0.0, -1.0);
                text_owned(format!("Tap count: {}", self.tap_count));
                sys::igSeparator();
                text(c"This sample owns NativeActivity, EGL, and backend shim lifecycle.");
            }
            sys::igEnd();
            if self.show_demo_window {
                sys::igShowDemoWindow(&mut self.show_demo_window);
            }
            sys::igRender();
            let clear_color = self.clear_color;
            glViewport(0, 0, window.width().max(1), window.height().max(1));
            glClearColor(
                clear_color[0] * clear_color[3],
                clear_color[1] * clear_color[3],
                clear_color[2] * clear_color[3],
                clear_color[3],
            );
            glClear(GL_COLOR_BUFFER_BIT);
            opengl3_backend::dear_imgui_backend_opengl3_render_draw_data(sys::igGetDrawData());
        }
    }
}

impl Drop for SmokeApp {
    fn drop(&mut self) {
        // android_main shuts both backends down while EGL is still current.
        unsafe { sys::igDestroyContext(self.context) };
    }
}

unsafe fn text(value: &CStr) {
    unsafe { sys::igTextUnformatted(value.as_ptr(), ptr::null()) };
}

unsafe fn text_owned(value: String) {
    let value = CString::new(value).expect("sample text contains no NUL");
    unsafe { text(&value) };
}

fn map_mouse_source(tool_type: ToolType) -> sys::ImGuiMouseSource {
    match tool_type {
        ToolType::Mouse => sys::ImGuiMouseSource_Mouse,
        ToolType::Stylus | ToolType::Eraser => sys::ImGuiMouseSource_Pen,
        _ => sys::ImGuiMouseSource_TouchScreen,
    }
}

unsafe fn update_modifiers(io: *mut sys::ImGuiIO, meta_state: android_activity::input::MetaState) {
    unsafe {
        sys::ImGuiIO_AddKeyEvent(io, sys::ImGuiMod_Ctrl, meta_state.ctrl_on());
        sys::ImGuiIO_AddKeyEvent(io, sys::ImGuiMod_Shift, meta_state.shift_on());
        sys::ImGuiIO_AddKeyEvent(io, sys::ImGuiMod_Alt, meta_state.alt_on());
        sys::ImGuiIO_AddKeyEvent(io, sys::ImGuiMod_Super, meta_state.meta_on());
    }
}

fn map_keycode(keycode: Keycode) -> Option<sys::ImGuiKey> {
    Some(match keycode {
        Keycode::Back | Keycode::Escape => sys::ImGuiKey_Escape,
        Keycode::Tab => sys::ImGuiKey_Tab,
        Keycode::Space => sys::ImGuiKey_Space,
        Keycode::Enter | Keycode::NumpadEnter => sys::ImGuiKey_Enter,
        Keycode::Del => sys::ImGuiKey_Backspace,
        Keycode::ForwardDel => sys::ImGuiKey_Delete,
        Keycode::DpadLeft => sys::ImGuiKey_LeftArrow,
        Keycode::DpadRight => sys::ImGuiKey_RightArrow,
        Keycode::DpadUp => sys::ImGuiKey_UpArrow,
        Keycode::DpadDown => sys::ImGuiKey_DownArrow,
        Keycode::PageUp => sys::ImGuiKey_PageUp,
        Keycode::PageDown => sys::ImGuiKey_PageDown,
        Keycode::MoveHome => sys::ImGuiKey_Home,
        Keycode::MoveEnd => sys::ImGuiKey_End,
        Keycode::Insert => sys::ImGuiKey_Insert,
        Keycode::CtrlLeft => sys::ImGuiKey_LeftCtrl,
        Keycode::CtrlRight => sys::ImGuiKey_RightCtrl,
        Keycode::ShiftLeft => sys::ImGuiKey_LeftShift,
        Keycode::ShiftRight => sys::ImGuiKey_RightShift,
        Keycode::AltLeft => sys::ImGuiKey_LeftAlt,
        Keycode::AltRight => sys::ImGuiKey_RightAlt,
        Keycode::MetaLeft => sys::ImGuiKey_LeftSuper,
        Keycode::MetaRight => sys::ImGuiKey_RightSuper,
        _ => return None,
    })
}

fn egl_check(result: EGLBoolean, operation: &str) -> Result<(), String> {
    if result == EGL_TRUE {
        Ok(())
    } else {
        Err(format!(
            "{operation} failed with EGL error {}",
            format_egl_error(unsafe { eglGetError() })
        ))
    }
}

fn format_egl_error(error: EGLint) -> String {
    format!("0x{error:04X}")
}

fn render_current_frame(smoke: &mut SmokeApp, egl_state: &mut Option<EglState>) {
    // A system redraw must be presented before its callback returns, including while paused.
    // Only continuous animation is gated on the activity's resumed state.
    if !smoke.platform_ready || !smoke.renderer_ready {
        return;
    }
    let Some(egl) = egl_state.as_ref() else {
        return;
    };
    if egl.window.width() <= 0 || egl.window.height() <= 0 {
        return;
    }

    smoke.prepare_frame();
    smoke.render_frame(&egl.window);
    if let Err(err) = egl.swap_buffers() {
        error!("render loop lost EGL surface: {err}");
        smoke.unbind_window();
        if let Some(egl) = egl_state.take() {
            egl.destroy();
        }
    }
}

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    info!("starting Dear ImGui Android smoke app");

    let mut smoke = SmokeApp::new();
    let mut egl_state: Option<EglState> = None;
    let mut should_quit = false;

    loop {
        if should_quit {
            break;
        }

        let timeout = if smoke.can_render() {
            Some(Duration::from_millis(16))
        } else {
            None
        };
        let mut redrawn = false;

        app.poll_events(timeout, |event| match event {
            PollEvent::Wake | PollEvent::Timeout => {}
            PollEvent::Main(main_event) => match main_event {
                MainEvent::InitWindow { .. } => {
                    if let Some(egl) = egl_state.take() {
                        smoke.unbind_window();
                        egl.destroy();
                    }

                    if let Some(window) = app.native_window() {
                        match EglState::create(window) {
                            Ok(egl) => {
                                if let Err(err) = smoke.bind_window(&egl.window) {
                                    error!("failed to bind Android backend: {err}");
                                    egl.destroy();
                                } else if let Err(err) = smoke.init_renderer() {
                                    error!("failed to init OpenGL3 backend: {err}");
                                    smoke.unbind_window();
                                    egl.destroy();
                                } else {
                                    egl_state = Some(egl);
                                }
                            }
                            Err(err) => {
                                error!("failed to create EGL state: {err}");
                            }
                        }
                    }
                }
                MainEvent::TerminateWindow { .. } => {
                    smoke.unbind_window();
                    if let Some(egl) = egl_state.take() {
                        egl.destroy();
                    }
                }
                MainEvent::InputAvailable => match app.input_events_iter() {
                    Ok(mut iter) => {
                        while iter.next(|input_event| smoke.handle_input(input_event)) {}
                    }
                    Err(err) => {
                        error!("failed to read Android input events: {err:?}");
                    }
                },
                MainEvent::GainedFocus => {
                    smoke.set_focus(true);
                }
                MainEvent::LostFocus => {
                    smoke.set_focus(false);
                }
                MainEvent::Resume { .. } => {
                    smoke.resumed = true;
                }
                MainEvent::Pause | MainEvent::Stop => {
                    smoke.resumed = false;
                }
                MainEvent::RedrawNeeded { .. } => {
                    // NativeActivity expects presentation before acknowledging redraw.
                    render_current_frame(&mut smoke, &mut egl_state);
                    redrawn = true;
                }
                MainEvent::WindowResized { .. } | MainEvent::Start => {}
                MainEvent::Destroy => {
                    should_quit = true;
                }
                _ => {}
            },
            _ => {}
        });

        if should_quit || redrawn || !smoke.can_render() {
            continue;
        }
        render_current_frame(&mut smoke, &mut egl_state);
    }

    smoke.unbind_window();
    if let Some(egl) = egl_state.take() {
        egl.destroy();
    }
    info!("stopping Dear ImGui Android smoke app");
}
