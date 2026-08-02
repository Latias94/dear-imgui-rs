//! Vulkan helpers (pipelines, descriptor sets, uploads).
//!
//! This module is inspired by `imgui-rs-vulkan-renderer`, adapted to `dear-imgui-rs`.

use crate::{Options, RendererError, RendererResult};
use ash::{Device, vk};
use std::ffi::CString;

use super::allocator::{Allocate, Allocator, Memory};
use super::shaders::{fragment_spirv, vertex_spirv};

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
    if fb_width == 0
        || fb_height == 0
        || !clip_rect
            .into_iter()
            .chain(clip_off)
            .chain(clip_scale)
            .all(f32::is_finite)
    {
        return None;
    }
    let clip_rect = [
        (clip_rect[0] - clip_off[0]) * clip_scale[0],
        (clip_rect[1] - clip_off[1]) * clip_scale[1],
        (clip_rect[2] - clip_off[0]) * clip_scale[0],
        (clip_rect[3] - clip_off[1]) * clip_scale[1],
    ];

    if clip_rect[2] <= clip_rect[0]
        || clip_rect[3] <= clip_rect[1]
        || clip_rect[0] >= fb_width as f32
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

pub(super) const MIN_SAMPLED_IMAGE_DESCRIPTORS: u32 = 8;
pub(super) const STANDARD_SAMPLER_DESCRIPTORS: u32 = 2;

#[derive(Clone, Copy)]
pub(crate) struct ImageUploadRegion {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

#[derive(Clone, Copy)]
struct ImageLayoutState {
    layout: vk::ImageLayout,
    stage: vk::PipelineStageFlags,
    access: vk::AccessFlags,
}

/// Create one descriptor set layout compatible with the graphics pipeline.
pub fn create_vulkan_descriptor_set_layout(
    device: &Device,
    descriptor_type: vk::DescriptorType,
) -> RendererResult<vk::DescriptorSetLayout> {
    let bindings = [vk::DescriptorSetLayoutBinding::default()
        .binding(0)
        .descriptor_type(descriptor_type)
        .descriptor_count(1)
        .stage_flags(vk::ShaderStageFlags::FRAGMENT)];

    let create_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
    unsafe { Ok(device.create_descriptor_set_layout(&create_info, None)?) }
}

pub fn create_vulkan_pipeline_layout(
    device: &Device,
    sampled_image_set_layout: vk::DescriptorSetLayout,
    sampler_set_layout: vk::DescriptorSetLayout,
) -> RendererResult<vk::PipelineLayout> {
    let push_const_range = [vk::PushConstantRange {
        stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
        offset: 0,
        size: std::mem::size_of::<PushConstants>() as u32,
    }];

    let set_layouts = [sampled_image_set_layout, sampler_set_layout];
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

    let vertex_spirv = vertex_spirv()?;
    let fragment_spirv = fragment_spirv()?;
    let vertex_create_info = vk::ShaderModuleCreateInfo::default().code(&vertex_spirv);
    let vertex_module = unsafe { device.create_shader_module(&vertex_create_info, None)? };

    let fragment_create_info = vk::ShaderModuleCreateInfo::default().code(&fragment_spirv);
    let fragment_module = match unsafe { device.create_shader_module(&fragment_create_info, None) }
    {
        Ok(fragment_module) => fragment_module,
        Err(err) => {
            unsafe { device.destroy_shader_module(vertex_module, None) };
            return Err(err.into());
        }
    };

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

    let pipeline = match unsafe {
        device.create_graphics_pipelines(
            vk::PipelineCache::null(),
            std::slice::from_ref(&pipeline_info),
            None,
        )
    } {
        Ok(mut pipelines) => match pipelines.pop() {
            Some(pipeline) => pipeline,
            None => {
                unsafe {
                    device.destroy_shader_module(vertex_module, None);
                    device.destroy_shader_module(fragment_module, None);
                }
                return Err(RendererError::Init(
                    "Vulkan pipeline creation returned no pipelines".into(),
                ));
            }
        },
        Err((pipelines, err)) => {
            for pipeline in pipelines {
                unsafe { device.destroy_pipeline(pipeline, None) };
            }
            unsafe {
                device.destroy_shader_module(vertex_module, None);
                device.destroy_shader_module(fragment_module, None);
            }
            return Err(err.into());
        }
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
    max_sampled_images: u32,
) -> RendererResult<vk::DescriptorPool> {
    let (max_sets, sizes) = descriptor_pool_plan(max_sampled_images)?;
    let create_info = vk::DescriptorPoolCreateInfo::default()
        .pool_sizes(&sizes)
        .max_sets(max_sets)
        .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET);
    unsafe { Ok(device.create_descriptor_pool(&create_info, None)?) }
}

fn descriptor_pool_plan(
    max_sampled_images: u32,
) -> RendererResult<(u32, [vk::DescriptorPoolSize; 2])> {
    if max_sampled_images < MIN_SAMPLED_IMAGE_DESCRIPTORS {
        return Err(RendererError::InvalidRenderState(format!(
            "Options::max_textures must be >= {MIN_SAMPLED_IMAGE_DESCRIPTORS}"
        )));
    }
    let max_sets = max_sampled_images
        .checked_add(STANDARD_SAMPLER_DESCRIPTORS)
        .ok_or_else(|| {
            RendererError::InvalidRenderState(
                "Options::max_textures overflows Vulkan descriptor-pool accounting".to_owned(),
            )
        })?;
    let sizes = [
        vk::DescriptorPoolSize {
            ty: vk::DescriptorType::SAMPLED_IMAGE,
            descriptor_count: max_sampled_images,
        },
        vk::DescriptorPoolSize {
            ty: vk::DescriptorType::SAMPLER,
            descriptor_count: STANDARD_SAMPLER_DESCRIPTORS,
        },
    ];
    Ok((max_sets, sizes))
}

/// Create a sampled-image descriptor set compatible with set 0.
pub fn create_vulkan_sampled_image_descriptor_set(
    device: &Device,
    set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    image_view: vk::ImageView,
    image_layout: vk::ImageLayout,
) -> RendererResult<vk::DescriptorSet> {
    let set = {
        let set_layouts = [set_layout];
        let allocate_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&set_layouts);
        let mut sets = unsafe { device.allocate_descriptor_sets(&allocate_info)? };
        sets.pop().ok_or_else(|| {
            RendererError::Init("Vulkan descriptor set allocation returned no sets".into())
        })?
    };

