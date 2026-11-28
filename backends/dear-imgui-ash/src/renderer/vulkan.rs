//! Vulkan helpers (pipelines, descriptor sets, uploads).
//!
//! This module is inspired by `imgui-rs-vulkan-renderer`, adapted to `dear-imgui-rs`.

use crate::{Options, RenderTarget, RendererError, RendererResult};
use ash::{Device, vk};
use std::ffi::CString;

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
    let extent = [
        fb_width.min(i32::MAX as u32),
        fb_height.min(i32::MAX as u32),
    ];
    let [x, y, width, height] =
        dear_imgui_backend_support::compute_scissor_rect(clip_rect, clip_off, clip_scale, extent)?;
    Some(vk::Rect2D {
        offset: vk::Offset2D {
            x: x as i32,
            y: y as i32,
        },
        extent: vk::Extent2D { width, height },
    })
}

pub(super) const MIN_SAMPLED_IMAGE_DESCRIPTORS: u32 = 8;
pub(super) const STANDARD_SAMPLER_DESCRIPTORS: u32 = 2;

#[derive(Clone, Copy)]
#[cfg_attr(test, derive(Debug, Eq, PartialEq))]
pub(crate) struct ImageUploadRegion {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
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

#[cfg(feature = "dynamic-rendering")]
fn pipeline_rendering_info(
    color_attachment_formats: &[vk::Format],
    depth_attachment_format: Option<vk::Format>,
    stencil_attachment_format: vk::Format,
) -> vk::PipelineRenderingCreateInfo<'_> {
    vk::PipelineRenderingCreateInfo::default()
        .color_attachment_formats(color_attachment_formats)
        .depth_attachment_format(depth_attachment_format.unwrap_or(vk::Format::UNDEFINED))
        .stencil_attachment_format(stencil_attachment_format)
}

pub(super) fn create_vulkan_pipeline(
    device: &Device,
    pipeline_layout: vk::PipelineLayout,
    render_target: RenderTarget,
    options: Options,
    #[cfg(feature = "dynamic-rendering")] stencil_attachment_format: vk::Format,
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

    let create_result = match render_target {
        #[cfg(feature = "render-pass")]
        RenderTarget::RenderPass(render_pass) => {
            let pipeline_info = pipeline_info.render_pass(render_pass);
            unsafe {
                device.create_graphics_pipelines(
                    vk::PipelineCache::null(),
                    std::slice::from_ref(&pipeline_info),
                    None,
                )
            }
        }
        #[cfg(feature = "dynamic-rendering")]
        RenderTarget::DynamicRendering(dynamic_rendering) => {
            let color_attachment_formats = [dynamic_rendering.color_attachment_format];
            let mut rendering_info = pipeline_rendering_info(
                &color_attachment_formats,
                dynamic_rendering.depth_attachment_format,
                stencil_attachment_format,
            );
            let pipeline_info = pipeline_info.push_next(&mut rendering_info);
            unsafe {
                device.create_graphics_pipelines(
                    vk::PipelineCache::null(),
                    std::slice::from_ref(&pipeline_info),
                    None,
                )
            }
        }
    };

    let pipeline = match create_result {
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
        render_target: RenderTarget,
        options: Options,
        #[cfg(feature = "dynamic-rendering")] stencil_attachment_format: vk::Format,
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
                render_target,
                options,
                #[cfg(feature = "dynamic-rendering")]
                stencil_attachment_format,
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

pub(super) const MANAGED_TEXTURE_FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;

const TEXTURE_SUBRESOURCE_RANGE: vk::ImageSubresourceRange = vk::ImageSubresourceRange {
    aspect_mask: vk::ImageAspectFlags::COLOR,
    base_mip_level: 0,
    level_count: 1,
    base_array_layer: 0,
    layer_count: 1,
};

const TEXTURE_SUBRESOURCE_LAYERS: vk::ImageSubresourceLayers = vk::ImageSubresourceLayers {
    aspect_mask: vk::ImageAspectFlags::COLOR,
    mip_level: 0,
    base_array_layer: 0,
    layer_count: 1,
};

pub(super) fn create_texture_image_view(
    device: &Device,
    image: vk::Image,
) -> RendererResult<vk::ImageView> {
    let create_info = vk::ImageViewCreateInfo::default()
        .image(image)
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(MANAGED_TEXTURE_FORMAT)
        .subresource_range(TEXTURE_SUBRESOURCE_RANGE);
    Ok(unsafe { device.create_image_view(&create_info, None)? })
}

pub(super) struct ImageUploadPixels<'pixels> {
    pub(super) region: ImageUploadRegion,
    pub(super) rgba: std::borrow::Cow<'pixels, [u8]>,
}

impl<'pixels> ImageUploadPixels<'pixels> {
    pub(super) fn full_image(
        width: u32,
        height: u32,
        rgba: std::borrow::Cow<'pixels, [u8]>,
    ) -> Self {
        Self {
            region: ImageUploadRegion {
                x: 0,
                y: 0,
                width,
                height,
            },
            rgba,
        }
    }
}

pub(super) struct ImageUpload<'pixels> {
    image: vk::Image,
    initialization: ImageUploadInitialization,
    pixels: &'pixels [ImageUploadPixels<'pixels>],
}

