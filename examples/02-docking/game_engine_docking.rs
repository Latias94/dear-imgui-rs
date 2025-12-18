use dear_imgui_rs::*;
use dear_imgui_wgpu::WgpuRenderer;
use dear_imgui_winit::WinitPlatform;
use pollster::block_on;
use std::{borrow::Cow, num::NonZeroU64, sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::Window,
};

#[cfg(feature = "multi-viewport")]
use dear_imgui_wgpu::multi_viewport as wgpu_mvp;
#[cfg(feature = "multi-viewport")]
use dear_imgui_winit::multi_viewport as winit_mvp;
#[cfg(feature = "multi-viewport")]
use winit::event::Event;

// Optional extensions - only imported if features are enabled
#[cfg(feature = "implot")]
use dear_implot::{LinePlot, Plot, PlotContext, PlotUi};

#[cfg(feature = "imguizmo")]
use dear_imguizmo::{GuizmoExt, Mode, Operation};

use glam::{Mat4, Quat, Vec3, Vec4};

#[derive(Clone)]
struct SceneEntity {
    name: String,
    position: [f32; 3],
    rotation_deg: [f32; 3],
    scale: [f32; 3],
}

impl SceneEntity {
    fn cube(name: impl Into<String>, position: [f32; 3]) -> Self {
        Self {
            name: name.into(),
            position,
            rotation_deg: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
        }
    }

    fn model_matrix(&self) -> Mat4 {
        let translation = Vec3::from_array(self.position);
        let scale = Vec3::from_array(self.scale);
        let rotation = Quat::from_euler(
            glam::EulerRot::XYZ,
            self.rotation_deg[0].to_radians(),
            self.rotation_deg[1].to_radians(),
            self.rotation_deg[2].to_radians(),
        );
        Mat4::from_scale_rotation_translation(scale, rotation, translation)
    }

    #[cfg(feature = "imguizmo")]
    fn set_from_matrix(&mut self, model: Mat4) {
        let (scale, rotation, translation) = model.to_scale_rotation_translation();
        self.position = translation.to_array();
        self.scale = scale.to_array();
        let euler = rotation.to_euler(glam::EulerRot::XYZ);
        self.rotation_deg = [
            euler.0.to_degrees(),
            euler.1.to_degrees(),
            euler.2.to_degrees(),
        ];
    }
}

/// Game engine state with various panels
struct GameEngineState {
    // Scene hierarchy
    selected_entity: Option<usize>,
    entities: Vec<SceneEntity>,

    // Inspector properties
    // Console logs
    console_logs: Vec<String>,
    console_input: dear_imgui_rs::ImString,

    // Asset browser
    current_folder: String,
    assets: Vec<String>,
    asset_search: dear_imgui_rs::ImString,
    project_columns: usize,

    // Viewport settings
    viewport_size: [f32; 2],
    show_wireframe: bool,
    show_grid: bool,

    // Project settings
    project_name: String,
    scene_name: String,

    // Performance stats
    fps: f32,
    frame_time: f32,
    draw_calls: u32,
    vertices: u32,
    fps_history: Vec<(f64, f32)>, // FPS history for graph: (timestamp, fps)

    // UI search/filter state
    hierarchy_search: dear_imgui_rs::ImString,

    // ImGuizmo state (optional)
    #[cfg(feature = "imguizmo")]
    gizmo_operation: Operation,
    #[cfg(feature = "imguizmo")]
    gizmo_mode: Mode,

    // Orbit camera for the Scene View (available in all builds)
    camera_view: Mat4,
    camera_proj: Mat4,
    camera_distance: f32,
    camera_y_angle: f32,
    camera_x_angle: f32,
}

impl Default for GameEngineState {
    fn default() -> Self {
        Self {
            selected_entity: None,
            entities: vec![
                SceneEntity::cube("Cube", [0.0, 0.5, 0.0]),
                SceneEntity::cube("Cube (2)", [1.5, 0.5, 0.0]),
            ],
            console_logs: vec![
                "[INFO] Game engine initialized".to_string(),
                "[INFO] Renderer started".to_string(),
                "[INFO] Scene loaded: MainScene".to_string(),
                "[WARNING] Texture quality reduced for performance".to_string(),
            ],
            console_input: dear_imgui_rs::ImString::new(""),
            current_folder: "Assets/".to_string(),
            assets: vec![
                "Textures/".to_string(),
                "Models/".to_string(),
                "Materials/".to_string(),
                "Scripts/".to_string(),
                "player_texture.png".to_string(),
                "building_model.fbx".to_string(),
                "wood_material.mat".to_string(),
                "player_controller.cs".to_string(),
            ],
            asset_search: dear_imgui_rs::ImString::new(""),
            project_columns: 4,
            viewport_size: [800.0, 600.0],
            show_wireframe: false,
            show_grid: true,
            project_name: "My Game Project".to_string(),
            scene_name: "MainScene".to_string(),
            fps: 60.0,
            frame_time: 16.67,
            draw_calls: 45,
            vertices: 12543,
            fps_history: Vec::new(), // Will be populated during runtime
            hierarchy_search: dear_imgui_rs::ImString::new(""),
            #[cfg(feature = "imguizmo")]
            gizmo_operation: Operation::TRANSLATE,
            #[cfg(feature = "imguizmo")]
            gizmo_mode: Mode::Local,
            camera_view: Mat4::look_at_rh(
                Vec3::new(5.0, 5.0, 5.0), // eye
                Vec3::new(0.0, 0.0, 0.0), // target
                Vec3::new(0.0, 1.0, 0.0), // up
            ),
            camera_proj: Mat4::perspective_rh(
                45.0_f32.to_radians(), // fov
                800.0 / 600.0,         // aspect
                0.1,                   // near
                100.0,                 // far
            ),
            camera_distance: 8.66, // sqrt(5^2 + 5^2 + 5^2)
            camera_y_angle: 45.0_f32.to_radians(),
            camera_x_angle: 35.26_f32.to_radians(), // atan(1/sqrt(2))
        }
    }
}

struct ImguiState {
    context: Context,
    platform: WinitPlatform,
    renderer: WgpuRenderer,
    #[allow(dead_code)] // Only used when the multi-viewport feature is enabled.
    enable_viewports: bool,
    clear_color: wgpu::Color,
    last_frame: Instant,
    game_state: GameEngineState,
    scene_renderer: SimpleSceneRenderer,
    scene_view_rtt: ViewRtt,
    game_view_rtt: ViewRtt,
    dockspace_id: u32,
    first_frame: bool,
    #[cfg(feature = "implot")]
    plot_context: PlotContext,
}

