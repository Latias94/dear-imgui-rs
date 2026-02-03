//! Vulkan (Ash) renderer implementation.

mod allocator;
#[cfg(feature = "multi-viewport-winit")]
pub mod multi_viewport;
mod shaders;
mod vulkan;

use crate::TextureUpdateResult;
use crate::{RendererError, RendererResult};
use ash::{Device, Instance, vk};
use dear_imgui_rs::{BackendFlags, Context};
use dear_imgui_rs::{TextureData, TextureFormat as ImGuiTextureFormat, TextureId, TextureStatus};
use std::collections::{HashMap, VecDeque};

use self::allocator::{Allocate, Allocator, Memory};
use self::vulkan::*;

/// Optional parameters of the renderer.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// The number of in-flight frames of the application.
    pub in_flight_frames: usize,
    /// If true enables depth test when rendering.
    pub enable_depth_test: bool,
    /// If true enables depth writes when rendering.
    pub enable_depth_write: bool,
    /// Subpass for the graphics pipeline.
    pub subpass: u32,
    /// Sample count for the graphics pipeline multisampling state.
    pub sample_count: vk::SampleCountFlags,
    /// Maximum number of texture descriptor sets allocated from the pool.
    pub max_textures: u32,
    /// If true, treat the render target as sRGB.
    ///
    /// This backend follows the WGPU renderer approach: ImGui provides colors/texels in sRGB
    /// space (stored as UNORM), and the fragment shader applies `pow(rgb, gamma)` to convert
    /// to linear before writing to an sRGB render target.
    pub framebuffer_srgb: bool,
    /// Override the gamma used for sRGB->linear conversion in the shader.
    ///
    /// - `None`: auto (2.2 when `framebuffer_srgb`, else 1.0)
    /// - `Some(gamma)`: force a value (e.g. 2.2 or 1.0)
    pub color_gamma_override: Option<f32>,
    /// Format used for internally managed RGBA textures (font atlas, `TextureData` uploads).
    ///
    /// Recommended: keep this as `vk::Format::R8G8B8A8_UNORM` to match the shader gamma path.
    pub texture_format: vk::Format,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            in_flight_frames: 1,
            enable_depth_test: false,
            enable_depth_write: false,
            subpass: 0,
            sample_count: vk::SampleCountFlags::TYPE_1,
            max_textures: 1024,
            framebuffer_srgb: false,
            color_gamma_override: None,
            texture_format: vk::Format::R8G8B8A8_UNORM,
        }
    }
}

/// `dynamic-rendering` feature related params.
#[cfg(feature = "dynamic-rendering")]
#[derive(Debug, Clone, Copy)]
pub struct DynamicRendering {
    pub color_attachment_format: vk::Format,
    pub depth_attachment_format: Option<vk::Format>,
}

#[cfg(feature = "multi-viewport-winit")]
struct ViewportPipeline {
    pipeline: vk::Pipeline,
    #[cfg(not(feature = "dynamic-rendering"))]
    render_pass: vk::RenderPass,
}

#[derive(Debug)]
struct VulkanTexture {
    image: vk::Image,
    image_mem: Memory,
    image_view: vk::ImageView,
    sampler: vk::Sampler,
    descriptor_set: vk::DescriptorSet,
    width: u32,
    height: u32,
}

impl VulkanTexture {
    fn destroy(self, device: &Device, allocator: &mut Allocator, pool: vk::DescriptorPool) {
        unsafe {
            device.destroy_sampler(self.sampler, None);
            device.destroy_image_view(self.image_view, None);
            let _ = device.free_descriptor_sets(pool, &[self.descriptor_set]);
        }
        let _ = allocator.destroy_image(device, self.image, self.image_mem);
    }
}

#[derive(Debug, Copy, Clone)]
struct ExternalTextureBinding {
    descriptor_set: vk::DescriptorSet,
    image_view: Option<vk::ImageView>,
    sampler: Option<vk::Sampler>,
    free_descriptor_set: bool,
}

impl ExternalTextureBinding {
    fn borrowed_descriptor_set(descriptor_set: vk::DescriptorSet) -> Self {
        Self {
            descriptor_set,
            image_view: None,
            sampler: None,
            free_descriptor_set: false,
        }
    }

    fn owned_descriptor_set(
        descriptor_set: vk::DescriptorSet,
        image_view: vk::ImageView,
        sampler: vk::Sampler,
    ) -> Self {
        Self {
            descriptor_set,
            image_view: Some(image_view),
            sampler: Some(sampler),
            free_descriptor_set: true,
        }
    }
}

#[derive(Debug)]
struct TextureManager {
    textures: HashMap<u64, VulkanTexture>,
    external_textures: HashMap<u64, ExternalTextureBinding>,
    next_id: u64,
}

impl TextureManager {
    fn new() -> Self {
        Self {
            textures: HashMap::new(),
            external_textures: HashMap::new(),
            next_id: 1,
        }
    }

    fn allocate_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1).max(1);
        id
    }

    fn get_descriptor_set(&self, texture_id: u64) -> Option<vk::DescriptorSet> {
        if let Some(tex) = self.textures.get(&texture_id) {
            Some(tex.descriptor_set)
        } else {
            self.external_textures
                .get(&texture_id)
                .map(|b| b.descriptor_set)
        }
    }

    fn register_external_descriptor_set(&mut self, set: vk::DescriptorSet) -> u64 {
        let id = self.allocate_id();
        self.external_textures
            .insert(id, ExternalTextureBinding::borrowed_descriptor_set(set));
        id
    }

    fn register_external_texture(
        &mut self,
        set: vk::DescriptorSet,
        image_view: vk::ImageView,
        sampler: vk::Sampler,
    ) -> u64 {
        let id = self.allocate_id();
        self.external_textures.insert(
            id,
            ExternalTextureBinding::owned_descriptor_set(set, image_view, sampler),
        );
        id
    }
}

/// Vulkan renderer for Dear ImGui using `ash`.
///
/// It records rendering commands to the provided command buffer and does not submit.
pub struct AshRenderer {
    device: Device,
    allocator: Allocator,
    queue: vk::Queue,
    command_pool: vk::CommandPool,
    pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
    descriptor_set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    textures: TextureManager,
    default_texture_id: u64,
    options: Options,
    frames: Frames,
    destroyed: bool,
    in_flight_uploads: VecDeque<InFlightUpload>,
    #[cfg(feature = "multi-viewport-winit")]
    viewport_pipelines: HashMap<vk::Format, ViewportPipeline>,
    #[cfg(feature = "multi-viewport-winit")]
    viewport_clear_color: [f32; 4],
}

