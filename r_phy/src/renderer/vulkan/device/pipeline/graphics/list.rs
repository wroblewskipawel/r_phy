use std::error::Error;

use crate::renderer::{
    shader::{ShaderType, ShaderTypeList},
    vulkan::device::{pipeline::ModuleLoader, VulkanDevice},
};
use type_list::{Cons, Nil};

use super::{GraphicsPipelineConfig, PipelinePack, PipelinePackRef, PipelinePackRefMut};

// pub trait GraphicsPipelineTypeList: ShaderTypeList {
//     type Pipeline: GraphicsPipelineConfig;
// }

pub trait GraphicsPipelineListBuilder: ShaderTypeList {
    type Pack: GraphicsPipelinePackList;

    fn build(&self, device: &VulkanDevice) -> Result<Self::Pack, Box<dyn Error>>;
}

// impl GraphicsPipelineTypeList for Nil {
//     type Pipeline = EmptyPipeline;
// }

impl GraphicsPipelineListBuilder for Nil {
    type Pack = Nil;

    fn build(&self, _device: &VulkanDevice) -> Result<Self::Pack, Box<dyn Error>> {
        Ok(Nil {})
    }
}

// impl<T: GraphicsPipelineConfig + ShaderType, N: GraphicsPipelineTypeList> GraphicsPipelineTypeList
//     for Cons<Vec<T>, N>
// {
//     type Pipeline = T;
// }

impl<T: GraphicsPipelineConfig + ShaderType, N: ShaderTypeList> ShaderTypeList
    for Cons<PipelinePack<T>, N>
{
    const LEN: usize = N::LEN + 1;
    type Item = T;
    type Next = N;
}

// impl<T: GraphicsPipelineConfig + ShaderType, N: GraphicsPipelinePackList> GraphicsPipelineTypeList
//     for Cons<PipelinePack<T>, N>
// {
//     type Pipeline = T;
// }

impl<T: GraphicsPipelineConfig + ModuleLoader + ShaderType, N: GraphicsPipelineListBuilder>
    GraphicsPipelineListBuilder for Cons<Vec<T>, N>
{
    type Pack = Cons<PipelinePack<T>, N::Pack>;

    fn build(&self, device: &VulkanDevice) -> Result<Self::Pack, Box<dyn Error>> {
        let mut pack = device.create_pipeline_pack()?;
        device.load_pipelines(&mut pack, &self.head)?;
        Ok(Cons {
            head: pack,
            tail: self.tail.build(device)?,
        })
    }
}

pub trait GraphicsPipelinePackList: ShaderTypeList {
    fn destroy(&mut self, device: &VulkanDevice);

    fn try_get<P: GraphicsPipelineConfig>(&self) -> Option<PipelinePackRef<P>>;

    fn try_get_mut<P: GraphicsPipelineConfig>(&mut self) -> Option<PipelinePackRefMut<P>>;
}

impl GraphicsPipelinePackList for Nil {
    fn destroy(&mut self, _device: &VulkanDevice) {}

    fn try_get<P: GraphicsPipelineConfig>(&self) -> Option<PipelinePackRef<P>> {
        None
    }

    fn try_get_mut<P: GraphicsPipelineConfig>(&mut self) -> Option<PipelinePackRefMut<P>> {
        None
    }
}

impl<T: GraphicsPipelineConfig + ShaderType, N: GraphicsPipelinePackList> GraphicsPipelinePackList
    for Cons<PipelinePack<T>, N>
{
    fn destroy(&mut self, device: &VulkanDevice) {
        device.destory_pipeline_pack(&mut self.head);
        self.tail.destroy(device);
    }

    fn try_get<P: GraphicsPipelineConfig>(&self) -> Option<PipelinePackRef<P>> {
        if let Ok(pipelines) = (&self.head).try_into() {
            Some(pipelines)
        } else {
            self.tail.try_get::<P>()
        }
    }

    fn try_get_mut<P: GraphicsPipelineConfig>(&mut self) -> Option<PipelinePackRefMut<P>> {
        if let Ok(pipelines) = (&mut self.head).try_into() {
            Some(pipelines)
        } else {
            self.tail.try_get_mut::<P>()
        }
    }
}