impl Drop for ImguiState {
    fn drop(&mut self) {
        // Avoid ImGui's shutdown assertion by ensuring platform windows are destroyed before the
        // context is dropped.
        #[cfg(feature = "multi-viewport")]
        if self.enable_viewports {
            winit_mvp::shutdown_multi_viewport_support();
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SceneVertex {
    position: [f32; 3],
    normal: [f32; 3],
    color: [f32; 3],
}

impl SceneVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3];

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SceneVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SceneUniform {
    view_proj: [[f32; 4]; 4],
    model: [[f32; 4]; 4],
}

struct SimpleSceneRenderer {
    grid_pipeline: wgpu::RenderPipeline,
    cube_pipeline: wgpu::RenderPipeline,
    grid_vertex_buffer: wgpu::Buffer,
    grid_vertex_count: u32,
    cube_vertex_buffer: wgpu::Buffer,
    cube_index_buffer: wgpu::Buffer,
    cube_index_count: u32,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    uniform_stride: u64,
    max_objects: usize,
}

impl SimpleSceneRenderer {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        use wgpu::util::DeviceExt;

        let depth_format = wgpu::TextureFormat::Depth24Plus;

        let shader_src = r#"
	struct SceneUniform {
	    view_proj: mat4x4<f32>,
	    model: mat4x4<f32>,
	};
@group(0) @binding(0)
var<uniform> u: SceneUniform;

	struct VsIn {
	    @location(0) pos: vec3<f32>,
	    @location(1) normal: vec3<f32>,
	    @location(2) color: vec3<f32>,
	};
	struct VsOut {
	    @builtin(position) pos: vec4<f32>,
	    @location(0) world_normal: vec3<f32>,
	    @location(1) color: vec3<f32>,
	};

	@vertex
	fn vs_main(v: VsIn) -> VsOut {
	    var o: VsOut;
	    let world_pos = u.model * vec4<f32>(v.pos, 1.0);
	    o.pos = u.view_proj * world_pos;
	    o.world_normal = (u.model * vec4<f32>(v.normal, 0.0)).xyz;
	    o.color = v.color;
	    return o;
	}

	@fragment
	fn fs_main(i: VsOut) -> @location(0) vec4<f32> {
	    // Minimal directional lighting for the cube. Grid uses a zero normal to opt out.
	    let n_len2 = dot(i.world_normal, i.world_normal);
	    if (n_len2 < 1e-6) {
	        return vec4<f32>(i.color, 1.0);
	    }

	    let n = normalize(i.world_normal);
	    let light_dir = normalize(vec3<f32>(-0.6, -1.0, -0.3));
	    let ambient = 0.25;
	    let diff = max(dot(n, -light_dir), 0.0);
	    let lit = i.color * (ambient + diff);
	    return vec4<f32>(lit, 1.0);
	}
	"#;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("simple_scene_shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(shader_src)),
        });

        let uniform_alignment = device.limits().min_uniform_buffer_offset_alignment as u64;
        let uniform_size = std::mem::size_of::<SceneUniform>() as u64;
        let uniform_stride =
            (uniform_size + uniform_alignment - 1) / uniform_alignment * uniform_alignment;
        let max_objects = 128usize;

        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("simple_scene_uniform_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: NonZeroU64::new(uniform_size),
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("simple_scene_pipeline_layout"),
            bind_group_layouts: &[&uniform_layout],
            immediate_size: 0,
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("simple_scene_uniform_buffer"),
            size: uniform_stride * max_objects as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("simple_scene_uniform_bg"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniform_buffer,
                    offset: 0,
                    size: NonZeroU64::new(uniform_size),
                }),
            }],
        });

        let depth_state_grid = wgpu::DepthStencilState {
            format: depth_format,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: Default::default(),
            bias: Default::default(),
        };
        let depth_state_cube = wgpu::DepthStencilState {
            format: depth_format,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: Default::default(),
            bias: Default::default(),
        };

        let grid_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("simple_scene_grid_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[SceneVertex::layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(depth_state_grid),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let cube_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("simple_scene_cube_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[SceneVertex::layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(depth_state_cube),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // Grid vertices on XZ plane.
        let mut grid_vertices: Vec<SceneVertex> = Vec::new();
        let grid_half = 5;
        let grid_color = [0.35, 0.37, 0.40];
        for i in -grid_half..=grid_half {
            let f = i as f32;
            // Lines parallel to X (varying Z)
            grid_vertices.push(SceneVertex {
                position: [-grid_half as f32, 0.0, f],
                normal: [0.0, 0.0, 0.0],
                color: grid_color,
            });
            grid_vertices.push(SceneVertex {
                position: [grid_half as f32, 0.0, f],
                normal: [0.0, 0.0, 0.0],
                color: grid_color,
            });
            // Lines parallel to Z (varying X)
            grid_vertices.push(SceneVertex {
                position: [f, 0.0, -grid_half as f32],
                normal: [0.0, 0.0, 0.0],
                color: grid_color,
            });
            grid_vertices.push(SceneVertex {
                position: [f, 0.0, grid_half as f32],
                normal: [0.0, 0.0, 0.0],
                color: grid_color,
            });
        }
        let grid_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("simple_scene_grid_vb"),
            contents: bytemuck::cast_slice(&grid_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Cube mesh (unit cube centered at origin) with per-face normals for lighting.
        let cube_color = [0.80, 0.20, 0.80];
        let cube_vertices: [SceneVertex; 24] = [
            // back (z-)
            SceneVertex {
                position: [-0.5, -0.5, -0.5],
                normal: [0.0, 0.0, -1.0],
                color: cube_color,
            },
            SceneVertex {
                position: [-0.5, 0.5, -0.5],
                normal: [0.0, 0.0, -1.0],
                color: cube_color,
            },
            SceneVertex {
                position: [0.5, 0.5, -0.5],
                normal: [0.0, 0.0, -1.0],
                color: cube_color,
            },
            SceneVertex {
                position: [0.5, -0.5, -0.5],
                normal: [0.0, 0.0, -1.0],
                color: cube_color,
            },
            // front (z+)
            SceneVertex {
                position: [-0.5, -0.5, 0.5],
                normal: [0.0, 0.0, 1.0],
                color: cube_color,
            },
            SceneVertex {
                position: [0.5, -0.5, 0.5],
                normal: [0.0, 0.0, 1.0],
                color: cube_color,
            },
            SceneVertex {
                position: [0.5, 0.5, 0.5],
                normal: [0.0, 0.0, 1.0],
                color: cube_color,
            },
            SceneVertex {
                position: [-0.5, 0.5, 0.5],
                normal: [0.0, 0.0, 1.0],
                color: cube_color,
            },
            // left (x-)
            SceneVertex {
                position: [-0.5, -0.5, -0.5],
                normal: [-1.0, 0.0, 0.0],
                color: cube_color,
            },
            SceneVertex {
                position: [-0.5, -0.5, 0.5],
                normal: [-1.0, 0.0, 0.0],
                color: cube_color,
            },
            SceneVertex {
                position: [-0.5, 0.5, 0.5],
                normal: [-1.0, 0.0, 0.0],
                color: cube_color,
            },
            SceneVertex {
                position: [-0.5, 0.5, -0.5],
                normal: [-1.0, 0.0, 0.0],
                color: cube_color,
            },
            // right (x+)
            SceneVertex {
                position: [0.5, -0.5, -0.5],
                normal: [1.0, 0.0, 0.0],
                color: cube_color,
            },
            SceneVertex {
                position: [0.5, 0.5, -0.5],
                normal: [1.0, 0.0, 0.0],
                color: cube_color,
            },
            SceneVertex {
                position: [0.5, 0.5, 0.5],
                normal: [1.0, 0.0, 0.0],
                color: cube_color,
            },
            SceneVertex {
                position: [0.5, -0.5, 0.5],
                normal: [1.0, 0.0, 0.0],
                color: cube_color,
            },
            // top (y+)
            SceneVertex {
                position: [-0.5, 0.5, -0.5],
                normal: [0.0, 1.0, 0.0],
                color: cube_color,
            },
            SceneVertex {
                position: [-0.5, 0.5, 0.5],
                normal: [0.0, 1.0, 0.0],
                color: cube_color,
            },
            SceneVertex {
                position: [0.5, 0.5, 0.5],
                normal: [0.0, 1.0, 0.0],
                color: cube_color,
            },
            SceneVertex {
                position: [0.5, 0.5, -0.5],
                normal: [0.0, 1.0, 0.0],
                color: cube_color,
            },
            // bottom (y-)
            SceneVertex {
                position: [-0.5, -0.5, -0.5],
                normal: [0.0, -1.0, 0.0],
                color: cube_color,
            },
            SceneVertex {
                position: [0.5, -0.5, -0.5],
                normal: [0.0, -1.0, 0.0],
                color: cube_color,
            },
            SceneVertex {
                position: [0.5, -0.5, 0.5],
                normal: [0.0, -1.0, 0.0],
                color: cube_color,
            },
            SceneVertex {
                position: [-0.5, -0.5, 0.5],
                normal: [0.0, -1.0, 0.0],
                color: cube_color,
            },
        ];
        // CCW winding for front faces (FrontFace::Ccw + back-face culling).
        let cube_indices: [u16; 36] = [
            0, 1, 2, 0, 2, 3, // back
            4, 5, 6, 4, 6, 7, // front
            8, 9, 10, 8, 10, 11, // left
            12, 13, 14, 12, 14, 15, // right
            16, 17, 18, 16, 18, 19, // top
            20, 21, 22, 20, 22, 23, // bottom
        ];
        let cube_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("simple_scene_cube_vb"),
            contents: bytemuck::cast_slice(&cube_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let cube_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("simple_scene_cube_ib"),
            contents: bytemuck::cast_slice(&cube_indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            grid_pipeline,
            cube_pipeline,
            grid_vertex_buffer,
            grid_vertex_count: grid_vertices.len() as u32,
            cube_vertex_buffer,
            cube_index_buffer,
            cube_index_count: cube_indices.len() as u32,
            uniform_buffer,
            uniform_bind_group,
            uniform_stride,
            max_objects,
        }
    }

    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        queue: &wgpu::Queue,
        view_proj: Mat4,
        models: &[Mat4],
        show_grid: bool,
    ) {
        let uniform_size = std::mem::size_of::<SceneUniform>() as usize;
        let object_count = models.len() + if show_grid { 1 } else { 0 };
        if object_count == 0 {
            return;
        }
        assert!(
            object_count <= self.max_objects,
            "too many scene objects for this example"
        );

        let mut uniform_bytes = vec![0u8; self.uniform_stride as usize * object_count];
        let mut write_uniform = |index: usize, model: Mat4| {
            let uniform = SceneUniform {
                view_proj: view_proj.to_cols_array_2d(),
                model: model.to_cols_array_2d(),
            };
            let start = index * self.uniform_stride as usize;
            uniform_bytes[start..start + uniform_size]
                .copy_from_slice(bytemuck::bytes_of(&uniform));
        };

        let mut base = 0usize;
        if show_grid {
            write_uniform(0, Mat4::IDENTITY);
            base = 1;
        }
        for (i, model) in models.iter().copied().enumerate() {
            write_uniform(base + i, model);
        }
        queue.write_buffer(&self.uniform_buffer, 0, &uniform_bytes);

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("simple_scene_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.10,
                        g: 0.18,
                        b: 0.35,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        if show_grid {
            pass.set_bind_group(0, &self.uniform_bind_group, &[0]);
            pass.set_pipeline(&self.grid_pipeline);
            pass.set_vertex_buffer(0, self.grid_vertex_buffer.slice(..));
            pass.draw(0..self.grid_vertex_count, 0..1);
        }

        pass.set_pipeline(&self.cube_pipeline);
        pass.set_vertex_buffer(0, self.cube_vertex_buffer.slice(..));
        pass.set_index_buffer(self.cube_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        for i in 0..models.len() {
            let idx = i + if show_grid { 1 } else { 0 };
            let offset_bytes = (idx as u64 * self.uniform_stride) as u32;
            pass.set_bind_group(0, &self.uniform_bind_group, &[offset_bytes]);
            pass.draw_indexed(0..self.cube_index_count, 0, 0..1);
        }
    }
}

struct ViewRtt {
    #[allow(dead_code)] // Keep the texture alive; the view alone doesn't own it.
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    #[allow(dead_code)] // Keep the depth texture alive for the depth view.
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    texture_id: TextureId,
    #[allow(dead_code)] // Fixed-size RTT for this example.
    size: (u32, u32),
}

impl ViewRtt {
    fn create(
        device: &wgpu::Device,
        renderer: &mut WgpuRenderer,
        format: wgpu::TextureFormat,
        sampler: &wgpu::Sampler,
        label: &str,
    ) -> Self {
        let size = (512, 512);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
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

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("{label}_depth")),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24Plus,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let id64 = renderer.register_external_texture_with_sampler(&texture, &view, sampler);
        let texture_id = TextureId::from(id64);

        Self {
            texture,
            view,
            depth_texture,
            depth_view,
            texture_id,
            size,
        }
    }

    fn render_into(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        scene_renderer: &SimpleSceneRenderer,
        queue: &wgpu::Queue,
        view: Mat4,
        proj: Mat4,
        models: &[Mat4],
        show_grid: bool,
    ) {
        let view_proj = proj * view;
        scene_renderer.render(
            encoder,
            &self.view,
            &self.depth_view,
            queue,
            view_proj,
            models,
            show_grid,
        );
    }
}

struct AppWindow {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    window: Arc<Window>,
    surface_desc: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    imgui: Option<ImguiState>,
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn setup_gpu(event_loop: &ActiveEventLoop) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let window = {
            let version = env!("CARGO_PKG_VERSION");
            let initial_size = LogicalSize::new(1600.0, 900.0); // Larger window for game engine UI

            Arc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title(format!("Game Engine Docking Demo - dear-imgui {version}"))
                            .with_inner_size(initial_size),
                    )
                    .expect("Failed to create window"),
            )
        };

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Failed to find an appropriate adapter");

        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: None,
            ..Default::default()
        }))
        .expect("Failed to create device");

        let size = window.inner_size();
        // Pick an sRGB surface format when available for consistent visuals
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

        let surface_desc = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width as u32,
            height: size.height as u32,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &surface_desc);

        Self {
            instance,
            adapter,
            device,
            queue,
            window,
            surface_desc,
            surface,
            imgui: None,
        }
    }

    fn setup_imgui(&mut self) {
        let mut context = Context::create();
        // Disable INI load/save to test DockBuilder-only layout reliably
        context
            .set_ini_filename::<std::path::PathBuf>(None)
            .unwrap();

        let enable_viewports = cfg!(feature = "multi-viewport")
            && cfg!(any(
                target_os = "windows",
                target_os = "macos",
                target_os = "linux"
            ));
        #[cfg(feature = "multi-viewport")]
        if enable_viewports {
            context.enable_multi_viewport();
        }

        // Enable docking
        let io = context.io_mut();
        let mut cf = io.config_flags();
        cf.insert(ConfigFlags::DOCKING_ENABLE);
        io.set_config_flags(cf);
        // Prevent click-drag in content from moving windows. This avoids accidental viewport window
        // moves when interacting with scene gizmos in multi-viewport mode.
        io.set_config_windows_move_from_title_bar_only(true);

        let mut platform = WinitPlatform::new(&mut context);
        platform.attach_window(
            &self.window,
            dear_imgui_winit::HiDpiMode::Default,
            &mut context,
        );

        // Initialize renderer with device and queue using one-step initialization
        let init_info = dear_imgui_wgpu::WgpuInitInfo::new(
            self.device.clone(),
            self.queue.clone(),
            self.surface_desc.format,
        )
        .with_instance(self.instance.clone())
        .with_adapter(self.adapter.clone());
        let mut renderer =
            WgpuRenderer::new(init_info, &mut context).expect("Failed to initialize WGPU renderer");
        // Unify visuals (sRGB): auto gamma by format
        renderer.set_gamma_mode(dear_imgui_wgpu::GammaMode::Auto);

        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("view_rtt_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let scene_renderer = SimpleSceneRenderer::new(&self.device, self.surface_desc.format);
        let scene_view_rtt = ViewRtt::create(
            &self.device,
            &mut renderer,
            self.surface_desc.format,
            &sampler,
            "scene_view_rtt",
        );
        let game_view_rtt = ViewRtt::create(
            &self.device,
            &mut renderer,
            self.surface_desc.format,
            &sampler,
            "game_view_rtt",
        );

        let clear_color = wgpu::Color {
            r: 0.1,
            g: 0.2,
            b: 0.3,
            a: 1.0,
        };
        #[cfg(feature = "multi-viewport")]
        if enable_viewports {
            // Use the same clear color for secondary viewport OS windows to avoid
            // jarring black flashes during platform-managed moves/resizes.
            renderer.set_viewport_clear_color(clear_color);
        }

        #[cfg(feature = "implot")]
        let plot_context = PlotContext::create(&context);

        self.imgui = Some(ImguiState {
            context,
            platform,
            renderer,
            enable_viewports,
            clear_color,
            last_frame: Instant::now(),
            game_state: GameEngineState::default(),
            scene_renderer,
            scene_view_rtt,
            game_view_rtt,
            dockspace_id: 0,
            first_frame: true,
            #[cfg(feature = "implot")]
            plot_context,
        });
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let imgui = self.imgui.as_mut().unwrap();

        let now = Instant::now();
        let delta_time = now - imgui.last_frame;
        imgui
            .context
            .io_mut()
            .set_delta_time(delta_time.as_secs_f32());
        imgui.last_frame = now;

        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        imgui
            .platform
            .prepare_frame(&self.window, &mut imgui.context);

        let ui = imgui.context.frame();

        // Stable dockspace id and layout setup before submitting DockSpace
        let dock_id_struct = ui.get_id("MainDockSpace");
        imgui.dockspace_id = dock_id_struct.into();
        if imgui.first_frame {
            setup_initial_docking_layout(dear_imgui_rs::Id::from(imgui.dockspace_id));
            imgui.first_frame = false;
        }
        let _ = ui.dockspace_over_main_viewport_with_flags(
            dear_imgui_rs::Id::from(imgui.dockspace_id),
            dear_imgui_rs::DockNodeFlags::PASSTHRU_CENTRAL_NODE
                | dear_imgui_rs::DockNodeFlags::AUTO_HIDE_TAB_BAR,
        );

        let actions = render_main_menu_bar(ui, &mut imgui.game_state);
        render_hierarchy(ui, &mut imgui.game_state);
        render_project(ui, &mut imgui.game_state);
        render_inspector(ui, &mut imgui.game_state);
        render_scene_view(
            ui,
            &mut imgui.game_state,
            Some(imgui.scene_view_rtt.texture_id),
        );
        render_game_view(
            ui,
            &mut imgui.game_state,
            Some(imgui.game_view_rtt.texture_id),
        );
        render_console(ui, &mut imgui.game_state);
        render_asset_browser(ui, &mut imgui.game_state);

        // Render performance panel with optional ImPlot support
        #[cfg(feature = "implot")]
        {
            let plot_ui = imgui.plot_context.get_plot_ui(ui);
            render_performance(ui, &plot_ui, &mut imgui.game_state);
        }
        #[cfg(not(feature = "implot"))]
        render_performance(ui, &mut imgui.game_state);

        // Let the platform backend finalize per-frame data (required for viewports)
        imgui
            .platform
            .prepare_render(&mut imgui.context, &self.window);
        let draw_data = imgui.context.render();

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        let models: Vec<Mat4> = imgui
            .game_state
            .entities
            .iter()
            .map(|e| e.model_matrix())
            .collect();

        // Scene View uses the orbit camera updated by the UI.
        imgui.scene_view_rtt.render_into(
            &mut encoder,
            &imgui.scene_renderer,
            &self.queue,
            imgui.game_state.camera_view,
            imgui.game_state.camera_proj,
            &models,
            imgui.game_state.show_grid,
        );

        // Game View uses a fixed, slightly tilted runtime camera.
        let game_view = Mat4::look_at_rh(Vec3::new(3.0, 3.0, 3.0), Vec3::ZERO, Vec3::Y);
        let game_proj = Mat4::perspective_rh(45.0_f32.to_radians(), 1.0, 0.1, 100.0);
        imgui.game_view_rtt.render_into(
            &mut encoder,
            &imgui.scene_renderer,
            &self.queue,
            game_view,
            game_proj,
            &models,
            imgui.game_state.show_grid,
        );

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(imgui.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            imgui
                .renderer
                .render_draw_data_with_fb_size(
                    draw_data,
                    &mut render_pass,
                    self.surface_desc.width,
                    self.surface_desc.height,
                )
                .unwrap_or_else(|e| {
                    eprintln!("ImGui render error: {:?}", e);
                });
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();

        #[cfg(feature = "multi-viewport")]
        if imgui.enable_viewports {
            imgui.context.update_platform_windows();
            imgui.context.render_platform_windows_default();
        }

        // Handle deferred actions (safe after frame is rendered)
        if actions.reset_layout {
            setup_initial_docking_layout(dear_imgui_rs::Id::from(imgui.dockspace_id));
        }
        if actions.load_ini {
            if let Ok(s) = std::fs::read_to_string("examples/02-docking/game_engine_docking.ini") {
                let target = {
                    let vp = dear_imgui_rs::Viewport::main();
                    let ws = vp.work_size();
                    (ws[0], ws[1])
                };
                if let Some(base) = detect_base_size_from_ini(&s) {
                    let scaled = scale_ini_for_target(&s, base, target);
                    imgui.context.load_ini_settings(&scaled);
                    imgui.game_state.console_logs.push(format!(
                        "[INFO] Layout loaded from INI (scaled {:.0}x{:.0} -> {:.0}x{:.0})",
                        base.0, base.1, target.0, target.1
                    ));
                } else {
                    imgui.context.load_ini_settings(&s);
                    imgui
                        .game_state
                        .console_logs
                        .push("[INFO] Layout loaded from INI".to_string());
                }
            } else {
                imgui.game_state.console_logs.push(
                    "[WARNING] Failed to read examples/02-docking/game_engine_docking.ini"
                        .to_string(),
                );
            }
        }
        if actions.save_ini {
            let mut buf = String::new();
            imgui.context.save_ini_settings(&mut buf);
            if std::fs::write("examples/02-docking/game_engine_docking.ini", buf).is_ok() {
                imgui
                    .game_state
                    .console_logs
                    .push("[INFO] Layout saved to INI".to_string());
            } else {
                imgui.game_state.console_logs.push(
                    "[WARNING] Failed to write examples/02-docking/game_engine_docking.ini"
                        .to_string(),
                );
            }
        }

        Ok(())
    }
}

impl App {
    fn exit(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(mut window) = self.window.take() {
            window.imgui = None;
        }
        event_loop.exit();
    }
}

/// Setup the initial docking layout - Unity-style game engine layout
fn setup_initial_docking_layout(dockspace_id: dear_imgui_rs::Id) {
    use dear_imgui_rs::{DockBuilder, SplitDirection};

    println!("Setting up initial docking layout...");

    // Clear any existing layout and create fresh dockspace (size comes from main viewport)
    DockBuilder::remove_node_docked_windows(dockspace_id, true);
    DockBuilder::remove_node(dockspace_id);
    DockBuilder::add_node(dockspace_id, dear_imgui_rs::DockNodeFlags::NONE);
    // Match node pos/size to main viewport work area (exclude menu bars) before splitting
    {
        let vp = dear_imgui_rs::Viewport::main();
        DockBuilder::set_node_pos(dockspace_id, vp.work_pos());
        DockBuilder::set_node_size(dockspace_id, vp.work_size());
    }

    // Unity-style Professional Layout:
    // +-------------------+---------------------------+-------------------+
    // |                   |        Scene View         |                   |
    // |   Hierarchy       |---------------------------|    Inspector      |
    // |                   |        Game View          |                   |
    // +-------------------+---------------------------+-------------------+
    // |      Project      |         Console           |   Performance     |
    // +-------------------+---------------------------+-------------------+

    // Split horizontally to match Unity-like proportions derived from INI (approx.):
    // Right Inspector ~25.1%, Left Hierarchy column ~21.7%, Center ~remaining
    // First split right from root
    let (inspector_panel, after_right) = DockBuilder::split_node(
        dockspace_id,
        SplitDirection::Right,
        0.251, // 402/1600
    );
    // Then split left from the remaining
    let (left_panel_id, center_area_id) = DockBuilder::split_node(
        after_right,
        SplitDirection::Left,
        0.2896, // normalized 0.217/(1-0.251)
    );

    // Split left panel vertically to match INI heights (approx):
    // total H ~881, Hierarchy 337, Project 264, Asset 276
    // First split bottom Asset (~276/881 ≈ 0.313)
    let (asset_id, top_left_stack) = DockBuilder::split_node(
        left_panel_id,
        SplitDirection::Down,
        0.313, // Asset Browser
    );
    // Then split the remaining into Hierarchy (337) and Project (264)
    let (project_id, hierarchy_id) = DockBuilder::split_node(
        top_left_stack,
        SplitDirection::Down,
        0.439, // 337/(337+264)
    );

    // Split right panel vertically: Performance (~20%) bottom, Inspector (~80%) top
    let (performance_id, inspector_id) =
        DockBuilder::split_node(inspector_panel, SplitDirection::Down, 0.2);

    // Split center vertically: Console (~27%) bottom, Scene/Game (~73%) top
    let (console_id, scene_game_id) = DockBuilder::split_node(
        center_area_id,
        SplitDirection::Down,
        0.313, // 276/881 approx
    );

    // Dock all windows to their designated areas
    DockBuilder::dock_window("Hierarchy", hierarchy_id);
    DockBuilder::dock_window("Project", project_id);
    DockBuilder::dock_window("Asset Browser", asset_id); // Tabbed vertically under Project
    DockBuilder::dock_window("Scene View", scene_game_id);
    DockBuilder::dock_window("Game View", scene_game_id); // Tabbed with Scene View
    DockBuilder::dock_window("Console", console_id); // Bottom center
    DockBuilder::dock_window("Inspector", inspector_id);
    DockBuilder::dock_window("Performance", performance_id);

    // Finalize the layout
    DockBuilder::finish(dockspace_id);

    println!("Docking layout setup complete");
}

#[derive(Default, Clone, Copy)]
struct MenuActions {
    reset_layout: bool,
    load_ini: bool,
    save_ini: bool,
}

/// Detect the base reference size from an ImGui .ini docking block.
/// Prefer the DockSpace Size=WxH line; fallback to WindowOverViewport size if present.
fn detect_base_size_from_ini(ini: &str) -> Option<(f32, f32)> {
    for line in ini.lines() {
        if line.trim_start().starts_with("DockSpace")
            && let Some(sz) = extract_pair_after_key(line, "Size=")
        {
            return Some(sz);
        }
    }
    // Fallback: search any WindowOverViewport_ section next, then Size=
    let mut in_viewport = false;
    for line in ini.lines() {
        if line.contains("[Window][WindowOverViewport_") {
            in_viewport = true;
            continue;
        }
        if in_viewport {
            if let Some(sz) = extract_pair_after_key(line, "Size=") {
                return Some(sz);
            }
            if line.starts_with('[') {
                break;
            }
        }
    }
    None
}

/// Scale all Pos=, Size= and SizeRef= pairs within the INI to target size ratios.
fn scale_ini_for_target(ini: &str, base: (f32, f32), target: (f32, f32)) -> String {
    let (bw, bh) = base;
    let (tw, th) = target;
    let sx = if bw > 0.0 { tw / bw } else { 1.0 };
    let sy = if bh > 0.0 { th / bh } else { 1.0 };

    let mut out = String::with_capacity(ini.len());
    for mut line in ini.lines().map(|l| l.to_string()) {
        for key in ["Pos=", "Size=", "SizeRef="] {
            if let Some((x, y, start, end)) = extract_pair_with_span(&line, key) {
                let nx = (x as f32 * sx).round() as i32;
                let ny = (y as f32 * sy).round() as i32;
                let mut new_line = String::with_capacity(line.len() + 8);
                new_line.push_str(&line[..start]);
                new_line.push_str(key);
                new_line.push_str(&format!("{},{}", nx, ny));
                new_line.push_str(&line[end..]);
                line = new_line;
            }
        }
        out.push_str(&line);
        out.push('\n');
    }
    out
}

fn extract_pair_after_key(line: &str, key: &str) -> Option<(f32, f32)> {
    if let Some(idx) = line.find(key) {
        let rest = &line[idx + key.len()..];
        let mut it = rest.split([',', ' ', '\t', '\r']);
        let a = it.next()?.trim();
        let b = it.next()?.trim();
        if let (Ok(ax), Ok(by)) = (a.parse::<f32>(), b.parse::<f32>()) {
            return Some((ax, by));
        }
    }
    None
}

fn extract_pair_with_span(line: &str, key: &str) -> Option<(i32, i32, usize, usize)> {
    let kpos = line.find(key)?;
    let start = kpos + key.len();
    let bytes = line.as_bytes();
    let mut i = start;
    if i < bytes.len() && bytes[i] as char == '-' {
        i += 1;
    }
    while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
        i += 1;
    }
    if i >= bytes.len() || bytes[i] as char != ',' {
        return None;
    }
    let x_str = &line[start..i];
    let mut j = i + 1;
    if j < bytes.len() && bytes[j] as char == '-' {
        j += 1;
    }
    while j < bytes.len() && (bytes[j] as char).is_ascii_digit() {
        j += 1;
    }
    let y_str = &line[i + 1..j];
    let x = x_str.parse::<i32>().ok()?;
    let y = y_str.parse::<i32>().ok()?;
    Some((x, y, kpos, j))
}

