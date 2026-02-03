//! Vulkan helpers (pipelines, descriptor sets, uploads).
//!
//! This module is inspired by `imgui-rs-vulkan-renderer`, adapted to `dear-imgui-rs`.

use crate::{Options, RendererError, RendererResult};
use ash::{Device, vk};
use std::ffi::CString;

use super::allocator::{Allocate, Allocator, Memory};
use super::shaders::{FRAG_SPV, VERT_SPV};

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct PushConstants {
    pub ortho: [f32; 16],
    pub gamma_pad: [f32; 4],
}

/// Return a `&[u8]` for any sized object passed in.
pub(crate) unsafe fn any_as_u8_slice<T: Sized>(any: &T) -> &[u8] {
    let ptr = (any as *const T).cast::<u8>();
    unsafe { std::slice::from_raw_parts(ptr, std::mem::size_of::<T>()) }
}

pub(crate) fn ortho_matrix_vk(display_pos: [f32; 2], display_size: [f32; 2]) -> [f32; 16] {
    let l = display_pos[0];
    let r = display_pos[0] + display_size[0];
    let b = display_pos[1];
    let t = display_pos[1] + display_size[1];

    let sx = 2.0 / (r - l);
    let sy = 2.0 / (t - b);
    let tx = (r + l) / (l - r);
    let ty = (t + b) / (b - t);

    // Column-major 4x4 matrix
    [
        sx, 0.0, 0.0, 0.0, //
        0.0, sy, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, //
        tx, ty, 0.0, 1.0, //
    ]
}

pub(crate) fn clip_rect_to_scissor(
    clip_rect: [f32; 4],
    clip_off: [f32; 2],
    clip_scale: [f32; 2],
    fb_width: u32,
    fb_height: u32,
) -> Option<vk::Rect2D> {
    let clip_rect = [
        (clip_rect[0] - clip_off[0]) * clip_scale[0],
        (clip_rect[1] - clip_off[1]) * clip_scale[1],
        (clip_rect[2] - clip_off[0]) * clip_scale[0],
        (clip_rect[3] - clip_off[1]) * clip_scale[1],
    ];

    if clip_rect[0] >= fb_width as f32
        || clip_rect[1] >= fb_height as f32
        || clip_rect[2] <= 0.0
        || clip_rect[3] <= 0.0
    {
        return None;
    }

    let x0 = clip_rect[0].max(0.0).floor() as i32;
    let y0 = clip_rect[1].max(0.0).floor() as i32;
    let x1 = clip_rect[2].min(fb_width as f32).ceil() as i32;
    let y1 = clip_rect[3].min(fb_height as f32).ceil() as i32;

    let w = (x1 - x0).max(0) as u32;
    let h = (y1 - y0).max(0) as u32;
    if w == 0 || h == 0 {
        return None;
    }

    Some(vk::Rect2D {
        offset: vk::Offset2D { x: x0, y: y0 },
        extent: vk::Extent2D {
            width: w,
            height: h,
        },
    })
}

/// Create a descriptor set layout compatible with the graphics pipeline.
pub fn create_vulkan_descriptor_set_layout(
    device: &Device,
) -> RendererResult<vk::DescriptorSetLayout> {
    let bindings = [vk::DescriptorSetLayoutBinding::default()
        .binding(0)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .descriptor_count(1)
        .stage_flags(vk::ShaderStageFlags::FRAGMENT)];

    let create_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
    unsafe { Ok(device.create_descriptor_set_layout(&create_info, None)?) }
}

pub fn create_vulkan_pipeline_layout(
    device: &Device,
    descriptor_set_layout: vk::DescriptorSetLayout,
) -> RendererResult<vk::PipelineLayout> {
    let push_const_range = [vk::PushConstantRange {
        stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
        offset: 0,
        size: std::mem::size_of::<PushConstants>() as u32,
    }];

    let set_layouts = [descriptor_set_layout];
    let layout_info = vk::PipelineLayoutCreateInfo::default()
        .set_layouts(&set_layouts)
        .push_constant_ranges(&push_const_range);
    let pipeline_layout = unsafe { device.create_pipeline_layout(&layout_info, None)? };
    Ok(pipeline_layout)
}

