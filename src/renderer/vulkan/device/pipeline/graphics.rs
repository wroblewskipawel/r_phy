mod presets;

pub use presets::*;

use std::{any::type_name, error::Error, marker::PhantomData};

use ash::vk::{self, Extent2D};

use crate::renderer::vulkan::device::{
    framebuffer::Attachments,
    render_pass::{RenderPassConfig, Subpass},
    VulkanDevice,
};

use super::{
    get_pipeline_states_info,
    layout::{Layout, PipelineLayout},
    ModuleLoader, PipelineStates,
};

pub trait GraphicspipelineConfig {
    type Attachments: Attachments;
    type Layout: Layout;
    type PipelineStates: PipelineStates;
    type RenderPass: RenderPassConfig<Attachments = Self::Attachments>;
    type Subpass: Subpass<Self::Attachments>;

    fn builder() -> GraphicsPipelineBuilder<
        Self::Attachments,
        Self::Layout,
        Self::PipelineStates,
        Self::RenderPass,
        Self::Subpass,
    > {
        GraphicsPipelineBuilder::builder()
    }
}

pub struct GraphicsPipelineBuilder<
    A: Attachments,
    L: Layout,
    P: PipelineStates,
    R: RenderPassConfig<Attachments = A>,
    S: Subpass<A>,
> {
    _phantom: PhantomData<(L, P, R, S)>,
}

impl<
        A: Attachments,
        L: Layout,
        P: PipelineStates,
        R: RenderPassConfig<Attachments = A>,
        S: Subpass<A>,
    > GraphicspipelineConfig for GraphicsPipelineBuilder<A, L, P, R, S>
{
    type Attachments = A;
    type Layout = L;
    type PipelineStates = P;
    type RenderPass = R;
    type Subpass = S;
}

pub struct GraphicsPipeline<C: GraphicspipelineConfig> {
    pub handle: vk::Pipeline,
    pub layout: PipelineLayout<C::Layout>,
}

impl VulkanDevice {
    pub fn create_graphics_pipeline<C: GraphicspipelineConfig>(
        &self,
        modules: impl ModuleLoader,
        extent: Extent2D,
    ) -> Result<GraphicsPipeline<C>, Box<dyn Error>> {
        let layout = self.get_pipeline_layout::<C::Layout>()?;
        let render_pass = self.get_render_pass::<C::RenderPass>()?;
        let states = get_pipeline_states_info::<C::PipelineStates>(&self.physical_device, extent);
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

    pub fn destroy_graphics_pipeline<C: GraphicspipelineConfig>(
        &self,
        pipeline: &mut GraphicsPipeline<C>,
    ) {
        unsafe {
            self.device.destroy_pipeline(pipeline.handle, None);
        }
    }
}