/// Render the main menu bar
fn render_main_menu_bar(ui: &Ui, game_state: &mut GameEngineState) -> MenuActions {
    let mut actions = MenuActions::default();
    if let Some(_main_menu_bar) = ui.begin_main_menu_bar() {
        ui.menu("File", || {
            if ui.menu_item("New Scene") {
                game_state
                    .console_logs
                    .push("[INFO] New scene created".to_string());
            }
            if ui.menu_item("Open Scene") {
                game_state
                    .console_logs
                    .push("[INFO] Scene opened".to_string());
            }
            if ui.menu_item("Save Scene") {
                game_state
                    .console_logs
                    .push("[INFO] Scene saved".to_string());
            }
            ui.same_line();
            ui.text("Search:");
            ui.same_line();
            ui.input_text_imstr("##asset_search", &mut game_state.asset_search)
                .build();

            ui.separator();
            if ui.menu_item("Exit") {
                // Handle exit
            }
        });

        ui.menu("Edit", || {
            ui.menu_item("Undo");
            ui.menu_item("Redo");
            ui.separator();
            ui.menu_item("Cut");
            ui.menu_item("Copy");
            ui.menu_item("Paste");
        });

        ui.menu("GameObject", || {
            if ui.menu_item("Create Empty") {
                let idx = game_state.entities.len() + 1;
                let name = format!("GameObject ({idx})");
                let pos = [idx as f32 * 0.5, 0.5, 0.0];
                game_state.entities.push(SceneEntity::cube(name, pos));
                game_state
                    .console_logs
                    .push("[INFO] Empty GameObject created".to_string());
            }
            ui.menu("3D Object", || {
                if ui.menu_item("Cube") {
                    let idx = game_state.entities.len() + 1;
                    let name = format!("Cube ({idx})");
                    let pos = [idx as f32 * 0.5, 0.5, 0.0];
                    game_state.entities.push(SceneEntity::cube(name, pos));
                }
                if ui.menu_item("Sphere") {
                    game_state
                        .console_logs
                        .push("[INFO] Sphere is not implemented in this example".to_string());
                }
                if ui.menu_item("Plane") {
                    game_state
                        .console_logs
                        .push("[INFO] Plane is not implemented in this example".to_string());
                }
            });
        });

        ui.menu("Window", || {
            ui.menu_item("Scene Hierarchy");
            ui.menu_item("Inspector");
            ui.menu_item("Console");
            ui.menu_item("Asset Browser");
            ui.menu_item("Performance Stats");
        });

        ui.menu("Layout", || {
            if ui.menu_item("Reset to Unity Layout") {
                actions.reset_layout = true;
            }
            ui.separator();
            if ui.menu_item("Load Layout (INI)") {
                actions.load_ini = true;
            }
            if ui.menu_item("Save Layout (INI)") {
                actions.save_ini = true;
            }
        });

        ui.menu("Help", || {
            ui.menu_item("About");
            ui.menu_item("Documentation");
        });
    }
    actions
}

