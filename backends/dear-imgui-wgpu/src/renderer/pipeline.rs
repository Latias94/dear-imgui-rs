// Renderer pipeline and device-objects creation

use super::*;

impl WgpuRenderer {
    /// Create device objects (pipeline, etc.)
    ///
    /// This corresponds to ImGui_ImplWGPU_CreateDeviceObjects in the C++ implementation
    pub(super) fn create_device_objects(
        &mut self,
        backend_data: &mut WgpuBackendData,
    ) -> RendererResult<()> {
        let device = &backend_data.device;

        // Reuse bind group layouts from shared render resources so that the
        // pipeline layout and the actually bound groups share the same layouts.
        let uniform_layout = backend_data
            .render_resources
            .uniform_buffer()
            .map(|ub| ub.bind_group_layout())
            .ok_or_else(|| {
                RendererError::InvalidRenderState("Uniform buffer not initialized".to_string())
            })?;

        let image_layout = backend_data
            .render_resources
            .image_bind_group_layout()
            .ok_or_else(|| {
                RendererError::InvalidRenderState(
                    "Image bind group layout not initialized".to_string(),
                )
            })?;

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Dear ImGui Pipeline Layout"),
            bind_group_layouts: &[uniform_layout, image_layout],
            push_constant_ranges: &[],
        });

        // Get shader module
        let shader_module = self.shader_manager.get_shader_module()?;

        // Create vertex buffer layout
        let vertex_buffer_layout = crate::shaders::create_vertex_buffer_layout();
        let vertex_buffer_layouts = [vertex_buffer_layout];

        // Create vertex state
        let vertex_state =
            crate::shaders::create_vertex_state(shader_module, &vertex_buffer_layouts);

        // Create color target state
        let color_target = ColorTargetState {
            format: backend_data.render_target_format,
            blend: Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::SrcAlpha,
                    dst_factor: BlendFactor::OneMinusSrcAlpha,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::OneMinusSrcAlpha,
                    operation: BlendOperation::Add,
                },
            }),
            write_mask: ColorWrites::ALL,
        };

        // Determine if we need gamma correction based on format
        let use_gamma_correction = matches!(
            backend_data.render_target_format,
            TextureFormat::Rgba8UnormSrgb
                | TextureFormat::Bgra8UnormSrgb
                | TextureFormat::Bc1RgbaUnormSrgb
                | TextureFormat::Bc2RgbaUnormSrgb
                | TextureFormat::Bc3RgbaUnormSrgb
                | TextureFormat::Bc7RgbaUnormSrgb
        );

        // Create fragment state
        let color_targets = [Some(color_target)];
        let fragment_state = crate::shaders::create_fragment_state(
            shader_module,
            &color_targets,
            use_gamma_correction,
        );

        // Create depth stencil state if needed (matches imgui_impl_wgpu.cpp depth-stencil setup)
        let depth_stencil = backend_data
            .depth_stencil_format
            .map(|format| DepthStencilState {
                format,
                depth_write_enabled: false, // matches WGPUOptionalBool_False in C++
                depth_compare: CompareFunction::Always, // matches WGPUCompareFunction_Always
                stencil: StencilState {
                    front: StencilFaceState {
                        compare: CompareFunction::Always, // matches WGPUCompareFunction_Always
                        fail_op: StencilOperation::Keep,  // matches WGPUStencilOperation_Keep
                        depth_fail_op: StencilOperation::Keep, // matches WGPUStencilOperation_Keep
                        pass_op: StencilOperation::Keep,  // matches WGPUStencilOperation_Keep
                    },
                    back: StencilFaceState {
                        compare: CompareFunction::Always, // matches WGPUCompareFunction_Always
                        fail_op: StencilOperation::Keep,  // matches WGPUStencilOperation_Keep
                        depth_fail_op: StencilOperation::Keep, // matches WGPUStencilOperation_Keep
                        pass_op: StencilOperation::Keep,  // matches WGPUStencilOperation_Keep
                    },
                    read_mask: 0xff,  // default value
                    write_mask: 0xff, // default value
                },
                bias: DepthBiasState::default(),
            });

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Dear ImGui Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: vertex_state,
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Cw,
                cull_mode: None,
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil,
            multisample: backend_data.init_info.pipeline_multisample_state,
            fragment: Some(fragment_state),
            multiview: None,
            cache: None,
        });

        backend_data.pipeline_state = Some(pipeline);
        Ok(())
    }
}