impl AshRenderer {
    /// Configure Dear ImGui context with Vulkan backend capabilities.
    pub fn configure_imgui_context(&self, imgui_context: &mut Context) {
        let should_set_name = imgui_context.io().backend_renderer_name().is_none();
        if should_set_name {
            let _ = imgui_context.set_renderer_name(Some(format!(
                "dear-imgui-ash {}",
                env!("CARGO_PKG_VERSION")
            )));
        }

        let io = imgui_context.io_mut();
        let mut flags = io.backend_flags();
        flags.insert(BackendFlags::RENDERER_HAS_VTX_OFFSET);
        flags.insert(BackendFlags::RENDERER_HAS_TEXTURES);
        #[cfg(feature = "multi-viewport-winit")]
        {
            flags.insert(BackendFlags::RENDERER_HAS_VIEWPORTS);
        }
        io.set_backend_flags(flags);
    }

    /// Create a new renderer using the internal default allocator.
    ///
    /// The provided `command_pool` is used for short-lived upload command buffers.
    #[cfg(not(any(feature = "gpu-allocator", feature = "vk-mem")))]
    pub fn with_default_allocator(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
        device: Device,
        queue: vk::Queue,
        command_pool: vk::CommandPool,
        #[cfg(not(feature = "dynamic-rendering"))] render_pass: vk::RenderPass,
        #[cfg(feature = "dynamic-rendering")] dynamic_rendering: DynamicRendering,
        imgui: &mut Context,
        options: Option<Options>,
    ) -> RendererResult<Self> {
        let memory_properties =
            unsafe { instance.get_physical_device_memory_properties(physical_device) };
        let allocator = Allocator::new(memory_properties);

        Self::init_renderer(
            device,
            allocator,
            queue,
            command_pool,
            #[cfg(not(feature = "dynamic-rendering"))]
            render_pass,
            #[cfg(feature = "dynamic-rendering")]
            dynamic_rendering,
            imgui,
            options,
        )
    }

    /// Create a new renderer using a shared `gpu-allocator` allocator.
    #[cfg(feature = "gpu-allocator")]
    pub fn with_gpu_allocator(
        allocator: std::sync::Arc<std::sync::Mutex<gpu_allocator::vulkan::Allocator>>,
        device: Device,
        queue: vk::Queue,
        command_pool: vk::CommandPool,
        #[cfg(not(feature = "dynamic-rendering"))] render_pass: vk::RenderPass,
        #[cfg(feature = "dynamic-rendering")] dynamic_rendering: DynamicRendering,
        imgui: &mut Context,
        options: Option<Options>,
    ) -> RendererResult<Self> {
        let allocator = Allocator::new(allocator);
        Self::init_renderer(
            device,
            allocator,
            queue,
            command_pool,
            #[cfg(not(feature = "dynamic-rendering"))]
            render_pass,
            #[cfg(feature = "dynamic-rendering")]
            dynamic_rendering,
            imgui,
            options,
        )
    }

    /// Create a new renderer using a shared `vk-mem` allocator.
    #[cfg(feature = "vk-mem")]
    pub fn with_vk_mem_allocator(
        allocator: std::sync::Arc<std::sync::Mutex<vk_mem::Allocator>>,
        device: Device,
        queue: vk::Queue,
        command_pool: vk::CommandPool,
        #[cfg(not(feature = "dynamic-rendering"))] render_pass: vk::RenderPass,
        #[cfg(feature = "dynamic-rendering")] dynamic_rendering: DynamicRendering,
        imgui: &mut Context,
        options: Option<Options>,
    ) -> RendererResult<Self> {
        let allocator = Allocator::new(allocator);
        Self::init_renderer(
            device,
            allocator,
            queue,
            command_pool,
            #[cfg(not(feature = "dynamic-rendering"))]
            render_pass,
            #[cfg(feature = "dynamic-rendering")]
            dynamic_rendering,
            imgui,
            options,
        )
    }

    fn init_renderer(
        device: Device,
        allocator: Allocator,
        queue: vk::Queue,
        command_pool: vk::CommandPool,
        #[cfg(not(feature = "dynamic-rendering"))] render_pass: vk::RenderPass,
        #[cfg(feature = "dynamic-rendering")] dynamic_rendering: DynamicRendering,
        imgui: &mut Context,
        options: Option<Options>,
    ) -> RendererResult<Self> {
        let options = options.unwrap_or_default();
        if options.in_flight_frames == 0 {
            return Err(RendererError::Init(
                "Options::in_flight_frames must be >= 1".to_string(),
            ));
        }

        let descriptor_set_layout = create_vulkan_descriptor_set_layout(&device)?;
        let pipeline_layout = create_vulkan_pipeline_layout(&device, descriptor_set_layout)?;
        let pipeline = create_vulkan_pipeline(
            &device,
            pipeline_layout,
            #[cfg(not(feature = "dynamic-rendering"))]
            render_pass,
            #[cfg(feature = "dynamic-rendering")]
            dynamic_rendering,
            options,
        )?;
        let descriptor_pool = create_vulkan_descriptor_pool(&device, options.max_textures)?;

        let mut renderer = Self {
            device,
            allocator,
            queue,
            command_pool,
            pipeline,
            pipeline_layout,
            descriptor_set_layout,
            descriptor_pool,
            textures: TextureManager::new(),
            default_texture_id: 0,
            options,
            frames: Frames::new(options.in_flight_frames),
            destroyed: false,
            in_flight_uploads: VecDeque::new(),
            #[cfg(feature = "multi-viewport-winit")]
            viewport_pipelines: HashMap::new(),
            #[cfg(feature = "multi-viewport-winit")]
            viewport_clear_color: [0.0, 0.0, 0.0, 1.0],
        };

        renderer.configure_imgui_context(imgui);
        renderer.default_texture_id = renderer.create_default_texture()?;
        Ok(renderer)
    }

    /// Register an external Vulkan `vk::DescriptorSet` and return a `TextureId` for Dear ImGui.
    pub fn register_texture_descriptor_set(&mut self, set: vk::DescriptorSet) -> TextureId {
        TextureId::from(self.textures.register_external_descriptor_set(set))
    }

    /// Remove a previously registered external texture descriptor set.
    pub fn remove_texture_descriptor_set(&mut self, id: TextureId) {
        self.unregister_texture(id);
    }

