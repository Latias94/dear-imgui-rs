use glam::Mat4;
use std::{borrow::Cow, num::NonZeroU64};

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

pub(crate) struct SceneRenderer {
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

impl SceneRenderer {
    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
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
            bind_group_layouts: &[Some(&uniform_layout)],
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
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: Default::default(),
            bias: Default::default(),
        };
        let depth_state_cube = wgpu::DepthStencilState {
            format: depth_format,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: Default::default(),
            bias: Default::default(),
        };

        let grid_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("simple_scene_grid_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(SceneVertex::layout())],
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
                buffers: &[Some(SceneVertex::layout())],
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

pub(crate) struct RenderTarget {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    _depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    texture_id: Option<dear_imgui_wgpu::ExternalTextureId>,
}

impl RenderTarget {
    pub(crate) fn create(device: &wgpu::Device, format: wgpu::TextureFormat, label: &str) -> Self {
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

        Self {
            _texture: texture,
            view,
            _depth_texture: depth_texture,
            depth_view,
            texture_id: None,
        }
    }

    pub(crate) fn register_with<E>(
        &mut self,
        register: impl FnOnce(&wgpu::TextureView) -> Result<dear_imgui_wgpu::ExternalTextureId, E>,
    ) -> Result<(), E> {
        if self.texture_id.is_none() {
            self.texture_id = Some(register(&self.view)?);
        }
        Ok(())
    }

    pub(crate) fn texture_id(&self) -> Option<dear_imgui_rs::TextureId> {
        self.texture_id.map(|texture| texture.texture_id())
    }

    pub(crate) fn unregister_with<E>(
        &mut self,
        unregister: impl FnOnce(dear_imgui_wgpu::ExternalTextureId) -> Result<(), E>,
    ) -> Result<(), E> {
        let Some(texture_id) = self.texture_id else {
            return Ok(());
        };
        unregister(texture_id)?;
        self.texture_id = None;
        Ok(())
    }

    pub(crate) fn render_into(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        scene_renderer: &SceneRenderer,
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