/// Render the Hierarchy panel (Unity-style)
fn render_hierarchy(ui: &Ui, game_state: &mut GameEngineState) {
    ui.window("Hierarchy")
        .size([300.0, 400.0], Condition::FirstUseEver)
        .build(|| {
            // Scene name header
            ui.text_colored(
                [0.8, 0.8, 0.8, 1.0],
                format!("Scene: {}", game_state.scene_name),
            );
            ui.separator();

            // Search filter with icon
            ui.text("[Search]");
            ui.same_line();
            ui.input_text_imstr("##search", &mut game_state.hierarchy_search)
                .build();

            ui.separator();

            // Entity list with hierarchy (filter by search query)
            let mut selected_entity: Option<usize> = None;
            let mut entity_to_duplicate: Option<usize> = None;
            let mut entity_to_delete: Option<usize> = None;

            let query = game_state.hierarchy_search.to_str().to_lowercase();
            for (idx, entity) in game_state.entities.iter().enumerate() {
                if !query.is_empty() && !entity.name.to_lowercase().contains(&query) {
                    continue;
                }
                let is_selected = game_state.selected_entity == Some(idx);

                // Add hierarchy indentation and icons
                let icon = if entity.name.contains("Camera") {
                    "[C]"
                } else if entity.name.contains("Light") {
                    "[L]"
                } else if entity.name.contains("Mesh") {
                    "[M]"
                } else {
                    "[O]"
                };

                ui.text(icon);
                ui.same_line();

                if ui
                    .selectable_config(&entity.name)
                    .selected(is_selected)
                    .build()
                {
                    selected_entity = Some(idx);
                }

                // Right-click context menu for entities
                if let Some(_popup) = ui.begin_popup_context_item() {
                    if ui.menu_item("Create Empty Child") {
                        entity_to_duplicate = Some(idx);
                    }
                    ui.separator();
                    if ui.menu_item("Duplicate") {
                        entity_to_duplicate = Some(idx);
                    }
                    if ui.menu_item("Delete") {
                        entity_to_delete = Some(idx);
                    }
                    ui.separator();
                    if ui.menu_item("Rename") {
                        game_state
                            .console_logs
                            .push("[INFO] Rename is not implemented in this example".to_string());
                    }
                }
            }

            // Handle actions outside the loop
            if let Some(idx) = selected_entity {
                game_state.selected_entity = Some(idx);
            }

            if let Some(idx) = entity_to_duplicate {
                if let Some(src) = game_state.entities.get(idx).cloned() {
                    let mut copy = src;
                    copy.name = format!("{} (Copy)", copy.name);
                    game_state.entities.push(copy);
                }
            }

            if let Some(idx) = entity_to_delete {
                if idx < game_state.entities.len() {
                    let name = game_state.entities[idx].name.clone();
                    game_state
                        .console_logs
                        .push(format!("[INFO] {} deleted", name));
                    game_state.entities.remove(idx);

                    match game_state.selected_entity {
                        Some(sel) if sel == idx => game_state.selected_entity = None,
                        Some(sel) if sel > idx => game_state.selected_entity = Some(sel - 1),
                        _ => {}
                    }
                }
            }

            ui.separator();

            // Create buttons
            if ui.button("Create Empty") {
                let idx = game_state.entities.len() + 1;
                let name = format!("GameObject ({idx})");
                let pos = [idx as f32 * 0.5, 0.5, 0.0];
                game_state.entities.push(SceneEntity::cube(name, pos));
            }
            ui.same_line();
            if ui.button("Create Cube") {
                let idx = game_state.entities.len() + 1;
                let name = format!("Cube ({idx})");
                let pos = [idx as f32 * 0.5, 0.5, 0.0];
                game_state.entities.push(SceneEntity::cube(name, pos));
            }
        });
}

