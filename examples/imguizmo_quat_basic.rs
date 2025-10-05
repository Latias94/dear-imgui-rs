//! ImGuIZMO.quat basic demo (WGPU backend)
//!
//! Run:
//!   cargo run -p dear-imgui-examples --features dear-imguizmo-quat --bin imguizmo_quat_basic

use dear_imgui_rs::*;
use dear_imgui_wgpu::WgpuRenderer;
use dear_imgui_winit::WinitPlatform;
use dear_imguizmo_quat::{GizmoQuatExt, Mode, Modifiers};
use pollster::block_on;
use std::{sync::Arc, time::Instant};
use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

struct ImguiState {
    context: Context,
    platform: WinitPlatform,
    renderer: WgpuRenderer,
    last_frame: Instant,
}

// --- Scene data: cube vertex (Position, Normal, Color) ---
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct VertexPNC {
    pos: [f32; 4],
    nrm: [f32; 4],
    col: [f32; 4],
}

impl VertexPNC {
    fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem::size_of;
        wgpu::VertexBufferLayout {
            array_stride: size_of::<VertexPNC>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

const fn v(pos: [f32; 4], nrm: [f32; 4], col: [f32; 4]) -> VertexPNC {
    VertexPNC { pos, nrm, col }
}

// Cube vertices copied from upstream examples (commons/assets/cubePNC.h)
const CUBE_PNC: [VertexPNC; 36] = [
    // blue face
    v(
        [-1.0, -1.0, 1.0, 1.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0, 1.0],
    ),
    v(
        [-1.0, 1.0, 1.0, 1.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0, 1.0],
    ),
    v(
        [1.0, -1.0, 1.0, 1.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0, 1.0],
    ),
    v(
        [1.0, -1.0, 1.0, 1.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0, 1.0],
    ),
    v(
        [-1.0, 1.0, 1.0, 1.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0, 1.0],
    ),
    v(
        [1.0, 1.0, 1.0, 1.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0, 1.0],
    ),
    // light blue
    v(
        [-1.0, -1.0, -1.0, 1.0],
        [0.0, 0.0, -1.0, 0.0],
        [0.0, 0.5, 0.5, 1.0],
    ),
    v(
        [1.0, -1.0, -1.0, 1.0],
        [0.0, 0.0, -1.0, 0.0],
        [0.0, 0.5, 0.5, 1.0],
    ),
    v(
        [-1.0, 1.0, -1.0, 1.0],
        [0.0, 0.0, -1.0, 0.0],
        [0.0, 0.5, 0.5, 1.0],
    ),
    v(
        [-1.0, 1.0, -1.0, 1.0],
        [0.0, 0.0, -1.0, 0.0],
        [0.0, 0.5, 0.5, 1.0],
    ),
    v(
        [1.0, -1.0, -1.0, 1.0],
        [0.0, 0.0, -1.0, 0.0],
        [0.0, 0.5, 0.5, 1.0],
    ),
    v(
        [1.0, 1.0, -1.0, 1.0],
        [0.0, 0.0, -1.0, 0.0],
        [0.0, 0.5, 0.5, 1.0],
    ),
    // light red
    v(
        [-1.0, 1.0, 1.0, 1.0],
        [-1.0, 0.0, 0.0, 0.0],
        [0.5, 0.0, 0.5, 1.0],
    ),
    v(
        [-1.0, -1.0, 1.0, 1.0],
        [-1.0, 0.0, 0.0, 0.0],
        [0.5, 0.0, 0.5, 1.0],
    ),
    v(
        [-1.0, 1.0, -1.0, 1.0],
        [-1.0, 0.0, 0.0, 0.0],
        [0.5, 0.0, 0.5, 1.0],
    ),
    v(
        [-1.0, 1.0, -1.0, 1.0],
        [-1.0, 0.0, 0.0, 0.0],
        [0.5, 0.0, 0.5, 1.0],
    ),
    v(
        [-1.0, -1.0, 1.0, 1.0],
        [-1.0, 0.0, 0.0, 0.0],
        [0.5, 0.0, 0.5, 1.0],
    ),
    v(
        [-1.0, -1.0, -1.0, 1.0],
        [-1.0, 0.0, 0.0, 0.0],
        [0.5, 0.0, 0.5, 1.0],
    ),
    // red
    v(
        [1.0, 1.0, 1.0, 1.0],
        [1.0, 0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0, 1.0],
    ),
    v(
        [1.0, 1.0, -1.0, 1.0],
        [1.0, 0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0, 1.0],
    ),
    v(
        [1.0, -1.0, 1.0, 1.0],
        [1.0, 0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0, 1.0],
    ),
    v(
        [1.0, -1.0, 1.0, 1.0],
        [1.0, 0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0, 1.0],
    ),
    v(
        [1.0, 1.0, -1.0, 1.0],
        [1.0, 0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0, 1.0],
    ),
    v(
        [1.0, -1.0, -1.0, 1.0],
        [1.0, 0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0, 1.0],
    ),
    // green
    v(
        [1.0, 1.0, 1.0, 1.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 1.0],
    ),
    v(
        [-1.0, 1.0, 1.0, 1.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 1.0],
    ),
    v(
        [1.0, 1.0, -1.0, 1.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 1.0],
    ),
    v(
        [1.0, 1.0, -1.0, 1.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 1.0],
    ),
    v(
        [-1.0, 1.0, 1.0, 1.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 1.0],
    ),
    v(
        [-1.0, 1.0, -1.0, 1.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 1.0],
    ),
    // light green
    v(
        [1.0, -1.0, 1.0, 1.0],
        [0.0, -1.0, 0.0, 0.0],
        [0.5, 0.5, 0.0, 1.0],
    ),
    v(
        [1.0, -1.0, -1.0, 1.0],
        [0.0, -1.0, 0.0, 0.0],
        [0.5, 0.5, 0.0, 1.0],
    ),
    v(
        [-1.0, -1.0, 1.0, 1.0],
        [0.0, -1.0, 0.0, 0.0],
        [0.5, 0.5, 0.0, 1.0],
    ),
    v(
        [-1.0, -1.0, 1.0, 1.0],
        [0.0, -1.0, 0.0, 0.0],
        [0.5, 0.5, 0.0, 1.0],
    ),
    v(
        [1.0, -1.0, -1.0, 1.0],
        [0.0, -1.0, 0.0, 0.0],
        [0.5, 0.5, 0.0, 1.0],
    ),
    v(
        [-1.0, -1.0, -1.0, 1.0],
        [0.0, -1.0, 0.0, 0.0],
        [0.5, 0.5, 0.0, 1.0],
    ),
];

// Uniform buffer layout matching upstream cube_light.wgsl (with vec3 padded to 4 floats)
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SceneUbo {
    p_mat: [[f32; 4]; 4],
    v_mat: [[f32; 4]; 4],
    m_mat: [[f32; 4]; 4],
    c_mat: [[f32; 4]; 4],
    l_mat: [[f32; 4]; 4],
    light_p: [f32; 4], // vec3 padded
    pov: [f32; 4],     // vec3 padded
}

// Upstream WGSL is wrapped into a C++ R"(...)" block; we sanitize to extract raw WGSL.
const UPSTREAM_WGSL_RAW: &str =
    include_str!("../repo-ref/imGuIZMO.quat/commons/shaders/cube_light.wgsl");

fn sanitize_cpp_wrapped_wgsl(src: &str) -> String {
    // Take contents between first occurrence of R"( and the trailing )"
    if let Some(start) = src.find("R\"(") {
        if let Some(end) = src.rfind(")\"") {
            let inner = &src[start + 3..end];
            return inner.to_string();
        }
    }
    src.to_string()
}

struct OffscreenRtt {
    size: (u32, u32),
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    depth: wgpu::Texture,
    depth_view: wgpu::TextureView,
    texture_id: dear_imgui_rs::TextureId,
}

impl OffscreenRtt {
    fn create(
        device: &wgpu::Device,
        renderer: &mut WgpuRenderer,
        size: (u32, u32),
        format: wgpu::TextureFormat,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("quat-rtt"),
            size: wgpu::Extent3d {
                width: size.0.max(1),
                height: size.1.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("quat-rtt-depth"),
            size: wgpu::Extent3d {
                width: size.0.max(1),
                height: size.1.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());

        let id64 =
            renderer
                .texture_manager_mut()
                .register_texture(dear_imgui_wgpu::WgpuTexture::new(
                    texture.clone(),
                    view.clone(),
                ));
        let texture_id = dear_imgui_rs::TextureId::from(id64);

        Self {
            size,
            texture,
            view,
            depth,
            depth_view,
            texture_id,
        }
    }

    fn recreate(
        &mut self,
        device: &wgpu::Device,
        renderer: &mut WgpuRenderer,
        size: (u32, u32),
        format: wgpu::TextureFormat,
    ) {
        renderer
            .texture_manager_mut()
            .destroy_texture(self.texture_id);
        *self = Self::create(device, renderer, size, format);
    }
}

struct ScenePipeline {
    pipeline: wgpu::RenderPipeline,
    bind_layout: wgpu::BindGroupLayout,
    ubo: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    vbuf: wgpu::Buffer,
    vcount: u32,
}

fn create_scene_pipeline(device: &wgpu::Device, format: wgpu::TextureFormat) -> ScenePipeline {
    let wgsl = sanitize_cpp_wrapped_wgsl(UPSTREAM_WGSL_RAW);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("quat-scene-shader"),
        source: wgpu::ShaderSource::Wgsl(wgsl.into()),
    });

    let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("quat-scene-bind-layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<SceneUbo>() as u64),
            },
            count: None,
        }],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("quat-scene-pipeline-layout"),
        bind_group_layouts: &[&bind_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("quat-scene-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs"),
            buffers: &[VertexPNC::layout()],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        multiview: None,
        cache: None,
    });

    let ubo = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("quat-scene-ubo"),
        size: std::mem::size_of::<SceneUbo>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("quat-scene-bind-group"),
        layout: &bind_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &ubo,
                offset: 0,
                size: None,
            }),
        }],
    });

    let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("quat-cube-vbuf"),
        contents: bytemuck::cast_slice(&CUBE_PNC),
        usage: wgpu::BufferUsages::VERTEX,
    });

    ScenePipeline {
        pipeline,
        bind_layout,
        ubo,
        bind_group,
        vbuf,
        vcount: CUBE_PNC.len() as u32,
    }
}
struct AppWindow {
    device: wgpu::Device,
    queue: wgpu::Queue,
    window: Arc<Window>,
    surface_desc: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    imgui: ImguiState,
    // Offscreen WGPU scene
    rtt: OffscreenRtt,
    scene: ScenePipeline,
    // Gizmo state
    rot: glam::Quat,
    light_dir: glam::Vec3,
    pan_dolly: glam::Vec3,
    size: f32,
    use_pan_dolly: bool,
    mode: Mode,
    // Style/config controls
    dir_color: [f32; 4],
    plane_color: [f32; 4],
    sphere_a: [f32; 4],
    sphere_b: [f32; 4],
    feeling_rot: f32,
    pan_scale: f32,
    dolly_scale: f32,
    dolly_wheel_scale: f32,
    pan_mod: Modifiers,
    dolly_mod: Modifiers,
    // Geometry
    axes_len: f32,
    axes_thickness: f32,
    cone_thickness: f32,
    solid_size: f32,
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

