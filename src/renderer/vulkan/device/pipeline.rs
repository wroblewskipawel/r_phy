use super::{render_pass::VulkanRenderPass, swapchain::VulkanSwapchain, VulkanDevice};
use ash::vk;
use std::{error::Error, ffi::CStr, mem::size_of, path::Path};

struct ShaderModule {
    module: vk::ShaderModule,
    stage: vk::ShaderStageFlags,
}

impl ShaderModule {
    const ENTRY_POINT: &'static CStr = unsafe { CStr::from_bytes_with_nul_unchecked(b"main\0") };

    fn get_stage_create_info(&self) -> vk::PipelineShaderStageCreateInfo {
        vk::PipelineShaderStageCreateInfo {
            module: self.module,
            stage: self.stage,
            p_name: Self::ENTRY_POINT.as_ptr(),
            ..Default::default()
        }
    }

    fn get_shader_stage(path: &Path) -> Result<vk::ShaderStageFlags, Box<dyn Error>> {
        match path.file_stem().map(|stem| stem.to_str().unwrap_or("")) {
            Some(stem) => match stem {
                "frag" => Ok(vk::ShaderStageFlags::FRAGMENT),
                "vert" => Ok(vk::ShaderStageFlags::VERTEX),
                stem => Err(format!(
                    "Invalid shader module path - unknown shader file type: {}!",
                    stem
                ))?,
            },
            None => Err("Invalid shader module path - mising file name component!")?,
        }
    }
}

pub struct GraphicsPipeline {
    handle: vk::Pipeline,
    layout: vk::PipelineLayout,
}

impl GraphicsPipeline {
    fn get_vertex_input_state() -> vk::PipelineVertexInputStateCreateInfo {
        const VERTEX_BINDINGS: &[vk::VertexInputBindingDescription] =
            &[vk::VertexInputBindingDescription {
                binding: 0,
                stride: (size_of::<(f32, f32, f32)>() * 2) as u32,
                input_rate: vk::VertexInputRate::VERTEX,
            }];
        const VERTEX_ATTRIBUTES: &[vk::VertexInputAttributeDescription] = &[
            vk::VertexInputAttributeDescription {
                location: 0,
                binding: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: 0,
            },
            vk::VertexInputAttributeDescription {
                location: 1,
                binding: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: size_of::<(f32, f32, f32)>() as u32,
            },
        ];
        vk::PipelineVertexInputStateCreateInfo {
            vertex_binding_description_count: VERTEX_BINDINGS.len() as u32,
            p_vertex_binding_descriptions: VERTEX_BINDINGS.as_ptr(),
            vertex_attribute_description_count: VERTEX_ATTRIBUTES.len() as u32,
            p_vertex_attribute_descriptions: VERTEX_ATTRIBUTES.as_ptr(),
            ..Default::default()
        }
    }
    fn get_input_assembly_state() -> vk::PipelineInputAssemblyStateCreateInfo {
        vk::PipelineInputAssemblyStateCreateInfo {
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            primitive_restart_enable: vk::FALSE,
            ..Default::default()
        }
    }
    fn get_viewport_state(
        image_extent: vk::Extent2D,
    ) -> (
        vk::PipelineViewportStateCreateInfo,
        Vec<vk::Viewport>,
        Vec<vk::Rect2D>,
    ) {
        let viewports = vec![vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: image_extent.width as f32,
            height: image_extent.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        }];
        let scissors = vec![vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: image_extent,
        }];
        let create_info = vk::PipelineViewportStateCreateInfo {
            viewport_count: viewports.len() as u32,
            p_viewports: viewports.as_ptr(),
            scissor_count: scissors.len() as u32,
            p_scissors: scissors.as_ptr(),
            ..Default::default()
        };
        (create_info, viewports, scissors)
    }
    fn get_rasterization_state() -> vk::PipelineRasterizationStateCreateInfo {
        vk::PipelineRasterizationStateCreateInfo {
            polygon_mode: vk::PolygonMode::FILL,
            cull_mode: vk::CullModeFlags::BACK,
            front_face: vk::FrontFace::COUNTER_CLOCKWISE,
            line_width: 1.0,
            ..Default::default()
        }
    }
    fn get_depth_stencil_state() -> vk::PipelineDepthStencilStateCreateInfo {
        vk::PipelineDepthStencilStateCreateInfo {
            depth_test_enable: vk::TRUE,
            depth_write_enable: vk::TRUE,
            depth_compare_op: vk::CompareOp::LESS_OR_EQUAL,
            ..Default::default()
        }
    }
    fn get_color_blend_state() -> vk::PipelineColorBlendStateCreateInfo {
        let attachments = VulkanRenderPass::get_color_attachments_blend_state();
        vk::PipelineColorBlendStateCreateInfo {
            attachment_count: attachments.len() as u32,
            p_attachments: attachments.as_ptr(),
            ..Default::default()
        }
    }

    fn get_multisample_state() -> vk::PipelineMultisampleStateCreateInfo {
        vk::PipelineMultisampleStateCreateInfo {
            rasterization_samples: vk::SampleCountFlags::TYPE_1,
            ..Default::default()
        }
    }
}