pub fn create_vulkan_pipeline(
    device: &Device,
    pipeline_layout: vk::PipelineLayout,
    #[cfg(not(feature = "dynamic-rendering"))] render_pass: vk::RenderPass,
    #[cfg(feature = "dynamic-rendering")] dynamic_rendering: super::DynamicRendering,
    options: Options,
) -> RendererResult<vk::Pipeline> {
    let entry_point_name = CString::new("main").unwrap();

    let vertex_create_info = vk::ShaderModuleCreateInfo::default().code(VERT_SPV);
    let vertex_module = unsafe { device.create_shader_module(&vertex_create_info, None)? };

    let fragment_create_info = vk::ShaderModuleCreateInfo::default().code(FRAG_SPV);
    let fragment_module = unsafe { device.create_shader_module(&fragment_create_info, None)? };

    let shader_states_infos = [
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vertex_module)
            .name(&entry_point_name),
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(fragment_module)
            .name(&entry_point_name),
    ];

    let binding_desc = [vk::VertexInputBindingDescription::default()
        .binding(0)
        .stride(20)
        .input_rate(vk::VertexInputRate::VERTEX)];
    let attribute_desc = [
        vk::VertexInputAttributeDescription::default()
            .binding(0)
            .location(0)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(0),
        vk::VertexInputAttributeDescription::default()
            .binding(0)
            .location(1)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(8),
        vk::VertexInputAttributeDescription::default()
            .binding(0)
            .location(2)
            .format(vk::Format::R8G8B8A8_UNORM)
            .offset(16),
    ];
    let vertex_input_info = vk::PipelineVertexInputStateCreateInfo::default()
        .vertex_binding_descriptions(&binding_desc)
        .vertex_attribute_descriptions(&attribute_desc);

    let input_assembly_info = vk::PipelineInputAssemblyStateCreateInfo::default()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
        .primitive_restart_enable(false);

    let viewport_info = vk::PipelineViewportStateCreateInfo::default()
        .viewport_count(1)
        .scissor_count(1);

    let rasterizer_info = vk::PipelineRasterizationStateCreateInfo::default()
        .depth_clamp_enable(false)
        .rasterizer_discard_enable(false)
        .polygon_mode(vk::PolygonMode::FILL)
        .line_width(1.0)
        .cull_mode(vk::CullModeFlags::NONE)
        .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
        .depth_bias_enable(false);

    let multisampling_info = vk::PipelineMultisampleStateCreateInfo::default()
        .sample_shading_enable(false)
        .rasterization_samples(options.sample_count);

    let color_blend_attachments = [vk::PipelineColorBlendAttachmentState::default()
        .color_write_mask(vk::ColorComponentFlags::RGBA)
        .blend_enable(true)
        .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
        .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .color_blend_op(vk::BlendOp::ADD)
        .src_alpha_blend_factor(vk::BlendFactor::ONE)
        .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .alpha_blend_op(vk::BlendOp::ADD)];
    let color_blending_info = vk::PipelineColorBlendStateCreateInfo::default()
        .logic_op_enable(false)
        .logic_op(vk::LogicOp::COPY)
        .attachments(&color_blend_attachments)
        .blend_constants([0.0, 0.0, 0.0, 0.0]);

    let depth_stencil_state_create_info = vk::PipelineDepthStencilStateCreateInfo::default()
        .depth_test_enable(options.enable_depth_test)
        .depth_write_enable(options.enable_depth_write)
        .depth_compare_op(vk::CompareOp::ALWAYS)
        .depth_bounds_test_enable(false)
        .stencil_test_enable(false);

    let dynamic_states = [vk::DynamicState::SCISSOR, vk::DynamicState::VIEWPORT];
    let dynamic_states_info =
        vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

    let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
        .stages(&shader_states_infos)
        .vertex_input_state(&vertex_input_info)
        .input_assembly_state(&input_assembly_info)
        .rasterization_state(&rasterizer_info)
        .viewport_state(&viewport_info)
        .multisample_state(&multisampling_info)
        .color_blend_state(&color_blending_info)
        .depth_stencil_state(&depth_stencil_state_create_info)
        .dynamic_state(&dynamic_states_info)
        .layout(pipeline_layout)
        .subpass(options.subpass);

    #[cfg(not(feature = "dynamic-rendering"))]
    let pipeline_info = pipeline_info.render_pass(render_pass);

    #[cfg(feature = "dynamic-rendering")]
    let color_attachment_formats = [dynamic_rendering.color_attachment_format];
    #[cfg(feature = "dynamic-rendering")]
    let mut rendering_info = {
        let mut rendering_info = vk::PipelineRenderingCreateInfo::default()
            .color_attachment_formats(&color_attachment_formats);
        if let Some(depth_attachment_format) = dynamic_rendering.depth_attachment_format {
            rendering_info = rendering_info.depth_attachment_format(depth_attachment_format);
        }
        rendering_info
    };
    #[cfg(feature = "dynamic-rendering")]
    let pipeline_info = pipeline_info.push_next(&mut rendering_info);

    let pipeline = unsafe {
        device
            .create_graphics_pipelines(
                vk::PipelineCache::null(),
                std::slice::from_ref(&pipeline_info),
                None,
            )
            .map_err(|e| e.1)?[0]
    };

    unsafe {
        device.destroy_shader_module(vertex_module, None);
        device.destroy_shader_module(fragment_module, None);
    }

    Ok(pipeline)
}