        let window = {
            let size = LogicalSize::new(1280.0, 720.0);
            Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title("ImGuIZMO.quat - WGPU Demo")
                        .with_inner_size(size),
                )?,
            )
        };

        let surface = instance.create_surface(window.clone())?;
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("No suitable GPU adapters found on the system!");

        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;

        let caps = surface.get_capabilities(&adapter);
        let preferred_srgb = [
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ];
        let format = preferred_srgb
            .iter()
            .cloned()
            .find(|f| caps.formats.contains(f))
            .unwrap_or(caps.formats[0]);

        let size = window.inner_size();
        let surface_desc = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_desc);

        // ImGui context
        let mut context = Context::create();
        context.set_ini_filename(None::<String>).unwrap();
        // Enable Docking (so our windows can dock into a DockSpace)
        {
            let io = context.io_mut();
            let mut flags = io.config_flags();
            flags.insert(ConfigFlags::DOCKING_ENABLE);
            io.set_config_flags(flags);
        }
        let mut platform = WinitPlatform::new(&mut context);
        platform.attach_window(&window, dear_imgui_winit::HiDpiMode::Default, &mut context);

        // Renderer
        let init_info =
            dear_imgui_wgpu::WgpuInitInfo::new(device.clone(), queue.clone(), surface_desc.format);
        let mut renderer = WgpuRenderer::new(init_info, &mut context)?;
        renderer.set_gamma_mode(dear_imgui_wgpu::GammaMode::Auto);

        // Offscreen scene resources
        let rtt = OffscreenRtt::create(&device, &mut renderer, (960, 540), surface_desc.format);
        let scene = create_scene_pipeline(&device, surface_desc.format);

        let imgui = ImguiState {
            context,
            platform,
            renderer,
            last_frame: Instant::now(),
        };

        Ok(Self {
            device,
            queue,
            window,
            surface_desc,
            surface,
            imgui,
            rtt,
            scene,
            rot: glam::Quat::IDENTITY,
            light_dir: glam::Vec3::new(1.0, 0.0, 0.0),
            pan_dolly: glam::Vec3::ZERO,
            size: 220.0,
            use_pan_dolly: false,
            mode: Mode::MODE_DUAL | Mode::CUBE_AT_ORIGIN,
            dir_color: [0.9, 0.2, 0.2, 1.0],
            plane_color: [0.9, 0.9, 0.9, 0.75],
            sphere_a: [0.2, 0.6, 1.0, 1.0],
            sphere_b: [0.8, 0.8, 0.2, 1.0],
            feeling_rot: 1.0,
            pan_scale: 1.0,
            dolly_scale: 1.0,
            dolly_wheel_scale: 1.0,
            pan_mod: Modifiers::CONTROL,
            dolly_mod: Modifiers::SHIFT,
            axes_len: 1.0,
            axes_thickness: 1.0,
            cone_thickness: 1.0,
            solid_size: 1.0,
        })
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.surface_desc.width = new_size.width;
            self.surface_desc.height = new_size.height;
            self.surface.configure(&self.device, &self.surface_desc);
            // Resize offscreen viewport to ~75% of window
            let tgt = (
                (new_size.width as f32 * 0.75) as u32,
                (new_size.height as f32 * 0.75) as u32,
            );
            self.rtt.recreate(
                &self.device,
                &mut self.imgui.renderer,
                tgt,
                self.surface_desc.format,
            );
        }
    }

    fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let now = Instant::now();
        let delta_s = (now - self.imgui.last_frame).as_secs_f32();
        self.imgui.context.io_mut().set_delta_time(delta_s);
        self.imgui.last_frame = now;

        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.surface_desc);
                return Ok(());
            }
            Err(wgpu::SurfaceError::Timeout) => {
                return Ok(());
            }
            Err(e) => return Err(Box::new(e)),
        };

        self.imgui
            .platform
            .prepare_frame(&self.window, &mut self.imgui.context);
        // Work on local copies during the UI frame to avoid borrowing self while Ui is alive
        let mut rot = self.rot;
        let mut light_dir = self.light_dir;
        let mut pan_dolly = self.pan_dolly;
        let mut size = self.size;
        let mut use_pan_dolly = self.use_pan_dolly;
        let mut mode = self.mode;
        let mut dir_color = self.dir_color;
        let mut plane_color = self.plane_color;
        let mut sphere_a = self.sphere_a;
        let mut sphere_b = self.sphere_b;
        let mut feeling_rot = self.feeling_rot;
        let mut pan_scale = self.pan_scale;
        let mut dolly_scale = self.dolly_scale;
        let mut dolly_wheel_scale = self.dolly_wheel_scale;
        let ui = self.imgui.context.frame();
        // Create a DockSpace over main viewport so windows can be docked properly
        let dockspace_id = ui.dockspace_over_main_viewport();
        // Prefer to dock the main controls window on first use
        ui.set_next_window_dock_id_with_cond(dockspace_id, Condition::FirstUseEver);
        show_quat_gizmo_window(
            &ui,
            &mut rot,
            &mut light_dir,
            &mut pan_dolly,
            &mut size,
            &mut use_pan_dolly,
            &mut mode,
            &mut dir_color,
            &mut plane_color,
            &mut sphere_a,
            &mut sphere_b,
            &mut feeling_rot,
            &mut pan_scale,
            &mut dolly_scale,
            &mut dolly_wheel_scale,
            &mut self.pan_mod,
            &mut self.dolly_mod,
            &mut self.axes_len,
            &mut self.axes_thickness,
            &mut self.cone_thickness,
            &mut self.solid_size,
        );

        // Show the offscreen texture inside an ImGui window (resizable)
        ui.set_next_window_dock_id_with_cond(dockspace_id, Condition::FirstUseEver);
        ui.window("Scene Preview##imguizmo_quat_preview")
            .size([640.0, 380.0], Condition::FirstUseEver)
            .build(|| {
                let avail = ui.content_region_avail();
                let w = avail[0].max(100.0);
                let h = (w / (self.rtt.size.0 as f32 / self.rtt.size.1 as f32))
                    .min(avail[1].max(100.0));
                ui.image(self.rtt.texture_id, [w, h]);
            });

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        self.imgui
            .platform
            .prepare_render_with_ui(&ui, &self.window);
        let draw_data = self.imgui.context.render();
        // Persist state after the UI frame ends
        self.rot = rot;
        self.light_dir = light_dir;
        self.pan_dolly = pan_dolly;
        self.size = size;
        self.use_pan_dolly = use_pan_dolly;
        self.mode = mode;
        self.dir_color = dir_color;
        self.plane_color = plane_color;
        self.sphere_a = sphere_a;
        self.sphere_b = sphere_b;
        self.feeling_rot = feeling_rot;
        self.pan_scale = pan_scale;
        self.dolly_scale = dolly_scale;
        self.dolly_wheel_scale = dolly_wheel_scale;
        // geometry already persisted via &mut self.* in call

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.07,
                            g: 0.08,
                            b: 0.09,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.imgui.renderer.new_frame()?;
            self.imgui
                .renderer
                .render_draw_data(draw_data, &mut rpass)?;
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();

        // Update offscreen scene after presenting (applies next frame)
        self.render_scene_to_offscreen(&rot, &light_dir, &pan_dolly);

        Ok(())
    }
}

