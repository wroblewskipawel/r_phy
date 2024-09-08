use std::error::Error;

use crate::{
    core::{Cons, Nil},
    renderer::vulkan::device::{pipeline::ModuleLoader, VulkanDevice},
};

use super::{
    EmptyPipeline, GraphicsPipelineConfig, PipelinePack, PipelinePackRef, PipelinePackRefMut,
};

pub trait GraphicsPipelineTypeList: 'static {
    const LEN: usize;
    type Item: GraphicsPipelineConfig;
    type Next: GraphicsPipelineTypeList;
}

pub trait GraphicsPipelineListBuilder: GraphicsPipelineTypeList {
    type Pack: GraphicsPipelinePackList;

    fn build(&self, device: &VulkanDevice) -> Result<Self::Pack, Box<dyn Error>>;
}

impl GraphicsPipelineTypeList for Nil {
    const LEN: usize = 0;
    type Item = EmptyPipeline;
    type Next = Nil;
}

impl GraphicsPipelineListBuilder for Nil {
    type Pack = Nil;

    fn build(&self, _device: &VulkanDevice) -> Result<Self::Pack, Box<dyn Error>> {
        Ok(Nil {})
    }
}

impl<T: GraphicsPipelineConfig, N: GraphicsPipelineListBuilder> GraphicsPipelineTypeList
    for Cons<Vec<T>, N>
{
    const LEN: usize = N::LEN + 1;
    type Item = T;
    type Next = N;
}

impl<T: GraphicsPipelineConfig, N: GraphicsPipelinePackList> GraphicsPipelineTypeList
    for Cons<PipelinePack<T>, N>
{
    const LEN: usize = N::LEN + 1;
    type Item = T;
    type Next = N;
}

impl<T: GraphicsPipelineConfig + ModuleLoader, N: GraphicsPipelineListBuilder>
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

pub trait GraphicsPipelinePackList: GraphicsPipelineTypeList {
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

impl<T: GraphicsPipelineConfig, N: GraphicsPipelinePackList> GraphicsPipelinePackList
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