    unsafe {
        let image_info = [vk::DescriptorImageInfo {
            sampler: vk::Sampler::null(),
            image_view,
            image_layout,
        }];
        let write_desc_sets = [vk::WriteDescriptorSet::default()
            .dst_set(set)
            .dst_binding(0)
            .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
            .image_info(&image_info)];
        device.update_descriptor_sets(&write_desc_sets, &[]);
    }

    Ok(set)
}

fn create_vulkan_sampler_descriptor_set(
    device: &Device,
    set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    sampler: vk::Sampler,
) -> RendererResult<vk::DescriptorSet> {
    let set_layouts = [set_layout];
    let allocate_info = vk::DescriptorSetAllocateInfo::default()
        .descriptor_pool(descriptor_pool)
        .set_layouts(&set_layouts);
    let mut sets = unsafe { device.allocate_descriptor_sets(&allocate_info)? };
    let set = sets.pop().ok_or_else(|| {
        RendererError::Init("Vulkan sampler descriptor allocation returned no sets".into())
    })?;
    let image_info = [vk::DescriptorImageInfo {
        sampler,
        image_view: vk::ImageView::null(),
        image_layout: vk::ImageLayout::UNDEFINED,
    }];
    let writes = [vk::WriteDescriptorSet::default()
        .dst_set(set)
        .dst_binding(0)
        .descriptor_type(vk::DescriptorType::SAMPLER)
        .image_info(&image_info)];
    unsafe { device.update_descriptor_sets(&writes, &[]) };
    Ok(set)
}

fn standard_sampler_create_info(filter: vk::Filter) -> vk::SamplerCreateInfo<'static> {
    let mipmap_mode = if filter == vk::Filter::NEAREST {
        vk::SamplerMipmapMode::NEAREST
    } else {
        vk::SamplerMipmapMode::LINEAR
    };
    vk::SamplerCreateInfo::default()
        .mag_filter(filter)
        .min_filter(filter)
        .mipmap_mode(mipmap_mode)
        .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
        .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
        .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
        .min_lod(-1000.0)
        .max_lod(1000.0)
        .max_anisotropy(1.0)
}