/// Create a descriptor pool of sets compatible with the graphics pipeline.
pub fn create_vulkan_descriptor_pool(
    device: &Device,
    max_sets: u32,
) -> RendererResult<vk::DescriptorPool> {
    let sizes = [vk::DescriptorPoolSize {
        ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
        descriptor_count: max_sets,
    }];
    let create_info = vk::DescriptorPoolCreateInfo::default()
        .pool_sizes(&sizes)
        .max_sets(max_sets)
        .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET);
    unsafe { Ok(device.create_descriptor_pool(&create_info, None)?) }
}

/// Create a descriptor set compatible with the graphics pipeline from a texture.
pub fn create_vulkan_descriptor_set(
    device: &Device,
    set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    image_view: vk::ImageView,
    sampler: vk::Sampler,
) -> RendererResult<vk::DescriptorSet> {
    let set = {
        let set_layouts = [set_layout];
        let allocate_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&set_layouts);
        unsafe { device.allocate_descriptor_sets(&allocate_info)?[0] }
    };

    unsafe {
        let image_info = [vk::DescriptorImageInfo {
            sampler,
            image_view,
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        }];
        let write_desc_sets = [vk::WriteDescriptorSet::default()
            .dst_set(set)
            .dst_binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .image_info(&image_info)];
        device.update_descriptor_sets(&write_desc_sets, &[]);
    }

    Ok(set)
}

pub(crate) fn create_and_fill_buffer<T: Copy>(
    device: &Device,
    allocator: &mut Allocator,
    data: &[T],
    usage: vk::BufferUsageFlags,
) -> RendererResult<(vk::Buffer, Memory)> {
    let size = std::mem::size_of_val(data);
    let (buffer, mut memory) = allocator.create_buffer(device, size, usage)?;
    allocator.update_buffer(device, &mut memory, data)?;
    Ok((buffer, memory))
}

pub(crate) fn execute_one_time_commands<R, F: FnOnce(vk::CommandBuffer) -> R>(
    device: &Device,
    queue: vk::Queue,
    pool: vk::CommandPool,
    executor: F,
) -> RendererResult<R> {
    let command_buffer = {
        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_pool(pool)
            .command_buffer_count(1);
        unsafe { device.allocate_command_buffers(&alloc_info)?[0] }
    };
    let command_buffers = [command_buffer];

    let begin_info =
        vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    unsafe { device.begin_command_buffer(command_buffer, &begin_info)? };

    let executor_result = executor(command_buffer);

    unsafe { device.end_command_buffer(command_buffer)? };

    let submit_info = vk::SubmitInfo::default().command_buffers(&command_buffers);
    unsafe {
        let fence = device.create_fence(&vk::FenceCreateInfo::default(), None)?;
        device.queue_submit(queue, &[submit_info], fence)?;
        device.wait_for_fences(&[fence], true, u64::MAX)?;
        device.destroy_fence(fence, None);
        device.free_command_buffers(pool, &command_buffers);
    }

    Ok(executor_result)
}

pub(crate) struct Texture {
    pub image: vk::Image,
    pub image_mem: Memory,
    pub image_view: vk::ImageView,
    pub sampler: vk::Sampler,
}

