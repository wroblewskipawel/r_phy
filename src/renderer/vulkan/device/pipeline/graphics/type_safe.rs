use std::{any::type_name, error::Error};

use ash::vk::{self, Extent2D};

use crate::renderer::vulkan::device::{
    pipeline::{get_pipeline_states_info, ModuleLoader, PipelineLayout},
    render_pass::RenderPassConfig,
    VulkanDevice,
};

use super::GraphicsPipelineConfig;

pub struct GraphicsPipeline<C: GraphicsPipelineConfig> {
    pub handle: vk::Pipeline,
    pub layout: PipelineLayout<C::Layout>,
}

impl<C: GraphicsPipelineConfig> From<&mut GraphicsPipeline<C>> for vk::Pipeline {
    fn from(pipeline: &mut GraphicsPipeline<C>) -> Self {
        pipeline.handle
    }
}

impl VulkanDevice {
    pub fn create_graphics_pipeline<C: GraphicsPipelineConfig>(
        &self,
        modules: impl ModuleLoader,
        extent: Extent2D,
    ) -> Result<GraphicsPipeline<C>, Box<dyn Error>> {
        let layout = self.get_pipeline_layout::<C::Layout>()?;
        let render_pass = self.get_render_pass::<C::RenderPass>()?;
        let states = get_pipeline_states_info::<C::Attachments, C::Subpass, C::PipelineStates>(
            &self.physical_device,
            extent,
        );
        let modules = modules.load(self)?;
        let stages = modules.get_stages_info();
        let subpass = C::RenderPass::try_get_subpass_index::<C::Subpass>().unwrap_or_else(|| {
            panic!(
                "Subpass {} not present in RenderPass {}!",
                type_name::<C::Subpass>(),
                type_name::<C::RenderPass>(),
            )
        }) as u32;
        let create_infos = [vk::GraphicsPipelineCreateInfo {
            subpass,
            layout: layout.layout,
            render_pass: render_pass.handle,
            p_vertex_input_state: &states.vertex_input.create_info,
            p_input_assembly_state: &states.input_assembly,
            p_viewport_state: &states.viewport.create_info,
            p_rasterization_state: &states.rasterization,
            p_depth_stencil_state: &states.depth_stencil,
            p_color_blend_state: &states.color_blend.create_info,
            p_multisample_state: &states.multisample,
            stage_count: stages.stages.len() as u32,
            p_stages: stages.stages.as_ptr(),
            ..Default::default()
        }];
        let &handle = unsafe {
            self.device
                .create_graphics_pipelines(vk::PipelineCache::null(), &create_infos, None)
                .map_err(|(_, err)| err)?
                .first()
                .unwrap()
        };
        Ok(GraphicsPipeline { handle, layout })
    }
}
