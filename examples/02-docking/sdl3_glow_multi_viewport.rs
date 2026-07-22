//! SDL3 + Glow multi-viewport example.
//!
//! This example drives Dear ImGui using:
//! - SDL3 for the main window, input, and secondary platform windows;
//! - the Rust Glow renderer backend (`dear-imgui-glow`);
//! - the high-level `dear-imgui-rs` API.
//!
//! It does not use the official OpenGL3 renderer from `dear-imgui-sdl3`.
//!
//! Run with:
//! ```text
//! cargo run -p dear-imgui-examples --bin sdl3_glow_multi_viewport \
//!     --features sdl3-glow-multi-viewport
//! ```
//!
//! Automated Linux secondary-window lifecycle smoke:
//! ```text
//! python3 tools/ci/run_contract.py sdl3-glow-multi-viewport-smoke
//! ```

use std::error::Error;
use std::rc::Rc;
use std::time::Instant;

use dear_imgui_glow::{GlowRenderer, SimpleTextureMap, multi_viewport::GlowViewportRuntime};
use dear_imgui_rs::{Condition, ConfigFlags, Context};
use dear_imgui_sdl3::{self as imgui_sdl3_backend, Sdl3PlatformBackend};
#[cfg(feature = "test-engine")]
use dear_imgui_test_engine::{
    RunFlags, RunSpeed, ScriptCount, TestEngine, TestGroup, VerboseLevel,
};
use glow::HasContext;
use sdl3::event::Event;
use sdl3::keyboard::Keycode;
use sdl3::video::{GLProfile, SwapInterval, WindowPos};
#[cfg(feature = "test-engine")]
use std::{
    fmt,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

#[cfg(feature = "test-engine")]
#[derive(Clone)]
struct OpenGlRendererInfo {
    vendor: String,
    renderer: String,
    version: String,
}

#[cfg(feature = "test-engine")]
struct ViewportSmokeState {
    result_path: Option<PathBuf>,
    renderer: OpenGlRendererInfo,
    saw_secondary_viewport: bool,
    rendered_secondary_viewport: bool,
    saw_merged_viewport: bool,
    complete: bool,
}

#[cfg(feature = "test-engine")]
struct CompletedViewportSmoke {
    result_path: Option<PathBuf>,
    renderer: OpenGlRendererInfo,
    saw_secondary_viewport: bool,
    rendered_secondary_viewport: bool,
    saw_merged_viewport: bool,
}

#[cfg(feature = "test-engine")]
impl ViewportSmokeState {
    fn completed_result(&self) -> Option<CompletedViewportSmoke> {
        self.complete.then(|| CompletedViewportSmoke {
            result_path: self.result_path.clone(),
            renderer: self.renderer.clone(),
            saw_secondary_viewport: self.saw_secondary_viewport,
            rendered_secondary_viewport: self.rendered_secondary_viewport,
            saw_merged_viewport: self.saw_merged_viewport,
        })
    }
}

#[cfg(feature = "test-engine")]
impl CompletedViewportSmoke {
    fn write_after_teardown(self) -> Result<(), Box<dyn Error>> {
        let Some(path) = self.result_path else {
            return Ok(());
        };
        let json = format!(
            "{{\"schema_version\":1,\"outcome\":\"Passed\",\"renderer\":{{\"backend\":\"OpenGL\",\"vendor\":\"{}\",\"name\":\"{}\",\"version\":\"{}\"}},\"secondary_viewport_observed\":{},\"secondary_viewport_rendered\":{},\"merge_observed\":{},\"teardown_complete\":true}}",
            json_escape(&self.renderer.vendor),
            json_escape(&self.renderer.renderer),
            json_escape(&self.renderer.version),
            self.saw_secondary_viewport,
            self.rendered_secondary_viewport,
            self.saw_merged_viewport,
        );
        write_json_atomic(&path, &json)
    }
}

#[cfg(feature = "test-engine")]
fn query_opengl_renderer(gl: &glow::Context) -> OpenGlRendererInfo {
    unsafe {
        OpenGlRendererInfo {
            vendor: gl.get_parameter_string(glow::VENDOR),
            renderer: gl.get_parameter_string(glow::RENDERER),
            version: gl.get_parameter_string(glow::VERSION),
        }
    }
}

#[cfg(feature = "test-engine")]
fn validate_software_opengl_renderer(info: &OpenGlRendererInfo) -> Result<(), String> {
    let identity = format!("{} {} {}", info.vendor, info.renderer, info.version).to_lowercase();
    if !identity.contains("llvmpipe") && !identity.contains("lavapipe") {
        return Err(format!(
            "viewport smoke requires Mesa llvmpipe, selected '{}' ({}, {})",
            info.renderer, info.vendor, info.version
        ));
    }
    Ok(())
}

#[cfg(feature = "test-engine")]
fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                use fmt::Write as _;
                let _ = write!(escaped, "\\u{:04x}", character as u32);
            }
            character => escaped.push(character),
        }
    }
    escaped
}

