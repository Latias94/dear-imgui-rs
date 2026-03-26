use std::ffi::{CString, c_uint, c_void};
use std::ptr;
use std::time::{Duration, Instant};

use android_activity::input::{Axis, InputEvent, KeyAction, Keycode, MotionAction, ToolType};
use android_activity::{AndroidApp, InputStatus, MainEvent, PollEvent};
use dear_imgui_rs::input::{MouseButton, MouseSource};
use dear_imgui_rs::internal::RawWrapper;
use dear_imgui_rs::render::DrawData;
use dear_imgui_rs::{Condition, ConfigFlags, Context, Io, Key};
use dear_imgui_sys::backend_shim::{android as android_backend, opengl3 as opengl3_backend};
use dear_imgui_sys::{ImDrawData, ImGuiStyle_ScaleAllSizes};
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
}

impl EglState {
    fn create(window: &NativeWindow) -> Result<Self, String> {
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
            unsafe {
                let _ = eglTerminate(display);
            }
            return Err(format!(
                "eglCreateContext failed with EGL error {}",
                format_egl_error(unsafe { eglGetError() })
            ));
        }

        let surface = unsafe {
            eglCreateWindowSurface(display, config, window.ptr().as_ptr().cast(), ptr::null())
        };
        if surface == EGL_NO_SURFACE {
            unsafe {
                let _ = eglDestroyContext(display, context);
                let _ = eglTerminate(display);
            }
            return Err(format!(
                "eglCreateWindowSurface failed with EGL error {}",
                format_egl_error(unsafe { eglGetError() })
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
    imgui: Context,
    last_frame: Instant,
    platform_ready: bool,
    renderer_ready: bool,
    show_demo_window: bool,
    clear_color: [f32; 4],
    tap_count: u32,
}

impl SmokeApp {
    fn new() -> Self {
        let mut imgui = Context::create();
        imgui
            .set_ini_filename(None::<String>)
            .expect("disable imgui.ini for Android smoke sample");

        let io = imgui.io_mut();
        io.set_config_flags(
            io.config_flags() | ConfigFlags::IS_TOUCH_SCREEN | ConfigFlags::NAV_ENABLE_KEYBOARD,
        );

        let style_scale = 2.0;
        let style = imgui.style_mut();
        style.set_font_scale_dpi(style_scale);
        unsafe {
            ImGuiStyle_ScaleAllSizes(style.raw_mut(), style_scale);
        }

        Self {
            imgui,
            last_frame: Instant::now(),
            platform_ready: false,
            renderer_ready: false,
            show_demo_window: true,
            clear_color: [0.18, 0.24, 0.32, 1.0],
            tap_count: 0,
        }
    }

    fn bind_window(&mut self, window: &NativeWindow) -> Result<(), &'static str> {
        self.sync_display_metrics(window);
        self.last_frame = Instant::now();

        unsafe {
            if !android_backend::dear_imgui_backend_android_init(window.ptr().as_ptr().cast()) {
                return Err("ImGui Android backend init failed");
            }
        }

        self.platform_ready = true;
        Ok(())
    }

    fn sync_display_metrics(&mut self, window: &NativeWindow) {
        let width = window.width().max(1) as f32;
        let height = window.height().max(1) as f32;
        self.imgui.io_mut().set_display_size([width, height]);
    }

    fn unbind_window(&mut self) {
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

    unsafe fn init_renderer_once(&mut self) -> Result<(), &'static str> {
        if self.renderer_ready {
            return Ok(());
        }

        let glsl_version =
            CString::new("#version 300 es").expect("static GLSL version must not contain NUL");

        if !unsafe { opengl3_backend::dear_imgui_backend_opengl3_init(glsl_version.as_ptr()) } {
            return Err("ImGui OpenGL3 backend init failed");
        }

        self.renderer_ready = true;
        Ok(())
    }

    fn can_render(&self) -> bool {
        self.platform_ready && self.renderer_ready
    }

    fn prepare_frame(&mut self, window: &NativeWindow) {
        self.sync_display_metrics(window);

        let now = Instant::now();
        let delta = now.saturating_duration_since(self.last_frame);
        self.last_frame = now;
        self.imgui
            .io_mut()
            .set_delta_time(delta.as_secs_f32().max(1.0 / 240.0));

        unsafe {
            android_backend::dear_imgui_backend_android_new_frame();
            if self.renderer_ready {
                opengl3_backend::dear_imgui_backend_opengl3_new_frame();
            }
        }
    }

    fn handle_input(&mut self, event: &InputEvent<'_>) -> InputStatus {
        match event {
            InputEvent::MotionEvent(event) => self.handle_motion_event(event),
            InputEvent::KeyEvent(event) => self.handle_key_event(event),
            InputEvent::TextEvent(state) => {
                let io = self.imgui.io_mut();
                for ch in state.text.chars() {
                    if ch != '\0' {
                        io.add_input_character(ch);
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
        let pointer_index = event
            .pointer_index()
            .min(event.pointer_count().saturating_sub(1));
        let pointer = event.pointer_at_index(pointer_index);

        let io = self.imgui.io_mut();
        io.add_mouse_source_event(map_mouse_source(pointer.tool_type()));
        io.add_mouse_pos_event([pointer.x(), pointer.y()]);

        match event.action() {
            MotionAction::Down | MotionAction::PointerDown => {
                io.add_mouse_button_event(MouseButton::Left, true);
                InputStatus::Handled
            }
            MotionAction::Up | MotionAction::PointerUp | MotionAction::Cancel => {
                io.add_mouse_button_event(MouseButton::Left, false);
                InputStatus::Handled
            }
            MotionAction::Move | MotionAction::HoverMove => InputStatus::Handled,
            MotionAction::Scroll => {
                io.add_mouse_wheel_event([
                    pointer.axis_value(Axis::Hscroll),
                    pointer.axis_value(Axis::Vscroll),
                ]);
                InputStatus::Handled
            }
            _ => InputStatus::Unhandled,
        }
    }

    fn handle_key_event(&mut self, event: &android_activity::input::KeyEvent<'_>) -> InputStatus {
        let io = self.imgui.io_mut();
        update_modifiers(io, event.meta_state());

        let down = matches!(event.action(), KeyAction::Down);
        if let Some(key) = map_keycode(event.key_code()) {
            io.add_key_event(key, down);
            return InputStatus::Handled;
        }

        InputStatus::Unhandled
    }

    fn render_frame(&mut self, window: &NativeWindow) {
        let ui = self.imgui.frame();
        let show_demo_window = &mut self.show_demo_window;
        let clear_color = &mut self.clear_color;
        let tap_count = &mut self.tap_count;

        ui.window("Dear ImGui Android Smoke")
            .size([520.0, 360.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Low-level Android route: dear-imgui-rs + dear-imgui-sys");
                ui.separator();
                ui.text(format!(
                    "Display size: {:.0} x {:.0}",
                    ui.io().display_size()[0],
                    ui.io().display_size()[1]
                ));
                ui.text(format!(
                    "Platform backend ready: {}",
                    if self.platform_ready { "yes" } else { "no" }
                ));
                ui.text(format!(
                    "OpenGL3 renderer ready: {}",
                    if self.renderer_ready { "yes" } else { "no" }
                ));
                ui.text(format!("Framerate: {:.1} FPS", ui.io().framerate()));
                ui.separator();
                ui.checkbox("Show Dear ImGui demo window", show_demo_window);
                ui.color_edit4("Clear color", clear_color);
                if ui.button("Tap me") {
                    *tap_count += 1;
                }
                ui.same_line();
                ui.text(format!("Tap count: {}", *tap_count));
                ui.separator();
                ui.text(
                    "This sample owns NativeActivity packaging, EGL setup, and backend shim lifecycle.",
                );
            });

        if *show_demo_window {
            ui.show_demo_window(show_demo_window);
        }

        let clear_color = *clear_color;
        let draw_data = self.imgui.render();
        if self.renderer_ready {
            unsafe {
                glViewport(0, 0, window.width().max(1), window.height().max(1));
                glClearColor(
                    clear_color[0] * clear_color[3],
                    clear_color[1] * clear_color[3],
                    clear_color[2] * clear_color[3],
                    clear_color[3],
                );
                glClear(GL_COLOR_BUFFER_BIT);
                opengl3_backend::dear_imgui_backend_opengl3_render_draw_data(
                    draw_data as *const DrawData as *const ImDrawData,
                );
            }
        }
    }
}

fn map_mouse_source(tool_type: ToolType) -> MouseSource {
    match tool_type {
        ToolType::Mouse => MouseSource::Mouse,
        ToolType::Stylus | ToolType::Eraser => MouseSource::Pen,
        _ => MouseSource::TouchScreen,
    }
}

fn update_modifiers(io: &mut Io, meta_state: android_activity::input::MetaState) {
    io.add_key_event(Key::ModCtrl, meta_state.ctrl_on());
    io.add_key_event(Key::ModShift, meta_state.shift_on());
    io.add_key_event(Key::ModAlt, meta_state.alt_on());
    io.add_key_event(Key::ModSuper, meta_state.meta_on());
}

fn map_keycode(keycode: Keycode) -> Option<Key> {
    Some(match keycode {
        Keycode::Back | Keycode::Escape => Key::Escape,
        Keycode::Tab => Key::Tab,
        Keycode::Space => Key::Space,
        Keycode::Enter | Keycode::NumpadEnter => Key::Enter,
        Keycode::Del => Key::Backspace,
        Keycode::ForwardDel => Key::Delete,
        Keycode::DpadLeft => Key::LeftArrow,
        Keycode::DpadRight => Key::RightArrow,
        Keycode::DpadUp => Key::UpArrow,
        Keycode::DpadDown => Key::DownArrow,
        Keycode::PageUp => Key::PageUp,
        Keycode::PageDown => Key::PageDown,
        Keycode::MoveHome => Key::Home,
        Keycode::MoveEnd => Key::End,
        Keycode::Insert => Key::Insert,
        Keycode::CtrlLeft => Key::LeftCtrl,
        Keycode::CtrlRight => Key::RightCtrl,
        Keycode::ShiftLeft => Key::LeftShift,
        Keycode::ShiftRight => Key::RightShift,
        Keycode::AltLeft => Key::LeftAlt,
        Keycode::AltRight => Key::RightAlt,
        Keycode::MetaLeft => Key::LeftSuper,
        Keycode::MetaRight => Key::RightSuper,
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

        app.poll_events(timeout, |event| match event {
            PollEvent::Wake | PollEvent::Timeout => {}
            PollEvent::Main(main_event) => match main_event {
                MainEvent::InitWindow { .. } => {
                    if let Some(egl) = egl_state.take() {
                        smoke.unbind_window();
                        egl.destroy();
                    }

                    if let Some(window) = app.native_window() {
                        match EglState::create(&window) {
                            Ok(egl) => {
                                if let Err(err) = smoke.bind_window(&window) {
                                    error!("failed to bind Android backend: {err}");
                                    egl.destroy();
                                } else if let Err(err) = unsafe { smoke.init_renderer_once() } {
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
                    smoke.imgui.io_mut().add_focus_event(true);
                }
                MainEvent::LostFocus => {
                    smoke.imgui.io_mut().add_focus_event(false);
                }
                MainEvent::WindowResized { .. }
                | MainEvent::RedrawNeeded { .. }
                | MainEvent::Resume { .. }
                | MainEvent::Start => {}
                MainEvent::Destroy => {
                    should_quit = true;
                }
                _ => {}
            },
            _ => {}
        });

        if should_quit || !smoke.can_render() {
            continue;
        }

        let Some(window) = app.native_window() else {
            continue;
        };

        let Some(egl) = egl_state.as_ref() else {
            continue;
        };

        smoke.prepare_frame(&window);
        smoke.render_frame(&window);

        if let Err(err) = egl.swap_buffers() {
            error!("render loop lost EGL surface: {err}");
            smoke.unbind_window();
            if let Some(egl) = egl_state.take() {
                egl.destroy();
            }
        }
    }

    smoke.unbind_window();
    if let Some(egl) = egl_state.take() {
        egl.destroy();
    }
    info!("stopping Dear ImGui Android smoke app");
}
