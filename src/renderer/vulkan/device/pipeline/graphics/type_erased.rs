use std::any::TypeId;

use ash::vk;

use crate::renderer::vulkan::device::pipeline::PipelineLayoutRaw;

use super::{GraphicsPipeline, GraphicsPipelineConfig};

#[derive(Debug, Clone, Copy)]
pub struct GraphicsPipelineTypeErased {
    type_id: TypeId,
    pub handle: vk::Pipeline,
    pub layout: PipelineLayoutRaw,
}

impl From<&mut GraphicsPipelineTypeErased> for vk::Pipeline {
    fn from(pipeline: &mut GraphicsPipelineTypeErased) -> Self {
        pipeline.handle
    }
}

impl<C: GraphicsPipelineConfig> From<GraphicsPipeline<C>> for GraphicsPipelineTypeErased {
    fn from(pipeline: GraphicsPipeline<C>) -> Self {
        GraphicsPipelineTypeErased {
            type_id: TypeId::of::<C>(),
            handle: pipeline.handle,
            layout: pipeline.layout.into(),
        }
    }
}

impl<C: GraphicsPipelineConfig> TryFrom<GraphicsPipelineTypeErased> for GraphicsPipeline<C> {
    type Error = &'static str;

    fn try_from(value: GraphicsPipelineTypeErased) -> Result<Self, Self::Error> {
        if value.type_id == TypeId::of::<C>() {
            Ok(GraphicsPipeline {
                handle: value.handle,
                layout: value.layout.into(),
            })
        } else {
            Err("Invalid GraphicsPipelineConfig type!")
        }
    }
}