fn show_quat_gizmo_window(
    ui: &Ui,
    rot: &mut glam::Quat,
    light_dir: &mut glam::Vec3,
    pan_dolly: &mut glam::Vec3,
    size: &mut f32,
    use_pan_dolly: &mut bool,
    mode: &mut Mode,
    dir_color: &mut [f32; 4],
    plane_color: &mut [f32; 4],
    sphere_a: &mut [f32; 4],
    sphere_b: &mut [f32; 4],
    feeling_rot: &mut f32,
    pan_scale: &mut f32,
    dolly_scale: &mut f32,
    dolly_wheel_scale: &mut f32,
    pan_mod: &mut Modifiers,
    dolly_mod: &mut Modifiers,
    axes_len: &mut f32,
    axes_thickness: &mut f32,
    cone_thickness: &mut f32,
    solid_size: &mut f32,
) {
    ui.window("ImGuIZMO.quat Basic##imguizmo_quat_basic")
        .size([430.0, 500.0], Condition::FirstUseEver)
        .build(|| {
            ui.text("Quaternion + 3D gizmo (ImGuIZMO.quat)");
            ui.separator();

            // Mode toggles
            let id_modes = ui.push_id("modes");
            let mut m3 = mode.contains(Mode::MODE_3_AXES);
            let mut md = mode.contains(Mode::MODE_DIRECTION);
            let mut mp = mode.contains(Mode::MODE_DIR_PLANE);
            let mut mdul = mode.contains(Mode::MODE_DUAL);
            let mut mpd = mode.contains(Mode::MODE_PAN_DOLLY);
            if ui.checkbox("3 Axes", &mut m3) {}
            ui.same_line();
            if ui.checkbox("Direction", &mut md) {}
            ui.same_line();
            if ui.checkbox("Dir Plane", &mut mp) {}
            ui.same_line();
            if ui.checkbox("Dual", &mut mdul) {}
            if ui.checkbox("Pan+Dolly", &mut mpd) {}
            id_modes.pop();

            let id_solids = ui.push_id("solids");
            let mut cube = mode.contains(Mode::CUBE_AT_ORIGIN);
            let mut sphere = mode.contains(Mode::SPHERE_AT_ORIGIN);
            let mut no_solid = mode.contains(Mode::NO_SOLID_AT_ORIGIN);
            let mut full_axes = mode.contains(Mode::MODE_FULL_AXES);
            if ui.checkbox("Cube", &mut cube) {}
            ui.same_line();
            if ui.checkbox("Sphere", &mut sphere) {}
            ui.same_line();
            if ui.checkbox("No Solid", &mut no_solid) {}
            ui.same_line();
            if ui.checkbox("Full Axes", &mut full_axes) {}
            id_solids.pop();

            *mode = Mode::empty();
            if m3 {
                *mode |= Mode::MODE_3_AXES;
            }
            if md {
                *mode |= Mode::MODE_DIRECTION;
            }
            if mp {
                *mode |= Mode::MODE_DIR_PLANE;
            }
            if mdul {
                *mode |= Mode::MODE_DUAL;
            }
            if mpd {
                *mode |= Mode::MODE_PAN_DOLLY;
                *use_pan_dolly = true;
            } else {
                *use_pan_dolly = false;
            }
            if cube {
                *mode |= Mode::CUBE_AT_ORIGIN;
            }
            if sphere {
                *mode |= Mode::SPHERE_AT_ORIGIN;
            }
            if no_solid {
                *mode |= Mode::NO_SOLID_AT_ORIGIN;
            }
            if full_axes {
                *mode |= Mode::MODE_FULL_AXES;
            }

            ui.separator();
            ui.text("Presets:");
            let id_presets = ui.push_id("presets");
            if ui.button("Dual + Cube") {
                *mode = Mode::MODE_DUAL | Mode::CUBE_AT_ORIGIN;
                *use_pan_dolly = false;
            }
            ui.same_line();
            if ui.button("Pan+Dolly") {
                *mode = Mode::MODE_PAN_DOLLY | Mode::MODE_DUAL | Mode::CUBE_AT_ORIGIN;
                *use_pan_dolly = true;
            }
            ui.same_line();
            if ui.button("3Axes + NoSolid") {
                *mode = Mode::MODE_3_AXES | Mode::NO_SOLID_AT_ORIGIN;
                *use_pan_dolly = false;
            }
            ui.same_line();
            if ui.button("Direction + Sphere") {
                *mode = Mode::MODE_DIRECTION | Mode::SPHERE_AT_ORIGIN;
                *use_pan_dolly = false;
            }
            id_presets.pop();

            ui.separator();
            ui.slider("Gizmo size", 100.0, 400.0, size);

            // Show values
            let (x, y, z, w) = (rot.x, rot.y, rot.z, rot.w);
            ui.text(format!("Quat: [{:.3}, {:.3}, {:.3}, {:.3}]", x, y, z, w));
            ui.text(format!(
                "Light dir: [{:.3}, {:.3}, {:.3}]",
                light_dir.x, light_dir.y, light_dir.z
            ));
            if *use_pan_dolly {
                ui.text(format!(
                    "Pan/Dolly: [{:.3}, {:.3}, {:.3}]",
                    pan_dolly.x, pan_dolly.y, pan_dolly.z
                ));
            }

            ui.same_line();
            if ui.button("Reset Rot") {
                *rot = glam::Quat::IDENTITY;
            }
            ui.same_line();
            if ui.button("Reset Light") {
                *light_dir = glam::Vec3::new(1.0, 0.0, 0.0);
            }
            ui.same_line();
            if ui.button("Reset Pan") {
                *pan_dolly = glam::Vec3::ZERO;
            }
            ui.same_line();
            if ui.button("Reset All") {
                *rot = glam::Quat::IDENTITY;
                *light_dir = glam::Vec3::new(1.0, 0.0, 0.0);
                *pan_dolly = glam::Vec3::ZERO;
            }

            // Style & tuning (collapsing)
            if ui.collapsing_header("Style & Tuning##style", TreeNodeFlags::empty()) {
                let id_style = ui.push_id("style_tuning");
                if ui.color_edit4("Direction Color", dir_color)
                    || ui.color_edit4("Plane Color", plane_color)
                {
                    ui.gizmo_quat()
                        .set_direction_colors_vec4(*dir_color, *plane_color);
                }
                ui.same_line();
                if ui.button("Restore Dir/Plane") {
                    ui.gizmo_quat().restore_direction_color();
                }
                if ui.color_edit4("Sphere A", sphere_a) || ui.color_edit4("Sphere B", sphere_b) {
                    ui.gizmo_quat().set_sphere_colors_vec4(*sphere_a, *sphere_b);
                }
                ui.same_line();
                if ui.button("Restore Sphere") {
                    ui.gizmo_quat().restore_sphere_colors_u32();
                }

                ui.separator();
                ui.slider("Feeling Rot", 0.2, 3.0, feeling_rot);
                ui.slider("Pan Scale", 0.1, 5.0, pan_scale);
                ui.slider("Dolly Scale", 0.1, 5.0, dolly_scale);
                ui.slider("Dolly Wheel", 0.1, 5.0, dolly_wheel_scale);
                ui.gizmo_quat().set_gizmo_feeling_rot(*feeling_rot);
                ui.gizmo_quat().set_pan_scale(*pan_scale);
                ui.gizmo_quat().set_dolly_scale(*dolly_scale);
                ui.gizmo_quat().set_dolly_wheel_scale(*dolly_wheel_scale);

                ui.separator();
                // Flip / Reverse toggles
                let mut flip_x = ui.gizmo_quat().is_flip_rot_on_x();
                let mut flip_y = ui.gizmo_quat().is_flip_rot_on_y();
                let mut flip_z = ui.gizmo_quat().is_flip_rot_on_z();
                if ui.checkbox("Flip Rot X", &mut flip_x) {
                    ui.gizmo_quat().flip_rot_on_x(flip_x);
                }
                ui.same_line();
                if ui.checkbox("Flip Rot Y", &mut flip_y) {
                    ui.gizmo_quat().flip_rot_on_y(flip_y);
                }
                ui.same_line();
                if ui.checkbox("Flip Rot Z", &mut flip_z) {
                    ui.gizmo_quat().flip_rot_on_z(flip_z);
                }

                let mut flip_panx = ui.gizmo_quat().is_flip_pan_x();
                let mut flip_pany = ui.gizmo_quat().is_flip_pan_y();
                let mut flip_dolly = ui.gizmo_quat().is_flip_dolly();
                if ui.checkbox("Flip Pan X", &mut flip_panx) {
                    ui.gizmo_quat().flip_pan_x(flip_panx);
                }
                ui.same_line();
                if ui.checkbox("Flip Pan Y", &mut flip_pany) {
                    ui.gizmo_quat().flip_pan_y(flip_pany);
                }
                ui.same_line();
                if ui.checkbox("Flip Dolly", &mut flip_dolly) {
                    ui.gizmo_quat().flip_dolly(flip_dolly);
                }

                let mut rx = ui.gizmo_quat().is_reverse_x();
                let mut ry = ui.gizmo_quat().is_reverse_y();
                let mut rz = ui.gizmo_quat().is_reverse_z();
                if ui.checkbox("Reverse X", &mut rx) {
                    ui.gizmo_quat().reverse_x(rx);
                }
                ui.same_line();
                if ui.checkbox("Reverse Y", &mut ry) {
                    ui.gizmo_quat().reverse_y(ry);
                }
                ui.same_line();
                if ui.checkbox("Reverse Z", &mut rz) {
                    ui.gizmo_quat().reverse_z(rz);
                }

                ui.separator();
                ui.text("Geometry (Axes/Solid)");
                let id_geo = ui.push_id("geometry");
                ui.slider("Axes Length", 0.1, 3.0, axes_len);
                ui.slider("Axes Thickness", 0.1, 3.0, axes_thickness);
                ui.slider("Cone Thickness", 0.1, 3.0, cone_thickness);
                if ui.button("Apply Axes Size") {
                    let v = glam::Vec3::new(*axes_len, *axes_thickness, *cone_thickness);
                    ui.gizmo_quat().resize_axes_of(&v);
                }
                ui.same_line();
                if ui.button("Restore Axes Size") {
                    ui.gizmo_quat().restore_axes_size();
                }
                ui.slider("Solid Size", 0.1, 3.0, solid_size);
                ui.same_line();
                if ui.button("Apply Solid Size") {
                    ui.gizmo_quat().resize_solid_of(*solid_size);
                }
                ui.same_line();
                if ui.button("Restore Solid Size") {
                    ui.gizmo_quat().restore_solid_size();
                }
                id_geo.pop();
                id_style.pop();
            }

            // Controls: Modifiers mapping (Pan/Dolly)
            if ui.collapsing_header("Controls##modifiers", TreeNodeFlags::empty()) {
                let id_ctrl = ui.push_id("controls_modifiers");

                // Pan modifier
                let pan_preview = match pan_mod.bits() {
                    x if x == Modifiers::SHIFT.bits() => "Shift",
                    x if x == Modifiers::CONTROL.bits() => "Control",
                    x if x == Modifiers::ALT.bits() => "Alt",
                    x if x == Modifiers::SUPER.bits() => "Super",
                    _ => "None",
                };
                if let Some(_c) = ui.begin_combo("Pan Modifier", pan_preview) {
                    for &(name, val) in &[
                        ("None", Modifiers::NONE),
                        ("Shift", Modifiers::SHIFT),
                        ("Control", Modifiers::CONTROL),
                        ("Alt", Modifiers::ALT),
                        ("Super", Modifiers::SUPER),
                    ] {
                        if ui.selectable(name) {
                            *pan_mod = val;
                        }
                    }
                }

                // Dolly modifier
                let dolly_preview = match dolly_mod.bits() {
                    x if x == Modifiers::SHIFT.bits() => "Shift",
                    x if x == Modifiers::CONTROL.bits() => "Control",
                    x if x == Modifiers::ALT.bits() => "Alt",
                    x if x == Modifiers::SUPER.bits() => "Super",
                    _ => "None",
                };
                if let Some(_c) = ui.begin_combo("Dolly Modifier", dolly_preview) {
                    for &(name, val) in &[
                        ("None", Modifiers::NONE),
                        ("Shift", Modifiers::SHIFT),
                        ("Control", Modifiers::CONTROL),
                        ("Alt", Modifiers::ALT),
                        ("Super", Modifiers::SUPER),
                    ] {
                        if ui.selectable(name) {
                            *dolly_mod = val;
                        }
                    }
                }

                // Apply immediately
                ui.gizmo_quat().set_pan_modifier(*pan_mod);
                ui.gizmo_quat().set_dolly_modifier(*dolly_mod);

                id_ctrl.pop();
            }

            ui.separator();
            // Actual gizmo (builder style)
            let giz = ui.gizmo_quat();
            let builder = giz.builder().size(*size).mode(*mode);
            if *use_pan_dolly {
                let _ =
                    builder.pan_dolly_quat_light_vec3("##gizmo_quat_pd", pan_dolly, rot, light_dir);
            } else {
                let _ = builder.quat_light_vec3("##gizmo_quat", rot, light_dir);
            }
        });
}