    /// Register an external `vk::ImageView` + `vk::Sampler` as a legacy `TextureId`.
    ///
    /// This is the Vulkan equivalent of `dear-imgui-wgpu::WgpuRenderer::register_external_texture_with_sampler()`.
    /// The returned `TextureId` can be passed to `ui.image(tex_id, size)`.
    ///
    /// Note: this only allocates a descriptor set; the image and sampler are owned by the caller
    /// and must outlive rendering that references the returned id.
    pub fn register_external_texture_with_sampler(
        &mut self,
        image_view: vk::ImageView,
        sampler: vk::Sampler,
    ) -> RendererResult<TextureId> {
        let set = create_vulkan_descriptor_set(
            &self.device,
            self.descriptor_set_layout,
            self.descriptor_pool,
            image_view,
            sampler,
        )?;
        Ok(TextureId::from(
            self.textures
                .register_external_texture(set, image_view, sampler),
        ))
    }

    /// Update the view for an already-registered external texture.
    ///
    /// Returns false if the texture id is not an external texture registered via
    /// `register_external_texture_with_sampler()`.
    pub fn update_external_texture_view(
        &mut self,
        texture_id: TextureId,
        image_view: vk::ImageView,
    ) -> bool {
        let id = texture_id.id();
        let Some(binding) = self.textures.external_textures.get_mut(&id) else {
            return false;
        };
        if !binding.free_descriptor_set {
            return false;
        }
        let Some(sampler) = binding.sampler else {
            return false;
        };

        binding.image_view = Some(image_view);
        unsafe {
            let image_info = [vk::DescriptorImageInfo {
                sampler,
                image_view,
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            }];
            let write_desc_sets = [vk::WriteDescriptorSet::default()
                .dst_set(binding.descriptor_set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(&image_info)];
            self.device.update_descriptor_sets(&write_desc_sets, &[]);
        }
        true
    }

    /// Update (or set) a custom sampler for an already-registered external texture.
    ///
    /// Returns false if the texture id is not an external texture registered via
    /// `register_external_texture_with_sampler()`.
    pub fn update_external_texture_sampler(
        &mut self,
        texture_id: TextureId,
        sampler: vk::Sampler,
    ) -> bool {
        let id = texture_id.id();
        let Some(binding) = self.textures.external_textures.get_mut(&id) else {
            return false;
        };
        if !binding.free_descriptor_set {
            return false;
        }
        let Some(image_view) = binding.image_view else {
            return false;
        };

        binding.sampler = Some(sampler);
        unsafe {
            let image_info = [vk::DescriptorImageInfo {
                sampler,
                image_view,
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            }];
            let write_desc_sets = [vk::WriteDescriptorSet::default()
                .dst_set(binding.descriptor_set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(&image_info)];
            self.device.update_descriptor_sets(&write_desc_sets, &[]);
        }
        true
    }

    /// Unregister a texture id.
    ///
    /// For external textures registered via `register_external_texture_with_sampler()`, this also
    /// frees the underlying descriptor set from the pool. For descriptor sets registered via
    /// `register_texture_descriptor_set()`, this simply forgets the id (the descriptor set remains
    /// owned by the caller).
    pub fn unregister_texture(&mut self, texture_id: TextureId) {
        let id = texture_id.id();
        if let Some(binding) = self.textures.external_textures.remove(&id) {
            if binding.free_descriptor_set {
                unsafe {
                    let _ = self
                        .device
                        .free_descriptor_sets(self.descriptor_pool, &[binding.descriptor_set]);
                }
            }
        }
    }

    /// Update a single texture manually.
    ///
    /// This mirrors the `dear-imgui-wgpu` API and is useful when the texture is not registered
    /// in ImGui's `PlatformIO.Textures[]` list (e.g. user-created `ImTextureData` that isn't
    /// registered via ImGui's experimental `RegisterUserTexture()` API).
    ///
    /// Call this before rendering if you pass `&mut TextureData` to widgets (e.g. `ui.image()`),
    /// otherwise `ImDrawCmd_GetTexID()` may assert if `TexID` is still invalid.
    pub fn update_texture(
        &mut self,
        texture_data: &TextureData,
    ) -> RendererResult<TextureUpdateResult> {
        self.reap_completed_uploads()?;

        let status = texture_data.status();
        match status {
            TextureStatus::WantCreate => {
                let internal_id = texture_data.tex_id().id();
                let id = if internal_id != 0 && self.textures.textures.contains_key(&internal_id) {
                    internal_id
                } else {
                    self.textures.allocate_id()
                };

                let (w, h) = (texture_data.width() as u32, texture_data.height() as u32);
                if w == 0 || h == 0 {
                    return Ok(TextureUpdateResult::Failed);
                }
                let Some(pixels) = texture_data_to_rgba_full(texture_data) else {
                    return Ok(TextureUpdateResult::Failed);
                };

                let (texture, staging_buffer, staging_mem) = Texture::create(
                    &self.device,
                    &mut self.allocator,
                    w,
                    h,
                    self.options.texture_format,
                    &pixels,
                )?;

                let descriptor_set = create_vulkan_descriptor_set(
                    &self.device,
                    self.descriptor_set_layout,
                    self.descriptor_pool,
                    texture.image_view,
                    texture.sampler,
                )?;

                let (command_buffer, fence) = self.submit_upload_commands(|cmd| {
                    texture.upload(&self.device, cmd, staging_buffer, w, h);
                })?;

                self.in_flight_uploads.push_back(InFlightUpload {
                    fence,
                    command_buffer,
                    staging: vec![(staging_buffer, staging_mem)],
                });

                if let Some(old) = self.textures.textures.remove(&id) {
                    old.destroy(&self.device, &mut self.allocator, self.descriptor_pool);
                }
                self.textures.textures.insert(
                    id,
                    VulkanTexture {
                        image: texture.image,
                        image_mem: texture.image_mem,
                        image_view: texture.image_view,
                        sampler: texture.sampler,
                        descriptor_set,
                        width: w,
                        height: h,
                    },
                );

                Ok(TextureUpdateResult::Created {
                    texture_id: TextureId::from(id),
                })
            }
            TextureStatus::WantUpdates => {
                let internal_id = texture_data.tex_id().id();
                if internal_id == 0 || !self.textures.textures.contains_key(&internal_id) {
                    // Not created yet: treat updates as a full create.
                    return self.update_texture_with_forced_create(texture_data);
                }

                let Some(existing) = self.textures.textures.get(&internal_id) else {
                    return Ok(TextureUpdateResult::Failed);
                };

                let (tw, th) = (existing.width, existing.height);
                let rect = texture_data.update_rect();
                let (x, y, w, h) = clamp_rect(rect, tw, th);
                if w == 0 || h == 0 {
                    return Ok(TextureUpdateResult::Updated);
                }

                let Some(pixels) = texture_data_to_rgba_subrect(texture_data, x, y, w, h) else {
                    return Ok(TextureUpdateResult::Failed);
                };
                let (staging_buffer, staging_mem) = create_and_fill_buffer(
                    &self.device,
                    &mut self.allocator,
                    &pixels,
                    vk::BufferUsageFlags::TRANSFER_SRC,
                )?;

                let (command_buffer, fence) = self.submit_upload_commands(|cmd| {
                    upload_rgba_subrect_to_image(
                        &self.device,
                        cmd,
                        staging_buffer,
                        existing.image,
                        x,
                        y,
                        w,
                        h,
                    );
                })?;

                self.in_flight_uploads.push_back(InFlightUpload {
                    fence,
                    command_buffer,
                    staging: vec![(staging_buffer, staging_mem)],
                });

                Ok(TextureUpdateResult::Updated)
            }
            TextureStatus::WantDestroy => {
                let id = texture_data.tex_id().id();
                if let Some(tex) = self.textures.textures.remove(&id) {
                    tex.destroy(&self.device, &mut self.allocator, self.descriptor_pool);
                }
                Ok(TextureUpdateResult::Destroyed)
            }
            TextureStatus::OK | TextureStatus::Destroyed => Ok(TextureUpdateResult::NoAction),
        }
    }

    fn update_texture_with_forced_create(
        &mut self,
        texture_data: &TextureData,
    ) -> RendererResult<TextureUpdateResult> {
        // Force-create by temporarily treating it as WantCreate.
        // We don't mutate the passed-in TextureData here; the returned result will set TexID/Status.
        let internal_id = texture_data.tex_id().id();
        let id = if internal_id != 0 && self.textures.textures.contains_key(&internal_id) {
            internal_id
        } else {
            self.textures.allocate_id()
        };

        let (w, h) = (texture_data.width() as u32, texture_data.height() as u32);
        if w == 0 || h == 0 {
            return Ok(TextureUpdateResult::Failed);
        }
        let Some(pixels) = texture_data_to_rgba_full(texture_data) else {
            return Ok(TextureUpdateResult::Failed);
        };

        let (texture, staging_buffer, staging_mem) = Texture::create(
            &self.device,
            &mut self.allocator,
            w,
            h,
            self.options.texture_format,
            &pixels,
        )?;

        let descriptor_set = create_vulkan_descriptor_set(
            &self.device,
            self.descriptor_set_layout,
            self.descriptor_pool,
            texture.image_view,
            texture.sampler,
        )?;

        let (command_buffer, fence) = self.submit_upload_commands(|cmd| {
            texture.upload(&self.device, cmd, staging_buffer, w, h);
        })?;

        self.in_flight_uploads.push_back(InFlightUpload {
            fence,
            command_buffer,
            staging: vec![(staging_buffer, staging_mem)],
        });

        if let Some(old) = self.textures.textures.remove(&id) {
            old.destroy(&self.device, &mut self.allocator, self.descriptor_pool);
        }
        self.textures.textures.insert(
            id,
            VulkanTexture {
                image: texture.image,
                image_mem: texture.image_mem,
                image_view: texture.image_view,
                sampler: texture.sampler,
                descriptor_set,
                width: w,
                height: h,
            },
        );

        Ok(TextureUpdateResult::Created {
            texture_id: TextureId::from(id),
        })
    }

    /// Get the current renderer options.
    pub fn options(&self) -> Options {
        self.options
    }

    /// Set clear color for secondary viewports (multi-viewport mode).
    #[cfg(feature = "multi-viewport-winit")]
    pub fn set_viewport_clear_color(&mut self, color: [f32; 4]) {
        self.viewport_clear_color = color;
    }

    /// Get clear color for secondary viewports (multi-viewport mode).
    #[cfg(feature = "multi-viewport-winit")]
    pub fn viewport_clear_color(&self) -> [f32; 4] {
        self.viewport_clear_color
    }

    fn gamma(&self) -> f32 {
        self.options
            .color_gamma_override
            .unwrap_or(if self.options.framebuffer_srgb {
                2.2_f32
            } else {
                1.0_f32
            })
    }

    #[cfg(feature = "multi-viewport-winit")]
    fn gamma_for_format(&self, format: vk::Format) -> f32 {
        self.options
            .color_gamma_override
            .unwrap_or(if is_srgb_format(format) {
                2.2_f32
            } else {
                1.0_f32
            })
    }

    #[cfg(feature = "multi-viewport-winit")]
    fn viewport_pipeline(&mut self, format: vk::Format) -> RendererResult<&ViewportPipeline> {
        if self.viewport_pipelines.contains_key(&format) {
            return Ok(self
                .viewport_pipelines
                .get(&format)
                .expect("checked contains_key"));
        }

        let options = Options {
            // Viewports are rendered by ImGui itself; keep it simple and disable depth/MSAA.
            in_flight_frames: 1,
            enable_depth_test: false,
            enable_depth_write: false,
            subpass: 0,
            sample_count: vk::SampleCountFlags::TYPE_1,
            max_textures: self.options.max_textures,
            framebuffer_srgb: false,
            color_gamma_override: self.options.color_gamma_override,
            texture_format: self.options.texture_format,
        };

        #[cfg(not(feature = "dynamic-rendering"))]
        let render_pass = create_viewport_render_pass(&self.device, format)?;

        let pipeline = create_vulkan_pipeline(
            &self.device,
            self.pipeline_layout,
            #[cfg(not(feature = "dynamic-rendering"))]
            render_pass,
            #[cfg(feature = "dynamic-rendering")]
            DynamicRendering {
                color_attachment_format: format,
                depth_attachment_format: None,
            },
            options,
        )?;

        let vp = ViewportPipeline {
            pipeline,
            #[cfg(not(feature = "dynamic-rendering"))]
            render_pass,
        };

        self.viewport_pipelines.insert(format, vp);
        Ok(self.viewport_pipelines.get(&format).expect("just inserted"))
    }

    fn submit_upload_commands<F>(&self, record: F) -> RendererResult<(vk::CommandBuffer, vk::Fence)>
    where
        F: FnOnce(vk::CommandBuffer),
    {
        let command_buffer = unsafe {
            let alloc_info = vk::CommandBufferAllocateInfo::default()
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_pool(self.command_pool)
                .command_buffer_count(1);
            self.device.allocate_command_buffers(&alloc_info)?[0]
        };

        unsafe {
            let begin_info = vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
            self.device
                .begin_command_buffer(command_buffer, &begin_info)?;
        }

        record(command_buffer);

        unsafe {
            self.device.end_command_buffer(command_buffer)?;
        }

        let fence = unsafe {
            self.device
                .create_fence(&vk::FenceCreateInfo::default(), None)?
        };
        let submit_info =
            vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&command_buffer));
        unsafe {
            self.device
                .queue_submit(self.queue, std::slice::from_ref(&submit_info), fence)?;
        }

        Ok((command_buffer, fence))
    }

