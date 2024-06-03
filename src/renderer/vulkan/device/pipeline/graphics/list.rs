use std::{any::TypeId, collections::HashMap, error::Error, path::PathBuf};

use crate::renderer::vulkan::device::{
    framebuffer::presets::AttachmentsGBuffer, pipeline::ShaderDirectory, VulkanDevice,
};

use super::{
    GBufferDepthPrepasPipeline, GraphicsPipeline, GraphicsPipelineConfig,
    GraphicsPipelineTypeErased,
};

pub trait GraphicsPipelineTypeList {
    const LEN: usize;
    type Pipeline: GraphicsPipelineConfig;
    type Next: GraphicsPipelineTypeList;
}

pub struct GraphicsPipelineTerminator;

impl GraphicsPipelineTypeList for GraphicsPipelineTerminator {
    const LEN: usize = 0;
    type Pipeline = GBufferDepthPrepasPipeline<AttachmentsGBuffer>;
    type Next = Self;
}

pub struct PipelineList {
    pipelines: Vec<GraphicsPipelineTypeErased>,
}

impl PipelineList {
    pub fn try_get<T: GraphicsPipelineConfig>(&self) -> Option<GraphicsPipeline<T>> {
        self.pipelines
            .iter()
            .find_map(|&pipeline| pipeline.try_into().ok())
    }
}

impl VulkanDevice {
    fn insert_pipeline<C: GraphicsPipelineTypeList>(
        &self,
        mut pipelines: Vec<GraphicsPipelineTypeErased>,
        shaders: &HashMap<TypeId, PathBuf>,
    ) -> Result<Vec<GraphicsPipelineTypeErased>, Box<dyn Error>> {
        if C::LEN > 0 {
            pipelines.push(
                self.create_graphics_pipeline::<C::Pipeline>(
                    ShaderDirectory::new(shaders.get(&TypeId::of::<C::Pipeline>()).unwrap()),
                    self.physical_device.surface_properties.get_current_extent(),
                )?
                .into(),
            );
            self.insert_pipeline::<C::Next>(pipelines, shaders)
        } else {
            Ok(pipelines)
        }
    }

    pub fn create_pipeline_list<C: GraphicsPipelineTypeList>(
        &self,
        shaders: &HashMap<TypeId, PathBuf>,
    ) -> Result<PipelineList, Box<dyn Error>> {
        Ok(PipelineList {
            pipelines: self.insert_pipeline::<C>(Vec::new(), shaders)?,
        })
    }
    pub fn destory_pipeline_list(&self, list: &mut PipelineList) {
        list.pipelines
            .iter_mut()
            .for_each(|pipeline| self.destroy_pipeline(pipeline));
    }
}
