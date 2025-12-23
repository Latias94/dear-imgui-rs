//! dear-app + WGPU texture demo
//!
//! Demonstrates loading and updating textures using Dear ImGui's modern
//! TextureData system inside the dear-app runner. It mirrors the logic of
//! `examples/01-renderers/wgpu_textures.rs`, but uses the ergonomic
//! `dear-app` API and its `AddOns::textures.update()` to avoid first-use
//! hiccups.

use ::image::ImageReader;
use dear_app::{AddOnsConfig, AppBuilder, RedrawMode, RunnerConfig, Theme};
use dear_imgui_rs::*;
use std::{path::PathBuf, time::Instant};
use wgpu as wgpu_rs;

struct TexDemoState {
    img_tex: dear_imgui_rs::texture::OwnedTextureData,
    photo_tex: Option<dear_imgui_rs::texture::OwnedTextureData>,
    tex_size: (u32, u32),
    frame: u32,
    last_ui: Instant,
    // External GPU texture demo (game view style)
    ext_size: (u32, u32),
    ext_tex: Option<wgpu_rs::Texture>,
    ext_view: Option<wgpu_rs::TextureView>,
    ext_tex_id: Option<u64>,
}

impl TexDemoState {
    fn new() -> Self {
        // Animated CPU texture
        let tex_w: u32 = 128;
        let tex_h: u32 = 128;
        let mut img_tex = dear_imgui_rs::texture::TextureData::new();
        img_tex.create(
            dear_imgui_rs::texture::TextureFormat::RGBA32,
            tex_w as i32,
            tex_h as i32,
        );

        // Seed with a gradient for first frame
        let mut pixels = vec![0u8; (tex_w * tex_h * 4) as usize];
        for y in 0..tex_h {
            for x in 0..tex_w {
                let i = ((y * tex_w + x) * 4) as usize;
                pixels[i + 0] = (x as f32 / tex_w as f32 * 255.0) as u8;
                pixels[i + 1] = (y as f32 / tex_h as f32 * 255.0) as u8;
                pixels[i + 2] = 128;
                pixels[i + 3] = 255;
            }
        }
        img_tex.set_data(&pixels);

        Self {
            img_tex,
            photo_tex: Self::maybe_load_photo_texture(),
            tex_size: (tex_w, tex_h),
            frame: 0,
            last_ui: Instant::now(),
            ext_size: (256, 256),
            ext_tex: None,
            ext_view: None,
            ext_tex_id: None,
        }
    }

    fn maybe_load_photo_texture() -> Option<dear_imgui_rs::texture::OwnedTextureData> {
        // Prefer the shared gradient test image; fall back to the original JPEG
        // asset to keep behavior reasonable if the gradient is missing.
        let asset_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
        let candidates = [
            asset_dir.join("texture_clean.ppm"),
            asset_dir.join("texture.jpg"),
        ];

        let path = match candidates.iter().find(|p| p.exists()) {
            Some(p) => p.clone(),
            None => {
                eprintln!(
                    "No demo image found in {:?}. Current dir: {:?}",
                    asset_dir,
                    std::env::current_dir().ok()
                );
                return None;
            }
        };

        match ImageReader::open(&path)
            .and_then(|r| r.with_guessed_format())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
        {
            Ok(mut r) => match r.decode() {
                Ok(img) => {
                    let rgba = img.to_rgba8();
                    let (w, h) = rgba.dimensions();
                    let data = rgba.into_raw();
                    let mut t = dear_imgui_rs::texture::TextureData::new();
                    t.create(
                        dear_imgui_rs::texture::TextureFormat::RGBA32,
                        w as i32,
                        h as i32,
                    );
                    t.set_data(&data);
                    println!("Loaded demo image from {:?} ({}x{})", path, w, h);
                    Some(t)
                }
                Err(e) => {
                    eprintln!("Failed to decode demo image {:?}: {e}", path);
                    None
                }
            },
            Err(e) => {
                eprintln!("Failed to open demo image {:?}: {e}", path);
                None
            }
        }
    }

    fn update_anim_tex(&mut self) {
        let (w, h) = self.tex_size;
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        let t = self.frame as f32 * 0.08;
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                let fx = x as f32 / w as f32;
                let fy = y as f32 / h as f32;
                pixels[i + 0] = ((fx * 255.0 + t.sin() * 128.0).clamp(0.0, 255.0)) as u8;
                pixels[i + 1] = ((fy * 255.0 + (t * 1.7).cos() * 128.0).clamp(0.0, 255.0)) as u8;
                pixels[i + 2] = (((fx + fy + t * 0.1).sin().abs()) * 255.0) as u8;
                pixels[i + 3] = 255;
            }
        }
        self.img_tex.set_data(&pixels);
        self.frame = self.frame.wrapping_add(1);
    }
}