    fn reap_completed_uploads(&mut self) -> RendererResult<()> {
        while let Some(front) = self.in_flight_uploads.front() {
            let done = unsafe { self.device.get_fence_status(front.fence)? };
            if !done {
                break;
            }

            let upload = self.in_flight_uploads.pop_front().expect("front exists");
            for (buffer, mem) in upload.staging {
                self.allocator.destroy_buffer(&self.device, buffer, mem)?;
            }
            unsafe {
                self.device
                    .free_command_buffers(self.command_pool, &[upload.command_buffer]);
                self.device.destroy_fence(upload.fence, None);
            }
        }
        Ok(())
    }

    fn reap_all_uploads(&mut self) -> RendererResult<()> {
        while let Some(upload) = self.in_flight_uploads.pop_front() {
            for (buffer, mem) in upload.staging {
                self.allocator.destroy_buffer(&self.device, buffer, mem)?;
            }
            unsafe {
                self.device
                    .free_command_buffers(self.command_pool, &[upload.command_buffer]);
                self.device.destroy_fence(upload.fence, None);
            }
        }
        Ok(())
    }

    /// Record draw commands into `command_buffer`.
    ///
    /// Requirements:
    /// - `command_buffer` must be in the correct render pass/subpass matching the pipeline.
    /// - Caller is responsible for synchronization.
    pub fn cmd_draw(
        &mut self,
        command_buffer: vk::CommandBuffer,
        draw_data: &dear_imgui_rs::render::DrawData,
    ) -> RendererResult<()> {
        let gamma = self.gamma();
        if !draw_data.valid() || draw_data.total_vtx_count == 0 {
            return Ok(());
        }

        self.reap_completed_uploads()?;
        self.process_texture_requests(draw_data)?;

        let Some(mesh) = self.frames.next() else {
            return Err(RendererError::Init("frames not initialized".to_string()));
        };
        record_draw_commands(
            &self.device,
            &mut self.allocator,
            &self.textures,
            self.default_texture_id,
            self.pipeline_layout,
            command_buffer,
            draw_data,
            self.pipeline,
            gamma,
            mesh,
        )
    }

