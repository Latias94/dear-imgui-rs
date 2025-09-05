use dear_imgui::*;
use dear_imgui_wgpu::Renderer;
use dear_imgui_winit::WinitPlatform;
use std::time::Instant;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    env_logger::init();

    // Create event loop and window
    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new()
        .with_title("Dear ImGui - Color, Image & Plot Demo")
        .with_inner_size(winit::dpi::LogicalSize::new(1024, 768))
        .build(&event_loop)
        .unwrap();

    // Initialize wgpu
    let instance = wgpu::Instance::default();
    let surface = instance.create_surface(&window).unwrap();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }))
    .unwrap();

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
        },
        None,
    ))
    .unwrap();

    let surface_desc = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface.get_capabilities(&adapter).formats[0],
        width: 1024,
        height: 768,
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &surface_desc);

    // Initialize Dear ImGui
    let mut imgui = Context::create();
    let mut platform = WinitPlatform::init(&mut imgui);
    platform.attach_window(
        imgui.io_mut(),
        &window,
        dear_imgui_winit::HiDpiMode::Default,
    );

    let mut renderer = Renderer::new(&mut imgui, &device, &queue, surface_desc.format);

    // Demo state
    let mut color3 = [1.0, 0.5, 0.2];
    let mut color4 = [0.4, 0.7, 0.0, 0.5];
    let mut plot_values = vec![0.6, 0.1, 1.0, 0.5, 0.92, 0.1, 0.2];
    let mut histogram_values = vec![0.2, 0.1, 1.0, 0.5, 0.92, 0.1, 0.2, 0.8, 0.3];

    // Create a simple texture for demo
    let texture_data = create_demo_texture(&device, &queue);
    let texture_id = ImageTextureId::new(texture_data.as_raw() as usize);

    let start_time = Instant::now();
    let mut last_frame = Instant::now();

    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll);

            platform.handle_event(imgui.io_mut(), &window, &event);

            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => elwt.exit(),
                Event::WindowEvent {
                    event: WindowEvent::Resized(size),
                    ..
                } => {
                    if size.width > 0 && size.height > 0 {
                        surface_desc.width = size.width;
                        surface_desc.height = size.height;
                        surface.configure(&device, &surface_desc);
                    }
                }
                Event::AboutToWait => {
                    let now = Instant::now();
                    let delta_time = now - last_frame;
                    last_frame = now;
                    let elapsed = start_time.elapsed().as_secs_f32();

                    // Update animated values
                    for (i, val) in plot_values.iter_mut().enumerate() {
                        *val = 0.5 + 0.5 * (elapsed + i as f32 * 0.5).sin();
                    }

                    platform.prepare_frame(imgui.io_mut(), &window).unwrap();

                    let ui = imgui.frame();

                    // Main demo window
                    if ui.begin("Color, Image & Plot Demo") {
                        ui.text("Color Edit Widgets:");
                        ui.separator();

                        // Color edit widgets
                        if ui.color_edit3("Color RGB", &mut color3) {
                            println!("Color3 changed: {:?}", color3);
                        }

                        if ui.color_edit4("Color RGBA", &mut color4) {
                            println!("Color4 changed: {:?}", color4);
                        }

                        // Color picker
                        ui.text("Color Picker:");
                        ui.color_picker3("Pick Color", &mut color3);

                        // Color button
                        if ui.color_button("color_btn", color4) {
                            println!("Color button clicked!");
                        }
                        ui.same_line();
                        ui.text("Click the color button");

                        ui.separator();
                        ui.text("Image Widgets:");

                        // Image widget
                        ui.text("Demo Texture:");
                        ui.image(texture_id, [100.0, 100.0]);

                        // Image button
                        if ui.image_button("img_btn", texture_id, [50.0, 50.0]) {
                            println!("Image button clicked!");
                        }
                        ui.same_line();
                        ui.text("Click the image button");

                        ui.separator();
                        ui.text("Plot Widgets:");

                        // Plot lines
                        ui.plot_lines("Animated Plot", &plot_values);

                        // Plot histogram
                        ui.plot_histogram("Histogram", &histogram_values);

                        // Advanced color edit with flags
                        ui.separator();
                        ui.text("Advanced Color Controls:");

                        ui.color_edit4_config("Advanced Color", &mut color4)
                            .flags(ColorEditFlags::ALPHA_BAR | ColorEditFlags::DISPLAY_HSV)
                            .build();

                        // Advanced plot with custom settings
                        ui.plot_lines_config("Custom Plot", &plot_values)
                            .scale_min(0.0)
                            .scale_max(1.0)
                            .graph_size([200.0, 100.0])
                            .overlay_text("Custom overlay")
                            .build();
                    }
                    ui.end();

                    // Render
                    let output = surface.get_current_texture().unwrap();
                    let view = output
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());

                    let mut encoder =
                        device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Render Encoder"),
                        });

                    {
                        let mut render_pass =
                            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("Render Pass"),
                                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                    view: &view,
                                    resolve_target: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(wgpu::Color {
                                            r: 0.1,
                                            g: 0.2,
                                            b: 0.3,
                                            a: 1.0,
                                        }),
                                        store: wgpu::StoreOp::Store,
                                    },
                                })],
                                depth_stencil_attachment: None,
                                timestamp_writes: None,
                                occlusion_query_set: None,
                            });

                        renderer
                            .render(ui.render(), &queue, &device, &mut render_pass)
                            .unwrap();
                    }

                    queue.submit(std::iter::once(encoder.finish()));
                    output.present();

                    window.request_redraw();
                }
                _ => {}
            }
        })
        .unwrap();
}

fn create_demo_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::Texture {
    // Create a simple 64x64 checkerboard texture for demo
    let size = 64u32;
    let mut data = vec![0u8; (size * size * 4) as usize];

    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;
            let checker = ((x / 8) + (y / 8)) % 2 == 0;
            if checker {
                data[idx] = 255; // R
                data[idx + 1] = 255; // G
                data[idx + 2] = 255; // B
            } else {
                data[idx] = 128; // R
                data[idx + 1] = 128; // G
                data[idx + 2] = 128; // B
            }
            data[idx + 3] = 255; // A
        }
    }

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Demo Texture"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &data,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * size),
            rows_per_image: Some(size),
        },
        wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
    );

    texture
}