fn main() {
    dear_imgui_rs::logging::init_tracing_with_filter(
        "dear_imgui=info,dear_app_wgpu_textures=info,wgpu=warn",
    );

    let runner = RunnerConfig {
        window_title: "Dear App - WGPU Textures".to_string(),
        window_size: (1280.0, 720.0),
        present_mode: wgpu::PresentMode::Fifo,
        clear_color: [0.1, 0.12, 0.14, 1.0],
        docking: dear_app::DockingConfig::default(),
        ini_filename: None,
        restore_previous_geometry: true,
        redraw: RedrawMode::Poll,
        io_config_flags: None,
        theme: Some(Theme::Dark),
    };

    let addons_cfg = AddOnsConfig::default();
    let mut state = TexDemoState::new();

    AppBuilder::new()
        .with_config(runner)
        .with_addons(addons_cfg)
        .on_frame(move |ui, addons| {
            // Update delta (informational / pacing)
            let now = Instant::now();
            let dt = now - state.last_ui;
            state.last_ui = now;

            // Update animated texture; for real-time use we can push updates immediately
            state.update_anim_tex();
            let _ = addons.gpu.update_texture_data(&mut state.img_tex);
            if let Some(photo) = state.photo_tex.as_mut() {
                let _ = addons.gpu.update_texture_data(&mut **photo);
            }

            // External GPU texture: create once and update per-frame on GPU
            if state.ext_tex_id.is_none() {
                let device = addons.gpu.device();
                let size = wgpu_rs::Extent3d {
                    width: state.ext_size.0,
                    height: state.ext_size.1,
                    depth_or_array_layers: 1,
                };
                let texture = device.create_texture(&wgpu_rs::TextureDescriptor {
                    label: Some("dear-app-ext-texture"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu_rs::TextureDimension::D2,
                    format: wgpu_rs::TextureFormat::Rgba8Unorm,
                    usage: wgpu_rs::TextureUsages::TEXTURE_BINDING
                        | wgpu_rs::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                let view = texture.create_view(&wgpu_rs::TextureViewDescriptor::default());
                let tex_id = addons.gpu.register_texture(&texture, &view);
                state.ext_tex = Some(texture);
                state.ext_view = Some(view);
                state.ext_tex_id = Some(tex_id);
            }

            // Update external texture pixels (checker animation)
            if let (Some(tex), Some(tex_id)) = (state.ext_tex.as_ref(), state.ext_tex_id) {
                let (w, h) = state.ext_size;
                let bpp = 4u32;
                let mut pixels = vec![0u8; (w * h * bpp) as usize];
                let t = state.frame as f32 * 0.15;
                for y in 0..h {
                    for x in 0..w {
                        let i = ((y * w + x) * 4) as usize;
                        let checker = (((x / 16 + y / 16) % 2) as u8) * 255;
                        pixels[i + 0] =
                            checker.saturating_add(((t.sin() * 64.0) as i32).unsigned_abs() as u8);
                        pixels[i + 1] = 64;
                        pixels[i + 2] = 255 - checker;
                        pixels[i + 3] = 255;
                    }
                }
                let queue = addons.gpu.queue();
                let bytes_per_row = w * bpp;
                let align = wgpu_rs::COPY_BYTES_PER_ROW_ALIGNMENT;
                let padded = bytes_per_row.div_ceil(align) * align;
                if padded == bytes_per_row {
                    queue.write_texture(
                        wgpu_rs::TexelCopyTextureInfo {
                            texture: tex,
                            mip_level: 0,
                            origin: wgpu_rs::Origin3d::ZERO,
                            aspect: wgpu_rs::TextureAspect::All,
                        },
                        &pixels,
                        wgpu_rs::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(bytes_per_row),
                            rows_per_image: Some(h),
                        },
                        wgpu_rs::Extent3d {
                            width: w,
                            height: h,
                            depth_or_array_layers: 1,
                        },
                    );
                } else {
                    // pad rows
                    let mut padded_buf = vec![0u8; (padded * h) as usize];
                    for row in 0..h as usize {
                        let src_off = row * (bytes_per_row as usize);
                        let dst_off = row * (padded as usize);
                        padded_buf[dst_off..dst_off + (bytes_per_row as usize)]
                            .copy_from_slice(&pixels[src_off..src_off + (bytes_per_row as usize)]);
                    }
                    queue.write_texture(
                        wgpu_rs::TexelCopyTextureInfo {
                            texture: tex,
                            mip_level: 0,
                            origin: wgpu_rs::Origin3d::ZERO,
                            aspect: wgpu_rs::TextureAspect::All,
                        },
                        &padded_buf,
                        wgpu_rs::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(padded),
                            rows_per_image: Some(h),
                        },
                        wgpu_rs::Extent3d {
                            width: w,
                            height: h,
                            depth_or_array_layers: 1,
                        },
                    );
                }
                // No API call needed after upload; renderer uses legacy TextureId path immediately
                let _ = tex_id; // silence unused
            }

            ui.window("dear-app WGPU Texture Demo")
                .size([560.0, 520.0], Condition::FirstUseEver)
                .build(|| {
                    ui.text(format!(
                        "Frame: {}  dt: {:.3} ms",
                        state.frame,
                        dt.as_secs_f64() * 1000.0
                    ));
                    ui.separator();
                    ui.text("Animated Texture (CPU -> backend -> GPU):");
                    Image::new(ui, &mut *state.img_tex, [256.0, 256.0]).build();

                    if let Some(photo) = state.photo_tex.as_mut() {
                        ui.separator();
                        ui.text("Loaded Image:");
                        let w = photo.width() as f32;
                        let h = photo.height() as f32;
                        let max_dim = 256.0;
                        let scale = if w > h { max_dim / w } else { max_dim / h };
                        Image::new(ui, &mut **photo, [w * scale, h * scale]).build();
                    } else {
                        ui.separator();
                        ui.text_wrapped(
                            "Place examples/assets/texture.jpg to preview a real image.",
                        );
                    }

                    ui.separator();
                    ui.text("External GPU Texture (legacy TextureId path):");
                    if let Some(tex_id) = state.ext_tex_id {
                        let tid = TextureId::from(tex_id);
                        ui.image(tid, [256.0, 256.0]);
                    }
                    ui.separator();
                    if ui.button("Reload Image") {
                        state.photo_tex = TexDemoState::maybe_load_photo_texture();
                    }
                });
        })
        .run()
        .unwrap();
}