enum ImageUploadInitialization {
    New,
    PreviouslySampled,
    CopyFrom(vk::Image, vk::Extent3D),
}

impl<'pixels> ImageUpload<'pixels> {
    pub(super) fn new_image(
        image: vk::Image,
        pixels: &'pixels [ImageUploadPixels<'pixels>],
    ) -> Self {
        Self {
            image,
            initialization: ImageUploadInitialization::New,
            pixels,
        }
    }

    pub(super) fn subrect(image: vk::Image, pixels: &'pixels ImageUploadPixels<'pixels>) -> Self {
        Self {
            image,
            initialization: ImageUploadInitialization::PreviouslySampled,
            pixels: std::slice::from_ref(pixels),
        }
    }

    pub(super) fn replacement(
        image: vk::Image,
        previous: vk::Image,
        width: u32,
        height: u32,
        pixels: &'pixels [ImageUploadPixels<'pixels>],
    ) -> Self {
        Self {
            image,
            initialization: ImageUploadInitialization::CopyFrom(
                previous,
                vk::Extent3D {
                    width,
                    height,
                    depth: 1,
                },
            ),
            pixels,
        }
    }

    pub(super) fn pixel_chunks(&self) -> impl Iterator<Item = &[u8]> + Clone {
        self.pixels.iter().map(|pixels| pixels.rgba.as_ref())
    }

    pub(super) fn record(
        &self,
        device: &Device,
        command_buffer: vk::CommandBuffer,
        buffer: vk::Buffer,
        offset: &mut vk::DeviceSize,
    ) {
        debug_assert!(!self.pixels.is_empty());
        let image = self.image;
        let initial_layout = match self.initialization {
            ImageUploadInitialization::PreviouslySampled => {
                ImageLayoutState::sampled_after_upload()
            }
            _ => ImageLayoutState::UNDEFINED,
        };
        image_barrier(
            device,
            command_buffer,
            image,
            initial_layout,
            ImageLayoutState::TRANSFER_DST,
        );

        let copied_previous =
            if let ImageUploadInitialization::CopyFrom(previous, extent) = self.initialization {
                copy_sampled_image(device, command_buffer, previous, image, extent);
                true
            } else {
                false
            };

        for (index, pixels) in self.pixels.iter().enumerate() {
            debug_assert!(!pixels.rgba.is_empty() && pixels.rgba.len().is_multiple_of(4));
            debug_assert!(offset.is_multiple_of(4));
            if copied_previous || index != 0 {
                // The initial image copy and overlapping patches must write in request order.
                image_barrier(
                    device,
                    command_buffer,
                    image,
                    ImageLayoutState::TRANSFER_DST,
                    ImageLayoutState::TRANSFER_DST,
                );
            }
            copy_buffer_to_texture(
                device,
                command_buffer,
                buffer,
                *offset,
                image,
                pixels.region,
            );
            *offset += pixels.rgba.len() as vk::DeviceSize;
        }

        // This dependency also covers fragment reads in later submissions on this queue.
        image_barrier(
            device,
            command_buffer,
            image,
            ImageLayoutState::TRANSFER_DST,
            ImageLayoutState::SAMPLED,
        );
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(test, derive(Debug, Eq, PartialEq))]
struct ImageLayoutState {
    layout: vk::ImageLayout,
    stage: vk::PipelineStageFlags,
    access: vk::AccessFlags,
}

impl ImageLayoutState {
    const UNDEFINED: Self = Self {
        layout: vk::ImageLayout::UNDEFINED,
        stage: vk::PipelineStageFlags::TOP_OF_PIPE,
        access: vk::AccessFlags::empty(),
    };
    const SAMPLED: Self = Self {
        layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        stage: vk::PipelineStageFlags::FRAGMENT_SHADER,
        access: vk::AccessFlags::SHADER_READ,
    };
    const TRANSFER_SRC: Self = Self {
        layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        stage: vk::PipelineStageFlags::TRANSFER,
        access: vk::AccessFlags::TRANSFER_READ,
    };
    const TRANSFER_DST: Self = Self {
        layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        stage: vk::PipelineStageFlags::TRANSFER,
        access: vk::AccessFlags::TRANSFER_WRITE,
    };

