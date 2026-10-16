//! A deliberately bounded inspector: three source formats, two layers, three mips.
//!
//! The UI only receives RGBA8 views. Depth, integer IDs, and HDR values are interpreted by an
//! explicit GPU conversion pass; registering their original views as UI textures is invalid.

use std::{error::Error, io, sync::mpsc, time::Duration};

use wgpu::util::DeviceExt;

pub const LAYERS: u32 = 2;
pub const MIPS: u32 = 3;
pub const LABELS: [&str; 3] = ["Depth32Float", "R32Uint object IDs", "Rgba16Float HDR"];

#[derive(Clone, Copy, Debug)]
pub struct Selection {
    pub layer: u32,
    pub mip: u32,
    pub exposure: f32,
    pub depth_min: f32,
    pub depth_max: f32,
}

impl Default for Selection {
    fn default() -> Self {
        Self {
            layer: 0,
            mip: 0,
            exposure: 0.0,
            depth_min: 0.0,
            depth_max: 1.0,
        }
    }
}

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct Parameters {
    width: u32,
    height: u32,
    layer: u32,
    mip: u32,
    exposure: f32,
    depth_min: f32,
    depth_max: f32,
    padding: u32,
}

struct Subresource {
    depth: wgpu::TextureView,
    producer: wgpu::BindGroup,
    depth_parameters: wgpu::BindGroup,
}

pub struct TexturePreview {
    pub side: u32,
    sources: [wgpu::Texture; 3],
    outputs: [wgpu::Texture; 3],
    views: [wgpu::TextureView; 3],
    subresources: Vec<Subresource>,
    producer: wgpu::ComputePipeline,
    depth_producer: wgpu::RenderPipeline,
    converter: wgpu::ComputePipeline,
    conversion_layout: wgpu::BindGroupLayout,
}

fn uniform_entry(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn storage_entry(binding: u32, format: wgpu::TextureFormat) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format,
            view_dimension: wgpu::TextureViewDimension::D2,
        },
        count: None,
    }
}

fn texture_entry(binding: u32, sample_type: wgpu::TextureSampleType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Texture {
            sample_type,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn subresource(texture: &wgpu::Texture, layer: u32, mip: u32) -> wgpu::TextureView {
    texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2),
        base_array_layer: layer,
        array_layer_count: Some(1),
        base_mip_level: mip,
        mip_level_count: Some(1),
        ..Default::default()
    })
}