/// Render the Project panel (Unity-style)
fn render_project(ui: &Ui, game_state: &mut GameEngineState) {
    ui.window("Project")
        .size([300.0, 200.0], Condition::FirstUseEver)
        .build(|| {
            // Project folder navigation
            ui.text_colored(
                [0.8, 0.8, 0.8, 1.0],
                format!("Project: {}", game_state.project_name),
            );
            ui.separator();

            // Folder path
            ui.text("[Folder]");
            ui.same_line();
            ui.text(&game_state.current_folder);

            ui.separator();

            // Asset grid view
            if ui.button("List View") {
                game_state.project_columns = 1;
            }
            ui.same_line();
            if ui.button("Grid View") {
                game_state.project_columns = 4;
            }
            ui.same_line();
            ui.text("Search:");
            ui.same_line();
            ui.input_text_imstr("##project_asset_search", &mut game_state.asset_search)
                .build();

            ui.separator();

            // Assets display
            let aquery = game_state.asset_search.to_str().to_lowercase();
            for (i, asset) in game_state.assets.iter().enumerate() {
                if !aquery.is_empty() && !asset.to_lowercase().contains(&aquery) {
                    continue;
                }
                if i % game_state.project_columns != 0 {
                    ui.same_line();
                }

                let icon = if asset.ends_with(".cs") {
                    "[TXT]"
                } else if asset.ends_with(".png") || asset.ends_with(".jpg") {
                    "[IMG]"
                } else if asset.ends_with(".fbx") || asset.ends_with(".obj") {
                    "[3D]"
                } else if asset.ends_with(".wav") || asset.ends_with(".mp3") {
                    "[SND]"
                } else {
                    "[FILE]"
                };

                ui.button(format!("{}\n{}", icon, asset));

                // Right-click context menu for assets
                if let Some(popup) = ui.begin_popup_context_item() {
                    if ui.menu_item("Import") {
                        game_state
                            .console_logs
                            .push(format!("[INFO] Importing {}", asset));
                    }
                    if ui.menu_item("Delete") {
                        game_state
                            .console_logs
                            .push(format!("[INFO] Deleted {}", asset));
                    }
                    ui.separator();
                    if ui.menu_item("Show in Explorer") {
                        game_state
                            .console_logs
                            .push(format!("[INFO] Opening {}", asset));
                    }
                    popup.end();
                }
            }

            ui.separator();

            // Import button
            if ui.button("Import New Asset") {
                game_state
                    .assets
                    .push(format!("NewAsset_{}.png", game_state.assets.len()));
            }
        });
}

/// Render the inspector panel
fn render_inspector(ui: &Ui, game_state: &mut GameEngineState) {
    ui.window("Inspector")
        .size([350.0, 500.0], Condition::FirstUseEver)
        .build(|| {
            if let Some(selected_idx) = game_state.selected_entity {
                let Some(entity) = game_state.entities.get_mut(selected_idx) else {
                    game_state.selected_entity = None;
                    ui.text("No object selected");
                    return;
                };

                ui.text(format!("Selected: {}", entity.name));
                ui.separator();

                // Transform component
                if ui.collapsing_header("Transform", TreeNodeFlags::DEFAULT_OPEN) {
                    // Calculate width for each component (label + 3 inputs)
                    let available_width = ui.content_region_avail()[0];
                    let label_width = 70.0; // Increased from 60.0 to prevent "Position" text overflow
                    let spacing = ui.clone_style().item_spacing()[0];
                    // Reserve more space for spacing to prevent overlap
                    let input_width = (available_width - label_width - spacing * 3.0) / 3.0;

                    // Position
                    ui.text("Position");
                    ui.same_line_with_pos(label_width);
                    ui.set_next_item_width(input_width);
                    let mut pos_changed = ui
                        .drag_float_config("##pos_x")
                        .speed(0.1)
                        .build(ui, &mut entity.position[0]);
                    ui.same_line();
                    ui.set_next_item_width(input_width);
                    pos_changed = ui
                        .drag_float_config("##pos_y")
                        .speed(0.1)
                        .build(ui, &mut entity.position[1])
                        || pos_changed;
                    ui.same_line();
                    ui.set_next_item_width(input_width);
                    pos_changed = ui
                        .drag_float_config("##pos_z")
                        .speed(0.1)
                        .build(ui, &mut entity.position[2])
                        || pos_changed;

                    // Rotation
                    ui.text("Rotation");
                    ui.same_line_with_pos(label_width);
                    ui.set_next_item_width(input_width);
                    let mut rot_changed = ui
                        .drag_float_config("##rot_x")
                        .speed(1.0)
                        .build(ui, &mut entity.rotation_deg[0]);
                    ui.same_line();
                    ui.set_next_item_width(input_width);
                    rot_changed = ui
                        .drag_float_config("##rot_y")
                        .speed(1.0)
                        .build(ui, &mut entity.rotation_deg[1])
                        || rot_changed;
                    ui.same_line();
                    ui.set_next_item_width(input_width);
                    rot_changed = ui
                        .drag_float_config("##rot_z")
                        .speed(1.0)
                        .build(ui, &mut entity.rotation_deg[2])
                        || rot_changed;

                    // Scale
                    ui.text("Scale");
                    ui.same_line_with_pos(label_width);
                    ui.set_next_item_width(input_width);
                    let mut scale_changed = ui
                        .drag_float_config("##scale_x")
                        .speed(0.01)
                        .build(ui, &mut entity.scale[0]);
                    ui.same_line();
                    ui.set_next_item_width(input_width);
                    scale_changed = ui
                        .drag_float_config("##scale_y")
                        .speed(0.01)
                        .build(ui, &mut entity.scale[1])
                        || scale_changed;
                    ui.same_line();
                    ui.set_next_item_width(input_width);
                    scale_changed = ui
                        .drag_float_config("##scale_z")
                        .speed(0.01)
                        .build(ui, &mut entity.scale[2])
                        || scale_changed;

                    let _ = (pos_changed, rot_changed, scale_changed);
                }

                // Renderer component (example)
                if ui.collapsing_header("Mesh Renderer", TreeNodeFlags::empty()) {
                    ui.text("Material: Default");
                    if ui.button("Select Material") {
                        game_state
                            .console_logs
                            .push("[INFO] Material selector opened".to_string());
                    }

                    ui.checkbox("Cast Shadows", &mut true);
                    ui.checkbox("Receive Shadows", &mut true);
                }

                // Collider component (example)
                if entity.name.contains("Cube")
                    && ui.collapsing_header("Box Collider", TreeNodeFlags::empty())
                {
                    let mut is_trigger = false;
                    ui.checkbox("Is Trigger", &mut is_trigger);
                    ui.text("Size");
                    let mut size = [1.0, 1.0, 1.0];
                    ui.drag_float("X##size", &mut size[0]);
                    ui.same_line();
                    ui.drag_float("Y##size", &mut size[1]);
                    ui.same_line();
                    ui.drag_float("Z##size", &mut size[2]);
                }

                ui.separator();
                // Temporarily disabled popup to test
                // if ui.button("Add Component") {
                //     ui.open_popup("add_component");
                // }

                // ui.popup("add_component", || {
                //     if ui.menu_item("Rigidbody") {
                //         game_state.console_logs.push("[INFO] Rigidbody component added".to_string());
                //     }
                //     if ui.menu_item("Audio Source") {
                //         game_state.console_logs.push("[INFO] Audio Source component added".to_string());
                //     }
                //     if ui.menu_item("Script") {
                //         game_state.console_logs.push("[INFO] Script component added".to_string());
                //     }
                // });
            } else {
                ui.text("No object selected");
                ui.text_colored(
                    [0.7, 0.7, 0.7, 1.0],
                    "Select an object in the Scene Hierarchy to view its properties",
                );
            }
        });
}