    fn cmd_draw_with_mesh(
        &mut self,
        command_buffer: vk::CommandBuffer,
        draw_data: &dear_imgui_rs::render::DrawData,
        pipeline: vk::Pipeline,
        gamma: f32,
        mesh: &mut Mesh,
    ) -> RendererResult<()> {
        if !draw_data.valid() || draw_data.total_vtx_count == 0 {
            return Ok(());
        }

        self.reap_completed_uploads()?;
        self.process_texture_requests(draw_data)?;
        record_draw_commands(
            &self.device,
            &mut self.allocator,
            &self.textures,
            self.default_texture_id,
            self.pipeline_layout,
            command_buffer,
            draw_data,
            pipeline,
            gamma,
            mesh,
        )
    }

    fn create_default_texture(&mut self) -> RendererResult<u64> {
        // 1x1 white RGBA.
        let pixels = [255u8, 255u8, 255u8, 255u8];
        let texture_id = self.textures.allocate_id();

        let (texture, staging_buffer, staging_mem) = Texture::create(
            &self.device,
            &mut self.allocator,
            1,
            1,
            self.options.texture_format,
            &pixels,
        )?;

        execute_one_time_commands(&self.device, self.queue, self.command_pool, |cmd| {
            texture.upload(&self.device, cmd, staging_buffer, 1, 1);
        })?;

        self.allocator
            .destroy_buffer(&self.device, staging_buffer, staging_mem)?;

        let descriptor_set = create_vulkan_descriptor_set(
            &self.device,
            self.descriptor_set_layout,
            self.descriptor_pool,
            texture.image_view,
            texture.sampler,
        )?;

        self.textures.textures.insert(
            texture_id,
            VulkanTexture {
                image: texture.image,
                image_mem: texture.image_mem,
                image_view: texture.image_view,
                sampler: texture.sampler,
                descriptor_set,
                width: 1,
                height: 1,
            },
        );

        Ok(texture_id)
    }