impl TexturePreview {
    pub fn new(device: &wgpu::Device, side: u32) -> Self {
        assert!(side.is_power_of_two() && (16..=512).contains(&side));
        let make_source = |format, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("inspector source"),
                size: wgpu::Extent3d {
                    width: side,
                    height: side,
                    depth_or_array_layers: LAYERS,
                },
                mip_level_count: MIPS,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let sources = [
            make_source(
                wgpu::TextureFormat::Depth32Float,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            ),
            make_source(
                wgpu::TextureFormat::R32Uint,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            ),
            make_source(
                wgpu::TextureFormat::Rgba16Float,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            ),
        ];
        let outputs = std::array::from_fn(|_| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("inspector RGBA8 preview"),
                size: wgpu::Extent3d {
                    width: side,
                    height: side,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::STORAGE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        });
        let views = outputs
            .each_ref()
            .map(|texture| texture.create_view(&Default::default()));
        let source_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("inspector real-format producers"),
            source: wgpu::ShaderSource::Wgsl(include_str!("texture_preview_source.wgsl").into()),
        });
        let producer_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("inspector source bindings"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::COMPUTE),
                storage_entry(1, wgpu::TextureFormat::R32Uint),
                storage_entry(2, wgpu::TextureFormat::Rgba16Float),
            ],
        });
        let producer_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[Some(&producer_layout)],
                immediate_size: 0,
            });
        let producer = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("inspector integer and HDR producer"),
            layout: Some(&producer_pipeline_layout),
            module: &source_module,
            entry_point: Some("produce"),
            compilation_options: Default::default(),
            cache: None,
        });
        let depth_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("inspector depth parameters"),
            entries: &[uniform_entry(0, wgpu::ShaderStages::FRAGMENT)],
        });
        let depth_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[Some(&depth_layout)],
                immediate_size: 0,
            });
        let depth_producer = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("inspector depth ramp producer"),
            layout: Some(&depth_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &source_module,
                entry_point: Some("fullscreen"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &source_module,
                entry_point: Some("depth_ramp"),
                compilation_options: Default::default(),
                targets: &[],
            }),
            multiview_mask: None,
            cache: None,
        });
        let mut subresources = Vec::new();
        for layer in 0..LAYERS {
            for mip in 0..MIPS {
                let parameters = Parameters {
                    width: side >> mip,
                    height: side >> mip,
                    layer,
                    mip,
                    exposure: 0.0,
                    depth_min: 0.0,
                    depth_max: 1.0,
                    padding: 0,
                };
                let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("inspector source parameters"),
                    contents: bytemuck::bytes_of(&parameters),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
                let id_view = subresource(&sources[1], layer, mip);
                let hdr_view = subresource(&sources[2], layer, mip);
                subresources.push(Subresource {
                    depth: subresource(&sources[0], layer, mip),
                    producer: device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: None,
                        layout: &producer_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: buffer.as_entire_binding(),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::TextureView(&id_view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: wgpu::BindingResource::TextureView(&hdr_view),
                            },
                        ],
                    }),
                    depth_parameters: device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: None,
                        layout: &depth_layout,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: buffer.as_entire_binding(),
                        }],
                    }),
                });
            }
        }
        let conversion_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("inspector explicit conversion bindings"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::COMPUTE),
                texture_entry(1, wgpu::TextureSampleType::Depth),
                texture_entry(2, wgpu::TextureSampleType::Uint),
                texture_entry(3, wgpu::TextureSampleType::Float { filterable: false }),
                storage_entry(4, wgpu::TextureFormat::Rgba8Unorm),
                storage_entry(5, wgpu::TextureFormat::Rgba8Unorm),
                storage_entry(6, wgpu::TextureFormat::Rgba8Unorm),
            ],
        });
        let conversion_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("inspector explicit format conversion"),
            source: wgpu::ShaderSource::Wgsl(include_str!("texture_preview_convert.wgsl").into()),
        });
        let conversion_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[Some(&conversion_layout)],
                immediate_size: 0,
            });
        let converter = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("inspector RGBA8 conversion"),
            layout: Some(&conversion_pipeline_layout),
            module: &conversion_module,
            entry_point: Some("convert"),
            compilation_options: Default::default(),
            cache: None,
        });
        Self {
            side,
            sources,
            outputs,
            views,
            subresources,
            producer,
            depth_producer,
            converter,
            conversion_layout,
        }
    }

    pub fn views(&self) -> &[wgpu::TextureView; 3] {
        &self.views
    }

    /// Record all producers before conversion; the caller then records its UI consumer.
    pub fn record(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        selection: Selection,
    ) {
        assert!(selection.layer < LAYERS && selection.mip < MIPS);
        for (index, resource) in self.subresources.iter().enumerate() {
            let mip = index as u32 % MIPS;
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("produce real depth ramp"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &resource.depth,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    ..Default::default()
                });
                pass.set_pipeline(&self.depth_producer);
                pass.set_bind_group(0, &resource.depth_parameters, &[]);
                pass.draw(0..3, 0..1);
            }
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("produce real integer and HDR textures"),
                ..Default::default()
            });
            pass.set_pipeline(&self.producer);
            pass.set_bind_group(0, &resource.producer, &[]);
            let groups = (self.side >> mip).div_ceil(8);
            pass.dispatch_workgroups(groups, groups, 1);
        }
        let parameters = Parameters {
            width: self.side,
            height: self.side,
            layer: selection.layer,
            mip: selection.mip,
            exposure: selection.exposure,
            depth_min: selection.depth_min,
            depth_max: selection.depth_max,
            padding: 0,
        };
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("inspector conversion parameters"),
            contents: bytemuck::bytes_of(&parameters),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let selected = self
            .sources
            .each_ref()
            .map(|texture| subresource(texture, selection.layer, selection.mip));
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("inspector selected layer and mip"),
            layout: &self.conversion_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&selected[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&selected[1]),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&selected[2]),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&self.views[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&self.views[1]),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&self.views[2]),
                },
            ],
        });
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("convert sources to UI-supported RGBA8"),
            ..Default::default()
        });
        pass.set_pipeline(&self.converter);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(self.side.div_ceil(8), self.side.div_ceil(8), 1);
    }
}

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// A required-adapter test: no adapter, validation failure, or readback failure is an error.
pub fn run_readback_tests() -> TestResult {
    pollster::block_on(async {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = match instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                force_fallback_adapter: true,
                ..Default::default()
            })
            .await
        {
            Ok(adapter) => adapter,
            Err(_) => instance.request_adapter(&Default::default()).await?,
        };
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await?;
        let preview = TexturePreview::new(&device, 64);
        let selections = [
            Selection::default(),
            Selection {
                layer: 1,
                ..Selection::default()
            },
            Selection {
                mip: 2,
                ..Selection::default()
            },
            Selection {
                layer: 1,
                mip: 2,
                exposure: 1.0,
                depth_min: 0.125,
                depth_max: 0.625,
            },
        ];
        for selection in selections {
            verify_readback(&device, &queue, &preview, selection)?;
        }
        // Replacement resources use the same shaders and subresource selection at a new extent.
        let resized = TexturePreview::new(&device, 128);
        verify_readback(&device, &queue, &resized, selections[3])?;
        println!(
            "texture inspector: depth/ID/HDR, layer/mip, exposure/depth range, and resize GPU readbacks passed on {}",
            adapter.get_info().name
        );
        Ok(())
    })
}