    fn sampled_after_upload() -> Self {
        Self {
            stage: vk::PipelineStageFlags::FRAGMENT_SHADER | vk::PipelineStageFlags::TRANSFER,
            access: vk::AccessFlags::SHADER_READ | vk::AccessFlags::TRANSFER_WRITE,
            ..Self::SAMPLED
        }
    }
}

fn image_barrier(
    device: &Device,
    command_buffer: vk::CommandBuffer,
    image: vk::Image,
    from: ImageLayoutState,
    to: ImageLayoutState,
) {
    let barrier = vk::ImageMemoryBarrier::default()
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(image)
        .subresource_range(TEXTURE_SUBRESOURCE_RANGE)
        .old_layout(from.layout)
        .new_layout(to.layout)
        .src_access_mask(from.access)
        .dst_access_mask(to.access);

    unsafe {
        device.cmd_pipeline_barrier(
            command_buffer,
            from.stage,
            to.stage,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier],
        );
    }
}

fn copy_sampled_image(
    device: &Device,
    command_buffer: vk::CommandBuffer,
    source: vk::Image,
    destination: vk::Image,
    extent: vk::Extent3D,
) {
    image_barrier(
        device,
        command_buffer,
        source,
        ImageLayoutState::sampled_after_upload(),
        ImageLayoutState::TRANSFER_SRC,
    );
    let region = vk::ImageCopy::default()
        .src_subresource(TEXTURE_SUBRESOURCE_LAYERS)
        .dst_subresource(TEXTURE_SUBRESOURCE_LAYERS)
        .extent(extent);
    unsafe {
        device.cmd_copy_image(
            command_buffer,
            source,
            ImageLayoutState::TRANSFER_SRC.layout,
            destination,
            ImageLayoutState::TRANSFER_DST.layout,
            &[region],
        );
    }
    // Already recorded draws still bind the old view and expect its original layout and contents.
    image_barrier(
        device,
        command_buffer,
        source,
        ImageLayoutState::TRANSFER_SRC,
        ImageLayoutState::SAMPLED,
    );
}

