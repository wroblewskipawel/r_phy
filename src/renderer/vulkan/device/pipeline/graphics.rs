use std::{error::Error, marker::PhantomData, path::Path};

use ash::vk;

use crate::renderer::vulkan::device::{
    render_pass::VulkanRenderPass, swapchain::VulkanSwapchain, VulkanDevice,
};

use super::{
    layout::{Layout, PipelineLayout},
    ColorBlend, DepthStencil, Multisample, PipelineStates, Rasterization, VertexAssembly,
    VertexInput, Viewport,
};

pub struct GraphicsPipeline<L: Layout, S: PipelineStates> {
    pub handle: vk::Pipeline,
    pub layout: PipelineLayout<L>,
    _phantom: PhantomData<S>,
}

impl VulkanDevice {
    pub fn create_graphics_pipeline<L: Layout, S: PipelineStates>(
        &self,
        layout: L,
        _states: S,
        render_pass: &VulkanRenderPass,
        swapchain: &VulkanSwapchain,
        modules: &[&Path],
    ) -> Result<GraphicsPipeline<L, S>, Box<dyn Error>> {
        let layout = self.get_pipeline_layout(layout)?;
        let vertex_input = S::VertexInput::get_state();
        let input_assembly = S::VertexAssembly::get_input_assembly();
        let viewport = S::Viewport::get_state(swapchain.image_extent);
        let rasterization = S::Rasterization::get_state();
        let depth_stencil = S::DepthStencil::get_state();
        let color_blend = S::ColorBlend::get_state();
        let multisample = S::Multisample::get_state(
            &self.physical_device.properties,
            &self.physical_device.attachment_properties,
        );
        let modules = modules
            .iter()
            .map(|module_path| self.load_shader_module(module_path))
            .collect::<Result<Vec<_>, _>>()?;
        let stages = modules
            .iter()
            .map(|module| module.get_stage_create_info())
            .collect::<Vec<_>>();
        let create_infos = [vk::GraphicsPipelineCreateInfo {
            layout: layout.layout,
            render_pass: render_pass.into(),
            subpass: 0,
            p_vertex_input_state: &vertex_input.create_info,
            p_input_assembly_state: &input_assembly,
            p_viewport_state: &viewport.create_info,
            p_rasterization_state: &rasterization,
            p_depth_stencil_state: &depth_stencil,
            p_color_blend_state: &color_blend,
            p_multisample_state: &multisample,
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
        Ok(GraphicsPipeline {
            handle,
            layout,
            _phantom: PhantomData,
        })
    }

    pub fn destroy_graphics_pipeline<L: Layout, S: PipelineStates>(
        &self,
        pipeline: &mut GraphicsPipeline<L, S>,
    ) {
        unsafe {
            self.device.destroy_pipeline(pipeline.handle, None);
        }
    }
}