// Separate impl block for extra AppWindow methods
impl AppWindow {
    fn render_scene_to_offscreen(
        &mut self,
        rot: &glam::Quat,
        light_dir: &glam::Vec3,
        pan_dolly: &glam::Vec3,
    ) {
        // Camera/view parameters (mirrors upstream example)
        let eye = glam::Vec3::new(12.0, 6.0, 4.0);
        let target = glam::Vec3::ZERO;
        let up = glam::Vec3::new(3.0, 1.0, 0.0).normalize();
        let view = glam::Mat4::look_at_rh(eye, target, up);
        // Match upstream: aspectRatio = height/width; fov = 45deg * aspectRatio; perspective(fov, 1/aspectRatio, ...)
        let aspect_ratio_hw = self.rtt.size.1.max(1) as f32 / self.rtt.size.0.max(1) as f32;
        let fov = std::f32::consts::FRAC_PI_4 * aspect_ratio_hw;
        let proj = glam::Mat4::perspective_rh(fov, 1.0 / aspect_ratio_hw, 0.1, 100.0);

        // Model matrices
        let model_rot = glam::Mat4::from_quat(*rot);
        let model_trans = glam::Mat4::from_translation(*pan_dolly);
        let model = model_trans * model_rot;

        // Compensate view rotation (remove rotation part from view for pan/dolly screen-space feel)
        let qv = glam::Quat::from_mat4(&view);
        let comp = glam::Mat4::from_quat(qv.inverse());

        // Upstream mapping: derive a quaternion from the light vector, then rotate -X axis
        let light_quat = quat_from_vec3_upstream(*light_dir);
        let light_dir_from_quat = light_quat * glam::Vec3::new(-1.0, 0.0, 0.0);
        let light_distance = 3.5f32;
        let light_world =
            model_trans * glam::Mat4::from_translation(light_dir_from_quat * light_distance);
        let light_model = light_world; // upstream scales in shader by 0.1 for light instance

        // Fill uniform
        let lp = (light_world * glam::Vec4::new(0.0, 0.0, 0.0, 1.0)).truncate();
        let u = SceneUbo {
            p_mat: proj.to_cols_array_2d(),
            v_mat: view.to_cols_array_2d(),
            m_mat: model.to_cols_array_2d(),
            c_mat: comp.to_cols_array_2d(),
            l_mat: light_model.to_cols_array_2d(),
            light_p: [lp.x, lp.y, lp.z, 1.0],
            pov: [eye.x, eye.y, eye.z, 1.0],
        };

        // Render to offscreen
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Quat Scene Encoder"),
            });
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Quat Scene Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.rtt.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.02,
                            g: 0.02,
                            b: 0.03,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.rtt.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rpass.set_pipeline(&self.scene.pipeline);
            rpass.set_vertex_buffer(0, self.scene.vbuf.slice(..));

            // Single instanced draw: instance 0 = light cube, 1 = main cube
            self.queue
                .write_buffer(&self.scene.ubo, 0, bytemuck::bytes_of(&u));
            rpass.set_bind_group(0, &self.scene.bind_group, &[]);
            rpass.draw(0..self.scene.vcount, 0..2);
        }
        self.queue.submit(Some(encoder.finish()));
    }
}