impl VulkanDevice {
    fn load_shader_module(&self, path: &Path) -> Result<ShaderModule, Box<dyn Error>> {
        let code = std::fs::read(path)?;
        let stage = ShaderModule::get_shader_stage(path)?;
        let create_info = vk::ShaderModuleCreateInfo {
            code_size: code.len(),
            p_code: code.as_ptr() as *const _,
            ..Default::default()
        };
        let module = unsafe { self.device.create_shader_module(&create_info, None)? };
        Ok(ShaderModule { module, stage })
    }

    pub fn create_graphics_pipeline(
        &self,
        render_pass: &VulkanRenderPass,
        swapchain: &VulkanSwapchain,
        modules: &[&Path],
    ) -> Result<GraphicsPipeline, Box<dyn Error>> {
        let layout = self.create_graphics_pipeline_layout()?;
        let vertex_input_state = GraphicsPipeline::get_vertex_input_state();
        let input_assembly_state = GraphicsPipeline::get_input_assembly_state();
        let (viewport_state, _viewports, _scissors) =
            GraphicsPipeline::get_viewport_state(swapchain.image_extent);
        let rasterization_state = GraphicsPipeline::get_rasterization_state();
        let depth_stencil_state = GraphicsPipeline::get_depth_stencil_state();
        let color_blend_state = GraphicsPipeline::get_color_blend_state();
        let multisample_state = GraphicsPipeline::get_multisample_state();
        let modules = modules
            .iter()
            .map(|module_path| self.load_shader_module(module_path))
            .collect::<Result<Vec<_>, _>>()?;
        let stages = modules
            .iter()
            .map(|module| module.get_stage_create_info())
            .collect::<Vec<_>>();
        let create_infos = [vk::GraphicsPipelineCreateInfo {
            layout,
            render_pass: render_pass.into(),
            subpass: 0,
            p_vertex_input_state: &vertex_input_state,
            p_input_assembly_state: &input_assembly_state,
            p_viewport_state: &viewport_state,
            p_rasterization_state: &rasterization_state,
            p_depth_stencil_state: &depth_stencil_state,
            p_color_blend_state: &color_blend_state,
            p_multisample_state: &multisample_state,
            stage_count: stages.len() as u32,
            p_stages: stages.as_ptr(),
            ..Default::default()
        }];
        let &handle = unsafe {
            self.device
                .create_graphics_pipelines(vk::PipelineCache::null(), &create_infos, None)
                .map_err(|(_, err)| err)?
                .first()
                .unwrap()
        };
        modules
            .into_iter()
            .for_each(|module| unsafe { self.device.destroy_shader_module(module.module, None) });
        Ok(GraphicsPipeline { handle, layout })
    }

    pub fn destory_graphics_pipeline(&self, pipeline: &mut GraphicsPipeline) {
        unsafe {
            self.device.destroy_pipeline(pipeline.handle, None);
            self.device.destroy_pipeline_layout(pipeline.layout, None);
        }
    }

    fn create_graphics_pipeline_layout(&self) -> Result<vk::PipelineLayout, Box<dyn Error>> {
        let create_info = vk::PipelineLayoutCreateInfo::default();
        let layout = unsafe { self.device.create_pipeline_layout(&create_info, None)? };
        Ok(layout)
    }
}