    fn process_texture_requests(
        &mut self,
        draw_data: &dear_imgui_rs::render::DrawData,
    ) -> RendererResult<()> {
        struct PendingCreate {
            id: u64,
            texture: Texture,
            descriptor_set: vk::DescriptorSet,
            staging_buffer: vk::Buffer,
            staging_mem: Memory,
            w: u32,
            h: u32,
        }

        struct PendingUpdate {
            image: vk::Image,
            staging_buffer: vk::Buffer,
            staging_mem: Memory,
            x: u32,
            y: u32,
            w: u32,
            h: u32,
        }

        let mut creates: Vec<PendingCreate> = Vec::new();
        let mut updates: Vec<PendingUpdate> = Vec::new();

        for mut td in draw_data.textures() {
            let status = td.status();
            let internal_id = td.tex_id().id();
            let needs_create = matches!(status, TextureStatus::WantCreate)
                || (matches!(status, TextureStatus::WantUpdates)
                    && (internal_id == 0 || !self.textures.textures.contains_key(&internal_id)));

            if needs_create {
                let id = if internal_id == 0 || !self.textures.textures.contains_key(&internal_id) {
                    self.textures.allocate_id()
                } else {
                    internal_id
                };

                let (w, h) = (td.width() as u32, td.height() as u32);
                if w == 0 || h == 0 {
                    continue;
                }
                let Some(pixels) = texture_data_to_rgba_full(&td) else {
                    continue;
                };

                let (texture, staging_buffer, staging_mem) = Texture::create(
                    &self.device,
                    &mut self.allocator,
                    w,
                    h,
                    self.options.texture_format,
                    &pixels,
                )?;

                let descriptor_set = create_vulkan_descriptor_set(
                    &self.device,
                    self.descriptor_set_layout,
                    self.descriptor_pool,
                    texture.image_view,
                    texture.sampler,
                )?;

                creates.push(PendingCreate {
                    id,
                    texture,
                    descriptor_set,
                    staging_buffer,
                    staging_mem,
                    w,
                    h,
                });

                td.set_tex_id(TextureId::from(id));
                td.set_status(TextureStatus::OK);
                continue;
            }

            match status {
                TextureStatus::WantCreate => {
                    // Handled by `needs_create` branch above.
                }
                TextureStatus::WantUpdates => {
                    let id = internal_id;
                    let Some(existing) = self.textures.textures.get(&id) else {
                        // If the backend lost its copy but ImGui still asks for updates, fall back
                        // to a full recreate in the next frame.
                        td.set_status(TextureStatus::WantCreate);
                        continue;
                    };

                    let (tw, th) = (existing.width, existing.height);
                    if tw == 0 || th == 0 {
                        continue;
                    }

                    let rect = td.update_rect();
                    let (x, y, w, h) = clamp_rect(rect, tw, th);
                    if w == 0 || h == 0 {
                        continue;
                    }

                    let Some(pixels) = texture_data_to_rgba_subrect(&td, x, y, w, h) else {
                        continue;
                    };
                    let (staging_buffer, staging_mem) = create_and_fill_buffer(
                        &self.device,
                        &mut self.allocator,
                        &pixels,
                        vk::BufferUsageFlags::TRANSFER_SRC,
                    )?;

                    updates.push(PendingUpdate {
                        image: existing.image,
                        staging_buffer,
                        staging_mem,
                        x,
                        y,
                        w,
                        h,
                    });

                    td.set_status(TextureStatus::OK);
                }
                TextureStatus::WantDestroy => {
                    let id = internal_id;
                    if let Some(tex) = self.textures.textures.remove(&id) {
                        tex.destroy(&self.device, &mut self.allocator, self.descriptor_pool);
                    }
                    unsafe {
                        (*td.as_raw_mut()).WantDestroyNextFrame = true;
                    }
                    td.set_status(TextureStatus::Destroyed);
                }
                TextureStatus::OK | TextureStatus::Destroyed => {}
            }
        }

        if !creates.is_empty() || !updates.is_empty() {
            let (command_buffer, fence) = self.submit_upload_commands(|cmd| {
                for c in &creates {
                    c.texture
                        .upload(&self.device, cmd, c.staging_buffer, c.w, c.h);
                }
                for u in &updates {
                    upload_rgba_subrect_to_image(
                        &self.device,
                        cmd,
                        u.staging_buffer,
                        u.image,
                        u.x,
                        u.y,
                        u.w,
                        u.h,
                    );
                }
            })?;

            let mut staging: Vec<(vk::Buffer, Memory)> =
                Vec::with_capacity(creates.len() + updates.len());
            for c in &creates {
                staging.push((c.staging_buffer, c.staging_mem));
            }
            for u in &updates {
                staging.push((u.staging_buffer, u.staging_mem));
            }

            self.in_flight_uploads.push_back(InFlightUpload {
                fence,
                command_buffer,
                staging,
            });
        }

        for c in creates {
            if let Some(old) = self.textures.textures.remove(&c.id) {
                old.destroy(&self.device, &mut self.allocator, self.descriptor_pool);
            }

            self.textures.textures.insert(
                c.id,
                VulkanTexture {
                    image: c.texture.image,
                    image_mem: c.texture.image_mem,
                    image_view: c.texture.image_view,
                    sampler: c.texture.sampler,
                    descriptor_set: c.descriptor_set,
                    width: c.w,
                    height: c.h,
                },
            );
        }

        Ok(())
    }

    fn destroy_internal(&mut self) {
        if self.destroyed {
            return;
        }
        self.destroyed = true;

        // Best-effort: ensure in-flight uploads are complete before freeing staging memory.
        let _ = unsafe { self.device.device_wait_idle() };
        let _ = self.reap_all_uploads();

        let textures = std::mem::take(&mut self.textures.textures);
        for (_, tex) in textures {
            tex.destroy(&self.device, &mut self.allocator, self.descriptor_pool);
        }

        unsafe {
            #[cfg(feature = "multi-viewport-winit")]
            {
                // Ensure callbacks cannot reach this renderer during teardown.
                multi_viewport::clear_for_drop(self as *mut _);

                let viewport_pipelines = std::mem::take(&mut self.viewport_pipelines);
                for (_, vp) in viewport_pipelines {
                    self.device.destroy_pipeline(vp.pipeline, None);
                    #[cfg(not(feature = "dynamic-rendering"))]
                    self.device.destroy_render_pass(vp.render_pass, None);
                }
            }

            self.device
                .destroy_descriptor_pool(self.descriptor_pool, None);
            self.device
                .destroy_descriptor_set_layout(self.descriptor_set_layout, None);
            self.device.destroy_pipeline(self.pipeline, None);
            self.device
                .destroy_pipeline_layout(self.pipeline_layout, None);
        }

        let frames = std::mem::replace(&mut self.frames, Frames::new(0));
        let _ = frames.destroy(&self.device, &mut self.allocator);
    }
}

impl Drop for AshRenderer {
    fn drop(&mut self) {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.destroy_internal();
        }));
    }
}

struct InFlightUpload {
    fence: vk::Fence,
    command_buffer: vk::CommandBuffer,
    staging: Vec<(vk::Buffer, Memory)>,
}

struct Frames {
    meshes: Vec<Mesh>,
    index: usize,
}

impl Frames {
    fn new(count: usize) -> Self {
        Self {
            meshes: (0..count).map(|_| Mesh::default()).collect(),
            index: 0,
        }
    }

    fn next(&mut self) -> Option<&mut Mesh> {
        if self.meshes.is_empty() {
            return None;
        }
        let i = self.index;
        self.index = (self.index + 1) % self.meshes.len();
        Some(&mut self.meshes[i])
    }

    fn destroy(self, device: &Device, allocator: &mut Allocator) -> RendererResult<()> {
        for mesh in self.meshes {
            mesh.destroy(device, allocator)?;
        }
        Ok(())
    }
}

#[derive(Default)]
struct Mesh {
    vertices: vk::Buffer,
    vertices_mem: Option<Memory>,
    vertex_capacity: usize,
    indices: vk::Buffer,
    indices_mem: Option<Memory>,
    index_capacity: usize,
}