impl Texture {
    /// Create a GPU image, view, sampler, and a staging buffer filled with `pixels_rgba`.
    pub fn create(
        device: &Device,
        allocator: &mut Allocator,
        width: u32,
        height: u32,
        format: vk::Format,
        pixels_rgba: &[u8],
    ) -> RendererResult<(Self, vk::Buffer, Memory)> {
        let (image, image_mem) = allocator.create_image(device, width, height, format)?;

        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|v| v.checked_mul(4))
            .ok_or_else(|| RendererError::Allocator("texture size overflow".into()))?;
        if pixels_rgba.len() < expected {
            return Err(RendererError::Allocator(
                "texture pixel buffer too small".into(),
            ));
        }

        let (buffer, buffer_mem) = create_and_fill_buffer(
            device,
            allocator,
            &pixels_rgba[..expected],
            vk::BufferUsageFlags::TRANSFER_SRC,
        )?;

        let image_view = {
            let create_info = vk::ImageViewCreateInfo::default()
                .image(image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(format)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });
            unsafe { device.create_image_view(&create_info, None)? }
        };

        let sampler = {
            let sampler_info = vk::SamplerCreateInfo::default()
                .mag_filter(vk::Filter::LINEAR)
                .min_filter(vk::Filter::LINEAR)
                .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                .anisotropy_enable(false)
                .max_anisotropy(1.0)
                .border_color(vk::BorderColor::INT_OPAQUE_BLACK)
                .unnormalized_coordinates(false)
                .compare_enable(false)
                .compare_op(vk::CompareOp::ALWAYS)
                .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
                .mip_lod_bias(0.0)
                .min_lod(0.0)
                .max_lod(1.0);
            unsafe { device.create_sampler(&sampler_info, None)? }
        };

        Ok((
            Self {
                image,
                image_mem,
                image_view,
                sampler,
            },
            buffer,
            buffer_mem,
        ))
    }

    pub fn upload(
        &self,
        device: &Device,
        command_buffer: vk::CommandBuffer,
        buffer: vk::Buffer,
        width: u32,
        height: u32,
    ) {
        upload_buffer_to_image(
            device,
            command_buffer,
            buffer,
            self.image,
            0,
            0,
            width,
            height,
            vk::ImageLayout::UNDEFINED,
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::AccessFlags::empty(),
        );
    }
}

pub(crate) fn upload_rgba_subrect_to_image(
    device: &Device,
    command_buffer: vk::CommandBuffer,
    buffer: vk::Buffer,
    image: vk::Image,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) {
    upload_buffer_to_image(
        device,
        command_buffer,
        buffer,
        image,
        x,
        y,
        width,
        height,
        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        vk::PipelineStageFlags::FRAGMENT_SHADER,
        vk::AccessFlags::SHADER_READ,
    );
}

fn upload_buffer_to_image(
    device: &Device,
    command_buffer: vk::CommandBuffer,
    buffer: vk::Buffer,
    image: vk::Image,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    old_layout: vk::ImageLayout,
    src_stage: vk::PipelineStageFlags,
    src_access: vk::AccessFlags,
) {
    let mut barrier = vk::ImageMemoryBarrier::default()
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(image)
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        });

    unsafe {
        barrier.old_layout = old_layout;
        barrier.new_layout = vk::ImageLayout::TRANSFER_DST_OPTIMAL;
        barrier.src_access_mask = src_access;
        barrier.dst_access_mask = vk::AccessFlags::TRANSFER_WRITE;

        device.cmd_pipeline_barrier(
            command_buffer,
            src_stage,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier],
        );

        let region = vk::BufferImageCopy::default()
            .buffer_offset(0)
            .buffer_row_length(0)
            .buffer_image_height(0)
            .image_subresource(vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            })
            .image_offset(vk::Offset3D {
                x: x as i32,
                y: y as i32,
                z: 0,
            })
            .image_extent(vk::Extent3D {
                width,
                height,
                depth: 1,
            });
        device.cmd_copy_buffer_to_image(
            command_buffer,
            buffer,
            image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &[region],
        );

        barrier.old_layout = vk::ImageLayout::TRANSFER_DST_OPTIMAL;
        barrier.new_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        barrier.src_access_mask = vk::AccessFlags::TRANSFER_WRITE;
        barrier.dst_access_mask = vk::AccessFlags::SHADER_READ;

        device.cmd_pipeline_barrier(
            command_buffer,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::FRAGMENT_SHADER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier],
        );
    }
}