fn create_standard_sampler(device: &Device, filter: vk::Filter) -> RendererResult<vk::Sampler> {
    let create_info = standard_sampler_create_info(filter);
    unsafe { Ok(device.create_sampler(&create_info, None)?) }
}

pub(super) struct VulkanRendererResources {
    pub(super) pipeline: vk::Pipeline,
    pub(super) pipeline_layout: vk::PipelineLayout,
    pub(super) sampled_image_set_layout: vk::DescriptorSetLayout,
    pub(super) sampler_set_layout: vk::DescriptorSetLayout,
    pub(super) descriptor_pool: vk::DescriptorPool,
    pub(super) linear_sampler: vk::Sampler,
    pub(super) nearest_sampler: vk::Sampler,
    pub(super) linear_sampler_set: vk::DescriptorSet,
    pub(super) nearest_sampler_set: vk::DescriptorSet,
}

impl VulkanRendererResources {
    pub(super) fn create(
        device: &Device,
        #[cfg(not(feature = "dynamic-rendering"))] render_pass: vk::RenderPass,
        #[cfg(feature = "dynamic-rendering")] dynamic_rendering: super::DynamicRendering,
        options: Options,
    ) -> RendererResult<Self> {
        let mut resources = Self::empty();
        let result = (|| {
            resources.sampled_image_set_layout =
                create_vulkan_descriptor_set_layout(device, vk::DescriptorType::SAMPLED_IMAGE)?;
            resources.sampler_set_layout =
                create_vulkan_descriptor_set_layout(device, vk::DescriptorType::SAMPLER)?;
            resources.pipeline_layout = create_vulkan_pipeline_layout(
                device,
                resources.sampled_image_set_layout,
                resources.sampler_set_layout,
            )?;
            resources.pipeline = create_vulkan_pipeline(
                device,
                resources.pipeline_layout,
                #[cfg(not(feature = "dynamic-rendering"))]
                render_pass,
                #[cfg(feature = "dynamic-rendering")]
                dynamic_rendering,
                options,
            )?;
            resources.descriptor_pool =
                create_vulkan_descriptor_pool(device, options.max_textures)?;
            resources.linear_sampler = create_standard_sampler(device, vk::Filter::LINEAR)?;
            resources.nearest_sampler = create_standard_sampler(device, vk::Filter::NEAREST)?;
            resources.linear_sampler_set = create_vulkan_sampler_descriptor_set(
                device,
                resources.sampler_set_layout,
                resources.descriptor_pool,
                resources.linear_sampler,
            )?;
            resources.nearest_sampler_set = create_vulkan_sampler_descriptor_set(
                device,
                resources.sampler_set_layout,
                resources.descriptor_pool,
                resources.nearest_sampler,
            )?;
            Ok(())
        })();
        if let Err(error) = result {
            unsafe { resources.destroy_handles(device) };
            return Err(error);
        }
        Ok(resources)
    }

    pub(super) fn empty() -> Self {
        Self {
            pipeline: vk::Pipeline::null(),
            pipeline_layout: vk::PipelineLayout::null(),
            sampled_image_set_layout: vk::DescriptorSetLayout::null(),
            sampler_set_layout: vk::DescriptorSetLayout::null(),
            descriptor_pool: vk::DescriptorPool::null(),
            linear_sampler: vk::Sampler::null(),
            nearest_sampler: vk::Sampler::null(),
            linear_sampler_set: vk::DescriptorSet::null(),
            nearest_sampler_set: vk::DescriptorSet::null(),
        }
    }

    pub(super) fn destroy(mut self, device: &Device) {
        unsafe { self.destroy_handles(device) };
    }

    unsafe fn destroy_handles(&mut self, device: &Device) {
        unsafe {
            if self.pipeline != vk::Pipeline::null() {
                device.destroy_pipeline(self.pipeline, None);
                self.pipeline = vk::Pipeline::null();
            }
            if self.linear_sampler != vk::Sampler::null() {
                device.destroy_sampler(self.linear_sampler, None);
                self.linear_sampler = vk::Sampler::null();
            }
            if self.nearest_sampler != vk::Sampler::null() {
                device.destroy_sampler(self.nearest_sampler, None);
                self.nearest_sampler = vk::Sampler::null();
            }
            if self.descriptor_pool != vk::DescriptorPool::null() {
                device.destroy_descriptor_pool(self.descriptor_pool, None);
                self.descriptor_pool = vk::DescriptorPool::null();
            }
            if self.pipeline_layout != vk::PipelineLayout::null() {
                device.destroy_pipeline_layout(self.pipeline_layout, None);
                self.pipeline_layout = vk::PipelineLayout::null();
            }
            if self.sampled_image_set_layout != vk::DescriptorSetLayout::null() {
                device.destroy_descriptor_set_layout(self.sampled_image_set_layout, None);
                self.sampled_image_set_layout = vk::DescriptorSetLayout::null();
            }
            if self.sampler_set_layout != vk::DescriptorSetLayout::null() {
                device.destroy_descriptor_set_layout(self.sampler_set_layout, None);
                self.sampler_set_layout = vk::DescriptorSetLayout::null();
            }
        }
    }
}