fn verify_readback(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    preview: &TexturePreview,
    selection: Selection,
) -> TestResult {
    let row_bytes = (preview.side * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let image_bytes = u64::from(row_bytes) * u64::from(preview.side);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("inspector required GPU readback"),
        size: image_bytes * 3,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    preview.record(device, &mut encoder, selection);
    for (index, output) in preview.outputs.iter().enumerate() {
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: output,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: image_bytes * index as u64,
                    bytes_per_row: Some(row_bytes),
                    rows_per_image: Some(preview.side),
                },
            },
            wgpu::Extent3d {
                width: preview.side,
                height: preview.side,
                depth_or_array_layers: 1,
            },
        );
    }
    let submission = queue.submit([encoder.finish()]);
    let (sender, receiver) = mpsc::sync_channel(1);
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
    device.poll(wgpu::PollType::Wait {
        submission_index: Some(submission),
        timeout: Some(Duration::from_secs(10)),
    })?;
    receiver.recv_timeout(Duration::from_secs(10))??;
    let bytes = buffer.slice(..).get_mapped_range()?;
    // Include interior pixels to exercise the ramp, integer palette and half-float conversion.
    for (x, y) in [
        (0, 0),
        (preview.side / 3, preview.side / 2),
        (preview.side - 1, preview.side - 1),
    ] {
        let expected = expected_pixels(preview.side, selection, x, y);
        for (index, expected) in expected.iter().enumerate() {
            let offset = index * image_bytes as usize + (y * row_bytes + x * 4) as usize;
            let actual = &bytes[offset..offset + 4];
            if actual
                .iter()
                .zip(expected)
                .any(|(&actual, &expected)| actual.abs_diff(expected) > 1)
            {
                return Err(io::Error::other(format!(
                    "{} {selection:?} ({x},{y}): expected {expected:?}, got {actual:?}",
                    LABELS[index]
                ))
                .into());
            }
        }
    }
    drop(bytes);
    buffer.unmap();
    Ok(())
}

fn expected_pixels(side: u32, selection: Selection, x: u32, y: u32) -> [[u8; 4]; 3] {
    let source_side = side >> selection.mip;
    let sx = x * source_side / side;
    let sy = y * source_side / side;
    let uvx = sx as f32 / (source_side - 1) as f32;
    let uvy = sy as f32 / (source_side - 1) as f32;
    let depth = 0.5 * uvx + 0.125 * selection.layer as f32 + 0.0625 * selection.mip as f32;
    let depth = ((depth - selection.depth_min) / (selection.depth_max - selection.depth_min))
        .clamp(0.0, 1.0);
    let byte = |value: f32| (value * 255.0).round() as u8;
    let id = ((sx / 8) + (sy / 8) * 3 + selection.layer * 5 + selection.mip * 7) % 8;
    let hdr = [
        4.0 * uvx + selection.layer as f32,
        2.0 * uvy + selection.mip as f32,
        0.5 + 0.5 * selection.layer as f32,
    ];
    let mapped = hdr.map(|value| {
        let exposed = value * selection.exposure.exp2();
        byte(exposed / (1.0 + exposed))
    });
    [
        [byte(depth), byte(depth), byte(depth), 255],
        [
            (id & 1) as u8 * 255,
            ((id >> 1) & 1) as u8 * 255,
            ((id >> 2) & 1) as u8 * 255,
            255,
        ],
        [mapped[0], mapped[1], mapped[2], 255],
    ]
}

#[cfg(test)]
mod tests {
    #[test]
    fn real_formats_convert_known_values_on_required_gpu() {
        super::run_readback_tests().expect("texture inspector requires an available WGPU adapter");
    }
}