// Upstream helper: build quaternion from a vec3 representing light position direction
fn quat_from_vec3_upstream(v: glam::Vec3) -> glam::Quat {
    let len = v.length();
    if len < 1e-6 {
        return glam::Quat::IDENTITY;
    }
    // axis = normalize(vec3(FLT_EPSILON, lPos.z, -lPos.y))
    let axis = glam::Vec3::new(f32::EPSILON, v.z, -v.y).normalize_or_zero();
    // angle = acosf(-lPos.x/length(lPos))
    let x = (-v.x / len).clamp(-1.0, 1.0);
    let angle = x.acos();
    glam::Quat::from_axis_angle(axis, angle)
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            match AppWindow::new(event_loop) {
                Ok(window) => {
                    self.window = Some(window);
                }
                Err(e) => {
                    eprintln!("Failed to create window: {e}");
                    event_loop.exit();
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let window = match self.window.as_mut() {
            Some(window) => window,
            None => return,
        };

        let full_event: winit::event::Event<()> = winit::event::Event::WindowEvent {
            window_id,
            event: event.clone(),
        };
        window
            .imgui
            .platform
            .handle_event(&mut window.imgui.context, &window.window, &full_event);

        match event {
            WindowEvent::Resized(physical_size) => {
                window.resize(physical_size);
                window.window.request_redraw();
            }
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.logical_key == Key::Named(NamedKey::Escape) {
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = window.render() {
                    eprintln!("Render error: {e}");
                }
                window.window.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.window.request_redraw();
        }
    }
}

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