pub(crate) fn create_and_fill_buffer<T: Copy>(
    device: &Device,
    allocator: &mut Allocator,
    data: &[T],
    usage: vk::BufferUsageFlags,
) -> RendererResult<(vk::Buffer, Memory)> {
    let size = std::mem::size_of_val(data);
    let (buffer, mut memory) = allocator.create_buffer(device, size, usage)?;
    if let Err(err) = allocator.update_buffer(device, &mut memory, data) {
        let _ = allocator.destroy_buffer(device, buffer, memory);
        return Err(err);
    }
    Ok((buffer, memory))
}

pub(crate) struct Texture {
    pub image: vk::Image,
    pub image_mem: Memory,
    pub image_view: vk::ImageView,
}

pub(crate) const MANAGED_TEXTURE_FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;

impl Texture {
    /// Create a GPU image, view, and a staging buffer filled with `pixels_rgba`.
    pub fn create(
        device: &Device,
        allocator: &mut Allocator,
        width: u32,
        height: u32,
        pixels_rgba: &[u8],
    ) -> RendererResult<(Self, vk::Buffer, Memory)> {
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|v| v.checked_mul(4))
            .ok_or_else(|| RendererError::Allocator("texture size overflow".into()))?;
        if pixels_rgba.len() < expected {
            return Err(RendererError::Allocator(
                "texture pixel buffer too small".into(),
            ));
        }

        let (image, image_mem) =
            allocator.create_image(device, width, height, MANAGED_TEXTURE_FORMAT)?;

        let (buffer, buffer_mem) = match create_and_fill_buffer(
            device,
            allocator,
            &pixels_rgba[..expected],
            vk::BufferUsageFlags::TRANSFER_SRC,
        ) {
            Ok(staging) => staging,
            Err(err) => {
                let _ = allocator.destroy_image(device, image, image_mem);
                return Err(err);
            }
        };

        let image_view = {
            let create_info = vk::ImageViewCreateInfo::default()
                .image(image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(MANAGED_TEXTURE_FORMAT)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });
            match unsafe { device.create_image_view(&create_info, None) } {
                Ok(image_view) => image_view,
                Err(err) => {
                    let _ = allocator.destroy_buffer(device, buffer, buffer_mem);
                    let _ = allocator.destroy_image(device, image, image_mem);
                    return Err(err.into());
                }
            }
        };

        Ok((
            Self {
                image,
                image_mem,
                image_view,
            },
            buffer,
            buffer_mem,
        ))
    }

    pub fn destroy(self, device: &Device, allocator: &mut Allocator) -> RendererResult<()> {
        unsafe {
            device.destroy_image_view(self.image_view, None);
        }
        allocator.destroy_image(device, self.image, self.image_mem)
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
            ImageUploadRegion {
                x: 0,
                y: 0,
                width,
                height,
            },
            ImageLayoutState {
                layout: vk::ImageLayout::UNDEFINED,
                stage: vk::PipelineStageFlags::TOP_OF_PIPE,
                access: vk::AccessFlags::empty(),
            },
        );
    }
}

pub(crate) fn upload_rgba_subrect_to_image(
    device: &Device,
    command_buffer: vk::CommandBuffer,
    buffer: vk::Buffer,
    image: vk::Image,
    region: ImageUploadRegion,
) {
    upload_buffer_to_image(
        device,
        command_buffer,
        buffer,
        image,
        region,
        ImageLayoutState {
            layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            stage: vk::PipelineStageFlags::FRAGMENT_SHADER,
            access: vk::AccessFlags::SHADER_READ,
        },
    );
}