impl Mesh {
    fn update(
        &mut self,
        device: &Device,
        allocator: &mut Allocator,
        draw_data: &dear_imgui_rs::render::DrawData,
    ) -> RendererResult<()> {
        let vertices = create_vertices(draw_data);
        if vertices.len() > self.vertex_capacity {
            if self.vertices != vk::Buffer::null() {
                if let Some(mem) = self.vertices_mem.take() {
                    allocator.destroy_buffer(device, self.vertices, mem)?;
                }
            }
            let size = vertices
                .len()
                .checked_mul(std::mem::size_of::<dear_imgui_rs::render::DrawVert>())
                .ok_or_else(|| RendererError::Allocator("vertex buffer size overflow".into()))?;
            let (buffer, mem) =
                allocator.create_buffer(device, size, vk::BufferUsageFlags::VERTEX_BUFFER)?;
            self.vertices = buffer;
            self.vertices_mem = Some(mem);
            self.vertex_capacity = vertices.len();
        }
        if let Some(mem) = self.vertices_mem.as_mut() {
            allocator.update_buffer(device, mem, &vertices)?;
        }

        let indices = create_indices(draw_data);
        if indices.len() > self.index_capacity {
            if self.indices != vk::Buffer::null() {
                if let Some(mem) = self.indices_mem.take() {
                    allocator.destroy_buffer(device, self.indices, mem)?;
                }
            }
            let size = indices
                .len()
                .checked_mul(std::mem::size_of::<dear_imgui_rs::render::DrawIdx>())
                .ok_or_else(|| RendererError::Allocator("index buffer size overflow".into()))?;
            let (buffer, mem) =
                allocator.create_buffer(device, size, vk::BufferUsageFlags::INDEX_BUFFER)?;
            self.indices = buffer;
            self.indices_mem = Some(mem);
            self.index_capacity = indices.len();
        }
        if let Some(mem) = self.indices_mem.as_mut() {
            allocator.update_buffer(device, mem, &indices)?;
        }

        Ok(())
    }

    fn destroy(self, device: &Device, allocator: &mut Allocator) -> RendererResult<()> {
        if self.vertices != vk::Buffer::null() {
            if let Some(mem) = self.vertices_mem {
                allocator.destroy_buffer(device, self.vertices, mem)?;
            }
        }
        if self.indices != vk::Buffer::null() {
            if let Some(mem) = self.indices_mem {
                allocator.destroy_buffer(device, self.indices, mem)?;
            }
        }
        Ok(())
    }
}

fn create_vertices(
    draw_data: &dear_imgui_rs::render::DrawData,
) -> Vec<dear_imgui_rs::render::DrawVert> {
    let vertex_count = draw_data.total_vtx_count as usize;
    let mut vertices = Vec::with_capacity(vertex_count);
    for draw_list in draw_data.draw_lists() {
        vertices.extend_from_slice(draw_list.vtx_buffer());
    }
    vertices
}

fn create_indices(
    draw_data: &dear_imgui_rs::render::DrawData,
) -> Vec<dear_imgui_rs::render::DrawIdx> {
    let index_count = draw_data.total_idx_count as usize;
    let mut indices = Vec::with_capacity(index_count);
    for draw_list in draw_data.draw_lists() {
        indices.extend_from_slice(draw_list.idx_buffer());
    }
    indices
}

fn record_draw_commands(
    device: &Device,
    allocator: &mut Allocator,
    textures: &TextureManager,
    default_texture_id: u64,
    pipeline_layout: vk::PipelineLayout,
    command_buffer: vk::CommandBuffer,
    draw_data: &dear_imgui_rs::render::DrawData,
    pipeline: vk::Pipeline,
    gamma: f32,
    mesh: &mut Mesh,
) -> RendererResult<()> {
    let fb_width = (draw_data.display_size[0] * draw_data.framebuffer_scale[0]).round();
    let fb_height = (draw_data.display_size[1] * draw_data.framebuffer_scale[1]).round();
    if fb_width <= 0.0 || fb_height <= 0.0 {
        return Ok(());
    }
    let fb_width_u32 = fb_width as u32;
    let fb_height_u32 = fb_height as u32;

    mesh.update(device, allocator, draw_data)?;

    let viewport = vk::Viewport {
        x: 0.0,
        y: 0.0,
        width: fb_width,
        height: fb_height,
        min_depth: 0.0,
        max_depth: 1.0,
    };

    let ortho = ortho_matrix_vk(draw_data.display_pos, draw_data.display_size);
    let push_constants = PushConstants {
        ortho,
        gamma_pad: [gamma, 0.0, 0.0, 0.0],
    };

    unsafe {
        device.cmd_bind_pipeline(command_buffer, vk::PipelineBindPoint::GRAPHICS, pipeline);
        device.cmd_set_viewport(command_buffer, 0, &[viewport]);
        device.cmd_push_constants(
            command_buffer,
            pipeline_layout,
            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            0,
            any_as_u8_slice(&push_constants),
        );
        device.cmd_bind_vertex_buffers(command_buffer, 0, &[mesh.vertices], &[0]);
        device.cmd_bind_index_buffer(command_buffer, mesh.indices, 0, vk::IndexType::UINT16);
    }

    let clip_off = draw_data.display_pos;
    let clip_scale = draw_data.framebuffer_scale;

    let mut global_vtx_offset: i32 = 0;
    let mut global_idx_offset: u32 = 0;

    for draw_list in draw_data.draw_lists() {
        for cmd in draw_list.commands() {
            match cmd {
                dear_imgui_rs::render::DrawCmd::Elements {
                    count,
                    cmd_params,
                    raw_cmd,
                } => {
                    let tex_id = resolve_effective_texture_id(cmd_params.texture_id, raw_cmd);
                    let ds = textures
                        .get_descriptor_set(tex_id.id())
                        .or_else(|| {
                            if tex_id.is_null() {
                                textures.get_descriptor_set(default_texture_id)
                            } else {
                                None
                            }
                        })
                        .ok_or_else(|| RendererError::BadTextureId(tex_id.id()))?;

                    let scissor = clip_rect_to_scissor(
                        cmd_params.clip_rect,
                        clip_off,
                        clip_scale,
                        fb_width_u32,
                        fb_height_u32,
                    );
                    let Some(scissor) = scissor else {
                        continue;
                    };

                    unsafe {
                        device.cmd_set_scissor(command_buffer, 0, &[scissor]);
                        device.cmd_bind_descriptor_sets(
                            command_buffer,
                            vk::PipelineBindPoint::GRAPHICS,
                            pipeline_layout,
                            0,
                            &[ds],
                            &[],
                        );
                    }

                    let Some(count_u32) = u32::try_from(count).ok() else {
                        continue;
                    };
                    let Some(idx_offset_u32) = u32::try_from(cmd_params.idx_offset).ok() else {
                        continue;
                    };
                    let Some(first_index) = idx_offset_u32.checked_add(global_idx_offset) else {
                        continue;
                    };
                    let Ok(vtx_offset_i32) = i32::try_from(cmd_params.vtx_offset) else {
                        continue;
                    };
                    let Some(vertex_offset) = vtx_offset_i32.checked_add(global_vtx_offset) else {
                        continue;
                    };

                    unsafe {
                        device.cmd_draw_indexed(
                            command_buffer,
                            count_u32,
                            1,
                            first_index,
                            vertex_offset,
                            0,
                        );
                    }
                }
                dear_imgui_rs::render::DrawCmd::ResetRenderState => unsafe {
                    device.cmd_bind_pipeline(
                        command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        pipeline,
                    );
                    device.cmd_set_viewport(command_buffer, 0, &[viewport]);
                    device.cmd_push_constants(
                        command_buffer,
                        pipeline_layout,
                        vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                        0,
                        any_as_u8_slice(&push_constants),
                    );
                    device.cmd_bind_vertex_buffers(command_buffer, 0, &[mesh.vertices], &[0]);
                    device.cmd_bind_index_buffer(
                        command_buffer,
                        mesh.indices,
                        0,
                        vk::IndexType::UINT16,
                    );
                },
                dear_imgui_rs::render::DrawCmd::RawCallback { .. } => {
                    // Skip raw callbacks.
                }
            }
        }

        global_idx_offset = global_idx_offset.saturating_add(draw_list.idx_buffer().len() as u32);
        global_vtx_offset = global_vtx_offset.saturating_add(draw_list.vtx_buffer().len() as i32);
    }

    Ok(())
}