#[cfg(feature = "test-engine")]
fn write_json_atomic(path: &Path, contents: &str) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("DEAR_IMGUI_VIEWPORT_SMOKE_JSON must name a file")?;
    let temporary = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(contents.as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        Ok::<_, Box<dyn Error>>(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(feature = "test-engine")]
type RunResult = Option<CompletedViewportSmoke>;
#[cfg(not(feature = "test-engine"))]
type RunResult = ();

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(feature = "test-engine")]
    let result = run()?;
    #[cfg(not(feature = "test-engine"))]
    run()?;
    #[cfg(feature = "test-engine")]
    if let Some(result) = result {
        // The artifact is written only after every SDL, GL, backend, and Context owner in
        // `run` has been dropped.
        result.write_after_teardown()?;
    }
    Ok(())
}

fn run() -> Result<RunResult, Box<dyn Error>> {
    let sdl = sdl3::init()?;
    let video = sdl.video()?;
    #[cfg(feature = "test-engine")]
    let run_viewport_smoke =
        std::env::var("DEAR_IMGUI_VIEWPORT_SMOKE").is_ok_and(|value| value == "1");

    let gl_attr = video.gl_attr();
    gl_attr.set_context_version(3, 2);
    gl_attr.set_context_profile(GLProfile::Core);
    gl_attr.set_depth_size(0);

    let main_scale = video
        .get_primary_display()?
        .get_content_scale()
        .unwrap_or(1.0);

    let mut window = video
        .window(
            "Dear ImGui + SDL3 + Glow (multi-viewport)",
            (800.0 * main_scale) as u32,
            (600.0 * main_scale) as u32,
        )
        .opengl()
        .resizable()
        .hidden()
        .high_pixel_density()
        .build()
        .map_err(|error| format!("failed to create SDL3 window: {error}"))?;

    let gl_context = window
        .gl_create_context()
        .map_err(|error| format!("SDL_GL_CreateContext failed: {error}"))?;
    window
        .gl_make_current(&gl_context)
        .map_err(|error| format!("SDL_GL_MakeCurrent failed: {error}"))?;
    #[cfg(feature = "test-engine")]
    let main_swap_interval = if run_viewport_smoke {
        SwapInterval::Immediate
    } else {
        SwapInterval::VSync
    };
    #[cfg(not(feature = "test-engine"))]
    let main_swap_interval = SwapInterval::VSync;
    let _ = video.gl_set_swap_interval(main_swap_interval);
    window.set_position(WindowPos::Centered, WindowPos::Centered);
    window.show();

    let gl = Rc::new(unsafe { create_glow_context(&video) });

    #[cfg(feature = "test-engine")]
    let renderer_info = query_opengl_renderer(&gl);
    #[cfg(feature = "test-engine")]
    if run_viewport_smoke {
        println!(
            "OpenGL renderer: vendor='{}', renderer='{}', version='{}'",
            renderer_info.vendor, renderer_info.renderer, renderer_info.version
        );
        if std::env::var("DEAR_IMGUI_REQUIRE_SOFTWARE_OPENGL").is_ok_and(|value| value == "1") {
            validate_software_opengl_renderer(&renderer_info)?;
        }
    }

    let mut imgui = Context::create();
    #[cfg(feature = "test-engine")]
    if run_viewport_smoke {
        imgui.set_ini_filename(None::<String>)?;
    }
    {
        let io = imgui.io_mut();
        let mut flags = io.config_flags();
        flags.insert(ConfigFlags::DOCKING_ENABLE | ConfigFlags::VIEWPORTS_ENABLE);
        io.set_config_flags(flags);
    }

    // SAFETY: `window` and `gl_context` outlive renderer/platform shutdown and Context teardown.
    let mut sdl3_backend =
        unsafe { Sdl3PlatformBackend::init_platform_for_opengl(&mut imgui, &window, &gl_context)? };

    let window_scale = window.display_scale();
    imgui.style_mut().set_font_scale_dpi(window_scale);

    let renderer = GlowRenderer::with_shared_context(
        Rc::clone(&gl),
        &mut imgui,
        Box::new(SimpleTextureMap::default()),
    )?;
    // SAFETY: SDL3's OpenGL viewport backend sets SDL_GL_SHARE_WITH_CURRENT_CONTEXT before
    // creating each secondary context, makes that viewport context current for render callbacks,
    // and restores the previous context after the callback transaction.
    let mut renderer = unsafe { GlowViewportRuntime::attach(&mut imgui, renderer)? };

    #[cfg(feature = "test-engine")]
    let mut test_engine = if run_viewport_smoke {
        let (main_x, main_y) = window.position();
        let (main_width, _) = window.size();
        let external_pos = [
            main_x as f32 + main_width as f32 + 100.0,
            main_y as f32 + 100.0,
        ];
        let merged_pos = [main_x as f32 + 100.0, main_y as f32 + 100.0];

        let mut engine = TestEngine::create()?;
        engine.start(&mut imgui)?;
        engine.set_capture_enabled(false)?;
        engine.set_run_speed(RunSpeed::Fast)?;
        engine.set_verbose_level(VerboseLevel::Info)?;
        engine.add_script_test("sdl3-glow", "multi_viewport_surface_smoke", move |test| {
            test.wait_for_item("Main/Viewport Count", ScriptCount::new(240)?)?;
            test.window_move("Main", external_pos[0], external_pos[1])?;
            test.yield_frames(ScriptCount::new(30)?)?;
            test.assert_item_read_int_eq("Main/Viewport Count", 2)?;
            test.window_move("Main", merged_pos[0], merged_pos[1])?;
            test.yield_frames(ScriptCount::new(30)?)?;
            test.assert_item_read_int_eq("Main/Viewport Count", 1)
        })?;
        engine.queue_tests(
            TestGroup::Tests,
            Some("multi_viewport_surface_smoke"),
            RunFlags::RUN_FROM_COMMAND_LINE,
        )?;
        Some(engine)
    } else {
        None
    };

    #[cfg(feature = "test-engine")]
    let mut viewport_smoke = run_viewport_smoke.then(|| ViewportSmokeState {
        result_path: std::env::var_os("DEAR_IMGUI_VIEWPORT_SMOKE_JSON").map(PathBuf::from),
        renderer: renderer_info,
        saw_secondary_viewport: false,
        rendered_secondary_viewport: false,
        saw_merged_viewport: false,
        complete: false,
    });

    let mut last_frame = Instant::now();

    'main: loop {
        while let Some(raw) = imgui_sdl3_backend::sdl3_poll_event_ll() {
            let _ = sdl3_backend.process_event(&mut imgui, &raw)?;

            let event = Event::from_ll(raw);
            match event {
                Event::Quit { .. } => break 'main,
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'main,
                Event::Window {
                    win_event: sdl3::event::WindowEvent::CloseRequested,
                    window_id,
                    ..
                } if window_id == window.id() => break 'main,
                _ => {}
            }
        }

        let now = Instant::now();
        imgui
            .io_mut()
            .set_delta_time((now - last_frame).as_secs_f32());
        last_frame = now;

        sdl3_backend.new_frame(&mut imgui)?;
        #[cfg(feature = "test-engine")]
        let mut viewport_count =
            i32::try_from(imgui.platform_io().viewports_iter().count()).unwrap_or(i32::MAX);
        #[cfg(feature = "test-engine")]
        if let Some(smoke) = viewport_smoke.as_mut() {
            if viewport_count > 1 {
                smoke.saw_secondary_viewport = true;
            } else if smoke.saw_secondary_viewport {
                smoke.saw_merged_viewport = true;
            }
        }

        let ui = imgui.frame();
        ui.dockspace_over_main_viewport();
        ui.window("Main")
            .size([420.0, 260.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("SDL3 + Glow + Dear ImGui multi-viewport");
                ui.separator();
                ui.text("Drag this window outside the main viewport to spawn an OS window.");
                #[cfg(feature = "test-engine")]
                if test_engine.is_some() {
                    ui.input_int_config("Viewport Count")
                        .flags(dear_imgui_rs::InputScalarFlags::READ_ONLY)
                        .build(&mut viewport_count);
                }
            });

        let frame = imgui.render();
        unsafe {
            let (width, height) = window.size_in_pixels();
            gl.viewport(0, 0, width as i32, height as i32);
            gl.clear_color(0.1, 0.12, 0.15, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
        }

        renderer.new_frame()?;
        renderer.render(frame)?;
        imgui.update_platform_windows();
        imgui.render_platform_windows_default();
        window.gl_make_current(&gl_context)?;
        renderer.poll_fault()?;

        #[cfg(feature = "test-engine")]
        if viewport_count > 1
            && let Some(smoke) = viewport_smoke.as_mut()
        {
            smoke.rendered_secondary_viewport = true;
        }

        window.gl_swap_window();

        #[cfg(feature = "test-engine")]
        if let Some(engine) = test_engine.as_mut() {
            engine.post_swap()?;
            let smoke_pending = viewport_smoke.as_ref().is_some_and(|smoke| !smoke.complete);
            if smoke_pending && let Some(summary) = engine.take_terminal_summary()? {
                if summary.count_tested != 1 || summary.count_success != 1 {
                    return Err(format!(
                        "viewport smoke failed: tested={}, success={}",
                        summary.count_tested, summary.count_success
                    )
                    .into());
                }
                let smoke = viewport_smoke
                    .as_mut()
                    .expect("a pending viewport smoke state must exist");
                if !smoke.saw_secondary_viewport
                    || !smoke.rendered_secondary_viewport
                    || !smoke.saw_merged_viewport
                {
                    return Err(format!(
                        "viewport smoke did not observe the complete lifecycle: secondary={}, rendered={}, merged={}",
                        smoke.saw_secondary_viewport,
                        smoke.rendered_secondary_viewport,
                        smoke.saw_merged_viewport
                    )
                    .into());
                }
                println!("SDL3/Glow multi-viewport Test Engine smoke passed");
                smoke.complete = true;
            }
        }
        #[cfg(feature = "test-engine")]
        if viewport_smoke.as_ref().is_some_and(|smoke| smoke.complete) {
            break 'main;
        }
    }

    #[cfg(feature = "test-engine")]
    let completed_result = viewport_smoke
        .as_ref()
        .and_then(ViewportSmokeState::completed_result);
    #[cfg(feature = "test-engine")]
    if let Some(engine) = test_engine.as_mut() {
        engine.shutdown()?;
    }
    renderer.shutdown(&mut imgui)?;
    sdl3_backend.shutdown(&mut imgui)?;

    #[cfg(feature = "test-engine")]
    return Ok(completed_result);
    #[cfg(not(feature = "test-engine"))]
    Ok(())
}

/// Create a Glow context from an SDL3 `VideoSubsystem`.
///
/// # Safety
///
/// Call this only after there is a current OpenGL context for the thread.
unsafe fn create_glow_context(video: &sdl3::VideoSubsystem) -> glow::Context {
    use std::ffi::c_void;

    unsafe {
        glow::Context::from_loader_function(|name| {
            video
                .gl_get_proc_address(name)
                .map(|function| function as *const c_void)
                .unwrap_or(std::ptr::null())
        })
    }
}