fn update_orbit_camera(game_state: &mut GameEngineState, aspect: f32) {
    game_state.camera_proj =
        Mat4::perspective_rh(45.0_f32.to_radians(), aspect.max(0.01), 0.1, 100.0);

    let eye = Vec3::new(
        game_state.camera_distance
            * game_state.camera_y_angle.cos()
            * game_state.camera_x_angle.cos(),
        game_state.camera_distance * game_state.camera_x_angle.sin(),
        game_state.camera_distance
            * game_state.camera_y_angle.sin()
            * game_state.camera_x_angle.cos(),
    );
    game_state.camera_view = Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Y);
}

fn ray_from_screen_rect(
    mouse_pos: [f32; 2],
    rect_min: [f32; 2],
    rect_size: [f32; 2],
    view: Mat4,
    proj: Mat4,
) -> Option<(Vec3, Vec3)> {
    if rect_size[0] <= 0.0 || rect_size[1] <= 0.0 {
        return None;
    }

    let u = (mouse_pos[0] - rect_min[0]) / rect_size[0];
    let v = (mouse_pos[1] - rect_min[1]) / rect_size[1];
    if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
        return None;
    }

    let ndc_x = u * 2.0 - 1.0;
    let ndc_y = 1.0 - v * 2.0;

    let inv_vp = (proj * view).inverse();
    let near = inv_vp * Vec4::new(ndc_x, ndc_y, 0.0, 1.0);
    let far = inv_vp * Vec4::new(ndc_x, ndc_y, 1.0, 1.0);
    let near = near.truncate() / near.w;
    let far = far.truncate() / far.w;

    let dir = (far - near).normalize();
    Some((near, dir))
}

fn ray_aabb_intersect(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let inv_dir = Vec3::new(1.0 / dir.x, 1.0 / dir.y, 1.0 / dir.z);

    let mut tmin = (min.x - origin.x) * inv_dir.x;
    let mut tmax = (max.x - origin.x) * inv_dir.x;
    if tmin > tmax {
        std::mem::swap(&mut tmin, &mut tmax);
    }

    let mut tymin = (min.y - origin.y) * inv_dir.y;
    let mut tymax = (max.y - origin.y) * inv_dir.y;
    if tymin > tymax {
        std::mem::swap(&mut tymin, &mut tymax);
    }
    if tmin > tymax || tymin > tmax {
        return None;
    }
    tmin = tmin.max(tymin);
    tmax = tmax.min(tymax);

    let mut tzmin = (min.z - origin.z) * inv_dir.z;
    let mut tzmax = (max.z - origin.z) * inv_dir.z;
    if tzmin > tzmax {
        std::mem::swap(&mut tzmin, &mut tzmax);
    }
    if tmin > tzmax || tzmin > tmax {
        return None;
    }
    tmin = tmin.max(tzmin);
    tmax = tmax.min(tzmax);

    if tmax < 0.0 {
        return None;
    }
    Some(if tmin >= 0.0 { tmin } else { tmax })
}

fn pick_cube_entity(entities: &[SceneEntity], ray_origin: Vec3, ray_dir: Vec3) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (idx, entity) in entities.iter().enumerate() {
        let inv_model = entity.model_matrix().inverse();
        let o4 = inv_model * Vec4::new(ray_origin.x, ray_origin.y, ray_origin.z, 1.0);
        let d4 = inv_model * Vec4::new(ray_dir.x, ray_dir.y, ray_dir.z, 0.0);
        let o = o4.truncate();
        let d = d4.truncate();

        let min = Vec3::splat(-0.5);
        let max = Vec3::splat(0.5);
        let Some(t) = ray_aabb_intersect(o, d, min, max) else {
            continue;
        };

        match best {
            None => best = Some((idx, t)),
            Some((_, best_t)) if t < best_t => best = Some((idx, t)),
            _ => {}
        }
    }
    best.map(|(idx, _)| idx)
}

/// Render the Scene View (Unity-style editor view)
#[cfg(feature = "imguizmo")]
fn render_scene_view(ui: &Ui, game_state: &mut GameEngineState, scene_tex_id: Option<TextureId>) {
    ui.window("Scene View")
        .size([800.0, 600.0], Condition::FirstUseEver)
        .build(|| {
            // Scene view toolbar
            ui.text("[Tools]");
            ui.same_line();

            // Operation buttons
            if ui.button("Move") {
                game_state.gizmo_operation = Operation::TRANSLATE;
                game_state
                    .console_logs
                    .push("[INFO] Move tool selected".to_string());
            }
            ui.same_line();
            if ui.button("Rotate") {
                game_state.gizmo_operation = Operation::ROTATE;
                game_state
                    .console_logs
                    .push("[INFO] Rotate tool selected".to_string());
            }
            ui.same_line();
            if ui.button("Scale") {
                game_state.gizmo_operation = Operation::SCALE;
                game_state
                    .console_logs
                    .push("[INFO] Scale tool selected".to_string());
            }

            ui.same_line();
            ui.separator_vertical();
            ui.same_line();

            // Mode buttons
            if ui.button("Local") {
                game_state.gizmo_mode = Mode::Local;
            }
            ui.same_line();
            if ui.button("World") {
                game_state.gizmo_mode = Mode::World;
            }

            ui.same_line();
            ui.separator_vertical();
            ui.same_line();

            ui.checkbox("Grid", &mut game_state.show_grid);

            ui.separator();

            // Scene view area
            let content_region = ui.content_region_avail();

            if content_region[0] > 0.0 && content_region[1] > 0.0 {
                let side = content_region[0].min(content_region[1]).max(64.0);
                let canvas_size = [side, side];
                game_state.viewport_size = canvas_size;

                let canvas_pos = ui.cursor_screen_pos();
                if let Some(tex_id) = scene_tex_id {
                    ui.image(tex_id, canvas_size);
                } else {
                    ui.text("No scene render texture registered");
                }

                let aspect = canvas_size[0] / canvas_size[1];
                update_orbit_camera(game_state, aspect);

                let draw_list = ui.get_window_draw_list();
                let image_min = ui.item_rect_min();
                let image_max = ui.item_rect_max();
                let image_size = [image_max[0] - image_min[0], image_max[1] - image_min[1]];

                if ui.is_item_hovered() && ui.is_item_clicked() {
                    if let Some((ray_origin, ray_dir)) = ray_from_screen_rect(
                        ui.mouse_pos(),
                        image_min,
                        image_size,
                        game_state.camera_view,
                        game_state.camera_proj,
                    ) {
                        game_state.selected_entity =
                            pick_cube_entity(&game_state.entities, ray_origin, ray_dir);
                    }
                }

                // Setup ImGuizmo viewport for overlay gizmos.
                let giz = ui.guizmo();
                giz.set_drawlist_window();
                giz.set_rect(canvas_pos[0], canvas_pos[1], canvas_size[0], canvas_size[1]);

                if let Some(selected_idx) = game_state.selected_entity {
                    if let Some(entity) = game_state.entities.get_mut(selected_idx) {
                        let mut model = entity.model_matrix();
                        let used = giz
                            .manipulate_config(
                                &game_state.camera_view,
                                &game_state.camera_proj,
                                &mut model,
                            )
                            .operation(game_state.gizmo_operation)
                            .mode(game_state.gizmo_mode)
                            .build();
                        if used {
                            entity.set_from_matrix(model);
                        }
                    }
                } else {
                    let text = "Select an object in Hierarchy to manipulate";
                    let text_pos = [
                        canvas_pos[0] + canvas_size[0] * 0.5 - 150.0,
                        canvas_pos[1] + canvas_size[1] * 0.5,
                    ];
                    draw_list.add_text(text_pos, [0.5, 0.5, 0.5, 1.0], text);
                }

                ui.text(format!(
                    "Scene Size: {:.0}x{:.0}",
                    canvas_size[0], canvas_size[1]
                ));
            } else {
                ui.text("Scene view too small to render");
            }
        });
}