fn upload_buffer_to_image(
    device: &Device,
    command_buffer: vk::CommandBuffer,
    buffer: vk::Buffer,
    image: vk::Image,
    region: ImageUploadRegion,
    source_state: ImageLayoutState,
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
        barrier.old_layout = source_state.layout;
        barrier.new_layout = vk::ImageLayout::TRANSFER_DST_OPTIMAL;
        barrier.src_access_mask = source_state.access;
        barrier.dst_access_mask = vk::AccessFlags::TRANSFER_WRITE;

        device.cmd_pipeline_barrier(
            command_buffer,
            source_state.stage,
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
                x: region.x as i32,
                y: region.y as i32,
                z: 0,
            })
            .image_extent(vk::Extent3D {
                width: region.width,
                height: region.height,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_pool_reserves_images_and_two_renderer_owned_samplers() {
        assert!(matches!(
            descriptor_pool_plan(MIN_SAMPLED_IMAGE_DESCRIPTORS - 1),
            Err(RendererError::InvalidRenderState(_))
        ));
        assert!(matches!(
            descriptor_pool_plan(u32::MAX),
            Err(RendererError::InvalidRenderState(_))
        ));

        let (max_sets, sizes) = descriptor_pool_plan(MIN_SAMPLED_IMAGE_DESCRIPTORS).unwrap();
        assert_eq!(
            max_sets,
            MIN_SAMPLED_IMAGE_DESCRIPTORS + STANDARD_SAMPLER_DESCRIPTORS
        );
        assert_eq!(sizes[0].ty, vk::DescriptorType::SAMPLED_IMAGE);
        assert_eq!(sizes[0].descriptor_count, MIN_SAMPLED_IMAGE_DESCRIPTORS);
        assert_eq!(sizes[1].ty, vk::DescriptorType::SAMPLER);
        assert_eq!(sizes[1].descriptor_count, STANDARD_SAMPLER_DESCRIPTORS);
    }

    #[test]
    fn standard_samplers_match_upstream_filter_and_lod_contract() {
        for (filter, mipmap_mode) in [
            (vk::Filter::LINEAR, vk::SamplerMipmapMode::LINEAR),
            (vk::Filter::NEAREST, vk::SamplerMipmapMode::NEAREST),
        ] {
            let info = standard_sampler_create_info(filter);
            assert_eq!(info.mag_filter, filter);
            assert_eq!(info.min_filter, filter);
            assert_eq!(info.mipmap_mode, mipmap_mode);
            assert_eq!(info.address_mode_u, vk::SamplerAddressMode::CLAMP_TO_EDGE);
            assert_eq!(info.address_mode_v, vk::SamplerAddressMode::CLAMP_TO_EDGE);
            assert_eq!(info.address_mode_w, vk::SamplerAddressMode::CLAMP_TO_EDGE);
            assert_eq!(info.min_lod, -1000.0);
            assert_eq!(info.max_lod, 1000.0);
            assert_eq!(info.max_anisotropy, 1.0);
            assert_eq!(info.anisotropy_enable, vk::FALSE);
        }
    }

    #[test]
    fn scissor_rejects_non_finite_empty_and_offscreen_clip_rectangles() {
        let scissor = |clip_rect| clip_rect_to_scissor(clip_rect, [0.0, 0.0], [1.0, 1.0], 100, 80);
        assert!(scissor([f32::NAN, 0.0, 10.0, 10.0]).is_none());
        assert!(scissor([0.0, 0.0, f32::INFINITY, 10.0]).is_none());
        assert!(scissor([10.0, 5.0, 10.0, 20.0]).is_none());
        assert!(scissor([20.0, 20.0, 10.0, 30.0]).is_none());
        assert!(scissor([100.0, 0.0, 120.0, 20.0]).is_none());
        assert!(clip_rect_to_scissor([0.0; 4], [0.0; 2], [1.0; 2], 0, 80).is_none());
    }

    #[test]
    fn scissor_clamps_fractional_clip_rectangles_to_the_framebuffer() {
        let scissor =
            clip_rect_to_scissor([-3.2, 2.2, 120.8, 81.0], [0.0, 0.0], [1.0, 1.0], 100, 80)
                .unwrap();
        assert_eq!(scissor.offset, vk::Offset2D { x: 0, y: 2 });
        assert_eq!(
            scissor.extent,
            vk::Extent2D {
                width: 100,
                height: 78,
            }
        );
    }
}