fn resolve_effective_texture_id(
    legacy: TextureId,
    raw_cmd: *const dear_imgui_rs::sys::ImDrawCmd,
) -> TextureId {
    if raw_cmd.is_null() {
        return legacy;
    }
    unsafe {
        let mut copy = *raw_cmd;
        TextureId::from(dear_imgui_rs::sys::ImDrawCmd_GetTexID(&mut copy))
    }
}

fn texture_data_to_rgba_full(td: &TextureData) -> Option<Vec<u8>> {
    let w = td.width() as u32;
    let h = td.height() as u32;
    if w == 0 || h == 0 {
        return None;
    }
    texture_data_to_rgba_subrect(td, 0, 0, w, h)
}

fn texture_data_to_rgba_subrect(
    td: &TextureData,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
) -> Option<Vec<u8>> {
    let pixels = td.pixels()?;
    let tex_w = td.width() as usize;
    let tex_h = td.height() as usize;
    if tex_w == 0 || tex_h == 0 {
        return None;
    }

    let (x, y, w, h) = (x as usize, y as usize, w as usize, h as usize);
    if w == 0 || h == 0 || x >= tex_w || y >= tex_h {
        return None;
    }
    let w = w.min(tex_w.saturating_sub(x));
    let h = h.min(tex_h.saturating_sub(y));
    let bpp = td.bytes_per_pixel() as usize;

    let mut out = vec![0u8; w.checked_mul(h)?.checked_mul(4)?];
    match td.format() {
        ImGuiTextureFormat::RGBA32 => {
            for row in 0..h {
                let src_off = ((y + row) * tex_w + x) * bpp;
                let dst_off = row * w * 4;
                out[dst_off..dst_off + w * 4].copy_from_slice(&pixels[src_off..src_off + w * 4]);
            }
        }
        ImGuiTextureFormat::Alpha8 => {
            for row in 0..h {
                let src_off = ((y + row) * tex_w + x) * bpp;
                let dst_off = row * w * 4;
                for col in 0..w {
                    let a = pixels[src_off + col];
                    let o = dst_off + col * 4;
                    out[o..o + 4].copy_from_slice(&[255, 255, 255, a]);
                }
            }
        }
    }

    Some(out)
}

fn clamp_rect(rect: dear_imgui_rs::texture::TextureRect, tw: u32, th: u32) -> (u32, u32, u32, u32) {
    let x = u32::from(rect.x).min(tw);
    let y = u32::from(rect.y).min(th);
    let w = u32::from(rect.w);
    let h = u32::from(rect.h);
    if w == 0 || h == 0 || x >= tw || y >= th {
        return (x, y, 0, 0);
    }
    (x, y, w.min(tw - x), h.min(th - y))
}

#[cfg(feature = "multi-viewport-winit")]
fn is_srgb_format(format: vk::Format) -> bool {
    matches!(
        format,
        vk::Format::B8G8R8A8_SRGB | vk::Format::R8G8B8A8_SRGB | vk::Format::A8B8G8R8_SRGB_PACK32
    )
}

#[cfg(all(feature = "multi-viewport-winit", not(feature = "dynamic-rendering")))]
fn create_viewport_render_pass(
    device: &Device,
    format: vk::Format,
) -> RendererResult<vk::RenderPass> {
    let attachments = [vk::AttachmentDescription::default()
        .format(format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        // Swapchain images are in PRESENT_SRC_KHR when acquired.
        .initial_layout(vk::ImageLayout::PRESENT_SRC_KHR)
        .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)];

    let color_attachment_refs = [vk::AttachmentReference::default()
        .attachment(0)
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];

    let subpass = [vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&color_attachment_refs)];

    let dependencies = [vk::SubpassDependency::default()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_access_mask(
            vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        )];

    let rp_info = vk::RenderPassCreateInfo::default()
        .attachments(&attachments)
        .subpasses(&subpass)
        .dependencies(&dependencies);
    unsafe { Ok(device.create_render_pass(&rp_info, None)?) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dear_imgui_rs::texture::{TextureData, TextureFormat as ImFormat};

    #[test]
    fn texture_subrect_rgba32() {
        let mut tex = TextureData::new();
        tex.create(ImFormat::RGBA32, 2, 2);
        let pixels: [u8; 16] = [
            10, 20, 30, 40, // (0,0)
            50, 60, 70, 80, // (1,0)
            90, 100, 110, 120, // (0,1)
            130, 140, 150, 160, // (1,1)
        ];
        tex.set_data(&pixels);

        let out = texture_data_to_rgba_subrect(&tex, 1, 0, 1, 1).unwrap();
        assert_eq!(out, vec![50, 60, 70, 80]);
    }

    #[test]
    fn texture_subrect_alpha8() {
        let mut tex = TextureData::new();
        tex.create(ImFormat::Alpha8, 2, 2);
        let alphas: [u8; 4] = [0, 64, 128, 255];
        tex.set_data(&alphas);

        let out = texture_data_to_rgba_subrect(&tex, 0, 1, 2, 1).unwrap();
        assert_eq!(
            out,
            vec![
                255, 255, 255, 128, //
                255, 255, 255, 255,
            ]
        );
    }
}