/// Render the Scene View (Unity-style editor view) - No ImGuizmo version
#[cfg(not(feature = "imguizmo"))]
fn render_scene_view(ui: &Ui, game_state: &mut GameEngineState, scene_tex_id: Option<TextureId>) {
    ui.window("Scene View")
        .size([800.0, 600.0], Condition::FirstUseEver)
        .build(|| {
            // Scene view toolbar
            ui.text("[Tools]");
            ui.same_line();
            if ui.button("Move") {
                game_state
                    .console_logs
                    .push("[INFO] Move tool selected".to_string());
            }
            ui.same_line();
            if ui.button("Rotate") {
                game_state
                    .console_logs
                    .push("[INFO] Rotate tool selected".to_string());
            }
            ui.same_line();
            if ui.button("Scale") {
                game_state
                    .console_logs
                    .push("[INFO] Scale tool selected".to_string());
            }

            ui.same_line();
            ui.separator_vertical();
            ui.same_line();

            ui.checkbox("Wireframe", &mut game_state.show_wireframe);
            ui.same_line();
            ui.checkbox("Grid", &mut game_state.show_grid);

            ui.separator();

            // Scene view area
            let content_region = ui.content_region_avail();

            if content_region[0] > 0.0 && content_region[1] > 0.0 {
                let side = content_region[0].min(content_region[1]).max(64.0);
                let canvas_size = [side, side];
                game_state.viewport_size = canvas_size;

                if let Some(tex_id) = scene_tex_id {
                    ui.image(tex_id, canvas_size);
                } else {
                    ui.text("No scene render texture registered");
                }

                let aspect = canvas_size[0] / canvas_size[1];
                update_orbit_camera(game_state, aspect);

                let image_min = ui.item_rect_min();
                let image_max = ui.item_rect_max();
                let image_size = [image_max[0] - image_min[0], image_max[1] - image_min[1]];

                if ui.is_item_hovered() && ui.is_item_clicked() {
                    if let Some((ray_origin, ray_dir)) = ray_from_screen_rect(
                        ui.mouse_pos(),
                        image_min,
                        image_size,
                        game_state.camera_view,
                        game_state.camera_proj,
                    ) {
                        game_state.selected_entity =
                            pick_cube_entity(&game_state.entities, ray_origin, ray_dir);
                    }
                }

                ui.text(format!(
                    "Scene Size: {:.0}x{:.0}",
                    canvas_size[0], canvas_size[1]
                ));
                ui.text("(Enable 'imguizmo' feature for 3D gizmo)");
            } else {
                ui.text("Scene view too small to render");
            }
        });
}

/// Render the Game View (Unity-style play view)
fn render_game_view(ui: &Ui, game_state: &mut GameEngineState, game_tex_id: Option<TextureId>) {
    ui.window("Game View")
        .size([800.0, 600.0], Condition::FirstUseEver)
        .build(|| {
            // Game view toolbar
            if ui.button("▶ Play") {
                game_state
                    .console_logs
                    .push("[INFO] Play mode started".to_string());
            }
            ui.same_line();
            if ui.button("|| Pause") {
                game_state
                    .console_logs
                    .push("[INFO] Play mode paused".to_string());
            }
            ui.same_line();
            if ui.button("■ Stop") {
                game_state
                    .console_logs
                    .push("[INFO] Play mode stopped".to_string());
            }

            ui.same_line();
            ui.separator_vertical();
            ui.same_line();

            ui.text("Aspect:");
            ui.same_line();
            if ui.button("16:9") {
                game_state
                    .console_logs
                    .push("[INFO] Aspect ratio set to 16:9".to_string());
            }
            ui.same_line();
            if ui.button("4:3") {
                game_state
                    .console_logs
                    .push("[INFO] Aspect ratio set to 4:3".to_string());
            }

            ui.separator();

            // Game view area
            let content_region = ui.content_region_avail();

            if content_region[0] > 0.0 && content_region[1] > 0.0 {
                if let Some(tex_id) = game_tex_id {
                    let side = content_region[0].min(content_region[1]).max(64.0);
                    let size = [side, side];
                    ui.image(tex_id, size);
                    ui.text(format!("Game View RT: {:.0}x{:.0}", size[0], size[1]));
                } else {
                    ui.text("No game render texture registered");
                }
                ui.text(format!("FPS: {:.1}", ui.io().framerate()));
            } else {
                ui.text("Game view too small to render");
            }
        });
}

/// Render the console panel
fn render_console(ui: &Ui, game_state: &mut GameEngineState) {
    ui.window("Console")
        .size([800.0, 200.0], Condition::FirstUseEver)
        .build(|| {
            // Console toolbar
            if ui.button("Clear") {
                game_state.console_logs.clear();
                game_state
                    .console_logs
                    .push("[INFO] Console cleared".to_string());
            }
            ui.same_line();

            let mut show_info = true;
            let mut show_warning = true;
            let mut show_error = true;

            ui.checkbox("Info", &mut show_info);
            ui.same_line();
            ui.checkbox("Warning", &mut show_warning);
            ui.same_line();
            ui.checkbox("Error", &mut show_error);

            ui.separator();

            // Console output area
            let text_height = 16.0; // Hardcoded text height since text_line_height() is not available
            let footer_height = text_height + 10.0; // Approximate spacing

            ui.child_window("ConsoleOutput")
                .size([0.0, -footer_height])
                .build(ui, || {
                    for log in &game_state.console_logs {
                        let color = if log.contains("[ERROR]") {
                            [1.0, 0.4, 0.4, 1.0] // Red
                        } else if log.contains("[WARNING]") {
                            [1.0, 1.0, 0.4, 1.0] // Yellow
                        } else {
                            [1.0, 1.0, 1.0, 1.0] // White
                        };
                        ui.text_colored(color, log);
                    }

                    // Auto-scroll to bottom
                    if ui.scroll_y() >= ui.scroll_max_y() {
                        ui.set_scroll_here_y(1.0);
                    }
                });

            ui.separator();

            // Command input
            ui.text(">");
            ui.same_line();

            let mut input_changed = false;
            let _token = ui.push_item_width(-1.0);
            if ui
                .input_text_imstr("##console_input", &mut game_state.console_input)
                .enter_returns_true(true)
                .build()
            {
                input_changed = true;
            }

            if input_changed && !game_state.console_input.to_str().trim().is_empty() {
                let command = game_state.console_input.to_str().trim().to_string();
                game_state.console_logs.push(format!("> {}", command));

                // Process simple commands
                match command.as_str() {
                    "clear" => {
                        game_state.console_logs.clear();
                        game_state
                            .console_logs
                            .push("[INFO] Console cleared".to_string());
                    }
                    "help" => {
                        game_state.console_logs.push(
                            "[INFO] Available commands: clear, help, fps, version".to_string(),
                        );
                    }
                    "fps" => {
                        game_state
                            .console_logs
                            .push(format!("[INFO] Current FPS: {:.1}", game_state.fps));
                    }
                    "version" => {
                        game_state
                            .console_logs
                            .push("[INFO] Game Engine v1.0.0".to_string());
                    }
                    _ => {
                        game_state
                            .console_logs
                            .push(format!("[ERROR] Unknown command: {}", command));
                    }
                }

                game_state.console_input.clear();
            }
        });
}

/// Render the asset browser panel (already handled by Project panel)
fn render_asset_browser(ui: &Ui, game_state: &mut GameEngineState) {
    ui.window("Asset Browser")
        .size([300.0, 300.0], Condition::FirstUseEver)
        .build(|| {
            // Current folder path
            ui.text(format!("[Folder] {}", game_state.current_folder));
            ui.separator();

            // Navigation buttons
            if ui.button("↑ Up") && game_state.current_folder != "Assets/" {
                game_state.current_folder = "Assets/".to_string();
            }
            ui.same_line();
            if ui.button("⟳ Refresh") {
                game_state
                    .console_logs
                    .push("[INFO] Asset browser refreshed".to_string());
            }

            ui.separator();

            // Asset grid
            let button_size = [80.0, 80.0];
            let mut items_per_row = (ui.content_region_avail()[0] / (button_size[0] + 8.0)) as i32;
            if items_per_row < 1 {
                items_per_row = 1;
            }

            let aquery = game_state.asset_search.to_lowercase();
            for (i, asset) in game_state.assets.iter().enumerate() {
                if !aquery.is_empty() && !asset.to_lowercase().contains(&aquery) {
                    continue;
                }
                if i > 0 && (i as i32) % items_per_row != 0 {
                    ui.same_line();
                }

                let is_folder = asset.ends_with('/');
                let icon = if is_folder { "[DIR]" } else { "[FILE]" };
                let display_name = if is_folder {
                    asset.trim_end_matches('/')
                } else {
                    asset
                };

                if ui.button_with_size(format!("{}\n{}", icon, display_name), button_size) {
                    if is_folder {
                        game_state.current_folder = format!("Assets/{}", asset);
                        game_state
                            .console_logs
                            .push(format!("[INFO] Opened folder: {}", asset));
                    } else {
                        game_state
                            .console_logs
                            .push(format!("[INFO] Selected asset: {}", asset));
                    }
                }

                // Right-click context menu for asset browser items
                if let Some(_popup) = ui.begin_popup_context_item() {
                    if ui.menu_item("Import") {
                        game_state
                            .console_logs
                            .push(format!("[INFO] Importing {}", asset));
                    }
                    if ui.menu_item("Delete") {
                        game_state
                            .console_logs
                            .push(format!("[WARNING] Deleted {}", asset));
                    }
                    ui.separator();
                    ui.menu_item("Properties");
                }
            }
        });
}