fn copy_buffer_to_texture(
    device: &Device,
    command_buffer: vk::CommandBuffer,
    buffer: vk::Buffer,
    offset: vk::DeviceSize,
    image: vk::Image,
    region: ImageUploadRegion,
) {
    let region = vk::BufferImageCopy::default()
        .buffer_offset(offset)
        .image_subresource(TEXTURE_SUBRESOURCE_LAYERS)
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
    unsafe {
        device.cmd_copy_buffer_to_image(
            command_buffer,
            buffer,
            image,
            ImageLayoutState::TRANSFER_DST.layout,
            &[region],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_copies_and_restores_old_image_before_ordered_dirty_writes() {
        use ash::vk::Handle;
        use std::{cell::RefCell, ffi::c_void};

        #[derive(Debug, Eq, PartialEq)]
        enum Command {
            Barrier(vk::Image, ImageLayoutState, ImageLayoutState),
            CopyImage(
                vk::Image,
                vk::ImageLayout,
                vk::Image,
                vk::ImageLayout,
                vk::Extent3D,
            ),
            Patch(
                vk::Image,
                vk::ImageLayout,
                vk::DeviceSize,
                vk::Offset3D,
                vk::Extent3D,
            ),
        }

        thread_local! {
            static COMMANDS: RefCell<Vec<Command>> = const { RefCell::new(Vec::new()) };
        }

        unsafe extern "system" fn barrier(
            _: vk::CommandBuffer,
            src_stage: vk::PipelineStageFlags,
            dst_stage: vk::PipelineStageFlags,
            _: vk::DependencyFlags,
            _: u32,
            _: *const vk::MemoryBarrier<'_>,
            _: u32,
            _: *const vk::BufferMemoryBarrier<'_>,
            count: u32,
            barriers: *const vk::ImageMemoryBarrier<'_>,
        ) {
            // SAFETY: ash supplies the slice used by the recording code for this call.
            for barrier in unsafe { std::slice::from_raw_parts(barriers, count as usize) } {
                COMMANDS.with_borrow_mut(|commands| {
                    commands.push(Command::Barrier(
                        barrier.image,
                        ImageLayoutState {
                            layout: barrier.old_layout,
                            stage: src_stage,
                            access: barrier.src_access_mask,
                        },
                        ImageLayoutState {
                            layout: barrier.new_layout,
                            stage: dst_stage,
                            access: barrier.dst_access_mask,
                        },
                    ));
                });
            }
        }

        unsafe extern "system" fn copy_image(
            _: vk::CommandBuffer,
            src: vk::Image,
            src_layout: vk::ImageLayout,
            dst: vk::Image,
            dst_layout: vk::ImageLayout,
            count: u32,
            regions: *const vk::ImageCopy,
        ) {
            // SAFETY: ash supplies the slice used by the recording code for this call.
            for region in unsafe { std::slice::from_raw_parts(regions, count as usize) } {
                COMMANDS.with_borrow_mut(|commands| {
                    commands.push(Command::CopyImage(
                        src,
                        src_layout,
                        dst,
                        dst_layout,
                        region.extent,
                    ));
                });
            }
        }

        unsafe extern "system" fn copy_buffer(
            _: vk::CommandBuffer,
            _: vk::Buffer,
            dst: vk::Image,
            layout: vk::ImageLayout,
            count: u32,
            regions: *const vk::BufferImageCopy,
        ) {
            // SAFETY: ash supplies the slice used by the recording code for this call.
            for region in unsafe { std::slice::from_raw_parts(regions, count as usize) } {
                COMMANDS.with_borrow_mut(|commands| {
                    commands.push(Command::Patch(
                        dst,
                        layout,
                        region.buffer_offset,
                        region.image_offset,
                        region.image_extent,
                    ));
                });
            }
        }

        // SAFETY: all exercised Vulkan entry points use stubs with the corresponding ABI.
        let device = unsafe {
            Device::load_with(
                |name| match name.to_bytes() {
                    b"vkCmdPipelineBarrier" => barrier as *const c_void,
                    b"vkCmdCopyImage" => copy_image as *const c_void,
                    b"vkCmdCopyBufferToImage" => copy_buffer as *const c_void,
                    _ => std::ptr::null(),
                },
                vk::Device::null(),
            )
        };
        let old = vk::Image::from_raw(1);
        let new = vk::Image::from_raw(2);
        let pixels = [
            ImageUploadPixels {
                region: ImageUploadRegion {
                    x: 2,
                    y: 1,
                    width: 1,
                    height: 2,
                },
                rgba: (&[1; 8][..]).into(),
            },
            ImageUploadPixels {
                region: ImageUploadRegion {
                    x: 2,
                    y: 2,
                    width: 1,
                    height: 1,
                },
                rgba: (&[2; 4][..]).into(),
            },
        ];
        let upload = ImageUpload::replacement(new, old, 8, 4, &pixels);
        let mut offset = 4;
        upload.record(
            &device,
            vk::CommandBuffer::null(),
            vk::Buffer::null(),
            &mut offset,
        );
        assert_eq!(offset, 16);

        use ImageLayoutState as Layout;
        let transfer_write = ImageLayoutState {
            layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            stage: vk::PipelineStageFlags::TRANSFER,
            access: vk::AccessFlags::TRANSFER_WRITE,
        };
        assert_eq!(
            COMMANDS.take(),
            [
                Command::Barrier(new, Layout::UNDEFINED, transfer_write),
                Command::Barrier(
                    old,
                    ImageLayoutState {
                        layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                        stage: vk::PipelineStageFlags::FRAGMENT_SHADER
                            | vk::PipelineStageFlags::TRANSFER,
                        access: vk::AccessFlags::SHADER_READ | vk::AccessFlags::TRANSFER_WRITE,
                    },
                    Layout::TRANSFER_SRC
                ),
                Command::CopyImage(
                    old,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    new,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    vk::Extent3D {
                        width: 8,
                        height: 4,
                        depth: 1
                    }
                ),
                Command::Barrier(old, Layout::TRANSFER_SRC, Layout::SAMPLED),
                Command::Barrier(new, transfer_write, transfer_write),
                Command::Patch(
                    new,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    4,
                    vk::Offset3D { x: 2, y: 1, z: 0 },
                    vk::Extent3D {
                        width: 1,
                        height: 2,
                        depth: 1
                    }
                ),
                Command::Barrier(new, transfer_write, transfer_write),
                Command::Patch(
                    new,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    12,
                    vk::Offset3D { x: 2, y: 2, z: 0 },
                    vk::Extent3D {
                        width: 1,
                        height: 1,
                        depth: 1
                    }
                ),
                Command::Barrier(new, transfer_write, Layout::SAMPLED),
            ]
        );
    }

    #[cfg(feature = "dynamic-rendering")]
    #[test]
    fn dynamic_pipeline_declares_only_the_bound_depth_and_stencil_aspects() {
        let colors = [vk::Format::B8G8R8A8_UNORM];
        let combined = vk::Format::D24_UNORM_S8_UINT;
        for (depth, stencil) in [
            (None, vk::Format::UNDEFINED),
            (Some(vk::Format::D32_SFLOAT), vk::Format::UNDEFINED),
            (Some(combined), vk::Format::UNDEFINED),
            (Some(combined), combined),
            (None, vk::Format::S8_UINT),
        ] {
            let info = pipeline_rendering_info(&colors, depth, stencil);
            assert_eq!(info.color_attachment_count, 1);
            assert_eq!(
                info.depth_attachment_format,
                depth.unwrap_or(vk::Format::UNDEFINED)
            );
            assert_eq!(info.stencil_attachment_format, stencil);
        }
    }

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