/// Render the performance stats panel
#[cfg(feature = "implot")]
fn render_performance(ui: &Ui, plot_ui: &PlotUi, game_state: &mut GameEngineState) {
    ui.window("Performance")
        .size([350.0, 600.0], Condition::FirstUseEver) // Increased height to show full graph
        .build(|| {
            ui.text("Performance Statistics");
            ui.separator();

            // Update real performance data
            game_state.fps = ui.io().framerate();
            game_state.frame_time = 1000.0 / game_state.fps;
            // Keep fake data for draw calls and vertices (would come from real renderer)
            game_state.draw_calls = 45 + ((ui.time() * 0.5).sin() as f32 * 10.0) as u32;
            game_state.vertices = 12543 + ((ui.time() * 0.3).cos() as f32 * 1000.0) as u32;

            // Update FPS history with timestamp
            let current_time = ui.time();
            game_state.fps_history.push((current_time, game_state.fps));

            // Keep last 5 seconds of data (assuming ~60 FPS = 300 samples)
            let history_duration = 5.0; // seconds
            while !game_state.fps_history.is_empty() {
                if current_time - game_state.fps_history[0].0 > history_duration {
                    game_state.fps_history.remove(0);
                } else {
                    break;
                }
            }

            ui.text(format!("FPS: {:.1}", game_state.fps));
            ui.text(format!("Frame Time: {:.2}ms", game_state.frame_time));
            ui.text(format!("Draw Calls: {}", game_state.draw_calls));
            ui.text(format!("Vertices: {}", game_state.vertices));

            ui.separator();

            // Memory usage (fake data)
            let memory_used = 256.0 + (ui.time() * 0.1).sin() as f32 * 50.0;
            ui.text(format!("Memory: {:.1}MB", memory_used));

            ui.separator();

            // FPS Graph using ImPlot
            ui.text("FPS Graph (Last 5s):");

            if !game_state.fps_history.is_empty() {
                // Extract x (time) and y (fps) data
                let x_data: Vec<f64> = game_state.fps_history.iter().map(|(t, _)| *t).collect();
                let y_data: Vec<f64> = game_state
                    .fps_history
                    .iter()
                    .map(|(_, fps)| *fps as f64)
                    .collect();

                // Create plot with fixed X axis range (scrolling window)
                if let Some(token) = plot_ui.begin_plot("##FPS") {
                    // Set axes labels and limits
                    plot_ui.setup_axes(
                        Some("Time (s)"),
                        Some("FPS"),
                        dear_implot::AxisFlags::NONE,
                        dear_implot::AxisFlags::NONE,
                    );

                    // Set axes limits with scrolling window
                    plot_ui.setup_axes_limits(
                        current_time - history_duration, // x_min
                        current_time,                    // x_max
                        40.0,                            // y_min
                        80.0,                            // y_max
                        dear_implot::PlotCond::Always,
                    );

                    LinePlot::new("FPS", &x_data, &y_data).plot();
                    token.end();
                }
            } else {
                ui.text("Collecting data...");
            }
        });
}

#[cfg(not(feature = "implot"))]
fn render_performance(ui: &Ui, game_state: &mut GameEngineState) {
    ui.window("Performance")
        .size([250.0, 200.0], Condition::FirstUseEver)
        .build(|| {
            ui.text("Performance Statistics");
            ui.separator();

            // Update real performance data
            game_state.fps = ui.io().framerate();
            game_state.frame_time = 1000.0 / game_state.fps;
            // Keep fake data for draw calls and vertices (would come from real renderer)
            game_state.draw_calls = 45 + ((ui.time() * 0.5).sin() as f32 * 10.0) as u32;
            game_state.vertices = 12543 + ((ui.time() * 0.3).cos() as f32 * 1000.0) as u32;

            // Update FPS history with timestamp (even without ImPlot, for consistency)
            let current_time = ui.time();
            game_state.fps_history.push((current_time, game_state.fps));

            // Keep last 5 seconds of data
            let history_duration = 5.0;
            while !game_state.fps_history.is_empty() {
                if current_time - game_state.fps_history[0].0 > history_duration {
                    game_state.fps_history.remove(0);
                } else {
                    break;
                }
            }

            ui.text(format!("FPS: {:.1}", game_state.fps));
            ui.text(format!("Frame Time: {:.2}ms", game_state.frame_time));
            ui.text(format!("Draw Calls: {}", game_state.draw_calls));
            ui.text(format!("Vertices: {}", game_state.vertices));

            ui.separator();

            // Memory usage (fake data)
            let memory_used = 256.0 + (ui.time() * 0.1).sin() as f32 * 50.0;
            ui.text(format!("Memory: {:.1}MB", memory_used));

            ui.separator();
            ui.text("FPS Graph:");
            ui.text("(Enable 'implot' feature to see graph)");
        });
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let mut window = AppWindow::setup_gpu(event_loop);
            window.setup_imgui();
            // Request initial redraw to start the render loop
            window.window.request_redraw();
            self.window = Some(window);
            println!("Window created successfully");
            #[cfg(feature = "multi-viewport")]
            {
                if let Some(app) = self.window.as_mut() {
                    if let Some(imgui) = app.imgui.as_mut() {
                        if imgui.enable_viewports {
                            // Install platform (winit) viewport handlers first.
                            winit_mvp::init_multi_viewport_support(&mut imgui.context, &app.window);
                            // Then install renderer viewport callbacks.
                            wgpu_mvp::enable(&mut imgui.renderer, &mut imgui.context);
                        }
                    }
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(main_id) = self.window.as_ref().map(|w| w.window.id()) else {
            return;
        };
        let is_main_window = window_id == main_id;

        match &event {
            WindowEvent::CloseRequested if is_main_window => {
                println!("Close requested");
                self.exit(event_loop);
                return;
            }
            WindowEvent::KeyboardInput { event, .. }
                if is_main_window && event.logical_key == Key::Named(NamedKey::Escape) =>
            {
                println!("Escape pressed, exiting");
                self.exit(event_loop);
                return;
            }
            _ => {}
        }

        let Some(window) = self.window.as_mut() else {
            return;
        };

        // Route input to main + secondary windows when multi-viewport is enabled.
        #[cfg(feature = "multi-viewport")]
        {
            let full: Event<()> = Event::WindowEvent {
                window_id,
                event: event.clone(),
            };
            let imgui = window.imgui.as_mut().unwrap();
            let _ = winit_mvp::handle_event_with_multi_viewport(
                &mut imgui.platform,
                &mut imgui.context,
                &window.window,
                &full,
            );
        }
        #[cfg(not(feature = "multi-viewport"))]
        {
            let imgui = window.imgui.as_mut().unwrap();
            let _ = imgui
                .platform
                .handle_window_event(&mut imgui.context, &window.window, &event);
        }

        match event {
            WindowEvent::Resized(physical_size) => {
                if is_main_window && physical_size.width > 0 && physical_size.height > 0 {
                    window.surface_desc.width = physical_size.width;
                    window.surface_desc.height = physical_size.height;
                    window
                        .surface
                        .configure(&window.device, &window.surface_desc);
                    window.window.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if is_main_window {
                    let size = window.window.inner_size();
                    if size.width > 0 && size.height > 0 {
                        window.surface_desc.width = size.width;
                        window.surface_desc.height = size.height;
                        window
                            .surface
                            .configure(&window.device, &window.surface_desc);
                        window.window.request_redraw();
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if is_main_window {
                    #[cfg(feature = "multi-viewport")]
                    let _el_guard = if window
                        .imgui
                        .as_ref()
                        .map(|i| i.enable_viewports)
                        .unwrap_or(false)
                    {
                        Some(winit_mvp::set_event_loop_for_frame(event_loop))
                    } else {
                        None
                    };

                    let render_result = window.render();
                    match render_result {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                            eprintln!("Surface lost/outdated, reconfiguring");
                            window
                                .surface
                                .configure(&window.device, &window.surface_desc);
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => {
                            eprintln!("OutOfMemory");
                            event_loop.exit();
                        }
                        Err(wgpu::SurfaceError::Timeout) => {
                            eprintln!("Surface timeout");
                        }
                        Err(wgpu::SurfaceError::Other) => {
                            eprintln!("Other surface error occurred");
                        }
                    }
                    window.window.request_redraw();
                }
            }
            _ => {}
        }

        if is_main_window {
            let imgui = window.imgui.as_mut().unwrap();
            if !imgui.context.io().want_capture_mouse()
                && !imgui.context.io().want_capture_keyboard()
            {
                // Handle game input here when not captured by ImGui
            }
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

    println!("=== Game Engine Docking Demo ===");
    println!("Features:");
    println!("  * Scene Hierarchy - Manage game objects");
    println!("  * Inspector - Edit object properties");
    println!("  * Viewport - 3D scene view with controls");
    println!("  * Console - Command input and logging");
    println!("  * Asset Browser - File management");
    println!("  * Performance Stats - Real-time metrics");
    println!();
    println!("Controls:");
    println!("  * Drag panel tabs to rearrange layout");
    println!("  * Right-click on objects for context menus");
    println!("  * Use console commands: help, clear, fps, version");
    println!("  * Press ESC to exit");
    println!();

    event_loop.run_app(&mut app).unwrap();
}
